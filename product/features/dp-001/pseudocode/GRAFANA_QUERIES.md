# Grafana Dashboard Query Patterns

**Feature**: DP-001 - DuckDB Analytics Layer + Grafana Dashboards
**Component**: Grafana Query Specifications
**Author**: NDP Grafana Developer
**Date**: 2025-12-18
**Status**: Pseudocode

---

## 1. Query Pattern Fundamentals

### 1.1 DuckDB Connection Context

**Datasource**: `motherduck-duckdb-datasource` plugin
**Connection**: HTTP proxy to DuckDB container at `http://duckdb:8080`
**Database**: `ndp` (default)
**Read-Only**: Yes (Bronze layer protection)

### 1.2 Grafana Time Variables

Grafana provides these variables for time-based filtering:

| Variable | Type | Description | Example |
|----------|------|-------------|---------|
| `$__timeFrom()` | Function | Start of time range | `2025-12-11 00:00:00` |
| `$__timeTo()` | Function | End of time range | `2025-12-18 23:59:59` |
| `$__timeFilter(column)` | Macro | WHERE clause for time range | `timestamp BETWEEN ...` |
| `$__interval` | String | Auto-calculated interval | `1h`, `5m`, `1d` |

**Usage Pattern**:
```sql
WHERE timestamp >= $__timeFrom() AND timestamp <= $__timeTo()
-- OR
WHERE $__timeFilter(timestamp)
```

### 1.3 Available DuckDB Views

From `DUCKDB_SPECIFICATION.md`:

| View | Source | Purpose | Aggregation Level |
|------|--------|---------|------------------|
| `silver_indoor_air` | `air-quality` Bronze | Cleaned indoor air quality data | Raw (sub-minute) |
| `silver_outdoor_weather` | `outdoor-weather` Bronze | Cleaned outdoor weather data | Raw (~5 min) |
| `silver_outdoor_air` | `outdoor-air-quality` Bronze | Cleaned outdoor air quality data | Raw (~5 min) |
| `readings_hourly` | All streams | Hourly aggregates (AVG/MIN/MAX) | Hourly rollups |
| `readings_daily` | All streams | Daily aggregates (AVG/MIN/MAX) | Daily rollups |

**Performance Guidelines**:
- Use `readings_hourly` for time ranges > 24 hours
- Use `readings_daily` for time ranges > 7 days
- Use raw views (`silver_*`) only for real-time/recent queries

---

## 2. Core Query Templates

### 2.1 Time Series Query (Hourly Aggregates)

**Use Case**: Historical trends over 7-30 days

```sql
SELECT
    bucket AS time,
    avg_<field> AS "<Field Display Name>"
FROM readings_hourly
WHERE
    stream_id = '<stream-id>'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Example (PM2.5 Trend)**:
```sql
SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

### 2.2 Time Series Query (Raw Data)

**Use Case**: Real-time monitoring (last 1-6 hours)

```sql
SELECT
    timestamp AS time,
    pm25 AS "PM2.5 (µg/m³)"
FROM silver_indoor_air
WHERE
    timestamp >= $__timeFrom()
    AND timestamp <= $__timeTo()
ORDER BY timestamp
```

### 2.3 Latest Value Query (Stat Panel)

**Use Case**: Current readings display

```sql
SELECT
    avg_<field> AS "<field>",
    bucket AS time
FROM readings_hourly
WHERE stream_id = '<stream-id>'
ORDER BY bucket DESC
LIMIT 1
```

**Example (Current PM2.5)**:
```sql
SELECT
    avg_pm25 AS "pm25",
    bucket AS time
FROM readings_hourly
WHERE stream_id = 'air-quality'
ORDER BY bucket DESC
LIMIT 1
```

### 2.4 Min/Max Band Query (Temperature Range)

**Use Case**: Show temperature with uncertainty band

```sql
SELECT
    bucket AS time,
    avg_temperature AS "Temperature",
    max_temperature AS "Max",
    min_temperature AS "Min"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Grafana Config**:
- Field overrides: Set `Max` and `Min` as "Fill below to" series
- Result: Shaded band showing temperature range

### 2.5 Multi-Series Comparison (UNION ALL)

**Use Case**: Indoor vs Outdoor comparison on same chart

```sql
-- Indoor series
SELECT
    bucket AS time,
    avg_pm25 AS "Indoor PM2.5"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

