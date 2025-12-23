# Exploration Dashboard Recommendations for Silver Layer Design

## Executive Summary

This document proposes new Grafana dashboards designed to help make data-driven decisions about the Silver layer architecture. These dashboards complement existing visualization by focusing on data quality, reliability, completeness, and source comparison to inform TimescaleDB schema design.

## Current Dashboard Analysis

### Existing Dashboards
1. **indoor-air-quality.json** - Basic AirGradient sensor metrics
2. **indoor-vs-outdoor.json** - Indoor/outdoor condition deltas
3. **outdoor-conditions.json** - OpenWeatherMap (OWM) weather data
4. **nws-vs-owm-comparison.json** - Weather source comparison
5. **nws-forecast-accuracy.json** - Forecast validation

### Gaps Identified
- No data quality/completeness metrics
- No temporal resolution analysis
- No correlation analysis for feature engineering
- No anomaly detection visualization
- No gap analysis for ETL planning
- Limited decision-support metrics

## Recommended New Dashboards

---

## Dashboard 1: Data Quality & Completeness

**Purpose**: Understand data reliability, gaps, and quality issues to inform retention policies and ETL strategies.

**Key Decisions Supported**:
- Which streams are reliable enough for Silver layer?
- What aggregation windows are feasible?
- Where do we need gap-filling strategies?
- What's the actual vs expected data volume?

### Panel 1: Stream Health Overview (Stat Panel Grid)
```sql
-- Records per stream (last 24h)
SELECT
  location_id as stream,
  COUNT(*) as records,
  MIN(timestamp) as first_seen,
  MAX(timestamp) as last_seen,
  (MAX(timestamp) - MIN(timestamp)) / 1000000 / 3600 as hours_span
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY location_id
ORDER BY records DESC;
```

### Panel 2: Data Gaps Timeline (Time Series)
```sql
-- Identify gaps > 10 minutes
WITH time_series AS (
  SELECT
    location_id,
    to_timestamp(timestamp/1000000) as ts,
    LEAD(to_timestamp(timestamp/1000000)) OVER (
      PARTITION BY location_id ORDER BY timestamp
    ) as next_ts
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
),
gaps AS (
  SELECT
    location_id as stream,
    ts as gap_start,
    next_ts as gap_end,
    EXTRACT(EPOCH FROM (next_ts - ts)) / 60 as gap_minutes
  FROM time_series
  WHERE EXTRACT(EPOCH FROM (next_ts - ts)) / 60 > 10
)
SELECT
  gap_start as time,
  stream,
  gap_minutes as "Gap Duration (min)"
FROM gaps
ORDER BY gap_start;
```

### Panel 3: Expected vs Actual Data Rate (Time Series)
```sql
-- Compare actual ingestion rate to expected
WITH hourly_counts AS (
  SELECT
    location_id as stream,
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    COUNT(*) as actual_records
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__to}::BIGINT * 1000
  GROUP BY location_id, 2
)
SELECT
  hour as time,
  stream,
  actual_records as "Actual Records",
  CASE
    WHEN stream LIKE 'air-quality%' THEN 360  -- Expected: 6/min * 60min
    WHEN stream LIKE 'outdoor-weather%' THEN 12  -- Expected: 5min intervals
    WHEN stream LIKE 'nws%' THEN 60  -- Expected: 1min intervals
  END as "Expected Records",
  (actual_records * 100.0 / NULLIF(
    CASE
      WHEN stream LIKE 'air-quality%' THEN 360
      WHEN stream LIKE 'outdoor-weather%' THEN 12
      WHEN stream LIKE 'nws%' THEN 60
    END, 0
  )) as "Completeness %"
FROM hourly_counts
ORDER BY hour;
```

### Panel 4: Null Value Distribution (Bar Chart)
```sql
-- Check data quality by metric
SELECT
  metric,
  COUNT(*) as total_records,
  COUNT(*) FILTER (WHERE value IS NULL) as null_values,
  (COUNT(*) FILTER (WHERE value IS NULL) * 100.0 / NULLIF(COUNT(*), 0)) as "Null %"
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY metric
ORDER BY "Null %" DESC;
```

### Panel 5: Timestamp Drift Detection (Time Series)
```sql
-- Detect ingestion lag (timestamp vs ingest_time)
SELECT
  to_timestamp(timestamp/1000000) as time,
  location_id as stream,
  AVG((ingest_timestamp - timestamp) / 1000000) as "Lag Seconds"
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
  AND ingest_timestamp IS NOT NULL
GROUP BY time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)), location_id
ORDER BY time;
```

