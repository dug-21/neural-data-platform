#!/bin/bash
# Podman status script for Neural Trader
# This script shows the status of all pods, containers, and services

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Neural Trader Podman Status ===${NC}\n"

# Function to check service health
check_service_health() {
    local container_name=$1
    local health_cmd=$2
    
    if podman ps --filter "name=${container_name}" --format "{{.Names}}" | grep -q "${container_name}"; then
        if eval "${health_cmd}" >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Healthy${NC}"
        else
            echo -e "${YELLOW}⚠ Unhealthy${NC}"
        fi
    else
        echo -e "${RED}✗ Not running${NC}"
    fi
}

# Pod Status
echo -e "${CYAN}POD STATUS:${NC}"
echo "----------------------------------------"
podman pod ps --format "table {{.Name}}\t{{.Status}}\t{{.Created}}\t{{.Containers}}" | \
    grep -E "(NAME|neural-trader)" || echo "No pods found"
echo

# Container Status
echo -e "${CYAN}CONTAINER STATUS:${NC}"
echo "----------------------------------------"
podman ps -a --filter "label=app=neural-trader" \
    --format "table {{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}" || echo "No containers found"
echo

# Service Health Checks
echo -e "${CYAN}SERVICE HEALTH:${NC}"
echo "----------------------------------------"

# Database
echo -n "TimescaleDB: "
check_service_health "neural-trader-db-timescaledb" \
    "podman exec neural-trader-db-timescaledb pg_isready -U neural_trader -d neural_trader_db"

# Redis
echo -n "Redis: "
check_service_health "neural-trader-cache-redis" \
    "podman exec neural-trader-cache-redis redis-cli ping"

# Data Ingestion
echo -n "Data Ingestion: "
check_service_health "neural-trader-app-data-ingestion" \
    "curl -sf http://localhost:8001/health"

# Neural Trader
echo -n "Neural Trader: "
check_service_health "neural-trader-app-neural-trader" \
    "curl -sf http://localhost:3030/health"

# Prometheus
echo -n "Prometheus: "
check_service_health "neural-trader-monitoring-prometheus" \
    "curl -sf http://localhost:9090/-/healthy"

# Grafana
echo -n "Grafana: "
check_service_health "neural-trader-monitoring-grafana" \
    "curl -sf http://localhost:3000/api/health"

# pgAdmin
echo -n "pgAdmin: "
check_service_health "neural-trader-db-pgadmin" \
    "curl -sf http://localhost:8082/misc/ping"

# Redis Commander
echo -n "Redis Commander: "
check_service_health "neural-trader-cache-redis-commander" \
    "curl -sf http://localhost:8081"

echo

# Volume Status
echo -e "${CYAN}VOLUME STATUS:${NC}"
echo "----------------------------------------"
for volume in timescale-data timescale-logs redis-data ingestion-logs ingestion-cache trader-logs prometheus-data grafana-data pgadmin-data; do
    if podman volume exists "neural-trader-${volume}" 2>/dev/null; then
        size=$(podman volume inspect "neural-trader-${volume}" --format '{{.Mountpoint}}' | xargs du -sh 2>/dev/null | cut -f1)
        echo -e "neural-trader-${volume}: ${GREEN}✓${NC} (${size:-N/A})"
    else
        echo -e "neural-trader-${volume}: ${RED}✗${NC}"
    fi
done
echo

# Network Status
echo -e "${CYAN}NETWORK STATUS:${NC}"
echo "----------------------------------------"
if podman network exists neural-trader-net 2>/dev/null; then
    subnet=$(podman network inspect neural-trader-net --format '{{range .Subnets}}{{.Subnet}}{{end}}')
    containers=$(podman network inspect neural-trader-net --format '{{len .Containers}}')
    echo -e "neural-trader-net: ${GREEN}✓${NC} (Subnet: ${subnet}, Containers: ${containers})"
else
    echo -e "neural-trader-net: ${RED}✗ Not found${NC}"
fi
echo

# Resource Usage
echo -e "${CYAN}RESOURCE USAGE:${NC}"
echo "----------------------------------------"
podman stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}" \
    $(podman ps --filter "label=app=neural-trader" -q) 2>/dev/null || echo "No running containers"
echo

# Recent Logs (last 5 lines from each service)
echo -e "${CYAN}RECENT LOGS (last 5 lines):${NC}"
echo "----------------------------------------"
for container in $(podman ps --filter "label=app=neural-trader" --format "{{.Names}}"); do
    echo -e "${BLUE}${container}:${NC}"
    podman logs --tail 5 "${container}" 2>&1 | sed 's/^/  /'
    echo
done

# Access URLs
echo -e "${CYAN}ACCESS URLS:${NC}"
echo "----------------------------------------"
echo "PostgreSQL/TimescaleDB: localhost:5432"
echo "Redis: localhost:6379"
echo "Neural Trader App: http://localhost:3030"
echo "Data Ingestion: http://localhost:8001/metrics"
echo "Prometheus: http://localhost:9090"
echo "Grafana: http://localhost:3000 (admin/[password])"
echo "pgAdmin: http://localhost:8082 (admin@neuraltrader.local/[password])"
echo "Redis Commander: http://localhost:8081"
echo

# Systemd Integration Status
echo -e "${CYAN}SYSTEMD INTEGRATION:${NC}"
echo "----------------------------------------"
if systemctl --user list-unit-files | grep -q "podman-neural-trader"; then
    echo -e "Systemd units: ${GREEN}✓ Generated${NC}"
    systemctl --user list-units --no-pager | grep podman-neural-trader || true
else
    echo -e "Systemd units: ${YELLOW}Not generated${NC}"
    echo "Run ./scripts/generate-systemd-units.sh to create systemd units"
fi