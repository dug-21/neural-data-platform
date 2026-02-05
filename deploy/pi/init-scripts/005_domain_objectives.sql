-- ============================================================================
-- MIGRATION: 005_domain_objectives.sql
-- Feature: v11-007 (Objectives Storage) - SPEC-C03
-- Author: NDP Rust Developer
-- Date: 2026-02-04
--
-- Adds domain-centric configuration tables to data_dictionary schema:
--   - domains: Parent table for domain configurations
--   - domain_streams: Maps streams to domains with roles
--   - objectives: Target metrics to optimize toward
--   - constraints: Conditions that must be met for actions
--
-- Idempotent: Safe to run multiple times (IF NOT EXISTS, ON CONFLICT)
-- ============================================================================

-- Ensure schema exists (should already exist from 01-create-data-dictionary.sql)
CREATE SCHEMA IF NOT EXISTS data_dictionary;

-- ============================================================================
-- TABLE: domains
-- Purpose: Parent table for domain configurations
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.domains (
    -- Primary key: domain identifier (e.g., 'indoor-air-quality')
    domain_id           TEXT PRIMARY KEY,

    -- Human-readable description of the domain
    description         TEXT,

    -- Number of streams in this domain
    stream_count        INTEGER,

    -- Path to the domain configuration file
    config_path         TEXT,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.domains IS
    'Domain configurations for cross-stream alignment and objectives';
COMMENT ON COLUMN data_dictionary.domains.domain_id IS
    'Unique identifier for the domain (e.g., indoor-air-quality)';
COMMENT ON COLUMN data_dictionary.domains.config_path IS
    'Path to domain.yaml configuration file';

-- ============================================================================
-- TABLE: domain_streams
-- Purpose: Map streams to domains with roles
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.domain_streams (
    -- Composite primary key: (domain_id, stream_id)
    domain_id           TEXT NOT NULL
                        REFERENCES data_dictionary.domains(domain_id)
                        ON DELETE CASCADE,

    -- Stream identifier (may reference non-existent stream for future planning)
    stream_id           TEXT NOT NULL,

    -- Short alias for use in aligned views (e.g., 'indoor', 'outdoor')
    alias               TEXT NOT NULL,

    -- Role of stream in domain
    -- primary: main observation stream
    -- context: provides contextual data (weather, etc.)
    -- actuator: controllable entity (hvac, fans, etc.)
    -- constraint: provides constraint data for actions
    role                TEXT NOT NULL
                        CHECK (role IN ('primary', 'context', 'actuator', 'constraint')),

    PRIMARY KEY (domain_id, stream_id)
);

COMMENT ON TABLE data_dictionary.domain_streams IS
    'Maps streams to domains with roles for alignment and pattern detection';
COMMENT ON COLUMN data_dictionary.domain_streams.alias IS
    'Short alias used in aligned view column prefixes';
COMMENT ON COLUMN data_dictionary.domain_streams.role IS
    'Role: primary (main data), context (environmental), actuator (controllable), constraint (limits)';

-- ============================================================================
-- TABLE: objectives
-- Purpose: Target metrics to optimize toward
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.objectives (
    -- Composite primary key: (domain_id, objective_id)
    objective_id        TEXT NOT NULL,

    domain_id           TEXT NOT NULL
                        REFERENCES data_dictionary.domains(domain_id)
                        ON DELETE CASCADE,

    -- Human-readable description
    description         TEXT,

    -- Target stream and metric
    target_stream       TEXT NOT NULL,
    target_metric       TEXT NOT NULL,

    -- Condition operator
    -- <, >, <=, >=: comparison operators
    -- ==, !=: equality operators
    -- between: range check (requires threshold array)
    condition           TEXT NOT NULL
                        CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=', 'between')),

    -- Threshold value(s)
    -- For single conditions: single numeric value
    -- For 'between': stored as first element, use threshold_upper for second
    threshold           NUMERIC NOT NULL,

    -- Upper threshold for 'between' condition
    threshold_upper     NUMERIC,

    -- Unit of measurement (ppm, ug/m3, celsius, percent, etc.)
    unit                TEXT,

    -- Priority for objective evaluation
    priority            TEXT NOT NULL DEFAULT 'medium'
                        CHECK (priority IN ('low', 'medium', 'high', 'critical')),

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (domain_id, objective_id)
);

COMMENT ON TABLE data_dictionary.objectives IS
    'Target metrics to optimize toward - used for pattern detection and threshold crossing';
COMMENT ON COLUMN data_dictionary.objectives.condition IS
    'Comparison operator: <, >, <=, >=, ==, !=, between';
COMMENT ON COLUMN data_dictionary.objectives.threshold IS
    'Target threshold value (lower bound for between condition)';
COMMENT ON COLUMN data_dictionary.objectives.threshold_upper IS
    'Upper threshold for between condition, NULL for single-value conditions';
COMMENT ON COLUMN data_dictionary.objectives.priority IS
    'Importance: low, medium, high, critical';

-- ============================================================================
-- TABLE: constraints
-- Purpose: Conditions that must be met for actions (V1.3+ action framework)
-- ============================================================================

CREATE TABLE IF NOT EXISTS data_dictionary.constraints (
    -- Composite primary key: (domain_id, constraint_id)
    constraint_id       TEXT NOT NULL,

    domain_id           TEXT NOT NULL
                        REFERENCES data_dictionary.domains(domain_id)
                        ON DELETE CASCADE,

    -- Human-readable description
    description         TEXT,

    -- Constraint stream and metric
    constraint_stream   TEXT NOT NULL,
    constraint_metric   TEXT NOT NULL,

    -- Condition operator (same as objectives)
    condition           TEXT NOT NULL
                        CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),

    -- Threshold value
    threshold           NUMERIC NOT NULL,

    -- Unit of measurement
    unit                TEXT,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (domain_id, constraint_id)
);

