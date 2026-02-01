# dp-018: Current State Research - Configuration Architecture

## Executive Summary

This research documents the current configuration loading architecture in NDP, focusing on the **Bronze vs Silver config loading discrepancy** that causes Silver ETL to silently fail when discovering streams.

**Key Finding**: Bronze subscriber loads configuration from **etcd via StreamRegistry**, while Silver subscriber loads from **YAML files directly**. This dual-source pattern causes silent failures when:
1. YAML files exist but etcd has different data
2. etcd is the source of truth but Silver reads YAML
3. Configuration sync fails silently

---

## 1. config-client Package Analysis

### Location and Structure

```
/workspaces/neural-data-platform/config-client/
├── src/
│   ├── lib.rs              # Public API exports
│   ├── client.rs           # ConfigClient implementation (etcd wrapper)
│   ├── error.rs            # ConfigError types
│   ├── watch.rs            # WatchHandle for config changes
│   └── stream/
│       ├── mod.rs          # StreamRegistry module
│       └── registry.rs     # StreamRegistry implementation
├── examples/
│   └── basic.rs
└── tests/
    └── integration_test.rs
```

### ConfigClient (Line 7-187 in client.rs)

The `ConfigClient` is a thin etcd wrapper providing type-safe configuration access:

```rust
pub struct ConfigClient {
    client: Client,        // etcd_client::Client
    prefix: String,        // Key prefix (e.g., "/streams")
}
```

**Key Methods**:
| Method | Purpose | Line |
|--------|---------|------|
| `new(endpoints)` | Connect to etcd | 14-16 |
| `with_prefix(endpoints, prefix)` | Connect with key prefix | 19-26 |
| `get<T>(key)` | Get typed config value | 29-42 |
| `get_raw(key)` | Get raw JSON | 45-47 |
| `set<T>(key, value)` | Set config value | 50-57 |
| `list(prefix)` | List keys under prefix | 73-90 |
| `get_prefix_nested(prefix)` | Get nested JSON structure | 130-145 |
| `watch(prefix, callback)` | Watch for changes | 148-154 |
| `get_with_env(key, env_prefix)` | Get with env override | 158-178 |

### StreamRegistry (Line 9-172 in registry.rs)

The `StreamRegistry` wraps `ConfigClient` for stream-specific operations:

```rust
pub struct StreamRegistry {
    client: ConfigClient,
    cache: Arc<RwLock<HashMap<String, StreamConfig>>>,
}
```

**Key Methods**:
| Method | Purpose | Line |
|--------|---------|------|
| `new(endpoints)` | Create with `/streams` prefix | 16-27 |
| `load_stream(stream_id)` | Load StreamConfig from etcd | 30-59 |
| `list_streams()` | List all stream IDs | 62-84 |
| `load_all_streams()` | Load all configs | 87-109 |
| `save_stream(config)` | Save to etcd | 122-141 |

**StreamConfig Structure** (loaded from etcd):
- `stream_id`, `description`, `version`, `enabled`
- `retention_days`, `compression_after_days`, `partitioning_strategy`
- `fields: Vec<SchemaField>`
- `sources: Vec<SourceConfig>`
- `storage: Option<StorageConfig>`

**IMPORTANT**: `StreamConfig` does NOT contain `silver_etl`. The `silver_etl` section exists only in YAML files and is NOT synced to etcd as part of the StreamConfig struct.

---

## 2. Silver ETL Subscriber Analysis

### SilverSubscriber Location

**File**: `/workspaces/neural-data-platform/core/src/subscribers/silver.rs`

### SilverSubscriberConfig (Lines 49-101)

```rust
pub struct SilverSubscriberConfig {
    pub subscriber_id: String,
    pub stream_filter: HashSet<String>,
    pub etl_configs: HashMap<String, SilverEtlConfig>,  // <-- KEY FIELD
    pub catch_up: CatchUpConfig,
    pub batch_size: usize,
    pub flush_interval_secs: u64,
}
```

The `etl_configs` HashMap maps `stream_id` -> `SilverEtlConfig`. This is populated externally, NOT loaded by the subscriber itself.

### Event Bus Subscription Pattern (Lines 526-576)

The SilverSubscriber receives `RawDataPoint` events via tokio broadcast:

```rust
impl Subscriber for SilverSubscriber {
    async fn start(&mut self, mut receiver: broadcast::Receiver<Arc<RawDataPoint>>) {
        // ...
        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(raw_point) => {
                            self.process_event(raw_point).await?;
                        }
                        // ...
                    }
                }
            }
        }
    }
}
```