**Decision Support**:
- If completeness < 80%, implement gap-filling or increase retention
- If gaps > 1 hour are common, design for sparse data in Silver layer
- If null values > 5%, add validation before ETL

---

## Dashboard 2: Weather Source Reliability

**Purpose**: Determine which weather source (NWS vs OWM) should be primary for Silver layer.

**Key Decisions Supported**:
- Which source is more accurate?
- Which source has better uptime?
- Should we implement failover or source weighting?
- Which metrics are most reliable from each source?

### Panel 1: Source Uptime Comparison (Stat Panel)
```sql
-- Uptime percentage (last 7 days)
WITH expected_records AS (
  SELECT
    'NWS' as source,
    (EXTRACT(EPOCH FROM (NOW() - (NOW() - INTERVAL '7 days'))) / 60) as expected_count
  UNION ALL
  SELECT 'OWM', (EXTRACT(EPOCH FROM (NOW() - (NOW() - INTERVAL '7 days'))) / 300)
),
actual_records AS (
  SELECT 'NWS' as source, COUNT(*) as actual_count
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  UNION ALL
  SELECT 'OWM', COUNT(*)
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
)
SELECT
  e.source,
  (a.actual_count * 100.0 / NULLIF(e.expected_count, 0)) as "Uptime %"
FROM expected_records e
JOIN actual_records a ON e.source = a.source;
```

### Panel 2: Accuracy Comparison by Metric (Table)
```sql
-- Compare accuracy when both sources available
-- Using AirGradient as "ground truth" for temperature
WITH nws_temps AS (
  SELECT
    time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as bucket,
    AVG(value) as nws_temp
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE metric = 'temperature'
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
),
owm_temps AS (
  SELECT
    time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as bucket,
    AVG(value) as owm_temp
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE metric = 'temperature'
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
),
indoor_temps AS (
  SELECT
    time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as bucket,
    AVG(value) as indoor_temp
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE metric IN ('atmp', 'temperature')
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  'Temperature' as metric,
  AVG(ABS(n.nws_temp - i.indoor_temp)) as "NWS MAE",
  AVG(ABS(o.owm_temp - i.indoor_temp)) as "OWM MAE",
  COUNT(*) FILTER (WHERE n.nws_temp IS NOT NULL) as "NWS Records",
  COUNT(*) FILTER (WHERE o.owm_temp IS NOT NULL) as "OWM Records"
FROM indoor_temps i
LEFT JOIN nws_temps n ON i.bucket = n.bucket
LEFT JOIN owm_temps o ON i.bucket = o.bucket;
```

### Panel 3: Data Freshness Comparison (Time Series)
```sql
-- How quickly does each source update?
WITH nws_freshness AS (
  SELECT
    to_timestamp(timestamp/1000000) as time,
    LAG(to_timestamp(timestamp/1000000)) OVER (ORDER BY timestamp) as prev_time
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
),
owm_freshness AS (
  SELECT
    to_timestamp(timestamp/1000000) as time,
    LAG(to_timestamp(timestamp/1000000)) OVER (ORDER BY timestamp) as prev_time
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
)
SELECT
  time,
  'NWS' as source,
  EXTRACT(EPOCH FROM (time - prev_time)) / 60 as "Update Interval (min)"
FROM nws_freshness
WHERE prev_time IS NOT NULL
UNION ALL
SELECT
  time,
  'OWM' as source,
  EXTRACT(EPOCH FROM (time - prev_time)) / 60
FROM owm_freshness
WHERE prev_time IS NOT NULL
ORDER BY time;
```

### Panel 4: Divergence Over Time (Time Series)
```sql
-- When do NWS and OWM disagree most?
WITH nws AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    metric,
    AVG(value) as nws_value
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1, 2
),
owm AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    CASE
      WHEN metric = 'temperature' THEN 'temperature'
      WHEN metric = 'humidity' THEN 'relative_humidity'
      WHEN metric = 'pressure' THEN 'barometric_pressure'
      WHEN metric = 'wind_speed' THEN 'wind_speed'
    END as metric,
    AVG(CASE
      WHEN metric = 'wind_speed' THEN value * 3.6  -- m/s to km/h
      WHEN metric = 'pressure' THEN value * 100    -- hPa to Pa
      ELSE value
    END) as owm_value
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1, 2
)
SELECT
  n.hour as time,
  n.metric,
  ABS(n.nws_value - o.owm_value) as "Absolute Difference",
  ((n.nws_value - o.owm_value) / NULLIF(n.nws_value, 0) * 100) as "Percent Difference"
FROM nws n
JOIN owm o ON n.hour = o.hour AND n.metric = o.metric
ORDER BY time;
```

