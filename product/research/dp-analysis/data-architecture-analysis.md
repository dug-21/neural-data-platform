# Data Architecture Analysis: State-Change vs Event-Based Patterns

**Feature**: AIR-008 Home Events (Window State Tracking)
**Date**: 2025-12-29
**Author**: NDP Architect Agent
**Status**: Analysis Complete - Ready for Review

---

## Executive Summary

This analysis evaluates data architecture patterns for integrating Home Assistant events (window open/close state) into the Neural Data Platform. The key decision is between **state-change** and **event-based** data models, with implications extending beyond air quality to generic data platform capabilities (log streams, system events, etc.).

### Recommendation

**Hybrid Approach: Event-Sourced with Materialized State Views**

Store raw events (including state transitions) in Bronze layer, with DuckDB views that materialize current/historical state on demand. This provides:
- Full event history for ML training and pattern analysis
- Efficient current-state queries for dashboards
- Flexibility for future log stream and generic event patterns
- Alignment with existing NDP medallion architecture

---

## 1. Home Assistant Data Architecture Analysis

### 1.1 Home Assistant Database Schema

Home Assistant uses a sophisticated **hybrid event-state model** that provides valuable lessons:

#### States Table Structure
| Field | Type | Purpose |
|-------|------|---------|
| `state_id` | Integer (PK) | Unique identifier |
| `metadata_id` | Integer (FK) | References entity metadata |
| `state` | String(255) | Current state value (e.g., "on", "off", "open", "closed") |
| `last_changed_ts` | Float | Timestamp when state VALUE changed |
| `last_updated_ts` | Float | Timestamp of ANY update (including attributes) |
| `old_state_id` | Integer (FK) | Reference to previous state record |
| `attributes_id` | Integer (FK) | References state_attributes table |
| `context_id_bin` | Blob(16) | Links related events together |

**Key Insight**: Home Assistant distinguishes between `last_changed_ts` (state value changed) and `last_updated_ts` (any update, including attribute-only changes). This enables efficient querying of actual state transitions vs attribute updates.

#### Events Table Structure
| Field | Type | Purpose |
|-------|------|---------|
| `event_id` | Integer (PK) | Unique identifier |
| `time_fired_ts` | Float | Event timestamp |
| `event_type_id` | Integer (FK) | References event_types table |
| `data_id` | Integer (FK) | References event_data table |
| `context_id_bin` | Blob(16) | Links causally related events |

**Key Insight**: Events and states share a `context_id` that enables tracing causality (e.g., automation triggered -> state changed -> notification sent).

#### Statistics Tables (Long-term Storage)
| Table | Granularity | Retention | Purpose |
|-------|-------------|-----------|---------|
| `states` | Per-change | 10 days (default) | Full resolution state history |
| `statistics_short_term` | 5-minute | 10 days | Aggregated snapshots |
| `statistics` | Hourly | Indefinite | Long-term compressed history |

**Key Insight**: Home Assistant uses a tiered retention strategy - high-resolution for short-term, aggregated for long-term. This is directly applicable to NDP.

### 1.2 Home Assistant Patterns Applicable to NDP

1. **State with Previous Reference**: Each state record links to `old_state_id`, making state transition queries efficient without scanning event logs.

2. **Context Linking**: Events share context IDs to trace causality chains - essential for ML features that correlate window state with temperature changes.

3. **Normalized Attributes**: Attributes stored separately with many-to-one relationship, reducing storage for entities with identical attributes.

4. **Tiered Retention**: Raw data purged after configurable period, aggregates kept indefinitely.

---

## 2. State-Change vs Event-Based Pattern Comparison

### 2.1 Pattern Definitions

#### State-Change Pattern
Records the current state of an entity whenever it changes:
```
timestamp: 2025-12-29T10:00:00Z
entity_id: binary_sensor.living_room_window
state: "open"
previous_state: "closed"
attributes: { battery: 95, position: "fully_open" }
```

**Characteristics**:
- Each record represents a complete snapshot of entity state
- Implicit events (state transition inferred from consecutive records)
- Easy to query "what is the current state?"
- Harder to query "what events led to this state?"

