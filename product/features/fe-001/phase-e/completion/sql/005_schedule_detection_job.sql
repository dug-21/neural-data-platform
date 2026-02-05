-- ============================================================================
-- MIGRATION: 005_schedule_detection_job.sql
-- Feature: v11-013 (Event Detection Job Scheduling) - SPEC-E02
-- Author: NDP TimescaleDB Developer
-- Date: 2026-02-05
--
-- Schedules the gold.detect_events() procedure as a TimescaleDB background job.
-- Runs every 15 minutes to detect state transitions and threshold crossings.
--
-- Idempotent: Checks for existing job before creation
-- ============================================================================

-- ============================================================================
-- JOB SCHEDULING: gold.detect_events
-- Purpose: Run event detection every 15 minutes
-- ============================================================================

-- Check if job already exists to avoid duplicates
DO $$
DECLARE
    v_job_id INT;
    v_job_exists BOOLEAN := FALSE;
BEGIN
    -- Check for existing job with same procedure
    SELECT EXISTS (
        SELECT 1
        FROM timescaledb_information.jobs j
        WHERE j.proc_schema = 'gold'
          AND j.proc_name = 'detect_events'
    ) INTO v_job_exists;

    IF v_job_exists THEN
        -- Get existing job ID for logging
        SELECT job_id INTO v_job_id
        FROM timescaledb_information.jobs j
        WHERE j.proc_schema = 'gold'
          AND j.proc_name = 'detect_events'
        LIMIT 1;

        RAISE NOTICE 'Event detection job already exists (job_id: %)', v_job_id;
    ELSE
        -- Create the scheduled job
        SELECT add_job(
            'gold.detect_events',
            '15 minutes',
            config => '{"description": "Detect state transitions and threshold crossings"}'::JSONB,
            initial_start => NOW() + INTERVAL '1 minute',
            scheduled => TRUE
        ) INTO v_job_id;

        RAISE NOTICE 'Event detection job created (job_id: %)', v_job_id;
    END IF;
END $$;

-- ============================================================================
-- JOB CONFIGURATION VIEW
-- Purpose: Easy visibility into job status
-- ============================================================================

CREATE OR REPLACE VIEW gold.v_event_detection_job_status AS
SELECT
    j.job_id,
    j.proc_schema || '.' || j.proc_name AS procedure_name,
    j.schedule_interval,
    j.config,
    j.scheduled,
    js.last_run_started_at,
    js.last_successful_finish,
    js.last_run_status,
    js.total_runs,
    js.total_successes,
    js.total_failures,
    CASE
        WHEN js.last_run_status = 'Success' THEN 'Healthy'
        WHEN js.last_run_status = 'Failed' THEN 'Failed - Check logs'
        WHEN js.last_run_started_at IS NULL THEN 'Never run'
        ELSE 'Unknown'
    END AS health_status,
    CASE
        WHEN js.last_successful_finish IS NOT NULL THEN
            NOW() - js.last_successful_finish
        ELSE NULL
    END AS time_since_last_success
FROM timescaledb_information.jobs j
LEFT JOIN timescaledb_information.job_stats js ON j.job_id = js.job_id
WHERE j.proc_schema = 'gold'
  AND j.proc_name = 'detect_events';

COMMENT ON VIEW gold.v_event_detection_job_status IS
    'Status view for the event detection background job.
     Shows last run, success/failure counts, and health status.';

-- ============================================================================
-- MANUAL JOB CONTROL FUNCTIONS
-- Purpose: Allow manual job control without direct TimescaleDB calls
-- ============================================================================

-- Function to trigger immediate job execution
CREATE OR REPLACE FUNCTION gold.run_event_detection_now()
RETURNS TEXT AS $$
DECLARE
    v_job_id INT;
BEGIN
    -- Get job ID
    SELECT job_id INTO v_job_id
    FROM timescaledb_information.jobs j
    WHERE j.proc_schema = 'gold'
      AND j.proc_name = 'detect_events'
    LIMIT 1;

    IF v_job_id IS NULL THEN
        RETURN 'ERROR: Event detection job not found';
    END IF;

    -- Run the job immediately
    CALL run_job(v_job_id);

    RETURN 'Event detection job triggered (job_id: ' || v_job_id || ')';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION gold.run_event_detection_now IS
    'Trigger immediate execution of the event detection job.
     Use for testing or when immediate event detection is needed.';

-- Function to pause event detection
CREATE OR REPLACE FUNCTION gold.pause_event_detection()
RETURNS TEXT AS $$
DECLARE
    v_job_id INT;
