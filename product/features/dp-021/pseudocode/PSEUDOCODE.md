# dp-021: Config Lifecycle & Release Management - SPARC Pseudocode

**Document Type**: SPARC Pseudocode (Phase P)
**Feature**: dp-021 Config Lifecycle & Release Management
**Version**: 1.0
**Date**: 2026-02-02
**Author**: Pseudocode Agent
**Prerequisites**: SCOPE.md, dp-018 (JSON Config Foundation), dp-019 (Validation Pipeline), dp-020 (Declarative Deploy)

---

## 1. Executive Summary

This document provides detailed algorithmic specifications for the dp-021 Config Lifecycle & Release Management feature. The design covers three phases:

1. **Phase 4: Hot-Reload** - Source reconfiguration without application restart
2. **Phase 5: Schema Migration** - v1.1 to v2.0 config migration (entity_schemas removal)
3. **Phase R: Release Methodology** - Formalized versioning and deployment workflow

The algorithms prioritize:
- **Zero-downtime** source updates
- **No message loss** during MQTT reconnection
- **Safe migration** with dry-run capabilities
- **Consistent versioning** aligned with deployment manifests

---

## 2. Data Structures

### 2.1 Phase 4: Hot-Reload Types

```
STRUCT SourceHandle:
    source_id: String                   # Unique source identifier
    source_type: SourceType             # mqtt, http_poll, webhook, etc.
    stop_tx: Option<Sender<()>>         # Stop signal channel
    config_version: u64                 # Config version at spawn time
    last_health_check: Timestamp        # Last health check time
    metrics: SourceMetrics              # Performance metrics

STRUCT SourceMetrics:
    messages_received: u64
    messages_dropped: u64
    last_message_time: Option<Timestamp>
    reconnect_count: u64
    avg_latency_ms: f64

STRUCT ConfigChangeEvent:
    stream_id: String
    old_config: Option<StreamConfig>    # None if new stream
    new_config: Option<StreamConfig>    # None if deleted
    change_type: ConfigChangeType
    timestamp: Timestamp

ENUM ConfigChangeType:
    CREATED                             # New stream config
    UPDATED                             # Existing stream modified
    DELETED                             # Stream config removed
    DISABLED                            # Stream disabled (enabled: false)
    ENABLED                             # Stream re-enabled

STRUCT HotReloadResult:
    success: Boolean
    stream_id: String
    sources_stopped: Array<String>      # Source IDs that were stopped
    sources_started: Array<String>      # Source IDs that were started
    messages_preserved: u64             # Messages handled during transition
    duration_ms: u64                    # Reload duration
    error: Option<String>               # Error message if failed

STRUCT MqttReconnectState:
    client_id: String
    pending_messages: Queue<MqttMessage>
    last_message_id: u64
    disconnect_time: Timestamp
    reconnect_attempt: u32
```

### 2.2 Phase 5: Migration Types

```
STRUCT MigrationPlan:
    source_version: String              # "1.1"
    target_version: String              # "2.0"
    config_path: Path
    changes: Array<MigrationChange>
    dry_run: Boolean

STRUCT MigrationChange:
    path: String                        # JSONPath to changed field
    change_type: MigrationChangeType
    old_value: Option<Value>
    new_value: Option<Value>
    reason: String

ENUM MigrationChangeType:
    FIELD_REMOVED                       # entity_schemas deleted
    FIELD_ADDED                         # New required field
    VALUE_CHANGED                       # config_version updated
    STRUCTURE_CHANGED                   # fields array enriched

STRUCT MigrationResult:
    success: Boolean
    config_path: Path
    source_version: String
    target_version: String
    changes_applied: Array<MigrationChange>
    backup_path: Option<Path>           # If backup was created
    error: Option<String>

STRUCT ConfigVersion:
    major: u8                           # Breaking changes
    minor: u8                           # Backwards-compatible features
    config_version: u8                  # Internal schema version (1, 2)
```

### 2.3 Phase R: Release Types

```
STRUCT ReleaseManifest:
    schema: String                      # "$schema" reference
    version: String                     # Manifest schema version ("1.0")
    release_version: String             # "1.2.0" - semver
    description: String                 # Release description
    created_at: Timestamp
    changes: Array<DeployChange>
    rollback_info: Option<RollbackInfo>

STRUCT DeployChange:
    change_type: String                 # "config", "ddl", "etcd"
    stream_id: Option<String>
    action: String                      # "add", "modify", "delete"
    file_path: Option<String>
    description: String

STRUCT RollbackInfo:
    previous_version: String
    rollback_manifest: String           # Path to previous manifest
    auto_rollback: Boolean              # Auto-rollback on failure

STRUCT ReleaseResult:
    success: Boolean
    release_version: String
    git_tag: String
    manifest_path: Path
    changelog_entry: String
    errors: Array<String>

STRUCT DeployedVersion:
    version: String                     # e.g., "1.2.0"
    deployed_at: Timestamp
    manifest_path: String
    deployer: String                    # Who/what triggered deploy
    environment: String                 # "pi", "dev", "ci"
```

### 2.4 Complexity Analysis: Data Structures

| Structure | Space Complexity | Notes |
|-----------|------------------|-------|
| SourceHandle | O(1) | Fixed-size per source |
| ConfigChangeEvent | O(c) | c = config size |
| MqttReconnectState | O(m) | m = pending messages |
| MigrationPlan | O(n) | n = number of changes |
| ReleaseManifest | O(d) | d = number of deploy changes |

---

## 3. Phase 4: Hot-Reload Algorithms

### 3.1 etcd Watch Handler

```
ALGORITHM: on_etcd_config_change
INPUT:
    stream_id: String                   # Stream that changed
    new_config: Option<StreamConfig>    # New config (None if deleted)
    source_manager: &SourceManager      # Reference to source manager
OUTPUT:
    HotReloadResult

BEGIN
    start_time <- GET_CURRENT_TIME()

    // ========================================
    // PHASE 1: Determine Change Type
    // ========================================
    current_sources <- source_manager.get_sources_for_stream(stream_id)

    change_type <- DETERMINE_CHANGE_TYPE(
        has_current_sources: LENGTH(current_sources) > 0,
        has_new_config: new_config IS NOT NULL,
        config_enabled: new_config?.enabled OR false
    )

    LOG_INFO("Config change detected",
        stream_id: stream_id,
        change_type: change_type,
        current_source_count: LENGTH(current_sources)
    )

    // ========================================
    // PHASE 2: Validate New Config (if applicable)
    // ========================================
    IF new_config IS NOT NULL AND change_type IN {CREATED, UPDATED, ENABLED} THEN
        validation_result <- AWAIT validate_config(new_config)

        IF NOT validation_result.valid THEN
            LOG_ERROR("Hot-reload aborted: invalid config",
                stream_id: stream_id,
                errors: validation_result.errors
            )
            RETURN HotReloadResult {
                success: false,
                stream_id: stream_id,
                error: Some("Config validation failed: " + validation_result.errors[0].message)
            }
        END IF
    END IF

    // ========================================
    // PHASE 3: Execute Change
    // ========================================
    result <- MATCH change_type WITH
        CREATED ->
            handle_stream_created(stream_id, new_config.unwrap(), source_manager)

        UPDATED ->
            handle_stream_updated(stream_id, new_config.unwrap(), source_manager)

        DELETED ->
            handle_stream_deleted(stream_id, source_manager)

        DISABLED ->
            handle_stream_disabled(stream_id, source_manager)

        ENABLED ->
            handle_stream_enabled(stream_id, new_config.unwrap(), source_manager)
    END MATCH

    // ========================================
    // PHASE 4: Log Reload Event
    // ========================================
    duration_ms <- (GET_CURRENT_TIME() - start_time).as_millis()

    LOG_INFO("Hot-reload complete",
        stream_id: stream_id,
        change_type: change_type,
        success: result.success,
        duration_ms: duration_ms,
        sources_started: result.sources_started,
        sources_stopped: result.sources_stopped
    )

    // Record metrics
    metrics.record_reload_event(stream_id, change_type, result.success, duration_ms)

    RETURN HotReloadResult {
        ...result,
        duration_ms: duration_ms
    }
END

// ----------------------------------------
// Helper: Determine Change Type
// ----------------------------------------
FUNCTION DETERMINE_CHANGE_TYPE(has_current, has_new, config_enabled) -> ConfigChangeType:
    IF NOT has_current AND has_new AND config_enabled THEN
        RETURN CREATED
    ELSE IF has_current AND has_new AND config_enabled THEN
        RETURN UPDATED
    ELSE IF has_current AND NOT has_new THEN
        RETURN DELETED
    ELSE IF has_current AND has_new AND NOT config_enabled THEN
        RETURN DISABLED
    ELSE IF NOT has_current AND has_new AND NOT config_enabled THEN
        RETURN DISABLED  // Already disabled, no-op
    ELSE IF NOT has_current AND has_new AND config_enabled THEN
        RETURN ENABLED   // Re-enabling previously disabled
    ELSE
        RETURN UPDATED   // Default fallback
    END IF
END
```

