# dp-013: CSV Source Type & Dimension Tables - Pseudocode

## Overview

This document defines the algorithmic design for extending NDP to support:
1. **CSV as a source type** for stream configs (timeseries batch data)
2. **Dimension table configs** for reference/lookup data

All algorithms follow NDP's existing patterns: Source/Store traits, config-driven architecture, and Bronze/Silver layer separation.

---

## 1. CSV Source Adapter

The CSV Source Adapter implements the `RawSource` trait to read CSV files and produce `RawDataPoint` records for Bronze layer storage.

### 1.1 Core Algorithm

```pseudocode
ALGORITHM: CsvSourceAdapter
IMPLEMENTS: RawSource trait

STRUCT CsvSourceConfig:
    stream_id: String
    path: Path                    // Relative to config root or absolute
    timestamp_field: String       // Column name for timestamps
    timestamp_format: TimestampFormat  // iso8601 | epoch_seconds | custom
    delimiter: Char               // Default: ','
    encoding: String              // Default: "utf-8"
    on_error: ErrorPolicy         // skip | abort
    entity_schemas: Vec<EntitySchema>  // Column mappings
    ndp_id: Option<String>        // Stable identifier
    context: Option<JSON>         // Mutable attributes

FUNCTION fetch_raw_batch(config: CsvSourceConfig) -> Result<Vec<RawDataPoint>>
INPUT: config (CsvSourceConfig)
OUTPUT: Vec<RawDataPoint> or Error

BEGIN
    // Phase 1: File validation
    IF NOT file_exists(config.path) THEN
        RETURN Err(SourceError::FileNotFound(config.path))
    END IF

    file_size <- get_file_size(config.path)
    IF file_size == 0 THEN
        LOG warning("Empty CSV file: {}", config.path)
        RETURN Ok([])  // No-op for empty files
    END IF

    // Phase 2: Initialize CSV reader
    reader <- CsvReader::new(config.path)
        .with_delimiter(config.delimiter)
        .with_encoding(config.encoding)
        .with_has_headers(true)

    // Phase 3: Validate headers against schema
    headers <- reader.read_headers()
    validation_result <- validate_headers(headers, config.entity_schemas, config.timestamp_field)

    IF validation_result.is_err() THEN
        RETURN Err(validation_result.error)
    END IF

    // Phase 4: Process rows
    raw_points <- []
    line_number <- 1  // Header is line 0
    errors <- []

    FOR EACH row IN reader DO
        line_number <- line_number + 1

        result <- process_csv_row(row, config, line_number)

        MATCH result:
            Ok(point) => raw_points.append(point)
            Skip => CONTINUE  // Row skipped due to error policy
            Err(e) =>
                IF config.on_error == ErrorPolicy::Abort THEN
                    RETURN Err(e)
                ELSE
                    errors.append(e)
                    CONTINUE
                END IF
    END FOR

    // Phase 5: Log summary
    LOG info("CSV ingest complete: {} rows processed, {} skipped, {} errors",
             line_number - 1, errors.len(), errors.len())

    RETURN Ok(raw_points)
END
```

**Time Complexity**: O(n * m) where n = row count, m = column count
**Space Complexity**: O(n) for storing all RawDataPoints in memory (batch mode)

### 1.2 Row Processing

```pseudocode
FUNCTION process_csv_row(row: CsvRow, config: CsvSourceConfig, line_number: u64)
    -> Result<RawDataPoint, Skip, Error>
INPUT:
    row: CsvRow - raw CSV row data
    config: CsvSourceConfig - source configuration
    line_number: u64 - for error reporting
OUTPUT: RawDataPoint, Skip signal, or Error

BEGIN
    TRY
        // Step 1: Extract and parse timestamp
        timestamp_value <- row.get(config.timestamp_field)

        IF timestamp_value IS NULL OR timestamp_value IS EMPTY THEN
            RETURN handle_error(
                RowError::MissingTimestamp(line_number),
                config.on_error
            )
        END IF

        timestamp <- parse_timestamp(timestamp_value, config.timestamp_format)

        IF timestamp.is_err() THEN
            RETURN handle_error(
                RowError::InvalidTimestamp(line_number, timestamp_value),
                config.on_error
            )
        END IF

        // Step 2: Build raw payload from all columns
        raw_payload <- JSON::object()

        FOR EACH column IN row.columns() DO
            value <- row.get(column)
            raw_payload[column] <- value  // Store as-is, no type conversion
        END FOR

        // Step 3: Create RawDataPoint
        source_id <- format!("{}-Csv", config.stream_id)

        point <- RawDataPoint::new(source_id, raw_payload)
            .with_timestamp(timestamp)
            .with_ndp_id_opt(config.ndp_id)
            .with_context_opt(config.context)

        RETURN Ok(point)

    CATCH parse_error
        RETURN handle_error(
            RowError::ParseError(line_number, parse_error.message),
            config.on_error
        )
    END TRY
END

FUNCTION handle_error(error: RowError, policy: ErrorPolicy) -> Result<Skip, Error>
BEGIN
    MATCH policy:
        ErrorPolicy::Skip =>
            LOG warning("Skipping row {}: {}", error.line_number, error.message)
            RETURN Skip
        ErrorPolicy::Abort =>
            RETURN Err(error)
END
```

