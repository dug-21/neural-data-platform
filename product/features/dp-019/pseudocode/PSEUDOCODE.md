# dp-019: Config Validation Pipeline - SPARC Pseudocode

**Document Type**: SPARC Pseudocode (Phase P)
**Feature**: dp-019 Config Validation Pipeline
**Version**: 1.0
**Date**: 2026-02-02
**Author**: Pseudocode Agent
**Prerequisites**: SPECIFICATION.md, SUPPORTED-VALUES-RESEARCH.md, VALIDATION-ARCHITECTURE.md, DQ-VALIDATION-RESEARCH.md

---

## 1. Executive Summary

This document provides detailed algorithmic specifications for the dp-019 Config Validation Pipeline. The design implements a two-layer validation architecture:

1. **Layer 1 (Schema)**: JSON Schema validation for structural correctness
2. **Layer 2 (Semantic)**: Rust-based validation for application rules

The algorithms prioritize:
- **Error accumulation** (collect all errors, do not fail fast)
- **Clear error paths** (JSONPath notation)
- **Graceful degradation** (optional database checks)
- **Actionable messages** (suggestions for fixes)

---

## 2. Data Structures

### 2.1 Core Types

```
STRUCT ValidationResult:
    valid: Boolean                      # True if no errors (warnings OK)
    config_path: String                 # Path to validated config file
    summary: ValidationSummary          # Error/warning counts
    errors: Array<ValidationError>      # Error details
    warnings: Array<ValidationError>    # Warning details

STRUCT ValidationSummary:
    total_errors: Integer
    total_warnings: Integer
    by_layer: Map<String, Integer>      # {"syntax": 0, "schema": 2, "semantic": 1}

STRUCT ValidationError:
    layer: ValidationLayer              # syntax | schema | semantic
    code: String                        # Error code (e.g., "INVALID_SOURCE_PATH")
    path: String                        # JSONPath (e.g., "$.silver_etl.field_mappings[2].source_path")
    message: String                     # Human-readable description
    severity: Severity                  # error | warning
    suggestion: Optional<String>        # Actionable fix suggestion
    context: Optional<Map>              # Additional context (e.g., available_fields)

ENUM ValidationLayer:
    SYNTAX                              # JSON parsing errors
    SCHEMA                              # JSON Schema violations
    SEMANTIC                            # Application rule violations

ENUM Severity:
    ERROR                               # Blocks deployment
    WARNING                             # Informational, does not block

ENUM FieldType:
    FLOAT, INT, STRING, BOOL, JSON

ENUM SourceType:
    MQTT, HTTP_POLL, WEBHOOK, FILE_WATCH, CSV

ENUM DqRuleType:
    RANGE_CHECK, NULL_CHECK, ENUM_CHECK, PATTERN_CHECK,
    FRESHNESS_CHECK, MONOTONIC_CHECK, RATE_OF_CHANGE,
    CROSS_FIELD_CHECK, CONDITIONAL_CHECK,
    COMPLETENESS_CHECK, CARDINALITY_CHECK

ENUM DqAction:
    FLAG, REJECT, CLAMP, DROP, WARN
```

### 2.2 Validator Configuration

```
STRUCT ValidatorConfig:
    schema_path: Path                   # Path to JSON Schema file
    db_pool: Optional<PgPool>           # Database connection for table checks
    supported_field_types: Set<String>  # {"float", "int", "string", "bool", "json"}
    supported_source_types: Set<String> # {"mqtt", "http_poll", "webhook", "file_watch", "csv"}
    supported_dq_rules: Set<String>     # All 11 DQ rule types
    supported_dq_actions: Set<String>   # {"flag", "reject", "clamp", "drop", "warn"}

STRUCT ValidationOptions:
    schema_only: Boolean                # Skip semantic validation (fast mode)
    check_tables: Boolean               # Verify Silver tables exist in DB
    check_source_paths: Boolean         # Verify source_path cross-references
    strict: Boolean                     # Treat warnings as errors
    format: OutputFormat                # json | human
```

### 2.3 Complexity Analysis: Data Structures

| Structure | Space Complexity | Notes |
|-----------|------------------|-------|
| ValidationResult | O(e) | e = number of errors |
| FieldNameSet | O(f) | f = number of fields |
| ColumnNameSet | O(m) | m = number of field_mappings |
| DqRuleNameSet | O(r) | r = number of DQ rules |

---

## 3. Main Validation Algorithm

### 3.1 Primary Entry Point

```
ALGORITHM: validate_config
INPUT:
    config_path: Path               # Path to JSON config file
    options: ValidationOptions      # Validation options
OUTPUT:
    ValidationResult                # Validation result with all errors/warnings

BEGIN
    errors <- []
    warnings <- []

    // ========================================
    // PHASE 1: Read and Parse JSON
    // ========================================
    json_content <- READ_FILE(config_path)

    IF json_content IS error THEN
        RETURN ValidationResult {
            valid: false,
            config_path: config_path,
            errors: [ValidationError {
                layer: SYNTAX,
                code: "FILE_NOT_FOUND",
                path: "$",
                message: "Cannot read config file: " + error_message,
                severity: ERROR
            }]
        }
    END IF

    // ========================================
    // PHASE 2: JSON Syntax Validation
    // ========================================
    parse_result <- PARSE_JSON(json_content)

    IF parse_result IS error THEN
        RETURN ValidationResult {
            valid: false,
            config_path: config_path,
            errors: [create_syntax_error(parse_result.error)]
        }
    END IF

    json_value <- parse_result.value

    // ========================================
    // PHASE 3: Layer 1 - JSON Schema Validation
    // ========================================
    schema_errors <- validate_json_schema(json_value, SCHEMA)
    errors <- errors + schema_errors

    // If schema validation fails catastrophically, cannot continue
    IF has_structural_errors(schema_errors) THEN
        RETURN build_result(config_path, errors, warnings)
    END IF

    // ========================================
    // PHASE 4: Deserialize to StreamConfig
    // ========================================
    config <- DESERIALIZE<StreamConfig>(json_value)

    IF config IS error THEN
        errors <- errors + [ValidationError {
            layer: SCHEMA,
            code: "DESERIALIZATION_FAILED",
            path: "$",
            message: "Failed to parse config: " + error_message,
            severity: ERROR
        }]
        RETURN build_result(config_path, errors, warnings)
    END IF

    // ========================================
    // PHASE 5: Layer 2 - Semantic Validation
    // ========================================
    IF NOT options.schema_only THEN
        semantic_result <- validate_semantic(config, options)
        errors <- errors + semantic_result.errors
        warnings <- warnings + semantic_result.warnings
    END IF

    RETURN build_result(config_path, errors, warnings)
END

// ----------------------------------------
// Helper: Build ValidationResult
// ----------------------------------------
FUNCTION build_result(config_path, errors, warnings) -> ValidationResult:
    summary <- ValidationSummary {
        total_errors: LENGTH(errors),
        total_warnings: LENGTH(warnings),
        by_layer: count_by_layer(errors + warnings)
    }

    RETURN ValidationResult {
        valid: LENGTH(errors) == 0,
        config_path: config_path,
        summary: summary,
        errors: errors,
        warnings: warnings
    }
END
```

### 3.2 Complexity Analysis: Main Algorithm

