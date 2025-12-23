# Grafana Dashboard Analysis - Neural Data Platform

**Analysis Date**: 2025-12-23
**Dashboards Reviewed**: 6
**Purpose**: Identify metrics usage, comparison patterns, and exploration gaps

---

## Executive Summary

The existing dashboards provide solid single-source monitoring and basic two-way comparisons. However, there are significant gaps in multi-source correlation analysis, data quality exploration, temporal pattern analysis, and predictive comparison capabilities that would be essential for Silver layer development.

**Key Findings**:
- **Metric Coverage**: Good baseline coverage but inconsistent naming reveals data quality issues
- **Comparison Depth**: Limited to pairwise comparisons; no multi-source correlation
- **Missing Capabilities**: No data quality dashboards, gap analysis, or aggregation exploration
- **Naming Inconsistencies**: Multiple metric names for same concepts across streams

---

## Dashboard Inventory

### 1. Indoor Air Quality (`indoor-air-quality.json`)

**Purpose**: Monitor indoor environmental conditions from Aranet4 sensor
**Default Time Range**: 7 days
**Refresh**: 5 minutes

**Metrics Used**:
| Metric | Display Name | Unit | Thresholds |
|--------|--------------|------|------------|
| `pm02` | PM2.5 | conc | 12 (yellow), 35 (orange), 55 (red) |
| `rco2` | CO2 | ppm | 1000 (yellow), 2000 (red) |
| `atmp`, `temperature` | Temperature | celsius | None |
| `rhum`, `humidity` | Humidity | percent | None |

**Query Pattern**:
```sql
-- Current value (stat panels)
SELECT AVG(value) FROM (
  SELECT value FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE metric = 'pm02'
  ORDER BY timestamp DESC LIMIT 100
)

-- Time series (10-minute buckets)
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'pm02' THEN value END) as "PM2.5"
FROM read_parquet('/data/data/air-quality/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1 ORDER BY 1
```

**Observations**:
- Uses LIMIT 100 for current values (arbitrary, not time-based)
- Handles dual metric names: `atmp`/`temperature`, `rhum`/`humidity`
- 10-minute aggregation buckets

---

### 2. Indoor vs Outdoor Comparison (`indoor-vs-outdoor.json`)

**Purpose**: Compare indoor (Aranet4) vs outdoor (OpenWeatherMap) conditions
**Default Time Range**: 7 days
**Refresh**: 5 minutes

**Metrics Compared**:
| Indoor Stream | Outdoor Stream | Delta Panel |
|---------------|----------------|-------------|
| `air-quality/**` → `atmp`, `temperature` | `outdoor-weather/**` → `temperature` | Temperature Delta |
| `air-quality/**` → `pm02` | `outdoor-air-quality/**` → `pm2_5` | PM2.5 Delta |
| `air-quality/**` → `rhum`, `humidity` | `outdoor-weather/**` → `humidity` | Humidity Delta |
| `air-quality/**` → `rco2` | (none) | CO2 (indoor only) |

**Query Pattern**:
```sql
-- Delta calculation (WITH CTE pattern)
WITH indoor AS (
  SELECT AVG(value) as temp
  FROM (SELECT value FROM read_parquet('/data/data/air-quality/**/*.parquet')
        WHERE metric IN ('atmp', 'temperature')
        ORDER BY timestamp DESC LIMIT 100)
), outdoor AS (
  SELECT AVG(value) as temp
  FROM (SELECT value FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
        WHERE metric = 'temperature'
        ORDER BY timestamp DESC LIMIT 100)
)
SELECT (indoor.temp - outdoor.temp) AS "value" FROM indoor, outdoor

-- Time series comparison (FULL OUTER JOIN)
WITH indoor AS (...), outdoor AS (...)
SELECT
  COALESCE(i.bucket, o.bucket) as time,
  i.temp as "Indoor Temperature",
  o.temp as "Outdoor Temperature"
FROM indoor i FULL OUTER JOIN outdoor o ON i.bucket = o.bucket
ORDER BY time
```

