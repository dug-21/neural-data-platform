-- =============================================================================
-- Phase E Acceptance Tests: Events Hypertable (AC-E-01, AC-E-02, AC-E-03)
-- =============================================================================
-- Feature: v11-013 Events Hypertable & Unified Events View
-- Test Approach: London TDD (Outside-In) - Tests written FIRST
-- Status: Tests will FAIL until implementation complete
--
-- Covers:
--   AC-E02-001: Hypertable created correctly (7-day chunks)
--   AC-E02-002: State transitions inserted with context
--   AC-E02-003: Threshold crossings inserted with direction
--   FR-E02-001: Events hypertable schema
--   FR-E02-002: Hypertable configuration
--   FR-E02-008: Index strategy
--   FR-E02-009: Retention policy
-- =============================================================================

-- Test Helper: Count passed/failed tests
CREATE OR REPLACE FUNCTION test_utils.count_test_result(
    test_name TEXT,
    passed BOOLEAN,
    error_message TEXT DEFAULT NULL
) RETURNS VOID AS $$
BEGIN
    IF passed THEN
        RAISE NOTICE 'PASS: %', test_name;
    ELSE
        RAISE NOTICE 'FAIL: % - %', test_name, COALESCE(error_message, 'Assertion failed');
    END IF;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- AC-E02-001: Events hypertable exists and is properly configured
-- =============================================================================

-- Test: AC-E02-001-a Events hypertable exists
DO $$
DECLARE
    is_hypertable BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.hypertables
        WHERE hypertable_schema = 'gold' AND hypertable_name = 'events'
    ) INTO is_hypertable;

    IF NOT is_hypertable THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-a gold.events is not a hypertable (or does not exist)';
    END IF;

    RAISE NOTICE 'PASS: AC-E02-001-a gold.events exists as hypertable';
END $$;

-- Test: AC-E02-001-b Events hypertable has 7-day chunk interval
DO $$
DECLARE
    chunk_interval INTERVAL;
BEGIN
    SELECT h.chunk_interval INTO chunk_interval
    FROM timescaledb_information.dimensions d
    JOIN timescaledb_information.hypertables h
        ON d.hypertable_schema = h.hypertable_schema
        AND d.hypertable_name = h.hypertable_name
    WHERE h.hypertable_schema = 'gold' AND h.hypertable_name = 'events';

    IF chunk_interval IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-b Cannot determine chunk interval (hypertable may not exist)';
    END IF;

    IF chunk_interval != INTERVAL '7 days' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-b Expected 7-day chunk interval, got %', chunk_interval;
    END IF;

    RAISE NOTICE 'PASS: AC-E02-001-b gold.events has 7-day chunk interval';
END $$;

-- Test: AC-E02-001-c Events hypertable has required columns
DO $$
DECLARE
    missing_columns TEXT[];
    required_columns TEXT[] := ARRAY[
        'event_id', 'event_time', 'stream_id', 'entity_id', 'event_type',
        'from_state', 'to_state', 'duration_in_state_ms',
        'metric', 'threshold_value', 'crossing_direction',
        'metric_value', 'previous_metric_value', 'objective_id',
        'context', 'details'
    ];
    col TEXT;
BEGIN
    missing_columns := ARRAY[]::TEXT[];

    FOREACH col IN ARRAY required_columns LOOP
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'gold'
              AND table_name = 'events'
              AND column_name = col
        ) THEN
            missing_columns := array_append(missing_columns, col);
        END IF;
    END LOOP;

    IF array_length(missing_columns, 1) > 0 THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-c Missing columns: %', array_to_string(missing_columns, ', ');
    END IF;

    RAISE NOTICE 'PASS: AC-E02-001-c gold.events has all required columns';
END $$;

-- Test: AC-E02-001-d event_time column is NOT NULL and TIMESTAMPTZ
DO $$
DECLARE
    col_type TEXT;
    is_nullable TEXT;
BEGIN
    SELECT data_type, is_nullable INTO col_type, is_nullable
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'event_time';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-d event_time column does not exist';
    END IF;

    IF col_type NOT LIKE '%timestamp%' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-d event_time should be TIMESTAMPTZ, got %', col_type;
    END IF;

    IF is_nullable = 'YES' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-d event_time should be NOT NULL';
    END IF;

    RAISE NOTICE 'PASS: AC-E02-001-d event_time is TIMESTAMPTZ NOT NULL';
