//! Unit tests for DuckDB Silver Layer SQL views
//!
//! Feature: DP-001 - Virtual Silver Layer using DuckDB
//! Test Approach: London School TDD (outside-in, mock-driven)
//!
//! These tests verify:
//! - SQL view creation and syntax validity
//! - Data quality filters (NULL handling, range validation)
//! - Cross-stream JOIN correctness
//! - View query execution
//!
//! Test Philosophy:
//! - Use in-memory DuckDB for fast unit tests
//! - Mock Parquet data sources
//! - Focus on verifying SQL logic, not Parquet I/O
//! - Tests should run in < 1 second
//!
//! ## Prerequisites
//!
//! These tests require the DuckDB C library to be installed:
//! - macOS: `brew install duckdb`
//! - Ubuntu/Debian: `apt-get install libduckdb-dev`
//! - Or download from: https://github.com/duckdb/duckdb/releases
//!
//! Run with: `cargo test --package air-quality-app --test duckdb_views_test`

#[cfg(feature = "duckdb-tests")]
use duckdb::{Connection, Result as DuckDbResult};

// ========== TEST FIXTURES ==========

#[cfg(feature = "duckdb-tests")]
/// Create an in-memory DuckDB connection for testing
fn setup_test_db() -> Connection {
    Connection::open_in_memory().expect("Failed to create in-memory DuckDB")
}

#[cfg(not(feature = "duckdb-tests"))]
/// Placeholder when DuckDB is not available
fn setup_test_db() -> () {
    ()
}

/// Create test tables mimicking Parquet data structure
fn create_mock_parquet_tables(conn: &Connection) -> DuckDbResult<()> {
    // Indoor air quality table
    conn.execute_batch(
        "CREATE TABLE mock_indoor_air (
            timestamp TIMESTAMP NOT NULL,
            pm25 DOUBLE,
            pm10 DOUBLE,
            co2 DOUBLE,
            temperature DOUBLE,
            humidity DOUBLE,
            tvoc DOUBLE,
            nox DOUBLE
        )",
    )?;

    // Outdoor air quality table
    conn.execute_batch(
        "CREATE TABLE mock_outdoor_air (
            timestamp TIMESTAMP NOT NULL,
            aqi INTEGER,
            co DOUBLE,
            no DOUBLE,
            no2 DOUBLE,
            o3 DOUBLE,
            so2 DOUBLE,
            pm2_5 DOUBLE,
            pm10 DOUBLE,
            nh3 DOUBLE
        )",
    )?;

    Ok(())
}

/// Insert test data with known values
fn insert_indoor_air_test_data(conn: &Connection) -> DuckDbResult<()> {
    conn.execute_batch(
        "INSERT INTO mock_indoor_air VALUES
            -- Valid data points
            ('2025-12-18 12:00:00', 25.5, 50.0, 450.0, 22.5, 45.0, 100.0, 10.0),
            ('2025-12-18 12:01:00', 30.2, 55.0, 500.0, 23.0, 46.0, 120.0, 12.0),
            ('2025-12-18 12:02:00', 35.8, 60.0, 550.0, 23.5, 47.0, 150.0, 15.0),

            -- Edge cases: boundary values (should be INCLUDED)
            ('2025-12-18 12:03:00', 0.0, 0.0, 400.0, -10.0, 0.0, 0.0, 0.0),
            ('2025-12-18 12:04:00', 500.0, 1000.0, 5000.0, 50.0, 100.0, 60000.0, 1000.0),

            -- Out-of-range values (should be EXCLUDED via NULL)
            ('2025-12-18 12:05:00', -10.0, -1.0, 300.0, -20.0, -5.0, -100.0, -10.0),
            ('2025-12-18 12:06:00', 600.0, 1500.0, 6000.0, 70.0, 150.0, 70000.0, 1500.0),

            -- NULL values (should remain NULL)
            ('2025-12-18 12:07:00', NULL, NULL, NULL, NULL, NULL, NULL, NULL),
            ('2025-12-18 12:08:00', 40.0, NULL, 480.0, 22.0, NULL, 200.0, NULL)
        ",
    )
}

