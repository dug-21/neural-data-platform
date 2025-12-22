//! Integration tests for DuckDB Silver Layer with Parquet data
//!
//! Feature: DP-001 - Virtual Silver Layer using DuckDB
//! Test Approach: London School TDD (outside-in, integration with mocks)
//!
//! These tests verify:
//! - DuckDB can read Parquet files from Bronze layer
//! - Silver views return expected schema and data
//! - Query performance meets targets
//! - Integration with real Parquet data structures
//!
//! Test Philosophy:
//! - Use real DuckDB + mock Parquet files
//! - Test end-to-end data flow (Parquet → DuckDB → Views)
//! - Performance benchmarks (7-day query < 5s)
//! - Mark as #[ignore] for CI flexibility

use duckdb::{Connection, Result as DuckDbResult};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tempfile::TempDir;

// ========== TEST FIXTURES ==========

/// Create a temporary directory for test Parquet files
fn setup_test_parquet_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Create a DuckDB connection with temporary database
fn setup_duckdb() -> Connection {
    Connection::open_in_memory().expect("Failed to create DuckDB connection")
}

/// Generate test Parquet file with AirGradient schema
fn create_test_parquet_file(dir: &Path, filename: &str, row_count: usize) -> PathBuf {
    use chrono::{Duration, Utc};
    use parquet::basic::{Compression, Encoding};
    use parquet::file::properties::WriterProperties;
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;
    use std::fs::File;
    use std::sync::Arc;

    let file_path = dir.join(filename);

    // Define Parquet schema matching Bronze layer structure
    let schema = Arc::new(
        parse_message_type(
            "message schema {
                REQUIRED INT64 timestamp (TIMESTAMP(MICROS,true));
                OPTIONAL DOUBLE pm25;
                OPTIONAL DOUBLE pm10;
                OPTIONAL DOUBLE co2;
                OPTIONAL DOUBLE temperature;
                OPTIONAL DOUBLE humidity;
                OPTIONAL DOUBLE tvoc;
                OPTIONAL DOUBLE nox;
            }",
        )
        .expect("Failed to parse schema"),
    );

    let props = Arc::new(
        WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build(),
    );

    let file = File::create(&file_path).expect("Failed to create Parquet file");
    let mut writer =
        SerializedFileWriter::new(file, schema.clone(), props).expect("Failed to create writer");

    let base_time = Utc::now();

    // Write test rows
    for i in 0..row_count {
        let mut row_group_writer = writer.next_row_group().expect("Failed to get row group");

        // Write timestamp column
        if let Some(mut col_writer) = row_group_writer
            .next_column()
            .expect("Failed to get column")
        {
            use parquet::column::writer::ColumnWriter;
            if let ColumnWriter::Int64ColumnWriter(ref mut typed) = col_writer {
                let timestamp = (base_time + Duration::minutes(i as i64)).timestamp_micros();
                typed
                    .write_batch(&[timestamp], None, None)
                    .expect("Failed to write timestamp");
            }
            row_group_writer
                .close_column(col_writer)
                .expect("Failed to close column");
        }

        // Write pm25 column (with variety: valid, boundary, out-of-range, NULL)
        if let Some(mut col_writer) = row_group_writer
            .next_column()
            .expect("Failed to get column")
        {
            use parquet::column::writer::ColumnWriter;
            if let ColumnWriter::DoubleColumnWriter(ref mut typed) = col_writer {
                let value = match i % 10 {
                    0 => 0.0,                     // Boundary: min
                    1 => 500.0,                   // Boundary: max
                    2 => -10.0,                   // Out-of-range: below min
                    3 => 600.0,                   // Out-of-range: above max
                    _ => 25.0 + (i as f64 * 2.5), // Valid values
                };
                let def_level = if i % 10 == 4 { 0 } else { 1 }; // Every 5th is NULL
                typed
                    .write_batch(&[value], Some(&[def_level]), None)
                    .expect("Failed to write pm25");
            }
            row_group_writer
                .close_column(col_writer)
                .expect("Failed to close column");
        }

        // Write remaining columns (simplified - all valid or NULL)
        for col_idx in 2..8 {
            if let Some(mut col_writer) = row_group_writer
                .next_column()
                .expect("Failed to get column")
            {
                use parquet::column::writer::ColumnWriter;
                if let ColumnWriter::DoubleColumnWriter(ref mut typed) = col_writer {
                    let value = match col_idx {
                        2 => 50.0 + i as f64,         // pm10
                        3 => 450.0 + i as f64,        // co2
                        4 => 22.0 + (i as f64 * 0.1), // temperature
                        5 => 45.0 + (i as f64 * 0.5), // humidity
                        6 => 100.0 + i as f64,        // tvoc
                        7 => 10.0 + (i as f64 * 0.2), // nox
                        _ => 0.0,
                    };
                    let def_level = if i % 15 == 0 { 0 } else { 1 }; // Occasional NULLs
                    typed
                        .write_batch(&[value], Some(&[def_level]), None)
                        .expect("Failed to write column");
                }
                row_group_writer
                    .close_column(col_writer)
                    .expect("Failed to close column");
            }
        }

        row_group_writer.close().expect("Failed to close row group");
    }

    writer.close().expect("Failed to close writer");
    file_path
}

