# Proto-Only Testing Strategy: Strict Contract Enforcement

## Overview

This testing strategy focuses exclusively on validating strict protobuf contract enforcement. The system must **reject all non-protobuf data** and enforce schema compliance without exceptions.

## Core Testing Principles

1. **Zero Tolerance**: Any non-protobuf data is rejected
2. **Schema Validation**: All messages must conform to defined proto schemas
3. **Contract Enforcement**: No backward compatibility with legacy formats
4. **Performance**: All tests use proto-only data
5. **Security**: Validate that malformed data cannot bypass validation

## Test Categories

### 1. Proto-Only Validation Tests

#### 1.1 Valid Protobuf Acceptance
```rust
#[test]
fn test_valid_proto_message_accepted() {
    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        price: 150.25,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
        volume: 1000,
    };
    
    let encoded = market_data.encode_to_vec();
    let result = validate_proto_message(&encoded);
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap().symbol, "AAPL");
}

#[test]
fn test_all_proto_event_types_accepted() {
    let test_cases = vec![
        create_market_data_proto(),
        create_trade_signal_proto(),
        create_risk_metric_proto(),
        create_portfolio_update_proto(),
    ];
    
    for proto_message in test_cases {
        let encoded = proto_message.encode_to_vec();
        assert!(validate_proto_message(&encoded).is_ok());
    }
}
```

#### 1.2 Vec<u8> Rejection Tests
```rust
#[test]
fn test_raw_vec_u8_rejected() {
    let raw_data = vec![0x01, 0x02, 0x03, 0x04];
    let result = validate_proto_message(&raw_data);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::InvalidProtobuf);
}

#[test]
fn test_json_bytes_rejected() {
    let json_data = r#"{"symbol": "AAPL", "price": 150.25}"#.as_bytes().to_vec();
    let result = validate_proto_message(&json_data);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::InvalidProtobuf);
}

#[test]
fn test_binary_garbage_rejected() {
    let garbage_data: Vec<u8> = (0..100).map(|i| (i % 256) as u8).collect();
    let result = validate_proto_message(&garbage_data);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::InvalidProtobuf);
}
```

### 2. Schema Validation Tests

#### 2.1 Required Field Validation
```rust
#[test]
fn test_missing_required_fields_rejected() {
    // Create protobuf with missing required fields
    let incomplete_data = IncompleteMarketData {
        symbol: "AAPL".to_string(),
        // Missing required fields: price, timestamp
    };
    
    let encoded = incomplete_data.encode_to_vec();
    let result = validate_market_data_schema(&encoded);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SchemaError::MissingRequiredField("price"));
}

#[test]
fn test_invalid_field_types_rejected() {
    // Test with invalid field types (if possible in proto)
    let invalid_proto = create_proto_with_invalid_types();
    let result = validate_proto_message(&invalid_proto);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::InvalidFieldType);
}
```

#### 2.2 Schema Version Enforcement
```rust
#[test]
fn test_unknown_proto_schema_rejected() {
    let unknown_schema = UnknownProtoMessage {
        unknown_field: "value".to_string(),
    };
    
    let encoded = unknown_schema.encode_to_vec();
    let result = validate_proto_message(&encoded);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::UnknownSchema);
}

#[test]
fn test_deprecated_proto_versions_rejected() {
    let deprecated_v1 = MarketDataV1 {
        // Old schema structure
        symbol: "AAPL".to_string(),
        old_price_format: "150.25".to_string(), // String instead of f64
    };
    
    let encoded = deprecated_v1.encode_to_vec();
    let result = validate_proto_message(&encoded);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::DeprecatedSchema);
}
```

### 3. Contract Enforcement Tests

#### 3.1 Strict Parsing Tests
```rust
#[test]
fn test_malformed_protobuf_rejected() {
    let valid_proto = create_valid_market_data();
    let mut encoded = valid_proto.encode_to_vec();
    
    // Corrupt the protobuf data
    encoded[5] = 0xFF; // Corrupt a byte
    
    let result = validate_proto_message(&encoded);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::CorruptedData);
}

#[test]
fn test_truncated_protobuf_rejected() {
    let valid_proto = create_valid_market_data();
    let mut encoded = valid_proto.encode_to_vec();
    
    // Truncate the data
    encoded.truncate(encoded.len() / 2);
    
    let result = validate_proto_message(&encoded);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::TruncatedData);
}
```

