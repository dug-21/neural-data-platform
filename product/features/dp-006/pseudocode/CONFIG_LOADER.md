# CONFIG LOADER - Pseudocode Specification

**Module**: `apps/silver-etl/src/config.rs`
**Feature**: dp-006 (Silver Layer Implementation)
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Pseudocode Specialist

---

## 1. Overview

The CONFIG LOADER module is responsible for loading, validating, and watching Silver ETL configurations from etcd or YAML fallback. It provides typed `SilverEtlConfig` structures to the ETL engine and supports hot-reload for configuration changes without service restart.

### Module Responsibilities

| Responsibility | Description |
|----------------|-------------|
| Load configs | Retrieve configs from etcd or YAML fallback |
| Validate | Enforce schema constraints and cross-field validation |
| Cache | Maintain in-memory cache of validated configs |
| Watch | Monitor etcd for changes and trigger callbacks |
| Provide | Supply typed configs to ETL engine |

---

## 2. Data Structures

### 2.1 Core Types

```
STRUCT SilverConfigLoader:
    etcd_client: ConfigClient           // From config-client crate
    yaml_fallback_path: String          // e.g., "/workspaces/neural-data-platform/config/base/streams"
    cache: RwLock<HashMap<StreamId, CachedConfig>>
    watchers: Vec<WatchHandle>

STRUCT CachedConfig:
    config: SilverEtlConfig
    loaded_at: Timestamp
    source: ConfigSource
    version: u64                        // etcd revision for staleness detection

ENUM ConfigSource:
    Etcd { key: String, revision: u64 }
    Yaml { path: PathBuf, modified: Timestamp }

TYPE StreamId = String                  // e.g., "air-quality", "outdoor-weather"
```

### 2.2 Configuration Schema Types

```
STRUCT SilverEtlConfig:
    enabled: bool
    target_table: String                // Must be qualified: schema.table
    target_schema: Option<String>       // Schema version reference
    timestamp: TimestampConfig
    identity_fields: Vec<IdentityField>
    field_mappings: Vec<FieldMapping>
    dq_output: DqOutputConfig
    deduplication: DeduplicationConfig
    incremental: IncrementalConfig

STRUCT TimestampConfig:
    source_field: String                // Bronze column name
    target_field: String                // Silver column name
    transform: TimestampTransform       // Conversion type

ENUM TimestampTransform:
    MicrosecondsToTimestamp            // Bronze default (i64 microseconds)
    Iso8601                            // ISO8601 string parse
    UnixSeconds                        // Unix epoch seconds
    NwsDuration                        // NWS forecast duration string

STRUCT IdentityField:
    source: String                     // JSON path in Bronze
    target: String                     // Column name in Silver

STRUCT FieldMapping:
    source_path: String                // JSON path in raw_payload
    target_column: String              // Silver column name
    column_type: String                // PostgreSQL type
    nullable: bool
    transform: Option<Transform>
    dq_rules: Vec<DqRule>

STRUCT Transform:
    type: TransformType
    params: TransformParams            // Type-specific parameters

ENUM TransformType:
    UnitConversion
    Expression
    Lookup
    JsonExtract
    Timestamp
    Computed

STRUCT DqRule:
    rule_type: DqRuleType
    params: DqRuleParams
    action: DqAction

ENUM DqRuleType:
    RangeCheck                         // min, max bounds
    NotNull                            // NULL rejection
    Pattern                            // Regex match
    OneOf                              // Enumeration
    Custom                             // SQL expression

ENUM DqAction:
    Flag                               // Keep value, record violation
    Reject                             // Set NULL, record violation
    Clamp                              // Clamp to bounds, record
    Drop                               // Drop entire row

STRUCT DqOutputConfig:
    enabled: bool
    target_column: String              // Default: "dq_flags"

STRUCT DeduplicationConfig:
    enabled: bool
    key_columns: Vec<String>
    strategy: DeduplicationStrategy

ENUM DeduplicationStrategy:
    Upsert                             // INSERT ... ON CONFLICT UPDATE
    Skip                               // INSERT ... ON CONFLICT DO NOTHING
    Replace                            // DELETE + INSERT

STRUCT IncrementalConfig:
    enabled: bool
    watermark_column: String
    lag_interval: String               // e.g., "5 minutes"
```

### 2.3 Error Types

```
ENUM ConfigLoaderError:
    EtcdConnectionFailed { endpoint: String, cause: String }
    ConfigNotFound { stream_id: String }
    InvalidYaml { path: PathBuf, cause: String }
    ValidationFailed { stream_id: String, errors: Vec<ValidationError> }
    WatchFailed { cause: String }
    CacheError { cause: String }

STRUCT ValidationError:
    field_path: String                 // e.g., "field_mappings[0].source_path"
    message: String
    severity: ValidationSeverity

ENUM ValidationSeverity:
    Error                              // Blocks ETL
    Warning                            // Logged, ETL continues
```

---

## 3. Function Specifications

### 3.1 Constructor: `new`

