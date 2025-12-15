# AIR-004: Generic Multi-Stream Data Platform - SPARC Specification

## Document Information

- **Feature ID**: AIR-004
- **Version**: 1.2.0 (Pi Deployment Constraints)
- **Status**: Specification Phase - Aligned with Pi Production Deployment
- **Created**: 2025-12-15
- **Revised**: 2025-12-15
- **Author**: SPARC Specification Agent
- **Production Target**: Raspberry Pi 5 (Ubuntu 25.04 ARM64)
- **Deployment Path**: `/workspaces/neural-data-platform/deploy/pi/`
- **Related Documents**:
  - [Architecture Document](../architecture/PLATFORM_ARCHITECTURE.md)
  - AIR-001: Core platform foundation
  - AIR-002: Configuration management
  - AIR-003: etcd-based configuration with hot-reload

---

## 0. Current Implementation Baseline

### 0.1 What Works Now (MUST PRESERVE)

The neural-data-platform currently has a **WORKING air quality monitoring system** that MUST remain functional throughout AIR-004:

#### Proven MQTT Ingestion Pipeline
- **Component**: `MqttSource` (`core/src/sources/mqtt.rs`)
- **Features**:
  - Auto-reconnect with exponential backoff (1s → 30s max)
  - AirGradient sensor support (29 fields: pm02, co2, temperature, humidity, wifi, etc.)
  - Topic pattern subscription: `airgradient/readings/{SERIAL_NUMBER}` → `airgradient/readings/+`
  - Backpressure handling with bounded queues (1000 msg buffer)
  - Health monitoring and status reporting
- **Performance**: Sustained 1+ msg/sec, tested under production conditions

#### Proven Storage Pipeline
- **Component**: `ParquetStore` (`core/src/storage/parquet.rs`)
- **Features**:
  - Year/month/day partitioning: `data/{location}/year={YYYY}/month={MM}/day={DD}/readings.parquet`
  - Write-Ahead Log (WAL) for crash recovery
  - Batch writes (100 messages / 5s timeout)
  - Snappy compression
  - Query support with time-range filtering
  - Aggregation (mean, min, max, sum, count, median, percentile)
- **Performance**: 10k records/sec write throughput

#### Proven Configuration System
- **Component**: `config-client` library (260 LOC)
- **Features**:
  - etcd-backed typed configuration
  - Environment variable overrides (ENV_PREFIX_KEY pattern)
  - Watch mechanism with callbacks
  - Operations: get, set, delete, list, watch
- **Performance**: <10ms config reads

#### Current etcd Configuration Hierarchy
```
/air-quality/
├── server/
│   ├── host: "0.0.0.0"
│   ├── port: 3000
│   └── graceful_shutdown_timeout_secs: 30
├── mqtt/
│   ├── broker_url: "mqtt-broker"
│   ├── port: 1883
│   ├── client_id: "air-quality-client"
│   ├── topic_pattern: "airgradient/readings/{SERIAL_NUMBER}"
│   ├── qos: 1
│   ├── reconnect_delay_secs: 1
│   └── buffer_capacity: 1000
├── storage/
│   ├── base_path: "/app/data"
│   ├── wal_enabled: true
│   ├── batch_size: 100
│   └── batch_timeout_secs: 5
├── alerts/
│   ├── enabled: true
│   └── thresholds/...
└── logging/
    ├── level: "info"
    └── format: "json"
```

### 0.2 Partially Implemented (Code Exists, Not Integrated)

- **HTTP Polling Source**: `core/src/sources/http_poll.rs` exists but not integrated into main service
- **Alert Handlers**: Trait defined, no alerting logic implemented
- **Forecasting**: Mock implementation only

### 0.3 Not Yet Implemented

- **TimescaleDB Integration**: References exist, no working code
- **Stream Registry**: No multi-stream coordination
- **Schema Validation**: No runtime validation beyond type parsing
- **Cross-Stream Queries**: No infrastructure

### 0.4 Critical Constraints

**NON-NEGOTIABLE**: AIR-004 is an EXTENSION, not a REWRITE. Requirements:

1. **MQTT AirGradient ingestion MUST continue working** throughout implementation
2. **Existing etcd config keys MUST remain valid** (backward compatible)
3. **Current Parquet storage MUST remain queryable** (schema stability)
4. **No breaking changes to `/air-quality/*` etcd namespace**
5. **Performance MUST NOT regress** below current baselines:
   - Config reads: <10ms (current: <10ms)
   - MQTT ingestion: >1 msg/sec sustained (current: proven)
   - Storage writes: >1k records/sec (current: 10k+)

---

## 1. Problem Statement

### 1.1 Current State

The neural-data-platform currently supports a single, hardcoded data stream (air quality sensors via MQTT). This design limits the platform's utility and prevents it from serving its broader mission: correlating multiple data sources for predictive home analytics.

**Specific Limitations**:
- Cannot ingest data from multiple independent sources simultaneously
- Schema is hardcoded in application logic (not configuration-driven)
- Adding new data streams requires code changes and redeployment
- No unified storage strategy for heterogeneous data types
- Cross-stream analytics require manual data integration

### 1.2 Desired State

A generic, configuration-driven multi-stream data platform that:
- Ingests data from multiple heterogeneous sources (MQTT, HTTP polling, webhooks, file imports)
- Supports independent data streams with distinct schemas
- Enables real-time dashboards and historical analysis
- Provides foundation for cross-stream predictive analytics
- Maintains home-scale deployment simplicity

### 1.3 Success Vision

Users can add new data streams by updating etcd configuration (no code changes), visualize multi-stream data in unified dashboards, and build predictive models that correlate events across streams (e.g., "cooking events predict PM2.5 spikes 15 minutes later").

---

## 2. Functional Requirements

### FR-001: Stream Registry Management (EXTENDS EXISTING ETCD PATTERNS)

**Priority**: CRITICAL
**Category**: Configuration Management
**Implementation Note**: This EXTENDS the current `/air-quality/*` config pattern to support multiple streams

#### FR-001.1: Stream Definition
- **Requirement**: System SHALL support defining data streams in etcd configuration WHILE maintaining existing `/air-quality/*` keys
- **Acceptance Criteria**:
  - New namespace: `streams/{stream-id}/config` (e.g., `streams/weather/config`)
  - **BACKWARD COMPATIBILITY**: `/air-quality/*` keys remain valid and map to implicit `streams/air-quality/*`
  - Each stream has unique identifier (slug format: `^[a-z0-9-]+$`)
  - Configuration includes: description, retention_days, compression_after_days
  - Stream configuration validates on write
  - **MIGRATION PATH**: Existing `/air-quality/*` continues working, new streams use `streams/*`
- **Test Scenario**:
  ```yaml
  GIVEN etcd has existing /air-quality/mqtt config
  WHEN user writes stream config to "streams/weather/config"
  THEN new config is persisted and validated
  AND coordinator detects new stream via watch API
  AND existing air-quality stream continues functioning
  AND both streams operate independently
  ```

