#!/bin/bash
# Stream Configuration Initialization Script
#
# ============================================================================
# DEPRECATED: dp-018 JSON Config Foundation
# ============================================================================
#
# This script is DEPRECATED. Stream configurations are now managed via:
#   - JSON config files: config/base/streams/*/config.json
#   - Sync script:       scripts/sync-streams-to-etcd.sh
#   - Deploy command:    ./deploy.sh sync
#
# The new approach (dp-018 architecture):
#   - Stores complete JSON blobs at /streams/{stream_id}/config
#   - No transformation - JSON file content equals etcd value
#   - StreamRegistry loads from /streams/{stream_id}/config
#
# Migration:
#   1. Create/update config.json files in config/base/streams/{stream_id}/
#   2. Run: ./deploy.sh sync
#   3. Verify: ./deploy.sh list-streams
#   4. Clean up old keys: scripts/cleanup-legacy-etcd-keys.sh
#
# ============================================================================

set -e

ETCD_CONTAINER="${1:-etcd}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[STREAM-INIT]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Show deprecation warning
echo ""
warn "============================================================================"
warn "DEPRECATED: This script is deprecated since dp-018 (JSON Config Foundation)"
warn "============================================================================"
echo ""
warn "Stream configurations are now managed via JSON config files."
warn ""
warn "New approach:"
warn "  1. Edit JSON files in: config/base/streams/*/config.json"
warn "  2. Sync to etcd:       ./deploy.sh sync"
warn "  3. List streams:       ./deploy.sh list-streams"
warn ""
warn "Migration guide:"
warn "  - Old key pattern: /air-quality/streams/{id}/{key}"
warn "  - New key pattern: /streams/{stream_id}/config (JSON blob)"
warn ""
warn "To remove old keys after migration:"
warn "  scripts/cleanup-legacy-etcd-keys.sh"
warn ""
warn "============================================================================"
echo ""

# For backward compatibility, redirect to the new sync mechanism
log "Redirecting to sync mechanism..."

# Check if the new sync script exists
if [ -f "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" ]; then
    log "Running: scripts/sync-streams-to-etcd.sh --mode docker"
    ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" --mode docker
else
    error "New sync script not found at: $REPO_ROOT/scripts/sync-streams-to-etcd.sh"
    error "Please update your repository."
    exit 1
fi

echo ""
log "Stream sync complete via new mechanism."
log "Run './deploy.sh list-streams' to verify."