```
ALGORITHM: SilverConfigLoader::new
INPUT:
    etcd_endpoints: Vec<String>        // e.g., ["http://localhost:2379"]
    yaml_fallback_path: Option<String> // Optional YAML directory
OUTPUT:
    Result<SilverConfigLoader, ConfigLoaderError>

BEGIN
    // Step 1: Attempt etcd connection
    TRY
        etcd_client <- ConfigClient::with_prefix(etcd_endpoints, "/streams")
        LOG_INFO("Connected to etcd at {:?}", etcd_endpoints)
    CATCH connection_error
        IF yaml_fallback_path IS None THEN
            RETURN Err(ConfigLoaderError::EtcdConnectionFailed {
                endpoint: etcd_endpoints.join(","),
                cause: connection_error.to_string()
            })
        END IF
        LOG_WARN("etcd unavailable, using YAML fallback only")
        etcd_client <- None
    END TRY

    // Step 2: Validate fallback path if provided
    IF yaml_fallback_path IS Some(path) THEN
        IF NOT path.exists() OR NOT path.is_directory() THEN
            RETURN Err(ConfigLoaderError::InvalidYaml {
                path: path,
                cause: "Fallback path does not exist or is not a directory"
            })
        END IF
    END IF

    // Step 3: Initialize empty cache
    cache <- RwLock::new(HashMap::new())
    watchers <- Vec::new()

    RETURN Ok(SilverConfigLoader {
        etcd_client,
        yaml_fallback_path: yaml_fallback_path.unwrap_or_default(),
        cache,
        watchers
    })
END
```

**Complexity Analysis**:
- Time: O(1) for initialization, O(network) for etcd connection
- Space: O(1) - only struct allocation

---

### 3.2 Load All Silver Configs

```
ALGORITHM: load_all_silver_configs
INPUT:
    self: &SilverConfigLoader
OUTPUT:
    Result<Vec<SilverEtlConfig>, ConfigLoaderError>

BEGIN
    configs <- Vec::new()
    errors <- Vec::new()

    // Step 1: Discover all stream IDs
    stream_ids <- discover_stream_ids(self)
    LOG_INFO("Discovered {} streams", stream_ids.len())

    // Step 2: Load each stream's silver_etl config
    FOR EACH stream_id IN stream_ids DO
        MATCH load_stream_config(self, stream_id.clone())
            Ok(config) =>
                IF config.enabled THEN
                    configs.push(config)
                    LOG_DEBUG("Loaded silver_etl for stream: {}", stream_id)
                ELSE
                    LOG_DEBUG("Skipped disabled stream: {}", stream_id)
                END IF
            Err(ConfigLoaderError::ConfigNotFound { .. }) =>
                // Stream exists but has no silver_etl section - not an error
                LOG_DEBUG("No silver_etl section for stream: {}", stream_id)
            Err(e) =>
                errors.push((stream_id.clone(), e))
                LOG_WARN("Failed to load config for {}: {:?}", stream_id, e)
        END MATCH
    END FOR

    // Step 3: Report errors but don't fail completely
    IF NOT errors.is_empty() THEN
        LOG_WARN("Failed to load {} configs: {:?}", errors.len(), errors)
    END IF

    IF configs.is_empty() AND NOT errors.is_empty() THEN
        // All configs failed - return first error
        RETURN Err(errors[0].1)
    END IF

    LOG_INFO("Loaded {} enabled Silver ETL configs", configs.len())
    RETURN Ok(configs)
END

SUBROUTINE: discover_stream_ids
INPUT:
    self: &SilverConfigLoader
OUTPUT:
    Vec<StreamId>

BEGIN
    stream_ids <- HashSet::new()

    // Try etcd first
    IF self.etcd_client IS Some(client) THEN
        TRY
            keys <- client.list("/")
            FOR EACH key IN keys DO
                // Extract stream_id from key like "/streams/air-quality/config"
                IF key.ends_with("/config") THEN
                    parts <- key.split("/")
                    stream_id <- parts[2]  // /streams/{stream_id}/config
                    stream_ids.insert(stream_id)
                END IF
            END FOR
        CATCH e
            LOG_WARN("etcd list failed: {}", e)
        END TRY
    END IF

    // Supplement with YAML fallback directory
    IF NOT self.yaml_fallback_path.is_empty() THEN
        FOR EACH entry IN read_dir(self.yaml_fallback_path) DO
            IF entry.is_directory() THEN
                config_path <- entry.path().join("config.yaml")
                IF config_path.exists() THEN
                    stream_ids.insert(entry.file_name())
                END IF
            END IF
        END FOR
    END IF

    RETURN stream_ids.into_iter().collect()
END
```

**Complexity Analysis**:
- Time: O(n * m) where n = stream count, m = config load time per stream
- Space: O(n) for storing all configs

---

### 3.3 Load Single Stream Config

