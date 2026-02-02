#!/bin/bash
# Sync JSON Stream Configurations to etcd
# Part of dp-018: JSON Config Foundation
#
# This script syncs JSON stream configurations from config/base/streams/*/config.json
# to etcd at /streams/{stream_id}/config as complete JSON blobs (no transformation).
#
# Usage: ./sync-streams-to-etcd.sh [options]
#
# Options:
#   --mode docker|local    Execution mode (default: auto-detect)
#   --container NAME       Docker container name (default: etcd)
#   --endpoint URL         etcd endpoint for local mode (default: http://localhost:2379)
#   --validate             Validate JSON syntax before sync
#   --dry-run              Show what would be synced without writing
#   --verbose              Show detailed output
#   --help                 Show usage information
#
# Environment Variables:
#   ETCD_CONTAINER         Override container name for docker mode
#   ETCD_ENDPOINT          Override endpoint for local mode
#
# Architecture: ADR-018-001 JSON Pass-Through
#   - JSON file content equals etcd blob (no transformation)
#   - StreamRegistry expects: /streams/{stream_id}/config
#   - Value: Complete JSON blob deserializable to StreamConfig

set -e

# Script directory and repo root
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Defaults
MODE=""  # Will be auto-detected if not specified
ETCD_CONTAINER="${ETCD_CONTAINER:-etcd}"
ETCD_ENDPOINT="${ETCD_ENDPOINT:-http://localhost:2379}"
CONFIG_DIR="${REPO_ROOT}/config/base/streams"
VALIDATE=false
DRY_RUN=false
VERBOSE=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
SUCCESS_COUNT=0
FAILURE_COUNT=0
SKIP_COUNT=0

# Logging functions
log() { echo -e "${GREEN}[SYNC]${NC} $1"; }
info() { echo -e "${BLUE}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }
ok() { echo -e "${GREEN}[OK]${NC}   $1"; }
verbose() {
    if [ "$VERBOSE" = true ]; then
        echo -e "${BLUE}[DEBUG]${NC} $1"
    fi
}

# Show usage
show_help() {
    cat << EOF
Sync JSON Stream Configurations to etcd

Usage: $(basename "$0") [options]

Options:
  --mode docker|local    Execution mode (default: auto-detect)
  --container NAME       Docker container name (default: etcd)
  --endpoint URL         etcd endpoint for local mode (default: http://localhost:2379)
  --validate             Validate JSON syntax before sync
  --dry-run              Show what would be synced without writing
  --verbose              Show detailed output
  --help                 Show this help message

Environment Variables:
  ETCD_CONTAINER         Override container name for docker mode
  ETCD_ENDPOINT          Override endpoint for local mode

Examples:
  $(basename "$0")                           # Auto-detect mode
  $(basename "$0") --mode docker             # Force docker mode
  $(basename "$0") --mode local              # Force local mode
  $(basename "$0") --validate --dry-run      # Validate only
  ETCD_CONTAINER=integration-etcd $(basename "$0")  # Custom container

etcd Key Pattern:
  /streams/{stream_id}/config = <entire JSON blob>

EOF
    exit 0
}

# Parse command-line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --mode)
                MODE="$2"
                if [[ "$MODE" != "docker" && "$MODE" != "local" ]]; then
                    error "Invalid mode '$MODE'. Must be 'docker' or 'local'."
                    exit 1
                fi
                shift 2
                ;;
            --container)
                ETCD_CONTAINER="$2"
                shift 2
                ;;
            --endpoint)
                ETCD_ENDPOINT="$2"
                shift 2
                ;;
            --validate)
                VALIDATE=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --help|-h)
                show_help
                ;;
            *)
                error "Unknown option: $1"
                echo "Run '$(basename "$0") --help' for usage."
                exit 1
                ;;
        esac
    done
}

