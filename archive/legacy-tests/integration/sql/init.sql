-- Database initialization for Config-Store Integration Tests
-- This script sets up the necessary tables and data for testing

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Create configurations table for config-store
CREATE TABLE IF NOT EXISTS configurations (
    id SERIAL PRIMARY KEY,
    key VARCHAR(255) UNIQUE NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    version INTEGER DEFAULT 1,
    encrypted BOOLEAN DEFAULT FALSE,
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Create index for faster key lookups
CREATE INDEX IF NOT EXISTS idx_configurations_key ON configurations (key);
CREATE INDEX IF NOT EXISTS idx_configurations_updated_at ON configurations (updated_at);

-- Create configuration audit log
CREATE TABLE IF NOT EXISTS configuration_audit (
    id SERIAL PRIMARY KEY,
    config_key VARCHAR(255) NOT NULL,
    action VARCHAR(50) NOT NULL, -- SET, GET, DELETE
    old_value JSONB,
    new_value JSONB,
    user_id VARCHAR(100),
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    source VARCHAR(100) DEFAULT 'config-store'
);

-- Create hypertable for configuration audit (time-series data)
SELECT create_hypertable('configuration_audit', 'timestamp', if_not_exists => TRUE);

-- Create market_data table for data ingestion tests
CREATE TABLE IF NOT EXISTS market_data (
    time TIMESTAMP WITH TIME ZONE NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    price DECIMAL(15,6) NOT NULL,
    volume BIGINT,
    source VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create hypertable for market_data
SELECT create_hypertable('market_data', 'time', if_not_exists => TRUE);

-- Create index for symbol-based queries
CREATE INDEX IF NOT EXISTS idx_market_data_symbol_time ON market_data (symbol, time DESC);

-- Create provider_metrics table for testing rate limiting and performance
CREATE TABLE IF NOT EXISTS provider_metrics (
    time TIMESTAMP WITH TIME ZONE NOT NULL,
    provider VARCHAR(50) NOT NULL,
    requests_count INTEGER DEFAULT 0,
    errors_count INTEGER DEFAULT 0,
    avg_response_time_ms INTEGER DEFAULT 0,
    rate_limit_hits INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create hypertable for provider_metrics
SELECT create_hypertable('provider_metrics', 'time', if_not_exists => TRUE);

-- Insert test configuration data
INSERT INTO configurations (key, value, encrypted, metadata) VALUES
-- Provider configurations
('neural_trader_test.providers.polygon.api_key', '"test_polygon_api_key_12345"', true, '{"sensitive": true, "provider": "polygon"}'),
('neural_trader_test.providers.polygon.enabled', 'true', false, '{"provider": "polygon"}'),
('neural_trader_test.providers.polygon.websocket_enabled', 'true', false, '{"provider": "polygon"}'),
('neural_trader_test.providers.polygon.rate_limit', '5', false, '{"provider": "polygon", "unit": "requests_per_minute"}'),

('neural_trader_test.providers.alpaca.api_key', '"test_alpaca_api_key_67890"', true, '{"sensitive": true, "provider": "alpaca"}'),
('neural_trader_test.providers.alpaca.api_secret', '"test_alpaca_secret_abcdef"', true, '{"sensitive": true, "provider": "alpaca"}'),
('neural_trader_test.providers.alpaca.enabled', 'true', false, '{"provider": "alpaca"}'),
('neural_trader_test.providers.alpaca.subscription_level', '"basic"', false, '{"provider": "alpaca"}'),

-- Database configurations
('neural_trader_test.database.timescale.host', '"localhost"', false, '{"component": "database"}'),
('neural_trader_test.database.timescale.port', '5432', false, '{"component": "database"}'),
('neural_trader_test.database.timescale.database', '"neural_trader_test"', false, '{"component": "database"}'),
('neural_trader_test.database.timescale.user', '"postgres"', false, '{"component": "database"}'),
('neural_trader_test.database.timescale.password', '"test_password_123"', true, '{"sensitive": true, "component": "database"}'),
('neural_trader_test.database.timescale.max_connections', '20', false, '{"component": "database"}'),

-- Redis configurations
('neural_trader_test.redis.host', '"redis-test"', false, '{"component": "redis"}'),
('neural_trader_test.redis.port', '6379', false, '{"component": "redis"}'),
('neural_trader_test.redis.password', '"test_redis_pass"', true, '{"sensitive": true, "component": "redis"}'),
('neural_trader_test.redis.max_connections', '50', false, '{"component": "redis"}'),
('neural_trader_test.redis.timeout', '5', false, '{"component": "redis"}'),

-- Rate limiting configurations
('neural_trader_test.rate_limits.global.requests_per_minute', '1000', false, '{"component": "rate_limiter"}'),
('neural_trader_test.rate_limits.global.burst_size', '50', false, '{"component": "rate_limiter"}'),
('neural_trader_test.rate_limits.polygon.requests_per_minute', '5', false, '{"component": "rate_limiter", "provider": "polygon"}'),
('neural_trader_test.rate_limits.alpaca.requests_per_minute', '200', false, '{"component": "rate_limiter", "provider": "alpaca"}'),

-- Data ingestion configurations
('neural_trader_test.data.ingestion.batch_size', '1000', false, '{"component": "data_ingestion"}'),
('neural_trader_test.data.ingestion.interval_ms', '5000', false, '{"component": "data_ingestion"}'),
('neural_trader_test.data.ingestion.max_concurrent', '10', false, '{"component": "data_ingestion"}'),
('neural_trader_test.data.ingestion.timeout_seconds', '30', false, '{"component": "data_ingestion"}'),

-- Feature flags for testing
('neural_trader_test.features.hot_reload_enabled', 'true', false, '{"component": "feature_flags"}'),
('neural_trader_test.features.fallback_enabled', 'true', false, '{"component": "feature_flags"}'),
('neural_trader_test.features.metrics_enabled', 'true', false, '{"component": "feature_flags"}'),
('neural_trader_test.features.audit_logging', 'true', false, '{"component": "feature_flags"}'),

-- Migration settings
('neural_trader_test.migration.phase', '"partial"', false, '{"component": "migration"}'),
('neural_trader_test.migration.rollback_enabled', 'true', false, '{"component": "migration"}'),
('neural_trader_test.migration.validation_enabled', 'true', false, '{"component": "migration"}')

ON CONFLICT (key) DO UPDATE SET
    value = EXCLUDED.value,
    updated_at = NOW(),
    version = configurations.version + 1;

-- Insert test market data
INSERT INTO market_data (time, symbol, price, volume, source) VALUES
(NOW() - INTERVAL '1 hour', 'AAPL', 150.25, 1000000, 'polygon'),
(NOW() - INTERVAL '1 hour', 'GOOGL', 2800.50, 500000, 'polygon'),
(NOW() - INTERVAL '1 hour', 'MSFT', 350.75, 750000, 'alpaca'),
(NOW() - INTERVAL '30 minutes', 'AAPL', 150.50, 1200000, 'polygon'),
(NOW() - INTERVAL '30 minutes', 'GOOGL', 2805.25, 600000, 'polygon'),
(NOW() - INTERVAL '30 minutes', 'MSFT', 351.00, 800000, 'alpaca'),
(NOW() - INTERVAL '15 minutes', 'AAPL', 150.75, 1100000, 'polygon'),
(NOW() - INTERVAL '15 minutes', 'GOOGL', 2810.00, 550000, 'polygon'),
(NOW() - INTERVAL '15 minutes', 'MSFT', 351.25, 850000, 'alpaca');

-- Insert test provider metrics
INSERT INTO provider_metrics (time, provider, requests_count, errors_count, avg_response_time_ms, rate_limit_hits) VALUES
(NOW() - INTERVAL '1 hour', 'polygon', 100, 2, 150, 0),
(NOW() - INTERVAL '1 hour', 'alpaca', 150, 1, 120, 0),
(NOW() - INTERVAL '30 minutes', 'polygon', 95, 1, 140, 1),
(NOW() - INTERVAL '30 minutes', 'alpaca', 160, 0, 110, 0),
(NOW() - INTERVAL '15 minutes', 'polygon', 105, 3, 180, 2),
(NOW() - INTERVAL '15 minutes', 'alpaca', 155, 1, 125, 0);

-- Create functions for configuration management
CREATE OR REPLACE FUNCTION update_configuration_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    NEW.version = OLD.version + 1;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for automatic timestamp updates
DROP TRIGGER IF EXISTS trigger_update_configuration_timestamp ON configurations;
CREATE TRIGGER trigger_update_configuration_timestamp
    BEFORE UPDATE ON configurations
    FOR EACH ROW
    EXECUTE FUNCTION update_configuration_timestamp();

-- Create function for configuration audit logging
CREATE OR REPLACE FUNCTION log_configuration_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO configuration_audit (config_key, action, new_value, timestamp)
        VALUES (NEW.key, 'SET', NEW.value, NOW());
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO configuration_audit (config_key, action, old_value, new_value, timestamp)
        VALUES (NEW.key, 'UPDATE', OLD.value, NEW.value, NOW());
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO configuration_audit (config_key, action, old_value, timestamp)
        VALUES (OLD.key, 'DELETE', OLD.value, NOW());
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for audit logging
DROP TRIGGER IF EXISTS trigger_config_audit_insert ON configurations;
DROP TRIGGER IF EXISTS trigger_config_audit_update ON configurations;
DROP TRIGGER IF EXISTS trigger_config_audit_delete ON configurations;

CREATE TRIGGER trigger_config_audit_insert
    AFTER INSERT ON configurations
    FOR EACH ROW
    EXECUTE FUNCTION log_configuration_change();

CREATE TRIGGER trigger_config_audit_update
    AFTER UPDATE ON configurations
    FOR EACH ROW
    EXECUTE FUNCTION log_configuration_change();

CREATE TRIGGER trigger_config_audit_delete
    AFTER DELETE ON configurations
    FOR EACH ROW
    EXECUTE FUNCTION log_configuration_change();

-- Create views for easier configuration access
CREATE OR REPLACE VIEW provider_configurations AS
SELECT 
    SUBSTRING(key FROM 'neural_trader_test\.providers\.([^.]+)\.') AS provider,
    SUBSTRING(key FROM 'neural_trader_test\.providers\.[^.]+\.(.+)') AS setting,
    value,
    encrypted,
    updated_at
FROM configurations
WHERE key LIKE 'neural_trader_test.providers.%';

CREATE OR REPLACE VIEW rate_limit_configurations AS
SELECT 
    SUBSTRING(key FROM 'neural_trader_test\.rate_limits\.([^.]+)\.') AS scope,
    SUBSTRING(key FROM 'neural_trader_test\.rate_limits\.[^.]+\.(.+)') AS setting,
    value::text::integer AS limit_value,
    updated_at
FROM configurations
WHERE key LIKE 'neural_trader_test.rate_limits.%';

-- Create stored procedures for configuration management
CREATE OR REPLACE FUNCTION get_configuration(config_key VARCHAR(255))
RETURNS JSONB AS $$
DECLARE
    result JSONB;
BEGIN
    SELECT value INTO result FROM configurations WHERE key = config_key;
    RETURN result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION set_configuration(config_key VARCHAR(255), config_value JSONB, is_encrypted BOOLEAN DEFAULT FALSE)
RETURNS VOID AS $$
BEGIN
    INSERT INTO configurations (key, value, encrypted)
    VALUES (config_key, config_value, is_encrypted)
    ON CONFLICT (key) DO UPDATE SET
        value = EXCLUDED.value,
        encrypted = EXCLUDED.encrypted,
        updated_at = NOW(),
        version = configurations.version + 1;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION delete_configuration(config_key VARCHAR(255))
RETURNS BOOLEAN AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM configurations WHERE key = config_key;
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count > 0;
END;
$$ LANGUAGE plpgsql;

-- Grant necessary permissions for test user
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO postgres;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO postgres;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA public TO postgres;

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_config_audit_timestamp ON configuration_audit (timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_config_audit_key ON configuration_audit (config_key);
CREATE INDEX IF NOT EXISTS idx_config_audit_action ON configuration_audit (action);
CREATE INDEX IF NOT EXISTS idx_market_data_time ON market_data (time DESC);
CREATE INDEX IF NOT EXISTS idx_provider_metrics_provider_time ON provider_metrics (provider, time DESC);

-- Insert completion marker
INSERT INTO configurations (key, value, metadata) VALUES 
('neural_trader_test.database.initialized', 'true', '{"initialization_time": "' || NOW()::text || '"}')
ON CONFLICT (key) DO UPDATE SET
    value = EXCLUDED.value,
    updated_at = NOW();

-- Log initialization completion
INSERT INTO configuration_audit (config_key, action, new_value, timestamp, source)
VALUES ('neural_trader_test.database.initialized', 'INIT', '"Database initialization completed"', NOW(), 'init_script');

COMMIT;