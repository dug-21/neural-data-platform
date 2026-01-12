# DP-007: Pre-Transform Parser Integration - Pseudocode

**Feature**: DP-007 (Pre-Transform Parser Integration)
**Phase**: Pseudocode (SPARC P)
**Date**: 2026-01-12
**Author**: ndp-rust-dev

---

## 1. Overview

This document defines the pseudocode for integrating the existing `ColumnOrientedParser` into
the Silver ETL pipeline as a pre-transform step. The pre-transform flattens columnar array
data (like NWS gridpoints forecasts) before DuckDB SQL processing.

---

## 2. Data Structures

### 2.1 PreTransformConfig

Configuration for the pre-transform step, added to `SilverEtlConfig`:

```
struct PreTransformConfig {
    // Whether pre-transform is enabled for this stream
    enabled: bool

    // Parser type to use ("column_oriented" for NWS gridpoints)
    parser_type: String

    // Base path to metrics container in raw_payload
    // Example: "properties" for NWS gridpoints
    metrics_base_path: String

    // Column mappings: metric_path -> field_name
    columns: Vec<ColumnMapping>

    // Timestamp format for parsing validTime values
    timestamp_format: TimestampFormat

    // Optional unit conversions by field name
    unit_conversions: HashMap<String, UnitConversion>

    // Default tags to attach to all extracted points
    default_tags: HashMap<String, String>
}
```

### 2.2 ColumnMapping (Existing from core/src/parsers/config.rs)

```
struct ColumnMapping {
    // Path within metrics base (e.g., "temperature" for NWS)
    metric_path: String

    // Output field name in flattened row
    field_name: String

    // Path to values array within metric (default: "values")
    values_path: Option<String>

    // Path to timestamp within value entry (default: "validTime")
    timestamp_path: Option<String>

    // Path to value within entry (default: "value")
    value_path: Option<String>
}
```

### 2.3 TimestampFormat (Existing from core/src/parsers/config.rs)

```
enum TimestampFormat {
    // NWS format: "2025-12-23T00:00:00+00:00/PT1H"
    Iso8601Duration

    // Parallel array format (Open-Meteo style)
    ParallelArray { time_path: String }
}
```

### 2.4 FlattenedRow (Output of Pre-Transform)

```
struct FlattenedRow {
    // When the forecast was issued (Bronze timestamp in MICROSECONDS)
    // Stored as i64 microseconds - silver-etl uses microseconds_to_timestamp transform
    issue_time: i64

    // When the forecast is valid (from metric array validTime)
    // Stored as i64 UNIX SECONDS - silver-etl uses unix_seconds transform
    // IMPORTANT: This is SECONDS not microseconds (matches ColumnOrientedParser output)
    valid_time: i64

    // NDP identifier for deduplication
    ndp_id: String

    // Metric name (e.g., "temperature", "wind_speed")
    metric_name: String

    // Metric value (numeric)
    value: f64

    // Additional tags (source, grid_office, etc.)
    tags: HashMap<String, String>
}
```

**CRITICAL: Timestamp Format Consistency**

The Silver layer uses TIMESTAMPTZ for all timestamp columns. The existing silver-etl
supports multiple timestamp transforms via `TransformConfig::Timestamp { format }`:

| Field | Pre-Transform Output | Transform | Silver Type |
|-------|---------------------|-----------|-------------|
| `issue_time` | i64 microseconds | `microseconds_to_timestamp` | TIMESTAMPTZ |
| `valid_time` | i64 unix seconds | `unix_seconds` | TIMESTAMPTZ |

The `valid_time` uses unix seconds because that's what `ColumnOrientedParser` produces
in the `forecast_valid_time` tag (see `column_oriented.rs:285`).

---

## 3. Pre-Transform Algorithm

### 3.1 Main Pre-Transform Function

