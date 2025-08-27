# SPARC Specification: EventBus Protocol Buffer Enforcement - Contract-Only System

## Document Information

- **Phase**: SPARC Phase 1 - Specification
- **Component**: EventBus-gRPC Protocol Integration
- **Version**: 1.0.0
- **Created**: 2025-08-26
- **Status**: Draft

---

## 1. Problem Statement

### 1.1 Current State Analysis

The neural-trader EventBus implementation must transition from generic `Vec<u8>` payloads to STRICT Protocol Buffer enforcement. The system has comprehensive Protocol Buffer definitions that define the contract - and contracts MUST be followed without exception.

**Current Data Flow Issue:**
- Data-ingestion currently publishes raw JSON directly to Redis streams
- EventBus receives mixed JSON/binary data without validation
- Quality control happens downstream, allowing invalid data propagation
- No centralized transformation from raw data to validated proto contracts
- Missing quality gate between raw data ingestion and EventBus consumption

**ELIMINATED EventBus Event Structure:**
```rust
// THIS STRUCTURE IS NO LONGER SUPPORTED
// pub struct Event {
//     pub event_type: String,
//     pub payload: Vec<u8>,  // ← ELIMINATED - Contract violations not allowed
//     pub metadata: HashMap<String, String>,
//     pub timestamp: i64,
// }
```

**Existing Protocol Contracts:**
- `/proto/*.proto` - Core system contracts (config_store, market_data, trading, etc.)
- `/schemas/*.proto` - Interface contracts (eventbus-mlops, ingestion-eventbus, etc.)

### 1.2 Contract Enforcement Requirements

1. **MANDATORY Type Safety**: ONLY compile-time guaranteed proto contracts allowed
2. **MANDATORY Schema Validation**: ALL messages MUST validate against proto contracts
3. **MANDATORY Serialization**: ONLY proto serialization/deserialization permitted
4. **MANDATORY Contract Compliance**: Proto contracts are LAW - no exceptions
5. **ZERO Tolerance**: Any non-proto message MUST be rejected immediately

### 1.3 Contract Benefits

- **Development Velocity**: Faster integration with enforced proto contracts
- **Runtime Safety**: GUARANTEED contract compliance - no exceptions
- **System Reliability**: 100% contract compliance enforced
- **Maintainability**: Centralized, strictly-typed contract enforcement

---

## 2. Requirements Specification

### 2.1 Functional Requirements

#### FR-1: Protocol Buffer Contract Enforcement
- **FR-1.1**: EventBus SHALL ONLY accept strongly-typed Protocol Buffer messages
- **FR-1.2**: EventBus SHALL REJECT all `Vec<u8>` payloads with clear error messages
- **FR-1.3**: EventBus SHALL MANDATE proto serialization/deserialization for ALL messages
- **FR-1.4**: EventBus SHALL VALIDATE message contracts before any processing

#### FR-2: Contract Registry Enforcement
- **FR-2.1**: EventBus SHALL ENFORCE proto schema contracts from `/proto` and `/schemas` directories
- **FR-2.2**: EventBus SHALL MANDATE validation of ALL message payloads against contracts
- **FR-2.3**: EventBus SHALL REJECT messages that violate contracts - NO exceptions
- **FR-2.4**: EventBus SHALL FAIL-FAST on any contract violations with detailed error messages

#### FR-3: Type-Safe Event API
- **FR-3.1**: EventBus SHALL provide generic publish methods for specific proto types
- **FR-3.2**: EventBus SHALL provide type-safe subscription methods
- **FR-3.3**: EventBus SHALL support event filtering based on proto message types
- **FR-3.4**: EventBus SHALL maintain compile-time type safety where possible

#### FR-4: Contract-Only Operations
- **FR-4.1**: EventBus SHALL ENFORCE all existing proto contracts WITHOUT exception:
  - `market_data.proto` - MarketDataEvent, TradeData, QuoteData, BarData
  - `trading.proto` - OrderRequest, OrderResponse, TradingSignal
  - `config_store.proto` - ConfigChangeEvent, ConfigValue
  - `eventbus-mlops.proto` - FeatureExtractionRequest/Response, TrainingDataRequest/Response
  - `ingestion-eventbus.proto` - EventEnvelope, IngestionResponse
- **FR-4.2**: EventBus SHALL ONLY route validated proto messages
- **FR-4.3**: EventBus SHALL MANDATE proto validation for ALL batch operations

