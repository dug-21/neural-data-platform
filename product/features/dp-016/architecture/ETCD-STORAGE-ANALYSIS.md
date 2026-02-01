# etcd Storage Architecture Analysis

**Feature**: dp-016 (MCP Config Administration)
**Date**: 2026-02-01
**Author**: ndp-architect

## Executive Summary

The Neural Data Platform uses etcd as the central configuration store with a **flattened key structure**. YAML configuration files are synchronized to etcd via a Python-based sync script that flattens nested YAML into individual key-value pairs. The `config-client` crate provides retrieval with optional unflattening for nested structures. Watch capability exists and supports hot-reload patterns.

---

## 1. Storage Pattern: Flattened Keys (Not Blob JSON)

### Evidence

The sync script at `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh` uses Python to flatten YAML:

```python
# scripts/sync-config-to-etcd.sh:36-44
def flatten(d, parent_key='', sep='/'):
    items = []
    for k, v in d.items():
        new_key = f'{parent_key}{sep}{k}' if parent_key else k
        if isinstance(v, dict):
            items.extend(flatten(v, new_key, sep=sep).items())
        else:
            items.append((new_key, v))
    return dict(items)
```

### Key Structure

Stream configurations are stored under `/streams/{stream-id}/...` with each leaf value as a separate key:

```
/streams/outdoor-weather/stream_id            -> "outdoor-weather"
/streams/outdoor-weather/description          -> "Outdoor weather data..."
/streams/outdoor-weather/enabled              -> true
/streams/outdoor-weather/retention_days       -> 90
/streams/outdoor-weather/fields/0/name        -> "temperature"
/streams/outdoor-weather/fields/0/type        -> "float"
/streams/outdoor-weather/fields/0/unit        -> "celsius"
/streams/outdoor-weather/silver_etl/enabled   -> true
/streams/outdoor-weather/silver_etl/target_table -> "silver.weather_observations"
...
```

### Sync Script Key Patterns

| Config Type | Base Path | Example Key |
|-------------|-----------|-------------|
| Stream config | `/streams/{stream-id}/` | `/streams/air-quality/config/enabled` |
| Legacy service | `/{service-name}/` | `/mqtt/broker_url` |

The sync script handles stream configurations specially:
```bash
# scripts/sync-config-to-etcd.sh:98-106
if [ "$service_name" = "streams" ] && [ -d "$service_dir" ]; then
    for stream_dir in "$service_dir"/*/; do
        if [ -f "$stream_dir/config.yaml" ]; then
            stream_id=$(basename "$stream_dir")
            sync_yaml_to_etcd_with_prefix "$stream_dir/config.yaml" "/streams/$stream_id"
        fi
    done
fi
```

---

## 2. What Gets Stored in etcd

### Stored Content

**The entire YAML file is flattened and stored**, including:

| Section | Stored? | Key Prefix |
|---------|---------|------------|
| `stream_id`, `description`, `enabled` | Yes | `/streams/{id}/` |
| `fields` (Bronze schema) | Yes | `/streams/{id}/fields/` |
| `sources` | Yes | `/streams/{id}/sources/` |
| `storage` | Yes | `/streams/{id}/storage/` |
| `entity_schemas` | Yes | `/streams/{id}/entity_schemas/` |
| `silver_etl` | Yes | `/streams/{id}/silver_etl/` |

### Evidence from YAML Files

A sample stream config (`/workspaces/neural-data-platform/config/base/streams/outdoor-weather/config.yaml`) contains all sections:

- Lines 1-7: Stream metadata (`stream_id`, `description`, `version`, etc.)
- Lines 9-64: `fields` array (Bronze schema)
- Lines 66-126: `sources` array with http_poll config
- Lines 127-130: `storage` config
- Lines 132-202: `entity_schemas` (data dictionary)
- Lines 204-442: `silver_etl` configuration with field mappings and DQ rules

### What the ConfigSyncService Stores

The Rust `ConfigSyncService` (`/workspaces/neural-data-platform/apps/air-quality-app/src/config_sync/service.rs`) stores a `StreamConfig` struct which includes:

```rust
// apps/air-quality-app/src/config_sync/service.rs:346-349
fn to_stream_config(&self) -> Result<StreamConfig, ConfigSyncError> {
    // Converts YAML to StreamConfig containing:
    // - stream_id, description, version, enabled
    // - retention_days, compression_after_days, partitioning_strategy
    // - fields: Vec<SchemaField>
    // - sources: Vec<SourceConfig>
    // - storage: Option<StorageConfig>
}
```

**Note**: The Rust sync service only stores the core `StreamConfig` struct. The `entity_schemas` and `silver_etl` sections are stored separately by the shell sync script.

---

## 3. Retrieval Pattern

### Direct Key Access

Single values can be retrieved directly:

```rust
// config-client/src/client.rs:29-42
pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
    let full_key = self.full_key(key);
    let resp = self.client.clone().get(full_key.clone(), None).await?;
    let kv = resp.kvs().first()
        .ok_or_else(|| ConfigError::NotFound(full_key.clone()))?;
    let value: T = serde_json::from_slice(kv.value())?;
    Ok(value)
}
```

### Prefix-Based Retrieval with Unflattening

For reconstructing nested objects from flattened keys:

```rust
// config-client/src/client.rs:127-145
pub async fn get_prefix_nested(&self, prefix: &str) -> Result<serde_json::Value, ConfigError> {
    let pairs = self.get_prefix_raw(prefix).await?;
    if pairs.is_empty() {
        return Err(ConfigError::NotFound(prefix.to_string()));
    }
    let mut root = serde_json::Map::new();
    for (key, value) in pairs {
        let parts: Vec<&str> = key.split('/').collect();
        insert_nested(&mut root, &parts, value);
    }
    Ok(serde_json::Value::Object(root))
}
```

### StreamRegistry Caching

The `StreamRegistry` implements in-memory caching with `RwLock`:

```rust
// config-client/src/stream/registry.rs:9-12
pub struct StreamRegistry {
    client: ConfigClient,
    cache: Arc<RwLock<std::collections::HashMap<String, StreamConfig>>>,
}

// config-client/src/stream/registry.rs:30-59
pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError> {
    // Try cache first
    {
        let cache = self.cache.read().await;
        if let Some(config) = cache.get(stream_id) {
            return Ok(config.clone());
        }
    }
    // Load from etcd
    let config: StreamConfig = self.client.get(&key).await?;
    // Update cache
    {
        let mut cache = self.cache.write().await;
        cache.insert(stream_id.to_string(), config.clone());
    }
    Ok(config)
}
```

### Caching Characteristics

| Aspect | Behavior |
|--------|----------|
| Cache type | In-memory `HashMap` |
| Invalidation | Manual via `clear_cache()` |
| TTL | None (persistent until invalidated) |
| Write-through | Yes (save updates cache) |
| Delete-through | Yes (delete removes from cache) |

**Gap**: No automatic cache invalidation when etcd changes externally.

---

## 4. Watch Capability for Hot-Reload

### Watch Implementation

The `config-client` crate provides etcd watch support:

```rust
// config-client/src/watch.rs:11-80
pub struct WatchHandle {
    cancel_tx: mpsc::Sender<()>,
}

impl WatchHandle {
    pub(crate) async fn new<F>(
        client: Client,
        prefix: &str,
        callback: F,
    ) -> Result<Self, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
    {
        // Creates watch with prefix option
        let opts = WatchOptions::new().with_prefix();
        match client.clone().watch(prefix.clone(), Some(opts)).await {
            Ok((mut watcher, mut stream)) => {
                // Event loop handles Put and Delete
                for event in resp.events() {
                    let value = match event.event_type() {
                        EventType::Put => serde_json::from_slice(kv.value()).ok(),
                        EventType::Delete => None,
                    };
                    callback(key, value);
                }
            }
        }
    }
}
```

### Watch Features

| Feature | Supported | Evidence |
|---------|-----------|----------|
| Prefix watching | Yes | `WatchOptions::new().with_prefix()` (line 24) |
| Put events | Yes | `EventType::Put` handling (line 44) |
| Delete events | Yes | `EventType::Delete` handling (line 47) |
| Cancellation | Yes | `cancel_tx` channel with `watcher.cancel()` |
| Error handling | Partial | Logs errors but doesn't retry |

### ConfigClient Watch API

