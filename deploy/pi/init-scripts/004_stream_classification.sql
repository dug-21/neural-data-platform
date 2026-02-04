-- ============================================================================
-- MIGRATION: 004_stream_classification.sql
-- Feature: FE-001 Phase B v11-002 (Classification Propagation)
-- Author: NDP Agent (ndp-timescale-dev)
-- Date: 2026-02-04
--
-- Creates stream_classification table for Gold layer correlation analysis.
-- Stores stream type classification and derived correlation roles.
-- Idempotent: Safe to run multiple times (IF NOT EXISTS, CREATE OR REPLACE)
-- ============================================================================

-- Ensure schema exists
CREATE SCHEMA IF NOT EXISTS data_dictionary;

-- ============================================================================
-- TABLE: stream_classification
-- Purpose: Stream type classifications for Gold layer correlation analysis
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.stream_classification (
    -- Primary key: stream identifier (foreign key to streams table)
    stream_id           TEXT PRIMARY KEY
                        REFERENCES data_dictionary.streams(stream_id)
                        ON DELETE CASCADE,

    -- Stream type classification
    -- observation: Continuous numeric readings (PM2.5, temperature)
    -- state_event: Binary/discrete state changes (door open/close)
    -- forecast: Future predictions from external source (NWS)
    -- dimension: Slowly changing reference data
    stream_type         TEXT NOT NULL
                        CHECK (stream_type IN ('observation', 'state_event', 'forecast', 'dimension')),

    -- Correlation role derived from stream_type
    -- effect: Observation data (what we're trying to explain)
    -- cause: State events (what triggers changes)
    -- context: Forecast data (environmental context)
    -- metadata: Dimension data (reference information)
    correlation_role    TEXT NOT NULL
                        CHECK (correlation_role IN ('effect', 'cause', 'context', 'metadata')),

    -- NULL handling strategy derived from stream_type
    -- preserve: Keep NULLs as-is (observation, forecast)
    -- carry_forward: Last Observation Carried Forward (state_event, dimension)
    null_handling       TEXT NOT NULL
                        CHECK (null_handling IN ('preserve', 'carry_forward')),

    -- Optional description
    description         TEXT,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- INDEXES: Optimize common query patterns
-- ============================================================================

-- Index for filtering by stream type
CREATE INDEX IF NOT EXISTS idx_stream_classification_type
    ON data_dictionary.stream_classification(stream_type);

-- Index for filtering by correlation role
CREATE INDEX IF NOT EXISTS idx_stream_classification_role
    ON data_dictionary.stream_classification(correlation_role);

-- ============================================================================
-- COMMENTS: Documentation
-- ============================================================================

COMMENT ON TABLE data_dictionary.stream_classification IS
    'Stream type classifications for Gold layer correlation analysis (FE-001 v11-002)';

COMMENT ON COLUMN data_dictionary.stream_classification.stream_type IS
    'Classification: observation, state_event, forecast, dimension';

COMMENT ON COLUMN data_dictionary.stream_classification.correlation_role IS
    'Derived role: effect, cause, context, metadata';

COMMENT ON COLUMN data_dictionary.stream_classification.null_handling IS
    'NULL strategy: preserve (keep NULLs) or carry_forward (LOCF)';

-- ============================================================================
-- EXTEND gold_tables: Add source_stream_type column
-- ============================================================================

-- Create gold_tables if it doesn't exist (for idempotency)
CREATE TABLE IF NOT EXISTS data_dictionary.gold_tables (
    -- Primary key: fully-qualified table name (e.g., 'gold.air_quality_hourly')
    table_name          TEXT PRIMARY KEY,

    -- Object type: continuous_aggregate, materialized_view, aligned_view
    object_type         TEXT NOT NULL DEFAULT 'continuous_aggregate',

    -- Source Silver table
    source_silver_table TEXT,

    -- Source stream type (for correlation analysis)
    source_stream_type  TEXT
                        CHECK (source_stream_type IS NULL OR
                               source_stream_type IN ('observation', 'state_event', 'forecast', 'dimension')),

    -- Granularity (1 hour, 1 day, etc.)
    granularity         TEXT,

    -- Human-readable description
    description         TEXT,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add source_stream_type column if it doesn't exist (for existing installations)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'gold_tables'
        AND column_name = 'source_stream_type'
    ) THEN
        ALTER TABLE data_dictionary.gold_tables
        ADD COLUMN source_stream_type TEXT
            CHECK (source_stream_type IS NULL OR
                   source_stream_type IN ('observation', 'state_event', 'forecast', 'dimension'));
    END IF;
END $$;

COMMENT ON TABLE data_dictionary.gold_tables IS
    'Metadata for Gold layer tables and views (FE-001)';

COMMENT ON COLUMN data_dictionary.gold_tables.source_stream_type IS
    'Stream type of the source stream for correlation analysis';

-- ============================================================================
-- INDEX: gold_tables by stream type
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_gold_tables_stream_type
    ON data_dictionary.gold_tables(source_stream_type);

-- ============================================================================
-- VIEW: v_stream_classification_summary
-- Purpose: Summary view for stream classifications with counts
-- ============================================================================

CREATE OR REPLACE VIEW data_dictionary.v_stream_classification_summary AS
SELECT
    sc.stream_type,
    sc.correlation_role,
    sc.null_handling,
    COUNT(*) AS stream_count,
    ARRAY_AGG(sc.stream_id ORDER BY sc.stream_id) AS streams
FROM data_dictionary.stream_classification sc
GROUP BY sc.stream_type, sc.correlation_role, sc.null_handling
ORDER BY sc.stream_type;

COMMENT ON VIEW data_dictionary.v_stream_classification_summary IS
    'Summary of stream classifications by type with stream lists';

-- ============================================================================
-- VIEW: v_correlation_candidates
-- Purpose: Show potential cause-effect pairs for correlation analysis
-- ============================================================================

CREATE OR REPLACE VIEW data_dictionary.v_correlation_candidates AS
SELECT
    e.stream_id AS effect_stream,
    c.stream_id AS cause_stream,
    e.stream_type AS effect_type,
    c.stream_type AS cause_type
FROM data_dictionary.stream_classification e
CROSS JOIN data_dictionary.stream_classification c
WHERE e.correlation_role = 'effect'
  AND c.correlation_role = 'cause';

COMMENT ON VIEW data_dictionary.v_correlation_candidates IS
    'Potential cause-effect stream pairs for V1.2 correlation analysis';

-- ============================================================================
-- FUNCTION: derive_correlation_role
-- Purpose: Derive correlation role from stream type
-- ============================================================================

CREATE OR REPLACE FUNCTION data_dictionary.derive_correlation_role(
    p_stream_type TEXT
) RETURNS TEXT AS $$
BEGIN
    RETURN CASE p_stream_type
        WHEN 'observation' THEN 'effect'
        WHEN 'state_event' THEN 'cause'
        WHEN 'forecast' THEN 'context'
        WHEN 'dimension' THEN 'metadata'
        ELSE 'unknown'
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION data_dictionary.derive_correlation_role IS
    'Derive correlation role from stream type (FE-001)';

-- ============================================================================
-- FUNCTION: derive_null_handling
-- Purpose: Derive NULL handling strategy from stream type
-- ============================================================================

CREATE OR REPLACE FUNCTION data_dictionary.derive_null_handling(
    p_stream_type TEXT
) RETURNS TEXT AS $$
BEGIN
    RETURN CASE p_stream_type
        WHEN 'observation' THEN 'preserve'
        WHEN 'state_event' THEN 'carry_forward'
        WHEN 'forecast' THEN 'preserve'
        WHEN 'dimension' THEN 'carry_forward'
        ELSE 'preserve'
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION data_dictionary.derive_null_handling IS
    'Derive NULL handling strategy from stream type (FE-001)';

-- ============================================================================
-- FUNCTION: sync_stream_classification
-- Purpose: Sync a single stream's classification to data dictionary
-- ============================================================================

CREATE OR REPLACE FUNCTION data_dictionary.sync_stream_classification(
    p_stream_id TEXT,
    p_stream_type TEXT,
    p_description TEXT DEFAULT NULL
) RETURNS VOID AS $$
DECLARE
    v_correlation_role TEXT;
    v_null_handling TEXT;
BEGIN
    -- Derive role and null handling from stream type
    v_correlation_role := data_dictionary.derive_correlation_role(p_stream_type);
    v_null_handling := data_dictionary.derive_null_handling(p_stream_type);

    -- Upsert classification
    INSERT INTO data_dictionary.stream_classification
        (stream_id, stream_type, correlation_role, null_handling, description)
    VALUES
        (p_stream_id, p_stream_type, v_correlation_role, v_null_handling, p_description)
    ON CONFLICT (stream_id) DO UPDATE SET
        stream_type = EXCLUDED.stream_type,
        correlation_role = EXCLUDED.correlation_role,
        null_handling = EXCLUDED.null_handling,
        description = COALESCE(EXCLUDED.description, data_dictionary.stream_classification.description),
        updated_at = NOW();
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION data_dictionary.sync_stream_classification IS
    'Sync stream classification with automatic role/null derivation (FE-001 v11-002)';

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Stream Classification schema created successfully (FE-001 v11-002)';
    RAISE NOTICE 'Tables created: stream_classification, gold_tables (if not exists)';
    RAISE NOTICE 'Views created: v_stream_classification_summary, v_correlation_candidates';
    RAISE NOTICE 'Functions created: derive_correlation_role, derive_null_handling, sync_stream_classification';
END $$;
