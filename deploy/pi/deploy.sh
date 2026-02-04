#!/bin/bash
# Neural Data Platform - Deployment Script
# Supports both Pi production and local integration environments
#
# Usage: ./deploy.sh [command] [options]
#        ./deploy.sh --help              - Show detailed help
#
# Environment (set via DEPLOY_ENV):
#   DEPLOY_ENV=pi          - Production on Raspberry Pi (default)
#   DEPLOY_ENV=integration - Local integration testing
#
# Examples:
#   ./deploy.sh                          - Deploy to Pi (production)
#   DEPLOY_ENV=integration ./deploy.sh   - Deploy locally (integration)
#   DEPLOY_ENV=integration ./deploy.sh status
#
# Core Commands:
#   deploy          - Full deploy (build + start all services)
#   start           - Start all services
#   stop            - Stop all services
#   logs            - View logs (follows)
#   status          - Check service health and URLs
#   build           - Build Docker images only
#
# Update Commands:
#   update [--no-cache] [target] - Pull latest from git and rebuild
#                     Targets: app, mcp, silver, all (default)
#   refresh         - Pull latest configs only (no rebuild, restarts Grafana)
#
# Configuration Commands:
#   sync            - Sync configuration to etcd (includes JSON stream configs)
#   init-streams    - [DEPRECATED] Use 'sync' instead (dp-018)
#   list-streams    - List configured streams from etcd
#   sync-dictionary - Sync entity schemas to TimescaleDB data dictionary
#
# Analytics Commands:
#   analytics       - Start DuckDB + Grafana analytics stack
#   rollback        - Stop and remove analytics stack (preserves volumes)
#
# Silver ETL Commands:
#   silver-migrate       - Run Silver Layer TimescaleDB schema migrations
#   silver-etl           - Run Silver ETL once (Bronze -> TimescaleDB)
#   silver-daemon        - Start Silver ETL in daemon mode (continuous)
#   silver-daemon-stop   - Stop Silver ETL daemon
#   silver-daemon-logs   - View Silver ETL daemon logs (follows)
#   silver-daemon-status - Check Silver ETL daemon status
#
# Environment Variables:
#   DEPLOY_ENV           - pi (default) or integration
#   SILVER_ETL_INTERVAL  - Daemon ETL interval in seconds (default: 300)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Environment: pi (default) or integration
DEPLOY_ENV="${DEPLOY_ENV:-pi}"

if [ "$DEPLOY_ENV" = "integration" ]; then
    COMPOSE_FILE="$REPO_ROOT/docker-compose.integration.yml"
    ENV_NAME="development"
    # Container names for external scripts that can't use docker compose exec
    ETCD_CONTAINER="integration-etcd"
    # Config paths for integration environment
    CONFIG_STREAMS_DIR="$REPO_ROOT/config/integration/base/streams"
    CONFIG_DOMAINS_DIR="$REPO_ROOT/config/integration/domains"
else
    COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
    ENV_NAME="production"
    # Container names for external scripts that can't use docker compose exec
    ETCD_CONTAINER="etcd"
    # Config paths for production (pi) environment
    CONFIG_STREAMS_DIR="$REPO_ROOT/config/base/streams"
    CONFIG_DOMAINS_DIR="$REPO_ROOT/config/domains"
fi

# Fallback: If env-specific directory doesn't exist, use production
if [ ! -d "$CONFIG_STREAMS_DIR" ]; then
    warn "Config directory not found: $CONFIG_STREAMS_DIR"
    warn "Falling back to production config: config/base/streams"
    CONFIG_STREAMS_DIR="$REPO_ROOT/config/base/streams"
fi

# Helper to run docker compose with the correct file
dc() {
    docker compose -f "$COMPOSE_FILE" "$@"
}

# Helper for docker compose exec (uses service names, not container names)
# Service names are consistent across compose files: etcd, timescaledb, etc.
dcx() {
    dc exec -T "$@"
}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[DEPLOY]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# ============================================================================
# DDL Generator Functions (dp-020)
# ============================================================================
if [ -f "$SCRIPT_DIR/ddl-generator.sh" ]; then
    source "$SCRIPT_DIR/ddl-generator.sh"
fi

# ============================================================================
# YAML Helper Functions
# Used by sync_to_data_dictionary and sync_dimensions
# ============================================================================