```
function pre_transform(bronze_rows: Vec<BronzeRow>, config: PreTransformConfig) -> Result<Vec<FlattenedRow>, Error>:
    """
    Transform Bronze rows with columnar array data into flattened rows.

    Each Bronze row contains raw_payload with structure like:
    {
        "properties": {
            "temperature": { "values": [{ "validTime": "...", "value": 15.5 }, ...] },
            "windSpeed": { "values": [{ "validTime": "...", "value": 10.0 }, ...] }
        }
    }

    Output: One FlattenedRow per metric per validTime.
    """

    // Early return if pre-transform disabled
    if not config.enabled:
        return Error("Pre-transform not enabled")

    // Create parser from config
    parser = create_column_oriented_parser(config)

    // Pre-allocate output vector (estimate: rows * columns * ~150 values)
    estimated_capacity = bronze_rows.len() * config.columns.len() * 150
    flattened_rows = Vec::with_capacity(estimated_capacity)

    // Process each Bronze row
    for bronze_row in bronze_rows:
        result = process_bronze_row(bronze_row, parser, config)

        match result:
            Ok(rows):
                flattened_rows.extend(rows)
            Err(error):
                // Log warning but continue processing
                warn!("Failed to process Bronze row {}: {}", bronze_row.ndp_id, error)
                // Track for DQ transparency
                record_parse_failure(bronze_row.ndp_id, error)

    info!("Pre-transform produced {} flattened rows from {} Bronze rows",
          flattened_rows.len(), bronze_rows.len())

    return Ok(flattened_rows)
```

### 3.2 Process Single Bronze Row

```
function process_bronze_row(bronze_row: BronzeRow, parser: ColumnOrientedParser, config: PreTransformConfig) -> Result<Vec<FlattenedRow>, Error>:
    """
    Extract flattened rows from a single Bronze row using ColumnOrientedParser.

    TIMESTAMP HANDLING:
    - issue_time: Stored as i64 MICROSECONDS (from Bronze timestamp)
    - valid_time: Stored as i64 UNIX SECONDS (from parser's forecast_valid_time tag)

    The silver-etl field_mappings will apply the appropriate transforms:
    - issue_time: microseconds_to_timestamp -> TIMESTAMPTZ
    - valid_time: unix_seconds -> TIMESTAMPTZ
    """

    // Keep issue_time as microseconds (i64) - silver-etl transforms it
    issue_time_us: i64 = bronze_row.timestamp

    // Parse raw_payload JSON
    raw_payload = parse_json(bronze_row.raw_payload)?

    // Use existing parser to extract TimeSeriesPoints
    // Parser handles: path navigation, timestamp parsing, unit conversion
    // Note: We pass a DateTime for parser compatibility, but store microseconds
    points = parser.parse(raw_payload, microseconds_to_datetime(issue_time_us))?

    // Convert TimeSeriesPoints to FlattenedRows
    flattened_rows = Vec::with_capacity(points.len())

    for point in points:
        // Extract valid_time from tags as UNIX SECONDS (i64)
        // ColumnOrientedParser stores this as seconds string (column_oriented.rs:285)
        valid_time_secs: i64 = point.tags.get("forecast_valid_time")?.parse::<i64>()?

        // Extract metric name from tags
        metric_name = point.tags.get("metric")?

        // Build flattened row with timestamps as integers
        // Silver-etl applies transforms via field_mappings
        flattened_row = FlattenedRow {
            issue_time: issue_time_us,      // i64 microseconds
            valid_time: valid_time_secs,    // i64 unix seconds
            ndp_id: bronze_row.ndp_id.clone(),
            metric_name: metric_name.clone(),
            value: point.value,
            tags: point.tags.clone()
        }

        flattened_rows.push(flattened_row)

    return Ok(flattened_rows)
```

### 3.3 Create Parser from Config

```
function create_column_oriented_parser(config: PreTransformConfig) -> Result<ColumnOrientedParser, Error>:
    """
    Create ColumnOrientedParser instance from PreTransformConfig.
    Reuses existing ParserConfig and ColumnOrientedConfig types.
    """

    // Build ColumnOrientedConfig
    column_config = ColumnOrientedConfig {
        metrics_base_path: config.metrics_base_path,
        columns: config.columns,
        timestamp_format: config.timestamp_format,
        unit_conversions: config.unit_conversions
    }

    // Build ParserConfig wrapper
    parser_config = ParserConfig {
        parser_type: ParserType::ColumnOriented,
        location_id_field: "properties.gridId",
        default_location_id: Some("unknown"),
        skip_fields: vec![],
        field_mappings: None,
        default_tags: config.default_tags,
        array_config: None,
        column_config: Some(column_config)
    }

    // Create parser using existing factory method
    return ColumnOrientedParser::from_config(parser_config)
```

---

## 4. ETL Pipeline Integration

### 4.1 Modified ETL Run Function