### 3.2 Handle Stream Updated (Core Hot-Reload)

```
ALGORITHM: handle_stream_updated
INPUT:
    stream_id: String
    new_config: StreamConfig
    source_manager: &SourceManager
OUTPUT:
    HotReloadResult

BEGIN
    sources_stopped <- []
    sources_started <- []
    messages_preserved <- 0

    // ========================================
    // PHASE 1: Identify Sources to Update
    // ========================================
    current_sources <- source_manager.get_sources_for_stream(stream_id)
    new_source_configs <- new_config.sources

    // Build maps for comparison
    current_map <- BUILD_MAP(current_sources, s -> s.source_id)
    new_map <- BUILD_MAP(new_source_configs, s -> generate_source_id(stream_id, s))

    // Categorize changes
    to_remove <- KEYS(current_map) - KEYS(new_map)
    to_add <- KEYS(new_map) - KEYS(current_map)
    to_update <- KEYS(current_map) INTERSECT KEYS(new_map)

    // ========================================
    // PHASE 2: Stop Removed/Updated Sources
    // ========================================
    FOR EACH source_id IN (to_remove UNION to_update) DO
        // Get current source handle
        handle <- current_map[source_id]

        // For MQTT sources, gracefully drain messages
        IF handle.source_type == MQTT THEN
            preserved <- AWAIT graceful_mqtt_disconnect(handle)
            messages_preserved <- messages_preserved + preserved
        ELSE
            AWAIT source_manager.stop_source(source_id)
        END IF

        sources_stopped <- sources_stopped + [source_id]
        LOG_DEBUG("Stopped source for update", source_id: source_id)
    END FOR

    // ========================================
    // PHASE 3: Start New/Updated Sources
    // ========================================
    FOR EACH source_id IN (to_add UNION to_update) DO
        source_config <- new_map[source_id]

        TRY
            AWAIT source_manager.spawn_source(source_id, source_config)
            sources_started <- sources_started + [source_id]
            LOG_DEBUG("Started source", source_id: source_id)
        CATCH error
            LOG_ERROR("Failed to start source",
                source_id: source_id,
                error: error.message
            )
            // Continue with other sources
        END TRY
    END FOR

    RETURN HotReloadResult {
        success: LENGTH(sources_started) > 0 OR LENGTH(to_add) == 0,
        stream_id: stream_id,
        sources_stopped: sources_stopped,
        sources_started: sources_started,
        messages_preserved: messages_preserved,
        error: None
    }
END

// ----------------------------------------
// Helper: Generate Source ID
// ----------------------------------------
FUNCTION generate_source_id(stream_id, source_config) -> String:
    IF source_config.ndp_id IS NOT NULL THEN
        RETURN source_config.ndp_id
    ELSE
        // Generate deterministic ID from config
        hash <- HASH(stream_id + source_config.source_type + source_config.params)
        RETURN FORMAT("{}-{}-{}", stream_id, source_config.source_type, hash[0..8])
    END IF
END
```

### 3.3 Graceful MQTT Reconnect

```
ALGORITHM: graceful_mqtt_reconnect
INPUT:
    old_client: MqttClient
    new_config: SourceConfig
    message_handler: Fn(MqttMessage)
OUTPUT:
    Result<MqttClient, Error>

CONSTANTS:
    DRAIN_TIMEOUT_MS = 5000             # Max time to drain pending messages
    RECONNECT_DELAY_MS = 100            # Delay between disconnect and reconnect
    MAX_PENDING_MESSAGES = 1000         # Max messages to buffer

BEGIN
    pending_messages <- QUEUE()
    drain_start <- GET_CURRENT_TIME()

    // ========================================
    // PHASE 1: Set Up Message Capture
    // ========================================
    // Temporarily redirect incoming messages to buffer
    old_client.set_message_handler(|msg| {
        IF pending_messages.length() < MAX_PENDING_MESSAGES THEN
            pending_messages.push(msg)
        ELSE
            LOG_WARN("Pending message buffer full, dropping message")
        END IF
    })

    // ========================================
    // PHASE 2: Drain In-Flight Messages
    // ========================================
    // Wait for QoS 1/2 acknowledgments
    TRY
        AWAIT old_client.flush_with_timeout(DRAIN_TIMEOUT_MS)
        LOG_DEBUG("MQTT client flushed successfully")
    CATCH timeout_error
        LOG_WARN("MQTT flush timeout, proceeding with disconnect",
            pending_acks: old_client.pending_ack_count()
        )
    END TRY

    // ========================================
    // PHASE 3: Disconnect Old Client
    // ========================================
    old_subscriptions <- old_client.get_subscriptions()
    old_client_id <- old_client.client_id()

    TRY
        AWAIT old_client.disconnect_gracefully()
        LOG_INFO("MQTT client disconnected",
            client_id: old_client_id,
            pending_messages: pending_messages.length()
        )
    CATCH disconnect_error
        LOG_WARN("Error during disconnect, forcing close",
            error: disconnect_error.message
        )
        old_client.force_close()
    END TRY

    // Small delay to ensure broker recognizes disconnect
    SLEEP(RECONNECT_DELAY_MS)

    // ========================================
    // PHASE 4: Create New Client
    // ========================================
    new_mqtt_config <- extract_mqtt_config(new_config)

    // Use same client ID to maintain session (if clean_session=false)
    new_mqtt_config.client_id <- old_client_id

    new_client <- AWAIT MqttClient::connect(new_mqtt_config)

    IF new_client IS error THEN
        LOG_ERROR("Failed to connect new MQTT client",
            error: new_client.error
        )
        RETURN Err(new_client.error)
    END IF

    // ========================================
    // PHASE 5: Resubscribe to Topics
    // ========================================
    new_topics <- extract_topics(new_config)

    // Determine subscription changes
    topics_to_unsubscribe <- old_subscriptions - new_topics
    topics_to_subscribe <- new_topics

    // Note: We subscribe to all new topics (broker handles duplicates)
    FOR EACH topic IN topics_to_subscribe DO
        TRY
            AWAIT new_client.subscribe(topic, new_mqtt_config.qos)
        CATCH sub_error
            LOG_ERROR("Failed to subscribe to topic",
                topic: topic,
                error: sub_error.message
            )
        END TRY
    END FOR

    // ========================================
    // PHASE 6: Replay Buffered Messages
    // ========================================
    new_client.set_message_handler(message_handler)

    replayed <- 0
    WHILE NOT pending_messages.is_empty() DO
        msg <- pending_messages.pop()
        message_handler(msg)
        replayed <- replayed + 1
    END WHILE

    LOG_INFO("MQTT reconnect complete",
        client_id: new_client.client_id(),
        messages_replayed: replayed,
        topics_subscribed: LENGTH(new_topics)
    )

    RETURN Ok(new_client)
END

// ----------------------------------------
// Helper: Graceful MQTT Disconnect (for stop)
// ----------------------------------------
ALGORITHM: graceful_mqtt_disconnect
INPUT:
    handle: SourceHandle
OUTPUT:
    messages_preserved: u64

BEGIN
    IF handle.source_type != MQTT THEN
        RETURN 0
    END IF

    preserved <- 0

    TRY
        // Drain pending messages
        preserved <- AWAIT handle.mqtt_client.flush_and_count()

        // Disconnect
        AWAIT handle.mqtt_client.disconnect_gracefully()
    CATCH error
        LOG_WARN("Error during MQTT disconnect", error: error.message)
        handle.mqtt_client.force_close()
    END TRY

    RETURN preserved
END
```

