//! Integration tests for main.rs DAA flow
//!
//! Tests the complete integration flow from Redis -> EventBus -> DAA
//! focusing on the main.rs implementation.

use anyhow::Result;
use autonomous_platform::{
    data::TimeSeriesData,
    integration::{
        daa_coordinator::{DaaConfig, DaaCoordinator, TradingAction},
        data_access::DataAccessLayer,
    },
    neural::{NeuralConfig, NeuralPredictor},
    strategies::{MarketContext, Position, PositionSide},
    streaming::event_bus::{EventBusIntegration, MarketEvent},
};
use chrono::Utc;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

/// Test helper to create market events
fn create_market_event(symbol: &str, price: f64, sequence: u64) -> MarketEvent {
    MarketEvent {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        event_type: "market_update".to_string(),
        price,
        volume: 1000.0,
        bid: price - 5.0,
        ask: price + 5.0,
        spread: 10.0,
        order_book_depth: Some(20),
        sequence_number: sequence,
        source: "test".to_string(),
        quality_score: 0.95,
        metadata: None,
    }
}

/// Test helper to simulate the main.rs event processing logic
async fn process_market_events_to_daa(
    events: Vec<MarketEvent>,
    coordinator: Arc<DaaCoordinator>,
    current_positions: &mut HashMap<String, Position>,
) -> Result<Vec<TradingAction>> {
    let mut decisions = Vec::new();

    // Group events by symbol (similar to main.rs)
    let mut events_by_symbol: HashMap<String, Vec<MarketEvent>> = HashMap::new();

    for event in events {
        events_by_symbol
            .entry(event.symbol.clone())
            .or_insert_with(Vec::new)
            .push(event);
    }

    // Process each symbol's data
    for (symbol, symbol_events) in events_by_symbol {
        // Convert to TimeSeriesData (mimicking main.rs logic)
        let mut time_series_data: Vec<TimeSeriesData> = Vec::new();

        for event in &symbol_events {
            let ts_data = TimeSeriesData {
                symbol: symbol.clone(),
                timestamp: event.timestamp,
                open: event.price - 10.0,
                high: event.price + 20.0,
                low: event.price - 20.0,
                close: event.price,
                volume: event.volume,
                indicators: HashMap::new(),
                source: Some("event_bus".to_string()),
                entity: Some(symbol.clone()),
                value: Some(event.price),
                metadata: None,
            };

            time_series_data.push(ts_data);
        }

        // Only process if we have enough data
        if time_series_data.len() >= 10 {
            // Sort by timestamp
            time_series_data.sort_by_key(|d| d.timestamp);

            if let Some(latest) = time_series_data.last() {
                // Calculate volatility (from main.rs)
                let mut returns = Vec::new();
                for i in 1..time_series_data.len() {
                    let return_pct = (time_series_data[i].close - time_series_data[i - 1].close)
                        / time_series_data[i - 1].close;
                    returns.push(return_pct);
                }

                let avg_return = returns.iter().sum::<f64>() / returns.len() as f64;
                let volatility = (returns
                    .iter()
                    .map(|r| (r - avg_return).powi(2))
                    .sum::<f64>()
                    / returns.len() as f64)
                    .sqrt();

                // Create MarketContext
                let market_context = MarketContext {
                    symbol: symbol.clone(),
                    current_price: latest.close,
                    bid: latest.low,
                    ask: latest.high,
                    volume_24h: time_series_data.iter().map(|d| d.volume).sum::<f64>(),
                    volatility: volatility * 100.0,
                    timestamp: latest.timestamp.timestamp(),
                };

                // Get current position
                let position = current_positions.get(&symbol);

                // Make DAA decision
                let decision = coordinator
                    .make_decision(&market_context, position, &time_series_data)
                    .await?;

                // Update position tracking (from main.rs)
                match &decision.action {
                    TradingAction::Buy {
                        symbol: sym, size, ..
                    } => {
                        current_positions.insert(
                            sym.clone(),
                            Position {
                                symbol: sym.clone(),
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
                        symbol: sym, size, ..
                    } => {
                        if *size > 0.0 {
                            current_positions.insert(
                                sym.clone(),
                                Position {
                                    symbol: sym.clone(),
                                    side: PositionSide::Short,
                                    size: *size,
                                    entry_price: latest.close,
                                    current_price: latest.close,
                                    unrealized_pnl: 0.0,
                                    timestamp: latest.timestamp.timestamp(),
                                },
                            );
                        } else {
                            current_positions.remove(sym);
                        }
                    }
                    _ => {}
                }

                decisions.push(decision.action.clone());
            }
        }
    }

    Ok(decisions)
}

#[cfg(test)]
mod main_flow_tests {
    use super::*;

    /// Test 1: Basic event flow processing
    #[tokio::test]
    async fn test_basic_event_flow() {
        // Setup DAA components
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Create test events
        let mut events = Vec::new();
        for i in 0..15 {
            events.push(create_market_event(
                "BTC/USDT",
                50000.0 + (i as f64 * 10.0),
                i as u64,
            ));
        }

        let mut positions = HashMap::new();

        // Process events
        let decisions = process_market_events_to_daa(events, coordinator, &mut positions)
            .await
            .unwrap();

        // Verify decision was made
        assert!(!decisions.is_empty());

        // Verify decision was sent through channel
        let received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(received.is_ok());
    }

    /// Test 2: Multiple symbols processing
    #[tokio::test]
    async fn test_multiple_symbols() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Create events for multiple symbols
        let mut events = Vec::new();
        let symbols = vec!["BTC/USDT", "ETH/USDT", "SOL/USDT"];

        for (sym_idx, symbol) in symbols.iter().enumerate() {
            let base_price = 1000.0 * (sym_idx + 1) as f64;
            for i in 0..12 {
                events.push(create_market_event(
                    symbol,
                    base_price + (i as f64 * 5.0),
                    i as u64,
                ));
            }
        }

        let mut positions = HashMap::new();

        // Process all events
        let decisions = process_market_events_to_daa(events, coordinator, &mut positions)
            .await
            .unwrap();

        // Should have decisions for each symbol
        assert!(decisions.len() >= symbols.len());

        // Verify all decisions were sent
        let mut received_count = 0;
        while let Ok(Some(_)) = timeout(Duration::from_millis(10), rx.recv()).await {
            received_count += 1;
        }
        assert_eq!(received_count, decisions.len());
    }

    /// Test 3: Position management flow
    #[tokio::test]
    async fn test_position_management() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        let mut positions = HashMap::new();

        // First batch - should open position
        let mut events1 = Vec::new();
        for i in 0..15 {
            events1.push(create_market_event(
                "BTC/USDT",
                50000.0 + (i as f64 * 50.0),
                i as u64,
            ));
        }

        let decisions1 = process_market_events_to_daa(events1, coordinator.clone(), &mut positions)
            .await
            .unwrap();

        // Check if position was opened
        let initial_position_count = positions.len();
        println!("Positions after first batch: {}", initial_position_count);

        // Second batch - with existing position
        let mut events2 = Vec::new();
        for i in 15..30 {
            events2.push(create_market_event(
                "BTC/USDT",
                51000.0 - (i as f64 * 30.0),
                i as u64,
            ));
        }

        let decisions2 = process_market_events_to_daa(events2, coordinator, &mut positions)
            .await
            .unwrap();

        // Verify position management occurred
        assert!(!decisions1.is_empty() || !decisions2.is_empty());

        // Print decision types for debugging
        for decision in decisions1.iter().chain(decisions2.iter()) {
            match decision {
                TradingAction::Buy { symbol, size, .. } => {
                    println!("Buy {} size {}", symbol, size);
                }
                TradingAction::Sell { symbol, size, .. } => {
                    println!("Sell {} size {}", symbol, size);
                }
                TradingAction::Hold { reason } => {
                    println!("Hold: {}", reason);
                }
                TradingAction::AdjustPosition { symbol, .. } => {
                    println!("Adjust position for {}", symbol);
                }
            }
        }
    }

    /// Test 4: Volatility calculation
    #[tokio::test]
    async fn test_volatility_calculation() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Create volatile market events
        let mut events = Vec::new();
        for i in 0..20 {
            let price = 50000.0 + (i as f64 * 10.0).sin() * 1000.0; // Oscillating price
            events.push(create_market_event("BTC/USDT", price, i as u64));
        }

        let mut positions = HashMap::new();
        let _ = process_market_events_to_daa(events, coordinator, &mut positions)
            .await
            .unwrap();

        // Get the decision to check volatility was considered
        if let Ok(Some(decision)) = timeout(Duration::from_millis(100), rx.recv()).await {
            // High volatility should affect decision confidence
            println!(
                "Decision confidence with volatile market: {:.2}%",
                decision.confidence * 100.0
            );
            println!("Risk assessment: {:?}", decision.risk_assessment);

            // Volatility adjusted size should be lower than max risk
            assert!(decision.risk_assessment.volatility_adjusted_size < 0.02);
        }
    }

    /// Test 5: Insufficient data handling
    #[tokio::test]
    async fn test_insufficient_data() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Create only 5 events (less than required 10)
        let mut events = Vec::new();
        for i in 0..5 {
            events.push(create_market_event("BTC/USDT", 50000.0, i as u64));
        }

        let mut positions = HashMap::new();
        let decisions = process_market_events_to_daa(events, coordinator, &mut positions)
            .await
            .unwrap();

        // Should not make decisions with insufficient data
        assert!(decisions.is_empty());

        // No decision should be sent
        let received = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(received.is_err()); // Timeout expected
    }

    /// Test 6: Shutdown signal handling
    #[tokio::test]
    async fn test_shutdown_handling() {
        let shutdown_signal = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown_signal.clone();

        // Spawn a task that sets shutdown after delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            shutdown_clone.store(true, Ordering::Relaxed);
        });

        // Simulate main loop with shutdown check
        let mut iteration = 0;
        loop {
            if shutdown_signal.load(Ordering::Relaxed) {
                println!("Shutdown signal received after {} iterations", iteration);
                break;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
            iteration += 1;

            if iteration > 50 {
                panic!("Shutdown signal not received in time");
            }
        }

        assert!(iteration > 5 && iteration < 50);
    }

    /// Test 7: Concurrent event processing
    #[tokio::test]
    async fn test_concurrent_processing() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(1000);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Spawn multiple tasks processing different symbols
        let mut handles = vec![];
        let symbols = vec!["BTC/USDT", "ETH/USDT", "SOL/USDT", "ADA/USDT"];

        for symbol in symbols {
            let coordinator_clone = coordinator.clone();
            let symbol = symbol.to_string();

            let handle = tokio::spawn(async move {
                let mut events = Vec::new();
                for i in 0..15 {
                    events.push(create_market_event(
                        &symbol,
                        1000.0 + (i as f64 * 10.0),
                        i as u64,
                    ));
                }

                let mut positions = HashMap::new();
                process_market_events_to_daa(events, coordinator_clone, &mut positions).await
            });

            handles.push(handle);
        }

        // Wait for all tasks
        let mut total_decisions = 0;
        for handle in handles {
            let decisions = handle.await.unwrap().unwrap();
            total_decisions += decisions.len();
        }

        // Verify all decisions were sent
        let mut received = 0;
        while let Ok(Some(_)) = timeout(Duration::from_millis(10), rx.recv()).await {
            received += 1;
        }

        assert_eq!(received, total_decisions);
    }

    /// Test 8: Error recovery in event processing
    #[tokio::test]
    async fn test_error_recovery() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        // Create events with some invalid data
        let mut events = Vec::new();

        // Valid events
        for i in 0..10 {
            events.push(create_market_event(
                "BTC/USDT",
                50000.0 + (i as f64 * 10.0),
                i as u64,
            ));
        }

        // Add an event with extreme values
        let mut extreme_event = create_market_event("BTC/USDT", 50000.0, 10);
        extreme_event.volume = -100.0; // Invalid volume
        extreme_event.spread = -50.0; // Invalid spread
        events.push(extreme_event);

        // More valid events
        for i in 11..20 {
            events.push(create_market_event(
                "BTC/USDT",
                50100.0 + (i as f64 * 10.0),
                i as u64,
            ));
        }

        let mut positions = HashMap::new();

        // Should process successfully despite invalid data
        let result = process_market_events_to_daa(events, coordinator, &mut positions).await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Performance test: High-frequency event processing
    #[tokio::test]
    async fn test_high_frequency_events() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 20,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };

        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(10000);
        let coordinator = Arc::new(DaaCoordinator::new(
            DaaConfig::default(),
            neural_predictor,
            tx,
        ));

        let start = std::time::Instant::now();

        // Generate large batch of events
        let mut all_events = Vec::new();
        for batch in 0..5 {
            let mut events = Vec::new();
            for i in 0..100 {
                let seq = (batch * 100 + i) as u64;
                events.push(create_market_event(
                    "BTC/USDT",
                    50000.0 + (seq as f64 * 10.0),
                    seq,
                ));
            }
            all_events.extend(events);
        }

        let mut positions = HashMap::new();
        let decisions = process_market_events_to_daa(all_events, coordinator, &mut positions)
            .await
            .unwrap();

        let elapsed = start.elapsed();

        println!("Processed {} events in {:?}", 500, elapsed);
        println!("Generated {} decisions", decisions.len());

        // Should process efficiently
        assert!(elapsed.as_secs() < 10); // Should complete within 10 seconds

        // Verify decisions were sent
        let mut received = 0;
        while let Ok(Some(_)) = timeout(Duration::from_millis(1), rx.recv()).await {
            received += 1;
        }
        assert_eq!(received, decisions.len());
    }
}
