# SPEC-E02: Events Hypertable & Unified Events View

> **Feature ID:** v11-013
> **Priority:** Critical
> **Status:** Specification (Updated 2026-02-05)
> **Dependencies:** v11-006 (State Transitions), v11-012 (Threshold Crossings)
> **Blocks:** V1.2 Pattern Detection Engine

---

## Revision History

| Date | Change | Rationale |
|------|--------|-----------|
| 2026-02-05 | **MAJOR: Events Hypertable approach** | Enables CA on events, captures correlation context |
| 2026-02-04 | Initial specification | UNION ALL view approach |

---

## User Story

**As a** pattern detection system (V1.2),
**I want** a dedicated events hypertable with environmental context captured at event time,
**So that** I can correlate events with surrounding conditions and efficiently aggregate event data.

---

## Goal

Create `gold.events` - a dedicated TimescaleDB hypertable that:
1. Stores all events (state transitions, threshold crossings) as first-class citizens
2. Captures **environmental context** at event time for correlation analysis
3. Enables **continuous aggregates** on events (solving the CA-on-view limitation)
4. Provides a unified schema for all event types
5. Serves as the PRIMARY event interface for V1.2 Pattern Detection

**Key Insight**: Making events a hypertable (not a derived view) enables:
- TimescaleDB continuous aggregates for efficient hourly summaries
- Context snapshots for correlation without additional joins
- Direct INSERTs from detection jobs for real-time event capture
- Proper indexing for V1.2 query patterns

---

## Architecture Decision: Events Hypertable

### Previous Approach (Superseded)

```
gold.events_unified (VIEW)
    ├── UNION ALL of state_transitions
    └── UNION ALL of threshold_crossings

gold.events_hourly (CONTINUOUS AGGREGATE) ← FAILS: CA requires hypertable
```

### New Approach (Adopted)

```
gold.events (HYPERTABLE)
    ↑
    │ INSERT events via TimescaleDB job
    │
    ├── State transition detector
    └── Threshold crossing detector

gold.events_unified (VIEW)
    └── Simple SELECT * FROM gold.events

gold.events_hourly (CONTINUOUS AGGREGATE) ← WORKS: CA on hypertable
    └── Aggregates from gold.events
```

**Rationale:**
1. TimescaleDB continuous aggregates only work on hypertables
2. Event sourcing pattern - events are facts, not derived data
3. Context capture enables correlation without joins
4. Aligns with V1.2 Pattern Detection requirements

---

## Functional Requirements

### FR-E02-001: Events Hypertable Schema

The system SHALL create `gold.events` as a TimescaleDB hypertable with this schema:

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `event_id` | UUID | NOT NULL | Unique event identifier (PK) |
| `event_time` | TIMESTAMPTZ | NOT NULL | When the event occurred |
| `stream_id` | TEXT | NOT NULL | Source stream identifier |
| `entity_id` | TEXT | NOT NULL | Entity identifier (ndp_id) |
| `event_type` | TEXT | NOT NULL | Event type enum value |
| `from_state` | TEXT | NULL | Previous state (state transitions only) |
| `to_state` | TEXT | NULL | New state (state transitions only) |
| `duration_in_state_ms` | BIGINT | NULL | Time in previous state (ms) |
| `metric` | TEXT | NULL | Metric name (threshold crossings only) |
| `threshold_value` | DOUBLE PRECISION | NULL | Threshold crossed |
| `crossing_direction` | TEXT | NULL | 'rising', 'falling', etc. |
| `metric_value` | DOUBLE PRECISION | NULL | Value at crossing |
| `previous_metric_value` | DOUBLE PRECISION | NULL | Previous value |
| `objective_id` | TEXT | NULL | Objective reference |
| `context` | JSONB | NOT NULL | Environmental snapshot at event time |
| `details` | JSONB | NOT NULL | Extensible event details |

### FR-E02-002: Hypertable Configuration

