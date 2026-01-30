-- =============================================================================
-- Neural Data Platform - Dimension Tables Initialization
-- =============================================================================
-- Feature: dp-013 - CSV Source Type & Dimension Tables
-- Version: 1.0.0
-- Date: 2026-01-30
-- Author: ndp-timescale-dev
--
-- Purpose: Master initialization script for all dimension tables.
--          Run this after Silver schema initialization (001_silver_schema.sql).
--
-- Run order: After 001_silver_schema.sql, before any ETL operations
-- Idempotent: Yes (all statements use IF NOT EXISTS)
--
-- Usage:
--   docker exec -i pi5-timescaledb psql -U postgres -d ndp < deploy/pi/sql/dimensions/init.sql
--
-- Or via Docker entrypoint (add to init-scripts with numeric prefix):
--   cp deploy/pi/sql/dimensions/init.sql deploy/pi/init-scripts/04-dimension-tables.sql
-- =============================================================================

\echo '=========================================='
\echo 'NDP Dimension Tables Initialization'
\echo '=========================================='

-- =============================================================================
-- SECTION 1: Prerequisites Check
-- =============================================================================

-- Verify silver schema exists
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'silver') THEN
        RAISE EXCEPTION 'Silver schema does not exist. Run 001_silver_schema.sql first.';
    END IF;
    RAISE NOTICE 'Prerequisite check: silver schema exists';
END $$;

-- Verify schema_version table exists
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'silver' AND table_name = 'schema_version'
    ) THEN
        -- Create it if missing (for backward compatibility)
        CREATE TABLE IF NOT EXISTS silver.schema_version (
            version         TEXT PRIMARY KEY,
            applied_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            description     TEXT
        );
        RAISE NOTICE 'Created silver.schema_version table';
    END IF;
END $$;

-- =============================================================================
-- SECTION 2: Include Sync Functions
-- =============================================================================
-- These functions support all dimension tables

\echo 'Creating dimension sync functions...'

-- Dimension sync log table
CREATE TABLE IF NOT EXISTS silver.dimension_sync_log (
    id                  SERIAL PRIMARY KEY,
    dimension_id        TEXT NOT NULL,
    table_name          TEXT NOT NULL,
    schema_name         TEXT NOT NULL DEFAULT 'silver',
    sync_started_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sync_completed_at   TIMESTAMPTZ,
    strategy            TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'running',
    rows_before         INTEGER,
    rows_inserted       INTEGER,
    rows_updated        INTEGER,
    rows_deleted        INTEGER,
    rows_after          INTEGER,
    source_file         TEXT,
    error_message       TEXT,
    metadata            JSONB
);

CREATE INDEX IF NOT EXISTS idx_dimension_sync_log_dimension
    ON silver.dimension_sync_log(dimension_id, sync_started_at DESC);

CREATE INDEX IF NOT EXISTS idx_dimension_sync_log_status
    ON silver.dimension_sync_log(status, sync_started_at DESC);

-- Truncate and load function
CREATE OR REPLACE FUNCTION silver.truncate_and_load_dimension(
    p_table_name TEXT,
    p_schema_name TEXT DEFAULT 'silver'
)
RETURNS TABLE (
    rows_deleted INTEGER,
    truncate_time_ms DOUBLE PRECISION
) AS $$
DECLARE
    v_start_time TIMESTAMPTZ;
    v_row_count INTEGER;
    v_elapsed_ms DOUBLE PRECISION;
BEGIN
    v_start_time := clock_timestamp();
    EXECUTE format('SELECT COUNT(*)::INTEGER FROM %I.%I', p_schema_name, p_table_name)
    INTO v_row_count;
    EXECUTE format('TRUNCATE TABLE %I.%I', p_schema_name, p_table_name);
    v_elapsed_ms := EXTRACT(EPOCH FROM (clock_timestamp() - v_start_time)) * 1000;
    RETURN QUERY SELECT v_row_count, v_elapsed_ms;
END;
$$ LANGUAGE plpgsql;

-- Start sync tracking
CREATE OR REPLACE FUNCTION silver.start_dimension_sync(
    p_dimension_id TEXT,
    p_table_name TEXT,
    p_strategy TEXT,
    p_source_file TEXT DEFAULT NULL
)
RETURNS INTEGER AS $$
DECLARE
    v_sync_id INTEGER;
    v_rows_before INTEGER;
BEGIN
    EXECUTE format('SELECT COUNT(*)::INTEGER FROM silver.%I', p_table_name)
    INTO v_rows_before;
    INSERT INTO silver.dimension_sync_log (
        dimension_id, table_name, strategy, rows_before, source_file
    ) VALUES (
        p_dimension_id, p_table_name, p_strategy, v_rows_before, p_source_file
    )
    RETURNING id INTO v_sync_id;
    RETURN v_sync_id;
END;
$$ LANGUAGE plpgsql;

-- Complete sync tracking
CREATE OR REPLACE FUNCTION silver.complete_dimension_sync(
    p_sync_id INTEGER,
    p_status TEXT,
    p_rows_inserted INTEGER DEFAULT 0,
    p_rows_updated INTEGER DEFAULT 0,
    p_rows_deleted INTEGER DEFAULT 0,
    p_error_message TEXT DEFAULT NULL
)
RETURNS void AS $$
DECLARE
    v_table_name TEXT;
    v_rows_after INTEGER;
