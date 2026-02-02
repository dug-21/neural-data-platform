# dp-018: Sync JSON Stream Configs to etcd - Design Document

**Document Type**: Architecture Design
**Feature**: dp-018 JSON Config Foundation
**Date**: 2026-02-02
**Status**: Proposed
**Architecture**: Aligned with ADR-018-001 JSON Pass-Through Architecture

---

## 1. Executive Summary

This design document specifies a new `sync-streams-to-etcd.sh` script that syncs JSON stream configurations to etcd following the dp-018 JSON pass-through architecture. The script replaces the legacy YAML-based sync approach with a simpler JSON blob storage pattern.

### Key Changes

1. **New script**: `scripts/sync-streams-to-etcd.sh` - syncs JSON configs as complete blobs
2. **Update**: `deploy/pi/deploy.sh` - use new sync script
3. **Deprecate**: `deploy/pi/configs/streams/init-streams.sh` - legacy key-value approach
4. **Document**: Migration path from old `/air-quality/streams/` keys

---

## 2. Context

### Current State

The platform has multiple configuration sync mechanisms with inconsistent approaches:

| Script | Location | Format | etcd Path | Status |
|--------|----------|--------|-----------|--------|
| `sync-config-to-etcd.sh` | `scripts/` | YAML | `/streams/{id}/...` (flattened) | Converts YAML to key-value pairs |
| `init-streams.sh` | `deploy/pi/configs/streams/` | Hardcoded | `/air-quality/streams/{id}/{key}` | Legacy, individual keys |
| App sync_all() | `air-quality-app` | JSON | `/streams/{id}/config` | Correct approach |

### dp-018 Architecture (ADR-018-001)

The JSON pass-through architecture requires:

```
JSON file (source of truth)
    |
    v [validate against schema]
    |
    v [sync to etcd AS-IS]
    |
etcd: /streams/{stream_id}/config = <entire JSON blob>
```

**Key principle**: JSON file content equals etcd blob. No transformation.

### StreamRegistry Expectations

From `config-client/src/stream/registry.rs`:
- Key pattern: `/streams/{stream_id}/config`
- Value: Complete JSON blob deserializable to `StreamConfig`
- Method: `registry.load_stream("air-quality")` loads from `/streams/air-quality/config`

---

## 3. Design Specification

### 3.1 New Script: `scripts/sync-streams-to-etcd.sh`

#### Purpose

Sync all JSON stream configurations from `config/base/streams/*/config.json` to etcd at `/streams/{stream_id}/config`.

#### Interface

```bash
# Usage
./scripts/sync-streams-to-etcd.sh [options]

# Options
  --mode docker|local    Execution mode (default: auto-detect)
  --container NAME       Docker container name (default: etcd)
  --endpoint URL         etcd endpoint for local mode (default: http://localhost:2379)
  --validate             Validate JSON against schema before sync
  --dry-run              Show what would be synced without writing
  --verbose              Show detailed output
  --help                 Show usage information

# Environment Variables
  ETCD_CONTAINER         Override container name for docker mode
  ETCD_ENDPOINT          Override endpoint for local mode

# Examples
  ./scripts/sync-streams-to-etcd.sh                           # Auto-detect mode
  ./scripts/sync-streams-to-etcd.sh --mode docker             # Force docker mode
  ./scripts/sync-streams-to-etcd.sh --mode local              # Force local mode
  ./scripts/sync-streams-to-etcd.sh --validate --dry-run      # Validate only
  DEPLOY_ENV=integration ./scripts/sync-streams-to-etcd.sh    # Integration testing
```

#### Algorithm

```
FUNCTION sync_streams_to_etcd():
    1. Parse command-line arguments
    2. Detect execution mode (docker vs local)
    3. Test etcd connectivity

    4. FOR each directory in config/base/streams/*:
        a. Check if config.json exists
        b. Extract stream_id from JSON (or use directory name as fallback)
        c. IF --validate flag:
             Validate JSON against schema (if schema exists)
        d. Read entire JSON file content
        e. Store to etcd at /streams/{stream_id}/config
        f. Log success/failure

    5. Report summary (streams synced, failures)

    RETURN exit_code (0 if all succeeded, 1 if any failed)
```

#### etcd Key Structure

```
/streams/
  air-quality/
    config                  <- Entire JSON blob from config.json
  outdoor-weather/
    config                  <- Entire JSON blob from config.json
  nws-observations/
    config                  <- Entire JSON blob from config.json
  ...
```

