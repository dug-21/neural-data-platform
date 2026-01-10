-- =============================================================================
-- Neural Data Platform - Silver Layer Schema Migration
-- =============================================================================
-- Migration: 001_silver_schema.sql
-- Feature: DP-006 - Silver Layer Implementation
-- Version: 1.0.0
-- Date: 2026-01-10
-- Author: ndp-timescale-dev
--
-- This migration creates the Silver layer schema and hypertables.
-- Follows ADR-006-003: Flat silver.* schema for Phase 1.
--
-- Run order: 001 (first migration)
-- Idempotent: Yes (uses IF NOT EXISTS)
-- =============================================================================

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- =============================================================================
-- SECTION 1: Create Schema
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS silver;

COMMENT ON SCHEMA silver IS
    'Silver layer: Clean, typed time-series data from Bronze ETL.
     Source: Bronze Parquet files via DuckDB ETL.
     Use: Grafana dashboards, analytics queries, feature engineering.';

-- =============================================================================
-- SECTION 2: silver.air_quality_observations
-- =============================================================================
-- Source: Bronze air-quality stream (AirGradient sensors via MQTT)
-- Grain: One row per sensor reading (~1 minute intervals)
-- Use: Indoor air quality monitoring, window management decisions
-- Primary Key: (observation_time, ndp_id) per SPECIFICATION.md FR-007

CREATE TABLE IF NOT EXISTS silver.air_quality_observations (
    -- Time columns (required for hypertable)
    observation_time    TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identity columns
    ndp_id              TEXT NOT NULL,
    location_path       TEXT,

    -- Core Air Quality Metrics (per data dictionary)
    pm25                DOUBLE PRECISION,      -- ug/m3 (primary PM metric)
    pm10                DOUBLE PRECISION,      -- ug/m3
    co2                 SMALLINT,              -- ppm (380-10000)

    -- Environmental
    temperature_c       DOUBLE PRECISION,      -- Celsius
    humidity_pct        DOUBLE PRECISION,      -- 0-100%

    -- Gas Sensors (SGP41)
    voc_index           SMALLINT,              -- 1-500 (relative index)
    nox_index           SMALLINT,              -- 1-500 (relative index)

    -- DQ Transparency (per SPECIFICATION.md FR-013)
    dq_flags            TEXT[],                -- Array of rule violations

    -- Primary Key for hypertable
    PRIMARY KEY (observation_time, ndp_id)
);