```sql
SELECT create_hypertable('gold.events', 'event_time',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);
```

**Chunk interval:** 7 days balances query performance with chunk management overhead on Pi.

### FR-E02-003: Event Type Enumeration

Supported event types for V1.1:

| Event Type | Source | Description |
|------------|--------|-------------|
| `state_transition` | v11-006 + detection job | State field value changed |
| `threshold_crossing` | v11-012 + detection job | Metric crossed objective threshold |

Future event types (V1.2+):
- `anomaly` - Statistical anomaly detected
- `trend_change` - Significant trend direction change

### FR-E02-004: Context Snapshot

The `context` JSONB column SHALL capture environmental state at event time:

```json
{
  "indoor_co2": 823,
  "indoor_pm25": 8.2,
  "indoor_temp": 72.1,
  "indoor_humidity": 45,
  "outdoor_temp": 65.2,
  "outdoor_pm25": 12.1,
  "outdoor_aqi": 42,
  "window_state": "on",
  "time_since_last_window_change_ms": 7200000
}
```

Context is sourced from `gold.indoor_air_quality_aligned` at the event's hourly bucket.

### FR-E02-005: Event Detection Job

A TimescaleDB scheduled job SHALL detect and insert new events:

```sql
SELECT add_job('gold.detect_events', '15 minutes');
```

The job:
1. Identifies new state transitions since last run
2. Identifies new threshold crossings since last run
3. Captures context from aligned view
4. INSERTs events into gold.events

### FR-E02-006: Unified Events View

For API compatibility, `gold.events_unified` SHALL be a simple view:

```sql
CREATE VIEW gold.events_unified AS
SELECT * FROM gold.events;
```

This provides the V1.2 contract interface while events are stored in the hypertable.

### FR-E02-007: Hourly Events Aggregate (Continuous Aggregate)

Now that events are in a hypertable, we CAN create a continuous aggregate:

```sql
CREATE MATERIALIZED VIEW gold.events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,
    COUNT(DISTINCT entity_id) AS distinct_entities_with_events
FROM gold.events
GROUP BY bucket;

SELECT add_continuous_aggregate_policy('gold.events_hourly',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes'
);
```

### FR-E02-008: Index Strategy

Create indexes for V1.2 query patterns:

```sql
-- Primary access: time range
CREATE INDEX idx_events_time ON gold.events (event_time DESC);

-- Filter by type + time
CREATE INDEX idx_events_type_time ON gold.events (event_type, event_time DESC);

-- Filter by entity + time
CREATE INDEX idx_events_entity_time ON gold.events (entity_id, event_time DESC);

-- Filter by objective (threshold crossings)
CREATE INDEX idx_events_objective ON gold.events (objective_id, event_time DESC)
    WHERE event_type = 'threshold_crossing';

-- Context queries (GIN for flexible JSONB)
CREATE INDEX idx_events_context ON gold.events USING GIN (context);

-- Details queries
CREATE INDEX idx_events_details ON gold.events USING GIN (details);
```

### FR-E02-009: Retention Policy

Events older than 1 year SHALL be automatically dropped:

```sql
SELECT add_retention_policy('gold.events', INTERVAL '1 year');
```

### FR-E02-010: Domain Scoping

Events include `stream_id` for domain filtering:

```sql
-- Domain-scoped view (optional)
CREATE VIEW gold.indoor_air_quality_events AS
SELECT * FROM gold.events
WHERE stream_id IN ('air-quality', 'home-assistant-state', 'outdoor-weather', 'outdoor-air-quality');
```

---

## Non-Functional Requirements

### NFR-E02-001: Query Performance

| Query | Target | Rationale |
|-------|--------|-----------|
| All events in 30-day range | < 100ms | Indexed on event_time |
| Events filtered by type | < 50ms | Composite index |
| Hourly aggregate query | < 20ms | Continuous aggregate |
| Context-based filter | < 200ms | GIN index on JSONB |

