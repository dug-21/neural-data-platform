-- Test schema for Neural Trader database
-- This creates all necessary tables for testing with test-optimized settings

-- Market data table (hypertable)
CREATE TABLE IF NOT EXISTS market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    price DECIMAL(10,4) NOT NULL,
    volume BIGINT DEFAULT 0,
    open_price DECIMAL(10,4),
    high_price DECIMAL(10,4),
    low_price DECIMAL(10,4),
    close_price DECIMAL(10,4),
    provider VARCHAR(20) DEFAULT 'test',
    data_quality_score DECIMAL(3,2) DEFAULT 1.0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (time, symbol)
);

-- Convert to hypertable with shorter chunk intervals for testing
SELECT create_hypertable('market_data', 'time', 
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);

-- Features table for feature engineering
CREATE TABLE IF NOT EXISTS features (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(10) NOT NULL,
    time TIMESTAMPTZ NOT NULL,
    feature_name VARCHAR(100) NOT NULL,
    feature_value DECIMAL(15,6),
    feature_type VARCHAR(50),
    calculation_method VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert features to hypertable
SELECT create_hypertable('features', 'time', 
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);

-- Neural model predictions table
CREATE TABLE IF NOT EXISTS predictions (
    id SERIAL PRIMARY KEY,
    model_name VARCHAR(50) NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    prediction_time TIMESTAMPTZ NOT NULL,
    target_time TIMESTAMPTZ NOT NULL,
    predicted_price DECIMAL(10,4),
    confidence_score DECIMAL(3,2),
    model_version VARCHAR(20),
    features_used JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert predictions to hypertable
SELECT create_hypertable('predictions', 'prediction_time', 
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);

-- Trading orders table
CREATE TABLE IF NOT EXISTS orders (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(10) NOT NULL,
    order_type VARCHAR(10) NOT NULL, -- BUY, SELL
    quantity DECIMAL(15,6) NOT NULL,
    price DECIMAL(10,4),
    status VARCHAR(20) DEFAULT 'PENDING',
    order_time TIMESTAMPTZ NOT NULL,
    execution_time TIMESTAMPTZ,
    provider VARCHAR(20) DEFAULT 'test',
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Sentiment data table
CREATE TABLE IF NOT EXISTS sentiment_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    source VARCHAR(50) NOT NULL,
    sentiment_score DECIMAL(3,2), -- -1.0 to 1.0
    confidence DECIMAL(3,2),
    text_content TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (time, symbol, source)
);

-- Convert sentiment to hypertable
SELECT create_hypertable('sentiment_data', 'time', 
    chunk_time_interval => INTERVAL '2 hours',
    if_not_exists => TRUE);

-- Performance metrics table
CREATE TABLE IF NOT EXISTS performance_metrics (
    time TIMESTAMPTZ NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DECIMAL(15,6),
    tags JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (time, metric_name)
);

-- Convert metrics to hypertable
SELECT create_hypertable('performance_metrics', 'time', 
    chunk_time_interval => INTERVAL '30 minutes',
    if_not_exists => TRUE);

-- Test runs table for tracking test executions
CREATE TABLE IF NOT EXISTS test_runs (
    id SERIAL PRIMARY KEY,
    test_name VARCHAR(100) NOT NULL,
    test_type VARCHAR(50) NOT NULL,
    status VARCHAR(20) DEFAULT 'RUNNING',
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_seconds INTEGER,
    test_data JSONB,
    results JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_market_data_symbol ON market_data (symbol, time DESC);
CREATE INDEX IF NOT EXISTS idx_features_symbol_type ON features (symbol, feature_type, time DESC);
CREATE INDEX IF NOT EXISTS idx_predictions_model_symbol ON predictions (model_name, symbol, target_time DESC);
CREATE INDEX IF NOT EXISTS idx_orders_symbol_status ON orders (symbol, status, order_time DESC);
CREATE INDEX IF NOT EXISTS idx_sentiment_symbol_source ON sentiment_data (symbol, source, time DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_name ON performance_metrics (metric_name, time DESC);
CREATE INDEX IF NOT EXISTS idx_test_runs_name_type ON test_runs (test_name, test_type, start_time DESC);

-- Create retention policies for test data (shorter than production)
-- Keep only 7 days of market data in tests
SELECT add_retention_policy('market_data', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('features', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('predictions', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('sentiment_data', INTERVAL '3 days', if_not_exists => TRUE);
SELECT add_retention_policy('performance_metrics', INTERVAL '3 days', if_not_exists => TRUE);

-- Create continuous aggregates for common queries (test-optimized)
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_1min
WITH (timescaledb.continuous) AS
SELECT time_bucket('1 minute', time) AS bucket,
       symbol,
       FIRST(price, time) as open_price,
       MAX(price) as high_price,
       MIN(price) as low_price,
       LAST(price, time) as close_price,
       AVG(price) as avg_price,
       SUM(volume) as total_volume
FROM market_data
GROUP BY bucket, symbol;

-- Add refresh policy for continuous aggregates
SELECT add_continuous_aggregate_policy('market_data_1min',
    start_offset => INTERVAL '10 minutes',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '30 seconds',
    if_not_exists => TRUE);

-- Grant permissions on all tables to test user
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO test_user;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO test_user;