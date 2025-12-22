# AIR-006: NWS Forecast Storage - TimescaleDB Data Model Specification

**Version**: 1.0.0
**Last Updated**: 2025-12-21
**Status**: Specification Phase
**SPARC Phase**: Specification

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Design Principles](#design-principles)
3. [Bronze Layer (Parquet)](#bronze-layer-parquet)
4. [Silver Layer (TimescaleDB)](#silver-layer-timescaledb)
5. [Schema Design Rationale](#schema-design-rationale)
6. [Continuous Aggregates](#continuous-aggregates)
7. [Compression & Retention Policies](#compression--retention-policies)
8. [Query Patterns](#query-patterns)
9. [Migration from DuckDB](#migration-from-duckdb)
10. [Storage Estimation](#storage-estimation)

---

## Executive Summary

AIR-006 introduces **NWS hourly forecast tracking** with forecast evolution analysis and verification capabilities. The data model uses **tall format with absolute timestamps** to support:

1. **Forecast Evolution**: "How did tomorrow 3pm's forecast change over time?"
2. **Verification**: JOIN forecasts with observations to compute accuracy metrics
3. **TimescaleDB Optimization**: Hypertables, continuous aggregates, compression

### Key Design Decisions

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| **Time Representation** | Absolute timestamps (`issue_time`, `forecast_valid_time`) | Query flexibility, verification JOINs |
| **Schema Format** | Tall/normalized (1 row per hour per issue) | TimescaleDB hypertable optimization, compression |
| **Partitioning Key** | `forecast_valid_time` | Queries target "forecasts for future period X" |
| **Primary Key** | `(issue_time, forecast_valid_time, location_id)` | Tracks forecast evolution |
| **Compression** | After 7 days (80-95% reduction) | Automatic TimescaleDB policy |
| **Retention** | 90 days raw forecasts, 2 years aggregates | Balance detail vs. storage |

**Reference Research**: [Weather Forecast Time-Series Schema Research](../../../product/research/weather-forecast-timeseries-schema.md)

---

## Design Principles

### 1. Meteorological Standards Alignment

Following **NOAA/NWS conventions** for forecast time representation:

| Term | Definition | Example |
|------|------------|---------|
| **Issue Time** (Reference Time) | When forecast was generated/retrieved | `2025-12-21 12:00:00 UTC` |
| **Forecast Valid Time** (Target Time) | When forecasted conditions apply | `2025-12-21 15:00:00 UTC` |
| **Lead Time** | Duration between issue and valid time | `3 hours` (computed) |

**Sources**:
- [WeatherBench 2: Init vs Valid Time](https://weatherbench2.readthedocs.io/en/latest/init-vs-valid-time.html)
- [NWS Meteorological Time Definition](https://www.weather.gov/tg/time)

### 2. TimescaleDB Optimization

- **Hypertables**: Partition by `forecast_valid_time` for efficient range queries
- **Compression**: Columnar compression after 7 days (80-95% reduction)
- **Continuous Aggregates**: Auto-refreshing materialized views for forecast accuracy
- **Retention Policies**: Automatic deletion of old data

### 3. NDP Integration

- **Bronze Layer**: Raw JSON responses stored in Parquet (append-only, audit trail)
- **Silver Layer**: TimescaleDB hypertables (queryable, indexed, compressed)
- **ETL**: Periodic batch load from Parquet to TimescaleDB (DuckDB → PostgreSQL)

---

## Bronze Layer (Parquet)

### Purpose

- **Immutable audit trail** of raw API responses
- **Recovery**: Re-process if Silver layer corrupted
- **Research**: Access raw JSON for ML feature engineering

### Schema

**File Pattern**: `/data/nws-forecasts/YYYY-MM-DD_HH.parquet`

**Parquet Schema**:
```
message nws_forecast_raw {
  required int64 ingestion_time (TIMESTAMP_MILLIS);
  required binary api_response (STRING);       // Raw JSON from NWS API
  required binary station_id (STRING);         // "KAAF"
  optional binary grid_point (STRING);         // "TAE/58,53"
  required binary endpoint_type (STRING);      // "hourly_forecast"
}
```

**Partitioning**: Daily files, hourly ingestion (NWS updates every 1 hour)

**Retention**: 365 days (long-term archive)

**Sample Data**:
```json
{
  "ingestion_time": 1734782400000,
  "api_response": "{\"properties\":{\"periods\":[...]}}",
  "station_id": "KAAF",
  "grid_point": "TAE/58,53",
  "endpoint_type": "hourly_forecast"
}
```

---

## Silver Layer (TimescaleDB)

### 1. Observations Table

**Purpose**: Actual observed weather from NWS stations for verification

**Schema**:
```sql
-- Weather observations from NWS stations (actual conditions)
CREATE TABLE nws_observations (
    -- Temporal dimension
    observation_time TIMESTAMPTZ NOT NULL,

    -- Spatial dimensions
    station_id TEXT NOT NULL,
    location_id TEXT NOT NULL,  -- "sgi-kaaf" (Saint George Island - KAAF station)
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    elevation_m DOUBLE PRECISION,

    -- Meteorological fields (NWS standard units)
    temperature_c DOUBLE PRECISION,              -- Air temperature (Celsius)
    dewpoint_c DOUBLE PRECISION,                 -- Dew point temperature
    humidity_pct DOUBLE PRECISION,               -- Relative humidity (0-100)
    wind_speed_kmh DOUBLE PRECISION,             -- Wind speed (km/h)
    wind_direction_deg SMALLINT,                 -- Wind direction (0-360°)
    wind_gust_kmh DOUBLE PRECISION,              -- Wind gust speed
    pressure_pa DOUBLE PRECISION,                -- Barometric pressure (Pascals)
    sea_level_pressure_pa DOUBLE PRECISION,      -- Sea level pressure
    visibility_m INTEGER,                        -- Visibility distance (meters)
    cloud_cover TEXT,                            -- Cloud layer description
    precipitation_3h_mm DOUBLE PRECISION,        -- 3-hour precipitation

    -- Quality control
    quality_control JSONB,                       -- {"temperature": "V", "wind_speed": "C"}
    text_description TEXT,                       -- "Clear", "Partly Cloudy"

    -- Metadata
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source TEXT NOT NULL DEFAULT 'nws_api',

    PRIMARY KEY (observation_time, station_id)
);

-- Create TimescaleDB hypertable partitioned by observation time
SELECT create_hypertable('nws_observations', 'observation_time',
    chunk_time_interval => INTERVAL '1 day');

-- Compression policy: compress after 7 days
ALTER TABLE nws_observations SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'station_id,location_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);
SELECT add_compression_policy('nws_observations', INTERVAL '7 days');

-- Retention policy: delete observations older than 365 days
SELECT add_retention_policy('nws_observations', INTERVAL '365 days');

-- Indexes for common queries
CREATE INDEX idx_observations_station_time
    ON nws_observations (station_id, observation_time DESC);

CREATE INDEX idx_observations_location
    ON nws_observations (location_id, observation_time DESC);
```

**Field Mapping from NWS API**:
```javascript
// NWS API Response → TimescaleDB Field
{
  "timestamp": "2025-12-21T18:35:00+00:00"           → observation_time
  "temperature": {"value": 19.0, "unitCode": "..."}  → temperature_c
  "dewpoint": {"value": 9.0}                         → dewpoint_c
  "relativeHumidity": {"value": 52.28}               → humidity_pct
  "windSpeed": {"value": 11.124}                     → wind_speed_kmh
  "windDirection": {"value": 130}                    → wind_direction_deg
  "barometricPressure": {"value": 102302.8}          → pressure_pa
  "visibility": {"value": 16093.44}                  → visibility_m
}
```

---

### 2. Forecasts Table

**Purpose**: NWS hourly forecast predictions with evolution tracking

**Schema**:
```sql
-- Weather forecast predictions from NWS hourly forecast API
CREATE TABLE nws_forecasts (
    -- Temporal dimensions (absolute timestamps)
    issue_time TIMESTAMPTZ NOT NULL,            -- When forecast was generated/retrieved
    forecast_valid_time TIMESTAMPTZ NOT NULL,   -- When forecast applies (target time)

    -- Computed lead time for horizon analysis
    lead_time_hours SMALLINT GENERATED ALWAYS AS
        (EXTRACT(EPOCH FROM (forecast_valid_time - issue_time)) / 3600) STORED,

    -- Spatial dimensions
    location_id TEXT NOT NULL,                  -- "sgi-kaaf"
    grid_point TEXT NOT NULL,                   -- "TAE/58,53"
    station_id TEXT,                            -- "KAAF" (nearest station)
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,

    -- Forecast metadata
    forecast_source TEXT NOT NULL DEFAULT 'nws_hourly',
    forecast_period_number SMALLINT,            -- Period sequence from API (1, 2, 3...)
    is_daytime BOOLEAN,

    -- Meteorological fields (NWS forecast)
    temperature INTEGER,                        -- Temperature (Fahrenheit from API)
    temperature_c DOUBLE PRECISION,             -- Converted to Celsius
    temperature_trend TEXT,                     -- "rising", "falling", null
    dewpoint_c DOUBLE PRECISION,                -- Dew point (Celsius)
    humidity_pct DOUBLE PRECISION,              -- Relative humidity (0-100)

    -- Wind (NWS returns strings like "5 mph")
    wind_speed TEXT,                            -- Raw API value: "5 mph", "10 to 15 mph"
    wind_speed_kmh DOUBLE PRECISION,            -- Parsed numeric (km/h)
    wind_direction TEXT,                        -- Cardinal direction: "E", "NW"
    wind_direction_deg SMALLINT,                -- Converted to degrees (0-360)

    -- Precipitation
    precipitation_probability_pct SMALLINT,     -- Chance of precipitation (0-100)

    -- Conditions
    short_forecast TEXT,                        -- "Sunny", "Partly Cloudy"
    detailed_forecast TEXT,                     -- Full sentence description
    icon_url TEXT,                              -- NWS weather icon URL

    -- Ingestion metadata (NDP standard)
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (issue_time, forecast_valid_time, location_id)
);

-- Create TimescaleDB hypertable partitioned by forecast valid time
SELECT create_hypertable('nws_forecasts', 'forecast_valid_time',
    chunk_time_interval => INTERVAL '1 day');

-- Compression policy: compress after 7 days
ALTER TABLE nws_forecasts SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id,issue_time,grid_point',
    timescaledb.compress_orderby = 'forecast_valid_time DESC'
);
SELECT add_compression_policy('nws_forecasts', INTERVAL '7 days');

-- Retention policy: delete raw forecasts older than 90 days
SELECT add_retention_policy('nws_forecasts', INTERVAL '90 days');

-- Indexes for common query patterns
CREATE INDEX idx_forecasts_issue_time
    ON nws_forecasts (issue_time, location_id);

CREATE INDEX idx_forecasts_lead_time
    ON nws_forecasts (lead_time_hours, location_id, forecast_valid_time);

-- Partial index for latest forecasts (hot data, frequently queried)
CREATE INDEX idx_forecasts_latest
    ON nws_forecasts (location_id, forecast_valid_time DESC)
    WHERE issue_time > NOW() - INTERVAL '24 hours';

-- Verification join optimization
CREATE INDEX idx_forecasts_verification
    ON nws_forecasts (forecast_valid_time, location_id)
    INCLUDE (temperature_c, wind_speed_kmh);
```

**Field Mapping from NWS Hourly Forecast API**:
```javascript
// NWS Hourly Forecast → TimescaleDB Field
{
  "startTime": "2025-12-21T13:00:00-05:00"           → forecast_valid_time
  "number": 1                                        → forecast_period_number
  "temperature": 67                                  → temperature (F)
  "temperatureUnit": "F"                             → (convert to temperature_c)
  "temperatureTrend": null                           → temperature_trend
  "dewpoint": {"value": 10, "unitCode": "degC"}      → dewpoint_c
  "relativeHumidity": {"value": 54}                  → humidity_pct
  "windSpeed": "5 mph"                               → wind_speed (raw)
  "windDirection": "E"                               → wind_direction
  "shortForecast": "Sunny"                           → short_forecast
  "probabilityOfPrecipitation": {"value": 0}         → precipitation_probability_pct
}
```

---

## Schema Design Rationale

### Why Tall Format (Normalized)?

**Alternative**: Wide format with columns `temp_hour_01`, `temp_hour_02`, ..., `temp_hour_168`

**Reasons for Tall Format**:

| Criteria | Tall Format | Wide Format | Winner |
|----------|-------------|-------------|--------|
| **TimescaleDB Hypertables** | ✅ Partition by `valid_time` | ❌ Can't partition by hour column | **Tall** |
| **Compression** | ✅ Columnar (80-95% reduction) | ⚠️ Row-based only | **Tall** |
| **PostgreSQL Limits** | ✅ No issues | ❌ 1600 column limit (168 hrs × 10 fields = 1680) | **Tall** |
| **Verification JOINs** | ✅ Direct JOIN on timestamps | ❌ Complex unpivoting required | **Tall** |
| **Grafana Queries** | ✅ Standard SQL | ⚠️ Complex CASE statements | **Tall** |
| **Schema Evolution** | ✅ Add columns easily | ❌ Requires ALTER TABLE | **Tall** |

**Example Row Count** (1 location):
- Issue frequency: Every 1 hour (24 times/day)
- Forecast horizon: 156 hours (6.5 days)
- **Daily rows**: 24 × 156 = **3,744 rows/day**
- **Annual rows**: 3,744 × 365 = **1.37M rows/year**

With TimescaleDB compression (80% reduction), this is highly efficient.

---

### Why Absolute Timestamps (not Delta Time)?

**Alternative**: Store `issue_time` + `forecast_hour_offset` (e.g., `+6h`)

**Reasons for Absolute Timestamps**:

| Aspect | Absolute | Delta | Winner |
|--------|----------|-------|--------|
| **Verification JOINs** | `f.valid_time = o.time` | `f.issue + offset = o.time` (computed) | **Absolute** |
| **Grafana Charts** | Native timestamp axis | Must compute on client | **Absolute** |
| **Query Simplicity** | `WHERE valid_time = '2025-12-21 15:00'` | `WHERE issue + (offset * 1h) = ...` | **Absolute** |
| **Index Performance** | B-tree on both timestamps | Only on offset | **Absolute** |
| **Storage** | 16 bytes (2 timestamps) | 10 bytes (1 timestamp + smallint) | Delta |

**Decision**: Query flexibility outweighs 6-byte storage savings per row.

**Hybrid Approach**: `lead_time_hours GENERATED ALWAYS AS ...` provides best of both worlds.

---

## Continuous Aggregates

### 1. Forecast Accuracy by Lead Time

**Purpose**: Daily summary of forecast skill stratified by forecast horizon

```sql
CREATE MATERIALIZED VIEW forecast_accuracy_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', f.forecast_valid_time) AS day,
    f.location_id,
    f.lead_time_hours,
    COUNT(*) AS num_forecasts,
    COUNT(o.temperature_c) AS num_verified,

    -- Temperature metrics
    AVG(f.temperature_c - o.temperature_c) AS temp_bias_c,
    AVG(ABS(f.temperature_c - o.temperature_c)) AS temp_mae_c,
    SQRT(AVG(POWER(f.temperature_c - o.temperature_c, 2))) AS temp_rmse_c,

    -- Wind speed metrics
    AVG(f.wind_speed_kmh - o.wind_speed_kmh) AS wind_bias_kmh,
    AVG(ABS(f.wind_speed_kmh - o.wind_speed_kmh)) AS wind_mae_kmh,

    -- Humidity metrics
    AVG(f.humidity_pct - o.humidity_pct) AS humidity_bias_pct,
    AVG(ABS(f.humidity_pct - o.humidity_pct)) AS humidity_mae_pct

FROM nws_forecasts f
LEFT JOIN nws_observations o
    ON f.forecast_valid_time = time_bucket('1 hour', o.observation_time)
    AND f.location_id = o.location_id
GROUP BY day, f.location_id, f.lead_time_hours;

-- Auto-refresh policy: update daily at 2am
SELECT add_continuous_aggregate_policy('forecast_accuracy_daily',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');

-- Retention: keep accuracy metrics for 2 years
SELECT add_retention_policy('forecast_accuracy_daily', INTERVAL '730 days');
```

**Query Example** (Grafana):
```sql
-- Show forecast accuracy degradation with lead time
SELECT
    lead_time_hours AS "Lead Time (hours)",
    temp_mae_c AS "Temperature MAE (°C)",
    wind_mae_kmh AS "Wind Speed MAE (km/h)"
FROM forecast_accuracy_daily
WHERE location_id = 'sgi-kaaf'
    AND day >= NOW() - INTERVAL '30 days'
ORDER BY lead_time_hours;
```

---

### 2. Forecast Evolution Tracking

**Purpose**: Track how forecasts for a specific target time changed as issue time approached

```sql
CREATE MATERIALIZED VIEW forecast_evolution_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', f.forecast_valid_time) AS valid_hour,
    f.location_id,
    f.issue_time,
    f.lead_time_hours,
    AVG(f.temperature_c) AS avg_temperature_c,
    MIN(f.temperature_c) AS min_temperature_c,
    MAX(f.temperature_c) AS max_temperature_c,
    STDDEV(f.temperature_c) AS stddev_temperature_c,

    AVG(f.precipitation_probability_pct) AS avg_precip_prob,
    COUNT(*) AS forecast_count

FROM nws_forecasts f
GROUP BY valid_hour, f.location_id, f.issue_time, f.lead_time_hours;

-- Refresh every hour
SELECT add_continuous_aggregate_policy('forecast_evolution_hourly',
    start_offset => INTERVAL '6 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
```

**Query Example**:
```sql
-- How did the forecast for "tomorrow 3pm" change over time?
SELECT
    issue_time,
    avg_temperature_c,
    lead_time_hours,
    (EXTRACT(EPOCH FROM ('2025-12-22 15:00:00+00' - issue_time)) / 3600) AS hours_before
FROM forecast_evolution_hourly
WHERE valid_hour = time_bucket('1 hour', '2025-12-22 15:00:00+00'::TIMESTAMPTZ)
    AND location_id = 'sgi-kaaf'
ORDER BY issue_time DESC;
```

---

## Compression & Retention Policies

### Compression Strategy

**TimescaleDB Native Compression** (columnar format):

```sql
-- nws_observations compression
ALTER TABLE nws_observations SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'station_id,location_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);
SELECT add_compression_policy('nws_observations', INTERVAL '7 days');
```

**Expected Compression Ratios**:
- Observations: **85-90%** (mostly numeric fields)
- Forecasts: **80-85%** (includes TEXT fields)

**Storage Savings Example**:
- Uncompressed: 1.37M rows × 200 bytes = **274 MB/year**
- Compressed (85%): 274 MB × 0.15 = **41 MB/year**

---

### Retention Policies

| Table | Raw Retention | Aggregate Retention | Rationale |
|-------|---------------|---------------------|-----------|
| `nws_observations` | 365 days | N/A | Long-term weather history |
| `nws_forecasts` | 90 days | 730 days (2 years) | Balance detail vs. storage |
| `forecast_accuracy_daily` | N/A | 730 days | Long-term skill tracking |
| `forecast_evolution_hourly` | N/A | 90 days | Recent evolution analysis |

**Implementation**:
```sql
-- Delete old raw forecasts
SELECT add_retention_policy('nws_forecasts', INTERVAL '90 days');

-- Keep aggregates longer
SELECT add_retention_policy('forecast_accuracy_daily', INTERVAL '730 days');
```

---

## Query Patterns

### Pattern 1: Latest Forecast for Next 24 Hours

**Use Case**: Grafana dashboard showing current forecast

```sql
WITH latest_issue AS (
    SELECT MAX(issue_time) AS max_issue
    FROM nws_forecasts
    WHERE location_id = 'sgi-kaaf'
)
SELECT
    forecast_valid_time AS time,
    temperature_c AS "Temperature (°C)",
    precipitation_probability_pct AS "Precip Probability (%)",
    wind_speed_kmh AS "Wind Speed (km/h)",
    short_forecast AS "Conditions"
FROM nws_forecasts
WHERE location_id = 'sgi-kaaf'
    AND issue_time = (SELECT max_issue FROM latest_issue)
    AND forecast_valid_time BETWEEN NOW() AND NOW() + INTERVAL '24 hours'
ORDER BY forecast_valid_time;
```

---

### Pattern 2: Forecast Evolution for Specific Target Time

**Use Case**: "How did tomorrow 3pm's forecast change?"

```sql
SELECT
    issue_time,
    temperature_c,
    precipitation_probability_pct,
    (EXTRACT(EPOCH FROM ('2025-12-22 15:00:00+00'::TIMESTAMPTZ - issue_time)) / 3600)::INTEGER AS hours_before_valid
FROM nws_forecasts
WHERE location_id = 'sgi-kaaf'
    AND forecast_valid_time = '2025-12-22 15:00:00+00'::TIMESTAMPTZ
ORDER BY issue_time DESC;
```

**Expected Output**:
```
issue_time              | temperature_c | precip_prob | hours_before_valid
2025-12-22 14:00:00+00 | 18.3          | 10          | 1
2025-12-22 12:00:00+00 | 18.1          | 15          | 3
2025-12-22 06:00:00+00 | 17.8          | 20          | 9
2025-12-21 18:00:00+00 | 17.5          | 25          | 21
```

---

### Pattern 3: Forecast Verification (Actual vs. Predicted)

**Use Case**: Accuracy metrics for ML model evaluation

```sql
SELECT
    f.forecast_valid_time,
    f.lead_time_hours,
    f.temperature_c AS forecast_temp,
    o.temperature_c AS observed_temp,
    (f.temperature_c - o.temperature_c) AS error_c,
    ABS(f.temperature_c - o.temperature_c) AS abs_error_c
FROM nws_forecasts f
INNER JOIN nws_observations o
    ON f.forecast_valid_time = o.observation_time
    AND f.location_id = o.location_id
WHERE f.location_id = 'sgi-kaaf'
    AND f.issue_time >= NOW() - INTERVAL '7 days'
ORDER BY f.forecast_valid_time, f.lead_time_hours;
```

---

### Pattern 4: Forecast Skill by Lead Time

**Use Case**: "How accurate are 6-hour vs. 24-hour forecasts?"

```sql
SELECT
    lead_time_hours,
    COUNT(*) AS num_forecasts,
    AVG(temp_mae_c) AS avg_mae,
    MIN(temp_mae_c) AS best_mae,
    MAX(temp_mae_c) AS worst_mae
FROM forecast_accuracy_daily
WHERE location_id = 'sgi-kaaf'
    AND day >= NOW() - INTERVAL '30 days'
GROUP BY lead_time_hours
ORDER BY lead_time_hours;
```

---

## Migration from DuckDB

### Current State (DP-001)

NDP currently uses **DuckDB virtual views** over Bronze Parquet files.

**Limitations**:
- No continuous aggregates (manual cron jobs)
- No automatic compression
- Grafana plugin broken on ARM64 (requires SQLite export)

### Migration Path to TimescaleDB

**Phase 1**: Parallel Run (DP-002)
1. Keep DuckDB running for existing dashboards
2. Deploy TimescaleDB container
3. ETL Bronze Parquet → TimescaleDB (daily batch)
4. Test queries and performance

**Phase 2**: Grafana Migration
1. Add PostgreSQL datasource
2. Recreate dashboards using TimescaleDB
3. Compare query performance (expect 2-5x improvement)
4. Switch dashboards to TimescaleDB

**Phase 3**: Decommission DuckDB
1. Verify all queries migrated
2. Stop DuckDB container
3. Keep Parquet as Bronze archive

### ETL Pipeline (DuckDB → TimescaleDB)

**Option A**: DuckDB COPY TO PostgreSQL
```sql
-- DuckDB query to export to PostgreSQL
COPY (
    SELECT
        CAST(from_unixtime(issue_time / 1000) AS TIMESTAMP) AS issue_time,
        CAST(from_unixtime(forecast_valid_time / 1000) AS TIMESTAMP) AS forecast_valid_time,
        location_id,
        temperature_c,
        -- ... other fields
    FROM read_parquet('/data/nws-forecasts/*.parquet')
) TO 'postgresql://timescale:5432/ndp?table=nws_forecasts';
```

**Option B**: Rust Batch Loader (Preferred for NDP)
```rust
// apps/etl-service/src/nws_forecast_etl.rs
pub async fn load_parquet_to_timescale(
    parquet_path: &Path,
    pg_pool: &PgPool,
) -> Result<usize, CoreError> {
    let df = ParquetReader::new(File::open(parquet_path)?)
        .finish()?;

    let mut tx = pg_pool.begin().await?;
    let mut count = 0;

    for row in df.iter() {
        sqlx::query(r#"
            INSERT INTO nws_forecasts (
                issue_time, forecast_valid_time, location_id,
                temperature_c, wind_speed_kmh, humidity_pct
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (issue_time, forecast_valid_time, location_id) DO NOTHING
        "#)
        .bind(&row.issue_time)
        .bind(&row.forecast_valid_time)
        .bind(&row.location_id)
        .bind(&row.temperature_c)
        .bind(&row.wind_speed_kmh)
        .bind(&row.humidity_pct)
        .execute(&mut *tx)
        .await?;

        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}
```

---

## Storage Estimation

### Row Count (1 Location: KAAF)

**Observations**:
- Frequency: Every 30-60 minutes (48 per day)
- **Daily rows**: 48
- **Annual rows**: 17,520

**Forecasts**:
- Issue frequency: Every 1 hour (24 per day)
- Forecast horizon: 156 hours (6.5 days)
- **Daily rows**: 24 × 156 = 3,744
- **Annual rows**: 1,366,560

**Total Annual Rows**: 1.38M rows/year

---

### Storage Size

**Per-Row Storage** (uncompressed):

| Table | Row Size | Annual Rows | Annual Storage |
|-------|----------|-------------|----------------|
| `nws_observations` | ~150 bytes | 17,520 | 2.6 MB |
| `nws_forecasts` | ~200 bytes | 1,366,560 | 273 MB |
| **Total Uncompressed** | | | **276 MB/year** |

**With TimescaleDB Compression** (85% reduction):
- Compressed: 276 MB × 0.15 = **41 MB/year**

**5-Year Storage** (compressed):
- 41 MB × 5 = **205 MB** (negligible on Raspberry Pi 5)

**Continuous Aggregates**:
- `forecast_accuracy_daily`: ~5 KB/day × 730 days = 3.6 MB
- `forecast_evolution_hourly`: ~10 KB/day × 90 days = 900 KB

**Total Storage (5 years)**: ~210 MB

---

## References

### Meteorological Standards
- [WeatherBench 2: Init vs Valid Time Conventions](https://weatherbench2.readthedocs.io/en/latest/init-vs-valid-time.html)
- [NWS Meteorological Time Definition](https://www.weather.gov/tg/time)
- [NOAA Forecast Verification - MDL](https://vlab.noaa.gov/web/mdl/fv)

### TimescaleDB Best Practices
- [Wide vs. Narrow Postgres Tables - Timescale](https://www.tigerdata.com/learn/designing-your-database-schema-wide-vs-narrow-postgres-tables)
- [Time-Series Data Modeling - TimescaleDB](https://www.tigerdata.com/learn/best-practices-time-series-data-modeling-single-or-multiple-partitioned-tables-aka-hypertables)
- [AWS RDS Time-Series Design](https://aws.amazon.com/blogs/database/designing-high-performance-time-series-data-tables-on-amazon-rds-for-postgresql/)

### NDP Internal Documentation
- [Weather Forecast Time-Series Schema Research](../../../product/research/weather-forecast-timeseries-schema.md)
- [NWS API Integration Research - KAAF Station](../../../docs/research/nws-api-integration-ksgi.md)
- [AIR-005 Architecture - HTTP Polling Source](../../air-005/architecture/ARCHITECTURE.md)
- [Platform Architecture Overview](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-21 | ndp-timescale-dev | Initial specification with Bronze/Silver schema, continuous aggregates, compression policies |

---

**Status**: ✅ Ready for Architecture Review

**Next Steps**:
1. Review with `ndp-architect` for ADR creation
2. Validate query patterns with `ndp-grafana-dev`
3. Implement ETL pipeline with `ndp-parquet-dev`
4. Create integration tests with `ndp-tester`