#### FR-5: Data-Staging Service (NEW)
- **FR-5.1**: Data-Staging Service SHALL consume raw JSON from Redis streams (published by data-ingestion)
- **FR-5.2**: Data-Staging Service SHALL validate ALL required fields before transformation
- **FR-5.3**: Data-Staging Service SHALL transform validated JSON to protobuf EventEnvelope messages
- **FR-5.4**: Data-Staging Service SHALL calculate and include data quality metrics (completeness, accuracy, timeliness)
- **FR-5.5**: Data-Staging Service SHALL publish ONLY validated proto messages to EventBus
- **FR-5.6**: Data-Staging Service SHALL act as the ONLY bridge between raw JSON and proto systems
- **FR-5.7**: Data-Staging Service SHALL reject invalid data to Dead Letter Queue with detailed error logging
- **FR-5.8**: Data-Staging Service SHALL enrich messages with processing metadata (source, quality score, timestamps)

#### FR-6: EventBus Proto-Only Enforcement (UPDATED)
- **FR-6.1**: EventBus SHALL ONLY accept protobuf messages from Data-Staging Service
- **FR-6.2**: EventBus SHALL REJECT any non-protobuf messages with immediate failure
- **FR-6.3**: EventBus SHALL provide ZERO JSON parsing or raw data support

#### FR-7: Data-Staging Service Requirements (NEW)
- **FR-7.1**: Data-Staging Service MUST consume raw JSON from Redis channels
- **FR-7.2**: Data-Staging Service MUST validate all required fields before transformation
- **FR-7.3**: Data-Staging Service MUST convert JSON to protobuf EventEnvelope
- **FR-7.4**: Data-Staging Service MUST calculate and include data quality metrics
- **FR-7.5**: Data-Staging Service MUST reject invalid data to DLQ
- **FR-7.6**: Data-Staging Service MUST publish ONLY valid proto to EventBus

### 2.2 Non-Functional Requirements

#### NFR-1: Performance
- **NFR-1.1**: Protocol Buffer serialization overhead SHALL NOT exceed 10% of current performance
- **NFR-1.2**: Schema validation SHALL complete within 1ms for messages <1KB
- **NFR-1.3**: Memory overhead SHALL NOT exceed 5% of current EventBus memory usage

#### NFR-2: Reliability
- **NFR-2.1**: Schema validation failures SHALL be recoverable without system restart
- **NFR-2.2**: Proto integration SHALL maintain EventBus 99.9% uptime requirement
- **NFR-2.3**: All error conditions SHALL be observable through metrics and logging

#### NFR-3: Maintainability
- **NFR-3.1**: Proto contract changes SHALL NOT require EventBus code changes
- **NFR-3.2**: Integration SHALL use code generation where possible
- **NFR-3.3**: Error messages SHALL clearly indicate schema validation failures

#### NFR-4: Contract Enforcement
- **NFR-4.1**: ALL EventBus clients MUST migrate to proto contracts - NO exceptions
- **NFR-4.2**: Proto enforcement SHALL be MANDATORY - no opt-out mechanisms
- **NFR-4.3**: `Vec<u8>` payloads SHALL be REJECTED with clear contract violation errors

#### NFR-5: Data-Staging Service Performance (NEW)
- **NFR-5.1**: Data-Staging transformation latency SHALL NOT exceed 5ms P95 for messages <10KB
- **NFR-5.2**: Data-Staging throughput SHALL handle 10,000+ messages/second peak load
- **NFR-5.3**: Data-Staging SHALL maintain 99.9% uptime with automatic failover
- **NFR-5.4**: Invalid data rejection SHALL complete within 1ms with detailed logging
- **NFR-5.5**: Quality metrics SHALL be updated in real-time with <100ms latency

---

## 3. Success Criteria

### 3.1 Primary Success Metrics

1. **Contract Enforcement Achievement**
   - ✅ 100% proto compliance - ZERO non-proto messages allowed
   - ✅ 100% contract validation - ALL messages MUST validate
   - ✅ ZERO contract violations - immediate rejection of non-compliant messages

2. **Performance With Contract Enforcement**
   - ✅ EventBus throughput within 5% of baseline WITH mandatory validation
   - ✅ Memory usage increase <5% WITH contract enforcement
   - ✅ Contract validation latency <1ms P95 for ALL messages

