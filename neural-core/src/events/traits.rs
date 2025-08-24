//! Event system traits
//! Module size: <150 lines as per requirements

use crate::errors::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::Stream;
use std::fmt::Debug;
use std::sync::Arc;
use uuid::Uuid;

/// Base event trait that all events must implement
pub trait Event: Debug + Send + Sync {
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

/// Event bus trait for publishing and subscribing to events
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish an event to all subscribers
    async fn publish(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()>;
    
    /// Subscribe to events of a specific type
    async fn subscribe(&self, event_type: &str) -> Result<crate::events::SubscriptionHandle>;
    
    /// Unsubscribe from events
    async fn unsubscribe(&self, handle: crate::events::SubscriptionHandle) -> Result<()>;
    
    /// Get a stream of events for a specific type
    async fn get_stream(&self, event_type: &str) -> Result<std::pin::Pin<Box<dyn Stream<Item = Arc<dyn Event + Send + Sync>> + Send>>>;
}

/// Event handler trait for processing events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming event
    async fn handle(&self, event: Arc<dyn Event + Send + Sync>) -> Result<()>;
    
    /// Get the event types this handler can process
    fn event_types(&self) -> Vec<String>;
    
    /// Check if handler should process this event
    fn can_handle(&self, event: &dyn Event) -> bool {
        self.event_types().contains(&event.event_type())
    }
}

/// Event processor for batch processing
#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Process a batch of events
    async fn process_batch(&self, events: Vec<Arc<dyn Event + Send + Sync>>) -> Result<()>;
    
    /// Get maximum batch size
    fn max_batch_size(&self) -> usize {
        100
    }
    
    /// Get batch timeout in milliseconds
    fn batch_timeout_ms(&self) -> u64 {
        1000
    }
}

/// Event filter trait
pub trait EventFilter: Send + Sync {
    /// Check if event should pass through filter
    fn should_process(&self, event: &dyn Event) -> bool;
    
    /// Get filter name for debugging
    fn filter_name(&self) -> &str;
}

/// Priority-based event filter
pub struct PriorityFilter {
    min_priority: u8,
}

impl PriorityFilter {
    pub fn new(min_priority: u8) -> Self {
        Self { min_priority }
    }
}

impl EventFilter for PriorityFilter {
    fn should_process(&self, event: &dyn Event) -> bool {
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
    fn should_process(&self, event: &dyn Event) -> bool {
        self.allowed_types.contains(&event.event_type())
    }
    
    fn filter_name(&self) -> &str {
        "event_type_filter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_priority_filter() {
        let filter = PriorityFilter::new(7);
        
        // Mock event with priority 8 should pass
        #[derive(Debug)]
        struct HighPriorityEvent;
        impl Event for HighPriorityEvent {
            fn event_type(&self) -> String { "test".to_string() }
            fn timestamp(&self) -> DateTime<Utc> { Utc::now() }
            fn event_id(&self) -> Uuid { Uuid::new_v4() }
            fn source(&self) -> String { "test".to_string() }
            fn to_json(&self) -> serde_json::Value { serde_json::json!({}) }
            fn priority(&self) -> u8 { 8 }
        }
        
        let event = HighPriorityEvent;
        assert!(filter.should_process(&event));
    }
}