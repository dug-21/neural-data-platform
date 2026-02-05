-- ============================================================================
-- MIGRATION: 003_events_hourly_ca.sql
-- Feature: v11-013 (Events Hourly Continuous Aggregate) - SPEC-E02
-- Author: NDP TimescaleDB Developer
-- Date: 2026-02-05
--
-- Creates gold.events_hourly continuous aggregate for dashboard metrics.
-- Now works because gold.events is a hypertable (not a view).
--
-- Idempotent: Checks for existing CA before creation
-- ============================================================================

-- ============================================================================
-- CONTINUOUS AGGREGATE: gold.events_hourly
-- Purpose: Hourly event counts for dashboards and trending
-- ============================================================================

-- Check if CA exists before creating
-- TimescaleDB continuous aggregates cannot use CREATE ... IF NOT EXISTS
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'events_hourly'
    ) THEN
        -- Create the continuous aggregate
        CREATE MATERIALIZED VIEW gold.events_hourly
        WITH (timescaledb.continuous) AS
        SELECT
            -- Time bucket (hourly)
            time_bucket('1 hour', event_time) AS bucket,

            -- Total event count
            COUNT(*) AS total_events,

            -- Event type breakdown
            COUNT(*) FILTER (WHERE event_type = 'state_transition')
                AS state_transition_count,
            COUNT(*) FILTER (WHERE event_type = 'threshold_crossing')
                AS threshold_crossing_count,

            -- Future event types (will be 0 until implemented)
            COUNT(*) FILTER (WHERE event_type = 'anomaly')
                AS anomaly_count,
            COUNT(*) FILTER (WHERE event_type = 'trend_change')
                AS trend_change_count,

            -- Distinct entity count (how many entities had events)
            COUNT(DISTINCT entity_id) AS distinct_entities_with_events,

            -- Distinct stream count (how many streams generated events)
            COUNT(DISTINCT stream_id) AS distinct_streams_with_events,

            -- State transition specifics
            COUNT(*) FILTER (WHERE event_type = 'state_transition' AND to_state = 'on')
                AS window_open_count,
            COUNT(*) FILTER (WHERE event_type = 'state_transition' AND to_state = 'off')
                AS window_close_count,

            -- Threshold crossing specifics (by direction)
            COUNT(*) FILTER (WHERE event_type = 'threshold_crossing' AND crossing_direction = 'rising')
                AS rising_threshold_count,
            COUNT(*) FILTER (WHERE event_type = 'threshold_crossing' AND crossing_direction = 'falling')
                AS falling_threshold_count

        FROM gold.events
        GROUP BY bucket
        WITH NO DATA;

        RAISE NOTICE 'gold.events_hourly continuous aggregate created';
    ELSE
        RAISE NOTICE 'gold.events_hourly already exists, skipping creation';
    END IF;
END $$;

-- ============================================================================
-- REFRESH POLICY
-- Purpose: Automatically refresh the continuous aggregate
-- ============================================================================

-- Refresh policy: every 15 minutes
-- start_offset: Look back 2 hours (ensure we catch late data)
-- end_offset: Don't refresh the most recent hour (data still arriving)
SELECT add_continuous_aggregate_policy('gold.events_hourly',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes',
    if_not_exists => TRUE
);

-- ============================================================================
-- INDEX: Optimize time-range queries on CA
-- ============================================================================

-- Index on bucket for efficient time-range queries
-- Note: Index creation on materialized views requires explicit naming
CREATE INDEX IF NOT EXISTS idx_events_hourly_bucket
    ON gold.events_hourly (bucket DESC);

-- ============================================================================
-- COMMENTS: Documentation
-- ============================================================================

COMMENT ON MATERIALIZED VIEW gold.events_hourly IS
    'Hourly event aggregates from gold.events hypertable.
     Counts events by type, tracks distinct entities, and provides state transition metrics.
     Refreshes automatically every 15 minutes.
     Use for: Dashboard time-series, trending analysis, event monitoring.';

-- ============================================================================
-- OPTIONAL: Events by Entity Continuous Aggregate
-- Purpose: Hourly counts per entity for entity-specific dashboards
-- ============================================================================

-- This CA provides per-entity breakdown for detailed analysis
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'events_hourly_by_entity'
    ) THEN
        CREATE MATERIALIZED VIEW gold.events_hourly_by_entity
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 hour', event_time) AS bucket,
            entity_id,
            stream_id,

            COUNT(*) AS total_events,
            COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
            COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count

        FROM gold.events
        GROUP BY bucket, entity_id, stream_id
        WITH NO DATA;

        RAISE NOTICE 'gold.events_hourly_by_entity continuous aggregate created';
    ELSE
        RAISE NOTICE 'gold.events_hourly_by_entity already exists, skipping creation';
    END IF;
END $$;

-- Refresh policy for per-entity CA
SELECT add_continuous_aggregate_policy('gold.events_hourly_by_entity',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes',
    if_not_exists => TRUE
);

-- Index for entity-specific queries
CREATE INDEX IF NOT EXISTS idx_events_hourly_by_entity_bucket
    ON gold.events_hourly_by_entity (bucket DESC);

CREATE INDEX IF NOT EXISTS idx_events_hourly_by_entity_entity
    ON gold.events_hourly_by_entity (entity_id, bucket DESC);

COMMENT ON MATERIALIZED VIEW gold.events_hourly_by_entity IS
    'Hourly event aggregates per entity from gold.events hypertable.
     Use for: Entity-specific dashboards, per-sensor event tracking.';

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'gold.events_hourly continuous aggregates created successfully (v11-013 SPEC-E02)';
    RAISE NOTICE 'Refresh interval: 15 minutes';
    RAISE NOTICE 'Start offset: 2 hours, End offset: 1 hour';
    RAISE NOTICE 'Aggregates: events_hourly (global), events_hourly_by_entity (per entity)';
END $$;
