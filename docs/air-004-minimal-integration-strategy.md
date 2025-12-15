# AIR-004: Multi-Stream Integration - Minimal Integration Strategy

**Analysis Date:** 2025-12-15
**Status:** Ready for Implementation
**Risk Level:** LOW

## Executive Summary

This document proposes a **minimal, non-breaking integration** of the StreamRegistry system into the air-quality-app. The goal is to enable future multi-stream support while maintaining 100% backward compatibility with the current single-stream (MQTT air quality) deployment.

## Current State Analysis

### Working Components

1. **Stream Registry** (`config-client/src/stream/registry.rs`)
   - ✅ Fully implemented with load_stream, list_streams, save_stream, delete_stream
   - ✅ Caching mechanism for performance
   - ✅ Validation before save/load
   - ✅ Connected to etcd at `/streams` prefix

2. **StreamConfig Type** (`core/src/types/stream_config.rs`)
   - ✅ Comprehensive schema definition with fields, sources, storage config
   - ✅ Validation for stream_id (kebab-case), field names (snake_case)
   - ✅ Support for MQTT, HTTP Poll, Webhook, FileWatch sources
   - ✅ Field types: Float, Int, String, Bool, Json

3. **Current Deployment** (`apps/air-quality-app/src/main.rs`)
   - ✅ Loads config from etcd at `/air-quality/*` paths
   - ✅ MQTT handler ingesting from AirGradient sensors
   - ✅ ParquetStore writing to `/app/data`
   - ✅ Working in production on Raspberry Pi

### Integration Gap

**The StreamRegistry exists but is NOT integrated into main.rs startup flow.**

Current etcd paths:
- `/air-quality/server/host`, `/air-quality/server/port`
- `/air-quality/mqtt/broker_url`, `/air-quality/mqtt/port`, etc.
- `/air-quality/storage/base_path`, `/air-quality/storage/wal_enabled`, etc.

New multi-stream path (not yet used):
- `/streams/{stream-id}/config` (StreamConfig JSON)

## Minimal Integration Strategy

### Phase 1: Backward-Compatible Integration (This Task)

**Objective:** Load the existing air-quality stream as a StreamConfig without changing any behavior.

#### Changes Required

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

Add after existing config load (around line 66):

```rust
use config_client::StreamRegistry;

// After loading AppConfig, initialize StreamRegistry
let etcd_endpoint = std::env::var("ETCD_ENDPOINT")
    .unwrap_or_else(|_| "http://localhost:2379".to_string());

// Try to load air-quality stream from registry
match StreamRegistry::new(&[&etcd_endpoint]).await {
    Ok(registry) => {
        match registry.load_stream("air-quality").await {
            Ok(stream_config) => {
                tracing::info!(
                    "Loaded air-quality StreamConfig: {} fields, {} sources, version {}",
                    stream_config.fields.len(),
                    stream_config.sources.len(),
                    stream_config.version
                );

                // Validate that StreamConfig matches current schema expectations
                if let Err(e) = stream_config.validate() {
                    tracing::warn!("StreamConfig validation failed: {}", e);
                }

                // Future: Use stream_config to drive handler creation (AIR-005+)
                // For now, just log it to verify registry integration works
            }
            Err(e) => {
                tracing::info!(
                    "No air-quality stream in registry ({}). This is expected for existing deployments.",
                    e
                );
                tracing::info!("Future: Create StreamConfig from current AppConfig for migration");
            }
        }
    }
    Err(e) => {
        tracing::warn!("Failed to initialize StreamRegistry: {}", e);
    }
}
```

**Impact:**
- ✅ Zero breaking changes
- ✅ Existing `/air-quality/*` paths continue to work
- ✅ If `/streams/air-quality/config` exists, it's loaded and validated
- ✅ If it doesn't exist, app continues normally
- ✅ Logs StreamConfig schema for verification

#### Testing Strategy

1. **Existing Deployment (No StreamConfig in etcd)**
   - App starts normally
   - Logs "No air-quality stream in registry"
   - MQTT handler works as before
   - Data writes to ParquetStore as before

2. **New Deployment (With StreamConfig in etcd)**
   - App starts normally
   - Logs StreamConfig details (fields, sources, version)
   - MQTT handler works as before
   - Data writes to ParquetStore as before

3. **Migration Path** (Future task)
   - Create utility to convert AppConfig → StreamConfig
   - Save to etcd at `/streams/air-quality/config`
   - Verify both paths coexist without conflict

### Phase 2: Extensibility Framework (Future)

**Objective:** Enable adding new data sources with minimal code changes.

#### Developer Workflow for Adding a New Stream

1. **Define StreamConfig** (JSON or YAML):

```json
{
  "stream_id": "home-energy",
  "description": "Home energy consumption monitoring",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 30,
  "partitioning_strategy": "daily",
  "fields": [
    {
      "name": "power_watts",
      "type": "float",
      "unit": "watts",
      "range": [0, 10000],
      "display_precision": 1,
      "nullable": false
    },
    {
      "name": "voltage",
      "type": "float",
      "unit": "volts",
      "range": [0, 250],
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
      "topic_pattern": "home/energy/+",
      "qos": 1
    }
  ],
  "storage": {
    "batch_size": 100,
    "batch_timeout_secs": 5,
    "buffer_capacity": 1000
  }
}
```