**Observations**:
- First use of delta calculations (indoor - outdoor)
- FULL OUTER JOIN ensures no data loss when sources have gaps
- No timestamp alignment validation shown

---

### 3. Outdoor Air Quality (`outdoor-air-quality.json`)

**Purpose**: Monitor EPA AirNow outdoor air quality data
**Default Time Range**: 7 days
**Refresh**: 5 minutes

**Metrics Used**:
| Metric | Display | Unit | Thresholds |
|--------|---------|------|------------|
| `aqi` | AQI Gauge | - | 51 (yellow), 101 (orange), 151 (red), 201 (purple), 301 (dark-red) |
| `pm2_5` | PM2.5 | conc | 12 (yellow), 35 (orange), 55 (red) |
| `pm10` | PM10 | conc | 50 (yellow), 100 (red) |
| `no2` | NO2 | conc | - |
| `o3` | O3 (Ozone) | conc | - |
| `co` | CO | conc | - |
| `so2` | SO2 | conc | - |

**Pollutant Breakdown Panel**:
```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'pm2_5' THEN value END) as "PM2.5",
  AVG(CASE WHEN metric = 'pm10' THEN value END) as "PM10",
  AVG(CASE WHEN metric = 'no2' THEN value END) as "NO2",
  AVG(CASE WHEN metric = 'o3' THEN value END) as "O3",
  AVG(CASE WHEN metric = 'co' THEN value END) as "CO",
  AVG(CASE WHEN metric = 'so2' THEN value END) as "SO2"
FROM read_parquet('/data/data/outdoor-air-quality/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1 ORDER BY 1
```

**Observations**:
- Most comprehensive pollutant coverage
- Uses gauge visualization for AQI (better than stat panel)
- Full EPA AQI color scale implemented

---

### 4. Outdoor Weather Conditions (`outdoor-conditions.json`)

**Purpose**: Monitor OpenWeatherMap outdoor weather
**Default Time Range**: 7 days
**Refresh**: 5 minutes

**Metrics Used**:
| Metric | Display | Unit | Thresholds |
|--------|---------|------|------------|
| `temperature` | Temperature | celsius | None |
| `feels_like` | Feels Like | celsius | None |
| `wind_speed` | Wind Speed | velocityms | 5 (yellow), 10 (red) |
| `humidity` | Humidity | percent | None |
| `pressure` | Pressure | pressurehpa | - |
| `clouds` | Cloud Cover | percent | - |

**Multi-Axis Panel Example**:
```sql
-- Wind & Pressure (dual-axis)
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'wind_speed' THEN value END) as "Wind Speed",
  AVG(CASE WHEN metric = 'pressure' THEN value END) as "Pressure"
FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1 ORDER BY 1

-- Field overrides for dual-axis:
// Wind Speed: unit=velocityms, axisPlacement=left
// Pressure: unit=pressurehpa, axisPlacement=right
```

**Observations**:
- Effective use of dual-axis for related metrics
- Consistent 10-minute bucket aggregation
- `feels_like` metric tracked but not compared to actual temperature

---

### 5. NWS vs OpenWeatherMap Comparison (`nws-vs-owm-comparison.json`)

**Purpose**: Compare NWS observations vs OpenWeatherMap current conditions
**Default Time Range**: 7 days
**Refresh**: 5 minutes

**Source Comparison**:
| Metric | NWS Stream | OWM Stream | Unit Conversion |
|--------|-----------|------------|-----------------|
| Temperature | `nws-observations/**` → `temperature` | `outdoor-weather/**` → `temperature` | None (both °C) |
| Wind Speed | `nws-observations/**` → `wind_speed` | `outdoor-weather/**` → `wind_speed` | OWM: m/s → km/h (×3.6) |
| Humidity | `nws-observations/**` → `relative_humidity` | `outdoor-weather/**` → `humidity` | None |
| Pressure | `nws-observations/**` → `barometric_pressure` | `outdoor-weather/**` → `pressure` | NWS: Pa → hPa (÷100) |

