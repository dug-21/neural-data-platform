-- =============================================================================
-- Neural Data Platform - Dimension Sync Functions
-- =============================================================================
-- Feature: dp-013 - CSV Source Type & Dimension Tables
-- Version: 1.0.0
-- Date: 2026-01-30
-- Author: ndp-timescale-dev
--
-- Purpose: Utility functions for dimension table management, including
--          truncate-and-load strategy, upsert operations, and metadata queries.
--
-- Related: dp-013 SCOPE.md, entity_context.sql
-- =============================================================================

-- Ensure silver schema exists
CREATE SCHEMA IF NOT EXISTS silver;

-- =============================================================================
-- SECTION 1: Dimension Metadata Table
-- =============================================================================
-- Tracks dimension sync history and metadata

CREATE TABLE IF NOT EXISTS silver.dimension_sync_log (
    id                  SERIAL PRIMARY KEY,
    dimension_id        TEXT NOT NULL,
    table_name          TEXT NOT NULL,
    schema_name         TEXT NOT NULL DEFAULT 'silver',
    sync_started_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sync_completed_at   TIMESTAMPTZ,
    strategy            TEXT NOT NULL,          -- 'truncate_and_load' or 'upsert'
    status              TEXT NOT NULL DEFAULT 'running', -- 'running', 'success', 'failed'
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

COMMENT ON TABLE silver.dimension_sync_log IS
    'Audit log for dimension table sync operations.
     Tracks each sync operation with before/after row counts and status.';

-- =============================================================================
-- SECTION 2: Truncate and Load Function
-- =============================================================================
-- Core function for clean replacement of dimension data

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

    -- Get current row count before truncate
    EXECUTE format(
        'SELECT COUNT(*)::INTEGER FROM %I.%I',
        p_schema_name, p_table_name
    ) INTO v_row_count;

    -- Truncate the table (faster than DELETE, resets sequences)
    EXECUTE format('TRUNCATE TABLE %I.%I', p_schema_name, p_table_name);

    v_elapsed_ms := EXTRACT(EPOCH FROM (clock_timestamp() - v_start_time)) * 1000;

    RETURN QUERY SELECT v_row_count, v_elapsed_ms;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION silver.truncate_and_load_dimension IS
    'Truncates a dimension table as first step of truncate-and-load strategy.
     Returns number of rows deleted and execution time in milliseconds.
     Usage: SELECT * FROM silver.truncate_and_load_dimension(''entity_context'');';

-- =============================================================================
-- SECTION 3: Dimension Metadata Function
-- =============================================================================
-- Returns metadata about a dimension table

CREATE OR REPLACE FUNCTION silver.get_dimension_info(
    p_dimension_id TEXT
)
RETURNS TABLE (
    dimension_id TEXT,
    table_name TEXT,
    schema_name TEXT,
    row_count BIGINT,
    last_sync_at TIMESTAMPTZ,
    last_sync_status TEXT,
    columns JSONB
) AS $$
DECLARE
    v_table_name TEXT;
    v_schema_name TEXT;
    v_row_count BIGINT;
    v_last_sync RECORD;
    v_columns JSONB;
BEGIN
    -- Dimension ID to table name mapping (convention: dimension_id = table_name)
    v_table_name := p_dimension_id;
    v_schema_name := 'silver';

    -- Check if table exists
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = v_schema_name
          AND table_name = v_table_name
    ) THEN
        RAISE EXCEPTION 'Dimension table %.% does not exist', v_schema_name, v_table_name;
    END IF;

    -- Get row count
    EXECUTE format('SELECT COUNT(*)::BIGINT FROM %I.%I', v_schema_name, v_table_name)
    INTO v_row_count;

    -- Get last sync info
    SELECT dsl.sync_completed_at, dsl.status
    INTO v_last_sync
    FROM silver.dimension_sync_log dsl
    WHERE dsl.dimension_id = p_dimension_id
      AND dsl.sync_completed_at IS NOT NULL
    ORDER BY dsl.sync_completed_at DESC
    LIMIT 1;

    -- Get column metadata
    SELECT jsonb_agg(
        jsonb_build_object(
            'name', column_name,
            'type', data_type,
            'nullable', is_nullable = 'YES'
        ) ORDER BY ordinal_position
    )
    INTO v_columns
    FROM information_schema.columns
    WHERE table_schema = v_schema_name
      AND table_name = v_table_name;

    RETURN QUERY SELECT
        p_dimension_id,
        v_table_name,
        v_schema_name,
        v_row_count,
        v_last_sync.sync_completed_at,
        v_last_sync.status,
        v_columns;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION silver.get_dimension_info IS
    'Returns metadata about a dimension table including row count, last sync, and columns.
     Usage: SELECT * FROM silver.get_dimension_info(''entity_context'');';

-- =============================================================================
-- SECTION 4: List All Dimensions Function
-- =============================================================================
-- Returns list of all dimension tables in silver schema

