#!/bin/bash
# Integration Test Environment Runner
# Mirrors production stack locally for fast iteration
#
# Usage:
#   ./scripts/integration-test.sh start    # Start full stack
#   ./scripts/integration-test.sh stop     # Stop and remove
#   ./scripts/integration-test.sh logs     # Follow logs
#   ./scripts/integration-test.sh sync     # Sync configs to etcd
#   ./scripts/integration-test.sh inject   # Inject test data
#   ./scripts/integration-test.sh status   # Check service health
#   ./scripts/integration-test.sh clean    # Full cleanup with volumes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$PROJECT_ROOT/docker-compose.integration.yml"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[integration]${NC} $1"; }
warn() { echo -e "${YELLOW}[integration]${NC} $1"; }
error() { echo -e "${RED}[integration]${NC} $1"; }

start() {
    log "Starting integration environment..."

    # Build images first (fast on x86_64)
    log "Building images..."
    docker compose -f "$COMPOSE_FILE" build --parallel

    # Start services
    log "Starting services..."
    docker compose -f "$COMPOSE_FILE" up -d

    # Wait for health
    log "Waiting for services to be healthy..."
    sleep 5

    # Sync configs
    sync_configs

    log "Integration environment ready!"
    status
}

stop() {
    log "Stopping integration environment..."
    docker compose -f "$COMPOSE_FILE" down
}

clean() {
    log "Cleaning integration environment (including volumes)..."
    docker compose -f "$COMPOSE_FILE" down -v --remove-orphans
}

logs() {
    docker compose -f "$COMPOSE_FILE" logs -f "$@"
}

sync_configs() {
    log "Syncing configurations to etcd..."

    # Wait for etcd
    until docker exec integration-etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd..."
        sleep 2
    done

    # Use existing sync script
    ETCD_CONTAINER=integration-etcd \
    ETCD_ENDPOINT=http://localhost:2379 \
    CONFIG_DIR="$PROJECT_ROOT/config" \
    "$PROJECT_ROOT/scripts/sync-config-to-etcd.sh" development

    log "Config sync complete"
}

inject_test_data() {
    log "Injecting test data via MQTT..."

    # Air quality reading
    mosquitto_pub -h localhost -p 1883 \
        -t "airgradient/integration-test/measures" \
        -m '{"wifi":-45,"pm02":12,"rco2":650,"atmp":22.5,"rhum":55,"tvoc":150,"nox":25}'

    log "Sent AirGradient test message"

    # Multiple readings for time series
    for i in {1..5}; do
        co2=$((600 + RANDOM % 200))
        pm=$((10 + RANDOM % 20))
        temp=$(echo "scale=1; 20 + $RANDOM % 10 / 10" | bc)

        mosquitto_pub -h localhost -p 1883 \
            -t "airgradient/integration-test/measures" \
            -m "{\"wifi\":-45,\"pm02\":$pm,\"rco2\":$co2,\"atmp\":$temp,\"rhum\":55}"

        sleep 1
    done

    log "Injected 5 test readings"
}

status() {
    log "Service Status:"
    echo ""
    docker compose -f "$COMPOSE_FILE" ps --format "table {{.Name}}\t{{.Status}}\t{{.Ports}}"
    echo ""

    # Check specific endpoints
    log "Health Checks:"

    # etcd
    if docker exec integration-etcd etcdctl endpoint health >/dev/null 2>&1; then
        echo -e "  etcd:        ${GREEN}healthy${NC}"
    else
        echo -e "  etcd:        ${RED}unhealthy${NC}"
    fi

    # TimescaleDB
    if docker exec integration-timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; then
        echo -e "  timescaledb: ${GREEN}healthy${NC}"
    else
        echo -e "  timescaledb: ${RED}unhealthy${NC}"
    fi

    # Air quality app
    if curl -sf http://localhost:8080/health >/dev/null 2>&1; then
        echo -e "  air-quality: ${GREEN}healthy${NC} (http://localhost:8080)"
    else
        echo -e "  air-quality: ${RED}unhealthy${NC}"
    fi

    # MQTT
    if mosquitto_sub -h localhost -t '$SYS/#' -C 1 -W 2 >/dev/null 2>&1; then
        echo -e "  mqtt:        ${GREEN}healthy${NC} (localhost:1883)"
    else
        echo -e "  mqtt:        ${RED}unhealthy${NC}"
    fi

    echo ""
}

query_silver() {
    log "Querying Silver layer..."
    docker exec integration-timescaledb psql -U postgres -d ndp -c "
        SELECT
            table_name,
            (SELECT COUNT(*) FROM silver.air_quality_observations) as air_quality_rows,
            (SELECT COUNT(*) FROM silver.weather_observations) as weather_rows
        FROM information_schema.tables
        WHERE table_schema = 'silver'
        LIMIT 1;
    " 2>/dev/null || warn "Silver tables may not exist yet"
}

# Main command router
case "${1:-start}" in
    start)
        start
        ;;
    stop)
        stop
        ;;
    clean)
        clean
        ;;
    logs)
        shift
        logs "$@"
        ;;
    sync)
        sync_configs
        ;;
    inject)
        inject_test_data
        ;;
    status)
        status
        ;;
    query)
        query_silver
        ;;
    *)
        echo "Usage: $0 {start|stop|clean|logs|sync|inject|status|query}"
        exit 1
        ;;
esac
