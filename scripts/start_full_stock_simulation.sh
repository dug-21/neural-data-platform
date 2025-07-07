#!/bin/bash

# Neural Trader Full Stack Stock Trading Simulation
# Runs complete Docker stack with all services for stock trading

set -e

echo "📈 Neural Trader Stock Trading Simulation Startup"
echo "================================================"

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Check for at least one API key in environment
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_VANTAGE_API_KEY" ] && [ -z "$IEX_CLOUD_API_KEY" ] && [ -z "$POLYGON_API_KEY" ]; then
    echo ""
    echo "⚠️  WARNING: No API key found in environment variables!"
    echo ""
    echo "You need to set at least one API key in your environment:"
    echo ""
    echo "  export FINNHUB_API_KEY='your_actual_key'"
    echo "  or"
    echo "  export ALPHA_VANTAGE_API_KEY='your_actual_key'"
    echo "  or"
    echo "  export IEX_CLOUD_API_KEY='your_actual_key'"
    echo ""
    echo "Get free API keys from:"
    echo "  - Finnhub: https://finnhub.io/register (BEST - 60 calls/min)"
    echo "  - Alpha Vantage: https://www.alphavantage.co/support/#api-key"
    echo "  - IEX Cloud: https://iexcloud.io/console/tokens"
    echo ""
    echo ""
    echo "Run ./scripts/setup_stock_env.sh to set up your environment"
    exit 1
fi

# Export environment variables
echo "📋 Loading stock trading configuration..."
set -a
source .env.stock-simulation
set +a

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
docker-compose down 2>/dev/null || true

# Build images if needed
echo "🔨 Building Docker images..."
docker-compose build

# Start the full stack
echo "🚀 Starting full trading stack..."
echo "  - TimescaleDB (time-series database)"
echo "  - Redis (real-time cache)"
echo "  - Data Ingestion Service"
echo "  - Neural Trader Application"
echo "  - Prometheus (metrics)"
echo "  - Grafana (visualization)"
echo ""

docker-compose up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 20

# Check service health
echo "🏥 Checking service health..."
docker-compose ps

# Show connection info
echo ""
echo "✅ Stock Trading Simulation Started!"
echo "===================================="
echo ""
echo "📊 Trading Configuration:"
echo "  - Mode: Paper Trading (Simulation)"
echo "  - Initial Capital: \$10,000"
echo "  - Asset Class: US Stocks"
echo "  - Primary Symbols: ${TRADING_SYMBOLS_PRIMARY}"
echo "  - Data Provider: ${PRIMARY_PROVIDER}"
echo ""
echo "🌐 Service URLs:"
echo "  - Grafana Dashboard: http://localhost:3000"
echo "    Username: admin"
echo "    Password: Check GRAFANA_ADMIN_PASSWORD in .env.stock-simulation"
echo "  - Prometheus: http://localhost:9090"
echo "  - Trading API: http://localhost:3030"
echo "  - Data Ingestion API: http://localhost:8001"
echo ""
echo "📝 Useful Commands:"
echo "  - View logs: docker-compose logs -f"
echo "  - Stop services: docker-compose down"
echo "  - View specific service: docker-compose logs -f [service-name]"
echo "  - Check database: docker exec -it neural_trader_timescaledb psql -U neural_trader"
echo ""
echo "💡 Tips:"
echo "  1. Wait 2-3 minutes for initial data collection"
echo "  2. Check Grafana for real-time trading metrics"
echo "  3. Monitor logs for trading decisions"
echo "  4. Stock markets are open Mon-Fri 9:30 AM - 4:00 PM ET"
echo ""

# Option to tail logs
read -p "Would you like to view the logs now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Showing logs (Press Ctrl+C to exit)..."
    docker-compose logs -f
fi