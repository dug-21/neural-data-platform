-- ============================================================================
-- MIGRATION: 004_detect_events_procedure.sql
-- Feature: v11-013 (Event Detection Procedure) - SPEC-E02
-- Author: NDP TimescaleDB Developer
-- Date: 2026-02-05
--
-- Creates gold.detect_events() procedure for detecting and inserting events.
-- Runs as a TimescaleDB scheduled job every 15 minutes.
--
-- Detects:
--   1. State transitions from silver.state_events
--   2. Threshold crossings (placeholder for v11-012 integration)
--
-- Context captured from gold.indoor_air_quality_aligned at event's hourly bucket.
--
-- Idempotent: CREATE OR REPLACE PROCEDURE
-- ============================================================================

-- ============================================================================
-- HELPER FUNCTION: Get context snapshot
-- Purpose: Retrieve environmental context at a given time
-- ============================================================================

CREATE OR REPLACE FUNCTION gold.get_event_context(
    p_event_time TIMESTAMPTZ
) RETURNS JSONB AS $$
DECLARE
    v_context JSONB;
    v_bucket TIMESTAMPTZ;
BEGIN
    -- Calculate the hourly bucket for the event time
    v_bucket := time_bucket('1 hour', p_event_time);

    -- Attempt to get context from aligned view
    SELECT jsonb_build_object(
        'indoor_co2', a.indoor_co2_mean,
        'indoor_pm25', a.indoor_pm25_mean,
        'indoor_pm25_max', a.indoor_pm25_max,
        'indoor_temp', a.indoor_temperature_c_mean,
        'indoor_humidity', a.indoor_humidity_pct_mean,
        'outdoor_temp', a.outdoor_temperature_c_mean,
        'outdoor_humidity', a.outdoor_humidity_pct_mean,
        'window_state', a.state_last_state,
        'bucket', v_bucket
    ) INTO v_context
    FROM gold.indoor_air_quality_aligned a
    WHERE a.bucket = v_bucket;

    -- Return empty object if no aligned data found
    RETURN COALESCE(v_context, '{}'::JSONB);
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION gold.get_event_context IS
    'Retrieves environmental context from aligned view at the events hourly bucket.
     Returns JSONB with indoor/outdoor metrics and state for correlation analysis.';