| Phase | Time Complexity | Space Complexity |
|-------|-----------------|------------------|
| File Read | O(n) | O(n) where n = file size |
| JSON Parse | O(n) | O(n) for AST |
| Schema Validation | O(n * s) | O(e) where s = schema size, e = errors |
| Semantic Validation | O(f + m + r) | O(e) where f = fields, m = mappings, r = rules |
| **Total** | O(n * s + f + m + r) | O(n + e) |

---

## 4. Layer 1: JSON Schema Validation

### 4.1 Schema Validation Algorithm

```
ALGORITHM: validate_json_schema
INPUT:
    json_value: Value               # Parsed JSON value
    schema: CompiledSchema          # Pre-compiled JSON Schema
OUTPUT:
    Array<ValidationError>          # Schema validation errors

BEGIN
    errors <- []

    // Run JSON Schema validation (jsonschema crate)
    validation_result <- schema.validate(json_value)

    IF validation_result IS success THEN
        RETURN []
    END IF

    // Collect all errors (do not fail fast)
    FOR EACH schema_error IN validation_result.errors DO
        error <- convert_schema_error(schema_error)
        errors <- errors + [error]
    END FOR

    RETURN errors
END

// ----------------------------------------
// Helper: Convert Schema Error to ValidationError
// ----------------------------------------
FUNCTION convert_schema_error(schema_error) -> ValidationError:
    path <- convert_instance_path_to_jsonpath(schema_error.instance_path)
    code <- determine_error_code(schema_error.kind)
    message <- format_schema_error_message(schema_error)
    suggestion <- generate_suggestion(schema_error)

    RETURN ValidationError {
        layer: SCHEMA,
        code: code,
        path: path,
        message: message,
        severity: ERROR,
        suggestion: suggestion
    }
END

// ----------------------------------------
// Helper: Convert Instance Path to JSONPath
// ----------------------------------------
FUNCTION convert_instance_path_to_jsonpath(instance_path) -> String:
    // instance_path example: ["silver_etl", "field_mappings", 2, "source_path"]
    // output: "$.silver_etl.field_mappings[2].source_path"

    path <- "$"

    FOR EACH segment IN instance_path DO
        IF segment IS integer THEN
            path <- path + "[" + segment + "]"
        ELSE
            path <- path + "." + segment
        END IF
    END FOR

    RETURN path
END

// ----------------------------------------
// Helper: Determine Error Code from Schema Error Kind
// ----------------------------------------
FUNCTION determine_error_code(kind) -> String:
    MATCH kind WITH
        "required" -> "MISSING_REQUIRED"
        "type" -> "INVALID_TYPE"
        "additionalProperties" -> "UNKNOWN_FIELD"
        "pattern" -> "PATTERN_MISMATCH"
        "enum" -> "ENUM_VIOLATION"
        "minItems", "maxItems" -> "ARRAY_BOUNDS"
        "minimum", "maximum" -> "VALUE_OUT_OF_RANGE"
        _ -> "SCHEMA_VIOLATION"
    END MATCH
END
```

### 4.2 Syntax Error with Line Numbers

```
ALGORITHM: create_syntax_error
INPUT:
    parse_error: JsonParseError     # Error from JSON parser
OUTPUT:
    ValidationError

BEGIN
    // Extract line and column from parser error
    line <- parse_error.line
    column <- parse_error.column

    message <- FORMAT(
        "JSON syntax error at line {}, column {}: {}",
        line, column, parse_error.message
    )

    RETURN ValidationError {
        layer: SYNTAX,
        code: "SYNTAX_ERROR",
        path: "$",
        message: message,
        severity: ERROR,
        context: {
            "line": line,
            "column": column
        }
    }
END
```

### 4.3 Schema Error Codes

| Code | JSON Schema Keyword | Example |
|------|---------------------|---------|
| `MISSING_REQUIRED` | `required` | `stream_id` missing |
| `INVALID_TYPE` | `type` | String where number expected |
| `UNKNOWN_FIELD` | `additionalProperties` | `silver_elt` (typo) |
| `PATTERN_MISMATCH` | `pattern` | `stream_id` not kebab-case |
| `ENUM_VIOLATION` | `enum` | `type: decimal` not supported |
| `ARRAY_BOUNDS` | `minItems`, `maxItems` | Empty `fields` array |
| `VALUE_OUT_OF_RANGE` | `minimum`, `maximum` | `retention_days: -1` |

---

## 5. Layer 2: Semantic Validation

### 5.1 Main Semantic Validation Algorithm

```
ALGORITHM: validate_semantic
INPUT:
    config: StreamConfig            # Deserialized config
    options: ValidationOptions      # Validation options
OUTPUT:
    SemanticResult {errors, warnings}

BEGIN
    errors <- []
    warnings <- []

    // ========================================
    // Build lookup sets for cross-reference validation
    // ========================================
    field_names <- BUILD_SET(config.fields, f -> f.name)

    silver_columns <- EMPTY_SET()
    IF config.silver_etl IS NOT NULL THEN
        silver_columns <- BUILD_SET(
            config.silver_etl.field_mappings,
            m -> m.target_column
        )
    END IF

    // ========================================
    // Run validation checks
    // ========================================

    // 5.2: Validate field types
    errors <- errors + validate_field_types(config.fields)

    // 5.3: Validate source types and required config
    errors <- errors + validate_sources(config.sources)

    // 5.4: Validate device_class (warning only)
    warnings <- warnings + validate_device_class(config.fields)

    // 5.5: Validate source_path cross-references
    IF options.check_source_paths AND config.silver_etl IS NOT NULL THEN
        errors <- errors + validate_source_paths(config, field_names)
    END IF

    // 5.6: Validate Silver table existence (if DB available)
    IF options.check_tables AND config.silver_etl IS NOT NULL THEN
        table_errors <- AWAIT validate_table_exists(config.silver_etl, DB_POOL)
        errors <- errors + table_errors
    END IF

    // 5.7: Validate Silver column compatibility
    IF options.check_tables AND config.silver_etl IS NOT NULL THEN
        column_errors <- AWAIT validate_column_compatibility(config.silver_etl, DB_POOL)
        errors <- errors + column_errors
    END IF

    // 5.8: Validate DQ rules
    IF config.silver_etl IS NOT NULL THEN
        dq_errors <- validate_dq_rules(config.silver_etl.dq_rules, silver_columns)
        errors <- errors + dq_errors
    END IF

    // 5.9: Validate transform functions
    IF config.silver_etl IS NOT NULL THEN
        transform_errors <- validate_transforms(config.silver_etl.field_mappings)
        errors <- errors + transform_errors
    END IF

    // 5.10: Validate retention/compression relationship
    IF config.retention_days IS NOT NULL AND config.compression_after_days IS NOT NULL THEN
        IF config.compression_after_days > config.retention_days THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "CONSTRAINT_VIOLATION",
                path: "$.retention_days",
                message: FORMAT(
                    "compression_after_days ({}) must be <= retention_days ({})",
                    config.compression_after_days, config.retention_days
                ),
                severity: ERROR
            }]
        END IF
    END IF

    // 5.11: Validate unique field names
    errors <- errors + validate_unique_names(config.fields, "name", "$.fields")

    // 5.12: Validate unique target_columns
    IF config.silver_etl IS NOT NULL THEN
        errors <- errors + validate_unique_names(
            config.silver_etl.field_mappings,
            "target_column",
            "$.silver_etl.field_mappings"
        )
    END IF

    RETURN {errors: errors, warnings: warnings}
END
```

