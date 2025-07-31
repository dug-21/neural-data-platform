//! Event Subscription Mechanism Tests
//!
//! Tests for event bus subscription, async handling, and error scenarios

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{timeout, Duration};

// Import the actual event bus implementation
use neural_trader::integration::event_bus::{EventBus, EventBusConfig, SubscriberInfo};
use neural_trader::neural::monitoring::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder,
    PerformanceSource, PerformanceEventType, EventPriority, AlertType, AlertSeverity,
};

#[derive(Debug, Clone, PartialEq)]
struct TestEvent {
    id: String,
    timestamp: DateTime<Utc>,
    data: String,
    sequence: u64,
}

#[tokio::test]
async fn test_basic_event_subscription() {
    let event_bus = EventBus::<TestEvent>::with_defaults();
    
    // Subscribe multiple listeners
    let mut receiver1 = event_bus.subscribe("subscriber_1".to_string(), "test".to_string());
    let mut receiver2 = event_bus.subscribe("subscriber_2".to_string(), "test".to_string());
    let mut receiver3 = event_bus.subscribe("subscriber_3".to_string(), "test".to_string());
    
    // Create test event
    let test_event = TestEvent {
        id: "evt_001".to_string(),
        timestamp: Utc::now(),
        data: "Hello EventBus".to_string(),
        sequence: 1,
    };
    
    // Publish event
    let subscriber_count = event_bus.publish(test_event.clone()).await.unwrap();
    assert_eq!(subscriber_count, 3);
    
    // All receivers should get the event
    let recv1 = timeout(Duration::from_millis(100), receiver1.recv()).await;
    let recv2 = timeout(Duration::from_millis(100), receiver2.recv()).await;
    let recv3 = timeout(Duration::from_millis(100), receiver3.recv()).await;
    
    assert!(recv1.is_ok());
    assert!(recv2.is_ok());
    assert!(recv3.is_ok());
    
    assert_eq!(recv1.unwrap().unwrap(), test_event);
    assert_eq!(recv2.unwrap().unwrap(), test_event);
    assert_eq!(recv3.unwrap().unwrap(), test_event);
}

#[tokio::test]
async fn test_concurrent_event_publishing() {
    let event_bus = EventBus::<TestEvent>::with_defaults();
    let received_events = Arc::new(Mutex::new(Vec::new()));
    
    // Single subscriber to collect events
    let mut receiver = event_bus.subscribe("collector".to_string(), "test".to_string());
    let events_clone = Arc::clone(&received_events);
    
    // Spawn receiver task
    let receiver_task = tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            events_clone.lock().unwrap().push(event);
        }
    });
    
    // Publish events concurrently
    let publish_handles: Vec<_> = (0..10)
        .map(|i| {
            let bus = event_bus.clone();
            tokio::spawn(async move {
                let event = TestEvent {
                    id: format!("evt_{:03}", i),
                    timestamp: Utc::now(),
                    data: format!("Concurrent event {}", i),
                    sequence: i as u64,
                };
                bus.publish(event).await.unwrap()
            })
        })
        .collect();
    
    // Wait for all publishers
    for handle in publish_handles {
        handle.await.unwrap();
    }
    
    // Give receiver time to process
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Check all events were received
    let events = received_events.lock().unwrap();
    assert_eq!(events.len(), 10);
}

#[tokio::test]
async fn test_subscriber_management() {
    let event_bus = EventBus::<TestEvent>::with_defaults();
    
    // Add subscribers
    let _rx1 = event_bus.subscribe("sub_1".to_string(), "type_a".to_string());
    let _rx2 = event_bus.subscribe("sub_2".to_string(), "type_b".to_string());
    let _rx3 = event_bus.subscribe("sub_3".to_string(), "type_a".to_string());
    
    // Check subscriber info
    let subscribers = event_bus.get_subscribers();
    assert_eq!(subscribers.len(), 3);
    assert!(subscribers.contains_key("sub_1"));
    assert!(subscribers.contains_key("sub_2"));
    assert!(subscribers.contains_key("sub_3"));
    
    // Verify subscriber types
    assert_eq!(subscribers["sub_1"].subscriber_type, "type_a");
    assert_eq!(subscribers["sub_2"].subscriber_type, "type_b");
    
    // Unsubscribe one
    event_bus.unsubscribe("sub_2").unwrap();
    
    let subscribers_after = event_bus.get_subscribers();
    assert_eq!(subscribers_after.len(), 2);
    assert!(!subscribers_after.contains_key("sub_2"));
}

#[tokio::test]
async fn test_event_persistence_and_retrieval() {
    let config = EventBusConfig {
        max_stored_events: 5,
        enable_persistence: true,
        ..Default::default()
    };
    
    let event_bus = EventBus::<TestEvent>::new(config);
    
    // Publish more events than buffer size
    for i in 0..10 {
        let event = TestEvent {
            id: format!("persist_{:03}", i),
            timestamp: Utc::now(),
            data: format!("Persistent event {}", i),
            sequence: i as u64,
        };
        event_bus.publish(event).await.unwrap();
    }
    
    // Should only have last 5 events
    let recent = event_bus.get_recent_events(10);
    assert_eq!(recent.len(), 5);
    
    // Verify they are the most recent ones (in reverse order)
    assert_eq!(recent[0].sequence, 9);
    assert_eq!(recent[1].sequence, 8);
    assert_eq!(recent[2].sequence, 7);
    assert_eq!(recent[3].sequence, 6);
    assert_eq!(recent[4].sequence, 5);
}