### 3.4 HTTP Polling Update

```
ALGORITHM: update_http_poll_source
INPUT:
    source_id: String
    old_config: HttpPollingConfig
    new_config: HttpPollingConfig
    source_manager: &SourceManager
OUTPUT:
    Result<(), Error>

BEGIN
    // ========================================
    // Check for Configuration Changes
    // ========================================
    changes <- detect_http_config_changes(old_config, new_config)

    IF changes.is_empty() THEN
        LOG_DEBUG("No HTTP config changes detected", source_id: source_id)
        RETURN Ok(())
    END IF

    LOG_INFO("HTTP poll config changed",
        source_id: source_id,
        changes: changes
    )

    // ========================================
    // Handle Poll Interval Change (Hot Update)
    // ========================================
    IF changes.contains("poll_interval") THEN
        // Poll interval can be updated without restart
        source_handle <- source_manager.get_source(source_id)
        source_handle.http_source.set_poll_interval(new_config.poll_interval)

        LOG_INFO("Updated poll interval",
            source_id: source_id,
            old_interval: old_config.poll_interval,
            new_interval: new_config.poll_interval
        )

        changes.remove("poll_interval")
    END IF

    // ========================================
    // Handle Endpoint/Sensor Changes (Requires Restart)
    // ========================================
    IF NOT changes.is_empty() THEN
        // These changes require source restart
        AWAIT source_manager.stop_source(source_id)

        new_source_config <- build_source_config_from_http(new_config)
        AWAIT source_manager.spawn_source(source_id, new_source_config)

        LOG_INFO("Restarted HTTP source for config changes",
            source_id: source_id,
            changes: changes
        )
    END IF

    RETURN Ok(())
END

// ----------------------------------------
// Helper: Detect HTTP Config Changes
// ----------------------------------------
FUNCTION detect_http_config_changes(old, new) -> Set<String>:
    changes <- SET()

    IF old.poll_interval != new.poll_interval THEN
        changes.add("poll_interval")
    END IF

    IF old.base_url_template != new.base_url_template THEN
        changes.add("base_url")
    END IF

    IF old.timeout != new.timeout THEN
        changes.add("timeout")
    END IF

    IF old.sensors != new.sensors THEN
        changes.add("sensors")
    END IF

    RETURN changes
END
```

### 3.5 Hot-Reload API Endpoint

```
ALGORITHM: handle_reload_request
INPUT:
    request: HttpRequest
    source_manager: &SourceManager
    config_registry: &ConfigRegistry
OUTPUT:
    HttpResponse

BEGIN
    // ========================================
    // Parse Request
    // ========================================
    stream_id <- request.query_param("stream_id")
    force <- request.query_param("force").unwrap_or(false)

    IF stream_id IS NULL THEN
        RETURN HttpResponse {
            status: 400,
            body: {"error": "stream_id query parameter required"}
        }
    END IF

    // ========================================
    // Load Current Config
    // ========================================
    config <- AWAIT config_registry.load_stream_config(stream_id)

    IF config IS error THEN
        RETURN HttpResponse {
            status: 404,
            body: {"error": FORMAT("Stream not found: {}", stream_id)}
        }
    END IF

    // ========================================
    // Trigger Reload
    // ========================================
    result <- AWAIT on_etcd_config_change(
        stream_id: stream_id,
        new_config: Some(config),
        source_manager: source_manager
    )

    IF result.success THEN
        RETURN HttpResponse {
            status: 200,
            body: {
                "success": true,
                "stream_id": stream_id,
                "sources_reloaded": result.sources_started,
                "duration_ms": result.duration_ms
            }
        }
    ELSE
        RETURN HttpResponse {
            status: 500,
            body: {
                "success": false,
                "stream_id": stream_id,
                "error": result.error
            }
        }
    END IF
END
```

### 3.6 Complexity Analysis: Hot-Reload

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Config change detection | O(s) | O(1) where s = sources |
| Source stop | O(m) | O(m) where m = pending messages |
| Source start | O(1) | O(1) |
| MQTT reconnect | O(m + t) | O(m) where t = topics |
| HTTP config update | O(1) | O(1) |
| **Total reload** | O(s * m) | O(m) |

---

## 4. Phase 5: Migration Algorithms

### 4.1 Main Migration Algorithm

