#!/bin/bash
# Build script for production images

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}Building Neural Trader production images...${NC}"

# Check if we're in the right directory
if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
    echo -e "${RED}Error: Must run from neural-trader project root${NC}"
    exit 1
fi

# Build context is the docker/production directory
cd "$SCRIPT_DIR"

# Build images
echo -e "${YELLOW}Building neural-trader image...${NC}"
docker build --no-cache -f images/neural-trader.dockerfile -t neural-trader:prod "$PROJECT_ROOT"

echo -e "${YELLOW}Building timescaledb image...${NC}"
docker build -f images/timescaledb.dockerfile -t neural-trader/timescaledb:prod .

echo -e "${YELLOW}Building prometheus image...${NC}"
docker build -f images/prometheus.dockerfile -t neural-trader/prometheus:prod .

echo -e "${YELLOW}Building grafana image...${NC}"
docker build -f images/grafana.dockerfile  -t neural-trader/grafana:prod .

echo -e "${YELLOW}Building data-ingestion image...${NC}"
docker build -f images/data-ingestion.dockerfile --no-cache -t neural-trader/data-ingestion:prod "$PROJECT_ROOT"

echo -e "${GREEN}All images built successfully!${NC}"

# Optional: Tag for registry
if [ -n "$DOCKER_REGISTRY" ]; then
    echo -e "${YELLOW}Tagging images for registry: $DOCKER_REGISTRY${NC}"
    docker tag neural-trader:prod "$DOCKER_REGISTRY/neural-trader:prod"
    docker tag neural-trader/timescaledb:prod "$DOCKER_REGISTRY/neural-trader/timescaledb:prod"
    docker tag neural-trader/prometheus:prod "$DOCKER_REGISTRY/neural-trader/prometheus:prod"
    docker tag neural-trader/grafana:prod "$DOCKER_REGISTRY/neural-trader/grafana:prod"
    docker tag neural-trader/data-ingestion:prod "$DOCKER_REGISTRY/neural-trader/data-ingestion:prod"
fi

echo -e "${GREEN}Build complete!${NC}"
echo -e "To run: cd $SCRIPT_DIR && docker-compose -f docker-compose.prod.yml up -d"