/// Create Silver view for integration testing
fn create_silver_view_from_parquet(conn: &Connection, parquet_path: &Path) -> DuckDbResult<()> {
    let path_str = parquet_path.to_str().expect("Invalid path");

    conn.execute(
        &format!(
            "CREATE OR REPLACE VIEW silver_indoor_air AS
            SELECT
                timestamp,
                CASE
                    WHEN pm25 >= 0 AND pm25 <= 500
                    THEN ROUND(pm25, 1)
                    ELSE NULL
                END as pm25,
                CASE
                    WHEN pm10 >= 0 AND pm10 <= 1000
                    THEN ROUND(pm10, 1)
                    ELSE NULL
                END as pm10,
                CASE
                    WHEN co2 >= 400 AND co2 <= 5000
                    THEN ROUND(co2, 0)
                    ELSE NULL
                END as co2,
                CASE
                    WHEN temperature >= -10 AND temperature <= 50
                    THEN ROUND(temperature, 1)
                    ELSE NULL
                END as temperature,
                CASE
                    WHEN humidity >= 0 AND humidity <= 100
                    THEN ROUND(humidity, 1)
                    ELSE NULL
                END as humidity,
                CASE
                    WHEN tvoc >= 0 AND tvoc <= 60000
                    THEN ROUND(tvoc, 0)
                    ELSE NULL
                END as tvoc,
                CASE
                    WHEN nox >= 0 AND nox <= 1000
                    THEN ROUND(nox, 0)
                    ELSE NULL
                END as nox
            FROM read_parquet('{}')
            WHERE timestamp IS NOT NULL
            ORDER BY timestamp DESC",
            path_str
        ),
        [],
    )?;

    Ok(())
}

// ========== T-DB-001: PARQUET FILE DISCOVERY AND LOADING ==========

#[test]
#[ignore] // Run with: cargo test --ignored
fn test_parquet_file_loading() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "test_data.parquet", 100);
    let conn = setup_duckdb();

    // Act: Load Parquet file via read_parquet()
    let count: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM read_parquet('{}')",
                parquet_file.to_str().unwrap()
            ),
            [],
            |row| row.get(0),
        )
        .expect("Failed to query Parquet file");

    // Assert: Should find 100 rows
    assert_eq!(count, 100, "Should load 100 rows from Parquet file");
}

#[test]
#[ignore]
fn test_parquet_wildcard_expansion() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    create_test_parquet_file(temp_dir.path(), "data_001.parquet", 50);
    create_test_parquet_file(temp_dir.path(), "data_002.parquet", 50);
    create_test_parquet_file(temp_dir.path(), "data_003.parquet", 50);
    let conn = setup_duckdb();

    // Act: Load multiple files via wildcard
    let pattern = format!("{}/*.parquet", temp_dir.path().to_str().unwrap());
    let count: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM read_parquet('{}')", pattern),
            [],
            |row| row.get(0),
        )
        .expect("Failed to query Parquet files");

    // Assert: Should load all 150 rows from 3 files
    assert_eq!(
        count, 150,
        "Should load 150 rows from 3 Parquet files via wildcard"
    );
}

#[test]
#[ignore]
fn test_parquet_loading_empty_directory() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let conn = setup_duckdb();

    // Act: Try to load from empty directory
    let pattern = format!("{}/*.parquet", temp_dir.path().to_str().unwrap());
    let result = conn.query_row(
        &format!("SELECT COUNT(*) FROM read_parquet('{}')", pattern),
        [],
        |row| row.get::<_, i64>(0),
    );

    // Assert: Should error or return zero rows
    // DuckDB returns error for no files matching pattern
    assert!(
        result.is_err(),
        "Should error when no Parquet files are found"
    );
}