```
function run_etl(stream_id: String, config: SilverEtlConfig) -> Result<EtlResult, Error>:
    """
    Main ETL pipeline with optional pre-transform step.
    """

    info!("Starting ETL for stream: {}", stream_id)

    // Step 1: Load Bronze Parquet data
    bronze_path = format!("/data/raw/{}/**/*.parquet", stream_id)
    bronze_data = load_bronze_parquet(bronze_path, config.incremental)?

    info!("Loaded {} Bronze rows", bronze_data.len())

    // Step 2: Pre-transform if configured (NEW)
    working_data = if config.pre_transform.enabled:
        info!("Applying pre-transform with parser: {}", config.pre_transform.parser_type)

        flattened = pre_transform(bronze_data, config.pre_transform)?

        info!("Pre-transform expanded {} Bronze rows to {} flattened rows",
              bronze_data.len(), flattened.len())

        // Convert to DuckDB-compatible format
        flattened_to_duckdb_rows(flattened)
    else:
        // Pass through unchanged for non-array streams
        bronze_to_duckdb_rows(bronze_data)

    // Step 3: Load into DuckDB temporary table
    duckdb = DuckDB::new()?
    duckdb.register_postgres(config.postgres_connection)?

    if config.pre_transform.enabled:
        // Flattened schema: issue_time, valid_time, ndp_id, metric_name, value, tags
        duckdb.create_table("bronze_data", working_data, FLATTENED_SCHEMA)?
    else:
        // Standard schema: timestamp, ndp_id, raw_payload, context
        duckdb.create_table("bronze_data", working_data, BRONZE_SCHEMA)?

    // Step 4: Generate and execute ETL SQL
    etl_sql = generate_etl_sql(config, config.pre_transform.enabled)

    result = duckdb.execute(etl_sql)?

    info!("ETL completed: {} rows inserted/updated", result.rows_affected)

    // Step 5: Update watermark
    update_watermark(stream_id, result.max_timestamp)?

    return Ok(EtlResult {
        rows_processed: bronze_data.len(),
        rows_produced: result.rows_affected,
        pre_transform_applied: config.pre_transform.enabled
    })
```

### 4.2 Generate ETL SQL (Modified for Pre-Transform)

```
function generate_etl_sql(config: SilverEtlConfig, pre_transform_enabled: bool) -> String:
    """
    Generate DuckDB SQL for ETL.
    Different SQL structure based on whether pre-transform was applied.
    """

    if pre_transform_enabled:
        return generate_pivoted_sql(config)
    else:
        return generate_standard_sql(config)


function generate_pivoted_sql(config: SilverEtlConfig) -> String:
    """
    Generate SQL that pivots flattened metric_name/value columns into
    individual metric columns (temperature_c, wind_speed_kmh, etc.)
    """

    sql = """
    INSERT INTO pg.{target_table} (
        ingestion_time,
        issue_time,
        valid_time,
        ndp_id,
        {metric_columns},
        dq_flags
    )
    WITH pivoted AS (
        SELECT
            issue_time,
            valid_time,
            ndp_id,
            {pivot_expressions}
        FROM bronze_data
        GROUP BY issue_time, valid_time, ndp_id
    ),
    with_dq AS (
        SELECT
            current_timestamp AS ingestion_time,
            issue_time,
            valid_time,
            ndp_id,
            {metric_columns_with_dq},
            {dq_flags_expression} AS dq_flags
        FROM pivoted
    )
    SELECT * FROM with_dq
    ON CONFLICT ({dedup_keys}) DO UPDATE SET
        ingestion_time = EXCLUDED.ingestion_time,
        {upsert_columns}
    """.format(
        target_table = config.target_table,
        metric_columns = generate_metric_column_list(config),
        pivot_expressions = generate_pivot_expressions(config),
        metric_columns_with_dq = generate_columns_with_dq(config),
        dq_flags_expression = generate_dq_flags(config),
        dedup_keys = config.deduplication.key_columns.join(", "),
        upsert_columns = generate_upsert_set_clause(config)
    )

    return sql
```

---

## 5. Pivot Logic for Multiple Metrics

### 5.1 Generate Pivot Expressions

```
function generate_pivot_expressions(config: SilverEtlConfig) -> String:
    """
    Generate SQL CASE expressions to pivot metric_name/value into individual columns.

    Input: rows with (metric_name, value) pairs
    Output: columns like temperature_c, wind_speed_kmh, humidity_pct

    Example output:
        MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
        MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh,
        ...
    """

    expressions = []

    for mapping in config.field_mappings:
        // Extract the metric name that ColumnOrientedParser produces
        // e.g., "temperature" from column_config.columns[].field_name
        metric_name = extract_metric_name_from_source_path(mapping.source_path)

        expression = """
            MAX(CASE WHEN metric_name = '{metric_name}' THEN value END) AS {target_column}
        """.format(
            metric_name = metric_name,
            target_column = mapping.target_column
        )

        expressions.push(expression)

    return expressions.join(",\n")


function extract_metric_name_from_source_path(source_path: String) -> String:
    """
    Extract metric name from source_path for mapping to pre-transform output.

    source_path: "raw_payload.properties.temperature"
    returns: "temperature"

    This matches what ColumnOrientedParser puts in tags["metric"].
    """

    // source_path format: raw_payload.properties.{metric_path}
    // We need the last segment which matches column_config.columns[].metric_path
    parts = source_path.split(".")
    return parts.last()
```

