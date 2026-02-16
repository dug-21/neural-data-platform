#!/bin/bash
# ============================================================================
# DDL Generator for Silver Layer Tables (dp-020)
# ============================================================================
# Generates idempotent DDL statements from stream configuration files.
# Designed to be sourced by deploy.sh or run standalone.
#
# Usage:
#   source ddl-generator.sh
#   generate_silver_ddl "air-quality"
#
# Or standalone:
#   ./ddl-generator.sh generate air-quality
#   ./ddl-generator.sh evolve air-quality     # Schema evolution only
#   ./ddl-generator.sh all                    # All Silver-enabled streams
# ============================================================================

# Get script directory for standalone mode
DDL_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DDL_REPO_ROOT="${DDL_REPO_ROOT:-$(cd "$DDL_SCRIPT_DIR/../.." && pwd)}"

# ============================================================================
# TYPE MAPPING
# Maps config types to PostgreSQL DDL types
# Reference: dp020-type-mapping from claude-flow memory
# Based on dp-019 SUPPORTED-VALUES-RESEARCH.md
# ============================================================================

map_type() {
    local config_type="$1"
    case "$config_type" in
        float|double_precision)
            echo "DOUBLE PRECISION"
            ;;
        real)
            echo "REAL"
            ;;
        smallint)
            echo "SMALLINT"
            ;;
        int|integer)
            echo "INTEGER"
            ;;
        bigint)
            echo "BIGINT"
            ;;
        string|text)
            echo "TEXT"
            ;;
        varchar)
            echo "VARCHAR"
            ;;
        bool|boolean)
            echo "BOOLEAN"
            ;;
        timestamp|timestamptz)
            echo "TIMESTAMPTZ"
            ;;
        json|jsonb)
            echo "JSONB"
            ;;
        "text[]"|"TEXT[]")
            echo "TEXT[]"
            ;;
        *)
            # Default to TEXT for unknown types
            echo "TEXT"
            ;;
    esac
}

# ============================================================================
# YAML HELPER FUNCTIONS
# These are copied from deploy.sh for standalone operation
# When sourced from deploy.sh, these are already available
# ============================================================================

if ! command -v yaml_get &> /dev/null; then

