use air_quality_app::{
    api::create_router,
    config::AppConfig,
    coordinator::{IngestionCoordinator, IngestionRouter, SourceManager},
    stream_integration::load_from_stream_config,
};
use config_client::StreamRegistry;
use neural_core::ParquetStore;
// DP-012: EventBus subscriber infrastructure
use neural_core::{
    BronzeSubscriber, BronzeSubscriberConfig, NoBronzeReader, Subscriber, SubscriberCoordinator,
};
// DP-012 Phase 4: SilverSubscriber for real-time Bronze-to-Silver ETL
use neural_core::config::SilverEtlConfig;
use neural_core::silver::outputs::{SilverOutput, TimescaleConfig, TimescaleOutput};
use neural_core::subscribers::{SilverSubscriber, SilverSubscriberConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "air_quality_app=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration with priority: StreamRegistry > etcd > config.yaml > defaults
    let etcd_endpoint =
        std::env::var("ETCD_ENDPOINT").unwrap_or_else(|_| "http://localhost:2379".to_string());

    let config = match load_from_stream_config(&[&etcd_endpoint], "air-quality").await {
        Ok(stream_config) => {
            tracing::info!("Loaded configuration from /streams/air-quality (GitOps)");
            stream_config
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load stream config: {}. Trying legacy /air-quality...",
                e
            );

            // Fallback to legacy etcd
            match air_quality_app::load_from_etcd().await {
                Ok(etcd_config) => {
                    tracing::info!("Loaded configuration from etcd");
                    // Convert EtcdAppConfig to AppConfig
                    AppConfig {
                        server: air_quality_app::config::ServerConfig {
                            host: etcd_config.server.host,
                            port: etcd_config.server.port,
                        },
                        mqtt: air_quality_app::config::MqttConfig {
                            broker_url: etcd_config.mqtt.broker_url,
                            port: etcd_config.mqtt.port,
                            client_id: etcd_config.mqtt.client_id,
                            topic_pattern: etcd_config.mqtt.topic_pattern,
                            subscriptions: etcd_config
                                .mqtt
                                .subscriptions
                                .into_iter()
                                .map(|s| air_quality_app::config::SubscriptionConfig {
                                    stream_id: s.stream_id,
                                    topic_pattern: s.topic_pattern,
                                    enabled: s.enabled,
                                })
                                .collect(),
                            qos: etcd_config.mqtt.qos,
                            reconnect_delay_secs: etcd_config.mqtt.reconnect_delay_secs,
                            max_reconnect_delay_secs: etcd_config.mqtt.max_reconnect_delay_secs,
                            buffer_capacity: etcd_config.mqtt.buffer_capacity,
                            default_stream_id: etcd_config.mqtt.default_stream_id,
                        },
                        storage: air_quality_app::config::StorageConfig {
                            base_path: etcd_config.storage.base_path,
                            wal_enabled: etcd_config.storage.wal_enabled,
                            batch_size: etcd_config.storage.batch_size,
                            batch_timeout_secs: etcd_config.storage.batch_timeout_secs,
                        },
                        // DP-012: These are loaded from env vars via apply_env_overrides
                        event_notifier: None,
                        threshold_processor: None,
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load config from etcd: {}. Trying config.yaml...",
                        e
                    );
                    match AppConfig::from_yaml("config.yaml") {
                        Ok(cfg) => {
                            tracing::info!("Loaded configuration from config.yaml");
                            cfg
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load config.yaml: {}, using defaults with env overrides",
                                e
                            );
                            AppConfig::default_config()
                        }
                    }
                }
            }
        }
    };

    tracing::info!(
        "Starting air quality server on {}:{}",
        config.server.host,
        config.server.port
    );

    // Initialize real ParquetStore
    tracing::info!("Initializing ParquetStore at: {}", config.storage.base_path);
    let store = Arc::new(ParquetStore::new(&config.storage.base_path)?);

    // Replay WAL on startup for crash recovery
    if config.storage.wal_enabled {
        tracing::info!("Replaying WAL for crash recovery...");
        match store.replay_wal().await {
            Ok(_) => tracing::info!("WAL replay completed successfully"),
            Err(e) => tracing::warn!("WAL replay failed (may be empty): {}", e),
        }
    }

    // Note: Legacy MQTT path removed. MQTT is now managed by IngestionCoordinator
    // and routes through IngestionRouter for proper stream_id tagging.

    // ========== AIR-005: Config Sync - Sync YAML configs to etcd ==========
    // Sync stream configurations from GitOps YAML files to etcd StreamRegistry
    let config_dir = std::env::var("STREAM_CONFIG_DIR")
        .unwrap_or_else(|_| "/workspaces/neural-data-platform/config/base/streams".to_string());

    if std::path::Path::new(&config_dir).exists() {
        tracing::info!("Syncing stream configs from {}", config_dir);
        let sync_service = air_quality_app::config_sync::ConfigSyncService::new(&config_dir);

        // Create a temporary registry for syncing
        match config_client::StreamRegistry::new(&[&etcd_endpoint]).await {
            Ok(registry) => match sync_service.sync_all(&registry).await {
                Ok(count) => {
                    tracing::info!(
                        "Synced {} stream configs to etcd (AIR-005 config sync)",
                        count
                    );
                }
                Err(e) => {
                    tracing::warn!("Config sync failed: {}. Using existing etcd configs.", e);
                }
            },
            Err(e) => {
                tracing::warn!("Failed to connect to registry for config sync: {}", e);
            }
        }
    } else {
        tracing::warn!(
            "Stream config directory not found: {}. Skipping config sync.",
            config_dir
        );
    }

    // ========== AIR-005: Multi-Stream Coordinator - ALL SOURCES (MQTT + HTTP) ==========
    // Initialize the multi-stream ingestion coordinator for all data sources
    // MQTT now routes through IngestionRouter for proper stream_id tagging
    //
    // DP-012: Also initializes EventBus and SubscriberCoordinator for multi-consumer
    // event broadcasting. BronzeSubscriber writes to Parquet via EventBus.
    let (coordinator_task, subscriber_task) =
        match initialize_multi_stream_coordinator(&etcd_endpoint, store.clone()).await {
            Ok((_coordinator, coord_task, sub_task)) => {
                tracing::info!(
                    "Multi-stream coordinator initialized - managing all sources (MQTT + HTTP)"
                );
                tracing::info!("DP-012: SubscriberCoordinator started with BronzeSubscriber");
                (Some(coord_task), Some(sub_task))
            }
            Err(e) => {
                tracing::warn!(
                    "Multi-stream coordinator not available: {}. All data sources disabled.",
                    e
                );
                (None, None)
            }
        };

    // Create mock source and forecast (these will be replaced in future tasks)
    let services = create_services_with_real_store(store.clone());

    // Create router
    let app = create_router(services);

    // Create TCP listener
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Listening on {}", addr);

    // Set up graceful shutdown handler
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn task to handle shutdown signals
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        tracing::info!("Received shutdown signal");
        let _ = shutdown_tx.send(());
    });

    // Start server with graceful shutdown
    tracing::info!("Server running. Press Ctrl+C to gracefully shutdown.");

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = &mut shutdown_rx => {
            tracing::info!("Starting graceful shutdown...");

            // Wait for background tasks to complete
            if let Some(task) = coordinator_task {
                tracing::debug!("Waiting for ingestion coordinator to stop...");
                let _ = task.await;
            }

            // DP-012: Wait for subscriber coordinator to stop
            if let Some(task) = subscriber_task {
                tracing::debug!("Waiting for subscriber coordinator to stop...");
                let _ = task.await;
            }

            tracing::info!("All background tasks completed. Shutdown complete.");
        }
    }

    Ok(())
}

