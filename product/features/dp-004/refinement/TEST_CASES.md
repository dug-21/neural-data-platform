# DP-004: Detailed Test Cases

## Test Case Format

Each test case follows Given-When-Then (GWT) format with London School annotations:

```
TC-XXX: <Test Name>
Type: Unit | Integration | Acceptance
SUT: System Under Test
Mocks: List of mocked collaborators
Given: Preconditions
When: Action performed
Then: Expected outcome
```

---

## Raw JSON Storage (ADR-001)

**Key Change**: Bronze layer stores raw JSON payloads instead of parsed metrics.

**New Schema:**

| Column | Type | Description |
|--------|------|-------------|
| `timestamp` | DateTime | Ingestion timestamp |
| `source_id` | String | Source identifier (e.g., "air-quality-Mqtt") |
| `ndp_id` | String? | Platform-assigned stable ID |
| `context` | JSON? | Config-derived metadata snapshot |
| `raw_payload` | JSON | Exact payload from source |

---

## Unit Tests: RawDataPoint Construction

### TC-001: Construct RawDataPoint with all fields

**Type**: Unit
**SUT**: RawDataPoint struct
**Mocks**: None (pure data structure)

```
Given: All required and optional fields provided
When: RawDataPoint is constructed
Then: All fields accessible and match input values
```

```rust
#[test]
fn tc_001_construct_raw_data_point_with_all_fields() {
    let timestamp = Utc::now();
    let point = RawDataPoint {
        timestamp,
        source_id: "air-quality-Mqtt".to_string(),
        ndp_id: Some("airgradient-office-001".to_string()),
        context: Some(json!({
            "room": "office",
            "floor": 2
        })),
        raw_payload: json!({
            "pm02": 12.5,
            "rco2": 450,
            "atmp": 22.3,
            "serialno": "abc123"
        }),
    };

    assert_eq!(point.source_id, "air-quality-Mqtt");
    assert_eq!(point.ndp_id, Some("airgradient-office-001".to_string()));
    assert_eq!(point.context.unwrap()["room"], "office");
    assert_eq!(point.raw_payload["pm02"], 12.5);
    assert_eq!(point.raw_payload["serialno"], "abc123");
}
```

---

### TC-002: Construct RawDataPoint with minimal fields

**Type**: Unit
**SUT**: RawDataPoint struct
**Mocks**: None

```
Given: Only required fields (timestamp, source_id, raw_payload)
When: RawDataPoint is constructed
Then: Optional fields are None
```

```rust
#[test]
fn tc_002_construct_raw_data_point_minimal() {
    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "simple-source".to_string(),
        ndp_id: None,
        context: None,
        raw_payload: json!({"value": 42}),
    };

    assert_eq!(point.source_id, "simple-source");
    assert!(point.ndp_id.is_none());
    assert!(point.context.is_none());
    assert_eq!(point.raw_payload["value"], 42);
}
```

---

### TC-003: RawDataPoint preserves non-numeric data

**Type**: Unit
**SUT**: RawDataPoint raw_payload field
**Mocks**: None

```
Given: raw_payload contains strings, booleans, nested objects
When: RawDataPoint is constructed
Then: All non-numeric data preserved exactly
```

```rust
#[test]
fn tc_003_raw_data_point_preserves_non_numeric() {
    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "status-source".to_string(),
        ndp_id: None,
        context: None,
        raw_payload: json!({
            "status": "active",
            "connected": true,
            "error_code": null,
            "firmware": "v2.1.3",
            "metadata": {
                "model": "ONE-V9",
                "region": "us-east"
            },
            "tags": ["primary", "calibrated"]
        }),
    };

    assert_eq!(point.raw_payload["status"], "active");
    assert_eq!(point.raw_payload["connected"], true);
    assert!(point.raw_payload["error_code"].is_null());
    assert_eq!(point.raw_payload["firmware"], "v2.1.3");
    assert_eq!(point.raw_payload["metadata"]["model"], "ONE-V9");
    assert_eq!(point.raw_payload["tags"][0], "primary");
}
```

---

### TC-004: RawDataPoint serializes to JSON

**Type**: Unit
**SUT**: RawDataPoint serialization
**Mocks**: None

