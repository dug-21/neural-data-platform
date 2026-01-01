# DP-004 BUG: Bronze Layer Stores Parsed Data Instead of Raw Payloads

## Bug Summary

The Bronze layer is storing parsed/reconstructed data instead of the original raw payloads from sources. The `source_manager.rs` file currently:

1. Fetches data from HTTP/MQTT sources
2. Parses it into `TimeSeriesPoint` structures
3. Reconstructs a JSON payload from the parsed values
4. Stores this reconstructed JSON as `raw_payload`

This violates ADR-001's core principle: **"raw_payload is sacred: Exactly what the source sent, byte-for-byte (as JSON)"**

## Root Cause Location

**File**: `apps/air-quality-app/src/coordinator/source_manager.rs`

**Affected Code Sections**:
- Lines 446-459: `run_http_polling_source` - Creates RawDataPoint from parsed TimeSeriesPoint
- Lines 783-796: `run_mqtt_source` - Same pattern
- Lines 859-872: `run_generic_http_polling_source` - Same pattern

**Current (Incorrect) Pattern**:
```rust
// DP-004: Convert TimeSeriesPoint to RawDataPoint for Bronze layer
let raw_point = RawDataPoint::new(
    &source_id,
    serde_json::json!({
        "value": point.value,          // PARSED value
        "location_id": point.location_id,
        "tags": point.tags,
    }),
)
```

**Required (Correct) Pattern**:
```rust
// DP-004: Store raw HTTP response directly
let raw_point = RawDataPoint::new(
    &source_id,
    response_body,  // Original JSON from source
)
```

---

## Implementation Checklist

### Phase 1: Capture Raw Payloads in Sources

- [ ] **1.1 Modify HTTP source to capture raw response**
  - File: `core/src/sources/http_poll.rs`
  - Change: Store raw response body before parsing
  - Add method: `fetch_raw() -> Vec<(String, Value)>` returning (endpoint_id, raw_json) tuples
  - Keep existing `fetch()` for backward compatibility (Silver layer)

- [ ] **1.2 Modify MQTT source to capture raw payload**
  - File: `core/src/sources/mqtt/mod.rs` or `subscription.rs`
  - Change: Store raw MQTT message payload before parsing
  - Add method: `fetch_raw() -> Vec<(String, Value)>` returning (topic, raw_json) tuples

- [ ] **1.3 Update GenericHttpPollingSource for raw capture**
  - File: `core/src/sources/http_poll.rs` (GenericHttpPollingSource)
  - Change: Preserve raw response body alongside parsed data
  - Pattern: Same as 1.1

- [ ] **1.4 Add RawSource trait method (if not exists)**
  - File: `core/src/traits.rs`
  - Verify `RawSource` trait has appropriate method signatures
  - Trait should support fetching raw payloads

### Phase 2: Update source_manager.rs to Send Raw Payloads

- [ ] **2.1 Refactor run_http_polling_source**
  - File: `apps/air-quality-app/src/coordinator/source_manager.rs`
  - Change: Call `source.fetch_raw()` instead of `source.fetch()`
  - Remove: TimeSeriesPoint to JSON reconstruction
  - Direct: Raw response -> RawDataPoint.raw_payload

- [ ] **2.2 Refactor run_mqtt_source**
  - File: `apps/air-quality-app/src/coordinator/source_manager.rs`
  - Change: Use raw MQTT payload directly
  - Remove: Parser invocation in Bronze path
  - Direct: MQTT message payload -> RawDataPoint.raw_payload

- [ ] **2.3 Refactor run_generic_http_polling_source**
  - File: `apps/air-quality-app/src/coordinator/source_manager.rs`
  - Change: Same pattern as 2.1
  - Remove: Parser invocation for Bronze layer

- [ ] **2.4 Remove TimeSeriesPoint serialization from Bronze path**
  - All three run_*_source functions
  - Delete JSON reconstruction: `serde_json::json!({ "value": ..., "location_id": ... })`
  - Use original source payload directly

