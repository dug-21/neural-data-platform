# air-012: Home Assistant Integration - Specification

## Document Information

| Field | Value |
|-------|-------|
| Feature | air-012 |
| Phase | Specification (SPARC S) |
| Version | 1.0.0 |
| Date | 2026-01-30 |
| Author | Specification Agent |
| Status | Draft |

---

## 1. Overview

This specification defines the requirements for integrating Home Assistant window/door binary sensors into the Neural Data Platform. The integration captures state change events (open/closed) and flows them through Bronze and Silver layers for future correlation analysis with air quality data.

### 1.1 Scope Boundaries

**In Scope:**
- MQTT subscription to Home Assistant binary sensor topics
- Bronze layer storage of raw state events
- Silver layer `state_events` table (simple event log)
- Pipeline health monitoring with sparse-data thresholds
- Dimension table entries for 3 initial sensors

**Out of Scope (Deferred to dp-014):**
- SCD (Slowly Changing Dimension) semantics
- Point-in-time state queries
- Previous state tracking
- Correlation analysis with air quality

---

## 2. Functional Requirements

### FR-001: MQTT Connection

**Description:** The system shall connect to the Home Assistant MQTT broker and maintain a persistent connection.

| Attribute | Value |
|-----------|-------|
| Priority | High |
| Testable | Yes |

**Acceptance Criteria:**
- AC-001.1: System connects to broker at `192.168.52.103:1883`
- AC-001.2: No authentication required (internal network)
- AC-001.3: Connection auto-reconnects on network interruption
- AC-001.4: Connection status is observable via health endpoint

---

### FR-002: Topic Subscription

**Description:** The system shall subscribe to Home Assistant binary sensor state topics using a wildcard pattern.

| Attribute | Value |
|-----------|-------|
| Priority | High |
| Testable | Yes |

**Acceptance Criteria:**
- AC-002.1: Subscribe to pattern `homeassistant/binary_sensor/+/state`
- AC-002.2: Receives messages for all matching sensors
- AC-002.3: Initial 3 sensors are: `door_backslider`, `door_officewindow`, `door_dinettewindow`

---

### FR-003: Message Parsing

**Description:** The system shall parse incoming MQTT messages to extract sensor identity and state.

| Attribute | Value |
|-----------|-------|
| Priority | High |
| Testable | Yes |

**Acceptance Criteria:**
- AC-003.1: Extract `ndp_id` from topic path (e.g., `door_backslider` from `homeassistant/binary_sensor/door_backslider/state`)
- AC-003.2: Extract `source_entity_id` as `binary_sensor.<sensor_name>` (e.g., `binary_sensor.door_backslider`)
- AC-003.3: Parse payload as literal text: `"on"` or `"off"`
- AC-003.4: Assign ingestion timestamp at message receipt (not from payload)

---

### FR-004: Bronze Layer Storage

**Description:** The system shall store raw MQTT messages in Bronze layer Parquet files.

| Attribute | Value |
|-----------|-------|
| Priority | High |
| Testable | Yes |

**Acceptance Criteria:**
- AC-004.1: Raw payload stored verbatim (`"on"` or `"off"`)
- AC-004.2: Topic metadata preserved
- AC-004.3: Ingestion timestamp recorded
- AC-004.4: `ndp_id` extracted and stored
- AC-004.5: Files partitioned by date (daily partitioning strategy)

---

### FR-005: Silver Layer ETL

**Description:** The system shall transform Bronze data into Silver layer `state_events` table.

| Attribute | Value |
|-----------|-------|
| Priority | High |
| Testable | Yes |

**Acceptance Criteria:**
- AC-005.1: Data inserted into `silver.state_events` hypertable
- AC-005.2: `event_time` populated from ingestion timestamp
- AC-005.3: `state` field contains `"on"` or `"off"`
- AC-005.4: `ndp_id` and `source_entity_id` correctly mapped
- AC-005.5: Deduplication by `(event_time, ndp_id)` primary key

---

