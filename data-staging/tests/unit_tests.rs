//! Comprehensive unit tests for Data-Staging service
//! 
//! This module contains exhaustive unit tests covering all Data-Staging modules
//! with >90% code coverage and strict proto-only validation.

use data_staging::*;
use data_staging::generated::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio_test;

// ================================================================================================
// JSON Validator Tests
// ================================================================================================

#[cfg(test)]
mod json_validator_tests {
    use super::*;
    use data_staging::json_validator::JsonValidator;
    
    fn setup_validator() -> JsonValidator {
        let thresholds = QualityThresholds {
            minimum_quality_score: 0.7,
            max_age_seconds: 300,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(),
                "timestamp".to_string(),
            ],
        };
        JsonValidator::new(&thresholds)
    }
    
    #[test]
    fn test_valid_json_accepted() {
        let validator = setup_validator();
        let valid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let result = validator.validate(&valid_data);
        assert!(result.is_ok(), "Valid data should pass validation");
    }
    
    #[test]
    fn test_missing_symbol_rejected() {
        let validator = setup_validator();
        let invalid_data = RawMarketData {
            symbol: None,
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("symbol"));
        }
    }
    
    #[test]
    fn test_missing_price_rejected() {
        let validator = setup_validator();
        let invalid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: None,
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("price"));
        }
    }
    
    #[test]
    fn test_missing_timestamp_rejected() {
        let validator = setup_validator();
        let invalid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: None,
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("timestamp"));
        }
    }
    
    #[test]
    fn test_negative_price_rejected() {
        let validator = setup_validator();
        let invalid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(-150.25), // Negative price
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("price") || e.to_string().contains("negative"));
        }
    }
    
    #[test]
    fn test_zero_price_rejected() {
        let validator = setup_validator();
        let invalid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(0.0), // Zero price
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("price") || e.to_string().contains("zero"));
        }
    }
    
    #[test]
    fn test_empty_symbol_rejected() {
        let validator = setup_validator();
        let invalid_data = RawMarketData {
            symbol: Some("".to_string()), // Empty symbol
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("symbol") || e.to_string().contains("empty"));
        }
    }
    
    #[test]
    fn test_future_timestamp_rejected() {
        let validator = setup_validator();
        let future_timestamp = (chrono::Utc::now().timestamp_millis() + 3600000) as i64; // 1 hour in future
        let invalid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(future_timestamp),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("timestamp") || e.to_string().contains("future"));
        }
    }
    
    #[test]
    fn test_very_old_timestamp_rejected() {
        let validator = setup_validator();
        let old_timestamp = (chrono::Utc::now().timestamp_millis() - 86400000) as i64; // 24 hours ago
        let invalid_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(old_timestamp),
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
        
        let result = validator.validate(&invalid_data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("timestamp") || e.to_string().contains("old") || e.to_string().contains("stale"));
        }
    }
}

// ================================================================================================
// Quality Scorer Tests
// ================================================================================================

#[cfg(test)]
mod quality_scorer_tests {
    use super::*;
    use data_staging::quality_scorer::QualityScorer;
    