/// Initialize the multi-stream ingestion coordinator (AIR-005)
///
/// This sets up HTTP polling sources for external APIs like OpenWeatherMap
/// which provide outdoor weather and air quality data.
///
/// DP-012: Also initializes EventBus and SubscriberCoordinator with BronzeSubscriber
/// for multi-consumer event broadcasting. The subscriber coordinator runs alongside
/// the existing mpsc-based storage pipeline.
async fn initialize_multi_stream_coordinator(
    etcd_endpoint: &str,
    store: Arc<ParquetStore>,
) -> Result<
    (
        Arc<IngestionCoordinator>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Initialize StreamRegistry for loading stream configurations
    let registry = Arc::new(
        StreamRegistry::new(&[etcd_endpoint])
            .await
            .map_err(|e| format!("Failed to create StreamRegistry: {}", e))?,
    );

    // Check if we have any HTTP polling streams configured
    let streams = registry.list_streams().await.unwrap_or_default();
    let has_http_streams = streams
        .iter()
        .any(|s| s.contains("weather") || s.contains("air-quality"));

    if !has_http_streams && streams.is_empty() {
        return Err("No streams configured in registry".into());
    }

    tracing::info!("Found {} stream configurations", streams.len());

    // Create dead letter channel for invalid points
    let (dead_letter_tx, mut dead_letter_rx) =
        mpsc::channel::<air_quality_app::coordinator::DeadLetterItem>(100);

    // Spawn dead letter handler
    tokio::spawn(async move {
        while let Some(item) = dead_letter_rx.recv().await {
            tracing::warn!(
                "Dead letter: stream={}, source={}, error={}",
                item.stream_id,
                item.source_id,
                item.error
            );
        }
    });

    // Create ingestion router (for dead letter handling)
    let router = Arc::new(IngestionRouter::new(registry.clone(), dead_letter_tx));

    // ==========================================================================
    // DP-012 FULL INTEGRATION: EventBus is the SOLE data flow mechanism
    // ==========================================================================
    // REMOVED: mpsc channel and RawStorageWriter
    // Sources now publish directly to EventBus (set during coordinator.start())
    // BronzeSubscriber handles all Bronze layer writes via EventBus subscription

    // Create source manager (EventBus will be set during coordinator.start())
    let source_manager = Arc::new(RwLock::new(SourceManager::new(registry.clone())));

    // Create coordinator
    let coordinator = Arc::new(IngestionCoordinator::new(
        router,
        source_manager,
        1000, // buffer size
    ));

    // Start coordinator
    coordinator
        .start()
        .await
        .map_err(|e| format!("Failed to start coordinator: {}", e))?;

    tracing::info!("Multi-stream coordinator started successfully");

    // Create monitoring task
    let coord_clone = coordinator.clone();
    let monitor_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if coord_clone.is_running().await {
                let health = coord_clone.get_source_health().await;
                tracing::debug!("Coordinator health: {} sources active", health.len());
            } else {
                tracing::warn!("Coordinator stopped unexpectedly");
                break;
            }
        }
    });

    // ==========================================================================
    // DP-012: EventBus Subscriber Infrastructure
    // ==========================================================================
    // Create SubscriberCoordinator with EventBus from IngestionCoordinator.
    // This is ADDITIVE - the existing mpsc-based RawStorageWriter continues to work.
    // The EventBus broadcasts events to subscribers for parallel processing.
    //
    // NOTE: BronzeSubscriber is currently disabled because sources do not yet
    // publish to EventBus. Phase 2 will add EventBus publishing to SourceManager.
    // For now, we just set up the infrastructure.

    let event_bus = coordinator.event_bus();
    let mut subscriber_coordinator = SubscriberCoordinator::new(event_bus);

    // Create BronzeSubscriber with default config
    // This subscriber will write to the same ParquetStore as RawStorageWriter
    // once sources publish to EventBus.
    let bronze_config = BronzeSubscriberConfig {
        batch_size: 50,
        flush_interval_secs: 30,
        max_retries: 3,
        stream_filter: Vec::new(), // Accept all streams
    };
    let bronze_subscriber = BronzeSubscriber::new("bronze-parquet", bronze_config, store.clone());

    // Register subscriber with coordinator
    if let Err(e) = subscriber_coordinator.register(Box::new(bronze_subscriber)) {
        tracing::warn!("Failed to register BronzeSubscriber: {}", e);
    } else {
        tracing::info!("BronzeSubscriber registered with SubscriberCoordinator");
    }

    // DP-012: SilverSubscriber deferred to future PR - EventBus wiring complete

    // Start subscriber coordinator (spawns subscriber tasks)
    if let Err(e) = subscriber_coordinator.start_all().await {
        tracing::warn!("Failed to start SubscriberCoordinator: {}", e);
    } else {
        tracing::info!(
            "SubscriberCoordinator started with {} subscribers",
            subscriber_coordinator.subscriber_count()
        );
    }

    // Create subscriber monitoring task
    let subscriber_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let health = subscriber_coordinator.health_check().await;
            if health.overall_healthy {
                tracing::debug!(
                    "SubscriberCoordinator health: {} running, {} total",
                    health.running_count,
                    health.subscriber_count
                );
            } else {
                tracing::warn!(
                    "SubscriberCoordinator unhealthy: {} running of {} total",
                    health.running_count,
                    health.subscriber_count
                );
            }
        }
    });

    Ok((coordinator, monitor_task, subscriber_task))
}

