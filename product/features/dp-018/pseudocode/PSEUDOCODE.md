# dp-018: JSON Config Foundation - Pseudocode Design

## Document Overview

This document provides the algorithmic design for dp-018's two phases:
- **Phase 0**: JSON Migration (YAML to JSON with field enrichment)
- **Phase 1**: Unified Config Loading (ConfigLoader trait, EtcdConfigLoader)

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

    // Copy silver_etl config
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

## Phase 1: Unified Config Loading

### 1.1 ConfigLoader Trait Definition

```
TRAIT: ConfigLoader

DESCRIPTION:
    Unified interface for loading configuration from any source.
    Implementations must be thread-safe (Send + Sync).

METHODS:
    async load_stream_config(stream_id: string) -> Result<StreamConfig, ConfigError>
        // Load complete stream configuration including all sections

    async load_silver_etl_config(stream_id: string) -> Result<SilverEtlConfig, ConfigError>
        // Load only the silver_etl section for ETL processing

    async list_streams() -> Result<Array<string>, ConfigError>
        // List all available stream IDs

ERRORS:
    ConfigError::NotFound(stream_id)        // Stream config does not exist
    ConfigError::ParseError(message)        // JSON parsing failed
    ConfigError::ValidationError(message)   // Config failed validation
    ConfigError::ConnectionError(message)   // Connection to config source failed
```

### 1.2 EtcdConfigLoader Implementation

```
ALGORITHM: EtcdConfigLoader.load_stream_config
INPUT: stream_id (string)
OUTPUT: Result<StreamConfig, ConfigError>

CONSTANTS:
    KEY_PREFIX = "/streams"
    CACHE_TTL = 300 seconds

BEGIN
    // Step 1: Check cache
    cache_key <- stream_id
    IF cache.has(cache_key) AND NOT cache.is_expired(cache_key) THEN
        LOG DEBUG "Config loaded from cache for stream={stream_id}"
        RETURN Ok(cache.get(cache_key))
    END IF

    // Step 2: Build etcd key
    etcd_key <- KEY_PREFIX + "/" + stream_id + "/config"

    // Step 3: Fetch from etcd
    TRY
        response <- etcd_client.get(etcd_key)
    CATCH connection_error
        LOG ERROR "etcd connection failed: {connection_error}"
        RETURN Err(ConfigError::ConnectionError(connection_error.message))
    END TRY

    // Step 4: Handle not found
    IF response.kvs IS EMPTY THEN
        LOG WARN "Config not found for stream={stream_id}"
        RETURN Err(ConfigError::NotFound(stream_id))
    END IF

    // Step 5: Parse JSON
    json_blob <- response.kvs[0].value
    TRY
        config <- serde_json::from_slice<StreamConfig>(json_blob)
    CATCH parse_error
        LOG ERROR "JSON parse error for stream={stream_id}: {parse_error}"
        RETURN Err(ConfigError::ParseError(parse_error.message))
    END TRY

    // Step 6: Validate config
    validation_result <- config.validate()
    IF validation_result IS Err THEN
        LOG ERROR "Config validation failed for stream={stream_id}: {validation_result.error}"
        RETURN Err(ConfigError::ValidationError(validation_result.error.message))
    END IF

    // Step 7: Update cache
    cache.set(cache_key, config, ttl: CACHE_TTL)

    // Step 8: Log success
    LOG INFO "Config loaded from etcd for stream={stream_id} config_version={config.config_version}"

    RETURN Ok(config)
END
```

### 1.3 Load Silver ETL Config Algorithm

```
ALGORITHM: EtcdConfigLoader.load_silver_etl_config
INPUT: stream_id (string)
OUTPUT: Result<SilverEtlConfig, ConfigError>

BEGIN
    // Step 1: Load full stream config
    stream_config_result <- load_stream_config(stream_id)

    IF stream_config_result IS Err THEN
        RETURN stream_config_result.propagate_error()
    END IF

    stream_config <- stream_config_result.unwrap()

    // Step 2: Extract silver_etl section
    IF NOT stream_config HAS "silver_etl" OR stream_config.silver_etl IS NULL THEN
        LOG WARN "No silver_etl config for stream={stream_id}"
        RETURN Err(ConfigError::NotFound(
            "silver_etl section not found for stream: " + stream_id
        ))
    END IF

    silver_etl_config <- stream_config.silver_etl

    // Step 3: Check if enabled
    IF NOT silver_etl_config.enabled THEN
        LOG INFO "Silver ETL disabled for stream={stream_id}"
        RETURN Err(ConfigError::NotFound(
            "silver_etl is disabled for stream: " + stream_id
        ))
    END IF

    // Step 4: Validate silver_etl config
    validation_result <- silver_etl_config.validate()
    IF validation_result IS Err THEN
        LOG ERROR "Silver ETL config invalid for stream={stream_id}: {validation_result.error}"
        RETURN Err(ConfigError::ValidationError(validation_result.error.message))
    END IF

    LOG INFO "Silver ETL config loaded from etcd for stream={stream_id} target_table={silver_etl_config.target_table}"

    RETURN Ok(silver_etl_config)
END
```

