#!/bin/bash

# Start services natively without Docker

set -e

echo "📈 Starting Neural Trader Services (Native - No Docker)"
echo "===================================================="

# Load environment
source .env.stock-simulation

# Install PostgreSQL and Redis if needed
if ! command -v psql &> /dev/null; then
    echo "📦 Installing PostgreSQL..."
    sudo apt-get update
    sudo apt-get install -y postgresql postgresql-contrib
fi

if ! command -v redis-server &> /dev/null; then
    echo "📦 Installing Redis..."
    sudo apt-get install -y redis-server
fi

# Start PostgreSQL
echo "🐘 Starting PostgreSQL..."
sudo service postgresql start

# Create database and user
sudo -u postgres psql <<EOF
CREATE USER neural_trader WITH PASSWORD 'dev_password';
CREATE DATABASE neural_trader_db OWNER neural_trader;
GRANT ALL PRIVILEGES ON DATABASE neural_trader_db TO neural_trader;
EOF

# Start Redis
echo "🔴 Starting Redis..."
redis-server --daemonize yes

# Install Python dependencies for data ingestion
echo "🐍 Setting up Python environment..."
cd data_ingestion
pip install -r requirements.txt

# Start data ingestion service
echo "📊 Starting Data Ingestion Service..."
python main.py &

echo ""
echo "✅ Services Started!"
echo "==================="
echo ""
echo "🌐 Service Endpoints:"
echo "  - PostgreSQL: localhost:5432"
echo "  - Redis: localhost:6379"
echo "  - Data Ingestion: http://localhost:8001"
echo ""
echo "To stop services:"
echo "  sudo service postgresql stop"
echo "  redis-cli shutdown"
echo "  pkill -f 'python main.py'"