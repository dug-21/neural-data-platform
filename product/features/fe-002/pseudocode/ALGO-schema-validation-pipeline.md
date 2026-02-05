# ALGO-003: Schema Validation Pipeline for Domain Configs

## Overview

Algorithm for adding domain configuration validation to `ndp-validate`, implementing the two-layer validation pattern (dp-019) for domain configs. This resolves GAP-003 from FE-001 Phase D.

**Feature:** FE-002 Domain Configuration Standardization
**Phase:** Pseudocode (SPARC P)
**ADR Reference:** dp-019 Two-Layer Validation

---

## Two-Layer Validation Architecture

```
                    ┌────────────────────────────────────┐
                    │         ndp-validate CLI           │
                    │    --domain <path> | --all-domains │
                    └──────────────┬─────────────────────┘
                                   │
           ┌───────────────────────┼───────────────────────┐
           │                       │                       │
           ▼                       ▼                       ▼
    ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
    │   Layer 0    │       │   Layer 1    │       │   Layer 2    │
    │   Syntax     │──────▶│   Schema     │──────▶│   Semantic   │
    │  (JSON/YAML) │       │  (JSON Sch.) │       │   (Rust)     │
    └──────────────┘       └──────────────┘       └──────────────┘
           │                       │                       │
           ▼                       ▼                       ▼
    Parse errors            Structural errors      Business logic
    Line/column             JSONPath location      Domain rules
                            Type mismatches        Stream references
```

---

## Algorithm Specification

### ALGORITHM: ValidateDomainConfig

```
ALGORITHM: ValidateDomainConfig
INPUT:
    config_path: Path to domain.json file
    options: ValidationOptions {
        schema_only: Boolean (skip Layer 2)
        check_streams: Boolean (verify stream references)
        strict: Boolean (warnings as errors)
        verbose: Boolean (progress output)
    }
OUTPUT:
    result: ValidationResult {
        valid: Boolean
        errors: Array<ValidationError>
        warnings: Array<ValidationError>
        summary: ValidationSummary
    }

PRECONDITIONS:
    - config_path exists and is readable
    - domain.schema.json is available at known location
    - If check_streams: stream configs are accessible

POSTCONDITIONS:
    - result.valid == TRUE iff no errors (or warnings in strict mode)
    - All errors have proper JSONPath locations
    - Exit code follows dp-019 convention (0/1/2)

COMPLEXITY:
    Time: O(n + s * m) where n = config size, s = streams, m = available streams
    Space: O(n + e) where e = error count
```

---

## Main Validation Flow

