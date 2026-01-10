-- =============================================================================
-- Neural Data Platform - Silver Layer TimescaleDB Schema
-- =============================================================================
-- Feature: DP-006 - Silver Layer Implementation
-- Version: 1.0.0
-- Date: 2026-01-10
--
-- This script initializes the Silver layer tables in TimescaleDB.
-- Tables are designed for:
--   1. Raspberry Pi 5 resource constraints (1-day chunks, 256MB memory)
--   2. Grafana dashboard queries (indexed by ndp_id, time DESC)
--   3. Config-driven ETL from Bronze Parquet layer
--   4. Data quality transparency (dq_flags on all tables)
--
-- Run order: 001 (first init script)
-- =============================================================================

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Create silver schema
CREATE SCHEMA IF NOT EXISTS silver;
CREATE SCHEMA IF NOT EXISTS analytics;

-- =============================================================================
-- SECTION 1: Helper Functions
-- =============================================================================

-- Linear interpolation helper for AQI calculation
CREATE OR REPLACE FUNCTION silver.linear_interpolate(
    value DOUBLE PRECISION,
    bp_low DOUBLE PRECISION,
    bp_high DOUBLE PRECISION,
    aqi_low INTEGER,
    aqi_high INTEGER
) RETURNS SMALLINT AS $$
BEGIN
    IF value IS NULL OR bp_high = bp_low THEN
        RETURN NULL;
    END IF;
    RETURN ROUND(
        ((aqi_high - aqi_low)::DOUBLE PRECISION / (bp_high - bp_low))
        * (value - bp_low) + aqi_low
    )::SMALLINT;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

-- EPA PM2.5 AQI calculation (2024 breakpoints)
CREATE OR REPLACE FUNCTION silver.calculate_aqi_pm25(pm25_value DOUBLE PRECISION)
RETURNS SMALLINT AS $$
BEGIN
    IF pm25_value IS NULL THEN
        RETURN NULL;
    ELSIF pm25_value <= 9.0 THEN
        RETURN silver.linear_interpolate(pm25_value, 0, 9.0, 0, 50);
    ELSIF pm25_value <= 35.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 9.1, 35.4, 51, 100);
    ELSIF pm25_value <= 55.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 35.5, 55.4, 101, 150);
    ELSIF pm25_value <= 125.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 55.5, 125.4, 151, 200);
    ELSIF pm25_value <= 225.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 125.5, 225.4, 201, 300);
    ELSIF pm25_value <= 325.4 THEN
        RETURN silver.linear_interpolate(pm25_value, 225.5, 325.4, 301, 500);
    ELSE
        RETURN 500;  -- Beyond scale
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;