#### FR-001.2: Schema Definition
- **Requirement**: System SHALL support field-level schema definition per stream
- **Acceptance Criteria**:
  - Schema stored under `streams/{stream-id}/schema`
  - Each field specifies: name, type, unit, nullable flag
  - Supported types: int, float, string, boolean, json, timestamp
  - Schema changes are versioned (v1, v2, etc.)
- **Test Scenario**:
  ```gherkin
  Feature: Schema Definition
    Scenario: Valid schema registration
      Given a stream "air-quality" exists
      When schema with fields [pm25:float, co2:int] is registered
      Then schema validation succeeds
      And schema version is incremented
  ```

#### FR-001.3: Source Configuration
- **Requirement**: System SHALL support configuring multiple sources per stream
- **Acceptance Criteria**:
  - Sources stored under `streams/{stream-id}/sources`
  - Each source specifies: type, connection parameters, authentication
  - Multiple sources can feed single stream
  - Source types: mqtt, http_poll, webhook, file_watch
- **Test Scenario**:
  ```gherkin
  Scenario: Multi-source stream
    Given stream "home-events" exists
    When sources [mqtt:home/events/#, webhook:/api/events] are configured
    Then both sources are activated
    And data from both sources flows to single stream
  ```

#### FR-001.4: Dynamic Reconfiguration
- **Requirement**: System SHALL reload configuration without restart when stream definitions change
- **Acceptance Criteria**:
  - Coordinator watches etcd keys: `streams/*/config`, `streams/*/schema`, `streams/*/sources`
  - Configuration changes trigger graceful source restart
  - In-flight data is not lost during reconfiguration
  - Reconfiguration completes within 5 seconds
- **Test Scenario**:
  ```gherkin
  Scenario: Hot-reload configuration
    Given stream "weather" is actively ingesting
    When user updates polling interval from 5m to 1m
    Then coordinator detects change within 1 second
    And source is gracefully restarted
    And no data loss occurs
  ```

---

### FR-002: Multi-Source Ingestion (BUILDS ON EXISTING MqttSource)

**Priority**: CRITICAL
**Category**: Data Ingestion
**Implementation Note**: START with existing working `MqttSource`, then ADD http_poll and webhook support

#### FR-002.1: MQTT Source Support (ALREADY IMPLEMENTED - PRESERVE)
- **Requirement**: System SHALL continue supporting MQTT ingestion with existing `MqttSource` implementation
- **Current Implementation**: `core/src/sources/mqtt.rs` (593 LOC, fully tested)
- **Acceptance Criteria** (ALREADY MET):
  - ✅ Supports MQTT 3.1.1 via rumqttc
  - ✅ Configurable QoS levels (0, 1, 2)
  - ✅ Topic wildcards supported (`+`, `#`)
  - ✅ Automatic reconnection with exponential backoff (1s → 30s)
  - ✅ AirGradient JSON payload parsing
  - ✅ Health monitoring and status reporting
- **Performance Target**: ✅ ACHIEVED - Sustained 1+ msg/sec in production
- **AIR-004 Extension**:
  - Generalize MqttSource to support non-AirGradient payloads
  - Make payload parser configurable per stream
  - Add to multi-stream coordinator (new component)
- **Test Scenario** (ALREADY PASSING):
  ```rust
  // Existing tests in mqtt.rs:
  // - test_mqtt_source_creation
  // - test_health_check_before_start
  // - test_parse_payload_success
  // - test_exponential_backoff_calculation
  // - test_fetch_returns_cached_points
  ```

#### FR-002.2: HTTP Polling Source Support (CODE EXISTS - INTEGRATE)
- **Requirement**: System SHALL poll HTTP endpoints at configurable intervals
- **Current Status**: `core/src/sources/http_poll.rs` exists (100+ LOC) but not integrated
- **Acceptance Criteria**:
  - ✅ PARTIAL: HttpPollingSource struct with reqwest client
  - ✅ PARTIAL: Timeout configuration support
  - ✅ PARTIAL: AirGradient measures endpoint parser
  - ⚠️ TODO: Integrate into multi-stream coordinator
  - ⚠️ TODO: Add retry logic with exponential backoff
  - ⚠️ TODO: Support non-AirGradient endpoints (configurable parser)
  - Configurable poll interval (minimum: 10 seconds)
  - Supports GET and POST methods
  - Response parsing: JSON (start), CSV (future)
  - Authentication: API key, Bearer token, Basic auth
  - Timeout: configurable (default 30 seconds)
- **Performance Target**: Support 20 concurrent pollers
- **Test Scenario**:
  ```gherkin
  Scenario: HTTP polling with authentication
    Given HTTP source polling "https://api.weather.com/data"
    And interval is "5m"
    And auth is API key
    When first poll executes
    Then request includes authentication header
    And response is parsed as JSON
    And data is routed to correct stream
  ```

#### FR-002.3: Webhook Handler Support
- **Requirement**: System SHALL expose HTTP endpoints for push-based data ingestion
- **Acceptance Criteria**:
  - REST API endpoint: `POST /api/streams/{stream-id}/events`
  - Request validation: schema conformance, authentication
  - Response: 202 Accepted (async processing)
  - Rate limiting: 1000 requests/minute per IP
  - Authentication: Bearer token, API key
- **Performance Target**: Handle 500 requests/second
- **Test Scenario**:
  ```bash
  curl -X POST http://localhost:3000/api/streams/home-events/events \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"event_type": "cooking_start", "target": "kitchen", "state": "active"}'

  # Expected: 202 Accepted
  # Expected: Event appears in stream within 1 second
  ```

#### FR-002.4: Source Health Monitoring
- **Requirement**: System SHALL track and report health status of all active sources
- **Acceptance Criteria**:
  - Health check endpoint: `GET /api/health/sources`
  - Per-source metrics: last_success, error_count, latency
  - Status values: healthy, degraded, unhealthy, disconnected
  - Automatic alerting on prolonged unhealthy state (>5 minutes)
- **Test Scenario**:
  ```gherkin
  Scenario: Detect unhealthy MQTT source
    Given MQTT source is connected
    When MQTT broker becomes unreachable
    Then source status changes to "disconnected"
    And error count increments
    And alert is generated after 5 minutes
  ```

---

### FR-003: Schema Validation and Metadata

**Priority**: HIGH
**Category**: Data Quality

#### FR-003.1: Ingestion-Time Validation
- **Requirement**: System SHALL validate all ingested data against stream schema before storage
- **Acceptance Criteria**:
  - Field type validation (int, float, string, etc.)
  - Nullable constraint enforcement
  - Range validation (if configured)
  - Validation failures logged with error details
  - Invalid records routed to dead-letter queue
- **Test Scenario**:
  ```gherkin
  Scenario: Invalid data rejected
    Given stream "air-quality" expects field "pm25" as float
    When message arrives with "pm25": "invalid"
    Then validation fails
    And record is sent to dead-letter queue
    And error is logged with details
  ```

