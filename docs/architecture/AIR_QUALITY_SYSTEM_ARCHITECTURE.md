# Air Quality System Architecture Overview

**Document Version:** 1.0
**Date:** 2025-12-14
**Status:** Current Implementation Analysis
**Author:** System Architect

## Executive Summary

The Neural Data Platform is a multi-domain time-series data processing system built on Rust, featuring a modular workspace architecture. The air-quality domain demonstrates the platform's capabilities for IoT sensor data ingestion, real-time processing, and storage.

### Key Characteristics
- **Architecture Pattern**: Modular monolith with domain-driven design
- **Technology Stack**: Rust, Tokio async runtime, MQTT, Parquet, Polars
- **Current State**: Air quality domain operational with MQTT-to-Parquet pipeline
- **Scalability**: Designed for horizontal scaling with partition-based storage

---

## 1. System Context (C4 Level 1)

### External Actors and Systems

1. **AirGradient ONE Sensors**
   - IoT devices measuring air quality metrics
   - Communication: MQTT protocol
   - Data frequency: Real-time (typically every 9 seconds)
   - Metrics: PM2.5, CO2, temperature, humidity, VOC, WiFi signal

2. **MQTT Broker**
   - Message broker (external or embedded)
   - Protocol: MQTT v3.1.1
   - QoS: At-least-once delivery (QoS 1)
   - Topic pattern: `airgradient/readings/{SERIAL_NUMBER}`

3. **API Consumers**
   - Web applications, dashboards
   - Mobile applications
   - Third-party integrations
   - Protocol: REST API (Axum framework)

4. **Monitoring Systems**
   - Prometheus metrics scraping
   - Grafana dashboards
   - Alert managers

### System Boundaries

The Neural Data Platform consists of:
- **Core Platform Services**: Shared infrastructure (config-store, neural-core)
- **Domain Modules**: Domain-specific logic (domains/air-quality)
- **Applications**: Deployable binaries (apps/air-quality-app)
- **Storage Layer**: ParquetStore with WAL (Write-Ahead Log)

---

## 2. Container Architecture (C4 Level 2)

### Workspace Structure

```
neural-data-platform/
├── core/                    # Platform-core library
├── config-store/            # Configuration management service
├── neural-core/             # Shared neural/ML capabilities
├── neural-trading/          # Trading domain (separate)
├── neural-ml-ops/           # ML operations (separate)
├── data-staging/            # Data transformation layer
├── domains/
│   └── air-quality/         # Air quality domain models
└── apps/
    └── air-quality-app/     # Air quality REST API + MCP server
```

### Container Responsibilities

#### 1. Platform Core (`core/`)
- **Purpose**: Foundational abstractions and traits
- **Key Exports**:
  - `Source` trait: Data ingestion abstraction
  - `Store` trait: Storage abstraction
  - `Forecast` trait: Prediction abstraction
  - `TimeSeriesPoint`: Generic time-series data model

#### 2. Neural Core (`neural-core/`)
- **Purpose**: Shared neural network and ML infrastructure
- **Components**:
  - MQTT source implementation
  - HTTP polling source
  - Parquet storage implementation
  - Feature engineering utilities
  - Model abstractions

#### 3. Air Quality Domain (`domains/air-quality/`)
- **Purpose**: Business logic for air quality data
- **Components**:
  - `AirQualityReading`: 29-field domain model
  - Parser: MQTT and HTTP API payload parsing
  - Validator: Data quality validation
  - Adapter: Platform integration layer

#### 4. Air Quality Application (`apps/air-quality-app/`)
- **Purpose**: Deployable REST API and MCP server
- **Binaries**:
  - `air-quality-server`: REST API (port 8000)
  - `air-quality-mcp`: MCP (Model Context Protocol) server
- **Components**:
  - Ingestion pipeline: MQTT handler
  - Storage pipeline: Batching writer
  - API routes: Health, readings, locations, alerts, forecast
  - MCP tools: Air quality data access for AI assistants

#### 5. Config Store (`config-store/`)
- **Purpose**: Centralized configuration management
- **Technology**: gRPC service with Redis backend
- **Status**: Available but not yet integrated with air-quality

---

## 3. Component Architecture (C4 Level 3)

### Air Quality Ingestion Pipeline

#### MQTT Ingestion Flow