-- Convert to hypertable with 1-day chunks (per FR-010, Pi memory constraint)
SELECT create_hypertable(
    'silver.air_quality_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

COMMENT ON TABLE silver.air_quality_observations IS
    'Indoor air quality measurements from AirGradient sensors.
     Source: air-quality Bronze stream (MQTT).
     Grain: One row per sensor reading (~1 minute intervals).
     Use: Window management decisions, indoor air quality monitoring.
     Primary Key: (observation_time, ndp_id)';

-- =============================================================================
-- SECTION 3: silver.weather_observations
-- =============================================================================
-- Source: Bronze nws-observations and outdoor-weather streams
-- Grain: One row per observation
-- Use: Ground truth weather, forecast verification
-- Note: Merged table for NWS + OWM per FR-008, with source_provider distinction

CREATE TABLE IF NOT EXISTS silver.weather_observations (
    -- Time columns
    observation_time    TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identity columns
    ndp_id              TEXT NOT NULL,
    source_provider     TEXT NOT NULL,         -- 'nws' or 'owm' per FR-008

    -- Core Weather Metrics (SI units per ADR-006-003)
    temperature_c       DOUBLE PRECISION,      -- Celsius
    humidity_pct        DOUBLE PRECISION,      -- 0-100%
    pressure_pa         DOUBLE PRECISION,      -- Pascals (OWM: hPa * 100)

    -- Wind
    wind_speed_kmh      DOUBLE PRECISION,      -- km/h (OWM: m/s * 3.6)
    wind_direction_deg  DOUBLE PRECISION,      -- 0-360 degrees

    -- Visibility/Cloud
    visibility_m        DOUBLE PRECISION,      -- Meters
    cloud_cover_pct     DOUBLE PRECISION,      -- 0-100%
    dew_point_c         DOUBLE PRECISION,      -- Celsius (dewpoint)

    -- Qualitative
    weather_description TEXT,                  -- 'Cloudy', 'Mostly Clear', etc.

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable(
    'silver.weather_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

COMMENT ON TABLE silver.weather_observations IS
    'Weather observations merged from NWS and OpenWeatherMap.
     Source: nws-observations, outdoor-weather Bronze streams.
     source_provider: nws or owm to distinguish data origin.
     Unit normalization: OWM Kelvin->Celsius, m/s->km/h, hPa->Pa.
     Primary Key: (observation_time, ndp_id)';

-- =============================================================================
-- SECTION 4: silver.weather_forecasts
-- =============================================================================
-- Source: Bronze nws-forecast-hourly, nws-gridpoints-forecast streams
-- Grain: One row per (issue_time, valid_time, location)
-- Use: Forecast evaluation, planning decisions
-- Note: lead_time_hours is computed per FR-009

CREATE TABLE IF NOT EXISTS silver.weather_forecasts (
    -- Time columns (valid_time is hypertable dimension)
    issue_time          TIMESTAMPTZ NOT NULL,  -- When NWS generated this forecast
    valid_time          TIMESTAMPTZ NOT NULL,  -- When prediction applies
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identity columns
    ndp_id              TEXT NOT NULL,

    -- Computed: Lead time in hours (per FR-009)
    -- Note: Using regular column due to hypertable limitations with GENERATED
    lead_time_hours     INTEGER GENERATED ALWAYS AS
                        (EXTRACT(EPOCH FROM (valid_time - issue_time)) / 3600)::INTEGER STORED,

    -- Core Forecast Metrics
    temperature_c       DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    wind_speed_kmh      DOUBLE PRECISION,
    precipitation_probability_pct DOUBLE PRECISION,

    -- Qualitative
    weather_description TEXT,                  -- Short forecast text

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (issue_time, valid_time, ndp_id)
);

SELECT create_hypertable(
    'silver.weather_forecasts',
    'valid_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

COMMENT ON TABLE silver.weather_forecasts IS
    'Weather forecasts from NWS gridpoints API.
     Source: nws-forecast-hourly, nws-gridpoints-forecast Bronze streams.
     Grain: One row per (issue_time, valid_time, location).
     lead_time_hours: Computed as (valid_time - issue_time) / 3600.
     Key Analysis: Join with observations on valid_time = observation_time.
     Primary Key: (issue_time, valid_time, ndp_id)';

-- =============================================================================
-- SECTION 5: silver.outdoor_air_quality
-- =============================================================================
-- Source: Bronze outdoor-air-quality stream (OpenWeatherMap Air Pollution API)
-- Grain: One row per API response (~10 minute intervals)
-- Use: Outdoor AQ monitoring, indoor/outdoor comparison

CREATE TABLE IF NOT EXISTS silver.outdoor_air_quality (
    -- Time columns
    observation_time    TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Identity columns
    ndp_id              TEXT NOT NULL,

    -- Overall AQI
    aqi                 SMALLINT,              -- OpenWeatherMap scale: 1-5

    -- Pollutant Concentrations (all ug/m3)
    pm25                DOUBLE PRECISION,      -- Particulate matter < 2.5um
    pm10                DOUBLE PRECISION,      -- Particulate matter < 10um
    o3                  DOUBLE PRECISION,      -- Ozone
    no2                 DOUBLE PRECISION,      -- Nitrogen dioxide
    so2                 DOUBLE PRECISION,      -- Sulfur dioxide
    co                  DOUBLE PRECISION,      -- Carbon monoxide

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable(
    'silver.outdoor_air_quality',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

COMMENT ON TABLE silver.outdoor_air_quality IS
    'Outdoor air quality data from OpenWeatherMap Air Pollution API.
     Source: outdoor-air-quality Bronze stream.
     Grain: One row per API response (~10 minute intervals).
     Use: Window management decisions, indoor/outdoor PM2.5 comparison.
     Primary Key: (observation_time, ndp_id)';

-- =============================================================================
-- SECTION 6: Schema Version Tracking
-- =============================================================================

CREATE TABLE IF NOT EXISTS silver.schema_version (
    version         TEXT PRIMARY KEY,
    applied_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    description     TEXT
);

INSERT INTO silver.schema_version (version, description)
VALUES ('001', 'Initial Silver layer schema with 4 hypertables')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- Summary
-- =============================================================================
-- Tables created:
--   - silver.air_quality_observations (indoor AQ from AirGradient)
--   - silver.weather_observations (NWS + OWM merged weather)
--   - silver.weather_forecasts (NWS forecasts with lead_time_hours)
--   - silver.outdoor_air_quality (OWM outdoor AQ)
--   - silver.schema_version (migration tracking)
--
-- All tables:
--   - Converted to hypertables with 1-day chunk interval
--   - Include dq_flags TEXT[] column for DQ transparency
--   - Use (observation_time/valid_time, ndp_id) as primary key
--   - Follow ADR-006-003 naming convention (silver.{domain}_{entity_type})
-- =============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Silver schema migration 001 completed successfully';
END $$;
