# dp-018: JSON Config Foundation - Pseudocode Design

## Document Overview

This document provides the algorithmic design for dp-018's JSON pass-through architecture:
- **Phase 0**: JSON Migration (YAML to JSON with field enrichment)
- **Phase 1**: Pass-Through Sync (ConfigSyncService simplification)

### Architecture Principle: Pass-Through, Not Transformation

The fundamental design principle is **pass-through**:

```
JSON file (source of truth)
    | validate against JSON Schema
    | deserialize to StreamConfig
    v
etcd (same JSON blob)
    | deserialize
    v
StreamConfig (complete, including silver_etl)
```

**Key insight**: We do NOT create:
- ConfigLoader trait (not needed - use existing StreamRegistry)
- EtcdConfigLoader (not needed - StreamRegistry already does this)
- SilverRegistry (not needed - Silver uses same StreamRegistry as Bronze)

Both Bronze and Silver use the same `StreamRegistry.load_stream()` method.

---

## Phase 0: JSON Migration

### 0.1 Migration Script Master Algorithm

```
ALGORITHM: MigrateYamlToJson
INPUT: config_dir (path to config/base/streams/)
OUTPUT: migration_results (array of MigrationResult)

CONSTANTS:
    SCHEMA_VERSION = 1.1
    SCHEMA_FILE = "schemas/stream-config.v1.schema.json"

BEGIN
    // Phase 1: Discovery
    stream_dirs <- FindAllStreamDirectories(config_dir)
    results <- []

    LOG "Found {length(stream_dirs)} streams to migrate"

    // Phase 2: Process each stream
    FOR EACH stream_dir IN stream_dirs DO
        result <- MigrateSingleStream(stream_dir)
        results.append(result)

        IF result.success THEN
            LOG "SUCCESS: {stream_dir.name} migrated"
        ELSE
            LOG "ERROR: {stream_dir.name} failed: {result.error}"
        END IF
    END FOR

    // Phase 3: Summary
    success_count <- COUNT(results WHERE success = true)
    failure_count <- COUNT(results WHERE success = false)

    LOG "Migration complete: {success_count} succeeded, {failure_count} failed"

    IF failure_count > 0 THEN
        RETURN (false, results)
    ELSE
        RETURN (true, results)
    END IF
END

STRUCTURE MigrationResult:
    stream_id: string
    success: boolean
    error: string (optional)
    yaml_path: string
    json_path: string
    fields_enriched: integer
```

### 0.2 Single Stream Migration Algorithm

```
ALGORITHM: MigrateSingleStream
INPUT: stream_dir (path to stream directory)
OUTPUT: MigrationResult

BEGIN
    yaml_path <- stream_dir + "/config.yaml"
    json_path <- stream_dir + "/config.json"

    // Step 1: Check if YAML exists
    IF NOT FileExists(yaml_path) THEN
        RETURN MigrationResult(
            stream_id: basename(stream_dir),
            success: false,
            error: "config.yaml not found"
        )
    END IF

    // Step 2: Check idempotency (already migrated?)
    IF FileExists(json_path) THEN
        existing_json <- ReadJsonFile(json_path)
        IF existing_json.config_version >= 1.1 THEN
            LOG "Stream already migrated to v1.1, skipping"
            RETURN MigrationResult(
                stream_id: existing_json.stream_id,
                success: true,
                json_path: json_path
            )
        END IF
    END IF

    // Step 3: Parse YAML
    TRY
        yaml_content <- ReadYamlFile(yaml_path)
    CATCH parse_error
        RETURN MigrationResult(
            success: false,
            error: "YAML parse error: " + parse_error.message
        )
    END TRY

    // Step 4: Convert structure to JSON format
    json_config <- ConvertYamlStructureToJson(yaml_content)

    // Step 5: Enrich fields with entity_schemas data
    enrichment_result <- EnrichFieldsFromEntitySchemas(json_config)
    json_config <- enrichment_result.config

    // Step 6: Set schema version
    json_config.config_version <- 1.1

    // Step 7: Validate against schema
    validation_result <- ValidateAgainstSchema(json_config, SCHEMA_FILE)
    IF NOT validation_result.valid THEN
        RETURN MigrationResult(
            success: false,
            error: "Schema validation failed: " + validation_result.errors
        )
    END IF

    // Step 8: Write JSON file
    WriteJsonFile(json_path, json_config, pretty_print: true)

    RETURN MigrationResult(
        stream_id: json_config.stream_id,
        success: true,
        yaml_path: yaml_path,
        json_path: json_path,
        fields_enriched: enrichment_result.fields_enriched_count
    )
END
```

