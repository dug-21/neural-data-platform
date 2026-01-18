//! Pre-Transform Parser Integration Module
//!
//! Integrates ColumnOrientedParser from neural-core into silver-etl to enable
//! config-driven pre-transformation of columnar array data (e.g., NWS gridpoints forecasts).
//!
//! # Overview
//!
//! The pre-transform stage flattens columnar array JSON structures into individual rows
//! before DuckDB SQL processing. This enables complex nested data to flow through the
//! standard config-driven Silver ETL pipeline.
//!
//! # Performance
//!
//! Uses batch inserts (default 1000 rows per INSERT) for ~50-100x speedup over row-by-row
//! insertion. Processing 2.8M rows takes ~30-60 seconds instead of ~45 minutes.
//!
//! # Data Flow
//!
//! ```text
//! Bronze Parquet (raw JSON)
//!        |
//!        v
//! [Pre-Transform Stage]  <-- ColumnOrientedParser called here
//!        |
//!        v
//! Flattened temp table (one row per metric per validTime)
//!        |
//!        v
//! DuckDB SQL (pivot + field extraction)
//!        |
//!        v
//! Silver TimescaleDB
//! ```
//!
//! # References
//!
//! - ADR-001: `product/features/dp-007/architecture/ADR-001-PRE-TRANSFORM-DESIGN.md`
//! - ColumnOrientedParser: `core/src/parsers/column_oriented.rs`

use chrono::{DateTime, Utc};
use duckdb::Connection;
use neural_core::parsers::{ColumnOrientedConfig, ColumnOrientedParser, ParserConfig};
use neural_core::Parser;
use serde_json::Value;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Batch size for multi-row INSERT statements.
/// 1000 rows per INSERT provides ~50-100x speedup over row-by-row insertion.
const BATCH_SIZE: usize = 1000;

// =============================================================================
// Error Types
// =============================================================================

/// Pre-transform execution errors
#[derive(Debug, Error)]
pub enum PreTransformError {
    /// Parser creation or execution error
    #[error("Parser error: {0}")]
    Parser(String),

    /// DuckDB database error
    #[error("Database error: {0}")]
    Database(#[from] duckdb::Error),

    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

// =============================================================================
// Result Types
// =============================================================================

/// Result of pre-transform operation
#[derive(Debug)]
pub struct PreTransformResult {
    /// Name of the temp table containing flattened data
    pub table_name: String,
    /// Number of rows inserted into temp table
    pub row_count: usize,
}

/// Row data for batch insertion
#[derive(Clone)]
struct BatchRow {
    issue_time: String,
    valid_time: Option<String>,
    ndp_id: String,
    location_id: String,
    metric_name: String,
    value: f64,
}

/// Flush a batch of rows to the temp table using multi-row INSERT
///
/// Builds a single INSERT statement with multiple value tuples for efficiency.
/// DuckDB handles this much faster than individual INSERT statements.
fn flush_batch(conn: &Connection, batch: &[BatchRow]) -> Result<usize, PreTransformError> {
    if batch.is_empty() {
        return Ok(0);
    }

    // Build multi-row INSERT: INSERT INTO t VALUES (?,?,?,?,?,?), (?,?,?,?,?,?), ...
    let placeholders: Vec<String> = batch
        .iter()
        .map(|_| "(?, ?, ?, ?, ?, ?)".to_string())
        .collect();
    let sql = format!(
        "INSERT INTO pre_transformed (issue_time, valid_time, ndp_id, location_id, metric_name, value) VALUES {}",
        placeholders.join(", ")
    );

    // Flatten all row values into a single params vector
    let mut params_vec: Vec<Box<dyn duckdb::ToSql>> = Vec::with_capacity(batch.len() * 6);
    for row in batch {
        params_vec.push(Box::new(row.issue_time.clone()));
        params_vec.push(Box::new(row.valid_time.clone()));
        params_vec.push(Box::new(row.ndp_id.clone()));
        params_vec.push(Box::new(row.location_id.clone()));
        params_vec.push(Box::new(row.metric_name.clone()));
        params_vec.push(Box::new(row.value));
    }

    // Convert to slice of references for duckdb
    let params_refs: Vec<&dyn duckdb::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())?;

    Ok(batch.len())
}

// =============================================================================
// Temp Table Management
// =============================================================================

/// Create the pre_transformed temp table schema
///
/// The temp table has this schema:
/// - issue_time: TIMESTAMPTZ - when the forecast was issued (Bronze timestamp)
/// - valid_time: TIMESTAMPTZ - when the forecast applies (from validTime in array)
/// - ndp_id: VARCHAR - stable identifier for deduplication
/// - location_id: VARCHAR - grid ID or station ID
/// - metric_name: VARCHAR - metric identifier (e.g., "temperature")
/// - value: DOUBLE - the numeric value
///
/// # Errors
///
/// Returns error if table creation fails.
pub fn create_temp_table(conn: &Connection) -> Result<(), PreTransformError> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS pre_transformed;
        CREATE TEMP TABLE pre_transformed (
            issue_time TIMESTAMPTZ,
            valid_time TIMESTAMPTZ,
            ndp_id VARCHAR,
            location_id VARCHAR,
            metric_name VARCHAR,
            value DOUBLE
        );
    "#,
    )?;
    Ok(())
}

