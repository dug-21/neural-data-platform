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
cd "$SCRIPT_DIR"

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
    cd ../..
    if [ -f scripts/sync-config-to-etcd.sh ]; then
        ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production
    else
        warn "Config sync script not found, skipping"
    fi
    cd "$SCRIPT_DIR"
}

build() {
    log "Building Docker images (this may take 15-30 minutes on first run)..."
    docker compose build --progress=plain
    log "Build complete"
}

start() {
    log "Starting services..."
    docker compose up -d

    log "Waiting for services to be healthy..."
    sleep 10

    # Sync config after services are up
    sync_config

    log "Services started successfully!"
    status
}

stop() {
    log "Stopping services..."
    docker compose down
    log "Services stopped"
}

logs() {
    docker compose logs -f
}

status() {
    echo ""
    log "Service Status:"
    docker compose ps
    echo ""

    log "Health Checks:"
    echo "  MQTT Broker: $(curl -s -o /dev/null -w '%{http_code}' http://localhost:1883 2>/dev/null || echo 'N/A (TCP only)')"
    echo "  etcd:        $(docker exec etcd etcdctl endpoint health 2>/dev/null || echo 'Not running')"
    echo "  Air Quality: $(curl -s http://localhost:8080/health 2>/dev/null || echo 'Not running')"
    echo ""

    log "Data Volume:"
    docker exec air-quality-app du -sh /data 2>/dev/null || echo "  Not available"
    echo ""

    log "Useful URLs:"
    PI_IP=$(hostname -I | awk '{print $1}')
    echo "  Air Quality API: http://${PI_IP}:8080"
    echo "  Metrics:         http://${PI_IP}:9090"
    echo "  MQTT Broker:     mqtt://${PI_IP}:1883"
}

update() {
    log "Updating deployment..."

    # Pull latest code
    cd ../..
    git pull origin main || git pull origin feature/air-001-implementation
    cd "$SCRIPT_DIR"

    # Rebuild and restart
    build
    docker compose up -d
    sync_config

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
    *)
        echo "Usage: $0 {deploy|start|stop|logs|status|update|build|sync}"
        exit 1
        ;;
esac