```
ALGORITHM: migrate_config_v1_to_v2
INPUT:
    config_path: Path                   # Path to config.json
    options: MigrationOptions           # dry_run, backup, etc.
OUTPUT:
    MigrationResult

BEGIN
    // ========================================
    // PHASE 1: Read and Parse Config
    // ========================================
    LOG_INFO("Starting migration", config_path: config_path)

    content <- READ_FILE(config_path)
    IF content IS error THEN
        RETURN MigrationResult {
            success: false,
            config_path: config_path,
            error: Some(FORMAT("Cannot read file: {}", content.error))
        }
    END IF

    config <- PARSE_JSON(content)
    IF config IS error THEN
        RETURN MigrationResult {
            success: false,
            config_path: config_path,
            error: Some(FORMAT("Invalid JSON: {}", config.error))
        }
    END IF

    // ========================================
    // PHASE 2: Detect Current Version
    // ========================================
    current_version <- detect_config_version(config)

    LOG_INFO("Detected config version",
        config_path: config_path,
        version: current_version
    )

    IF current_version == "2.0" THEN
        LOG_INFO("Config already at v2.0, no migration needed")
        RETURN MigrationResult {
            success: true,
            config_path: config_path,
            source_version: "2.0",
            target_version: "2.0",
            changes_applied: []
        }
    END IF

    IF current_version != "1.1" THEN
        RETURN MigrationResult {
            success: false,
            config_path: config_path,
            error: Some(FORMAT(
                "Cannot migrate from version {}. Only v1.1 -> v2.0 supported.",
                current_version
            ))
        }
    END IF

    // ========================================
    // PHASE 3: Build Migration Plan
    // ========================================
    plan <- build_migration_plan(config, config_path)

    LOG_INFO("Migration plan created",
        changes: LENGTH(plan.changes),
        dry_run: options.dry_run
    )

    // ========================================
    // PHASE 4: Execute or Preview
    // ========================================
    IF options.dry_run THEN
        RETURN preview_migration(plan)
    END IF

    // Create backup before modification
    backup_path <- None
    IF options.backup THEN
        backup_path <- Some(config_path.with_extension("v1.1.backup.json"))
        COPY_FILE(config_path, backup_path.unwrap())
        LOG_INFO("Created backup", backup_path: backup_path)
    END IF

    // Apply migration
    result <- apply_migration(config, plan)

    IF result IS error THEN
        // Restore from backup if available
        IF backup_path IS NOT NULL THEN
            COPY_FILE(backup_path.unwrap(), config_path)
            LOG_WARN("Migration failed, restored from backup")
        END IF

        RETURN MigrationResult {
            success: false,
            config_path: config_path,
            error: Some(result.error)
        }
    END IF

    // Write migrated config
    migrated_json <- SERIALIZE_JSON(result.config, pretty: true)
    WRITE_FILE(config_path, migrated_json)

    LOG_INFO("Migration complete",
        config_path: config_path,
        changes_applied: LENGTH(plan.changes)
    )

    RETURN MigrationResult {
        success: true,
        config_path: config_path,
        source_version: "1.1",
        target_version: "2.0",
        changes_applied: plan.changes,
        backup_path: backup_path
    }
END
```

### 4.2 Detect Config Version

```
ALGORITHM: detect_config_version
INPUT:
    config: JsonValue
OUTPUT:
    version: String                     # "1.0", "1.1", "2.0"

BEGIN
    // Check for explicit config_version field
    IF config.has_field("config_version") THEN
        cv <- config["config_version"]
        IF cv == 2 OR cv == "2" OR cv == "2.0" THEN
            RETURN "2.0"
        ELSE IF cv == 1 OR cv == "1" OR cv == "1.1" THEN
            RETURN "1.1"
        END IF
    END IF

    // Infer version from structure
    has_entity_schemas <- config.has_field("entity_schemas")
    has_enriched_fields <- check_fields_enriched(config)

    IF NOT has_entity_schemas AND has_enriched_fields THEN
        // v2.0: No entity_schemas, fields are enriched
        RETURN "2.0"
    ELSE IF has_entity_schemas OR NOT has_enriched_fields THEN
        // v1.1: Has entity_schemas (even if deprecated) or bare fields
        RETURN "1.1"
    END IF

    // Default to v1.1 for safety
    RETURN "1.1"
END

// ----------------------------------------
// Helper: Check if Fields are Enriched
// ----------------------------------------
FUNCTION check_fields_enriched(config) -> Boolean:
    fields <- config.get("fields")
    IF fields IS NULL OR NOT fields.is_array() THEN
        RETURN false
    END IF

    // Consider enriched if any field has description or device_class
    FOR EACH field IN fields DO
        IF field.has_field("description") AND field["description"] != "" THEN
            RETURN true
        END IF
    END FOR

    RETURN false
END
```

### 4.3 Build Migration Plan

```
ALGORITHM: build_migration_plan
INPUT:
    config: JsonValue
    config_path: Path
OUTPUT:
    MigrationPlan

BEGIN
    changes <- []

    // ========================================
    // Change 1: Remove entity_schemas
    // ========================================
    IF config.has_field("entity_schemas") THEN
        entity_schemas <- config["entity_schemas"]

        changes <- changes + [MigrationChange {
            path: "$.entity_schemas",
            change_type: FIELD_REMOVED,
            old_value: Some(entity_schemas),
            new_value: None,
            reason: "entity_schemas deprecated in v1.1, removed in v2.0"
        }]

        // If entity_schemas had metadata not yet on fields, warn
        missing_metadata <- find_metadata_not_on_fields(config)
        IF LENGTH(missing_metadata) > 0 THEN
            LOG_WARN("Some metadata in entity_schemas not found on fields",
                fields: missing_metadata
            )
        END IF
    END IF

    // ========================================
    // Change 2: Set config_version = 2
    // ========================================
    old_version <- config.get("config_version")

    changes <- changes + [MigrationChange {
        path: "$.config_version",
        change_type: VALUE_CHANGED,
        old_value: old_version,
        new_value: Some(2),
        reason: "Update config_version to indicate v2.0 schema"
    }]

    // ========================================
    // Change 3: Ensure fields are enriched (if needed)
    // ========================================
    IF config.has_field("entity_schemas") THEN
        enrichment_changes <- generate_field_enrichment_changes(config)
        changes <- changes + enrichment_changes
    END IF

    RETURN MigrationPlan {
        source_version: "1.1",
        target_version: "2.0",
        config_path: config_path,
        changes: changes,
        dry_run: false
    }
END

// ----------------------------------------
// Helper: Generate Field Enrichment Changes
// ----------------------------------------
FUNCTION generate_field_enrichment_changes(config) -> Array<MigrationChange>:
    changes <- []
    fields <- config.get("fields").unwrap_or([])
    entity_schemas <- config.get("entity_schemas").unwrap_or([])

    // Build lookup from entity_schemas
    attr_map <- MAP()
    FOR EACH schema IN entity_schemas DO
        FOR EACH attr IN schema.get("attributes").unwrap_or([]) DO
            attr_map[attr["name"]] <- attr
        END FOR
    END FOR

    // Check each field for missing metadata
    FOR idx, field IN ENUMERATE(fields) DO
        field_name <- field["name"]

        IF field_name IN attr_map THEN
            attr <- attr_map[field_name]

            // Check for missing description
            IF NOT field.has_field("description") OR field["description"] == "" THEN
                IF attr.has_field("description") AND attr["description"] != "" THEN
                    changes <- changes + [MigrationChange {
                        path: FORMAT("$.fields[{}].description", idx),
                        change_type: FIELD_ADDED,
                        old_value: None,
                        new_value: Some(attr["description"]),
                        reason: "Migrated description from entity_schemas"
                    }]
                END IF
            END IF

            // Check for missing unit
            IF NOT field.has_field("unit") OR field["unit"] == "" THEN
                IF attr.has_field("unit") AND attr["unit"] != "" THEN
                    changes <- changes + [MigrationChange {
                        path: FORMAT("$.fields[{}].unit", idx),
                        change_type: FIELD_ADDED,
                        old_value: None,
                        new_value: Some(attr["unit"]),
                        reason: "Migrated unit from entity_schemas"
                    }]
                END IF
            END IF

            // Check for missing range
            IF NOT field.has_field("range") THEN
                IF attr.has_field("range") THEN
                    changes <- changes + [MigrationChange {
                        path: FORMAT("$.fields[{}].range", idx),
                        change_type: FIELD_ADDED,
                        old_value: None,
                        new_value: Some(attr["range"]),
                        reason: "Migrated range from entity_schemas"
                    }]
                END IF
            END IF
        END IF
    END FOR

    RETURN changes
END
```