// =============================================================================
// Pre-Transform Logic
// =============================================================================

/// Apply pre-transform to Bronze Parquet data
///
/// Reads raw_payload JSON from Bronze rows, parses through ColumnOrientedParser,
/// and inserts flattened rows into the pre_transformed temp table.
///
/// # Arguments
///
/// * `conn` - DuckDB connection with temp table access
/// * `parser` - Configured ColumnOrientedParser instance
/// * `raw_payloads` - JSON payloads from Bronze raw_payload column
/// * `timestamps` - Bronze timestamps in microseconds
/// * `ndp_ids` - NDP identifiers for each row
///
/// # Returns
///
/// PreTransformResult with table name and row count on success.
///
/// # Errors
///
/// Returns error if temp table creation fails or parser encounters fatal error.
/// Individual row/value errors are logged and skipped (graceful degradation).
pub fn apply_pre_transform(
    conn: &Connection,
    parser: &ColumnOrientedParser,
    raw_payloads: &[Value],
    timestamps: &[i64],
    ndp_ids: &[Option<String>],
) -> Result<PreTransformResult, PreTransformError> {
    create_temp_table(conn)?;

    let mut total_rows = 0;
    let mut rows_failed = 0;
    let mut batch: Vec<BatchRow> = Vec::with_capacity(BATCH_SIZE);
    let mut batches_flushed = 0;

    for (i, payload) in raw_payloads.iter().enumerate() {
        // Convert microseconds timestamp to DateTime
        let issue_time = DateTime::from_timestamp_micros(timestamps[i]).unwrap_or_else(Utc::now);

        let ndp_id = ndp_ids
            .get(i)
            .and_then(|o| o.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("");

        // Parse through ColumnOrientedParser
        let points = match parser.parse(payload, issue_time) {
            Ok(pts) => pts,
            Err(e) => {
                warn!(
                    ndp_id = %ndp_id,
                    error = %e,
                    "Failed to parse payload, skipping row"
                );
                rows_failed += 1;
                continue;
            }
        };

        for point in points {
            // Extract valid_time from tags (forecast_valid_time is Unix timestamp string)
            let valid_time: Option<DateTime<Utc>> = point
                .tags
                .get("forecast_valid_time")
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(|ts| DateTime::from_timestamp(ts, 0));

            // Extract metric name from tags
            let metric_name = point
                .tags
                .get("metric")
                .map(|s| s.as_str())
                .unwrap_or("unknown");

            // Add to batch instead of immediate insert
            batch.push(BatchRow {
                issue_time: issue_time.to_rfc3339(),
                valid_time: valid_time.map(|t| t.to_rfc3339()),
                ndp_id: ndp_id.to_string(),
                location_id: point.location_id.clone(),
                metric_name: metric_name.to_string(),
                value: point.value,
            });

            // Flush batch when full
            if batch.len() >= BATCH_SIZE {
                let flushed = flush_batch(conn, &batch)?;
                total_rows += flushed;
                batches_flushed += 1;
                batch.clear();

                // Log progress every 100 batches (~100k rows)
                if batches_flushed % 100 == 0 {
                    debug!(
                        batches_flushed = batches_flushed,
                        total_rows = total_rows,
                        "Pre-transform batch progress"
                    );
                }
            }
        }
    }

    // Flush remaining rows
    if !batch.is_empty() {
        let flushed = flush_batch(conn, &batch)?;
        total_rows += flushed;
        batches_flushed += 1;
    }

    if rows_failed > 0 {
        let failure_rate = rows_failed as f64 / raw_payloads.len() as f64;
        if failure_rate > 0.1 {
            warn!(
                rows_failed = rows_failed,
                total_rows = raw_payloads.len(),
                failure_rate = format!("{:.1}%", failure_rate * 100.0),
                "High pre-transform failure rate"
            );
        }
    }

    info!(
        rows_input = raw_payloads.len(),
        rows_output = total_rows,
        rows_failed = rows_failed,
        batches = batches_flushed,
        batch_size = BATCH_SIZE,
        "Pre-transform completed"
    );

    Ok(PreTransformResult {
        table_name: "pre_transformed".to_string(),
        row_count: total_rows,
    })
}

// =============================================================================
// Parser Factory
// =============================================================================

/// Build a ColumnOrientedParser from configuration
///
/// Creates a parser instance from ParserConfig with column_config.
/// The column_config must be set in the ParserConfig for this to succeed.
///
/// # Arguments
///
/// * `parser_config` - Base parser configuration
/// * `column_config` - Column-oriented specific configuration
///
/// # Returns
///
/// Configured ColumnOrientedParser on success.
///
/// # Errors
///
/// Returns error if parser cannot be created from config.
pub fn build_parser(
    parser_config: &ParserConfig,
    column_config: &ColumnOrientedConfig,
) -> Result<ColumnOrientedParser, PreTransformError> {
    // Clone config and set column_config
    let mut config = parser_config.clone();
    config.column_config = Some(column_config.clone());

    ColumnOrientedParser::from_config(config).map_err(|e| PreTransformError::Parser(e.to_string()))
}

/// Build a ColumnOrientedParser from PreTransformConfig
///
/// Converts the enum-based PreTransformConfig into a working parser instance.
/// This bridges the gap between SilverEtlConfig's PreTransformConfig and the
/// neural-core ColumnOrientedParser.
///
/// # Arguments
///
/// * `config` - PreTransformConfig from silver_etl config
///
/// # Returns
///
/// Configured ColumnOrientedParser on success.
///
/// # Errors
///
/// Returns error if parser cannot be created from config.
pub fn build_parser_from_config(
    config: &neural_core::config::PreTransformConfig,
) -> Result<ColumnOrientedParser, PreTransformError> {
    use neural_core::config::PreTransformType;
    use neural_core::parsers::{ColumnMapping, ParserType, TimestampFormat};
    use std::collections::HashMap;

    match &config.transform_type {
        PreTransformType::ArrayExplosion(explosion) => {
            // Convert MetricExplosionMapping to ColumnMapping
            let columns: Vec<ColumnMapping> = explosion
                .metrics
                .iter()
                .map(|m| ColumnMapping {
                    metric_path: m.metric_path.clone(),
                    field_name: m.target_column.clone(),
                    values_path: Some(explosion.values_path.clone()),
                    timestamp_path: Some(explosion.timestamp_field.clone()),
                    value_path: Some(explosion.value_field.clone()),
                })
                .collect();

            let column_config = ColumnOrientedConfig {
                metrics_base_path: explosion.metrics_base_path.clone(),
                columns,
                timestamp_format: TimestampFormat::Iso8601Duration,
                unit_conversions: HashMap::new(),
            };

            let base_config = ParserConfig {
                parser_type: ParserType::ColumnOriented,
                location_id_field: "location".to_string(),
                default_location_id: Some("unknown".to_string()),
                skip_fields: vec![],
                field_mappings: None,
                default_tags: HashMap::new(),
                array_config: None,
                column_config: Some(column_config),
            };

            ColumnOrientedParser::from_config(base_config)
                .map_err(|e| PreTransformError::Parser(e.to_string()))
        }
    }
}

/// Get row count from pre_transformed table
///
/// Utility function to verify pre-transform results.
pub fn get_pre_transformed_count(conn: &Connection) -> Result<usize, PreTransformError> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM pre_transformed", [], |row| row.get(0))?;
    Ok(count as usize)
}