**Not used** (legacy patterns to avoid):
- `/air-quality/streams/{id}/{key}` - Old init-streams.sh pattern
- `/streams/{id}/{nested/key}` - Old sync-config-to-etcd.sh flattening

#### Execution Modes

**Docker Mode** (default for Pi deployment):
```bash
# Uses docker exec to run etcdctl inside container
docker exec "$ETCD_CONTAINER" etcdctl put "/streams/${stream_id}/config" "$json_content"
```

**Local Mode** (for development/testing):
```bash
# Uses local etcdctl binary
etcdctl --endpoints="$ETCD_ENDPOINT" put "/streams/${stream_id}/config" "$json_content"
```

**Auto-detection**:
1. If `docker` command exists and etcd container is running -> docker mode
2. If `etcdctl` command exists locally -> local mode
3. Otherwise -> error with instructions

#### JSON Validation (Optional)

If `--validate` flag is provided and schema exists:

```bash
# Schema location
SCHEMA_FILE="${REPO_ROOT}/schemas/stream-config.v1.schema.json"

# Validate using jq or ajv (if available)
if command -v ajv &> /dev/null; then
    ajv validate -s "$SCHEMA_FILE" -d "$config_file"
elif command -v jq &> /dev/null; then
    # Basic JSON syntax validation only
    jq empty "$config_file"
fi
```

#### Error Handling

| Condition | Action |
|-----------|--------|
| etcd not reachable | Exit with error, suggest checking container/endpoint |
| config.json not found | Skip directory, warn |
| Invalid JSON syntax | Skip file, log ERROR |
| Schema validation fails | Skip file if --validate, log ERROR |
| etcdctl put fails | Log ERROR, continue with other streams |
| All streams fail | Exit with code 1 |

#### Output Format

```
[SYNC] Environment: docker (container: etcd)
[SYNC] Config directory: /workspaces/neural-data-platform/config/base/streams
[SYNC]
[SYNC] Syncing air-quality...
[OK]   air-quality -> /streams/air-quality/config (2.3KB)
[SYNC] Syncing outdoor-weather...
[OK]   outdoor-weather -> /streams/outdoor-weather/config (1.8KB)
[SYNC] Syncing nws-observations...
[OK]   nws-observations -> /streams/nws-observations/config (3.1KB)
[SYNC]
[SYNC] Summary: 7/7 streams synced successfully
```

### 3.2 Changes to `deploy/pi/deploy.sh`

#### Current sync_config() Function

```bash
sync_config() {
    log "Syncing configuration to etcd..."
    # Wait for etcd...
    if [ -f "$REPO_ROOT/scripts/sync-config-to-etcd.sh" ]; then
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-config-to-etcd.sh" $ENV_NAME
    fi
}
```

#### Updated sync_config() Function

```bash
sync_config() {
    log "Syncing configuration to etcd..."

    # Wait for etcd to be ready
    until dcx etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd to be ready..."
        sleep 2
    done

    # Sync JSON stream configs (dp-018 architecture)
    if [ -f "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" ]; then
        log "Syncing stream configurations..."
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-streams-to-etcd.sh" --mode docker
    else
        warn "Stream sync script not found at $REPO_ROOT/scripts/sync-streams-to-etcd.sh"
    fi

    # Legacy: sync-config-to-etcd.sh for non-stream configs (if still needed)
    # TODO: Migrate remaining configs to JSON and consolidate
    if [ -f "$REPO_ROOT/scripts/sync-config-to-etcd.sh" ]; then
        log "Syncing legacy configurations..."
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-config-to-etcd.sh" $ENV_NAME
    fi
}
```

#### Deprecate init_streams()

The `init-streams` command becomes a no-op with deprecation warning:

```bash
init_streams() {
    warn "DEPRECATED: init-streams is deprecated since dp-018"
    warn "Stream configs are now synced via 'sync' command using JSON files"
    warn "Run './deploy.sh sync' instead"

    # For backward compatibility during transition, still sync
    sync_config
}
```

#### Update Help Text

Add to help section:
```bash
# Old (remove or mark deprecated):
#   init-streams    - Initialize stream configurations in etcd

# New:
#   sync            - Sync all configurations to etcd (includes JSON stream configs)
```