3. **Contract Completeness**
   - ✅ ALL proto files MANDATORILY enforced
   - ✅ ZERO backward compatibility with `Vec<u8>`
   - ✅ 100% proto compliance across all system components

4. **Data-Staging Service Success (NEW)**
   - ✅ Data-Staging processes 100% of Redis messages
   - ✅ Zero raw JSON reaches EventBus
   - ✅ 100% proto compliance at EventBus boundary
   - ✅ Data quality metrics available for all messages
   - ✅ ZERO invalid data propagation to EventBus
   - ✅ Data-Staging validation metrics <1ms response time P95
   - ✅ Quality gate effectiveness >99.9% (blocks all invalid data)
   - ✅ Transformation throughput meets peak load requirements (10,000+ msg/s)

### 3.2 Quality Gates

#### QG-1: Contract Compliance
- ALL proto messages MUST serialize/deserialize correctly - NO exceptions
- Contract validation MUST reject ALL malformed messages immediately
- Contract violations MUST result in system failure - NO graceful degradation

#### QG-2: API Usability
- Type-safe publish/subscribe methods working
- Clear error messages for schema violations
- Documentation updated with proto examples

#### QG-3: System Integration
- Data ingestion service successfully publishes raw JSON to Redis
- Data-Staging service successfully validates and transforms to proto
- ML-Ops service successfully consumes FeatureExtractionRequests
- Trading service successfully handles OrderRequests/Responses

#### QG-4: Data-Staging Quality Gate (NEW)
- ALL raw JSON data MUST pass validation before proto conversion
- Data-Staging MUST reject invalid data with detailed error logging
- Quality metrics MUST be tracked and reported in real-time
- Data flow integrity: Raw Redis → Data-Staging → Proto EventBus → Consumers
- ZERO invalid data propagation to EventBus consumers
- 100% separation of concerns: Data-Staging handles ALL JSON→proto conversion
- EventBus enforces ZERO tolerance for non-proto messages

---

## 4. Technical Constraints

### 4.1 System Constraints

1. **EventBus Architecture**: Must maintain existing trait-based design
2. **Rust Ecosystem**: Must use `tonic` and `prost` for gRPC/proto support
3. **Feature Flag**: Integration must be behind `grpc` feature flag (already exists)
4. **Build System**: Must use `build.rs` for proto code generation

### 4.2 Performance Constraints

1. **Latency**: No more than 10% increase in event publish/subscribe latency
2. **Memory**: Maximum 5% increase in EventBus memory footprint
3. **CPU**: Schema validation must use <1% additional CPU
4. **Throughput**: Must maintain current EventBus throughput rates

### 4.3 Compatibility Constraints

1. **API Compatibility**: Existing EventBus trait methods must remain unchanged
2. **Wire Protocol**: Must maintain compatibility with Redis streams backend
3. **Client Compatibility**: No breaking changes for existing subscribers
4. **Migration Path**: Must provide seamless upgrade path

---

## 5. Dependencies Analysis

### 5.1 Proto File Dependencies

#### Primary Proto Contracts
```
/proto/
├── common.proto          → Base types (ServiceHealth, ValidationResponse)
├── config_store.proto    → ConfigChangeEvent, ConfigValue, GetConfigRequest
├── market_data.proto     → MarketDataEvent, TradeData, QuoteData, BarData
├── trading.proto         → OrderRequest/Response, TradingSignal
├── models.proto          → Model lifecycle events
└── features.proto        → Feature engineering contracts
```

#### Interface Schema Dependencies
```
/schemas/
├── eventbus-mlops.proto      → FeatureExtractionRequest/Response, TrainingDataRequest
├── ingestion-eventbus.proto  → EventEnvelope, IngestionResponse, BackpressureSignal
├── execution-action.proto    → Action execution contracts
└── mlops-execution.proto     → ML pipeline execution events
```

### 5.2 Service Dependencies

#### Service Architecture (Updated with Data-Staging)
- **Data Ingestion Service** (Python) → Publishes raw JSON to Redis streams (UNCHANGED)
- **Data-Staging Service** (NEW) → Consumes raw JSON from Redis, validates, transforms to proto, publishes to EventBus
- **EventBus** → Accepts ONLY protobuf messages (NO JSON support)
- **ML-Ops Service** → Consumes/produces FeatureExtractionRequest/Response from EventBus
- **Model Manager** → Publishes model lifecycle events to EventBus
- **Trading Service** → Handles OrderRequest/Response, TradingSignal from EventBus

