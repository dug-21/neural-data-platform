# dp-020: Declarative Deploy - SPARC Pseudocode

**Document Type**: SPARC Pseudocode (Phase P)
**Feature**: dp-020 Declarative Deploy
**Version**: 1.0
**Date**: 2026-02-02
**Author**: Pseudocode Agent
**Prerequisites**: SCOPE.md, ADR-016-002, dp-019 SUPPORTED-VALUES-RESEARCH.md

---

## 1. Executive Summary

This document provides detailed algorithmic specifications for the dp-020 Declarative Deploy system. The design implements a manifest-driven deployment orchestrator that:

1. Parses and validates `.deploy/manifest.json`
2. Resolves declaration dependencies for correct execution order
3. Generates idempotent DDL for Silver tables (dp-015 absorption)
4. Executes declarations through an extensible handler system
5. Tracks device deployment state

Key Design Principles:
- **Idempotency**: All operations safe to re-run
- **Extensibility**: New declaration types easy to add
- **Error Accumulation**: Collect all errors before failing
- **Correct Ordering**: Dependencies resolved automatically

---

## 2. Data Structures

### 2.1 Manifest Types

```
STRUCT Manifest:
    version: String                     # Manifest schema version (e.g., "1.0")
    changes: Array<Declaration>         # Ordered list of declarations

STRUCT Declaration:
    type: DeclarationType               # stream | silver-table | migration | dimensions | dictionary
    # Type-specific fields via tagged union

ENUM DeclarationType:
    STREAM
    SILVER_TABLE
    MIGRATION
    DIMENSIONS
    DICTIONARY
    CONTAINER
    CONTAINER_BUILD       // Container with action="build"
    CONTAINER_RESTART     // Container with action="restart"
    DEVICE_STATE

STRUCT StreamDeclaration:
    type: "stream"
    id: String                          # Stream ID (e.g., "air-quality")
    action: StreamAction                # create | update | delete
    reload: ReloadType                  # sources | full | none

STRUCT SilverTableDeclaration:
    type: "silver-table"
    stream_id: String                   # Stream ID containing silver_etl config
    action: SilverTableAction           # sync | validate-only

STRUCT MigrationDeclaration:
    type: "migration"
    file: String                        # Path relative to repo root (e.g., "migrations/002-add-indexes.sql")

STRUCT DimensionsDeclaration:
    type: "dimensions"
    action: "sync"
    id: Optional<String>                # Specific dimension ID, or all if omitted

STRUCT DictionaryDeclaration:
    type: "dictionary"
    action: "sync"

STRUCT ContainerDeclaration:
    type: "container"
    target: ContainerTarget              # Container service to manage
    action: ContainerAction              # build | restart
    no_cache: Optional<Boolean>          # For build action, default: false

ENUM ContainerTarget:
    AIR_QUALITY_APP                      # "air-quality-app"
    NDP_MCP_SERVER                       # "ndp-mcp-server"
    SILVER_ETL                           # "silver-etl"
    GRAFANA                              # "grafana"

ENUM ContainerAction:
    BUILD                                # Build container image
    RESTART                              # Restart container with latest image/config

ENUM StreamAction:
    CREATE, UPDATE, DELETE

ENUM SilverTableAction:
    SYNC, VALIDATE_ONLY

ENUM ReloadType:
    SOURCES                             # Hot-reload MQTT/HTTP sources only
    FULL                                # Full app restart
    NONE                                # Sync config only, no reload
```

### 2.2 DDL Generator Types

```
STRUCT SilverEtlConfig:
    enabled: Boolean
    target_table: String                # e.g., "silver.air_quality_observations"
    field_mappings: Array<FieldMapping>
    timestamp: TimestampConfig
    identity_fields: Array<IdentityField>
    dq_rules: Array<DqRule>
    dq_output: DqOutputConfig
    deduplication: DeduplicationConfig
    incremental: IncrementalConfig

STRUCT FieldMapping:
    source_path: String                 # e.g., "raw_payload.pm02Compensated"
    target_column: String               # e.g., "pm25"
    type: ColumnType                    # PostgreSQL type
    unit: Optional<String>
    description: Optional<String>
    nullable: Boolean
    dq_rules: Array<DqRule>

ENUM ColumnType:
    DOUBLE_PRECISION                    # DOUBLE PRECISION
    REAL                                # REAL
    SMALLINT                            # SMALLINT
    INTEGER                             # INTEGER
    BIGINT                              # BIGINT
    TEXT                                # TEXT
    VARCHAR                             # VARCHAR
    BOOLEAN                             # BOOLEAN
    TIMESTAMPTZ                         # TIMESTAMPTZ
    JSONB                               # JSONB
    TEXT_ARRAY                          # TEXT[]

STRUCT GeneratedDDL:
    statements: Array<DDLStatement>     # Ordered SQL statements
    is_new_table: Boolean               # True if CREATE TABLE, false if ALTER
    columns_added: Array<String>        # New columns if ALTER TABLE

STRUCT DDLStatement:
    sql: String                         # SQL statement
    description: String                 # Human-readable purpose
    idempotent: Boolean                 # True if safe to re-run
```

### 2.3 Execution Types

```
STRUCT ExecutionPlan:
    phases: Array<ExecutionPhase>       # Ordered phases
    total_declarations: Integer

STRUCT ExecutionPhase:
    name: String                        # e.g., "migrations", "silver-tables", "streams"
    declarations: Array<Declaration>
    parallel: Boolean                   # Can declarations run in parallel?

STRUCT ExecutionResult:
    success: Boolean
    phase_results: Array<PhaseResult>
    device_state: DeviceState
    errors: Array<ExecutionError>
    warnings: Array<String>

STRUCT PhaseResult:
    phase_name: String
    success: Boolean
    declarations_processed: Integer
    duration_ms: Integer
    errors: Array<ExecutionError>

STRUCT ExecutionError:
    declaration_type: String
    declaration_id: String
    error_code: String
    message: String
    suggestion: Optional<String>

STRUCT DeviceState:
    deployed_version: String            # Git commit SHA
    deployed_at: String                 # ISO 8601 timestamp
    manifest_hash: String               # SHA256 of manifest.json
```

### 2.4 Handler Interface

```
INTERFACE DeclarationHandler:
    // Check if this handler can process the declaration
    can_handle(declaration: Declaration) -> Boolean

    // Validate the declaration before execution
    validate(declaration: Declaration, context: HandlerContext) -> ValidationResult

    // Execute the declaration
    execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult

STRUCT HandlerContext:
    repo_root: Path                     # Repository root path
    config_dir: Path                    # Config directory (config/base/)
    db_pool: Optional<PgPool>           # Database connection pool
    etcd_client: Optional<EtcdClient>   # etcd client
    dry_run: Boolean                    # If true, don't execute, just validate
    verbose: Boolean                    # Verbose logging

STRUCT ValidationResult:
    valid: Boolean
    errors: Array<ValidationError>
    warnings: Array<String>
```

### 2.5 Complexity Analysis: Data Structures

| Structure | Space Complexity | Notes |
|-----------|------------------|-------|
| Manifest | O(d) | d = number of declarations |
| SilverEtlConfig | O(f + r) | f = field_mappings, r = dq_rules |
| GeneratedDDL | O(s) | s = number of SQL statements |
| ExecutionPlan | O(d) | d = total declarations |

---

## 3. Manifest Parser Algorithm

### 3.1 Parse and Validate Manifest

```
ALGORITHM: parse_manifest
INPUT:
    manifest_path: Path                 # Path to .deploy/manifest.json
    schema_path: Path                   # Path to manifest.schema.json
OUTPUT:
    Result<Manifest, ParseError>

BEGIN
    // ========================================
    // PHASE 1: Read manifest file
    // ========================================
    content <- READ_FILE(manifest_path)

    IF content IS error THEN
        RETURN Error(ParseError {
            code: "MANIFEST_NOT_FOUND",
            message: FORMAT("Cannot read manifest: {}", manifest_path),
            suggestion: "Create .deploy/manifest.json with your deployment declarations"
        })
    END IF

    // ========================================
    // PHASE 2: Parse JSON
    // ========================================
    json_value <- PARSE_JSON(content)

    IF json_value IS error THEN
        RETURN Error(ParseError {
            code: "MANIFEST_SYNTAX_ERROR",
            message: FORMAT("Invalid JSON in manifest: {}", json_value.error),
            line: json_value.line,
            column: json_value.column
        })
    END IF

    // ========================================
    // PHASE 3: Validate against JSON Schema
    // ========================================
    schema <- LOAD_JSON_SCHEMA(schema_path)

    IF schema IS error THEN
        RETURN Error(ParseError {
            code: "SCHEMA_NOT_FOUND",
            message: "Manifest schema not found",
            suggestion: "Ensure schemas/manifest.schema.json exists"
        })
    END IF

    schema_errors <- VALIDATE_SCHEMA(json_value, schema)

    IF LENGTH(schema_errors) > 0 THEN
        RETURN Error(ParseError {
            code: "MANIFEST_SCHEMA_INVALID",
            message: "Manifest does not conform to schema",
            details: schema_errors
        })
    END IF

    // ========================================
    // PHASE 4: Deserialize to typed structures
    // ========================================
    manifest <- DESERIALIZE<Manifest>(json_value)

    IF manifest IS error THEN
        RETURN Error(ParseError {
            code: "MANIFEST_DESERIALIZE_FAILED",
            message: FORMAT("Failed to parse manifest: {}", manifest.error)
        })
    END IF

    // ========================================
    // PHASE 5: Validate declaration references
    // ========================================
    validation_errors <- validate_declaration_references(manifest)

    IF LENGTH(validation_errors) > 0 THEN
        RETURN Error(ParseError {
            code: "MANIFEST_REFERENCES_INVALID",
            message: "Manifest contains invalid references",
            details: validation_errors
        })
    END IF

    RETURN Ok(manifest)
END
```

