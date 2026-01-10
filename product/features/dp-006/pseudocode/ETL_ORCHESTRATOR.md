# ETL Orchestrator Pseudocode

**Document**: ETL_ORCHESTRATOR.md
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Pseudocode Specialist
**Feature**: DP-006 (Silver Layer Implementation)
**Phase**: SPARC Pseudocode

---

## 1. Overview

This document defines the algorithmic design for the Silver ETL Orchestrator - the main entry point for the `silver-etl` binary. The orchestrator coordinates configuration loading, DuckDB initialization, stream processing, and metrics reporting.

### Design Principles

1. **Graceful Degradation**: One stream failure does not stop others
2. **Incremental Processing**: Only process new data since last watermark
3. **Config-Driven**: All behavior controlled by YAML configuration
4. **Observable**: Comprehensive metrics and logging
5. **Idempotent**: Re-running produces identical results

---

## 2. Data Structures

### 2.1 Core Types

```
TYPE Timestamp = i64  // Unix microseconds

TYPE StreamId = String  // e.g., "air-quality", "outdoor-weather"

TYPE TableName = String  // e.g., "silver.air_quality_observations"

STRUCT CliArgs:
    stream: Option<StreamId>      // --stream <ID>: process single stream
    full_reload: bool             // --full-reload: ignore watermark
    dry_run: bool                 // --dry-run: generate SQL only
    config_source: ConfigSource   // --config-source: etcd | yaml

ENUM ConfigSource:
    Etcd
    Yaml

STRUCT SilverEtlConfig:
    enabled: bool
    target_table: TableName
    target_schema: String
    timestamp: TimestampMapping
    identity_fields: Vec<IdentityField>
    field_mappings: Vec<SilverFieldMapping>
    dq_rules: Vec<DqRule>
    dq_output: DqOutputConfig
    deduplication: DeduplicationConfig
    incremental: IncrementalConfig

STRUCT StreamResult:
    stream_id: StreamId
    rows_processed: usize
    rows_flagged: usize           // Rows with DQ flags
    rows_rejected: usize          // Rows with reject action
    rows_dropped: usize           // Rows dropped by drop action
    duration_ms: u64
    watermark_before: Option<Timestamp>
    watermark_after: Option<Timestamp>
    error: Option<String>

STRUCT EtlReport:
    start_time: Timestamp
    end_time: Timestamp
    total_duration_ms: u64
    streams_attempted: usize
    streams_succeeded: usize
    streams_failed: usize
    stream_results: Vec<StreamResult>
    total_rows_processed: usize
    total_rows_flagged: usize
    config_source: ConfigSource
    dry_run: bool

ENUM EtlError:
    ConfigLoadError { message: String }
    DuckDbInitError { message: String }
    PostgresConnectionError { message: String }
    ParquetReadError { stream_id: StreamId, message: String }
    SqlExecutionError { stream_id: StreamId, sql: String, message: String }
    WatermarkError { table: TableName, message: String }
    ValidationError { stream_id: StreamId, message: String }
```

### 2.2 DuckDB Connection Context

```
STRUCT DuckDbContext:
    connection: DuckDB::Connection
    postgres_attached: bool
    parquet_loaded: bool
    data_path: String             // /data/raw

STRUCT PostgresConfig:
    host: String                  // NDP_TIMESCALE_HOST
    port: u16                     // NDP_TIMESCALE_PORT
    database: String              // NDP_TIMESCALE_DB
    user: String                  // NDP_TIMESCALE_USER
    password: String              // NDP_TIMESCALE_PASSWORD (from env/secret)
```

---

## 3. Function Signatures

```
// Entry point
FUNCTION main() -> Result<(), EtlError>

// Initialization
FUNCTION parse_cli_args() -> CliArgs
FUNCTION init_logging(verbosity: LogLevel) -> ()
FUNCTION init_duckdb(config: PostgresConfig, data_path: String) -> Result<DuckDbContext, EtlError>
FUNCTION load_postgres_config_from_env() -> Result<PostgresConfig, EtlError>

// Configuration
FUNCTION load_stream_configs(source: ConfigSource) -> Result<Vec<(StreamId, SilverEtlConfig)>, EtlError>
FUNCTION load_configs_from_etcd(endpoints: Vec<String>) -> Result<Vec<(StreamId, SilverEtlConfig)>, EtlError>
FUNCTION load_configs_from_yaml(config_path: String) -> Result<Vec<(StreamId, SilverEtlConfig)>, EtlError>
FUNCTION filter_enabled_configs(configs: Vec<(StreamId, SilverEtlConfig)>) -> Vec<(StreamId, SilverEtlConfig)>

// Processing
FUNCTION process_all_streams(
    ctx: &DuckDbContext,
    configs: Vec<(StreamId, SilverEtlConfig)>,
    args: &CliArgs
) -> EtlReport

FUNCTION process_stream(
    ctx: &DuckDbContext,
    stream_id: &StreamId,
    config: &SilverEtlConfig,
    args: &CliArgs
) -> StreamResult

// Watermark Management
FUNCTION get_watermark(
    ctx: &DuckDbContext,
    table: &TableName,
    column: &str
) -> Result<Option<Timestamp>, EtlError>

FUNCTION calculate_watermark_filter(
    watermark: Option<Timestamp>,
    lag_interval: &str,
    full_reload: bool
) -> String

// SQL Execution
FUNCTION generate_etl_sql(
    stream_id: &StreamId,
    config: &SilverEtlConfig,
    watermark_filter: &str,
    data_path: &str
) -> Result<String, EtlError>

FUNCTION execute_etl(
    ctx: &DuckDbContext,
    sql: &str,
    dry_run: bool
) -> Result<usize, EtlError>

// Metrics and Logging
FUNCTION report_metrics(report: &EtlReport) -> ()
FUNCTION log_stream_result(result: &StreamResult) -> ()
FUNCTION determine_exit_code(report: &EtlReport) -> i32
```

