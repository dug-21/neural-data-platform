# Bronze Layer Configuration System Research

## Executive Summary

This document details how new streams are configured and consumed at the Bronze layer in the Neural Data Platform (NDP). The system follows a **GitOps-driven architecture** where YAML configuration files are synced to etcd, then consumed by the application to spawn data sources and write raw data to Parquet files.

---

## 1. YAML Config Structure

### Location
Stream configurations live in: `config/base/streams/{stream-id}/config.yaml`

### Example Streams Found
- `air-quality/config.yaml` - AirGradient MQTT sensors
- `outdoor-weather/config.yaml` - OpenWeatherMap HTTP API
- `nws-forecast-hourly/config.yaml` - NWS HTTP API with array parsing
- `home-assistant-state/config.yaml` - Home Assistant MQTT with plain text payloads
- `outdoor-air-quality/config.yaml` - OpenWeatherMap Air Pollution API

### Required Top-Level Sections

| Section | Required | Description | File Reference |
|---------|----------|-------------|----------------|
| `stream_id` | Yes | Unique identifier (kebab-case, 3-64 chars) | `config/base/streams/air-quality/config.yaml:4` |
| `description` | Yes | Human-readable description | Lines 5 |
| `version` | No (default: "1.0.0") | Semver version | Lines 6 |
| `enabled` | No (default: true) | Whether stream is active | Lines 7 |
| `retention_days` | No | Days to retain data | Lines 8 |
| `compression_after_days` | No | Days before compression | Lines 9 |
| `partitioning_strategy` | No (default: "daily") | How to partition data | Lines 10 |
| `fields` | Yes | Field definitions (array or map) | Lines 13-48 |
| `sources` | Yes | Data source configurations | Lines 52-81 |
| `storage` | No | Storage overrides (batch_size, timeout) | Lines 96-99 |

### Optional Top-Level Sections

| Section | Purpose | File Reference |
|---------|---------|----------------|
| `entity_schemas` | Data dictionary definitions (DP-002) | Lines 101-148 |
| `silver_etl` | Silver layer ETL configuration (DP-006) | Lines 150-318 |
| `pipeline_health` | Freshness thresholds for monitoring | `home-assistant-state/config.yaml:168-176` |
| `mqtt` (legacy) | Backward-compatible MQTT config | Lines 83-94 |

### Fields Section Format

Fields can be defined as **array** (preferred) or **map** (legacy):

**Array Format (new):**
```yaml
# From outdoor-weather/config.yaml:9-64
fields:
  - name: temperature
    type: float
    nullable: false
    unit: celsius
    range: [-50.0, 60.0]
```

**Map Format (legacy):**
```yaml
# From air-quality/config.yaml:13-48
fields:
  pm25:
    type: "float"
    unit: "ug/m3"
    description: "Particulate Matter 2.5 micrometers"
    nullable: false
```

**Field Types:** `float`, `int`, `string`, `bool`, `json`

### Sources Section Structure

Each source in the `sources` array has:

```yaml
# From air-quality/config.yaml:52-81
sources:
  - type: mqtt           # Required: mqtt, http_poll, webhook, file_watch, csv
    enabled: true        # Optional (default: true)
    ndp_id: "aq_airgradient_1"  # AIR-009: Stable source identifier
    context:             # AIR-009: Mutable metadata
      device_type: airgradient
      location:
        coordinates: [29.95838, -81.30878]
        type: indoor
        path: /beachhouse/livingroom
    # Source-type specific params (flattened):
    broker_url: "mosquitto"
    port: 1883
    topic_pattern: "airgradient/readings/+"
    parser:              # Parser configuration
      parser_type: flat_json
      location_id_field: serialno
```

### Source Types and Their Parameters

| Type | Key Parameters | Example Stream |
|------|---------------|----------------|
| `mqtt` | `broker_url`, `port`, `topic_pattern`, `qos`, `parser` | air-quality |
| `http_poll` | `poll_interval_secs`, `timeout_secs`, `endpoints[]`, `parser_name`, `parser` | outdoor-weather |

### Silver ETL Section (extra)

The `silver_etl` section defines Bronze-to-Silver ETL transformation:

```yaml
# From air-quality/config.yaml:150-318
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  description: "Indoor air quality measurements"
  grain: "One row per sensor reading"

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

  dq_rules:
    - rule: cross_field_check
    - rule: freshness_check
    - rule: rate_of_change

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert
```

---

## 2. ConfigSyncService

### Location
`apps/air-quality-app/src/config_sync/service.rs`

### Purpose
Syncs YAML configuration files from the filesystem to etcd via StreamRegistry.

### Key Components

#### Struct Definition (Lines 95-97)
```rust
pub struct ConfigSyncService {
    config_dir: PathBuf,
}
```

