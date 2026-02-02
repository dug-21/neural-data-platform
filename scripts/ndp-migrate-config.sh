#!/usr/bin/env bash
# =============================================================================
# ndp-migrate-config.sh - Schema Migration v1.1 -> v2.0
# =============================================================================
#
# dp-021 Phase 5: Schema Migration
# Migrates stream configurations from v1.1 (with deprecated entity_schemas)
# to v2.0 (entity_schemas forbidden, enriched fields required).
#
# Requirements:
#   - jq (JSON processor)
#   - bash 4.0+
#
# Exit Codes:
#   0 - Success (or nothing to migrate)
#   1 - Migration/validation error
#   2 - Nothing to migrate (already v2.0)
#
# Usage:
#   ndp-migrate-config.sh [OPTIONS] [CONFIG_PATH]
#
# Options:
#   --dry-run     Preview changes without writing files
#   --all         Migrate all configs in config/base/streams/
#   --from VER    Source version (default: auto-detect)
#   --to VER      Target version (default: 2)
#   --no-backup   Skip creating backup files
#   --help        Show usage information
#
# Examples:
#   ndp-migrate-config.sh config/base/streams/air-quality/config.json
#   ndp-migrate-config.sh --dry-run --all
#   ndp-migrate-config.sh --all
#
# =============================================================================

set -euo pipefail

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_DIR="${REPO_ROOT}/config/base/streams"
SCHEMA_DIR="${REPO_ROOT}/schemas"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
MIGRATED=0
SKIPPED=0
FAILED=0
ALREADY_V2=0

# -----------------------------------------------------------------------------
# Logging functions
# -----------------------------------------------------------------------------
log() { echo -e "${GREEN}[migrate]${NC} $*"; }
warn() { echo -e "${YELLOW}[migrate]${NC} $*"; }
error() { echo -e "${RED}[migrate]${NC} $*" >&2; }
info() { echo -e "${BLUE}[migrate]${NC} $*"; }

# -----------------------------------------------------------------------------
# Usage
# -----------------------------------------------------------------------------
usage() {
    cat << 'EOF'
Usage: ndp-migrate-config.sh [OPTIONS] [CONFIG_PATH]

Migrate stream configs from v1.1 to v2.0 (removes entity_schemas)

Options:
  --dry-run     Preview changes without writing files
  --all         Migrate all configs in config/base/streams/
  --from VER    Source version (default: auto-detect)
  --to VER      Target version (default: 2)
  --no-backup   Skip creating backup files
  --verbose     Show detailed output
  --help        Show this help

Arguments:
  CONFIG_PATH   Path to a single config file to migrate
                (mutually exclusive with --all)

Exit Codes:
  0  Success (migrations completed or nothing needed)
  1  Migration/validation error
  2  Nothing to migrate (all configs already at v2.0)

Examples:
  ndp-migrate-config.sh config/base/streams/air-quality/config.json
  ndp-migrate-config.sh --dry-run --all
  ndp-migrate-config.sh --all
  ndp-migrate-config.sh --dry-run config.json

Migration Details:
  v1.1 -> v2.0 changes:
    - Removes deprecated entity_schemas section
    - Sets config_version to 2
    - Validates fields have description (required in v2.0)
EOF
}

# -----------------------------------------------------------------------------
# Check dependencies
# -----------------------------------------------------------------------------
check_dependencies() {
    if ! command -v jq &> /dev/null; then
        error "jq is required but not installed. Install with: apt-get install jq"
        exit 1
    fi
}

# -----------------------------------------------------------------------------
# Detect config version
# -----------------------------------------------------------------------------
detect_version() {
    local config_file="$1"

    # Check for explicit config_version field
    local version
    version=$(jq -r '.config_version // empty' "$config_file" 2>/dev/null)

    if [[ -n "$version" ]]; then
        # Normalize version string
        case "$version" in
            2|"2"|"2.0") echo "2.0" ;;
            1|"1"|"1.1") echo "1.1" ;;
            *) echo "$version" ;;
        esac
        return
    fi

    # Infer version from structure
    local has_entity_schemas
    has_entity_schemas=$(jq 'has("entity_schemas")' "$config_file")

    if [[ "$has_entity_schemas" == "true" ]]; then
        echo "1.1"
    else
        # Check if fields have descriptions (v2.0 indicator)
        local has_descriptions
        has_descriptions=$(jq '[.fields[] | has("description")] | all' "$config_file" 2>/dev/null || echo "false")

        if [[ "$has_descriptions" == "true" ]]; then
            echo "2.0"
        else
            echo "1.1"
        fi
    fi
}

