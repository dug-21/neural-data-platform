-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS timescaledb;
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS pg_cron;
CREATE EXTENSION IF NOT EXISTS pgvector;

-- Create schema for neural trader
CREATE SCHEMA IF NOT EXISTS neural_trader;

-- Set search path
SET search_path TO neural_trader, public;

-- Create optimized time_series_data table
CREATE TABLE IF NOT EXISTS time_series_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    open NUMERIC(20, 8) NOT NULL,
    high NUMERIC(20, 8) NOT NULL,
    low NUMERIC(20, 8) NOT NULL,
    close NUMERIC(20, 8) NOT NULL,
    volume NUMERIC(30, 8) NOT NULL,
    trades_count INTEGER,
    vwap NUMERIC(20, 8),
    bid NUMERIC(20, 8),
    ask NUMERIC(20, 8),
    bid_size NUMERIC(20, 8),
    ask_size NUMERIC(20, 8),
    -- Technical indicators (calculated on insert)
    rsi_14 NUMERIC(8, 4),
    macd_signal NUMERIC(20, 8),
    bollinger_upper NUMERIC(20, 8),
    bollinger_lower NUMERIC(20, 8),
    -- Market microstructure
    order_imbalance NUMERIC(8, 4),
    spread NUMERIC(20, 8) GENERATED ALWAYS AS (ask - bid) STORED,
    spread_pct NUMERIC(8, 6) GENERATED ALWAYS AS ((ask - bid) / NULLIF(bid, 0) * 100) STORED,
    PRIMARY KEY (time, symbol, exchange)
) PARTITION BY RANGE (time);

-- Create hypertable with optimized settings
SELECT create_hypertable(
    'time_series_data',
    'time',
    chunk_time_interval => INTERVAL '6 hours',
    create_default_indexes => FALSE,
    if_not_exists => TRUE
);

-- Create optimized indexes
CREATE INDEX idx_time_series_symbol_time ON time_series_data (symbol, time DESC) WITH (timescaledb.transaction_per_chunk);
CREATE INDEX idx_time_series_exchange_time ON time_series_data (exchange, time DESC) WITH (timescaledb.transaction_per_chunk);
CREATE INDEX idx_time_series_volume ON time_series_data (volume) WHERE volume > 0 WITH (timescaledb.transaction_per_chunk);

-- Create predictions table with partitioning
CREATE TABLE IF NOT EXISTS predictions (
    prediction_id UUID DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    model_name VARCHAR(100) NOT NULL,
    model_version VARCHAR(50) NOT NULL,
    prediction_time TIMESTAMPTZ NOT NULL,
    prediction_type VARCHAR(50) NOT NULL,
    prediction_horizon INTEGER NOT NULL,
    predicted_value JSONB NOT NULL,
    confidence NUMERIC(5, 4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    features_used JSONB,
    model_metadata JSONB,
    actual_value JSONB,
    error_metrics JSONB,
    PRIMARY KEY (prediction_id, created_at)
) PARTITION BY RANGE (created_at);

-- Create hypertable for predictions
SELECT create_hypertable(
    'predictions',
    'created_at',
    chunk_time_interval => INTERVAL '1 day',
    create_default_indexes => FALSE,
    if_not_exists => TRUE
);

-- Optimized indexes for predictions
CREATE INDEX idx_predictions_symbol_created ON predictions (symbol, created_at DESC) WITH (timescaledb.transaction_per_chunk);
CREATE INDEX idx_predictions_model_created ON predictions (model_name, created_at DESC) WITH (timescaledb.transaction_per_chunk);
CREATE INDEX idx_predictions_confidence ON predictions (confidence DESC) WHERE confidence > 0.8 WITH (timescaledb.transaction_per_chunk);

-- Create trades table with better partitioning
CREATE TABLE IF NOT EXISTS trades (
    trade_id UUID DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    executed_at TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('buy', 'sell')),
    quantity NUMERIC(20, 8) NOT NULL,
    price NUMERIC(20, 8) NOT NULL,
    total_value NUMERIC(30, 8) NOT NULL,
    fees NUMERIC(20, 8) DEFAULT 0,
    net_value NUMERIC(30, 8) GENERATED ALWAYS AS (
        CASE 
            WHEN side = 'buy' THEN -(total_value + fees)
            ELSE total_value - fees
        END
    ) STORED,
    strategy_name VARCHAR(100) NOT NULL,
    strategy_version VARCHAR(50) NOT NULL,
    prediction_id UUID,
    order_type VARCHAR(20) NOT NULL,
    status VARCHAR(20) NOT NULL,
    slippage NUMERIC(20, 8),
    metadata JSONB,
    PRIMARY KEY (trade_id, executed_at)
) PARTITION BY RANGE (executed_at);

-- Create hypertable for trades
SELECT create_hypertable(
    'trades',
    'executed_at',
    chunk_time_interval => INTERVAL '1 day',
    create_default_indexes => FALSE,
    if_not_exists => TRUE
);