# Helper function to extract YAML values (compatible with both Python yq and Go yq)
# Falls back to Python if yq is unavailable, then grep/sed for simple top-level keys
yaml_get() {
    local file="$1"
    local key="$2"
    local default="$3"
    local result=""

    # For JSON files, use jq (more reliable)
    if [[ "$file" == *.json ]] && command -v jq &> /dev/null; then
        local jq_path=".${key}"
        result=$(jq -r "$jq_path // \"$default\"" "$file" 2>/dev/null)
    elif command -v yq &> /dev/null; then
        # Detect which yq variant is installed
        if yq --version 2>&1 | grep -q "mikefarah"; then
            # Go yq (mikefarah/yq)
            result=$(yq eval ".$key // \"$default\"" "$file" 2>/dev/null)
        else
            # Python yq (kislyuk/yq) - uses jq syntax
            result=$(yq -r ".$key // \"$default\"" "$file" 2>/dev/null)
        fi
    fi

    # Fallback to Python for nested keys if yq failed or not installed
    if [ -z "$result" ] || [ "$result" = "null" ]; then
        if command -v python3 &> /dev/null; then
            result=$(python3 -c "
import yaml
import sys
try:
    with open('$file') as f:
        data = yaml.safe_load(f)
    keys = '$key'.split('.')
    val = data
    for k in keys:
        if val is None:
            val = None
            break
        val = val.get(k) if isinstance(val, dict) else None
    if val is None:
        print('$default')
    else:
        print(val)
except Exception:
    print('$default')
" 2>/dev/null)
        fi
    fi

    # Final fallback to grep/sed for simple top-level keys only
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

    # For JSON files, use jq
    if [[ "$file" == *.json ]] && command -v jq &> /dev/null; then
        result=$(jq -r ".${key} | length // 0" "$file" 2>/dev/null || echo "0")
    elif command -v yq &> /dev/null; then
        if yq --version 2>&1 | grep -q "mikefarah"; then
            result=$(yq eval ".$key | length" "$file" 2>/dev/null || echo "0")
        else
            result=$(yq -r ".$key | length" "$file" 2>/dev/null || echo "0")
        fi
    fi

    # Fallback to Python if yq not available or failed
    if ! [[ "$result" =~ ^[0-9]+$ ]] || [ "$result" = "0" ]; then
        if command -v python3 &> /dev/null; then
            result=$(python3 -c "
import yaml
try:
    with open('$file') as f:
        data = yaml.safe_load(f)
    keys = '$key'.split('.')
    val = data
    for k in keys:
        if val is None:
            break
        if k.endswith(']'):
            # Handle array index in key path
            arr_key = k[:k.index('[')]
            idx = int(k[k.index('[')+1:-1])
            val = val.get(arr_key, [])[idx] if isinstance(val, dict) else None
        else:
            val = val.get(k) if isinstance(val, dict) else None
    print(len(val) if isinstance(val, list) else 0)
except Exception:
    print(0)
" 2>/dev/null || echo "0")
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

    # For JSON files, use jq (more reliable)
    if [[ "$file" == *.json ]] && command -v jq &> /dev/null; then
        result=$(jq -r "$path // \"$default\"" "$file" 2>/dev/null)
    elif command -v yq &> /dev/null; then
        if yq --version 2>&1 | grep -q "mikefarah"; then
            result=$(yq eval "$path // \"$default\"" "$file" 2>/dev/null)
        else
            result=$(yq -r "$path // \"$default\"" "$file" 2>/dev/null)
        fi
    fi

    # Fallback to Python if yq not available or failed
    if [ -z "$result" ] || [ "$result" = "null" ]; then
        if command -v python3 &> /dev/null; then
            result=$(python3 -c "
import yaml
import re
try:
    with open('$file') as f:
        data = yaml.safe_load(f)
    # Parse path like .silver_etl.field_mappings[0].source_path
    path = '$path'.lstrip('.')
    # Split by . but preserve array indices
    parts = re.split(r'\.(?![^\[]*\])', path)
    val = data
    for part in parts:
        if val is None:
            break
        # Check for array index
        match = re.match(r'(.+)\[(\d+)\]$', part)
        if match:
            key, idx = match.groups()
            val = val.get(key, []) if isinstance(val, dict) else val
            val = val[int(idx)] if isinstance(val, list) and len(val) > int(idx) else None
        else:
            val = val.get(part) if isinstance(val, dict) else None
    if val is None:
        print('$default')
    else:
        print(val)
except Exception:
    print('$default')
" 2>/dev/null)
        fi
    fi

    if [ -z "$result" ] || [ "$result" = "null" ]; then
        echo "$default"
    else
        echo "$result"
    fi
}

# Extract column names from dimension config schema.fields array
# Returns comma-separated list of column names for SQL COPY
yaml_get_schema_columns() {
    local file="$1"
    local result=""

    if command -v python3 &> /dev/null; then
        result=$(python3 -c "
import yaml
try:
    with open('$file') as f:
        data = yaml.safe_load(f)
    fields = data.get('schema', {}).get('fields', [])
    if fields:
        names = [f['name'] for f in fields if 'name' in f]
        print(','.join(names))
    else:
        print('')
except Exception as e:
    print('')
" 2>/dev/null)
    elif command -v yq &> /dev/null; then
        if yq --version 2>&1 | grep -q "mikefarah"; then
            result=$(yq eval '.schema.fields[].name' "$file" 2>/dev/null | tr '\n' ',' | sed 's/,$//')
        else
            result=$(yq -r '.schema.fields[].name' "$file" 2>/dev/null | tr '\n' ',' | sed 's/,$//')
        fi
    fi

    echo "$result"
}

# Show environment on startup
log "Environment: $DEPLOY_ENV (compose: $(basename $COMPOSE_FILE))"
log "Config paths: streams=${CONFIG_STREAMS_DIR#$REPO_ROOT/}"

check_prereqs() {
    log "Checking prerequisites..."
    command -v docker >/dev/null 2>&1 || error "Docker not installed"
    command -v docker compose >/dev/null 2>&1 || error "Docker Compose not installed"
    log "Prerequisites OK"
}

sync_config() {
    log "Syncing configuration to etcd..."

    # Wait for etcd to be ready
    until dcx etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd to be ready..."
        sleep 2
    done

    # Sync JSON stream configs (dp-018 architecture)
    # This is the primary sync method - stores complete JSON blobs at /streams/{id}/config
    if [ -f "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" ]; then
        log "Syncing stream configurations (JSON)..."
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" --mode docker
    else
        warn "Stream sync script not found at $REPO_ROOT/scripts/sync-streams-to-etcd.sh"
    fi

    # Legacy: sync-config-to-etcd.sh for non-stream configs (if still needed)
    # TODO(dp-018): Migrate remaining configs to JSON and consolidate
    if [ -f "$REPO_ROOT/scripts/sync-config-to-etcd.sh" ]; then
        log "Syncing legacy configurations..."
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-config-to-etcd.sh" $ENV_NAME
    fi
}

init_streams() {
    # DEPRECATED: dp-018 migrates to JSON config files synced via sync_config()
    warn "DEPRECATED: init-streams command is deprecated since dp-018"
    warn "Stream configs are now synced via 'sync' command using JSON files from:"
    warn "  config/base/streams/*/config.json"
    warn ""
    warn "To sync streams, run: ./deploy.sh sync"
    warn ""

    # For backward compatibility during transition, run sync_config instead
    # This ensures streams are synced even if someone runs the old command
    sync_config
}

sync_to_data_dictionary() {
    log "Syncing Data Dictionary to TimescaleDB..."

    # Check if TimescaleDB is running
    until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
        warn "Waiting for TimescaleDB to be ready..."
        sleep 2
    done

    local CONFIG_DIR="$CONFIG_STREAMS_DIR"
    local SQL_FILE="/tmp/data_dictionary_sync_$$.sql"

    # Note: yaml_get, yaml_array_len, yaml_array_get are defined at top-level

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

        # ========================================================================
        # SILVER LAYER METADATA SYNC
        # Uses UPSERT (ON CONFLICT DO UPDATE) since multiple streams can feed
        # the same Silver table (e.g., outdoor-weather + nws-observations -> weather_observations)
        # ========================================================================

        echo ""
        echo "-- ============================================"
        echo "-- SILVER LAYER DATA DICTIONARY"
        echo "-- ============================================"
        echo ""

        # Declare associative arrays for two-pass collection
        # SILVER_TABLES[target_table] = "stream1 stream2 ..."
        # SILVER_DESCRIPTIONS[target_table] = "description"
        # SILVER_GRAINS[target_table] = "grain"
        # SILVER_TIMESTAMP_COLS[target_table] = "observation_time"
        declare -A SILVER_TABLES
        declare -A SILVER_DESCRIPTIONS
        declare -A SILVER_GRAINS
        declare -A SILVER_TIMESTAMP_COLS

        local silver_table_count=0
        local silver_column_count=0
        local silver_lineage_count=0
        local silver_dq_rule_count=0

        # ---------------------------------------------------------------------------
        # PASS 1: Collect all Silver-enabled streams and group by target table
        # ---------------------------------------------------------------------------
        for config_dir in "$CONFIG_DIR"/*/; do
            if [ -f "$config_dir/config.yaml" ]; then
                local stream_id=$(basename "$config_dir")
                local config_file="$config_dir/config.yaml"

                # Check if silver_etl is enabled
                local silver_enabled=$(yaml_get "$config_file" "silver_etl.enabled" "false")
                if [ "$silver_enabled" != "true" ]; then
                    continue
                fi

                # Get target table
                local target_table=$(yaml_get "$config_file" "silver_etl.target_table" "")
                if [ -z "$target_table" ] || [ "$target_table" = "null" ]; then
                    continue
                fi

                # Accumulate streams for this table
                if [ -n "${SILVER_TABLES[$target_table]}" ]; then
                    SILVER_TABLES[$target_table]="${SILVER_TABLES[$target_table]} $stream_id"
                else
                    SILVER_TABLES[$target_table]="$stream_id"

                    # First stream defines the table metadata (can be overridden)
                    local silver_desc=$(yaml_get "$config_file" "silver_etl.description" "")
                    local silver_grain=$(yaml_get "$config_file" "silver_etl.grain" "")
                    local timestamp_col=$(yaml_get "$config_file" "silver_etl.timestamp.target_field" "observation_time")

                    SILVER_DESCRIPTIONS[$target_table]="$silver_desc"
                    SILVER_GRAINS[$target_table]="$silver_grain"
                    SILVER_TIMESTAMP_COLS[$target_table]="$timestamp_col"
                fi
            fi
        done

        # ---------------------------------------------------------------------------
        # PASS 2: Generate UPSERT SQL for silver_tables
        # ---------------------------------------------------------------------------
        for target_table in "${!SILVER_TABLES[@]}"; do
            local streams="${SILVER_TABLES[$target_table]}"
            local description="${SILVER_DESCRIPTIONS[$target_table]}"
            local grain="${SILVER_GRAINS[$target_table]}"
            local timestamp_col="${SILVER_TIMESTAMP_COLS[$target_table]}"

            # Extract schema name from fully-qualified table name
            local schema_name="${target_table%%.*}"

            # Build PostgreSQL array literal from space-separated stream list
            local pg_array="ARRAY["
            local first=true
            for s in $streams; do
                if [ "$first" = true ]; then
                    pg_array="${pg_array}'$s'"
                    first=false
                else
                    pg_array="${pg_array},'$s'"
                fi
            done
            pg_array="${pg_array}]"

            # Escape single quotes for SQL
            description=$(echo "$description" | sed "s/'/''/g")
            grain=$(echo "$grain" | sed "s/'/''/g")

            # Handle NULL for optional fields
            local desc_sql="'$description'"
            local grain_sql="'$grain'"
            [ -z "$description" ] || [ "$description" = "null" ] && desc_sql="NULL"
            [ -z "$grain" ] || [ "$grain" = "null" ] && grain_sql="NULL"

            echo "-- Silver table: $target_table (sources: $streams)"
            echo "INSERT INTO data_dictionary.silver_tables (table_name, schema_name, description, grain, source_streams, hypertable_column)"
            echo "VALUES ('$target_table', '$schema_name', $desc_sql, $grain_sql, $pg_array, '$timestamp_col')"
            echo "ON CONFLICT (table_name) DO UPDATE SET"
            echo "    description = EXCLUDED.description,"
            echo "    grain = EXCLUDED.grain,"
            echo "    source_streams = EXCLUDED.source_streams,"
            echo "    hypertable_column = EXCLUDED.hypertable_column,"
            echo "    updated_at = NOW();"
            echo ""

            silver_table_count=$((silver_table_count + 1))
        done

        # ---------------------------------------------------------------------------
        # PASS 3: Generate silver_columns, silver_lineage, silver_dq_rules per stream
        # ---------------------------------------------------------------------------
        for config_dir in "$CONFIG_DIR"/*/; do
            if [ -f "$config_dir/config.yaml" ]; then
                local stream_id=$(basename "$config_dir")
                local config_file="$config_dir/config.yaml"

                # Check if silver_etl is enabled
                local silver_enabled=$(yaml_get "$config_file" "silver_etl.enabled" "false")
                if [ "$silver_enabled" != "true" ]; then
                    continue
                fi

                local target_table=$(yaml_get "$config_file" "silver_etl.target_table" "")
                if [ -z "$target_table" ] || [ "$target_table" = "null" ]; then
                    continue
                fi

                echo "-- Stream: $stream_id -> $target_table"
                echo ""

                # Process field_mappings for columns, lineage, and column-level DQ rules
                local fm_count=$(yaml_array_len "$config_file" "silver_etl.field_mappings")

                for i in $(seq 0 $((fm_count - 1))); do
                    local source_path=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].source_path" "")
                    local target_column=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].target_column" "")
                    local col_type=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].type" "text")
                    local col_unit=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].unit" "")
                    local col_desc=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].description" "")
                    local col_nullable=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].nullable" "true")

                    # Skip if no target_column defined
                    [ -z "$target_column" ] || [ "$target_column" = "null" ] && continue

                    # Map type names to PostgreSQL types
                    local pg_type="TEXT"
                    case "$col_type" in
                        double_precision) pg_type="DOUBLE PRECISION" ;;
                        smallint) pg_type="SMALLINT" ;;
                        integer|int) pg_type="INTEGER" ;;
                        bigint) pg_type="BIGINT" ;;
                        text) pg_type="TEXT" ;;
                        timestamptz) pg_type="TIMESTAMPTZ" ;;
                        boolean|bool) pg_type="BOOLEAN" ;;
                        jsonb) pg_type="JSONB" ;;
                        *) pg_type="TEXT" ;;
                    esac

                    # Escape and handle NULLs
                    col_desc=$(echo "$col_desc" | sed "s/'/''/g")
                    local unit_sql="'$col_unit'"
                    local desc_sql="'$col_desc'"
                    [ -z "$col_unit" ] || [ "$col_unit" = "null" ] && unit_sql="NULL"
                    [ -z "$col_desc" ] || [ "$col_desc" = "null" ] && desc_sql="NULL"

                    # Convert nullable string to boolean
                    local nullable_bool="true"
                    [ "$col_nullable" = "false" ] && nullable_bool="false"

                    # UPSERT silver_columns
                    echo "INSERT INTO data_dictionary.silver_columns (table_name, column_name, data_type, unit, description, nullable, sort_order)"
                    echo "VALUES ('$target_table', '$target_column', '$pg_type', $unit_sql, $desc_sql, $nullable_bool, $i)"
                    echo "ON CONFLICT (table_name, column_name) DO UPDATE SET"
                    echo "    data_type = EXCLUDED.data_type,"
                    echo "    unit = EXCLUDED.unit,"
                    echo "    description = EXCLUDED.description,"
                    echo "    nullable = EXCLUDED.nullable,"
                    echo "    sort_order = EXCLUDED.sort_order,"
                    echo "    updated_at = NOW();"

                    silver_column_count=$((silver_column_count + 1))

                    # Determine transformation type
                    local transform_type="direct"
                    local has_transform=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].transform.type" "")
                    [ -n "$has_transform" ] && [ "$has_transform" != "null" ] && transform_type="$has_transform"

                    # UPSERT silver_lineage
                    echo "INSERT INTO data_dictionary.silver_lineage (silver_table, silver_column, source_stream, source_path, transformation)"
                    echo "VALUES ('$target_table', '$target_column', '$stream_id', '$source_path', '$transform_type')"
                    echo "ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET"
                    echo "    source_path = EXCLUDED.source_path,"
                    echo "    transformation = EXCLUDED.transformation,"
                    echo "    updated_at = NOW();"

                    silver_lineage_count=$((silver_lineage_count + 1))

                    # Process column-level DQ rules
                    local dq_count=$(yaml_array_len "$config_file" "silver_etl.field_mappings[$i].dq_rules")
                    for j in $(seq 0 $((dq_count - 1))); do
                        local rule_name=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].dq_rules[$j].rule" "")
                        local rule_action=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].dq_rules[$j].action" "flag")

                        [ -z "$rule_name" ] || [ "$rule_name" = "null" ] && continue

                        # Build rule_params JSONB from available fields
                        local rule_params="{"
                        local param_first=true

                        # Extract common DQ rule parameters
                        local dq_min=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].dq_rules[$j].min" "")
                        local dq_max=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].dq_rules[$j].max" "")
                        local dq_clamp=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].dq_rules[$j].clamp_to_bounds" "")

                        if [ -n "$dq_min" ] && [ "$dq_min" != "null" ]; then
                            [ "$param_first" = false ] && rule_params="${rule_params},"
                            rule_params="${rule_params}\"min\":$dq_min"
                            param_first=false
                        fi
                        if [ -n "$dq_max" ] && [ "$dq_max" != "null" ]; then
                            [ "$param_first" = false ] && rule_params="${rule_params},"
                            rule_params="${rule_params}\"max\":$dq_max"
                            param_first=false
                        fi
                        if [ -n "$dq_clamp" ] && [ "$dq_clamp" != "null" ]; then
                            [ "$param_first" = false ] && rule_params="${rule_params},"
                            rule_params="${rule_params}\"clamp_to_bounds\":$dq_clamp"
                            param_first=false
                        fi

                        rule_params="${rule_params}}"

                        echo "INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)"
                        echo "VALUES ('$target_table', '$target_column', '$rule_name', '$rule_params'::jsonb, '$rule_action')"
                        echo "ON CONFLICT (silver_table, COALESCE(silver_column, ''), rule_name) DO UPDATE SET"
                        echo "    rule_params = EXCLUDED.rule_params,"
                        echo "    action = EXCLUDED.action,"
                        echo "    updated_at = NOW();"

                        silver_dq_rule_count=$((silver_dq_rule_count + 1))
                    done

                    echo ""
                done

                # Process table-level DQ rules (cross-field, freshness, rate_of_change, etc.)
                local table_dq_count=$(yaml_array_len "$config_file" "silver_etl.dq_rules")

                if [ "$table_dq_count" -gt 0 ]; then
                    echo "-- Table-level DQ rules for $target_table (from $stream_id)"

                    for k in $(seq 0 $((table_dq_count - 1))); do
                        local rule_type=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].rule" "")
                        local rule_action=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].action" "flag")

                        [ -z "$rule_type" ] || [ "$rule_type" = "null" ] && continue

                        # For cross-field rules, use the 'name' field as rule_name
                        # For other rules, use rule type + field as identifier
                        local rule_name=""
                        local rule_params="{"
                        local param_first=true
                        local silver_column="NULL"  # Table-level rules have no specific column

                        case "$rule_type" in
                            cross_field_check)
                                rule_name=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].name" "")
                                local expression=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].expression" "" | sed "s/'/''/g")
                                local message=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].message" "")

                                rule_params="\"expression\":\"$expression\""
                                [ -n "$message" ] && [ "$message" != "null" ] && rule_params="${rule_params},\"message\":\"$message\""
                                ;;
                            freshness_check)
                                local field=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].field" "")
                                local max_age=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].max_age" "")
                                local max_future=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].max_future" "")
                                local reference=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].reference" "")

                                rule_name="freshness_check_${field}"
                                rule_params="\"field\":\"$field\""
                                [ -n "$max_age" ] && [ "$max_age" != "null" ] && rule_params="${rule_params},\"max_age\":\"$max_age\""
                                [ -n "$max_future" ] && [ "$max_future" != "null" ] && rule_params="${rule_params},\"max_future\":\"$max_future\""
                                [ -n "$reference" ] && [ "$reference" != "null" ] && rule_params="${rule_params},\"reference\":\"$reference\""
                                ;;
                            rate_of_change)
                                local field=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].field" "")
                                local max_change=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].max_change_per_minute" "")

                                rule_name="rate_of_change_${field}"
                                rule_params="\"field\":\"$field\""
                                [ -n "$max_change" ] && [ "$max_change" != "null" ] && rule_params="${rule_params},\"max_change_per_minute\":$max_change"
                                ;;
                            completeness_check)
                                local level=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].level" "")
                                local field=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].field" "")
                                local min_comp=$(yaml_array_get "$config_file" ".silver_etl.dq_rules[$k].min_completeness" "")

                                rule_name="completeness_check_${field}"
                                rule_params="\"level\":\"$level\",\"field\":\"$field\""
                                [ -n "$min_comp" ] && [ "$min_comp" != "null" ] && rule_params="${rule_params},\"min_completeness\":$min_comp"
                                ;;
                            *)
                                # Generic handling for unknown rule types
                                rule_name="${rule_type}"
                                rule_params=""
                                ;;
                        esac

                        rule_params="{${rule_params}}"

                        [ -z "$rule_name" ] && continue

                        echo "INSERT INTO data_dictionary.silver_dq_rules (silver_table, silver_column, rule_name, rule_params, action)"
                        echo "VALUES ('$target_table', $silver_column, '$rule_name', '$rule_params'::jsonb, '$rule_action')"
                        echo "ON CONFLICT (silver_table, COALESCE(silver_column, ''), rule_name) DO UPDATE SET"
                        echo "    rule_params = EXCLUDED.rule_params,"
                        echo "    action = EXCLUDED.action,"
                        echo "    updated_at = NOW();"

                        silver_dq_rule_count=$((silver_dq_rule_count + 1))
                    done
                    echo ""
                fi
            fi
        done

        echo "-- Update sync status"
        echo "UPDATE data_dictionary.sync_status"
        echo "SET completed_at = NOW(),"
        echo "    status = 'success',"
        echo "    streams_synced = (SELECT COUNT(*) FROM data_dictionary.streams),"
        echo "    schemas_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schemas),"
        echo "    attributes_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schema_attributes),"
        echo "    silver_tables_synced = (SELECT COUNT(*) FROM data_dictionary.silver_tables),"
        echo "    silver_columns_synced = (SELECT COUNT(*) FROM data_dictionary.silver_columns)"
        echo "WHERE status = 'running' AND completed_at IS NULL;"
        echo ""
        echo "COMMIT;"

    } > "$SQL_FILE"

    # Execute sync
    log "Executing sync..."
    if dcx timescaledb psql -U postgres -d ndp < "$SQL_FILE" > /dev/null 2>&1; then
        log "Data Dictionary sync successful"
        rm -f "$SQL_FILE"

        # Show summary (Bronze + Silver)
        dcx timescaledb psql -U postgres -d ndp -c \
            "SELECT streams_synced AS bronze_streams, schemas_synced AS bronze_schemas, attributes_synced AS bronze_attrs, silver_tables_synced AS silver_tables, silver_columns_synced AS silver_cols, completed_at FROM data_dictionary.sync_status ORDER BY id DESC LIMIT 1;"
    else
        error "Data Dictionary sync failed"
        rm -f "$SQL_FILE"
        return 1
    fi
}

