# ALGO-gold-ddl-cli: CLI Entry Point and Command Routing

> **Algorithm ID:** A01
> **Feature:** v11-A02 (Gold DDL Tool)
> **Created:** 2026-02-04

---

## Purpose

The `ndp-gold-ddl` CLI is the entry point for all Gold layer DDL generation. It parses commands, loads configuration, validates inputs, and delegates to appropriate generators.

---

## Algorithm: Main Entry Point

```
ALGORITHM: MainEntryPoint
INPUT: command_line_args (Vec<String>)
OUTPUT: exit_code (0 = success, 1 = validation error, 2 = system error)
REQUIRES: Valid CLI arguments

BEGIN
    // 1. Initialize tracing/logging
    initialize_tracing(verbosity_level)

    // 2. Parse CLI arguments using clap
    cli <- Cli::parse_from(command_line_args)

    // 3. Create config loader based on options
    config_loader <- create_config_loader(cli.config_dir, cli.etcd_endpoint)

    // 4. Route to appropriate command handler
    result <- MATCH cli.command WITH
        | Generate { stream, domain, action } =>
            handle_generate(config_loader, stream, domain, action)
        | Validate { stream, domain } =>
            handle_validate(config_loader, stream, domain)
        | Version =>
            print_version()

    // 5. Handle result and determine exit code
    MATCH result WITH
        | Ok(output) =>
            write_to_stdout(output)
            RETURN 0
        | Err(ValidationError) =>
            write_to_stderr(error_message)
            RETURN 1
        | Err(SystemError) =>
            write_to_stderr(error_message)
            RETURN 2
END
```

---

## Algorithm: Create Config Loader

```
ALGORITHM: CreateConfigLoader
INPUT:
    config_dir: Option<PathBuf>
    etcd_endpoint: Option<String>
OUTPUT: Box<dyn ConfigLoader>
REQUIRES: At least one source specified

BEGIN
    // 1. Try etcd first if endpoint provided
    IF etcd_endpoint IS Some(endpoint) THEN
        TRY
            client <- EtcdClient::connect(endpoint)
            RETURN Box::new(EtcdConfigLoader::new(client))
        CATCH ConnectionError =>
            log_warning("etcd unavailable, falling back to filesystem")
        END TRY
    END IF

    // 2. Fall back to filesystem
    dir <- config_dir.unwrap_or(default_config_dir())

    IF NOT dir.exists() THEN
        RETURN Err(SystemError::ConfigDirNotFound(dir))
    END IF

    RETURN Box::new(FileSystemConfigLoader::new(dir))
END
```

---

## Algorithm: Handle Generate Command

```
ALGORITHM: HandleGenerate
INPUT:
    config_loader: Box<dyn ConfigLoader>
    stream: Option<String>
    domain: Option<String>
    action: Action (sync | recreate)
OUTPUT: Result<String, GeneratorError>
REQUIRES: Exactly one of stream or domain is Some

BEGIN
    // 1. Validate mutually exclusive options
    IF stream.is_some() AND domain.is_some() THEN
        RETURN Err(GeneratorError::MutuallyExclusive("stream", "domain"))
    END IF

    IF stream.is_none() AND domain.is_none() THEN
        RETURN Err(GeneratorError::MissingRequired("stream or domain"))
    END IF

    // 2. Route to appropriate generator
    IF stream IS Some(stream_id) THEN
        RETURN generate_stream_ddl(config_loader, stream_id, action)
    ELSE IF domain IS Some(domain_id) THEN
        RETURN generate_domain_ddl(config_loader, domain_id, action)
    END IF
END
```

---

## Algorithm: Generate Stream DDL

