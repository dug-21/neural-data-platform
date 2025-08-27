//! Proto-only EventBus enforcement tests
//!
//! These tests verify that Phase 4 proto-only enforcement is working correctly.
//! ALL Vec<u8> and JSON payloads MUST be rejected with ContractViolation errors.

use std::collections::HashMap;

use crate::eventbus::{
    ProtoInMemoryEventBus, ProtoEventBus, ProtoEvent, ProtoEventBusConfig,
    MarketDataEvent, OrderRequest, FeatureExtractionRequest,
    EventBusError, SubscriptionConfig,
    types::StartPosition,
    
    // Legacy types that should be rejected
    InMemoryEventBus, Event, EventBus,
};

#[tokio::test]
async fn test_proto_only_enforcement_legacy_rejection() {
    let eventbus = InMemoryEventBus::new();
    
    // Create a legacy event with Vec<u8> payload (BANNED)
    let legacy_event = Event {
        event_type: "MarketData".to_string(),
        payload: vec![1, 2, 3, 4], // Vec<u8> payload - BANNED
        metadata: HashMap::new(),
        timestamp: chrono::Utc::now().timestamp(),
    };
    
    // Attempt to publish legacy event - MUST be rejected
    let result = eventbus.publish("stream:symbol:AAPL", legacy_event.clone()).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::ContractViolation(_)));
    
    // Attempt batch publish - MUST be rejected
    let result = eventbus.publish_batch("stream:symbol:AAPL", vec![legacy_event]).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::ContractViolation(_)));
}

#[tokio::test]
async fn test_proto_eventbus_accepts_only_proto_messages() {
    let eventbus = ProtoInMemoryEventBus::for_testing();
    
    // Test 1: MarketDataEvent proto message - SHOULD BE ACCEPTED
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let proto_event = ProtoEvent::new(market_data)
        .with_quality_score(0.95);
    
    let result = eventbus.publish_proto("stream:symbol:AAPL", proto_event).await;
    assert!(result.is_ok(), "Proto message should be accepted");
    
    // Test 2: OrderRequest proto message - SHOULD BE ACCEPTED
    let order = OrderRequest::new_market_buy("AAPL", 100.0);
    let proto_event = ProtoEvent::new(order)
        .with_quality_score(0.9);
    
    let result = eventbus.publish_proto("stream:action:buy", proto_event).await;
    assert!(result.is_ok(), "Proto message should be accepted");
    
    // Test 3: FeatureExtractionRequest proto message - SHOULD BE ACCEPTED
    let feature_request = FeatureExtractionRequest {
        request_id: "req-123".to_string(),
        pipeline_id: "ml-pipeline-v1".to_string(),
        source: None,
        config: None,
        window: None,
        quality: None,
    };
    let proto_event = ProtoEvent::new(feature_request);
    
    let result = eventbus.publish_proto("stream:ml:feature_extraction", proto_event).await;
    assert!(result.is_ok(), "Proto message should be accepted");
}

#[tokio::test]
async fn test_proto_eventbus_contract_violation_methods() {
    let eventbus = ProtoInMemoryEventBus::new();
    
    // Test that legacy raw methods are rejected with contract violations
    let raw_result = eventbus.publish_raw("test-channel", vec![1, 2, 3]).await;
    assert!(raw_result.is_err());
    assert!(matches!(raw_result.as_ref().unwrap_err(), EventBusError::ContractViolation(_)));
    let error_msg = raw_result.unwrap_err().to_string();
    assert!(error_msg.contains("Vec<u8> payloads are REJECTED"));
    
    let json_result = eventbus.publish_json("test-channel", "{\"test\": \"data\"}").await;
    assert!(json_result.is_err());
    assert!(matches!(json_result.as_ref().unwrap_err(), EventBusError::ContractViolation(_)));
    let error_msg = json_result.unwrap_err().to_string();
    assert!(error_msg.contains("JSON messages are not allowed"));
    
    let batch_result = eventbus.publish_batch_raw("test-channel", vec![vec![1, 2, 3]]).await;
    assert!(batch_result.is_err());
    assert!(matches!(batch_result.as_ref().unwrap_err(), EventBusError::ContractViolation(_)));
}

