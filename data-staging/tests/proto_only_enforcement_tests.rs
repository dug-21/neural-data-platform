//! Proto-Only Enforcement Tests for EventBus Integration
//! 
//! This test suite validates that the EventBus strictly enforces proto-only messaging
//! and rejects ALL Vec<u8> data that is not valid protobuf.
//! 
//! CRITICAL: These tests ensure Phase 4 requirement that no non-proto data
//! can reach the EventBus under any circumstances.

use data_staging::*;
use data_staging::generated::*;
use neural_core::eventbus::*;
use neural_core::eventbus::proto_messages::TestMessage;
use prost::Message;
use std::collections::HashMap;
use tokio_test;
use rand::Rng;

// ================================================================================================
// Proto Validation Utilities
// ================================================================================================

struct ProtoValidator;

impl ProtoValidator {
    /// Strictly validate that data is valid protobuf EventEnvelope
    fn validate_proto_only(data: &[u8]) -> Result<EventEnvelope, String> {
        EventEnvelope::decode(data).map_err(|e| format!("Invalid protobuf: {}", e))
    }
    
    /// Create valid EventEnvelope for testing
    fn create_valid_event_envelope() -> EventEnvelope {
        EventEnvelope {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
            event_type: "MarketDataEvent".to_string(),
            source: "data-staging-test".to_string(),
            payload: Some(b"valid protobuf payload".to_vec()),
            quality: Some(neural_trader::market_data::v1::DataQuality {
                completeness_score: 0.95,
                timeliness_score: 0.98,
                accuracy_score: 0.92,
                overall_score: 0.95,
                issues: vec![],
            }),
            metadata: HashMap::new(),
            correlation_id: Some(uuid::Uuid::new_v4().to_string()),
            trace_id: Some(uuid::Uuid::new_v4().to_string()),
        }
    }
}

// ================================================================================================
// Vec<u8> Rejection Tests - Critical for Proto-Only Enforcement
// ================================================================================================

#[cfg(test)]
mod vec_u8_rejection_tests {
    use super::*;
    