#### 3.2 No Exception Handling Tests
```rust
#[test]
fn test_no_fallback_to_legacy_formats() {
    let legacy_json = r#"{"symbol": "AAPL", "price": 150.25}"#;
    let legacy_msgpack = encode_msgpack_data();
    let legacy_bincode = encode_bincode_data();
    
    // All legacy formats should be rejected
    assert!(process_message(legacy_json.as_bytes()).is_err());
    assert!(process_message(&legacy_msgpack).is_err());
    assert!(process_message(&legacy_bincode).is_err());
}

#[test]
fn test_strict_mode_enforcement() {
    // Verify system is in strict mode - no compatibility layers
    let system_config = get_system_configuration();
    
    assert_eq!(system_config.compatibility_mode, false);
    assert_eq!(system_config.proto_only, true);
    assert_eq!(system_config.validation_level, ValidationLevel::Strict);
}
```

### 4. Negative Testing

#### 4.1 Attack Vector Tests
```rust
#[test]
fn test_buffer_overflow_attempts_rejected() {
    let oversized_data = vec![0xFF; 10_000_000]; // 10MB of garbage
    let result = validate_proto_message(&oversized_data);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::MessageTooLarge);
}

#[test]
fn test_nested_bomb_attacks_rejected() {
    // Create deeply nested protobuf that could cause stack overflow
    let nested_bomb = create_deeply_nested_proto(10000);
    let result = validate_proto_message(&nested_bomb);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::NestingTooDeep);
}

#[test]
fn test_invalid_utf8_in_strings_rejected() {
    let proto_with_invalid_utf8 = create_proto_with_invalid_utf8();
    let result = validate_proto_message(&proto_with_invalid_utf8);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::InvalidUtf8);
}
```

#### 4.2 Edge Case Rejection Tests
```rust
#[test]
fn test_empty_message_handling() {
    let empty_data = vec![];
    let result = validate_proto_message(&empty_data);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::EmptyMessage);
}

#[test]
fn test_null_byte_injection_rejected() {
    let data_with_nulls = vec![0x00, 0x01, 0x00, 0x02];
    let result = validate_proto_message(&data_with_nulls);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::InvalidProtobuf);
}
```

### 5. Performance Tests (Proto-Only)

#### 5.1 Throughput Testing
```rust
#[test]
fn test_proto_validation_throughput() {
    let proto_messages: Vec<Vec<u8>> = (0..10000)
        .map(|i| create_market_data_proto_with_id(i).encode_to_vec())
        .collect();
    
    let start = Instant::now();
    
    for message in &proto_messages {
        validate_proto_message(message).expect("All messages should be valid proto");
    }
    
    let duration = start.elapsed();
    let throughput = proto_messages.len() as f64 / duration.as_secs_f64();
    
    // Ensure we can process at least 10,000 proto messages per second
    assert!(throughput > 10_000.0);
}

#[test]
fn test_large_proto_message_performance() {
    let large_proto = create_large_market_data_proto(); // ~1MB proto
    let encoded = large_proto.encode_to_vec();
    
    let start = Instant::now();
    let result = validate_proto_message(&encoded);
    let duration = start.elapsed();
    
    assert!(result.is_ok());
    assert!(duration < Duration::from_millis(10)); // Should validate in <10ms
}
```

#### 5.2 Memory Efficiency Tests
```rust
#[test]
fn test_proto_memory_usage() {
    let initial_memory = get_memory_usage();
    
    // Process 1000 proto messages
    for i in 0..1000 {
        let proto = create_market_data_proto_with_id(i);
        let encoded = proto.encode_to_vec();
        validate_proto_message(&encoded).unwrap();
    }
    
    let final_memory = get_memory_usage();
    let memory_increase = final_memory - initial_memory;
    
    // Memory increase should be minimal (less than 10MB)
    assert!(memory_increase < 10 * 1024 * 1024);
}
```