UNION ALL

-- Outdoor series
SELECT
    bucket AS time,
    avg_pm2_5 AS "Outdoor PM2.5"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

ORDER BY time
```

### 2.6 Gauge Query (AQI Scale)

**Use Case**: AQI gauge (0-500 scale)

```sql
SELECT
    avg_us_aqi AS "AQI",
    bucket AS time
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Grafana Config**:
- Min: 0, Max: 500
- Thresholds: Green (0-50), Yellow (51-100), Orange (101-150), Red (151-200), Purple (201-300), Maroon (301+)

### 2.7 Bar Chart Query (Pollutant Breakdown)

**Use Case**: Latest values for multiple pollutants

```sql
SELECT
    'PM2.5' AS pollutant, avg_pm2_5 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'PM10' AS pollutant, avg_pm10 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'NO2' AS pollutant, avg_no2 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'O3' AS pollutant, avg_o3 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'CO' AS pollutant, avg_co AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'SO2' AS pollutant, avg_so2 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Grafana Config**:
- Visualization: Bar chart (horizontal)
- Field: `pollutant` (X-axis), `value` (Y-axis)

---

## 3. Dashboard 1: Indoor Air Quality Monitoring

**UID**: `ndp-indoor-air-quality`
**Refresh**: 5 minutes
**Default Range**: Last 7 days

### Panel 1.1: Current PM2.5 (Stat)

**Type**: Stat
**Position**: Top row, left

```sql
SELECT
    avg_pm25 AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Thresholds**:
- Green: 0-12 µg/m³ (Good)
- Yellow: 12-35 µg/m³ (Moderate)
- Orange: 35-55 µg/m³ (Unhealthy for Sensitive Groups)
- Red: >55 µg/m³ (Unhealthy)

**Display**:
- Unit: `µg/m³`
- Decimals: 1
- Sparkline: Show (7-day trend)
- Color mode: Background

### Panel 1.2: Current CO2 (Stat)

**Type**: Stat
**Position**: Top row, middle

```sql
SELECT
    avg_co2 AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Thresholds**:
- Green: <1000 ppm (Normal)
- Yellow: 1000-2000 ppm (Elevated)
- Red: >2000 ppm (High)

**Display**:
- Unit: `ppm`
- Decimals: 0
- Sparkline: Show
- Color mode: Background

### Panel 1.3: Current Temperature (Stat)

**Type**: Stat
**Position**: Top row, right

```sql
SELECT
    avg_temperature AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Thresholds**: None (informational)

**Display**:
- Unit: `°C`
- Decimals: 1
- Sparkline: Show
- Color mode: None

### Panel 1.4: PM2.5 Trend (Time Series)

**Type**: Time series
**Position**: Row 2, full width

```sql
SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds** (color regions on Y-axis):
- 0-12: Green
- 12-35: Yellow
- 35-55: Orange
- 55+: Red

**Display**:
- Line style: Smooth
- Fill opacity: 20%
- Point size: Auto
- Y-axis: 0-100 µg/m³

### Panel 1.5: CO2 Levels (Time Series)

**Type**: Time series
**Position**: Row 3, full width

```sql
SELECT
    bucket AS time,
    avg_co2 AS "CO2 (ppm)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**:
- <1000: Green
- 1000-2000: Yellow
- >2000: Red

**Display**:
- Line style: Smooth
- Fill opacity: 10%
- Y-axis: 400-3000 ppm (log scale optional)

### Panel 1.6: Temperature & Humidity (Time Series)

**Type**: Time series (dual Y-axis)
**Position**: Row 4, full width

```sql
SELECT
    bucket AS time,
    avg_temperature AS "Temperature (°C)",
    avg_humidity AS "Humidity (%)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Display**:
- Temperature: Left Y-axis (10-30°C)
- Humidity: Right Y-axis (0-100%)
- Line styles: Solid for temp, dashed for humidity

### Panel 1.7: VOC Index (Time Series)

**Type**: Time series
**Position**: Row 5, full width

```sql
SELECT
    bucket AS time,
    avg_tvoc AS "VOC Index"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**:
- <100: Green (Good)
- 100-300: Yellow (Moderate)
- >300: Red (Poor)