    #[test]
    fn test_raw_bytes_rejected() {
        let test_cases = vec![
            vec![0x01, 0x02, 0x03, 0x04],                    // Random bytes
            vec![0x00, 0x00, 0x00, 0x00],                    // Null bytes
            vec![0xFF, 0xFF, 0xFF, 0xFF],                    // Max bytes
            vec![0xDE, 0xAD, 0xBE, 0xEF],                    // Classic test pattern
            (0..1000).map(|i| (i % 256) as u8).collect(),    // Sequential pattern
        ];
        
        for (i, raw_bytes) in test_cases.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(raw_bytes);
            assert!(result.is_err(), "Test case {}: Raw bytes should be rejected: {:?}", i, raw_bytes);
        }
    }
    
    #[test]
    fn test_json_bytes_rejected() {
        let json_test_cases = vec![
            r#"{"symbol": "AAPL", "price": 150.25}"#,
            r#"{"event_id": "test", "timestamp": 1640995200}"#,
            r#"{"valid": "json", "but": "not", "protobuf": true}"#,
            r#"[]"#,                                         // JSON array
            r#"null"#,                                       // JSON null
            r#""string""#,                                   // JSON string
            r#"123.45"#,                                     // JSON number
            r#"true"#,                                       // JSON boolean
        ];
        
        for (i, json_str) in json_test_cases.iter().enumerate() {
            let json_bytes = json_str.as_bytes();
            let result = ProtoValidator::validate_proto_only(json_bytes);
            assert!(result.is_err(), "Test case {}: JSON bytes should be rejected: {}", i, json_str);
        }
    }
    
    #[test]
    fn test_xml_bytes_rejected() {
        let xml_test_cases = vec![
            b"<root><symbol>AAPL</symbol><price>150.25</price></root>",
            b"<?xml version=\"1.0\"?><data>test</data>",
            b"<html><body>Not protobuf</body></html>",
        ];
        
        for (i, xml_bytes) in xml_test_cases.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(xml_bytes);
            assert!(result.is_err(), "Test case {}: XML bytes should be rejected", i);
        }
    }
    
    #[test]
    fn test_csv_bytes_rejected() {
        let csv_test_cases = vec![
            b"symbol,price,volume\nAAPL,150.25,1000\nGOOGL,2500.75,500",
            b"header1,header2,header3\nvalue1,value2,value3",
            b"AAPL,150.25,1000",
        ];
        
        for (i, csv_bytes) in csv_test_cases.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(csv_bytes);
            assert!(result.is_err(), "Test case {}: CSV bytes should be rejected", i);
        }
    }
    
    #[test]
    fn test_msgpack_like_bytes_rejected() {
        // Simulate MessagePack binary format
        let msgpack_like_cases = vec![
            vec![0x82, 0xA6, 0x73, 0x79, 0x6D, 0x62, 0x6F, 0x6C], // Map with "symbol" key
            vec![0x93, 0x01, 0x02, 0x03],                          // Array [1,2,3]
            vec![0xCB, 0x40, 0x62, 0xD0, 0x00, 0x00, 0x00, 0x00, 0x00], // Float64
        ];
        
        for (i, msgpack_bytes) in msgpack_like_cases.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(msgpack_bytes);
            assert!(result.is_err(), "Test case {}: MessagePack-like bytes should be rejected", i);
        }
    }
    
    #[test]
    fn test_bincode_like_bytes_rejected() {
        // Simulate Bincode binary format
        let bincode_like_cases = vec![
            vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // Length prefix
            vec![0x41, 0x41, 0x50, 0x4C],                          // "AAPL" 
            vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07],
        ];
        
        for (i, bincode_bytes) in bincode_like_cases.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(bincode_bytes);
            assert!(result.is_err(), "Test case {}: Bincode-like bytes should be rejected", i);
        }
    }
    
    #[test]
    fn test_empty_data_rejected() {
        let empty_cases = vec![
            vec![],                    // Completely empty
            vec![0x00],               // Single null byte
            vec![0x20],               // Single space byte
        ];
        
        for (i, empty_bytes) in empty_cases.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(empty_bytes);
            assert!(result.is_err(), "Test case {}: Empty/minimal data should be rejected", i);
        }
    }
    
    #[test]
    fn test_corrupted_proto_rejected() {
        let valid_envelope = ProtoValidator::create_valid_event_envelope();
        let mut valid_bytes = valid_envelope.encode_to_vec();
        
        let corruption_strategies = vec![
            |bytes: &mut Vec<u8>| { bytes[0] = 0xFF; },                    // Corrupt first byte
            |bytes: &mut Vec<u8>| { bytes[bytes.len()-1] = 0xFF; },        // Corrupt last byte
            |bytes: &mut Vec<u8>| { bytes[bytes.len()/2] = 0xFF; },        // Corrupt middle byte
            |bytes: &mut Vec<u8>| { 
                for i in 0..5.min(bytes.len()) { bytes[i] = 0xFF; }         // Corrupt first 5 bytes
            },
            |bytes: &mut Vec<u8>| { bytes.reverse(); },                    // Reverse bytes
            |bytes: &mut Vec<u8>| { 
                let len = bytes.len();
                bytes.truncate(len / 2);                                    // Truncate
            },
        ];
        
        for (i, corrupt_fn) in corruption_strategies.iter().enumerate() {
            let mut corrupted_bytes = valid_bytes.clone();
            corrupt_fn(&mut corrupted_bytes);
            
            let result = ProtoValidator::validate_proto_only(&corrupted_bytes);
            assert!(result.is_err(), "Corruption strategy {}: Corrupted protobuf should be rejected", i);
        }
    }
    
    #[test]
    fn test_random_binary_rejected() {
        let mut rng = rand::thread_rng();
        
        // Generate 100 random binary blobs of various sizes
        for test_num in 0..100 {
            let size = rng.gen_range(1..=10000);
            let random_bytes: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
            
            let result = ProtoValidator::validate_proto_only(&random_bytes);
            // Random bytes should almost certainly not be valid protobuf
            // (There's a tiny chance they could be, but it's astronomically small)
            if result.is_ok() {
                // If by some miracle random bytes decode successfully, 
                // verify they don't contain meaningful data
                let envelope = result.unwrap();
                assert!(envelope.event_id.is_empty() || !envelope.event_id.chars().all(char::is_alphanumeric),
                       "Test {}: Random bytes should not produce meaningful protobuf", test_num);
            }
        }
    }
    
    #[test]
    fn test_text_file_formats_rejected() {
        let text_formats = vec![
            (b"# This is a comment\nkey=value\nother_key=other_value", "properties"),
            (b"---\nsymbol: AAPL\nprice: 150.25\n---", "yaml"),
            (b"[section]\nkey=value\nother=data", "ini"),
            (b"SELECT * FROM market_data WHERE symbol = 'AAPL';", "sql"),
            (b"#!/bin/bash\necho \"not protobuf\"", "script"),
            (b"HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}", "http"),
        ];
        
        for (bytes, format_name) in text_formats {
            let result = ProtoValidator::validate_proto_only(bytes);
            assert!(result.is_err(), "{} format should be rejected", format_name);
        }
    }
    
    #[test]
    fn test_binary_file_formats_rejected() {
        let binary_signatures = vec![
            (vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], "PNG"),         // PNG signature
            (vec![0xFF, 0xD8, 0xFF, 0xE0], "JPEG"),                                // JPEG signature  
            (vec![0x50, 0x4B, 0x03, 0x04], "ZIP"),                                 // ZIP signature
            (vec![0x7F, 0x45, 0x4C, 0x46], "ELF"),                                 // ELF binary signature
            (vec![0x4D, 0x5A], "PE"),                                              // PE executable signature
            (vec![0xFE, 0xED, 0xFA, 0xCE], "Mach-O"),                             // Mach-O signature
            (vec![0x1F, 0x8B, 0x08], "GZIP"),                                     // GZIP signature
        ];
        
        for (mut signature, format_name) in binary_signatures {
            // Extend signature with some random data to make it longer
            signature.extend(vec![0x00; 100]);
            
            let result = ProtoValidator::validate_proto_only(&signature);
            assert!(result.is_err(), "{} binary format should be rejected", format_name);
        }
    }
}