### 0.3 YAML to JSON Structure Conversion

```
ALGORITHM: ConvertYamlStructureToJson
INPUT: yaml_content (YAML object)
OUTPUT: json_config (JSON object)

BEGIN
    json_config <- {}

    // Copy top-level scalar fields directly
    SCALAR_FIELDS <- [
        "stream_id", "description", "version", "enabled",
        "retention_days", "compression_after_days", "partitioning_strategy"
    ]

    FOR EACH field IN SCALAR_FIELDS DO
        IF yaml_content HAS field THEN
            json_config[field] <- yaml_content[field]
        END IF
    END FOR

    // Convert fields from map to array format
    // YAML: fields:
    //         pm25:
    //           type: "float"
    // JSON: fields: [{ name: "pm25", type: "float" }]

    IF yaml_content HAS "fields" THEN
        json_config.fields <- ConvertFieldsMapToArray(yaml_content.fields)
    ELSE
        json_config.fields <- []
    END IF

    // Copy sources array (already array format)
    IF yaml_content HAS "sources" THEN
        json_config.sources <- yaml_content.sources
    END IF

    // Copy storage config
    IF yaml_content HAS "storage" THEN
        json_config.storage <- yaml_content.storage
    END IF

    // Copy entity_schemas (will be deprecated but kept for v1.1 compatibility)
    IF yaml_content HAS "entity_schemas" THEN
        json_config.entity_schemas <- yaml_content.entity_schemas
    END IF

    // Copy silver_etl config (CRITICAL - this was being lost before)
    IF yaml_content HAS "silver_etl" THEN
        json_config.silver_etl <- yaml_content.silver_etl
    END IF

    RETURN json_config
END

ALGORITHM: ConvertFieldsMapToArray
INPUT: fields_map (map of field_name -> field_config)
OUTPUT: fields_array (array of field objects)

BEGIN
    fields_array <- []

    FOR EACH (field_name, field_config) IN fields_map DO
        field_obj <- {
            name: field_name,
            type: field_config.type
        }

        // Copy optional attributes
        OPTIONAL_ATTRS <- ["unit", "description", "nullable", "range", "device_class"]
        FOR EACH attr IN OPTIONAL_ATTRS DO
            IF field_config HAS attr THEN
                field_obj[attr] <- field_config[attr]
            END IF
        END FOR

        fields_array.append(field_obj)
    END FOR

    RETURN fields_array
END
```

### 0.4 Field Enrichment Algorithm

```
ALGORITHM: EnrichFieldsFromEntitySchemas
INPUT: json_config (JSON config object)
OUTPUT: { config: enriched_config, fields_enriched_count: integer }

BEGIN
    enriched_count <- 0

    // Skip if no entity_schemas
    IF NOT json_config HAS "entity_schemas" OR json_config.entity_schemas IS EMPTY THEN
        RETURN { config: json_config, fields_enriched_count: 0 }
    END IF

    // Build lookup map from entity_schemas
    // entity_schemas structure:
    // [{ schema_name: "airgradient", device_class: "air_quality",
    //    attributes: [{ name: "pm25", description: "...", device_class: "sensor" }] }]

    schema_lookup <- BuildEntitySchemaLookup(json_config.entity_schemas)

    // Enrich each field
    FOR EACH field IN json_config.fields DO
        // Find matching attribute in entity_schemas
        entity_attr <- schema_lookup.find(field.name)

        IF entity_attr IS NOT NULL THEN
            // Enrich description if not already present
            IF NOT field HAS "description" OR field.description IS EMPTY THEN
                IF entity_attr HAS "description" THEN
                    field.description <- entity_attr.description
                    enriched_count <- enriched_count + 1
                END IF
            END IF

            // Enrich device_class if not already present
            IF NOT field HAS "device_class" OR field.device_class IS EMPTY THEN
                IF entity_attr HAS "device_class" THEN
                    field.device_class <- entity_attr.device_class
                END IF
            END IF

            // Enrich range if not already present
            IF NOT field HAS "range" OR field.range IS EMPTY THEN
                IF entity_attr HAS "range" THEN
                    field.range <- entity_attr.range
                END IF
            END IF

            // Enrich unit if not already present
            IF NOT field HAS "unit" OR field.unit IS EMPTY THEN
                IF entity_attr HAS "unit" THEN
                    field.unit <- entity_attr.unit
                END IF
            END IF
        END IF
    END FOR

    RETURN { config: json_config, fields_enriched_count: enriched_count }
END

ALGORITHM: BuildEntitySchemaLookup
INPUT: entity_schemas (array of entity schema objects)
OUTPUT: lookup_map (map of field_name -> attribute_config)

BEGIN
    lookup_map <- {}

    FOR EACH schema IN entity_schemas DO
        IF schema HAS "attributes" THEN
            FOR EACH attr IN schema.attributes DO
                // First occurrence wins (if multiple schemas have same attribute name)
                IF NOT lookup_map HAS attr.name THEN
                    lookup_map[attr.name] <- attr
                END IF
            END FOR
        END IF
    END FOR

    RETURN lookup_map
END
```

