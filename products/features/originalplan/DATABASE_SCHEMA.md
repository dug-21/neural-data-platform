# Database Schema Design

## Overview

The Neural Trading Platform uses TimescaleDB (PostgreSQL extension) for time-series data storage and Redis for high-performance caching. This document defines the complete database schema optimized for trading data and neural network operations.

## TimescaleDB Schema

### Core Tables

#### 1. Market Data Tables

**market_data_ticks** - Raw tick data
```sql
CREATE TABLE IF NOT EXISTS market_data_ticks (
    id BIGSERIAL,
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(10) NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    size INTEGER NOT NULL,
    conditions VARCHAR(50),
    trade_id VARCHAR(50),
    source VARCHAR(20) NOT NULL,
    quality_score REAL DEFAULT 1.0,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (timestamp, symbol, exchange, trade_id)
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('market_data_ticks', 'timestamp', 
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Compression policy for older data
ALTER TABLE market_data_ticks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,exchange',
    timescaledb.compress_orderby = 'timestamp'
);

SELECT add_compression_policy('market_data_ticks', INTERVAL '7 days');
```

**market_data_ohlcv** - OHLCV bars
```sql
CREATE TABLE IF NOT EXISTS market_data_ohlcv (
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    timeframe VARCHAR(10) NOT NULL, -- '1m', '5m', '1h', '1d'
    open_price DECIMAL(18, 8) NOT NULL,
    high_price DECIMAL(18, 8) NOT NULL,
    low_price DECIMAL(18, 8) NOT NULL,
    close_price DECIMAL(18, 8) NOT NULL,
    volume BIGINT NOT NULL,
    trade_count INTEGER,
    vwap DECIMAL(18, 8),
    source VARCHAR(20) NOT NULL,
    quality_score REAL DEFAULT 1.0,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (timestamp, symbol, timeframe)
);

SELECT create_hypertable('market_data_ohlcv', 'timestamp', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Compression for OHLCV data
ALTER TABLE market_data_ohlcv SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,timeframe',
    timescaledb.compress_orderby = 'timestamp'
);

SELECT add_compression_policy('market_data_ohlcv', INTERVAL '30 days');
```

**order_book_snapshots** - Level 2 order book data
```sql
CREATE TABLE IF NOT EXISTS order_book_snapshots (
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    exchange VARCHAR(10) NOT NULL,
    bids JSONB NOT NULL, -- Array of [price, size] arrays
    asks JSONB NOT NULL, -- Array of [price, size] arrays
    bid_count INTEGER,
    ask_count INTEGER,
    spread DECIMAL(18, 8),
    mid_price DECIMAL(18, 8),
    source VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (timestamp, symbol, exchange)
);

SELECT create_hypertable('order_book_snapshots', 'timestamp', 
    chunk_time_interval => INTERVAL '6 hours',
    if_not_exists => TRUE
);
```

#### 2. Trading Tables

**positions** - Current and historical positions
```sql
CREATE TABLE IF NOT EXISTS positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(50) NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    quantity DECIMAL(18, 8) NOT NULL,
    entry_price DECIMAL(18, 8) NOT NULL,
    current_price DECIMAL(18, 8),
    market_value DECIMAL(18, 2),
    unrealized_pnl DECIMAL(18, 2),
    side VARCHAR(10) NOT NULL CHECK (side IN ('long', 'short')),
    entry_timestamp TIMESTAMPTZ NOT NULL,
    last_updated TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    status VARCHAR(20) NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'partial')),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_positions_account_symbol ON positions (account_id, symbol);
CREATE INDEX idx_positions_status ON positions (status);
CREATE INDEX idx_positions_entry_timestamp ON positions (entry_timestamp);
```