**Display**:
- Unit: `ppb`
- Line style: Smooth

---

## 4. Dashboard 2: Outdoor Weather Conditions

**UID**: `ndp-outdoor-conditions`
**Refresh**: 5 minutes
**Default Range**: Last 7 days

### Panel 2.1: Current Temperature (Stat)

**Type**: Stat
**Position**: Top row, left

```sql
SELECT
    avg_temperature AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'outdoor-conditions'
ORDER BY bucket DESC
LIMIT 1
```

**Display**:
- Unit: `°C`
- Decimals: 1
- Sparkline: Show

### Panel 2.2: Current Wind Speed (Stat)

**Type**: Stat
**Position**: Top row, middle

```sql
SELECT
    avg_wind_speed AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'outdoor-conditions'
ORDER BY bucket DESC
LIMIT 1
```

**Thresholds**:
- <5: Green (Calm)
- 5-10: Yellow (Moderate)
- 10-15: Orange (Strong)
- >15: Red (Very Strong)

**Display**:
- Unit: `m/s`
- Decimals: 1
- Sparkline: Show

### Panel 2.3: Current Pressure (Stat)

**Type**: Stat
**Position**: Top row, right

```sql
SELECT
    avg_pressure AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'outdoor-conditions'
ORDER BY bucket DESC
LIMIT 1
```

**Display**:
- Unit: `hPa`
- Decimals: 0
- Sparkline: Show

### Panel 2.4: Temperature & Feels Like (Time Series)

**Type**: Time series
**Position**: Row 2, full width

```sql
SELECT
    bucket AS time,
    avg_temperature AS "Temperature (°C)",
    avg_apparent_temperature AS "Feels Like (°C)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Display**:
- Temperature: Solid blue line
- Feels Like: Dashed orange line
- Y-axis: Auto-range

### Panel 2.5: Wind Speed & Direction (Time Series)

**Type**: Time series
**Position**: Row 3, full width

```sql
SELECT
    bucket AS time,
    avg_wind_speed AS "Wind Speed (m/s)",
    avg_wind_direction AS "Wind Direction (°)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Display**:
- Wind Speed: Left Y-axis (0-20 m/s)
- Wind Direction: Right Y-axis (0-360°)
- Wind Direction: Represent as compass rose overlay (optional)

### Panel 2.6: Atmospheric Pressure (Time Series)

**Type**: Time series
**Position**: Row 4, left half

```sql
SELECT
    bucket AS time,
    avg_pressure AS "Pressure (hPa)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Display**:
- Y-axis: 980-1050 hPa
- Line style: Smooth

### Panel 2.7: Cloud Cover & Precipitation (Time Series)

**Type**: Time series
**Position**: Row 4, right half

```sql
SELECT
    bucket AS time,
    avg_cloud_cover AS "Cloud Cover (%)",
    avg_precipitation_probability AS "Precipitation Probability (%)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Display**:
- Cloud Cover: Area fill (gray)
- Precipitation: Bar overlay (blue)
- Y-axis: 0-100%

### Panel 2.8: UV Index (Time Series)

**Type**: Time series
**Position**: Row 5, full width

```sql
SELECT
    bucket AS time,
    avg_uv_index AS "UV Index"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**:
- 0-2: Green (Low)
- 3-5: Yellow (Moderate)
- 6-7: Orange (High)
- 8-10: Red (Very High)
- 11+: Purple (Extreme)

**Display**:
- Y-axis: 0-12
- Line style: Smooth

---

## 5. Dashboard 3: Outdoor Air Quality Monitoring

**UID**: `ndp-outdoor-air-quality`
**Refresh**: 5 minutes
**Default Range**: Last 7 days

### Panel 3.1: Current AQI (Gauge)

**Type**: Gauge
**Position**: Top row, left (2 columns wide)

```sql
SELECT
    avg_us_aqi AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Thresholds** (US AQI scale):
- 0-50: Green (Good)
- 51-100: Yellow (Moderate)
- 101-150: Orange (Unhealthy for Sensitive Groups)
- 151-200: Red (Unhealthy)
- 201-300: Purple (Very Unhealthy)
- 301-500: Maroon (Hazardous)

**Display**:
- Min: 0, Max: 500
- Show threshold labels
- Show threshold markers

