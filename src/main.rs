use anyhow::{Context, Result};
use autonomous_platform::load_default_config;
use futures::StreamExt;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tokio::time::Duration as TokioDuration;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber;
use sqlx;
use chrono;

// Import existing DAA components
use autonomous_platform::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use autonomous_platform::integration::daa_coordinator::{DaaConfig, DaaCoordinator, TradingAction};
use autonomous_platform::integration::data_access::{DataAccessLayer, Timeframe};
use autonomous_platform::neural::NeuralPredictor;
use autonomous_platform::strategies::{Position, PositionSide, StrategyConfig, StrategyFactory};
use autonomous_platform::streaming::event_bus::{EventBusIntegration, MarketEvent};

// Import Redis adapter
use autonomous_platform::adapters::redis::{RedisAdapter, RedisConfig};
use autonomous_platform::adapters::DataAdapter;

// Import MarketHours and Exchange
use autonomous_platform::utils::market_hours::{MarketHours, Exchange};

// Import health monitoring
use autonomous_platform::monitoring::health::{
    AsyncHealthMonitor, HealthServer, HealthServerConfig, HealthMonitorConfig,
    ComponentType
};

/// Load initial historical data from the database and populate the event bus
async fn load_initial_historical_data(
    data_access: &Arc<DataAccessLayer>,
    event_bus: &Arc<EventBusIntegration>,
) -> Result<usize> {
    let symbols = vec!["AAPL", "NVDA", "MSFT", "GOOGL", "TSLA"];
    let mut total_loaded = 0;

    for symbol in symbols {
        // Get the latest 100 data points for each symbol
        match data_access.get_market_data(symbol, Timeframe::Hourly).await {
            Ok(market_data) => {
                for data_point in market_data.into_iter().take(100) {
                    // Convert to MarketEvent format
                    let market_event = MarketEvent {
                        symbol: data_point.symbol.clone(),
                        timestamp: data_point.timestamp,
                        event_type: "historical_data".to_string(),
                        price: data_point.close,
                        volume: data_point.volume_value,
                        bid: data_point.low,
                        ask: data_point.high,
                        spread: data_point.high - data_point.low,
                        order_book_depth: None,
                        sequence_number: data_point.timestamp.timestamp() as u64,
                        source: "historical_load".to_string(),
                        quality_score: 0.90,
                        metadata: Some(serde_json::json!({
                            "open": data_point.open,
                            "high": data_point.high,
                            "low": data_point.low,
                            "close": data_point.close,
                            "symbol": data_point.symbol
                        })),
                    };

                    // Publish to event bus
                    if let Err(e) = event_bus.publish_market_event(market_event).await {
                        warn!("Failed to publish historical market event for {}: {}", symbol, e);
                    } else {
                        total_loaded += 1;
                    }
                }
            }
            Err(e) => {
                warn!("Failed to load historical data for {}: {}", symbol, e);
            }
        }
    }

    Ok(total_loaded)
}

#[derive(Debug, sqlx::FromRow)]
struct HistoricalMarketData {
    pub bucket: chrono::DateTime<chrono::Utc>,
    pub symbol: String,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: Option<f64>,
    pub volume: Option<f64>,
}

