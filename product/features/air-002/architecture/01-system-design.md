# AIR-002: Ingestion Pipeline Architecture

**Version:** 1.0.0
**Date:** 2025-12-14
**Author:** System Architect
**Status:** Design Phase

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Component Diagram](#component-diagram)
3. [Component Responsibilities](#component-responsibilities)
4. [Interface Contracts](#interface-contracts)
5. [Concurrency Model](#concurrency-model)
6. [Error Handling Strategy](#error-handling-strategy)
7. [Configuration Architecture](#configuration-architecture)
8. [Implementation Plan](#implementation-plan)
9. [ADRs](#architecture-decision-records)

---

## Executive Summary

AIR-002 wires existing components into a complete MQTT → Storage pipeline. **No new core functionality is required** - all components exist and are tested. This design focuses on integration, concurrency, and error handling.

### Key Principle
**REUSE OVER REWRITE**: Leverage existing traits, implementations, and domain logic. Add only the minimal orchestration needed to wire components together.

### System Flow
```
AirGradient Sensor → MQTT Broker → MqttSource → Parser → Validator → Adapter → ParquetStore (WAL) → REST API
```

---

## Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         AIR QUALITY APPLICATION                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────┐   │
│  │                    INGESTION PIPELINE                           │   │
│  │                                                                  │   │
│  │  ┌──────────┐    ┌─────────┐    ┌──────────┐    ┌──────────┐ │   │
│  │  │  MQTT    │───▶│ Parser  │───▶│Validator │───▶│ Adapter  │ │   │
│  │  │  Source  │    │         │    │          │    │          │ │   │
│  │  └──────────┘    └─────────┘    └──────────┘    └──────────┘ │   │
│  │       │                                               │         │   │
│  │       │ (raw MQTT)                          (TimeSeriesPoint)  │   │
│  │       │                                               ▼         │   │
│  │       │                                        ┌───────────┐   │   │
│  │       │                                        │  Parquet  │   │   │
│  │       │                                        │  Store    │   │   │
│  │       │                                        │  + WAL    │   │   │
│  │       │                                        └───────────┘   │   │
│  │       │                                               │         │   │
│  └───────┼───────────────────────────────────────────────┼─────────┘   │
│          │                                               │              │
│  ┌───────▼─────────┐                             ┌──────▼────────┐    │
│  │  Error Handler  │                             │  REST API     │    │
│  │  + DLQ          │                             │  (Query)      │    │
│  └─────────────────┘                             └───────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘

External Components:
┌─────────────────┐         ┌──────────────────┐
│ AirGradient     │  MQTT   │  MQTT Broker     │
│ Sensor (HW)     │────────▶│  (Mosquitto)     │
└─────────────────┘         └──────────────────┘
```

### Data Flow Sequence

```
1. Sensor → MQTT Broker
   - Topic: airgradient/readings/{SERIAL_NUMBER}
   - Payload: JSON with subset of 29 fields
   - QoS: 1 (at least once)

2. MqttSource (Background Task)
   - Subscribes to wildcard: airgradient/readings/+
   - Receives messages via rumqttc event loop
   - Places raw payloads in mpsc channel
   - Auto-reconnects on failure

3. Parser (Per Message)
   - Consumes from mpsc channel
   - Parses JSON → AirQualityReading
   - Handles partial data gracefully

4. Validator (Per Message)
   - Validates sensor ranges
   - Collects all errors
   - Sends invalid readings to DLQ

5. Adapter (Per Message)
   - Converts AirQualityReading → Vec<TimeSeriesPoint>
   - One point per metric (co2, pm25, temp, etc.)
   - Preserves metadata in tags

6. Storage (Batched)
   - Writes to WAL immediately
   - Batches points to Parquet files
   - Partitioned by location/year/month/day

7. Query (On Demand)
   - REST API reads from Parquet
   - Uses partition pruning
   - Returns TimeSeriesPoint format
```

---

## Component Responsibilities

### 1. MqttSource (`core/src/sources/mqtt.rs`)

**Status:** ✅ Exists, needs instantiation

**Responsibilities:**
- Connect to MQTT broker with auto-reconnect
- Subscribe to topic pattern: `airgradient/readings/+`
- Parse incoming messages to `TimeSeriesPoint`
- Buffer messages in bounded mpsc channel
- Expose via `Source` trait

**Key Methods:**
```rust
impl MqttSource {
    pub fn new(config: MqttConfig) -> Self;
    pub async fn start(&mut self) -> CoreResult<()>;
    pub async fn stop(&mut self) -> CoreResult<()>;
}

#[async_trait]
impl Source for MqttSource {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

**Current Gap:**
- Built-in parser converts directly to TimeSeriesPoint
- Need to modify to return raw payloads OR accept domain parser

**Proposed Change:**
```rust
// Option A: Return raw payloads (minimal change)
pub struct MqttSource {
    // ... existing fields
    raw_mode: bool, // If true, skip parsing
}

// Option B: Accept injected parser (dependency inversion)
pub struct MqttSource<P: Parser> {
    parser: P,
    // ... existing fields
}
```

### 2. Parser (`domains/air-quality/src/parser.rs`)

**Status:** ✅ Exists, ready to use

**Responsibilities:**
- Parse JSON payloads to `AirQualityReading`
- Handle partial data (Option fields)
- Support both MQTT and Local API formats
- Add timestamp if missing

**Key Functions:**
```rust
pub fn parse_mqtt_payload(json: &str) -> Result<AirQualityReading, ParserError>;
pub fn parse_local_api_payload(json: &str) -> Result<AirQualityReading, ParserError>;
```

**No changes needed** - ready to use as-is.

### 3. Validator (`domains/air-quality/src/validation.rs`)

**Status:** ✅ Exists, ready to use

**Responsibilities:**
- Validate sensor reading ranges per hardware specs
- Collect all validation errors
- Return structured errors with context

**Key Functions:**
```rust
pub fn validate_reading(reading: &AirQualityReading) -> Result<(), ValidationError>;
```

**Validation Ranges:**
- CO2: 380-10,000 ppm (SenseAir S8)
- PM: 0-500 µg/m³ (PMS5003)
- TVOC/NOx Index: 1-500 (SGP41)
- Temperature: -10 to 50°C (SHT40)
- Humidity: 0-100% (SHT40)
- WiFi: -100 to 0 dBm

**No changes needed** - ready to use as-is.

### 4. Adapter (`domains/air-quality/src/adapter.rs`)

**Status:** ✅ Exists, ready to use

**Responsibilities:**
- Convert `AirQualityReading` → `Vec<TimeSeriesPoint>`
- Create one point per metric
- Add metadata tags (firmware, model, metric name)
- Preserve timestamp and location_id

**Key Methods:**
```rust
impl AirQualityAdapter {
    pub fn to_time_series_points(reading: &AirQualityReading) -> Vec<TimeSeriesPoint>;
    pub fn extract_metric(reading: &AirQualityReading, metric_name: &str) -> Option<TimeSeriesPoint>;
    pub fn available_metrics(reading: &AirQualityReading) -> Vec<String>;
}
```

**No changes needed** - ready to use as-is.

### 5. ParquetStore (`core/src/storage/parquet.rs`)

**Status:** ✅ Exists, needs instantiation

**Responsibilities:**
- Write time series points to Parquet files
- Maintain Write-Ahead Log (WAL) for durability
- Partition by location/year/month/day
- Support batch writes
- Handle WAL replay on startup

**Key Methods:**
```rust
impl ParquetStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> CoreResult<Self>;
    pub async fn replay_wal(&self) -> CoreResult<()>;
}

#[async_trait]
impl Store for ParquetStore {
    async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()>;
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()>;
    async fn query(...) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

**No changes needed** - ready to use as-is.

### 6. IngestionPipeline (NEW)

**Status:** ❌ Needs implementation

**Location:** `apps/air-quality-app/src/ingestion/pipeline.rs`

**Responsibilities:**
- Orchestrate MQTT → Parser → Validator → Adapter → Storage
- Manage concurrent tasks
- Handle errors and send to DLQ
- Expose health status

**Proposed Interface:**
```rust
pub struct IngestionPipeline {
    mqtt_source: Arc<MqttSource>,
    store: Arc<ParquetStore>,
    config: PipelineConfig,
    shutdown: broadcast::Sender<()>,
}

pub struct PipelineConfig {
    pub batch_size: usize,
    pub batch_timeout: Duration,
    pub max_retries: u32,
    pub dlq_path: PathBuf,
}

impl IngestionPipeline {
    pub fn new(
        mqtt_source: Arc<MqttSource>,
        store: Arc<ParquetStore>,
        config: PipelineConfig,
    ) -> Self;

    pub async fn start(&mut self) -> Result<()>;
    pub async fn stop(&mut self) -> Result<()>;
    pub async fn health(&self) -> HealthStatus;
}
```

---

## Interface Contracts

### Trait: Source
```rust
#[async_trait]
pub trait Source: Send + Sync {
    /// Fetch available data points (non-blocking, drains buffer)
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Check connection health
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

**Contract:**
- `fetch()` MUST be non-blocking
- `fetch()` MUST drain internal buffer (not peek)
- `health_check()` MUST reflect current connection state
- Implementation MUST handle reconnection internally

### Trait: Store
```rust
#[async_trait]
pub trait Store: Send + Sync {
    /// Write single point (logs to WAL)
    async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()>;

    /// Write batch (optimized, logs to WAL)
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()>;

    /// Query time range
    async fn query(
        &self,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: Option<HashMap<String, String>>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Check storage health
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

**Contract:**
- `write()` MUST log to WAL before returning
- `write_batch()` SHOULD be atomic (all or nothing)
- `query()` MUST return sorted by timestamp
- `health_check()` MUST verify write permissions

### Domain Functions

**Parser:**
```rust
pub fn parse_mqtt_payload(json: &str) -> Result<AirQualityReading, ParserError>;
```
**Contract:**
- MUST accept partial JSON (Option fields)
- MUST return error for invalid JSON
- MUST return error if `serialno` missing
- MUST NOT panic on malformed input

**Validator:**
```rust
pub fn validate_reading(reading: &AirQualityReading) -> Result<(), ValidationError>;
```
**Contract:**
- MUST validate all present (non-None) fields
- MUST skip None fields (not an error)
- MUST collect ALL errors before returning
- MUST use hardware-spec ranges

**Adapter:**
```rust
impl AirQualityAdapter {
    pub fn to_time_series_points(reading: &AirQualityReading) -> Vec<TimeSeriesPoint>;
}
```
**Contract:**
- MUST create one point per non-None metric
- MUST preserve original timestamp OR use Utc::now()
- MUST include metric name in tags
- MUST include firmware/model if available

---

## Concurrency Model

### Task Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Main Tokio Runtime                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Task 1: MQTT Event Loop (spawned by MqttSource)           │
│  ├─ Polls rumqttc AsyncClient                              │
│  ├─ Sends messages to mpsc channel                         │
│  └─ Handles reconnection with exponential backoff          │
│                                                              │
│  Task 2: Ingestion Loop (spawned by Pipeline)              │
│  ├─ Receives from mpsc channel (bounded 1000)              │
│  ├─ Parses → Validates → Adapts                            │
│  ├─ Batches points (size=100 OR timeout=1s)                │
│  └─ Writes to ParquetStore                                 │
│                                                              │
│  Task 3: HTTP Server (Axum)                                │
│  ├─ Handles REST API requests                              │
│  ├─ Queries ParquetStore                                   │
│  └─ Shares Arc<ParquetStore> with Task 2                   │
│                                                              │
│  Task 4: Health Monitor (optional)                         │
│  ├─ Polls health every 30s                                 │
│  ├─ Updates metrics                                        │
│  └─ Logs warnings                                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Shared State

```rust
// Thread-safe shared state
pub struct AppState {
    pub mqtt_source: Arc<MqttSource>,
    pub store: Arc<ParquetStore>,
    pub pipeline: Arc<Mutex<IngestionPipeline>>,
}
```

**Ownership:**
- `MqttSource`: Arc-shared, internally uses Mutex for client
- `ParquetStore`: Arc-shared, internally uses Mutex for WAL
- `IngestionPipeline`: Mutex-wrapped for start/stop control

### Channels

**MQTT Message Channel:**
```rust
let (tx, rx) = mpsc::channel::<TimeSeriesPoint>(1000);
```
- **Capacity:** 1000 messages (backpressure)
- **Sender:** MqttSource (in event loop task)
- **Receiver:** IngestionPipeline
- **Backpressure:** Blocks MQTT processing if full (prevents OOM)

**Shutdown Channel:**
```rust
let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
```
- **Type:** broadcast (multiple receivers)
- **Senders:** main() on SIGTERM/SIGINT
- **Receivers:** All spawned tasks
- **Cleanup:** Each task drains buffer before exit

### Concurrency Patterns

**1. Message Processing:**
```rust
// Ingestion loop
loop {
    tokio::select! {
        Some(payload) = mqtt_rx.recv() => {
            // Process message
            match parse_and_validate(payload) {
                Ok(points) => batch.extend(points),
                Err(e) => dlq.write(payload, e),
            }

            // Flush batch
            if batch.len() >= 100 || last_flush.elapsed() > 1s {
                store.write_batch(batch.drain(..).collect()).await?;
            }
        }
        _ = shutdown_rx.recv() => {
            // Flush remaining
            store.write_batch(batch.drain(..).collect()).await?;
            break;
        }
    }
}
```

**2. Batch Optimization:**
- **Size-based:** Flush at 100 points
- **Time-based:** Flush after 1 second
- **Shutdown:** Immediate flush

**3. Error Isolation:**
- Parser errors don't crash pipeline
- Validator errors sent to DLQ
- Storage errors retry with backoff
- MQTT errors trigger reconnect

---

## Error Handling Strategy

### Error Categories

**1. Transient Errors (Retry):**
- MQTT connection lost
- Storage temporarily unavailable
- Network timeout

**Strategy:** Exponential backoff retry
```rust
for attempt in 0..max_retries {
    match operation().await {
        Ok(result) => return Ok(result),
        Err(e) if e.is_transient() => {
            let delay = min(base_delay * 2^attempt, max_delay);
            tokio::time::sleep(delay).await;
        }
        Err(e) => return Err(e),
    }
}
```

**2. Permanent Errors (DLQ):**
- Invalid JSON
- Validation failures
- Missing required fields

**Strategy:** Write to Dead Letter Queue
```rust
struct DLQEntry {
    payload: Vec<u8>,
    error: String,
    timestamp: DateTime<Utc>,
    attempt_count: u32,
}

impl DeadLetterQueue {
    async fn write(&self, payload: Vec<u8>, error: &dyn Error) -> Result<()>;
    async fn list(&self) -> Result<Vec<DLQEntry>>;
    async fn retry(&self, entry_id: &str) -> Result<()>;
}
```

**3. Fatal Errors (Crash):**
- Configuration invalid
- Storage directory unwritable
- WAL corrupted

**Strategy:** Panic with clear error message
```rust
// On startup
let store = ParquetStore::new(&config.storage_path)
    .expect("FATAL: Cannot create storage - check permissions");
```

### Error Flow

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

### Dead Letter Queue Implementation

**Location:** `data/dlq/YYYY-MM-DD.jsonl`

**Format:**
```json
{"timestamp":"2024-01-15T10:30:00Z","error":"CO2 out of range: 100","payload":"{\"serialno\":\"abc\",\"rco2\":100}","attempts":1}
```

**Management:**
- Rotate daily
- Compress after 7 days
- Delete after 30 days
- Expose via REST API for retry

---

## Configuration Architecture

### Configuration Hierarchy

```
config.yaml (file)
     ↓
Environment Variables (override)
     ↓
Defaults (fallback)
```

### Config Structure

**File:** `apps/air-quality-app/config.yaml`
```yaml
server:
  host: "0.0.0.0"
  port: 3000

mqtt:
  broker_url: "mqtt://localhost:1883"
  client_id: "air-quality-app"
  topic_pattern: "airgradient/readings/+"
  qos: 1
  buffer_capacity: 1000
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30

storage:
  type: "parquet"
  base_path: "./data/parquet"
  wal_path: "./data/wal"

pipeline:
  batch_size: 100
  batch_timeout_secs: 1
  max_retries: 3
  dlq_path: "./data/dlq"

logging:
  level: "info"
  format: "json"
```

### Config Flow

```
main.rs
  │
  ├─▶ Load config.yaml
  │
  ├─▶ Create MqttConfig from config.mqtt
  │     └─▶ MqttSource::new(mqtt_config)
  │
  ├─▶ Create StorageConfig from config.storage
  │     └─▶ ParquetStore::new(storage_config.base_path)
  │
  ├─▶ Create PipelineConfig from config.pipeline
  │     └─▶ IngestionPipeline::new(..., pipeline_config)
  │
  └─▶ Start server on config.server.host:port
```

### Environment Variable Overrides

```bash
# Example overrides
MQTT_BROKER_URL=mqtt://prod-broker:1883
STORAGE_BASE_PATH=/mnt/data/parquet
LOG_LEVEL=debug
```

**Mapping:**
```rust
impl AppConfig {
    pub fn from_env() -> Self {
        let mut config = Self::from_yaml("config.yaml").unwrap_or_default();

        if let Ok(url) = env::var("MQTT_BROKER_URL") {
            config.mqtt.broker_url = url;
        }
        if let Ok(path) = env::var("STORAGE_BASE_PATH") {
            config.storage.base_path = path;
        }

        config
    }
}
```

---

## Implementation Plan

### Phase 1: Core Integration (Files to Modify)

**1. Create Pipeline Module**

*File:* `apps/air-quality-app/src/ingestion/mod.rs` (NEW)
```rust
mod pipeline;
pub use pipeline::{IngestionPipeline, PipelineConfig};
```

*File:* `apps/air-quality-app/src/ingestion/pipeline.rs` (NEW)
```rust
// Wire Parser → Validator → Adapter → Storage
pub struct IngestionPipeline { ... }
```
**Lines of Code:** ~200

**2. Modify MqttSource Integration**

*File:* `apps/air-quality-app/src/sources/mod.rs` (NEW)
```rust
// Re-export core MqttSource or wrap with domain parser
pub use platform_core::sources::MqttSource;
```
**Lines of Code:** ~50

**3. Update Main Application**

*File:* `apps/air-quality-app/src/main.rs` (MODIFY)

Changes:
```rust
// Remove mock implementations
// Add real implementations

use air_quality::parser;
use air_quality::validation;
use air_quality::adapter::AirQualityAdapter;
use platform_core::storage::ParquetStore;
use platform_core::sources::MqttSource;

async fn main() -> Result<()> {
    let config = AppConfig::from_yaml("config.yaml")?;

    // Create storage
    let store = Arc::new(ParquetStore::new(&config.storage.base_path)?);
    store.replay_wal().await?;

    // Create MQTT source
    let mqtt_config = MqttConfig {
        broker_url: config.mqtt.broker_url.clone(),
        port: 1883,
        client_id: config.mqtt.client_id.clone(),
        topic_pattern: config.mqtt.topic_pattern.clone(),
        qos: QoS::AtLeastOnce,
        reconnect_delay: Duration::from_secs(config.mqtt.reconnect_delay_secs),
        max_reconnect_delay: Duration::from_secs(config.mqtt.max_reconnect_delay_secs),
        buffer_capacity: config.mqtt.buffer_capacity,
    };
    let mut mqtt_source = Arc::new(MqttSource::new(mqtt_config));

    // Start MQTT ingestion
    mqtt_source.start().await?;

    // Create ingestion pipeline
    let pipeline_config = PipelineConfig {
        batch_size: config.pipeline.batch_size,
        batch_timeout: Duration::from_secs(config.pipeline.batch_timeout_secs),
        max_retries: config.pipeline.max_retries,
        dlq_path: PathBuf::from(&config.pipeline.dlq_path),
    };
    let mut pipeline = IngestionPipeline::new(
        mqtt_source.clone(),
        store.clone(),
        pipeline_config,
    );

    // Start pipeline
    pipeline.start().await?;

    // Start HTTP server
    let app = create_router(AppServices {
        store: store.clone(),
        source: mqtt_source.clone(),
        // ... other services
    });

    // ... server startup
}
```
**Lines Modified:** ~100

**4. Update Config Structs**

*File:* `apps/air-quality-app/src/config.rs` (MODIFY)

Add:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub batch_size: usize,
    pub batch_timeout_secs: u64,
    pub max_retries: u32,
    pub dlq_path: String,
}
```
**Lines Added:** ~20

### Phase 2: Error Handling (New Files)

**1. Create DLQ Module**

*File:* `apps/air-quality-app/src/ingestion/dlq.rs` (NEW)
```rust
pub struct DeadLetterQueue { ... }
impl DeadLetterQueue {
    pub async fn write(&self, payload: Vec<u8>, error: &dyn Error) -> Result<()>;
    pub async fn list(&self) -> Result<Vec<DLQEntry>>;
    pub async fn retry(&self, entry_id: &str) -> Result<()>;
}
```
**Lines of Code:** ~150

### Phase 3: Health & Monitoring (File Modifications)

**1. Add Health Endpoint**

*File:* `apps/air-quality-app/src/api/handlers/health.rs` (MODIFY)

Update to check:
- MQTT connection status
- Storage health
- Pipeline status
- Last message timestamp

**Lines Modified:** ~50

### Summary of File Changes

**New Files:**
1. `apps/air-quality-app/src/ingestion/mod.rs` (~10 lines)
2. `apps/air-quality-app/src/ingestion/pipeline.rs` (~200 lines)
3. `apps/air-quality-app/src/ingestion/dlq.rs` (~150 lines)

**Modified Files:**
1. `apps/air-quality-app/src/main.rs` (~100 lines modified)
2. `apps/air-quality-app/src/config.rs` (~20 lines added)
3. `apps/air-quality-app/src/api/handlers/health.rs` (~50 lines modified)
4. `apps/air-quality-app/Cargo.toml` (add rumqttc dependency)

**Total New Code:** ~360 lines
**Total Modified Code:** ~170 lines
**No Core Changes Required** - All existing components used as-is

---

## Architecture Decision Records

### ADR-001: Reuse Core MqttSource vs Domain-Specific Wrapper

**Status:** ACCEPTED

**Context:**
- `core/src/sources/mqtt.rs` has built-in parser for AirGradient format
- Domain parser in `domains/air-quality/src/parser.rs` is more comprehensive
- Need to decide: modify core or wrap it?

**Decision:**
Use core MqttSource as-is. It already parses to TimeSeriesPoint correctly.

**Rationale:**
- Core MqttSource.parse_payload() handles same JSON format
- Validation can happen after fetching from Source trait
- Reduces code duplication
- Adapter pattern already bridges TimeSeriesPoint → Storage

**Consequences:**
- Positive: No changes to core components
- Positive: Clear separation: Source→Traits, Domain→Validation
- Negative: Parser logic duplicated (acceptable for independence)
- Mitigation: Can refactor later if needed

### ADR-002: Batching Strategy

**Status:** ACCEPTED

**Context:**
- Storage writes are expensive (Parquet file I/O)
- MQTT messages arrive individually
- Need balance between latency and throughput

**Decision:**
Batch writes using dual criteria:
- Size: Flush at 100 points
- Time: Flush after 1 second

**Rationale:**
- Prevents small file proliferation
- Ensures max 1s latency for queries
- WAL provides durability before batch flush
- 100 points ≈ 1 reading from 100 metrics OR 100 readings from 1 sensor

**Consequences:**
- Positive: Good write performance
- Positive: Acceptable query latency
- Negative: Slightly more complex than immediate writes
- Mitigation: Well-tested pattern in data pipelines

### ADR-003: Error Handling via DLQ

**Status:** ACCEPTED

**Context:**
- Invalid readings should not crash pipeline
- Need visibility into failed messages
- Need ability to retry after fixes

**Decision:**
Write invalid messages to Dead Letter Queue (DLQ) as JSONL files.

**Rationale:**
- Simple implementation (file append)
- Easy to inspect and retry
- Standard pattern in message processing
- Prevents data loss

**Consequences:**
- Positive: Resilient pipeline
- Positive: Debugging capability
- Negative: Requires DLQ management
- Mitigation: Auto-rotation and cleanup

### ADR-004: Concurrency Model

**Status:** ACCEPTED

**Context:**
- Need concurrent MQTT processing and HTTP serving
- Rust async/await with Tokio runtime
- Need to share state safely

**Decision:**
Use Tokio with Arc + Mutex for shared state, mpsc for message passing.

**Rationale:**
- Standard Rust async pattern
- Arc provides cheap cloning
- Mutex for interior mutability where needed
- mpsc provides backpressure

**Consequences:**
- Positive: Safe concurrency
- Positive: Bounded memory usage
- Negative: Mutex contention possible
- Mitigation: Minimize critical sections

### ADR-005: Configuration via YAML

**Status:** ACCEPTED

**Context:**
- Need runtime configuration (broker URL, ports, etc.)
- Deployment in Docker and bare metal
- Need environment overrides

**Decision:**
Use YAML files with environment variable overrides.

**Rationale:**
- Human-readable defaults
- Standard for Rust services
- Environment variables for secrets
- serde_yaml mature and stable

**Consequences:**
- Positive: Easy to configure
- Positive: Docker-friendly
- Negative: Another dependency
- Mitigation: Dependency is tiny and stable

---

## Testing Strategy

### Unit Tests (Existing)
- ✅ Parser tests (domains/air-quality)
- ✅ Validator tests (domains/air-quality)
- ✅ Adapter tests (domains/air-quality)
- ✅ ParquetStore tests (core)
- ✅ MqttSource tests (core)

### Integration Tests (New)

**Test:** `tests/ingestion_pipeline.rs`
```rust
#[tokio::test]
async fn test_mqtt_to_storage_flow() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Simulate MQTT message
    let payload = r#"{"serialno":"test","pm02":12.5,"rco2":450}"#;

    // Process through pipeline
    let reading = parser::parse_mqtt_payload(payload).unwrap();
    validation::validate_reading(&reading).unwrap();
    let points = AirQualityAdapter::to_time_series_points(&reading);
    store.write_batch(points).await.unwrap();

    // Query and verify
    let results = store.query("test", start, end, None).await.unwrap();
    assert_eq!(results.len(), 2); // pm02 and rco2
}
```

### E2E Tests (After Implementation)

**Scenario:** Full pipeline with real MQTT broker
```rust
#[tokio::test]
#[ignore] // Requires Docker
async fn test_e2e_ingestion() {
    // Start Mosquitto in Docker
    // Start application
    // Publish test message
    // Query API
    // Assert data visible
}
```

---

## Performance Considerations

### Throughput Targets

**Single Sensor:**
- 1 reading/minute = 1,440 readings/day
- 5 metrics/reading = 7,200 points/day
- Negligible load

**100 Sensors:**
- 144,000 points/day
- ~2 points/second average
- Batch writes easily handle this

### Latency Targets

**Data Path:**
- MQTT delivery: < 100ms
- Parse + Validate: < 1ms
- Batch accumulation: < 1s
- Parquet write: < 100ms
- **Total: < 1.2s sensor → queryable**

### Storage Projections

**Per Sensor Per Year:**
- 525,600 readings (1/min)
- 2.6M points (5 metrics/reading)
- ~50MB Parquet (compressed)

**100 Sensors:**
- 5GB/year
- Easily scalable

---

## Security Considerations

### Authentication (Future)
- MQTT: TLS + client certificates
- HTTP: JWT tokens
- Storage: File permissions

### Data Privacy
- No PII in readings
- Device serial numbers anonymizable
- Location data via separate mapping

### Deployment
- Run as non-root user
- Read-only filesystem except data dir
- Network policies (MQTT and HTTP only)

---

## Appendix: Key Type Definitions

### AirQualityReading
```rust
pub struct AirQualityReading {
    pub device: DeviceMetadata,       // serialno, wifi, firmware, model
    pub particles: ParticleData,      // pm01, pm02, pm10, counts
    pub gases: GasData,               // tvoc_index, nox_index
    pub environment: EnvironmentalData, // temp, humidity
    pub metrics: QualityMetrics,      // co2
    pub timestamp: Option<DateTime<Utc>>,
}
```

### TimeSeriesPoint
```rust
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,    // Device serialno
    pub value: f64,             // Metric value
    pub tags: HashMap<String, String>, // metric, firmware, model, etc.
}
```

### MqttConfig
```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,
    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
}
```

---

## Next Steps

1. **Review & Approve** this architecture document
2. **Create AIR-002.1** for Pipeline implementation (IngestionPipeline)
3. **Create AIR-002.2** for DLQ implementation
4. **Create AIR-002.3** for Integration testing
5. **Update AIR-001** to wire in real pipeline (replace mocks)

**Estimated Total Effort:** 3-5 days
- Day 1: Pipeline implementation
- Day 2: Main.rs integration + config
- Day 3: DLQ + error handling
- Day 4: Testing + debugging
- Day 5: Documentation + deployment guide

---

**Document Version:** 1.0.0
**Last Updated:** 2025-12-14
**Status:** Ready for Implementation