### 3.2 Validate Declaration References

```
ALGORITHM: validate_declaration_references
INPUT:
    manifest: Manifest
OUTPUT:
    Array<ValidationError>

BEGIN
    errors <- []

    FOR idx, declaration IN ENUMERATE(manifest.changes) DO
        base_path <- FORMAT("$.changes[{}]", idx)

        MATCH declaration.type WITH
            STREAM ->
                // Validate stream config exists
                stream_id <- declaration.id
                config_path <- FORMAT("config/base/streams/{}/config.json", stream_id)

                IF NOT FILE_EXISTS(config_path) THEN
                    errors <- errors + [ValidationError {
                        path: base_path + ".id",
                        code: "STREAM_CONFIG_NOT_FOUND",
                        message: FORMAT("Stream config not found: {}", config_path),
                        suggestion: FORMAT("Create {} with stream configuration", config_path)
                    }]
                END IF

            SILVER_TABLE ->
                // Validate stream has silver_etl config
                stream_id <- declaration.stream_id
                config_path <- FORMAT("config/base/streams/{}/config.json", stream_id)

                IF FILE_EXISTS(config_path) THEN
                    config <- LOAD_STREAM_CONFIG(config_path)
                    IF config.silver_etl IS NULL OR NOT config.silver_etl.enabled THEN
                        errors <- errors + [ValidationError {
                            path: base_path + ".stream_id",
                            code: "SILVER_ETL_NOT_ENABLED",
                            message: FORMAT("Stream '{}' has no enabled silver_etl config", stream_id),
                            suggestion: "Add silver_etl section to stream config"
                        }]
                    END IF
                ELSE
                    errors <- errors + [ValidationError {
                        path: base_path + ".stream_id",
                        code: "STREAM_CONFIG_NOT_FOUND",
                        message: FORMAT("Stream config not found: {}", config_path)
                    }]
                END IF

            MIGRATION ->
                // Validate migration file exists
                migration_path <- declaration.file

                IF NOT FILE_EXISTS(migration_path) THEN
                    errors <- errors + [ValidationError {
                        path: base_path + ".file",
                        code: "MIGRATION_FILE_NOT_FOUND",
                        message: FORMAT("Migration file not found: {}", migration_path),
                        suggestion: "Check migration file path"
                    }]
                END IF

            DIMENSIONS ->
                // Validate dimension configs exist (if specific ID provided)
                IF declaration.id IS NOT NULL THEN
                    dim_config_path <- FORMAT("config/base/dimensions/{}.yaml", declaration.id)
                    IF NOT FILE_EXISTS(dim_config_path) THEN
                        errors <- errors + [ValidationError {
                            path: base_path + ".id",
                            code: "DIMENSION_CONFIG_NOT_FOUND",
                            message: FORMAT("Dimension config not found: {}", dim_config_path)
                        }]
                    END IF
                END IF

            DICTIONARY ->
                // No additional validation needed
                PASS

        END MATCH
    END FOR

    RETURN errors
END
```

### 3.3 Complexity Analysis: Parser

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Read file | O(n) | O(n) |
| Parse JSON | O(n) | O(n) |
| Schema validation | O(n * s) | O(e) |
| Reference validation | O(d * f) | O(e) |
| **Total** | O(n * s + d * f) | O(n + e) |

Where: n = file size, s = schema size, d = declarations, f = file ops, e = errors

---

## 4. Declaration Orchestrator Algorithm

### 4.1 Build Execution Plan

```
ALGORITHM: build_execution_plan
INPUT:
    manifest: Manifest
OUTPUT:
    ExecutionPlan

CONSTANTS:
    // Declaration types ordered by dependency
    // Earlier phases must complete before later phases
    // Container builds happen first, restarts happen last
    PHASE_ORDER <- [
        ("container-build", [CONTAINER_BUILD]),   // Builds first
        ("migrations", [MIGRATION]),
        ("silver-tables", [SILVER_TABLE]),
        ("streams", [STREAM]),
        ("dimensions", [DIMENSIONS]),
        ("dictionary", [DICTIONARY]),
        ("container-restart", [CONTAINER_RESTART]), // Restarts last
        ("device-state", [DEVICE_STATE])
    ]

BEGIN
    phases <- []

    // Group declarations by type
    declarations_by_type <- GROUP_BY(manifest.changes, d -> d.type)

    // Build phases in dependency order
    FOR phase_name, declaration_types IN PHASE_ORDER DO
        phase_declarations <- []

        FOR dtype IN declaration_types DO
            IF dtype IN declarations_by_type THEN
                phase_declarations <- phase_declarations + declarations_by_type[dtype]
            END IF
        END FOR

        IF LENGTH(phase_declarations) > 0 THEN
            // Determine if phase can run in parallel
            parallel <- can_parallelize_phase(phase_name, phase_declarations)

            phases <- phases + [ExecutionPhase {
                name: phase_name,
                declarations: phase_declarations,
                parallel: parallel
            }]
        END IF
    END FOR

    RETURN ExecutionPlan {
        phases: phases,
        total_declarations: LENGTH(manifest.changes)
    }
END

// ----------------------------------------
// Helper: Determine Parallelization
// ----------------------------------------
FUNCTION can_parallelize_phase(phase_name: String, declarations: Array<Declaration>) -> Boolean:
    MATCH phase_name WITH
        "container-build" ->
            // Container builds run sequentially (resource contention)
            RETURN false

        "migrations" ->
            // Migrations must run sequentially (order matters)
            RETURN false

        "silver-tables" ->
            // Silver tables can run in parallel (independent tables)
            RETURN true

        "streams" ->
            // Stream syncs can run in parallel
            RETURN true

        "dimensions" ->
            // Dimensions can run in parallel
            RETURN true

        "dictionary" ->
            // Single operation
            RETURN false

        "container-restart" ->
            // Container restarts run sequentially (dependency order)
            RETURN false

        "device-state" ->
            // Single operation
            RETURN false

        _ ->
            RETURN false
    END MATCH
END
```

### 4.2 Execute Plan

```
ALGORITHM: execute_plan
INPUT:
    plan: ExecutionPlan
    context: HandlerContext
    handlers: Array<DeclarationHandler>
OUTPUT:
    ExecutionResult

BEGIN
    phase_results <- []
    all_errors <- []
    all_warnings <- []
    overall_success <- true

    FOR phase IN plan.phases DO
        log_info(FORMAT("Executing phase: {}", phase.name))

        phase_start <- NOW()
        phase_errors <- []
        declarations_processed <- 0

        IF phase.parallel AND NOT context.dry_run THEN
            // Execute declarations in parallel
            results <- PARALLEL_FOR_EACH(phase.declarations, decl ->
                execute_declaration(decl, context, handlers)
            )

            FOR result IN results DO
                IF result.success THEN
                    declarations_processed <- declarations_processed + 1
                ELSE
                    phase_errors <- phase_errors + result.errors
                END IF
            END FOR
        ELSE
            // Execute declarations sequentially
            FOR declaration IN phase.declarations DO
                result <- execute_declaration(declaration, context, handlers)

                IF result.success THEN
                    declarations_processed <- declarations_processed + 1
                ELSE
                    phase_errors <- phase_errors + result.errors

                    // For migrations, stop on first failure
                    IF phase.name == "migrations" THEN
                        BREAK
                    END IF
                END IF
            END FOR
        END IF

        phase_duration <- NOW() - phase_start
        phase_success <- LENGTH(phase_errors) == 0

        IF NOT phase_success THEN
            overall_success <- false
        END IF

        phase_results <- phase_results + [PhaseResult {
            phase_name: phase.name,
            success: phase_success,
            declarations_processed: declarations_processed,
            duration_ms: phase_duration.as_millis(),
            errors: phase_errors
        }]

        all_errors <- all_errors + phase_errors

        // Stop if critical phase fails
        IF NOT phase_success AND phase.name IN ["migrations", "silver-tables"] THEN
            log_error(FORMAT("Critical phase '{}' failed, aborting", phase.name))
            BREAK
        END IF
    END FOR

    // Update device state if successful
    device_state <- NULL
    IF overall_success AND NOT context.dry_run THEN
        device_state <- update_device_state(context)
    END IF

    RETURN ExecutionResult {
        success: overall_success,
        phase_results: phase_results,
        device_state: device_state,
        errors: all_errors,
        warnings: all_warnings
    }
END
```

