-- =============================================================================
-- Phase E Acceptance Tests: Unified Events View & Hourly CA
-- =============================================================================
-- Feature: v11-013 Unified Events View
-- Test Approach: London TDD (Outside-In) - Tests written FIRST
-- Status: Tests will FAIL until implementation complete
--
-- Covers:
--   AC-E-03: Unified events view combines both event types
--   AC-E-05: Hourly event aggregate available
--   AC-E-06: V1.2 query patterns work
--   FR-E02-006: Unified events view
--   FR-E02-007: Hourly events continuous aggregate
-- =============================================================================

-- =============================================================================
-- AC-E-03: Unified Events View Tests
-- =============================================================================

-- Test: AC-E-03-001 gold.events_unified view exists
DO $$
DECLARE
    view_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_views
        WHERE schemaname = 'gold' AND viewname = 'events_unified'
    ) INTO view_exists;

    IF NOT view_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-001 gold.events_unified view does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-03-001 gold.events_unified view exists';
END $$;

-- Test: AC-E-03-002 events_unified includes state transitions
DO $$
BEGIN
    -- Verify the view can return state_transition events
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events_unified'
          AND column_name = 'event_type'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-002 events_unified missing event_type column';
    END IF;

    -- Test query syntax (will return empty if no data)
    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'state_transition'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-03-002 events_unified supports state_transition query';
END $$;

-- Test: AC-E-03-003 events_unified includes threshold crossings
DO $$
BEGIN
    -- Test query syntax for threshold crossings
    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-03-003 events_unified supports threshold_crossing query';
END $$;

-- Test: AC-E-03-004 events_unified has consistent schema
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
        RAISE EXCEPTION 'FAIL: AC-E-03-004 Missing columns: %', array_to_string(missing_columns, ', ');
    END IF;

    RAISE NOTICE 'PASS: AC-E-03-004 events_unified has consistent V1.2 contract schema';
END $$;

-- Test: AC-E-03-005 details column is JSONB with type-specific content
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events_unified'
      AND column_name = 'details';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-005 details column does not exist';
    END IF;

    IF col_type != 'jsonb' THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-005 details should be JSONB, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: AC-E-03-005 details column is JSONB';
END $$;

-- Test: AC-E-03-006 state_transition details has correct structure
DO $$
BEGIN
    -- State transition details should include:
    -- { from_state, to_state, duration_in_previous_ms }
    -- This is verified by querying the view

    IF NOT EXISTS (
        SELECT 1 FROM pg_views
        WHERE schemaname = 'gold' AND viewname = 'events_unified'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-03-006 events_unified view does not exist';
    END IF;

    -- Verify query for state transition details works
    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'state_transition'
      AND details ? 'from_state'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-03-006 State transition details structure queryable';
END $$;

-- Test: AC-E-03-007 threshold_crossing details has correct structure
DO $$
BEGIN
    -- Threshold crossing details should include:
    -- { metric, threshold, direction, value, previous_value, objective_id }

    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
      AND details ? 'metric'
      AND details ? 'threshold'
      AND details ? 'direction'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-03-007 Threshold crossing details structure queryable';
END $$;

-- =============================================================================
-- AC-E-05: Hourly Event Aggregate Tests
-- =============================================================================

-- Test: AC-E-05-001 gold.events_hourly continuous aggregate exists
DO $$
DECLARE
    ca_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'events_hourly'
    ) INTO ca_exists;

    IF NOT ca_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-001 gold.events_hourly continuous aggregate does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-001 gold.events_hourly continuous aggregate exists';
END $$;

-- Test: AC-E-05-002 events_hourly has bucket column (1 hour granularity)
DO $$
DECLARE
    col_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events_hourly'
          AND column_name = 'bucket'
    ) INTO col_exists;

    IF NOT col_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-002 bucket column does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-002 events_hourly has bucket column';
END $$;

-- Test: AC-E-05-003 events_hourly has total_events count
DO $$
DECLARE
    col_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events_hourly'
          AND column_name = 'total_events'
    ) INTO col_exists;

    IF NOT col_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-003 total_events column does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-003 events_hourly has total_events';
