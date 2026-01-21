# Home Assistant Window Sensor Data Model

**Feature**: air-012 - Home Assistant Integration
**Author**: ndp-analytics-engineer
**Date**: 2026-01-19
**Status**: Draft

---

## Overview

This document defines the data model for Home Assistant window sensor integration, covering Bronze (raw events), Silver (clean analytics-ready), and Gold (feature store) layers. Window sensors are fundamentally different from our existing weather/air quality streams - they produce **sparse event data** (state changes) rather than **dense time-series** (periodic measurements).

## Domain Understanding

### Data Characteristics

| Aspect | Dense Time-Series (Existing) | Sparse Event Data (Window Sensors) |
|--------|------------------------------|-----------------------------------|
| Example | AirGradient readings every 60s | Window state changes 0-20x/day |
| Volume | ~1440 rows/day/sensor | ~5-10 rows/day/window |
| Pattern | Regular intervals | Irregular, event-driven |
| Primary Key | (observation_time, ndp_id) | (event_time, entity_id) |
| Analytics | Aggregations, rolling averages | Duration calculations, state at time |

### Home Assistant Response Structure

```json
{
  "entity_id": "binary_sensor.living_room_window",
  "state": "on",
  "attributes": {
    "device_class": "window",
    "friendly_name": "Living Room Window"
  },
  "last_changed": "2026-01-19T10:30:00.000000+00:00",
  "last_reported": "2026-01-19T10:30:05.123456+00:00",
  "last_updated": "2026-01-19T10:30:00.000000+00:00",
  "context": {
    "id": "01ABCD...",
    "parent_id": null,
    "user_id": null
  }
}
```

Key timestamps:
- `last_changed`: When state actually changed (use this for analytics)
- `last_updated`: When entity was last updated (includes attribute-only changes)
- `last_reported`: When HA last received data from device

---

## Bronze Layer (Raw Event Storage)

### Schema: Wide Raw JSON

Following the established Bronze pattern (arch-bronze-schema), store the complete Home Assistant response:

```
data/bronze/home-assistant-states/
  YYYY/MM/DD/
    home-assistant-states_{timestamp}.parquet
```

### Parquet Schema

| Column | Type | Description |
|--------|------|-------------|
| `timestamp` | TIMESTAMPTZ | Ingestion timestamp (when NDP received it) |
| `source_id` | TEXT | "home-assistant-{instance}" |
| `ndp_id` | TEXT | Stable NDP identifier (e.g., "hass-window-living-room") |
| `context` | JSON | Config-derived metadata snapshot |
| `raw_payload` | JSON | Complete HA state response |

### Partitioning Strategy

**Recommendation**: Daily partitions (same as existing streams)

Rationale:
- Event data is sparse, so daily partitions will be small (< 1MB)
- Consistent with other Bronze streams simplifies ETL
- Query patterns typically filter by day anyway

### Stream Configuration (Conceptual)

```yaml
stream_id: home-assistant-states
stream_type: events  # NEW: per arch-dp-006-stream-types pattern
description: Home Assistant entity state changes

retention_days: 730  # 2 years for ML training
compression_after_days: 7
partitioning_strategy: daily

sources:
  - type: http_poll
    ndp_id: hass-window-living-room
    context:
      location:
        path: home/living-room
        type: indoor
      device_class: window
      friendly_name: Living Room Window

    poll_interval_secs: 60  # Poll every minute

    endpoints:
      - endpoint_id: living_room_window
        url: "http://192.168.52.221:8123/api/states/binary_sensor.living_room_window"
        auth_type: bearer
        auth_value: "${HASS_TOKEN}"
```

---

## Silver Layer Analysis

### Option A: Event Table Only

Store one row per state change with derived duration.

**Pros**:
- Natural representation of event data
- Efficient storage (one row per actual change)
- Easy to calculate durations

**Cons**:
- Point-in-time queries require `WHERE event_time <= target ORDER BY event_time DESC LIMIT 1`
- Joining with time-series data is complex

### Option B: State Snapshot Table

Store current state, updated on each change.

**Pros**:
- Simple current state queries
- Easy joins with time-series data

**Cons**:
- Loses history (need separate audit table)
- Not suitable for TimescaleDB hypertable (no time dimension)