### 5.2 Validate Field Types

```
ALGORITHM: validate_field_types
INPUT:
    fields: Array<Field>            # Config fields
OUTPUT:
    Array<ValidationError>

CONSTANTS:
    SUPPORTED_FIELD_TYPES <- {"float", "int", "string", "bool", "json"}

BEGIN
    errors <- []

    FOR idx, field IN ENUMERATE(fields) DO
        IF field.type NOT IN SUPPORTED_FIELD_TYPES THEN
            suggestion <- find_closest_match(field.type, SUPPORTED_FIELD_TYPES)

            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_FIELD_TYPE",
                path: FORMAT("$.fields[{}].type", idx),
                message: FORMAT(
                    "Field type '{}' is not supported. Must be one of: {}",
                    field.type, JOIN(SUPPORTED_FIELD_TYPES, ", ")
                ),
                severity: ERROR,
                suggestion: IF suggestion THEN "Did you mean '" + suggestion + "'?" ELSE NULL
            }]
        END IF

        // Validate type-specific constraints
        IF field.type IN {"string", "bool", "json"} THEN
            IF field.range IS NOT NULL THEN
                errors <- errors + [ValidationError {
                    layer: SEMANTIC,
                    code: "INVALID_RANGE",
                    path: FORMAT("$.fields[{}].range", idx),
                    message: FORMAT("Field type '{}' cannot have a range constraint", field.type),
                    severity: ERROR
                }]
            END IF

            IF field.display_precision IS NOT NULL THEN
                errors <- errors + [ValidationError {
                    layer: SEMANTIC,
                    code: "INVALID_PRECISION",
                    path: FORMAT("$.fields[{}].display_precision", idx),
                    message: FORMAT("Field type '{}' cannot have display_precision", field.type),
                    severity: ERROR
                }]
            END IF
        END IF

        IF field.type == "int" AND field.display_precision IS NOT NULL THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_PRECISION",
                path: FORMAT("$.fields[{}].display_precision", idx),
                message: "Integer fields cannot have display_precision",
                severity: ERROR
            }]
        END IF
    END FOR

    RETURN errors
END
```

### 5.3 Validate Sources

```
ALGORITHM: validate_sources
INPUT:
    sources: Array<Source>          # Config sources
OUTPUT:
    Array<ValidationError>

CONSTANTS:
    SUPPORTED_SOURCE_TYPES <- {"mqtt", "http_poll", "webhook", "file_watch", "csv"}

BEGIN
    errors <- []

    FOR idx, source IN ENUMERATE(sources) DO
        // Validate source type
        IF source.source_type NOT IN SUPPORTED_SOURCE_TYPES THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_SOURCE_TYPE",
                path: FORMAT("$.sources[{}].type", idx),
                message: FORMAT(
                    "Source type '{}' is not supported. Must be one of: {}",
                    source.source_type, JOIN(SUPPORTED_SOURCE_TYPES, ", ")
                ),
                severity: ERROR
            }]
            CONTINUE  // Skip further validation for this source
        END IF

        // Validate source-specific required fields
        MATCH source.source_type WITH
            "mqtt" ->
                IF source.broker_url IS NULL OR source.broker_url == "" THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "MISSING_SOURCE_CONFIG",
                        path: FORMAT("$.sources[{}]", idx),
                        message: "MQTT source requires 'broker_url'",
                        severity: ERROR
                    }]
                END IF
                IF source.topics IS NULL OR LENGTH(source.topics) == 0 THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "MISSING_SOURCE_CONFIG",
                        path: FORMAT("$.sources[{}]", idx),
                        message: "MQTT source requires at least one topic",
                        severity: ERROR
                    }]
                END IF

            "http_poll" ->
                IF source.endpoints IS NULL OR LENGTH(source.endpoints) == 0 THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "MISSING_SOURCE_CONFIG",
                        path: FORMAT("$.sources[{}]", idx),
                        message: "HTTP poll source requires at least one endpoint",
                        severity: ERROR
                    }]
                END IF
                IF source.poll_interval_secs IS NULL OR source.poll_interval_secs <= 0 THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "INVALID_SOURCE_CONFIG",
                        path: FORMAT("$.sources[{}].poll_interval_secs", idx),
                        message: "HTTP poll source requires positive poll_interval_secs",
                        severity: ERROR
                    }]
                END IF

            "csv" ->
                IF source.path IS NULL THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "MISSING_SOURCE_CONFIG",
                        path: FORMAT("$.sources[{}]", idx),
                        message: "CSV source requires 'path'",
                        severity: ERROR
                    }]
                END IF
                IF source.timestamp_field IS NULL THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "MISSING_SOURCE_CONFIG",
                        path: FORMAT("$.sources[{}]", idx),
                        message: "CSV source requires 'timestamp_field'",
                        severity: ERROR
                    }]
                END IF
        END MATCH
    END FOR

    RETURN errors
END
```

### 5.4 Validate Device Class (Warning)

```
ALGORITHM: validate_device_class
INPUT:
    fields: Array<Field>
OUTPUT:
    Array<ValidationError>          # Warnings only

CONSTANTS:
    // Home Assistant compatible device classes
    KNOWN_DEVICE_CLASSES <- {
        "air_quality", "binary_sensor", "temperature", "humidity",
        "pressure", "weather", "wind_speed", "precipitation",
        "illuminance", "motion", "door", "window", "moisture",
        "gas", "smoke", "co2", "pm25", "pm10"
    }

BEGIN
    warnings <- []

    FOR idx, field IN ENUMERATE(fields) DO
        IF field.device_class IS NOT NULL THEN
            IF field.device_class NOT IN KNOWN_DEVICE_CLASSES THEN
                warnings <- warnings + [ValidationError {
                    layer: SEMANTIC,
                    code: "UNKNOWN_DEVICE_CLASS",
                    path: FORMAT("$.fields[{}].device_class", idx),
                    message: FORMAT(
                        "Unknown device_class '{}'. This may be intentional for custom integrations.",
                        field.device_class
                    ),
                    severity: WARNING,
                    suggestion: "Known device classes: " + JOIN(KNOWN_DEVICE_CLASSES, ", ")
                }]
            END IF
        END IF
    END FOR

    RETURN warnings
END
```

### 5.5 Validate Source Path Cross-References

