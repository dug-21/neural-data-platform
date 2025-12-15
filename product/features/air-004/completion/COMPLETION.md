# AIR-004: Generic Multi-Stream Data Platform - SPARC Completion

## Document Status

**Status**: Integration Planning Complete
**Version**: 1.0.0
**Last Updated**: 2025-12-15
**Related Documents**:
- [Platform Architecture](/workspaces/neural-data-platform/product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md)
- [AIR-003 Implementation](/workspaces/neural-data-platform/product/features/air-003/)
- [AIR-002 Configuration System](/workspaces/neural-data-platform/product/features/air-002/)

---

## Executive Summary

This document provides the complete integration and deployment plan for transforming the neural-data-platform from a single-stream air quality system into a generic multi-stream data platform. The design builds on existing infrastructure (AIR-001, AIR-002, AIR-003) while introducing new capabilities for heterogeneous data ingestion, multi-stream correlation, and predictive analytics.

**Key Deliverables**:
1. Stream Registry in etcd with hot-reload capability
2. Generic ingestion coordinator supporting MQTT, HTTP polling, and webhooks
3. Dual-layer storage (Bronze Parquet + Silver TimescaleDB)
4. Stream-agnostic dashboards and monitoring
5. Migration path from single-stream to multi-stream architecture

**Timeline**: 6 phases, 4-6 weeks total

---

## Table of Contents

