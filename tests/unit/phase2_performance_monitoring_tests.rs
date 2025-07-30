//! Phase 2 Performance Monitoring TDD Tests
//! 
//! Focused test suite for PerformanceChannel integration and monitoring.
//! These tests ensure that performance monitoring is deeply integrated
//! into the prediction pipeline.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use chrono::Utc;
use tokio::sync::{broadcast, RwLock};
use anyhow::Result;

use neural_trader::neural::{
    FannPredictor,
    NeuralPredictorTrait,
    PerformanceChannel,
    PerformanceEvent,
    PerformanceSource,
    PerformanceEventType,
    PerformanceMetrics as ChannelMetrics,
    PerformanceEventBuilder,
};
use neural_trader::data::TimeSeriesData;
use neural_trader::config::NeuralConfig;

#[cfg(test)]
mod performance_channel_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_predictor_accepts_performance_channel() {
        // Test: FannPredictor should accept PerformanceChannel at construction
        let (tx, _rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = create_test_config();
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        );
        
        assert!(predictor.is_ok(), "Should create predictor with performance channel");
        
        // Verify channel is used
        let predictor = predictor.unwrap();
        let test_data = create_test_data(5);
        let _ = predictor.predict(&test_data, 1, None).await;
        
        // Channel should have received events (once implemented)
    }

    #[tokio::test]
    async fn test_performance_monitoring_can_be_disabled() {
        // Test: Performance monitoring should respect config flag
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let mut config = create_test_config();
        config.enable_performance_monitoring = false;
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        let test_data = create_test_data(5);
        let _ = predictor.predict(&test_data, 1, None).await;
        
        // Should NOT receive events when disabled
        let result = tokio::time::timeout(
            Duration::from_millis(50),
            rx.recv()
        ).await;
        
        assert!(result.is_err(), "Should not emit events when monitoring disabled");
    }

    #[tokio::test]
    async fn test_performance_metrics_calculation() {
        // Test: Performance metrics should be accurately calculated
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..create_test_config()
        };
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        // Make multiple predictions to test metric aggregation
        let test_data = create_test_data(10);
        let num_predictions = 10;
        
        for _ in 0..num_predictions {
            let _ = predictor.predict(&test_data, 1, None).await;
        }
        
        // Collect all events
        let mut events = Vec::new();
        while let Ok(event) = tokio::time::timeout(
            Duration::from_millis(50),
            rx.recv()
        ).await {
            if let Ok(e) = event {
                events.push(e);
            }
        }
        
        // Should have received events for all predictions
        assert_eq!(events.len(), num_predictions, "Should emit event per prediction");
        
        // Calculate aggregate metrics
        let latencies: Vec<u64> = events.iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::PredictionCompleted { latency_ms, .. } => Some(*latency_ms),
                _ => None,
            })
            .collect();
        
        assert_eq!(latencies.len(), num_predictions);
        
        // Verify metrics make sense
        let avg_latency = latencies.iter().sum::<u64>() / latencies.len() as u64;
        assert!(avg_latency > 0, "Average latency should be positive");
    }

    #[tokio::test]
    async fn test_performance_event_custom_metrics() {
        // Test: Custom metrics can be added to performance events
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..create_test_config()
        };
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        // Make prediction
        let test_data = create_test_data(50); // Larger dataset
        let _ = predictor.predict(&test_data, 5, None).await;
        
        // Get event
        let event = rx.recv().await.expect("Should receive event");
        
        // Should include standard metrics
        assert!(event.metrics.throughput.is_some(), "Should have throughput");
        assert!(event.metrics.success_count.is_some(), "Should have success count");
        
        // Should be able to include custom metrics (implementation dependent)
        // e.g., cache_hit_rate, model_complexity, feature_count, etc.
    }

    #[tokio::test]
    async fn test_performance_degradation_detection() {
        // Test: System should detect performance degradation
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..create_test_config()
        };
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        // Make predictions with varying data sizes
        for size in [10, 100, 1000] {
            let test_data = create_test_data(size);
            let start = Instant::now();
            let _ = predictor.predict(&test_data, 1, None).await;
            let duration = start.elapsed();
            
            // Get event
            let event = rx.recv().await.expect("Should receive event");
            
            if let PerformanceEventType::PredictionCompleted { latency_ms, .. } = &event.event_type {
                // Larger datasets should take longer
                if size > 100 {
                    assert!(*latency_ms > 10, "Large datasets should have higher latency");
                }
            }
        }
    }

    fn create_test_config() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            lookback_window: 24,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        }
    }

    fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
        (0..count)
            .map(|i| TimeSeriesData {
                timestamp: Utc::now() - chrono::Duration::hours(count as i64 - i as i64),
                value: 100.0 + (i as f64 * 0.1),
                volume: Some(1000.0 + i as f64 * 10.0),
                metadata: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod performance_buffer_management_tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_channel_bounded_buffer() {
        // Test: PerformanceChannel should maintain bounded buffer
        let buffer_size = 10;
        let (channel, _rx) = PerformanceChannel::new(buffer_size);
        
        // Fill buffer beyond capacity
        for i in 0..20 {
            let event = create_test_event_with_id(i);
            channel.emit(event).await.expect("Should emit");
        }
        
        // Buffer should only contain last 10 events
        assert_eq!(channel.buffer_size(), buffer_size);
        
        // Recent metrics should return most recent
        let recent = channel.get_recent_metrics(5);
        assert_eq!(recent.len(), 5);
        
        // Verify FIFO behavior - oldest events dropped
        if let PerformanceEventType::PredictionCompleted { model, .. } = &recent[0].event_type {
            let id: usize = model.split('-').last().unwrap().parse().unwrap();
            assert!(id >= 15, "Should have most recent events");
        }
    }

    #[tokio::test]
    async fn test_performance_channel_memory_efficiency() {
        // Test: Channel should be memory efficient
        let (channel, _rx) = PerformanceChannel::new(1000);
        
        // Emit many events
        for i in 0..10000 {
            let event = create_test_event_with_id(i);
            channel.emit(event).await.expect("Should emit");
        }
        
        // Should still only have 1000 events
        assert_eq!(channel.buffer_size(), 1000);
        
        // Clear should free memory
        channel.clear_buffer().expect("Should clear");
        assert_eq!(channel.buffer_size(), 0);
    }

    #[tokio::test]
    async fn test_performance_metrics_aggregation() {
        // Test: Channel should support metric aggregation
        let (channel, _rx) = PerformanceChannel::new(100);
        
        // Emit events with varying metrics
        for i in 0..20 {
            let mut event = create_test_event_with_id(i);
            if let PerformanceEventType::PredictionCompleted { ref mut latency_ms, ref mut accuracy, .. } = event.event_type {
                *latency_ms = 50 + (i as u64 * 10);
                *accuracy = 0.8 + (i as f64 * 0.01);
            }
            channel.emit(event).await.expect("Should emit");
        }
        
        // Get recent metrics for analysis
        let recent = channel.get_recent_metrics(20);
        
        // Calculate aggregates
        let latencies: Vec<u64> = recent.iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::PredictionCompleted { latency_ms, .. } => Some(*latency_ms),
                _ => None,
            })
            .collect();
        
        let accuracies: Vec<f64> = recent.iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::PredictionCompleted { accuracy, .. } => Some(*accuracy),
                _ => None,
            })
            .collect();
        
        // Verify aggregation possibilities
        assert!(!latencies.is_empty());
        assert!(!accuracies.is_empty());
        
        let avg_latency = latencies.iter().sum::<u64>() / latencies.len() as u64;
        let avg_accuracy = accuracies.iter().sum::<f64>() / accuracies.len() as f64;
        
        assert!(avg_latency > 0);
        assert!(avg_accuracy > 0.0 && avg_accuracy <= 1.0);
    }

    fn create_test_event_with_id(id: usize) -> PerformanceEvent {
        PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: format!("MLP-{}", id),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                model: format!("MLP-{}", id),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                timestamp: Utc::now(),
            },
            metrics: ChannelMetrics::default(),
        }
    }
}

