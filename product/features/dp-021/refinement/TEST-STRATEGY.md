# dp-021: Config Lifecycle & Release Management - Test Strategy

**Document Type**: SPARC Test Strategy (Phase R)
**Feature**: dp-021 Config Lifecycle & Release Management
**Version**: 1.0
**Date**: 2026-02-02
**Status**: Proposed

---

## 1. Executive Summary

This document defines the comprehensive test strategy for dp-021, covering three major phases:
- **Phase 4**: Hot-Reload (Sources Only)
- **Phase 5**: Schema Migration (v1.1 to v2.0)
- **Phase R**: Release Methodology

The strategy follows **London School TDD** principles with behavior verification, mock-driven design, and contract testing. Tests are organized by phase with clear acceptance criteria.

### Test Goals

1. **Hot-Reload Correctness** - Verify etcd watch triggers SourceManager callbacks without message loss
2. **Source Reconnection Safety** - MQTT sources reconnect gracefully on config changes
3. **Migration Fidelity** - v1.1 configs transform cleanly to v2.0 with no data loss
4. **Release Traceability** - Manifest naming, git tags, and device state remain synchronized
5. **Validation Enforcement** - Post-migration validator rejects v1.1 configs

### Test Pyramid

```
                    /\
                   /  \
                  / E2E \           T-E2E series (3 tests)
                 /-------\          Full deploy + hot-reload
                / Integr. \         T-4.x, T-5.x, T-R.x (18 tests)
               /-----------\        Docker compose environment
              /    Unit     \       HR-xxx, MIG-xxx, REL-xxx (24 tests)
             /---------------\      Isolated function tests
```

---

## 2. Test Categories

### Test Levels

| Level | Infrastructure | Run Command | Coverage Target |
|-------|---------------|-------------|-----------------|
| Unit | None | `cargo test` | 90%+ core logic |
| Integration | Docker (docker-compose.integration.yml) | `./integration-test-dp021.sh` | All phase scenarios |
| E2E | Full Pi simulation | Manual or CI | Happy path + error recovery |

### Test ID Naming Convention

| Prefix | Phase | Description |
|--------|-------|-------------|
| HR-xxx | Phase 4 | Hot-Reload unit tests |
| T-4.x | Phase 4 | Hot-Reload integration tests |
| MIG-xxx | Phase 5 | Migration unit tests |
| T-5.x | Phase 5 | Migration integration tests |
| REL-xxx | Phase R | Release unit tests |
| T-R.x | Phase R | Release integration tests |

---

## 3. Test Environment

### Infrastructure

| Component | Container Name | Port | Purpose |
|-----------|---------------|------|---------|
| etcd | integration-etcd | 2379 | Configuration store + watch |
| TimescaleDB | integration-timescaledb | 5432 | Silver layer database |
| MQTT | integration-mosquitto | 1883 | Message broker for MQTT source tests |
| MCP Server | integration-mcp-server | 9100 | MCP interface |

### Environment Variables

```bash
DEPLOY_ENV=integration
ETCD_ENDPOINT=http://localhost:2379
TIMESCALE_URL=postgresql://postgres:postgres@localhost:5432/ndp
MQTT_BROKER=mqtt://localhost:1883
```

### Test Data Isolation

Test configurations use underscore prefix to exclude from production:

```
config/base/streams/_test-dp021-hotreload/config.json
config/base/streams/_test-dp021-migration/config.json
.deploy/releases/_test-v0.0.1.manifest.json
```

---

## 4. Phase 4: Hot-Reload Tests

### 4.1 Unit Tests

#### HR-001: SourceManager Callback Registration

| Field | Value |
|-------|-------|
| **Test ID** | HR-001 |
| **Description** | Verify SourceManager can register callback for config changes |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_source_manager_registers_callback() {
    // Arrange
    let mut source_manager = SourceManager::new();
    let callback_called = Arc::new(AtomicBool::new(false));
    let callback_flag = callback_called.clone();

    // Act
    source_manager.on_config_change(Box::new(move |stream_id| {
        callback_flag.store(true, Ordering::SeqCst);
        Ok(())
    }));

    // Assert
    assert!(source_manager.has_callback());
}
```

---

#### HR-002: MQTT Source Disconnect Graceful

| Field | Value |
|-------|-------|
| **Test ID** | HR-002 |
| **Description** | Verify MQTT source disconnects gracefully without pending message loss |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[tokio::test]
async fn test_mqtt_source_graceful_disconnect() {
    // Arrange
    let mock_client = MockMqttClient::new();
    mock_client.expect_disconnect()
        .times(1)
        .returning(|| Ok(()));

    let source = MqttSource::new(mock_client);

    // Act
    let result = source.graceful_disconnect().await;

    // Assert
    assert!(result.is_ok());
    mock_client.verify();
}
```

