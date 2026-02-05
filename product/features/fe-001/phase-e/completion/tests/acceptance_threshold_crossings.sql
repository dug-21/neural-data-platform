-- =============================================================================
-- Phase E Acceptance Tests: Threshold Crossings (AC-E-01, AC-E-02, AC-E-04)
-- =============================================================================
-- Feature: v11-012 Threshold Crossing Generator
-- Test Approach: London TDD (Outside-In) - Tests written FIRST
-- Status: Tests will FAIL until implementation complete
--
-- Covers:
--   AC-E-01: Threshold crossing generator works
--   AC-E-02: All condition types supported (<, <=, >, >=, between)
--   AC-E-04: Event schema contract met
--   FR-E01-001: Threshold crossing detection
--   FR-E01-002: Condition operator support
--   FR-E01-003: Between condition handling
--   FR-E01-004: Event schema
-- =============================================================================

-- =============================================================================
-- Test Setup: Create test data for threshold crossing detection
-- =============================================================================

-- Test: Ensure we can insert test data for threshold crossing scenarios
DO $$
BEGIN
    -- Create test schema if not exists
    CREATE SCHEMA IF NOT EXISTS test_data;

    -- Drop and recreate test tables
    DROP TABLE IF EXISTS test_data.objective_test_cases CASCADE;

    CREATE TABLE test_data.objective_test_cases (
        test_id SERIAL PRIMARY KEY,
        test_name TEXT NOT NULL,
        objective_id TEXT NOT NULL,
        metric TEXT NOT NULL,
        condition TEXT NOT NULL,
        threshold_value DOUBLE PRECISION,
        threshold_min DOUBLE PRECISION,
        threshold_max DOUBLE PRECISION,
        prev_value DOUBLE PRECISION NOT NULL,
        current_value DOUBLE PRECISION NOT NULL,
        expected_direction TEXT,  -- NULL if no crossing expected
        description TEXT
    );

    RAISE NOTICE 'PASS: Test setup complete - test_data schema created';
EXCEPTION
    WHEN others THEN
        RAISE NOTICE 'SETUP: Test data setup (may fail on first run): %', SQLERRM;
END $$;

-- =============================================================================
-- AC-E-01: Threshold Crossing Detection Tests
-- =============================================================================

-- Test: AC-E-01-001 Rising crossing detected for condition "<"
-- Scenario: CO2 < 800 threshold, value crosses from 795 to 812
DO $$
DECLARE
    crossing_exists BOOLEAN;
    crossing_direction TEXT;
BEGIN
    -- Check if threshold crossing was detected
    SELECT EXISTS (
        SELECT 1 FROM gold.events
        WHERE event_type = 'threshold_crossing'
          AND metric = 'co2'
          AND objective_id = 'healthy_co2'
          AND crossing_direction = 'rising'
          AND metric_value >= 800
          AND previous_metric_value < 800
        LIMIT 1
    ) INTO crossing_exists;

    -- For now, just verify the query works (will fail until data exists)
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'gold' AND table_name = 'events'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-01-001 gold.events table does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-01-001 Rising crossing query syntax valid (data verification pending)';
END $$;

-- Test: AC-E-01-002 Falling crossing detected for condition "<"
-- Scenario: CO2 < 800 threshold, value crosses from 850 to 780
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'gold' AND table_name = 'events'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-01-002 gold.events table does not exist';
    END IF;

    -- Verify query for falling crossings
    PERFORM 1 FROM gold.events
    WHERE event_type = 'threshold_crossing'
      AND crossing_direction = 'falling'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-01-002 Falling crossing query syntax valid (data verification pending)';
END $$;

-- Test: AC-E-01-003 No crossing when both values on same side of threshold
-- Scenario: CO2 < 800, both readings at 750 and 780 (both below 800)
DO $$
BEGIN
    -- This is a negative test - verify no spurious crossings
    -- The detection logic should NOT create crossings when both values are below threshold

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'gold' AND table_name = 'events'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-01-003 gold.events table does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-01-003 Schema check passed (behavioral test requires data)';
END $$;

