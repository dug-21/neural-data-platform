//! Integration tests for event system using ProtoEvent
//! Test actual event flow and processing with proto-only messages

use neural_core::eventbus::{
    proto_messages::*,
    types::{ProtoEvent, ProtoMessage, SubscriptionConfig, StartPosition},
    implementations::proto_inmemory::ProtoInMemoryEventBus,
    traits::ProtoEventBus,
};
use std::sync::Arc;
use futures::StreamExt;

#[tokio::test]
async fn test_event_bus_integration() {
    let bus = ProtoInMemoryEventBus::for_testing();
    
    // Create a market data event (proto-only)
    let trade_data = MarketDataEvent::new_trade("AAPL", 155.0, 100.0, "NASDAQ");
    let proto_event = ProtoEvent::new(trade_data.clone())
        .with_quality_score(0.95)
        .with_metadata("exchange".to_string(), "NASDAQ".to_string());
    
    // Publish the proto event using proper channel format
    let event_id = bus.publish_proto("stream:symbol:AAPL", proto_event).await.unwrap();
    assert!(!event_id.as_str().is_empty());
    
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
    
    // Subscribe to market data events
    let _subscriber = bus.subscribe_proto::<MarketDataEvent>(
        &["stream:symbol:AAPL".to_string()], 
        config
    ).await.unwrap();
    
    // Verify channel information
    let channel_info = bus.get_proto_channel_info("stream:symbol:AAPL").await.unwrap();
    assert_eq!(channel_info.message_count, 1);
    assert!(channel_info.proto_type_counts.contains_key("neural_trader.market_data.v1.MarketDataEvent"));
}

#[tokio::test]
async fn test_multiple_event_types() {
    let bus = ProtoInMemoryEventBus::for_testing();
    
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
    
    // Create different proto event types
    let trade_event = MarketDataEvent::new_trade("AAPL", 155.0, 100.0, "NASDAQ");
    let proto_trade = ProtoEvent::new(trade_event);
    
    let order_event = OrderRequest::new_market_buy("AAPL", 100.0);
    let proto_order = ProtoEvent::new(order_event);
    
    let config_event = ConfigChangeEvent {
        event_id: "cfg-123".to_string(),
        config_key: "model.threshold".to_string(),
        old_value: "0.5".to_string(),
        new_value: "0.7".to_string(),
        changed_by: "admin".to_string(),
        timestamp: None,
        reason: "Test update".to_string(),
    };
    let proto_config = ProtoEvent::new(config_event);
    
    // Publish all proto events to proper channels
    bus.publish_proto("stream:symbol:AAPL", proto_trade).await.unwrap();
    bus.publish_proto("stream:action:BUY", proto_order).await.unwrap();
    bus.publish_proto("stream:ml:CONFIG", proto_config).await.unwrap();
    
    // Verify we can subscribe to each channel
    let _market_handle = bus.subscribe_proto::<MarketDataEvent>(
        &["stream:symbol:AAPL".to_string()], config.clone()).await.unwrap();
    let _order_handle = bus.subscribe_proto::<OrderRequest>(
        &["stream:action:BUY".to_string()], config.clone()).await.unwrap();
    let _config_handle = bus.subscribe_proto::<ConfigChangeEvent>(
        &["stream:ml:CONFIG".to_string()], config).await.unwrap();
}

#[tokio::test]
async fn test_feature_extraction_requests() {
    let bus = ProtoInMemoryEventBus::for_testing();
    
    let config = SubscriptionConfig {
        group_name: "mlops-group".to_string(),
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
    
    let feature_request = FeatureExtractionRequest {
        request_id: "req-123".to_string(),
        pipeline_id: "lstm_v1".to_string(),
        source: Some(DataSource {
            source_type: SourceType::Stream as i32,
            topic: "market_data".to_string(),
            query: "symbol=AAPL".to_string(),
            partitions: vec!["0".to_string()],
            filters: std::collections::HashMap::new(),
        }),
        config: Some(FeatureConfig {
            feature_set_id: "technical_indicators_v1".to_string(),
            version: "1.0.0".to_string(),
        }),
        window: None,
        quality: Some(QualityRequirements {
            min_completeness: 0.95,
            max_latency_ms: 100,
            min_quality_score: 0.90,
            allow_missing: false,
            allow_outliers: true,
        }),
    };
    
    let proto_request = ProtoEvent::new(feature_request);
    
    // Test publishing ML-Ops proto events
    bus.publish_proto("stream:ml:FEATURES", proto_request).await.unwrap();
    
    let _mlops_handle = bus.subscribe_proto::<FeatureExtractionRequest>(
        &["stream:ml:FEATURES".to_string()], config).await.unwrap();
}

#[tokio::test]
async fn test_proto_event_validation() {
    // Valid proto events should pass validation
    let valid_trade = MarketDataEvent::new_trade("AAPL", 155.0, 100.0, "NASDAQ");
    let valid_proto_event = ProtoEvent::new(valid_trade).with_quality_score(0.95);
    assert!(valid_proto_event.validate().is_ok());
    
    // Invalid quality scores should fail validation
    let invalid_proto_event = ProtoEvent::new(
        MarketDataEvent::new_trade("AAPL", 155.0, 100.0, "NASDAQ")
    ).with_quality_score(1.5); // Invalid score > 1.0
    assert!(invalid_proto_event.validate().is_err());
    
    // Valid orders should pass validation
    let valid_order = OrderRequest::new_market_buy("AAPL", 100.0);
    let valid_order_event = ProtoEvent::new(valid_order);
    assert!(valid_order_event.validate().is_ok());
}

#[tokio::test]
async fn test_proto_event_serialization() {
    let trade_event = MarketDataEvent::new_trade("AAPL", 155.0, 100.0, "NASDAQ");
    let proto_event = ProtoEvent::new(trade_event.clone())
        .with_quality_score(0.95)
        .with_metadata("exchange".to_string(), "NASDAQ".to_string());
    
    // Test protobuf serialization
    let proto_bytes = proto_event.to_proto_bytes().unwrap();
    assert!(!proto_bytes.is_empty());
    
    // Verify we can deserialize the proto message back
    let deserialized = MarketDataEvent::decode(&proto_bytes[..]).unwrap();
    assert_eq!(deserialized.symbol, "AAPL");
    
    // Verify proto type name
    assert_eq!(proto_event.proto_type_name(), "neural_trader.market_data.v1.MarketDataEvent");
}

#[tokio::test]
async fn test_concurrent_proto_event_publishing() {
    let bus = Arc::new(ProtoInMemoryEventBus::for_testing());
    
    // Spawn multiple tasks publishing proto events concurrently
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let bus_clone = bus.clone();
        let handle = tokio::spawn(async move {
            let trade_event = MarketDataEvent::new_trade(
                &format!("STOCK{}", i),
                100.0 + i as f64,
                50.0,
                "NASDAQ"
            );
            let proto_event = ProtoEvent::new(trade_event)
                .with_quality_score(0.90 + (i as f64 * 0.01))
                .with_metadata("batch".to_string(), i.to_string());
            
            let channel = format!("stream:symbol:STOCK{}", i);
            bus_clone.publish_proto(&channel, proto_event).await.unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify that all channels were created with events
    for i in 0..10 {
        let channel = format!("stream:symbol:STOCK{}", i);
        let info = bus.get_proto_channel_info(&channel).await.unwrap();
        assert_eq!(info.message_count, 1);
    }
}