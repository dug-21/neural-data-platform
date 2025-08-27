//! TDD Tests for Proto Transformer Module
//! 
//! Tests the transformation of raw JSON market data to EventEnvelope protobuf messages

use data_staging::proto_transformer::*;
use data_staging::{RawMarketData, DataQualityMetrics, generated::*};
use std::collections::HashMap;
use prost_types::Timestamp;

#[tokio::test]
async fn test_market_data_to_event_envelope_transformation() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: Some(151.0),
        low: Some(149.5),
        open: Some(150.0),
        close: Some(150.25),
        vwap: Some(150.1),
        metadata: HashMap::new(),
    };
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 8,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    let result = transformer.transform_to_event_envelope(&raw_data, &quality_metrics);
    
    assert!(result.is_ok());
    
    let envelope = result.unwrap();
    
    // Verify envelope structure
    assert!(!envelope.message_id.is_empty());
    assert_eq!(envelope.source, "data-staging");
    assert_eq!(envelope.domain, "market-data");
    assert_eq!(envelope.event_type, "MarketDataEvent");
    assert!(!envelope.schema_version.is_empty());
    
    // Verify timestamps
    assert!(envelope.created_at.is_some());
    assert!(envelope.ingested_at.is_some());
    
    // Verify routing metadata
    assert!(envelope.routing.is_some());
    let routing = envelope.routing.unwrap();
    assert_eq!(routing.topic, "market_data_proto");
    assert_eq!(routing.partition_key, "AAPL");
    
    // Verify quality metadata
    assert!(envelope.quality.is_some());
    let quality = envelope.quality.unwrap();
    assert_eq!(quality.quality_score, 90.0); // 0.9 * 100
    assert_eq!(quality.completeness, 85.0);
    
    // Verify payload exists
    assert!(envelope.payload.is_some());
}

#[tokio::test]
async fn test_trade_data_payload_creation() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(1640995200), // Fixed timestamp for testing
        bid: None,
        ask: None,
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let payload = transformer.create_market_data_payload(&raw_data).unwrap();
    
    // Should create TradeData payload since we have price and volume
    assert!(payload.payload.is_some());
    
    if let Some(market_data_payload::Payload::Trade(trade)) = payload.payload {
        assert_eq!(trade.price, 150.25);
        assert_eq!(trade.size, 1000.0);
        assert_eq!(trade.exchange, "NASDAQ");
        assert_eq!(trade.sequence, 12345);
        
        // Verify timestamp conversion
        assert!(trade.timestamp.is_some());
        let ts = trade.timestamp.unwrap();
        assert_eq!(ts.seconds, 1640995200);
    } else {
        panic!("Expected TradeData payload");
    }
}

#[tokio::test]
async fn test_quote_data_payload_creation() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: None, // No price - should create QuoteData
        volume: None,
        timestamp: Some(1640995200),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let payload = transformer.create_market_data_payload(&raw_data).unwrap();
    
    // Should create QuoteData payload since we have bid/ask but no price/volume
    assert!(payload.payload.is_some());
    
    if let Some(market_data_payload::Payload::Quote(quote)) = payload.payload {
        assert_eq!(quote.bid_price, 150.20);
        assert_eq!(quote.ask_price, 150.30);
        assert_eq!(quote.exchange, "NASDAQ");
        assert_eq!(quote.sequence, 12345);
        
        // Verify timestamp conversion
        assert!(quote.timestamp.is_some());
        let ts = quote.timestamp.unwrap();
        assert_eq!(ts.seconds, 1640995200);
    } else {
        panic!("Expected QuoteData payload");
    }
}

#[tokio::test]
async fn test_bar_data_payload_creation() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: None,
        volume: Some(10000.0),
        timestamp: Some(1640995200),
        bid: None,
        ask: None,
        exchange: Some("NASDAQ".to_string()),
        sequence: None,
        high: Some(151.0),
        low: Some(149.5),
        open: Some(150.0),
        close: Some(150.25),
        vwap: Some(150.1),
        metadata: HashMap::new(),
    };
    
    let payload = transformer.create_market_data_payload(&raw_data).unwrap();
    
    // Should create BarData payload since we have OHLC data
    assert!(payload.payload.is_some());
    
    if let Some(market_data_payload::Payload::Bar(bar)) = payload.payload {
        assert_eq!(bar.open, 150.0);
        assert_eq!(bar.high, 151.0);
        assert_eq!(bar.low, 149.5);
        assert_eq!(bar.close, 150.25);
        assert_eq!(bar.volume, 10000.0);
        assert_eq!(bar.vwap, 150.1);
        assert_eq!(bar.exchange, "NASDAQ");
        
        // Verify timestamp conversion
        assert!(bar.timestamp.is_some());
        let ts = bar.timestamp.unwrap();
        assert_eq!(ts.seconds, 1640995200);
    } else {
        panic!("Expected BarData payload");
    }
}

#[tokio::test]
async fn test_minimal_data_payload_creation() {
    let transformer = ProtoTransformer::new();
    
    // Minimal data with just symbol and timestamp
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: None,
        volume: None,
        timestamp: Some(1640995200),
        bid: None,
        ask: None,
        exchange: None,
        sequence: None,
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let result = transformer.create_market_data_payload(&raw_data);
    
    // Should fail because we don't have enough data for any payload type
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("insufficient data") || error_msg.contains("payload"));
}

