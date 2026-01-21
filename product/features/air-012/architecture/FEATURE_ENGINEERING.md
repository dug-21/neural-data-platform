# Feature Engineering Analysis: Window Open/Close Prediction

**Feature**: air-012 (Home Assistant Integration)
**Document**: Feature Engineering Analysis
**Version**: 1.0
**Date**: 2026-01-19
**Author**: NDP Feature Engineer
**Status**: Draft

---

## Executive Summary

This document analyzes feature engineering requirements for predicting optimal window open/close times using Home Assistant sensor data integrated with existing NDP air quality and weather streams. The analysis covers feature categories, target variable design, aggregation strategies, and concrete SQL/pseudocode implementations.

### Key Findings

| Aspect | Recommendation |
|--------|----------------|
| **Primary Target** | Binary classification: "Should window be open now?" |
| **Event Data Strategy** | Forward-fill window state + state duration features |
| **Join Strategy** | Point-in-time joins with 5-minute tolerance |
| **Key Feature Categories** | Differentials, temporal, duration, lagged outcomes |
| **TimescaleDB Support** | Requires `stream_type: events` per ADR-006-006 |

---

## 1. Data Sources Overview

### 1.1 Existing Streams (Observations)

| Stream | Type | Cadence | Key Fields |
|--------|------|---------|------------|
| `air-quality` | observations | ~1 min | pm25, co2, temperature_c, humidity_pct, tvoc_index |
| `outdoor-weather` | observations | ~10 min | temperature_c, humidity_pct, wind_speed_kmh, pressure_pa |
| `outdoor-air-quality` | observations | ~10 min | pm25, pm10, aqi_epa, o3_ugm3 |

### 1.2 New Stream: Window Events (Home Assistant)

```yaml
# config/base/streams/home-assistant-window-events/config.yaml
stream_id: home-assistant-window-events
stream_type: events  # Per ADR-006-006
description: "Window open/close state changes from Home Assistant contact sensors"

fields:
  - name: entity_id
    type: string
    nullable: false
    description: "Home Assistant entity ID (e.g., binary_sensor.window_living_room)"
  - name: state
    type: string
    nullable: false
    description: "Window state: 'on' (open) or 'off' (closed)"
  - name: previous_state
    type: string
    nullable: true
    description: "Previous state before change"
  - name: friendly_name
    type: string
    nullable: true
    description: "Human-readable window name"
  - name: device_class
    type: string
    nullable: true
    description: "Home Assistant device class (window, door, opening)"

sources:
  - type: http_poll
    enabled: true
    ndp_id: "hass-window-sensors"
    context:
      source_type:
        provider: homeassistant
        purpose: window_state_events
      location:
        type: indoor
        path: beachhouse
    poll_interval_secs: 60
    # ... additional config

silver_etl:
  enabled: true
  target_table: silver.window_events

  # Event-specific timestamp handling
  timestamp:
    source_field: raw_payload.last_changed
    target_field: event_time
    transform: iso8601_to_timestamp

  identity_fields:
    - source: raw_payload.entity_id
      target: entity_id
    - source: raw_payload.state
      target: new_state

  deduplication:
    enabled: true
    key_columns: [event_time, entity_id]
    window: 5s  # Dedupe within 5-second window
    strategy: skip
```

### 1.3 Silver Table Schema: Window Events

```sql
-- Silver layer: Event table per ADR-006-006
CREATE TABLE silver.window_events (
    event_time          TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id              TEXT NOT NULL,
    entity_id           TEXT NOT NULL,
    previous_state      TEXT,
    new_state           TEXT NOT NULL,
    friendly_name       TEXT,
    room                TEXT,  -- Extracted from entity_id or friendly_name
    orientation         TEXT,  -- N/S/E/W if known
    dq_flags            TEXT[],
    PRIMARY KEY (event_time, entity_id)
);

SELECT create_hypertable('silver.window_events', 'event_time');

-- Index for state duration queries
CREATE INDEX idx_window_events_entity_state
ON silver.window_events (entity_id, new_state, event_time DESC);
```

---

## 2. Feature Categories

### 2.1 Category Matrix

| Category | Purpose | Example Features | Computation |
|----------|---------|------------------|-------------|
| **Differentials** | Indoor/outdoor comparison | temp_diff, pm25_ratio | Real-time calculation |
| **Time-Based** | Cyclical patterns | hour_of_day, is_weekend | Timestamp extraction |
| **Window State** | Current context | window_open, any_window_open | Forward-filled |
| **Duration** | Time in state | minutes_since_last_change | Event-based |
| **Historical** | Past behavior | windows_opened_same_hour_7d | Aggregation |
| **Lagged Outcomes** | Causal learning | pm25_change_after_open_30m | Temporal join |
| **Weather Context** | External conditions | wind_direction_favorable | Domain logic |

### 2.2 Detailed Feature Definitions

#### 2.2.1 Indoor/Outdoor Differential Features

```sql
-- View: Real-time differential features
CREATE VIEW gold.differential_features AS
SELECT
    i.observation_time,
    i.ndp_id AS indoor_ndp_id,

    -- Temperature differentials
    i.temperature_c AS temp_indoor_c,
    w.temperature_c AS temp_outdoor_c,
    (i.temperature_c - w.temperature_c) AS temp_diff_c,
    ABS(i.temperature_c - w.temperature_c) AS temp_diff_abs_c,

    -- Opening window is favorable if outdoor temp closer to comfort
    CASE
        WHEN i.temperature_c > 24 AND w.temperature_c < i.temperature_c
        THEN (i.temperature_c - w.temperature_c)  -- Positive = favorable for cooling
        WHEN i.temperature_c < 20 AND w.temperature_c > i.temperature_c
        THEN (w.temperature_c - i.temperature_c)  -- Positive = favorable for warming
        ELSE 0
    END AS temp_favorability_score,

    -- Humidity differentials
    i.humidity_pct AS humidity_indoor_pct,
    w.humidity_pct AS humidity_outdoor_pct,
    (i.humidity_pct - w.humidity_pct) AS humidity_diff_pct,

    -- PM2.5 differentials (CRITICAL for air quality)
    i.pm25 AS pm25_indoor,
    o.pm25 AS pm25_outdoor,
    (i.pm25 - o.pm25) AS pm25_diff,
    CASE
        WHEN o.pm25 < i.pm25 THEN (i.pm25 - o.pm25)  -- Positive = favorable
        ELSE -(o.pm25 - i.pm25)  -- Negative = unfavorable
    END AS pm25_favorability_score,

    -- Ratio features (more stable than differences)
    NULLIF(i.pm25, 0) / NULLIF(o.pm25, 0.1) AS pm25_indoor_outdoor_ratio,

    -- CO2 differential (indoor only, but indicates need for ventilation)
    i.co2 AS co2_indoor,
    CASE WHEN i.co2 > 1000 THEN 1 ELSE 0 END AS co2_ventilation_needed,

    -- AQI comparison
    o.aqi_epa AS aqi_outdoor,
    CASE WHEN o.aqi_epa <= 50 THEN 1 ELSE 0 END AS outdoor_aqi_good

FROM silver.air_quality_observations i
-- Point-in-time join: Get most recent outdoor reading before indoor reading
LEFT JOIN LATERAL (
    SELECT temperature_c, humidity_pct
    FROM silver.weather_observations w
    WHERE w.observation_time <= i.observation_time
      AND w.observation_time >= i.observation_time - INTERVAL '15 minutes'
    ORDER BY w.observation_time DESC
    LIMIT 1
) w ON true
LEFT JOIN LATERAL (
    SELECT pm25, aqi_epa
    FROM silver.outdoor_air_quality o
    WHERE o.observation_time <= i.observation_time
      AND o.observation_time >= i.observation_time - INTERVAL '15 minutes'
    ORDER BY o.observation_time DESC
    LIMIT 1
) o ON true;
```