```
ALGORITHM: load_stream_config
INPUT:
    self: &SilverConfigLoader
    stream_id: String
OUTPUT:
    Result<SilverEtlConfig, ConfigLoaderError>

BEGIN
    // Step 1: Check cache first
    cache_read <- self.cache.read()
    IF cache_read.contains_key(stream_id) THEN
        cached <- cache_read.get(stream_id)
        // Check if cache is still valid (5 minute TTL)
        IF (now() - cached.loaded_at) < Duration::from_secs(300) THEN
            LOG_DEBUG("Cache hit for stream: {}", stream_id)
            RETURN Ok(cached.config.clone())
        END IF
    END IF
    DROP cache_read

    // Step 2: Try etcd source
    silver_config <- None
    config_source <- None

    IF self.etcd_client IS Some(client) THEN
        etcd_key <- format!("/{}/config", stream_id)
        TRY
            full_config: StreamConfig <- client.get(etcd_key)

            IF full_config.silver_etl IS Some(silver_etl) THEN
                silver_config <- Some(silver_etl)
                config_source <- Some(ConfigSource::Etcd {
                    key: etcd_key,
                    revision: client.last_revision()
                })
                LOG_DEBUG("Loaded {} from etcd", stream_id)
            END IF
        CATCH ConfigError::NotFound(_)
            LOG_DEBUG("Stream {} not in etcd, trying YAML", stream_id)
        CATCH e
            LOG_WARN("etcd error for {}: {}", stream_id, e)
        END TRY
    END IF

    // Step 3: Fall back to YAML if not found in etcd
    IF silver_config IS None AND NOT self.yaml_fallback_path.is_empty() THEN
        yaml_path <- Path::new(&self.yaml_fallback_path)
                        .join(&stream_id)
                        .join("config.yaml")

        IF yaml_path.exists() THEN
            TRY
                yaml_content <- read_file(yaml_path)
                full_config: StreamConfig <- serde_yaml::from_str(yaml_content)

                IF full_config.silver_etl IS Some(silver_etl) THEN
                    silver_config <- Some(silver_etl)
                    config_source <- Some(ConfigSource::Yaml {
                        path: yaml_path.clone(),
                        modified: yaml_path.metadata().modified()
                    })
                    LOG_DEBUG("Loaded {} from YAML: {:?}", stream_id, yaml_path)
                END IF
            CATCH e
                RETURN Err(ConfigLoaderError::InvalidYaml {
                    path: yaml_path,
                    cause: e.to_string()
                })
            END TRY
        END IF
    END IF

    // Step 4: Handle not found
    IF silver_config IS None THEN
        RETURN Err(ConfigLoaderError::ConfigNotFound { stream_id })
    END IF

    // Step 5: Validate the config
    config <- silver_config.unwrap()
    validate_silver_config(&config, &stream_id)?

    // Step 6: Update cache
    cache_write <- self.cache.write()
    cache_write.insert(stream_id.clone(), CachedConfig {
        config: config.clone(),
        loaded_at: now(),
        source: config_source.unwrap(),
        version: 0
    })
    DROP cache_write

    RETURN Ok(config)
END
```

**Complexity Analysis**:
- Time: O(1) cache hit, O(network + parse) cache miss
- Space: O(config_size) for the returned config

---

### 3.4 Validate Silver Config