### 4.3 Execute Single Declaration

```
ALGORITHM: execute_declaration
INPUT:
    declaration: Declaration
    context: HandlerContext
    handlers: Array<DeclarationHandler>
OUTPUT:
    DeclarationResult

BEGIN
    // Find appropriate handler
    handler <- NULL
    FOR h IN handlers DO
        IF h.can_handle(declaration) THEN
            handler <- h
            BREAK
        END IF
    END FOR

    IF handler IS NULL THEN
        RETURN DeclarationResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: declaration.type,
                declaration_id: get_declaration_id(declaration),
                error_code: "NO_HANDLER",
                message: FORMAT("No handler for declaration type: {}", declaration.type)
            }]
        }
    END IF

    // Validate first
    validation <- handler.validate(declaration, context)

    IF NOT validation.valid THEN
        RETURN DeclarationResult {
            success: false,
            errors: validation.errors
        }
    END IF

    // Execute if not dry run
    IF context.dry_run THEN
        log_info(FORMAT("[DRY RUN] Would execute: {} {}",
            declaration.type, get_declaration_id(declaration)))
        RETURN DeclarationResult { success: true, errors: [] }
    END IF

    // Execute
    TRY
        result <- handler.execute(declaration, context)
        RETURN result
    CATCH error
        RETURN DeclarationResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: declaration.type,
                declaration_id: get_declaration_id(declaration),
                error_code: "EXECUTION_FAILED",
                message: FORMAT("Execution failed: {}", error.message)
            }]
        }
    END TRY
END
```

---

## 5. DDL Generator Algorithm (dp-015)

### 5.1 Main DDL Generation Entry Point

```
ALGORITHM: generate_silver_ddl
INPUT:
    stream_config: StreamConfig         # Full stream config with silver_etl
    db_pool: Optional<PgPool>           # Database for schema comparison
OUTPUT:
    Result<GeneratedDDL, DdlError>

CONSTANTS:
    // Type mapping: config type -> PostgreSQL type
    TYPE_MAP <- {
        "double_precision": "DOUBLE PRECISION",
        "real": "REAL",
        "smallint": "SMALLINT",
        "integer": "INTEGER",
        "bigint": "BIGINT",
        "text": "TEXT",
        "varchar": "VARCHAR",
        "boolean": "BOOLEAN",
        "timestamptz": "TIMESTAMPTZ",
        "jsonb": "JSONB",
        "text[]": "TEXT[]"
    }

    // Standard columns added to every Silver table
    STANDARD_COLUMNS <- [
        ("timestamp", "TIMESTAMPTZ", "NOT NULL"),
        ("ndp_id", "TEXT", "NOT NULL"),
        ("dq_flags", "TEXT[]", ""),
        ("_bronze_id", "UUID", ""),
        ("_ingested_at", "TIMESTAMPTZ", "DEFAULT NOW()")
    ]

BEGIN
    silver_etl <- stream_config.silver_etl

    IF silver_etl IS NULL OR NOT silver_etl.enabled THEN
        RETURN Error(DdlError {
            code: "SILVER_ETL_DISABLED",
            message: "silver_etl is not enabled for this stream"
        })
    END IF

    target_table <- silver_etl.target_table

    // Parse schema.table format
    parts <- target_table.split(".")
    IF LENGTH(parts) != 2 THEN
        RETURN Error(DdlError {
            code: "INVALID_TABLE_NAME",
            message: FORMAT("Invalid table name '{}'. Expected 'schema.table'", target_table)
        })
    END IF

    schema_name <- parts[0]
    table_name <- parts[1]

    // Check if table exists
    table_exists <- false
    existing_columns <- EMPTY_SET()

    IF db_pool IS NOT NULL THEN
        table_exists <- AWAIT check_table_exists(db_pool, schema_name, table_name)

        IF table_exists THEN
            existing_columns <- AWAIT get_existing_columns(db_pool, schema_name, table_name)
        END IF
    END IF

    // Generate DDL
    IF table_exists THEN
        RETURN generate_alter_table_ddl(silver_etl, schema_name, table_name, existing_columns)
    ELSE
        RETURN generate_create_table_ddl(silver_etl, schema_name, table_name)
    END IF
END
```

### 5.2 Generate CREATE TABLE DDL

```
ALGORITHM: generate_create_table_ddl
INPUT:
    silver_etl: SilverEtlConfig
    schema_name: String
    table_name: String
OUTPUT:
    Result<GeneratedDDL, DdlError>

BEGIN
    statements <- []
    full_table_name <- FORMAT("{}.{}", schema_name, table_name)

    // ========================================
    // 5.2.1: CREATE TABLE statement
    // ========================================
    column_defs <- []

    // Add standard columns first
    FOR col_name, col_type, col_constraint IN STANDARD_COLUMNS DO
        column_defs <- column_defs + [FORMAT("    {} {} {}", col_name, col_type, col_constraint)]
    END FOR

    // Add field mapping columns
    FOR mapping IN silver_etl.field_mappings DO
        pg_type <- map_type_to_postgres(mapping.type)

        null_constraint <- ""
        IF NOT mapping.nullable THEN
            null_constraint <- "NOT NULL"
        END IF

        column_defs <- column_defs + [FORMAT("    {} {} {}",
            mapping.target_column, pg_type, null_constraint)]
    END FOR

    create_table_sql <- FORMAT(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        full_table_name,
        JOIN(column_defs, ",\n")
    )

    statements <- statements + [DDLStatement {
        sql: create_table_sql,
        description: FORMAT("Create Silver table {}", full_table_name),
        idempotent: true
    }]

    // ========================================
    // 5.2.2: Create indexes
    // ========================================

    // Primary index: (timestamp, ndp_id)
    timestamp_col <- silver_etl.timestamp.target_field OR "timestamp"

    idx_time_id_sql <- FORMAT(
        "CREATE INDEX IF NOT EXISTS idx_{}_time_id ON {} ({}, ndp_id);",
        table_name, full_table_name, timestamp_col
    )

    statements <- statements + [DDLStatement {
        sql: idx_time_id_sql,
        description: "Create primary time/identity index",
        idempotent: true
    }]

    // GIN index on dq_flags for efficient flag queries
    idx_dq_flags_sql <- FORMAT(
        "CREATE INDEX IF NOT EXISTS idx_{}_dq_flags ON {} USING GIN (dq_flags);",
        table_name, full_table_name
    )

    statements <- statements + [DDLStatement {
        sql: idx_dq_flags_sql,
        description: "Create GIN index on dq_flags",
        idempotent: true
    }]

    // ========================================
    // 5.2.3: Convert to hypertable
    // ========================================
    hypertable_sql <- FORMAT(
        "SELECT create_hypertable('{}', '{}', chunk_time_interval => INTERVAL '1 day', if_not_exists => TRUE);",
        full_table_name, timestamp_col
    )

    statements <- statements + [DDLStatement {
        sql: hypertable_sql,
        description: "Convert to TimescaleDB hypertable",
        idempotent: true
    }]

    // ========================================
    // 5.2.4: Compression policy
    // ========================================
    compression_days <- stream_config.compression_after_days OR 7

    compression_sql <- FORMAT(
        "SELECT add_compression_policy('{}', INTERVAL '{} days', if_not_exists => TRUE);",
        full_table_name, compression_days
    )

    statements <- statements + [DDLStatement {
        sql: compression_sql,
        description: FORMAT("Add compression policy (after {} days)", compression_days),
        idempotent: true
    }]

    // ========================================
    // 5.2.5: Retention policy
    // ========================================
    retention_days <- stream_config.retention_days OR 90

    retention_sql <- FORMAT(
        "SELECT add_retention_policy('{}', INTERVAL '{} days', if_not_exists => TRUE);",
        full_table_name, retention_days
    )

    statements <- statements + [DDLStatement {
        sql: retention_sql,
        description: FORMAT("Add retention policy ({} days)", retention_days),
        idempotent: true
    }]

    // ========================================
    // 5.2.6: Grant permissions
    // ========================================
    grant_app_sql <- FORMAT(
        "GRANT SELECT, INSERT ON {} TO ndp_app;",
        full_table_name
    )

    grant_reader_sql <- FORMAT(
        "GRANT SELECT ON {} TO grafana_reader;",
        full_table_name
    )

    statements <- statements + [DDLStatement {
        sql: grant_app_sql,
        description: "Grant SELECT, INSERT to ndp_app role",
        idempotent: true
    }]

    statements <- statements + [DDLStatement {
        sql: grant_reader_sql,
        description: "Grant SELECT to grafana_reader role",
        idempotent: true
    }]

    RETURN Ok(GeneratedDDL {
        statements: statements,
        is_new_table: true,
        columns_added: []
    })
END
```

### 5.3 Generate ALTER TABLE DDL (ADD COLUMN)