#[tokio::test]
async fn test_proto_message_validation_enforcement() {
    let eventbus = ProtoInMemoryEventBus::new();
    
    // Test invalid proto message - should be rejected by schema validation
    let invalid_market_data = MarketDataEvent {
        event_id: "".to_string(), // INVALID: empty event_id
        timestamp: None,          // INVALID: missing timestamp
        symbol: "".to_string(),   // INVALID: empty symbol
        data_type: 1,
        payload: None,            // INVALID: missing payload
        quality: None,
        provider: "test".to_string(),
        metadata: HashMap::new(),
    };
    
    let proto_event = ProtoEvent::new(invalid_market_data);
    let result = eventbus.publish_proto("stream:symbol:INVALID", proto_event).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
}

#[tokio::test]
async fn test_quality_score_enforcement() {
    let config = ProtoEventBusConfig::strict().min_quality_score(0.8);
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    
    // Low quality event should be rejected
    let low_quality_event = ProtoEvent::new(market_data.clone())
        .with_quality_score(0.5); // Below 0.8 threshold
    
    let result = eventbus.publish_proto("stream:symbol:AAPL", low_quality_event).await;
    assert!(result.is_err());
    assert!(matches!(result.as_ref().unwrap_err(), EventBusError::SchemaValidation(_)));
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Quality score"));
    assert!(error_msg.contains("below minimum threshold"));
    
    // High quality event should be accepted
    let high_quality_event = ProtoEvent::new(market_data)
        .with_quality_score(0.95); // Above 0.8 threshold
    
    let result = eventbus.publish_proto("stream:symbol:AAPL", high_quality_event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_channel_name_validation_proto_only() {
    let eventbus = ProtoInMemoryEventBus::new();
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let proto_event = ProtoEvent::new(market_data);
    
    // Test invalid channel names
    let invalid_channels = [
        "invalid-channel",      // Wrong format
        "stream:invalid:AAPL",  // Invalid domain
        "stream:symbol:",       // Empty identifier
        "other:symbol:AAPL",    // Wrong prefix
        "",                     // Empty channel
    ];
    
    for channel in &invalid_channels {
        let result = eventbus.publish_proto(channel, proto_event.clone()).await;
        assert!(result.is_err(), "Invalid channel '{}' should be rejected", channel);
        assert!(matches!(result.unwrap_err(), EventBusError::InvalidChannel(_)));
    }
    
    // Test valid channels
    let valid_channels = [
        "stream:symbol:AAPL",
        "stream:sector:technology",
        "stream:ml:training",
        "stream:action:buy",
        "stream:portfolio:rebalance",
        "stream:cross_sector:analysis",
        "stream:dlq:failed_messages",
    ];
    
    for channel in &valid_channels {
        let result = eventbus.publish_proto(channel, proto_event.clone()).await;
        assert!(result.is_ok(), "Valid channel '{}' should be accepted", channel);
    }
}

#[tokio::test]
async fn test_proto_type_routing_and_filtering() {
    let eventbus = ProtoInMemoryEventBus::for_testing();
    
    // Publish different types of proto messages
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let market_event = ProtoEvent::new(market_data);
    let _market_id = eventbus.publish_proto("stream:symbol:AAPL", market_event).await.unwrap();
    
    let order = OrderRequest::new_market_buy("AAPL", 100.0);
    let order_event = ProtoEvent::new(order);
    let _order_id = eventbus.publish_proto("stream:action:buy", order_event).await.unwrap();
    
    // Get channel info and verify proto type counts
    let market_info = eventbus.get_proto_channel_info("stream:symbol:AAPL").await.unwrap();
    assert_eq!(market_info.message_count, 1);
    assert!(market_info.proto_type_counts.contains_key("neural_trader.market_data.v1.MarketDataEvent"));
    
    let action_info = eventbus.get_proto_channel_info("stream:action:buy").await.unwrap();
    assert_eq!(action_info.message_count, 1);
    assert!(action_info.proto_type_counts.contains_key("neural_trader.trading.v1.OrderRequest"));
    
    // List proto types on channels
    let market_types = eventbus.list_proto_types_on_channel("stream:symbol:AAPL").await.unwrap();
    assert!(market_types.contains(&"neural_trader.market_data.v1.MarketDataEvent".to_string()));
    
    let action_types = eventbus.list_proto_types_on_channel("stream:action:buy").await.unwrap();
    assert!(action_types.contains(&"neural_trader.trading.v1.OrderRequest".to_string()));
}

#[tokio::test]
async fn test_typed_subscription_proto_only() {
    let eventbus = ProtoInMemoryEventBus::for_testing();
    
    // Create subscription configuration
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
    
    // Subscribe to MarketDataEvent specifically
    let subscriber = eventbus.subscribe_proto::<MarketDataEvent>(
        &["stream:symbol:AAPL".to_string()],
        config.clone()
    ).await;
    assert!(subscriber.is_ok(), "Proto subscription should succeed");
    
    // Subscribe to OrderRequest specifically
    let subscriber = eventbus.subscribe_proto::<OrderRequest>(
        &["stream:action:buy".to_string()],
        config.clone()
    ).await;
    assert!(subscriber.is_ok(), "Proto subscription should succeed");
    
    // Dynamic subscription to multiple proto types
    let dynamic_subscriber = eventbus.subscribe_dynamic_proto(
        &["stream:symbol:AAPL".to_string(), "stream:action:buy".to_string()],
        &["neural_trader.market_data.v1.MarketDataEvent", "neural_trader.trading.v1.OrderRequest"],
        config
    ).await;
    assert!(dynamic_subscriber.is_ok(), "Dynamic proto subscription should succeed");
}

#[tokio::test] 
async fn test_batch_proto_publishing() {
    let eventbus = ProtoInMemoryEventBus::for_testing();
    
    // Create multiple proto events
    let events = vec![
        ProtoEvent::new(MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ")),
        ProtoEvent::new(MarketDataEvent::new_trade("AAPL", 150.30, 200.0, "NASDAQ")),
        ProtoEvent::new(MarketDataEvent::new_trade("AAPL", 150.28, 50.0, "NASDAQ")),
    ];
    
    // Publish batch
    let event_ids = eventbus.publish_proto_batch("stream:symbol:AAPL", events).await;
    assert!(event_ids.is_ok());
    
    let event_ids = event_ids.unwrap();
    assert_eq!(event_ids.len(), 3);
    
    // Verify channel contains all events
    let info = eventbus.get_proto_channel_info("stream:symbol:AAPL").await.unwrap();
    assert_eq!(info.message_count, 3);
    assert_eq!(info.total_events, 3);
    
    // Verify proto type count
    let proto_count = info.proto_type_counts.get("neural_trader.market_data.v1.MarketDataEvent").unwrap();
    assert_eq!(*proto_count, 3);
}

#[tokio::test]
async fn test_proto_validation_edge_cases() {
    let eventbus = ProtoInMemoryEventBus::with_config(ProtoEventBusConfig::strict());
    
    // Test 1: Empty proto message fields
    let empty_order = OrderRequest {
        request_id: "".to_string(), // INVALID
        symbol: "".to_string(),     // INVALID
        side: 1,
        order_type: 1,
        quantity: 0.0,              // INVALID
        price: None,
        stop_price: None,
        timestamp: None,
        metadata: HashMap::new(),
    };
    let proto_event = ProtoEvent::new(empty_order);
    let result = eventbus.publish_proto("stream:action:buy", proto_event).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
    
    // Test 2: Quality score out of range
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let invalid_quality_event = ProtoEvent::new(market_data)
        .with_quality_score(1.5); // INVALID: > 1.0
    let result = eventbus.publish_proto("stream:symbol:AAPL", invalid_quality_event).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
    
    // Test 3: Timestamp too far in future
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let future_timestamp = chrono::Utc::now().timestamp() + 86400 * 2; // 2 days in future
    let future_event = ProtoEvent::new(market_data)
        .with_timestamp(future_timestamp);
    let result = eventbus.publish_proto("stream:symbol:AAPL", future_event).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
}

#[tokio::test]
async fn test_proto_payload_size_limits() {
    let config = ProtoEventBusConfig::strict().max_payload_size(100); // Very small limit
    let eventbus = ProtoInMemoryEventBus::with_config(config);
    
    // Create a large proto message that exceeds size limit
    let large_order = OrderRequest {
        request_id: "req-123".to_string(),
        symbol: "A".repeat(1000), // Very long symbol to exceed payload limit
        side: 1,
        order_type: 1,
        quantity: 100.0,
        price: Some(150.0),
        stop_price: None,
        timestamp: None,
        metadata: HashMap::new(),
    };
    
    let proto_event = ProtoEvent::new(large_order);
    let result = eventbus.publish_proto("stream:action:buy", proto_event).await;
    assert!(result.is_err());
    assert!(matches!(result.as_ref().unwrap_err(), EventBusError::SchemaValidation(_)));
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("payload size"));
    assert!(error_msg.contains("exceeds maximum"));
}

#[tokio::test]
async fn test_zero_tolerance_contract_enforcement() {
    // This test ensures ZERO tolerance for contract violations
    let eventbus = ProtoInMemoryEventBus::new();
    
    // All these attempts MUST fail with ContractViolation errors
    let test_cases = [
        ("Raw bytes", eventbus.publish_raw("test", vec![1, 2, 3])),
        ("JSON string", eventbus.publish_json("test", "{}")),
        ("Raw batch", eventbus.publish_batch_raw("test", vec![vec![1]])),
    ];
    
    for (test_name, result_future) in test_cases {
        let result = result_future.await;
        assert!(result.is_err(), "{} should be rejected", test_name);
        assert!(
            matches!(result.unwrap_err(), EventBusError::ContractViolation(_)),
            "{} should return ContractViolation error", 
            test_name
        );
    }
    
    // Only proto messages should be accepted
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let proto_event = ProtoEvent::new(market_data);
    let result = eventbus.publish_proto("stream:symbol:AAPL", proto_event).await;
    assert!(result.is_ok(), "Valid proto message should be accepted");
}

// Integration test simulating Data-Staging → EventBus → Consumer flow
#[tokio::test]
async fn test_data_staging_eventbus_consumer_flow() {
    let eventbus = ProtoInMemoryEventBus::for_testing();
    
    // Simulate Data-Staging service publishing proto messages to EventBus
    // (In real system, Data-Staging would convert JSON from Redis to proto)
    
    // Step 1: Data-Staging publishes MarketDataEvent (converted from JSON)
    let market_data = MarketDataEvent::new_trade("AAPL", 150.25, 100.0, "NASDAQ");
    let data_staging_event = ProtoEvent::new(market_data)
        .with_metadata("source".to_string(), "data-staging".to_string())
        .with_metadata("original_format".to_string(), "json".to_string())
        .with_quality_score(0.95); // Data-Staging calculates quality score
        
    let event_id = eventbus.publish_proto("stream:symbol:AAPL", data_staging_event).await;
    assert!(event_id.is_ok(), "Data-Staging should be able to publish proto events");
    
    // Step 2: ML-Ops service publishes FeatureExtractionRequest
    let feature_request = FeatureExtractionRequest {
        request_id: "ml-req-456".to_string(),
        pipeline_id: "feature-pipeline-v1".to_string(),
        source: None,
        config: None,
        window: None,
        quality: None,
    };
    let mlops_event = ProtoEvent::new(feature_request)
        .with_metadata("service".to_string(), "ml-ops".to_string())
        .with_quality_score(1.0);
        
    let event_id = eventbus.publish_proto("stream:ml:feature_extraction", mlops_event).await;
    assert!(event_id.is_ok(), "ML-Ops should be able to publish proto events");
    
    // Step 3: Trading service publishes OrderRequest
    let order = OrderRequest::new_limit_sell("AAPL", 100.0, 155.0);
    let trading_event = ProtoEvent::new(order)
        .with_metadata("service".to_string(), "trading".to_string())
        .with_quality_score(0.98);
        
    let event_id = eventbus.publish_proto("stream:action:sell", trading_event).await;
    assert!(event_id.is_ok(), "Trading service should be able to publish proto events");
    
    // Step 4: Verify all proto types are properly routed
    let market_types = eventbus.list_proto_types_on_channel("stream:symbol:AAPL").await.unwrap();
    assert!(market_types.contains(&"neural_trader.market_data.v1.MarketDataEvent".to_string()));
    
    let ml_types = eventbus.list_proto_types_on_channel("stream:ml:feature_extraction").await.unwrap();
    assert!(ml_types.contains(&"neural_trader.interfaces.mlops.FeatureExtractionRequest".to_string()));
    
    let trading_types = eventbus.list_proto_types_on_channel("stream:action:sell").await.unwrap();
    assert!(trading_types.contains(&"neural_trader.trading.v1.OrderRequest".to_string()));
    
    // Step 5: Verify EventBus rejects any non-proto attempts
    let raw_result = eventbus.publish_raw("stream:symbol:MSFT", vec![1, 2, 3]).await;
    assert!(raw_result.is_err());
    assert!(matches!(raw_result.unwrap_err(), EventBusError::ContractViolation(_)));
}