**Unit Conversion Examples**:
```sql
-- Wind Speed: OWM m/s → km/h
owm AS (
  SELECT AVG(value * 3.6) as wind
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  WHERE metric = 'wind_speed' ...
)

-- Pressure: NWS Pa → hPa
nws AS (
  SELECT AVG(value/100) as pressure
  FROM read_parquet('/data/data/nws-observations/**/*.parquet')
  WHERE metric = 'barometric_pressure' ...
)
```

**Naming Inconsistencies Revealed**:
- NWS: `relative_humidity` vs OWM: `humidity`
- NWS: `barometric_pressure` (Pa) vs OWM: `pressure` (hPa)
- Different base units require conversion

---

### 6. NWS Forecast Accuracy (`nws-forecast-accuracy.json`)

**Purpose**: Validate NWS forecast accuracy against actual observations
**Default Time Range**: 4 hours (shorter for recent forecast evaluation)
**Refresh**: 5 minutes

**Metrics Compared**:
| Forecast Stream | Observation Stream | Metric Alignment |
|----------------|-------------------|------------------|
| `nws-forecast-hourly/**` → `temperature` (°F) | `nws-observations/**` → `temperature` (°C) | Convert forecast: `(value-32)*5/9` |
| `nws-forecast-hourly/**` → `wind_speed` (mph) | `nws-observations/**` → `wind_speed` (km/h) | Convert forecast: `value * 1.60934` |
| `nws-forecast-hourly/**` → `relative_humidity` (%) | `nws-observations/**` → `relative_humidity` (%) | Direct comparison |

**Advanced Analysis Queries**:

**1. Forecast Lead Time Analysis**:
```sql
WITH forecast AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(f.timestamp/1000000)) as valid_hour,
    (f.value-32)*5/9 as temp_c,
    i.value as issue_time,
    CASE
      WHEN i.value IS NOT NULL
      THEN (f.timestamp/1000000 - i.value) / 3600
      ELSE NULL
    END as lead_hours
  FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet') f
  LEFT JOIN read_parquet('/data/data/nws-forecast-hourly/**/*.parquet') i
    ON f.timestamp = i.timestamp
    AND f.location_id = i.location_id
    AND i.metric = 'forecast_issue_time'
  WHERE f.metric = 'temperature' ...
),
observations AS (...)
SELECT
  CASE
    WHEN lead_hours < 3 THEN '0-3h'
    WHEN lead_hours < 6 THEN '3-6h'
    WHEN lead_hours < 12 THEN '6-12h'
    WHEN lead_hours < 24 THEN '12-24h'
    WHEN lead_hours < 48 THEN '24-48h'
    ELSE '48h+'
  END as "Lead Time",
  AVG(ABS(f.temp_c - o.temp_c)) as "MAE (°C)"
FROM forecast f
INNER JOIN observations o ON f.valid_hour = o.hour
WHERE lead_hours IS NOT NULL
GROUP BY 1
ORDER BY CASE ... END  -- Custom ordering
```

**2. Accuracy Percentage (within 2°C threshold)**:
```sql
SELECT
  /* Lead Time bucketing */,
  (COUNT(*) FILTER (WHERE ABS(f.temp_c - o.temp_c) <= 2.0) * 100.0
   / NULLIF(COUNT(*), 0)) as "Accuracy %"
FROM forecast f
INNER JOIN observations o ON f.valid_hour = o.hour
WHERE lead_hours IS NOT NULL
GROUP BY 1
```

**Observations**:
- Most sophisticated dashboard with MAE (Mean Absolute Error) calculations
- Uses `forecast_issue_time` metadata for lead time analysis
- Bar chart visualization for lead time vs accuracy
- Error bars show signed error (forecast - actual)
- 1-hour bucket aggregation (finer than other dashboards)
- **Critical**: Assumes `forecast_issue_time` exists in stream data

---

## Metric Naming Inconsistencies

### Cross-Stream Naming Problems