### NFR-E02-002: Storage Efficiency

| Metric | Target | Notes |
|--------|--------|-------|
| Events per day (typical) | < 100 | State + threshold events |
| Storage per event | ~500 bytes | With context snapshot |
| 1-year storage | < 20 MB | Well within Pi constraints |

### NFR-E02-003: Job Execution

| Metric | Target |
|--------|--------|
| Detection job runtime | < 5 seconds |
| Job schedule | Every 15 minutes |
| Event latency | < 15 minutes from source data |

---

## Acceptance Criteria

### AC-E02-001: Hypertable Created

```gherkin
Scenario: Events hypertable is created correctly
  Given ndp-gold-ddl generates events table SQL
  When SQL is executed on TimescaleDB
  Then gold.events EXISTS as a hypertable
  And chunk_time_interval = '7 days'
```

### AC-E02-002: State Transitions Inserted

```gherkin
Scenario: State transitions are inserted into events table
  Given a state transition detected in silver.home_assistant_state
  And the detection job runs
  When I query gold.events
  Then the state transition event EXISTS
  And event_type = 'state_transition'
  And from_state, to_state, duration_in_state_ms are populated
  And context contains environmental snapshot
```

### AC-E02-003: Threshold Crossings Inserted

```gherkin
Scenario: Threshold crossings are inserted into events table
  Given a metric crosses an objective threshold
  And the detection job runs
  When I query gold.events
  Then the threshold crossing event EXISTS
  And event_type = 'threshold_crossing'
  And metric, threshold_value, crossing_direction are populated
  And context contains environmental snapshot
```

### AC-E02-004: Context Captured

```gherkin
Scenario: Context snapshot is captured at event time
  Given a state transition event at 10:30
  And indoor CO2 at 10:00 bucket was 823 ppm
  When I query the event's context
  Then context->>'indoor_co2' = '823'
  And context contains all aligned view metrics
```

### AC-E02-005: Continuous Aggregate Works

```gherkin
Scenario: Hourly events aggregate updates correctly
  Given 5 events in hour 10:00
  And 3 events in hour 11:00
  When I query gold.events_hourly
  Then bucket 10:00 has total_events = 5
  And bucket 11:00 has total_events = 3
  And the CA refreshes automatically
```

### AC-E02-006: V1.2 Query Patterns

```gherkin
Scenario: V1.2 can query events with context
  Given threshold crossing where CO2 crossed 800
  And window was closed at event time
  When V1.2 queries: SELECT * FROM gold.events WHERE event_type = 'threshold_crossing'
  Then event is returned with context->>'window_state' = 'off'
```

### AC-E02-007: Correlation Query Works

```gherkin
Scenario: V1.2 can correlate events with context
  Given threshold crossing for CO2 at time T
  When V1.2 queries for context at event time
  Then all stream values are available in context column
  And no additional JOIN to aligned view required
```

---

## SQL Generation

### Events Hypertable DDL

```sql
-- Generated by ndp-gold-ddl for events hypertable
-- Domain: indoor-air-quality

CREATE SCHEMA IF NOT EXISTS gold;

-- Events hypertable
CREATE TABLE IF NOT EXISTS gold.events (
    -- Identity
    event_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL,

    -- Event classification
    stream_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,

    -- State transition fields (NULL for threshold crossings)
    from_state TEXT,
    to_state TEXT,
    duration_in_state_ms BIGINT,

    -- Threshold crossing fields (NULL for state transitions)
    metric TEXT,
    threshold_value DOUBLE PRECISION,
    crossing_direction TEXT,
    metric_value DOUBLE PRECISION,
    previous_metric_value DOUBLE PRECISION,
    objective_id TEXT,

    -- Context snapshot at event time (for correlation)
    context JSONB NOT NULL DEFAULT '{}',

    -- Extensible details
    details JSONB NOT NULL DEFAULT '{}'
);

-- Convert to hypertable
SELECT create_hypertable('gold.events', 'event_time',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Indexes for V1.2 query patterns
CREATE INDEX IF NOT EXISTS idx_events_time
    ON gold.events (event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_type_time
    ON gold.events (event_type, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_entity_time
    ON gold.events (entity_id, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_events_objective
    ON gold.events (objective_id, event_time DESC)
    WHERE event_type = 'threshold_crossing';
CREATE INDEX IF NOT EXISTS idx_events_context
    ON gold.events USING GIN (context);
CREATE INDEX IF NOT EXISTS idx_events_details
    ON gold.events USING GIN (details);

-- Retention policy (1 year)
SELECT add_retention_policy('gold.events', INTERVAL '1 year', if_not_exists => TRUE);

-- Comment for documentation
COMMENT ON TABLE gold.events IS
    'Events hypertable: state transitions and threshold crossings with context snapshots. For V1.2 Pattern Detection.';
```

