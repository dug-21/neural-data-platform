# AIR-004: Stream Registry Integration Architecture

## Executive Summary

This document specifies the **minimal integration** of StreamRegistry into the air-quality-app. The design principle is: **Don't refactor what works**. We add StreamRegistry as an optional enhancement layer while preserving the existing etcd config loading fallback.

## 1. Component Diagram (Data Flow)

```
┌─────────────────────────────────────────────────────────────────────┐
│                           main.rs (Startup)                          │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Configuration Loading Pipeline                    │
│  Priority: StreamRegistry > Legacy Etcd > config.yaml > defaults    │
└─────────────────────────────────────────────────────────────────────┘
        │                         │                         │
        ▼                         ▼                         ▼
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│ StreamRegistry│        │ Legacy Etcd  │         │  config.yaml │
│ (NEW)         │        │ (EXISTING)   │         │  (EXISTING)  │
└──────────────┘         └──────────────┘         └──────────────┘
        │                         │                         │
        │   /streams/             │   /air-quality/         │
        │   air-quality/config    │   mqtt/*, storage/*     │
        │                         │                         │
        ▼                         ▼                         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Unified AppConfig                               │
│  Contains: server, mqtt, storage (existing structure preserved)     │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
         ┌───────────────────────┼───────────────────────┐
         ▼                       ▼                       ▼
┌──────────────┐        ┌──────────────┐        ┌──────────────┐
│ MqttHandler  │        │ ParquetStore │        │ StorageWriter│
│              │        │              │        │              │
│ (MQTT Config)│        │ (Storage     │        │ (Batch/      │
│              │        │  base_path)  │        │  Timeout)    │
└──────────────┘        └──────────────┘        └──────────────┘
```

## 2. Integration Points (Exact Code Locations)

### 2.1 Where to Insert StreamRegistry Logic

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

**Location**: Lines 24-65 (Configuration loading section)

**Current Flow**:
```rust
// Line 25-51: Try etcd
let config = match air_quality_app::load_from_etcd().await {
    Ok(etcd_config) => { /* Convert to AppConfig */ }
    Err(e) => {
        // Line 52-64: Fallback to config.yaml
        match AppConfig::from_yaml("config.yaml") { ... }
    }
}
```

**New Flow** (MINIMAL change):
```rust
// STEP 1: Try StreamRegistry (NEW)
let config = match load_from_stream_registry().await {
    Ok(registry_config) => {
        tracing::info!("Loaded configuration from StreamRegistry");
        registry_config
    }
    Err(e) => {
        tracing::warn!("StreamRegistry unavailable: {}. Falling back to legacy etcd...", e);

        // STEP 2: Try legacy etcd (EXISTING - unchanged)
        match air_quality_app::load_from_etcd().await {
            Ok(etcd_config) => {
                tracing::info!("Loaded configuration from etcd");
                // Convert EtcdAppConfig to AppConfig (EXISTING code - unchanged)
                AppConfig { ... }
            }
            Err(e) => {
                // STEP 3: Try config.yaml (EXISTING - unchanged)
                tracing::warn!("Failed to load config from etcd: {}. Trying config.yaml...", e);
                match AppConfig::from_yaml("config.yaml") {
                    Ok(cfg) => { ... }
                    Err(e) => { ... }
                }
            }
        }
    }
};
```

### 2.2 New Function to Add

**Location**: Add to `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (after imports, before `main()`)

```rust
use config_client::stream::StreamRegistry;
use neural_core::StreamConfig;

/// Load configuration from StreamRegistry
/// Maps StreamConfig to AppConfig
async fn load_from_stream_registry() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let etcd_endpoint = std::env::var("ETCD_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:2379".to_string());

    // Initialize StreamRegistry (uses /streams prefix internally)
    let registry = StreamRegistry::new(&[&etcd_endpoint]).await?;

    // Load air-quality stream config
    let stream_config: StreamConfig = registry.load_stream("air-quality").await?;

    // Convert StreamConfig to AppConfig
    stream_config_to_app_config(stream_config)
}
```

### 2.3 Config Mapping Function

**Location**: Add to `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