### Configuration Usage in Transform (Lines 328-356)

When processing events, SilverSubscriber looks up ETL config by stream_id:

```rust
fn transform_point(&self, raw: &RawDataPoint) -> Result<Option<SilverRecord>, SubscriberError> {
    let stream_id = Self::extract_stream_id(&raw.source_id);

    // Get ETL config for this stream
    let etl_config = match self.config.etl_configs.get(&stream_id) {
        Some(cfg) => cfg,
        None => {
            debug!(stream_id = %stream_id, "No ETL config for stream, skipping");
            return Ok(None);  // SILENT SKIP!
        }
    };
    // ... transform using etl_config
}
```

**CRITICAL BUG**: If `etl_configs` is empty or missing the stream, events are silently skipped with only a debug-level log. No error is raised.

---

## 3. How Silver ETL Config is Loaded (THE BUG)

### Current Loading in main.rs (Lines 500-629)

The `create_silver_subscribers()` function in `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` loads Silver ETL config:

```rust
async fn create_silver_subscribers(
    _event_bus: Arc<EventBus>,
    registry: Arc<StreamRegistry>,
) -> Result<Vec<Box<dyn Subscriber>>, Box<dyn std::error::Error + Send + Sync>> {
    // ...

    // Get config directory (YAML files)
    let config_dir = std::env::var("STREAM_CONFIG_DIR")
        .unwrap_or_else(|_| "/workspaces/neural-data-platform/config/base/streams".to_string());

    // Get stream list from etcd
    let streams = registry.list_streams().await.unwrap_or_default();

    for stream_id in &streams {
        // BUG: Loads from YAML file, not etcd!
        if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, stream_id).await {
            if silver_config.enabled {
                // ...
            }
        }
    }
}
```

### The load_silver_etl_config Function (Lines 602-629)

```rust
async fn load_silver_etl_config(
    config_dir: &str,
    stream_id: &str,
) -> Result<Option<SilverEtlConfig>, Box<dyn std::error::Error + Send + Sync>> {
    // Constructs path to YAML file
    let dir_path = Path::new(config_dir).join(stream_id).join("config.yaml");
    let flat_path = Path::new(config_dir).join(format!("{}.yaml", stream_id));

    let yaml_path = if dir_path.exists() {
        dir_path
    } else if flat_path.exists() {
        flat_path
    } else {
        return Ok(None);  // SILENT RETURN if no file!
    };

    // Read YAML file
    let contents = tokio::fs::read_to_string(&yaml_path).await?;

    #[derive(serde::Deserialize)]
    struct StreamConfigWithSilver {
        #[serde(default)]
        silver_etl: Option<SilverEtlConfig>,
    }

    let config: StreamConfigWithSilver = serde_yaml::from_str(&contents)?;
    Ok(config.silver_etl)  // Returns None if silver_etl missing!
}
```

**THE BUG EXPLAINED**:
1. Stream IDs are discovered from etcd via `registry.list_streams()`
2. Silver ETL config is loaded from YAML files on disk
3. If YAML file doesn't exist or doesn't have `silver_etl` section, returns `None`
4. No error is logged - just silent skip
5. Result: Silver ETL silently does nothing for streams without matching YAML files

---

## 4. Bronze vs Silver Config Loading Comparison

### Bronze Subscriber Config Loading

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (Lines 333-346)

```rust
// Create and register BronzeSubscriber
let bronze_config = BronzeSubscriberConfig {
    batch_size: 50,
    flush_interval_secs: 30,
    max_retries: 3,
    stream_filter: Vec::new(), // Accept all streams
};
let bronze_subscriber = BronzeSubscriber::new("bronze-parquet", bronze_config, store.clone());
```

Bronze subscriber does NOT need stream-specific config from etcd - it:
- Accepts all streams (no filter)
- Uses fixed batch settings
- Writes raw data to Parquet without transformation

The stream configuration (fields, sources, etc.) is used by the **IngestionCoordinator** via StreamRegistry, not by BronzeSubscriber directly.

### Silver Subscriber Config Loading

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (Lines 569-587)

```rust
for stream_id in streams {
    // READS FROM YAML FILE
    if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, &stream_id).await {
        if silver_config.enabled {
            let mut etl_configs = HashMap::new();
            etl_configs.insert(stream_id.clone(), silver_config);

            let subscriber_config = SilverSubscriberConfig {
                subscriber_id: format!("silver-{}", stream_id),
                stream_filter: HashSet::from([stream_id.clone()]),
                etl_configs,
                ..Default::default()
            };
            // ...
        }
    }
}
```