### Unified Events View (V1.2 API)

```sql
-- Unified events view for V1.2 API compatibility
CREATE OR REPLACE VIEW gold.events_unified AS
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    -- Build details JSONB for backward compatibility
    CASE event_type
        WHEN 'state_transition' THEN
            jsonb_build_object(
                'from_state', from_state,
                'to_state', to_state,
                'duration_in_previous_ms', duration_in_state_ms
            )
        WHEN 'threshold_crossing' THEN
            jsonb_build_object(
                'metric', metric,
                'threshold', threshold_value,
                'direction', crossing_direction,
                'value', metric_value,
                'previous_value', previous_metric_value,
                'objective_id', objective_id
            )
        ELSE details
    END AS details,
    context
FROM gold.events
ORDER BY event_time, event_type, event_id;

COMMENT ON VIEW gold.events_unified IS
    'V1.2 API view on events hypertable. Provides backward-compatible schema.';
```

### Hourly Events Continuous Aggregate

```sql
-- Hourly events aggregate (NOW WORKS - on hypertable!)
CREATE MATERIALIZED VIEW gold.events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,
    COUNT(DISTINCT entity_id) AS distinct_entities_with_events
FROM gold.events
GROUP BY bucket
WITH NO DATA;

-- Refresh policy
SELECT add_continuous_aggregate_policy('gold.events_hourly',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes',
    if_not_exists => TRUE
);

-- Index for time range queries
CREATE INDEX IF NOT EXISTS idx_events_hourly_bucket
    ON gold.events_hourly (bucket DESC);
```

### Event Detection Procedure

```sql
-- Event detection procedure (runs as TimescaleDB job)
CREATE OR REPLACE PROCEDURE gold.detect_events(job_id INT, config JSONB)
LANGUAGE plpgsql AS $$
DECLARE
    last_run TIMESTAMPTZ;
    events_inserted INT := 0;
BEGIN
    -- Get last successful run time
    SELECT last_successful_finish INTO last_run
    FROM timescaledb_information.job_stats
    WHERE job_id = detect_events.job_id;

    -- Default to 2 hours ago if first run
    last_run := COALESCE(last_run, NOW() - INTERVAL '2 hours');

    -- Insert new state transition events
    WITH new_transitions AS (
        SELECT
            s.time AS event_time,
            'home-assistant-state' AS stream_id,
            s.entity_id,
            'state_transition' AS event_type,
            LAG(s.state) OVER (PARTITION BY s.entity_id ORDER BY s.time) AS from_state,
            s.state AS to_state,
            EXTRACT(EPOCH FROM (s.time - LAG(s.time) OVER (PARTITION BY s.entity_id ORDER BY s.time))) * 1000 AS duration_ms
        FROM silver.home_assistant_state s
        WHERE s.time > last_run
    ),
    actual_transitions AS (
        SELECT * FROM new_transitions
        WHERE from_state IS NOT NULL
          AND from_state != to_state
    )
    INSERT INTO gold.events (
        event_time, stream_id, entity_id, event_type,
        from_state, to_state, duration_in_state_ms,
        context, details
    )
    SELECT
        t.event_time,
        t.stream_id,
        t.entity_id,
        t.event_type,
        t.from_state,
        t.to_state,
        t.duration_ms::BIGINT,
        -- Context from aligned view
        COALESCE(
            (SELECT jsonb_build_object(
                'indoor_co2', a.indoor_co2_mean,
                'indoor_pm25', a.indoor_pm25_mean,
                'indoor_temp', a.indoor_temp_mean,
                'outdoor_temp', a.outdoor_temp_mean,
                'outdoor_aqi', a.outdoor_aqi_mean,
                'window_state', a.window_last_state
            ) FROM gold.indoor_air_quality_aligned a
            WHERE a.bucket = time_bucket('1 hour', t.event_time)),
            '{}'::JSONB
        ),
        '{}'::JSONB
    FROM actual_transitions t;

    GET DIAGNOSTICS events_inserted = ROW_COUNT;
    RAISE NOTICE 'Inserted % state transition events', events_inserted;

    -- Insert new threshold crossing events
    -- (Similar pattern for threshold crossings using objectives comparison)

    COMMIT;
END;
$$;

-- Schedule the detection job (every 15 minutes)
SELECT add_job('gold.detect_events', '15 minutes');
```

