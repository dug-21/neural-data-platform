# SPEC-E01: Threshold Crossing Generator

> **Feature ID:** v11-012
> **Priority:** Critical
> **Status:** Specification (Updated 2026-02-05)
> **Dependencies:** v11-007 (Objectives Storage), v11-013 (Events Hypertable)
> **Blocks:** V1.2 Pattern Detection

---

## Revision History

| Date | Change | Rationale |
|------|--------|-----------|
| 2026-02-05 | **Updated for Events Hypertable** | Crossings INSERT into gold.events, not standalone view |
| 2026-02-04 | Initial specification | Threshold crossing view approach |

---

## User Story

**As a** pattern detection system (V1.2),
**I want** threshold crossing events inserted into the events hypertable with environmental context,
**So that** I can correlate metric violations with surrounding conditions and state changes.

---

## Goal

Detect threshold crossing events when observation metrics cross declarative objective thresholds, and **INSERT them into the `gold.events` hypertable** with environmental context captured at the moment of crossing.

**Key Changes from v1:**
- Crossings are **inserted** into `gold.events` hypertable (not a standalone view)
- Environmental **context snapshot** is captured at crossing time
- Detection runs via **TimescaleDB scheduled job** (every 15 minutes)
- Explicit columns for crossing data enable efficient queries

**Key Insight**: Objectives define what matters. Threshold crossings are the moments when those things change from "ok" to "not ok" (or vice versa). Combined with context, V1.2 can answer "what was happening when the violation occurred?"

---

## Functional Requirements

### FR-E01-001: Threshold Crossing Detection

The system SHALL generate a threshold crossing event when:
1. A metric's value crosses an objective's threshold
2. The crossing is detected by comparing consecutive readings
3. Both rising (into violation) and falling (out of violation) crossings are detected

**Crossing Definition**:
```
Rising crossing:  prev_value < threshold AND current_value >= threshold (for condition "<")
Falling crossing: prev_value >= threshold AND current_value < threshold (for condition "<")
```

### FR-E01-002: Condition Operator Support

The system SHALL support all condition operators from SPEC-A05:

| Operator | Rising Crossing | Falling Crossing |
|----------|-----------------|------------------|
| `<` | prev < threshold AND curr >= threshold | prev >= threshold AND curr < threshold |
| `<=` | prev <= threshold AND curr > threshold | prev > threshold AND curr <= threshold |
| `>` | prev > threshold AND curr <= threshold | prev <= threshold AND curr > threshold |
| `>=` | prev >= threshold AND curr < threshold | prev < threshold AND curr >= threshold |
| `between` | Entry into range / Exit from range | Opposite |

### FR-E01-003: Between Condition Handling

For `between` conditions with `threshold: [min, max]`:

| State | Definition |
|-------|------------|
| In range | min <= value <= max |
| Below range | value < min |
| Above range | value > max |

Crossings detected:
- **entering_range**: Was outside, now inside
- **exiting_range_low**: Was in range, now below min
- **exiting_range_high**: Was in range, now above max

Direction encoding:
```json
{
  "direction": "entering_range",      // Was outside, now in [min,max]
  "direction": "exiting_range_low",   // Was in range, now < min
  "direction": "exiting_range_high"   // Was in range, now > max
}
```

### FR-E01-004: Event Schema (Events Hypertable)

Threshold crossing events are inserted into `gold.events` with these fields:

| Field | Type | Description |
|-------|------|-------------|
| `event_id` | UUID | Generated via `gen_random_uuid()` |
| `event_time` | TIMESTAMPTZ | Time when crossing was detected |
| `stream_id` | TEXT | Source observation stream |
| `entity_id` | TEXT | Entity identifier (ndp_id) |
| `event_type` | TEXT | Always `'threshold_crossing'` |
| `metric` | TEXT | Metric name (e.g., 'co2', 'pm25') |
| `threshold_value` | DOUBLE PRECISION | Threshold that was crossed |
| `crossing_direction` | TEXT | 'rising', 'falling', etc. |
| `metric_value` | DOUBLE PRECISION | Value at crossing |
| `previous_metric_value` | DOUBLE PRECISION | Previous value |
| `objective_id` | TEXT | Reference to objective |
| `context` | JSONB | Environmental snapshot (see FR-E01-011) |
| `details` | JSONB | Additional details |

