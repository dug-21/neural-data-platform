#!/bin/bash

# Minimal footprint startup for Codespaces
# Everything runs within the Codespace VM with minimal disk usage

set -e

echo "📈 Neural Trader - Minimal Footprint Startup"
echo "==========================================="
echo "Running entirely within Codespaces (no external connections)"
echo ""

# Check current disk usage
echo "💾 Current disk usage:"
df -h / | grep -E "Filesystem|overlay"
echo ""

# Clean up Docker aggressively first
echo "🧹 Cleaning up Docker to free maximum space..."
docker system prune -af --volumes || true
docker builder prune -af || true
echo ""

# Check disk after cleanup
echo "💾 Disk usage after cleanup:"
df -h / | grep overlay
echo ""

# Load configuration
if [ -f .env.stock-simulation ]; then
    set -a
    source .env.stock-simulation
    set +a
fi

# Check Codespaces secrets
echo "🔍 Checking Codespaces secrets..."
echo "  FINNHUB_API_KEY: ${FINNHUB_API_KEY:+[SET]}"
echo "  ALPHA_VANTAGE_API_KEY: ${ALPHA_VANTAGE_API_KEY:+[SET]}"
echo "  POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:+[SET]}"
echo "  REDIS_PASSWORD: ${REDIS_PASSWORD:+[SET]}"
echo ""

# If secrets are missing, generate dev ones
if [ -z "$POSTGRES_PASSWORD" ]; then
    export POSTGRES_PASSWORD="dev_postgres_$(date +%s)"
    echo "Generated POSTGRES_PASSWORD"
fi
if [ -z "$REDIS_PASSWORD" ]; then
    export REDIS_PASSWORD="dev_redis_$(date +%s)"
    echo "Generated REDIS_PASSWORD"
fi

# Create ultra-minimal compose file
cat > docker-compose.ultramin.yml << 'EOF'
version: '3.8'

services:
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    environment:
      POSTGRES_USER: neural_trader
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: neural_trader_db
    ports:
      - "5432:5432"
    command: >
      postgres
      -c shared_buffers=128MB
      -c work_mem=4MB
      -c maintenance_work_mem=64MB
      -c effective_cache_size=256MB
      -c max_connections=50
    healthcheck:
      test: ["CMD", "pg_isready", "-U", "neural_trader"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    command: >
      redis-server
      --requirepass ${REDIS_PASSWORD}
      --maxmemory 128mb
      --maxmemory-policy allkeys-lru
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
EOF

# Start only the databases
echo "🗄️  Starting minimal database services..."
docker-compose -f docker-compose.ultramin.yml up -d

# Wait for services
echo "⏳ Waiting for databases..."
sleep 15

# Check if services are running
echo "📊 Service status:"
docker-compose -f docker-compose.ultramin.yml ps
echo ""

# Build and run the app locally in Codespaces
echo "🔨 Building application locally (no Docker)..."
export DATABASE_URL="postgresql://neural_trader:${POSTGRES_PASSWORD}@localhost:5432/neural_trader_db"
export REDIS_URL="redis://:${REDIS_PASSWORD}@localhost:6379"

# Check if we can build
if command -v cargo &> /dev/null; then
    echo "✅ Cargo found, building Neural Trader..."
    cargo build --release --bin neural-trader
    
    echo ""
    echo "✅ Build complete! You can now run:"
    echo ""
    echo "export DATABASE_URL=\"$DATABASE_URL\""
    echo "export REDIS_URL=\"$REDIS_URL\""
    echo "./target/release/neural-trader"
else
    echo "❌ Cargo not found. Install Rust first."
fi

echo ""
echo "📝 Database connection info:"
echo "  PostgreSQL: localhost:5432 (user: neural_trader)"
echo "  Redis: localhost:6379"
echo ""
echo "🛑 To stop databases:"
echo "  docker-compose -f docker-compose.ultramin.yml down"
echo ""