```
AirGradient Sensor
    ↓ (MQTT Publish)
MQTT Broker
    ↓ (MQTT Subscribe)
MqttSource (neural_core)
    ↓ (Parsed TimeSeriesPoint)
MqttHandler (air-quality-app)
    ↓ (mpsc::channel)
StorageWriter (air-quality-app)
    ↓ (Batch write)
ParquetStore (neural_core)
    ↓ (Partitioned files)
Disk Storage (data/{location}/{year}/{month}/{day}/readings.parquet)
```

#### Component Details

**MqttSource** (`core/src/sources/mqtt.rs`)
- Manages MQTT connection lifecycle
- Auto-reconnect with exponential backoff (1s → 30s max)
- Parses AirGradient JSON payloads
- Converts to multiple `TimeSeriesPoint` (one per metric)
- Buffered queue for backpressure handling
- Health check support

**MqttHandler** (`apps/air-quality-app/src/ingestion/mqtt_handler.rs`)
- Wraps MqttSource for application use
- Fetches points at 100ms intervals
- Forwards to channel for async processing
- Error resilience: continues on fetch failures

**StorageWriter** (`apps/air-quality-app/src/pipeline/storage_writer.rs`)
- Receives points from mpsc channel
- Batching strategy:
  - Batch size: 100 points (configurable)
  - Timeout: 5 seconds (configurable)
  - Flush on either condition
- Graceful shutdown on channel close
- Flushes remaining buffer on exit

**ParquetStore** (`core/src/storage/parquet.rs`)
- Partition strategy: `{location_id}/year={yyyy}/month={mm}/day={dd}/readings.parquet`
- Write-Ahead Log (WAL) for crash recovery
- Columnar storage with Snappy compression
- Query capabilities: time-range, aggregations (mean, min, max, median, percentiles)
- Atomic operations via WAL commit

### REST API Architecture

**Router Structure** (`apps/air-quality-app/src/api/routes.rs`)
```
/health               → Health check (source, store, forecast status)
/api/v1/readings      → Query air quality readings
  GET ?location_id=X&start=T1&end=T2
/api/v1/locations     → Manage sensor locations
  GET, POST, PUT, DELETE
/api/v1/alerts        → Air quality alerts
  GET, POST /configure
/api/v1/forecast      → Prediction endpoints
  GET /predict/:location_id
  POST /train
```

**Handler Dependencies**
- `store: Arc<ParquetStore>`: Data storage
- `source: Arc<dyn Source>`: Currently mock (MQTT runs separately)
- `forecast: Arc<dyn Forecast>`: Currently mock (future ML integration)
- `alert_store: Arc<AlertStore>`: In-memory alert management
- `location_store: Arc<LocationStore>`: In-memory location registry

### MCP Server Architecture

**MCP Tools** (`apps/air-quality-app/src/mcp/tools.rs`)
- `get_air_quality_reading`: Fetch latest reading for location
- `query_air_quality_history`: Time-range queries
- `list_air_quality_locations`: Available sensors
- `get_air_quality_statistics`: Aggregated metrics
- Purpose: AI assistant integration (Claude, etc.)

---

## 4. Data Flow Architecture

### Ingestion Data Flow

```
┌─────────────────────┐
│ AirGradient Sensor  │
│ (IoT Device)        │
└──────────┬──────────┘
           │ MQTT Publish (JSON)
           │ Topic: airgradient/readings/{serial}
           ↓
┌─────────────────────┐
│   MQTT Broker       │
│ (mosquitto/EMQ)     │
└──────────┬──────────┘
           │ MQTT Subscribe
           │ QoS: 1 (at-least-once)
           ↓
┌─────────────────────────────────┐
│  MqttSource (neural_core)       │
│  - Parse JSON payload           │
│  - Create TimeSeriesPoint       │
│  - Cache in memory buffer       │
└──────────┬──────────────────────┘
           │ fetch() call (100ms interval)
           ↓
┌─────────────────────────────────┐
│  MqttHandler (air-quality-app)  │
│  - Retrieve cached points       │
│  - Send to channel              │
└──────────┬──────────────────────┘
           │ mpsc::channel (capacity: 1000)
           ↓
┌─────────────────────────────────┐
│  StorageWriter                  │
│  - Accumulate batch (100 pts)  │
│  - Timeout trigger (5s)         │
└──────────┬──────────────────────┘
           │ write_batch()
           ↓
┌─────────────────────────────────┐
│  WriteAheadLog (WAL)            │
│  - Append entries               │
│  - Fsync for durability         │
└──────────┬──────────────────────┘
           │ After WAL commit
           ↓
┌─────────────────────────────────┐
│  ParquetStore                   │
│  - Partition by location/date   │
│  - Columnar write               │
│  - Snappy compression           │
└──────────┬──────────────────────┘
           │
           ↓
┌─────────────────────────────────┐
│  Filesystem                     │
│  data/ABC123/year=2025/         │
│      month=12/day=14/           │
│      readings.parquet           │
└─────────────────────────────────┘
```

