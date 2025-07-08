#!/bin/bash

# Simple startup script for Neural Trader Stock Trading
# Avoids complex caching issues while still being efficient

set -e

echo "📈 Neural Trader Stock Trading - Simple Startup"
echo "=============================================="

# Check for .env.stock-simulation
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    exit 1
fi

# Load configuration
echo "📋 Loading configuration..."
set -a
source .env.stock-simulation
set +a

# Check for API keys in environment
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_VANTAGE_API_KEY" ] && [ -z "$IEX_CLOUD_API_KEY" ] && [ -z "$POLYGON_API_KEY" ]; then
    echo ""
    echo "⚠️  No API keys found in environment!"
    echo ""
    echo "Set at least one:"
    echo "  export FINNHUB_API_KEY='your_key'"
    echo "  export ALPHA_VANTAGE_API_KEY='your_key'"
    echo "  export IEX_CLOUD_API_KEY='your_key'"
    echo "  export POLYGON_API_KEY='your_key'"
    echo ""
    exit 1
fi

# Check for required secrets
if [ -z "$POSTGRES_PASSWORD" ] || [ -z "$REDIS_PASSWORD" ]; then
    echo ""
    echo "⚠️  Missing required passwords!"
    echo ""
    echo "Run: ./scripts/generate_dev_secrets.sh"
    echo ""
    exit 1
fi

# Enable BuildKit
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Clean up any problematic state
echo "🧹 Cleaning up..."
docker-compose -f docker-compose.dev.yml down 2>/dev/null || true

# Use the regular docker-compose.dev.yml without the optimized caching
echo "🔨 Building services..."
docker-compose -f docker-compose.dev.yml build --no-cache neural-trader data-ingestion

echo "🚀 Starting services..."
docker-compose -f docker-compose.dev.yml up -d

# Wait for services
echo "⏳ Waiting for services to be ready..."
sleep 20

# Check health
echo "🏥 Checking service health..."
docker-compose -f docker-compose.dev.yml ps

echo ""
echo "✅ Stock Trading Simulation Started!"
echo "===================================="
echo ""
echo "📊 Configuration:"
echo "  - Mode: ${TRADING_MODE}"
echo "  - Capital: $${INITIAL_CAPITAL}"
echo "  - Provider: ${PRIMARY_PROVIDER}"
echo ""
echo "🌐 Services:"
echo "  - Trading API: http://localhost:3030"
echo "  - Grafana: http://localhost:3000 (admin/${GRAFANA_ADMIN_PASSWORD})"
echo "  - Prometheus: http://localhost:9090"
echo ""
echo "📝 Commands:"
echo "  - Logs: docker-compose -f docker-compose.dev.yml logs -f"
echo "  - Stop: docker-compose -f docker-compose.dev.yml down"
echo ""

# Optional: tail logs
read -p "View logs? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker-compose -f docker-compose.dev.yml logs -f
fi