#### 2.2.2 Time-Based Features

```sql
-- Function: Extract cyclical time features
CREATE OR REPLACE FUNCTION gold.time_features(ts TIMESTAMPTZ)
RETURNS TABLE (
    hour_of_day SMALLINT,
    hour_sin DOUBLE PRECISION,
    hour_cos DOUBLE PRECISION,
    day_of_week SMALLINT,
    dow_sin DOUBLE PRECISION,
    dow_cos DOUBLE PRECISION,
    is_weekend BOOLEAN,
    is_night BOOLEAN,
    is_morning BOOLEAN,
    is_afternoon BOOLEAN,
    is_evening BOOLEAN,
    month_of_year SMALLINT,
    month_sin DOUBLE PRECISION,
    month_cos DOUBLE PRECISION,
    is_summer BOOLEAN
) AS $$
DECLARE
    hour_val SMALLINT := EXTRACT(HOUR FROM ts);
    dow_val SMALLINT := EXTRACT(DOW FROM ts);
    month_val SMALLINT := EXTRACT(MONTH FROM ts);
BEGIN
    RETURN QUERY SELECT
        hour_val,
        SIN(2 * PI() * hour_val / 24),
        COS(2 * PI() * hour_val / 24),
        dow_val,
        SIN(2 * PI() * dow_val / 7),
        COS(2 * PI() * dow_val / 7),
        dow_val IN (0, 6),  -- Sunday=0, Saturday=6
        hour_val BETWEEN 22 AND 23 OR hour_val BETWEEN 0 AND 5,
        hour_val BETWEEN 6 AND 11,
        hour_val BETWEEN 12 AND 17,
        hour_val BETWEEN 18 AND 21,
        month_val,
        SIN(2 * PI() * month_val / 12),
        COS(2 * PI() * month_val / 12),
        month_val IN (6, 7, 8);  -- Northern hemisphere summer
END;
$$ LANGUAGE plpgsql IMMUTABLE;
```

#### 2.2.3 Window State Features (Forward-Fill Pattern)

```sql
-- Materialized view: Forward-filled window state at observation times
-- This is the critical join between sparse events and dense observations

CREATE MATERIALIZED VIEW gold.window_state_at_observations AS
WITH observation_times AS (
    SELECT DISTINCT observation_time
    FROM silver.air_quality_observations
    WHERE observation_time >= NOW() - INTERVAL '90 days'
),
window_entities AS (
    SELECT DISTINCT entity_id, friendly_name, room
    FROM silver.window_events
),
-- Forward-fill: Get most recent state for each window at each observation time
window_states AS (
    SELECT
        ot.observation_time,
        we.entity_id,
        we.friendly_name,
        we.room,
        (
            SELECT new_state
            FROM silver.window_events e
            WHERE e.entity_id = we.entity_id
              AND e.event_time <= ot.observation_time
            ORDER BY e.event_time DESC
            LIMIT 1
        ) AS window_state,
        (
            SELECT event_time
            FROM silver.window_events e
            WHERE e.entity_id = we.entity_id
              AND e.event_time <= ot.observation_time
            ORDER BY e.event_time DESC
            LIMIT 1
        ) AS last_state_change
    FROM observation_times ot
    CROSS JOIN window_entities we
)
SELECT
    observation_time,
    entity_id,
    friendly_name,
    room,
    window_state,
    last_state_change,

    -- Binary: Is this window open?
    (window_state = 'on')::INT AS window_open,

    -- Duration since last state change (minutes)
    EXTRACT(EPOCH FROM (observation_time - last_state_change)) / 60.0 AS minutes_since_state_change

FROM window_states
WHERE window_state IS NOT NULL;

-- Refresh policy
SELECT add_continuous_aggregate_policy('gold.window_state_at_observations',
    start_offset => INTERVAL '7 days',
    end_offset => INTERVAL '5 minutes',
    schedule_interval => INTERVAL '5 minutes');
```

```sql
-- Aggregate view: Any window open / count of open windows
CREATE VIEW gold.window_aggregate_features AS
SELECT
    observation_time,

    -- Any window open?
    MAX(window_open) AS any_window_open,

    -- Count of open windows
    SUM(window_open) AS open_window_count,

    -- Count by room/orientation (if available)
    SUM(CASE WHEN room = 'living_room' THEN window_open ELSE 0 END) AS living_room_windows_open,
    SUM(CASE WHEN room = 'bedroom' THEN window_open ELSE 0 END) AS bedroom_windows_open,

    -- Average minutes since state change across open windows
    AVG(CASE WHEN window_open = 1 THEN minutes_since_state_change END) AS avg_minutes_open,

    -- Maximum minutes any window has been open
    MAX(CASE WHEN window_open = 1 THEN minutes_since_state_change END) AS max_minutes_open

FROM gold.window_state_at_observations
GROUP BY observation_time;
```

#### 2.2.4 Duration Features

```sql
-- View: Duration-based features for each window
CREATE VIEW gold.window_duration_features AS
WITH state_durations AS (
    SELECT
        entity_id,
        event_time,
        new_state,
        LEAD(event_time) OVER (PARTITION BY entity_id ORDER BY event_time) AS next_event_time,
        LEAD(event_time) OVER (PARTITION BY entity_id ORDER BY event_time) - event_time AS state_duration
    FROM silver.window_events
)
SELECT
    entity_id,
    event_time AS state_start,
    new_state AS state,
    COALESCE(next_event_time, NOW()) AS state_end,
    COALESCE(state_duration, NOW() - event_time) AS duration,
    EXTRACT(EPOCH FROM COALESCE(state_duration, NOW() - event_time)) / 60.0 AS duration_minutes,

    -- Categorize duration
    CASE
        WHEN EXTRACT(EPOCH FROM COALESCE(state_duration, NOW() - event_time)) / 60 < 5 THEN 'brief'
        WHEN EXTRACT(EPOCH FROM COALESCE(state_duration, NOW() - event_time)) / 60 < 30 THEN 'short'
        WHEN EXTRACT(EPOCH FROM COALESCE(state_duration, NOW() - event_time)) / 60 < 120 THEN 'medium'
        ELSE 'long'
    END AS duration_category

FROM state_durations;

-- Rolling statistics: Recent window usage patterns
CREATE VIEW gold.window_usage_stats AS
SELECT
    entity_id,

    -- Last 24 hours
    COUNT(*) FILTER (WHERE event_time >= NOW() - INTERVAL '24 hours') AS events_24h,
    SUM(CASE WHEN new_state = 'on' THEN 1 ELSE 0 END)
        FILTER (WHERE event_time >= NOW() - INTERVAL '24 hours') AS opens_24h,

    -- Last 7 days
    COUNT(*) FILTER (WHERE event_time >= NOW() - INTERVAL '7 days') AS events_7d,
    SUM(CASE WHEN new_state = 'on' THEN 1 ELSE 0 END)
        FILTER (WHERE event_time >= NOW() - INTERVAL '7 days') AS opens_7d,

    -- Average open duration (last 7 days)
    AVG(duration_minutes) FILTER (
        WHERE new_state = 'on'
        AND event_time >= NOW() - INTERVAL '7 days'
    ) AS avg_open_duration_7d,

    -- Typical hours when opened (mode)
    MODE() WITHIN GROUP (ORDER BY EXTRACT(HOUR FROM event_time))
        FILTER (WHERE new_state = 'on' AND event_time >= NOW() - INTERVAL '30 days')
        AS typical_open_hour

FROM gold.window_duration_features
GROUP BY entity_id;
```

