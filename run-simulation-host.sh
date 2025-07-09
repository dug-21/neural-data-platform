#!/bin/bash
# Script to run neural-trader simulation from host machine
# This avoids the complexity of Podman-in-Podman

echo "🚀 Starting Neural Trader Simulation on Host..."

# Check if Podman is available
if ! command -v podman &> /dev/null; then
    echo "❌ Podman not found. Please install Podman Desktop."
    exit 1
fi

# Build the application image if needed
echo "📦 Building application image..."
podman build -t neural-trader:simulation -f Dockerfile .

# Create a network for containers to communicate
echo "🌐 Creating container network..."
podman network create neural-trader-net 2>/dev/null || true

# Run PostgreSQL for simulation
echo "🗄️ Starting PostgreSQL..."
podman run -d \
    --name neural-trader-postgres \
    --network neural-trader-net \
    -e POSTGRES_USER=postgres \
    -e POSTGRES_PASSWORD=dev_password \
    -e POSTGRES_DB=neural_trader \
    -p 5432:5432 \
    postgres:16-alpine

# Run Redis for simulation
echo "📮 Starting Redis..."
podman run -d \
    --name neural-trader-redis \
    --network neural-trader-net \
    -p 6379:6379 \
    redis:7-alpine

# Wait for services to be ready
echo "⏳ Waiting for services to start..."
sleep 5

# Run the neural trader application
echo "🤖 Starting Neural Trader Application..."
podman run -d \
    --name neural-trader-app \
    --network neural-trader-net \
    -e DATABASE_URL="postgresql://postgres:dev_password@neural-trader-postgres:5432/neural_trader" \
    -e REDIS_URL="redis://neural-trader-redis:6379" \
    -e RUST_LOG="info" \
    -p 3030:3030 \
    neural-trader:simulation

echo "✅ Simulation environment is running!"
echo ""
echo "📊 Services available at:"
echo "  - Neural Trader API: http://localhost:3030"
echo "  - PostgreSQL: localhost:5432"
echo "  - Redis: localhost:6379"
echo ""
echo "📊 View logs with:"
echo "  podman logs -f neural-trader-app"
echo ""
echo "🛑 Stop simulation with:"
echo "  podman stop neural-trader-app neural-trader-postgres neural-trader-redis"
echo "  podman rm neural-trader-app neural-trader-postgres neural-trader-redis"
echo "  podman network rm neural-trader-net"