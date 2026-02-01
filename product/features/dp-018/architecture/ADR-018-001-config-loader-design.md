# ADR-018-001: Extend StreamConfig with silver_etl

**Status**: Proposed
**Date**: 2026-02-01
**Decision Makers**: NDP Architecture Team
**Feature**: dp-018 JSON Config Foundation

---

## Context

The NDP already has a well-designed configuration system:

- **config-client crate** provides `StreamRegistry` for etcd-backed config
- **StreamRegistry** loads `StreamConfig` with caching
- **Bronze layer** uses StreamRegistry correctly

The problem is that `StreamConfig` doesn't include `silver_etl`, so Silver ETL loads from YAML files directly. This creates a dual source of truth.

### Current Architecture

```
config-client/
├── ConfigClient       # Low-level etcd wrapper
└── StreamRegistry     # Loads StreamConfig from etcd (with cache)

core/src/types/
└── StreamConfig       # stream_id, fields, sources, storage
                       # ❌ NO silver_etl field

apps/air-quality-app/
├── SourceManager      # Uses StreamRegistry ✅
├── ConfigSyncService  # Syncs YAML → etcd, but DISCARDS silver_etl ❌
└── load_silver_etl_config()  # Reads YAML files directly ❌
```

### The Bug

In `ConfigSyncService.to_stream_config()`:
- YAML `silver_etl` section is captured via `#[serde(flatten)]` into `extra` HashMap
- But `to_stream_config()` ignores `extra` - silver_etl is never synced to etcd
- Silver subscriber calls `load_silver_etl_config()` which reads YAML files
- If YAML file is missing/stale, Silver silently fails

---

## Decision

**Add `silver_etl: Option<SilverEtlConfig>` to StreamConfig.**

This is the simplest fix:
1. StreamConfig gains the silver_etl field
2. ConfigSyncService syncs the complete config (including silver_etl)
3. Silver subscriber uses `StreamRegistry.load_stream()` like Bronze does
4. No new traits, no new registries, no new abstractions

### Why NOT a New ConfigLoader Trait?

- StreamRegistry already provides caching, loading, and watching
- Adding a trait creates unnecessary indirection
- config-client already handles etcd connections
- We want consolidation, not more abstractions

---

## Implementation

### 1. Extend StreamConfig (core/src/types/stream_config.rs)

```rust
pub struct StreamConfig {
    pub stream_id: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub retention_days: u32,
    pub compression_after_days: u32,
    pub partitioning_strategy: String,
    pub fields: Vec<SchemaField>,
    pub sources: Vec<SourceConfig>,
    pub storage: Option<StorageConfig>,

    // NEW: Silver ETL configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silver_etl: Option<SilverEtlConfig>,
}
```

### 2. Define SilverEtlConfig (core/src/types/silver_etl.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverEtlConfig {
    pub target_table: String,
    pub timestamp_field: String,
    pub identity_fields: Vec<String>,
    pub field_mappings: Vec<SilverFieldMapping>,
    #[serde(default)]
    pub dq_rules: Vec<DqRule>,
    #[serde(default)]
    pub deduplication: Option<DeduplicationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverFieldMapping {
    pub target_column: String,
    pub source_path: String,
    #[serde(default)]
    pub target_type: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}
```

### 3. Fix ConfigSyncService (apps/air-quality-app/src/config_sync/service.rs)

```rust
impl StreamConfigYaml {
    pub fn to_stream_config(&self) -> StreamConfig {
        StreamConfig {
            stream_id: self.stream_id.clone(),
            description: self.description.clone().unwrap_or_default(),
            // ... existing fields ...

            // NEW: Extract silver_etl from the config
            silver_etl: self.silver_etl.clone(),
        }
    }
}
```

### 4. Silver Subscriber Uses StreamRegistry

```rust
// BEFORE (broken):
fn load_silver_etl_config(stream_id: &str) -> Option<SilverEtlConfig> {
    let path = format!("config/base/streams/{}/config.yaml", stream_id);
    let yaml = fs::read_to_string(&path).ok()?;
    // ... parse YAML ...
}

// AFTER (fixed):
async fn get_silver_etl_config(
    registry: &StreamRegistry,
    stream_id: &str
) -> Result<Option<SilverEtlConfig>, ConfigError> {
    let config = registry.load_stream(stream_id).await?;
    Ok(config.silver_etl)
}
```

---

## Architecture After Fix

```
config-client/
└── StreamRegistry     # Loads StreamConfig (now includes silver_etl)

core/src/types/
├── StreamConfig       # stream_id, fields, sources, storage, silver_etl ✅
└── SilverEtlConfig    # target_table, field_mappings, dq_rules

apps/air-quality-app/
├── SourceManager      # Uses StreamRegistry ✅
├── ConfigSyncService  # Syncs complete config including silver_etl ✅
└── SilverSubscriber   # Uses StreamRegistry.load_stream() ✅
```

---

## Consequences

### Positive

1. **Single source of truth** - All config from etcd via StreamRegistry
2. **No new abstractions** - Extends existing, proven pattern
3. **Simpler** - One struct, one registry, one code path
4. **Testable** - StreamRegistry can still be mocked for tests
5. **Backward compatible** - `silver_etl` is optional

### Negative

1. **StreamConfig grows** - But it's the natural place for this config
2. **Migration needed** - Existing etcd data needs silver_etl populated

### Neutral

1. **YAML format unchanged** - silver_etl section already exists
2. **JSON migration still needed** - Part of dp-018 Phase 0

---

## Validation

```rust
// Test that Silver can load config via StreamRegistry
#[tokio::test]
async fn test_silver_loads_from_registry() {
    let registry = StreamRegistry::new(&["http://localhost:2379"]).await?;
    let config = registry.load_stream("air-quality").await?;

    assert!(config.silver_etl.is_some());
    let silver = config.silver_etl.unwrap();
    assert_eq!(silver.target_table, "silver.air_quality_readings");
}
```

---

## References

- [dp-016 ADR-016-001: Config Source of Truth](../dp-016/architecture/ADR-016-001-config-source-of-truth.md)
- [config-client/src/stream/registry.rs](../../../../config-client/src/stream/registry.rs)
- [core/src/types/stream_config.rs](../../../../core/src/types/stream_config.rs)
