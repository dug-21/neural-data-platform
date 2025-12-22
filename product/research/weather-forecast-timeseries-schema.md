# Weather Forecast Time-Series Data Modeling Research

**Research Date**: 2025-12-21
**Context**: Neural Data Platform - Silver Layer Schema Design
**Purpose**: Design optimal data model for weather forecast time-series tracking issue time, forecast time, and verification

---

## Executive Summary

This research examines best practices for storing weather forecast time-series data, specifically addressing the dual-timestamp challenge (issue time vs. forecast time), storage format optimization (absolute vs. delta time), schema design patterns (wide vs. tall), and forecast verification workflows.

### Key Recommendations

| Aspect | Recommendation | Rationale |
|--------|---------------|-----------|
| **Time Representation** | ✅ **Tall Format with Absolute Times** | Query flexibility, forecast comparison, standard meteorological practice |
| **Schema Design** | ✅ **Tall/Long Format (Normalized)** | TimescaleDB hypertable optimization, query performance, scalability |
| **Primary Keys** | `(issue_time, forecast_valid_time, location_id)` | Supports forecast evolution tracking and verification joins |
| **Storage Strategy** | Hypertable partitioned by `forecast_valid_time` | Efficient queries by target period, automatic compression |
| **Verification** | LEFT JOIN forecasts with observations on `(valid_time, location)` | Standard meteorological verification pattern |

**Bottom Line**: Use **tall format with absolute timestamps** for both issue and valid times. This balances query flexibility, storage efficiency, and forecast verification requirements for NDP's TimescaleDB-based Silver layer.

---

## 1. Issue Time vs. Forecast Time (Reference Time vs. Valid Time)

### Meteorological Standard Terminology

Based on WeatherBench 2 and ECMWF standards:

| Term | Definition | Example |
|------|------------|---------|
| **Issue Time** (Reference Time, Init Time) | When the forecast was generated/retrieved | `2025-12-21 06:00:00 UTC` |
| **Forecast Valid Time** (Target Time) | When the forecasted conditions are predicted for | `2025-12-21 15:00:00 UTC` |
| **Lead Time** (Forecast Horizon) | Duration between issue and valid time | `9 hours` or `+9h` |

### Two Convention Approaches

**Init-Time Convention** (ECMWF format):
- Primary dimension: `issue_time` (initialization time)
- Valid time calculated: `valid_time = issue_time + lead_time`
- Example: Issue at 06:00 UTC with lead_time=+9h → Valid at 15:00 UTC

**Valid-Time Convention** (Alternative format):
- Primary dimension: `forecast_valid_time` (when forecast applies)
- Issue time calculated: `issue_time = valid_time - lead_time`
- Example: Valid at 15:00 UTC with lead_time=+9h → Issued at 06:00 UTC

**NDP Recommendation**: Store **both absolute times** explicitly to avoid runtime calculations and support bidirectional queries.