### 0.5 Schema Validation Algorithm

```
ALGORITHM: ValidateAgainstSchema
INPUT: json_config (JSON object), schema_path (path to JSON Schema file)
OUTPUT: ValidationResult

STRUCTURE ValidationResult:
    valid: boolean
    errors: array of ValidationError

STRUCTURE ValidationError:
    path: string (JSON path like "$.fields[0].type")
    message: string
    severity: "error" | "warning"

BEGIN
    errors <- []

    // Layer 1: JSON syntax validation (already done during parsing)

    // Layer 2: Schema structure validation
    schema <- LoadJsonSchema(schema_path)

    // Required fields check
    REQUIRED_FIELDS <- ["stream_id", "fields"]
    FOR EACH field IN REQUIRED_FIELDS DO
        IF NOT json_config HAS field OR json_config[field] IS NULL THEN
            errors.append(ValidationError(
                path: "$." + field,
                message: "Required field '" + field + "' is missing",
                severity: "error"
            ))
        END IF
    END FOR

    // stream_id format validation (lowercase letters, numbers, hyphens)
    IF json_config HAS "stream_id" THEN
        IF NOT Matches(json_config.stream_id, "^[a-z0-9-]+$") THEN
            errors.append(ValidationError(
                path: "$.stream_id",
                message: "stream_id must contain only lowercase letters, numbers, and hyphens",
                severity: "error"
            ))
        END IF
    END IF

    // Fields array validation
    IF json_config HAS "fields" AND json_config.fields IS ARRAY THEN
        FOR i <- 0 TO length(json_config.fields) - 1 DO
            field <- json_config.fields[i]
            field_errors <- ValidateField(field, i)
            errors <- errors + field_errors
        END FOR
    END IF

    // silver_etl validation (if present)
    IF json_config HAS "silver_etl" THEN
        etl_errors <- ValidateSilverEtlConfig(json_config.silver_etl)
        errors <- errors + etl_errors
    END IF

    // config_version validation
    IF json_config HAS "config_version" THEN
        IF json_config.config_version NOT IN [1, 1.1, 2] THEN
            errors.append(ValidationError(
                path: "$.config_version",
                message: "config_version must be 1, 1.1, or 2",
                severity: "error"
            ))
        END IF
    END IF

    RETURN ValidationResult(
        valid: length(errors WHERE severity = "error") = 0,
        errors: errors
    )
END

ALGORITHM: ValidateField
INPUT: field (field object), index (integer)
OUTPUT: errors (array of ValidationError)

BEGIN
    errors <- []
    base_path <- "$.fields[" + index + "]"

    // name is required
    IF NOT field HAS "name" OR field.name IS EMPTY THEN
        errors.append(ValidationError(
            path: base_path + ".name",
            message: "Field name is required",
            severity: "error"
        ))
    END IF

    // type is required
    IF NOT field HAS "type" OR field.type IS EMPTY THEN
        errors.append(ValidationError(
            path: base_path + ".type",
            message: "Field type is required",
            severity: "error"
        ))
    ELSE
        // Validate type value
        VALID_TYPES <- ["float", "int", "integer", "string", "boolean", "timestamp", "json"]
        IF field.type NOT IN VALID_TYPES THEN
            errors.append(ValidationError(
                path: base_path + ".type",
                message: "Invalid field type: " + field.type,
                severity: "error"
            ))
        END IF
    END IF

    // range validation (if present)
    IF field HAS "range" THEN
        IF NOT IsArray(field.range) OR length(field.range) != 2 THEN
            errors.append(ValidationError(
                path: base_path + ".range",
                message: "range must be an array of [min, max]",
                severity: "error"
            ))
        ELSE IF field.range[0] > field.range[1] THEN
            errors.append(ValidationError(
                path: base_path + ".range",
                message: "range min must be <= max",
                severity: "error"
            ))
        END IF
    END IF

    RETURN errors
END
```