#### FR-003.2: Field-Level Metadata
- **Requirement**: System SHALL support metadata annotations on schema fields
- **Acceptance Criteria**:
  - Metadata includes: unit, description, min, max, enum_values
  - Metadata stored in etcd alongside schema
  - Metadata accessible via API: `GET /api/streams/{stream-id}/schema`
  - Metadata used for dashboard generation
- **Example**:
  ```yaml
  fields:
    - name: pm25
      type: float
      unit: µg/m³
      description: Particulate matter 2.5 micrometers
      min: 0
      max: 500
      nullable: false
  ```

#### FR-003.3: Schema Evolution
- **Requirement**: System SHALL support backward-compatible schema changes
- **Acceptance Criteria**:
  - Adding nullable fields is allowed
  - Removing fields is disallowed (must deprecate)
  - Type changes are disallowed
  - Schema version increments on change
  - Old data remains queryable under old schema
- **Test Scenario**:
  ```gherkin
  Scenario: Add nullable field
    Given stream schema version 1 has fields [pm25, co2]
    When user adds nullable field "pm10"
    Then schema version increments to 2
    And new data includes pm10
    And old data shows pm10 as NULL
  ```

---

### FR-004: Bronze Layer Storage (Parquet)

**Priority**: CRITICAL
**Category**: Data Storage

#### FR-004.1: Parquet File Management
- **Requirement**: System SHALL write all ingested data to Parquet files for long-term storage
- **Acceptance Criteria**:
  - File path: `data/bronze/{stream-id}/year={YYYY}/month={MM}/day={DD}/{uuid}.parquet`
  - Partitioning: by stream_id, year, month, day
  - File rotation: new file every 1 hour OR 100MB, whichever comes first
  - Compression: Snappy codec
  - Schema stored in Parquet metadata
- **Performance Target**: Write throughput 10,000 records/second
- **Test Scenario**:
  ```rust
  #[test]
  fn parquet_file_partitioning() {
      let store = ParquetStore::new("data/bronze");
      let record = StreamRecord {
          stream_id: "air-quality".to_string(),
          timestamp: Utc.with_ymd_and_hms(2025, 12, 15, 10, 30, 0).unwrap(),
          data: json!({"pm25": 12.5}),
      };

      store.write(record).await.unwrap();

      let expected_path = "data/bronze/air-quality/year=2025/month=12/day=15/";
      assert!(Path::new(expected_path).exists());
  }
  ```

#### FR-004.2: Append-Only Guarantee
- **Requirement**: System SHALL never modify or delete existing Parquet files
- **Acceptance Criteria**:
  - All writes are append operations
  - File immutability enforced by filesystem permissions
  - Retention policy implemented as separate archival process
  - File integrity verified via checksums
- **Test Scenario**:
  ```gherkin
  Scenario: Immutability enforcement
    Given Parquet file written at 2025-12-15 10:00
    When system attempts to modify file
    Then operation fails with permission error
  ```

#### FR-004.3: Query Interface
- **Requirement**: System SHALL support querying Bronze layer via Polars/DuckDB
- **Acceptance Criteria**:
  - API endpoint: `GET /api/query/bronze?stream={id}&start={ts}&end={ts}`
  - Predicate pushdown for date filtering
  - Schema discovery from Parquet metadata
  - Result format: JSON, CSV, Arrow IPC
- **Performance Target**: Query 1 million records in <5 seconds
- **Test Scenario**:
  ```sql
  -- Example query via DuckDB
  SELECT
    stream_id,
    AVG(pm25) as avg_pm25
  FROM read_parquet('data/bronze/air-quality/year=2025/month=12/day=*/*.parquet')
  WHERE timestamp BETWEEN '2025-12-15' AND '2025-12-16'
  GROUP BY stream_id;
  ```

---

### FR-005: Silver/Gold Layer Storage (TimescaleDB)

**Priority**: CRITICAL
**Category**: Data Storage

#### FR-005.1: Hypertable Management
- **Requirement**: System SHALL create TimescaleDB hypertable per stream automatically
- **Acceptance Criteria**:
  - Table naming: `{stream_id}` (e.g., `air_quality`, `home_events`)
  - Hypertable chunking: 1 day intervals
  - Primary key: `(timestamp, stream_id)` for multi-tenancy
  - Indexes: timestamp (BRIN), stream_id (hash)
  - Auto-creation on first ingestion to new stream
- **DDL Generation**:
  ```sql
  CREATE TABLE air_quality (
    timestamp TIMESTAMPTZ NOT NULL,
    stream_id TEXT NOT NULL,
    pm25 DOUBLE PRECISION NOT NULL,
    pm10 DOUBLE PRECISION,
    co2 INTEGER NOT NULL,
    voc INTEGER,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION
  );

  SELECT create_hypertable('air_quality', 'timestamp',
    chunk_time_interval => INTERVAL '1 day');

  CREATE INDEX idx_air_quality_timestamp ON air_quality (timestamp DESC);
  ```

#### FR-005.2: Continuous Aggregates
- **Requirement**: System SHALL create automatic rollup tables for common time windows
- **Acceptance Criteria**:
  - Materialized views: 5min, 1hr, 1day aggregates
  - Aggregation functions: AVG, MIN, MAX, COUNT
  - Refresh policy: continuous (real-time)
  - View naming: `mv_{stream_id}_{interval}` (e.g., `mv_air_quality_5min`)
- **Example**:
  ```sql
  CREATE MATERIALIZED VIEW mv_air_quality_5min
  WITH (timescaledb.continuous) AS
  SELECT
    time_bucket('5 minutes', timestamp) AS bucket,
    stream_id,
    AVG(pm25) as avg_pm25,
    MIN(pm25) as min_pm25,
    MAX(pm25) as max_pm25,
    COUNT(*) as sample_count
  FROM air_quality
  GROUP BY bucket, stream_id;

  SELECT add_continuous_aggregate_policy('mv_air_quality_5min',
    start_offset => INTERVAL '1 hour',
    end_offset => INTERVAL '5 minutes',
    schedule_interval => INTERVAL '5 minutes');
  ```

#### FR-005.3: Compression and Retention
- **Requirement**: System SHALL compress old data and enforce retention policies
- **Acceptance Criteria**:
  - Compression policy: compress chunks older than 7 days
  - Compression algorithm: TimescaleDB native (columnar)
  - Retention policy: configurable per stream (default 365 days)
  - Policy enforcement: daily background job
- **Configuration**:
  ```sql
  SELECT add_compression_policy('air_quality', INTERVAL '7 days');
  SELECT add_retention_policy('air_quality', INTERVAL '365 days');
  ```

#### FR-005.4: Cross-Stream Queries (ASOF JOIN)
- **Requirement**: System SHALL support time-correlation queries across streams
- **Acceptance Criteria**:
  - TimescaleDB ASOF JOIN support enabled
  - Query API: `GET /api/query/cross-stream`
  - Maximum join skew: 5 minutes