#### 2.2.5 Historical Pattern Features

```sql
-- Historical pattern: Same hour/day patterns over past weeks
CREATE VIEW gold.historical_window_patterns AS
WITH hourly_patterns AS (
    SELECT
        entity_id,
        EXTRACT(HOUR FROM event_time) AS hour_of_day,
        EXTRACT(DOW FROM event_time) AS day_of_week,
        COUNT(*) FILTER (WHERE new_state = 'on') AS opens_count,
        COUNT(*) AS total_events
    FROM silver.window_events
    WHERE event_time >= NOW() - INTERVAL '30 days'
    GROUP BY entity_id, EXTRACT(HOUR FROM event_time), EXTRACT(DOW FROM event_time)
)
SELECT
    entity_id,
    hour_of_day::SMALLINT,
    day_of_week::SMALLINT,
    opens_count,
    total_events,
    opens_count::FLOAT / NULLIF(total_events, 0) AS open_probability,

    -- Is this a common time to have window open?
    CASE
        WHEN opens_count::FLOAT / NULLIF(total_events, 0) > 0.5 THEN 'high'
        WHEN opens_count::FLOAT / NULLIF(total_events, 0) > 0.2 THEN 'medium'
        ELSE 'low'
    END AS open_likelihood_category

FROM hourly_patterns;
```

#### 2.2.6 Lagged Outcome Features (Causal Learning)

```sql
-- Feature: Air quality change N minutes after window opened
-- This helps the model learn the EFFECT of opening windows

CREATE VIEW gold.window_effect_features AS
WITH window_open_events AS (
    SELECT
        entity_id,
        event_time AS window_opened_at
    FROM silver.window_events
    WHERE new_state = 'on'
),
-- Get air quality at window open time
air_quality_at_open AS (
    SELECT
        wo.entity_id,
        wo.window_opened_at,
        (
            SELECT pm25
            FROM silver.air_quality_observations aq
            WHERE aq.observation_time <= wo.window_opened_at
              AND aq.observation_time >= wo.window_opened_at - INTERVAL '5 minutes'
            ORDER BY aq.observation_time DESC
            LIMIT 1
        ) AS pm25_at_open,
        (
            SELECT co2
            FROM silver.air_quality_observations aq
            WHERE aq.observation_time <= wo.window_opened_at
              AND aq.observation_time >= wo.window_opened_at - INTERVAL '5 minutes'
            ORDER BY aq.observation_time DESC
            LIMIT 1
        ) AS co2_at_open
    FROM window_open_events wo
),
-- Get air quality 30 minutes after window opened
air_quality_after AS (
    SELECT
        ao.entity_id,
        ao.window_opened_at,
        ao.pm25_at_open,
        ao.co2_at_open,
        (
            SELECT pm25
            FROM silver.air_quality_observations aq
            WHERE aq.observation_time >= ao.window_opened_at + INTERVAL '25 minutes'
              AND aq.observation_time <= ao.window_opened_at + INTERVAL '35 minutes'
            ORDER BY aq.observation_time ASC
            LIMIT 1
        ) AS pm25_after_30m,
        (
            SELECT co2
            FROM silver.air_quality_observations aq
            WHERE aq.observation_time >= ao.window_opened_at + INTERVAL '25 minutes'
              AND aq.observation_time <= ao.window_opened_at + INTERVAL '35 minutes'
            ORDER BY aq.observation_time ASC
            LIMIT 1
        ) AS co2_after_30m
    FROM air_quality_at_open ao
)
SELECT
    entity_id,
    window_opened_at,
    pm25_at_open,
    pm25_after_30m,
    (pm25_after_30m - pm25_at_open) AS pm25_change_30m,

    -- Was opening the window beneficial?
    CASE
        WHEN pm25_after_30m < pm25_at_open THEN 1
        ELSE 0
    END AS pm25_improved_30m,

    co2_at_open,
    co2_after_30m,
    (co2_after_30m - co2_at_open) AS co2_change_30m,

    CASE
        WHEN co2_after_30m < co2_at_open THEN 1
        ELSE 0
    END AS co2_improved_30m

FROM air_quality_after
WHERE pm25_at_open IS NOT NULL AND pm25_after_30m IS NOT NULL;
```

#### 2.2.7 Weather Context Features

```sql
-- Weather favorability for window opening
CREATE VIEW gold.weather_window_features AS
SELECT
    observation_time,

    -- Raw weather
    temperature_c,
    humidity_pct,
    wind_speed_kmh,
    wind_direction_deg,
    precipitation_mm,

    -- Comfort zone check (typically 18-26C, <70% humidity)
    CASE
        WHEN temperature_c BETWEEN 18 AND 26 AND humidity_pct < 70 THEN 1
        ELSE 0
    END AS outdoor_comfortable,

    -- Wind considerations
    CASE
        WHEN wind_speed_kmh < 15 THEN 'calm'
        WHEN wind_speed_kmh < 30 THEN 'moderate'
        ELSE 'strong'
    END AS wind_category,

    -- Wind direction relative to known window orientations
    -- (Requires window orientation metadata)
    wind_direction_deg,

    -- Rain check
    CASE WHEN precipitation_mm > 0 THEN 1 ELSE 0 END AS is_raining,

    -- Combined favorability score
    CASE
        WHEN precipitation_mm > 0 THEN -2  -- Rain = don't open
        WHEN temperature_c < 10 OR temperature_c > 32 THEN -1  -- Extreme temps
        WHEN temperature_c BETWEEN 18 AND 26
             AND humidity_pct < 70
             AND wind_speed_kmh < 25 THEN 2  -- Ideal
        WHEN temperature_c BETWEEN 15 AND 28
             AND humidity_pct < 80 THEN 1  -- Acceptable
        ELSE 0  -- Neutral
    END AS weather_favorability_score

FROM silver.weather_observations;
```

