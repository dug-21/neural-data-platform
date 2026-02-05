-- =============================================================================
-- Neural Data Platform - Update detect_events Procedure for Full Threshold Crossings
-- =============================================================================
-- Feature: v11-012 - Threshold Crossing Generator (complete implementation)
-- Version: 1.0.0
-- Date: 2026-02-05
-- Author: ndp-timescale-dev
--
-- Purpose: Replace the placeholder threshold crossing detection in detect_events
--          with the full implementation using gold.detect_threshold_crossings().
--
-- This file REPLACES 004_detect_events_procedure.sql with enhanced threshold
-- crossing detection that supports all condition types (<, <=, >, >=, between).
--
-- Run order: After 006_threshold_crossing_detection.sql
-- Idempotent: Yes (CREATE OR REPLACE)
--
-- Dependencies:
--   - gold.events hypertable (from 001_events_hypertable.sql)
--   - gold.detect_threshold_crossings() (from 006_threshold_crossing_detection.sql)
--   - silver.state_events (for state transitions)
--   - gold.indoor_air_quality_aligned (for context capture)
--   - data_dictionary.objectives (for threshold definitions)
--
-- Usage:
--   CALL gold.detect_events(NULL, '{}');
--   -- Or via TimescaleDB scheduled job (automatic)
-- =============================================================================

\echo '=========================================='
\echo 'NDP Update detect_events Procedure (v11-012)'
\echo '=========================================='

-- Ensure gold schema exists
CREATE SCHEMA IF NOT EXISTS gold;

-- =============================================================================
-- SECTION 1: Enhanced Context Capture Helper Function
-- =============================================================================

\echo 'Creating/replacing gold.get_event_context function...'

CREATE OR REPLACE FUNCTION gold.get_event_context(
    p_event_time TIMESTAMPTZ
)
RETURNS JSONB
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    v_context JSONB;
    v_bucket TIMESTAMPTZ;
BEGIN
    -- Get the hourly bucket for the event time
    v_bucket := time_bucket('1 hour', p_event_time);

    -- Capture comprehensive context from aligned view at the event's bucket
    SELECT jsonb_build_object(
        'indoor_co2', a.indoor_co2_mean,
        'indoor_co2_std', a.indoor_co2_std,
        'indoor_pm25', a.indoor_pm25_mean,
        'indoor_pm25_max', a.indoor_pm25_max,
        'indoor_pm25_p95', a.indoor_pm25_p95,
        'indoor_temp', a.indoor_temperature_c_mean,
        'indoor_temp_min', a.indoor_temperature_c_min,
        'indoor_temp_max', a.indoor_temperature_c_max,
        'indoor_humidity', a.indoor_humidity_pct_mean,
        'indoor_humidity_min', a.indoor_humidity_pct_min,
        'indoor_humidity_max', a.indoor_humidity_pct_max,
        'outdoor_temp', a.outdoor_temperature_c_mean,
        'outdoor_humidity', a.outdoor_humidity_pct_mean,
        'window_state', a.state_last_state,
        'indoor_sample_count', a.indoor_sample_count,
        'outdoor_sample_count', a.outdoor_sample_count,
        'bucket', v_bucket
    )
    INTO v_context
    FROM gold.indoor_air_quality_aligned a
    WHERE a.bucket = v_bucket
    LIMIT 1;

    -- Return empty object if no aligned data found
    RETURN COALESCE(v_context, '{}'::JSONB);
END;
$$;

COMMENT ON FUNCTION gold.get_event_context(TIMESTAMPTZ) IS
'Retrieve comprehensive environmental context from aligned view at the events hourly bucket.
Returns JSONB with indoor/outdoor metrics and state for correlation analysis.
Used by detect_events() to capture context at event time.';

-- =============================================================================
-- SECTION 2: Updated detect_events Procedure (Full Threshold Crossings)
-- =============================================================================

\echo 'Creating/replacing gold.detect_events procedure with full threshold crossing support...'