**Example INSERT**:
```sql
INSERT INTO gold.events (
    event_time, stream_id, entity_id, event_type,
    metric, threshold_value, crossing_direction,
    metric_value, previous_metric_value, objective_id,
    context, details
) VALUES (
    '2026-02-05 10:00:00+00',
    'air-quality',
    'sensor_living_room',
    'threshold_crossing',
    'co2', 800, 'rising',
    812, 795, 'healthy_co2',
    '{"indoor_pm25": 8.2, "window_state": "off", ...}'::JSONB,
    '{"condition": "<", "unit": "ppm"}'::JSONB
);
```

### FR-E01-011: Context Capture

Each threshold crossing SHALL include environmental context at event time:

```json
{
  "indoor_co2": 812,
  "indoor_pm25": 8.2,
  "indoor_temp": 72.1,
  "indoor_humidity": 45,
  "outdoor_temp": 65.2,
  "outdoor_pm25": 12.1,
  "outdoor_aqi": 42,
  "window_state": "off",
  "time_since_last_window_change_ms": 7200000
}
```

Context is sourced from `gold.indoor_air_quality_aligned` at the crossing's hourly bucket.

### FR-E01-005: Objective Source

Threshold crossings SHALL be generated from:
1. Objectives stored in domain configuration (etcd: `/domains/{id}/config`)
2. Only objectives with `target.stream` matching an `observation` type stream

Constraints are NOT used for threshold crossing generation (they are for action filtering).

### FR-E01-006: Data Source

Threshold crossings SHALL be detected from:
1. Per-stream continuous aggregates (`gold.{stream}_hourly`)
2. Using the metric specified in `target.metric`
3. Comparing consecutive hourly buckets

### FR-E01-007: Multi-Entity Support

When an observation stream has multiple entities (ndp_id values):
- Threshold crossings SHALL be detected PER entity
- Each entity has its own crossing state
- Events include the specific `entity_id`

### FR-E01-008: Metric Aggregation Selection

For threshold crossing detection, use the **mean** aggregate:
- `{metric}_mean` column from continuous aggregate
- This aligns with typical objective definitions ("average CO2 should be < 800")

### FR-E01-009: Idempotent Detection

The crossing detection job SHALL be:
- Idempotent (only inserts NEW crossings since last run)
- Uses `last_successful_finish` from job stats to determine lookback
- Prevents duplicate events via time-based filtering

```sql
-- Detection job tracks its last run
SELECT last_successful_finish INTO last_run
FROM timescaledb_information.job_stats
WHERE job_id = detect_crossings.job_id;

-- Only process data since last run
WHERE bucket > last_run
```

### FR-E01-010: Time-Window Objectives

For objectives with `time_window`:
- Threshold crossings SHALL only be generated during the specified window
- Outside the window, crossings are NOT generated (objective does not apply)

---

## Non-Functional Requirements

### NFR-E01-001: Query Performance

Query for threshold crossings in 30-day range SHALL complete in < 50ms on Pi.

Events are stored in `gold.events` hypertable with appropriate indexes.

### NFR-E01-002: Detection Job Performance

| Metric | Target |
|--------|--------|
| Detection job runtime | < 5 seconds |
| Job schedule | Every 15 minutes |
| Event latency | < 15 minutes from source data |

### NFR-E01-003: Storage Efficiency

Events are stored in `gold.events` hypertable:
- ~500 bytes per event (with context)
- 7-day chunk interval
- 1-year retention policy
- Expected: < 100 crossings per day

### NFR-E01-004: Index Support

Indexes on `gold.events` support crossing queries:
```sql
-- Filter by event type + time (most common pattern)
CREATE INDEX idx_events_type_time ON gold.events (event_type, event_time DESC);

-- Filter by objective
CREATE INDEX idx_events_objective ON gold.events (objective_id, event_time DESC)
    WHERE event_type = 'threshold_crossing';

-- Context queries (for correlation)
CREATE INDEX idx_events_context ON gold.events USING GIN (context);
```

---

## Acceptance Criteria

### AC-E01-001: Rising Crossing Detection

```gherkin
Scenario: Detect rising threshold crossing
  Given objective "healthy_co2" with condition "<" and threshold 800
  And hourly observation at 10:00 with co2_mean = 795
  And hourly observation at 11:00 with co2_mean = 812
  When threshold crossings are computed
  Then a crossing event SHALL be generated
  And event_time = 11:00
  And direction = "rising"
  And value = 812
  And previous_value = 795
  And objective_id = "healthy_co2"
```