```
ALGORITHM: validate_silver_config
INPUT:
    config: &SilverEtlConfig
    stream_id: &str
OUTPUT:
    Result<(), ConfigLoaderError>

BEGIN
    errors <- Vec::new()
    warnings <- Vec::new()

    // ========================================
    // RULE 1: target_table must be qualified
    // ========================================
    IF NOT config.target_table.contains(".") THEN
        errors.push(ValidationError {
            field_path: "target_table",
            message: format!(
                "target_table '{}' must be qualified (schema.table), e.g., 'silver.air_quality_observations'",
                config.target_table
            ),
            severity: Error
        })
    ELSE
        parts <- config.target_table.split(".")
        IF parts.len() != 2 THEN
            errors.push(ValidationError {
                field_path: "target_table",
                message: "target_table must have exactly one dot (schema.table)",
                severity: Error
            })
        END IF
        IF parts[0].is_empty() OR parts[1].is_empty() THEN
            errors.push(ValidationError {
                field_path: "target_table",
                message: "schema and table names cannot be empty",
                severity: Error
            })
        END IF
    END IF

    // ========================================
    // RULE 2: timestamp.source_field validation
    // ========================================
    IF config.timestamp.source_field.is_empty() THEN
        errors.push(ValidationError {
            field_path: "timestamp.source_field",
            message: "timestamp.source_field is required",
            severity: Error
        })
    END IF

    // Validate source_field is a valid Bronze column or path
    valid_bronze_columns <- ["timestamp", "ndp_id", "source_id", "context", "raw_payload"]
    base_field <- config.timestamp.source_field.split(".").next()
    IF NOT valid_bronze_columns.contains(base_field)
       AND NOT config.timestamp.source_field.starts_with("raw_payload.") THEN
        warnings.push(ValidationError {
            field_path: "timestamp.source_field",
            message: format!(
                "source_field '{}' may not exist in Bronze schema. Valid columns: {:?}",
                config.timestamp.source_field, valid_bronze_columns
            ),
            severity: Warning
        })
    END IF

    IF config.timestamp.target_field.is_empty() THEN
        errors.push(ValidationError {
            field_path: "timestamp.target_field",
            message: "timestamp.target_field is required",
            severity: Error
        })
    END IF

    // ========================================
    // RULE 3: source_path validation (JSON paths)
    // ========================================
    FOR (index, mapping) IN config.field_mappings.enumerate() DO
        field_path <- format!("field_mappings[{}]", index)

        // 3a: source_path must be valid JSON path
        IF mapping.source_path.is_empty() THEN
            errors.push(ValidationError {
                field_path: format!("{}.source_path", field_path),
                message: "source_path cannot be empty",
                severity: Error
            })
        ELSE IF NOT is_valid_json_path(&mapping.source_path) THEN
            errors.push(ValidationError {
                field_path: format!("{}.source_path", field_path),
                message: format!(
                    "source_path '{}' is not a valid JSON path. Expected format: raw_payload.field or raw_payload.nested.field",
                    mapping.source_path
                ),
                severity: Error
            })
        END IF

        // 3b: target_column must be valid SQL identifier
        IF mapping.target_column.is_empty() THEN
            errors.push(ValidationError {
                field_path: format!("{}.target_column", field_path),
                message: "target_column cannot be empty",
                severity: Error
            })
        ELSE IF NOT is_valid_sql_identifier(&mapping.target_column) THEN
            errors.push(ValidationError {
                field_path: format!("{}.target_column", field_path),
                message: format!(
                    "target_column '{}' is not a valid SQL identifier (use snake_case, no special chars)",
                    mapping.target_column
                ),
                severity: Error
            })
        END IF

        // 3c: column_type must be valid PostgreSQL type
        valid_types <- [
            "boolean", "smallint", "integer", "bigint",
            "real", "double precision", "float",
            "text", "varchar", "char",
            "timestamptz", "timestamp", "date", "time", "interval",
            "json", "jsonb", "text[]", "integer[]"
        ]
        IF NOT valid_types.contains(mapping.column_type.to_lowercase()) THEN
            warnings.push(ValidationError {
                field_path: format!("{}.column_type", field_path),
                message: format!(
                    "column_type '{}' may not be a standard PostgreSQL type",
                    mapping.column_type
                ),
                severity: Warning
            })
        END IF

        // 3d: Validate transform if present
        IF mapping.transform IS Some(transform) THEN
            validate_transform(&transform, &format!("{}.transform", field_path), &mut errors)
        END IF

        // 3e: Validate DQ rules
        FOR (rule_idx, rule) IN mapping.dq_rules.enumerate() DO
            validate_dq_rule(&rule, &format!("{}.dq_rules[{}]", field_path, rule_idx), &mut errors)
        END FOR
    END FOR

    // ========================================
    // RULE 4: DQ rules validation
    // ========================================
    // (Handled in field_mappings loop above)

    // ========================================
    // RULE 5: deduplication.key_columns validation
    // ========================================
    IF config.deduplication.enabled THEN
        IF config.deduplication.key_columns.is_empty() THEN
            errors.push(ValidationError {
                field_path: "deduplication.key_columns",
                message: "key_columns cannot be empty when deduplication is enabled",
                severity: Error
            })
        ELSE
            // Collect all output column names
            output_columns <- HashSet::new()
            output_columns.insert(config.timestamp.target_field.clone())
            FOR identity IN config.identity_fields DO
                output_columns.insert(identity.target.clone())
            END FOR
            FOR mapping IN config.field_mappings DO
                output_columns.insert(mapping.target_column.clone())
            END FOR

            // Verify key_columns are subset of output columns
            FOR key_col IN config.deduplication.key_columns DO
                IF NOT output_columns.contains(key_col) THEN
                    errors.push(ValidationError {
                        field_path: "deduplication.key_columns",
                        message: format!(
                            "key_column '{}' is not in output columns. Available: {:?}",
                            key_col, output_columns
                        ),
                        severity: Error
                    })
                END IF
            END FOR
        END IF
    END IF

    // ========================================
    // RULE 6: incremental.watermark_column validation
    // ========================================
    IF config.incremental.enabled THEN
        IF config.incremental.watermark_column.is_empty() THEN
            errors.push(ValidationError {
                field_path: "incremental.watermark_column",
                message: "watermark_column is required when incremental is enabled",
                severity: Error
            })
        END IF

        // Validate lag_interval format
        IF NOT config.incremental.lag_interval.is_empty() THEN
            IF NOT is_valid_interval(&config.incremental.lag_interval) THEN
                errors.push(ValidationError {
                    field_path: "incremental.lag_interval",
                    message: format!(
                        "lag_interval '{}' is not a valid interval. Examples: '5 minutes', '1 hour', '30 seconds'",
                        config.incremental.lag_interval
                    ),
                    severity: Error
                })
            END IF
        END IF
    END IF

    // ========================================
    // Log warnings
    // ========================================
    FOR warning IN warnings DO
        LOG_WARN("[{}] Validation warning at {}: {}",
                 stream_id, warning.field_path, warning.message)
    END FOR

    // ========================================
    // Return result
    // ========================================
    IF NOT errors.is_empty() THEN
        RETURN Err(ConfigLoaderError::ValidationFailed {
            stream_id: stream_id.to_string(),
            errors
        })
    END IF

    RETURN Ok(())
END

SUBROUTINE: is_valid_json_path
INPUT:
    path: &str
OUTPUT:
    bool
BEGIN
    // Valid paths: "raw_payload.field", "raw_payload.nested.field", "context.location.type"
    // Must start with recognized root: raw_payload, context, ndp_id, timestamp, source_id

    IF path.is_empty() THEN
        RETURN false
    END IF

    valid_roots <- ["raw_payload", "context", "ndp_id", "timestamp", "source_id"]
    root <- path.split(".").next()

    IF NOT valid_roots.contains(root) THEN
        RETURN false
    END IF

    // Check for valid characters (alphanumeric, underscore, dot, brackets for arrays)
    valid_pattern <- Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*|\[\d+\])*$")
    RETURN valid_pattern.is_match(path)
END

SUBROUTINE: is_valid_sql_identifier
INPUT:
    name: &str
OUTPUT:
    bool
BEGIN
    // Valid SQL identifier: starts with letter/underscore, contains only alphanumeric/underscore
    IF name.is_empty() OR name.len() > 63 THEN
        RETURN false
    END IF

    valid_pattern <- Regex::new(r"^[a-z_][a-z0-9_]*$")
    RETURN valid_pattern.is_match(name.to_lowercase())
END

SUBROUTINE: is_valid_interval
INPUT:
    interval: &str
OUTPUT:
    bool
BEGIN
    // Valid formats: "5 minutes", "1 hour", "30 seconds", "2 days"
    valid_pattern <- Regex::new(r"^\d+\s+(second|seconds|minute|minutes|hour|hours|day|days)$")
    RETURN valid_pattern.is_match(interval.to_lowercase())
END

SUBROUTINE: validate_transform
INPUT:
    transform: &Transform
    field_path: &str
    errors: &mut Vec<ValidationError>
OUTPUT:
    void
BEGIN
    MATCH transform.type
        UnitConversion =>
            IF transform.params.from.is_empty() OR transform.params.to.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "UnitConversion requires 'from' and 'to' parameters",
                    severity: Error
                })
            END IF
            IF transform.params.formula IS None THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "UnitConversion requires 'formula' parameter",
                    severity: Error
                })
            END IF
        Expression =>
            IF transform.params.expr.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "Expression transform requires 'expr' parameter",
                    severity: Error
                })
            END IF
        Lookup =>
            IF transform.params.table.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "Lookup transform requires non-empty 'table' parameter",
                    severity: Error
                })
            END IF
        JsonExtract =>
            IF transform.params.path.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "JsonExtract transform requires 'path' parameter",
                    severity: Error
                })
            END IF
        Timestamp =>
            // format enum is validated by serde
            PASS
        Computed =>
            IF transform.params.expr.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "Computed transform requires 'expr' parameter",
                    severity: Error
                })
            END IF
            IF transform.params.depends_on.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "Computed transform requires 'depends_on' parameter",
                    severity: Error
                })
            END IF
    END MATCH
END

SUBROUTINE: validate_dq_rule
INPUT:
    rule: &DqRule
    field_path: &str
    errors: &mut Vec<ValidationError>
OUTPUT:
    void
BEGIN
    MATCH rule.rule_type
        RangeCheck =>
            IF rule.params.min IS None OR rule.params.max IS None THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "RangeCheck requires 'min' and 'max' parameters",
                    severity: Error
                })
            ELSE IF rule.params.min > rule.params.max THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: format!(
                        "RangeCheck min ({}) cannot be greater than max ({})",
                        rule.params.min, rule.params.max
                    ),
                    severity: Error
                })
            END IF
        NotNull =>
            // No parameters required
            PASS
        Pattern =>
            IF rule.params.regex.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "Pattern rule requires 'regex' parameter",
                    severity: Error
                })
            ELSE
                // Validate regex compiles
                TRY
                    Regex::new(&rule.params.regex)
                CATCH e
                    errors.push(ValidationError {
                        field_path: field_path.to_string(),
                        message: format!("Invalid regex '{}': {}", rule.params.regex, e),
                        severity: Error
                    })
                END TRY
            END IF
        OneOf =>
            IF rule.params.values.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "OneOf rule requires non-empty 'values' array",
                    severity: Error
                })
            END IF
        Custom =>
            IF rule.params.name.is_empty() OR rule.params.expr.is_empty() THEN
                errors.push(ValidationError {
                    field_path: field_path.to_string(),
                    message: "Custom rule requires 'name' and 'expr' parameters",
                    severity: Error
                })
            END IF
    END MATCH

    // Validate action is appropriate for rule type
    IF rule.action == Clamp AND rule.rule_type != RangeCheck THEN
        errors.push(ValidationError {
            field_path: field_path.to_string(),
            message: "Clamp action is only valid for RangeCheck rules",
            severity: Error
        })
    END IF
END
```