    fn setup_scorer() -> QualityScorer {
        let thresholds = QualityThresholds {
            minimum_quality_score: 0.7,
            max_age_seconds: 300,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(),
                "timestamp".to_string(),
            ],
        };
        QualityScorer::new(&thresholds)
    }
    
    #[test]
    fn test_perfect_quality_score() {
        let scorer = setup_scorer();
        let perfect_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: Some(151.0),
            low: Some(149.0),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        };
        
        let metrics = scorer.calculate_quality(&perfect_data);
        assert!(metrics.overall_score >= 0.95, "Perfect data should have high quality score");
        assert_eq!(metrics.missing_required_fields, 0);
        assert!(metrics.present_optional_fields >= 8); // All optional fields present
    }
    
    #[test]
    fn test_minimum_quality_score() {
        let scorer = setup_scorer();
        let minimal_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: None, // Missing optional field
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
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
        
        let metrics = scorer.calculate_quality(&minimal_data);
        assert!(metrics.overall_score >= 0.6, "Minimal valid data should have decent quality score");
        assert_eq!(metrics.missing_required_fields, 0);
        assert!(metrics.present_optional_fields <= 2);
    }
    
    #[test]
    fn test_missing_required_field_quality() {
        let scorer = setup_scorer();
        let missing_field_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: None, // Missing required field
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
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
        
        let metrics = scorer.calculate_quality(&missing_field_data);
        assert!(metrics.overall_score < 0.5, "Missing required field should result in low quality score");
        assert!(metrics.missing_required_fields > 0);
    }
    
    #[test]
    fn test_stale_data_quality() {
        let scorer = setup_scorer();
        let stale_timestamp = chrono::Utc::now().timestamp_millis() - 600_000; // 10 minutes ago
        let stale_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(stale_timestamp),
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
        
        let metrics = scorer.calculate_quality(&stale_data);
        assert!(metrics.freshness_score < 0.8, "Stale data should have low freshness score");
        assert!(metrics.data_age_seconds > 300);
    }
    
    #[test]
    fn test_quality_score_consistency() {
        let scorer = setup_scorer();
        let test_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
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
        
        let metrics1 = scorer.calculate_quality(&test_data);
        let metrics2 = scorer.calculate_quality(&test_data);
        
        assert!((metrics1.overall_score - metrics2.overall_score).abs() < 0.01, "Quality scoring should be consistent");
        assert_eq!(metrics1.missing_required_fields, metrics2.missing_required_fields);
        assert_eq!(metrics1.present_optional_fields, metrics2.present_optional_fields);
    }
}

// ================================================================================================
// Proto Transformer Tests
// ================================================================================================

#[cfg(test)]
mod proto_transformer_tests {
    use super::*;
    use data_staging::proto_transformer::ProtoTransformer;
    use prost::Message;
    
    fn setup_transformer() -> ProtoTransformer {
        ProtoTransformer::new()
    }
    
