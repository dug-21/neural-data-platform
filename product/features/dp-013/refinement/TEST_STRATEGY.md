# dp-013: CSV Source Type & Dimension Tables - Test Strategy

## Overview

This document defines the comprehensive test strategy for dp-013, following NDP's established London School TDD patterns with mockall for behavior verification. The testing approach mirrors patterns from dp-011 (ETL persistence) and ndp-mcp-server testing.

---

## 1. Test Pyramid

### Unit Tests (Fast, Isolated)

Test individual functions with mocked dependencies. Target: 80%+ coverage.

#### CSV Parser Tests
```rust
#[cfg(test)]
mod csv_parser_tests {
    // File: core/src/sources/csv.rs or apps/silver-etl/src/csv_source.rs

    #[test]
    fn test_parse_csv_valid_simple() {
        // Standard CSV with headers
    }

    #[test]
    fn test_parse_csv_with_custom_delimiter() {
        // Semicolon, tab, pipe delimiters
    }

    #[test]
    fn test_parse_csv_quoted_fields() {
        // Fields with commas inside quotes
    }

    #[test]
    fn test_parse_csv_escaped_quotes() {
        // Double-quote escaping: ""value""
    }

    #[test]
    fn test_parse_csv_unicode_utf8() {
        // Non-ASCII characters
    }

    #[test]
    fn test_parse_csv_windows_line_endings() {
        // CRLF handling
    }

    #[test]
    fn test_parse_csv_empty_fields() {
        // Empty strings vs nulls
    }

    #[test]
    fn test_parse_csv_whitespace_trimming() {
        // Leading/trailing whitespace
    }
}
```

#### Timestamp Parsing Tests
```rust
#[cfg(test)]
mod timestamp_tests {
    #[test]
    fn test_timestamp_iso8601_standard() {
        // 2024-01-15T10:30:00Z
    }

    #[test]
    fn test_timestamp_iso8601_with_timezone() {
        // 2024-01-15T10:30:00-05:00
    }

    #[test]
    fn test_timestamp_iso8601_milliseconds() {
        // 2024-01-15T10:30:00.123Z
    }

    #[test]
    fn test_timestamp_epoch_seconds() {
        // 1705312200
    }

    #[test]
    fn test_timestamp_epoch_milliseconds() {
        // 1705312200000
    }

    #[test]
    fn test_timestamp_custom_format() {
        // strftime: "%Y/%m/%d %H:%M"
    }

    #[test]
    fn test_timestamp_invalid_format() {
        // Malformed timestamps -> error
    }

    #[test]
    fn test_timestamp_missing_field() {
        // Row without timestamp column -> error
    }
}
```

#### Column Mapping Tests
```rust
#[cfg(test)]
mod column_mapping_tests {
    #[test]
    fn test_mapping_source_to_target_name() {
        // source_field: "pm25" -> name: "pm25_value"
    }

    #[test]
    fn test_mapping_with_type_conversion() {
        // String "22.5" -> float 22.5
    }

    #[test]
    fn test_mapping_missing_optional_field() {
        // Optional field absent -> null
    }

    #[test]
    fn test_mapping_missing_required_field() {
        // Required field absent -> error
    }

    #[test]
    fn test_mapping_type_mismatch() {
        // "not_a_number" -> float -> error
    }

    #[test]
    fn test_mapping_boolean_variants() {
        // "true", "TRUE", "1", "yes" -> true
    }

    #[test]
    fn test_mapping_extra_columns_ignored() {
        // Unmapped columns don't cause errors
    }
}
```

#### Schema Validation Tests
```rust
#[cfg(test)]
mod schema_validation_tests {
    #[test]
    fn test_validate_config_valid() {
        // All required fields present
    }

    #[test]
    fn test_validate_config_missing_path() {
        // source.path required for CSV
    }

    #[test]
    fn test_validate_config_missing_timestamp_field() {
        // timestamp_field required
    }

    #[test]
    fn test_validate_dimension_primary_key() {
        // target.primary_key required for dimensions
    }

    #[test]
    fn test_validate_dimension_fields_match_pk() {
        // Primary key columns must exist in schema.fields
    }

    #[test]
    fn test_validate_load_strategy_valid() {
        // truncate_and_load, upsert are valid
    }

    #[test]
    fn test_validate_load_strategy_invalid() {
        // Unknown strategy -> error
    }
}
```