## 9. Data-Staging Service Testing (NEW)

### 9.1 Unit Tests
```rust
#[cfg(test)]
mod data_staging_tests {
    use super::*;
    use crate::data_staging::*;
    use crate::proto::market::*;
    use redis_test::*;

    // Test JSON parsing
    #[test]
    fn test_parse_valid_market_json() {
        let valid_json = r#"{
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 1000,
            "timestamp": 1640995200000
        }"#;
        
        let result = parse_market_json(valid_json);
        assert!(result.is_ok());
        
        let parsed = result.unwrap();
        assert_eq!(parsed.symbol, "AAPL");
        assert_eq!(parsed.price, 150.25);
    }
    
    // Test proto conversion
    #[test]
    fn test_json_to_proto_conversion() {
        let json_data = create_test_market_json();
        let proto_result = convert_json_to_market_data_proto(json_data);
        
        assert!(proto_result.is_ok());
        
        let proto = proto_result.unwrap();
        let encoded = proto.encode_to_vec();
        
        // Ensure it's valid protobuf
        let decoded = MarketData::decode(&encoded[..]);
        assert!(decoded.is_ok());
        assert_eq!(decoded.unwrap().symbol, "AAPL");
    }
    
    // Test quality scoring
    #[test]
    fn test_quality_score_calculation() {
        let high_quality = MarketDataJson {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000),
            timestamp: Some(1640995200000),
            bid: Some(150.20),
            ask: Some(150.30),
        };
        
        let low_quality = MarketDataJson {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: None, // Missing volume
            timestamp: None, // Missing timestamp
            bid: None,
            ask: None,
        };
        
        let high_score = calculate_quality_score(&high_quality);
        let low_score = calculate_quality_score(&low_quality);
        
        assert!(high_score > 0.9);
        assert!(low_score < 0.5);
        assert!(high_score > low_score);
    }
    
    // Test field validation
    #[test]
    fn test_required_field_validation() {
        let missing_symbol = r#"{
            "price": 150.25,
            "volume": 1000,
            "timestamp": 1640995200000
        }"#;
        
        let missing_price = r#"{
            "symbol": "AAPL",
            "volume": 1000,
            "timestamp": 1640995200000
        }"#;
        
        assert!(validate_required_fields(missing_symbol).is_err());
        assert!(validate_required_fields(missing_price).is_err());
        
        let valid_json = r#"{
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 1000,
            "timestamp": 1640995200000
        }"#;
        
        assert!(validate_required_fields(valid_json).is_ok());
    }

    #[test]
    fn test_range_validation() {
        let negative_price = r#"{
            "symbol": "AAPL",
            "price": -150.25,
            "volume": 1000,
            "timestamp": 1640995200000
        }"#;
        
        let zero_volume = r#"{
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 0,
            "timestamp": 1640995200000
        }"#;
        
        let future_timestamp = r#"{
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 1000,
            "timestamp": 9999999999999
        }"#;
        
        assert!(validate_field_ranges(negative_price).is_err());
        assert!(validate_field_ranges(zero_volume).is_err());
        assert!(validate_field_ranges(future_timestamp).is_err());
    }

    #[test]
    fn test_malformed_json_handling() {
        let malformed_jsons = vec![
            r#"{"symbol": "AAPL", "price": 150.25"#, // Missing closing brace
            r#"{"symbol": "AAPL" "price": 150.25}"#, // Missing comma
            r#"{"symbol": AAPL, "price": 150.25}"#,  // Unquoted string
            "",                                      // Empty string
            "not json at all",                      // Invalid JSON
        ];
        
        for malformed_json in malformed_jsons {
            let result = parse_market_json(malformed_json);
            assert!(result.is_err(), "Should reject malformed JSON: {}", malformed_json);
        }
    }
}
```