COMMENT ON TABLE data_dictionary.constraints IS
    'Conditions that must be met before taking actions (V1.3+ action framework)';
COMMENT ON COLUMN data_dictionary.constraints.condition IS
    'Condition that must be true for action to be allowed';

-- ============================================================================
-- INDEXES
-- Purpose: Optimize common query patterns
-- ============================================================================

-- domain_streams: lookup by domain
CREATE INDEX IF NOT EXISTS idx_domain_streams_domain
    ON data_dictionary.domain_streams(domain_id);

-- domain_streams: lookup by stream (find which domains use a stream)
CREATE INDEX IF NOT EXISTS idx_domain_streams_stream
    ON data_dictionary.domain_streams(stream_id);

-- objectives: lookup by domain
CREATE INDEX IF NOT EXISTS idx_objectives_domain
    ON data_dictionary.objectives(domain_id);

-- objectives: lookup by target stream (for threshold crossing)
CREATE INDEX IF NOT EXISTS idx_objectives_stream
    ON data_dictionary.objectives(target_stream);

-- objectives: lookup by priority (for prioritized evaluation)
CREATE INDEX IF NOT EXISTS idx_objectives_priority
    ON data_dictionary.objectives(priority);

-- objectives: composite for threshold crossing queries
CREATE INDEX IF NOT EXISTS idx_objectives_stream_metric
    ON data_dictionary.objectives(target_stream, target_metric);

-- constraints: lookup by domain
CREATE INDEX IF NOT EXISTS idx_constraints_domain
    ON data_dictionary.constraints(domain_id);

-- constraints: lookup by stream
CREATE INDEX IF NOT EXISTS idx_constraints_stream
    ON data_dictionary.constraints(constraint_stream);

-- ============================================================================
-- VIEWS: Domain Overview
-- ============================================================================

-- ----------------------------------------------------------------------------
-- VIEW: v_domain_overview
-- Purpose: Domain summary with stream and objective counts
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_domain_overview AS
SELECT
    d.domain_id,
    d.description,
    d.stream_count,
    d.config_path,
    COUNT(DISTINCT ds.stream_id) AS actual_stream_count,
    COUNT(DISTINCT o.objective_id) AS objective_count,
    COUNT(DISTINCT c.constraint_id) AS constraint_count,
    d.created_at,
    d.updated_at
FROM data_dictionary.domains d
LEFT JOIN data_dictionary.domain_streams ds ON d.domain_id = ds.domain_id
LEFT JOIN data_dictionary.objectives o ON d.domain_id = o.domain_id
LEFT JOIN data_dictionary.constraints c ON d.domain_id = c.domain_id
GROUP BY d.domain_id, d.description, d.stream_count, d.config_path,
         d.created_at, d.updated_at;

COMMENT ON VIEW data_dictionary.v_domain_overview IS
    'Domain summary with stream, objective, and constraint counts';

-- ----------------------------------------------------------------------------
-- VIEW: v_objectives_with_context
-- Purpose: Objectives with domain and stream context
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_objectives_with_context AS
SELECT
    o.domain_id,
    d.description AS domain_description,
    o.objective_id,
    o.description AS objective_description,
    o.target_stream,
    ds.alias AS stream_alias,
    ds.role AS stream_role,
    o.target_metric,
    o.condition,
    o.threshold,
    o.threshold_upper,
    CASE
        WHEN o.condition = 'between' THEN
            o.condition || ' [' || o.threshold || ', ' || o.threshold_upper || '] ' || COALESCE(o.unit, '')
        ELSE
            o.condition || ' ' || o.threshold || ' ' || COALESCE(o.unit, '')
    END AS condition_display,
    o.unit,
    o.priority,
    o.created_at,
    o.updated_at
