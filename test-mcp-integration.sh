#!/bin/bash
# Quick test script for MCP integration in Codespaces

echo "🚀 Neural Trader MCP Test Script"
echo "================================"

# Export test environment
export DATABASE_URL="postgresql://neural_trader:testpass123@localhost:5432/neural_trader_db"
export REDIS_URL="redis://:testredis123@localhost:6379"

# Create necessary tables
echo "📊 Setting up database..."
PGPASSWORD=testpass123 psql -h localhost -U neural_trader -d neural_trader_db << EOF
-- Create market data table
CREATE TABLE IF NOT EXISTS market_data (
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    open DOUBLE PRECISION NOT NULL,
    high DOUBLE PRECISION NOT NULL,
    low DOUBLE PRECISION NOT NULL,
    close DOUBLE PRECISION NOT NULL,
    volume DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (timestamp, symbol)
);

-- Convert to hypertable
SELECT create_hypertable('market_data', 'timestamp', if_not_exists => TRUE);

-- Insert test data
INSERT INTO market_data (timestamp, symbol, open, high, low, close, volume)
VALUES 
    (NOW() - INTERVAL '5 minutes', 'BTC/USD', 44900, 45100, 44800, 45000, 1000),
    (NOW() - INTERVAL '4 minutes', 'BTC/USD', 45000, 45200, 44950, 45150, 1100),
    (NOW() - INTERVAL '3 minutes', 'BTC/USD', 45150, 45300, 45100, 45250, 1200),
    (NOW() - INTERVAL '2 minutes', 'BTC/USD', 45250, 45400, 45200, 45350, 1300),
    (NOW() - INTERVAL '1 minute', 'BTC/USD', 45350, 45500, 45300, 45450, 1400),
    (NOW(), 'BTC/USD', 45450, 45600, 45400, 45550, 1500)
ON CONFLICT DO NOTHING;

SELECT COUNT(*) as row_count FROM market_data;
EOF

# Add test data to Redis
echo "💾 Adding test data to Redis..."
redis-cli -a testredis123 SET "market:btc:latest" '{"price":45550,"volume":1500,"trend":"bullish"}' EX 300 > /dev/null 2>&1
redis-cli -a testredis123 SET "market:eth:latest" '{"price":3200,"volume":500,"trend":"stable"}' EX 300 > /dev/null 2>&1

echo "✅ Test environment ready!"
echo ""
echo "📋 Next steps:"
echo "1. Build the project: cargo build --release"
echo "2. Run the MCP server: cargo run --bin mcp_server"
echo "3. Test with Claude using the MCP tools!"
echo ""
echo "🧪 Quick test commands:"
echo "   cargo test test_market_data"
echo "   cargo test test_cache_data"
echo "   cargo test test_end_to_end"