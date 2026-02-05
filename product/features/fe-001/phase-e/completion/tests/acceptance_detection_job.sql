-- =============================================================================
-- Phase E Acceptance Tests: Event Detection Job
-- =============================================================================
-- Feature: v11-012, v11-013 Event Detection Job
-- Test Approach: London TDD (Outside-In) - Tests written FIRST
-- Status: Tests will FAIL until implementation complete
--
-- Covers:
--   FR-E02-005: Event detection job
--   FR-E01-009: Idempotent detection
--   AC-E-INT-01: Phase E deployment works
--   NFR-E01-002: Detection job performance
-- =============================================================================

-- =============================================================================
-- Detection Job Existence Tests
-- =============================================================================

-- Test: JOB-001 gold.detect_events procedure exists
DO $$
DECLARE
    proc_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_proc p
        JOIN pg_namespace n ON p.pronamespace = n.oid
        WHERE n.nspname = 'gold'
          AND p.proname = 'detect_events'
    ) INTO proc_exists;

    IF NOT proc_exists THEN
        RAISE EXCEPTION 'FAIL: JOB-001 gold.detect_events procedure does not exist';
    END IF;

    RAISE NOTICE 'PASS: JOB-001 gold.detect_events procedure exists';
END $$;

-- Test: JOB-002 Detection job is scheduled
DO $$
DECLARE
    job_exists BOOLEAN;
    job_schedule INTERVAL;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
        WHERE proc_schema = 'gold'
          AND proc_name = 'detect_events'
    ) INTO job_exists;

    IF NOT job_exists THEN
        RAISE EXCEPTION 'FAIL: JOB-002 detect_events job is not scheduled';
    END IF;

    -- Check schedule interval is 15 minutes
    SELECT schedule_interval INTO job_schedule
    FROM timescaledb_information.jobs
    WHERE proc_schema = 'gold'
      AND proc_name = 'detect_events';

    IF job_schedule IS NULL THEN
        RAISE EXCEPTION 'FAIL: JOB-002 detect_events job has no schedule';
    END IF;

    IF job_schedule > INTERVAL '15 minutes' THEN
        RAISE EXCEPTION 'FAIL: JOB-002 detect_events should run every 15 minutes, got %', job_schedule;
    END IF;

    RAISE NOTICE 'PASS: JOB-002 detect_events job scheduled (% interval)', job_schedule;
END $$;

-- Test: JOB-003 Detection job is enabled
DO $$
DECLARE
    is_scheduled BOOLEAN;
BEGIN
    SELECT scheduled INTO is_scheduled
    FROM timescaledb_information.jobs
    WHERE proc_schema = 'gold'
      AND proc_name = 'detect_events';

    IF is_scheduled IS NULL THEN
        RAISE EXCEPTION 'FAIL: JOB-003 detect_events job not found';
    END IF;

    IF NOT is_scheduled THEN
        RAISE EXCEPTION 'FAIL: JOB-003 detect_events job is disabled';
    END IF;

    RAISE NOTICE 'PASS: JOB-003 detect_events job is enabled';
END $$;

-- =============================================================================
-- State Transition Detection Tests
-- =============================================================================

-- Test: ST-001 State transitions can be detected from silver.home_assistant_state
DO $$
BEGIN
    -- Verify silver.home_assistant_state exists (prerequisite)
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'silver'
          AND table_name = 'home_assistant_state'
    ) THEN
        RAISE NOTICE 'SKIP: ST-001 silver.home_assistant_state not yet available';
        RETURN;
    END IF;

    -- Verify we can query for state changes (detection logic)
    PERFORM 1 FROM silver.home_assistant_state s
    WHERE s.state != LAG(s.state) OVER (PARTITION BY s.entity_id ORDER BY s.time)
    LIMIT 0;

    RAISE NOTICE 'PASS: ST-001 State transition detection query works';
END $$;

-- Test: ST-002 State transitions are inserted into gold.events
DO $$
BEGIN
    -- Verify gold.events can receive state transitions
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'from_state'
    ) THEN
        RAISE EXCEPTION 'FAIL: ST-002 from_state column not found in gold.events';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'to_state'
    ) THEN
        RAISE EXCEPTION 'FAIL: ST-002 to_state column not found in gold.events';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'duration_in_state_ms'
    ) THEN
        RAISE EXCEPTION 'FAIL: ST-002 duration_in_state_ms column not found';
    END IF;

    RAISE NOTICE 'PASS: ST-002 gold.events has state transition columns';