### 4.4 Apply Migration

```
ALGORITHM: apply_migration
INPUT:
    config: JsonValue
    plan: MigrationPlan
OUTPUT:
    Result<{config: JsonValue}, Error>

BEGIN
    // Work on a copy
    migrated <- DEEP_CLONE(config)

    FOR EACH change IN plan.changes DO
        TRY
            MATCH change.change_type WITH
                FIELD_REMOVED ->
                    // Remove the field using JSONPath
                    migrated <- json_remove(migrated, change.path)

                FIELD_ADDED ->
                    // Add new field
                    migrated <- json_set(migrated, change.path, change.new_value.unwrap())

                VALUE_CHANGED ->
                    // Update value
                    migrated <- json_set(migrated, change.path, change.new_value.unwrap())

                STRUCTURE_CHANGED ->
                    // Apply structural transformation
                    migrated <- apply_structure_change(migrated, change)
            END MATCH

            LOG_DEBUG("Applied change",
                path: change.path,
                change_type: change.change_type
            )
        CATCH error
            RETURN Err(FORMAT("Failed to apply change at {}: {}", change.path, error))
        END TRY
    END FOR

    RETURN Ok({config: migrated})
END

// ----------------------------------------
// Helper: JSON Path Operations (jq-equivalent)
// ----------------------------------------
FUNCTION json_remove(obj, path) -> JsonValue:
    // In shell: jq 'del(.path)'
    // In Rust: Use jsonpath or manual traversal

    parts <- parse_jsonpath(path)
    parent <- traverse_to_parent(obj, parts)
    key <- parts.last()

    parent.remove(key)
    RETURN obj
END

FUNCTION json_set(obj, path, value) -> JsonValue:
    // In shell: jq '.path = value'

    parts <- parse_jsonpath(path)
    parent <- traverse_to_parent(obj, parts)
    key <- parts.last()

    parent[key] <- value
    RETURN obj
END
```

### 4.5 Migration CLI

```
ALGORITHM: migrate_cli_main
INPUT:
    args: CliArgs
OUTPUT:
    ExitCode

BEGIN
    // Parse arguments
    options <- MigrationOptions {
        from_version: args.from.unwrap_or("1.1"),
        to_version: args.to.unwrap_or("2"),
        dry_run: args.dry_run,
        backup: args.backup.unwrap_or(true),
        all: args.all,
        config_path: args.config_path
    }

    // Determine config files to migrate
    config_files <- []

    IF options.all THEN
        config_files <- discover_config_files(CONFIG_BASE_DIR)
    ELSE IF options.config_path IS NOT NULL THEN
        config_files <- [options.config_path]
    ELSE
        PRINT_ERROR("Specify config path or use --all")
        RETURN EXIT_USAGE
    END IF

    // Migrate each config
    results <- []
    has_errors <- false

    FOR EACH config_path IN config_files DO
        IF options.dry_run THEN
            PRINT(FORMAT("DRY-RUN: {}", config_path))
        ELSE
            PRINT(FORMAT("Migrating: {}", config_path))
        END IF

        result <- migrate_config_v1_to_v2(config_path, options)
        results <- results + [result]

        IF NOT result.success THEN
            has_errors <- true
            PRINT_ERROR(FORMAT("  FAILED: {}", result.error))
        ELSE
            PRINT_SUCCESS(FORMAT("  {} changes applied", LENGTH(result.changes_applied)))

            IF options.dry_run THEN
                FOR EACH change IN result.changes_applied DO
                    PRINT(FORMAT("    {}: {}", change.path, change.reason))
                END FOR
            END IF
        END IF
    END FOR

    // Summary
    PRINT("")
    PRINT(FORMAT("Migration complete: {} files, {} succeeded, {} failed",
        LENGTH(results),
        COUNT(results, r -> r.success),
        COUNT(results, r -> NOT r.success)
    ))

    RETURN IF has_errors THEN EXIT_FAILURE ELSE EXIT_SUCCESS
END
```

### 4.6 Complexity Analysis: Migration

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Read config | O(n) | O(n) where n = file size |
| Version detection | O(f) | O(1) where f = fields |
| Build plan | O(f + a) | O(c) where a = attributes, c = changes |
| Apply migration | O(c) | O(n) for deep clone |
| **Total** | O(n + f + a + c) | O(n + c) |

---

## 5. Phase R: Release Workflow Algorithms

### 5.1 Create Release

```
ALGORITHM: create_release
INPUT:
    version: String                     # e.g., "1.2.0"
    description: String
    changes: Array<DeployChange>
OUTPUT:
    ReleaseResult

BEGIN
    // ========================================
    // PHASE 1: Validate Version Format
    // ========================================
    IF NOT is_valid_semver(version) THEN
        RETURN ReleaseResult {
            success: false,
            release_version: version,
            errors: ["Invalid semver format. Expected MAJOR.MINOR.PATCH"]
        }
    END IF

    // Check version doesn't already exist
    existing_tags <- GIT_TAGS_LIST("v*")
    IF ("v" + version) IN existing_tags THEN
        RETURN ReleaseResult {
            success: false,
            release_version: version,
            errors: [FORMAT("Version v{} already exists", version)]
        }
    END IF

    // ========================================
    // PHASE 2: Create Manifest
    // ========================================
    manifest <- ReleaseManifest {
        schema: "../schemas/manifest.schema.json",
        version: "1.0",
        release_version: version,
        description: description,
        created_at: GET_CURRENT_TIME(),
        changes: changes,
        rollback_info: create_rollback_info(existing_tags)
    }

    manifest_path <- FORMAT(".deploy/releases/v{}.manifest.json", version)
    manifest_json <- SERIALIZE_JSON(manifest, pretty: true)

    // Validate manifest against schema
    validation <- validate_manifest(manifest)
    IF NOT validation.valid THEN
        RETURN ReleaseResult {
            success: false,
            release_version: version,
            errors: validation.errors
        }
    END IF

    // Write manifest file
    WRITE_FILE(manifest_path, manifest_json)
    LOG_INFO("Created release manifest", path: manifest_path)

    // ========================================
    // PHASE 3: Update CHANGELOG
    // ========================================
    changelog_entry <- generate_changelog_entry(version, description, changes)
    prepend_to_changelog(CHANGELOG_PATH, changelog_entry)
    LOG_INFO("Updated CHANGELOG.md")

    // ========================================
    // PHASE 4: Create Git Tag
    // ========================================
    tag_name <- "v" + version
    tag_message <- FORMAT("Release {} - {}", version, description)

    // Stage changes
    GIT_ADD(manifest_path)
    GIT_ADD(CHANGELOG_PATH)

    // Commit
    commit_message <- FORMAT("release: v{}\n\n{}", version, description)
    GIT_COMMIT(commit_message)

    // Create annotated tag
    GIT_TAG(tag_name, tag_message, annotated: true)

    LOG_INFO("Created git tag", tag: tag_name)

    RETURN ReleaseResult {
        success: true,
        release_version: version,
        git_tag: tag_name,
        manifest_path: manifest_path,
        changelog_entry: changelog_entry,
        errors: []
    }
END

// ----------------------------------------
// Helper: Validate Semver
// ----------------------------------------
FUNCTION is_valid_semver(version) -> Boolean:
    REGEX pattern <- /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[\da-zA-Z-]+(\.[da-zA-Z-]+)*)?(\+[\da-zA-Z-]+(\.[da-zA-Z-]+)*)?$/
    RETURN pattern.matches(version)
END

// ----------------------------------------
// Helper: Create Rollback Info
// ----------------------------------------
FUNCTION create_rollback_info(existing_tags) -> Option<RollbackInfo>:
    IF existing_tags.is_empty() THEN
        RETURN None
    END IF

    // Find latest version tag
    version_tags <- FILTER(existing_tags, t -> t.matches(/^v\d+\.\d+\.\d+$/))
    IF version_tags.is_empty() THEN
        RETURN None
    END IF

    latest <- SORT(version_tags, semver_compare).last()

    RETURN Some(RollbackInfo {
        previous_version: latest.strip_prefix("v"),
        rollback_manifest: FORMAT(".deploy/releases/{}.manifest.json", latest),
        auto_rollback: false
    })
END
```

