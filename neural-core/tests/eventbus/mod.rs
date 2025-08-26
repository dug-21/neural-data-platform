// EventBus module tests
// Following London School TDD - focus on interactions and contracts

pub mod trait_compliance;
pub mod channel_validation;
pub mod error_handling;

// Common test utilities and mocks
use mockall::predicate::*;
use mockall::mock;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// Import our real traits and types
use neural_core::eventbus::{EventBus as EventBusTrait, EventSubscriber, EventBusError};

// Mock event type for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockEvent {
    pub id: String,
    pub data: String,
    pub timestamp: i64,
}

impl MockEvent {
    pub fn new(data: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            data: data.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

// Note: We now use the real traits from neural_core::eventbus

// Mock implementations using mockall
mock! {
    pub EventBusImpl {}

    #[async_trait]
    impl EventBusTrait for EventBusImpl {
        type Event = MockEvent;
        type Error = EventBusError;

        async fn publish(&self, channel: &str, event: Self::Event) -> Result<(), Self::Error>;
        async fn subscribe(&self, channel: &str) -> Result<Box<dyn EventSubscriber<Event = Self::Event>>, Self::Error>;
        async fn unsubscribe(&self, channel: &str, subscriber_id: &str) -> Result<(), Self::Error>;
        async fn list_channels(&self) -> Result<Vec<String>, Self::Error>;
        async fn channel_subscriber_count(&self, channel: &str) -> Result<usize, Self::Error>;
    }
}

mock! {
    pub EventSubscriberImpl {}

    #[async_trait]
    impl EventSubscriber for EventSubscriberImpl {
        type Event = MockEvent;

        fn id(&self) -> &str;
        async fn receive(&mut self) -> Option<Self::Event>;
        async fn close(&mut self);
    }
}

// Note: We now use the real EventBusError from neural_core::eventbus

// Test utilities
pub struct TestContext {
    pub event_bus: MockEventBusImpl,
    pub mock_subscriber: MockEventSubscriberImpl,
}

impl TestContext {
    pub fn new() -> Self {
        Self {
            event_bus: MockEventBusImpl::new(),
            mock_subscriber: MockEventSubscriberImpl::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_mock_event_creation() {
        let event = MockEvent::new("test data");
        assert_eq!(event.data, "test data");
        assert!(!event.id.is_empty());
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_context_creation() {
        let _context = TestContext::new();
        // Just verify we can create the test context
    }
}