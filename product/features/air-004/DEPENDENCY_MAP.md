# AIR-004: Component Dependency Map (REVISED)

**Generated:** 2025-12-15
**Status:** Architecture Analysis Complete - **UPDATED WITH EXISTING COMPONENTS**

---

## 0. Existing Component Inventory

### 🟢 FULLY IMPLEMENTED & WORKING

#### apps/air-quality-app/ (~2,813 lines)

**Core Application Components:**
```
[EXISTING] main.rs (265 lines)
  - Complete startup orchestration
  - etcd → config.yaml → defaults hierarchy
  - MQTT + Storage pipeline initialization
  - Health checks and graceful shutdown

[EXISTING] config.rs + config_etcd.rs (432 lines)
  - AppConfig, ServerConfig, MqttConfig, StorageConfig structs
  - YAML parsing with env overrides
  - etcd loading with fallback
  - Config hierarchy: etcd > env > yaml > defaults

[EXISTING] ingestion/mqtt_handler.rs (147 lines)
  - MqttHandler wrapping neural_core::MqttSource
  - Channel-based forwarding to storage
  - Health checks
  - Reconnect logic

[EXISTING] pipeline/storage_writer.rs (150 lines)
  - StorageWriter with batching (default 100 points)
  - Timeout-based flushing (default 5s)
  - Writes to ParquetStore
  - Buffer management with WAL support

[EXISTING] api/routes.rs + handlers/ (~800 lines)
  - REST endpoints: /health, /api/v1/readings, /api/v1/forecast
  - /api/v1/locations, /api/v1/alerts, /api/v1/aggregate
  - Full CORS support
  - Comprehensive test coverage (500+ lines)
```

**What Already Works:**
- ✅ End-to-end MQTT ingestion → Parquet storage
- ✅ Config loading hierarchy (etcd preferred)
- ✅ Batching and WAL for durability
- ✅ REST API with 6+ endpoints
- ✅ Health checks and monitoring
- ✅ AirGradient sensor parsing

#### config-client/ (~260 lines)

```
[EXISTING] client.rs (100+ lines)
  - ConfigClient with etcd connection
  - get/set/delete operations
  - list() for prefix queries
  - get_with_env() for env override support
  - Typed deserialization via serde

[EXISTING] watch.rs (~100 lines)
  - WatchHandle for hot-reload
  - Callback-based change notifications
  - Async watch streams
```

**What Already Works:**
- ✅ Connect to etcd cluster
- ✅ CRUD operations with JSON serialization
- ✅ Watch streams for config changes
- ✅ Environment variable precedence

#### neural-core/ (if ParquetStore exists)

```
[EXISTING] ParquetStore (location: core/src/storage/parquet.rs)
  - write() and write_batch() operations
  - WAL support for crash recovery
  - TimeSeriesPoint storage
  - Query interface

[EXISTING] TimeSeriesPoint (location: core/src/traits.rs or types)
  - timestamp: DateTime<Utc>
  - location_id: String
  - value: f64
  - tags: HashMap<String, String>

[EXISTING] MqttSource (location: core/src/sources/)
  - Connects to MQTT broker
  - Topic subscription with wildcards
  - fetch() returns Vec<TimeSeriesPoint>
  - Health checks

[EXISTING] Trait interfaces
  - trait Source (fetch, health_check)
  - trait Store (write, write_batch, query, aggregate)
  - trait Forecast (train, predict, evaluate)
```

**What Already Works:**
- ✅ Parquet file writing
- ✅ MQTT source abstraction
- ✅ TimeSeriesPoint data model
- ✅ Common trait interfaces

### 🟡 INFRASTRUCTURE RUNNING

```
[EXISTING] etcd v3.5.11
  - Running in Docker
  - Ports: 2379 (client), 2380 (peer)
  - Health checks configured
  - Data persistence with volumes

[EXISTING] Mosquitto 2.0
  - Running in Docker
  - Ports: 1883 (MQTT), 9001 (WebSocket)
  - Config at mosquitto/config/mosquitto.conf
  - Data and log volumes

[EXISTING] Parquet Files
  - Location: ./data/parquet/ or /app/data/parquet
  - WAL directory for crash recovery
  - Already receiving AirGradient data

[PARTIAL] TimescaleDB
  - Service defined but NOT integrated with air-quality-app
  - No schema yet
  - No adapter implementation
```

### 🔴 NOT IMPLEMENTED (Truly New)

```
[NEW] TimescaleDB Adapter
[NEW] DDL Generator for dynamic schemas
[NEW] Stream Registry (multi-stream support)
[NEW] Ingestion Router (schema validation)
[NEW] Storage Layer Manager (dual-write Bronze+Silver)
[NEW] Source Manager (dynamic source spawning)
[NEW] HttpPoller Source
[NEW] WebhookHandler Source
[NEW] Ingestion Coordinator (orchestrator binary)
[NEW] Grafana dashboards for TimescaleDB
```

---

## 1. Component Dependency Graph (REVISED)

