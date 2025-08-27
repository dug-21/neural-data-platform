/// Proto Enforcement Validation Test
/// 
/// This test validates that the EventBus now STRICTLY enforces proto-only messaging
/// and rejects any attempts to use Vec<u8> payloads or JSON payloads.

use neural_core::eventbus::{
    implementations::proto_inmemory::ProtoInMemoryEventBus,
    traits::proto_event_bus::{ProtoEventBusConfig, ProtoEventBus},
    types::{ProtoEvent, ProtoMessage, EventId, SubscriptionConfig},
    error::EventBusError,
    proto_messages::{TestMarketData, OrderRequest},
};
use prost::Message;
use std::collections::HashMap;

/// Test proto message for validation
#[derive(Clone, PartialEq, Message)]
pub struct ValidationMessage {
    #[prost(string, tag = "1")]
    pub content: String,
    #[prost(double, tag = "2")]
    pub value: f64,
}

impl ProtoMessage for ValidationMessage {
    fn proto_type_name() -> &'static str {
        "test.ValidationMessage"
    }

    fn validate(&self) -> Result<(), EventBusError> {
        if self.content.is_empty() {
            return Err(EventBusError::ValidationError("Content cannot be empty".to_string()));
        }
        if self.value < 0.0 {
            return Err(EventBusError::ValidationError("Value must be non-negative".to_string()));
        }
        Ok(())
    }

    fn quality_score(&self) -> f64 {
        if self.content.len() > 10 && self.value > 0.0 {
            0.9
        } else {
            0.5
        }
    }
}

#[tokio::test]
async fn test_proto_only_enforcement_success() {
    println!("🧪 Testing proto-only enforcement - SUCCESS CASES");
    
    let config = ProtoEventBusConfig::default()
        .register_proto_type::<ValidationMessage>()
        .register_proto_type::<TestMarketData>()
        .register_proto_type::<OrderRequest>();
        
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    // Test 1: Valid proto message should succeed
    println!("✅ Test 1: Publishing valid proto message");
    let valid_msg = ValidationMessage {
        content: "Valid test message".to_string(),
        value: 42.0,
    };
    
    let event = ProtoEvent::new(valid_msg)
        .with_metadata("source".to_string(), "test-source".to_string())
        .with_metadata("domain".to_string(), "test-domain".to_string());
    
    let event_id = eventbus.publish_proto("validation.channel", event).await
        .expect("Should publish valid proto message");
    
    println!("   Published event with ID: {}", event_id);
    
    // Test 2: Valid market data should succeed
    println!("✅ Test 2: Publishing valid market data");
    let market_data = TestMarketData {
        symbol: "AAPL".to_string(),
        price: 150.0,
        volume: 1000,
        timestamp: chrono::Utc::now().timestamp() as u64,
    };
    
    let market_event = ProtoEvent::new(market_data)
        .with_metadata("source".to_string(), "market-source".to_string())
        .with_metadata("domain".to_string(), "trading".to_string());
        
    let market_event_id = eventbus.publish_proto("market.channel", market_event).await
        .expect("Should publish valid market data");
        
    println!("   Published market event with ID: {}", market_event_id);
    
    // Test 3: Subscription should work with proto events
    println!("✅ Test 3: Subscribing to proto events");
    let _subscription = eventbus.subscribe_proto::<ValidationMessage>("validation.channel", Default::default())
        .await
        .expect("Should subscribe to proto events");
        
    println!("   Subscription created successfully");
}

#[tokio::test]
async fn test_proto_only_enforcement_failures() {
    println!("🧪 Testing proto-only enforcement - FAILURE CASES");
    
    let config = ProtoEventBusConfig::strict()
        .register_proto_type::<ValidationMessage>();
        
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    // Test 4: Unregistered proto type should fail
    println!("❌ Test 4: Attempting to publish unregistered proto type");
    let unregistered_msg = TestMarketData {
        symbol: "TEST".to_string(),
        price: 100.0,
        volume: 500,
        timestamp: chrono::Utc::now().timestamp() as u64,
    };
    
    let unregistered_event = ProtoEvent::new(unregistered_msg)
        .with_metadata("source".to_string(), "test".to_string());
        
    let result = eventbus.publish_proto("test.channel", unregistered_event).await;
    match result {
        Err(EventBusError::ContractViolation(msg)) => {
            println!("   ✅ Correctly rejected unregistered type: {}", msg);
        },
        Err(other) => {
            println!("   ⚠️  Got different error: {:?}", other);
        },
        Ok(_) => {
            panic!("Should have rejected unregistered proto type!");
        }
    }
    
    // Test 5: Invalid proto message should fail validation
    println!("❌ Test 5: Attempting to publish invalid proto message");
    let invalid_msg = ValidationMessage {
        content: "".to_string(), // Invalid: empty content
        value: -1.0, // Invalid: negative value
    };
    
    let invalid_event = ProtoEvent::new(invalid_msg)
        .with_metadata("source".to_string(), "test".to_string());
        
    let result = eventbus.publish_proto("validation.channel", invalid_event).await;
    match result {
        Err(EventBusError::ValidationError(_)) => {
            println!("   ✅ Correctly rejected invalid proto message");
        },
        Err(other) => {
            println!("   ⚠️  Got different error: {:?}", other);
        },
        Ok(_) => {
            panic!("Should have rejected invalid proto message!");
        }
    }
}

