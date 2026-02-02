#!/bin/bash
# Add a new stream configuration to etcd
#
# ============================================================================
# DEPRECATED: dp-018 JSON Config Foundation
# ============================================================================
#
# This script is DEPRECATED. To add a new stream:
#
#   1. Create a new directory: config/base/streams/{stream_id}/
#   2. Create config.json in that directory (copy from an existing stream)
#   3. Run: ./deploy.sh sync
#   4. Verify: ./deploy.sh list-streams
#
# Example:
#   mkdir -p config/base/streams/my-new-stream
#   cp config/base/streams/air-quality/config.json config/base/streams/my-new-stream/
#   # Edit the config.json file as needed
#   ./deploy.sh sync
#
# ============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }
log() { echo -e "${GREEN}[INFO]${NC} $1"; }

# Show deprecation warning
echo ""
warn "============================================================================"
warn "DEPRECATED: This script is deprecated since dp-018 (JSON Config Foundation)"
warn "============================================================================"
echo ""
warn "Stream configurations are now managed via JSON config files."
echo ""
warn "To add a new stream:"
echo ""
log "  1. Create directory:   mkdir -p config/base/streams/{stream_id}"
log "  2. Create config.json: cp config/base/streams/air-quality/config.json \\"
log "                            config/base/streams/{stream_id}/config.json"
log "  3. Edit the config:    Edit the JSON file with your stream settings"
log "  4. Sync to etcd:       ./deploy.sh sync"
log "  5. Verify:             ./deploy.sh list-streams"
echo ""
warn "The new approach stores complete JSON blobs at /streams/{stream_id}/config"
warn "This provides:"
warn "  - Version-controlled configuration"
warn "  - Consistent schema across all streams"
warn "  - Easy rollback via git"
echo ""
warn "============================================================================"
echo ""

# Show old usage for reference
if [ $# -lt 5 ]; then
    echo "Old usage (deprecated):"
    echo "  $0 <stream_id> <stream_name> <device_id> <mqtt_topic> <location> [description]"
    echo ""
    echo "New usage:"
    echo "  1. Create: config/base/streams/{stream_id}/config.json"
    echo "  2. Sync:   ./deploy.sh sync"
    exit 1
fi

# If someone still tries to use this script with arguments, show guidance
STREAM_ID="$1"
STREAM_NAME="$2"
DEVICE_ID="$3"
MQTT_TOPIC="$4"
LOCATION="$5"
DESCRIPTION="${6:-Air quality monitoring stream}"

error "This script no longer creates streams directly in etcd."
echo ""
log "To create stream '$STREAM_ID', create this file:"
echo ""
echo "  config/base/streams/$STREAM_ID/config.json"
echo ""
log "With content like:"
echo ""
cat << EOF
{
  "stream_id": "$STREAM_ID",
  "description": "$DESCRIPTION",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,
  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "ndp_id": "${DEVICE_ID}",
      "context": {
        "device_type": "airgradient",
        "location": {
          "path": "$LOCATION"
        }
      },
      "broker_url": "mosquitto",
      "port": 1883,
      "topic_pattern": "$MQTT_TOPIC",
      "qos": 1
    }
  ]
}
EOF
echo ""
log "Then run: ./deploy.sh sync"
exit 1