```
EXTERNAL INFRASTRUCTURE (EXISTING ✅)
┌──────────────────────────────────────────────────────────────────────┐
│  ┌─────────┐    ┌──────────────┐    ┌────────────┐                  │
│  │  etcd   │    │ TimescaleDB  │    │ MQTT Broker│                  │
│  │ v3.5.11 │    │ (not wired)  │    │ (Mosquitto)│                  │
│  │ [EXIST] │    │ [PARTIAL]    │    │ [EXIST]    │                  │
│  └────┬────┘    └──────┬───────┘    └─────┬──────┘                  │
│       │                │                   │                         │
└───────┼────────────────┼───────────────────┼─────────────────────────┘
        │                │                   │
        │ config         │ (not used yet)    │ topics
        │                │                   │
┌───────▼────────────────────────────────────▼─────────────────────────┐
│                   CORE PLATFORM COMPONENTS                           │
│                                                                      │
│  ┌──────────────────────────────────────────────────┐               │
│  │  STREAM REGISTRY CLIENT [NEW - EXTEND config-   │               │
│  │         client for multi-stream support]         │               │
│  │  - Loads stream configs from etcd                │               │
│  │  - Watch for updates [REUSE watch.rs]            │               │
│  │  - Schema validation [NEW]                       │               │
│  └────────────────┬─────────────────────────────────┘               │
│                   │ provides: StreamConfig                          │
│  ┌────────────────▼─────────────────────────────────┐               │
│  │      INGESTION COORDINATOR [NEW - but copies     │               │
│  │         patterns from air-quality-app/main.rs]   │               │
│  │  - Spawns sources based on registry              │               │
│  │  - Routes records to storage layers              │               │
│  └───┬────────────────────────────┬─────────────────┘               │
│      │ spawns                     │ routes                          │
│  ┌───▼─────────────────┐     ┌────▼───────────────────────┐         │
│  │  SOURCE MANAGER     │     │  INGESTION ROUTER [NEW]    │         │
│  │     [NEW]           │     │  - Schema validation       │         │
│  │                     │     │  - Stream routing          │         │
│  │  ┌───────────────┐  │     └────┬──────────────┬────────┘         │
│  │  │  MqttSource   │  │          │              │                  │
│  │  │  [EXISTING ✅]│◄─┼──────────┘ StreamRecord │                  │
│  │  │  neural_core  │  │                         │                  │
│  │  └───────────────┘  │                         │                  │
│  │  ┌───────────────┐  │                         │                  │
│  │  │ HttpPoller    │  │                         │                  │
│  │  │    [NEW]      │◄─┼─────────────────────────┘                  │
│  │  └───────────────┘  │                                            │
│  │  ┌───────────────┐  │                                            │
│  │  │WebhookHandler │  │                                            │
│  │  │    [NEW]      │◄─┼────────────────────────────────────────────┤
│  │  └───────────────┘  │                                            │
│  └─────────────────────┘                                            │
│                          │ StreamRecord                             │
│  ┌───────────────────────▼──────────────────────────┐               │
│  │    STORAGE LAYER MANAGER [NEW - but based on     │               │
│  │       storage_writer.rs batching patterns]       │               │
│  │  - Dual-write coordination                       │               │
│  └────────┬─────────────────────────┬────────────────┘               │
│           │ write                   │ write                          │
│  ┌────────▼─────────┐     ┌─────────▼────────────┐                  │
│  │  ParquetStore    │     │ TimescaleDB Adapter  │                  │
│  │  [EXISTING ✅]   │     │      [NEW]           │                  │
│  │                  │     │                      │                  │
│  │ - write_batch()  │     │ - sqlx-based writer  │                  │
│  │ - WAL support    │     │ - DDL generation     │                  │
│  │ - Bronze storage │     │ - Hypertable mgmt    │                  │
│  └──────────────────┘     └──────────────────────┘                  │
│           │                         │                                │
└───────────┼─────────────────────────┼────────────────────────────────┘
            │                         │
            ▼                         ▼
┌────────────────────┐    ┌────────────────────┐
│  BRONZE LAYER      │    │  SILVER/GOLD       │
│  [EXISTING ✅]     │    │  [NEW]             │
│                    │    │                    │
│  data/parquet/     │    │  hypertables:      │
│  (single stream)   │    │  ├─air_quality     │
│                    │    │  ├─home_events     │
│                    │    │  └─weather         │
└────────────────────┘    └──────┬─────────────┘
                                 │
                          ┌──────▼──────────────┐
                          │ DASHBOARDS [NEW]    │
                          │ - Grafana           │
                          └─────────────────────┘


LEGEND:
  [EXISTING ✅]  - Already implemented and working
  [PARTIAL]      - Service running but not integrated
  [EXTEND]       - Existing component requiring feature additions
  [NEW]          - Truly new component to be built
  ────►          - Data flow direction
  ═══►           - Watch/config flow
```

---

## 2. Build Order and Critical Path (REVISED)

### ⚠️ CRITICAL CHANGE: Start with Verification

**Phase 0: Verification and Protection (1 day)**

```
Priority: CRITICAL (Prevents breaking existing functionality)
Estimated: 1 day

0. Document Current Interfaces
   - Read and document ParquetStore API
   - Read and document TimeSeriesPoint structure
   - Read and document MqttSource interface
   - Read and document config-client API
   - Blocks: All new development
   - Preserve: Existing data files, API contracts

1. Create Integration Tests for Existing Components
   - Test current MQTT → Storage pipeline
   - Test config loading hierarchy
   - Test WAL replay
   - Baseline performance metrics
   - Blocks: Refactoring work
   - Preserve: Current behavior
```

### Phase 1: Foundation (Can Build in Parallel)

**Group 1A: Extend Existing Types**
```
Priority: HIGH (Minimal disruption)
Estimated: 2-3 days

1. Extend TimeSeriesPoint [EXTEND existing]
   - Location: Already exists, verify structure
   - Add stream_id field if not present
   - Ensure backward compatibility with existing Parquet files
   - Dependencies: Read existing definition
   - Blocks: StreamRecord type
   - Preserve: Existing serialization format

2. Create StreamRecord wrapper [NEW]
   - Location: core/src/types/stream_record.rs
   - Wraps TimeSeriesPoint + adds metadata
   - Uses existing TimeSeriesPoint internally
   - Dependencies: TimeSeriesPoint
   - Blocks: Router, new sources
   - Preserve: TimeSeriesPoint interface

3. StreamConfig type [NEW]
   - Location: core/src/types/stream_config.rs
   - Output: StreamConfig, SchemaField structs
   - Dependencies: None
   - Blocks: Registry client, Router
```