#[tokio::test]
async fn test_metadata_and_headers_transformation() {
    let transformer = ProtoTransformer::new();
    
    let mut metadata = HashMap::new();
    metadata.insert("source_feed".to_string(), serde_json::Value::String("polygonio".to_string()));
    metadata.insert("market_center".to_string(), serde_json::Value::String("Q".to_string()));
    metadata.insert("conditions".to_string(), serde_json::Value::Array(vec![
        serde_json::Value::String("T".to_string()),
        serde_json::Value::String("I".to_string()),
    ]));
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata,
    };
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 6,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
    
    // Verify headers were created from metadata
    assert!(!envelope.headers.is_empty());
    assert!(envelope.headers.contains_key("source_feed"));
    assert_eq!(envelope.headers.get("source_feed"), Some(&"polygonio".to_string()));
    assert!(envelope.headers.contains_key("market_center"));
    assert_eq!(envelope.headers.get("market_center"), Some(&"Q".to_string()));
    
    // Complex metadata should be serialized to JSON strings
    assert!(envelope.headers.contains_key("conditions"));
}

#[tokio::test]
async fn test_tracing_context_creation() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: None,
        ask: None,
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 4,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
    
    // Verify tracing context exists
    assert!(envelope.tracing.is_some());
    
    let tracing = envelope.tracing.unwrap();
    assert!(!tracing.trace_id.is_empty());
    assert!(!tracing.span_id.is_empty());
    
    // Should have baggage with useful information
    assert!(!tracing.baggage.is_empty());
    assert!(tracing.baggage.contains_key("symbol"));
    assert_eq!(tracing.baggage.get("symbol"), Some(&"AAPL".to_string()));
}

#[tokio::test]
async fn test_quality_metadata_conversion() {
    let transformer = ProtoTransformer::new();
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.85,
        freshness_score: 0.9,
        completeness_score: 0.8,
        validity_score: 0.95,
        missing_required_fields: 1,
        present_optional_fields: 7,
        data_age_seconds: 45,
        validation_errors: vec![
            "Minor validation warning".to_string(),
        ],
    };
    
    let quality_metadata = transformer.create_quality_metadata(&quality_metrics);
    
    // Verify score conversion (0-1 scale to 0-100 scale)
    assert_eq!(quality_metadata.quality_score, 85.0);
    assert_eq!(quality_metadata.completeness, 80.0);
    
    // Verify latency calculation (data age in milliseconds)
    assert_eq!(quality_metadata.latency_ms, 45000); // 45 seconds = 45000ms
    
    // Verify validation status
    assert_eq!(quality_metadata.validation_status, ValidationStatus::Partial as i32);
    
    // Verify anomalies created from validation errors
    assert_eq!(quality_metadata.anomalies.len(), 1);
    let anomaly = &quality_metadata.anomalies[0];
    assert_eq!(anomaly.r#type, "validation_warning");
    assert!(anomaly.description.contains("Minor validation warning"));
}

#[tokio::test]
async fn test_routing_metadata_creation() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("MSFT".to_string()),
        price: Some(300.50),
        volume: Some(2000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(300.45),
        ask: Some(300.55),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(67890),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let routing = transformer.create_routing_metadata(&raw_data);
    
    assert_eq!(routing.topic, "market_data_proto");
    assert_eq!(routing.partition_key, "MSFT");
    assert_eq!(routing.priority, 1); // High priority for market data
    assert!(routing.ttl_seconds > 0);
    
    // Should have appropriate tags
    assert!(!routing.tags.is_empty());
    assert!(routing.tags.contains(&"market-data".to_string()));
    assert!(routing.tags.contains(&"real-time".to_string()));
    
    // Should have retry policy
    assert!(routing.retry_policy.is_some());
    let retry_policy = routing.retry_policy.unwrap();
    assert!(retry_policy.max_attempts > 0);
    assert!(retry_policy.initial_delay_ms > 0);
}

#[tokio::test]
async fn test_error_handling_invalid_timestamp() {
    let transformer = ProtoTransformer::new();
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(-1), // Invalid negative timestamp
        bid: None,
        ask: None,
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 4,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    let result = transformer.transform_to_event_envelope(&raw_data, &quality_metrics);
    
    // Should handle invalid timestamp gracefully (might use current time or fail)
    match result {
        Ok(envelope) => {
            // If it succeeds, should have valid timestamps
            assert!(envelope.created_at.is_some());
            assert!(envelope.ingested_at.is_some());
        }
        Err(e) => {
            // If it fails, should be due to timestamp issue
            assert!(e.to_string().contains("timestamp") || e.to_string().contains("time"));
        }
    }
}

#[tokio::test]
async fn test_large_metadata_handling() {
    let transformer = ProtoTransformer::new();
    
    // Create large metadata to test size limits
    let mut metadata = HashMap::new();
    for i in 0..1000 {
        metadata.insert(
            format!("key_{}", i),
            serde_json::Value::String(format!("value_{}_with_some_additional_content", i))
        );
    }
    
    let raw_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: None,
        ask: None,
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata,
    };
    
    let quality_metrics = DataQualityMetrics {
        overall_score: 0.9,
        freshness_score: 0.95,
        completeness_score: 0.85,
        validity_score: 1.0,
        missing_required_fields: 0,
        present_optional_fields: 4,
        data_age_seconds: 10,
        validation_errors: vec![],
    };
    
    let result = transformer.transform_to_event_envelope(&raw_data, &quality_metrics);
    
    match result {
        Ok(envelope) => {
            // Headers should be present but might be truncated for size
            assert!(!envelope.headers.is_empty());
        }
        Err(e) => {
            // Might fail due to size limits
            assert!(e.to_string().contains("size") || e.to_string().contains("large"));
        }
    }
}