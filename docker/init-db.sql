-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create schema for neural trader
CREATE SCHEMA IF NOT EXISTS neural_trader;

-- Set search path
SET search_path TO neural_trader, public;

-- Create time_series_data table for market data
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
    PRIMARY KEY (time, symbol, exchange)
);

-- Create hypertable for time_series_data
SELECT create_hypertable(
    'time_series_data',
    'time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Create index on symbol for faster queries
CREATE INDEX IF NOT EXISTS idx_time_series_symbol 
    ON time_series_data (symbol, time DESC);

-- Create index on exchange
CREATE INDEX IF NOT EXISTS idx_time_series_exchange 
    ON time_series_data (exchange, time DESC);

-- Create predictions table
CREATE TABLE IF NOT EXISTS predictions (
    prediction_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    model_name VARCHAR(100) NOT NULL,
    model_version VARCHAR(50) NOT NULL,
    prediction_time TIMESTAMPTZ NOT NULL,
    prediction_type VARCHAR(50) NOT NULL, -- 'price', 'direction', 'volatility', etc.
    prediction_horizon INTEGER NOT NULL, -- minutes into future
    predicted_value JSONB NOT NULL, -- flexible structure for different prediction types
    confidence NUMERIC(5, 4) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    features_used JSONB,
    model_metadata JSONB,
    actual_value JSONB, -- populated after the fact for backtesting
    error_metrics JSONB -- populated after the fact
);

-- Create hypertable for predictions
SELECT create_hypertable(
    'predictions',
    'created_at',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Create indexes for predictions
CREATE INDEX IF NOT EXISTS idx_predictions_symbol 
    ON predictions (symbol, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_predictions_model 
    ON predictions (model_name, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_predictions_type 
    ON predictions (prediction_type, created_at DESC);

-- Create trades table for tracking executed trades
CREATE TABLE IF NOT EXISTS trades (
    trade_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    executed_at TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('buy', 'sell')),
    quantity NUMERIC(20, 8) NOT NULL,
    price NUMERIC(20, 8) NOT NULL,
    total_value NUMERIC(30, 8) NOT NULL,
    fees NUMERIC(20, 8) DEFAULT 0,
    strategy_name VARCHAR(100) NOT NULL,
    strategy_version VARCHAR(50) NOT NULL,
    prediction_id UUID REFERENCES predictions(prediction_id),
    order_type VARCHAR(20) NOT NULL, -- 'market', 'limit', 'stop', etc.
    status VARCHAR(20) NOT NULL, -- 'pending', 'filled', 'cancelled', 'failed'
    metadata JSONB
);

-- Create hypertable for trades
SELECT create_hypertable(
    'trades',
    'executed_at',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Create indexes for trades
CREATE INDEX IF NOT EXISTS idx_trades_symbol 
    ON trades (symbol, executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_strategy 
    ON trades (strategy_name, executed_at DESC);

-- Create performance_metrics table
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
    max_drawdown NUMERIC(10, 4),
    avg_trade_duration INTERVAL,
    metrics JSONB -- Additional flexible metrics
);

-- Create continuous aggregates for common queries
CREATE MATERIALIZED VIEW IF NOT EXISTS hourly_ohlcv
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS hour,
    symbol,
    exchange,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume,
    SUM(trades_count) AS trades_count
FROM time_series_data
GROUP BY hour, symbol, exchange
WITH NO DATA;

-- Create refresh policy for continuous aggregate
SELECT add_continuous_aggregate_policy('hourly_ohlcv',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Create data retention policies (keep raw data for 90 days)
SELECT add_retention_policy('time_series_data', INTERVAL '90 days', if_not_exists => TRUE);
SELECT add_retention_policy('predictions', INTERVAL '180 days', if_not_exists => TRUE);
SELECT add_retention_policy('trades', INTERVAL '365 days', if_not_exists => TRUE);

-- Grant permissions
GRANT ALL PRIVILEGES ON SCHEMA neural_trader TO neural_trader;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA neural_trader TO neural_trader;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA neural_trader TO neural_trader;