### 9.2 Integration Tests
```rust
#[cfg(test)]
mod data_staging_integration_tests {
    use super::*;
    use tokio_test;
    use redis::Commands;

    // Redis consumption from live channels
    #[tokio::test]
    async fn test_redis_consumption_integration() {
        let redis_client = setup_test_redis().await;
        let staging_service = DataStagingService::new(redis_client.clone());
        
        // Publish test data to Redis channel
        let test_json = r#"{"symbol": "AAPL", "price": 150.25, "volume": 1000, "timestamp": 1640995200000}"#;
        redis_client.publish("market_data_raw", test_json).await.unwrap();
        
        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify proto was published to EventBus
        let proto_messages = get_eventbus_messages("market_data_proto").await;
        assert_eq!(proto_messages.len(), 1);
        
        let proto = MarketData::decode(&proto_messages[0]).unwrap();
        assert_eq!(proto.symbol, "AAPL");
        assert_eq!(proto.price, 150.25);
    }
    
    // Proto publishing to EventBus
    #[tokio::test]
    async fn test_proto_publishing_integration() {
        let staging_service = setup_staging_service().await;
        let eventbus_client = setup_test_eventbus().await;
        
        let json_data = create_test_market_json();
        let result = staging_service.process_json_message(json_data).await;
        
        assert!(result.is_ok());
        
        // Verify message was published to correct EventBus topic
        let messages = eventbus_client.consume("market_data_proto", 1).await;
        assert_eq!(messages.len(), 1);
        
        // Verify it's valid protobuf
        let proto = MarketData::decode(&messages[0].payload).unwrap();
        assert!(!proto.symbol.is_empty());
        assert!(proto.price > 0.0);
    }
    
    // DLQ handling for invalid data
    #[tokio::test]
    async fn test_dlq_handling_integration() {
        let staging_service = setup_staging_service().await;
        let dlq_consumer = setup_dlq_consumer().await;
        
        let invalid_jsons = vec![
            r#"{"symbol": "AAPL"}"#,              // Missing required fields
            r#"{"symbol": "AAPL", "price": -1}"#, // Invalid range
            "not json",                          // Malformed JSON
            "",                                  // Empty message
        ];
        
        for invalid_json in invalid_jsons {
            let result = staging_service.process_json_message(invalid_json).await;
            assert!(result.is_err());
        }
        
        // Verify DLQ received all invalid messages
        let dlq_messages = dlq_consumer.consume_all().await;
        assert_eq!(dlq_messages.len(), 4);
        
        // Verify DLQ messages contain error information
        for dlq_message in dlq_messages {
            assert!(dlq_message.error_reason.is_some());
            assert!(dlq_message.original_payload.is_some());
            assert!(dlq_message.timestamp > 0);
        }
    }
    
    // Quality metrics aggregation
    #[tokio::test]
    async fn test_quality_metrics_aggregation() {
        let staging_service = setup_staging_service().await;
        let metrics_collector = setup_metrics_collector().await;
        
        // Process mix of high and low quality data
        let test_data = vec![
            create_high_quality_json(),  // Quality score: 1.0
            create_medium_quality_json(), // Quality score: 0.7
            create_low_quality_json(),    // Quality score: 0.3
            create_high_quality_json(),   // Quality score: 1.0
        ];
        
        for json_data in test_data {
            staging_service.process_json_message(json_data).await.unwrap();
        }
        
        // Wait for metrics aggregation
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let metrics = metrics_collector.get_quality_metrics().await;
        assert_eq!(metrics.total_messages, 4);
        assert_eq!(metrics.high_quality_count, 2);   // Score >= 0.8
        assert_eq!(metrics.medium_quality_count, 1); // 0.5 <= Score < 0.8
        assert_eq!(metrics.low_quality_count, 1);    // Score < 0.5
        assert!((metrics.average_quality_score - 0.75).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_end_to_end_data_flow() {
        // Setup entire pipeline
        let redis_client = setup_test_redis().await;
        let staging_service = setup_staging_service().await;
        let eventbus = setup_test_eventbus().await;
        let consumer = setup_test_consumer().await;
        
        // Publish raw JSON to Redis
        let json_data = r#"{
            "symbol": "TSLA",
            "price": 800.50,
            "volume": 2500,
            "timestamp": 1640995200000,
            "bid": 800.45,
            "ask": 800.55
        }"#;
        
        redis_client.publish("market_data_raw", json_data).await.unwrap();
        
        // Wait for complete processing pipeline
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify proto message was consumed
        let consumed_messages = consumer.get_consumed_messages().await;
        assert_eq!(consumed_messages.len(), 1);
        
        let proto = MarketData::decode(&consumed_messages[0].data).unwrap();
        assert_eq!(proto.symbol, "TSLA");
        assert_eq!(proto.price, 800.50);
        assert_eq!(proto.volume, 2500);
        
        // Verify quality was tracked
        let quality_score = consumed_messages[0].quality_score;
        assert!(quality_score > 0.9); // Should be high quality
        
        // Verify no raw JSON made it to EventBus
        let raw_messages = eventbus.get_raw_messages().await;
        assert!(raw_messages.is_empty());
    }
}
```