# -----------------------------------------------------------------------------
# Validate config is ready for v2.0 migration
# -----------------------------------------------------------------------------
validate_v2_ready() {
    local config_file="$1"
    local stream_id="$2"
    local errors=()

    # Check all fields have descriptions (required for v2.0)
    local fields_without_desc
    fields_without_desc=$(jq -r '
        .fields[]
        | select(.description == null or .description == "")
        | .name
    ' "$config_file" 2>/dev/null | tr '\n' ', ')

    if [[ -n "$fields_without_desc" ]]; then
        # Trim trailing comma
        fields_without_desc="${fields_without_desc%,}"
        errors+=("Fields missing description: $fields_without_desc")
    fi

    # Check stream_id exists
    local stream_id_value
    stream_id_value=$(jq -r '.stream_id // empty' "$config_file")
    if [[ -z "$stream_id_value" ]]; then
        errors+=("Missing required field: stream_id")
    fi

    # Check fields array exists and has items
    local fields_count
    fields_count=$(jq '.fields | length' "$config_file" 2>/dev/null || echo "0")
    if [[ "$fields_count" -eq 0 ]]; then
        errors+=("Fields array is empty or missing")
    fi

    if [[ ${#errors[@]} -gt 0 ]]; then
        for err in "${errors[@]}"; do
            error "  $stream_id: $err"
        done
        return 1
    fi

    return 0
}

# -----------------------------------------------------------------------------
# Transform v1.1 -> v2.0
# -----------------------------------------------------------------------------
transform_v1_1_to_v2() {
    local input="$1"

    jq '
        # Remove entity_schemas section
        del(.entity_schemas) |
        # Set config_version to integer 2
        .config_version = 2
    ' "$input"
}

# -----------------------------------------------------------------------------
# Show diff for dry-run
# -----------------------------------------------------------------------------
show_diff() {
    local config_file="$1"
    local stream_id="$2"

    info "  Changes for $stream_id:"

    # Count entity_schemas entries
    local entity_count
    entity_count=$(jq '.entity_schemas | length // 0' "$config_file" 2>/dev/null || echo "0")

    if [[ "$entity_count" -gt 0 ]]; then
        info "    - Remove entity_schemas section ($entity_count entries)"
    fi

    # Show config_version change
    local old_version
    old_version=$(jq -r '.config_version // "unset"' "$config_file")
    info "    - Set config_version: $old_version -> 2"

    # Show size change (compare pretty-printed to pretty-printed)
    local old_size new_size
    old_size=$(jq '.' "$config_file" | wc -c)
    new_size=$(transform_v1_1_to_v2 "$config_file" | wc -c)
    local size_change=$((old_size - new_size))
    info "    - Size: $old_size -> $new_size bytes (+$size_change bytes saved)"
}

# -----------------------------------------------------------------------------
# Migrate a single config file
# -----------------------------------------------------------------------------
migrate_config() {
    local config_file="$1"
    local dry_run="$2"
    local create_backup="$3"
    local verbose="$4"

    # Get stream_id from directory name or file content
    local stream_id
    if [[ "$(basename "$config_file")" == "config.json" ]]; then
        stream_id=$(basename "$(dirname "$config_file")")
    else
        stream_id=$(jq -r '.stream_id // "unknown"' "$config_file" 2>/dev/null)
    fi

    # Check file exists
    if [[ ! -f "$config_file" ]]; then
        error "$stream_id: Config file not found: $config_file"
        ((FAILED++))
        return 1
    fi

    # Validate JSON syntax
    if ! jq empty "$config_file" 2>/dev/null; then
        error "$stream_id: Invalid JSON in $config_file"
        ((FAILED++))
        return 1
    fi

    # Detect current version
    local current_version
    current_version=$(detect_version "$config_file")

    # Check if already v2.0
    if [[ "$current_version" == "2.0" ]]; then
        if [[ "$verbose" == "true" ]]; then
            log "$stream_id: Already at v2.0, skipping"
        fi
        ((ALREADY_V2++))
        return 0
    fi

    # Check if unsupported version
    if [[ "$current_version" != "1.1" ]]; then
        error "$stream_id: Unsupported version '$current_version'. Only v1.1 -> v2.0 supported."
        ((FAILED++))
        return 1
    fi

    # Validate ready for v2.0
    if ! validate_v2_ready "$config_file" "$stream_id"; then
        error "$stream_id: Not ready for v2.0 migration. Ensure all fields have descriptions."
        ((FAILED++))
        return 1
    fi

    if [[ "$dry_run" == "true" ]]; then
        log "$stream_id: Would migrate v$current_version -> v2.0"
        show_diff "$config_file" "$stream_id"
        ((MIGRATED++))
        return 0
    fi

    # Perform migration
    log "$stream_id: Migrating v$current_version -> v2.0"

    # Create backup if requested
    if [[ "$create_backup" == "true" ]]; then
        local backup_file="${config_file}.v${current_version}.bak"
        cp "$config_file" "$backup_file"
        if [[ "$verbose" == "true" ]]; then
            info "  Backup created: $backup_file"
        fi
    fi

    # Transform to temp file
    local temp_file="${config_file}.tmp"
    if ! transform_v1_1_to_v2 "$config_file" > "$temp_file"; then
        error "$stream_id: Transform failed"
        rm -f "$temp_file"
        ((FAILED++))
        return 1
    fi

    # Validate transformed output is valid JSON
    if ! jq empty "$temp_file" 2>/dev/null; then
        error "$stream_id: Transform produced invalid JSON"
        rm -f "$temp_file"
        ((FAILED++))
        return 1
    fi

    # Validate against v2.0 schema if ndp-validate is available
    if command -v ndp-validate &> /dev/null; then
        if [[ -f "$SCHEMA_DIR/stream-config.v2.schema.json" ]]; then
            if ! ndp-validate --schema-path "$SCHEMA_DIR/stream-config.v2.schema.json" "$temp_file" &>/dev/null; then
                warn "$stream_id: Output does not pass v2.0 schema validation (continuing anyway)"
            fi
        fi
    fi

    # Atomic move
    mv "$temp_file" "$config_file"

    if [[ "$verbose" == "true" ]]; then
        log "  Migrated successfully"
    fi

    ((MIGRATED++))
    return 0
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------
main() {
    local from_version="auto"
    local to_version="2"
    local dry_run="false"
    local create_backup="true"
    local process_all="false"
    local config_path=""
    local verbose="false"

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --from)
                from_version="$2"
                shift 2
                ;;
            --to)
                to_version="$2"
                shift 2
                ;;
            --dry-run)
                dry_run="true"
                shift
                ;;
            --all)
                process_all="true"
                shift
                ;;
            --no-backup)
                create_backup="false"
                shift
                ;;
            --verbose)
                verbose="true"
                shift
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            -*)
                error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                if [[ -n "$config_path" ]]; then
                    error "Multiple config paths specified. Use --all for batch migration."
                    exit 1
                fi
                config_path="$1"
                shift
                ;;
        esac
    done

    # Check dependencies
    check_dependencies

    # Validate arguments
    if [[ "$process_all" == "true" && -n "$config_path" ]]; then
        error "Cannot use --all with a config path"
        usage
        exit 1
    fi

    if [[ "$process_all" == "false" && -z "$config_path" ]]; then
        error "Must specify a config path or use --all"
        usage
        exit 1
    fi

    # Only v1.1 -> v2.0 migration is currently supported
    if [[ "$to_version" != "2" ]]; then
        error "Only migration to v2.0 is currently supported"
        exit 1
    fi

    # Header
    log "Config schema migration: v1.1 -> v2.0"
    if [[ "$dry_run" == "true" ]]; then
        warn "DRY RUN - no changes will be made"
    fi
    echo ""

    # Build list of configs to process
    local configs=()

    if [[ "$process_all" == "true" ]]; then
        # Find all config.json files in streams directory
        if [[ ! -d "$CONFIG_DIR" ]]; then
            error "Config directory not found: $CONFIG_DIR"
            exit 1
        fi

        while IFS= read -r -d '' config_file; do
            configs+=("$config_file")
        done < <(find "$CONFIG_DIR" -name "config.json" -print0)

        if [[ ${#configs[@]} -eq 0 ]]; then
            warn "No config files found in $CONFIG_DIR"
            exit 0
        fi

        log "Found ${#configs[@]} config(s) to process"
    else
        # Single config file
        if [[ ! -f "$config_path" ]]; then
            # Try relative to config dir
            if [[ -f "$CONFIG_DIR/$config_path/config.json" ]]; then
                config_path="$CONFIG_DIR/$config_path/config.json"
            elif [[ -f "$REPO_ROOT/$config_path" ]]; then
                config_path="$REPO_ROOT/$config_path"
            else
                error "Config file not found: $config_path"
                exit 1
            fi
        fi
        configs+=("$config_path")
    fi

    # Process each config
    for config_file in "${configs[@]}"; do
        migrate_config "$config_file" "$dry_run" "$create_backup" "$verbose" || true
    done

    # Summary
    echo ""
    log "Migration summary:"
    log "  Migrated:   $MIGRATED"
    log "  Already v2: $ALREADY_V2"
    log "  Skipped:    $SKIPPED"
    log "  Failed:     $FAILED"

    # Exit code
    if [[ "$FAILED" -gt 0 ]]; then
        exit 1
    elif [[ "$MIGRATED" -eq 0 && "$ALREADY_V2" -gt 0 ]]; then
        log "All configs already at v2.0 - nothing to migrate"
        exit 2
    else
        exit 0
    fi
}

main "$@"
