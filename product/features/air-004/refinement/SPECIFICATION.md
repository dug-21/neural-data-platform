# AIR-004: Stream Registry Integration - SPARC Specification

## 1. Problem Statement

### Current State
The air-quality application has a working MQTT ingestion pipeline but uses a **legacy configuration approach**:

- Configuration is loaded from etcd at the path `/air-quality/*` (hardcoded prefix)
- Configuration structure is app-specific and non-extensible (`EtcdAppConfig`)
- No centralized stream registry or metadata management
- Adding new data streams requires duplicating the entire configuration pattern
- No standardized schema definition for data fields
- Storage and ingestion are tightly coupled to app-specific config types

**Existing Implementation:**
- Location: `apps/air-quality-app/src/config_etcd.rs`
- Prefix: `/air-quality` (hardcoded in `load_from_etcd()`)
- Config Type: `EtcdAppConfig` with separate `ServerConfig`, `MqttConfig`, `StorageConfig`
- Usage: Direct field access in `main.rs` lines 24-65

### Desired State
A **stream-centric architecture** where:

- Each data stream (air-quality, weather, etc.) is defined as a `StreamConfig` in etcd
- Stream configurations are stored at `/streams/{stream-id}/config`
- The `StreamRegistry` provides a centralized API for loading stream metadata
- Applications load stream configurations via `StreamRegistry.load_stream("air-quality")`
- Stream configs include schema definitions, source configurations, and storage parameters
- New streams can be added without code changes (configuration-driven)

**Target Implementation:**
- Location: Use existing `config-client/src/stream/registry.rs` (already implemented)
- Prefix: `/streams` (managed by `StreamRegistry`)
- Config Type: `StreamConfig` from `core/src/types/stream_config.rs`
- etcd Path: `/streams/air-quality/config` (standardized pattern)

### Gap Analysis
The `StreamRegistry` and `StreamConfig` types are **already implemented** but **not integrated** with the air-quality app:

| Component | Status | Integration Required |
|-----------|--------|---------------------|
| `StreamRegistry` | ✅ Implemented | Connect to air-quality app |
| `StreamConfig` | ✅ Implemented | Map to MQTT/Storage setup |
| etcd schema | ❌ Missing | Seed `/streams/air-quality/config` |
| App integration | ❌ Missing | Replace `load_from_etcd()` call |
| Migration path | ❌ Missing | Backward compatibility during transition |

## 2. Functional Requirements

### FR-1: Stream Configuration Schema
**Priority:** High
**Description:** Define air-quality as a `StreamConfig` in etcd

**Acceptance Criteria:**
- StreamConfig stored at `/streams/air-quality/config` in etcd
- Schema includes all air-quality data fields (pm25, temperature, humidity, co2, etc.)
- MQTT source configuration includes broker, topic, qos
- Storage configuration includes batch size, timeout, buffer capacity
- Configuration validates successfully using `StreamConfig.validate()`

**Example Configuration:**
```json
{
  "stream_id": "air-quality",
  "description": "AirGradient sensor readings from MQTT",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 7,
  "partitioning_strategy": "daily",
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "description": "Particulate Matter 2.5",
      "range": [0.0, 500.0],
      "display_precision": 1,
      "nullable": false
    },
    {
      "name": "temperature",
      "type": "float",
      "unit": "celsius",
      "range": [-40.0, 60.0],
      "display_precision": 1,
      "nullable": false
    }
  ],
  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "broker_url": "localhost",
      "port": 1883,
      "client_id": "air-quality-app",
      "topic_pattern": "airgradient/readings/+",
      "qos": 1,
      "reconnect_delay_secs": 1,
      "max_reconnect_delay_secs": 30
    }
  ],
  "storage": {
    "batch_size": 100,
    "batch_timeout_secs": 5,
    "buffer_capacity": 1000
  }
}
```

### FR-2: StreamRegistry Integration
**Priority:** High
**Description:** Application loads configuration via StreamRegistry