CREATE OR REPLACE PROCEDURE gold.detect_events(
    job_id INT DEFAULT NULL,
    config JSONB DEFAULT NULL
)
LANGUAGE plpgsql AS $$
DECLARE
    v_last_run TIMESTAMPTZ;
    v_now TIMESTAMPTZ := NOW();
    v_state_events_inserted INT := 0;
    v_threshold_events_inserted INT := 0;
    v_lookback INTERVAL := INTERVAL '2 hours';
BEGIN
    -- =========================================================================
    -- STEP 1: Determine last successful run time
    -- =========================================================================

    -- Try to get last_successful_finish from job stats (if running as scheduled job)
    IF job_id IS NOT NULL THEN
        SELECT last_successful_finish INTO v_last_run
        FROM timescaledb_information.job_stats
        WHERE timescaledb_information.job_stats.job_id = detect_events.job_id;
    END IF;

    -- Default to lookback interval if no previous run or manual execution
    v_last_run := COALESCE(v_last_run, v_now - v_lookback);

    RAISE NOTICE 'Event detection starting. Last run: %, Now: %', v_last_run, v_now;

    -- =========================================================================
    -- STEP 2: Detect and insert state transition events
    -- Source: silver.state_events
    -- =========================================================================

    RAISE NOTICE 'Detecting state transitions from silver.state_events...';

    -- Detect state changes in silver.state_events since last run
    -- Use LAG to find the previous state for each entity
    WITH ordered_states AS (
        -- Get all state records for entities with activity since last run
        SELECT
            s.event_time,
            s.ndp_id AS entity_id,
            s.source_stream,
            s.state,
            LAG(s.state) OVER (PARTITION BY s.ndp_id ORDER BY s.event_time) AS prev_state,
            LAG(s.event_time) OVER (PARTITION BY s.ndp_id ORDER BY s.event_time) AS prev_time
        FROM silver.state_events s
        WHERE s.event_time > v_last_run - INTERVAL '1 hour'  -- Look back a bit for LAG context
    ),
    new_transitions AS (
        -- Filter to actual state changes in the detection window
        SELECT
            o.event_time,
            o.entity_id,
            COALESCE(o.source_stream, 'home-assistant-state') AS stream_id,
            o.prev_state AS from_state,
            o.state AS to_state,
            CASE
                WHEN o.prev_time IS NOT NULL THEN
                    EXTRACT(EPOCH FROM (o.event_time - o.prev_time)) * 1000
                ELSE NULL
            END AS duration_ms
        FROM ordered_states o
        WHERE o.event_time > v_last_run
          AND o.prev_state IS NOT NULL
          AND o.prev_state != o.state
    )
    INSERT INTO gold.events (
        event_time,
        stream_id,
        entity_id,
        event_type,
        from_state,
        to_state,
        duration_in_state_ms,
        context,
        details
    )
    SELECT
        t.event_time,
        t.stream_id,
        t.entity_id,
        'state_transition',
        t.from_state,
        t.to_state,
        t.duration_ms::BIGINT,
        gold.get_event_context(t.event_time),
        jsonb_build_object(
            'detection_job_id', job_id,
            'detected_at', v_now,
            'transition_type',
                CASE
                    WHEN t.to_state = 'on' THEN 'activation'
                    WHEN t.to_state = 'off' THEN 'deactivation'
                    ELSE 'state_change'
                END
        )
    FROM new_transitions t
    -- Avoid duplicates: check if event already exists
    WHERE NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = t.event_time
          AND e.entity_id = t.entity_id
          AND e.event_type = 'state_transition'
          AND e.to_state = t.to_state
    );

    GET DIAGNOSTICS v_state_events_inserted = ROW_COUNT;
    RAISE NOTICE 'Inserted % state transition events', v_state_events_inserted;

    -- =========================================================================
    -- STEP 3: Detect and insert threshold crossing events
    -- Source: gold.detect_threshold_crossings() function (v11-012)
    -- Supports: <, <=, >, >=, between conditions
    -- =========================================================================

    RAISE NOTICE 'Detecting threshold crossings via gold.detect_threshold_crossings()...';

    -- Use the dedicated threshold crossing detection function
    -- This supports all condition types from data_dictionary.objectives
    WITH detected_crossings AS (
        SELECT
            tc.event_time,
            tc.stream_id,
            tc.entity_id,
            tc.objective_id,
            tc.metric,
            tc.condition,
            tc.threshold_value,
            tc.threshold_min,
            tc.threshold_max,
            tc.unit,
            tc.metric_value,
            tc.previous_metric_value,
            tc.crossing_direction
        FROM gold.detect_threshold_crossings(v_last_run, 'indoor-air-quality') tc
    )
    INSERT INTO gold.events (
        event_time,
        stream_id,
        entity_id,
        event_type,
        metric,
        threshold_value,
        crossing_direction,
        metric_value,
        previous_metric_value,
        objective_id,
        context,
        details
    )
    SELECT
        c.event_time,
        c.stream_id,
        c.entity_id,
        'threshold_crossing',
        c.metric,
        COALESCE(c.threshold_value, c.threshold_min),  -- Use threshold_min for between
        c.crossing_direction,
        c.metric_value,
        c.previous_metric_value,
        c.objective_id,
        gold.get_event_context(c.event_time),
        jsonb_build_object(
            'detection_job_id', job_id,
            'detected_at', v_now,
            'condition', c.condition,
            'unit', c.unit,
            'threshold', c.threshold_value,
            'threshold_min', c.threshold_min,
            'threshold_max', c.threshold_max,
            'severity',
                CASE c.crossing_direction
                    WHEN 'rising' THEN 'warning'
                    WHEN 'exiting_range_low' THEN 'warning'
                    WHEN 'exiting_range_high' THEN 'warning'
                    WHEN 'falling' THEN 'info'
                    WHEN 'entering_range' THEN 'info'
                    ELSE 'info'
                END,
            'violation_type',
                CASE c.crossing_direction
                    WHEN 'rising' THEN 'entering_violation'
                    WHEN 'exiting_range_low' THEN 'entering_violation'
                    WHEN 'exiting_range_high' THEN 'entering_violation'
                    WHEN 'falling' THEN 'leaving_violation'
                    WHEN 'entering_range' THEN 'leaving_violation'
                    ELSE 'unknown'
                END
        )
    FROM detected_crossings c
    -- Avoid duplicates: check for existing events with same time/entity/objective/direction
    WHERE NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = c.event_time
          AND e.entity_id = c.entity_id
          AND e.event_type = 'threshold_crossing'
          AND e.objective_id = c.objective_id
          AND e.crossing_direction = c.crossing_direction
    );

    GET DIAGNOSTICS v_threshold_events_inserted = ROW_COUNT;
    RAISE NOTICE 'Inserted % threshold crossing events', v_threshold_events_inserted;

    -- =========================================================================
    -- STEP 4: Log summary
    -- =========================================================================

    RAISE NOTICE 'Event detection complete. Total: % (state: %, threshold: %)',
        v_state_events_inserted + v_threshold_events_inserted,
        v_state_events_inserted,
        v_threshold_events_inserted;

    -- Commit the transaction
    COMMIT;
