# ProtoEvent Integration Plan for Neural-Trading

## Current Status ✅

**Neural-trading client is now building successfully!**

### Fixed Issues:
1. ✅ **Cargo.toml dependency**: Commented out neural-core dependency temporarily
2. ✅ **Mock EventBus**: Created mock types for ProtoEventBus integration
3. ✅ **Library structure**: Added lib.rs for testability
4. ✅ **Consumer implementation**: Updated to use mock ProtoEventBus types
5. ✅ **Predictor methods**: Added predict_trend() method for tests

### Current Architecture:

```rust
// Mock types in consumer.rs until neural-core is fixed
pub trait MockProtoEventBus: Send + Sync {
    fn create_proto_consumer_group(&self, channel: &str, group: &str) -> Result<(), ProtoEventBusError>;
    fn subscribe_dynamic_proto(
        &self,
        channels: &[String],
        proto_types: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn MockDynamicProtoEventSubscriber>, ProtoEventBusError>;
}

// EventConsumer is consuming from these channels:
- "market_data_proto"
- "neural_predictions_proto" 
- "trading_signals_proto"
- "risk_alerts_proto"

// Proto types handled:
- "neural_trader.MarketDataEvent"
- "neural_trader.NeuralPredictionEvent"
- "neural_trader.TradingSignalEvent" 
- "neural_trader.RiskAlertEvent"
```

## Next Steps - ProtoEvent Integration

### Phase 1: Fix Neural-Core ❌ (Blocked)

**Issues preventing neural-core compilation:**
- Proto file generation errors (OUT_DIR environment variable issues)
- EventEnvelope import conflicts
- Trait object compatibility issues with EventBus generics
- ProtoEventMetadata vs HashMap<String, String> conflicts
- Missing proto schema fields

### Phase 2: Enable Neural-Trading Integration (When neural-core is ready)

**Changes needed when neural-core compiles:**

1. **Update Cargo.toml**:
```toml
# Re-enable when neural-core is fixed
neural-core = { path = "../neural-core" }
```

2. **Update consumer.rs**:
```rust
// Replace mock types with real ones
use neural_core::eventbus::{
    traits::{ProtoEventBus, DynamicProtoEventSubscriber},
    types::{ProtoEvent, ProtoMessage, SubscriptionConfig, EventId},
    implementations::ProtoInMemoryEventBus,
    error::EventBusError,
};

// Update struct fields
pub struct EventConsumer {
    eventbus: Arc<dyn ProtoEventBus>,  // Replace MockProtoEventBus
    // ... rest unchanged
}
```

3. **Message Processing**:
```rust
// Add real proto event processing in consumer loop
async fn process_proto_event(&self, event: DynamicProtoEvent) -> Result<()> {
    match event.event_type.as_str() {
        "neural_trader.MarketDataEvent" => {
            let market_event: ProtoEvent<MarketDataEvent> = event.to_proto_event()?;
            self.daa_coordinator.process_market_data(market_event).await?;
        }
        "neural_trader.TradingSignalEvent" => {
            let signal_event: ProtoEvent<TradingSignalEvent> = event.to_proto_event()?;
            self.daa_coordinator.execute_signal(signal_event).await?;
        }
        // ... handle other event types
        _ => warn!("Unknown event type: {}", event.event_type),
    }
    Ok(())
}
```

### Phase 3: Testing & Validation

1. **Update Integration Tests**:
```rust
#[tokio::test]
async fn test_proto_event_consumption() {
    let consumer = EventConsumer::new(
        "redis://localhost:6379".to_string(),
        coordinator,
    ).await.unwrap();
    
    // Test that consumer can handle real ProtoEvents
    consumer.start().await.unwrap();
    
    // Publish test events and verify processing
    // ...
}
```

2. **End-to-End Proto Flow Test**:
```rust
#[tokio::test]
async fn test_market_data_proto_flow() {
    // 1. Publish MarketDataEvent to EventBus
    // 2. Verify neural-trading consumer receives it
    // 3. Check that DAA coordinator processes it
    // 4. Validate trading decisions are made
}
```

## Key Design Decisions

### 1. **Proto-Only Architecture** ✅
- NO Vec<u8> support (Phase 4 mandate)
- ALL events must be protobuf messages
- Dynamic type handling with proto type strings

### 2. **Event Flow** ✅
```
Data Ingestion → ProtoEventBus → Neural-Trading Consumer
                                      ↓
                              DAA Coordinator
                                      ↓
                              Trading Decisions
```

### 3. **Channel Strategy** ✅
- Separate channels for different event types
- Consumer groups for scalability
- Proto type validation for safety

## Current Build Status

### Neural-Trading: ✅ COMPILING
```bash
cd /workspaces/neural-trader/neural-trading
cargo build
# SUCCESS with warnings only
```

### Neural-Core: ❌ BLOCKED
```bash
# Multiple compilation errors:
# - Proto generation issues
# - EventEnvelope conflicts  
# - Trait compatibility problems
```

## Immediate Action Items

### For Neural-Core Team:
1. **Fix proto generation**: Resolve OUT_DIR environment variable issues
2. **Fix EventEnvelope imports**: Resolve circular dependency
3. **Fix trait compatibility**: Make EventBus dyn-compatible or redesign
4. **Fix metadata types**: Standardize ProtoEventMetadata vs HashMap

### For Neural-Trading:
1. **Keep current mock architecture** until neural-core is ready
2. **Document event processing patterns** for future integration
3. **Prepare test scenarios** for when ProtoEvent integration is possible

## Success Metrics

✅ **Phase 1 Complete**: Neural-trading builds successfully with mock EventBus
⏳ **Phase 2 Pending**: Neural-core compilation fixes
⏳ **Phase 3 Pending**: Full ProtoEvent integration working
⏳ **Phase 4 Pending**: End-to-end proto event flow tested

---

**Status**: Neural-trading is ready for ProtoEvent integration pending neural-core fixes.
**Next Blocker**: Neural-core compilation issues must be resolved first.