```
ALGORITHM: validate_source_paths
INPUT:
    config: StreamConfig
    field_names: Set<String>        # Valid field names from config.fields
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []
    silver_etl <- config.silver_etl

    FOR idx, mapping IN ENUMERATE(silver_etl.field_mappings) DO
        source_path <- mapping.source_path

        // Extract field reference from source_path
        // Format: "raw_payload.field_name" or "raw_payload.nested.path"
        IF source_path STARTS_WITH "raw_payload." THEN
            field_ref <- source_path.substring_after("raw_payload.")

            // For nested paths, check the root field
            root_field <- field_ref.split(".")[0]

            IF root_field NOT IN field_names THEN
                // Find closest match for suggestion
                closest <- find_closest_match(root_field, field_names)

                errors <- errors + [ValidationError {
                    layer: SEMANTIC,
                    code: "INVALID_SOURCE_PATH",
                    path: FORMAT("$.silver_etl.field_mappings[{}].source_path", idx),
                    message: FORMAT(
                        "source_path '{}' references field '{}' which is not defined in config.fields",
                        source_path, root_field
                    ),
                    severity: ERROR,
                    suggestion: IF closest THEN "Did you mean '" + closest + "'?" ELSE NULL,
                    context: {
                        "available_fields": ARRAY(field_names)
                    }
                }]
            END IF
        ELSE
            // source_path must start with raw_payload.
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_SOURCE_PATH",
                path: FORMAT("$.silver_etl.field_mappings[{}].source_path", idx),
                message: FORMAT(
                    "source_path '{}' must start with 'raw_payload.'",
                    source_path
                ),
                severity: ERROR
            }]
        END IF
    END FOR

    RETURN errors
END

// ----------------------------------------
// Helper: Levenshtein Distance for Suggestions
// ----------------------------------------
FUNCTION find_closest_match(input: String, candidates: Set<String>) -> Optional<String>:
    min_distance <- INFINITY
    closest <- NULL

    FOR EACH candidate IN candidates DO
        distance <- levenshtein_distance(input.lowercase(), candidate.lowercase())
        IF distance < min_distance AND distance <= 3 THEN  // Max 3 edits
            min_distance <- distance
            closest <- candidate
        END IF
    END FOR

    RETURN closest
END
```

### 5.6 Validate Silver Table Existence

```
ALGORITHM: validate_table_exists
INPUT:
    silver_etl: SilverEtlConfig
    db_pool: PgPool
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []
    target_table <- silver_etl.target_table

    // Parse "schema.table" format
    parts <- target_table.split(".")

    IF LENGTH(parts) != 2 THEN
        RETURN [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_TABLE_FORMAT",
            path: "$.silver_etl.target_table",
            message: FORMAT(
                "Invalid table format '{}'. Expected 'schema.table'",
                target_table
            ),
            severity: ERROR
        }]
    END IF

    schema_name <- parts[0]
    table_name <- parts[1]

    // Query database for table existence
    TRY
        exists <- AWAIT db_pool.query_scalar(
            SQL"SELECT EXISTS(
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = $1 AND table_name = $2
            )",
            [schema_name, table_name]
        )

        IF NOT exists THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "TABLE_NOT_FOUND",
                path: "$.silver_etl.target_table",
                message: FORMAT(
                    "Silver table '{}' does not exist in TimescaleDB",
                    target_table
                ),
                severity: ERROR,
                suggestion: "Create the table first or check the table name"
            }]
        END IF

    CATCH db_error
        // Graceful degradation - cannot check, emit warning
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "TABLE_CHECK_FAILED",
            path: "$.silver_etl.target_table",
            message: FORMAT(
                "Could not verify table existence: {}. Table check skipped.",
                db_error.message
            ),
            severity: WARNING
        }]
    END TRY

    RETURN errors
END
```

### 5.7 Validate Column Compatibility

```
ALGORITHM: validate_column_compatibility
INPUT:
    silver_etl: SilverEtlConfig
    db_pool: PgPool
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []
    target_table <- silver_etl.target_table
    parts <- target_table.split(".")

    IF LENGTH(parts) != 2 THEN
        RETURN []  // Already caught by table existence check
    END IF

    schema_name <- parts[0]
    table_name <- parts[1]

    TRY
        // Get actual columns from database
        db_columns <- AWAIT db_pool.query(
            SQL"SELECT column_name, data_type, is_nullable
                FROM information_schema.columns
                WHERE table_schema = $1 AND table_name = $2",
            [schema_name, table_name]
        )

        db_column_map <- BUILD_MAP(db_columns, c -> c.column_name, c -> c)

        FOR idx, mapping IN ENUMERATE(silver_etl.field_mappings) DO
            target_col <- mapping.target_column

            IF target_col NOT IN db_column_map THEN
                errors <- errors + [ValidationError {
                    layer: SEMANTIC,
                    code: "COLUMN_NOT_FOUND",
                    path: FORMAT("$.silver_etl.field_mappings[{}].target_column", idx),
                    message: FORMAT(
                        "Column '{}' does not exist in table '{}'",
                        target_col, target_table
                    ),
                    severity: ERROR,
                    context: {
                        "available_columns": KEYS(db_column_map)
                    }
                }]
            ELSE
                // Optional: Validate type compatibility
                db_type <- db_column_map[target_col].data_type
                config_type <- mapping.target_type

                IF NOT types_compatible(config_type, db_type) THEN
                    errors <- errors + [ValidationError {
                        layer: SEMANTIC,
                        code: "TYPE_MISMATCH",
                        path: FORMAT("$.silver_etl.field_mappings[{}].target_type", idx),
                        message: FORMAT(
                            "Config type '{}' may not be compatible with database type '{}'",
                            config_type, db_type
                        ),
                        severity: WARNING
                    }]
                END IF
            END IF
        END FOR

    CATCH db_error
        // Graceful degradation
        RETURN [ValidationError {
            layer: SEMANTIC,
            code: "COLUMN_CHECK_FAILED",
            path: "$.silver_etl.field_mappings",
            message: FORMAT("Could not verify column compatibility: {}", db_error.message),
            severity: WARNING
        }]
    END TRY

    RETURN errors
END
```

---

## 6. DQ Rule Validation

### 6.1 Main DQ Rule Validation

