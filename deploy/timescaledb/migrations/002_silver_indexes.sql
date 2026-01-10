-- =============================================================================
-- Neural Data Platform - Silver Layer Index Migration
-- =============================================================================
-- Migration: 002_silver_indexes.sql
-- Feature: DP-006 - Silver Layer Implementation
-- Version: 1.0.0
-- Date: 2026-01-10
-- Author: ndp-timescale-dev
--
-- This migration creates secondary indexes for query optimization.
-- Follows patterns from research/agenticdataplatform/silver/03-data-dictionary.md
--
-- Run order: 002 (after 001_silver_schema.sql)
-- Idempotent: Yes (uses IF NOT EXISTS)
-- =============================================================================

-- =============================================================================
-- SECTION 1: silver.air_quality_observations Indexes
-- =============================================================================
-- Query patterns:
--   1. Recent readings for specific sensor: WHERE ndp_id = ? ORDER BY time DESC
--   2. Readings by location: WHERE location_path = ? ORDER BY time DESC
--   3. DQ flag queries: WHERE dq_flags @> ARRAY['range_check:pm25']

-- Index for sensor lookups (most common query pattern)
CREATE INDEX IF NOT EXISTS idx_aq_obs_ndp_id
    ON silver.air_quality_observations (ndp_id, observation_time DESC);

-- Index for location-based queries
CREATE INDEX IF NOT EXISTS idx_aq_obs_location
    ON silver.air_quality_observations (location_path, observation_time DESC)
    WHERE location_path IS NOT NULL;

-- GIN index for DQ flag queries (per SPECIFICATION.md FR-013)
CREATE INDEX IF NOT EXISTS idx_aq_obs_dq_flags
    ON silver.air_quality_observations USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

COMMENT ON INDEX silver.idx_aq_obs_ndp_id IS
    'Optimizes: SELECT * FROM air_quality_observations WHERE ndp_id = ? ORDER BY observation_time DESC LIMIT N';

COMMENT ON INDEX silver.idx_aq_obs_dq_flags IS
    'Optimizes: SELECT * FROM air_quality_observations WHERE dq_flags @> ARRAY[''rule_name'']';

-- =============================================================================
-- SECTION 2: silver.weather_observations Indexes
-- =============================================================================
-- Query patterns:
--   1. Recent observations for location: WHERE ndp_id = ? ORDER BY time DESC
--   2. Source provider filtering: WHERE source_provider = 'nws'
--   3. DQ flag queries

-- Index for location + time queries
CREATE INDEX IF NOT EXISTS idx_weather_obs_ndp
    ON silver.weather_observations (ndp_id, observation_time DESC);

-- Index for source provider filtering (useful for NWS vs OWM comparisons)
CREATE INDEX IF NOT EXISTS idx_weather_obs_provider
    ON silver.weather_observations (source_provider, observation_time DESC);

-- GIN index for DQ flag queries
CREATE INDEX IF NOT EXISTS idx_weather_obs_dq_flags
    ON silver.weather_observations USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

COMMENT ON INDEX silver.idx_weather_obs_ndp IS
    'Optimizes: SELECT * FROM weather_observations WHERE ndp_id = ? ORDER BY observation_time DESC';

COMMENT ON INDEX silver.idx_weather_obs_provider IS
    'Optimizes: SELECT * FROM weather_observations WHERE source_provider = ''nws'' ORDER BY observation_time DESC';

-- =============================================================================
-- SECTION 3: silver.weather_forecasts Indexes
-- =============================================================================
-- Query patterns:
--   1. Lead time analysis: WHERE lead_time_hours = ? ORDER BY valid_time DESC
--   2. Latest forecasts: ORDER BY issue_time DESC
--   3. Forecast verification join: ON valid_time = observation_time AND ndp_id
--   4. DQ flag queries

-- Critical index for lead_time analysis (per FR-009)
CREATE INDEX IF NOT EXISTS idx_forecasts_lead_time
    ON silver.weather_forecasts (lead_time_hours, valid_time DESC);

-- Index for joining with observations (forecast verification)
CREATE INDEX IF NOT EXISTS idx_forecasts_valid_ndp
    ON silver.weather_forecasts (valid_time, ndp_id);

-- Index for "latest forecast" queries
CREATE INDEX IF NOT EXISTS idx_forecasts_issue
    ON silver.weather_forecasts (issue_time DESC);

-- Index for ndp_id + time queries
CREATE INDEX IF NOT EXISTS idx_forecasts_ndp
    ON silver.weather_forecasts (ndp_id, valid_time DESC);

-- GIN index for DQ flag queries
CREATE INDEX IF NOT EXISTS idx_forecasts_dq_flags
    ON silver.weather_forecasts USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

COMMENT ON INDEX silver.idx_forecasts_lead_time IS
    'Optimizes: Forecast accuracy analysis by lead time bucket (1h, 6h, 12h, 24h, etc.)';

COMMENT ON INDEX silver.idx_forecasts_valid_ndp IS
    'Optimizes: JOIN weather_forecasts f ON f.valid_time = o.observation_time AND f.ndp_id = o.ndp_id';

COMMENT ON INDEX silver.idx_forecasts_issue IS
    'Optimizes: SELECT * FROM weather_forecasts ORDER BY issue_time DESC (latest forecasts)';

-- =============================================================================
-- SECTION 4: silver.outdoor_air_quality Indexes
-- =============================================================================
-- Query patterns:
--   1. Recent readings for location: WHERE ndp_id = ? ORDER BY time DESC
--   2. Indoor/outdoor comparison: JOIN with air_quality_observations on time bucket
--   3. DQ flag queries

-- Index for location + time queries
CREATE INDEX IF NOT EXISTS idx_outdoor_aq_ndp
    ON silver.outdoor_air_quality (ndp_id, observation_time DESC);

-- GIN index for DQ flag queries
CREATE INDEX IF NOT EXISTS idx_outdoor_aq_dq_flags
    ON silver.outdoor_air_quality USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

COMMENT ON INDEX silver.idx_outdoor_aq_ndp IS
    'Optimizes: SELECT * FROM outdoor_air_quality WHERE ndp_id = ? ORDER BY observation_time DESC';

-- =============================================================================
-- SECTION 5: Update schema version
-- =============================================================================

INSERT INTO silver.schema_version (version, description)
VALUES ('002', 'Secondary indexes for Silver layer tables')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- Summary
-- =============================================================================
-- Indexes created:
--
-- silver.air_quality_observations:
--   - idx_aq_obs_ndp_id: (ndp_id, observation_time DESC)
--   - idx_aq_obs_location: (location_path, observation_time DESC)
--   - idx_aq_obs_dq_flags: GIN(dq_flags)
--
-- silver.weather_observations:
--   - idx_weather_obs_ndp: (ndp_id, observation_time DESC)
--   - idx_weather_obs_provider: (source_provider, observation_time DESC)
--   - idx_weather_obs_dq_flags: GIN(dq_flags)
--
-- silver.weather_forecasts:
--   - idx_forecasts_lead_time: (lead_time_hours, valid_time DESC)
--   - idx_forecasts_valid_ndp: (valid_time, ndp_id)
--   - idx_forecasts_issue: (issue_time DESC)
--   - idx_forecasts_ndp: (ndp_id, valid_time DESC)
--   - idx_forecasts_dq_flags: GIN(dq_flags)
--
-- silver.outdoor_air_quality:
--   - idx_outdoor_aq_ndp: (ndp_id, observation_time DESC)
--   - idx_outdoor_aq_dq_flags: GIN(dq_flags)
-- =============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Silver indexes migration 002 completed successfully';
END $$;
