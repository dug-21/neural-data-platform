# DuckDB Analytics Layer Specification

**Feature**: dp-001 - DuckDB Analytics Layer + Grafana Dashboards
**Component**: DuckDB Analytics Layer
**Version**: 1.0.0
**Status**: Specification
**Created**: 2025-12-18

---

## 1. DuckDB Container Requirements

### 1.1 Container Image Selection

**Primary Choice**: Official DuckDB CLI image

```yaml
# docker-compose.yml
services:
  duckdb:
    image: duckdb/duckdb:latest
    container_name: ndp-duckdb
    volumes:
      - /data:/data:ro                                    # Read-only Bronze layer
      - ./config/duckdb:/config:ro                        # SQL scripts
      - duckdb_data:/workspace                            # Persistent workspace
    working_dir: /workspace
    command: ["-readonly", "-init", "/config/init.sql"]
    restart: unless-stopped
    mem_limit: 512m
    mem_reservation: 256m
    cpus: 2
```

**Alternative**: Custom Dockerfile (if needed for extensions)

```dockerfile
FROM duckdb/duckdb:latest

# Install extensions if needed
RUN duckdb -c "INSTALL parquet; INSTALL json;"

COPY config/duckdb/init.sql /init.sql
ENTRYPOINT ["duckdb", "-init", "/init.sql"]
```

### 1.2 Volume Mounts

| Volume | Source | Target | Mode | Purpose |
|--------|--------|--------|------|---------|
| Data | `/data` (host) | `/data` | `ro` | Read-only access to Parquet files |
| Config | `./config/duckdb` | `/config` | `ro` | SQL initialization scripts |
| Workspace | `duckdb_data` (volume) | `/workspace` | `rw` | Persistent DuckDB database |

### 1.3 Memory and Resource Configuration

```yaml
# Resource limits for Raspberry Pi 5 (16GB total)
mem_limit: 512m              # Hard limit
mem_reservation: 256m        # Soft guarantee
cpus: 2                      # CPU quota
```

**Rationale**:
- 512MB should handle 30-day queries across 3 streams
- Pi 5 has quad-core CPU, allocate 2 for analytical workload
- Remaining resources for Grafana (256MB) and existing stack (900MB)

### 1.4 Initialization Script Approach

**Bootstrap sequence**:

1. Container starts with `-init /config/init.sql`
2. `init.sql` sources all view definitions
3. Views are created in memory or persisted based on configuration
4. Container remains running for Grafana connections

**Initialization Script** (`config/duckdb/init.sql`):

```sql
-- DuckDB Initialization Script
-- Neural Data Platform - Bronze to Silver Views

.echo on
.mode box

-- Load required extensions
INSTALL parquet;
LOAD parquet;

-- Create persistent database (if needed)
-- ATTACH 'file:///workspace/ndp.db' AS ndp;
-- USE ndp;

-- Source all view definitions
.read /config/views/silver_indoor_air.sql
.read /config/views/silver_outdoor_weather.sql
.read /config/views/silver_outdoor_air.sql
.read /config/views/cross_stream_aligned.sql

-- Verify views created
SHOW TABLES;

-- Display sample counts
SELECT 'silver_indoor_air' AS view_name, COUNT(*) AS row_count
FROM silver_indoor_air WHERE timestamp >= NOW() - INTERVAL '7 days';

SELECT 'silver_outdoor_weather' AS view_name, COUNT(*) AS row_count
FROM silver_outdoor_weather WHERE timestamp >= NOW() - INTERVAL '7 days';

SELECT 'silver_outdoor_air' AS view_name, COUNT(*) AS row_count
FROM silver_outdoor_air WHERE timestamp >= NOW() - INTERVAL '7 days';

.echo off
```

---

## 2. Bronze Layer Access (Raw Parquet)

### 2.1 Parquet File Patterns

**Current Storage Structure** (from `parquet.rs` analysis):

```
/data/
├── {stream_id}/
│   └── year={YYYY}/
│       └── month={MM}/
│           └── day={DD}/
│               └── readings.parquet
```

**Example Paths**:
```
/data/air-quality/year=2025/month=12/day=17/readings.parquet
/data/outdoor-weather/year=2025/month=12/day=17/readings.parquet
/data/outdoor-air-quality/year=2025/month=12/day=17/readings.parquet
```