BEGIN
    SELECT table_name INTO v_table_name
    FROM silver.dimension_sync_log
    WHERE id = p_sync_id;
    EXECUTE format('SELECT COUNT(*)::INTEGER FROM silver.%I', v_table_name)
    INTO v_rows_after;
    UPDATE silver.dimension_sync_log
    SET sync_completed_at = NOW(),
        status = p_status,
        rows_inserted = p_rows_inserted,
        rows_updated = p_rows_updated,
        rows_deleted = p_rows_deleted,
        rows_after = v_rows_after,
        error_message = p_error_message
    WHERE id = p_sync_id;
END;
$$ LANGUAGE plpgsql;

\echo 'Dimension sync functions created.'

-- =============================================================================
-- SECTION 3: Entity Context Dimension Table
-- =============================================================================

\echo 'Creating entity_context dimension table...'

CREATE TABLE IF NOT EXISTS silver.entity_context (
    ndp_id              TEXT PRIMARY KEY,
    category            TEXT NOT NULL,
    friendly_name       TEXT NOT NULL,
    location_path       TEXT,
    correlates_with     TEXT[],
    orientation         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_entity_context_category
    ON silver.entity_context(category);

CREATE INDEX IF NOT EXISTS idx_entity_context_location
    ON silver.entity_context(location_path);

CREATE INDEX IF NOT EXISTS idx_entity_context_correlates
    ON silver.entity_context USING GIN (correlates_with)
    WHERE correlates_with IS NOT NULL;

-- Constraints (use DO block for idempotency)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_entity_context_category_not_empty'
    ) THEN
        ALTER TABLE silver.entity_context
            ADD CONSTRAINT chk_entity_context_category_not_empty
            CHECK (length(trim(category)) > 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_entity_context_friendly_name_not_empty'
    ) THEN
        ALTER TABLE silver.entity_context
            ADD CONSTRAINT chk_entity_context_friendly_name_not_empty
            CHECK (length(trim(friendly_name)) > 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_entity_context_orientation_valid'
    ) THEN
        ALTER TABLE silver.entity_context
            ADD CONSTRAINT chk_entity_context_orientation_valid
            CHECK (orientation IS NULL OR orientation IN (
                'north', 'south', 'east', 'west',
                'northeast', 'northwest', 'southeast', 'southwest'
            ));
    END IF;
END $$;

-- Updated timestamp trigger
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

-- Table comment
COMMENT ON TABLE silver.entity_context IS
    'Entity context dimension for enriching sensor data with human-readable metadata.
     Source: config/base/dimensions/entity_context.yaml + CSV file
     Load Strategy: truncate_and_load';

\echo 'Entity context dimension table created.'

-- =============================================================================
-- SECTION 4: Gold Layer View (Optional - Enriched Events)
-- =============================================================================

-- Create gold schema if not exists
CREATE SCHEMA IF NOT EXISTS gold;

-- Example enriched view for future use with state_events table
-- This view will be useful when air-012 (Home Assistant state events) is implemented
CREATE OR REPLACE VIEW gold.events_with_context AS
SELECT
    -- All columns from air_quality_observations
    aq.observation_time,
    aq.ingestion_time,
    aq.ndp_id,
    aq.location_path AS aq_location_path,
    aq.pm25,
    aq.pm10,
    aq.co2,
    aq.temperature_c,
    aq.humidity_pct,
    aq.voc_index,
    aq.nox_index,
    aq.dq_flags,

    -- Enrichment from entity_context
    c.category,
    c.friendly_name,
    c.location_path AS entity_location_path,
    c.correlates_with,
    c.orientation
FROM silver.air_quality_observations aq
LEFT JOIN silver.entity_context c USING (ndp_id);

COMMENT ON VIEW gold.events_with_context IS
    'Air quality observations enriched with entity context dimension.
     Use: Dashboards that need human-readable names and location metadata.
     Join: LEFT JOIN preserves all observations even without context match.';

-- =============================================================================
-- SECTION 5: Schema Version Tracking
-- =============================================================================

INSERT INTO silver.schema_version (version, description)
VALUES ('003-dimensions', 'Dimension tables initialization for dp-013')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- SECTION 6: Verification
-- =============================================================================

\echo ''
\echo '=========================================='
\echo 'Verification'
\echo '=========================================='

-- List created objects
SELECT 'Tables' AS object_type, table_name
FROM information_schema.tables
WHERE table_schema = 'silver'
  AND table_name IN ('entity_context', 'dimension_sync_log')
UNION ALL
SELECT 'Views', table_name
FROM information_schema.views
WHERE table_schema = 'gold'
  AND table_name = 'events_with_context'
UNION ALL
SELECT 'Functions', routine_name
FROM information_schema.routines
WHERE routine_schema = 'silver'
  AND routine_name LIKE '%dimension%'
ORDER BY 1, 2;

\echo ''
\echo '=========================================='
\echo 'NDP Dimension Tables Initialization Complete'
\echo '=========================================='
