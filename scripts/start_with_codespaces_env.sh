#!/bin/bash

# Start Neural Trader with Codespaces environment variables properly passed
# This script ensures all environment variables are available to Docker Compose

set -e

echo "📈 Neural Trader - Codespaces Environment Startup"
echo "================================================"

# Check for .env.stock-simulation
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    exit 1
fi

# Load the .env file for non-secret configuration
echo "📋 Loading configuration..."
set -a
source .env.stock-simulation
set +a

# Debug: Show what environment variables we have
echo "🔍 Checking environment variables..."
echo "  FINNHUB_API_KEY: ${FINNHUB_API_KEY:+[SET]}"
echo "  ALPHA_VANTAGE_API_KEY: ${ALPHA_VANTAGE_API_KEY:+[SET]}"
echo "  IEX_CLOUD_API_KEY: ${IEX_CLOUD_API_KEY:+[SET]}"
echo "  POLYGON_API_KEY: ${POLYGON_API_KEY:+[SET]}"
echo "  POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:+[SET]}"
echo "  REDIS_PASSWORD: ${REDIS_PASSWORD:+[SET]}"
echo "  JWT_SECRET: ${JWT_SECRET:+[SET]}"
echo "  GRAFANA_ADMIN_PASSWORD: ${GRAFANA_ADMIN_PASSWORD:+[SET]}"

# Check for at least one API key
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_VANTAGE_API_KEY" ] && [ -z "$IEX_CLOUD_API_KEY" ] && [ -z "$POLYGON_API_KEY" ]; then
    echo ""
    echo "⚠️  WARNING: No API key found in environment!"
    echo ""
    echo "Codespaces secrets should be automatically available."
    echo "Check your Codespaces settings to ensure secrets are configured."
    echo ""
    # Don't exit - let's see what happens
fi

# Enable BuildKit
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Clean up to save space
echo "🧹 Cleaning up Docker to free space..."
docker system prune -f --volumes || true

# Create a temporary .env file that explicitly exports all variables
# This ensures Docker Compose sees them
echo "📝 Creating temporary environment file..."
cat > .env.compose.tmp << EOF
# Generated from Codespaces environment
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
REDIS_PASSWORD=${REDIS_PASSWORD}
JWT_SECRET=${JWT_SECRET}
ENCRYPTION_KEY=${ENCRYPTION_KEY}
SESSION_SECRET=${SESSION_SECRET}
GRAFANA_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD}
FINNHUB_API_KEY=${FINNHUB_API_KEY}
ALPHA_VANTAGE_API_KEY=${ALPHA_VANTAGE_API_KEY}
IEX_CLOUD_API_KEY=${IEX_CLOUD_API_KEY}
POLYGON_API_KEY=${POLYGON_API_KEY}
FRED_API_KEY=${FRED_API_KEY}
NEWSAPI_KEY=${NEWSAPI_KEY}
YAHOO_API_KEY=${YAHOO_API_KEY}
NASDAQ_API_KEY=${NASDAQ_API_KEY}
REDDIT_CLIENT_ID=${REDDIT_CLIENT_ID}
REDDIT_CLIENT_SECRET=${REDDIT_CLIENT_SECRET}
QUANDL_API_KEY=${QUANDL_API_KEY}
SMTP_HOST=${SMTP_HOST}
SMTP_PORT=${SMTP_PORT}
SMTP_USER=${SMTP_USER}
SMTP_PASSWORD=${SMTP_PASSWORD}
ALERT_EMAIL=${ALERT_EMAIL}
EOF

# Use the temporary env file with docker-compose
echo "🚀 Starting services with explicit environment..."
docker-compose -f docker-compose.dev.yml \
    --env-file .env.compose.tmp \
    --env-file .env.stock-simulation \
    up -d --build

# Clean up temp file
rm -f .env.compose.tmp

# Wait for services
echo "⏳ Waiting for services to be ready..."
sleep 20

# Check health
echo "🏥 Checking service health..."
docker-compose -f docker-compose.dev.yml ps

echo ""
echo "✅ Services started!"
echo ""
echo "📊 Access points:"
echo "  - API: http://localhost:3030"
echo "  - Grafana: http://localhost:3000"
echo "  - Prometheus: http://localhost:9090"
echo ""
echo "📝 Commands:"
echo "  - Logs: docker-compose -f docker-compose.dev.yml logs -f"
echo "  - Stop: docker-compose -f docker-compose.dev.yml down"
echo ""