### Panel 5: Correlation Coefficient by Metric (Stat Panel)
```sql
-- Statistical correlation between sources
WITH paired_data AS (
  SELECT
    n.metric,
    n.value as nws_value,
    o.value as owm_value
  FROM read_parquet('/data/data/nws-observations/**/*.parquet') n
  JOIN read_parquet('/data/data/outdoor-weather/**/*.parquet') o
    ON ABS(n.timestamp - o.timestamp) < 600000000  -- Within 10 minutes
    AND n.metric = CASE
      WHEN o.metric = 'temperature' THEN 'temperature'
      WHEN o.metric = 'humidity' THEN 'relative_humidity'
      WHEN o.metric = 'pressure' THEN 'barometric_pressure'
    END
  WHERE n.timestamp >= ${__from}::BIGINT * 1000
)
SELECT
  metric,
  CORR(nws_value, owm_value) as "Correlation"
FROM paired_data
GROUP BY metric;
```

**Decision Support**:
- If uptime difference > 10%, use more reliable source as primary
- If MAE difference > 20%, consider single-source strategy
- If correlation < 0.8 for any metric, investigate and possibly exclude
- Freshness analysis informs continuous aggregate refresh intervals

---

## Dashboard 3: Indoor-Outdoor Correlation Analysis

**Purpose**: Understand relationships for feature engineering and predictive modeling.

**Key Decisions Supported**:
- Which outdoor factors predict indoor conditions?
- What lag exists between outdoor changes and indoor impact?
- Which features should be included in ML models?
- What aggregation windows capture meaningful patterns?

### Panel 1: Cross-Correlation Heatmap (Table)
```sql
-- Correlation matrix for all metrics
WITH indoor AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    MAX(CASE WHEN metric = 'pm02' THEN value END) as indoor_pm25,
    MAX(CASE WHEN metric = 'rco2' THEN value END) as indoor_co2,
    MAX(CASE WHEN metric IN ('atmp', 'temperature') THEN value END) as indoor_temp,
    MAX(CASE WHEN metric IN ('rhum', 'humidity') THEN value END) as indoor_humidity
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
),
outdoor AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    MAX(CASE WHEN metric = 'temperature' THEN value END) as outdoor_temp,
    MAX(CASE WHEN metric = 'humidity' THEN value END) as outdoor_humidity,
    MAX(CASE WHEN metric = 'wind_speed' THEN value END) as outdoor_wind,
    MAX(CASE WHEN metric = 'pressure' THEN value END) as outdoor_pressure
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
),
outdoor_aq AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    MAX(CASE WHEN metric = 'pm2_5' THEN value END) as outdoor_pm25
  FROM read_parquet('/data/data/outdoor-air-quality/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  'Indoor Temp' as metric,
  CORR(i.indoor_temp, o.outdoor_temp) as "Outdoor Temp",
  CORR(i.indoor_temp, o.outdoor_humidity) as "Outdoor Humidity",
  CORR(i.indoor_temp, o.outdoor_wind) as "Wind Speed",
  CORR(i.indoor_temp, o.outdoor_pressure) as "Pressure"
FROM indoor i
JOIN outdoor o ON i.hour = o.hour
UNION ALL
SELECT
  'Indoor PM2.5',
  CORR(i.indoor_pm25, o.outdoor_temp),
  CORR(i.indoor_pm25, o.outdoor_humidity),
  CORR(i.indoor_pm25, o.outdoor_wind),
  CORR(i.indoor_pm25, o.outdoor_pressure)
FROM indoor i
JOIN outdoor o ON i.hour = o.hour
UNION ALL
SELECT
  'Indoor Humidity',
  CORR(i.indoor_humidity, o.outdoor_temp),
  CORR(i.indoor_humidity, o.outdoor_humidity),
  CORR(i.indoor_humidity, o.outdoor_wind),
  CORR(i.indoor_humidity, o.outdoor_pressure)
FROM indoor i
JOIN outdoor o ON i.hour = o.hour;
```

