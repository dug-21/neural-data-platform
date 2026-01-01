# DP-004: Integration Tests

## Overview

This document defines the integration test scenarios for verifying the Bronze Raw JSON Schema implementation. Tests cover end-to-end data flow, backward compatibility, and performance validation.

---

## Test Environment

### Prerequisites

```bash
# Test database setup
docker run -d --name test-duckdb -v /tmp/test-bronze:/data duckdb/duckdb

# Test fixtures
cargo run --bin generate-fixtures -- --output /tmp/test-fixtures/
```

### Test Data

| Source | Sample File | Records | Description |
|--------|-------------|---------|-------------|
| AirGradient | `airgradient_sample.json` | 100 | Air quality sensor data |
| HomeAssistant | `homeassistant_sample.json` | 50 | Mixed entity states |
| OpenWeatherMap | `owm_sample.json` | 20 | Weather API responses |
| Window Sensor | `window_sample.json` | 30 | Binary state events |

---

## End-to-End Test Scenarios

### E2E-001: Single Source Ingestion

**Objective**: Verify a single HTTP source writes valid RawDataPoint to Parquet

**Setup**:
```rust
#[tokio::test]
async fn test_single_source_ingestion() {
    // Start mock HTTP server with AirGradient data
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({
                "pm02": 12.5,
                "rco2": 450,
                "serialno": "test-001"
            })))
        .mount(&mock_server)
        .await;

    // Configure source
    let config = SourceConfig {
        source_id: "test-airgradient",
        url: mock_server.uri(),
        poll_interval: Duration::from_secs(1),
    };

    // Run pipeline for 5 seconds
    // ...

    // Verify Parquet output
    let result = read_parquet("/tmp/test/bronze/*.parquet");
    assert!(result.len() >= 1);
    assert_eq!(result[0].source_id, "test-airgradient");
    assert!(result[0].raw_payload.is_object());
}
```

**Expected Results**:
- [ ] Parquet file created in output directory
- [ ] Schema has 5 columns: timestamp, source_id, ndp_id, context, raw_payload
- [ ] raw_payload contains exact JSON from mock server
- [ ] timestamp is within expected range

---

### E2E-002: Multiple Sources Concurrent

**Objective**: Verify multiple sources can write concurrently without data corruption

**Setup**:
```rust
#[tokio::test]
async fn test_multiple_sources_concurrent() {
    // Start 3 mock servers for different sources
    let air_server = start_mock_server(AIR_QUALITY_RESPONSES).await;
    let weather_server = start_mock_server(WEATHER_RESPONSES).await;
    let sensor_server = start_mock_server(SENSOR_RESPONSES).await;

    // Configure all sources
    let sources = vec![
        SourceConfig::new("air-quality", air_server.uri()),
        SourceConfig::new("weather", weather_server.uri()),
        SourceConfig::new("sensors", sensor_server.uri()),
    ];

    // Run pipeline with all sources
    let pipeline = Pipeline::new(sources);
    pipeline.run_for(Duration::from_secs(10)).await;

    // Verify all sources wrote data
    let results = read_all_parquet("/tmp/test/bronze/");

    assert!(results.iter().any(|r| r.source_id == "air-quality"));
    assert!(results.iter().any(|r| r.source_id == "weather"));
    assert!(results.iter().any(|r| r.source_id == "sensors"));
}
```

**Expected Results**:
- [ ] All three sources produce Parquet files
- [ ] No data corruption or mixing between sources
- [ ] Record counts match expected polling frequency
- [ ] No race conditions or deadlocks

---

### E2E-003: Raw Payload Preservation

**Objective**: Verify exact source payload is preserved without transformation

**Setup**:
```rust
#[tokio::test]
async fn test_raw_payload_preservation() {
    // Create payload with various data types
    let original_payload = json!({
        "numeric_int": 42,
        "numeric_float": 3.14159,
        "string_value": "hello world",
        "boolean_true": true,
        "boolean_false": false,
        "null_value": null,
        "nested_object": {
            "inner_key": "inner_value",
            "inner_array": [1, 2, 3]
        },
        "array_mixed": [1, "two", true, null]
    });

    // Ingest and retrieve
    let result = ingest_and_read(original_payload.clone()).await;

    // Verify exact match
    assert_eq!(result.raw_payload, original_payload);
}
```

**Expected Results**:
- [ ] Integer values preserved exactly
- [ ] Float precision maintained
- [ ] String values unmodified
- [ ] Boolean values preserved as true/false
- [ ] Null values preserved
- [ ] Nested structures intact
- [ ] Arrays preserved with correct types

---

### E2E-004: Context Metadata Snapshot

**Objective**: Verify context is captured from config at ingestion time

