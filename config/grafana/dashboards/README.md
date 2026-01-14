# NDP Silver Layer Grafana Dashboards

Dashboard inventory for the Neural Data Platform (DP-008).

## Data Source

| Name | Type | Database | Description |
|------|------|----------|-------------|
| `timescaledb-silver` | PostgreSQL | ndp | Silver layer TimescaleDB (read-only) |

## Dashboard Inventory

| Dashboard | UID | File | Purpose |
|-----------|-----|------|---------|
| Pipeline Health | `ndp-pipeline-health` | `pipeline-health.json` | "Is everything still working?" - Operational monitoring |
| Forecast Accuracy | `ndp-forecast-accuracy-silver` | `forecast-accuracy.json` | "How much can I trust the forecast?" - NWS accuracy analysis |
| Indoor Environment | `ndp-indoor-environment` | `indoor-environment.json` | "Should I open windows?" - Indoor AQ + ventilation |

## Cross-Dashboard Features

### Temperature Unit Toggle

All dashboards include a `temp_unit` variable to switch between Celsius and Fahrenheit.

**SQL Pattern:**
```sql
CASE
  WHEN '${temp_unit}' = 'Fahrenheit'
  THEN (temperature_c * 9.0/5.0) + 32
  ELSE temperature_c
END as temperature
```

---

## Dashboard 1: Pipeline Health

**Purpose:** Operational monitoring - know at a glance if all data streams are working.

**Default Time Range:** 24h

### Panels

| Panel | Type | Description |
|-------|------|-------------|
| Stream Status Grid | Table | All streams with last record time and health status |
| Data Freshness Gauges | Gauge | Time since last record per stream |
| Record Volume (24h) | Bar | Record counts per stream |
| DQ Flag Summary | Table | Percentage of records with data quality flags |
| Ingestion Timeline | Time Series | Stacked area chart of records over time |

### Key Queries

**Stream Status (config-driven discovery):**
```sql
-- Dynamically discovers all Silver tables and their status
-- Uses UNION ALL across tables to enumerate streams
SELECT
  'air_quality_observations' as stream,
  MAX(observation_time) as last_record,
  EXTRACT(EPOCH FROM (NOW() - MAX(observation_time))) as age_seconds,
  COUNT(*) FILTER (WHERE observation_time > NOW() - INTERVAL '24 hours') as records_24h
FROM silver.air_quality_observations
UNION ALL
-- ... repeated for each Silver table
```

**Freshness Thresholds:**

| Stream | Expected Interval | Yellow (Warning) | Red (Critical) |
|--------|-------------------|------------------|----------------|
| air_quality_observations | 30 sec | >1 min | >2 min |
| weather_observations | 10 min | >20 min | >40 min |
| outdoor_air_quality | 10 min | >20 min | >40 min |
| weather_forecasts | 1 hour | >2 hours | >4 hours |

---

## Dashboard 2: Forecast Accuracy

**Purpose:** Determine how far into the future NWS forecasts remain reliable.

**Default Time Range:** 7d (need sufficient samples for statistical significance)

### Panels

| Panel | Type | Description |
|-------|------|-------------|
| Temperature MAE by Lead Time | Bar | Mean Absolute Error per lead time bucket |
| Temperature Bias | Bar | Mean Error (positive = forecast too warm) |
| Accuracy % (within 2°C) | Gauge | Percentage of forecasts within 2°C of actual |
| Forecast vs Actual Overlay | Time Series | Visual comparison of predictions vs reality |
| Wind Speed Accuracy | Bar | MAE for wind speed |
| Humidity Accuracy | Bar | MAE for humidity |
| Trustworthy Horizon | Stat | "Forecasts reliable up to X hours" |

### Lead Time Buckets

| Bucket | Range | Use Case |
|--------|-------|----------|
| 1 hour | 0-1.5h | Immediate decisions |
| 3 hours | 1.5-4.5h | Near-term planning |
| 6 hours | 4.5-9h | Same-day planning |
| 12 hours | 9-18h | Next-day planning |
| 24 hours | 18-36h | Tomorrow |
| 48 hours | 36-60h | Weekend planning |

### Key Queries

**Forecast-to-Observation Join:**
```sql
-- Join forecasts to nearest observation within ±30 minutes
-- Lead time = valid_time - issue_time
WITH forecast_obs AS (
  SELECT
    f.valid_time,
    f.issue_time,
    f.temperature_c as forecast_temp,
    o.temperature_c as observed_temp,
    EXTRACT(EPOCH FROM (f.valid_time - f.issue_time)) / 3600.0 as lead_hours
  FROM silver.weather_forecasts f
  JOIN silver.weather_observations o
    ON ABS(EXTRACT(EPOCH FROM (f.valid_time - o.observation_time))) <= 1800  -- ±30 min
  WHERE f.dq_flags IS NULL AND o.dq_flags IS NULL
)
SELECT
  CASE
    WHEN lead_hours <= 1.5 THEN '1 hour'
    WHEN lead_hours <= 4.5 THEN '3 hours'
    WHEN lead_hours <= 9 THEN '6 hours'
    WHEN lead_hours <= 18 THEN '12 hours'
    WHEN lead_hours <= 36 THEN '24 hours'
    ELSE '48 hours'
  END as lead_bucket,
  AVG(ABS(forecast_temp - observed_temp)) as mae,
  AVG(forecast_temp - observed_temp) as bias
FROM forecast_obs
GROUP BY 1
ORDER BY MIN(lead_hours)
```