```
ALGORITHM: generate_alter_table_ddl
INPUT:
    silver_etl: SilverEtlConfig
    schema_name: String
    table_name: String
    existing_columns: Set<String>       # Columns already in table
OUTPUT:
    Result<GeneratedDDL, DdlError>

BEGIN
    statements <- []
    columns_added <- []
    full_table_name <- FORMAT("{}.{}", schema_name, table_name)

    // ========================================
    // 5.3.1: Find new columns to add
    // ========================================
    FOR mapping IN silver_etl.field_mappings DO
        target_column <- mapping.target_column

        IF target_column IN existing_columns THEN
            // Column already exists - skip
            CONTINUE
        END IF

        // Column doesn't exist - generate ADD COLUMN
        pg_type <- map_type_to_postgres(mapping.type)

        // Use DO block for idempotent ADD COLUMN
        add_column_sql <- FORMAT(
            "DO $$\n" +
            "BEGIN\n" +
            "    IF NOT EXISTS (\n" +
            "        SELECT 1 FROM information_schema.columns\n" +
            "        WHERE table_schema = '{}'\n" +
            "        AND table_name = '{}'\n" +
            "        AND column_name = '{}'\n" +
            "    ) THEN\n" +
            "        ALTER TABLE {} ADD COLUMN {} {};\n" +
            "    END IF;\n" +
            "END $$;",
            schema_name, table_name, target_column,
            full_table_name, target_column, pg_type
        )

        statements <- statements + [DDLStatement {
            sql: add_column_sql,
            description: FORMAT("Add column {} ({})", target_column, pg_type),
            idempotent: true
        }]

        columns_added <- columns_added + [target_column]
    END FOR

    // ========================================
    // 5.3.2: Check for standard columns
    // ========================================
    FOR col_name, col_type, col_constraint IN STANDARD_COLUMNS DO
        IF col_name NOT IN existing_columns THEN
            add_standard_sql <- FORMAT(
                "DO $$\n" +
                "BEGIN\n" +
                "    IF NOT EXISTS (\n" +
                "        SELECT 1 FROM information_schema.columns\n" +
                "        WHERE table_schema = '{}'\n" +
                "        AND table_name = '{}'\n" +
                "        AND column_name = '{}'\n" +
                "    ) THEN\n" +
                "        ALTER TABLE {} ADD COLUMN {} {} {};\n" +
                "    END IF;\n" +
                "END $$;",
                schema_name, table_name, col_name,
                full_table_name, col_name, col_type, col_constraint
            )

            statements <- statements + [DDLStatement {
                sql: add_standard_sql,
                description: FORMAT("Add standard column {}", col_name),
                idempotent: true
            }]

            columns_added <- columns_added + [col_name]
        END IF
    END FOR

    IF LENGTH(statements) == 0 THEN
        // No changes needed
        RETURN Ok(GeneratedDDL {
            statements: [],
            is_new_table: false,
            columns_added: []
        })
    END IF

    RETURN Ok(GeneratedDDL {
        statements: statements,
        is_new_table: false,
        columns_added: columns_added
    })
END
```

### 5.4 Type Mapping

```
FUNCTION map_type_to_postgres(config_type: ColumnType) -> String:
    // Based on SUPPORTED-VALUES-RESEARCH.md section 4
    MATCH config_type WITH
        DOUBLE_PRECISION -> "DOUBLE PRECISION"
        REAL -> "REAL"
        SMALLINT -> "SMALLINT"
        INTEGER -> "INTEGER"
        BIGINT -> "BIGINT"
        TEXT -> "TEXT"
        VARCHAR -> "VARCHAR"
        BOOLEAN -> "BOOLEAN"
        TIMESTAMPTZ -> "TIMESTAMPTZ"
        JSONB -> "JSONB"
        TEXT_ARRAY -> "TEXT[]"
        _ -> "TEXT"  // Default fallback
    END MATCH
END

// Legacy type mapping for backward compatibility
FUNCTION map_legacy_type(legacy_type: String) -> String:
    MATCH legacy_type.lowercase() WITH
        "float" -> "DOUBLE PRECISION"
        "int" -> "INTEGER"
        "string" -> "TEXT"
        "bool" -> "BOOLEAN"
        "json" -> "JSONB"
        _ -> "TEXT"
    END MATCH
END
```

### 5.5 Database Helper Functions

```
ALGORITHM: check_table_exists
INPUT:
    db_pool: PgPool
    schema_name: String
    table_name: String
OUTPUT:
    Boolean

BEGIN
    result <- AWAIT db_pool.query_scalar(
        SQL"SELECT EXISTS(
            SELECT 1 FROM information_schema.tables
            WHERE table_schema = $1 AND table_name = $2
        )",
        [schema_name, table_name]
    )

    RETURN result OR false
END


ALGORITHM: get_existing_columns
INPUT:
    db_pool: PgPool
    schema_name: String
    table_name: String
OUTPUT:
    Set<String>

BEGIN
    rows <- AWAIT db_pool.query(
        SQL"SELECT column_name FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2",
        [schema_name, table_name]
    )

    columns <- EMPTY_SET()
    FOR row IN rows DO
        columns <- columns + {row.column_name}
    END FOR

    RETURN columns
END
```

### 5.6 Complexity Analysis: DDL Generation

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Type mapping | O(1) | O(1) |
| Column generation | O(f) | O(f) |
| Index generation | O(1) | O(1) |
| Database check | O(1) per query | O(c) |
| **Total** | O(f + c) | O(f + s) |

Where: f = field_mappings count, c = existing columns, s = SQL statements

---

## 6. Declaration Handlers

### 6.1 Handler Registry

```
ALGORITHM: create_handler_registry
OUTPUT:
    Array<DeclarationHandler>

BEGIN
    RETURN [
        StreamHandler {},
        SilverTableHandler {},
        MigrationHandler {},
        DimensionsHandler {},
        DictionaryHandler {},
        ContainerHandler {}
    ]
END
```

### 6.2 Stream Handler

```
CLASS StreamHandler IMPLEMENTS DeclarationHandler:

    FUNCTION can_handle(declaration: Declaration) -> Boolean:
        RETURN declaration.type == STREAM
    END

    FUNCTION validate(declaration: Declaration, context: HandlerContext) -> ValidationResult:
        errors <- []

        stream_decl <- declaration AS StreamDeclaration
        config_path <- FORMAT("{}/base/streams/{}/config.json",
            context.config_dir, stream_decl.id)

        // Validate config file exists
        IF NOT FILE_EXISTS(config_path) THEN
            errors <- errors + [ValidationError {
                code: "STREAM_CONFIG_NOT_FOUND",
                path: config_path,
                message: FORMAT("Stream config not found: {}", stream_decl.id)
            }]
            RETURN ValidationResult { valid: false, errors: errors }
        END IF

        // Run dp-019 validation on the config
        validation_result <- AWAIT validate_stream_config(config_path, context)

        IF NOT validation_result.valid THEN
            errors <- errors + validation_result.errors
        END IF

        RETURN ValidationResult {
            valid: LENGTH(errors) == 0,
            errors: errors,
            warnings: validation_result.warnings
        }
    END

    FUNCTION execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult:
        stream_decl <- declaration AS StreamDeclaration

        // Load config
        config_path <- FORMAT("{}/base/streams/{}/config.json",
            context.config_dir, stream_decl.id)
        config_content <- READ_FILE(config_path)

        // Sync to etcd
        etcd_key <- FORMAT("/streams/{}/config", stream_decl.id)

        TRY
            AWAIT context.etcd_client.put(etcd_key, config_content)
            log_info(FORMAT("Synced stream '{}' to etcd", stream_decl.id))
        CATCH error
            RETURN ExecutionResult {
                success: false,
                errors: [ExecutionError {
                    declaration_type: "stream",
                    declaration_id: stream_decl.id,
                    error_code: "ETCD_SYNC_FAILED",
                    message: FORMAT("Failed to sync to etcd: {}", error.message)
                }]
            }
        END TRY

        // Handle reload
        MATCH stream_decl.reload WITH
            SOURCES ->
                // Hot-reload sources (if supported)
                AWAIT trigger_source_reload(stream_decl.id)

            FULL ->
                // Mark for full restart (handled at end of deploy)
                context.streams_requiring_restart <- context.streams_requiring_restart + {stream_decl.id}

            NONE ->
                // No reload needed
                PASS
        END MATCH

        RETURN ExecutionResult { success: true, errors: [] }
    END
END
```

### 6.3 Silver Table Handler