```rust
/// Convert StreamConfig to AppConfig
fn stream_config_to_app_config(stream: StreamConfig) -> Result<AppConfig, Box<dyn std::error::Error>> {
    // Extract MQTT source config
    let mqtt_source = stream.sources.iter()
        .find(|s| matches!(s.source_type, neural_core::SourceType::Mqtt))
        .ok_or("No MQTT source found in stream config")?;

    // Extract MQTT parameters
    let broker_url = mqtt_source.params.get("broker_url")
        .and_then(|v| v.as_str())
        .ok_or("Missing broker_url in MQTT source")?
        .to_string();

    let port = mqtt_source.params.get("port")
        .and_then(|v| v.as_u64())
        .unwrap_or(1883) as u16;

    let client_id = mqtt_source.params.get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("air-quality-app")
        .to_string();

    let topic_pattern = mqtt_source.params.get("topic_pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("airgradient/readings/+")
        .to_string();

    let qos = mqtt_source.params.get("qos")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u8;

    let buffer_capacity = mqtt_source.params.get("buffer_capacity")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000) as usize;

    // Extract storage config (with sensible defaults)
    let storage = stream.storage.as_ref();
    let base_path = std::env::var("DATA_DIR")
        .unwrap_or_else(|_| "./data/parquet".to_string());

    let batch_size = storage
        .map(|s| s.batch_size)
        .unwrap_or(100);

    let batch_timeout_secs = storage
        .map(|s| s.batch_timeout_secs)
        .unwrap_or(5);

    // Server config from env vars (not in StreamConfig)
    let server_host = std::env::var("AIR_QUALITY_SERVER_HOST")
        .unwrap_or_else(|_| "0.0.0.0".to_string());

    let server_port = std::env::var("AIR_QUALITY_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    Ok(AppConfig {
        server: air_quality_app::config::ServerConfig {
            host: server_host,
            port: server_port,
        },
        mqtt: air_quality_app::config::MqttConfig {
            broker_url,
            port,
            client_id,
            topic_pattern,
            qos,
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 30,
            buffer_capacity,
        },
        storage: air_quality_app::config::StorageConfig {
            base_path,
            wal_enabled: true,
            batch_size,
            batch_timeout_secs,
        },
    })
}
```

## 3. Config Mapping (StreamConfig → AppConfig)

### 3.1 Field Mapping Table

| **AppConfig Field**                  | **Source**                                    | **Fallback**          |
|--------------------------------------|-----------------------------------------------|-----------------------|
| `server.host`                        | `ENV:AIR_QUALITY_SERVER_HOST`                 | `"0.0.0.0"`          |
| `server.port`                        | `ENV:AIR_QUALITY_SERVER_PORT`                 | `8080`               |
| `mqtt.broker_url`                    | `stream.sources[mqtt].params.broker_url`      | **ERROR** (required) |
| `mqtt.port`                          | `stream.sources[mqtt].params.port`            | `1883`               |
| `mqtt.client_id`                     | `stream.sources[mqtt].params.client_id`       | `"air-quality-app"`  |
| `mqtt.topic_pattern`                 | `stream.sources[mqtt].params.topic_pattern`   | `"airgradient/readings/+"` |
| `mqtt.qos`                           | `stream.sources[mqtt].params.qos`             | `1`                  |
| `mqtt.reconnect_delay_secs`          | `stream.sources[mqtt].params.reconnect_delay` | `1`                  |
| `mqtt.max_reconnect_delay_secs`      | `stream.sources[mqtt].params.max_reconnect_delay` | `30`           |
| `mqtt.buffer_capacity`               | `stream.sources[mqtt].params.buffer_capacity` | `1000`               |
| `storage.base_path`                  | `ENV:DATA_DIR` or `ENV:STORAGE_PATH`          | `"./data/parquet"`   |
| `storage.wal_enabled`                | (Hardcoded)                                   | `true`               |
| `storage.batch_size`                 | `stream.storage.batch_size`                   | `100`                |
| `storage.batch_timeout_secs`         | `stream.storage.batch_timeout_secs`           | `5`                  |

### 3.2 StreamConfig MQTT Source Example

The StreamConfig in etcd at `/streams/air-quality/config` should look like:

```json
{
  "stream_id": "air-quality",
  "description": "AirGradient sensor readings",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 7,
  "partitioning_strategy": "daily",

  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "nullable": false,
      "range": [0.0, 500.0],
      "display_precision": 1
    },
    {
      "name": "temperature",
      "type": "float",
      "unit": "celsius",
      "nullable": true,
      "range": [-40.0, 60.0],
      "display_precision": 1
    }
  ],

  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "broker_url": "localhost",
      "port": 1883,
      "client_id": "air-quality-app",
      "topic_pattern": "airgradient/readings/+",
      "qos": 1,
      "buffer_capacity": 1000,
      "reconnect_delay": 1,
      "max_reconnect_delay": 30
    }
  ],

  "storage": {
    "batch_size": 100,
    "batch_timeout_secs": 5,
    "buffer_capacity": 1000
  }
}
```