### Panel 3.2: AQI Trend (Time Series)

**Type**: Time series
**Position**: Top row, right (4 columns wide)

```sql
SELECT
    bucket AS time,
    avg_us_aqi AS "US AQI",
    avg_european_aqi AS "European AQI"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**: Same as gauge (color bands on Y-axis)

**Display**:
- US AQI: Solid line
- European AQI: Dashed line
- Y-axis: 0-200 (auto-expand if higher)

### Panel 3.3: PM2.5 Indoor vs Outdoor Comparison (Time Series)

**Type**: Time series
**Position**: Row 2, full width

```sql
-- Indoor PM2.5
SELECT
    bucket AS time,
    avg_pm25 AS "Indoor PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

UNION ALL

-- Outdoor PM2.5
SELECT
    bucket AS time,
    avg_pm2_5 AS "Outdoor PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

ORDER BY time
```

**Display**:
- Indoor: Blue solid line
- Outdoor: Orange solid line
- Thresholds: PM2.5 EPA standard bands

### Panel 3.4: PM10 Levels (Time Series)

**Type**: Time series
**Position**: Row 3, full width

```sql
SELECT
    bucket AS time,
    avg_pm10 AS "PM10 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**:
- 0-50: Green
- 51-100: Yellow
- >100: Red

### Panel 3.5: Ozone & Nitrogen Dioxide (Time Series)

**Type**: Time series
**Position**: Row 4, full width

```sql
SELECT
    bucket AS time,
    avg_o3 AS "Ozone (µg/m³)",
    avg_no2 AS "Nitrogen Dioxide (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**:
- O3: Green <100, Yellow 100-200, Red >200
- NO2: Green <40, Yellow 40-200, Red >200

**Display**:
- O3: Blue line
- NO2: Red line
- Dual Y-axis if scales differ significantly

### Panel 3.6: Pollutant Breakdown (Bar Chart)

**Type**: Bar chart (horizontal)
**Position**: Row 5, full width

```sql
SELECT
    'PM2.5' AS pollutant, avg_pm2_5 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'PM10' AS pollutant, avg_pm10 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'NO2' AS pollutant, avg_no2 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'O3' AS pollutant, avg_o3 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'CO' AS pollutant, avg_co AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1

UNION ALL

SELECT
    'SO2' AS pollutant, avg_so2 AS value, 'µg/m³' AS unit
FROM readings_hourly
WHERE stream_id = 'outdoor-air-quality'
ORDER BY bucket DESC
LIMIT 1
```

**Display**:
- X-axis: pollutant
- Y-axis: value
- Color: Per-pollutant thresholds

---

## 6. Dashboard 4: Indoor vs Outdoor Comparison

**UID**: `ndp-comparison`
**Refresh**: 5 minutes
**Default Range**: Last 7 days
**Special**: Synchronized time range across all panels

### Panel 4.1: Temperature Comparison (Time Series)

**Type**: Time series
**Position**: Row 1, left half

```sql
SELECT
    bucket AS time,
    avg_temperature AS "Indoor Temperature (°C)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

UNION ALL

SELECT
    bucket AS time,
    avg_temperature AS "Outdoor Temperature (°C)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

ORDER BY time
```

**Display**:
- Indoor: Blue line
- Outdoor: Orange line
- Shared Y-axis

### Panel 4.2: PM2.5 Comparison (Time Series)

**Type**: Time series
**Position**: Row 1, right half

```sql
SELECT
    bucket AS time,
    avg_pm25 AS "Indoor PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

UNION ALL

SELECT
    bucket AS time,
    avg_pm2_5 AS "Outdoor PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

ORDER BY time
```

**Display**:
- Indoor: Blue line
- Outdoor: Orange line
- Thresholds: PM2.5 EPA bands

### Panel 4.3: Humidity Comparison (Time Series)

**Type**: Time series
**Position**: Row 2, left half

```sql
SELECT
    bucket AS time,
    avg_humidity AS "Indoor Humidity (%)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

UNION ALL

SELECT
    bucket AS time,
    avg_humidity AS "Outdoor Humidity (%)"
FROM readings_hourly
WHERE
    stream_id = 'outdoor-conditions'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()

