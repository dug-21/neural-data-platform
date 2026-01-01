# Forecast Evaluation Schema Design

## Overview

This document describes the Silver layer schema design for weather forecasts, specifically structured to enable forecast accuracy evaluation against observations.

## Design Principle

The schema is driven by the **domain model and use cases**, not by the API response format.

**Use Case**: Evaluate forecast accuracy by lead_time to determine how far in advance predictions can be trusted for decision-making (e.g., window open/close).

## Key Schema Requirements

From the domain model, we need to capture:

1. **issue_time**: When the forecast was generated
2. **valid_time**: When the prediction applies
3. **lead_time**: Derived (valid_time - issue_time) - the key analysis dimension
4. **valid_duration**: How long the prediction is valid (PT1H, PT6H, etc.)
5. **metrics**: The actual forecast values

## Forecasts Table

```sql
CREATE TABLE silver.weather_forecasts (
    -- Audit/debugging
    ingestion_time      TIMESTAMPTZ NOT NULL,

    -- DOMAIN KEYS (from understanding the domain, not the API)
    issue_time          TIMESTAMPTZ NOT NULL,  -- When NWS generated this forecast
    valid_time          TIMESTAMPTZ NOT NULL,  -- When prediction applies
    valid_duration      INTERVAL,              -- How long valid (PT1H, PT6H, etc.)

    -- Derived: Essential for analysis
    lead_time_hours     INTEGER GENERATED ALWAYS AS
                        (EXTRACT(EPOCH FROM valid_time - issue_time) / 3600) STORED,

    -- Location identifiers
    ndp_id              TEXT NOT NULL,
    grid_office         TEXT,
    grid_x              INTEGER,
    grid_y              INTEGER,

    -- Core metrics (dashboard-critical, always present)
    temperature_c       DOUBLE PRECISION,
    dewpoint_c          DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    wind_speed_kmh      DOUBLE PRECISION,
    wind_direction_deg  DOUBLE PRECISION,
    wind_gust_kmh       DOUBLE PRECISION,
    precip_prob_pct     DOUBLE PRECISION,
    sky_cover_pct       DOUBLE PRECISION,
    visibility_m        DOUBLE PRECISION,

    -- Derived comfort metrics
    apparent_temp_c     DOUBLE PRECISION,
    heat_index_c        DOUBLE PRECISION,
    wind_chill_c        DOUBLE PRECISION,

    -- DQ flags (set by Transform DQ)
    dq_flags            TEXT[],  -- Array of rule names that flagged this row

    PRIMARY KEY (issue_time, valid_time, ndp_id)
);

-- Hypertable on valid_time (for joining with observations)
SELECT create_hypertable('silver.weather_forecasts', 'valid_time');

-- Index for lead_time analysis
CREATE INDEX idx_forecasts_lead_time
ON silver.weather_forecasts (lead_time_hours, valid_time);

-- Index for location queries
CREATE INDEX idx_forecasts_ndp_id
ON silver.weather_forecasts (ndp_id, valid_time DESC);
```

## Observations Table

```sql
CREATE TABLE silver.weather_observations (
    ingestion_time      TIMESTAMPTZ NOT NULL,
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    station_id          TEXT,

    -- Same metrics as forecasts (enables direct comparison)
    temperature_c       DOUBLE PRECISION,
    dewpoint_c          DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    wind_speed_kmh      DOUBLE PRECISION,
    wind_direction_deg  DOUBLE PRECISION,
    wind_gust_kmh       DOUBLE PRECISION,
    sky_cover_pct       DOUBLE PRECISION,
    visibility_m        DOUBLE PRECISION,

    -- Observation-specific
    pressure_hpa        DOUBLE PRECISION,

    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable('silver.weather_observations', 'observation_time');
```

## Forecast Accuracy View

This view joins forecasts to observations for accuracy analysis:

```sql
CREATE VIEW analytics.forecast_accuracy AS
SELECT
    f.valid_time,
    f.issue_time,
    f.lead_time_hours,
    f.ndp_id,

    -- Forecast values
    f.temperature_c AS forecast_temp,
    f.humidity_pct AS forecast_humidity,
    f.wind_speed_kmh AS forecast_wind,
    f.precip_prob_pct AS forecast_precip_prob,

    -- Observed values
    o.temperature_c AS observed_temp,
    o.humidity_pct AS observed_humidity,
    o.wind_speed_kmh AS observed_wind,

    -- Errors
    ABS(f.temperature_c - o.temperature_c) AS temp_error,
    ABS(f.humidity_pct - o.humidity_pct) AS humidity_error,
    ABS(f.wind_speed_kmh - o.wind_speed_kmh) AS wind_error,

    -- Signed errors (for bias detection)
    f.temperature_c - o.temperature_c AS temp_bias,
    f.humidity_pct - o.humidity_pct AS humidity_bias

FROM silver.weather_forecasts f
JOIN silver.weather_observations o
  ON f.valid_time = o.observation_time
 AND f.ndp_id = o.ndp_id;
```