// ========== T-DB-002: SCHEMA INFERENCE CORRECTNESS ==========

#[test]
#[ignore]
fn test_schema_inference_types() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "schema_test.parquet", 10);
    let conn = setup_duckdb();

    // Act: Query schema information via DESCRIBE
    let result = conn
        .prepare(&format!(
            "DESCRIBE SELECT * FROM read_parquet('{}')",
            parquet_file.to_str().unwrap()
        ))
        .expect("Failed to describe schema");

    let mut stmt = result;
    let columns: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("Failed to query schema")
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to collect schema");

    // Assert: Check column types
    let timestamp_col = columns.iter().find(|(name, _)| name == "timestamp");
    let pm25_col = columns.iter().find(|(name, _)| name == "pm25");

    assert!(timestamp_col.is_some(), "Should have timestamp column");
    assert!(pm25_col.is_some(), "Should have pm25 column");

    // DuckDB infers TIMESTAMP for INT64 with TIMESTAMP annotation
    assert!(
        timestamp_col.unwrap().1.contains("TIMESTAMP"),
        "timestamp should be TIMESTAMP type"
    );
    assert!(
        pm25_col.unwrap().1.contains("DOUBLE"),
        "pm25 should be DOUBLE type"
    );
}

#[test]
#[ignore]
fn test_schema_consistency_across_files() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    create_test_parquet_file(temp_dir.path(), "file1.parquet", 10);
    create_test_parquet_file(temp_dir.path(), "file2.parquet", 10);
    let conn = setup_duckdb();

    // Act: Load multiple files and check schema consistency
    let pattern = format!("{}/*.parquet", temp_dir.path().to_str().unwrap());
    let result = conn.execute(&format!("SELECT * FROM read_parquet('{}')", pattern), []);

    // Assert: Should succeed (DuckDB validates schema compatibility)
    assert!(
        result.is_ok(),
        "Should load files with consistent schema without error"
    );
}

// ========== T-DB-003: SILVER VIEW INTEGRATION ==========

#[test]
#[ignore]
fn test_view_creation_with_parquet() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "view_test.parquet", 100);
    let conn = setup_duckdb();

    // Act: Create Silver view
    let result = create_silver_view_from_parquet(&conn, &parquet_file);

    // Assert: View creation should succeed
    assert!(result.is_ok(), "Silver view creation should succeed");

    // Verify view is queryable
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM silver_indoor_air", [], |row| {
            row.get(0)
        })
        .expect("Failed to query view");

    assert!(count >= 0, "View should be queryable and return count");
}

#[test]
#[ignore]
fn test_view_filters_invalid_data() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "filter_test.parquet", 100);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Query raw Parquet vs filtered view
    let raw_count: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM read_parquet('{}')",
                parquet_file.to_str().unwrap()
            ),
            [],
            |row| row.get(0),
        )
        .unwrap();

    let view_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM silver_indoor_air", [], |row| {
            row.get(0)
        })
        .unwrap();

    // Assert: View count should equal raw count (all have valid timestamps)
    assert_eq!(
        view_count, raw_count,
        "View should include all rows (timestamp filter only)"
    );

    // Check that out-of-range pm25 values are NULL in view
    let null_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM silver_indoor_air WHERE pm25 IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(
        null_count > 0,
        "View should have NULLs for out-of-range pm25 values"
    );
}

// ========== T-DB-007: QUERY PERFORMANCE BENCHMARKS ==========

#[test]
#[ignore]
fn bench_7_day_query() {
    // Arrange: Simulate 7 days of data (1 reading/minute = 10,080 rows)
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "7day_data.parquet", 10_080);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Query 7-day data
    let start = Instant::now();
    let _count: i64 = conn
        .query_row("SELECT COUNT(*) FROM silver_indoor_air", [], |row| {
            row.get(0)
        })
        .unwrap();
    let duration = start.elapsed();

    // Assert: Should complete in < 5 seconds
    println!("7-day query duration: {:?}", duration);
    assert!(
        duration.as_secs() < 5,
        "7-day query should complete in < 5s, took {:?}",
        duration
    );
}