```
BEGIN ValidateDomainConfig(config_path, options):

    result <- ValidationResult.new(config_path)

    // ========================================
    // LAYER 0: Syntax Validation
    // ========================================

    IF options.verbose THEN:
        Log("Layer 0: Syntax validation...")
    END IF

    TRY:
        content <- ReadFile(config_path)
    CATCH FileNotFound:
        result.add_error(SystemError("File not found: " + config_path))
        RETURN result
    END TRY

    // Detect format and parse
    IF EndsWith(config_path, ".json") THEN:
        TRY:
            json_value <- ParseJson(content)
        CATCH JsonSyntaxError(line, column, message):
            result.add_error(SyntaxError(
                path="$",
                line=line,
                column=column,
                message="JSON parse error: " + message
            ))
            RETURN result  // Cannot continue without valid JSON
        END TRY
    ELSE IF EndsWith(config_path, ".yaml") OR EndsWith(config_path, ".yml") THEN:
        TRY:
            json_value <- ParseYamlToJson(content)
        CATCH YamlSyntaxError(line, column, message):
            result.add_error(SyntaxError(
                path="$",
                line=line,
                column=column,
                message="YAML parse error: " + message
            ))
            RETURN result
        END TRY
    ELSE:
        result.add_error(SystemError("Unknown file format: " + config_path))
        RETURN result
    END IF

    // ========================================
    // LAYER 1: Schema Validation
    // ========================================

    IF options.verbose THEN:
        Log("Layer 1: Schema validation...")
    END IF

    schema <- LoadDomainSchema()
    schema_errors <- ValidateAgainstSchema(json_value, schema)

    FOR EACH error IN schema_errors:
        result.add_error(SchemaError(
            path=error.path,
            message=error.message,
            suggestion=GenerateSchemaSuggestion(error)
        ))
    END FOR

    // Stop if schema validation fails (cannot trust structure for Layer 2)
    IF result.has_errors() THEN:
        IF options.verbose THEN:
            Log("Layer 1 failed with " + result.error_count() + " errors")
            Log("Skipping Layer 2 (semantic validation)")
        END IF
        RETURN result
    END IF

    // ========================================
    // LAYER 2: Semantic Validation
    // ========================================

    IF options.schema_only THEN:
        IF options.verbose THEN:
            Log("Layer 2: Skipped (--schema-only)")
        END IF
        RETURN result
    END IF

    IF options.verbose THEN:
        Log("Layer 2: Semantic validation...")
    END IF

    // Load available streams for reference validation
    available_streams <- SET()
    IF options.check_streams THEN:
        available_streams <- DiscoverAvailableStreams(options.config_dir)
    ELSE:
        // Use streams from config directory (default behavior)
        available_streams <- DiscoverAvailableStreams(DEFAULT_CONFIG_DIR)
    END IF

    semantic_errors <- ValidateDomainSemantics(json_value, available_streams)

    FOR EACH error IN semantic_errors:
        IF error.severity == WARNING THEN:
            result.add_warning(error)
        ELSE:
            result.add_error(error)
        END IF
    END FOR

    // ========================================
    // Final Result
    // ========================================

    IF options.verbose THEN:
        Log("Validation complete: " + result.error_count() + " errors, " +
            result.warning_count() + " warnings")
    END IF

    RETURN result

END ValidateDomainConfig
```

---

## Subroutine: Schema Loading

```
SUBROUTINE: LoadDomainSchema()
OUTPUT: Compiled JSON Schema validator

BEGIN:
    // Schema location priority:
    // 1. --schema-path CLI argument
    // 2. config/schemas/domain.schema.json (relative)
    // 3. /opt/ndp/config/schemas/domain.schema.json (Pi deployment)
    // 4. Embedded schema (compiled in binary)

    schema_paths <- [
        GetCliOption("schema-path"),
        "./config/schemas/domain.schema.json",
        "/opt/ndp/config/schemas/domain.schema.json"
    ]

    FOR EACH path IN schema_paths:
        IF path IS NOT NULL AND FileExists(path) THEN:
            TRY:
                schema_content <- ReadFile(path)
                schema <- ParseJson(schema_content)
                RETURN CompileSchema(schema, draft="draft-07")
            CATCH:
                CONTINUE  // Try next path
            END TRY
        END IF
    END FOR

    // Fallback: use embedded schema
    RETURN CompileEmbeddedSchema(DOMAIN_SCHEMA_JSON)

END LoadDomainSchema
```

---

## Subroutine: Schema Validation