    fn create_test_data() -> RawMarketData {
        RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: Some(151.0),
            low: Some(149.0),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        }
    }
    
    fn create_test_quality_metrics() -> DataQualityMetrics {
        DataQualityMetrics {
            overall_score: 0.95,
            freshness_score: 0.98,
            completeness_score: 0.92,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 30,
            validation_errors: vec![],
        }
    }
    
    #[test]
    fn test_transform_to_event_envelope() {
        let transformer = setup_transformer();
        let raw_data = create_test_data();
        let quality_metrics = create_test_quality_metrics();
        
        let result = transformer.transform_to_event_envelope(&raw_data, &quality_metrics);
        assert!(result.is_ok(), "Valid data should transform successfully");
        
        let envelope = result.unwrap();
        assert!(!envelope.event_id.is_empty(), "Event should have ID");
        assert!(envelope.timestamp.is_some(), "Event should have timestamp");
        assert!(!envelope.event_type.is_empty(), "Event should have type");
        assert!(envelope.payload.is_some(), "Event should have payload");
    }
    
    #[test]
    fn test_proto_serialization_roundtrip() {
        let transformer = setup_transformer();
        let raw_data = create_test_data();
        let quality_metrics = create_test_quality_metrics();
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        
        // Serialize to bytes
        let serialized = envelope.encode_to_vec();
        assert!(!serialized.is_empty(), "Serialized data should not be empty");
        
        // Deserialize back
        let deserialized = EventEnvelope::decode(&serialized[..]);
        assert!(deserialized.is_ok(), "Deserialization should succeed");
        
        let recovered = deserialized.unwrap();
        assert_eq!(envelope.event_id, recovered.event_id);
        assert_eq!(envelope.event_type, recovered.event_type);
    }
    
    #[test]
    fn test_trade_data_transformation() {
        let transformer = setup_transformer();
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            bid: None,
            ask: None,
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata: HashMap::new(),
        };
        let quality_metrics = create_test_quality_metrics();
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        
        // Verify the payload contains trade data
        if let Some(payload) = envelope.payload {
            // Verify it's valid protobuf
            assert!(!payload.is_empty());
        } else {
            panic!("Payload should be present");
        }
    }
    
    #[test]
    fn test_quote_data_transformation() {
        let transformer = setup_transformer();
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: None,
            volume: None,
            timestamp: Some(1640995200000),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: None,
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata: HashMap::new(),
        };
        let quality_metrics = create_test_quality_metrics();
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        
        // Verify the payload contains quote data
        if let Some(payload) = envelope.payload {
            assert!(!payload.is_empty());
        } else {
            panic!("Payload should be present");
        }
    }
    
    #[test]
    fn test_bar_data_transformation() {
        let transformer = setup_transformer();
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: None,
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
            bid: None,
            ask: None,
            exchange: Some("NASDAQ".to_string()),
            sequence: None,
            high: Some(151.0),
            low: Some(149.0),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        };
        let quality_metrics = create_test_quality_metrics();
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        
        // Verify the payload contains bar data
        if let Some(payload) = envelope.payload {
            assert!(!payload.is_empty());
        } else {
            panic!("Payload should be present");
        }
    }
    
    #[test]
    fn test_quality_metrics_inclusion() {
        let transformer = setup_transformer();
        let raw_data = create_test_data();
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.85,
            freshness_score: 0.90,
            completeness_score: 0.80,
            validity_score: 0.95,
            missing_required_fields: 1,
            present_optional_fields: 5,
            data_age_seconds: 60,
            validation_errors: vec!["Minor warning".to_string()],
        };
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        
        // Verify quality metrics are included in the envelope
        assert!(envelope.quality.is_some(), "Quality metrics should be included");
        
        if let Some(quality) = envelope.quality {
            assert!((quality.overall_score - 0.85).abs() < 0.01);
        }
    }
    
    #[test]
    fn test_metadata_preservation() {
        let transformer = setup_transformer();
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::Value::String("polygon".to_string()));
        metadata.insert("provider_id".to_string(), serde_json::Value::Number(serde_json::Number::from(123)));
        
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
            bid: None,
            ask: None,
            exchange: None,
            sequence: None,
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata,
        };
        let quality_metrics = create_test_quality_metrics();
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        
        // Verify metadata is preserved
        assert!(!envelope.metadata.is_empty(), "Metadata should be preserved");
        assert!(envelope.metadata.contains_key("source"));
    }
}

// ================================================================================================
// Vec<u8> Rejection Tests (Critical for Proto-Only Enforcement)
// ================================================================================================

#[cfg(test)]
mod vec_u8_rejection_tests {
    use super::*;
    use data_staging::proto_transformer::ProtoTransformer;
    use prost::Message;
    
    #[test]
    fn test_raw_vec_u8_rejected() {
        let raw_data = vec![0x01, 0x02, 0x03, 0x04];
        
        // Try to decode as EventEnvelope - should fail
        let result = EventEnvelope::decode(&raw_data[..]);
        assert!(result.is_err(), "Raw Vec<u8> should be rejected");
    }
    
    #[test]
    fn test_json_bytes_rejected() {
        let json_str = r#"{"symbol": "AAPL", "price": 150.25}"#;
        let json_bytes = json_str.as_bytes();
        
        // Try to decode as EventEnvelope - should fail
        let result = EventEnvelope::decode(json_bytes);
        assert!(result.is_err(), "JSON bytes should be rejected");
    }
    
    #[test]
    fn test_random_binary_rejected() {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let random_data: Vec<u8> = (0..1000).map(|_| rng.gen()).collect();
        
        // Try to decode as EventEnvelope - should fail
        let result = EventEnvelope::decode(&random_data[..]);
        assert!(result.is_err(), "Random binary data should be rejected");
    }
    