**Setup**:
```rust
#[tokio::test]
async fn test_context_metadata_snapshot() {
    // Configure stream with context
    let stream_config = StreamConfig {
        stream_id: "test-stream",
        ndp_id: Some("sensor-001".into()),
        context: Some(json!({
            "room": "office",
            "floor": 2,
            "building": "HQ"
        })),
    };

    // Ingest data
    let result = ingest_with_config(stream_config).await;

    // Verify context captured
    assert_eq!(result.ndp_id, Some("sensor-001".into()));
    assert_eq!(result.context["room"], "office");
    assert_eq!(result.context["floor"], 2);
}
```

**Expected Results**:
- [ ] ndp_id populated from stream config
- [ ] context JSON matches config exactly
- [ ] Context frozen at ingestion time (not affected by config changes after)

---

## Data Flow Verification

### FLOW-001: Source to Channel

```rust
#[tokio::test]
async fn test_source_to_channel_flow() {
    let (tx, rx) = mpsc::channel(100);

    // Create source that emits RawDataPoint
    let source = HttpPollSource::new(config);
    source.start(tx).await;

    // Receive and verify
    let point = rx.recv().await.unwrap();
    assert!(point.timestamp <= Utc::now());
    assert!(!point.source_id.is_empty());
    assert!(point.raw_payload.is_object());
}
```

### FLOW-002: Channel to Storage

```rust
#[tokio::test]
async fn test_channel_to_storage_flow() {
    let (tx, rx) = mpsc::channel(100);
    let storage = ParquetStorage::new("/tmp/test/bronze/");

    // Send test points
    for i in 0..100 {
        tx.send(create_test_point(i)).await.unwrap();
    }
    drop(tx);

    // Process channel to storage
    storage.consume(rx).await;

    // Verify all written
    let count = count_parquet_records("/tmp/test/bronze/");
    assert_eq!(count, 100);
}
```

### FLOW-003: Full Pipeline Latency

```rust
#[tokio::test]
async fn test_full_pipeline_latency() {
    let start = Instant::now();

    // Inject known timestamp
    let injection_time = Utc::now();
    let point = RawDataPoint {
        timestamp: injection_time,
        source_id: "latency-test".into(),
        ..Default::default()
    };

    // Ingest through full pipeline
    pipeline.ingest(point).await;
    pipeline.flush().await;

    let latency = start.elapsed();

    // Should complete within 1 second
    assert!(latency < Duration::from_secs(1));

    // Read back and verify timestamp preserved
    let result = read_latest("/tmp/test/bronze/");
    assert_eq!(result.timestamp, injection_time);
}
```

---

## Performance Benchmarks

### PERF-001: Write Throughput

```rust
#[bench]
fn bench_write_throughput(b: &mut Bencher) {
    let points: Vec<RawDataPoint> = generate_test_points(10_000);
    let storage = ParquetStorage::new("/tmp/bench/");

    b.iter(|| {
        storage.write_batch(&points).unwrap();
    });
}
```

**Baseline Requirements**:
- [ ] >= 1,000 points/second sustained write
- [ ] < 100ms p99 write latency
- [ ] Linear scaling with batch size

### PERF-002: Storage Efficiency

```rust
#[test]
fn test_storage_efficiency() {
    // Write 10,000 points with realistic payloads
    let points = generate_realistic_points(10_000);
    let storage = ParquetStorage::new("/tmp/bench/");
    storage.write_batch(&points).unwrap();

    // Calculate storage efficiency
    let file_size = fs::metadata("/tmp/bench/data.parquet").unwrap().len();
    let avg_size = file_size / 10_000;

    // Should be < 500 bytes per record (with compression)
    assert!(avg_size < 500, "Average record size: {} bytes", avg_size);
}
```

**Baseline Requirements**:
- [ ] < 500 bytes per record (compressed)
- [ ] Comparable or better than v1 schema
- [ ] Parquet compression ratio > 3:1

### PERF-003: Query Performance

```rust
#[test]
fn test_query_performance() {
    // Write test data
    write_test_data("/tmp/bench/", 100_000);

    // Time JSON extraction query
    let start = Instant::now();
    let conn = duckdb::Connection::open_in_memory().unwrap();
    conn.execute(r#"
        SELECT
            timestamp,
            json_extract_string(raw_payload, '$.pm02') as pm02
        FROM read_parquet('/tmp/bench/*.parquet')
        WHERE timestamp > NOW() - INTERVAL '1 hour'
    "#, []).unwrap();
    let query_time = start.elapsed();

    // Should complete within 500ms for 100k records
    assert!(query_time < Duration::from_millis(500));
}
```

**Baseline Requirements**:
- [ ] JSON extraction < 500ms for 100k records
- [ ] Full table scan < 2s for 1M records
- [ ] Aggregation queries < 1s for 100k records

---

## Backward Compatibility Tests

### COMPAT-001: Read V1 Schema