### 3.3 Legacy Key Cleanup

#### Old Keys to Document

The old `init-streams.sh` created keys at:
```
/air-quality/streams/{stream_id}/id
/air-quality/streams/{stream_id}/name
/air-quality/streams/{stream_id}/device_id
/air-quality/streams/{stream_id}/mqtt_topic
/air-quality/streams/{stream_id}/location
/air-quality/streams/{stream_id}/description
/air-quality/streams/{stream_id}/enabled
/air-quality/streams/{stream_id}/created_at
/air-quality/streams/{stream_id}/storage/path
/air-quality/streams/{stream_id}/storage/retention_days
/air-quality/streams/{stream_id}/storage/compression
/air-quality/multi_stream/enabled
/air-quality/multi_stream/max_concurrent_streams
/air-quality/multi_stream/webhook_enabled
/air-quality/multi_stream/webhook_port
```

#### Cleanup Script (Optional)

Create `scripts/cleanup-legacy-etcd-keys.sh`:

```bash
#!/bin/bash
# Cleanup legacy etcd keys from init-streams.sh
# Run AFTER verifying new sync works correctly

set -e
ETCD_CONTAINER="${1:-etcd}"

log() { echo "[CLEANUP] $1"; }
warn() { echo "[WARN] $1"; }

# Show what would be deleted
log "Legacy keys to delete:"
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/air-quality/streams/" --keys-only
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/air-quality/multi_stream/" --keys-only

echo ""
read -p "Delete these keys? (y/N) " confirm
if [ "$confirm" = "y" ]; then
    docker exec "$ETCD_CONTAINER" etcdctl del --prefix "/air-quality/streams/"
    docker exec "$ETCD_CONTAINER" etcdctl del --prefix "/air-quality/multi_stream/"
    log "Legacy keys deleted"
else
    log "Cleanup cancelled"
fi
```

### 3.4 Update list-streams.sh

The current `list-streams.sh` reads from old key structure. Update to read from new structure:

**Updated `deploy/pi/configs/streams/list-streams.sh`**:

```bash
#!/bin/bash
# List all configured streams from etcd
# Updated for dp-018 JSON config architecture

set -e

ETCD_CONTAINER="${1:-etcd}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[STREAMS]${NC} $1"; }

log "Configured Streams (from /streams/*/config):"
echo ""

# Get all stream configs from new key pattern
docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/streams/" --keys-only 2>/dev/null | \
    grep "/config$" | while read key; do

    # Extract stream_id from key like /streams/air-quality/config
    stream_id=$(echo "$key" | sed 's|/streams/||' | sed 's|/config$||')

    # Get the JSON config
    config=$(docker exec "$ETCD_CONTAINER" etcdctl get "$key" --print-value-only 2>/dev/null)

    if [ -n "$config" ]; then
        # Parse JSON fields using jq (if available) or grep
        if command -v jq &> /dev/null; then
            description=$(echo "$config" | jq -r '.description // "N/A"')
            enabled=$(echo "$config" | jq -r '.enabled // false')
            version=$(echo "$config" | jq -r '.version // "N/A"')
            source_count=$(echo "$config" | jq -r '.sources | length')
        else
            # Fallback to grep for basic parsing
            description=$(echo "$config" | grep -o '"description"[^,}]*' | head -1 | cut -d'"' -f4)
            enabled=$(echo "$config" | grep -o '"enabled"[^,}]*' | head -1 | grep -o 'true\|false')
            version="N/A"
            source_count="?"
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
        echo -e "  Status:      $status"
        echo ""
    fi
done

# Count total
total=$(docker exec "$ETCD_CONTAINER" etcdctl get --prefix "/streams/" --keys-only 2>/dev/null | grep -c "/config$" || echo "0")
log "Total: $total streams configured"
```

---

## 4. Migration Path

### Phase 1: Add New Script (Non-Breaking)

1. Create `scripts/sync-streams-to-etcd.sh`
2. Test with `DEPLOY_ENV=integration`
3. Verify streams appear in etcd at `/streams/{id}/config`

### Phase 2: Update deploy.sh (Additive)

1. Update `sync_config()` to call new script first
2. Keep legacy `sync-config-to-etcd.sh` call for non-stream configs
3. Deploy and verify both new and old keys exist

### Phase 3: Update Consumers