**Group 1B: Registry Client [EXTEND config-client]**
```
Priority: HIGH
Estimated: 2-3 days

4. Extend ConfigClient for Streams [EXTEND existing]
   - Location: config-client/src/stream_registry.rs (new file)
   - Reuse existing ConfigClient connection logic
   - Add load_stream(), list_streams() methods
   - Reuse existing watch.rs for stream updates
   - Dependencies: config-client crate, StreamConfig type
   - Blocks: Ingestion Coordinator
   - Preserve: Existing ConfigClient API
```

### Phase 2: Storage Layer (Careful Extension)

**Group 2: Extend Storage [EXTEND + NEW]**
```
Priority: HIGH (Data durability risk)
Estimated: 3-4 days

5. Extend ParquetStore for Multi-Stream [EXTEND existing]
   - Location: core/src/storage/parquet.rs
   - Add stream_id-based partitioning
   - Keep existing write_batch() signature
   - Add write_batch_for_stream() new method
   - Dependencies: Read existing implementation
   - Preserve: Existing single-stream writes
   - Preserve: Existing Parquet file format
   - Preserve: WAL functionality

6. TimescaleDB Adapter [NEW - but copy patterns]
   - Location: core/src/storage/timescale.rs
   - Copy batching patterns from storage_writer.rs
   - Use sqlx (new dependency)
   - Dependencies: StreamConfig, StreamRecord
   - Does NOT block existing pipeline
   - Preserve: Nothing (net new)

7. DDL Generator [NEW]
   - Location: core/src/storage/ddl_generator.rs
   - Generate CREATE HYPERTABLE from StreamConfig
   - Dependencies: StreamConfig
   - Required by: TimescaleDB Adapter
```

### Phase 3: Sources (Mostly New)

**Group 3: Source Implementations**
```
Priority: MEDIUM
Estimated: 3-4 days

8. Verify MqttSource Interface [EXISTING - document only]
   - Location: neural-core/src/sources/mqtt.rs
   - Document current fetch() signature
   - Confirm returns Vec<TimeSeriesPoint>
   - NO CHANGES if it already works
   - Preserve: Existing MQTT functionality

9. Adapt MqttSource for StreamRecord [EXTEND if needed]
   - Only if multi-stream support is needed
   - Add stream_id parameter to constructor
   - Wrap TimeSeriesPoint → StreamRecord
   - Dependencies: StreamRecord type
   - Preserve: Backward compatibility

10. HttpPoller [NEW]
    - Location: core/src/sources/http.rs
    - Copy error handling from mqtt_handler.rs
    - Dependencies: StreamRecord
    - Can work in parallel with WebhookHandler

11. WebhookHandler [NEW]
    - Location: core/src/sources/webhook.rs
    - Use Axum (already in air-quality-app)
    - Dependencies: StreamRecord, Axum
    - Can work in parallel with HttpPoller
```

### Phase 4: Coordination Layer (New but Copy Patterns)

**Group 4: Coordination Components**
```
Priority: CRITICAL PATH
Estimated: 4-5 days

12. Ingestion Router [NEW]
    - Location: core/src/ingestion/router.rs
    - Copy validation patterns from existing handlers
    - Dependencies: StreamRecord, StreamConfig
    - Blocks: Storage Layer Manager

13. Storage Layer Manager [NEW]
    - Location: core/src/storage/manager.rs
    - Copy batching logic from storage_writer.rs
    - Coordinate dual writes
    - Dependencies: ParquetStore (extended), TimescaleDB Adapter
    - Blocks: Ingestion Coordinator

14. Source Manager [NEW]
    - Location: core/src/sources/manager.rs
    - Copy startup patterns from air-quality-app/main.rs
    - Dynamic source spawning
    - Dependencies: All source implementations
    - Blocks: Ingestion Coordinator

15. Ingestion Coordinator [NEW - copy from air-quality-app]
    - Location: apps/ingestion-coordinator/src/main.rs
    - Copy main.rs structure from air-quality-app
    - Copy config loading hierarchy pattern
    - Dependencies: ALL above components
    - Build: LAST in critical path
    - Preserve: air-quality-app should still work standalone
```

### Phase 5: Deployment and Visualization

**Group 5: Consumption Layer**
```
Priority: MEDIUM (Post-MVP)
Estimated: 2-3 days

16. TimescaleDB Schema Initialization
    - Location: docker/timescaledb/init.sql
    - Run DDL for initial streams
    - Dependencies: TimescaleDB Adapter deployed

17. Grafana Dashboards [NEW]
    - Location: docker/production/configs/grafana/
    - Copy existing dashboard patterns if any
    - Dependencies: TimescaleDB schema
```

---

## 3. Interface Contracts (ACTUAL EXISTING + PLANNED)

### 3.1 TimeSeriesPoint (EXISTING)

**Current Definition (verify location):**
```rust
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}
```

**Used by (EXISTING):**
- ✅ MqttSource (produces)
- ✅ StorageWriter (consumes via channel)
- ✅ ParquetStore (persists)
- ✅ API handlers (queries)

**Preservation Requirements:**
- MUST maintain serialization format (Parquet compatibility)
- MUST NOT change field names
- CAN add optional fields with #[serde(default)]

### 3.2 ParquetStore (EXISTING)

**Current Interface (verify from code):**
```rust
pub struct ParquetStore {
    base_path: PathBuf,
    // ... internal fields
}

impl ParquetStore {
    pub fn new(base_path: &str) -> Result<Self, CoreError>;

    pub async fn write(&self, point: TimeSeriesPoint) -> Result<(), CoreError>;

    pub async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<(), CoreError>;

    pub async fn replay_wal(&self) -> Result<(), CoreError>;

    pub async fn query(
        &self,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: Option<HashMap<String, String>>,
    ) -> Result<Vec<TimeSeriesPoint>, CoreError>;
}
```