---

## 3. Target Variable Design

### 3.1 Framing Options Analysis

| Framing | Target | Pros | Cons | Recommended For |
|---------|--------|------|------|-----------------|
| **Binary Classification** | Should window be open? (0/1) | Simple, interpretable | Ignores "which window" | Initial deployment |
| **Multi-label Classification** | Which windows should be open? | Handles multiple windows | Sparse labels, harder to train | Multi-room optimization |
| **Regression** | Optimal duration (minutes) | Continuous output | Harder to validate | Duration optimization |
| **Reinforcement Learning** | Action: open/close/keep | Learns from feedback | Needs simulation environment | Long-term optimization |

### 3.2 Recommended Primary Target: Binary Classification

```sql
-- Target variable: Should ANY window be open at this observation time?
-- Based on rule-based "ground truth" derived from conditions

CREATE VIEW gold.window_recommendation_target AS
SELECT
    d.observation_time,

    -- Features (inputs)
    d.pm25_indoor,
    d.pm25_outdoor,
    d.pm25_diff,
    d.temp_indoor_c,
    d.temp_outdoor_c,
    d.temp_diff_c,
    d.co2_indoor,
    w.outdoor_comfortable,
    w.is_raining,
    w.weather_favorability_score,
    wa.any_window_open,

    -- Target variable (supervised label)
    -- Rule-based "should open" based on domain logic
    CASE
        -- Definite NO: Rain, extreme outdoor conditions
        WHEN w.is_raining = 1 THEN 0
        WHEN w.temperature_c < 5 OR w.temperature_c > 35 THEN 0

        -- Definite YES: High CO2 AND favorable outdoor conditions
        WHEN d.co2_indoor > 1200
             AND w.weather_favorability_score >= 1
             AND d.pm25_outdoor < 25 THEN 1

        -- Definite YES: Much better outdoor air AND comfortable weather
        WHEN d.pm25_outdoor < d.pm25_indoor * 0.5
             AND w.outdoor_comfortable = 1 THEN 1

        -- Conditional YES: Indoor too hot, outdoor cooler
        WHEN d.temp_indoor_c > 26
             AND d.temp_outdoor_c < d.temp_indoor_c - 3
             AND w.is_raining = 0 THEN 1

        -- Default: Use historical pattern (was window actually open?)
        ELSE wa.any_window_open

    END AS should_window_be_open,

    -- Actual state (for comparison/learning)
    wa.any_window_open AS actual_window_open

FROM gold.differential_features d
LEFT JOIN gold.weather_window_features w
    ON w.observation_time = d.observation_time
LEFT JOIN gold.window_aggregate_features wa
    ON wa.observation_time = d.observation_time;
```

### 3.3 Secondary Target: Multi-Window Optimization

```sql
-- For multi-room houses: Which specific windows should be open?
CREATE VIEW gold.window_specific_recommendation AS
WITH window_room_features AS (
    SELECT
        ws.observation_time,
        ws.entity_id,
        ws.room,
        ws.window_open,

        -- Room-specific indoor conditions (if available)
        -- For now, assume single indoor sensor
        d.pm25_indoor,
        d.co2_indoor,
        d.temp_indoor_c,

        -- Outdoor conditions
        d.pm25_outdoor,
        w.temperature_c AS temp_outdoor_c,
        w.wind_direction_deg,

        -- Historical pattern for this specific window
        hp.open_probability AS historical_open_prob

    FROM gold.window_state_at_observations ws
    LEFT JOIN gold.differential_features d
        ON d.observation_time = ws.observation_time
    LEFT JOIN gold.weather_window_features w
        ON w.observation_time = ws.observation_time
    LEFT JOIN gold.historical_window_patterns hp
        ON hp.entity_id = ws.entity_id
        AND hp.hour_of_day = EXTRACT(HOUR FROM ws.observation_time)
        AND hp.day_of_week = EXTRACT(DOW FROM ws.observation_time)
)
SELECT
    observation_time,
    entity_id,
    room,

    -- Features
    pm25_indoor,
    co2_indoor,
    temp_indoor_c,
    pm25_outdoor,
    temp_outdoor_c,
    wind_direction_deg,
    historical_open_prob,

    -- Target: Should THIS window be open?
    -- (More sophisticated logic could consider room-specific needs)
    window_open AS actual_state

FROM window_room_features;
```

---

## 4. Aggregation Strategies

### 4.1 Event-Observation Join Pattern

The core challenge is joining **sparse event data** (window state changes) with **dense observation data** (minute-by-minute sensor readings).

```
Event Data (Sparse):         |--OPEN--|--------CLOSE--------|--OPEN--
                             t1       t2                    t3

Observation Data (Dense):    . . . . . . . . . . . . . . . . . . . .
                             every minute

Result (Forward-Fill):       0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 0 0 1 1 1
```

### 4.2 Implementation: Point-in-Time Join Function

```sql
-- Function: Get window state at any point in time
CREATE OR REPLACE FUNCTION gold.get_window_state_at_time(
    p_entity_id TEXT,
    p_timestamp TIMESTAMPTZ
) RETURNS TABLE (
    window_state TEXT,
    state_since TIMESTAMPTZ,
    duration_minutes DOUBLE PRECISION
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.new_state,
        e.event_time,
        EXTRACT(EPOCH FROM (p_timestamp - e.event_time)) / 60.0
    FROM silver.window_events e
    WHERE e.entity_id = p_entity_id
      AND e.event_time <= p_timestamp
    ORDER BY e.event_time DESC
    LIMIT 1;
END;
$$ LANGUAGE plpgsql STABLE;

-- Usage example
SELECT
    o.observation_time,
    o.pm25,
    (gold.get_window_state_at_time('binary_sensor.window_living_room', o.observation_time)).*
FROM silver.air_quality_observations o
WHERE o.observation_time >= NOW() - INTERVAL '1 day';
```

### 4.3 Batch Processing: Efficient Join for Training Data

```sql
-- Efficient batch processing using window functions
CREATE MATERIALIZED VIEW gold.training_data_raw AS
WITH
-- Step 1: Create timestamp spine from observations
observation_spine AS (
    SELECT DISTINCT observation_time
    FROM silver.air_quality_observations
    WHERE observation_time >= NOW() - INTERVAL '90 days'
),
-- Step 2: Expand window events with forward-fill
window_events_expanded AS (
    SELECT
        entity_id,
        event_time,
        new_state,
        LEAD(event_time, 1, NOW()) OVER (
            PARTITION BY entity_id
            ORDER BY event_time
        ) AS next_event_time
    FROM silver.window_events
),
-- Step 3: Join observations to window event ranges
observations_with_state AS (
    SELECT
        os.observation_time,
        we.entity_id,
        we.new_state AS window_state,
        we.event_time AS state_since,
        EXTRACT(EPOCH FROM (os.observation_time - we.event_time)) / 60.0 AS state_duration_min
    FROM observation_spine os
    LEFT JOIN window_events_expanded we
        ON os.observation_time >= we.event_time
        AND os.observation_time < we.next_event_time
)
SELECT * FROM observations_with_state;

-- Index for fast lookups
CREATE INDEX idx_training_data_time
ON gold.training_data_raw (observation_time);
```