### 2.2 Glob Patterns for Each Stream

```sql
-- Air Quality (Indoor)
SELECT * FROM read_parquet('/data/air-quality/year=*/month=*/day=*/readings.parquet',
                           hive_partitioning=true);

-- Outdoor Weather
SELECT * FROM read_parquet('/data/outdoor-weather/year=*/month=*/day=*/readings.parquet',
                           hive_partitioning=true);

-- Outdoor Air Quality
SELECT * FROM read_parquet('/data/outdoor-air-quality/year=*/month=*/day=*/readings.parquet',
                           hive_partitioning=true);
```

**Options**:
- `hive_partitioning=true` - Automatically parse year/month/day as columns
- `union_by_name=true` - Handle schema evolution across files
- `filename=true` - Include source filename as column (for debugging)

### 2.3 Timestamp Field Handling

**Current Schema** (from `parquet.rs` lines 86-107):

```rust
timestamp: i64  // Stored as microseconds since epoch
location_id: String
value: f64
// tags: not stored in current implementation
```

**DuckDB Conversion**:

```sql
-- Convert microsecond timestamp to TIMESTAMP type
SELECT
    to_timestamp(timestamp / 1000000) AS timestamp,
    location_id,
    value
FROM read_parquet('/data/air-quality/year=*/month=*/day=*/readings.parquet',
                  hive_partitioning=true);
```

### 2.4 Schema Inference vs Explicit Schema

**Approach**: Use schema inference with validation

**Rationale**:
- Parquet files already contain schema metadata
- DuckDB's inference is reliable for Parquet
- Explicit schema adds maintenance overhead
- Validation queries will catch schema drift

**Validation Query**:

```sql
-- Verify schema consistency across all files
SELECT DISTINCT
    file,
    column_name,
    data_type
FROM (
    SELECT
        current_setting('filename') AS file,
        unnest(list_value(columns)) AS column_name,
        unnest(list_value(types)) AS data_type
    FROM read_parquet('/data/air-quality/year=*/month=*/day=*/readings.parquet',
                      filename=true)
);
```

---

## 3. Virtual Silver Views (Data Quality)

### 3.1 Silver Indoor Air View

**File**: `config/duckdb/views/silver_indoor_air.sql`

```sql
-- Silver View: Indoor Air Quality
-- Source: air-quality stream (MQTT AirGradient sensor)
-- Applies: Null handling, range filtering, unit normalization

CREATE OR REPLACE VIEW silver_indoor_air AS
SELECT
    to_timestamp(timestamp / 1000000) AS timestamp,
    location_id,

    -- PM2.5: 0-500 μg/m³ (EPA AQI max ~500)
    CASE
        WHEN value >= 0 AND value <= 500
        THEN ROUND(value, 2)
        ELSE NULL
    END AS pm25,

    -- PM10: 0-600 μg/m³
    CASE
        WHEN value >= 0 AND value <= 600
        THEN ROUND(value, 2)
        ELSE NULL
    END AS pm10,

    -- CO2: 400-5000 ppm (indoor range)
    CASE
        WHEN value >= 400 AND value <= 5000
        THEN CAST(value AS INTEGER)
        ELSE NULL
    END AS co2,

    -- Temperature: -10 to 50°C (indoor sensor range)
    CASE
        WHEN value >= -10 AND value <= 50
        THEN ROUND(value, 1)
        ELSE NULL
    END AS temperature,

    -- Humidity: 0-100%
    CASE
        WHEN value >= 0 AND value <= 100
        THEN ROUND(value, 1)
        ELSE NULL
    END AS humidity,

    -- TVOC: 0-60000 ppb
    CASE
        WHEN value >= 0 AND value <= 60000
        THEN CAST(value AS INTEGER)
        ELSE NULL
    END AS tvoc,

    -- NOx: 0-1000 ppb
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN CAST(value AS INTEGER)
        ELSE NULL
    END AS nox,

    -- Metadata
    year,
    month,
    day
FROM read_parquet('/data/air-quality/year=*/month=*/day=*/readings.parquet',
                  hive_partitioning=true)
WHERE
    -- Filter out NULL timestamps
    timestamp IS NOT NULL
    -- Filter out future timestamps (within 5 min tolerance)
    AND to_timestamp(timestamp / 1000000) <= CURRENT_TIMESTAMP + INTERVAL '5 minutes'
ORDER BY timestamp DESC;
```