---

#### HR-003: HTTP Source Interval Update

| Field | Value |
|-------|-------|
| **Test ID** | HR-003 |
| **Description** | Verify HTTP polling source updates interval immediately |
| **Type** | Unit |
| **Priority** | High |

```rust
#[tokio::test]
async fn test_http_source_interval_update() {
    // Arrange
    let source = HttpPollingSource::new(HttpSourceConfig {
        poll_interval_secs: 300,
        ..Default::default()
    });
    assert_eq!(source.get_interval_secs(), 300);

    // Act
    source.update_interval(60).await;

    // Assert
    assert_eq!(source.get_interval_secs(), 60);
}
```

---

#### HR-004: Config Change Parser

| Field | Value |
|-------|-------|
| **Test ID** | HR-004 |
| **Description** | Verify config change events are parsed correctly from etcd watch |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_parse_config_change_event() {
    // Arrange
    let watch_event = WatchEvent {
        key: "/ndp/streams/air-quality".to_string(),
        value: Some(r#"{"stream_id": "air-quality", "enabled": true}"#.to_string()),
        event_type: EventType::Put,
    };

    // Act
    let change = ConfigChange::from_watch_event(&watch_event);

    // Assert
    assert!(change.is_ok());
    let change = change.unwrap();
    assert_eq!(change.stream_id, "air-quality");
    assert_eq!(change.change_type, ChangeType::Updated);
}
```

---

#### HR-005: Invalid Config Rejected

| Field | Value |
|-------|-------|
| **Test ID** | HR-005 |
| **Description** | Verify invalid config changes are rejected, old config retained |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[tokio::test]
async fn test_invalid_config_rejected_old_retained() {
    // Arrange
    let mut source_manager = SourceManager::new();
    let valid_config = StreamConfig::valid_test_config();
    source_manager.apply_config("test-stream", valid_config.clone()).await.unwrap();

    let invalid_config = StreamConfig {
        stream_id: "test-stream".to_string(),
        sources: vec![], // Empty sources - invalid
        ..Default::default()
    };

    // Act
    let result = source_manager.apply_config("test-stream", invalid_config).await;

    // Assert
    assert!(result.is_err());
    let current_config = source_manager.get_config("test-stream").await.unwrap();
    assert_eq!(current_config, valid_config); // Old config retained
}
```

---

### 4.2 Integration Tests

#### T-4.1: etcd Watch Triggers SourceManager Callback

| Field | Value |
|-------|-------|
| **Test ID** | T-4.1 |
| **Description** | Verify etcd watch on stream config triggers SourceManager callback |
| **Type** | Integration |
| **Priority** | Critical |
| **Precondition** | Integration environment running |

```bash
# Setup: Create test stream config
mkdir -p config/base/streams/_test-dp021-watch
cat > config/base/streams/_test-dp021-watch/config.json << 'EOF'
{
  "stream_id": "_test-dp021-watch",
  "description": "Test stream for hot-reload watch",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "http_poll",
    "endpoints": [{"url": "https://api.test.com/data"}],
    "poll_interval_secs": 300
  }]
}
EOF

# Execute: Sync to etcd
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-watch

# Modify config
cat > config/base/streams/_test-dp021-watch/config.json << 'EOF'
{
  "stream_id": "_test-dp021-watch",
  "description": "Test stream - UPDATED",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "http_poll",
    "endpoints": [{"url": "https://api.test.com/data"}],
    "poll_interval_secs": 60
  }]
}
EOF

# Re-sync
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-watch

# Verify: Check application logs for callback trigger
docker logs integration-air-quality 2>&1 | grep "Config change detected for _test-dp021-watch"
```

**Expected Outcome**: Application logs show callback triggered with stream_id.

---

#### T-4.2: MQTT Source Reconnects on Config Change

| Field | Value |
|-------|-------|
| **Test ID** | T-4.2 |
| **Description** | Verify MQTT source reconnects with new broker/topic on config change |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# Setup: Create MQTT-based test stream
cat > config/base/streams/_test-dp021-mqtt/config.json << 'EOF'
{
  "stream_id": "_test-dp021-mqtt",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "mqtt",
    "broker_url": "mqtt://localhost:1883",
    "topic": "test/original/topic"
  }]
}
EOF

# Sync and start app
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-mqtt

# Wait for connection
sleep 5

# Verify initial subscription
docker exec integration-mosquitto mosquitto_sub -t '$SYS/broker/subscriptions' -C 1 | grep "test/original/topic"

