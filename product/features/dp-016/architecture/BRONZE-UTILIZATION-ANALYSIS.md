# Bronze Configuration Utilization Analysis

**Feature**: dp-016 - Configuration Architecture Cleanup
**Date**: 2026-02-01
**Author**: ndp-rust-dev agent

---

## Executive Summary

The Bronze layer configuration in NDP stream YAMLs has **three sections with distinct runtime purposes**:

| Section | Runtime Usage | Consumers |
|---------|---------------|-----------|
| `sources` | **Actively Used** - Drives ingestion runtime | SourceManager |
| `fields` | **Passive/Metadata** - Used for validation and documentation only | ConfigSync validation, MCP schema tools |
| `storage` | **Actively Used** - Controls batching and buffering | SourceManager, storage layer |

The `fields` section is NOT parsed during Bronze ingestion - raw JSON payloads are stored as-is. The fields definition serves as documentation and is used by the MCP server for schema introspection.

---

## Q1: Where is Bronze Config Loaded From?

### Answer: etcd (synced from YAML via GitOps)

**Data Flow:**
```
YAML files               ConfigSyncService              etcd                 StreamRegistry
config/base/streams/ --> sync_all() on startup -->  /streams/{id}/config --> load_stream()
```

### Evidence

**Startup Config Sync** (`apps/air-quality-app/src/main.rs:137-168`):
```rust
// ========== AIR-005: Config Sync - Sync YAML configs to etcd ==========
let config_dir = std::env::var("STREAM_CONFIG_DIR")
    .unwrap_or_else(|_| "/workspaces/neural-data-platform/config/base/streams".to_string());

if std::path::Path::new(&config_dir).exists() {
    tracing::info!("Syncing stream configs from {}", config_dir);
    let sync_service = air_quality_app::config_sync::ConfigSyncService::new(&config_dir);

    match config_client::StreamRegistry::new(&[&etcd_endpoint]).await {
        Ok(registry) => match sync_service.sync_all(&registry).await {
            Ok(count) => {
                tracing::info!("Synced {} stream configs to etcd (AIR-005 config sync)", count);
            }
            ...
        }
    }
}
```

**Runtime Loading from etcd** (`apps/air-quality-app/src/coordinator/source_manager.rs:102-114`):
```rust
pub async fn start_all_sources(&mut self) -> Result<(), SourceManagerError> {
    // Load all stream configurations
    let streams = self.registry.list_streams().await
        .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

    for stream_id in streams {
        let config = self.registry.load_stream(&stream_id).await
            .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;
        ...
    }
}
```

**StreamRegistry loads from etcd** (`config-client/src/stream/registry.rs:30-58`):
```rust
pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError> {
    // Load from etcd
    let key = format!("/{}/config", stream_id);
    let config: StreamConfig = self.client.get(&key).await?;

    // Validate before caching
    config.validate()
        .map_err(|e| ConfigError::EnvError(format!("Invalid stream config: {}", e)))?;
    ...
}
```

### Key Finding

**YAML is the source of truth, etcd is the runtime store.** On startup, ConfigSyncService syncs YAML files to etcd. At runtime, SourceManager reads from etcd via StreamRegistry.

---

## Q2: Which Bronze Fields Are Actually Used?

### sources Section: FULLY USED

The `sources` section drives the entire ingestion pipeline.

**SourceManager.spawn_source()** (`apps/air-quality-app/src/coordinator/source_manager.rs:151-371`):
```rust
async fn spawn_source(&mut self, stream_id: &str, source_config: &SourceConfig) -> Result<String, SourceManagerError> {
    // Source type determines handler
    let task_handle = match source_config.source_type {
        SourceType::HttpPoll => {
            // Extract config params
            let has_parser = source_config.params.get("parser_name").and_then(|v| v.as_str()).is_some();
            let ndp_id = source_config.ndp_id.clone();
            let context = source_config.context.clone();
            ...
        }
        SourceType::Mqtt => {
            let config = self.parse_mqtt_config(stream_id, source_config)?;
            let parser_config = if let Some(parser_val) = source_config.params.get("parser") {
                serde_json::from_value::<ParserConfig>(parser_val.clone())?
            } else {
                // Fallback to default FlatJson parser
                ParserConfig { parser_type: ParserType::FlatJson, ... }
            };
            ...
        }
    }
}
```

**Source params actually consumed** (`apps/air-quality-app/src/coordinator/source_manager.rs:703-801`):

