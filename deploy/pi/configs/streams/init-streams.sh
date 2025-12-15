#!/bin/bash
# Stream Configuration Initialization Script
# Loads stream configurations into etcd for multi-stream air quality monitoring
#
# Usage: ./init-streams.sh [etcd_container_name]

set -e

ETCD_CONTAINER="${1:-etcd}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[STREAM-INIT]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Wait for etcd to be ready
log "Waiting for etcd to be ready..."
until docker exec "$ETCD_CONTAINER" etcdctl endpoint health >/dev/null 2>&1; do
    warn "etcd not ready, retrying in 2s..."
    sleep 2
done

log "etcd is ready, loading stream configurations..."

# Function to load stream config into etcd
load_stream_config() {
    local stream_id=$1
    local stream_name=$2
    local device_id=$3
    local topic=$4
    local location=$5
    local description=$6

    log "Loading stream: $stream_name ($stream_id)"

    # Set stream metadata
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/id" "$stream_id"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/name" "$stream_name"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/device_id" "$device_id"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/mqtt_topic" "$topic"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/location" "$location"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/description" "$description"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/enabled" "true"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/created_at" "$(date -Iseconds)"

    # Set stream-specific storage config
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/storage/path" "/app/data/streams/$stream_id"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/storage/retention_days" "30"
    docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$stream_id/storage/compression" "true"
}

# Load default stream configurations
# Stream 1: Primary AirGradient sensor
load_stream_config \
    "airgradient-001" \
    "Office - Primary Sensor" \
    "84fce612f5f8" \
    "airgradient/readings/84fce612f5f8" \
    "Office - Main Floor" \
    "Primary air quality monitoring station in main office area"

# Stream 2: Secondary sensor (example - disabled by default)
load_stream_config \
    "airgradient-002" \
    "Conference Room Sensor" \
    "device-002" \
    "airgradient/readings/device-002" \
    "Conference Room" \
    "Air quality monitoring for conference room"

# Set to disabled initially
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/airgradient-002/enabled" "false"

# Set global multi-stream configuration
log "Setting global multi-stream configuration..."
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/multi_stream/enabled" "true"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/multi_stream/max_concurrent_streams" "10"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/multi_stream/webhook_enabled" "true"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/multi_stream/webhook_port" "8081"

log "Stream configurations loaded successfully!"

# Verify configurations
log "Verifying stream configurations..."
echo ""
log "Loaded streams:"
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/air-quality/streams/" --keys-only | grep "/id$" | while read key; do
    stream_id=$(docker exec "$ETCD_CONTAINER" etcdctl get "$key" --print-value-only)
    name_key="${key/\/id/\/name}"
    stream_name=$(docker exec "$ETCD_CONTAINER" etcdctl get "$name_key" --print-value-only 2>/dev/null || echo "Unknown")
    enabled_key="${key/\/id/\/enabled}"
    enabled=$(docker exec "$ETCD_CONTAINER" etcdctl get "$enabled_key" --print-value-only 2>/dev/null || echo "false")
    echo "  - $stream_name [$stream_id] (enabled: $enabled)"
done

echo ""
log "Multi-stream configuration:"
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/air-quality/multi_stream/" --print-value-only | while read value; do
    echo "  $value"
done

echo ""
log "Stream initialization complete!"
