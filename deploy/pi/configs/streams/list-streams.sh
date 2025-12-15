#!/bin/bash
# List all configured streams from etcd
#
# Usage: ./list-streams.sh [etcd_container_name]

set -e

ETCD_CONTAINER="${1:-etcd}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[STREAMS]${NC} $1"; }

log "Configured Air Quality Streams:"
echo ""

# Get all stream IDs
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/air-quality/streams/" --keys-only | grep "/id$" | while read key; do
    stream_path=$(dirname "$key")
    stream_id=$(docker exec "$ETCD_CONTAINER" etcdctl get "$key" --print-value-only)

    # Get stream details
    name=$(docker exec "$ETCD_CONTAINER" etcdctl get "$stream_path/name" --print-value-only 2>/dev/null || echo "Unknown")
    device_id=$(docker exec "$ETCD_CONTAINER" etcdctl get "$stream_path/device_id" --print-value-only 2>/dev/null || echo "N/A")
    topic=$(docker exec "$ETCD_CONTAINER" etcdctl get "$stream_path/mqtt_topic" --print-value-only 2>/dev/null || echo "N/A")
    location=$(docker exec "$ETCD_CONTAINER" etcdctl get "$stream_path/location" --print-value-only 2>/dev/null || echo "N/A")
    enabled=$(docker exec "$ETCD_CONTAINER" etcdctl get "$stream_path/enabled" --print-value-only 2>/dev/null || echo "false")

    # Color code by enabled status
    if [ "$enabled" = "true" ]; then
        status="${GREEN}ENABLED${NC}"
    else
        status="${YELLOW}DISABLED${NC}"
    fi

    echo -e "${BLUE}Stream:${NC} $name"
    echo "  ID:       $stream_id"
    echo "  Device:   $device_id"
    echo "  Topic:    $topic"
    echo "  Location: $location"
    echo -e "  Status:   $status"
    echo ""
done

# Show global multi-stream config
log "Multi-Stream Configuration:"
multi_stream_enabled=$(docker exec "$ETCD_CONTAINER" etcdctl get "/air-quality/multi_stream/enabled" --print-value-only 2>/dev/null || echo "false")
max_streams=$(docker exec "$ETCD_CONTAINER" etcdctl get "/air-quality/multi_stream/max_concurrent_streams" --print-value-only 2>/dev/null || echo "N/A")
webhook_enabled=$(docker exec "$ETCD_CONTAINER" etcdctl get "/air-quality/multi_stream/webhook_enabled" --print-value-only 2>/dev/null || echo "false")
webhook_port=$(docker exec "$ETCD_CONTAINER" etcdctl get "/air-quality/multi_stream/webhook_port" --print-value-only 2>/dev/null || echo "N/A")

echo "  Enabled:        $multi_stream_enabled"
echo "  Max Streams:    $max_streams"
echo "  Webhook:        $webhook_enabled (port $webhook_port)"
