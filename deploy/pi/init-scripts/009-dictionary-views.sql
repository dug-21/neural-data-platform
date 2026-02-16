-- ops-008: Layer 0 Foundation — All Data Dictionary Views and Functions
-- Consolidated from: 01-create-data-dictionary.sql, 003_silver_data_dictionary.sql,
--                    004_stream_classification.sql, 005_domain_objectives.sql
-- Run order: 9th (LAST — depends on all tables from 005-008)
-- Idempotent: Yes (CREATE OR REPLACE)

-- ============================================================================
-- VIEWS from 01-create-data-dictionary.sql
-- ============================================================================

CREATE OR REPLACE VIEW data_dictionary.v_data_dictionary AS
SELECT
    s.stream_id,
    es.schema_name,
    es.description AS schema_description,
    es.device_class,
    esa.attribute_name,
    esa.attribute_type,
    esa.unit,
    esa.description AS attribute_description,
    esa.nullable,
    esa.range_min,
    esa.range_max
FROM data_dictionary.streams s
JOIN data_dictionary.entity_schemas es ON s.stream_id = es.stream_id
JOIN data_dictionary.entity_schema_attributes esa ON es.id = esa.schema_id
ORDER BY s.stream_id, es.schema_name, esa.sort_order;

CREATE OR REPLACE VIEW data_dictionary.stream_overview AS
SELECT
    s.stream_id,
    s.description,
    s.version,
    s.enabled,
    s.retention_days,
    COUNT(DISTINCT f.id) AS field_count,
    COUNT(DISTINCT src.id) AS source_count,
    COUNT(DISTINCT es.id) AS schema_count,
    s.created_at,
    s.updated_at
FROM data_dictionary.streams s
LEFT JOIN data_dictionary.fields f ON s.stream_id = f.stream_id
LEFT JOIN data_dictionary.sources src ON s.stream_id = src.stream_id
LEFT JOIN data_dictionary.entity_schemas es ON s.stream_id = es.stream_id
GROUP BY s.stream_id, s.description, s.version, s.enabled,
         s.retention_days, s.created_at, s.updated_at;

-- ============================================================================
-- VIEWS from 003_silver_data_dictionary.sql
-- ============================================================================

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
    (dr.rule_params->>'min')::DOUBLE PRECISION AS range_min,
    (dr.rule_params->>'max')::DOUBLE PRECISION AS range_max
FROM data_dictionary.silver_columns sc
LEFT JOIN data_dictionary.silver_dq_rules dr
    ON sc.table_name = dr.silver_table
    AND sc.column_name = dr.silver_column
    AND dr.rule_name = 'range_check';

COMMENT ON VIEW data_dictionary.v_complete_dictionary IS
    'Unified view of Bronze and Silver column definitions';

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

CREATE OR REPLACE VIEW data_dictionary.v_lineage AS
SELECT
    sl.source_stream,
    sl.source_path AS bronze_path,
    bf.field_type AS bronze_type,
    bf.unit AS bronze_unit,
    sl.silver_table,
    sl.silver_column,
    sc.data_type AS silver_type,
    sc.unit AS silver_unit,
    sl.transformation,
    sc.description AS column_description
FROM data_dictionary.silver_lineage sl
LEFT JOIN data_dictionary.silver_columns sc
    ON sl.silver_table = sc.table_name
    AND sl.silver_column = sc.column_name
LEFT JOIN data_dictionary.fields bf
    ON sl.source_stream = bf.stream_id
    AND SPLIT_PART(sl.source_path, '.', 2) = bf.field_name;

COMMENT ON VIEW data_dictionary.v_lineage IS
    'Full Bronze-to-Silver lineage with metadata from both layers';

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
-- FUNCTIONS from 003_silver_data_dictionary.sql
-- ============================================================================

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
-- VIEWS from 004_stream_classification.sql
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
    'Potential cause-effect stream pairs for correlation analysis';

-- ============================================================================
-- FUNCTIONS from 004_stream_classification.sql
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
    'Derive correlation role from stream type';

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
    'Derive NULL handling strategy from stream type';

CREATE OR REPLACE FUNCTION data_dictionary.sync_stream_classification(
    p_stream_id TEXT,
    p_stream_type TEXT,
    p_description TEXT DEFAULT NULL
) RETURNS VOID AS $$
DECLARE
    v_correlation_role TEXT;
    v_null_handling TEXT;
BEGIN
    v_correlation_role := data_dictionary.derive_correlation_role(p_stream_type);
    v_null_handling := data_dictionary.derive_null_handling(p_stream_type);

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
    'Sync stream classification with automatic role/null derivation';

-- ============================================================================
-- VIEWS from 005_domain_objectives.sql
-- ============================================================================

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
-- FUNCTIONS from 005_domain_objectives.sql
-- ============================================================================

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

CREATE OR REPLACE FUNCTION data_dictionary.check_objective_violation(
    p_condition TEXT,
    p_value NUMERIC,
    p_threshold NUMERIC,
    p_threshold_upper NUMERIC DEFAULT NULL
) RETURNS BOOLEAN AS $$
BEGIN
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
-- Verification
-- ============================================================================

DO $$ BEGIN
  RAISE NOTICE 'NDP init [009]: Data dictionary views and functions created (12 views, 7 functions)';
END $$;