### 1.4 Silver Subscriber Config Loading (Event-Driven)

```
ALGORITHM: SilverSubscriber.on_bronze_event
INPUT: event (BronzeEvent)
OUTPUT: ProcessingResult

STATE:
    config_cache: Map<stream_id, CachedConfig>
    config_loader: ConfigLoader (injected)

STRUCTURE CachedConfig:
    config: SilverEtlConfig
    cached_at: timestamp
    ttl: duration

BEGIN
    stream_id <- event.stream_id

    // Step 1: Check config cache
    IF config_cache.has(stream_id) THEN
        cached <- config_cache.get(stream_id)

        IF NOT cached.is_expired() THEN
            // Use cached config
            config <- cached.config
        ELSE
            // Reload expired config
            config_result <- reload_config(stream_id)
            IF config_result IS Err THEN
                RETURN handle_config_error(config_result.error, event)
            END IF
            config <- config_result.unwrap()
        END IF
    ELSE
        // First time seeing this stream - load config
        config_result <- load_and_cache_config(stream_id)
        IF config_result IS Err THEN
            RETURN handle_config_error(config_result.error, event)
        END IF
        config <- config_result.unwrap()
    END IF

    // Step 2: Process event with config
    RETURN process_with_config(event, config)
END

ALGORITHM: SilverSubscriber.load_and_cache_config
INPUT: stream_id (string)
OUTPUT: Result<SilverEtlConfig, ConfigError>

BEGIN
    LOG DEBUG "Loading config for new stream={stream_id}"

    // Load from config loader (etcd)
    config_result <- config_loader.load_silver_etl_config(stream_id)

    IF config_result IS Err THEN
        LOG ERROR "Failed to load config for stream={stream_id}: {config_result.error}"
        RETURN config_result
    END IF

    config <- config_result.unwrap()

    // Cache the config
    config_cache.set(stream_id, CachedConfig(
        config: config,
        cached_at: now(),
        ttl: 300 seconds
    ))

    LOG INFO "Config cached for stream={stream_id} target_table={config.target_table}"

    RETURN Ok(config)
END

ALGORITHM: SilverSubscriber.handle_config_error
INPUT: error (ConfigError), event (BronzeEvent)
OUTPUT: ProcessingResult

BEGIN
    MATCH error:
        ConfigError::NotFound(stream_id):
            // Stream not configured for Silver ETL - this is normal
            LOG DEBUG "Stream {stream_id} not configured for Silver ETL, skipping"
            RETURN ProcessingResult::Skipped

        ConfigError::ValidationError(message):
            // Config exists but is invalid - this is an error
            LOG ERROR "Invalid config for stream {event.stream_id}: {message}"
            RETURN ProcessingResult::Error(error)

        ConfigError::ConnectionError(message):
            // Transient error - should retry
            LOG WARN "Config fetch failed for stream {event.stream_id}, will retry: {message}"
            RETURN ProcessingResult::RetryLater

        ConfigError::ParseError(message):
            // Permanent error - config is malformed
            LOG ERROR "Config parse error for stream {event.stream_id}: {message}"
            RETURN ProcessingResult::Error(error)
    END MATCH
END
```

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

### 1.6 Config Source Logging

```
ALGORITHM: LogConfigSource

DESCRIPTION:
    Standardized logging format for config loading operations.
    Used by all config loaders for consistent audit trail.

LOG_FORMAT:
    "{level} {timestamp} [config] action={action} stream={stream_id} source={source} [details]"

ACTIONS:
    "loaded"     - Config successfully loaded
    "cached"     - Config retrieved from cache
    "not_found"  - Config does not exist
    "error"      - Error occurred during loading
    "validated"  - Config passed validation
    "invalid"    - Config failed validation

EXAMPLES:
    INFO  2026-02-01T10:30:00Z [config] action=loaded stream=air-quality source=etcd config_version=1.1
    DEBUG 2026-02-01T10:30:05Z [config] action=cached stream=air-quality cache_age_ms=5000
    WARN  2026-02-01T10:30:10Z [config] action=not_found stream=unknown-stream source=etcd
    ERROR 2026-02-01T10:30:15Z [config] action=error stream=air-quality source=etcd error="connection refused"

IMPLEMENTATION:

FUNCTION log_config_loaded(stream_id, source, config):
    LOG INFO "[config] action=loaded stream={stream_id} source={source} config_version={config.config_version}"

FUNCTION log_config_cached(stream_id, cache_age_ms):
    LOG DEBUG "[config] action=cached stream={stream_id} cache_age_ms={cache_age_ms}"

FUNCTION log_config_not_found(stream_id, source):
    LOG WARN "[config] action=not_found stream={stream_id} source={source}"

FUNCTION log_config_error(stream_id, source, error):
    LOG ERROR "[config] action=error stream={stream_id} source={source} error=\"{error}\""

FUNCTION log_config_validated(stream_id, duration_ms):
    LOG DEBUG "[config] action=validated stream={stream_id} duration_ms={duration_ms}"

FUNCTION log_config_invalid(stream_id, errors):
    LOG ERROR "[config] action=invalid stream={stream_id} errors={errors}"
```