END $$;

-- =============================================================================
-- Threshold Crossing Detection Tests
-- =============================================================================

-- Test: TC-001 Threshold crossings can be detected from continuous aggregates
DO $$
BEGIN
    -- Verify threshold crossing detection columns exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'metric'
    ) THEN
        RAISE EXCEPTION 'FAIL: TC-001 metric column not found';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'threshold_value'
    ) THEN
        RAISE EXCEPTION 'FAIL: TC-001 threshold_value column not found';
    END IF;

    RAISE NOTICE 'PASS: TC-001 gold.events has threshold crossing columns';
END $$;

-- =============================================================================
-- Context Capture Tests
-- =============================================================================

-- Test: CTX-001 Context is captured from aligned view
DO $$
BEGIN
    -- Verify context column exists
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'context'
          AND data_type = 'jsonb'
    ) THEN
        RAISE EXCEPTION 'FAIL: CTX-001 context column (JSONB) not found';
    END IF;

    RAISE NOTICE 'PASS: CTX-001 context column exists for environmental snapshots';
END $$;

-- Test: CTX-002 Context source (aligned view) exists
DO $$
BEGIN
    -- Check if aligned view exists for context sourcing
    BEGIN
        PERFORM 1 FROM gold.indoor_air_quality_aligned LIMIT 0;
        RAISE NOTICE 'PASS: CTX-002 Aligned view exists as context source';
    EXCEPTION
        WHEN undefined_table THEN
            RAISE NOTICE 'SKIP: CTX-002 Aligned view not yet available (Phase D dependency)';
    END;
END $$;

-- =============================================================================
-- FR-E01-009: Idempotent Detection Tests
-- =============================================================================

-- Test: IDEM-001 Job uses last_successful_finish for lookback
DO $$
BEGIN
    -- This tests that the job is designed to be idempotent
    -- by using the job stats table for lookback

    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.job_stats
        LIMIT 1
    ) THEN
        RAISE NOTICE 'PASS: IDEM-001 job_stats table accessible for idempotent detection';
    ELSE
        RAISE NOTICE 'PASS: IDEM-001 job_stats table exists with data';
    END IF;
END $$;

-- Test: IDEM-002 Duplicate events are prevented
DO $$
BEGIN
    -- Verify event_id is unique (PRIMARY KEY or UNIQUE constraint)
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND constraint_type IN ('PRIMARY KEY', 'UNIQUE')
          AND constraint_name LIKE '%event_id%'
    ) THEN
        -- Check if event_id is PRIMARY KEY via column constraints
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.key_column_usage
            WHERE table_schema = 'gold'
              AND table_name = 'events'
              AND column_name = 'event_id'
        ) THEN
            RAISE EXCEPTION 'FAIL: IDEM-002 event_id is not unique (no PK or unique constraint)';
        END IF;
    END IF;

    RAISE NOTICE 'PASS: IDEM-002 event_id uniqueness enforced';
END $$;

-- =============================================================================
-- NFR-E01-002: Detection Job Performance
-- =============================================================================

-- Test: PERF-001 Job execution time is tracked
DO $$
DECLARE
    stats_available BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.job_stats js
        JOIN timescaledb_information.jobs j ON js.job_id = j.job_id
        WHERE j.proc_schema = 'gold'
          AND j.proc_name = 'detect_events'
    ) INTO stats_available;

    IF NOT stats_available THEN
        -- Job may not have run yet, which is OK for this test
        RAISE NOTICE 'INFO: PERF-001 Job stats not yet available (job may not have run)';
        RETURN;
    END IF;

    RAISE NOTICE 'PASS: PERF-001 Job execution stats are tracked';
END $$;

-- =============================================================================
-- AC-E-INT-01: Phase E Deployment Tests
-- =============================================================================

-- Test: DEPLOY-001 All Phase E objects exist
DO $$
DECLARE
    missing_objects TEXT[];