---

## Phase 1: Pass-Through Config Sync

### 1.1 ConfigSyncService Algorithm (Simplified)

The key architectural change is **pass-through**: no transformation between JSON file and etcd.

```
ALGORITHM: ConfigSyncService.sync_stream
INPUT: json_path (path to config.json file)
OUTPUT: Result<(), SyncError>

DESCRIPTION:
    Syncs a JSON config file to etcd WITHOUT transformation.
    The JSON file is the source of truth.
    What goes in is what comes out.

BEGIN
    // Step 1: Read JSON file
    json_content <- read_file(json_path)

    // Step 2: Validate against JSON Schema (catches errors early)
    validation_result <- validate_against_schema(json_content, SCHEMA_FILE)
    IF NOT validation_result.valid THEN
        LOG ERROR "[sync] Schema validation failed for {json_path}: {validation_result.errors}"
        RETURN Err(SyncError::ValidationFailed(validation_result.errors))
    END IF

    // Step 3: Deserialize to StreamConfig (same struct everywhere)
    // NO to_stream_config() transformation - direct deserialization
    TRY
        config <- serde_json::from_str<StreamConfig>(json_content)
    CATCH parse_error
        LOG ERROR "[sync] JSON parse error for {json_path}: {parse_error}"
        RETURN Err(SyncError::ParseError(parse_error))
    END TRY

    // Step 4: Save to etcd (pass-through - serializes same struct)
    // Uses existing StreamRegistry - no new abstractions needed
    TRY
        registry.save_stream(config)
    CATCH save_error
        LOG ERROR "[sync] Failed to save to etcd for stream={config.stream_id}: {save_error}"
        RETURN Err(SyncError::EtcdError(save_error))
    END TRY

    LOG INFO "[sync] Config synced for stream={config.stream_id} config_version={config.config_version}"
    RETURN Ok(())
END

NOTE:
    BEFORE (lossy transformation):
        yaml: StreamConfigYaml = read_yaml(yaml_path)
        config: StreamConfig = yaml.to_stream_config()  // LOSSY - silver_etl lost
        registry.save_stream(config)

    AFTER (pass-through):
        json_content = read_file(json_path)
        validate_against_schema(json_content, schema)
        config: StreamConfig = serde_json::from_str(json_content)  // PASS-THROUGH
        registry.save_stream(config)
```

### 1.2 Batch Sync Algorithm

```
ALGORITHM: ConfigSyncService.sync_all_streams
INPUT: config_dir (path to config/base/streams/)
OUTPUT: SyncSummary

STRUCTURE SyncSummary:
    total: integer
    succeeded: integer
    failed: integer
    errors: array of { stream_id: string, error: string }

BEGIN
    stream_dirs <- FindAllStreamDirectories(config_dir)
    summary <- SyncSummary(total: length(stream_dirs))

    FOR EACH stream_dir IN stream_dirs DO
        json_path <- stream_dir + "/config.json"

        IF NOT FileExists(json_path) THEN
            LOG WARN "[sync] No config.json found for {stream_dir.name}, skipping"
            CONTINUE
        END IF

        result <- sync_stream(json_path)

        IF result IS Ok THEN
            summary.succeeded <- summary.succeeded + 1
        ELSE
            summary.failed <- summary.failed + 1
            summary.errors.append({
                stream_id: basename(stream_dir),
                error: result.error.message
            })
        END IF
    END FOR

    LOG INFO "[sync] Batch sync complete: {summary.succeeded}/{summary.total} succeeded"

    IF summary.failed > 0 THEN
        LOG ERROR "[sync] {summary.failed} streams failed to sync"
        FOR EACH err IN summary.errors DO
            LOG ERROR "[sync]   - {err.stream_id}: {err.error}"
        END FOR
    END IF

    RETURN summary
END
```

### 1.3 Silver Config Loading (Uses Same StreamRegistry)

