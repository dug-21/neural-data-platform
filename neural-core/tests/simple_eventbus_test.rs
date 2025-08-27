//! Simple EventBus implementation test using ProtoEvent
//! 
//! Tests that our EventBus traits compile and can be used correctly with proto-only messages

use neural_core::eventbus::{
    error::EventBusError,
    types::{EventId, ProtoEvent, ProtoEventEnvelope, ProtoMessage},
    proto_messages::*,
};

#[tokio::test] 
async fn test_eventbus_traits_compile() {
    // This test verifies that our proto traits compile and the types exist
    
    // Create a simple proto event for testing
    let trade_event = MarketDataEvent::new_trade("TEST", 100.0, 50.0, "TEST_EXCHANGE");
    let proto_event = ProtoEvent::new(trade_event.clone());
    
    // Verify proto event properties
    assert_eq!(proto_event.event_type, "neural_trader.market_data.v1.MarketDataEvent");
    assert_eq!(proto_event.message.symbol, "TEST");
    assert!(proto_event.quality_score >= 0.0 && proto_event.quality_score <= 1.0);
    
    // Verify EventId works
    let id = EventId::new();
    assert!(!id.to_string().is_empty());
    
    // Test proto event envelope creation
    let test_envelope = ProtoEventEnvelope::new(
        id.clone(),
        "test_channel".to_string(),
        proto_event
    );
    
    assert!(test_envelope.is_ok());
    let envelope = test_envelope.unwrap();
    
    assert_eq!(envelope.event_id, id);
    assert_eq!(envelope.channel, "test_channel");
    assert_eq!(envelope.proto_type, "neural_trader.market_data.v1.MarketDataEvent");
    
    println!("EventBus proto traits and types compile successfully!");
}

#[tokio::test]
async fn test_eventbus_error_types() {
    // Test that our error types work correctly with proto validation
    let error = EventBusError::channel_not_found("test_channel");
    assert!(matches!(error, EventBusError::ChannelNotFound(_)));
    
    let error = EventBusError::subscriber_not_found("test_sub");
    assert!(matches!(error, EventBusError::SubscriberNotFound(_)));
    
    let error = EventBusError::internal("test message");
    assert!(matches!(error, EventBusError::Internal(_)));
    
    let error = EventBusError::send_failed("send error");
    assert!(matches!(error, EventBusError::SendFailed(_)));
    
    // Test proto-specific errors
    let error = EventBusError::contract_violation("Vec<u8> payloads are REJECTED");
    assert!(matches!(error, EventBusError::ContractViolation(_)));
    
    let error = EventBusError::schema_validation("Invalid proto message");
    assert!(matches!(error, EventBusError::SchemaValidation(_)));
    
    let error = EventBusError::proto_deserialization("Failed to decode proto");
    assert!(matches!(error, EventBusError::ProtoDeserialization(_)));
    
    println!("EventBus proto error types work correctly!");
}

#[test]
fn test_eventbus_result_type() {
    // Test our Result type alias with proto events
    type EventBusResult<T> = Result<T, EventBusError>;
    
    let ok_result: EventBusResult<ProtoEvent<MarketDataEvent>> = {
        let trade_event = MarketDataEvent::new_trade("AAPL", 150.0, 100.0, "NASDAQ");
        Ok(ProtoEvent::new(trade_event))
    };
    assert!(ok_result.is_ok());
    
    let err_result: EventBusResult<ProtoEvent<MarketDataEvent>> = 
        Err(EventBusError::contract_violation("Test error"));
    assert!(err_result.is_err());
    
    println!("EventBus Result type works correctly with proto events!");
}

#[test]
fn test_proto_message_validation() {
    // Test that proto message validation works
    let valid_trade = MarketDataEvent::new_trade("AAPL", 150.0, 100.0, "NASDAQ");
    assert!(valid_trade.validate().is_ok());
    
    let valid_order = OrderRequest::new_market_buy("AAPL", 100.0);
    assert!(valid_order.validate().is_ok());
    
    // Test invalid order with zero quantity
    let mut invalid_order = OrderRequest::new_market_buy("AAPL", 100.0);
    invalid_order.quantity = 0.0;
    assert!(invalid_order.validate().is_err());
    
    println!("Proto message validation works correctly!");
}