**Preservation Requirements:**
- MUST keep existing method signatures
- CAN add new methods for multi-stream
- MUST maintain WAL format
- MUST handle existing Parquet files

**Proposed Extensions:**
```rust
// NEW methods to add
impl ParquetStore {
    pub async fn write_batch_for_stream(
        &self,
        stream_id: &str,
        points: Vec<TimeSeriesPoint>
    ) -> Result<(), CoreError>;

    pub async fn query_stream(
        &self,
        stream_id: &str,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TimeSeriesPoint>, CoreError>;
}
```

### 3.3 ConfigClient (EXISTING)

**Current Interface:**
```rust
pub struct ConfigClient {
    client: Client,
    prefix: String,
}

impl ConfigClient {
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError>;

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError>;

    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError>;

    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, ConfigError>;

    pub async fn watch<F>(&self, prefix: &str, callback: F) -> Result<WatchHandle, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static;

    pub async fn get_with_env<T>(&self, key: &str, env_prefix: &str) -> Result<T, ConfigError>;
}
```

**Preservation Requirements:**
- MUST NOT change existing methods
- air-quality-app depends on this API

**Proposed Extensions:**
```rust
// NEW module: config-client/src/stream_registry.rs
pub struct StreamRegistry {
    client: ConfigClient, // wraps existing client
}

impl StreamRegistry {
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> {
        let client = ConfigClient::with_prefix(endpoints, "/streams").await?;
        Ok(Self { client })
    }

    pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;

    pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;

    pub async fn watch_streams(&self) -> Result<Receiver<StreamEvent>, ConfigError>;
}
```

### 3.4 StreamRecord (NEW - Wraps Existing)

**Definition:**
```rust
pub struct StreamRecord {
    pub stream_id: String,
    pub point: TimeSeriesPoint, // REUSE existing type
    pub metadata: Option<RecordMetadata>,
}

pub struct RecordMetadata {
    pub source_id: String,
    pub ingestion_time: DateTime<Utc>,
}

impl From<TimeSeriesPoint> for StreamRecord {
    fn from(point: TimeSeriesPoint) -> Self {
        Self {
            stream_id: "air-quality".to_string(), // default for backward compat
            point,
            metadata: None,
        }
    }
}
```

**Used by (NEW):**
- Ingestion Router (validates)
- Storage Layer Manager (persists)
- New source implementations

**Backward Compatibility:**
- Can convert TimeSeriesPoint → StreamRecord
- Allows gradual migration

### 3.5 MqttSource (EXISTING)

**Current Interface (verify):**
```rust
pub struct MqttSource {
    // internal fields
}

impl MqttSource {
    pub fn new(config: MqttConfig) -> Self;

    pub async fn start(&mut self) -> Result<(), CoreError>;

    pub async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;

    pub async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}
```

**Preservation Requirements:**
- MUST maintain current interface
- air-quality-app/mqtt_handler.rs depends on this

**Proposed Source Trait (NEW - MqttSource can optionally implement):**
```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn stream_id(&self) -> &str;
    async fn fetch(&self) -> Result<Vec<StreamRecord>, CoreError>;
    async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}

// Adapter to allow existing MqttSource to implement new trait
impl Source for MqttSourceAdapter {
    fn stream_id(&self) -> &str { &self.stream_id }

    async fn fetch(&self) -> Result<Vec<StreamRecord>, CoreError> {
        let points = self.inner.fetch().await?;
        Ok(points.into_iter().map(|p| p.into()).collect())
    }
}
```

### 3.6 TimescaleDB Adapter (NEW)

**Provides:**
```rust
pub struct TimescaleAdapter {
    pool: sqlx::PgPool,
}

impl TimescaleAdapter {
    pub async fn new(connection_string: &str) -> Result<Self, CoreError>;

    pub async fn ensure_table(&self, config: &StreamConfig) -> Result<(), CoreError>;

    pub async fn write_batch(
        &self,
        stream_id: &str,
        points: Vec<TimeSeriesPoint>, // REUSE existing type
    ) -> Result<(), CoreError>;

    pub async fn setup_compression(
        &self,
        stream_id: &str,
        after_days: u32,
    ) -> Result<(), CoreError>;
}
```

**Consumes:**
- TimeSeriesPoint (existing)
- StreamConfig (new)

**Writes to:**
- TimescaleDB hypertables

---

## 4. Preservation Matrix

| Component | Current State | Must Preserve | Can Extend | Risk Level |
|-----------|---------------|---------------|------------|------------|
| **TimeSeriesPoint** | ✅ Working | Field names, serialization | Add optional fields | MEDIUM |
| **ParquetStore** | ✅ Working | Method signatures, WAL | Add stream methods | MEDIUM |
| **MqttSource** | ✅ Working | fetch(), health_check() | Wrap in adapter | LOW |
| **ConfigClient** | ✅ Working | All methods | Add StreamRegistry wrapper | LOW |
| **MqttHandler** | ✅ Working | Pipeline logic | Nothing (leave as-is) | LOW |
| **StorageWriter** | ✅ Working | Batching logic | Copy patterns to manager | LOW |
| **API Routes** | ✅ Working | All endpoints | Add new endpoints | LOW |
| **Config hierarchy** | ✅ Working | etcd > env > yaml > defaults | Nothing | LOW |
| **WAL files** | ✅ Working | Format, replay logic | Nothing | HIGH |
| **Parquet files** | ✅ Working | Schema, partitioning | Add stream_id partition | HIGH |
| **Docker setup** | ✅ Working | Service definitions | Add TimescaleDB wiring | MEDIUM |