**Acceptance Criteria:**
- Application creates `StreamRegistry` instance on startup
- Calls `StreamRegistry.load_stream("air-quality")` to fetch configuration
- StreamRegistry connects to etcd at `ETCD_ENDPOINT` (env var)
- Configuration is cached in StreamRegistry for subsequent access
- Errors are handled gracefully (fallback to file config if etcd unavailable)

**Implementation Location:**
- File: `apps/air-quality-app/src/main.rs`
- Replace: Lines 25-65 (current `load_from_etcd()` call)
- Add: StreamRegistry initialization before config loading

### FR-3: MQTT Handler Configuration
**Priority:** High
**Description:** MQTT handler configured from StreamConfig

**Acceptance Criteria:**
- Extract MQTT parameters from `StreamConfig.sources[0]` (where `source_type == Mqtt`)
- Create `neural_core::MqttConfig` from stream source parameters
- Map `broker_url`, `port`, `client_id`, `topic_pattern`, `qos` from source params
- Handle missing parameters with sensible defaults
- Validate QoS value is 0, 1, or 2

**Mapping Logic:**
```rust
// Extract MQTT source from StreamConfig
let mqtt_source = stream_config.sources
    .iter()
    .find(|s| matches!(s.source_type, SourceType::Mqtt))
    .ok_or("No MQTT source found")?;

// Map to MqttConfig
let mqtt_config = MqttConfig {
    broker_url: mqtt_source.params.get("broker_url")?.as_str()?,
    port: mqtt_source.params.get("port")?.as_u64()? as u16,
    client_id: mqtt_source.params.get("client_id")?.as_str()?,
    topic_pattern: mqtt_source.params.get("topic_pattern")?.as_str()?,
    qos: map_qos(mqtt_source.params.get("qos")?.as_u64()?),
    // ... other fields
};
```

### FR-4: Storage Writer Configuration
**Priority:** High
**Description:** StorageWriter configured from StreamConfig

**Acceptance Criteria:**
- Extract storage parameters from `StreamConfig.storage` (or use defaults)
- Create StorageWriter with batch_size, batch_timeout, buffer_capacity
- Base storage path comes from existing `DATA_DIR` or `STORAGE_PATH` env vars
- Stream-specific data written to `{base_path}/{stream_id}/` subdirectory
- WAL enabled based on existing app config (not from StreamConfig)

**Storage Path Structure:**
```
{DATA_DIR}/
  air-quality/
    year=2024/
      month=01/
        day=15/
          data.parquet
  weather/          # Future stream
    year=2024/
      ...
```

### FR-5: Environment Variable Overrides
**Priority:** Medium
**Description:** Environment variables override StreamConfig values

**Acceptance Criteria:**
- `AIR_QUALITY_SERVER_HOST` and `AIR_QUALITY_SERVER_PORT` override server config
- `AIR_QUALITY_MQTT_BROKER_URL` overrides MQTT broker
- Pattern: `{STREAM_ID}_{SECTION}_{KEY}` in SCREAMING_SNAKE_CASE
- Overrides applied after loading StreamConfig but before using values
- Logging indicates when overrides are applied

**Environment Variable Pattern:**
- Stream ID: `air-quality` → Prefix: `AIR_QUALITY_`
- Example: `AIR_QUALITY_MQTT_PORT=1884` overrides `sources[0].params.port`

### FR-6: Backward Compatibility
**Priority:** High
**Description:** Maintain compatibility with legacy `/air-quality` paths during transition

**Acceptance Criteria:**
- If `/streams/air-quality/config` not found, fall back to legacy `load_from_etcd()`
- Application logs which configuration source was used
- Both configuration paths work simultaneously during migration period
- Legacy path will be deprecated in a future release (with warning logs)
- Clear migration path documented

**Fallback Logic:**
```rust
let config = match StreamRegistry::new(&[etcd_endpoint]).await {
    Ok(registry) => {
        match registry.load_stream("air-quality").await {
            Ok(stream_config) => {
                info!("Loaded configuration from /streams/air-quality/config");
                StreamConfig::into_app_config(stream_config)
            }
            Err(ConfigError::NotFound(_)) => {
                warn!("Stream config not found, falling back to legacy /air-quality path");
                load_from_etcd().await?
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(e) => {
        warn!("Failed to initialize StreamRegistry: {}, using legacy config", e);
        load_from_etcd().await?
    }
};
```