**orders** - Order history and management
```sql
CREATE TABLE IF NOT EXISTS orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id VARCHAR(50) NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    order_type VARCHAR(20) NOT NULL CHECK (order_type IN ('market', 'limit', 'stop', 'stop_limit')),
    side VARCHAR(10) NOT NULL CHECK (side IN ('buy', 'sell')),
    quantity DECIMAL(18, 8) NOT NULL,
    price DECIMAL(18, 8),
    stop_price DECIMAL(18, 8),
    filled_quantity DECIMAL(18, 8) DEFAULT 0,
    remaining_quantity DECIMAL(18, 8),
    average_fill_price DECIMAL(18, 8),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'open', 'filled', 'partial', 'cancelled', 'rejected')
    ),
    time_in_force VARCHAR(10) DEFAULT 'day' CHECK (time_in_force IN ('day', 'gtc', 'ioc', 'fok')),
    broker_order_id VARCHAR(100),
    client_order_id VARCHAR(100) UNIQUE,
    execution_instructions JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    filled_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ
);

CREATE INDEX idx_orders_account_id ON orders (account_id);
CREATE INDEX idx_orders_symbol ON orders (symbol);
CREATE INDEX idx_orders_status ON orders (status);
CREATE INDEX idx_orders_created_at ON orders (created_at);
CREATE INDEX idx_orders_client_order_id ON orders (client_order_id);
```

**executions** - Trade execution details
```sql
CREATE TABLE IF NOT EXISTS executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES orders(id),
    execution_id VARCHAR(100) UNIQUE NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    side VARCHAR(10) NOT NULL,
    quantity DECIMAL(18, 8) NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    commission DECIMAL(18, 8) DEFAULT 0,
    execution_timestamp TIMESTAMPTZ NOT NULL,
    venue VARCHAR(50),
    liquidity_flag VARCHAR(10), -- 'added', 'removed'
    execution_quality JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_executions_order_id ON executions (order_id);
CREATE INDEX idx_executions_symbol ON executions (symbol);
CREATE INDEX idx_executions_timestamp ON executions (execution_timestamp);
```

#### 3. Neural Network Tables

**neural_models** - Model metadata and configuration
```sql
CREATE TABLE IF NOT EXISTS neural_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    model_type VARCHAR(50) NOT NULL, -- 'NHITS', 'DeepAR', 'TCN', 'MLP'
    version VARCHAR(20) NOT NULL,
    agent_type VARCHAR(50) NOT NULL, -- 'market_analyzer', 'risk_manager', etc.
    configuration JSONB NOT NULL,
    architecture JSONB NOT NULL,
    parameters JSONB NOT NULL,
    training_data_info JSONB,
    performance_metrics JSONB DEFAULT '{}',
    model_file_path VARCHAR(500),
    is_active BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_neural_models_type ON neural_models (model_type);
CREATE INDEX idx_neural_models_agent_type ON neural_models (agent_type);
CREATE INDEX idx_neural_models_active ON neural_models (is_active);
```

**neural_predictions** - Model predictions for analysis
```sql
CREATE TABLE IF NOT EXISTS neural_predictions (
    timestamp TIMESTAMPTZ NOT NULL,
    model_id UUID NOT NULL REFERENCES neural_models(id),
    symbol VARCHAR(20) NOT NULL,
    prediction_type VARCHAR(50) NOT NULL, -- 'price', 'volatility', 'risk', 'allocation'
    input_features JSONB NOT NULL,
    predictions JSONB NOT NULL,
    confidence REAL NOT NULL,
    latency_ms INTEGER,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (timestamp, model_id, symbol, prediction_type)
);

SELECT create_hypertable('neural_predictions', 'timestamp', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Compression for predictions
ALTER TABLE neural_predictions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'model_id,symbol,prediction_type',
    timescaledb.compress_orderby = 'timestamp'
);

SELECT add_compression_policy('neural_predictions', INTERVAL '7 days');
```