#### YAML Loading (Lines 112-142)
```rust
pub async fn load_yaml_config(
    &self,
    yaml_path: impl AsRef<Path>,
) -> Result<StreamConfig, ConfigSyncError>
```

**Process:**
1. Read file content via `tokio::fs::read_to_string()`
2. Parse YAML into `StreamConfigYaml` struct via serde_yaml
3. Convert to `StreamConfig` via `yaml_config.to_stream_config()`
4. Validate via `config.validate()`

#### Config Discovery (Lines 145-195)
```rust
pub async fn discover_stream_configs(&self) -> Result<Vec<PathBuf>, ConfigSyncError>
```

**Discovery Pattern:**
- Recursively walks subdirectories under `config_dir`
- Looks for `config.yaml` files in each subdirectory
- Returns list of discovered config file paths

#### Sync to Registry (Lines 198-213)
```rust
pub async fn save_to_registry(
    &self,
    registry: &StreamRegistry,
    config: &StreamConfig,
) -> Result<(), ConfigSyncError>
```

**Process:**
1. Validate config
2. Call `registry.save_stream(config)` to persist to etcd

#### Full Sync (Lines 216-249)
```rust
pub async fn sync_all(&self, registry: &StreamRegistry) -> Result<usize, ConfigSyncError>
```

**Process:**
1. Discover all YAML configs
2. For each config:
   - Load and parse
   - Skip if `enabled: false`
   - Save to registry
3. Continue on error (logs warning, doesn't fail entire sync)
4. Return count of synced configs

### Validation

Validation occurs at multiple points:

1. **YAML Parsing** (Line 129): `serde_yaml::from_str()` - validates syntax
2. **Config Conversion** (Line 132): `to_stream_config()` - validates field types, source types
3. **StreamConfig Validation** (Line 135): `config.validate()` - validates:
   - Stream ID format (kebab-case, 3-64 chars)
   - At least one field
   - At least one source
   - Field name format (snake_case, 1-64 chars)

### Error Handling

```rust
// From Lines 11-29
pub enum ConfigSyncError {
    YamlReadError(String),      // File not found
    YamlParseError(String),     // Invalid YAML syntax
    InvalidConfig(String),       // Validation failure
    RegistryError(String),       // etcd write failure
    IoError(std::io::Error),     // Filesystem error
    DirectoryNotFound(String),   // Config dir missing
}
```

**Failure Modes:**
1. **Directory not found**: Returns `DirectoryNotFound` immediately
2. **File read error**: Logs warning, continues with other files
3. **YAML parse error**: Logs warning, continues with other files
4. **Validation error**: Logs warning, continues with other files
5. **Registry error**: Logs warning, continues with other files

---

## 3. StreamRegistry

### Location
`config-client/src/stream/registry.rs`

### Purpose
Manages stream configurations in etcd with caching.

### Key Components

#### Struct Definition (Lines 9-12)
```rust
pub struct StreamRegistry {
    client: ConfigClient,
    cache: Arc<RwLock<HashMap<String, StreamConfig>>>,
}
```

#### Initialization (Lines 16-27)
```rust
pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError>
```

Creates ConfigClient with prefix `/streams` for all operations.

#### List Streams (Lines 62-84)
```rust
pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError>
```

**Process:**
1. Call `client.list("/")` to get all keys under `/streams/`
2. Filter for keys matching pattern `/streams/{stream_id}/config`
3. Extract unique stream_id values
4. Return deduplicated list

**etcd Key Pattern:**
```
/streams/air-quality/config
/streams/outdoor-weather/config
/streams/nws-forecast-hourly/config
```

#### Load Stream (Lines 30-59)
```rust
pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>
```

**Process:**
1. Check cache first
2. If not cached, load from etcd key `/{stream_id}/config`
3. Validate loaded config
4. Update cache
5. Return config

#### Save Stream (Lines 122-141)
```rust
pub async fn save_stream(&self, config: &StreamConfig) -> Result<(), ConfigError>
```

**Process:**
1. Validate config
2. Write to etcd key `/{stream_id}/config`
3. Update cache

### StreamConfig Struct

Defined in `core/src/types/stream_config.rs:368-405`:

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
}
```

### SourceConfig Struct

Defined in `core/src/types/stream_config.rs:191-214`:

```rust
pub struct SourceConfig {
    pub source_type: SourceType,
    pub enabled: bool,
    pub ndp_id: Option<String>,     // AIR-009: Stable source identifier
    pub context: Option<serde_json::Value>,  // AIR-009: Mutable metadata
    pub params: HashMap<String, serde_json::Value>,  // Flattened params
}
```

---

## 4. Bronze Subscriber / Data Flow

### SourceManager

**Location:** `apps/air-quality-app/src/coordinator/source_manager.rs`

**Purpose:** Spawns and manages data sources based on StreamConfig.

#### Initialization (Lines 99-128)
```rust
pub async fn start_all_sources(&mut self) -> Result<(), SourceManagerError>
```

**Process:**
1. Get list of streams from registry: `registry.list_streams()`
2. For each stream, load config: `registry.load_stream(&stream_id)`
3. Skip disabled streams (`config.enabled == false`)
4. For each enabled source in stream: `spawn_source()`

#### Source Spawning (Lines 151-371)

```rust
async fn spawn_source(
    &mut self,
    stream_id: &str,
    source_config: &SourceConfig,
) -> Result<String, SourceManagerError>
```

**Process:**
1. Generate internal ID: `"{stream_id}-{:?}", source_type`
2. Check for existing source (stop if exists)
3. Get EventBus reference
4. Parse source-type-specific config
5. Spawn tokio task for source
6. Store SourceInfo in HashMap

#### MQTT Source Flow (Lines 803-870)
```rust
async fn run_mqtt_source(
    stream_id: String,
    config: MqttConfig,
    parser_config: ParserConfig,
    event_bus: Arc<EventBus>,
    ...
)
```

1. Create parser from config
2. Create MqttSource with `with_raw_config()`
3. Start source: `source.start()`
4. Poll loop every 100ms:
   - Fetch raw batch: `source.fetch_raw_batch()`
   - Publish each point to EventBus: `event_bus.publish(Arc::new(raw_point))`

#### HTTP Source Flow (Lines 872-942)
```rust
async fn run_generic_http_polling_source(
    stream_id: String,
    config: GenericHttpPollingConfig,
    parser_config: ParserConfig,
    event_bus: Arc<EventBus>,
    ...
)
```

1. Create parser from config
2. Create GenericHttpPollingSource with `with_raw_config()`
3. Poll loop at `config.poll_interval`:
   - Fetch raw batch: `source.fetch_raw_batch()`
   - Publish each point to EventBus

### BronzeSubscriber

**Location:** `core/src/subscribers/bronze.rs`

**Purpose:** Consumes RawDataPoint events from EventBus and writes to Parquet.

#### Configuration (Lines 39-56)
```rust
pub struct BronzeSubscriberConfig {
    pub batch_size: usize,         // Default: 100
    pub flush_interval_secs: u64,  // Default: 5
    pub max_retries: u32,          // Default: 3
    pub stream_filter: Vec<String>, // Empty = accept all
}
```

#### Event Processing (Lines 206-287)
```rust
async fn start(
    &mut self,
    mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
) -> Result<(), SubscriberError>
```

**Process:**
1. Subscribe to EventBus
2. Event loop with flush timer:
   - On event: add to buffer, flush if batch full
   - On timer tick: flush buffer
   - On cancellation: final flush and exit

#### Flush with Retry (Lines 125-179)
```rust
async fn flush(&mut self) -> Result<(), SubscriberError>
```

1. Drain buffer
2. Retry up to `max_retries` times with exponential backoff
3. Write via `store.write_raw_batch(batch)`

---

## 5. Data Flow Diagram

```
                                    STARTUP
                                       |
                                       v
