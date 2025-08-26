//! Simple EventBus implementation test
//! 
//! Tests that our EventBus traits compile and can be used correctly

use neural_core::eventbus::{EventBusError, EventId, EventEnvelope, Event};

#[tokio::test] 
async fn test_eventbus_traits_compile() {
    // This test verifies that our traits compile and the types exist
    
    // Create a simple event for testing
    let event = EventEnvelope::new("test_event".to_string(), "test data".to_string());
    
    // Verify event properties
    assert_eq!(event.event_type(), "test_event");
    assert_eq!(event.data, "test data");
    
    // Verify EventId works
    let id = EventId::new();
    assert!(!id.to_string().is_empty());
    
    println!("EventBus traits and types compile successfully!");
}

#[tokio::test]
async fn test_eventbus_error_types() {
    // Test that our error types work correctly
    let error = EventBusError::channel_not_found("test_channel");
    assert!(matches!(error, EventBusError::ChannelNotFound(_)));
    
    let error = EventBusError::subscriber_not_found("test_sub");
    assert!(matches!(error, EventBusError::SubscriberNotFound(_)));
    
    let error = EventBusError::internal("test message");
    assert!(matches!(error, EventBusError::Internal(_)));
    
    let error = EventBusError::send_failed("send error");
    assert!(matches!(error, EventBusError::SendFailed(_)));
    
    println!("EventBus error types work correctly!");
}

#[test]
fn test_eventbus_result_type() {
    // Test our Result type alias
    let ok_result: neural_core::eventbus::Result<String> = Ok("success".to_string());
    assert!(ok_result.is_ok());
    
    let err_result: neural_core::eventbus::Result<String> = Err(EventBusError::internal("error"));
    assert!(err_result.is_err());
    
    println!("EventBus Result type works correctly!");
}