#!/bin/bash
# Neural Data Platform - Pi Deployment Script
# Raspberry Pi 5 with Ubuntu 25.04
#
# Usage: ./deploy.sh [command]
#   ./deploy.sh         - Full deploy (build + start)
#   ./deploy.sh start   - Start services
#   ./deploy.sh stop    - Stop services
#   ./deploy.sh logs    - View logs
#   ./deploy.sh status  - Check status
#   ./deploy.sh update  - Pull latest and rebuild

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Helper to run docker compose with the correct file
dc() {
    docker compose -f "$COMPOSE_FILE" "$@"
}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[DEPLOY]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

check_prereqs() {
    log "Checking prerequisites..."
    command -v docker >/dev/null 2>&1 || error "Docker not installed"
    command -v docker compose >/dev/null 2>&1 || error "Docker Compose not installed"
    log "Prerequisites OK"
}

sync_config() {
    log "Syncing configuration to etcd..."

    # Wait for etcd to be ready
    until docker exec etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd to be ready..."
        sleep 2
    done

    # Run the sync script from the repo root
    if [ -f "$REPO_ROOT/scripts/sync-config-to-etcd.sh" ]; then
        ETCD_CONTAINER=etcd "$REPO_ROOT/scripts/sync-config-to-etcd.sh" production
    else
        warn "Config sync script not found, skipping"
    fi
}

init_streams() {
    log "Initializing stream configurations..."

    # Wait for etcd to be ready
    until docker exec etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd to be ready..."
        sleep 2
    done

    # Check if streams are already initialized (informational only)
    if docker exec etcd etcdctl get --prefix "/air-quality/streams/" --keys-only >/dev/null 2>&1; then
        stream_count=$(docker exec etcd etcdctl get --prefix "/air-quality/streams/" --keys-only | grep -c "/id$" || echo "0")
        if [ "$stream_count" -gt 0 ]; then
            log "Updating existing stream configurations ($stream_count streams found)"
        fi
    fi

    # Run stream initialization script
    if [ -f "$SCRIPT_DIR/configs/streams/init-streams.sh" ]; then
        bash "$SCRIPT_DIR/configs/streams/init-streams.sh" etcd
    else
        warn "Stream initialization script not found at $SCRIPT_DIR/configs/streams/init-streams.sh"
        warn "Multi-stream mode enabled but no streams configured!"
    fi
}

build() {
    log "Building Docker images (this may take 15-30 minutes on first run)..."
    dc build --progress=plain
    log "Build complete"
}

start() {
    log "Starting services..."
    dc up -d

    log "Waiting for services to be healthy..."
    sleep 10

    # Sync config after services are up
    sync_config

    # Initialize stream configurations for multi-stream mode
    init_streams

    log "Services started successfully!"
    status
}

stop() {
    log "Stopping services..."
    dc down
    log "Services stopped"
}

logs() {
    dc logs -f
}

wait_for_health() {
    local service=$1
    local timeout=${2:-60}
    local elapsed=0

    log "Waiting for $service to be healthy..."

    while [ $elapsed -lt $timeout ]; do
        if dc ps "$service" 2>/dev/null | grep -q "healthy"; then
            log "$service is healthy"
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done

    warn "$service did not become healthy within ${timeout}s"
    return 1
}