### Option C: Hybrid Approach (Recommended)

Events table for history + materialized view for current state.

**Pros**:
- Complete history preserved
- Efficient point-in-time queries via events
- Current state view for dashboards
- Natural fit for TimescaleDB (events are time-indexed)

**Cons**:
- Slightly more complex schema
- Materialized view requires refresh

---

## Silver Layer Design (Recommended: Option C)

### Table: silver.window_events

The core event log with derived duration calculated at query time or via trigger.

```sql
-- =============================================================================
-- Silver Layer: Window State Events
-- =============================================================================
-- Source: Bronze home-assistant-states stream
-- Grain: One row per state change event
-- Use: Window open/close history, duration analysis, ML features

CREATE TABLE IF NOT EXISTS silver.window_events (
    -- Time columns
    event_time          TIMESTAMPTZ NOT NULL,   -- last_changed from HA
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identity columns
    entity_id           TEXT NOT NULL,          -- HA entity_id (binary_sensor.xxx)
    ndp_id              TEXT NOT NULL,          -- NDP stable identifier

    -- State change
    new_state           TEXT NOT NULL,          -- 'on' (open) or 'off' (closed)
    old_state           TEXT,                   -- Previous state (NULL for first event)

    -- Context (denormalized for query efficiency)
    location_path       TEXT,                   -- e.g., 'home/living-room'
    friendly_name       TEXT,                   -- e.g., 'Living Room Window'

    -- HA metadata
    ha_context_id       TEXT,                   -- HA context.id for debugging

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (event_time, entity_id)
);

-- Create hypertable with 7-day chunks (sparse data = larger chunks OK)
SELECT create_hypertable(
    'silver.window_events',
    'event_time',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

COMMENT ON TABLE silver.window_events IS
    'Window state change events from Home Assistant.
     Source: home-assistant-states Bronze stream.
     Grain: One row per state change.
     State values: on=open, off=closed (HA binary_sensor convention).
     Use: Duration analysis, point-in-time state queries, ML features.';

-- Index for entity queries
CREATE INDEX IF NOT EXISTS idx_window_events_entity
ON silver.window_events (entity_id, event_time DESC);

-- Index for location queries
CREATE INDEX IF NOT EXISTS idx_window_events_location
ON silver.window_events (location_path, event_time DESC);
```

### Hypertable vs Regular Table Decision

**Recommendation: Hypertable with 7-day chunks**

Rationale:
- Even sparse data benefits from time-based partitioning for retention policies
- TimescaleDB compression works well on sparse data
- Continuous aggregates can calculate window duration stats
- 7-day chunks (vs 1-day for dense data) because ~70 events/week is reasonable chunk size

### View: silver.v_window_current_state

Materialized view for current state of all windows.

```sql
-- =============================================================================
-- Current Window State (Materialized View)
-- =============================================================================

CREATE MATERIALIZED VIEW silver.v_window_current_state AS
WITH latest_events AS (
    SELECT DISTINCT ON (entity_id)
        entity_id,
        ndp_id,
        event_time,
        new_state as current_state,
        location_path,
        friendly_name
    FROM silver.window_events
    ORDER BY entity_id, event_time DESC
)
SELECT
    entity_id,
    ndp_id,
    event_time as state_since,
    current_state,
    -- Duration in current state
    EXTRACT(EPOCH FROM (NOW() - event_time)) / 60 as minutes_in_state,
    location_path,
    friendly_name,
    -- Convenience boolean
    current_state = 'on' as is_open
FROM latest_events;

-- Index for fast lookups
CREATE UNIQUE INDEX idx_window_current_entity
ON silver.v_window_current_state (entity_id);

-- Refresh on schedule (e.g., every minute)
-- In production: use pg_cron or application-level refresh
-- REFRESH MATERIALIZED VIEW CONCURRENTLY silver.v_window_current_state;

COMMENT ON MATERIALIZED VIEW silver.v_window_current_state IS
    'Current state of all windows. Refresh every minute for dashboard use.
     Use for: Real-time dashboards, current state joins.
     Do NOT use for: Historical analysis (use window_events instead).';
```

### Function: Window State at Time