**Preservation Strategy:**
1. Copy patterns from existing components instead of modifying them
2. Create new files/modules rather than editing working code
3. Use adapter pattern to bridge old and new interfaces
4. Run integration tests before and after changes
5. Maintain backward compatibility in data formats

---

## 5. External Dependencies

### 5.1 Rust Crates Required

**Already in Use (verify from Cargo.toml):**
```toml
[dependencies]
# Confirmed existing
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"
rumqttc = "0.23"  # MQTT client
axum = "0.7"      # Web framework (in air-quality-app)
chrono = "0.4"
parquet = "50.0"  # Verify version
arrow = "50.0"    # Verify version
etcd-client = "0.12"  # In config-client

# NEW dependencies to add
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "time"] }
polars = { version = "0.36", features = ["parquet", "lazy"] }  # Optional for analytics
async-trait = "0.1"  # May already exist
```

### 5.2 Infrastructure Services (ACTUAL)

**Currently Running (Pi Deployment):**
```yaml
# Location: deploy/pi/docker-compose.yml
services:
  mosquitto:
    image: eclipse-mosquitto:2.0  ✅
    ports: ["1883:1883", "9001:9001"]
    status: Running and integrated
    used_by: MqttSource, air-quality-app MQTT ingestion
    location: deploy/pi/docker-compose.yml

  etcd:
    image: quay.io/coreos/etcd:v3.5.11  ✅
    ports: ["2379:2379"]
    status: Running and integrated
    used_by: config-client, air-quality-app config loading
    location: deploy/pi/docker-compose.yml
    volumes: pi_etcd-data

  air-quality-app:
    image: neural-data-platform/air-quality-app:latest  ✅
    ports: ["8080:8080", "9090:9090"]
    status: Running and working
    depends_on: [etcd, mosquitto]
    location: deploy/pi/docker-compose.yml
    volumes: pi_air-quality-data mounted at /app/data
    memory_limit: 896MB (Raspberry Pi constraint)
    build_time: 15-30 minutes (Rust ARM64 compilation)
```

**Needs Wiring:**
```yaml
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    ports: ["5432:5432"]
    status: Service defined but NOT connected to air-quality-app
    needs: Schema initialization, adapter implementation
    note: Separate deployment, not part of deploy/pi stack

  grafana:
    image: grafana/grafana:10.2.0
    ports: ["3000:3000"]
    status: May exist, needs TimescaleDB datasource
    needs: Dashboard definitions
    note: Separate deployment, not part of deploy/pi stack
```

### 5.3 Existing Test Coverage

**Confirmed Tests:**
```
apps/air-quality-app/tests/
  ✅ integration_test.rs - Full pipeline tests
  ✅ etcd_config_test.rs - Config loading tests
  ✅ server_test.rs - API endpoint tests
  ✅ config_hierarchy_test.rs - Config precedence tests

apps/air-quality-app/src/api/routes.rs
  ✅ 500+ lines of route tests
  ✅ Mock-based handler tests
  ✅ CORS and error handling tests

config-client/tests/
  ✅ integration_test.rs - etcd operations
```

**Test Strategy:**
1. Run existing tests as baseline
2. Add tests for new components
3. Integration tests for dual-write
4. DO NOT modify existing test fixtures

---

## 6. Risk Dependencies (REVISED)

### 6.1 High-Risk Components (EXISTING - Preserve Carefully)

**1. ParquetStore (CRITICAL PRESERVATION)**
```
Risk Level: CRITICAL
Dependents: air-quality-app, existing data pipeline
Impact: Data loss if WAL or schema changes break
Status: EXISTING and WORKING ✅

Components depending on ParquetStore:
├─ air-quality-app/StorageWriter (active production use)
├─ Existing Parquet files on disk
├─ WAL files for crash recovery
└─ API query handlers

Mitigation:
- ⚠️ DO NOT modify existing methods
- ✅ ADD new methods for multi-stream
- ✅ Test with existing Parquet files
- ✅ Verify WAL compatibility
- ✅ Run integration tests before deployment
```

**2. Config Loading Hierarchy (CRITICAL PRESERVATION)**
```
Risk Level: CRITICAL
Dependents: air-quality-app startup
Impact: Service won't start if config loading breaks
Status: EXISTING and WORKING ✅

Hierarchy (etcd > env > yaml > defaults):
├─ config_etcd.rs (etcd loading)
├─ config.rs (yaml + env)
└─ Default values

Mitigation:
- ⚠️ DO NOT change existing config struct fields
- ✅ Test all fallback scenarios
- ✅ Verify env overrides still work
- ✅ Keep backward compatibility with existing etcd keys
```

**3. MQTT Pipeline (CRITICAL PRESERVATION)**
```
Risk Level: CRITICAL
Dependents: Live sensor data ingestion
Impact: Data loss if MQTT connection breaks
Status: EXISTING and WORKING ✅

Pipeline:
MqttSource → MqttHandler → Channel → StorageWriter → ParquetStore

Mitigation:
- ⚠️ DO NOT modify mqtt_handler.rs unless necessary
- ✅ Keep existing channel capacity (1000)
- ✅ Preserve reconnect logic
- ✅ Test with real MQTT broker
```

### 6.2 Medium-Risk Components (NEW - Can Fail Gracefully)

**1. TimescaleDB Adapter (MEDIUM RISK)**
```
Risk Level: MEDIUM
Dependents: None yet (net new)
Impact: Silver layer unavailable, Bronze still works
Status: NEW ⚡

Mitigation:
- ✅ Implement as separate service initially
- ✅ DO NOT couple to existing pipeline
- ✅ Bronze layer continues working if TimescaleDB fails
- ✅ Add circuit breaker for dual-write failures
```