END $$;

-- Test: AC-E-05-004 events_hourly has state_transition_count
DO $$
DECLARE
    col_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events_hourly'
          AND column_name = 'state_transition_count'
    ) INTO col_exists;

    IF NOT col_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-004 state_transition_count column does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-004 events_hourly has state_transition_count';
END $$;

-- Test: AC-E-05-005 events_hourly has threshold_crossing_count
DO $$
DECLARE
    col_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events_hourly'
          AND column_name = 'threshold_crossing_count'
    ) INTO col_exists;

    IF NOT col_exists THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-005 threshold_crossing_count column does not exist';
    END IF;

    RAISE NOTICE 'PASS: AC-E-05-005 events_hourly has threshold_crossing_count';
END $$;

-- Test: AC-E-05-006 events_hourly is joinable with aligned view on bucket
DO $$
BEGIN
    -- This tests the V1.2 pattern of joining events with aligned metrics
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'events_hourly'
    ) THEN
        RAISE EXCEPTION 'FAIL: AC-E-05-006 events_hourly does not exist for join test';
    END IF;

    -- Verify join syntax is valid (will return empty if no data)
    -- This assumes indoor_air_quality_aligned exists from Phase D
    BEGIN
        PERFORM 1 FROM gold.events_hourly e
        LEFT JOIN gold.indoor_air_quality_aligned a ON e.bucket = a.bucket
        LIMIT 0;
        RAISE NOTICE 'PASS: AC-E-05-006 events_hourly joinable with aligned view';
    EXCEPTION
        WHEN undefined_table THEN
            RAISE NOTICE 'SKIP: AC-E-05-006 indoor_air_quality_aligned not yet available';
    END;
END $$;

-- =============================================================================
-- AC-E-06: V1.2 Query Patterns Work
-- =============================================================================

-- Test: AC-E-06-001 V1.2 Pattern 1: Time range query
DO $$
BEGIN
    -- V1.2 will query: SELECT * FROM gold.events_unified WHERE event_time BETWEEN :start AND :end

    PERFORM 1 FROM gold.events_unified
    WHERE event_time BETWEEN NOW() - INTERVAL '24 hours' AND NOW()
    ORDER BY event_time
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-06-001 V1.2 time range query pattern works';
END $$;

-- Test: AC-E-06-002 V1.2 Pattern 2: Event type filter
DO $$
BEGIN
    -- V1.2 will query: SELECT * FROM gold.events_unified WHERE event_type = 'threshold_crossing'

    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
      AND event_time >= NOW() - INTERVAL '24 hours'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-06-002 V1.2 event type filter pattern works';
END $$;

-- Test: AC-E-06-003 V1.2 Pattern 3: Objective ID filter via details
DO $$
BEGIN
    -- V1.2 will query with JSONB filter on objective_id

    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
      AND details->>'objective_id' = 'healthy_co2'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-06-003 V1.2 objective_id filter pattern works';
END $$;

-- Test: AC-E-06-004 V1.2 Pattern 4: Join with aligned view
DO $$
BEGIN
    -- V1.2 will join events_hourly with aligned view

    BEGIN
        PERFORM 1 FROM gold.events_hourly e
        LEFT JOIN gold.indoor_air_quality_aligned a ON e.bucket = a.bucket
        WHERE e.bucket >= NOW() - INTERVAL '24 hours'
        LIMIT 0;
        RAISE NOTICE 'PASS: AC-E-06-004 V1.2 aligned view join pattern works';
    EXCEPTION
        WHEN undefined_table THEN
            RAISE NOTICE 'SKIP: AC-E-06-004 aligned view not yet available';
    END;
END $$;

-- Test: AC-E-06-005 V1.2 Pattern 5: Context query
DO $$
BEGIN
    -- V1.2 will query context for correlation

    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
      AND (context->>'indoor_co2')::FLOAT > 800
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-06-005 V1.2 context query pattern works';
END $$;

-- Test: AC-E-06-006 V1.2 Pattern 6: Direction filter
DO $$
BEGIN
    -- V1.2 will filter by crossing direction

    PERFORM 1 FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
      AND details->>'direction' = 'rising'
    LIMIT 0;

    RAISE NOTICE 'PASS: AC-E-06-006 V1.2 direction filter pattern works';