```
CLASS SilverTableHandler IMPLEMENTS DeclarationHandler:

    FUNCTION can_handle(declaration: Declaration) -> Boolean:
        RETURN declaration.type == SILVER_TABLE
    END

    FUNCTION validate(declaration: Declaration, context: HandlerContext) -> ValidationResult:
        errors <- []

        silver_decl <- declaration AS SilverTableDeclaration

        // Load stream config
        config_path <- FORMAT("{}/base/streams/{}/config.json",
            context.config_dir, silver_decl.stream_id)

        config <- LOAD_STREAM_CONFIG(config_path)

        IF config IS error THEN
            errors <- errors + [ValidationError {
                code: "CONFIG_LOAD_FAILED",
                message: FORMAT("Failed to load config: {}", config.error)
            }]
            RETURN ValidationResult { valid: false, errors: errors }
        END IF

        // Validate silver_etl is enabled
        IF config.silver_etl IS NULL OR NOT config.silver_etl.enabled THEN
            errors <- errors + [ValidationError {
                code: "SILVER_ETL_DISABLED",
                message: FORMAT("silver_etl not enabled for stream '{}'", silver_decl.stream_id)
            }]
        END IF

        // Validate field_mappings
        IF config.silver_etl IS NOT NULL THEN
            IF LENGTH(config.silver_etl.field_mappings) == 0 THEN
                errors <- errors + [ValidationError {
                    code: "NO_FIELD_MAPPINGS",
                    message: "silver_etl.field_mappings is empty"
                }]
            END IF

            // Validate each mapping has required fields
            FOR idx, mapping IN ENUMERATE(config.silver_etl.field_mappings) DO
                IF mapping.target_column IS NULL OR mapping.target_column == "" THEN
                    errors <- errors + [ValidationError {
                        code: "MISSING_TARGET_COLUMN",
                        path: FORMAT("silver_etl.field_mappings[{}].target_column", idx),
                        message: "target_column is required"
                    }]
                END IF

                IF mapping.type IS NULL OR mapping.type == "" THEN
                    errors <- errors + [ValidationError {
                        code: "MISSING_TYPE",
                        path: FORMAT("silver_etl.field_mappings[{}].type", idx),
                        message: "type is required"
                    }]
                END IF
            END FOR
        END IF

        RETURN ValidationResult {
            valid: LENGTH(errors) == 0,
            errors: errors
        }
    END

    FUNCTION execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult:
        silver_decl <- declaration AS SilverTableDeclaration

        // Load stream config
        config_path <- FORMAT("{}/base/streams/{}/config.json",
            context.config_dir, silver_decl.stream_id)
        config <- LOAD_STREAM_CONFIG(config_path)

        // Generate DDL
        ddl_result <- generate_silver_ddl(config, context.db_pool)

        IF ddl_result IS error THEN
            RETURN ExecutionResult {
                success: false,
                errors: [ExecutionError {
                    declaration_type: "silver-table",
                    declaration_id: silver_decl.stream_id,
                    error_code: "DDL_GENERATION_FAILED",
                    message: ddl_result.error.message
                }]
            }
        END IF

        ddl <- ddl_result.value

        // Handle validate-only action
        IF silver_decl.action == VALIDATE_ONLY THEN
            log_info(FORMAT("DDL validation passed for '{}'", config.silver_etl.target_table))
            RETURN ExecutionResult { success: true, errors: [] }
        END IF

        // Execute DDL statements
        IF LENGTH(ddl.statements) == 0 THEN
            log_info(FORMAT("No DDL changes needed for '{}'", config.silver_etl.target_table))
            RETURN ExecutionResult { success: true, errors: [] }
        END IF

        FOR stmt IN ddl.statements DO
            log_info(FORMAT("Executing: {}", stmt.description))

            TRY
                AWAIT context.db_pool.execute(stmt.sql)
            CATCH db_error
                // Check if error is acceptable for idempotent operations
                IF stmt.idempotent AND is_acceptable_idempotent_error(db_error) THEN
                    log_info(FORMAT("Skipping (already exists): {}", stmt.description))
                    CONTINUE
                END IF

                RETURN ExecutionResult {
                    success: false,
                    errors: [ExecutionError {
                        declaration_type: "silver-table",
                        declaration_id: silver_decl.stream_id,
                        error_code: "DDL_EXECUTION_FAILED",
                        message: FORMAT("Failed to execute DDL: {}", db_error.message),
                        suggestion: "Check database connection and permissions"
                    }]
                }
            END TRY
        END FOR

        IF ddl.is_new_table THEN
            log_info(FORMAT("Created Silver table '{}'", config.silver_etl.target_table))
        ELSE IF LENGTH(ddl.columns_added) > 0 THEN
            log_info(FORMAT("Added columns to '{}': {}",
                config.silver_etl.target_table, JOIN(ddl.columns_added, ", ")))
        END IF

        RETURN ExecutionResult { success: true, errors: [] }
    END
END
```

### 6.4 Migration Handler

```
CLASS MigrationHandler IMPLEMENTS DeclarationHandler:

    FUNCTION can_handle(declaration: Declaration) -> Boolean:
        RETURN declaration.type == MIGRATION
    END

    FUNCTION validate(declaration: Declaration, context: HandlerContext) -> ValidationResult:
        errors <- []

        migration_decl <- declaration AS MigrationDeclaration
        migration_path <- FORMAT("{}/{}", context.repo_root, migration_decl.file)

        // Check file exists
        IF NOT FILE_EXISTS(migration_path) THEN
            errors <- errors + [ValidationError {
                code: "MIGRATION_NOT_FOUND",
                path: migration_decl.file,
                message: FORMAT("Migration file not found: {}", migration_path)
            }]
            RETURN ValidationResult { valid: false, errors: errors }
        END IF

        // Validate SQL syntax (basic check)
        content <- READ_FILE(migration_path)
        IF content.trim().is_empty() THEN
            errors <- errors + [ValidationError {
                code: "EMPTY_MIGRATION",
                path: migration_decl.file,
                message: "Migration file is empty"
            }]
        END IF

        // Extract migration version from filename
        version <- extract_migration_version(migration_decl.file)
        IF version IS NULL THEN
            errors <- errors + [ValidationError {
                code: "INVALID_MIGRATION_NAME",
                path: migration_decl.file,
                message: "Migration filename must start with version number (e.g., 001-description.sql)"
            }]
        END IF

        RETURN ValidationResult {
            valid: LENGTH(errors) == 0,
            errors: errors
        }
    END

    FUNCTION execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult:
        migration_decl <- declaration AS MigrationDeclaration
        migration_path <- FORMAT("{}/{}", context.repo_root, migration_decl.file)

        // Extract version
        version <- extract_migration_version(migration_decl.file)

        // Check if already applied
        already_applied <- AWAIT check_migration_applied(context.db_pool, version)

        IF already_applied THEN
            log_info(FORMAT("Migration {} already applied, skipping", version))
            RETURN ExecutionResult { success: true, errors: [] }
        END IF

        // Read and execute migration
        sql_content <- READ_FILE(migration_path)

        TRY
            // Wrap in transaction
            AWAIT context.db_pool.execute("BEGIN")
            AWAIT context.db_pool.execute(sql_content)

            // Record migration
            AWAIT context.db_pool.execute(
                SQL"INSERT INTO schema_version (version, applied_at, script_name)
                    VALUES ($1, NOW(), $2)",
                [version, migration_decl.file]
            )

            AWAIT context.db_pool.execute("COMMIT")

            log_info(FORMAT("Applied migration: {}", migration_decl.file))

        CATCH db_error
            AWAIT context.db_pool.execute("ROLLBACK")

            RETURN ExecutionResult {
                success: false,
                errors: [ExecutionError {
                    declaration_type: "migration",
                    declaration_id: migration_decl.file,
                    error_code: "MIGRATION_FAILED",
                    message: FORMAT("Migration failed: {}", db_error.message),
                    suggestion: "Check SQL syntax and database state"
                }]
            }
        END TRY

        RETURN ExecutionResult { success: true, errors: [] }
    END
END

// ----------------------------------------
// Helper: Extract Migration Version
// ----------------------------------------
FUNCTION extract_migration_version(filename: String) -> Optional<String>:
    // Filename pattern: "NNN-description.sql" or "migrations/NNN-description.sql"
    basename <- filename.split("/").last()

    REGEX pattern <- /^(\d{3})-/
    match <- pattern.match(basename)

    IF match IS NOT NULL THEN
        RETURN match.group(1)  // e.g., "001", "002"
    ELSE
        RETURN NULL
    END IF
END
```

### 6.5 Dimensions Handler