**2. Stream Registry (MEDIUM RISK)**
```
Risk Level: MEDIUM
Dependents: New Ingestion Coordinator only
Impact: Only affects new coordinator, not air-quality-app
Status: NEW (extends config-client) ⚡

Mitigation:
- ✅ Keep as wrapper around ConfigClient
- ✅ DO NOT modify config-client internals
- ✅ Fallback to default stream config if registry unavailable
```

### 6.3 Low-Risk Components (Additive Only)

**1. New Sources (HttpPoller, WebhookHandler)**
```
Risk Level: LOW
Dependents: None yet
Impact: None, purely additive
Status: NEW ⚡

Mitigation:
- ✅ Build independently
- ✅ Test in isolation
- ✅ No impact on existing MQTT source
```

**2. Grafana Dashboards**
```
Risk Level: LOW
Dependents: TimescaleDB data only
Impact: Visualization only, no data risk
Status: NEW ⚡
```

### 6.4 Dependency Risk Matrix (REVISED)

| Component | Status | # Dependencies | # Dependents | Risk Score | Build Priority | Must Preserve |
|-----------|--------|----------------|--------------|------------|----------------|---------------|
| **ParquetStore** | ✅ EXISTING | 0 | 5+ | CRITICAL | Verify 1st | File format, WAL, methods |
| **TimeSeriesPoint** | ✅ EXISTING | 0 | 10+ | CRITICAL | Verify 1st | Fields, serialization |
| **MqttSource** | ✅ EXISTING | 1 | 2 | HIGH | Verify 1st | fetch(), health_check() |
| **ConfigClient** | ✅ EXISTING | 1 | 3 | HIGH | Verify 1st | All methods |
| **Config Hierarchy** | ✅ EXISTING | 3 | 1 | HIGH | Verify 1st | Fallback logic |
| **MQTT Pipeline** | ✅ EXISTING | 4 | 1 | HIGH | Verify 1st | Channel flow |
| **API Routes** | ✅ EXISTING | 3 | 0 | MEDIUM | Verify 2nd | Endpoints, responses |
| **StreamRecord** | ⚡ NEW | 1 | 5+ | MEDIUM | Build 2nd | TimeSeriesPoint compat |
| **StreamRegistry** | ⚡ NEW (wrap) | 2 | 3 | MEDIUM | Build 3rd | ConfigClient delegation |
| **TimescaleDB Adapter** | ⚡ NEW | 2 | 1 | MEDIUM | Build 4th | None (new) |
| **Storage Manager** | ⚡ NEW | 4 | 1 | HIGH | Build 5th | Batching patterns |
| **Ingestion Coordinator** | ⚡ NEW | 8+ | 0 | HIGH | Build LAST | main.rs patterns |
| **HttpPoller** | ⚡ NEW | 1 | 0 | LOW | Parallel | None (new) |
| **WebhookHandler** | ⚡ NEW | 2 | 0 | LOW | Parallel | None (new) |
| **Grafana Dashboards** | ⚡ NEW | 1 | 0 | LOW | Post-MVP | None (new) |

**Legend:**
- ✅ EXISTING - Working component, must preserve
- ⚡ NEW - New component, no preservation needed
- (wrap) - Wraps existing component without modifying it

---

## 7. Integration Points (EXISTING + NEW)

### 7.1 Current Data Flow (WORKING ✅)

```
AirGradient Sensor → MQTT Broker → MqttSource → MqttHandler
                                                      ↓
                                               Channel (1000 cap)
                                                      ↓
                                               StorageWriter
                                                      ↓
                                               ParquetStore
                                                      ↓
                                           data/parquet/*.parquet
                                                      ↓
                                          API Handlers (queries)
```

**Preservation:** Keep this pipeline working while adding new paths

### 7.2 Future Multi-Stream Flow (NEW)

```
Multiple Sources → Source Manager → Ingestion Router
                                           ↓
                                  Storage Layer Manager
                                  ↓                  ↓
                          ParquetStore    TimescaleDB Adapter
                          (Bronze)            (Silver)
```

**Strategy:** Run in parallel with existing pipeline initially

### 7.3 Configuration Flow (EXISTING + EXTENDED)

**Current (WORKING ✅):**
```
etcd → ConfigClient.get() → air-quality-app config
  ↓ fallback
config.yaml → AppConfig::from_yaml()
  ↓ fallback
Environment variables → apply_env_overrides()
  ↓ fallback
Default values → AppConfig::default_config()
```

**Extended (NEW):**
```
etcd /streams/* → StreamRegistry.load_stream() → StreamConfig
                                                      ↓
                                          Ingestion Coordinator
```

**Preservation:** DO NOT modify existing config loading in air-quality-app

---

## 8. Testing Strategy by Dependency

### 8.1 Baseline Tests (Run BEFORE changes)

**Tier 0: Existing Component Verification**
```
1. Run all existing integration tests
   - apps/air-quality-app/tests/integration_test.rs
   - apps/air-quality-app/tests/etcd_config_test.rs
   - apps/air-quality-app/tests/server_test.rs
   - config-client/tests/integration_test.rs

2. Verify current pipeline manually
   - Start MQTT broker
   - Publish test message
   - Verify Parquet file written
   - Query via API

3. Document baseline metrics
   - Ingestion rate
   - Batch write latency
   - API response times
   - Memory usage
```

### 8.2 Unit Test Priorities (NEW components)

**Tier 1: Foundation (Test First)**
```
1. StreamRecord serialization/deserialization
2. StreamConfig parsing from etcd format
3. SchemaField validation logic
4. TimeSeriesPoint → StreamRecord conversion
```