**Time Complexity**: O(m) where m = column count
**Space Complexity**: O(m) for the JSON payload

---

## 2. Timestamp Parsing

```pseudocode
ENUM TimestampFormat:
    Iso8601           // "2024-01-15T10:30:00Z"
    EpochSeconds      // "1705315800"
    EpochMillis       // "1705315800000"
    Custom(String)    // strftime format like "%Y-%m-%d %H:%M:%S"

FUNCTION parse_timestamp(value: String, format: TimestampFormat) -> Result<DateTime<Utc>>
INPUT:
    value: String - raw timestamp string from CSV
    format: TimestampFormat - expected format
OUTPUT: DateTime<Utc> or Error

BEGIN
    // Trim whitespace
    value <- value.trim()

    IF value IS EMPTY THEN
        RETURN Err(TimestampError::Empty)
    END IF

    MATCH format:
        Iso8601 =>
            // Try RFC 3339 first, then fallback to other ISO formats
            result <- DateTime::parse_rfc3339(value)
            IF result.is_ok() THEN
                RETURN Ok(result.to_utc())
            END IF

            // Try without timezone (assume UTC)
            result <- NaiveDateTime::parse("%Y-%m-%dT%H:%M:%S", value)
            IF result.is_ok() THEN
                RETURN Ok(result.and_utc())
            END IF

            RETURN Err(TimestampError::InvalidFormat(value, "iso8601"))

        EpochSeconds =>
            seconds <- parse_i64(value)
            IF seconds.is_err() THEN
                RETURN Err(TimestampError::InvalidEpoch(value))
            END IF
            RETURN Ok(DateTime::from_unix_timestamp(seconds))

        EpochMillis =>
            millis <- parse_i64(value)
            IF millis.is_err() THEN
                RETURN Err(TimestampError::InvalidEpoch(value))
            END IF
            RETURN Ok(DateTime::from_unix_timestamp_millis(millis))

        Custom(pattern) =>
            result <- NaiveDateTime::parse(pattern, value)
            IF result.is_err() THEN
                RETURN Err(TimestampError::InvalidFormat(value, pattern))
            END IF
            // Custom formats assume UTC unless specified
            RETURN Ok(result.and_utc())
END
```

**Time Complexity**: O(k) where k = timestamp string length
**Space Complexity**: O(1)

---

## 3. Header Validation

```pseudocode
FUNCTION validate_headers(
    headers: Vec<String>,
    entity_schemas: Vec<EntitySchema>,
    timestamp_field: String
) -> Result<HeaderMapping>
INPUT:
    headers: Vec<String> - column names from CSV
    entity_schemas: Vec<EntitySchema> - expected schema mappings
    timestamp_field: String - required timestamp column
OUTPUT: HeaderMapping or ValidationError

BEGIN
    // Create header index for O(1) lookup
    header_set <- HashSet::from(headers)
    missing_required <- []

    // Step 1: Verify timestamp field exists
    IF NOT header_set.contains(timestamp_field) THEN
        RETURN Err(ValidationError::MissingColumn(timestamp_field, "timestamp_field"))
    END IF

    // Step 2: Verify all mapped source_fields exist
    FOR EACH schema IN entity_schemas DO
        FOR EACH field IN schema.fields DO
            source_field <- field.source_field OR field.name

            IF NOT header_set.contains(source_field) THEN
                IF field.required THEN
                    missing_required.append(source_field)
                ELSE
                    LOG warning("Optional column '{}' not found in CSV", source_field)
                END IF
            END IF
        END FOR
    END FOR

    IF NOT missing_required.is_empty() THEN
        RETURN Err(ValidationError::MissingRequiredColumns(missing_required))
    END IF

    // Step 3: Build column index mapping
    mapping <- HeaderMapping::new()
    FOR i, header IN headers.enumerate() DO
        mapping.add(header, i)
    END FOR

    RETURN Ok(mapping)
END
```

**Time Complexity**: O(h + f) where h = header count, f = total field count
**Space Complexity**: O(h) for the header set

