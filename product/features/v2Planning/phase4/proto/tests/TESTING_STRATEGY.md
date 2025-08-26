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