- **Example Query**:
  ```sql
  SELECT
    aq.timestamp,
    aq.pm25,
    he.event_type
  FROM air_quality aq
  ASOF LEFT JOIN home_events he
    ON aq.timestamp >= he.timestamp
   AND aq.timestamp < he.timestamp + INTERVAL '5 minutes'
  WHERE aq.timestamp BETWEEN '2025-12-15 10:00' AND '2025-12-15 11:00'
    AND he.event_type = 'cooking_start';
  ```

---

### FR-006: Dual-Write Synchronization

**Priority**: HIGH
**Category**: Data Consistency

#### FR-006.1: Atomic Dual-Write
- **Requirement**: System SHALL write ingested data to both Bronze and Silver layers atomically
- **Acceptance Criteria**:
  - Write to Bronze (Parquet) first
  - Write to Silver (TimescaleDB) second
  - Rollback Bronze on Silver write failure (via tombstone file)
  - Transaction log for retry on partial failure
  - Maximum write latency: 100ms per record
- **Failure Handling**:
  ```gherkin
  Scenario: Silver write fails
    Given record written to Bronze successfully
    When TimescaleDB write fails due to connection error
    Then record is added to retry queue
    And retry occurs with exponential backoff
    And maximum 5 retry attempts before dead-letter
  ```

#### FR-006.2: Eventual Consistency Verification
- **Requirement**: System SHALL verify Bronze and Silver layers converge to consistency
- **Acceptance Criteria**:
  - Background reconciliation job runs every 1 hour
  - Compares record counts per stream per hour
  - Detects missing records in Silver layer
  - Auto-repair by backfilling from Bronze
- **Monitoring**:
  ```sql
  -- Consistency check query
  WITH bronze_count AS (
    SELECT COUNT(*) as cnt
    FROM read_parquet('data/bronze/air-quality/year=2025/month=12/day=15/*.parquet')
  ),
  silver_count AS (
    SELECT COUNT(*) as cnt
    FROM air_quality
    WHERE timestamp::date = '2025-12-15'
  )
  SELECT
    bronze_count.cnt - silver_count.cnt as missing_records
  FROM bronze_count, silver_count;
  ```

---

## 3. Non-Functional Requirements

### NFR-001: Performance (BASELINE: CURRENT PROVEN METRICS)

**Category**: System Performance
**Constraint**: AIR-004 MUST NOT regress below current baselines

#### NFR-001.1: Ingestion Throughput
- **BASELINE (Current)**: ParquetStore sustained 10k records/sec in tests
- **BASELINE (Production)**: MQTT ingestion sustained 1+ msg/sec continuously
- **Requirement**: System SHALL sustain 10,000 records/second aggregate ingestion rate
- **Acceptance**:
  - MUST maintain current 10k records/sec batch write capability
  - MUST maintain current 1+ msg/sec MQTT ingestion
  - Multi-stream overhead MUST NOT reduce single-stream throughput >10%
- **Measurement**: Prometheus metric `ingestion_records_per_second`
- **Test Method**:
  1. Baseline test: air-quality stream alone (must match current perf)
  2. Load test: 10 streams × 1,000 msgs/sec each
- **Acceptance**: p99 latency < 500ms, no regression vs current air-quality stream

#### NFR-001.2: Query Latency
- **BASELINE (Current)**: ParquetStore query tested, no production metrics yet
- **Requirement**: Real-time queries SHALL complete within 200ms for 95% of requests
- **Measurement**: Histogram `query_duration_seconds{p95}`
- **Test Method**: Benchmark queries on 1TB dataset
- **Acceptance**: p95 < 200ms, p99 < 1000ms

#### NFR-001.3: Storage Efficiency
- **BASELINE (Current)**: Parquet with Snappy compression active
- **Requirement**: Parquet compression SHALL achieve 10:1 ratio vs raw JSON
- **Measurement**: File size comparison
- **Test Method**: Store 1M records, compare sizes
- **Acceptance**: Parquet < 10% of JSON size (maintain current compression)

#### NFR-001.4: Configuration Performance
- **BASELINE (Current)**: config-client reads <10ms
- **Requirement**: Stream configuration reads SHALL remain <10ms
- **Acceptance**: Multi-stream config overhead MUST NOT degrade config read performance
- **Test Method**: Benchmark config reads with 1, 10, 100 streams registered

---

### NFR-002: Reliability

**Category**: System Reliability

#### NFR-002.1: Data Durability
- **Requirement**: System SHALL guarantee zero data loss under normal operation
- **Measurement**: Record ingestion vs storage count audit
- **Acceptance**: 100% durability for accepted records
- **Mechanism**:
  - fsync on Parquet writes
  - WAL for TimescaleDB
  - Dead-letter queue for invalid records

#### NFR-002.2: Fault Tolerance
- **Requirement**: System SHALL recover from component failures without manual intervention
- **Test Scenarios**:
  - MQTT broker restart: reconnect within 30 seconds
  - TimescaleDB crash: buffer writes, replay on recovery
  - etcd unavailable: continue with last-known configuration
- **Acceptance**: RTO < 5 minutes, RPO = 0 (no data loss)

#### NFR-002.3: Uptime
- **Requirement**: System SHALL maintain 99.9% uptime (43 minutes downtime/month)
- **Measurement**: Prometheus `up` metric
- **Exclusions**: Planned maintenance windows

---

### NFR-003: Extensibility

**Category**: Architectural Quality

#### NFR-003.1: Source Plugin Architecture
- **Requirement**: Adding new source type SHALL require <200 lines of code
- **Measurement**: Lines added for reference implementation
- **Test Method**: Implement WebSocket source as benchmark
- **Acceptance**: Core trait implementation < 200 LOC

#### NFR-003.2: Schema Evolution
- **Requirement**: Adding field to stream SHALL not require code changes
- **Acceptance**:
  - Configuration-only operation
  - Zero downtime deployment
  - Backward compatible queries

#### NFR-003.3: Stream Addition
- **Requirement**: Adding new stream SHALL complete via configuration in <5 minutes
- **Steps**:
  1. Write stream config to etcd
  2. Define schema
  3. Configure sources
  4. Verify ingestion
- **Acceptance**: End-to-end <5 minutes, zero code deployment

---

### NFR-004: Observability

**Category**: Operations

#### NFR-004.1: Metrics
- **Requirement**: System SHALL expose Prometheus metrics for all components
- **Required Metrics**:
  - `ingestion_records_total{stream, source}` (counter)
  - `ingestion_errors_total{stream, source, error_type}` (counter)
  - `ingestion_latency_seconds{stream}` (histogram)
  - `storage_bytes_written{layer, stream}` (counter)
  - `query_duration_seconds{endpoint}` (histogram)
  - `source_health_status{stream, source}` (gauge: 0=unhealthy, 1=healthy)

