//! Enhanced Performance Channel Tests - TDD approach
//!
//! These tests verify:
//! - Event emission with <1ms latency
//! - Overflow handling for broadcast channels
//! - Metrics collection for events and subscribers
//! - Critical alert emission
//! - Performance statistics tracking

use tokio::sync::broadcast;
use tokio::time::{timeout, Duration, Instant};
use tracing_test::traced_test;
use std::sync::Arc;

use crate::neural::monitoring::performance_channel::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder, 
    PerformanceEventType, PerformanceSource, ChannelConfig,
    EventPriority, AlertType, AlertSeverity,
};

#[tokio::test]
#[traced_test]
async fn test_event_emission_latency_under_1ms() {
    let config = ChannelConfig {
        buffer_size: 1000,
        channel_capacity: 10000,
        enable_persistence: true,
        enable_metrics: true,
        max_subscribers: 100,
    };
    
    let (channel, mut receiver) = PerformanceChannel::new(config);
    
    // Create a high-priority event
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "critical_model".to_string(),
            predictor_id: "pred_1".to_string(),
        })
        .event_type(PerformanceEventType::Alert {
            alert_type: AlertType::ModelFailure,
            message: "Critical model failure".to_string(),
            severity: AlertSeverity::Critical,
            resolution_required: true,
        })
        .priority(EventPriority::Critical)
        .build()
        .unwrap();
    
    // Measure emission latency
    let start = Instant::now();
    channel.emit_fast(event.clone()); // Use fast emission for <1ms latency
    let emission_time = start.elapsed();
    
    // Verify emission completed in <1ms
    assert!(emission_time.as_micros() < 1000, 
        "Event emission took {:?}, expected <1ms", emission_time);
    
    // Verify event was received
    let received = timeout(Duration::from_millis(10), receiver.recv())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");
    
    assert_eq!(received.id, event.id);
    println!("Event emission latency: {:?}", emission_time);
}

#[tokio::test]
#[traced_test]
async fn test_broadcast_channel_overflow_handling() {
    // Create channel with small capacity to test overflow
    let config = ChannelConfig {
        buffer_size: 10,
        channel_capacity: 5, // Small capacity to trigger overflow
        enable_persistence: true,
        enable_metrics: true,
        max_subscribers: 10,
    };
    
    let (channel, mut receiver1) = PerformanceChannel::new(config);
    let mut receiver2 = channel.subscribe();
    
    // Emit more events than channel capacity
    let mut events_sent = 0;
    for i in 0..20 {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::System {
                service_name: format!("service_{}", i),
            })
            .event_type(PerformanceEventType::MetricsUpdate {
                component: format!("component_{}", i),
                metrics: Default::default(),
                timestamp: chrono::Utc::now(),
            })
            .build()
            .unwrap();
        
        channel.emit_fast(event);
        events_sent += 1;
    }
    
    // Check channel statistics for dropped events
    let stats = channel.get_statistics().await.unwrap();
    assert!(stats.dropped_events > 0, "Should have dropped events due to overflow");
    assert_eq!(stats.total_events_emitted as usize, events_sent);
    
    // Receivers should handle lagged state gracefully
    let mut received_count = 0;
    while let Ok(result) = timeout(Duration::from_millis(100), receiver1.recv()).await {
        match result {
            Ok(_) => received_count += 1,
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                println!("Receiver lagged, missed {} events", missed);
                assert!(missed > 0);
                break;
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    
    println!("Sent: {}, Received: {}, Dropped: {}", 
        events_sent, received_count, stats.dropped_events);
}

#[tokio::test]
#[traced_test]
async fn test_metrics_collection_events_and_subscribers() {
    let config = ChannelConfig::default();
    let (channel, _receiver1) = PerformanceChannel::new(config);
    
    // Add multiple subscribers
    let _receiver2 = channel.subscribe();
    let _receiver3 = channel.subscribe();
    let _receiver4 = channel.subscribe();
    
    // Emit various events
    for i in 0..10 {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test_model".to_string(),
                predictor_id: format!("pred_{}", i),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test_model".to_string(),
                accuracy: 0.9 - (i as f64 * 0.01),
                confidence: 0.85,
                latency_ms: 100 + i * 10,
                input_features: 10,
                output_dimension: 1,
                timestamp: chrono::Utc::now(),
            })
            .build()
            .unwrap();
        
        channel.emit(event).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    // Check comprehensive metrics
    let stats = channel.get_statistics().await.unwrap();
    
    // Verify events metrics
    assert_eq!(stats.total_events_emitted, 10);
    assert!(stats.events_per_second > 0.0);
    assert!(stats.average_latency_ms > 0.0);
    assert!(stats.average_latency_ms < 100.0); // Should be fast
    
    // Verify subscriber metrics
    assert_eq!(stats.active_subscribers, 4); // 1 initial + 3 subscribed
    
    // Verify buffer utilization
    assert!(stats.buffer_utilization_percent > 0.0);
    assert!(stats.buffer_utilization_percent <= 100.0);
    
    // Verify timestamp tracking
    assert!(stats.last_event_timestamp.is_some());
    
    println!("Channel statistics: {:?}", stats);
}

