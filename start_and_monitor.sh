#!/bin/bash

# Start Neural Trader and monitor its activity

echo "🚀 Starting Neural Trader with monitoring..."
echo "==========================================="

# Set up environment
export DATABASE_URL="postgresql://neural_trader:${POSTGRES_PASSWORD:-password}@localhost:5432/neural_trader_db"
export REDIS_URL="redis://:${REDIS_PASSWORD:-password}@localhost:6379"
export RUST_LOG="info,neural_trader=debug"
export LOG_LEVEL="debug"

# Load trading configuration from .env.stock-simulation
if [ -f .env.stock-simulation ]; then
    set -a
    source .env.stock-simulation
    set +a
fi

echo "📊 Configuration:"
echo "  Database: $DATABASE_URL"
echo "  Redis: $REDIS_URL"
echo "  Trading Mode: ${TRADING_MODE:-paper}"
echo "  Primary Provider: ${PRIMARY_PROVIDER:-finnhub}"
echo "  API Keys Available: ${FINNHUB_API_KEY:+✓} ${ALPHA_VANTAGE_API_KEY:+✓} ${IEX_CLOUD_API_KEY:+✓}"
echo ""

# Create a log directory
mkdir -p logs

# Start the application in the background with logging
echo "Starting Neural Trader..."
nohup ./target/release/neural-trader > logs/neural-trader.log 2>&1 &
APP_PID=$!

echo "✅ Neural Trader started with PID: $APP_PID"
echo ""

# Give it a moment to start
sleep 5

# Check if it's still running
if ps -p $APP_PID > /dev/null; then
    echo "✅ Application is running!"
    echo ""
    echo "📝 Monitoring options:"
    echo ""
    echo "1. View live logs:"
    echo "   tail -f logs/neural-trader.log"
    echo ""
    echo "2. Check API health:"
    echo "   curl http://localhost:3030/health"
    echo ""
    echo "3. View database activity:"
    echo "   docker exec -it neural_trader_stocks-timescaledb-1 psql -U neural_trader -c 'SELECT COUNT(*) FROM market_data;'"
    echo ""
    echo "4. Monitor Redis activity:"
    echo "   docker exec -it neural_trader_stocks-redis-1 redis-cli -a \${REDIS_PASSWORD} MONITOR"
    echo ""
    echo "5. Check metrics:"
    echo "   curl http://localhost:3031/metrics"
    echo ""
    echo "6. Stop the application:"
    echo "   kill $APP_PID"
    echo ""
    
    # Show initial log output
    echo "📋 Initial log output:"
    echo "====================="
    tail -20 logs/neural-trader.log
else
    echo "❌ Application failed to start!"
    echo "Check logs/neural-trader.log for errors"
    tail -20 logs/neural-trader.log
fi