---

## 4. Dimension Configuration Types

```pseudocode
STRUCT DimensionConfig:
    dimension_id: String          // kebab-case identifier
    target: DimensionTarget       // Where to load
    source: DimensionSource       // Where to read from
    schema: DimensionSchema       // Field definitions
    load: LoadConfig              // Load strategy

STRUCT DimensionTarget:
    table: String                 // e.g., "silver.entity_context"
    primary_key: Vec<String>      // e.g., ["ndp_id"]

STRUCT DimensionSource:
    type: SourceType              // Currently only "csv"
    path: Path                    // Relative or absolute path
    delimiter: Char               // Default: ','
    encoding: String              // Default: "utf-8"

STRUCT DimensionSchema:
    fields: Vec<DimensionField>

STRUCT DimensionField:
    name: String                  // Column name
    data_type: DataType           // text | integer | float | boolean | timestamp
    required: bool                // Default: false
    source_field: Option<String>  // CSV column name if different

STRUCT LoadConfig:
    strategy: LoadStrategy        // truncate_and_load | upsert

ENUM LoadStrategy:
    TruncateAndLoad               // DELETE all, INSERT new (default)
    Upsert                        // INSERT or UPDATE based on primary_key
```

---

## 5. Dimension Loader

```pseudocode
ALGORITHM: DimensionLoader

FUNCTION load_dimension(config: DimensionConfig, db: Database) -> Result<LoadStats>
INPUT:
    config: DimensionConfig
    db: Database connection
OUTPUT: LoadStats or Error

BEGIN
    // Phase 1: Validate configuration
    validation_result <- validate_dimension_config(config)
    IF validation_result.is_err() THEN
        RETURN Err(validation_result.error)
    END IF

    // Phase 2: Read and parse source CSV
    records <- read_dimension_csv(config.source, config.schema)
    IF records.is_err() THEN
        RETURN Err(records.error)
    END IF

    IF records.is_empty() THEN
        LOG warning("Empty dimension source: {}", config.source.path)
        RETURN Ok(LoadStats::empty())
    END IF

    // Phase 3: Ensure target table exists
    ensure_table_exists(db, config.target.table, config.schema)

    // Phase 4: Execute load strategy
    stats <- MATCH config.load.strategy:
        TruncateAndLoad => truncate_and_load(db, config.target, records)
        Upsert => upsert(db, config.target, records, config.schema)

    // Phase 5: Log and return statistics
    LOG info("Dimension '{}' loaded: {} inserted, {} updated, {} deleted",
             config.dimension_id, stats.inserted, stats.updated, stats.deleted)

    RETURN Ok(stats)
END

STRUCT LoadStats:
    total_source_rows: u64
    inserted: u64
    updated: u64
    deleted: u64
    errors: u64
    duration_ms: u64
```

**Time Complexity**: O(n) for reading + O(n) for loading = O(n)
**Space Complexity**: O(n) for holding all records in memory

### 5.1 Dimension CSV Reading

```pseudocode
FUNCTION read_dimension_csv(source: DimensionSource, schema: DimensionSchema)
    -> Result<Vec<DimensionRecord>>
INPUT:
    source: DimensionSource
    schema: DimensionSchema
OUTPUT: Vec<DimensionRecord> or Error

BEGIN
    // Validate file exists
    IF NOT file_exists(source.path) THEN
        RETURN Err(SourceError::FileNotFound(source.path))
    END IF

    // Initialize reader
    reader <- CsvReader::new(source.path)
        .with_delimiter(source.delimiter)
        .with_encoding(source.encoding)

    // Validate headers
    headers <- reader.read_headers()
    required_columns <- schema.fields
        .filter(|f| f.required)
        .map(|f| f.source_field OR f.name)

    FOR EACH required IN required_columns DO
        IF NOT headers.contains(required) THEN
            RETURN Err(ValidationError::MissingRequiredColumn(required))
        END IF
    END FOR

    // Read and convert rows
    records <- []
    line_number <- 1

    FOR EACH row IN reader DO
        line_number <- line_number + 1
        record <- convert_dimension_row(row, schema, line_number)

        IF record.is_err() THEN
            RETURN Err(record.error)  // Abort on any error for dimensions
        END IF

        records.append(record)
    END FOR

    RETURN Ok(records)
END
```

### 5.2 Row Conversion with Type Coercion

