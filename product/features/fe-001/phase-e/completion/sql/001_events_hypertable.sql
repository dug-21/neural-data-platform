-- ============================================================================
-- MIGRATION: 001_events_hypertable.sql
-- Feature: v11-013 (Events Hypertable) - SPEC-E02
-- Author: NDP TimescaleDB Developer
-- Date: 2026-02-05
--
-- Creates gold.events hypertable for unified event storage.
-- Events include state transitions and threshold crossings with context snapshots.
-- Enables continuous aggregates on events (hypertable, not view).
--
-- Idempotent: Safe to run multiple times (IF NOT EXISTS, if_not_exists => TRUE)
-- ============================================================================

-- Ensure gold schema exists
CREATE SCHEMA IF NOT EXISTS gold;

-- ============================================================================
-- TABLE: gold.events
-- Purpose: Unified events hypertable for state transitions and threshold crossings
-- ============================================================================

CREATE TABLE IF NOT EXISTS gold.events (
    -- Identity
    event_id            UUID DEFAULT gen_random_uuid() NOT NULL,
    event_time          TIMESTAMPTZ NOT NULL,

    -- Event classification
    stream_id           TEXT NOT NULL,
    entity_id           TEXT NOT NULL,
    event_type          TEXT NOT NULL,

    -- State transition fields (NULL for threshold crossings)
    from_state          TEXT,
    to_state            TEXT,
    duration_in_state_ms BIGINT,

    -- Threshold crossing fields (NULL for state transitions)
    metric              TEXT,
    threshold_value     DOUBLE PRECISION,
    crossing_direction  TEXT,
    metric_value        DOUBLE PRECISION,
    previous_metric_value DOUBLE PRECISION,
    objective_id        TEXT,

    -- Context snapshot at event time (for correlation)
    context             JSONB NOT NULL DEFAULT '{}',

    -- Extensible details
    details             JSONB NOT NULL DEFAULT '{}',

    -- Primary key on event_id
    PRIMARY KEY (event_id, event_time)
);

-- ============================================================================
-- HYPERTABLE CONVERSION
-- Purpose: Enable TimescaleDB time-series features on events
-- ============================================================================

-- Convert to hypertable with 7-day chunk interval
-- 7 days balances query performance with chunk management overhead on Pi
SELECT create_hypertable('gold.events', 'event_time',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- ============================================================================
-- INDEXES: V1.2 Query Pattern Optimization
-- ============================================================================

-- Primary access pattern: Time range queries
CREATE INDEX IF NOT EXISTS idx_events_time
    ON gold.events (event_time DESC);

-- Filter by event type + time (most common filter)
CREATE INDEX IF NOT EXISTS idx_events_type_time
    ON gold.events (event_type, event_time DESC);

-- Filter by entity + time (entity-specific event history)
CREATE INDEX IF NOT EXISTS idx_events_entity_time
    ON gold.events (entity_id, event_time DESC);

-- Filter by stream + time (stream-specific events)
CREATE INDEX IF NOT EXISTS idx_events_stream_time
    ON gold.events (stream_id, event_time DESC);

-- Filter by objective (threshold crossing queries)
-- Partial index: only threshold_crossing events have objective_id
CREATE INDEX IF NOT EXISTS idx_events_objective
    ON gold.events (objective_id, event_time DESC)
    WHERE event_type = 'threshold_crossing';

-- Context queries (flexible JSONB filtering)
-- GIN index supports @>, ?, ?| operators on JSONB
CREATE INDEX IF NOT EXISTS idx_events_context
    ON gold.events USING GIN (context);

-- Details queries (extensible metadata)
CREATE INDEX IF NOT EXISTS idx_events_details
    ON gold.events USING GIN (details);

-- Compound index for state transition queries
-- Useful for: "all window open events" or "all window close events"
CREATE INDEX IF NOT EXISTS idx_events_state_transition
    ON gold.events (to_state, event_time DESC)
    WHERE event_type = 'state_transition';

-- ============================================================================
-- RETENTION POLICY
-- Purpose: Automatically drop events older than 1 year
-- ============================================================================

-- Events older than 1 year are automatically dropped
-- Rationale: Events can be reconstructed from Silver if needed
SELECT add_retention_policy('gold.events',
    INTERVAL '1 year',
    if_not_exists => TRUE
);

-- ============================================================================
-- COMPRESSION POLICY
-- Purpose: Compress older chunks to save storage on Pi
-- ============================================================================

-- Enable compression on the hypertable
ALTER TABLE gold.events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'stream_id, event_type',
    timescaledb.compress_orderby = 'event_time DESC'
);

