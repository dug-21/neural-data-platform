#!/bin/bash
# List all configured streams from etcd
# Updated for dp-018 JSON config architecture
#
# Usage: ./list-streams.sh [etcd_container_name]
#
# Key Pattern (dp-018):
#   /streams/{stream_id}/config = <complete JSON blob>

set -e

ETCD_CONTAINER="${1:-etcd}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[STREAMS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Function to run etcdctl
run_etcdctl() {
    docker exec "$ETCD_CONTAINER" etcdctl "$@"
}

# Check if jq is available in the container or locally
HAS_JQ=false
if docker exec "$ETCD_CONTAINER" which jq >/dev/null 2>&1; then
    HAS_JQ=true
elif command -v jq &> /dev/null; then
    HAS_JQ=true
fi

log "Configured Streams (from /streams/*/config):"
echo ""

# Count for summary
STREAM_COUNT=0
NEW_FORMAT_COUNT=0
OLD_FORMAT_COUNT=0

# Get all stream configs from new key pattern (dp-018)
while IFS= read -r key; do
    [ -z "$key" ] && continue

    # Extract stream_id from key like /streams/air-quality/config
    stream_id=$(echo "$key" | sed 's|/streams/||' | sed 's|/config$||')

    # Get the JSON config
    config=$(run_etcdctl get "$key" --print-value-only 2>/dev/null)

    if [ -n "$config" ]; then
        STREAM_COUNT=$((STREAM_COUNT + 1))
        NEW_FORMAT_COUNT=$((NEW_FORMAT_COUNT + 1))

        # Parse JSON fields
        if [ "$HAS_JQ" = true ]; then
            # Use jq for proper JSON parsing
            description=$(echo "$config" | jq -r '.description // "N/A"' 2>/dev/null || echo "N/A")
            enabled=$(echo "$config" | jq -r '.enabled // false' 2>/dev/null || echo "false")
            version=$(echo "$config" | jq -r '.version // "N/A"' 2>/dev/null || echo "N/A")
            source_count=$(echo "$config" | jq -r '.sources | length // 0' 2>/dev/null || echo "0")
            retention=$(echo "$config" | jq -r '.retention_days // "N/A"' 2>/dev/null || echo "N/A")
        else
            # Fallback to grep for basic parsing (less reliable but works without jq)
            description=$(echo "$config" | grep -oP '"description"\s*:\s*"\K[^"]+' | head -1)
            enabled=$(echo "$config" | grep -oP '"enabled"\s*:\s*\K(true|false)' | head -1)
            version=$(echo "$config" | grep -oP '"version"\s*:\s*"\K[^"]+' | head -1)
            source_count="?"
            retention=$(echo "$config" | grep -oP '"retention_days"\s*:\s*\K[0-9]+' | head -1)
            [ -z "$description" ] && description="N/A"
            [ -z "$enabled" ] && enabled="false"
            [ -z "$version" ] && version="N/A"
            [ -z "$retention" ] && retention="N/A"
        fi

        # Color code by enabled status
        if [ "$enabled" = "true" ]; then
            status="${GREEN}ENABLED${NC}"
        else
            status="${YELLOW}DISABLED${NC}"
        fi

        echo -e "${BLUE}Stream:${NC} $stream_id"
        echo "  Description: $description"
        echo "  Version:     $version"
        echo "  Sources:     $source_count"
        echo "  Retention:   ${retention} days"
        echo -e "  Status:      $status"
        echo ""
    fi
done < <(run_etcdctl get --prefix "/streams/" --keys-only 2>/dev/null | grep "/config$")

# Check for legacy format streams (for migration visibility)
OLD_STREAMS=$(run_etcdctl get --prefix "/air-quality/streams/" --keys-only 2>/dev/null | grep "/id$" | wc -l || echo "0")
if [ "$OLD_STREAMS" -gt 0 ]; then
    echo ""
    warn "Legacy streams found at /air-quality/streams/ ($OLD_STREAMS streams)"
    warn "These use the deprecated key format from init-streams.sh"
    warn "Run 'scripts/cleanup-legacy-etcd-keys.sh' after verifying new format works"
    echo ""

    # Show legacy streams for visibility
    run_etcdctl get --prefix "/air-quality/streams/" --keys-only 2>/dev/null | grep "/id$" | while read key; do
        stream_path=$(dirname "$key")
        stream_id=$(run_etcdctl get "$key" --print-value-only 2>/dev/null)
        name=$(run_etcdctl get "$stream_path/name" --print-value-only 2>/dev/null || echo "Unknown")
        enabled=$(run_etcdctl get "$stream_path/enabled" --print-value-only 2>/dev/null || echo "false")

        if [ "$enabled" = "true" ]; then
            status="${GREEN}ENABLED${NC}"
        else
            status="${YELLOW}DISABLED${NC}"
        fi

        echo -e "${YELLOW}[LEGACY]${NC} $name [$stream_id] - $status"
        OLD_FORMAT_COUNT=$((OLD_FORMAT_COUNT + 1))
    done
    echo ""
fi

# Summary
echo ""
log "Summary:"
echo "  New format (dp-018): $NEW_FORMAT_COUNT streams"
if [ "$OLD_STREAMS" -gt 0 ]; then
    echo "  Legacy format:       $OLD_STREAMS streams (deprecated)"
fi
echo ""
echo "  Key pattern (new):  /streams/{stream_id}/config"
echo "  Key pattern (old):  /air-quality/streams/{stream_id}/* (deprecated)"