### 5.2 Complete Pivot SQL Example

```
-- Example: Pivoting NWS forecast flattened rows
-- Input: bronze_data with columns (issue_time, valid_time, ndp_id, metric_name, value, tags)
-- Output: Silver table with individual metric columns

INSERT INTO pg.silver.nws_forecasts (
    ingestion_time,
    issue_time,
    valid_time,
    ndp_id,
    temperature_c,
    dewpoint_c,
    wind_speed_kmh,
    wind_direction_deg,
    humidity_pct,
    sky_cover_pct,
    precip_probability_pct,
    dq_flags
)
WITH pivoted AS (
    SELECT
        issue_time,
        valid_time,
        ndp_id,
        -- Pivot each metric into its own column
        MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
        MAX(CASE WHEN metric_name = 'dewpoint' THEN value END) AS dewpoint_c,
        MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh,
        MAX(CASE WHEN metric_name = 'wind_direction' THEN value END) AS wind_direction_deg,
        MAX(CASE WHEN metric_name = 'relative_humidity' THEN value END) AS humidity_pct,
        MAX(CASE WHEN metric_name = 'sky_cover' THEN value END) AS sky_cover_pct,
        MAX(CASE WHEN metric_name = 'probability_of_precipitation' THEN value END) AS precip_probability_pct
    FROM bronze_data
    GROUP BY issue_time, valid_time, ndp_id
),
with_dq AS (
    SELECT
        current_timestamp AS ingestion_time,
        issue_time,
        valid_time,
        ndp_id,
        -- Apply DQ checks during pivot
        CASE
            WHEN temperature_c NOT BETWEEN -50.0 AND 60.0 THEN NULL
            ELSE temperature_c
        END AS temperature_c,
        CASE
            WHEN dewpoint_c NOT BETWEEN -50.0 AND 60.0 THEN NULL
            ELSE dewpoint_c
        END AS dewpoint_c,
        wind_speed_kmh,
        -- Clamp wind direction to 0-360
        LEAST(GREATEST(wind_direction_deg, 0.0), 360.0) AS wind_direction_deg,
        -- Clamp humidity to 0-100
        LEAST(GREATEST(humidity_pct, 0.0), 100.0) AS humidity_pct,
        LEAST(GREATEST(sky_cover_pct, 0.0), 100.0) AS sky_cover_pct,
        LEAST(GREATEST(precip_probability_pct, 0.0), 100.0) AS precip_probability_pct,
        -- DQ flags
        ARRAY_REMOVE(ARRAY[
            CASE WHEN temperature_c NOT BETWEEN -50.0 AND 60.0 THEN 'range:temperature_c' END,
            CASE WHEN dewpoint_c NOT BETWEEN -50.0 AND 60.0 THEN 'range:dewpoint_c' END
        ], NULL) AS dq_flags
    FROM pivoted
)
SELECT * FROM with_dq
ON CONFLICT (issue_time, valid_time, ndp_id) DO UPDATE SET
    ingestion_time = EXCLUDED.ingestion_time,
    temperature_c = EXCLUDED.temperature_c,
    dewpoint_c = EXCLUDED.dewpoint_c,
    wind_speed_kmh = EXCLUDED.wind_speed_kmh,
    wind_direction_deg = EXCLUDED.wind_direction_deg,
    humidity_pct = EXCLUDED.humidity_pct,
    sky_cover_pct = EXCLUDED.sky_cover_pct,
    precip_probability_pct = EXCLUDED.precip_probability_pct,
    dq_flags = EXCLUDED.dq_flags;
```

---

## 6. Error Handling

### 6.1 Row-Level Error Handling