/// Load historical data from TimescaleDB continuous aggregate and publish to event bus
async fn load_historical_data(
    storage: &Arc<TimescaleDBStorage>,
    event_bus: &Arc<EventBusIntegration>,
) -> Result<usize> {
    let end_time = chrono::Utc::now();
    let start_time = end_time - chrono::Duration::hours(4);

    info!("Querying market_data_1h from {} to {}", start_time.format("%Y-%m-%d %H:%M:%S UTC"), end_time.format("%Y-%m-%d %H:%M:%S UTC"));

    // Query the continuous aggregate market_data_1h for last 4 hours
    let rows = sqlx::query_as::<_, HistoricalMarketData>(
        r#"
        SELECT 
            bucket,
            symbol,
            open,
            high,
            low,
            close,
            volume::float8 as volume
        FROM market_data_1h
        WHERE bucket >= $1 AND bucket <= $2
        ORDER BY bucket DESC, symbol
        "#,
    )
    .bind(start_time)
    .bind(end_time)
    .fetch_all(&storage.pool)
    .await?;

    let mut loaded_count = 0;

    for row in rows {
        let symbol = row.symbol.clone();
        let market_event = MarketEvent {
            symbol: row.symbol,
            timestamp: row.bucket,
            event_type: "historical_market_update".to_string(),
            price: row.close.unwrap_or(0.0),
            volume: row.volume.unwrap_or(0.0),
            bid: row.low.unwrap_or(row.close.unwrap_or(0.0)),
            ask: row.high.unwrap_or(row.close.unwrap_or(0.0)),
            spread: (row.high.unwrap_or(0.0) - row.low.unwrap_or(0.0)),
            order_book_depth: None,
            sequence_number: row.bucket.timestamp() as u64,
            source: "historical_timescaledb".to_string(),
            quality_score: 0.90, // Historical data has slightly lower quality score
            metadata: Some(serde_json::json!({
                "open": row.open,
                "high": row.high,
                "low": row.low,
                "close": row.close,
                "volume": row.volume,
                "data_type": "historical",
                "source": "market_data_1h",
                "bucket": row.bucket.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            })),
        };

        // Publish to event bus
        if let Err(e) = event_bus.publish_market_event(market_event).await {
            error!("Failed to publish historical market event for {}: {}", symbol, e);
        } else {
            loaded_count += 1;
            debug!("Published historical data point for {} at {}", symbol, row.bucket);
        }
    }

    info!("Historical data loading completed - processed {} rows from market_data_1h", loaded_count);
    Ok(loaded_count)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Neural Trading Platform...");

    // Load configuration
    let config = load_default_config().context("Failed to load platform configuration")?;

    info!("Configuration loaded successfully");
    info!("   Database: {}", config.database.url);
    info!("   Redis: {}", config.redis.url);
    info!("   Neural Memory: {}GB", config.neural.memory_gb);
    info!("   Models: {:?}", config.neural.models);
    
    // Log feature flags
    info!("Feature Flags:");
    info!("   Enforce FANN Routing: {}", config.feature_flags.enable_enhanced_neural_adapter);
    info!("   Enable DAA Orchestration: {}", config.feature_flags.enable_performance_monitoring);

    // Check environment variables for neural system initialization
    let enable_sector_models = env::var("ENABLE_SECTOR_MODELS")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    let enable_autonomous_training = env::var("ENABLE_AUTONOMOUS_TRAINING")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    let enable_realtime_adaptation = env::var("ENABLE_REALTIME_ADAPTATION")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    let enable_data_discovery = env::var("ENABLE_DATA_DISCOVERY")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    // Log neural system configuration
    info!("Neural System Configuration:");
    info!("   Sector Models: {}", enable_sector_models);
    info!("   Autonomous Training: {}", enable_autonomous_training);
    info!("   Real-time Adaptation: {}", enable_realtime_adaptation);
    info!("   Data Discovery: {}", enable_data_discovery);
    
    // Initialize DAA components with proper error handling
    info!("Initializing neural predictor with VendorPredictor...");
    
    // Initialize neural predictor based on configuration
    let neural_predictor = if enable_sector_models {
        info!("Using VendorPredictor with sector model support");
        Arc::new(
            NeuralPredictor::with_vendor_predictor(config.neural.clone())
                .await
                .context("Failed to initialize VendorPredictor")?,
        )
    } else {
        info!("Using standard VendorPredictor");
        // Create required dependencies for VendorPredictor
        let sector_config = autonomous_platform::data::sector_mapper::SectorMapperConfig::default();
        let sector_mapper = Arc::new(
            autonomous_platform::data::sector_mapper::SectorMapper::new(sector_config)
        );
        let performance_tracker = Arc::new(
            autonomous_platform::monitoring::model_performance_tracker::ModelPerformanceTracker::new()
        );
        
        Arc::new(
            NeuralPredictor::new(&config.neural, sector_mapper, performance_tracker)
                .context("Failed to initialize neural predictor")?,
        )
    };
    
    // Initialize autonomous training if enabled
    if enable_autonomous_training {
        info!("Initializing autonomous training system...");
        if let Err(e) = neural_predictor.enable_autonomous_training().await {
            warn!("Failed to enable autonomous training: {}", e);
        } else {
            info!("✅ Autonomous training system initialized");
        }
    }
    
    // Initialize real-time adaptation if enabled
    if enable_realtime_adaptation {
        info!("Initializing real-time adaptation system...");
        if let Err(e) = neural_predictor.enable_realtime_adaptation().await {
            warn!("Failed to enable real-time adaptation: {}", e);
        } else {
            info!("✅ Real-time adaptation system initialized");
        }
    }
    
    // Initialize data discovery if enabled
    if enable_data_discovery {
        info!("Initializing data discovery system...");
        if let Err(e) = neural_predictor.enable_data_discovery().await {
            warn!("Failed to enable data discovery: {}", e);
        } else {
            info!("✅ Data discovery system initialized");
        }
    }

    info!("Initializing market hours tracker...");
    let market_hours = Arc::new(MarketHours::new());

    info!("Initializing DAA coordinator...");
    let daa_config = DaaConfig::default();
    let (decision_sender, mut decision_receiver) = tokio::sync::mpsc::channel(1000);
    let daa_coordinator = Arc::new(
        DaaCoordinator::new(daa_config, neural_predictor.clone(), decision_sender, market_hours.clone())
            .context("Failed to initialize DAA coordinator")?,
    );

    info!("Registering trading strategies...");

    // Register momentum strategy
    let momentum_config = StrategyConfig {
        name: "momentum".to_string(),
        enabled: true,
        risk_limit: 0.02,
        position_size: 0.1,
        parameters: HashMap::new(),
    };
    match StrategyFactory::create_strategy(&momentum_config, None) {
        Ok(mut momentum_strategy) => {
            // Initialize the strategy before registering
            if let Err(e) = momentum_strategy.initialize(momentum_config.clone()).await {
                error!("Failed to initialize momentum strategy: {}", e);
            } else {
                daa_coordinator
                    .register_strategy("momentum".to_string(), momentum_strategy)
                    .await;
                info!("Momentum strategy registered and initialized");
            }
        }
        Err(e) => {
            error!("Failed to create momentum strategy: {}", e);
        }
    }

    // Register neural-enhanced strategy
    let neural_config = StrategyConfig {
        name: "neural_enhanced".to_string(),
        enabled: true,
        risk_limit: 0.02,
        position_size: 0.1,
        parameters: HashMap::new(),
    };
    match StrategyFactory::create_strategy(&neural_config, Some(neural_predictor.clone())) {
        Ok(mut neural_strategy) => {
            // Initialize the strategy before registering
            if let Err(e) = neural_strategy.initialize(neural_config.clone()).await {
                error!("Failed to initialize neural-enhanced strategy: {}", e);
            } else {
                daa_coordinator
                    .register_strategy("neural_enhanced".to_string(), neural_strategy)
                    .await;
                info!("Neural-enhanced strategy registered and initialized");
            }
        }
        Err(e) => {
            error!("Failed to create neural-enhanced strategy: {}", e);
        }
    }

    info!("Initializing Redis adapter...");
    // Parse Redis URL to extract host, port, and password
    let redis_url = &config.redis.url;
    let (host, port, password) = if redis_url.starts_with("redis://") {
        let url = redis_url.trim_start_matches("redis://");
        let parts: Vec<&str> = url.split('@').collect();

        let (password, host_port) = if parts.len() > 1 {
            // Has authentication
            let auth = parts[0];
            let password = auth.split(':').nth(1).map(|p| p.to_string());
            (password, parts[1])
        } else {
            // No authentication
            (None, parts[0])
        };

        let host_port_parts: Vec<&str> = host_port.split(':').collect();
        let host = host_port_parts[0].to_string();
        let port = host_port_parts
            .get(1)
            .and_then(|p| p.split('/').next())
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(6379);

        (host, port, password)
    } else {
        // Default values if URL parsing fails
        ("localhost".to_string(), 6379, None)
    };

    let redis_config = RedisConfig {
        host,
        port,
        password,
        db: 0,
        pool_size: 10,
    };

    let mut redis_adapter = RedisAdapter::new(redis_config);
    redis_adapter
        .connect()
        .await
        .context("Failed to connect to Redis")?;
    let redis_adapter = Arc::new(redis_adapter);

    info!("Initializing storage components...");
    // Initialize TimescaleDB storage
    let storage = Arc::new(
        TimescaleDBStorage::new(&config.database.url)
            .await
            .context("Failed to initialize TimescaleDB storage")?,
    );

    // Initialize Redis cache
    let cache = Arc::new(
        RedisCache::new(&config.redis.url)
            .await
            .context("Failed to initialize Redis cache")?,
    );

    // Initialize Data Access Layer
    let data_access = Arc::new(
        DataAccessLayer::new(storage.clone(), cache.clone())
            .await
            .context("Failed to initialize data access layer")?,
    );

    info!("Initializing event bus...");
    let event_bus = Arc::new(
        EventBusIntegration::new(data_access.clone())
            .await
            .context("Failed to initialize event bus")?,
    );

    // Load historical data on startup
    info!("Loading historical data from market_data_1h...");
    match load_historical_data(&storage, &event_bus).await {
        Ok(count) => {
            info!("✅ Successfully loaded {} historical data points", count);
        }
        Err(e) => {
            warn!("Failed to load historical data: {}", e);
        }
    }

    info!("All DAA components initialized successfully");

    // Initialize health monitoring system
    info!("Initializing health monitoring system...");
    let health_config = HealthMonitorConfig::default();
    let mut async_health_monitor = AsyncHealthMonitor::new(health_config);
    
    // Register components for health monitoring
    async_health_monitor.register_component(ComponentType::Database).await?;
    async_health_monitor.register_component(ComponentType::Redis).await?;
    async_health_monitor.register_component(ComponentType::NeuralSystem).await?;
    async_health_monitor.register_component(ComponentType::DAAOrchestrator).await?;

    // Start health monitoring
    async_health_monitor.start().await?;
    info!("Health monitoring started successfully");

    // Initialize health server
    info!("Starting health server...");
    let health_server_config = HealthServerConfig {
        port: 9092,
        bind_address: "0.0.0.0".to_string(),
        request_timeout: std::time::Duration::from_secs(30),
    };
    
    let mut health_server = HealthServer::with_monitor(health_server_config, async_health_monitor);
    health_server.start().await?;
    info!("Health server started on http://0.0.0.0:9092");

    // Setup graceful shutdown handler
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);
    let neural_shutdown = neural_predictor.clone();

    // Spawn shutdown signal handler with MCP server panic fix
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received shutdown signal (Ctrl+C)");
                
                // Save checkpoints before shutdown
                info!("💾 Saving model checkpoints before shutdown...");
                for model_name in ["MLP", "NHITS", "DeepAR", "TCN", "Transformer"].iter() {
                    match neural_shutdown.save_checkpoint(model_name).await {
                        Ok(_) => info!("✅ Saved checkpoint for {}", model_name),
                        Err(e) => error!("❌ Failed to save checkpoint for {}: {}", model_name, e),
                    }
                }
                
                shutdown_clone.store(true, Ordering::Relaxed);
            }
            Err(err) => {
                error!("Failed to install CTRL+C signal handler: {}", err);
                // MCP server panic fix: Don't panic, just set shutdown signal
                shutdown_clone.store(true, Ordering::Relaxed);
            }
        }
    });

    // Start decision processing loop
    let _daa_clone = daa_coordinator.clone();
    let loop_signal = shutdown_signal.clone();
    tokio::spawn(async move {
        info!("Starting DAA decision processing loop...");
        while let Some(decision) = decision_receiver.recv().await {
            if loop_signal.load(Ordering::Relaxed) {
                break;
            }
            info!("DAA Decision: {:?}", decision);
            // Here decisions would be executed via trading adapters
        }
    });
    
    // Start hourly checkpoint saving during market hours
    let neural_checkpoint = neural_predictor.clone();
    let market_hours_checkpoint = market_hours.clone();
    let checkpoint_signal = shutdown_signal.clone();
    tokio::spawn(async move {
        info!("Starting hourly checkpoint saving during market hours...");
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 1 hour
        
        loop {
            interval.tick().await;
            if checkpoint_signal.load(Ordering::Relaxed) {
                break;
            }
            
            // Check if any major exchange is open
            let now = chrono::Utc::now();
            let nyse_open = market_hours_checkpoint.is_market_open(Exchange::NYSE, now).await;
            let nasdaq_open = market_hours_checkpoint.is_market_open(Exchange::NASDAQ, now).await;
            
            if nyse_open || nasdaq_open {
                info!("⏰ Performing hourly checkpoint save during market hours");
                for model_name in ["MLP", "NHITS", "DeepAR", "TCN", "Transformer"].iter() {
                    match neural_checkpoint.save_checkpoint(model_name).await {
                        Ok(_) => info!("✅ Saved hourly checkpoint for {}", model_name),
                        Err(e) => warn!("⚠️ Failed to save hourly checkpoint for {}: {}", model_name, e),
                    }
                }
            } else {
                debug!("Markets closed - skipping hourly checkpoint");
            }
        }
    });

    // Start Redis market data streaming loop
    let redis_clone = redis_adapter.clone();
    let event_bus_clone = event_bus.clone();
    let stream_signal = shutdown_signal.clone();
    tokio::spawn(async move {
        info!("Starting Redis market data streaming...");

        // PHASE 2: Multi-channel subscription with fair processing
        // Check if multi-channel mode is enabled via environment variable
        let enable_multi_channel = std::env::var("ENABLE_MULTI_CHANNEL")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";

        if enable_multi_channel {
            info!("Multi-channel mode enabled - starting symbol-specific subscriptions");
            
            // Multi-channel subscription for fair processing
            let symbols = vec!["AAPL", "NVDA", "MSFT", "GOOGL", "TSLA"];
            let mut subscription_handles = Vec::new();
            
            for symbol in symbols {
                let channel = format!("market:{}", symbol);
                let redis_for_symbol = redis_clone.clone();
                let event_bus_for_symbol = event_bus_clone.clone();
                let signal_for_symbol = stream_signal.clone();
                let symbol_name = symbol.to_string();
                
                info!("Starting subscription for symbol {} on channel {}", symbol, channel);
                
                let handle = tokio::spawn(async move {
                    match redis_for_symbol.subscribe_market_data(&channel).await {
                        Ok(mut stream) => {
                            info!("Successfully subscribed to channel {}", channel);
                            
                            while let Some(result) = stream.next().await {
                                if signal_for_symbol.load(Ordering::Relaxed) {
                                    break;
                                }
                                
                                match result {
                                    Ok(market_data) => {
                                        // Fair processing check - simple version
                                        // In full implementation, this would use FairProcessingScheduler
                                        
                                        // Convert to EventBus format
                                        let market_event = MarketEvent {
                                            symbol: symbol_name.clone(),
                                            timestamp: chrono::Utc::now(),
                                            event_type: "market_update".to_string(),
                                            price: market_data.close,
                                            volume: market_data.volume,
                                            bid: market_data.low,
                                            ask: market_data.high,
                                            spread: market_data.high - market_data.low,
                                            order_book_depth: None,
                                            sequence_number: market_data.timestamp as u64,
                                            source: format!("redis:{}", channel),
                                            quality_score: 0.95,
                                            metadata: Some(serde_json::json!({
                                                "open": market_data.open,
                                                "high": market_data.high,
                                                "low": market_data.low,
                                                "close": market_data.close,
                                                "channel": channel,
                                                "symbol": symbol_name
                                            })),
                                        };
                                        
                                        // Publish to event bus
                                        if let Err(e) = event_bus_for_symbol.publish_market_event(market_event).await {
                                            error!("Failed to publish market event for {}: {}", symbol_name, e);
                                        } else {
                                            debug!("Published market event for {} from channel {}", symbol_name, channel);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error receiving data from channel {}: {}", channel, e);
                                        // In production, implement reconnection logic here
                                    }
                                }
                            }
                            
                            info!("Subscription for {} stopped", symbol_name);
                        }
                        Err(e) => {
                            error!("Failed to subscribe to channel {}: {}", channel, e);
                        }
                    }
                });
                
                subscription_handles.push(handle);
            }
            
            // Keep all subscriptions alive until shutdown
            while !stream_signal.load(Ordering::Relaxed) {
                tokio::time::sleep(TokioDuration::from_secs(1)).await;
            }
            
            // Shutdown all subscriptions
            info!("Shutting down all symbol subscriptions");
            for handle in subscription_handles {
                handle.abort();
            }
            
        } else {
            info!("Legacy single-channel mode - subscribing to market:updates");
            
            // Legacy single-channel subscription
            match redis_clone.subscribe_market_data("market:updates").await {
            Ok(mut stream) => {
                info!("Subscribed to Redis market data channel");

                while let Some(result) = stream.next().await {
                    if stream_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    match result {
                        Ok(market_data) => {
                            // Convert to EventBus format
                            let market_event = MarketEvent {
                                symbol: market_data.symbol.clone(),
                                timestamp: chrono::Utc::now(),
                                event_type: "market_update".to_string(),
                                price: market_data.close,
                                volume: market_data.volume,
                                bid: market_data.low, // Simplified - using low as bid
                                ask: market_data.high, // Simplified - using high as ask
                                spread: market_data.high - market_data.low,
                                order_book_depth: None,
                                sequence_number: market_data.timestamp as u64,
                                source: "redis_legacy".to_string(),
                                quality_score: 0.95,
                                metadata: Some(serde_json::json!({
                                    "open": market_data.open,
                                    "high": market_data.high,
                                    "low": market_data.low,
                                    "close": market_data.close
                                })),
                            };

                            // Publish to event bus
                            if let Err(e) = event_bus_clone.publish_market_event(market_event).await {
                                error!("Failed to publish market event: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error receiving market data: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to subscribe to Redis market data: {}", e);
            }
            }
        }

        info!("Redis market data streaming stopped");
    });

    // Start the main DAA coordination loop
    let coordinator_clone = daa_coordinator.clone();
    let event_bus_for_daa = event_bus.clone();
    let coordination_signal = shutdown_signal.clone();
    tokio::spawn(async move {
        info!("Starting DAA coordination loop...");

        // Enable event bus performance monitoring
        event_bus_for_daa
            .enable_performance_monitoring(true)
            .await
            .ok();

        // Track current positions (in production, this would come from broker)
        let mut current_positions: HashMap<String, Position> = HashMap::new();

        // Route events to DAA agents
        loop {
            if coordination_signal.load(Ordering::Relaxed) {
                break;
            }

            // Get recent market events from event bus
            match event_bus_for_daa.get_published_events("market").await {
                Ok(market_events) => {
                    // Process only recent events (last 100)
                    let recent_events: Vec<_> = market_events.into_iter().rev().take(100).collect();

                    if !recent_events.is_empty() {
                        // Group events by symbol
                        let mut events_by_symbol: HashMap<String, Vec<_>> = HashMap::new();

                        for event in recent_events {
                            if let Some(symbol) =
                                event.payload.get("symbol").and_then(|s| s.as_str())
                            {
                                events_by_symbol
                                    .entry(symbol.to_string())
                                    .or_insert_with(Vec::new)
                                    .push(event);
                            }
                        }

                        // Process each symbol's data
                        for (symbol, events) in events_by_symbol {
                            // Convert DAA events to TimeSeriesData
                            let mut time_series_data: Vec<TimeSeriesData> = Vec::new();

                            for event in &events {
                                if let (Some(price), Some(volume)) = (
                                    event.payload.get("price").and_then(|v| v.as_f64()),
                                    event.payload.get("volume").and_then(|v| v.as_f64()),
                                ) {
                                    // Extract timestamp - it's stored as a string representation of DateTime
                                    let timestamp = if let Some(ts_val) =
                                        event.payload.get("timestamp")
                                    {
                                        if let Some(ts_str) = ts_val.as_str() {
                                            chrono::DateTime::parse_from_rfc3339(ts_str)
                                                .ok()
                                                .map(|dt| dt.timestamp())
                                                .unwrap_or_else(|| chrono::Utc::now().timestamp())
                                        } else {
                                            chrono::Utc::now().timestamp()
                                        }
                                    } else {
                                        chrono::Utc::now().timestamp()
                                    };
                                    // Extract OHLC data from payload
                                    let open = event
                                        .payload
                                        .get("open")
                                        .and_then(|o| o.as_f64())
                                        .unwrap_or(price);
                                    let high = event
                                        .payload
                                        .get("high")
                                        .and_then(|h| h.as_f64())
                                        .unwrap_or(price);
                                    let low = event
                                        .payload
                                        .get("low")
                                        .and_then(|l| l.as_f64())
                                        .unwrap_or(price);
                                    let close = event
                                        .payload
                                        .get("close")
                                        .and_then(|c| c.as_f64())
                                        .unwrap_or(price);

                                    let ts_data = TimeSeriesData {
                                        symbol: symbol.clone(),
                                        timestamp: chrono::DateTime::from_timestamp(timestamp, 0)
                                            .unwrap_or_else(chrono::Utc::now),
                                        open,
                                        high,
                                        low,
                                        close,
                                        volume: vec![volume],
                                        volume_value: volume,
                                        indicators: HashMap::new(),
                                        source: Some("event_bus".to_string()),
                                        entity: Some(symbol.clone()),
                                        value: Some(close),
                                        metadata: Some(serde_json::Value::Object(
                                            event.payload.clone().into_iter()
                                                .collect::<serde_json::Map<String, serde_json::Value>>()
                                        )),
                                        // Add required fields for vendor model integration
                                        values: vec![close],
                                        intervals: vec![60000], // Default to 1-minute intervals
                                        timestamps: vec![chrono::DateTime::from_timestamp(timestamp, 0)
                                            .unwrap_or_else(chrono::Utc::now)],
                                        metadata_map: HashMap::new(),
                                    };

                                    time_series_data.push(ts_data);
                                }
                            }

                            // Only process if we have enough data
                            if time_series_data.len() >= 10 {
                                // Sort by timestamp
                                time_series_data.sort_by_key(|d| d.timestamp);

                                // Get the latest data point for current market context
                                if let Some(latest) = time_series_data.last() {
                                    // Calculate simple volatility (standard deviation of returns)
                                    let mut returns = Vec::new();
                                    for i in 1..time_series_data.len() {
                                        let return_pct = (time_series_data[i].close
                                            - time_series_data[i - 1].close)
                                            / time_series_data[i - 1].close;
                                        returns.push(return_pct);
                                    }

                                    let avg_return =
                                        returns.iter().sum::<f64>() / returns.len() as f64;
                                    let volatility = (returns
                                        .iter()
                                        .map(|r| (r - avg_return).powi(2))
                                        .sum::<f64>()
                                        / returns.len() as f64)
                                        .sqrt();

                                    // Determine trend (simple moving average comparison)
                                    let recent_avg = time_series_data
                                        .iter()
                                        .rev()
                                        .take(5)
                                        .map(|d| d.close)
                                        .sum::<f64>()
                                        / 5.0;

                                    let older_avg = time_series_data
                                        .iter()
                                        .rev()
                                        .skip(5)
                                        .take(5)
                                        .map(|d| d.close)
                                        .sum::<f64>()
                                        / 5.0;

                                    let trend = if recent_avg > older_avg * 1.01 {
                                        "bullish"
                                    } else if recent_avg < older_avg * 0.99 {
                                        "bearish"
                                    } else {
                                        "neutral"
                                    };

                                    // Create MarketContext for DAA
                                    let market_context =
                                        autonomous_platform::strategies::MarketContext {
                                            symbol: symbol.clone(),
                                            current_price: latest.close,
                                            bid: latest.low, // Using low as bid approximation
                                            ask: latest.high, // Using high as ask approximation
                                            volume_24h: time_series_data
                                                .iter()
                                                .map(|d| d.volume_value)
                                                .sum::<f64>(),
                                            volatility: volatility * 100.0, // Convert to percentage
                                            timestamp: latest.timestamp.timestamp(),
                                        };

                                    // Get current position for this symbol
                                    let position = current_positions.get(&symbol);

                                    info!("Making DAA decision for {} - Price: ${:.2}, Trend: {}, Volatility: {:.2}%",
                                          symbol, latest.close, trend, volatility * 100.0);

                                    // Call DAA coordinator to make a decision
                                    match coordinator_clone
                                        .make_decision(&market_context, position, &time_series_data)
                                        .await
                                    {
                                        Ok(decision) => {
                                            info!(
                                                "DAA Decision for {}: {:?} (confidence: {:.2}%)",
                                                symbol,
                                                decision.action,
                                                decision.confidence * 100.0
                                            );

                                            // Update position tracking based on decision
                                            match &decision.action {
                                                TradingAction::Buy {
                                                    symbol: _, size, ..
                                                } => {
                                                    current_positions.insert(
                                                        symbol.clone(),
                                                        Position {
                                                            symbol: symbol.clone(),
                                                            side: PositionSide::Long,
                                                            size: *size,
                                                            entry_price: latest.close,
                                                            current_price: latest.close,
                                                            unrealized_pnl: 0.0,
                                                            timestamp: latest.timestamp.timestamp(),
                                                        },
                                                    );
                                                }
                                                TradingAction::Sell {
                                                    symbol: _, size, ..
                                                } => {
                                                    if *size > 0.0 {
                                                        current_positions.insert(
                                                            symbol.clone(),
                                                            Position {
                                                                symbol: symbol.clone(),
                                                                side: PositionSide::Short,
                                                                size: *size,
                                                                entry_price: latest.close,
                                                                current_price: latest.close,
                                                                unrealized_pnl: 0.0,
                                                                timestamp: latest
                                                                    .timestamp
                                                                    .timestamp(),
                                                            },
                                                        );
                                                    } else {
                                                        current_positions.remove(&symbol);
                                                    }
                                                }
                                                TradingAction::Hold { .. } => {
                                                    // No position change
                                                }
                                                TradingAction::AdjustPosition { .. } => {
                                                    // Position adjustment logic would go here
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to make DAA decision for {}: {}",
                                                symbol, e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get market events from event bus: {}", e);
                }
            }

            // Route events from event bus to DAA coordinator
            if let Err(e) = event_bus_for_daa.route_events_to_daa().await {
                error!("Failed to route events to DAA: {}", e);
            }

            // Store metrics in memory for coordination
            if let Err(e) = event_bus_for_daa
                .store_results_in_memory("daa_metrics_current")
                .await
            {
                error!("Failed to store metrics: {}", e);
            }

            // Sleep for a short interval before next iteration
            tokio::time::sleep(TokioDuration::from_millis(1000)).await;
        }
    });

    info!("Started");
    info!("Running");

    // Main application loop - wait for shutdown signal
    loop {
        // Check for shutdown signal
        if shutdown_signal.load(Ordering::Relaxed) {
            info!("Shutdown");
            break;
        }

        // Sleep briefly to prevent busy waiting
        tokio::time::sleep(TokioDuration::from_millis(100)).await;
    }

    // Gracefully shutdown health monitoring
    info!("Shutting down health monitoring...");
    // Note: health_server and async_health_monitor are consumed, so we can't shut them down here
    // In production, we would keep references to shut them down properly
    
    info!("Done");
    Ok(())
}
