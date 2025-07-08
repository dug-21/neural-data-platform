#!/bin/bash

# Neural Trader External Docker Solution
# Uses external volumes and optimized build to avoid disk space issues

set -e

echo "📈 Neural Trader External Docker Solution"
echo "========================================"
echo "Using external volumes to avoid disk space issues"
echo ""

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Check for at least one API key in environment
if [ -z "$FINNHUB_API_KEY" ] && [ -z "$ALPHA_VANTAGE_API_KEY" ] && [ -z "$IEX_CLOUD_API_KEY" ] && [ -z "$POLYGON_API_KEY" ]; then
    echo "⚠️  WARNING: No API key found in environment variables!"
    echo "Please set at least one API key. See setup_stock_env.sh for details."
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

# Create external volumes if they don't exist
echo "🗄️  Creating external Docker volumes..."
docker volume create neural_trader_timescale_data 2>/dev/null || true
docker volume create neural_trader_redis_data 2>/dev/null || true
docker volume create neural_trader_build_cache 2>/dev/null || true

# Show volume info
echo "📊 Volume status:"
docker volume ls | grep neural_trader || echo "  No volumes found, will create new ones"

# Create optimized compose file with external volumes
cat > docker-compose.external.yml << 'EOF'
services:
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    environment:
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-dev_password}
      - POSTGRES_USER=postgres
      - POSTGRES_DB=neural_trader
      - TIMESCALEDB_TELEMETRY=off
    ports:
      - "5432:5432"
    volumes:
      - neural_trader_timescale_data:/var/lib/postgresql/data
      - ./docker/timescaledb/init-scripts/01-init-db-minimal.sql:/docker-entrypoint-initdb.d/01-init-db.sql:ro
    networks:
      - neural_trader_dev
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - neural_trader_redis_data:/data
    networks:
      - neural_trader_dev
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 5

  # Development tools only - main app runs locally
  redis-commander:
    image: rediscommander/redis-commander:latest
    environment:
      - REDIS_HOSTS=local:redis:6379
    ports:
      - "8081:8081"
    depends_on:
      - redis
    networks:
      - neural_trader_dev

  pgadmin:
    image: dpage/pgadmin4:latest
    environment:
      - PGADMIN_DEFAULT_EMAIL=admin@neural-trader.local
      - PGADMIN_DEFAULT_PASSWORD=admin
    ports:
      - "8082:80"
    depends_on:
      - timescaledb
    networks:
      - neural_trader_dev

volumes:
  neural_trader_timescale_data:
    external: true
  neural_trader_redis_data:
    external: true

networks:
  neural_trader_dev:
    driver: bridge
EOF

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
docker-compose -f docker-compose.external.yml down 2>/dev/null || true
docker-compose -f docker-compose.dev.noversion.yml down 2>/dev/null || true

# Start only the database and cache services
echo "🚀 Starting database and cache services with external volumes..."
docker-compose -f docker-compose.external.yml up -d timescaledb redis redis-commander pgadmin

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
attempts=0
max_attempts=30
while [ $attempts -lt $max_attempts ]; do
    if docker-compose -f docker-compose.external.yml ps | grep -q "healthy"; then
        echo "✓ Services are healthy"
        break
    fi
    echo -n "."
    sleep 2
    attempts=$((attempts + 1))
done
echo ""

# Check service health
echo "🏥 Checking service health..."
docker-compose -f docker-compose.external.yml ps

# Show disk usage
echo ""
echo "💾 Disk usage after startup:"
df -h . | grep -E "(Filesystem|/workspaces)"
echo ""
echo "🗄️  Docker volume usage:"
docker system df -v 2>/dev/null | grep -A5 "VOLUME NAME" | grep neural_trader || echo "Volumes created successfully"

# Show connection info
echo ""
echo "✅ External Docker Solution Started!"
echo "===================================="
echo ""
echo "🚀 Next Steps:"
echo "1. Build and run the main application locally:"
echo "   export DATABASE_URL=postgresql://postgres:dev_password@localhost:5432/neural_trader"
echo "   export REDIS_URL=redis://localhost:6379"
echo "   cargo build --release"
echo "   cargo run --release"
echo ""
echo "2. Or run the data ingestion service:"
echo "   cd data_ingestion"
echo "   pip install -r requirements.txt"
echo "   python main.py"
echo ""
echo "🌐 Service URLs:"
echo "  - Redis Commander: http://localhost:8081"
echo "  - pgAdmin: http://localhost:8082"
echo "    Email: admin@neural-trader.local"
echo "    Password: admin"
echo ""
echo "📝 Useful Commands:"
echo "  - View logs: docker-compose -f docker-compose.external.yml logs -f"
echo "  - Stop services: docker-compose -f docker-compose.external.yml down"
echo "  - Clean volumes: docker volume rm neural_trader_timescale_data neural_trader_redis_data"
echo ""
echo "💡 This solution uses external Docker volumes to avoid disk space issues"
echo "   and runs the main application locally for faster development."