```
ALGORITHM: get_silver_config
INPUT: registry (StreamRegistry), stream_id (string)
OUTPUT: Result<SilverEtlConfig, ConfigError>

DESCRIPTION:
    Silver uses the SAME StreamRegistry as Bronze.
    No separate SilverRegistry or ConfigLoader trait needed.
    The config is already complete in etcd (thanks to pass-through).

BEGIN
    // Step 1: Load stream config (same method Bronze uses)
    config_result <- registry.load_stream(stream_id)

    IF config_result IS Err THEN
        RETURN Err(config_result.error)
    END IF

    config <- config_result.unwrap()

    // Step 2: Extract silver_etl section
    // This now works because pass-through preserved silver_etl
    IF config.silver_etl IS None THEN
        LOG DEBUG "No silver_etl config for stream={stream_id}"
        RETURN Err(ConfigError::NotFound("silver_etl not configured"))
    END IF

    silver_etl <- config.silver_etl.unwrap()

    // Step 3: Check if enabled
    IF NOT silver_etl.enabled THEN
        LOG DEBUG "Silver ETL disabled for stream={stream_id}"
        RETURN Err(ConfigError::NotFound("silver_etl is disabled"))
    END IF

    LOG INFO "Silver config loaded for stream={stream_id} target_table={silver_etl.target_table}"
    RETURN Ok(silver_etl)
END

NOTE:
    BEFORE: SilverSubscriber had its own load_silver_etl_config() that read YAML files directly
    AFTER:  SilverSubscriber calls registry.load_stream() and accesses .silver_etl

    This unifies Bronze and Silver config loading.
```

### 1.4 Silver Subscriber Event Handler

```
ALGORITHM: SilverSubscriber.on_bronze_event
INPUT: event (BronzeEvent)
OUTPUT: ProcessingResult

STATE:
    config_cache: Map<stream_id, CachedConfig>
    registry: StreamRegistry (injected - same as Bronze uses)

STRUCTURE CachedConfig:
    config: SilverEtlConfig
    cached_at: timestamp
    ttl: duration (default 300 seconds)

BEGIN
    stream_id <- event.stream_id

    // Step 1: Check config cache
    IF config_cache.has(stream_id) AND NOT config_cache.get(stream_id).is_expired() THEN
        config <- config_cache.get(stream_id).config
    ELSE
        // Load/reload config using StreamRegistry
        config_result <- get_silver_config(registry, stream_id)

        IF config_result IS Err THEN
            RETURN handle_config_error(config_result.error, event)
        END IF

        config <- config_result.unwrap()

        // Cache the config
        config_cache.set(stream_id, CachedConfig(
            config: config,
            cached_at: now(),
            ttl: 300 seconds
        ))
    END IF

    // Step 2: Process event with config
    RETURN process_with_config(event, config)
END

ALGORITHM: handle_config_error
INPUT: error (ConfigError), event (BronzeEvent)
OUTPUT: ProcessingResult

BEGIN
    MATCH error:
        ConfigError::NotFound(_):
            // Stream not configured for Silver ETL - normal case
            LOG DEBUG "Stream {event.stream_id} not configured for Silver ETL"
            RETURN ProcessingResult::Skipped

        ConfigError::ConnectionError(_):
            // Transient error - should retry
            LOG WARN "Config fetch failed for stream {event.stream_id}, will retry"
            RETURN ProcessingResult::RetryLater

        _:
            // Permanent error
            LOG ERROR "Config error for stream {event.stream_id}: {error}"
            RETURN ProcessingResult::Error(error)
    END MATCH
END
```

---

## Phase 1: Dictionary Loader (Field Description Fallback)

### 1.5 Dictionary Loader with Fields Fallback

