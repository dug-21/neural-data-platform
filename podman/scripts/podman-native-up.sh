#!/bin/bash
# Native Podman commands to start Neural Trader without YAML files
# This demonstrates pure Podman CLI usage

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NETWORK_NAME="neural-trader-net"
NETWORK_SUBNET="172.20.0.0/16"

echo -e "${BLUE}Starting Neural Trader with native Podman commands...${NC}"

# Create network
echo -e "${BLUE}Creating network...${NC}"
podman network exists "${NETWORK_NAME}" || \
    podman network create \
        --driver bridge \
        --subnet "${NETWORK_SUBNET}" \
        "${NETWORK_NAME}"

# Create volumes
echo -e "${BLUE}Creating volumes...${NC}"
for volume in timescale-data timescale-logs redis-data ingestion-logs ingestion-cache trader-logs prometheus-data grafana-data pgadmin-data; do
    podman volume create "neural-trader-${volume}" 2>/dev/null || true
done

# Create secrets (if not already created)
"${SCRIPT_DIR}/create-secrets.sh"

# Create Database Pod
echo -e "${BLUE}Creating database pod...${NC}"
podman pod create \
    --name neural-trader-db \
    --network "${NETWORK_NAME}" \
    --publish 5432:5432 \
    --publish 8082:8082

# Start TimescaleDB
echo -e "${BLUE}Starting TimescaleDB...${NC}"
podman run -d \
    --name neural-trader-timescaledb \
    --pod neural-trader-db \
    --secret neural-trader-secrets,type=env,target=POSTGRES_PASSWORD \
    --secret neural-trader-secrets,type=env,target=POSTGRES_USER \
    -e POSTGRES_DB=neural_trader_db \
    -e POSTGRES_SHARED_BUFFERS=2GB \
    -e POSTGRES_EFFECTIVE_CACHE_SIZE=6GB \
    -e POSTGRES_MAINTENANCE_WORK_MEM=512MB \
    -e POSTGRES_WORK_MEM=128MB \
    -e POSTGRES_MAX_CONNECTIONS=200 \
    -e POSTGRES_MAX_PARALLEL_WORKERS=8 \
    -e POSTGRES_MAX_PARALLEL_WORKERS_PER_GATHER=4 \
    -v neural-trader-timescale-data:/var/lib/postgresql/data:Z \
    -v "${PROJECT_ROOT}/docker/timescaledb/init-scripts":/docker-entrypoint-initdb.d:ro,Z \
    -v neural-trader-timescale-logs:/var/log/postgresql:Z \
    --health-cmd="pg_isready -U neural_trader -d neural_trader_db" \
    --health-interval=10s \
    --health-timeout=5s \
    --health-retries=5 \
    --health-start-period=30s \
    --restart=unless-stopped \
    --memory=8g \
    --cpus=4 \
    localhost/neural-trader-timescaledb:latest

# Start pgAdmin (development)
if [[ "${PROFILE:-development}" == "development" ]]; then
    echo -e "${BLUE}Starting pgAdmin...${NC}"
    podman run -d \
        --name neural-trader-pgadmin \
        --pod neural-trader-db \
        --secret neural-trader-secrets,type=env,target=PGADMIN_DEFAULT_PASSWORD \
        -e PGADMIN_DEFAULT_EMAIL=admin@neuraltrader.local \
        -e PGADMIN_CONFIG_SERVER_MODE=False \
        -e PGADMIN_LISTEN_PORT=8082 \
        -v neural-trader-pgadmin-data:/var/lib/pgadmin:Z \
        --restart=unless-stopped \
        --memory=512m \
        --cpus=0.5 \
        docker.io/dpage/pgadmin4:latest
fi

# Create Cache Pod
echo -e "${BLUE}Creating cache pod...${NC}"
podman pod create \
    --name neural-trader-cache \
    --network "${NETWORK_NAME}" \
    --publish 6379:6379 \
    --publish 8081:8081

# Start Redis
echo -e "${BLUE}Starting Redis...${NC}"
podman run -d \
    --name neural-trader-redis \
    --pod neural-trader-cache \
    --secret neural-trader-secrets,type=env,target=REDIS_PASSWORD \
    -v neural-trader-redis-data:/data:Z \
    -v "${PROJECT_ROOT}/docker/redis/redis.conf":/usr/local/etc/redis/redis.conf:ro,Z \
    --health-cmd="redis-cli ping" \
    --health-interval=10s \
    --health-timeout=5s \
    --health-retries=5 \
    --health-start-period=20s \
    --restart=unless-stopped \
    --memory=4g \
    --cpus=2 \
    localhost/neural-trader-redis:latest \
    redis-server /usr/local/etc/redis/redis.conf --requirepass \${REDIS_PASSWORD}

# Start Redis Commander (development)
if [[ "${PROFILE:-development}" == "development" ]]; then
    echo -e "${BLUE}Starting Redis Commander...${NC}"
    podman run -d \
        --name neural-trader-redis-commander \
        --pod neural-trader-cache \
        --secret neural-trader-secrets,type=env,target=REDIS_PASSWORD \
        -e REDIS_HOSTS=local:localhost:6379:0:\${REDIS_PASSWORD} \
        --restart=unless-stopped \
        --memory=256m \
        --cpus=0.5 \
        docker.io/rediscommander/redis-commander:latest
fi

# Wait for dependencies
echo -e "${BLUE}Waiting for database and cache to be ready...${NC}"
sleep 10

# Create Application Pod
echo -e "${BLUE}Creating application pod...${NC}"
podman pod create \
    --name neural-trader-app \
    --network "${NETWORK_NAME}" \
    --publish 8001:8000 \
    --publish 3030:3030

