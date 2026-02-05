-- =============================================================================
-- Neural Data Platform - Threshold Crossing Detection Function
-- =============================================================================
-- Feature: v11-012 - Threshold Crossing Generator
-- Version: 1.0.0
-- Date: 2026-02-05
-- Author: ndp-timescale-dev
--
-- Purpose: Detect when observation metrics cross objective thresholds.
--          Returns threshold crossing events with direction (rising/falling).
--
-- Dependencies:
--   - data_dictionary.objectives (from 005_domain_objectives.sql)
--   - gold.air_quality_hourly (continuous aggregate)
--
-- Run order: After events hypertable creation (001_events_hypertable.sql)
-- Idempotent: Yes (CREATE OR REPLACE)
--
-- Objectives Supported (from data_dictionary.objectives):
--   - healthy_co2: co2 < 800 ppm
--   - healthy_pm25: pm25 < 12 ug/m3
--   - comfortable_humidity_min: humidity_pct >= 40%
--   - comfortable_humidity_max: humidity_pct <= 60%
--   - comfortable_temperature_min: temperature_c >= 20C
--   - comfortable_temperature_max: temperature_c <= 24C
--
-- Usage:
--   SELECT * FROM gold.detect_threshold_crossings(NOW() - INTERVAL '2 hours');
-- =============================================================================

\echo '=========================================='
\echo 'NDP Threshold Crossing Detection Function'
\echo '=========================================='

-- Ensure gold schema exists
CREATE SCHEMA IF NOT EXISTS gold;

-- =============================================================================
-- SECTION 1: Crossing Direction Type
-- =============================================================================

-- Create crossing direction enum if not exists
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'crossing_direction_type') THEN
        CREATE TYPE crossing_direction_type AS ENUM (
            'rising',           -- Into violation (was ok, now bad)
            'falling',          -- Out of violation (was bad, now ok)
            'entering_range',   -- Into [min, max] range
            'exiting_range_low',   -- Out of range below min
            'exiting_range_high'   -- Out of range above max
        );
        RAISE NOTICE 'Created crossing_direction_type enum';
    END IF;
END $$;

-- =============================================================================
-- SECTION 2: Threshold Crossing Detection Function
-- =============================================================================

\echo 'Creating gold.detect_threshold_crossings function...'