**Sources**:
- [Init vs Valid Time Conventions - WeatherBench 2](https://weatherbench2.readthedocs.io/en/latest/init-vs-valid-time.html)
- [Meteorological Time Definition - NWS](https://www.weather.gov/tg/time)

---

## 2. Absolute Time vs. Delta Time Representation

### Option A: Absolute Time (RECOMMENDED)

**Schema**:
```sql
CREATE TABLE weather_forecasts (
    issue_time TIMESTAMPTZ NOT NULL,
    forecast_valid_time TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    temperature DOUBLE PRECISION,
    precipitation DOUBLE PRECISION,
    -- ... other forecast fields
    PRIMARY KEY (issue_time, forecast_valid_time, location_id)
);
```

**Pros**:
- ✅ **Direct querying**: "Show all forecasts valid for Dec 21, 3pm" without calculation
- ✅ **Charting flexibility**: Plot forecast evolution across multiple issue times
- ✅ **Verification simplicity**: Direct JOIN with observations on `valid_time`
- ✅ **Time zone handling**: TIMESTAMPTZ handles UTC and local time conversions
- ✅ **Index efficiency**: B-tree indexes on both timestamps enable fast range queries

**Cons**:
- ⚠️ **Storage overhead**: Two 8-byte timestamps vs. one timestamp + 2-byte smallint (~14 bytes)
- ⚠️ **Redundancy**: Lead time can be calculated but isn't explicitly stored

**Query Examples**:
```sql
-- Get all forecasts targeting today 6pm (regardless of when issued)
SELECT * FROM weather_forecasts
WHERE forecast_valid_time = '2025-12-21 18:00:00+00'
ORDER BY issue_time DESC;

-- Compare forecast evolution for a specific target time
SELECT issue_time, temperature
FROM weather_forecasts
WHERE forecast_valid_time = '2025-12-21 18:00:00+00'
  AND location_id = 'nyc'
ORDER BY issue_time;

-- Get latest forecast for next 24 hours
SELECT forecast_valid_time, temperature
FROM weather_forecasts
WHERE issue_time = (SELECT MAX(issue_time) FROM weather_forecasts)
  AND forecast_valid_time BETWEEN NOW() AND NOW() + INTERVAL '24 hours';
```

### Option B: Delta Time (Hours Offset)

**Schema**:
```sql
CREATE TABLE weather_forecasts_delta (
    issue_time TIMESTAMPTZ NOT NULL,
    forecast_hour_offset SMALLINT NOT NULL,  -- +1, +2, +3...+168
    location_id TEXT NOT NULL,
    temperature DOUBLE PRECISION,
    PRIMARY KEY (issue_time, forecast_hour_offset, location_id)
);
```

**Pros**:
- ✅ **Storage efficiency**: ~2 bytes for offset vs. 8 bytes for full timestamp
- ✅ **Semantic clarity**: "+6h forecast" is explicit in data
- ✅ **Forecast horizon queries**: Easy to filter by lead time (e.g., all +24h forecasts)

**Cons**:
- ❌ **Runtime calculation required**: Must compute `issue_time + (forecast_hour_offset * INTERVAL '1 hour')`
- ❌ **Complex verification**: Joining with observations requires calculated field
- ❌ **Charting complexity**: Visualization tools need to compute valid times
- ❌ **Index limitations**: Can't index on computed `valid_time` without materialized column

**Query Examples**:
```sql
-- Get all forecasts targeting today 6pm (REQUIRES CALCULATION)
SELECT *, issue_time + (forecast_hour_offset * INTERVAL '1 hour') AS valid_time
FROM weather_forecasts_delta
WHERE issue_time + (forecast_hour_offset * INTERVAL '1 hour') = '2025-12-21 18:00:00+00';

-- Get all 24-hour-ahead forecasts (EASY)
SELECT * FROM weather_forecasts_delta
WHERE forecast_hour_offset = 24;
```

### Comparison Analysis

| Criteria | Absolute Time | Delta Time | Winner |
|----------|---------------|------------|--------|
| **Storage Size** | 16 bytes (2 timestamps) | 10 bytes (1 timestamp + smallint) | Delta |
| **Query Simplicity** | Direct timestamp filters | Requires calculation | **Absolute** |
| **Charting Flexibility** | Native timestamp support | Must compute on client | **Absolute** |
| **Verification Joins** | Direct JOIN on timestamps | Calculated field JOIN | **Absolute** |
| **Forecast Horizon Analysis** | Requires calculation | Native offset filtering | Delta |
| **Index Performance** | B-tree on both timestamps | B-tree on offset only | **Absolute** |
| **Developer Experience** | Intuitive SQL queries | Complex date arithmetic | **Absolute** |

### NDP Recommendation: **Absolute Time**

**Decision Rationale**:
1. **Query flexibility** outweighs storage savings (6 bytes per row negligible)
2. **Grafana integration** simpler with native timestamps
3. **Forecast verification** requires direct time matching with observations
4. **TimescaleDB hypertables** optimize timestamp-based partitioning

**Hybrid Approach** (Optional):
Store both for best of both worlds:
```sql
CREATE TABLE weather_forecasts (
    issue_time TIMESTAMPTZ NOT NULL,
    forecast_valid_time TIMESTAMPTZ NOT NULL,
    lead_time_hours SMALLINT GENERATED ALWAYS AS
        (EXTRACT(EPOCH FROM (forecast_valid_time - issue_time)) / 3600) STORED,
    location_id TEXT NOT NULL,
    -- ... forecast fields
);
```
- `lead_time_hours`: Automatically computed, stored for fast horizon filtering
- Adds ~2 bytes per row but enables both query patterns

---

## 3. Schema Design: Wide vs. Tall Format

### Option A: Tall/Long Format (RECOMMENDED)

**Schema**:
```sql
CREATE TABLE weather_forecasts_tall (
    issue_time TIMESTAMPTZ NOT NULL,
    forecast_valid_time TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    forecast_hour SMALLINT NOT NULL,  -- +1, +2, +3...
    temperature DOUBLE PRECISION,
    precipitation DOUBLE PRECISION,
    wind_speed DOUBLE PRECISION,
    -- ... other metrics
    PRIMARY KEY (issue_time, forecast_valid_time, location_id)
);

-- Example data:
-- issue_time              | forecast_valid_time     | location | temp | precip
-- 2025-12-21 06:00:00+00 | 2025-12-21 07:00:00+00 | nyc      | 15.2 | 0.0
-- 2025-12-21 06:00:00+00 | 2025-12-21 08:00:00+00 | nyc      | 15.5 | 0.1
-- 2025-12-21 06:00:00+00 | 2025-12-21 09:00:00+00 | nyc      | 16.1 | 0.3
```

**Pros**:
- ✅ **TimescaleDB hypertable optimization**: Partitions by `forecast_valid_time`
- ✅ **Efficient compression**: Columnar compression on older partitions (80-95% reduction)
- ✅ **Query performance**: Index on `(forecast_valid_time, location_id)` for fast lookups
- ✅ **Flexible filtering**: Time-range queries leverage partition pruning
- ✅ **Schema evolution**: Add new forecast metrics without restructuring
- ✅ **Verification joins**: Natural JOIN with observations table

**Cons**:
- ⚠️ **Row count**: N issue times × M forecast hours = N×M rows per location
- ⚠️ **Query verbosity**: Multi-hour queries need WHERE clause with range

**TimescaleDB Integration**:
```sql
-- Create hypertable partitioned by forecast valid time
SELECT create_hypertable('weather_forecasts_tall', 'forecast_valid_time',
    chunk_time_interval => INTERVAL '1 day');

-- Add compression policy (compress after 7 days)
ALTER TABLE weather_forecasts_tall SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id,issue_time'
);
SELECT add_compression_policy('weather_forecasts_tall', INTERVAL '7 days');

-- Retention policy (delete forecasts older than 90 days)
SELECT add_retention_policy('weather_forecasts_tall', INTERVAL '90 days');
```

### Option B: Wide Format (Not Recommended)

**Schema**:
```sql
CREATE TABLE weather_forecasts_wide (
    issue_time TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    temp_hour_01 DOUBLE PRECISION,
    temp_hour_02 DOUBLE PRECISION,
    temp_hour_03 DOUBLE PRECISION,
    -- ... up to temp_hour_168 (7 days)
    precip_hour_01 DOUBLE PRECISION,
    precip_hour_02 DOUBLE PRECISION,
    -- ... hundreds of columns
    PRIMARY KEY (issue_time, location_id)
);

-- Example data:
-- issue_time              | location | temp_hour_01 | temp_hour_02 | temp_hour_03
-- 2025-12-21 06:00:00+00 | nyc      | 15.2         | 15.5         | 16.1
```

**Pros**:
- ✅ **Single row per issue**: All forecast hours in one row
- ✅ **Fewer rows**: Only N issue times (not N×M)

**Cons**:
- ❌ **PostgreSQL column limit**: 1600 column maximum (168 hours × metrics exceeds limit)
- ❌ **Query complexity**: Querying specific forecast hour requires dynamic SQL or CASE statements
- ❌ **TimescaleDB limitations**: Cannot partition by forecast valid time (not a column)
- ❌ **Compression inefficiency**: Row-based storage doesn't compress well
- ❌ **Schema rigidity**: Adding forecast hours requires ALTER TABLE
- ❌ **Verification nightmare**: Cannot JOIN with observations easily
- ❌ **NULL bloat**: Sparse forecasts (not all hours) waste space

**Query Example** (Ugly):
```sql
-- Get temperature forecast for hour +6
SELECT issue_time, location_id, temp_hour_06
FROM weather_forecasts_wide
WHERE issue_time = '2025-12-21 06:00:00+00';

-- Get all forecasts for a target valid time (IMPOSSIBLE without calculation)
-- Cannot query "all forecasts valid for 3pm" without knowing which column
```

### Comparison Analysis

| Criteria | Tall Format | Wide Format | Winner |
|----------|-------------|-------------|--------|
| **TimescaleDB Hypertables** | ✅ Partitions by `valid_time` | ❌ Cannot partition by forecast hour | **Tall** |
| **Compression Efficiency** | ✅ Columnar (80-95% reduction) | ⚠️ Row-based only | **Tall** |
| **Query Performance** | ✅ Index + partition pruning | ⚠️ Full table scan | **Tall** |
| **Schema Flexibility** | ✅ Add columns easily | ❌ ALTER TABLE required | **Tall** |
| **Verification Joins** | ✅ Natural JOIN on timestamps | ❌ Complex unpivoting required | **Tall** |
| **Row Count** | ⚠️ High (N×M) | ✅ Low (N) | Wide |
| **PostgreSQL Limits** | ✅ No issues | ❌ 1600 column limit | **Tall** |
| **Grafana Queries** | ✅ Standard SQL | ⚠️ Complex pivoting | **Tall** |

### NDP Recommendation: **Tall Format**

**Decision**: Tall/long format is **universally recommended** for time-series databases, especially TimescaleDB.

**Sources**:
- [Designing Wide vs. Narrow Postgres Tables - Timescale](https://www.tigerdata.com/learn/designing-your-database-schema-wide-vs-narrow-postgres-tables)
- [Best Practices for Time-Series Data Modeling - TimescaleDB](https://www.tigerdata.com/learn/best-practices-time-series-data-modeling-single-or-multiple-partitioned-tables-aka-hypertables)
- [AWS RDS High-Performance Time-Series Design](https://aws.amazon.com/blogs/database/designing-high-performance-time-series-data-tables-on-amazon-rds-for-postgresql/)

---

## 4. Tracking Forecast Accuracy & Verification

### Challenge: Joining Forecasts with Observations

**Goal**: Compare forecasted values with actual observed values to compute accuracy metrics (MAE, RMSE, bias).

**Meteorological Verification Standard**: Point-to-point comparison of forecasts against observations at matching valid times and locations.

### Recommended Schema Design

**Forecasts Table** (Tall Format):
```sql
CREATE TABLE weather_forecasts (
    issue_time TIMESTAMPTZ NOT NULL,
    forecast_valid_time TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    temperature_forecast DOUBLE PRECISION,
    precipitation_forecast DOUBLE PRECISION,
    wind_speed_forecast DOUBLE PRECISION,
    source TEXT DEFAULT 'openweathermap',
    PRIMARY KEY (issue_time, forecast_valid_time, location_id)
);

SELECT create_hypertable('weather_forecasts', 'forecast_valid_time');
```

**Observations Table** (NDP Bronze Layer Equivalent):
```sql
CREATE TABLE weather_observations (
    observation_time TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    temperature_observed DOUBLE PRECISION,
    precipitation_observed DOUBLE PRECISION,
    wind_speed_observed DOUBLE PRECISION,
    sensor_id TEXT,
    PRIMARY KEY (observation_time, location_id)
);

SELECT create_hypertable('weather_observations', 'observation_time');
```

### Verification Query Pattern

**LEFT JOIN** forecasts with observations on `(valid_time, location)`:

```sql
-- Compute forecast errors for all forecasts issued today
WITH forecast_verification AS (
    SELECT
        f.issue_time,
        f.forecast_valid_time,
        f.location_id,
        f.temperature_forecast,
        o.temperature_observed,
        (f.forecast_valid_time - f.issue_time) AS lead_time,
        (f.temperature_forecast - o.temperature_observed) AS error,
        ABS(f.temperature_forecast - o.temperature_observed) AS abs_error
    FROM weather_forecasts f
    LEFT JOIN weather_observations o
        ON f.forecast_valid_time = o.observation_time
        AND f.location_id = o.location_id
    WHERE f.issue_time >= CURRENT_DATE
)
SELECT
    location_id,
    EXTRACT(EPOCH FROM lead_time) / 3600 AS lead_time_hours,
    COUNT(*) AS forecast_count,
    AVG(error) AS mean_error_bias,
    AVG(abs_error) AS mean_absolute_error,
    SQRT(AVG(error ^ 2)) AS root_mean_squared_error
FROM forecast_verification
WHERE temperature_observed IS NOT NULL  -- Only verified forecasts
GROUP BY location_id, lead_time_hours
ORDER BY location_id, lead_time_hours;
```

**Output**:
```
location_id | lead_time_hours | forecast_count | mean_error_bias | mae  | rmse
nyc         | 1               | 24             | 0.3             | 0.8  | 1.1
nyc         | 6               | 24             | 0.5             | 1.2  | 1.6
nyc         | 24              | 24             | -0.2            | 2.1  | 2.7
```

### Continuous Aggregate for Real-Time Metrics

**TimescaleDB Materialized View** (Auto-Refreshed):
```sql
CREATE MATERIALIZED VIEW forecast_accuracy_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', f.forecast_valid_time) AS bucket,
    f.location_id,
    EXTRACT(EPOCH FROM (f.forecast_valid_time - f.issue_time)) / 3600 AS lead_time_hours,
    COUNT(*) AS num_forecasts,
    AVG(f.temperature_forecast - o.temperature_observed) AS bias,
    AVG(ABS(f.temperature_forecast - o.temperature_observed)) AS mae
FROM weather_forecasts f
LEFT JOIN weather_observations o
    ON f.forecast_valid_time = o.observation_time
    AND f.location_id = o.location_id
WHERE o.temperature_observed IS NOT NULL
GROUP BY bucket, f.location_id, lead_time_hours;

-- Auto-refresh policy (refresh every hour)
SELECT add_continuous_aggregate_policy('forecast_accuracy_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
```

### Grafana Dashboard Query Example

```sql
-- Show forecast accuracy by lead time for past 7 days
SELECT
    bucket AS time,
    lead_time_hours,
    mae AS "Mean Absolute Error"
FROM forecast_accuracy_hourly
WHERE location_id = 'nyc'
    AND bucket >= NOW() - INTERVAL '7 days'
ORDER BY bucket, lead_time_hours;
```

**Visualization**: Heatmap or line chart showing MAE degradation with increasing lead time.

### Best Practices for Forecast Verification

1. **LEFT JOIN (not INNER)**: Include forecasts without observations to track coverage
2. **Time Bucket Alignment**: Use `time_bucket()` to align irregular observations with hourly forecasts
3. **Lead Time Stratification**: Group accuracy metrics by forecast horizon (1h, 6h, 24h, etc.)
4. **Retention Policy**: Keep raw forecasts for 90 days, aggregated metrics for 2+ years
5. **Index Strategy**:
   ```sql
   CREATE INDEX idx_forecasts_verification
       ON weather_forecasts (forecast_valid_time, location_id)
       INCLUDE (temperature_forecast);

   CREATE INDEX idx_observations_verification
       ON weather_observations (observation_time, location_id)
       INCLUDE (temperature_observed);
   ```

**Sources**:
- [NOAA Forecast Verification - MDL](https://vlab.noaa.gov/web/mdl/fv)
- [NDFD Verification Help](https://vlab.noaa.gov/web/mdl/ndfd-verification-help)
- [Forecast Verification Methods - CAWCR](https://www.cawcr.gov.au/projects/verification/verif_web_page.html)

---

## 5. Recommended NDP Schema Design

### Production Schema for NDP Silver Layer

```sql
-- Weather forecast predictions from external APIs
CREATE TABLE weather_forecasts (
    -- Temporal dimensions (explicit absolute times)
    issue_time TIMESTAMPTZ NOT NULL,
    forecast_valid_time TIMESTAMPTZ NOT NULL,

    -- Computed lead time for horizon analysis
    lead_time_hours SMALLINT GENERATED ALWAYS AS
        (EXTRACT(EPOCH FROM (forecast_valid_time - issue_time)) / 3600) STORED,

    -- Spatial dimension
    location_id TEXT NOT NULL,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,

    -- Forecast metadata
    forecast_source TEXT NOT NULL DEFAULT 'openweathermap',
    forecast_model TEXT,  -- e.g., 'GFS', 'ECMWF', 'HRRR'

    -- Meteorological fields
    temperature DOUBLE PRECISION,  -- °C
    feels_like DOUBLE PRECISION,
    humidity DOUBLE PRECISION,  -- %
    pressure DOUBLE PRECISION,  -- hPa
    wind_speed DOUBLE PRECISION,  -- m/s
    wind_direction SMALLINT,  -- degrees (0-360)
    precipitation DOUBLE PRECISION,  -- mm
    precipitation_probability DOUBLE PRECISION,  -- % (0-100)
    cloud_cover SMALLINT,  -- % (0-100)
    visibility INTEGER,  -- meters
    weather_condition TEXT,  -- 'clear', 'rain', 'snow', etc.

    -- Ingestion metadata (NDP standard fields)
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (issue_time, forecast_valid_time, location_id)
);

-- Create TimescaleDB hypertable partitioned by forecast valid time
SELECT create_hypertable('weather_forecasts', 'forecast_valid_time',
    chunk_time_interval => INTERVAL '1 day');

-- Compression policy: compress data older than 7 days
ALTER TABLE weather_forecasts SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id,issue_time,forecast_source',
    timescaledb.compress_orderby = 'forecast_valid_time DESC'
);
SELECT add_compression_policy('weather_forecasts', INTERVAL '7 days');

-- Retention policy: delete forecasts older than 90 days
SELECT add_retention_policy('weather_forecasts', INTERVAL '90 days');

-- Indexes for common query patterns
CREATE INDEX idx_forecasts_issue_time
    ON weather_forecasts (issue_time, location_id);

CREATE INDEX idx_forecasts_lead_time
    ON weather_forecasts (lead_time_hours, location_id);

-- Partial index for latest forecasts only (hot data)
CREATE INDEX idx_forecasts_latest
    ON weather_forecasts (location_id, forecast_valid_time)
    WHERE issue_time > NOW() - INTERVAL '24 hours';
```

### Observations Table (Bronze Layer Integration)

```sql
-- Actual observed weather (from sensors or APIs)
CREATE TABLE weather_observations (
    observation_time TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,

    -- Observed meteorological fields
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    pressure DOUBLE PRECISION,
    wind_speed DOUBLE PRECISION,
    wind_direction SMALLINT,
    precipitation DOUBLE PRECISION,

    -- Data source metadata
    source TEXT NOT NULL,  -- 'sensor', 'openweathermap_actual', etc.
    sensor_id TEXT,

    PRIMARY KEY (observation_time, location_id)
);

SELECT create_hypertable('weather_observations', 'observation_time');
```

### Continuous Aggregate: Forecast Accuracy Metrics

```sql
CREATE MATERIALIZED VIEW forecast_accuracy_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', f.forecast_valid_time) AS day,
    f.location_id,
    f.lead_time_hours,
    COUNT(*) AS num_forecasts,
    COUNT(o.temperature) AS num_verified,

    -- Temperature metrics
    AVG(f.temperature - o.temperature) AS temp_bias,
    AVG(ABS(f.temperature - o.temperature)) AS temp_mae,
    SQRT(AVG(POWER(f.temperature - o.temperature, 2))) AS temp_rmse,

    -- Precipitation metrics
    AVG(f.precipitation - o.precipitation) AS precip_bias,
    AVG(ABS(f.precipitation - o.precipitation)) AS precip_mae

FROM weather_forecasts f
LEFT JOIN weather_observations o
    ON f.forecast_valid_time = time_bucket('1 hour', o.observation_time)
    AND f.location_id = o.location_id
GROUP BY day, f.location_id, f.lead_time_hours;

-- Refresh policy: update daily
SELECT add_continuous_aggregate_policy('forecast_accuracy_daily',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');
```

---

## 6. Query Performance Optimization

### Partitioning Strategy

**Recommendation**: Partition by `forecast_valid_time` (not `issue_time`)

**Rationale**:
- Most queries target "forecasts for future period X" (valid time)
- Verification joins on `valid_time = observation_time`
- Grafana dashboards show "forecast for next 7 days" (valid time ranges)

**Alternative**: If primarily analyzing "forecast skill by issue date", partition by `issue_time`. However, this complicates verification queries.

### Index Strategy

```sql
-- Primary index: Queries targeting specific valid time
CREATE INDEX idx_forecasts_valid_time_location
    ON weather_forecasts (forecast_valid_time, location_id)
    INCLUDE (temperature, precipitation);

-- Secondary index: Queries grouping by issue time
CREATE INDEX idx_forecasts_issue_time_location
    ON weather_forecasts (issue_time, location_id);

-- Specialized index: Lead time analysis
CREATE INDEX idx_forecasts_lead_time
    ON weather_forecasts (lead_time_hours, location_id, forecast_valid_time);

-- Partial index: Latest forecasts (hot queries)
CREATE INDEX idx_latest_forecasts
    ON weather_forecasts (location_id, forecast_valid_time DESC)
    WHERE issue_time > NOW() - INTERVAL '24 hours';
```

### Query Patterns

**Pattern 1: Get latest forecast for next 24 hours**
```sql
WITH latest_issue AS (
    SELECT MAX(issue_time) AS max_issue
    FROM weather_forecasts
    WHERE location_id = 'nyc'
)
SELECT forecast_valid_time, temperature, precipitation
FROM weather_forecasts
WHERE location_id = 'nyc'
    AND issue_time = (SELECT max_issue FROM latest_issue)
    AND forecast_valid_time BETWEEN NOW() AND NOW() + INTERVAL '24 hours'
ORDER BY forecast_valid_time;
```

**Pattern 2: Compare forecast evolution for specific target time**
```sql
-- How did temperature forecast for "tomorrow 3pm" change as issue time approached?
SELECT
    issue_time,
    temperature,
    (EXTRACT(EPOCH FROM ('2025-12-22 15:00:00+00' - issue_time)) / 3600) AS hours_before_valid
FROM weather_forecasts
WHERE location_id = 'nyc'
    AND forecast_valid_time = '2025-12-22 15:00:00+00'
ORDER BY issue_time;
```

**Pattern 3: Forecast accuracy by lead time**
```sql
SELECT
    lead_time_hours,
    AVG(ABS(f.temperature - o.temperature)) AS mae_temperature
FROM weather_forecasts f
JOIN weather_observations o
    ON f.forecast_valid_time = o.observation_time
    AND f.location_id = o.location_id
WHERE f.location_id = 'nyc'
    AND f.forecast_valid_time >= NOW() - INTERVAL '30 days'
GROUP BY lead_time_hours
ORDER BY lead_time_hours;
```

**Sources**:
- [TimescaleDB Query Performance Guide](https://www.tigerdata.com/learn/guide-to-postgresql-performance)
- [PostgreSQL Time-Series Best Practices - Alibaba Cloud](https://www.alibabacloud.com/blog/best-practices-for-postgresql-time-series-database-design_599374)

---

## 7. Storage Efficiency Analysis

### Row Count Estimation

**Scenario**: OpenWeatherMap hourly forecasts for 1 location
- Issue frequency: Every 3 hours (8 times per day)
- Forecast horizon: 48 hours (48 data points per issue)
- Locations: 1

**Daily rows**: 8 issues × 48 hours × 1 location = **384 rows/day**
**Annual rows**: 384 × 365 = **140,160 rows/year**

**Multi-location scaling**:
- 10 locations: 1.4M rows/year
- 100 locations: 14M rows/year

### Storage Size Estimation

**Row size** (tall format):
```
issue_time: 8 bytes (TIMESTAMPTZ)
forecast_valid_time: 8 bytes (TIMESTAMPTZ)
lead_time_hours: 2 bytes (SMALLINT, computed)
location_id: ~8 bytes (avg TEXT length)
latitude: 8 bytes (DOUBLE)
longitude: 8 bytes (DOUBLE)
forecast_source: ~15 bytes (avg TEXT)
temperature: 8 bytes (DOUBLE)
precipitation: 8 bytes (DOUBLE)
wind_speed: 8 bytes (DOUBLE)
humidity: 8 bytes (DOUBLE)
pressure: 8 bytes (DOUBLE)
cloud_cover: 2 bytes (SMALLINT)
visibility: 4 bytes (INTEGER)
weather_condition: ~10 bytes (avg TEXT)
ingestion_time: 8 bytes (TIMESTAMPTZ)
Row overhead: ~24 bytes (PostgreSQL tuple header)
─────────────────────────────────
Total: ~145 bytes/row (uncompressed)
```

**Annual storage** (1 location, uncompressed):
- 140,160 rows × 145 bytes = **20.3 MB/year**

**With TimescaleDB compression** (80-95% reduction):
- Compressed: 20.3 MB × 0.15 = **~3 MB/year**

**100 locations, 5 years, compressed**:
- 100 × 5 × 3 MB = **1.5 GB** (manageable on Raspberry Pi)

### Comparison: Tall vs. Wide Format

**Wide format** (168-hour horizon, 10 metrics):
```
1 row per issue × (168 forecast hours × 10 metrics) = 1,680 columns
Row size: ~13,500 bytes (massive)
Daily rows: 8 issues × 1 location = 8 rows/day
Annual storage: 8 × 365 × 13,500 = 39.4 MB/year (UNCOMPRESSED)
PostgreSQL limit: 1,600 columns (EXCEEDS LIMIT)
```

**Verdict**: Tall format is **more storage-efficient** after compression and stays within PostgreSQL limits.

---

## 8. NDP Implementation Roadmap

### Phase 1: Bronze Layer (Current - AIR-005)
**Status**: ✅ Complete
- Parquet storage of current weather API responses
- No forecast data yet (single-point "now" observations)

### Phase 2: Forecast Ingestion (DP-002 Scope)
**Tasks**:
1. Extend `HttpPollingSource` to support forecast APIs:
   - OpenWeatherMap 5-day/3-hour forecast (free tier)
   - Parse JSON response into multiple `TimeSeriesPoint` records
2. Create `ForecastParser` implementing `ResponseParser` trait:
   ```rust
   pub struct ForecastParser {
       location_id: String,
   }

   impl ResponseParser for ForecastParser {
       async fn parse(&self, response: &str, source_time: DateTime<Utc>)
           -> Result<Vec<TimeSeriesPoint>, CoreError> {
           // Parse JSON forecast array
           // Extract issue_time (API response time)
           // For each forecast hour:
           //   - Create TimeSeriesPoint with forecast_valid_time
           //   - Add fields: temperature, precipitation, etc.
       }
   }
   ```
3. Store in Bronze Parquet with schema:
   ```
   issue_time, forecast_valid_time, location, temperature_forecast, ...
   ```

### Phase 3: Silver Layer - TimescaleDB Migration (DP-002)
**Tasks**:
1. Deploy TimescaleDB container (replace DuckDB)
2. Create `weather_forecasts` hypertable (schema above)
3. ETL pipeline: Parquet → TimescaleDB
   - Option A: DuckDB `read_parquet()` → `COPY TO` PostgreSQL
   - Option B: Rust async batch loader
4. Create continuous aggregates for forecast accuracy
5. Grafana dashboard updates (PostgreSQL datasource)

### Phase 4: Forecast Verification (FE-001 Scope)
**Tasks**:
1. Create `weather_observations` table from Bronze sensor data
2. Implement verification queries (LEFT JOIN pattern)
3. Build Grafana dashboard showing:
   - Forecast vs. actual comparison charts
   - MAE/RMSE by lead time
   - Forecast skill heatmaps
4. Optional: Alerting on poor forecast accuracy (>5°C MAE)

---

## 9. Alternative Considerations

### DuckDB Virtual Lakehouse Approach (Current NDP)

**If staying with DuckDB** (not migrating to TimescaleDB):

**Bronze Parquet Schema**:
```parquet
issue_time: INT64 (timestamp_millis)
forecast_valid_time: INT64 (timestamp_millis)
location_id: BYTE_ARRAY (UTF8)
temperature_forecast: DOUBLE
precipitation_forecast: DOUBLE
...
```

**DuckDB Silver Views**:
```sql
CREATE VIEW silver_forecasts AS
SELECT
    CAST(from_unixtime(issue_time / 1000) AS TIMESTAMP) AS issue_time,
    CAST(from_unixtime(forecast_valid_time / 1000) AS TIMESTAMP) AS forecast_valid_time,
    CAST((forecast_valid_time - issue_time) / 3600000 AS INTEGER) AS lead_time_hours,
    location_id,
    temperature_forecast,
    ROUND(temperature_forecast, 1) AS temperature_display
FROM read_parquet('/data/bronze/forecasts/*.parquet')
WHERE temperature_forecast BETWEEN -50 AND 60;  -- Data quality filter

CREATE VIEW forecast_verification AS
SELECT
    f.issue_time,
    f.forecast_valid_time,
    f.location_id,
    f.temperature_forecast,
    o.temperature AS temperature_observed,
    (f.temperature_forecast - o.temperature) AS error
FROM silver_forecasts f
LEFT JOIN read_parquet('/data/bronze/observations/*.parquet') o
    ON f.forecast_valid_time = o.timestamp
    AND f.location_id = o.location;
```

**Pros**:
- ✅ No new database to learn
- ✅ Continues current virtual lakehouse pattern
- ✅ Parquet-native storage

**Cons**:
- ❌ No continuous aggregates (must pre-compute with cron)
- ❌ No automatic compression policies
- ❌ Grafana plugin still broken on ARM64 (requires SQLite export)

---

## 10. Final Recommendations Summary

### ✅ Recommended Approach for NDP

| Decision | Recommendation | Rationale |
|----------|---------------|-----------|
| **Time Representation** | Absolute timestamps (issue + valid) | Query flexibility, verification simplicity |
| **Schema Format** | Tall/long (normalized) | TimescaleDB optimization, compression, scalability |
| **Primary Key** | `(issue_time, forecast_valid_time, location_id)` | Supports all query patterns |
| **Partitioning** | Partition by `forecast_valid_time` | Most queries target future periods |
| **Storage Backend** | TimescaleDB (DP-002 phase) | Continuous aggregates, proven on Pi |
| **Verification Strategy** | LEFT JOIN on `(valid_time, location)` | Standard meteorological practice |
| **Retention** | 90 days raw, 2 years aggregated | Balance detail vs. storage |
| **Compression** | After 7 days (80-95% reduction) | Automatic via TimescaleDB policy |

### Migration Path

**Short-term** (Continue with current):
- Bronze: Parquet with `(issue_time, forecast_valid_time, ...)`
- Silver: DuckDB views + SQLite export workaround
- Verification: Manual SQL queries

**Medium-term** (DP-002 scope):
- Migrate to TimescaleDB hypertables
- Implement continuous aggregates for accuracy metrics
- Build verification dashboards in Grafana

**Long-term** (Cloud migration):
- Keep Parquet in Bronze (portable to S3/GCS)
- Add Apache Iceberg metadata layer
- Scale TimescaleDB to Timescale Cloud or self-hosted cluster

---

## References & Sources

### Meteorological Standards
- [Init vs Valid Time Conventions - WeatherBench 2](https://weatherbench2.readthedocs.io/en/latest/init-vs-valid-time.html)
- [Meteorological Time Definition - NWS](https://www.weather.gov/tg/time)
- [NOAA Forecast Verification - MDL](https://vlab.noaa.gov/web/mdl/fv)
- [NDFD Verification Help](https://vlab.noaa.gov/web/mdl/ndfd-verification-help)
- [Forecast Verification Methods - CAWCR](https://www.cawcr.gov.au/projects/verification/verif_web_page.html)

### Time-Series Database Best Practices
- [Designing Wide vs. Narrow Postgres Tables - Timescale](https://www.tigerdata.com/learn/designing-your-database-schema-wide-vs-narrow-postgres-tables)
- [Best Practices for Time-Series Data Modeling - TimescaleDB](https://www.tigerdata.com/learn/best-practices-time-series-data-modeling-single-or-multiple-partitioned-tables-aka-hypertables)
- [AWS RDS High-Performance Time-Series Design](https://aws.amazon.com/blogs/database/designing-high-performance-time-series-data-tables-on-amazon-rds-for-postgresql/)
- [PostgreSQL Time-Series Best Practices - Alibaba Cloud](https://www.alibabacloud.com/blog/best-practices-for-postgresql-time-series-database-design_599374)
- [TimescaleDB Query Performance Guide](https://www.tigerdata.com/learn/guide-to-postgresql-performance)

### NDP Internal Research
- [Time-Series Database Comparison](./Silver/timeseries-database-comparison.md)
- [Silver Layer Decision Matrix](./Silver/DECISION-MATRIX.md)
- [Platform Architecture Overview](../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)

---

**Document Version**: 1.0
**Last Updated**: 2025-12-21
**Author**: Research Agent (NDP)
**Status**: Complete - Ready for Architecture Review