```
ALGORITHM: validate_dq_rules
INPUT:
    dq_rules: Array<DqRule>         # DQ rules from config
    silver_columns: Set<String>     # Valid Silver column names
OUTPUT:
    Array<ValidationError>

CONSTANTS:
    SUPPORTED_DQ_RULES <- {
        "range_check", "null_check", "enum_check", "pattern_check",
        "freshness_check", "monotonic_check", "rate_of_change",
        "cross_field_check", "conditional_check",
        "completeness_check", "cardinality_check"
    }

    SUPPORTED_ACTIONS <- {"flag", "reject", "clamp", "drop", "warn"}

    // Action compatibility matrix
    ACTION_COMPATIBILITY <- {
        "range_check": {"flag", "reject", "clamp"},
        "null_check": {"flag", "reject"},
        "enum_check": {"flag", "reject"},
        "pattern_check": {"flag", "reject"},
        "freshness_check": {"flag", "reject"},
        "monotonic_check": {"flag"},
        "rate_of_change": {"flag"},
        "cross_field_check": {"flag", "reject"},
        "conditional_check": {"flag", "reject"},
        "completeness_check": {"warn", "flag"},
        "cardinality_check": {"warn", "flag"}
    }

BEGIN
    errors <- []
    rule_names <- EMPTY_SET()  // Track uniqueness of rule names

    FOR idx, rule IN ENUMERATE(dq_rules) DO
        base_path <- FORMAT("$.silver_etl.dq_rules[{}]", idx)

        // Validate rule type
        IF rule.rule NOT IN SUPPORTED_DQ_RULES THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_DQ_RULE_TYPE",
                path: base_path + ".rule",
                message: FORMAT(
                    "DQ rule type '{}' is not supported. Must be one of: {}",
                    rule.rule, JOIN(SUPPORTED_DQ_RULES, ", ")
                ),
                severity: ERROR
            }]
            CONTINUE
        END IF

        // Validate action compatibility
        IF rule.action IS NOT NULL THEN
            IF rule.action NOT IN ACTION_COMPATIBILITY[rule.rule] THEN
                errors <- errors + [ValidationError {
                    layer: SEMANTIC,
                    code: "INVALID_DQ_ACTION",
                    path: base_path + ".action",
                    message: FORMAT(
                        "Action '{}' is not valid for rule type '{}'. Valid actions: {}",
                        rule.action, rule.rule, JOIN(ACTION_COMPATIBILITY[rule.rule], ", ")
                    ),
                    severity: ERROR
                }]
            END IF
        END IF

        // Validate field reference (for field-based rules)
        IF rule.field IS NOT NULL AND rule.rule NOT IN {"cross_field_check", "conditional_check"} THEN
            IF rule.field NOT IN silver_columns THEN
                errors <- errors + [ValidationError {
                    layer: SEMANTIC,
                    code: "INVALID_DQ_COLUMN",
                    path: base_path + ".field",
                    message: FORMAT(
                        "DQ rule references unknown column '{}'. Must be one of: {}",
                        rule.field, JOIN(silver_columns, ", ")
                    ),
                    severity: ERROR
                }]
            END IF
        END IF

        // Rule-specific validation
        MATCH rule.rule WITH
            "range_check" -> errors <- errors + validate_range_check(rule, base_path)
            "enum_check" -> errors <- errors + validate_enum_check(rule, base_path)
            "pattern_check" -> errors <- errors + validate_pattern_check(rule, base_path)
            "freshness_check" -> errors <- errors + validate_freshness_check(rule, base_path)
            "monotonic_check" -> errors <- errors + validate_monotonic_check(rule, base_path, silver_columns)
            "rate_of_change" -> errors <- errors + validate_rate_of_change(rule, base_path, silver_columns)
            "cross_field_check" -> errors <- errors + validate_cross_field_check(rule, base_path, silver_columns, rule_names)
            "conditional_check" -> errors <- errors + validate_conditional_check(rule, base_path, silver_columns, rule_names)
            "completeness_check" -> errors <- errors + validate_completeness_check(rule, base_path)
            "cardinality_check" -> errors <- errors + validate_cardinality_check(rule, base_path)
        END MATCH
    END FOR

    RETURN errors
END
```

### 6.2 Validate range_check

```
ALGORITHM: validate_range_check
INPUT:
    rule: DqRule
    base_path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    // At least one of min or max required
    IF rule.min IS NULL AND rule.max IS NULL THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path,
            message: "range_check requires at least one of 'min' or 'max'",
            severity: ERROR
        }]
        RETURN errors
    END IF

    // If both specified, min must be less than max
    IF rule.min IS NOT NULL AND rule.max IS NOT NULL THEN
        IF rule.min >= rule.max THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_DQ_RULE",
                path: base_path,
                message: FORMAT(
                    "range_check min ({}) must be less than max ({})",
                    rule.min, rule.max
                ),
                severity: ERROR
            }]
        END IF
    END IF

    // clamp_to_bounds only valid with clamp action
    IF rule.clamp_to_bounds == true AND rule.action != "clamp" THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".clamp_to_bounds",
            message: "clamp_to_bounds is only valid when action is 'clamp'",
            severity: WARNING
        }]
    END IF

    RETURN errors
END
```

### 6.3 Validate enum_check

```
ALGORITHM: validate_enum_check
INPUT:
    rule: DqRule
    base_path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    IF rule.allowed_values IS NULL OR LENGTH(rule.allowed_values) == 0 THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".allowed_values",
            message: "enum_check requires non-empty 'allowed_values' array",
            severity: ERROR
        }]
    END IF

    RETURN errors
END
```

### 6.4 Validate pattern_check (Regex)

```
ALGORITHM: validate_pattern_check
INPUT:
    rule: DqRule
    base_path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    IF rule.pattern IS NULL OR rule.pattern == "" THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".pattern",
            message: "pattern_check requires a 'pattern' regex",
            severity: ERROR
        }]
        RETURN errors
    END IF

    // Validate regex syntax
    TRY
        COMPILE_REGEX(rule.pattern)
    CATCH regex_error
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_REGEX",
            path: base_path + ".pattern",
            message: FORMAT("Invalid regex pattern: {}", regex_error.message),
            severity: ERROR
        }]
    END TRY

    RETURN errors
END
```

### 6.5 Validate freshness_check (Interval)

```
ALGORITHM: validate_freshness_check
INPUT:
    rule: DqRule
    base_path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    // At least one of max_age or max_future should be specified
    IF rule.max_age IS NULL AND rule.max_future IS NULL THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path,
            message: "freshness_check should have 'max_age' or 'max_future'",
            severity: WARNING
        }]
    END IF

    // Validate interval format for max_age
    IF rule.max_age IS NOT NULL THEN
        IF NOT is_valid_interval(rule.max_age) THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_INTERVAL",
                path: base_path + ".max_age",
                message: FORMAT(
                    "Invalid interval '{}'. Examples: '2 hours', '30 minutes', '1 day'",
                    rule.max_age
                ),
                severity: ERROR
            }]
        END IF
    END IF

    // Validate interval format for max_future
    IF rule.max_future IS NOT NULL THEN
        IF NOT is_valid_interval(rule.max_future) THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_INTERVAL",
                path: base_path + ".max_future",
                message: FORMAT(
                    "Invalid interval '{}'. Examples: '5 minutes', '1 hour'",
                    rule.max_future
                ),
                severity: ERROR
            }]
        END IF
    END IF

    RETURN errors
END

// ----------------------------------------
// Helper: Validate PostgreSQL Interval Format
// ----------------------------------------
FUNCTION is_valid_interval(interval: String) -> Boolean:
    // Valid patterns: "N unit", "N unit N unit"
    // Units: seconds, minutes, hours, days, weeks, months, years
    REGEX pattern <- /^\d+\s+(second|seconds|sec|s|minute|minutes|min|m|hour|hours|h|day|days|d|week|weeks|w|month|months|year|years)(\s+\d+\s+(second|seconds|sec|minute|minutes|min|m|hour|hours|h|day|days|d))?$/i

    RETURN pattern.matches(interval)
END
```

### 6.6 Validate cross_field_check (SQL Expression)