-- ============================================================================
-- PROCEDURE: gold.detect_events
-- Purpose: Detect and insert new events into gold.events
-- ============================================================================

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
    -- ========================================================================
    -- STEP 1: Determine last successful run time
    -- ========================================================================

    -- Try to get last_successful_finish from job stats (if running as scheduled job)
    IF job_id IS NOT NULL THEN
        SELECT last_successful_finish INTO v_last_run
        FROM timescaledb_information.job_stats
        WHERE timescaledb_information.job_stats.job_id = detect_events.job_id;
    END IF;

    -- Default to lookback interval if no previous run or manual execution
    v_last_run := COALESCE(v_last_run, v_now - v_lookback);

    RAISE NOTICE 'Event detection starting. Last run: %, Now: %', v_last_run, v_now;

    -- ========================================================================
    -- STEP 2: Detect and insert state transition events
    -- ========================================================================

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
            'detected_at', v_now
        )
    FROM new_transitions t
    -- Avoid duplicates: check if event already exists
    WHERE NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = t.event_time
          AND e.entity_id = t.entity_id
          AND e.event_type = 'state_transition'
    );

    GET DIAGNOSTICS v_state_events_inserted = ROW_COUNT;
    RAISE NOTICE 'Inserted % state transition events', v_state_events_inserted;

    -- ========================================================================
    -- STEP 3: Detect and insert threshold crossing events
    -- ========================================================================

    -- Threshold crossing detection using objectives from data_dictionary
    -- This is a placeholder implementation - v11-012 will provide full logic

    -- For now, detect CO2 threshold crossings as an example
    -- Uses gold.air_quality_hourly aggregates to detect when CO2 crosses thresholds
    WITH hourly_co2 AS (
        SELECT
            bucket,
            ndp_id,
            co2_mean AS current_value,
            LAG(co2_mean) OVER (PARTITION BY ndp_id ORDER BY bucket) AS prev_value
        FROM gold.air_quality_hourly
        WHERE bucket > v_last_run - INTERVAL '1 hour'
    ),
    objective_thresholds AS (
        -- Get CO2 objectives from data_dictionary
        SELECT
            o.objective_id,
            o.target_metric,
            o.condition,
            o.threshold,
            o.threshold_upper
        FROM data_dictionary.objectives o
        WHERE o.target_metric = 'co2'
          AND o.domain_id = 'indoor-air-quality'
    ),
    crossings AS (
        SELECT
            h.bucket AS event_time,
            h.ndp_id AS entity_id,
            'air-quality' AS stream_id,
            'co2' AS metric,
            t.threshold AS threshold_value,
            t.objective_id,
            h.current_value AS metric_value,
            h.prev_value AS previous_value,
            CASE
                WHEN h.prev_value < t.threshold AND h.current_value >= t.threshold THEN 'rising'
                WHEN h.prev_value >= t.threshold AND h.current_value < t.threshold THEN 'falling'
                ELSE NULL
            END AS crossing_direction
        FROM hourly_co2 h
        CROSS JOIN objective_thresholds t
        WHERE h.bucket > v_last_run
          AND h.prev_value IS NOT NULL
          AND (
              -- Rising crossing
              (h.prev_value < t.threshold AND h.current_value >= t.threshold)
              OR
              -- Falling crossing
              (h.prev_value >= t.threshold AND h.current_value < t.threshold)
          )
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
        c.threshold_value,
        c.crossing_direction,
        c.metric_value,
        c.previous_value,
        c.objective_id,
        gold.get_event_context(c.event_time),
        jsonb_build_object(
            'detection_job_id', job_id,
            'detected_at', v_now
        )
    FROM crossings c
    WHERE c.crossing_direction IS NOT NULL
    -- Avoid duplicates
    AND NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = c.event_time
          AND e.entity_id = c.entity_id
          AND e.event_type = 'threshold_crossing'
          AND e.metric = c.metric
          AND e.objective_id = c.objective_id
    );

    GET DIAGNOSTICS v_threshold_events_inserted = ROW_COUNT;
    RAISE NOTICE 'Inserted % threshold crossing events', v_threshold_events_inserted;

    -- ========================================================================
    -- STEP 4: Log summary
    -- ========================================================================

    RAISE NOTICE 'Event detection complete. Total events: %',
        v_state_events_inserted + v_threshold_events_inserted;

    -- Commit the transaction
    COMMIT;
END;
$$;

COMMENT ON PROCEDURE gold.detect_events IS
    'Detects and inserts events into gold.events hypertable.
     Runs as a TimescaleDB scheduled job every 15 minutes.
     Detects:
       1. State transitions from silver.state_events
       2. Threshold crossings using objectives from data_dictionary
     Context captured from gold.indoor_air_quality_aligned at event time.
     Idempotent: skips events that already exist.';

-- ============================================================================
-- HELPER FUNCTION: Manual event detection (for testing/backfill)
-- ============================================================================

CREATE OR REPLACE FUNCTION gold.detect_events_for_range(
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ
) RETURNS TABLE (
    event_type TEXT,
    events_inserted INT
) AS $$
DECLARE
    v_state_count INT := 0;
    v_threshold_count INT := 0;
BEGIN
    -- Run detection for specified time range
    -- Note: This is a simplified version for manual testing

    RAISE NOTICE 'Detecting events from % to %', p_start_time, p_end_time;

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
        '{}'::JSONB
    FROM transitions t
    WHERE NOT EXISTS (
        SELECT 1 FROM gold.events e
        WHERE e.event_time = t.event_time
          AND e.entity_id = t.entity_id
          AND e.event_type = 'state_transition'
    );

    GET DIAGNOSTICS v_state_count = ROW_COUNT;

    -- Return results
    event_type := 'state_transition';
    events_inserted := v_state_count;
    RETURN NEXT;

    event_type := 'threshold_crossing';
    events_inserted := v_threshold_count;
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION gold.detect_events_for_range IS
    'Manually detect events for a specific time range. Use for testing and backfill.
     Returns count of events inserted by type.';

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'gold.detect_events procedure created successfully (v11-013 SPEC-E02)';
    RAISE NOTICE 'Helper function: gold.get_event_context(TIMESTAMPTZ)';
    RAISE NOTICE 'Manual detection: gold.detect_events_for_range(start, end)';
    RAISE NOTICE 'Use 005_schedule_detection_job.sql to schedule the job';
END $$;
