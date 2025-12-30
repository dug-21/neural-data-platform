# Time-Series Data Platform Strategies for Event/State Hybrid Data

**Feature**: AIR-008 - Home Events (Window Open/Close)
**Author**: ndp-feature-engineer
**Date**: 2025-12-29
**Status**: Research Complete

---

## Executive Summary

This research document analyzes time-series data platform strategies for integrating Home Assistant event/state hybrid data into the Neural Data Platform. The primary use case is capturing window open/close events to enable neural prediction for optimal window timing based on air quality conditions.

### Key Recommendations

1. **Data Architecture**: Use a **hybrid event-state model** - store discrete events with derived state views
2. **Storage Pattern**: Event-first with materialized state for query efficiency
3. **Feature Engineering**: Implement multi-window aggregations for ML feature extraction
4. **Schema Design**: Support both instantaneous events and duration-based state

---

## 1. Time-Series Platform Patterns

### 1.1 Core Architecture Models

Modern time-series platforms follow several established patterns, each with distinct trade-offs:

#### Event-Driven Architecture (EDA)
Event-driven architectures represent all changes as discrete events. This is the foundation of [Home Assistant's data model](https://data.home-assistant.io/docs/data) where "everything that happens is represented as an event."

**Characteristics**:
- Immutable event log (append-only)
- Complete audit trail
- Natural fit for IoT sensor data
- Events carry context (who/what triggered them)

**Trade-offs**:
- Requires reconstruction for current state queries
- Higher query complexity for "what is the current value?"
- Natural for transitions, complex for continuous values

#### State-Based Architecture
Traditional approach storing the current value with timestamp.

**Characteristics**:
- Simple "current value" queries
- Lower storage for slowly-changing data
- Direct representation of reality

**Trade-offs**:
- Loses historical transition information
- Difficult to answer "when did it change?"
- No causality tracking

#### Hybrid Row-Columnar Storage
[TimescaleDB's hypercore](https://github.com/timescale/timescaledb) represents modern hybrid approaches:

> "Data is inserted in row format in the rowstore and converted to columnar format in the columnstore based on configuration."

**Benefits**:
- Fast writes (row-based)
- Fast analytics (columnar)
- 90%+ compression on historical data
- Seamless transition between hot and cold data

### 1.2 Time-Series Database Selection Criteria

Based on [ClickHouse's TSDB guide](https://clickhouse.com/resources/engineering/what-is-time-series-database) and current NDP architecture:

| Criteria | Current NDP (Bronze) | Recommended Addition |
|----------|---------------------|---------------------|
| Write Pattern | Append-only Parquet | Keep (works well) |
| Query Pattern | DuckDB virtual views | Add event-specific views |
| Data Model | Continuous metrics | Extend for discrete events |
| Compression | Parquet columnar | Excellent for events |
| Time Bucketing | 10-minute alignment | Add event-aware bucketing |

### 1.3 TSDB + Data Lake Hybrid Pattern

The [InfluxData hybrid model](https://www.influxdata.com/blog/TSDB-data-lakes-together/) aligns with NDP's existing Bronze/Silver architecture:

> "In a hybrid model, raw time series data is initially captured and stored in a TSDB, taking advantage of its optimized performance for real-time analytics and immediate data processing."

**NDP Alignment**:
```
Bronze Layer (Raw Parquet) = Data Lake storage
Silver Layer (DuckDB Views) = Query-time TSDB semantics
```

This means NDP can store event data in the same Bronze layer pattern while providing TSDB-like query capabilities through Silver views.

---

## 2. Event/State Hybrid Data Approaches

### 2.1 Home Assistant Data Model Analysis

Based on [Home Assistant's state documentation](https://data.home-assistant.io/docs/states):

#### State Record Structure
```
states table:
  - state_id (PK)
  - entity_id (FK to states_meta)
  - state (string value: "on", "off", "open", "closed")
  - attributes (JSON blob)
  - last_changed_ts (when STATE value changed)
  - last_updated_ts (when anything changed, incl. attributes)
  - old_state_id (FK for state chain)
  - context_id, context_user_id, context_parent_id
```

#### Key Insight: Dual Timestamp Pattern
> "last_changed_ts only updates when the state value was changed while last_updated_ts is updated on any change to the state, even if that included just attributes."

This distinction is critical for home events:
- **State change** (window opens): Both timestamps update
- **Attribute change** (temperature reading while window open): Only `last_updated_ts`

### 2.2 Event Sourcing vs CRUD for IoT

From [Sense Tecnic's CQRS/Event Sourcing analysis](http://sensetecnic.com/cqrs-and-event-sourcing-for-the-iot/):

> "IoT applications often need to capture the state of things over time such as sensor data values for historical data analysis. The event store does this automatically."

**Event Sourcing Benefits for Home Events**:
1. Complete history of all window state changes
2. Automatic temporal queries ("when was the window last open?")
3. Causal tracking (why did the window state change?)
4. Natural integration with existing continuous sensor data

**When CRUD is Better** ([RisingStack analysis](https://blog.risingstack.com/event-sourcing-vs-crud/)):
> "CRUD is useful if the data to be stored does not contain any semantics because it is only raw data."

For window events, the data IS semantic (the event means something), so event-style storage is appropriate.

### 2.3 Recommended Hybrid Pattern for NDP

Based on research, recommend a **Dual-Table Event-State Pattern**:

#### Table 1: Event Log (Append-Only)
```yaml
stream_id: "home-events"
fields:
  entity_id:
    type: string
    description: "Home Assistant entity ID (e.g., binary_sensor.living_room_window)"
  event_type:
    type: string
    description: "Event type (state_changed, attribute_updated)"
  old_state:
    type: string
    nullable: true
    description: "Previous state value"
  new_state:
    type: string
    description: "New state value"
  attributes:
    type: json
    nullable: true
    description: "Attribute snapshot at event time"
  context_id:
    type: string
    nullable: true
    description: "HA context for causal tracking"
  trigger_source:
    type: string
    nullable: true
    description: "What triggered this event (user, automation, sensor)"
```

#### View: Current State (Derived)
```sql
-- Silver view: current_home_state
SELECT DISTINCT ON (entity_id)
  entity_id,
  new_state as current_state,
  timestamp as state_since,
  attributes
FROM home_events
WHERE event_type = 'state_changed'
ORDER BY entity_id, timestamp DESC;
```

#### View: State Duration (Derived)
```sql
-- Silver view: state_durations
SELECT
  entity_id,
  old_state,
  new_state,
  timestamp as transition_time,
  LEAD(timestamp) OVER (PARTITION BY entity_id ORDER BY timestamp) - timestamp as duration
FROM home_events
WHERE event_type = 'state_changed';
```

---

## 3. Feature Engineering for Neural Prediction

### 3.1 ML Features for Window Timing Prediction

The goal is predicting optimal window open/close timing based on:
- Current and forecasted air quality
- Indoor/outdoor temperature differential
- Historical window behavior patterns
- Time of day and day of week patterns

Based on [feature engineering best practices](https://dotdata.com/blog/practical-guide-for-feature-engineering-of-time-series-data/) and [Statsig's time-series guide](https://www.statsig.com/perspectives/feature-engineering-timeseries):

#### Feature Categories

**1. Lag Features (Temporal Memory)**
```rust
// Past window states
window_was_open_1h_ago: bool    // State 1 hour ago
window_was_open_24h_ago: bool   // Same time yesterday
window_open_count_24h: u32      // How many times opened today
last_window_duration_mins: f64  // Duration of last open period
```

**2. Rolling Window Statistics**
```rust
// Air quality context when window was last opened
pm25_at_last_open: f64
temp_diff_at_last_open: f64
outdoor_aqi_at_last_open: u8

// Aggregations
avg_open_duration_7d: f64       // Average open period this week
typical_first_open_time: f64    // What time usually first opened
```

**3. Expanding Window Features**
```rust
// Cumulative patterns
total_open_duration_today: f64  // Minutes open so far today
windows_opened_this_week: u32   // Count this week
```

**4. Time-Based Features**
```rust
hour_of_day: u8                 // 0-23
day_of_week: u8                 // 0-6
is_weekend: bool
is_typical_open_time: bool      // Based on historical patterns
```

**5. Cross-Stream Context Features**
```rust
// From air-quality stream
indoor_pm25_current: f64
indoor_temp_current: f64
indoor_co2_current: u16

// From outdoor streams
outdoor_pm25_current: f64
outdoor_temp_current: f64
outdoor_aqi: u8

// Derived differentials
temp_differential: f64          // outdoor - indoor
pm25_ratio: f64                 // indoor/outdoor
```

### 3.2 Research on Neural Prediction for Home Automation

From [research on ML-powered home scenes](https://www.wevolver.com/article/machine-learning-powered-home-scenes-a-blueprint-for-intelligent-home-automation):

> "The implementation of ML-based scenes hinges on the collection and processing of large volumes of data about operations, triggers, and actions."

From [indoor temperature forecasting research](https://www.nature.com/articles/s41598-024-85026-3):

> "Long Short-Term Memory (LSTM) networks for time-series modeling significantly enhance the capture of temporal dependencies in temperature predictions."

**Model Architecture Recommendations**:

1. **Short-term (next hour) prediction**: Random Forest or Gradient Boosting
   - Features: Current conditions, recent patterns, time of day
   - Output: Probability of "should open window now"

2. **Pattern learning (when typically open)**: LSTM/Transformer
   - Features: Multi-day sequences of window states with conditions
   - Output: Optimal opening/closing schedule

3. **Comfort prediction**: Neural Network
   - Features: All sensor readings, window state, external conditions
   - Output: Predicted indoor comfort score

### 3.3 Feature Store Design for NDP

```yaml
# config/features/home-events-features.yaml
features:
  # Event-derived features
  - name: window_state_current
    description: "Current window state (0=closed, 1=open)"
    type: numeric
    source_streams: [home-events]
    aggregation: last

  - name: window_open_duration_current
    description: "How long current open period has lasted (minutes)"
    type: numeric
    source_streams: [home-events]
    calculation: "now() - last_state_change WHERE current_state='open'"

  - name: window_transitions_24h
    description: "Number of state changes in past 24 hours"
    type: numeric
    source_streams: [home-events]
    window: 24h
    aggregation: count
    filter: "event_type='state_changed'"

  # Cross-stream features
  - name: temp_diff_at_last_transition
    description: "Indoor-outdoor temp difference at last window change"
    type: numeric
    source_streams: [home-events, air-quality, outdoor-weather]
    calculation: "air_quality.temperature - outdoor_weather.temperature AT home_events.timestamp"

  - name: pm25_improvement_expected
    description: "Expected PM2.5 change if window state toggled"
    type: numeric
    source_streams: [air-quality, outdoor-air-quality, home-events]
    calculation: "model_prediction(indoor_pm25, outdoor_pm25, window_state)"
```

---

## 4. Windowing Strategies for Home Events

### 4.1 Time Bucket Patterns

From [Google Cloud's time-series schema design](https://cloud.google.com/bigtable/docs/schema-design-time-series):

> "In a time bucket pattern, each row represents a 'bucket' of time such as an hour, day, or month."

**Recommended Bucketing Strategy for Home Events**:

| Analysis Type | Bucket Size | Use Case |
|---------------|-------------|----------|
| Real-time | 1 minute | Dashboard current state |
| Short-term | 15 minutes | Pattern detection, anomaly |
| Daily patterns | 1 hour | Typical behavior analysis |
| Weekly patterns | 1 day | Long-term trends |
| Seasonal | 1 week | Year-over-year comparison |

### 4.2 DuckDB View Implementation

```sql
-- Time-bucketed event summary
CREATE VIEW home_events_hourly AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    entity_id,
    COUNT(*) FILTER (WHERE event_type = 'state_changed') AS transitions,
    SUM(CASE WHEN new_state = 'open' THEN 1 ELSE 0 END) AS open_events,
    SUM(CASE WHEN new_state = 'closed' THEN 1 ELSE 0 END) AS close_events,
    -- Duration calculation (complex: requires window functions)
    SUM(CASE
        WHEN new_state = 'closed' THEN
            EXTRACT(EPOCH FROM (timestamp - LAG(timestamp) OVER (
                PARTITION BY entity_id ORDER BY timestamp
            ))) / 60.0
        ELSE 0
    END) AS total_open_minutes
FROM read_parquet('/data/home-events/*.parquet')
WHERE entity_id LIKE 'binary_sensor.%_window'
GROUP BY bucket, entity_id;
```

### 4.3 Real-Time Window Aggregations

From [Tecton's time window aggregations](https://www.tecton.ai/product/predictive-ml/time-window-aggregations/):

> "Time window aggregations allow features to have extreme freshness, enabling up-to-the-second aggregations."

For NDP, implement via Rust rolling windows:

```rust
pub struct HomeEventAggregator {
    // Per-entity state tracking
    entity_states: HashMap<String, EntityState>,

    // Rolling windows
    transitions_1h: RollingWindow<u32>,
    open_duration_1h: RollingWindow<Duration>,
}

struct EntityState {
    current_state: String,
    state_since: DateTime<Utc>,
    last_transition: DateTime<Utc>,
}

impl HomeEventAggregator {
    pub fn process_event(&mut self, event: &HomeEvent) -> FeatureVector {
        // Update entity state
        let entity = self.entity_states
            .entry(event.entity_id.clone())
            .or_insert_with(|| EntityState::default());

        let was_open = entity.current_state == "open";
        let now_open = event.new_state == "open";

        // Calculate duration if closing
        if was_open && !now_open {
            let duration = event.timestamp - entity.state_since;
            self.open_duration_1h.add(duration, event.timestamp);
        }

        // Update state
        entity.current_state = event.new_state.clone();
        entity.state_since = event.timestamp;
        entity.last_transition = event.timestamp;

        // Increment transition counter
        self.transitions_1h.add(1, event.timestamp);

        // Generate features
        FeatureVector {
            window_is_open: now_open,
            current_open_duration: if now_open {
                Some(Utc::now() - entity.state_since)
            } else {
                None
            },
            transitions_1h: self.transitions_1h.sum(),
            avg_open_duration_1h: self.open_duration_1h.mean(),
            time_since_last_transition: Utc::now() - entity.last_transition,
        }
    }
}
```

---

## 5. Schema Design Recommendations

### 5.1 Stream Configuration for Home Events

```yaml
# config/base/streams/home-events/config.yaml
stream_id: "home-events"
description: "Home Assistant state change events"
version: "1.0.0"
enabled: true
retention_days: 730  # 2 years for ML training
compression_after_days: 7
partitioning_strategy: "daily"

fields:
  # Entity identification
  entity_id:
    type: "string"
    description: "Home Assistant entity ID"
    nullable: false

  entity_domain:
    type: "string"
    description: "Entity domain (binary_sensor, sensor, switch)"
    nullable: false

  friendly_name:
    type: "string"
    description: "Human-readable entity name"
    nullable: true

  # Event data
  event_type:
    type: "string"
    description: "Event type (state_changed, attribute_updated)"
    nullable: false

  old_state:
    type: "string"
    description: "Previous state value"
    nullable: true

  new_state:
    type: "string"
    description: "New state value"
    nullable: false

  # Attributes (for future extensibility)
  attributes:
    type: "json"
    description: "Additional entity attributes at event time"
    nullable: true

  # Causality tracking
  context_id:
    type: "string"
    description: "Home Assistant context ID for event chain"
    nullable: true

  trigger_source:
    type: "string"
    description: "What triggered this event"
    nullable: true

sources:
  - type: webhook  # or mqtt depending on HA integration
    enabled: true
    params:
      endpoint: "/api/v1/home-events/ingest"
      auth_header: "X-HA-Webhook-Token"
      auth_value: "${HOME_ASSISTANT_WEBHOOK_TOKEN}"
    parser:
      parser_type: home_assistant_event
      entity_filter: "binary_sensor.*_window,binary_sensor.*_door"
```

### 5.2 Supporting Both Event and State Queries

The schema above supports both query patterns:

**Event Queries** (What happened?):
```sql
-- All window events today
SELECT timestamp, entity_id, old_state, new_state
FROM home_events
WHERE entity_domain = 'binary_sensor'
  AND entity_id LIKE '%window%'
  AND timestamp > now() - INTERVAL '1 day'
ORDER BY timestamp;
```

**State Queries** (What is the current state?):
```sql
-- Current state of all windows (Silver view)
SELECT DISTINCT ON (entity_id)
    entity_id,
    friendly_name,
    new_state as current_state,
    timestamp as state_since,
    now() - timestamp as duration
FROM home_events
WHERE entity_domain = 'binary_sensor'
  AND entity_id LIKE '%window%'
ORDER BY entity_id, timestamp DESC;
```

---

## 6. Comparison: Log Streams vs Home Events

The scope mentions considering "completely different category of data (such as log streams from systems)." Here's how the architecture generalizes:

| Aspect | Home Events | System Logs | Generalization |
|--------|-------------|-------------|----------------|
| Event Type | State transitions | Log entries | Discrete events |
| Temporal Pattern | Irregular (user-driven) | Bursty (system activity) | Flexible ingestion |
| State Derivation | Current state matters | Aggregate patterns matter | Configurable views |
| Retention | Long (ML training) | Medium (debugging) | Per-stream policy |
| Query Pattern | "What is open now?" | "How many errors today?" | Aggregation + latest |

### Unified Event Schema Pattern

```yaml
# Generic event stream pattern
stream_id: "{domain}-events"
fields:
  # Universal fields
  event_id:
    type: "string"
    description: "Unique event identifier"

  event_type:
    type: "string"
    description: "Event classification"

  source_id:
    type: "string"
    description: "Origin of the event"

  # Domain-specific payload
  payload:
    type: "json"
    description: "Event-specific data"

  # Optional state tracking
  previous_state:
    type: "json"
    nullable: true

  current_state:
    type: "json"
    nullable: true

  # Context/causality
  correlation_id:
    type: "string"
    nullable: true
    description: "Links related events"

  parent_event_id:
    type: "string"
    nullable: true
    description: "Causal parent event"
```

---

## 7. Home Assistant Integration Considerations

### 7.1 Data Access Patterns

Home Assistant provides several integration options:

1. **REST API**: Direct state queries
2. **WebSocket API**: Real-time event streaming
3. **MQTT Integration**: Publish state changes to MQTT
4. **Webhook Automations**: Push events to NDP endpoint

**Recommended for NDP**: MQTT or Webhook
- Both are already supported in the Source architecture
- Low latency event delivery
- Decoupled from HA availability

### 7.2 Home Assistant Data Science Portal Value

The [data.home-assistant.io](https://data.home-assistant.io) portal provides:

**Benefits**:
- SQL access to HA's SQLite database
- Built-in Jupyter notebook integration
- Pre-built visualizations

**Limitations for NDP**:
- Tied to Home Assistant's infrastructure
- Limited to HA's data model
- No integration with external data (weather APIs, etc.)

**Recommendation**: Use HA Data Science portal for initial exploration, but build long-term analytics in NDP for:
- Cross-stream correlation (home events + air quality + weather)
- Custom ML pipelines
- Longer retention than HA's defaults
- Integration with neural prediction models

---

## 8. Implementation Roadmap

### Phase 1: Data Collection (AIR-008)
1. Define `home-events` stream schema
2. Implement Home Assistant webhook source OR MQTT bridge
3. Create `HomeAssistantEventParser`
4. Deploy and validate data collection

### Phase 2: Silver Views (DP-002)
1. Create `current_home_state` view
2. Create `state_durations` view
3. Create `home_events_hourly` aggregation view
4. Integrate with Grafana dashboards

### Phase 3: Feature Engineering (FE-001)
1. Implement `HomeEventAggregator` rolling windows
2. Create cross-stream feature joins
3. Build feature store entries for ML

### Phase 4: Neural Prediction (ML-001)
1. Train initial model (Random Forest/XGBoost)
2. Evaluate LSTM for pattern learning
3. Deploy inference endpoint
4. Create recommendation UI/notification

---

## 9. References

### Time-Series Architecture
- [ClickHouse TSDB Guide](https://clickhouse.com/resources/engineering/what-is-time-series-database)
- [TimescaleDB](https://github.com/timescale/timescaledb)
- [InfluxData Hybrid Architecture](https://www.influxdata.com/blog/TSDB-data-lakes-together/)
- [Google Cloud Time-Series Schema Design](https://cloud.google.com/bigtable/docs/schema-design-time-series)

### Event Sourcing & State Management
- [CQRS and Event Sourcing for IoT](http://sensetecnic.com/cqrs-and-event-sourcing-for-the-iot/)
- [Microsoft Event Sourcing Pattern](https://learn.microsoft.com/en-us/azure/architecture/patterns/event-sourcing)
- [Event Sourcing vs CRUD](https://blog.risingstack.com/event-sourcing-vs-crud/)
- [Event-Driven vs State-Based](https://developer.confluent.io/courses/event-sourcing/event-driven-vs-state-based/)

### Home Assistant
- [Home Assistant Data Portal](https://data.home-assistant.io)
- [Home Assistant States Documentation](https://data.home-assistant.io/docs/states)

### Feature Engineering
- [Practical Guide for Time-Series Feature Engineering](https://dotdata.com/blog/practical-guide-for-feature-engineering-of-time-series-data/)
- [Statsig Time-Series Feature Engineering](https://www.statsig.com/perspectives/feature-engineering-timeseries)
- [Tecton Time Window Aggregations](https://www.tecton.ai/product/predictive-ml/time-window-aggregations/)
- [Azure AutoML Lag Features](https://learn.microsoft.com/en-us/azure/machine-learning/concept-automl-forecasting-lags)

### Neural Prediction for Smart Homes
- [ML-Powered Home Scenes](https://www.wevolver.com/article/machine-learning-powered-home-scenes-a-blueprint-for-intelligent-home-automation)
- [Indoor Temperature Forecasting with LSTM](https://www.nature.com/articles/s41598-024-85026-3)
- [IoT Occupancy Estimation with ML](https://www.sciencedirect.com/science/article/abs/pii/S0360132323010065)

---

## 10. Appendix: NDP Architecture Alignment

This research aligns with existing NDP patterns:

| NDP Component | Home Events Integration |
|---------------|------------------------|
| Bronze Layer (Parquet) | Store raw events as-is |
| Silver Layer (DuckDB) | Derived state views, aggregations |
| Source Trait | WebhookHandler or MqttSource |
| ResponseParser | HomeAssistantEventParser |
| IngestionCoordinator | Route to home-events stream |
| Feature Store | Cross-stream ML features |

The recommended approach extends rather than changes the existing architecture, maintaining consistency with established patterns while adding event/state hybrid capabilities.