### AC-E01-002: Falling Crossing Detection

```gherkin
Scenario: Detect falling threshold crossing
  Given objective "healthy_co2" with condition "<" and threshold 800
  And hourly observation at 10:00 with co2_mean = 850
  And hourly observation at 11:00 with co2_mean = 780
  When threshold crossings are computed
  Then a crossing event SHALL be generated
  And direction = "falling"
  And value = 780
  And previous_value = 850
```

### AC-E01-003: No Crossing When Both Sides Same

```gherkin
Scenario: No crossing when both readings on same side
  Given objective "healthy_co2" with condition "<" and threshold 800
  And hourly observation at 10:00 with co2_mean = 750
  And hourly observation at 11:00 with co2_mean = 780
  When threshold crossings are computed
  Then NO crossing event SHALL be generated
```

### AC-E01-004: Greater Than Condition

```gherkin
Scenario: Detect crossing for greater-than condition
  Given objective "min_temp" with condition ">" and threshold 18
  And hourly observation at 10:00 with temp_mean = 20
  And hourly observation at 11:00 with temp_mean = 17
  When threshold crossings are computed
  Then a crossing event SHALL be generated
  And direction = "rising"  # Rising into violation
  And objective_id = "min_temp"
```

### AC-E01-005: Between Condition - Entering Range

```gherkin
Scenario: Detect entering range crossing
  Given objective "comfort_temp" with condition "between" and threshold [20, 24]
  And hourly observation at 10:00 with temp_mean = 18
  And hourly observation at 11:00 with temp_mean = 21
  When threshold crossings are computed
  Then a crossing event SHALL be generated
  And direction = "entering_range"
```

### AC-E01-006: Between Condition - Exiting Range

```gherkin
Scenario: Detect exiting range crossing (high)
  Given objective "comfort_temp" with condition "between" and threshold [20, 24]
  And hourly observation at 10:00 with temp_mean = 23
  And hourly observation at 11:00 with temp_mean = 26
  When threshold crossings are computed
  Then a crossing event SHALL be generated
  And direction = "exiting_range_high"
```

### AC-E01-007: Multi-Entity Crossings

```gherkin
Scenario: Detect crossings per entity
  Given objective "healthy_co2" for stream "air-quality"
  And entity "sensor_living_room" with co2 crossing from 750 to 850
  And entity "sensor_bedroom" with co2 staying at 600
  When threshold crossings are computed
  Then ONE crossing event SHALL be generated
  And entity_id = "sensor_living_room"
```

### AC-E01-008: Time Window Respect

```gherkin
Scenario: No crossing outside time window
  Given objective "night_quiet" with time_window 22:00-07:00
  And the current hour is 14:00
  And a threshold crossing would occur
  When threshold crossings are computed
  Then NO crossing event SHALL be generated
```

### AC-E01-009: NULL Handling

```gherkin
Scenario: No crossing when previous value is NULL
  Given hourly observation at 10:00 is missing (NULL)
  And hourly observation at 11:00 has valid co2_mean
  When threshold crossings are computed
  Then NO crossing event SHALL be generated
  # Cannot determine direction without previous value
```

---

## SQL Generation

### Overview

The `ndp-gold-ddl` tool generates the threshold crossing view based on:
1. Domain configuration (objectives)
2. Stream continuous aggregates (source data)

### Generated SQL Structure

