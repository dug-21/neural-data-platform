# EventBus Client Integration Guide
## Phase 4 Proto-Only Specification Compliance

**CRITICAL**: All EventBus clients MUST migrate to proto-only messaging. Vec<u8> and JSON are BANNED.

## Overview

This guide documents the complete migration of ALL EventBus clients to the proto-only specification mandated in Phase 4. Every service that interacts with EventBus has been updated to:

1. **REJECT** all Vec<u8> and JSON messages
2. **ENFORCE** protobuf-only communication  
3. **IMPLEMENT** typed message extraction with `decode_payload<T>()`
4. **VALIDATE** proto messages before processing
5. **HANDLE** validation errors gracefully

## Client Services Migrated

### 1. Neural-Trading EventConsumer

**Location**: `/workspaces/neural-trader/neural-trading/src/events/consumer.rs`

**Key Changes**:
- Migrated to `ProtoEventBus` trait
- Implements `DynamicProtoEventSubscriber` for multi-type handling
- Uses typed proto message extraction: `decode_payload<T>()`
- Handles 4 proto message types:
  - `neural_trader.MarketDataEvent`
  - `neural_trader.NeuralPredictionEvent`
  - `neural_trader.TradingSignalEvent` 
  - `neural_trader.RiskAlertEvent`

**Proto Message Processing**:
```rust
// Extract typed proto message using decode_payload<T>()
match event_envelope.proto_type.as_str() {
    "neural_trader.MarketDataEvent" => {
        let market_data = event_envelope.decode_payload::<MarketDataEvent>()?;
        daa_coordinator.process_market_data(market_data).await?;
    },
    // ... other message types
}
```

### 2. Neural-ML-Ops EventPublisher  

**Location**: `/workspaces/neural-trader/neural-ml-ops/src/events/publisher.rs`

**Key Changes**:
- Migrated from generic backends to `ProtoEventBus` integration
- Added proto message type definitions in `proto_types.rs`
- Implements channel routing based on ML event types
- **BANNED** all JSON/raw publishing methods

**Proto Publishing Pattern**:
```rust
// ONLY proto messages accepted
pub async fn publish_proto<T: ProtoMessage>(&self, event: ProtoEvent<T>) -> Result<()> {
    let ml_proto_event = MLProtoEvent::from_proto_event(event)?;
    let channel = ml_proto_event.get_channel(&self.channels);
    self.eventbus.publish_proto(channel, ml_proto_event).await?;
}

// BANNED: JSON publishing
pub async fn publish(&self, _event: serde_json::Value) -> Result<()> {
    Err(anyhow!("JSON publishing is BANNED. Use publish_proto<T>() only."))
}
```

### 3. Data-Staging EventBusPublisher

**Location**: `/workspaces/neural-trader/data-staging/src/eventbus_publisher.rs`

**Key Changes**:
- Enhanced with **ACTIVE REJECTION** of Vec<u8> attempts
- Uses `EnforcedProtoEventBus` that throws `ContractViolation` errors
- Validates `EventEnvelope` proto messages before publishing
- Tracks success/failure metrics for proto publishing

**Enforcement Pattern**:
```rust
struct EnforcedProtoEventBus;

impl EventBusCore for EnforcedProtoEventBus {
    // BANNED: Vec<u8> publishing is REJECTED
    async fn publish(&mut self, _topic: &str, _event: Vec<u8>) -> Result<()> {
        Err(DataStagingError::ContractViolation(
            "Vec<u8> publishing is BANNED. Use ProtoEventBus only."
        ))
    }
}
```

## Proto Message Types Supported

### Neural Trading Messages
```protobuf
message MarketDataEvent {
    string symbol = 1;
    double price = 2;
    double volume = 3;
    int64 timestamp = 4;
}

message NeuralPredictionEvent {
    string model_id = 1;
    string symbol = 2;
    double predicted_price = 3;
    double confidence = 4;
    int64 timestamp = 5;
}

message TradingSignalEvent {
    string signal_id = 1;
    string symbol = 2;
    string action = 3;  // BUY, SELL, HOLD
    double quantity = 4;
    double confidence = 5;
    int64 timestamp = 6;
}

message RiskAlertEvent {
    string alert_id = 1;
    string alert_type = 2;
    string message = 3;
    string severity = 4;
    int64 timestamp = 5;
}
```

### ML Operations Messages
```protobuf
message TrainingStartedEvent {
    string job_id = 1;
    string model_type = 2;
    string dataset_path = 3;
    int64 started_at = 4;
}

message TrainingCompletedEvent {
    string job_id = 1;
    string model_id = 2;
    double final_accuracy = 3;
    int64 training_duration_ms = 4;
    int64 completed_at = 5;
}

message ModelRegisteredEvent {
    string model_id = 1;
    string model_name = 2;
    string version = 3;
    string model_type = 4;
    int64 registered_at = 5;
}
```

## Channel Routing Strategy

### Neural Trading Channels
- `market_data_proto` - Real-time market data events
- `neural_predictions_proto` - ML model predictions  
- `trading_signals_proto` - Buy/sell/hold decisions
- `risk_alerts_proto` - Risk management notifications

### ML Operations Channels  
- `ml_training_proto` - Training lifecycle events
- `ml_inference_proto` - Inference requests/results
- `ml_models_proto` - Model registry events
- `ml_features_proto` - Feature engineering events
- `ml_monitoring_proto` - Metrics and alerts

## Error Handling Patterns

### Contract Violation Errors
All services now return `ContractViolation` errors when Vec<u8> or JSON is attempted:

```rust
match result {
    Err(EventBusError::ContractViolation(msg)) => {
        error!("Proto-only contract violated: {}", msg);
        // Log violation and continue with next message
    }
    Err(EventBusError::SchemaValidation(msg)) => {
        error!("Proto validation failed: {}", msg);
        // NACK the message for retry or DLQ
    }
    Ok(event_id) => {
        debug!("Proto event published successfully: {}", event_id);
        // ACK the message
    }
}
```

### Message Acknowledgment
```rust
// Successful processing
eventbus.ack_proto(channel, consumer_group, &event_id).await?;

// Failed processing  
eventbus.nack_proto(channel, consumer_group, &event_id).await?;
```

## Integration Testing

### Comprehensive Test Suite
**Location**: `/workspaces/neural-trader/data-staging/tests/client_integration_tests.rs`

**Test Coverage**:
1. ✅ Proto-only compliance validation
2. ✅ Vec<u8> rejection enforcement  
3. ✅ JSON rejection enforcement
4. ✅ Typed message extraction verification
5. ✅ Proto validation error handling
6. ✅ End-to-end message flow validation
7. ✅ Consumer acknowledgment patterns
8. ✅ Channel routing verification

### Running Integration Tests
```bash
# Run all client integration tests
cargo test client_integration_tests --package data-staging

# Run specific compliance test
cargo test test_vec_u8_rejection_enforcement --package data-staging

# Run end-to-end flow test
cargo test test_end_to_end_proto_message_flow --package data-staging
```

## Migration Checklist

For any new EventBus clients, ensure:

- [ ] ✅ Uses `ProtoEventBus` trait exclusively
- [ ] ✅ Implements `ProtoMessage` for all message types
- [ ] ✅ Uses `decode_payload<T>()` for typed extraction
- [ ] ✅ Handles `ContractViolation` and `SchemaValidation` errors
- [ ] ✅ Properly routes messages to proto channels
- [ ] ✅ Implements ACK/NACK acknowledgment patterns
- [ ] ✅ Includes integration tests for proto compliance
- [ ] ✅ **REJECTS** all Vec<u8> and JSON attempts
- [ ] ✅ Validates proto messages before processing
- [ ] ✅ Uses consumer groups for reliable processing

## Best Practices

### 1. Proto Message Design
- Keep messages small and focused
- Use semantic versioning for schema evolution
- Include required validation in `validate()` method
- Use clear, descriptive field names

### 2. Error Handling
- Always handle `ContractViolation` errors gracefully
- Log detailed error context for debugging
- Use NACK for retryable validation failures
- Implement circuit breakers for persistent failures

### 3. Performance Optimization
- Use batching for high-volume channels
- Implement proper consumer group scaling
- Monitor channel lag and throughput
- Cache proto message validators

### 4. Monitoring and Observability
- Track proto message validation success rates
- Monitor channel-specific throughput metrics
- Alert on `ContractViolation` error spikes
- Measure end-to-end message processing latency

## Security Considerations

### Message Validation
- All proto messages MUST pass validation before processing
- Reject messages with suspicious or malformed content
- Implement rate limiting per consumer group
- Log security-relevant message patterns

### Access Control
- Use consumer group isolation for service boundaries
- Implement channel-level access controls
- Audit proto message access patterns
- Monitor for unauthorized message publishing

## Future Enhancements

### Planned Improvements
1. **Schema Registry Integration** - Centralized proto schema management
2. **Message Encryption** - End-to-end encryption for sensitive data
3. **Dead Letter Queue** - Enhanced DLQ handling for failed messages
4. **Message Tracing** - Distributed tracing across service boundaries
5. **Auto-scaling** - Dynamic consumer group scaling based on load

### Compatibility Notes
- **Phase 3**: Legacy Vec<u8> support (DEPRECATED)
- **Phase 4**: Proto-only enforcement (CURRENT)
- **Phase 5**: Enhanced schema validation (PLANNED)

## Troubleshooting

### Common Issues

1. **ContractViolation Errors**
   - Cause: Attempting to use Vec<u8> or JSON
   - Solution: Migrate to `ProtoEventBus` and typed messages

2. **SchemaValidation Errors**  
   - Cause: Invalid or malformed proto messages
   - Solution: Fix proto message validation logic

3. **Consumer Group Lag**
   - Cause: Slow message processing or high volume
   - Solution: Scale consumer groups or optimize processing

4. **Channel Routing Issues**
   - Cause: Incorrect channel names or routing logic
   - Solution: Verify channel configuration and message types

### Debug Commands
```bash
# Check EventBus channel info
cargo run --bin debug-eventbus -- channel-info market_data_proto

# Validate proto message schema
cargo run --bin proto-validator -- --message MarketDataEvent

# Monitor consumer group lag
cargo run --bin consumer-monitor -- --group neural-trading
```

## Support and Resources

- **Architecture Documentation**: `/docs/architecture/`
- **Proto Schemas**: `/proto/`  
- **Integration Tests**: `/data-staging/tests/client_integration_tests.rs`
- **Error Handling Examples**: `/examples/eventbus_error_handling.rs`

---

## Summary

✅ **COMPLETED**: ALL EventBus clients have been successfully migrated to proto-only operation

✅ **ENFORCED**: Vec<u8> and JSON publishing is actively rejected across all services

✅ **VALIDATED**: Comprehensive integration tests ensure full compliance

✅ **DOCUMENTED**: Migration patterns and best practices are established

The EventBus client integration is **FULLY COMPLIANT** with the proto-only specification and ready for production deployment.