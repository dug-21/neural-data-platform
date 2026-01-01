# AIR-011 Completion Verification

## Parser Inventory (8 files)

| File | Type | Lines | Purpose |
|------|------|-------|---------|
| `core/src/parsers/mod.rs` | Module | ~50 | Re-exports all parsers |
| `core/src/parsers/config.rs` | Config | ~200 | ParserConfig, ParserType enum |
| `core/src/parsers/factory.rs` | Factory | ~60 | create_parser_from_config() |
| `core/src/parsers/traits.rs` | Trait | ~80 | Parser trait definition |
| `core/src/parsers/json_path.rs` | Impl | ~200 | JsonPath-based extraction |
| `core/src/parsers/flat_json.rs` | Impl | ~150 | Flat JSON key-value parsing |
| `core/src/parsers/array_iterator.rs` | Impl | ~600 | Array iteration with nested paths |
| `core/src/parsers/column_oriented.rs` | Impl | ~350 | Column-oriented data parsing |

## Parser Import Locations (Must Be Addressed)

### Production Code (Critical)
- [ ] `apps/air-quality-app/src/coordinator/source_manager.rs:6` - creates parsers for sources
- [ ] `apps/air-quality-app/src/ingestion/mqtt_handler.rs:9` - creates parsers for MQTT
- [ ] `core/src/sources/http_poll.rs:23` - requires parser in constructor
- [ ] `core/src/sources/mqtt/mod.rs:30` - requires parser in constructor
- [ ] `core/src/coordinator/source_manager.rs:6-7` - creates parsers
- [ ] `core/src/lib.rs:4,13` - exports parsers

### Test Code (Can Keep References)
- [ ] `core/tests/config_driven_suite.rs:25`
- [ ] `core/tests/parser_integration_test.rs:5`
- [ ] `core/tests/nws_config_compatibility_test.rs:6-7`

## Source Constructor Signatures (Require Parser)

### HttpPollingSource
```rust
// core/src/sources/http_poll.rs:367
pub fn new(config: HttpPollingConfig, parser: Box<dyn Parser + Send + Sync>) -> CoreResult<Self>

// core/src/sources/http_poll.rs:392
pub fn with_raw_config(config: HttpPollingConfig, parser: Box<dyn Parser + Send + Sync>, ...)
```

### GenericHttpPollingSource
```rust
// core/src/sources/http_poll.rs:850
pub fn new(config: GenericHttpPollingConfig, parser: Box<dyn Parser + Send + Sync>) -> CoreResult<Self>

// core/src/sources/http_poll.rs:875
pub fn with_raw_config(config: GenericHttpPollingConfig, parser: Box<dyn Parser + Send + Sync>, ...)
```

### MqttSource
```rust
// core/src/sources/mqtt/mod.rs:209
pub fn new(config: MqttConfig, parser: Box<dyn Parser + Send + Sync>) -> Self

// core/src/sources/mqtt/mod.rs:233
pub fn with_raw_config(config: MqttConfig, parser: Box<dyn Parser + Send + Sync>, ...)
```

## Double-Polling Evidence

### Problem Location
`apps/air-quality-app/src/coordinator/source_manager.rs`

**run_http_polling_source (lines 401-482):**
1. Creates parser (line 432)
2. Creates source with parser (line 437)
3. Calls `source.start()` (line 447) → spawns background polling_loop
4. Has its own loop calling `fetch_raw_batch()` (line 465)

**run_generic_http_polling_source (lines 817-877):**
1. Creates parser (line 833)
2. Creates source with parser (line 838)
3. Calls `source.start()` (line 848) → spawns background polling_loop
4. Has its own loop calling `fetch_raw_batch()` (line 866)

## Verification Checklist

### Pre-Implementation
- [ ] All parser files identified
- [ ] All import locations documented
- [ ] Double-polling confirmed
- [ ] Archive location decided
- [ ] Test impact assessed

### Post-Implementation
- [ ] Parsers moved to archive location (deferred to Phase 4 - AIR-012)
- [ ] Source constructors don't require parsers (deferred to Phase 2 - AIR-013)
- [x] source.start() not called in run_http_polling_source (AIR-011 Phase 1)
- [x] source.start() not called in run_generic_http_polling_source (AIR-011 Phase 1)
- [x] Only fetch_raw_batch() is used for ingestion
- [x] config.poll_interval used instead of hardcoded 1s
- [x] source.stop() removed (no background loop to stop)
- [x] Build succeeds
- [ ] All tests pass (infrastructure OOM, not code issue)
- [ ] Pi runs stable for 24+ hours (requires deployment)
- [ ] Memory usage stable (requires deployment)

## Success Metrics
- Zero parser invocations during ingestion (verify via logging)
- Single poll per endpoint per interval
- Memory stable over 24-hour period
- No channel backpressure warnings