BEGIN
    SELECT job_id INTO v_job_id
    FROM timescaledb_information.jobs j
    WHERE j.proc_schema = 'gold'
      AND j.proc_name = 'detect_events'
    LIMIT 1;

    IF v_job_id IS NULL THEN
        RETURN 'ERROR: Event detection job not found';
    END IF;

    -- Disable scheduling
    SELECT alter_job(v_job_id, scheduled => FALSE);

    RETURN 'Event detection job paused (job_id: ' || v_job_id || ')';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION gold.pause_event_detection IS
    'Pause the event detection scheduled job.
     Job can still be run manually via gold.run_event_detection_now().';

-- Function to resume event detection
CREATE OR REPLACE FUNCTION gold.resume_event_detection()
RETURNS TEXT AS $$
DECLARE
    v_job_id INT;
BEGIN
    SELECT job_id INTO v_job_id
    FROM timescaledb_information.jobs j
    WHERE j.proc_schema = 'gold'
      AND j.proc_name = 'detect_events'
    LIMIT 1;

    IF v_job_id IS NULL THEN
        RETURN 'ERROR: Event detection job not found';
    END IF;

    -- Enable scheduling
    SELECT alter_job(v_job_id, scheduled => TRUE);

    RETURN 'Event detection job resumed (job_id: ' || v_job_id || ')';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION gold.resume_event_detection IS
    'Resume the event detection scheduled job after it was paused.';

-- Function to change detection interval
CREATE OR REPLACE FUNCTION gold.set_event_detection_interval(
    p_interval INTERVAL
)
RETURNS TEXT AS $$
DECLARE
    v_job_id INT;
BEGIN
    SELECT job_id INTO v_job_id
    FROM timescaledb_information.jobs j
    WHERE j.proc_schema = 'gold'
      AND j.proc_name = 'detect_events'
    LIMIT 1;

    IF v_job_id IS NULL THEN
        RETURN 'ERROR: Event detection job not found';
    END IF;

    -- Change interval
    SELECT alter_job(v_job_id, schedule_interval => p_interval);

    RETURN 'Event detection interval changed to ' || p_interval || ' (job_id: ' || v_job_id || ')';
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION gold.set_event_detection_interval IS
    'Change the event detection job schedule interval.
     Example: SELECT gold.set_event_detection_interval(INTERVAL ''5 minutes'')';

-- ============================================================================
-- MONITORING: Event detection metrics
-- ============================================================================

-- View for event detection metrics
CREATE OR REPLACE VIEW gold.v_event_detection_metrics AS
SELECT
    -- Recent event counts
    COUNT(*) FILTER (WHERE event_time > NOW() - INTERVAL '1 hour')
        AS events_last_hour,
    COUNT(*) FILTER (WHERE event_time > NOW() - INTERVAL '24 hours')
        AS events_last_24_hours,
    COUNT(*) FILTER (WHERE event_time > NOW() - INTERVAL '7 days')
        AS events_last_7_days,

    -- By type (last 24 hours)
    COUNT(*) FILTER (WHERE event_type = 'state_transition' AND event_time > NOW() - INTERVAL '24 hours')
        AS state_transitions_24h,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing' AND event_time > NOW() - INTERVAL '24 hours')
        AS threshold_crossings_24h,

    -- Most recent event
    MAX(event_time) AS most_recent_event,

    -- Event gap (time since last event)
    NOW() - MAX(event_time) AS time_since_last_event

FROM gold.events;

COMMENT ON VIEW gold.v_event_detection_metrics IS
    'Metrics for event detection health monitoring.
     Shows event counts, most recent event, and time since last event.';

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
DECLARE
    v_job_id INT;
BEGIN
    -- Get job ID for final message
    SELECT job_id INTO v_job_id
    FROM timescaledb_information.jobs j
    WHERE j.proc_schema = 'gold'
      AND j.proc_name = 'detect_events'
    LIMIT 1;

    RAISE NOTICE '=============================================================';
    RAISE NOTICE 'Event detection job scheduled successfully (v11-013 SPEC-E02)';
    RAISE NOTICE '=============================================================';
    RAISE NOTICE 'Job ID: %', COALESCE(v_job_id::TEXT, 'Not found');
    RAISE NOTICE 'Schedule: Every 15 minutes';
    RAISE NOTICE '';
    RAISE NOTICE 'Management functions:';
    RAISE NOTICE '  SELECT gold.run_event_detection_now();     -- Run immediately';
    RAISE NOTICE '  SELECT gold.pause_event_detection();       -- Pause job';
    RAISE NOTICE '  SELECT gold.resume_event_detection();      -- Resume job';
    RAISE NOTICE '  SELECT gold.set_event_detection_interval(INTERVAL ''5 min'');';
    RAISE NOTICE '';
    RAISE NOTICE 'Monitoring:';
    RAISE NOTICE '  SELECT * FROM gold.v_event_detection_job_status;';
    RAISE NOTICE '  SELECT * FROM gold.v_event_detection_metrics;';
    RAISE NOTICE '=============================================================';
END $$;