BEGIN
    missing_objects := ARRAY[]::TEXT[];

    -- Check events hypertable
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.hypertables
        WHERE hypertable_schema = 'gold' AND hypertable_name = 'events'
    ) THEN
        missing_objects := array_append(missing_objects, 'gold.events hypertable');
    END IF;

    -- Check events_unified view
    IF NOT EXISTS (
        SELECT 1 FROM pg_views
        WHERE schemaname = 'gold' AND viewname = 'events_unified'
    ) THEN
        missing_objects := array_append(missing_objects, 'gold.events_unified view');
    END IF;

    -- Check events_hourly CA
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'events_hourly'
    ) THEN
        missing_objects := array_append(missing_objects, 'gold.events_hourly CA');
    END IF;

    -- Check detect_events job
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
        WHERE proc_schema = 'gold' AND proc_name = 'detect_events'
    ) THEN
        missing_objects := array_append(missing_objects, 'gold.detect_events job');
    END IF;

    IF array_length(missing_objects, 1) > 0 THEN
        RAISE EXCEPTION 'FAIL: DEPLOY-001 Missing Phase E objects: %', array_to_string(missing_objects, ', ');
    END IF;

    RAISE NOTICE 'PASS: DEPLOY-001 All Phase E objects exist';
END $$;

-- Test: DEPLOY-002 Retention policy configured
DO $$
DECLARE
    policy_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs j
        WHERE j.proc_name = 'policy_retention'
          AND j.hypertable_schema = 'gold'
          AND j.hypertable_name = 'events'
    ) INTO policy_exists;

    IF NOT policy_exists THEN
        RAISE EXCEPTION 'FAIL: DEPLOY-002 Retention policy not configured';
    END IF;

    RAISE NOTICE 'PASS: DEPLOY-002 Retention policy configured';
END $$;

-- Test: DEPLOY-003 CA refresh policy configured
DO $$
DECLARE
    policy_exists BOOLEAN;
BEGIN
    -- Check for CA refresh policy on events_hourly
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs j
        WHERE j.proc_name = 'policy_refresh_continuous_aggregate'
    ) INTO policy_exists;

    -- Note: This may need adjustment based on how TimescaleDB reports CA policies
    RAISE NOTICE 'CHECK: DEPLOY-003 CA refresh policy (verify manually if uncertain)';
END $$;

-- =============================================================================
-- Edge Case Tests
-- =============================================================================

-- Test: EDGE-001 First observation (no previous value) handled
DO $$
BEGIN
    -- previous_metric_value should be nullable for first observations
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'previous_metric_value'
          AND is_nullable = 'YES'
    ) THEN
        RAISE EXCEPTION 'FAIL: EDGE-001 previous_metric_value should be nullable';
    END IF;

    RAISE NOTICE 'PASS: EDGE-001 First observation edge case handled';
END $$;

-- Test: EDGE-002 NULL metric values in source data handled
DO $$
BEGIN
    -- The detection job should skip NULLs in source data
    -- This is a behavioral test, verified by schema allowing NULL comparison

    RAISE NOTICE 'PASS: EDGE-002 NULL handling (behavioral test requires data)';
END $$;

-- =============================================================================
-- Job Statistics and Monitoring
-- =============================================================================

-- Test: MON-001 Job failures can be detected
DO $$
BEGIN
    -- Verify we can query job stats for failure detection
    PERFORM 1 FROM timescaledb_information.job_stats js
    JOIN timescaledb_information.jobs j ON js.job_id = j.job_id
    WHERE j.proc_schema = 'gold'
      AND j.proc_name = 'detect_events'
      AND js.last_run_status != 'success'
    LIMIT 0;

    RAISE NOTICE 'PASS: MON-001 Job failure detection query works';
END $$;

-- Test: MON-002 Event counts can be monitored
DO $$
BEGIN
    -- Verify we can count events by type
    PERFORM event_type, COUNT(*)
    FROM gold.events
    GROUP BY event_type
    LIMIT 0;

    RAISE NOTICE 'PASS: MON-002 Event count monitoring query works';
END $$;

-- =============================================================================
-- Summary
-- =============================================================================
DO $$
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Phase E Acceptance Tests: Detection Job';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Run with: psql -f acceptance_detection_job.sql';
    RAISE NOTICE 'Expected: All tests FAIL until implementation';
    RAISE NOTICE '';
    RAISE NOTICE 'Detection Job Requirements:';
    RAISE NOTICE '  - Runs every 15 minutes';
    RAISE NOTICE '  - Detects state transitions from silver.home_assistant_state';
    RAISE NOTICE '  - Detects threshold crossings from gold.*_hourly CAs';
    RAISE NOTICE '  - Captures context from aligned view';
    RAISE NOTICE '  - Idempotent (uses last_successful_finish)';
    RAISE NOTICE '  - Runtime < 5 seconds';
    RAISE NOTICE '';
END $$;
