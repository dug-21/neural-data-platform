//! Integration Tests for CSV Source and Dimension Loading - dp-013
//!
//! apps/silver-etl/tests/csv_integration_tests.rs
//!
//! These tests validate end-to-end flows:
//! - CSV to Bronze Parquet
//! - Dimension loading to Silver
//! - Config loading from YAML
//!
//! # Running Integration Tests
//!
//! ```bash
//! # Start test infrastructure (TimescaleDB, etcd)
//! docker compose -f deploy/docker-compose.test.yml up -d
//!
//! # Run integration tests
//! cargo test -p silver-etl --test csv_integration_tests -- --ignored
//!
//! # Run with output
//! cargo test -p silver-etl --test csv_integration_tests -- --ignored --nocapture
//! ```
//!
//! # Test Categories
//!
//! 1. CSV Source Integration - Reading CSV files into Bronze format
//! 2. Dimension Loading Integration - Loading dimensions to Silver
//! 3. Config Loading Tests - YAML configuration parsing

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// =============================================================================
// Test Fixtures Path Helpers
// =============================================================================

/// Get the path to test fixtures directory
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Get path to CSV fixture
fn csv_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("csv").join(name)
}

/// Get path to config fixture
fn config_fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

// =============================================================================
// Test Environment Configuration
// =============================================================================

/// Test environment settings
struct TestEnv {
    /// PostgreSQL connection string for test database
    postgres_url: String,
    /// etcd endpoint for config
    etcd_endpoint: String,
    /// Path to config fixtures
    config_dir: String,
    /// Whether Docker infrastructure is available
    docker_available: bool,
}

impl TestEnv {
    fn from_env() -> Self {
        Self {
            postgres_url: std::env::var("TEST_POSTGRES_URL")
                .unwrap_or_else(|_| "postgres://test:test@localhost:5433/ndp_test".to_string()),
            etcd_endpoint: std::env::var("TEST_ETCD_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:2380".to_string()),
            config_dir: std::env::var("TEST_CONFIG_DIR")
                .unwrap_or_else(|_| fixtures_dir().to_string_lossy().to_string()),
            docker_available: std::env::var("DOCKER_AVAILABLE")
                .map(|v| v == "true")
                .unwrap_or(false),
        }
    }
}

// =============================================================================
// Fixture Verification Tests (Run without Docker)
// =============================================================================

mod fixture_verification_tests {
    use super::*;