# Helper function to extract YAML/JSON values
# Uses jq for JSON files, yq for YAML, falls back to Python
yaml_get() {
    local file="$1"
    local key="$2"
    local default="$3"
    local result=""

    # For JSON files, use jq (more reliable)
    if [[ "$file" == *.json ]] && command -v jq &> /dev/null; then
        # Convert key.path.to.value to .key.path.to.value
        local jq_path=".${key}"
        result=$(jq -r "$jq_path // \"$default\"" "$file" 2>/dev/null)
    elif command -v yq &> /dev/null; then
        if yq --version 2>&1 | grep -q "mikefarah"; then
            result=$(yq eval ".$key // \"$default\"" "$file" 2>/dev/null)
        else
            result=$(yq -r ".$key // \"$default\"" "$file" 2>/dev/null)
        fi
    fi

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

    if [ -z "$result" ] || [ "$result" = "null" ]; then
        result=$(grep -E "^${key}:" "$file" 2>/dev/null | sed 's/^[^:]*: *//' | tr -d '"' || echo "$default")
    fi

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

    # For JSON files, use jq
    if [[ "$file" == *.json ]] && command -v jq &> /dev/null; then
        result=$(jq -r "$path // \"$default\"" "$file" 2>/dev/null)
    elif command -v yq &> /dev/null; then
        if yq --version 2>&1 | grep -q "mikefarah"; then
            result=$(yq eval "$path // \"$default\"" "$file" 2>/dev/null)
        else
            result=$(yq -r "$path // \"$default\"" "$file" 2>/dev/null)
        fi
    fi

    if [ -z "$result" ] || [ "$result" = "null" ]; then
        if command -v python3 &> /dev/null; then
            result=$(python3 -c "
import yaml
import re
try:
    with open('$file') as f:
        data = yaml.safe_load(f)
    path = '$path'.lstrip('.')
    parts = re.split(r'\.(?![^\[]*\])', path)
    val = data
    for part in parts:
        if val is None:
            break
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

fi  # end yaml helpers check

# ============================================================================
# CREATE TABLE DDL GENERATOR
# Generates CREATE TABLE IF NOT EXISTS statement
# Standard columns + field mappings from config
# ============================================================================

generate_create_table_ddl() {
    local stream_id="$1"
    local config_file="$2"
    local target_table="$3"

    # Extract schema and table name from fully-qualified name
    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"

    # Get timestamp target field (default: observation_time)
    local timestamp_col
    timestamp_col=$(yaml_get "$config_file" "silver_etl.timestamp.target_field" "observation_time")

    # Start CREATE TABLE statement
    echo "-- CREATE TABLE: ${target_table}"
    echo "CREATE TABLE IF NOT EXISTS ${target_table} ("
    echo "    -- Standard columns (all Silver tables)"
    echo "    ${timestamp_col} TIMESTAMPTZ NOT NULL,"
    echo "    ndp_id TEXT NOT NULL,"

    # Process field_mappings to add columns
    local fm_count
    fm_count=$(yaml_array_len "$config_file" "silver_etl.field_mappings")

    local i
    for i in $(seq 0 $((fm_count - 1))); do
        local target_column
        local col_type
        local col_nullable
        local col_description

        target_column=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].target_column" "")
        col_type=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].type" "text")
        col_nullable=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].nullable" "true")
        col_description=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].description" "")

        # Skip if no target_column
        [ -z "$target_column" ] || [ "$target_column" = "null" ] && continue

        # Map to PostgreSQL type
        local pg_type
        pg_type=$(map_type "$col_type")

        # Add NOT NULL constraint if not nullable
        local null_constraint=""
        [ "$col_nullable" = "false" ] && null_constraint=" NOT NULL"

        # Add inline comment if description exists
        local comment=""
        if [ -n "$col_description" ] && [ "$col_description" != "null" ]; then
            # Truncate long descriptions for inline comment
            local short_desc="${col_description:0:50}"
            [ ${#col_description} -gt 50 ] && short_desc="${short_desc}..."
            comment="  -- ${short_desc}"
        fi

        echo "    ${target_column} ${pg_type}${null_constraint},${comment}"
    done

    # Add standard metadata columns and close table
    echo "    -- Standard metadata columns"
    echo "    dq_flags TEXT[] DEFAULT '{}',"
    echo "    _bronze_id BIGINT,"
    echo "    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),"
    echo "    -- Primary key for deduplication"
    echo "    PRIMARY KEY (${timestamp_col}, ndp_id)"
    echo ");"
    echo ""
}

# ============================================================================
# INDEX DDL GENERATOR
# Generates index statements for Silver tables
# ============================================================================

generate_indexes_ddl() {
    local target_table="$1"
    local timestamp_col="${2:-observation_time}"

    # Extract table name for index naming
    local table_name="${target_table##*.}"

    echo "-- INDEXES: ${target_table}"
    echo "-- Composite index on timestamp and ndp_id (most common query pattern)"
    echo "CREATE INDEX IF NOT EXISTS idx_${table_name}_time_ndp"
    echo "    ON ${target_table} (${timestamp_col} DESC, ndp_id);"
    echo ""
    echo "-- GIN index on dq_flags for quality filtering"
    echo "CREATE INDEX IF NOT EXISTS idx_${table_name}_dq_flags"
    echo "    ON ${target_table} USING GIN (dq_flags);"
    echo ""
    echo "-- Index on ingestion time for incremental processing"
    echo "CREATE INDEX IF NOT EXISTS idx_${table_name}_ingested"
    echo "    ON ${target_table} (ingestion_time DESC);"
    echo ""
}

# ============================================================================
# HYPERTABLE DDL GENERATOR
# Converts regular table to TimescaleDB hypertable
# ============================================================================

generate_hypertable_ddl() {
    local target_table="$1"
    local timestamp_col="${2:-observation_time}"
    local chunk_interval="${3:-1 day}"

    echo "-- HYPERTABLE: ${target_table}"
    echo "-- Convert to hypertable (idempotent with if_not_exists)"
    echo "SELECT create_hypertable("
    echo "    '${target_table}',"
    echo "    '${timestamp_col}',"
    echo "    chunk_time_interval => INTERVAL '${chunk_interval}',"
    echo "    if_not_exists => TRUE"
    echo ");"
    echo ""
}

# ============================================================================
# POLICIES DDL GENERATOR
# Generates compression and retention policies
# ============================================================================

generate_policies_ddl() {
    local target_table="$1"
    local retention_days="${2:-90}"
    local compression_after_days="${3:-7}"

    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"

    echo "-- POLICIES: ${target_table}"
    echo ""
    echo "-- Enable compression on the hypertable"
    echo "ALTER TABLE ${target_table} SET ("
    echo "    timescaledb.compress,"
    echo "    timescaledb.compress_segmentby = 'ndp_id'"
    echo ");"
    echo ""

    # Compression policy with idempotent DO block
    cat <<'COMPRESSION_EOF'
-- Add compression policy (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
COMPRESSION_EOF
    echo "        WHERE hypertable_name = '${table_name}'"
    echo "        AND hypertable_schema = '${schema_name}'"
    cat <<'COMPRESSION_EOF2'
        AND proc_name = 'policy_compression'
    ) THEN
        PERFORM add_compression_policy(
COMPRESSION_EOF2
    echo "            '${target_table}',"
    echo "            INTERVAL '${compression_after_days} days'"
    cat <<'COMPRESSION_EOF3'
        );
        RAISE NOTICE 'Added compression policy';
    ELSE
        RAISE NOTICE 'Compression policy already exists';
    END IF;
END $$;

COMPRESSION_EOF3

    # Retention policy with idempotent DO block
    cat <<'RETENTION_EOF'
-- Add retention policy (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
RETENTION_EOF
    echo "        WHERE hypertable_name = '${table_name}'"
    echo "        AND hypertable_schema = '${schema_name}'"
    cat <<'RETENTION_EOF2'
        AND proc_name = 'policy_retention'
    ) THEN
        PERFORM add_retention_policy(
RETENTION_EOF2
    echo "            '${target_table}',"
    echo "            INTERVAL '${retention_days} days'"
    cat <<'RETENTION_EOF3'
        );
        RAISE NOTICE 'Added retention policy';
    ELSE
        RAISE NOTICE 'Retention policy already exists';
    END IF;
END $$;

RETENTION_EOF3
}

# ============================================================================
# PERMISSIONS DDL GENERATOR
# Grants access to ndp_app and grafana_reader roles
# ============================================================================

generate_permissions_ddl() {
    local target_table="$1"
    local schema_name="${target_table%%.*}"

    echo "-- PERMISSIONS: ${target_table}"
    echo ""
    echo "-- Grant permissions (idempotent, handles missing roles)"
    cat << EOF
DO \$\$
BEGIN
    -- Grant to ndp_app if role exists
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'ndp_app') THEN
        EXECUTE 'GRANT USAGE ON SCHEMA ${schema_name} TO ndp_app';
        EXECUTE 'GRANT SELECT, INSERT, UPDATE, DELETE ON ${target_table} TO ndp_app';
        EXECUTE 'GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA ${schema_name} TO ndp_app';
        RAISE NOTICE 'Granted permissions to ndp_app';
    ELSE
        RAISE NOTICE 'Role ndp_app does not exist, skipping permissions';
    END IF;

    -- Grant to grafana_reader if role exists
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'grafana_reader') THEN
        EXECUTE 'GRANT USAGE ON SCHEMA ${schema_name} TO grafana_reader';
        EXECUTE 'GRANT SELECT ON ${target_table} TO grafana_reader';
        EXECUTE 'GRANT SELECT ON ALL SEQUENCES IN SCHEMA ${schema_name} TO grafana_reader';
        RAISE NOTICE 'Granted permissions to grafana_reader';
    ELSE
        RAISE NOTICE 'Role grafana_reader does not exist, skipping permissions';
    END IF;
