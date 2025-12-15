#!/bin/bash
# Add a new stream configuration to etcd
#
# Usage: ./add-stream.sh <stream_id> <stream_name> <device_id> <mqtt_topic> <location> [description]

set -e

if [ $# -lt 5 ]; then
    echo "Usage: $0 <stream_id> <stream_name> <device_id> <mqtt_topic> <location> [description]"
    echo ""
    echo "Example:"
    echo "  $0 airgradient-003 \"Lab Sensor\" device-003 \"airgradient/readings/device-003\" \"Research Lab\" \"Lab monitoring\""
    exit 1
fi

STREAM_ID="$1"
STREAM_NAME="$2"
DEVICE_ID="$3"
MQTT_TOPIC="$4"
LOCATION="$5"
DESCRIPTION="${6:-Air quality monitoring stream}"

ETCD_CONTAINER="${ETCD_CONTAINER:-etcd}"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${GREEN}[ADD-STREAM]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Validate stream ID format
if [[ ! "$STREAM_ID" =~ ^[a-z0-9-]+$ ]]; then
    error "Stream ID must contain only lowercase letters, numbers, and hyphens"
fi

# Check if stream already exists
if docker exec "$ETCD_CONTAINER" etcdctl get "/air-quality/streams/$STREAM_ID/id" --print-value-only >/dev/null 2>&1; then
    warn "Stream $STREAM_ID already exists. Do you want to update it? (y/N)"
    read -r response
    if [[ ! "$response" =~ ^[Yy]$ ]]; then
        log "Cancelled."
        exit 0
    fi
fi

log "Adding stream: $STREAM_NAME ($STREAM_ID)"

# Set stream configuration
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/id" "$STREAM_ID"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/name" "$STREAM_NAME"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/device_id" "$DEVICE_ID"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/mqtt_topic" "$MQTT_TOPIC"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/location" "$LOCATION"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/description" "$DESCRIPTION"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/enabled" "true"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/created_at" "$(date -Iseconds)"

# Set storage configuration
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/storage/path" "/app/data/streams/$STREAM_ID"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/storage/retention_days" "30"
docker exec "$ETCD_CONTAINER" etcdctl put "/air-quality/streams/$STREAM_ID/storage/compression" "true"

log "Stream added successfully!"

# Display configuration
log "Stream configuration:"
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/air-quality/streams/$STREAM_ID/" --print-value-only | while read value; do
    echo "  $value"
done

log "Stream is now active and will start processing data from topic: $MQTT_TOPIC"