**Data Quality Rules**:
1. **Null Handling**: Invalid values set to NULL
2. **Range Filtering**: Values outside physical sensor ranges rejected
3. **Precision Normalization**: Consistent decimal places (2 for floats, integers for counts)
4. **Timestamp Validation**: No future timestamps beyond 5-minute tolerance
5. **Type Casting**: Enforce correct types (INT for CO2/TVOC, FLOAT for PM)

### 3.2 Silver Outdoor Weather View

**File**: `config/duckdb/views/silver_outdoor_weather.sql`

```sql
-- Silver View: Outdoor Weather
-- Source: outdoor-weather stream (OpenWeatherMap API)
-- Applies: Null handling, reasonable ranges, unit consistency

CREATE OR REPLACE VIEW silver_outdoor_weather AS
SELECT
    to_timestamp(timestamp / 1000000) AS timestamp,
    location_id,

    -- Temperature: -50 to 60°C (from config)
    CASE
        WHEN value >= -50 AND value <= 60
        THEN ROUND(value, 1)
        ELSE NULL
    END AS temperature,

    -- Feels Like: -50 to 60°C
    CASE
        WHEN value >= -50 AND value <= 60
        THEN ROUND(value, 1)
        ELSE NULL
    END AS feels_like,

    -- Pressure: 800-1200 hPa (from config)
    CASE
        WHEN value >= 800 AND value <= 1200
        THEN ROUND(value, 1)
        ELSE NULL
    END AS pressure,

    -- Humidity: 0-100% (from config)
    CASE
        WHEN value >= 0 AND value <= 100
        THEN ROUND(value, 1)
        ELSE NULL
    END AS humidity,

    -- Wind Speed: 0-100 m/s (from config)
    CASE
        WHEN value >= 0 AND value <= 100
        THEN ROUND(value, 1)
        ELSE NULL
    END AS wind_speed,

    -- Wind Degree: 0-360 degrees (from config)
    CASE
        WHEN value >= 0 AND value <= 360
        THEN ROUND(value, 0)
        ELSE NULL
    END AS wind_deg,

    -- Wind Gust: 0-150 m/s (from config)
    CASE
        WHEN value >= 0 AND value <= 150
        THEN ROUND(value, 1)
        ELSE NULL
    END AS wind_gust,

    -- Clouds: 0-100% (from config)
    CASE
        WHEN value >= 0 AND value <= 100
        THEN ROUND(value, 0)
        ELSE NULL
    END AS clouds,

    -- Visibility: 0-50000 meters (from config)
    CASE
        WHEN value >= 0 AND value <= 50000
        THEN ROUND(value, 0)
        ELSE NULL
    END AS visibility,

    -- Rain 1h: 0-500 mm (from config)
    CASE
        WHEN value >= 0 AND value <= 500
        THEN ROUND(value, 2)
        ELSE NULL
    END AS rain_1h,

    -- Snow 1h: 0-500 mm (from config)
    CASE
        WHEN value >= 0 AND value <= 500
        THEN ROUND(value, 2)
        ELSE NULL
    END AS snow_1h,

    -- Metadata
    year,
    month,
    day
FROM read_parquet('/data/outdoor-weather/year=*/month=*/day=*/readings.parquet',
                  hive_partitioning=true)
WHERE
    timestamp IS NOT NULL
    AND to_timestamp(timestamp / 1000000) <= CURRENT_TIMESTAMP + INTERVAL '5 minutes'
ORDER BY timestamp DESC;
```

**Data Quality Rules**:
1. **Range Validation**: All ranges from stream config (`outdoor-weather/config.yaml`)
2. **Null Handling**: Out-of-range values become NULL
3. **Precision**: 1 decimal for temperatures/speeds, 0 for degrees/percentages
4. **Nullable Fields**: Matches config (rain, snow, wind_gust nullable)

### 3.3 Silver Outdoor Air Quality View

