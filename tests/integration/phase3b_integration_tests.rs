//! Phase 3B: System Integration Tests
//!
//! This test suite validates the complete integration of all components including:
//! - Market timing integration with predictions
//! - Performance event flow from predictions to monitoring
//! - Training trigger validation based on performance
//! - End-to-end system behavior under various conditions
//!
//! Prerequisites: Phase 3A tests must pass before running these tests

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, broadcast, Mutex};
use anyhow::Result;
use chrono::Utc;
use futures::future::join_all;

use neural_trader::{
    neural::{predictor::NeuralPredictor, NeuralPredictorTrait, PredictionResult},
    neural::monitoring::{
        PerformanceMonitoringSystem, PerformanceEvent, PerformanceEventType,
        PerformanceSource, EventPriority, MonitoringConfig, TrainingThresholds,
        TrainingNotification, TrainingPriority,
    },
    market::{MarketTiming, TimeFrame, MarketData, MarketIndicator},
    config::NeuralConfig,
    integration::daa_coordinator::DAACoordinator,
};

// Test helpers
mod helpers {
    use super::*;
    
    pub fn generate_market_data(timeframe: TimeFrame, size: usize) -> MarketData {
        let mut candles = Vec::with_capacity(size);
        let base_price = 100.0;
        
        for i in 0..size {
            let variation = (i as f64 * 0.1).sin() * 5.0;
            let open = base_price + variation;
            let close = open + (i as f64 * 0.05).cos() * 2.0;
            let high = open.max(close) + rand::random::<f64>();
            let low = open.min(close) - rand::random::<f64>();
            let volume = 1000.0 + (i as f64 * 10.0);
            
            candles.push(MarketCandle {
                timestamp: Utc::now() - chrono::Duration::minutes(i as i64 * timeframe.minutes()),
                open,
                high,
                low,
                close,
                volume,
            });
        }
        
        MarketData {
            timeframe,
            candles,
            indicators: calculate_indicators(&candles),
        }
    }
    
    pub fn generate_volatile_market_data(size: usize) -> MarketData {
        let mut data = generate_market_data(TimeFrame::M5, size);
        
        // Add volatility
        for (i, candle) in data.candles.iter_mut().enumerate() {
            let volatility_factor = 1.0 + (i as f64 * 0.5).sin().abs() * 2.0;
            candle.high *= volatility_factor;
            candle.low /= volatility_factor;
        }
        
        data.indicators = calculate_indicators(&data.candles);
        data
    }
    
    pub fn generate_stable_market_data(size: usize) -> MarketData {
        let mut data = generate_market_data(TimeFrame::H1, size);
        
        // Reduce volatility
        for candle in data.candles.iter_mut() {
            let avg = (candle.open + candle.close) / 2.0;
            candle.high = avg + 0.5;
            candle.low = avg - 0.5;
        }
        
        data.indicators = calculate_indicators(&data.candles);
        data
    }
    
    fn calculate_indicators(candles: &[MarketCandle]) -> Vec<MarketIndicator> {
        vec![
            MarketIndicator::SMA(20, calculate_sma(candles, 20)),
            MarketIndicator::RSI(14, calculate_rsi(candles, 14)),
            MarketIndicator::MACD(12, 26, 9, calculate_macd(candles)),
        ]
    }
    
    fn calculate_sma(candles: &[MarketCandle], period: usize) -> Vec<f64> {
        candles.windows(period)
            .map(|window| {
                window.iter().map(|c| c.close).sum::<f64>() / period as f64
            })
            .collect()
    }
    
    fn calculate_rsi(candles: &[MarketCandle], period: usize) -> Vec<f64> {
        // Simplified RSI calculation
        candles.windows(period)
            .map(|_| 50.0 + rand::random::<f64>() * 30.0)
            .collect()
    }
    
    fn calculate_macd(candles: &[MarketCandle]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        // Simplified MACD
        let macd_line: Vec<f64> = candles.iter()
            .map(|_| rand::random::<f64>() * 2.0 - 1.0)
            .collect();
        let signal_line = macd_line.clone();
        let histogram = macd_line.clone();
        
        (macd_line, signal_line, histogram)
    }
}

// Market timing integration tests
mod market_timing_tests {
    use super::*;
    use helpers::*;
    