END \$\$;
EOF
    echo ""
}

# ============================================================================
# ADD COLUMN DDL GENERATOR
# Generates ADD COLUMN statements for schema evolution
# Uses information_schema check for idempotency
# ============================================================================

generate_add_column_ddl() {
    local target_table="$1"
    local column_name="$2"
    local column_type="$3"
    local nullable="${4:-true}"
    local default_value="$5"

    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"

    # Map config type to PostgreSQL type
    local pg_type
    pg_type=$(map_type "$column_type")

    # Build default clause
    local default_clause=""
    if [ -n "$default_value" ] && [ "$default_value" != "null" ]; then
        default_clause=" DEFAULT ${default_value}"
    fi

    # Build null constraint (only add NOT NULL if there's a default value)
    local null_constraint=""
    if [ "$nullable" = "false" ] && [ -n "$default_value" ]; then
        null_constraint=" NOT NULL"
    fi

    echo "-- Add column ${column_name} to ${target_table} if not exists"
    cat <<ADD_COL_EOF
DO \$\$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = '${schema_name}'
        AND table_name = '${table_name}'
        AND column_name = '${column_name}'
    ) THEN
        ALTER TABLE ${target_table}
        ADD COLUMN ${column_name} ${pg_type}${null_constraint}${default_clause};
        RAISE NOTICE 'Added column ${column_name} to ${target_table}';
    ELSE
        RAISE NOTICE 'Column ${column_name} already exists in ${target_table}';
    END IF;
