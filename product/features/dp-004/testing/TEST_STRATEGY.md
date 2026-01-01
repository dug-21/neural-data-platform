# DP-004: Bronze Raw JSON Schema - Test Strategy

## Overview

This document defines the test strategy for DP-004, which changes the Bronze layer to store raw JSON payloads instead of parsed, typed metrics.

## Scope of Changes

Based on ADR-001, the following components are affected:

| Component | Change Description | Test Priority |
|-----------|-------------------|---------------|
| `core/src/traits.rs` | New `RawDataPoint` struct | **High** |
| `core/src/storage/parquet.rs` | New 5-column schema | **High** |
| `core/src/sources/*.rs` | Return `RawDataPoint` instead of `Vec<TimeSeriesPoint>` | **Medium** |
| `core/src/parsers/*.rs` | Simplified metadata extraction only | **Medium** |
| `apps/air-quality-app/src/pipeline/*.rs` | Handle `RawDataPoint` | **Medium** |

## Test Taxonomy

### 1. Unit Tests (Fast, Isolated)

#### 1.1 RawDataPoint Struct Tests

**Location**: `core/src/traits.rs` (in `#[cfg(test)] mod tests`)

| Test Name | Purpose | Priority |
|-----------|---------|----------|
| `test_raw_data_point_creation` | Verify struct instantiation with all fields | Must-have |
| `test_raw_data_point_with_optional_fields` | Test with None ndp_id and context | Must-have |
| `test_raw_data_point_serde_roundtrip` | Serialize and deserialize preserves data | Must-have |
| `test_raw_data_point_serde_skip_none_fields` | None fields not serialized to JSON | Must-have |
| `test_raw_data_point_json_payload_nested` | Test deeply nested JSON in raw_payload | Must-have |
| `test_raw_data_point_json_payload_array` | Test array values in raw_payload | Must-have |
| `test_raw_data_point_json_payload_special_chars` | Test unicode/special characters | Nice-to-have |
| `test_raw_data_point_equality` | PartialEq implementation works | Must-have |
| `test_raw_data_point_clone` | Clone implementation works | Must-have |
| `test_raw_data_point_debug` | Debug implementation for logging | Nice-to-have |

**Template**:
```rust
#[test]
fn test_raw_data_point_creation() {
    let raw_payload = serde_json::json!({
        "pm02": 12.5,
        "rco2": 450,
        "serialno": "abc123"
    });

    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "air-quality-Mqtt".to_string(),
        ndp_id: Some("airgradient-001".to_string()),
        context: Some(serde_json::json!({"room": "office"})),
        raw_payload,
    };

    assert_eq!(point.source_id, "air-quality-Mqtt");
    assert!(point.ndp_id.is_some());
    assert!(point.context.is_some());
    assert!(point.raw_payload.is_object());
}
```

#### 1.2 Parquet Schema Tests

**Location**: `core/src/storage/parquet.rs` (in `#[cfg(test)] mod tests`)

| Test Name | Purpose | Priority |
|-----------|---------|----------|
| `test_raw_schema_column_count` | New schema has exactly 5 columns | Must-have |
| `test_raw_schema_column_names` | Column names match: timestamp, source_id, ndp_id, context, raw_payload | Must-have |
| `test_raw_schema_column_types` | Types match: DateTime, String, String?, JSON?, JSON | Must-have |
| `test_write_raw_data_point` | Write single RawDataPoint to Parquet | Must-have |
| `test_write_raw_data_point_batch` | Batch write multiple RawDataPoints | Must-have |
| `test_query_raw_data_point` | Query returns RawDataPoints correctly | Must-have |
| `test_raw_payload_json_integrity` | JSON structure preserved exactly | Must-have |
| `test_context_json_integrity` | Context JSON preserved exactly | Must-have |
| `test_null_ndp_id_handling` | None ndp_id stored/retrieved as null | Must-have |
| `test_null_context_handling` | None context stored/retrieved as null | Must-have |
| `test_partition_path_uses_source_id` | Partitioning by source_id not location_id | Must-have |