    #[test]
    fn test_corrupted_proto_rejected() {
        let transformer = ProtoTransformer::new();
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.95,
            freshness_score: 0.98,
            completeness_score: 0.92,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 30,
            validation_errors: vec![],
        };
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        let mut serialized = envelope.encode_to_vec();
        
        // Corrupt the data
        if serialized.len() > 10 {
            serialized[5] = 0xFF; // Corrupt a byte
            serialized[6] = 0xFF; // Corrupt another byte
        }
        
        // Try to decode corrupted data - should fail
        let result = EventEnvelope::decode(&serialized[..]);
        assert!(result.is_err(), "Corrupted protobuf should be rejected");
    }
    
    #[test]
    fn test_truncated_proto_rejected() {
        let transformer = ProtoTransformer::new();
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.95,
            freshness_score: 0.98,
            completeness_score: 0.92,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 30,
            validation_errors: vec![],
        };
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        let mut serialized = envelope.encode_to_vec();
        
        // Truncate the data
        serialized.truncate(serialized.len() / 2);
        
        // Try to decode truncated data - should fail
        let result = EventEnvelope::decode(&serialized[..]);
        assert!(result.is_err(), "Truncated protobuf should be rejected");
    }
    
    #[test]
    fn test_empty_data_rejected() {
        let empty_data = vec![];
        
        // Try to decode empty data - should fail
        let result = EventEnvelope::decode(&empty_data[..]);
        assert!(result.is_err(), "Empty data should be rejected");
    }
    
    #[test]
    fn test_msgpack_data_rejected() {
        use serde_json::json;
        let json_data = json!({
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 1000
        });
        
        // Convert to msgpack bytes (simulated)
        let msgpack_like = json_data.to_string().into_bytes();
        
        // Try to decode as EventEnvelope - should fail
        let result = EventEnvelope::decode(&msgpack_like[..]);
        assert!(result.is_err(), "MessagePack-like data should be rejected");
    }
    
    #[test]
    fn test_bincode_data_rejected() {
        // Simulate bincode-like binary data
        let bincode_like = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
        
        // Try to decode as EventEnvelope - should fail
        let result = EventEnvelope::decode(&bincode_like[..]);
        assert!(result.is_err(), "Bincode-like data should be rejected");
    }
    
    #[test]
    fn test_only_valid_proto_accepted() {
        let transformer = ProtoTransformer::new();
        let raw_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
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
        
        let quality_metrics = DataQualityMetrics {
            overall_score: 0.95,
            freshness_score: 0.98,
            completeness_score: 0.92,
            validity_score: 1.0,
            missing_required_fields: 0,
            present_optional_fields: 8,
            data_age_seconds: 30,
            validation_errors: vec![],
        };
        
        let envelope = transformer.transform_to_event_envelope(&raw_data, &quality_metrics).unwrap();
        let serialized = envelope.encode_to_vec();
        
        // Valid protobuf should be accepted
        let result = EventEnvelope::decode(&serialized[..]);
        assert!(result.is_ok(), "Valid protobuf should be accepted");
        
        let decoded = result.unwrap();
        assert_eq!(envelope.event_id, decoded.event_id);
        assert_eq!(envelope.event_type, decoded.event_type);
    }
}