### Query Data Flow

```
REST API Request
    ↓
GET /api/v1/readings?location_id=ABC123&start=2025-12-14T00:00:00Z&end=2025-12-14T23:59:59Z
    ↓
ReadingsHandler
    ↓
ParquetStore::query()
    ↓
Scan partitions (year=2025/month=12/day=14/)
    ↓
Read Parquet files via Polars
    ↓
Filter by timestamp range
    ↓
Convert to TimeSeriesPoint
    ↓
JSON response with ApiResponse wrapper
```

---

## 5. Technology Stack

### Core Technologies

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Language | Rust | 2021 Edition | Type safety, performance, concurrency |
| Async Runtime | Tokio | 1.40+ | Async I/O, task scheduling |
| MQTT Client | rumqttc | 0.24 | MQTT protocol implementation |
| Storage Format | Parquet | via Polars 0.35 | Columnar storage, compression |
| DataFrames | Polars | 0.35 | Fast query processing |
| Web Framework | Axum | 0.7 | REST API endpoints |
| Serialization | Serde | 1.0 | JSON/YAML parsing |
| Error Handling | thiserror, anyhow | 1.0 | Structured error types |
| Logging | tracing | 0.1 | Structured logging |

### Workspace Dependencies

Managed in root `Cargo.toml` with workspace inheritance:
```toml
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
polars = { version = "0.35", features = ["parquet", "lazy"] }
rumqttc = "0.24"
```

---

## 6. Design Patterns

### 1. Trait-Based Abstraction
- `Source`, `Store`, `Forecast` traits in platform-core
- Enables polymorphism and testing (mockall)
- Clean dependency injection

### 2. Domain-Driven Design
- Domains separated from infrastructure
- `air-quality` domain owns business logic
- Platform-core provides shared abstractions

### 3. Actor-Like Concurrency
- MQTT handler as background task
- Storage writer as background task
- Communication via mpsc channels
- Graceful shutdown coordination

### 4. Write-Ahead Logging
- Crash recovery mechanism
- Atomic batch writes
- Replay on startup

### 5. Partition Pruning
- Date-based partitioning
- Reduces query scan scope
- Scales with data volume

### 6. Batching Pattern
- Accumulate points before write
- Reduce I/O operations
- Configurable batch size and timeout

---

## 7. Data Models

### TimeSeriesPoint (Generic)
```rust
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}
```

### AirQualityReading (Domain-Specific)
```rust
pub struct AirQualityReading {
    pub device: DeviceMetadata,      // serial_no, firmware, model
    pub particles: ParticleData,     // pm01, pm02, pm10, pm003_count
    pub gases: GasData,              // rco2, tvoc, nox_index
    pub environmental: EnvironmentalData, // temperature, humidity
    pub quality: QualityMetrics,     // aqi_pm, aqi_co2, aqi_voc
    pub timestamp: DateTime<Utc>,
}
```

### Conversion Strategy
AirGradient → Multiple TimeSeriesPoints (one per metric)
- `metric` tag identifies the metric type (pm02, co2, temperature, etc.)
- `source` tag identifies ingestion source (mqtt, http)
- Enables flexible querying and aggregation

---

## 8. Integration Points

### Current Integrations

1. **MQTT Broker**
   - Connection: TCP socket to broker
   - Auto-reconnect on failure
   - Configurable via `config.yaml`

2. **Filesystem Storage**
   - Base path: Configurable (default: `./data`)
   - Partition strategy: Hive-style partitioning
   - WAL path: `{base_path}/wal.log`

3. **REST API Clients**
   - JSON request/response
   - CORS enabled
   - Request ID tracing

### Future Integrations (Not Yet Implemented)

1. **Config Store (gRPC)**
   - Centralized configuration
   - Hot-reload capabilities
   - Schema validation