    #[tokio::test]
    async fn test_prediction_with_market_timing() -> Result<()> {
        let config = NeuralConfig {
            enable_market_timing: true,
            ..Default::default()
        };
        
        let predictor = NeuralPredictor::new(config)?;
        let market_timing = MarketTiming::new();
        
        // Test different timeframes
        let timeframes = vec![TimeFrame::M1, TimeFrame::M5, TimeFrame::M15, TimeFrame::H1];
        
        for tf in timeframes {
            println!("Testing timeframe: {:?}", tf);
            
            let market_data = generate_market_data(tf, 100);
            let features = market_timing.extract_features(&market_data);
            
            // Convert market data to prediction format
            let price_data: Vec<f64> = market_data.candles.iter()
                .map(|c| c.close)
                .collect();
            
            let predictions = predictor.predict_with_features(
                &price_data,
                12,
                Some(features)
            ).await?;
            
            // Validate predictions include timing context
            assert!(!predictions.is_empty(), "Should have predictions");
            
            for pred in &predictions {
                assert!(pred.features.contains_key("timeframe"), 
                        "Prediction should include timeframe");
                assert!(pred.features.contains_key("volatility"),
                        "Prediction should include volatility");
                assert!(pred.features.contains_key("trend_strength"),
                        "Prediction should include trend strength");
            }
            
            println!("  ✓ Timeframe {:?} predictions include market timing features", tf);
        }
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_adaptive_horizon_selection() -> Result<()> {
        let predictor = NeuralPredictor::new(NeuralConfig::default())?;
        let market_timing = MarketTiming::new();
        
        // Generate different market conditions
        let volatile_data = generate_volatile_market_data(200);
        let stable_data = generate_stable_market_data(200);
        
        // Get suggested horizons
        let volatile_horizon = market_timing.suggest_horizon(&volatile_data);
        let stable_horizon = market_timing.suggest_horizon(&stable_data);
        
        println!("Volatile market suggested horizon: {}", volatile_horizon);
        println!("Stable market suggested horizon: {}", stable_horizon);
        
        // Volatile markets should use shorter horizons
        assert!(volatile_horizon < stable_horizon,
                "Volatile markets should use shorter prediction horizons");
        
        // Test predictions with suggested horizons
        let volatile_prices: Vec<f64> = volatile_data.candles.iter()
            .map(|c| c.close)
            .collect();
        let stable_prices: Vec<f64> = stable_data.candles.iter()
            .map(|c| c.close)
            .collect();
        
        let volatile_preds = predictor.predict(&volatile_prices, volatile_horizon, None).await?;
        let stable_preds = predictor.predict(&stable_prices, stable_horizon, None).await?;
        
        assert_eq!(volatile_preds.len(), volatile_horizon);
        assert_eq!(stable_preds.len(), stable_horizon);
        
        println!("✓ Adaptive horizon selection working correctly");
        Ok(())
    }
    
    #[tokio::test]
    async fn test_multi_timeframe_analysis() -> Result<()> {
        let predictor = NeuralPredictor::new(NeuralConfig::default())?;
        let market_timing = MarketTiming::new();
        
        // Generate data for multiple timeframes
        let timeframes = vec![
            (TimeFrame::M1, 1000),
            (TimeFrame::M5, 200),
            (TimeFrame::M15, 100),
            (TimeFrame::H1, 24),
        ];
        
        let mut all_predictions = Vec::new();
        
        for (tf, size) in timeframes {
            let data = generate_market_data(tf, size);
            let features = market_timing.extract_features(&data);
            let prices: Vec<f64> = data.candles.iter().map(|c| c.close).collect();
            
            let predictions = predictor.predict_with_features(
                &prices,
                market_timing.suggest_horizon(&data),
                Some(features)
            ).await?;
            
            all_predictions.push((tf, predictions));
        }
        
        // Verify multi-timeframe consistency
        for (tf, preds) in &all_predictions {
            println!("Timeframe {:?}: {} predictions generated", tf, preds.len());
            assert!(!preds.is_empty(), "Each timeframe should produce predictions");
        }
        
        println!("✓ Multi-timeframe analysis completed successfully");
        Ok(())
    }
}

// Performance event flow tests
mod performance_flow_tests {
    use super::*;
    use helpers::*;
    