```
Given: RawDataPoint with all fields
When: Serialized to JSON string
Then: All fields present in output
```

```rust
#[test]
fn tc_004_raw_data_point_serializes_to_json() {
    let point = RawDataPoint {
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
        source_id: "test-source".to_string(),
        ndp_id: Some("test-001".to_string()),
        context: Some(json!({"room": "lab"})),
        raw_payload: json!({"temp": 22.5}),
    };

    let json_str = serde_json::to_string(&point).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert!(parsed["timestamp"].is_string());
    assert_eq!(parsed["source_id"], "test-source");
    assert_eq!(parsed["ndp_id"], "test-001");
    assert_eq!(parsed["context"]["room"], "lab");
    assert_eq!(parsed["raw_payload"]["temp"], 22.5);
}
```

---

### TC-005: RawDataPoint round-trips through serialization

**Type**: Unit
**SUT**: RawDataPoint serialization/deserialization
**Mocks**: None

```
Given: RawDataPoint instance
When: Serialized to JSON and deserialized back
Then: Original equals deserialized
```

```rust
#[test]
fn tc_005_raw_data_point_round_trips() {
    let original = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "round-trip-source".to_string(),
        ndp_id: Some("rt-001".to_string()),
        context: Some(json!({"nested": {"key": "value"}})),
        raw_payload: json!({"array": [1, 2, 3], "obj": {"a": 1}}),
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let restored: RawDataPoint = serde_json::from_str(&json_str).unwrap();

    assert_eq!(original.source_id, restored.source_id);
    assert_eq!(original.ndp_id, restored.ndp_id);
    assert_eq!(original.context, restored.context);
    assert_eq!(original.raw_payload, restored.raw_payload);
}
```

---

## Unit Tests: Source ID Generation

### TC-010: Source ID format is stream-type

**Type**: Unit
**SUT**: Source ID generation
**Mocks**: None

```
Given: Stream ID "air-quality" and source type "Mqtt"
When: Source ID is generated
Then: Result is "air-quality-Mqtt"
```

```rust
#[test]
fn tc_010_source_id_format() {
    let stream_id = "air-quality";
    let source_type = SourceType::Mqtt;

    let source_id = generate_source_id(stream_id, &source_type);

    assert_eq!(source_id, "air-quality-Mqtt");
}

#[test]
fn tc_010_source_id_http_format() {
    let stream_id = "outdoor-weather";
    let source_type = SourceType::HttpPolling;

    let source_id = generate_source_id(stream_id, &source_type);

    assert_eq!(source_id, "outdoor-weather-Http");
}
```

---

### TC-011: Source ID handles multi-source streams

**Type**: Unit
**SUT**: Source ID generation with index
**Mocks**: None

```
Given: Stream with multiple sources of same type
When: Source IDs are generated
Then: Each has unique identifier
```

```rust
#[test]
fn tc_011_source_id_multi_source() {
    let stream_id = "air-quality";
    let source_type = SourceType::Mqtt;

    let source_id_0 = generate_source_id_indexed(stream_id, &source_type, 0);
    let source_id_1 = generate_source_id_indexed(stream_id, &source_type, 1);

    assert_eq!(source_id_0, "air-quality-Mqtt-0");
    assert_eq!(source_id_1, "air-quality-Mqtt-1");
}
```

---

## Integration Tests: Source Adapters

### TC-020: HTTP source returns RawDataPoint

**Type**: Integration
**SUT**: HttpPollingSource
**Mocks**: MockServer (wiremock)

```
Given: HTTP source configured with endpoint
When: Source fetches data
Then: Returns RawDataPoint with unmodified response
```

```rust
#[tokio::test]
async fn tc_020_http_source_returns_raw_data_point() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pm25": 12.5,
            "temp": 22.3,
            "status": "online",
            "firmware": "v2.1"
        })))
        .mount(&mock_server)
        .await;

    let source = HttpPollingSource::new(
        "test-stream",
        &mock_server.uri(),
        "/api/current",
    );

    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.source_id, "test-stream-Http");
    assert_eq!(result.raw_payload["pm25"], 12.5);
    assert_eq!(result.raw_payload["status"], "online");
    assert_eq!(result.raw_payload["firmware"], "v2.1");
}
```

---

### TC-021: HTTP source preserves nested JSON