```
ALGORITHM: GenerateStreamDdl
INPUT:
    config_loader: Box<dyn ConfigLoader>
    stream_id: String
    action: Action
OUTPUT: Result<String, GeneratorError>
REQUIRES: Stream exists and has gold_etl config

BEGIN
    // 1. Load stream configuration
    stream_config <- config_loader.load_stream_config(stream_id)?

    // 2. Extract gold_etl configuration
    gold_etl <- stream_config.gold_etl
        .ok_or(GeneratorError::NoGoldConfig(stream_id))?

    // 3. Validate gold_etl is enabled
    IF NOT gold_etl.enabled THEN
        RETURN Err(GeneratorError::GoldDisabled(stream_id))
    END IF

    // 4. Run pre-generation validation
    validation_errors <- validate_gold_config(gold_etl, stream_config.fields)
    IF NOT validation_errors.is_empty() THEN
        RETURN Err(GeneratorError::ValidationFailed(validation_errors))
    END IF

    // 5. Create SQL output builder
    output <- StringWriter::new()

    // 6. Write header comment
    output.write_comment(format!("Gold DDL for stream: {}", stream_id))
    output.write_comment(format!("Generated: {}", current_timestamp()))
    output.write_comment(format!("Action: {}", action))
    output.write_blank_line()

    // 7. Write schema creation
    output.write_statement("CREATE SCHEMA IF NOT EXISTS gold;")
    output.write_blank_line()

    // 8. Generate continuous aggregates for each granularity
    agg_generator <- ContinuousAggregateGenerator::new()
    feature_registry <- FeatureRegistry::default()

    FOR EACH granularity IN gold_etl.aggregates.granularities DO
        // 8a. Generate the view DDL
        view_sql <- agg_generator.generate(
            stream_id,
            gold_etl,
            stream_config.silver_etl.target_table,
            granularity,
            action,
            feature_registry
        )?

        output.write_statement(view_sql)
        output.write_blank_line()

        // 8b. Generate refresh policy
        policy_sql <- agg_generator.generate_refresh_policy(
            stream_id,
            granularity,
            gold_etl.aggregates.refresh_interval,
            gold_etl.aggregates.start_offset,
            gold_etl.aggregates.end_offset
        )?

        output.write_statement(policy_sql)
        output.write_blank_line()
    END FOR

    // 9. Return complete DDL
    RETURN output.finish()
END
```

---

## Algorithm: Generate Domain DDL

```
ALGORITHM: GenerateDomainDdl
INPUT:
    config_loader: Box<dyn ConfigLoader>
    domain_id: String
    action: Action
OUTPUT: Result<String, GeneratorError>
REQUIRES: Domain exists and references valid streams

BEGIN
    // 1. Load domain configuration
    domain_config <- config_loader.load_domain_config(domain_id)?

    // 2. Validate all referenced streams exist and have Gold layer
    FOR EACH stream_ref IN domain_config.streams DO
        stream_config <- config_loader.load_stream_config(stream_ref.stream_id)?

        IF stream_config.gold_etl.is_none() THEN
            RETURN Err(GeneratorError::StreamNoGoldLayer(stream_ref.stream_id))
        END IF

        IF NOT stream_config.gold_etl.enabled THEN
            RETURN Err(GeneratorError::GoldDisabled(stream_ref.stream_id))
        END IF
    END FOR

    // 3. Create SQL output builder
    output <- StringWriter::new()

    // 4. Write header comment
    output.write_comment(format!("Domain DDL for: {}", domain_id))
    output.write_comment(format!("Streams: {}", domain_config.streams.join(", ")))
    output.write_comment(format!("Generated: {}", current_timestamp()))
    output.write_blank_line()

    // 5. Generate aligned view using AlignmentInterpreter
    aligned_gen <- AlignedViewGenerator::new(config_loader)

    aligned_sql <- aligned_gen.generate(domain_config, action)?
    output.write_statement(aligned_sql)
    output.write_blank_line()

    // 6. Generate index on bucket column
    index_sql <- format!(
        "CREATE INDEX IF NOT EXISTS idx_{}_bucket ON gold.{} (bucket);",
        domain_config.alignment.view_name,
        domain_config.alignment.view_name
    )
    output.write_statement(index_sql)
    output.write_blank_line()

    // 7. Add refresh comment (not automated for regular materialized views)
    output.write_comment("Refresh command (run manually or via scheduler):")
    output.write_comment(format!("REFRESH MATERIALIZED VIEW gold.{};",
        domain_config.alignment.view_name))

    RETURN output.finish()
END
```

---

## Algorithm: Handle Validate Command

```
ALGORITHM: HandleValidate
INPUT:
    config_loader: Box<dyn ConfigLoader>
    stream: Option<String>
    domain: Option<String>
OUTPUT: Result<(), ValidationError>
REQUIRES: At least one of stream or domain specified

BEGIN
    validation_errors <- Vec::new()

    // 1. Validate stream if specified
    IF stream IS Some(stream_id) THEN
        stream_errors <- validate_stream_gold_config(config_loader, stream_id)
        validation_errors.extend(stream_errors)
    END IF

    // 2. Validate domain if specified
    IF domain IS Some(domain_id) THEN
        domain_errors <- validate_domain_config(config_loader, domain_id)
        validation_errors.extend(domain_errors)
    END IF

    // 3. Report results
    IF validation_errors.is_empty() THEN
        print_success("Validation passed")
        RETURN Ok(())
    ELSE
        FOR EACH error IN validation_errors DO
            print_error(error)
        END FOR
        RETURN Err(ValidationError::Multiple(validation_errors))
    END IF
END
```

