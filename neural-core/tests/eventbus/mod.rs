// EventBus module tests using ProtoEvent
// Following London School TDD - focus on interactions and contracts with proto-only messaging

pub mod trait_compliance;
pub mod channel_validation;
pub mod error_handling;

// Common test utilities and mocks for proto events
use mockall::predicate::*;
use mockall::mock;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// Import our real traits and types for proto events
use neural_core::eventbus::{
    traits::{
        ProtoEventBus, ProtoEventSubscriber, DynamicProtoEventSubscriber,
        ProtoChannelInfo
    },
    types::{ProtoEventEnvelope, ProtoEvent, ProtoMessage, SubscriptionConfig, EventId},
    error::EventBusError,
    proto_messages::*,
};

// Mock proto event type for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, prost::Message)]
pub struct MockEvent {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub data: String,
    #[prost(int64, tag = "3")]
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

impl ProtoMessage for MockEvent {
    fn proto_type_name() -> &'static str {
        "test.MockEvent"
    }
    
    fn validate(&self) -> Result<(), EventBusError> {
        if self.data.is_empty() {
            return Err(EventBusError::schema_validation("Mock event data cannot be empty"));
        }
        Ok(())
    }
}

// Mock implementations using mockall for ProtoEventBus
mock! {
    pub ProtoEventBusImpl {}

    #[async_trait]
    impl ProtoEventBus for ProtoEventBusImpl {
        async fn publish_proto<T: ProtoMessage + Default>(
            &self,
            channel: &str,
            event: ProtoEvent<T>,
        ) -> Result<EventId, EventBusError>;
        
        async fn publish_proto_batch<T: ProtoMessage + Default>(
            &self,
            channel: &str,
            events: Vec<ProtoEvent<T>>,
        ) -> Result<Vec<EventId>, EventBusError>;
        
        async fn subscribe_proto<T: ProtoMessage + Default>(
            &self,
            channels: &[String],
            config: SubscriptionConfig,
        ) -> Result<Box<dyn ProtoEventSubscriber<T>>, EventBusError>;
        
        async fn subscribe_dynamic_proto(
            &self,
            channels: &[String],
            proto_types: &[&'static str],
            config: SubscriptionConfig,
        ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError>;
        
        async fn ack_proto(
            &self, 
            channel: &str, 
            group: &str, 
            event_id: &EventId
        ) -> Result<(), EventBusError>;
        
        async fn nack_proto(
            &self, 
            channel: &str, 
            group: &str, 
            event_id: &EventId
        ) -> Result<(), EventBusError>;
        
        async fn create_proto_consumer_group(
            &self, 
            channel: &str, 
            group: &str
        ) -> Result<(), EventBusError>;
        
        async fn get_proto_channel_info(&self, channel: &str) -> Result<ProtoChannelInfo, EventBusError>;
        
        async fn list_proto_types_on_channel(&self, channel: &str) -> Result<Vec<String>, EventBusError>;
    }
}

mock! {
    pub ProtoEventSubscriberImpl<T> {}

    #[async_trait]
    impl<T: ProtoMessage + Default> ProtoEventSubscriber<T> for ProtoEventSubscriberImpl<T> {
        async fn next_proto(&mut self) -> Result<Option<ProtoEvent<T>>, EventBusError>;
        async fn next_proto_envelope(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError>;
        async fn close(&mut self) -> Result<(), EventBusError>;
        fn id(&self) -> &str;
    }
}

// Test utilities for proto events
pub struct TestContext {
    pub event_bus: MockProtoEventBusImpl,
    pub mock_subscriber: MockProtoEventSubscriberImpl<MockEvent>,
}

impl TestContext {
    pub fn new() -> Self {
        Self {
            event_bus: MockProtoEventBusImpl::new(),
            mock_subscriber: MockProtoEventSubscriberImpl::new(),
        }
    }
    
    pub fn new_with_proto_events() -> Self {
        let mut context = Self::new();
        
        // Set up common expectations for proto events
        // Note: mockall expectations would be set up here in a real implementation
        // For now, just return the context
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_mock_proto_event_creation() {
        let event = MockEvent::new("test proto data");
        assert_eq!(event.data, "test proto data");
        assert!(!event.id.is_empty());
        assert!(event.timestamp > 0);
        
        // Test proto message validation
        assert!(event.validate().is_ok());
        
        // Test proto type name
        assert_eq!(MockEvent::proto_type_name(), "test.MockEvent");
    }
    
    #[test]
    fn test_proto_event_wrapper() {
        let mock_message = MockEvent::new("test proto data");
        let proto_event = ProtoEvent::new(mock_message.clone())
            .with_quality_score(0.95)
            .with_metadata("test_key".to_string(), "test_value".to_string());
        
        assert_eq!(proto_event.event_type, "test.MockEvent");
        assert_eq!(proto_event.message.data, "test proto data");
        assert_eq!(proto_event.quality_score, 0.95);
        assert!(proto_event.metadata.contains_key("test_key"));
        
        // Test validation
        assert!(proto_event.validate().is_ok());
    }

    #[test]
    fn test_context_creation() {
        let _context = TestContext::new();
        // Just verify we can create the test context for proto events
        
        let _proto_context = TestContext::new_with_proto_events();
        // Verify we can create context with proto event expectations
    }
    
    #[test]
    fn test_real_proto_messages() {
        // Test with actual proto messages from our system
        let trade_event = MarketDataEvent::new_trade("AAPL", 150.0, 100.0, "NASDAQ");
        assert!(trade_event.validate().is_ok());
        
        let proto_trade = ProtoEvent::new(trade_event);
        assert_eq!(proto_trade.proto_type_name(), "neural_trader.market_data.v1.MarketDataEvent");
        assert!(proto_trade.validate().is_ok());
        
        let order_event = OrderRequest::new_market_buy("AAPL", 100.0);
        assert!(order_event.validate().is_ok());
        
        let proto_order = ProtoEvent::new(order_event);
        assert_eq!(proto_order.proto_type_name(), "neural_trader.trading.v1.OrderRequest");
        assert!(proto_order.validate().is_ok());
    }
}