# Auto-detect execution mode
detect_mode() {
    if [ -n "$MODE" ]; then
        verbose "Mode explicitly set to: $MODE"
        return
    fi

    # Check if docker is available and etcd container is running
    if command -v docker &> /dev/null; then
        if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^${ETCD_CONTAINER}$"; then
            MODE="docker"
            verbose "Auto-detected mode: docker (container '$ETCD_CONTAINER' is running)"
            return
        fi
    fi

    # Check if etcdctl is available locally
    if command -v etcdctl &> /dev/null; then
        MODE="local"
        verbose "Auto-detected mode: local (etcdctl found)"
        return
    fi

    # In dry-run mode, we can proceed without etcd
    if [ "$DRY_RUN" = true ]; then
        MODE="dry-run"
        verbose "Dry-run mode: no etcd required"
        return
    fi

    error "Cannot detect execution mode."
    error "Either:"
    error "  1. Start the etcd container: docker start $ETCD_CONTAINER"
    error "  2. Install etcdctl locally"
    error "  3. Specify mode with --mode docker|local"
    exit 1
}

# Run etcdctl command based on mode
run_etcdctl() {
    if [ "$MODE" = "docker" ]; then
        docker exec "$ETCD_CONTAINER" etcdctl "$@"
    else
        etcdctl --endpoints="$ETCD_ENDPOINT" "$@"
    fi
}

# Run etcdctl with stdin support (for piping data)
run_etcdctl_stdin() {
    if [ "$MODE" = "docker" ]; then
        docker exec -i "$ETCD_CONTAINER" etcdctl "$@"
    else
        etcdctl --endpoints="$ETCD_ENDPOINT" "$@"
    fi
}

# Test etcd connectivity
test_connectivity() {
    verbose "Testing etcd connectivity..."

    if [ "$DRY_RUN" = true ]; then
        verbose "Dry-run mode: skipping connectivity test"
        return 0
    fi

    if ! run_etcdctl endpoint health >/dev/null 2>&1; then
        error "Cannot connect to etcd."
        if [ "$MODE" = "docker" ]; then
            error "Container '$ETCD_CONTAINER' may not be healthy."
            error "Check: docker exec $ETCD_CONTAINER etcdctl endpoint health"
        else
            error "Endpoint '$ETCD_ENDPOINT' is not reachable."
            error "Check: etcdctl --endpoints=$ETCD_ENDPOINT endpoint health"
        fi
        exit 1
    fi

    verbose "etcd connectivity OK"
}

# Validate JSON file
validate_json() {
    local file="$1"

    # Basic JSON syntax check using jq or python
    if command -v jq &> /dev/null; then
        if ! jq empty "$file" 2>/dev/null; then
            return 1
        fi
    elif command -v python3 &> /dev/null; then
        if ! python3 -c "import json; json.load(open('$file'))" 2>/dev/null; then
            return 1
        fi
    else
        # No JSON validator available - warn but don't fail
        warn "No JSON validator (jq or python3) available. Skipping validation."
        return 0
    fi

    return 0
}

# Extract stream_id from JSON config
get_stream_id() {
    local file="$1"
    local fallback="$2"

    # Try to extract stream_id from JSON
    local stream_id=""
    if command -v jq &> /dev/null; then
        stream_id=$(jq -r '.stream_id // empty' "$file" 2>/dev/null)
    elif command -v python3 &> /dev/null; then
        stream_id=$(python3 -c "import json; d=json.load(open('$file')); print(d.get('stream_id', ''))" 2>/dev/null)
    fi

    # Use directory name as fallback if stream_id not in JSON
    if [ -z "$stream_id" ]; then
        stream_id="$fallback"
    fi

    echo "$stream_id"
}

# Get file size for display
get_file_size() {
    local file="$1"
    local size

    if [[ "$OSTYPE" == "darwin"* ]]; then
        size=$(stat -f%z "$file" 2>/dev/null || echo "0")
    else
        size=$(stat -c%s "$file" 2>/dev/null || echo "0")
    fi

    # Convert to human-readable
    if [ "$size" -lt 1024 ]; then
        echo "${size}B"
    elif [ "$size" -lt 1048576 ]; then
        echo "$((size / 1024))KB"
    else
        echo "$((size / 1048576))MB"
    fi
}