#### Load Strategy Logic Tests
```rust
#[cfg(test)]
mod load_strategy_tests {
    #[test]
    fn test_truncate_and_load_generates_delete_insert() {
        // Strategy generates: DELETE + INSERT
    }

    #[test]
    fn test_upsert_generates_on_conflict() {
        // Strategy generates: INSERT ... ON CONFLICT DO UPDATE
    }

    #[test]
    fn test_upsert_pk_columns_in_conflict() {
        // primary_key columns used in ON CONFLICT
    }

    #[test]
    fn test_strategy_default_is_truncate() {
        // Missing strategy defaults to truncate_and_load
    }
}
```

### Integration Tests (Slower, Real Dependencies)

Test component interactions with actual services. Mark with `#[ignore]`.

#### CSV to Bronze Parquet Flow
```rust
#[tokio::test]
#[ignore] // Requires: temp directory for Parquet output
async fn test_csv_source_ingests_to_bronze() {
    // Setup: CSV file + stream config
    // Execute: CSV adapter reads file
    // Verify: Parquet file created with correct schema
}

#[tokio::test]
#[ignore]
async fn test_csv_multiple_files_in_directory() {
    // Setup: Multiple CSV files in source.path
    // Execute: Adapter processes all files
    // Verify: All rows combined in Bronze
}

#[tokio::test]
#[ignore]
async fn test_csv_ingest_idempotent() {
    // Setup: CSV file already ingested
    // Execute: Run ingest again
    // Verify: No duplicate data (based on strategy)
}
```

#### Bronze to Silver ETL with CSV Data
```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB
async fn test_csv_data_promoted_to_silver() {
    // Setup: Bronze Parquet from CSV source
    // Execute: Normal ETL pipeline
    // Verify: Data in Silver table with correct types
}

#[tokio::test]
#[ignore]
async fn test_csv_dq_flags_applied() {
    // Setup: CSV with out-of-range values
    // Execute: ETL with DQ rules
    // Verify: dq_flags populated correctly
}
```

#### Dimension Truncate-and-Load
```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB
async fn test_dimension_truncate_load_empty_table() {
    // Setup: Empty target table
    // Execute: Load dimension
    // Verify: All rows inserted
}

#[tokio::test]
#[ignore]
async fn test_dimension_truncate_load_replaces_existing() {
    // Setup: Table with existing data
    // Execute: Load dimension with different data
    // Verify: Old data removed, new data present
}

#[tokio::test]
#[ignore]
async fn test_dimension_truncate_load_atomic() {
    // Setup: Table with data, CSV with error in middle
    // Execute: Load dimension (should fail)
    // Verify: Original data preserved (transaction rollback)
}
```

#### Dimension Upsert
```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB
async fn test_dimension_upsert_inserts_new() {
    // Setup: Empty table
    // Execute: Upsert dimension
    // Verify: All rows inserted
}

#[tokio::test]
#[ignore]
async fn test_dimension_upsert_updates_existing() {
    // Setup: Table with row (pk=A, value=1)
    // Execute: Upsert with (pk=A, value=2)
    // Verify: Row updated to value=2
}

#[tokio::test]
#[ignore]
async fn test_dimension_upsert_mixed_insert_update() {
    // Setup: Table with row A
    // Execute: Upsert with rows A (modified) and B (new)
    // Verify: A updated, B inserted
}

#[tokio::test]
#[ignore]
async fn test_dimension_upsert_preserves_unmatched() {
    // Setup: Table with rows A, B
    // Execute: Upsert with row C only
    // Verify: A, B unchanged, C added
}
```