**model_training_runs** - Training execution tracking
```sql
CREATE TABLE IF NOT EXISTS model_training_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id UUID NOT NULL REFERENCES neural_models(id),
    run_type VARCHAR(20) NOT NULL, -- 'initial', 'retrain', 'fine_tune'
    training_config JSONB NOT NULL,
    dataset_info JSONB NOT NULL,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'running' CHECK (
        status IN ('running', 'completed', 'failed', 'cancelled')
    ),
    metrics JSONB DEFAULT '{}',
    error_info JSONB,
    model_checkpoints JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_training_runs_model_id ON model_training_runs (model_id);
CREATE INDEX idx_training_runs_status ON model_training_runs (status);
CREATE INDEX idx_training_runs_start_time ON model_training_runs (start_time);
```

#### 4. Agent and DAA Tables

**agents** - DAA agent registration and status
```sql
CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id VARCHAR(100) NOT NULL UNIQUE,
    agent_type VARCHAR(50) NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    capabilities JSONB NOT NULL DEFAULT '[]',
    configuration JSONB NOT NULL DEFAULT '{}',
    status VARCHAR(20) NOT NULL DEFAULT 'inactive' CHECK (
        status IN ('inactive', 'initializing', 'active', 'paused', 'error', 'stopped')
    ),
    health_score REAL DEFAULT 1.0,
    last_heartbeat TIMESTAMPTZ,
    performance_metrics JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_agents_agent_type ON agents (agent_type);
CREATE INDEX idx_agents_status ON agents (status);
CREATE INDEX idx_agents_last_heartbeat ON agents (last_heartbeat);
```

**agent_decisions** - Agent decision history
```sql
CREATE TABLE IF NOT EXISTS agent_decisions (
    timestamp TIMESTAMPTZ NOT NULL,
    agent_id VARCHAR(100) NOT NULL,
    decision_type VARCHAR(50) NOT NULL,
    input_data JSONB NOT NULL,
    decision JSONB NOT NULL,
    confidence REAL NOT NULL,
    reasoning JSONB DEFAULT '[]',
    execution_time_ms INTEGER,
    outcome VARCHAR(20), -- 'pending', 'success', 'failure'
    outcome_metrics JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (timestamp, agent_id, decision_type)
);

SELECT create_hypertable('agent_decisions', 'timestamp', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
```

#### 5. Risk Management Tables

**risk_metrics** - Portfolio and position risk metrics
```sql
CREATE TABLE IF NOT EXISTS risk_metrics (
    timestamp TIMESTAMPTZ NOT NULL,
    account_id VARCHAR(50) NOT NULL,
    metric_type VARCHAR(50) NOT NULL, -- 'var', 'expected_shortfall', 'beta', etc.
    symbol VARCHAR(20), -- NULL for portfolio-level metrics
    value DECIMAL(18, 8) NOT NULL,
    confidence_level REAL, -- For VaR calculations
    time_horizon INTEGER, -- Days
    calculation_method VARCHAR(50),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (timestamp, account_id, metric_type, COALESCE(symbol, ''))
);

SELECT create_hypertable('risk_metrics', 'timestamp', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
```

#### 6. Performance and Analytics Tables

**portfolio_snapshots** - Daily portfolio snapshots
```sql
CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    date DATE NOT NULL,
    account_id VARCHAR(50) NOT NULL,
    total_value DECIMAL(18, 2) NOT NULL,
    cash_balance DECIMAL(18, 2) NOT NULL,
    positions_value DECIMAL(18, 2) NOT NULL,
    unrealized_pnl DECIMAL(18, 2) NOT NULL,
    realized_pnl_daily DECIMAL(18, 2) NOT NULL,
    number_of_positions INTEGER NOT NULL,
    sector_allocation JSONB DEFAULT '{}',
    performance_metrics JSONB DEFAULT '{}',
    risk_metrics JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (date, account_id)
);

CREATE INDEX idx_portfolio_snapshots_account_id ON portfolio_snapshots (account_id);
CREATE INDEX idx_portfolio_snapshots_date ON portfolio_snapshots (date);
```