---

## 4. Main Algorithm

### 4.1 Entry Point: main()

```
ALGORITHM: main
INPUT: command line arguments (implicit)
OUTPUT: Result<(), EtlError> with appropriate exit code

BEGIN
    // Step 1: Initialize logging
    init_logging(LogLevel::Info)
    log_info("Silver ETL starting", version: env!("CARGO_PKG_VERSION"))

    // Step 2: Parse CLI arguments
    args <- parse_cli_args()
    log_info("Configuration", {
        stream: args.stream,
        full_reload: args.full_reload,
        dry_run: args.dry_run,
        config_source: args.config_source
    })

    // Step 3: Load environment configuration
    TRY:
        pg_config <- load_postgres_config_from_env()
        data_path <- env::var("NDP_RAW_PATH").unwrap_or("/data/raw")
    CATCH error:
        log_error("Failed to load environment config", error)
        RETURN Err(EtlError::ConfigLoadError { message: error.to_string() })

    // Step 4: Initialize DuckDB with extensions
    TRY:
        duckdb_ctx <- init_duckdb(pg_config, data_path)
        log_info("DuckDB initialized", {
            postgres_attached: duckdb_ctx.postgres_attached,
            parquet_loaded: duckdb_ctx.parquet_loaded,
            data_path: duckdb_ctx.data_path
        })
    CATCH error:
        log_error("Failed to initialize DuckDB", error)
        RETURN Err(error)

    // Step 5: Load stream configurations
    TRY:
        all_configs <- load_stream_configs(args.config_source)
        enabled_configs <- filter_enabled_configs(all_configs)

        // Filter to single stream if specified
        IF args.stream IS Some(stream_id) THEN:
            enabled_configs <- enabled_configs
                .filter(|(id, _)| id == stream_id)
            IF enabled_configs.is_empty() THEN:
                log_error("Stream not found or not enabled", stream_id)
                RETURN Err(EtlError::ConfigLoadError {
                    message: format!("Stream '{}' not found or silver_etl.enabled=false", stream_id)
                })
        END IF

        log_info("Loaded configurations", {
            total_streams: all_configs.len(),
            enabled_streams: enabled_configs.len(),
            streams: enabled_configs.iter().map(|(id, _)| id).collect()
        })
    CATCH error:
        log_error("Failed to load configurations", error)
        RETURN Err(error)

    // Step 6: Process all streams
    report <- process_all_streams(&duckdb_ctx, enabled_configs, &args)

    // Step 7: Report metrics and results
    report_metrics(&report)

    // Step 8: Log summary
    log_info("Silver ETL completed", {
        duration_ms: report.total_duration_ms,
        streams_succeeded: report.streams_succeeded,
        streams_failed: report.streams_failed,
        total_rows: report.total_rows_processed,
        total_flagged: report.total_rows_flagged
    })

    // Step 9: Exit with appropriate code
    exit_code <- determine_exit_code(&report)
    IF exit_code != 0 THEN:
        log_warn("ETL completed with failures", exit_code)
    END IF

    std::process::exit(exit_code)
    RETURN Ok(())
END
```

### 4.2 Exit Code Logic

```
ALGORITHM: determine_exit_code
INPUT: report (EtlReport)
OUTPUT: exit_code (i32)

BEGIN
    // Exit codes:
    // 0 = All streams succeeded
    // 1 = All streams failed
    // 2 = Partial success (some streams failed)
    // 3 = No streams to process

    IF report.streams_attempted == 0 THEN:
        RETURN 3  // No work to do
    END IF

    IF report.streams_failed == 0 THEN:
        RETURN 0  // Complete success
    END IF

    IF report.streams_succeeded == 0 THEN:
        RETURN 1  // Complete failure
    END IF

    RETURN 2  // Partial success
END
```

---

## 5. DuckDB Initialization

### 5.1 init_duckdb()