## 3. Non-Functional Requirements

### NFR-1: Performance
**Requirement:** Configuration loading must not significantly impact startup time
**Measurement:** Startup time increase < 100ms compared to current implementation
**Justification:** StreamRegistry caches configurations in memory after first load

### NFR-2: Backward Compatibility
**Requirement:** Zero breaking changes during initial deployment
**Measurement:** All existing deployments continue to work without modification
**Implementation:** Fallback to legacy `/air-quality` paths if stream config not found

### NFR-3: Extensibility
**Requirement:** Adding new streams requires only configuration changes, no code changes
**Measurement:** New stream (e.g., "weather") can be added by seeding etcd only
**Validation:** Document process for adding new stream without rebuilding app

### NFR-4: Maintainability
**Requirement:** Configuration schema is validated and self-documenting
**Implementation:**
- StreamConfig validation enforces schema correctness
- Field types, units, and ranges are explicitly declared
- Invalid configurations fail fast at startup with clear error messages

### NFR-5: Reliability
**Requirement:** Configuration errors must not crash the application
**Implementation:**
- Graceful degradation: fall back to legacy config if stream config invalid
- Fallback to file-based config if etcd unavailable
- Degraded mode: run without MQTT ingestion if broker unavailable

### NFR-6: Observability
**Requirement:** Configuration source and values must be logged at startup
**Implementation:**
- Log which configuration method was used (StreamRegistry vs legacy)
- Log all overrides from environment variables
- Log validation warnings (e.g., missing optional fields)

## 4. Acceptance Criteria

### AC-1: Stream Configuration Loaded Successfully
**Given:** A valid StreamConfig exists at `/streams/air-quality/config`
**When:** The application starts
**Then:**
- StreamRegistry successfully loads the configuration
- No errors are logged during configuration loading
- Application logs "Loaded configuration from /streams/air-quality/config"

### AC-2: MQTT Handler Configured from Stream
**Given:** StreamConfig contains an enabled MQTT source
**When:** MQTT handler is initialized
**Then:**
- Handler connects to the configured broker and topic
- QoS level matches configuration
- Reconnect behavior follows configured parameters
- Messages are successfully received and processed

### AC-3: Storage Writer Uses Stream Configuration
**Given:** StreamConfig contains storage parameters
**When:** StorageWriter processes messages
**Then:**
- Batch size matches `storage.batch_size`
- Batch timeout matches `storage.batch_timeout_secs`
- Buffer capacity matches `storage.buffer_capacity`
- Data is written to `{base_path}/air-quality/` subdirectory

### AC-4: Environment Variables Override Config
**Given:** Environment variable `AIR_QUALITY_MQTT_PORT=1884` is set
**And:** StreamConfig specifies `port: 1883`
**When:** Application loads configuration
**Then:**
- MQTT handler connects to port 1884
- Application logs "Override: AIR_QUALITY_MQTT_PORT=1884"

### AC-5: Backward Compatibility Maintained
**Given:** No StreamConfig exists at `/streams/air-quality/config`
**When:** Application starts
**Then:**
- Falls back to legacy `load_from_etcd()` using `/air-quality` prefix
- Application logs "Stream config not found, falling back to legacy path"
- All functionality continues to work as before

### AC-6: New Stream Can Be Added (Extensibility Test)
**Given:** A new `StreamConfig` for "weather" is seeded at `/streams/weather/config`
**When:** A new application instance loads the "weather" stream
**Then:**
- Configuration loads successfully without code changes
- MQTT handler subscribes to weather-specific topics
- Data is written to `{base_path}/weather/` subdirectory
- No application rebuild required

### AC-7: Invalid Configuration Handled Gracefully
**Given:** StreamConfig has invalid field name (e.g., "PM-2.5" instead of "pm25")
**When:** Application attempts to load configuration
**Then:**
- Validation error is logged with clear message
- Application falls back to legacy configuration
- Application does not crash

## 5. Out of Scope