```sql
-- Generated by ndp-gold-ddl for domain: indoor-air-quality
CREATE OR REPLACE VIEW gold.indoor_air_quality_threshold_crossings AS

WITH objective_thresholds AS (
    -- Inline objective definitions from config
    SELECT
        'healthy_co2'::TEXT AS objective_id,
        'air-quality'::TEXT AS stream_id,
        'co2'::TEXT AS metric,
        '<'::TEXT AS condition,
        800::NUMERIC AS threshold,
        'ppm'::TEXT AS unit,
        NULL::TIME AS time_window_start,
        NULL::TIME AS time_window_end
    UNION ALL
    SELECT
        'healthy_pm25'::TEXT AS objective_id,
        'air-quality'::TEXT AS stream_id,
        'pm25'::TEXT AS metric,
        '<'::TEXT AS condition,
        12::NUMERIC AS threshold,
        'ug/m3'::TEXT AS unit,
        NULL::TIME AS time_window_start,
        NULL::TIME AS time_window_end
    -- ... more objectives
),

-- Get observation data with previous values
observations_with_lag AS (
    SELECT
        aq.bucket,
        aq.ndp_id AS entity_id,
        'air-quality'::TEXT AS stream_id,
        aq.co2_mean AS co2_value,
        LAG(aq.co2_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS co2_prev,
        aq.pm25_mean AS pm25_value,
        LAG(aq.pm25_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS pm25_prev,
        aq.temperature_c_mean AS temperature_c_value,
        LAG(aq.temperature_c_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS temperature_c_prev
    FROM gold.air_quality_hourly aq
),

-- Unpivot to metric rows for joining with objectives
observation_metrics AS (
    SELECT
        bucket,
        entity_id,
        stream_id,
        metric,
        value,
        prev_value
    FROM observations_with_lag
    CROSS JOIN LATERAL (
        VALUES
            ('co2', co2_value, co2_prev),
            ('pm25', pm25_value, pm25_prev),
            ('temperature_c', temperature_c_value, temperature_c_prev)
    ) AS metrics(metric, value, prev_value)
    WHERE value IS NOT NULL AND prev_value IS NOT NULL
),

-- Join with objectives and detect crossings
crossings AS (
    SELECT
        om.bucket AS event_time,
        om.stream_id,
        om.entity_id,
        'threshold_crossing'::TEXT AS event_type,
        ot.objective_id,
        ot.metric,
        ot.threshold,
        ot.condition,
        ot.unit,
        om.value,
        om.prev_value,
        CASE
            -- Less than condition
            WHEN ot.condition = '<' THEN
                CASE
                    WHEN om.prev_value < ot.threshold AND om.value >= ot.threshold THEN 'rising'
                    WHEN om.prev_value >= ot.threshold AND om.value < ot.threshold THEN 'falling'
                END
            -- Less than or equal condition
            WHEN ot.condition = '<=' THEN
                CASE
                    WHEN om.prev_value <= ot.threshold AND om.value > ot.threshold THEN 'rising'
                    WHEN om.prev_value > ot.threshold AND om.value <= ot.threshold THEN 'falling'
                END
            -- Greater than condition
            WHEN ot.condition = '>' THEN
                CASE
                    WHEN om.prev_value > ot.threshold AND om.value <= ot.threshold THEN 'rising'
                    WHEN om.prev_value <= ot.threshold AND om.value > ot.threshold THEN 'falling'
                END
            -- Greater than or equal condition
            WHEN ot.condition = '>=' THEN
                CASE
                    WHEN om.prev_value >= ot.threshold AND om.value < ot.threshold THEN 'rising'
                    WHEN om.prev_value < ot.threshold AND om.value >= ot.threshold THEN 'falling'
                END
        END AS direction
    FROM observation_metrics om
    JOIN objective_thresholds ot
        ON om.stream_id = ot.stream_id
        AND om.metric = ot.metric
    WHERE
        -- Apply time window filter if specified
        (ot.time_window_start IS NULL OR
         ot.time_window_end IS NULL OR
         om.bucket::TIME BETWEEN ot.time_window_start AND ot.time_window_end)
)

SELECT
    gen_random_uuid() AS event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    jsonb_build_object(
        'metric', metric,
        'threshold', threshold,
        'direction', direction,
        'value', value,
        'previous_value', prev_value,
        'objective_id', objective_id,
        'condition', condition,
        'unit', unit
    ) AS details
FROM crossings
WHERE direction IS NOT NULL;  -- Only actual crossings
```

### Between Condition SQL Extension

For `between` conditions, additional CASE logic:

```sql
-- Between condition (threshold is array [min, max])
WHEN ot.condition = 'between' THEN
    CASE
        -- Was outside, now inside
        WHEN (om.prev_value < ot.threshold_min OR om.prev_value > ot.threshold_max)
             AND (om.value >= ot.threshold_min AND om.value <= ot.threshold_max)
            THEN 'entering_range'
        -- Was inside, now below
        WHEN (om.prev_value >= ot.threshold_min AND om.prev_value <= ot.threshold_max)
             AND om.value < ot.threshold_min
            THEN 'exiting_range_low'
        -- Was inside, now above
        WHEN (om.prev_value >= ot.threshold_min AND om.prev_value <= ot.threshold_max)
             AND om.value > ot.threshold_max
            THEN 'exiting_range_high'
    END
```

---

## Configuration

### Domain Config Extension

No changes to domain config schema - uses existing objectives from SPEC-A05.

