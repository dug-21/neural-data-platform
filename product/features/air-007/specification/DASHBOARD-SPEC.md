# Grafana Dashboard Specifications - AIR-007

**Feature**: AIR-007 - National Weather Service Data Integration
**Version**: 1.0.0
**Created**: 2025-12-24
**Status**: Draft

---

## Overview

This document specifies three Grafana dashboards for visualizing National Weather Service (NWS) data from two streams:
- `nws-forecast-hourly` - Hourly forecast data (156-hour rolling window)
- `nws-observations` - Real-time station observations (KSGJ)

All dashboards query Bronze Parquet files directly using the DuckDB datasource plugin, following the established pattern from existing dashboards.

---

## Dashboard Architecture

### Data Source Configuration

**DuckDB Datasource** (queries Parquet files directly):
- **Type**: `motherduck-duckdb-datasource`
- **UID**: `duckdb-ndp`
- **Access Mode**: Read-only (Parquet files)
- **Data Path Pattern**: `/data/data/{stream-id}/**/*.parquet`
- **No separate DuckDB server required** - plugin queries Parquet directly

### Query Pattern Standards

All queries follow these conventions:

1. **Timestamp Conversion**:
   ```sql
   to_timestamp(timestamp/1000000)  -- Convert microseconds to timestamp
   ```

2. **Time Filtering**:
   ```sql
   WHERE timestamp >= ${__from}::BIGINT * 1000
     AND timestamp <= ${__to}::BIGINT * 1000
   ```

3. **Time Bucketing**:
   ```sql
   time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time
   ```

4. **Metric Selection**:
   ```sql
   AVG(CASE WHEN metric = 'temperature' THEN value END) as "Temperature"
   ```

5. **Latest Value**:
   ```sql
   SELECT AVG(value) as value
   FROM (
     SELECT value
     FROM read_parquet('/data/data/nws-observations/**/*.parquet')
     WHERE metric = 'temperature'
     ORDER BY timestamp DESC
     LIMIT 100
   )
   ```

---

## Dashboard 1: NWS Hourly Forecast

### Metadata

```json
{
  "uid": "ndp-nws-forecast-hourly",
  "title": "NWS Hourly Forecast (156h)",
  "tags": ["ndp", "nws", "forecast", "weather"],
  "timezone": "browser",
  "refresh": "1h",
  "time": {
    "from": "now-7d",
    "to": "now+7d"
  }
}
```

### Panel Layout (4 Rows)

#### Row 1: Current Forecast Conditions (y=0, h=4)

**Panel 1.1: Forecast Temperature** (x=0, w=6)
- **Type**: `stat`
- **Query**: Latest temperature from forecast
- **Unit**: `fahrenheit`
- **Graph Mode**: `area`
- **Decimals**: 1

```sql
SELECT AVG(value) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE metric = 'temperature'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

**Panel 1.2: Dewpoint** (x=6, w=6)
- **Type**: `stat`
- **Query**: Latest dewpoint
- **Unit**: `celsius`
- **Graph Mode**: `area`
- **Decimals**: 1

```sql
SELECT AVG(value) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE metric = 'dewpoint'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

**Panel 1.3: Humidity** (x=12, w=6)
- **Type**: `stat`
- **Query**: Latest relative_humidity
- **Unit**: `percent`
- **Graph Mode**: `none`
- **Decimals**: 0

```sql
SELECT AVG(value) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE metric = 'relative_humidity'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

**Panel 1.4: Precipitation Probability** (x=18, w=6)
- **Type**: `stat`
- **Query**: Latest probability_of_precipitation
- **Unit**: `percent`
- **Graph Mode**: `none`
- **Color Mode**: `background`
- **Thresholds**:
  - Green: 0-30%
  - Yellow: 30-60%
  - Orange: 60-80%
  - Red: 80-100%

```sql
SELECT AVG(value) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE metric = 'probability_of_precipitation'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