END \$\$;

ADD_COL_EOF
}

# ============================================================================
# SCHEMA EVOLUTION DDL GENERATOR
# Compares config to existing table and generates ADD COLUMN statements
# ============================================================================

generate_schema_evolution_ddl() {
    local stream_id="$1"
    local config_file="$2"
    local target_table="$3"

    echo "-- ============================================================================"
    echo "-- SCHEMA EVOLUTION: ${target_table}"
    echo "-- Stream: ${stream_id}"
    echo "-- Adds new columns from config that don't exist in the table"
    echo "-- ============================================================================"
    echo ""

    # Process field_mappings
    local fm_count
    fm_count=$(yaml_array_len "$config_file" "silver_etl.field_mappings")

    local i
    for i in $(seq 0 $((fm_count - 1))); do
        local target_column
        local col_type
        local col_nullable

        target_column=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].target_column" "")
        col_type=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].type" "text")
        col_nullable=$(yaml_array_get "$config_file" ".silver_etl.field_mappings[$i].nullable" "true")

        # Skip if no target_column
        [ -z "$target_column" ] || [ "$target_column" = "null" ] && continue

        # Generate ADD COLUMN statement
        generate_add_column_ddl "$target_table" "$target_column" "$col_type" "$col_nullable"
    done
}

# ============================================================================
# TABLE EXISTS CHECK HELPER
# Returns SQL to check if table exists
# ============================================================================

check_table_exists_sql() {
    local target_table="$1"
    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"

    echo "SELECT EXISTS ("
    echo "    SELECT 1 FROM information_schema.tables"
    echo "    WHERE table_schema = '${schema_name}'"
    echo "    AND table_name = '${table_name}'"
    echo ") AS table_exists;"
}

# ============================================================================
# MAIN ENTRY POINT
# Generates complete DDL for a Silver table from stream config
# ============================================================================

