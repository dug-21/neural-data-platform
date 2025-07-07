#!/bin/bash
# Podman startup script for Neural Trader
# This script creates and starts all pods and containers

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Base directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PODMAN_DIR="${PROJECT_ROOT}/podman"

echo -e "${BLUE}Starting Neural Trader with Podman...${NC}"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check for podman
if ! command_exists podman; then
    echo -e "${RED}Error: Podman is not installed${NC}"
    exit 1
fi

# Check if running rootless
if [[ $(id -u) -eq 0 ]]; then
    echo -e "${YELLOW}Warning: Running as root. Consider using rootless podman for better security.${NC}"
fi

# Create network if it doesn't exist
echo -e "${BLUE}Creating podman network...${NC}"
podman network exists neural-trader-net 2>/dev/null || \
    podman network create \
        --driver bridge \
        --subnet 172.20.0.0/16 \
        --gateway 172.20.0.1 \
        neural-trader-net

# Create secrets if they don't exist
echo -e "${BLUE}Creating secrets...${NC}"
"${SCRIPT_DIR}/create-secrets.sh"

# Create volumes
echo -e "${BLUE}Creating volumes...${NC}"
for volume in timescale-data timescale-logs redis-data ingestion-logs ingestion-cache trader-logs prometheus-data grafana-data pgadmin-data; do
    podman volume exists "neural-trader-${volume}" 2>/dev/null || \
        podman volume create "neural-trader-${volume}"
done

# Build images if needed
echo -e "${BLUE}Building container images...${NC}"
"${SCRIPT_DIR}/build-images.sh"

# Create ConfigMaps
echo -e "${BLUE}Creating ConfigMaps...${NC}"
# Redis config
podman secret exists redis-config 2>/dev/null || \
    podman secret create redis-config "${PROJECT_ROOT}/docker/redis/redis.conf"

# Prometheus config
podman secret exists prometheus-config 2>/dev/null || \
    podman secret create prometheus-config "${PROJECT_ROOT}/docker/prometheus/prometheus.yml"

# Function to start a pod
start_pod() {
    local pod_name=$1
    local pod_file=$2
    
    echo -e "${BLUE}Starting pod: ${pod_name}...${NC}"
    
    # Check if pod exists
    if podman pod exists "${pod_name}" 2>/dev/null; then
        echo -e "${YELLOW}Pod ${pod_name} already exists. Restarting...${NC}"
        podman pod restart "${pod_name}"
    else
        # Create pod using play kube
        podman play kube \
            --network neural-trader-net \
            --configmap redis-config="${PROJECT_ROOT}/docker/redis/redis.conf" \
            --configmap prometheus-config="${PROJECT_ROOT}/docker/prometheus/prometheus.yml" \
            "${pod_file}"
    fi
}

# Start pods in order
echo -e "${BLUE}Starting pods...${NC}"

# Database pod first
start_pod "neural-trader-db" "${PODMAN_DIR}/pods/database.yml"

# Wait for database to be ready
echo -e "${BLUE}Waiting for database to be ready...${NC}"
for i in {1..30}; do
    if podman exec neural-trader-db-timescaledb pg_isready -U neural_trader -d neural_trader_db >/dev/null 2>&1; then
        echo -e "${GREEN}Database is ready!${NC}"
        break
    fi
    echo -n "."
    sleep 2
done

# Cache pod
start_pod "neural-trader-cache" "${PODMAN_DIR}/pods/cache.yml"

# Wait for Redis to be ready
echo -e "${BLUE}Waiting for Redis to be ready...${NC}"
for i in {1..30}; do
    if podman exec neural-trader-cache-redis redis-cli ping >/dev/null 2>&1; then
        echo -e "${GREEN}Redis is ready!${NC}"
        break
    fi
    echo -n "."
    sleep 2
done

# Application pod
start_pod "neural-trader-app" "${PODMAN_DIR}/pods/application.yml"

# Monitoring pod
start_pod "neural-trader-monitoring" "${PODMAN_DIR}/pods/monitoring.yml"

# Show status
echo -e "${GREEN}All pods started successfully!${NC}"
echo -e "${BLUE}Pod Status:${NC}"
podman pod ps

echo -e "${BLUE}Container Status:${NC}"
podman ps --filter "label=app=neural-trader"

# Show access information
echo -e "${GREEN}Services are available at:${NC}"
echo -e "  PostgreSQL/TimescaleDB: localhost:5432"
echo -e "  Redis: localhost:6379"
echo -e "  Neural Trader App: http://localhost:3030"
echo -e "  Data Ingestion: http://localhost:8001"
echo -e "  Prometheus: http://localhost:9090"
echo -e "  Grafana: http://localhost:3000"
echo -e "  pgAdmin: http://localhost:8082"
echo -e "  Redis Commander: http://localhost:8081"

# Save pod state for systemd generation
echo -e "${BLUE}Saving pod state...${NC}"
mkdir -p "${PODMAN_DIR}/state"
podman pod inspect neural-trader-db > "${PODMAN_DIR}/state/db-pod.json"
podman pod inspect neural-trader-cache > "${PODMAN_DIR}/state/cache-pod.json"
podman pod inspect neural-trader-app > "${PODMAN_DIR}/state/app-pod.json"
podman pod inspect neural-trader-monitoring > "${PODMAN_DIR}/state/monitoring-pod.json"

echo -e "${GREEN}Neural Trader is up and running!${NC}"
echo -e "${BLUE}To generate systemd units, run: ${SCRIPT_DIR}/generate-systemd-units.sh${NC}"