```
CLASS DimensionsHandler IMPLEMENTS DeclarationHandler:

    FUNCTION can_handle(declaration: Declaration) -> Boolean:
        RETURN declaration.type == DIMENSIONS
    END

    FUNCTION validate(declaration: Declaration, context: HandlerContext) -> ValidationResult:
        errors <- []

        dim_decl <- declaration AS DimensionsDeclaration
        dim_dir <- FORMAT("{}/base/dimensions", context.config_dir)

        IF dim_decl.id IS NOT NULL THEN
            // Validate specific dimension
            config_path <- FORMAT("{}/{}.yaml", dim_dir, dim_decl.id)
            IF NOT FILE_EXISTS(config_path) THEN
                errors <- errors + [ValidationError {
                    code: "DIMENSION_NOT_FOUND",
                    message: FORMAT("Dimension config not found: {}", dim_decl.id)
                }]
            END IF
        ELSE
            // Validate dimension directory exists
            IF NOT DIRECTORY_EXISTS(dim_dir) THEN
                errors <- errors + [ValidationError {
                    code: "DIMENSION_DIR_NOT_FOUND",
                    message: FORMAT("Dimensions directory not found: {}", dim_dir)
                }]
            END IF
        END IF

        RETURN ValidationResult {
            valid: LENGTH(errors) == 0,
            errors: errors
        }
    END

    FUNCTION execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult:
        dim_decl <- declaration AS DimensionsDeclaration
        dim_dir <- FORMAT("{}/base/dimensions", context.config_dir)

        // Collect dimension configs to sync
        configs_to_sync <- []

        IF dim_decl.id IS NOT NULL THEN
            // Sync specific dimension
            config_path <- FORMAT("{}/{}.yaml", dim_dir, dim_decl.id)
            configs_to_sync <- [config_path]
        ELSE
            // Sync all dimensions
            configs_to_sync <- GLOB(FORMAT("{}/*.yaml", dim_dir))
        END IF

        FOR config_path IN configs_to_sync DO
            dim_id <- extract_dimension_id(config_path)
            result <- sync_single_dimension(dim_id, config_path, context)

            IF NOT result.success THEN
                RETURN result
            END IF
        END FOR

        log_info(FORMAT("Synced {} dimension(s)", LENGTH(configs_to_sync)))
        RETURN ExecutionResult { success: true, errors: [] }
    END
END
```

### 6.6 Dictionary Handler

```
CLASS DictionaryHandler IMPLEMENTS DeclarationHandler:

    FUNCTION can_handle(declaration: Declaration) -> Boolean:
        RETURN declaration.type == DICTIONARY
    END

    FUNCTION validate(declaration: Declaration, context: HandlerContext) -> ValidationResult:
        // Dictionary sync is always valid if database is available
        IF context.db_pool IS NULL THEN
            RETURN ValidationResult {
                valid: false,
                errors: [ValidationError {
                    code: "NO_DATABASE",
                    message: "Database connection required for dictionary sync"
                }]
            }
        END IF

        RETURN ValidationResult { valid: true, errors: [] }
    END

    FUNCTION execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult:
        // Sync data dictionary from all stream configs
        // This reuses the logic from deploy.sh sync_to_data_dictionary

        streams_dir <- FORMAT("{}/base/streams", context.config_dir)

        TRY
            AWAIT context.db_pool.execute("BEGIN")

            // Clear existing data
            AWAIT context.db_pool.execute("DELETE FROM data_dictionary.entity_schema_attributes")
            AWAIT context.db_pool.execute("DELETE FROM data_dictionary.entity_schemas")
            AWAIT context.db_pool.execute("DELETE FROM data_dictionary.streams")

            // Sync each stream
            FOR stream_dir IN GLOB(FORMAT("{}/*/", streams_dir)) DO
                config_path <- FORMAT("{}/config.json", stream_dir)

                IF FILE_EXISTS(config_path) THEN
                    config <- LOAD_STREAM_CONFIG(config_path)
                    AWAIT sync_stream_to_dictionary(config, context.db_pool)
                END IF
            END FOR

            AWAIT context.db_pool.execute("COMMIT")

            log_info("Synced data dictionary")
            RETURN ExecutionResult { success: true, errors: [] }

        CATCH db_error
            AWAIT context.db_pool.execute("ROLLBACK")

            RETURN ExecutionResult {
                success: false,
                errors: [ExecutionError {
                    declaration_type: "dictionary",
                    declaration_id: "data_dictionary",
                    error_code: "DICTIONARY_SYNC_FAILED",
                    message: FORMAT("Failed to sync dictionary: {}", db_error.message)
                }]
            }
        END TRY
    END
END
```

### 6.7 Container Handler

```
CLASS ContainerHandler IMPLEMENTS DeclarationHandler:

    // ========================================
    // Container Declaration Structure
    // ========================================
    // ContainerDeclaration {
    //     type: "container"
    //     target: "air-quality-app" | "ndp-mcp-server" | "silver-etl" | "grafana"
    //     action: "build" | "restart"
    //     no_cache: boolean (optional, default: false)
    // }

    FUNCTION can_handle(declaration: Declaration) -> Boolean:
        RETURN declaration.type == CONTAINER
    END

    FUNCTION validate(declaration: Declaration, context: HandlerContext) -> ValidationResult:
        errors <- []

        container_decl <- declaration AS ContainerDeclaration

        // Validate target is a known container
        valid_targets <- ["air-quality-app", "ndp-mcp-server", "silver-etl", "grafana"]

        IF container_decl.target NOT IN valid_targets THEN
            errors <- errors + [ValidationError {
                code: "UNKNOWN_CONTAINER_TARGET",
                path: "target",
                message: FORMAT("Unknown container target: {}. Valid targets: {}",
                    container_decl.target, JOIN(valid_targets, ", "))
            }]
        END IF

        // Validate action is valid
        IF container_decl.action NOT IN ["build", "restart"] THEN
            errors <- errors + [ValidationError {
                code: "INVALID_CONTAINER_ACTION",
                path: "action",
                message: FORMAT("Invalid action: {}. Must be 'build' or 'restart'",
                    container_decl.action)
            }]
        END IF

        RETURN ValidationResult {
            valid: LENGTH(errors) == 0,
            errors: errors
        }
    END

    FUNCTION execute(declaration: Declaration, context: HandlerContext) -> ExecutionResult:
        container_decl <- declaration AS ContainerDeclaration

        MATCH container_decl.action WITH
            "build" ->
                RETURN handle_container_build(container_decl, context)
            "restart" ->
                RETURN handle_container_restart(container_decl, context)
            _ ->
                RETURN ExecutionResult {
                    success: false,
                    errors: [ExecutionError {
                        declaration_type: "container",
                        declaration_id: container_decl.target,
                        error_code: "INVALID_ACTION",
                        message: FORMAT("Unknown action: {}", container_decl.action)
                    }]
                }
        END MATCH
    END
END

// ----------------------------------------
// Algorithm: handle_container_build
// ----------------------------------------
ALGORITHM: handle_container_build
INPUT:
    declaration: ContainerDeclaration
    context: HandlerContext
OUTPUT:
    ExecutionResult

BEGIN
    target <- declaration.target
    no_cache <- declaration.no_cache OR false

    // Map target to docker compose service name
    service <- map_target_to_service(target)

    IF service IS error THEN
        RETURN ExecutionResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: "container",
                declaration_id: target,
                error_code: "750",
                message: service.error.message
            }]
        }
    END IF

    // Build command
    compose_file <- context.repo_root + "/deploy/docker-compose.yml"

    IF no_cache THEN
        cmd <- FORMAT("docker compose -f {} build --no-cache {}", compose_file, service)
    ELSE
        cmd <- FORMAT("docker compose -f {} build {}", compose_file, service)
    END IF

    log_info(FORMAT("Building container: {} (no_cache: {})", target, no_cache))

    // Execute build
    result <- EXECUTE_COMMAND(cmd)

    IF result.exit_code != 0 THEN
        RETURN ExecutionResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: "container",
                declaration_id: target,
                error_code: "750",
                message: FORMAT("Container build failed: {}", target),
                suggestion: FORMAT("Check build output: {}", result.stderr)
            }]
        }
    END IF

    log_info(FORMAT("Container built: {}", target))
    RETURN ExecutionResult { success: true, errors: [] }
END

// ----------------------------------------
// Algorithm: handle_container_restart
// ----------------------------------------
ALGORITHM: handle_container_restart
INPUT:
    declaration: ContainerDeclaration
    context: HandlerContext
OUTPUT:
    ExecutionResult

BEGIN
    target <- declaration.target
    service <- map_target_to_service(target)

    IF service IS error THEN
        RETURN ExecutionResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: "container",
                declaration_id: target,
                error_code: "751",
                message: service.error.message
            }]
        }
    END IF

    compose_file <- context.repo_root + "/deploy/docker-compose.yml"

    // Use 'up -d' to recreate container with latest image/config
    cmd <- FORMAT("docker compose -f {} up -d {}", compose_file, service)

    log_info(FORMAT("Restarting container: {}", target))

    result <- EXECUTE_COMMAND(cmd)

    IF result.exit_code != 0 THEN
        RETURN ExecutionResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: "container",
                declaration_id: target,
                error_code: "751",
                message: FORMAT("Container restart failed: {}", target),
                suggestion: FORMAT("Check docker output: {}", result.stderr)
            }]
        }
    END IF

    // Wait for health check
    IF NOT wait_for_healthy(service, timeout: 60) THEN
        RETURN ExecutionResult {
            success: false,
            errors: [ExecutionError {
                declaration_type: "container",
                declaration_id: target,
                error_code: "752",
                message: FORMAT("Container unhealthy after restart: {}", target),
                suggestion: "Check container logs and health check configuration"
            }]
        }
    END IF

    log_info(FORMAT("Container restarted: {}", target))
    RETURN ExecutionResult { success: true, errors: [] }
END

// ----------------------------------------
// Target to Service Mapping
// ----------------------------------------
FUNCTION map_target_to_service(target: String) -> Result<String, Error>:
    MATCH target WITH
        "air-quality-app" -> RETURN Ok("air-quality-app")
        "ndp-mcp-server" -> RETURN Ok("ndp-mcp-server")
        "silver-etl" -> RETURN Ok("silver-etl")
        "grafana" -> RETURN Ok("grafana")
        _ -> RETURN Error(FORMAT("Unknown container target: {}", target))
    END MATCH
END

// ----------------------------------------
// Helper: Wait for Container Health
// ----------------------------------------
FUNCTION wait_for_healthy(service: String, timeout: Integer) -> Boolean:
    start_time <- NOW()

    WHILE (NOW() - start_time).seconds < timeout DO
        // Check container health status
        cmd <- FORMAT("docker inspect --format='{{{{.State.Health.Status}}}}' {}", service)
        result <- EXECUTE_COMMAND(cmd)

        IF result.exit_code == 0 THEN
            status <- result.stdout.trim()

            IF status == "healthy" THEN
                RETURN true
            ELSE IF status == "unhealthy" THEN
                RETURN false
            END IF
            // "starting" - continue waiting
        END IF

        SLEEP(2 seconds)
    END WHILE

    // Timeout reached
    log_warn(FORMAT("Health check timeout for {}", service))
    RETURN false
END
```