```
ALGORITHM: init_duckdb
INPUT:
    pg_config (PostgresConfig)
    data_path (String)
OUTPUT: Result<DuckDbContext, EtlError>

BEGIN
    // Step 1: Create in-memory DuckDB connection
    TRY:
        conn <- duckdb::Connection::open_in_memory()
    CATCH error:
        RETURN Err(EtlError::DuckDbInitError {
            message: format!("Failed to create DuckDB connection: {}", error)
        })

    // Step 2: Install and load Parquet extension
    TRY:
        conn.execute("INSTALL parquet")
        conn.execute("LOAD parquet")
        log_debug("Parquet extension loaded")
    CATCH error:
        RETURN Err(EtlError::DuckDbInitError {
            message: format!("Failed to load Parquet extension: {}", error)
        })

    // Step 3: Install and load PostgreSQL extension
    TRY:
        conn.execute("INSTALL postgres")
        conn.execute("LOAD postgres")
        log_debug("PostgreSQL extension loaded")
    CATCH error:
        RETURN Err(EtlError::DuckDbInitError {
            message: format!("Failed to load PostgreSQL extension: {}", error)
        })

    // Step 4: Attach PostgreSQL database as 'pg'
    attach_sql <- format!(
        "ATTACH 'host={} port={} dbname={} user={} password={}' AS pg (TYPE postgres)",
        pg_config.host,
        pg_config.port,
        pg_config.database,
        pg_config.user,
        pg_config.password
    )

    TRY:
        conn.execute(attach_sql)
        log_info("PostgreSQL attached", {
            host: pg_config.host,
            port: pg_config.port,
            database: pg_config.database
        })
    CATCH error:
        RETURN Err(EtlError::PostgresConnectionError {
            message: format!("Failed to attach PostgreSQL: {}", error)
        })

    // Step 5: Verify connection with simple query
    TRY:
        result <- conn.execute("SELECT 1 FROM pg.information_schema.tables LIMIT 1")
        log_debug("PostgreSQL connection verified")
    CATCH error:
        RETURN Err(EtlError::PostgresConnectionError {
            message: format!("PostgreSQL verification failed: {}", error)
        })

    // Step 6: Return initialized context
    RETURN Ok(DuckDbContext {
        connection: conn,
        postgres_attached: true,
        parquet_loaded: true,
        data_path: data_path
    })
END
```

---

## 6. Configuration Loading

### 6.1 load_stream_configs()

```
ALGORITHM: load_stream_configs
INPUT: source (ConfigSource)
OUTPUT: Result<Vec<(StreamId, SilverEtlConfig)>, EtlError>

BEGIN
    MATCH source:
        ConfigSource::Etcd:
            endpoints <- env::var("NDP_ETCD_ENDPOINTS")
                .unwrap_or("http://etcd:2379")
                .split(',')
                .collect()
            RETURN load_configs_from_etcd(endpoints)

        ConfigSource::Yaml:
            config_path <- env::var("NDP_CONFIG_PATH")
                .unwrap_or("/config/base/streams")
            RETURN load_configs_from_yaml(config_path)
END
```

### 6.2 load_configs_from_etcd()

```
ALGORITHM: load_configs_from_etcd
INPUT: endpoints (Vec<String>)
OUTPUT: Result<Vec<(StreamId, SilverEtlConfig)>, EtlError>

BEGIN
    // Step 1: Connect to etcd
    TRY:
        client <- etcd_client::Client::connect(endpoints)
    CATCH error:
        RETURN Err(EtlError::ConfigLoadError {
            message: format!("Failed to connect to etcd: {}", error)
        })

    // Step 2: List all stream keys
    TRY:
        response <- client.get_prefix("/streams/")
        stream_keys <- response.kvs()
            .filter(|kv| kv.key().ends_with("/config"))
            .map(|kv| extract_stream_id(kv.key()))
            .collect()
    CATCH error:
        RETURN Err(EtlError::ConfigLoadError {
            message: format!("Failed to list streams from etcd: {}", error)
        })

    // Step 3: Load each stream's silver_etl config
    configs <- Vec::new()
    FOR stream_id IN stream_keys DO:
        key <- format!("/streams/{}/config", stream_id)
        TRY:
            response <- client.get(key)
            IF response.kvs().is_empty() THEN:
                log_warn("No config found for stream", stream_id)
                CONTINUE
            END IF

            yaml_content <- response.kvs()[0].value_str()
            full_config <- serde_yaml::from_str(yaml_content)

            IF full_config.silver_etl IS Some(silver_config) THEN:
                configs.push((stream_id, silver_config))
            ELSE:
                log_debug("Stream has no silver_etl config", stream_id)
            END IF
        CATCH error:
            log_warn("Failed to load config for stream", {
                stream_id: stream_id,
                error: error.to_string()
            })
            // Continue with other streams
        END TRY
    END FOR

    RETURN Ok(configs)
END
```

### 6.3 filter_enabled_configs()

```
ALGORITHM: filter_enabled_configs
INPUT: configs (Vec<(StreamId, SilverEtlConfig)>)
OUTPUT: Vec<(StreamId, SilverEtlConfig)>

BEGIN
    RETURN configs.into_iter()
        .filter(|(stream_id, config)| {
            IF config.enabled THEN:
                true
            ELSE:
                log_debug("Stream disabled", stream_id)
                false
            END IF
        })
        .collect()
END
```