# ============================================================================
# DIMENSION TABLE SYNC (dp-013)
# Syncs dimension tables from config/base/dimensions/ to Silver layer
# ============================================================================

# State file for tracking dimension sync status
DIMENSION_STATE_FILE="$REPO_ROOT/data/.dimension_state"

# Update dimension sync state tracking
update_dimension_state() {
    local dimension_id="$1"
    local status="$2"
    local timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    # Ensure state directory exists
    mkdir -p "$(dirname "$DIMENSION_STATE_FILE")"

    # Update or add entry
    if [ -f "$DIMENSION_STATE_FILE" ] && grep -q "^$dimension_id:" "$DIMENSION_STATE_FILE" 2>/dev/null; then
        sed -i "s/^$dimension_id:.*/$dimension_id:$status:$timestamp/" "$DIMENSION_STATE_FILE"
    else
        echo "$dimension_id:$status:$timestamp" >> "$DIMENSION_STATE_FILE"
    fi
}

# Get dimension sync state
get_dimension_state() {
    local dimension_id="$1"
    if [ -f "$DIMENSION_STATE_FILE" ]; then
        grep "^$dimension_id:" "$DIMENSION_STATE_FILE" 2>/dev/null | cut -d: -f2
    else
        echo "unknown"
    fi
}

