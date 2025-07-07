#!/bin/bash

# Neural Trader Development Stack - Simplified Setup
# Uses pre-built images for faster startup

set -e

echo "📈 Neural Trader Development Stack Startup"
echo "========================================="

# Check if .env.stock-simulation exists
if [ ! -f .env.stock-simulation ]; then
    echo "❌ Error: .env.stock-simulation not found!"
    echo "Please ensure .env.stock-simulation exists with your configuration."
    exit 1
fi

# Export environment variables
echo "📋 Loading stock trading configuration..."
set -a
source .env.stock-simulation
set +a

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
docker-compose -f docker-compose.dev.yml down 2>/dev/null || true

# Start the dev stack
echo "🚀 Starting development stack..."
echo "  - TimescaleDB (PostgreSQL + TimescaleDB)"
echo "  - Redis (in-memory cache)"
echo "  - Data Ingestion Service"
echo "  - Redis Commander (Redis UI)"
echo "  - pgAdmin (PostgreSQL UI)"
echo ""

docker-compose -f docker-compose.dev.yml up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be ready..."
sleep 15

# Check service health
echo "🏥 Checking service health..."
docker-compose -f docker-compose.dev.yml ps

echo ""
echo "✅ Development Stack Started!"
echo "============================"
echo ""
echo "🌐 Service URLs:"
echo "  - Redis Commander: http://localhost:8081"
echo "  - pgAdmin: http://localhost:8082"
echo "    Email: admin@neural-trader.local"
echo "    Password: admin"
echo "  - PostgreSQL: localhost:5432"
echo "    Database: neural_trader"
echo "    User: postgres"
echo "    Password: dev_password"
echo "  - Redis: localhost:6379"
echo ""
echo "📝 Useful Commands:"
echo "  - View logs: docker-compose -f docker-compose.dev.yml logs -f"
echo "  - Stop services: docker-compose -f docker-compose.dev.yml down"
echo "  - View specific service: docker-compose -f docker-compose.dev.yml logs -f [service-name]"
echo ""
echo "💡 Note: This is the development stack with simplified configuration."
echo "For production, use the full docker-compose.yml with security features."