---

## V1.2 Handoff Documentation

### Query Interface Contract

V1.2 Pattern Detection Engine should use these query patterns:

```sql
-- 1. Get recent events with context
SELECT
    event_id,
    event_time,
    event_type,
    context->>'indoor_co2' AS co2_at_event,
    context->>'window_state' AS window_at_event
FROM gold.events
WHERE event_time >= NOW() - INTERVAL '24 hours'
ORDER BY event_time;

-- 2. Correlation query: What was CO2 when windows opened?
SELECT
    event_time,
    (context->>'indoor_co2')::FLOAT AS co2_at_open,
    (context->>'indoor_pm25')::FLOAT AS pm25_at_open
FROM gold.events
WHERE event_type = 'state_transition'
  AND to_state = 'on'
ORDER BY event_time DESC;

-- 3. Time between CO2 crossing and window action
WITH co2_crossings AS (
    SELECT event_id, event_time, context->>'window_state' AS window_at_crossing
    FROM gold.events
    WHERE event_type = 'threshold_crossing'
      AND metric = 'co2'
      AND crossing_direction = 'rising'
),
window_opens AS (
    SELECT event_time
    FROM gold.events
    WHERE event_type = 'state_transition'
      AND to_state = 'on'
)
SELECT
    c.event_time AS co2_crossed,
    c.window_at_crossing,
    MIN(w.event_time) AS next_window_open,
    EXTRACT(EPOCH FROM (MIN(w.event_time) - c.event_time)) / 60 AS minutes_to_action
FROM co2_crossings c
LEFT JOIN window_opens w ON w.event_time > c.event_time
                        AND w.event_time < c.event_time + INTERVAL '2 hours'
GROUP BY c.event_id, c.event_time, c.window_at_crossing;

-- 4. Hourly summary with aligned metrics
SELECT
    a.bucket,
    a.indoor_co2_mean,
    a.indoor_pm25_mean,
    eh.total_events,
    eh.state_transition_count,
    eh.threshold_crossing_count
FROM gold.indoor_air_quality_aligned a
LEFT JOIN gold.events_hourly eh ON a.bucket = eh.bucket
WHERE a.bucket >= NOW() - INTERVAL '7 days'
ORDER BY a.bucket;
```

### Schema Contract