### Continuous Aggregates (Real-time Views)

**hourly_ohlcv** - Pre-aggregated hourly OHLCV data
```sql
CREATE MATERIALIZED VIEW IF NOT EXISTS hourly_ohlcv
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) as hour,
    symbol,
    FIRST(open_price, timestamp) as open_price,
    MAX(high_price) as high_price,
    MIN(low_price) as low_price,
    LAST(close_price, timestamp) as close_price,
    SUM(volume) as volume,
    COUNT(*) as bar_count,
    AVG(close_price) as avg_price
FROM market_data_ohlcv 
WHERE timeframe = '1m'
GROUP BY hour, symbol
WITH NO DATA;

SELECT add_continuous_aggregate_policy('hourly_ohlcv',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '10 minutes'
);
```

**daily_portfolio_metrics** - Daily aggregated metrics
```sql
CREATE MATERIALIZED VIEW IF NOT EXISTS daily_portfolio_metrics
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', execution_timestamp) as day,
    symbol,
    SUM(CASE WHEN side = 'buy' THEN quantity ELSE -quantity END) as net_quantity,
    SUM(quantity * price) as total_volume,
    COUNT(*) as trade_count,
    AVG(price) as vwap,
    STDDEV(price) as price_volatility
FROM executions
GROUP BY day, symbol
WITH NO DATA;

SELECT add_continuous_aggregate_policy('daily_portfolio_metrics',
    start_offset => INTERVAL '1 day',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);
```

### Indexes for Performance

```sql
-- Market data indexes
CREATE INDEX idx_market_data_ticks_symbol_timestamp ON market_data_ticks (symbol, timestamp DESC);
CREATE INDEX idx_market_data_ohlcv_symbol_timeframe_timestamp ON market_data_ohlcv (symbol, timeframe, timestamp DESC);
CREATE INDEX idx_order_book_snapshots_symbol_timestamp ON order_book_snapshots (symbol, timestamp DESC);

-- Trading indexes
CREATE INDEX idx_positions_unrealized_pnl ON positions (unrealized_pnl) WHERE status = 'open';
CREATE INDEX idx_orders_broker_order_id ON orders (broker_order_id) WHERE broker_order_id IS NOT NULL;
CREATE INDEX idx_executions_venue_timestamp ON executions (venue, execution_timestamp);

-- Neural network indexes
CREATE INDEX idx_neural_predictions_symbol_timestamp ON neural_predictions (symbol, timestamp DESC);
CREATE INDEX idx_neural_predictions_confidence ON neural_predictions (confidence) WHERE confidence > 0.7;

-- Agent indexes
CREATE INDEX idx_agent_decisions_agent_confidence ON agent_decisions (agent_id, confidence DESC);
CREATE INDEX idx_agent_decisions_outcome ON agent_decisions (outcome, timestamp) WHERE outcome = 'success';

-- Risk management indexes
CREATE INDEX idx_risk_metrics_metric_type_timestamp ON risk_metrics (metric_type, timestamp DESC);
CREATE INDEX idx_risk_metrics_account_symbol ON risk_metrics (account_id, symbol, timestamp DESC);
```

### Retention Policies

```sql
-- Keep raw tick data for 30 days, compress after 1 day
SELECT add_retention_policy('market_data_ticks', INTERVAL '30 days');

-- Keep OHLCV data for 2 years, compress after 7 days  
SELECT add_retention_policy('market_data_ohlcv', INTERVAL '2 years');

-- Keep order book snapshots for 7 days
SELECT add_retention_policy('order_book_snapshots', INTERVAL '7 days');

-- Keep neural predictions for 90 days
SELECT add_retention_policy('neural_predictions', INTERVAL '90 days');

-- Keep agent decisions for 180 days
SELECT add_retention_policy('agent_decisions', INTERVAL '180 days');

-- Keep risk metrics for 1 year
SELECT add_retention_policy('risk_metrics', INTERVAL '1 year');
```