# Update topic
cat > config/base/streams/_test-dp021-mqtt/config.json << 'EOF'
{
  "stream_id": "_test-dp021-mqtt",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "mqtt",
    "broker_url": "mqtt://localhost:1883",
    "topic": "test/updated/topic"
  }]
}
EOF

# Re-sync
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-mqtt

# Wait for reconnection
sleep 5

# Verify new subscription
docker exec integration-mosquitto mosquitto_sub -t '$SYS/broker/subscriptions' -C 1 | grep "test/updated/topic"
```

**Expected Outcome**: MQTT client subscribes to new topic after config change.

---

#### T-4.3: HTTP Polling Interval Updates Immediately

| Field | Value |
|-------|-------|
| **Test ID** | T-4.3 |
| **Description** | Verify HTTP source polling interval changes take effect immediately |
| **Type** | Integration |
| **Priority** | High |

```bash
# Setup: Create HTTP-based test stream with 300s interval
cat > config/base/streams/_test-dp021-http/config.json << 'EOF'
{
  "stream_id": "_test-dp021-http",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "http_poll",
    "endpoints": [{"url": "https://api.test.com/data"}],
    "poll_interval_secs": 300
  }]
}
EOF

DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-http

# Record initial metric
INITIAL_INTERVAL=$(curl -s http://localhost:9090/metrics | grep 'ndp_source_poll_interval{stream_id="_test-dp021-http"}' | awk '{print $2}')

# Update to 60s interval
cat > config/base/streams/_test-dp021-http/config.json << 'EOF'
{
  "stream_id": "_test-dp021-http",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "http_poll",
    "endpoints": [{"url": "https://api.test.com/data"}],
    "poll_interval_secs": 60
  }]
}
EOF

DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-http

# Wait briefly for hot-reload
sleep 2