END;
$$;

COMMENT ON PROCEDURE gold.detect_events(INT, JSONB) IS
'Detects and inserts events into gold.events hypertable.
Runs as a TimescaleDB scheduled job every 15 minutes.

Detects:
  1. State transitions from silver.state_events
  2. Threshold crossings using gold.detect_threshold_crossings()

v11-012 Enhancement:
  - Full threshold crossing support for all condition types: <, <=, >, >=, between
  - Crossings read from data_dictionary.objectives
  - Context captured from gold.indoor_air_quality_aligned at event time

Idempotent: skips events that already exist in gold.events.';

-- =============================================================================
-- SECTION 3: Scheduled Job Management
-- =============================================================================

\echo 'Managing detect_events scheduled job...'

-- Remove existing job if present (for idempotent re-runs)
DO $$
DECLARE
    v_job_id INT;
BEGIN
    SELECT job_id INTO v_job_id
    FROM timescaledb_information.jobs
    WHERE proc_name = 'detect_events'
      AND proc_schema = 'gold';

    IF v_job_id IS NOT NULL THEN
        PERFORM delete_job(v_job_id);
        RAISE NOTICE 'Removed existing detect_events job (ID: %)', v_job_id;
    END IF;
END $$;

-- Create new scheduled job (every 15 minutes)
SELECT add_job(
    'gold.detect_events',
    '15 minutes',
    config => '{"enabled": true}'::JSONB,
    initial_start => NOW() + INTERVAL '1 minute'
);