+------------------+    YAML Files    +----------------------+
|  config/base/    | --------------> | ConfigSyncService    |
|  streams/*.yaml  |   discover_     | (air-quality-app)    |
+------------------+   stream_       +----------------------+
                      configs()               |
                                              | sync_all()
                                              v
                                    +----------------------+
                                    |   StreamRegistry     |
                                    |   (config-client)    |
                                    +----------------------+
                                              |
                                              | save_stream()
                                              v
                                    +----------------------+
                                    |        etcd          |
                                    | /streams/{id}/config |
                                    +----------------------+
                                              ^
                                              |
                                              | load_stream()
                                              |
                                    +----------------------+
                                    |    SourceManager     |
                                    | start_all_sources()  |
                                    +----------------------+
                                              |
                      +------------------------------------------+
                      |                       |                  |
               spawn MQTT           spawn HTTP Poll        spawn Webhook
                      |                       |                  |
                      v                       v                  v
               +-------------+        +-------------+      +-----------+
               | MqttSource  |        | HttpSource  |      | (future)  |
               +-------------+        +-------------+      +-----------+
                      |                       |
                      +----------+------------+
                                 |
                                 | RawDataPoint
                                 v
                        +----------------+
                        |   EventBus     |
                        |  (broadcast)   |
                        +----------------+
                                 |
                 +---------------+---------------+
                 |                               |
                 v                               v
        +------------------+            +------------------+
        | BronzeSubscriber |            | SilverSubscriber |
        +------------------+            +------------------+
                 |                               |
                 v                               v
        +------------------+            +------------------+
        | ParquetStore     |            | TimescaleDB      |
        | /data/raw/{id}/  |            | silver.table     |
        +------------------+            +------------------+
```

---

## 6. Adding a New Stream: Step-by-Step

### Step 1: Create YAML Configuration

Create `config/base/streams/{stream-id}/config.yaml`:

```yaml
stream_id: "my-new-stream"
description: "Description of my stream"
version: "1.0.0"
enabled: true
retention_days: 90
partitioning_strategy: daily

fields:
  - name: my_field
    type: float
    nullable: false
    unit: units

sources:
  - type: http_poll  # or mqtt
    enabled: true
    ndp_id: "my-source-001"
    poll_interval_secs: 600
    endpoints:
      - endpoint_id: my_endpoint
        url: "https://api.example.com/data"
        location_id: home
    parser:
      parser_type: flat_json
      location_id_field: id

storage:
  batch_size: 50
  batch_timeout_secs: 30
```

### Step 2: Application Startup (Automatic)

On startup, the application:

1. **ConfigSyncService** discovers and syncs YAML to etcd:
   ```rust
   // main.rs:147-168
   let sync_service = ConfigSyncService::new(&config_dir);
   sync_service.sync_all(&registry).await;
   ```

2. **SourceManager** loads configs and spawns sources:
   ```rust
   // main.rs:314
   source_manager.start_all_sources().await;
   ```

3. **BronzeSubscriber** starts receiving events:
   ```rust
   // main.rs:339-345
   let bronze_subscriber = BronzeSubscriber::new("bronze-parquet", config, store);
   subscriber_coordinator.register(Box::new(bronze_subscriber));
   subscriber_coordinator.start_all().await;
   ```

### Step 3: Verify

1. Check etcd for config:
   ```bash
   etcdctl get /streams/my-new-stream/config
   ```

2. Check application logs for source startup
3. Check `/data/raw/my-new-stream/` for Parquet files

---

## 7. Failure Modes

### Configuration Failures

| Failure | Detection Point | Behavior |
|---------|-----------------|----------|
| Invalid stream_id format | `StreamConfig.validate()` | Sync skipped, warning logged |
| No fields defined | `StreamConfig.validate()` | Sync skipped, warning logged |
| No sources defined | `StreamConfig.validate()` | Sync skipped, warning logged |
| Unknown source type | `parse_source_type()` | Sync skipped, warning logged |
| Unknown field type | `parse_field_type()` | Sync skipped, warning logged |
| YAML syntax error | `serde_yaml::from_str()` | Sync skipped, warning logged |
| etcd connection failure | `StreamRegistry::new()` | Application startup fails |
| etcd write failure | `registry.save_stream()` | Sync skipped, warning logged |

### Runtime Failures

| Failure | Detection Point | Behavior |
|---------|-----------------|----------|
| MQTT broker unreachable | `MqttSource.start()` | Reconnect with exponential backoff |
| HTTP endpoint timeout | `GenericHttpPollingSource.fetch_raw_batch()` | Log warning, retry next interval |
| EventBus no subscribers | `event_bus.publish()` | Events dropped (logged as debug) |
| Parquet write failure | `BronzeSubscriber.flush()` | Retry 3x with backoff, then log error |
| Stream disabled | `config.enabled == false` | Stream skipped entirely |

### Recovery Patterns

1. **Graceful degradation**: Individual stream failures don't affect other streams
2. **Retry with backoff**: Network failures use exponential backoff
3. **Hot reload**: `SourceManager.update_sources_for_stream()` can reload config
4. **Batch flush**: Timer-based flush ensures data written even with low volume

---

## 8. Key File References

| File | Purpose | Key Lines |
|------|---------|-----------|
| `config/base/streams/*/config.yaml` | Stream definitions | All |
| `apps/air-quality-app/src/config_sync/service.rs` | YAML sync | 95-97, 112-142, 216-249 |
| `config-client/src/stream/registry.rs` | etcd registry | 9-12, 30-59, 62-84, 122-141 |
| `config-client/src/client.rs` | etcd client | 29-41, 50-57, 73-90 |
| `core/src/types/stream_config.rs` | StreamConfig struct | 368-405, 191-214 |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Source spawning | 99-128, 151-371, 803-870 |
| `apps/air-quality-app/src/main.rs` | Startup orchestration | 137-168, 257-420 |
| `core/src/subscribers/bronze.rs` | Parquet writes | 39-56, 206-287 |

---

## 9. Appendix: Parser Types

The parser configuration in sources determines how raw data is transformed:

| Parser Type | Use Case | Example Stream |
|-------------|----------|----------------|
| `flat_json` | Simple JSON with direct field mapping | air-quality, home-assistant-state |
| `json_path` | Nested JSON with path-based extraction | outdoor-weather |
| `array_iterator` | JSON arrays that expand to multiple points | nws-forecast-hourly |

Parser configs are defined in the `parser` section of each source and processed by the `create_parser_from_config()` function in `core/src/parsers/traits.rs`.