generate_silver_ddl() {
    local stream_id="$1"
    local mode="${2:-full}"  # full, create, evolve

    # Locate config file (support both YAML and JSON)
    local config_dir="${DDL_REPO_ROOT}/config/base/streams/${stream_id}"
    local config_file=""

    if [ -f "${config_dir}/config.yaml" ]; then
        config_file="${config_dir}/config.yaml"
    elif [ -f "${config_dir}/config.yml" ]; then
        config_file="${config_dir}/config.yml"
    elif [ -f "${config_dir}/config.json" ]; then
        config_file="${config_dir}/config.json"
    else
        echo "-- ERROR: Config file not found in ${config_dir}" >&2
        return 1
    fi

    # Check if silver_etl is enabled
    local silver_enabled
    silver_enabled=$(yaml_get "$config_file" "silver_etl.enabled" "false")

    if [ "$silver_enabled" != "true" ]; then
        echo "-- SKIP: silver_etl not enabled for stream ${stream_id}" >&2
        return 0
    fi

    # Get target table
    local target_table
    target_table=$(yaml_get "$config_file" "silver_etl.target_table" "")

    if [ -z "$target_table" ] || [ "$target_table" = "null" ]; then
        echo "-- ERROR: No target_table specified for stream ${stream_id}" >&2
        return 1
    fi

    # Get timestamp column
    local timestamp_col
    timestamp_col=$(yaml_get "$config_file" "silver_etl.timestamp.target_field" "observation_time")

    # Get retention and compression settings
    local retention_days
    local compression_after_days
    retention_days=$(yaml_get "$config_file" "retention_days" "90")
    compression_after_days=$(yaml_get "$config_file" "compression_after_days" "7")

    # Ensure schema exists
    local schema_name="${target_table%%.*}"

    # Generate DDL header
    echo "-- ============================================================================"
    echo "-- NDP Silver Table DDL"
    echo "-- Stream: ${stream_id}"
    echo "-- Target: ${target_table}"
    echo "-- Mode: ${mode}"
    echo "-- Generated: $(date -Iseconds)"
    echo "-- ============================================================================"
    echo ""
    echo "BEGIN;"
    echo ""
    echo "-- Ensure schema exists"
    echo "CREATE SCHEMA IF NOT EXISTS ${schema_name};"
    echo ""

    case "$mode" in
        full|create)
            # Generate full table DDL
            generate_create_table_ddl "$stream_id" "$config_file" "$target_table"
            generate_indexes_ddl "$target_table" "$timestamp_col"
            generate_hypertable_ddl "$target_table" "$timestamp_col"
            generate_policies_ddl "$target_table" "$retention_days" "$compression_after_days"
            generate_permissions_ddl "$target_table"
            ;;
        evolve)
            # Schema evolution only
            generate_schema_evolution_ddl "$stream_id" "$config_file" "$target_table"
            ;;
        *)
            echo "-- ERROR: Unknown mode: ${mode}. Use: full, create, evolve" >&2
            return 1
            ;;
    esac

    echo "COMMIT;"
    echo ""
    echo "-- ============================================================================"
    echo "-- DDL generation complete for ${stream_id}"
    echo "-- ============================================================================"

    return 0
}

# ============================================================================
# GENERATE ALL STREAMS
# Iterates over all streams and generates DDL
# ============================================================================