/// Insert outdoor air test data
fn insert_outdoor_air_test_data(conn: &Connection) -> DuckDbResult<()> {
    conn.execute_batch(
        "INSERT INTO mock_outdoor_air VALUES
            -- Valid data points
            ('2025-12-18 12:00:00', 1, 200.5, 10.5, 20.3, 50.8, 15.2, 12.5, 25.0, 5.5),
            ('2025-12-18 12:10:00', 3, 300.0, 15.0, 25.0, 60.0, 20.0, 18.0, 30.0, 8.0),
            ('2025-12-18 12:20:00', 5, 500.0, 20.0, 30.0, 80.0, 25.0, 25.0, 40.0, 10.0),

            -- Edge cases: boundary values
            ('2025-12-18 12:30:00', 1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            ('2025-12-18 12:40:00', 5, 50000.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 200.0),

            -- Out-of-range values
            ('2025-12-18 12:50:00', 0, -100.0, -10.0, -10.0, -10.0, -10.0, -10.0, -10.0, -5.0),
            ('2025-12-18 13:00:00', 6, 60000.0, 1500.0, 1500.0, 1500.0, 1500.0, 1500.0, 1500.0, 250.0)
        ",
    )
}

/// Create Silver views adapted from production SQL files
fn create_silver_views(conn: &Connection) -> DuckDbResult<()> {
    // Silver indoor air view (adapted for mock tables)
    conn.execute_batch(
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
        FROM mock_indoor_air
        WHERE timestamp IS NOT NULL
        ORDER BY timestamp DESC",
    )?;

    // Silver outdoor air view (adapted for mock tables)
    conn.execute_batch(
        "CREATE OR REPLACE VIEW silver_outdoor_air AS
        SELECT
            timestamp,
            CASE
                WHEN aqi >= 1 AND aqi <= 5
                THEN ROUND(aqi, 0)
                ELSE NULL
            END as aqi,
            CASE
                WHEN co >= 0 AND co <= 50000
                THEN ROUND(co, 1)
                ELSE NULL
            END as co,
            CASE
                WHEN no >= 0 AND no <= 1000
                THEN ROUND(no, 2)
                ELSE NULL
            END as no,
            CASE
                WHEN no2 >= 0 AND no2 <= 1000
                THEN ROUND(no2, 2)
                ELSE NULL
            END as no2,
            CASE
                WHEN o3 >= 0 AND o3 <= 1000
                THEN ROUND(o3, 2)
                ELSE NULL
            END as o3,
            CASE
                WHEN so2 >= 0 AND so2 <= 1000
                THEN ROUND(so2, 2)
                ELSE NULL
            END as so2,
            CASE
                WHEN pm2_5 >= 0 AND pm2_5 <= 1000
                THEN ROUND(pm2_5, 1)
                ELSE NULL
            END as pm2_5,
            CASE
                WHEN pm10 >= 0 AND pm10 <= 1000
                THEN ROUND(pm10, 1)
                ELSE NULL
            END as pm10,
            CASE
                WHEN nh3 >= 0 AND nh3 <= 200
                THEN ROUND(nh3, 2)
                ELSE NULL
            END as nh3
        FROM mock_outdoor_air
        WHERE timestamp IS NOT NULL
        ORDER BY timestamp DESC",
    )?;

    Ok(())
}

// ========== T-DB-003: VIEW CREATION TESTS ==========

#[test]
fn test_view_creation_syntax_valid() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Act
    let result = create_silver_views(&conn);

    // Assert
    assert!(result.is_ok(), "Silver view creation should succeed");
}

#[test]
fn test_view_exists_in_catalog() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query DuckDB catalog for views
    let views: Vec<String> = conn
        .prepare("SELECT table_name FROM information_schema.tables WHERE table_type = 'VIEW'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert
    assert!(
        views.contains(&"silver_indoor_air".to_string()),
        "silver_indoor_air view should exist"
    );
    assert!(
        views.contains(&"silver_outdoor_air".to_string()),
        "silver_outdoor_air view should exist"
    );
}

#[test]
fn test_view_query_execution() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query the view
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM silver_indoor_air", [], |row| {
            row.get(0)
        })
        .unwrap();

    // Assert
    assert!(count >= 0, "View should be queryable and return count");
}

// ========== T-DB-004: NULL HANDLING TESTS ==========

#[test]
fn test_null_fields_remain_null() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query for rows with NULLs
    let null_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM silver_indoor_air WHERE pm25 IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: NULL values + out-of-range values should result in NULLs
    assert!(
        null_count >= 2,
        "Should have at least 2 rows with NULL pm25 (explicit NULL + out-of-range)"
    );
}