---

## 7. Stream Processing

### 7.1 process_all_streams()

```
ALGORITHM: process_all_streams
INPUT:
    ctx (&DuckDbContext)
    configs (Vec<(StreamId, SilverEtlConfig)>)
    args (&CliArgs)
OUTPUT: EtlReport

BEGIN
    start_time <- Timestamp::now()
    stream_results <- Vec::new()

    total_rows <- 0
    total_flagged <- 0
    succeeded <- 0
    failed <- 0

    // Process each stream independently
    // One failure does not stop others
    FOR (stream_id, config) IN configs DO:
        log_info("Processing stream", stream_id)

        result <- process_stream(ctx, &stream_id, &config, args)

        // Log result
        log_stream_result(&result)

        // Update counters
        IF result.error IS None THEN:
            succeeded <- succeeded + 1
            total_rows <- total_rows + result.rows_processed
            total_flagged <- total_flagged + result.rows_flagged
        ELSE:
            failed <- failed + 1
            log_error("Stream processing failed", {
                stream_id: stream_id,
                error: result.error
            })
        END IF

        stream_results.push(result)
    END FOR

    end_time <- Timestamp::now()

    RETURN EtlReport {
        start_time: start_time,
        end_time: end_time,
        total_duration_ms: (end_time - start_time).as_millis(),
        streams_attempted: configs.len(),
        streams_succeeded: succeeded,
        streams_failed: failed,
        stream_results: stream_results,
        total_rows_processed: total_rows,
        total_rows_flagged: total_flagged,
        config_source: args.config_source,
        dry_run: args.dry_run
    }
END
```

### 7.2 process_stream()

```
ALGORITHM: process_stream
INPUT:
    ctx (&DuckDbContext)
    stream_id (&StreamId)
    config (&SilverEtlConfig)
    args (&CliArgs)
OUTPUT: StreamResult

BEGIN
    stream_start <- Timestamp::now()

    // Step 1: Get current watermark from Silver table
    watermark_before <- None
    IF NOT args.full_reload AND config.incremental.enabled THEN:
        TRY:
            watermark_before <- get_watermark(
                ctx,
                &config.target_table,
                &config.incremental.watermark_column
            )
            log_debug("Retrieved watermark", {
                stream_id: stream_id,
                watermark: watermark_before
            })
        CATCH error:
            // Non-fatal: proceed with full load if watermark fails
            log_warn("Failed to get watermark, proceeding with full load", {
                stream_id: stream_id,
                error: error.to_string()
            })
        END TRY
    END IF

    // Step 2: Calculate watermark filter for SQL
    watermark_filter <- calculate_watermark_filter(
        watermark_before,
        &config.incremental.lag_interval,
        args.full_reload
    )

    // Step 3: Generate ETL SQL
    TRY:
        etl_sql <- generate_etl_sql(
            stream_id,
            config,
            &watermark_filter,
            &ctx.data_path
        )

        IF args.dry_run THEN:
            log_info("Dry run SQL", {
                stream_id: stream_id,
                sql: etl_sql
            })
            // Write SQL to stdout or file for inspection
            println!("-- Stream: {}\n{}\n", stream_id, etl_sql)
        END IF
    CATCH error:
        RETURN StreamResult {
            stream_id: stream_id.clone(),
            rows_processed: 0,
            rows_flagged: 0,
            rows_rejected: 0,
            rows_dropped: 0,
            duration_ms: (Timestamp::now() - stream_start).as_millis(),
            watermark_before: watermark_before,
            watermark_after: None,
            error: Some(format!("SQL generation failed: {}", error))
        }

    // Step 4: Execute ETL (unless dry run)
    rows_processed <- 0
    IF NOT args.dry_run THEN:
        TRY:
            rows_processed <- execute_etl(ctx, &etl_sql, false)
            log_info("ETL executed", {
                stream_id: stream_id,
                rows: rows_processed
            })
        CATCH error:
            RETURN StreamResult {
                stream_id: stream_id.clone(),
                rows_processed: 0,
                rows_flagged: 0,
                rows_rejected: 0,
                rows_dropped: 0,
                duration_ms: (Timestamp::now() - stream_start).as_millis(),
                watermark_before: watermark_before,
                watermark_after: None,
                error: Some(format!("ETL execution failed: {}", error))
            }
        END TRY
    END IF

    // Step 5: Get new watermark after processing
    watermark_after <- None
    IF NOT args.dry_run AND config.incremental.enabled THEN:
        TRY:
            watermark_after <- get_watermark(
                ctx,
                &config.target_table,
                &config.incremental.watermark_column
            )
        CATCH error:
            log_warn("Failed to get new watermark", error)
        END TRY
    END IF

    // Step 6: Query DQ statistics
    rows_flagged <- 0
    rows_rejected <- 0
    rows_dropped <- 0
    IF NOT args.dry_run AND config.dq_output.enabled THEN:
        TRY:
            dq_stats <- query_dq_statistics(
                ctx,
                &config.target_table,
                &config.dq_output.target_column,
                watermark_before,
                watermark_after
            )
            rows_flagged <- dq_stats.flagged
            rows_rejected <- dq_stats.rejected
            rows_dropped <- dq_stats.dropped
        CATCH error:
            log_warn("Failed to query DQ statistics", error)
        END TRY
    END IF

    stream_end <- Timestamp::now()

    RETURN StreamResult {
        stream_id: stream_id.clone(),
        rows_processed: rows_processed,
        rows_flagged: rows_flagged,
        rows_rejected: rows_rejected,
        rows_dropped: rows_dropped,
        duration_ms: (stream_end - stream_start).as_millis(),
        watermark_before: watermark_before,
        watermark_after: watermark_after,
        error: None
    }
END
```