### FR-006: Dimension Table Registration

**Description:** The 3 initial sensors shall be registered in the entity context dimension table.

| Attribute | Value |
|-----------|-------|
| Priority | Medium |
| Testable | Yes |

**Acceptance Criteria:**
- AC-006.1: `door_backslider` registered with category `door`, orientation `south`
- AC-006.2: `door_officewindow` registered with category `window`, orientation `east`
- AC-006.3: `door_dinettewindow` registered with category `window`, orientation `west`
- AC-006.4: All sensors have `correlates_with: aq_airgradient_1`
- AC-006.5: Dimension sync (`./deploy.sh sync-dimensions`) loads entries

---

### FR-007: Pipeline Health Monitoring

**Description:** The system shall monitor data freshness with sparse-data-appropriate thresholds.

| Attribute | Value |
|-----------|-------|
| Priority | Medium |
| Testable | Yes |

**Acceptance Criteria:**
- AC-007.1: Fresh threshold: < 18 hours since last event
- AC-007.2: Stale threshold: 18-36 hours since last event
- AC-007.3: Critical threshold: > 36 hours since last event
- AC-007.4: No false alarms when windows remain closed for extended periods

---

## 3. Non-Functional Requirements

### NFR-001: Latency

**Description:** End-to-end latency from MQTT publish to Silver availability.

| Attribute | Value |
|-----------|-------|
| Category | Performance |
| Measurement | Time from MQTT publish to queryable in Silver |

**Requirements:**
- NFR-001.1: MQTT to Bronze < 1 second (typical)
- NFR-001.2: Bronze to Silver < 30 seconds (batch interval)
- NFR-001.3: Total latency < 35 seconds under normal operation

---

### NFR-002: Reliability

**Description:** System reliability for event capture.

| Attribute | Value |
|-----------|-------|
| Category | Reliability |
| Measurement | Event capture rate |

**Requirements:**
- NFR-002.1: Zero event loss during normal operation
- NFR-002.2: Auto-reconnect within 30 seconds of broker availability
- NFR-002.3: Tolerate broker restarts without manual intervention

---

### NFR-003: Resource Constraints

**Description:** Resource usage on Raspberry Pi 5.

| Attribute | Value |
|-----------|-------|
| Category | Resources |
| Target | Raspberry Pi 5 (4GB RAM) |

**Requirements:**
- NFR-003.1: MQTT client memory < 10MB
- NFR-003.2: Bronze storage growth < 1MB/month (sparse events)
- NFR-003.3: Silver chunk size aligned with existing 1-day policy

---

## 4. Interface Contracts

### 4.1 MQTT Message Format

**Topic Pattern:**
```
homeassistant/binary_sensor/{sensor_name}/state
```

**Payload:**
```
on
```
or
```
off
```

**Notes:**
- Plain text payload (not JSON)
- No timestamp in payload; use ingestion time
- Sensor name extracted from topic path

**Example Messages:**
```
Topic: homeassistant/binary_sensor/door_backslider/state
Payload: off

Topic: homeassistant/binary_sensor/door_officewindow/state
Payload: on
```

---

### 4.2 Bronze Schema

**Stream ID:** `home-assistant-state`

**Parquet Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | INT64 (microseconds) | Ingestion time |
| `ndp_id` | STRING | Sensor identifier (e.g., `door_backslider`) |
| `topic` | STRING | Full MQTT topic |
| `raw_payload` | STRING | Literal `"on"` or `"off"` |
| `source_stream` | STRING | `"home-assistant-state"` |

**Partitioning:** Daily by ingestion date

---

### 4.3 Silver Schema

**Table:** `silver.state_events`

```sql
CREATE TABLE silver.state_events (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_time          TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'home-assistant-state',

    -- Identity
    ndp_id              TEXT NOT NULL,
    source_entity_id    TEXT,

    -- State
    state               TEXT NOT NULL,

    -- DQ Transparency
    dq_flags            TEXT[],

    -- Primary Key
    PRIMARY KEY (event_time, ndp_id)
);

SELECT create_hypertable('silver.state_events', 'event_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE);
```