/// Query pre_transformed table for verification
///
/// Returns all rows from the temp table for testing/debugging.
/// Timestamps are returned as strings via CAST for cross-platform compatibility.
pub fn query_pre_transformed(
    conn: &Connection,
) -> Result<Vec<PreTransformedRow>, PreTransformError> {
    let mut stmt = conn.prepare(
        "SELECT CAST(issue_time AS VARCHAR) as issue_time, CAST(valid_time AS VARCHAR) as valid_time, ndp_id, location_id, metric_name, value FROM pre_transformed ORDER BY issue_time, valid_time, metric_name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(PreTransformedRow {
            issue_time: row.get(0)?,
            valid_time: row.get(1)?,
            ndp_id: row.get(2)?,
            location_id: row.get(3)?,
            metric_name: row.get(4)?,
            value: row.get(5)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// Row from pre_transformed temp table
#[derive(Debug, Clone)]
pub struct PreTransformedRow {
    pub issue_time: Option<String>,
    pub valid_time: Option<String>,
    pub ndp_id: Option<String>,
    pub location_id: Option<String>,
    pub metric_name: Option<String>,
    pub value: Option<f64>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::params;
    use neural_core::parsers::{ColumnMapping, ParserType, TimestampFormat};
    use neural_core::Parser as _NeuralParser; // Import for .name() method
    use serde_json::json;
    use std::collections::HashMap;

    /// Create a test parser with given column mappings
    fn create_test_parser(
        metrics_base_path: &str,
        columns: Vec<ColumnMapping>,
    ) -> ColumnOrientedParser {
        let column_config = ColumnOrientedConfig {
            metrics_base_path: metrics_base_path.to_string(),
            columns,
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions: HashMap::new(),
        };

        let base_config = ParserConfig {
            parser_type: ParserType::ColumnOriented,
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: None,
            column_config: Some(column_config),
        };

        ColumnOrientedParser::from_config(base_config).unwrap()
    }

    #[test]
    fn test_create_temp_table() {
        let conn = Connection::open_in_memory().unwrap();
        create_temp_table(&conn).unwrap();

        // Verify table exists by querying it
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM pre_transformed", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_create_temp_table_has_correct_columns() {
        let conn = Connection::open_in_memory().unwrap();
        create_temp_table(&conn).unwrap();

        // Try inserting a row to verify schema
        conn.execute(
            "INSERT INTO pre_transformed VALUES (?, ?, ?, ?, ?, ?)",
            params![
                "2025-12-24T00:00:00Z",
                "2025-12-24T06:00:00Z",
                "test-ndp-id",
                "location-001",
                "temperature",
                15.5,
            ],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM pre_transformed", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_apply_pre_transform_flattens_array() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![
            ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temperature".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
            ColumnMapping {
                metric_path: "humidity".to_string(),
                field_name: "humidity".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
        ];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5},
                        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 14.8}
                    ]
                },
                "humidity": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 68.0}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64]; // 2023-12-24T00:00:00Z in microseconds
        let ndp_ids = vec![Some("nws-001".to_string())];

        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);

        assert!(result.is_ok());
        let result = result.unwrap();

        // 2 temperature values + 1 humidity value = 3 rows
        assert_eq!(result.row_count, 3);
        assert_eq!(result.table_name, "pre_transformed");

        // Verify data in table
        let count = get_pre_transformed_count(&conn).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_valid_time_extracted_from_tags() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T12:00:00+00:00/PT1H", "value": 20.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("nws-001".to_string())];

        apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids).unwrap();

        // Query the data and verify valid_time
        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1);

        // valid_time should be set from the forecast_valid_time tag
        assert!(rows[0].valid_time.is_some());
        let valid_time = rows[0].valid_time.as_ref().unwrap();
        assert!(valid_time.contains("2025-12-24"));
    }

    #[test]
    fn test_metric_name_extracted_correctly() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![
            ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temp_c".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
            ColumnMapping {
                metric_path: "windSpeed".to_string(),
                field_name: "wind_speed".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
        ];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                },
                "windSpeed": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 10.0}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("nws-001".to_string())];

        apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids).unwrap();

        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 2);

        // Verify metric names match field_name from column mapping
        let metric_names: Vec<_> = rows
            .iter()
            .map(|r| r.metric_name.as_ref().unwrap().as_str())
            .collect();
        assert!(metric_names.contains(&"temp_c"));
        assert!(metric_names.contains(&"wind_speed"));
    }

    #[test]
    fn test_ndp_id_preserved() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("weather-nws-gridpoint-001".to_string())];

        apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids).unwrap();

        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].ndp_id.as_ref().unwrap(),
            "weather-nws-gridpoint-001"
        );
    }

    #[test]
    fn test_location_id_preserved() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "MTR-50-75",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("nws-001".to_string())];

        apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids).unwrap();

        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].location_id.as_ref().unwrap(), "MTR-50-75");
    }

    #[test]
    fn test_graceful_handling_missing_metric() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![
            ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temperature".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
            ColumnMapping {
                metric_path: "nonexistent".to_string(),
                field_name: "missing".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
        ];

        let parser = create_test_parser("properties", columns);

        // Payload only has temperature, not "nonexistent"
        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("nws-001".to_string())];

        // Should not fail - gracefully skip missing metric
        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);
        assert!(result.is_ok());

        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1); // Only temperature row
    }

    #[test]
    fn test_multiple_payloads() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload1 = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let payload2 = json!({
            "location": "station-002",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 18.0},
                        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 17.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload1, payload2];
        let timestamps = vec![1703376000000000_i64, 1703379600000000_i64];
        let ndp_ids = vec![Some("nws-001".to_string()), Some("nws-002".to_string())];

        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);
        assert!(result.is_ok());

        let result = result.unwrap();
        // 1 from payload1 + 2 from payload2 = 3 rows
        assert_eq!(result.row_count, 3);
    }

    #[test]
    fn test_build_parser() {
        let column_config = ColumnOrientedConfig {
            metrics_base_path: "properties".to_string(),
            columns: vec![ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temperature".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            }],
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions: HashMap::new(),
        };

        let parser_config = ParserConfig {
            parser_type: ParserType::ColumnOriented,
            location_id_field: "location".to_string(),
            default_location_id: Some("default".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: None,
            column_config: None, // Will be set by build_parser
        };

        let result = build_parser(&parser_config, &column_config);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.name(), "column_oriented");
    }

    #[test]
    fn test_empty_payloads() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let raw_payloads: Vec<Value> = vec![];
        let timestamps: Vec<i64> = vec![];
        let ndp_ids: Vec<Option<String>> = vec![];

        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.row_count, 0);
    }

    #[test]
    fn test_value_stored_correctly() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("nws-001".to_string())];

        apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids).unwrap();

        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert!((rows[0].value.unwrap() - 15.5).abs() < 0.001);
    }

    #[test]
    fn test_none_ndp_id_handled() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station-001",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![None]; // No ndp_id

        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);
        assert!(result.is_ok());

        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        // ndp_id should be empty string
        assert_eq!(rows[0].ndp_id.as_ref().unwrap(), "");
    }

    // ============================================================
    // Test: build_parser_from_config creates correct parser (London TDD)
    // ============================================================
    #[test]
    fn test_build_parser_from_config_array_explosion() {
        use neural_core::config::{
            ArrayExplosionConfig, MetricExplosionMapping, PreTransformConfig, PreTransformType,
        };

        let config = PreTransformConfig {
            transform_type: PreTransformType::ArrayExplosion(ArrayExplosionConfig {
                metrics_base_path: "properties".to_string(),
                timestamp_field: "validTime".to_string(),
                value_field: "value".to_string(),
                values_path: "values".to_string(),
                metrics: vec![
                    MetricExplosionMapping {
                        metric_path: "temperature".to_string(),
                        target_column: "temp_c".to_string(),
                        column_type: "double_precision".to_string(),
                    },
                    MetricExplosionMapping {
                        metric_path: "windSpeed".to_string(),
                        target_column: "wind_speed_ms".to_string(),
                        column_type: "double_precision".to_string(),
                    },
                ],
            }),
        };

        let parser = build_parser_from_config(&config);
        assert!(parser.is_ok(), "Should create parser from config");

        let parser = parser.unwrap();
        assert_eq!(parser.name(), "column_oriented");
    }

    #[test]
    fn test_build_parser_from_config_with_defaults() {
        use neural_core::config::{
            ArrayExplosionConfig, MetricExplosionMapping, PreTransformConfig, PreTransformType,
        };

        let config = PreTransformConfig {
            transform_type: PreTransformType::ArrayExplosion(ArrayExplosionConfig {
                metrics_base_path: "data".to_string(),
                timestamp_field: "validTime".to_string(),
                value_field: "value".to_string(),
                values_path: "values".to_string(),
                metrics: vec![MetricExplosionMapping {
                    metric_path: "metric1".to_string(),
                    target_column: "col1".to_string(),
                    column_type: "double_precision".to_string(),
                }],
            }),
        };

        let parser = build_parser_from_config(&config);
        assert!(parser.is_ok());
    }

    /// Test that batch inserts work correctly across multiple batch boundaries.
    /// This creates >BATCH_SIZE (1000) rows to verify multi-batch insertion.
    #[test]
    fn test_batch_insert_across_multiple_batches() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        // Create a payload with many values to exceed BATCH_SIZE
        // Each validTime entry generates 1 row, so 1500 values = 1500 rows
        let values: Vec<serde_json::Value> = (0..1500)
            .map(|i| {
                json!({
                    "validTime": format!("2025-12-24T{:02}:{:02}:00+00:00/PT1H", i / 60 % 24, i % 60),
                    "value": i as f64 * 0.1
                })
            })
            .collect();

        let payload = json!({
            "location": "batch-test-station",
            "properties": {
                "temperature": {
                    "values": values
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("batch-test-001".to_string())];

        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);
        assert!(result.is_ok(), "Batch insert should succeed");

        let result = result.unwrap();
        // Should have exactly 1500 rows (spanning 2 batches: 1000 + 500)
        assert_eq!(
            result.row_count, 1500,
            "Should insert all 1500 rows across batches"
        );

        // Verify actual data in table
        let count = get_pre_transformed_count(&conn).unwrap();
        assert_eq!(count, 1500, "Table should contain all 1500 rows");

        // Verify data integrity: all rows should have correct ndp_id and location_id
        let rows = query_pre_transformed(&conn).unwrap();
        assert_eq!(rows.len(), 1500);

        // Check all rows have correct metadata
        for row in &rows {
            assert_eq!(row.ndp_id.as_ref().unwrap(), "batch-test-001");
            assert_eq!(row.location_id.as_ref().unwrap(), "batch-test-station");
            assert_eq!(row.metric_name.as_ref().unwrap(), "temperature");
            assert!(row.value.is_some(), "All rows should have values");
        }

        // Verify value range: should have values from 0.0 to 149.9
        let values: Vec<f64> = rows.iter().filter_map(|r| r.value).collect();
        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!((min_val - 0.0).abs() < 0.001, "Min value should be 0.0");
        assert!((max_val - 149.9).abs() < 0.001, "Max value should be 149.9");
    }

    /// Test batch insert with exact BATCH_SIZE (no remainder)
    #[test]
    fn test_batch_insert_exact_batch_size() {
        let conn = Connection::open_in_memory().unwrap();

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        // Create exactly BATCH_SIZE (1000) values
        let values: Vec<serde_json::Value> = (0..super::BATCH_SIZE)
            .map(|i| {
                json!({
                    "validTime": format!("2025-12-24T{:02}:{:02}:00+00:00/PT1H", i / 60 % 24, i % 60),
                    "value": i as f64
                })
            })
            .collect();

        let payload = json!({
            "location": "exact-batch-station",
            "properties": {
                "temperature": {
                    "values": values
                }
            }
        });

        let raw_payloads = vec![payload];
        let timestamps = vec![1703376000000000_i64];
        let ndp_ids = vec![Some("exact-batch-001".to_string())];

        let result = apply_pre_transform(&conn, &parser, &raw_payloads, &timestamps, &ndp_ids);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(
            result.row_count, 1000,
            "Should insert exactly BATCH_SIZE rows"
        );

        let count = get_pre_transformed_count(&conn).unwrap();
        assert_eq!(count, 1000);
    }
}