**Note**: The `sources[0]` object has `"type": "mqtt"` plus all MQTT-specific params flattened due to `#[serde(flatten)]` on `SourceConfig.params`.

## 4. Fallback Strategy

### 4.1 Graceful Degradation Chain

```
┌─────────────────────────────────────────────────────────┐
│ 1. StreamRegistry (/streams/air-quality/config)         │
│    Status: PREFERRED (new unified config)               │
│    Fails if: etcd down, key missing, invalid JSON       │
└─────────────────────────────────────────────────────────┘
                          │ (on error)
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 2. Legacy Etcd (/air-quality/mqtt/*, /storage/*)        │
│    Status: FALLBACK (existing production config)        │
│    Fails if: etcd down, keys missing                    │
└─────────────────────────────────────────────────────────┘
                          │ (on error)
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 3. config.yaml (./config.yaml)                          │
│    Status: FALLBACK (local dev config)                  │
│    Fails if: file missing, invalid YAML                 │
└─────────────────────────────────────────────────────────┘
                          │ (on error)
                          ▼
┌─────────────────────────────────────────────────────────┐
│ 4. Hardcoded Defaults (AppConfig::default_config())     │
│    Status: LAST RESORT (degraded mode)                  │
│    Always succeeds                                      │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Migration Path

**Phase 1 (Current)**: StreamRegistry added, legacy etcd still primary for production
- Deploy code with StreamRegistry support
- Production uses legacy `/air-quality/*` keys (fallback #2)
- Dev/test can use StreamRegistry if `/streams/air-quality/config` exists

**Phase 2 (Future)**: Populate StreamRegistry in production
- Use `etcdctl put` or StreamRegistry API to create `/streams/air-quality/config`
- App automatically picks up StreamRegistry (fallback #1)
- Legacy config still present as backup

**Phase 3 (Optional)**: Remove legacy etcd support
- After successful StreamRegistry rollout, deprecate `config_etcd.rs`
- Update fallback chain to: StreamRegistry → config.yaml → defaults

### 4.3 Failure Handling

| **Failure Scenario**                | **Behavior**                                                |
|-------------------------------------|-------------------------------------------------------------|
| etcd completely down                | Skip StreamRegistry, skip legacy etcd, use `config.yaml`   |
| `/streams/air-quality/config` missing | Log warning, try legacy etcd `/air-quality/*`             |
| Invalid JSON in StreamConfig        | Log error with details, try legacy etcd                     |
| Missing required field (broker_url) | Return error from `stream_config_to_app_config()`, fallback |
| MQTT broker unreachable             | App starts in degraded mode (existing behavior at line 107-114) |

## 5. Extension Points (Future Source Types)

### 5.1 Architecture for Pluggable Sources

The `StreamConfig.sources` array already supports multiple source types:

```rust
pub enum SourceType {
    Mqtt,       // ← Currently supported
    HttpPoll,   // ← Future: Periodic HTTP GET
    Webhook,    // ← Future: HTTP POST endpoint
    FileWatch,  // ← Future: inotify file changes
}
```

### 5.2 Adding HTTP Poll Source (Example)

**Step 1**: Add new source config to etcd:

```json
{
  "stream_id": "air-quality",
  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "broker_url": "localhost",
      "port": 1883,
      "topic_pattern": "airgradient/readings/+"
    },
    {
      "type": "http_poll",
      "enabled": true,
      "url": "https://api.example.com/sensors",
      "interval_secs": 60,
      "auth_token": "Bearer ${API_TOKEN}"
    }
  ]
}
```

**Step 2**: Create `HttpPollHandler` (similar to `MqttHandler`):

```rust
// apps/air-quality-app/src/ingestion/http_poll.rs
pub struct HttpPollHandler {
    config: HttpPollConfig,
    tx: mpsc::Sender<TimeSeriesPoint>,
}

impl HttpPollHandler {
    pub async fn new(config: HttpPollConfig, tx: mpsc::Sender<TimeSeriesPoint>) -> Result<Self, Error> {
        // Initialize HTTP client
    }

    pub async fn run(self) -> Result<(), Error> {
        loop {
            // Poll HTTP endpoint
            // Parse response
            // Send to channel
            tokio::time::sleep(Duration::from_secs(config.interval_secs)).await;
        }
    }
}
```

**Step 3**: Update `main.rs` to spawn multiple handlers:

```rust
// In main(), after MQTT handler initialization
let mut ingestion_tasks = Vec::new();

// Spawn MQTT handler if MQTT source exists
if let Some(handler) = mqtt_handler {
    ingestion_tasks.push(tokio::spawn(async move { handler.run().await }));
}

// Spawn HTTP Poll handler if HTTP source exists
if let Some(http_config) = extract_http_poll_config(&stream_config) {
    let http_handler = HttpPollHandler::new(http_config, tx.clone()).await?;
    ingestion_tasks.push(tokio::spawn(async move { http_handler.run().await }));
}

// Await all handlers in shutdown
for task in ingestion_tasks {
    let _ = task.await;
}
```

### 5.3 Source Abstraction Pattern

**Do NOT create abstract traits yet**. Only add abstractions when we have 2+ concrete implementations.

```rust
// Future: apps/air-quality-app/src/ingestion/source.rs
#[async_trait]
pub trait DataSource {
    async fn initialize(&self) -> Result<(), Error>;
    async fn run(self, tx: mpsc::Sender<TimeSeriesPoint>) -> Result<(), Error>;
}

// Then MqttHandler and HttpPollHandler implement DataSource
```

**Current**: Keep `MqttHandler` as concrete type (lines 102-114). Add `HttpPollHandler` as separate concrete type when needed.

## 6. Code Sketch (Pseudocode)

### 6.1 Complete Integration Pseudocode

```rust
// ========== NEW IMPORTS ==========
use config_client::stream::StreamRegistry;
use neural_core::StreamConfig;

// ========== NEW FUNCTION 1: Load from StreamRegistry ==========
async fn load_from_stream_registry() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let etcd_endpoint = env::var("ETCD_ENDPOINT").unwrap_or("http://localhost:2379");
    let registry = StreamRegistry::new(&[&etcd_endpoint]).await?;
    let stream_config = registry.load_stream("air-quality").await?;
    stream_config_to_app_config(stream_config)
}

// ========== NEW FUNCTION 2: Config Mapping ==========
fn stream_config_to_app_config(stream: StreamConfig) -> Result<AppConfig, Box<dyn std::error::Error>> {
    // 1. Find MQTT source
    let mqtt_source = stream.sources.iter()
        .find(|s| s.source_type == SourceType::Mqtt)
        .ok_or("No MQTT source")?;

    // 2. Extract MQTT params
    let broker_url = mqtt_source.params.get("broker_url")
        .and_then(|v| v.as_str())
        .ok_or("Missing broker_url")?;

    let port = mqtt_source.params.get("port")
        .and_then(|v| v.as_u64())
        .unwrap_or(1883) as u16;

    // ... (extract other MQTT fields)

    // 3. Extract storage config
    let batch_size = stream.storage.as_ref()
        .map(|s| s.batch_size)
        .unwrap_or(100);

    // 4. Build AppConfig
    Ok(AppConfig {
        server: ServerConfig {
            host: env::var("AIR_QUALITY_SERVER_HOST").unwrap_or("0.0.0.0"),
            port: env::var("AIR_QUALITY_SERVER_PORT").unwrap_or(8080),
        },
        mqtt: MqttConfig {
            broker_url,
            port,
            // ... (all MQTT fields)
        },
        storage: StorageConfig {
            base_path: env::var("DATA_DIR").unwrap_or("./data/parquet"),
            batch_size,
            // ... (all storage fields)
        },
    })
}

// ========== MODIFIED main() FUNCTION ==========
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (unchanged)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()...)
        .init();

    // ========== MODIFIED: Configuration Loading ==========
    let config = match load_from_stream_registry().await {
        Ok(config) => {
            tracing::info!("✓ Loaded config from StreamRegistry (/streams/air-quality/config)");
            config
        }
        Err(e) => {
            tracing::warn!("StreamRegistry failed: {}. Trying legacy etcd...", e);

            // FALLBACK 1: Legacy etcd (EXISTING CODE - unchanged)
            match air_quality_app::load_from_etcd().await {
                Ok(etcd_config) => {
                    tracing::info!("✓ Loaded config from legacy etcd (/air-quality/*)");
                    AppConfig { /* conversion code */ }
                }
                Err(e) => {
                    tracing::warn!("Legacy etcd failed: {}. Trying config.yaml...", e);

                    // FALLBACK 2: config.yaml (EXISTING CODE - unchanged)
                    match AppConfig::from_yaml("config.yaml") {
                        Ok(cfg) => {
                            tracing::info!("✓ Loaded config from config.yaml");
                            cfg
                        }
                        Err(e) => {
                            tracing::warn!("config.yaml failed: {}. Using defaults.", e);

                            // FALLBACK 3: Defaults (EXISTING CODE - unchanged)
                            AppConfig::default_config()
                        }
                    }
                }
            }
        }
    };

    // ========== REST OF main() UNCHANGED ==========
    // Lines 74-192: ParquetStore, MQTT handler, storage writer, API server
    // (No changes to existing logic)
}
```

### 6.2 Test Migration Path

```bash
# Step 1: Deploy code with StreamRegistry support (backward compatible)
cargo build --release
docker-compose up -d