For point-in-time correct queries:

```sql
-- =============================================================================
-- Get window state at a specific time
-- =============================================================================

CREATE OR REPLACE FUNCTION silver.get_window_state_at(
    p_entity_id TEXT,
    p_target_time TIMESTAMPTZ
) RETURNS TABLE (
    entity_id TEXT,
    state TEXT,
    state_since TIMESTAMPTZ,
    seconds_in_state DOUBLE PRECISION
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        we.entity_id,
        we.new_state as state,
        we.event_time as state_since,
        EXTRACT(EPOCH FROM (p_target_time - we.event_time)) as seconds_in_state
    FROM silver.window_events we
    WHERE we.entity_id = p_entity_id
      AND we.event_time <= p_target_time
    ORDER BY we.event_time DESC
    LIMIT 1;
END;
$$ LANGUAGE plpgsql STABLE;

-- Example: What was the window state when this air quality reading was taken?
-- SELECT * FROM silver.get_window_state_at(
--     'binary_sensor.living_room_window',
--     '2026-01-19T14:30:00Z'
-- );
```

### Function: Calculate Duration Between Events

```sql
-- =============================================================================
-- Calculate window open/close durations
-- =============================================================================

CREATE OR REPLACE FUNCTION silver.calculate_window_durations(
    p_entity_id TEXT,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '7 days',
    p_end_time TIMESTAMPTZ DEFAULT NOW()
) RETURNS TABLE (
    entity_id TEXT,
    state TEXT,
    started_at TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    duration_minutes DOUBLE PRECISION
) AS $$
BEGIN
    RETURN QUERY
    WITH ordered_events AS (
        SELECT
            we.entity_id,
            we.new_state,
            we.event_time,
            LEAD(we.event_time) OVER (
                PARTITION BY we.entity_id
                ORDER BY we.event_time
            ) as next_event_time
        FROM silver.window_events we
        WHERE we.entity_id = p_entity_id
          AND we.event_time >= p_start_time
          AND we.event_time < p_end_time
    )
    SELECT
        oe.entity_id,
        oe.new_state as state,
        oe.event_time as started_at,
        COALESCE(oe.next_event_time, p_end_time) as ended_at,
        EXTRACT(EPOCH FROM (
            COALESCE(oe.next_event_time, p_end_time) - oe.event_time
        )) / 60 as duration_minutes
    FROM ordered_events oe
    ORDER BY oe.event_time;
END;
$$ LANGUAGE plpgsql STABLE;
```

---

## Gold Layer / Feature Store

### Design Philosophy

For ML features, we need **point-in-time correct** features that can be:
1. Joined with air quality observations
2. Computed efficiently for both training and inference
3. Pre-aggregated where beneficial

### Option Analysis: Pre-aggregated vs Compute-on-Demand

| Approach | Use Case | Storage | Query Speed | Freshness |
|----------|----------|---------|-------------|-----------|
| **Pre-aggregated** | Training datasets, dashboards | Higher | Fastest | Batch (hourly/daily) |
| **Compute-on-demand** | Real-time inference | None | Slower | Real-time |
| **Hybrid** | Best of both | Moderate | Fast | Near real-time |

**Recommendation**: Hybrid approach
- Pre-aggregate hourly/daily summaries for training
- Compute-on-demand for real-time inference using indexed events table

### Table: gold.window_hourly_features

Pre-aggregated hourly features for ML training:

```sql
-- =============================================================================
-- Gold Layer: Hourly Window Features
-- =============================================================================
-- Grain: One row per (hour, entity_id)
-- Use: ML training datasets, correlation analysis

CREATE TABLE IF NOT EXISTS gold.window_hourly_features (
    -- Time bucket
    hour                TIMESTAMPTZ NOT NULL,

    -- Identity
    entity_id           TEXT NOT NULL,
    ndp_id              TEXT NOT NULL,
    location_path       TEXT,

    -- State features
    minutes_open        DOUBLE PRECISION,     -- Minutes window was open in hour
    minutes_closed      DOUBLE PRECISION,     -- Minutes window was closed
    pct_time_open       DOUBLE PRECISION,     -- Percentage of hour open (0-100)

    -- Transition features
    open_count          INTEGER,              -- Number of times opened in hour
    close_count         INTEGER,              -- Number of times closed in hour
    total_transitions   INTEGER,              -- Total state changes

    -- State at boundaries (for joins)
    state_at_hour_start TEXT,                 -- 'on' or 'off'
    state_at_hour_end   TEXT,                 -- 'on' or 'off'

    PRIMARY KEY (hour, entity_id)
);

SELECT create_hypertable(
    'gold.window_hourly_features',
    'hour',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

COMMENT ON TABLE gold.window_hourly_features IS
    'Hourly aggregated window state features for ML training.
     Grain: One row per (hour, entity_id).
     Use: Training datasets, correlation with hourly air quality.
     Refresh: Hourly via ETL job.';

-- Index for entity-time queries
CREATE INDEX IF NOT EXISTS idx_window_hourly_entity
ON gold.window_hourly_features (entity_id, hour DESC);
```

### Materialized View: Window-AirQuality Join

The key join pattern for correlation analysis:

```sql
-- =============================================================================
-- Analytics View: Indoor Air Quality with Window State
-- =============================================================================

CREATE MATERIALIZED VIEW analytics.air_quality_with_window_state AS
WITH window_states AS (
    -- Get most recent window state before each hour
    SELECT DISTINCT ON (hour_bucket, entity_id)
        time_bucket('1 hour', we.event_time) as hour_bucket,
        we.entity_id,
        we.ndp_id as window_ndp_id,
        we.new_state as window_state,
        we.location_path
    FROM silver.window_events we
    ORDER BY hour_bucket, entity_id, we.event_time DESC
),
hourly_aq AS (
    -- Aggregate air quality to hourly
    SELECT
        time_bucket('1 hour', observation_time) as hour,
        ndp_id,
        location_path,
        AVG(pm25) as avg_pm25,
        AVG(co2) as avg_co2,
        AVG(temperature_c) as avg_temp_c,
        AVG(humidity_pct) as avg_humidity_pct
    FROM silver.air_quality_observations
    GROUP BY 1, 2, 3
)
SELECT
    aq.hour,
    aq.ndp_id as aq_sensor_ndp_id,
    aq.location_path,
    aq.avg_pm25,
    aq.avg_co2,
    aq.avg_temp_c,
    aq.avg_humidity_pct,
    ws.entity_id as window_entity_id,
    ws.window_state,
    ws.window_state = 'on' as window_is_open,
    -- Feature: hourly window features
    whf.minutes_open,
    whf.pct_time_open,
    whf.total_transitions
FROM hourly_aq aq
LEFT JOIN window_states ws
    ON aq.hour = ws.hour_bucket
    AND aq.location_path = ws.location_path  -- Join on location!
LEFT JOIN gold.window_hourly_features whf
    ON aq.hour = whf.hour
    AND ws.entity_id = whf.entity_id;

-- Refresh daily for training data
-- REFRESH MATERIALIZED VIEW analytics.air_quality_with_window_state;
```

### View: Indoor/Outdoor Differential with Window State

The key feature for window management ML:

```sql
-- =============================================================================
-- Feature: Indoor/Outdoor Differential When Window Open vs Closed
-- =============================================================================

CREATE VIEW analytics.indoor_outdoor_window_correlation AS
WITH hourly_indoor AS (
    SELECT
        time_bucket('1 hour', observation_time) as hour,
        location_path,
        AVG(pm25) as indoor_pm25,
        AVG(temperature_c) as indoor_temp_c,
        AVG(humidity_pct) as indoor_humidity_pct,
        AVG(co2) as indoor_co2
    FROM silver.air_quality_observations
    GROUP BY 1, 2
),
hourly_outdoor AS (
    SELECT
        time_bucket('1 hour', observation_time) as hour,
        AVG(pm25) as outdoor_pm25,
        AVG(temperature_c) as outdoor_temp_c,
        AVG(humidity_pct) as outdoor_humidity_pct
    FROM silver.outdoor_air_quality
    GROUP BY 1
),
hourly_weather AS (
    SELECT
        time_bucket('1 hour', observation_time) as hour,
        AVG(temperature_c) as weather_temp_c,
        AVG(humidity_pct) as weather_humidity_pct,
        AVG(wind_speed_kmh) as wind_speed_kmh
    FROM silver.weather_observations
    GROUP BY 1
)
SELECT
    i.hour,
    i.location_path,

    -- Indoor metrics
    i.indoor_pm25,
    i.indoor_temp_c,
    i.indoor_humidity_pct,
    i.indoor_co2,

    -- Outdoor metrics
    o.outdoor_pm25,
    COALESCE(o.outdoor_temp_c, w.weather_temp_c) as outdoor_temp_c,
    COALESCE(o.outdoor_humidity_pct, w.weather_humidity_pct) as outdoor_humidity_pct,
    w.wind_speed_kmh,

    -- Differentials (KEY FEATURES)
    i.indoor_pm25 - COALESCE(o.outdoor_pm25, 0) as pm25_differential,
    i.indoor_temp_c - COALESCE(o.outdoor_temp_c, w.weather_temp_c) as temp_differential,
    i.indoor_humidity_pct - COALESCE(o.outdoor_humidity_pct, w.weather_humidity_pct) as humidity_differential,

    -- Window state features (from hourly features table)
    whf.pct_time_open as window_pct_open,
    whf.total_transitions as window_transitions,
    whf.state_at_hour_end as window_state_at_end,

    -- Derived feature: condition-aware differential
    CASE
        WHEN whf.pct_time_open > 50 THEN 'mostly_open'
        WHEN whf.pct_time_open > 0 THEN 'partially_open'
        ELSE 'closed'
    END as window_category

FROM hourly_indoor i
LEFT JOIN hourly_outdoor o ON i.hour = o.hour
LEFT JOIN hourly_weather w ON i.hour = w.hour
LEFT JOIN gold.window_hourly_features whf
    ON i.hour = whf.hour
    AND i.location_path = whf.location_path;

COMMENT ON VIEW analytics.indoor_outdoor_window_correlation IS
    'Key ML feature view: Indoor/outdoor differentials correlated with window state.
     Use: Training window management models, correlation analysis.
     Key insight: How does pm25_differential change when window is open vs closed?';
```

### Example Query: Impact Analysis

```sql
-- What's the average PM2.5 differential when windows are open vs closed?
SELECT
    window_category,
    COUNT(*) as sample_hours,
    AVG(pm25_differential) as avg_pm25_diff,
    AVG(temp_differential) as avg_temp_diff,
    AVG(indoor_co2) as avg_indoor_co2
FROM analytics.indoor_outdoor_window_correlation
WHERE hour > NOW() - INTERVAL '30 days'
GROUP BY window_category
ORDER BY window_category;
```

---

## TimescaleDB Considerations

### Compression Settings

For sparse event data, use aggressive compression:

```sql
-- Enable compression on window_events (after 7 days)
ALTER TABLE silver.window_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id',
    timescaledb.compress_orderby = 'event_time DESC'
);

-- Add compression policy: compress chunks older than 7 days
SELECT add_compression_policy('silver.window_events', INTERVAL '7 days');

-- For gold hourly features
ALTER TABLE gold.window_hourly_features SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id',
    timescaledb.compress_orderby = 'hour DESC'
);

SELECT add_compression_policy('gold.window_hourly_features', INTERVAL '7 days');
```

### Continuous Aggregates for Duration Stats

```sql
-- =============================================================================
-- Continuous Aggregate: Daily Window Duration Summary
-- =============================================================================

CREATE MATERIALIZED VIEW silver.window_daily_summary
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', event_time) as day,
    entity_id,
    ndp_id,
    location_path,
    COUNT(*) FILTER (WHERE new_state = 'on') as opens_count,
    COUNT(*) FILTER (WHERE new_state = 'off') as closes_count,
    COUNT(*) as total_events,
    MIN(event_time) as first_event,
    MAX(event_time) as last_event
FROM silver.window_events
GROUP BY time_bucket('1 day', event_time), entity_id, ndp_id, location_path
WITH NO DATA;

-- Refresh policy: update daily aggregates
SELECT add_continuous_aggregate_policy('silver.window_daily_summary',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);
```

---

## ETL Pipeline Design

