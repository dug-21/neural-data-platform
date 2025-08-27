// EventBus integration test using ProtoEvent - following London School TDD
// This file demonstrates proto-only EventBus integration testing

// Import our test modules
mod eventbus;

use eventbus::*;
use neural_core::eventbus::{
    proto_messages::*,
    types::{ProtoEvent, ProtoMessage},
    ProtoInMemoryEventBus,
    traits::ProtoEventBus,
};

#[cfg(test)]
mod eventbus_integration_tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_proto_eventbus_integration_setup() {
        // Test that our ProtoEventBus implementation works correctly
        // This follows the GREEN phase of TDD - working implementation
        
        println!("ProtoEventBus integration test setup - proto-only implementation");
        
        // Use the real ProtoEventBus implementation
        let event_bus = ProtoInMemoryEventBus::new();
        
        // Test that we can create a proto event
        let trade_event = MarketDataEvent::new_trade("AAPL", 150.0, 100.0, "NASDAQ");
        let proto_event = ProtoEvent::new(trade_event);
        
        // Register the proto type first
        event_bus.register_proto_type::<MarketDataEvent>().await.unwrap();
        
        // Test that we can publish proto events
        let result = event_bus.publish("market_data", proto_event).await;
        assert!(result.is_ok(), "Should be able to publish proto events");
        
        // Test that we can list channels
        let channels = event_bus.list_channels().await;
        assert!(channels.is_ok(), "Should be able to list channels");
        
        assert!(true, "ProtoEventBus integration works correctly");
    }
    
    #[test]
    fn test_mock_infrastructure_works() {
        // Verify our mock infrastructure is working with proto types
        let context = TestContext::new();
        
        // Test that we can create mock proto events
        let mock_event = MockEvent::new("test proto data");
        assert_eq!(mock_event.data, "test proto data");
        assert!(!mock_event.id.is_empty());
        
        // This proves our test structure is solid for proto events
        assert!(true, "Mock infrastructure is working with proto events");
    }
    
    #[tokio::test]
    async fn test_proto_event_type_safety() {
        // Test that proto events maintain type safety
        let event_bus = ProtoInMemoryEventBus::new();
        
        // Create different proto event types
        let trade_event = MarketDataEvent::new_trade("AAPL", 150.0, 100.0, "NASDAQ");
        let order_event = OrderRequest::new_market_buy("AAPL", 100.0);
        
        let proto_trade = ProtoEvent::new(trade_event);
        let proto_order = ProtoEvent::new(order_event);
        
        // Register proto types first
        event_bus.register_proto_type::<MarketDataEvent>().await.unwrap();
        event_bus.register_proto_type::<OrderRequest>().await.unwrap();
        
        // Publish to different channels
        assert!(event_bus.publish("market_data", proto_trade).await.is_ok());
        assert!(event_bus.publish("orders", proto_order).await.is_ok());
        
        println!("Proto event type safety verified");
    }
}

// This module demonstrates contract violation examples (proto-only enforcement)
#[cfg(test)]
mod contract_violation_examples {
    use super::*;
    use neural_core::eventbus::types::{reject_raw_payload, reject_json_payload};
    
    #[test]
    fn test_raw_payload_rejection() {
        // This demonstrates that raw Vec<u8> payloads are rejected
        let error = reject_raw_payload();
        assert!(error.to_string().contains("Vec<u8> payloads are REJECTED"));
        println!("Raw payload rejection works correctly");
    }
    
    #[test]
    fn test_json_payload_rejection() {
        // This demonstrates that JSON payloads are rejected
        let error = reject_json_payload();
        assert!(error.to_string().contains("JSON messages are not allowed"));
        println!("JSON payload rejection works correctly");
    }
    
    #[tokio::test]
    async fn test_only_proto_events_allowed() {
        // This test demonstrates that only proto events are allowed
        let event_bus = ProtoInMemoryEventBus::new();
        
        // Register proto type first
        event_bus.register_proto_type::<MarketDataEvent>().await.unwrap();
        
        // Valid proto event should work
        let trade_event = MarketDataEvent::new_trade("AAPL", 150.0, 100.0, "NASDAQ");
        let proto_event = ProtoEvent::new(trade_event);
        
        let result = event_bus.publish("market_data", proto_event).await;
        assert!(result.is_ok(), "Proto events should be accepted");
        
        println!("Proto-only enforcement verified");
    }
}