### Panel 2: Lag Analysis - Temperature Propagation (Time Series)
```sql
-- How long for outdoor temp changes to affect indoor?
WITH outdoor_changes AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    AVG(value) as temp,
    AVG(value) - LAG(AVG(value)) OVER (ORDER BY time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000))) as temp_change
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE metric = 'temperature'
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
),
indoor_response AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    AVG(value) as temp,
    AVG(value) - LAG(AVG(value)) OVER (ORDER BY time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000))) as temp_change
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE metric IN ('atmp', 'temperature')
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  o.hour as time,
  o.temp_change as "Outdoor Temp Change",
  i.temp_change as "Indoor Temp Change (0h lag)",
  LAG(i.temp_change, 1) OVER (ORDER BY o.hour) as "Indoor Temp Change (1h lag)",
  LAG(i.temp_change, 2) OVER (ORDER BY o.hour) as "Indoor Temp Change (2h lag)",
  LAG(i.temp_change, 3) OVER (ORDER BY o.hour) as "Indoor Temp Change (3h lag)"
FROM outdoor_changes o
LEFT JOIN indoor_response i ON o.hour = i.hour
ORDER BY time;
```

### Panel 3: PM2.5 Indoor/Outdoor Ratio by Conditions (Scatter Plot)
```sql
-- Understanding filtration effectiveness under different conditions
WITH combined AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(i.timestamp/1000000)) as hour,
    AVG(i.value) as indoor_pm25,
    AVG(o.value) as outdoor_pm25,
    AVG(w.value) FILTER (WHERE w.metric = 'humidity') as humidity,
    AVG(w.value) FILTER (WHERE w.metric = 'wind_speed') as wind_speed
  FROM read_parquet('/data/data/air-quality/**/*.parquet') i
  LEFT JOIN read_parquet('/data/data/outdoor-air-quality/**/*.parquet') o
    ON ABS(i.timestamp - o.timestamp) < 3600000000  -- Within 1 hour
    AND o.metric = 'pm2_5'
  LEFT JOIN read_parquet('/data/data/outdoor-weather/**/*.parquet') w
    ON ABS(i.timestamp - w.timestamp) < 3600000000
  WHERE i.metric = 'pm02'
    AND i.timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  hour as time,
  outdoor_pm25 as "Outdoor PM2.5",
  indoor_pm25 as "Indoor PM2.5",
  (indoor_pm25 / NULLIF(outdoor_pm25, 0)) as "I/O Ratio",
  humidity as "Humidity %",
  wind_speed as "Wind Speed"
FROM combined
WHERE outdoor_pm25 IS NOT NULL AND indoor_pm25 IS NOT NULL
ORDER BY time;
```

### Panel 4: Diurnal Pattern Comparison (Time Series)
```sql
-- Compare 24-hour patterns between indoor/outdoor
WITH hourly_patterns AS (
  SELECT
    EXTRACT(HOUR FROM to_timestamp(timestamp/1000000)) as hour_of_day,
    'Indoor Temp' as metric,
    AVG(value) as avg_value
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE metric IN ('atmp', 'temperature')
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
  UNION ALL
  SELECT
    EXTRACT(HOUR FROM to_timestamp(timestamp/1000000)),
    'Outdoor Temp',
    AVG(value)
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE metric = 'temperature'
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
  UNION ALL
  SELECT
    EXTRACT(HOUR FROM to_timestamp(timestamp/1000000)),
    'Indoor PM2.5',
    AVG(value)
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE metric = 'pm02'
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
  UNION ALL
  SELECT
    EXTRACT(HOUR FROM to_timestamp(timestamp/1000000)),
    'Outdoor PM2.5',
    AVG(value)
  FROM read_parquet('/data/data/outdoor-air-quality/**/*.parquet')
  WHERE metric = 'pm2_5'
    AND timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  hour_of_day as "Hour",
  metric,
  avg_value as "Average Value"
FROM hourly_patterns
ORDER BY hour_of_day, metric;
```

### Panel 5: Feature Importance Ranking (Bar Chart)
```sql
-- Which features have strongest predictive power for indoor PM2.5?
WITH features AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(i.timestamp/1000000)) as hour,
    AVG(i.value) as indoor_pm25,
    AVG(o.value) FILTER (WHERE o.metric = 'pm2_5') as outdoor_pm25,
    AVG(w.value) FILTER (WHERE w.metric = 'temperature') as outdoor_temp,
    AVG(w.value) FILTER (WHERE w.metric = 'humidity') as outdoor_humidity,
    AVG(w.value) FILTER (WHERE w.metric = 'wind_speed') as wind_speed,
    AVG(w.value) FILTER (WHERE w.metric = 'pressure') as pressure
  FROM read_parquet('/data/data/air-quality/**/*.parquet') i
  LEFT JOIN read_parquet('/data/data/outdoor-air-quality/**/*.parquet') o
    ON ABS(i.timestamp - o.timestamp) < 3600000000
  LEFT JOIN read_parquet('/data/data/outdoor-weather/**/*.parquet') w
    ON ABS(i.timestamp - w.timestamp) < 3600000000
  WHERE i.metric = 'pm02'
    AND i.timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  'Outdoor PM2.5' as feature,
  ABS(CORR(indoor_pm25, outdoor_pm25)) as "Correlation Strength"
FROM features
UNION ALL
SELECT 'Temperature', ABS(CORR(indoor_pm25, outdoor_temp)) FROM features
UNION ALL
SELECT 'Humidity', ABS(CORR(indoor_pm25, outdoor_humidity)) FROM features
UNION ALL
SELECT 'Wind Speed', ABS(CORR(indoor_pm25, wind_speed)) FROM features
UNION ALL
SELECT 'Pressure', ABS(CORR(indoor_pm25, pressure)) FROM features
ORDER BY "Correlation Strength" DESC;
```

