# ADR-001: Multi-Stream Architecture Foundation

**Status**: Approved
**Date**: 2025-12-15
**Decision Makers**: Architecture Team
**Context**: AIR-004 Multi-Stream Data Platform
**Supersedes**: Single-stream architecture from AIR-001/002/003

---

## Context and Problem Statement

The neural-data-platform currently supports a single data stream (air quality from AirGradient sensors) with a hardcoded pipeline: MQTT → Parquet → API. To enable predictive analytics correlating multiple data types (air quality, home events, weather, etc.), we need to evolve to a generic multi-stream platform while preserving existing functionality.

### Business Requirements

1. Support 3-5 concurrent streams initially (air quality, home events, weather)
2. Enable cross-stream temporal correlation for predictive models
3. Provide real-time dashboards with multi-stream visualization
4. Maintain backward compatibility with existing air-quality-app
5. Deploy on Raspberry Pi 5 (memory constraint: < 1.5GB)

### Technical Requirements

1. Dynamic stream registration without code changes
2. Per-stream schema validation
3. Multiple source types (MQTT, HTTP polling, webhooks)
4. Both archival (Bronze/Parquet) and queryable (Silver/TimescaleDB) storage
5. Sub-100ms cross-stream query latency (p99)

### Current Architecture Constraints

**Existing Components (PRESERVE)**:
- `apps/air-quality-app`: 2,813 LOC working MQTT → Parquet pipeline
- `config-client`: 260 LOC etcd wrapper with watch API
- `neural-core/ParquetStore`: Bronze layer with WAL support
- `neural-core/MqttSource`: MQTT ingestion abstraction
- Docker Compose orchestration (etcd, Mosquitto, air-quality-app)

**Performance Baseline**:
- MQTT throughput: 1 message/sec sustained per stream
- Config retrieval: < 10ms p95 (etcd)
- Watch notification: < 100ms (etcd)
- Parquet write: Batching 100 points OR 5s timeout

---

## Decision

**Adopt a hybrid multi-stream architecture with the following pillars**:

1. **Stream Registry Pattern**: etcd-based registry with watch API for dynamic stream configuration
2. **Hexagonal Architecture Extension**: Generic `Source` trait supporting push/poll patterns
3. **Independent Storage Layers**: Bronze (Parquet) for archival, Silver (TimescaleDB) for queries
4. **Dual-Write Coordination**: Ingestion writes to both Bronze and Silver atomically
5. **Parallel Deployment Strategy**: New `ingestion-coordinator` runs alongside existing `air-quality-app`

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  STREAM REGISTRY (etcd)                     │
│  streams/air-quality/, streams/home-events/, streams/weather│
│  (schema, sources, retention policies)                      │
└────────────────────┬────────────────────────────────────────┘
                     │ watch API
                     ▼
┌─────────────────────────────────────────────────────────────┐
│           INGESTION COORDINATOR (new Rust binary)           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ MqttSource  │  │ HttpPoller  │  │  Webhook    │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         └────────────────┴────────────────┘                 │
│                          │                                   │
│                   Ingestion Router                          │
│                   (schema validation)                       │
│                          │                                   │
│              Storage Layer Manager (dual-write)             │
│         ┌────────────────┴────────────────┐                 │
└─────────┼─────────────────────────────────┼─────────────────┘
          │                                 │
┌─────────▼─────────┐           ┌───────────▼──────────┐
│ BRONZE (Parquet)  │           │ SILVER (TimescaleDB) │
│ - Append-only     │           │ - Queryable          │
│ - Full history    │           │ - Hypertables        │
│ - Analytics       │           │ - Aggregates         │
└───────────────────┘           └──────────────────────┘
          │                                 │
          └─────────────┬───────────────────┘
                        ▼
          ┌─────────────────────────────┐
          │  DASHBOARDS & ANALYTICS     │
          │  - Grafana (Silver)         │
          │  - Predictive Models (Both) │
          └─────────────────────────────┘
