# BUG-001 Test Updates

## Overview

This document specifies all test additions and updates needed for the DP-007 fix.

---

## Existing Tests (Verification)

### `core/src/config/silver_etl.rs`

These existing tests verify the **correct** config format and should PASS:

| Test # | Name | Purpose |
|--------|------|---------|
| 26 | `test_parse_pre_transform_config_array_explosion` | Parse ArrayExplosion config |
| 27 | `test_parse_array_explosion_config_defaults` | Default values applied |
| 28 | `test_parse_metric_explosion_mapping` | Single metric parsing |
| 29 | `test_silver_etl_config_with_pre_transform` | Full config with pre_transform |
| 30 | `test_silver_etl_config_without_pre_transform` | Backward compatibility |
| 31 | `test_pre_transform_serialization_round_trip` | Serialization works |

**Run**: `cargo test -p neural-core config::silver_etl::tests`

### `apps/silver-etl/src/etl.rs`

These existing tests verify SQL generation behavior:

| Test # | Name | Purpose |
|--------|------|---------|
| 12 | `test_pre_transform_not_applied_when_disabled` | No pre_transform → read_parquet |
| 13 | `test_pre_transform_applied_when_enabled` | pre_transform → FROM pre_transformed |
| 14 | `test_sql_uses_temp_table_after_pre_transform` | SQL structure correct |

**Run**: `cargo test -p silver-etl etl::tests`

### `apps/silver-etl/src/pre_transform.rs`

These existing tests verify the pre-transform logic:

| Test | Name | Purpose |
|------|------|---------|
| 1 | `test_create_temp_table` | Table creation works |
| 2 | `test_create_temp_table_has_correct_columns` | Schema is correct |
| 3 | `test_apply_pre_transform_flattens_array` | Array flattening works |
| 4 | `test_valid_time_extracted_from_tags` | valid_time extraction |
| 5 | `test_metric_name_extracted_correctly` | metric_name mapping |
| 6 | `test_ndp_id_preserved` | ndp_id passthrough |
| 7 | `test_location_id_preserved` | location_id passthrough |
| 8 | `test_graceful_handling_missing_metric` | Missing metrics skipped |
| 9 | `test_multiple_payloads` | Batch processing |
| 10 | `test_build_parser` | Parser factory works |
| 11 | `test_empty_payloads` | Empty input handled |
| 12 | `test_value_stored_correctly` | Values preserved |
| 13 | `test_none_ndp_id_handled` | None ndp_id → empty string |

**Run**: `cargo test -p silver-etl pre_transform::tests`

---

## New Tests to Add

### Test 1: `build_parser_from_config`

**File**: `apps/silver-etl/src/pre_transform.rs`

**Add to `mod tests`**:

```rust
// ============================================================
// Test: build_parser_from_config creates correct parser
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

// ============================================================
// Test: build_parser_from_config applies defaults
// ============================================================
#[test]
fn test_build_parser_from_config_with_defaults() {
    use neural_core::config::{
        ArrayExplosionConfig, MetricExplosionMapping, PreTransformConfig, PreTransformType,
    };

    // Config with default values
    let config = PreTransformConfig {
        transform_type: PreTransformType::ArrayExplosion(ArrayExplosionConfig {
            metrics_base_path: "data".to_string(),
            timestamp_field: "validTime".to_string(), // default
            value_field: "value".to_string(),         // default
            values_path: "values".to_string(),        // default
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
```

### Test 2: `extract_bronze_raw_data`

**File**: `apps/silver-etl/src/etl.rs`

**Add to `mod tests`**:

```rust
// ============================================================
// Test: extract_bronze_raw_data reads from Parquet
// ============================================================
#[test]
fn test_extract_bronze_raw_data() {
    let temp_dir = TempDir::new().unwrap();
    let stream_dir = temp_dir.path().join("test-stream/year=2026/month=01/day=12");
    std::fs::create_dir_all(&stream_dir).unwrap();

    // Create test parquet with raw_payload
    let parquet_path = stream_dir.join("data.parquet");
    create_test_parquet_with_payload(&parquet_path, r#"{"temp": 25.5}"#);

    let runner = EtlRunner::new_in_memory().unwrap();
    let raw_data = runner
        .extract_bronze_raw_data("test-stream", temp_dir.path().to_str().unwrap(), None)
        .expect("Should extract raw data");

    assert_eq!(raw_data.timestamps.len(), 1);
    assert_eq!(raw_data.ndp_ids.len(), 1);
    assert_eq!(raw_data.raw_payloads.len(), 1);

    // Verify payload content
    let payload = &raw_data.raw_payloads[0];
    assert_eq!(payload["temp"], 25.5);
}

// ============================================================
// Test: extract_bronze_raw_data with watermark filter
// ============================================================
#[test]
fn test_extract_bronze_raw_data_with_watermark() {
    let temp_dir = TempDir::new().unwrap();
    let stream_dir = temp_dir.path().join("test-stream/year=2026/month=01/day=12");
    std::fs::create_dir_all(&stream_dir).unwrap();

    // Create test parquet
    let parquet_path = stream_dir.join("data.parquet");
    create_test_parquet_with_timestamp(&parquet_path, 1704886800000000_i64);

    let runner = EtlRunner::new_in_memory().unwrap();

    // Watermark after the data timestamp - should return empty
    let future_watermark = Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap();
    let raw_data = runner
        .extract_bronze_raw_data(
            "test-stream",
            temp_dir.path().to_str().unwrap(),
            Some(future_watermark),
        )
        .expect("Should succeed");

    assert!(raw_data.timestamps.is_empty(), "Should filter out old data");
}

// Helper function
fn create_test_parquet_with_payload(path: &std::path::Path, payload: &str) {
    use polars::prelude::*;

    let df = df! {
        "timestamp" => &[1704886800000000_i64],
        "ndp_id" => &["test-ndp-id"],
        "source_id" => &["test://source"],
        "context" => &[r#"{}"#],
        "raw_payload" => &[payload]
    }
    .unwrap();

    let file = std::fs::File::create(path).unwrap();
    ParquetWriter::new(file).finish(&mut df.clone()).unwrap();
}

fn create_test_parquet_with_timestamp(path: &std::path::Path, ts: i64) {
    use polars::prelude::*;

    let df = df! {
        "timestamp" => &[ts],
        "ndp_id" => &["test-ndp-id"],
        "source_id" => &["test://source"],
        "context" => &[r#"{}"#],
        "raw_payload" => &[r#"{"value": 1}"#]
    }
    .unwrap();

    let file = std::fs::File::create(path).unwrap();
    ParquetWriter::new(file).finish(&mut df.clone()).unwrap();
}
```

### Test 3: End-to-End Pre-Transform Integration

**File**: `apps/silver-etl/src/etl.rs`

**Add to `mod tests`**:

```rust
// ============================================================
// Test: End-to-end pre-transform integration
// ============================================================
#[test]
fn test_pre_transform_end_to_end_integration() {
    use neural_core::config::{
        ArrayExplosionConfig, MetricExplosionMapping, PreTransformConfig, PreTransformType,
    };

    let temp_dir = TempDir::new().unwrap();
    let stream_dir = temp_dir
        .path()
        .join("nws-forecast/year=2026/month=01/day=12");
    std::fs::create_dir_all(&stream_dir).unwrap();

    // Create test parquet with NWS-like payload
    let parquet_path = stream_dir.join("data.parquet");
    let nws_payload = r#"{
        "id": "MTR-50-75",
        "properties": {
            "temperature": {
                "values": [
                    {"validTime": "2026-01-12T00:00:00+00:00/PT1H", "value": 15.5},
                    {"validTime": "2026-01-12T01:00:00+00:00/PT1H", "value": 14.8}
                ]
            }
        }
    }"#;
    create_test_parquet_with_payload(&parquet_path, nws_payload);

    // Create config with pre_transform
    let mut config = create_test_silver_config();
    config.target_table = "silver.weather_forecasts".to_string();
    config.pre_transform = Some(PreTransformConfig {
        transform_type: PreTransformType::ArrayExplosion(ArrayExplosionConfig {
            metrics_base_path: "properties".to_string(),
            timestamp_field: "validTime".to_string(),
            value_field: "value".to_string(),
            values_path: "values".to_string(),
            metrics: vec![MetricExplosionMapping {
                metric_path: "temperature".to_string(),
                target_column: "temp_c".to_string(),
                column_type: "double_precision".to_string(),
            }],
        }),
    });

    let runner = EtlRunner::new_in_memory().unwrap();

    // Dry run should generate SQL using pre_transformed table
    let sql = runner
        .dry_run(&config, "nws-forecast", temp_dir.path().to_str().unwrap())
        .expect("Should generate SQL");

    assert!(
        sql.contains("FROM pre_transformed"),
        "SQL should use pre_transformed table"
    );
}
```