---

## 7. Device State Tracker

### 7.1 Update Device State

```
ALGORITHM: update_device_state
INPUT:
    context: HandlerContext
OUTPUT:
    DeviceState

CONSTANTS:
    STATE_DIR <- "/var/ndp"
    VERSION_FILE <- "/var/ndp/deployed-version"
    TIMESTAMP_FILE <- "/var/ndp/deployed-at"
    MANIFEST_HASH_FILE <- "/var/ndp/manifest-applied"

BEGIN
    // Ensure state directory exists
    CREATE_DIRECTORY_IF_NOT_EXISTS(STATE_DIR)

    // Get current git SHA
    git_sha <- EXECUTE_COMMAND("git rev-parse HEAD")
    git_sha <- git_sha.trim()

    // Get current timestamp
    timestamp <- NOW().format_iso8601()

    // Calculate manifest hash
    manifest_content <- READ_FILE(FORMAT("{}/.deploy/manifest.json", context.repo_root))
    manifest_hash <- SHA256(manifest_content)

    // Write state files
    WRITE_FILE(VERSION_FILE, git_sha)
    WRITE_FILE(TIMESTAMP_FILE, timestamp)
    WRITE_FILE(MANIFEST_HASH_FILE, manifest_hash)

    log_info(FORMAT("Updated device state: version={}, deployed_at={}", git_sha, timestamp))

    RETURN DeviceState {
        deployed_version: git_sha,
        deployed_at: timestamp,
        manifest_hash: manifest_hash
    }
END
```

### 7.2 Read Device State

```
ALGORITHM: read_device_state
OUTPUT:
    Optional<DeviceState>

BEGIN
    IF NOT FILE_EXISTS(VERSION_FILE) THEN
        RETURN NULL
    END IF

    TRY
        version <- READ_FILE(VERSION_FILE).trim()
        timestamp <- READ_FILE(TIMESTAMP_FILE).trim()
        manifest_hash <- READ_FILE(MANIFEST_HASH_FILE).trim()

        RETURN DeviceState {
            deployed_version: version,
            deployed_at: timestamp,
            manifest_hash: manifest_hash
        }
    CATCH error
        log_warn(FORMAT("Failed to read device state: {}", error.message))
        RETURN NULL
    END TRY
END
```

### 7.3 Check for Drift

```
ALGORITHM: check_deployment_drift
INPUT:
    repo_root: Path
OUTPUT:
    DriftReport

BEGIN
    current_state <- read_device_state()

    IF current_state IS NULL THEN
        RETURN DriftReport {
            has_drift: true,
            reason: "No deployment state found",
            current_version: NULL,
            repo_version: get_current_git_sha(repo_root)
        }
    END IF

    repo_sha <- get_current_git_sha(repo_root)

    IF current_state.deployed_version != repo_sha THEN
        RETURN DriftReport {
            has_drift: true,
            reason: "Git SHA mismatch",
            current_version: current_state.deployed_version,
            repo_version: repo_sha
        }
    END IF

    // Check manifest hash
    manifest_content <- READ_FILE(FORMAT("{}/.deploy/manifest.json", repo_root))
    manifest_hash <- SHA256(manifest_content)

    IF current_state.manifest_hash != manifest_hash THEN
        RETURN DriftReport {
            has_drift: true,
            reason: "Manifest changed since last deployment",
            current_version: current_state.deployed_version,
            repo_version: repo_sha
        }
    END IF

    RETURN DriftReport {
        has_drift: false,
        current_version: current_state.deployed_version,
        repo_version: repo_sha
    }
END
```

---

## 8. Main Deploy Entry Point

### 8.1 deploy.sh apply Command

```
ALGORITHM: deploy_apply
INPUT:
    options: DeployOptions
OUTPUT:
    ExitCode

STRUCT DeployOptions:
    dry_run: Boolean                    # --dry-run: Validate only
    verbose: Boolean                    # --verbose: Detailed output
    skip_validation: Boolean            # --skip-validation: Skip dp-019 validation
    force: Boolean                      # --force: Deploy even with drift

BEGIN
    log_info("Starting declarative deploy...")

    // ========================================
    // PHASE 1: Parse manifest
    // ========================================
    manifest_path <- ".deploy/manifest.json"
    schema_path <- "schemas/manifest.schema.json"

    manifest_result <- parse_manifest(manifest_path, schema_path)

    IF manifest_result IS error THEN
        log_error(FORMAT("Manifest parsing failed: {}", manifest_result.error.message))
        RETURN EXIT_FAILURE
    END IF

    manifest <- manifest_result.value
    log_info(FORMAT("Parsed manifest: {} declarations", LENGTH(manifest.changes)))

    // ========================================
    // PHASE 2: Validate all configs (dp-019)
    // ========================================
    IF NOT options.skip_validation THEN
        log_info("Validating configurations...")

        validation_result <- validate_all_declared_configs(manifest, options)

        IF NOT validation_result.valid THEN
            log_error("Configuration validation FAILED")

            FOR error IN validation_result.errors DO
                log_error(FORMAT("  [{}] {}: {}", error.layer, error.path, error.message))
            END FOR

            log_error("Deploy aborted. Fix errors above and retry.")
            RETURN EXIT_FAILURE
        END IF

        log_info("Configuration validation PASSED")
    END IF

    // ========================================
    // PHASE 3: Build execution plan
    // ========================================
    plan <- build_execution_plan(manifest)

    log_info("Execution plan:")
    FOR phase IN plan.phases DO
        log_info(FORMAT("  Phase '{}': {} declarations (parallel: {})",
            phase.name, LENGTH(phase.declarations), phase.parallel))
    END FOR

    // ========================================
    // PHASE 4: Initialize context
    // ========================================
    context <- HandlerContext {
        repo_root: GET_REPO_ROOT(),
        config_dir: GET_CONFIG_DIR(),
        db_pool: AWAIT create_db_pool(),
        etcd_client: AWAIT create_etcd_client(),
        dry_run: options.dry_run,
        verbose: options.verbose
    }

    // ========================================
    // PHASE 5: Execute plan
    // ========================================
    handlers <- create_handler_registry()
    result <- execute_plan(plan, context, handlers)

    // ========================================
    // PHASE 6: Report results
    // ========================================
    IF result.success THEN
        log_info("Deploy completed successfully!")

        IF result.device_state IS NOT NULL THEN
            log_info(FORMAT("  Version: {}", result.device_state.deployed_version))
            log_info(FORMAT("  Deployed at: {}", result.device_state.deployed_at))
        END IF

        // Handle any pending stream restarts
        IF LENGTH(context.streams_requiring_restart) > 0 THEN
            log_info(FORMAT("Streams requiring restart: {}",
                JOIN(context.streams_requiring_restart, ", ")))

            IF NOT options.dry_run THEN
                trigger_app_restart()
            END IF
        END IF

        RETURN EXIT_SUCCESS
    ELSE
        log_error("Deploy FAILED")

        FOR error IN result.errors DO
            log_error(FORMAT("  [{}] {}: {}",
                error.declaration_type, error.declaration_id, error.message))
        END FOR

        RETURN EXIT_FAILURE
    END IF
END
```

### 8.2 Complexity Analysis: Main Deploy

| Phase | Time Complexity | Space Complexity |
|-------|-----------------|------------------|
| Manifest parsing | O(n * s) | O(n + e) |
| Config validation | O(c * v) | O(e) |
| Plan building | O(d) | O(d) |
| Plan execution | O(d * h) | O(d) |
| State update | O(1) | O(1) |
| **Total** | O(n * s + c * v + d * h) | O(n + d + e) |

Where: n = manifest size, s = schema size, c = configs, v = validation cost, d = declarations, h = handler cost, e = errors

---

## 9. Error Code Reference