**Complexity Analysis**:
- Time: O(f * r) where f = field_mappings count, r = average rules per field
- Space: O(e + w) where e = errors count, w = warnings count

---

### 3.5 Watch Config Changes (Hot Reload)

```
ALGORITHM: watch_config_changes
INPUT:
    self: &mut SilverConfigLoader
    callback: Fn(StreamId, ConfigChangeEvent)
OUTPUT:
    Result<WatchHandle, ConfigLoaderError>

DATA STRUCTURES:
    ENUM ConfigChangeEvent:
        Updated { config: SilverEtlConfig }
        Deleted
        ValidationFailed { errors: Vec<ValidationError> }

BEGIN
    IF self.etcd_client IS None THEN
        RETURN Err(ConfigLoaderError::WatchFailed {
            cause: "etcd client not available for watching"
        })
    END IF

    client <- self.etcd_client.unwrap()
    cache_ref <- self.cache.clone()  // Arc<RwLock<...>>
    callback_arc <- Arc::new(callback)

    // Create closure for etcd watch callback
    watch_callback <- move |key: String, value: Option<JsonValue>| {
        // Extract stream_id from key: "/streams/{stream_id}/config"
        stream_id <- extract_stream_id_from_key(&key)
        IF stream_id IS None THEN
            LOG_WARN("Ignoring watch event for non-stream key: {}", key)
            RETURN
        END IF
        stream_id <- stream_id.unwrap()

        MATCH value
            Some(json_value) =>
                // Config was updated
                TRY
                    full_config: StreamConfig <- serde_json::from_value(json_value)

                    IF full_config.silver_etl IS Some(silver_etl) THEN
                        // Validate the new config
                        MATCH validate_silver_config(&silver_etl, &stream_id)
                            Ok(()) =>
                                // Update cache
                                cache_write <- cache_ref.write()
                                cache_write.insert(stream_id.clone(), CachedConfig {
                                    config: silver_etl.clone(),
                                    loaded_at: now(),
                                    source: ConfigSource::Etcd {
                                        key: key.clone(),
                                        revision: 0  // Updated by etcd
                                    },
                                    version: 0
                                })
                                DROP cache_write

                                LOG_INFO("Config updated for stream: {}", stream_id)
                                callback_arc(stream_id, ConfigChangeEvent::Updated {
                                    config: silver_etl
                                })
                            Err(e) =>
                                LOG_ERROR("Validation failed for updated config {}: {:?}",
                                         stream_id, e)
                                IF let ConfigLoaderError::ValidationFailed { errors, .. } = e THEN
                                    callback_arc(stream_id, ConfigChangeEvent::ValidationFailed {
                                        errors
                                    })
                                END IF
                        END MATCH
                    ELSE
                        // silver_etl section was removed
                        LOG_INFO("silver_etl section removed for stream: {}", stream_id)

                        cache_write <- cache_ref.write()
                        cache_write.remove(&stream_id)
                        DROP cache_write

                        callback_arc(stream_id, ConfigChangeEvent::Deleted)
                    END IF
                CATCH parse_error
                    LOG_ERROR("Failed to parse config update for {}: {}",
                             stream_id, parse_error)
                END TRY
            None =>
                // Config was deleted
                LOG_INFO("Config deleted for stream: {}", stream_id)

                cache_write <- cache_ref.write()
                cache_write.remove(&stream_id)
                DROP cache_write

                callback_arc(stream_id, ConfigChangeEvent::Deleted)
        END MATCH
    }

    // Start etcd watch on /streams/ prefix
    TRY
        watch_handle <- client.watch("/", watch_callback)
        self.watchers.push(watch_handle.clone())
        LOG_INFO("Started watching /streams/ for config changes")
        RETURN Ok(watch_handle)
    CATCH e
        RETURN Err(ConfigLoaderError::WatchFailed {
            cause: e.to_string()
        })
    END TRY
END

SUBROUTINE: extract_stream_id_from_key
INPUT:
    key: &str
OUTPUT:
    Option<String>
BEGIN
    // Key format: "/streams/{stream_id}/config"
    IF NOT key.ends_with("/config") THEN
        RETURN None
    END IF

    parts <- key.split("/").collect::<Vec<_>>()
    // parts: ["", "streams", "{stream_id}", "config"]
    IF parts.len() >= 4 AND parts[1] == "streams" THEN
        RETURN Some(parts[2].to_string())
    END IF

    RETURN None
END
```