### 5.2 Deploy Release

```
ALGORITHM: deploy_release
INPUT:
    manifest_path: Path
    environment: String                 # "pi", "dev", "ci"
OUTPUT:
    DeployResult

BEGIN
    // ========================================
    // PHASE 1: Load and Validate Manifest
    // ========================================
    manifest_content <- READ_FILE(manifest_path)
    IF manifest_content IS error THEN
        RETURN DeployResult {
            success: false,
            error: Some(FORMAT("Cannot read manifest: {}", manifest_content.error))
        }
    END IF

    manifest <- PARSE_JSON(manifest_content)
    IF manifest IS error THEN
        RETURN DeployResult {
            success: false,
            error: Some(FORMAT("Invalid manifest JSON: {}", manifest.error))
        }
    END IF

    release_version <- manifest["release_version"]

    LOG_INFO("Deploying release",
        version: release_version,
        environment: environment,
        changes: LENGTH(manifest["changes"])
    )

    // ========================================
    // PHASE 2: Pre-Deploy Validation
    // ========================================
    // Validate all configs referenced in manifest
    FOR EACH change IN manifest["changes"] DO
        IF change["change_type"] == "config" AND change["action"] IN {"add", "modify"} THEN
            config_path <- change["file_path"]
            validation <- AWAIT validate_config(config_path)

            IF NOT validation.valid THEN
                RETURN DeployResult {
                    success: false,
                    error: Some(FORMAT(
                        "Config validation failed for {}: {}",
                        config_path, validation.errors[0].message
                    ))
                }
            END IF
        END IF
    END FOR

    // ========================================
    // PHASE 3: Execute Deploy (via deploy.sh)
    // ========================================
    deploy_command <- FORMAT("./deploy.sh apply {}", manifest_path)

    result <- EXECUTE_SHELL(deploy_command, env: {
        "DEPLOY_ENV": environment,
        "DEPLOY_VERSION": release_version
    })

    IF result.exit_code != 0 THEN
        LOG_ERROR("Deploy failed",
            exit_code: result.exit_code,
            stderr: result.stderr
        )

        // Attempt auto-rollback if configured
        IF manifest["rollback_info"]?.["auto_rollback"] == true THEN
            LOG_WARN("Attempting auto-rollback")
            rollback_manifest <- manifest["rollback_info"]["rollback_manifest"]
            EXECUTE_SHELL(FORMAT("./deploy.sh apply {}", rollback_manifest))
        END IF

        RETURN DeployResult {
            success: false,
            error: Some(FORMAT("Deploy failed: {}", result.stderr))
        }
    END IF

    // ========================================
    // PHASE 4: Update Deployed Version Tracking
    // ========================================
    deployed_version <- DeployedVersion {
        version: release_version,
        deployed_at: GET_CURRENT_TIME(),
        manifest_path: manifest_path.to_string(),
        deployer: GET_ENV("USER").unwrap_or("system"),
        environment: environment
    }

    version_file <- "/var/ndp/deployed-version"
    version_json <- SERIALIZE_JSON(deployed_version, pretty: true)
    WRITE_FILE(version_file, version_json)

    LOG_INFO("Updated deployed version",
        version_file: version_file,
        version: release_version
    )

    RETURN DeployResult {
        success: true,
        version: release_version,
        deployed_at: deployed_version.deployed_at
    }
END
```

### 5.3 Generate Changelog Entry

```
ALGORITHM: generate_changelog_entry
INPUT:
    version: String
    description: String
    changes: Array<DeployChange>
OUTPUT:
    changelog_entry: String

BEGIN
    date <- FORMAT_DATE(GET_CURRENT_TIME(), "YYYY-MM-DD")

    // Group changes by type
    config_changes <- FILTER(changes, c -> c.change_type == "config")
    ddl_changes <- FILTER(changes, c -> c.change_type == "ddl")
    etcd_changes <- FILTER(changes, c -> c.change_type == "etcd")

    // Build markdown entry
    entry <- FORMAT("## [{}] - {}\n\n", version, date)
    entry <- entry + description + "\n\n"

    IF LENGTH(config_changes) > 0 THEN
        entry <- entry + "### Configuration Changes\n\n"
        FOR EACH change IN config_changes DO
            action_emoji <- MATCH change.action WITH
                "add" -> "+"
                "modify" -> "~"
                "delete" -> "-"
                _ -> "*"
            END MATCH
            entry <- entry + FORMAT("- [{}] {}: {}\n",
                action_emoji, change.stream_id, change.description)
        END FOR
        entry <- entry + "\n"
    END IF

    IF LENGTH(ddl_changes) > 0 THEN
        entry <- entry + "### Schema Changes\n\n"
        FOR EACH change IN ddl_changes DO
            entry <- entry + FORMAT("- {}\n", change.description)
        END FOR
        entry <- entry + "\n"
    END IF

    RETURN entry
END

// ----------------------------------------
// Helper: Prepend to Changelog
// ----------------------------------------
FUNCTION prepend_to_changelog(changelog_path, new_entry):
    existing <- READ_FILE(changelog_path).unwrap_or("# Changelog\n\n")

    // Find where to insert (after header)
    header_end <- existing.find("\n## ")
    IF header_end IS NOT NULL THEN
        updated <- existing[0..header_end] + "\n" + new_entry + existing[header_end..]
    ELSE
        updated <- existing + "\n" + new_entry
    END IF

    WRITE_FILE(changelog_path, updated)
END
```

### 5.4 Release CLI