```
enum PreTransformError {
    // JSON parsing failed
    JsonParseError { ndp_id: String, error: String }

    // Required path not found in payload
    PathNotFound { ndp_id: String, path: String }

    // Timestamp parsing failed
    TimestampParseError { ndp_id: String, timestamp: String, error: String }

    // Value extraction failed (not numeric)
    ValueExtractionError { ndp_id: String, metric: String, error: String }

    // Parser configuration error
    ConfigError { message: String }
}


function handle_row_error(error: PreTransformError, dq_tracker: &mut DqTracker) -> ErrorAction:
    """
    Determine action to take when row processing fails.
    Graceful degradation: skip bad rows, continue processing.
    """

    match error:
        JsonParseError { ndp_id, error }:
            warn!("JSON parse failed for {}: {}", ndp_id, error)
            dq_tracker.record_error(ndp_id, "json_parse_error", error)
            return ErrorAction::SkipRow

        PathNotFound { ndp_id, path }:
            // This is common for optional metrics - warn but not error
            debug!("Path {} not found in row {}", path, ndp_id)
            return ErrorAction::SkipMetric

        TimestampParseError { ndp_id, timestamp, error }:
            warn!("Timestamp parse failed for {} ({}): {}", ndp_id, timestamp, error)
            dq_tracker.record_error(ndp_id, "timestamp_parse_error", error)
            return ErrorAction::SkipValue

        ValueExtractionError { ndp_id, metric, error }:
            debug!("Value extraction failed for {}.{}: {}", ndp_id, metric, error)
            return ErrorAction::SkipValue

        ConfigError { message }:
            // Configuration errors are fatal
            error!("Configuration error: {}", message)
            return ErrorAction::Abort


enum ErrorAction {
    SkipRow      // Skip entire Bronze row
    SkipMetric   // Skip this metric column, continue others
    SkipValue    // Skip this value entry, continue processing
    Abort        // Stop ETL execution
}
```

### 6.2 Graceful Degradation Strategy

```
function pre_transform_with_graceful_degradation(
    bronze_rows: Vec<BronzeRow>,
    config: PreTransformConfig,
    dq_tracker: &mut DqTracker
) -> Result<Vec<FlattenedRow>, Error>:
    """
    Pre-transform with graceful degradation.

    Strategy:
    1. Row-level failures skip the row
    2. Metric-level failures skip the metric
    3. Value-level failures skip the value
    4. Continue processing remaining data
    5. Track all errors in DQ transparency table
    """

    flattened_rows = vec![]

    stats = PreTransformStats::new()

    for bronze_row in bronze_rows:
        stats.rows_attempted += 1

        match process_bronze_row_safe(bronze_row, config, dq_tracker):
            Ok(rows):
                stats.rows_succeeded += 1
                stats.values_produced += rows.len()
                flattened_rows.extend(rows)

            Err(error) if error.is_recoverable():
                stats.rows_failed += 1
                // Already logged and tracked in dq_tracker
                continue

            Err(error):
                // Non-recoverable error (config issue)
                return Err(error)

    info!("Pre-transform stats: {} rows attempted, {} succeeded, {} failed, {} values produced",
          stats.rows_attempted, stats.rows_succeeded, stats.rows_failed, stats.values_produced)

    // Emit warning if failure rate is high
    failure_rate = stats.rows_failed as f64 / stats.rows_attempted as f64
    if failure_rate > 0.1:  // >10% failure rate
        warn!("High pre-transform failure rate: {:.1}%", failure_rate * 100.0)

    return Ok(flattened_rows)


function process_bronze_row_safe(
    bronze_row: BronzeRow,
    config: PreTransformConfig,
    dq_tracker: &mut DqTracker
) -> Result<Vec<FlattenedRow>, PreTransformError>:
    """
    Process a single Bronze row with error wrapping.
    """

    // Parse JSON with error context
    raw_payload = parse_json(bronze_row.raw_payload)
        .map_err(|e| PreTransformError::JsonParseError {
            ndp_id: bronze_row.ndp_id.clone(),
            error: e.to_string()
        })?

    // Navigate to metrics base
    metrics_base = extract_path(raw_payload, config.metrics_base_path)
        .ok_or(PreTransformError::PathNotFound {
            ndp_id: bronze_row.ndp_id.clone(),
            path: config.metrics_base_path.clone()
        })?

    issue_time = microseconds_to_datetime(bronze_row.timestamp)

    flattened = vec![]

    // Process each metric column
    for column in config.columns:
        match process_metric_column_safe(
            metrics_base,
            column,
            issue_time,
            bronze_row.ndp_id,
            config,
            dq_tracker
        ):
            Ok(values):
                flattened.extend(values)
            Err(error):
                // Log and skip this metric, continue with others
                let action = handle_row_error(error, dq_tracker)
                if action == ErrorAction::SkipMetric:
                    continue

    return Ok(flattened)
```

### 6.3 DQ Transparency Tracking