### Stream Config

Threshold crossings require continuous aggregate for source stream:

```yaml
gold_etl:
  enabled: true
  aggregates:
    granularities: ["1 hour"]
    fields:
      co2: { metrics: [mean] }  # Required for threshold crossing
      pm25: { metrics: [mean] }
```

---

## Monitoring for Deferred Deduplication Decision

### Crossing Frequency Query

```sql
-- Monitor crossing frequency per objective per day
SELECT
    DATE_TRUNC('day', event_time) AS day,
    details->>'objective_id' AS objective_id,
    COUNT(*) AS crossing_count
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
GROUP BY 1, 2
ORDER BY 1 DESC, 3 DESC;
```

### Oscillation Detection Query

```sql
-- Detect rapid oscillations (crossings within 1 hour of each other)
WITH crossings AS (
    SELECT
        event_time,
        details->>'objective_id' AS objective_id,
        details->>'direction' AS direction,
        LAG(event_time) OVER (
            PARTITION BY details->>'objective_id'
            ORDER BY event_time
        ) AS prev_crossing_time
    FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
)
SELECT
    objective_id,
    DATE_TRUNC('day', event_time) AS day,
    COUNT(*) FILTER (WHERE event_time - prev_crossing_time < INTERVAL '1 hour') AS oscillations
FROM crossings
WHERE prev_crossing_time IS NOT NULL
GROUP BY 1, 2
HAVING COUNT(*) FILTER (WHERE event_time - prev_crossing_time < INTERVAL '1 hour') > 5
ORDER BY 3 DESC;
```

---

## London TDD Interfaces

### Trait: ThresholdCrossingGenerator

```rust
pub trait ThresholdCrossingGenerator {
    /// Generate SQL for threshold crossing view
    fn generate_crossing_view(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;

    /// Validate that objectives reference valid metrics
    fn validate_objectives(&self, domain: &DomainConfig, streams: &[StreamConfig]) -> Vec<ValidationError>;
}
```

### Struct: ThresholdCrossing

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ThresholdCrossing {
    pub event_time: DateTime<Utc>,
    pub stream_id: String,
    pub entity_id: String,
    pub objective_id: String,
    pub metric: String,
    pub threshold: f64,
    pub direction: CrossingDirection,
    pub value: f64,
    pub previous_value: f64,
    pub condition: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum CrossingDirection {
    Rising,
    Falling,
    EnteringRange,
    ExitingRangeLow,
    ExitingRangeHigh,
}
```

---

## Integration Test Requirements

### Test: Crossing Detection Logic

```rust
#[test]
fn test_rising_crossing_less_than() {
    let objective = create_objective("healthy_co2", "<", 800);
    let observations = vec![
        Observation::new("10:00", 795.0),
        Observation::new("11:00", 812.0),
    ];

    let crossings = detect_crossings(&objective, &observations);

    assert_eq!(crossings.len(), 1);
    assert_eq!(crossings[0].direction, CrossingDirection::Rising);
    assert_eq!(crossings[0].value, 812.0);
    assert_eq!(crossings[0].previous_value, 795.0);
}

#[test]
fn test_no_crossing_same_side() {
    let objective = create_objective("healthy_co2", "<", 800);
    let observations = vec![
        Observation::new("10:00", 750.0),
        Observation::new("11:00", 780.0),  // Still below threshold
    ];

    let crossings = detect_crossings(&objective, &observations);

    assert!(crossings.is_empty());
}
```

### Test: SQL Generation

```rust
#[test]
fn test_generates_crossing_view_sql() {
    let domain = create_test_domain_with_objectives();
    let generator = ThresholdCrossingGenerator::new();

    let sql = generator.generate_crossing_view(&domain).unwrap();

    assert!(sql.contains("CREATE OR REPLACE VIEW gold."));
    assert!(sql.contains("threshold_crossings"));
    assert!(sql.contains("LAG("));
    assert!(sql.contains("jsonb_build_object"));
}
```

---

## References

- [SCOPE.md](../../SCOPE.md) - v11-012 description
- [SPEC-A05](../phase-a/specification/SPEC-A05-objectives-schema.md) - Objectives schema
- [DECISIONS.md](../../architecture/DECISIONS.md) - Deferred deduplication decision
- [PHASE-E-OVERVIEW.md](./PHASE-E-OVERVIEW.md) - Phase E context

---

*SPEC-E01 created: 2026-02-04 by specification-agent*
