-- TimescaleDB schema for neural-trader market data
-- Designed for data normalization and consistency

-- Create market_data table with light constraints
CREATE TABLE IF NOT EXISTS market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    open DECIMAL(10, 4) NOT NULL CHECK (open > 0),
    high DECIMAL(10, 4) NOT NULL CHECK (high > 0),
    low DECIMAL(10, 4) NOT NULL CHECK (low > 0),
    close DECIMAL(10, 4) NOT NULL CHECK (close > 0),
    volume BIGINT NOT NULL CHECK (volume >= 0),
    provider VARCHAR(50) NOT NULL,
    metadata JSONB,
    -- Ensure OHLC consistency
    CONSTRAINT check_high_low CHECK (high >= low),
    CONSTRAINT check_ohlc_range CHECK (
        high >= open AND high >= close AND
        low <= open AND low <= close
    ),
    -- Composite primary key to prevent duplicates
    PRIMARY KEY (time, symbol, provider)
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('market_data', 'time', if_not_exists => TRUE);

-- Create index for symbol queries
CREATE INDEX IF NOT EXISTS idx_market_data_symbol 
ON market_data (symbol, time DESC);

-- Create index for provider queries
CREATE INDEX IF NOT EXISTS idx_market_data_provider 
ON market_data (provider, time DESC);

-- Create tick_data table for trade-level data
CREATE TABLE IF NOT EXISTS tick_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    price DECIMAL(10, 4) NOT NULL CHECK (price > 0),
    size BIGINT NOT NULL CHECK (size > 0),
    exchange VARCHAR(10),
    conditions TEXT,
    provider VARCHAR(50) NOT NULL,
    PRIMARY KEY (time, symbol, provider)
);

-- Convert to hypertable
SELECT create_hypertable('tick_data', 'time', if_not_exists => TRUE);

-- Create order_book table
CREATE TABLE IF NOT EXISTS order_book (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    bid_price DECIMAL(10, 4) NOT NULL CHECK (bid_price > 0),
    bid_size BIGINT NOT NULL CHECK (bid_size >= 0),
    ask_price DECIMAL(10, 4) NOT NULL CHECK (ask_price > 0),
    ask_size BIGINT NOT NULL CHECK (ask_size >= 0),
    mid_price DECIMAL(10, 4) NOT NULL CHECK (mid_price > 0),
    spread DECIMAL(10, 4) NOT NULL CHECK (spread >= 0),
    provider VARCHAR(50) NOT NULL,
    -- Ensure bid < ask
    CONSTRAINT check_bid_ask CHECK (bid_price < ask_price),
    PRIMARY KEY (time, symbol, provider)
);

-- Convert to hypertable
SELECT create_hypertable('order_book', 'time', if_not_exists => TRUE);

-- Create continuous aggregate for 1-hour bars
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_1h
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    symbol,
    provider,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume
FROM market_data
GROUP BY bucket, symbol, provider;

-- Create continuous aggregate for daily bars
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data_1d
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', time) AS bucket,
    symbol,
    provider,
    FIRST(open, time) AS open,
    MAX(high) AS high,
    MIN(low) AS low,
    LAST(close, time) AS close,
    SUM(volume) AS volume
FROM market_data
GROUP BY bucket, symbol, provider;

-- Add refresh policies for continuous aggregates
SELECT add_continuous_aggregate_policy('market_data_1h',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);

SELECT add_continuous_aggregate_policy('market_data_1d',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE);

-- Add retention policy (optional, adjust as needed)
-- Keep raw data for 1 year, aggregated data forever
SELECT add_retention_policy('market_data', INTERVAL '1 year', if_not_exists => TRUE);
SELECT add_retention_policy('tick_data', INTERVAL '3 months', if_not_exists => TRUE);
SELECT add_retention_policy('order_book', INTERVAL '1 month', if_not_exists => TRUE);