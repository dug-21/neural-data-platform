#!/bin/bash

# Start Neural Trader using simple docker-compose configuration
# This avoids the cache export conflicts

set -e

echo "📈 Neural Trader - Starting with Simple Configuration"
echo "===================================================="

# Check environment
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    exit 1
fi

# Load config
set -a
source .env.stock-simulation
set +a

# Check API keys
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_VANTAGE_API_KEY" ] && [ -z "$IEX_CLOUD_API_KEY" ] && [ -z "$POLYGON_API_KEY" ]; then
    echo "⚠️  No API keys found! Set at least one."
    exit 1
fi

# Check secrets
if [ -z "$POSTGRES_PASSWORD" ] || [ -z "$REDIS_PASSWORD" ]; then
    echo "⚠️  Missing passwords! Run: ./scripts/generate_dev_secrets.sh"
    exit 1
fi

# Enable BuildKit
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Use regular docker-compose.dev.yml
echo "🚀 Starting services..."
docker-compose -f docker-compose.dev.yml up -d --build

echo ""
echo "✅ Services starting! Wait ~30 seconds for full initialization."
echo ""
echo "📊 Access points:"
echo "  - API: http://localhost:3030"
echo "  - Grafana: http://localhost:3000"
echo ""