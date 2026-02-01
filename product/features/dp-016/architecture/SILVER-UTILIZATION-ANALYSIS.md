# Silver ETL Configuration Utilization Analysis

**Feature**: dp-016 (Config Unification)
**Date**: 2026-02-01
**Analyst**: ndp-timescale-dev

## Executive Summary

The silver_etl configuration section is loaded **differently** between batch ETL and streaming ETL:

| Component | Stream List Source | silver_etl Config Source | Root Cause |
|-----------|-------------------|-------------------------|------------|
| `silver-etl` (batch) | etcd OR YAML | etcd -> YAML fallback | ConfigLoader with dual source |
| `air-quality-app` (streaming) | etcd (`list_streams()`) | YAML only | `load_silver_etl_config()` reads YAML directly |

This split causes **air-013**: when etcd has stream keys but YAML files are missing (or vice versa), the streaming SilverSubscriber fails to find ETL configs.

---

## Question 1: Where is silver_etl Loaded From?

### Batch ETL: `apps/silver-etl/src/config.rs`

The batch `silver-etl` binary uses `ConfigLoader` which tries **etcd first, then YAML fallback**:

```rust
// apps/silver-etl/src/config.rs:33-49
pub async fn load_stream_config(&self, stream_id: &str) -> Result<SilverEtlConfig> {
    debug!(stream_id = %stream_id, "Loading stream config");

    // Try etcd first
    match self.load_from_etcd(stream_id).await {
        Ok(config) => {
            debug!("Loaded from etcd successfully");
            return Ok(config);
        }
        Err(e) => {
            debug!(error = %e, "etcd failed, falling back to YAML");
        }
    }

    // Fallback to YAML
    self.load_from_yaml(stream_id).await
}
```

The etcd path uses `get_prefix_nested` to unflatten keys:

```rust
// apps/silver-etl/src/config.rs:77-86
let prefix = format!("/streams/{}/silver_etl", stream_id);
let nested_value = client.get_prefix_nested(&prefix).await.context(format!(
    "Stream '{}' has no silver_etl config in etcd (run sync script)",
    stream_id
))?;

let config: SilverEtlConfig = serde_json::from_value(nested_value)...
```

### Streaming ETL: `apps/air-quality-app/src/main.rs`

The streaming app (`air-quality-app`) uses a **local function that reads YAML directly**:

```rust
// apps/air-quality-app/src/main.rs:602-629
#[allow(dead_code)]
async fn load_silver_etl_config(
    config_dir: &str,
    stream_id: &str,
) -> Result<Option<SilverEtlConfig>, Box<dyn std::error::Error + Send + Sync>> {
    use std::path::Path;

    let dir_path = Path::new(config_dir).join(stream_id).join("config.yaml");
    let flat_path = Path::new(config_dir).join(format!("{}.yaml", stream_id));

    let yaml_path = if dir_path.exists() {
        dir_path
    } else if flat_path.exists() {
        flat_path
    } else {
        return Ok(None);  // <-- Returns None, no etcd fallback!
    };

    let contents = tokio::fs::read_to_string(&yaml_path).await?;

    #[derive(serde::Deserialize)]
    struct StreamConfigWithSilver {
        #[serde(default)]
        silver_etl: Option<SilverEtlConfig>,
    }

    let config: StreamConfigWithSilver = serde_yaml::from_str(&contents)?;
    Ok(config.silver_etl)
}
```

**Key Difference**: This function has **no etcd path**. It only reads from YAML files.

---

## Question 2: The Split - Why Different Sources?

### Stream Discovery: etcd via `list_streams()`

The streaming app uses `StreamRegistry::list_streams()` which queries etcd:

```rust
// config-client/src/stream/registry.rs:62-83
pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError> {
    debug!("Listing all streams");

    let keys = self.client.list("/").await?;

    // Extract stream IDs from keys like "/streams/air-quality/config"
    let stream_ids: Vec<String> = keys
        .iter()
        .filter_map(|key| {
            let parts: Vec<&str> = key.trim_start_matches("/streams/").split('/').collect();
            if parts.len() >= 2 && parts[1] == "config" {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    info!("Found {} streams", stream_ids.len());
    Ok(stream_ids)
}
```

### silver_etl Config: YAML directly