**Indexes:**

| Index | Columns | Purpose |
|-------|---------|---------|
| `idx_state_events_ndp` | `(ndp_id, event_time DESC)` | Sensor-specific queries |

**Compression Policy:**
```sql
ALTER TABLE silver.state_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'event_time DESC'
);

SELECT add_compression_policy('silver.state_events',
    INTERVAL '7 days',
    if_not_exists => TRUE);
```

**Retention Policy:**
```sql
SELECT add_retention_policy('silver.state_events',
    INTERVAL '90 days',
    if_not_exists => TRUE);
```

---

### 4.4 Dimension Table Entry Format

**File:** `data/dimensions/entity_context.csv`

**Schema:**
```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
```

**New Entries:**
```csv
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,/home/dining,aq_airgradient_1,west
```

---

## 5. Data Flow Specification

### 5.1 End-to-End Flow

```
+------------------+
| Home Assistant   |
| (MQTT Publisher) |
+--------+---------+
         |
         | MQTT Publish
         | Topic: homeassistant/binary_sensor/door_backslider/state
         | Payload: "on"
         v
+------------------+
| MQTT Broker      |
| 192.168.52.103   |
+--------+---------+
         |
         | Subscribe: homeassistant/binary_sensor/+/state
         v
+------------------+
| NDP MQTT Source  |
| (Rust)           |
+--------+---------+
         |
         | Parse topic -> ndp_id
         | Add ingestion timestamp
         v
+------------------+
| Bronze Layer     |
| (Parquet)        |
+--------+---------+
         |
         | ETL: Extract state, map fields
         v
+------------------+
| Silver Layer     |
| (TimescaleDB)    |
| state_events     |
+--------+---------+
         |
         | Query
         v
+------------------+
| Pipeline Health  |
| Dashboard        |
+------------------+
```

### 5.2 Message Processing Steps

1. **MQTT Receipt:**
   - Receive message from broker
   - Record ingestion timestamp (UTC)
   - Extract `ndp_id` from topic: `homeassistant/binary_sensor/{ndp_id}/state`

2. **Bronze Storage:**
   - Create `RawDataPoint` with:
     - `timestamp`: Current UTC microseconds
     - `ndp_id`: Extracted sensor name
     - `topic`: Full MQTT topic
     - `raw_payload`: Literal payload text
   - Write to daily Parquet partition

3. **Silver ETL:**
   - Read Bronze Parquet files
   - Transform to `state_events` schema:
     - `event_time` = Bronze `timestamp` converted to `TIMESTAMPTZ`
     - `source_entity_id` = `binary_sensor.` + `ndp_id`
     - `state` = `raw_payload`
   - Upsert to `silver.state_events` (dedupe on PK)

4. **Health Monitoring:**
   - Query `MAX(event_time)` per `ndp_id`
   - Apply sparse-data thresholds (18h/36h)
   - Display status in dashboard

---

## 6. Configuration Requirements

### 6.1 Stream Configuration

**File:** `config/base/streams/home-assistant-state/config.yaml`