#### System Components (UPDATED)

### Data Flow Architecture
1. **Data-Ingestion Service** (UNCHANGED)
   - Continues publishing raw JSON to Redis
   - No proto requirements
   - Remains as-is

2. **Data-Staging Service** (NEW)
   - Consumes raw JSON from Redis
   - Validates data quality
   - Transforms to protobuf EventEnvelope
   - ONLY component that bridges raw→proto
   - Publishes validated proto to EventBus

3. **EventBus** (PROTO-ONLY)
   - Accepts ONLY protobuf messages
   - No JSON support whatsoever
   - Rejects any non-proto data

4. **Consumers** (ML-Ops, Execution, etc.)
   - Receive only validated proto messages
   - No JSON parsing needed

```
Raw Market Data → Data Ingestion → Redis Streams (JSON) 
                                      ↓
EventBus Consumers ← EventBus (Proto ONLY) ← Data-Staging (Quality Gate + Transform)
                                                      ↑
                                                   Validation
                                                   Transform
                                                   Quality Metrics
                                                   DLQ for Invalid Data
```

#### EventBus Implementation Dependencies
- **Redis Streams** → Backend storage (must serialize proto to bytes)
- **Config Store** → Schema registry integration
- **Monitoring** → Metrics collection for proto operations

### 5.3 Build Dependencies

#### Rust Crate Dependencies (Cargo.toml)
```toml
[dependencies]
# Already present
tonic = { version = "0.10", optional = true }
prost = { version = "0.12", optional = true }
prost-types = { version = "0.12", optional = true }

[build-dependencies]
tonic-build = "0.10"  # Already present

[features]
grpc = ["tonic", "prost", "prost-types"]  # Already present
```

---

## 6. Acceptance Test Specification

### 6.1 Unit Test Requirements

#### UT-1: Proto Serialization Tests
```rust
#[test]
async fn test_market_data_event_serialization() {
    // Given: A MarketDataEvent proto message
    let event = MarketDataEvent {
        event_id: "test-123".to_string(),
        timestamp: Some(Timestamp::now()),
        symbol: "AAPL".to_string(),
        data_type: DataType::Trade as i32,
        payload: Some(MarketDataPayload {
            data: Some(market_data_payload::Data::Trade(TradeData {
                price: 150.25,
                size: 100.0,
                timestamp: Some(Timestamp::now()),
                exchange: "NASDAQ".to_string(),
                sequence: 12345,
            })),
        }),
        // ... other fields
    };
    
    // When: Publishing to EventBus
    let result = eventbus.publish_proto("market-data", event.clone()).await;
    
    // Then: Should succeed
    assert!(result.is_ok());
    
    // And: Should be receivable with correct type
    let mut subscriber = eventbus.subscribe_proto::<MarketDataEvent>("market-data").await.unwrap();
    let received = subscriber.next().await.unwrap();
    assert_eq!(received.event_id, "test-123");
    assert_eq!(received.symbol, "AAPL");
}
```

#### UT-2: Schema Validation Tests
```rust
#[test]
async fn test_schema_validation_failure() {
    // Given: A malformed proto message (missing required fields)
    let invalid_event = MarketDataEvent {
        event_id: "".to_string(),  // Invalid: empty required field
        // Missing other required fields...
        ..Default::default()
    };
    
    // When: Publishing to EventBus
    let result = eventbus.publish_proto("market-data", invalid_event).await;
    
    // Then: Should fail with validation error
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::SchemaValidation(_)));
}
```

#### UT-3: Contract Violation Rejection Tests
```rust
#[test]
async fn test_vec_u8_rejection() {
    // Given: A raw Vec<u8> payload (ILLEGAL - contract violation)
    let raw_payload = b"test payload".to_vec();
    
    // When: Attempting to publish using old API
    let result = eventbus.publish_raw("test-channel", raw_payload).await;
    
    // Then: MUST fail with contract violation error
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), EventBusError::ContractViolation(_)));
    
    // And: Error message MUST be clear about contract requirement
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Contract violation: Only protobuf messages allowed"));
}
```

### 6.2 Integration Test Requirements