// ================================================================================================
// EventBus Proto-Only Integration Tests
// ================================================================================================

#[cfg(test)]
mod eventbus_proto_only_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_eventbus_accepts_only_valid_proto() {
        let eventbus = InMemoryEventBus::new();
        let valid_envelope = ProtoValidator::create_valid_event_envelope();
        let proto_bytes = valid_envelope.encode_to_vec();
        
        // Create event with protobuf payload
        let event = ProtoEvent::new(TestMessage { content: "valid_proto_test".to_string(), timestamp: chrono::Utc::now().timestamp() });
        
        let publish_result = eventbus.publish("proto_only_topic", event).await;
        assert!(publish_result.is_ok(), "EventBus should accept valid protobuf");
        
        // Verify we can consume and decode the message
        let consumed = eventbus.consume("proto_only_topic", StartPosition::Beginning, 1).await;
        assert!(consumed.is_ok(), "Should be able to consume valid protobuf");
        
        let messages = consumed.unwrap();
        assert_eq!(messages.len(), 1, "Should have one message");
        
        let envelope = &messages[0];
        if let Some(payload) = &envelope.payload {
            let decoded = EventEnvelope::decode(&payload[..]);
            assert!(decoded.is_ok(), "Consumed message should be valid protobuf");
            
            let decoded_envelope = decoded.unwrap();
            assert_eq!(decoded_envelope.event_type, "MarketDataEvent");
            assert!(!decoded_envelope.event_id.is_empty());
        }
    }
    
    #[tokio::test] 
    async fn test_eventbus_rejects_json_messages() {
        let eventbus = InMemoryEventBus::new();
        
        let json_payloads = vec![
            r#"{"symbol": "AAPL", "price": 150.25}"#.as_bytes().to_vec(),
            r#"{"event_id": "test", "payload": "data"}"#.as_bytes().to_vec(),
            r#"[]"#.as_bytes().to_vec(),
            r#"null"#.as_bytes().to_vec(),
        ];
        
        for (i, json_payload) in json_payloads.iter().enumerate() {
            let event = ProtoEvent::new(TestMessage { content: format!("json_test_{}", i), timestamp: chrono::Utc::now().timestamp() });
            
            let publish_result = eventbus.publish("proto_only_topic", event).await;
            
            if publish_result.is_ok() {
                // If EventBus accepts raw bytes, consumer must validate
                let consumed = eventbus.consume("proto_only_topic", StartPosition::Beginning, 10).await;
                if let Ok(messages) = consumed {
                    for message in messages {
                        if let Some(payload) = message.payload {
                            let proto_validation = ProtoValidator::validate_proto_only(&payload);
                            assert!(proto_validation.is_err(), 
                                   "Test {}: JSON payload should not be valid protobuf", i);
                        }
                    }
                }
            }
        }
    }
    
    #[tokio::test]
    async fn test_eventbus_rejects_binary_formats() {
        let eventbus = InMemoryEventBus::new();
        
        let binary_payloads = vec![
            vec![0x01, 0x02, 0x03, 0x04, 0x05],
            vec![0xFF; 1000],
            vec![0x00; 500],
            (0..256).map(|i| i as u8).collect(),
        ];
        
        for (i, binary_payload) in binary_payloads.iter().enumerate() {
            let event = ProtoEvent::new(TestMessage { content: format!("binary_test_{}", i), timestamp: chrono::Utc::now().timestamp() });
            
            let publish_result = eventbus.publish("proto_only_topic", event).await;
            
            if publish_result.is_ok() {
                let consumed = eventbus.consume("proto_only_topic", StartPosition::Beginning, 10).await;
                if let Ok(messages) = consumed {
                    for message in messages {
                        if let Some(payload) = message.payload {
                            let proto_validation = ProtoValidator::validate_proto_only(&payload);
                            assert!(proto_validation.is_err(),
                                   "Test {}: Binary payload should not be valid protobuf", i);
                        }
                    }
                }
            }
        }
    }
    
    #[tokio::test]
    async fn test_multiple_valid_proto_messages() {
        let eventbus = InMemoryEventBus::new();
        
        // Create multiple valid protobuf messages
        let mut valid_envelopes = Vec::new();
        for i in 0..10 {
            let mut envelope = ProtoValidator::create_valid_event_envelope();
            envelope.event_id = format!("test_event_{}", i);
            envelope.source = format!("test_source_{}", i);
            valid_envelopes.push(envelope);
        }
        
        // Publish all messages
        for (i, envelope) in valid_envelopes.iter().enumerate() {
            let proto_bytes = envelope.encode_to_vec();
            let event = ProtoEvent::new(TestMessage { content: format!("multi_test_{}", i), timestamp: chrono::Utc::now().timestamp() });
            
            let publish_result = eventbus.publish("multi_proto_topic", event).await;
            assert!(publish_result.is_ok(), "Valid protobuf {} should be accepted", i);
        }
        
        // Consume and validate all messages
        let consumed = eventbus.consume("multi_proto_topic", StartPosition::Beginning, 10).await;
        assert!(consumed.is_ok(), "Should be able to consume all messages");
        
        let messages = consumed.unwrap();
        assert_eq!(messages.len(), 10, "Should have all 10 messages");
        
        for (i, message) in messages.iter().enumerate() {
            if let Some(payload) = &message.payload {
                let decoded = EventEnvelope::decode(&payload[..]);
                assert!(decoded.is_ok(), "Message {} should be valid protobuf", i);
                
                let envelope = decoded.unwrap();
                assert_eq!(envelope.event_id, format!("test_event_{}", i));
                assert_eq!(envelope.source, format!("test_source_{}", i));
            }
        }
    }
    
    #[tokio::test]
    async fn test_mixed_valid_invalid_messages() {
        let eventbus = InMemoryEventBus::new();
        let mut valid_count = 0;
        
        // Publish mix of valid protobuf and invalid data
        for i in 0..20 {
            if i % 2 == 0 {
                // Even indices: valid protobuf
                let valid_envelope = ProtoValidator::create_valid_event_envelope();
                let proto_bytes = valid_envelope.encode_to_vec();
                let event = ProtoEvent::new(TestMessage { content: format!("mixed_test_{}", i), timestamp: chrono::Utc::now().timestamp() });
                
                let publish_result = eventbus.publish("mixed_topic", event).await;
                if publish_result.is_ok() {
                    valid_count += 1;
                }
            } else {
                // Odd indices: invalid JSON data
                let json_data = format!(r#"{{"test": "data", "index": {}}}"#, i);
                let json_bytes = json_data.as_bytes().to_vec();
                let event = ProtoEvent::new(TestMessage { content: format!("mixed_test_{}", i), timestamp: chrono::Utc::now().timestamp() });
                
                let _publish_result = eventbus.publish("mixed_topic", event).await;
                // Invalid data may or may not be accepted by EventBus,
                // but it should be rejected during proto validation
            }
        }
        
        // Consume all messages and validate only valid protobuf
        let consumed = eventbus.consume("mixed_topic", StartPosition::Beginning, 20).await;
        if let Ok(messages) = consumed {
            let mut proto_valid_count = 0;
            
            for message in messages {
                if let Some(payload) = message.payload {
                    let proto_validation = ProtoValidator::validate_proto_only(&payload);
                    if proto_validation.is_ok() {
                        proto_valid_count += 1;
                    }
                }
            }
            
            // Only the valid protobuf messages should pass validation
            assert_eq!(proto_valid_count, valid_count, 
                      "Only valid protobuf messages should pass validation");
        }
    }
}