1. Verify `StreamRegistry.load_stream()` works (it should - same key pattern)
2. Update `list-streams.sh` to read new keys
3. Test end-to-end

### Phase 4: Deprecate Legacy

1. Mark `init-streams.sh` deprecated
2. Update deploy.sh help text
3. Create cleanup script for old keys
4. Document in CHANGELOG

### Phase 5: Cleanup (After Verification)

1. Run cleanup script to remove `/air-quality/streams/*` keys
2. Remove deprecated `init-streams.sh`
3. Remove stream handling from `sync-config-to-etcd.sh`

---

## 5. Testing Strategy

### Unit Tests (Shell)

Test script behavior with mock etcd:

```bash
# Test: Config file not found
mkdir -p /tmp/test-streams/empty-stream
./scripts/sync-streams-to-etcd.sh --dry-run
# Expected: Skip empty-stream with warning

# Test: Invalid JSON
echo "{invalid" > /tmp/test-streams/bad-json/config.json
./scripts/sync-streams-to-etcd.sh --dry-run
# Expected: ERROR for bad-json, continue others

# Test: Valid JSON
echo '{"stream_id":"test","enabled":true}' > /tmp/test-streams/test/config.json
./scripts/sync-streams-to-etcd.sh --dry-run
# Expected: Would sync test -> /streams/test/config
```

### Integration Tests

With `DEPLOY_ENV=integration`:

```bash
# Start integration environment
DEPLOY_ENV=integration ./deploy/pi/deploy.sh start

# Sync streams
DEPLOY_ENV=integration ./scripts/sync-streams-to-etcd.sh

# Verify in etcd
docker exec integration-etcd etcdctl get --prefix "/streams/" --keys-only

# Verify StreamRegistry can load
# (via air-quality-app startup logs)
```

### Acceptance Criteria

| Test | Expected Result |
|------|-----------------|
| Script runs without errors | Exit code 0 |
| All config.json files synced | Count matches directory count |
| JSON stored as complete blob | `etcdctl get` returns exact JSON |
| StreamRegistry loads config | No errors in app logs |
| list-streams.sh shows streams | Output includes all stream IDs |
| --dry-run doesn't write | No changes in etcd |
| --validate catches bad JSON | ERROR logged, stream skipped |

---

## 6. Dependencies

| Dependency | Required | Notes |
|------------|----------|-------|
| etcdctl | Yes | In container (docker mode) or local (local mode) |
| jq | Optional | For JSON validation and parsing |
| docker | Yes (docker mode) | For container exec |
| bash | Yes | Script language |

---

## 7. Security Considerations

1. **No secrets in config.json**: Stream configs should not contain secrets. Secrets should be in environment variables or separate secret management.

2. **etcd access**: Script runs with same etcd permissions as deploy.sh. No elevation required.

3. **Validation**: Optional schema validation catches malformed configs before they reach etcd.

---

## 8. Rollback Plan

If issues arise:

1. **Immediate**: Old keys still exist at `/air-quality/streams/*` until Phase 5
2. **Quick fix**: Revert deploy.sh changes, old sync continues working
3. **Full rollback**: Git revert commit, redeploy

---

## 9. Implementation Checklist

- [ ] Create `scripts/sync-streams-to-etcd.sh`
- [ ] Add `--mode`, `--validate`, `--dry-run` options
- [ ] Test docker mode with Pi container
- [ ] Test local mode for development
- [ ] Update `deploy/pi/deploy.sh` sync_config()
- [ ] Deprecate init_streams() with warning
- [ ] Update `deploy/pi/configs/streams/list-streams.sh`
- [ ] Create `scripts/cleanup-legacy-etcd-keys.sh`
- [ ] Add integration tests
- [ ] Update CHANGELOG
- [ ] Update deploy.sh help text

---

## 10. References

| Document | Path |
|----------|------|
| ADR-018-001 | `product/features/dp-018/architecture/ADR-018-001-config-loader-design.md` |
| dp-018 Specification | `product/features/dp-018/specification/SPECIFICATION.md` |
| StreamRegistry | `config-client/src/stream/registry.rs` |
| Current sync script | `scripts/sync-config-to-etcd.sh` |
| Current init-streams | `deploy/pi/configs/streams/init-streams.sh` |
| deploy.sh | `deploy/pi/deploy.sh` |

---

*Design created: 2026-02-02*
*Aligned with: dp-018 JSON Pass-Through Architecture*