#### Event-Based Pattern
Records discrete events that may or may not affect state:
```
timestamp: 2025-12-29T10:00:00Z
event_type: "window_opened"
entity_id: binary_sensor.living_room_window
event_data: { trigger: "manual", user: null }
```

**Characteristics**:
- Each record is an immutable fact about something that happened
- State must be derived by replaying/aggregating events
- Natural audit trail and causality tracking
- Harder to query current state without views/projections

### 2.2 Comparison Matrix

| Aspect | State-Change | Event-Based | NDP Relevance |
|--------|--------------|-------------|---------------|
| **Storage Efficiency** | Higher (only on change) | Medium (all events) | Important for Pi edge |
| **Query Current State** | Simple SELECT | Requires aggregation | Dashboard queries |
| **Historical Analysis** | Good (with previous_state) | Excellent (full audit) | ML training data |
| **Causality Tracking** | Limited | Native | Correlation features |
| **Schema Evolution** | Medium complexity | Higher complexity | Future extensibility |
| **Log Stream Fit** | Poor | Excellent | Future use cases |
| **Real-time Processing** | Simple | Requires projection | Grafana integration |
| **ML Training Data** | Needs transformation | Direct use | Prediction models |

### 2.3 Industry Patterns

#### Event Sourcing for IoT
Research indicates event sourcing is "a natural fit for IoT applications" because:
- IoT systems need to "capture the state of things over time for historical data analysis"
- The event store provides automatic state history
- "The write model often consists of time series data collected from the real world"