1. [Integration Roadmap](#1-integration-roadmap)
2. [Infrastructure Changes](#2-infrastructure-changes)
3. [Deployment Strategy](#3-deployment-strategy)
4. [Operational Runbook](#4-operational-runbook)
5. [Future Extensions](#5-future-extensions)
6. [Documentation Deliverables](#6-documentation-deliverables)

---

## 1. Integration Roadmap

### 1.1 Migration Philosophy

**Core Principle**: Evolutionary, not revolutionary

- Preserve existing air-quality-app functionality throughout migration
- Introduce new components alongside existing ones
- Enable gradual feature flag activation
- Maintain backward compatibility for external integrations

### 1.2 Phase-by-Phase Integration Plan

#### Phase 1: Foundation - Stream Registry (Week 1)

**Objective**: Establish stream metadata management in etcd

**Tasks**:

1. **Define Stream Registry Schema** (2 hours)
   - Create etcd key structure: `streams/{stream-id}/{config|schema|sources}`
   - Document field types and validation rules
   - Define retention and compression policies

2. **Implement Registry Client** (1 day)
   ```rust
   // Location: core/src/registry/mod.rs
   pub struct StreamRegistry {
       etcd_client: Arc<EtcdClient>,
       cache: Arc<RwLock<HashMap<String, StreamConfig>>>,
   }

   impl StreamRegistry {
       pub async fn watch_streams(&self) -> Result<Receiver<StreamEvent>>;
       pub async fn get_stream(&self, stream_id: &str) -> Result<StreamConfig>;
       pub async fn register_stream(&self, config: StreamConfig) -> Result<()>;
       pub async fn delete_stream(&self, stream_id: &str) -> Result<()>;
   }
   ```

3. **Add Initial Stream Definitions** (4 hours)
   - Migrate air-quality config to stream registry format
   - Create etcd loader script: `scripts/load-stream-configs.sh`
   - Define home-events and weather stream templates

4. **Build Registry Watch Integration** (1 day)
   - Implement etcd watch for hot-reload
   - Add stream config validation
   - Create integration tests

**Dependencies**: Existing etcd infrastructure from AIR-003

**Deliverables**:
- `core/src/registry/` module
- Stream config YAML templates in `config/streams/`
- Migration script for air-quality config
- Integration tests: `tests/integration/stream_registry_tests.rs`

**Validation**:
```bash
# Load streams into etcd
./scripts/load-stream-configs.sh

# Verify registry
curl http://localhost:2379/v3/kv/range -X POST -d '{"key":"c3RyZWFtcy8="}' | jq
```

---

#### Phase 2: Generic Source Abstraction (Week 1-2)

**Objective**: Refactor existing sources to unified trait interface

**Tasks**:

1. **Define Source Trait** (4 hours)
   ```rust
   // Location: core/src/sources/mod.rs
   #[async_trait]
   pub trait Source: Send + Sync {
       fn stream_id(&self) -> &str;
       fn source_type(&self) -> SourceType;

       // For poll-based sources (HTTP, file)
       async fn fetch(&self) -> Result<Vec<StreamRecord>>;

       // For push-based sources (MQTT, WebSocket)
       async fn subscribe(&self) -> Result<Receiver<StreamRecord>>;

       async fn health_check(&self) -> Result<HealthStatus>;
   }

   pub enum SourceType {
       Mqtt,
       HttpPoll,
       Webhook,
       WebSocket,
       FileWatch,
   }

   pub struct StreamRecord {
       pub stream_id: String,
       pub timestamp: DateTime<Utc>,
       pub data: serde_json::Value,
       pub metadata: HashMap<String, String>,
   }
   ```

2. **Refactor MqttSource** (1 day)
   - Adapt `core/src/sources/mqtt.rs` to new trait
   - Add stream_id field
   - Implement subscribe() method
   - Maintain backward compatibility with existing air-quality usage

3. **Implement HttpPollingSource** (1 day)
   ```rust
   // Location: core/src/sources/http_poll.rs
   pub struct HttpPollingSource {
       stream_id: String,
       url: String,
       interval: Duration,
       auth: Option<AuthConfig>,
       client: reqwest::Client,
   }
   ```

4. **Implement WebhookHandler** (1 day)
   ```rust
   // Location: core/src/sources/webhook.rs
   pub struct WebhookSource {
       stream_id: String,
       path: String,
       auth: WebhookAuth,
       receiver: Receiver<StreamRecord>,
   }
   ```

**Integration Points**:
- Existing `neural_core::MqttConfig` usage
- air-quality-app MQTT handler

**Deliverables**:
- `core/src/sources/mod.rs` with unified trait
- Updated MqttSource, HttpPollingSource, WebhookSource
- Unit tests for each source type
- Migration guide for existing code

**Validation**:
```bash
# Test MQTT source with new trait
cargo test --package platform-core sources::mqtt::tests

# Test HTTP polling source
cargo test --package platform-core sources::http_poll::tests
```

---

#### Phase 3: Ingestion Coordinator (Week 2-3)

**Objective**: Build central coordinator to manage multiple sources and route to streams

**Tasks**:

1. **Implement Ingestion Router** (2 days)
   ```rust
   // Location: core/src/ingestion/router.rs
   pub struct IngestionRouter {
       registry: Arc<StreamRegistry>,
       validators: HashMap<String, Arc<dyn SchemaValidator>>,
   }

   impl IngestionRouter {
       pub async fn route_record(&self, record: StreamRecord) -> Result<ValidatedRecord>;
       pub async fn validate_schema(&self, stream_id: &str, data: &Value) -> Result<()>;
   }
   ```

2. **Build Ingestion Coordinator** (3 days)
   ```rust
   // Location: core/src/ingestion/coordinator.rs
   pub struct IngestionCoordinator {
       registry: Arc<StreamRegistry>,
       router: Arc<IngestionRouter>,
       sources: Arc<RwLock<HashMap<String, Box<dyn Source>>>>,
       writer: Arc<dyn StorageWriter>,
   }

   impl IngestionCoordinator {
       pub async fn spawn_sources(&mut self) -> Result<()>;
       pub async fn run(&self) -> Result<()>;
       pub async fn reload_config(&mut self) -> Result<()>; // Hot-reload
   }
   ```

3. **Implement Schema Validation** (1 day)
   - Build JSON Schema validator from registry schema
   - Add type coercion and nullable checks
   - Create validation error reporting

4. **Add Dynamic Source Spawning** (2 days)
   - Watch registry for source config changes
   - Spawn/stop Tokio tasks for each source
   - Handle source failures and restarts

**Integration with Existing Code**:
- Replace hardcoded MQTT handler in `apps/air-quality-app/src/main.rs`
- Preserve existing channel-based pipeline
- Add coordinator as optional feature flag

**Deliverables**:
- `core/src/ingestion/` module with router and coordinator
- Schema validation engine
- Integration tests for multi-source scenarios
- Performance benchmarks (latency, throughput)

**Validation**:
```bash
# Test coordinator with multiple sources
cargo test --package platform-core ingestion::coordinator::tests::test_multi_source

# Benchmark ingestion throughput
cargo bench --package platform-core ingestion_throughput
```

---

#### Phase 4: Storage Layer (Week 3-4)

**Objective**: Implement dual-write to Bronze (Parquet) and Silver (TimescaleDB)

**Tasks**:

1. **Extend ParquetStore for Multi-Stream** (2 days)
   ```rust
   // Location: neural-core/src/storage/parquet.rs
   impl ParquetStore {
       // Add stream-aware methods
       pub async fn write_stream(&self, stream_id: &str, records: Vec<StreamRecord>) -> Result<()>;
       pub fn get_stream_path(&self, stream_id: &str, date: Date) -> PathBuf;
   }
   ```

   **Directory Structure**:
   ```
   data/bronze/
   ├── air-quality/
   │   └── 2025/12/15/
   │       ├── batch-001.parquet
   │       └── batch-002.parquet
   ├── home-events/
   │   └── 2025/12/15/
   │       └── events-001.parquet
   └── weather/
       └── 2025/12/15/
           └── readings-001.parquet
   ```

2. **Implement TimescaleDB Adapter** (3 days)
   ```rust
   // Location: core/src/storage/timescale.rs
   pub struct TimescaleAdapter {
       pool: sqlx::PgPool,
       registry: Arc<StreamRegistry>,
   }

   impl TimescaleAdapter {
       pub async fn create_hypertable(&self, stream_id: &str, schema: &StreamSchema) -> Result<()>;
       pub async fn write_batch(&self, stream_id: &str, records: Vec<StreamRecord>) -> Result<()>;
       pub async fn query_range(&self, stream_id: &str, start: DateTime, end: DateTime) -> Result<Vec<StreamRecord>>;
   }
   ```

3. **Build Auto-DDL Generator** (1 day)
   - Generate CREATE TABLE from registry schema
   - Create hypertable with time partitioning
   - Add indexes for common query patterns
   - Handle schema evolution

4. **Implement Dual-Write Storage Writer** (2 days)
   ```rust
   // Location: core/src/storage/writer.rs
   pub struct DualStorageWriter {
       bronze: Arc<ParquetStore>,
       silver: Arc<TimescaleAdapter>,
       batch_size: usize,
       batch_timeout: Duration,
   }

   impl DualStorageWriter {
       pub async fn write(&self, record: StreamRecord) -> Result<()>;
       pub async fn flush(&self) -> Result<()>;
   }
   ```

5. **Add Storage Health Checks** (1 day)
   - Verify Parquet write latency
   - Check TimescaleDB connection pool
   - Monitor disk space usage
   - Alert on write failures

**TimescaleDB Schema Example**:
```sql
-- Auto-generated from stream registry
CREATE TABLE air_quality (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    pm25 REAL NOT NULL,
    pm10 REAL,
    co2 INTEGER NOT NULL,
    voc INTEGER,
    temperature REAL,
    humidity REAL,
    metadata JSONB
);

SELECT create_hypertable('air_quality', 'timestamp');

-- Continuous aggregates
CREATE MATERIALIZED VIEW mv_air_quality_5min
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('5 minutes', timestamp) as bucket,
    location_id,
    avg(pm25) as pm25_avg,
    max(pm25) as pm25_max,
    avg(co2) as co2_avg
FROM air_quality
GROUP BY bucket, location_id;

-- Compression policy
ALTER TABLE air_quality SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id'
);

SELECT add_compression_policy('air_quality', INTERVAL '7 days');

-- Retention policy
SELECT add_retention_policy('air_quality', INTERVAL '365 days');
```

**Deliverables**:
- Extended ParquetStore with stream support
- TimescaleDB adapter with connection pooling
- Auto-DDL generator
- SQL migration scripts: `migrations/timescaledb/`
- Storage integration tests

**Validation**:
```bash
# Initialize TimescaleDB
psql -h localhost -U postgres -d neural_data -f migrations/timescaledb/001_init_streams.sql

# Test dual-write
cargo test --package platform-core storage::writer::tests::test_dual_write

# Verify Parquet files
ls -lh data/bronze/air-quality/2025/12/15/

# Query TimescaleDB
psql -h localhost -U postgres -d neural_data -c "SELECT COUNT(*) FROM air_quality;"
```

---

#### Phase 5: Dashboards and Monitoring (Week 4-5)

**Objective**: Provide visibility into multi-stream platform health and data

**Tasks**:

1. **Create Grafana Dashboard Templates** (2 days)
   - Stream ingestion rates (records/sec per stream)
   - Storage lag (Bronze vs Silver write times)
   - Schema validation errors
   - Source health status
   - Cross-stream correlation views

2. **Add Prometheus Metrics** (1 day)
   ```rust
   // Metrics to add
   ingestion_records_total{stream_id, source_type}
   ingestion_latency_seconds{stream_id, source_type}
   validation_errors_total{stream_id, error_type}
   storage_write_duration_seconds{layer, stream_id}
   storage_size_bytes{layer, stream_id}
   source_health{stream_id, source_id}
   ```

3. **Build Stream Registry Dashboard** (1 day)
   - Show all registered streams
   - Display source configurations
   - View schema definitions
   - Monitor retention policies

4. **Create Alerting Rules** (1 day)
   ```yaml
   # File: docker/production/configs/prometheus/stream_alerts.yml
   groups:
     - name: stream_ingestion
       rules:
         - alert: StreamIngestionStopped
           expr: rate(ingestion_records_total[5m]) == 0
           for: 10m
           annotations:
             summary: "No data ingested for stream {{ $labels.stream_id }}"

         - alert: HighValidationErrorRate
           expr: rate(validation_errors_total[5m]) > 0.1
           for: 5m
           annotations:
             summary: "High validation error rate for {{ $labels.stream_id }}"
   ```

**Deliverables**:
- Grafana dashboards: `docker/production/configs/grafana/dashboards/streams/`
  - `multi_stream_overview.json`
  - `stream_detail.json`
  - `storage_performance.json`
- Prometheus alerting rules
- Metrics instrumentation in Rust code

**Validation**:
```bash
# Access Grafana
open http://localhost:3000

# Check Prometheus targets
curl http://localhost:9090/api/v1/targets | jq

# Test alert rules
curl http://localhost:9090/api/v1/rules | jq '.data.groups[] | select(.name=="stream_ingestion")'
```

---

#### Phase 6: New Stream Integration (Week 5-6)

**Objective**: Add home-events and weather streams to validate multi-stream capability

**Tasks**:

1. **Deploy Home Events Stream** (2 days)
   - Configure MQTT source for `home/events/#`
   - Add webhook endpoint `/api/events`
   - Create TimescaleDB table
   - Set up Grafana dashboard

2. **Deploy Weather Stream** (1 day)
   - Configure HTTP polling source (OpenWeatherMap API)
   - Set 5-minute polling interval
   - Store outdoor conditions
   - Add weather overlay to air quality dashboard

3. **Build Cross-Stream Correlation Queries** (2 days)
   ```sql
   -- Example: CO2 spike correlation with window state
   SELECT
       a.timestamp,
       a.co2,
       h.event_type,
       h.state
   FROM air_quality a
   ASOF JOIN home_events h
       ON a.timestamp >= h.timestamp
       AND h.event_type = 'window_state'
   WHERE a.timestamp > NOW() - INTERVAL '1 hour'
       AND a.co2 > 1000
   ORDER BY a.timestamp;
   ```

4. **Create Multi-Stream Analytics Dashboard** (1 day)
   - Indoor vs outdoor temperature comparison
   - CO2 levels vs window open events
   - Duration calculations for home events
   - Predictive model input feature view

**Deliverables**:
- Home events stream configuration
- Weather stream configuration
- Cross-stream SQL query templates
- Multi-stream analytics dashboard

**Validation**:
```bash
# Trigger home event via webhook
curl -X POST http://localhost:8080/api/events \
  -H "Content-Type: application/json" \
  -d '{"event_type":"window_state","target":"bedroom","state":"open"}'

# Verify weather polling
docker logs air-quality-app | grep "weather"

# Query cross-stream data
psql -h localhost -U postgres -d neural_data -f queries/cross_stream_examples.sql
```

---

### 1.3 Backward Compatibility Strategy

**Preserving Existing Functionality**:

1. **Air Quality App Continues to Work**
   - Existing MQTT handler remains functional
   - API endpoints unchanged (`/api/v1/air-quality/*`)
   - Parquet storage location preserved

2. **Gradual Migration Approach**
   ```rust
   // Feature flag in config
   features:
     enable_multi_stream: false  # Default: use legacy single-stream mode
     enable_stream_registry: false
     enable_dual_write: false
   ```

3. **API Version Compatibility**
   - `/api/v1/*` - Legacy air-quality endpoints (unchanged)
   - `/api/v2/streams/*` - New multi-stream endpoints

4. **Configuration Compatibility**
   - `config/base/air-quality.yaml` - Legacy config (still supported)
   - `config/streams/*.yaml` - New stream configs (optional)

**Migration Timeline**:
- Week 1-4: New components added, legacy mode default
- Week 5: Enable feature flags in development
- Week 6: Production testing with feature flags
- Week 7+: Gradual rollout to production

---

## 2. Infrastructure Changes

### 2.1 Docker Compose Updates

#### Development Environment (`docker-compose.yml`)

**Add TimescaleDB Service**:
```yaml
services:
  # ... existing mosquitto, etcd, air-quality-app ...

  # TimescaleDB - Time-series database
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    container_name: neural-timescaledb
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=postgres
      - POSTGRES_DB=neural_data
    volumes:
      - timescaledb-data:/var/lib/postgresql/data
      - ./migrations/timescaledb:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped

  # Grafana - Update to include TimescaleDB datasource
  grafana:
    # ... existing config ...
    environment:
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - POSTGRES_HOST=timescaledb
      - POSTGRES_PORT=5432
      - POSTGRES_DB=neural_data
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=postgres
    depends_on:
      - prometheus
      - timescaledb

volumes:
  # ... existing volumes ...
  timescaledb-data:
    driver: local
```

**Updated Air Quality App Service**:
```yaml
  air-quality-app:
    # ... existing config ...
    environment:
      # ... existing env vars ...
      - TIMESCALE_HOST=timescaledb
      - TIMESCALE_PORT=5432
      - TIMESCALE_DB=neural_data
      - TIMESCALE_USER=postgres
      - TIMESCALE_PASSWORD=postgres
      - ENABLE_MULTI_STREAM=true
      - ENABLE_DUAL_WRITE=true
    depends_on:
      mosquitto:
        condition: service_healthy
      etcd:
        condition: service_healthy
      timescaledb:
        condition: service_healthy
```

#### Production Environment (`docker/production/docker-compose.prod.yml`)

**Add Environment-Specific Overrides**:
```yaml
services:
  timescaledb:
    image: timescale/timescaledb:latest-pg15
    environment:
      - POSTGRES_USER=${TIMESCALE_USER}
      - POSTGRES_PASSWORD=${TIMESCALE_PASSWORD}
      - POSTGRES_DB=${TIMESCALE_DB}
    volumes:
      - timescaledb-data:/var/lib/postgresql/data
      - timescaledb-backups:/backups
    deploy:
      resources:
        limits:
          memory: 4G
          cpus: '2'
        reservations:
          memory: 2G
    command:
      - postgres
      - -c
      - shared_preload_libraries=timescaledb
      - -c
      - max_connections=200
      - -c
      - shared_buffers=1GB
      - -c
      - effective_cache_size=3GB
      - -c
      - work_mem=16MB

volumes:
  timescaledb-backups:
    driver: local
```

**Complete Docker Compose File**: `/workspaces/neural-data-platform/docker/multi-stream-compose.yml`

---

### 2.2 etcd Schema Additions

**Stream Registry Structure**:

```yaml
# Prefix: /streams/

# Stream Configuration
/streams/air-quality/config:
  stream_id: air-quality
  description: Indoor air quality measurements
  retention_days: 365
  compression_after_days: 7
  enabled: true

# Stream Schema
/streams/air-quality/schema:
  fields:
    - name: pm25
      type: float
      unit: µg/m³
      nullable: false
      range: [0, 1000]
    - name: pm10
      type: float
      unit: µg/m³
      nullable: true
      range: [0, 1000]
    - name: co2
      type: int
      unit: ppm
      nullable: false
      range: [400, 5000]
    - name: voc
      type: int
      unit: index
      nullable: true
      range: [0, 500]
    - name: temperature
      type: float
      unit: celsius
      nullable: true
      range: [-50, 100]
    - name: humidity
      type: float
      unit: percent
      nullable: true
      range: [0, 100]

# Stream Sources
/streams/air-quality/sources:
  sources:
    - id: mqtt-airgradient
      type: mqtt
      enabled: true
      config:
        topic: airgradient/+/measures
        qos: 1
        transform: airgradient_v1  # Optional data transformation

# Similar structure for other streams
/streams/home-events/config: {...}
/streams/home-events/schema: {...}
/streams/home-events/sources: {...}

/streams/weather/config: {...}
/streams/weather/schema: {...}
/streams/weather/sources: {...}
```

**etcd Loader Script**: `/workspaces/neural-data-platform/scripts/load-stream-configs.sh`

```bash
#!/bin/bash
# Load stream configurations into etcd

ETCD_ENDPOINTS="${ETCD_ENDPOINTS:-http://localhost:2379}"
CONFIG_DIR="${CONFIG_DIR:-./config/streams}"

echo "Loading stream configurations from $CONFIG_DIR to $ETCD_ENDPOINTS"

for stream_dir in "$CONFIG_DIR"/*; do
    if [ -d "$stream_dir" ]; then
        stream_id=$(basename "$stream_dir")
        echo "Loading stream: $stream_id"

        # Load config
        if [ -f "$stream_dir/config.yaml" ]; then
            etcdctl --endpoints="$ETCD_ENDPOINTS" put \
                "/streams/$stream_id/config" \
                "$(cat "$stream_dir/config.yaml")"
        fi

        # Load schema
        if [ -f "$stream_dir/schema.yaml" ]; then
            etcdctl --endpoints="$ETCD_ENDPOINTS" put \
                "/streams/$stream_id/schema" \
                "$(cat "$stream_dir/schema.yaml")"
        fi

        # Load sources
        if [ -f "$stream_dir/sources.yaml" ]; then
            etcdctl --endpoints="$ETCD_ENDPOINTS" put \
                "/streams/$stream_id/sources" \
                "$(cat "$stream_dir/sources.yaml")"
        fi
    fi
done

echo "Stream configurations loaded successfully"

# Verify
echo -e "\nRegistered streams:"
etcdctl --endpoints="$ETCD_ENDPOINTS" get /streams/ --prefix --keys-only | grep config | sed 's|/streams/||' | sed 's|/config||'
```

---

### 2.3 TimescaleDB Table Creation

**Auto-Generated Migration**: `/workspaces/neural-data-platform/migrations/timescaledb/001_init_streams.sql`

```sql
-- Initialize TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- ============================================================
-- STREAM: air-quality
-- Auto-generated from /streams/air-quality/schema
-- ============================================================

CREATE TABLE IF NOT EXISTS air_quality (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    pm25 REAL NOT NULL,
    pm10 REAL,
    co2 INTEGER NOT NULL,
    voc INTEGER,
    temperature REAL,
    humidity REAL,
    metadata JSONB
);

-- Create hypertable (partitioned by time)
SELECT create_hypertable('air_quality', 'timestamp', if_not_exists => TRUE);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_air_quality_location_time ON air_quality (location_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_air_quality_co2 ON air_quality (co2, timestamp DESC);

-- Continuous aggregates (5-minute rollups)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_air_quality_5min
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('5 minutes', timestamp) as bucket,
    location_id,
    COUNT(*) as reading_count,
    AVG(pm25) as pm25_avg,
    MAX(pm25) as pm25_max,
    MIN(pm25) as pm25_min,
    AVG(co2) as co2_avg,
    MAX(co2) as co2_max,
    AVG(temperature) as temp_avg,
    AVG(humidity) as humidity_avg
FROM air_quality
GROUP BY bucket, location_id
WITH NO DATA;

-- Continuous aggregates (1-hour rollups)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_air_quality_1hr
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) as bucket,
    location_id,
    COUNT(*) as reading_count,
    AVG(pm25) as pm25_avg,
    MAX(pm25) as pm25_max,
    MIN(pm25) as pm25_min,
    AVG(co2) as co2_avg,
    MAX(co2) as co2_max,
    AVG(temperature) as temp_avg,
    AVG(humidity) as humidity_avg
FROM air_quality
GROUP BY bucket, location_id
WITH NO DATA;

-- Refresh policies
SELECT add_continuous_aggregate_policy('mv_air_quality_5min',
    start_offset => INTERVAL '1 hour',
    end_offset => INTERVAL '5 minutes',
    schedule_interval => INTERVAL '5 minutes',
    if_not_exists => TRUE
);

SELECT add_continuous_aggregate_policy('mv_air_quality_1hr',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Compression policy (compress data older than 7 days)
ALTER TABLE air_quality SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id',
    timescaledb.compress_orderby = 'timestamp DESC'
);

SELECT add_compression_policy('air_quality', INTERVAL '7 days', if_not_exists => TRUE);

-- Retention policy (delete data older than 365 days)
SELECT add_retention_policy('air_quality', INTERVAL '365 days', if_not_exists => TRUE);

-- ============================================================
-- STREAM: home-events
-- Auto-generated from /streams/home-events/schema
-- ============================================================

CREATE TABLE IF NOT EXISTS home_events (
    timestamp TIMESTAMPTZ NOT NULL,
    event_type TEXT NOT NULL,
    target TEXT NOT NULL,
    state TEXT,
    metadata JSONB
);

SELECT create_hypertable('home_events', 'timestamp', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_home_events_type_time ON home_events (event_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_home_events_target_time ON home_events (target, timestamp DESC);

-- Compression and retention
ALTER TABLE home_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'event_type, target',
    timescaledb.compress_orderby = 'timestamp DESC'
);

SELECT add_compression_policy('home_events', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('home_events', INTERVAL '730 days', if_not_exists => TRUE);

-- ============================================================
-- STREAM: weather
-- Auto-generated from /streams/weather/schema
-- ============================================================

CREATE TABLE IF NOT EXISTS weather (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    temperature REAL NOT NULL,
    humidity REAL NOT NULL,
    pressure REAL,
    wind_speed REAL,
    conditions TEXT,
    metadata JSONB
);

SELECT create_hypertable('weather', 'timestamp', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_weather_location_time ON weather (location_id, timestamp DESC);

-- Continuous aggregates (1-hour rollups)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_weather_1hr
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) as bucket,
    location_id,
    AVG(temperature) as temp_avg,
    AVG(humidity) as humidity_avg,
    AVG(pressure) as pressure_avg,
    AVG(wind_speed) as wind_speed_avg
FROM weather
GROUP BY bucket, location_id
WITH NO DATA;

SELECT add_continuous_aggregate_policy('mv_weather_1hr',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Compression and retention
ALTER TABLE weather SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id',
    timescaledb.compress_orderby = 'timestamp DESC'
);

SELECT add_compression_policy('weather', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('weather', INTERVAL '365 days', if_not_exists => TRUE);

-- ============================================================
-- Helper Functions and Views
-- ============================================================

-- Function to get latest reading for all streams
CREATE OR REPLACE FUNCTION get_latest_readings()
RETURNS TABLE(
    stream_id TEXT,
    latest_timestamp TIMESTAMPTZ,
    record_count BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 'air-quality'::TEXT, MAX(timestamp), COUNT(*)::BIGINT FROM air_quality
    UNION ALL
    SELECT 'home-events'::TEXT, MAX(timestamp), COUNT(*)::BIGINT FROM home_events
    UNION ALL
    SELECT 'weather'::TEXT, MAX(timestamp), COUNT(*)::BIGINT FROM weather;
END;
$$ LANGUAGE plpgsql;

-- View for stream health monitoring
CREATE OR REPLACE VIEW stream_health AS
SELECT * FROM get_latest_readings();
```

**DDL Generator Script**: `/workspaces/neural-data-platform/scripts/generate-timescale-ddl.sh`

```bash
#!/bin/bash
# Generate TimescaleDB DDL from etcd stream registry

ETCD_ENDPOINTS="${ETCD_ENDPOINTS:-http://localhost:2379}"
OUTPUT_FILE="${OUTPUT_FILE:-migrations/timescaledb/002_generated_streams.sql}"

echo "-- Auto-generated TimescaleDB DDL from stream registry" > "$OUTPUT_FILE"
echo "-- Generated at: $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Get list of streams from etcd
streams=$(etcdctl --endpoints="$ETCD_ENDPOINTS" get /streams/ --prefix --keys-only | grep config | sed 's|/streams/||' | sed 's|/config||')

for stream_id in $streams; do
    echo "Processing stream: $stream_id"

    # Fetch schema from etcd
    schema=$(etcdctl --endpoints="$ETCD_ENDPOINTS" get "/streams/$stream_id/schema" --print-value-only)

    # Convert YAML schema to SQL DDL (use Python/Ruby/etc for YAML parsing)
    # For now, this is a placeholder - implement full generator
    echo "-- Stream: $stream_id" >> "$OUTPUT_FILE"
    echo "-- TODO: Parse schema and generate DDL" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
done

echo "DDL generation complete: $OUTPUT_FILE"
```

---

### 2.4 Grafana Dashboard Templates

**Dashboard Provisioning Config**: `/workspaces/neural-data-platform/docker/production/configs/grafana/provisioning/dashboards/streams.yml`

```yaml
apiVersion: 1

providers:
  - name: 'Multi-Stream Platform'
    orgId: 1
    folder: 'Streams'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /etc/grafana/provisioning/dashboards/streams
```

**TimescaleDB Datasource**: `/workspaces/neural-data-platform/docker/production/configs/grafana/provisioning/datasources/timescaledb.yml`

```yaml
apiVersion: 1

datasources:
  - name: TimescaleDB
    type: postgres
    access: proxy
    url: timescaledb:5432
    database: neural_data
    user: postgres
    secureJsonData:
      password: ${POSTGRES_PASSWORD}
    jsonData:
      sslmode: disable
      postgresVersion: 1500
      timescaledb: true
    isDefault: false
    editable: true
```

**Dashboard Template**: `/workspaces/neural-data-platform/docker/production/configs/grafana/dashboards/streams/multi_stream_overview.json`

(See detailed dashboard JSON in section 6.5)

---

## 3. Deployment Strategy

### 3.1 Pre-Deployment Checklist

**Infrastructure Readiness**:
- [ ] TimescaleDB service running and healthy
- [ ] etcd cluster accessible and backed up
- [ ] MQTT broker operational
- [ ] Sufficient disk space for Bronze layer (estimate: 10GB/month/stream)
- [ ] Network connectivity between all services

**Configuration Readiness**:
- [ ] Stream configs loaded into etcd
- [ ] TimescaleDB schemas created
- [ ] Grafana datasources configured
- [ ] Prometheus scrape targets updated
- [ ] Environment variables set in `.env`

**Code Readiness**:
- [ ] All unit tests passing
- [ ] Integration tests passing
- [ ] Performance benchmarks meeting SLAs
- [ ] Docker images built and tagged
- [ ] Feature flags configured

**Team Readiness**:
- [ ] Operations team trained on new architecture
- [ ] Runbook reviewed and understood
- [ ] Rollback procedure tested
- [ ] Monitoring alerts configured

---

### 3.2 Rolling Deployment Steps

#### Step 1: Deploy Infrastructure (Day 1)

```bash
# 1. Backup existing data
./scripts/backup-parquet.sh
./scripts/backup-etcd.sh

# 2. Pull latest Docker images
docker compose pull

# 3. Start TimescaleDB
docker compose up -d timescaledb

# 4. Wait for health check
until docker exec neural-timescaledb pg_isready -U postgres; do
    echo "Waiting for TimescaleDB..."
    sleep 2
done

# 5. Initialize database
docker exec -i neural-timescaledb psql -U postgres -d neural_data < migrations/timescaledb/001_init_streams.sql

# 6. Verify schema
docker exec neural-timescaledb psql -U postgres -d neural_data -c "\dt"
```

**Verification**:
```bash
# Check TimescaleDB health
docker logs neural-timescaledb | tail -20

# Test connection
psql -h localhost -U postgres -d neural_data -c "SELECT version();"

# Verify hypertables
psql -h localhost -U postgres -d neural_data -c "SELECT * FROM timescaledb_information.hypertables;"
```

---

#### Step 2: Load Stream Configurations (Day 1)

```bash
# 1. Load stream configs into etcd
./scripts/load-stream-configs.sh

# 2. Verify configs loaded
etcdctl get /streams/ --prefix --keys-only

# 3. Validate schema
./scripts/validate-stream-configs.sh

# 4. Test registry client
cargo test --package platform-core registry::tests
```

**Verification**:
```bash
# Check each stream
for stream in air-quality home-events weather; do
    echo "=== Stream: $stream ==="
    etcdctl get /streams/$stream/config
done
```

---

#### Step 3: Deploy Updated Application (Day 2)

**Blue-Green Deployment**:

```bash
# 1. Build new image with multi-stream support
docker build -t neural-air-quality:multi-stream .

# 2. Start new instance (green) alongside old (blue)
docker run -d \
    --name air-quality-app-green \
    --network neural-network \
    -p 8081:8080 \
    -e ENABLE_MULTI_STREAM=true \
    -e ENABLE_DUAL_WRITE=false \  # Start without dual-write
    neural-air-quality:multi-stream

# 3. Run smoke tests against green instance
./tests/smoke-test.sh http://localhost:8081

# 4. Switch traffic (update nginx/load balancer)
# ... traffic switch logic ...

# 5. Monitor for issues (15 minutes)
watch -n 5 'docker logs air-quality-app-green | tail -20'

# 6. If stable, stop blue instance
docker stop air-quality-app
docker rm air-quality-app

# 7. Rename green to primary
docker rename air-quality-app-green air-quality-app
```

**Verification**:
```bash
# Health check
curl http://localhost:8080/health | jq

# Check stream registry connection
curl http://localhost:8080/api/v2/streams | jq

# Verify ingestion still working
docker logs air-quality-app | grep "ingestion"
```

---

#### Step 4: Enable Dual-Write (Day 3)

```bash
# 1. Update config to enable dual-write
etcdctl put /config/air-quality-app/features/enable_dual_write "true"

# 2. Reload config (hot-reload via etcd watch)
# Or restart if needed:
docker restart air-quality-app

# 3. Monitor write latency
watch -n 2 'curl -s http://localhost:9090/metrics | grep storage_write_duration'

# 4. Verify data in both layers
ls -lh data/bronze/air-quality/$(date +%Y/%m/%d)/
psql -h localhost -U postgres -d neural_data -c "SELECT COUNT(*) FROM air_quality WHERE timestamp > NOW() - INTERVAL '5 minutes';"

# 5. Check for write errors
docker logs air-quality-app | grep -i error
```

**Verification**:
```bash
# Compare record counts
bronze_count=$(find data/bronze/air-quality -name "*.parquet" -exec wc -l {} + | tail -1 | awk '{print $1}')
silver_count=$(psql -h localhost -U postgres -d neural_data -t -c "SELECT COUNT(*) FROM air_quality;")

echo "Bronze: $bronze_count, Silver: $silver_count"

# Acceptable delta: < 1% (due to batching)
```

---

#### Step 5: Deploy Monitoring (Day 3)

```bash
# 1. Update Prometheus config
docker cp docker/production/configs/prometheus/prometheus.yml neural_prometheus:/etc/prometheus/
docker exec neural_prometheus kill -HUP 1

# 2. Load Grafana dashboards
for dashboard in docker/production/configs/grafana/dashboards/streams/*.json; do
    curl -X POST http://admin:admin@localhost:3000/api/dashboards/db \
        -H "Content-Type: application/json" \
        -d @$dashboard
done

# 3. Test alerting rules
curl http://localhost:9090/api/v1/rules | jq '.data.groups[] | select(.name=="stream_ingestion")'

# 4. Trigger test alert
# ... temporarily stop ingestion to test alert ...
```

**Verification**:
```bash
# Open Grafana
open http://localhost:3000/d/multi-stream-overview

# Check datasources
curl -u admin:admin http://localhost:3000/api/datasources | jq

# Verify Prometheus scraping
curl http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.labels.job=="air-quality-app")'
```

---

#### Step 6: Add New Streams (Day 4-5)

**Home Events Stream**:
```bash
# 1. Enable stream in registry
etcdctl put /streams/home-events/config/enabled "true"

# 2. Verify coordinator spawned source
docker logs air-quality-app | grep "home-events"

# 3. Test webhook endpoint
curl -X POST http://localhost:8080/api/events \
    -H "Content-Type: application/json" \
    -d '{
        "event_type": "window_state",
        "target": "bedroom",
        "state": "open"
    }'

# 4. Verify data in TimescaleDB
psql -h localhost -U postgres -d neural_data -c "SELECT * FROM home_events ORDER BY timestamp DESC LIMIT 5;"
```

**Weather Stream**:
```bash
# 1. Set API key in etcd
etcdctl put /streams/weather/sources/http-openweathermap/api_key "$OPENWEATHERMAP_API_KEY"

# 2. Enable stream
etcdctl put /streams/weather/config/enabled "true"

# 3. Watch logs for polling activity
docker logs -f air-quality-app | grep "weather"

# 4. Verify data
psql -h localhost -U postgres -d neural_data -c "SELECT * FROM weather ORDER BY timestamp DESC LIMIT 5;"
```

---

### 3.3 Health Check Verification

**Automated Health Check Script**: `/workspaces/neural-data-platform/scripts/health-check.sh`

```bash
#!/bin/bash
# Comprehensive health check for multi-stream platform

set -e

ETCD_ENDPOINTS="${ETCD_ENDPOINTS:-http://localhost:2379}"
TIMESCALE_HOST="${TIMESCALE_HOST:-localhost}"
APP_HOST="${APP_HOST:-localhost:8080}"

echo "=== Multi-Stream Platform Health Check ==="
echo ""

# 1. Check etcd
echo "1. Checking etcd..."
if etcdctl --endpoints="$ETCD_ENDPOINTS" endpoint health; then
    echo "✓ etcd healthy"
else
    echo "✗ etcd unhealthy"
    exit 1
fi

# 2. Check TimescaleDB
echo "2. Checking TimescaleDB..."
if psql -h "$TIMESCALE_HOST" -U postgres -d neural_data -c "SELECT 1;" > /dev/null 2>&1; then
    echo "✓ TimescaleDB healthy"
else
    echo "✗ TimescaleDB unhealthy"
    exit 1
fi

# 3. Check application
echo "3. Checking application..."
health_response=$(curl -s http://$APP_HOST/health)
if echo "$health_response" | jq -e '.healthy == true' > /dev/null; then
    echo "✓ Application healthy"
else
    echo "✗ Application unhealthy: $health_response"
    exit 1
fi

# 4. Check stream registry
echo "4. Checking stream registry..."
streams=$(curl -s http://$APP_HOST/api/v2/streams | jq -r '.[].stream_id')
stream_count=$(echo "$streams" | wc -l)
echo "✓ Found $stream_count registered streams"

# 5. Check ingestion activity
echo "5. Checking ingestion activity..."
for stream in $streams; do
    count=$(psql -h "$TIMESCALE_HOST" -U postgres -d neural_data -t -c \
        "SELECT COUNT(*) FROM ${stream//-/_} WHERE timestamp > NOW() - INTERVAL '5 minutes';")
    if [ "$count" -gt 0 ]; then
        echo "✓ $stream: $count records in last 5 minutes"
    else
        echo "⚠ $stream: No recent records"
    fi
done

# 6. Check storage health
echo "6. Checking storage health..."
parquet_size=$(du -sh data/bronze/ | awk '{print $1}')
echo "✓ Bronze layer size: $parquet_size"

db_size=$(psql -h "$TIMESCALE_HOST" -U postgres -d neural_data -t -c \
    "SELECT pg_size_pretty(pg_database_size('neural_data'));")
echo "✓ TimescaleDB size: $db_size"

# 7. Check monitoring
echo "7. Checking monitoring..."
if curl -s http://localhost:9090/api/v1/query?query=up | jq -e '.data.result | length > 0' > /dev/null; then
    echo "✓ Prometheus scraping targets"
else
    echo "⚠ Prometheus not scraping"
fi

echo ""
echo "=== Health Check Complete ==="
```

**Run Health Check**:
```bash
./scripts/health-check.sh

# Expected output:
# === Multi-Stream Platform Health Check ===
#
# 1. Checking etcd...
# ✓ etcd healthy
# 2. Checking TimescaleDB...
# ✓ TimescaleDB healthy
# 3. Checking application...
# ✓ Application healthy
# 4. Checking stream registry...
# ✓ Found 3 registered streams
# 5. Checking ingestion activity...
# ✓ air-quality: 142 records in last 5 minutes
# ✓ home-events: 3 records in last 5 minutes
# ✓ weather: 1 records in last 5 minutes
# 6. Checking storage health...
# ✓ Bronze layer size: 2.3G
# ✓ TimescaleDB size: 1.8 GB
# 7. Checking monitoring...
# ✓ Prometheus scraping targets
#
# === Health Check Complete ===
```

---

### 3.4 Rollback Procedures

#### Scenario 1: Application Failure

**Symptoms**:
- Health check failures
- High error rate in logs
- No data ingestion

**Rollback Steps**:
```bash
# 1. Stop failing instance
docker stop air-quality-app

# 2. Revert to previous image
docker run -d \
    --name air-quality-app \
    --network neural-network \
    -p 8080:8080 \
    neural-air-quality:previous-version

# 3. Verify legacy mode working
curl http://localhost:8080/health

# 4. Investigate logs from failed instance
docker logs air-quality-app > /tmp/failure-logs.txt
```

**Recovery Time**: < 5 minutes

---

#### Scenario 2: TimescaleDB Issues

**Symptoms**:
- Slow queries
- Write timeouts
- Disk space exhaustion

**Rollback Steps**:
```bash
# 1. Disable dual-write to relieve database load
etcdctl put /config/air-quality-app/features/enable_dual_write "false"
docker restart air-quality-app

# 2. Bronze layer continues working (Parquet only)

# 3. Investigate database
psql -h localhost -U postgres -d neural_data

# Common fixes:
# - VACUUM ANALYZE air_quality;
# - Check compression policies
# - Monitor disk space
```

**Recovery Time**: < 10 minutes (dual-write disabled, ingestion continues)

---

#### Scenario 3: Data Corruption

**Symptoms**:
- Schema validation errors
- Mismatched record counts
- Invalid data in queries

**Rollback Steps**:
```bash
# 1. Stop ingestion
docker stop air-quality-app

# 2. Restore TimescaleDB from backup
pg_restore -h localhost -U postgres -d neural_data backups/neural_data_backup.dump

# 3. Replay Bronze layer (Parquet) to Silver (TimescaleDB)
./scripts/bronze-to-silver-backfill.sh \
    --start-date "2025-12-14" \
    --end-date "2025-12-15" \
    --streams air-quality

# 4. Restart ingestion
docker start air-quality-app
```

**Recovery Time**: 30 minutes - 2 hours (depending on backfill volume)

---

#### Scenario 4: Complete Rollback to Legacy Mode

**Last Resort: Revert All Changes**

```bash
# 1. Stop all new components
docker stop air-quality-app
docker stop timescaledb

# 2. Restore original docker-compose.yml
git checkout HEAD~5 docker-compose.yml

# 3. Start legacy stack
docker compose up -d

# 4. Verify Parquet-only mode working
ls -lh data/air-quality/
curl http://localhost:8080/api/v1/air-quality/locations
```

**Data Preservation**:
- Bronze layer (Parquet) remains intact
- No data loss for air-quality stream
- New streams (home-events, weather) temporarily unavailable

**Recovery Time**: < 15 minutes

---

## 4. Operational Runbook

### 4.1 Adding a New Stream (Step-by-Step)

#### Example: Adding "Energy Consumption" Stream

**Step 1: Define Stream Configuration** (15 minutes)

Create `/workspaces/neural-data-platform/config/streams/energy/config.yaml`:
```yaml
stream_id: energy
description: Household energy consumption readings
retention_days: 730  # 2 years
compression_after_days: 30
enabled: true
```

Create `/workspaces/neural-data-platform/config/streams/energy/schema.yaml`:
```yaml
fields:
  - name: kwh
    type: float
    unit: kilowatt-hours
    nullable: false
    range: [0, 100]
  - name: voltage
    type: float
    unit: volts
    nullable: true
    range: [110, 250]
  - name: current
    type: float
    unit: amperes
    nullable: true
    range: [0, 100]
  - name: power_factor
    type: float
    unit: ratio
    nullable: true
    range: [0, 1]
  - name: circuit
    type: string
    nullable: false
```

Create `/workspaces/neural-data-platform/config/streams/energy/sources.yaml`:
```yaml
sources:
  - id: mqtt-energy-monitor
    type: mqtt
    enabled: true
    config:
      topic: energy/monitor/+/readings
      qos: 1
```

---

**Step 2: Load Configuration into etcd** (5 minutes)

```bash
# Load configs
./scripts/load-stream-configs.sh

# Verify
etcdctl get /streams/energy/config
etcdctl get /streams/energy/schema
etcdctl get /streams/energy/sources
```

---

**Step 3: Generate TimescaleDB Schema** (10 minutes)

```bash
# Auto-generate DDL
./scripts/generate-timescale-ddl.sh \
    --stream energy \
    --output migrations/timescaledb/003_energy_stream.sql

# Review generated SQL
cat migrations/timescaledb/003_energy_stream.sql
```

Expected output:
```sql
CREATE TABLE IF NOT EXISTS energy (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    kwh REAL NOT NULL,
    voltage REAL,
    current REAL,
    power_factor REAL,
    circuit TEXT NOT NULL,
    metadata JSONB
);

SELECT create_hypertable('energy', 'timestamp', if_not_exists => TRUE);

-- Indexes, compression, retention policies...
```

---

**Step 4: Apply Database Migration** (5 minutes)

```bash
# Apply migration
psql -h localhost -U postgres -d neural_data -f migrations/timescaledb/003_energy_stream.sql

# Verify table created
psql -h localhost -U postgres -d neural_data -c "\d energy"
```

---

**Step 5: Enable Stream** (2 minutes)

```bash
# The coordinator watches etcd, so enabling the stream triggers auto-spawning
etcdctl put /streams/energy/config/enabled "true"

# Watch logs for source spawning
docker logs -f air-quality-app | grep "energy"
```

Expected log output:
```
[INFO] Stream registry detected new stream: energy
[INFO] Spawning MQTT source for stream: energy, topic: energy/monitor/+/readings
[INFO] MQTT source connected successfully: energy/mqtt-energy-monitor
```

---

**Step 6: Test Ingestion** (10 minutes)

```bash
# Publish test message to MQTT
mosquitto_pub -h localhost -t energy/monitor/main/readings -m '{
    "timestamp": "2025-12-15T12:00:00Z",
    "location_id": "home-main",
    "kwh": 2.5,
    "voltage": 120.2,
    "current": 20.8,
    "power_factor": 0.95,
    "circuit": "main_panel"
}'

# Verify Bronze layer (Parquet)
ls -lh data/bronze/energy/$(date +%Y/%m/%d)/

# Verify Silver layer (TimescaleDB)
psql -h localhost -U postgres -d neural_data -c \
    "SELECT * FROM energy ORDER BY timestamp DESC LIMIT 5;"
```

---

**Step 7: Create Grafana Dashboard** (30 minutes)

```bash
# Copy dashboard template
cp docker/production/configs/grafana/dashboards/streams/template.json \
   docker/production/configs/grafana/dashboards/streams/energy.json

# Edit dashboard (replace placeholders)
# - Update stream_id: energy
# - Configure panels for kwh, voltage, current
# - Add circuit breakdown panel

# Reload Grafana dashboards
docker restart grafana

# Access dashboard
open http://localhost:3000/d/energy-stream
```

---

**Step 8: Configure Alerts** (15 minutes)

Create `/workspaces/neural-data-platform/docker/production/configs/prometheus/energy_alerts.yml`:
```yaml
groups:
  - name: energy_monitoring
    rules:
      - alert: HighEnergyUsage
        expr: avg_over_time(energy_kwh[5m]) > 5
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High energy consumption detected"
          description: "Energy usage at {{ $labels.location_id }} exceeded 5 kWh for 10 minutes"

      - alert: PowerFactorLow
        expr: energy_power_factor < 0.8
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Low power factor detected"
```

Reload Prometheus config:
```bash
docker exec neural_prometheus kill -HUP 1
```

---

**Step 9: Validation** (10 minutes)

```bash
# Run comprehensive check
./scripts/validate-stream.sh energy

# Expected output:
# ✓ Stream config loaded in etcd
# ✓ TimescaleDB table exists
# ✓ Source spawned and healthy
# ✓ Ingestion active (42 records in last hour)
# ✓ Bronze layer has data
# ✓ Silver layer has data
# ✓ Grafana dashboard accessible
# ✓ Prometheus alerts loaded
```

---

**Total Time**: ~2 hours (mostly automation, 30 minutes manual work)

---

### 4.2 Monitoring and Alerting Setup

#### Key Metrics to Monitor

**Ingestion Metrics**:
```promql
# Records ingested per second (by stream)
rate(ingestion_records_total{stream_id="air-quality"}[5m])

# Ingestion latency (source to storage)
histogram_quantile(0.95, rate(ingestion_latency_seconds_bucket[5m]))

# Validation error rate
rate(validation_errors_total[5m]) / rate(ingestion_records_total[5m])
```

**Storage Metrics**:
```promql
# Bronze layer write duration
histogram_quantile(0.95, rate(storage_write_duration_seconds_bucket{layer="bronze"}[5m]))

# Silver layer write duration
histogram_quantile(0.95, rate(storage_write_duration_seconds_bucket{layer="silver"}[5m]))

# Storage size by stream
storage_size_bytes{layer="bronze", stream_id="air-quality"}
```

**Source Health**:
```promql
# Source up/down status
source_health{stream_id="air-quality", source_id="mqtt-airgradient"}

# MQTT reconnection rate
rate(mqtt_reconnects_total[5m])
```

**TimescaleDB Metrics**:
```promql
# Query latency
rate(pg_stat_statements_total_time_seconds[5m]) / rate(pg_stat_statements_calls[5m])

# Compression ratio
timescaledb_compression_ratio{table="air_quality"}

# Chunk count
timescaledb_chunks_total{table="air_quality"}
```

---

#### Alert Rules

**Critical Alerts** (PagerDuty):
```yaml
# File: docker/production/configs/prometheus/critical_alerts.yml
groups:
  - name: critical
    rules:
      - alert: NoDataIngestion
        expr: rate(ingestion_records_total[10m]) == 0
        for: 15m
        labels:
          severity: critical
        annotations:
          summary: "No data ingested for 15 minutes"
          description: "Stream {{ $labels.stream_id }} has not received any data"

      - alert: TimescaleDBDown
        expr: up{job="timescaledb"} == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "TimescaleDB is down"

      - alert: StorageWriteFailures
        expr: rate(storage_write_errors_total[5m]) > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High rate of storage write failures"
```

**Warning Alerts** (Slack):
```yaml
# File: docker/production/configs/prometheus/warning_alerts.yml
groups:
  - name: warnings
    rules:
      - alert: HighIngestionLatency
        expr: histogram_quantile(0.95, rate(ingestion_latency_seconds_bucket[5m])) > 5
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Ingestion latency > 5 seconds"

      - alert: HighValidationErrorRate
        expr: rate(validation_errors_total[5m]) / rate(ingestion_records_total[5m]) > 0.05
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Validation error rate > 5%"

      - alert: DiskSpaceRunningLow
        expr: node_filesystem_avail_bytes{mountpoint="/data"} / node_filesystem_size_bytes < 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Less than 10% disk space available"
```

---

#### Monitoring Dashboard

**Key Panels for Grafana**:

1. **Ingestion Overview**
   - Line chart: Records/sec by stream
   - Gauge: Current ingestion rate
   - Table: Latest record timestamps per stream

2. **Storage Performance**
   - Heatmap: Write latency distribution
   - Time series: Bronze vs Silver write duration
   - Stat: Compression ratios

3. **Source Health**
   - Stat panels: Up/down status per source
   - Line chart: MQTT reconnection events
   - Table: Last successful fetch by HTTP sources

4. **Data Quality**
   - Line chart: Validation error rate
   - Table: Recent validation errors with details
   - Pie chart: Error types distribution

5. **Database Health**
   - Time series: Query latency
   - Bar chart: Table sizes
   - Line chart: Chunk creation rate

**Dashboard JSON**: See Section 6.5 for full dashboard definition

---

### 4.3 Troubleshooting Common Issues

#### Issue 1: No Data Ingested for a Stream

**Symptoms**:
- Stream shows 0 records in last 5 minutes
- Alert: `NoDataIngestion`

**Diagnosis**:
```bash
# 1. Check if stream is enabled
etcdctl get /streams/air-quality/config | grep enabled

# 2. Check if source is running
docker logs air-quality-app | grep "air-quality" | tail -20

# 3. Check MQTT broker connectivity (if MQTT source)
mosquitto_sub -h localhost -t "airgradient/#" -v

# 4. Check source health endpoint
curl http://localhost:8080/api/v2/streams/air-quality/sources | jq
```

**Common Causes**:
1. **Stream disabled in config**: Enable via `etcdctl put /streams/air-quality/config/enabled "true"`
2. **MQTT broker unreachable**: Check network, restart mosquitto
3. **Invalid credentials**: Verify API keys in etcd
4. **Source crash**: Check logs for panic/error, restart app

**Resolution**:
```bash
# Restart source (hot-reload)
etcdctl put /streams/air-quality/sources/mqtt-airgradient/restart "true"

# Or restart entire app
docker restart air-quality-app

# Verify data flowing
watch -n 2 'psql -h localhost -U postgres -d neural_data -t -c "SELECT COUNT(*) FROM air_quality WHERE timestamp > NOW() - INTERVAL \"1 minute\";"'
```

---

#### Issue 2: High Validation Error Rate

**Symptoms**:
- Alert: `HighValidationErrorRate`
- Logs show schema validation errors

**Diagnosis**:
```bash
# 1. Check recent validation errors
docker logs air-quality-app | grep "validation error" | tail -20

# 2. Query error metrics
curl -s http://localhost:9090/api/v1/query?query=validation_errors_total | jq

# 3. Get sample invalid record
docker logs air-quality-app | grep "invalid record" -A 5
```

**Common Causes**:
1. **Schema changed upstream**: Sensor firmware updated, new fields added
2. **Invalid data types**: String sent instead of number
3. **Missing required fields**: Incomplete MQTT messages
4. **Out-of-range values**: Temperature reading of 200°C (faulty sensor)

**Resolution**:
```bash
# Option 1: Update schema to accept new fields
# Edit config/streams/air-quality/schema.yaml
# Add new field or make field nullable
./scripts/load-stream-configs.sh

# Option 2: Add data transformation rule
etcdctl put /streams/air-quality/sources/mqtt-airgradient/transform "filter_invalid_temps"

# Option 3: Investigate and fix upstream data source
# Contact sensor vendor, update firmware
```

---

#### Issue 3: TimescaleDB Write Timeouts

**Symptoms**:
- Alert: `StorageWriteFailures`
- Logs: `timeout writing to TimescaleDB`

**Diagnosis**:
```bash
# 1. Check database load
psql -h localhost -U postgres -d neural_data -c "
    SELECT
        datname,
        numbackends,
        xact_commit,
        xact_rollback,
        blks_read,
        blks_hit
    FROM pg_stat_database
    WHERE datname = 'neural_data';
"

# 2. Check active queries
psql -h localhost -U postgres -d neural_data -c "
    SELECT
        pid,
        now() - query_start as duration,
        state,
        query
    FROM pg_stat_activity
    WHERE state != 'idle'
    ORDER BY duration DESC;
"

# 3. Check table bloat
psql -h localhost -U postgres -d neural_data -c "
    SELECT
        schemaname,
        tablename,
        pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
    FROM pg_tables
    WHERE schemaname = 'public'
    ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
"
```

**Common Causes**:
1. **High concurrent writes**: Too many streams writing simultaneously
2. **Uncompressed chunks**: Compression policy not running
3. **Missing indexes**: Slow queries blocking writes
4. **Insufficient resources**: CPU/memory exhaustion

**Resolution**:
```bash
# 1. Increase batch timeout to reduce write frequency
etcdctl put /config/air-quality-app/storage/batch_timeout_secs "10"

# 2. Run vacuum and analyze
psql -h localhost -U postgres -d neural_data -c "VACUUM ANALYZE air_quality;"

# 3. Manually trigger compression
psql -h localhost -U postgres -d neural_data -c "
    CALL run_job((SELECT job_id FROM timescaledb_information.jobs WHERE proc_name = 'policy_compression'));
"

# 4. Scale up database resources (if persistent)
# Edit docker-compose.yml: increase memory/CPU limits
docker compose up -d timescaledb
```

---

#### Issue 4: Parquet Write Lag

**Symptoms**:
- Bronze layer has fewer records than Silver layer
- Alert: `BronzeLayerLag`

**Diagnosis**:
```bash
# 1. Compare record counts
bronze_count=$(./scripts/count-parquet-records.sh air-quality)
silver_count=$(psql -h localhost -U postgres -d neural_data -t -c "SELECT COUNT(*) FROM air_quality;")
echo "Bronze: $bronze_count, Silver: $silver_count, Lag: $((silver_count - bronze_count))"

# 2. Check disk I/O
iostat -x 5 3

# 3. Check Parquet write errors
docker logs air-quality-app | grep "parquet write" | grep -i error
```

**Common Causes**:
1. **Slow disk**: Network storage latency
2. **Batch size too small**: Frequent small writes
3. **WAL replay failure**: Write-ahead log corruption

**Resolution**:
```bash
# 1. Increase batch size
etcdctl put /config/air-quality-app/storage/batch_size "500"

# 2. Tune Parquet compression
# Edit code to use SNAPPY compression instead of GZIP (faster)

# 3. Backfill from Silver to Bronze (if lag critical)
./scripts/silver-to-bronze-backfill.sh \
    --stream air-quality \
    --start-date "2025-12-14"
```

---

#### Issue 5: Stream Hot-Reload Not Working

**Symptoms**:
- Config changed in etcd, but app doesn't react
- No log message about config update

**Diagnosis**:
```bash
# 1. Check etcd watch connection
docker logs air-quality-app | grep "etcd watch"

# 2. Test etcd connectivity
docker exec air-quality-app etcdctl --endpoints=http://etcd:2379 endpoint health

# 3. Verify watch key prefix
etcdctl get /streams/ --prefix --keys-only
```

**Common Causes**:
1. **etcd watch connection dropped**: Network hiccup
2. **Watch key prefix mismatch**: Watching wrong prefix
3. **Config reload panic**: Exception during reload

**Resolution**:
```bash
# 1. Restart app to re-establish watch
docker restart air-quality-app

# 2. Check for watch re-connection in logs
docker logs -f air-quality-app | grep "watch"

# 3. Manual reload via API (if implemented)
curl -X POST http://localhost:8080/api/v2/admin/reload-config
```

---

### 4.4 Maintenance Procedures

#### Monthly Maintenance

**Tasks**:
1. Review storage growth trends
2. Verify compression policies running
3. Check retention policy effectiveness
4. Update stream schemas if needed
5. Review alert fatigue (tune thresholds)

**Script**: `/workspaces/neural-data-platform/scripts/monthly-maintenance.sh`

```bash
#!/bin/bash
# Monthly maintenance tasks

echo "=== Monthly Maintenance: $(date) ==="

# 1. Storage report
echo "## Storage Report"
psql -h localhost -U postgres -d neural_data -c "
    SELECT
        schemaname || '.' || tablename AS table,
        pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS total_size,
        pg_size_pretty(pg_relation_size(schemaname||'.'||tablename)) AS table_size,
        pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename) - pg_relation_size(schemaname||'.'||tablename)) AS index_size
    FROM pg_tables
    WHERE schemaname = 'public'
    ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
"

# 2. Compression report
echo "## Compression Report"
psql -h localhost -U postgres -d neural_data -c "
    SELECT
        hypertable_name,
        before_compression_total_bytes,
        after_compression_total_bytes,
        compression_ratio
    FROM timescaledb_information.compression_settings;
"

# 3. Alert summary
echo "## Alert Summary (Last 30 Days)"
# Query Prometheus alert history
# ... implementation ...

# 4. Backup verification
echo "## Backup Verification"
ls -lh backups/timescaledb/ | tail -10

echo "=== Maintenance Complete ==="
```

---

#### Quarterly Maintenance

**Tasks**:
1. Archive old Bronze layer data to cold storage
2. Review and optimize continuous aggregates
3. Update Grafana dashboards
4. Conduct disaster recovery drill
5. Review capacity planning

---

## 5. Future Extensions

### 5.1 Neural Predictions Integration Point

**Objective**: Enable cross-stream predictive models (e.g., predict CO2 spike based on window state + weather)

**Integration Point**: `/workspaces/neural-data-platform/core/src/prediction/mod.rs`

```rust
// Future implementation
pub struct MultiStreamPredictor {
    bronze_reader: Arc<ParquetStore>,
    silver_reader: Arc<TimescaleAdapter>,
    model_registry: Arc<ModelRegistry>,
}

impl MultiStreamPredictor {
    /// Train model using multiple streams as features
    pub async fn train_cross_stream(
        &mut self,
        target_stream: &str,
        feature_streams: Vec<&str>,
        horizon: Duration,
    ) -> Result<ModelMetrics> {
        // 1. Load Bronze data for all streams
        // 2. Join streams by timestamp (ASOF join)
        // 3. Feature engineering
        // 4. Train model (XGBoost, LSTM, etc.)
        // 5. Store model in model registry
        todo!()
    }

    /// Run inference using latest data from Silver layer
    pub async fn predict(
        &self,
        model_id: &str,
        horizon: Duration,
    ) -> Result<Vec<PredictedPoint>> {
        // 1. Query recent window from TimescaleDB
        // 2. Load model from registry
        // 3. Run inference
        // 4. Publish predictions to MQTT
        todo!()
    }
}
```

**Use Case Example**:
```yaml
# Model definition in etcd: /models/co2-spike-predictor
model_id: co2-spike-predictor
type: classification
target:
  stream: air-quality
  field: co2
  threshold: 1200  # Predict if CO2 will exceed 1200 ppm
features:
  - stream: air-quality
    fields: [co2, temperature, humidity]
    window: 1h
  - stream: home-events
    fields: [window_state, cooking_state]
    aggregation: last
  - stream: weather
    fields: [temperature, pressure]
    window: 1h
horizon: 30m  # Predict 30 minutes ahead
training:
  bronze_history_days: 90
  retrain_interval: 7d
inference:
  interval: 5m
  publish_topic: predictions/co2-spike
```

**Benefits**:
- Cross-stream correlation learning
- Proactive alerting (predict issues before they happen)
- Homebridge integration (auto-open window if CO2 spike predicted)

---

### 5.2 Trigger/Action System Hooks

**Objective**: Define rules to trigger actions based on stream data patterns

**Architecture**:
```
Stream Data → Rule Engine → Action Executor → External Systems
                  ↓
          (etcd rules registry)
```

**Rule Definition** (etcd: `/triggers/co2-ventilation`):
```yaml
trigger_id: co2-ventilation
description: Auto-ventilation when CO2 high and weather suitable
enabled: true

condition:
  type: and
  rules:
    - stream: air-quality
      field: co2
      operator: '>'
      value: 1200
      duration: 5m  # Must be high for 5 minutes

    - stream: weather
      field: temperature
      operator: between
      value: [15, 25]  # Outdoor temp comfortable

    - stream: home-events
      field: window_state
      operator: '=='
      value: closed

actions:
  - type: homebridge
    device: bedroom_window_actuator
    command: open
    params:
      percentage: 50

  - type: notification
    channel: slack
    message: "Auto-opened bedroom window due to high CO2 ({{ air_quality.co2 }} ppm)"

  - type: mqtt
    topic: home/commands/ventilation
    payload:
      action: start
      mode: auto

cooldown: 30m  # Don't trigger again for 30 minutes
```

**Implementation Sketch**:
```rust
// Location: core/src/triggers/mod.rs
pub struct TriggerEngine {
    registry: Arc<TriggerRegistry>,
    silver: Arc<TimescaleAdapter>,
    executors: HashMap<String, Box<dyn ActionExecutor>>,
}

impl TriggerEngine {
    pub async fn evaluate_triggers(&self) -> Result<()> {
        // 1. Load active triggers from etcd
        // 2. Query TimescaleDB for latest data
        // 3. Evaluate conditions
        // 4. Execute actions if triggered
        // 5. Update cooldown state
        todo!()
    }
}

#[async_trait]
pub trait ActionExecutor: Send + Sync {
    async fn execute(&self, params: ActionParams) -> Result<()>;
}

pub struct HomebridgeExecutor {
    base_url: String,
    auth_token: String,
}

#[async_trait]
impl ActionExecutor for HomebridgeExecutor {
    async fn execute(&self, params: ActionParams) -> Result<()> {
        // Call Homebridge API
        todo!()
    }
}
```

**Benefits**:
- Automation without external scripts
- Declarative rule definition
- Cross-stream logic (air quality + weather)
- Homebridge integration for smart home control

---

### 5.3 Self-Learning Layer Foundation

**Objective**: Platform learns optimal configurations based on historical patterns

**Capabilities**:

1. **Auto-Tuning Storage Policies**
   - Learn compression timing based on query patterns
   - Adjust retention based on data access frequency
   - Optimize continuous aggregate intervals

2. **Adaptive Source Configuration**
   - Increase polling frequency during high-variance periods
   - Decrease during stable periods (save API calls)
   - Auto-disable faulty sources

3. **Anomaly Detection**
   - Learn normal ranges for each stream
   - Flag outliers automatically
   - Suggest schema updates (e.g., expand range)

**Implementation Approach**:
```rust
// Location: core/src/learning/mod.rs
pub struct AdaptivePlatform {
    bronze: Arc<ParquetStore>,
    silver: Arc<TimescaleAdapter>,
    registry: Arc<StreamRegistry>,
    learner: Arc<dyn PlatformLearner>,
}

impl AdaptivePlatform {
    /// Analyze historical patterns and suggest optimizations
    pub async fn analyze_and_optimize(&self) -> Result<Vec<Optimization>> {
        let optimizations = vec![];

        // 1. Analyze query patterns from pg_stat_statements
        // 2. Suggest index additions
        // 3. Analyze compression ratios
        // 4. Suggest policy adjustments

        Ok(optimizations)
    }

    /// Apply approved optimizations
    pub async fn apply_optimization(&self, opt: Optimization) -> Result<()> {
        match opt.kind {
            OptimizationKind::AddIndex => {
                // CREATE INDEX ...
            },
            OptimizationKind::AdjustCompression => {
                // ALTER POLICY ...
            },
            OptimizationKind::UpdateRetention => {
                // Modify etcd config
            },
        }
        Ok(())
    }
}
```

**Example Learning Scenarios**:

- **Learned**: Air quality stream queried mostly for last 24 hours
  - **Action**: Create continuous aggregate for 1-hour rollups, keep detailed data for 24h only

- **Learned**: Weather stream has 99.9% uptime, rarely fails
  - **Action**: Reduce health check frequency from 30s to 5m

- **Learned**: Home events stream has bursty writes (10 events/sec for 1 minute, then 0 for hours)
  - **Action**: Adjust batch timeout dynamically based on traffic

**Benefits**:
- Reduced operational overhead
- Automatic performance optimization
- Cost savings (fewer API calls, optimized storage)

---

## 6. Documentation Deliverables

### 6.1 API Documentation

**API Reference**: `/workspaces/neural-data-platform/docs/api/multi-stream-api.md`

#### Stream Registry Endpoints

**GET /api/v2/streams**

List all registered streams.

**Request**:
```bash
curl http://localhost:8080/api/v2/streams
```

**Response**:
```json
{
  "streams": [
    {
      "stream_id": "air-quality",
      "description": "Indoor air quality measurements",
      "enabled": true,
      "retention_days": 365,
      "compression_after_days": 7,
      "sources": [
        {
          "id": "mqtt-airgradient",
          "type": "mqtt",
          "status": "healthy"
        }
      ],
      "stats": {
        "last_record_timestamp": "2025-12-15T12:34:56Z",
        "record_count_24h": 28800,
        "storage_size_bytes": 2147483648
      }
    },
    {
      "stream_id": "home-events",
      "description": "Discrete home activity events",
      "enabled": true,
      "retention_days": 730,
      "sources": [
        {
          "id": "mqtt-home-events",
          "type": "mqtt",
          "status": "healthy"
        },
        {
          "id": "webhook-manual",
          "type": "webhook",
          "status": "healthy"
        }
      ],
      "stats": {
        "last_record_timestamp": "2025-12-15T12:30:00Z",
        "record_count_24h": 45,
        "storage_size_bytes": 524288
      }
    }
  ],
  "total": 2
}
```

---

**GET /api/v2/streams/{stream_id}**

Get details for a specific stream.

**Request**:
```bash
curl http://localhost:8080/api/v2/streams/air-quality
```

**Response**:
```json
{
  "stream_id": "air-quality",
  "description": "Indoor air quality measurements",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 7,
  "schema": {
    "fields": [
      {
        "name": "pm25",
        "type": "float",
        "unit": "µg/m³",
        "nullable": false,
        "range": [0, 1000]
      },
      {
        "name": "co2",
        "type": "int",
        "unit": "ppm",
        "nullable": false,
        "range": [400, 5000]
      }
    ]
  },
  "sources": [
    {
      "id": "mqtt-airgradient",
      "type": "mqtt",
      "config": {
        "topic": "airgradient/+/measures",
        "qos": 1
      },
      "status": "healthy",
      "last_health_check": "2025-12-15T12:35:00Z",
      "stats": {
        "messages_received_24h": 28800,
        "errors_24h": 0
      }
    }
  ]
}
```

---

**POST /api/v2/streams**

Register a new stream (admin only).

**Request**:
```bash
curl -X POST http://localhost:8080/api/v2/streams \
  -H "Content-Type: application/json" \
  -d '{
    "stream_id": "energy",
    "description": "Household energy consumption",
    "retention_days": 730,
    "compression_after_days": 30,
    "schema": {
      "fields": [
        {
          "name": "kwh",
          "type": "float",
          "unit": "kilowatt-hours",
          "nullable": false
        }
      ]
    },
    "sources": [
      {
        "type": "mqtt",
        "config": {
          "topic": "energy/monitor/+/readings",
          "qos": 1
        }
      }
    ]
  }'
```

**Response**:
```json
{
  "stream_id": "energy",
  "status": "created",
  "message": "Stream registered successfully. DDL migration required."
}
```

---

**POST /api/v2/events** (Webhook Ingestion)

Ingest event data via webhook.

**Request**:
```bash
curl -X POST http://localhost:8080/api/v2/events \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{
    "stream_id": "home-events",
    "timestamp": "2025-12-15T12:00:00Z",
    "data": {
      "event_type": "window_state",
      "target": "bedroom",
      "state": "open"
    }
  }'
```

**Response**:
```json
{
  "status": "accepted",
  "record_id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2025-12-15T12:00:00Z"
}
```

---

#### Query Endpoints

**GET /api/v2/streams/{stream_id}/data**

Query stream data with filters.

**Request**:
```bash
curl "http://localhost:8080/api/v2/streams/air-quality/data?start=2025-12-15T00:00:00Z&end=2025-12-15T23:59:59Z&location_id=living-room&fields=pm25,co2&limit=100"
```

**Query Parameters**:
- `start` (required): Start timestamp (ISO 8601)
- `end` (required): End timestamp (ISO 8601)
- `location_id` (optional): Filter by location
- `fields` (optional): Comma-separated field list
- `limit` (optional): Max records to return (default: 1000)
- `aggregation` (optional): Aggregation interval (5m, 1h, 1d)

**Response**:
```json
{
  "stream_id": "air-quality",
  "data": [
    {
      "timestamp": "2025-12-15T12:00:00Z",
      "location_id": "living-room",
      "pm25": 12.3,
      "co2": 850
    },
    {
      "timestamp": "2025-12-15T12:05:00Z",
      "location_id": "living-room",
      "pm25": 11.8,
      "co2": 870
    }
  ],
  "total": 2,
  "aggregation": null
}
```

---

**GET /api/v2/streams/{stream_id}/stats**

Get stream statistics.

**Request**:
```bash
curl "http://localhost:8080/api/v2/streams/air-quality/stats?start=2025-12-15T00:00:00Z&end=2025-12-15T23:59:59Z"
```

**Response**:
```json
{
  "stream_id": "air-quality",
  "period": {
    "start": "2025-12-15T00:00:00Z",
    "end": "2025-12-15T23:59:59Z"
  },
  "record_count": 28800,
  "fields": {
    "pm25": {
      "avg": 15.2,
      "min": 8.1,
      "max": 45.3,
      "stddev": 7.8
    },
    "co2": {
      "avg": 920,
      "min": 450,
      "max": 1850,
      "stddev": 250
    }
  }
}
```

---

### 6.2 Configuration Reference

**Configuration File Structure**: `/workspaces/neural-data-platform/docs/config/streams-reference.md`

#### Stream Configuration

**File**: `config/streams/{stream-id}/config.yaml`

```yaml
# Stream identifier (unique, lowercase, hyphen-separated)
stream_id: air-quality

# Human-readable description
description: Indoor air quality measurements from AirGradient sensors

# Data retention (days) - applies to TimescaleDB
retention_days: 365

# Compression policy (days) - compress chunks older than this
compression_after_days: 7

# Enable/disable stream ingestion
enabled: true

# Optional: Tags for organization
tags:
  - indoor
  - environmental
  - sensor

# Optional: Alert thresholds
alerts:
  no_data_duration: 15m  # Alert if no data for this long
  validation_error_rate: 0.05  # Alert if >5% validation errors
```

---

#### Stream Schema

**File**: `config/streams/{stream-id}/schema.yaml`

```yaml
# Field definitions
fields:
  - name: pm25
    type: float  # Supported: int, float, string, boolean, json
    unit: µg/m³  # Optional: human-readable unit
    nullable: false  # Can field be null?
    range: [0, 1000]  # Optional: valid range for validation
    description: Particulate matter 2.5 µm concentration

  - name: co2
    type: int
    unit: ppm
    nullable: false
    range: [400, 5000]
    description: Carbon dioxide concentration

  - name: temperature
    type: float
    unit: celsius
    nullable: true
    range: [-50, 100]
    description: Ambient temperature

  - name: metadata
    type: json
    nullable: true
    description: Additional sensor-specific metadata

# Optional: Composite indexes for TimescaleDB
indexes:
  - fields: [location_id, timestamp]
    order: desc
  - fields: [co2, timestamp]
    order: desc
    where: co2 > 1000  # Partial index

# Optional: Continuous aggregates
aggregates:
  - name: mv_air_quality_5min
    interval: 5m
    fields:
      - field: pm25
        functions: [avg, max, min]
      - field: co2
        functions: [avg, max]
```

---

#### Stream Sources

**File**: `config/streams/{stream-id}/sources.yaml`

```yaml
sources:
  # MQTT Source Example
  - id: mqtt-airgradient
    type: mqtt
    enabled: true
    config:
      topic: airgradient/+/measures
      qos: 1  # 0, 1, or 2
      retain: false
      # Optional: Message transformation
      transform: airgradient_v1  # Reference to transform function

    # Optional: Health check override
    health_check:
      interval: 30s
      timeout: 5s

  # HTTP Polling Source Example
  - id: http-weather-api
    type: http_poll
    enabled: true
    config:
      url: https://api.openweathermap.org/data/2.5/weather
      method: GET
      interval: 5m
      timeout: 10s
      auth:
        type: api_key
        key_param: appid  # Query parameter name
        key_env: OPENWEATHERMAP_API_KEY  # Environment variable
      # Optional: Response transformation
      transform: openweathermap_current

  # Webhook Source Example
  - id: webhook-manual-events
    type: webhook
    enabled: true
    config:
      path: /api/events  # Endpoint path
      auth:
        type: bearer
        token_env: WEBHOOK_TOKEN
      # Optional: Rate limiting
      rate_limit:
        requests_per_minute: 60
```

---

### 6.3 Stream Schema Examples

**Example 1: Air Quality**

```yaml
# config/streams/air-quality/schema.yaml
fields:
  - name: pm25
    type: float
    unit: µg/m³
    nullable: false
    range: [0, 1000]

  - name: pm10
    type: float
    unit: µg/m³
    nullable: true
    range: [0, 1000]

  - name: co2
    type: int
    unit: ppm
    nullable: false
    range: [400, 5000]

  - name: voc
    type: int
    unit: index
    nullable: true
    range: [0, 500]

  - name: temperature
    type: float
    unit: celsius
    nullable: true
    range: [-50, 100]

  - name: humidity
    type: float
    unit: percent
    nullable: true
    range: [0, 100]
```

**Example 2: Home Events**

```yaml
# config/streams/home-events/schema.yaml
fields:
  - name: event_type
    type: string
    nullable: false
    enum: [window_state, door_state, motion, cooking, shower]

  - name: target
    type: string
    nullable: false
    description: Target of event (e.g., bedroom, kitchen)

  - name: state
    type: string
    nullable: true
    description: New state (e.g., open, closed, on, off)

  - name: metadata
    type: json
    nullable: true
    description: Event-specific data
```

**Example 3: Weather**

```yaml
# config/streams/weather/schema.yaml
fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
    range: [-50, 50]

  - name: humidity
    type: float
    unit: percent
    nullable: false
    range: [0, 100]

  - name: pressure
    type: float
    unit: hPa
    nullable: true
    range: [900, 1100]

  - name: wind_speed
    type: float
    unit: m/s
    nullable: true
    range: [0, 50]

  - name: wind_direction
    type: int
    unit: degrees
    nullable: true
    range: [0, 360]

  - name: conditions
    type: string
    nullable: true
    description: Weather description (e.g., clear, rain, snow)

  - name: visibility
    type: float
    unit: km
    nullable: true
    range: [0, 100]
```

**Example 4: Energy Consumption**

```yaml
# config/streams/energy/schema.yaml
fields:
  - name: kwh
    type: float
    unit: kilowatt-hours
    nullable: false
    range: [0, 100]

  - name: voltage
    type: float
    unit: volts
    nullable: true
    range: [110, 250]

  - name: current
    type: float
    unit: amperes
    nullable: true
    range: [0, 100]

  - name: power_factor
    type: float
    unit: ratio
    nullable: true
    range: [0, 1]

  - name: circuit
    type: string
    nullable: false
    description: Circuit identifier
```

---

### 6.4 Operational Runbook Summary

**Quick Reference Card**: `/workspaces/neural-data-platform/docs/operations/quick-reference.md`

#### Common Operations

| Task | Command |
|------|---------|
| List streams | `curl http://localhost:8080/api/v2/streams \| jq` |
| Health check | `./scripts/health-check.sh` |
| Add stream | `./scripts/add-stream.sh <stream-id>` |
| Reload config | `docker restart air-quality-app` |
| Check ingestion | `docker logs -f air-quality-app \| grep ingestion` |
| Query stream data | `psql -h localhost -U postgres -d neural_data -c "SELECT * FROM air_quality LIMIT 10;"` |
| Backup database | `./scripts/backup-timescaledb.sh` |
| Check metrics | `curl http://localhost:9090/metrics \| grep ingestion` |

#### Troubleshooting Flowchart

```
No data ingested?
│
├─ Is stream enabled? → etcdctl get /streams/<id>/config
│  └─ No → Enable it
│
├─ Is source healthy? → curl localhost:8080/api/v2/streams/<id>/sources
│  └─ No → Check logs, restart
│
├─ Validation errors? → docker logs air-quality-app | grep validation
│  └─ Yes → Fix schema or data source
│
└─ Database issue? → psql ... (check connectivity)
```

---

### 6.5 Grafana Dashboard Template

**File**: `/workspaces/neural-data-platform/docker/production/configs/grafana/dashboards/streams/multi_stream_overview.json`

```json
{
  "dashboard": {
    "title": "Multi-Stream Platform Overview",
    "tags": ["streams", "platform"],
    "timezone": "browser",
    "panels": [
      {
        "title": "Ingestion Rate (records/sec)",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(ingestion_records_total[5m])",
            "legendFormat": "{{ stream_id }}"
          }
        ],
        "yaxes": [
          {"label": "records/sec", "format": "short"}
        ]
      },
      {
        "title": "Stream Health",
        "type": "stat",
        "targets": [
          {
            "expr": "source_health",
            "legendFormat": "{{ stream_id }}"
          }
        ],
        "thresholds": [
          {"value": 0, "color": "red"},
          {"value": 1, "color": "green"}
        ]
      },
      {
        "title": "Storage Write Latency (p95)",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(storage_write_duration_seconds_bucket[5m]))",
            "legendFormat": "{{ layer }}/{{ stream_id }}"
          }
        ]
      },
      {
        "title": "Validation Error Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(validation_errors_total[5m]) / rate(ingestion_records_total[5m])",
            "legendFormat": "{{ stream_id }}"
          }
        ]
      },
      {
        "title": "Latest Records per Stream",
        "type": "table",
        "targets": [
          {
            "rawSql": "SELECT * FROM stream_health ORDER BY stream_id;",
            "format": "table"
          }
        ]
      },
      {
        "title": "Storage Size by Stream",
        "type": "piechart",
        "targets": [
          {
            "expr": "storage_size_bytes{layer=\"bronze\"}",
            "legendFormat": "{{ stream_id }}"
          }
        ]
      }
    ],
    "time": {
      "from": "now-6h",
      "to": "now"
    },
    "refresh": "30s"
  }
}
```

---

## Appendix A: File Locations Reference

**Configuration Files**:
- `/workspaces/neural-data-platform/config/streams/{stream-id}/config.yaml`
- `/workspaces/neural-data-platform/config/streams/{stream-id}/schema.yaml`
- `/workspaces/neural-data-platform/config/streams/{stream-id}/sources.yaml`

**Code Components**:
- `/workspaces/neural-data-platform/core/src/registry/mod.rs` - Stream registry
- `/workspaces/neural-data-platform/core/src/sources/mod.rs` - Source trait
- `/workspaces/neural-data-platform/core/src/ingestion/coordinator.rs` - Coordinator
- `/workspaces/neural-data-platform/core/src/storage/timescale.rs` - TimescaleDB adapter
- `/workspaces/neural-data-platform/neural-core/src/storage/parquet.rs` - Parquet store

**Infrastructure**:
- `/workspaces/neural-data-platform/docker-compose.yml` - Development compose
- `/workspaces/neural-data-platform/docker/production/docker-compose.prod.yml` - Production compose
- `/workspaces/neural-data-platform/migrations/timescaledb/001_init_streams.sql` - DB schema

**Scripts**:
- `/workspaces/neural-data-platform/scripts/load-stream-configs.sh` - Load etcd configs
- `/workspaces/neural-data-platform/scripts/health-check.sh` - Platform health check
- `/workspaces/neural-data-platform/scripts/add-stream.sh` - Add new stream
- `/workspaces/neural-data-platform/scripts/backup-timescaledb.sh` - Backup database

**Monitoring**:
- `/workspaces/neural-data-platform/docker/production/configs/prometheus/stream_alerts.yml`
- `/workspaces/neural-data-platform/docker/production/configs/grafana/dashboards/streams/`

**Documentation**:
- `/workspaces/neural-data-platform/docs/api/multi-stream-api.md` - API reference
- `/workspaces/neural-data-platform/docs/config/streams-reference.md` - Config reference
- `/workspaces/neural-data-platform/docs/operations/quick-reference.md` - Operations guide

---

## Appendix B: Glossary

**Bronze Layer**: Raw, immutable data storage using Parquet files. Optimized for batch analytics and model training.

**Silver Layer**: Queryable, indexed data storage using TimescaleDB. Optimized for real-time queries and dashboards.

**Stream**: Independent data pipeline with its own schema, sources, and retention policy.

**Source**: Data ingestion endpoint (MQTT topic, HTTP API, webhook, etc.).

**Stream Registry**: etcd-based configuration store defining all streams, schemas, and sources.

**Ingestion Coordinator**: Central service that spawns sources and routes data to appropriate streams.

**Dual-Write**: Writing data to both Bronze (Parquet) and Silver (TimescaleDB) layers simultaneously.

**Hypertable**: TimescaleDB's time-partitioned table optimized for time-series data.

**Continuous Aggregate**: Materialized view in TimescaleDB that automatically maintains rollup data.

**ASOF JOIN**: TimescaleDB's time-based join for correlating streams with different sampling rates.

---

## Appendix C: Migration Checklist

**Pre-Migration**:
- [ ] Backup existing Parquet data
- [ ] Backup etcd configuration
- [ ] Document current air-quality-app behavior
- [ ] Test rollback procedure in staging

**Migration Steps**:
- [ ] Phase 1: Stream Registry (Week 1)
- [ ] Phase 2: Generic Sources (Week 1-2)
- [ ] Phase 3: Ingestion Coordinator (Week 2-3)
- [ ] Phase 4: Storage Layer (Week 3-4)
- [ ] Phase 5: Dashboards (Week 4-5)
- [ ] Phase 6: New Streams (Week 5-6)

**Post-Migration Validation**:
- [ ] All existing air-quality data accessible
- [ ] API endpoints respond correctly
- [ ] Grafana dashboards show data
- [ ] Prometheus alerts firing correctly
- [ ] No data loss during migration
- [ ] Performance metrics within SLA

**Success Criteria**:
- [ ] 3+ streams ingesting data
- [ ] <5s p95 latency for queries
- [ ] >99% uptime for 7 days
- [ ] Zero critical alerts
- [ ] Operations team trained

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-15 | SPARC Completion Agent | Initial version |

---

**End of Document**