#### Row 2: Temperature Forecast (y=4, h=8)

**Panel 2.1: Temperature & Dewpoint Trend** (x=0, w=24)
- **Type**: `timeseries`
- **Query**: Time series with temperature and dewpoint

```sql
SELECT
  time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'temperature' THEN value END) as "Temperature (°F)",
  AVG(CASE WHEN metric = 'dewpoint' THEN
    -- Convert Celsius to Fahrenheit for comparison
    (value * 9/5) + 32
  END) as "Dewpoint (°F)"
FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `fahrenheit`
- **Line Width**: 2
- **Fill Opacity**: 10

#### Row 3: Wind Forecast (y=12, h=8)

**Panel 3.1: Wind Speed & Direction** (x=0, w=24)
- **Type**: `timeseries`
- **Query**: Wind speed with direction overlay

```sql
SELECT
  time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'wind_speed' THEN value END) as "Wind Speed (mph)",
  AVG(CASE WHEN metric = 'wind_direction' THEN value END) as "Direction (°)"
FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config Overrides**:
- **Wind Speed**:
  - Unit: `velocitymph`
  - Axis: Left
  - Line Width: 2
- **Direction**:
  - Unit: `degree`
  - Axis: Right
  - Line Width: 1
  - Draw Style: Points

#### Row 4: Precipitation & Humidity (y=20, h=8)

**Panel 4.1: Precipitation Probability** (x=0, w=12)
- **Type**: `timeseries`
- **Query**: Precipitation probability over time

```sql
SELECT
  time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'probability_of_precipitation' THEN value END) as "Precip %"
FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `percent`
- **Line Width**: 2
- **Fill Opacity**: 30
- **Gradient**: Blue gradient
- **Thresholds** (for coloring):
  - 0-30%: Green
  - 30-60%: Yellow
  - 60-80%: Orange
  - 80-100%: Red

**Panel 4.2: Relative Humidity** (x=12, w=12)
- **Type**: `timeseries`
- **Query**: Humidity forecast

```sql
SELECT
  time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'relative_humidity' THEN value END) as "Humidity %"
FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `percent`
- **Line Width**: 2
- **Fill Opacity**: 20

---

## Dashboard 2: NWS Station Observations (KSGJ)

### Metadata

```json
{
  "uid": "ndp-nws-observations",
  "title": "NWS Station Observations (KSGJ)",
  "tags": ["ndp", "nws", "observations", "ksgj"],
  "timezone": "browser",
  "refresh": "5m",
  "time": {
    "from": "now-7d",
    "to": "now"
  }
}
```

### Panel Layout (4 Rows)

#### Row 1: Current Conditions (y=0, h=4)

**Panel 1.1: Temperature** (x=0, w=4)
- **Type**: `stat`
- **Query**: Latest temperature
- **Unit**: `celsius`
- **Graph Mode**: `area`

```sql
SELECT AVG(value) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE metric = 'temperature'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

**Panel 1.2: Dewpoint** (x=4, w=4)
- **Type**: `stat`
- **Query**: Latest dewpoint
- **Unit**: `celsius`
- **Graph Mode**: `area`

**Panel 1.3: Wind Speed** (x=8, w=4)
- **Type**: `stat`
- **Query**: Latest wind_speed
- **Unit**: `velocitykmh`
- **Color Mode**: `background`
- **Thresholds**:
  - Green: 0-20
  - Yellow: 20-40
  - Red: 40+

**Panel 1.4: Wind Gust** (x=12, w=4)
- **Type**: `stat`
- **Query**: Latest wind_gust
- **Unit**: `velocitykmh`
- **Color Mode**: `background`
- **Thresholds**:
  - Green: 0-30
  - Yellow: 30-50
  - Red: 50+

**Panel 1.5: Pressure** (x=16, w=4)
- **Type**: `stat`
- **Query**: Latest barometric_pressure
- **Unit**: `pressurehpa`
- **Transform**: Divide by 100 (Pa to hPa)

```sql
SELECT AVG(value/100) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE metric = 'barometric_pressure'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