status() {
    echo ""
    log "Service Status:"
    dc ps
    echo ""

    log "Health Checks:"
    echo "  MQTT Broker: $(curl -s -o /dev/null -w '%{http_code}' http://localhost:1883 2>/dev/null || echo 'N/A (TCP only)')"
    echo "  etcd:        $(docker exec etcd etcdctl endpoint health 2>/dev/null || echo 'Not running')"
    echo "  Air Quality: $(curl -s http://localhost:8080/health 2>/dev/null || echo 'Not running')"
    echo "  DuckDB:      $(docker exec duckdb test -f /var/duckdb/grafana.db 2>/dev/null && echo 'Running (SQLite export OK)' || echo 'Not running')"
    echo "  Grafana:     $(curl -s -o /dev/null -w '%{http_code}' http://localhost:3000/api/health 2>/dev/null || echo 'Not running')"
    echo ""

    log "Data Volume:"
    docker exec air-quality-app du -sh /data 2>/dev/null || echo "  Not available"
    echo ""

    log "Stream Status:"
    if [ -f "$SCRIPT_DIR/configs/streams/list-streams.sh" ]; then
        bash "$SCRIPT_DIR/configs/streams/list-streams.sh" etcd 2>/dev/null || echo "  Unable to fetch stream status"
    else
        echo "  Stream listing tool not available"
    fi
    echo ""

    log "Useful URLs:"
    PI_IP=$(hostname -I | awk '{print $1}')
    echo "  Air Quality API: http://${PI_IP}:8080"
    echo "  Grafana UI:      http://${PI_IP}:3000"
    echo "  MQTT Broker:     mqtt://${PI_IP}:1883"
    echo "  etcd:            http://${PI_IP}:2379"
}

update() {
    log "Updating deployment..."

    # Fetch latest from origin
    git -C "$REPO_ROOT" fetch origin

    # Check for local uncommitted changes to tracked files
    if ! git -C "$REPO_ROOT" diff --quiet HEAD 2>/dev/null; then
        warn "Local changes detected in tracked files:"
        git -C "$REPO_ROOT" diff --stat HEAD
        warn "These will be overwritten. Ctrl+C to abort, or wait 5 seconds to continue..."
        sleep 5
    fi

    # Reset to match origin exactly (safe: only affects tracked files, not data volumes)
    log "Syncing to origin/main..."
    git -C "$REPO_ROOT" reset --hard origin/main

    # Rebuild and restart
    build
    dc up -d
    sync_config
    init_streams

    log "Update complete!"
    status
}

# Main
case "${1:-deploy}" in
    deploy)
        check_prereqs
        build
        start
        ;;
    start)
        check_prereqs
        start
        ;;
    stop)
        stop
        ;;
    logs)
        logs
        ;;
    status)
        status
        ;;
    update)
        check_prereqs
        update
        ;;
    build)
        check_prereqs
        build
        ;;
    sync)
        sync_config
        ;;
    init-streams)
        init_streams
        ;;
    list-streams)
        if [ -f "$SCRIPT_DIR/configs/streams/list-streams.sh" ]; then
            bash "$SCRIPT_DIR/configs/streams/list-streams.sh" etcd
        else
            error "Stream listing script not found"
        fi
        ;;
    analytics)
        log "Starting analytics stack (DuckDB + Grafana)..."
        dc up -d duckdb
        wait_for_health duckdb 60
        dc up -d grafana
        wait_for_health grafana 60
        log "Analytics stack started"
        status
        ;;
    rollback)
        log "Rolling back analytics stack..."
        dc stop grafana duckdb
        dc rm -f grafana duckdb
        warn "DuckDB and Grafana stopped. Data volumes preserved."
        warn "To remove data: docker volume rm pi_duckdb_data pi_grafana_data"
        ;;
    *)
        echo "Usage: $0 {deploy|start|stop|logs|status|update|build|sync|init-streams|list-streams|analytics|rollback}"
        echo ""
        echo "Commands:"
        echo "  deploy       - Full deploy (build + start all services)"
        echo "  start        - Start all services"
        echo "  stop         - Stop all services"
        echo "  logs         - View logs"
        echo "  status       - Check service health and URLs"
        echo "  update       - Pull latest and rebuild"
        echo "  build        - Build Docker images"
        echo "  sync         - Sync configuration to etcd"
        echo "  init-streams - Initialize stream configurations"
        echo "  list-streams - List configured streams"
        echo "  analytics    - Start DuckDB + Grafana analytics stack"
        echo "  rollback     - Stop and remove analytics stack"
        exit 1
        ;;
esac