ORDER BY time
```

**Display**:
- Indoor: Blue line
- Outdoor: Orange line
- Y-axis: 0-100%

### Panel 4.4: CO2 Levels (Indoor Only) (Time Series)

**Type**: Time series
**Position**: Row 2, right half

```sql
SELECT
    bucket AS time,
    avg_co2 AS "CO2 (ppm)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Thresholds**:
- <1000: Green
- 1000-2000: Yellow
- >2000: Red

**Note**: Outdoor CO2 is not measured (atmospheric baseline ~420 ppm)

### Panel 4.5: Correlation Matrix (Heatmap)

**Type**: Heatmap
**Position**: Row 3, full width

```sql
-- This is a complex query that calculates correlation coefficients
-- between all indoor and outdoor metrics
WITH combined_data AS (
    SELECT
        bucket,
        avg_temperature AS indoor_temp,
        avg_humidity AS indoor_humidity,
        avg_pm25 AS indoor_pm25,
        avg_co2 AS indoor_co2
    FROM readings_hourly
    WHERE
        stream_id = 'air-quality'
        AND bucket >= $__timeFrom()
        AND bucket <= $__timeTo()
),
outdoor_data AS (
    SELECT
        bucket,
        avg_temperature AS outdoor_temp,
        avg_humidity AS outdoor_humidity
    FROM readings_hourly
    WHERE
        stream_id = 'outdoor-conditions'
        AND bucket >= $__timeFrom()
        AND bucket <= $__timeTo()
),
outdoor_aqi_data AS (
    SELECT
        bucket,
        avg_pm2_5 AS outdoor_pm25,
        avg_us_aqi AS outdoor_aqi
    FROM readings_hourly
    WHERE
        stream_id = 'outdoor-air-quality'
        AND bucket >= $__timeFrom()
        AND bucket <= $__timeTo()
),
joined AS (
    SELECT
        c.bucket,
        c.indoor_temp,
        c.indoor_humidity,
        c.indoor_pm25,
        c.indoor_co2,
        o.outdoor_temp,
        o.outdoor_humidity,
        a.outdoor_pm25,
        a.outdoor_aqi
    FROM combined_data c
    LEFT JOIN outdoor_data o ON c.bucket = o.bucket
    LEFT JOIN outdoor_aqi_data a ON c.bucket = a.bucket
)
SELECT
    'Indoor Temp' AS metric1,
    'Outdoor Temp' AS metric2,
    CORR(indoor_temp, outdoor_temp) AS correlation
FROM joined

UNION ALL

SELECT
    'Indoor PM2.5' AS metric1,
    'Outdoor PM2.5' AS metric2,
    CORR(indoor_pm25, outdoor_pm25) AS correlation
FROM joined

UNION ALL

SELECT
    'Indoor Humidity' AS metric1,
    'Outdoor Humidity' AS metric2,
    CORR(indoor_humidity, outdoor_humidity) AS correlation
FROM joined

UNION ALL

SELECT
    'Indoor Temp' AS metric1,
    'Outdoor AQI' AS metric2,
    CORR(indoor_temp, outdoor_aqi) AS correlation
FROM joined

UNION ALL

SELECT
    'Indoor CO2' AS metric1,
    'Outdoor Temp' AS metric2,
    CORR(indoor_co2, outdoor_temp) AS correlation
FROM joined

-- Add more correlation pairs as needed
```

**Display**:
- X-axis: metric1
- Y-axis: metric2
- Color scale: Red (-1) → White (0) → Green (+1)
- Cell annotation: Show correlation coefficient

**Note**: This is a V2 feature. For V1, simplify to a table of correlation values.

---

## 7. Performance Optimization Strategies

### 7.1 Time Range Switching

**Automatic aggregation level selection based on time range**:

```sql
-- Use this pattern in panel queries
SELECT
    CASE
        WHEN DATEDIFF('day', $__timeFrom(), $__timeTo()) <= 1 THEN 'raw'
        WHEN DATEDIFF('day', $__timeFrom(), $__timeTo()) <= 7 THEN 'hourly'
        ELSE 'daily'
    END AS aggregation_level
```

**Implementation approach**:
- Grafana doesn't support conditional queries directly
- Solution: Create multiple queries (A, B, C) and hide/show based on time range variable
- Alternative: Use transformation plugin for query switching