### 9.1 Manifest Errors (100-199)

| Code | Name | Description |
|------|------|-------------|
| 100 | MANIFEST_NOT_FOUND | .deploy/manifest.json does not exist |
| 101 | MANIFEST_SYNTAX_ERROR | Invalid JSON syntax |
| 102 | MANIFEST_SCHEMA_INVALID | Does not conform to manifest.schema.json |
| 103 | MANIFEST_DESERIALIZE_FAILED | Failed to parse into typed structures |
| 104 | MANIFEST_REFERENCES_INVALID | References non-existent configs |

### 9.2 Stream Errors (200-299)

| Code | Name | Description |
|------|------|-------------|
| 200 | STREAM_CONFIG_NOT_FOUND | Stream config file not found |
| 201 | STREAM_VALIDATION_FAILED | Config failed dp-019 validation |
| 202 | ETCD_SYNC_FAILED | Failed to sync config to etcd |
| 203 | STREAM_RELOAD_FAILED | Failed to trigger reload |

### 9.3 Silver Table Errors (300-399)

| Code | Name | Description |
|------|------|-------------|
| 300 | SILVER_ETL_DISABLED | silver_etl not enabled in config |
| 301 | INVALID_TABLE_NAME | target_table not in schema.table format |
| 302 | NO_FIELD_MAPPINGS | field_mappings array is empty |
| 303 | MISSING_TARGET_COLUMN | field_mapping missing target_column |
| 304 | MISSING_TYPE | field_mapping missing type |
| 305 | DDL_GENERATION_FAILED | Failed to generate DDL |
| 306 | DDL_EXECUTION_FAILED | Failed to execute DDL |

### 9.4 Migration Errors (400-499)

| Code | Name | Description |
|------|------|-------------|
| 400 | MIGRATION_NOT_FOUND | Migration SQL file not found |
| 401 | EMPTY_MIGRATION | Migration file is empty |
| 402 | INVALID_MIGRATION_NAME | Filename doesn't match pattern |
| 403 | MIGRATION_FAILED | SQL execution failed |

### 9.5 Dimension Errors (500-599)

| Code | Name | Description |
|------|------|-------------|
| 500 | DIMENSION_NOT_FOUND | Dimension config not found |
| 501 | DIMENSION_DIR_NOT_FOUND | Dimensions directory not found |
| 502 | DIMENSION_SYNC_FAILED | Failed to sync dimension data |

### 9.6 Dictionary Errors (600-699)

| Code | Name | Description |
|------|------|-------------|
| 600 | NO_DATABASE | Database connection required |
| 601 | DICTIONARY_SYNC_FAILED | Failed to sync data dictionary |

### 9.7 Container Errors (750-799)

| Code | Name | Description |
|------|------|-------------|
| 750 | CONTAINER_BUILD_FAILED | Container build failed |
| 751 | CONTAINER_RESTART_FAILED | Container restart failed |
| 752 | CONTAINER_UNHEALTHY | Container unhealthy after restart |

### 9.8 Execution Errors (700-749)

| Code | Name | Description |
|------|------|-------------|
| 700 | NO_HANDLER | No handler for declaration type |
| 701 | EXECUTION_FAILED | Handler execution failed |
| 702 | PHASE_FAILED | Critical phase failed |

---

## 10. Design Patterns Used

### 10.1 Handler Pattern (Strategy)

```
// Each declaration type has its own handler
// Handlers are registered and selected dynamically

INTERFACE DeclarationHandler:
    can_handle(declaration) -> Boolean
    validate(declaration, context) -> ValidationResult
    execute(declaration, context) -> ExecutionResult

// Registry pattern for handler lookup
handlers <- [StreamHandler, SilverTableHandler, MigrationHandler, ...]

FOR declaration IN manifest.changes DO
    handler <- handlers.find(h -> h.can_handle(declaration))
    result <- handler.execute(declaration, context)
END FOR
```

### 10.2 Builder Pattern (DDL Generation)

```
// DDL is built incrementally through chained operations
statements <- []
statements <- statements + create_table_statement(...)
statements <- statements + create_index_statements(...)
statements <- statements + create_hypertable_statement(...)
statements <- statements + create_policy_statements(...)
statements <- statements + create_grant_statements(...)

RETURN GeneratedDDL { statements: statements }
```

### 10.3 Phased Execution Pattern

```
// Declarations grouped into ordered phases
// Each phase can be parallel or sequential

phases <- [
    ("migrations", sequential, [m1, m2]),
    ("silver-tables", parallel, [t1, t2, t3]),
    ("streams", parallel, [s1, s2]),
    ("dimensions", parallel, [d1]),
    ("dictionary", sequential, [dict])
]

FOR phase IN phases DO
    IF phase.parallel THEN
        PARALLEL_FOR_EACH(phase.declarations, execute)
    ELSE
        FOR_EACH(phase.declarations, execute)
    END IF
END FOR
```

### 10.4 Idempotency Pattern

```
// All DDL uses IF NOT EXISTS or DO blocks

// CREATE TABLE IF NOT EXISTS
"CREATE TABLE IF NOT EXISTS silver.readings (...);"

// CREATE INDEX IF NOT EXISTS
"CREATE INDEX IF NOT EXISTS idx_time ON silver.readings (timestamp);"

// ADD COLUMN with existence check
"DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'readings' AND column_name = 'new_col'
    ) THEN
        ALTER TABLE silver.readings ADD COLUMN new_col TEXT;
    END IF;
END $$;"

// Hypertable with if_not_exists
"SELECT create_hypertable('silver.readings', 'timestamp', if_not_exists => TRUE);"
```

---

## 11. Extensibility: Adding New Declaration Types

### 11.1 Steps to Add a New Declaration Type

```
1. Define declaration structure:
   STRUCT NewDeclaration:
       type: "new-type"
       // Type-specific fields

2. Add to DeclarationType enum:
   ENUM DeclarationType:
       ...
       NEW_TYPE

3. Update manifest.schema.json:
   {
     "type": "object",
     "properties": {
       "type": {"const": "new-type"},
       // Type-specific properties
     }
   }

4. Implement handler:
   CLASS NewTypeHandler IMPLEMENTS DeclarationHandler:
       can_handle(d) -> d.type == NEW_TYPE
       validate(d, ctx) -> // Validation logic
       execute(d, ctx) -> // Execution logic

5. Register handler:
   handlers <- [..., NewTypeHandler {}]

6. Add to phase order (if needed):
   PHASE_ORDER <- [
       ...,
       ("new-phase", [NEW_TYPE])
   ]
```

### 11.2 Example: Adding "continuous-aggregate" Declaration

```
// 1. Define structure
STRUCT ContinuousAggregateDeclaration:
    type: "continuous-aggregate"
    name: String                    # Aggregate name
    source_table: String            # Source hypertable
    time_bucket: String             # e.g., "1 hour"
    refresh_policy: RefreshPolicy

// 2. Implement handler
CLASS ContinuousAggregateHandler IMPLEMENTS DeclarationHandler:
    can_handle(d) -> d.type == CONTINUOUS_AGGREGATE

    validate(d, ctx):
        // Check source table exists
        // Validate time_bucket format
        // Validate refresh policy

    execute(d, ctx):
        // Generate CREATE MATERIALIZED VIEW WITH timescaledb.continuous
        // Add refresh policy
```

---

## 12. Summary and Performance Targets

### 12.1 Overall Complexity

| Component | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Manifest parsing | O(n * s) | O(n) |
| Plan building | O(d) | O(d) |
| Stream sync | O(s) | O(s) per stream |
| DDL generation | O(f) | O(f + s) |
| Migration execution | O(m) | O(m) |
| Dictionary sync | O(c * a) | O(c * a) |
| State update | O(1) | O(1) |

Where: n = manifest size, s = schema size, d = declarations, f = field_mappings, m = migration size, c = streams, a = attributes

### 12.2 Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Manifest parse | < 50ms | JSON parsing + schema validation |
| Config validation | < 500ms | Full dp-019 validation per stream |
| DDL generation | < 100ms | Per table |
| DDL execution | < 2s | Per table (includes hypertable) |
| Full deploy (small) | < 10s | 1-3 declarations |
| Full deploy (medium) | < 30s | 5-10 declarations |
| Full deploy (large) | < 60s | 20+ declarations |

---

## 13. References

| Document | Purpose |
|----------|---------|
| `dp-020/SCOPE.md` | Requirements and acceptance criteria |
| `dp-016/architecture/ADR-016-002-declarative-deploy.md` | Architecture decision |
| `dp-019/specification/SUPPORTED-VALUES-RESEARCH.md` | Type mappings |
| `dp-019/pseudocode/PSEUDOCODE.md` | Validation algorithms |
| `dp-015/SCOPE.md` | DDL generation requirements (absorbed) |
| `deploy/pi/deploy.sh` | Current deployment script |

---

*Pseudocode created: 2026-02-02*
*SPARC Phase: Pseudocode (P)*
*Next Phase: Architecture (A) - ADRs and system design*
*Then: Refinement (R) - TDD Implementation*