```yaml
stream_id: "home-assistant-state"
description: "Home Assistant binary sensor state events"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: "daily"

# Schema definition
fields:
  state:
    type: "text"
    description: "Binary state: on (open) or off (closed)"
    nullable: false

# MQTT Source Configuration
sources:
  - type: mqtt
    enabled: true
    ndp_id_from_topic: true
    topic_segment: 3  # homeassistant/binary_sensor/{3}/state
    broker_url: "192.168.52.103"
    port: 1883
    client_id: "ndp-homeassistant-state"
    topic_pattern: "homeassistant/binary_sensor/+/state"
    qos: 1
    reconnect_delay_secs: 1
    max_reconnect_delay_secs: 30
    buffer_capacity: 100
    parser:
      parser_type: raw_text
      default_tags:
        source: mqtt
        stream_id: home-assistant-state

# Silver ETL Configuration
silver_etl:
  enabled: true
  target_table: silver.state_events
  description: "Window/door state change events from Home Assistant"
  grain: "One row per state change event"

  timestamp:
    source_field: timestamp
    target_field: event_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload
      target_column: state
      type: text
      description: "Binary state (on/off)"
      nullable: false
      dq_rules:
        - rule: value_in_set
          values: ["on", "off"]
          action: flag

    - source_path: source_entity_id
      target_column: source_entity_id
      type: text
      description: "Home Assistant entity ID"
      nullable: true
      transform: "concat('binary_sensor.', ndp_id)"

  deduplication:
    enabled: true
    key_columns: [event_time, ndp_id]
    strategy: upsert

  incremental:
    enabled: true
    watermark_column: event_time
    lag_interval: 5 minutes
```

### 6.2 Pipeline Health Configuration

**Freshness Thresholds (per stream):**

| Stream | Fresh | Stale | Critical |
|--------|-------|-------|----------|
| `air-quality` | < 5 min | 5-15 min | > 15 min |
| `home-assistant-state` | < 18 hours | 18-36 hours | > 36 hours |

**Implementation:** Update pipeline health query to select threshold based on `source_stream`.

---

## 7. Data Quality Rules

### 7.1 Field-Level Rules

| Field | Rule | Action | Description |
|-------|------|--------|-------------|
| `state` | `value_in_set(["on", "off"])` | Flag | Only valid binary states |
| `event_time` | `freshness_check(max_future=5min)` | Flag | Reject future timestamps |

### 7.2 DQ Output

- DQ flags stored in `dq_flags` array column
- No separate transparency table (simple schema)
- Flagged but not rejected (retain all data)

---

## 8. Error Handling

### 8.1 MQTT Connection Errors

| Error | Handling | Recovery |
|-------|----------|----------|
| Broker unreachable | Log error, retry with backoff | Auto-reconnect |
| Connection dropped | Log warning | Auto-reconnect within 30s |
| Subscribe failed | Log error, retry | Retry on reconnect |

### 8.2 Message Processing Errors

| Error | Handling | Recovery |
|-------|----------|----------|
| Invalid topic format | Log warning, skip message | None (log only) |
| Unexpected payload | Flag in DQ, store anyway | Data retained with flag |
| Bronze write failure | Log error, buffer in memory | Retry with backoff |

### 8.3 ETL Errors

| Error | Handling | Recovery |
|-------|----------|----------|
| Silver insert failure | Log error, retry batch | Retry with backoff |
| PK conflict | Upsert semantics | Automatic resolution |
| TimescaleDB unavailable | Log error, skip batch | Retry next cycle |

---

## 9. Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| MQTT source adapter | Ready | Implemented in `core/src/sources/mqtt/` |
| Bronze Parquet storage | Ready | Working for air-quality stream |
| Dimension tables (dp-013) | Ready | `entity_context` infrastructure exists |
| Pipeline health dashboard | Ready | Needs threshold customization |
| TimescaleDB | Ready | Silver schema v1.0.0 deployed |

---

## 10. Glossary

| Term | Definition |
|------|------------|
| Binary Sensor | Home Assistant sensor with on/off state |
| Bronze Layer | Raw data storage in Parquet format |
| Silver Layer | Cleaned, structured data in TimescaleDB |
| ndp_id | Neural Data Platform entity identifier |
| Sparse Data | Data that arrives infrequently (event-driven) |
| SCD | Slowly Changing Dimension (deferred to dp-014) |
| Hypertable | TimescaleDB partitioned table |

---

## 11. References

- SCOPE.md: `/workspaces/neural-data-platform/product/features/air-012/SCOPE.md`
- Silver Schema: `/workspaces/neural-data-platform/deploy/timescaledb/init/001_silver_schema.sql`
- MQTT Source: `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs`
- Air Quality Config: `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml`
- dp-014: Config-Driven Gold Layer (future)