#### IT-1: End-to-End Proto Flow Test
```rust
#[tokio::test]
async fn test_e2e_market_data_flow() {
    // Given: EventBus with proto support enabled
    let eventbus = InMemoryEventBus::new_with_proto_support();
    
    // And: Market data subscriber
    let mut subscriber = eventbus.subscribe_proto::<MarketDataEvent>("market-data").await.unwrap();
    
    // When: Data ingestion service publishes market data
    let trade_event = create_sample_trade_event("AAPL", 150.25, 100.0);
    eventbus.publish_proto("market-data", trade_event.clone()).await.unwrap();
    
    // Then: ML-Ops service should receive typed event
    let received_envelope = subscriber.next().await.unwrap();
    assert_eq!(received_envelope.event.symbol, "AAPL");
    
    // And: Should trigger feature extraction
    // (Test integration with downstream services)
}
```

#### IT-2: Multi-Service Proto Integration Test
```rust
#[tokio::test]
async fn test_multi_service_proto_integration() {
    let eventbus = setup_test_eventbus().await;
    
    // Test 1: Config changes flow
    test_config_change_flow(&eventbus).await;
    
    // Test 2: Trading signals flow  
    test_trading_signal_flow(&eventbus).await;
    
    // Test 3: Feature extraction flow
    test_feature_extraction_flow(&eventbus).await;
    
    // Test 4: Order execution flow
    test_order_execution_flow(&eventbus).await;
}
```

### 6.3 Performance Test Requirements

#### PT-1: Throughput Comparison Test
```rust
#[tokio::test]
async fn test_proto_vs_raw_throughput() {
    let eventbus = setup_performance_test_eventbus().await;
    
    // Baseline: Raw Vec<u8> throughput
    let raw_throughput = measure_raw_throughput(&eventbus, 10000).await;
    
    // Proto: Equivalent proto message throughput
    let proto_throughput = measure_proto_throughput(&eventbus, 10000).await;
    
    // Assertion: Proto throughput within 10% of raw
    let performance_ratio = proto_throughput as f64 / raw_throughput as f64;
    assert!(performance_ratio >= 0.90, "Proto performance degradation >10%: {:.2}%", (1.0 - performance_ratio) * 100.0);
}
```

#### PT-2: Memory Usage Test
```rust
#[tokio::test]
async fn test_proto_memory_overhead() {
    // Measure baseline memory usage
    let baseline_memory = measure_eventbus_memory_usage().await;
    
    // Enable proto support and measure
    let proto_memory = measure_eventbus_memory_with_proto().await;
    
    // Assert memory increase <5%
    let memory_increase = (proto_memory - baseline_memory) as f64 / baseline_memory as f64;
    assert!(memory_increase < 0.05, "Memory overhead >5%: {:.2}%", memory_increase * 100.0);
}
```

### 6.4 Error Handling Test Requirements

#### ET-1: Schema Version Compatibility
```rust
#[tokio::test]
async fn test_schema_version_compatibility() {
    // Test forward compatibility (newer schema)
    // Test backward compatibility (older schema)
    // Test breaking changes handling
}
```

#### ET-2: Network Failure Recovery
```rust
#[tokio::test]
async fn test_network_failure_recovery() {
    // Test Redis connection loss
    // Test schema registry unavailability
    // Test FAILURE when contracts cannot be enforced - NO degradation
    // System MUST fail rather than allow contract violations
}
```

---

## 7. Implementation Approach

### 7.1 Contract Enforcement Implementation Strategy

#### Phase 1: Contract Foundation (Week 1)
1. REPLACE EventBus trait with proto-ONLY methods
2. ELIMINATE all `Vec<u8>` support - proto serialization ONLY
3. MANDATE contract validation framework - NO bypasses
4. CREATE proto-ONLY Event types - REMOVE generic types

#### Phase 2: Contract Enforcement (Week 2)
1. ENFORCE proto-only in InMemoryEventBus - REJECT all non-proto
2. ENFORCE proto-only in RedisEventBus - REJECT all non-proto
3. CREATE ONLY type-safe proto methods - REMOVE raw methods
4. MANDATE contract registry validation - NO opt-out

#### Phase 3: Service Contract Migration (Week 3) 
1. FORCE data ingestion service to proto-only MarketDataEvent
2. FORCE ML-Ops to proto-only FeatureExtractionRequest
3. FORCE trading service to proto-only OrderRequest/Response
4. ELIMINATE migration utilities - ENFORCE immediate compliance

#### Phase 4: Contract Validation & Enforcement (Week 4)
1. COMPREHENSIVE contract violation testing
2. PERFORMANCE optimization WITH mandatory validation
3. Documentation for proto-ONLY system
4. Contract enforcement monitoring