\echo 'Scheduled job created: gold.detect_events (every 15 minutes)'

-- =============================================================================
-- SECTION 4: Monitoring Views for Deferred Deduplication Decision
-- =============================================================================

\echo 'Creating monitoring views for oscillation analysis...'

-- View to monitor threshold crossing frequency per objective per day
CREATE OR REPLACE VIEW gold.v_threshold_crossing_frequency AS
SELECT
    DATE_TRUNC('day', event_time) AS day,
    objective_id,
    metric,
    crossing_direction,
    COUNT(*) AS crossing_count,
    COUNT(DISTINCT entity_id) AS entities_affected,
    AVG(metric_value) AS avg_value_at_crossing,
    MIN(metric_value) AS min_value_at_crossing,
    MAX(metric_value) AS max_value_at_crossing
FROM gold.events
WHERE event_type = 'threshold_crossing'
GROUP BY 1, 2, 3, 4
ORDER BY 1 DESC, 5 DESC;

COMMENT ON VIEW gold.v_threshold_crossing_frequency IS
'Monitor threshold crossing frequency by day/objective.
Use to inform deferred deduplication decision (SPEC-E01).
High frequency may indicate need for hysteresis.';

-- View to detect rapid oscillations (crossings within 1 hour of each other)
CREATE OR REPLACE VIEW gold.v_threshold_crossing_oscillations AS
WITH crossing_gaps AS (
    SELECT
        event_time,
        entity_id,
        objective_id,
        metric,
        crossing_direction,
        metric_value,
        threshold_value,
        event_time - LAG(event_time) OVER (
            PARTITION BY entity_id, objective_id
            ORDER BY event_time
        ) AS gap_since_previous,
        LAG(crossing_direction) OVER (
            PARTITION BY entity_id, objective_id
            ORDER BY event_time
        ) AS prev_direction
    FROM gold.events
    WHERE event_type = 'threshold_crossing'
)
SELECT
    DATE_TRUNC('day', event_time) AS day,
    entity_id,
    objective_id,
    metric,
    threshold_value,
    COUNT(*) FILTER (WHERE gap_since_previous < INTERVAL '1 hour') AS rapid_crossings_1h,
    COUNT(*) FILTER (WHERE gap_since_previous < INTERVAL '2 hours') AS rapid_crossings_2h,
    COUNT(*) AS total_crossings,
    -- Oscillation detection: rapid crossings with alternating directions
    COUNT(*) FILTER (
        WHERE gap_since_previous < INTERVAL '1 hour'
          AND crossing_direction != prev_direction
    ) AS oscillations
FROM crossing_gaps
WHERE gap_since_previous IS NOT NULL
GROUP BY 1, 2, 3, 4, 5
HAVING COUNT(*) FILTER (WHERE gap_since_previous < INTERVAL '2 hours') > 0
ORDER BY 1 DESC, 6 DESC;

COMMENT ON VIEW gold.v_threshold_crossing_oscillations IS
'Detect rapid oscillations (crossings within 1-2 hours) for deferred deduplication decision.
Oscillation = rapid crossing with direction alternating (rising -> falling -> rising).
High oscillation counts suggest need for hysteresis or debouncing.';