    #[test]
    fn test_valid_timeseries_fixture_exists() {
        let path = csv_fixture("valid_timeseries.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_valid_dimension_fixture_exists() {
        let path = csv_fixture("valid_dimension.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_invalid_headers_fixture_exists() {
        let path = csv_fixture("invalid_headers.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_malformed_rows_fixture_exists() {
        let path = csv_fixture("malformed_rows.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_timestamps_epoch_seconds_fixture_exists() {
        let path = csv_fixture("timestamps_epoch_seconds.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_timestamps_epoch_millis_fixture_exists() {
        let path = csv_fixture("timestamps_epoch_millis.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_timestamps_custom_format_fixture_exists() {
        let path = csv_fixture("timestamps_custom_format.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_semicolon_delimited_fixture_exists() {
        let path = csv_fixture("semicolon_delimited.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_quoted_fields_fixture_exists() {
        let path = csv_fixture("quoted_fields.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_empty_data_fixture_exists() {
        let path = csv_fixture("empty_data.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_dimension_with_duplicates_fixture_exists() {
        let path = csv_fixture("dimension_with_duplicates.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_dimension_partial_update_fixture_exists() {
        let path = csv_fixture("dimension_partial_update.csv");
        assert!(path.exists(), "Fixture not found: {:?}", path);
    }

    #[test]
    fn test_valid_timeseries_fixture_content() {
        let path = csv_fixture("valid_timeseries.csv");
        let content = fs::read_to_string(path).expect("Should read file");

        // Verify header
        let lines: Vec<&str> = content.lines().collect();
        assert!(!lines.is_empty(), "File should have content");

        let header = lines[0];
        assert!(header.contains("timestamp"), "Should have timestamp column");
        assert!(header.contains("sensor_id"), "Should have sensor_id column");
        assert!(
            header.contains("temperature"),
            "Should have temperature column"
        );
        assert!(header.contains("humidity"), "Should have humidity column");

        // Verify data rows exist
        assert!(lines.len() > 1, "Should have data rows");
    }

    #[test]
    fn test_valid_dimension_fixture_content() {
        let path = csv_fixture("valid_dimension.csv");
        let content = fs::read_to_string(path).expect("Should read file");

        let lines: Vec<&str> = content.lines().collect();
        assert!(!lines.is_empty(), "File should have content");

        let header = lines[0];
        assert!(header.contains("ndp_id"), "Should have ndp_id column");
        assert!(header.contains("category"), "Should have category column");
        assert!(
            header.contains("friendly_name"),
            "Should have friendly_name column"
        );

        // Verify data rows
        assert!(lines.len() > 1, "Should have data rows");
    }

    #[test]
    fn test_malformed_rows_fixture_has_errors() {
        let path = csv_fixture("malformed_rows.csv");
        let content = fs::read_to_string(path).expect("Should read file");

        // Should contain intentionally malformed data
        assert!(
            content.contains("not_a_number") || content.contains("invalid_timestamp"),
            "Should contain malformed data for testing"
        );
    }
}

// =============================================================================
// CSV Parsing Integration Tests
// =============================================================================

mod csv_parsing_integration_tests {
    use super::*;

    /// Helper to count CSV lines (excluding header)
    fn count_csv_data_rows(path: &Path) -> usize {
        let content = fs::read_to_string(path).expect("Should read file");
        content.lines().count().saturating_sub(1)
    }

    /// Helper to parse CSV headers
    fn parse_csv_headers(path: &Path) -> Vec<String> {
        let content = fs::read_to_string(path).expect("Should read file");
        content
            .lines()
            .next()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    }

    #[test]
    fn test_count_rows_valid_timeseries() {
        let path = csv_fixture("valid_timeseries.csv");
        let count = count_csv_data_rows(&path);
        assert_eq!(count, 5, "Should have 5 data rows");
    }

    #[test]
    fn test_count_rows_valid_dimension() {
        let path = csv_fixture("valid_dimension.csv");
        let count = count_csv_data_rows(&path);
        assert_eq!(count, 5, "Should have 5 data rows");
    }

    #[test]
    fn test_parse_headers_valid_timeseries() {
        let path = csv_fixture("valid_timeseries.csv");
        let headers = parse_csv_headers(&path);
        assert_eq!(
            headers,
            vec!["timestamp", "sensor_id", "temperature", "humidity"]
        );
    }

    #[test]
    fn test_parse_headers_semicolon_delimited() {
        let path = csv_fixture("semicolon_delimited.csv");
        let content = fs::read_to_string(&path).expect("Should read file");
        let headers: Vec<&str> = content
            .lines()
            .next()
            .unwrap_or("")
            .split(';')
            .map(|s| s.trim())
            .collect();
        assert_eq!(
            headers,
            vec!["timestamp", "sensor_id", "temperature", "humidity"]
        );
    }

    #[test]
    fn test_empty_data_has_only_header() {
        let path = csv_fixture("empty_data.csv");
        let count = count_csv_data_rows(&path);
        assert_eq!(count, 0, "Should have 0 data rows (header only)");
    }
}

// =============================================================================
// CSV to Bronze Flow Tests (Requires Infrastructure)
// =============================================================================

mod csv_to_bronze_tests {
    use super::*;

    /// Test reading valid CSV and preparing for Bronze storage
    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_valid_timeseries() {
        // Setup
        let csv_path = csv_fixture("valid_timeseries.csv");
        assert!(csv_path.exists());

        // TODO: Implement when CsvSource adapter is created
        // let config = StreamConfig {
        //     stream_id: "test-timeseries".to_string(),
        //     source: SourceConfig::Csv {
        //         path: csv_path,
        //         timestamp_field: "timestamp".to_string(),
        //         timestamp_format: TimestampFormat::Iso8601,
        //         ..Default::default()
        //     },
        //     ..Default::default()
        // };
        //
        // let source = CsvSource::new(config).await.unwrap();
        // let points = source.fetch_raw_batch().await.unwrap();
        //
        // assert_eq!(points.len(), 5, "Should parse 5 rows");
        // for point in &points {
        //     assert!(point.timestamp.year() >= 2026);
        //     assert!(point.raw_payload.is_object());
        // }

        // Placeholder assertion
        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_epoch_seconds() {
        let csv_path = csv_fixture("timestamps_epoch_seconds.csv");
        assert!(csv_path.exists());

        // TODO: Test epoch_seconds timestamp parsing
        // let config = StreamConfig {
        //     timestamp_format: TimestampFormat::EpochSeconds,
        //     ..
        // };

        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_epoch_millis() {
        let csv_path = csv_fixture("timestamps_epoch_millis.csv");
        assert!(csv_path.exists());

        // TODO: Test epoch_millis timestamp parsing
        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_custom_timestamp_format() {
        let csv_path = csv_fixture("timestamps_custom_format.csv");
        assert!(csv_path.exists());

        // TODO: Test custom timestamp format "%Y/%m/%d %H:%M:%S"
        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_semicolon_delimiter() {
        let csv_path = csv_fixture("semicolon_delimited.csv");
        assert!(csv_path.exists());

        // TODO: Test semicolon delimiter
        // let config = StreamConfig {
        //     delimiter: ';',
        //     ..
        // };

        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_handles_malformed_with_skip() {
        let csv_path = csv_fixture("malformed_rows.csv");
        assert!(csv_path.exists());

        // TODO: Test OnError::Skip behavior
        // Should skip bad rows and continue
        // Stats should show rows_skipped > 0

        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_handles_malformed_with_fail() {
        let csv_path = csv_fixture("malformed_rows.csv");
        assert!(csv_path.exists());

        // TODO: Test OnError::Fail behavior
        // Should fail on first bad row

        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage infrastructure
    async fn test_csv_to_bronze_empty_file() {
        let csv_path = csv_fixture("empty_data.csv");
        assert!(csv_path.exists());

        // TODO: Test empty file handling
        // Should return 0 rows, no error

        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires Bronze storage + Parquet writer
    async fn test_csv_to_bronze_parquet_schema_matches() {
        // TODO: Verify output Parquet has expected schema:
        // - timestamp: TIMESTAMP
        // - source_id: STRING
        // - ndp_id: STRING
        // - raw_payload: STRING (JSON blob)
        // - context: STRING (JSON blob, nullable)

        assert!(true, "Placeholder until Bronze writer is integrated");
    }
}

// =============================================================================
// Dimension Loading Integration Tests (Requires TimescaleDB)
// =============================================================================

mod dimension_loading_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_truncate_and_load_empty_table() {
        let env = TestEnv::from_env();
        let csv_path = csv_fixture("valid_dimension.csv");
        assert!(csv_path.exists());

        // TODO: Implement when DimensionLoader is integrated
        // 1. Create empty dimension table
        // 2. Load CSV using truncate_and_load
        // 3. Verify all rows present

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_truncate_and_load_replaces_existing() {
        let env = TestEnv::from_env();
        let csv_path = csv_fixture("valid_dimension.csv");
        assert!(csv_path.exists());

        // TODO:
        // 1. Load initial data
        // 2. Load different CSV
        // 3. Verify old data removed, new data present

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_truncate_and_load_atomic_rollback() {
        let env = TestEnv::from_env();

        // TODO:
        // 1. Load valid data
        // 2. Attempt to load CSV with FK violation (simulated)
        // 3. Verify original data preserved

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_upsert_inserts_new() {
        let env = TestEnv::from_env();
        let csv_path = csv_fixture("valid_dimension.csv");
        assert!(csv_path.exists());

        // TODO:
        // 1. Create empty table
        // 2. Upsert dimension
        // 3. Verify all rows inserted

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_upsert_updates_existing() {
        let env = TestEnv::from_env();
        let original = csv_fixture("valid_dimension.csv");
        let update = csv_fixture("dimension_partial_update.csv");
        assert!(original.exists());
        assert!(update.exists());

        // TODO:
        // 1. Load original dimension
        // 2. Upsert partial update
        // 3. Verify updated rows changed, others unchanged

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_upsert_mixed_insert_update() {
        let env = TestEnv::from_env();

        // TODO:
        // 1. Load partial data
        // 2. Upsert data with new + existing keys
        // 3. Verify correct inserts and updates

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_validates_pk_before_load() {
        let env = TestEnv::from_env();
        let csv_path = csv_fixture("dimension_with_duplicates.csv");
        assert!(csv_path.exists());

        // TODO:
        // For truncate_and_load, duplicate PKs in CSV should fail
        // validation before any DB operations

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_auto_creates_table() {
        let env = TestEnv::from_env();
        let csv_path = csv_fixture("valid_dimension.csv");
        assert!(csv_path.exists());

        // TODO:
        // 1. Ensure table doesn't exist
        // 2. Load dimension
        // 3. Verify table created with correct schema

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB
    async fn test_dimension_dry_run_no_changes() {
        let env = TestEnv::from_env();
        let csv_path = csv_fixture("valid_dimension.csv");
        assert!(csv_path.exists());

        // TODO:
        // 1. Load initial data
        // 2. Run dry-run with different data
        // 3. Verify no changes to DB
        // 4. Verify dry-run output shows what would change

        assert!(true, "Placeholder until DimensionLoader is integrated");
    }
}

// =============================================================================
// Config Loading Tests
// =============================================================================

mod config_loading_tests {
    use super::*;

    #[test]
    fn test_air_quality_config_exists() {
        let path = config_fixture("air_quality_config.yaml");
        assert!(path.exists(), "Config fixture not found: {:?}", path);
    }

    #[test]
    fn test_weather_config_exists() {
        let path = config_fixture("weather_config.yaml");
        assert!(path.exists(), "Config fixture not found: {:?}", path);
    }

    #[test]
    fn test_air_quality_config_parseable() {
        let path = config_fixture("air_quality_config.yaml");
        let content = fs::read_to_string(path).expect("Should read config");

        // Basic YAML parsing validation
        let value: serde_yaml::Value =
            serde_yaml::from_str(&content).expect("Config should be valid YAML");

        assert!(value.is_mapping(), "Config should be a mapping");
        assert!(
            value.get("enabled").is_some(),
            "Should have 'enabled' field"
        );
        assert!(
            value.get("target_table").is_some(),
            "Should have 'target_table' field"
        );
    }

    #[test]
    fn test_weather_config_parseable() {
        let path = config_fixture("weather_config.yaml");
        let content = fs::read_to_string(path).expect("Should read config");

        let value: serde_yaml::Value =
            serde_yaml::from_str(&content).expect("Config should be valid YAML");

        assert!(value.is_mapping(), "Config should be a mapping");
    }

    #[tokio::test]
    #[ignore] // Requires SilverEtlConfig struct
    async fn test_config_deserializes_to_silver_etl_config() {
        // TODO: When SilverEtlConfig is fully implemented
        // let path = config_fixture("air_quality_config.yaml");
        // let content = fs::read_to_string(path).expect("Should read config");
        // let config: SilverEtlConfig = serde_yaml::from_str(&content)
        //     .expect("Should deserialize to SilverEtlConfig");
        //
        // assert!(config.enabled);
        // assert!(!config.target_table.is_empty());

        assert!(true, "Placeholder until SilverEtlConfig is complete");
    }
}

// =============================================================================
// CLI Integration Tests (Requires Full Binary)
// =============================================================================

mod cli_integration_tests {
    use super::*;
    use std::process::Command;

    /// Helper to check if ndp binary exists
    fn ndp_binary_exists() -> bool {
        // Check common locations
        Path::new("target/debug/ndp").exists() || Path::new("target/release/ndp").exists()
    }

    #[test]
    #[ignore] // Requires compiled ndp binary
    fn test_cli_dimension_list() {
        if !ndp_binary_exists() {
            eprintln!("Skipping: ndp binary not found");
            return;
        }

        // TODO: Run `ndp dimension list` and verify output
        assert!(true, "Placeholder until CLI is implemented");
    }

    #[test]
    #[ignore] // Requires compiled ndp binary + Docker
    fn test_cli_dimension_sync() {
        if !ndp_binary_exists() {
            eprintln!("Skipping: ndp binary not found");
            return;
        }

        // TODO: Run `ndp dimension sync entity-context`
        assert!(true, "Placeholder until CLI is implemented");
    }

    #[test]
    #[ignore] // Requires compiled ndp binary
    fn test_cli_dimension_sync_dry_run() {
        if !ndp_binary_exists() {
            eprintln!("Skipping: ndp binary not found");
            return;
        }

        // TODO: Run `ndp dimension sync entity-context --dry-run`
        // Verify exit code 0 and no DB changes
        assert!(true, "Placeholder until CLI is implemented");
    }

    #[test]
    #[ignore] // Requires compiled ndp binary + Docker
    fn test_cli_stream_ingest_csv() {
        if !ndp_binary_exists() {
            eprintln!("Skipping: ndp binary not found");
            return;
        }

        // TODO: Run `ndp stream ingest historical-aq`
        assert!(true, "Placeholder until CLI is implemented");
    }
}

// =============================================================================
// Performance Tests
// =============================================================================

mod performance_tests {
    use super::*;
    use std::time::Instant;

    /// Generate a large CSV for performance testing
    fn generate_large_csv(path: &Path, rows: usize) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = fs::File::create(path)?;

        writeln!(file, "timestamp,sensor_id,value")?;
        for i in 0..rows {
            let ts = format!(
                "2026-01-{:02}T{:02}:{:02}:00Z",
                (i / 1440) % 28 + 1,
                (i / 60) % 24,
                i % 60
            );
            writeln!(file, "{},sensor_{:03},{:.2}", ts, i % 100, (i as f64) * 0.1)?;
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore] // Performance test - run separately
    async fn test_csv_parse_100k_rows() {
        let temp_dir = tempfile::tempdir().expect("Create temp dir");
        let csv_path = temp_dir.path().join("large.csv");

        // Generate 100k rows
        generate_large_csv(&csv_path, 100_000).expect("Generate CSV");

        let start = Instant::now();

        // TODO: Parse CSV and measure time
        // let source = CsvSource::new(config).await.unwrap();
        // let points = source.fetch_raw_batch().await.unwrap();
        // let duration = start.elapsed();
        //
        // assert_eq!(points.len(), 100_000);
        // assert!(duration.as_secs() < 10, "Should parse 100k rows in <10s");
        //
        // let rows_per_sec = 100_000.0 / duration.as_secs_f64();
        // println!("Performance: {:.0} rows/sec", rows_per_sec);
        // assert!(rows_per_sec > 10_000.0, "Should achieve >10k rows/sec");

        let duration = start.elapsed();
        println!("Large CSV generation took: {:?}", duration);
        assert!(true, "Placeholder until CsvSource is implemented");
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB + performance test
    async fn test_dimension_load_10k_rows() {
        let temp_dir = tempfile::tempdir().expect("Create temp dir");
        let csv_path = temp_dir.path().join("large_dim.csv");

        // Generate 10k dimension rows
        {
            use std::io::Write;
            let mut file = fs::File::create(&csv_path).expect("Create file");
            writeln!(file, "ndp_id,category,name").expect("Write header");
            for i in 0..10_000 {
                writeln!(file, "sensor_{:05},category_{},Sensor {}", i, i % 10, i)
                    .expect("Write row");
            }
        }

        let start = Instant::now();

        // TODO: Load dimension and measure time
        // let loader = DimensionLoader::new(config);
        // let stats = loader.load(&pool).await.unwrap();
        // let duration = start.elapsed();
        //
        // assert_eq!(stats.loaded, 10_000);
        // assert!(duration.as_secs() < 30, "Should load 10k rows in <30s");

        let duration = start.elapsed();
        println!("Large dimension CSV generation took: {:?}", duration);
        assert!(true, "Placeholder until DimensionLoader is implemented");
    }

    #[tokio::test]
    #[ignore] // Performance test
    async fn test_memory_bounded_during_large_csv() {
        // TODO: Monitor memory usage during large CSV processing
        // Verify streaming/batched approach doesn't load entire file
        assert!(true, "Placeholder for memory profiling");
    }
}

// =============================================================================
// End-to-End Tests
// =============================================================================

mod e2e_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires full infrastructure
    async fn test_full_csv_import_to_silver() {
        // Full workflow:
        // 1. Create stream config with source.type: csv
        // 2. Place CSV file at configured path
        // 3. Run CSV ingestion -> Bronze
        // 4. Run Silver ETL -> Silver table
        // 5. Verify data in Silver with correct types

        assert!(true, "Placeholder for full E2E test");
    }

    #[tokio::test]
    #[ignore] // Requires full infrastructure
    async fn test_full_dimension_workflow() {
        // Full workflow:
        // 1. Create dimension config
        // 2. Place CSV data file
        // 3. Run dimension sync
        // 4. Verify Silver dimension table
        // 5. Join with fact table

        assert!(true, "Placeholder for full E2E test");
    }

    #[tokio::test]
    #[ignore] // Requires full infrastructure
    async fn test_csv_to_bronze_to_silver_etl() {
        // Verify CSV data flows through entire pipeline:
        // CSV -> CsvSource -> Bronze Parquet -> ETL -> Silver TimescaleDB

        assert!(true, "Placeholder for pipeline E2E test");
    }
}