### 4.4 Time Alignment for Cross-Stream Features

```sql
-- Function: Align multiple streams to common time grid
CREATE OR REPLACE FUNCTION gold.align_streams_to_grid(
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ,
    p_interval INTERVAL DEFAULT '5 minutes'
) RETURNS TABLE (
    grid_time TIMESTAMPTZ,
    indoor_pm25 DOUBLE PRECISION,
    indoor_co2 SMALLINT,
    indoor_temp DOUBLE PRECISION,
    outdoor_pm25 DOUBLE PRECISION,
    outdoor_temp DOUBLE PRECISION,
    outdoor_aqi SMALLINT,
    any_window_open INT
) AS $$
BEGIN
    RETURN QUERY
    WITH time_grid AS (
        SELECT generate_series(
            date_trunc('hour', p_start_time),
            date_trunc('hour', p_end_time),
            p_interval
        ) AS grid_time
    ),
    -- Indoor air quality (nearest observation within tolerance)
    indoor AS (
        SELECT DISTINCT ON (tg.grid_time)
            tg.grid_time,
            aq.pm25,
            aq.co2,
            aq.temperature_c
        FROM time_grid tg
        LEFT JOIN silver.air_quality_observations aq
            ON aq.observation_time BETWEEN tg.grid_time - p_interval AND tg.grid_time + p_interval
        ORDER BY tg.grid_time, ABS(EXTRACT(EPOCH FROM (aq.observation_time - tg.grid_time)))
    ),
    -- Outdoor weather (same pattern)
    outdoor_weather AS (
        SELECT DISTINCT ON (tg.grid_time)
            tg.grid_time,
            w.temperature_c
        FROM time_grid tg
        LEFT JOIN silver.weather_observations w
            ON w.observation_time BETWEEN tg.grid_time - INTERVAL '15 minutes' AND tg.grid_time + INTERVAL '15 minutes'
        ORDER BY tg.grid_time, ABS(EXTRACT(EPOCH FROM (w.observation_time - tg.grid_time)))
    ),
    -- Outdoor AQI
    outdoor_aqi AS (
        SELECT DISTINCT ON (tg.grid_time)
            tg.grid_time,
            oaq.pm25,
            oaq.aqi_epa
        FROM time_grid tg
        LEFT JOIN silver.outdoor_air_quality oaq
            ON oaq.observation_time BETWEEN tg.grid_time - INTERVAL '15 minutes' AND tg.grid_time + INTERVAL '15 minutes'
        ORDER BY tg.grid_time, ABS(EXTRACT(EPOCH FROM (oaq.observation_time - tg.grid_time)))
    ),
    -- Window state (forward-fill)
    window_state AS (
        SELECT
            tg.grid_time,
            COALESCE(MAX(CASE WHEN we.new_state = 'on' THEN 1 ELSE 0 END), 0) AS any_open
        FROM time_grid tg
        LEFT JOIN silver.window_events we
            ON we.event_time <= tg.grid_time
        GROUP BY tg.grid_time
    )
    SELECT
        i.grid_time,
        i.pm25,
        i.co2,
        i.temperature_c,
        oaq.pm25,
        ow.temperature_c,
        oaq.aqi_epa,
        ws.any_open
    FROM indoor i
    LEFT JOIN outdoor_weather ow ON ow.grid_time = i.grid_time
    LEFT JOIN outdoor_aqi oaq ON oaq.grid_time = i.grid_time
    LEFT JOIN window_state ws ON ws.grid_time = i.grid_time
    ORDER BY i.grid_time;
END;
$$ LANGUAGE plpgsql STABLE;
```

---

## 5. Complete Feature Store Design

### 5.1 Feature Table Schema

```sql
-- Gold layer: Complete feature table for ML training/inference
CREATE TABLE gold.window_prediction_features (
    -- Timestamp and identity
    observation_time TIMESTAMPTZ NOT NULL,
    feature_version TEXT NOT NULL DEFAULT 'v1',

    -- Indoor conditions
    pm25_indoor DOUBLE PRECISION,
    co2_indoor SMALLINT,
    temp_indoor_c DOUBLE PRECISION,
    humidity_indoor_pct DOUBLE PRECISION,
    tvoc_indoor SMALLINT,

    -- Outdoor conditions
    pm25_outdoor DOUBLE PRECISION,
    temp_outdoor_c DOUBLE PRECISION,
    humidity_outdoor_pct DOUBLE PRECISION,
    wind_speed_kmh DOUBLE PRECISION,
    wind_direction_deg DOUBLE PRECISION,
    aqi_outdoor SMALLINT,
    precipitation_mm DOUBLE PRECISION,

    -- Differential features
    temp_diff_c DOUBLE PRECISION,
    pm25_diff DOUBLE PRECISION,
    pm25_ratio DOUBLE PRECISION,
    humidity_diff_pct DOUBLE PRECISION,
    temp_favorability_score DOUBLE PRECISION,
    pm25_favorability_score DOUBLE PRECISION,

    -- Time features
    hour_of_day SMALLINT,
    hour_sin DOUBLE PRECISION,
    hour_cos DOUBLE PRECISION,
    day_of_week SMALLINT,
    dow_sin DOUBLE PRECISION,
    dow_cos DOUBLE PRECISION,
    is_weekend BOOLEAN,
    is_night BOOLEAN,
    month_of_year SMALLINT,
    month_sin DOUBLE PRECISION,
    month_cos DOUBLE PRECISION,

    -- Weather context
    outdoor_comfortable BOOLEAN,
    weather_favorability_score SMALLINT,
    is_raining BOOLEAN,
    wind_category TEXT,

    -- Window state features (current)
    any_window_open INT,
    open_window_count INT,
    minutes_since_state_change DOUBLE PRECISION,

    -- Historical window patterns
    historical_open_probability DOUBLE PRECISION,
    opens_24h INT,
    opens_7d INT,
    avg_open_duration_7d DOUBLE PRECISION,

    -- Lagged air quality (for learning effects)
    pm25_indoor_lag_1h DOUBLE PRECISION,
    pm25_indoor_lag_4h DOUBLE PRECISION,
    co2_indoor_lag_1h SMALLINT,

    -- Rolling window features
    pm25_indoor_mean_1h DOUBLE PRECISION,
    pm25_indoor_std_1h DOUBLE PRECISION,
    pm25_indoor_trend_1h DOUBLE PRECISION,  -- Slope
    co2_mean_1h DOUBLE PRECISION,
    temp_indoor_mean_1h DOUBLE PRECISION,

    -- Target variable
    should_window_be_open INT,  -- Rule-based recommendation
    actual_window_open INT,      -- Ground truth (what user did)

    PRIMARY KEY (observation_time, feature_version)
);

SELECT create_hypertable('gold.window_prediction_features', 'observation_time');

-- Compression for old data
ALTER TABLE gold.window_prediction_features
SET (timescaledb.compress);

SELECT add_compression_policy('gold.window_prediction_features', INTERVAL '30 days');
```