---

## 8. Watermark Management

### 8.1 get_watermark()

```
ALGORITHM: get_watermark
INPUT:
    ctx (&DuckDbContext)
    table (&TableName)
    column (&str)
OUTPUT: Result<Option<Timestamp>, EtlError>

BEGIN
    // Query max timestamp from Silver table
    sql <- format!(
        "SELECT MAX({}) AS watermark FROM pg.{}",
        column,
        table
    )

    TRY:
        result <- ctx.connection.query_row(sql)

        IF result.is_null("watermark") THEN:
            // Table is empty or column has no values
            log_debug("No watermark found (table empty or null)", table)
            RETURN Ok(None)
        END IF

        // Extract timestamp value
        // DuckDB returns TIMESTAMPTZ, convert to microseconds
        watermark_ts <- result.get::<Timestamp>("watermark")

        log_debug("Watermark retrieved", {
            table: table,
            watermark: watermark_ts
        })

        RETURN Ok(Some(watermark_ts))

    CATCH error:
        // Table might not exist yet
        IF error.message.contains("does not exist") THEN:
            log_debug("Table does not exist, no watermark", table)
            RETURN Ok(None)
        END IF

        RETURN Err(EtlError::WatermarkError {
            table: table.clone(),
            message: error.to_string()
        })
    END TRY
END
```

### 8.2 calculate_watermark_filter()

```
ALGORITHM: calculate_watermark_filter
INPUT:
    watermark (Option<Timestamp>)
    lag_interval (&str)
    full_reload (bool)
OUTPUT: watermark_filter (String)

BEGIN
    // Full reload: no filter
    IF full_reload THEN:
        log_info("Full reload requested, no watermark filter")
        RETURN "1=1"  // Always true
    END IF

    // No watermark: no filter (initial load)
    IF watermark IS None THEN:
        log_info("No existing watermark, initial load")
        RETURN "1=1"
    END IF

    // Calculate filter with lag interval for late arrivals
    // Lag interval is a safety buffer (e.g., "5 minutes")
    watermark_value <- watermark.unwrap()

    // Parse lag interval (e.g., "5 minutes" -> 300 seconds)
    lag_seconds <- parse_interval_to_seconds(lag_interval)

    // Adjust watermark back by lag interval
    adjusted_watermark <- watermark_value - (lag_seconds * 1_000_000)  // microseconds

    // Generate filter expression
    // Bronze timestamp is in microseconds
    filter <- format!(
        "timestamp > {}",
        adjusted_watermark
    )

    log_debug("Watermark filter calculated", {
        original_watermark: watermark_value,
        lag_interval: lag_interval,
        lag_seconds: lag_seconds,
        adjusted_watermark: adjusted_watermark,
        filter: filter
    })

    RETURN filter
END

FUNCTION parse_interval_to_seconds(interval: &str) -> i64:
    // Parse interval strings like "5 minutes", "1 hour", "30 seconds"
    parts <- interval.split_whitespace()
    IF parts.len() != 2 THEN:
        log_warn("Invalid interval format, defaulting to 300s", interval)
        RETURN 300
    END IF

    value <- parts[0].parse::<i64>().unwrap_or(5)
    unit <- parts[1].to_lowercase()

    MATCH unit:
        "second" | "seconds" | "s": RETURN value
        "minute" | "minutes" | "m": RETURN value * 60
        "hour" | "hours" | "h": RETURN value * 3600
        "day" | "days" | "d": RETURN value * 86400
        _:
            log_warn("Unknown interval unit, defaulting to minutes", unit)
            RETURN value * 60
END
```

---

## 9. SQL Generation and Execution

### 9.1 generate_etl_sql()

