-- =============================================================================
-- Neural Data Platform - Entity Context Dimension Table
-- =============================================================================
-- Feature: dp-013 - CSV Source Type & Dimension Tables
-- Version: 1.1.0
-- Date: 2026-01-30
-- Author: ndp-timescale-dev
--
-- Purpose: Reference data for enriching Home Assistant state events and
--          sensor observations with human-readable metadata.
--
-- Load Strategy: truncate_and_load (clean replace on each sync)
-- This is a DIMENSION table, not a fact table. It does not flow through
-- Bronze - it loads directly to Silver as configuration/reference data.
--
-- IMPORTANT: Configuration-Driven DDL (DP-013)
-- ---------------------------------------------
-- The core table structure (CREATE TABLE, basic indexes) can be generated
-- dynamically from config/base/dimensions/entity_context.yaml using:
--
--   use platform_core::dimensions::DdlGenerator;
--   let ddl = DdlGenerator::generate_create_table(&config);
--   let indexes = DdlGenerator::generate_indexes(&config);
--
-- This SQL file adds ADVANCED features not covered by config:
--   - Audit columns (created_at, updated_at)
--   - Triggers for automatic timestamp updates
--   - CHECK constraints with validation rules
--   - GIN indexes for TEXT[] array columns (correlates_with)
--   - Table and column COMMENTs
--   - Schema version tracking
--
-- For simple table creation via Rust code, use DdlGenerator.
-- For production deployment with full features, use this SQL file.
--
-- Related: dp-013 SCOPE.md, air-012 (Home Assistant state events)
-- =============================================================================

-- Ensure silver schema exists
CREATE SCHEMA IF NOT EXISTS silver;

-- =============================================================================
-- SECTION 1: Entity Context Dimension Table
-- =============================================================================
-- Grain: One row per entity (ndp_id)
-- Use: LEFT JOIN with fact tables to enrich observations with context

CREATE TABLE IF NOT EXISTS silver.entity_context (
    -- Primary Key
    ndp_id              TEXT PRIMARY KEY,

    -- Core Classification
    category            TEXT NOT NULL,      -- 'temperature', 'humidity', 'door', 'window', etc.
    friendly_name       TEXT NOT NULL,      -- Human-readable name: 'Living Room Temperature'

    -- Location Hierarchy
    location_path       TEXT,               -- Hierarchical path: 'home/living_room'

    -- Correlation Metadata
    correlates_with     TEXT[],             -- Related entity ndp_ids for cross-correlation

    -- Physical Context
    orientation         TEXT,               -- 'north', 'south', 'east', 'west' (for windows/doors)

    -- Audit Columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- SECTION 2: Indexes for Common Query Patterns
-- =============================================================================

-- Index for filtering by category (e.g., all temperature sensors)
CREATE INDEX IF NOT EXISTS idx_entity_context_category
    ON silver.entity_context(category);

-- Index for filtering by location (e.g., all entities in living room)
CREATE INDEX IF NOT EXISTS idx_entity_context_location
    ON silver.entity_context(location_path);

-- Index for correlation lookups
CREATE INDEX IF NOT EXISTS idx_entity_context_correlates
    ON silver.entity_context USING GIN (correlates_with)
    WHERE correlates_with IS NOT NULL;

-- =============================================================================
-- SECTION 3: Table Documentation
-- =============================================================================

COMMENT ON TABLE silver.entity_context IS
    'Entity context dimension for enriching sensor data with human-readable metadata.
     Source: config/base/dimensions/entity_context.yaml + CSV file
     Load Strategy: truncate_and_load (full refresh on each sync)
     Use: LEFT JOIN with silver.air_quality_observations, silver.state_events, etc.
     Primary Key: ndp_id (matches identity column in fact tables)';

COMMENT ON COLUMN silver.entity_context.ndp_id IS
    'Neural Data Platform entity identifier. Matches ndp_id in fact tables.';

COMMENT ON COLUMN silver.entity_context.category IS
    'Entity classification: temperature, humidity, door, window, motion, air_quality, etc.';

COMMENT ON COLUMN silver.entity_context.friendly_name IS
    'Human-readable display name for dashboards and reports.';

COMMENT ON COLUMN silver.entity_context.location_path IS
    'Hierarchical location path using forward slashes: home/living_room, outdoor/backyard';

COMMENT ON COLUMN silver.entity_context.correlates_with IS
    'Array of related ndp_ids for cross-correlation analysis.
     Example: Temperature sensor correlates with nearby humidity sensor.';

COMMENT ON COLUMN silver.entity_context.orientation IS
    'Compass direction for physical entities like windows and doors.
     Values: north, south, east, west, null (for non-directional entities).';

-- =============================================================================
-- SECTION 4: Trigger for Updated Timestamp
-- =============================================================================

CREATE OR REPLACE FUNCTION silver.update_entity_context_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tr_entity_context_updated ON silver.entity_context;
CREATE TRIGGER tr_entity_context_updated
    BEFORE UPDATE ON silver.entity_context
    FOR EACH ROW
    EXECUTE FUNCTION silver.update_entity_context_timestamp();

-- =============================================================================
-- SECTION 5: Validation Constraints
-- =============================================================================

-- Category must be non-empty
ALTER TABLE silver.entity_context
    ADD CONSTRAINT chk_entity_context_category_not_empty
    CHECK (length(trim(category)) > 0);

-- Friendly name must be non-empty
ALTER TABLE silver.entity_context
    ADD CONSTRAINT chk_entity_context_friendly_name_not_empty
    CHECK (length(trim(friendly_name)) > 0);

-- Orientation must be valid compass direction or null
ALTER TABLE silver.entity_context
    ADD CONSTRAINT chk_entity_context_orientation_valid
    CHECK (orientation IS NULL OR orientation IN ('north', 'south', 'east', 'west', 'northeast', 'northwest', 'southeast', 'southwest'));

-- =============================================================================
-- Schema Version Entry
-- =============================================================================

INSERT INTO silver.schema_version (version, description)
VALUES ('002-dimensions', 'Entity context dimension table for dp-013')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- Summary
-- =============================================================================
-- Table created: silver.entity_context
-- Indexes: category, location_path, correlates_with (GIN)
-- Constraints: Non-empty category/friendly_name, valid orientation
-- Trigger: Auto-update updated_at on changes
--
-- Example JOIN usage:
--   SELECT
--       o.observation_time,
--       o.pm25,
--       c.friendly_name,
--       c.location_path
--   FROM silver.air_quality_observations o
--   LEFT JOIN silver.entity_context c USING (ndp_id);
-- =============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Entity context dimension table created successfully';
END $$;
