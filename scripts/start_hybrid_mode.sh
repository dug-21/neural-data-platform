#!/bin/bash

# Hybrid mode: Run databases in Docker, application locally
# This completely avoids Docker build issues

set -e

echo "🚀 Neural Trader - Hybrid Mode (Local App + Docker DBs)"
echo "======================================================"

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

# Check environment
echo "🔍 Checking environment variables..."
echo "  API Keys: ${FINNHUB_API_KEY:+✓} ${ALPHA_VANTAGE_API_KEY:+✓} ${IEX_CLOUD_API_KEY:+✓} ${POLYGON_API_KEY:+✓}"
echo "  Passwords: ${POSTGRES_PASSWORD:+✓} ${REDIS_PASSWORD:+✓}"

if [ -z "$POSTGRES_PASSWORD" ] || [ -z "$REDIS_PASSWORD" ]; then
    echo ""
    echo "⚠️  Missing passwords! Generating defaults for development..."
    export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-dev_postgres_$(date +%s)}"
    export REDIS_PASSWORD="${REDIS_PASSWORD:-dev_redis_$(date +%s)}"
    echo "  POSTGRES_PASSWORD: [GENERATED]"
    echo "  REDIS_PASSWORD: [GENERATED]"
fi

# Create minimal docker-compose for just databases
cat > docker-compose.hybrid.yml << EOF
version: '3.8'

services:
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    container_name: neural_trader_timescaledb
    environment:
      POSTGRES_USER: neural_trader
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: neural_trader_db
    ports:
      - "5432:5432"
    volumes:
      - ./docker/timescaledb/init-scripts:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U neural_trader"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    container_name: neural_trader_redis
    command: redis-server --requirepass ${REDIS_PASSWORD}
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "--raw", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  grafana:
    image: grafana/grafana:latest
    container_name: neural_trader_grafana
    environment:
      GF_SECURITY_ADMIN_USER: admin
      GF_SECURITY_ADMIN_PASSWORD: ${GRAFANA_ADMIN_PASSWORD:-admin}
      GF_USERS_ALLOW_SIGN_UP: false
    ports:
      - "3000:3000"

  prometheus:
    image: prom/prometheus:latest
    container_name: neural_trader_prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./docker/prometheus:/etc/prometheus:ro
EOF

# Start databases only
echo "🗄️  Starting database services..."
docker-compose -f docker-compose.hybrid.yml up -d

# Wait for databases
echo "⏳ Waiting for databases to be ready..."
for i in {1..30}; do
    if docker exec neural_trader_timescaledb pg_isready -U neural_trader >/dev/null 2>&1; then
        echo "✅ PostgreSQL is ready!"
        break
    fi
    echo -n "."
    sleep 1
done

# Export database URLs for local app
export DATABASE_URL="postgresql://neural_trader:${POSTGRES_PASSWORD}@localhost:5432/neural_trader_db"
export REDIS_URL="redis://:${REDIS_PASSWORD}@localhost:6379"

echo ""
echo "📦 Building application locally..."
cargo build --release --bin neural-trader

echo ""
echo "🚀 Starting Neural Trader application..."
echo ""
echo "Database URL: $DATABASE_URL"
echo "Redis URL: $REDIS_URL"
echo ""

# Create a startup script for the app
cat > run_neural_trader.sh << EOF
#!/bin/bash
export DATABASE_URL="$DATABASE_URL"
export REDIS_URL="$REDIS_URL"
export LOG_LEVEL="${LOG_LEVEL:-info}"
export RUST_LOG="${RUST_LOG:-info}"
export TRADING_MODE="${TRADING_MODE}"
export INITIAL_CAPITAL="${INITIAL_CAPITAL}"
export PRIMARY_PROVIDER="${PRIMARY_PROVIDER}"
export FINNHUB_API_KEY="${FINNHUB_API_KEY}"
export ALPHA_VANTAGE_API_KEY="${ALPHA_VANTAGE_API_KEY}"
export IEX_CLOUD_API_KEY="${IEX_CLOUD_API_KEY}"
export POLYGON_API_KEY="${POLYGON_API_KEY}"

echo "Starting Neural Trader..."
./target/release/neural-trader
EOF

chmod +x run_neural_trader.sh

echo ""
echo "✅ Setup complete!"
echo ""
echo "📊 Services:"
echo "  - PostgreSQL: localhost:5432"
echo "  - Redis: localhost:6379"
echo "  - Grafana: http://localhost:3000 (admin/${GRAFANA_ADMIN_PASSWORD:-admin})"
echo "  - Prometheus: http://localhost:9090"
echo ""
echo "🏃 To run the application:"
echo "  ./run_neural_trader.sh"
echo ""
echo "🛑 To stop databases:"
echo "  docker-compose -f docker-compose.hybrid.yml down"
echo ""

# Option to run now
read -p "Start the application now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    ./run_neural_trader.sh
fi