/// Create services with real ParquetStore
/// Note: Source and Forecast still use mock implementations (to be replaced in future tasks)
fn create_services_with_real_store(
    store: Arc<ParquetStore>,
) -> air_quality_app::api::routes::AppServices {
    use air_quality_app::api::handlers::alerts::AlertStore;
    use air_quality_app::api::handlers::locations::LocationStore;
    use neural_core::{Forecast, Source};

    // Mock source (still needed for health endpoint until we have a real implementation)
    struct MockSource;

    #[async_trait::async_trait]
    impl Source for MockSource {
        async fn fetch(&self) -> Result<Vec<neural_core::TimeSeriesPoint>, neural_core::CoreError> {
            Ok(vec![])
        }

        async fn health_check(&self) -> Result<neural_core::HealthStatus, neural_core::CoreError> {
            Ok(neural_core::HealthStatus {
                healthy: true,
                message: "Mock source operational (MQTT handler running separately)".to_string(),
                details: std::collections::HashMap::new(),
            })
        }
    }

    // Mock forecast (to be replaced in future tasks)
    struct MockForecast;

    #[async_trait::async_trait]
    impl Forecast for MockForecast {
        async fn train(
            &mut self,
            _data: Vec<neural_core::TimeSeriesPoint>,
        ) -> Result<neural_core::ModelMetrics, neural_core::CoreError> {
            Ok(neural_core::ModelMetrics {
                mae: 0.0,
                rmse: 0.0,
                mape: 0.0,
            })
        }

        async fn predict(
            &self,
            _location_id: &str,
            _horizon: usize,
        ) -> Result<Vec<neural_core::ForecastedPoint>, neural_core::CoreError> {
            Ok(vec![])
        }

        async fn evaluate(
            &self,
            _actual: Vec<neural_core::TimeSeriesPoint>,
        ) -> Result<neural_core::ModelMetrics, neural_core::CoreError> {
            Ok(neural_core::ModelMetrics {
                mae: 0.0,
                rmse: 0.0,
                mape: 0.0,
            })
        }
    }

    air_quality_app::api::routes::AppServices {
        store, // Using real ParquetStore!
        source: Arc::new(MockSource),
        forecast: Arc::new(MockForecast),
        alert_store: Arc::new(AlertStore::new()),
        location_store: Arc::new(LocationStore::new()),
    }
}