### 9.3 Data Validation Tests
```rust
#[cfg(test)]
mod data_validation_tests {
    use super::*;

    #[test]
    fn test_reject_missing_required_fields() {
        let invalid_json = r#"{"price": 100.0}"#; // missing symbol
        let dlq = setup_test_dlq();
        let staging = DataStagingService::new_with_dlq(dlq.clone());
        
        let result = staging.transform(invalid_json);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::MissingRequiredField);
        assert_eq!(dlq.count(), 1);
        
        let dlq_message = dlq.peek_latest();
        assert_eq!(dlq_message.error_reason, "Missing required field: symbol");
        assert_eq!(dlq_message.original_payload, invalid_json);
    }

    #[test]
    fn test_reject_invalid_ranges() {
        let test_cases = vec![
            (r#"{"symbol": "AAPL", "price": -100.0}"#, "Negative price not allowed"),
            (r#"{"symbol": "AAPL", "price": 0.0}"#, "Zero price not allowed"),
            (r#"{"symbol": "AAPL", "price": 150.0, "volume": -1}"#, "Negative volume not allowed"),
            (r#"{"symbol": "", "price": 150.0}"#, "Empty symbol not allowed"),
            (r#"{"symbol": "TOOLONGSYMBOLNAME", "price": 150.0}"#, "Symbol too long"),
        ];
        
        let dlq = setup_test_dlq();
        let staging = DataStagingService::new_with_dlq(dlq.clone());
        
        for (invalid_json, expected_error) in test_cases {
            let result = staging.transform(invalid_json);
            assert!(result.is_err(), "Should reject: {}", invalid_json);
            
            let error = result.unwrap_err();
            assert!(error.message().contains(expected_error.split_whitespace().next().unwrap()));
        }
        
        assert_eq!(dlq.count(), 5);
    }

    #[test]
    fn test_ensure_no_json_to_eventbus() {
        let eventbus = setup_test_eventbus();
        let staging = DataStagingService::new_with_eventbus(eventbus.clone());
        
        // Attempt to publish raw JSON to EventBus
        let raw_json = r#"{"test": "data"}"#;
        let result = eventbus.publish_raw("market_data", raw_json);
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Proto required");
        
        // Verify EventBus rejects non-protobuf data
        let json_bytes = raw_json.as_bytes();
        let publish_result = eventbus.publish_bytes("market_data", json_bytes);
        
        assert!(publish_result.is_err());
        assert!(publish_result.unwrap_err().to_string().contains("Invalid protobuf"));
        
        // Verify no messages were published
        let messages = eventbus.consume_all("market_data");
        assert!(messages.is_empty());
    }

    #[test]
    fn test_proto_only_validation() {
        let eventbus = setup_test_eventbus();
        
        // Valid protobuf should be accepted
        let valid_proto = MarketData {
            symbol: "AAPL".to_string(),
            price: 150.25,
            volume: 1000,
            timestamp: 1640995200000,
        };
        let proto_bytes = valid_proto.encode_to_vec();
        
        let result = eventbus.publish_proto("market_data", &proto_bytes);
        assert!(result.is_ok());
        
        // Raw bytes should be rejected
        let raw_bytes = vec![0x01, 0x02, 0x03, 0x04];
        let result = eventbus.publish_proto("market_data", &raw_bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid protobuf"));
        
        // JSON bytes should be rejected
        let json_bytes = r#"{"symbol": "AAPL"}"#.as_bytes();
        let result = eventbus.publish_proto("market_data", json_bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid protobuf"));
    }

    #[test]
    fn test_schema_version_enforcement() {
        let staging = DataStagingService::new();
        
        // Test with deprecated v1 schema
        let v1_json = r#"{
            "stock_symbol": "AAPL",
            "stock_price": "150.25",
            "trade_volume": 1000
        }"#;
        
        let result = staging.transform_v1_format(v1_json);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::DeprecatedSchema);
        
        // Test with unknown fields
        let unknown_fields_json = r#"{
            "symbol": "AAPL",
            "price": 150.25,
            "unknown_field": "should_be_ignored",
            "deprecated_field": "should_cause_error"
        }"#;
        
        let result = staging.transform_strict(unknown_fields_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("unknown_field"));
    }

    #[test]
    fn test_concurrent_validation() {
        use std::sync::Arc;
        use tokio::task::JoinSet;
        
        let staging = Arc::new(DataStagingService::new());
        let dlq = Arc::new(setup_test_dlq());
        
        let mut join_set = JoinSet::new();
        
        // Spawn 100 concurrent validation tasks
        for i in 0..100 {
            let staging_clone = staging.clone();
            let dlq_clone = dlq.clone();
            
            join_set.spawn(async move {
                let json = if i % 3 == 0 {
                    // 1/3 invalid data
                    r#"{"invalid": "data"}"#
                } else {
                    // 2/3 valid data
                    &format!(r#"{{"symbol": "AAPL{}", "price": {}.0, "volume": {}, "timestamp": 1640995200000}}"#, i, 150 + i, 1000 + i)
                };
                
                staging_clone.transform(json)
            });
        }
        
        let mut valid_count = 0;
        let mut invalid_count = 0;
        
        while let Some(result) = join_set.join_next().await {
            match result.unwrap() {
                Ok(_) => valid_count += 1,
                Err(_) => invalid_count += 1,
            }
        }
        
        assert_eq!(valid_count, 67); // ~2/3 of 100
        assert_eq!(invalid_count, 33); // ~1/3 of 100
        assert_eq!(dlq.count(), 33);
    }
}
```