```
ALGORITHM: release_cli_main
INPUT:
    subcommand: String
    args: CliArgs
OUTPUT:
    ExitCode

BEGIN
    MATCH subcommand WITH
        "create" ->
            version <- args.version
            IF version IS NULL THEN
                PRINT_ERROR("--version required")
                RETURN EXIT_USAGE
            END IF

            description <- args.description.unwrap_or("")
            changes <- load_changes_from_args_or_interactive(args)

            result <- create_release(version, description, changes)

            IF result.success THEN
                PRINT_SUCCESS(FORMAT("Release v{} created", result.release_version))
                PRINT(FORMAT("  Manifest: {}", result.manifest_path))
                PRINT(FORMAT("  Git tag: {}", result.git_tag))

                IF args.push THEN
                    GIT_PUSH("--tags")
                    PRINT("Pushed to remote")
                END IF

                RETURN EXIT_SUCCESS
            ELSE
                PRINT_ERROR("Release creation failed:")
                FOR EACH error IN result.errors DO
                    PRINT_ERROR(FORMAT("  - {}", error))
                END FOR
                RETURN EXIT_FAILURE
            END IF

        "deploy" ->
            manifest <- args.manifest
            environment <- args.env.unwrap_or("pi")

            IF manifest IS NULL THEN
                // Find latest manifest
                manifest <- find_latest_manifest()
            END IF

            result <- deploy_release(manifest, environment)

            IF result.success THEN
                PRINT_SUCCESS(FORMAT("Deployed v{}", result.version))
                RETURN EXIT_SUCCESS
            ELSE
                PRINT_ERROR(FORMAT("Deploy failed: {}", result.error))
                RETURN EXIT_FAILURE
            END IF

        "list" ->
            manifests <- GLOB(".deploy/releases/v*.manifest.json")
            SORT(manifests, semver_from_path, reverse: true)

            PRINT("Available releases:")
            FOR EACH manifest IN manifests DO
                content <- READ_FILE(manifest)
                parsed <- PARSE_JSON(content)
                PRINT(FORMAT("  v{} - {} ({})",
                    parsed["release_version"],
                    parsed["description"],
                    parsed["created_at"]
                ))
            END FOR
            RETURN EXIT_SUCCESS

        "status" ->
            version_file <- "/var/ndp/deployed-version"
            IF FILE_EXISTS(version_file) THEN
                content <- READ_FILE(version_file)
                deployed <- PARSE_JSON(content)
                PRINT(FORMAT("Deployed version: v{}", deployed["version"]))
                PRINT(FORMAT("  Deployed at: {}", deployed["deployed_at"]))
                PRINT(FORMAT("  Environment: {}", deployed["environment"]))
            ELSE
                PRINT("No version deployed yet")
            END IF
            RETURN EXIT_SUCCESS

        _ ->
            PRINT_ERROR(FORMAT("Unknown subcommand: {}", subcommand))
            RETURN EXIT_USAGE
    END MATCH
END
```

### 5.5 Manifest Schema

```
ALGORITHM: validate_manifest
INPUT:
    manifest: ReleaseManifest
OUTPUT:
    ValidationResult

BEGIN
    errors <- []

    // Required fields
    IF manifest.release_version IS NULL OR manifest.release_version == "" THEN
        errors <- errors + ["release_version is required"]
    ELSE IF NOT is_valid_semver(manifest.release_version) THEN
        errors <- errors + ["release_version must be valid semver"]
    END IF

    IF manifest.version IS NULL OR manifest.version != "1.0" THEN
        errors <- errors + ["version must be '1.0' (manifest schema version)"]
    END IF

    // Validate changes array
    IF manifest.changes IS NULL THEN
        errors <- errors + ["changes array is required"]
    ELSE
        FOR idx, change IN ENUMERATE(manifest.changes) DO
            IF change.change_type NOT IN {"config", "ddl", "etcd", "other"} THEN
                errors <- errors + [FORMAT(
                    "changes[{}].change_type must be config|ddl|etcd|other",
                    idx
                )]
            END IF

            IF change.action NOT IN {"add", "modify", "delete", "none"} THEN
                errors <- errors + [FORMAT(
                    "changes[{}].action must be add|modify|delete|none",
                    idx
                )]
            END IF
        END FOR
    END IF

    RETURN ValidationResult {
        valid: LENGTH(errors) == 0,
        errors: errors
    }
END
```

### 5.6 Complexity Analysis: Release

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Version validation | O(1) | O(1) |
| Tag listing | O(t) | O(t) where t = tags |
| Manifest creation | O(c) | O(c) where c = changes |
| Changelog update | O(n) | O(n) where n = changelog size |
| Git operations | O(1) | O(1) |
| Deploy | O(c) | O(1) |
| **Total create** | O(t + c + n) | O(t + c + n) |

---

## 6. State Machine Diagrams

### 6.1 Source Lifecycle State Machine

```
                    ┌─────────────────────────────────────────┐
                    │                                         │
                    ▼                                         │
    ┌──────────┐ spawn ┌─────────┐ config_change ┌──────────┐│
    │ STOPPED  │──────>│ RUNNING │─────────────>│ UPDATING ││
    └──────────┘       └─────────┘               └──────────┘│
         ▲                 │                         │       │
         │                 │ stop                    │       │
         │                 │                         │       │
         │                 ▼                         │       │
         │            ┌──────────┐                   │       │
         └────────────│ STOPPING │<──────────────────┘       │
                      └──────────┘                           │
                           │                                 │
                           │ stopped                         │
                           │                                 │
                           ▼                                 │
                      ┌──────────┐                           │
                      │ STOPPED  │───────────────────────────┘
                      └──────────┘
```

### 6.2 Config Migration State Machine

```
    ┌─────────┐
    │ v1.0    │  (YAML, entity_schemas required)
    └────┬────┘
         │ dp-018 migration
         ▼
    ┌─────────┐
    │ v1.1    │  (JSON, entity_schemas deprecated, fields enriched)
    └────┬────┘
         │ dp-021 migration (this feature)
         ▼
    ┌─────────┐
    │ v2.0    │  (JSON, entity_schemas forbidden, fields required)
    └─────────┘
```

### 6.3 Release Lifecycle

```
    ┌────────────┐
    │  DEVELOP   │
    └─────┬──────┘
          │ create manifest
          ▼
    ┌────────────┐
    │  STAGED    │
    └─────┬──────┘
          │ git tag + commit
          ▼
    ┌────────────┐
    │  TAGGED    │
    └─────┬──────┘
          │ deploy.sh apply
          ▼
    ┌────────────┐
    │  DEPLOYED  │
    └────────────┘
```

---

## 7. Error Handling

### 7.1 Hot-Reload Error Scenarios

```
ENUM HotReloadError:
    CONFIG_VALIDATION_FAILED {
        stream_id: String
        errors: Array<ValidationError>
    }

    SOURCE_START_FAILED {
        source_id: String
        reason: String
    }

    MQTT_RECONNECT_FAILED {
        client_id: String
        attempt: u32
        error: String
    }

    HTTP_UPDATE_FAILED {
        source_id: String
        reason: String
    }

    ETCD_WATCH_ERROR {
        key: String
        error: String
    }

ALGORITHM: handle_hot_reload_error
INPUT:
    error: HotReloadError
    source_manager: &SourceManager
OUTPUT:
    RecoveryAction

BEGIN
    MATCH error WITH
        CONFIG_VALIDATION_FAILED {stream_id, errors} ->
            // Log but don't crash - keep existing sources running
            LOG_ERROR("Config validation failed, keeping current sources",
                stream_id: stream_id,
                errors: errors
            )
            RETURN RecoveryAction::KEEP_CURRENT

        SOURCE_START_FAILED {source_id, reason} ->
            // Retry with exponential backoff
            LOG_WARN("Source start failed, will retry",
                source_id: source_id,
                reason: reason
            )
            RETURN RecoveryAction::RETRY_WITH_BACKOFF {
                max_attempts: 3,
                initial_delay_ms: 1000,
                max_delay_ms: 30000
            }

        MQTT_RECONNECT_FAILED {client_id, attempt, error} ->
            IF attempt < 5 THEN
                // Retry reconnect
                RETURN RecoveryAction::RETRY_WITH_BACKOFF {
                    max_attempts: 5 - attempt,
                    initial_delay_ms: 1000 * attempt,
                    max_delay_ms: 60000
                }
            ELSE
                // Give up, mark source as unhealthy
                LOG_ERROR("MQTT reconnect failed after max attempts",
                    client_id: client_id
                )
                RETURN RecoveryAction::MARK_UNHEALTHY
            END IF

        HTTP_UPDATE_FAILED {source_id, reason} ->
            // HTTP sources are stateless, safe to retry immediately
            LOG_WARN("HTTP source update failed",
                source_id: source_id,
                reason: reason
            )
            RETURN RecoveryAction::RETRY_IMMEDIATE

        ETCD_WATCH_ERROR {key, error} ->
            // Re-establish watch
            LOG_ERROR("etcd watch error, reconnecting",
                key: key,
                error: error
            )
            RETURN RecoveryAction::RECONNECT_WATCH
    END MATCH
END
```