```pseudocode
FUNCTION convert_dimension_row(row: CsvRow, schema: DimensionSchema, line: u64)
    -> Result<DimensionRecord>
INPUT:
    row: CsvRow
    schema: DimensionSchema
    line: u64 - line number for error reporting
OUTPUT: DimensionRecord or Error

BEGIN
    record <- DimensionRecord::new()

    FOR EACH field IN schema.fields DO
        source_column <- field.source_field OR field.name
        raw_value <- row.get(source_column)

        // Handle missing/null values
        IF raw_value IS NULL OR raw_value.trim() IS EMPTY THEN
            IF field.required THEN
                RETURN Err(ValidationError::RequiredFieldMissing(field.name, line))
            ELSE
                record.set(field.name, NULL)
                CONTINUE
            END IF
        END IF

        // Type conversion
        converted <- convert_value(raw_value, field.data_type)

        IF converted.is_err() THEN
            RETURN Err(ValidationError::TypeConversion {
                field: field.name,
                value: raw_value,
                expected_type: field.data_type,
                line: line
            })
        END IF

        record.set(field.name, converted)
    END FOR

    RETURN Ok(record)
END

FUNCTION convert_value(value: String, data_type: DataType) -> Result<Value>
BEGIN
    MATCH data_type:
        DataType::Text =>
            RETURN Ok(Value::Text(value))

        DataType::Integer =>
            parsed <- parse_i64(value.trim())
            IF parsed.is_err() THEN
                RETURN Err(ConversionError::InvalidInteger)
            END IF
            RETURN Ok(Value::Integer(parsed))

        DataType::Float =>
            parsed <- parse_f64(value.trim())
            IF parsed.is_err() THEN
                RETURN Err(ConversionError::InvalidFloat)
            END IF
            RETURN Ok(Value::Float(parsed))

        DataType::Boolean =>
            lower <- value.trim().to_lowercase()
            MATCH lower:
                "true" | "1" | "yes" | "t" => RETURN Ok(Value::Boolean(true))
                "false" | "0" | "no" | "f" => RETURN Ok(Value::Boolean(false))
                _ => RETURN Err(ConversionError::InvalidBoolean)

        DataType::Timestamp =>
            // Try ISO8601 first, then common formats
            parsed <- parse_timestamp(value, TimestampFormat::Iso8601)
            IF parsed.is_err() THEN
                RETURN Err(ConversionError::InvalidTimestamp)
            END IF
            RETURN Ok(Value::Timestamp(parsed))
END
```

**Time Complexity**: O(f) where f = field count per row
**Space Complexity**: O(f) per record

---

## 6. Truncate and Load Strategy

```pseudocode
FUNCTION truncate_and_load(
    db: Database,
    target: DimensionTarget,
    records: Vec<DimensionRecord>
) -> Result<LoadStats>
INPUT:
    db: Database connection
    target: DimensionTarget
    records: Vec<DimensionRecord>
OUTPUT: LoadStats or Error

BEGIN
    start_time <- now()

    // Execute in single transaction for atomicity
    tx <- db.begin_transaction()

    TRY
        // Step 1: Count existing rows (for stats)
        existing_count <- tx.execute_scalar(
            "SELECT COUNT(*) FROM {}",
            target.table
        )

        // Step 2: Delete all existing rows
        tx.execute("DELETE FROM {}", target.table)

        // Step 3: Batch insert new records
        // Use prepared statement for efficiency
        insert_sql <- build_insert_sql(target.table, records[0].columns())
        stmt <- tx.prepare(insert_sql)

        inserted <- 0
        FOR EACH record IN records DO
            stmt.execute(record.values())
            inserted <- inserted + 1
        END FOR

        // Step 4: Commit transaction
        tx.commit()

        RETURN Ok(LoadStats {
            total_source_rows: records.len(),
            inserted: inserted,
            updated: 0,
            deleted: existing_count,
            errors: 0,
            duration_ms: elapsed_ms(start_time)
        })

    CATCH error
        tx.rollback()
        RETURN Err(LoadError::TransactionFailed(error.message))
    END TRY
END

FUNCTION build_insert_sql(table: String, columns: Vec<String>) -> String
BEGIN
    column_list <- columns.join(", ")
    placeholders <- columns.map(|_| "?").join(", ")
    RETURN format!("INSERT INTO {} ({}) VALUES ({})", table, column_list, placeholders)
END
```

**Time Complexity**: O(n) for n records
**Space Complexity**: O(1) (streaming inserts)

---

## 7. Upsert Strategy