```
struct DqTracker {
    // Parse errors by ndp_id
    parse_errors: HashMap<String, Vec<ParseError>>

    // Missing metrics by ndp_id
    missing_metrics: HashMap<String, Vec<String>>

    // Value-level errors
    value_errors: Vec<ValueError>
}


function record_to_transparency_table(dq_tracker: DqTracker, stream_id: String):
    """
    Write DQ issues to transparency table for audit trail.
    """

    for (ndp_id, errors) in dq_tracker.parse_errors:
        for error in errors:
            insert_dq_event(
                stream_id = stream_id,
                ndp_id = ndp_id,
                event_type = "parse_error",
                event_time = current_timestamp(),
                details = error.to_json()
            )

    // ... similar for missing_metrics and value_errors
```

---

## 7. Configuration Examples

### 7.1 YAML Configuration for NWS Gridpoints

```yaml
# config/base/streams/nws-gridpoints-forecast/config.yaml
# Addition to silver_etl section

silver_etl:
  enabled: true
  target_table: silver.nws_forecasts

  # NEW: Pre-transform configuration
  pre_transform:
    enabled: true
    parser_type: column_oriented
    metrics_base_path: properties
    timestamp_format:
      type: iso8601_duration
    columns:
      - metric_path: temperature
        field_name: temperature
      - metric_path: dewpoint
        field_name: dewpoint
      - metric_path: windSpeed
        field_name: wind_speed
      - metric_path: windDirection
        field_name: wind_direction
      - metric_path: windGust
        field_name: wind_gust
      - metric_path: relativeHumidity
        field_name: relative_humidity
      - metric_path: skyCover
        field_name: sky_cover
      - metric_path: probabilityOfPrecipitation
        field_name: probability_of_precipitation
    default_tags:
      source: nws
      api: gridpoints

  # Primary timestamp mapping (issue_time from Bronze)
  # Uses microseconds_to_timestamp - consistent with all other streams
  timestamp:
    source_field: issue_time          # From pre-transform output (microseconds)
    target_field: issue_time
    transform: microseconds_to_timestamp

  # Field mappings - includes valid_time with timestamp transform
  field_mappings:
    # CRITICAL: valid_time uses unix_seconds transform (not microseconds!)
    # This ensures consistency with Silver layer TIMESTAMPTZ format
    - source_path: valid_time           # From pre-transform output (unix seconds)
      target_column: valid_time
      type: timestamptz
      nullable: false
      transform:
        type: timestamp
        format: unix_seconds            # Generates: to_timestamp(valid_time) AS valid_time

    # Metric columns (pivoted from metric_name/value)
    - source_path: temperature          # Metric name from pre-transform
      target_column: temperature_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: -50.0
          max: 60.0
          action: flag

    - source_path: dewpoint
      target_column: dewpoint_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: -50.0
          max: 60.0
          action: flag

    - source_path: wind_speed
      target_column: wind_speed_kmh
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 300.0
          action: flag

    # ... additional metric mappings

  # Cross-field DQ rules
  dq_rules:
    # Forecast validity check: valid_time must be after issue_time
    - rule: cross_field_check
      name: valid_after_issue
      expression: "valid_time >= issue_time"
      message: "valid_time_before_issue_time"
      action: flag

    # Forecast horizon check (max 7 days)
    - rule: cross_field_check
      name: forecast_horizon
      expression: "EXTRACT(EPOCH FROM (valid_time - issue_time)) <= 604800"
      message: "forecast_exceeds_7_days"
      action: flag

  # Deduplication on (issue_time, valid_time, ndp_id)
  deduplication:
    enabled: true
    key_columns: [issue_time, valid_time, ndp_id]
    strategy: upsert
```

**Timestamp Transform Summary:**

| Column | Source | Format | Transform | DuckDB SQL Generated |
|--------|--------|--------|-----------|---------------------|
| `issue_time` | Bronze timestamp | microseconds | `microseconds_to_timestamp` | `to_timestamp(issue_time / 1000000)` |
| `valid_time` | Pre-transform output | unix seconds | `unix_seconds` | `to_timestamp(valid_time)` |

Both result in TIMESTAMPTZ columns, consistent with all other Silver layer streams.

### 7.2 SilverEtlConfig Rust Type Extension

```
// Extension to existing SilverEtlConfig
struct SilverEtlConfig {
    // ... existing fields ...

    // NEW: Optional pre-transform configuration
    #[serde(default)]
    pre_transform: Option<PreTransformConfig>
}

// Default implementation
impl Default for PreTransformConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            parser_type: "column_oriented".to_string(),
            metrics_base_path: String::new(),
            columns: vec![],
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions: HashMap::new(),
            default_tags: HashMap::new()
        }
    }
}
```

