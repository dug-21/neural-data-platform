#!/bin/bash

# Hybrid Development Script for Neural Trader
# Runs database services in Docker, application locally for better performance

set -e

echo "🚀 Neural Trader Hybrid Development Mode"
echo "======================================="
echo ""

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Export environment variables
echo "📋 Loading configuration..."
set -a
source .env.stock-simulation
set +a

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
echo "🔍 Checking prerequisites..."

if ! command_exists cargo; then
    echo "❌ Rust/Cargo not found. Please install Rust first."
    echo "   Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

if ! command_exists docker; then
    echo "❌ Docker not found. Please install Docker first."
    exit 1
fi

if ! command_exists docker-compose; then
    echo "❌ docker-compose not found. Please install docker-compose first."
    exit 1
fi

echo "✅ All prerequisites found"
echo ""

# Enable BuildKit for efficient builds
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Check if using external Docker
if [ -n "$DOCKER_HOST" ]; then
    echo "🐳 Using external Docker: $DOCKER_HOST"
else
    echo "🐳 Using local Docker daemon"
fi

# Create external volumes if they don't exist
echo "📦 Creating Docker volumes..."
docker volume create neural_trader_timescale_data 2>/dev/null || true
docker volume create neural_trader_redis_data 2>/dev/null || true

# Stop any existing services
echo "🛑 Stopping existing services..."
docker-compose -f docker-compose.external.yml down 2>/dev/null || true

# Start only database services in Docker
echo "🗄️  Starting database services..."
docker-compose -f docker-compose.external.yml up -d timescaledb redis

# Wait for databases to be ready
echo "⏳ Waiting for databases..."
echo -n "  TimescaleDB: "
until docker-compose -f docker-compose.external.yml exec -T timescaledb pg_isready -U postgres >/dev/null 2>&1; do
    echo -n "."
    sleep 1
done
echo " ✅"

echo -n "  Redis: "
until docker-compose -f docker-compose.external.yml exec -T redis redis-cli ping >/dev/null 2>&1; do
    echo -n "."
    sleep 1
done
echo " ✅"

# Optional: Start development tools
read -p "Start development tools (Redis Commander, pgAdmin)? (y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🛠️  Starting development tools..."
    docker-compose -f docker-compose.external.yml up -d redis-commander pgadmin
fi

# Set up environment for local development
export DATABASE_URL="postgresql://postgres:${POSTGRES_PASSWORD:-dev_password}@localhost:5432/neural_trader"
export REDIS_URL="redis://localhost:6379"
export LOG_LEVEL=debug
export RUST_LOG=debug

# Build the Rust application locally
echo ""
echo "🔨 Building Neural Trader locally..."
cargo build --bin neural-trader

# Install cargo-watch for hot reload if not present
if ! command_exists cargo-watch; then
    echo "📦 Installing cargo-watch for hot reload..."
    cargo install cargo-watch
fi

# Create a tmux/screen session or just run in foreground
echo ""
echo "✅ Setup Complete!"
echo "================="
echo ""
echo "📊 Service Status:"
docker-compose -f docker-compose.external.yml ps
echo ""
echo "🌐 URLs:"
echo "  - Trading API: http://localhost:3030 (will start when you run the app)"
echo "  - Redis Commander: http://localhost:8081"
echo "  - pgAdmin: http://localhost:8082"
echo "     Email: admin@neural-trader.local"
echo "     Password: admin"
echo ""
echo "🚀 Start the application with one of these commands:"
echo ""
echo "  # Run with hot reload (recommended):"
echo "  cargo watch -x 'run --bin neural-trader'"
echo ""
echo "  # Run normally:"
echo "  cargo run --bin neural-trader"
echo ""
echo "  # Run data ingestion:"
echo "  cd data_ingestion && python main.py"
echo ""
echo "💾 Resource Usage:"
docker system df
echo ""
echo "💡 Tips:"
echo "  - Services are running in Docker, app runs locally"
echo "  - This saves ~4GB by not building Rust in Docker"
echo "  - Use 'docker-compose -f docker-compose.external.yml logs -f' to view service logs"
echo "  - Run './scripts/docker_cleanup.sh' to free space"
echo ""

# Optionally start the app
read -p "Start Neural Trader now with hot reload? (y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Starting Neural Trader with hot reload..."
    echo "Press Ctrl+C to stop"
    echo ""
    cargo watch -x 'run --bin neural-trader'
fi