| Concept | Stream A | Stream B | Issue |
|---------|----------|----------|-------|
| **Temperature** | `air-quality`: `atmp`, `temperature` | `outdoor-weather`: `temperature` | Dual names in Aranet4 |
| **Humidity** | `air-quality`: `rhum`, `humidity` | `outdoor-weather`: `humidity` | Dual names in Aranet4 |
|  | `nws-observations`: `relative_humidity` | `outdoor-weather`: `humidity` | Different field names |
| **PM2.5** | `air-quality`: `pm02` | `outdoor-air-quality`: `pm2_5` | Different naming conventions |
| **Pressure** | `nws-observations`: `barometric_pressure` (Pa) | `outdoor-weather`: `pressure` (hPa) | Different units + names |
| **Wind Speed** | `nws-forecast`: mph | `nws-observations`: km/h | Different units in same source |
|  | `outdoor-weather`: m/s | `nws-observations`: km/h | Different units across sources |

**Impact**:
- Requires `IN ('metric1', 'metric2')` clauses in queries
- Unit conversions scattered across dashboards
- Difficult to generalize queries
- **Silver Layer Opportunity**: Normalize metric names and units

---

## Query Patterns Summary

### 1. Current Value Pattern (Last 100 Records)
```sql
SELECT AVG(value) as value
FROM (
  SELECT value FROM read_parquet('/data/data/{stream}/**/*.parquet')
  WHERE metric = '{metric_name}'
  ORDER BY timestamp DESC
  LIMIT 100
)
```
**Issues**:
- Arbitrary LIMIT 100 (could be 10 minutes or 10 hours depending on frequency)
- No time-based windowing (e.g., "last 5 minutes")

### 2. Time Series Aggregation (10-Minute Buckets)
```sql
SELECT
  time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = '{name}' THEN value END) as "{label}"
FROM read_parquet('/data/data/{stream}/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1 ORDER BY 1
```
**Issues**:
- Hardcoded 10-minute interval (not dynamic based on time range)
- No handling of sparse/missing data

### 3. Delta Calculation (CTE + Simple Subtraction)
```sql
WITH source_a AS (...), source_b AS (...)
SELECT (a.value - b.value) AS "delta"
FROM source_a a, source_b b
```
**Issues**:
- No timestamp alignment validation
- Assumes both sources have recent data

### 4. Two-Way Comparison (FULL OUTER JOIN)
```sql
WITH source_a AS (...), source_b AS (...)
SELECT
  COALESCE(a.bucket, b.bucket) as time,
  a.metric as "Source A",
  b.metric as "Source B"
FROM source_a a FULL OUTER JOIN source_b b ON a.bucket = b.bucket
ORDER BY time
```
**Strengths**: Handles gaps in either source
**Issues**: No gap visualization or data quality indicators

### 5. Forecast Accuracy (Complex JOIN + Metadata)
```sql
-- Requires forecast_issue_time metadata
LEFT JOIN ... ON ... AND i.metric = 'forecast_issue_time'
-- Lead time calculation
(f.timestamp/1000000 - i.value) / 3600 as lead_hours
-- Bucketed aggregation
CASE WHEN lead_hours < 3 THEN '0-3h' ... END
```
**Strengths**: Sophisticated statistical analysis
**Dependencies**: Requires metadata fields in stream

---

## What's MISSING: Critical Gaps for Data Exploration

### 1. Multi-Source Correlation Analysis

**Missing**:
- No 3+ source comparisons (e.g., Indoor vs NWS vs OWM vs AirNow simultaneously)
- No correlation coefficient calculations
- No lag/lead analysis (e.g., "does indoor PM2.5 spike 30min after outdoor?")

**Use Case**:
```
"When outdoor PM2.5 from AirNow exceeds 35 µg/m³, how long until indoor
 PM2.5 from Aranet4 rises above 12 µg/m³? Does OWM visibility data correlate?"
```

**What Would Help**:
```sql
-- Multi-source correlation dashboard
WITH indoor AS (...),
     outdoor_aq AS (...),
     outdoor_weather AS (...)
SELECT
  time,
  indoor.pm25 as "Indoor PM2.5",
  outdoor_aq.pm25 as "AirNow PM2.5",
  outdoor_weather.visibility as "Visibility",
  -- Correlation coefficient per window
  corr(indoor.pm25, outdoor_aq.pm25) OVER (
    ORDER BY time ROWS BETWEEN 12 PRECEDING AND CURRENT ROW
  ) as "Indoor-Outdoor Correlation (2hr window)"
```