CREATE OR REPLACE FUNCTION gold.detect_threshold_crossings(
    p_since TIMESTAMPTZ DEFAULT NULL,
    p_domain_id TEXT DEFAULT 'indoor-air-quality'
)
RETURNS TABLE (
    event_time          TIMESTAMPTZ,
    stream_id           TEXT,
    entity_id           TEXT,
    objective_id        TEXT,
    metric              TEXT,
    condition           TEXT,
    threshold_value     DOUBLE PRECISION,
    threshold_min       DOUBLE PRECISION,
    threshold_max       DOUBLE PRECISION,
    unit                TEXT,
    metric_value        DOUBLE PRECISION,
    previous_metric_value DOUBLE PRECISION,
    crossing_direction  TEXT
)
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    v_since TIMESTAMPTZ;
BEGIN
    -- Default to 2 hours ago if not specified
    v_since := COALESCE(p_since, NOW() - INTERVAL '2 hours');

    RETURN QUERY
    WITH
    -- ==========================================================================
    -- Objective thresholds from data_dictionary
    -- Dynamic: reads from objectives table seeded from domain.yaml
    -- ==========================================================================
    objective_thresholds AS (
        SELECT
            o.objective_id::TEXT AS objective_id,
            o.target_stream::TEXT AS stream_id,
            o.target_metric::TEXT AS metric,
            o.condition::TEXT AS condition,
            o.threshold::DOUBLE PRECISION AS threshold,
            o.threshold::DOUBLE PRECISION AS threshold_min,  -- For between, threshold is lower bound
            o.threshold_upper::DOUBLE PRECISION AS threshold_max,  -- For between, threshold_upper is upper bound
            o.unit::TEXT AS unit
        FROM data_dictionary.objectives o
        WHERE o.domain_id = p_domain_id
    ),

    -- ==========================================================================
    -- Air quality observations with LAG for previous values
    -- Partitioned by entity (ndp_id) for per-entity crossing detection
    -- ==========================================================================
    air_quality_observations AS (
        SELECT
            aq.bucket,
            aq.ndp_id AS entity_id,
            'air-quality'::TEXT AS stream_id,
            -- CO2
            aq.co2_mean AS co2_value,
            LAG(aq.co2_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS co2_prev,
            -- PM2.5
            aq.pm25_mean AS pm25_value,
            LAG(aq.pm25_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS pm25_prev,
            -- Temperature
            aq.temperature_c_mean AS temperature_c_value,
            LAG(aq.temperature_c_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS temperature_c_prev,
            -- Humidity
            aq.humidity_pct_mean AS humidity_pct_value,
            LAG(aq.humidity_pct_mean) OVER (PARTITION BY aq.ndp_id ORDER BY aq.bucket) AS humidity_pct_prev
        FROM gold.air_quality_hourly aq
        WHERE aq.bucket > v_since - INTERVAL '1 hour'  -- Need 1 extra hour for LAG
    ),

    -- ==========================================================================
    -- Unpivot observations to metric rows for joining with objectives
    -- ==========================================================================
    observation_metrics AS (
        SELECT
            obs.bucket,
            obs.entity_id,
            obs.stream_id,
            m.metric,
            m.value,
            m.prev_value
        FROM air_quality_observations obs
        CROSS JOIN LATERAL (
            VALUES
                ('co2', obs.co2_value, obs.co2_prev),
                ('pm25', obs.pm25_value, obs.pm25_prev),
                ('temperature_c', obs.temperature_c_value, obs.temperature_c_prev),
                ('humidity_pct', obs.humidity_pct_value, obs.humidity_pct_prev)
        ) AS m(metric, value, prev_value)
        WHERE m.value IS NOT NULL
          AND m.prev_value IS NOT NULL  -- Both values required for crossing detection
          AND obs.bucket > v_since      -- Only buckets after the since parameter
    ),

    -- ==========================================================================
    -- Detect crossings by joining metrics with objectives
    -- ==========================================================================
    crossings AS (
        SELECT
            om.bucket AS event_time,
            om.stream_id,
            om.entity_id,
            ot.objective_id,
            ot.metric,
            ot.condition,
            ot.threshold,
            ot.threshold_min,
            ot.threshold_max,
            ot.unit,
            om.value AS metric_value,
            om.prev_value AS previous_metric_value,
            -- Crossing direction detection based on condition type
            CASE
                -- ==========================================================
                -- Less than condition: value < threshold is healthy
                -- Rising = entering violation (was < threshold, now >= threshold)
                -- Falling = leaving violation (was >= threshold, now < threshold)
                -- ==========================================================
                WHEN ot.condition = '<' THEN
                    CASE
                        WHEN om.prev_value < ot.threshold AND om.value >= ot.threshold
                            THEN 'rising'
                        WHEN om.prev_value >= ot.threshold AND om.value < ot.threshold
                            THEN 'falling'
                        ELSE NULL
                    END

                -- ==========================================================
                -- Less than or equal condition: value <= threshold is healthy
                -- Rising = entering violation (was <= threshold, now > threshold)
                -- Falling = leaving violation (was > threshold, now <= threshold)
                -- ==========================================================
                WHEN ot.condition = '<=' THEN
                    CASE
                        WHEN om.prev_value <= ot.threshold AND om.value > ot.threshold
                            THEN 'rising'
                        WHEN om.prev_value > ot.threshold AND om.value <= ot.threshold
                            THEN 'falling'
                        ELSE NULL
                    END

                -- ==========================================================
                -- Greater than condition: value > threshold is healthy
                -- Rising = entering violation (was > threshold, now <= threshold)
                -- Falling = leaving violation (was <= threshold, now > threshold)
                -- ==========================================================
                WHEN ot.condition = '>' THEN
                    CASE
                        WHEN om.prev_value > ot.threshold AND om.value <= ot.threshold
                            THEN 'rising'
                        WHEN om.prev_value <= ot.threshold AND om.value > ot.threshold
                            THEN 'falling'
                        ELSE NULL
                    END

                -- ==========================================================
                -- Greater than or equal condition: value >= threshold is healthy
                -- Rising = entering violation (was >= threshold, now < threshold)
                -- Falling = leaving violation (was < threshold, now >= threshold)
                -- ==========================================================
                WHEN ot.condition = '>=' THEN
                    CASE
                        WHEN om.prev_value >= ot.threshold AND om.value < ot.threshold
                            THEN 'rising'
                        WHEN om.prev_value < ot.threshold AND om.value >= ot.threshold
                            THEN 'falling'
                        ELSE NULL
                    END

                -- ==========================================================
                -- Between condition: value in [min, max] is healthy
                -- entering_range = was outside, now inside
                -- exiting_range_low = was inside, now below min
                -- exiting_range_high = was inside, now above max
                -- ==========================================================
                WHEN ot.condition = 'between' THEN
                    CASE
                        -- Was outside range, now inside
                        WHEN (om.prev_value < ot.threshold_min OR om.prev_value > ot.threshold_max)
                             AND (om.value >= ot.threshold_min AND om.value <= ot.threshold_max)
                            THEN 'entering_range'
                        -- Was inside range, now below min
                        WHEN (om.prev_value >= ot.threshold_min AND om.prev_value <= ot.threshold_max)
                             AND om.value < ot.threshold_min
                            THEN 'exiting_range_low'
                        -- Was inside range, now above max
                        WHEN (om.prev_value >= ot.threshold_min AND om.prev_value <= ot.threshold_max)
                             AND om.value > ot.threshold_max
                            THEN 'exiting_range_high'
                        ELSE NULL
                    END

                ELSE NULL
            END AS crossing_direction

        FROM observation_metrics om
        JOIN objective_thresholds ot
            ON om.stream_id = ot.stream_id
            AND om.metric = ot.metric
    )

    -- ==========================================================================
    -- Return only actual crossings (where direction is not null)
    -- ==========================================================================
    SELECT
        c.event_time,
        c.stream_id,
        c.entity_id,
        c.objective_id,
        c.metric,
        c.condition,
        c.threshold AS threshold_value,
        c.threshold_min,
        c.threshold_max,
        c.unit,
        c.metric_value,
        c.previous_metric_value,
        c.crossing_direction
    FROM crossings c
    WHERE c.crossing_direction IS NOT NULL
    ORDER BY c.event_time, c.objective_id, c.entity_id;

END;
$$;

COMMENT ON FUNCTION gold.detect_threshold_crossings(TIMESTAMPTZ, TEXT) IS
'Detect threshold crossings for all objectives since a given time.
Compares consecutive hourly readings to detect when metrics cross thresholds.
Returns crossing events with direction (rising/falling/entering_range/exiting_range_*).
Used by gold.detect_events() procedure to populate the events hypertable.

Objectives are loaded from data_dictionary.objectives table (seeded from domain.yaml).

Parameters:
  p_since: Start time for detection (defaults to 2 hours ago)
  p_domain_id: Domain to detect crossings for (defaults to indoor-air-quality)

Crossing Direction Logic:
  For condition "<" (e.g., CO2 < 800):
    - rising: value went from below threshold to at/above threshold (entering violation)
    - falling: value went from at/above threshold to below threshold (leaving violation)

  For condition ">=" (e.g., humidity >= 40):
    - rising: value went from at/above threshold to below threshold (entering violation)
    - falling: value went from below threshold to at/above threshold (leaving violation)

  For condition "between" (e.g., humidity between 40-60):
    - entering_range: value moved from outside [min,max] to inside
    - exiting_range_low: value moved from inside to below min
    - exiting_range_high: value moved from inside to above max
';

-- =============================================================================
-- SECTION 3: Verification and Examples
-- =============================================================================

\echo ''
\echo 'Verification: Function signature'
SELECT
    proname AS function_name,
    pg_get_function_arguments(oid) AS arguments,
    pg_get_function_result(oid) AS return_type
FROM pg_proc
WHERE proname = 'detect_threshold_crossings'
  AND pronamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'gold');

-- Show objectives that will be used
\echo ''
\echo 'Objectives from data_dictionary (for threshold crossing detection):'
SELECT
    objective_id,
    target_stream,
    target_metric,
    condition,
    threshold,
    threshold_upper,
    unit,
    priority
FROM data_dictionary.objectives
WHERE domain_id = 'indoor-air-quality'
ORDER BY priority DESC, objective_id;

\echo ''
\echo '=========================================='
\echo 'Threshold Crossing Detection Function Created'
\echo '=========================================='
\echo ''
\echo 'Usage examples:'
\echo '  -- Detect crossings in last 2 hours (default)'
\echo '  SELECT * FROM gold.detect_threshold_crossings();'
\echo ''
\echo '  -- Detect crossings since specific time'
\echo '  SELECT * FROM gold.detect_threshold_crossings(''2026-02-05 08:00:00+00'');'
\echo ''
\echo '  -- Detect crossings for a specific domain'
\echo '  SELECT * FROM gold.detect_threshold_crossings(NOW() - INTERVAL ''24 hours'', ''indoor-air-quality'');'
\echo ''
\echo '  -- Count crossings by objective and direction'
\echo '  SELECT objective_id, crossing_direction, COUNT(*)'
\echo '  FROM gold.detect_threshold_crossings(NOW() - INTERVAL ''24 hours'')'
\echo '  GROUP BY 1, 2 ORDER BY 1, 2;'
\echo ''
