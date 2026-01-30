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
#   sync            - Sync configuration to etcd
#   init-streams    - Initialize stream configurations in etcd
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
else
    COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
    ENV_NAME="production"
    # Container names for external scripts that can't use docker compose exec
    ETCD_CONTAINER="etcd"
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

    if command -v yq &> /dev/null; then
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

    if command -v yq &> /dev/null; then
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

# Show environment on startup
log "Environment: $DEPLOY_ENV (compose: $(basename $COMPOSE_FILE))"

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

    # Run the sync script from the repo root
    if [ -f "$REPO_ROOT/scripts/sync-config-to-etcd.sh" ]; then
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-config-to-etcd.sh" $ENV_NAME
    else
        warn "Config sync script not found, skipping"
    fi
}

init_streams() {
    log "Initializing stream configurations..."

    # Wait for etcd to be ready
    until dcx etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd to be ready..."
        sleep 2
    done

    # Check if streams are already initialized (informational only)
    if dcx etcd etcdctl get --prefix "/air-quality/streams/" --keys-only >/dev/null 2>&1; then
        stream_count=$(dcx etcd etcdctl get --prefix "/air-quality/streams/" --keys-only | grep -c "/id$" || echo "0")
        if [ "$stream_count" -gt 0 ]; then
            log "Updating existing stream configurations ($stream_count streams found)"
        fi
    fi

    # Run stream initialization script
    if [ -f "$SCRIPT_DIR/configs/streams/init-streams.sh" ]; then
        bash "$SCRIPT_DIR/configs/streams/init-streams.sh" $ETCD_CONTAINER
    else
        warn "Stream initialization script not found at $SCRIPT_DIR/configs/streams/init-streams.sh"
        warn "Multi-stream mode enabled but no streams configured!"
    fi
}

sync_to_data_dictionary() {
    log "Syncing Data Dictionary to TimescaleDB..."

    # Check if TimescaleDB is running
    until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
        warn "Waiting for TimescaleDB to be ready..."
        sleep 2
    done

    local CONFIG_DIR="$REPO_ROOT/config/base/streams"
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

    log "Importing dimension data to ${schema_name}.${table_name}..."

    if [ "$strategy" = "truncate_and_load" ]; then
        # Truncate existing data
        dcx timescaledb psql -U postgres -d ndp -c \
            "TRUNCATE TABLE ${schema_name}.${table_name};" 2>/dev/null || true
    fi

    # Import CSV data using COPY
    # Note: We copy the file into the container first, then import
    local temp_file="/tmp/dim_import_$$.csv"
    docker cp "$source_file" "$(docker compose -f "$COMPOSE_FILE" ps -q timescaledb):$temp_file"

    if dcx timescaledb psql -U postgres -d ndp -c \
        "\\COPY ${schema_name}.${table_name} FROM '$temp_file' WITH (FORMAT csv, HEADER true);" 2>/dev/null; then
        log "Successfully imported dimension data to ${schema_name}.${table_name}"
        # Clean up temp file in container
        dcx timescaledb rm -f "$temp_file" 2>/dev/null || true
        return 0
    else
        warn "Failed to import dimension data to ${schema_name}.${table_name}"
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
    docker exec air-quality-app du -sh /data 2>/dev/null || echo "  Not available"
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
    docker restart grafana

    log "Refresh complete!"
    status
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
    *)
        echo "Error: Unknown command '$1'"
        echo ""
        echo "Run '$0 --help' for usage information."
        exit 1
        ;;
esac