#[tokio::test]
async fn test_performance_channel_integration() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Create performance event
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "test_model".to_string(),
            predictor_id: "pred_001".to_string(),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: "test_model".to_string(),
            accuracy: 0.92,
            confidence: 0.88,
            latency_ms: 45,
            input_features: 20,
            output_dimension: 3,
            timestamp: Utc::now(),
        })
        .priority(EventPriority::High)
        .tag("environment".to_string(), "test".to_string())
        .build()
        .unwrap();
    
    // Emit event
    channel.emit(event.clone()).await.unwrap();
    
    // Receive event
    let received = timeout(Duration::from_millis(100), receiver.recv())
        .await
        .expect("Timeout")
        .expect("Failed to receive");
    
    assert_eq!(received.id, event.id);
    assert_eq!(received.priority, EventPriority::High);
}

#[tokio::test]
async fn test_error_recovery_scenarios() {
    let event_bus = EventBus::<TestEvent>::with_defaults();
    
    // Subscribe and then drop receiver to simulate disconnection
    {
        let _receiver = event_bus.subscribe("temp_sub".to_string(), "test".to_string());
        // Receiver dropped here
    }
    
    // Publishing should still work even with dropped receivers
    let event = TestEvent {
        id: "error_test".to_string(),
        timestamp: Utc::now(),
        data: "Error scenario".to_string(),
        sequence: 1,
    };
    
    let result = event_bus.publish(event).await;
    assert!(result.is_ok());
    // Should return 0 or remaining subscriber count
}

#[tokio::test]
async fn test_event_ordering_guarantee() {
    let event_bus = EventBus::<TestEvent>::with_defaults();
    let mut receiver = event_bus.subscribe("order_test".to_string(), "test".to_string());
    
    // Publish events in sequence
    for i in 0..20 {
        let event = TestEvent {
            id: format!("order_{:03}", i),
            timestamp: Utc::now(),
            data: format!("Ordered event {}", i),
            sequence: i as u64,
        };
        event_bus.publish(event).await.unwrap();
    }
    
    // Verify order is maintained
    for i in 0..20 {
        let received = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("Timeout")
            .expect("Failed to receive");
        assert_eq!(received.sequence, i as u64);
    }
}

#[tokio::test]
async fn test_performance_channel_fast_emit() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(1000);
    
    // Use fast emit for high throughput
    for i in 0..100 {
        let event = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::MetricsUpdate {
                component: "test".to_string(),
                metrics: {
                    let mut m = HashMap::new();
                    m.insert("counter".to_string(), i as f64);
                    m
                },
                timestamp: Utc::now(),
            })
            .build()
            .unwrap();
        
        channel.emit_fast(event);
    }
    
    // Give some time for events to propagate
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Count received events
    let mut count = 0;
    while let Ok(result) = timeout(Duration::from_millis(10), receiver.recv()).await {
        if result.is_ok() {
            count += 1;
        } else {
            break;
        }
    }
    
    // Should receive most if not all events
    assert!(count > 90, "Expected > 90 events, got {}", count);
}

#[tokio::test]
async fn test_alert_event_prioritization() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Create events with different priorities
    let critical_event = PerformanceEventBuilder::new()
        .event_type(PerformanceEventType::Alert {
            alert_type: AlertType::SystemError,
            message: "Critical system failure".to_string(),
            severity: AlertSeverity::Critical,
            resolution_required: true,
        })
        .priority(EventPriority::Critical)
        .build()
        .unwrap();
    
    let info_event = PerformanceEventBuilder::new()
        .event_type(PerformanceEventType::Alert {
            alert_type: AlertType::Custom("Info".to_string()),
            message: "Informational message".to_string(),
            severity: AlertSeverity::Info,
            resolution_required: false,
        })
        .priority(EventPriority::Low)
        .build()
        .unwrap();
    
    // Emit both events
    channel.emit(info_event).await.unwrap();
    channel.emit(critical_event).await.unwrap();
    
    // Both should be received
    let recv1 = receiver.recv().await.unwrap();
    let recv2 = receiver.recv().await.unwrap();
    
    // Verify we got both events
    let priorities = vec![recv1.priority.clone(), recv2.priority.clone()];
    assert!(priorities.contains(&EventPriority::Critical));
    assert!(priorities.contains(&EventPriority::Low));
}

#[tokio::test]
async fn test_metrics_tracking() {
    let event_bus = EventBus::<TestEvent>::with_defaults();
    let _receiver = event_bus.subscribe("metrics_test".to_string(), "test".to_string());
    
    // Publish several events
    for i in 0..50 {
        let event = TestEvent {
            id: format!("metric_{:03}", i),
            timestamp: Utc::now(),
            data: "Metrics test".to_string(),
            sequence: i as u64,
        };
        event_bus.publish(event).await.unwrap();
    }
    
    // Check metrics
    let metrics = event_bus.get_metrics();
    assert_eq!(metrics.total_events_published, 50);
    assert_eq!(metrics.total_events_delivered, 50);
    assert_eq!(metrics.active_subscribers, 1);
    assert!(metrics.average_latency_ms >= 0.0);
}

#[tokio::test]
async fn test_channel_statistics() {
    let (channel, _receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Emit some events
    for i in 0..10 {
        let event = PerformanceEventBuilder::new()
            .custom_metric("test_metric".to_string(), i as f64)
            .build()
            .unwrap();
        channel.emit(event).await.unwrap();
    }
    
    // Get statistics
    let stats = channel.get_statistics().unwrap();
    assert_eq!(stats.total_events_emitted, 10);
    assert!(stats.average_latency_ms > 0.0);
    assert!(stats.buffer_utilization_percent > 0.0);
}

// Test helper for creating events with specific patterns
fn create_pattern_event(pattern: &str, seq: u64) -> TestEvent {
    TestEvent {
        id: format!("{}_{:03}", pattern, seq),
        timestamp: Utc::now(),
        data: format!("Pattern: {}", pattern),
        sequence: seq,
    }
}