---

### 2. Data Quality & Freshness Monitoring

**Missing**:
- No "last update" timestamps per stream
- No data gap detection/visualization
- No source comparison of update frequency
- No schema validation errors visible

**Use Case**:
```
"nws-forecast-hourly hasn't updated in 3 hours - is the source down or is
 there a parsing error? Which streams are currently stale?"
```

**What Would Help**:
- **Stream Health Dashboard**:
  - Last successful update per stream
  - Records/hour rate over time
  - Gap detection (expected vs actual intervals)
  - Parse error counts
- **Data Completeness Heatmap**:
  - Grid: Stream × Metric, color-coded by % completeness per day
  - Missing metric fields highlighted

---

### 3. Temporal Pattern Analysis

**Missing**:
- No hourly/daily/weekly seasonality views
- No "same time yesterday/last week" comparisons
- No anomaly detection overlays
- No moving averages or trend lines

**Use Case**:
```
"Indoor CO2 peaks at 8pm on weekdays but 10am on weekends. Does this
 correlate with occupancy patterns?"
```

**What Would Help**:
```sql
-- Daily pattern comparison
SELECT
  EXTRACT(HOUR FROM time) as hour_of_day,
  EXTRACT(DOW FROM time) as day_of_week,
  AVG(value) as avg_co2,
  STDDEV(value) as stddev_co2
FROM ...
GROUP BY 1, 2
-- Visualized as heatmap: hour × day_of_week
```

---

### 4. Aggregation Interval Exploration

**Missing**:
- All dashboards use 10-minute buckets (hardcoded)
- No dynamic aggregation based on time range
- No comparison of aggregation effects (raw vs 10min vs 1hr vs 1day)

**Use Case**:
```
"How much detail do we lose at 1-hour aggregation vs 10-minute?
 What's the optimal interval for 30-day views?"
```

**What Would Help**:
- **Aggregation Comparison Panel**:
  - Same metric, multiple aggregation levels side-by-side
  - Show: Raw (if feasible), 10min, 1hr, 6hr, 1day
  - Highlight: Min/Max/Avg differences per level

---

### 5. Unit Conversion Transparency

**Missing**:
- Conversions hidden in SQL queries
- No visual indication which sources required conversion
- No validation that conversions are correct

**Use Case**:
```
"Why is NWS wind speed consistently higher than OWM?
 Oh wait, is the m/s → km/h conversion factor correct?"
```

**What Would Help**:
- **Unit Conversion Dashboard**:
  - Show original units alongside converted values
  - Highlight which sources needed conversion
  - Add conversion validation (e.g., "OWM 5 m/s = 18 km/h ✓")

---

### 6. Forecast Model Comparison

**Missing** (beyond current NWS accuracy dashboard):
- No comparison between NWS vs OWM forecasts
- No multi-model ensemble views
- No probabilistic forecast ranges (if available in source data)

**Use Case**:
```
"NWS predicts rain, OWM predicts clear. Which is more reliable for
 our location based on historical accuracy?"
```

**What Would Help**:
```sql
-- Compare NWS forecast vs OWM forecast vs actual
WITH nws_fc AS (...),
     owm_fc AS (...),
     actual AS (...)
SELECT
  time,
  nws_fc.temp as "NWS Forecast",
  owm_fc.temp as "OWM Forecast",
  actual.temp as "Actual",
  ABS(nws_fc.temp - actual.temp) as "NWS Error",
  ABS(owm_fc.temp - actual.temp) as "OWM Error"
```

---

### 7. Metadata Exploration

**Missing**:
- No dashboard showing available metrics per stream
- No data dictionary or schema viewer
- No "what fields are populated?" exploratory queries

**Use Case**:
```
"I want to compare dew point across sources. Which streams have
 'dew_point' as a metric? What's the field name variation?"
```