#[test]
#[ignore]
fn bench_aggregation_query() {
    // Arrange: 30 days of data (43,200 rows)
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "30day_data.parquet", 43_200);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Hourly aggregation query
    let start = Instant::now();
    let _rows = conn
        .prepare(
            "SELECT
                DATE_TRUNC('hour', timestamp) as hour,
                AVG(pm25) as avg_pm25,
                MAX(pm25) as max_pm25,
                MIN(pm25) as min_pm25
             FROM silver_indoor_air
             WHERE pm25 IS NOT NULL
             GROUP BY hour
             ORDER BY hour",
        )
        .unwrap()
        .query_map([], |_row| Ok(()))
        .unwrap()
        .count();
    let duration = start.elapsed();

    // Assert: Should complete in < 15 seconds
    println!("30-day aggregation query duration: {:?}", duration);
    assert!(
        duration.as_secs() < 15,
        "30-day aggregation should complete in < 15s, took {:?}",
        duration
    );
}

#[test]
#[ignore]
fn bench_time_range_filter() {
    // Arrange: Large dataset
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "large_data.parquet", 50_000);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Query with time range filter
    let start = Instant::now();
    let _count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM silver_indoor_air
             WHERE timestamp >= current_timestamp - INTERVAL '7 days'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let duration = start.elapsed();

    // Assert: Should complete quickly
    println!("Time range filter duration: {:?}", duration);
    assert!(
        duration.as_secs() < 10,
        "Time range filter should complete in < 10s, took {:?}",
        duration
    );
}

// ========== DATA QUALITY INTEGRATION TESTS ==========

#[test]
#[ignore]
fn test_null_handling_integration() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "null_test.parquet", 100);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Query for NULL values
    let null_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM silver_indoor_air WHERE pm25 IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: Should have NULLs from test data
    assert!(
        null_count > 0,
        "View should have NULL pm25 values from test data"
    );
}

#[test]
#[ignore]
fn test_range_validation_integration() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "range_test.parquet", 100);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Query for values outside expected range
    let invalid_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM silver_indoor_air
             WHERE pm25 IS NOT NULL AND (pm25 < 0 OR pm25 > 500)",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: All out-of-range values should be NULL (count = 0)
    assert_eq!(
        invalid_count, 0,
        "View should not contain out-of-range pm25 values"
    );
}

// ========== PARQUET COMPRESSION AND ENCODING TESTS ==========

#[test]
#[ignore]
fn test_parquet_with_compression() {
    // Arrange
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "compressed.parquet", 1000);
    let conn = setup_duckdb();

    // Act: Load compressed Parquet file
    let count: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM read_parquet('{}')",
                parquet_file.to_str().unwrap()
            ),
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: Should read compressed file correctly
    assert_eq!(count, 1000, "Should read all rows from compressed Parquet");
}

// ========== MEMORY AND RESOURCE TESTS ==========

#[test]
#[ignore]
fn test_large_dataset_memory_efficiency() {
    // Arrange: Large dataset to test memory handling
    let temp_dir = setup_test_parquet_dir();
    let parquet_file = create_test_parquet_file(temp_dir.path(), "large.parquet", 100_000);
    let conn = setup_duckdb();
    create_silver_view_from_parquet(&conn, &parquet_file).unwrap();

    // Act: Query large dataset
    let start = Instant::now();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM silver_indoor_air", [], |row| {
            row.get(0)
        })
        .unwrap();
    let duration = start.elapsed();

    // Assert: Should handle large dataset efficiently
    assert_eq!(count, 100_000, "Should count all 100,000 rows");
    println!("Large dataset query took: {:?}", duration);
}

// ========== ERROR HANDLING TESTS ==========

#[test]
#[ignore]
fn test_invalid_parquet_path() {
    // Arrange
    let conn = setup_duckdb();

    // Act: Try to load from non-existent path
    let result = conn.query_row(
        "SELECT COUNT(*) FROM read_parquet('/nonexistent/path/*.parquet')",
        [],
        |row| row.get::<_, i64>(0),
    );

    // Assert: Should error gracefully
    assert!(
        result.is_err(),
        "Should error for non-existent Parquet path"
    );
}

#[test]
#[ignore]
fn test_corrupted_parquet_handling() {
    // Arrange: Create a corrupted file
    let temp_dir = setup_test_parquet_dir();
    let corrupt_file = temp_dir.path().join("corrupt.parquet");
    std::fs::write(&corrupt_file, b"not a parquet file").unwrap();
    let conn = setup_duckdb();

    // Act: Try to load corrupted file
    let result = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM read_parquet('{}')",
            corrupt_file.to_str().unwrap()
        ),
        [],
        |row| row.get::<_, i64>(0),
    );

    // Assert: Should error for corrupted file
    assert!(result.is_err(), "Should error for corrupted Parquet file");
}
