#!/bin/bash
# Start all services using Podman pods

set -e

echo "🚀 Starting Neural Trader services with Podman..."

# Source environment variables
if [ -f /workspace/.env ]; then
    export $(grep -v '^#' /workspace/.env | xargs)
fi

# Create pods for service grouping
echo "🔸 Creating Podman pods..."

# Database pod (PostgreSQL + pgAdmin)
podman pod create \
    --name neural-trader-db-pod \
    --publish 5432:5432 \
    --publish 8082:80 \
    --network neural-trader-net \
    2>/dev/null || true

# Cache pod (Redis + Redis Commander)
podman pod create \
    --name neural-trader-cache-pod \
    --publish 6379:6379 \
    --publish 8081:8081 \
    --network neural-trader-net \
    2>/dev/null || true

# Monitoring pod (Prometheus + Grafana)
podman pod create \
    --name neural-trader-monitoring-pod \
    --publish 9090:9090 \
    --publish 3000:3000 \
    --network neural-trader-net \
    2>/dev/null || true

# Start TimescaleDB
echo "🐘 Starting TimescaleDB..."
podman run -d \
    --pod neural-trader-db-pod \
    --name timescaledb \
    --env POSTGRES_USER=postgres \
    --env POSTGRES_PASSWORD=dev_password \
    --env POSTGRES_DB=neural_trader \
    --env TIMESCALEDB_TELEMETRY=off \
    --volume neural-trader-postgres-data:/var/lib/postgresql/data:Z \
    --volume /workspace/docker/timescaledb/init-scripts:/docker-entrypoint-initdb.d:ro,Z \
    --health-cmd "pg_isready -U postgres" \
    --health-interval 5s \
    --health-timeout 5s \
    --health-retries 5 \
    docker.io/timescale/timescaledb:latest-pg16

# Start pgAdmin
echo "🎛️  Starting pgAdmin..."
podman run -d \
    --pod neural-trader-db-pod \
    --name pgadmin \
    --env PGADMIN_DEFAULT_EMAIL=admin@neural-trader.local \
    --env PGADMIN_DEFAULT_PASSWORD=admin \
    --env PGADMIN_CONFIG_SERVER_MODE=False \
    docker.io/dpage/pgadmin4:latest

# Start Redis
echo "🔴 Starting Redis..."
podman run -d \
    --pod neural-trader-cache-pod \
    --name redis \
    --volume neural-trader-redis-data:/data:Z \
    --health-cmd "redis-cli ping" \
    --health-interval 5s \
    --health-timeout 3s \
    --health-retries 5 \
    docker.io/redis:7-alpine \
    redis-server --appendonly yes

# Start Redis Commander
echo "🎛️  Starting Redis Commander..."
podman run -d \
    --pod neural-trader-cache-pod \
    --name redis-commander \
    --env REDIS_HOSTS=local:localhost:6379 \
    docker.io/rediscommander/redis-commander:latest

# Wait for databases to be ready
echo "⏳ Waiting for databases to be ready..."
for i in {1..30}; do
    if podman exec timescaledb pg_isready -U postgres >/dev/null 2>&1 && \
       podman exec redis redis-cli ping >/dev/null 2>&1; then
        echo "✅ Databases are ready!"
        break
    fi
    echo -n "."
    sleep 2
done

# Show status
echo ""
echo "📊 Service Status:"
podman pod ps
podman ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "✅ All services started!"
echo ""
echo "🌐 Service URLs:"
echo "  PostgreSQL:      localhost:5432 (user: postgres, pass: dev_password)"
echo "  Redis:           localhost:6379"
echo "  pgAdmin:         http://localhost:8082"
echo "  Redis Commander: http://localhost:8081"
echo ""
echo "💡 Tips:"
echo "  - View logs: podman logs <container-name>"
echo "  - Stop all: bash .devcontainer_podman/scripts/stop-services.sh"
echo "  - Access DB: podman exec -it timescaledb psql -U postgres -d neural_trader"