#### CLI Command Execution
```rust
#[tokio::test]
#[ignore] // Requires: Full CLI binary
async fn test_cli_dimension_list() {
    // Setup: Config with dimensions
    // Execute: ndp dimension list
    // Verify: Output lists all dimensions
}

#[tokio::test]
#[ignore]
async fn test_cli_dimension_sync_specific() {
    // Setup: Config with entity-context dimension
    // Execute: ndp dimension sync entity-context
    // Verify: Dimension loaded, output shows stats
}

#[tokio::test]
#[ignore]
async fn test_cli_dimension_sync_all() {
    // Setup: Config with multiple dimensions
    // Execute: ndp dimension sync --all
    // Verify: All dimensions loaded
}

#[tokio::test]
#[ignore]
async fn test_cli_dimension_dry_run() {
    // Setup: Config with dimension
    // Execute: ndp dimension sync entity-context --dry-run
    // Verify: No database changes, output shows what would happen
}

#[tokio::test]
#[ignore]
async fn test_cli_stream_ingest_csv() {
    // Setup: Stream config with source.type: csv
    // Execute: ndp stream ingest historical-aq
    // Verify: CSV data in Bronze
}
```

#### deploy.sh Sync Integration
```rust
#[tokio::test]
#[ignore] // Requires: Docker + etcd
async fn test_deploy_sync_includes_dimensions() {
    // Setup: Dimension configs in config/base/dimensions/
    // Execute: ./deploy.sh sync
    // Verify: Dimensions synced to etcd, loaded to Silver
}
```

### End-to-End Tests

Full workflow validation.

```rust
#[tokio::test]
#[ignore] // Requires: Full infrastructure
async fn test_full_csv_import_workflow() {
    // 1. Create stream config with source.type: csv
    // 2. Place CSV file at configured path
    // 3. Run ndp stream ingest <stream-id>
    // 4. Verify Bronze Parquet created
    // 5. Run silver-etl
    // 6. Verify Silver table populated
    // 7. Query via gold views
}

#[tokio::test]
#[ignore]
async fn test_full_dimension_workflow() {
    // 1. Create dimension config
    // 2. Create CSV data file
    // 3. Run ndp dimension sync
    // 4. Verify Silver dimension table
    // 5. Create gold view joining dimension
    // 6. Query enriched data
}

#[tokio::test]
#[ignore]
async fn test_error_recovery_workflow() {
    // 1. Start dimension load
    // 2. Simulate network failure mid-load
    // 3. Verify partial data not committed
    // 4. Retry load
    // 5. Verify complete data loaded
}
```

---

## 2. Test Data

### Test Fixtures Directory Structure

```
apps/silver-etl/tests/fixtures/csv/
├── valid/
│   ├── simple.csv                    # Basic CSV, all types
│   ├── timestamps_iso8601.csv        # ISO8601 timestamps
│   ├── timestamps_epoch.csv          # Epoch timestamps
│   ├── timestamps_custom.csv         # Custom format timestamps
│   ├── quoted_fields.csv             # Fields with quotes/commas
│   ├── unicode.csv                   # UTF-8 characters
│   ├── semicolon_delimited.csv       # Non-comma delimiter
│   └── large_1m_rows.csv             # Performance testing
├── invalid/
│   ├── malformed_row.csv             # Bad row in middle
│   ├── missing_header.csv            # No header row
│   ├── missing_required_column.csv   # Required column absent
│   ├── type_mismatch.csv             # String where float expected
│   ├── invalid_timestamp.csv         # Unparseable timestamp
│   └── empty.csv                     # Zero rows (just header)
└── dimensions/
    ├── entity_context.csv            # Reference data example
    ├── with_duplicates.csv           # Duplicate primary keys
    └── partial_update.csv            # For upsert testing
```

### Fixture Definitions

#### Valid CSV with All Supported Types (simple.csv)
```csv
timestamp,stream_id,pm25,temperature,humidity,is_outdoor,status,metadata
2024-01-15T10:00:00Z,sensor-001,12.5,22.3,45.2,true,active,{"model":"v2"}
2024-01-15T10:05:00Z,sensor-001,14.2,22.1,46.0,true,active,{"model":"v2"}
2024-01-15T10:10:00Z,sensor-001,11.8,22.5,44.8,true,active,{"model":"v2"}
```

#### CSV with Malformed Rows (malformed_row.csv)
```csv
timestamp,pm25,temperature
2024-01-15T10:00:00Z,12.5,22.3
2024-01-15T10:05:00Z,invalid,22.1
2024-01-15T10:10:00Z,11.8,22.5
```