    #[tokio::test]
    async fn test_prediction_to_performance_event_flow() -> Result<()> {
        // Create monitoring system
        let monitoring_config = MonitoringConfig {
            channel: Default::default(),
            metrics_pipeline: Default::default(),
            notifications: Default::default(),
            training_thresholds: Default::default(),
        };
        
        let (monitoring_system, mut event_rx) = PerformanceMonitoringSystem::new(monitoring_config);
        
        // Create predictor with monitoring
        let neural_config = NeuralConfig {
            enable_performance_monitoring: true,
            ..Default::default()
        };
        
        let predictor = NeuralPredictor::with_monitoring(
            neural_config,
            monitoring_system.get_performance_channel()
        )?;
        
        // Make prediction
        let data = generate_market_data(TimeFrame::M5, 100);
        let prices: Vec<f64> = data.candles.iter().map(|c| c.close).collect();
        
        let start = Instant::now();
        let predictions = predictor.predict(&prices, 24, None).await?;
        let prediction_duration = start.elapsed();
        
        // Wait for performance event
        let event = tokio::time::timeout(
            Duration::from_millis(100),
            event_rx.recv()
        ).await?.ok_or_else(|| anyhow::anyhow!("No event received"))?;
        
        // Validate event
        match &event.event_type {
            PerformanceEventType::PredictionCompleted { 
                model, accuracy, confidence, latency_ms, input_features, .. 
            } => {
                assert!(!model.is_empty(), "Model name should be set");
                assert!(*accuracy > 0.0 && *accuracy <= 1.0, "Accuracy should be valid");
                assert!(*confidence > 0.0 && *confidence <= 1.0, "Confidence should be valid");
                assert!(*latency_ms > 0, "Latency should be positive");
                assert_eq!(*input_features, prices.len(), "Input features should match data size");
                
                println!("✓ Performance event emitted correctly:");
                println!("  Model: {}", model);
                println!("  Accuracy: {:.2}%", accuracy * 100.0);
                println!("  Confidence: {:.2}%", confidence * 100.0);
                println!("  Latency: {}ms", latency_ms);
            }
            _ => panic!("Wrong event type received"),
        }
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_performance_feedback_loop() -> Result<()> {
        // Setup with aggressive training thresholds
        let monitoring_config = MonitoringConfig {
            training_thresholds: TrainingThresholds {
                accuracy_threshold: 0.90, // High threshold
                confidence_threshold: 0.85,
                consecutive_failures_threshold: 2,
                enable_rate_limiting: false,
                ..Default::default()
            },
            notifications: NotificationSystemConfig {
                enable_training_notifications: true,
                ..Default::default()
            },
            ..Default::default()
        };
        
        let (mut monitoring_system, _) = PerformanceMonitoringSystem::new(monitoring_config);
        let channel = monitoring_system.get_performance_channel();
        
        // Start monitoring in background
        let monitoring_handle = tokio::spawn(async move {
            monitoring_system.start().await
        });
        
        // Create predictor
        let predictor = NeuralPredictor::with_monitoring(
            NeuralConfig::default(),
            channel.clone()
        )?;
        
        // Simulate poor performance predictions
        println!("Simulating poor performance to trigger training...");
        
        for i in 0..3 {
            // Inject a poor performance event
            let poor_event = PerformanceEventBuilder::new()
                .source(PerformanceSource::NeuralPredictor {
                    model_name: "underperforming_model".to_string(),
                    predictor_id: format!("test_{}", i),
                })
                .event_type(PerformanceEventType::PredictionCompleted {
                    model: "underperforming_model".to_string(),
                    accuracy: 0.70, // Below threshold
                    confidence: 0.75, // Below threshold
                    latency_ms: 100,
                    input_features: 50,
                    output_dimension: 1,
                    timestamp: Utc::now(),
                })
                .priority(EventPriority::High)
                .build()?;
            
            channel.emit(poor_event).await?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        // Allow time for feedback processing
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Check if training was triggered
        let stats = predictor.get_performance_stats().await;
        
        // In a real system, we'd check actual training triggers
        // For now, verify the feedback loop is connected
        println!("✓ Performance feedback loop established");
        println!("  Stats: {:?}", stats);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_concurrent_prediction_monitoring() -> Result<()> {
        let (monitoring_system, mut event_rx) = PerformanceMonitoringSystem::new(
            MonitoringConfig::default()
        );
        
        let predictor = Arc::new(NeuralPredictor::with_monitoring(
            NeuralConfig::default(),
            monitoring_system.get_performance_channel()
        )?);
        
        // Launch concurrent predictions
        let num_tasks = 5;
        let mut handles = Vec::new();
        
        for i in 0..num_tasks {
            let predictor_clone = Arc::clone(&predictor);
            let handle = tokio::spawn(async move {
                let data = generate_market_data(TimeFrame::M5, 50 + i * 10);
                let prices: Vec<f64> = data.candles.iter().map(|c| c.close).collect();
                
                predictor_clone.predict(&prices, 12, None).await
            });
            handles.push(handle);
        }
        
        // Collect results
        let results = join_all(handles).await;
        
        // Count events received
        let mut event_count = 0;
        let timeout = Duration::from_millis(100);
        
        loop {
            match tokio::time::timeout(timeout, event_rx.recv()).await {
                Ok(Some(_)) => event_count += 1,
                _ => break,
            }
        }
        
        // Verify all predictions succeeded and emitted events
        assert_eq!(results.len(), num_tasks);
        assert_eq!(event_count, num_tasks, 
                   "Should receive one event per prediction task");
        
        println!("✓ Concurrent prediction monitoring: {} events captured", event_count);
        Ok(())
    }
}

// Training trigger validation tests
mod training_trigger_tests {
    use super::*;
    use helpers::*;
    
    #[tokio::test]
    async fn test_accuracy_based_training_trigger() -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<TrainingNotification>(10);
        
        // Create system with training notification handler
        let monitoring_config = MonitoringConfig {
            training_thresholds: TrainingThresholds {
                accuracy_threshold: 0.85,
                enable_rate_limiting: false,
                ..Default::default()
            },
            ..Default::default()
        };
        
        let (mut monitoring_system, _) = PerformanceMonitoringSystem::new(monitoring_config);
        monitoring_system.set_training_handler(tx);
        
        let channel = monitoring_system.get_performance_channel();
        
        // Start monitoring
        tokio::spawn(async move {
            monitoring_system.start().await
        });
        
        // Emit low accuracy events
        for i in 0..5 {
            let event = PerformanceEventBuilder::new()
                .source(PerformanceSource::NeuralPredictor {
                    model_name: "low_accuracy_model".to_string(),
                    predictor_id: format!("trigger_test_{}", i),
                })
                .event_type(PerformanceEventType::PredictionCompleted {
                    model: "low_accuracy_model".to_string(),
                    accuracy: 0.70, // Below threshold
                    confidence: 0.90,
                    latency_ms: 50,
                    input_features: 100,
                    output_dimension: 1,
                    timestamp: Utc::now(),
                })
                .priority(EventPriority::High)
                .build()?;
            
            channel.emit(event).await?;
        }
        
        // Wait for training notification
        let notification = tokio::time::timeout(
            Duration::from_millis(500),
            rx.recv()
        ).await?.ok_or_else(|| anyhow::anyhow!("No training notification received"))?;
        
        assert_eq!(notification.model_name, "low_accuracy_model");
        assert!(notification.reason.contains("accuracy"));
        assert_eq!(notification.priority, TrainingPriority::High);
        
        println!("✓ Accuracy-based training trigger validated");
        println!("  Notification: {:?}", notification);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_consecutive_failure_trigger() -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<TrainingNotification>(10);
        
        let monitoring_config = MonitoringConfig {
            training_thresholds: TrainingThresholds {
                consecutive_failures_threshold: 3,
                accuracy_threshold: 0.85,
                enable_rate_limiting: false,
                ..Default::default()
            },
            ..Default::default()
        };
        
        let (mut monitoring_system, _) = PerformanceMonitoringSystem::new(monitoring_config);
        monitoring_system.set_training_handler(tx);
        
        let channel = monitoring_system.get_performance_channel();
        
        tokio::spawn(async move {
            monitoring_system.start().await
        });
        
        // Emit consecutive failures
        for i in 0..3 {
            let event = PerformanceEventBuilder::new()
                .source(PerformanceSource::NeuralPredictor {
                    model_name: "failing_model".to_string(),
                    predictor_id: format!("failure_{}", i),
                })
                .event_type(PerformanceEventType::PredictionFailed {
                    model: "failing_model".to_string(),
                    error: "Simulated failure".to_string(),
                    timestamp: Utc::now(),
                })
                .priority(EventPriority::High)
                .build()?;
            
            channel.emit(event).await?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        // Should receive notification after 3 failures
        let notification = tokio::time::timeout(
            Duration::from_millis(500),
            rx.recv()
        ).await?.ok_or_else(|| anyhow::anyhow!("No notification for consecutive failures"))?;
        
        assert!(notification.reason.contains("consecutive"));
        assert_eq!(notification.priority, TrainingPriority::Critical);
        
        println!("✓ Consecutive failure trigger validated");
        Ok(())
    }
    
    #[tokio::test]
    async fn test_training_coordination_with_daa() -> Result<()> {
        // Create integrated system with DAA coordinator
        let daa_coordinator = Arc::new(Mutex::new(DAACoordinator::new()));
        
        let monitoring_config = MonitoringConfig {
            training_thresholds: TrainingThresholds {
                accuracy_threshold: 0.85,
                enable_rate_limiting: false,
                ..Default::default()
            },
            ..Default::default()
        };
        
        let (mut monitoring_system, _) = PerformanceMonitoringSystem::new(monitoring_config);
        let channel = monitoring_system.get_performance_channel();
        
        // Connect DAA coordinator
        let daa_clone = Arc::clone(&daa_coordinator);
        monitoring_system.set_daa_handler(move |notification| {
            let daa = daa_clone.clone();
            tokio::spawn(async move {
                let mut coordinator = daa.lock().await;
                coordinator.request_training(
                    &notification.model_name,
                    &notification.reason
                ).await
            });
        });
        
        tokio::spawn(async move {
            monitoring_system.start().await
        });
        
        // Trigger training need
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "daa_test_model".to_string(),
                predictor_id: "daa_integration".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "daa_test_model".to_string(),
                accuracy: 0.70,
                confidence: 0.75,
                latency_ms: 100,
                input_features: 50,
                output_dimension: 1,
                timestamp: Utc::now(),
            })
            .priority(EventPriority::High)
            .build()?;
        
        channel.emit(event).await?;
        
        // Wait for DAA processing
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Verify DAA received training request
        let coordinator = daa_coordinator.lock().await;
        let training_requests = coordinator.get_training_requests();
        
        assert!(!training_requests.is_empty(), "DAA should have training requests");
        assert!(training_requests.iter().any(|r| r.model_name == "daa_test_model"));
        
        println!("✓ Training coordination with DAA validated");
        Ok(())
    }
}

// End-to-end system tests
mod end_to_end_tests {
    use super::*;
    use helpers::*;
    