```
ALGORITHM: DictionaryLoader.get_field_description
INPUT: stream_config (StreamConfig), field_name (string)
OUTPUT: Option<string>

DESCRIPTION:
    Retrieves field description with fallback logic for v1.0/v1.1 compatibility.
    - v1.1+: Reads from fields[].description (preferred)
    - v1.0: Falls back to entity_schemas[].attributes[].description

BEGIN
    // Priority 1: Try enriched fields first (v1.1+ pattern)
    IF stream_config HAS "fields" AND stream_config.fields IS NOT EMPTY THEN
        FOR EACH field IN stream_config.fields DO
            IF field.name = field_name THEN
                IF field HAS "description" AND field.description IS NOT EMPTY THEN
                    LOG DEBUG "Description found in fields for {field_name}"
                    RETURN Some(field.description)
                END IF
            END IF
        END FOR
    END IF

    // Priority 2: Fallback to entity_schemas (v1.0 compatibility)
    IF stream_config HAS "entity_schemas" AND stream_config.entity_schemas IS NOT EMPTY THEN
        FOR EACH schema IN stream_config.entity_schemas DO
            IF schema HAS "attributes" THEN
                FOR EACH attr IN schema.attributes DO
                    IF attr.name = field_name THEN
                        IF attr HAS "description" AND attr.description IS NOT EMPTY THEN
                            LOG DEBUG "Description found in entity_schemas for {field_name} (legacy fallback)"
                            RETURN Some(attr.description)
                        END IF
                    END IF
                END FOR
            END IF
        END FOR
    END IF

    // Not found in either location
    LOG DEBUG "No description found for field {field_name}"
    RETURN None
END

ALGORITHM: DictionaryLoader.get_field_metadata
INPUT: stream_config (StreamConfig), field_name (string)
OUTPUT: FieldMetadata

STRUCTURE FieldMetadata:
    name: string
    type: string
    unit: Option<string>
    description: Option<string>
    device_class: Option<string>
    range: Option<[number, number]>
    nullable: boolean

BEGIN
    metadata <- FieldMetadata(name: field_name)

    // Step 1: Get base field info from fields array
    field <- stream_config.fields.find(f => f.name = field_name)

    IF field IS NOT NULL THEN
        metadata.type <- field.type
        metadata.nullable <- field.nullable OR true  // default true

        // Copy attributes that exist on field
        IF field HAS "unit" THEN metadata.unit <- Some(field.unit) END IF
        IF field HAS "description" THEN metadata.description <- Some(field.description) END IF
        IF field HAS "device_class" THEN metadata.device_class <- Some(field.device_class) END IF
        IF field HAS "range" THEN metadata.range <- Some(field.range) END IF
    END IF

    // Step 2: Fill gaps from entity_schemas (v1.0 compatibility)
    entity_attr <- FindEntitySchemaAttribute(stream_config, field_name)

    IF entity_attr IS NOT NULL THEN
        // Only fill if not already set
        IF metadata.description IS None AND entity_attr HAS "description" THEN
            metadata.description <- Some(entity_attr.description)
        END IF
        IF metadata.device_class IS None AND entity_attr HAS "device_class" THEN
            metadata.device_class <- Some(entity_attr.device_class)
        END IF
        IF metadata.range IS None AND entity_attr HAS "range" THEN
            metadata.range <- Some(entity_attr.range)
        END IF
        IF metadata.unit IS None AND entity_attr HAS "unit" THEN
            metadata.unit <- Some(entity_attr.unit)
        END IF
    END IF

    RETURN metadata
END

ALGORITHM: FindEntitySchemaAttribute
INPUT: stream_config (StreamConfig), field_name (string)
OUTPUT: Option<EntityAttribute>

BEGIN
    IF NOT stream_config HAS "entity_schemas" THEN
        RETURN None
    END IF

    FOR EACH schema IN stream_config.entity_schemas DO
        IF schema HAS "attributes" THEN
            FOR EACH attr IN schema.attributes DO
                IF attr.name = field_name THEN
                    RETURN Some(attr)
                END IF
            END FOR
        END IF
    END FOR

    RETURN None
END
```

---

## State Transitions

### Config Sync State Machine

```
STATE MACHINE: ConfigSyncStates

STATES:
    IDLE            - No sync in progress
    READING         - Reading JSON file from disk
    VALIDATING      - Validating against JSON Schema
    SAVING          - Saving to etcd
    COMPLETED       - Sync succeeded
    FAILED          - Sync failed

TRANSITIONS:
    IDLE -> READING
        trigger: sync_stream() called
        action: read JSON file

    READING -> VALIDATING
        trigger: file read successfully
        action: validate against schema

    READING -> FAILED
        trigger: file read error
        action: log error, return

    VALIDATING -> SAVING
        trigger: validation passed
        action: save to etcd

    VALIDATING -> FAILED
        trigger: validation failed
        action: log errors, return

    SAVING -> COMPLETED
        trigger: etcd save succeeded
        action: log success

    SAVING -> FAILED
        trigger: etcd save failed
        action: log error, return

STATE DIAGRAM:

    +--------+
    |  IDLE  |
    +---+----+
        |
        | sync_stream()
        v
    +--------+     error      +--------+
    |READING +--------------->| FAILED |
    +---+----+                +--------+
        |                          ^
        | success                  |
        v                          |
    +----------+   invalid    -----+
    |VALIDATING+------------------+
    +---+------+                  |
        |                         |
        | valid                   |
        v                         |
    +--------+     error     -----+
    | SAVING +-------------------+
    +---+----+
        |
        | success
        v
    +----------+
    |COMPLETED |
    +----------+
```