# Fallback SQL import for dimension data when ndp CLI is not available
import_dimension_sql() {
    local config_file="$1"
    local source_file="$2"
    local strategy="$3"

    local table_name=$(yaml_get "$config_file" "target.table" "")
    local schema_name=$(yaml_get "$config_file" "target.schema" "silver")

    if [ -z "$table_name" ]; then
        warn "No target table specified in $config_file"
        return 1
    fi

    # Extract column names from schema.fields in YAML config
    # This ensures we only import to columns defined in the CSV, not audit columns
    local columns=$(yaml_get_schema_columns "$config_file")
    if [ -z "$columns" ]; then
        warn "No schema.fields defined in $config_file, attempting import without column spec"
    fi

    log "Importing dimension data to ${schema_name}.${table_name}..."

    if [ "$strategy" = "truncate_and_load" ]; then
        # Truncate existing data
        if ! dcx timescaledb psql -U postgres -d ndp -c \
            "TRUNCATE TABLE ${schema_name}.${table_name};" 2>&1; then
            warn "Truncate failed (table may not exist yet)"
        fi
    fi

    # Import CSV data using COPY
    # Note: We copy the file into the container first, then import
    local temp_file="/tmp/dim_import_$$.csv"
    docker cp "$source_file" "$(docker compose -f "$COMPOSE_FILE" ps -q timescaledb):$temp_file"

    # Build COPY command with explicit columns if available
    local copy_cmd
    if [ -n "$columns" ]; then
        copy_cmd="\\COPY ${schema_name}.${table_name}(${columns}) FROM '$temp_file' WITH (FORMAT csv, HEADER true);"
    else
        copy_cmd="\\COPY ${schema_name}.${table_name} FROM '$temp_file' WITH (FORMAT csv, HEADER true);"
    fi

    local output
    if output=$(dcx timescaledb psql -U postgres -d ndp -c "$copy_cmd" 2>&1); then
        log "Successfully imported dimension data to ${schema_name}.${table_name}"
        log "$output"
        # Clean up temp file in container
        dcx timescaledb rm -f "$temp_file" 2>/dev/null || true
        return 0
    else
        warn "Failed to import dimension data to ${schema_name}.${table_name}"
        warn "Error: $output"
        dcx timescaledb rm -f "$temp_file" 2>/dev/null || true
        return 1
    fi
}

# Sync a single dimension table
sync_dimension() {
    local dimension_id="$1"
    local config_file="$2"
    local source_file="$3"
    local strategy="$4"

    log "Loading dimension $dimension_id with strategy: $strategy"

    update_dimension_state "$dimension_id" "syncing"

    # Call the Rust CLI to perform the actual sync if available
    if command -v ndp &> /dev/null; then
        if ndp dimension sync "$dimension_id" --config "$config_file" --source "$source_file"; then
            update_dimension_state "$dimension_id" "success"
            return 0
        else
            update_dimension_state "$dimension_id" "failed"
            return 1
        fi
    else
        # Fallback to direct SQL import
        if import_dimension_sql "$config_file" "$source_file" "$strategy"; then
            update_dimension_state "$dimension_id" "success"
            return 0
        else
            update_dimension_state "$dimension_id" "failed"
            return 1
        fi
    fi
}