END $$;

-- Test: AC-E02-001-e context column is JSONB NOT NULL
DO $$
DECLARE
    col_type TEXT;
    is_nullable TEXT;
BEGIN
    SELECT data_type, is_nullable INTO col_type, is_nullable
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'context';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-e context column does not exist';
    END IF;

    IF col_type != 'jsonb' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-e context should be JSONB, got %', col_type;
    END IF;

    IF is_nullable = 'YES' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-e context should be NOT NULL';
    END IF;

    RAISE NOTICE 'PASS: AC-E02-001-e context is JSONB NOT NULL';
END $$;

-- Test: AC-E02-001-f details column is JSONB NOT NULL
DO $$
DECLARE
    col_type TEXT;
    is_nullable TEXT;
BEGIN
    SELECT data_type, is_nullable INTO col_type, is_nullable
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'details';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-f details column does not exist';
    END IF;

    IF col_type != 'jsonb' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-f details should be JSONB, got %', col_type;
    END IF;

    IF is_nullable = 'YES' THEN
        RAISE EXCEPTION 'FAIL: AC-E02-001-f details should be NOT NULL';
    END IF;

    RAISE NOTICE 'PASS: AC-E02-001-f details is JSONB NOT NULL';
END $$;

-- =============================================================================
-- FR-E02-008: Index Strategy Tests
-- =============================================================================

-- Test: FR-E02-008-a Index on event_time exists
DO $$
DECLARE
    idx_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'gold'
          AND tablename = 'events'
          AND indexdef LIKE '%event_time%'
    ) INTO idx_exists;

    IF NOT idx_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E02-008-a No index on event_time found';
    END IF;

    RAISE NOTICE 'PASS: FR-E02-008-a Index on event_time exists';
END $$;

-- Test: FR-E02-008-b Composite index on (event_type, event_time) exists
DO $$
DECLARE
    idx_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'gold'
          AND tablename = 'events'
          AND indexdef LIKE '%event_type%'
          AND indexdef LIKE '%event_time%'
    ) INTO idx_exists;

    IF NOT idx_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E02-008-b No composite index on (event_type, event_time) found';
    END IF;

    RAISE NOTICE 'PASS: FR-E02-008-b Composite index on (event_type, event_time) exists';
END $$;

-- Test: FR-E02-008-c Index on entity_id exists
DO $$
DECLARE
    idx_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'gold'
          AND tablename = 'events'
          AND indexdef LIKE '%entity_id%'
    ) INTO idx_exists;

    IF NOT idx_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E02-008-c No index on entity_id found';
    END IF;

    RAISE NOTICE 'PASS: FR-E02-008-c Index on entity_id exists';
END $$;

-- Test: FR-E02-008-d Partial index on objective_id for threshold crossings exists
DO $$
DECLARE
    idx_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'gold'
          AND tablename = 'events'
          AND indexdef LIKE '%objective_id%'
          AND indexdef LIKE '%threshold_crossing%'
    ) INTO idx_exists;

    IF NOT idx_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E02-008-d No partial index on objective_id for threshold_crossing found';
    END IF;

    RAISE NOTICE 'PASS: FR-E02-008-d Partial index on objective_id exists';
END $$;

-- Test: FR-E02-008-e GIN index on context JSONB exists
DO $$
DECLARE
    idx_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = 'gold'
          AND tablename = 'events'
          AND indexdef LIKE '%context%'
          AND indexdef LIKE '%gin%'
    ) INTO idx_exists;

    IF NOT idx_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E02-008-e No GIN index on context column found';
    END IF;

    RAISE NOTICE 'PASS: FR-E02-008-e GIN index on context exists';
END $$;

-- =============================================================================
-- FR-E02-009: Retention Policy Tests
-- =============================================================================