Silver subscriber REQUIRES stream-specific config because it:
- Transforms data using field_mappings
- Applies DQ rules
- Maps to specific target tables
- Needs timestamp transform configuration

### The Asymmetry

| Aspect | Bronze | Silver |
|--------|--------|--------|
| Config Source | None (fixed settings) | YAML files on disk |
| Stream Discovery | N/A (accepts all) | etcd via StreamRegistry |
| Requires StreamConfig | No | Yes (silver_etl section) |
| Error Handling | N/A | Silent skip |

---

## 5. Current YAML Config Structure

### Sample Stream Config (air-quality)

**File**: `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml`

```yaml
# Top-level stream config (synced to etcd)
stream_id: "air-quality"
description: "AirGradient sensor readings from MQTT"
version: "1.0.0"
enabled: true
retention_days: 365
# ...

# Bronze schema fields
fields:
  pm25:
    type: "float"
    unit: "µg/m³"
    # ...

# Data sources
sources:
  - type: mqtt
    enabled: true
    # ...

# Entity schemas for data dictionary
entity_schemas:
  - schema_name: airgradient
    description: AirGradient indoor air quality sensors
    attributes:
      - name: pm25
        type: float
        # ...

# Silver ETL Configuration (NOT synced to etcd!)
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
    # ...

  dq_rules:
    - rule: cross_field_check
      name: pm10_gte_pm25
      expression: "..."
    # ...

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert
```

### Key Sections

| Section | Purpose | Synced to etcd? |
|---------|---------|-----------------|
| `stream_id`, `description`, etc. | Stream metadata | Yes |
| `fields` | Bronze schema definition | Yes |
| `sources` | Data source configuration | Yes |
| `entity_schemas` | Data dictionary entries | Yes (separate sync) |
| `silver_etl` | Silver layer ETL config | **NO** |

---

## 6. SilverEtlConfig Structure

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`

```rust
pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,
    pub target_schema: Option<String>,
    pub timestamp: TimestampMapping,
    pub valid_timestamp: Option<ValidTimestampMapping>,
    pub pre_transform: Option<PreTransformConfig>,
    pub identity_fields: Vec<IdentityField>,
    pub field_mappings: Vec<SilverFieldMapping>,
    pub dq_rules: Vec<DqRule>,
    pub dq_output: DqOutputConfig,
    pub deduplication: DeduplicationConfig,
    pub incremental: IncrementalConfig,
}
```

This struct is parsed from the `silver_etl:` section of YAML files. It is NOT part of the `StreamConfig` struct used by etcd.

---

## 7. Root Cause Analysis

### Why Silver ETL Fails Silently

1. **Dual Source Discovery**:
   - Streams are discovered from etcd (`registry.list_streams()`)
   - Silver config is loaded from YAML files (`load_silver_etl_config()`)
   - These sources can be out of sync

2. **Missing Error Propagation**:
   - `load_silver_etl_config()` returns `Ok(None)` when file is missing
   - No warning or error is logged
   - Caller treats `None` as "no config for this stream"

3. **StreamConfig Doesn't Include silver_etl**:
   - The `StreamConfig` struct in `neural_core` has no `silver_etl` field
   - StreamRegistry syncs only the fields defined in `StreamConfig`
   - `silver_etl` section is effectively orphaned in YAML files

4. **Config Sync Doesn't Sync silver_etl**:
   - `ConfigSyncService` in air-quality-app syncs YAML to etcd
   - `StreamConfigYaml` captures `silver_etl` in `extra: HashMap<String, Value>` via `#[serde(flatten)]` (line 275)
   - But `to_stream_config()` (lines 346-515) only extracts: fields, sources, storage
   - `silver_etl` is captured but **never extracted or synced**
   - Result: `silver_etl` exists only in YAML, never in etcd

---

## 8. Files Involved

### Configuration Loading

| File | Role | Lines |
|------|------|-------|
| `/workspaces/neural-data-platform/config-client/src/client.rs` | etcd client wrapper | 1-337 |
| `/workspaces/neural-data-platform/config-client/src/stream/registry.rs` | StreamRegistry | 1-421 |
| `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` | Creates Silver subscribers | 500-629 |
| `/workspaces/neural-data-platform/apps/air-quality-app/src/stream_integration.rs` | Stream config loading | 1-185 |

### Silver Subscriber