### Phase 3: Update Tests

- [ ] **3.1 Unit tests for raw payload capture**
  - File: `core/src/sources/http_poll.rs` (test module)
  - Test: HTTP response body preserved exactly
  - Test: Nested JSON structures preserved
  - Test: Non-numeric values (strings, booleans) preserved

- [ ] **3.2 Unit tests for MQTT raw payload**
  - File: `core/src/sources/mqtt/mod.rs` (test module)
  - Test: MQTT message payload preserved exactly
  - Test: Topic information captured correctly

- [ ] **3.3 Integration tests for Bronze layer**
  - File: `apps/air-quality-app/tests/dp004_pipeline_integration.rs`
  - Test: End-to-end raw payload storage
  - Test: Query raw_payload with DuckDB JSON functions
  - Test: Verify original fields accessible

- [ ] **3.4 Update existing source_manager tests**
  - File: `apps/air-quality-app/src/coordinator/source_manager.rs`
  - Update mocks to verify raw payloads
  - Add assertions for payload content

---

## Code Cleanup Checklist

### Identify Parser Usage

- [ ] **4.1 Audit parser invocations**
  - Search: `create_parser_from_config` in source_manager.rs
  - Document: Which parsers are created
  - Determine: Which are still needed for Silver layer

- [ ] **4.2 Document parser locations**
  | Function | Parser Created | Bronze Needed | Silver Needed |
  |----------|---------------|---------------|---------------|
  | run_http_polling_source | FlatJson | NO | YES (future) |
  | run_mqtt_source | FlatJson | NO | YES (future) |
  | run_generic_http_polling_source | From config | NO | YES (future) |

### Remove Unused Parser Invocations

- [ ] **4.3 Remove parser from run_http_polling_source Bronze path**
  - Lines 398-419: Parser creation can be deferred to Silver ETL
  - Keep Source creation (needs parser for internal buffering)
  - Alternatively: Add `fetch_raw()` method that bypasses parser

- [ ] **4.4 Remove parser from run_mqtt_source Bronze path**
  - Lines 740-760: Same pattern
  - Parser only needed for Silver layer extraction

- [ ] **4.5 Remove parser from run_generic_http_polling_source Bronze path**
  - Lines 830-832: Same pattern

- [ ] **4.6 Clean up dead code paths**
  - After refactoring, remove any unused helper functions
  - Remove TimeSeriesPoint construction in Bronze path

---

## Verification Checklist

### Build Verification

- [ ] **5.1 Cargo build succeeds**
  ```bash
  cargo build --all
  ```

- [ ] **5.2 Cargo test passes**
  ```bash
  cargo test --all
  ```

- [ ] **5.3 Cargo clippy clean**
  ```bash
  cargo clippy --all -- -D warnings
  ```

- [ ] **5.4 Cargo fmt check**
  ```bash
  cargo fmt --all -- --check
  ```

### Functional Verification

- [ ] **5.5 Verify raw payload structure in Parquet**
  ```bash
  # After running ingestion
  duckdb -c "
    SELECT
      timestamp,
      source_id,
      raw_payload
    FROM read_parquet('/data/raw/**/*.parquet')
    LIMIT 5;
  "
  ```

- [ ] **5.6 Verify original JSON fields accessible**
  ```bash
  duckdb -c "
    SELECT
      timestamp,
      raw_payload->>'pm02' as pm25,
      raw_payload->>'rco2' as co2,
      raw_payload->>'atmp' as temp
    FROM read_parquet('/data/raw/**/*.parquet')
    WHERE source_id LIKE '%air-quality%'
    LIMIT 5;
  "
  ```

- [ ] **5.7 Verify non-numeric fields preserved**
  ```bash
  duckdb -c "
    SELECT
      raw_payload->>'serialno' as serial,
      raw_payload->>'wifi' as wifi_strength,
      raw_payload->>'firmware' as firmware
    FROM read_parquet('/data/raw/**/*.parquet')
    LIMIT 5;
  "
  ```