**Decision Support**:
- High correlations (> 0.7) indicate features to include in Silver layer
- Lag analysis informs window functions and lookback periods
- I/O ratio patterns guide filtration effectiveness metrics
- Feature ranking prioritizes which aggregations to compute

---

## Dashboard 4: Anomaly Detection & Data Profiling

**Purpose**: Identify outliers, sensor errors, and data quality issues before ETL to Silver layer.

**Key Decisions Supported**:
- What validation rules should ETL include?
- Are there systematic sensor biases?
- Which records should be filtered/corrected?
- What are normal operating ranges?

### Panel 1: Statistical Outliers by Metric (Table)
```sql
-- Z-score based outlier detection
WITH stats AS (
  SELECT
    metric,
    AVG(value) as mean,
    STDDEV(value) as stddev,
    MIN(value) as min_val,
    MAX(value) as max_val,
    PERCENTILE_CONT(0.01) WITHIN GROUP (ORDER BY value) as p01,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY value) as p99
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY metric
),
outliers AS (
  SELECT
    d.metric,
    COUNT(*) FILTER (WHERE ABS(d.value - s.mean) > 3 * s.stddev) as outlier_count,
    COUNT(*) as total_count
  FROM read_parquet('/data/data/**/*.parquet') d
  JOIN stats s ON d.metric = s.metric
  WHERE d.timestamp >= ${__from}::BIGINT * 1000
  GROUP BY d.metric
)
SELECT
  s.metric,
  s.mean as "Mean",
  s.stddev as "Std Dev",
  s.min_val as "Min",
  s.max_val as "Max",
  s.p01 as "1st Percentile",
  s.p99 as "99th Percentile",
  o.outlier_count as "Outliers (>3σ)",
  (o.outlier_count * 100.0 / NULLIF(o.total_count, 0)) as "Outlier %"
FROM stats s
JOIN outliers o ON s.metric = o.metric
ORDER BY "Outlier %" DESC;
```

### Panel 2: Anomaly Timeline (Time Series with Annotations)
```sql
-- Show anomalous readings over time
WITH stats AS (
  SELECT
    metric,
    AVG(value) as mean,
    STDDEV(value) as stddev
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY metric
)
SELECT
  to_timestamp(d.timestamp/1000000) as time,
  d.metric,
  d.value,
  s.mean as "Expected Mean",
  CASE
    WHEN ABS(d.value - s.mean) > 3 * s.stddev THEN 1
    ELSE 0
  END as "Is Anomaly"
FROM read_parquet('/data/data/**/*.parquet') d
JOIN stats s ON d.metric = s.metric
WHERE d.timestamp >= ${__from}::BIGINT * 1000
  AND ABS(d.value - s.mean) > 3 * s.stddev
ORDER BY time;
```

### Panel 3: Value Distribution Histograms (Histogram Panel)
```sql
-- Understand data distributions for each metric
SELECT
  metric,
  value,
  COUNT(*) as frequency
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
GROUP BY metric, value
ORDER BY metric, value;
```

### Panel 4: Sensor Drift Detection (Time Series)
```sql
-- Detect systematic bias over time (comparing to expected ranges)
WITH daily_stats AS (
  SELECT
    DATE_TRUNC('day', to_timestamp(timestamp/1000000)) as day,
    metric,
    AVG(value) as daily_mean,
    STDDEV(value) as daily_stddev,
    COUNT(*) as record_count
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1, 2
)
SELECT
  day as time,
  metric,
  daily_mean as "Daily Mean",
  daily_stddev as "Daily Std Dev",
  daily_mean - LAG(daily_mean, 7) OVER (PARTITION BY metric ORDER BY day) as "Week-over-Week Change",
  record_count as "Records"
FROM daily_stats
ORDER BY day, metric;
```