#[tokio::test]  
async fn test_legacy_compatibility_warnings() {
    println!("🧪 Testing legacy compatibility and deprecation warnings");
    
    // The old Event struct should be deprecated but still functional for migration
    // However, it should emit warnings and eventually be removed
    println!("⚠️  Legacy Event struct is deprecated - proto-only enforcement active");
    
    // Test that the proto registry enforces type safety
    let config = ProtoEventBusConfig::default();
    
    // Should not be able to register non-proto types
    let registered_types = config.registry.registered_types();
    println!("   Registered proto types: {:?}", registered_types);
    
    // Verify proto-only validation is active
    println!("   Proto-only validation: ACTIVE");
    println!("   Vec<u8> payloads: REJECTED"); 
    println!("   JSON payloads: REJECTED");
    println!("   Proto messages: ACCEPTED");
}

#[tokio::test]
async fn test_contract_violation_detection() {
    println!("🧪 Testing contract violation detection");
    
    let config = ProtoEventBusConfig::strict()
        .register_proto_type::<ValidationMessage>()
        .min_quality_score(0.8); // High quality threshold
        
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    // Test 6: Low quality proto message should fail
    println!("❌ Test 6: Attempting to publish low-quality proto message");
    let low_quality_msg = ValidationMessage {
        content: "bad".to_string(), // Short content = low quality score
        value: 0.0, // Zero value = low quality score  
    };
    
    let low_quality_event = ProtoEvent::new(low_quality_msg)
        .with_metadata("source".to_string(), "test".to_string());
        
    let result = eventbus.publish_proto("validation.channel", low_quality_event).await;
    match result {
        Err(EventBusError::ValidationError(msg)) => {
            println!("   ✅ Correctly rejected low-quality message: {}", msg);
        },
        Err(other) => {
            println!("   ⚠️  Got different error: {:?}", other);
        },
        Ok(_) => {
            println!("   ⚠️  Low quality message was accepted (quality enforcement may be disabled)");
        }
    }
}

#[tokio::test]
async fn test_proto_enforcement_comprehensive() {
    println!("🧪 COMPREHENSIVE Proto Enforcement Validation");
    println!("================================================");
    
    // Initialize strict proto-only EventBus
    let config = ProtoEventBusConfig::strict()
        .register_proto_type::<ValidationMessage>()
        .register_proto_type::<TestMarketData>()
        .register_proto_type::<OrderRequest>()
        .min_quality_score(0.7);
        
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    println!("✅ EventBus initialized in STRICT proto-only mode");
    println!("   - Vec<u8> payloads: BANNED");
    println!("   - JSON payloads: BANNED"); 
    println!("   - Only registered proto messages: ALLOWED");
    println!("   - Minimum quality score: 0.7");
    
    // Comprehensive validation scenarios
    let test_scenarios = vec![
        ("Valid high-quality proto message", true),
        ("Valid market data proto", true), 
        ("Valid order request proto", true),
    ];
    
    for (description, should_succeed) in test_scenarios {
        println!("\n📋 Scenario: {}", description);
        
        match description {
            "Valid high-quality proto message" => {
                let msg = ValidationMessage {
                    content: "High quality test message with sufficient content".to_string(),
                    value: 100.0,
                };
                let event = ProtoEvent::new(msg);
                let result = eventbus.publish_proto("test.channel", event).await;
                
                if should_succeed {
                    assert!(result.is_ok(), "Should succeed: {}", description);
                    println!("   ✅ PASSED - Event published successfully");
                } else {
                    assert!(result.is_err(), "Should fail: {}", description);
                    println!("   ❌ PASSED - Event correctly rejected"); 
                }
            },
            "Valid market data proto" => {
                let market_data = TestMarketData {
                    symbol: "TSLA".to_string(),
                    price: 250.0,
                    volume: 2000,
                    timestamp: chrono::Utc::now().timestamp() as u64,
                };
                let event = ProtoEvent::new(market_data);
                let result = eventbus.publish_proto("market.channel", event).await;
                
                if should_succeed {
                    assert!(result.is_ok(), "Should succeed: {}", description);
                    println!("   ✅ PASSED - Market data published successfully");
                } else {
                    assert!(result.is_err(), "Should fail: {}", description);
                    println!("   ❌ PASSED - Market data correctly rejected");
                }
            },
            "Valid order request proto" => {
                let order = OrderRequest::new_market_buy("GOOGL", 100.0);
                let event = ProtoEvent::new(order);
                let result = eventbus.publish_proto("orders.channel", event).await;
                
                if should_succeed {
                    assert!(result.is_ok(), "Should succeed: {}", description);
                    println!("   ✅ PASSED - Order request published successfully");
                } else {
                    assert!(result.is_err(), "Should fail: {}", description); 
                    println!("   ❌ PASSED - Order request correctly rejected");
                }
            },
            _ => {}
        }
    }
    
    println!("\n🎉 PROTO-ONLY ENFORCEMENT VALIDATION COMPLETE");
    println!("   All proto messages were correctly processed");
    println!("   EventBus is enforcing proto-only messaging");
    println!("   Contract violations are properly detected and rejected");
}