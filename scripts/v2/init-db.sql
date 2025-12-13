-- Neural Trader V2 Database Initialization Script
-- TimescaleDB setup for time-series data

-- Create extensions
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
CREATE EXTENSION IF NOT EXISTS uuid-ossp;

-- Create schemas
CREATE SCHEMA IF NOT EXISTS market_data;
CREATE SCHEMA IF NOT EXISTS trading;
CREATE SCHEMA IF NOT EXISTS ml_ops;
CREATE SCHEMA IF NOT EXISTS config;
CREATE SCHEMA IF NOT EXISTS audit;

-- =============================================================================
-- CONFIG SCHEMA - Configuration Management
-- =============================================================================

CREATE TABLE IF NOT EXISTS config.service_configs (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    service_name VARCHAR(100) NOT NULL,
    environment VARCHAR(50) NOT NULL,
    config_version VARCHAR(50) NOT NULL,
    config_data JSONB NOT NULL,
    schema_version VARCHAR(20),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(service_name, environment, config_version)
);

CREATE TABLE IF NOT EXISTS config.feature_flags (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    flag_name VARCHAR(100) NOT NULL UNIQUE,
    is_enabled BOOLEAN DEFAULT false,
    description TEXT,
    config JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- =============================================================================
-- MARKET DATA SCHEMA - Time-Series Data
-- =============================================================================

CREATE TABLE IF NOT EXISTS market_data.candles (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    open DECIMAL(10, 2) NOT NULL,
    high DECIMAL(10, 2) NOT NULL,
    low DECIMAL(10, 2) NOT NULL,
    close DECIMAL(10, 2) NOT NULL,
    volume BIGINT NOT NULL,
    vwap DECIMAL(10, 4),
    trades INTEGER,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert to TimescaleDB hypertable
SELECT create_hypertable('market_data.candles', 'time', 
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_candles_symbol_time 
    ON market_data.candles (symbol, time DESC);

CREATE TABLE IF NOT EXISTS market_data.ticks (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    price DECIMAL(10, 4) NOT NULL,
    size INTEGER NOT NULL,
    conditions VARCHAR(10),
    exchange VARCHAR(10),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert to TimescaleDB hypertable
SELECT create_hypertable('market_data.ticks', 'time',
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_ticks_symbol_time 
    ON market_data.ticks (symbol, time DESC);

-- =============================================================================
-- ML OPS SCHEMA - Machine Learning Operations
-- =============================================================================

CREATE TABLE IF NOT EXISTS ml_ops.models (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    model_name VARCHAR(200) NOT NULL,
    model_version VARCHAR(50) NOT NULL,
    model_type VARCHAR(100) NOT NULL,
    parameters JSONB,
    metrics JSONB,
    file_path TEXT,
    is_active BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(model_name, model_version)
);

CREATE TABLE IF NOT EXISTS ml_ops.predictions (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    model_id UUID REFERENCES ml_ops.models(id),
    prediction_time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    prediction_type VARCHAR(50) NOT NULL,
    prediction_value DECIMAL(10, 4),
    confidence DECIMAL(5, 4),
    features JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create index for time-based queries
CREATE INDEX IF NOT EXISTS idx_predictions_time 
    ON ml_ops.predictions (prediction_time DESC);

CREATE INDEX IF NOT EXISTS idx_predictions_symbol 
    ON ml_ops.predictions (symbol, prediction_time DESC);

-- =============================================================================
-- TRADING SCHEMA - Trading Operations
-- =============================================================================

CREATE TABLE IF NOT EXISTS trading.signals (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    signal_time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    signal_type VARCHAR(20) NOT NULL CHECK (signal_type IN ('BUY', 'SELL', 'HOLD')),
    strength DECIMAL(5, 4),
    strategy_name VARCHAR(100),
    parameters JSONB,
    model_id UUID REFERENCES ml_ops.models(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_signals_time 
    ON trading.signals (signal_time DESC);

CREATE INDEX IF NOT EXISTS idx_signals_symbol 
    ON trading.signals (symbol, signal_time DESC);

CREATE TABLE IF NOT EXISTS trading.orders (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    order_time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    order_type VARCHAR(20) NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('BUY', 'SELL')),
    quantity INTEGER NOT NULL,
    price DECIMAL(10, 2),
    status VARCHAR(20) NOT NULL,
    signal_id UUID REFERENCES trading.signals(id),
    filled_quantity INTEGER DEFAULT 0,
    average_price DECIMAL(10, 2),
    commission DECIMAL(10, 2),
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_orders_time 
    ON trading.orders (order_time DESC);

CREATE INDEX IF NOT EXISTS idx_orders_symbol_status 
    ON trading.orders (symbol, status);

CREATE TABLE IF NOT EXISTS trading.positions (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    symbol VARCHAR(20) NOT NULL,
    quantity INTEGER NOT NULL,
    entry_price DECIMAL(10, 2) NOT NULL,
    current_price DECIMAL(10, 2),
    unrealized_pnl DECIMAL(10, 2),
    realized_pnl DECIMAL(10, 2),
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_positions_symbol 
    ON trading.positions (symbol, status);

-- =============================================================================
-- AUDIT SCHEMA - System Audit Trail
-- =============================================================================

CREATE TABLE IF NOT EXISTS audit.events (
    id UUID DEFAULT uuid_generate_v4() PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_type VARCHAR(100) NOT NULL,
    service_name VARCHAR(100) NOT NULL,
    user_id VARCHAR(100),
    action VARCHAR(200) NOT NULL,
    entity_type VARCHAR(100),
    entity_id VARCHAR(100),
    old_value JSONB,
    new_value JSONB,
    metadata JSONB,
    ip_address INET,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert to TimescaleDB hypertable
SELECT create_hypertable('audit.events', 'event_time',
    chunk_time_interval => INTERVAL '1 week',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_audit_service_time 
    ON audit.events (service_name, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_audit_entity 
    ON audit.events (entity_type, entity_id);

-- =============================================================================
-- PERFORMANCE METRICS TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS ml_ops.performance_metrics (
    time TIMESTAMPTZ NOT NULL,
    service_name VARCHAR(100) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DECIMAL(10, 4) NOT NULL,
    unit VARCHAR(20),
    tags JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert to TimescaleDB hypertable
SELECT create_hypertable('ml_ops.performance_metrics', 'time',
    chunk_time_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_metrics_service_time 
    ON ml_ops.performance_metrics (service_name, time DESC);

-- =============================================================================
-- VIEWS
-- =============================================================================

-- Active configurations view
CREATE OR REPLACE VIEW config.active_configs AS
SELECT 
    service_name,
    environment,
    config_version,
    config_data,
    updated_at
FROM config.service_configs
WHERE is_active = true;

-- Recent signals view
CREATE OR REPLACE VIEW trading.recent_signals AS
SELECT 
    s.*,
    m.model_name,
    m.model_version
FROM trading.signals s
LEFT JOIN ml_ops.models m ON s.model_id = m.id
WHERE s.signal_time > NOW() - INTERVAL '24 hours'
ORDER BY s.signal_time DESC;

-- Position summary view
CREATE OR REPLACE VIEW trading.position_summary AS
SELECT 
    symbol,
    SUM(CASE WHEN status = 'OPEN' THEN quantity ELSE 0 END) as open_quantity,
    AVG(CASE WHEN status = 'OPEN' THEN entry_price ELSE NULL END) as avg_entry_price,
    SUM(realized_pnl) as total_realized_pnl,
    SUM(CASE WHEN status = 'OPEN' THEN unrealized_pnl ELSE 0 END) as total_unrealized_pnl
FROM trading.positions
GROUP BY symbol;

-- =============================================================================
-- FUNCTIONS
-- =============================================================================

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply update trigger to relevant tables
CREATE TRIGGER update_service_configs_updated_at 
    BEFORE UPDATE ON config.service_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_models_updated_at 
    BEFORE UPDATE ON ml_ops.models
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_orders_updated_at 
    BEFORE UPDATE ON trading.orders
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_positions_updated_at 
    BEFORE UPDATE ON trading.positions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- =============================================================================
-- DATA RETENTION POLICIES (TimescaleDB)
-- =============================================================================

-- Add retention policies for time-series data
SELECT add_retention_policy('market_data.candles', INTERVAL '90 days', if_not_exists => TRUE);
SELECT add_retention_policy('market_data.ticks', INTERVAL '30 days', if_not_exists => TRUE);
SELECT add_retention_policy('audit.events', INTERVAL '180 days', if_not_exists => TRUE);
SELECT add_retention_policy('ml_ops.performance_metrics', INTERVAL '30 days', if_not_exists => TRUE);

-- =============================================================================
-- CONTINUOUS AGGREGATES (TimescaleDB)
-- =============================================================================

-- Create continuous aggregate for hourly candles
CREATE MATERIALIZED VIEW IF NOT EXISTS market_data.candles_hourly
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) as hour,
    symbol,
    FIRST(open, time) as open,
    MAX(high) as high,
    MIN(low) as low,
    LAST(close, time) as close,
    SUM(volume) as volume,
    AVG(vwap) as vwap
FROM market_data.candles
GROUP BY hour, symbol
WITH NO DATA;

-- Add refresh policy for continuous aggregate
SELECT add_continuous_aggregate_policy('market_data.candles_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- =============================================================================
-- INITIAL DATA
-- =============================================================================

-- Insert default feature flags
INSERT INTO config.feature_flags (flag_name, is_enabled, description) VALUES
('enable_ml_trading', false, 'Enable ML-based trading signals'),
('enable_paper_trading', true, 'Enable paper trading mode'),
('enable_risk_management', true, 'Enable risk management controls'),
('enable_real_time_data', false, 'Enable real-time market data ingestion')
ON CONFLICT (flag_name) DO NOTHING;

-- Insert default configuration
INSERT INTO config.service_configs (
    service_name, 
    environment, 
    config_version, 
    config_data,
    schema_version
) VALUES 
(
    'global',
    'dev',
    'v1.0.0',
    '{
        "log_level": "debug",
        "max_connections": 100,
        "timeout_seconds": 30,
        "retry_attempts": 3
    }'::jsonb,
    '1.0'
)
ON CONFLICT (service_name, environment, config_version) DO NOTHING;

-- Grant permissions
GRANT USAGE ON SCHEMA market_data, trading, ml_ops, config, audit TO PUBLIC;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA market_data, trading, ml_ops, config, audit TO PUBLIC;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA market_data, trading, ml_ops, config, audit TO PUBLIC;

-- Output success message
DO $$
BEGIN
    RAISE NOTICE 'Neural Trader V2 database initialization complete!';
    RAISE NOTICE 'Schemas created: market_data, trading, ml_ops, config, audit';
    RAISE NOTICE 'TimescaleDB hypertables created for time-series data';
    RAISE NOTICE 'Retention policies and continuous aggregates configured';
END $$;