```
SUBROUTINE: ValidateAgainstSchema(json_value, schema)
INPUT:
    json_value: Parsed JSON document
    schema: Compiled JSON Schema
OUTPUT:
    Array of schema validation errors

BEGIN:
    errors <- []

    validator <- CreateValidator(schema)

    FOR EACH error IN validator.iter_errors(json_value):
        errors.APPEND(SchemaValidationError(
            path=FormatJsonPath(error.absolute_path),
            message=error.message,
            validator_type=error.validator,
            schema_path=FormatSchemaPath(error.schema_path),
            expected=ExtractExpected(error),
            actual=ExtractActual(error, json_value)
        ))
    END FOR

    // Sort errors by path for consistent output
    errors <- SortByPath(errors)

    RETURN errors

END ValidateAgainstSchema


SUBROUTINE: FormatJsonPath(path_elements)
INPUT: Array of path elements (strings and integers)
OUTPUT: JSONPath string (e.g., "$.domain.streams[0].stream_id")

BEGIN:
    IF path_elements IS EMPTY THEN:
        RETURN "$"
    END IF

    result <- "$"

    FOR EACH element IN path_elements:
        IF IsInteger(element) THEN:
            result <- result + "[" + element + "]"
        ELSE:
            result <- result + "." + element
        END IF
    END FOR

    RETURN result

END FormatJsonPath


SUBROUTINE: GenerateSchemaSuggestion(error)
INPUT: Schema validation error
OUTPUT: Actionable suggestion string or NULL

BEGIN:
    // Pattern: Missing required field
    IF error.validator_type == "required" THEN:
        missing_field <- ExtractMissingField(error.message)
        RETURN "Add the required field '" + missing_field + "'"
    END IF

    // Pattern: Type mismatch
    IF error.validator_type == "type" THEN:
        expected_type <- error.expected
        RETURN "Expected type '" + expected_type + "'"
    END IF

    // Pattern: Enum violation
    IF error.validator_type == "enum" THEN:
        valid_values <- error.expected
        RETURN "Valid values: " + Join(valid_values, ", ")
    END IF

    // Pattern: Pattern mismatch
    IF error.validator_type == "pattern" THEN:
        pattern <- error.expected
        IF pattern == "^[a-z][a-z0-9-]*$" THEN:
            RETURN "Use kebab-case (lowercase letters, numbers, hyphens)"
        ELSE IF pattern == "^[a-z][a-z0-9_]*$" THEN:
            RETURN "Use snake_case (lowercase letters, numbers, underscores)"
        ELSE IF pattern MATCHES "granularity" THEN:
            RETURN "Format: '<number> <unit>' (e.g., '1 hour', '15 minutes')"
        END IF
    END IF

    // Pattern: Additional properties
    IF error.validator_type == "additionalProperties" THEN:
        unknown_field <- ExtractUnknownField(error)
        closest <- FindClosestField(unknown_field, GetSchemaFields(error.schema_path))
        IF closest IS NOT NULL THEN:
            RETURN "Unknown field. Did you mean '" + closest + "'?"
        END IF
    END IF

    RETURN NULL

END GenerateSchemaSuggestion
```

---

## Subroutine: Semantic Validation