### OS-1: HTTP Polling Source Implementation
**Rationale:** StreamConfig supports `SourceType::HttpPoll`, but no HTTP polling handler exists
**Future Work:** Build HTTP polling source handler when needed (separate feature)
**Current State:** Only `SourceType::Mqtt` is implemented

### OS-2: Webhook Source Implementation
**Rationale:** No webhook receiver infrastructure exists
**Future Work:** Add webhook handler in future if push-based ingestion is required

### OS-3: Dynamic Stream Hot-Reloading
**Rationale:** Current architecture requires application restart for config changes
**Future Work:** Implement etcd watch-based hot-reloading (AIR-005 candidate)
**Current State:** Configuration is loaded once at startup

### OS-4: Multi-Stream Single Application
**Rationale:** Current application architecture is single-stream focused
**Future Work:** Refactor to support multiple streams in one process (AIR-006)
**Current State:** One application instance per stream

### OS-5: StreamConfig UI/Admin Panel
**Rationale:** Configuration is currently managed via etcdctl or scripts
**Future Work:** Build web UI for stream management (separate epic)
**Current State:** Manual etcd configuration via CLI

### OS-6: Field-Level Data Validation
**Rationale:** StreamConfig defines field schemas but app doesn't validate ingested data
**Future Work:** Add data validation pipeline that enforces field types/ranges
**Current State:** Schema is informational only, not enforced during ingestion

### OS-7: Schema Evolution & Versioning
**Rationale:** No mechanism for handling schema changes over time
**Future Work:** Implement schema versioning and migration strategy
**Current State:** `version` field exists but is not used for compatibility checks

## 6. Migration Path

### Phase 1: Parallel Configuration (Week 1)
**Goal:** Both legacy and stream-based configs work simultaneously

**Steps:**
1. Seed initial StreamConfig to `/streams/air-quality/config` in etcd
2. Deploy updated application with StreamRegistry integration
3. Application tries StreamConfig first, falls back to legacy
4. Monitor logs to confirm both paths work

**Success Criteria:**
- Both configuration methods work in production
- No user-facing disruption
- Metrics show which config path is used

**Rollback Plan:**
- Remove `/streams/air-quality/config` from etcd
- Application automatically falls back to legacy path

### Phase 2: Primary Stream Configuration (Week 2-3)
**Goal:** StreamConfig becomes the primary configuration source

**Steps:**
1. Migrate all environment-specific overrides to etcd stream configs
2. Update deployment scripts to seed `/streams/air-quality/config`
3. Validate all environments (dev, staging, prod) using stream config
4. Update documentation to reference stream-based configuration

**Success Criteria:**
- 100% of deployments use `/streams/air-quality/config`
- Legacy path no longer accessed in logs
- All environment variables correctly override stream config

**Rollback Plan:**
- Remove `/streams/air-quality/config`
- Automatic fallback to legacy path

### Phase 3: Deprecate Legacy Path (Week 4+)
**Goal:** Remove legacy configuration code

**Steps:**
1. Add deprecation warnings when legacy path is used
2. Update monitoring to alert on legacy path usage
3. After 2 weeks of zero legacy usage, remove `load_from_etcd()` function
4. Remove `/air-quality` etcd prefix entirely

**Success Criteria:**
- Zero fallback to legacy path in all environments
- Code simplified by removing legacy config loading
- Documentation updated to remove legacy references

**Rollback Plan:**
- Revert code changes to restore legacy path
- Re-add `/air-quality` prefix to etcd

### Phase 4: Multi-Stream Enablement (Future)
**Goal:** Add additional streams (weather, energy, etc.)

**Steps:**
1. Seed new StreamConfig at `/streams/weather/config`
2. Deploy new application instance for weather stream
3. No code changes required (configuration-driven)

**Success Criteria:**
- New stream operational within 1 hour (configuration only)
- No code deployment required for new streams

## 7. Testing Strategy

### London TDD Approach: Outside-In Testing

#### Integration Tests (Outside)
Test the system boundary from the application's perspective:

```rust
#[tokio::test]
async fn test_app_loads_stream_config_from_etcd() {
    // Given: Stream config exists in etcd
    seed_test_stream_config("test-air-quality").await;

    // When: Application loads configuration
    let config = load_config_with_stream_registry().await.unwrap();

    // Then: Configuration matches stream config
    assert_eq!(config.stream_id, "test-air-quality");
    assert!(config.sources.iter().any(|s| matches!(s.source_type, SourceType::Mqtt)));
}

#[tokio::test]
async fn test_app_falls_back_to_legacy_when_stream_missing() {
    // Given: No stream config exists
    // And: Legacy config exists at /air-quality

    // When: Application loads configuration
    let config = load_config_with_stream_registry().await.unwrap();

    // Then: Legacy configuration is used
    assert!(config.mqtt.broker_url.len() > 0);
}
```

#### Unit Tests (Inside)
Test individual components in isolation with mocked dependencies:

```rust
#[tokio::test]
async fn test_stream_config_to_mqtt_config_conversion() {
    // Given: StreamConfig with MQTT source
    let stream_config = create_test_stream_config();

    // When: Converting to MqttConfig
    let mqtt_config = extract_mqtt_config(&stream_config).unwrap();

    // Then: All fields are correctly mapped
    assert_eq!(mqtt_config.broker_url, "localhost");
    assert_eq!(mqtt_config.port, 1883);
    assert_eq!(mqtt_config.qos, QoS::AtLeastOnce);
}

#[test]
fn test_environment_variable_overrides_mqtt_port() {
    // Given: StreamConfig with default port
    // And: Environment variable set
    temp_env::with_var("AIR_QUALITY_MQTT_PORT", Some("1884"), || {
        let config = apply_env_overrides(stream_config);

        // Then: Port is overridden
        assert_eq!(config.mqtt_port(), 1884);
    });
}
```

### Test Coverage Requirements
- **Integration Tests:** 90% coverage of configuration loading paths
- **Unit Tests:** 95% coverage of mapping/conversion logic
- **Edge Cases:** All error paths (missing config, invalid format, etcd down)

### Test Execution
```bash
# Run all tests
cargo test --package air-quality-app --package config-client

# Run only integration tests (requires etcd)
cargo test --package air-quality-app --test '*' -- --include-ignored

# Run unit tests only (no etcd required)
cargo test --package air-quality-app --lib
```

## 8. Implementation Checklist

### Development Tasks
- [ ] Create `StreamConfig` builder/converter in `apps/air-quality-app/src/stream_config_adapter.rs`
- [ ] Add `StreamRegistry` initialization in `main.rs`
- [ ] Implement fallback logic (StreamConfig → legacy → file → defaults)
- [ ] Map `StreamConfig` to `MqttConfig` and `StorageConfig`
- [ ] Implement environment variable override logic
- [ ] Add logging for configuration source and overrides
- [ ] Write integration tests for configuration loading
- [ ] Write unit tests for config conversion and overrides
- [ ] Update error handling for graceful degradation

### Infrastructure Tasks
- [ ] Create etcd seed script for `/streams/air-quality/config`
- [ ] Update deployment scripts to seed stream config before app start
- [ ] Add health check endpoint showing active configuration source
- [ ] Update monitoring to track config load times
- [ ] Create migration runbook for production deployment

### Documentation Tasks
- [ ] Document StreamConfig schema in `docs/stream-config-schema.md`
- [ ] Create migration guide for operators
- [ ] Update `README.md` with new configuration approach
- [ ] Document environment variable override patterns
- [ ] Add examples for creating new streams
- [ ] Update troubleshooting guide with config loading errors

### Validation Tasks
- [ ] Manual test: Start with stream config (verify MQTT connection)
- [ ] Manual test: Start without stream config (verify fallback to legacy)
- [ ] Manual test: Start with invalid stream config (verify graceful failure)
- [ ] Manual test: Apply env var override (verify override logs)
- [ ] Load test: Verify performance impact < 100ms
- [ ] Chaos test: Kill etcd during startup (verify file config fallback)

## 9. Success Metrics

### Technical Metrics
- **Configuration Load Time:** < 100ms (p95)
- **Test Coverage:** > 90% for new code paths
- **Startup Reliability:** 99.9% (with fallback mechanisms)
- **Config Cache Hit Rate:** > 95% (after first load)

