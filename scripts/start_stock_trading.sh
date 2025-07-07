#!/bin/bash

# Neural Trader Stock Trading Startup Script
# Starts paper trading with $10,000 capital for US stocks

set -e

echo "📈 Starting Neural Trader Stock Trading Setup..."

# Check if .env.stock exists, otherwise create from .env.minimal
if [ ! -f .env.stock ]; then
    echo "📝 Creating .env.stock with stock trading configuration..."
    
    # Copy .env.minimal if it exists
    if [ -f .env.minimal ]; then
        cp .env.minimal .env.stock
    else
        cat > .env.stock << EOF
# Stock Trading Configuration
TRADING_MODE=paper
INITIAL_CAPITAL=10000
RUST_LOG=info

# Database Configuration
DATABASE_URL=postgresql://neural_trader:neural_trader_password@localhost:5432/neural_trader_db
REDIS_URL=redis://:neural_trader_redis_password@localhost:6379

# Data Provider API Keys (add at least one)
# Get free keys from:
# - Finnhub: https://finnhub.io/ (Best free tier - 60 calls/min)
# - Alpha Vantage: https://www.alphavantage.co/support/#api-key
# - IEX Cloud: https://iexcloud.io/ (50k messages/month)

# FINNHUB_API_KEY=your_key_here
# ALPHA_VANTAGE_API_KEY=your_key_here
# IEX_API_KEY=your_key_here
EOF
    fi
fi

# Check for stock data provider API keys
if ! grep -q -E "(FINNHUB_API_KEY|ALPHA_VANTAGE_API_KEY|IEX_API_KEY)=" .env.stock || \
   grep -q -E "(FINNHUB_API_KEY|ALPHA_VANTAGE_API_KEY|IEX_API_KEY)=your_key_here" .env.stock; then
    echo ""
    echo "⚠️  WARNING: No stock data provider API key found!"
    echo ""
    echo "To trade stocks, you need at least one API key from:"
    echo ""
    echo "1. Finnhub (Recommended - Best free tier):"
    echo "   - Visit: https://finnhub.io/"
    echo "   - Sign up for free account"
    echo "   - Get API key and add to .env.stock:"
    echo "   - FINNHUB_API_KEY=your_actual_key"
    echo ""
    echo "2. Alpha Vantage:"
    echo "   - Visit: https://www.alphavantage.co/support/#api-key"
    echo "   - Get free API key"
    echo "   - Add to .env.stock:"
    echo "   - ALPHA_VANTAGE_API_KEY=your_actual_key"
    echo ""
    echo "3. IEX Cloud:"
    echo "   - Visit: https://iexcloud.io/"
    echo "   - Sign up for free tier (50k messages/month)"
    echo "   - Add to .env.stock:"
    echo "   - IEX_API_KEY=your_actual_key"
    echo ""
    read -p "Do you want to edit .env.stock now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        ${EDITOR:-nano} .env.stock
    else
        echo "Please add an API key to .env.stock before running stock trading."
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

# Create stock trading config if not exists
if [ ! -f config/stock_trading.yaml ]; then
    echo "📝 Stock trading configuration created at config/stock_trading.yaml"
fi

# Display trading setup
echo ""
echo "💰 Starting STOCK PAPER TRADING with $10,000 capital..."
echo "📊 Monitoring stocks: AAPL, MSFT, GOOGL, AMZN, SPY, QQQ"
echo "🎯 Strategy: Momentum trading with proper risk management"
echo "⏰ Market hours: 9:35 AM - 3:45 PM ET (avoiding open/close volatility)"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# Source the stock environment file
set -a
source .env.stock
set +a

# Run the trading platform with stock configuration
cargo run --release -- \
    --mode paper \
    --capital 10000 \
    --strategy momentum \
    --symbols "AAPL,MSFT,GOOGL,AMZN,SPY,QQQ" \
    --config config/stock_trading.yaml \
    --data-provider "${PRIMARY_PROVIDER:-finnhub}"