# Step 2: Test with legacy config (should work unchanged)
curl http://localhost:8080/api/locations
# Logs: "✓ Loaded config from legacy etcd (/air-quality/*)"

# Step 3: Add StreamRegistry config to etcd
etcdctl put /streams/air-quality/config "$(cat stream-config.json)"

# Step 4: Restart app (should auto-detect StreamRegistry)
docker-compose restart air-quality-server
# Logs: "✓ Loaded config from StreamRegistry (/streams/air-quality/config)"

# Step 5: Remove StreamRegistry key to test fallback
etcdctl del /streams/air-quality/config
docker-compose restart air-quality-server
# Logs: "StreamRegistry failed: key not found. Trying legacy etcd..."
# Logs: "✓ Loaded config from legacy etcd (/air-quality/*)"
```

## 7. Implementation Checklist

### 7.1 Code Changes

- [ ] Add imports to `main.rs`: `StreamRegistry`, `StreamConfig`
- [ ] Implement `load_from_stream_registry()` function
- [ ] Implement `stream_config_to_app_config()` function
- [ ] Update `main()` configuration loading section (lines 24-65)
- [ ] Add error handling for missing `broker_url` in MQTT source
- [ ] Preserve all existing fallback logic (legacy etcd, config.yaml, defaults)

### 7.2 Testing

- [ ] Unit test: `stream_config_to_app_config()` with valid StreamConfig
- [ ] Unit test: `stream_config_to_app_config()` with missing broker_url (should error)
- [ ] Unit test: `stream_config_to_app_config()` with missing optional fields (should use defaults)
- [ ] Integration test: App starts with StreamRegistry config
- [ ] Integration test: App falls back to legacy etcd when StreamRegistry missing
- [ ] Integration test: App falls back to config.yaml when both unavailable

### 7.3 Documentation

- [ ] Update `README.md` with StreamRegistry setup instructions
- [ ] Add example StreamConfig JSON for air-quality
- [ ] Document migration path from legacy etcd to StreamRegistry
- [ ] Update deployment docs with new etcd key paths

### 7.4 Deployment

- [ ] Ensure `config-client` crate includes `StreamRegistry` (already exists)
- [ ] Verify `neural-core` exports `StreamConfig` and `SourceType` (already exists)
- [ ] Test with both etcd scenarios (StreamRegistry present/absent)
- [ ] Monitor logs during rollout for fallback behavior

## 8. Dependencies

### 8.1 Existing Crates (No Changes Needed)

- `config-client` v0.1.0 - Already has `StreamRegistry` implementation
- `neural-core` - Already has `StreamConfig`, `SourceType`, `SourceConfig` types
- `air-quality-app` - Needs modification (this feature)

### 8.2 New Dependencies

**None**. All required types already exist:

```rust
// config-client/src/stream/registry.rs (EXISTING)
pub struct StreamRegistry { ... }
impl StreamRegistry {
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> { ... }
    pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError> { ... }
}

