#!/bin/bash
# Ultra-simple deployment for host

set -e

echo "🚀 Neural Trader - Quick Start"
echo "=============================="

# Create minimal .env if it doesn't exist
if [ ! -f ".env" ]; then
    echo "Creating basic .env file..."
    cat > .env << EOF
# Database
POSTGRES_PASSWORD=dev_password
TIMESCALE_PASSWORD=dev_password

# Grafana
GRAFANA_PASSWORD=admin123

# Optional API Keys (leave blank if not available)
YAHOO_API_KEY=
FINNHUB_API_KEY=
ALPHA_VANTAGE_API_KEY=
ALPHA_ADVANTAGE_API_KEY=
IEX_CLOUD_API_KEY=
POLYGON_API_KEY=
QUANDL_API_KEY=
FRED_API_KEY=
NASDAQ_API_KEY=
NEWSAPI_KEY=
REDDIT_CLIENT_ID=
REDDIT_CLIENT_SECRET=

# Trading
SYMBOLS=BTC/USD,ETH/USD,SOL/USD
UPDATE_INTERVAL=60
PRIMARY_PROVIDER=finnhub
EOF
    echo "✅ Created .env file with defaults"
fi

# Start everything
echo "🔧 Starting services..."
docker-compose -f docker-compose.prod.yml up -d

echo "⏳ Waiting for services to be ready..."
sleep 10

echo "🎉 Done! Access your services:"
echo "   Neural Trader API: http://localhost:8080"
echo "   Data Ingestion: http://localhost:8001" 
echo "   Grafana: http://localhost:3000 (admin/admin123)"
echo "   Prometheus: http://localhost:9090"
echo ""
echo "📋 Useful commands:"
echo "   View logs: docker-compose -f docker-compose.prod.yml logs -f"
echo "   Stop: docker-compose -f docker-compose.prod.yml down"