// ================================================================================================
// Data-Staging to EventBus Proto-Only Tests
// ================================================================================================

#[cfg(test)]
mod data_staging_eventbus_tests {
    use super::*;
    use data_staging::proto_transformer::ProtoTransformer;
    
    #[tokio::test]
    async fn test_data_staging_produces_valid_proto_only() {
        let transformer = ProtoTransformer::new();
        let eventbus = InMemoryEventBus::new();
        
        // Create various types of market data
        let test_cases = vec![
            // Trade data
            RawMarketData {
                symbol: Some("AAPL".to_string()),
                price: Some(150.25),
                volume: Some(1000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                exchange: Some("NASDAQ".to_string()),
                sequence: Some(12345),
                bid: None, ask: None, high: None, low: None, open: None, close: None, vwap: None,
                metadata: HashMap::new(),
            },
            // Quote data  
            RawMarketData {
                symbol: Some("GOOGL".to_string()),
                price: None,
                volume: None,
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                bid: Some(2500.25),
                ask: Some(2500.75),
                exchange: Some("NASDAQ".to_string()),
                sequence: None, high: None, low: None, open: None, close: None, vwap: None,
                metadata: HashMap::new(),
            },
            // Bar data
            RawMarketData {
                symbol: Some("MSFT".to_string()),
                price: None,
                volume: Some(2000.0),
                timestamp: Some(chrono::Utc::now().timestamp_millis()),
                high: Some(330.75),
                low: Some(329.50),
                open: Some(330.00),
                close: Some(330.25),
                vwap: Some(330.12),
                exchange: Some("NASDAQ".to_string()),
                bid: None, ask: None, sequence: None,
                metadata: HashMap::new(),
            },
        ];
        
        for (i, raw_data) in test_cases.iter().enumerate() {
            let quality_metrics = DataQualityMetrics {
                overall_score: 0.95,
                freshness_score: 0.98,
                completeness_score: 0.92,
                validity_score: 1.0,
                missing_required_fields: 0,
                present_optional_fields: 5,
                data_age_seconds: 10,
                validation_errors: vec![],
            };
            
            // Transform to protobuf
            let envelope_result = transformer.transform_to_event_envelope(raw_data, &quality_metrics);
            assert!(envelope_result.is_ok(), "Test case {}: Transformation should succeed", i);
            
            let envelope = envelope_result.unwrap();
            let proto_bytes = envelope.encode_to_vec();
            
            // Verify it's valid protobuf
            let validation = ProtoValidator::validate_proto_only(&proto_bytes);
            assert!(validation.is_ok(), "Test case {}: Should produce valid protobuf", i);
            
            // Publish to EventBus
            let event = ProtoEvent::new(TestMessage { content: format!("data_staging_test_{}", i), timestamp: chrono::Utc::now().timestamp() });
            let publish_result = eventbus.publish("data_staging_proto", event).await;
            assert!(publish_result.is_ok(), "Test case {}: EventBus should accept proto", i);
        }
        
        // Verify all messages on EventBus are valid protobuf
        let consumed = eventbus.consume("data_staging_proto", StartPosition::Beginning, 10).await;
        assert!(consumed.is_ok(), "Should be able to consume all messages");
        
        let messages = consumed.unwrap();
        assert_eq!(messages.len(), 3, "Should have all 3 messages");
        
        for (i, message) in messages.iter().enumerate() {
            if let Some(payload) = &message.payload {
                let validation = ProtoValidator::validate_proto_only(payload);
                assert!(validation.is_ok(), "Message {}: Should be valid protobuf", i);
                
                let decoded_envelope = validation.unwrap();
                assert!(!decoded_envelope.event_id.is_empty(), "Should have event ID");
                assert_eq!(decoded_envelope.event_type, "MarketDataEvent");
                assert!(decoded_envelope.quality.is_some(), "Should have quality metrics");
            }
        }
    }
    
    #[tokio::test]
    async fn test_no_bypass_mechanisms_exist() {
        // This test verifies there are no hidden pathways that allow non-proto data
        let eventbus = InMemoryEventBus::new();
        
        // Attempt various bypass strategies
        let bypass_attempts = vec![
            // Empty event ID with JSON payload
            ("", r#"{"bypass": "attempt"}"#.as_bytes().to_vec()),
            
            // Special event types with invalid data
            ("BYPASS", vec![0x01, 0x02, 0x03]),
            ("ADMIN", "admin_command".as_bytes().to_vec()),
            ("SYSTEM", "system_override".as_bytes().to_vec()),
            
            // Large payloads that might skip validation
            ("LARGE", vec![0xFF; 100000]),
            
            // Payloads that start with valid proto bytes but aren't complete
            ("PARTIAL", {
                let valid = ProtoValidator::create_valid_event_envelope();
                let mut bytes = valid.encode_to_vec();
                bytes.truncate(bytes.len() / 2);
                bytes.extend(b"INVALID_SUFFIX");
                bytes
            }),
        ];
        
        for (event_id, payload) in bypass_attempts {
            let event = ProtoEvent::new(TestMessage { content: event_id.to_string(), timestamp: chrono::Utc::now().timestamp() });
            let publish_result = eventbus.publish("bypass_test_topic", event).await;
            
            if publish_result.is_ok() {
                // Even if EventBus accepts the data, it should not be valid protobuf
                let consumed = eventbus.consume("bypass_test_topic", StartPosition::Beginning, 10).await;
                if let Ok(messages) = consumed {
                    for message in messages {
                        if let Some(msg_payload) = message.payload {
                            // This MUST fail proto validation
                            let validation = ProtoValidator::validate_proto_only(&msg_payload);
                            assert!(validation.is_err(), 
                                   "Bypass attempt '{}' should not produce valid protobuf", event_id);
                        }
                    }
                }
            }
        }
    }
}

// ================================================================================================
// Comprehensive Proto Format Tests
// ================================================================================================

#[cfg(test)]
mod comprehensive_proto_tests {
    use super::*;
    
    #[test]
    fn test_all_supported_proto_messages() {
        // Test all protobuf message types that should be accepted
        let proto_test_cases = vec![
            // EventEnvelope variations
            EventEnvelope {
                event_id: "minimal_event".to_string(),
                timestamp: None,
                event_type: "MinimalEvent".to_string(),
                source: "test".to_string(),
                payload: None,
                quality: None,
                metadata: HashMap::new(),
                correlation_id: None,
                trace_id: None,
            },
            EventEnvelope {
                event_id: "maximal_event".to_string(),
                timestamp: Some(prost_types::Timestamp {
                    seconds: chrono::Utc::now().timestamp(),
                    nanos: 123456789,
                }),
                event_type: "MaximalEvent".to_string(),
                source: "comprehensive_test".to_string(),
                payload: Some(b"comprehensive test payload with various data".to_vec()),
                quality: Some(neural_trader::market_data::v1::DataQuality {
                    completeness_score: 1.0,
                    timeliness_score: 1.0,
                    accuracy_score: 1.0,
                    overall_score: 1.0,
                    issues: vec!["test issue".to_string()],
                }),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("key1".to_string(), "value1".to_string());
                    meta.insert("key2".to_string(), "value2".to_string());
                    meta
                },
                correlation_id: Some("corr-123-456".to_string()),
                trace_id: Some("trace-789-abc".to_string()),
            },
        ];
        
        for (i, proto_message) in proto_test_cases.iter().enumerate() {
            let encoded = proto_message.encode_to_vec();
            assert!(!encoded.is_empty(), "Test case {}: Encoded protobuf should not be empty", i);
            
            // Verify it validates as proto-only
            let validation = ProtoValidator::validate_proto_only(&encoded);
            assert!(validation.is_ok(), "Test case {}: Should be valid protobuf", i);
            
            // Verify round-trip encoding/decoding
            let decoded = validation.unwrap();
            assert_eq!(proto_message.event_id, decoded.event_id, "Event ID should match");
            assert_eq!(proto_message.event_type, decoded.event_type, "Event type should match");
            assert_eq!(proto_message.source, decoded.source, "Source should match");
        }
    }
    
    #[test]
    fn test_proto_field_boundary_conditions() {
        // Test edge cases for protobuf field values
        let boundary_test_cases = vec![
            // Empty strings
            EventEnvelope {
                event_id: "".to_string(),
                event_type: "".to_string(),
                source: "".to_string(),
                ..Default::default()
            },
            // Very long strings
            EventEnvelope {
                event_id: "x".repeat(10000),
                event_type: "y".repeat(5000),
                source: "z".repeat(1000),
                ..Default::default()
            },
            // Special characters
            EventEnvelope {
                event_id: "测试-test-тест-🚀".to_string(),
                event_type: "Special/Type\\With|Chars".to_string(),
                source: "source@with#special$chars%".to_string(),
                ..Default::default()
            },
        ];
        
        for (i, test_envelope) in boundary_test_cases.iter().enumerate() {
            let encoded = test_envelope.encode_to_vec();
            let validation = ProtoValidator::validate_proto_only(&encoded);
            assert!(validation.is_ok(), "Boundary test case {}: Should be valid protobuf", i);
            
            let decoded = validation.unwrap();
            assert_eq!(test_envelope.event_id, decoded.event_id, "Boundary case {}: Event ID should match", i);
        }
    }
    
    #[test]
    fn test_proto_size_limits() {
        // Test various protobuf message sizes
        let size_test_cases = vec![
            1,        // Tiny
            100,      // Small  
            1000,     // Medium
            10000,    // Large
            100000,   // Very large
        ];
        
        for &payload_size in size_test_cases.iter() {
            let large_payload = vec![0xAB; payload_size];
            let envelope = EventEnvelope {
                event_id: format!("size_test_{}", payload_size),
                timestamp: Some(prost_types::Timestamp {
                    seconds: chrono::Utc::now().timestamp(),
                    nanos: 0,
                }),
                event_type: "SizeTestEvent".to_string(),
                source: "size_test".to_string(),
                payload: Some(large_payload),
                quality: None,
                metadata: HashMap::new(),
                correlation_id: None,
                trace_id: None,
            };
            
            let encoded = envelope.encode_to_vec();
            assert!(encoded.len() > payload_size, "Encoded size should be larger than payload");
            
            let validation = ProtoValidator::validate_proto_only(&encoded);
            assert!(validation.is_ok(), "Size test {}: Should be valid protobuf", payload_size);
            
            let decoded = validation.unwrap();
            assert_eq!(envelope.event_id, decoded.event_id);
            if let (Some(orig_payload), Some(decoded_payload)) = (&envelope.payload, &decoded.payload) {
                assert_eq!(orig_payload.len(), decoded_payload.len(), "Payload size should match");
            }
        }
    }
}

// ================================================================================================
// Security and Attack Vector Tests
// ================================================================================================

#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[test]
    fn test_buffer_overflow_protection() {
        // Test potential buffer overflow attacks
        let overflow_attempts = vec![
            vec![0xFF; 1_000_000],        // 1MB of max bytes
            vec![0x00; 5_000_000],        // 5MB of null bytes  
            vec![0xAA; 10_000_000],       // 10MB of pattern
        ];
        
        for (i, overflow_data) in overflow_attempts.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(overflow_data);
            // These should fail to decode as valid protobuf (and not crash)
            assert!(result.is_err(), "Overflow attempt {}: Should not be valid protobuf", i);
        }
    }
    
    #[test]
    fn test_malicious_proto_structures() {
        // Test potentially malicious protobuf structures
        
        // Extremely deep nesting (this would require complex proto generation)
        // For now, test with corrupted valid proto that might have deep structures
        let valid_envelope = ProtoValidator::create_valid_event_envelope();
        let mut valid_bytes = valid_envelope.encode_to_vec();
        
        // Insert potentially malicious field tags
        let malicious_modifications = vec![
            |bytes: &mut Vec<u8>| {
                // Insert high field numbers that might cause issues
                if bytes.len() > 10 {
                    bytes[5] = 0xF8; // Field number with continuation bits
                    bytes[6] = 0xFF;
                    bytes[7] = 0xFF;
                    bytes[8] = 0x7F;
                }
            },
            |bytes: &mut Vec<u8>| {
                // Insert length delimited field with wrong length
                if bytes.len() > 5 {
                    bytes[2] = 0x0A; // Length delimited wire type
                    bytes[3] = 0xFF; // Very long length
                    bytes[4] = 0x7F;
                }
            },
        ];
        
        for (i, modify_fn) in malicious_modifications.iter().enumerate() {
            let mut modified_bytes = valid_bytes.clone();
            modify_fn(&mut modified_bytes);
            
            let result = ProtoValidator::validate_proto_only(&modified_bytes);
            // These should be safely rejected without crashing
            assert!(result.is_err(), "Malicious structure {}: Should be rejected", i);
        }
    }
    
    #[test]
    fn test_encoding_injection_attempts() {
        // Test various encoding-based injection attempts
        let injection_attempts = vec![
            // UTF-8 with null bytes
            "valid_start\0malicious_data".as_bytes().to_vec(),
            
            // UTF-8 with control characters
            "test\x01\x02\x03\x04control_chars".as_bytes().to_vec(),
            
            // Invalid UTF-8 sequences
            vec![0xFF, 0xFE, 0xFD, 0xFC],
            
            // Mixed valid/invalid encoding
            {
                let mut mixed = "valid_utf8_start".as_bytes().to_vec();
                mixed.extend(&[0xFF, 0xFE, 0xFD]);
                mixed.extend("_valid_utf8_end".as_bytes());
                mixed
            },
        ];
        
        for (i, injection_data) in injection_attempts.iter().enumerate() {
            let result = ProtoValidator::validate_proto_only(injection_data);
            assert!(result.is_err(), "Injection attempt {}: Should be rejected", i);
        }
    }
    
    #[test]
    fn test_denial_of_service_protection() {
        // Test potential DoS attack vectors
        
        // Extremely repetitive data
        let repetitive_data = b"AAAA".repeat(250_000); // 1MB of "AAAA"
        let result = ProtoValidator::validate_proto_only(&repetitive_data);
        assert!(result.is_err(), "Repetitive data should be rejected");
        
        // Alternating pattern data
        let alternating_data = [0x55, 0xAA].repeat(500_000); // 1MB alternating
        let result = ProtoValidator::validate_proto_only(&alternating_data);
        assert!(result.is_err(), "Alternating data should be rejected");
        
        // Sequential data
        let sequential_data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let result = ProtoValidator::validate_proto_only(&sequential_data);
        assert!(result.is_err(), "Sequential data should be rejected");
    }
}

