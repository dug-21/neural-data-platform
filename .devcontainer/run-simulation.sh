#!/bin/bash
# Script to run neural-trader simulation in a separate container
# This runs from within the dev container using the mounted Podman socket

echo "🚀 Starting Neural Trader Simulation..."

# Check if Podman is available
if ! command -v podman &> /dev/null; then
    echo "❌ Podman not found. Please ensure Podman socket is mounted correctly."
    exit 1
fi

# Build the application image if needed
echo "📦 Building application image..."
podman build -t neural-trader:simulation -f Dockerfile .

# Run PostgreSQL for simulation
echo "🗄️ Starting PostgreSQL..."
podman run -d \
    --name neural-trader-postgres \
    -e POSTGRES_USER=postgres \
    -e POSTGRES_PASSWORD=dev_password \
    -e POSTGRES_DB=neural_trader \
    -p 5432:5432 \
    postgres:16-alpine

# Run Redis for simulation
echo "📮 Starting Redis..."
podman run -d \
    --name neural-trader-redis \
    -p 6379:6379 \
    redis:7-alpine

# Wait for services to be ready
echo "⏳ Waiting for services to start..."
sleep 5

# Run the neural trader application
echo "🤖 Starting Neural Trader Application..."
podman run -d \
    --name neural-trader-app \
    --network host \
    -e DATABASE_URL="postgresql://postgres:dev_password@localhost:5432/neural_trader" \
    -e REDIS_URL="redis://localhost:6379" \
    -e RUST_LOG="info" \
    neural-trader:simulation

echo "✅ Simulation environment is running!"
echo ""
echo "📊 View logs with:"
echo "  podman logs -f neural-trader-app"
echo ""
echo "🛑 Stop simulation with:"
echo "  podman stop neural-trader-app neural-trader-postgres neural-trader-redis"
echo "  podman rm neural-trader-app neural-trader-postgres neural-trader-redis"