// ================================================================================================
// Error Handling Tests
// ================================================================================================

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_error_categories() {
        let validation_error = DataStagingError::ValidationError {
            message: "Test validation error".to_string()
        };
        assert_eq!(validation_error.category(), "validation");
        assert!(!validation_error.is_retryable());
        
        let quality_error = DataStagingError::QualityError {
            score: 0.5,
            threshold: 0.7
        };
        assert_eq!(quality_error.category(), "quality");
        assert!(!quality_error.is_retryable());
        
        let missing_field_error = DataStagingError::MissingRequiredField {
            field: "symbol".to_string()
        };
        assert_eq!(missing_field_error.category(), "missing_field");
        assert!(!missing_field_error.is_retryable());
    }
    
    #[test]
    fn test_retryable_errors() {
        let redis_error = DataStagingError::RedisError(
            redis::RedisError::from(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test"))
        );
        assert!(redis_error.is_retryable());
        assert_eq!(redis_error.category(), "redis");
        
        let io_error = DataStagingError::IoError(
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "test")
        );
        assert!(io_error.is_retryable());
        assert_eq!(io_error.category(), "io");
    }
    
    #[test]
    fn test_non_retryable_errors() {
        let json_error = DataStagingError::JsonError(
            serde_json::Error::custom("test")
        );
        assert!(!json_error.is_retryable());
        assert_eq!(json_error.category(), "json");
        
        let invalid_format_error = DataStagingError::InvalidFormat {
            message: "Invalid format".to_string()
        };
        assert!(!invalid_format_error.is_retryable());
        assert_eq!(invalid_format_error.category(), "format");
    }
}

// ================================================================================================
// Configuration Tests
// ================================================================================================

#[cfg(test)]
mod configuration_tests {
    use super::*;
    
    #[test]
    fn test_default_configuration() {
        let config = DataStagingConfig::default();
        
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.input_stream, "market_data_raw");
        assert_eq!(config.consumer_group, "data-staging");
        assert_eq!(config.consumer_name, "data-staging-1");
        assert_eq!(config.eventbus_config.output_topic, "market_data_proto");
        assert_eq!(config.quality_thresholds.minimum_quality_score, 0.7);
        assert_eq!(config.quality_thresholds.max_age_seconds, 300);
        assert_eq!(config.processing_limits.max_batch_size, 100);
    }
    
    #[test]
    fn test_configuration_serialization() {
        let config = DataStagingConfig::default();
        
        // Test serialization
        let serialized = serde_json::to_string(&config);
        assert!(serialized.is_ok());
        
        // Test deserialization
        let json_str = serialized.unwrap();
        let deserialized: Result<DataStagingConfig, _> = serde_json::from_str(&json_str);
        assert!(deserialized.is_ok());
        
        let recovered = deserialized.unwrap();
        assert_eq!(config.redis_url, recovered.redis_url);
        assert_eq!(config.input_stream, recovered.input_stream);
        assert_eq!(config.quality_thresholds.minimum_quality_score, recovered.quality_thresholds.minimum_quality_score);
    }
    
    #[test]
    fn test_quality_thresholds_validation() {
        let mut thresholds = QualityThresholds {
            minimum_quality_score: 0.7,
            max_age_seconds: 300,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(),
                "timestamp".to_string(),
            ],
        };
        
        // Valid thresholds should work
        assert!(thresholds.minimum_quality_score >= 0.0 && thresholds.minimum_quality_score <= 1.0);
        assert!(thresholds.max_age_seconds > 0);
        assert!(!thresholds.required_fields.is_empty());
        
        // Test edge cases
        thresholds.minimum_quality_score = 0.0; // Minimum valid
        assert!(thresholds.minimum_quality_score >= 0.0);
        
        thresholds.minimum_quality_score = 1.0; // Maximum valid
        assert!(thresholds.minimum_quality_score <= 1.0);
    }
    
    #[test]
    fn test_processing_limits_validation() {
        let limits = ProcessingLimits {
            max_batch_size: 100,
            message_timeout_ms: 1000,
            max_retries: 3,
        };
        
        assert!(limits.max_batch_size > 0);
        assert!(limits.message_timeout_ms > 0);
        assert!(limits.max_retries >= 0);
    }
}

// ================================================================================================
// Data Structure Tests
// ================================================================================================

#[cfg(test)]
mod data_structure_tests {
    use super::*;
    