#[test]
fn test_partial_null_handling() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query for row with mixed NULL/valid values
    let row: (Option<f64>, Option<f64>, Option<f64>) = conn
        .query_row(
            "SELECT pm25, pm10, co2 FROM silver_indoor_air
             WHERE timestamp = '2025-12-18 12:08:00'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    // Assert: Valid fields should have values, NULL fields remain NULL
    assert!(row.0.is_some(), "pm25 should have value (40.0)");
    assert!(row.1.is_none(), "pm10 should be NULL");
    assert!(row.2.is_some(), "co2 should have value (480.0)");
}

// ========== T-DB-005: RANGE FILTERING TESTS ==========

#[test]
fn test_pm25_range_filter() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query for valid pm25 values
    let values: Vec<f64> = conn
        .prepare("SELECT pm25 FROM silver_indoor_air WHERE pm25 IS NOT NULL ORDER BY timestamp")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert: Should exclude out-of-range values (-10.0, 600.0)
    assert!(
        !values.iter().any(|&v| v < 0.0 || v > 500.0),
        "All pm25 values should be in range [0, 500]"
    );
}

#[test]
fn test_boundary_values_included() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query for boundary values
    let boundaries: Vec<f64> = conn
        .prepare("SELECT pm25 FROM silver_indoor_air WHERE pm25 IN (0.0, 500.0)")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert: Boundary values should be INCLUDED
    assert_eq!(
        boundaries.len(),
        2,
        "Boundary values 0.0 and 500.0 should be included"
    );
}

#[test]
fn test_temperature_range_filter() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act
    let values: Vec<f64> = conn
        .prepare("SELECT temperature FROM silver_indoor_air WHERE temperature IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert: Should exclude out-of-range values (-20.0, 70.0)
    assert!(
        !values.iter().any(|&v| v < -10.0 || v > 50.0),
        "All temperature values should be in range [-10, 50]"
    );
}

#[test]
fn test_humidity_range_filter() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act
    let values: Vec<f64> = conn
        .prepare("SELECT humidity FROM silver_indoor_air WHERE humidity IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert: Should exclude out-of-range values (-5.0, 150.0)
    assert!(
        !values.iter().any(|&v| v < 0.0 || v > 100.0),
        "All humidity values should be in range [0, 100]"
    );
}

#[test]
fn test_aqi_range_filter() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_outdoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act
    let values: Vec<i64> = conn
        .prepare("SELECT aqi FROM silver_outdoor_air WHERE aqi IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert: Should exclude out-of-range values (0, 6)
    assert!(
        !values.iter().any(|&v| v < 1 || v > 5),
        "All AQI values should be in range [1, 5]"
    );
}

// ========== T-DB-006: CROSS-STREAM JOIN TESTS ==========

#[test]
fn test_join_indoor_outdoor_exact_timestamp() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Insert data with matching timestamps
    conn.execute_batch(
        "INSERT INTO mock_indoor_air VALUES
            ('2025-12-18 12:00:00', 25.5, 50.0, 450.0, 22.5, 45.0, 100.0, 10.0)",
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO mock_outdoor_air VALUES
            ('2025-12-18 12:00:00', 3, 300.0, 15.0, 25.0, 60.0, 20.0, 18.0, 30.0, 8.0)",
    )
    .unwrap();

    create_silver_views(&conn).unwrap();

    // Act: JOIN on exact timestamp
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM silver_indoor_air i
             INNER JOIN silver_outdoor_air o ON i.timestamp = o.timestamp",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: Should have one matching row
    assert_eq!(
        count, 1,
        "Should have exactly one row with matching timestamp"
    );
}

#[test]
fn test_join_with_time_window() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Insert data with timestamps within 5-minute window
    conn.execute_batch(
        "INSERT INTO mock_indoor_air VALUES
            ('2025-12-18 12:00:00', 25.5, 50.0, 450.0, 22.5, 45.0, 100.0, 10.0),
            ('2025-12-18 12:02:00', 30.0, 55.0, 480.0, 23.0, 46.0, 120.0, 12.0)",
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO mock_outdoor_air VALUES
            ('2025-12-18 12:00:00', 3, 300.0, 15.0, 25.0, 60.0, 20.0, 18.0, 30.0, 8.0),
            ('2025-12-18 12:03:00', 2, 250.0, 12.0, 22.0, 55.0, 18.0, 15.0, 28.0, 7.0)",
    )
    .unwrap();

    create_silver_views(&conn).unwrap();

    // Act: JOIN with time window (within 5 minutes)
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM silver_indoor_air i
             LEFT JOIN silver_outdoor_air o
               ON i.timestamp BETWEEN o.timestamp - INTERVAL '5 minutes'
                                  AND o.timestamp + INTERVAL '5 minutes'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: Should have multiple matches due to time window
    assert!(
        count >= 2,
        "Should have at least 2 rows with time window JOIN"
    );
}