#### NFR-004.2: Structured Logging
- **Requirement**: All components SHALL emit structured logs (JSON format)
- **Log Levels**: ERROR, WARN, INFO, DEBUG
- **Required Fields**: timestamp, level, component, stream_id, message, context
- **Sink**: stdout (captured by Docker logging driver)

#### NFR-004.3: Distributed Tracing
- **Requirement**: Request flows SHALL be traceable end-to-end
- **Implementation**: OpenTelemetry with Jaeger backend
- **Traced Operations**:
  - HTTP request → ingestion → Bronze write → Silver write
  - MQTT message → validation → storage
  - Query request → data retrieval → response

---

### NFR-005: Security

**Category**: Security

#### NFR-005.1: Authentication
- **Requirement**: All API endpoints SHALL require authentication
- **Methods**: Bearer token, API key
- **Storage**: Secrets in etcd (encrypted at rest)
- **Rotation**: API keys rotatable without downtime

#### NFR-005.2: Authorization
- **Requirement**: Stream access SHALL be controlled by RBAC policies
- **Granularity**: Per-stream read/write permissions
- **Example**: User A can write to `air-quality`, read from `weather`

#### NFR-005.3: Data Encryption
- **Requirement**: Data SHALL be encrypted in transit and at rest
- **In Transit**: TLS 1.3 for all HTTP/MQTT connections
- **At Rest**: Filesystem encryption (LUKS), etcd encryption enabled

---

### NFR-006: Pi Deployment Compatibility

**Category**: Deployment
**Constraint**: All changes MUST deploy successfully on Raspberry Pi 5

#### NFR-006.1: Deployment Process
- **Requirement**: System SHALL deploy via `./deploy.sh` in `deploy/pi/` directory
- **Acceptance Criteria**:
  - Script execution succeeds on Pi 5 hardware
  - All services start within 5 minutes
  - Health checks pass for mosquitto, etcd, air-quality-app
  - Existing data volumes remain accessible
- **Test Method**: Execute full deployment on clean Pi 5 installation

#### NFR-006.2: Build Time
- **Requirement**: ARM64 build SHALL complete within 30 minutes on Pi 5
- **Measurement**: Time from `docker compose build` to image ready
- **Acceptance**: Total build time <30 minutes (initial) or <5 minutes (incremental)
- **Optimization**: Use build cache, multi-stage builds, minimal dependencies

#### NFR-006.3: Resource Constraints
- **Requirement**: Platform services SHALL operate within memory budget
- **Allocation**:
  - mosquitto: ~50MB
  - etcd: ~300MB
  - air-quality-app: ~500MB
  - Total: ~850MB (leaves margin for system)
- **Measurement**: `docker stats` after 24 hours of operation
- **Acceptance**: No OOM kills, stable RSS within limits

#### NFR-006.4: Backward Compatibility
- **Requirement**: Updated deployment SHALL preserve existing data and configuration
- **Acceptance Criteria**:
  - etcd keys from previous deployment remain accessible
  - Parquet files at `/app/data` remain queryable
  - Docker volume mounts work with existing volumes
  - No data migration required for updates

---

## 4. Constraints

### 4.1 Technical Constraints

#### TC-001: Home-Scale Deployment (Raspberry Pi Production)
- **Constraint**: System MUST run on single machine (no distributed deployment)
- **Production Hardware**: Raspberry Pi 5 with Ubuntu 25.04 ARM64
- **Specifications**:
  - CPU: 4 cores ARM64 (BCM2712)
  - RAM: 16GB total (platform services: ~896MB allocated)
  - Storage: 1TB minimum (SSD preferred)
  - Architecture: aarch64-unknown-linux-gnu
- **Deployment Location**: `deploy/pi/` (NOT docker/production)
- **Services**: mosquitto, etcd, air-quality-app
- **Memory Budget**: <1GB total for platform services
- **Implication**: No distributed consensus protocols, single-node optimization, ARM64 binary builds

#### TC-002: Existing Infrastructure
- **Constraint**: MUST integrate with AND NOT BREAK existing components
- **Components**:
  - ✅ etcd for configuration (already deployed with `/air-quality/*` keys)
  - ✅ Docker Compose for orchestration
  - ✅ Grafana for visualization (if configured)
  - ✅ MQTT broker (Mosquitto, actively used)
  - ✅ config-client library (actively used)
  - ✅ MqttSource (actively ingesting)
  - ✅ ParquetStore (actively writing)
- **Implication**:
  - Extend existing patterns from AIR-003 (NOT replace)
  - Maintain backward compatibility with all current etcd keys
  - Preserve current MQTT ingestion pipeline
  - Keep current storage paths readable

#### TC-003: Rust Implementation
- **Constraint**: Core platform MUST be written in Rust
- **Rationale**: Memory safety, performance, ecosystem maturity
- **Allowed Exceptions**: Configuration scripts (Bash), dashboards (JSON)

#### TC-004: Data Retention
- **Constraint**: Storage growth MUST be bounded
- **Limits**:
  - Bronze: configurable per stream (default 365 days)
  - Silver: configurable per stream (default 365 days)
  - Aggregates: 2 years
- **Enforcement**: Automated retention policies

#### TC-005: Pi Deployment Preservation
- **Constraint**: Production Pi deployment MUST remain functional throughout AIR-004
- **Protected Assets**:
  - `deploy/pi/docker-compose.yml` - MUST remain functional
  - `deploy/pi/deploy.sh` - Deployment workflow MUST NOT break
  - Volume names: `pi_air-quality-data`, `pi_etcd-data` - MUST persist
  - Data path: `/app/data` - Existing data MUST remain accessible
- **Deployment Process**:
  - Build: Cross-compile for ARM64 or build on Pi
  - Deploy: `./deploy.sh` in `deploy/pi/` directory
  - Rollback: Previous image tags must be preserved
- **Testing Requirement**: All changes MUST be validated on Pi 5 hardware before merge
- **Implication**: Docker Compose configuration is source of truth, not Kubernetes/production configs

---

### 4.2 Business Constraints

#### BC-001: Zero Licensing Cost
- **Constraint**: All components MUST use open-source licenses (MIT, Apache 2.0, PostgreSQL License)
- **Prohibited**: Commercial databases, proprietary message brokers

#### BC-002: Single-Person Operations
- **Constraint**: System MUST be operable by single administrator
- **Requirements**:
  - Automated deployment (Docker Compose)
  - Self-healing capabilities
  - Clear error messages and runbooks

---

### 4.3 Regulatory Constraints

#### RC-001: Data Privacy
- **Constraint**: PII MUST NOT be logged or stored unencrypted
- **Applicability**: If home-events stream contains PII
- **Enforcement**: Schema validation prohibits PII fields without encryption flag

---

## 5. Success Criteria

### 5.1 Functional Success