### 9.4 Performance Requirements
```rust
#[cfg(test)]
mod data_staging_performance_tests {
    use super::*;
    use criterion::Criterion;

    // Process 10,000 msgs/sec minimum
    #[tokio::test]
    async fn test_throughput_requirement() {
        let staging_service = DataStagingService::new();
        let test_json = r#"{"symbol": "AAPL", "price": 150.25, "volume": 1000, "timestamp": 1640995200000}"#;
        
        let start = Instant::now();
        let mut successful_transformations = 0;
        
        // Process for 1 second
        while start.elapsed() < Duration::from_secs(1) {
            let result = staging_service.transform(test_json);
            if result.is_ok() {
                successful_transformations += 1;
            }
        }
        
        // Should process at least 10,000 messages per second
        assert!(successful_transformations >= 10_000, 
               "Processed only {} msgs/sec, required: 10,000", successful_transformations);
    }

    // Proto conversion < 1ms per message
    #[test]
    fn test_proto_conversion_latency() {
        let staging_service = DataStagingService::new();
        let test_json = r#"{"symbol": "AAPL", "price": 150.25, "volume": 1000, "timestamp": 1640995200000}"#;
        
        // Warm up
        for _ in 0..100 {
            staging_service.transform(test_json).unwrap();
        }
        
        // Measure 1000 conversions
        let start = Instant::now();
        for _ in 0..1000 {
            staging_service.transform(test_json).unwrap();
        }
        let duration = start.elapsed();
        
        let avg_latency = duration / 1000;
        assert!(avg_latency < Duration::from_millis(1), 
               "Average conversion latency: {:?}, required: <1ms", avg_latency);
    }

    // Quality scoring < 0.5ms per message
    #[test]
    fn test_quality_scoring_latency() {
        let quality_scorer = QualityScorer::new();
        let test_data = create_test_market_data_json();
        
        // Warm up
        for _ in 0..100 {
            quality_scorer.calculate_score(&test_data);
        }
        
        // Measure 1000 quality calculations
        let start = Instant::now();
        for _ in 0..1000 {
            quality_scorer.calculate_score(&test_data);
        }
        let duration = start.elapsed();
        
        let avg_latency = duration / 1000;
        assert!(avg_latency < Duration::from_micros(500), 
               "Average quality scoring latency: {:?}, required: <0.5ms", avg_latency);
    }

    // End-to-end latency < 10ms
    #[tokio::test]
    async fn test_end_to_end_latency() {
        let redis_client = setup_test_redis().await;
        let staging_service = setup_staging_service().await;
        let eventbus_consumer = setup_eventbus_consumer("market_data_proto").await;
        
        let test_json = r#"{"symbol": "AAPL", "price": 150.25, "volume": 1000, "timestamp": 1640995200000}"#;
        
        // Measure end-to-end latency: Redis -> Staging -> EventBus
        let start = Instant::now();
        
        // Publish to Redis
        redis_client.publish("market_data_raw", test_json).await.unwrap();
        
        // Wait for message to appear on EventBus
        let message = eventbus_consumer.wait_for_message(Duration::from_secs(1)).await.unwrap();
        
        let end_to_end_latency = start.elapsed();
        
        // Verify it's the correct message
        let proto = MarketData::decode(&message.payload).unwrap();
        assert_eq!(proto.symbol, "AAPL");
        
        // Verify latency requirement
        assert!(end_to_end_latency < Duration::from_millis(10), 
               "End-to-end latency: {:?}, required: <10ms", end_to_end_latency);
    }

    #[test]
    fn test_memory_usage_efficiency() {
        let staging_service = DataStagingService::new();
        let initial_memory = get_process_memory_usage();
        
        // Process 10,000 messages
        let test_json = r#"{"symbol": "AAPL", "price": 150.25, "volume": 1000, "timestamp": 1640995200000}"#;
        for _ in 0..10_000 {
            staging_service.transform(test_json).unwrap();
        }
        
        // Force garbage collection
        std::hint::black_box(staging_service);
        
        let final_memory = get_process_memory_usage();
        let memory_increase = final_memory - initial_memory;
        
        // Should not increase memory by more than 50MB for 10k messages
        assert!(memory_increase < 50 * 1024 * 1024, 
               "Memory increase: {}MB, should be <50MB", memory_increase / (1024 * 1024));
    }

    #[tokio::test]
    async fn test_concurrent_processing_performance() {
        let staging_service = Arc::new(DataStagingService::new());
        let test_json = r#"{"symbol": "AAPL", "price": 150.25, "volume": 1000, "timestamp": 1640995200000}"#;
        
        let start = Instant::now();
        let mut handles = vec![];
        
        // Spawn 100 concurrent processing tasks
        for _ in 0..100 {
            let service = staging_service.clone();
            let json = test_json.to_string();
            
            let handle = tokio::spawn(async move {
                let mut count = 0;
                // Each task processes for 1 second
                let task_start = Instant::now();
                while task_start.elapsed() < Duration::from_secs(1) {
                    if service.transform(&json).is_ok() {
                        count += 1;
                    }
                }
                count
            });
            
            handles.push(handle);
        }
        
        let mut total_processed = 0;
        for handle in handles {
            total_processed += handle.await.unwrap();
        }
        
        let total_duration = start.elapsed();
        let throughput = total_processed as f64 / total_duration.as_secs_f64();
        
        // Should maintain high throughput under concurrent load
        assert!(throughput >= 100_000.0, 
               "Concurrent throughput: {:.0} msgs/sec, required: >=100,000", throughput);
    }
}
```