### Panel 5: Rate of Change Anomalies (Time Series)
```sql
-- Detect impossible/suspicious rapid changes
WITH changes AS (
  SELECT
    to_timestamp(timestamp/1000000) as time,
    metric,
    value,
    value - LAG(value) OVER (PARTITION BY metric ORDER BY timestamp) as change,
    (timestamp - LAG(timestamp) OVER (PARTITION BY metric ORDER BY timestamp)) / 1000000 as time_diff_sec
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
)
SELECT
  time,
  metric,
  change as "Value Change",
  time_diff_sec as "Time Diff (sec)",
  (change / NULLIF(time_diff_sec, 0)) as "Rate of Change per Second"
FROM changes
WHERE ABS(change / NULLIF(time_diff_sec, 0)) >
  CASE
    WHEN metric = 'temperature' THEN 0.5    -- > 0.5°C/sec is suspicious
    WHEN metric = 'pm02' THEN 5             -- > 5 µg/m³/sec is suspicious
    WHEN metric = 'rco2' THEN 10            -- > 10 ppm/sec is suspicious
    ELSE 999
  END
ORDER BY time;
```

### Panel 6: Cross-Stream Consistency Checks (Table)
```sql
-- Do multiple sensors measuring same thing agree?
-- Example: Temperature from AirGradient vs NWS vs OWM
WITH temp_comparison AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
    MAX(value) FILTER (WHERE location_id = 'air-quality' AND metric = 'atmp') as ag_temp,
    MAX(value) FILTER (WHERE location_id = 'nws-observations' AND metric = 'temperature') as nws_temp,
    MAX(value) FILTER (WHERE location_id = 'outdoor-weather' AND metric = 'temperature') as owm_temp
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY 1
)
SELECT
  hour as time,
  ag_temp as "AirGradient",
  nws_temp as "NWS",
  owm_temp as "OWM",
  GREATEST(ag_temp, nws_temp, owm_temp) - LEAST(ag_temp, nws_temp, owm_temp) as "Range",
  STDDEV_POP(ARRAY[ag_temp, nws_temp, owm_temp]) as "Std Dev"
FROM temp_comparison
WHERE ag_temp IS NOT NULL OR nws_temp IS NOT NULL OR owm_temp IS NOT NULL
ORDER BY time;
```

**Decision Support**:
- Outlier % > 5% suggests need for validation rules in ETL
- Systematic drift indicates sensor calibration issues
- Impossible rate-of-change values should be filtered
- Cross-stream inconsistencies guide source selection

---

## Dashboard 5: Temporal Resolution & Aggregation Planning

**Purpose**: Determine optimal aggregation windows and retention policies for Silver layer.

**Key Decisions Supported**:
- What time buckets preserve meaningful patterns?
- What raw data retention is needed?
- Which continuous aggregates to create?
- What refresh intervals for materialized views?

### Panel 1: Information Loss by Aggregation Window (Line Chart)
```sql
-- Compare variance at different aggregation levels
WITH raw_variance AS (
  SELECT
    metric,
    VARIANCE(value) as raw_var
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY metric
),
agg_1min AS (
  SELECT
    metric,
    VARIANCE(avg_value) as var_1min
  FROM (
    SELECT
      metric,
      time_bucket(INTERVAL '1 minute', to_timestamp(timestamp/1000000)) as bucket,
      AVG(value) as avg_value
    FROM read_parquet('/data/data/**/*.parquet')
    WHERE timestamp >= ${__from}::BIGINT * 1000
    GROUP BY metric, bucket
  )
  GROUP BY metric
),
agg_10min AS (
  SELECT
    metric,
    VARIANCE(avg_value) as var_10min
  FROM (
    SELECT
      metric,
      time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as bucket,
      AVG(value) as avg_value
    FROM read_parquet('/data/data/**/*.parquet')
    WHERE timestamp >= ${__from}::BIGINT * 1000
    GROUP BY metric, bucket
  )
  GROUP BY metric
),
agg_1hour AS (
  SELECT
    metric,
    VARIANCE(avg_value) as var_1hour
  FROM (
    SELECT
      metric,
      time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as bucket,
      AVG(value) as avg_value
    FROM read_parquet('/data/data/**/*.parquet')
    WHERE timestamp >= ${__from}::BIGINT * 1000
    GROUP BY metric, bucket
  )
  GROUP BY metric
)
SELECT
  r.metric,
  r.raw_var as "Raw Variance",
  m1.var_1min as "1-Min Variance",
  (1 - m1.var_1min / NULLIF(r.raw_var, 0)) * 100 as "1-Min Loss %",
  m10.var_10min as "10-Min Variance",
  (1 - m10.var_10min / NULLIF(r.raw_var, 0)) * 100 as "10-Min Loss %",
  h1.var_1hour as "1-Hour Variance",
  (1 - h1.var_1hour / NULLIF(r.raw_var, 0)) * 100 as "1-Hour Loss %"
FROM raw_variance r
JOIN agg_1min m1 ON r.metric = m1.metric
JOIN agg_10min m10 ON r.metric = m10.metric
JOIN agg_1hour h1 ON r.metric = h1.metric;
```