- **SC-001**: Ingest data from 3+ heterogeneous sources simultaneously (MQTT, HTTP poll, webhook)
- **SC-002**: Store 1 million records without data loss or corruption
- **SC-003**: Query historical data across 30-day window in <5 seconds
- **SC-004**: Add new stream via configuration with zero code changes
- **SC-005**: Grafana dashboard displays multi-stream correlation

### 5.2 Performance Success

- **SC-006**: Sustain 10,000 records/second ingestion for 1 hour
- **SC-007**: Achieve <200ms p95 query latency under load
- **SC-008**: Parquet compression ratio >5:1

### 5.3 Operational Success

- **SC-009**: System runs for 7 days without manual intervention
- **SC-010**: MQTT broker failure recovers automatically within 30 seconds
- **SC-011**: All components expose health checks (Docker HEALTHCHECK passes)

### 5.4 User Success

- **SC-012**: Administrator can add weather stream in <10 minutes (end-to-end)
- **SC-013**: Predictive model achieves >80% accuracy using cross-stream features
- **SC-014**: Dashboard load time <2 seconds for 24-hour view

---

## 6. Dependencies

### 6.1 Existing Components (Pi Deployment)

- **DEP-001**: etcd cluster (v3.5+) deployed via `deploy/pi/docker-compose.yml`
  - Service name: `etcd`
  - Volume: `pi_etcd-data:/etcd-data`
  - Port: 2379 (internal)
  - Status: ✅ DEPLOYED on production Pi

- **DEP-002**: MQTT broker (Mosquitto) running via `deploy/pi/docker-compose.yml`
  - Service name: `mosquitto`
  - Port: 1883 (internal)
  - Status: ✅ DEPLOYED on production Pi

- **DEP-003**: air-quality-app service deployed via `deploy/pi/docker-compose.yml`
  - Service name: `air-quality-app`
  - Volume: `pi_air-quality-data:/app/data`
  - Port: 3000 (exposed)
  - Binary: `/app/air-quality-server`
  - Status: ✅ DEPLOYED on production Pi

- **DEP-004**: TimescaleDB (v2.11+) instance provisioned
  - Status: ⚠️ NOT currently deployed on Pi
  - Future consideration for Silver layer storage
  - Alternative: Continue with Parquet-only for v1.0

- **DEP-005**: Grafana (v10.0+) configured with datasource
  - Status: ⚠️ Optional, not required for core platform
  - Can connect to air-quality-app API or Parquet directly

- **DEP-006**: Docker and Docker Compose installed on Pi 5
  - Platform: linux/arm64
  - Status: ✅ INSTALLED

### 6.2 Rust Dependencies

- **DEP-006**: `tokio` (async runtime)
- **DEP-007**: `rumqttc` (MQTT client)
- **DEP-008**: `reqwest` (HTTP client)
- **DEP-009**: `sqlx` (TimescaleDB adapter)
- **DEP-010**: `polars` (Parquet I/O and querying)
- **DEP-011**: `serde` / `serde_json` (serialization)
- **DEP-012**: `etcd-client` (configuration watching)

### 6.3 External Services

- **DEP-013**: Weather API (OpenWeatherMap or similar) for testing HTTP polling
- **DEP-014**: Test MQTT broker (for integration tests)

---

## 7. Out of Scope

### 7.1 Explicitly Excluded Features

- **OOS-001**: Multi-node distributed deployment (future consideration)
- **OOS-002**: Real-time stream processing (e.g., Apache Flink, Kafka Streams)
- **OOS-003**: Built-in machine learning training (use external tools like Python/Polars)
- **OOS-004**: User authentication UI (API-level auth only)
- **OOS-005**: Mobile app or web UI for configuration (etcd CLI/API only)
- **OOS-006**: Support for non-time-series data (e.g., graph databases)
- **OOS-007**: Data export to cloud providers (AWS S3, GCP, Azure)

### 7.2 Deferred to Future Phases

- **DEFER-001**: WebSocket source support (after v1.0)
- **DEFER-002**: File watch source (CSV imports) (after v1.0)
- **DEFER-003**: Advanced alerting rules engine (use Grafana alerts for v1.0)
- **DEFER-004**: GraphQL query API (REST API only for v1.0)
- **DEFER-005**: Multi-tenancy (single-user deployment for v1.0)

---

## 8. Acceptance Test Scenarios

### 8.1 End-to-End Scenario: Air Quality + Home Events Correlation

```gherkin
Feature: Multi-Stream Predictive Analytics

  Background:
    Given system is deployed and running
    And streams "air-quality" and "home-events" are configured
    And MQTT sources are publishing data
    And TimescaleDB is accepting writes

  Scenario: Cooking event correlates with PM2.5 spike
    Given home-events stream receives:
      | timestamp           | event_type    | target  | state  |
      | 2025-12-15 10:00:00 | cooking_start | kitchen | active |

    When air-quality measurements arrive:
      | timestamp           | pm25  |
      | 2025-12-15 10:00:00 | 8.5   |
      | 2025-12-15 10:05:00 | 12.3  |
      | 2025-12-15 10:10:00 | 18.7  |
      | 2025-12-15 10:15:00 | 25.4  |

    Then cross-stream query returns:
      """sql
      SELECT
        aq.timestamp,
        aq.pm25,
        he.event_type,
        aq.timestamp - he.timestamp as lag
      FROM air_quality aq
      ASOF LEFT JOIN home_events he
        ON aq.timestamp >= he.timestamp
      WHERE he.event_type = 'cooking_start'
        AND aq.timestamp BETWEEN '2025-12-15 10:00' AND '2025-12-15 10:30'
      """

    And results show PM2.5 increased by 200% within 15 minutes of cooking start
    And Grafana dashboard displays correlation annotation
```

### 8.2 Resilience Scenario: MQTT Broker Failure

```gherkin
Feature: Fault Tolerance

  Scenario: MQTT broker restart with zero data loss
    Given MQTT source is connected and ingesting
    And air-quality data is flowing at 10 msg/sec

    When MQTT broker is stopped
    Then source status changes to "disconnected"
    And error metric increments
    And buffering begins (up to 10,000 messages)

    When MQTT broker restarts after 30 seconds
    Then source reconnects within 5 seconds
    And buffered messages are published
    And ingestion resumes normal operation
    And zero messages are lost
```

### 8.3 Configuration Scenario: Add New Stream

```gherkin
Feature: Dynamic Stream Registration

  Scenario: Add weather stream without restart
    Given system is running with streams [air-quality, home-events]

    When administrator executes:
      """bash
      etcdctl put streams/weather/config '{
        "stream_id": "weather",
        "description": "Outdoor weather data",
        "retention_days": 365
      }'

      etcdctl put streams/weather/schema '{
        "fields": [
          {"name": "temperature", "type": "float", "unit": "celsius"},
          {"name": "humidity", "type": "float", "unit": "percent"}
        ]
      }'

      etcdctl put streams/weather/sources '[{
        "type": "http_poll",
        "url": "https://api.openweathermap.org/data/2.5/weather",
        "interval": "5m",
        "auth": {"type": "api_key", "key_param": "appid"}
      }]'
      """

    Then coordinator detects new stream within 1 second
    And HTTP poller is spawned
    And TimescaleDB table "weather" is created
    And Parquet directory "data/bronze/weather/" is created
    And first weather data arrives within 5 minutes
    And data is queryable via API
    And Grafana datasource includes weather table
```

