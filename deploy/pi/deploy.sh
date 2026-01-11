#!/bin/bash
# Neural Data Platform - Pi Deployment Script
# Raspberry Pi 5 with Ubuntu 25.04
#
# Usage: ./deploy.sh [command] [options]
#   ./deploy.sh            - Full deploy (build + start)
#   ./deploy.sh start      - Start services
#   ./deploy.sh stop       - Stop services
#   ./deploy.sh logs       - View logs
#   ./deploy.sh status     - Check status
#   ./deploy.sh update [--no-cache] [target]
#     Examples:
#       ./deploy.sh update              - Rebuild all (uses cache)
#       ./deploy.sh update --no-cache   - Rebuild all (no cache)
#       ./deploy.sh update silver       - Rebuild silver-etl only (uses cache)
#       ./deploy.sh update --no-cache silver - Rebuild silver-etl (no cache)

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

sync_to_data_dictionary() {
    log "Syncing Data Dictionary to TimescaleDB..."

    # Check if TimescaleDB is running
    until docker exec pi5-timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
        warn "Waiting for TimescaleDB to be ready..."
        sleep 2
    done

    local CONFIG_DIR="$REPO_ROOT/config/base/streams"
    local SQL_FILE="/tmp/data_dictionary_sync_$$.sql"

    # Helper function to extract YAML values (compatible with both Python yq and Go yq)
    # Falls back to grep/sed if yq is unavailable
    yaml_get() {
        local file="$1"
        local key="$2"
        local default="$3"
        local result=""

        if command -v yq &> /dev/null; then
            # Detect which yq variant is installed
            if yq --version 2>&1 | grep -q "mikefarah"; then
                # Go yq (mikefarah/yq)
                result=$(yq eval ".$key // \"$default\"" "$file" 2>/dev/null)
            else
                # Python yq (kislyuk/yq) - uses jq syntax
                result=$(yq -r ".$key // \"$default\"" "$file" 2>/dev/null)
            fi
        fi

        # Fallback to grep/sed if yq failed or not installed
        if [ -z "$result" ] || [ "$result" = "null" ]; then
            result=$(grep -E "^${key}:" "$file" 2>/dev/null | sed 's/^[^:]*: *//' | tr -d '"' || echo "$default")
        fi

        # Return default if still empty
        if [ -z "$result" ] || [ "$result" = "null" ]; then
            echo "$default"
        else
            echo "$result"
        fi
    }

    # Helper to get array length
    yaml_array_len() {
        local file="$1"
        local key="$2"
        local result=0

        if command -v yq &> /dev/null; then
            if yq --version 2>&1 | grep -q "mikefarah"; then
                result=$(yq eval ".$key | length" "$file" 2>/dev/null || echo "0")
            else
                result=$(yq -r ".$key | length" "$file" 2>/dev/null || echo "0")
            fi
        fi

        # Validate it's a number
        if ! [[ "$result" =~ ^[0-9]+$ ]]; then
            result=0
        fi

        echo "$result"
    }

    # Helper to get array item value
    yaml_array_get() {
        local file="$1"
        local path="$2"
        local default="$3"
        local result=""

        if command -v yq &> /dev/null; then
            if yq --version 2>&1 | grep -q "mikefarah"; then
                result=$(yq eval "$path // \"$default\"" "$file" 2>/dev/null)
            else
                result=$(yq -r "$path // \"$default\"" "$file" 2>/dev/null)
            fi
        fi

        if [ -z "$result" ] || [ "$result" = "null" ]; then
            echo "$default"
        else
            echo "$result"
        fi
    }

    # Generate SQL
    {
        echo "-- Data Dictionary Sync"
        echo "-- Generated: $(date -Iseconds)"
        echo ""
        echo "BEGIN;"
        echo ""
        echo "-- Record sync start"
        echo "INSERT INTO data_dictionary.sync_status (sync_type, status) VALUES ('full', 'running');"
        echo ""
        echo "-- Clear existing data"
        echo "DELETE FROM data_dictionary.entity_schema_attributes;"
        echo "DELETE FROM data_dictionary.entity_schemas;"
        echo "DELETE FROM data_dictionary.sources;"
        echo "DELETE FROM data_dictionary.fields;"
        echo "DELETE FROM data_dictionary.streams;"
        echo ""

        # Process each stream config
        local stream_count=0
        local schema_count=0
        for config_dir in "$CONFIG_DIR"/*/; do
            if [ -f "$config_dir/config.yaml" ]; then
                local stream_id=$(basename "$config_dir")
                local config_file="$config_dir/config.yaml"

                # Extract stream metadata using helper functions
                local description=$(yaml_get "$config_file" "description" "" | sed "s/'/''/g")
                local version=$(yaml_get "$config_file" "version" "1.0.0")
                local enabled=$(yaml_get "$config_file" "enabled" "true")
                local retention_days=$(yaml_get "$config_file" "retention_days" "90")

                echo "-- Stream: $stream_id"
                echo "INSERT INTO data_dictionary.streams (stream_id, description, version, enabled, retention_days)"
                echo "VALUES ('$stream_id', '$description', '$version', $enabled, $retention_days);"
                echo ""

                # Process entity_schemas if present
                local es_count=$(yaml_array_len "$config_file" "entity_schemas")
                if [ "$es_count" -gt 0 ]; then
                    for i in $(seq 0 $((es_count - 1))); do
                        local schema_name=$(yaml_array_get "$config_file" ".entity_schemas[$i].schema_name" "" | sed "s/'/''/g")
                        local schema_desc=$(yaml_array_get "$config_file" ".entity_schemas[$i].description" "" | sed "s/'/''/g")
                        local device_class=$(yaml_array_get "$config_file" ".entity_schemas[$i].device_class" "null")

                        if [ "$device_class" = "null" ] || [ -z "$device_class" ]; then
                            device_class="NULL"
                        else
                            device_class="'$device_class'"
                        fi

                        echo "INSERT INTO data_dictionary.entity_schemas (stream_id, schema_name, description, device_class)"
                        echo "VALUES ('$stream_id', '$schema_name', '$schema_desc', $device_class);"

                        # Process attributes
                        local attr_count=$(yaml_array_len "$config_file" "entity_schemas[$i].attributes")
                        for j in $(seq 0 $((attr_count - 1))); do
                            local attr_name=$(yaml_array_get "$config_file" ".entity_schemas[$i].attributes[$j].name" "")
                            local attr_type=$(yaml_array_get "$config_file" ".entity_schemas[$i].attributes[$j].type" "String")
                            local attr_unit=$(yaml_array_get "$config_file" ".entity_schemas[$i].attributes[$j].unit" "null")
                            local attr_desc=$(yaml_array_get "$config_file" ".entity_schemas[$i].attributes[$j].description" "" | sed "s/'/''/g")
                            local attr_nullable=$(yaml_array_get "$config_file" ".entity_schemas[$i].attributes[$j].nullable" "true")

                            if [ "$attr_unit" = "null" ] || [ -z "$attr_unit" ]; then
                                attr_unit="NULL"
                            else
                                attr_unit="'$attr_unit'"
                            fi

                            echo "INSERT INTO data_dictionary.entity_schema_attributes (schema_id, attribute_name, attribute_type, unit, description, nullable, sort_order)"
                            echo "SELECT id, '$attr_name', '$attr_type', $attr_unit, '$attr_desc', $attr_nullable, $j"
                            echo "FROM data_dictionary.entity_schemas WHERE stream_id = '$stream_id' AND schema_name = '$schema_name';"
                        done
                        echo ""
                        schema_count=$((schema_count + 1))
                    done
                fi

                stream_count=$((stream_count + 1))
            fi
        done

        echo "-- Update sync status"
        echo "UPDATE data_dictionary.sync_status"
        echo "SET completed_at = NOW(),"
        echo "    status = 'success',"
        echo "    streams_synced = (SELECT COUNT(*) FROM data_dictionary.streams),"
        echo "    schemas_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schemas),"
        echo "    attributes_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schema_attributes)"
        echo "WHERE status = 'running' AND completed_at IS NULL;"
        echo ""
        echo "COMMIT;"

    } > "$SQL_FILE"

    # Execute sync
    log "Executing sync..."
    if docker exec -i pi5-timescaledb psql -U postgres -d ndp < "$SQL_FILE" > /dev/null 2>&1; then
        log "Data Dictionary sync successful"
        rm -f "$SQL_FILE"

        # Show summary
        docker exec pi5-timescaledb psql -U postgres -d ndp -c \
            "SELECT streams_synced, schemas_synced, attributes_synced, completed_at FROM data_dictionary.sync_status ORDER BY id DESC LIMIT 1;"
    else
        error "Data Dictionary sync failed"
        rm -f "$SQL_FILE"
        return 1
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
    echo "  MCP Server:  $(curl -sf http://localhost:9100/health 2>/dev/null && echo 'Running' || echo 'Not running')"
    echo "  TimescaleDB: $(docker exec pi5-timescaledb pg_isready -U postgres -d ndp 2>/dev/null && echo 'Running' || echo 'Not running')"
    echo "  Grafana:     $(curl -s -o /dev/null -w '%{http_code}' http://localhost:3000/api/health 2>/dev/null || echo 'Not running')"
    echo ""

    log "Silver Layer Status:"
    if docker exec pi5-timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; then
        # Check if silver schema exists
        if docker exec pi5-timescaledb psql -U postgres -d ndp -tAc "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = 'silver'" 2>/dev/null | grep -q "1"; then
            echo "  Schema:      silver schema exists"
            # Count hypertables
            hypertable_count=$(docker exec pi5-timescaledb psql -U postgres -d ndp -tAc "SELECT COUNT(*) FROM timescaledb_information.hypertables WHERE hypertable_schema = 'silver'" 2>/dev/null || echo "0")
            echo "  Hypertables: $hypertable_count"
        else
            echo "  Schema:      silver schema not created (run: ./deploy.sh silver-migrate)"
        fi
    else
        echo "  TimescaleDB not running"
    fi
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
    echo "  MCP Server:      http://${PI_IP}:9100"
    echo "  Grafana UI:      http://${PI_IP}:3000"
    echo "  MQTT Broker:     mqtt://${PI_IP}:1883"
    echo "  etcd:            http://${PI_IP}:2379"
}

update() {
    local no_cache=""
    local target="all"

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --no-cache)
                no_cache="--no-cache"
                shift
                ;;
            *)
                target="$1"
                shift
                ;;
        esac
    done

    log "Updating deployment (target: $target, no-cache: ${no_cache:-no})..."

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

    # Rebuild (with optional --no-cache)
    case "$target" in
        mcp)
            log "Rebuilding ndp-mcp-server only..."
            dc build $no_cache --progress=plain ndp-mcp-server
            dc up -d ndp-mcp-server
            ;;
        app)
            log "Rebuilding air-quality-app only..."
            dc build $no_cache --progress=plain air-quality-app
            dc up -d air-quality-app
            ;;
        silver)
            log "Rebuilding silver-etl only..."
            dc --profile silver build $no_cache --progress=plain silver-etl
            log "silver-etl rebuilt. Run './deploy.sh silver-migrate' or './deploy.sh silver-etl' to use it."
            ;;
        all|*)
            log "Rebuilding all services..."
            dc build $no_cache --progress=plain
            dc up -d
            ;;
    esac

    sync_config
    init_streams

    log "Update complete!"
    status
}

refresh() {
    log "Refreshing configuration (no rebuild)..."

    # Fetch latest from origin
    git -C "$REPO_ROOT" fetch origin

    # Check for local uncommitted changes to tracked files
    if ! git -C "$REPO_ROOT" diff --quiet HEAD 2>/dev/null; then
        warn "Local changes detected in tracked files:"
        git -C "$REPO_ROOT" diff --stat HEAD
        warn "These will be overwritten. Ctrl+C to abort, or wait 5 seconds to continue..."
        sleep 5
    fi

    # Reset to match origin exactly
    log "Syncing to origin/main..."
    git -C "$REPO_ROOT" reset --hard origin/main

    # Sync configurations (no rebuild)
    sync_config
    init_streams

    # Sync data dictionary if TimescaleDB is running
    if docker exec pi5-timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; then
        sync_to_data_dictionary
    fi

    # Restart Grafana to pick up dashboard/datasource changes
    log "Restarting Grafana..."
    docker restart grafana

    log "Refresh complete!"
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
        shift  # remove 'update' from args
        update "$@"
        ;;
    refresh)
        refresh
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
    sync-dictionary)
        sync_to_data_dictionary
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
    silver-etl)
        log "Running Silver ETL (Bronze -> TimescaleDB)..."
        # Ensure TimescaleDB is ready
        until docker exec pi5-timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
            warn "Waiting for TimescaleDB to be ready..."
            sleep 2
        done
        # Run silver-etl with the silver profile
        dc --profile silver run --rm silver-etl run
        log "Silver ETL complete"
        ;;
    silver-migrate)
        log "Running Silver Layer migrations..."
        # Ensure TimescaleDB is ready
        until docker exec pi5-timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
            warn "Waiting for TimescaleDB to be ready..."
            sleep 2
        done
        # Run migrations via silver-etl migrate command
        dc --profile silver run --rm silver-etl migrate
        log "Silver migrations complete"
        ;;
    *)
        echo "Usage: $0 {deploy|start|stop|logs|status|update|refresh|build|sync|init-streams|list-streams|sync-dictionary|analytics|rollback|silver-etl|silver-migrate}"
        echo ""
        echo "Commands:"
        echo "  deploy          - Full deploy (build + start all services)"
        echo "  start           - Start all services"
        echo "  stop            - Stop all services"
        echo "  logs            - View logs"
        echo "  status          - Check service health and URLs"
        echo "  update [--no-cache] [target] - Pull latest and rebuild"
        echo "                    --no-cache: Force full rebuild (skip Docker cache)"
        echo "                    Targets: app, mcp, silver, all (default)"
        echo "  refresh         - Pull latest configs only (no rebuild)"
        echo "  build           - Build Docker images"
        echo "  sync            - Sync configuration to etcd"
        echo "  init-streams    - Initialize stream configurations"
        echo "  list-streams    - List configured streams"
        echo "  sync-dictionary - Sync entity schemas to TimescaleDB data dictionary"
        echo "  analytics       - Start DuckDB + Grafana analytics stack"
        echo "  rollback        - Stop and remove analytics stack"
        echo "  silver-etl      - Run Silver ETL once (Bronze -> TimescaleDB)"
        echo "  silver-migrate  - Run Silver Layer TimescaleDB migrations"
        exit 1
        ;;
esac