**Tier 2: Implementations**
```
5. TimescaleDB DDL generation (unit test, no DB)
6. Ingestion Router validation rules (mocked)
7. HttpPoller with mock HTTP server
8. WebhookHandler with test Axum server
```

**Tier 3: Integration (NEW with EXISTING)**
```
9. StreamRegistry wrapping ConfigClient
10. ParquetStore multi-stream writes (verify existing files untouched)
11. Storage Manager dual-write (verify Bronze still works if Silver fails)
12. End-to-end coordinator with all sources
13. Pi deployment test: build time, memory constraints, ARM64 compatibility
```

### 8.3 Regression Tests (CRITICAL)

**After Each Change:**
```
1. Re-run all existing integration tests
2. Verify air-quality-app still starts
3. Verify config loading hierarchy
4. Verify MQTT ingestion still works
5. Verify existing Parquet files are readable
6. Verify API endpoints return same data
7. Check WAL replay still works
```

### 8.4 Integration Test Dependencies

**Test Environment Requirements:**
```yaml
services:
  etcd:
    required_for: [ConfigClient tests, StreamRegistry tests, Coordinator tests]
    status: ✅ Already running in deploy/pi/docker-compose.yml

  timescaledb:
    required_for: [Adapter tests, DDL tests, Dual-write tests]
    status: ⚠️ Separate deployment, not on Pi - need dev/staging instance

  mqtt_broker:
    required_for: [MqttSource tests, E2E tests]
    status: ✅ Already running (Mosquitto) in deploy/pi/

  mock_http_server:
    required_for: [HttpPoller tests]
    status: ⚡ NEW - use wiremock or similar

  pi_constraints:
    required_for: [Integration tests, deployment tests]
    status: ⚠️ Test on ARM64 architecture, 896MB memory limit
```

---

## 9. Deployment Dependencies

### 9.1 Current Deployment (WORKING ✅)

**Pi Deployment (deploy/pi/)**
```
1. Infrastructure Layer (RUNNING):
   ✅ etcd v3.5.11
      - Single-node deployment
      - Port: 2379
      - Volume: pi_etcd-data
   ✅ MQTT broker (Mosquitto 2.0)
      - Ports: 1883 (MQTT), 9001 (WebSocket)

2. Application Layer (RUNNING):
   ✅ air-quality-app
      - Build: deploy/pi/deploy.sh build (15-30 min on ARM64)
      - Depends on etcd + mosquitto
      - Volume: pi_air-quality-data mounted at /app/data
      - Ports: 8080 (API), 9090 (metrics)
      - Memory limit: 896MB
      - Parquet storage: /app/data/parquet/

3. Deployment Commands:
   ✅ ./deploy/pi/deploy.sh build  - Build images
   ✅ ./deploy/pi/deploy.sh start  - Start services
   ✅ ./deploy/pi/deploy.sh sync   - Sync etcd config
   ✅ ./deploy/pi/deploy.sh status - Check status

4. Health Checks (WORKING):
   ✅ /health endpoint
   ✅ etcd health check
   ✅ MQTT health check via mosquitto_sub
```

### 9.2 Extended Deployment (NEW)

```
1. Add to Infrastructure Layer:
   ⚡ TimescaleDB instance
   ⚡ Initialize schema with init.sql
   ⚡ Configure retention and compression policies

2. Add to Application Layer:
   ⚡ ingestion-coordinator
      - NEW Dockerfile (copy air-quality-app pattern)
      - Depends on etcd + mosquitto + timescaledb
      - Does NOT replace air-quality-app

3. Add to Monitoring:
   ⚡ Grafana dashboards
   ⚡ TimescaleDB datasource
   ⚡ Dual-write lag alerts
```

### 9.3 Deployment Order (SAFE)

**Pi Deployment:**
```
1. Deploy TimescaleDB (separate stack, not Pi):
   - NOT part of deploy/pi stack
   - Separate deployment for dev/staging
   - Run init.sql for schema
   - Test connection

2. Current Pi Deployment (WORKING):
   - cd deploy/pi
   - ./deploy.sh build    # 15-30 min for Rust ARM64 compilation
   - ./deploy.sh start    # Start mosquitto, etcd, air-quality-app
   - ./deploy.sh sync     # Sync config to etcd
   - ./deploy.sh status   # Verify all services running

3. Deploy Ingestion Coordinator (NEW, parallel):
   - Add to deploy/pi/docker-compose.yml or separate stack
   - Build new Docker image (similar ARM64 build time)
   - Deploy alongside air-quality-app (NOT replacing it)
   - Verify both services running

4. Gradual Migration:
   - Keep air-quality-app handling AirGradient sensors
   - Use ingestion-coordinator for new streams
   - Monitor dual-write lag
   - Monitor memory usage (Pi has 896MB allocated)

5. Cutover (AFTER validation):
   - Redirect AirGradient to ingestion-coordinator
   - Keep air-quality-app as backup
   - Monitor for 24-48 hours before decommission
```

### 9.4 Rollback Dependencies (SAFE)

**Rollback Strategy (Pi Deployment):**
```
Bronze Layer (SAFE):
  - Parquet files at /app/data/parquet/ are immutable
  - Volume: pi_air-quality-data persists data
  - Existing files remain readable
  - WAL can be replayed
  - No schema changes needed

Silver Layer (ISOLATED):
  - TimescaleDB is separate deployment, not on Pi
  - No existing data to lose
  - Can rebuild from Bronze if needed

Application (PARALLEL):
  - air-quality-app keeps running
  - ingestion-coordinator can be stopped
  - No downtime required
  - Rollback: ./deploy/pi/deploy.sh stop && ./deploy.sh start

Config (BACKWARD COMPATIBLE):
  - Old config keys still work
  - New stream configs are additive
  - Can remove /streams/* from etcd safely
  - etcd data persisted in pi_etcd-data volume

Pi Volumes (PERSISTENT):
  - pi_air-quality-data: /app/data (Parquet files)
  - pi_etcd-data: etcd configuration
  - Both survive container restarts/rebuilds
```