### 5.2 Feature Population Pipeline

```sql
-- Procedure: Populate features for a time range
CREATE OR REPLACE PROCEDURE gold.populate_window_features(
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ
) AS $$
BEGIN
    INSERT INTO gold.window_prediction_features
    SELECT
        aq.observation_time,
        'v1' AS feature_version,

        -- Indoor conditions
        aq.pm25,
        aq.co2,
        aq.temperature_c,
        aq.humidity_pct,
        aq.tvoc_index,

        -- Outdoor conditions (point-in-time join)
        oaq.pm25,
        w.temperature_c,
        w.humidity_pct,
        w.wind_speed_kmh,
        w.wind_direction_deg,
        oaq.aqi_epa,
        w.precipitation_mm,

        -- Differential features
        aq.temperature_c - w.temperature_c,
        aq.pm25 - oaq.pm25,
        aq.pm25 / NULLIF(oaq.pm25, 0),
        aq.humidity_pct - w.humidity_pct,
        CASE WHEN aq.temperature_c > 24 AND w.temperature_c < aq.temperature_c
             THEN aq.temperature_c - w.temperature_c ELSE 0 END,
        CASE WHEN oaq.pm25 < aq.pm25 THEN aq.pm25 - oaq.pm25 ELSE 0 END,

        -- Time features
        EXTRACT(HOUR FROM aq.observation_time)::SMALLINT,
        SIN(2 * PI() * EXTRACT(HOUR FROM aq.observation_time) / 24),
        COS(2 * PI() * EXTRACT(HOUR FROM aq.observation_time) / 24),
        EXTRACT(DOW FROM aq.observation_time)::SMALLINT,
        SIN(2 * PI() * EXTRACT(DOW FROM aq.observation_time) / 7),
        COS(2 * PI() * EXTRACT(DOW FROM aq.observation_time) / 7),
        EXTRACT(DOW FROM aq.observation_time) IN (0, 6),
        EXTRACT(HOUR FROM aq.observation_time) NOT BETWEEN 6 AND 22,
        EXTRACT(MONTH FROM aq.observation_time)::SMALLINT,
        SIN(2 * PI() * EXTRACT(MONTH FROM aq.observation_time) / 12),
        COS(2 * PI() * EXTRACT(MONTH FROM aq.observation_time) / 12),

        -- Weather context
        w.temperature_c BETWEEN 18 AND 26 AND w.humidity_pct < 70,
        CASE
            WHEN COALESCE(w.precipitation_mm, 0) > 0 THEN -2
            WHEN w.temperature_c BETWEEN 18 AND 26 AND w.humidity_pct < 70 THEN 2
            ELSE 0
        END::SMALLINT,
        COALESCE(w.precipitation_mm, 0) > 0,
        CASE
            WHEN w.wind_speed_kmh < 15 THEN 'calm'
            WHEN w.wind_speed_kmh < 30 THEN 'moderate'
            ELSE 'strong'
        END,

        -- Window state features
        wa.any_window_open,
        wa.open_window_count,
        wa.avg_minutes_open,

        -- Historical patterns
        hp.open_probability,
        us.opens_24h,
        us.opens_7d,
        us.avg_open_duration_7d,

        -- Lagged features
        LAG(aq.pm25, 60) OVER (ORDER BY aq.observation_time),
        LAG(aq.pm25, 240) OVER (ORDER BY aq.observation_time),
        LAG(aq.co2, 60) OVER (ORDER BY aq.observation_time),

        -- Rolling window features
        AVG(aq.pm25) OVER (ORDER BY aq.observation_time ROWS BETWEEN 59 PRECEDING AND CURRENT ROW),
        STDDEV(aq.pm25) OVER (ORDER BY aq.observation_time ROWS BETWEEN 59 PRECEDING AND CURRENT ROW),
        REGR_SLOPE(aq.pm25, EXTRACT(EPOCH FROM aq.observation_time))
            OVER (ORDER BY aq.observation_time ROWS BETWEEN 59 PRECEDING AND CURRENT ROW),
        AVG(aq.co2) OVER (ORDER BY aq.observation_time ROWS BETWEEN 59 PRECEDING AND CURRENT ROW),
        AVG(aq.temperature_c) OVER (ORDER BY aq.observation_time ROWS BETWEEN 59 PRECEDING AND CURRENT ROW),

        -- Target variables
        CASE
            WHEN COALESCE(w.precipitation_mm, 0) > 0 THEN 0
            WHEN aq.co2 > 1200 AND oaq.pm25 < 25 THEN 1
            WHEN oaq.pm25 < aq.pm25 * 0.5 THEN 1
            ELSE wa.any_window_open
        END,
        wa.any_window_open

    FROM silver.air_quality_observations aq
    -- Point-in-time outdoor weather join
    LEFT JOIN LATERAL (
        SELECT temperature_c, humidity_pct, wind_speed_kmh, wind_direction_deg, precipitation_mm
        FROM silver.weather_observations
        WHERE observation_time <= aq.observation_time
          AND observation_time >= aq.observation_time - INTERVAL '15 minutes'
        ORDER BY observation_time DESC LIMIT 1
    ) w ON true
    -- Point-in-time outdoor AQI join
    LEFT JOIN LATERAL (
        SELECT pm25, aqi_epa
        FROM silver.outdoor_air_quality
        WHERE observation_time <= aq.observation_time
          AND observation_time >= aq.observation_time - INTERVAL '15 minutes'
        ORDER BY observation_time DESC LIMIT 1
    ) oaq ON true
    -- Window aggregate features
    LEFT JOIN gold.window_aggregate_features wa
        ON wa.observation_time = aq.observation_time
    -- Historical patterns (join on hour/dow)
    LEFT JOIN gold.historical_window_patterns hp
        ON hp.hour_of_day = EXTRACT(HOUR FROM aq.observation_time)
        AND hp.day_of_week = EXTRACT(DOW FROM aq.observation_time)
        AND hp.entity_id = (SELECT entity_id FROM silver.window_events LIMIT 1)
    -- Usage stats (most recent)
    LEFT JOIN gold.window_usage_stats us
        ON us.entity_id = (SELECT entity_id FROM silver.window_events LIMIT 1)
    WHERE aq.observation_time BETWEEN p_start_time AND p_end_time
    ON CONFLICT (observation_time, feature_version) DO UPDATE SET
        pm25_indoor = EXCLUDED.pm25_indoor,
        co2_indoor = EXCLUDED.co2_indoor
        -- ... update all columns
    ;
END;
$$ LANGUAGE plpgsql;
```

---

## 6. Rust Feature Extraction (Pseudocode)

### 6.1 Real-Time Feature Extraction