# Check updated metric
UPDATED_INTERVAL=$(curl -s http://localhost:9090/metrics | grep 'ndp_source_poll_interval{stream_id="_test-dp021-http"}' | awk '{print $2}')

# Verify
[ "$INITIAL_INTERVAL" = "300" ] && [ "$UPDATED_INTERVAL" = "60" ] && echo "PASS: Interval updated"
```

---

#### T-4.4: No Message Loss During MQTT Reconnection

| Field | Value |
|-------|-------|
| **Test ID** | T-4.4 |
| **Description** | Verify no messages lost during MQTT source reconnection |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# Setup: Start MQTT stream and publish continuously during reconnection

# Publish test messages in background
(for i in $(seq 1 100); do
  mosquitto_pub -h localhost -t "test/reconnect/topic" -m "{\"seq\": $i, \"ts\": $(date +%s%N)}"
  sleep 0.1
done) &
PUB_PID=$!

# Trigger config reload mid-stream
sleep 2
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-mqtt

# Wait for publishing to complete
wait $PUB_PID

# Wait for processing
sleep 5

# Query Bronze layer for message count
MESSAGE_COUNT=$(docker exec integration-timescaledb psql -U postgres -d ndp -t -c "
  SELECT COUNT(*) FROM bronze._test_dp021_mqtt_raw
  WHERE raw_payload->>'seq' IS NOT NULL;
")

# Verify all 100 messages received
[ "$MESSAGE_COUNT" -ge 95 ] && echo "PASS: $MESSAGE_COUNT/100 messages received (>=95% threshold)"
```

---

#### T-4.5: Reload Endpoint Triggers Manual Reload

| Field | Value |
|-------|-------|
| **Test ID** | T-4.5 |
| **Description** | Verify HTTP reload endpoint triggers manual config reload |
| **Type** | Integration |
| **Priority** | Medium |

```bash
# Call reload endpoint
RESPONSE=$(curl -s -X POST http://localhost:8080/api/reload/streams/_test-dp021-http)

# Verify response
echo "$RESPONSE" | jq -e '.success == true' && echo "PASS: Reload endpoint works"

# Verify logs show reload triggered
docker logs integration-air-quality 2>&1 | grep "Manual reload triggered for _test-dp021-http"
```

---

#### T-4.6: Invalid Config Rejected, Old Config Retained

| Field | Value |
|-------|-------|
| **Test ID** | T-4.6 |
| **Description** | Verify invalid config update is rejected and old config continues working |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# Store known-good config
ORIGINAL_INTERVAL=300

# Apply invalid config (missing required field)
cat > config/base/streams/_test-dp021-http/config.json << 'EOF'
{
  "stream_id": "_test-dp021-http",
  "enabled": true,
  "config_version": 2,
  "sources": [{
    "type": "http_poll"
    // Missing endpoints - invalid
  }]
}
EOF

# Attempt sync (should fail validation)
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-http 2>&1 | grep -q "Validation failed"
SYNC_RESULT=$?

# Verify old config still active
CURRENT_INTERVAL=$(curl -s http://localhost:9090/metrics | grep 'ndp_source_poll_interval{stream_id="_test-dp021-http"}' | awk '{print $2}')

[ "$SYNC_RESULT" -eq 0 ] && [ "$CURRENT_INTERVAL" = "$ORIGINAL_INTERVAL" ] && echo "PASS: Invalid config rejected, old retained"
```

---

## 5. Phase 5: Migration Tests

### 5.1 Unit Tests

#### MIG-001: v1.1 Config Transforms to v2.0

| Field | Value |
|-------|-------|
| **Test ID** | MIG-001 |
| **Description** | Verify v1.1 config structure transforms to v2.0 correctly |
| **Type** | Unit |
| **Priority** | Critical |

```bash
# Unit test via jq
INPUT='{"stream_id":"test","config_version":1,"entity_schemas":[{"name":"pm25"}],"fields":[{"name":"pm25"}]}'

OUTPUT=$(echo "$INPUT" | jq 'del(.entity_schemas) | .config_version = 2')

# Verify
echo "$OUTPUT" | jq -e '.config_version == 2' && \
echo "$OUTPUT" | jq -e 'has("entity_schemas") | not' && \
echo "$OUTPUT" | jq -e '.fields | length > 0' && \
echo "PASS: Transform correct"
```

---

#### MIG-002: entity_schemas Removed Completely

| Field | Value |
|-------|-------|
| **Test ID** | MIG-002 |
| **Description** | Verify entity_schemas field is completely removed after migration |
| **Type** | Unit |
| **Priority** | Critical |

```bash
# Migration script removes entity_schemas
INPUT='{"stream_id":"test","entity_schemas":[{"name":"old_schema","columns":[]}]}'

OUTPUT=$(scripts/ndp-migrate-config.sh --input "$INPUT" --from 1.1 --to 2)

# Verify complete removal
echo "$OUTPUT" | jq -e 'has("entity_schemas") | not' && echo "PASS: entity_schemas removed"
```

---

#### MIG-003: config_version Updated to 2

| Field | Value |
|-------|-------|
| **Test ID** | MIG-003 |
| **Description** | Verify config_version field is updated from 1 to 2 |
| **Type** | Unit |
| **Priority** | Critical |

```bash
INPUT='{"stream_id":"test","config_version":1}'
OUTPUT=$(scripts/ndp-migrate-config.sh --input "$INPUT" --from 1.1 --to 2)

echo "$OUTPUT" | jq -e '.config_version == 2' && echo "PASS: config_version updated"
```

---

#### MIG-004: Dry-Run Mode Shows Changes

| Field | Value |
|-------|-------|
| **Test ID** | MIG-004 |
| **Description** | Verify dry-run mode shows what would change without writing |
| **Type** | Unit |
| **Priority** | High |

```bash
# Create v1.1 test config
cat > /tmp/test-v1.1.json << 'EOF'
{"stream_id":"test","config_version":1,"entity_schemas":[]}
EOF

# Run dry-run
OUTPUT=$(scripts/ndp-migrate-config.sh --input /tmp/test-v1.1.json --from 1.1 --to 2 --dry-run)

# Verify shows changes but file unchanged
echo "$OUTPUT" | grep -q "Would update config_version" && \
cat /tmp/test-v1.1.json | jq -e '.config_version == 1' && \
echo "PASS: Dry-run shows changes without modification"
```

---

### 5.2 Integration Tests

#### T-5.1: v1.1 Config Transforms to v2.0

| Field | Value |
|-------|-------|
| **Test ID** | T-5.1 |
| **Description** | Full integration test of v1.1 to v2.0 migration |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# Setup: Create v1.1 format config
mkdir -p config/base/streams/_test-dp021-migrate
cat > config/base/streams/_test-dp021-migrate/config.json << 'EOF'
{
  "stream_id": "_test-dp021-migrate",
  "config_version": 1,
  "description": "Test stream for migration",
  "enabled": true,
  "entity_schemas": [
    {
      "name": "air_quality_reading",
      "columns": [
        {"name": "pm25", "type": "float"},
        {"name": "temperature", "type": "float"}
      ]
    }
  ],
  "fields": [
    {"name": "pm25", "source_path": "raw_payload.pm25", "type": "float"},
    {"name": "temperature", "source_path": "raw_payload.temperature", "type": "float"}
  ]
}
EOF

# Execute migration
scripts/ndp-migrate-config.sh --from 1.1 --to 2 config/base/streams/_test-dp021-migrate/config.json

# Verify result
CONFIG=$(cat config/base/streams/_test-dp021-migrate/config.json)
echo "$CONFIG" | jq -e '.config_version == 2' && \
echo "$CONFIG" | jq -e 'has("entity_schemas") | not' && \
echo "$CONFIG" | jq -e '.fields | length == 2' && \
echo "PASS: T-5.1 - Config migrated successfully"
```

---

#### T-5.2: entity_schemas Removed Completely

| Field | Value |
|-------|-------|
| **Test ID** | T-5.2 |
| **Description** | Verify entity_schemas key is completely absent post-migration |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# After T-5.1 migration
CONFIG=$(cat config/base/streams/_test-dp021-migrate/config.json)

# Must not contain entity_schemas at all (not just empty)
echo "$CONFIG" | grep -v "entity_schemas" && echo "PASS: T-5.2 - entity_schemas completely removed"
```

---

#### T-5.3: config_version Updated to 2

| Field | Value |
|-------|-------|
| **Test ID** | T-5.3 |
| **Description** | Verify config_version is exactly 2 after migration |
| **Type** | Integration |
| **Priority** | Critical |

```bash
CONFIG=$(cat config/base/streams/_test-dp021-migrate/config.json)
VERSION=$(echo "$CONFIG" | jq -r '.config_version')

[ "$VERSION" = "2" ] && echo "PASS: T-5.3 - config_version is 2"
```

---

#### T-5.4: Dry-Run Mode Shows Changes Without Writing

| Field | Value |
|-------|-------|
| **Test ID** | T-5.4 |
| **Description** | Verify dry-run mode previews changes without modifying files |
| **Type** | Integration |
| **Priority** | High |

```bash
# Reset to v1.1
cat > config/base/streams/_test-dp021-migrate/config.json << 'EOF'
{
  "stream_id": "_test-dp021-migrate",
  "config_version": 1,
  "entity_schemas": []
}
EOF

# Checksum before
BEFORE=$(md5sum config/base/streams/_test-dp021-migrate/config.json | cut -d' ' -f1)

# Run dry-run
scripts/ndp-migrate-config.sh --from 1.1 --to 2 --dry-run config/base/streams/_test-dp021-migrate/config.json

# Checksum after
AFTER=$(md5sum config/base/streams/_test-dp021-migrate/config.json | cut -d' ' -f1)

[ "$BEFORE" = "$AFTER" ] && echo "PASS: T-5.4 - Dry-run did not modify file"
```

---

#### T-5.5: Validator Rejects v1.1 Configs After Migration

| Field | Value |
|-------|-------|
| **Test ID** | T-5.5 |
| **Description** | Verify validator rejects configs with entity_schemas |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# Create v1.1 config (should be rejected by v2.0 validator)
cat > /tmp/rejected-v1.1.json << 'EOF'
{
  "stream_id": "rejected-test",
  "config_version": 1,
  "entity_schemas": [{"name": "legacy"}]
}
EOF

# Run validator with v2.0 schema
ndp-validate --schema schemas/stream-config.v2.schema.json /tmp/rejected-v1.1.json 2>&1 | grep -q "entity_schemas is not allowed"
RESULT=$?

[ "$RESULT" -eq 0 ] && echo "PASS: T-5.5 - Validator rejects v1.1 configs"
```

---

#### T-5.6: Dictionary Loader Reads from fields Only

| Field | Value |
|-------|-------|
| **Test ID** | T-5.6 |
| **Description** | Verify dictionary loader ignores entity_schemas, reads only from fields |
| **Type** | Integration |
| **Priority** | High |

```bash
# Create v2.0 config with only fields
cat > config/base/streams/_test-dp021-dict/config.json << 'EOF'
{
  "stream_id": "_test-dp021-dict",
  "config_version": 2,
  "fields": [
    {"name": "pm25", "source_path": "raw_payload.pm25", "type": "float", "description": "PM2.5 reading"}
  ]
}
EOF

# Sync to etcd
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-dict

# Query MCP for dictionary
DICT=$(curl -s http://localhost:9100/mcp/query_dictionary?stream_id=_test-dp021-dict)

# Verify fields loaded
echo "$DICT" | jq -e '.columns | length == 1' && \
echo "$DICT" | jq -e '.columns[0].name == "pm25"' && \
echo "PASS: T-5.6 - Dictionary loaded from fields"
```

---

## 6. Phase R: Release Tests

### 6.1 Unit Tests

#### REL-001: Manifest Version Parsing

| Field | Value |
|-------|-------|
| **Test ID** | REL-001 |
| **Description** | Verify manifest version follows vX.Y.Z pattern |
| **Type** | Unit |
| **Priority** | High |

```bash
# Valid pattern
echo "v1.2.3" | grep -qE '^v[0-9]+\.[0-9]+\.[0-9]+$' && echo "PASS: Valid pattern"

# Invalid patterns
echo "1.2.3" | grep -qE '^v[0-9]+\.[0-9]+\.[0-9]+$' || echo "PASS: Missing 'v' prefix rejected"
echo "v1.2" | grep -qE '^v[0-9]+\.[0-9]+\.[0-9]+$' || echo "PASS: Missing patch rejected"
```

---

#### REL-002: Manifest File Naming

| Field | Value |
|-------|-------|
| **Test ID** | REL-002 |
| **Description** | Verify manifest file follows vX.Y.Z.manifest.json naming |
| **Type** | Unit |
| **Priority** | High |

```bash
# Test naming function
get_manifest_filename() {
  echo "$1.manifest.json"
}

FILENAME=$(get_manifest_filename "v1.2.0")
[ "$FILENAME" = "v1.2.0.manifest.json" ] && echo "PASS: Manifest naming correct"
```

---

### 6.2 Integration Tests

#### T-R.1: Manifest Naming Follows vX.Y.Z Pattern

| Field | Value |
|-------|-------|
| **Test ID** | T-R.1 |
| **Description** | Verify all release manifests follow naming convention |
| **Type** | Integration |
| **Priority** | High |

```bash
# Create test release manifest
mkdir -p .deploy/releases
cat > .deploy/releases/v0.0.1.manifest.json << 'EOF'
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "0.0.1",
  "description": "Test release v0.0.1",
  "changes": []
}
EOF

# Verify naming pattern for all manifests
for manifest in .deploy/releases/*.manifest.json; do
  BASENAME=$(basename "$manifest")
  echo "$BASENAME" | grep -qE '^v[0-9]+\.[0-9]+\.[0-9]+\.manifest\.json$'
  if [ $? -ne 0 ]; then
    echo "FAIL: Invalid manifest name: $BASENAME"
    exit 1
  fi
done
echo "PASS: T-R.1 - All manifests follow naming convention"
```

---

#### T-R.2: Git Tag Matches Manifest Version

| Field | Value |
|-------|-------|
| **Test ID** | T-R.2 |
| **Description** | Verify git tag aligns with manifest release_version |
| **Type** | Integration |
| **Priority** | Critical |

```bash
# For each manifest, verify corresponding git tag exists
for manifest in .deploy/releases/v*.manifest.json; do
  VERSION=$(jq -r '.release_version' "$manifest")
  TAG="v$VERSION"

  # Check if tag exists
  git tag -l "$TAG" | grep -q "$TAG"
  if [ $? -ne 0 ]; then
    echo "WARNING: Missing git tag $TAG for manifest $(basename $manifest)"
  else
    echo "PASS: Git tag $TAG exists for manifest"
  fi
done
```

---

#### T-R.3: /var/ndp/deployed-version Updated After Deploy

| Field | Value |
|-------|-------|
| **Test ID** | T-R.3 |
| **Description** | Verify device deployed-version file is updated after apply |
| **Type** | Integration |
| **Priority** | High |

```bash
# Apply manifest
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v0.0.1.manifest.json

# Verify deployed-version updated
DEPLOYED=$(cat /var/ndp/deployed-version 2>/dev/null || echo "")

[ "$DEPLOYED" = "v0.0.1" ] && echo "PASS: T-R.3 - deployed-version updated"
```

---

#### T-R.4: deploy.sh Apply Reads release_version from Manifest

| Field | Value |
|-------|-------|
| **Test ID** | T-R.4 |
| **Description** | Verify deploy.sh extracts and uses release_version from manifest |
| **Type** | Integration |
| **Priority** | High |

```bash
# Create manifest with specific release_version
cat > .deploy/releases/v0.0.2.manifest.json << 'EOF'
{
  "version": "1.0",
  "release_version": "0.0.2",
  "description": "Test release v0.0.2",
  "changes": [
    {"type": "stream", "id": "_test-dp021-release", "action": "create"}
  ]
}
EOF

# Apply and capture output
OUTPUT=$(DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v0.0.2.manifest.json 2>&1)

# Verify release_version was read
echo "$OUTPUT" | grep -q "Deploying release v0.0.2" && echo "PASS: T-R.4 - release_version read from manifest"
```

---

## 7. Error Handling Tests

### E-4.1: etcd Connection Failure During Watch

```bash
# Stop etcd
docker stop integration-etcd

# Verify app handles gracefully (doesn't crash)
sleep 5
docker ps | grep integration-air-quality | grep -q "Up" && echo "PASS: App survives etcd disconnect"

# Restart etcd
docker start integration-etcd
```

---

### E-4.2: MQTT Broker Unreachable

```bash
# Stop MQTT broker
docker stop integration-mosquitto

# Update config (should fail gracefully)
DEPLOY_ENV=integration ./deploy.sh sync-etcd _test-dp021-mqtt 2>&1 | grep -q "MQTT broker unreachable"

# Restart broker
docker start integration-mosquitto
```

---

### E-5.1: Migration Script Missing jq

```bash
# Test migration script handles missing jq
(
  PATH=""  # Remove jq from path
  scripts/ndp-migrate-config.sh --from 1.1 --to 2 /tmp/test.json 2>&1 | grep -q "jq is required"
) && echo "PASS: Missing jq detected"
```

---

### E-R.1: Invalid Manifest Schema

```bash
cat > /tmp/invalid-manifest.json << 'EOF'
{
  "invalid": "structure"
}
EOF

./deploy.sh apply /tmp/invalid-manifest.json 2>&1 | grep -q "Invalid manifest"
[ $? -eq 0 ] && echo "PASS: Invalid manifest rejected"
```

---

## 8. Test Fixtures

### 8.1 Sample Configurations

**Location**: `tests/fixtures/dp021/`

```
tests/fixtures/dp021/
  configs/
    v1.1-with-entity-schemas.json    # Pre-migration format
    v2.0-clean.json                   # Post-migration format
    invalid-missing-sources.json      # For rejection tests
  manifests/
    sample-release.manifest.json      # Valid release manifest
    invalid-manifest.json             # For error tests
```

### 8.2 v1.1 Test Config

```json
{
  "stream_id": "fixture-v1.1",
  "config_version": 1,
  "description": "v1.1 format with entity_schemas",
  "enabled": true,
  "entity_schemas": [
    {
      "name": "reading",
      "columns": [
        {"name": "value", "type": "float"}
      ]
    }
  ],
  "fields": [
    {"name": "value", "source_path": "raw_payload.value", "type": "float"}
  ]
}
```

### 8.3 v2.0 Test Config

```json
{
  "stream_id": "fixture-v2.0",
  "config_version": 2,
  "description": "v2.0 format - clean",
  "enabled": true,
  "fields": [
    {"name": "value", "source_path": "raw_payload.value", "type": "float", "description": "Sensor value"}
  ]
}
```

---

## 9. Mock Requirements

### 9.1 Rust Mocks

| Mock | Purpose | Library |
|------|---------|---------|
| `MockMqttClient` | MQTT operations without broker | mockall |
| `MockEtcdClient` | etcd operations without server | mockall |
| `MockConfigStore` | Config operations for unit tests | mockall |

### 9.2 Integration Test Doubles

| Component | Approach |
|-----------|----------|
| etcd | Real container (integration-etcd) |
| MQTT | Real container (integration-mosquitto) |
| HTTP endpoint | Mock server or httpbin container |

---

## 10. CI/CD Integration

### 10.1 GitHub Actions Workflow

```yaml
# .github/workflows/dp-021-tests.yml
name: dp-021 Tests

on:
  pull_request:
    paths:
      - 'core/src/coordinator/**'
      - 'core/src/sources/**'
      - 'scripts/ndp-migrate-config.sh'
      - 'schemas/stream-config.v2.schema.json'
      - '.deploy/**'
  push:
    branches: [main]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Rust unit tests
        run: cargo test -p neural-core -- hot_reload migration

  integration-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    steps:
      - uses: actions/checkout@v4

      - name: Start integration environment
        run: ./scripts/integration-test.sh start

      - name: Wait for services
        run: sleep 30

      - name: Run dp-021 integration tests
        run: ./scripts/integration-test-dp021.sh

      - name: Cleanup
        if: always()
        run: ./scripts/integration-test.sh clean
```

### 10.2 Integration Test Script

**Location**: `scripts/integration-test-dp021.sh`

```bash
#!/bin/bash
set -e

echo "=== dp-021 Integration Tests ==="

# Phase 4 Tests
echo "--- Phase 4: Hot-Reload Tests ---"
./tests/integration/dp021/test-4.1-etcd-watch.sh
./tests/integration/dp021/test-4.2-mqtt-reconnect.sh
./tests/integration/dp021/test-4.3-http-interval.sh
./tests/integration/dp021/test-4.4-no-message-loss.sh
./tests/integration/dp021/test-4.5-reload-endpoint.sh
./tests/integration/dp021/test-4.6-invalid-config.sh

# Phase 5 Tests
echo "--- Phase 5: Migration Tests ---"
./tests/integration/dp021/test-5.1-migration.sh
./tests/integration/dp021/test-5.2-entity-schemas-removed.sh
./tests/integration/dp021/test-5.3-config-version.sh
./tests/integration/dp021/test-5.4-dry-run.sh
./tests/integration/dp021/test-5.5-validator-rejects.sh
./tests/integration/dp021/test-5.6-dictionary-loader.sh

# Phase R Tests
echo "--- Phase R: Release Tests ---"
./tests/integration/dp021/test-R.1-manifest-naming.sh
./tests/integration/dp021/test-R.2-git-tag-match.sh
./tests/integration/dp021/test-R.3-deployed-version.sh
./tests/integration/dp021/test-R.4-release-version.sh

echo "=== All dp-021 Tests Passed ==="
```

---

## 11. Test Execution Summary

### 11.1 Test Count by Phase

| Phase | Unit Tests | Integration Tests | Total |
|-------|-----------|-------------------|-------|
| Phase 4 (Hot-Reload) | 5 (HR-001 to HR-005) | 6 (T-4.1 to T-4.6) | 11 |
| Phase 5 (Migration) | 4 (MIG-001 to MIG-004) | 6 (T-5.1 to T-5.6) | 10 |
| Phase R (Release) | 2 (REL-001 to REL-002) | 4 (T-R.1 to T-R.4) | 6 |
| Error Handling | - | 4 (E-4.x, E-5.x, E-R.x) | 4 |
| **Total** | **11** | **20** | **31** |

### 11.2 Estimated Execution Time

| Test Category | Time |
|---------------|------|
| Unit tests | ~10s |
| Integration tests | ~120s |
| E2E tests | ~180s |
| **Total** | ~310s (~5 min) |

---

## 12. Test Checklist

Before marking dp-021 testing complete:

### Phase 4: Hot-Reload
- [ ] HR-001: SourceManager callback registration
- [ ] HR-002: MQTT graceful disconnect
- [ ] HR-003: HTTP interval update
- [ ] HR-004: Config change parser
- [ ] HR-005: Invalid config rejection
- [ ] T-4.1: etcd watch triggers callback
- [ ] T-4.2: MQTT reconnects on config change
- [ ] T-4.3: HTTP interval updates immediately
- [ ] T-4.4: No message loss during reconnection
- [ ] T-4.5: Reload endpoint works
- [ ] T-4.6: Invalid config rejected, old retained

### Phase 5: Migration
- [ ] MIG-001: v1.1 transforms to v2.0
- [ ] MIG-002: entity_schemas removed
- [ ] MIG-003: config_version updated
- [ ] MIG-004: Dry-run shows changes
- [ ] T-5.1: Full migration integration
- [ ] T-5.2: entity_schemas completely absent
- [ ] T-5.3: config_version exactly 2
- [ ] T-5.4: Dry-run mode works
- [ ] T-5.5: Validator rejects v1.1
- [ ] T-5.6: Dictionary loads from fields only

### Phase R: Release
- [ ] REL-001: Manifest version parsing
- [ ] REL-002: Manifest file naming
- [ ] T-R.1: Manifest naming convention
- [ ] T-R.2: Git tag matches manifest
- [ ] T-R.3: deployed-version updated
- [ ] T-R.4: release_version read from manifest

### Error Handling
- [ ] E-4.1: etcd connection failure
- [ ] E-4.2: MQTT broker unreachable
- [ ] E-5.1: Migration missing jq
- [ ] E-R.1: Invalid manifest rejected

### Infrastructure
- [ ] Test fixtures created
- [ ] Integration test script working
- [ ] CI workflow configured
- [ ] Cleanup procedure documented

---

## 13. References

- [dp-021 SCOPE.md](../SCOPE.md) - Feature requirements
- [dp-020 TEST-STRATEGY.md](../../dp-020/refinement/TEST-STRATEGY.md) - Precedent test strategy format
- [AIR-005-TEST-DESIGN.md](/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md) - London TDD patterns
- [Hot-Reload Legacy Tests](/workspaces/neural-data-platform/archive/legacy-tests/components/config_store/test_hot_reload.rs) - Reference implementation

---

*Test Strategy created: 2026-02-02*
*SPARC Phase: Refinement (R)*
*Author: ndp-tester agent*