**Panel 1.6: Visibility** (x=20, w=4)
- **Type**: `stat`
- **Query**: Latest visibility
- **Unit**: `lengthkm`
- **Transform**: Divide by 1000 (meters to km)

```sql
SELECT AVG(value/1000) as value
FROM (
  SELECT value
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE metric = 'visibility'
  ORDER BY timestamp DESC
  LIMIT 100
)
```

#### Row 2: Temperature & Humidity (y=4, h=8)

**Panel 2.1: Temperature Trends** (x=0, w=24)
- **Type**: `timeseries`
- **Query**: Temperature, dewpoint, wind_chill, heat_index

```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'temperature' THEN value END) as "Temperature",
  AVG(CASE WHEN metric = 'dewpoint' THEN value END) as "Dewpoint",
  AVG(CASE WHEN metric = 'wind_chill' THEN value END) as "Wind Chill",
  AVG(CASE WHEN metric = 'heat_index' THEN value END) as "Heat Index"
FROM read_parquet('/data/data/nws-observations/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `celsius`
- **Line Width**: 2
- **Legend**: Bottom, list mode

#### Row 3: Wind & Pressure (y=12, h=8)

**Panel 3.1: Wind Speed & Gusts** (x=0, w=12)
- **Type**: `timeseries`
- **Query**: Wind speed with gust overlay

```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'wind_speed' THEN value END) as "Wind Speed",
  AVG(CASE WHEN metric = 'wind_gust' THEN value END) as "Gusts"
FROM read_parquet('/data/data/nws-observations/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `velocitykmh`
- **Line Width**: 2
- **Fill Opacity**: Wind Speed = 10, Gusts = 0

**Panel 3.2: Barometric Pressure** (x=12, w=12)
- **Type**: `timeseries`
- **Query**: Barometric and sea level pressure

```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'barometric_pressure' THEN value/100 END) as "Barometric",
  AVG(CASE WHEN metric = 'sea_level_pressure' THEN value/100 END) as "Sea Level"
FROM read_parquet('/data/data/nws-observations/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `pressurehpa`
- **Line Width**: 2

#### Row 4: Precipitation & Extremes (y=20, h=8)

**Panel 4.1: Precipitation (1h/3h/6h)** (x=0, w=12)
- **Type**: `timeseries`
- **Query**: Multiple precipitation windows

```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'precipitation_1h' THEN value*1000 END) as "1 Hour",
  AVG(CASE WHEN metric = 'precipitation_3h' THEN value*1000 END) as "3 Hours",
  AVG(CASE WHEN metric = 'precipitation_6h' THEN value*1000 END) as "6 Hours"
FROM read_parquet('/data/data/nws-observations/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `lengthmm` (converted from meters)
- **Line Width**: 2
- **Draw Style**: Bars

**Panel 4.2: 24h Temperature Range** (x=12, w=12)
- **Type**: `timeseries`
- **Query**: Min/max temperatures

```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'max_temperature_24h' THEN value END) as "Max (24h)",
  AVG(CASE WHEN metric = 'min_temperature_24h' THEN value END) as "Min (24h)"
FROM read_parquet('/data/data/nws-observations/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**Field Config**:
- **Unit**: `celsius`
- **Line Width**: 2

---

## Dashboard 3: Forecast vs Observations

### Metadata

```json
{
  "uid": "ndp-nws-forecast-comparison",
  "title": "NWS Forecast vs Observations",
  "tags": ["ndp", "nws", "comparison", "accuracy"],
  "timezone": "browser",
  "refresh": "10m",
  "time": {
    "from": "now-7d",
    "to": "now"
  }
}
```

### Panel Layout (3 Rows)

#### Row 1: Temperature Comparison (y=0, h=10)

**Panel 1.1: Forecast vs Observed Temperature** (x=0, w=18)
- **Type**: `timeseries`
- **Query**: Join forecast and observation temperatures

```sql
WITH forecast AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'temperature' THEN (value - 32) * 5/9 END) as temp_c
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
),
observed AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'temperature' THEN value END) as temp_c
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  COALESCE(f.time, o.time) as time,
  f.temp_c as "Forecast",
  o.temp_c as "Observed"