**Complexity Analysis**:
- Time: O(1) to set up watch, O(parse + validate) per change event
- Space: O(1) for watch setup, O(config_size) per change

---

### 3.6 Reload Config

```
ALGORITHM: reload_config
INPUT:
    self: &mut SilverConfigLoader
    stream_id: String
OUTPUT:
    Result<SilverEtlConfig, ConfigLoaderError>

BEGIN
    // Invalidate cache for this stream
    cache_write <- self.cache.write()
    cache_write.remove(&stream_id)
    DROP cache_write

    LOG_DEBUG("Cache invalidated for stream: {}", stream_id)

    // Reload from source
    RETURN load_stream_config(self, stream_id)
END
```

---

### 3.7 Get Cached Config (No Network)

```
ALGORITHM: get_cached_config
INPUT:
    self: &SilverConfigLoader
    stream_id: &str
OUTPUT:
    Option<SilverEtlConfig>

BEGIN
    cache_read <- self.cache.read()

    IF cache_read.contains_key(stream_id) THEN
        RETURN Some(cache_read.get(stream_id).config.clone())
    END IF

    RETURN None
END
```

---

### 3.8 Stop Watchers

```
ALGORITHM: stop_watchers
INPUT:
    self: &mut SilverConfigLoader
OUTPUT:
    void

BEGIN
    FOR handle IN self.watchers.drain(..) DO
        TRY
            handle.cancel().await
            LOG_DEBUG("Cancelled watch handle")
        CATCH e
            LOG_WARN("Failed to cancel watch: {}", e)
        END TRY
    END FOR

    LOG_INFO("All config watchers stopped")
END
```

---

## 4. Error Handling Patterns

### 4.1 Graceful Degradation

```
PATTERN: Graceful Degradation

SCENARIO: etcd unavailable during operation

BEHAVIOR:
    1. Log warning about etcd failure
    2. Continue serving from cache if available
    3. Attempt YAML fallback if cache miss
    4. Only fail if no config source available

IMPLEMENTATION:
    - Cache has no TTL for critical path
    - Background refresh attempts reconnection
    - Metrics track degraded state
```

### 4.2 Partial Success

```
PATTERN: Partial Success

SCENARIO: Some stream configs fail validation

BEHAVIOR:
    1. Load all configs that validate successfully
    2. Log errors for failed configs
    3. Return successful configs to ETL engine
    4. ETL processes available streams

RATIONALE:
    - One bad config shouldn't block all ETL
    - Operator can fix config while ETL runs
    - Metrics expose partial state
```

### 4.3 Validation Error Aggregation

