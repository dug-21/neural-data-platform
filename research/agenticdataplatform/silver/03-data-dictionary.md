# Silver Layer Data Dictionary

**Document**: 03-data-dictionary.md
**Version**: 1.0
**Date**: 2026-01-05
**Author**: NDP Analytics Engineer
**Status**: Draft - Ready for Review

---

## Executive Summary

This document defines the typed column schemas for Silver layer tables in the Neural Data Platform. The Silver layer transforms raw Bronze JSON payloads into structured, queryable TimescaleDB tables optimized for analytics.

**Key Principles**:
1. **Domain-Driven Design**: Schema reflects domain model (observations vs forecasts), not API structure
2. **Standardized Units**: All values converted to SI units with consistent naming
3. **Computed Fields**: Lead time, AQI, and comfort indices calculated at query/materialization time
4. **DQ Integration**: Every table includes `dq_flags` array for transparency
5. **Efficient Indexing**: Hypertables with time-based partitioning and dimension indexes

---

## Table of Contents

1. [Data Type Standards](#1-data-type-standards)
2. [Unit Standardization](#2-unit-standardization)
3. [Air Quality Tables](#3-air-quality-tables)
4. [Weather Observation Tables](#4-weather-observation-tables)
5. [Weather Forecast Tables](#5-weather-forecast-tables)
6. [Outdoor Air Quality Tables](#6-outdoor-air-quality-tables)
7. [Analytics Views](#7-analytics-views)
8. [Computed Fields](#8-computed-fields)
9. [Indexing Strategy](#9-indexing-strategy)
10. [NULL Handling Policy](#10-null-handling-policy)

---

## 1. Data Type Standards

### 1.1 Numeric Type Selection

| Use Case | PostgreSQL Type | Rationale |
|----------|-----------------|-----------|
| Temperatures | `DOUBLE PRECISION` | Decimal precision for sub-degree accuracy |
| Percentages (0-100) | `DOUBLE PRECISION` | Sensors report decimal percentages |
| Particle counts | `DOUBLE PRECISION` | Sensors report fractional counts |
| Concentrations (PM, gases) | `DOUBLE PRECISION` | Continuous measurements |
| Indexes (AQI, TVOC) | `SMALLINT` | Bounded integer ranges (0-500) |
| CO2 (ppm) | `SMALLINT` | Integer ppm, max ~10000 |
| Pressure (Pa) | `DOUBLE PRECISION` | Large values with decimals |
| Visibility (m) | `DOUBLE PRECISION` | Continuous measurement |
| Direction (degrees) | `DOUBLE PRECISION` | 0-360 with decimals |
| Speed (km/h) | `DOUBLE PRECISION` | Continuous with decimals |
| Lead time | `INTEGER` | Hours as whole numbers |
| Boot/count | `INTEGER` | Device counters |

### 1.2 Temporal Types

| Use Case | PostgreSQL Type | Rationale |
|----------|-----------------|-----------|
| Timestamps | `TIMESTAMPTZ` | Always store with timezone (UTC) |
| Durations | `INTERVAL` | Native PostgreSQL interval support |
| Derived intervals | `INTEGER` | Hours/minutes for indexing (lead_time_hours) |

### 1.3 Text Types

| Use Case | PostgreSQL Type | Rationale |
|----------|-----------------|-----------|
| Identifiers | `TEXT` | Variable-length, no padding |
| DQ flags | `TEXT[]` | Array of rule names |
| JSON context | `JSONB` | Queryable JSON with compression |

---

## 2. Unit Standardization

### 2.1 Standard Units (SI-Based)

All Silver layer columns use standardized units. ETL transforms source units to these targets:

| Measurement | Silver Unit | Column Suffix | Conversion Notes |
|-------------|-------------|---------------|------------------|
| Temperature | Celsius | `_c` | NWS: already C, OWM: Kelvin - 273.15 |
| Humidity | Percent | `_pct` | 0-100 scale |
| Pressure | Pascals | `_pa` | NWS: Pa, OWM: hPa * 100 |
| Wind Speed | km/h | `_kmh` | NWS: km/h, OWM: m/s * 3.6 |
| Direction | Degrees | `_deg` | 0-360, meteorological convention |
| Visibility | Meters | `_m` | NWS: m, OWM: m |
| Precipitation | mm | `_mm` | NWS: mm, OWM: mm |
| PM Concentration | ug/m3 | (no suffix) | Standard AQ unit |
| CO2 | ppm | (no suffix) | Parts per million |
| Gas Index | unitless | `_index` | 1-500 scale (SGP41) |

### 2.2 Source-to-Silver Unit Mapping

| Source | Field | Source Unit | Silver Column | Target Unit |
|--------|-------|-------------|---------------|-------------|
| NWS | temperature | `wmoUnit:degC` | `temperature_c` | Celsius |
| NWS | windSpeed | `wmoUnit:km_h-1` | `wind_speed_kmh` | km/h |
| NWS | barometricPressure | `wmoUnit:Pa` | `pressure_pa` | Pascals |
| OWM | main.temp | Kelvin | `temperature_c` | Celsius |
| OWM | wind.speed | m/s | `wind_speed_kmh` | km/h |
| OWM | main.pressure | hPa | `pressure_pa` | Pascals |
| AirGradient | atmp | Celsius | `temperature_c` | Celsius |
| AirGradient | pm02 | ug/m3 | `pm25` | ug/m3 |
| AirGradient | rco2 | ppm | `co2` | ppm |

---

## 3. Air Quality Tables

### 3.1 silver.air_quality_observations

Indoor air quality measurements from AirGradient sensors.

```sql
CREATE TABLE silver.air_quality_observations (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    -- Device Context (denormalized for query performance)
    device_serial       TEXT,
    device_model        TEXT,
    firmware_version    TEXT,
    location_type       TEXT,          -- 'indoor' or 'outdoor'
    location_path       TEXT,          -- e.g., '/beachhouse/livingroom'

    -- Core Air Quality Metrics
    co2                 SMALLINT,      -- ppm (380-10000)
    pm1                 DOUBLE PRECISION,  -- ug/m3
    pm25                DOUBLE PRECISION,  -- ug/m3 (primary PM metric)
    pm25_compensated    DOUBLE PRECISION,  -- ug/m3 (temp/humidity adjusted)
    pm10                DOUBLE PRECISION,  -- ug/m3

    -- Gas Sensors (SGP41)
    tvoc_index          SMALLINT,      -- 1-500 (relative index)
    nox_index           SMALLINT,      -- 1-500 (relative index)

    -- Environmental
    temperature_c       DOUBLE PRECISION,  -- Celsius
    temperature_c_compensated DOUBLE PRECISION, -- Device-heat compensated
    humidity_pct        DOUBLE PRECISION,  -- 0-100%
    humidity_pct_compensated DOUBLE PRECISION, -- Device-heat compensated

    -- Device Diagnostics (optional)
    wifi_signal_dbm     SMALLINT,      -- dBm (negative values)
    boot_count          INTEGER,

    -- DQ Transparency
    dq_flags            TEXT[],        -- Array of rule violations

    -- Primary Key for hypertable
    PRIMARY KEY (observation_time, ndp_id)
);

-- Create hypertable partitioned by observation_time
SELECT create_hypertable('silver.air_quality_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);

-- Secondary indexes
CREATE INDEX idx_aq_ndp_id ON silver.air_quality_observations (ndp_id, observation_time DESC);
CREATE INDEX idx_aq_location ON silver.air_quality_observations (location_path, observation_time DESC);
CREATE INDEX idx_aq_dq_flags ON silver.air_quality_observations USING GIN (dq_flags) WHERE dq_flags IS NOT NULL;

COMMENT ON TABLE silver.air_quality_observations IS
    'Indoor air quality measurements from AirGradient sensors.
     Source: air-quality Bronze stream (MQTT).
     Grain: One row per sensor reading (~1 minute intervals).';
```

### 3.2 Column Rationale

| Column | Type Rationale | NULL Policy |
|--------|----------------|-------------|
| `co2` | SMALLINT - max 10000 ppm, always integer | NULL if sensor reports null/invalid |
| `pm25` | DOUBLE PRECISION - fractional ug/m3 | NOT NULL - primary metric |
| `pm25_compensated` | DOUBLE PRECISION - more accurate indoor | NULL if compensation unavailable |
| `tvoc_index` | SMALLINT - 1-500 bounded | NULL if sensor warming up |
| `temperature_c_compensated` | DOUBLE PRECISION - preferred for analysis | NULL if compensation unavailable |
| `wifi_signal_dbm` | SMALLINT - small negative integers | NULL - diagnostic only |

---

## 4. Weather Observation Tables

### 4.1 silver.weather_observations

Ground truth weather measurements from NWS stations.

```sql
CREATE TABLE silver.weather_observations (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    -- Station Context
    station_id          TEXT,          -- e.g., 'KSGJ'
    station_name        TEXT,          -- Human-readable name
    station_elevation_m DOUBLE PRECISION,

    -- Core Weather Metrics (aligned with forecast schema)
    temperature_c       DOUBLE PRECISION,
    dewpoint_c          DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,

    -- Wind
    wind_speed_kmh      DOUBLE PRECISION,
    wind_direction_deg  DOUBLE PRECISION,
    wind_gust_kmh       DOUBLE PRECISION,

    -- Pressure
    pressure_pa         DOUBLE PRECISION,  -- Barometric pressure
    sea_level_pressure_pa DOUBLE PRECISION,

    -- Visibility/Cloud
    visibility_m        DOUBLE PRECISION,
    cloud_cover_pct     DOUBLE PRECISION,  -- Derived from cloudLayers
    ceiling_height_m    DOUBLE PRECISION,

    -- Derived Comfort Metrics (NULL when conditions don't apply)
    heat_index_c        DOUBLE PRECISION,  -- NULL when temp < 27C
    wind_chill_c        DOUBLE PRECISION,  -- NULL when temp > 10C

    -- Precipitation
    precip_1h_mm        DOUBLE PRECISION,
    precip_3h_mm        DOUBLE PRECISION,
    precip_6h_mm        DOUBLE PRECISION,

    -- 24-hour Extremes
    max_temp_24h_c      DOUBLE PRECISION,
    min_temp_24h_c      DOUBLE PRECISION,

    -- Qualitative
    text_description    TEXT,          -- 'Cloudy', 'Mostly Clear', etc.

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable('silver.weather_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);

CREATE INDEX idx_weather_obs_ndp ON silver.weather_observations (ndp_id, observation_time DESC);
CREATE INDEX idx_weather_obs_station ON silver.weather_observations (station_id, observation_time DESC);

COMMENT ON TABLE silver.weather_observations IS
    'Ground truth weather observations from NWS stations.
     Source: nws-observations Bronze stream.
     Grain: One row per observation (~10-minute intervals).
     Use: Join with forecasts for accuracy analysis.';
```

### 4.2 Column Alignment with Forecasts

Weather observations and forecasts share column naming for easy comparison:

| Observation Column | Forecast Column | Join Compatibility |
|--------------------|-----------------|-------------------|
| `temperature_c` | `temperature_c` | Direct comparison |
| `humidity_pct` | `humidity_pct` | Direct comparison |
| `wind_speed_kmh` | `wind_speed_kmh` | Direct comparison |
| `wind_direction_deg` | `wind_direction_deg` | Direct comparison |
| `visibility_m` | `visibility_m` | Direct comparison |
| `precip_*_mm` | `precip_prob_pct` | Forecast is probability |

---

## 5. Weather Forecast Tables

### 5.1 silver.weather_forecasts

Core forecast metrics from NWS gridpoints.

```sql
CREATE TABLE silver.weather_forecasts (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- DOMAIN KEYS (Critical for forecast evaluation)
    issue_time          TIMESTAMPTZ NOT NULL,  -- When NWS generated this forecast
    valid_time          TIMESTAMPTZ NOT NULL,  -- When prediction applies
    valid_duration      INTERVAL,              -- How long valid (PT1H, PT6H, etc.)

    -- Computed: Essential for analysis
    lead_time_hours     INTEGER GENERATED ALWAYS AS
                        (EXTRACT(EPOCH FROM valid_time - issue_time) / 3600)::INTEGER STORED,

    -- Location identifiers
    ndp_id              TEXT NOT NULL,
    grid_office         TEXT,          -- 'JAX', 'MLB', etc.
    grid_x              SMALLINT,
    grid_y              SMALLINT,

    -- Core Temperature Metrics
    temperature_c       DOUBLE PRECISION,
    dewpoint_c          DOUBLE PRECISION,
    max_temperature_c   DOUBLE PRECISION,
    min_temperature_c   DOUBLE PRECISION,
    apparent_temperature_c DOUBLE PRECISION,
    heat_index_c        DOUBLE PRECISION,
    wind_chill_c        DOUBLE PRECISION,
    wet_bulb_globe_temp_c DOUBLE PRECISION,

    -- Humidity
    humidity_pct        DOUBLE PRECISION,

    -- Wind Metrics
    wind_speed_kmh      DOUBLE PRECISION,
    wind_direction_deg  DOUBLE PRECISION,
    wind_gust_kmh       DOUBLE PRECISION,

    -- Precipitation
    precip_prob_pct     DOUBLE PRECISION,  -- Probability of precipitation
    precip_amount_mm    DOUBLE PRECISION,  -- Quantitative precipitation forecast
    snowfall_mm         DOUBLE PRECISION,
    ice_accumulation_mm DOUBLE PRECISION,

    -- Sky/Visibility
    sky_cover_pct       DOUBLE PRECISION,
    visibility_m        DOUBLE PRECISION,
    ceiling_height_m    DOUBLE PRECISION,

    -- Thunderstorm
    thunder_prob_pct    DOUBLE PRECISION,

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (issue_time, valid_time, ndp_id)
);

SELECT create_hypertable('silver.weather_forecasts',
    'valid_time',
    chunk_time_interval => INTERVAL '1 day'
);

-- Critical index for lead_time analysis
CREATE INDEX idx_forecasts_lead_time ON silver.weather_forecasts (lead_time_hours, valid_time);

-- Index for joining with observations
CREATE INDEX idx_forecasts_valid_ndp ON silver.weather_forecasts (valid_time, ndp_id);

-- Index for issue_time queries (when was forecast generated?)
CREATE INDEX idx_forecasts_issue ON silver.weather_forecasts (issue_time DESC);

COMMENT ON TABLE silver.weather_forecasts IS
    'Weather forecasts from NWS gridpoints API.
     Source: nws-gridpoints-forecast Bronze stream.
     Grain: One row per (issue_time, valid_time, location).
     Key Analysis: Join with observations on valid_time = observation_time for accuracy evaluation.';
```

### 5.2 silver.weather_forecast_extended

Sparse/specialized metrics stored in tall format.

```sql
CREATE TABLE silver.weather_forecast_extended (
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    issue_time          TIMESTAMPTZ NOT NULL,
    valid_time          TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    metric_name         TEXT NOT NULL,
    value               DOUBLE PRECISION,

    dq_flags            TEXT[],

    PRIMARY KEY (valid_time, ndp_id, metric_name)
);

SELECT create_hypertable('silver.weather_forecast_extended',
    'valid_time',
    chunk_time_interval => INTERVAL '7 days'
);

COMMENT ON TABLE silver.weather_forecast_extended IS
    'Extended forecast metrics (fire weather, marine, etc.) stored in tall format.
     Metrics: hainesIndex, davisStabilityIndex, redFlagThreatIndex, mixingHeight,
              dispersionIndex, waveHeight, wavePeriod, waveDirection, primarySwellHeight, etc.
     Use: Research and specialized applications where these metrics are needed.';
```

### 5.3 Extended Metrics Reference

| Metric Name | Description | Valid Range | Use Case |
|-------------|-------------|-------------|----------|
| `haines_index` | Fire weather severity | 2-6 | Wildfire risk |
| `davis_stability_index` | Atmospheric stability | varies | Smoke dispersion |
| `red_flag_threat_index` | Fire danger index | 0-100 | Fire warnings |
| `mixing_height` | Height of mixed layer (m) | 0-5000 | Air quality |
| `dispersion_index` | Pollution dispersion | 0-100 | Air quality |
| `wave_height` | Ocean wave height (m) | 0-20 | Marine |
| `wave_period` | Wave period (s) | 0-30 | Marine |
| `wave_direction` | Wave direction (deg) | 0-360 | Marine |
| `primary_swell_height` | Primary swell (m) | 0-20 | Marine |
| `transport_wind_speed` | Transport wind (km/h) | 0-200 | Fire weather |
| `twenty_foot_wind_speed` | 20ft wind (km/h) | 0-200 | Fire weather |

---

## 6. Outdoor Air Quality Tables

### 6.1 silver.outdoor_air_quality

Outdoor AQ from OpenWeatherMap Air Pollution API.

```sql
CREATE TABLE silver.outdoor_air_quality (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    -- Location
    latitude            DOUBLE PRECISION,
    longitude           DOUBLE PRECISION,

    -- Overall AQI (OpenWeatherMap scale: 1=Good, 5=Very Poor)
    aqi_owm             SMALLINT,      -- 1-5 scale
    aqi_epa             SMALLINT,      -- Calculated EPA AQI (0-500)

    -- Pollutant Concentrations (all ug/m3)
    co                  DOUBLE PRECISION,  -- Carbon monoxide
    no                  DOUBLE PRECISION,  -- Nitric oxide
    no2                 DOUBLE PRECISION,  -- Nitrogen dioxide
    o3                  DOUBLE PRECISION,  -- Ozone
    so2                 DOUBLE PRECISION,  -- Sulfur dioxide
    nh3                 DOUBLE PRECISION,  -- Ammonia
    pm10                DOUBLE PRECISION,  -- Particulate matter < 10um
    pm25                DOUBLE PRECISION,  -- Particulate matter < 2.5um

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable('silver.outdoor_air_quality',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);

CREATE INDEX idx_outdoor_aq_ndp ON silver.outdoor_air_quality (ndp_id, observation_time DESC);

COMMENT ON TABLE silver.outdoor_air_quality IS
    'Outdoor air quality data from OpenWeatherMap Air Pollution API.
     Source: outdoor-air-quality Bronze stream.
     Grain: One row per API response (~10-minute intervals).
     Use: Window management decisions, indoor/outdoor comparison.';
```

### 6.2 OWM AQI to EPA AQI Mapping

OpenWeatherMap uses a simple 1-5 scale. For EPA compatibility:

| OWM AQI | Description | Approximate EPA AQI |
|---------|-------------|---------------------|
| 1 | Good | 0-50 |
| 2 | Fair | 51-100 |
| 3 | Moderate | 101-150 |
| 4 | Poor | 151-200 |
| 5 | Very Poor | 201+ |

**Note**: True EPA AQI should be calculated from pollutant concentrations using EPA breakpoints. See [Computed Fields](#8-computed-fields).

---

## 7. Analytics Views

### 7.1 analytics.forecast_accuracy

Core view for forecast evaluation.

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

    -- Absolute errors
    ABS(f.temperature_c - o.temperature_c) AS temp_error,
    ABS(f.humidity_pct - o.humidity_pct) AS humidity_error,
    ABS(f.wind_speed_kmh - o.wind_speed_kmh) AS wind_error,

    -- Signed errors (positive = forecast too high)
    f.temperature_c - o.temperature_c AS temp_bias,
    f.humidity_pct - o.humidity_pct AS humidity_bias,
    f.wind_speed_kmh - o.wind_speed_kmh AS wind_bias

FROM silver.weather_forecasts f
JOIN silver.weather_observations o
    ON f.valid_time = o.observation_time
   AND f.ndp_id = o.ndp_id
WHERE f.dq_flags IS NULL OR ARRAY_LENGTH(f.dq_flags, 1) = 0
  AND o.dq_flags IS NULL OR ARRAY_LENGTH(o.dq_flags, 1) = 0;

COMMENT ON VIEW analytics.forecast_accuracy IS
    'Joins forecasts to observations for accuracy analysis.
     Key dimension: lead_time_hours (how far in advance was the forecast?).
     Filters: Excludes rows with DQ flags for clean analysis.';
```

### 7.2 analytics.indoor_outdoor_comparison

For window management use case.

```sql
CREATE VIEW analytics.indoor_outdoor_comparison AS
WITH indoor AS (
    SELECT
        time_bucket('1 hour', observation_time) AS hour,
        AVG(pm25_compensated) FILTER (WHERE pm25_compensated IS NOT NULL) AS indoor_pm25,
        AVG(pm25) FILTER (WHERE pm25_compensated IS NULL) AS indoor_pm25_raw,
        AVG(co2) AS indoor_co2,
        AVG(temperature_c_compensated) AS indoor_temp,
        AVG(humidity_pct_compensated) AS indoor_humidity
    FROM silver.air_quality_observations
    WHERE location_type = 'indoor'
    GROUP BY 1
),
outdoor AS (
    SELECT
        time_bucket('1 hour', observation_time) AS hour,
        AVG(pm25) AS outdoor_pm25,
        AVG(o3) AS outdoor_ozone
    FROM silver.outdoor_air_quality
    GROUP BY 1
),
weather AS (
    SELECT
        time_bucket('1 hour', observation_time) AS hour,
        AVG(temperature_c) AS outdoor_temp,
        AVG(humidity_pct) AS outdoor_humidity,
        AVG(wind_speed_kmh) AS outdoor_wind
    FROM silver.weather_observations
    GROUP BY 1
)
SELECT
    COALESCE(i.hour, o.hour, w.hour) AS hour,

    -- Indoor metrics
    COALESCE(i.indoor_pm25, i.indoor_pm25_raw) AS indoor_pm25,
    i.indoor_co2,
    i.indoor_temp,
    i.indoor_humidity,

    -- Outdoor metrics
    o.outdoor_pm25,
    o.outdoor_ozone,
    w.outdoor_temp,
    w.outdoor_humidity,
    w.outdoor_wind,

    -- Differentials
    COALESCE(i.indoor_pm25, i.indoor_pm25_raw) - o.outdoor_pm25 AS pm25_differential,
    i.indoor_temp - w.outdoor_temp AS temp_differential,

    -- Window recommendation logic
    CASE
        WHEN o.outdoor_pm25 < COALESCE(i.indoor_pm25, i.indoor_pm25_raw) * 0.8
             AND w.outdoor_temp BETWEEN 18 AND 26
             AND w.outdoor_humidity < 80
        THEN 'OPEN_WINDOWS'
        WHEN o.outdoor_pm25 > COALESCE(i.indoor_pm25, i.indoor_pm25_raw) * 1.2
        THEN 'KEEP_CLOSED'
        ELSE 'NEUTRAL'
    END AS window_recommendation

FROM indoor i
FULL OUTER JOIN outdoor o ON i.hour = o.hour
FULL OUTER JOIN weather w ON COALESCE(i.hour, o.hour) = w.hour;

COMMENT ON VIEW analytics.indoor_outdoor_comparison IS
    'Compares indoor and outdoor conditions hourly.
     Use: Window management decisions.
     Window open: When outdoor PM < indoor AND temp comfortable.';
```

---

## 8. Computed Fields

### 8.1 Lead Time Calculation

Stored as generated column for indexing efficiency:

```sql
lead_time_hours INTEGER GENERATED ALWAYS AS
    (EXTRACT(EPOCH FROM valid_time - issue_time) / 3600)::INTEGER STORED
```

**Rationale**: Lead time is the key dimension for forecast evaluation. Storing it enables efficient indexing.

### 8.2 EPA AQI Calculation

SQL function for PM2.5 AQI:

```sql
CREATE OR REPLACE FUNCTION calculate_aqi_pm25(pm25_value DOUBLE PRECISION)
RETURNS SMALLINT AS $$
DECLARE
    aqi SMALLINT;
BEGIN
    -- 2024 EPA PM2.5 breakpoints
    IF pm25_value IS NULL THEN
        RETURN NULL;
    ELSIF pm25_value <= 9.0 THEN
        aqi := linear_interpolate(pm25_value, 0, 9.0, 0, 50);
    ELSIF pm25_value <= 35.4 THEN
        aqi := linear_interpolate(pm25_value, 9.1, 35.4, 51, 100);
    ELSIF pm25_value <= 55.4 THEN
        aqi := linear_interpolate(pm25_value, 35.5, 55.4, 101, 150);
    ELSIF pm25_value <= 125.4 THEN
        aqi := linear_interpolate(pm25_value, 55.5, 125.4, 151, 200);
    ELSIF pm25_value <= 225.4 THEN
        aqi := linear_interpolate(pm25_value, 125.5, 225.4, 201, 300);
    ELSIF pm25_value <= 325.4 THEN
        aqi := linear_interpolate(pm25_value, 225.5, 325.4, 301, 500);
    ELSE
        aqi := 500;  -- Beyond scale
    END IF;

    RETURN aqi;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

-- Helper function
CREATE OR REPLACE FUNCTION linear_interpolate(
    value DOUBLE PRECISION,
    bp_low DOUBLE PRECISION,
    bp_high DOUBLE PRECISION,
    aqi_low INTEGER,
    aqi_high INTEGER
) RETURNS SMALLINT AS $$
BEGIN
    RETURN ROUND(
        ((aqi_high - aqi_low)::DOUBLE PRECISION / (bp_high - bp_low))
        * (value - bp_low) + aqi_low
    )::SMALLINT;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;
```

### 8.3 Heat Index Calculation

```sql
CREATE OR REPLACE FUNCTION calculate_heat_index(
    temp_c DOUBLE PRECISION,
    humidity_pct DOUBLE PRECISION
) RETURNS DOUBLE PRECISION AS $$
DECLARE
    temp_f DOUBLE PRECISION;
    hi_f DOUBLE PRECISION;
BEGIN
    -- Heat index only valid when temp >= 27C (80F)
    IF temp_c IS NULL OR humidity_pct IS NULL OR temp_c < 27 THEN
        RETURN NULL;
    END IF;

    temp_f := temp_c * 9/5 + 32;

    -- Rothfusz regression equation
    hi_f := -42.379 + 2.04901523*temp_f + 10.14333127*humidity_pct
            - 0.22475541*temp_f*humidity_pct - 6.83783e-3*temp_f*temp_f
            - 5.481717e-2*humidity_pct*humidity_pct
            + 1.22874e-3*temp_f*temp_f*humidity_pct
            + 8.5282e-4*temp_f*humidity_pct*humidity_pct
            - 1.99e-6*temp_f*temp_f*humidity_pct*humidity_pct;

    -- Convert back to Celsius
    RETURN (hi_f - 32) * 5/9;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;
```

### 8.4 Mold Risk Index

```sql
CREATE OR REPLACE FUNCTION calculate_mold_risk(
    temp_c DOUBLE PRECISION,
    humidity_pct DOUBLE PRECISION
) RETURNS TEXT AS $$
BEGIN
    IF humidity_pct IS NULL THEN
        RETURN 'UNKNOWN';
    ELSIF humidity_pct < 50 THEN
        RETURN 'LOW';
    ELSIF humidity_pct < 60 THEN
        RETURN 'MODERATE';
    ELSIF humidity_pct < 65 THEN
        RETURN 'ELEVATED';
    ELSIF humidity_pct < 80 THEN
        RETURN 'HIGH';
    ELSE
        RETURN 'CRITICAL';
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;
```

---

## 9. Indexing Strategy

### 9.1 Index Design Principles

| Index Type | Use Case | Example |
|------------|----------|---------|
| Hypertable time column | All tables | `observation_time`, `valid_time` |
| Composite (ndp_id, time) | Location + time range | Recent data for specific sensor |
| Lead time | Forecast accuracy analysis | `(lead_time_hours, valid_time)` |
| GIN on arrays | DQ flag queries | `USING GIN (dq_flags)` |
| BRIN | Large sequential scans | Alternative for archival |

### 9.2 Recommended Indexes Summary

| Table | Index | Columns | Purpose |
|-------|-------|---------|---------|
| air_quality_observations | Primary | `(observation_time, ndp_id)` | Hypertable dimension |
| air_quality_observations | Secondary | `(ndp_id, observation_time DESC)` | Sensor lookups |
| weather_observations | Primary | `(observation_time, ndp_id)` | Hypertable dimension |
| weather_observations | Secondary | `(station_id, observation_time DESC)` | Station lookups |
| weather_forecasts | Primary | `(issue_time, valid_time, ndp_id)` | Unique forecasts |
| weather_forecasts | Hypertable | `valid_time` | Time-based partition |
| weather_forecasts | Lead time | `(lead_time_hours, valid_time)` | Accuracy analysis |
| outdoor_air_quality | Primary | `(observation_time, ndp_id)` | Hypertable dimension |

---

## 10. NULL Handling Policy

### 10.1 NULL Semantics

| NULL Meaning | Example | Handling |
|--------------|---------|----------|
| Sensor unavailable | CO2 sensor warming up | Store NULL |
| Value not applicable | Heat index when cold | Store NULL |
| Missing from source | API didn't return field | Store NULL |
| DQ rejected | Value out of valid range | Set NULL + flag |

### 10.2 NOT NULL Constraints

Only columns that MUST exist for a valid row:

| Table | NOT NULL Columns | Rationale |
|-------|------------------|-----------|
| All tables | `ingestion_time`, time column, `ndp_id` | Required for routing/audit |
| air_quality_observations | `pm25` (at least raw) | Primary metric for AQ |
| weather_observations | `temperature_c` | Primary weather metric |
| weather_forecasts | `issue_time`, `valid_time` | Required for domain model |

### 10.3 Default Values

Avoid defaults that could mask missing data. Use NULL instead.

**Exception**: `ingestion_time DEFAULT NOW()` for audit purposes.

---

## Appendix A: Bronze to Silver Field Mapping

### A.1 Air Quality (AirGradient)

| Bronze Field | Silver Column | Transformation |
|--------------|---------------|----------------|
| `raw_payload.atmp` | `temperature_c` | Direct copy |
| `raw_payload.atmpCompensated` | `temperature_c_compensated` | Direct copy |
| `raw_payload.rhum` | `humidity_pct` | Direct copy |
| `raw_payload.rhumCompensated` | `humidity_pct_compensated` | Direct copy |
| `raw_payload.rco2` | `co2` | Direct copy |
| `raw_payload.pm01` | `pm1` | Direct copy |
| `raw_payload.pm02` | `pm25` | Direct copy |
| `raw_payload.pm02Compensated` | `pm25_compensated` | Direct copy |
| `raw_payload.pm10` | `pm10` | Direct copy |
| `raw_payload.tvocIndex` | `tvoc_index` | Direct copy |
| `raw_payload.noxIndex` | `nox_index` | Direct copy |
| `raw_payload.wifi` | `wifi_signal_dbm` | Direct copy |
| `raw_payload.serialno` | `device_serial` | Direct copy |
| `raw_payload.model` | `device_model` | Direct copy |
| `raw_payload.firmware` | `firmware_version` | Direct copy |
| `context.location.type` | `location_type` | Direct copy |
| `context.location.path` | `location_path` | Direct copy |
| `timestamp` | `observation_time` | Microseconds to TIMESTAMPTZ |

### A.2 NWS Observations

| Bronze Field | Silver Column | Transformation |
|--------------|---------------|----------------|
| `raw_payload.properties.timestamp` | `observation_time` | ISO8601 to TIMESTAMPTZ |
| `raw_payload.properties.temperature.value` | `temperature_c` | Direct (already Celsius) |
| `raw_payload.properties.dewpoint.value` | `dewpoint_c` | Direct |
| `raw_payload.properties.relativeHumidity.value` | `humidity_pct` | Direct |
| `raw_payload.properties.windSpeed.value` | `wind_speed_kmh` | Direct (already km/h) |
| `raw_payload.properties.windDirection.value` | `wind_direction_deg` | Direct |
| `raw_payload.properties.windGust.value` | `wind_gust_kmh` | Direct |
| `raw_payload.properties.barometricPressure.value` | `pressure_pa` | Direct (already Pa) |
| `raw_payload.properties.visibility.value` | `visibility_m` | Direct (already m) |
| `raw_payload.properties.heatIndex.value` | `heat_index_c` | Direct, NULL if null |
| `raw_payload.properties.windChill.value` | `wind_chill_c` | Direct, NULL if null |
| `raw_payload.properties.textDescription` | `text_description` | Direct |
| `raw_payload.properties.stationId` | `station_id` | Direct |
| `raw_payload.properties.stationName` | `station_name` | Direct |
| `raw_payload.properties.elevation.value` | `station_elevation_m` | Direct |

### A.3 Outdoor Air Quality (OpenWeatherMap)

| Bronze Field | Silver Column | Transformation |
|--------------|---------------|----------------|
| `raw_payload.list[0].dt` | `observation_time` | Unix timestamp to TIMESTAMPTZ |
| `raw_payload.list[0].main.aqi` | `aqi_owm` | Direct (1-5 scale) |
| `raw_payload.list[0].components.co` | `co` | Direct (ug/m3) |
| `raw_payload.list[0].components.no` | `no` | Direct |
| `raw_payload.list[0].components.no2` | `no2` | Direct |
| `raw_payload.list[0].components.o3` | `o3` | Direct |
| `raw_payload.list[0].components.so2` | `so2` | Direct |
| `raw_payload.list[0].components.nh3` | `nh3` | Direct |
| `raw_payload.list[0].components.pm10` | `pm10` | Direct |
| `raw_payload.list[0].components.pm2_5` | `pm25` | Direct |
| `raw_payload.coord.lat` | `latitude` | Direct |
| `raw_payload.coord.lon` | `longitude` | Direct |

---

## Appendix B: DQ Rules Reference

### B.1 Air Quality DQ Rules

| Rule Name | Column | Condition | Action |
|-----------|--------|-----------|--------|
| `co2_range` | `co2` | 380-10000 ppm | Clamp to range |
| `pm25_range` | `pm25` | 0-1000 ug/m3 | Flag if exceeded |
| `temp_range` | `temperature_c` | -40 to 60 C | Flag if exceeded |
| `humidity_range` | `humidity_pct` | 0-100% | Clamp to 0 or 100 |
| `tvoc_range` | `tvoc_index` | 1-500 | Clamp to range |

### B.2 Weather DQ Rules

| Rule Name | Column | Condition | Action |
|-----------|--------|-----------|--------|
| `temp_physical` | `temperature_c` | -60 to 60 C | Flag if exceeded |
| `wind_direction` | `wind_direction_deg` | 0-360 | Modulo 360 |
| `humidity_range` | `humidity_pct` | 0-100% | Clamp |
| `precip_prob` | `precip_prob_pct` | 0-100% | Clamp |
| `visibility_range` | `visibility_m` | 0-100000 m | Flag if exceeded |
| `lead_time_valid` | `lead_time_hours` | 0-168 hrs | Reject if > 8 days |

---

## Appendix C: Continuous Aggregates

### C.1 Hourly Air Quality Aggregate

```sql
CREATE MATERIALIZED VIEW silver.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS hour,
    ndp_id,
    location_path,

    -- Aggregations
    AVG(co2) AS avg_co2,
    MAX(co2) AS max_co2,
    AVG(pm25_compensated) AS avg_pm25,
    MAX(pm25_compensated) AS max_pm25,
    AVG(tvoc_index) AS avg_tvoc,
    AVG(temperature_c_compensated) AS avg_temp,
    AVG(humidity_pct_compensated) AS avg_humidity,

    -- Sample count for completeness
    COUNT(*) AS sample_count
FROM silver.air_quality_observations
GROUP BY 1, 2, 3
WITH NO DATA;

-- Refresh policy
SELECT add_continuous_aggregate_policy('silver.air_quality_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);
```

### C.2 Accuracy by Lead Time Aggregate

```sql
CREATE MATERIALIZED VIEW analytics.accuracy_by_lead_time
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', f.valid_time) AS day,
    f.lead_time_hours,
    f.ndp_id,

    COUNT(*) AS sample_count,

    -- Temperature metrics
    AVG(ABS(f.temperature_c - o.temperature_c)) AS avg_temp_error,
    AVG(f.temperature_c - o.temperature_c) AS avg_temp_bias,

    -- Humidity metrics
    AVG(ABS(f.humidity_pct - o.humidity_pct)) AS avg_humidity_error,

    -- Wind metrics
    AVG(ABS(f.wind_speed_kmh - o.wind_speed_kmh)) AS avg_wind_error

FROM silver.weather_forecasts f
JOIN silver.weather_observations o
    ON f.valid_time = o.observation_time
   AND f.ndp_id = o.ndp_id
GROUP BY 1, 2, 3
WITH NO DATA;
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-05 | NDP Analytics Engineer | Initial draft |

---

## References

1. **EPA PM2.5 Standards (2024)**: Annual standard 9.0 ug/m3
2. **TimescaleDB Documentation**: Hypertables, continuous aggregates
3. **NWS API Documentation**: Unit codes, field definitions
4. **AirGradient Documentation**: Sensor specifications, field names
5. **Domain Model**: `02-WEATHER-DOMAIN-MODEL.md`
6. **Forecast Schema**: `05-FORECAST-EVALUATION-SCHEMA.md`
7. **Air Quality Analytics**: `02-air-quality-analytics.md`