```
ALGORITHM: validate_cross_field_check
INPUT:
    rule: DqRule
    base_path: String
    silver_columns: Set<String>
    rule_names: Set<String>         # For uniqueness check
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    // Name is required and must be unique
    IF rule.name IS NULL OR rule.name == "" THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".name",
            message: "cross_field_check requires a 'name'",
            severity: ERROR
        }]
    ELSE
        IF rule.name IN rule_names THEN
            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "DUPLICATE_NAME",
                path: base_path + ".name",
                message: FORMAT("Duplicate DQ rule name '{}'", rule.name),
                severity: ERROR
            }]
        ELSE
            rule_names.add(rule.name)
        END IF
    END IF

    // Expression is required
    IF rule.expression IS NULL OR rule.expression == "" THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".expression",
            message: "cross_field_check requires an 'expression'",
            severity: ERROR
        }]
        RETURN errors
    END IF

    // Parse and validate SQL expression
    expression_errors <- validate_sql_expression(rule.expression, silver_columns, base_path + ".expression")
    errors <- errors + expression_errors

    RETURN errors
END

// ----------------------------------------
// Sub-algorithm: Validate SQL Expression
// ----------------------------------------
ALGORITHM: validate_sql_expression
INPUT:
    expression: String
    valid_columns: Set<String>
    path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    // Attempt to parse as SQL
    TRY
        // Wrap in SELECT to make it a valid SQL statement
        ast <- PARSE_SQL("SELECT " + expression)
    CATCH parse_error
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_SYNTAX",
            path: path,
            message: FORMAT("Invalid SQL expression: {}", parse_error.message),
            severity: ERROR
        }]
        RETURN errors
    END TRY

    // Extract column references from AST
    referenced_columns <- extract_column_references(ast)

    // Validate all referenced columns exist
    FOR EACH col IN referenced_columns DO
        IF col NOT IN valid_columns THEN
            closest <- find_closest_match(col, valid_columns)

            errors <- errors + [ValidationError {
                layer: SEMANTIC,
                code: "INVALID_DQ_COLUMN",
                path: path,
                message: FORMAT("Unknown column '{}' in expression", col),
                severity: ERROR,
                suggestion: IF closest THEN "Did you mean '" + closest + "'?" ELSE NULL
            }]
        END IF
    END FOR

    // Check for disallowed constructs (subqueries not supported in streaming)
    IF contains_subquery(ast) THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_SYNTAX",
            path: path,
            message: "Subqueries are not supported in DQ expressions",
            severity: ERROR
        }]
    END IF

    RETURN errors
END
```

### 6.7 Validate completeness_check and cardinality_check

```
ALGORITHM: validate_completeness_check
INPUT:
    rule: DqRule
    base_path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    // level must be "batch"
    IF rule.level != "batch" THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".level",
            message: "completeness_check requires level: 'batch'",
            severity: ERROR
        }]
    END IF

    // min_completeness must be 0.0-1.0
    IF rule.min_completeness IS NULL THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".min_completeness",
            message: "completeness_check requires 'min_completeness'",
            severity: ERROR
        }]
    ELSE IF rule.min_completeness < 0.0 OR rule.min_completeness > 1.0 THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".min_completeness",
            message: FORMAT(
                "min_completeness must be between 0.0 and 1.0, got {}",
                rule.min_completeness
            ),
            severity: ERROR
        }]
    END IF

    RETURN errors
END

ALGORITHM: validate_cardinality_check
INPUT:
    rule: DqRule
    base_path: String
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    // level must be "batch"
    IF rule.level != "batch" THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".level",
            message: "cardinality_check requires level: 'batch'",
            severity: ERROR
        }]
    END IF

    // expected_range must be [min, max] with min <= max
    IF rule.expected_range IS NULL THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".expected_range",
            message: "cardinality_check requires 'expected_range' array",
            severity: ERROR
        }]
    ELSE IF LENGTH(rule.expected_range) != 2 THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".expected_range",
            message: "expected_range must be exactly [min, max]",
            severity: ERROR
        }]
    ELSE IF rule.expected_range[0] > rule.expected_range[1] THEN
        errors <- errors + [ValidationError {
            layer: SEMANTIC,
            code: "INVALID_DQ_RULE",
            path: base_path + ".expected_range",
            message: FORMAT(
                "expected_range[0] ({}) must be <= expected_range[1] ({})",
                rule.expected_range[0], rule.expected_range[1]
            ),
            severity: ERROR
        }]
    END IF

    RETURN errors
END
```

### 6.8 Complexity Analysis: DQ Validation

| Rule Type | Time Complexity | Notes |
|-----------|-----------------|-------|
| range_check | O(1) | Constant field checks |
| enum_check | O(v) | v = allowed_values count |
| pattern_check | O(p) | p = pattern length (regex compile) |
| freshness_check | O(1) | Interval string parsing |
| cross_field_check | O(e + c) | e = expression length, c = column count |
| **Total DQ validation** | O(r * (e + c)) | r = rule count |

---

## 7. CLI Interface Algorithm

### 7.1 Main CLI Entry Point

```
ALGORITHM: cli_main
INPUT:
    args: Array<String>             # Command line arguments
OUTPUT:
    ExitCode                        # 0 = success, 1 = validation error, 2 = system error

BEGIN
    // Parse CLI arguments
    options <- parse_cli_args(args)

    IF options IS error THEN
        PRINT_STDERR("Error: " + options.error_message)
        PRINT_STDERR(USAGE_STRING)
        RETURN EXIT_CODE_SYSTEM_ERROR  // 2
    END IF

    // Handle --help
    IF options.help THEN
        PRINT(HELP_STRING)
        RETURN EXIT_CODE_SUCCESS  // 0
    END IF

    // Handle --version
    IF options.version THEN
        PRINT("ndp-validate " + VERSION)
        RETURN EXIT_CODE_SUCCESS  // 0
    END IF

    // Determine config files to validate
    config_files <- []

    IF options.all THEN
        // Discover all configs in base directory
        config_files <- discover_config_files(CONFIG_BASE_DIR)
    ELSE IF options.config_path IS NOT NULL THEN
        config_files <- [options.config_path]
    ELSE
        PRINT_STDERR("Error: Specify config path or use --all")
        RETURN EXIT_CODE_SYSTEM_ERROR  // 2
    END IF

    IF LENGTH(config_files) == 0 THEN
        PRINT_STDERR("No config files found")
        RETURN EXIT_CODE_SYSTEM_ERROR  // 2
    END IF

    // Initialize validator
    validator <- create_validator(options)

    IF validator IS error THEN
        PRINT_STDERR("Failed to initialize validator: " + validator.error_message)
        RETURN EXIT_CODE_SYSTEM_ERROR  // 2
    END IF

    // Validate all configs
    all_results <- []
    has_errors <- false

    FOR EACH config_path IN config_files DO
        IF options.verbose THEN
            PRINT_STDERR("Validating: " + config_path)
        END IF

        result <- AWAIT validator.validate(config_path, options)
        all_results <- all_results + [result]

        IF NOT result.valid THEN
            has_errors <- true
        END IF
    END FOR

    // Output results
    IF options.format == "json" THEN
        output_json(all_results)
    ELSE
        output_human(all_results)
    END IF

    // Determine exit code
    IF options.strict THEN
        // In strict mode, warnings also fail
        total_issues <- SUM(all_results, r -> r.summary.total_errors + r.summary.total_warnings)
        IF total_issues > 0 THEN
            RETURN EXIT_CODE_VALIDATION_ERROR  // 1
        END IF
    ELSE
        IF has_errors THEN
            RETURN EXIT_CODE_VALIDATION_ERROR  // 1
        END IF
    END IF

    RETURN EXIT_CODE_SUCCESS  // 0
END
```

### 7.2 CLI Argument Parsing

