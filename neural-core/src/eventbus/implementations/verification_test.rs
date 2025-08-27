//! Verification tests for EventBus implementations
//!
//! Simple tests to verify that all implementations compile and basic functionality works.

#[cfg(test)]
mod tests {
    use super::super::{InMemoryEventBus, RecordingEventBus};
    use crate::eventbus::{
        traits::EventBus,
        types::{Event, SubscriptionConfig},
    };
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_inmemory_basic_operations() {
        let event_bus = InMemoryEventBus::new();
        
        // Create a test event
        let event = Event {
            event_type: "TestEvent".to_string(),
            payload: b"test payload".to_vec(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        // Test publish
        let event_id = event_bus
            .publish("stream:symbol:TEST", event.clone())
            .await
            .expect("Failed to publish event");
        
        assert!(!event_id.to_string().is_empty());
        
        // Test channel info
        let channel_info = event_bus
            .get_channel_info("stream:symbol:TEST")
            .await
            .expect("Failed to get channel info");
        
        assert_eq!(channel_info.channel_name, "stream:symbol:TEST");
        assert_eq!(channel_info.message_count, 1);
        
        // Test subscription
        let config = SubscriptionConfig::default();
        let _subscriber = event_bus
            .subscribe(&["stream:symbol:TEST".to_string()], config)
            .await
            .expect("Failed to create subscription");
    }
    
    #[tokio::test]
    async fn test_recording_wrapper() {
        let inner = Box::new(InMemoryEventBus::new());
        let recording_bus = RecordingEventBus::new(inner);
        
        // Create a test event
        let event = Event {
            event_type: "RecordingTest".to_string(),
            payload: b"recording test".to_vec(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        // Test publish with recording
        let event_id = recording_bus
            .publish("stream:symbol:RECORD", event)
            .await
            .expect("Failed to publish to recording bus");
        
        assert!(!event_id.to_string().is_empty());
        
        // Verify recording captured the event
        assert!(recording_bus
            .assert_event_published("stream:symbol:RECORD", "RecordingTest")
            .await);
        
        assert_eq!(recording_bus.get_publish_count(None).await, 1);
        assert_eq!(recording_bus.get_publish_count(Some("stream:symbol:RECORD")).await, 1);
    }
    
    #[tokio::test]
    async fn test_channel_validation() {
        let event_bus = InMemoryEventBus::new();
        let event = Event::new("Test".to_string(), vec![1, 2, 3]);
        
        // Valid channel names
        assert!(event_bus.publish("stream:symbol:AAPL", event.clone()).await.is_ok());
        assert!(event_bus.publish("stream:sector:technology", event.clone()).await.is_ok());
        assert!(event_bus.publish("stream:ml:training", event.clone()).await.is_ok());
        
        // Invalid channel names should fail
        assert!(event_bus.publish("invalid:channel", event.clone()).await.is_err());
        assert!(event_bus.publish("market:AAPL", event.clone()).await.is_err()); // Old format
        assert!(event_bus.publish("stream:invalid_domain:test", event).await.is_err());
    }
    
    #[tokio::test]
    async fn test_channel_info_structure() {
        let event_bus = InMemoryEventBus::new();
        let event = Event::new("InfoTest".to_string(), vec![42]);
        
        // Publish some events
        event_bus.publish("stream:symbol:INFO", event.clone()).await.unwrap();
        event_bus.publish("stream:symbol:INFO", event).await.unwrap();
        
        let info = event_bus.get_channel_info("stream:symbol:INFO").await.unwrap();
        
        // Verify all required fields are present
        assert_eq!(info.channel_name, "stream:symbol:INFO");
        assert_eq!(info.name, "stream:symbol:INFO");
        assert_eq!(info.message_count, 2);
        assert_eq!(info.total_events, 2);
        assert!(info.created_at > 0);
        assert!(info.last_event_id.is_some());
        // In a real implementation, these would have meaningful values
        assert_eq!(info.subscriber_count, 0);
        assert!(!info.active); // No active subscribers yet
        assert!(info.consumer_groups.is_empty());
    }
}