-- Optimized indexes for trades
CREATE INDEX idx_trades_symbol_executed ON trades (symbol, executed_at DESC) WITH (timescaledb.transaction_per_chunk);
CREATE INDEX idx_trades_strategy_executed ON trades (strategy_name, executed_at DESC) WITH (timescaledb.transaction_per_chunk);
CREATE INDEX idx_trades_status ON trades (status) WHERE status != 'filled' WITH (timescaledb.transaction_per_chunk);

-- Create performance metrics table
CREATE TABLE IF NOT EXISTS performance_metrics (
    metric_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    strategy_name VARCHAR(100) NOT NULL,
    strategy_version VARCHAR(50) NOT NULL,
    symbol VARCHAR(20),
    total_trades INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    losing_trades INTEGER NOT NULL,
    total_pnl NUMERIC(30, 8) NOT NULL,
    win_rate NUMERIC(5, 4) NOT NULL,
    sharpe_ratio NUMERIC(10, 4),
    sortino_ratio NUMERIC(10, 4),
    max_drawdown NUMERIC(10, 4),
    max_drawdown_duration INTERVAL,
    calmar_ratio NUMERIC(10, 4),
    avg_win NUMERIC(20, 8),
    avg_loss NUMERIC(20, 8),
    profit_factor NUMERIC(10, 4),
    avg_trade_duration INTERVAL,
    metrics JSONB
);

-- Create neural model performance tracking
CREATE TABLE IF NOT EXISTS neural_model_metrics (
    model_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    model_name VARCHAR(100) NOT NULL,
    model_type VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    training_started_at TIMESTAMPTZ,
    training_completed_at TIMESTAMPTZ,
    training_duration INTERVAL GENERATED ALWAYS AS (training_completed_at - training_started_at) STORED,
    parameters JSONB NOT NULL,
    training_metrics JSONB,
    validation_metrics JSONB,
    test_metrics JSONB,
    feature_importance JSONB,
    model_size_bytes BIGINT,
    inference_time_ms NUMERIC(10, 3),
    status VARCHAR(50) NOT NULL DEFAULT 'created'
);

-- Create materialized views for common aggregations
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_1min_ohlcv
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 minute', time) AS bucket,
    symbol,
    exchange,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    SUM(trades_count) AS trades_count,
    AVG(spread_pct) AS avg_spread_pct
FROM time_series_data
GROUP BY bucket, symbol, exchange
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_5min_ohlcv
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('5 minutes', time) AS bucket,
    symbol,
    exchange,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    SUM(trades_count) AS trades_count,
    AVG(spread_pct) AS avg_spread_pct
FROM time_series_data
GROUP BY bucket, symbol, exchange
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS mv_hourly_ohlcv
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS bucket,
    symbol,
    exchange,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    SUM(trades_count) AS trades_count,
    AVG(spread_pct) AS avg_spread_pct,
    STDDEV(close) AS price_volatility
FROM time_series_data
GROUP BY bucket, symbol, exchange
WITH NO DATA;

-- Create refresh policies
SELECT add_continuous_aggregate_policy('mv_1min_ohlcv',
    start_offset => INTERVAL '10 minutes',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute',
    if_not_exists => TRUE
);

SELECT add_continuous_aggregate_policy('mv_5min_ohlcv',
    start_offset => INTERVAL '30 minutes',
    end_offset => INTERVAL '5 minutes',
    schedule_interval => INTERVAL '5 minutes',
    if_not_exists => TRUE
);

SELECT add_continuous_aggregate_policy('mv_hourly_ohlcv',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Create compression policies
ALTER TABLE time_series_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,exchange',
    timescaledb.compress_orderby = 'time DESC'
);

SELECT add_compression_policy('time_series_data', INTERVAL '7 days', if_not_exists => TRUE);

ALTER TABLE predictions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,model_name',
    timescaledb.compress_orderby = 'created_at DESC'
);

SELECT add_compression_policy('predictions', INTERVAL '30 days', if_not_exists => TRUE);

-- Create data retention policies
SELECT add_retention_policy('time_series_data', INTERVAL '90 days', if_not_exists => TRUE);
SELECT add_retention_policy('predictions', INTERVAL '180 days', if_not_exists => TRUE);
SELECT add_retention_policy('trades', INTERVAL '365 days', if_not_exists => TRUE);

-- Create automated jobs using pg_cron
SELECT cron.schedule(
    'cleanup-old-logs',
    '0 2 * * *',
    $$DELETE FROM pg_stat_statements WHERE query LIKE '%DEBUG%' AND calls < 10$$
);

SELECT cron.schedule(
    'update-performance-metrics',
    '*/15 * * * *',
    $$REFRESH MATERIALIZED VIEW CONCURRENTLY mv_1min_ohlcv$$
);

-- Grant permissions
GRANT ALL PRIVILEGES ON SCHEMA neural_trader TO neural_trader;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA neural_trader TO neural_trader;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA neural_trader TO neural_trader;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA neural_trader TO neural_trader;