```

---

## Rationale

### Alternative Architectures Considered

#### Alternative 1: Single Unified Table

**Approach**: All streams in one TimescaleDB table with `stream_id` column and JSONB data

**Pros**:
- Simple schema (no DDL per stream)
- Easy to add streams (no migration)
- Flexible schema per stream

**Cons**:
- Poor query performance (JSONB aggregations slow)
- Weak type safety (schema errors at runtime)
- Compression ineffective (mixed types)
- Complex cross-stream joins

**Verdict**: Rejected - Performance unacceptable for analytics

---

#### Alternative 2: Microservices per Stream

**Approach**: Separate service binary for each stream (air-quality-service, home-events-service, etc.)

**Pros**:
- Independent scaling per stream
- Language diversity (Rust, Python, Go)
- Team ownership boundaries

**Cons**:
- Operational complexity (3-5 services)
- Duplicate infrastructure code
- Overkill for home-scale deployment
- Higher memory usage (multiple processes)

**Verdict**: Rejected - Over-engineering for current scale

---

#### Alternative 3: Custom Event Bus (Redis/Kafka)

**Approach**: Central event bus routing messages to multiple consumers

**Pros**:
- Decoupled producers/consumers
- Replay capability
- Industry standard pattern

**Cons**:
- Additional infrastructure (Redis Streams or Kafka)
- Memory overhead (message buffering)
- Complexity for simple use case
- Raspberry Pi resource constraints

**Verdict**: Deferred to future scaling needs

---

### Chosen Approach: Single Coordinator with Independent Tables

**Why This Architecture**:

1. **Preserves Existing Investment**:
   - Reuses etcd (AIR-003 pattern)
   - Extends ParquetStore (AIR-001/002)
   - Leverages config-client (260 LOC)

2. **Balances Performance and Simplicity**:
   - Typed tables beat JSONB for aggregations
   - Single binary reduces overhead
   - Stream count manageable (3-5 streams)

3. **Enables Key Requirements**:
   - Dynamic registration via etcd watch
   - Sub-100ms cross-stream queries (ASOF JOIN)
   - Bronze + Silver dual-write
   - Schema validation per stream

4. **Fits Deployment Constraints**:
   - Single process (low memory)
   - Raspberry Pi 5 compatible
   - Docker Compose orchestration

---

## Design Details

### 1. Stream Registry Schema (etcd)

```yaml
# Key: /streams/air-quality/config
stream_id: air-quality
description: Indoor air quality measurements
retention_days: 365
compression_after_days: 7
enabled: true

# Key: /streams/air-quality/schema
fields:
  - name: pm25
    type: float
    unit: µg/m³
    nullable: false
    validation:
      min: 0.0
      max: 500.0
  - name: co2
    type: int
    unit: ppm
    nullable: false
    validation:
      min: 380
      max: 10000

# Key: /streams/air-quality/sources
sources:
  - id: airgradient-mqtt
    type: mqtt
    topic: airgradient/readings/#
    qos: 1
    enabled: true
