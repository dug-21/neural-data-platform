#!/bin/bash
# Stop development environment
# Usage: ./scripts/dev-down.sh [--clean]

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Configuration
COMPOSE_FILE="docker-compose.yml"
PROJECT_NAME="neural-air-quality"

echo -e "${YELLOW}=== Stopping Development Environment ===${NC}"
echo ""

# Use docker compose (new) or docker-compose (legacy)
DOCKER_COMPOSE="docker compose"
if ! docker compose version &> /dev/null; then
    DOCKER_COMPOSE="docker-compose"
fi

# Stop and remove containers
echo "Stopping services..."
$DOCKER_COMPOSE -p "${PROJECT_NAME}" -f "${COMPOSE_FILE}" down

# Clean volumes if requested
if [ "${1:-}" = "--clean" ]; then
    echo -e "${YELLOW}Cleaning volumes...${NC}"
    $DOCKER_COMPOSE -p "${PROJECT_NAME}" -f "${COMPOSE_FILE}" down -v

    echo -e "${YELLOW}Removing local data...${NC}"
    read -p "Are you sure you want to remove all local data? (yes/no): " confirm
    if [ "$confirm" = "yes" ]; then
        rm -rf mosquitto/data/*
        rm -rf mosquitto/log/*
        rm -rf data/*
        echo -e "${GREEN}✓ Local data removed${NC}"
    else
        echo "Skipping local data removal"
    fi
fi

echo ""
echo -e "${GREEN}✓ Development environment stopped${NC}"
echo ""
echo "To start again:"
echo "  ./scripts/dev-up.sh"
echo ""
echo "To clean volumes:"
echo "  ./scripts/dev-down.sh --clean"
echo ""