#[tokio::test]
#[traced_test]
async fn test_critical_alert_emission() {
    let config = ChannelConfig::default();
    let (channel, mut receiver) = PerformanceChannel::new(config);
    
    // Create channel for critical alerts
    let mut critical_receiver = channel.subscribe_to_critical_alerts().await;
    
    // Emit non-critical event
    let normal_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::System { 
            service_name: "test".to_string() 
        })
        .event_type(PerformanceEventType::MetricsUpdate {
            component: "test".to_string(),
            metrics: Default::default(),
            timestamp: chrono::Utc::now(),
        })
        .priority(EventPriority::Low)
        .build()
        .unwrap();
    
    channel.emit(normal_event).await.unwrap();
    
    // Emit critical performance degradation
    let critical_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "production_model".to_string(),
            predictor_id: "prod_1".to_string(),
        })
        .event_type(PerformanceEventType::PerformanceDegradation {
            metric_name: "accuracy".to_string(),
            current_value: 0.45,
            baseline_value: 0.90,
            degradation_percent: 50.0,
            impact_severity: "critical".to_string(),
        })
        .priority(EventPriority::Critical)
        .build()
        .unwrap();
    
    channel.emit_critical_alert(critical_event.clone()).await.unwrap();
    
    // Critical receiver should only get critical event
    let received_critical = timeout(Duration::from_millis(100), critical_receiver.recv())
        .await
        .expect("Timeout waiting for critical event")
        .expect("Failed to receive critical event");
    
    assert_eq!(received_critical.id, critical_event.id);
    assert!(matches!(received_critical.priority, EventPriority::Critical));
    
    // Regular receiver should get both events
    let received1 = receiver.recv().await.unwrap();
    let received2 = receiver.recv().await.unwrap();
    
    assert_ne!(received1.id, received2.id);
}

#[tokio::test]
#[traced_test]
async fn test_performance_statistics_by_event_type() {
    let config = ChannelConfig::default();
    let (channel, _receiver) = PerformanceChannel::new(config);
    
    // Emit different types of events
    // 5 predictions
    for i in 0..5 {
        let event = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "model".to_string(),
                accuracy: 0.9,
                confidence: 0.85,
                latency_ms: 100,
                input_features: 10,
                output_dimension: 1,
                timestamp: chrono::Utc::now(),
            })
            .build()
            .unwrap();
        channel.emit(event).await.unwrap();
    }
    
    // 3 alerts
    for i in 0..3 {
        let event = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::Alert {
                alert_type: AlertType::HighLatency,
                message: format!("High latency detected {}", i),
                severity: AlertSeverity::Warning,
                resolution_required: false,
            })
            .build()
            .unwrap();
        channel.emit(event).await.unwrap();
    }
    
    // 2 system health updates
    for i in 0..2 {
        let event = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::SystemHealth {
                component: "neural_engine".to_string(),
                cpu_usage_percent: 45.0 + i as f64,
                memory_usage_mb: 1024.0,
                error_rate: 0.01,
                availability_percent: 99.9,
            })
            .build()
            .unwrap();
        channel.emit(event).await.unwrap();
    }
    
    // Get statistics by event type
    let stats_by_type = channel.get_statistics_by_event_type().await.unwrap();
    
    assert_eq!(stats_by_type.get("PredictionCompleted").unwrap(), &5);
    assert_eq!(stats_by_type.get("Alert").unwrap(), &3);
    assert_eq!(stats_by_type.get("SystemHealth").unwrap(), &2);
    
    println!("Statistics by event type: {:?}", stats_by_type);
}

#[tokio::test]
#[traced_test]
async fn test_high_throughput_performance() {
    let config = ChannelConfig {
        buffer_size: 10000,
        channel_capacity: 100000,
        enable_persistence: true,
        enable_metrics: true,
        max_subscribers: 10,
    };
    
    let (channel, mut receiver) = PerformanceChannel::new(config);
    let channel = Arc::new(channel);
    
    // Spawn multiple producers to simulate high throughput
    let mut handles = vec![];
    let events_per_producer = 1000;
    let num_producers = 10;
    
    let start = Instant::now();
    
    for producer_id in 0..num_producers {
        let channel_clone = channel.clone();
        let handle = tokio::spawn(async move {
            for i in 0..events_per_producer {
                let event = PerformanceEventBuilder::new()
                    .source(PerformanceSource::System {
                        service_name: format!("producer_{}", producer_id),
                    })
                    .event_type(PerformanceEventType::MetricsUpdate {
                        component: format!("component_{}_{}", producer_id, i),
                        metrics: Default::default(),
                        timestamp: chrono::Utc::now(),
                    })
                    .build()
                    .unwrap();
                
                channel_clone.emit_fast(event);
            }
        });
        handles.push(handle);
    }
    
    // Wait for all producers
    for handle in handles {
        handle.await.unwrap();
    }
    
    let elapsed = start.elapsed();
    let total_events = events_per_producer * num_producers;
    let events_per_second = total_events as f64 / elapsed.as_secs_f64();
    
    println!("Throughput test results:");
    println!("  Total events: {}", total_events);
    println!("  Time elapsed: {:?}", elapsed);
    println!("  Events/second: {:.0}", events_per_second);
    
    // Verify throughput exceeds 10k events/sec
    assert!(events_per_second > 10000.0, 
        "Throughput {:.0} events/sec is below 10k requirement", events_per_second);
    
    // Check final statistics
    let stats = channel.get_statistics().await.unwrap();
    assert!(stats.total_events_emitted >= total_events as u64);
}