### 8.4 Deployment Scenario: Pi Production Deployment

```gherkin
Feature: Raspberry Pi Deployment

  Background:
    Given Raspberry Pi 5 with Ubuntu 25.04 ARM64
    And Docker and Docker Compose are installed
    And Previous deployment data exists at volumes

  Scenario: Fresh deployment on clean Pi
    Given no previous deployment exists

    When administrator executes:
      """bash
      cd /workspaces/neural-data-platform/deploy/pi
      ./deploy.sh
      """

    Then Docker images build successfully within 30 minutes
    And services start in order: etcd, mosquitto, air-quality-app
    And all containers reach healthy status within 5 minutes
    And etcd is accessible at localhost:2379
    And mosquitto is accessible at localhost:1883
    And air-quality-app API responds at localhost:3000
    And volumes are created: pi_etcd-data, pi_air-quality-data
    And directory /app/data is mounted and writable

  Scenario: Update existing deployment preserving data
    Given system is deployed and running
    And etcd contains configuration keys
    And Parquet files exist at /app/data

    When administrator updates code and executes:
      """bash
      cd /workspaces/neural-data-platform/deploy/pi
      git pull origin main
      ./deploy.sh
      """

    Then new images build using cache (< 5 minutes)
    And services restart gracefully (no data loss)
    And existing etcd keys remain accessible
    And existing Parquet files remain queryable
    And air-quality ingestion resumes within 30 seconds
    And memory usage stays within 896MB total

  Scenario: Rollback on deployment failure
    Given current deployment version is v1.0
    And new version v1.1 is being deployed

    When deployment of v1.1 fails (healthcheck timeout)

    Then administrator can rollback:
      """bash
      docker compose down
      docker compose up -d --scale air-quality-app=0
      docker tag air-quality-app:v1.0 air-quality-app:latest
      docker compose up -d
      """

    And system returns to v1.0 state
    And no configuration data is lost
    And no Parquet data is lost
```

---

## 9. Risk Assessment

### 9.1 Technical Risks

| Risk ID | Description | Likelihood | Impact | Mitigation |
|---------|-------------|------------|--------|------------|
| R-001 | Dual-write inconsistency under load | Medium | High | Implement transaction log and reconciliation job |
| R-002 | Parquet write performance bottleneck | Low | Medium | Batch writes, use async I/O |
| R-003 | TimescaleDB schema migrations block ingestion | Medium | High | Use online migrations, background indexing |
| R-004 | etcd watch API latency causes config drift | Low | Medium | Buffer configuration changes, debounce reloads |

### 9.2 Operational Risks

| Risk ID | Description | Likelihood | Impact | Mitigation |
|---------|-------------|------------|--------|------------|
| R-005 | Disk space exhaustion from retention policy failure | Medium | Critical | Alerting on disk usage >80%, manual override |
| R-006 | MQTT broker overwhelmed by message volume | Low | High | QoS throttling, message sampling |
| R-007 | Administrator misconfigures schema causing data loss | Medium | High | Schema validation on write, dry-run mode |

### 9.3 Pi Deployment Risks

| Risk ID | Description | Likelihood | Impact | Mitigation |
|---------|-------------|------------|--------|------------|
| R-008 | ARM64 build fails on Pi due to memory constraints | Medium | High | Use cross-compilation from x86_64, or build with swap enabled |
| R-009 | Docker volume corruption after unclean shutdown | Low | Critical | Regular backups of etcd-data volume, WAL for Parquet writes |
| R-010 | Memory limit exceeded causing OOM kills | Medium | High | Conservative memory limits, monitoring with alerts, graceful degradation |
| R-011 | SD card wear from frequent Parquet writes | Medium | Medium | Use external SSD for data volumes, implement write batching |
| R-012 | Deployment script breaks existing configuration | Low | Critical | Pre-deployment backup, rollback procedure documented, validation checks |

---

## 10. Implementation Phases (REVISED: ADDITIVE APPROACH)

### Phase 0: Baseline Verification (Week 1)
**Goal**: Establish regression test suite for current functionality
- ✅ Document current etcd config structure (`/air-quality/*`)
- ✅ Create integration tests for existing MQTT ingestion
- ✅ Benchmark current performance (config reads, MQTT throughput, storage writes)
- ✅ Verify current Parquet data is queryable
- **Deliverable**: Regression test suite that MUST pass throughout AIR-004

### Phase 1: Foundation (Weeks 2-3)
**Goal**: Add stream registry WITHOUT breaking existing air-quality stream
- Implement `streams/{stream-id}/*` etcd namespace
- Add backward compatibility layer: `/air-quality/*` → `streams/air-quality/*`
- Unified Source trait (MqttSource ALREADY implements this pattern)
- **NO REFACTORING** of existing air-quality-app yet
- **Deliverable**: Can register new streams in etcd, air-quality continues unchanged

### Phase 2: Multi-Stream Coordinator (Weeks 4-5)
**Goal**: Coordinate multiple streams while preserving single-stream behavior
- Implement stream coordinator that manages multiple sources
- Migrate air-quality to coordinator (feature flag for rollback)
- Integrate existing HttpPollingSource from `core/src/sources/http_poll.rs`
- **Testing**: Run air-quality via coordinator AND via legacy path, verify identical behavior
- **Deliverable**: Coordinator managing air-quality + 1 test HTTP polling stream

### Phase 3: Schema & Storage Extension (Weeks 6-7)
**Goal**: Add schema validation and multi-stream storage routing
- Schema definition in etcd (`streams/{id}/schema`)
- Schema validation pipeline (air-quality schema auto-inferred from current parser)
- Multi-stream ParquetStore router (preserves current partition structure)
- **NO TimescaleDB** in this phase (deferred)
- **Deliverable**: 2+ streams writing to separate Parquet partitions

### Phase 4: Webhook & Observability (Week 8)
**Goal**: Add webhook ingestion and comprehensive metrics
- Webhook handler implementation (Axum endpoints)
- Enhanced health checks per source
- Prometheus metrics for multi-stream
- **Deliverable**: 3-source demo (MQTT, HTTP poll, webhook)

### Phase 5: Validation & Stabilization (Week 9)
**Goal**: Prove AIR-004 is production-ready
- Integration tests (end-to-end scenarios)
- Load testing: verify NO regression vs Phase 0 baseline
- Failure injection: verify air-quality stream survives coordinator crashes
- Documentation: migration guide, runbooks
- **Deliverable**: Production-ready multi-stream platform with air-quality still working

---

## 11. Open Questions