-- View to show crossing context correlation (what environmental conditions accompany crossings)
CREATE OR REPLACE VIEW gold.v_crossing_context_analysis AS
SELECT
    objective_id,
    metric,
    crossing_direction,
    COUNT(*) AS total_crossings,
    -- Window state at crossing time
    COUNT(*) FILTER (WHERE context->>'window_state' = 'on') AS crossings_window_open,
    COUNT(*) FILTER (WHERE context->>'window_state' = 'off') AS crossings_window_closed,
    -- Average environmental conditions at crossing
    AVG((context->>'indoor_co2')::FLOAT) AS avg_co2_at_crossing,
    AVG((context->>'indoor_pm25')::FLOAT) AS avg_pm25_at_crossing,
    AVG((context->>'indoor_temp')::FLOAT) AS avg_temp_at_crossing,
    AVG((context->>'indoor_humidity')::FLOAT) AS avg_humidity_at_crossing,
    AVG((context->>'outdoor_temp')::FLOAT) AS avg_outdoor_temp_at_crossing
FROM gold.events
WHERE event_type = 'threshold_crossing'
GROUP BY objective_id, metric, crossing_direction
ORDER BY total_crossings DESC;

COMMENT ON VIEW gold.v_crossing_context_analysis IS
'Analyze environmental context at threshold crossing times.
Helps identify patterns (e.g., CO2 crossings correlate with window state).
Input for V1.2 Pattern Detection Engine.';

-- =============================================================================
-- SECTION 5: Manual Detection Helper (for testing/backfill)
-- =============================================================================

\echo 'Creating manual detection helper function...'

CREATE OR REPLACE FUNCTION gold.detect_events_for_range(
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ,
    p_domain_id TEXT DEFAULT 'indoor-air-quality'
) RETURNS TABLE (
    event_type TEXT,
    events_inserted INT
) AS $$
DECLARE
    v_state_count INT := 0;
    v_threshold_count INT := 0;
BEGIN
    RAISE NOTICE 'Detecting events from % to % for domain %', p_start_time, p_end_time, p_domain_id;

    -- Detect state transitions in range
    WITH ordered_states AS (
        SELECT
            s.event_time,
            s.ndp_id AS entity_id,
            s.source_stream,
            s.state,
            LAG(s.state) OVER (PARTITION BY s.ndp_id ORDER BY s.event_time) AS prev_state,
            LAG(s.event_time) OVER (PARTITION BY s.ndp_id ORDER BY s.event_time) AS prev_time
        FROM silver.state_events s
        WHERE s.event_time >= p_start_time - INTERVAL '1 day'
          AND s.event_time <= p_end_time
    ),
    transitions AS (
        SELECT *
        FROM ordered_states o
        WHERE o.event_time >= p_start_time
          AND o.event_time <= p_end_time
          AND o.prev_state IS NOT NULL
          AND o.prev_state != o.state
    )
    INSERT INTO gold.events (
        event_time, stream_id, entity_id, event_type,
        from_state, to_state, duration_in_state_ms, context, details
    )
    SELECT
        t.event_time,
        COALESCE(t.source_stream, 'home-assistant-state'),
        t.entity_id,
        'state_transition',
        t.prev_state,
        t.state,
        EXTRACT(EPOCH FROM (t.event_time - t.prev_time)) * 1000,
        gold.get_event_context(t.event_time),
        jsonb_build_object('backfill', true, 'range_start', p_start_time, 'range_end', p_end_time)
    FROM transitions t
    WHERE NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = t.event_time
          AND e.entity_id = t.entity_id
          AND e.event_type = 'state_transition'
    );

    GET DIAGNOSTICS v_state_count = ROW_COUNT;

    -- Detect threshold crossings in range (using the function)
    -- Need to iterate through hourly buckets in range
    WITH crossings AS (
        SELECT tc.*
        FROM gold.detect_threshold_crossings(p_start_time, p_domain_id) tc
        WHERE tc.event_time <= p_end_time
    )
    INSERT INTO gold.events (
        event_time, stream_id, entity_id, event_type,
        metric, threshold_value, crossing_direction,
        metric_value, previous_metric_value, objective_id,
        context, details
    )
    SELECT
        c.event_time,
        c.stream_id,
        c.entity_id,
        'threshold_crossing',
        c.metric,
        c.threshold_value,
        c.crossing_direction,
        c.metric_value,
        c.previous_metric_value,
        c.objective_id,
        gold.get_event_context(c.event_time),
        jsonb_build_object(
            'backfill', true,
            'range_start', p_start_time,
            'range_end', p_end_time,
            'condition', c.condition,
            'unit', c.unit
        )
    FROM crossings c
    WHERE NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = c.event_time
          AND e.entity_id = c.entity_id
          AND e.event_type = 'threshold_crossing'
          AND e.objective_id = c.objective_id
    );

    GET DIAGNOSTICS v_threshold_count = ROW_COUNT;

    -- Return results
    event_type := 'state_transition';
    events_inserted := v_state_count;
    RETURN NEXT;

    event_type := 'threshold_crossing';
    events_inserted := v_threshold_count;
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION gold.detect_events_for_range(TIMESTAMPTZ, TIMESTAMPTZ, TEXT) IS
'Manually detect events for a specific time range. Use for testing and backfill.
Returns count of events inserted by type.