CREATE OR REPLACE FUNCTION silver.list_dimensions()
RETURNS TABLE (
    dimension_id TEXT,
    table_name TEXT,
    row_count BIGINT,
    last_sync_at TIMESTAMPTZ,
    last_sync_status TEXT
) AS $$
BEGIN
    RETURN QUERY
    WITH dimension_tables AS (
        -- Convention: dimension tables have specific naming patterns
        SELECT t.table_name
        FROM information_schema.tables t
        WHERE t.table_schema = 'silver'
          AND t.table_type = 'BASE TABLE'
          AND t.table_name IN ('entity_context')  -- Expand as new dimensions are added
    ),
    row_counts AS (
        SELECT
            dt.table_name,
            (xpath('//row-count/text()', query_to_xml(
                format('SELECT COUNT(*) AS row_count FROM silver.%I', dt.table_name),
                false, true, ''
            )))[1]::text::bigint AS row_count
        FROM dimension_tables dt
    ),
    last_syncs AS (
        SELECT DISTINCT ON (dsl.dimension_id)
            dsl.dimension_id,
            dsl.sync_completed_at,
            dsl.status
        FROM silver.dimension_sync_log dsl
        WHERE dsl.sync_completed_at IS NOT NULL
        ORDER BY dsl.dimension_id, dsl.sync_completed_at DESC
    )
    SELECT
        rc.table_name AS dimension_id,
        rc.table_name,
        rc.row_count,
        ls.sync_completed_at AS last_sync_at,
        ls.status AS last_sync_status
    FROM row_counts rc
    LEFT JOIN last_syncs ls ON rc.table_name = ls.dimension_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION silver.list_dimensions IS
    'Returns list of all dimension tables with row counts and last sync status.
     Usage: SELECT * FROM silver.list_dimensions();';

-- =============================================================================
-- SECTION 5: Start Sync Tracking Function
-- =============================================================================
-- Records the start of a dimension sync operation

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
    -- Get current row count
    EXECUTE format(
        'SELECT COUNT(*)::INTEGER FROM silver.%I',
        p_table_name
    ) INTO v_rows_before;

    -- Insert sync log entry
    INSERT INTO silver.dimension_sync_log (
        dimension_id, table_name, strategy, rows_before, source_file
    ) VALUES (
        p_dimension_id, p_table_name, p_strategy, v_rows_before, p_source_file
    )
    RETURNING id INTO v_sync_id;

    RETURN v_sync_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION silver.start_dimension_sync IS
    'Records the start of a dimension sync operation and returns sync_id for tracking.
     Usage: SELECT silver.start_dimension_sync(''entity_context'', ''entity_context'', ''truncate_and_load'', ''entity_context.csv'');';

-- =============================================================================
-- SECTION 6: Complete Sync Tracking Function
-- =============================================================================
-- Records the completion of a dimension sync operation

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
    -- Get table name from sync log
    SELECT table_name INTO v_table_name
    FROM silver.dimension_sync_log
    WHERE id = p_sync_id;

    -- Get current row count
    EXECUTE format(
        'SELECT COUNT(*)::INTEGER FROM silver.%I',
        v_table_name
    ) INTO v_rows_after;

    -- Update sync log
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

COMMENT ON FUNCTION silver.complete_dimension_sync IS
    'Records the completion of a dimension sync operation.
     Usage: PERFORM silver.complete_dimension_sync(1, ''success'', 50, 0, 0);';

-- =============================================================================
-- SECTION 7: Upsert Helper Function
-- =============================================================================
-- Generic upsert function for dimension tables with single-column primary key

CREATE OR REPLACE FUNCTION silver.upsert_dimension_row(
    p_table_name TEXT,
    p_pk_column TEXT,
    p_pk_value TEXT,
    p_data JSONB
)
RETURNS TEXT AS $$
DECLARE
    v_columns TEXT[];
    v_values TEXT[];
    v_updates TEXT[];
    v_key TEXT;
    v_value TEXT;
    v_result TEXT;
BEGIN
    -- Build column/value arrays from JSONB
    FOR v_key, v_value IN
        SELECT key, value::TEXT FROM jsonb_each_text(p_data)
    LOOP
        v_columns := array_append(v_columns, format('%I', v_key));
        v_values := array_append(v_values, format('%L', v_value));
        IF v_key != p_pk_column THEN
            v_updates := array_append(v_updates, format('%I = EXCLUDED.%I', v_key, v_key));
        END IF;
    END LOOP;

    -- Execute upsert
    EXECUTE format(
        'INSERT INTO silver.%I (%s) VALUES (%s)
         ON CONFLICT (%I) DO UPDATE SET %s, updated_at = NOW()
         RETURNING %I',
        p_table_name,
        array_to_string(v_columns, ', '),
        array_to_string(v_values, ', '),
        p_pk_column,
        array_to_string(v_updates, ', '),
        p_pk_column
    ) INTO v_result;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION silver.upsert_dimension_row IS
    'Upserts a single row into a dimension table using JSONB data.
     Usage: SELECT silver.upsert_dimension_row(''entity_context'', ''ndp_id'', ''temp_living'',
            ''{"ndp_id": "temp_living", "category": "temperature", "friendly_name": "Living Room Temp"}''::jsonb);';

-- =============================================================================
-- Schema Version Entry
-- =============================================================================

INSERT INTO silver.schema_version (version, description)
VALUES ('002-sync-functions', 'Dimension sync utility functions for dp-013')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- Summary
-- =============================================================================
-- Tables created:
--   - silver.dimension_sync_log (audit trail for sync operations)
--
-- Functions created:
--   - silver.truncate_and_load_dimension(table, schema) - Truncate for clean load
--   - silver.get_dimension_info(dimension_id) - Metadata about a dimension
--   - silver.list_dimensions() - List all dimension tables
--   - silver.start_dimension_sync(...) - Record sync start
--   - silver.complete_dimension_sync(...) - Record sync completion
--   - silver.upsert_dimension_row(...) - Generic upsert helper
-- =============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Dimension sync functions created successfully';
END $$;