### Bronze to Silver ETL

```
Bronze Parquet → DuckDB Query → Silver window_events
```

Key transformations:
1. Extract `last_changed` as `event_time`
2. Map `state` to `new_state`
3. Calculate `old_state` from previous event (via LAG)
4. Extract context fields (location_path, friendly_name)

### Silver to Gold ETL

```
Silver window_events → Hourly aggregation → Gold window_hourly_features
```

```sql
-- ETL query to populate hourly features
INSERT INTO gold.window_hourly_features
SELECT
    time_bucket('1 hour', event_time) as hour,
    entity_id,
    ndp_id,
    location_path,
    -- Minutes open: requires duration calculation
    -- (complex - see calculate_window_durations function)
    NULL as minutes_open,  -- Populated separately
    NULL as minutes_closed,
    NULL as pct_time_open,
    COUNT(*) FILTER (WHERE new_state = 'on') as open_count,
    COUNT(*) FILTER (WHERE new_state = 'off') as close_count,
    COUNT(*) as total_transitions,
    -- State at boundaries requires window function
    FIRST_VALUE(new_state) OVER (
        PARTITION BY entity_id, time_bucket('1 hour', event_time)
        ORDER BY event_time
    ) as state_at_hour_start,
    LAST_VALUE(new_state) OVER (
        PARTITION BY entity_id, time_bucket('1 hour', event_time)
        ORDER BY event_time
        ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
    ) as state_at_hour_end
FROM silver.window_events
WHERE event_time >= :start_time
  AND event_time < :end_time
GROUP BY time_bucket('1 hour', event_time), entity_id, ndp_id, location_path
ON CONFLICT (hour, entity_id) DO UPDATE SET
    open_count = EXCLUDED.open_count,
    close_count = EXCLUDED.close_count,
    total_transitions = EXCLUDED.total_transitions;
```

---

## Data Quality Rules

### Bronze DQ

| Rule | Check | Action |
|------|-------|--------|
| entity_id_present | entity_id IS NOT NULL | FLAG |
| state_valid | state IN ('on', 'off', 'unknown', 'unavailable') | FLAG |
| timestamp_valid | last_changed IS NOT NULL AND last_changed <= NOW() | FLAG |
| future_timestamp | last_changed > NOW() + INTERVAL '5 minutes' | REJECT |

### Silver DQ

| Rule | Check | Action |
|------|-------|--------|
| duplicate_event | No duplicate (event_time, entity_id) | UPSERT |
| orphan_close | First event is 'off' without prior 'on' | FLAG |
| rapid_toggle | > 10 transitions in 1 minute | FLAG (sensor issue) |

---

## Schema Summary

| Layer | Table/View | Type | Purpose |
|-------|------------|------|---------|
| Bronze | home-assistant-states/ | Parquet | Raw HA state responses |
| Silver | `silver.window_events` | Hypertable | Event log with state changes |
| Silver | `silver.v_window_current_state` | Materialized View | Current state (refresh: 1min) |
| Silver | `silver.window_daily_summary` | Continuous Aggregate | Daily event counts |
| Gold | `gold.window_hourly_features` | Hypertable | ML features (hourly) |
| Analytics | `analytics.air_quality_with_window_state` | Materialized View | AQ + window join |
| Analytics | `analytics.indoor_outdoor_window_correlation` | View | ML training features |

---

## Migration Path

### Phase 1: Bronze Stream

1. Create stream configuration for `home-assistant-states`
2. Implement HTTP polling source for HA API
3. Deploy and verify Bronze data landing

### Phase 2: Silver Schema

1. Run `002_window_events.sql` migration
2. Create Silver ETL job
3. Backfill from Bronze (if needed)

### Phase 3: Gold Features

1. Create hourly features table
2. Implement ETL job
3. Create continuous aggregate

### Phase 4: Analytics Views

1. Create join views
2. Validate feature accuracy
3. Connect to ML pipeline

---

## References

- ADR-006-006: Stream Types (events vs observations)
- arch-data-lake-layers: Bronze/Silver/Gold architecture
- 02-WEATHER-DOMAIN-MODEL.md: Domain modeling approach
- 02-air-quality-analytics.md: AQ analytics requirements