-- Mold risk index calculation
CREATE OR REPLACE FUNCTION silver.calculate_mold_risk(
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

-- =============================================================================
-- SECTION 2: silver.air_quality_observations
-- =============================================================================
-- Source: Bronze air-quality stream (AirGradient sensors via MQTT)
-- Grain: One row per sensor reading (~1 minute intervals)
-- Use: Indoor air quality monitoring, window management decisions

CREATE TABLE silver.air_quality_observations (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'air-quality',
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

-- Convert to hypertable with 1-day chunks (Pi memory constraint)
SELECT create_hypertable('silver.air_quality_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Secondary indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_aq_obs_ndp_id
    ON silver.air_quality_observations (ndp_id, observation_time DESC);
CREATE INDEX IF NOT EXISTS idx_aq_obs_location
    ON silver.air_quality_observations (location_path, observation_time DESC);
CREATE INDEX IF NOT EXISTS idx_aq_obs_dq_flags
    ON silver.air_quality_observations USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

COMMENT ON TABLE silver.air_quality_observations IS
    'Indoor air quality measurements from AirGradient sensors.
     Source: air-quality Bronze stream (MQTT).
     Grain: One row per sensor reading (~1 minute intervals).
     Use: Window management decisions, indoor air quality monitoring.';

-- =============================================================================
-- SECTION 3: silver.weather_observations
-- =============================================================================
-- Source: Bronze nws-observations stream (NWS stations)
-- Grain: One row per observation (~10-15 minute intervals)
-- Use: Ground truth weather, forecast verification

CREATE TABLE silver.weather_observations (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'nws-observations',
    ndp_id              TEXT NOT NULL,

    -- Station Context
    station_id          TEXT,          -- e.g., 'KSGJ'
    station_name        TEXT,          -- Human-readable name
    station_elevation_m DOUBLE PRECISION,

    -- Core Weather Metrics (aligned with forecast schema for comparison)
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
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_weather_obs_ndp
    ON silver.weather_observations (ndp_id, observation_time DESC);
CREATE INDEX IF NOT EXISTS idx_weather_obs_station
    ON silver.weather_observations (station_id, observation_time DESC);

COMMENT ON TABLE silver.weather_observations IS
    'Ground truth weather observations from NWS stations.
     Source: nws-observations Bronze stream.
     Grain: One row per observation (~10-15 minute intervals).
     Use: Forecast accuracy verification, current conditions display.';

-- =============================================================================
-- SECTION 4: silver.weather_forecasts
-- =============================================================================
-- Source: Bronze nws-forecast-hourly, nws-gridpoints-forecast streams
-- Grain: One row per (issue_time, valid_time, location)
-- Use: Forecast evaluation, planning decisions

CREATE TABLE silver.weather_forecasts (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_stream       TEXT NOT NULL,  -- 'nws-forecast-hourly' or 'nws-gridpoints-forecast'

    -- DOMAIN KEYS (Critical for forecast evaluation)
    issue_time          TIMESTAMPTZ NOT NULL,  -- When NWS generated this forecast
    valid_time          TIMESTAMPTZ NOT NULL,  -- When prediction applies
    valid_duration      INTERVAL,              -- How long valid (PT1H, PT6H, etc.)

    -- Computed: Essential for accuracy analysis
    -- Stored as regular column (generated columns have limitations in hypertables)
    lead_time_hours     INTEGER,

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

    -- Short forecast text
    short_forecast      TEXT,

    -- DQ Transparency
    dq_flags            TEXT[],

    -- Primary key: unique forecast for each (issue, valid, location)
    PRIMARY KEY (valid_time, issue_time, ndp_id)
);

-- Hypertable partitioned by valid_time (queries are typically "forecasts for next N hours")
SELECT create_hypertable('silver.weather_forecasts',
    'valid_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Critical index for lead_time analysis
CREATE INDEX IF NOT EXISTS idx_forecasts_lead_time
    ON silver.weather_forecasts (lead_time_hours, valid_time DESC);

-- Index for joining with observations (forecast verification)
CREATE INDEX IF NOT EXISTS idx_forecasts_valid_ndp
    ON silver.weather_forecasts (valid_time, ndp_id);

-- Index for "latest forecast" queries
CREATE INDEX IF NOT EXISTS idx_forecasts_issue
    ON silver.weather_forecasts (issue_time DESC);

-- Index for source stream filtering
CREATE INDEX IF NOT EXISTS idx_forecasts_source
    ON silver.weather_forecasts (source_stream, valid_time DESC);

COMMENT ON TABLE silver.weather_forecasts IS
    'Weather forecasts from NWS gridpoints API.
     Source: nws-forecast-hourly, nws-gridpoints-forecast Bronze streams.
     Grain: One row per (issue_time, valid_time, location).
     Key Analysis: Join with observations on valid_time = observation_time for accuracy.
     lead_time_hours = hours between issue_time and valid_time.';

-- =============================================================================
-- SECTION 5: silver.outdoor_air_quality
-- =============================================================================
-- Source: Bronze outdoor-air-quality stream (OpenWeatherMap Air Pollution API)
-- Grain: One row per API response (~10 minute intervals)
-- Use: Outdoor AQ monitoring, indoor/outdoor comparison

CREATE TABLE silver.outdoor_air_quality (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'outdoor-air-quality',
    ndp_id              TEXT NOT NULL,

    -- Location
    latitude            DOUBLE PRECISION,
    longitude           DOUBLE PRECISION,

    -- Overall AQI
    aqi_owm             SMALLINT,      -- OpenWeatherMap scale: 1=Good, 5=Very Poor
    aqi_epa             SMALLINT,      -- Calculated EPA AQI (0-500, computed from PM2.5)

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
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_outdoor_aq_ndp
    ON silver.outdoor_air_quality (ndp_id, observation_time DESC);

COMMENT ON TABLE silver.outdoor_air_quality IS
    'Outdoor air quality data from OpenWeatherMap Air Pollution API.
     Source: outdoor-air-quality Bronze stream.
     Grain: One row per API response (~10 minute intervals).
     Use: Window management decisions, indoor/outdoor PM2.5 comparison.';

-- =============================================================================
-- SECTION 6: Compression Policies
-- =============================================================================
-- Compress data older than 7 days to reduce storage on Pi
-- Compression is particularly effective for time-series with repeated values

-- Air quality observations compression
ALTER TABLE silver.air_quality_observations SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);

SELECT add_compression_policy('silver.air_quality_observations',
    INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Weather observations compression
ALTER TABLE silver.weather_observations SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);

SELECT add_compression_policy('silver.weather_observations',
    INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Weather forecasts compression
ALTER TABLE silver.weather_forecasts SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id, source_stream',
    timescaledb.compress_orderby = 'valid_time DESC, issue_time DESC'
);

SELECT add_compression_policy('silver.weather_forecasts',
    INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Outdoor air quality compression
ALTER TABLE silver.outdoor_air_quality SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);

SELECT add_compression_policy('silver.outdoor_air_quality',
    INTERVAL '7 days',
    if_not_exists => TRUE
);

-- =============================================================================
-- SECTION 7: Retention Policies
-- =============================================================================
-- Keep raw Silver data for 90 days (can be rebuilt from Bronze)
-- Continuous aggregates kept longer (see Section 8)

SELECT add_retention_policy('silver.air_quality_observations',
    INTERVAL '90 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('silver.weather_observations',
    INTERVAL '90 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('silver.weather_forecasts',
    INTERVAL '90 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('silver.outdoor_air_quality',
    INTERVAL '90 days',
    if_not_exists => TRUE
);

-- =============================================================================
-- SECTION 8: Continuous Aggregates
-- =============================================================================
-- Pre-computed aggregations for efficient dashboard queries
-- Note: Out of scope for dp-006 MVP but structure provided for dp-007+

-- 8.1 Hourly Air Quality Aggregate
CREATE MATERIALIZED VIEW IF NOT EXISTS silver.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    location_path,
    source_stream,

    -- Aggregations
    AVG(co2) AS avg_co2,
    MAX(co2) AS max_co2,
    MIN(co2) AS min_co2,
    AVG(COALESCE(pm25_compensated, pm25)) AS avg_pm25,
    MAX(COALESCE(pm25_compensated, pm25)) AS max_pm25,
    AVG(tvoc_index) AS avg_tvoc,
    AVG(COALESCE(temperature_c_compensated, temperature_c)) AS avg_temp_c,
    AVG(COALESCE(humidity_pct_compensated, humidity_pct)) AS avg_humidity_pct,

    -- Sample count for completeness assessment
    COUNT(*) AS sample_count

FROM silver.air_quality_observations
GROUP BY bucket, ndp_id, location_path, source_stream
WITH NO DATA;

-- Refresh policy: refresh hourly, looking back 3 hours for late arrivals
SELECT add_continuous_aggregate_policy('silver.air_quality_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 8.2 Hourly Weather Observations Aggregate
CREATE MATERIALIZED VIEW IF NOT EXISTS silver.weather_observations_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    station_id,
    source_stream,

    -- Temperature
    AVG(temperature_c) AS avg_temp_c,
    MAX(temperature_c) AS max_temp_c,
    MIN(temperature_c) AS min_temp_c,
    AVG(dewpoint_c) AS avg_dewpoint_c,

    -- Humidity/Pressure
    AVG(humidity_pct) AS avg_humidity_pct,
    AVG(pressure_pa) AS avg_pressure_pa,

    -- Wind
    AVG(wind_speed_kmh) AS avg_wind_speed_kmh,
    MAX(wind_gust_kmh) AS max_wind_gust_kmh,

    -- Visibility
    AVG(visibility_m) AS avg_visibility_m,

    -- Sample count
    COUNT(*) AS sample_count

FROM silver.weather_observations
GROUP BY bucket, ndp_id, station_id, source_stream
WITH NO DATA;

SELECT add_continuous_aggregate_policy('silver.weather_observations_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 8.3 Hourly Outdoor Air Quality Aggregate
CREATE MATERIALIZED VIEW IF NOT EXISTS silver.outdoor_air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    source_stream,

    -- AQI
    AVG(aqi_owm) AS avg_aqi_owm,
    MAX(aqi_owm) AS max_aqi_owm,

    -- Key pollutants
    AVG(pm25) AS avg_pm25,
    MAX(pm25) AS max_pm25,
    AVG(pm10) AS avg_pm10,
    AVG(o3) AS avg_o3,
    AVG(no2) AS avg_no2,

    -- Sample count
    COUNT(*) AS sample_count

FROM silver.outdoor_air_quality
GROUP BY bucket, ndp_id, source_stream
WITH NO DATA;

SELECT add_continuous_aggregate_policy('silver.outdoor_air_quality_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 8.4 Daily Air Quality Aggregate (for long-term trends)
CREATE MATERIALIZED VIEW IF NOT EXISTS silver.air_quality_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', observation_time) AS bucket,
    ndp_id,
    location_path,

    -- Daily aggregations
    AVG(co2) AS avg_co2,
    MAX(co2) AS max_co2,
    AVG(COALESCE(pm25_compensated, pm25)) AS avg_pm25,
    MAX(COALESCE(pm25_compensated, pm25)) AS max_pm25,
    AVG(COALESCE(temperature_c_compensated, temperature_c)) AS avg_temp_c,
    MAX(COALESCE(temperature_c_compensated, temperature_c)) AS max_temp_c,
    MIN(COALESCE(temperature_c_compensated, temperature_c)) AS min_temp_c,
    AVG(COALESCE(humidity_pct_compensated, humidity_pct)) AS avg_humidity_pct,

    COUNT(*) AS sample_count

FROM silver.air_quality_observations
GROUP BY bucket, ndp_id, location_path
WITH NO DATA;

SELECT add_continuous_aggregate_policy('silver.air_quality_daily',
    start_offset => INTERVAL '2 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Retention for continuous aggregates (keep longer than raw data)
SELECT add_retention_policy('silver.air_quality_hourly',
    INTERVAL '365 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('silver.weather_observations_hourly',
    INTERVAL '365 days',
    if_not_exists => TRUE
);

SELECT add_retention_policy('silver.outdoor_air_quality_hourly',
    INTERVAL '365 days',
    if_not_exists => TRUE
);

-- Keep daily aggregates indefinitely (small data volume)
-- No retention policy on silver.air_quality_daily

-- =============================================================================
-- SECTION 9: Analytics Views
-- =============================================================================
-- Views for common analytics patterns

-- 9.1 Forecast Accuracy View (for joining forecasts with observations)
CREATE OR REPLACE VIEW analytics.forecast_accuracy AS
SELECT
    f.valid_time,
    f.issue_time,
    f.lead_time_hours,
    f.ndp_id,
    f.source_stream AS forecast_source,

    -- Forecast values
    f.temperature_c AS forecast_temp_c,
    f.humidity_pct AS forecast_humidity_pct,
    f.wind_speed_kmh AS forecast_wind_kmh,
    f.precip_prob_pct AS forecast_precip_prob,

    -- Observed values
    o.temperature_c AS observed_temp_c,
    o.humidity_pct AS observed_humidity_pct,
    o.wind_speed_kmh AS observed_wind_kmh,

    -- Absolute errors
    ABS(f.temperature_c - o.temperature_c) AS temp_error_c,
    ABS(f.humidity_pct - o.humidity_pct) AS humidity_error_pct,
    ABS(f.wind_speed_kmh - o.wind_speed_kmh) AS wind_error_kmh,

    -- Signed errors (positive = forecast too high)
    f.temperature_c - o.temperature_c AS temp_bias_c,
    f.humidity_pct - o.humidity_pct AS humidity_bias_pct,
    f.wind_speed_kmh - o.wind_speed_kmh AS wind_bias_kmh

FROM silver.weather_forecasts f
INNER JOIN silver.weather_observations o
    ON f.valid_time = o.observation_time
   AND f.ndp_id = o.ndp_id
WHERE (f.dq_flags IS NULL OR array_length(f.dq_flags, 1) = 0)
  AND (o.dq_flags IS NULL OR array_length(o.dq_flags, 1) = 0);

COMMENT ON VIEW analytics.forecast_accuracy IS
    'Joins forecasts to observations for accuracy analysis.
     Key dimension: lead_time_hours (how far in advance was the forecast?).
     Filters: Excludes rows with DQ flags for clean analysis.
     Use: Dashboard panels showing forecast error by lead time.';

-- 9.2 Indoor/Outdoor Comparison View (for window management)
CREATE OR REPLACE VIEW analytics.indoor_outdoor_comparison AS
WITH indoor AS (
    SELECT
        time_bucket('1 hour', observation_time) AS hour,
        AVG(COALESCE(pm25_compensated, pm25)) AS indoor_pm25,
        AVG(co2) AS indoor_co2,
        AVG(COALESCE(temperature_c_compensated, temperature_c)) AS indoor_temp_c,
        AVG(COALESCE(humidity_pct_compensated, humidity_pct)) AS indoor_humidity_pct
    FROM silver.air_quality_observations
    WHERE location_type = 'indoor'
    GROUP BY 1
),
outdoor_aq AS (
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
        AVG(temperature_c) AS outdoor_temp_c,
        AVG(humidity_pct) AS outdoor_humidity_pct,
        AVG(wind_speed_kmh) AS outdoor_wind_kmh
    FROM silver.weather_observations
    GROUP BY 1
)
SELECT
    COALESCE(i.hour, o.hour, w.hour) AS hour,

    -- Indoor metrics
    i.indoor_pm25,
    i.indoor_co2,
    i.indoor_temp_c,
    i.indoor_humidity_pct,

    -- Outdoor metrics
    o.outdoor_pm25,
    o.outdoor_ozone,
    w.outdoor_temp_c,
    w.outdoor_humidity_pct,
    w.outdoor_wind_kmh,

    -- Differentials
    i.indoor_pm25 - o.outdoor_pm25 AS pm25_differential,
    i.indoor_temp_c - w.outdoor_temp_c AS temp_differential_c,

    -- Window recommendation logic
    CASE
        WHEN o.outdoor_pm25 < i.indoor_pm25 * 0.8
             AND w.outdoor_temp_c BETWEEN 18 AND 26
             AND w.outdoor_humidity_pct < 80
        THEN 'OPEN_WINDOWS'
        WHEN o.outdoor_pm25 > i.indoor_pm25 * 1.2
        THEN 'KEEP_CLOSED'
        ELSE 'NEUTRAL'
    END AS window_recommendation

FROM indoor i
FULL OUTER JOIN outdoor_aq o ON i.hour = o.hour
FULL OUTER JOIN weather w ON COALESCE(i.hour, o.hour) = w.hour;

COMMENT ON VIEW analytics.indoor_outdoor_comparison IS
    'Compares indoor and outdoor conditions hourly.
     Use: Window management decisions.
     Window open: When outdoor PM < indoor AND temp comfortable.
     Note: Uses time_bucket for alignment across data sources.';

-- 9.3 Latest Readings View (for dashboard current values)
CREATE OR REPLACE VIEW analytics.latest_readings AS
WITH latest_indoor AS (
    SELECT DISTINCT ON (ndp_id)
        ndp_id,
        observation_time,
        location_path,
        co2,
        COALESCE(pm25_compensated, pm25) AS pm25,
        COALESCE(temperature_c_compensated, temperature_c) AS temperature_c,
        COALESCE(humidity_pct_compensated, humidity_pct) AS humidity_pct,
        tvoc_index,
        nox_index
    FROM silver.air_quality_observations
    ORDER BY ndp_id, observation_time DESC
),
latest_outdoor_aq AS (
    SELECT DISTINCT ON (ndp_id)
        ndp_id,
        observation_time,
        aqi_owm,
        pm25,
        o3,
        no2
    FROM silver.outdoor_air_quality
    ORDER BY ndp_id, observation_time DESC
),
latest_weather AS (
    SELECT DISTINCT ON (ndp_id)
        ndp_id,
        observation_time,
        station_id,
        temperature_c,
        humidity_pct,
        wind_speed_kmh,
        wind_direction_deg,
        text_description
    FROM silver.weather_observations
    ORDER BY ndp_id, observation_time DESC
)
SELECT
    'indoor_aq' AS data_type,
    i.ndp_id,
    i.observation_time,
    i.location_path AS location,
    jsonb_build_object(
        'co2', i.co2,
        'pm25', i.pm25,
        'temperature_c', i.temperature_c,
        'humidity_pct', i.humidity_pct,
        'tvoc_index', i.tvoc_index
    ) AS metrics
FROM latest_indoor i
UNION ALL
SELECT
    'outdoor_aq' AS data_type,
    o.ndp_id,
    o.observation_time,
    NULL AS location,
    jsonb_build_object(
        'aqi_owm', o.aqi_owm,
        'pm25', o.pm25,
        'o3', o.o3,
        'no2', o.no2
    ) AS metrics
FROM latest_outdoor_aq o
UNION ALL
SELECT
    'weather' AS data_type,
    w.ndp_id,
    w.observation_time,
    w.station_id AS location,
    jsonb_build_object(
        'temperature_c', w.temperature_c,
        'humidity_pct', w.humidity_pct,
        'wind_speed_kmh', w.wind_speed_kmh,
        'description', w.text_description
    ) AS metrics
FROM latest_weather w;

COMMENT ON VIEW analytics.latest_readings IS
    'Latest readings from all Silver tables for dashboard display.
     Returns one row per data type per ndp_id with most recent values.
     Use: Dashboard "current conditions" panels.';

-- =============================================================================
-- SECTION 10: DQ Transparency Table (Optional)
-- =============================================================================
-- Separate table for detailed DQ event logging
-- Enables audit trail and DQ trend analysis

CREATE TABLE IF NOT EXISTS silver.dq_events (
    event_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_stream       TEXT NOT NULL,
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    column_name         TEXT NOT NULL,
    rule_name           TEXT NOT NULL,
    original_value      TEXT,          -- Stored as text for flexibility
    action_taken        TEXT NOT NULL, -- 'flag', 'reject', 'clamp', 'drop'
    result_value        TEXT,          -- Value after action (NULL if rejected)

    PRIMARY KEY (event_time, source_stream, ndp_id, column_name)
);

SELECT create_hypertable('silver.dq_events',
    'event_time',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Shorter retention for DQ events (detailed logging)
SELECT add_retention_policy('silver.dq_events',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_dq_events_stream
    ON silver.dq_events (source_stream, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_dq_events_rule
    ON silver.dq_events (rule_name, event_time DESC);

COMMENT ON TABLE silver.dq_events IS
    'Detailed DQ event logging for transparency and audit.
     Captures each DQ rule violation with original/result values.
     Use: DQ trend dashboards, investigating data quality issues.
     Retention: 30 days (summary in dq_flags column persists longer).';

-- =============================================================================
-- SECTION 11: Grant Permissions
-- =============================================================================
-- Create application role if not exists
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ndp_app') THEN
        CREATE ROLE ndp_app WITH LOGIN PASSWORD 'CHANGE_ME_IN_PRODUCTION';
    END IF;
END
$$;

-- Grant schema usage
GRANT USAGE ON SCHEMA silver TO ndp_app;
GRANT USAGE ON SCHEMA analytics TO ndp_app;

-- Grant table permissions
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA silver TO ndp_app;
GRANT SELECT ON ALL TABLES IN SCHEMA analytics TO ndp_app;

-- Grant sequence permissions (for any auto-increment columns)
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA silver TO ndp_app;

-- Grant function permissions
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA silver TO ndp_app;

-- Default privileges for future tables
ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT, INSERT, UPDATE ON TABLES TO ndp_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA analytics GRANT SELECT ON TABLES TO ndp_app;

-- =============================================================================
-- Schema version tracking
-- =============================================================================
CREATE TABLE IF NOT EXISTS silver.schema_version (
    version         TEXT PRIMARY KEY,
    applied_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    description     TEXT
);

INSERT INTO silver.schema_version (version, description)
VALUES ('1.0.0', 'Initial Silver layer schema for DP-006')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- Summary
-- =============================================================================
-- Tables created:
--   - silver.air_quality_observations (indoor AQ from AirGradient)
--   - silver.weather_observations (NWS ground truth)
--   - silver.weather_forecasts (NWS forecasts)
--   - silver.outdoor_air_quality (OWM outdoor AQ)
--   - silver.dq_events (DQ transparency logging)
--   - silver.schema_version (schema versioning)
--
-- Continuous Aggregates:
--   - silver.air_quality_hourly
--   - silver.weather_observations_hourly
--   - silver.outdoor_air_quality_hourly
--   - silver.air_quality_daily
--
-- Analytics Views:
--   - analytics.forecast_accuracy
--   - analytics.indoor_outdoor_comparison
--   - analytics.latest_readings
--
-- Policies:
--   - Compression: After 7 days
--   - Retention (raw): 90 days
--   - Retention (hourly aggregates): 365 days
--   - Retention (daily aggregates): Indefinite
--   - Retention (DQ events): 30 days
-- =============================================================================
