#!/bin/bash

# Neural Trader Full Stack Stock Trading Simulation - External Docker Version
# Runs complete Docker stack using external Docker daemon to avoid disk space issues

set -e

echo "📈 Neural Trader Stock Trading Simulation Startup (External Docker)"
echo "=================================================================="

# Configuration for external Docker daemon
EXTERNAL_DOCKER_HOST="tcp://host.docker.internal:2375"  # Default for Docker Desktop
BUILD_DIR="/tmp/neural-trader-build"
CONTEXT_DIR="/tmp/neural-trader-context"

# Allow user to override Docker host
if [ ! -z "$DOCKER_HOST_OVERRIDE" ]; then
    EXTERNAL_DOCKER_HOST="$DOCKER_HOST_OVERRIDE"
fi

echo "🔧 Configuration:"
echo "  - External Docker: $EXTERNAL_DOCKER_HOST"
echo "  - Build Directory: $BUILD_DIR"
echo "  - Context Directory: $CONTEXT_DIR"
echo ""

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

# Test external Docker connection
echo "🔍 Testing external Docker connection..."
if ! DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker info > /dev/null 2>&1; then
    echo "❌ Error: Cannot connect to external Docker at $EXTERNAL_DOCKER_HOST"
    echo ""
    echo "Please ensure:"
    echo "1. Docker Desktop or external Docker daemon is running"
    echo "2. Docker is configured to accept TCP connections"
    echo "3. You can override the host with: DOCKER_HOST_OVERRIDE=tcp://your-host:2375"
    echo ""
    echo "For Docker Desktop, enable 'Expose daemon on tcp://localhost:2375 without TLS'"
    exit 1
fi

echo "✅ Successfully connected to external Docker"

# Create temporary build directory
echo "📁 Preparing build context..."
rm -rf $BUILD_DIR $CONTEXT_DIR
mkdir -p $BUILD_DIR $CONTEXT_DIR

# Copy necessary files to build context (minimal copy)
echo "📦 Copying build files..."
cp -r . $CONTEXT_DIR/
cd $CONTEXT_DIR

# Stop any existing containers on external Docker
echo "🛑 Stopping any existing containers on external Docker..."
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose down 2>/dev/null || true

# Build images on external Docker
echo "🔨 Building Docker images on external Docker..."
echo "  This will use the external Docker's disk space instead of the container's"
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml build

# Start the full stack on external Docker
echo "🚀 Starting full trading stack on external Docker..."
echo "  - TimescaleDB (time-series database)"
echo "  - Redis (real-time cache)"
echo "  - Data Ingestion Service"
echo "  - Neural Trader Application"
echo "  - Redis Commander (optional dev tool)"
echo "  - PgAdmin (optional dev tool)"
echo ""

# Start core services
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml up -d

# Optionally start dev tools
read -p "Would you like to start development tools (Redis Commander, PgAdmin)? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🔧 Starting development tools..."
    DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml --profile dev-tools up -d
fi

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 20

# Check service health
echo "🏥 Checking service health..."
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml ps

# Show connection info
echo ""
echo "✅ Stock Trading Simulation Started on External Docker!"
echo "======================================================"
echo ""
echo "📊 Trading Configuration:"
echo "  - Mode: Paper Trading (Simulation)"
echo "  - Initial Capital: \$10,000"
echo "  - Asset Class: US Stocks"
echo "  - Primary Symbols: ${TRADING_SYMBOLS_PRIMARY}"
echo "  - Data Provider: ${PRIMARY_PROVIDER}"
echo ""
echo "🌐 Service URLs (accessible from host):"
echo "  - Grafana Dashboard: http://localhost:3000"
echo "    Username: admin"
echo "    Password: Check GRAFANA_ADMIN_PASSWORD in .env.stock-simulation"
echo "  - Prometheus: http://localhost:9090"
echo "  - Trading API: http://localhost:3030"
echo "  - Data Ingestion API: http://localhost:8001"
echo ""
echo "📝 Useful Commands (all use external Docker):"
echo "  - View logs: DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml logs -f"
echo "  - Stop services: DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml down"
echo "  - View specific service: DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml logs -f [service-name]"
echo "  - Check database: DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker exec -it neural-trader-timescaledb-1 psql -U postgres neural_trader"
echo "  - Start dev tools: DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml --profile dev-tools up -d"
echo ""
echo "💡 Tips:"
echo "  1. Wait 2-3 minutes for initial data collection"
echo "  2. Check Grafana for real-time trading metrics"
echo "  3. Monitor logs for trading decisions"
echo "  4. Stock markets are open Mon-Fri 9:30 AM - 4:00 PM ET"
echo "  5. All containers are running on external Docker to save disk space"
echo ""

# Clean up build directory
echo "🧹 Cleaning up temporary files..."
rm -rf $BUILD_DIR

# Option to tail logs
read -p "Would you like to view the logs now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Showing logs (Press Ctrl+C to exit)..."
    cd $CONTEXT_DIR
    DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml logs -f
fi