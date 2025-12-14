use air_quality_app::{
    api::create_router,
    config::AppConfig,
    ingestion::MqttHandler,
    pipeline::StorageWriter,
};
use neural_core::{MqttConfig, ParquetStore};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
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

    // Load configuration with priority: etcd > env vars > config.yaml > defaults
    let config = match air_quality_app::load_from_etcd().await {
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
                    qos: etcd_config.mqtt.qos,
                    reconnect_delay_secs: etcd_config.mqtt.reconnect_delay_secs,
                    max_reconnect_delay_secs: etcd_config.mqtt.max_reconnect_delay_secs,
                    buffer_capacity: etcd_config.mqtt.buffer_capacity,
                },
                storage: air_quality_app::config::StorageConfig {
                    base_path: etcd_config.storage.base_path,
                    wal_enabled: etcd_config.storage.wal_enabled,
                    batch_size: etcd_config.storage.batch_size,
                    batch_timeout_secs: etcd_config.storage.batch_timeout_secs,
                },
            }
        }
        Err(e) => {
            tracing::warn!("Failed to load config from etcd: {}. Trying config.yaml...", e);
            match AppConfig::from_yaml("config.yaml") {
                Ok(cfg) => {
                    tracing::info!("Loaded configuration from config.yaml");
                    cfg
                }
                Err(e) => {
                    tracing::warn!("Failed to load config.yaml: {}, using defaults with env overrides", e);
                    AppConfig::default_config()
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

    // Create channel for MQTT -> Storage pipeline
    let (tx, rx) = mpsc::channel(config.mqtt.buffer_capacity);

    // Create MqttConfig from AppConfig
    let mqtt_config = MqttConfig {
        broker_url: config.mqtt.broker_url.clone(),
        port: config.mqtt.port,
        client_id: config.mqtt.client_id.clone(),
        topic_pattern: config.mqtt.topic_pattern.clone(),
        qos: config.mqtt.get_qos(),
        reconnect_delay: config.mqtt.get_reconnect_delay(),
        max_reconnect_delay: config.mqtt.get_max_reconnect_delay(),
        buffer_capacity: config.mqtt.buffer_capacity,
    };

    // Initialize MQTT handler (may fail if broker not available)
    let mqtt_handler = match MqttHandler::new(mqtt_config.clone(), tx.clone()).await {
        Ok(handler) => {
            tracing::info!("MQTT handler initialized successfully");
            Some(handler)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to initialize MQTT handler: {}. Running in degraded mode (no ingestion)",
                e
            );
            None
        }
    };

    // Create StorageWriter
    let storage_writer = StorageWriter::new(
        store.clone(),
        rx,
        Some(100), // batch size
        Some(Duration::from_secs(5)), // batch timeout
    );

    // Spawn storage writer background task
    let storage_task = tokio::spawn(async move {
        if let Err(e) = storage_writer.run().await {
            tracing::error!("Storage writer failed: {}", e);
        }
    });

    // Spawn MQTT ingestion background task if handler was initialized
    let ingestion_task = if let Some(handler) = mqtt_handler {
        Some(tokio::spawn(async move {
            if let Err(e) = handler.run().await {
                tracing::error!("MQTT handler failed: {}", e);
            }
        }))
    } else {
        None
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

            // Close the channel to signal shutdown to storage writer
            drop(tx);

            // Wait for background tasks to complete
            if let Some(task) = ingestion_task {
                let _ = task.await;
            }
            let _ = storage_task.await;

            tracing::info!("All background tasks completed. Shutdown complete.");
        }
    }

    Ok(())
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
