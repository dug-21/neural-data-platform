-- ============================================================================
-- MIGRATION: 003_silver_data_dictionary.sql
-- Feature: dp-009 (Silver Layer Data Dictionary)
-- Author: NDP Architect
-- Date: 2026-01-16
--
-- Extends data_dictionary schema with Silver layer metadata tables.
-- Idempotent: Safe to run multiple times (IF NOT EXISTS, CREATE OR REPLACE)
-- ============================================================================

-- Ensure schema exists (should already exist from 001_create_data_dictionary.sql)
CREATE SCHEMA IF NOT EXISTS data_dictionary;

-- ============================================================================
-- TABLE: silver_tables
-- Purpose: Metadata for Silver layer tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.silver_tables (
    -- Primary key: fully-qualified table name (e.g., 'silver.weather_observations')
    table_name          TEXT PRIMARY KEY,

    -- PostgreSQL schema name (e.g., 'silver')
    schema_name         TEXT NOT NULL DEFAULT 'silver',

    -- Human-readable description of table purpose
    description         TEXT,

    -- What one row represents (e.g., 'One row per sensor reading')
    grain               TEXT,

    -- Array of Bronze stream IDs that feed this table
    source_streams      TEXT[] NOT NULL DEFAULT '{}',

    -- TimescaleDB hypertable time column
    hypertable_column   TEXT DEFAULT 'observation_time',

    -- TimescaleDB chunk interval (NULL if not a hypertable)
    chunk_interval      INTERVAL,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.silver_tables IS
    'Metadata for Silver layer TimescaleDB tables';
COMMENT ON COLUMN data_dictionary.silver_tables.table_name IS
    'Fully-qualified table name (schema.table)';
COMMENT ON COLUMN data_dictionary.silver_tables.grain IS
    'Describes what one row represents';
COMMENT ON COLUMN data_dictionary.silver_tables.source_streams IS
    'Bronze stream IDs that feed this table';

-- ============================================================================
-- TABLE: silver_columns
-- Purpose: Column-level metadata for Silver tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.silver_columns (
    -- Surrogate primary key
    id                  SERIAL PRIMARY KEY,

    -- Reference to parent Silver table
    table_name          TEXT NOT NULL
                        REFERENCES data_dictionary.silver_tables(table_name)
                        ON DELETE CASCADE,

    -- Column name in Silver table
    column_name         TEXT NOT NULL,

    -- PostgreSQL data type (DOUBLE PRECISION, TIMESTAMPTZ, TEXT, etc.)
    data_type           TEXT NOT NULL,

    -- Measurement unit (celsius, ug/m3, percent, etc.)
    unit                TEXT,

    -- Human-readable description
    description         TEXT,

    -- Whether column allows NULL values
    nullable            BOOLEAN NOT NULL DEFAULT true,

    -- Whether column is part of primary key
    is_primary_key      BOOLEAN NOT NULL DEFAULT false,

    -- Display ordering (lower = first)
    sort_order          INTEGER NOT NULL DEFAULT 0,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Natural key: one definition per column per table
    UNIQUE(table_name, column_name)
);

COMMENT ON TABLE data_dictionary.silver_columns IS
    'Column definitions for Silver layer tables including units and descriptions';
COMMENT ON COLUMN data_dictionary.silver_columns.unit IS
    'Measurement unit (e.g., celsius, ug/m3, percent)';
COMMENT ON COLUMN data_dictionary.silver_columns.data_type IS
    'PostgreSQL data type (DOUBLE PRECISION, TIMESTAMPTZ, TEXT, etc.)';

-- ============================================================================
-- TABLE: silver_lineage
-- Purpose: Track Bronze-to-Silver field mappings (data lineage)
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.silver_lineage (
    -- Surrogate primary key
    id                  SERIAL PRIMARY KEY,

    -- Target Silver table (fully-qualified name)
    silver_table        TEXT NOT NULL,

    -- Target Silver column
    silver_column       TEXT NOT NULL,

    -- Source Bronze stream ID
    source_stream       TEXT NOT NULL,

    -- JSON path in Bronze raw_payload (e.g., 'raw_payload.pm02')
    source_path         TEXT NOT NULL,

    -- Transformation applied (direct, unit_conversion, expression, etc.)
    transformation      TEXT NOT NULL DEFAULT 'direct',

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Natural key: one mapping per (table, column, stream) combination
    -- Allows same column to have multiple sources (multi-stream merge)
    UNIQUE(silver_table, silver_column, source_stream)
);

COMMENT ON TABLE data_dictionary.silver_lineage IS
    'Bronze-to-Silver field mappings for data lineage tracking';
COMMENT ON COLUMN data_dictionary.silver_lineage.source_path IS
    'JSON path in Bronze raw_payload (e.g., raw_payload.main.temp)';
COMMENT ON COLUMN data_dictionary.silver_lineage.transformation IS
    'Transform type: direct, unit_conversion, expression, etc.';

-- ============================================================================
-- TABLE: silver_dq_rules
-- Purpose: Document DQ rules applied during Silver ETL
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.silver_dq_rules (
    -- Surrogate primary key
    id                  SERIAL PRIMARY KEY,

    -- Target Silver table
    silver_table        TEXT NOT NULL,

    -- Target Silver column (NULL for cross-field rules)
    silver_column       TEXT,

    -- Rule identifier (range_check, null_check, cross_field_check, etc.)
    rule_name           TEXT NOT NULL,

    -- Rule parameters as JSONB (min, max, expression, etc.)
    rule_params         JSONB NOT NULL DEFAULT '{}',

    -- Action on violation (flag, reject, clamp, warn)
    action              TEXT NOT NULL DEFAULT 'flag',

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
    -- Note: Unique constraint handled by index below (COALESCE not allowed in UNIQUE)
);

-- Unique index to handle NULL silver_column for cross-field rules
CREATE UNIQUE INDEX IF NOT EXISTS idx_silver_dq_rules_unique
    ON data_dictionary.silver_dq_rules(silver_table, COALESCE(silver_column, ''), rule_name);

COMMENT ON TABLE data_dictionary.silver_dq_rules IS
    'Data quality rules applied during Bronze-to-Silver ETL';
COMMENT ON COLUMN data_dictionary.silver_dq_rules.silver_column IS
    'Target column (NULL for cross-field rules)';
COMMENT ON COLUMN data_dictionary.silver_dq_rules.rule_params IS
    'Rule parameters (e.g., {"min": 0, "max": 100})';
COMMENT ON COLUMN data_dictionary.silver_dq_rules.action IS
    'Action on violation: flag, reject, clamp, warn';

-- ============================================================================
-- INDEXES
-- Purpose: Optimize common query patterns
-- ============================================================================

-- silver_columns: lookup by table
CREATE INDEX IF NOT EXISTS idx_silver_columns_table
    ON data_dictionary.silver_columns(table_name);

-- silver_columns: lookup by column name (for "find all temperature columns")
CREATE INDEX IF NOT EXISTS idx_silver_columns_column_name
    ON data_dictionary.silver_columns(column_name);

-- silver_lineage: lookup by Silver table
CREATE INDEX IF NOT EXISTS idx_silver_lineage_table
    ON data_dictionary.silver_lineage(silver_table);

-- silver_lineage: lookup by source stream (for "what does air-quality feed?")
CREATE INDEX IF NOT EXISTS idx_silver_lineage_stream
    ON data_dictionary.silver_lineage(source_stream);

-- silver_lineage: lookup by Silver column (for "where does pm25 come from?")
CREATE INDEX IF NOT EXISTS idx_silver_lineage_column
    ON data_dictionary.silver_lineage(silver_column);

-- silver_dq_rules: lookup by table
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_table
    ON data_dictionary.silver_dq_rules(silver_table);

-- silver_dq_rules: lookup by column
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_column
    ON data_dictionary.silver_dq_rules(silver_column);

-- silver_dq_rules: lookup by rule type
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_name
    ON data_dictionary.silver_dq_rules(rule_name);

-- silver_dq_rules: GIN index for JSONB params queries
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_params
    ON data_dictionary.silver_dq_rules USING GIN (rule_params);

-- ============================================================================
-- VIEWS: Unified Dictionary Access
-- ============================================================================

-- ----------------------------------------------------------------------------
-- VIEW: v_complete_dictionary
-- Purpose: Unified view of Bronze and Silver column definitions
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_complete_dictionary AS
SELECT
    'bronze' AS layer,
    stream_id AS entity,
    field_name AS column_name,
    field_type AS data_type,
    unit,
    description,
    nullable,
    validation_min AS range_min,
    validation_max AS range_max
FROM data_dictionary.fields

UNION ALL

SELECT
    'silver' AS layer,
    sc.table_name AS entity,
    sc.column_name,
    sc.data_type,
    sc.unit,
    sc.description,
    sc.nullable,
    -- Extract range from DQ rules if available
    (dr.rule_params->>'min')::DOUBLE PRECISION AS range_min,
    (dr.rule_params->>'max')::DOUBLE PRECISION AS range_max
FROM data_dictionary.silver_columns sc
LEFT JOIN data_dictionary.silver_dq_rules dr
    ON sc.table_name = dr.silver_table
    AND sc.column_name = dr.silver_column
    AND dr.rule_name = 'range_check';

COMMENT ON VIEW data_dictionary.v_complete_dictionary IS
    'Unified view of Bronze and Silver column definitions';

-- ----------------------------------------------------------------------------
-- VIEW: v_silver_table_overview
-- Purpose: Silver table summary with counts
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_silver_table_overview AS
SELECT
    st.table_name,
    st.schema_name,
    st.description,
    st.grain,
    st.source_streams,
    st.hypertable_column,
    COUNT(DISTINCT sc.id) AS column_count,
    COUNT(DISTINCT sl.id) AS lineage_count,
    COUNT(DISTINCT sr.id) AS dq_rule_count,
    st.created_at,
    st.updated_at
FROM data_dictionary.silver_tables st
LEFT JOIN data_dictionary.silver_columns sc ON st.table_name = sc.table_name
LEFT JOIN data_dictionary.silver_lineage sl ON st.table_name = sl.silver_table
LEFT JOIN data_dictionary.silver_dq_rules sr ON st.table_name = sr.silver_table
GROUP BY st.table_name, st.schema_name, st.description, st.grain,
         st.source_streams, st.hypertable_column, st.created_at, st.updated_at;

COMMENT ON VIEW data_dictionary.v_silver_table_overview IS
    'Silver table summary with column, lineage, and DQ rule counts';

-- ----------------------------------------------------------------------------
-- VIEW: v_lineage
-- Purpose: Full lineage from Bronze to Silver with metadata
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_lineage AS
SELECT
    -- Source (Bronze)
    sl.source_stream,
    sl.source_path AS bronze_path,
    bf.field_type AS bronze_type,
    bf.unit AS bronze_unit,

    -- Target (Silver)
    sl.silver_table,
    sl.silver_column,
    sc.data_type AS silver_type,
    sc.unit AS silver_unit,

    -- Transformation
    sl.transformation,

    -- Metadata
    sc.description AS column_description
FROM data_dictionary.silver_lineage sl
-- Join to Silver column metadata
LEFT JOIN data_dictionary.silver_columns sc
    ON sl.silver_table = sc.table_name
    AND sl.silver_column = sc.column_name
-- Join to Bronze field metadata (extract field name from source_path)
LEFT JOIN data_dictionary.fields bf
    ON sl.source_stream = bf.stream_id
    AND SPLIT_PART(sl.source_path, '.', 2) = bf.field_name;

COMMENT ON VIEW data_dictionary.v_lineage IS
    'Full Bronze-to-Silver lineage with metadata from both layers';

-- ----------------------------------------------------------------------------
-- VIEW: v_dq_rules_summary
-- Purpose: DQ rules with Silver column context
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_dq_rules_summary AS
SELECT
    dr.silver_table,
    dr.silver_column,
    CASE WHEN dr.silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS rule_scope,
    dr.rule_name,
    dr.rule_params,
    dr.action,
    sc.data_type,
    sc.unit
FROM data_dictionary.silver_dq_rules dr
LEFT JOIN data_dictionary.silver_columns sc
    ON dr.silver_table = sc.table_name
    AND dr.silver_column = sc.column_name
ORDER BY dr.silver_table, COALESCE(dr.silver_column, 'zzz'), dr.rule_name;

COMMENT ON VIEW data_dictionary.v_dq_rules_summary IS
    'DQ rules with column context and rule scope indicator';

-- ----------------------------------------------------------------------------
-- VIEW: v_column_search
-- Purpose: Search columns across Bronze and Silver with context
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_column_search AS
SELECT
    'bronze' AS layer,
    f.stream_id AS source,
    f.field_name AS column_name,
    f.field_type AS data_type,
    f.unit,
    f.description,
    NULL::TEXT AS silver_table,
    ARRAY[f.stream_id] AS related_streams
FROM data_dictionary.fields f

UNION ALL

SELECT
    'silver' AS layer,
    sc.table_name AS source,
    sc.column_name,
    sc.data_type,
    sc.unit,
    sc.description,
    sc.table_name AS silver_table,
    st.source_streams AS related_streams
FROM data_dictionary.silver_columns sc
JOIN data_dictionary.silver_tables st ON sc.table_name = st.table_name;

COMMENT ON VIEW data_dictionary.v_column_search IS
    'Searchable view of all columns across Bronze and Silver layers';

-- ============================================================================
-- FUNCTIONS: Helper utilities
-- ============================================================================

-- ----------------------------------------------------------------------------
-- FUNCTION: get_column_lineage
-- Purpose: Get full lineage for a Silver column
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION data_dictionary.get_column_lineage(
    p_table_name TEXT,
    p_column_name TEXT
) RETURNS TABLE (
    source_stream TEXT,
    source_path TEXT,
    transformation TEXT,
    bronze_type TEXT,
    bronze_unit TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        sl.source_stream,
        sl.source_path,
        sl.transformation,
        bf.field_type,
        bf.unit
    FROM data_dictionary.silver_lineage sl
    LEFT JOIN data_dictionary.fields bf
        ON sl.source_stream = bf.stream_id
        AND SPLIT_PART(sl.source_path, '.', 2) = bf.field_name
    WHERE sl.silver_table = p_table_name
      AND sl.silver_column = p_column_name;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION data_dictionary.get_column_lineage IS
    'Get full lineage for a Silver column including Bronze metadata';

-- ----------------------------------------------------------------------------
-- FUNCTION: get_column_dq_rules
-- Purpose: Get all DQ rules for a Silver column
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION data_dictionary.get_column_dq_rules(
    p_table_name TEXT,
    p_column_name TEXT
) RETURNS TABLE (
    rule_name TEXT,
    rule_params JSONB,
    action TEXT,
    rule_scope TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        dr.rule_name,
        dr.rule_params,
        dr.action,
        CASE
            WHEN dr.silver_column IS NULL THEN 'cross-field'
            ELSE 'column'
        END AS rule_scope
    FROM data_dictionary.silver_dq_rules dr
    WHERE dr.silver_table = p_table_name
      AND (dr.silver_column = p_column_name OR dr.silver_column IS NULL);
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION data_dictionary.get_column_dq_rules IS
    'Get all DQ rules for a Silver column including cross-field rules';

-- ============================================================================
-- UPDATE sync_status table to track Silver sync
-- ============================================================================

-- Add Silver-specific counters if not present
DO $$
BEGIN
    -- Add silver_tables_synced column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'silver_tables_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN silver_tables_synced INTEGER DEFAULT 0;
    END IF;

    -- Add silver_columns_synced column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'silver_columns_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN silver_columns_synced INTEGER DEFAULT 0;
    END IF;
END $$;

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Silver Data Dictionary schema created successfully (dp-009)';
    RAISE NOTICE 'Tables created: silver_tables, silver_columns, silver_lineage, silver_dq_rules';
    RAISE NOTICE 'Views created: v_complete_dictionary, v_silver_table_overview, v_lineage, v_dq_rules_summary, v_column_search';
END $$;
