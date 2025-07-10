#!/bin/bash
# Simple host deployment script

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}Neural Trader - Host Deployment${NC}"

# Check if .env exists
if [ ! -f ".env" ]; then
    echo -e "${YELLOW}Creating .env from template...${NC}"
    if [ -f ".env.template" ]; then
        cp .env.template .env
        echo -e "${RED}Please edit .env file with your API keys before continuing${NC}"
        exit 1
    else
        echo -e "${RED}No .env.template found. Please create .env file manually${NC}"
        exit 1
    fi
fi

# Load environment variables
export $(grep -v '^#' .env | xargs)

# Check if images exist
echo -e "${YELLOW}Checking for required images...${NC}"
IMAGES=("neural-trader:prod" "neural-trader/data-ingestion:prod" "neural-trader/timescaledb:prod" "neural-trader/prometheus:prod" "neural-trader/grafana:prod")
MISSING=0

for img in "${IMAGES[@]}"; do
    if ! docker image inspect "$img" >/dev/null 2>&1; then
        echo -e "${RED}Missing image: $img${NC}"
        echo "Run: docker load -i /path/to/your/saved/images.tar"
        MISSING=1
    fi
done

if [ $MISSING -eq 1 ]; then
    echo -e "${RED}Load images first, then run this script again${NC}"
    exit 1
fi

# Stop any existing containers
echo -e "${YELLOW}Stopping existing containers...${NC}"
docker-compose -f docker-compose.prod.yml down 2>/dev/null || true

# Start services
echo -e "${YELLOW}Starting services...${NC}"
docker-compose -f docker-compose.prod.yml up -d

# Wait for services
echo -e "${YELLOW}Waiting for services to start...${NC}"
sleep 15

# Check health
echo -e "${YELLOW}Checking service health...${NC}"
docker-compose -f docker-compose.prod.yml ps

echo -e "${GREEN}Deployment complete!${NC}"
echo ""
echo "Access URLs:"
echo "  Neural Trader API: http://localhost:8080"
echo "  Data Ingestion API: http://localhost:8001"
echo "  Data Ingestion Metrics: http://localhost:9091"
echo "  Prometheus: http://localhost:9090"
echo "  Grafana: http://localhost:3000 (admin/\${GRAFANA_PASSWORD})"
echo "  TimescaleDB: localhost:5432 (database: neural_trader)"
echo ""
echo "Commands:"
echo "  View logs: docker-compose -f docker-compose.prod.yml logs -f"
echo "  Stop all: docker-compose -f docker-compose.prod.yml down"