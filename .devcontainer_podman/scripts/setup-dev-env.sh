#!/bin/bash
# Setup development environment for Podman-based Neural Trader

set -e

echo "🚀 Setting up Neural Trader development environment with Podman..."

# Create necessary directories
mkdir -p ~/.local/share/containers/storage
mkdir -p ~/.config/containers
mkdir -p ~/workspace/.cargo-cache
mkdir -p ~/workspace/.rustup-cache

# Setup Podman rootless
if ! podman system info > /dev/null 2>&1; then
    echo "📦 Initializing Podman rootless setup..."
    podman system migrate
    podman system reset -f
fi

# Pull required images
echo "🐳 Pulling required container images..."
podman pull docker.io/timescale/timescaledb:latest-pg16
podman pull docker.io/redis:7-alpine
podman pull docker.io/prom/prometheus:latest
podman pull docker.io/grafana/grafana:latest
podman pull docker.io/dpage/pgadmin4:latest
podman pull docker.io/rediscommander/redis-commander:latest

# Create Podman network
echo "🌐 Creating Podman network..."
podman network create neural-trader-net 2>/dev/null || true

# Create volumes
echo "💾 Creating persistent volumes..."
podman volume create neural-trader-postgres-data
podman volume create neural-trader-redis-data
podman volume create neural-trader-prometheus-data
podman volume create neural-trader-grafana-data

# Install additional Python packages
echo "🐍 Installing Python development packages..."
pip3 install --user -r /workspace/data_ingestion/requirements.txt || true

# Setup Rust environment
echo "🦀 Setting up Rust development environment..."
cd /workspace
cargo fetch || true

# Create environment file if it doesn't exist
if [ ! -f /workspace/.env ]; then
    echo "📝 Creating default .env file..."
    cp /workspace/.env.example /workspace/.env 2>/dev/null || cat > /workspace/.env << 'EOF'
# Development Environment Configuration
DATABASE_URL=postgresql://postgres:dev_password@localhost:5432/neural_trader
REDIS_URL=redis://localhost:6379
RUST_LOG=info
LOG_LEVEL=debug

# API Keys (set your own)
ALPHA_VANTAGE_API_KEY=
POLYGON_API_KEY=
FINNHUB_API_KEY=
IEX_CLOUD_API_KEY=
EOF
fi

# Setup git configuration for the workspace
git config --global --add safe.directory /workspace

echo "✅ Development environment setup complete!"
echo ""
echo "📋 Next steps:"
echo "  1. Set your API keys in the .env file"
echo "  2. Run 'bash .devcontainer_podman/scripts/start-services.sh' to start services"
echo "  3. Run 'cargo run' to start the Neural Trader application"