```
ALGORITHM: generate_etl_sql
INPUT:
    stream_id (&StreamId)
    config (&SilverEtlConfig)
    watermark_filter (&str)
    data_path (&str)
OUTPUT: Result<String, EtlError>

BEGIN
    // Build SQL components

    // 1. Build SELECT columns
    select_columns <- Vec::new()

    // Add ingestion timestamp
    select_columns.push("current_timestamp AS ingestion_time")

    // Add timestamp mapping
    ts_expr <- generate_timestamp_expr(&config.timestamp)
    select_columns.push(format!("{} AS {}", ts_expr, config.timestamp.target_field))

    // Add identity fields
    FOR field IN config.identity_fields DO:
        expr <- generate_identity_expr(&field)
        select_columns.push(format!("{} AS {}", expr, field.target))
    END FOR

    // Add field mappings with transforms
    FOR mapping IN config.field_mappings DO:
        expr <- generate_field_expr(&mapping)
        select_columns.push(format!("{} AS {}", expr, mapping.target_column))
    END FOR

    // Add DQ flags column if enabled
    IF config.dq_output.enabled THEN:
        dq_expr <- generate_dq_flags_expr(&config.field_mappings, &config.dq_rules)
        select_columns.push(format!("{} AS {}", dq_expr, config.dq_output.target_column))
    END IF

    // 2. Build FROM clause with Parquet glob
    parquet_path <- format!("{}/{}/**/*.parquet", data_path, stream_id)
    from_clause <- format!("read_parquet('{}')", parquet_path)

    // 3. Build WHERE clause
    where_clauses <- Vec::new()
    where_clauses.push(watermark_filter)

    // Add drop conditions (inverse - exclude dropped rows)
    drop_conditions <- generate_drop_conditions(&config.dq_rules)
    IF NOT drop_conditions.is_empty() THEN:
        where_clauses.push(format!("NOT ({})", drop_conditions.join(" OR ")))
    END IF

    // 4. Build INSERT statement
    target_columns <- get_target_column_names(config)

    // 5. Build ON CONFLICT clause for upsert
    conflict_clause <- ""
    IF config.deduplication.enabled THEN:
        key_cols <- config.deduplication.key_columns.join(", ")
        update_cols <- target_columns
            .iter()
            .filter(|c| NOT config.deduplication.key_columns.contains(c))
            .map(|c| format!("{} = EXCLUDED.{}", c, c))
            .join(", ")

        MATCH config.deduplication.strategy:
            DeduplicationStrategy::Upsert:
                conflict_clause <- format!(
                    "ON CONFLICT ({}) DO UPDATE SET {}",
                    key_cols, update_cols
                )
            DeduplicationStrategy::Skip:
                conflict_clause <- format!(
                    "ON CONFLICT ({}) DO NOTHING",
                    key_cols
                )
            DeduplicationStrategy::Replace:
                // Delete then insert (handled separately)
                conflict_clause <- format!(
                    "ON CONFLICT ({}) DO UPDATE SET {}",
                    key_cols, update_cols
                )
    END IF

    // 6. Assemble final SQL
    sql <- format!(
        "INSERT INTO pg.{} ({})\n\
         SELECT {}\n\
         FROM {}\n\
         WHERE {}\n\
         {}",
        config.target_table,
        target_columns.join(", "),
        select_columns.join(",\n       "),
        from_clause,
        where_clauses.join(" AND "),
        conflict_clause
    )

    // Validate SQL structure
    IF NOT validate_sql_syntax(&sql) THEN:
        RETURN Err(EtlError::ValidationError {
            stream_id: stream_id.clone(),
            message: "Generated SQL failed validation"
        })
    END IF

    RETURN Ok(sql)
END
```

### 9.2 execute_etl()

```
ALGORITHM: execute_etl
INPUT:
    ctx (&DuckDbContext)
    sql (&str)
    dry_run (bool)
OUTPUT: Result<usize, EtlError>

BEGIN
    IF dry_run THEN:
        log_debug("Dry run, skipping execution")
        RETURN Ok(0)
    END IF

    // Execute within transaction for atomicity
    TRY:
        ctx.connection.execute("BEGIN TRANSACTION")

        result <- ctx.connection.execute(sql)
        rows_affected <- result.rows_changed()

        ctx.connection.execute("COMMIT")

        log_debug("ETL transaction committed", rows_affected)
        RETURN Ok(rows_affected)

    CATCH error:
        // Attempt rollback
        TRY:
            ctx.connection.execute("ROLLBACK")
            log_warn("Transaction rolled back due to error")
        CATCH rollback_error:
            log_error("Rollback failed", rollback_error)
        END TRY

        RETURN Err(EtlError::SqlExecutionError {
            stream_id: "unknown".to_string(),
            sql: sql.to_string(),
            message: error.to_string()
        })
    END TRY
END
```

---

## 10. Metrics and Logging

### 10.1 report_metrics()