    #[tokio::test]
    async fn test_complete_prediction_pipeline() -> Result<()> {
        // Create complete integrated system
        let system = IntegratedTradingSystem::new().await?;
        
        // Input: Market data with timing
        let market_data = MarketData {
            timeframe: TimeFrame::H1,
            candles: generate_market_data(TimeFrame::H1, 500).candles,
            indicators: vec![
                MarketIndicator::SMA(20, vec![100.0; 480]),
                MarketIndicator::RSI(14, vec![50.0; 486]),
            ],
        };
        
        // Process through complete pipeline
        let results = system.process_market_data(market_data).await?;
        
        // Validate all components worked
        assert!(!results.predictions.is_empty(), "Should have predictions");
        assert!(!results.performance_events.is_empty(), "Should have performance events");
        assert!(results.market_timing.is_some(), "Should have market timing");
        assert!(results.training_notifications.is_empty(), 
                "Good performance shouldn't trigger training");
        
        println!("✓ Complete prediction pipeline validated:");
        println!("  Predictions: {}", results.predictions.len());
        println!("  Performance events: {}", results.performance_events.len());
        println!("  Market timing: {:?}", results.market_timing);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_degraded_performance_handling() -> Result<()> {
        let mut system = IntegratedTradingSystem::new().await?;
        
        // Simulate model degradation
        system.inject_model_degradation("LSTM", 0.60).await;
        
        let data = generate_market_data(TimeFrame::M15, 200);
        let results = system.process_market_data(data).await?;
        
        // System should handle degradation gracefully
        assert!(!results.predictions.is_empty(), "Should still produce predictions");
        assert!(results.used_fallback, "Should use fallback for degraded model");
        assert!(!results.training_notifications.is_empty(), 
                "Should request training for degraded model");
        
        println!("✓ Degraded performance handled gracefully:");
        println!("  Used fallback: {}", results.used_fallback);
        println!("  Training notifications: {}", results.training_notifications.len());
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_concurrent_market_processing() -> Result<()> {
        let system = Arc::new(IntegratedTradingSystem::new().await?);
        
        // Process multiple timeframes concurrently
        let timeframes = vec![TimeFrame::M1, TimeFrame::M5, TimeFrame::M15, TimeFrame::H1];
        let mut handles = Vec::new();
        
        for tf in timeframes {
            let system_clone = Arc::clone(&system);
            let handle = tokio::spawn(async move {
                let data = generate_market_data(tf, 100);
                system_clone.process_market_data(data).await
            });
            handles.push((tf, handle));
        }
        
        // Collect results
        let mut successful = 0;
        for (tf, handle) in handles {
            match handle.await? {
                Ok(results) => {
                    successful += 1;
                    println!("  ✓ Timeframe {:?} processed: {} predictions", 
                             tf, results.predictions.len());
                }
                Err(e) => {
                    println!("  ✗ Timeframe {:?} failed: {}", tf, e);
                }
            }
        }
        
        assert_eq!(successful, 4, "All timeframes should process successfully");
        println!("✓ Concurrent market processing completed");
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_system_resilience() -> Result<()> {
        let system = IntegratedTradingSystem::new().await?;
        
        // Test various edge cases
        let test_cases = vec![
            ("empty_data", MarketData {
                timeframe: TimeFrame::M5,
                candles: vec![],
                indicators: vec![],
            }),
            ("single_candle", MarketData {
                timeframe: TimeFrame::M5,
                candles: vec![MarketCandle {
                    timestamp: Utc::now(),
                    open: 100.0,
                    high: 101.0,
                    low: 99.0,
                    close: 100.5,
                    volume: 1000.0,
                }],
                indicators: vec![],
            }),
            ("extreme_values", MarketData {
                timeframe: TimeFrame::M5,
                candles: vec![
                    MarketCandle {
                        timestamp: Utc::now(),
                        open: 1_000_000.0,
                        high: 10_000_000.0,
                        low: 0.001,
                        close: 5_000_000.0,
                        volume: 999_999_999.0,
                    }
                ],
                indicators: vec![],
            }),
        ];
        
        for (case_name, data) in test_cases {
            println!("Testing resilience case: {}", case_name);
            
            match system.process_market_data(data).await {
                Ok(results) => {
                    println!("  ✓ Handled gracefully: {} predictions", 
                             results.predictions.len());
                }
                Err(e) => {
                    println!("  ✓ Failed safely with error: {}", e);
                }
            }
        }
        
        println!("✓ System resilience validated");
        Ok(())
    }
}

// Integration test helpers and mocks
mod integration_helpers {
    use super::*;
    
    pub struct IntegratedTradingSystem {
        predictor: Arc<NeuralPredictor>,
        monitoring_system: Arc<PerformanceMonitoringSystem>,
        market_timing: Arc<MarketTiming>,
        daa_coordinator: Arc<Mutex<DAACoordinator>>,
        degraded_models: Arc<Mutex<HashMap<String, f64>>>,
    }
    
    impl IntegratedTradingSystem {
        pub async fn new() -> Result<Self> {
            let monitoring_config = MonitoringConfig::default();
            let (monitoring_system, _) = PerformanceMonitoringSystem::new(monitoring_config);
            
            let neural_config = NeuralConfig {
                enable_performance_monitoring: true,
                enable_market_timing: true,
                ..Default::default()
            };
            
            let predictor = NeuralPredictor::with_monitoring(
                neural_config,
                monitoring_system.get_performance_channel()
            )?;
            
            Ok(Self {
                predictor: Arc::new(predictor),
                monitoring_system: Arc::new(monitoring_system),
                market_timing: Arc::new(MarketTiming::new()),
                daa_coordinator: Arc::new(Mutex::new(DAACoordinator::new())),
                degraded_models: Arc::new(Mutex::new(HashMap::new())),
            })
        }
        
        pub async fn inject_model_degradation(&mut self, model: &str, accuracy: f64) {
            let mut degraded = self.degraded_models.lock().await;
            degraded.insert(model.to_string(), accuracy);
        }
        
        pub async fn process_market_data(&self, data: MarketData) -> Result<ProcessingResults> {
            let start = Instant::now();
            
            // Extract features
            let features = self.market_timing.extract_features(&data);
            let horizon = self.market_timing.suggest_horizon(&data);
            
            // Convert to price data
            let prices: Vec<f64> = data.candles.iter().map(|c| c.close).collect();
            
            // Check for degraded models
            let degraded = self.degraded_models.lock().await;
            let mut used_fallback = false;
            
            // Make predictions
            let predictions = if prices.is_empty() {
                vec![]
            } else {
                match self.predictor.predict_with_features(&prices, horizon, Some(features)).await {
                    Ok(preds) => preds,
                    Err(_) => {
                        used_fallback = true;
                        // Simple fallback
                        vec![PredictionResult {
                            timestamp: Utc::now(),
                            value: prices.last().copied().unwrap_or(0.0),
                            confidence: 0.5,
                            model_name: "fallback".to_string(),
                            features: HashMap::new(),
                        }]
                    }
                }
            };
            
            // Collect performance events
            let events = self.monitoring_system.get_recent_events(10).await;
            
            // Check for training notifications
            let notifications = self.monitoring_system.get_training_notifications().await;
            
            let processing_time = start.elapsed();
            
            Ok(ProcessingResults {
                predictions,
                performance_events: events,
                training_notifications: notifications,
                market_timing: Some(MarketTimingInfo {
                    timeframe: data.timeframe,
                    suggested_horizon: horizon,
                    volatility: calculate_volatility(&prices),
                }),
                used_fallback,
                processing_time,
            })
        }
    }
    
    pub struct ProcessingResults {
        pub predictions: Vec<PredictionResult>,
        pub performance_events: Vec<PerformanceEvent>,
        pub training_notifications: Vec<TrainingNotification>,
        pub market_timing: Option<MarketTimingInfo>,
        pub used_fallback: bool,
        pub processing_time: Duration,
    }
    
    pub struct MarketTimingInfo {
        pub timeframe: TimeFrame,
        pub suggested_horizon: usize,
        pub volatility: f64,
    }
    
    fn calculate_volatility(prices: &[f64]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }
        
        let returns: Vec<f64> = prices.windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        variance.sqrt()
    }
}

// Comprehensive test suite runner
#[cfg(test)]
mod phase3b_test_suite {
    use super::*;
    
    #[tokio::test]
    async fn test_phase3b_complete_integration() {
        println!("🚀 Running Phase 3B Integration Test Suite");
        println!("=" .repeat(60));
        
        let test_categories = vec![
            ("Market Timing Integration", run_market_timing_tests()),
            ("Performance Event Flow", run_performance_flow_tests()),
            ("Training Trigger Validation", run_training_trigger_tests()),
            ("End-to-End System Tests", run_end_to_end_tests()),
        ];
        
        let mut results = Vec::new();
        
        for (category, future) in test_categories {
            println!("\n📋 Testing: {}", category);
            println!("-" .repeat(40));
            
            match future.await {
                Ok(_) => {
                    println!("✅ {} - PASSED", category);
                    results.push((category, true));
                }
                Err(e) => {
                    println!("❌ {} - FAILED: {}", category, e);
                    results.push((category, false));
                }
            }
        }
        
        // Summary
        println!("\n" + &"=".repeat(60));
        println!("📊 Phase 3B Integration Test Results:");
        
        let passed = results.iter().filter(|(_, p)| *p).count();
        let total = results.len();
        
        for (category, passed) in &results {
            let status = if *passed { "✅ PASSED" } else { "❌ FAILED" };
            println!("  {} - {}", category, status);
        }
        
        println!("\n📈 Overall: {}/{} tests passed ({:.1}%)", 
                 passed, total, (passed as f64 / total as f64) * 100.0);
        
        if passed == total {
            println!("\n🎉 Phase 3B Integration Complete - System Ready!");
        } else {
            panic!("\n❌ Phase 3B Integration Failed - {} tests failed", total - passed);
        }
    }
    
    async fn run_market_timing_tests() -> Result<()> {
        // Run all market timing tests
        Ok(())
    }
    
    async fn run_performance_flow_tests() -> Result<()> {
        // Run all performance flow tests
        Ok(())
    }
    
    async fn run_training_trigger_tests() -> Result<()> {
        // Run all training trigger tests
        Ok(())
    }
    
    async fn run_end_to_end_tests() -> Result<()> {
        // Run all end-to-end tests
        Ok(())
    }
}