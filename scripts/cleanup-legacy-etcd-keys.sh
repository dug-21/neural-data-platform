#!/bin/bash
# Cleanup legacy etcd keys from init-streams.sh
# Part of dp-018: JSON Config Foundation
#
# This script removes the old key format at /air-quality/streams/*
# Run AFTER verifying the new /streams/*/config format works correctly.
#
# Usage: ./cleanup-legacy-etcd-keys.sh [options]
#
# Options:
#   --container NAME    Docker container name (default: etcd)
#   --dry-run           Show what would be deleted without deleting
#   --force             Delete without confirmation prompt
#   --help              Show usage information

set -e

# Defaults
ETCD_CONTAINER="${ETCD_CONTAINER:-etcd}"
DRY_RUN=false
FORCE=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[CLEANUP]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --container)
            ETCD_CONTAINER="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --force)
            FORCE=true
            shift
            ;;
        --help|-h)
            echo "Cleanup legacy etcd keys from init-streams.sh"
            echo ""
            echo "Usage: $(basename "$0") [options]"
            echo ""
            echo "Options:"
            echo "  --container NAME    Docker container name (default: etcd)"
            echo "  --dry-run           Show what would be deleted without deleting"
            echo "  --force             Delete without confirmation prompt"
            echo "  --help              Show this help message"
            echo ""
            echo "This script removes:"
            echo "  /air-quality/streams/*     - Old per-field stream configs"
            echo "  /air-quality/multi_stream/* - Old multi-stream settings"
            echo ""
            echo "The new format (dp-018) uses:"
            echo "  /streams/{stream_id}/config - Complete JSON blob"
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Run etcdctl
run_etcdctl() {
    docker exec "$ETCD_CONTAINER" etcdctl "$@"
}

# Check if container is running
if ! docker ps --format '{{.Names}}' | grep -q "^${ETCD_CONTAINER}$"; then
    error "etcd container '$ETCD_CONTAINER' is not running"
    exit 1
fi

log "Checking for legacy keys in etcd..."
echo ""

# Collect legacy keys
LEGACY_STREAM_KEYS=$(run_etcdctl get --prefix "/air-quality/streams/" --keys-only 2>/dev/null || echo "")
LEGACY_MULTI_KEYS=$(run_etcdctl get --prefix "/air-quality/multi_stream/" --keys-only 2>/dev/null || echo "")

# Count keys
STREAM_KEY_COUNT=$(echo "$LEGACY_STREAM_KEYS" | grep -c . || echo "0")
MULTI_KEY_COUNT=$(echo "$LEGACY_MULTI_KEYS" | grep -c . || echo "0")
TOTAL_COUNT=$((STREAM_KEY_COUNT + MULTI_KEY_COUNT))

if [ "$TOTAL_COUNT" -eq 0 ]; then
    log "No legacy keys found. Nothing to clean up."
    exit 0
fi

log "Found $TOTAL_COUNT legacy keys:"
echo ""

# Show stream keys
if [ "$STREAM_KEY_COUNT" -gt 0 ]; then
    echo "  /air-quality/streams/* ($STREAM_KEY_COUNT keys):"
    echo "$LEGACY_STREAM_KEYS" | head -20 | sed 's/^/    /'
    if [ "$STREAM_KEY_COUNT" -gt 20 ]; then
        echo "    ... and $((STREAM_KEY_COUNT - 20)) more"
    fi
    echo ""
fi

# Show multi_stream keys
if [ "$MULTI_KEY_COUNT" -gt 0 ]; then
    echo "  /air-quality/multi_stream/* ($MULTI_KEY_COUNT keys):"
    echo "$LEGACY_MULTI_KEYS" | sed 's/^/    /'
    echo ""
fi

# Dry-run mode
if [ "$DRY_RUN" = true ]; then
    log "DRY-RUN: Would delete $TOTAL_COUNT keys"
    log "Run without --dry-run to actually delete"
    exit 0
fi

# Confirmation prompt
if [ "$FORCE" != true ]; then
    echo ""
    warn "Before deleting, verify that the new format works:"
    echo "  1. Run: ./deploy.sh list-streams"
    echo "  2. Check that all streams appear under 'New format (dp-018)'"
    echo "  3. Test that the application loads configs correctly"
    echo ""
    read -p "Delete these $TOTAL_COUNT legacy keys? (y/N) " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        log "Cleanup cancelled"
        exit 0
    fi
fi

# Delete keys
log "Deleting legacy keys..."

if [ "$STREAM_KEY_COUNT" -gt 0 ]; then
    run_etcdctl del --prefix "/air-quality/streams/" >/dev/null
    log "Deleted /air-quality/streams/* ($STREAM_KEY_COUNT keys)"
fi

if [ "$MULTI_KEY_COUNT" -gt 0 ]; then
    run_etcdctl del --prefix "/air-quality/multi_stream/" >/dev/null
    log "Deleted /air-quality/multi_stream/* ($MULTI_KEY_COUNT keys)"
fi

echo ""
log "Legacy keys deleted successfully!"
log "Run './deploy.sh list-streams' to verify only new format remains."