```rust
// apps/air-quality-app/src/main.rs:510-527
// Build table_mapping from ETL configs (source of truth: YAML config.target_table)
// This matches how batch silver-etl uses config.target_table from YAML
let streams = registry.list_streams().await.unwrap_or_default();  // <-- etcd!
let mut table_mapping = HashMap::new();

for stream_id in &streams {
    if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, stream_id).await {  // <-- YAML!
        if silver_config.enabled {
            // Use target_table directly from config
            tracing::debug!(
                stream_id = %stream_id,
                target_table = %silver_config.target_table,
                "Adding table mapping from silver_etl config"
            );
            table_mapping.insert(stream_id.clone(), silver_config.target_table.clone());
        }
    }
}
```

### Why This Happened

Historical reasons documented in reflexion episode 23:

> "Fix SilverSubscriber race condition - scan YAML directory instead of etcd"

The YAML-only approach was introduced to fix a race condition during startup. The intent was to use YAML as the authoritative source for `silver_etl` config while etcd handles stream discovery. However, this creates a **semantic mismatch**:

1. A stream can exist in etcd (synced from YAML) but YAML might be missing on the filesystem
2. The `silver_etl` section must be present in YAML for streaming ETL to work
3. Batch ETL has dual-source fallback, but streaming does not

---

## Question 3: Critical silver_etl Fields

Based on `core/src/config/silver_etl.rs`, here are the critical fields:

### Required Fields

| Field | Type | Used By | Purpose |
|-------|------|---------|---------|
| `enabled` | bool | All | Master switch for ETL processing |
| `target_table` | String | All | Target TimescaleDB table (e.g., `silver.air_quality_observations`) |
| `timestamp` | TimestampMapping | All | Maps Bronze timestamp to Silver column |

### Transform Configuration

| Field | Type | Used By | Purpose |
|-------|------|---------|---------|
| `timestamp.source_field` | String | transform | Bronze field containing timestamp |
| `timestamp.target_field` | String | transform | Silver column name for timestamp |
| `timestamp.transform` | TimestampTransform | transform | How to parse (microseconds, ISO8601, etc.) |
| `identity_fields` | Vec<IdentityField> | transform, dedup | Fields that pass through unchanged (ndp_id) |
| `field_mappings` | Vec<SilverFieldMapping> | transform | Maps Bronze paths to Silver columns with types/transforms |

### Data Quality Configuration

| Field | Type | Used By | Purpose |
|-------|------|---------|---------|
| `dq_rules` | Vec<DqRule> | DQ engine | Cross-field and temporal rules |
| `field_mappings[].dq_rules` | Vec<DqRule> | DQ engine | Per-field validation rules |
| `dq_output.enabled` | bool | DQ engine | Whether to add dq_flags column |
| `dq_output.target_column` | String | DQ engine | Column name for flags (default: `dq_flags`) |

### Deduplication Configuration

| Field | Type | Used By | Purpose |
|-------|------|---------|---------|
| `deduplication.enabled` | bool | batch, streaming | Whether to deduplicate |
| `deduplication.key_columns` | Vec<String> | batch, streaming | Columns forming unique key |
| `deduplication.strategy` | DeduplicationStrategy | batch | Upsert, Skip, or Replace |

### Incremental Load Configuration

| Field | Type | Used By | Purpose |
|-------|------|---------|---------|
| `incremental.enabled` | bool | batch | Use watermark for incremental loads |
| `incremental.watermark_column` | String | batch | Column to track progress |
| `incremental.lag_interval` | String | batch | Buffer for late arrivals |

### Pre-Transform (Array Explosion)

| Field | Type | Used By | Purpose |
|-------|------|---------|---------|
| `pre_transform` | Option<PreTransformConfig> | NWS forecasts | Array explosion for forecast data |
| `valid_timestamp` | Option<ValidTimestampMapping> | NWS forecasts | Secondary timestamp for forecast valid time |

---

## Question 4: Batch vs Streaming Config Loading

### Batch ETL (`apps/silver-etl/src/main.rs`)

```rust
// apps/silver-etl/src/main.rs:295
let config_loader = ConfigLoader::new(&cli.etcd_endpoint, &cli.config_dir);

// Line 300-305: Get streams from ConfigLoader (etcd -> YAML fallback)
let streams = match stream {
    Some(s) => vec![s],
    None => config_loader
        .load_all_enabled()  // Uses ConfigLoader's dual-source approach
        .await
        .context("Failed to load enabled streams")?,
};

// Line 333: Load individual stream config (etcd -> YAML fallback)
let stream_config = match config_loader.load_stream_config(stream_id).await {
    Ok(config) => config,
    Err(e) => { ... }
};
```