```
PATTERN: Validation Error Aggregation

SCENARIO: Multiple validation errors in one config

BEHAVIOR:
    1. Collect ALL validation errors (don't fail fast)
    2. Return complete list to caller
    3. Log all errors in structured format
    4. Enable batch fixing by operator

EXAMPLE OUTPUT:
    ValidationFailed {
        stream_id: "outdoor-weather",
        errors: [
            { field: "target_table", message: "not qualified" },
            { field: "field_mappings[0].source_path", message: "invalid JSON path" },
            { field: "deduplication.key_columns", message: "unknown column 'foo'" }
        ]
    }
```

---

## 5. Hot Reload Mechanism Design

### 5.1 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CONFIG LOADER                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐         ┌──────────────────────────────┐  │
│  │   etcd Watch    │────────▶│   Change Processing Queue    │  │
│  │   (per prefix)  │         │   (bounded channel)          │  │
│  └─────────────────┘         └───────────────┬──────────────┘  │
│                                              │                  │
│                                              ▼                  │
│  ┌─────────────────┐         ┌──────────────────────────────┐  │
│  │   Config Cache  │◀────────│   Validation + Parse         │  │
│  │   (RwLock)      │         │   (per-config)               │  │
│  └────────┬────────┘         └───────────────┬──────────────┘  │
│           │                                  │                  │
│           │                                  ▼                  │
│           │                  ┌──────────────────────────────┐  │
│           │                  │   Callback Dispatch          │  │
│           │                  │   (ETL engine notification)  │  │
│           │                  └──────────────────────────────┘  │
│           │                                                    │
│           ▼                                                    │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                    ETL Engine                            │  │
│  │   - Receives config change events                        │  │
│  │   - Reschedules affected streams                         │  │
│  │   - Does NOT interrupt running ETL jobs                  │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Change Event Flow

```
SEQUENCE: Hot Reload Event Flow

1. OPERATOR: Updates config in etcd
   $ etcdctl put /streams/air-quality/config '{"silver_etl": {...}}'

2. ETCD: Publishes watch event
   Event { key: "/streams/air-quality/config", value: {...}, type: PUT }

3. CONFIG LOADER: Receives event via WatchHandle
   - Parses JSON to StreamConfig
   - Extracts silver_etl section
   - Validates config

4. CONFIG LOADER: Updates cache (if valid)
   cache.insert("air-quality", validated_config)

5. CONFIG LOADER: Invokes callback
   callback("air-quality", ConfigChangeEvent::Updated { config })

6. ETL ENGINE: Handles event
   - If stream ETL running: mark for refresh after completion
   - If stream ETL idle: update config immediately
   - Reschedule next ETL run if needed

7. LOGGING: Record event
   INFO "Config updated for stream: air-quality"
   DEBUG "New field_mappings count: 12"
```

### 5.3 Race Condition Handling

```
SCENARIO: Config change during ETL execution

PROBLEM:
    ETL reads config at T0
    Config changes at T1 (during ETL)
    ETL writes to Silver at T2 (with old config)

SOLUTION:
    - ETL engine snapshots config at job start
    - Config loader marks pending changes
    - After job completes, engine checks for pending changes
    - If pending, re-run with new config

PSEUDOCODE:
    BEGIN ETL_JOB(stream_id)
        config_snapshot <- config_loader.get_cached_config(stream_id)
        config_version <- config_snapshot.version

        // Run ETL with snapshot
        execute_etl(config_snapshot)

        // Check if config changed during execution
        current_version <- config_loader.get_cached_config(stream_id).version
        IF current_version > config_version THEN
            LOG_INFO("Config changed during ETL, marking for re-run")
            schedule_immediate_rerun(stream_id)
        END IF
    END
```

### 5.4 Debouncing Rapid Changes

```
ALGORITHM: Debounced Change Handler
INPUT:
    stream_id: String
    new_config: SilverEtlConfig
    debounce_window: Duration (default: 5 seconds)

STATE:
    pending_changes: HashMap<StreamId, PendingChange>

STRUCT PendingChange:
    config: SilverEtlConfig
    first_seen: Timestamp
    last_seen: Timestamp
    change_count: u32

BEGIN
    current_time <- now()

    IF pending_changes.contains_key(stream_id) THEN
        pending <- pending_changes.get_mut(stream_id)
        pending.config <- new_config
        pending.last_seen <- current_time
        pending.change_count += 1
        LOG_DEBUG("Debouncing change #{} for {}", pending.change_count, stream_id)
    ELSE
        pending_changes.insert(stream_id, PendingChange {
            config: new_config,
            first_seen: current_time,
            last_seen: current_time,
            change_count: 1
        })
    END IF

    // Schedule flush after debounce window
    schedule_after(debounce_window, || {
        flush_pending_change(stream_id)
    })
END

SUBROUTINE: flush_pending_change
INPUT:
    stream_id: String
BEGIN
    IF pending_changes.contains_key(stream_id) THEN
        pending <- pending_changes.remove(stream_id)
        time_since_last <- now() - pending.last_seen

        IF time_since_last >= debounce_window THEN
            LOG_INFO("Flushing {} changes for {} after debounce",
                    pending.change_count, stream_id)
            apply_config_change(stream_id, pending.config)
        ELSE
            // More changes came in, re-insert and wait
            pending_changes.insert(stream_id, pending)
        END IF
    END IF
END
```

---

## 6. Integration with Existing config-client

### 6.1 Dependency Relationship

