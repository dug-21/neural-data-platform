# ADR-018-001: JSON Config Pass-Through Architecture

**Status**: Proposed
**Date**: 2026-02-01
**Decision Makers**: NDP Architecture Team
**Feature**: dp-018 JSON Config Foundation

---

## Context

### The Real Problem: Lossy Transformation

The current config system has a fundamental flaw - it transforms config during sync:

```
YAML file
    ↓ deserialize
StreamConfigYaml (has silver_etl in 'extra' HashMap)
    ↓ to_stream_config()  ← LOSSY TRANSFORMATION
StreamConfig (silver_etl discarded)
    ↓ serialize
etcd
    ↓ deserialize
StreamConfig (missing silver_etl)
```

The `to_stream_config()` function transforms `StreamConfigYaml` into `StreamConfig`, but:
- `silver_etl` is captured in `extra` via `#[serde(flatten)]`
- `to_stream_config()` ignores `extra`
- Data is lost in transformation

**This isn't a Silver-specific bug. It's a systemic architecture issue.**

Any config section not explicitly mapped in `to_stream_config()` is silently dropped.

### Why Two Structs?

The codebase has two config structs because YAML and etcd had different shapes:
- `StreamConfigYaml` - matches YAML file structure
- `StreamConfig` - matches what components expect

This dual-struct pattern creates the transformation layer where data gets lost.

---

## Decision

**Migrate to JSON with pass-through architecture. No transformation. One struct.**

```
JSON file (source of truth)
    ↓ validate against JSON Schema
    ↓ sync to etcd AS-IS
etcd (same JSON blob)
    ↓ deserialize
StreamConfig (complete, including silver_etl)
```

### Key Principles

1. **JSON file = etcd blob** - No transformation during sync
2. **One struct: StreamConfig** - Eliminate StreamConfigYaml
3. **Schema validation replaces transformation** - Validate at sync time, not transform
4. **Both Bronze and Silver use same config** - Unified source of truth

---

## Implementation

### 1. Extend StreamConfig (core/src/types/stream_config.rs)

Add `silver_etl` to the canonical struct that everything uses:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfig>,

    // Silver ETL configuration - now part of unified config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silver_etl: Option<SilverEtlConfig>,

    // Entity schemas (deprecated in v1.1, removed in v2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_schemas: Option<Vec<EntitySchema>>,
}
```

### 2. Simplify ConfigSyncService

**Before (lossy transformation):**
```rust
pub fn sync_stream(&self, yaml_path: &Path) -> Result<()> {
    let yaml: StreamConfigYaml = read_yaml(yaml_path)?;
    let config: StreamConfig = yaml.to_stream_config(); // LOSSY
    self.registry.save_stream(&config)?;
}
```

**After (pass-through):**
```rust
pub fn sync_stream(&self, json_path: &Path) -> Result<()> {
    let json = fs::read_to_string(json_path)?;

    // Validate against schema (catches errors early)
    validate_json_schema(&json, &self.schema)?;

    // Deserialize directly to StreamConfig (same struct everywhere)
    let config: StreamConfig = serde_json::from_str(&json)?;

    // Save to etcd (serializes same struct)
    self.registry.save_stream(&config)?;
}
```

### 3. Eliminate StreamConfigYaml

The `StreamConfigYaml` struct with `#[serde(flatten)] extra: HashMap` becomes unnecessary:

```rust
// DELETE THIS:
pub struct StreamConfigYaml {
    pub stream_id: String,
    // ... fields ...
    #[serde(flatten)]
    pub extra: HashMap<String, Value>, // Caught silver_etl but lost it
}

impl StreamConfigYaml {
    pub fn to_stream_config(&self) -> StreamConfig { // LOSSY
        // ...
    }
}
```

With JSON pass-through, we just use `StreamConfig` directly.

### 4. Both Bronze and Silver Use StreamRegistry

```rust
// Bronze (already correct)
let config = registry.load_stream("air-quality").await?;
let sources = &config.sources;

// Silver (now fixed - same pattern)
let config = registry.load_stream("air-quality").await?;
let silver_etl = config.silver_etl.as_ref()
    .ok_or("Stream has no silver_etl config")?;
```

---

## Architecture Comparison

