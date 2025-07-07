#!/bin/bash
# Podman shutdown script for Neural Trader
# This script stops and optionally removes all pods and containers

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
REMOVE_VOLUMES=false
REMOVE_IMAGES=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --remove-volumes)
            REMOVE_VOLUMES=true
            shift
            ;;
        --remove-images)
            REMOVE_IMAGES=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --remove-volumes  Remove all volumes (data will be lost!)"
            echo "  --remove-images   Remove all built images"
            echo "  --help           Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}Stopping Neural Trader...${NC}"

# Function to stop and remove a pod
stop_pod() {
    local pod_name=$1
    
    if podman pod exists "${pod_name}" 2>/dev/null; then
        echo -e "${BLUE}Stopping pod: ${pod_name}...${NC}"
        podman pod stop "${pod_name}" 2>/dev/null || true
        podman pod rm "${pod_name}" 2>/dev/null || true
    else
        echo -e "${YELLOW}Pod ${pod_name} not found, skipping...${NC}"
    fi
}

# Stop all pods
for pod in neural-trader-monitoring neural-trader-app neural-trader-cache neural-trader-db; do
    stop_pod "${pod}"
done

# Clean up any remaining containers with our label
echo -e "${BLUE}Cleaning up remaining containers...${NC}"
podman ps -a --filter "label=app=neural-trader" -q | xargs -r podman rm -f 2>/dev/null || true

# Remove volumes if requested
if [[ "${REMOVE_VOLUMES}" == "true" ]]; then
    echo -e "${YELLOW}Removing volumes (all data will be lost!)...${NC}"
    read -p "Are you sure? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        for volume in timescale-data timescale-logs redis-data ingestion-logs ingestion-cache trader-logs prometheus-data grafana-data pgadmin-data; do
            podman volume exists "neural-trader-${volume}" 2>/dev/null && \
                podman volume rm "neural-trader-${volume}" || true
        done
        echo -e "${GREEN}Volumes removed${NC}"
    else
        echo -e "${BLUE}Skipping volume removal${NC}"
    fi
fi

# Remove images if requested
if [[ "${REMOVE_IMAGES}" == "true" ]]; then
    echo -e "${YELLOW}Removing images...${NC}"
    for image in neural-trader-timescaledb neural-trader-redis neural-trader-data-ingestion neural-trader; do
        podman image exists "localhost/${image}:latest" 2>/dev/null && \
            podman rmi "localhost/${image}:latest" || true
    done
    echo -e "${GREEN}Images removed${NC}"
fi

# Remove secrets
echo -e "${BLUE}Cleaning up secrets...${NC}"
for secret in neural-trader-secrets neural-trader-api-keys redis-config prometheus-config; do
    podman secret exists "${secret}" 2>/dev/null && \
        podman secret rm "${secret}" || true
done

# Remove network (only if no containers are using it)
echo -e "${BLUE}Removing network...${NC}"
podman network exists neural-trader-net 2>/dev/null && \
    podman network rm neural-trader-net 2>/dev/null || true

# Clean up state directory
rm -rf "${SCRIPT_DIR}/../state"

echo -e "${GREEN}Neural Trader has been stopped${NC}"

# Show remaining resources
echo -e "${BLUE}Remaining Podman resources:${NC}"
echo -e "${BLUE}Pods:${NC}"
podman pod ls
echo -e "${BLUE}Containers:${NC}"
podman ps -a
echo -e "${BLUE}Volumes:${NC}"
podman volume ls | grep neural-trader || echo "  None"