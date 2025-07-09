#!/bin/bash

# Neural Trader Full Stack Stock Trading Simulation - Podman Version
# Runs complete Podman stack with all services for stock trading

set -e

echo "📈 Neural Trader Stock Trading Simulation Startup (Podman)"
echo "========================================================="

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Check for at least one API key secret in Podman
echo "🔍 Checking for API key secrets..."
SECRETS=$(podman secret ls | tail -n +2 | awk '{print $2}' || echo "")

echo $SECRETS

if ! echo "$SECRETS" | grep -qiE "(finnhub_api_key|alpha_vantage_api_key|iex_cloud_api_key|polygon_api_key)"; then
    echo ""
    echo "⚠️  WARNING: No API key secrets found in Podman!"
    echo ""
    echo "You need to set up at least one API key as a Podman secret."
    echo ""
    echo "Run: ./scripts/setup_podman_secrets.sh"
    echo ""
    echo "This will securely store your API keys without putting them on disk."
    echo ""
    echo "Get free API keys from:"
    echo "  - Finnhub: https://finnhub.io/register (BEST - 60 calls/min)"
    echo "  - Alpha Vantage: https://www.alphavantage.co/support/#api-key"
    echo "  - IEX Cloud: https://iexcloud.io/console/tokens"
    echo "  - Polygon.io: https://polygon.io/dashboard/signup"
    echo ""
    exit 1
fi

# Show which API secrets are available
echo "✅ Found API secrets:"
echo "$SECRETS" | grep -E "(finnhub_api_key|alpha_vantage_api_key|iex_cloud_api_key|polygon_api_key)" | sed 's/^/  - /'
echo ""

# Export environment variables
echo "📋 Loading stock trading configuration..."
set -a
source .env.stock-simulation
set +a

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
podman-compose -f docker-compose.podman.yml down 2>/dev/null || true

# Build images if needed
echo "🔨 Building container images..."
podman-compose -f docker-compose.podman.yml build

# Start the full stack
echo "🚀 Starting full trading stack..."
echo "  - TimescaleDB (time-series database)"
echo "  - Redis (real-time cache)"
echo "  - Data Ingestion Service"
echo "  - Neural Trader Application"
echo "  - Prometheus (metrics)"
echo "  - Grafana (visualization)"
echo ""

podman-compose -f docker-compose.podman.yml up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 20

# Check service health
echo "🏥 Checking service health..."
podman-compose -f docker-compose.podman.yml ps

# Show connection info
echo ""
echo "✅ Stock Trading Simulation Started!"
echo "===================================="
echo ""
echo "📊 Trading Configuration:"
echo "  - Mode: Paper Trading (Simulation)"
echo "  - Initial Capital: $10,000"
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
echo "  - View logs: podman-compose -f docker-compose.podman.yml logs -f"
echo "  - Stop services: podman-compose -f docker-compose.podman.yml down"
echo "  - View specific service: podman-compose -f docker-compose.podman.yml logs -f [service-name]"
echo "  - Check database: podman exec -it neural_trader_timescaledb psql -U neural_trader"
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
    podman-compose -f docker-compose.podman.yml logs -f
fi