### 6. Integration Tests

#### 6.1 End-to-End Proto Flow
```rust
#[test]
fn test_complete_proto_pipeline() {
    // Create -> Validate -> Process -> Store -> Retrieve
    let original_proto = create_market_data_proto();
    let encoded = original_proto.encode_to_vec();
    
    // Validation
    let validated = validate_proto_message(&encoded).unwrap();
    
    // Processing
    let processed = process_market_data(&validated).unwrap();
    
    // Storage
    store_proto_message(&processed).unwrap();
    
    // Retrieval
    let retrieved = retrieve_proto_message(&processed.id).unwrap();
    
    assert_eq!(original_proto, retrieved);
}

#[test]
fn test_multi_service_proto_communication() {
    let services = vec!["market_data", "risk_engine", "portfolio_manager"];
    let proto_message = create_trade_signal_proto();
    
    for service in services {
        let response = send_proto_to_service(service, &proto_message).unwrap();
        
        // All services should only accept and return proto
        assert!(response.is_valid_proto());
        assert!(!response.contains_legacy_format());
    }
}
```

## Test Execution Strategy

### 1. Continuous Integration
```yaml
# CI Pipeline for Proto-Only Testing
proto_validation_tests:
  - Unit tests (proto validation)
  - Schema compliance tests
  - Contract enforcement tests
  - Negative testing suite
  - Performance benchmarks
  
failure_conditions:
  - Any non-proto data accepted
  - Schema validation bypassed
  - Legacy format fallback detected
  - Performance degradation > 5%
```