### Config Loading State Machine (StreamRegistry)

```
STATE MACHINE: ConfigLoadingStates

STATES:
    INITIAL         - No config loaded
    CACHED          - Config in local cache
    LOADING         - Fetching from etcd
    LOADED          - Config loaded and validated
    NOT_FOUND       - Config does not exist
    ERROR           - Transient error

TRANSITIONS:
    INITIAL -> LOADING
        trigger: first request for stream config
        action: initiate etcd fetch

    LOADING -> LOADED
        trigger: etcd returns valid config
        action: cache config, return

    LOADING -> NOT_FOUND
        trigger: etcd returns empty response
        action: return NotFound error

    LOADING -> ERROR
        trigger: connection failure
        action: return error, schedule retry

    CACHED -> LOADED
        trigger: cache hit (not expired)
        action: return cached config

    CACHED -> LOADING
        trigger: cache TTL expired
        action: refresh from etcd
```

---

## Decision Trees

### Field Description Source Decision Tree

```
DECISION TREE: GetFieldDescriptionSource

START: Need description for field_name in stream_config

Q1: Does fields[] array exist and have entries?
    NO  -> Go to Q3 (entity_schemas fallback)
    YES -> Q2

Q2: Does field with matching name exist in fields[]?
    NO  -> Go to Q3
    YES -> Q2a

Q2a: Does field have non-empty description attribute?
    NO  -> Go to Q3 (try entity_schemas fallback)
    YES -> Return fields[].description (PREFERRED SOURCE)

Q3: Does entity_schemas[] exist?
    NO  -> Return None (NO DESCRIPTION AVAILABLE)
    YES -> Q4

Q4: Does any schema.attributes[] have matching field name?
    NO  -> Return None
    YES -> Q4a

Q4a: Does matching attribute have non-empty description?
    NO  -> Return None
    YES -> Return entity_schemas.attributes[].description (LEGACY FALLBACK)
```

---

## Error Handling Patterns

### Config Sync Error Handling

```
PATTERN: ConfigSyncErrorHandling

ERROR_TYPES:
    1. FileNotFound      - JSON file does not exist
    2. ParseError        - JSON is malformed
    3. ValidationError   - Schema validation failed
    4. ConnectionError   - Cannot reach etcd
    5. SaveError         - etcd rejected the save

HANDLING_STRATEGY:

    FileNotFound:
        - Log WARN (stream may not be configured yet)
        - Skip stream, continue with others
        - Do not retry

    ParseError:
        - Log ERROR with file path and parse error
        - Do not retry (permanent until file fixed)
        - Alert operations

    ValidationError:
        - Log ERROR with validation failures
        - Do not retry (permanent until file fixed)
        - Alert operations

    ConnectionError:
        - Log ERROR with connection details
        - Retry with exponential backoff
        - After N failures, promote to ERROR
        - Alert operations

    SaveError:
        - Log ERROR with etcd error
        - Retry with exponential backoff
        - After N failures, alert operations
```

### Sync Error Promotion (WARN -> ERROR)

```
ALGORITHM: SyncConfigWithErrorPromotion
INPUT: json_path (string)
OUTPUT: SyncResult

CONSTANTS:
    MAX_RETRIES = 3
    RETRY_DELAY_MS = 1000

BEGIN
    retry_count <- 0
    last_error <- null

    WHILE retry_count < MAX_RETRIES DO
        TRY
            result <- sync_stream(json_path)

            IF result.success THEN
                LOG INFO "[sync] Config sync succeeded for {json_path}"
                RETURN SyncResult::Success
            END IF

        CATCH error
            retry_count <- retry_count + 1
            last_error <- error

            IF retry_count < MAX_RETRIES THEN
                LOG WARN "[sync] Config sync attempt {retry_count}/{MAX_RETRIES} failed: {error}"
                sleep(RETRY_DELAY_MS * retry_count)
            END IF
        END TRY
    END WHILE

    // All retries exhausted - promote to ERROR
    LOG ERROR "[sync] Config sync FAILED for {json_path} error=\"{last_error}\" attempts={MAX_RETRIES}"

    RETURN SyncResult::Failed(last_error)
END
```

---

## Complexity Analysis

### Migration Script Complexity

