#!/bin/bash

# Simple Docker startup script for Neural Trader
set -e

echo "📈 Neural Trader Simple Docker Startup"
echo "===================================="

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    exit 1
fi

# Try to load Podman secrets if no API keys in environment
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_ADVANTAGE_API_KEY" ]; then
    if command -v podman &> /dev/null && [ -f ./scripts/export_podman_secrets.sh ]; then
        echo "📦 Loading Podman secrets..."
        eval "$(./scripts/export_podman_secrets.sh)"
    fi
fi

# Load environment
set -a
source .env.stock-simulation
set +a

# Use real Docker, not Podman
DOCKER_CMD="/usr/local/bin/docker"
COMPOSE_CMD="/usr/local/bin/docker-compose"

# Stop existing containers
echo "🧹 Cleaning up..."
$COMPOSE_CMD -f docker-compose.simple.yml down -v || true

# Build and start
echo "🔨 Building images..."
$COMPOSE_CMD -f docker-compose.simple.yml build

echo "🚀 Starting services..."
$COMPOSE_CMD -f docker-compose.simple.yml up -d

# Wait for services
echo "⏳ Waiting for services..."
sleep 20

# Show status
echo "📊 Service status:"
$COMPOSE_CMD -f docker-compose.simple.yml ps

echo ""
echo "✅ Services started!"
echo ""
echo "🌐 URLs:"
echo "  - Trading API: http://localhost:3030"
echo "  - Data Ingestion: http://localhost:8001"
echo "  - Grafana: http://localhost:3000 (admin/neural_trader)"
echo ""
echo "📝 Commands:"
echo "  - Logs: $COMPOSE_CMD -f docker-compose.simple.yml logs -f"
echo "  - Stop: $COMPOSE_CMD -f docker-compose.simple.yml down"