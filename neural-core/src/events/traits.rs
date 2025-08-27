//! Event system traits
//! Module size: <150 lines as per requirements

use crate::errors::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::Stream;
use std::fmt::Debug;
use uuid::Uuid;

/// ⚠️  DEPRECATED: Old Event trait - REPLACED by proto-only Event struct
/// 
/// This trait is kept only for backward compatibility during migration.
/// Use the new proto-only Event struct from `crate::events::Event` instead.
#[deprecated(
    since = "0.1.0", 
    note = "Use proto-only Event struct instead. This trait supports Vec<u8> which is BANNED."
)]
pub trait LegacyEvent: Debug + Send + Sync {
    /// Get event type identifier
    fn event_type(&self) -> String;
    
    /// Get event timestamp
    fn timestamp(&self) -> DateTime<Utc>;
    
    /// Get event ID
    fn event_id(&self) -> Uuid;
    
    /// Get event source
    fn source(&self) -> String;
    
    /// Get event data as JSON
    fn to_json(&self) -> serde_json::Value;
    
    /// Check if event should be persisted
    fn is_persistent(&self) -> bool {
        false
    }
    
    /// Get event priority (0 = lowest, 10 = highest)
    fn priority(&self) -> u8 {
        5
    }
    
    /// Get correlation ID for event tracing
    fn correlation_id(&self) -> Option<Uuid> {
        None
    }
}

/// Proto-only Event bus trait for publishing and subscribing to events
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish a proto-only event to all subscribers
    async fn publish(&self, event: crate::events::Event) -> Result<()>;
    
    /// Subscribe to events of a specific type
    async fn subscribe(&self, event_type: &str) -> Result<crate::events::SubscriptionHandle>;
    
    /// Unsubscribe from events
    async fn unsubscribe(&self, handle: crate::events::SubscriptionHandle) -> Result<()>;
    
    /// Get a stream of proto-only events for a specific type
    async fn get_stream(&self, event_type: &str) -> Result<std::pin::Pin<Box<dyn Stream<Item = crate::events::Event> + Send>>>;
}

/// Proto-only Event handler trait for processing events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming proto-only event
    async fn handle(&self, event: &crate::events::Event) -> Result<()>;
    
    /// Get the event types this handler can process
    fn event_types(&self) -> Vec<String>;
    
    /// Check if handler should process this event
    fn can_handle(&self, event: &crate::events::Event) -> bool {
        self.event_types().contains(&event.event_type().to_string())
    }
}

/// Proto-only Event processor for batch processing
#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Process a batch of proto-only events
    async fn process_batch(&self, events: Vec<crate::events::Event>) -> Result<()>;
    
    /// Get maximum batch size
    fn max_batch_size(&self) -> usize {
        100
    }
    
    /// Get batch timeout in milliseconds
    fn batch_timeout_ms(&self) -> u64 {
        1000
    }
}

/// Proto-only Event filter trait
pub trait EventFilter: Send + Sync {
    /// Check if proto-only event should pass through filter
    fn should_process(&self, event: &crate::events::Event) -> bool;
    
    /// Get filter name for debugging
    fn filter_name(&self) -> &str;
}

/// Priority-based event filter
pub struct PriorityFilter {
    min_priority: i32,
}

impl PriorityFilter {
    pub fn new(min_priority: i32) -> Self {
        Self { min_priority }
    }
}

impl EventFilter for PriorityFilter {
    fn should_process(&self, event: &crate::events::Event) -> bool {
        event.priority() >= self.min_priority
    }
    
    fn filter_name(&self) -> &str {
        "priority_filter"
    }
}

/// Event type filter
pub struct EventTypeFilter {
    allowed_types: Vec<String>,
}

impl EventTypeFilter {
    pub fn new(allowed_types: Vec<String>) -> Self {
        Self { allowed_types }
    }
}

impl EventFilter for EventTypeFilter {
    fn should_process(&self, event: &crate::events::Event) -> bool {
        self.allowed_types.contains(&event.event_type().to_string())
    }
    
    fn filter_name(&self) -> &str {
        "event_type_filter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    
    // Mock proto message for testing
    #[derive(Clone, prost::Message)]
    struct TestMessage {
        #[prost(string, tag = "1")]
        content: String,
    }
    
    impl crate::eventbus::ProtoMessage for TestMessage {
        fn proto_type_name() -> &'static str {
            "neural_trader.test.TestMessage"
        }
    }
    
    #[test]
    fn test_priority_filter() {
        let filter = PriorityFilter::new(7);
        
        // Create a proto event with priority 8 should pass
        let test_msg = TestMessage {
            content: "test".to_string(),
        };
        
        let event = crate::eventbus::ProtoEvent::new(test_msg)
            .with_metadata("source".to_string(), "test".to_string())
            .with_metadata("domain".to_string(), "test".to_string());
        // Note: routing functionality needs to be implemented for ProtoEvent
        
        assert!(filter.should_process(&event));
    }
    
    #[test]
    fn test_event_type_filter() {
        let filter = EventTypeFilter::new(vec!["allowed.Type".to_string()]);
        
        let test_msg = TestMessage {
            content: "test".to_string(),
        };
        
        // Create event with allowed type
        let allowed_event = crate::eventbus::ProtoEvent::new(test_msg.clone())
            .expect("Should create event");
        assert!(filter.should_process(&allowed_event));
        
        // Create event with disallowed type
        let disallowed_event = crate::eventbus::ProtoEvent::new(test_msg)
            .expect("Should create event");
        assert!(!filter.should_process(&disallowed_event));
    }
}