---

## 10. Summary (REVISED)

### Existing Assets (PRESERVE CAREFULLY)

**Working Components (~3,500 LOC):**
- ✅ apps/air-quality-app (2,813 lines) - Full MQTT ingestion pipeline
- ✅ config-client (260 lines) - etcd integration with watch support
- ✅ neural-core ParquetStore - Bronze layer storage with WAL
- ✅ neural-core MqttSource - MQTT client abstraction
- ✅ Docker Compose setup - etcd, Mosquitto, Prometheus running

**Working Infrastructure:**
- ✅ etcd v3.5.11 (config storage + watch)
- ✅ Mosquitto 2.0 (MQTT broker)
- ✅ Parquet files (data/parquet/) with WAL
- ✅ REST API (6+ endpoints, 500+ lines of tests)
- ✅ Config hierarchy (etcd > env > yaml > defaults)

**Test Coverage:**
- ✅ Integration tests (4+ test files)
- ✅ API route tests (500+ lines)
- ✅ Config loading tests
- ✅ MQTT pipeline tests

### New Development Required (~2,000-2,500 LOC)

**Truly New Components:**
1. StreamRecord type + metadata (~150 lines)
2. StreamConfig type + schema (~200 lines)
3. StreamRegistry (wraps ConfigClient) (~150 lines)
4. TimescaleDB Adapter (~400 lines)
5. DDL Generator (~200 lines)
6. Ingestion Router (~300 lines)
7. Storage Layer Manager (~350 lines)
8. Source Manager (~250 lines)
9. HttpPoller Source (~200 lines)
10. WebhookHandler Source (~250 lines)
11. Ingestion Coordinator main.rs (~200 lines, copy from air-quality-app)
12. Tests for all above (~500 lines)

**Extensions to Existing:**
1. ParquetStore multi-stream methods (~150 lines)
2. MqttSource adapter (if needed) (~100 lines)
3. Grafana dashboards (~100 lines config)

### Critical Path (REVISED)

**Total Estimated Timeline:** 15-20 days (REDUCED from 18-23 days)

**Phase 0: Verification (1 day)**
- Document existing interfaces
- Run baseline tests
- Establish metrics

**Phase 1: Foundation (2-3 days)**
- StreamRecord, StreamConfig types
- StreamRegistry (wrap ConfigClient)

**Phase 2: Storage (3-4 days)**
- Extend ParquetStore carefully
- Build TimescaleDB Adapter
- DDL Generator

**Phase 3: Sources (3-4 days)**
- Verify MqttSource
- Build HttpPoller, WebhookHandler
- Test in isolation

**Phase 4: Coordination (4-5 days)**
- Ingestion Router
- Storage Layer Manager
- Source Manager
- Ingestion Coordinator (copy patterns from air-quality-app)

**Phase 5: Deployment (2-3 days)**
- Wire up TimescaleDB
- Deploy coordinator alongside air-quality-app
- Grafana dashboards

### Highest Risk Items (PRESERVATION FOCUSED)

**1. ParquetStore Extension (MEDIUM-HIGH RISK)**
- Risk: Breaking existing WAL or file format
- Mitigation: Add new methods, don't modify existing ones
- Testing: Verify existing files remain readable

**2. Config Loading Hierarchy (MEDIUM RISK)**
- Risk: Breaking air-quality-app startup
- Mitigation: Don't touch config.rs or config_etcd.rs
- Testing: Verify all fallback scenarios

**3. TimeSeriesPoint Changes (MEDIUM RISK)**
- Risk: Parquet schema incompatibility
- Mitigation: Only add optional fields with #[serde(default)]
- Testing: Deserialize existing Parquet files

**4. Dual-Write Coordination (MEDIUM RISK)**
- Risk: Data loss if Bronze write fails
- Mitigation: Bronze write first, Silver write is best-effort
- Testing: Test Bronze-only fallback mode

### Success Criteria

**Must Have:**
- ✅ air-quality-app continues working unchanged
- ✅ Existing Parquet files remain readable
- ✅ All existing tests still pass
- ✅ Config loading hierarchy preserved
- ✅ No data loss during migration
- ✅ Dual-write to Bronze + Silver operational
- ✅ Multi-stream support for 3+ streams

**Nice to Have:**
- ⚡ Grafana dashboards
- ⚡ HttpPoller and WebhookHandler sources
- ⚡ Predictive models using Silver layer
- ⚡ Alert rules in Grafana

**Stretch Goals:**
- ⚡ Schema evolution support
- ⚡ Backfill Silver layer from Bronze
- ⚡ Plugin architecture for sources

---

*Last Updated: 2025-12-15 (REVISED with existing component inventory)*
*Next Review: After Phase 0 verification complete*

**KEY CHANGES FROM ORIGINAL:**
1. ✅ Added "Existing Component Inventory" section showing ~3,500 LOC already working
2. ✅ Marked components as [EXISTING], [EXTEND], [PARTIAL], or [NEW]
3. ✅ Added "Preserve" column to all matrices
4. ✅ Updated build order to start with verification phase
5. ✅ Documented actual interfaces from existing code
6. ✅ Reduced timeline estimate (less greenfield work)
7. ✅ Added preservation requirements and backward compatibility notes
8. ✅ Updated risk assessment to focus on preservation risks
9. ✅ Changed deployment strategy to parallel deployment (not replacement)
10. ✅ Added rollback safety notes based on existing architecture