### 2. Test Data Generation
```rust
// Generate comprehensive test data
fn generate_proto_test_suite() -> Vec<TestCase> {
    vec![
        // Valid cases
        TestCase::valid_proto(create_market_data_proto()),
        TestCase::valid_proto(create_trade_signal_proto()),
        
        // Invalid cases  
        TestCase::invalid_raw_bytes(vec![1, 2, 3]),
        TestCase::invalid_json_bytes(json_to_bytes()),
        TestCase::invalid_malformed_proto(corrupt_proto()),
        
        // Edge cases
        TestCase::edge_case_empty_message(),
        TestCase::edge_case_oversized_message(),
    ]
}
```

### 3. Monitoring and Metrics

```rust
#[test]
fn test_validation_metrics_collection() {
    let metrics = ValidationMetrics::new();
    
    // Process mixed valid/invalid data
    process_test_data_with_metrics(&metrics);
    
    // Verify metrics
    assert_eq!(metrics.valid_proto_count, 1000);
    assert_eq!(metrics.rejected_non_proto_count, 500);
    assert_eq!(metrics.rejection_rate, 0.33);
    assert!(metrics.average_validation_time < Duration::from_micros(100));
}
```

## Success Criteria

### ✅ Must Pass
- All non-proto data is rejected (100% rejection rate)
- All valid proto data is accepted (100% acceptance rate)
- Schema validation enforced without exceptions
- Performance targets met (>10k validations/sec)
- Zero security vulnerabilities in validation logic

### ❌ Must Fail
- Any Vec<u8> that's not valid protobuf is accepted
- Legacy format compatibility detected
- Schema validation bypassed
- Performance degradation beyond thresholds
- Memory leaks in validation process

## Test Environment Setup

```rust
// Test configuration - Proto-only mode
#[cfg(test)]
mod test_config {
    use super::*;
    
    pub fn setup_strict_proto_environment() {
        env::set_var("PROTO_ONLY_MODE", "true");
        env::set_var("LEGACY_SUPPORT", "false");
        env::set_var("VALIDATION_LEVEL", "strict");
        env::set_var("COMPATIBILITY_MODE", "false");
    }
    
    pub fn verify_no_legacy_support() {
        assert_eq!(env::var("LEGACY_SUPPORT").unwrap(), "false");
        assert_eq!(env::var("COMPATIBILITY_MODE").unwrap(), "false");
    }
}
```

This testing strategy ensures **zero tolerance** for non-protobuf data and validates that the system maintains strict contract enforcement without any exceptions or fallback mechanisms.