# Start Data Ingestion
echo -e "${BLUE}Starting Data Ingestion service...${NC}"
podman run -d \
    --name neural-trader-data-ingestion \
    --pod neural-trader-app \
    --secret neural-trader-secrets,type=env \
    --secret neural-trader-api-keys,type=env \
    -e TIMESCALE_HOST=neural-trader-db \
    -e TIMESCALE_PORT=5432 \
    -e TIMESCALE_DATABASE=neural_trader_db \
    -e REDIS_HOST=neural-trader-cache \
    -e REDIS_PORT=6379 \
    -e LOG_LEVEL=INFO \
    -e PYTHONUNBUFFERED=1 \
    -e TZ=UTC \
    -v "${PROJECT_ROOT}/data_ingestion":/app/data_ingestion:ro,Z \
    -v "${PROJECT_ROOT}/config":/app/config:ro,Z \
    -v neural-trader-ingestion-logs:/app/logs:Z \
    -v neural-trader-ingestion-cache:/app/cache:Z \
    --health-cmd="curl -f http://localhost:8000/health || exit 1" \
    --health-interval=10s \
    --health-timeout=5s \
    --health-retries=5 \
    --health-start-period=30s \
    --restart=unless-stopped \
    --memory=2g \
    --cpus=2 \
    localhost/neural-trader-data-ingestion:latest

# Start Neural Trader
echo -e "${BLUE}Starting Neural Trader application...${NC}"
podman run -d \
    --name neural-trader-main \
    --pod neural-trader-app \
    --secret neural-trader-secrets,type=env \
    -e DATABASE_URL=postgresql://\${POSTGRES_USER}:\${POSTGRES_PASSWORD}@neural-trader-db:5432/neural_trader_db \
    -e REDIS_URL=redis://:\${REDIS_PASSWORD}@neural-trader-cache:6379/0 \
    -e RUST_LOG=info \
    -e RUST_BACKTRACE=1 \
    -e TZ=UTC \
    -v "${PROJECT_ROOT}/config":/app/config:ro,Z \
    -v neural-trader-trader-logs:/app/logs:Z \
    --health-cmd="curl -f http://localhost:3030/health || exit 1" \
    --health-interval=10s \
    --health-timeout=5s \
    --health-retries=5 \
    --health-start-period=30s \
    --restart=unless-stopped \
    --memory=4g \
    --cpus=4 \
    localhost/neural-trader:latest

# Create Monitoring Pod
echo -e "${BLUE}Creating monitoring pod...${NC}"
podman pod create \
    --name neural-trader-monitoring \
    --network "${NETWORK_NAME}" \
    --publish 9090:9090 \
    --publish 3000:3000

# Start Prometheus
echo -e "${BLUE}Starting Prometheus...${NC}"
podman run -d \
    --name neural-trader-prometheus \
    --pod neural-trader-monitoring \
    -v "${PROJECT_ROOT}/docker/prometheus/prometheus.yml":/etc/prometheus/prometheus.yml:ro,Z \
    -v neural-trader-prometheus-data:/prometheus:Z \
    --health-cmd="wget --no-verbose --tries=1 --spider http://localhost:9090/-/healthy || exit 1" \
    --health-interval=10s \
    --health-timeout=5s \
    --health-retries=5 \
    --health-start-period=30s \
    --restart=unless-stopped \
    --memory=1g \
    --cpus=1 \
    docker.io/prom/prometheus:latest \
    --config.file=/etc/prometheus/prometheus.yml \
    --storage.tsdb.path=/prometheus \
    --web.console.libraries=/usr/share/prometheus/console_libraries \
    --web.console.templates=/usr/share/prometheus/consoles \
    --web.enable-lifecycle \
    --storage.tsdb.retention.time=30d

# Start Grafana
echo -e "${BLUE}Starting Grafana...${NC}"
podman run -d \
    --name neural-trader-grafana \
    --pod neural-trader-monitoring \
    --secret neural-trader-secrets,type=env,target=GF_SECURITY_ADMIN_PASSWORD \
    -e GF_SECURITY_ADMIN_USER=admin \
    -e GF_USERS_ALLOW_SIGN_UP=false \
    -e GF_INSTALL_PLUGINS=grafana-clock-panel,grafana-simple-json-datasource \
    -v "${PROJECT_ROOT}/docker/grafana/provisioning":/etc/grafana/provisioning:ro,Z \
    -v "${PROJECT_ROOT}/docker/grafana/dashboards":/var/lib/grafana/dashboards:ro,Z \
    -v neural-trader-grafana-data:/var/lib/grafana:Z \
    --health-cmd="wget --no-verbose --tries=1 --spider http://localhost:3000/api/health || exit 1" \
    --health-interval=10s \
    --health-timeout=5s \
    --health-retries=5 \
    --health-start-period=30s \
    --restart=unless-stopped \
    --memory=512m \
    --cpus=1 \
    docker.io/grafana/grafana:latest

# Show status
echo -e "${GREEN}All services started!${NC}"
echo -e "${BLUE}Pod Status:${NC}"
podman pod ps

echo -e "${BLUE}Container Status:${NC}"
podman ps

echo -e "${GREEN}Services available at:${NC}"
echo "  TimescaleDB: localhost:5432"
echo "  Redis: localhost:6379"
echo "  Neural Trader: http://localhost:3030"
echo "  Data Ingestion: http://localhost:8001"
echo "  Prometheus: http://localhost:9090"
echo "  Grafana: http://localhost:3000"
[[ "${PROFILE:-development}" == "development" ]] && echo "  pgAdmin: http://localhost:8082"
[[ "${PROFILE:-development}" == "development" ]] && echo "  Redis Commander: http://localhost:8081"