```pseudocode
FUNCTION upsert(
    db: Database,
    target: DimensionTarget,
    records: Vec<DimensionRecord>,
    schema: DimensionSchema
) -> Result<LoadStats>
INPUT:
    db: Database connection
    target: DimensionTarget
    records: Vec<DimensionRecord>
    schema: DimensionSchema
OUTPUT: LoadStats or Error

BEGIN
    start_time <- now()

    // Build column lists
    pk_columns <- target.primary_key
    non_pk_columns <- schema.fields
        .map(|f| f.name)
        .filter(|c| NOT pk_columns.contains(c))

    // PostgreSQL/TimescaleDB upsert using ON CONFLICT
    upsert_sql <- build_upsert_sql(target.table, pk_columns, non_pk_columns)

    tx <- db.begin_transaction()

    TRY
        stmt <- tx.prepare(upsert_sql)

        inserted <- 0
        updated <- 0

        FOR EACH record IN records DO
            // Execute upsert and check if insert or update
            result <- stmt.execute_returning(record.values())

            IF result.was_insert THEN
                inserted <- inserted + 1
            ELSE
                updated <- updated + 1
            END IF
        END FOR

        tx.commit()

        RETURN Ok(LoadStats {
            total_source_rows: records.len(),
            inserted: inserted,
            updated: updated,
            deleted: 0,
            errors: 0,
            duration_ms: elapsed_ms(start_time)
        })

    CATCH error
        tx.rollback()
        RETURN Err(LoadError::TransactionFailed(error.message))
    END TRY
END

FUNCTION build_upsert_sql(
    table: String,
    pk_columns: Vec<String>,
    non_pk_columns: Vec<String>
) -> String
BEGIN
    all_columns <- pk_columns + non_pk_columns
    column_list <- all_columns.join(", ")
    placeholders <- all_columns.map(|_| "?").join(", ")

    pk_list <- pk_columns.join(", ")

    // Build UPDATE SET clause for non-pk columns
    update_set <- non_pk_columns
        .map(|c| format!("{} = EXCLUDED.{}", c, c))
        .join(", ")

    RETURN format!(
        "INSERT INTO {} ({}) VALUES ({})
         ON CONFLICT ({}) DO UPDATE SET {}
         RETURNING (xmax = 0) AS was_insert",
        table, column_list, placeholders, pk_list, update_set
    )
END
```

**Time Complexity**: O(n) for n records
**Space Complexity**: O(1) (streaming upserts)

### 7.1 Batch Optimization for Large Datasets

```pseudocode
FUNCTION upsert_batched(
    db: Database,
    target: DimensionTarget,
    records: Vec<DimensionRecord>,
    schema: DimensionSchema,
    batch_size: usize  // Default: 1000
) -> Result<LoadStats>
INPUT:
    db: Database connection
    target: DimensionTarget
    records: Vec<DimensionRecord>
    schema: DimensionSchema
    batch_size: usize
OUTPUT: LoadStats or Error

BEGIN
    stats <- LoadStats::empty()

    // Process in batches to limit memory and transaction size
    FOR batch IN records.chunks(batch_size) DO
        batch_stats <- upsert(db, target, batch, schema)

        IF batch_stats.is_err() THEN
            RETURN Err(batch_stats.error)
        END IF

        stats <- stats.merge(batch_stats)
    END FOR

    RETURN Ok(stats)
END
```

---

## 8. Table Auto-Creation

```pseudocode
FUNCTION ensure_table_exists(
    db: Database,
    table: String,
    schema: DimensionSchema
) -> Result<()>
INPUT:
    db: Database connection
    table: String - fully qualified table name (e.g., "silver.entity_context")
    schema: DimensionSchema
OUTPUT: Success or Error

BEGIN
    // Parse schema and table name
    parts <- table.split(".")
    IF parts.len() != 2 THEN
        RETURN Err(ConfigError::InvalidTableName(table))
    END IF

    schema_name <- parts[0]
    table_name <- parts[1]

    // Check if table exists
    exists <- db.execute_scalar(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = ? AND table_name = ?
        )",
        [schema_name, table_name]
    )

    IF exists THEN
        RETURN Ok(())
    END IF

    // Build CREATE TABLE statement
    create_sql <- build_create_table_sql(table, schema)

    TRY
        db.execute(create_sql)
        LOG info("Created dimension table: {}", table)
        RETURN Ok(())
    CATCH error
        RETURN Err(LoadError::TableCreationFailed(table, error.message))
    END TRY
END

FUNCTION build_create_table_sql(table: String, schema: DimensionSchema) -> String
BEGIN
    columns <- []

    FOR EACH field IN schema.fields DO
        sql_type <- MATCH field.data_type:
            DataType::Text => "TEXT"
            DataType::Integer => "BIGINT"
            DataType::Float => "DOUBLE PRECISION"
            DataType::Boolean => "BOOLEAN"
            DataType::Timestamp => "TIMESTAMPTZ"

        nullable <- IF field.required THEN "NOT NULL" ELSE ""

        columns.append(format!("{} {} {}", field.name, sql_type, nullable))
    END FOR

    column_defs <- columns.join(",\n    ")

    RETURN format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n)",
        table, column_defs
    )
END
```