---

## 8. Interface with Existing Components

### 8.1 ColumnOrientedParser Interface (Existing)

The pre-transform uses the existing `ColumnOrientedParser` trait implementation:

```
// Existing interface from core/src/parsers/traits.rs
trait Parser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &str;
    fn config(&self) -> &ParserConfig;
}

// TimeSeriesPoint structure (existing)
struct TimeSeriesPoint {
    timestamp: DateTime<Utc>     // Ingestion timestamp
    location_id: String          // Grid ID or station
    value: f64                   // Metric value
    tags: HashMap<String, String> // Includes "metric" and "forecast_valid_time"
    ndp_id: Option<String>       // NDP identifier
    context: Option<Value>       // Additional context
}
```

### 8.2 Data Flow Diagram

```
Bronze Parquet Row
    |
    | { timestamp: i64 (µs), ndp_id: String, raw_payload: JSON, context: JSON }
    |
    v
+-------------------+
| Pre-Transform     |
| (Rust)            |
+-------------------+
    |
    | ColumnOrientedParser.parse()
    | - Parses ISO8601 duration timestamps
    | - Extracts forecast_valid_time as Unix SECONDS (i64)
    |
    v
Vec<TimeSeriesPoint>
    |
    | Convert to FlattenedRow
    | - issue_time: i64 MICROSECONDS (from Bronze timestamp)
    | - valid_time: i64 UNIX SECONDS (from forecast_valid_time tag)
    |
    v
Vec<FlattenedRow>
    |
    | { issue_time: i64 µs, valid_time: i64 sec, ndp_id, metric_name, value }
    |
    v
+-------------------+
| DuckDB            |
| (SQL Pivot +      |
|  Timestamp Xform) |
+-------------------+
    |
    | -- Timestamp transforms (existing silver-etl code):
    | to_timestamp(issue_time / 1000000) AS issue_time  -- microseconds_to_timestamp
    | to_timestamp(valid_time) AS valid_time            -- unix_seconds
    |
    | -- Pivot metrics:
    | MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c
    |
    v
Silver Row (all TIMESTAMPTZ)
    |
    | { issue_time: TIMESTAMPTZ, valid_time: TIMESTAMPTZ, ndp_id, temperature_c, ... }
    |
    v
+-------------------+
| TimescaleDB       |
| (Silver Layer)    |
+-------------------+

TIMESTAMP CONSISTENCY:
======================
All Silver tables use TIMESTAMPTZ. The transforms ensure:

Stream Type          | Source Format    | Transform                  | Result
---------------------|------------------|----------------------------|-------------
Observations         | µs (Bronze)      | microseconds_to_timestamp  | TIMESTAMPTZ
Forecasts issue_time | µs (Bronze)      | microseconds_to_timestamp  | TIMESTAMPTZ
Forecasts valid_time | sec (Parser tag) | unix_seconds               | TIMESTAMPTZ
```

---

## 9. Testing Strategy

### 9.1 Unit Tests

```
// Test: Pre-transform produces correct flattened rows
test_pre_transform_basic():
    bronze_row = BronzeRow {
        timestamp: 1704067200000000,  // 2024-01-01 00:00:00 UTC
        ndp_id: "weather-nws-002",
        raw_payload: """
        {
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2024-01-01T06:00:00+00:00/PT1H", "value": 15.5},
                        {"validTime": "2024-01-01T07:00:00+00:00/PT1H", "value": 16.0}
                    ]
                },
                "windSpeed": {
                    "values": [
                        {"validTime": "2024-01-01T06:00:00+00:00/PT1H", "value": 10.0}
                    ]
                }
            }
        }
        """
    }

    config = PreTransformConfig {
        enabled: true,
        parser_type: "column_oriented",
        metrics_base_path: "properties",
        columns: [
            ColumnMapping { metric_path: "temperature", field_name: "temperature" },
            ColumnMapping { metric_path: "windSpeed", field_name: "wind_speed" }
        ],
        timestamp_format: TimestampFormat::Iso8601Duration
    }

    result = pre_transform([bronze_row], config)

    assert result.is_ok()
    flattened = result.unwrap()

    // 2 temperature + 1 wind_speed = 3 rows
    assert_eq flattened.len(), 3

    // Check first temperature row
    assert_eq flattened[0].metric_name, "temperature"
    assert_eq flattened[0].value, 15.5
    assert_eq flattened[0].valid_time.hour(), 6


// Test: Missing metric column is gracefully skipped
test_pre_transform_missing_metric():
    bronze_row = BronzeRow {
        raw_payload: """
        {
            "properties": {
                "temperature": { "values": [{"validTime": "...", "value": 15.5}] }
                // windSpeed is missing
            }
        }
        """
    }

    config = PreTransformConfig {
        columns: [
            ColumnMapping { metric_path: "temperature", field_name: "temperature" },
            ColumnMapping { metric_path: "windSpeed", field_name: "wind_speed" }  // Not in payload
        ]
    }

    result = pre_transform([bronze_row], config)

    // Should succeed with only temperature rows
    assert result.is_ok()
    assert_eq result.unwrap().len(), 1  // Only temperature row


// Test: Invalid timestamp is skipped
test_pre_transform_invalid_timestamp():
    bronze_row = BronzeRow {
        raw_payload: """
        {
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "invalid-timestamp", "value": 15.5},
                        {"validTime": "2024-01-01T06:00:00+00:00/PT1H", "value": 16.0}
                    ]
                }
            }
        }
        """
    }

    result = pre_transform([bronze_row], config)

    // Should succeed with only valid row
    assert result.is_ok()
    assert_eq result.unwrap().len(), 1
```