**Trustworthy Horizon Calculation:**
```sql
-- Find maximum lead time where MAE < 2°C
-- Returns hours as single stat
```

---

## Dashboard 3: Indoor Environment + Ventilation

**Purpose:** Combine indoor AQ, outdoor conditions, and forecasts for actionable ventilation decisions.

**Default Time Range:** 24h

### Panels - Current Status Row

| Panel | Source Table | Key Metrics |
|-------|--------------|-------------|
| Indoor Temperature | air_quality_observations | temperature_c + 6h sparkline |
| Indoor Humidity | air_quality_observations | humidity_pct (comfort: 30-60%) |
| Indoor CO2 | air_quality_observations | co2 (green <800, yellow 800-1200, red >1200) |
| Indoor PM2.5 | air_quality_observations | pm25 (EPA thresholds: 12/35/55) |
| Outdoor Temperature | weather_observations | temperature_c + feels_like_c |
| Outdoor Humidity | weather_observations | humidity_pct |
| Outdoor AQI | outdoor_air_quality | aqi_epa (EPA category coloring) |

### Panels - Ventilation Decision

| Panel | Type | Description |
|-------|------|-------------|
| Ventilation Recommendation | Stat | "OPEN WINDOWS" (green) or "KEEP CLOSED" (red) |
| Ventilation Factors | Table | Status of each decision factor |
| Upcoming Conditions | Table | Next 6-12h forecast summary |

### Panels - Trends

| Panel | Type | Description |
|-------|------|-------------|
| Indoor vs Outdoor Temperature | Time Series | Dual line comparison |
| Indoor vs Outdoor PM2.5 | Time Series | With indoor/outdoor ratio |
| CO2 Trend (24h) | Time Series | Threshold lines at 800 and 1200 ppm |
| Humidity Comparison | Time Series | Indoor vs outdoor with 60% threshold |

### Ventilation Logic

**OPEN WINDOWS** when ALL conditions are met:

```sql
-- Ventilation recommendation query
SELECT
  CASE
    WHEN co2_ok AND temp_ok AND humidity_ok AND aqi_ok AND precip_ok
    THEN 'OPEN WINDOWS'
    ELSE 'KEEP CLOSED'
  END as recommendation
FROM (
  SELECT
    -- Would benefit from fresh air?
    (SELECT co2 > 800 FROM silver.air_quality_observations
     ORDER BY observation_time DESC LIMIT 1) as co2_ok,

    -- Comfortable outdoor temp (18-26°C / 65-79°F)?
    (SELECT temperature_c BETWEEN 18 AND 26 FROM silver.weather_observations
     ORDER BY observation_time DESC LIMIT 1) as temp_ok,

    -- Outdoor humidity acceptable (<70%)?
    (SELECT humidity_pct < 70 FROM silver.weather_observations
     ORDER BY observation_time DESC LIMIT 1) as humidity_ok,

    -- Good air quality (AQI < 50)?
    (SELECT aqi_epa < 50 FROM silver.outdoor_air_quality
     ORDER BY observation_time DESC LIMIT 1) as aqi_ok,

    -- No rain expected (<20% in next 2h)?
    (SELECT precip_probability_pct < 20 FROM silver.weather_forecasts
     WHERE valid_time > NOW() AND valid_time < NOW() + INTERVAL '2 hours'
     ORDER BY valid_time LIMIT 1) as precip_ok
) conditions
```

| Factor | Condition | Rationale |
|--------|-----------|-----------|
| Indoor CO2 | > 800 ppm | Would benefit from fresh air exchange |
| Outdoor Temp | 18-26°C (65-79°F) | Comfortable range without HVAC load |
| Outdoor Humidity | < 70% | Avoid bringing moisture indoors |
| Outdoor AQI | < 50 (Good) | EPA "Good" category - safe to breathe |
| Precipitation | < 20% (next 2h) | Don't open windows if rain expected |

---

## Silver Layer Tables Reference

| Table | Time Column | Key Metrics |
|-------|-------------|-------------|
| `silver.air_quality_observations` | observation_time | pm25, pm10, co2, temperature_c, humidity_pct, tvoc_index, nox_index |
| `silver.weather_observations` | observation_time | temperature_c, feels_like_c, dewpoint_c, humidity_pct, pressure_pa, wind_speed_kmh |
| `silver.outdoor_air_quality` | observation_time | aqi_owm, aqi_epa, pm25, pm10, co_ugm3, no2_ugm3, o3_ugm3 |
| `silver.weather_forecasts` | valid_time (+ issue_time) | temperature_c, precip_probability_pct, precip_amount_mm, humidity_pct, wind_speed_kmh |

All tables include:
- `ndp_id` - Entity identifier
- `dq_flags` - Data quality flags (NULL = clean data)

---

## Historical Note

These Silver layer dashboards (DP-008) replace 12 Bronze layer dashboards that queried DuckDB/Parquet directly. The Bronze data source is retained for potential future validation views.

**Removed Bronze Dashboards:**
- indoor-air-quality.json, indoor-vs-outdoor.json, outdoor-air-quality.json
- outdoor-conditions.json, nws-vs-owm-comparison.json, nws-forecast-accuracy.json
- anomaly-detection.json, weather-source-reliability.json, data-quality-completeness.json
- indoor-outdoor-correlation.json, nws-gridpoints-forecast.json, homeassistant-data-quality.json
