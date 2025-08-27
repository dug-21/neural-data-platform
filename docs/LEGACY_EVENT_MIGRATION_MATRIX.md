# Legacy Event Migration Matrix

## Critical Violations Summary

| Priority | Category | File | Lines | Issue | Action Required |
|----------|----------|------|-------|-------|----------------|
| **CRITICAL** | Core Trait | `src/eventbus/traits/event_bus_v2.rs` | 7,16,19 | Trait methods accept banned `Event` struct | Replace with `ProtoEvent<T>` |
| **CRITICAL** | Re-exports | `src/lib.rs` | 17 | Re-exports deprecated `Event` | Remove from public API |
| **CRITICAL** | Re-exports | `src/eventbus/mod.rs` | 37 | Re-exports deprecated types | Replace with proto types |
| **HIGH** | Example | `examples/eventbus_live_demo.rs` | 15,40,80,84,88,94 | 6x `Event::new()` calls | Convert to `ProtoEvent<T>` |
| **HIGH** | Example | `examples/eventbus_proof.rs` | 26,39,86,100,139 | 5x `Event::new()` calls | Convert to `ProtoEvent<T>` |
| **HIGH** | Test | `tests/eventbus_integration.rs` | 15+ locations | Extensive legacy usage | Full test migration |
| **MEDIUM** | Example | `examples/eventbus_simple_proof.rs` | 14,21,41 | 3x `Event::new()` calls | Convert to examples |
| **MEDIUM** | Example | `examples/working_eventbus_demo.rs` | 25,48 | 2x `Event::new()` calls | Convert to examples |
| **MEDIUM** | Example | `examples/service_integration_example.rs` | 65,82,211 | Service integration examples | Update integration patterns |
| **LOW** | Test Data | `src/eventbus/controllers/*.rs` | Various | Test artifacts with `vec![]` | Clean up test data |

## Event::new Usage Hotspots

### Examples Directory (7 files, 24+ violations)
- `eventbus_live_demo.rs` - **6 violations** (highest count)
- `eventbus_proof.rs` - **5 violations** 
- `eventbus_simple_proof.rs` - **3 violations**
- `working_eventbus_demo.rs` - **2 violations**
- `simple_eventbus_demo.rs` - **1 violation**
- `service_integration_example.rs` - **3 violations**

### Test Directory (2 major files, 30+ violations)
- `tests/eventbus_integration.rs` - **15+ violations** (comprehensive test suite)
- `tests/events_test.rs` - **10+ violations** (event validation tests)

## Phase 4 Enforcement Status

### ✅ ACTIVE Enforcement
- `reject_raw_payload()` function deployed
- InMemoryEventBus rejects all Vec<u8> payloads
- Runtime ContractViolation errors for legacy Event::new()

### ❌ MISSING Enforcement
- Trait definitions still accept `Event` struct
- Public re-exports expose deprecated types
- Import conflicts between old/new types

## Migration Command Reference

### Replace Event::new Patterns
```rust
// OLD (BANNED)
Event::new("MarketData".to_string(), payload_bytes)

// NEW (REQUIRED)
ProtoEvent::new(MarketDataEvent {
    symbol: "AAPL".to_string(),
    price: 150.25,
    volume: 1000.0,
    exchange: "NASDAQ".to_string(),
})
```

### Replace Trait Signatures
```rust
// OLD (BANNED) 
async fn publish(&self, channel: &str, event: Event) -> Result<EventId, EventBusError>;

// NEW (REQUIRED)
async fn publish_proto<T: ProtoMessage>(&self, channel: &str, event: ProtoEvent<T>) -> Result<EventId, EventBusError>;
```

### Replace Library Imports
```rust
// OLD (DEPRECATED)
use neural_core::eventbus::{EventBus, Event};

// NEW (PROTO-ONLY)
use neural_core::eventbus::{ProtoEventBus, ProtoEvent, MarketDataEvent};
```

## Compilation Error Patterns

### Current Warnings (8+)
```
warning: use of deprecated struct `eventbus::types::event::Event`: Use ProtoEvent<T> instead. Vec<u8> payloads are no longer supported.
```

### Runtime Errors (Phase 4 Enforcement)
```
Error: ContractViolation("Contract violation: Only protobuf messages are allowed. Vec<u8> payloads are REJECTED. Use Data-Staging service to convert JSON to proto messages.")
```

## Migration Checklist

### Phase 1: Core Infrastructure ⚠️
- [ ] Update `EventBus` trait to be proto-only
- [ ] Remove deprecated `Event` from lib.rs re-exports  
- [ ] Replace `Event` with `ProtoEvent<T>` in module exports

### Phase 2: Examples & Documentation 📚
- [ ] Convert `eventbus_live_demo.rs` (6 violations)
- [ ] Convert `eventbus_proof.rs` (5 violations)
- [ ] Convert `eventbus_simple_proof.rs` (3 violations)
- [ ] Convert remaining 4 example files
- [ ] Create migration guide showing conversion patterns

### Phase 3: Test Suite Migration 🧪
- [ ] Convert `eventbus_integration.rs` test suite
- [ ] Convert `events_test.rs` validation tests
- [ ] Update all test helper functions
- [ ] Clean up test data artifacts

### Phase 4: Final Cleanup 🧹
- [ ] Remove deprecated `Event` struct entirely
- [ ] Remove legacy trait implementations  
- [ ] Remove enforcement rejection code (no longer needed)
- [ ] Verify all compilation warnings resolved

## Success Metrics

### Pre-Migration
- **Deprecation Warnings**: 8+
- **Legacy Event::new Calls**: 24+ in examples, 30+ in tests
- **Runtime Rejections**: All Vec<u8> payloads fail

### Post-Migration Target
- **Deprecation Warnings**: 0
- **Legacy Event::new Calls**: 0  
- **Proto Event Usage**: 100% adoption
- **Runtime Success**: All proto events accepted

This migration matrix provides a systematic approach to eliminate all legacy Event struct usage and achieve full proto-only compliance in Phase 4.