**Template**:
```rust
#[tokio::test]
async fn test_write_raw_data_point() {
    let (store, _temp) = create_test_store();
    let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

    let raw_payload = serde_json::json!({
        "pm02": 12.5,
        "rco2": 450,
        "serialno": "abc123"
    });

    let point = RawDataPoint {
        timestamp,
        source_id: "air-quality-Mqtt".to_string(),
        ndp_id: Some("airgradient-001".to_string()),
        context: Some(serde_json::json!({"room": "office"})),
        raw_payload: raw_payload.clone(),
    };

    let result = store.write_raw(point).await;
    assert!(result.is_ok());

    // Query back and verify
    let results = store.query_raw("air-quality-Mqtt", timestamp - Duration::hours(1), timestamp + Duration::hours(1)).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].raw_payload, raw_payload);
}
```

### 2. Integration Tests (Slower, Real Dependencies)

**Location**: `tests/integration/dp_004_bronze_schema_test.rs`

| Test Name | Purpose | Priority |
|-----------|---------|----------|
| `test_http_source_emits_raw_data_point` | HTTP source returns RawDataPoint | Must-have |
| `test_mqtt_source_emits_raw_data_point` | MQTT source returns RawDataPoint | Must-have |
| `test_raw_data_point_through_pipeline` | Full ingestion pipeline with RawDataPoint | Must-have |
| `test_backward_compatible_old_files` | Can still read old Parquet schema | Must-have |
| `test_duckdb_query_raw_payload` | DuckDB can query JSON in raw_payload column | Nice-to-have |
| `test_multi_source_partition_isolation` | Different sources write to different partitions | Must-have |

**Template**:
```rust
#[tokio::test]
#[ignore] // Run with --ignored when infrastructure available
async fn test_http_source_emits_raw_data_point() {
    // Setup mock HTTP server
    let mock_server = MockServer::start().await;
    let response_body = r#"{"pm02": 12.5, "rco2": 450, "serialno": "TEST123"}"#;

    Mock::given(method("GET"))
        .and(path("/measures/current"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
        .mount(&mock_server)
        .await;

    // Create source
    let config = create_test_http_config(&mock_server);
    let source = HttpPollingSource::new_raw(config).unwrap();

    // Fetch should return RawDataPoint
    let points = source.fetch_raw().await.unwrap();
    assert_eq!(points.len(), 1);

    // Verify raw payload is preserved
    let raw = &points[0].raw_payload;
    assert_eq!(raw["pm02"], 12.5);
    assert_eq!(raw["rco2"], 450);
    assert_eq!(raw["serialno"], "TEST123");
}
```

### 3. Backward Compatibility Tests

**Location**: `tests/components/parquet/backward_compat_test.rs`

| Test Name | Purpose | Priority |
|-----------|---------|----------|
| `test_read_old_schema_parquet` | Query layer reads old (tall) schema files | Must-have |
| `test_read_new_schema_parquet` | Query layer reads new (wide) schema files | Must-have |
| `test_mixed_schema_query` | Query across partition boundary with mixed schemas | Must-have |
| `test_schema_detection` | Automatic schema version detection | Must-have |
| `test_migration_dual_write` | Can write both formats during migration | Nice-to-have |

### 4. Edge Case Tests

**Location**: `core/src/storage/parquet.rs` (unit tests) and `tests/integration/` (integration tests)

| Test Name | Purpose | Priority |
|-----------|---------|----------|
| `test_empty_raw_payload` | Empty JSON object `{}` as payload | Must-have |
| `test_large_raw_payload` | Very large JSON payload (>1MB) | Nice-to-have |
| `test_special_characters_in_json` | Unicode, newlines, null bytes | Must-have |
| `test_deeply_nested_json` | 10+ levels of nesting | Must-have |
| `test_json_array_root` | Root payload is array, not object | Must-have |
| `test_invalid_json_handling` | Source returns invalid JSON | Must-have |
| `test_timestamp_precision` | Microsecond precision preserved | Must-have |

## Mock Requirements

### MockRawSource

For testing the new Source trait method that returns `RawDataPoint`:

```rust
mock! {
    pub RawSource {}

    #[async_trait]
    impl RawSource for RawSource {
        async fn fetch_raw(&self) -> CoreResult<Vec<RawDataPoint>>;
        async fn health_check(&self) -> CoreResult<HealthStatus>;
    }
}
```

### MockRawStore

For testing the new Store trait methods:

```rust
mock! {
    pub RawStore {}

    #[async_trait]
    impl RawStore for RawStore {
        async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>;
        async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>;
        async fn query_raw(
            &self,
            source_id: &str,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
        ) -> CoreResult<Vec<RawDataPoint>>;
    }
}
```

## Test Data Fixtures

Create test fixtures in `tests/fixtures/dp_004/`:

```
tests/fixtures/dp_004/
├── air_quality_payload.json       # AirGradient sensor payload
├── weather_payload.json           # OpenWeatherMap payload
├── window_sensor_payload.json     # Binary state sensor
├── nested_complex_payload.json    # Deeply nested structure
├── unicode_payload.json           # Special characters
└── large_payload.json             # ~1MB JSON file
```

Example fixture (`air_quality_payload.json`):
```json
{
  "pm02": 12.5,
  "rco2": 450,
  "atmp": 22.3,
  "rhum": 55.0,
  "serialno": "abc123",
  "firmware": "3.4.1",
  "wifi": -45
}
```

## TDD Implementation Order

Following London School TDD (outside-in), implement tests in this order:

### Phase 1: Core Types (Day 1)

1. **RawDataPoint struct tests** - Define the contract
2. **RawDataPoint serde tests** - Serialization behavior
3. **RawDataPoint equality/clone tests** - Standard traits

### Phase 2: Storage Layer (Day 2-3)

4. **RawStore trait tests** - Define storage interface
5. **ParquetStore write_raw tests** - Single point write
6. **ParquetStore write_raw_batch tests** - Batch write
7. **ParquetStore query_raw tests** - Query with new schema
8. **Schema column validation tests** - 5-column schema

### Phase 3: Backward Compatibility (Day 3)

9. **Schema detection tests** - Old vs new schema
10. **Old schema read tests** - Read existing files
11. **Mixed schema query tests** - Cross-partition queries

### Phase 4: Source Integration (Day 4)

12. **HTTP source fetch_raw tests** - HTTP returns RawDataPoint
13. **MQTT source fetch_raw tests** - MQTT returns RawDataPoint
14. **Parser simplification tests** - Metadata-only parsing

### Phase 5: Pipeline Integration (Day 5)

15. **End-to-end pipeline test** - Source -> Storage with RawDataPoint
16. **Multi-source test** - Multiple sources, correct partitioning
17. **Error handling tests** - Invalid JSON, network failures

## Coverage Targets

| Component | Target Coverage | Rationale |
|-----------|-----------------|-----------|
| RawDataPoint struct | 95% | Core type, must be bulletproof |
| ParquetStore (raw methods) | 90% | Storage layer critical |
| Source implementations | 80% | Integration points |
| Backward compatibility | 85% | Migration safety |
| Edge cases | 70% | Defensive coverage |

## Running Tests

```bash
# Unit tests only (fast)
cargo test --package platform-core raw_data_point
cargo test --package platform-core parquet::tests::test_raw

# Integration tests (requires mocks)
cargo test --test dp_004_bronze_schema_test

# All dp-004 related tests
cargo test dp_004

# With coverage
cargo tarpaulin --out Html --packages platform-core -- dp_004
```

## Test Dependencies

Add to `Cargo.toml` if not present:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
mockall = "0.12"
wiremock = "0.6"
tempfile = "3"
serde_json = "1"
```

## Success Criteria

1. All must-have tests pass
2. Coverage targets met per component
3. No regressions in existing tests
4. Backward compatibility verified with old Parquet files
5. JSON payload integrity verified through roundtrip tests

## Related Documents

- [ADR-001: Bronze Raw JSON Schema](/workspaces/neural-data-platform/product/features/dp-004/architecture/ADR-001-bronze-raw-json-schema.md)
- [AIR-005 Test Design](/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md) - London School TDD patterns
- [SCOPE.md](/workspaces/neural-data-platform/product/features/dp-004/SCOPE.md) - Feature requirements