**What Would Help**:
- **Stream Metadata Dashboard**:
```sql
-- List all unique metrics per stream
SELECT
  regexp_extract(filename, '/data/data/([^/]+)/', 1) as stream_id,
  metric,
  COUNT(*) as record_count,
  MIN(timestamp) as first_seen,
  MAX(timestamp) as last_seen
FROM read_parquet('/data/data/**/*.parquet')
GROUP BY 1, 2
ORDER BY 1, 3 DESC
```
  - Visualized as table with stream × metric grid
  - Shows which metrics are available per stream

---

### 8. Outlier & Spike Detection

**Missing**:
- No automated outlier highlighting
- No "events" timeline (e.g., "PM2.5 spike on 2025-12-20 14:30")
- No threshold exceedance history

**Use Case**:
```
"Show me all times in the last 30 days when indoor CO2 exceeded 2000ppm
 for >1 hour, with outdoor weather conditions at those times."
```

**What Would Help**:
- **Events Table**:
```sql
-- Detect CO2 exceedances
WITH high_co2_windows AS (
  SELECT
    MIN(time) as start_time,
    MAX(time) as end_time,
    AVG(value) as avg_co2
  FROM (
    SELECT
      time,
      value,
      -- Create groups where CO2 > 2000 continuously
      SUM(CASE WHEN value > 2000 THEN 0 ELSE 1 END)
        OVER (ORDER BY time) as group_id
    FROM ...
  )
  WHERE value > 2000
  GROUP BY group_id
  HAVING MAX(time) - MIN(time) > INTERVAL '1 hour'
)
SELECT * FROM high_co2_windows
```
  - Display as timeline with annotations on time series panels

---

### 9. Statistical Summaries

**Missing**:
- No percentile calculations (P50, P95, P99)
- No distribution histograms
- No min/max range bands

**Use Case**:
```
"What's the 95th percentile of outdoor PM2.5 over the last year?
 How often do we exceed it?"
```

**What Would Help**:
```sql
-- Percentile calculation
SELECT
  date_trunc('week', time) as week,
  approx_percentile(value, 0.5) as p50_pm25,
  approx_percentile(value, 0.95) as p95_pm25,
  MAX(value) as max_pm25
FROM ...
GROUP BY 1
```
  - Visualized with shaded percentile bands on time series

---

### 10. Cross-Dashboard Navigation

**Missing**:
- No drill-down links between dashboards
- No shared time range synchronization across dashboards
- No dashboard variables (e.g., select stream, select metric)

**What Would Help**:
- Dashboard variables:
  - `$stream_id` dropdown (air-quality, outdoor-weather, nws-observations, etc.)
  - `$metric_name` dropdown (filtered by selected stream)
  - `$aggregation_interval` (10min, 1hr, 1day)
- Drill-down links:
  - Click "PM2.5 Delta" stat → navigate to detailed comparison view with that time range

---

## Silver Layer Implications

### What the Dashboards Tell Us About Silver Layer Requirements

**1. Normalization Needs**:
- Metric name standardization (e.g., all humidity → `humidity`, not `rhum` or `relative_humidity`)
- Unit standardization (all temps in °C, all wind in m/s or km/h consistently)
- Timestamp alignment (ensure all sources use same microsecond epoch format)

**2. Pre-Aggregation Opportunities**:
- 10-minute buckets are universal → create materialized rollups
- Common calculations (deltas, MAE) → store as computed columns
- Forecast lead time → pre-calculate and store with forecasts

**3. Metadata Requirements**:
- Stream health metrics (last_update, records_per_hour, gap_count)
- Forecast issue time (already needed by forecast accuracy dashboard)
- Data quality scores (completeness %, outlier count)

**4. Missing Tables/Views**:
- `stream_health` - real-time freshness monitoring
- `metric_catalog` - available metrics per stream
- `forecast_accuracy_history` - pre-calculated MAE by lead time
- `correlation_matrix` - pre-calculated cross-stream correlations