### 7.2 Migration Error Scenarios

```
ENUM MigrationError:
    FILE_NOT_FOUND {path: Path}
    INVALID_JSON {path: Path, error: String}
    UNSUPPORTED_VERSION {version: String}
    BACKUP_FAILED {path: Path, error: String}
    WRITE_FAILED {path: Path, error: String}
    PARTIAL_MIGRATION {completed: u32, total: u32}

ALGORITHM: handle_migration_error
INPUT:
    error: MigrationError
    options: MigrationOptions
OUTPUT:
    RecoveryAction

BEGIN
    MATCH error WITH
        BACKUP_FAILED {path, error} ->
            IF options.require_backup THEN
                LOG_ERROR("Backup required but failed", path: path, error: error)
                RETURN RecoveryAction::ABORT
            ELSE
                LOG_WARN("Backup failed, continuing without backup")
                RETURN RecoveryAction::CONTINUE_WITHOUT_BACKUP
            END IF

        PARTIAL_MIGRATION {completed, total} ->
            LOG_ERROR("Partial migration failure",
                completed: completed,
                total: total
            )
            // Restore from backup if available
            RETURN RecoveryAction::ROLLBACK_FROM_BACKUP

        _ ->
            // All other errors are fatal
            RETURN RecoveryAction::ABORT
    END MATCH
END
```

---

## 8. Design Patterns Used

### 8.1 Observer Pattern (etcd Watch)

```
INTERFACE ConfigObserver:
    on_config_change(event: ConfigChangeEvent)

CLASS SourceManagerObserver IMPLEMENTS ConfigObserver:
    source_manager: SourceManager

    on_config_change(event):
        handle_stream_updated(event.stream_id, event.new_config, self.source_manager)

CLASS EtcdWatcher:
    observers: List<ConfigObserver>

    add_observer(observer):
        observers.append(observer)

    notify_change(event):
        FOR EACH observer IN observers DO
            TRY
                observer.on_config_change(event)
            CATCH error
                LOG_ERROR("Observer failed", error: error)
            END TRY
        END FOR
```

### 8.2 Strategy Pattern (Source Update)

```
INTERFACE SourceUpdateStrategy:
    update(old_config: SourceConfig, new_config: SourceConfig) -> Result

CLASS MqttUpdateStrategy IMPLEMENTS SourceUpdateStrategy:
    update(old_config, new_config):
        RETURN graceful_mqtt_reconnect(...)

CLASS HttpUpdateStrategy IMPLEMENTS SourceUpdateStrategy:
    update(old_config, new_config):
        RETURN update_http_poll_source(...)

CLASS SourceUpdater:
    strategies: Map<SourceType, SourceUpdateStrategy>

    update_source(source_type, old_config, new_config):
        strategy <- strategies.get(source_type)
        RETURN strategy.update(old_config, new_config)
```

### 8.3 Command Pattern (Migration)

```
INTERFACE MigrationCommand:
    execute(config: JsonValue) -> JsonValue
    undo(config: JsonValue) -> JsonValue

CLASS RemoveFieldCommand IMPLEMENTS MigrationCommand:
    field_path: String
    backup_value: Option<JsonValue>

    execute(config):
        backup_value <- json_get(config, field_path)
        RETURN json_remove(config, field_path)

    undo(config):
        IF backup_value IS NOT NULL THEN
            RETURN json_set(config, field_path, backup_value)
        END IF
        RETURN config

CLASS MigrationExecutor:
    commands: List<MigrationCommand>

    execute_all(config):
        result <- config
        FOR EACH command IN commands DO
            result <- command.execute(result)
        END FOR
        RETURN result

    rollback_all(config):
        result <- config
        FOR EACH command IN REVERSE(commands) DO
            result <- command.undo(result)
        END FOR
        RETURN result
```

---

## 9. Integration Points

### 9.1 Hot-Reload Integration with Coordinator

```
// In apps/air-quality-app/src/main.rs or coordinator.rs

ALGORITHM: integrate_hot_reload
INPUT:
    config_registry: ConfigRegistry
    source_manager: SourceManager

BEGIN
    // Create etcd watcher
    watcher <- EtcdWatcher::new(config_registry.etcd_client())

    // Create observer that updates sources
    observer <- SourceManagerObserver::new(source_manager)
    watcher.add_observer(observer)

    // Start watching config keys
    watcher.watch("/ndp/streams/*/config")

    // Optional: Add reload HTTP endpoint
    api_router.route("/api/reload", handle_reload_request)
END
```

### 9.2 Migration Integration with deploy.sh

```bash
# In deploy/pi/deploy.sh or scripts/ndp-migrate-config.sh

ndp_migrate_configs() {
    local dry_run="${1:-false}"

    echo "Checking config versions..."

    # Find all v1.1 configs
    local v1_configs=$(find config/base/streams -name "config.json" -exec \
        sh -c 'grep -l "entity_schemas" "$1" 2>/dev/null || true' _ {} \;)

    if [ -z "$v1_configs" ]; then
        echo "All configs already at v2.0"
        return 0
    fi

    echo "Found $(echo "$v1_configs" | wc -l) v1.1 configs to migrate"

    if [ "$dry_run" = "true" ]; then
        ndp-migrate-config --all --from 1.1 --to 2 --dry-run
    else
        ndp-migrate-config --all --from 1.1 --to 2 --backup
    fi
}
```

### 9.3 Release Integration with GitHub Actions

```yaml
# In .github/workflows/release.yml (future dp-023)

on:
  push:
    tags:
      - 'v*'

jobs:
  deploy:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4

      - name: Extract version
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Deploy release
        run: |
          ./deploy.sh apply .deploy/releases/v${{ steps.version.outputs.VERSION }}.manifest.json
```

---

## 10. Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Hot-reload latency | < 5s | Time from config change to source ready |
| MQTT message loss | 0 | During reconnection |
| Migration per config | < 100ms | v1.1 to v2.0 |
| Release creation | < 10s | Including git operations |
| Deploy execution | < 60s | Full manifest apply |

---

## 11. References

| Document | Purpose |
|----------|---------|
| `dp-021/SCOPE.md` | Feature scope and requirements |
| `dp-018/specification/` | JSON Config Foundation |
| `dp-019/pseudocode/PSEUDOCODE.md` | Validation pipeline pseudocode |
| `dp-020/specification/` | Declarative deploy specification |
| `core/src/coordinator/source_manager.rs` | Current source manager implementation |
| `core/src/types/stream_config.rs` | StreamConfig type definitions |

---

*Pseudocode created: 2026-02-02*
*SPARC Phase: Pseudocode (P)*
*Next Phase: Architecture (A)*
*Then: Refinement (R) - TDD Implementation*