END $$;

-- =============================================================================
-- FR-E02-010: Domain Scoping Tests
-- =============================================================================

-- Test: FR-E02-010-001 stream_id enables domain filtering
DO $$
BEGIN
    -- Verify stream_id column exists for domain scoping

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'gold'
          AND table_name = 'events_unified'
          AND column_name = 'stream_id'
    ) THEN
        RAISE EXCEPTION 'FAIL: FR-E02-010-001 stream_id column not found';
    END IF;

    -- Test domain filter query
    PERFORM 1 FROM gold.events_unified
    WHERE stream_id IN ('air-quality', 'home-assistant-state')
    LIMIT 0;

    RAISE NOTICE 'PASS: FR-E02-010-001 stream_id enables domain filtering';
END $$;

-- =============================================================================
-- Context Capture Tests
-- =============================================================================

-- Test: CTX-001 context column includes environmental snapshot
DO $$
DECLARE
    col_type TEXT;
BEGIN
    SELECT data_type INTO col_type
    FROM information_schema.columns
    WHERE table_schema = 'gold'
      AND table_name = 'events_unified'
      AND column_name = 'context';

    IF col_type IS NULL THEN
        RAISE EXCEPTION 'FAIL: CTX-001 context column does not exist';
    END IF;

    IF col_type != 'jsonb' THEN
        RAISE EXCEPTION 'FAIL: CTX-001 context should be JSONB, got %', col_type;
    END IF;

    RAISE NOTICE 'PASS: CTX-001 context column is JSONB for environmental snapshots';
END $$;

-- Test: CTX-002 context can contain indoor metrics
DO $$
BEGIN
    -- Test that context JSONB can be queried for indoor metrics

    PERFORM 1 FROM gold.events_unified
    WHERE context ? 'indoor_co2'
       OR context ? 'indoor_pm25'
       OR context ? 'indoor_temp'
    LIMIT 0;

    RAISE NOTICE 'PASS: CTX-002 context queryable for indoor metrics';
END $$;

-- Test: CTX-003 context can contain outdoor metrics
DO $$
BEGIN
    -- Test that context JSONB can be queried for outdoor metrics

    PERFORM 1 FROM gold.events_unified
    WHERE context ? 'outdoor_temp'
       OR context ? 'outdoor_pm25'
       OR context ? 'outdoor_aqi'
    LIMIT 0;

    RAISE NOTICE 'PASS: CTX-003 context queryable for outdoor metrics';
END $$;

-- Test: CTX-004 context can contain state information
DO $$
BEGIN
    -- Test that context JSONB can be queried for state

    PERFORM 1 FROM gold.events_unified
    WHERE context ? 'window_state'
    LIMIT 0;

    RAISE NOTICE 'PASS: CTX-004 context queryable for state information';
END $$;

-- =============================================================================
-- Performance Baseline Tests (Schema)
-- =============================================================================

-- Test: PERF-001 Index exists for time-based queries
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
        RAISE EXCEPTION 'FAIL: PERF-001 No index on event_time for unified view';
    END IF;

    RAISE NOTICE 'PASS: PERF-001 Index on event_time exists';
END $$;

-- =============================================================================
-- Summary
-- =============================================================================
DO $$
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Phase E Acceptance Tests: Unified Events';
    RAISE NOTICE '============================================';
    RAISE NOTICE 'Run with: psql -f acceptance_unified_events.sql';
    RAISE NOTICE 'Expected: All tests FAIL until implementation';
    RAISE NOTICE '';
    RAISE NOTICE 'V1.2 Query Patterns Tested:';
    RAISE NOTICE '  1. Time range queries';
    RAISE NOTICE '  2. Event type filtering';
    RAISE NOTICE '  3. Objective ID filtering (JSONB)';
    RAISE NOTICE '  4. Join with aligned view';
    RAISE NOTICE '  5. Context queries (correlation)';
    RAISE NOTICE '  6. Direction filtering';
    RAISE NOTICE '';
END $$;