## Redis Schema

Redis is used for high-performance caching and real-time data distribution.

### Key Patterns

#### 1. Real-time Market Data
```
market:tick:{symbol} -> JSON (latest tick)
market:quote:{symbol} -> JSON (latest quote)
market:book:{symbol} -> JSON (order book snapshot)
```

#### 2. Agent State Cache
```
agent:state:{agent_id} -> JSON (current agent state)
agent:performance:{agent_id} -> JSON (performance metrics)
agent:heartbeat:{agent_id} -> timestamp
```

#### 3. Neural Model Cache
```
model:prediction:{model_id}:{symbol} -> JSON (latest prediction)
model:metadata:{model_id} -> JSON (model information)
model:performance:{model_id} -> JSON (performance metrics)
```

#### 4. Trading State
```
portfolio:positions:{account_id} -> JSON (current positions)
portfolio:orders:{account_id} -> JSON (active orders)
portfolio:metrics:{account_id} -> JSON (real-time metrics)
```

#### 5. Risk Management Cache
```
risk:limits:{account_id} -> JSON (risk limits)
risk:current:{account_id} -> JSON (current risk metrics)
risk:alerts:{account_id} -> LIST (active alerts)
```

### Redis Data Structures

#### Streams for Real-time Events
```
XADD market_events * symbol AAPL price 150.25 volume 1000
XADD trading_events * type order_filled order_id abc123
XADD agent_events * agent_id analyzer_1 decision buy confidence 0.85
```

#### Sorted Sets for Rankings
```
ZADD price_movers 2.5 AAPL 1.8 GOOGL -0.5 TSLA
ZADD agent_performance 0.95 analyzer_1 0.88 risk_manager_1
```

#### Hash Maps for Structured Data
```
HSET portfolio:summary:account_1 
    total_value 100000 
    cash 25000 
    positions_value 75000 
    unrealized_pnl 2500
```

### Cache Expiration Policies
```
# Real-time data expires quickly
EXPIRE market:tick:AAPL 60          # 1 minute
EXPIRE market:quote:AAPL 30         # 30 seconds

# Agent state expires if not updated
EXPIRE agent:heartbeat:analyzer_1 300  # 5 minutes

# Predictions expire based on time horizon
EXPIRE model:prediction:nhits_1:AAPL 3600  # 1 hour

# Portfolio data expires more slowly
EXPIRE portfolio:metrics:account_1 1800    # 30 minutes
```

## Migration Scripts

### Initial Schema Migration (001_initial_schema.sql)
```sql
-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS timescaledb;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create enum types
CREATE TYPE order_status AS ENUM ('pending', 'open', 'filled', 'partial', 'cancelled', 'rejected');
CREATE TYPE position_side AS ENUM ('long', 'short');
CREATE TYPE agent_status AS ENUM ('inactive', 'initializing', 'active', 'paused', 'error', 'stopped');

-- Set timezone to UTC
SET timezone = 'UTC';

-- Create initial tables (market_data_ticks, market_data_ohlcv, order_book_snapshots)
-- ... (include the table creation SQL from above)
```

### Market Data Migration (002_market_data_tables.sql)
```sql
-- Create market data tables and hypertables
-- ... (include market data table SQL)

-- Create indexes for market data
-- ... (include market data indexes)

-- Setup compression policies
-- ... (include compression policies)
```

### Trading Migration (003_trading_tables.sql)
```sql
-- Create trading-related tables
-- ... (include trading table SQL)

-- Create trading indexes
-- ... (include trading indexes)
```

### Neural Networks Migration (004_neural_tables.sql)
```sql
-- Create neural network and AI tables
-- ... (include neural table SQL)

-- Create neural network indexes
-- ... (include neural indexes)
```

This database schema provides a comprehensive foundation for the neural trading platform with optimized performance for time-series data, efficient querying, and proper data retention policies.