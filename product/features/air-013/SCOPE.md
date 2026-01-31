# air-013: Unified Config Source for Silver ETL

## Problem Statement

Silver ETL configuration (`silver_etl`) is loaded from YAML files directly, while Bronze configuration (`StreamConfig`) is loaded from etcd. This inconsistency causes silent failures and deployment complexity.

**Discovered during air-012 debugging:**

1. `list_streams()` queries etcd for stream IDs
2. For each stream, `load_silver_etl_config()` reads YAML files directly
3. If etcd sync fails (e.g., validation error), the stream isn't in `list_streams()`
4. Silver ETL never runs for that stream - even though the YAML file exists

**The failure mode:**
```
YAML config exists with valid silver_etl section
        ↓
ConfigSyncService.sync_all() fails (validation error in fields section)
        ↓
/streams/{stream_id}/config key NOT created in etcd
        ↓
list_streams() doesn't return the stream
        ↓
load_silver_etl_config() never called
        ↓
SilverSubscriber not created - SILENT FAILURE
```

---

## Current Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      YAML Config File                        │
│  ┌─────────────────────┐  ┌──────────────────────────────┐  │
│  │ fields, sources,    │  │ silver_etl:                  │  │
│  │ storage, etc.       │  │   target_table, field_mappings│  │
│  └──────────┬──────────┘  └──────────────┬───────────────┘  │
└─────────────┼────────────────────────────┼──────────────────┘
              │                            │
              ▼                            │
┌─────────────────────────┐                │
│ ConfigSyncService       │                │
│ sync_all() → etcd       │                │
└──────────┬──────────────┘                │
           │                               │
           ▼                               │
┌─────────────────────────┐                │
│ etcd: StreamConfig      │                │
│ (NO silver_etl!)        │                │
└──────────┬──────────────┘                │
           │                               │
           ▼                               ▼
┌─────────────────────────┐  ┌─────────────────────────────┐
│ list_streams()          │  │ load_silver_etl_config()    │
│ queries etcd            │  │ reads YAML directly         │
└──────────┬──────────────┘  └──────────────┬──────────────┘
           │                               │
           └───────────┬───────────────────┘
                       ▼
              ┌─────────────────┐
              │ SilverSubscriber │
              │ (if both work)   │
              └─────────────────┘
```

**Problems:**
1. Two sources of truth (etcd + YAML)
2. Silent failure when they disagree
3. YAML files must be mounted into containers
4. etcd sync failure blocks Silver ETL even when YAML is valid

---

## Proposed Solution

**Add `silver_etl` to `StreamConfig` and store in etcd.**

```
┌─────────────────────────────────────────────────────────────┐
│                      YAML Config File                        │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ fields, sources, storage, silver_etl (all in one)       ││
│  └──────────────────────────┬──────────────────────────────┘│
└─────────────────────────────┼───────────────────────────────┘
                              │
                              ▼
               ┌─────────────────────────┐
               │ ConfigSyncService       │
               │ sync_all() → etcd       │
               └──────────┬──────────────┘
                          │
                          ▼
               ┌─────────────────────────┐
               │ etcd: StreamConfig      │
               │ (includes silver_etl!)  │
               └──────────┬──────────────┘
                          │
           ┌──────────────┴──────────────┐
           ▼                             ▼
┌─────────────────────┐      ┌─────────────────────────┐
│ Bronze (RawSource)  │      │ Silver (SilverSubscriber)│
│ reads from etcd     │      │ reads from etcd          │
└─────────────────────┘      └─────────────────────────┘
```

**Benefits:**
1. Single source of truth (etcd)
2. Consistent failure modes
3. No YAML file dependency at runtime
4. Config changes via etcd API possible

---

## Implementation Plan

### Phase 1: Extend StreamConfig

**File: `core/src/types/stream_config.rs`**

Add `silver_etl` field to `StreamConfig`:
```rust
pub struct StreamConfig {
    // ... existing fields ...

    /// Silver ETL configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silver_etl: Option<SilverEtlConfig>,
}
```

### Phase 2: Update ConfigSyncService

**File: `apps/air-quality-app/src/config_sync/service.rs`**

Update `StreamConfigYaml.to_stream_config()` to include `silver_etl`:
```rust
fn to_stream_config(&self) -> Result<StreamConfig, ConfigSyncError> {
    // ... existing field conversion ...

    // Extract silver_etl from extra
    let silver_etl = self.extra.get("silver_etl")
        .and_then(|v| serde_yaml::from_value(v.clone()).ok());

    Ok(StreamConfig {
        // ... existing fields ...
        silver_etl,
    })
}
```

### Phase 3: Update SilverSubscriber Creation

**File: `apps/air-quality-app/src/main.rs`**

Replace `load_silver_etl_config()` with etcd lookup:
```rust
async fn create_silver_subscribers(...) {
    let streams = registry.list_streams().await?;

    for stream_id in streams {
        // Load full config from etcd (includes silver_etl)
        if let Ok(config) = registry.load_stream(&stream_id).await {
            if let Some(silver_etl) = config.silver_etl {
                if silver_etl.enabled {
                    // Create SilverSubscriber
                }
            }
        }
    }
}
```

### Phase 4: Remove YAML Dependency

- Remove `load_silver_etl_config()` function
- Remove YAML file mounts from docker-compose.yml (for Silver ETL)
- Update documentation

---

## Migration

**Backward compatible** - existing configs without `silver_etl` continue to work (Bronze only).

**Migration steps:**
1. Deploy new code
2. Restart app - `ConfigSyncService.sync_all()` syncs `silver_etl` to etcd
3. SilverSubscribers now load from etcd

---

## Out of Scope

- Changing Bronze layer to read from YAML (opposite direction)
- etcd watch for live config updates
- Config validation beyond current checks

---

## Success Criteria

1. `silver_etl` stored in etcd as part of `StreamConfig`
2. `SilverSubscriber` loads config from etcd, not YAML files
3. No silent failures - if config sync fails, it's logged and visible
4. Existing streams continue working without config changes

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| `SilverEtlConfig` | Exists | In `core/src/config/silver_etl.rs` |
| `StreamConfig` | Exists | Needs `silver_etl` field added |
| `ConfigSyncService` | Exists | Needs to extract `silver_etl` |

---

## Estimated Effort

- Phase 1: ~30 min (add field to struct)
- Phase 2: ~1 hour (update sync service)
- Phase 3: ~1 hour (update main.rs)
- Phase 4: ~30 min (cleanup)
- Testing: ~1 hour

**Total: ~4 hours**

---

## References

- **air-012**: Exposed this inconsistency during Home Assistant integration debugging
- **dp-015**: Related - config-driven Silver table creation
- **dp-006**: Defines `SilverEtlConfig` format