#### Large CSV for Performance (generate script)
```rust
fn generate_large_csv(path: &str, rows: usize) {
    let mut wtr = csv::Writer::from_path(path).unwrap();
    wtr.write_record(&["timestamp", "sensor_id", "value"]).unwrap();
    for i in 0..rows {
        let ts = format!("2024-01-15T10:{:02}:{:02}Z", i / 3600 % 60, i / 60 % 60);
        wtr.write_record(&[&ts, &format!("sensor-{}", i % 100), &format!("{:.2}", i as f64 * 0.1)]).unwrap();
    }
}
```

#### Dimension CSV with Duplicates (with_duplicates.csv)
```csv
ndp_id,category,friendly_name
door_back,door,Back Door
door_back,window,Back Window
door_front,door,Front Door
```

#### Various Timestamp Formats
```csv
# timestamps_iso8601.csv
timestamp,value
2024-01-15T10:00:00Z,100
2024-01-15T10:00:00.123Z,101
2024-01-15T10:00:00+05:00,102

# timestamps_epoch.csv
timestamp,value
1705312200,100
1705312260,101
1705312320,102

# timestamps_custom.csv (format: "%Y/%m/%d %H:%M")
timestamp,value
2024/01/15 10:00,100
2024/01/15 10:01,101
```

---

## 3. Test Cases Matrix

### Part 1: CSV Source Type

| AC | Test Type | Test Case | Expected Result | Priority |
|----|-----------|-----------|-----------------|----------|
| `source.type: csv` recognized | Unit | `test_validate_config_csv_source_type` | Config with csv type passes validation | High |
| CSV adapter implemented | Unit | `test_csv_adapter_implements_source_trait` | CsvSource implements Source trait | High |
| timestamp_field parsing | Unit | `test_timestamp_field_extracted` | Correct column used for timestamp | High |
| timestamp_format: iso8601 | Unit | `test_timestamp_iso8601_parsed` | ISO8601 strings converted to DateTime | High |
| timestamp_format: epoch_seconds | Unit | `test_timestamp_epoch_seconds_parsed` | Unix timestamps converted to DateTime | High |
| timestamp_format: custom | Unit | `test_timestamp_custom_format_parsed` | Custom strftime pattern applied | Medium |
| Column mapping via entity_schemas | Unit | `test_entity_schema_column_mapping` | source_field maps to name | High |
| Data lands in Bronze Parquet | Integration | `test_csv_source_creates_parquet` | Parquet file with correct schema | High |
| Normal ETL promotes to Silver | Integration | `test_csv_data_promoted_to_silver` | Data in Silver table | High |
| Invalid rows logged and skipped | Unit | `test_invalid_row_skipped_with_log` | Error logged, processing continues | High |
| on_error: skip | Unit | `test_on_error_skip_continues` | Invalid rows skipped | Medium |
| on_error: abort | Unit | `test_on_error_abort_stops` | First error stops processing | Medium |
| CLI: ndp stream ingest | Integration | `test_cli_stream_ingest_csv` | CSV ingested to Bronze | High |

### Part 2: Dimension Table Configs

| AC | Test Type | Test Case | Expected Result | Priority |
|----|-----------|-----------|-----------------|----------|
| Dimension config schema | Unit | `test_dimension_config_deserializes` | YAML parsed to struct | High |
| dimension_id required | Unit | `test_dimension_missing_id_fails` | Validation error | High |
| target.table required | Unit | `test_dimension_missing_table_fails` | Validation error | High |
| target.primary_key required | Unit | `test_dimension_missing_pk_fails` | Validation error | High |
| Config files in dimensions/ | Integration | `test_dimension_configs_loaded_from_dir` | All YAML files in dir loaded | High |
| CSV source for dimensions | Unit | `test_dimension_csv_source_parsed` | source.type: csv works | High |
| Schema validation: required | Unit | `test_dimension_required_field_enforced` | Missing required field errors | High |
| Schema validation: data types | Unit | `test_dimension_type_validation` | Type mismatches detected | High |
| truncate_and_load strategy | Integration | `test_truncate_and_load_replaces_data` | DELETE + INSERT in transaction | High |
| upsert strategy | Integration | `test_upsert_updates_or_inserts` | ON CONFLICT DO UPDATE works | High |
| Table auto-created | Integration | `test_dimension_table_created_if_missing` | CREATE TABLE IF NOT EXISTS | Medium |
| deploy.sh sync processes | Integration | `test_deploy_sync_loads_dimensions` | Dimensions loaded after sync | High |