**Type**: Integration
**SUT**: HttpPollingSource
**Mocks**: MockServer

```
Given: HTTP endpoint returns nested JSON
When: Source fetches data
Then: raw_payload contains exact nested structure
```

```rust
#[tokio::test]
async fn tc_021_http_source_preserves_nested() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "main": {
                "temp": 295.15,
                "pressure": 1013
            },
            "wind": {
                "speed": 3.5,
                "deg": 180
            },
            "weather": [
                {"id": 800, "main": "Clear"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let source = create_http_source(&mock_server.uri());
    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.raw_payload["main"]["temp"], 295.15);
    assert_eq!(result.raw_payload["wind"]["speed"], 3.5);
    assert_eq!(result.raw_payload["weather"][0]["main"], "Clear");
}
```

---

### TC-022: HTTP source attaches metadata

**Type**: Integration
**SUT**: HttpPollingSource
**Mocks**: MockServer

```
Given: Source configured with ndp_id and context
When: Source fetches data
Then: RawDataPoint includes metadata
```

```rust
#[tokio::test]
async fn tc_022_http_source_attaches_metadata() {
    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"value": 1})))
        .mount(&mock_server)
        .await;

    let config = SourceConfig {
        source_type: SourceType::HttpPolling,
        ndp_id: Some("owm-home-001".into()),
        context: Some(json!({
            "provider": "openweathermap",
            "location": {"lat": 29.95, "lon": -81.31}
        })),
        ..default()
    };

    let source = HttpPollingSource::with_config(&mock_server.uri(), config);
    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.ndp_id, Some("owm-home-001".into()));
    let ctx = result.context.unwrap();
    assert_eq!(ctx["provider"], "openweathermap");
    assert_eq!(ctx["location"]["lat"], 29.95);
}
```

---

### TC-023: MQTT source returns RawDataPoint

**Type**: Integration
**SUT**: MqttSource
**Mocks**: MockMqttClient (or testcontainer)

```
Given: MQTT message received
When: Source processes message
Then: Returns RawDataPoint with raw payload
```

```rust
#[tokio::test]
async fn tc_023_mqtt_source_returns_raw_data_point() {
    let mock_mqtt = MockMqttClient::new();
    mock_mqtt.publish(
        "sensors/office/air",
        json!({"pm02": 15, "rco2": 500, "atmp": 23.5}).to_string().as_bytes(),
    );

    let source = MqttSource::new("air-quality", mock_mqtt);
    let result = source.receive_raw().await.unwrap();

    assert_eq!(result.source_id, "air-quality-Mqtt");
    assert_eq!(result.raw_payload["pm02"], 15);
    assert_eq!(result.raw_payload["rco2"], 500);
}
```

---

### TC-024: Source handles non-JSON response gracefully

**Type**: Integration
**SUT**: HttpPollingSource
**Mocks**: MockServer

```
Given: HTTP endpoint returns non-JSON (e.g., HTML error)
When: Source fetches data
Then: Returns error, not panic
```

```rust
#[tokio::test]
async fn tc_024_source_handles_non_json() {
    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200)
            .set_body_string("<html>Error</html>"))
        .mount(&mock_server)
        .await;

    let source = create_http_source(&mock_server.uri());
    let result = source.fetch_raw().await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SourceError::ParseError(_)));
}
```

---

## Integration Tests: Parquet Storage

### TC-030: ParquetStore writes RawDataPoint

**Type**: Integration
**SUT**: ParquetStore
**Mocks**: TempDir

```
Given: RawDataPoint with all fields
When: ParquetStore.write_raw() is called
Then: Parquet file created with 5 columns
```

```rust
#[tokio::test]
async fn tc_030_parquet_writes_raw_data_point() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "test-source-Http".to_string(),
        ndp_id: Some("test-001".to_string()),
        context: Some(json!({"room": "office"})),
        raw_payload: json!({"pm25": 12.5, "status": "healthy"}),
    };

    store.write_raw(point).await.unwrap();

    // Verify file exists
    let files: Vec<_> = std::fs::read_dir(temp_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension() == Some("parquet".as_ref()))
        .collect();
    assert!(!files.is_empty());
}
```

---

### TC-031: ParquetStore schema has 5 columns