```rust
use chrono::{DateTime, Utc, Timelike, Datelike};
use std::collections::VecDeque;

/// Feature vector for window prediction model
#[derive(Debug, Clone)]
pub struct WindowPredictionFeatures {
    // Indoor conditions
    pub pm25_indoor: f64,
    pub co2_indoor: i16,
    pub temp_indoor_c: f64,
    pub humidity_indoor_pct: f64,

    // Outdoor conditions
    pub pm25_outdoor: f64,
    pub temp_outdoor_c: f64,
    pub aqi_outdoor: i16,

    // Differentials
    pub temp_diff_c: f64,
    pub pm25_diff: f64,
    pub pm25_ratio: f64,

    // Time features (cyclical encoding)
    pub hour_sin: f64,
    pub hour_cos: f64,
    pub dow_sin: f64,
    pub dow_cos: f64,
    pub is_weekend: bool,

    // Window state
    pub any_window_open: bool,
    pub minutes_since_state_change: f64,

    // Historical
    pub historical_open_probability: f64,

    // Rolling stats
    pub pm25_mean_1h: f64,
    pub pm25_std_1h: f64,
    pub pm25_trend_1h: f64,
}

/// Rolling window buffer for streaming features
pub struct RollingBuffer {
    values: VecDeque<(DateTime<Utc>, f64)>,
    window_duration: chrono::Duration,
}

impl RollingBuffer {
    pub fn new(window_duration: chrono::Duration) -> Self {
        Self {
            values: VecDeque::new(),
            window_duration,
        }
    }

    pub fn add(&mut self, timestamp: DateTime<Utc>, value: f64) {
        // Remove expired values
        let cutoff = timestamp - self.window_duration;
        while self.values.front().map_or(false, |(ts, _)| *ts < cutoff) {
            self.values.pop_front();
        }
        self.values.push_back((timestamp, value));
    }

    pub fn mean(&self) -> Option<f64> {
        if self.values.is_empty() { return None; }
        let sum: f64 = self.values.iter().map(|(_, v)| v).sum();
        Some(sum / self.values.len() as f64)
    }

    pub fn std(&self) -> Option<f64> {
        let mean = self.mean()?;
        let variance: f64 = self.values.iter()
            .map(|(_, v)| (v - mean).powi(2))
            .sum::<f64>() / self.values.len() as f64;
        Some(variance.sqrt())
    }

    pub fn trend(&self) -> Option<f64> {
        if self.values.len() < 2 { return None; }

        // Simple linear regression slope
        let n = self.values.len() as f64;
        let (sum_x, sum_y, sum_xy, sum_xx) = self.values.iter()
            .enumerate()
            .fold((0.0, 0.0, 0.0, 0.0), |(sx, sy, sxy, sxx), (i, (_, v))| {
                let x = i as f64;
                (sx + x, sy + v, sxy + x * v, sxx + x * x)
            });

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
        Some(slope)
    }
}

/// Feature extractor with rolling buffers
pub struct WindowFeatureExtractor {
    pm25_buffer: RollingBuffer,
    co2_buffer: RollingBuffer,
    temp_buffer: RollingBuffer,

    // Cache for window state
    last_window_state: Option<(DateTime<Utc>, bool)>,

    // Historical patterns (loaded from DB)
    historical_patterns: std::collections::HashMap<(u32, u32), f64>, // (hour, dow) -> prob
}

impl WindowFeatureExtractor {
    pub fn new() -> Self {
        Self {
            pm25_buffer: RollingBuffer::new(chrono::Duration::hours(1)),
            co2_buffer: RollingBuffer::new(chrono::Duration::hours(1)),
            temp_buffer: RollingBuffer::new(chrono::Duration::hours(1)),
            last_window_state: None,
            historical_patterns: std::collections::HashMap::new(),
        }
    }

    pub fn extract(
        &mut self,
        indoor: &IndoorReading,
        outdoor: &OutdoorReading,
        window_state: Option<(DateTime<Utc>, bool)>,
    ) -> WindowPredictionFeatures {
        let ts = indoor.timestamp;

        // Update rolling buffers
        self.pm25_buffer.add(ts, indoor.pm25);
        self.co2_buffer.add(ts, indoor.co2 as f64);
        self.temp_buffer.add(ts, indoor.temperature_c);

        // Update window state cache
        if let Some(state) = window_state {
            self.last_window_state = Some(state);
        }

        // Compute time features (cyclical encoding)
        let hour = ts.hour() as f64;
        let dow = ts.weekday().num_days_from_sunday() as f64;

        let hour_sin = (2.0 * std::f64::consts::PI * hour / 24.0).sin();
        let hour_cos = (2.0 * std::f64::consts::PI * hour / 24.0).cos();
        let dow_sin = (2.0 * std::f64::consts::PI * dow / 7.0).sin();
        let dow_cos = (2.0 * std::f64::consts::PI * dow / 7.0).cos();

        // Window state features
        let (any_window_open, minutes_since_state_change) = self.last_window_state
            .map(|(state_time, is_open)| {
                let mins = (ts - state_time).num_minutes() as f64;
                (is_open, mins)
            })
            .unwrap_or((false, 0.0));

        // Historical pattern lookup
        let historical_prob = self.historical_patterns
            .get(&(ts.hour(), ts.weekday().num_days_from_sunday()))
            .copied()
            .unwrap_or(0.5);

        WindowPredictionFeatures {
            pm25_indoor: indoor.pm25,
            co2_indoor: indoor.co2,
            temp_indoor_c: indoor.temperature_c,
            humidity_indoor_pct: indoor.humidity_pct,

            pm25_outdoor: outdoor.pm25,
            temp_outdoor_c: outdoor.temperature_c,
            aqi_outdoor: outdoor.aqi_epa,

            temp_diff_c: indoor.temperature_c - outdoor.temperature_c,
            pm25_diff: indoor.pm25 - outdoor.pm25,
            pm25_ratio: indoor.pm25 / outdoor.pm25.max(0.1),

            hour_sin,
            hour_cos,
            dow_sin,
            dow_cos,
            is_weekend: dow == 0.0 || dow == 6.0,

            any_window_open,
            minutes_since_state_change,

            historical_open_probability: historical_prob,

            pm25_mean_1h: self.pm25_buffer.mean().unwrap_or(indoor.pm25),
            pm25_std_1h: self.pm25_buffer.std().unwrap_or(0.0),
            pm25_trend_1h: self.pm25_buffer.trend().unwrap_or(0.0),
        }
    }

    /// Convert features to ruv-FANN input vector
    pub fn to_fann_input(&self, features: &WindowPredictionFeatures) -> Vec<f64> {
        vec![
            // Normalized indoor conditions
            features.pm25_indoor / 100.0,  // Assume max ~100 ug/m3
            features.co2_indoor as f64 / 2000.0,  // Assume max ~2000 ppm
            (features.temp_indoor_c - 20.0) / 15.0,  // Center around 20C
            features.humidity_indoor_pct / 100.0,

            // Normalized outdoor conditions
            features.pm25_outdoor / 100.0,
            (features.temp_outdoor_c - 20.0) / 20.0,
            features.aqi_outdoor as f64 / 300.0,

            // Differentials (already scaled)
            features.temp_diff_c / 20.0,
            features.pm25_diff / 50.0,
            (features.pm25_ratio - 1.0) / 2.0,  // Center around 1.0

            // Time features (already -1 to 1)
            features.hour_sin,
            features.hour_cos,
            features.dow_sin,
            features.dow_cos,
            if features.is_weekend { 1.0 } else { 0.0 },

            // Window state
            if features.any_window_open { 1.0 } else { 0.0 },
            (features.minutes_since_state_change / 60.0).min(1.0),  // Cap at 1 hour

            // Historical
            features.historical_open_probability,

            // Rolling stats
            features.pm25_mean_1h / 100.0,
            features.pm25_std_1h / 20.0,
            features.pm25_trend_1h.clamp(-1.0, 1.0),
        ]
    }
}
```