### 1.7 Sync Error Promotion (WARN -> ERROR)

```
ALGORITHM: PromoteSyncErrors

DESCRIPTION:
    Promote synchronization errors from WARN to ERROR level.
    Makes failures visible in logs and monitoring systems.

BEFORE (Silent Failures):
    WARN  [sync] Failed to load stream config: timeout
    WARN  [sync] Skipping stream air-quality
    // Continues processing other streams, no indication of problem

AFTER (Visible Failures):
    ERROR [sync] Config sync failed for stream=air-quality error="timeout" attempt=3/3
    ERROR [sync] Stream air-quality will not receive updates until sync succeeds
    // Alerts fire, operators notified

IMPLEMENTATION:

ALGORITHM: SyncConfigWithErrorPromotion
INPUT: stream_id (string)
OUTPUT: SyncResult

CONSTANTS:
    MAX_RETRIES = 3
    RETRY_DELAY_MS = 1000

BEGIN
    retry_count <- 0
    last_error <- null

    WHILE retry_count < MAX_RETRIES DO
        TRY
            // Attempt to sync config
            result <- sync_config_to_etcd(stream_id)

            IF result.success THEN
                LOG INFO "[sync] Config sync succeeded for stream={stream_id}"
                RETURN SyncResult::Success
            END IF

        CATCH error
            retry_count <- retry_count + 1
            last_error <- error

            IF retry_count < MAX_RETRIES THEN
                LOG WARN "[sync] Config sync attempt {retry_count}/{MAX_RETRIES} failed for stream={stream_id}: {error}"
                sleep(RETRY_DELAY_MS * retry_count)  // Exponential backoff
            END IF
        END TRY
    END WHILE

    // All retries exhausted - promote to ERROR
    LOG ERROR "[sync] Config sync FAILED for stream={stream_id} error=\"{last_error}\" attempts={MAX_RETRIES}"
    LOG ERROR "[sync] Stream {stream_id} will not receive updates until sync succeeds"

    // Optionally increment error metric
    metrics.increment("config_sync_failures", tags: { stream: stream_id })

    RETURN SyncResult::Failed(last_error)
END
```

---

## State Transitions

### Config Loading State Machine

```
STATE MACHINE: ConfigLoadingStates

STATES:
    INITIAL         - No config loaded, fresh start
    CACHED          - Config available in local cache
    LOADING         - Fetching config from etcd
    LOADED          - Config successfully loaded and validated
    NOT_FOUND       - Config does not exist for stream
    INVALID         - Config exists but failed validation
    ERROR           - Transient error (connection, timeout)

TRANSITIONS:
    INITIAL -> LOADING
        trigger: first request for stream config
        action: initiate etcd fetch

    LOADING -> LOADED
        trigger: etcd returns valid config
        action: cache config, log success

    LOADING -> NOT_FOUND
        trigger: etcd returns empty response
        action: log warning

    LOADING -> INVALID
        trigger: config fails validation
        action: log error, do not cache

    LOADING -> ERROR
        trigger: connection failure, timeout
        action: log error, schedule retry

    CACHED -> LOADING
        trigger: cache TTL expired
        action: refresh from etcd

    CACHED -> LOADED
        trigger: cache hit (not expired)
        action: return cached config

    ERROR -> LOADING
        trigger: retry timer fires
        action: retry etcd fetch

    NOT_FOUND -> LOADING
        trigger: periodic check or explicit refresh
        action: check if config now exists

STATE DIAGRAM:

    +----------+
    | INITIAL  |
    +----+-----+
         |
         | first request
         v
    +----------+     success     +----------+
    | LOADING  +---------------->|  LOADED  |
    +----+-----+                 +----+-----+
         |                            ^
         | cache expired              | cache hit
         |                            |
         |    +----------+            |
         +--->|  CACHED  +------------+
         |    +----------+
         |
         | not found
         v
    +----------+
    |NOT_FOUND |
    +----------+
         |
         | error
         v
    +----------+
    |  ERROR   |----> retry ----> LOADING
    +----------+
         |
         | validation failed
         v
    +----------+
    | INVALID  |
    +----------+
```