FROM data_dictionary.objectives o
JOIN data_dictionary.domains d ON o.domain_id = d.domain_id
LEFT JOIN data_dictionary.domain_streams ds
    ON o.domain_id = ds.domain_id
    AND o.target_stream = ds.stream_id
ORDER BY o.domain_id, o.priority DESC, o.objective_id;

COMMENT ON VIEW data_dictionary.v_objectives_with_context IS
    'Objectives with domain context and human-readable condition display';

-- ----------------------------------------------------------------------------
-- VIEW: v_high_priority_objectives
-- Purpose: Quick access to high/critical priority objectives
-- ----------------------------------------------------------------------------

CREATE OR REPLACE VIEW data_dictionary.v_high_priority_objectives AS
SELECT
    domain_id,
    objective_id,
    target_stream,
    target_metric,
    condition,
    threshold,
    threshold_upper,
    unit,
    priority
FROM data_dictionary.objectives
WHERE priority IN ('high', 'critical')
ORDER BY
    CASE priority
        WHEN 'critical' THEN 1
        WHEN 'high' THEN 2
    END,
    domain_id,
    objective_id;

COMMENT ON VIEW data_dictionary.v_high_priority_objectives IS
    'High and critical priority objectives for pattern detection focus';

-- ============================================================================
-- FUNCTIONS: Helper utilities
-- ============================================================================

-- ----------------------------------------------------------------------------
-- FUNCTION: get_objectives_for_stream
-- Purpose: Get all objectives targeting a specific stream
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION data_dictionary.get_objectives_for_stream(
    p_domain_id TEXT,
    p_stream_id TEXT
) RETURNS TABLE (
    objective_id TEXT,
    target_metric TEXT,
    condition TEXT,
    threshold NUMERIC,
    threshold_upper NUMERIC,
    unit TEXT,
    priority TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        o.objective_id,
        o.target_metric,
        o.condition,
        o.threshold,
        o.threshold_upper,
        o.unit,
        o.priority
    FROM data_dictionary.objectives o
    WHERE o.domain_id = p_domain_id
      AND o.target_stream = p_stream_id
    ORDER BY
        CASE o.priority
            WHEN 'critical' THEN 1
            WHEN 'high' THEN 2
            WHEN 'medium' THEN 3
            WHEN 'low' THEN 4
        END,
        o.objective_id;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION data_dictionary.get_objectives_for_stream IS
    'Get all objectives for a domain targeting a specific stream, ordered by priority';

-- ----------------------------------------------------------------------------
-- FUNCTION: check_objective_violation
-- Purpose: Check if a value violates an objective
-- Returns: true if objective is violated (value does NOT meet condition)
-- ----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION data_dictionary.check_objective_violation(
    p_condition TEXT,
    p_value NUMERIC,
    p_threshold NUMERIC,
    p_threshold_upper NUMERIC DEFAULT NULL
) RETURNS BOOLEAN AS $$
BEGIN
    -- Returns TRUE if the value VIOLATES the objective (does not meet condition)
    RETURN CASE p_condition
        WHEN '<' THEN p_value >= p_threshold
        WHEN '>' THEN p_value <= p_threshold
        WHEN '<=' THEN p_value > p_threshold
        WHEN '>=' THEN p_value < p_threshold
        WHEN '==' THEN p_value != p_threshold
        WHEN '!=' THEN p_value = p_threshold
        WHEN 'between' THEN p_value < p_threshold OR p_value > COALESCE(p_threshold_upper, p_threshold)
        ELSE NULL
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION data_dictionary.check_objective_violation IS
    'Returns TRUE if value violates the objective (does not meet the condition)';

-- ============================================================================
-- UPDATE sync_status table to track domain sync
-- ============================================================================

-- Add domain-specific counters if not present
DO $$
BEGIN
    -- Add domains_synced column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'domains_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN domains_synced INTEGER DEFAULT 0;
    END IF;

    -- Add objectives_synced column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'objectives_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN objectives_synced INTEGER DEFAULT 0;
    END IF;
END $$;

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'Domain objectives schema created successfully (v11-007 SPEC-C03)';
    RAISE NOTICE 'Tables created: domains, domain_streams, objectives, constraints';
    RAISE NOTICE 'Views created: v_domain_overview, v_objectives_with_context, v_high_priority_objectives';
    RAISE NOTICE 'Functions created: get_objectives_for_stream, check_objective_violation';
END $$;
