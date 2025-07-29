use anyhow::{Context, Result};
use autonomous_platform::load_default_config;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, Level};
use tracing_subscriber;

// Import existing DAA components
use autonomous_platform::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use autonomous_platform::integration::daa_coordinator::{DaaConfig, DaaCoordinator, TradingAction};
use autonomous_platform::integration::data_access::DataAccessLayer;
use autonomous_platform::neural::NeuralPredictor;
use autonomous_platform::strategies::{Position, PositionSide, StrategyConfig, StrategyFactory};
use autonomous_platform::streaming::event_bus::{EventBusIntegration, MarketEvent};

// Import Redis adapter
use autonomous_platform::adapters::redis::{RedisAdapter, RedisConfig};
use autonomous_platform::adapters::DataAdapter;

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

    // Initialize DAA components
    info!("Initializing neural predictor...");
    let neural_predictor = Arc::new(
        NeuralPredictor::new(config.neural.clone())
            .context("Failed to initialize neural predictor")?,
    );

    info!("Initializing DAA coordinator...");
    let daa_config = DaaConfig::default();
    let (decision_sender, mut decision_receiver) = tokio::sync::mpsc::channel(1000);
    let daa_coordinator = Arc::new(
        DaaCoordinator::new(daa_config, neural_predictor.clone(), decision_sender)
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

    info!("All DAA components initialized successfully");

    // Setup graceful shutdown handler
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received shutdown signal (Ctrl+C)");
                shutdown_clone.store(true, Ordering::Relaxed);
            }
            Err(err) => {
                error!("Failed to install CTRL+C signal handler: {}", err);
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

    // Start Redis market data streaming loop
    let redis_clone = redis_adapter.clone();
    let event_bus_clone = event_bus.clone();
    let stream_signal = shutdown_signal.clone();
    tokio::spawn(async move {
        info!("Starting Redis market data streaming...");

        // Subscribe to market data channel
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
                                source: "redis".to_string(),
                                quality_score: 0.95,
                                metadata: Some(serde_json::json!({
                                    "open": market_data.open,
                                    "high": market_data.high,
                                    "low": market_data.low,
                                    "close": market_data.close
                                })),
                            };

                            // Publish to event bus
                            if let Err(e) = event_bus_clone.publish_market_event(market_event).await
                            {
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
                                        volume,
                                        indicators: HashMap::new(),
                                        source: Some("event_bus".to_string()),
                                        entity: Some(symbol.clone()),
                                        value: Some(close),
                                        metadata: Some(serde_json::Value::Object(
                                            event.payload.clone().into_iter()
                                                .collect::<serde_json::Map<String, serde_json::Value>>()
                                        )),
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
                                                .map(|d| d.volume)
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
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
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
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    info!("Done");
    Ok(())
}