```
ALGORITHM: report_metrics
INPUT: report (&EtlReport)
OUTPUT: () (side effects: metrics emission)

BEGIN
    // Prometheus-style metrics

    // Counter: Total rows processed
    metrics::counter!(
        "silver_etl_rows_processed_total",
        report.total_rows_processed,
        "config_source" => report.config_source.to_string()
    )

    // Counter: Rows with DQ flags
    metrics::counter!(
        "silver_etl_rows_flagged_total",
        report.total_rows_flagged
    )

    // Histogram: ETL duration
    metrics::histogram!(
        "silver_etl_duration_seconds",
        report.total_duration_ms as f64 / 1000.0
    )

    // Per-stream metrics
    FOR result IN report.stream_results DO:
        labels <- [
            ("stream_id", result.stream_id.as_str()),
            ("status", IF result.error.is_some() THEN "failed" ELSE "success")
        ]

        // Rows processed per stream
        metrics::counter!(
            "silver_etl_stream_rows_total",
            result.rows_processed,
            &labels
        )

        // DQ metrics per stream
        metrics::counter!(
            "silver_etl_stream_flagged_total",
            result.rows_flagged,
            &labels
        )

        metrics::counter!(
            "silver_etl_stream_rejected_total",
            result.rows_rejected,
            &labels
        )

        metrics::counter!(
            "silver_etl_stream_dropped_total",
            result.rows_dropped,
            &labels
        )

        // Duration per stream
        metrics::histogram!(
            "silver_etl_stream_duration_seconds",
            result.duration_ms as f64 / 1000.0,
            &labels
        )
    END FOR

    // Counter: ETL runs
    metrics::counter!(
        "silver_etl_runs_total",
        1,
        "status" => IF report.streams_failed == 0 THEN "success"
                    ELSE IF report.streams_succeeded == 0 THEN "failed"
                    ELSE "partial"
    )

    // Gauge: Streams processed
    metrics::gauge!(
        "silver_etl_streams_succeeded",
        report.streams_succeeded as f64
    )

    metrics::gauge!(
        "silver_etl_streams_failed",
        report.streams_failed as f64
    )
END
```

### 10.2 log_stream_result()

```
ALGORITHM: log_stream_result
INPUT: result (&StreamResult)
OUTPUT: () (side effects: logging)

BEGIN
    IF result.error IS Some(error) THEN:
        log_error("Stream processing failed", {
            stream_id: result.stream_id,
            duration_ms: result.duration_ms,
            error: error
        })
    ELSE:
        log_info("Stream processed successfully", {
            stream_id: result.stream_id,
            rows_processed: result.rows_processed,
            rows_flagged: result.rows_flagged,
            rows_rejected: result.rows_rejected,
            rows_dropped: result.rows_dropped,
            duration_ms: result.duration_ms,
            watermark_before: result.watermark_before,
            watermark_after: result.watermark_after
        })
    END IF
END
```

---

## 11. CLI Argument Processing

### 11.1 parse_cli_args()

```
ALGORITHM: parse_cli_args
INPUT: implicit command line arguments
OUTPUT: CliArgs

BEGIN
    // Using clap derive pattern

    STRUCT CliArgs (clap::Parser):
        /// Process single stream only
        #[arg(short, long)]
        stream: Option<String>

        /// Ignore watermark, reload all data
        #[arg(long, default_value = "false")]
        full_reload: bool

        /// Generate SQL but don't execute
        #[arg(long, default_value = "false")]
        dry_run: bool

        /// Configuration source
        #[arg(long, value_enum, default_value = "etcd")]
        config_source: ConfigSource

        /// Verbosity level
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8

    args <- CliArgs::parse()

    // Validate combinations
    IF args.stream.is_some() AND args.full_reload THEN:
        log_warn("--full-reload with --stream will reload all data for that stream only")
    END IF

    RETURN args
END
```

---

## 12. Error Handling and Recovery

### 12.1 Error Recovery Patterns

```
ALGORITHM: with_retry
INPUT:
    operation: Fn() -> Result<T, EtlError>
    max_retries: usize
    retry_delay_ms: u64
OUTPUT: Result<T, EtlError>

BEGIN
    last_error <- None

    FOR attempt IN 1..=max_retries DO:
        TRY:
            result <- operation()
            RETURN Ok(result)
        CATCH error:
            last_error <- Some(error)

            IF attempt < max_retries THEN:
                log_warn("Operation failed, retrying", {
                    attempt: attempt,
                    max_retries: max_retries,
                    error: error.to_string(),
                    retry_in_ms: retry_delay_ms
                })
                sleep(Duration::from_millis(retry_delay_ms))
                // Exponential backoff
                retry_delay_ms <- retry_delay_ms * 2
            END IF
        END TRY
    END FOR

    log_error("All retries exhausted", {
        max_retries: max_retries,
        last_error: last_error
    })

    RETURN Err(last_error.unwrap())
END
```

### 12.2 Graceful Degradation

```
ALGORITHM: process_with_fallback
INPUT:
    primary_operation: Fn() -> Result<T, EtlError>
    fallback_operation: Fn() -> Result<T, EtlError>
    operation_name: &str
OUTPUT: Result<T, EtlError>

BEGIN
    TRY:
        RETURN primary_operation()
    CATCH primary_error:
        log_warn("Primary operation failed, attempting fallback", {
            operation: operation_name,
            primary_error: primary_error.to_string()
        })

        TRY:
            result <- fallback_operation()
            log_info("Fallback succeeded", operation_name)
            RETURN Ok(result)
        CATCH fallback_error:
            log_error("Both primary and fallback failed", {
                operation: operation_name,
                primary_error: primary_error.to_string(),
                fallback_error: fallback_error.to_string()
            })
            // Return primary error as it's more informative
            RETURN Err(primary_error)
        END TRY
    END TRY
END
```