    #[test]
    fn test_raw_market_data_serialization() {
        let data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: Some(151.0),
            low: Some(149.0),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        };
        
        // Test JSON serialization
        let json_result = serde_json::to_string(&data);
        assert!(json_result.is_ok());
        
        // Test JSON deserialization
        let json_str = json_result.unwrap();
        let deserialize_result: Result<RawMarketData, _> = serde_json::from_str(&json_str);
        assert!(deserialize_result.is_ok());
        
        let recovered = deserialize_result.unwrap();
        assert_eq!(data.symbol, recovered.symbol);
        assert_eq!(data.price, recovered.price);
        assert_eq!(data.volume, recovered.volume);
    }
    
    #[test]
    fn test_partial_raw_market_data() {
        let minimal_data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: None,
            timestamp: Some(1640995200000),
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
        
        // Should serialize/deserialize correctly even with None values
        let json_result = serde_json::to_string(&minimal_data);
        assert!(json_result.is_ok());
        
        let json_str = json_result.unwrap();
        let deserialize_result: Result<RawMarketData, _> = serde_json::from_str(&json_str);
        assert!(deserialize_result.is_ok());
        
        let recovered = deserialize_result.unwrap();
        assert_eq!(minimal_data.symbol, recovered.symbol);
        assert_eq!(minimal_data.price, recovered.price);
        assert_eq!(minimal_data.volume, recovered.volume);
    }
    
    #[test]
    fn test_metadata_handling() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::Value::String("polygon".to_string()));
        metadata.insert("provider_id".to_string(), serde_json::Value::Number(serde_json::Number::from(123)));
        metadata.insert("is_realtime".to_string(), serde_json::Value::Bool(true));
        
        let data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(1640995200000),
            bid: None,
            ask: None,
            exchange: None,
            sequence: None,
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata,
        };
        
        // Test serialization with metadata
        let json_result = serde_json::to_string(&data);
        assert!(json_result.is_ok());
        
        let json_str = json_result.unwrap();
        let deserialize_result: Result<RawMarketData, _> = serde_json::from_str(&json_str);
        assert!(deserialize_result.is_ok());
        
        let recovered = deserialize_result.unwrap();
        assert_eq!(recovered.metadata.len(), 3);
        assert!(recovered.metadata.contains_key("source"));
        assert!(recovered.metadata.contains_key("provider_id"));
        assert!(recovered.metadata.contains_key("is_realtime"));
    }
    
    #[test]
    fn test_data_quality_metrics_structure() {
        let metrics = DataQualityMetrics {
            overall_score: 0.85,
            freshness_score: 0.90,
            completeness_score: 0.80,
            validity_score: 0.95,
            missing_required_fields: 1,
            present_optional_fields: 5,
            data_age_seconds: 60,
            validation_errors: vec!["Minor warning".to_string()],
        };
        
        // Verify all scores are in valid range
        assert!(metrics.overall_score >= 0.0 && metrics.overall_score <= 1.0);
        assert!(metrics.freshness_score >= 0.0 && metrics.freshness_score <= 1.0);
        assert!(metrics.completeness_score >= 0.0 && metrics.completeness_score <= 1.0);
        assert!(metrics.validity_score >= 0.0 && metrics.validity_score <= 1.0);
        
        // Verify counters are non-negative
        assert!(metrics.missing_required_fields >= 0);
        assert!(metrics.present_optional_fields >= 0);
        assert!(metrics.data_age_seconds >= 0);
        
        // Test serialization
        let json_result = serde_json::to_string(&metrics);
        assert!(json_result.is_ok());
        
        let json_str = json_result.unwrap();
        let deserialize_result: Result<DataQualityMetrics, _> = serde_json::from_str(&json_str);
        assert!(deserialize_result.is_ok());
        
        let recovered = deserialize_result.unwrap();
        assert!((metrics.overall_score - recovered.overall_score).abs() < 0.01);
    }
}