```
STRUCT CliOptions:
    all: Boolean                    # --all: Validate all configs
    schema_only: Boolean            # --schema-only: Skip semantic validation
    check_tables: Boolean           # --check-tables: Verify table existence
    strict: Boolean                 # --strict: Treat warnings as errors
    format: String                  # --format: json | human
    verbose: Boolean                # --verbose: Show progress
    config_path: Optional<Path>     # Positional: single config path
    help: Boolean                   # --help
    version: Boolean                # --version

FUNCTION parse_cli_args(args: Array<String>) -> Result<CliOptions, ParseError>:
    options <- CliOptions {
        all: false,
        schema_only: false,
        check_tables: false,
        strict: false,
        format: "json",             # Default to JSON for scripting
        verbose: false,
        config_path: NULL,
        help: false,
        version: false
    }

    i <- 0
    WHILE i < LENGTH(args) DO
        arg <- args[i]

        MATCH arg WITH
            "--all", "-a" -> options.all <- true
            "--schema-only" -> options.schema_only <- true
            "--check-tables" -> options.check_tables <- true
            "--strict" -> options.strict <- true
            "--verbose", "-v" -> options.verbose <- true
            "--help", "-h" -> options.help <- true
            "--version", "-V" -> options.version <- true

            "--format" ->
                IF i + 1 >= LENGTH(args) THEN
                    RETURN Error("--format requires value (json|human)")
                END IF
                i <- i + 1
                IF args[i] NOT IN {"json", "human"} THEN
                    RETURN Error("--format must be 'json' or 'human'")
                END IF
                options.format <- args[i]

            _ ->
                IF arg STARTS_WITH "-" THEN
                    RETURN Error("Unknown option: " + arg)
                ELSE
                    options.config_path <- arg
                END IF
        END MATCH

        i <- i + 1
    END WHILE

    RETURN Ok(options)
END
```

### 7.3 Output Formatting

```
ALGORITHM: output_json
INPUT:
    results: Array<ValidationResult>

BEGIN
    IF LENGTH(results) == 1 THEN
        // Single config: output result directly
        PRINT(JSON_SERIALIZE(results[0]))
    ELSE
        // Multiple configs: output array with summary
        summary <- {
            "total_configs": LENGTH(results),
            "valid_configs": COUNT(results, r -> r.valid),
            "invalid_configs": COUNT(results, r -> NOT r.valid),
            "total_errors": SUM(results, r -> r.summary.total_errors),
            "total_warnings": SUM(results, r -> r.summary.total_warnings)
        }

        output <- {
            "summary": summary,
            "results": results
        }

        PRINT(JSON_SERIALIZE(output))
    END IF
END

ALGORITHM: output_human
INPUT:
    results: Array<ValidationResult>

BEGIN
    FOR EACH result IN results DO
        IF result.valid THEN
            PRINT(GREEN("[PASS]") + " " + result.config_path)
        ELSE
            PRINT(RED("[FAIL]") + " " + result.config_path)

            // Print errors
            IF LENGTH(result.errors) > 0 THEN
                PRINT("")
                PRINT("  ERRORS:")
                FOR EACH error IN result.errors DO
                    PRINT(FORMAT("    [{}] {}", error.layer, error.path))
                    PRINT("      " + error.message)
                    IF error.suggestion IS NOT NULL THEN
                        PRINT(YELLOW("      Suggestion: " + error.suggestion))
                    END IF
                END FOR
            END IF

            // Print warnings
            IF LENGTH(result.warnings) > 0 THEN
                PRINT("")
                PRINT("  WARNINGS:")
                FOR EACH warning IN result.warnings DO
                    PRINT(FORMAT("    [{}] {}", warning.layer, warning.path))
                    PRINT("      " + warning.message)
                END FOR
            END IF
        END IF

        PRINT("")
    END FOR

    // Print summary
    total_errors <- SUM(results, r -> r.summary.total_errors)
    total_warnings <- SUM(results, r -> r.summary.total_warnings)
    valid_count <- COUNT(results, r -> r.valid)

    PRINT("=" * 60)
    PRINT(FORMAT(
        "SUMMARY: {} configs validated, {} passed, {} failed",
        LENGTH(results), valid_count, LENGTH(results) - valid_count
    ))
    PRINT(FORMAT("         {} errors, {} warnings", total_errors, total_warnings))
END
```

---

## 8. deploy.sh Integration

### 8.1 Deploy Validation Gate

```
ALGORITHM: deploy_validate
INPUT:
    environment: String             # e.g., "pi", "dev"
OUTPUT:
    ExitCode

# Location: deploy/pi/deploy.sh (or deploy/common/validate.sh)

BEGIN
    PRINT("Validating configurations...")

    // Determine validation mode based on environment
    IF environment == "ci" THEN
        // CI: schema-only (no DB connection)
        validation_args <- ["--all", "--schema-only", "--format", "human"]
    ELSE
        // Production: full validation if DB available
        IF DATABASE_URL IS SET THEN
            validation_args <- ["--all", "--check-tables", "--format", "human"]
        ELSE
            validation_args <- ["--all", "--format", "human"]
            PRINT_WARNING("DATABASE_URL not set; skipping table existence checks")
        END IF
    END IF

    // Run validator
    exit_code <- EXECUTE("ndp-validate", validation_args)

    IF exit_code != 0 THEN
        PRINT_ERROR("Configuration validation FAILED")
        PRINT_ERROR("Deploy aborted. Fix the errors above and retry.")
        RETURN 1
    END IF

    PRINT_SUCCESS("Configuration validation PASSED")
    RETURN 0
END

# Integration in deploy.sh sync action:
#
# sync_action() {
#     # Validate first
#     deploy_validate || exit 1
#
#     # Then sync to etcd
#     sync_configs_to_etcd
# }
```

### 8.2 Pre-Commit Hook Integration

```
ALGORITHM: pre_commit_validate
# Location: .git/hooks/pre-commit or via pre-commit framework

BEGIN
    // Find changed config files
    changed_files <- GIT_DIFF_CACHED("--name-only", "--diff-filter=ACM")
    config_files <- FILTER(changed_files, f -> f MATCHES "config/base/streams/*/config.json")

    IF LENGTH(config_files) == 0 THEN
        // No config changes, skip validation
        RETURN 0
    END IF

    PRINT("Validating changed config files...")

    // Validate each changed config (schema-only for speed)
    FOR EACH config_file IN config_files DO
        exit_code <- EXECUTE("ndp-validate", ["--schema-only", config_file])

        IF exit_code != 0 THEN
            PRINT_ERROR("Validation failed for: " + config_file)
            PRINT_ERROR("Commit aborted. Fix errors and try again.")
            RETURN 1
        END IF
    END FOR

    PRINT_SUCCESS("Config validation passed")
    RETURN 0
END
```

---

## 9. Runtime Startup Validation

### 9.1 Application Startup Validation