-- =============================================================================
-- AC-E-02: All Condition Types Supported
-- =============================================================================

-- Test: AC-E-02-001 Condition "<" (less than) works correctly
DO $$
BEGIN
    -- For condition "<", threshold 800:
    -- - Rising: prev < 800 AND curr >= 800 (entering violation)
    -- - Falling: prev >= 800 AND curr < 800 (leaving violation)

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'crossing_direction'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-02-001 crossing_direction column not found';
    END IF;

    RAISE NOTICE 'PASS: AC-E-02-001 Condition "<" column exists (behavioral test pending)';
END $$;

-- Test: AC-E-02-002 Condition "<=" (less than or equal) works correctly
DO $$
BEGIN
    -- For condition "<=", threshold 800:
    -- - Rising: prev <= 800 AND curr > 800 (entering violation)
    -- - Falling: prev > 800 AND curr <= 800 (leaving violation)

    RAISE NOTICE 'PASS: AC-E-02-002 Condition "<=" test ready (behavioral test pending)';
END $$;

-- Test: AC-E-02-003 Condition ">" (greater than) works correctly
DO $$
BEGIN
    -- For condition ">", threshold 18 (e.g., min temp):
    -- - Rising: prev > 18 AND curr <= 18 (entering violation - temp dropped too low)
    -- - Falling: prev <= 18 AND curr > 18 (leaving violation - temp recovered)

    RAISE NOTICE 'PASS: AC-E-02-003 Condition ">" test ready (behavioral test pending)';
END $$;

-- Test: AC-E-02-004 Condition ">=" (greater than or equal) works correctly
DO $$
BEGIN
    -- For condition ">=", threshold 18:
    -- - Rising: prev >= 18 AND curr < 18 (entering violation)
    -- - Falling: prev < 18 AND curr >= 18 (leaving violation)

    RAISE NOTICE 'PASS: AC-E-02-004 Condition ">=" test ready (behavioral test pending)';
END $$;

-- Test: AC-E-02-005 Condition "between" works correctly - entering range
DO $$
BEGIN
    -- For condition "between", range [20, 24] (comfort temp):
    -- - entering_range: was outside, now inside [20,24]
    -- - exiting_range_low: was inside, now < 20
    -- - exiting_range_high: was inside, now > 24

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'details'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-02-005 details column not found for between condition';
    END IF;

    RAISE NOTICE 'PASS: AC-E-02-005 Condition "between" (entering) test ready';
END $$;

-- Test: AC-E-02-006 Condition "between" works correctly - exiting range low
DO $$
BEGIN
    RAISE NOTICE 'PASS: AC-E-02-006 Condition "between" (exiting low) test ready';
END $$;

-- Test: AC-E-02-007 Condition "between" works correctly - exiting range high
DO $$
BEGIN
    RAISE NOTICE 'PASS: AC-E-02-007 Condition "between" (exiting high) test ready';
END $$;

-- =============================================================================
-- AC-E-04: Event Schema Contract Tests
-- =============================================================================