### Part 3: CLI

| AC | Test Type | Test Case | Expected Result | Priority |
|----|-----------|-----------|-----------------|----------|
| dimension list | Integration | `test_cli_dimension_list_output` | All dimensions shown | Medium |
| dimension sync <id> | Integration | `test_cli_dimension_sync_single` | Specific dimension loaded | High |
| dimension sync --all | Integration | `test_cli_dimension_sync_all` | All dimensions loaded | High |
| dimension sync --dry-run | Integration | `test_cli_dimension_dry_run_no_changes` | Validation only, no DB writes | High |
| Summary output | Unit | `test_cli_output_format` | "rows: N processed, N loaded, N errors" | Medium |
| Exit code 0 on success | Integration | `test_cli_exit_code_success` | Exit code 0 | High |
| Exit code non-zero on failure | Integration | `test_cli_exit_code_failure` | Exit code != 0 | High |

### Error Handling

| Error Type | Test Case | Expected Behavior | Priority |
|------------|-----------|-------------------|----------|
| Malformed CSV | `test_malformed_csv_parse_error` | Error with line number, abort | High |
| Missing required columns | `test_missing_column_validation_error` | Clear error before load | High |
| Type conversion failures | `test_type_conversion_error_handling` | Log, skip row (or abort) | High |
| File not found | `test_file_not_found_error` | Clear error with path | High |
| Empty file | `test_empty_file_warning` | Warning logged, no-op | Medium |

---

## 4. Mocking Strategy

### What to Mock (London School TDD)

Following the dp-011 pattern with `#[cfg_attr(test, mockall::automock)]`:

#### File System for CSV Reading
```rust
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait CsvReader: Send + Sync {
    async fn read_csv(&self, path: &str) -> Result<Vec<CsvRow>, CsvError>;
    fn file_exists(&self, path: &str) -> bool;
    fn list_csv_files(&self, dir: &str) -> Result<Vec<String>, CsvError>;
}
```

#### TimescaleDB for Dimension Loading
```rust
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait DimensionStore: Send + Sync {
    async fn truncate_and_load(&self, table: &str, rows: Vec<DimensionRow>) -> Result<LoadStats, DimensionError>;
    async fn upsert(&self, table: &str, pk_columns: &[String], rows: Vec<DimensionRow>) -> Result<LoadStats, DimensionError>;
    async fn table_exists(&self, table: &str) -> bool;
    async fn create_table(&self, table: &str, schema: &DimensionSchema) -> Result<(), DimensionError>;
}
```

#### etcd for Config Retrieval
```rust
// Reuse existing ConfigStore trait from ndp-mcp-server
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait ConfigStore: Send + Sync {
    async fn get_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
    async fn get_dimension_config(&self, dimension_id: &str) -> Result<DimensionConfig, ConfigError>;
    async fn list_dimensions(&self) -> Result<Vec<String>, ConfigError>;
}
```

### What NOT to Mock

These components have core logic that must be tested directly:

1. **CSV Parsing Logic**
   - Delimiter handling
   - Quote escaping
   - Line ending normalization
   - Encoding conversion

2. **Schema Validation**
   - Field type checking
   - Required field enforcement
   - Primary key validation

3. **Timestamp Parsing**
   - Format detection
   - Timezone handling
   - Epoch conversion

4. **Load Strategy SQL Generation**
   - DELETE + INSERT generation
   - ON CONFLICT clause building
   - Transaction wrapping

### Mock Factory Pattern (from dp-011)