```
┌─────────────────────────────────────────────────────────────────┐
│                      silver-etl binary                          │
│                                                                 │
│  ┌─────────────────┐         ┌──────────────────────────────┐  │
│  │ SilverConfigLoader │──────│ config-client::ConfigClient   │  │
│  │ (new module)       │      │ (existing crate)              │  │
│  └─────────────────────┘     └──────────────────────────────┘  │
│           │                           │                        │
│           │                           │                        │
│           │                           ▼                        │
│           │               ┌──────────────────────────────┐    │
│           │               │ config-client::StreamRegistry │    │
│           │               │ (existing - for Bronze)       │    │
│           │               └──────────────────────────────┘    │
│           │                           │                        │
│           ▼                           ▼                        │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                          etcd                            │  │
│  │   /streams/{stream_id}/config                            │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Reused Components from config-client

| Component | Usage in SilverConfigLoader |
|-----------|----------------------------|
| `ConfigClient` | etcd connection, get/set/watch |
| `WatchHandle` | Config change notifications |
| `ConfigError` | Error propagation (extended) |
| Key prefix convention | `/streams/{id}/config` |

### 6.3 Extension Points

```rust
// Extend ConfigError for Silver-specific errors
impl From<ConfigLoaderError> for ConfigError {
    fn from(e: ConfigLoaderError) -> Self {
        match e {
            ConfigLoaderError::ValidationFailed { stream_id, errors } => {
                ConfigError::EnvError(format!(
                    "Silver config validation failed for {}: {:?}",
                    stream_id, errors
                ))
            }
            ConfigLoaderError::EtcdConnectionFailed { cause, .. } => {
                ConfigError::ConnectionFailed(cause)
            }
            // ... other mappings
        }
    }
}
```

---

## 7. Usage Example

```rust
// apps/silver-etl/src/main.rs

async fn main() -> Result<()> {
    // Initialize config loader
    let loader = SilverConfigLoader::new(
        vec!["http://localhost:2379".to_string()],
        Some("/workspaces/neural-data-platform/config/base/streams".to_string())
    ).await?;

    // Load all enabled Silver configs
    let configs = loader.load_all_silver_configs().await?;
    info!("Loaded {} Silver ETL configurations", configs.len());

    // Set up hot reload
    let etl_engine = Arc::new(RwLock::new(EtlEngine::new()));
    let engine_ref = etl_engine.clone();

    loader.watch_config_changes(move |stream_id, event| {
        match event {
            ConfigChangeEvent::Updated { config } => {
                info!("Reloading config for stream: {}", stream_id);
                engine_ref.write().unwrap().update_config(stream_id, config);
            }
            ConfigChangeEvent::Deleted => {
                info!("Disabling stream: {}", stream_id);
                engine_ref.write().unwrap().disable_stream(stream_id);
            }
            ConfigChangeEvent::ValidationFailed { errors } => {
                error!("Invalid config for {}: {:?}", stream_id, errors);
                // Don't update engine - keep running with old config
            }
        }
    }).await?;

    // Run ETL loop
    loop {
        for config in loader.load_all_silver_configs().await? {
            etl_engine.read().unwrap().run_etl(&config).await?;
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
```

---

## 8. Test Scenarios

### 8.1 Unit Test Cases

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| `test_valid_config_loads` | Valid YAML config | Ok(SilverEtlConfig) |
| `test_missing_target_table` | Config without target_table | ValidationError |
| `test_unqualified_target_table` | target_table: "foo" | ValidationError about qualification |
| `test_invalid_json_path` | source_path: "invalid..path" | ValidationError |
| `test_key_column_not_in_output` | key_columns: ["nonexistent"] | ValidationError |
| `test_clamp_on_non_range_rule` | Pattern rule with Clamp action | ValidationError |
| `test_cache_hit` | Load same stream twice | Second call from cache |
| `test_etcd_fallback_to_yaml` | etcd unavailable | Loads from YAML |

### 8.2 Integration Test Cases

| Test Case | Setup | Verification |
|-----------|-------|--------------|
| `test_hot_reload_updates_config` | Update etcd, wait for callback | Callback invoked with new config |
| `test_hot_reload_handles_deletion` | Delete key in etcd | Callback invoked with Deleted event |
| `test_invalid_update_preserves_old` | Update with invalid config | Old config preserved in cache |
| `test_multiple_streams_load` | 4 streams in etcd | All 4 configs loaded |

---

## 9. Complexity Summary

| Function | Time Complexity | Space Complexity |
|----------|-----------------|------------------|
| `new` | O(1) + O(network) | O(1) |
| `load_all_silver_configs` | O(n * m) | O(n) |
| `load_stream_config` | O(1) cache, O(network) miss | O(config_size) |
| `validate_silver_config` | O(f * r) | O(errors) |
| `watch_config_changes` | O(1) setup | O(1) |

Where:
- n = number of streams
- m = average config load time
- f = field_mappings count
- r = average rules per field

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Pseudocode Specialist | Initial pseudocode specification |

---

## References

1. config-client crate: `/workspaces/neural-data-platform/config-client/`
2. Specification: `/workspaces/neural-data-platform/product/features/dp-006/specification/SPECIFICATION.md`
3. Architecture: `/workspaces/neural-data-platform/product/features/dp-006/architecture/ARCHITECTURE_OVERVIEW.md`
4. Config Design: `/workspaces/neural-data-platform/docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md`
