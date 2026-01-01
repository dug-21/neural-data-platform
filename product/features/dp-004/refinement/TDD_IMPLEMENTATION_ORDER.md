# DP-004: TDD Implementation Order

## Overview

This document provides the exact sequence of test-first implementation steps for Bronze Layer Raw JSON Schema. Each step follows the Red-Green-Refactor cycle.

**Approach**: Raw JSON storage as defined in ADR-001. Store exact source payloads in Bronze; move parsing to Silver ETL.

---

## London School Approach: Outside-In

```
Phase 1: Acceptance Test (RED)      <- Start here, drives discovery
    |                                  Tests full pipeline writes raw JSON
    v
Phase 2: Integration Tests (RED)    <- Discover component interfaces
    |                                  Tests Parquet schema, source adapters
    v
Phase 3: Unit Tests (RED)           <- Implement smallest units
    |                                  Tests RawDataPoint construction
    v
Phase 4: Implementation (GREEN)     <- Make tests pass bottom-up
    |                                  RawDataPoint → Sources → Storage → Pipeline
    v
Phase 5: Refactor                   <- Clean up, optimize
```

---

## Implementation Cycles

### Cycle 1: RawDataPoint Struct

**RED**: Write failing test for RawDataPoint construction

```rust
// core/src/types/raw_data_point.rs

#[test]
fn test_raw_data_point_construction() {
    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "test-source-Http".to_string(),
        ndp_id: Some("device-001".to_string()),
        context: Some(json!({"room": "office"})),
        raw_payload: json!({"pm25": 12.5, "status": "active"}),
    };

    assert_eq!(point.source_id, "test-source-Http");
    assert_eq!(point.ndp_id, Some("device-001".to_string()));
    assert_eq!(point.raw_payload["pm25"], 12.5);
    assert_eq!(point.raw_payload["status"], "active");
}
```

**GREEN**: Add RawDataPoint struct

```rust
// core/src/types/raw_data_point.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bronze layer record - raw JSON storage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDataPoint {
    /// Ingestion timestamp (when NDP received the message)
    pub timestamp: DateTime<Utc>,

    /// Source identifier (e.g., "air-quality-Http")
    pub source_id: String,

    /// Platform-assigned stable identifier (from config)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Config-derived metadata snapshot at ingestion time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    /// Exact payload from source, untransformed
    pub raw_payload: Value,
}
```

**REFACTOR**: Add Default impl and builder pattern

```rust
impl Default for RawDataPoint {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: String::new(),
            ndp_id: None,
            context: None,
            raw_payload: Value::Null,
        }
    }
}

impl RawDataPoint {
    pub fn new(source_id: impl Into<String>, raw_payload: Value) -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: source_id.into(),
            ndp_id: None,
            context: None,
            raw_payload,
        }
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn with_ndp_id(mut self, ndp_id: impl Into<String>) -> Self {
        self.ndp_id = Some(ndp_id.into());
        self
    }

    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }
}
```

---

### Cycle 2: RawDataPoint Serialization

**RED**: Write test for JSON round-trip

```rust
#[test]
fn test_raw_data_point_serialization() {
    let original = RawDataPoint {
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
        source_id: "test-Http".to_string(),
        ndp_id: Some("test-001".to_string()),
        context: Some(json!({"nested": {"key": "value"}})),
        raw_payload: json!({"array": [1, 2, 3], "bool": true}),
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let restored: RawDataPoint = serde_json::from_str(&json_str).unwrap();

    assert_eq!(original, restored);
}
```

**GREEN**: The derive macro handles this automatically.

**REFACTOR**: None needed.

---

### Cycle 3: RawDataPoint Preserves Non-Numeric Types

**RED**: Write test for type preservation

```rust
#[test]
fn test_raw_data_point_preserves_all_types() {
    let point = RawDataPoint::new("test-source", json!({
        "string": "hello",
        "number": 42,
        "float": 3.14,
        "boolean": true,
        "null": null,
        "array": [1, "two", false],
        "object": {"nested": "value"},
    }));

    assert_eq!(point.raw_payload["string"], "hello");
    assert_eq!(point.raw_payload["number"], 42);
    assert_eq!(point.raw_payload["float"], 3.14);
    assert_eq!(point.raw_payload["boolean"], true);
    assert!(point.raw_payload["null"].is_null());
    assert_eq!(point.raw_payload["array"][1], "two");
    assert_eq!(point.raw_payload["object"]["nested"], "value");
}
```

