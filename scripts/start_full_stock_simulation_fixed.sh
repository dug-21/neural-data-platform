#!/bin/bash

# Neural Trader Full Stack Stock Trading Simulation - Fixed for Docker Desktop
# Uses fixed Dockerfiles that work with Docker Desktop on macOS

set -e

echo "📈 Neural Trader Stock Trading Simulation Startup (Fixed)"
echo "========================================================="

# Enable Docker BuildKit
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
    echo "Loading Podman secrets if available..."
    
    # Try to load from Podman secrets
    if command -v podman &> /dev/null && [ -f ./scripts/export_podman_secrets.sh ]; then
        echo "📦 Loading Podman secrets..."
        eval "$(./scripts/export_podman_secrets.sh)"
        echo "✅ Secrets loaded"
    else
        echo ""
        echo "You need to set at least one API key in your environment:"
        echo ""
        echo "  export FINNHUB_API_KEY='your_actual_key'"
        echo "  or"
        echo "  export ALPHA_ADVANTAGE_API_KEY='your_actual_key'"
        echo ""
        echo "Get free API keys from:"
        echo "  - Finnhub: https://finnhub.io/register"
        echo "  - Alpha Vantage: https://www.alphavantage.co/support/#api-key"
        echo ""
        exit 1
    fi
fi

# Export environment variables
echo "📋 Loading stock trading configuration..."
set -a
source .env.stock-simulation
set +a

# Clean up any problematic containers
echo "🧹 Cleaning up any existing containers..."
docker-compose -f docker-compose.optimized.fixed.yml down --volumes --remove-orphans 2>/dev/null || true

# Clean up Docker build cache if needed
echo "🧹 Cleaning Docker build cache..."
docker builder prune -f --filter "until=1h" 2>/dev/null || true

# Build images with fixed Dockerfiles
echo "🔨 Building Docker images (fixed version)..."
docker-compose -f docker-compose.optimized.fixed.yml build --no-cache data-ingestion
docker-compose -f docker-compose.optimized.fixed.yml build neural-trader

# Start services
echo "🚀 Starting services..."
docker-compose -f docker-compose.optimized.fixed.yml up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 20

# Check service health
echo "🏥 Checking service health..."
docker-compose -f docker-compose.optimized.fixed.yml ps

# Show logs for debugging if needed
echo ""
echo "📋 Recent logs from data-ingestion:"
docker-compose -f docker-compose.optimized.fixed.yml logs --tail=20 data-ingestion

# Show connection info
echo ""
echo "✅ Stock Trading Simulation Started!"
echo "===================================="
echo ""
echo "📊 Trading Configuration:"
echo "  - Mode: Paper Trading (Simulation)"
echo "  - Initial Capital: $10,000"
echo "  - Primary Symbols: ${TRADING_SYMBOLS_PRIMARY}"
echo "  - Data Provider: ${PRIMARY_PROVIDER}"
echo ""
echo "🌐 Service URLs:"
echo "  - Grafana Dashboard: http://localhost:3000"
echo "    Username: admin"
echo "    Password: neural_trader"
echo "  - Trading API: http://localhost:3030"
echo "  - Data Ingestion API: http://localhost:8001"
echo ""
echo "📝 Useful Commands:"
echo "  - View logs: docker-compose -f docker-compose.optimized.fixed.yml logs -f"
echo "  - Stop services: docker-compose -f docker-compose.optimized.fixed.yml down"
echo "  - View specific service: docker-compose -f docker-compose.optimized.fixed.yml logs -f [service-name]"
echo ""

# Option to tail logs
read -p "Would you like to view the logs now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Showing logs (Press Ctrl+C to exit)..."
    docker-compose -f docker-compose.optimized.fixed.yml logs -f
fi