Source: [CQRS and Event Sourcing for the IoT](http://sensetecnic.com/cqrs-and-event-sourcing-for-the-iot/)

#### When CRUD/State-Change is Preferred
"CRUD is useful if the data to be stored does not contain any semantics because it is only raw data. For example, this can be the case on the internet of things (IoT), where you have to capture and persist large amounts of sensor data."

Source: [Event sourcing vs CRUD - RisingStack](https://blog.risingstack.com/event-sourcing-vs-crud/)

**Key Insight**: Continuous sensor readings (temperature, humidity) favor state-change patterns. Discrete events (window opened, door unlocked) favor event-based patterns.

---

## 3. NDP-Specific Considerations

### 3.1 Current Architecture

The Neural Data Platform currently uses a **state-change pattern** for continuous sensor data:

```
Bronze Layer (Parquet):
  timestamp: i64
  location_id: String
  pm25: f64
  pm10: f64
  co2: f64
  temperature: f64
  humidity: f64
```

This works well for:
- Continuous measurements (every N seconds/minutes)
- Range-based queries (temperature over time)
- Aggregations (hourly averages)

### 3.2 Window State Characteristics

Window open/close events have different characteristics:

| Characteristic | Continuous Sensors | Window Events |
|----------------|-------------------|---------------|
| **Frequency** | Regular intervals | Irregular, sparse |
| **Duration** | Instantaneous reading | Duration matters |
| **Causality** | Independent | Affects other metrics |
| **State Model** | Continuous values | Binary/discrete |
| **Query Pattern** | "What was temperature at T?" | "Was window open at T?" |

**Key Insight**: Window state requires both:
1. **Event capture**: When did the state change? (for ML correlation)
2. **State queries**: What was the state at time T? (for context enrichment)

### 3.3 Future Use Cases: Log Streams

The SCOPE.md mentions considering "log streams from systems" - a completely different category:

| Aspect | Sensor Data | Window Events | Log Streams |
|--------|-------------|---------------|-------------|
| **Nature** | Continuous | Discrete | Discrete |
| **Volume** | Moderate | Low | High |
| **Schema** | Fixed | Fixed | Variable |
| **Query Pattern** | Time-range | Point-in-time | Search/filter |
| **Retention** | Days/weeks | Weeks/months | Hours/days |

**Implication**: The architecture must support multiple data models without forcing everything into one pattern.

---

## 4. Proposed Architecture

### 4.1 Hybrid Event-State Model

Store events in Bronze, materialize state views in Silver:

```
Bronze Layer (Parquet):
  home_events/YYYY-MM-DD_events.parquet
    - event_id: String (UUID)
    - timestamp: i64
    - event_type: String ("state_changed", "automation_triggered", etc.)
    - entity_id: String ("binary_sensor.living_room_window")
    - entity_domain: String ("binary_sensor", "switch", "automation")
    - new_state: String ("open", "closed", null for non-state events)
    - old_state: String (previous state, null for initial)
    - attributes: String (JSON blob for extensibility)
    - context_id: String (UUID for causality linking)
    - source: String ("home_assistant", "manual", "automation")

Silver Layer (DuckDB Views):
  - current_entity_state: Latest state per entity
  - state_transitions: State changes with duration
  - window_state_at_time(timestamp): Point-in-time state lookup
  - event_causality_chain(context_id): Related events
```

### 4.2 Schema Design

#### Bronze Event Schema
```yaml
stream_id: home-events
description: "Home Assistant events and state changes"
version: "1.0.0"
partitioning_strategy: daily
fields:
  - name: event_id
    field_type: String
    nullable: false
    description: "Unique event identifier (UUID)"

  - name: timestamp
    field_type: Int
    nullable: false
    unit: "epoch_ms"
    description: "Event timestamp"

  - name: event_type
    field_type: String
    nullable: false
    description: "Event type (state_changed, call_service, automation_triggered)"

  - name: entity_id
    field_type: String
    nullable: false
    description: "Home Assistant entity ID"

  - name: entity_domain
    field_type: String
    nullable: false
    description: "Entity domain (binary_sensor, switch, light, etc.)"

  - name: new_state
    field_type: String
    nullable: true
    description: "New state value (null for non-state events)"

  - name: old_state
    field_type: String
    nullable: true
    description: "Previous state value"

  - name: state_changed
    field_type: Bool
    nullable: false
    description: "True if state value changed (vs attribute-only update)"

  - name: attributes
    field_type: Json
    nullable: true
    description: "Entity attributes at event time"

  - name: context_id
    field_type: String
    nullable: true
    description: "Context ID for causality tracking"

  - name: source
    field_type: String
    nullable: false
    description: "Event source (home_assistant, manual, api)"
```

#### Silver State Views

```sql
-- Current state of all entities
CREATE VIEW silver_current_entity_state AS
SELECT
  entity_id,
  entity_domain,
  new_state AS current_state,
  attributes,
  timestamp AS last_updated,
  event_id AS last_event_id
FROM (
  SELECT *,
    ROW_NUMBER() OVER (PARTITION BY entity_id ORDER BY timestamp DESC) AS rn
  FROM bronze_home_events
  WHERE new_state IS NOT NULL
)
WHERE rn = 1;

-- State transitions with duration
CREATE VIEW silver_state_transitions AS
SELECT
  entity_id,
  entity_domain,
  old_state,
  new_state,
  timestamp AS transition_time,
  LEAD(timestamp) OVER (PARTITION BY entity_id ORDER BY timestamp) AS next_transition,
  LEAD(timestamp) OVER (PARTITION BY entity_id ORDER BY timestamp) - timestamp AS duration_ms,
  context_id
FROM bronze_home_events
WHERE state_changed = true
ORDER BY entity_id, timestamp;

-- Window state summary for correlation with air quality
CREATE VIEW silver_window_state_summary AS
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000)) AS bucket,
  SUM(CASE WHEN new_state = 'on' OR new_state = 'open' THEN 1 ELSE 0 END) AS windows_opened,
  SUM(CASE WHEN new_state = 'off' OR new_state = 'closed' THEN 1 ELSE 0 END) AS windows_closed,
  COUNT(DISTINCT entity_id) AS entities_changed
FROM bronze_home_events
WHERE entity_domain = 'binary_sensor'
  AND entity_id LIKE '%window%'
  AND state_changed = true
GROUP BY bucket;
```

### 4.3 Point-in-Time State Function

For ML features that need "was window open at time T?":

```sql
-- Function to get entity state at a specific point in time
CREATE OR REPLACE FUNCTION entity_state_at_time(
  p_entity_id VARCHAR,
  p_timestamp BIGINT
) RETURNS VARCHAR AS $$
SELECT new_state
FROM bronze_home_events
WHERE entity_id = p_entity_id
  AND timestamp <= p_timestamp
  AND new_state IS NOT NULL
ORDER BY timestamp DESC
LIMIT 1;
$$ LANGUAGE SQL;

-- Usage in cross-stream correlation
CREATE VIEW cross_stream_with_window_state AS
SELECT
  a.timestamp,
  a.temperature,
  a.humidity,
  a.pm25,
  entity_state_at_time('binary_sensor.living_room_window', a.timestamp) AS window_state
FROM silver_indoor_air a
WHERE a.timestamp >= (SELECT MIN(timestamp) FROM bronze_home_events);
```

---

## 5. Generic Data Platform Implications

### 5.1 Log Stream Support

The event-based Bronze layer naturally extends to log streams:

```yaml
stream_id: system-logs
description: "System and application log events"
version: "1.0.0"
partitioning_strategy: daily
fields:
  - name: event_id
    field_type: String
    nullable: false
  - name: timestamp
    field_type: Int
    nullable: false
  - name: event_type
    field_type: String  # "log_entry", "metric", "trace"
    nullable: false
  - name: severity
    field_type: String  # "debug", "info", "warn", "error", "fatal"
    nullable: true
  - name: source
    field_type: String  # "nginx", "app", "database"
    nullable: false
  - name: message
    field_type: String
    nullable: true
  - name: attributes
    field_type: Json
    nullable: true
  - name: context_id
    field_type: String  # Request/trace ID for correlation
    nullable: true
```

### 5.2 Unified Event Schema Pattern

Both home events and log streams share a common structure:

```
BaseEvent:
  - event_id: UUID
  - timestamp: epoch_ms
  - event_type: String
  - source: String
  - context_id: String (optional, for correlation)
  - attributes: JSON (extensible metadata)

HomeEvent extends BaseEvent:
  - entity_id
  - entity_domain
  - new_state
  - old_state
  - state_changed

LogEvent extends BaseEvent:
  - severity
  - message
  - trace_id
  - span_id
```

### 5.3 Storage Strategy by Data Type

| Data Type | Bronze Format | Partitioning | Retention | Silver Pattern |
|-----------|--------------|--------------|-----------|----------------|
| Continuous Sensors | Columnar Parquet | Daily | 90 days | Aggregation views |
| Home Events | Row-oriented Parquet | Daily | 180 days | State materialization |
| Log Streams | Row-oriented Parquet | Hourly | 7 days | Search indexes |
| Metrics | Columnar Parquet | Daily | 30 days | Rollup views |

---

## 6. ADR: Event-Sourced Home Events with State Views

### ADR-005: Hybrid Event-State Model for Discrete Entity Data

#### Status
Proposed

#### Context
The Neural Data Platform needs to integrate Home Assistant window open/close events for:
1. Real-time dashboard display of current window state
2. Historical correlation with air quality metrics
3. ML training data for predictive models (when to open/close windows)
4. Future extension to other discrete event sources (logs, system events)

The existing NDP architecture uses a state-change model optimized for continuous sensor data. Home events are discrete and sparse, with state transitions that have meaningful duration.

#### Decision
**Adopt a hybrid event-sourced model with materialized state views:**

1. **Bronze Layer**: Store raw events with full context
   - Event-based schema with `old_state`, `new_state`, and `context_id`
   - Append-only, immutable event log
   - Daily partitioned Parquet files

2. **Silver Layer**: DuckDB views for state queries
   - `current_entity_state`: Latest state per entity
   - `state_transitions`: State changes with duration calculation
   - `entity_state_at_time()`: Point-in-time state lookup function
   - Cross-stream correlation views

3. **Integration Pattern**:
   - Home Assistant webhook or MQTT source
   - ResponseParser implementation for state_changed events
   - Stream configuration following existing patterns

#### Consequences

**Positive**:
- Full audit trail for ML training and debugging
- Efficient current-state queries via materialized views
- Duration calculation for "time window was open" features
- Context linking enables causality analysis
- Natural extension to log streams and other event sources
- Aligns with Home Assistant's proven hybrid model

**Negative**:
- More complex schema than pure state-change
- Point-in-time queries require window functions (performance consideration)
- Two conceptual models (events for discrete, states for continuous)

**Neutral**:
- Storage overhead acceptable for sparse home events
- View refresh strategy to be determined (real-time vs periodic)

#### Alternatives Considered

**Alternative 1: Pure State-Change (like existing sensors)**
```
timestamp, entity_id, state, attributes
```
- Rejected: Loses event context, harder to calculate durations, no causality tracking

**Alternative 2: Pure Event Sourcing (derive all state)**
```
timestamp, event_type, entity_id, event_data
```
- Rejected: Complex state reconstruction, poor dashboard query performance

**Alternative 3: Separate State and Event Tables**
- Rejected: Data duplication, synchronization complexity, violates DRY

#### Implementation Impact

**Files to Create**:
- `core/src/sources/parsers/home_assistant.rs` - Event parser
- `core/src/types/home_event.rs` - Event type definitions
- `config/base/streams/home-events/config.yaml` - Stream configuration
- `config/duckdb/views/home_events.sql` - Silver layer views

**Files to Modify**:
- `core/src/sources/parsers/mod.rs` - Export new parser
- `core/src/sources/parsers/registry.rs` - Register parser

---

## 7. Recommendations

### 7.1 Immediate Actions (AIR-008)

1. **Create Home Events Stream**: Define `home-events` stream with event-based schema
2. **Implement HomeAssistantParser**: Parse `state_changed` events from HA webhook/MQTT
3. **Add Silver Views**: Create DuckDB views for state queries
4. **Cross-Stream Correlation**: Add window state to air quality correlation dashboards

### 7.2 Future Considerations

1. **Log Stream Architecture**: Use same event-based pattern when adding log ingestion
2. **Event Compression**: Consider columnar storage optimization for high-volume event streams
3. **Real-time State**: Evaluate Redis/DuckDB in-memory for sub-second state queries
4. **Context Tracing**: Implement distributed tracing pattern for complex event chains

### 7.3 Home Assistant Integration Options

| Method | Latency | Complexity | NDP Fit |
|--------|---------|------------|---------|
| **MQTT (Recommended)** | Real-time | Low | Existing MqttSource |
| **Webhook** | Near real-time | Medium | Existing HttpPollingSource |
| **Database Polling** | High (5-10s) | High | New source type needed |
| **WebSocket** | Real-time | Medium | New source type needed |

**Recommendation**: Use MQTT integration with Home Assistant MQTT integration add-on. This leverages existing NDP MqttSource with a new HomeAssistantParser.

---

## 8. Sources

### Home Assistant Data Architecture
- [Home Assistant Data Primer](https://data.home-assistant.io/docs/data) - Core data concepts
- [Home Assistant States](https://data.home-assistant.io/docs/states) - States table schema
- [Home Assistant Events](https://data.home-assistant.io/docs/events) - Events table schema
- [Home Assistant Statistics](https://data.home-assistant.io/docs/statistics) - Long-term storage approach

### Event Sourcing and IoT Patterns
- [Event Sourcing vs CRUD - RisingStack](https://blog.risingstack.com/event-sourcing-vs-crud/)
- [CQRS and Event Sourcing for the IoT - Sense Tecnic](http://sensetecnic.com/cqrs-and-event-sourcing-for-the-iot/)
- [Event Sourcing Pattern - Microservices.io](https://microservices.io/patterns/data/event-sourcing.html)
- [Event-Driven vs State-Based - Confluent](https://developer.confluent.io/courses/event-sourcing/event-driven-vs-state-based/)
- [Architectural Patterns For IoT - Medium](https://medium.com/@prashunjaveri/architectural-patterns-for-iot-event-driven-architectures-557be35fa626)

### Time Series and Data Lake Patterns
- [Top 10 DBaaS for IoT & Time-Series Data 2024](https://daily.dev/blog/top-10-dbaas-for-iot-and-time-series-data-2024)
- [Event Sourcing and Event-Driven Architecture - RisingWave](https://risingwave.com/blog/event-sourcing-and-event-driven-architecture-a-comparative-analysis/)
- [Event Sourcing Outgrows the Database - Confluent](https://www.confluent.io/blog/event-sourcing-outgrows-the-database/)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-29 | Initial analysis |