### 7.2 Query Result Limiting

**Prevent excessive data transfer**:

```sql
SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
LIMIT 10000  -- Safety limit
```

### 7.3 Indexing Recommendations

**For DuckDB persistence layer** (if implemented):

```sql
-- Create indexes on frequently filtered columns
CREATE INDEX idx_hourly_stream_bucket ON readings_hourly(stream_id, bucket);
CREATE INDEX idx_daily_stream_bucket ON readings_daily(stream_id, bucket);

-- Analyze tables for query optimization
ANALYZE readings_hourly;
ANALYZE readings_daily;
```

---

## 8. Query Debugging and Validation

### 8.1 Test Query Execution Times

**Run in DuckDB CLI before deploying to Grafana**:

```bash
# SSH into DuckDB container
docker exec -it ndp-duckdb duckdb /workspace/ndp.db

# Enable timing
.timer on

# Test query
SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= NOW() - INTERVAL '7 days'
ORDER BY bucket;
```

**Performance targets**:
- 7-day query: < 100ms
- 30-day query: < 500ms
- Correlation query: < 2s

### 8.2 Validate Data Completeness

**Check for gaps in time series**:

```sql
-- Find missing hours in last 7 days
WITH expected_hours AS (
    SELECT
        generate_series(
            DATE_TRUNC('hour', NOW() - INTERVAL '7 days'),
            DATE_TRUNC('hour', NOW()),
            INTERVAL '1 hour'
        ) AS expected_bucket
),
actual_hours AS (
    SELECT DISTINCT bucket
    FROM readings_hourly
    WHERE stream_id = 'air-quality'
)
SELECT expected_bucket
FROM expected_hours
WHERE expected_bucket NOT IN (SELECT bucket FROM actual_hours)
ORDER BY expected_bucket;
```

### 8.3 Verify Aggregation Logic

**Compare raw vs aggregated values**:

```sql
-- Manual hourly average calculation
SELECT
    DATE_TRUNC('hour', timestamp) AS hour,
    AVG(pm25) AS manual_avg_pm25
FROM silver_indoor_air
WHERE timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY hour
ORDER BY hour;

-- Compare with readings_hourly
SELECT
    bucket AS hour,
    avg_pm25
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= NOW() - INTERVAL '24 hours'
ORDER BY bucket;
```

---

## 9. Grafana Panel Configuration Reference

### 9.1 Time Series Panel Settings

**Common settings for all time series panels**:

```json
{
  "type": "timeseries",
  "options": {
    "legend": {
      "displayMode": "list",
      "placement": "bottom",
      "showLegend": true,
      "calcs": ["mean", "lastNotNull", "max"]
    },
    "tooltip": {
      "mode": "multi",
      "sort": "none"
    }
  },
  "fieldConfig": {
    "defaults": {
      "custom": {
        "drawStyle": "line",
        "lineInterpolation": "smooth",
        "lineWidth": 2,
        "fillOpacity": 10,
        "showPoints": "auto",
        "pointSize": 5
      },
      "thresholds": {
        "mode": "absolute",
        "steps": [
          { "color": "green", "value": null }
        ]
      }
    }
  }
}
```

### 9.2 Stat Panel Settings

**Common settings for stat panels**:

```json
{
  "type": "stat",
  "options": {
    "graphMode": "area",
    "colorMode": "background",
    "justifyMode": "auto",
    "textMode": "value_and_name",
    "orientation": "auto",
    "reduceOptions": {
      "values": false,
      "calcs": ["lastNotNull"]
    }
  },
  "fieldConfig": {
    "defaults": {
      "decimals": 1,
      "thresholds": {
        "mode": "absolute",
        "steps": [
          { "color": "green", "value": null }
        ]
      }
    }
  }
}
```

### 9.3 Gauge Panel Settings

**AQI gauge configuration**:

```json
{
  "type": "gauge",
  "options": {
    "showThresholdLabels": true,
    "showThresholdMarkers": true,
    "orientation": "auto",
    "reduceOptions": {
      "values": false,
      "calcs": ["lastNotNull"]
    }
  },
  "fieldConfig": {
    "defaults": {
      "min": 0,
      "max": 500,
      "thresholds": {
        "mode": "absolute",
        "steps": [
          { "color": "green", "value": 0 },
          { "color": "yellow", "value": 51 },
          { "color": "orange", "value": 101 },
          { "color": "red", "value": 151 },
          { "color": "purple", "value": 201 },
          { "color": "dark-red", "value": 301 }
        ]
      }
    }
  }
}
```

