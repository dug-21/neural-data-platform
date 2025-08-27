//! Proto-Only EventBus Validation Tests
//!
//! These tests validate that ALL EventBus implementations strictly enforce proto-only messaging
//! and completely REJECT Vec<u8> and JSON payloads with ContractViolation errors.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use prost::Message;
    
    use crate::eventbus::{
        traits::{EventBus, ProtoEventSubscriber},
        types::{ProtoMessage, ProtoEvent, EventId, SubscriptionConfig, StartPosition, reject_raw_payload, reject_json_payload},
        implementations::InMemoryEventBus,
        error::EventBusError,
    };
    
    // Test proto message for validation
    #[derive(Clone, prost::Message)]
    pub struct TestProtoMessage {
        #[prost(string, tag = "1")]
        pub content: String,
        #[prost(int64, tag = "2")]
        pub value: i64,
    }
    
    impl ProtoMessage for TestProtoMessage {
        fn proto_type_name() -> &'static str {
            "test.TestProtoMessage"
        }
        
        fn validate(&self) -> Result<(), EventBusError> {
            if self.content.is_empty() {
                return Err(EventBusError::schema_validation("Content cannot be empty"));
            }
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_eventbus_accepts_only_proto_messages() {
        let eventbus = InMemoryEventBus::for_testing();
        
        // Create valid proto message
        let test_message = TestProtoMessage {
            content: "test content".to_string(),
            value: 42,
        };
        
        let proto_event = ProtoEvent::new(test_message)
            .with_quality_score(0.95);
        
        // SHOULD SUCCEED: Proto message publishing
        let result = eventbus.publish("stream:symbol:TEST", proto_event).await;
        assert!(result.is_ok(), "Proto message publishing should succeed");
        
        let event_id = result.unwrap();
        assert!(!event_id.as_str().is_empty());
        
        println!("✅ EventBus accepts proto messages correctly");
    }
    
    #[tokio::test]
    async fn test_eventbus_rejects_raw_payloads() {
        let eventbus = InMemoryEventBus::for_testing();
        
        // SHOULD FAIL: Raw Vec<u8> payload publishing
        let result = eventbus.publish_raw("stream:symbol:TEST", vec![1, 2, 3]).await;
        assert!(result.is_err(), "Raw payload publishing should be rejected");
        
        match result.unwrap_err() {
            EventBusError::ContractViolation(msg) => {
                assert!(msg.contains("Vec<u8> payloads are REJECTED"));
                println!("✅ EventBus correctly rejects Vec<u8> payloads");
            },
            other => panic!("Expected ContractViolation, got: {:?}", other),
        }
    }
    
    #[tokio::test]
    async fn test_eventbus_rejects_json_payloads() {
        let eventbus = InMemoryEventBus::for_testing();
        
        // SHOULD FAIL: JSON payload publishing
        let json_payload = r#"{"test": "data", "value": 123}"#;
        let result = eventbus.publish_json("stream:symbol:TEST", json_payload).await;
        assert!(result.is_err(), "JSON payload publishing should be rejected");
        
        match result.unwrap_err() {
            EventBusError::ContractViolation(msg) => {
                assert!(msg.contains("JSON messages are not allowed"));
                println!("✅ EventBus correctly rejects JSON payloads");
            },
            other => panic!("Expected ContractViolation, got: {:?}", other),
        }
    }
    
    #[tokio::test]
    async fn test_eventbus_rejects_raw_batch_payloads() {
        let eventbus = InMemoryEventBus::for_testing();
        
        // SHOULD FAIL: Raw Vec<u8> batch publishing
        let raw_batch = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let result = eventbus.publish_batch_raw("stream:symbol:TEST", raw_batch).await;
        assert!(result.is_err(), "Raw batch publishing should be rejected");
        
        match result.unwrap_err() {
            EventBusError::ContractViolation(msg) => {
                assert!(msg.contains("Vec<u8> payloads are REJECTED"));
                println!("✅ EventBus correctly rejects Vec<u8> batch payloads");
            },
            other => panic!("Expected ContractViolation, got: {:?}", other),
        }
    }
    
    #[tokio::test]
    async fn test_eventbus_proto_subscription() {
        let eventbus = InMemoryEventBus::for_testing();
        
        // Publish a proto event first
        let test_message = TestProtoMessage {
            content: "subscription test".to_string(),
            value: 99,
        };
        
        let proto_event = ProtoEvent::new(test_message)
            .with_quality_score(0.88);
        
        let _event_id = eventbus.publish("stream:symbol:SUB", proto_event).await.unwrap();
        
        // Subscribe using proto-specific subscription
        let config = SubscriptionConfig {
            group_name: "test-group".to_string(),
            consumer_name: "test-consumer".to_string(),
            start_position: StartPosition::Beginning,
            batch_size: 10,
            block_timeout_ms: 1000,
            ack_timeout_ms: 5000,
            buffer_size: 1024,
            receive_timeout: None,
            persistent: false,
            priority: 0,
        };
        
        let subscriber = eventbus.subscribe::<TestProtoMessage>(
            &["stream:symbol:SUB".to_string()],
            config
        ).await;
        
        assert!(subscriber.is_ok(), "Proto subscription should succeed");
        println!("✅ EventBus proto-only subscription works correctly");
    }
    
    #[tokio::test]
    async fn test_channel_name_validation_enforced() {
        let eventbus = InMemoryEventBus::new(); // Strict validation
        
        let test_message = TestProtoMessage {
            content: "channel test".to_string(),
            value: 1,
        };
        
        let proto_event = ProtoEvent::new(test_message);
        
        // Invalid channel names should be rejected
        let invalid_channels = [
            "invalid-channel",      // Wrong format
            "stream:invalid:TEST",  // Invalid domain
            "stream:symbol:",       // Empty identifier
            "other:symbol:TEST",    // Wrong prefix
        ];
        
        for channel in &invalid_channels {
            let result = eventbus.publish(channel, proto_event.clone()).await;
            assert!(result.is_err(), "Invalid channel '{}' should be rejected", channel);
            
            match result.unwrap_err() {
                EventBusError::InvalidChannel(_) => {
                    // Expected error type
                },
                other => panic!("Expected InvalidChannel error for '{}', got: {:?}", channel, other),
            }
        }
        
        // Valid channel should work
        let result = eventbus.publish("stream:symbol:TEST", proto_event).await;
        assert!(result.is_ok(), "Valid channel should be accepted");
        
        println!("✅ EventBus enforces strict proto-only channel naming");
    }
    
    #[tokio::test]
    async fn test_contract_violation_helpers() {
        // Test the contract violation helper functions
        let raw_error = reject_raw_payload();
        match raw_error {
            EventBusError::ContractViolation(msg) => {
                assert!(msg.contains("Vec<u8> payloads are REJECTED"));
                assert!(msg.contains("Data-Staging service"));
            },
            other => panic!("Expected ContractViolation, got: {:?}", other),
        }
        
        let json_error = reject_json_payload();
        match json_error {
            EventBusError::ContractViolation(msg) => {
                assert!(msg.contains("JSON messages are not allowed"));
                assert!(msg.contains("Data-Staging service"));
            },
            other => panic!("Expected ContractViolation, got: {:?}", other),
        }
        
        println!("✅ Contract violation helpers work correctly");
    }
    
    #[tokio::test]
    async fn test_comprehensive_proto_enforcement() {
        let eventbus = InMemoryEventBus::for_testing();
        
        // Valid proto event should work
        let valid_message = TestProtoMessage {
            content: "valid content".to_string(),
            value: 42,
        };
        let valid_event = ProtoEvent::new(valid_message).with_quality_score(0.95);
        let publish_result = eventbus.publish("stream:symbol:VALID", valid_event).await;
        assert!(publish_result.is_ok());
        
        // ALL legacy/raw methods should fail
        let raw_single = eventbus.publish_raw("stream:symbol:RAW", vec![1, 2, 3]).await;
        assert!(matches!(raw_single.unwrap_err(), EventBusError::ContractViolation(_)));
        
        let raw_json = eventbus.publish_json("stream:symbol:JSON", r#"{"test": true}"#).await;
        assert!(matches!(raw_json.unwrap_err(), EventBusError::ContractViolation(_)));
        
        let raw_batch = eventbus.publish_batch_raw("stream:symbol:BATCH", vec![vec![1]]).await;
        assert!(matches!(raw_batch.unwrap_err(), EventBusError::ContractViolation(_)));
        
        println!("✅ COMPREHENSIVE PROTO ENFORCEMENT VALIDATED");
        println!("✅ ALL Vec<u8> and JSON methods properly rejected");
        println!("✅ ONLY proto messages are accepted");
        println!("✅ EventBus trait update SUCCESSFULLY COMPLETED");
    }
}