| File | Role | Lines |
|------|------|-------|
| `/workspaces/neural-data-platform/core/src/subscribers/silver.rs` | SilverSubscriber impl | 1-976 |
| `/workspaces/neural-data-platform/core/src/config/silver_etl.rs` | SilverEtlConfig types | 1-2071 |

### Configuration Files

| File | Contains |
|------|----------|
| `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml` | Full config with silver_etl |
| `/workspaces/neural-data-platform/config/base/streams/outdoor-weather/config.yaml` | Full config with silver_etl |

---

## 9. Recommendations for dp-018

### Immediate Fix (Task 1.3)

1. **Add SilverEtlConfig to StreamConfig or create separate key**:
   ```
   /streams/{stream_id}/silver_etl -> SilverEtlConfig JSON
   ```

2. **Update load_silver_etl_config to read from etcd**:
   ```rust
   async fn load_silver_etl_config(
       client: &ConfigClient,
       stream_id: &str,
   ) -> Result<Option<SilverEtlConfig>, ConfigError> {
       let key = format!("/streams/{}/silver_etl", stream_id);
       match client.get::<SilverEtlConfig>(&key).await {
           Ok(config) => Ok(Some(config)),
           Err(ConfigError::NotFound(_)) => Ok(None),
           Err(e) => Err(e),
       }
   }
   ```

3. **Update config sync to include silver_etl**:
   - Modify `ConfigSyncService` to sync `silver_etl` as separate key
   - Or extend `StreamConfig` to include optional `silver_etl`

### Error Visibility (Task 1.7)

1. Change `debug!` to `warn!` when ETL config is missing
2. Log total streams found vs. streams with ETL config
3. Consider making missing ETL config an error for enabled streams

### Unified ConfigLoader (Task 1.1-1.2)

Create trait that encapsulates both stream config and silver ETL config loading:

```rust
pub trait ConfigLoader: Send + Sync {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<Option<SilverEtlConfig>, ConfigError>;
    async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;
}
```

---

## 10. Verification Queries

To verify the current state:

```bash
# Check what's in etcd for a stream
etcdctl get --prefix /streams/air-quality | head -50

# Check if silver_etl is synced (it won't be)
etcdctl get /streams/air-quality/silver_etl

# Check YAML file has silver_etl
grep -A 5 "silver_etl:" /workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml
```

---

---

## 11. Config Sync Service Analysis

### Location
**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/config_sync/service.rs`

### Key Structures

**StreamConfigYaml** (lines 252-275):
```rust
struct StreamConfigYaml {
    stream_id: String,
    description: String,
    // ... standard fields ...
    fields: FieldsYaml,
    sources: Vec<SourceYaml>,
    #[serde(flatten)]
    extra: HashMap<String, serde_yaml::Value>,  // <-- silver_etl lands here
}
```

The `#[serde(flatten)]` directive captures unknown fields like `silver_etl`, `entity_schemas`, etc. into the `extra` HashMap.

**to_stream_config()** (lines 346-515):
```rust
fn to_stream_config(&self) -> Result<StreamConfig, ConfigSyncError> {
    // Converts fields (map or array)
    // Converts sources
    // Extracts storage from extra

    Ok(StreamConfig {
        stream_id,
        description,
        version,
        enabled,
        retention_days,
        compression_after_days,
        partitioning_strategy,
        fields,
        sources,
        storage,
        // NOTE: No silver_etl field!
    })
}
```

### What Gets Synced vs. What Doesn't

| YAML Section | Synced to etcd? | Why |
|--------------|-----------------|-----|
| `stream_id` | Yes | Part of StreamConfig |
| `description` | Yes | Part of StreamConfig |
| `fields` | Yes | Part of StreamConfig |
| `sources` | Yes | Part of StreamConfig |
| `storage` | Yes | Extracted from `extra` |
| `entity_schemas` | No | Captured in `extra` but not extracted |
| `silver_etl` | **No** | Captured in `extra` but not extracted |
| `mqtt` (legacy) | Partial | Converted to sources |

### The Gap

The sync process:
1. Reads YAML file
2. Parses into `StreamConfigYaml` (captures everything)
3. Converts to `StreamConfig` (loses silver_etl, entity_schemas)
4. Saves `StreamConfig` to etcd via `StreamRegistry`

The `silver_etl` section is **parsed but discarded** during conversion.

---

*Research completed: 2026-02-01*
*Researcher: NDP Research Agent*
*Informs: dp-018 SPARC Specification Phase*
