# AIR-002: Comprehensive Architecture Summary

**Version:** 1.0.0
**Date:** 2025-12-14
**Feature:** MQTT Ingestion Pipeline - Complete Architecture Analysis
**Status:** Production Architecture Documentation

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Configuration Management Architecture](#configuration-management-architecture)
3. [Runtime Configuration Hot-Reload Patterns](#runtime-configuration-hot-reload-patterns)
4. [etcd Integration Patterns](#etcd-integration-patterns)
5. [Environment Variable Hierarchy](#environment-variable-hierarchy)
6. [Service Configuration Patterns](#service-configuration-patterns)
7. [Key Design Rationale and Trade-offs](#key-design-rationale-and-trade-offs)
8. [Architecture Decision Records (ADRs)](#architecture-decision-records-adrs)
9. [Implementation Patterns](#implementation-patterns)
10. [Production Validation Findings](#production-validation-findings)

---

## Executive Summary

### Core Architecture Principle

**REUSE OVER REWRITE**: AIR-002 leverages existing platform components (MQTT source, parser, validator, adapter, Parquet storage) and adds only minimal orchestration needed for the ingestion pipeline.

### System Flow
```
AirGradient Sensor → MQTT Broker → MqttSource → Parser → Validator → Adapter → ParquetStore (WAL) → REST API
```

### Configuration Strategy Decision

**DECISION**: Minimal YAML configuration for AIR-002, defer standardization to AIR-003

**Timeline Impact**:
- AIR-002 with minimal config: 22-30 hours (2.75 days)
- Alternative with full config-store: 33-44 hours (4.5 days)
- **Time saved**: 37% faster to E2E testing

**Rationale**:
1. Unblock E2E testing immediately
2. Low technical debt (isolated to single app)
3. Clear migration path to config-store in AIR-003
4. Zero impact on platform-core components

---

## Configuration Management Architecture

### 1. Configuration Hierarchy

```
┌────────────────────────────────────────┐
│         Configuration Sources          │
├────────────────────────────────────────┤
│  Priority 1: Environment Variables     │  ← Highest priority
│  Priority 2: config.yaml (file)        │  ← Development defaults
│  Priority 3: Hardcoded Defaults        │  ← Fallback
└────────────────────────────────────────┘
```

### 2. File Structure

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/config.yaml`

```yaml
server:
  host: "0.0.0.0"
  port: 8080

mqtt:
  broker_url: "localhost"
  port: 1883
  client_id: "air-quality-app"
  topic_pattern: "airgradient/readings/+"
  qos: 1
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30
  buffer_capacity: 1000

storage:
  base_path: "/data/parquet"
  wal_enabled: true
```

### 3. Type-Safe Configuration Structs

**AIR-002 Implementation** (Minimal, YAML-friendly):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub mqtt: MqttConfigYaml,
    pub storage: StorageConfigYaml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfigYaml {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,
    pub qos: u8,  // 0, 1, or 2 (YAML-friendly)
    pub reconnect_delay_secs: u64,  // Duration as u64
    pub max_reconnect_delay_secs: u64,
    pub buffer_capacity: usize,
}

impl MqttConfigYaml {
    /// Convert to platform-core MqttConfig
    pub fn to_mqtt_config(&self) -> platform_core::sources::mqtt::MqttConfig {
        platform_core::sources::mqtt::MqttConfig {
            broker_url: self.broker_url.clone(),
            port: self.port,
            client_id: self.client_id.clone(),
            topic_pattern: self.topic_pattern.clone(),
            qos: match self.qos {
                0 => QoS::AtMostOnce,
                1 => QoS::AtLeastOnce,
                2 => QoS::ExactlyOnce,
                _ => QoS::AtLeastOnce,  // Safe default
            },
            reconnect_delay: Duration::from_secs(self.reconnect_delay_secs),
            max_reconnect_delay: Duration::from_secs(self.max_reconnect_delay_secs),
            buffer_capacity: self.buffer_capacity,
        }
    }
}
```

### 4. Environment Variable Override Pattern

**Implementation**:

```rust
impl AppConfig {
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AppConfig = serde_yaml::from_str(&content)?;

        // Apply environment variable overrides
        config.apply_env_overrides();
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        // MQTT overrides
        if let Ok(url) = std::env::var("MQTT_BROKER_URL") {
            self.mqtt.broker_url = url;
        }
        if let Ok(port) = std::env::var("MQTT_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                self.mqtt.port = port_num;
            }
        }

        // Storage overrides
        if let Ok(path) = std::env::var("STORAGE_PATH") {
            self.storage.base_path = path;
        }

        // Server overrides
        if let Ok(host) = std::env::var("SERVER_HOST") {
            self.server.host = host;
        }
        if let Ok(port) = std::env::var("SERVER_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                self.server.port = port_num;
            }
        }
    }
}
```

**Supported Environment Variables**:
- `MQTT_BROKER_URL` - Override broker hostname
- `MQTT_PORT` - Override broker port
- `MQTT_CLIENT_ID` - Override client identifier
- `STORAGE_PATH` - Override Parquet storage path
- `SERVER_HOST` - Override HTTP bind address
- `SERVER_PORT` - Override HTTP port

---

## Runtime Configuration Hot-Reload Patterns

### AIR-003 Future Pattern (Deferred)

**Smart Client with Hot-Reload**:

```rust
pub struct ConfigClient {
    cache: Arc<RwLock<LruCache<String, CachedValue>>>,
    watcher: Arc<Watcher>,  // Background task for updates
    providers: Vec<Box<dyn Provider>>,
}

impl ConfigClient {
    pub async fn watch(&self, path: &str) -> impl Stream<Item = ConfigValue> {
        // Stream updates from gRPC provider
        self.watcher.subscribe(path)
    }

    pub async fn reload(&self) -> Result<()> {
        // Invalidate cache and fetch fresh values
        self.cache.write().await.clear();
        // Notify subscribers
        self.watcher.notify_reload().await
    }
}
```

**Timeline**: Not in AIR-002, planned for AIR-003 (3-4 weeks)

---

## etcd Integration Patterns

### Current State (AIR-001 Implementation)

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/config/manager.rs`

**Pattern**: Direct etcd client with config hierarchy

```rust
pub struct ConfigManager {
    etcd_client: Option<Client>,
    cache: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl ConfigManager {
    pub async fn get_config(&self, key: &str) -> Result<serde_json::Value> {
        // 1. Check cache first
        if let Some(value) = self.cache.read().await.get(key) {
            return Ok(value.clone());
        }

        // 2. Query etcd
        if let Some(ref client) = self.etcd_client {
            let response = client.get(key, None).await?;
            if let Some(kv) = response.kvs().first() {
                let value: serde_json::Value = serde_json::from_slice(kv.value())?;
                // Update cache
                self.cache.write().await.insert(key.to_string(), value.clone());
                return Ok(value);
            }
        }

        // 3. Fallback to defaults
        Err(ConfigError::NotFound(key.to_string()))
    }

    pub async fn watch_config(&self, key: &str) -> Result<impl Stream<Item = Event>> {
        // Watch for config changes
        if let Some(ref client) = self.etcd_client {
            let (watcher, stream) = client.watch(key, None).await?;
            Ok(stream)
        } else {
            Err(ConfigError::NoEtcdClient)
        }
    }
}
```

**Configuration Hierarchy in etcd**:
```
/neural-data-platform/
├── apps/
│   ├── air-quality/
│   │   ├── mqtt/broker_url
│   │   ├── mqtt/port
│   │   ├── mqtt/topic_pattern
│   │   ├── storage/base_path
│   │   └── storage/wal_enabled
│   └── config-store/
│       └── ...
├── environments/
│   ├── production/
│   ├── staging/
│   └── development/
└── global/
    ├── logging/level
    └── monitoring/enabled
```

**Key Pattern**: Hierarchical path-based configuration with environment-specific overrides

---

## Environment Variable Hierarchy

### Priority Order (Highest to Lowest)

1. **Runtime Environment Variables** (`MQTT_BROKER_URL=...`)
2. **etcd Configuration** (future, AIR-003+)
3. **config.yaml File** (current AIR-002)
4. **Hardcoded Defaults** (fallback)

### Variable Naming Convention

**Pattern**: `{COMPONENT}_{SUBSYSTEM}_{PARAMETER}`

**Examples**:
- `MQTT_BROKER_URL` (not `BROKER_URL`)
- `MQTT_PORT` (not `PORT`)
- `STORAGE_PATH` (not `DATA_PATH`)

**Rationale**: Prevents collisions, clear component ownership

### Docker Deployment Example

```yaml
# docker-compose.yml
services:
  air-quality-app:
    image: air-quality-server:latest
    environment:
      - MQTT_BROKER_URL=mosquitto
      - MQTT_PORT=1883
      - STORAGE_PATH=/data/parquet
      - SERVER_HOST=0.0.0.0
      - SERVER_PORT=8080
    volumes:
      - ./data:/data
```

### Kubernetes ConfigMap Pattern

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: air-quality-config
data:
  MQTT_BROKER_URL: "mqtt.production.svc.cluster.local"
  MQTT_PORT: "1883"
  STORAGE_PATH: "/mnt/parquet"
---
apiVersion: apps/v1
kind: Deployment
spec:
  template:
    spec:
      containers:
      - name: air-quality-server
        envFrom:
        - configMapRef:
            name: air-quality-config
```

---

## Service Configuration Patterns

### 1. MQTT Source Configuration

**Platform-Core Type** (runtime):

```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,
    pub qos: QoS,  // Enum: AtMostOnce, AtLeastOnce, ExactlyOnce
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
}
```

**Design Rationale**:
- `QoS` enum provides type safety vs raw integers
- `Duration` types prevent unit confusion (seconds vs milliseconds)
- `buffer_capacity` for backpressure management

### 2. Storage Configuration

**Pattern**: Base path + automatic partitioning

```rust
pub struct StorageConfigYaml {
    pub base_path: String,  // e.g., "/data/parquet"
    pub wal_enabled: bool,
}

// Automatic partition structure:
// {base_path}/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet
```

**Rationale**:
- Simple base path configuration
- Partitioning logic encapsulated in ParquetStore
- WAL enabled by default for durability

### 3. Server Configuration

**Pattern**: Bind address + port

```rust
pub struct ServerConfig {
    pub host: String,  // "0.0.0.0" for Docker, "127.0.0.1" for local
    pub port: u16,     // Default: 8080
}

impl ServerConfig {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
```

### 4. Pipeline Configuration (Future)

**Deferred to AIR-002 implementation**:

```rust
pub struct PipelineConfig {
    pub batch_size: usize,           // Default: 100 points
    pub batch_timeout: Duration,     // Default: 1 second
    pub max_retries: u32,            // Default: 3
    pub dlq_path: PathBuf,           // Dead Letter Queue path
}
```

---

## Key Design Rationale and Trade-offs

### 1. YAML vs TOML vs config-store

**Decision**: YAML for AIR-002, migrate to config-store in AIR-003

| Format | Pros | Cons | Decision |
|--------|------|------|----------|
| **YAML** | Human-readable, nested structures, wide support | No type safety, whitespace-sensitive | **Use for AIR-002** |
| **TOML** | Type hints, simpler syntax | Less flexible nesting | Consider for AIR-003 |
| **config-store** | Versioning, hot-reload, gRPC API | Requires infrastructure setup | **AIR-003+** |

**Trade-off Analysis**:

**Why YAML Now?**
- ✅ Fastest to implement (1-2 hours vs 6-8 hours)
- ✅ Zero infrastructure dependencies
- ✅ Easy to debug and test
- ✅ Standard for Rust services (serde_yaml)

**Why Defer config-store?**
- ⚠️ Requires gRPC server setup
- ⚠️ Needs client crate (3-4 weeks effort)
- ⚠️ Overkill for single-app MVP
- ⚠️ Not on critical path to E2E testing

**Migration Path**:
1. **AIR-002**: Simple YAML (1-2 hours)
2. **AIR-003**: Build config-store-client crate (3-4 weeks)
3. **AIR-004**: Migrate all apps to unified config

### 2. Environment Variable Overrides

**Decision**: Support env vars for deployment flexibility

**Rationale**:
- Docker/Kubernetes require runtime config changes
- Secrets (MQTT credentials) shouldn't be in config files
- CI/CD pipelines need per-environment customization

**Trade-off**:
- ✅ Deployment flexibility
- ⚠️ Config can be hard to debug (multiple sources)
- **Mitigation**: Log loaded config on startup (mask secrets)

### 3. Type Conversion Layer

**Decision**: Separate YAML-friendly types from runtime types

**Pattern**:
```rust
// YAML-friendly (serde)
pub struct MqttConfigYaml {
    pub qos: u8,  // 0, 1, 2
    pub reconnect_delay_secs: u64,
}

// Runtime (platform-core)
pub struct MqttConfig {
    pub qos: QoS,  // Enum
    pub reconnect_delay: Duration,
}

// Conversion
impl MqttConfigYaml {
    pub fn to_mqtt_config(&self) -> MqttConfig { ... }
}
```

**Rationale**:
- YAML can't serialize complex types (enums, Duration)
- Platform-core types provide type safety
- Clear conversion boundary

**Trade-off**:
- ⚠️ Duplication of struct definitions
- ✅ Type safety at runtime
- ✅ YAML simplicity

### 4. Configuration Validation

**AIR-002 Approach**: Minimal validation

```rust
impl AppConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Basic checks
        if self.mqtt.broker_url.is_empty() {
            return Err(ConfigError::InvalidBrokerUrl);
        }
        if self.mqtt.port == 0 {
            return Err(ConfigError::InvalidPort);
        }
        Ok(())
    }
}
```

**AIR-003+ Approach**: Schema validation

```rust
// Future: Use JSON Schema or custom validation
pub trait ConfigSchema {
    fn schema() -> Schema;
    fn validate(&self) -> Vec<ValidationError>;
}
```

**Trade-off**:
- **Now**: Fail-fast on invalid config (panic in main)
- **Future**: Detailed validation errors with suggestions

---

## Architecture Decision Records (ADRs)

### ADR-001: Reuse Core MqttSource vs Domain-Specific Wrapper

**Status**: ACCEPTED

**Context**:
- `core/src/sources/mqtt.rs` has built-in parser for AirGradient format
- `domains/air-quality/src/parser.rs` is more comprehensive
- Need decision: modify core or wrap it?

**Decision**: Use core MqttSource as-is

**Rationale**:
- Core MqttSource.parse_payload() handles same JSON format
- Validation can happen after fetching from Source trait
- Reduces code duplication
- Clear separation: Source→Traits, Domain→Validation

**Consequences**:
- ✅ No changes to core components
- ✅ Clear separation of concerns
- ⚠️ Parser logic duplicated (acceptable for independence)

---

### ADR-002: Batching Strategy

**Status**: ACCEPTED

**Context**:
- Storage writes are expensive (Parquet file I/O)
- MQTT messages arrive individually
- Need balance between latency and throughput

**Decision**: Batch writes using dual criteria
- **Size**: Flush at 100 points
- **Time**: Flush after 1 second

**Rationale**:
- Prevents small file proliferation
- Ensures max 1s latency for queries
- WAL provides durability before batch flush
- 100 points ≈ 1 reading from 100 metrics OR 100 readings from 1 sensor

**Consequences**:
- ✅ Good write performance
- ✅ Acceptable query latency
- ⚠️ Slightly more complex than immediate writes

**Performance Projections**:
- Single sensor: 1,440 readings/day → negligible load
- 100 sensors: 144,000 points/day → ~2 points/sec average
- Batch writes easily handle this throughput

---

### ADR-003: Error Handling via Dead Letter Queue

**Status**: ACCEPTED

**Context**:
- Invalid readings should not crash pipeline
- Need visibility into failed messages
- Need ability to retry after fixes

**Decision**: Write invalid messages to DLQ as JSONL files

**DLQ Structure**:
```
data/air_quality/dlq/
├── year=2025/
│   ├── month=12/
│   │   ├── day=14/
│   │   │   └── errors.ndjson
```

**Entry Format**:
```json
{
  "timestamp": "2025-12-14T10:30:00Z",
  "error": "CO2 out of range: 15000",
  "payload": "{\"serialno\":\"abc\",\"rco2\":15000}",
  "attempts": 1
}
```

**Rationale**:
- Simple implementation (file append)
- Easy to inspect and retry
- Standard pattern in message processing
- Prevents data loss

**Consequences**:
- ✅ Resilient pipeline
- ✅ Debugging capability
- ⚠️ Requires DLQ management (rotation, cleanup)

**Management**:
- Rotate daily
- Compress after 7 days
- Delete after 30 days
- Expose via REST API for retry

---

### ADR-004: Concurrency Model

**Status**: ACCEPTED

**Context**:
- Need concurrent MQTT processing and HTTP serving
- Rust async/await with Tokio runtime
- Need to share state safely

**Decision**: Use Tokio with Arc + Mutex for shared state, mpsc for message passing

**Architecture**:
```rust
pub struct AppState {
    pub mqtt_source: Arc<MqttSource>,
    pub store: Arc<ParquetStore>,
    pub pipeline: Arc<Mutex<IngestionPipeline>>,
}
```

**Channel Strategy**:
```rust
// MQTT Message Channel
let (tx, rx) = mpsc::channel::<TimeSeriesPoint>(1000);
// Capacity: 1000 messages (backpressure)
// Sender: MqttSource (in event loop task)
// Receiver: IngestionPipeline

// Shutdown Channel
let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
// Type: broadcast (multiple receivers)
// Senders: main() on SIGTERM/SIGINT
// Receivers: All spawned tasks
```

**Task Architecture**:
```
┌─────────────────────────────────────────────────┐
│             Main Tokio Runtime                  │
├─────────────────────────────────────────────────┤
│  Task 1: MQTT Event Loop                       │
│  Task 2: Ingestion Loop                        │
│  Task 3: HTTP Server (Axum)                    │
│  Task 4: Health Monitor (optional)             │
└─────────────────────────────────────────────────┘
```

**Rationale**:
- Standard Rust async pattern
- Arc provides cheap cloning
- Mutex for interior mutability where needed
- mpsc provides backpressure

**Consequences**:
- ✅ Safe concurrency
- ✅ Bounded memory usage
- ⚠️ Mutex contention possible (mitigate: minimize critical sections)

---

### ADR-005: Configuration via YAML + Environment Overrides

**Status**: ACCEPTED (for AIR-002)

**Context**:
- Need runtime configuration (broker URL, ports, etc.)
- Deployment in Docker and bare metal
- Need environment overrides

**Decision**: Use YAML files with environment variable overrides

**Alternatives Considered**:

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| Environment only | Docker-friendly | Hard to manage 20+ vars | ❌ |
| YAML only | Human-readable | Can't override in deployment | ❌ |
| **YAML + Env overrides** | Best of both | Need conversion layer | ✅ **CHOSEN** |
| config-store | Production-grade | Too complex for MVP | ⏸️ Deferred |

**Rationale**:
- Human-readable defaults in YAML
- Environment variables for secrets/deployment
- serde_yaml mature and stable
- Standard pattern in Rust ecosystem

**Consequences**:
- ✅ Easy to configure
- ✅ Docker-friendly
- ⚠️ Another dependency (acceptable: tiny and stable)

---

### ADR-006: Minimal Config for AIR-002, Standardization in AIR-003

**Status**: ACCEPTED

**Context**:
- Platform has existing config-store with gRPC, versioning, hot-reload
- AIR-002 is blocking E2E testing
- Building config-store client takes 3-4 weeks

**Decision**: Use simple YAML config for AIR-002, defer standardization to AIR-003

**Timeline Impact**:
- **Minimal YAML**: 1-2 hours → 22-30 total hours (2.75 days)
- **Full config-store**: 6-8 hours + 9-12 prereqs → 33-44 hours (4.5 days)
- **Savings**: 37% faster to E2E testing

**Technical Debt**:
- ⚠️ Duplication of config types (AppConfig vs PlatformConfig)
- ⚠️ Two config systems temporarily

**Mitigation**:
- ✅ Clear conversion methods (`to_mqtt_config()`)
- ✅ Documented migration path in AIR-003
- ✅ Isolated to single app
- ✅ No impact on platform-core

**Migration Plan** (AIR-003):
1. Build config-store-client crate (3-4 weeks)
2. Add MqttConfig, StorageConfig to config-store
3. Migrate air-quality-app
4. Remove duplicate AppConfig

**Rationale**:
- Primary goal: Unblock E2E testing ASAP
- Low-risk approach (proven YAML pattern)
- Manageable technical debt
- Clean refactoring path

---

## Implementation Patterns

### 1. Configuration Loading Pattern

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load configuration
    let config = match AppConfig::from_yaml("config.yaml") {
        Ok(cfg) => {
            tracing::info!("Loaded configuration from config.yaml");
            cfg
        }
        Err(e) => {
            tracing::warn!("Config file not found ({}), using defaults", e);
            AppConfig::default_config()
        }
    };

    // 2. Log loaded config (mask secrets)
    tracing::info!("MQTT broker: {}:{}", config.mqtt.broker_url, config.mqtt.port);
    tracing::info!("Storage path: {}", config.storage.base_path);

    // 3. Convert to platform-core types
    let mqtt_config = config.mqtt.to_mqtt_config();
    let storage_path = &config.storage.base_path;

    // 4. Initialize components
    let mut mqtt_source = MqttSource::new(mqtt_config);
    mqtt_source.start().await?;

    let store = ParquetStore::new(storage_path)?;
    store.replay_wal().await?;

    // 5. Start application
    // ...
}
```

### 2. Component Wiring Pattern

**Current State (AIR-001)**: Mock implementations

**File**: `apps/air-quality-app/src/main.rs` lines 34-162

**Problem**: Production code uses mock services
```rust
// ❌ CURRENT - MUST BE REMOVED
let services = create_mock_services();

fn create_mock_services() -> AppServices {
    struct MockStore;  // Does nothing!
    struct MockSource; // Does nothing!
    // ...
}
```

**Solution**: Wire real implementations

```rust
// ✅ CORRECT - Real Services
async fn create_real_services(config: &AppConfig) -> AppServices {
    // Real MQTT source
    let mqtt_config = config.mqtt.to_mqtt_config();
    let mut mqtt_source = Arc::new(MqttSource::new(mqtt_config));
    mqtt_source.start().await.expect("MQTT source failed");

    // Real Parquet storage
    let store = Arc::new(ParquetStore::new(&config.storage.base_path)
        .expect("Storage initialization failed"));

    AppServices {
        source: mqtt_source,
        store,
        // ... other real services
    }
}
```

### 3. Error Flow Pattern

```
Message Received
    │
    ├─▶ Parse Error ──────▶ DLQ (JSON invalid)
    │
    ├─▶ Validation Error ──▶ DLQ (out of range)
    │
    ├─▶ Storage Error
    │       │
    │       ├─▶ Retry (3x with backoff)
    │       │
    │       └─▶ DLQ (after retries exhausted)
    │
    └─▶ Success ──▶ ACK
```

**Implementation**:
```rust
async fn process_message(payload: Vec<u8>) -> Result<()> {
    // 1. Parse
    let reading = match parse_mqtt_payload(&payload) {
        Ok(r) => r,
        Err(e) => {
            dlq.write(payload, format!("Parse error: {}", e)).await?;
            return Ok(()); // Don't crash pipeline
        }
    };

    // 2. Validate
    if let Err(e) = validate_reading(&reading) {
        dlq.write(payload, format!("Validation error: {}", e)).await?;
        return Ok(());
    }

    // 3. Adapt
    let points = AirQualityAdapter::to_time_series_points(&reading);

    // 4. Store with retry
    let mut retries = 0;
    loop {
        match store.write_batch(points.clone()).await {
            Ok(()) => break,
            Err(e) if retries < 3 => {
                retries += 1;
                tokio::time::sleep(Duration::from_secs(2_u64.pow(retries))).await;
            }
            Err(e) => {
                dlq.write(payload, format!("Storage error: {}", e)).await?;
                return Ok(());
            }
        }
    }

    Ok(())
}
```

### 4. Metrics Instrumentation Pattern

```rust
use metrics::{counter, histogram, gauge};

// Counter metrics
counter!("ingestion_messages_received_total", "topic" => topic_name);
counter!("ingestion_parse_errors_total");
counter!("ingestion_validation_errors_total", "reason" => error_type);
counter!("ingestion_points_written_total");

// Histogram metrics
let start = Instant::now();
// ... process message ...
histogram!("ingestion_latency_seconds", start.elapsed());

// Gauge metrics
gauge!("ingestion_mqtt_connected", if connected { 1.0 } else { 0.0 });
gauge!("ingestion_buffer_size", batch.len() as f64);
```

---

## Production Validation Findings

### Critical Blockers (MUST FIX)

**Source**: `/product/features/air-002/validation-report.md`

#### 🚨 BLOCKER 1: Main Application Uses Mock Services

**File**: `apps/air-quality-app/src/main.rs` lines 34-162

**Issue**: Production code path uses mock implementations

**Impact**:
- ❌ Application will NOT store any data
- ❌ Application will NOT receive any MQTT messages
- ❌ All API endpoints return empty data

**Required Action**: Replace `create_mock_services()` with real implementations

---

#### 🚨 BLOCKER 2: MCP Server Uses Placeholders

**File**: `apps/air-quality-app/src/mcp/server.rs` lines 17-87

**Issue**: Placeholder implementations return fake/empty data

**Impact**:
- ❌ MCP tools return fake data (hardcoded CO2=850, PM2.5=12.5)
- ❌ No actual forecasting capability
- ❌ Alert system non-functional

**Required Action**: Wire real storage adapters to MCP server

---

### Production-Ready Components ✅

| Component | File | Status | Tests |
|-----------|------|--------|-------|
| MQTT Source | `core/src/sources/mqtt.rs` | ✅ READY | 478 lines |
| AirGradient Parser | `domains/air-quality/src/parser.rs` | ✅ READY | 329 lines |
| Parquet Storage | `core/src/storage/parquet.rs` | ✅ READY | 286 lines |
| Write-Ahead Log | `core/src/storage/wal.rs` | ✅ READY | 172 lines |
| Health Check Handler | `apps/air-quality-app/src/api/handlers/health.rs` | ✅ READY | (pending wiring) |

**Verdict**: Core infrastructure is production-ready, needs integration wiring

---

### Data Flow Verification

**Current State (BROKEN)**:
```
[MQTT Broker]
    ↓
[MockSource] ← DOES NOTHING
    ↓
[MockStore] ← DOES NOTHING
    ↓
[API] ← Returns empty data
```

**Required State (PRODUCTION)**:
```
[MQTT Broker]
    ↓
[MqttSource] ← Real implementation ✅
    ↓ (parse_mqtt_payload)
[Parser] ← Real implementation ✅
    ↓ (write_batch)
[ParquetStore + WAL] ← Real implementation ✅
    ↓ (query)
[API Handlers] ← Uses real traits ✅
```

**Gap**: Main.rs doesn't wire the real implementations together

**Estimated Fix Time**: 4-8 hours

---

## Configuration Migration Roadmap

### Phase 1: AIR-002 (Current - Minimal Config)

**Timeline**: 1-2 hours
**Status**: In Progress

**Deliverables**:
- ✅ `config.yaml` with MQTT, storage, server settings
- ✅ `AppConfig::from_yaml()` with env overrides
- ✅ Type conversion to platform-core structs
- ✅ Default config fallback

**Files**:
- `/apps/air-quality-app/config.yaml` (new)
- `/apps/air-quality-app/src/config.rs` (modify)

---

### Phase 2: AIR-003 (Config Standardization)

**Timeline**: 3-4 weeks
**Status**: Planned

**Deliverables**:
- Build config-store-client crate with:
  - Provider system (Env → gRPC → File)
  - LRU cache with TTL
  - Type-safe deserialization
  - Hot-reload capability
- Add MqttConfig to config-store
- Add StorageConfig to config-store
- Migrate air-quality-app to use client
- Remove duplicate AppConfig structs

**Architecture**:
```
┌─────────────────────────────────────────────────┐
│                ConfigClient                     │
├─────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐        │
│  │  Cache  │  │ Watcher │  │ Metrics │        │
│  └────┬────┘  └────┬────┘  └────┬────┘        │
│       │            │            │              │
│  ┌────▼────────────▼────────────▼────┐        │
│  │         Provider System            │        │
│  ├──────────┬──────────┬─────────────┤        │
│  │   Env    │   File   │    gRPC     │        │
│  │ Provider │ Provider │  Provider   │        │
│  └──────────┴──────────┴─────────────┘        │
└─────────────────────────────────────────────────┘
```

---

### Phase 3: AIR-004+ (Platform-Wide Migration)

**Timeline**: TBD
**Status**: Future

**Scope**:
- Migrate all services to config-store
- Implement config-as-code workflows
- Add advanced features:
  - Config versioning
  - A/B testing flags
  - Secrets management (Vault integration)
  - Config audit logging

---

## Summary of Key Patterns

### 1. Configuration Patterns
- ✅ YAML-based config with env overrides (AIR-002)
- ⏸️ gRPC config-store with hot-reload (AIR-003+)
- ✅ Type conversion layer (YAML-friendly → runtime types)
- ✅ Hierarchical config paths

### 2. Runtime Patterns
- ✅ Dual-criteria batching (size + time)
- ✅ Dead Letter Queue for resilience
- ✅ Exponential backoff retry
- ✅ Write-Ahead Log for durability

### 3. Integration Patterns
- ✅ Reuse existing components (MQTT, Parser, Storage)
- ✅ Trait-based abstractions (Source, Store)
- ✅ Arc + Mutex for shared state
- ✅ mpsc channels for backpressure

### 4. Observability Patterns
- ✅ Metrics (Prometheus-compatible)
- ✅ Health checks (component status)
- ✅ Structured logging (tracing crate)
- ✅ DLQ for debugging

---

## References

**Primary Documents**:
1. `/product/features/air-002/architecture/01-system-design.md` - System architecture
2. `/product/features/air-002/specs/01-specification.md` - Requirements
3. `/product/features/air-002/implementation/05-config-implementation-guide.md` - Config guide
4. `/product/features/air-002/implementation/02-config-scope-analysis.md` - Scope analysis
5. `/product/features/air-002/validation-report.md` - Production validation
6. `/product/features/air-002/analysis/CONFIG_SWARM_RECOMMENDATIONS.md` - Config strategy

**Related Features**:
- AIR-001: Air Quality Module (parent feature)
- AIR-003: Configuration Standardization (successor)

**Platform Components**:
- `/core/src/sources/mqtt.rs` - MQTT client
- `/core/src/storage/parquet.rs` - Parquet storage
- `/domains/air-quality/src/parser.rs` - AirGradient parser
- `/config-store/` - Platform config service

---

**Document Version**: 1.0.0
**Last Updated**: 2025-12-14
**Status**: Complete Architecture Analysis