---

## Decision Trees

### Should Load Config Decision Tree

```
DECISION TREE: ShouldLoadConfig

START: Received request for stream config

Q1: Is stream_id in local cache?
    NO  -> Load from etcd
    YES -> Q2

Q2: Is cached config expired (TTL > threshold)?
    NO  -> Return cached config (CACHE HIT)
    YES -> Q3

Q3: Is etcd connection healthy?
    NO  -> Return stale cache with warning (STALE CACHE)
    YES -> Load from etcd

LOAD FROM ETCD:
    Q4: Does key exist in etcd?
        NO  -> Return NOT_FOUND error
        YES -> Q5

    Q5: Is JSON valid?
        NO  -> Return PARSE_ERROR
        YES -> Q6

    Q6: Does config pass validation?
        NO  -> Return VALIDATION_ERROR
        YES -> Cache and return config (SUCCESS)
```

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

### Config Loading Error Handling

```
PATTERN: ConfigLoadingErrorHandling

ERROR_TYPES:
    1. ConnectionError   - Cannot reach etcd
    2. NotFoundError     - Key does not exist
    3. ParseError        - JSON is malformed
    4. ValidationError   - Config structure invalid
    5. TimeoutError      - Request took too long

HANDLING_STRATEGY:

    ConnectionError:
        - Log ERROR with connection details
        - Return stale cache if available
        - Schedule retry with exponential backoff
        - Increment connection_failure metric
        - After N failures, alert operations

    NotFoundError:
        - Log WARN (this may be expected for new streams)
        - Return specific NotFound error
        - Do not retry (permanent until config created)

    ParseError:
        - Log ERROR with stream_id and parse error details
        - Do not cache (permanent error)
        - Increment parse_error metric
        - Alert operations (config is corrupt)

    ValidationError:
        - Log ERROR with validation failure details
        - Do not cache
        - Increment validation_failure metric
        - Alert operations (config needs fixing)

    TimeoutError:
        - Log WARN with timeout duration
        - Retry with shorter timeout or backoff
        - After N retries, return stale cache if available
        - Increment timeout metric
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

### Config Loading Complexity

```
ALGORITHM: EtcdConfigLoader.load_stream_config

TIME COMPLEXITY:
    - Cache lookup: O(1) hash map lookup
    - Network round-trip: O(n) where n = network latency (~10-50ms typically)
    - JSON parsing: O(c) where c = config size
    - Validation: O(c + f + r) where f = fields, r = DQ rules
    - Cache update: O(1)

    Cold load: O(n + c + f + r)
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

## Optimization Notes

### Migration Performance

1. **Parallel Migration**: Streams could be migrated in parallel since they are independent
   - Current: Sequential for simplicity and error visibility
   - Future: Add `--parallel` flag for large deployments

2. **Incremental Migration**: Check for existing JSON files and skip if already v1.1+
   - Implemented via idempotency check
   - Enables safe re-runs

### Config Loading Performance

1. **Cache Warming**: Pre-load configs for known streams at startup
   - Reduces first-request latency
   - Trade-off: Startup time vs. request latency

2. **Batch Loading**: Load all stream configs in single etcd range query
   - More efficient for "list all streams" operations
   - Already implemented in StreamRegistry.load_all_streams()

3. **Watch for Changes**: Use etcd watch to invalidate cache on config changes
   - Enables near-real-time config updates
   - Future enhancement for hot-reload (Phase 4)

### Dictionary Loader Performance

1. **Pre-build Lookup Map**: Convert entity_schemas to hash map at config load time
   - O(1) field lookup instead of O(e * a) scan
   - Trade-off: Memory for speed

2. **Memoize Results**: Cache get_field_description results
   - Useful when repeatedly accessing same field
   - Clear on config reload

---

## References

- [dp-018 SCOPE.md](../SCOPE.md) - Feature scope and acceptance criteria
- [dp-016 IMPLEMENTATION-ROADMAP.md](../../dp-016/IMPLEMENTATION-ROADMAP.md) - Parent architecture roadmap
- [ADR-016-001](../../dp-016/architecture/ADR-016-001-config-source-of-truth.md) - Config source of truth decision
- [config-client](../../../../config-client/src/) - Existing etcd client implementation
- [core/config/silver_etl.rs](../../../../core/src/config/silver_etl.rs) - Silver ETL config types

---

*Pseudocode created: 2026-02-01*
*SPARC Phase: Pseudocode*
*Parent Feature: dp-018 JSON Config Foundation*