```
ALGORITHM: MigrateYamlToJson

TIME COMPLEXITY:
    - Directory scan: O(s) where s = number of streams
    - YAML parsing: O(c) where c = config size per stream
    - Field enrichment: O(f * a) where f = fields, a = attributes in entity_schemas
    - JSON validation: O(c)
    - File write: O(c)

    Total: O(s * (c + f * a))

    With typical values:
    - s = 7 streams
    - c = 500 lines average config
    - f = 10 fields average
    - a = 10 attributes average

    Total: O(7 * (500 + 100)) = O(4200) = constant time for typical deployment

SPACE COMPLEXITY:
    - Config in memory: O(c) per stream
    - Entity schema lookup map: O(a)
    - Validation errors: O(e) where e = error count

    Total: O(c + a + e) per stream
    Peak: O(c + a + e) since processed sequentially
```

### Config Sync Complexity

```
ALGORITHM: ConfigSyncService.sync_stream

TIME COMPLEXITY:
    - File read: O(c) where c = config size
    - Schema validation: O(c + f + r) where f = fields, r = rules
    - JSON parsing: O(c)
    - Network round-trip to etcd: O(n) where n = network latency (~10-50ms)

    Total: O(c + f + r + n)
    Dominated by network latency in practice

SPACE COMPLEXITY:
    - JSON content: O(c)
    - Parsed config: O(c)
    - Validation errors: O(e)

    Total: O(c + e)
```

### Config Loading Complexity (StreamRegistry)

```
ALGORITHM: StreamRegistry.load_stream

TIME COMPLEXITY:
    - Cache lookup: O(1) hash map lookup
    - Network round-trip: O(n) where n = network latency
    - JSON parsing: O(c) where c = config size

    Cold load: O(n + c)
    Cached load: O(1)

SPACE COMPLEXITY:
    - Single config: O(c)
    - Cache: O(s * c) where s = cached stream count
    - Typical: With 7 streams at 2KB each = 14KB cache footprint
```

### Dictionary Loader Complexity

```
ALGORITHM: DictionaryLoader.get_field_description

TIME COMPLEXITY:
    - Fields array scan: O(f) where f = field count
    - Entity schemas scan: O(e * a) where e = schemas, a = attributes per schema

    Worst case: O(f + e * a) when field not found
    Best case: O(1) when first field matches

SPACE COMPLEXITY:
    - O(1) - no additional space beyond input config
```

---

## Summary: Pass-Through vs. Transformation

### What We Removed

| Component | Reason |
|-----------|--------|
| `ConfigLoader` trait | Not needed - StreamRegistry already provides this |
| `EtcdConfigLoader` | Not needed - StreamRegistry already does this |
| `SilverRegistry` | Not needed - Silver uses same StreamRegistry as Bronze |
| `StreamConfigYaml` | Not needed - one struct (StreamConfig) for everything |
| `to_stream_config()` | This was the source of data loss |

### What We Keep

| Component | Purpose |
|-----------|---------|
| JSON migration script | One-time conversion from YAML to JSON |
| Field enrichment | Moves descriptions from entity_schemas to fields |
| Schema validation | Validates JSON before sync |
| ConfigSyncService | Simplified to pass-through (no transformation) |
| StreamRegistry | Existing component - unchanged |
| Dictionary loader fallback | v1.0 compatibility for entity_schemas |

### The Key Insight

**BEFORE** (lossy):
```
YAML -> StreamConfigYaml -> to_stream_config() -> StreamConfig -> etcd
                                  ^
                                  |
                            DATA LOST HERE
```

**AFTER** (pass-through):
```
JSON -> validate -> StreamConfig -> etcd
          |              |
          v              v
      (same data)   (same data)
```

---

## References

- [dp-018 SCOPE.md](../SCOPE.md) - Feature scope and acceptance criteria
- [ADR-018-001](../architecture/ADR-018-001-config-loader-design.md) - JSON Pass-Through Architecture
- [dp-016 ADR-016-001](../../dp-016/architecture/ADR-016-001-config-source-of-truth.md) - Config source of truth
- [config-client](../../../../config-client/src/) - Existing etcd client implementation
- [core/types/stream_config.rs](../../../../core/src/types/stream_config.rs) - StreamConfig struct

---

*Pseudocode created: 2026-02-01*
*Updated: 2026-02-01 - Aligned with JSON pass-through architecture (ADR-018-001)*
*SPARC Phase: Pseudocode*
*Parent Feature: dp-018 JSON Config Foundation*