```rust
// tests/helpers/mock_factories.rs

pub fn success_csv_reader() -> MockCsvReader {
    let mut mock = MockCsvReader::new();
    mock.expect_read_csv()
        .returning(|_| Ok(vec![test_csv_row()]));
    mock.expect_file_exists()
        .returning(|_| true);
    mock
}

pub fn empty_csv_reader() -> MockCsvReader {
    let mut mock = MockCsvReader::new();
    mock.expect_read_csv()
        .returning(|_| Ok(vec![]));
    mock.expect_file_exists()
        .returning(|_| true);
    mock
}

pub fn failing_csv_reader(error: &str) -> MockCsvReader {
    let err = error.to_string();
    let mut mock = MockCsvReader::new();
    mock.expect_read_csv()
        .returning(move |_| Err(CsvError::Parse(err.clone())));
    mock
}

pub fn success_dimension_store() -> MockDimensionStore {
    let mut mock = MockDimensionStore::new();
    mock.expect_truncate_and_load()
        .returning(|_, rows| Ok(LoadStats { loaded: rows.len(), errors: 0 }));
    mock.expect_upsert()
        .returning(|_, _, rows| Ok(LoadStats { loaded: rows.len(), errors: 0 }));
    mock.expect_table_exists()
        .returning(|_| true);
    mock
}

pub fn tracking_dimension_store() -> (MockDimensionStore, Arc<Mutex<Vec<DimensionCall>>>) {
    let calls = Arc::new(Mutex::new(vec![]));
    let calls_clone = calls.clone();

    let mut mock = MockDimensionStore::new();
    mock.expect_truncate_and_load()
        .returning(move |table, rows| {
            calls_clone.lock().unwrap().push(DimensionCall::TruncateLoad {
                table: table.to_string(),
                row_count: rows.len(),
            });
            Ok(LoadStats { loaded: rows.len(), errors: 0 })
        });

    (mock, calls)
}
```

---

## 5. Error Scenarios

### Malformed CSV Tests

```rust
#[test]
fn test_malformed_csv_missing_quote() {
    let csv = r#"name,value
"unclosed,123"#;
    let result = parse_csv(csv);
    assert!(matches!(result, Err(CsvError::Parse { line: 2, .. })));
}

#[test]
fn test_malformed_csv_wrong_column_count() {
    let csv = r#"name,value
one,1
two,2,extra
three,3"#;
    let result = parse_csv(csv);
    assert!(matches!(result, Err(CsvError::ColumnCount { line: 3, .. })));
}

#[test]
fn test_malformed_csv_encoding_error() {
    let invalid_utf8 = vec![0x80, 0x81]; // Invalid UTF-8
    let result = parse_csv_bytes(&invalid_utf8);
    assert!(matches!(result, Err(CsvError::Encoding(_))));
}
```

### Missing Required Columns Tests

```rust
#[test]
fn test_missing_timestamp_field() {
    let csv = "id,value\n1,100";
    let config = stream_config_with_timestamp_field("timestamp");
    let result = validate_csv_against_config(csv, &config);
    assert!(matches!(
        result,
        Err(CsvError::MissingColumn { column: c, .. }) if c == "timestamp"
    ));
}

#[test]
fn test_missing_primary_key_column() {
    let csv = "name,value\none,100";
    let config = dimension_config_with_pk(vec!["id"]);
    let result = validate_csv_against_config(csv, &config);
    assert!(matches!(
        result,
        Err(CsvError::MissingColumn { column: c, .. }) if c == "id"
    ));
}
```

### Type Conversion Failure Tests

```rust
#[test]
fn test_type_conversion_float_from_string() {
    let row = csv_row_with("value", "not_a_number");
    let schema = field_schema("value", DataType::Float);
    let result = convert_field(&row, &schema);
    assert!(matches!(
        result,
        Err(CsvError::TypeConversion { field: f, expected: e, .. })
            if f == "value" && e == "float"
    ));
}

#[test]
fn test_type_conversion_int_from_float_string() {
    let row = csv_row_with("count", "3.14");
    let schema = field_schema("count", DataType::Int);
    let result = convert_field(&row, &schema);
    // Should error - can't convert "3.14" to int
    assert!(matches!(result, Err(CsvError::TypeConversion { .. })));
}

#[test]
fn test_type_conversion_bool_from_invalid() {
    let row = csv_row_with("enabled", "maybe");
    let schema = field_schema("enabled", DataType::Bool);
    let result = convert_field(&row, &schema);
    assert!(matches!(result, Err(CsvError::TypeConversion { .. })));
}
```

### File Not Found Tests