**5. Query Performance Insights**:
- All dashboards use `read_parquet('/data/data/**/*.parquet')` → full scans
- 7-day time ranges are common → optimize for weekly rollups
- FULL OUTER JOIN pattern → ensure proper indexing on time buckets

---

## Recommendations for Data Exploration Gaps

### Immediate Quick Wins

**1. Stream Health Dashboard** (2-3 hours)
- Last update per stream (simple MAX(timestamp))
- Records per hour trend
- Visual "traffic light" status indicators

**2. Multi-Source Overlay** (3-4 hours)
- Single time series panel with 4-5 key metrics from different streams
- Use dual/triple Y-axes
- Add correlation coefficient annotation

**3. Aggregation Explorer** (2-3 hours)
- Same metric at 10min, 1hr, 6hr, 1day intervals
- Side-by-side comparison
- Show % data reduction at each level

### Medium-Term Enhancements

**4. Data Quality Heatmap** (1-2 days)
- Grid: Stream × Day (last 30 days)
- Color: % completeness (expected records vs actual)
- Click to drill into gap details

**5. Anomaly Detection Timeline** (2-3 days)
- Detect statistical outliers (>3σ)
- Annotate on existing time series panels
- Create events table for historical anomalies

**6. Forecast Ensemble Dashboard** (2-3 days)
- Compare NWS vs OWM forecasts
- Show accuracy metrics per source
- Highlight best-performing model for location

### Long-Term Strategic

**7. Predictive Comparison** (1 week)
- ML-based forecast from historical indoor data
- Compare to external forecasts
- "If outdoor PM2.5 is X, predict indoor PM2.5 in 30min"

**8. Seasonal Pattern Analysis** (1 week)
- Hourly/daily/weekly/monthly heatmaps
- Anomaly detection based on seasonal norms
- "This week vs same week last year"

**9. Silver Layer Query Optimizer** (1-2 weeks)
- Replace Bronze Parquet queries with Silver TimescaleDB
- Continuous aggregates for 10min, 1hr, 1day
- Hypertable compression for older data

---

## Dashboard-Specific Recommendations

### Indoor Air Quality
**Add**:
- Ventilation events annotation (if available)
- Historical CO2 peak times (hourly heatmap)
- "Same time yesterday" overlay

### Indoor vs Outdoor Comparison
**Add**:
- Lag analysis (e.g., "outdoor spike → indoor spike +30min")
- Filtration effectiveness metric (PM2.5 outdoor/indoor ratio)
- Correlation coefficient trend

### Outdoor Air Quality
**Add**:
- AQI forecast (if available from EPA)
- Dominant pollutant indicator (what's driving AQI?)
- Historical AQI distribution (percentiles)

### Outdoor Conditions
**Add**:
- "Feels like" delta (feels_like - temperature)
- Wind chill / heat index calculations
- Dew point (if available)

### NWS vs OWM Comparison
**Add**:
- "Winner" badge (which source is closer to NWS observations?)
- Historical accuracy trend (rolling 7-day MAE)
- Bias detection (is OWM consistently warmer?)

### NWS Forecast Accuracy
**Add**:
- Forecast vs OWM forecast comparison
- Probabilistic forecast ranges (if available)
- Accuracy by time of day (e.g., "forecasts degrade after midnight")

---

## Conclusion

The existing dashboards provide a solid foundation for single-source monitoring and basic pairwise comparisons. However, significant exploration gaps exist in:

1. **Multi-source correlation** - No 3+ source analysis
2. **Data quality visibility** - No gap detection or freshness monitoring
3. **Statistical depth** - Limited percentiles, distributions, or anomaly detection
4. **Temporal patterns** - No seasonality or "same time last week" views
5. **Metadata exploration** - No schema/metric discovery tools

**For Silver Layer development**, addressing these gaps will require:
- Normalized metric names and units
- Pre-aggregated rollup tables
- Stream health monitoring infrastructure
- Metadata catalogs

**Priority**: Build the **Stream Health Dashboard** and **Multi-Source Overlay** first to gain immediate visibility into data quality issues that will inform Silver layer schema design.