**GREEN**: Using `serde_json::Value` preserves all types automatically.

**REFACTOR**: None needed.

---

### Cycle 4: Source ID Generation

**RED**: Write test for source ID format

```rust
// core/src/sources/mod.rs

#[test]
fn test_generate_source_id() {
    let source_id = generate_source_id("air-quality", &SourceType::HttpPolling);
    assert_eq!(source_id, "air-quality-Http");

    let mqtt_id = generate_source_id("sensors", &SourceType::Mqtt);
    assert_eq!(mqtt_id, "sensors-Mqtt");
}
```

**GREEN**: Add source ID generation function

```rust
pub fn generate_source_id(stream_id: &str, source_type: &SourceType) -> String {
    let type_suffix = match source_type {
        SourceType::HttpPolling => "Http",
        SourceType::Mqtt => "Mqtt",
        SourceType::Webhook => "Webhook",
    };
    format!("{}-{}", stream_id, type_suffix)
}
```

**REFACTOR**: Add indexed variant for multi-source streams

```rust
pub fn generate_source_id_indexed(stream_id: &str, source_type: &SourceType, index: usize) -> String {
    format!("{}-{}", generate_source_id(stream_id, source_type), index)
}
```

---

### Cycle 5: HTTP Source Returns RawDataPoint

**RED**: Write test for HTTP source

```rust
// core/src/sources/http_poll.rs

#[tokio::test]
async fn test_http_source_fetch_raw() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pm25": 12.5,
            "status": "online",
            "firmware": "v2.1"
        })))
        .mount(&mock_server)
        .await;

    let source = HttpPollingSource::new_raw(
        "test-stream",
        &mock_server.uri(),
        "/api/current",
        None,  // ndp_id
        None,  // context
    );

    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.source_id, "test-stream-Http");
    assert_eq!(result.raw_payload["pm25"], 12.5);
    assert_eq!(result.raw_payload["status"], "online");
    assert!(result.ndp_id.is_none());
}
```

**GREEN**: Add `fetch_raw()` method to HttpPollingSource

```rust
impl HttpPollingSource {
    /// Fetch raw data without parsing
    pub async fn fetch_raw(&self) -> Result<RawDataPoint, SourceError> {
        let response = self.client
            .get(&self.url)
            .send()
            .await
            .map_err(|e| SourceError::HttpError(e.to_string()))?;

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SourceError::ParseError(e.to_string()))?;

        Ok(RawDataPoint {
            timestamp: Utc::now(),
            source_id: self.source_id.clone(),
            ndp_id: self.ndp_id.clone(),
            context: self.context.clone(),
            raw_payload: json,
        })
    }
}
```

**REFACTOR**: Extract common metadata injection

---

### Cycle 6: HTTP Source with Metadata

**RED**: Write test with ndp_id and context

```rust
#[tokio::test]
async fn test_http_source_with_metadata() {
    let mock_server = setup_mock_server(json!({"value": 1})).await;

    let source = HttpPollingSource::new_raw(
        "test-stream",
        &mock_server.uri(),
        "/api",
        Some("device-001".to_string()),
        Some(json!({"room": "office", "floor": 2})),
    );

    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.ndp_id, Some("device-001".to_string()));
    let ctx = result.context.unwrap();
    assert_eq!(ctx["room"], "office");
    assert_eq!(ctx["floor"], 2);
}
```

**GREEN**: Already handled in Cycle 5 implementation.

**REFACTOR**: None needed.

---

### Cycle 7: Parquet Store RawDataPoint Schema

**RED**: Write test for 5-column schema

```rust
// core/src/storage/parquet.rs

#[tokio::test]
async fn test_parquet_raw_schema_has_5_columns() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = RawDataPoint::new("test-Http", json!({"value": 1}))
        .with_ndp_id("test-001")
        .with_context(json!({"room": "lab"}));

    store.write_raw(point).await.unwrap();

    let schema = store.get_raw_schema();
    let column_names: Vec<&str> = schema.fields().iter()
        .map(|f| f.name().as_str())
        .collect();

    assert_eq!(column_names.len(), 5);
    assert!(column_names.contains(&"timestamp"));
    assert!(column_names.contains(&"source_id"));
    assert!(column_names.contains(&"ndp_id"));
    assert!(column_names.contains(&"context"));
    assert!(column_names.contains(&"raw_payload"));
}
```