```rust
#[tokio::test]
async fn test_csv_file_not_found() {
    let config = stream_config_with_path("/nonexistent/file.csv");
    let result = CsvSource::new(&config).await;
    assert!(matches!(
        result,
        Err(CsvError::FileNotFound { path: p, .. }) if p.contains("nonexistent")
    ));
}

#[tokio::test]
async fn test_dimension_csv_not_found() {
    let config = dimension_config_with_path("/missing/dimension.csv");
    let loader = DimensionLoader::new(mock_store());
    let result = loader.load(&config).await;
    assert!(matches!(result, Err(DimensionError::CsvError(CsvError::FileNotFound { .. }))));
}
```

### Empty File Tests

```rust
#[test]
fn test_empty_csv_header_only() {
    let csv = "timestamp,value\n";
    let result = parse_csv(csv);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_empty_csv_completely_empty() {
    let csv = "";
    let result = parse_csv(csv);
    assert!(matches!(result, Err(CsvError::EmptyFile)));
}

#[tokio::test]
async fn test_dimension_load_empty_csv_warning() {
    let config = dimension_config_with_fixture("empty.csv");
    let loader = DimensionLoader::new(mock_store());
    let stats = loader.load(&config).await.unwrap();
    assert_eq!(stats.loaded, 0);
    // Verify warning was logged
}
```

---

## 6. Performance Tests

### Large File Handling (1M rows)

```rust
#[tokio::test]
#[ignore] // Performance test - run separately
async fn test_large_csv_1m_rows() {
    // Setup: Generate 1M row CSV (or use fixture)
    let path = generate_large_csv(1_000_000);
    let config = stream_config_with_path(&path);

    let start = Instant::now();
    let source = CsvSource::new(&config).await.unwrap();
    let points = source.fetch().await.unwrap();
    let duration = start.elapsed();

    assert_eq!(points.len(), 1_000_000);
    assert!(
        duration.as_secs() < 60,
        "Should process 1M rows in under 60 seconds, took {:?}",
        duration
    );

    // ~16k rows/sec minimum acceptable
    let rows_per_sec = points.len() as f64 / duration.as_secs_f64();
    assert!(rows_per_sec > 15_000.0, "Throughput too low: {:.0} rows/sec", rows_per_sec);
}

#[tokio::test]
#[ignore]
async fn test_large_csv_streaming_memory() {
    // Verify streaming doesn't load entire file into memory
    let path = generate_large_csv(10_000_000); // 10M rows

    let initial_mem = get_memory_usage_kb();

    let config = stream_config_with_path(&path);
    let source = CsvSource::new(&config).await.unwrap();

    // Stream through without collecting all
    let mut count = 0;
    let mut stream = source.stream();
    while let Some(batch) = stream.next().await {
        count += batch.len();
        if count > 100_000 {
            break; // Don't need to process all for this test
        }
    }

    let peak_mem = get_memory_usage_kb();
    let delta_mb = (peak_mem - initial_mem) / 1024;

    assert!(
        delta_mb < 100,
        "Memory usage should stay under 100MB for streaming, used {} MB",
        delta_mb
    );
}
```

### Memory Usage During Parsing

```rust
#[tokio::test]
#[ignore]
async fn test_memory_bounded_during_parse() {
    let large_csv = generate_large_csv(500_000);

    let initial = get_rss_kb();

    for _ in 0..5 {
        let points = parse_csv_file(&large_csv).await.unwrap();
        drop(points); // Ensure cleanup
    }

    let final_mem = get_rss_kb();
    let growth = (final_mem - initial) / 1024;

    assert!(
        growth < 50,
        "Memory should not grow significantly over iterations, grew {} MB",
        growth
    );
}
```

### Transaction Performance for Dimension Loads