FROM forecast f
FULL OUTER JOIN observed o ON f.time = o.time
ORDER BY 1
```

**Field Config**:
- **Unit**: `celsius`
- **Line Width**: Forecast=1, Observed=2
- **Draw Style**: Forecast=Line (dashed), Observed=Line (solid)

**Panel 1.2: Temperature Error** (x=18, w=6)
- **Type**: `stat`
- **Query**: Average absolute error

```sql
WITH forecast AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'temperature' THEN (value - 32) * 5/9 END) as temp_c
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
),
observed AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'temperature' THEN value END) as temp_c
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
)
SELECT AVG(ABS(f.temp_c - o.temp_c)) as value
FROM forecast f
INNER JOIN observed o ON f.time = o.time
WHERE f.temp_c IS NOT NULL AND o.temp_c IS NOT NULL
```

**Field Config**:
- **Unit**: `celsius`
- **Decimals**: 2
- **Color Mode**: `background`
- **Thresholds**:
  - Green: 0-1°C
  - Yellow: 1-2°C
  - Red: 2°C+

#### Row 2: Wind Comparison (y=10, h=8)

**Panel 2.1: Wind Speed Comparison** (x=0, w=24)
- **Type**: `timeseries`
- **Query**: Forecast vs observed wind speed

```sql
WITH forecast AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'wind_speed' THEN value * 1.60934 END) as wind_kmh
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
),
observed AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'wind_speed' THEN value END) as wind_kmh
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  COALESCE(f.time, o.time) as time,
  f.wind_kmh as "Forecast Wind",
  o.wind_kmh as "Observed Wind"
FROM forecast f
FULL OUTER JOIN observed o ON f.time = o.time
ORDER BY 1
```

**Field Config**:
- **Unit**: `velocitykmh`
- **Line Width**: 2

#### Row 3: Humidity Comparison (y=18, h=8)

**Panel 3.1: Humidity Comparison** (x=0, w=24)
- **Type**: `timeseries`
- **Query**: Forecast vs observed humidity

```sql
WITH forecast AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'relative_humidity' THEN value END) as humidity_pct
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
),
observed AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'relative_humidity' THEN value END) as humidity_pct
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  COALESCE(f.time, o.time) as time,
  f.humidity_pct as "Forecast Humidity",
  o.humidity_pct as "Observed Humidity"
