-- Test data insertion script
-- This creates initial test data for basic functionality testing

-- Insert test symbols and basic market data
INSERT INTO market_data (time, symbol, price, volume, open_price, high_price, low_price, close_price, provider)
VALUES
    -- AAPL test data (last hour)
    (NOW() - INTERVAL '60 minutes', 'AAPL', 150.25, 1000000, 150.00, 150.50, 149.80, 150.25, 'test'),
    (NOW() - INTERVAL '50 minutes', 'AAPL', 150.30, 1100000, 150.25, 150.60, 150.10, 150.30, 'test'),
    (NOW() - INTERVAL '40 minutes', 'AAPL', 150.45, 950000, 150.30, 150.70, 150.20, 150.45, 'test'),
    (NOW() - INTERVAL '30 minutes', 'AAPL', 150.60, 1200000, 150.45, 150.80, 150.35, 150.60, 'test'),
    (NOW() - INTERVAL '20 minutes', 'AAPL', 150.55, 1050000, 150.60, 150.75, 150.40, 150.55, 'test'),
    (NOW() - INTERVAL '10 minutes', 'AAPL', 150.70, 1300000, 150.55, 150.85, 150.50, 150.70, 'test'),
    
    -- MSFT test data
    (NOW() - INTERVAL '60 minutes', 'MSFT', 280.50, 800000, 280.00, 281.00, 279.75, 280.50, 'test'),
    (NOW() - INTERVAL '50 minutes', 'MSFT', 280.75, 850000, 280.50, 281.20, 280.30, 280.75, 'test'),
    (NOW() - INTERVAL '40 minutes', 'MSFT', 281.00, 900000, 280.75, 281.40, 280.60, 281.00, 'test'),
    (NOW() - INTERVAL '30 minutes', 'MSFT', 281.25, 950000, 281.00, 281.60, 280.80, 281.25, 'test'),
    (NOW() - INTERVAL '20 minutes', 'MSFT', 281.10, 870000, 281.25, 281.50, 280.90, 281.10, 'test'),
    (NOW() - INTERVAL '10 minutes', 'MSFT', 281.40, 920000, 281.10, 281.70, 281.00, 281.40, 'test'),
    
    -- GOOGL test data
    (NOW() - INTERVAL '60 minutes', 'GOOGL', 2150.25, 500000, 2150.00, 2155.00, 2145.00, 2150.25, 'test'),
    (NOW() - INTERVAL '50 minutes', 'GOOGL', 2152.50, 520000, 2150.25, 2157.00, 2148.00, 2152.50, 'test'),
    (NOW() - INTERVAL '40 minutes', 'GOOGL', 2155.00, 480000, 2152.50, 2160.00, 2151.00, 2155.00, 'test'),
    (NOW() - INTERVAL '30 minutes', 'GOOGL', 2157.75, 540000, 2155.00, 2162.00, 2153.00, 2157.75, 'test'),
    (NOW() - INTERVAL '20 minutes', 'GOOGL', 2156.50, 510000, 2157.75, 2160.00, 2154.00, 2156.50, 'test'),
    (NOW() - INTERVAL '10 minutes', 'GOOGL', 2159.25, 560000, 2156.50, 2163.00, 2155.00, 2159.25, 'test')
ON CONFLICT (time, symbol) DO NOTHING;

-- Insert test features
INSERT INTO features (symbol, time, feature_name, feature_value, feature_type, calculation_method)
VALUES
    -- Moving averages for AAPL
    ('AAPL', NOW() - INTERVAL '10 minutes', 'sma_5', 150.51, 'technical', 'simple_moving_average'),
    ('AAPL', NOW() - INTERVAL '10 minutes', 'sma_10', 150.45, 'technical', 'simple_moving_average'),
    ('AAPL', NOW() - INTERVAL '10 minutes', 'ema_5', 150.58, 'technical', 'exponential_moving_average'),
    ('AAPL', NOW() - INTERVAL '10 minutes', 'rsi_14', 65.5, 'momentum', 'relative_strength_index'),
    ('AAPL', NOW() - INTERVAL '10 minutes', 'volume_sma_10', 1100000, 'volume', 'simple_moving_average'),
    
    -- Features for MSFT
    ('MSFT', NOW() - INTERVAL '10 minutes', 'sma_5', 280.93, 'technical', 'simple_moving_average'),
    ('MSFT', NOW() - INTERVAL '10 minutes', 'sma_10', 280.85, 'technical', 'simple_moving_average'),
    ('MSFT', NOW() - INTERVAL '10 minutes', 'ema_5', 281.02, 'technical', 'exponential_moving_average'),
    ('MSFT', NOW() - INTERVAL '10 minutes', 'rsi_14', 58.2, 'momentum', 'relative_strength_index'),
    ('MSFT', NOW() - INTERVAL '10 minutes', 'volume_sma_10', 875000, 'volume', 'simple_moving_average'),
    
    -- Features for GOOGL
    ('GOOGL', NOW() - INTERVAL '10 minutes', 'sma_5', 2154.38, 'technical', 'simple_moving_average'),
    ('GOOGL', NOW() - INTERVAL '10 minutes', 'sma_10', 2152.85, 'technical', 'simple_moving_average'),
    ('GOOGL', NOW() - INTERVAL '10 minutes', 'ema_5', 2156.12, 'technical', 'exponential_moving_average'),
    ('GOOGL', NOW() - INTERVAL '10 minutes', 'rsi_14', 62.8, 'momentum', 'relative_strength_index'),
    ('GOOGL', NOW() - INTERVAL '10 minutes', 'volume_sma_10', 520000, 'volume', 'simple_moving_average');

