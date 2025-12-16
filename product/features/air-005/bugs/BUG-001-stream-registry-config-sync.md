# BUG-001: Stream Registry Configuration Sync Gap

**Feature**: AIR-005 - External Data Integration
**Severity**: High
**Status**: Open
**Reported**: 2025-12-16
**Reporter**: Automated Analysis

---

## Summary

Stream configurations defined in GitOps YAML files (`config/base/streams/`) are not being synchronized to etcd, causing the StreamRegistry to find 0 streams. This prevents the SourceManager from discovering and starting external HTTP polling sources (OpenWeatherMap weather/air-quality APIs).

## Symptoms

```
2025-12-16T14:18:39.195040Z  INFO config_client::stream::registry: Found 0 streams
2025-12-16T14:18:39.195136Z  WARN air_quality_server: Multi-stream coordinator not available: No streams configured in registry. External data sources disabled.
```

## Root Cause Analysis

### Two Conflicting Configuration Patterns

1. **Pattern A (stream_integration.rs)** - Used by air-quality app's own config:
   - Loads from flattened etcd keys: `/streams/air-quality/mqtt/broker_url`, etc.
   - Works because app manually populates these keys

2. **Pattern B (StreamRegistry)** - Expected by SourceManager for external streams:
   - Expects full config blob at: `/streams/{stream_id}/config`
   - Parses as `StreamConfig` struct with fields, sources, storage

### Missing Synchronization

The SPARC architecture specified GitOps sync from YAML to etcd, but:
- YAML configs exist: `config/base/streams/outdoor-weather/config.yaml`
- etcd contains: Only `/streams/test-stream/config` (from tests)
- **No sync mechanism** populates etcd from GitOps YAML files

### Code Flow

```
main.rs:161 → initialize_multi_stream_coordinator()
  ↓
main.rs:239 → StreamRegistry::new()
  ↓
main.rs:246 → registry.list_streams()  ← Returns empty vec!
  ↓
main.rs:247-249 → if streams.is_empty() → ERROR
```

## Impact

- **AIR-005 external data integration completely disabled**
- OpenWeatherMap weather API polling not started
- OpenWeatherMap air pollution API polling not started
- Indoor air quality still works (uses separate MQTT pattern)

## Files Affected

| File | Issue |
|------|-------|
| `config-client/src/stream/registry.rs` | Expects `/streams/{id}/config` format |
| `apps/air-quality-app/src/main.rs:234-253` | Coordinator fails on 0 streams |
| `apps/air-quality-app/src/coordinator/source_manager.rs:81-103` | Can't start sources without configs |
| `config/base/streams/outdoor-weather/config.yaml` | Exists but not in etcd |
| `config/base/streams/outdoor-air-quality/config.yaml` | Exists but not in etcd |

## Proposed Solution

### Option A: Startup Config Sync (Recommended)

Add a config sync step to app startup that:
1. Reads YAML files from `config/base/streams/`
2. Validates and converts to `StreamConfig` format
3. Saves each config to etcd via `StreamRegistry::save_stream()`

**Pros**:
- Single source of truth (GitOps YAML)
- Automatic sync on every deployment
- No external tooling required

**Cons**:
- Requires restart to pick up config changes

### Option B: External Sync Script

Create `scripts/sync-stream-configs.sh` that:
1. Runs as init container or pre-start hook
2. Parses YAML files and loads to etcd
3. Can be triggered by CI/CD or GitOps tools

**Pros**:
- Decoupled from app startup
- Can run independently

**Cons**:
- Additional operational complexity
- Potential race conditions

### Option C: etcd Watch + Hot Reload

Implement config watcher that:
1. Watches filesystem for YAML changes
2. Auto-syncs to etcd
3. StreamRegistry already supports watch

**Pros**:
- Real-time config updates
- No restart required

**Cons**:
- More complex implementation
- Volume mount requirements in containers

## Acceptance Criteria

1. [ ] `registry.list_streams()` returns `["outdoor-weather", "outdoor-air-quality"]`
2. [ ] SourceManager successfully starts HTTP polling sources
3. [ ] Logs show: `Starting generic HTTP polling source for stream outdoor-weather`
4. [ ] Weather data points appear in `/data/outdoor-weather/` directory
5. [ ] No `Found 0 streams` warning in logs

## Test Cases (London TDD)

### Unit Tests (Mocked)

```rust
#[tokio::test]
async fn test_config_sync_loads_yaml_configs() {
    // Given: YAML configs exist in config/base/streams/
    // When: ConfigSyncService::sync_all() is called
    // Then: Registry contains outdoor-weather and outdoor-air-quality
}

#[tokio::test]
async fn test_source_manager_discovers_synced_streams() {
    // Given: Registry has outdoor-weather config
    // When: SourceManager::start_all_sources() is called
    // Then: HTTP polling source is spawned for outdoor-weather
}
```

### Integration Tests

```rust
#[tokio::test]
#[ignore] // Requires etcd
async fn test_full_config_sync_to_etcd() {
    // Given: Fresh etcd, YAML configs exist
    // When: App starts with config sync enabled
    // Then: etcd contains /streams/outdoor-weather/config
}
```

## Related Documents

- [AIR-005 Architecture](../architecture/ARCHITECTURE.md) - Section 6.1: etcd integration
- [AIR-005 Completion](../completion/COMPLETION.md) - Section 7.1: Troubleshooting
- [AIR-005 Pseudocode](../pseudocode/PSEUDOCODE.md) - Section 8.1: Config loading

## Resolution Timeline

- **Discovery**: 2025-12-16
- **Analysis Complete**: 2025-12-16
- **Target Fix**: 2025-12-16 (London TDD implementation)

---

## Notes

The original air-quality stream config works because it uses flattened key pattern populated by a different mechanism. This tech debt was identified in the SPARC documentation but the unified approach was not implemented.