---

## 9. CLI Command Flow

### 9.1 Dimension Sync Command

```pseudocode
FUNCTION cli_dimension_sync(args: DimensionSyncArgs) -> Result<ExitCode>
INPUT:
    args: DimensionSyncArgs
        - id: Option<String>      // Specific dimension or None for --all
        - all: bool               // Sync all dimensions
        - dry_run: bool           // Validate only
        - config_path: Path       // Config directory
OUTPUT: ExitCode

BEGIN
    // Phase 1: Load configuration
    configs <- IF args.id.is_some() THEN
        [load_dimension_config(args.config_path, args.id)]
    ELSE IF args.all THEN
        load_all_dimension_configs(args.config_path)
    ELSE
        PRINT "Error: specify --id <dimension> or --all"
        RETURN ExitCode::Error
    END IF

    IF configs.is_empty() THEN
        PRINT "No dimension configurations found"
        RETURN ExitCode::Success
    END IF

    // Phase 2: Validate all configs
    validation_errors <- []
    FOR EACH config IN configs DO
        result <- validate_dimension_config(config)
        IF result.is_err() THEN
            validation_errors.append((config.dimension_id, result.error))
        END IF
    END FOR

    IF NOT validation_errors.is_empty() THEN
        FOR EACH (id, error) IN validation_errors DO
            PRINT "Validation error in '{}': {}", id, error
        END FOR
        RETURN ExitCode::ValidationError
    END IF

    // Phase 3: Dry run - just report validation
    IF args.dry_run THEN
        PRINT "Validation successful for {} dimension(s):", configs.len()
        FOR EACH config IN configs DO
            source_rows <- count_csv_rows(config.source.path)
            PRINT "  - {}: {} rows from {}",
                  config.dimension_id, source_rows, config.source.path
        END FOR
        RETURN ExitCode::Success
    END IF

    // Phase 4: Execute load
    db <- connect_to_database()
    total_stats <- LoadStats::empty()
    errors <- []

    FOR EACH config IN configs DO
        PRINT "Loading dimension '{}'...", config.dimension_id

        result <- load_dimension(config, db)

        IF result.is_err() THEN
            errors.append((config.dimension_id, result.error))
            CONTINUE
        END IF

        stats <- result.unwrap()
        total_stats <- total_stats.merge(stats)

        PRINT "  {} inserted, {} updated, {} deleted ({} ms)",
              stats.inserted, stats.updated, stats.deleted, stats.duration_ms
    END FOR

    // Phase 5: Summary
    PRINT ""
    PRINT "Summary: {} dimension(s) processed", configs.len()
    PRINT "  Total inserted: {}", total_stats.inserted
    PRINT "  Total updated: {}", total_stats.updated
    PRINT "  Total deleted: {}", total_stats.deleted
    PRINT "  Total time: {} ms", total_stats.duration_ms

    IF NOT errors.is_empty() THEN
        PRINT ""
        PRINT "Errors ({}):", errors.len()
        FOR EACH (id, error) IN errors DO
            PRINT "  - {}: {}", id, error
        END FOR
        RETURN ExitCode::PartialFailure
    END IF

    RETURN ExitCode::Success
END
```

### 9.2 Dimension List Command

```pseudocode
FUNCTION cli_dimension_list(args: DimensionListArgs) -> Result<ExitCode>
INPUT:
    args: DimensionListArgs
        - config_path: Path
        - format: OutputFormat  // table | json
OUTPUT: ExitCode

BEGIN
    configs <- load_all_dimension_configs(args.config_path)

    IF configs.is_empty() THEN
        PRINT "No dimension configurations found in {}", args.config_path
        RETURN ExitCode::Success
    END IF

    IF args.format == OutputFormat::Json THEN
        PRINT json_serialize(configs)
        RETURN ExitCode::Success
    END IF

    // Table format
    PRINT "Dimension ID          Target Table               Strategy         Source Path"
    PRINT "--------------------  -------------------------  ---------------  --------------------------"

    FOR EACH config IN configs DO
        PRINT "{:<20}  {:<25}  {:<15}  {}",
              config.dimension_id,
              config.target.table,
              config.load.strategy,
              config.source.path
    END FOR

    PRINT ""
    PRINT "{} dimension(s) configured", configs.len()

    RETURN ExitCode::Success
END
```

### 9.3 Stream Ingest Command (CSV Source)