**Type**: Integration
**SUT**: ParquetStore schema
**Mocks**: TempDir

```
Given: RawDataPoint written to Parquet
When: Schema is examined
Then: Contains timestamp, source_id, ndp_id, context, raw_payload
```

```rust
#[tokio::test]
async fn tc_031_parquet_schema_has_5_columns() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    store.write_raw(create_test_raw_point()).await.unwrap();

    let schema = store.get_raw_schema();
    let column_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();

    assert_eq!(column_names.len(), 5);
    assert!(column_names.contains(&"timestamp"));
    assert!(column_names.contains(&"source_id"));
    assert!(column_names.contains(&"ndp_id"));
    assert!(column_names.contains(&"context"));
    assert!(column_names.contains(&"raw_payload"));
}
```

---

### TC-032: ParquetStore reads back RawDataPoint

**Type**: Integration
**SUT**: ParquetStore read
**Mocks**: TempDir

```
Given: RawDataPoint written to Parquet
When: Query is executed
Then: Original data retrieved
```

```rust
#[tokio::test]
async fn tc_032_parquet_reads_back_raw_data_point() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let original = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "read-test-Http".to_string(),
        ndp_id: Some("read-001".to_string()),
        context: Some(json!({"test": true})),
        raw_payload: json!({"value": 42, "nested": {"a": 1}}),
    };

    store.write_raw(original.clone()).await.unwrap();

    let results = store.query_raw(
        original.timestamp - chrono::Duration::hours(1),
        original.timestamp + chrono::Duration::hours(1),
        None,
    ).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id, "read-test-Http");
    assert_eq!(results[0].raw_payload["value"], 42);
    assert_eq!(results[0].raw_payload["nested"]["a"], 1);
}
```

---

### TC-033: ParquetStore handles nullable fields

**Type**: Integration
**SUT**: ParquetStore
**Mocks**: TempDir

```
Given: RawDataPoint with ndp_id=None and context=None
When: Written and read back
Then: Null values preserved
```

```rust
#[tokio::test]
async fn tc_033_parquet_handles_nullable_fields() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "minimal-source".to_string(),
        ndp_id: None,
        context: None,
        raw_payload: json!({"data": 1}),
    };

    store.write_raw(point.clone()).await.unwrap();

    let results = store.query_raw_by_source("minimal-source").await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].ndp_id.is_none());
    assert!(results[0].context.is_none());
}
```

---

### TC-034: ParquetStore batch write

**Type**: Integration
**SUT**: ParquetStore batch operations
**Mocks**: TempDir

```
Given: Multiple RawDataPoints
When: Batch write called
Then: All points written in single file
```

```rust
#[tokio::test]
async fn tc_034_parquet_batch_write() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let points: Vec<RawDataPoint> = (0..100)
        .map(|i| RawDataPoint {
            timestamp: Utc::now() + chrono::Duration::seconds(i),
            source_id: format!("batch-source-{}", i % 3),
            ndp_id: Some(format!("batch-{}", i)),
            context: None,
            raw_payload: json!({"index": i}),
        })
        .collect();

    store.write_raw_batch(points.clone()).await.unwrap();

    let results = store.query_raw_all().await.unwrap();
    assert_eq!(results.len(), 100);
}
```

---

## Integration Tests: Pipeline

### TC-040: Pipeline routes RawDataPoint to storage

**Type**: Integration
**SUT**: Ingestion Pipeline
**Mocks**: SpyParquetStore

```
Given: Pipeline with storage configured
When: RawDataPoint submitted
Then: Storage receives the point
```

```rust
#[tokio::test]
async fn tc_040_pipeline_routes_to_storage() {
    let spy_store = SpyParquetStore::new();
    let pipeline = create_pipeline_with_raw_store(spy_store.clone());

    pipeline.start().await.unwrap();

    let point = create_test_raw_point();
    pipeline.ingest_raw(point.clone()).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let written = spy_store.get_written_raw_points();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].source_id, point.source_id);
}
```

---

### TC-041: Pipeline handles multiple sources

**Type**: Integration
**SUT**: Ingestion Pipeline
**Mocks**: SpyParquetStore, MockSources

```
Given: Pipeline with multiple source types
When: Data from each source ingested
Then: All points stored with correct source_id
```