**GREEN**: Add raw schema to ParquetStore

```rust
fn build_raw_schema() -> Schema {
    Schema::new(vec![
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("source_id", DataType::Utf8, false),
        Field::new("ndp_id", DataType::Utf8, true),  // nullable
        Field::new("context", DataType::Utf8, true), // JSON as string, nullable
        Field::new("raw_payload", DataType::Utf8, false), // JSON as string
    ])
}
```

**REFACTOR**: Consider using LargeUtf8 for raw_payload if needed

---

### Cycle 8: Parquet Store Write RawDataPoint

**RED**: Write test for writing and reading back

```rust
#[tokio::test]
async fn test_parquet_write_and_read_raw() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let original = RawDataPoint::new("read-test-Http", json!({"value": 42, "nested": {"a": 1}}))
        .with_ndp_id("read-001")
        .with_context(json!({"test": true}));

    store.write_raw(original.clone()).await.unwrap();

    let results = store.query_raw(
        original.timestamp - chrono::Duration::hours(1),
        original.timestamp + chrono::Duration::hours(1),
        None,
    ).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id, "read-test-Http");
    assert_eq!(results[0].ndp_id, Some("read-001".to_string()));
    assert_eq!(results[0].raw_payload["value"], 42);
    assert_eq!(results[0].raw_payload["nested"]["a"], 1);
}
```

**GREEN**: Implement `write_raw()` and `query_raw()` methods

```rust
impl ParquetStore {
    pub async fn write_raw(&self, point: RawDataPoint) -> Result<(), StorageError> {
        self.write_raw_batch(vec![point]).await
    }

    pub async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> Result<(), StorageError> {
        if points.is_empty() {
            return Ok(());
        }

        // Build arrays
        let timestamps: Vec<i64> = points.iter()
            .map(|p| p.timestamp.timestamp_millis())
            .collect();

        let source_ids: Vec<&str> = points.iter()
            .map(|p| p.source_id.as_str())
            .collect();

        let ndp_ids: Vec<Option<&str>> = points.iter()
            .map(|p| p.ndp_id.as_deref())
            .collect();

        let contexts: Vec<Option<String>> = points.iter()
            .map(|p| p.context.as_ref().map(|c| c.to_string()))
            .collect();

        let payloads: Vec<String> = points.iter()
            .map(|p| p.raw_payload.to_string())
            .collect();

        // Create record batch and write
        let batch = RecordBatch::try_new(
            Arc::new(self.raw_schema.clone()),
            vec![
                Arc::new(TimestampMillisecondArray::from(timestamps)),
                Arc::new(StringArray::from(source_ids)),
                Arc::new(StringArray::from(ndp_ids)),
                Arc::new(StringArray::from(contexts.iter().map(|o| o.as_deref()).collect::<Vec<_>>())),
                Arc::new(StringArray::from(payloads.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            ],
        )?;

        self.write_batch(batch, "raw").await
    }

    pub async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, StorageError> {
        // Read parquet files and filter
        // ... implementation
    }
}
```

**REFACTOR**: Add partitioning by date

---

### Cycle 9: Parquet Handles Nullable Fields

**RED**: Write test for null ndp_id and context

```rust
#[tokio::test]
async fn test_parquet_handles_nulls() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = RawDataPoint::new("minimal-Http", json!({"data": 1}));
    // ndp_id and context are None

    store.write_raw(point).await.unwrap();

    let results = store.query_raw_by_source("minimal-Http").await.unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].ndp_id.is_none());
    assert!(results[0].context.is_none());
    assert_eq!(results[0].raw_payload["data"], 1);
}
```

**GREEN**: Already handled with nullable columns in schema.

**REFACTOR**: None needed.

---

### Cycle 10: MQTT Source Returns RawDataPoint

**RED**: Write test for MQTT source

```rust
// core/src/sources/mqtt.rs

#[tokio::test]
async fn test_mqtt_source_receive_raw() {
    let mock_client = MockMqttClient::new();
    mock_client.queue_message("sensors/air", json!({
        "pm25": 15,
        "co2": 500,
        "status": "calibrating"
    }));

    let source = MqttSource::new_raw(
        "mqtt-stream",
        mock_client,
        Some("mqtt-001".to_string()),
        None,
    );

    let result = source.receive_raw().await.unwrap();

    assert_eq!(result.source_id, "mqtt-stream-Mqtt");
    assert_eq!(result.raw_payload["pm25"], 15);
    assert_eq!(result.raw_payload["status"], "calibrating");
}
```