2. **Neural ML Ops**
   - Model training pipeline
   - Feature computation
   - Prediction serving

3. **Data Staging**
   - Proto transformation
   - Event bus integration
   - Cross-domain data sharing

4. **Forecast Module**
   - FANN neural network
   - Time-series forecasting
   - Model persistence

---

## 9. Scalability Considerations

### Current Architecture

**Vertical Scaling**
- Single process handles all ingestion
- ParquetStore thread-safe with Arc<Mutex<>>
- Bounded channels prevent memory exhaustion

**Partition-Based Scaling**
- Date partitioning enables parallel reads
- Partition pruning reduces query cost
- Storage grows linearly with data retention

### Limitations

1. **Single Ingestion Process**
   - MqttSource is single-threaded per broker
   - Cannot scale beyond single broker throughput
   - Mitigated by high MQTT throughput (~10k msg/s)

2. **Parquet Write Contention**
   - Appending to existing files requires read-modify-write
   - Could become bottleneck with high write rates
   - Mitigation: Increase batch size, use time-based partitions

3. **No Query Layer**
   - Direct Parquet reads via Polars
   - No query optimizer or caching
   - Future: Consider TimescaleDB or Databend for analytics

### Horizontal Scaling Paths

1. **Multi-Broker Sharding**
   - Run multiple MqttHandler instances
   - Each subscribes to different broker/topics
   - Separate storage paths per handler

2. **Storage Layer Separation**
   - Move ParquetStore to dedicated service
   - Use object storage (S3, MinIO)
   - Enables distributed queries

3. **API Layer Scaling**
   - Load balancer + multiple API instances
   - Shared storage backend
   - Stateless API design enables easy scaling

---

## 10. Technical Debt and Improvement Areas

### Current Technical Debt

1. **Mock Services in Production**
   - `Source` trait is mocked in API server
   - `Forecast` trait is mocked
   - Reason: MQTT handler runs separately from API server
   - Impact: Health check doesn't reflect MQTT status

2. **Tight Coupling in main.rs**
   - Service creation logic in main function
   - Hard to test in isolation
   - Should use builder pattern or DI container

3. **No Graceful Degradation**
   - MQTT failure → degraded mode (no ingestion)
   - No fallback to HTTP polling
   - Alert mechanisms not implemented

4. **Limited Error Recovery**
   - WAL replay on startup only
   - No periodic consistency checks
   - No repair mechanisms for corrupted Parquet files

5. **Configuration Management**
   - File-based config only (config.yaml)
   - No remote configuration
   - Config-store not integrated

### Recommended Improvements

1. **Unified Service Architecture**
   - Merge MQTT handler into API server lifecycle
   - Share health status between components
   - Use shared task spawning

2. **Multi-Source Support**
   - Implement ReadingMerger from neural-core
   - Combine MQTT + HTTP polling
   - Deduplication strategy

3. **Observability Enhancements**
   - Prometheus metrics integration
   - Structured logging with correlation IDs
   - Distributed tracing (OpenTelemetry)

4. **Storage Optimization**
   - Implement compaction for Parquet files
   - Archive old partitions to cold storage
   - Add query cache layer

5. **Testing Infrastructure**
   - Integration tests with test MQTT broker
   - Property-based tests for parsers
   - Chaos engineering for failure scenarios

---

## 11. Deployment Architecture

### Current Deployment Model

**Single Binary Deployment**
```
air-quality-server
├── Ingestion Pipeline (background task)
│   ├── MqttHandler
│   └── StorageWriter
├── REST API Server (main task)
│   └── Axum HTTP server (port 8000)
└── Parquet Storage
    └── ./data/{location}/{partition}/readings.parquet
```

**Configuration**
- `config.yaml`: Application config (server, MQTT, storage)
- Environment variables: Runtime overrides
- Command-line args: Not currently used

**Dependencies**
- MQTT broker (external): mosquitto, EMQ, HiveMQ
- Filesystem: Local or network-mounted storage

### Production Deployment Considerations

1. **Process Management**
   - Use systemd service for supervision
   - Auto-restart on failure
   - Resource limits (memory, CPU)

2. **Storage**
   - Dedicated partition for data directory
   - Backup strategy for WAL + Parquet files
   - Retention policy implementation

3. **Monitoring**
   - Health check endpoint: `/health`
   - Metrics export: Prometheus-compatible
   - Log aggregation: JSON structured logs