**File**: `config/duckdb/views/silver_outdoor_air.sql`

```sql
-- Silver View: Outdoor Air Quality
-- Source: outdoor-air-quality stream (OpenWeatherMap Air Pollution API)
-- Applies: Null handling, AQI validation, pollutant ranges

CREATE OR REPLACE VIEW silver_outdoor_air AS
SELECT
    to_timestamp(timestamp / 1000000) AS timestamp,
    location_id,

    -- AQI: 1-5 scale (from config, non-nullable)
    CASE
        WHEN value >= 1 AND value <= 5
        THEN CAST(value AS INTEGER)
        ELSE NULL
    END AS aqi,

    -- CO: 0-50000 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 50000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS co,

    -- NO: 0-1000 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS no,

    -- NO2: 0-1000 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS no2,

    -- O3: 0-1000 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS o3,

    -- SO2: 0-1000 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS so2,

    -- PM2.5: 0-1000 μg/m³ (from config, non-nullable)
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS pm2_5,

    -- PM10: 0-1000 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 1000
        THEN ROUND(value, 2)
        ELSE NULL
    END AS pm10,

    -- NH3: 0-200 μg/m³ (from config)
    CASE
        WHEN value >= 0 AND value <= 200
        THEN ROUND(value, 2)
        ELSE NULL
    END AS nh3,

    -- Metadata
    year,
    month,
    day
FROM read_parquet('/data/outdoor-air-quality/year=*/month=*/day=*/readings.parquet',
                  hive_partitioning=true)
WHERE
    timestamp IS NOT NULL
    AND to_timestamp(timestamp / 1000000) <= CURRENT_TIMESTAMP + INTERVAL '5 minutes'
    -- AQI is non-nullable per config
    AND value IS NOT NULL
ORDER BY timestamp DESC;
```

**Data Quality Rules**:
1. **AQI Validation**: Integer 1-5 scale, non-nullable
2. **Range Validation**: All pollutant ranges from config
3. **Precision**: 2 decimal places for all measurements
4. **Required Fields**: AQI and PM2.5 are non-nullable per config

---

## 4. Cross-Stream View

### 4.1 Cross-Stream Aligned View

**File**: `config/duckdb/views/cross_stream_aligned.sql`

**Challenge**: Different poll rates
- Indoor (MQTT): Every ~1 minute
- Outdoor Weather: Every 10 minutes (600 seconds)
- Outdoor Air: Every 10 minutes (600 seconds)

**Approach**: Time bucketing with ASOF JOIN

