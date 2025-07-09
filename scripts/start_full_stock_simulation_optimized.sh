#!/bin/bash

# Neural Trader Full Stack Stock Trading Simulation - Optimized Version
# Uses Docker BuildKit and efficient build strategies to minimize disk usage

set -e

echo "📈 Neural Trader Stock Trading Simulation Startup (Optimized)"
echo "============================================================="

# Enable Docker BuildKit for better caching and smaller builds
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Check for at least one API key in environment
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_ADVANTAGE_API_KEY" ] && [ -z "$IEX_CLOUD_API_KEY" ] && [ -z "$POLYGON_API_KEY" ]; then
    echo ""
    echo "⚠️  WARNING: No API key found in environment variables!"
    echo ""
    echo "You need to set at least one API key in your environment:"
    echo ""
    echo "  export FINNHUB_API_KEY='your_actual_key'"
    echo "  or"
    echo "  export ALPHA_ADVANTAGE_API_KEY='your_actual_key'"
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

# Clean up old Docker resources first
echo "🧹 Cleaning up Docker resources..."
docker system prune -f --volumes 2>/dev/null || true

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
docker-compose down --volumes --remove-orphans 2>/dev/null || true

# Check if we're in Codespaces
if [ -n "$CODESPACES" ]; then
    echo "🌐 Detected GitHub Codespaces environment"
    echo "  - Using optimized build strategy"
    echo "  - BuildKit caching enabled"
    
    # For Codespaces, use the host Docker daemon if available
    if [ -S /var/run/docker-host.sock ]; then
        echo "  - Using host Docker daemon for builds"
        export DOCKER_HOST=unix:///var/run/docker-host.sock
    fi
fi

# Pull base images first to ensure we have them
echo "📥 Pulling base images..."
docker pull rustlang/rust:nightly || true
docker pull python:3.11-slim || true
docker pull debian:bookworm-slim || true

# Build images with optimized settings
echo "🔨 Building Docker images (optimized)..."
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1
if [ -f docker-compose.optimized.yml ]; then
    # Use optimized compose file if available
    docker-compose -f docker-compose.optimized.yml build
else
    # Fallback to regular compose with BuildKit
    docker-compose -f docker-compose.dev.yml build
fi

# Start only essential services first
echo "🚀 Starting core services..."
if [ -f docker-compose.optimized.yml ]; then
    docker-compose -f docker-compose.optimized.yml up -d timescaledb redis
else
    docker-compose -f docker-compose.dev.yml up -d timescaledb redis
fi

# Wait for core services
echo "⏳ Waiting for core services..."
sleep 10

# Start remaining services
echo "🚀 Starting application services..."
if [ -f docker-compose.optimized.yml ]; then
    docker-compose -f docker-compose.optimized.yml up -d neural-trader data-ingestion
else
    docker-compose -f docker-compose.dev.yml up -d neural-trader data-ingestion
fi

# Start monitoring services last (optional)
echo "📊 Starting monitoring services..."
docker-compose up -d prometheus grafana 2>/dev/null || echo "  - Monitoring services are optional"

# Wait for services to be healthy
echo "⏳ Waiting for all services to be ready..."
sleep 20

# Check service health
echo "🏥 Checking service health..."
docker-compose ps

# Clean up build cache
echo "🧹 Cleaning up build cache..."
docker builder prune -f --keep-storage=1GB

# Show disk usage
echo ""
echo "💾 Disk Usage Summary:"
docker system df
echo ""
df -h /

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
echo "  - Clean Docker: docker system prune -af --volumes"
echo ""
echo "💡 Optimization Tips:"
echo "  1. This script uses BuildKit for efficient caching"
echo "  2. Build cache is limited to 1GB to save space"
echo "  3. Services start in order of priority"
echo "  4. Run 'docker system prune -af' to free space if needed"
echo ""

# Option to tail logs
read -p "Would you like to view the logs now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Showing logs (Press Ctrl+C to exit)..."
    docker-compose logs -f
fi