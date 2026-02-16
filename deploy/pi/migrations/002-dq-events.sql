-- ops-008: Migration — DQ Events Hypertable
-- Source: deploy/timescaledb/init/001_silver_schema.sql Section 10
-- Runs after Phase 4 Silver table creation (auto-migrations)
-- Idempotent: Yes (IF NOT EXISTS, if_not_exists => TRUE)

CREATE TABLE IF NOT EXISTS silver.dq_events (
    event_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_stream       TEXT NOT NULL,
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    column_name         TEXT NOT NULL,
    rule_name           TEXT NOT NULL,
    original_value      TEXT,
    action_taken        TEXT NOT NULL,
    result_value        TEXT,
    PRIMARY KEY (event_time, source_stream, ndp_id, column_name)
);

SELECT create_hypertable('silver.dq_events', 'event_time',
    chunk_time_interval => INTERVAL '7 days', if_not_exists => TRUE);

SELECT add_retention_policy('silver.dq_events',
    INTERVAL '30 days', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_dq_events_stream
    ON silver.dq_events (source_stream, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_dq_events_rule
    ON silver.dq_events (rule_name, event_time DESC);

GRANT SELECT, INSERT ON silver.dq_events TO ndp_app;
GRANT SELECT ON silver.dq_events TO grafana_reader;

DO $$ BEGIN
  RAISE NOTICE 'ops-008 migration [002]: silver.dq_events hypertable created with 30-day retention';
END $$;