### Panel 2: Data Volume by Retention Strategy (Bar Chart)
```sql
-- Estimate storage needs for different strategies
WITH volume_calc AS (
  SELECT
    '10-sec raw (30 days)' as strategy,
    COUNT(*) * 30 as estimated_records
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__from}::BIGINT * 1000 + 86400000000  -- 1 day sample
  UNION ALL
  SELECT
    '1-min agg (90 days)',
    COUNT(DISTINCT time_bucket(INTERVAL '1 minute', to_timestamp(timestamp/1000000))) * 90
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__from}::BIGINT * 1000 + 86400000000
  UNION ALL
  SELECT
    '10-min agg (1 year)',
    COUNT(DISTINCT time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000))) * 365
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__from}::BIGINT * 1000 + 86400000000
  UNION ALL
  SELECT
    '1-hour agg (5 years)',
    COUNT(DISTINCT time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000))) * 1825
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
    AND timestamp <= ${__from}::BIGINT * 1000 + 86400000000
)
SELECT
  strategy,
  estimated_records as "Records",
  (estimated_records * 50 / 1024 / 1024) as "Estimated Size (MB)"  -- ~50 bytes per record
FROM volume_calc
ORDER BY estimated_records DESC;
```

### Panel 3: Peak Detection at Different Resolutions (Time Series)
```sql
-- How many peaks/events are lost with coarser granularity?
WITH peaks_raw AS (
  SELECT
    to_timestamp(timestamp/1000000) as time,
    value,
    CASE
      WHEN value > LAG(value) OVER (ORDER BY timestamp)
       AND value > LEAD(value) OVER (ORDER BY timestamp)
      THEN 1 ELSE 0
    END as is_peak
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE metric = 'pm02'
    AND timestamp >= ${__from}::BIGINT * 1000
),
peaks_10min AS (
  SELECT
    bucket as time,
    avg_value as value,
    CASE
      WHEN avg_value > LAG(avg_value) OVER (ORDER BY bucket)
       AND avg_value > LEAD(avg_value) OVER (ORDER BY bucket)
      THEN 1 ELSE 0
    END as is_peak
  FROM (
    SELECT
      time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as bucket,
      AVG(value) as avg_value
    FROM read_parquet('/data/data/air-quality/**/*.parquet')
    WHERE metric = 'pm02'
      AND timestamp >= ${__from}::BIGINT * 1000
    GROUP BY bucket
  )
)
SELECT
  time,
  value as "PM2.5 Value",
  is_peak as "Is Peak (Raw)"
FROM peaks_raw
UNION ALL
SELECT
  time,
  value,
  is_peak
FROM peaks_10min
ORDER BY time;
```

### Panel 4: Recommended Aggregation Strategy (Table)
```sql
-- Suggest optimal strategy per metric based on variance and update frequency
WITH metric_stats AS (
  SELECT
    metric,
    COUNT(*) as total_records,
    COUNT(*) / ((MAX(timestamp) - MIN(timestamp)) / 1000000 / 60) as records_per_minute,
    VARIANCE(value) as variance,
    STDDEV(value) / NULLIF(AVG(value), 0) as coefficient_of_variation
  FROM read_parquet('/data/data/**/*.parquet')
  WHERE timestamp >= ${__from}::BIGINT * 1000
  GROUP BY metric
)
SELECT
  metric,
  records_per_minute as "Records/Min",
  variance as "Variance",
  coefficient_of_variation as "CV",
  CASE
    WHEN coefficient_of_variation > 0.5 AND records_per_minute > 1
      THEN '1-minute aggregation'
    WHEN coefficient_of_variation > 0.3 OR records_per_minute > 0.5
      THEN '5-minute aggregation'
    WHEN records_per_minute > 0.1
      THEN '10-minute aggregation'
    ELSE '1-hour aggregation'
  END as "Recommended Window",
  CASE
    WHEN coefficient_of_variation > 0.5 THEN '30 days raw + 90 days agg'
    WHEN coefficient_of_variation > 0.3 THEN '7 days raw + 90 days agg'
    ELSE '1 day raw + 365 days agg'
  END as "Recommended Retention"
FROM metric_stats
ORDER BY records_per_minute DESC;
```