---

## 13. Complexity Analysis

### 13.1 Time Complexity

| Function | Time Complexity | Notes |
|----------|-----------------|-------|
| `main()` | O(S * R) | S = streams, R = rows per stream |
| `init_duckdb()` | O(1) | Fixed extension loads |
| `load_stream_configs()` | O(S) | S = number of streams |
| `process_all_streams()` | O(S * R) | Iterates streams, processes rows |
| `process_stream()` | O(R) | R = rows in time window |
| `get_watermark()` | O(log N) | N = total rows, indexed query |
| `generate_etl_sql()` | O(F + D) | F = fields, D = DQ rules |
| `execute_etl()` | O(R) | R = rows affected |

### 13.2 Space Complexity

| Component | Space Complexity | Notes |
|-----------|------------------|-------|
| DuckDB Context | O(1) | Connection handle only |
| Stream Configs | O(S * F) | S = streams, F = fields per stream |
| ETL Report | O(S) | One result per stream |
| Generated SQL | O(F + D) | Proportional to config size |

### 13.3 Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Total ETL duration | < 60 seconds | For hourly batch, all streams |
| Per-stream processing | < 15 seconds | Average stream processing time |
| Memory usage | < 256 MB | Peak RSS during execution |
| Rows per second | > 10,000 | Throughput target |

---

## 14. Implementation Notes

### 14.1 DuckDB Considerations

```
NOTES:
1. DuckDB postgres extension requires pg_config in PATH for ARM64 builds
2. Use bundled feature for duckdb-rs to avoid system library issues
3. PostgreSQL ATTACH is persistent for connection lifetime
4. read_parquet() supports glob patterns for efficient file discovery
5. JSON functions: json_extract(), json_extract_string()
```

### 14.2 Error Handling Strategy

```
STRATEGY:
1. Config errors: Fail fast, clear error message
2. Connection errors: Retry with exponential backoff
3. Stream processing errors: Log and continue to next stream
4. SQL execution errors: Rollback transaction, record in result
5. Watermark errors: Fall back to full load for that stream
```

### 14.3 Logging Levels

```
LEVELS:
- ERROR: ETL failures, connection failures, data corruption
- WARN: DQ violations, retries, fallbacks, skipped streams
- INFO: ETL start/complete, row counts, stream processing
- DEBUG: SQL generation, watermark calculations, detailed metrics
- TRACE: Raw SQL, individual row processing (development only)
```

---

## 15. Test Scenarios

### 15.1 Unit Test Cases

```
TEST: test_watermark_filter_calculation
GIVEN: watermark = Some(1704067200000000), lag_interval = "5 minutes"
WHEN: calculate_watermark_filter() is called
THEN: filter = "timestamp > 1704066900000000"

TEST: test_watermark_filter_no_watermark
GIVEN: watermark = None
WHEN: calculate_watermark_filter() is called
THEN: filter = "1=1"

TEST: test_watermark_filter_full_reload
GIVEN: watermark = Some(1704067200000000), full_reload = true
WHEN: calculate_watermark_filter() is called
THEN: filter = "1=1"

TEST: test_exit_code_all_success
GIVEN: report with streams_succeeded = 4, streams_failed = 0
WHEN: determine_exit_code() is called
THEN: exit_code = 0

TEST: test_exit_code_partial_failure
GIVEN: report with streams_succeeded = 3, streams_failed = 1
WHEN: determine_exit_code() is called
THEN: exit_code = 2

TEST: test_exit_code_all_failed
GIVEN: report with streams_succeeded = 0, streams_failed = 4
WHEN: determine_exit_code() is called
THEN: exit_code = 1
```

### 15.2 Integration Test Cases

```
TEST: test_etl_happy_path
SETUP: Bronze Parquet files exist, TimescaleDB table exists
WHEN: process_stream() is called
THEN: Rows are inserted, watermark advances

TEST: test_etl_incremental_processing
SETUP: Previous ETL run exists with watermark
WHEN: process_stream() is called
THEN: Only rows after watermark are processed

TEST: test_etl_dq_flagging
SETUP: Bronze data contains out-of-range values
WHEN: process_stream() is called
THEN: dq_flags column populated, rows still inserted

TEST: test_etl_stream_isolation
SETUP: One stream has invalid config
WHEN: process_all_streams() is called
THEN: Other streams still process successfully

TEST: test_etl_dry_run
WHEN: process_stream() called with dry_run=true
THEN: SQL is generated but not executed, no DB changes
```

---

## 16. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Pseudocode Specialist | Initial pseudocode |

---

## 17. References

1. `product/features/dp-006/SCOPE.md` - Feature scope
2. `product/features/dp-006/specification/SPECIFICATION.md` - Requirements
3. `product/features/dp-006/architecture/ADR-006-002-binary-architecture.md` - Binary design
4. `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` - DQ framework
5. `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` - Config schema
6. DuckDB Rust API: https://duckdb.org/docs/api/rust
7. DuckDB PostgreSQL Extension: https://duckdb.org/docs/extensions/postgres
