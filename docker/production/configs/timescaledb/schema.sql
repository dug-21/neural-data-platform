-- Market data table
CREATE TABLE IF NOT EXISTS market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    open DOUBLE PRECISION,
    high DOUBLE PRECISION,
    low DOUBLE PRECISION,
    close DOUBLE PRECISION NOT NULL,
    volume BIGINT,
    provider TEXT,
    metadata JSONB,
    CONSTRAINT market_data_time_symbol_provider_key UNIQUE (time, symbol, provider)
);

-- Convert to hypertable
SELECT create_hypertable('market_data', 'time', if_not_exists => TRUE);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_market_data_symbol_time ON market_data (symbol, time DESC);
CREATE INDEX IF NOT EXISTS idx_market_data_metadata ON market_data USING GIN (metadata);

-- Predictions table
CREATE TABLE IF NOT EXISTS predictions (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    model_name TEXT NOT NULL,
    horizon INTEGER NOT NULL,
    predicted_value DOUBLE PRECISION NOT NULL,
    confidence DOUBLE PRECISION,
    interval_low DOUBLE PRECISION,
    interval_high DOUBLE PRECISION,
    metadata JSONB
);

-- Convert to hypertable
SELECT create_hypertable('predictions', 'time', if_not_exists => TRUE);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_predictions_symbol_model ON predictions (symbol, model_name, time DESC);

-- Trading decisions table
CREATE TABLE IF NOT EXISTS trading_decisions (
    time TIMESTAMPTZ NOT NULL,
    decision_id UUID NOT NULL,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,
    position_size DOUBLE PRECISION,
    reasoning TEXT,
    agent_id TEXT,
    metadata JSONB
);

-- Convert to hypertable
SELECT create_hypertable('trading_decisions', 'time', if_not_exists => TRUE);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_trading_decisions_symbol ON trading_decisions (symbol, time DESC);
CREATE INDEX IF NOT EXISTS idx_trading_decisions_agent ON trading_decisions (agent_id, time DESC);

-- Performance metrics table
CREATE TABLE IF NOT EXISTS performance_metrics (
    time TIMESTAMPTZ NOT NULL,
    metric_name TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    labels JSONB
);

-- Convert to hypertable
SELECT create_hypertable('performance_metrics', 'time', if_not_exists => TRUE);

-- Create continuous aggregates for common queries
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_1h
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS bucket,
    symbol,
    first(open, time) as open,
    max(high) as high,
    min(low) as low,
    last(close, time) as close,
    sum(volume) as volume
FROM market_data
GROUP BY bucket, symbol
WITH NO DATA;

-- Refresh policy for continuous aggregate
SELECT add_continuous_aggregate_policy('market_data_1h',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);

-- Data retention policy (keep 1 year of raw data)
SELECT add_retention_policy('market_data', INTERVAL '1 year', if_not_exists => TRUE);
SELECT add_retention_policy('predictions', INTERVAL '3 months', if_not_exists => TRUE);
SELECT add_retention_policy('trading_decisions', INTERVAL '1 year', if_not_exists => TRUE);

-- Grant permissions dynamically
DO $$
DECLARE
    db_owner text;
    readonly_user text;
BEGIN
    -- Get the database owner
    SELECT pg_database.datdba::regrole::text INTO db_owner 
    FROM pg_database 
    WHERE datname = current_database();
    
    -- Create readonly username based on the database owner
    readonly_user := db_owner || '_readonly';
    
    -- Grant permissions to main user
    EXECUTE format('GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO %I', db_owner);
    EXECUTE format('GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO %I', db_owner);
    
    -- Grant read permissions to readonly user (if exists)
    IF EXISTS (SELECT FROM pg_user WHERE usename = readonly_user) THEN
        EXECUTE format('GRANT SELECT ON ALL TABLES IN SCHEMA public TO %I', readonly_user);
    END IF;
END $$;