### Before: Dual-Struct with Transformation

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│  YAML file  │ ──→ │ StreamConfigYaml │ ──→ │ StreamConfig│
│             │     │  (with extra)    │     │ (no silver) │
└─────────────┘     └──────────────────┘     └─────────────┘
                           ↓                        ↓
                    to_stream_config()         etcd save
                      (LOSSY)                       ↓
                                              ┌─────────────┐
                                              │    etcd     │
                                              │ (incomplete)│
                                              └─────────────┘
```

### After: Single-Struct Pass-Through

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  JSON file  │ ──→ │StreamConfig │ ──→ │    etcd     │
│  (source)   │     │ (complete)  │     │ (same JSON) │
└─────────────┘     └─────────────┘     └─────────────┘
       ↓                   ↑                    ↓
  JSON Schema         Bronze uses          Silver uses
  validation          same struct          same struct
```

---

## Cascading Benefits

### Bronze (air-quality-app)

| Component | Change |
|-----------|--------|
| ConfigSyncService | Simplified - no transformation |
| SourceManager | No change - already uses StreamRegistry |
| IngestionCoordinator | No change - already uses StreamRegistry |

### Silver (air-quality-app subscriber)

| Component | Change |
|-----------|--------|
| SilverSubscriber | Use StreamRegistry instead of YAML files |
| load_silver_etl_config() | Delete - use registry.load_stream().silver_etl |

### MCP Server

| Component | Change |
|-----------|--------|
| Data Dictionary | Read from config.fields (enriched with descriptions) |
| list_streams | No change - already uses StreamRegistry |

---

## Migration Path

### Phase 0: JSON Migration

1. Create JSON Schema for StreamConfig (including silver_etl)
2. Convert YAML files to JSON: `scripts/migrate-yaml-to-json.sh`
3. Enrich `fields` with descriptions from `entity_schemas`
4. Validate all JSON configs against schema

### Phase 1: Code Changes

1. Add `silver_etl` field to StreamConfig
2. Simplify ConfigSyncService (remove transformation)
3. Delete StreamConfigYaml struct
4. Update Silver to use StreamRegistry
5. Delete load_silver_etl_config() function

### Backward Compatibility

- JSON format accepts both `entity_schemas` (v1.0) and enriched `fields` (v1.1)
- `silver_etl` is optional - streams without it still work
- No changes to etcd key structure (`/streams/{id}/config`)

---

## Consequences

### Positive

1. **No data loss** - What goes in is what comes out
2. **Single source of truth** - JSON file = etcd = runtime config
3. **Simpler code** - Delete transformation layer
4. **Unified Bronze/Silver** - Same config path for all components
5. **Schema validation** - Catch errors at sync time, not runtime
6. **JSON-native** - Works well with MCP, agents, tooling

### Negative

1. **Migration effort** - Convert all YAML to JSON
2. **Schema discipline** - Must maintain JSON Schema

### Neutral

1. **StreamConfig struct grows** - But it's the natural place
2. **etcd stores larger blobs** - But cleaner than key fragmentation

---

## Validation

```rust
#[tokio::test]
async fn test_json_pass_through() {
    // JSON file content
    let json = r#"{
        "stream_id": "air-quality",
        "silver_etl": {
            "target_table": "silver.air_quality_readings"
        }
    }"#;

    // Deserialize directly (no transformation)
    let config: StreamConfig = serde_json::from_str(json)?;

    // Save to etcd
    registry.save_stream(&config).await?;

    // Load from etcd - should be identical
    let loaded = registry.load_stream("air-quality").await?;

    assert!(loaded.silver_etl.is_some());
    assert_eq!(loaded.silver_etl.unwrap().target_table, "silver.air_quality_readings");
}
```

---

## Summary

**The fix isn't "add silver_etl to StreamConfig."**

**The fix is "eliminate the lossy transformation pipeline."**

JSON pass-through architecture:
- One format (JSON)
- One struct (StreamConfig)
- No transformation (pass-through)
- Both Bronze and Silver benefit

---

## References

- [dp-016 ADR-016-001: Config Source of Truth](../dp-016/architecture/ADR-016-001-config-source-of-truth.md)
- [config-client/src/stream/registry.rs](../../../../config-client/src/stream/registry.rs)
- [apps/air-quality-app/src/config_sync/service.rs](../../../../apps/air-quality-app/src/config_sync/service.rs)