### Panel 5: Query Performance Simulation (Table)
```sql
-- Estimate query speed improvement with different strategies
SELECT
  'Full scan (raw data)' as query_type,
  COUNT(*) as records_scanned,
  COUNT(*) / 1000000.0 as "Estimated Query Time (sec)"
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
UNION ALL
SELECT
  '1-minute aggregates',
  COUNT(DISTINCT time_bucket(INTERVAL '1 minute', to_timestamp(timestamp/1000000))),
  COUNT(DISTINCT time_bucket(INTERVAL '1 minute', to_timestamp(timestamp/1000000))) / 1000000.0
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
UNION ALL
SELECT
  '10-minute aggregates',
  COUNT(DISTINCT time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000))),
  COUNT(DISTINCT time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000))) / 1000000.0
FROM read_parquet('/data/data/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000;
```

**Decision Support**:
- Information loss < 10% suggests aggregation is safe
- Storage projections inform infrastructure planning
- Peak detection analysis guides event-based features
- Query performance simulations justify continuous aggregates

---

## Implementation Priority

### Phase 1: Immediate (Before Silver Layer Design)
1. **Data Quality & Completeness Dashboard** - Identifies data issues to address in ETL
2. **Weather Source Reliability Dashboard** - Determines NWS vs OWM strategy

### Phase 2: Design Phase
3. **Temporal Resolution & Aggregation Planning Dashboard** - Guides schema design
4. **Anomaly Detection & Data Profiling Dashboard** - Defines validation rules

### Phase 3: Feature Engineering
5. **Indoor-Outdoor Correlation Analysis Dashboard** - Informs ML features

## Technical Notes

### DuckDB Query Optimization
- All queries use `time_bucket()` for temporal alignment
- `read_parquet()` with glob patterns for file scanning
- Filter pushdown with timestamp comparisons
- Avoid `SELECT *`, specify columns for better performance
- Use CTEs for complex multi-step queries

### Grafana Variables to Include
```json
{
  "stream_filter": {
    "type": "query",
    "query": "SELECT DISTINCT location_id FROM read_parquet('/data/data/**/*.parquet')",
    "multi": true
  },
  "metric_filter": {
    "type": "query",
    "query": "SELECT DISTINCT metric FROM read_parquet('/data/data/**/*.parquet')",
    "multi": true
  },
  "aggregation_window": {
    "type": "custom",
    "options": ["1 minute", "5 minutes", "10 minutes", "1 hour"],
    "current": "10 minutes"
  }
}
```

### Refresh Strategies
- Data Quality dashboard: 5-minute refresh (monitor ingestion health)
- Source Reliability: 1-hour refresh (trends change slowly)
- Correlation Analysis: 1-hour refresh (statistical patterns stable)
- Anomaly Detection: 5-minute refresh (catch issues quickly)
- Temporal Resolution: Manual refresh (used during design phase only)

## Success Metrics

Each dashboard should help answer specific questions:

| Dashboard | Key Question | Success Metric |
|-----------|-------------|----------------|
| Data Quality | Is our data reliable? | Completeness > 95%, Gaps < 1/day |
| Source Reliability | Which weather source is better? | Clear accuracy winner with > 10% difference |
| Correlation Analysis | What features matter? | Identified top 5 features with correlation > 0.5 |
| Anomaly Detection | Do we need validation? | Outlier rate < 1%, no systematic drift |
| Temporal Resolution | What aggregations to use? | Information loss < 10% at chosen window |

## Next Steps

1. **Create dashboard JSON files** following existing naming pattern
2. **Test queries** against actual Parquet files in `/data/data/`
3. **Validate with MotherduckDB datasource** configuration
4. **Document findings** in Silver layer ADRs
5. **Iterate based on insights** - dashboards should evolve with understanding

## Appendix: Example Dashboard JSON Structure

```json
{
  "uid": "ndp-data-quality",
  "title": "Data Quality & Completeness",
  "tags": ["ndp", "exploration", "quality"],
  "timezone": "browser",
  "refresh": "5m",
  "time": {"from": "now-7d", "to": "now"},
  "schemaVersion": 36,
  "version": 1,
  "editable": true,
  "panels": [
    // Panels as specified above
  ]
}
```

---

**Document Version**: 1.0
**Created**: 2025-12-23
**Purpose**: Inform Silver layer design with data-driven insights
**Author**: Code Analyzer Agent (Data Visualization Specialist)