```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB
async fn test_dimension_load_performance() {
    let rows = generate_dimension_rows(100_000);
    let store = TimescaleDimensionStore::from_env().await.unwrap();

    // truncate_and_load performance
    let start = Instant::now();
    let stats = store.truncate_and_load("test.perf_dim", rows.clone()).await.unwrap();
    let truncate_duration = start.elapsed();

    assert_eq!(stats.loaded, 100_000);
    assert!(
        truncate_duration.as_secs() < 10,
        "truncate_and_load 100k rows should complete in <10s, took {:?}",
        truncate_duration
    );

    // upsert performance (update all rows)
    let start = Instant::now();
    let stats = store.upsert("test.perf_dim", &["id"], rows).await.unwrap();
    let upsert_duration = start.elapsed();

    assert_eq!(stats.loaded, 100_000);
    assert!(
        upsert_duration.as_secs() < 30,
        "upsert 100k rows should complete in <30s, took {:?}",
        upsert_duration
    );
}

#[tokio::test]
#[ignore]
async fn test_batch_insert_size_optimization() {
    // Test different batch sizes to find optimal
    let rows = generate_dimension_rows(50_000);
    let store = TimescaleDimensionStore::from_env().await.unwrap();

    for batch_size in [100, 500, 1000, 5000] {
        let config = LoadConfig { batch_size, ..Default::default() };
        let start = Instant::now();
        store.load_with_config("test.batch_dim", rows.clone(), &config).await.unwrap();
        let duration = start.elapsed();
        println!("Batch size {}: {:?}", batch_size, duration);
    }

    // Assert reasonable performance with recommended batch size (1000)
}
```

---

## 7. Test Execution

### Running Tests

```bash
# All unit tests
cargo test -p silver-etl --lib

# Specific module tests
cargo test -p silver-etl csv_parser
cargo test -p silver-etl timestamp_parsing
cargo test -p silver-etl dimension_loader

# With output
cargo test -p silver-etl -- --nocapture

# Integration tests (requires Docker)
docker compose -f deploy/docker-compose.test.yml up -d
cargo test -p silver-etl --test integration_tests -- --ignored

# Specific integration test
cargo test -p silver-etl test_csv_source_ingests_to_bronze -- --ignored

# Performance tests (run separately)
cargo test -p silver-etl --test performance_tests -- --ignored --test-threads=1

# Coverage (requires cargo-tarpaulin)
cargo tarpaulin -p silver-etl --out Html --output-dir target/coverage
```

### Test Infrastructure Setup

```bash
# Start test infrastructure
docker compose -f deploy/docker-compose.test.yml up -d

# Verify services
docker compose -f deploy/docker-compose.test.yml ps

# Expected services:
# - timescaledb-test (port 5433)
# - etcd-test (port 2380)

# Load test fixtures
./scripts/load_test_fixtures.sh

# Cleanup after tests
docker compose -f deploy/docker-compose.test.yml down -v
```

### CI Configuration

```yaml
# .github/workflows/dp-013-tests.yml
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test -p silver-etl --lib

  integration-tests:
    runs-on: ubuntu-latest
    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg16
        env:
          POSTGRES_PASSWORD: test
        ports:
          - 5433:5432
      etcd:
        image: bitnami/etcd:latest
        env:
          ALLOW_NONE_AUTHENTICATION: "yes"
        ports:
          - 2380:2379
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test -p silver-etl --test integration_tests -- --ignored
```

---

## 8. Related Patterns

This test strategy incorporates patterns from:

- **mcp-tool-testing-pattern**: MockConfigStore, MockBronzeStorage with `#[cfg_attr(test, automock)]`
- **dp011-persistence-tdd-pattern**: Mock factories (success_persistence, failing_persistence, tracking_persistence)
- **mock-etl-run-persistence-pattern**: Expectation setup with `.expect_method().with(...).times(N).returning(...)`
- **AIR-005-TEST-DESIGN**: London School TDD principles, behavior verification, integration contract testing

---

## 9. Test Checklist

Before marking dp-013 testing complete:

- [ ] Unit tests for CSV parsing (all edge cases)
- [ ] Unit tests for timestamp parsing (all formats)
- [ ] Unit tests for column mapping
- [ ] Unit tests for schema validation
- [ ] Unit tests for load strategy logic
- [ ] Integration tests for CSV -> Bronze flow
- [ ] Integration tests for Bronze -> Silver ETL
- [ ] Integration tests for dimension truncate_and_load
- [ ] Integration tests for dimension upsert
- [ ] Integration tests for CLI commands
- [ ] Integration tests for deploy.sh sync
- [ ] Error scenario tests (malformed, missing, type errors)
- [ ] Performance tests for large files
- [ ] All mocks follow factory pattern
- [ ] Test fixtures created and documented
- [ ] CI configuration updated
- [ ] Coverage > 80% for CSV parsing logic