### Streaming ETL (`apps/air-quality-app/src/main.rs`)

```rust
// apps/air-quality-app/src/main.rs:349
async fn create_silver_subscribers(
    _event_bus: Arc<neural_core::EventBus>,
    registry: Arc<StreamRegistry>,  // <-- etcd-connected
) -> Result<Vec<Box<dyn Subscriber>>, Box<dyn std::error::Error + Send + Sync>> {

    let config_dir = std::env::var("STREAM_CONFIG_DIR")
        .unwrap_or_else(|_| "/workspaces/neural-data-platform/config/base/streams".to_string());

    // Line 512: List streams from etcd
    let streams = registry.list_streams().await.unwrap_or_default();

    // Line 516: Load silver_etl from YAML only (no etcd fallback!)
    for stream_id in &streams {
        if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, stream_id).await {
            if silver_config.enabled {
                table_mapping.insert(stream_id.clone(), silver_config.target_table.clone());
            }
        }
    }
```

### Comparison Table

| Aspect | Batch (`silver-etl`) | Streaming (`air-quality-app`) |
|--------|---------------------|-------------------------------|
| Stream discovery | `ConfigLoader.list_all_streams()` | `StreamRegistry.list_streams()` |
| Stream discovery source | etcd -> YAML fallback | etcd only |
| silver_etl config source | etcd -> YAML fallback | YAML only |
| Config loader | `silver_etl::ConfigLoader` | Local `load_silver_etl_config()` |
| Handles missing etcd | Yes, falls back to YAML | N/A for silver_etl |
| Handles missing YAML | Yes, uses etcd | Silently skips stream |

---

## Code Evidence Summary

| File | Line(s) | Finding |
|------|---------|---------|
| `apps/silver-etl/src/config.rs` | 33-49 | ConfigLoader tries etcd first, YAML fallback |
| `apps/silver-etl/src/config.rs` | 77-86 | etcd path uses `/streams/{id}/silver_etl` prefix |
| `apps/silver-etl/src/config.rs` | 95-129 | YAML path tries `{stream_id}/config.yaml` then `{stream_id}.yaml` |
| `apps/air-quality-app/src/main.rs` | 602-629 | `load_silver_etl_config()` is YAML-only, no etcd |
| `apps/air-quality-app/src/main.rs` | 510-527 | Streaming uses etcd for list, YAML for config |
| `config-client/src/stream/registry.rs` | 62-83 | `list_streams()` queries etcd `/streams/*/config` |
| `core/src/config/silver_etl.rs` | 59-106 | SilverEtlConfig struct definition |
| `core/src/subscribers/silver.rs` | 50-101 | SilverSubscriberConfig uses `etl_configs` HashMap |

---

## Recommendations for dp-016

1. **Unify Config Loading**: Create a shared `SilverConfigLoader` that both batch and streaming use, with consistent etcd -> YAML fallback behavior.

2. **Add etcd Support to Streaming**: Modify `load_silver_etl_config()` in `air-quality-app/src/main.rs` to try etcd first (like batch does).

3. **Document Source of Truth**: Clarify whether YAML or etcd is authoritative for `silver_etl` config. Currently it's inconsistent.

4. **Consider MCP Integration**: The MCP server already has tools for both Bronze and Silver schema discovery. Could expose a unified "get silver_etl config" tool.

5. **Test Cross-Source Scenarios**: Add integration tests that verify behavior when:
   - Stream exists in etcd but not YAML
   - Stream exists in YAML but not etcd
   - silver_etl section missing but stream exists

---

## Related ADRs and Features

- **air-013**: Original race condition fix that introduced YAML-only path
- **dp-006**: Silver ETL Configuration types
- **dp-009**: Silver metadata fields
- **dp-012**: SilverSubscriber streaming integration
- **ADR-012-005**: Parser deprecation (parsers moved to config-driven transforms)

---

## Pattern Feedback

This analysis used pattern `configuration:silver-metadata-fields` (ID: 62) which was helpful for understanding the field structure. The pattern correctly identified the key config sections.

Recommendation: Store this utilization analysis as a new pattern for future reference.