### Q-001: Webhook Authentication
- **Question**: Should webhook endpoints support multiple auth methods simultaneously (Bearer + API key)?
- **Impact**: Security model complexity
- **Decision Needed By**: Phase 2 start
- **Options**:
  - A) Single method per stream (simpler)
  - B) Multiple methods with priority (more flexible)

### Q-002: Schema Evolution - Breaking Changes
- **Question**: How to handle breaking schema changes (e.g., type change)?
- **Impact**: Data migration strategy
- **Options**:
  - A) Disallow entirely (strict immutability)
  - B) Support via versioned streams (e.g., `air-quality-v2`)
  - C) In-place migration with dual-schema period

### Q-003: Backfill Strategy
- **Question**: How to replay Bronze → Silver after Silver corruption?
- **Impact**: Disaster recovery procedure
- **Options**:
  - A) Manual script per stream
  - B) Generic backfill CLI tool
  - C) Automated repair on consistency check failure

### Q-004: Cross-Stream Join Performance
- **Question**: Will ASOF JOIN scale to 5+ streams with 1TB data?
- **Impact**: Multi-stream analytics feasibility
- **Validation**: Benchmark with synthetic data (Phase 4)

---

## 12. Glossary

| Term | Definition |
|------|------------|
| **Bronze Layer** | Raw, immutable data stored in Parquet format for long-term retention and batch analytics |
| **Silver Layer** | Queryable, structured data in TimescaleDB for real-time dashboards and aggregations |
| **Gold Layer** | Pre-aggregated, business-logic-enriched views (continuous aggregates) |
| **Stream** | Independent data pipeline with unique schema, sources, and storage configuration |
| **Source** | Component that ingests data from external system (MQTT, HTTP, webhook, etc.) |
| **Hypertable** | TimescaleDB abstraction over partitioned time-series table |
| **ASOF JOIN** | Time-correlation join that matches records based on temporal proximity |
| **Dual-Write** | Writing same data to multiple storage layers (Bronze + Silver) in single ingestion flow |
| **Continuous Aggregate** | Materialized view that incrementally updates based on new data |

---

## 13. References

- [TimescaleDB Documentation](https://docs.timescale.com/)
- [Apache Parquet Format Specification](https://parquet.apache.org/docs/)
- [MQTT v5.0 Specification](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html)
- [OpenTelemetry Rust SDK](https://github.com/open-telemetry/opentelemetry-rust)
- [Polars User Guide](https://pola-rs.github.io/polars-book/)
- AIR-003: etcd Configuration Hot-Reload Implementation

---

## 14. Specification Validation Checklist

- [x] All requirements are testable with clear acceptance criteria
- [x] Edge cases documented (MQTT reconnect, dual-write failure, schema evolution)
- [x] Performance metrics defined with measurable targets
- [x] Security requirements specified (authentication, encryption)
- [x] Dependencies identified (etcd, TimescaleDB, Rust crates)
- [x] Constraints documented (home-scale, single-node, existing infra)
- [x] Success criteria are measurable and time-bound
- [x] Out-of-scope items explicitly listed
- [x] Risks assessed with mitigation strategies
- [x] Open questions captured for decision tracking
- [x] **NEW**: Current implementation baseline documented
- [x] **NEW**: Backward compatibility requirements specified
- [x] **NEW**: Non-regression criteria defined
- [x] **NEW**: Additive implementation approach defined
- [x] **Pi DEPLOYMENT**: Raspberry Pi 5 production constraints documented (TC-001, TC-005)
- [x] **Pi DEPLOYMENT**: Correct deployment location specified (deploy/pi/, not docker/production)
- [x] **Pi DEPLOYMENT**: Memory budget constraints defined (<1GB total for platform)
- [x] **Pi DEPLOYMENT**: ARM64 build requirements specified
- [x] **Pi DEPLOYMENT**: Volume preservation requirements documented
- [x] **Pi DEPLOYMENT**: Deployment process acceptance criteria defined (NFR-006)
- [x] **Pi DEPLOYMENT**: Pi-specific risks identified and mitigated (R-008 through R-012)

---

## 15. Summary: AIR-004 Alignment with Current Implementation

### What Changes in This Revision

**Original AIR-004 Specification (v1.0.0)**: Described a greenfield multi-stream platform
**Revised AIR-004 Specification (v1.1.0)**: Extension of WORKING air-quality system

### Key Revisions

1. **Section 0 Added**: "Current Implementation Baseline"
   - Documents what works NOW (MQTT, Parquet, config-client)
   - Defines what MUST be preserved (no breaking changes)
   - Establishes performance baselines (no regression)

2. **FR-001 Revised**: Stream registry EXTENDS existing `/air-quality/*` config
   - Backward compatibility layer specified
   - Migration path from legacy to multi-stream

3. **FR-002 Revised**: Multi-source ingestion BUILDS ON existing MqttSource
   - Existing MQTT implementation marked as "ALREADY IMPLEMENTED"
   - HTTP polling marked as "CODE EXISTS - INTEGRATE"
   - Clear extension path vs rewrite

4. **NFR-001 Revised**: Performance requirements reference CURRENT baselines
   - Config reads: <10ms (current proven)
   - MQTT ingestion: 1+ msg/sec (current sustained)
   - Storage writes: 10k records/sec (current tested)

5. **Phase Plan Revised**: Additive approach with Phase 0 regression testing
   - Phase 0: Baseline verification (NEW)
   - Each phase includes "air-quality continues working" verification
   - NO breaking refactors until multi-stream proven

### Critical Constraints Honored

- ✅ MQTT AirGradient ingestion continues unchanged
- ✅ Existing etcd `/air-quality/*` keys remain valid
- ✅ Current Parquet data remains queryable
- ✅ No performance regression below baselines
- ✅ Implementation is ADDITIVE, not replacement

---

**Status**: SPECIFICATION COMPLETE (Pi Deployment Constraints Added)

**Next Steps**:
1. **VERIFY**: Run existing air-quality system on Pi, confirm it works
2. **BASELINE**: Execute Phase 0 regression test suite creation
3. Review Pi deployment constraints with stakeholders
4. Resolve open questions (Q-001 through Q-004)
5. Validate all changes on Pi 5 hardware before merge
6. Proceed to SPARC Pseudocode phase with additive approach
7. Create detailed implementation plan maintaining Pi deployment compatibility

**Critical Deployment Notes**:
- All development MUST test on `deploy/pi/` configuration
- Memory budget: <1GB total for all platform services
- Build target: `aarch64-unknown-linux-gnu`
- Volume preservation: `pi_air-quality-data`, `pi_etcd-data` MUST NOT change
- Deployment script: `./deploy.sh` in `deploy/pi/` MUST remain functional

---

*Document Version: 1.2.0 (Pi Deployment Constraints)*
*Last Updated: 2025-12-15*
*SPARC Phase: Specification (Complete - Pi Production Deployment)*
*Revision Focus: Raspberry Pi 5 production constraints and deployment preservation*