```rust
#[tokio::test]
async fn tc_041_pipeline_multiple_sources() {
    let spy_store = SpyParquetStore::new();
    let pipeline = create_multi_source_pipeline(spy_store.clone());

    pipeline.ingest_raw(RawDataPoint {
        source_id: "stream-Mqtt".into(),
        raw_payload: json!({"from": "mqtt"}),
        ..default()
    }).await.unwrap();

    pipeline.ingest_raw(RawDataPoint {
        source_id: "stream-Http".into(),
        raw_payload: json!({"from": "http"}),
        ..default()
    }).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let written = spy_store.get_written_raw_points();
    assert_eq!(written.len(), 2);

    let source_ids: Vec<_> = written.iter().map(|p| &p.source_id).collect();
    assert!(source_ids.contains(&&"stream-Mqtt".to_string()));
    assert!(source_ids.contains(&&"stream-Http".to_string()));
}
```

---

## Acceptance Tests: End-to-End

### AT-001: Full pipeline stores raw JSON

**Type**: Acceptance (E2E)
**SUT**: Full pipeline
**Mocks**: Real components, isolated environment

```
Given: Configured source with real endpoint
When: Full pipeline runs ingestion
Then: Parquet file contains exact raw payload
```

```rust
#[tokio::test]
async fn at_001_full_pipeline_stores_raw_json() {
    let env = TestEnvironment::new().await;

    // Setup mock HTTP endpoint
    env.setup_http_source(json!({
        "pm25": 15.3,
        "co2": 580,
        "status": "healthy",
        "firmware": "v2.1.3",
        "nested": {
            "calibration": {"offset": 0.5}
        }
    }));

    // Configure and run pipeline
    let config = create_test_stream_config();
    env.run_pipeline(&config).await.unwrap();

    // Query Parquet directly
    let results = env.query_parquet(
        "SELECT source_id, raw_payload FROM bronze"
    ).await.unwrap();

    assert!(!results.is_empty());
    let payload: serde_json::Value = serde_json::from_str(&results[0]["raw_payload"]).unwrap();

    // Exact match - no transformation
    assert_eq!(payload["pm25"], 15.3);
    assert_eq!(payload["status"], "healthy");
    assert_eq!(payload["firmware"], "v2.1.3");
    assert_eq!(payload["nested"]["calibration"]["offset"], 0.5);
}
```

---

### AT-002: DuckDB can query raw_payload

**Type**: Acceptance (E2E)
**SUT**: DuckDB JSON queries
**Mocks**: Real components

```
Given: Raw data stored in Parquet
When: DuckDB JSON extraction query executed
Then: Fields extracted correctly
```

```rust
#[tokio::test]
async fn at_002_duckdb_json_extraction() {
    let env = TestEnvironment::new().await;

    // Store test data
    env.store_raw_point(RawDataPoint {
        timestamp: Utc::now(),
        source_id: "sensor-Http".into(),
        ndp_id: Some("sensor-001".into()),
        context: Some(json!({"room": "office"})),
        raw_payload: json!({
            "readings": {
                "pm25": 12.5,
                "co2": 450
            },
            "meta": {
                "version": "1.0"
            }
        }),
    }).await;

    // DuckDB JSON path extraction
    let results = env.query_duckdb(r#"
        SELECT
            source_id,
            raw_payload->>'$.readings.pm25' as pm25,
            raw_payload->>'$.readings.co2' as co2,
            raw_payload->>'$.meta.version' as version,
            context->>'$.room' as room
        FROM read_parquet('*.parquet')
    "#).await.unwrap();

    assert_eq!(results[0]["pm25"], "12.5");
    assert_eq!(results[0]["co2"], "450");
    assert_eq!(results[0]["version"], "1.0");
    assert_eq!(results[0]["room"], "office");
}
```

---

> **Note**: AT-003 (Backward Compatibility) removed - platform is <1 week old, no backward compat needed.

---

### AT-004: Non-numeric data preserved

**Type**: Acceptance (E2E)
**SUT**: Full pipeline with text/boolean data
**Mocks**: Real components

```
Given: Source returns non-numeric data (strings, booleans, nulls)
When: Full pipeline processes
Then: All values preserved in Bronze
```