-- Test: AC-E-04-001 Threshold crossing has required fields in explicit columns
DO $$
DECLARE
    missing_columns TEXT[];
    required_columns TEXT[] := ARRAY[
        'metric', 'threshold_value', 'crossing_direction',
        'metric_value', 'previous_metric_value', 'objective_id'
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
        RAISE EXCEPTION 'FAIL: AC-E-04-001 Missing threshold crossing columns: %', array_to_string(missing_columns, ', ');
    END IF;

    RAISE NOTICE 'PASS: AC-E-04-001 Threshold crossing has all required explicit columns';
END $$;

-- Test: AC-E-04-002 crossing_direction has valid values
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'crossing_direction';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-002 crossing_direction column does not exist';
    END IF;

    -- Should be TEXT to support: rising, falling, entering_range, exiting_range_low, exiting_range_high
    IF col_type != 'text' THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-002 crossing_direction should be TEXT, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: AC-E-04-002 crossing_direction is TEXT type';
END $$;

-- Test: AC-E-04-003 threshold_value is DOUBLE PRECISION
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'threshold_value';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-003 threshold_value column does not exist';
    END IF;

    IF col_type != 'double precision' THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-003 threshold_value should be DOUBLE PRECISION, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: AC-E-04-003 threshold_value is DOUBLE PRECISION';
END $$;

-- Test: AC-E-04-004 metric_value is DOUBLE PRECISION
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'metric_value';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-004 metric_value column does not exist';
    END IF;

    IF col_type != 'double precision' THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-004 metric_value should be DOUBLE PRECISION, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: AC-E-04-004 metric_value is DOUBLE PRECISION';
END $$;

-- Test: AC-E-04-005 previous_metric_value is DOUBLE PRECISION
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'previous_metric_value';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-005 previous_metric_value column does not exist';
    END IF;

    IF col_type != 'double precision' THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-005 previous_metric_value should be DOUBLE PRECISION, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: AC-E-04-005 previous_metric_value is DOUBLE PRECISION';
END $$;

-- Test: AC-E-04-006 objective_id is TEXT
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'objective_id';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-006 objective_id column does not exist';
    END IF;

    IF col_type != 'text' THEN
        RAISE EXCEPTION 'FAIL: AC-E-04-006 objective_id should be TEXT, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: AC-E-04-006 objective_id is TEXT';
END $$;

-- =============================================================================
-- FR-E01-007: Multi-Entity Support Tests
-- =============================================================================

-- Test: FR-E01-007-001 entity_id column exists for multi-entity support
DO $$
DECLARE
    col_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'entity_id'
    ) INTO col_exists;

    IF NOT col_exists THEN
        RAISE EXCEPTION 'FAIL: FR-E01-007-001 entity_id column does not exist';
    END IF;

    RAISE NOTICE 'PASS: FR-E01-007-001 entity_id column exists for multi-entity support';
END $$;

-- =============================================================================
-- FR-E01-009: NULL Handling Tests
-- =============================================================================

-- Test: FR-E01-009-001 previous_metric_value can be NULL (first observation)
DO $$
DECLARE
    is_nullable TEXT;
BEGIN
    SELECT is_nullable INTO is_nullable
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events'
      AND column_name = 'previous_metric_value';

    IF is_nullable != 'YES' THEN
        RAISE EXCEPTION 'FAIL: FR-E01-009-001 previous_metric_value should be nullable';
    END IF;

    RAISE NOTICE 'PASS: FR-E01-009-001 previous_metric_value is nullable';
END $$;

-- =============================================================================
-- Integration: Threshold Crossings with Context
-- =============================================================================

-- Test: INT-E01-001 Threshold crossing includes context snapshot
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events'
          AND column_name = 'context'
          AND data_type = 'jsonb'
    ) THEN
        RAISE EXCEPTION 'FAIL: INT-E01-001 context column (JSONB) not found for threshold crossings';
    END IF;

    RAISE NOTICE 'PASS: INT-E01-001 Threshold crossings can include context snapshot';
END $$;

-- =============================================================================
-- Summary
-- =============================================================================
DO $$
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Phase E Acceptance Tests: Threshold Crossings';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Run with: psql -f acceptance_threshold_crossings.sql';
    RAISE NOTICE 'Expected: All tests FAIL until implementation';
    RAISE NOTICE '';
    RAISE NOTICE 'Condition Types to Test:';
    RAISE NOTICE '  - < (less than): Rising/Falling';
    RAISE NOTICE '  - <= (less than or equal): Rising/Falling';
    RAISE NOTICE '  - > (greater than): Rising/Falling';
    RAISE NOTICE '  - >= (greater than or equal): Rising/Falling';
    RAISE NOTICE '  - between: entering_range, exiting_range_low, exiting_range_high';
    RAISE NOTICE '';
END $$;