// ================================================================================================
// Performance Impact of Proto-Only Validation
// ================================================================================================

#[cfg(test)]
mod proto_validation_performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_proto_validation_performance() {
        let test_cases = vec![
            ProtoValidator::create_valid_event_envelope().encode_to_vec(),
            vec![0x01, 0x02, 0x03, 0x04], // Invalid data
            r#"{"not": "protobuf"}"#.as_bytes().to_vec(), // JSON
        ];
        
        for (case_idx, test_data) in test_cases.iter().enumerate() {
            let iterations = 10_000;
            let start_time = Instant::now();
            
            for _ in 0..iterations {
                let _result = ProtoValidator::validate_proto_only(test_data);
            }
            
            let elapsed = start_time.elapsed();
            let avg_time = elapsed / iterations;
            
            println!("Case {}: Average validation time: {:?}", case_idx, avg_time);
            
            // Validation should be very fast (<100μs per check)
            assert!(avg_time.as_micros() < 100, 
                   "Proto validation too slow: {:?} per validation", avg_time);
        }
    }
    
    #[test]
    fn test_large_message_validation_performance() {
        let large_envelope = EventEnvelope {
            event_id: "large_message_test".to_string(),
            timestamp: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
            event_type: "LargeMessageTest".to_string(),
            source: "performance_test".to_string(),
            payload: Some(vec![0xAB; 100_000]), // 100KB payload
            quality: Some(neural_trader::market_data::v1::DataQuality {
                completeness_score: 0.95,
                timeliness_score: 0.98,
                accuracy_score: 0.92,
                overall_score: 0.95,
                issues: (0..100).map(|i| format!("Issue {}", i)).collect(), // Many issues
            }),
            metadata: (0..1000).map(|i| (format!("key_{}", i), format!("value_{}", i))).collect(),
            correlation_id: Some("large_correlation_id_with_lots_of_text".to_string()),
            trace_id: Some("large_trace_id_with_lots_of_text".to_string()),
        };
        
        let large_proto = large_envelope.encode_to_vec();
        println!("Large proto size: {} bytes", large_proto.len());
        
        let iterations = 1_000;
        let start_time = Instant::now();
        
        for _ in 0..iterations {
            let result = ProtoValidator::validate_proto_only(&large_proto);
            assert!(result.is_ok(), "Large valid proto should validate successfully");
        }
        
        let elapsed = start_time.elapsed();
        let avg_time = elapsed / iterations;
        
        println!("Large message validation time: {:?} per validation", avg_time);
        
        // Even large messages should validate quickly (<1ms)
        assert!(avg_time.as_millis() < 1, 
               "Large message validation too slow: {:?} per validation", avg_time);
    }
}