// ==========================================================================
// DP-012 Phase 4: SilverSubscriber Creation Helper
// ==========================================================================

/// Create SilverSubscribers for streams with enabled silver_etl configuration.
/// If TimescaleDB is unavailable, returns error and caller continues without Silver.
#[allow(dead_code)]
async fn create_silver_subscribers(
    _event_bus: Arc<neural_core::EventBus>,
    registry: Arc<StreamRegistry>,
) -> Result<Vec<Box<dyn Subscriber>>, Box<dyn std::error::Error + Send + Sync>> {
    let timescale_url = std::env::var("TIMESCALE_URL")
        .map_err(|_| "TIMESCALE_URL environment variable not set")?;

    let mut table_mapping = HashMap::new();
    table_mapping.insert("air-quality".to_string(), "air_quality_observations".to_string());
    table_mapping.insert("outdoor-weather".to_string(), "weather_observations".to_string());
    table_mapping.insert("outdoor-air-quality".to_string(), "outdoor_air_quality".to_string());
    table_mapping.insert("nws-observations".to_string(), "weather_observations".to_string());
    table_mapping.insert("nws-gridpoints-forecast".to_string(), "weather_forecasts".to_string());

    let timescale_config = TimescaleConfig {
        connection_string: timescale_url,
        max_connections: std::env::var("TIMESCALE_MAX_CONNECTIONS")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(5),
        connection_timeout_secs: std::env::var("TIMESCALE_TIMEOUT_SECS")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(10),
        default_table: "silver.observations".to_string(),
        table_mapping,
        timestamp_column: "observation_time".to_string(),
        use_upsert: true,
    };

    tracing::info!("Attempting to connect to TimescaleDB for Silver layer");
    let timescale_output: Arc<TimescaleOutput> = match TimescaleOutput::new(timescale_config).await {
        Ok(output) => {
            match output.health_check().await {
                Ok(true) => {
                    tracing::info!("TimescaleDB connection established successfully");
                    Arc::new(output)
                }
                Ok(false) => return Err("TimescaleDB health check failed".into()),
                Err(e) => return Err(format!("TimescaleDB health check error: {}", e).into()),
            }
        }
        Err(e) => return Err(format!("Failed to create TimescaleDB connection: {}", e).into()),
    };

    let config_dir = std::env::var("STREAM_CONFIG_DIR")
        .unwrap_or_else(|_| "/workspaces/neural-data-platform/config/base/streams".to_string());

    let streams = registry.list_streams().await.unwrap_or_default();
    let mut subscribers: Vec<Box<dyn Subscriber>> = Vec::new();

    for stream_id in streams {
        if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, &stream_id).await {
            if silver_config.enabled {
                tracing::debug!(stream_id = %stream_id, "Found enabled silver_etl config");

                let mut etl_configs = std::collections::HashMap::new();
                etl_configs.insert(stream_id.clone(), silver_config);

                let subscriber_config = SilverSubscriberConfig {
                    subscriber_id: format!("silver-{}", stream_id),
                    stream_filter: std::collections::HashSet::from([stream_id.clone()]),
                    etl_configs,
                    ..Default::default()
                };

                // Use NoBronzeReader as we don't support catch-up yet
                let subscriber: SilverSubscriber<TimescaleOutput, NoBronzeReader> =
                    SilverSubscriber::new(subscriber_config, timescale_output.clone());
                subscribers.push(Box::new(subscriber));
            }
        }
    }

    if subscribers.is_empty() {
        tracing::info!("No streams with enabled silver_etl configuration found");
    } else {
        tracing::info!("Created {} SilverSubscribers", subscribers.len());
    }

    Ok(subscribers)
}

#[allow(dead_code)]
async fn load_silver_etl_config(
    config_dir: &str,
    stream_id: &str,
) -> Result<Option<SilverEtlConfig>, Box<dyn std::error::Error + Send + Sync>> {
    use std::path::Path;

    let dir_path = Path::new(config_dir).join(stream_id).join("config.yaml");
    let flat_path = Path::new(config_dir).join(format!("{}.yaml", stream_id));

    let yaml_path = if dir_path.exists() {
        dir_path
    } else if flat_path.exists() {
        flat_path
    } else {
        return Ok(None);
    };

    let contents = tokio::fs::read_to_string(&yaml_path).await?;

    #[derive(serde::Deserialize)]
    struct StreamConfigWithSilver {
        #[serde(default)]
        silver_etl: Option<SilverEtlConfig>,
    }

    let config: StreamConfigWithSilver = serde_yaml::from_str(&contents)?;
    Ok(config.silver_etl)
}