Parameters:
  p_start_time: Start of range to detect
  p_end_time: End of range to detect
  p_domain_id: Domain to detect crossings for (default: indoor-air-quality)

Example:
  SELECT * FROM gold.detect_events_for_range(
    NOW() - INTERVAL ''7 days'',
    NOW(),
    ''indoor-air-quality''
  );';

-- =============================================================================
-- SECTION 6: Verification
-- =============================================================================

\echo ''
\echo '=========================================='
\echo 'Verification'
\echo '=========================================='

-- List created/updated objects
SELECT 'Procedure' AS object_type, 'gold.detect_events' AS object_name
UNION ALL
SELECT 'Function', 'gold.detect_threshold_crossings'
UNION ALL
SELECT 'Function', 'gold.get_event_context'
UNION ALL
SELECT 'Function', 'gold.detect_events_for_range'
UNION ALL
SELECT 'View', 'gold.v_threshold_crossing_frequency'
UNION ALL
SELECT 'View', 'gold.v_threshold_crossing_oscillations'
UNION ALL
SELECT 'View', 'gold.v_crossing_context_analysis';

-- Show scheduled job
\echo ''
\echo 'Scheduled Job:'
SELECT
    job_id,
    proc_schema || '.' || proc_name AS procedure,
    schedule_interval,
    next_start
FROM timescaledb_information.jobs
WHERE proc_name = 'detect_events'
  AND proc_schema = 'gold';

-- Show objectives used for threshold crossing
\echo ''
\echo 'Objectives for Threshold Crossing Detection:'
SELECT
    objective_id,
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
\echo 'detect_events Procedure Updated (v11-012)'
\echo '=========================================='
\echo ''
\echo 'The procedure now detects:'
\echo '  1. State transitions (from silver.state_events)'
\echo '  2. Threshold crossings for ALL condition types:'
\echo '     - <  (e.g., CO2 < 800)'
\echo '     - <= (e.g., humidity <= 60)'
\echo '     - >  (e.g., temp > 18)'
\echo '     - >= (e.g., humidity >= 40)'
\echo '     - between (e.g., humidity between 40-60)'
\echo ''
\echo 'Manual execution:'
\echo '  CALL gold.detect_events(NULL, NULL);'
\echo ''
\echo 'Backfill for date range:'
\echo '  SELECT * FROM gold.detect_events_for_range('
\echo '    NOW() - INTERVAL ''7 days'','
\echo '    NOW()'
\echo '  );'
\echo ''
\echo 'Check recent events:'
\echo '  SELECT event_type, COUNT(*) FROM gold.events'
\echo '  WHERE event_time > NOW() - INTERVAL ''1 day'''
\echo '  GROUP BY event_type;'
\echo ''
\echo 'Monitor oscillations (for deferred dedup decision):'
\echo '  SELECT * FROM gold.v_threshold_crossing_oscillations;'
\echo ''