---

## Algorithm: Validate Stream Gold Config

```
ALGORITHM: ValidateStreamGoldConfig
INPUT:
    config_loader: Box<dyn ConfigLoader>
    stream_id: String
OUTPUT: Vec<ValidationError>

BEGIN
    errors <- Vec::new()

    // 1. Load stream config
    TRY
        stream_config <- config_loader.load_stream_config(stream_id)
    CATCH ConfigError::NotFound =>
        errors.push(ValidationError::StreamNotFound(stream_id))
        RETURN errors
    END TRY

    // 2. Check gold_etl exists
    IF stream_config.gold_etl.is_none() THEN
        errors.push(ValidationError::NoGoldConfig(stream_id))
        RETURN errors
    END IF

    gold_etl <- stream_config.gold_etl.unwrap()

    // 3. Check gold_etl.enabled
    IF NOT gold_etl.enabled THEN
        errors.push(ValidationError::GoldDisabled(stream_id))
        RETURN errors
    END IF

    // 4. Build set of valid field names from stream config
    valid_fields <- stream_config.fields
        .iter()
        .map(|f| f.name.clone())
        .collect::<HashSet<String>>()

    // 5. Validate aggregate field references
    IF gold_etl.aggregates IS Some(aggregates) THEN
        FOR EACH (field_name, metrics) IN aggregates.fields DO
            IF NOT valid_fields.contains(field_name) THEN
                errors.push(ValidationError::InvalidGoldField {
                    code: 400,
                    field: field_name,
                    stream: stream_id,
                    suggestion: find_similar_field(field_name, valid_fields)
                })
            END IF

            // Validate each metric is known
            FOR EACH metric IN metrics.metrics DO
                IF NOT is_valid_metric(metric) THEN
                    errors.push(ValidationError::InvalidAggregateMetric {
                        code: 403,
                        metric: metric,
                        valid_metrics: ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]
                    })
                END IF
            END FOR
        END FOR

        // Validate granularities
        FOR EACH granularity IN aggregates.granularities DO
            IF NOT is_valid_granularity(granularity) THEN
                errors.push(ValidationError::InvalidGranularity {
                    code: 406,
                    value: granularity,
                    valid_examples: ["1 hour", "1 day", "15 minutes"]
                })
            END IF
        END FOR
    END IF

    // 6. Validate feature field references
    IF gold_etl.features IS Some(features) THEN
        errors.extend(validate_feature_fields(features, valid_fields, stream_id))
    END IF

    // 7. Validate transitions config matches stream type
    IF gold_etl.transitions IS Some(transitions) THEN
        IF stream_config.stream_type != "state_event" THEN
            errors.push(ValidationError::InvalidStreamType {
                code: 401,
                expected: "state_event",
                actual: stream_config.stream_type,
                feature: "transitions"
            })
        END IF
    END IF

    RETURN errors
END
```

---

## Algorithm: Is Valid Metric

```
ALGORITHM: IsValidMetric
INPUT: metric: String
OUTPUT: boolean

CONSTANTS:
    VALID_METRICS = ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]

BEGIN
    RETURN VALID_METRICS.contains(metric.to_lowercase())
END
```

---

## Algorithm: Is Valid Granularity

```
ALGORITHM: IsValidGranularity
INPUT: granularity: String
OUTPUT: boolean

BEGIN
    // Parse the granularity string
    parts <- granularity.trim().split_whitespace().collect::<Vec<_>>()

    IF parts.len() != 2 THEN
        RETURN false
    END IF

    // First part must be a positive integer
    TRY
        value <- parts[0].parse::<u32>()
        IF value < 1 THEN
            RETURN false
        END IF
    CATCH ParseError =>
        RETURN false
    END TRY

    // Second part must be a valid time unit
    valid_units <- ["second", "seconds", "minute", "minutes",
                    "hour", "hours", "day", "days", "week", "weeks"]

    RETURN valid_units.contains(parts[1].to_lowercase())
END
```

---

## Algorithm: Granularity to Suffix