```
SUBROUTINE: ValidateDomainSemantics(json_value, available_streams)
INPUT:
    json_value: Parsed and schema-validated JSON
    available_streams: Set of valid stream IDs
OUTPUT:
    Array of semantic validation errors/warnings

BEGIN:
    errors <- []

    // Extract domain content
    domain <- json_value["domain"]

    // ========================================
    // Rule 1: Stream References Must Exist
    // ========================================

    streams <- domain["streams"]

    FOR idx, stream IN Enumerate(streams):
        stream_id <- stream["stream_id"]

        IF stream_id NOT IN available_streams THEN:
            closest <- FindClosestStream(stream_id, available_streams)

            errors.APPEND(SemanticError(
                code=ErrorCode.INVALID_DOMAIN_STREAM,
                path="$.domain.streams[" + idx + "].stream_id",
                message="Stream '" + stream_id + "' not found. " +
                        "Available: " + FormatStreamList(available_streams),
                severity=ERROR,
                suggestion=closest
            ))
        END IF
    END FOR

    // ========================================
    // Rule 2: Exactly One Primary Stream
    // ========================================

    primary_count <- CountWhere(streams, s => s["role"] == "primary")

    IF primary_count == 0 THEN:
        errors.APPEND(SemanticError(
            code=ErrorCode.INVALID_DOMAIN_STREAM,
            path="$.domain.streams",
            message="Domain must have exactly one stream with role 'primary'",
            severity=WARNING,
            suggestion="Add role: 'primary' to the main stream being optimized"
        ))
    ELSE IF primary_count > 1 THEN:
        errors.APPEND(SemanticError(
            code=ErrorCode.INVALID_DOMAIN_STREAM,
            path="$.domain.streams",
            message="Domain has " + primary_count + " primary streams; only one allowed",
            severity=ERROR,
            suggestion="Designate only one stream as 'primary'"
        ))
    END IF

    // ========================================
    // Rule 3: Unique Aliases
    // ========================================

    seen_aliases <- MAP()

    FOR idx, stream IN Enumerate(streams):
        alias <- stream["alias"] OR stream["stream_id"]

        IF alias IN seen_aliases THEN:
            first_idx <- seen_aliases[alias]
            errors.APPEND(SemanticError(
                code=ErrorCode.DUPLICATE_NAME,
                path="$.domain.streams[" + idx + "].alias",
                message="Duplicate alias '" + alias + "' (first used at streams[" +
                        first_idx + "])",
                severity=ERROR,
                suggestion="Each stream must have a unique alias"
            ))
        ELSE:
            seen_aliases[alias] <- idx
        END IF
    END FOR

    // ========================================
    // Rule 4: Objective Streams Must Be in Domain
    // ========================================

    domain_stream_ids <- SET(s["stream_id"] FOR s IN streams)

    IF domain HAS KEY "objectives" THEN:
        FOR idx, objective IN Enumerate(domain["objectives"]):
            target_stream <- objective["target"]["stream"]

            IF target_stream NOT IN domain_stream_ids THEN:
                errors.APPEND(SemanticError(
                    code=ErrorCode.INVALID_DOMAIN_STREAM,
                    path="$.domain.objectives[" + idx + "].target.stream",
                    message="Objective references stream '" + target_stream +
                            "' which is not in this domain",
                    severity=ERROR,
                    suggestion="Add stream to domain.streams or use existing stream"
                ))
            END IF
        END FOR
    END IF

    // ========================================
    // Rule 5: Unique Objective IDs
    // ========================================

    IF domain HAS KEY "objectives" THEN:
        seen_obj_ids <- SET()

        FOR idx, objective IN Enumerate(domain["objectives"]):
            obj_id <- objective["id"]

            IF obj_id IN seen_obj_ids THEN:
                errors.APPEND(SemanticError(
                    code=ErrorCode.DUPLICATE_NAME,
                    path="$.domain.objectives[" + idx + "].id",
                    message="Duplicate objective ID '" + obj_id + "'",
                    severity=ERROR,
                    suggestion=NULL
                ))
            ELSE:
                seen_obj_ids.ADD(obj_id)
            END IF
        END FOR
    END IF

    // ========================================
    // Rule 6: Valid Granularity Format
    // ========================================

    granularity <- domain["alignment"]["granularity"]

    IF NOT MatchesGranularityPattern(granularity) THEN:
        errors.APPEND(SemanticError(
            code=ErrorCode.INVALID_GRANULARITY,
            path="$.domain.alignment.granularity",
            message="Invalid granularity format: '" + granularity + "'",
            severity=ERROR,
            suggestion="Use format: '<number> <unit>' (e.g., '1 hour', '15 minutes')"
        ))
    END IF

    // ========================================
    // Rule 7: Constraint Stream References (if present)
    // ========================================

    IF domain HAS KEY "constraints" THEN:
        FOR idx, constraint IN Enumerate(domain["constraints"]):
            IF constraint HAS KEY "condition" THEN:
                cond_stream <- constraint["condition"]["stream"]

                IF cond_stream NOT IN domain_stream_ids THEN:
                    errors.APPEND(SemanticError(
                        code=ErrorCode.INVALID_DOMAIN_STREAM,
                        path="$.domain.constraints[" + idx + "].condition.stream",
                        message="Constraint references stream '" + cond_stream +
                                "' which is not in this domain",
                        severity=ERROR,
                        suggestion=NULL
                    ))
                END IF
            END IF
        END FOR
    END IF

    RETURN errors

END ValidateDomainSemantics


SUBROUTINE: MatchesGranularityPattern(granularity)
INPUT: Granularity string
OUTPUT: Boolean

BEGIN:
    // Pattern: <number> <unit>(s)
    // Valid units: minute, hour, day
    pattern <- "^\\d+\\s+(minute|hour|day)s?$"
    RETURN RegexMatch(granularity, pattern)

END MatchesGranularityPattern


SUBROUTINE: FindClosestStream(input, candidates)
INPUT:
    input: User-provided stream ID
    candidates: Set of valid stream IDs
OUTPUT:
    Suggestion string or NULL

BEGIN:
    input_lower <- ToLowerCase(input)
    best_match <- NULL
    best_distance <- INFINITY

    FOR EACH candidate IN candidates:
        distance <- LevenshteinDistance(input_lower, ToLowerCase(candidate))

        IF distance <= 3 AND distance < best_distance THEN:
            best_match <- candidate
            best_distance <- distance
        END IF
    END FOR

    IF best_match IS NOT NULL THEN:
        RETURN "Did you mean '" + best_match + "'?"
    END IF

    RETURN NULL

END FindClosestStream
```