```pseudocode
FUNCTION cli_stream_ingest(args: StreamIngestArgs) -> Result<ExitCode>
INPUT:
    args: StreamIngestArgs
        - stream_id: String
        - config_path: Path
OUTPUT: ExitCode

BEGIN
    // Load stream config
    stream_config <- load_stream_config(args.config_path, args.stream_id)

    IF stream_config.is_err() THEN
        PRINT "Error: stream '{}' not found", args.stream_id
        RETURN ExitCode::NotFound
    END IF

    // Find CSV source in stream config
    csv_source <- stream_config.sources
        .find(|s| s.source_type == SourceType::Csv)

    IF csv_source.is_none() THEN
        PRINT "Error: stream '{}' has no CSV source", args.stream_id
        RETURN ExitCode::ConfigError
    END IF

    // Build CSV source adapter config
    adapter_config <- CsvSourceConfig::from_stream_config(stream_config, csv_source)

    // Execute ingest
    PRINT "Ingesting CSV data for stream '{}'...", args.stream_id

    raw_points <- fetch_raw_batch(adapter_config)

    IF raw_points.is_err() THEN
        PRINT "Error during ingest: {}", raw_points.error
        RETURN ExitCode::Error
    END IF

    points <- raw_points.unwrap()

    IF points.is_empty() THEN
        PRINT "Warning: no data ingested (empty CSV)"
        RETURN ExitCode::Success
    END IF

    // Write to Bronze layer
    bronze_store <- get_bronze_store()
    bronze_store.write_raw_batch(points)

    PRINT "Successfully ingested {} records to Bronze layer", points.len()

    RETURN ExitCode::Success
END
```

---

## 10. Configuration Loading

```pseudocode
FUNCTION load_dimension_config(config_path: Path, dimension_id: String)
    -> Result<DimensionConfig>
INPUT:
    config_path: Path - base config directory
    dimension_id: String
OUTPUT: DimensionConfig or Error

BEGIN
    // Construct file path
    file_path <- config_path / "dimensions" / format!("{}.yaml", dimension_id)

    IF NOT file_exists(file_path) THEN
        RETURN Err(ConfigError::NotFound(dimension_id))
    END IF

    // Parse YAML
    content <- read_file(file_path)
    config <- yaml_parse<DimensionConfig>(content)

    IF config.is_err() THEN
        RETURN Err(ConfigError::ParseError(file_path, config.error))
    END IF

    // Resolve relative paths
    config <- config.unwrap()
    IF NOT config.source.path.is_absolute() THEN
        config.source.path <- config_path / config.source.path
    END IF

    RETURN Ok(config)
END

FUNCTION load_all_dimension_configs(config_path: Path) -> Vec<DimensionConfig>
BEGIN
    dimension_dir <- config_path / "dimensions"

    IF NOT directory_exists(dimension_dir) THEN
        RETURN []
    END IF

    configs <- []

    FOR EACH file IN list_files(dimension_dir, "*.yaml") DO
        dimension_id <- file.stem()  // filename without extension

        result <- load_dimension_config(config_path, dimension_id)

        IF result.is_ok() THEN
            configs.append(result.unwrap())
        ELSE
            LOG warning("Failed to load dimension config '{}': {}",
                       dimension_id, result.error)
        END IF
    END FOR

    RETURN configs
END
```

---

## 11. Error Handling Flow

### 11.1 Error Types

```pseudocode
ENUM CsvError:
    FileNotFound { path: Path }
    EmptyFile { path: Path }
    EncodingError { path: Path, encoding: String }
    HeaderMissing { column: String }
    RequiredColumnMissing { column: String, line: u64 }
    TimestampParseError { value: String, format: String, line: u64 }
    TypeConversionError { field: String, value: String, expected: DataType, line: u64 }
    IoError { message: String }

ENUM DimensionError:
    ConfigNotFound { dimension_id: String }
    ConfigParseError { path: Path, message: String }
    ValidationError { dimension_id: String, message: String }
    SourceError { dimension_id: String, cause: CsvError }
    LoadError { dimension_id: String, cause: DatabaseError }
    TableCreationFailed { table: String, cause: DatabaseError }
```

### 11.2 Error Formatting for CLI