**GREEN**: Add `receive_raw()` method to MqttSource

```rust
impl MqttSource {
    pub async fn receive_raw(&mut self) -> Result<RawDataPoint, SourceError> {
        let message = self.client.receive().await?;

        let json: serde_json::Value = serde_json::from_slice(&message.payload)
            .map_err(|e| SourceError::ParseError(e.to_string()))?;

        Ok(RawDataPoint {
            timestamp: Utc::now(),
            source_id: self.source_id.clone(),
            ndp_id: self.ndp_id.clone(),
            context: self.context.clone(),
            raw_payload: json,
        })
    }
}
```

**REFACTOR**: None needed.

---

### Cycle 11: Pipeline Routes RawDataPoint

**RED**: Write test for pipeline routing

```rust
// apps/air-quality-app/src/pipeline/ingestion.rs

#[tokio::test]
async fn test_pipeline_routes_raw_data() {
    let spy_store = SpyParquetStore::new();
    let pipeline = create_pipeline_with_raw_store(spy_store.clone());

    pipeline.start().await.unwrap();

    let point = RawDataPoint::new("test-Http", json!({"value": 42}))
        .with_ndp_id("test-001");

    pipeline.ingest_raw(point.clone()).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let written = spy_store.get_written_points();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].source_id, "test-Http");
    assert_eq!(written[0].raw_payload["value"], 42);
}
```

**GREEN**: Update pipeline to handle RawDataPoint

```rust
impl IngestionPipeline {
    pub async fn ingest_raw(&self, point: RawDataPoint) -> Result<(), PipelineError> {
        self.raw_sender.send(point).await
            .map_err(|_| PipelineError::ChannelClosed)?;
        Ok(())
    }

    async fn raw_writer_task(
        mut rx: mpsc::Receiver<RawDataPoint>,
        store: Arc<dyn RawDataStore>,
    ) {
        let mut batch = Vec::with_capacity(100);
        let flush_interval = Duration::from_secs(5);
        let mut last_flush = Instant::now();

        loop {
            tokio::select! {
                Some(point) = rx.recv() => {
                    batch.push(point);
                    if batch.len() >= 100 || last_flush.elapsed() >= flush_interval {
                        if let Err(e) = store.write_raw_batch(batch.drain(..).collect()).await {
                            tracing::error!("Failed to write raw batch: {}", e);
                        }
                        last_flush = Instant::now();
                    }
                }
                _ = tokio::time::sleep(flush_interval) => {
                    if !batch.is_empty() {
                        if let Err(e) = store.write_raw_batch(batch.drain(..).collect()).await {
                            tracing::error!("Failed to write raw batch: {}", e);
                        }
                        last_flush = Instant::now();
                    }
                }
            }
        }
    }
}
```

**REFACTOR**: Add graceful shutdown handling

---

### Cycle 12: End-to-End Acceptance Test

**RED**: Write full pipeline test

```rust
// tests/acceptance/test_raw_json_pipeline.rs

#[tokio::test]
async fn at_001_full_pipeline_stores_raw_json() {
    let env = TestEnvironment::new().await;

    // Setup HTTP source with complex response
    env.setup_http_source(json!({
        "pm25": 15.3,
        "co2": 580,
        "status": "healthy",
        "firmware": "v2.1.3",
        "nested": {
            "calibration": {"offset": 0.5}
        },
        "tags": ["primary", "calibrated"]
    }));

    // Configure source
    let config = SourceConfig {
        source_type: SourceType::HttpPolling,
        ndp_id: Some("test-device-001".into()),
        context: Some(json!({"room": "lab", "floor": 2})),
        ..default()
    };

    // Run full pipeline
    env.run_pipeline(&config).await.unwrap();

    // Query Bronze layer
    let results = env.query_parquet(
        "SELECT source_id, ndp_id, context, raw_payload FROM bronze"
    ).await.unwrap();

    assert!(!results.is_empty());

    // Verify raw_payload is exact match
    let payload: serde_json::Value = serde_json::from_str(&results[0]["raw_payload"]).unwrap();
    assert_eq!(payload["pm25"], 15.3);
    assert_eq!(payload["status"], "healthy");
    assert_eq!(payload["nested"]["calibration"]["offset"], 0.5);
    assert_eq!(payload["tags"][0], "primary");

    // Verify metadata
    assert_eq!(results[0]["ndp_id"], "test-device-001");
    let ctx: serde_json::Value = serde_json::from_str(&results[0]["context"]).unwrap();
    assert_eq!(ctx["room"], "lab");
}
```