# Sync all dimension tables from config/base/dimensions/
sync_dimensions() {
    local dimension_dir="$REPO_ROOT/config/base/dimensions"

    if [ ! -d "$dimension_dir" ]; then
        log "No dimensions directory found at $dimension_dir"
        return 0
    fi

    log "Syncing dimension tables..."

    # Ensure TimescaleDB is ready
    until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
        warn "Waiting for TimescaleDB to be ready..."
        sleep 2
    done

    local dimension_count=0
    local success_count=0

    # Find all dimension config files
    for config_file in "$dimension_dir"/*.yaml "$dimension_dir"/*.yml; do
        if [ -f "$config_file" ]; then
            local dimension_id=$(yaml_get "$config_file" "dimension_id" "")
            local table_name=$(yaml_get "$config_file" "target.table" "")
            local schema_name=$(yaml_get "$config_file" "target.schema" "silver")
            local source_path=$(yaml_get "$config_file" "source.path" "")
            local strategy=$(yaml_get "$config_file" "load.strategy" "truncate_and_load")

            if [ -z "$dimension_id" ]; then
                dimension_id=$(basename "$config_file" | sed 's/\.\(yaml\|yml\)$//')
            fi

            if [ -z "$table_name" ]; then
                warn "Skipping $config_file: no target table specified"
                continue
            fi

            log "Processing dimension: $dimension_id -> $schema_name.$table_name"

            # Check if source file exists
            local full_path="$REPO_ROOT/$source_path"
            if [ ! -f "$full_path" ]; then
                warn "Source file not found: $full_path"
                update_dimension_state "$dimension_id" "source_missing"
                continue
            fi

            dimension_count=$((dimension_count + 1))

            # Execute dimension sync
            if sync_dimension "$dimension_id" "$config_file" "$full_path" "$strategy"; then
                success_count=$((success_count + 1))
            fi
        fi
    done

    if [ "$dimension_count" -eq 0 ]; then
        log "No dimension configurations found in $dimension_dir"
    else
        log "Dimension sync complete: $success_count/$dimension_count succeeded"
    fi
}

# List all configured dimensions and their sync status
list_dimensions() {
    local dimension_dir="$REPO_ROOT/config/base/dimensions"

    if [ ! -d "$dimension_dir" ]; then
        log "No dimensions directory found at $dimension_dir"
        return 0
    fi

    echo ""
    log "Configured Dimensions:"
    echo "  ID                           TABLE                    STATUS           LAST_SYNC"
    echo "  -------------------------------------------------------------------------"

    for config_file in "$dimension_dir"/*.yaml "$dimension_dir"/*.yml; do
        if [ -f "$config_file" ]; then
            local dimension_id=$(yaml_get "$config_file" "dimension_id" "")
            local table_name=$(yaml_get "$config_file" "target.table" "")
            local schema_name=$(yaml_get "$config_file" "target.schema" "silver")

            if [ -z "$dimension_id" ]; then
                dimension_id=$(basename "$config_file" | sed 's/\.\(yaml\|yml\)$//')
            fi

            local status="unknown"
            local last_sync=""
            if [ -f "$DIMENSION_STATE_FILE" ]; then
                local state_line=$(grep "^$dimension_id:" "$DIMENSION_STATE_FILE" 2>/dev/null)
                if [ -n "$state_line" ]; then
                    status=$(echo "$state_line" | cut -d: -f2)
                    last_sync=$(echo "$state_line" | cut -d: -f3-)
                fi
            fi

            printf "  %-28s %-24s %-16s %s\n" "$dimension_id" "$schema_name.$table_name" "$status" "$last_sync"
        fi
    done

    echo ""

    # Document expected CLI interface
    echo "  Expected ndp CLI commands (when available):"
    echo "    ndp dimension list              - List all configured dimensions"
    echo "    ndp dimension sync <id>         - Sync specific dimension"
    echo "    ndp dimension sync --all        - Sync all dimensions"
    echo "    ndp dimension status            - Show dimension sync status"
    echo ""
}

# Show dimension sync status
dimension_status() {
    local dimension_dir="$REPO_ROOT/config/base/dimensions"

    if [ ! -d "$dimension_dir" ]; then
        log "No dimensions directory found at $dimension_dir"
        return 0
    fi

    log "Dimension Sync Status:"

    if [ ! -f "$DIMENSION_STATE_FILE" ]; then
        echo "  No sync history found. Run './deploy.sh sync-dimensions' to sync."
        return 0
    fi

    echo ""
    echo "  State file: $DIMENSION_STATE_FILE"
    echo ""

    while IFS=: read -r dim_id status timestamp; do
        printf "  %-28s %-16s %s\n" "$dim_id" "$status" "$timestamp"
    done < "$DIMENSION_STATE_FILE"

    echo ""
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
    echo "  etcd:        $(dcx etcd etcdctl endpoint health 2>/dev/null || echo 'Not running')"
    echo "  Air Quality: $(curl -s http://localhost:8080/health 2>/dev/null || echo 'Not running')"
    echo "  MCP Server:  $(curl -sf http://localhost:9100/health 2>/dev/null && echo 'Running' || echo 'Not running')"
    echo "  TimescaleDB: $(dcx timescaledb pg_isready -U postgres -d ndp 2>/dev/null && echo 'Running' || echo 'Not running')"
    echo "  Grafana:     $(curl -s -o /dev/null -w '%{http_code}' http://localhost:3000/api/health 2>/dev/null || echo 'Not running')"
    echo ""

    log "Silver Layer Status:"
    if dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; then
        # Check if silver schema exists
        if dcx timescaledb psql -U postgres -d ndp -tAc "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = 'silver'" 2>/dev/null | grep -q "1"; then
            echo "  Schema:      silver schema exists"
            # Count hypertables
            hypertable_count=$(dcx timescaledb psql -U postgres -d ndp -tAc "SELECT COUNT(*) FROM timescaledb_information.hypertables WHERE hypertable_schema = 'silver'" 2>/dev/null || echo "0")
            echo "  Hypertables: $hypertable_count"
        else
            echo "  Schema:      silver schema not created (run: ./deploy.sh silver-migrate)"
        fi
    else
        echo "  TimescaleDB not running"
    fi
    echo ""

    log "Data Volume:"
    dcx air-quality-app du -sh /data 2>/dev/null || echo "  Not available"
    echo ""

    log "Stream Status:"
    if [ -f "$SCRIPT_DIR/configs/streams/list-streams.sh" ]; then
        bash "$SCRIPT_DIR/configs/streams/list-streams.sh" $ETCD_CONTAINER 2>/dev/null || echo "  Unable to fetch stream status"
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

    # Sync data dictionary and dimensions if TimescaleDB is running
    if dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; then
        sync_to_data_dictionary
        sync_dimensions
    fi

    # Restart Grafana to pick up dashboard/datasource changes
    log "Restarting Grafana..."
    dc restart grafana

    log "Refresh complete!"
    status
}

# ============================================================================
# DECLARATIVE DEPLOY (dp-020)
# Applies changes from .deploy/manifest.json
# ============================================================================

MANIFEST_FILE=".deploy/manifest.json"

# Validate manifest JSON against schema
# Returns: 0 if valid, 1 if invalid
validate_manifest() {
    local manifest_file="${1:-$REPO_ROOT/$MANIFEST_FILE}"

    if [ ! -f "$manifest_file" ]; then
        error "Manifest not found: $manifest_file"
    fi

    # Check required fields - support both version formats
    local version=$(jq -r '.version // empty' "$manifest_file")
    if [ -z "$version" ]; then
        error "Invalid or missing manifest version"
    fi

    # Version can be "1.0" (old format) or semver like "1.0.0" (release format)
    # Accept "1.0", "1.0.x", or any valid release version
    if [[ ! "$version" =~ ^1\.0($|\..*$|$) ]] && [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
        error "Invalid manifest version format (got: $version)"
    fi

    # Support both old format (.changes array) and new format (.declarations object)
    local changes_count=0
    local declarations_count=0

    # Check old format: .changes array
    changes_count=$(jq '.changes | length // 0' "$manifest_file" 2>/dev/null || echo "0")

    # Check new format: .declarations object (fe-001 Gold layer)
    # Count all items in all declaration arrays
    declarations_count=$(jq '[.declarations // {} | to_entries[] | .value | length] | add // 0' "$manifest_file" 2>/dev/null || echo "0")

    local total_count=$((changes_count + declarations_count))

    if [ "$total_count" -eq 0 ]; then
        warn "Manifest has no changes to apply"
        return 1
    fi

    # Log what was found
    if [ "$changes_count" -gt 0 ] && [ "$declarations_count" -gt 0 ]; then
        log "Manifest valid: $changes_count change(s) + $declarations_count declaration(s)"
    elif [ "$changes_count" -gt 0 ]; then
        log "Manifest valid: $changes_count change(s) declared"
    else
        log "Manifest valid: $declarations_count declaration(s)"
    fi

    # Validate known declaration types (fe-001)
    local known_types='["etcd-config", "dimensions", "silver-tables", "streams", "dashboards", "gold-tables", "domains", "migrations"]'
    local unknown_types=$(jq -r --argjson known "$known_types" '
        [.declarations // {} | keys[] | select(. as $k | $known | index($k) | not)] | join(", ")
    ' "$manifest_file" 2>/dev/null || echo "")

    if [ -n "$unknown_types" ]; then
        warn "Unknown declaration types will be ignored: $unknown_types"
    fi

    return 0
}

# Handle stream declaration
# Args: $1 = declaration JSON
# Note: Currently syncs all streams when any stream is declared.
# TODO: Add individual stream sync support to sync-streams-to-etcd.sh
handle_stream() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.id')
    local action=$(echo "$declaration" | jq -r '.action // "update"')
    local reload=$(echo "$declaration" | jq -r '.reload // "none"')

    log "Stream: $stream_id (action=$action, reload=$reload)"

    # Check for dry-run mode
    if [ "${DRY_RUN:-false}" = "true" ]; then
        log "  [DRY-RUN] Would sync stream $stream_id to etcd"
        return 0
    fi

    local config_file="$CONFIG_STREAMS_DIR/$stream_id/config.json"
    local config_yaml="$CONFIG_STREAMS_DIR/$stream_id/config.yaml"

    if [ ! -f "$config_file" ] && [ ! -f "$config_yaml" ]; then
        error "Stream config not found: $config_file or $config_yaml"
    fi

    case "$action" in
        create|update)
            # Sync all stream configs to etcd
            # Note: Individual stream sync not yet supported
            log "  Syncing all streams to etcd..."
            ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" --mode docker
            ;;
        validate-only)
            log "  Validation only - no changes applied"
            ;;
        *)
            error "Unknown stream action: $action"
            ;;
    esac

    return 0
}

# Handle silver-table declaration
# Args: $1 = declaration JSON
handle_silver_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Silver Table: $stream_id (action=$action)"

    # Check for dry-run mode
    if [ "${DRY_RUN:-false}" = "true" ]; then
        log "  [DRY-RUN] Would generate and apply Silver DDL for $stream_id"
        return 0
    fi

    # Check if ddl-generator functions are available
    if ! type generate_silver_ddl &>/dev/null; then
        error "DDL generator not loaded. Check ddl-generator.sh"
    fi

    # Set DDL_REPO_ROOT for ddl-generator.sh
    export DDL_REPO_ROOT="$REPO_ROOT"

    case "$action" in
        sync)
            # Generate DDL (mode=full)
            local ddl=$(generate_silver_ddl "$stream_id" "full" 2>&1)

            if echo "$ddl" | grep -q "^-- SKIP"; then
                warn "  Skipped: silver_etl not enabled or no target_table"
                return 0
            fi

            if echo "$ddl" | grep -q "^-- ERROR"; then
                error "DDL generation failed: $ddl"
            fi

            # Apply DDL to TimescaleDB
            log "  Applying DDL to TimescaleDB..."
            echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
            ;;
        validate-only)
            log "  Validation only - DDL generation test"
            generate_silver_ddl "$stream_id" "full" > /dev/null
            ;;
        *)
            error "Unknown silver-table action: $action"
            ;;
    esac

    return 0
}

# Handle migration declaration
# Args: $1 = declaration JSON
handle_migration() {
    local declaration="$1"
    local migration_file=$(echo "$declaration" | jq -r '.file')

    log "Migration: $migration_file"

    local full_path="$REPO_ROOT/$migration_file"
    if [ ! -f "$full_path" ]; then
        error "Migration file not found: $full_path"
        return 1
    fi

    # Apply migration to TimescaleDB
    # Note: We pipe the file content via stdin since the file is on the host,
    # not inside the container. Using -f - reads SQL from stdin.
    log "  Applying migration..."
    cat "$full_path" | dcx timescaledb psql -U postgres -d ndp -f -

    return 0
}

# Handle dimensions declaration
# Args: $1 = declaration JSON
handle_dimensions() {
    local declaration="$1"
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Dimensions: (action=$action)"

    case "$action" in
        sync)
            sync_dimensions
            ;;
        *)
            error "Unknown dimensions action: $action"
            ;;
    esac

    return 0
}

# Handle dictionary declaration
# Args: $1 = declaration JSON
handle_dictionary() {
    local declaration="$1"
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Dictionary: (action=$action)"

    case "$action" in
        sync)
            sync_to_data_dictionary
            ;;
        *)
            error "Unknown dictionary action: $action"
            ;;
    esac

    return 0
}

# Handle container build declaration
# Args: $1 = declaration JSON
handle_container_build() {
    local declaration="$1"
    local target=$(echo "$declaration" | jq -r '.target')
    local no_cache=$(echo "$declaration" | jq -r '.no_cache // false')

    log "Container Build: $target (no_cache=$no_cache)"

    local build_args=""
    if [ "$no_cache" = "true" ]; then
        build_args="--no-cache"
    fi

    case "$target" in
        air-quality-app)
            dc build $build_args air-quality-app
            ;;
        ndp-mcp-server)
            dc build $build_args ndp-mcp-server
            ;;
        silver-etl)
            dc --profile silver build $build_args silver-etl
            ;;
        grafana)
            dc build $build_args grafana
            ;;
        *)
            error "Unknown container target: $target"
            ;;
    esac

    return 0
}

# ============================================================================
# TOOL BUILD HANDLERS (fe-001 Phase B)
# ============================================================================

# Handle tool declaration - builds Rust CLI tools declaratively
# Args: $1 = declaration JSON
# Example: {"type": "tool", "id": "ndp-gold-ddl", "action": "build"}
#
# Supported tools:
#   - ndp-gold-ddl: Gold layer DDL generator
#   - ndp-validate: Configuration validator
#
# This handler maintains the declarative deployment promise by allowing
# tool builds to be specified in manifests rather than hardcoded in deploy.sh
handle_tool() {
    local declaration="$1"
    local tool_id=$(echo "$declaration" | jq -r '.id')
    local action=$(echo "$declaration" | jq -r '.action // "build"')
    local profile=$(echo "$declaration" | jq -r '.profile // "release"')

    log "Tool Build: $tool_id (action=$action, profile=$profile)"

    # Validate tool_id is provided
    if [ -z "$tool_id" ] || [ "$tool_id" = "null" ]; then
        error "Tool declaration missing 'id' field"
        return 1
    fi

    # Validate action
    if [ "$action" != "build" ]; then
        error "Tool declaration has unsupported action: $action (only 'build' supported)"
        return 1
    fi

    # Map tool_id to Cargo package name and binary location
    local cargo_package=""
    local binary_name=""
    case "$tool_id" in
        ndp-gold-ddl)
            cargo_package="ndp-gold-ddl"
            binary_name="ndp-gold-ddl"
            ;;
        ndp-validate)
            cargo_package="ndp-validate"
            binary_name="ndp-validate"
            ;;
        *)
            error "Unknown tool: $tool_id"
            error "Supported tools: ndp-gold-ddl, ndp-validate"
            return 1
            ;;
    esac

    # Determine build profile
    local build_args=""
    local target_dir=""
    if [ "$profile" = "release" ]; then
        build_args="--release"
        target_dir="release"
    else
        target_dir="debug"
    fi

    # Build the tool
    log "  Building $cargo_package with profile=$profile..."
    if ! cargo build $build_args --manifest-path "$REPO_ROOT/tools/$cargo_package/Cargo.toml" 2>&1 | while read -r line; do
        # Show cargo output with indentation
        echo "    $line"
    done; then
        error "Failed to build $tool_id"
        return 1
    fi

    # Verify binary was created
    local binary_path="$REPO_ROOT/target/$target_dir/$binary_name"
    if [ ! -x "$binary_path" ]; then
        error "Build succeeded but binary not found at: $binary_path"
        return 1
    fi

    log "  ✓ Built: $binary_path"
    return 0
}

# ============================================================================
# GOLD LAYER HANDLERS (fe-001 Phase A)
# ============================================================================

# Handle gold-table declaration
# Args: $1 = declaration JSON
# Calls ndp-gold-ddl Rust tool to generate DDL, then applies to TimescaleDB
# The tool handles idempotency by checking database state when --database-url is provided
handle_gold_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Gold Table: $stream_id (action=$action)"

    # Validate stream_id is provided
    if [ -z "$stream_id" ] || [ "$stream_id" = "null" ]; then
        error "Gold table declaration missing stream_id"
        return 1
    fi

    # Check for dry-run mode (set by --dry-run flag)
    if [ "${DRY_RUN:-false}" = "true" ]; then
        log "  [DRY-RUN] Would call: ndp-gold-ddl generate --stream $stream_id --action $action"
        return 0
    fi

    # Check if ndp-gold-ddl tool is available
    local gold_ddl_tool=""
    if command -v ndp-gold-ddl &> /dev/null; then
        gold_ddl_tool="ndp-gold-ddl"
    elif [ -x "/opt/ndp/bin/ndp-gold-ddl" ]; then
        gold_ddl_tool="/opt/ndp/bin/ndp-gold-ddl"
    elif [ -x "$REPO_ROOT/target/release/ndp-gold-ddl" ]; then
        gold_ddl_tool="$REPO_ROOT/target/release/ndp-gold-ddl"
    elif [ -x "$REPO_ROOT/target/debug/ndp-gold-ddl" ]; then
        gold_ddl_tool="$REPO_ROOT/target/debug/ndp-gold-ddl"
    else
        warn "  ndp-gold-ddl tool not found, skipping Gold DDL generation"
        warn "  Build the tool with: cargo build --release -p ndp-gold-ddl"
        return 0
    fi

    # Build database URL for the tool to connect and check existence
    # The tool handles idempotency internally when given a database URL
    local db_password="${POSTGRES_PASSWORD:-ndp_secure_password}"
    local db_url="postgresql://postgres:${db_password}@localhost:5432/ndp"

    # Call Rust tool for DDL generation with database connectivity
    # Tool connects to DB, checks what exists, and outputs only needed DDL
    log "  Generating Gold DDL using $gold_ddl_tool (with DB check)..."
    local ddl
    ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" \
        --database-url "$db_url" \
        --db-timeout 10 \
        generate --stream "$stream_id" --action "$action" 2>&1)
    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        error "Gold DDL generation failed: $ddl"
        return 1
    fi

    # Check if DDL indicates nothing to do (all skipped)
    if echo "$ddl" | grep -q "Skipping.*already exists" && ! echo "$ddl" | grep -q "Creating"; then
        log "  All Gold tables for $stream_id already exist, nothing to create"
        # Still apply the DDL - it may contain refresh policies for existing tables
    fi

    # Apply DDL to TimescaleDB
    # The tool has already done the existence checks, so DDL is ready to execute
    log "  Applying Gold DDL to TimescaleDB..."
    if echo "$ddl" | dcx timescaledb psql -U postgres -d ndp 2>&1; then
        log "  Gold table(s) for $stream_id created/updated successfully"
        return 0
    else
        error "  Failed to apply Gold DDL for $stream_id"
        return 1
    fi
}

# Handle domain declaration
# Args: $1 = declaration JSON
# Syncs domain config to etcd and generates aligned view DDL
handle_domain() {
    local declaration="$1"
    local domain_id=$(echo "$declaration" | jq -r '.domain_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Domain: $domain_id (action=$action)"

    # Validate domain_id is provided
    if [ -z "$domain_id" ] || [ "$domain_id" = "null" ]; then
        error "Domain declaration missing domain_id"
        return 1
    fi

    # Check for dry-run mode
    if [ "${DRY_RUN:-false}" = "true" ]; then
        log "  [DRY-RUN] Would sync domain config to etcd and generate aligned view DDL"
        return 0
    fi

    # Check if domain config exists (env-specific or fallback to production)
    local config_file="$CONFIG_DOMAINS_DIR/$domain_id/domain.yaml"
    local config_file_fallback="$REPO_ROOT/config/domains/$domain_id/domain.yaml"

    if [ -f "$config_file" ]; then
        : # Use env-specific config_file
    elif [ -f "$config_file_fallback" ]; then
        config_file="$config_file_fallback"
    else
        warn "  Domain config not found: $config_file"
        warn "  Create config/domains/$domain_id/domain.yaml to configure this domain"
        return 0
    fi

    # Sync domain config to etcd if available
    log "  Syncing domain config to etcd..."
    if dcx etcd etcdctl endpoint health >/dev/null 2>&1; then
        if cat "$config_file" | dcx etcd etcdctl put "/domains/$domain_id/config" -; then
            log "  Domain config synced to etcd at /domains/$domain_id/config"
        else
            warn "  Failed to sync domain config to etcd (non-fatal)"
        fi
    else
        warn "  etcd not available, skipping domain config sync"
    fi

    # Check if ndp-gold-ddl tool is available for aligned view generation
    local gold_ddl_tool=""
    if command -v ndp-gold-ddl &> /dev/null; then
        gold_ddl_tool="ndp-gold-ddl"
    elif [ -x "/opt/ndp/bin/ndp-gold-ddl" ]; then
        gold_ddl_tool="/opt/ndp/bin/ndp-gold-ddl"
    elif [ -x "$REPO_ROOT/target/release/ndp-gold-ddl" ]; then
        gold_ddl_tool="$REPO_ROOT/target/release/ndp-gold-ddl"
    elif [ -x "$REPO_ROOT/target/debug/ndp-gold-ddl" ]; then
        gold_ddl_tool="$REPO_ROOT/target/debug/ndp-gold-ddl"
    fi

    if [ -z "$gold_ddl_tool" ]; then
        warn "  ndp-gold-ddl tool not found, skipping aligned view DDL generation"
        warn "  Build the tool with: cargo build --release -p ndp-gold-ddl"
        return 0
    fi

    # Generate and apply aligned view DDL
    # Note: --config-dir is a top-level option, must come before subcommand
    log "  Generating aligned view DDL using $gold_ddl_tool..."
    local ddl
    ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" generate --domain "$domain_id" --action "$action" 2>&1)
    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        error "Domain DDL generation failed: $ddl"
        return 1
    fi

    # Check if DDL is empty or just comments
    if [ -z "$ddl" ] || echo "$ddl" | grep -qE '^--.*$|^$' && ! echo "$ddl" | grep -q 'CREATE'; then
        log "  No Domain DDL changes required for $domain_id"
        return 0
    fi

    # Apply DDL to TimescaleDB
    log "  Applying Domain DDL to TimescaleDB..."
    if echo "$ddl" | dcx timescaledb psql -U postgres -d ndp; then
        log "  Aligned view(s) for domain $domain_id created/updated successfully"
        return 0
    else
        error "Failed to apply Domain DDL to TimescaleDB"
        return 1
    fi
}

# ============================================================================
# CLASSIFICATION SYNC HELPERS (FE-001 Phase B v11-002)
# ============================================================================

# Derive correlation role from stream type
# Returns: effect, cause, context, metadata
derive_correlation_role() {
    local stream_type="$1"
    case "$stream_type" in
        observation) echo "effect" ;;
        state_event) echo "cause" ;;
        forecast)    echo "context" ;;
        dimension)   echo "metadata" ;;
        *)           echo "unknown" ;;
    esac
}

# Derive NULL handling from stream type
# Returns: preserve, carry_forward
derive_null_handling() {
    local stream_type="$1"
    case "$stream_type" in
        observation) echo "preserve" ;;
        state_event) echo "carry_forward" ;;
        forecast)    echo "preserve" ;;
        dimension)   echo "carry_forward" ;;
        *)           echo "preserve" ;;
    esac
}

# Sync stream classification to data dictionary (v11-002)
# Called during sync_to_data_dictionary() for streams with stream_type
sync_stream_classification() {
    local stream_id="$1"
    local stream_type="$2"
    local description="$3"

    if [ -z "$stream_type" ] || [ "$stream_type" = "null" ]; then
        # No stream_type defined, skip classification sync
        return 0
    fi

    local correlation_role=$(derive_correlation_role "$stream_type")
    local null_handling=$(derive_null_handling "$stream_type")

    # Escape single quotes in description
    if [ -n "$description" ] && [ "$description" != "null" ]; then
        description=$(echo "$description" | sed "s/'/''/g")
        local desc_sql="'$description'"
    else
        local desc_sql="NULL"
    fi

    log "  Classification: $stream_id -> $stream_type ($correlation_role)"

    echo "-- Stream Classification: $stream_id"
    echo "INSERT INTO data_dictionary.stream_classification"
    echo "    (stream_id, stream_type, correlation_role, null_handling, description)"
    echo "VALUES"
    echo "    ('$stream_id', '$stream_type', '$correlation_role', '$null_handling', $desc_sql)"
    echo "ON CONFLICT (stream_id) DO UPDATE SET"
    echo "    stream_type = EXCLUDED.stream_type,"
    echo "    correlation_role = EXCLUDED.correlation_role,"
    echo "    null_handling = EXCLUDED.null_handling,"
    echo "    description = COALESCE(EXCLUDED.description, data_dictionary.stream_classification.description),"
    echo "    updated_at = NOW();"
    echo ""
}

# Sync gold table metadata to data dictionary (v11-002)
# Called when generating Gold DDL
sync_gold_table_metadata() {
    local table_name="$1"
    local object_type="$2"
    local source_silver_table="$3"
    local source_stream_type="$4"
    local granularity="$5"
    local description="$6"

    # Handle NULL values
    local source_silver_sql="NULL"
    local source_type_sql="NULL"
    local granularity_sql="NULL"
    local desc_sql="NULL"

    [ -n "$source_silver_table" ] && [ "$source_silver_table" != "null" ] && source_silver_sql="'$source_silver_table'"
    [ -n "$source_stream_type" ] && [ "$source_stream_type" != "null" ] && source_type_sql="'$source_stream_type'"
    [ -n "$granularity" ] && [ "$granularity" != "null" ] && granularity_sql="'$granularity'"

    if [ -n "$description" ] && [ "$description" != "null" ]; then
        description=$(echo "$description" | sed "s/'/''/g")
        desc_sql="'$description'"
    fi

    echo "-- Gold Table Metadata: $table_name"
    echo "INSERT INTO data_dictionary.gold_tables"
    echo "    (table_name, object_type, source_silver_table, source_stream_type, granularity, description)"
    echo "VALUES"
    echo "    ('$table_name', '$object_type', $source_silver_sql, $source_type_sql, $granularity_sql, $desc_sql)"
    echo "ON CONFLICT (table_name) DO UPDATE SET"
    echo "    object_type = EXCLUDED.object_type,"
    echo "    source_silver_table = EXCLUDED.source_silver_table,"
    echo "    source_stream_type = EXCLUDED.source_stream_type,"
    echo "    granularity = EXCLUDED.granularity,"
    echo "    description = COALESCE(EXCLUDED.description, data_dictionary.gold_tables.description),"
    echo "    updated_at = NOW();"
    echo ""
}

# Handle container restart declaration
# Args: $1 = declaration JSON
handle_container_restart() {
    local declaration="$1"
    local target=$(echo "$declaration" | jq -r '.target')

    log "Container Restart: $target"

    case "$target" in
        air-quality-app)
            dc restart air-quality-app
            wait_for_health air-quality-app 60
            ;;
        ndp-mcp-server)
            dc restart ndp-mcp-server
            wait_for_health ndp-mcp-server 60
            ;;
        silver-etl)
            # Silver ETL runs as one-shot, not a daemon - skip restart
            warn "  silver-etl is not a persistent service, skipping restart"
            ;;
        grafana)
            dc restart grafana
            wait_for_health grafana 60
            ;;
        *)
            error "Unknown container target: $target"
            ;;
    esac

    return 0
}

# Main apply function - orchestrates 9-phase deployment
# Args: $1 = manifest file (optional, defaults to .deploy/manifest.json)
apply() {
    local manifest_file="${1:-$REPO_ROOT/$MANIFEST_FILE}"

    log "=========================================="
    log "Declarative Deploy (dp-020)"
    log "Manifest: $manifest_file"
    log "=========================================="

    # Phase 1: Validation
    log ""
    log "Phase 1: Validation"
    log "-------------------"

    if ! validate_manifest "$manifest_file"; then
        return 0  # No changes to apply
    fi

    # Wait for infrastructure (skip in dry-run mode)
    if [ "${DRY_RUN:-false}" = "true" ]; then
        log "Skipping infrastructure readiness check (dry-run mode)"
    else
        log "Checking infrastructure readiness..."
        until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
            warn "Waiting for TimescaleDB to be ready..."
            sleep 2
        done
        until dcx etcd etcdctl endpoint health >/dev/null 2>&1; do
            warn "Waiting for etcd to be ready..."
            sleep 2
        done
        log "Infrastructure ready"
    fi

    # Parse manifest into phases (handle both old .changes and new .declarations formats)
    local container_builds=$(jq -c '[(.changes // [])[] | select(.type == "container" and .action == "build")]' "$manifest_file" 2>/dev/null || echo "[]")
    local tool_builds=$(jq -c '[(.changes // [])[] | select(.type == "tool")]' "$manifest_file" 2>/dev/null || echo "[]")
    local migrations=$(jq -c '[(.changes // [])[] | select(.type == "migration")]' "$manifest_file" 2>/dev/null || echo "[]")
    local silver_tables=$(jq -c '[(.changes // [])[] | select(.type == "silver-table")]' "$manifest_file" 2>/dev/null || echo "[]")
    local streams=$(jq -c '[(.changes // [])[] | select(.type == "stream")]' "$manifest_file" 2>/dev/null || echo "[]")
    local dimensions=$(jq -c '[(.changes // [])[] | select(.type == "dimensions")]' "$manifest_file" 2>/dev/null || echo "[]")
    local dictionary=$(jq -c '[(.changes // [])[] | select(.type == "dictionary")]' "$manifest_file" 2>/dev/null || echo "[]")
    local container_restarts=$(jq -c '[(.changes // [])[] | select(.type == "container" and .action == "restart")]' "$manifest_file" 2>/dev/null || echo "[]")

    # Phase 2: Container Builds
    local build_count=$(echo "$container_builds" | jq 'length' 2>/dev/null || echo "0")
    build_count=${build_count:-0}
    if [ "$build_count" -gt 0 ]; then
        log ""
        log "Phase 2: Container Builds ($build_count)"
        log "-------------------"
        echo "$container_builds" | jq -c '.[]' | while read -r decl; do
            handle_container_build "$decl"
        done
    fi

    # Phase 2.5: Tool Builds (fe-001 Phase B)
    # Builds Rust CLI tools required by later phases (e.g., ndp-gold-ddl for Gold Tables)
    local tool_count=$(echo "$tool_builds" | jq 'length' 2>/dev/null || echo "0")
    tool_count=${tool_count:-0}
    if [ "$tool_count" -gt 0 ]; then
        log ""
        log "Phase 2.5: Tool Builds ($tool_count)"
        log "-------------------"
        echo "$tool_builds" | jq -c '.[]' | while read -r decl; do
            handle_tool "$decl"
        done
    fi

    # Phase 3: Migrations
    local migration_count=$(echo "$migrations" | jq 'length' 2>/dev/null || echo "0")
    migration_count=${migration_count:-0}
    if [ "$migration_count" -gt 0 ]; then
        log ""
        log "Phase 3: Migrations ($migration_count)"
        log "-------------------"
        echo "$migrations" | jq -c '.[]' | while read -r decl; do
            handle_migration "$decl"
        done
    fi

    # Phase 4: Silver Tables
    local silver_count=$(echo "$silver_tables" | jq 'length' 2>/dev/null || echo "0")
    silver_count=${silver_count:-0}
    if [ "$silver_count" -gt 0 ]; then
        log ""
        log "Phase 4: Silver Tables ($silver_count)"
        log "-------------------"
        echo "$silver_tables" | jq -c '.[]' | while read -r decl; do
            handle_silver_table "$decl"
        done
    fi

    # Phase 5: Gold Tables (fe-001)
    # Parse gold-tables from both .changes array and .declarations format (for backwards compat)
    local gold_tables_changes=$(jq -c '[(.changes // [])[] | select(.type == "gold-tables")]' "$manifest_file" 2>/dev/null || echo "[]")
    local gold_tables_decl=$(jq -c '.declarations["gold-tables"] // []' "$manifest_file" 2>/dev/null || echo "[]")
    # Merge both sources
    local gold_tables=$(echo "[$gold_tables_changes, $gold_tables_decl]" | jq -c 'flatten')
    local gold_count=$(echo "$gold_tables" | jq 'length' 2>/dev/null || echo "0")
    gold_count=${gold_count:-0}
    if [ "$gold_count" -gt 0 ] && [ "$gold_tables" != "[]" ] && [ "$gold_tables" != "null" ]; then
        log ""
        log "Phase 5: Gold Tables ($gold_count)"
        log "-------------------"
        echo "$gold_tables" | jq -c '.[]' | while read -r decl; do
            handle_gold_table "$decl" || true
        done
    fi

    # Phase 6: Domains (fe-001)
    # Parse domains from declarations array (new manifest format)
    local domains=$(jq -c '.declarations["domains"] // []' "$manifest_file" 2>/dev/null || echo "[]")
    local domain_count=$(echo "$domains" | jq 'length' 2>/dev/null || echo "0")
    domain_count=${domain_count:-0}
    if [ "$domain_count" -gt 0 ] && [ "$domains" != "[]" ] && [ "$domains" != "null" ]; then
        log ""
        log "Phase 6: Domains ($domain_count)"
        log "-------------------"
        echo "$domains" | jq -c '.[]' | while read -r decl; do
            handle_domain "$decl" || true
        done
    fi

    # Phase 7: Streams
    local stream_count=$(echo "$streams" | jq 'length' 2>/dev/null || echo "0")
    stream_count=${stream_count:-0}
    if [ "$stream_count" -gt 0 ]; then
        log ""
        log "Phase 7: Streams ($stream_count)"
        log "-------------------"
        echo "$streams" | jq -c '.[]' | while read -r decl; do
            handle_stream "$decl"
        done
    fi

    # Phase 8: Dimensions
    local dim_count=$(echo "$dimensions" | jq 'length' 2>/dev/null || echo "0")
    dim_count=${dim_count:-0}
    if [ "$dim_count" -gt 0 ]; then
        log ""
        log "Phase 8: Dimensions ($dim_count)"
        log "-------------------"
        echo "$dimensions" | jq -c '.[]' | while read -r decl; do
            handle_dimensions "$decl"
        done
    fi

    # Phase 9: Dictionary
    local dict_count=$(echo "$dictionary" | jq 'length' 2>/dev/null || echo "0")
    dict_count=${dict_count:-0}
    if [ "$dict_count" -gt 0 ]; then
        log ""
        log "Phase 9: Dictionary ($dict_count)"
        log "-------------------"
        echo "$dictionary" | jq -c '.[]' | while read -r decl; do
            handle_dictionary "$decl"
        done
    fi

    # Phase 10: Container Restarts
    local restart_count=$(echo "$container_restarts" | jq 'length' 2>/dev/null || echo "0")
    restart_count=${restart_count:-0}
    if [ "$restart_count" -gt 0 ]; then
        log ""
        log "Phase 10: Container Restarts ($restart_count)"
        log "-------------------"
        echo "$container_restarts" | jq -c '.[]' | while read -r decl; do
            handle_container_restart "$decl"
        done
    fi

    # Phase 11: Device State Update (FR-R.4)
    log ""
    log "Phase 11: Device State Update"
    log "-------------------"

    # Extract release_version from manifest if present (FR-R.4.1)
    local release_version=$(jq -r '.release_version // empty' "$manifest_file")
    local deployed_version

    if [ -n "$release_version" ]; then
        # Use release_version from manifest (normalize with 'v' prefix)
        deployed_version="v${release_version#v}"
        log "  Release Version: $deployed_version (from manifest)"
    else
        # Fallback to git describe for ad-hoc manifests
        deployed_version=$(git -C "$REPO_ROOT" describe --tags --always 2>/dev/null || echo "unknown")
        log "  Version: $deployed_version (from git)"
    fi

    local deployed_at=$(date -Iseconds)
    log "  Deployed At: $deployed_at"

    # Calculate manifest hash for integrity tracking (FR-R.4.5)
    local manifest_hash=""
    if command -v sha256sum &> /dev/null; then
        manifest_hash=$(sha256sum "$manifest_file" | cut -d' ' -f1)
    elif command -v shasum &> /dev/null; then
        manifest_hash=$(shasum -a 256 "$manifest_file" | cut -d' ' -f1)
    fi
    log "  Manifest Hash: ${manifest_hash:0:16}..."

    # Get relative manifest path for tracking
    local manifest_relative="${manifest_file#$REPO_ROOT/}"
    log "  Manifest Path: $manifest_relative"

    # Update device state files (FR-R.4.1, FR-R.4.3, FR-R.4.5, FR-R.4.6)
    local state_dir="/var/ndp"

    # Create state directory if it doesn't exist
    if mkdir -p "$state_dir" 2>/dev/null; then
        # Write via temp file + rename for atomicity (FR-R.4.3)
        local temp_version=$(mktemp "$state_dir/deployed-version.XXXXXX" 2>/dev/null)
        local temp_at=$(mktemp "$state_dir/deployed-at.XXXXXX" 2>/dev/null)
        local temp_manifest=$(mktemp "$state_dir/manifest-applied.XXXXXX" 2>/dev/null)

        if [ -n "$temp_version" ] && [ -n "$temp_at" ] && [ -n "$temp_manifest" ]; then
            echo "$deployed_version" > "$temp_version"
            echo "$deployed_at" > "$temp_at"
            echo "$manifest_hash" > "$temp_manifest"

            mv "$temp_version" "$state_dir/deployed-version" 2>/dev/null || true
            mv "$temp_at" "$state_dir/deployed-at" 2>/dev/null || true
            mv "$temp_manifest" "$state_dir/manifest-applied" 2>/dev/null || true

            log "  Device state updated in $state_dir"
        else
            # Fallback to direct write if temp file creation fails
            echo "$deployed_version" > "$state_dir/deployed-version" 2>/dev/null || true
            echo "$deployed_at" > "$state_dir/deployed-at" 2>/dev/null || true
            echo "$manifest_hash" > "$state_dir/manifest-applied" 2>/dev/null || true
            log "  Device state updated (fallback mode)"
        fi
    else
        # Integration mode - echo state instead of writing
        log "  Device state (integration mode - not written):"
        log "    deployed-version: $deployed_version"
        log "    deployed-at: $deployed_at"
        log "    manifest-applied: $manifest_hash"
    fi

    log ""
    log "=========================================="
    log "Declarative Deploy Complete!"
    log "=========================================="

    return 0
}

# Main
case "${1:-deploy}" in
    -h|--help|help)
        echo "Neural Data Platform - Deployment Script"
        echo ""
        echo "Usage: $0 [command] [options]"
        echo ""
        echo "Environment (set via DEPLOY_ENV):"
        echo "  DEPLOY_ENV=pi          - Production on Raspberry Pi (default)"
        echo "  DEPLOY_ENV=integration - Local integration testing"
        echo ""
        echo "Core Commands:"
        echo "  deploy          - Full deploy (build + start all services)"
        echo "  start           - Start all services"
        echo "  stop            - Stop all services"
        echo "  logs            - View logs (follows)"
        echo "  status          - Check service health and URLs"
        echo "  build           - Build Docker images only"
        echo ""
        echo "Update Commands:"
        echo "  update [--no-cache] [target] - Pull latest from git and rebuild"
        echo "                    --no-cache: Force full rebuild (skip Docker cache)"
        echo "                    Targets: app, mcp, silver, all (default)"
        echo "  refresh         - Pull latest configs only (no rebuild, restarts Grafana)"
        echo ""
        echo "Configuration Commands:"
        echo "  sync            - Sync configuration to etcd"
        echo "  init-streams    - Initialize stream configurations in etcd"
        echo "  list-streams    - List configured streams from etcd"
        echo "  sync-dictionary - Sync entity schemas to TimescaleDB data dictionary"
        echo ""
        echo "Declarative Deploy (dp-020, fe-001):"
        echo "  apply [file] [--dry-run]"
        echo "                  - Apply changes from manifest (default: .deploy/manifest.json)"
        echo "                    Orchestrates 11 phases:"
        echo "                      1. Validation"
        echo "                      2. Container Builds"
        echo "                      3. Migrations"
        echo "                      4. Silver Tables"
        echo "                      5. Gold Tables (fe-001)"
        echo "                      6. Domains (fe-001)"
        echo "                      7. Streams"
        echo "                      8. Dimensions"
        echo "                      9. Dictionary"
        echo "                      10. Container Restarts"
        echo "                      11. Device State Update"
        echo "  version         - Show deployed version, timestamp, and manifest hash"
        echo ""
        echo "Dimension Commands:"
        echo "  sync-dimensions      - Sync dimension tables from config/base/dimensions/"
        echo "  list-dimensions      - List configured dimensions and sync status"
        echo "  dimension-status     - Show dimension sync status history"
        echo ""
        echo "Analytics Commands:"
        echo "  analytics       - Start DuckDB + Grafana analytics stack"
        echo "  rollback        - Stop and remove analytics stack (preserves volumes)"
        echo ""
        echo "Silver ETL Commands:"
        echo "  silver-migrate       - Run Silver Layer TimescaleDB schema migrations"
        echo "  silver-etl           - Run Silver ETL once (Bronze -> TimescaleDB)"
        echo "  silver-daemon        - Start Silver ETL in daemon mode (continuous)"
        echo "  silver-daemon-stop   - Stop Silver ETL daemon"
        echo "  silver-daemon-logs   - View Silver ETL daemon logs (follows)"
        echo "  silver-daemon-status - Check Silver ETL daemon status"
        echo ""
        echo "Environment Variables:"
        echo "  DEPLOY_ENV              - pi (default) or integration"
        echo "  SILVER_ETL_INTERVAL     - Daemon ETL interval in seconds (default: 300)"
        echo "  SILVER_ETL_PERSISTENCE  - Enable daemon run stats persistence (default: false)"
        echo ""
        echo "Examples:"
        echo "  $0                              - Deploy to Pi (production)"
        echo "  DEPLOY_ENV=integration $0       - Deploy locally (integration)"
        echo "  $0 status                       - Check service health"
        echo "  $0 update --no-cache silver     - Force rebuild silver-etl"
        exit 0
        ;;
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
            bash "$SCRIPT_DIR/configs/streams/list-streams.sh" $ETCD_CONTAINER
        else
            error "Stream listing script not found"
        fi
        ;;
    sync-dictionary)
        sync_to_data_dictionary
        ;;
    apply)
        shift  # remove 'apply' from args
        # Parse apply options
        APPLY_MANIFEST_ARG=""
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --dry-run)
                    export DRY_RUN=true
                    log "Dry-run mode enabled"
                    shift
                    ;;
                *)
                    APPLY_MANIFEST_ARG="$1"
                    shift
                    ;;
            esac
        done
        apply "$APPLY_MANIFEST_ARG"
        ;;
    sync-dimensions)
        sync_dimensions
        ;;
    list-dimensions)
        list_dimensions
        ;;
    dimension-status)
        dimension_status
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
        until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
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
        until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
            warn "Waiting for TimescaleDB to be ready..."
            sleep 2
        done
        # Run migrations via silver-etl migrate command
        dc --profile silver run --rm silver-etl migrate
        log "Silver migrations complete"
        ;;
    silver-daemon)
        log "Starting Silver ETL daemon mode..."
        # Ensure TimescaleDB is ready
        until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
            warn "Waiting for TimescaleDB to be ready..."
            sleep 2
        done
        # Start silver-etl-daemon service with silver-daemon profile
        dc --profile silver-daemon up -d silver-etl-daemon
        log "Silver ETL daemon started (interval: ${SILVER_ETL_INTERVAL:-300}s)"
        log "View logs: docker logs -f silver-etl-daemon"
        ;;
    silver-daemon-stop)
        log "Stopping Silver ETL daemon..."
        dc --profile silver-daemon stop silver-etl-daemon
        dc --profile silver-daemon rm -f silver-etl-daemon
        log "Silver ETL daemon stopped"
        ;;
    silver-daemon-logs)
        log "Silver ETL daemon logs:"
        docker logs -f silver-etl-daemon
        ;;
    silver-daemon-status)
        log "Silver ETL daemon status:"
        if docker ps -q -f name=silver-etl-daemon | grep -q .; then
            echo "  Status: Running"
            docker exec silver-etl-daemon ps aux 2>/dev/null || true
            echo ""
            log "Recent logs:"
            docker logs --tail 20 silver-etl-daemon 2>&1
        else
            echo "  Status: Not running"
            echo "  Start with: ./deploy.sh silver-daemon"
        fi
        ;;
    version)
        # Display deployed version information (FR-R.4.4)
        log "Deployed Version Information:"
        echo ""
        local state_dir="/var/ndp"
        if [ -f "$state_dir/deployed-version" ]; then
            echo "  Deployed Version: $(cat "$state_dir/deployed-version")"
            echo "  Deployed At:      $(cat "$state_dir/deployed-at" 2>/dev/null || echo 'unknown')"
            echo "  Manifest Hash:    $(cat "$state_dir/manifest-applied" 2>/dev/null || echo 'unknown')"
        else
            warn "No deployment state found in $state_dir"
            warn "Run './deploy.sh apply <manifest>' to deploy and track version."
            # Fallback to git info
            echo ""
            echo "  Git HEAD: $(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
            echo "  Git Tag:  $(git -C "$REPO_ROOT" describe --tags --exact-match 2>/dev/null || echo 'no tag')"
        fi
        echo ""
        ;;
    *)
        echo "Error: Unknown command '$1'"
        echo ""
        echo "Run '$0 --help' for usage information."
        exit 1
        ;;
esac