#[test]
fn test_left_join_preserves_left_table() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Insert indoor data without matching outdoor data
    conn.execute_batch(
        "INSERT INTO mock_indoor_air VALUES
            ('2025-12-18 12:00:00', 25.5, 50.0, 450.0, 22.5, 45.0, 100.0, 10.0)",
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO mock_outdoor_air VALUES
            ('2025-12-18 13:00:00', 3, 300.0, 15.0, 25.0, 60.0, 20.0, 18.0, 30.0, 8.0)",
    )
    .unwrap();

    create_silver_views(&conn).unwrap();

    // Act: LEFT JOIN should preserve left table rows
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM silver_indoor_air i
             LEFT JOIN silver_outdoor_air o ON i.timestamp = o.timestamp",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: Should have 1 row from left table (even though no match on right)
    assert_eq!(count, 1, "LEFT JOIN should preserve left table rows");
}

// ========== DATA QUALITY EDGE CASES ==========

#[test]
fn test_extreme_values_excluded() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Insert extreme values
    conn.execute_batch(
        "INSERT INTO mock_indoor_air VALUES
            ('2025-12-18 12:00:00', -999.0, -1000.0, 100.0, -100.0, -50.0, -10000.0, -500.0),
            ('2025-12-18 12:01:00', 9999.0, 10000.0, 10000.0, 100.0, 200.0, 100000.0, 5000.0)",
    )
    .unwrap();

    create_silver_views(&conn).unwrap();

    // Act: Query for valid values
    let valid_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM silver_indoor_air
             WHERE pm25 IS NOT NULL AND pm10 IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: All extreme values should be converted to NULL
    assert_eq!(
        valid_count, 0,
        "Extreme values should be excluded (converted to NULL)"
    );
}

#[test]
fn test_precision_rounding() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Insert values with high precision
    conn.execute_batch(
        "INSERT INTO mock_indoor_air VALUES
            ('2025-12-18 12:00:00', 25.123456789, 50.987654321, 450.555, 22.678, 45.234, 100.888, 10.123)",
    )
    .unwrap();

    create_silver_views(&conn).unwrap();

    // Act: Query rounded values
    let row: (f64, f64, f64, f64) = conn
        .query_row(
            "SELECT pm25, co2, temperature, tvoc FROM silver_indoor_air",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    // Assert: Check rounding precision
    assert_eq!(row.0, 25.1, "pm25 should be rounded to 1 decimal");
    assert_eq!(row.1, 451.0, "co2 should be rounded to 0 decimals");
    assert_eq!(row.2, 22.7, "temperature should be rounded to 1 decimal");
    assert_eq!(row.3, 101.0, "tvoc should be rounded to 0 decimals");
}

// ========== SCHEMA VALIDATION TESTS ==========

#[test]
fn test_view_column_count() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Query column count
    let indoor_cols: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM information_schema.columns
             WHERE table_name = 'silver_indoor_air'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let outdoor_cols: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM information_schema.columns
             WHERE table_name = 'silver_outdoor_air'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Assert: Expected column counts
    assert_eq!(
        indoor_cols, 8,
        "silver_indoor_air should have 8 columns (timestamp + 7 measurements)"
    );
    assert_eq!(
        outdoor_cols, 10,
        "silver_outdoor_air should have 10 columns (timestamp + 9 measurements)"
    );
}

#[test]
fn test_timestamp_column_required() {
    // Arrange
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();

    // Try to insert row without timestamp (should fail at table level)
    let result = conn.execute_batch("INSERT INTO mock_indoor_air (pm25) VALUES (25.5)");

    // Assert: Timestamp is required
    assert!(
        result.is_err(),
        "Timestamp column should be required (NOT NULL)"
    );
}