### 9.2 Integration Tests

```
// Test: Full ETL with pre-transform
test_etl_with_pre_transform():
    // Setup: Create test Bronze Parquet file
    bronze_path = create_test_bronze_parquet("nws-gridpoints-forecast")

    // Setup: Configure ETL with pre-transform
    config = load_config("nws-gridpoints-forecast")
    assert config.silver_etl.pre_transform.enabled

    // Execute ETL
    result = run_etl("nws-gridpoints-forecast", config)

    assert result.is_ok()

    // Verify: Silver table has pivoted rows
    rows = query_silver("SELECT * FROM silver.nws_forecasts LIMIT 10")

    assert rows.len() > 0
    assert rows[0].has_column("issue_time")
    assert rows[0].has_column("valid_time")
    assert rows[0].has_column("temperature_c")
    assert rows[0].has_column("wind_speed_kmh")
```

---

## 10. Performance Considerations

### 10.1 Memory Management

```
// Pre-allocate vectors based on expected expansion
estimated_points_per_row = columns.len() * 150  // ~150 values per metric
estimated_capacity = bronze_rows.len() * estimated_points_per_row

flattened_rows = Vec::with_capacity(estimated_capacity)

// Use streaming/batching for large datasets
const BATCH_SIZE: usize = 1000

for batch in bronze_rows.chunks(BATCH_SIZE):
    flattened_batch = pre_transform_batch(batch, config)?

    // Process batch through DuckDB
    duckdb.insert_batch("bronze_data", flattened_batch)?
```

### 10.2 Parallelization Opportunities

```
// Pre-transform is embarrassingly parallel at row level
// Consider using rayon for parallel processing

use rayon::prelude::*

function pre_transform_parallel(
    bronze_rows: Vec<BronzeRow>,
    config: PreTransformConfig
) -> Result<Vec<FlattenedRow>, Error>:

    let results: Vec<Result<Vec<FlattenedRow>, _>> = bronze_rows
        .par_iter()
        .map(|row| process_bronze_row_safe(row, &config))
        .collect()

    // Aggregate results
    let flattened: Vec<FlattenedRow> = results
        .into_iter()
        .filter_map(|r| r.ok())
        .flatten()
        .collect()

    return Ok(flattened)
```

---

## 11. Summary

This pseudocode defines:

1. **PreTransformConfig** - Configuration structure for enabling parser-based pre-transform
2. **Pre-transform Algorithm** - Uses existing ColumnOrientedParser to flatten array data
3. **ETL Integration** - Modified pipeline with optional pre-transform step
4. **Pivot Logic** - SQL generation to pivot metric_name/value into individual columns
5. **Error Handling** - Graceful degradation with DQ transparency tracking
6. **Configuration** - YAML examples and Rust type extensions

The implementation reuses the existing `ColumnOrientedParser` from `core/src/parsers/column_oriented.rs`,
requiring minimal new code while enabling complex columnar array data to flow through the
config-driven Silver ETL pipeline.

---

## References

- `core/src/parsers/column_oriented.rs` - Existing parser implementation
- `core/src/parsers/config.rs` - Parser configuration types
- `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` - Silver ETL design
- `config/base/streams/nws-gridpoints-forecast/config.yaml` - Target stream config
- `product/features/dp-007/SCOPE.md` - Feature scope document