#[cfg(test)]
mod performance_event_routing_tests {
    use super::*;

    #[tokio::test]
    async fn test_all_prediction_paths_emit_events() {
        // Test: Every prediction path must emit performance events
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..create_test_config()
        };
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        let test_data = create_test_data(10);
        
        // Test 1: Direct predict() method
        let _ = predictor.predict(&test_data, 1, None).await;
        let event1 = rx.recv().await.expect("predict() should emit event");
        assert!(matches!(event1.event_type, PerformanceEventType::PredictionCompleted { .. }));
        
        // Test 2: execute_model() method
        let _ = predictor.execute_model("LSTM", &test_data, 1, None).await;
        let event2 = rx.recv().await.expect("execute_model() should emit event");
        assert!(matches!(event2.event_type, PerformanceEventType::PredictionCompleted { .. }));
        
        // Test 3: execute_ensemble() method
        let _ = predictor.execute_ensemble(&["MLP", "GRU"], &test_data, 1, None).await;
        // Should get multiple events (one per model + ensemble)
        let mut ensemble_events = 0;
        while let Ok(event) = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            if let Ok(e) = event {
                ensemble_events += 1;
                assert!(matches!(e.event_type, PerformanceEventType::PredictionCompleted { .. }));
            }
        }
        assert!(ensemble_events >= 3, "Ensemble should emit multiple events");
    }

    #[tokio::test]
    async fn test_error_paths_emit_failure_events() {
        // Test: Error paths should emit failure events
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..create_test_config()
        };
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        // Test with invalid data
        let empty_data: Vec<TimeSeriesData> = vec![];
        
        // Test 1: Empty data error
        let _ = predictor.predict(&empty_data, 1, None).await;
        let event = rx.recv().await.expect("Error should emit event");
        assert!(matches!(event.event_type, PerformanceEventType::PredictionFailed { .. }));
        
        // Test 2: Invalid model name
        let test_data = create_test_data(5);
        let _ = predictor.execute_model("InvalidModel", &test_data, 1, None).await;
        let event2 = rx.recv().await.expect("Invalid model should emit error event");
        assert!(matches!(event2.event_type, PerformanceEventType::PredictionFailed { .. }));
    }

    #[tokio::test]
    async fn test_cached_predictions_emit_cache_hit_events() {
        // Test: Cached predictions should emit events with cache hit info
        let (tx, mut rx) = broadcast::channel(100);
        let channel = Arc::new(PerformanceChannel::from_sender(tx));
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            prediction_cache_ttl: 300, // Enable caching
            ..create_test_config()
        };
        
        let predictor = FannPredictor::new_with_performance_channel(
            config,
            channel.clone()
        ).expect("Should create predictor");
        
        let test_data = create_test_data(10);
        
        // First prediction - cache miss
        let _ = predictor.predict(&test_data, 1, None).await;
        let event1 = rx.recv().await.expect("First prediction should emit event");
        
        // Second identical prediction - should be cache hit
        let _ = predictor.predict(&test_data, 1, None).await;
        let event2 = rx.recv().await.expect("Cached prediction should emit event");
        
        // Cache hit should be faster
        if let (
            PerformanceEventType::PredictionCompleted { latency_ms: latency1, .. },
            PerformanceEventType::PredictionCompleted { latency_ms: latency2, .. }
        ) = (&event1.event_type, &event2.event_type) {
            // Cache hit should be significantly faster
            assert!(latency2 < latency1, "Cache hit should be faster");
        }
        
        // Should ideally include cache hit metric
        // This would be in custom_metrics once implemented
    }

    fn create_test_config() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            lookback_window: 24,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        }
    }

    fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
        (0..count)
            .map(|i| TimeSeriesData {
                timestamp: Utc::now() - chrono::Duration::hours(count as i64 - i as i64),
                value: 100.0 + (i as f64 * 0.1),
                volume: Some(1000.0 + i as f64 * 10.0),
                metadata: None,
            })
            .collect()
    }
}