```sql
-- Cross-Stream Aligned View
-- Joins all three streams using time buckets for correlation analysis
-- Resolution: 10-minute buckets to match outdoor poll rate

CREATE OR REPLACE VIEW cross_stream_aligned AS
WITH bucketed_indoor AS (
    SELECT
        time_bucket(INTERVAL '10 minutes', timestamp) AS bucket_time,
        AVG(pm25) AS avg_pm25_indoor,
        AVG(co2) AS avg_co2,
        AVG(temperature) AS avg_temp_indoor,
        AVG(humidity) AS avg_humidity_indoor,
        COUNT(*) AS indoor_sample_count
    FROM silver_indoor_air
    WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '30 days'
    GROUP BY bucket_time
),
bucketed_weather AS (
    SELECT
        time_bucket(INTERVAL '10 minutes', timestamp) AS bucket_time,
        AVG(temperature) AS avg_temp_outdoor,
        AVG(humidity) AS avg_humidity_outdoor,
        AVG(pressure) AS avg_pressure,
        AVG(wind_speed) AS avg_wind_speed,
        COUNT(*) AS weather_sample_count
    FROM silver_outdoor_weather
    WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '30 days'
    GROUP BY bucket_time
),
bucketed_air AS (
    SELECT
        time_bucket(INTERVAL '10 minutes', timestamp) AS bucket_time,
        AVG(aqi) AS avg_aqi,
        AVG(pm2_5) AS avg_pm25_outdoor,
        AVG(pm10) AS avg_pm10_outdoor,
        AVG(no2) AS avg_no2,
        AVG(o3) AS avg_o3,
        COUNT(*) AS air_sample_count
    FROM silver_outdoor_air
    WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '30 days'
    GROUP BY bucket_time
)
SELECT
    i.bucket_time AS timestamp,

    -- Indoor metrics
    ROUND(i.avg_pm25_indoor, 2) AS pm25_indoor,
    ROUND(i.avg_co2, 0) AS co2_indoor,
    ROUND(i.avg_temp_indoor, 1) AS temperature_indoor,
    ROUND(i.avg_humidity_indoor, 1) AS humidity_indoor,

    -- Outdoor weather metrics
    ROUND(w.avg_temp_outdoor, 1) AS temperature_outdoor,
    ROUND(w.avg_humidity_outdoor, 1) AS humidity_outdoor,
    ROUND(w.avg_pressure, 1) AS pressure,
    ROUND(w.avg_wind_speed, 1) AS wind_speed,

    -- Outdoor air quality metrics
    ROUND(a.avg_aqi, 0) AS aqi_outdoor,
    ROUND(a.avg_pm25_outdoor, 2) AS pm25_outdoor,
    ROUND(a.avg_pm10_outdoor, 2) AS pm10_outdoor,
    ROUND(a.avg_no2, 2) AS no2,
    ROUND(a.avg_o3, 2) AS o3,

    -- Derived metrics for correlation
    ROUND(i.avg_temp_indoor - w.avg_temp_outdoor, 1) AS temp_delta_indoor_outdoor,
    ROUND(i.avg_humidity_indoor - w.avg_humidity_outdoor, 1) AS humidity_delta_indoor_outdoor,
    ROUND(i.avg_pm25_indoor - a.avg_pm25_outdoor, 2) AS pm25_delta_indoor_outdoor,

    -- Sample counts for data quality
    i.indoor_sample_count,
    w.weather_sample_count,
    a.air_sample_count
FROM bucketed_indoor i
LEFT JOIN bucketed_weather w ON i.bucket_time = w.bucket_time
LEFT JOIN bucketed_air a ON i.bucket_time = a.bucket_time
WHERE
    -- Require at least indoor data
    i.indoor_sample_count > 0
ORDER BY timestamp DESC;
```

### 4.2 Alternative: ASOF JOIN Approach

**For real-time use cases** (if sub-10-minute resolution needed):

```sql
-- ASOF JOIN: Match each indoor reading to nearest outdoor readings
CREATE OR REPLACE VIEW cross_stream_asof AS
SELECT
    i.timestamp,
    i.pm25 AS pm25_indoor,
    i.temperature AS temperature_indoor,
    w.temperature AS temperature_outdoor,
    w.humidity AS humidity_outdoor,
    a.aqi AS aqi_outdoor,
    a.pm2_5 AS pm25_outdoor,

    -- Time difference indicators
    EXTRACT(EPOCH FROM (i.timestamp - w.timestamp)) AS weather_age_seconds,
    EXTRACT(EPOCH FROM (i.timestamp - a.timestamp)) AS air_age_seconds
FROM silver_indoor_air i
ASOF LEFT JOIN silver_outdoor_weather w
    ON i.timestamp >= w.timestamp
ASOF LEFT JOIN silver_outdoor_air a
    ON i.timestamp >= a.timestamp
WHERE
    i.timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days'
    -- Only join if outdoor data is within 15 minutes
    AND EXTRACT(EPOCH FROM (i.timestamp - w.timestamp)) <= 900
    AND EXTRACT(EPOCH FROM (i.timestamp - a.timestamp)) <= 900
ORDER BY i.timestamp DESC;
```

### 4.3 Resolution Handling

**Decision Matrix**:

| Use Case | View | Resolution | Join Type |
|----------|------|------------|-----------|
| Dashboard correlation | `cross_stream_aligned` | 10-minute buckets | LEFT JOIN on bucket |
| Real-time monitoring | `cross_stream_asof` | 1-minute (indoor rate) | ASOF JOIN |
| Hourly aggregates | New view | 1-hour buckets | LEFT JOIN on bucket |

**Recommendation**: Start with `cross_stream_aligned` (10-minute buckets) for V1.

---

## 5. Acceptance Criteria

### AC-001: Silver Indoor Air Returns Valid Data

**Test Query**:

```sql
SELECT
    COUNT(*) AS total_rows,
    COUNT(pm25) AS pm25_non_null,
    MIN(timestamp) AS earliest,
    MAX(timestamp) AS latest,
    AVG(pm25) AS avg_pm25
FROM silver_indoor_air
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days';
```

**Expected**:
- `total_rows > 0`
- `pm25_non_null / total_rows > 0.95` (95% data quality)
- `earliest` and `latest` within expected range
- `avg_pm25` between 0 and 100 (reasonable indoor PM2.5)

**Pass Criteria**: Query executes in <2 seconds, returns expected row count.

### AC-002: NULL Values Filtered or Handled Appropriately

**Test Query**:

```sql
-- Verify out-of-range values are NULL
SELECT
    COUNT(*) AS total_rows,
    COUNT(*) FILTER (WHERE pm25 < 0) AS negative_pm25,
    COUNT(*) FILTER (WHERE pm25 > 500) AS excessive_pm25,
    COUNT(*) FILTER (WHERE co2 < 400) AS low_co2,
    COUNT(*) FILTER (WHERE temperature < -10 OR temperature > 50) AS invalid_temp
FROM silver_indoor_air
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days';
```

**Expected**:
- `negative_pm25 = 0`
- `excessive_pm25 = 0`
- `low_co2 = 0`
- `invalid_temp = 0`

**Pass Criteria**: All invalid value counts are zero.

### AC-003: Out-of-Range Values Excluded or Flagged

**Test Query**:

```sql
-- Test weather ranges
SELECT
    COUNT(*) FILTER (WHERE temperature < -50 OR temperature > 60) AS invalid_temp,
    COUNT(*) FILTER (WHERE pressure < 800 OR pressure > 1200) AS invalid_pressure,
    COUNT(*) FILTER (WHERE humidity < 0 OR humidity > 100) AS invalid_humidity
FROM silver_outdoor_weather
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days';

-- Test air quality ranges
SELECT
    COUNT(*) FILTER (WHERE aqi < 1 OR aqi > 5) AS invalid_aqi,
    COUNT(*) FILTER (WHERE pm2_5 < 0 OR pm2_5 > 1000) AS invalid_pm25
FROM silver_outdoor_air
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days';
```

**Expected**: All counts should be zero.

**Pass Criteria**: Range validation working correctly.

### AC-004: Cross-Stream View Aligns Timestamps Within Tolerance

**Test Query**:

```sql
SELECT
    timestamp,
    indoor_sample_count,
    weather_sample_count,
    air_sample_count,
    -- Check if outdoor data exists
    CASE
        WHEN weather_sample_count > 0 THEN 'Weather data present'
        ELSE 'Weather data missing'
    END AS weather_status,
    CASE
        WHEN air_sample_count > 0 THEN 'Air data present'
        ELSE 'Air data missing'
    END AS air_status
FROM cross_stream_aligned
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days'
ORDER BY timestamp DESC
LIMIT 100;
```

**Expected**:
- At least 50% of buckets have all three streams present
- No gaps larger than 2 hours in indoor data
- Outdoor data present at 10-minute intervals

**Pass Criteria**: JOIN produces aligned data with reasonable coverage.

### AC-005: 7-Day Queries Complete in <5 Seconds

**Test Queries**:

```sql
-- Benchmark: Indoor air 7-day scan
EXPLAIN ANALYZE
SELECT timestamp, pm25, co2, temperature
FROM silver_indoor_air
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days'
ORDER BY timestamp DESC;

-- Benchmark: Cross-stream 7-day scan
EXPLAIN ANALYZE
SELECT *
FROM cross_stream_aligned
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '7 days'
ORDER BY timestamp DESC;
```

**Expected Execution Time**:
- Single stream: <2 seconds
- Cross-stream: <5 seconds

**Pass Criteria**: All 7-day queries complete within time budget on Raspberry Pi 5.

### AC-006: 30-Day Queries Complete in <15 Seconds

**Test Queries**:

```sql
-- Benchmark: 30-day aggregation
EXPLAIN ANALYZE
SELECT
    DATE_TRUNC('day', timestamp) AS day,
    AVG(pm25) AS avg_pm25,
    MAX(pm25) AS max_pm25,
    MIN(pm25) AS min_pm25
FROM silver_indoor_air
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '30 days'
GROUP BY day
ORDER BY day DESC;

-- Benchmark: Cross-stream 30-day
EXPLAIN ANALYZE
SELECT *
FROM cross_stream_aligned
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '30 days'
ORDER BY timestamp DESC;
```

**Expected Execution Time**:
- Single stream aggregation: <5 seconds
- Cross-stream scan: <15 seconds

**Pass Criteria**: 30-day queries are usable for Grafana dashboards without timeout.

---

## 6. Performance Optimization Strategies

### 6.1 Partition Pruning

**Current**: Hive-style partitioning by year/month/day enables automatic pruning.

**Optimization**: Ensure queries include partition filters:

```sql
-- GOOD: Partition filter applied
SELECT * FROM silver_indoor_air
WHERE timestamp >= '2025-12-01' AND timestamp < '2025-12-18';
-- DuckDB will only scan day=01 through day=17

-- BAD: No partition filter
SELECT * FROM silver_indoor_air
WHERE pm25 > 50;
-- Scans all files
```

### 6.2 View Materialization (Future)

**V1**: All views are virtual (query-time computation).

**Future Optimization** (if performance insufficient):

```sql
-- Create materialized view (requires persistent DuckDB database)
CREATE MATERIALIZED VIEW mat_silver_indoor_air AS
SELECT * FROM silver_indoor_air;

-- Refresh hourly via cron
REFRESH MATERIALIZED VIEW mat_silver_indoor_air;
```

### 6.3 Index Strategies (Future)

**V1**: No indexes (Parquet columnar format is efficient).

**Future**: Add indexes on timestamp if needed:

```sql
CREATE INDEX idx_indoor_timestamp ON silver_indoor_air(timestamp);
```

### 6.4 Query Result Caching

**DuckDB Configuration**:

```sql
-- In init.sql
SET enable_result_cache = true;
SET result_cache_size = '256MB';
```

**Rationale**: Grafana dashboards may issue same queries multiple times.

---

## 7. Monitoring and Validation

### 7.1 Health Check Queries

**File**: `config/duckdb/health_check.sql`

```sql
-- DuckDB Health Check
-- Run periodically to validate views

.echo on

-- Check 1: All views exist
SELECT table_name, table_type
FROM information_schema.tables
WHERE table_schema = 'main'
AND table_type = 'VIEW';

-- Check 2: Recent data availability
SELECT
    'silver_indoor_air' AS view_name,
    COUNT(*) AS row_count,
    MAX(timestamp) AS latest_timestamp,
    CURRENT_TIMESTAMP - MAX(timestamp) AS data_age
FROM silver_indoor_air;

SELECT
    'silver_outdoor_weather' AS view_name,
    COUNT(*) AS row_count,
    MAX(timestamp) AS latest_timestamp,
    CURRENT_TIMESTAMP - MAX(timestamp) AS data_age
FROM silver_outdoor_weather;

SELECT
    'silver_outdoor_air' AS view_name,
    COUNT(*) AS row_count,
    MAX(timestamp) AS latest_timestamp,
    CURRENT_TIMESTAMP - MAX(timestamp) AS data_age
FROM silver_outdoor_air;

-- Check 3: Data quality metrics
SELECT
    'Data Quality - Indoor' AS check_name,
    COUNT(*) AS total_rows,
    COUNT(pm25) AS pm25_non_null,
    ROUND(COUNT(pm25)::FLOAT / COUNT(*) * 100, 2) AS pm25_coverage_pct
FROM silver_indoor_air
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '24 hours';

-- Check 4: Cross-stream alignment
SELECT
    COUNT(*) AS aligned_buckets,
    SUM(CASE WHEN weather_sample_count > 0 THEN 1 ELSE 0 END) AS buckets_with_weather,
    SUM(CASE WHEN air_sample_count > 0 THEN 1 ELSE 0 END) AS buckets_with_air,
    ROUND(SUM(CASE WHEN weather_sample_count > 0 THEN 1 ELSE 0 END)::FLOAT / COUNT(*) * 100, 2) AS weather_coverage_pct
FROM cross_stream_aligned
WHERE timestamp >= CURRENT_TIMESTAMP - INTERVAL '24 hours';

.echo off
```