---

## Subroutine: Stream Discovery

```
SUBROUTINE: DiscoverAvailableStreams(config_dir)
INPUT: Base configuration directory
OUTPUT: Set of valid stream IDs

BEGIN:
    streams <- SET()

    streams_dir <- config_dir + "/base/streams"

    IF NOT DirectoryExists(streams_dir) THEN:
        Log("WARNING: Streams directory not found: " + streams_dir)
        RETURN streams
    END IF

    // Iterate over stream directories
    FOR EACH entry IN ListDirectory(streams_dir):
        IF IsDirectory(entry) THEN:
            config_path <- entry + "/config.json"

            IF FileExists(config_path) THEN:
                TRY:
                    content <- ReadFile(config_path)
                    config <- ParseJson(content)
                    stream_id <- config["stream_id"]

                    IF stream_id IS NOT NULL THEN:
                        streams.ADD(stream_id)
                    END IF
                CATCH:
                    Log("WARNING: Could not read stream config: " + config_path)
                END TRY
            END IF
        END IF
    END FOR

    RETURN streams

END DiscoverAvailableStreams
```

---

## CLI Extension

### New CLI Arguments

```
STRUCT: ExtendedCli (extends current Cli)

    // Existing stream validation options...

    // NEW: Domain validation options
    #[arg(long, conflicts_with = "config_path")]
    pub domain: Option<PathBuf>
        // Path to domain.json file to validate

    #[arg(long, conflicts_with = "config_path")]
    pub all_domains: bool
        // Validate all domain configs in domains/ directory

    #[arg(long, default_value = "config/schemas/domain.schema.json")]
    pub domain_schema_path: PathBuf
        // Path to domain JSON Schema

    #[arg(long)]
    pub check_streams: bool
        // Verify that referenced streams exist (default: true for domains)

END STRUCT
```

### Updated Argument Validation

```
SUBROUTINE: ValidateCliArgs(cli)
INPUT: Parsed CLI arguments
OUTPUT: Result<(), String>

BEGIN:
    // Existing validation...

    // NEW: Domain validation rules

    // --domain and --all-domains are mutually exclusive
    IF cli.domain IS NOT NULL AND cli.all_domains THEN:
        RETURN Error("Cannot use --domain with --all-domains")
    END IF

    // --domain requires a path
    IF cli.domain IS NOT NULL AND NOT FileExists(cli.domain) THEN:
        RETURN Error("Domain config not found: " + cli.domain)
    END IF

    // --domain-schema-path must exist if specified
    IF NOT FileExists(cli.domain_schema_path) THEN:
        RETURN Error("Domain schema not found: " + cli.domain_schema_path)
    END IF

    RETURN Ok(())

END ValidateCliArgs
```

---

## Main Entry Point Update

```
SUBROUTINE: UpdateMain(cli)

BEGIN:
    // ... existing stream validation code ...

    // NEW: Domain validation mode
    IF cli.domain IS NOT NULL THEN:
        // Single domain validation
        result <- ValidateDomainConfig(cli.domain, ValidationOptions{
            schema_only: cli.schema_only,
            check_streams: cli.check_streams,
            strict: cli.strict,
            verbose: cli.verbose
        })

        OutputResult(result, cli.format)
        EXIT(DetermineExitCode(result, cli.strict))

    ELSE IF cli.all_domains THEN:
        // Batch domain validation
        domains_dir <- cli.config_dir + "/domains"
        results <- []

        FOR EACH domain_dir IN ListDirectories(domains_dir):
            domain_json <- domain_dir + "/domain.json"

            IF FileExists(domain_json) THEN:
                result <- ValidateDomainConfig(domain_json, options)
                results.APPEND(result)
            END IF
        END FOR

        batch_result <- BatchValidationResult.from_results(results)
        OutputBatchResult(batch_result, cli.format)
        EXIT(DetermineBatchExitCode(batch_result, cli.strict))

    END IF

    // ... existing stream validation code ...

END UpdateMain
```