generate_all_silver_ddl() {
    local mode="${1:-full}"
    local config_dir="${DDL_REPO_ROOT}/config/base/streams"

    echo "-- ============================================================================"
    echo "-- SILVER LAYER DDL - ALL STREAMS"
    echo "-- Mode: ${mode}"
    echo "-- Generated: $(date -Iseconds)"
    echo "-- ============================================================================"
    echo ""

    local stream_count=0
    local processed_count=0

    # Process each stream
    for stream_dir in "$config_dir"/*/; do
        if [ -d "$stream_dir" ]; then
            local stream_id
            stream_id=$(basename "$stream_dir")

            # Find config file
            local config_file=""
            if [ -f "$stream_dir/config.yaml" ]; then
                config_file="$stream_dir/config.yaml"
            elif [ -f "$stream_dir/config.yml" ]; then
                config_file="$stream_dir/config.yml"
            elif [ -f "$stream_dir/config.json" ]; then
                config_file="$stream_dir/config.json"
            else
                continue
            fi

            stream_count=$((stream_count + 1))

            # Check if silver_etl is enabled
            local silver_enabled
            silver_enabled=$(yaml_get "$config_file" "silver_etl.enabled" "false")

            if [ "$silver_enabled" = "true" ]; then
                echo "-- =========================================="
                echo "-- Processing stream: ${stream_id}"
                echo "-- =========================================="
                generate_silver_ddl "$stream_id" "$mode"
                echo ""
                processed_count=$((processed_count + 1))
            fi
        fi
    done

    echo "-- ============================================================================"
    echo "-- Summary: Processed ${processed_count}/${stream_count} streams with silver_etl enabled"
    echo "-- ============================================================================"
}

# ============================================================================
# GET EXISTING COLUMNS HELPER
# Returns columns that exist in a table (for schema diff)
# ============================================================================

get_existing_columns() {
    local schema_name="$1"
    local table_name="$2"

    # This needs to be run against the actual database
    # Returns SQL to get column list
    echo "SELECT column_name FROM information_schema.columns"
    echo "WHERE table_schema = '${schema_name}'"
    echo "AND table_name = '${table_name}'"
    echo "ORDER BY ordinal_position;"
}

# ============================================================================
# STANDALONE CLI
# ============================================================================

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    # Running as standalone script, not sourced
    case "${1:-}" in
        generate)
            if [ -n "${2:-}" ]; then
                generate_silver_ddl "$2" "${3:-full}"
            else
                echo "Usage: $0 generate <stream_id> [mode]"
                echo "  mode: full (default), create, evolve"
                exit 1
            fi
            ;;
        all)
            generate_all_silver_ddl "${2:-full}"
            ;;
        evolve)
            if [ -n "${2:-}" ]; then
                generate_silver_ddl "$2" "evolve"
            else
                echo "Usage: $0 evolve <stream_id>"
                exit 1
            fi
            ;;
        map-type)
            if [ -n "${2:-}" ]; then
                map_type "$2"
            else
                echo "Usage: $0 map-type <config_type>"
                echo "Example: $0 map-type double_precision"
                exit 1
            fi
            ;;
        help|--help|-h)
            cat <<HELP_EOF
DDL Generator for Silver Layer Tables (dp-020)

Usage: $0 <command> [options]

Commands:
  generate <stream_id> [mode]   Generate DDL for a specific stream
                                Modes: full (default), create, evolve
  all [mode]                    Generate DDL for all Silver-enabled streams
  evolve <stream_id>            Generate schema evolution DDL only
  map-type <config_type>        Show PostgreSQL type mapping

Examples:
  $0 generate air-quality       # Full DDL for air-quality stream
  $0 generate air-quality evolve # Schema evolution only
  $0 all                        # Full DDL for all streams
  $0 evolve outdoor-weather     # Evolution DDL for outdoor-weather
  $0 map-type double_precision  # Shows: DOUBLE PRECISION

Type Mapping:
  float, double_precision -> DOUBLE PRECISION
  real                    -> REAL
  integer, int            -> INTEGER
  smallint                -> SMALLINT
  bigint                  -> BIGINT
  text, string            -> TEXT
  varchar                 -> VARCHAR
  boolean, bool           -> BOOLEAN
  timestamptz, timestamp  -> TIMESTAMPTZ
  jsonb, json             -> JSONB
  text[]                  -> TEXT[]

Standard Columns (added to all Silver tables):
  - observation_time (or custom timestamp column)
  - ndp_id
  - dq_flags (TEXT[])
  - _bronze_id
  - ingestion_time
HELP_EOF
            ;;
        *)
            echo "Unknown command: ${1:-}"
            echo "Run '$0 help' for usage information."
            exit 1
            ;;
    esac
fi