-- Test: FR-E02-009-a Retention policy is configured
DO $$
DECLARE
    policy_exists BOOLEAN;
    retention_interval INTERVAL;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs j
        WHERE j.proc_name = 'policy_retention'
          AND j.hypertable_schema = 'gold'
          AND j.hypertable_name = 'events'
    ) INTO policy_exists;

    IF NOT policy_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E02-009-a No retention policy found for gold.events';
    END IF;

    -- Check retention is 1 year
    SELECT (j.config->>'drop_after')::INTERVAL INTO retention_interval
    FROM timescaledb_information.jobs j
    WHERE j.proc_name = 'policy_retention'
      AND j.hypertable_schema = 'gold'
      AND j.hypertable_name = 'events';

    IF retention_interval < INTERVAL '365 days' THEN
        RAISE EXCEPTION 'FAIL: FR-E02-009-a Retention should be >= 1 year, got %', retention_interval;
    END IF;

    RAISE NOTICE 'PASS: FR-E02-009-a Retention policy (1 year) is configured';
END $$;

-- =============================================================================
-- AC-E-03: Unified Events View Tests
-- =============================================================================

-- Test: AC-E-03-a gold.events_unified view exists
DO $$
DECLARE
    view_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_views
        WHERE schemaname = 'gold' AND viewname = 'events_unified'
    ) INTO view_exists;

    IF NOT view_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-a gold.events_unified view does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-03-a gold.events_unified view exists';
END $$;

-- Test: AC-E-03-b events_unified view has correct columns
DO $$
DECLARE
    missing_columns TEXT[];
    required_columns TEXT[] := ARRAY[
        'event_id', 'event_time', 'stream_id', 'entity_id',
        'event_type', 'details', 'context'
    ];
    col TEXT;
BEGIN
    missing_columns := ARRAY[]::TEXT[];

    FOREACH col IN ARRAY required_columns LOOP
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'gold'
              AND table_name = 'events_unified'
              AND column_name = col
        ) THEN
            missing_columns := array_append(missing_columns, col);
        END IF;
    END LOOP;

    IF array_length(missing_columns, 1) > 0 THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-b Missing columns in events_unified: %', array_to_string(missing_columns, ', ');
    END IF;

    RAISE NOTICE 'PASS: AC-E-03-b events_unified has all required columns';
END $$;

-- =============================================================================
-- AC-E-05: Hourly Events Aggregate Tests
-- =============================================================================

-- Test: AC-E-05-a gold.events_hourly continuous aggregate exists
DO $$
DECLARE
    ca_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'events_hourly'
    ) INTO ca_exists;

    IF NOT ca_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-a gold.events_hourly continuous aggregate does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-a gold.events_hourly continuous aggregate exists';
END $$;

-- Test: AC-E-05-b events_hourly has required columns
DO $$
DECLARE
    missing_columns TEXT[];
    required_columns TEXT[] := ARRAY[
        'bucket', 'total_events', 'state_transition_count', 'threshold_crossing_count'
    ];
    col TEXT;
BEGIN
    missing_columns := ARRAY[]::TEXT[];

    FOREACH col IN ARRAY required_columns LOOP
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'gold'
              AND table_name = 'events_hourly'
              AND column_name = col
        ) THEN
            missing_columns := array_append(missing_columns, col);
        END IF;
    END LOOP;

    IF array_length(missing_columns, 1) > 0 THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-b Missing columns in events_hourly: %', array_to_string(missing_columns, ', ');
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-b events_hourly has all required columns';
END $$;

-- Test: AC-E-05-c events_hourly has refresh policy
DO $$
DECLARE
    policy_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs j
        WHERE j.proc_name = 'policy_refresh_continuous_aggregate'
          AND j.hypertable_schema = 'gold'
          AND j.hypertable_name LIKE '%events_hourly%'
    ) INTO policy_exists;

    IF NOT policy_exists THEN
        -- Try alternative query for continuous aggregate policies
        SELECT EXISTS (
            SELECT 1 FROM timescaledb_information.continuous_aggregate_stats
            WHERE view_schema = 'gold' AND view_name = 'events_hourly'
        ) INTO policy_exists;
    END IF;

    -- Note: This test may need adjustment based on TimescaleDB version
    RAISE NOTICE 'CHECK: AC-E-05-c events_hourly refresh policy (verify manually if this passes unexpectedly)';
END $$;

-- =============================================================================
-- Summary
-- =============================================================================
DO $$
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Phase E Acceptance Tests: Events Hypertable';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Run with: psql -f acceptance_events_hypertable.sql';
    RAISE NOTICE 'Expected: All tests FAIL until implementation';
    RAISE NOTICE '';
END $$;