```

**Key Decisions**:
- Hierarchical keys: `/streams/{stream-id}/{config|schema|sources}`
- JSON values for complex structures (serde compatibility)
- Watch `/streams/*` for dynamic updates
- Schema includes validation rules (ranges, nullability)

---

### 2. Generic Source Trait

```rust
#[async_trait]
pub trait Source: Send + Sync {
    /// Unique identifier for this stream
    fn stream_id(&self) -> &str;

    /// Type of source (mqtt, http_poll, webhook, etc.)
    fn source_type(&self) -> SourceType;

    /// For poll-based sources (HTTP, file)
    async fn fetch(&self) -> Result<Vec<StreamRecord>>;

    /// For push-based sources (MQTT, WebSocket)
    async fn subscribe(&self) -> Result<Receiver<StreamRecord>>;

    /// Health check for monitoring
    async fn health_check(&self) -> Result<HealthStatus>;
}

/// Unified record type across all sources
pub struct StreamRecord {
    pub stream_id: String,
    pub point: TimeSeriesPoint,  // Reuse existing type
    pub metadata: Option<RecordMetadata>,
}
```

**Key Decisions**:
- Single trait supports both push and poll patterns
- Wraps existing `TimeSeriesPoint` (backward compatible)
- Async-first (tokio runtime)
- Metadata for lineage tracking

**Backward Compatibility**:
```rust
impl From<TimeSeriesPoint> for StreamRecord {
    fn from(point: TimeSeriesPoint) -> Self {
        Self {
            stream_id: "air-quality".to_string(),
            point,
            metadata: None,
        }
    }
}
```

---

### 3. Storage Layer Architecture

#### Bronze Layer (Parquet)

**Partitioning Strategy**:
```
data/bronze/
├── air-quality/
│   └── 2025/12/15/*.parquet
├── home-events/
│   └── 2025/12/15/*.parquet
└── weather/
    └── 2025/12/15/*.parquet
```

**Implementation** (Extend existing ParquetStore):
```rust
impl ParquetStore {
    // Existing method (preserve)
    pub async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<()>;

    // NEW method (additive)
    pub async fn write_batch_for_stream(
        &self,
        stream_id: &str,
        points: Vec<TimeSeriesPoint>
    ) -> Result<()> {
        let path = format!("{}/bronze/{}/{}",
            self.base_path, stream_id, current_date());
        // ... existing write logic, just different path
    }
}
```

**Preservation Requirements**:
- DO NOT modify existing `write_batch()` signature
- Maintain WAL format compatibility
- Existing Parquet files remain readable

---

#### Silver Layer (TimescaleDB)

**Schema per Stream**:
```sql
-- air_quality table (hypertable)
CREATE TABLE air_quality (
    time        TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    pm25        DOUBLE PRECISION,
    pm10        DOUBLE PRECISION,
    co2         INTEGER,
    voc         INTEGER,
    temperature DOUBLE PRECISION,
    humidity    DOUBLE PRECISION
);
SELECT create_hypertable('air_quality', 'time');

-- home_events table (hypertable)
CREATE TABLE home_events (
    time        TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    target      TEXT NOT NULL,
    state       TEXT,
    metadata    JSONB
);
SELECT create_hypertable('home_events', 'time');

-- Continuous aggregates (automatic rollups)
CREATE MATERIALIZED VIEW air_quality_5min
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('5 minutes', time) AS bucket,
    location_id,
    AVG(pm25) AS pm25_avg,
    AVG(co2) AS co2_avg
FROM air_quality
GROUP BY bucket, location_id;
```

**Key Decisions**:
- Independent typed tables (not unified JSONB)
- Hypertables for time-series optimization
- Continuous aggregates for automatic rollups
- Compression after 7 days (policy per stream)

**Cross-Stream Query Example**:
```sql
-- Correlate air quality with window state
SELECT
    aq.time,
    aq.pm25,
    he.state AS window_state
FROM air_quality aq
ASOF JOIN home_events he
    ON he.time <= aq.time
   AND he.event_type = 'window_state'
WHERE aq.time > NOW() - INTERVAL '24 hours';
```

---

### 4. Dual-Write Coordination

**Storage Layer Manager**:
```rust
pub struct StorageLayerManager {
    bronze: Arc<ParquetStore>,
    silver: Arc<TimescaleAdapter>,
}

impl StorageLayerManager {
    pub async fn write_batch(
        &self,
        stream_id: &str,
        records: Vec<StreamRecord>
    ) -> Result<()> {
        let points: Vec<TimeSeriesPoint> =
            records.into_iter().map(|r| r.point).collect();

        // Write to Bronze FIRST (authoritative)
        self.bronze.write_batch_for_stream(stream_id, points.clone()).await?;

        // Write to Silver (best-effort)
        if let Err(e) = self.silver.write_batch(stream_id, points).await {
            tracing::warn!("Silver write failed: {}, Bronze intact", e);
            // Bronze data is safe, Silver can rebuild from Bronze
        }

        Ok(())
    }
}
```

**Key Decisions**:
- Bronze write is authoritative (MUST succeed)
- Silver write is best-effort (can fail)
- Silver can rebuild from Bronze (backfill)
- No distributed transaction (complexity vs benefit)

**Failure Modes**:
| Scenario | Bronze | Silver | Result |
|----------|--------|--------|--------|
| Both OK | ✅ | ✅ | Ideal state |
| Silver fails | ✅ | ❌ | Degraded (dashboards stale, backfill later) |
| Bronze fails | ❌ | ⏭️ | Error (reject write, retry) |

---

### 5. Parallel Deployment Strategy

**Migration Path**:

1. **Phase 1: Coexistence** (Week 1-2)
   - Deploy `ingestion-coordinator` alongside `air-quality-app`
   - Both services ingest air-quality stream (validation)
   - Monitor dual ingestion for correctness

2. **Phase 2: New Streams** (Week 3-4)
   - Add `home-events` and `weather` streams to coordinator
   - `air-quality-app` continues as-is
   - Validate cross-stream queries

3. **Phase 3: Cutover** (Week 5)
   - Redirect AirGradient sensors to coordinator
   - Keep `air-quality-app` as backup (standby mode)
   - Monitor for 48 hours

4. **Phase 4: Decommission** (Week 6+)
   - Remove `air-quality-app` from stack
   - Document migration lessons

**Rollback Safety**:
- Parquet files unchanged (Bronze layer)
- `air-quality-app` can restart immediately
- etcd config preserved
- No schema migrations needed

---

## Consequences

### Positive Consequences

1. **Preserves Existing Investment**:
   - Reuses 3,500+ LOC (air-quality-app, config-client, ParquetStore)
   - Extends proven patterns (etcd, hexagonal, Parquet)
   - Zero downtime migration path

2. **Enables Key Use Cases**:
   - Predictive models with cross-stream features
   - Real-time dashboards (Grafana + TimescaleDB)
   - Dynamic stream registration (etcd watch)

3. **Performance Characteristics**:
   - Cross-stream queries: < 100ms p99 (TimescaleDB ASOF JOIN)
   - Config updates: < 100ms (etcd watch)
   - Storage efficiency: Parquet compression + TimescaleDB compression

4. **Operational Simplicity**:
   - Single coordinator binary
   - Docker Compose orchestration
   - Raspberry Pi 5 compatible (< 1.5GB memory)

### Negative Consequences (Trade-offs Accepted)

1. **Schema Rigidity**:
   - Adding fields requires TimescaleDB migration
   - **Mitigation**: Automated DDL generation from registry
   - **Future**: Schema evolution support

2. **Dual-Write Complexity**:
   - Bronze and Silver can diverge on Silver failure
   - **Mitigation**: Bronze is authoritative, Silver backfill from Bronze
   - **Future**: Async ETL for Silver

3. **etcd Dependency**:
   - Single point of failure for stream registry
   - **Mitigation**: Plan for multi-node etcd cluster
   - **Current**: Acceptable for home deployment

4. **Limited Horizontal Scaling**:
   - Single coordinator binary limits throughput
   - **Mitigation**: Sufficient for 3-5 streams at 1 msg/sec each
   - **Future**: Event bus pattern for scale-out

---

## Compliance and Alignment

### Alignment with Existing Architecture Patterns

1. **AIR-001 Hexagonal Architecture**: ✅
   - Generic `Source` trait follows ports-adapter pattern
   - Domain logic isolated from infrastructure

2. **AIR-002 Pipeline Batching**: ✅
   - Reuse batching pattern (100 points OR 5s timeout)
   - Channel-based async pipeline

3. **AIR-003 etcd Configuration**: ✅
   - Stream registry extends etcd patterns
   - Watch API for dynamic updates
   - GitOps sync for stream definitions

### Performance Requirements

| Requirement | Target | Expected |
|-------------|--------|----------|
| Cross-stream query latency | < 100ms p99 | 50-80ms (TimescaleDB ASOF JOIN) |
| Config update propagation | < 100ms | < 100ms (etcd watch) |
| Ingestion throughput | 5 msg/sec total | 1 msg/sec × 5 streams |
| Memory usage | < 1.5GB | ~800MB (single binary) |

### Security Considerations

1. **Sensitive Configuration**: DO NOT store secrets in etcd
   - Use environment variables for API keys
   - Future: Integrate with Vault

2. **Stream Isolation**: No cross-stream data leakage
   - Independent Parquet partitions
   - Separate TimescaleDB tables

3. **Authentication**: etcd TLS + client certs (production)
   - Development: no auth (acceptable)

---

## Implementation Guidance

### Key Interfaces

```rust
// Stream Registry Client (extend config-client)
pub struct StreamRegistry {
    client: ConfigClient,
}

impl StreamRegistry {
    pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig>;
    pub async fn list_streams(&self) -> Result<Vec<String>>;
    pub async fn watch_streams(&self) -> Result<Receiver<StreamEvent>>;
}

// Ingestion Router
pub struct IngestionRouter {
    registry: Arc<StreamRegistry>,
}

impl IngestionRouter {
    pub async fn route(&self, record: StreamRecord) -> Result<()> {
        // 1. Load stream config from registry
        // 2. Validate against schema
        // 3. Forward to storage manager
    }
}

// Storage Layer Manager
pub struct StorageLayerManager {
    bronze: Arc<ParquetStore>,
    silver: Arc<TimescaleAdapter>,
}

impl StorageLayerManager {
    pub async fn write_batch(
        &self,
        stream_id: &str,
        records: Vec<StreamRecord>
    ) -> Result<()>;
}
```

### Critical Path

1. **Foundation** (2-3 days):
   - StreamRecord, StreamConfig types
   - StreamRegistry (wrap config-client)

2. **Storage** (3-4 days):
   - Extend ParquetStore for multi-stream
   - Build TimescaleAdapter
   - DDL generator

3. **Sources** (3-4 days):
   - Generic Source trait
   - HttpPoller, WebhookHandler

4. **Coordination** (4-5 days):
   - Ingestion Router
   - Storage Layer Manager
   - Ingestion Coordinator

---

## Related Decisions

- **ADR-002**: Stream Registry Design (etcd schema)
- **ADR-003**: Storage Layer Strategy (Bronze + Silver)
- **ADR-004**: Source Abstraction Pattern (push/poll)
- **ADR-005**: Dual-Write Coordination (Bronze-first)

---

## References

- AIR-001: Hexagonal Architecture (air-quality foundation)
- AIR-002: MQTT to Parquet Pipeline (batching patterns)
- AIR-003: etcd Configuration Architecture (config-client)
- PLATFORM_ARCHITECTURE.md: Detailed component design
- DEPENDENCY_MAP.md: Component dependencies and build order

---

**Last Updated**: 2025-12-15
**Next Review**: After Phase 1 implementation (ingestion-coordinator deployed)