### Test 4: Config Loading with Pre-Transform

**File**: `apps/silver-etl/src/config.rs`

**Add to `mod tests`**:

```rust
// ============================================================
// Test: YAML config with pre_transform deserializes correctly
// ============================================================
#[tokio::test]
async fn test_load_yaml_config_with_pre_transform() {
    let temp_dir = TempDir::new().unwrap();

    // YAML with correct pre_transform format
    let config_content = r#"
stream_id: nws-gridpoints-forecast
silver_etl:
  enabled: true
  target_table: silver.weather_forecasts
  pre_transform:
    transform_type:
      type: array_explosion
      metrics_base_path: properties
      metrics:
        - metric_path: temperature
          target_column: temperature_c
          type: double_precision
        - metric_path: windSpeed
          target_column: wind_speed_kmh
          type: double_precision
  timestamp:
    source_field: timestamp
    target_field: issue_time
    transform: microseconds_to_timestamp
  field_mappings: []
"#;

    let config_path = temp_dir.path().join("nws-gridpoints-forecast.yaml");
    tokio::fs::write(&config_path, config_content)
        .await
        .unwrap();

    let loader = ConfigLoader::new("http://localhost:2379", temp_dir.path().to_str().unwrap());

    let config = loader
        .load_from_yaml("nws-gridpoints-forecast")
        .await
        .expect("Should load config with pre_transform");

    assert!(config.enabled);
    assert!(config.pre_transform.is_some(), "pre_transform should be Some");

    let pre_transform = config.pre_transform.unwrap();
    match &pre_transform.transform_type {
        neural_core::config::PreTransformType::ArrayExplosion(explosion) => {
            assert_eq!(explosion.metrics_base_path, "properties");
            assert_eq!(explosion.metrics.len(), 2);
            assert_eq!(explosion.metrics[0].metric_path, "temperature");
            assert_eq!(explosion.metrics[0].target_column, "temperature_c");
        }
    }
}

// ============================================================
// Test: YAML with OLD/WRONG format fails gracefully
// ============================================================
#[tokio::test]
async fn test_load_yaml_config_with_wrong_pre_transform_format() {
    let temp_dir = TempDir::new().unwrap();

    // YAML with OLD wrong format (this should NOT work)
    let config_content = r#"
stream_id: test-stream
silver_etl:
  enabled: true
  target_table: silver.test
  pre_transform:
    enabled: true
    parser_type: column_oriented
    parser_config_ref: sources[0].parser
  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp
  field_mappings: []
"#;

    let config_path = temp_dir.path().join("test-stream.yaml");
    tokio::fs::write(&config_path, config_content)
        .await
        .unwrap();

    let loader = ConfigLoader::new("http://localhost:2379", temp_dir.path().to_str().unwrap());

    let result = loader.load_from_yaml("test-stream").await;

    // This should fail because the pre_transform format is wrong
    assert!(result.is_err(), "Wrong pre_transform format should cause error");
}
```

---

## Test Execution Commands

### Run All Affected Tests

```bash
# Core config tests
cargo test -p neural-core config::silver_etl::tests

# Silver ETL tests
cargo test -p silver-etl

# Specific test modules
cargo test -p silver-etl pre_transform::tests
cargo test -p silver-etl etl::tests
cargo test -p silver-etl config::tests
```

### Run Only New Tests

```bash
# Pre-transform parser builder
cargo test -p silver-etl test_build_parser_from_config

# Bronze data extraction
cargo test -p silver-etl test_extract_bronze_raw_data

# End-to-end integration
cargo test -p silver-etl test_pre_transform_end_to_end

# Config loading
cargo test -p silver-etl test_load_yaml_config_with_pre_transform
cargo test -p silver-etl test_load_yaml_config_with_wrong_pre_transform_format
```

---

## Test Coverage Checklist

| Component | Test Coverage |
|-----------|---------------|
| `PreTransformConfig` deserialization | ✅ Existing (tests 26-31) |
| `ArrayExplosionConfig` parsing | ✅ Existing (tests 26-28) |
| `build_parser_from_config` | 🆕 New (2 tests) |
| `extract_bronze_raw_data` | 🆕 New (2 tests) |
| `apply_pre_transform` | ✅ Existing (tests 3-13) |
| ETL SQL generation | ✅ Existing (tests 12-14) |
| YAML config loading | ✅ Existing + 🆕 New (2 tests) |
| End-to-end integration | 🆕 New (1 test) |

**Total new tests**: 7
**Total existing tests to verify**: 27+