```typescript
// V1.2 should expect this structure
interface Event {
  event_id: string;          // UUID
  event_time: string;        // ISO 8601 timestamp
  stream_id: string;         // Source stream
  entity_id: string;         // Entity (sensor)
  event_type: 'state_transition' | 'threshold_crossing';

  // State transition fields (null for crossings)
  from_state?: string;
  to_state?: string;
  duration_in_state_ms?: number;

  // Threshold crossing fields (null for transitions)
  metric?: string;
  threshold_value?: number;
  crossing_direction?: 'rising' | 'falling' | 'entering_range' | 'exiting_range_low' | 'exiting_range_high';
  metric_value?: number;
  previous_metric_value?: number;
  objective_id?: string;

  // Context at event time (for correlation!)
  context: {
    indoor_co2?: number;
    indoor_pm25?: number;
    indoor_temp?: number;
    indoor_humidity?: number;
    outdoor_temp?: number;
    outdoor_pm25?: number;
    outdoor_aqi?: number;
    window_state?: string;
    [key: string]: any;  // Extensible
  };

  // Extensible details
  details: Record<string, any>;
}
```

---

## Configuration

### gold_etl Extension for Events

```yaml
# In stream config or domain config
events:
  enabled: true
  table_type: hypertable
  chunk_interval: "7 days"
  retention: "1 year"

  detection_job:
    schedule: "15 minutes"
    lookback: "2 hours"

  context_sources:
    - aligned_view: gold.indoor_air_quality_aligned
      fields:
        - indoor_co2_mean AS indoor_co2
        - indoor_pm25_mean AS indoor_pm25
        - indoor_temp_mean AS indoor_temp
        - outdoor_temp_mean AS outdoor_temp
        - outdoor_aqi_mean AS outdoor_aqi
        - window_last_state AS window_state
```

### Deploy Manifest

```json
{
  "version": "1.2.0",
  "declarations": {
    "events": [
      {
        "domain_id": "indoor-air-quality",
        "action": "sync",
        "components": ["hypertable", "unified-view", "hourly-aggregate", "detection-job"]
      }
    ]
  }
}
```

---

## ndp-gold-ddl Changes Required

### New Generator: EventsHypertableGenerator

```rust
pub trait EventsHypertableGenerator {
    /// Generate SQL for events hypertable
    fn generate_events_hypertable(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;

    /// Generate SQL for unified events view
    fn generate_unified_view(&self) -> Result<String, GeneratorError>;

    /// Generate SQL for hourly events CA
    fn generate_hourly_aggregate(&self) -> Result<String, GeneratorError>;

    /// Generate SQL for event detection procedure
    fn generate_detection_procedure(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;

    /// Generate SQL for detection job
    fn generate_detection_job(&self, schedule: &str) -> Result<String, GeneratorError>;
}
```

### New Config Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsConfig {
    pub enabled: bool,
    pub table_type: GoldTableType,  // Hypertable
    pub chunk_interval: String,
    pub retention: Option<String>,
    pub detection_job: DetectionJobConfig,
    pub context_sources: Vec<ContextSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionJobConfig {
    pub schedule: String,  // e.g., "15 minutes"
    pub lookback: String,  // e.g., "2 hours"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSource {
    pub aligned_view: String,
    pub fields: Vec<String>,
}
```

---

## Migration from Previous Approach

If SPEC-E02 v1 (UNION ALL view) was implemented:

```sql
-- Drop old views
DROP VIEW IF EXISTS gold.events_unified CASCADE;
DROP MATERIALIZED VIEW IF EXISTS gold.events_hourly CASCADE;

-- Create new hypertable (events table)
-- ... (as specified above)

-- Migrate existing events (if any)
INSERT INTO gold.events (...)
SELECT ... FROM old_state_transitions_view
UNION ALL
SELECT ... FROM old_threshold_crossings_view;
```

---

## References

- [SCOPE.md](../../SCOPE.md) - v11-013 description
- [SPEC-E01](./SPEC-E01-threshold-crossings.md) - Threshold crossing events
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [PHASE-E-OVERVIEW.md](./PHASE-E-OVERVIEW.md) - Phase E context

---

*SPEC-E02 updated: 2026-02-05 - Events Hypertable approach adopted*