| Param | Location | Purpose |
|-------|----------|---------|
| `broker_url` | :709-716 | MQTT broker address |
| `port` | :718-722 | MQTT port |
| `topic_pattern` | :724-728 | MQTT subscription topic |
| `client_id` | :730-736 | MQTT client identifier |
| `buffer_capacity` | :737-741 | Channel buffer size |
| `qos` | :743-753 | MQTT quality of service |
| `reconnect_delay_secs` | :755-759 | Reconnection timing |
| `max_reconnect_delay_secs` | :760-765 | Max reconnection backoff |
| `ndp_id_topic_segment` | :767-772 | Topic segment for ndp_id extraction |
| `parser` | :283-309 | Parser configuration object |
| `endpoints` | :568-645 | HTTP polling endpoints |
| `poll_interval_secs` | :549-553 | HTTP polling frequency |
| `timeout_secs` | :555-559 | HTTP request timeout |
| `parser_name` | :537-547 | Named parser type |

### fields Section: METADATA ONLY (NOT PARSED AT RUNTIME)

The `fields` section in bronze config is **not used during ingestion**. Bronze stores raw JSON payloads without parsing individual fields.

**Evidence - Raw payload storage:**

The bronze subscriber stores `RawDataPoint` which contains the raw JSON payload:
- `RawDataPoint.raw_payload` - The full JSON from MQTT/HTTP response
- No field-by-field extraction occurs in bronze layer
- Parsing happens in Silver layer via `silver_etl.field_mappings`

**Where fields IS used:**

1. **ConfigSync Validation** (`apps/air-quality-app/src/config_sync/service.rs:134-136`):
```rust
// Validate
config.validate()?;
```

2. **StreamConfig.validate()** (`core/src/types/stream_config.rs:451-467`):
```rust
pub fn validate(&self) -> Result<(), StreamConfigError> {
    // Validate stream ID format
    if !is_valid_stream_id(&self.stream_id) {
        return Err(StreamConfigError::InvalidStreamId(self.stream_id.clone()));
    }
    // Must have at least one field
    if self.fields.is_empty() {
        return Err(StreamConfigError::NoFields);
    }
    // Must have at least one source
    if self.sources.is_empty() {
        return Err(StreamConfigError::NoSources);
    }
    // Validate each field
    for field in &self.fields {
        field.validate()?;
    }
    Ok(())
}
```

3. **MCP Schema Tools** (`core/ndp-mcp-server/src/etcd/registry_adapter.rs:63-88`):
```rust
// Convert fields to field mappings (for MCP introspection)
mcp_config.field_mappings = core_config.fields.iter()
    .map(|field| FieldMapping {
        source: field.name.clone(),
        target: Some(field.name.clone()),
        field_type: Some(format!("{:?}", field.field_type).to_lowercase()),
    })
    .collect();

// Build entity schema from description and version
mcp_config.entity_schema = EntitySchema {
    name: core_config.description.clone(),
    version: core_config.version.clone(),
    attributes: core_config.fields.iter()
        .map(|field| SchemaAttribute {
            name: field.name.clone(),
            attr_type: format!("{:?}", field.field_type).to_lowercase(),
            unit: field.unit.clone(),
            required: !field.nullable,
        })
        .collect(),
};
```

### storage Section: ACTIVELY USED

**Storage config consumption** (`apps/air-quality-app/src/config_sync/service.rs:472-501`):
```rust
// Extract storage config
let storage = if let Some(storage_yaml) = self.extra.get("storage") {
    if let serde_yaml::Value::Mapping(map) = storage_yaml {
        let batch_size = map.get(&serde_yaml::Value::String("batch_size".to_string()))
            .and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(100);
        let batch_timeout_secs = map.get(&serde_yaml::Value::String("batch_timeout_secs".to_string()))
            .and_then(|v| v.as_u64()).unwrap_or(5);
        let buffer_capacity = map.get(&serde_yaml::Value::String("buffer_capacity".to_string()))
            .and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(1000);
        Some(StorageConfig { batch_size, batch_timeout_secs, buffer_capacity })
    } else { None }
} else { None };
```

---

## Q3: Can Bronze Sources Be Added/Removed Without Restart?

### Answer: Partially - Methods Exist But Not Exposed

**Dynamic Update Method Exists** (`apps/air-quality-app/src/coordinator/source_manager.rs:1067-1099`):
```rust
/// Update sources based on new stream configuration
pub async fn update_sources_for_stream(&mut self, stream_id: &str) -> Result<(), SourceManagerError> {
    info!("Updating sources for stream: {}", stream_id);

    // Load new configuration
    let config = self.registry.load_stream(stream_id).await
        .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

    // Stop existing sources for this stream
    let source_ids: Vec<String> = {
        let sources = self.sources.read().await;
        sources.iter()
            .filter(|(_, info)| info.stream_id == stream_id)
            .map(|(id, _)| id.clone())
            .collect()
    };

    for source_id in source_ids {
        self.stop_source(&source_id).await?;
    }

    // Start new sources
    self.start_sources_for_stream(&config).await?;

    info!("Sources updated for stream: {}", stream_id);
    Ok(())
}
```