```rust
#[tokio::test]
async fn at_004_non_numeric_preserved() {
    let env = TestEnvironment::new().await;

    env.setup_http_source(json!({
        "numeric": 42,
        "string": "hello world",
        "boolean": true,
        "null_value": null,
        "array": [1, "two", false],
        "object": {"nested": "value"}
    }));

    env.run_pipeline_once().await.unwrap();

    let results = env.query_duckdb(
        "SELECT raw_payload FROM read_parquet('*.parquet')"
    ).await.unwrap();

    let payload: serde_json::Value = serde_json::from_str(&results[0]["raw_payload"]).unwrap();

    assert_eq!(payload["numeric"], 42);
    assert_eq!(payload["string"], "hello world");
    assert_eq!(payload["boolean"], true);
    assert!(payload["null_value"].is_null());
    assert_eq!(payload["array"][1], "two");
    assert_eq!(payload["object"]["nested"], "value");
}
```

---

## Test Execution Matrix

| Test ID | Component | Type | Priority | Dependencies |
|---------|-----------|------|----------|--------------|
| TC-001 | RawDataPoint | Unit | P0 | None |
| TC-002 | RawDataPoint | Unit | P0 | None |
| TC-003 | RawDataPoint | Unit | P0 | None |
| TC-004 | RawDataPoint | Unit | P1 | TC-001 |
| TC-005 | RawDataPoint | Unit | P1 | TC-001 |
| TC-010 | Source ID | Unit | P0 | None |
| TC-011 | Source ID | Unit | P1 | TC-010 |
| TC-020 | HTTP Source | Integration | P0 | TC-001 |
| TC-021 | HTTP Source | Integration | P0 | TC-020 |
| TC-022 | HTTP Source | Integration | P0 | TC-020 |
| TC-023 | MQTT Source | Integration | P0 | TC-001 |
| TC-024 | Source Error | Integration | P1 | TC-020 |
| TC-030 | ParquetStore | Integration | P0 | TC-001 |
| TC-031 | ParquetStore | Integration | P0 | TC-030 |
| TC-032 | ParquetStore | Integration | P0 | TC-030 |
| TC-033 | ParquetStore | Integration | P1 | TC-030 |
| TC-034 | ParquetStore | Integration | P1 | TC-030 |
| TC-040 | Pipeline | Integration | P0 | TC-030 |
| TC-041 | Pipeline | Integration | P1 | TC-040 |
| AT-001 | Full Pipeline | Acceptance | P0 | All above |
| AT-002 | DuckDB Query | Acceptance | P0 | AT-001 |
| ~~AT-003~~ | ~~Compatibility~~ | ~~Acceptance~~ | ~~N/A~~ | Removed - no backward compat |
| AT-004 | Non-numeric | Acceptance | P0 | AT-001 |

---

## Test Data Fixtures

### Fixture: Test RawDataPoint

```rust
pub fn create_test_raw_point() -> RawDataPoint {
    RawDataPoint {
        timestamp: Utc::now(),
        source_id: "test-stream-Http".to_string(),
        ndp_id: Some("test-device-001".to_string()),
        context: Some(json!({
            "room": "office",
            "floor": 2,
            "tags": ["primary"]
        })),
        raw_payload: json!({
            "pm02": 12.5,
            "rco2": 450,
            "atmp": 22.3,
            "rhum": 45,
            "serialno": "abc123",
            "status": "online"
        }),
    }
}
```

### Fixture: Test Stream Config

```rust
pub fn create_test_stream_config() -> StreamConfig {
    StreamConfig {
        stream_id: "air-quality".into(),
        sources: vec![SourceConfig {
            source_type: SourceType::HttpPolling,
            enabled: true,
            ndp_id: Some("airgradient-office-001".into()),
            context: Some(json!({
                "room": "office",
                "device_type": "airgradient"
            })),
            ..default()
        }],
        ..default()
    }
}
```

### Fixture: HTTP Response

```rust
pub fn create_test_http_response() -> serde_json::Value {
    json!({
        "pm02": 12,
        "pm10": 18,
        "rco2": 450,
        "atmp": 22.3,
        "rhum": 45,
        "wifi": -65,
        "serialno": "d83bda1cd074",
        "firmware": "3.1.1",
        "model": "I-9PSL"
    })
}
```