```rust
// config-client/src/client.rs:147-154
pub async fn watch<F>(&self, prefix: &str, callback: F) -> Result<WatchHandle, ConfigError>
where
    F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
{
    let full_prefix = self.full_key(prefix);
    WatchHandle::new(self.client.clone(), &full_prefix, callback).await
}
```

### Hot-Reload Gaps

1. **StreamRegistry does not use watches** - Cache is not automatically invalidated
2. **No reconnection logic** - Watch stream ends on error without retry
3. **No structured callback** - Callbacks receive raw key/value, not typed configs

---

## 5. Architecture Diagram

```
+------------------+     sync-config-to-etcd.sh      +------------------+
|  YAML Files      | -------------------------------->|     etcd         |
|  config/base/    |     (Python flatten)            |  /streams/{id}/  |
|  streams/        |                                 |  /silver_etl/    |
+------------------+                                 |  /sources/       |
                                                     +------------------+
                                                              |
                    +----------------+                        |
                    |  ConfigClient  |<-----------------------+
                    |  - get()       |     etcd-client
                    |  - get_prefix_nested()
                    |  - watch()     |
                    +----------------+
                            |
                    +----------------+
                    | StreamRegistry |
                    |  - load_stream()|
                    |  - cache (HashMap)
                    +----------------+
                            |
        +-------------------+-------------------+
        |                   |                   |
+---------------+   +---------------+   +---------------+
| air-quality-  |   | silver-etl    |   | MCP Server    |
| app           |   |               |   | (dp-016)      |
+---------------+   +---------------+   +---------------+
```

---

## 6. Key Findings for dp-016 MCP Admin

### Strengths

1. **Granular keys** - Can update individual fields without rewriting entire config
2. **Watch support exists** - Foundation for hot-reload is in place
3. **Unflattening available** - `get_prefix_nested()` reconstructs objects

### Gaps to Address

| Gap | Impact | dp-016 Recommendation |
|-----|--------|----------------------|
| No auto cache invalidation | Config changes require restart | Use watches + invalidation |
| Flattened arrays lose order | Array indices in keys | Consider blob storage for arrays |
| No watch reconnection | Long-running processes may miss updates | Add exponential backoff retry |
| entity_schemas not in StreamConfig | MCP can't manage via Rust types | Add to StreamConfig or new type |
| silver_etl not in StreamConfig | Same as above | Add SilverEtlConfig to type system |

### Recommended MCP Tools

Based on this analysis, dp-016 should implement:

1. **`config_get_stream`** - Use `get_prefix_nested()` for full stream config
2. **`config_set_field`** - Direct key updates for individual fields
3. **`config_list_streams`** - Use `StreamRegistry::list_streams()`
4. **`config_watch`** - Expose watch with typed callbacks
5. **`config_validate`** - Validate before writing

---

## 7. Related Patterns

### Existing AgentDB Patterns

- Pattern ID 8: `configuration:etcd-pattern` (90% success rate)
- Pattern ID 22: `architecture:mcp-etcd-config` (90% success rate)
- Pattern ID 15: `configuration:stream-files` (90% success rate)

### Referenced Files

| File | Lines | Purpose |
|------|-------|---------|
| `scripts/sync-config-to-etcd.sh` | 1-140 | YAML to etcd sync script |
| `config-client/src/client.rs` | 1-337 | etcd client with get/set/watch |
| `config-client/src/stream/registry.rs` | 1-421 | StreamRegistry with caching |
| `config-client/src/watch.rs` | 1-81 | Watch handle implementation |
| `apps/air-quality-app/src/config_sync/service.rs` | 1-1345 | YAML parsing and sync |
| `apps/silver-etl/src/config.rs` | 1-385 | Config loading with fallback |
| `core/src/types/stream_config.rs` | 1-903 | StreamConfig struct |
| `config/base/streams/outdoor-weather/config.yaml` | 1-442 | Sample full YAML |

---

## 8. Conclusion

The etcd storage architecture is well-suited for MCP-based administration with its granular key structure and existing watch support. The main work for dp-016 involves:

1. Exposing existing capabilities through MCP tools
2. Adding automatic cache invalidation via watches
3. Potentially extending type coverage for `entity_schemas` and `silver_etl`
4. Adding validation before writes

The foundation is solid; dp-016 builds upon it rather than replacing it.
