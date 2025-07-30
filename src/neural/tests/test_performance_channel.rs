//! Test Performance Channel Implementation
//!
//! Tests the PerformanceChannel with broadcast capabilities and
//! integration with EnhancedNeuralAdapter for 100% prediction tracking.

use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tracing_test::traced_test;
use std::collections::HashMap;

use crate::adapters::enhanced_neural_adapter::{EnhancedNeuralAdapter, EnhancedNeuralConfig};
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::monitoring::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder, PerformanceEventType,
    PerformanceSource, ChannelConfig,
};
use crate::adapters::PerformanceEmitter;

#[tokio::test]
#[traced_test]
async fn test_performance_channel_broadcast() {
    // Create performance channel with broadcast capabilities
    let config = ChannelConfig { buffer_size: 100, ..Default::default() };
    let (channel, mut receiver1) = PerformanceChannel::new(config);
    let mut receiver2 = channel.subscribe();
    let mut receiver3 = channel.subscribe();

    // Create test event
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "test_model".to_string(),
            predictor_id: "pred_1".to_string(),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: "test_model".to_string(),
            accuracy: 0.95,
            confidence: 0.88,
            latency_ms: 150,
            input_features: 10,
            output_dimension: 1,
            timestamp: chrono::Utc::now(),
        })
        .custom_metric("test_metric".to_string(), 42.0)
        .build()
        .expect("Failed to build test event");

    // Emit event
    channel.emit(event.clone()).await.expect("Failed to emit event");

    // All receivers should get the event
    let received1 = timeout(Duration::from_millis(100), receiver1.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");
    
    let received2 = timeout(Duration::from_millis(100), receiver2.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");
        
    let received3 = timeout(Duration::from_millis(100), receiver3.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");

    // Verify all events are identical
    assert_eq!(received1.timestamp, event.timestamp);
    assert_eq!(received2.timestamp, event.timestamp);
    assert_eq!(received3.timestamp, event.timestamp);

    // Verify performance source
    match (&received1.source, &event.source) {
        (
            PerformanceSource::NeuralPredictor { model_name: m1, .. },
            PerformanceSource::NeuralPredictor { model_name: m2, .. },
        ) => assert_eq!(m1, m2),
        _ => panic!("Performance source mismatch"),
    }

    // Verify metrics buffer
    assert_eq!(channel.buffer_size(), 1);
    let recent = channel.get_recent_metrics(5);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].timestamp, event.timestamp);
}

#[tokio::test]
#[traced_test]
async fn test_enhanced_adapter_performance_emission() {
    // Create enhanced neural adapter with disabled features for testing
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        neural: NeuralConfig {
            memory_gb: 1.0,
            models: vec!["FANN_MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: false,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: false,
            enable_fallback: false,
            lookback_window: 24,
            enable_circuit_breakers: false,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true, // Important for this test
            enable_adaptive_retry: false,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 1,
            error_threshold: 0.1,
        },
        ..Default::default()
    };

    let mut adapter = EnhancedNeuralAdapter::new(config)
        .await
        .expect("Failed to create enhanced adapter");

    // Create performance channel for receiving events
    let (tx, mut rx) = mpsc::unbounded_channel();
    adapter.set_performance_sender(tx);

    // Create test data
    let test_data = vec![TimeSeriesData {
        symbol: "BTC/USD".to_string(),
        timestamp: chrono::Utc::now(),
        open: 50000.0,
        high: 51000.0,
        low: 49500.0,
        close: 50500.0,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("test".to_string()),
        value: Some(50500.0),
        metadata: None,
    }];

    // Execute prediction - this should emit performance event
    let prediction_result = adapter.predict_enhanced(&test_data, 5, None).await;

    // Verify prediction succeeded or failed gracefully
    match prediction_result {
        Ok(result) => {
            assert_eq!(result.predictions.len(), 5);
            assert!(!result.model_used.is_empty());
            assert!(result.execution_time.as_millis() > 0);
            
            // Should have received a performance event
            let event = timeout(Duration::from_millis(500), rx.recv())
                .await
                .expect("Timeout waiting for performance event")
                .expect("No performance event received");

            // Verify event structure
            match event.event_type {
                PerformanceEventType::PredictionCompleted { model, accuracy, confidence, latency_ms, .. } => {
                    assert_eq!(model, result.model_used);
                    assert!(accuracy >= 0.0 && accuracy <= 1.0);
                    assert!(confidence >= 0.0 && confidence <= 1.0);
                    assert!(latency_ms > 0);
                }
                _ => panic!("Expected PredictionCompleted event, got: {:?}", event.event_type),
            }

            // Verify custom metrics
            if let Some(ref custom) = event.metrics.custom_metrics {
                assert!(custom.contains_key("prediction_count"));
                assert_eq!(custom["prediction_count"], 5.0);
            } else {
                panic!("Expected custom metrics in performance event");
            }
        }
        Err(error) => {
            // If prediction failed, should still emit error event
            let event = timeout(Duration::from_millis(500), rx.recv())
                .await
                .expect("Timeout waiting for error event")
                .expect("No error event received");

            // Verify error event structure
            match event.event_type {
                PerformanceEventType::SystemHealth { error_rate, .. } => {
                    assert_eq!(error_rate, 100.0); // 100% error rate for failure
                }
                _ => panic!("Expected SystemHealth error event, got: {:?}", event.event_type),
            }

            println!("Prediction failed as expected: {}", error);
        }
    }
}

