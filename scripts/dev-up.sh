#!/bin/bash
# Start development environment
# Usage: ./scripts/dev-up.sh [--monitoring]

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration
COMPOSE_FILE="docker-compose.yml"
PROJECT_NAME="neural-air-quality"

echo -e "${GREEN}=== Starting Development Environment ===${NC}"
echo ""

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo -e "${RED}Error: docker-compose is not available${NC}"
    exit 1
fi

# Use docker compose (new) or docker-compose (legacy)
DOCKER_COMPOSE="docker compose"
if ! docker compose version &> /dev/null; then
    DOCKER_COMPOSE="docker-compose"
fi

# Parse arguments
PROFILES=""
if [ "${1:-}" = "--monitoring" ]; then
    PROFILES="--profile monitoring"
    echo -e "${YELLOW}Starting with monitoring stack (Prometheus + Grafana)${NC}"
    echo ""
fi

# Create necessary directories
echo "Creating directories..."
mkdir -p mosquitto/config mosquitto/data mosquitto/log
mkdir -p config/base config/overlays/development config/overlays/production
mkdir -p data/logs models

# Set permissions
chmod -R 755 mosquitto/config
chmod -R 777 mosquitto/data mosquitto/log
chmod -R 777 data

# Build and start services
echo -e "${GREEN}Building and starting services...${NC}"
$DOCKER_COMPOSE -p "${PROJECT_NAME}" -f "${COMPOSE_FILE}" ${PROFILES} up -d --build

# Wait for services to be healthy
echo ""
echo "Waiting for services to be healthy..."
sleep 5

# Check service status
echo ""
echo -e "${GREEN}Service Status:${NC}"
$DOCKER_COMPOSE -p "${PROJECT_NAME}" -f "${COMPOSE_FILE}" ps

echo ""
echo -e "${GREEN}✓ Development environment is running!${NC}"
echo ""
echo "Services:"
echo "  - MQTT Broker:      mqtt://localhost:1883"
echo "  - Air Quality API:  http://localhost:8080"
echo "  - Metrics:          http://localhost:9090/metrics"
echo "  - Health Check:     http://localhost:8080/health"

if [ "${1:-}" = "--monitoring" ]; then
    echo "  - Prometheus:       http://localhost:9091"
    echo "  - Grafana:          http://localhost:3000 (admin/admin)"
fi

echo ""
echo "Useful commands:"
echo "  - View logs:        $DOCKER_COMPOSE -p ${PROJECT_NAME} logs -f"
echo "  - View app logs:    $DOCKER_COMPOSE -p ${PROJECT_NAME} logs -f air-quality-app"
echo "  - Stop services:    ./scripts/dev-down.sh"
echo "  - Restart:          $DOCKER_COMPOSE -p ${PROJECT_NAME} restart air-quality-app"
echo ""
echo "Test MQTT:"
echo "  - Subscribe:        mosquitto_sub -h localhost -t 'airgradient/+/measures'"
echo "  - Publish:          mosquitto_pub -h localhost -t 'airgradient/test/measures' -m '{\"pm02\":12.5}'"
echo ""