### Operational Metrics
- **New Stream Time-to-Deploy:** < 1 hour (config-only, no code changes)
- **Configuration Error Rate:** < 0.1% (validation catches most issues)
- **Fallback Usage:** < 5% after Phase 2 migration

### Business Metrics
- **Developer Velocity:** New streams deployable in 1 day (vs 1 week previously)
- **Operational Complexity:** Reduced (centralized stream registry)
- **Platform Extensibility:** Increased (configuration-driven streams)

## 10. Risks and Mitigations

### Risk 1: etcd Unavailable at Startup
**Impact:** High - Application cannot load configuration
**Probability:** Low
**Mitigation:**
- Multi-level fallback: StreamConfig → legacy → file → defaults
- Cache configuration in file after first successful load
- Health check warns if using fallback config

### Risk 2: Invalid Stream Configuration
**Impact:** Medium - Application fails to start or starts with wrong config
**Probability:** Medium
**Mitigation:**
- Validate StreamConfig before saving to etcd (using CLI tool)
- Application validates on load and falls back if invalid
- Integration tests cover invalid config scenarios

### Risk 3: Configuration Drift Between Environments
**Impact:** Medium - Dev and prod behave differently
**Probability:** Medium
**Mitigation:**
- Store environment-specific overrides in etcd, not in code
- Configuration versioning tracked in etcd
- Automated tests validate config consistency

### Risk 4: Performance Regression
**Impact:** Low - Startup time increases significantly
**Probability:** Low
**Mitigation:**
- StreamRegistry caches configurations
- Benchmark startup time before/after
- Timeout on etcd calls (5 seconds max)

### Risk 5: Breaking Changes During Migration
**Impact:** High - Existing deployments stop working
**Probability:** Low
**Mitigation:**
- Parallel configuration support (both paths work)
- Phased rollout with monitoring
- Immediate rollback plan (remove stream config from etcd)

## 11. Dependencies

### Internal Dependencies
- `config-client` v0.1.0 (already exists) - StreamRegistry implementation
- `neural-core` v0.1.0 (already exists) - StreamConfig type definitions
- `air-quality-app` v0.1.0 - Requires modification

### External Dependencies
- etcd 3.5+ - Stream configuration storage
- Rust 1.75+ - Async/await, tokio
- No new external crates required

### Development Dependencies
- Running etcd instance for integration tests
- Test data seeding scripts
- Docker Compose for local testing

## 12. Appendix: Technical Reference

### StreamConfig JSON Schema
See full schema in `core/src/types/stream_config.rs` (lines 209-246)

### StreamRegistry API
See full API in `config-client/src/stream/registry.rs` (lines 7-165)

### Key Etcd Paths
```
/streams/air-quality/config          # New stream-based config
/air-quality/server/host             # Legacy config (deprecated)
/air-quality/mqtt/broker_url         # Legacy config (deprecated)
```

### Environment Variable Patterns
```bash
# Server configuration
AIR_QUALITY_SERVER_HOST=0.0.0.0
AIR_QUALITY_SERVER_PORT=8080

# MQTT configuration
AIR_QUALITY_MQTT_BROKER_URL=mqtt.example.com
AIR_QUALITY_MQTT_PORT=1883
AIR_QUALITY_MQTT_CLIENT_ID=air-quality-prod
AIR_QUALITY_MQTT_QOS=1

# Storage configuration
DATA_DIR=/app/data
AIR_QUALITY_STORAGE_BATCH_SIZE=200
```

### Example CLI Commands
```bash
# Seed stream config
etcdctl put /streams/air-quality/config "$(cat stream-config.json)"

# Read stream config
etcdctl get /streams/air-quality/config

# List all streams
etcdctl get /streams/ --prefix --keys-only

# Delete stream config (rollback)
etcdctl del /streams/air-quality/config
```

---

**Document Version:** 1.0.0
**Author:** SPARC Specification Agent
**Date:** 2025-12-15
**Status:** Draft for Review
**Next Phase:** Pseudocode (Architecture Design)