## Key Analysis Queries

### Accuracy by Lead Time

```sql
-- How accurate is NWS at different lead times?
SELECT
    lead_time_hours,
    COUNT(*) as sample_count,
    AVG(temp_error) as avg_temp_error,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY temp_error) as median_temp_error,
    PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY temp_error) as p90_temp_error,
    AVG(temp_bias) as temp_bias  -- Positive = forecast too warm
FROM analytics.forecast_accuracy
WHERE lead_time_hours BETWEEN 1 AND 168
  AND valid_time > NOW() - INTERVAL '30 days'
GROUP BY lead_time_hours
ORDER BY lead_time_hours;
```

### Trustworthy Forecast Horizon

```sql
-- At what lead_time does error exceed acceptable threshold?
WITH accuracy_by_lead AS (
    SELECT
        lead_time_hours,
        PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY temp_error) as p90_error
    FROM analytics.forecast_accuracy
    WHERE valid_time > NOW() - INTERVAL '30 days'
    GROUP BY lead_time_hours
)
SELECT
    MAX(lead_time_hours) as max_trustworthy_hours
FROM accuracy_by_lead
WHERE p90_error <= 2.0;  -- 2°C threshold
```

### Forecast Improvement Over Time

```sql
-- Does the forecast improve as valid_time approaches?
-- Compare forecasts for same valid_time at different lead_times
SELECT
    valid_time,
    lead_time_hours,
    temperature_c,
    observed_temp,
    temp_error
FROM analytics.forecast_accuracy
WHERE valid_time = '2026-01-02T12:00:00Z'
ORDER BY lead_time_hours DESC;
```

## Extension Tables (Sparse Data)

For rarely-used metrics (fire weather, marine, etc.), use a tall table:

```sql
CREATE TABLE silver.weather_forecast_extended (
    ingestion_time      TIMESTAMPTZ NOT NULL,
    issue_time          TIMESTAMPTZ NOT NULL,
    valid_time          TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    metric_name         TEXT NOT NULL,
    value               DOUBLE PRECISION,
    unit                TEXT,

    PRIMARY KEY (valid_time, ndp_id, metric_name)
);

SELECT create_hypertable('silver.weather_forecast_extended', 'valid_time');
```

Metrics stored here:
- hainesIndex
- davisStabilityIndex
- redFlagThreatIndex
- mixingHeight
- dispersionIndex
- waveHeight, wavePeriod, waveDirection
- primarySwellHeight, primarySwellDirection
- etc.

## Weather Conditions (Qualitative Data)

For non-numeric weather descriptions:

```sql
CREATE TABLE silver.weather_conditions (
    ingestion_time      TIMESTAMPTZ NOT NULL,
    issue_time          TIMESTAMPTZ NOT NULL,
    valid_time          TIMESTAMPTZ NOT NULL,
    valid_duration      INTERVAL,
    ndp_id              TEXT NOT NULL,

    coverage            TEXT,  -- 'likely', 'patchy', 'areas', 'slight_chance'
    intensity           TEXT,  -- 'moderate', 'heavy', 'light'
    weather_type        TEXT,  -- 'rain_showers', 'thunderstorms', 'frost'
    visibility_impact   TEXT,  -- From visibility object if present

    PRIMARY KEY (valid_time, ndp_id, weather_type)
);

SELECT create_hypertable('silver.weather_conditions', 'valid_time');
```

## ETL Mapping Notes

### Issue Time Extraction

The NWS API provides `updateTime` in the response:
```json
"updateTime": "2026-01-01T13:56:49+00:00"
```

This becomes `issue_time` in Silver.

### Valid Time Parsing

NWS uses ISO 8601 intervals:
```json
"validTime": "2026-01-01T07:00:00+00:00/PT2H"
```

Parse to:
- `valid_time`: 2026-01-01T07:00:00Z
- `valid_duration`: PT2H (interval '2 hours')

### Duration Handling Options

**Option A: Keep as intervals (recommended for now)**
- Store `valid_time` and `valid_duration`
- Queries use range operations: `valid_time <= target AND valid_time + valid_duration > target`

**Option B: Expand to hourly (future optimization)**
- Create one row per hour within the duration
- Simpler queries but more storage
- Can be done in a materialized view if needed

## Schema Summary

| Table | Purpose | Hypertable Column |
|-------|---------|-------------------|
| `silver.weather_forecasts` | Core forecast metrics | valid_time |
| `silver.weather_observations` | Ground truth measurements | observation_time |
| `silver.weather_forecast_extended` | Sparse/specialized metrics | valid_time |
| `silver.weather_conditions` | Qualitative weather | valid_time |
| `analytics.forecast_accuracy` | View joining forecasts to observations | - |