**Also: restart_source()** (`apps/air-quality-app/src/coordinator/source_manager.rs:1024-1064`):
```rust
pub async fn restart_source(&mut self, source_id: &str) -> Result<(), SourceManagerError> {
    // Stop the source
    self.stop_source(source_id).await?;
    // Load stream config to get source config
    let stream_config = self.registry.load_stream(&stream_id).await?;
    // Restart the source
    self.spawn_source(&stream_id, source_config).await?;
}
```

### Current Limitation

These methods exist but are **not exposed via API or triggered automatically**:
- No etcd watch mechanism to detect config changes
- No admin API endpoint to trigger `update_sources_for_stream()`
- Restart currently required for config changes to take effect

### Recommendation for dp-016

Consider adding:
1. etcd watch on `/streams/*/config` keys
2. Admin endpoint: `POST /admin/streams/{id}/refresh`
3. Automatic source restart on config change detection

---

## Q4: What is the `fields` Section Purpose?

### Answer: Documentation and Validation (Not Runtime Parsing)

The `fields` section serves **three purposes**, none of which involve parsing data at Bronze layer:

### 1. Configuration Validation

Ensures stream configs have at least one field defined:

**StreamConfig.validate()** (`core/src/types/stream_config.rs:452-454`):
```rust
// Must have at least one field
if self.fields.is_empty() {
    return Err(StreamConfigError::NoFields);
}
```

### 2. MCP Schema Introspection

The fields are exposed via MCP tools for data dictionary queries:

**describe_schema tool** (`core/src/mcp/handler.rs:326-327`):
```
"Get schema information for a stream. Modes: 'source' shows raw_payload structure
and field mappings from parser config, 'target' shows entity_schemas attributes..."
```

**validate_config tool** (`core/src/mcp/handler.rs:347-348`):
```
"Compare stream configuration in etcd against actual Bronze Parquet schema.
Returns validation status (match, mismatch, partial), lists config_fields from
entity_schemas, raw_payload_fields from Parquet..."
```

### 3. Documentation for Data Consumers

Fields document the expected data shape for:
- Dashboard developers (Grafana)
- Silver layer ETL designers
- Feature engineers

### The Actual Parsing Happens in Silver Layer

**Silver ETL uses field_mappings** (from `config/base/streams/air-quality/config.yaml:169-256`):
```yaml
silver_etl:
  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
    - source_path: raw_payload.rco2
      target_column: co2
      type: smallint
```

**NOT the bronze `fields` section** which is metadata:
```yaml
fields:
  pm25:
    type: "float"
    unit: "ug/m3"
    description: "Particulate Matter 2.5 micrometers"
    nullable: false
```

---

## Summary: Config Section Utilization Matrix

| Section | Loaded From | Runtime Consumer | Purpose |
|---------|-------------|------------------|---------|
| `stream_id` | etcd | SourceManager | Identifies stream for routing |
| `enabled` | etcd | SourceManager | Enable/disable stream |
| `sources` | etcd | SourceManager | **Drives ingestion** - broker, topics, parser, auth |
| `sources[].ndp_id` | etcd | SourceManager | Stable source identifier (AIR-009) |
| `sources[].context` | etcd | SourceManager | Mutable context metadata (AIR-009) |
| `sources[].params` | etcd | SourceManager | Source-specific parameters |
| `storage` | etcd | SourceManager | Batching and buffer config |
| `fields` | etcd | ConfigSync, MCP | **Metadata only** - validation, documentation |
| `entity_schemas` | etcd | MCP handler | Data dictionary introspection |
| `silver_etl` | YAML file | SilverSubscriber | Bronze-to-Silver field extraction |

---

## Recommendations for dp-016

1. **Document the dual purpose of `fields`**: Make clear it's metadata, not parsing config
2. **Consider removing `fields` validation requirement**: If fields is metadata, why require it?
3. **Align `fields` with `entity_schemas`**: Currently both exist with overlapping purposes
4. **Add dynamic source refresh**: Expose `update_sources_for_stream()` via admin API

---

## Related Files

| File | Lines | Purpose |
|------|-------|---------|
| `apps/air-quality-app/src/main.rs` | 137-168 | Startup config sync |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | 99-128, 703-801, 1067-1099 | Source lifecycle, config parsing |
| `config-client/src/stream/registry.rs` | 30-58 | etcd config loading |
| `apps/air-quality-app/src/config_sync/service.rs` | 112-515 | YAML to etcd sync |
| `core/src/types/stream_config.rs` | 367-517 | StreamConfig struct and validation |
| `core/ndp-mcp-server/src/etcd/registry_adapter.rs` | 49-91 | MCP schema conversion |
| `core/src/mcp/handler.rs` | 685-768, 973-1000 | MCP schema tools |