-- Insert test predictions
INSERT INTO predictions (model_name, symbol, prediction_time, target_time, predicted_price, confidence_score, model_version)
VALUES
    ('NHITS', 'AAPL', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 150.85, 0.75, 'v1.0'),
    ('TCN', 'AAPL', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 150.80, 0.72, 'v1.0'),
    ('DeepAR', 'AAPL', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 150.90, 0.78, 'v1.0'),
    
    ('NHITS', 'MSFT', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 281.60, 0.73, 'v1.0'),
    ('TCN', 'MSFT', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 281.55, 0.71, 'v1.0'),
    ('DeepAR', 'MSFT', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 281.65, 0.76, 'v1.0'),
    
    ('NHITS', 'GOOGL', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 2161.50, 0.74, 'v1.0'),
    ('TCN', 'GOOGL', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 2160.80, 0.70, 'v1.0'),
    ('DeepAR', 'GOOGL', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '5 minutes', 2162.20, 0.77, 'v1.0');

-- Insert test sentiment data
INSERT INTO sentiment_data (time, symbol, source, sentiment_score, confidence, text_content)
VALUES
    (NOW() - INTERVAL '30 minutes', 'AAPL', 'twitter', 0.65, 0.8, 'Positive sentiment about Apple earnings'),
    (NOW() - INTERVAL '25 minutes', 'AAPL', 'reddit', 0.45, 0.75, 'Mixed sentiment on Apple stock discussion'),
    (NOW() - INTERVAL '20 minutes', 'AAPL', 'news', 0.75, 0.9, 'Apple announces new product launch'),
    
    (NOW() - INTERVAL '30 minutes', 'MSFT', 'twitter', 0.55, 0.82, 'Microsoft cloud growth positive'),
    (NOW() - INTERVAL '25 minutes', 'MSFT', 'reddit', 0.60, 0.77, 'Good discussion about Azure performance'),
    (NOW() - INTERVAL '20 minutes', 'MSFT', 'news', 0.70, 0.88, 'Microsoft beats quarterly expectations'),
    
    (NOW() - INTERVAL '30 minutes', 'GOOGL', 'twitter', 0.40, 0.78, 'Concerns about Google AI competition'),
    (NOW() - INTERVAL '25 minutes', 'GOOGL', 'reddit', 0.50, 0.73, 'Neutral discussion on Google stock'),
    (NOW() - INTERVAL '20 minutes', 'GOOGL', 'news', 0.85, 0.92, 'Google announces breakthrough in quantum computing');

-- Insert test orders
INSERT INTO orders (symbol, order_type, quantity, price, status, order_time, execution_time, provider)
VALUES
    ('AAPL', 'BUY', 100, 150.25, 'EXECUTED', NOW() - INTERVAL '45 minutes', NOW() - INTERVAL '44 minutes', 'test'),
    ('AAPL', 'SELL', 50, 150.70, 'EXECUTED', NOW() - INTERVAL '15 minutes', NOW() - INTERVAL '14 minutes', 'test'),
    ('MSFT', 'BUY', 75, 280.50, 'EXECUTED', NOW() - INTERVAL '40 minutes', NOW() - INTERVAL '39 minutes', 'test'),
    ('GOOGL', 'BUY', 25, 2150.00, 'PENDING', NOW() - INTERVAL '5 minutes', NULL, 'test');

-- Insert test performance metrics
INSERT INTO performance_metrics (time, metric_name, metric_value, tags)
VALUES
    (NOW() - INTERVAL '30 minutes', 'model_accuracy', 0.85, '{"model": "ensemble", "symbol": "AAPL"}'),
    (NOW() - INTERVAL '25 minutes', 'prediction_latency_ms', 125.5, '{"model": "NHITS", "symbol": "AAPL"}'),
    (NOW() - INTERVAL '20 minutes', 'data_ingestion_rate', 1500, '{"provider": "test", "type": "market_data"}'),
    (NOW() - INTERVAL '15 minutes', 'memory_usage_mb', 2048, '{"service": "neural_trader", "instance": "test"}'),
    (NOW() - INTERVAL '10 minutes', 'cpu_usage_percent', 45.2, '{"service": "data_ingestion", "instance": "test"}'),
    (NOW() - INTERVAL '5 minutes', 'disk_usage_gb', 15.7, '{"service": "timescaledb", "instance": "test"}');

-- Insert a test run record
INSERT INTO test_runs (test_name, test_type, status, start_time, end_time, duration_seconds, test_data, results)
VALUES
    ('initial_data_setup', 'data_setup', 'COMPLETED', NOW() - INTERVAL '1 minute', NOW(), 60, 
     '{"symbols": ["AAPL", "MSFT", "GOOGL"], "data_points": 18}',
     '{"rows_inserted": 18, "tables_created": 7, "indexes_created": 7}');

-- Refresh the continuous aggregate to include our test data
CALL refresh_continuous_aggregate('market_data_1min', NOW() - INTERVAL '2 hours', NOW());