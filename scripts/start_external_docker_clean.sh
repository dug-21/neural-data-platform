#!/bin/bash

# Neural Trader External Docker Solution - Clean Start
# Removes old volumes and starts fresh

set -e

echo "📈 Neural Trader External Docker Solution - Clean Start"
echo "======================================================"
echo ""

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Export environment variables
echo "📋 Loading stock trading configuration..."
set -a
source .env.stock-simulation
set +a

# Disable Docker BuildKit for compatibility
export DOCKER_BUILDKIT=0
export COMPOSE_DOCKER_CLI_BUILD=0

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
docker-compose -f docker-compose.external.yml down 2>/dev/null || true
docker-compose -f docker-compose.dev.noversion.yml down 2>/dev/null || true

# Remove old volumes for clean start
echo "🧹 Removing old volumes for clean start..."
docker volume rm neural_trader_timescale_data 2>/dev/null || true
docker volume rm neural_trader_redis_data 2>/dev/null || true

# Create external volumes
echo "🗄️  Creating fresh external Docker volumes..."
docker volume create neural_trader_timescale_data
docker volume create neural_trader_redis_data
docker volume create neural_trader_build_cache 2>/dev/null || true

# Start only the database and cache services
echo "🚀 Starting services with clean volumes..."
docker-compose -f docker-compose.external.yml up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 15

# Check service health
echo "🏥 Checking service health..."
docker-compose -f docker-compose.external.yml ps

# Test database connection
echo ""
echo "🔍 Testing database connection..."
docker exec neural_trader_stocks-timescaledb-1 psql -U postgres -c "SELECT version();" 2>/dev/null && echo "✅ Database is accessible!" || echo "⚠️  Database may still be initializing..."

# Show disk usage
echo ""
echo "💾 Disk usage after startup:"
df -h . | grep -E "(Filesystem|/workspaces)"

# Show connection info
echo ""
echo "✅ External Docker Solution Started!"
echo "===================================="
echo ""
echo "🌐 Service URLs:"
echo "  - Redis Commander: http://localhost:8081"
echo "  - pgAdmin: http://localhost:8082"
echo "    Email: admin@neural-trader.local"
echo "    Password: admin"
echo ""
echo "🚀 To run the Neural Trader app locally:"
echo "   export DATABASE_URL=postgresql://postgres:dev_password@localhost:5432/neural_trader"
echo "   export REDIS_URL=redis://localhost:6379"
echo "   cargo run --release"
echo ""
echo "📝 Useful Commands:"
echo "  - View logs: docker-compose -f docker-compose.external.yml logs -f"
echo "  - Stop services: docker-compose -f docker-compose.external.yml down"
echo ""