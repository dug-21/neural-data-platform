#!/bin/bash
# Deployment script for production

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check if .env exists
if [ ! -f "$SCRIPT_DIR/.env" ]; then
    echo -e "${RED}Error: .env file not found!${NC}"
    echo "Copy .env.template to .env and configure your settings:"
    echo "  cp .env.template .env"
    exit 1
fi

# Load environment variables
export $(grep -v '^#' .env | xargs)

echo -e "${GREEN}Deploying Neural Trader production stack...${NC}"

# Ensure images are built
echo -e "${YELLOW}Checking for required images...${NC}"
IMAGES=("neural-trader:prod" "neural-trader/timescaledb:prod" "neural-trader/prometheus:prod" "neural-trader/grafana:prod" "neural-trader/data-ingestion:prod")
MISSING=0

for img in "${IMAGES[@]}"; do
    if ! docker image inspect "$img" >/dev/null 2>&1; then
        echo -e "${RED}Missing image: $img${NC}"
        MISSING=1
    fi
done

if [ $MISSING -eq 1 ]; then
    echo -e "${YELLOW}Building missing images...${NC}"
    ./build.sh
fi

# Start services
echo -e "${YELLOW}Starting services...${NC}"
docker-compose -f docker-compose.prod.yml up -d

# Wait for services to be healthy
echo -e "${YELLOW}Waiting for services to be healthy...${NC}"
sleep 10

# Check service health
echo -e "${YELLOW}Checking service health...${NC}"
docker-compose -f docker-compose.prod.yml ps

# Show access URLs
echo -e "${GREEN}Services deployed successfully!${NC}"
echo ""
echo "Access URLs:"
echo "  Neural Trader API: http://localhost:8080"
echo "  Data Ingestion API: http://localhost:8001"
echo "  Data Ingestion Metrics: http://localhost:9091"
echo "  Prometheus: http://localhost:9090"
echo "  Grafana: http://localhost:3000 (admin/${GRAFANA_PASSWORD})"
echo "  TimescaleDB: localhost:5432 (database: neural_trader)"
echo ""
echo "To view logs: docker-compose -f docker-compose.prod.yml logs -f"
echo "To stop: docker-compose -f docker-compose.prod.yml down"