#[tokio::test]
#[traced_test]
async fn test_performance_channel_buffer_management() {
    // Create small buffer to test overflow behavior
    let config = ChannelConfig { buffer_size: 3, ..Default::default() };
    let (channel, _receiver) = PerformanceChannel::new(config);

    // Create test events
    for i in 0..10 {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: format!("model_{}", i),
                predictor_id: format!("pred_{}", i),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: format!("model_{}", i),
                accuracy: i as f64 / 10.0,
                confidence: 0.9,
                latency_ms: 100 + i * 10,
                input_features: 10,
                output_dimension: 1,
                timestamp: chrono::Utc::now(),
            })
            .build()
            .expect("Failed to build test event");

        channel.emit(event).await.expect("Failed to emit event");
    }

    // Buffer should only contain the last 3 events
    assert_eq!(channel.buffer_size(), 3);

    // Get recent metrics - should be the most recent events
    let recent = channel.get_recent_metrics(5);
    assert_eq!(recent.len(), 3);

    // Verify order (most recent first) - the last 3 events should be 7, 8, 9
    // But reversed in recent order, so 9, 8, 7
    match &recent[0].event_type {
        PerformanceEventType::PredictionCompleted { accuracy, .. } => {
            assert!((*accuracy - 0.9).abs() < 0.01); // 9th event (most recent)
        }
        _ => panic!("Unexpected event type"),
    }

    match &recent[2].event_type {
        PerformanceEventType::PredictionCompleted { accuracy, .. } => {
            assert!((*accuracy - 0.7).abs() < 0.01); // 7th event (oldest in buffer)
        }
        _ => panic!("Unexpected event type"),
    }
}

#[tokio::test]
#[traced_test]
async fn test_performance_emitter_trait_compliance() {
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        ..Default::default()
    };

    let mut adapter = EnhancedNeuralAdapter::new(config)
        .await
        .expect("Failed to create adapter");

    // Test PerformanceEmitter trait implementation
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Test set_performance_sender
    adapter.set_performance_sender(tx.clone());
    
    // Test get_performance_sender
    let sender = adapter.get_performance_sender();
    assert!(sender.is_some());

    // Test emit_performance
    let test_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "test".to_string(),
            predictor_id: "pred_test".to_string(),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: "test".to_string(),
            accuracy: 0.95,
            confidence: 0.9,
            latency_ms: 100,
            input_features: 10,
            output_dimension: 1,
            timestamp: chrono::Utc::now(),
        })
        .build()
        .expect("Failed to build test event");

    adapter.emit_performance(test_event.clone())
        .await
        .expect("Failed to emit performance event");

    // Verify event was received
    let received = timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("No event received");

    assert_eq!(received.timestamp, test_event.timestamp);
}

#[tokio::test]
#[traced_test]
async fn test_performance_metrics_aggregation() {
    let config = ChannelConfig { buffer_size: 100, ..Default::default() };
    let (channel, _receiver) = PerformanceChannel::new(config);

    // Simulate rapid prediction events
    let mut events = Vec::new();
    for i in 0..20 {
        let accuracy = 0.8 + (i as f64 * 0.01); // Increasing accuracy
        let latency = 100 + (i * 5); // Increasing latency
        
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "ensemble_model".to_string(),
                predictor_id: format!("pred_{}", i),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "ensemble_model".to_string(),
                accuracy,
                confidence: accuracy - 0.05,
                latency_ms: latency as u64,
                input_features: 10,
                output_dimension: 1,
                timestamp: chrono::Utc::now(),
            })
            .build()
            .expect("Failed to build event");

        events.push(event.clone());
        channel.emit(event).await.expect("Failed to emit event");
    }

    // Verify all events are in buffer
    assert_eq!(channel.buffer_size(), 20);

    // Get recent metrics for analysis
    let recent = channel.get_recent_metrics(10);
    assert_eq!(recent.len(), 10);

    // Verify metrics show performance trend
    let mut total_accuracy = 0.0;
    let mut total_latency = 0.0;
    let mut count = 0;

    for event in &recent {
        if let PerformanceEventType::PredictionCompleted { accuracy, latency_ms, .. } = &event.event_type {
            total_accuracy += accuracy;
            total_latency += *latency_ms as f64;
            count += 1;
        }
    }

    let avg_accuracy = total_accuracy / count as f64;
    let avg_latency = total_latency / count as f64;

    // The most recent 10 events should have higher accuracy (we added them in increasing order)
    assert!(avg_accuracy > 0.9, "Average accuracy should be > 0.9, got {}", avg_accuracy);
    assert!(avg_latency > 150.0, "Average latency should be > 150ms, got {}", avg_latency);

    println!("Performance aggregation test passed:");
    println!("  - Average accuracy: {:.3}", avg_accuracy);
    println!("  - Average latency: {:.1}ms", avg_latency);
    println!("  - Events processed: {}", count);
}