FROM forecast f
FULL OUTER JOIN observed o ON f.time = o.time
ORDER BY 1
```

**Field Config**:
- **Unit**: `percent`
- **Line Width**: 2

---

## Dashboard Variables

All three dashboards should include these template variables:

### Time Range Quick Selector

```json
{
  "name": "time_range",
  "type": "interval",
  "label": "Quick Range",
  "options": [
    {"text": "Last 6 Hours", "value": "6h"},
    {"text": "Last 24 Hours", "value": "24h"},
    {"text": "Last 3 Days", "value": "3d"},
    {"text": "Last 7 Days", "value": "7d"},
    {"text": "Last 30 Days", "value": "30d"}
  ],
  "current": {"text": "Last 7 Days", "value": "7d"}
}
```

### Location Filter (Future Enhancement)

```json
{
  "name": "location_id",
  "type": "constant",
  "label": "Location",
  "query": "ksgj",
  "current": {"text": "KSGJ", "value": "ksgj"},
  "hide": 2
}
```

---

## Implementation Checklist

### Phase 1: Basic Dashboards
- [ ] Create `nws-forecast-hourly.json` with 4 rows
- [ ] Create `nws-observations.json` with 4 rows
- [ ] Test queries against Bronze Parquet data
- [ ] Verify threshold coloring works
- [ ] Validate unit conversions (°F to °C, m to km, Pa to hPa, mph to km/h)

### Phase 2: Comparison Dashboard
- [ ] Create `nws-forecast-comparison.json`
- [ ] Implement JOIN queries for aligned time series
- [ ] Calculate error metrics (MAE, RMSE)
- [ ] Add error visualization panels

### Phase 3: Provisioning
- [ ] Add dashboard JSON files to `config/grafana/dashboards/`
- [ ] Update provisioning config to include new dashboards
- [ ] Test auto-provisioning on container restart
- [ ] Verify anonymous viewer access

### Phase 4: Documentation
- [ ] Document query patterns for NWS data
- [ ] Create troubleshooting guide for dashboard issues
- [ ] Add example screenshots to feature documentation
- [ ] Update platform architecture docs

---

## Performance Considerations

### Expected Query Performance

Based on DP-001 benchmarks (Raspberry Pi 5):
- **Stat Panels** (latest value): <100ms
- **7-day Time Series**: <1 second
- **30-day Time Series**: <5 seconds
- **Forecast Comparison JOINs**: <2 seconds

### Optimization Techniques

1. **Partition Pruning**: Timestamp filters enable daily file pruning
2. **Columnar Scanning**: Only read requested metric columns
3. **Time Bucketing**: Reduce data points for visualization
4. **Limit Clauses**: Use LIMIT 100 for latest value queries

### Monitoring

- Track query latency via Grafana metrics
- Monitor Parquet file growth in Bronze layer

---

## Unit Conversion Reference

| Stream | Metric | Stored Unit | Display Unit | Conversion |
|--------|--------|-------------|--------------|------------|
| nws-forecast-hourly | temperature | Fahrenheit | Fahrenheit | None |
| nws-forecast-hourly | dewpoint | Celsius | Fahrenheit | `(°C * 9/5) + 32` |
| nws-forecast-hourly | wind_speed | mph | mph | None |
| nws-observations | temperature | Celsius | Celsius | None |
| nws-observations | wind_speed | km/h | km/h | None |
| nws-observations | barometric_pressure | Pa | hPa | `/100` |
| nws-observations | visibility | meters | km | `/1000` |
| nws-observations | precipitation | meters | mm | `*1000` |

---

## Color Scheme Standards

Following NDP dashboard conventions:

### Temperature
- No thresholds (neutral display)

### Wind Speed
- **Green**: 0-20 km/h (calm)
- **Yellow**: 20-40 km/h (moderate)
- **Red**: 40+ km/h (strong)

### Precipitation Probability
- **Green**: 0-30% (unlikely)
- **Yellow**: 30-60% (possible)
- **Orange**: 60-80% (likely)
- **Red**: 80-100% (very likely)

### Humidity
- No thresholds (neutral display)

### Pressure
- No thresholds (neutral display)

---

## Related Documentation

- **Platform Architecture**: `/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
- **Stream Configs**:
  - `/config/base/streams/nws-gridpoints-forecast/config.yaml`
  - `/config/base/streams/nws-station-observations/config.yaml`
- **Existing Dashboards**: `/config/grafana/dashboards/*.json`

---

## Future Enhancements

### Short-Term
- Add "short_forecast" text description panel
- Implement wind direction compass visualization
- Add forecast age/staleness indicator

### Medium-Term
- Create alert rules for extreme conditions
- Add multi-location support via variables
- Implement forecast skill score metrics

### Long-Term
- Build ML model comparison dashboard (NWS vs internal forecasts)
- Add gridpoint-level forecast visualization (heatmaps)
- Integrate weather alerts and warnings