---

## 10. Dashboard Variables (Future Enhancement)

### 10.1 Stream ID Variable

**For multi-location support**:

```sql
-- Query for variable options
SELECT DISTINCT stream_id
FROM readings_hourly
ORDER BY stream_id
```

**Usage in panel queries**:
```sql
SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5"
FROM readings_hourly
WHERE
    stream_id = '$stream_id'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

### 10.2 Aggregation Level Variable

**Manual override for performance tuning**:

```sql
-- Variable: aggregation_level
-- Options: raw, hourly, daily
-- Default: auto

SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5"
FROM CASE
    WHEN '$aggregation_level' = 'raw' THEN silver_indoor_air
    WHEN '$aggregation_level' = 'hourly' THEN readings_hourly
    WHEN '$aggregation_level' = 'daily' THEN readings_daily
END
WHERE
    stream_id = 'air-quality'
    AND bucket >= $__timeFrom()
    AND bucket <= $__timeTo()
ORDER BY bucket
```

**Note**: DuckDB doesn't support dynamic table selection. This would require:
- Multiple queries with conditional display
- OR custom DuckDB macro/function

---

## 11. Implementation Checklist

### Phase 1: Core Queries

- [ ] Test all time series queries in DuckDB CLI
- [ ] Validate timestamp conversion (`to_timestamp(timestamp / 1000000)`)
- [ ] Verify `readings_hourly` aggregation accuracy
- [ ] Test `UNION ALL` queries for indoor vs outdoor comparisons
- [ ] Validate query performance for 7-day and 30-day ranges

### Phase 2: Panel Creation

- [ ] Create Indoor Air Quality dashboard with 7 panels
- [ ] Create Outdoor Conditions dashboard with 8 panels
- [ ] Create Outdoor Air Quality dashboard with 6 panels
- [ ] Create Comparison dashboard with 5 panels
- [ ] Configure thresholds for all panels
- [ ] Set up time range synchronization for Comparison dashboard

### Phase 3: Visual Polish

- [ ] Apply consistent color scheme across dashboards
- [ ] Configure legend displays (mean, last, max)
- [ ] Set appropriate Y-axis ranges
- [ ] Enable shared crosshair/tooltip on Comparison dashboard
- [ ] Add dashboard descriptions and links

### Phase 4: Performance Validation

- [ ] Measure query execution times
- [ ] Verify data refresh rate (5 minutes)
- [ ] Test dashboard load times
- [ ] Monitor DuckDB memory usage during queries
- [ ] Optimize slow queries

---

## 12. Future Enhancements

### 12.1 Advanced Visualizations

- **Heatmap Calendar**: PM2.5 levels by day of week and hour
- **Wind Rose**: Directional wind distribution
- **Scatter Plot**: Temperature vs humidity correlation
- **Histogram**: PM2.5 distribution over time

### 12.2 Calculated Fields

- **Dew Point**: Calculate from temperature and humidity
- **Heat Index**: Calculate apparent temperature
- **Air Quality Category**: Map AQI to text labels
- **Rate of Change**: Derivative of PM2.5 over time

### 12.3 Predictive Overlays

- **ML Forecast**: Overlay ruv-FANN predictions (ML-001 phase)
- **Trend Lines**: Linear regression overlays
- **Anomaly Detection**: Highlight outliers

---

## 13. References

- [DuckDB SQL Reference](https://duckdb.org/docs/sql/introduction)
- [Grafana Time Series Documentation](https://grafana.com/docs/grafana/latest/panels-visualizations/visualizations/time-series/)
- [Grafana Query Variables](https://grafana.com/docs/grafana/latest/dashboards/variables/)
- [DP-001 Feature Overview](../SCOPE.md)
- [DuckDB Specification](../specification/DUCKDB_SPECIFICATION.md)
- [Grafana Specification](../specification/GRAFANA_SPECIFICATION.md)

---

**Status**: Ready for Implementation
**Next Step**: Pseudocode → Architecture (ADR for dashboard structure)