---

## 7. Data Quality Considerations

### 7.1 Event Data Quality Rules

```yaml
# Window event DQ rules
dq_rules:
  # State transition validation
  - rule: state_transition_check
    valid_transitions:
      - from: "off"
        to: "on"
      - from: "on"
        to: "off"
    action: flag
    message: "invalid_state_transition"

  # Rapid toggle detection (possible sensor issue)
  - rule: rapid_toggle
    entity_partition: entity_id
    min_state_duration: 30  # seconds
    action: flag
    message: "rapid_toggle_detected"

  # Missing state changes (gap detection)
  - rule: event_gap_check
    max_gap: "24 hours"
    action: warn
    message: "no_events_24h"

  # Entity consistency
  - rule: entity_format
    pattern: "^binary_sensor\\.window_[a-z_]+$"
    action: flag
    message: "invalid_entity_format"
```

### 7.2 Feature Quality Validation

```sql
-- Feature quality checks before training
CREATE VIEW gold.feature_quality_report AS
SELECT
    date_trunc('day', observation_time) AS day,
    COUNT(*) AS total_rows,

    -- Completeness
    COUNT(*) FILTER (WHERE pm25_indoor IS NOT NULL) * 100.0 / COUNT(*) AS pm25_indoor_completeness,
    COUNT(*) FILTER (WHERE pm25_outdoor IS NOT NULL) * 100.0 / COUNT(*) AS pm25_outdoor_completeness,
    COUNT(*) FILTER (WHERE any_window_open IS NOT NULL) * 100.0 / COUNT(*) AS window_state_completeness,

    -- Range violations
    COUNT(*) FILTER (WHERE pm25_indoor < 0 OR pm25_indoor > 500) AS pm25_range_violations,
    COUNT(*) FILTER (WHERE temp_diff_c > 30 OR temp_diff_c < -30) AS temp_diff_outliers,

    -- Label distribution
    AVG(should_window_be_open::FLOAT) AS positive_label_rate,

    -- Feature distribution stats
    AVG(pm25_indoor) AS avg_pm25_indoor,
    STDDEV(pm25_indoor) AS std_pm25_indoor,
    AVG(co2_indoor) AS avg_co2_indoor

FROM gold.window_prediction_features
WHERE observation_time >= NOW() - INTERVAL '30 days'
GROUP BY date_trunc('day', observation_time)
ORDER BY day DESC;
```

---

## 8. Training Data Export

### 8.1 Export to Parquet for ruv-FANN

```sql
-- Export training dataset
COPY (
    SELECT
        observation_time,

        -- Feature columns (21 features)
        pm25_indoor / 100.0 AS f01_pm25_indoor_norm,
        co2_indoor / 2000.0 AS f02_co2_norm,
        (temp_indoor_c - 20) / 15.0 AS f03_temp_indoor_norm,
        humidity_indoor_pct / 100.0 AS f04_humidity_indoor_norm,
        pm25_outdoor / 100.0 AS f05_pm25_outdoor_norm,
        (temp_outdoor_c - 20) / 20.0 AS f06_temp_outdoor_norm,
        aqi_outdoor / 300.0 AS f07_aqi_norm,
        temp_diff_c / 20.0 AS f08_temp_diff_norm,
        pm25_diff / 50.0 AS f09_pm25_diff_norm,
        (pm25_ratio - 1) / 2.0 AS f10_pm25_ratio_norm,
        hour_sin AS f11_hour_sin,
        hour_cos AS f12_hour_cos,
        dow_sin AS f13_dow_sin,
        dow_cos AS f14_dow_cos,
        is_weekend::INT AS f15_is_weekend,
        any_window_open AS f16_window_open,
        LEAST(minutes_since_state_change / 60.0, 1.0) AS f17_state_duration_norm,
        historical_open_probability AS f18_historical_prob,
        pm25_mean_1h / 100.0 AS f19_pm25_mean_1h_norm,
        pm25_std_1h / 20.0 AS f20_pm25_std_1h_norm,
        GREATEST(LEAST(pm25_trend_1h, 1), -1) AS f21_pm25_trend_norm,

        -- Target
        should_window_be_open AS target

    FROM gold.window_prediction_features
    WHERE observation_time BETWEEN '2025-06-01' AND '2025-12-31'
      AND pm25_indoor IS NOT NULL
      AND pm25_outdoor IS NOT NULL
    ORDER BY observation_time
) TO '/data/training/window_prediction_v1.parquet' (FORMAT PARQUET);
```

---

## 9. Summary and Recommendations

### 9.1 Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Event storage** | `stream_type: events` | Per ADR-006-006, handles sparse data properly |
| **Join strategy** | Point-in-time with forward-fill | Prevents data leakage, handles sparse events |
| **Primary target** | Binary "should window be open" | Simple, interpretable, good starting point |
| **Feature count** | 21 normalized features | Balance of expressiveness and model complexity |
| **Time encoding** | Cyclical (sin/cos) | Handles midnight wrap-around properly |

### 9.2 Implementation Phases

1. **Phase 1: Data Collection** (air-012 scope)
   - Implement Home Assistant window sensor polling
   - Create `silver.window_events` table
   - Validate event data quality

2. **Phase 2: Feature Engineering** (fe-001 scope)
   - Create Gold layer feature views
   - Implement rolling window functions
   - Build feature population pipeline

3. **Phase 3: Model Training** (ml-001 scope)
   - Export training data to Parquet
   - Train ruv-FANN binary classifier
   - Validate model performance

4. **Phase 4: Inference Integration**
   - Implement real-time feature extraction (Rust)
   - Deploy model for live predictions
   - Create recommendation dashboard

### 9.3 Dependencies

- ADR-006-006: Stream Type Distinction (defines `events` type)
- Silver layer ETL (dp-006): Event table support
- TimescaleDB: Continuous aggregates for rolling features
- ruv-FANN: Neural network for prediction

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-19 | NDP Feature Engineer | Initial analysis |

---

## References

1. ADR-006-006: Stream Type Distinction
2. `product/research/Silver/ml-feature-engineering.md`
3. `research/agenticdataplatform/silver/07-ml-platform-assessment.md`
4. Home Assistant State Object: https://www.home-assistant.io/docs/configuration/state_object/
5. TimescaleDB Continuous Aggregates: https://docs.timescale.com/use-timescale/latest/continuous-aggregates/