---

## Error Aggregation and Formatting

```
SUBROUTINE: FormatValidationErrors(errors, format)
INPUT:
    errors: Array of ValidationError
    format: OutputFormat (JSON | HUMAN)
OUTPUT:
    Formatted string

BEGIN:
    IF format == JSON THEN:
        RETURN SerializeJson({
            "valid": errors IS EMPTY,
            "error_count": Length(errors),
            "errors": errors
        }, indent=2)

    ELSE IF format == HUMAN THEN:
        output <- StringBuilder()

        // Group errors by layer
        by_layer <- GroupBy(errors, e => e.layer)

        FOR EACH layer, layer_errors IN by_layer:
            output.AppendLine("")
            output.AppendLine(ToUpperCase(layer) + " ERRORS:")

            FOR EACH error IN layer_errors:
                output.AppendLine("  " + ColorRed("[" + error.code + "]") +
                                  " " + error.path)
                output.AppendLine("    " + error.message)

                IF error.suggestion IS NOT NULL THEN:
                    output.AppendLine("    " + ColorYellow("Suggestion: ") +
                                      error.suggestion)
                END IF
            END FOR
        END FOR

        RETURN output.ToString()

    END IF

END FormatValidationErrors
```

---

## Exit Code Determination

```
SUBROUTINE: DetermineExitCode(result, strict)
INPUT:
    result: ValidationResult
    strict: Boolean (treat warnings as errors)
OUTPUT:
    Integer exit code (0, 1, or 2)

BEGIN:
    // dp-019 Exit Codes:
    // 0 = Success (valid, may have warnings)
    // 1 = Validation Error (has errors)
    // 2 = System Error (file not found, etc.)

    IF result.has_system_errors() THEN:
        RETURN 2
    END IF

    IF result.has_errors() THEN:
        RETURN 1
    END IF

    IF strict AND result.has_warnings() THEN:
        RETURN 1
    END IF

    RETURN 0

END DetermineExitCode
```

---

## Complexity Analysis

| Component | Time Complexity | Space Complexity | Notes |
|-----------|-----------------|------------------|-------|
| File Read | O(n) | O(n) | n = file size |
| JSON Parse | O(n) | O(n) | DOM construction |
| Schema Validation | O(n * s) | O(s) | s = schema size |
| Semantic Validation | O(m * k) | O(m) | m = streams, k = available |
| Stream Discovery | O(d) | O(d) | d = stream count |
| Error Formatting | O(e) | O(e) | e = error count |

**Total: O(n * s + m * k + d)**
**Space: O(n + s + d + e)**

---

## Integration Points

### Existing Code to Modify

1. **`cli.rs`**: Add `--domain`, `--all-domains`, `--domain-schema-path` arguments
2. **`main.rs`**: Add domain validation flow before/after stream validation
3. **`schema.rs`**: Add `load_domain_schema()` function
4. **`semantic/mod.rs`**: Export domain validation module
5. **`lib.rs`**: Add domain validation to public API

### New Code to Create

1. **`semantic/domain.rs`**: Already exists (FE-001), just needs CLI wiring
2. **`schema/domain_schema.rs`**: Domain schema loading (mirrors stream schema)

---

## References

- **dp-019**: Two-Layer Validation Pattern
- **FE-002 SCOPE.md**: Feature specification (AC-B1 through AC-B8)
- **domain.schema.json**: JSON Schema for domain configs
- **semantic/domain.rs**: Existing semantic validation rules
- **cli.rs**: Current CLI implementation