```
ALGORITHM: startup_validation
INPUT:
    app_config: AppConfig           # Application configuration
    db_pool: PgPool                 # Database connection pool
OUTPUT:
    Result<(), StartupError>

# Location: apps/air-quality-app/src/startup.rs

BEGIN
    // Load validator (can be cached/singleton)
    validator <- Validator::with_database(SCHEMA_PATH, db_pool)

    IF validator IS error THEN
        LOG_ERROR("Failed to initialize validator: " + validator.error)
        IF app_config.strict_validation THEN
            RETURN Err(StartupError::ValidatorInit(validator.error))
        END IF
        LOG_WARN("Proceeding without validation (strict_validation=false)")
        RETURN Ok(())
    END IF

    // Get enabled streams from registry
    enabled_streams <- AWAIT stream_registry.list_enabled_streams()

    FOR EACH stream_id IN enabled_streams DO
        // Load stream config from etcd
        stream_config <- AWAIT stream_registry.load_stream(stream_id)

        IF stream_config IS error THEN
            LOG_ERROR("Failed to load config for stream: " + stream_id)
            IF app_config.strict_validation THEN
                RETURN Err(StartupError::ConfigLoad(stream_id, stream_config.error))
            END IF
            CONTINUE
        END IF

        // Validate the config
        result <- AWAIT validator.validate_config(stream_config)

        IF NOT result.valid THEN
            LOG_ERROR("Config validation failed for stream: " + stream_id)
            FOR EACH error IN result.errors DO
                LOG_ERROR("  [{layer}] {path}: {message}",
                    layer=error.layer, path=error.path, message=error.message)
            END FOR

            IF app_config.strict_validation THEN
                RETURN Err(StartupError::ValidationFailed {
                    stream_id: stream_id,
                    errors: result.errors
                })
            END IF

            LOG_WARN("Disabling stream {} due to validation errors", stream_id)
            AWAIT stream_registry.disable_stream(stream_id)
        ELSE
            LOG_INFO("Config validation passed for stream: " + stream_id)

            // Log warnings if any
            FOR EACH warning IN result.warnings DO
                LOG_WARN("  [{layer}] {path}: {message}",
                    layer=warning.layer, path=warning.path, message=warning.message)
            END FOR
        END IF
    END FOR

    RETURN Ok(())
END
```

---

## 10. Error Code Reference

### 10.1 Complete Error Code Enumeration

```
ENUM ErrorCode:
    // Syntax Layer (100-199)
    SYNTAX_ERROR = 100              # Malformed JSON
    FILE_NOT_FOUND = 101            # Config file not readable

    // Schema Layer (200-299)
    MISSING_REQUIRED = 200          # Required field missing
    INVALID_TYPE = 201              # Wrong JSON type
    UNKNOWN_FIELD = 202             # additionalProperties violation
    PATTERN_MISMATCH = 203          # Regex pattern not matched
    ENUM_VIOLATION = 204            # Value not in enum
    ARRAY_BOUNDS = 205              # minItems/maxItems violation
    VALUE_OUT_OF_RANGE = 206        # minimum/maximum violation
    DESERIALIZATION_FAILED = 207    # Serde deserialization error
    SCHEMA_VIOLATION = 299          # Generic schema error

    // Semantic Layer - Types (300-319)
    INVALID_FIELD_TYPE = 300        # Field type not NDP-supported
    INVALID_SOURCE_TYPE = 301       # Source type not supported
    INVALID_RANGE = 302             # Range on non-numeric type
    INVALID_PRECISION = 303         # Precision on non-float type

    // Semantic Layer - Cross-Reference (320-339)
    INVALID_SOURCE_PATH = 320       # source_path refs unknown field
    DUPLICATE_NAME = 321            # Duplicate field/rule name
    CONSTRAINT_VIOLATION = 322      # retention vs compression

    // Semantic Layer - External (340-359)
    TABLE_NOT_FOUND = 340           # Silver table doesn't exist
    COLUMN_NOT_FOUND = 341          # Target column doesn't exist
    TYPE_MISMATCH = 342             # Config type != DB type
    TABLE_CHECK_FAILED = 343        # Could not query database
    COLUMN_CHECK_FAILED = 344       # Could not verify columns

    // Semantic Layer - Source Config (360-379)
    MISSING_SOURCE_CONFIG = 360     # Required source field missing
    INVALID_SOURCE_CONFIG = 361     # Invalid source configuration

    // Semantic Layer - DQ Rules (380-399)
    INVALID_DQ_RULE_TYPE = 380      # Unknown DQ rule type
    INVALID_DQ_RULE = 381           # Rule-specific validation failure
    INVALID_DQ_ACTION = 382         # Action not valid for rule type
    INVALID_DQ_COLUMN = 383         # DQ rule refs unknown column
    INVALID_DQ_SYNTAX = 384         # Invalid SQL expression
    INVALID_REGEX = 385             # Invalid regex pattern
    INVALID_INTERVAL = 386          # Invalid interval format

    // Warnings (900-999)
    UNKNOWN_DEVICE_CLASS = 900      # device_class not recognized
```

---

## 11. Algorithm Summary and Complexity

### 11.1 Overall Complexity

| Component | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| File I/O | O(n) | O(n) |
| JSON Parsing | O(n) | O(n) |
| Schema Validation | O(n * s) | O(e) |
| Field Type Validation | O(f) | O(1) |
| Source Validation | O(s) | O(1) |
| Source Path Validation | O(m * f) | O(f) |
| Table Existence Check | O(1) | O(1) (per query) |
| Column Compatibility | O(m) | O(c) |
| DQ Rule Validation | O(r * (e + c)) | O(r) |
| **Total** | O(n * s + m * f + r * e) | O(n + f + m + r + e) |

Where:
- n = config file size
- s = schema size
- f = field count
- m = field_mappings count
- r = DQ rule count
- c = column count
- e = error count

### 11.2 Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Schema-only validation | < 100ms | No database queries |
| Full validation (local DB) | < 500ms | Includes table/column checks |
| Memory usage | < 50MB | For typical config (< 100 fields) |

---

## 12. Design Patterns Used

### 12.1 Accumulator Pattern (Error Collection)

```
// Instead of fail-fast:
IF error THEN RETURN error

// Use accumulator:
errors <- []
FOR EACH validation DO
    IF fails THEN
        errors <- errors + [new_error]
    END IF
END FOR
RETURN errors  // Return all errors at once
```

### 12.2 Strategy Pattern (Validation Layers)

```
INTERFACE ValidationStrategy:
    validate(config) -> Array<ValidationError>

CLASS SchemaValidation IMPLEMENTS ValidationStrategy:
    validate(config) -> // JSON Schema checks

CLASS SemanticValidation IMPLEMENTS ValidationStrategy:
    validate(config) -> // Application rules

CLASS Validator:
    strategies: Array<ValidationStrategy>

    validate(config):
        errors <- []
        FOR EACH strategy IN strategies DO
            errors <- errors + strategy.validate(config)
        END FOR
        RETURN errors
```

### 12.3 Builder Pattern (ValidationOptions)

```
options <- ValidationOptions::default()
    .schema_only(false)
    .check_tables(true)
    .check_source_paths(true)
    .strict(false)
    .build()
```

---

## 13. References

| Document | Purpose |
|----------|---------|
| `dp-019/specification/SPECIFICATION.md` | Requirements and acceptance criteria |
| `dp-019/specification/SUPPORTED-VALUES-RESEARCH.md` | Valid enum values |
| `dp-019/architecture/VALIDATION-ARCHITECTURE.md` | Two-layer architecture design |
| `dp-019/specification/DQ-VALIDATION-RESEARCH.md` | DQ rule validation rules |
| `schemas/stream-config.v1.1.schema.json` | JSON Schema reference |
| `core/src/types/stream_config.rs` | Rust type definitions |
| `core/src/config/silver_etl.rs` | Silver ETL configuration |

---

*Pseudocode created: 2026-02-02*
*SPARC Phase: Pseudocode (P)*
*Next Phase: Architecture (A) - Already complete*
*Then: Refinement (R) - TDD Implementation*