-- Compress chunks older than 30 days
SELECT add_compression_policy('gold.events',
    INTERVAL '30 days',
    if_not_exists => TRUE
);

-- ============================================================================
-- COMMENTS: Documentation for data dictionary
-- ============================================================================

COMMENT ON TABLE gold.events IS
    'Events hypertable: state transitions and threshold crossings with context snapshots.
     Source: Detection job from silver.state_events and threshold crossing detection.
     Grain: One row per event occurrence.
     Use: V1.2 Pattern Detection Engine, correlation analysis, event aggregates.
     Note: Context snapshot captured at event time for correlation without joins.';

COMMENT ON COLUMN gold.events.event_id IS
    'Unique event identifier (UUID). Generated via gen_random_uuid().';

COMMENT ON COLUMN gold.events.event_time IS
    'When the event occurred. Time dimension for hypertable.';

COMMENT ON COLUMN gold.events.stream_id IS
    'Source stream identifier (e.g., home-assistant-state, air-quality).';

COMMENT ON COLUMN gold.events.entity_id IS
    'Entity identifier (ndp_id) that generated the event.';

COMMENT ON COLUMN gold.events.event_type IS
    'Event type: state_transition, threshold_crossing. Future: anomaly, trend_change.';

COMMENT ON COLUMN gold.events.from_state IS
    'Previous state value (state transitions only). NULL for threshold crossings.';

COMMENT ON COLUMN gold.events.to_state IS
    'New state value (state transitions only). NULL for threshold crossings.';

COMMENT ON COLUMN gold.events.duration_in_state_ms IS
    'Time spent in previous state in milliseconds (state transitions only).';

COMMENT ON COLUMN gold.events.metric IS
    'Metric name that crossed threshold (threshold crossings only).';

COMMENT ON COLUMN gold.events.threshold_value IS
    'Threshold value that was crossed (threshold crossings only).';

COMMENT ON COLUMN gold.events.crossing_direction IS
    'Direction of crossing: rising, falling, entering_range, exiting_range_low, exiting_range_high.';

COMMENT ON COLUMN gold.events.metric_value IS
    'Metric value at time of crossing (threshold crossings only).';

COMMENT ON COLUMN gold.events.previous_metric_value IS
    'Previous metric value before crossing (threshold crossings only).';

COMMENT ON COLUMN gold.events.objective_id IS
    'Reference to data_dictionary.objectives (threshold crossings only).';

COMMENT ON COLUMN gold.events.context IS
    'Environmental snapshot at event time from aligned view. For correlation without joins.';

COMMENT ON COLUMN gold.events.details IS
    'Extensible JSONB for event-specific metadata. Future-proofing.';

-- ============================================================================
-- CONSTRAINT: Valid event types
-- ============================================================================

-- Add check constraint for valid event types
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_events_event_type'
    ) THEN
        ALTER TABLE gold.events
        ADD CONSTRAINT chk_events_event_type
        CHECK (event_type IN ('state_transition', 'threshold_crossing', 'anomaly', 'trend_change'));
    END IF;
END $$;

-- Add check constraint for valid crossing directions
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_events_crossing_direction'
    ) THEN
        ALTER TABLE gold.events
        ADD CONSTRAINT chk_events_crossing_direction
        CHECK (
            crossing_direction IS NULL OR
            crossing_direction IN ('rising', 'falling', 'entering_range', 'exiting_range_low', 'exiting_range_high')
        );
    END IF;
END $$;

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'gold.events hypertable created successfully (v11-013 SPEC-E02)';
    RAISE NOTICE 'Chunk interval: 7 days';
    RAISE NOTICE 'Retention: 1 year';
    RAISE NOTICE 'Compression: After 30 days';
    RAISE NOTICE 'Indexes: time, type+time, entity+time, stream+time, objective (partial), context (GIN), details (GIN)';
END $$;