```
ALGORITHM: GranularityToSuffix
INPUT: granularity: String
OUTPUT: String (e.g., "hourly", "daily")

BEGIN
    parts <- granularity.trim().split_whitespace().collect::<Vec<_>>()
    value <- parts[0].parse::<u32>().unwrap_or(1)
    unit <- parts[1].to_lowercase()

    // Normalize unit to singular
    unit_singular <- MATCH unit WITH
        | "hours" => "hour"
        | "days" => "day"
        | "minutes" => "minute"
        | "weeks" => "week"
        | other => other

    // Generate suffix
    MATCH (value, unit_singular) WITH
        | (1, "hour") => RETURN "hourly"
        | (1, "day") => RETURN "daily"
        | (1, "week") => RETURN "weekly"
        | (15, "minute") => RETURN "15min"
        | (n, unit) => RETURN format!("{}{}", n, unit_singular)
END
```

---

## CLI Structure Definition

```
STRUCT Cli:
    // Global options
    verbose: u8                       // -v, -vv, -vvv
    config_dir: Option<PathBuf>       // --config-dir
    etcd_endpoint: Option<String>     // --etcd-endpoint

    // Subcommand
    command: Command

ENUM Command:
    // Generate DDL for stream or domain
    Generate {
        stream: Option<String>        // --stream <stream_id>
        domain: Option<String>        // --domain <domain_id>
        action: Action                // --action sync|recreate
    }

    // Validate configuration
    Validate {
        stream: Option<String>        // --stream <stream_id>
        domain: Option<String>        // --domain <domain_id>
    }

    // Print version
    Version

ENUM Action:
    Sync      // Create if not exists
    Recreate  // Drop and recreate
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| CLI parsing | O(n) where n = args | O(1) |
| Config loading (file) | O(f) where f = file size | O(f) |
| Config loading (etcd) | O(1) network round-trip | O(f) |
| Validation | O(f + m) fields + metrics | O(e) errors |
| DDL generation | See A02, A03 | See A02, A03 |

---

## Error Handling

All errors include:
1. **Error code** - Numeric code for programmatic handling
2. **Message** - Human-readable description
3. **Location** - Where in config the error occurred
4. **Suggestion** - How to fix (when applicable)

```
STRUCT ValidationError:
    code: u16
    message: String
    path: Option<String>      // JSON path like "gold_etl.aggregates.fields.nonexistent"
    suggestion: Option<String>
```

---

## Invariants

1. **Mutual Exclusivity**: `--stream` and `--domain` cannot both be specified
2. **Required Target**: At least one of `--stream` or `--domain` must be specified for generate/validate
3. **Gold Layer Required**: Stream must have `gold_etl.enabled = true` for generation
4. **Silver Source Required**: Stream must have `silver_etl.target_table` defined
5. **Valid References**: All field references in gold_etl must exist in stream.fields

---

## Test Cases (London TDD)

```
TEST: GenerateRequiresMutuallyExclusiveOptions
    GIVEN cli with both --stream and --domain
    WHEN handle_generate() is called
    THEN return Err(MutuallyExclusive)

TEST: GenerateRequiresAtLeastOneTarget
    GIVEN cli with neither --stream nor --domain
    WHEN handle_generate() is called
    THEN return Err(MissingRequired)

TEST: ValidateRejectsUnknownField
    GIVEN stream config with gold_etl.aggregates.fields.nonexistent
    AND nonexistent not in stream.fields
    WHEN validate_stream_gold_config() is called
    THEN return error with code 400
    AND suggestion contains similar field name

TEST: ValidateRejectsUnknownMetric
    GIVEN gold_etl.aggregates.fields.pm25.metrics = ["invalid_metric"]
    WHEN validate_stream_gold_config() is called
    THEN return error with code 403
    AND message lists valid metrics

TEST: GranularitySuffixConversion
    ASSERT granularity_to_suffix("1 hour") == "hourly"
    ASSERT granularity_to_suffix("1 day") == "daily"
    ASSERT granularity_to_suffix("15 minutes") == "15min"
    ASSERT granularity_to_suffix("4 hours") == "4hour"
```

---

## References

- [SPEC-A02](../specification/SPEC-A02-gold-ddl-tool.md) - Full specification
- [Silver ETL main.rs](/workspaces/neural-data-platform/apps/silver-etl/src/main.rs) - Pattern reference
- [ndp-validate main.rs](/workspaces/neural-data-platform/tools/ndp-validate/src/main.rs) - Pattern reference