- [ ] **5.8 Compare before/after payload structure**
  - Before: `{"value": 12.5, "location_id": "abc123", "tags": {}}`
  - After: `{"pm02": 12.5, "rco2": 450, "serialno": "abc123", "wifi": -55, ...}`

### Deployment Verification

- [ ] **5.9 Deploy to Pi and verify data collection**
  ```bash
  ./deploy/pi/deploy.sh start
  # Wait for data collection
  ./deploy/pi/deploy.sh logs | grep -i "ingestion"
  ```

- [ ] **5.10 Verify Parquet file size reasonable**
  ```bash
  ls -lah /data/raw/**/*.parquet
  ```

---

## Documentation Updates

- [ ] **6.1 Update ADR-001 if needed**
  - File: `product/features/dp-004/architecture/ADR-001-bronze-raw-json-schema.md`
  - Add: Implementation notes section
  - Clarify: Parser relationship to Bronze/Silver layers

- [ ] **6.2 Update STATUS.md**
  - File: `product/features/dp-004/STATUS.md`
  - Add: Bug tracking entry
  - Update: Implementation phase progress

- [ ] **6.3 Update data flow documentation**
  - File: `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` (if exists)
  - Clarify: Raw payload flow vs parsed data flow

- [ ] **6.4 Add troubleshooting section**
  - Document: How to verify raw payloads
  - Document: Common DuckDB queries for Bronze layer

---

## Deployment Steps

### Pre-Deployment

- [ ] **7.1 Build release binary**
  ```bash
  cargo build --release -p air-quality-app
  ```

- [ ] **7.2 Run local integration tests**
  ```bash
  cargo test -p air-quality-app --test dp004_pipeline_integration
  ```

- [ ] **7.3 Backup existing data (if any)**
  ```bash
  # On Pi
  tar -czvf /data/backup-$(date +%Y%m%d).tar.gz /data/raw/
  ```

### Deployment

- [ ] **7.4 Stop running services**
  ```bash
  ./deploy/pi/deploy.sh stop
  ```

- [ ] **7.5 Deploy new binary**
  ```bash
  ./deploy/pi/deploy.sh sync
  ```

- [ ] **7.6 Start services**
  ```bash
  ./deploy/pi/deploy.sh start
  ```

### Post-Deployment

- [ ] **7.7 Verify data ingestion**
  ```bash
  ./deploy/pi/deploy.sh status
  ./deploy/pi/deploy.sh logs | tail -50
  ```

- [ ] **7.8 Check Parquet files being created**
  ```bash
  ls -la /data/raw/air-quality/
  ```

- [ ] **7.9 Verify raw payload content**
  ```bash
  duckdb -c "
    SELECT COUNT(*), MIN(timestamp), MAX(timestamp)
    FROM read_parquet('/data/raw/**/*.parquet');
  "
  ```

- [ ] **7.10 Update Grafana dashboards if needed**
  - Note: DuckDB views may need updates if they assume parsed structure
  - Test existing dashboards still work
  - Update JSON extraction queries if needed

---

## Rollback Plan

If deployment fails:

1. **Stop services**: `./deploy/pi/deploy.sh stop`
2. **Restore previous binary**: `./deploy/pi/deploy.sh restore`
3. **Restore data if needed**: `tar -xzvf /data/backup-*.tar.gz -C /`
4. **Start services**: `./deploy/pi/deploy.sh start`
5. **Verify restoration**: `./deploy/pi/deploy.sh status`

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | - | - | - |
| Reviewer | - | - | - |
| Tester | - | - | - |

---

## Related Documents

- [ADR-001: Bronze Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
- [IMPLEMENTATION_CHECKLIST.md](../completion/IMPLEMENTATION_CHECKLIST.md)
- [DEPLOYMENT_PLAN.md](../completion/DEPLOYMENT_PLAN.md)
- [STATUS.md](../STATUS.md)
