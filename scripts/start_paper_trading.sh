#!/bin/bash

# Neural Trader Paper Trading Startup Script
# Starts paper trading with $10,000 capital

set -e

echo "🚀 Starting Neural Trader Paper Trading Setup..."

# Check if .env.minimal exists
if [ ! -f .env.minimal ]; then
    echo "📝 Creating .env.minimal with paper trading configuration..."
    cat > .env.minimal << EOF
# Paper Trading Configuration
TRADING_MODE=paper
INITIAL_CAPITAL=10000
RUST_LOG=info

# Database Configuration
DATABASE_URL=postgresql://neural_trader:neural_trader_password@localhost:5432/neural_trader_db
REDIS_URL=redis://:neural_trader_redis_password@localhost:6379

# Add your data provider API key here
# ALPHA_VANTAGE_API_KEY=demo
# IEX_API_KEY=your_key_here
EOF
fi

# Check for API keys
if ! grep -q "API_KEY=" .env.minimal || grep -q "API_KEY=demo" .env.minimal || grep -q "API_KEY=your_key_here" .env.minimal; then
    echo "⚠️  WARNING: No valid API key found in .env.minimal"
    echo "Please add at least one data provider API key:"
    echo "  - Alpha Vantage: https://www.alphavantage.co/support/#api-key"
    echo "  - IEX Cloud: https://iexcloud.io/"
    echo ""
    read -p "Do you want to continue with demo mode? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Start Docker services
echo "🐳 Starting Docker services..."
docker-compose -f docker-compose.simple.yml up -d

# Wait for services
echo "⏳ Waiting for services to start..."
sleep 10

# Check service health
echo "🏥 Checking service health..."
docker ps | grep -E "(timescaledb|redis)" || {
    echo "❌ Docker services failed to start"
    exit 1
}

# Build the project
echo "🔨 Building neural-trader..."
cargo build --release

# Create paper trading config if not exists
if [ ! -f config/paper_trading.yaml ]; then
    echo "📝 Creating paper trading configuration..."
    mkdir -p config
    cat > config/paper_trading.yaml << EOF
paper_trading:
  initial_capital: 10000.00
  currency: USD
  
  risk_management:
    max_position_size_pct: 0.20
    max_total_exposure_pct: 1.00
    stop_loss_pct: 0.02
    take_profit_pct: 0.05
    max_daily_drawdown_pct: 0.05
  
  execution:
    enable_slippage: true
    slippage_bps: 10
    commission_pct: 0.001
    
  strategies:
    - name: momentum
      allocation_pct: 1.00
EOF
fi

# Run paper trading
echo "💰 Starting paper trading with $10,000 capital..."
echo "📊 Monitoring BTC/USD and ETH/USD..."
echo "🎯 Using momentum strategy..."
echo ""
echo "Press Ctrl+C to stop"
echo ""

# Set environment variables and run
export TRADING_MODE=paper
export INITIAL_CAPITAL=10000
export RUST_LOG=info

# Source the .env.minimal file
set -a
source .env.minimal
set +a

# Run the trading platform
cargo run --release -- \
    --mode paper \
    --capital 10000 \
    --strategy momentum \
    --symbols "BTC/USD,ETH/USD" \
    --config config/paper_trading.yaml