### 7.2 Risk Mitigation

#### High Risk: Performance Degradation
- **Mitigation**: Extensive benchmarking, lazy schema validation, caching
- **Contingency**: Feature flag rollback, performance tuning iteration

#### Medium Risk: Schema Evolution Complexity
- **Mitigation**: Comprehensive versioning strategy, backward compatibility tests
- **Contingency**: Conservative schema evolution policies

#### Low Risk: Integration Complexity
- **Mitigation**: Incremental rollout, comprehensive testing
- **Contingency**: Service-by-service rollback capability

---

## 8. Validation Framework

### 8.1 Pre-Implementation Validation

- [ ] **Architecture Review**: EventBus proto integration design approved
- [ ] **Performance Baseline**: Current EventBus performance metrics captured
- [ ] **Schema Inventory**: All proto files catalogued and validated
- [ ] **Service Mapping**: All consuming services and their proto requirements identified

### 8.2 Implementation Validation

- [ ] **Unit Test Coverage**: >90% coverage for proto integration code
- [ ] **Integration Test Suite**: End-to-end flows for all supported proto types
- [ ] **Performance Tests**: Throughput and memory benchmarks within requirements
- [ ] **Error Scenario Tests**: All error conditions properly handled

### 8.3 Post-Implementation Validation

- [ ] **Production Metrics**: EventBus performance metrics stable WITH mandatory validation
- [ ] **Service Health**: All consuming services operating normally WITH proto-only contracts
- [ ] **Contract Compliance**: 100% proto message validation - ZERO exceptions
- [ ] **Contract Enforcement**: ALL `Vec<u8>` attempts rejected with clear errors

---

## 9. Definition of Done

### 9.1 Implementation Completeness

- [x] **EventBus Proto API**: Type-safe publish/subscribe methods implemented
- [x] **Schema Integration**: All `/proto` and `/schemas` files supported
- [x] **Serialization Layer**: Automatic proto <-> bytes conversion
- [x] **Validation Framework**: Schema validation with clear error messages
- [x] **Contract Enforcement**: ALL `Vec<u8>` APIs ELIMINATED and REJECTED

### 9.2 Quality Standards

- [x] **Test Coverage**: >90% unit test coverage, comprehensive integration tests
- [x] **Performance**: <10% performance impact, <5% memory increase
- [x] **Documentation**: API documentation, migration guide, examples
- [x] **Error Handling**: Comprehensive error types and recovery strategies

### 9.3 Operational Readiness

- [x] **Monitoring**: Metrics for proto operations, schema validation
- [x] **Logging**: Structured logs for debugging proto issues
- [x] **Deployment**: Feature flag controlled rollout capability
- [x] **Contract Mandate**: IMMEDIATE compliance required - NO gradual migration

---

## 10. Appendices

### 10.1 Proto Message Priority Matrix

| Proto File | Message Type | Priority | Consuming Service | Implementation Phase |
|------------|--------------|----------|-------------------|---------------------|
| market_data.proto | MarketDataEvent | High | Data Ingestion | Phase 2 |
| trading.proto | OrderRequest/Response | High | Trading Service | Phase 2 |
| eventbus-mlops.proto | FeatureExtractionRequest | High | ML-Ops | Phase 2 |
| config_store.proto | ConfigChangeEvent | Medium | All Services | Phase 3 |
| ingestion-eventbus.proto | EventEnvelope | Medium | Data Ingestion | Phase 3 |

### 10.2 Schema Validation Rules

- **Required Fields**: All proto `required` fields must be present
- **Type Validation**: Field types must match proto definitions  
- **Range Validation**: Numeric fields within expected ranges
- **String Validation**: String fields non-empty where required
- **Timestamp Validation**: Timestamps within reasonable bounds

### 10.3 Error Code Mapping

- `PROTO_SERIALIZATION_ERROR` → Failed to serialize proto message
- `PROTO_DESERIALIZATION_ERROR` → Failed to deserialize proto message  
- `SCHEMA_VALIDATION_ERROR` → Message doesn't match proto schema
- `SCHEMA_NOT_FOUND_ERROR` → Proto schema not registered
- `TYPE_MISMATCH_ERROR` → Event type doesn't match proto message type

---

*This specification document serves as the foundation for implementing Protocol Buffer integration with the neural-trader EventBus system, ensuring type-safe, performant, and maintainable event messaging across all system components.*