```pseudocode
FUNCTION format_csv_error(error: CsvError) -> String
BEGIN
    MATCH error:
        FileNotFound { path } =>
            RETURN format!("File not found: {}", path)

        EmptyFile { path } =>
            RETURN format!("Empty file (no data rows): {}", path)

        HeaderMissing { column } =>
            RETURN format!("Required column '{}' not found in CSV header", column)

        RequiredColumnMissing { column, line } =>
            RETURN format!("Required value missing for '{}' at line {}", column, line)

        TimestampParseError { value, format, line } =>
            RETURN format!(
                "Invalid timestamp '{}' at line {} (expected format: {})",
                value, line, format
            )

        TypeConversionError { field, value, expected, line } =>
            RETURN format!(
                "Type error at line {}: '{}' is not a valid {} for field '{}'",
                line, value, expected, field
            )

        IoError { message } =>
            RETURN format!("I/O error: {}", message)
END
```

---

## 12. Complexity Analysis Summary

| Algorithm | Time Complexity | Space Complexity | Notes |
|-----------|-----------------|------------------|-------|
| `fetch_raw_batch` | O(n * m) | O(n) | n=rows, m=columns; holds all points in memory |
| `process_csv_row` | O(m) | O(m) | m=columns per row |
| `parse_timestamp` | O(k) | O(1) | k=timestamp string length |
| `validate_headers` | O(h + f) | O(h) | h=headers, f=schema fields |
| `read_dimension_csv` | O(n * f) | O(n) | n=rows, f=fields per row |
| `truncate_and_load` | O(n) | O(1) | Streaming inserts |
| `upsert` | O(n) | O(1) | Streaming upserts |
| `upsert_batched` | O(n) | O(b) | b=batch_size |
| `ensure_table_exists` | O(f) | O(f) | f=field count |
| `cli_dimension_sync` | O(d * n) | O(n) | d=dimensions, n=max rows |

---

## 13. Design Patterns Used

### 13.1 Adapter Pattern
- `CsvSourceAdapter` adapts CSV files to the `RawSource` trait interface
- Allows CSV to be treated identically to HTTP/MQTT sources in the ingestion pipeline

### 13.2 Strategy Pattern
- `LoadStrategy` enum encapsulates loading algorithms (truncate_and_load vs upsert)
- New strategies can be added without modifying DimensionLoader

### 13.3 Builder Pattern
- `RawDataPoint::new().with_timestamp().with_ndp_id()` for fluent construction
- `CsvReader::new().with_delimiter().with_encoding()` for reader configuration

### 13.4 Result Type (Railway-Oriented Programming)
- All functions return `Result<T, Error>` for explicit error handling
- Early returns with `?` operator in implementation
- Error accumulation where batch processing is needed

---

## 14. Implementation Roadmap

### Phase 1: Core Types (Est. 2 hours)
1. Define `CsvError` and `DimensionError` types in `core/src/error.rs`
2. Add `SourceType::Csv` variant to stream config
3. Define `DimensionConfig` and related structs

### Phase 2: CSV Source Adapter (Est. 4 hours)
1. Implement `CsvSourceConfig` parsing from stream config
2. Implement `fetch_raw_batch` for CSV files
3. Add timestamp parsing with multiple format support
4. Write unit tests for row processing and error handling

### Phase 3: Dimension Loader (Est. 4 hours)
1. Implement `load_dimension` with validation
2. Implement `truncate_and_load` strategy
3. Implement `upsert` strategy with PostgreSQL ON CONFLICT
4. Add table auto-creation logic

### Phase 4: CLI Commands (Est. 3 hours)
1. Add `ndp dimension list` command
2. Add `ndp dimension sync` command with --dry-run
3. Add `ndp stream ingest` command for CSV sources
4. Integrate dimension sync into `deploy.sh sync`

### Phase 5: Integration Tests (Est. 3 hours)
1. Test CSV stream source to Bronze flow
2. Test dimension truncate_and_load
3. Test dimension upsert
4. Test deploy.sh integration
5. Test error handling scenarios

---

## Appendix: Example Configurations

### A.1 CSV Stream Source

```yaml
# config/base/streams/historical-aq.yaml
stream_id: historical-aq
enabled: true
source:
  type: csv
  path: data/imports/historical_readings.csv
  timestamp_field: timestamp
  timestamp_format: iso8601
  on_error: skip
entity_schemas:
  - entity_type: air_quality
    fields:
      - name: pm25
        source_field: pm25
        data_type: float
      - name: temperature
        source_field: temp_c
        data_type: float
```

### A.2 Dimension Config

```yaml
# config/base/dimensions/entity_context.yaml
dimension_id: entity-context
target:
  table: silver.entity_context
  primary_key: [ndp_id]
source:
  type: csv
  path: config/dimensions/entity_context.csv
schema:
  fields:
    - name: ndp_id
      data_type: text
      required: true
    - name: category
      data_type: text
      required: true
    - name: friendly_name
      data_type: text
    - name: location_path
      data_type: text
load:
  strategy: truncate_and_load
```