4. **Security**
   - MQTT TLS encryption
   - API authentication (future)
   - File system permissions

---

## 12. Architecture Decision Records

### ADR-001: Trait-Based Abstraction for Data Sources
**Status**: Accepted
**Context**: Need flexible data ingestion from multiple sources (MQTT, HTTP, etc.)
**Decision**: Define `Source` trait in platform-core, implement in neural-core
**Consequences**:
- Positive: Easy to add new sources, testable with mocks
- Negative: Trait object overhead, runtime polymorphism

### ADR-002: Parquet for Time-Series Storage
**Status**: Accepted
**Context**: Need efficient storage for high-volume sensor data
**Decision**: Use Parquet with Polars for columnar storage
**Consequences**:
- Positive: Excellent compression, fast analytics, schema evolution
- Negative: Write amplification on updates, no built-in indexing

### ADR-003: Channel-Based Pipeline Architecture
**Status**: Accepted
**Context**: Decouple ingestion from storage for backpressure handling
**Decision**: Use tokio mpsc channels between MqttHandler and StorageWriter
**Consequences**:
- Positive: Async backpressure, graceful shutdown, testable components
- Negative: Bounded queue can drop messages if full (mitigated with capacity 1000)

### ADR-004: Workspace-Only Cargo Configuration
**Status**: Accepted
**Context**: Simplify dependency management across 8+ modules
**Decision**: Pure workspace with shared dependencies in root Cargo.toml
**Consequences**:
- Positive: Consistent versions, faster builds, easier upgrades
- Negative: All modules share same version of dependencies

### ADR-005: Domain Module Separation
**Status**: Accepted
**Context**: Support multiple data domains (air-quality, trading, etc.)
**Decision**: Separate domain models (domains/air-quality) from applications (apps/air-quality-app)
**Consequences**:
- Positive: Domain logic reusable, clear boundaries, independent evolution
- Negative: Additional indirection, requires careful interface design

---

## 13. Future Architecture Evolution

### Phase 1: Current State (Implemented)
- MQTT ingestion pipeline
- Parquet storage with WAL
- REST API for queries
- Basic health monitoring

### Phase 2: Observability and Resilience (Next 4-6 weeks)
- Prometheus metrics integration
- Grafana dashboards
- Multi-source ingestion (MQTT + HTTP)
- Alert system implementation
- Config-store integration

### Phase 3: ML Integration (8-12 weeks)
- Forecast module activation
- Feature engineering pipeline
- Model training automation
- Real-time predictions

### Phase 4: Multi-Domain Platform (12-16 weeks)
- Generic domain onboarding framework
- Cross-domain data sharing
- Event bus integration
- Domain registry service

### Phase 5: Production Hardening (16-20 weeks)
- Distributed deployment support
- Object storage backend (S3/MinIO)
- Query optimization layer
- Multi-tenancy support

---

## 14. Conclusion

The Neural Data Platform's air-quality domain demonstrates a well-architected system with:

**Strengths**:
1. Clean separation of concerns (domain, application, infrastructure)
2. Robust ingestion pipeline with fault tolerance
3. Efficient columnar storage with partition pruning
4. Trait-based abstractions for extensibility
5. Async-first design for high concurrency

**Challenges**:
1. Mock services in production code paths
2. Limited observability (metrics, tracing)
3. No distributed deployment support yet
4. Single-process scalability constraints

**Recommended Next Steps**:
1. Integrate real forecast module
2. Add Prometheus metrics
3. Implement multi-source ingestion
4. Deploy monitoring stack (Grafana)
5. Document operational runbooks

The architecture provides a solid foundation for a multi-domain time-series platform, with clear paths for evolution toward production-grade deployment.

---

## Appendix A: Key File References

**Domain Models**:
- `/workspaces/neural-data-platform/domains/air-quality/src/types.rs`
- `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs`

**Ingestion Pipeline**:
- `/workspaces/neural-data-platform/core/src/sources/mqtt.rs`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/ingestion/mqtt_handler.rs`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/storage_writer.rs`

**Storage Layer**:
- `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
- `/workspaces/neural-data-platform/core/src/storage/wal.rs`

**API Layer**:
- `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs`

**Configuration**:
- `/workspaces/neural-data-platform/Cargo.toml` (workspace)
- `/workspaces/neural-data-platform/apps/air-quality-app/config.yaml`