# Sync a single stream config to etcd
sync_stream() {
    local config_file="$1"
    local dir_name="$2"

    # Get stream_id (prefer JSON field, fallback to directory name)
    local stream_id
    stream_id=$(get_stream_id "$config_file" "$dir_name")

    if [ -z "$stream_id" ]; then
        error "Cannot determine stream_id for $config_file"
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
        return 1
    fi

    # Validate JSON if requested
    if [ "$VALIDATE" = true ]; then
        if ! validate_json "$config_file"; then
            error "$stream_id: Invalid JSON syntax in $config_file"
            FAILURE_COUNT=$((FAILURE_COUNT + 1))
            return 1
        fi
        verbose "$stream_id: JSON validation passed"
    fi

    # Read entire JSON content
    local json_content
    json_content=$(cat "$config_file")

    local key="/streams/${stream_id}/config"
    local size
    size=$(get_file_size "$config_file")

    if [ "$DRY_RUN" = true ]; then
        info "Would sync: $stream_id -> $key ($size)"
        if [ "$VERBOSE" = true ]; then
            echo "  File: $config_file"
        fi
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        return 0
    fi

    # Store to etcd (use stdin mode for docker exec -i)
    if echo "$json_content" | run_etcdctl_stdin put "$key"; then
        ok "$stream_id -> $key ($size)"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    else
        error "$stream_id: Failed to store to etcd"
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
        return 1
    fi
}

# Main sync function
sync_all_streams() {
    log "Environment: $MODE$([ "$MODE" = "docker" ] && echo " (container: $ETCD_CONTAINER)" || echo " (endpoint: $ETCD_ENDPOINT)")"
    log "Config directory: $CONFIG_DIR"

    if [ "$DRY_RUN" = true ]; then
        log "DRY-RUN MODE: No changes will be made"
    fi

    if [ "$VALIDATE" = true ]; then
        log "Validation: enabled"
    fi

    echo ""

    # Check if config directory exists
    if [ ! -d "$CONFIG_DIR" ]; then
        error "Config directory not found: $CONFIG_DIR"
        exit 1
    fi

    # Process each stream directory
    for stream_dir in "$CONFIG_DIR"/*/; do
        # Skip if not a directory
        [ ! -d "$stream_dir" ] && continue

        local dir_name
        dir_name=$(basename "$stream_dir")

        # Look for config.json
        local config_file="$stream_dir/config.json"

        if [ ! -f "$config_file" ]; then
            # Check for YAML fallback (for migration purposes)
            if [ -f "$stream_dir/config.yaml" ]; then
                warn "$dir_name: Only config.yaml found. JSON config not yet migrated."
                warn "  Run: scripts/migrate-yaml-to-json.sh to convert"
            else
                verbose "$dir_name: No config.json found, skipping"
            fi
            SKIP_COUNT=$((SKIP_COUNT + 1))
            continue
        fi

        log "Syncing $dir_name..."
        sync_stream "$config_file" "$dir_name"
    done

    echo ""

    # Summary
    local total=$((SUCCESS_COUNT + FAILURE_COUNT))
    if [ "$DRY_RUN" = true ]; then
        log "Summary: $SUCCESS_COUNT/$total streams would be synced"
    else
        log "Summary: $SUCCESS_COUNT/$total streams synced successfully"
    fi

    if [ "$SKIP_COUNT" -gt 0 ]; then
        warn "Skipped: $SKIP_COUNT directories (no config.json)"
    fi

    if [ "$FAILURE_COUNT" -gt 0 ]; then
        error "Failed: $FAILURE_COUNT streams"
        return 1
    fi

    return 0
}

# Main execution
main() {
    parse_args "$@"
    detect_mode
    test_connectivity
    sync_all_streams
}

main "$@"