```rust
#[test]
fn test_read_v1_schema() {
    // V1 schema: timestamp, location_id, metric, value, tags, ndp_id, context
    let v1_file = "/fixtures/v1_sample.parquet";

    // Should read without error
    let result = ParquetReader::read(v1_file);
    assert!(result.is_ok());

    // Should detect v1 schema
    let schema_version = ParquetReader::detect_schema_version(v1_file);
    assert_eq!(schema_version, SchemaVersion::V1);
}
```

### COMPAT-002: Mixed Schema Directory

```rust
#[test]
fn test_mixed_schema_directory() {
    // Directory with both v1 and v2 files
    let dir = "/fixtures/mixed/";

    // Should read all files
    let results = ParquetReader::read_directory(dir);
    assert!(results.is_ok());

    // Should have records from both schemas
    let records = results.unwrap();
    assert!(records.iter().any(|r| r.schema_version == "v1"));
    assert!(records.iter().any(|r| r.schema_version == "v2"));
}
```

### COMPAT-003: Query Abstraction

```rust
#[test]
fn test_query_abstraction_layer() {
    // Query should work regardless of underlying schema
    let query_result = query_bronze(
        "/fixtures/mixed/",
        "SELECT timestamp, source_id FROM bronze WHERE timestamp > '2026-01-01'"
    );

    assert!(query_result.is_ok());
    // Both v1 and v2 records included
    assert!(query_result.unwrap().len() > 0);
}
```

---

## Error Handling Tests

### ERR-001: Malformed JSON Payload

```rust
#[test]
fn test_malformed_json_handling() {
    let source_response = "not valid json {{{";

    // Should not crash
    let result = parse_response(source_response);

    // Should return error or store as raw string
    match result {
        Err(ParseError::InvalidJson(_)) => (),  // OK
        Ok(point) => {
            // If stored, should be as escaped string
            assert!(point.raw_payload.is_string());
        }
        _ => panic!("Unexpected result"),
    }
}
```

### ERR-002: Source Timeout

```rust
#[tokio::test]
async fn test_source_timeout_handling() {
    // Mock server that delays 30 seconds
    let slow_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
        .mount(&slow_server)
        .await;

    let source = HttpPollSource::new(SourceConfig {
        url: slow_server.uri(),
        timeout: Duration::from_secs(5),
        ..Default::default()
    });

    // Should timeout, not hang
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        source.poll_once()
    ).await;

    assert!(result.is_ok()); // Didn't hang
    assert!(result.unwrap().is_err()); // Source timed out
}
```

### ERR-003: Storage Write Failure

```rust
#[test]
fn test_storage_write_failure_recovery() {
    // Write to read-only directory
    let storage = ParquetStorage::new("/readonly/");
    let points = generate_test_points(10);

    let result = storage.write_batch(&points);
    assert!(result.is_err());

    // Points should be retained for retry
    assert_eq!(storage.pending_count(), 10);

    // After fixing permissions, retry should work
    // ...
}
```

---

## Test Fixtures

### Sample Payloads

```json
// fixtures/airgradient_payload.json
{
    "wifi": -67,
    "serialno": "abc123",
    "pm02": 12,
    "rco2": 450,
    "atmp": 22.5,
    "rhum": 45.2
}

// fixtures/homeassistant_event.json
{
    "entity_id": "sensor.living_room_temperature",
    "state": "21.5",
    "attributes": {
        "unit_of_measurement": "C",
        "friendly_name": "Living Room Temperature",
        "device_class": "temperature"
    },
    "last_changed": "2026-01-01T12:00:00Z"
}

// fixtures/window_sensor.json
{
    "state": "open",
    "battery": 85,
    "last_triggered": "2026-01-01T11:45:00Z"
}
```

### Expected Parquet Schema

```
root
 |-- timestamp: timestamp[us, tz=UTC] (nullable = false)
 |-- source_id: string (nullable = false)
 |-- ndp_id: string (nullable = true)
 |-- context: string (nullable = true)  -- JSON as string
 |-- raw_payload: string (nullable = false)  -- JSON as string
```

---

## Test Execution

### Local Development

```bash
# Run all integration tests
cargo test --test integration -- --test-threads=1

# Run specific test
cargo test --test integration test_single_source_ingestion

# Run with verbose output
cargo test --test integration -- --nocapture
```

### CI Pipeline

```yaml
# .github/workflows/test.yml
integration_tests:
  runs-on: ubuntu-latest
  services:
    duckdb:
      image: duckdb/duckdb
  steps:
    - uses: actions/checkout@v3
    - name: Run integration tests
      run: cargo test --test integration
    - name: Upload test results
      uses: actions/upload-artifact@v3
      with:
        name: test-results
        path: target/test-results/
```

---

## Coverage Requirements

| Category | Minimum Coverage | Target Coverage |
|----------|-----------------|-----------------|
| RawDataPoint | 90% | 95% |
| ParquetStorage | 85% | 90% |
| Sources | 80% | 90% |
| Integration | 70% | 80% |

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-01 | ndp-scrum-master | Initial draft |