**GREEN**: All previous cycles make this pass!

**REFACTOR**: None needed - integration of all components.

---

## Test Execution Summary

| Cycle | Component | Test Type | Complexity |
|-------|-----------|-----------|------------|
| 1 | RawDataPoint struct | Unit | Trivial |
| 2 | RawDataPoint serialization | Unit | Trivial (serde) |
| 3 | RawDataPoint type preservation | Unit | Trivial |
| 4 | Source ID generation | Unit | Simple |
| 5 | HTTP source fetch_raw | Integration | Medium |
| 6 | HTTP source with metadata | Integration | Simple |
| 7 | Parquet raw schema | Integration | Medium |
| 8 | Parquet write/read raw | Integration | Medium |
| 9 | Parquet nullable fields | Integration | Simple |
| 10 | MQTT source receive_raw | Integration | Medium |
| 11 | Pipeline routing | Integration | Medium |
| 12 | E2E acceptance | Acceptance | Complex |

---

## Implementation Checklist

### Unit Tests

- [ ] TC-001: RawDataPoint construction with all fields
- [ ] TC-002: RawDataPoint construction minimal fields
- [ ] TC-003: RawDataPoint preserves non-numeric types
- [ ] TC-004: RawDataPoint serializes to JSON
- [ ] TC-005: RawDataPoint round-trips through serialization
- [ ] TC-010: Source ID format (stream-type)
- [ ] TC-011: Source ID with index for multi-source

### Integration Tests

- [ ] TC-020: HTTP source returns RawDataPoint
- [ ] TC-021: HTTP source preserves nested JSON
- [ ] TC-022: HTTP source attaches metadata
- [ ] TC-023: MQTT source returns RawDataPoint
- [ ] TC-024: Source handles non-JSON response
- [ ] TC-030: ParquetStore writes RawDataPoint
- [ ] TC-031: ParquetStore schema has 5 columns
- [ ] TC-032: ParquetStore reads back RawDataPoint
- [ ] TC-033: ParquetStore handles nullable fields
- [ ] TC-034: ParquetStore batch write
- [ ] TC-040: Pipeline routes RawDataPoint to storage
- [ ] TC-041: Pipeline handles multiple sources

### Acceptance Tests

- [ ] AT-001: Full pipeline stores raw JSON
- [ ] AT-002: DuckDB can query raw_payload
- [ ] AT-004: Non-numeric data preserved

> **Note**: AT-003 (Backward Compatibility) removed - platform is <1 week old.

---

## Parallel vs Sequential Development

### Can Be Parallelized

```
[Cycle 1-3: RawDataPoint]  ─┬─►  [Cycle 7-9: Parquet]
                            │
[Cycle 4: Source ID]      ──┤
                            │
[Cycle 5-6: HTTP Source]  ──┴─►  [Cycle 11: Pipeline]
                                        │
[Cycle 10: MQTT Source]  ─────────►────┘
                                        │
                                        ▼
                              [Cycle 12: E2E]
```

### Must Be Sequential

1. RawDataPoint struct must exist before sources/storage use it
2. Source ID generation must exist before sources use it
3. Parquet schema must exist before pipeline writes
4. All above must pass before E2E acceptance test

---

## Verification Commands

After each cycle:

```bash
# Run the specific test
cargo test <test_name>

# Run all tests for the module
cargo test --package <package> <module>

# Check formatting and linting
cargo fmt --check
cargo clippy -- -D warnings
```

Full suite after all cycles:

```bash
cargo test --workspace
cargo tarpaulin --out Html  # Coverage report
```

---

## Definition of Done

DP-004 is complete when:

1. All unit tests pass (`cargo test --lib`)
2. All integration tests pass (`cargo test --test integration_*`)
3. All acceptance tests pass (`cargo test --test acceptance_*`)
4. Code coverage > 80% for new code
5. Raw JSON storage verified:
   - `RawDataPoint` struct implemented
   - 5-column Parquet schema works
   - Sources produce `RawDataPoint`
   - Non-numeric data preserved
6. DuckDB JSON extraction verified
7. Backward compatibility with old schema confirmed
8. Documentation updated
9. PR approved and merged