### 7.2 Automated Validation

**Cron Job** (run daily):

```bash
#!/bin/bash
# /deploy/pi/scripts/duckdb_health_check.sh

docker exec ndp-duckdb duckdb /workspace/ndp.db < /config/health_check.sql > /tmp/duckdb_health.log 2>&1

# Check for errors
if grep -q "Error" /tmp/duckdb_health.log; then
    echo "DuckDB health check failed!"
    cat /tmp/duckdb_health.log
    exit 1
else
    echo "DuckDB health check passed"
    exit 0
fi
```

---

## 8. Grafana Integration Points

### 8.1 DuckDB Datasource Configuration

**File**: `config/grafana/provisioning/datasources/duckdb.yaml`

```yaml
apiVersion: 1

datasources:
  - name: DuckDB
    type: motherduck-duckdb-datasource
    access: proxy
    url: http://duckdb:8080
    database: /workspace/ndp.db
    jsonData:
      defaultDatabase: main
      readOnly: true
    isDefault: true
    editable: false
```

**Alternative** (if using DuckDB HTTP API):

```yaml
datasources:
  - name: DuckDB
    type: postgres  # DuckDB supports PostgreSQL wire protocol
    access: proxy
    url: duckdb:5432
    database: ndp
    user: duckdb
    secureJsonData:
      password: ""
    jsonData:
      sslmode: disable
```

### 8.2 Example Dashboard Query

**Panel**: Indoor PM2.5 Over Time

```sql
SELECT
    timestamp AS time,
    pm25 AS "PM2.5 (μg/m³)"
FROM silver_indoor_air
WHERE
    $__timeFilter(timestamp)
ORDER BY timestamp ASC;
```

**Grafana Variables**:
- `$__timeFilter(timestamp)` - Auto-injected time range filter
- `$__interval` - Auto-calculated bucket size for aggregation

---

## 9. Open Issues and Decisions

### 9.1 DuckDB Persistence Strategy

**Options**:

| Option | Pros | Cons | Recommendation |
|--------|------|------|----------------|
| Ephemeral (views only) | Simpler, stateless | Views recreated on restart | OK for V1 |
| Persistent database | Faster startup, can cache results | Requires volume management | Consider for V2 |
| Hybrid | Views + metadata DB | Best of both | Overengineered for V1 |

**Decision**: Start ephemeral for V1, add persistence in V2 if needed.

### 9.2 DuckDB HTTP API

**Question**: Should DuckDB expose HTTP API for ad-hoc queries?

**Options**:
- Use DuckDB CLI only (Grafana connects via plugin)
- Add DuckDB HTTP server (e.g., `duckdb-wasm` or custom wrapper)

**Decision**: V1 uses CLI only, accessed via Grafana plugin. HTTP API deferred to V2.

### 9.3 View Refresh Strategy

**Question**: How to handle Parquet file updates?

**Answer**: Views are virtual, automatically reflect new data on each query. No refresh needed.

---

## 10. Related Documentation

- [SCOPE.md](../SCOPE.md) - Overall feature scope
- [Parquet Storage Implementation](../../../../core/src/storage/parquet.rs)
- [Stream Configurations](../../../../config/base/streams/)
- [DuckDB Parquet Documentation](https://duckdb.org/docs/data/parquet/overview)
- [Hive Partitioning](https://duckdb.org/docs/data/partitioning/hive_partitioning)

---

## 11. Implementation Checklist

- [ ] Create `config/duckdb/` directory structure
- [ ] Write `init.sql` bootstrap script
- [ ] Write `silver_indoor_air.sql` view definition
- [ ] Write `silver_outdoor_weather.sql` view definition
- [ ] Write `silver_outdoor_air.sql` view definition
- [ ] Write `cross_stream_aligned.sql` view definition
- [ ] Write `health_check.sql` validation script
- [ ] Update `docker-compose.yml` with DuckDB service
- [ ] Test view queries against actual Parquet files
- [ ] Validate acceptance criteria (AC-001 through AC-006)
- [ ] Document in `architecture/` directory
- [ ] Update `STATUS.md` with completion

---

**End of Specification**