2. **Save to etcd**:

```bash
# Using etcdctl
etcdctl put /streams/home-energy/config < home-energy-config.json

# Or using config-client API
npx config-client save-stream home-energy home-energy-config.json
```

3. **Auto-Discovery** (Future enhancement):
   - Registry.list_streams() discovers all configured streams
   - Generic handler factory creates MQTT/HTTP/Webhook handlers
   - Automatic ParquetStore schema creation from StreamConfig.fields
   - Automatic API endpoint registration (`/api/streams/{stream-id}/data`)

4. **Stream-Specific Logic** (Optional):
   - If generic handler insufficient, add custom handler
   - Register with stream_id for routing
   - Otherwise, framework handles everything

## Architecture Benefits

### 1. Separation of Concerns

- **Configuration:** etcd (declarative, version-controlled)
- **Schema Management:** StreamConfig (validated, typed)
- **Runtime:** Generic handlers (data-driven, extensible)

### 2. Operational Advantages

- Add new data sources without code deployment
- Update stream schemas via etcd
- Enable/disable streams dynamically
- Centralized configuration management

### 3. Developer Experience

- Clear contract for adding streams (StreamConfig)
- Self-documenting schemas (fields with units, descriptions, ranges)
- Validation catches errors before runtime
- Easy testing with mock StreamConfigs

## etcd Path Coexistence

### Current Paths (Unchanged)
```
/air-quality/server/host = "0.0.0.0"
/air-quality/server/port = 8080
/air-quality/mqtt/broker_url = "10.0.0.100"
/air-quality/mqtt/topic_pattern = "airgradient/readings/+"
/air-quality/storage/base_path = "/app/data"
```

### New Paths (Additive)
```
/streams/air-quality/config = {StreamConfig JSON}
/streams/home-energy/config = {StreamConfig JSON}
/streams/weather/config = {StreamConfig JSON}
```

### Migration Strategy

1. **Phase 1 (This Task):** App reads both paths, logs StreamConfig if exists
2. **Phase 2 (AIR-005):** App prefers StreamConfig, falls back to legacy paths
3. **Phase 3 (AIR-006):** Migration tool converts legacy → StreamConfig
4. **Phase 4 (AIR-007):** Deprecate legacy paths, StreamConfig-only

## Risk Assessment

### Low Risk Factors

✅ **No Breaking Changes:** Existing paths untouched
✅ **Additive Integration:** Registry load is optional
✅ **Graceful Degradation:** Failures logged, app continues
✅ **Tested Components:** StreamRegistry has comprehensive tests
✅ **Rollback Simple:** Remove registry initialization lines

### Mitigation Strategies

1. **Config Conflict:** If both paths exist with different values
   - Log warning with diff
   - Use legacy path for compatibility
   - Document migration path

2. **etcd Unavailable:**
   - Registry init fails gracefully
   - App uses fallback config (YAML or defaults)
   - Same behavior as current

3. **Invalid StreamConfig:**
   - Validation catches errors
   - Log detailed error message
   - Continue with legacy config

## Implementation Checklist

- [ ] Add StreamRegistry initialization in main.rs (3 lines)
- [ ] Test with no StreamConfig (existing deployment scenario)
- [ ] Create sample air-quality StreamConfig JSON
- [ ] Test with StreamConfig present (new deployment scenario)
- [ ] Verify validation errors are logged correctly
- [ ] Document StreamConfig schema for air-quality stream
- [ ] Create migration utility design doc (separate task)

## Future Work (Post-AIR-004)

### AIR-005: Generic Stream Handler Factory
- Auto-create MQTT/HTTP handlers from StreamConfig
- Dynamic ParquetStore schema generation
- Stream-to-handler routing

### AIR-006: Migration Tooling
- CLI tool: `migrate-to-stream-config air-quality`
- Reads legacy etcd paths
- Generates StreamConfig
- Saves to `/streams/{id}/config`

### AIR-007: API Endpoint Auto-Generation
- `/api/streams/{stream-id}/data` (query endpoint)
- `/api/streams/{stream-id}/schema` (metadata endpoint)
- `/api/streams/{stream-id}/health` (status endpoint)

### AIR-008: Stream Monitoring Dashboard
- List all configured streams
- Show enabled/disabled status
- Display field schemas
- Real-time ingestion metrics

## Conclusion

This minimal integration strategy achieves the goal of **making the system ready for multi-stream support** while maintaining **100% backward compatibility** with the existing single-stream deployment.

The changes are:
- ✅ **Minimal:** ~10 lines of code in main.rs
- ✅ **Safe:** No behavior changes, only additive logging
- ✅ **Deployable:** Can ship to production immediately
- ✅ **Extensible:** Foundation for future multi-stream features

**Next Step:** Implement the code changes in main.rs and test both scenarios (with and without StreamConfig).

---

**Memory Location:** `architecture/air-004/integration-strategy`
**Related Docs:**
- `/workspaces/neural-data-platform/config-client/src/stream/registry.rs`
- `/workspaces/neural-data-platform/core/src/types/stream_config.rs`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`
