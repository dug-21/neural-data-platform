#!/bin/bash

# Run pre-built images from Docker Hub or GitHub Container Registry

echo "📈 Running Pre-built Neural Trader Images"
echo "========================================"

# Create docker-compose for pre-built images
cat > docker-compose.prebuilt.yml << 'EOF'
version: '3.8'

services:
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    environment:
      - POSTGRES_USER=neural_trader
      - POSTGRES_PASSWORD=dev_password
      - POSTGRES_DB=neural_trader_db
    ports:
      - "5432:5432"
    volumes:
      - ./docker/timescaledb/init-scripts:/docker-entrypoint-initdb.d:ro

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    command: redis-server --appendonly yes

  # Pull pre-built images if available
  # neural-trader:
  #   image: ghcr.io/yourusername/neural-trader:latest
  #   environment:
  #     - DATABASE_URL=postgresql://neural_trader:dev_password@timescaledb:5432/neural_trader_db
  #     - REDIS_URL=redis://redis:6379
  #   ports:
  #     - "3030:3030"
  #   depends_on:
  #     - timescaledb
  #     - redis
EOF

# Run only the database services
docker-compose -f docker-compose.prebuilt.yml up -d timescaledb redis

echo ""
echo "✅ Database services started!"
echo ""
echo "Now run the Rust application locally:"
echo "  export DATABASE_URL=postgresql://neural_trader:dev_password@localhost:5432/neural_trader_db"
echo "  export REDIS_URL=redis://localhost:6379"
echo "  cargo run --release"