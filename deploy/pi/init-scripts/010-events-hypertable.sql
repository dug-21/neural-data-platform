-- ops-007: Gold Events Hypertable — Global cross-domain events table
-- Source: Extracted from EventsGenerator (crates/ndp-lib/src/gold/generators/events.rs)
-- Run order: 10th (depends on gold schema from 002-schemas.sql)
-- Idempotent: Yes (IF NOT EXISTS throughout)
--
-- Design decision: The events table is a GLOBAL cross-domain resource.
-- All domains emit events into a single table, enabling cross-domain
-- correlation. Domain-specific CAs and detection procedures remain in
-- EventsGenerator (config-driven, per-domain).

-- Table
CREATE TABLE IF NOT EXISTS gold.events (
    -- Identity
    event_id UUID DEFAULT gen_random_uuid(),
    event_time TIMESTAMPTZ NOT NULL,

    -- Event classification
    stream_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,

    -- State transition fields (NULL for threshold crossings)
    from_state TEXT,
    to_state TEXT,
    duration_in_state_ms BIGINT,

    -- Threshold crossing fields (NULL for state transitions)
    metric TEXT,
    threshold_value DOUBLE PRECISION,
    crossing_direction TEXT,
    metric_value DOUBLE PRECISION,
    previous_metric_value DOUBLE PRECISION,
    objective_id TEXT,

    -- Context snapshot at event time (for correlation)
    context JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Extensible details
    details JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Composite PK required for TimescaleDB hypertable partitioning on event_time
    PRIMARY KEY (event_id, event_time)
);

-- Convert to hypertable (idempotent)
SELECT create_hypertable('gold.events', 'event_time',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Indexes (all IF NOT EXISTS)
CREATE INDEX IF NOT EXISTS idx_events_time
    ON gold.events (event_time DESC);

CREATE INDEX IF NOT EXISTS idx_events_type_time
    ON gold.events (event_type, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_events_entity_time
    ON gold.events (entity_id, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_events_objective
    ON gold.events (objective_id, event_time DESC)
    WHERE event_type = 'threshold_crossing';

CREATE INDEX IF NOT EXISTS idx_events_context
    ON gold.events USING GIN (context);

CREATE INDEX IF NOT EXISTS idx_events_details
    ON gold.events USING GIN (details);

-- Retention policy (1 year default)
SELECT add_retention_policy('gold.events', INTERVAL '1 year', if_not_exists => TRUE);

-- Unified events view for V1.2 API compatibility
CREATE OR REPLACE VIEW gold.events_unified AS
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    CASE event_type
        WHEN 'state_transition' THEN
            jsonb_build_object(
                'from_state', from_state,
                'to_state', to_state,
                'duration_in_previous_ms', duration_in_state_ms
            )
        WHEN 'threshold_crossing' THEN
            jsonb_build_object(
                'metric', metric,
                'threshold', threshold_value,
                'direction', crossing_direction,
                'value', metric_value,
                'previous_value', previous_metric_value,
                'objective_id', objective_id
            )
        ELSE details
    END AS details,
    context
FROM gold.events
ORDER BY event_time, event_type, event_id;

COMMENT ON TABLE gold.events IS
    'Events hypertable: state transitions and threshold crossings with context snapshots. For V1.2 Pattern Detection.';

COMMENT ON VIEW gold.events_unified IS
    'V1.2 API view on events hypertable. Provides backward-compatible schema.';

DO $$ BEGIN
  RAISE NOTICE 'NDP init [010]: Gold events hypertable, indexes, retention, unified view created';
END $$;