// core/src/types/stream_config.rs (EXISTING)
pub struct StreamConfig { ... }
pub enum SourceType { Mqtt, HttpPoll, Webhook, FileWatch }
pub struct SourceConfig { ... }
```

## 9. Risk Analysis

### 9.1 Low-Risk Changes

✅ **Adding new fallback layer**: StreamRegistry is tried first but doesn't break existing fallback chain
✅ **No breaking changes**: Existing deployments continue using legacy etcd
✅ **Gradual migration**: Can add StreamRegistry config without code changes
✅ **Type safety**: `StreamConfig` has validation (`validate()` method)

### 9.2 Medium-Risk Areas

⚠️ **Config mapping errors**: If `broker_url` missing, app won't start
   - **Mitigation**: Comprehensive error messages, unit tests for mapping logic

⚠️ **etcd key collision**: Both `/streams/air-quality/*` and `/air-quality/*` coexist
   - **Mitigation**: Different prefixes, StreamRegistry uses `/streams` exclusively

⚠️ **Performance**: Extra etcd roundtrip for StreamRegistry before fallback
   - **Mitigation**: Fast-fail on connection error, cached by StreamRegistry

### 9.3 Monitoring Points

- Log level for config source (StreamRegistry vs legacy vs file)
- Measure config load time per source
- Alert on repeated fallbacks (indicates StreamRegistry issues)
- Track which deployments use StreamRegistry vs legacy

## 10. Future Enhancements (Out of Scope)

The following are **NOT** part of AIR-004 but enabled by this architecture:

1. **Multi-stream support**: Load multiple streams (`weather`, `power`, etc.)
2. **Hot reload**: Watch etcd for config changes, reload without restart
3. **Schema validation**: Use `StreamConfig.fields` to validate incoming MQTT data
4. **Dynamic routing**: Create MQTT subscriptions from `topic_pattern` in config
5. **Source plugins**: Abstract `DataSource` trait for HTTP/Webhook/FileWatch
6. **Config API**: REST endpoints to manage StreamRegistry via API

---

## Appendix A: Example etcd Commands

### A.1 Create StreamRegistry Config

```bash
# Save StreamConfig JSON
cat > /tmp/air-quality-stream.json <<EOF
{
  "stream_id": "air-quality",
  "description": "AirGradient sensor readings",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 7,
  "partitioning_strategy": "daily",
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "nullable": false,
      "range": [0.0, 500.0],
      "display_precision": 1
    },
    {
      "name": "temperature",
      "type": "float",
      "unit": "celsius",
      "nullable": true,
      "range": [-40.0, 60.0],
      "display_precision": 1
    }
  ],
  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "broker_url": "localhost",
      "port": 1883,
      "client_id": "air-quality-app",
      "topic_pattern": "airgradient/readings/+",
      "qos": 1,
      "buffer_capacity": 1000
    }
  ],
  "storage": {
    "batch_size": 100,
    "batch_timeout_secs": 5,
    "buffer_capacity": 1000
  }
}
EOF

# Put into etcd
etcdctl put /streams/air-quality/config "$(cat /tmp/air-quality-stream.json)"
```

### A.2 Verify Configuration

```bash
# Get StreamRegistry config
etcdctl get /streams/air-quality/config

# List all streams
etcdctl get --prefix /streams/

# Get legacy config (for comparison)
etcdctl get --prefix /air-quality/
```

### A.3 Rollback to Legacy

```bash
# Delete StreamRegistry config
etcdctl del /streams/air-quality/config

# App automatically falls back to /air-quality/* keys
```

## Appendix B: Error Messages

### B.1 Expected Log Output

**Success (StreamRegistry)**:
```
INFO  Connecting to etcd at http://localhost:2379
INFO  ✓ Loaded config from StreamRegistry (/streams/air-quality/config)
INFO  Starting air quality server on 0.0.0.0:8080
INFO  MQTT handler initialized successfully
```

**Fallback to Legacy**:
```
INFO  Connecting to etcd at http://localhost:2379
WARN  StreamRegistry failed: Key not found: /streams/air-quality/config. Trying legacy etcd...
INFO  ✓ Loaded config from legacy etcd (/air-quality/*)
INFO  Starting air quality server on 0.0.0.0:8080
```

**Fallback to File**:
```
WARN  StreamRegistry failed: connection refused
WARN  Legacy etcd failed: connection refused. Trying config.yaml...
INFO  ✓ Loaded config from config.yaml
INFO  Starting air quality server on 0.0.0.0:8080
```

**Invalid Config**:
```
ERROR Failed to start: Invalid stream config: No MQTT source found in stream config
```

---

**End of Architecture Document**

**Key Principle**: This integration adds StreamRegistry as an **optional enhancement** without breaking existing deployments. The fallback chain ensures the app always starts, even if StreamRegistry is unavailable.
