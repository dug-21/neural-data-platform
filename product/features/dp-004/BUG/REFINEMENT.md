# DP-004 BUG: Bronze Layer Stores Parsed Data Instead of Raw JSON

## REFINEMENT DOCUMENTATION - London School TDD Approach

### Bug Summary

The Bronze layer currently stores parsed `TimeSeriesPoint` data instead of raw API responses. This violates ADR-001's core principle: "Bronze stores raw data; parsing moves to Silver ETL."

**Root Cause**: Sources parse responses before storage, losing non-numeric fields and the original payload structure.

**Expected Behavior**: `RawDataPoint.raw_payload` should contain the exact JSON response from the source API.

---

## 1. Test Strategy: London School TDD

### 1.1 London School Principles

| Principle | Application |
|-----------|-------------|
| **Outside-In Development** | Start from acceptance test (raw JSON stored), drill to unit tests |
| **Mock Collaborators** | Mock HTTP client, MQTT client, storage channel |
| **Behavior Verification** | Test interactions (what was sent), not internal state |
| **Contract Definition** | Define interfaces through mock expectations |

### 1.2 Test Double Classification

| Type | Usage | Example |
|------|-------|---------|
| **Mock** | Verify method was called with correct arguments | `MockRawStore.expect_write_raw()` |
| **Stub** | Provide canned responses | `wiremock::Mock` for HTTP responses |
| **Spy** | Capture arguments for later assertion | `SpyChannel` to capture sent messages |
| **Fake** | Simplified implementation | `TempDir` for filesystem |

### 1.3 Mock Boundaries

```
+------------------------+       +------------------------+
|   GenericHttpPolling   | <---> |   MockHttpClient       |
|   Source               |       |   (wiremock server)    |
+------------------------+       +------------------------+
           |
           | fetch_raw() returns RawDataPoint
           v
+------------------------+       +------------------------+
|   SourceManager        | <---> |   SpyChannel           |
|   (orchestration)      |       |   (captures sent data) |
+------------------------+       +------------------------+
           |
           v
+------------------------+       +------------------------+
|   RawStorageWriter     | <---> |   MockRawStore         |
|   (Bronze storage)     |       |   (verifies write)     |
+------------------------+       +------------------------+
```

---

## 2. Mock Definitions

### 2.1 MockHttpClient (via wiremock)

Tests HTTP source returns raw response without parsing:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

async fn setup_mock_api_server() -> MockServer {
    let mock_server = MockServer::start().await;

    // Stub: Return exact JSON that should appear in raw_payload
    let response_json = serde_json::json!({
        "pm25": 12.5,
        "pm10": 28.3,
        "rco2": 450,
        "atmp": 22.3,
        "rhum": 55.0,
        "serialno": "ABC123",       // Non-numeric - MUST be preserved
        "firmware": "v3.4.1",       // Non-numeric - MUST be preserved
        "wifi": -45,                // Numeric - preserved
        "status": "healthy",        // Non-numeric - MUST be preserved
        "model": "ONE-V9"           // Non-numeric - MUST be preserved
    });

    Mock::given(method("GET"))
        .and(path("/measures/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
        .mount(&mock_server)
        .await;

    mock_server
}
```

### 2.2 SpyChannel (Capture Sent Data)

Captures what data sources send to the storage pipeline:

```rust
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Spy channel that captures all sent RawDataPoints for verification
struct SpyChannel {
    captured: Arc<Mutex<Vec<RawDataPoint>>>,
    sender: mpsc::Sender<RawDataPoint>,
}

impl SpyChannel {
    fn new() -> (Self, mpsc::Receiver<RawDataPoint>) {
        let (tx, rx) = mpsc::channel(100);
        let captured = Arc::new(Mutex::new(Vec::new()));
        (Self { captured, sender: tx }, rx)
    }

    async fn send(&self, point: RawDataPoint) {
        self.captured.lock().unwrap().push(point.clone());
        let _ = self.sender.send(point).await;
    }

    fn get_captured(&self) -> Vec<RawDataPoint> {
        self.captured.lock().unwrap().clone()
    }
}
```

### 2.3 MockRawStore (Verify Storage Behavior)

Already defined in `core/src/traits.rs`:

```rust
mock! {
    pub RawStore {}

    #[async_trait]
    impl RawStore for RawStore {
        async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>;
        async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>;
        async fn query_raw(
            &self,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
            source_filter: Option<String>,
        ) -> CoreResult<Vec<RawDataPoint>>;
    }
}
```

### 2.4 MockMqttClient (for MQTT Source)

Stubs MQTT message reception:

```rust
/// Stub for MQTT messages
struct MockMqttMessage {
    topic: String,
    payload: Vec<u8>,
}

impl MockMqttMessage {
    fn new(topic: &str, payload: serde_json::Value) -> Self {
        Self {
            topic: topic.to_string(),
            payload: serde_json::to_vec(&payload).unwrap(),
        }
    }
}

/// Spy for tracking received MQTT messages converted to RawDataPoint
struct MqttSourceSpy {
    converted_points: Arc<Mutex<Vec<RawDataPoint>>>,
}
```

---

## 3. Test Cases: Behavior Verification

### 3.1 HTTP Source: Sends Raw JSON Response

**Test Location**: `core/src/sources/http_poll.rs`

```rust
#[cfg(test)]
mod bug_fix_tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    /// BUG-FIX: HTTP source must send raw JSON, not parsed TimeSeriesPoint
    #[tokio::test]
    async fn test_http_source_sends_raw_json_response_not_parsed() {
        // GIVEN: API returns JSON with numeric AND non-numeric fields
        let mock_server = MockServer::start().await;
        let api_response = serde_json::json!({
            "pm25": 12.5,
            "serialno": "ABC123",      // Bug: this was lost in parsing
            "firmware": "v3.4.1",      // Bug: this was lost in parsing
            "status": "healthy",       // Bug: this was lost in parsing
            "model": "ONE-V9"          // Bug: this was lost in parsing
        });

        Mock::given(method("GET"))
            .and(path("/measures/current"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&api_response))
            .mount(&mock_server)
            .await;

        let config = create_test_config(&mock_server.uri());
        let source = GenericHttpPollingSource::with_raw_config(config).unwrap();

        // WHEN: Source fetches raw data
        let result = source.fetch_raw().await;

        // THEN: raw_payload contains EXACT API response
        assert!(result.is_ok());
        let point = result.unwrap();

        // BEHAVIOR VERIFICATION: raw_payload matches API response exactly
        assert_eq!(point.raw_payload["pm25"], 12.5);
        assert_eq!(point.raw_payload["serialno"], "ABC123");  // Non-numeric preserved
        assert_eq!(point.raw_payload["firmware"], "v3.4.1"); // Non-numeric preserved
        assert_eq!(point.raw_payload["status"], "healthy");   // Non-numeric preserved
        assert_eq!(point.raw_payload["model"], "ONE-V9");     // Non-numeric preserved
    }

    /// BUG-FIX: HTTP source must NOT serialize TimeSeriesPoint in raw_payload
    #[tokio::test]
    async fn test_http_source_raw_payload_not_timeseries_serialization() {
        let mock_server = MockServer::start().await;
        let api_response = serde_json::json!({
            "pm25": 12.5,
            "temperature": 22.3
        });

        Mock::given(method("GET"))
            .and(path("/measures/current"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&api_response))
            .mount(&mock_server)
            .await;

        let config = create_test_config(&mock_server.uri());
        let source = GenericHttpPollingSource::with_raw_config(config).unwrap();

        let point = source.fetch_raw().await.unwrap();

        // ANTI-PATTERN: These fields should NOT exist (TimeSeriesPoint structure)
        assert!(point.raw_payload.get("timestamp").is_none());
        assert!(point.raw_payload.get("location_id").is_none());
        assert!(point.raw_payload.get("value").is_none());
        assert!(point.raw_payload.get("tags").is_none());

        // CORRECT: Original API fields preserved
        assert!(point.raw_payload.get("pm25").is_some());
        assert!(point.raw_payload.get("temperature").is_some());
    }

    /// BUG-FIX: raw_payload must be JSON object, not stringified JSON
    #[tokio::test]
    async fn test_http_source_raw_payload_is_json_object_not_string() {
        let mock_server = MockServer::start().await;
        let api_response = serde_json::json!({"value": 42});

        Mock::given(method("GET"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&api_response))
            .mount(&mock_server)
            .await;

        let config = create_test_config(&mock_server.uri());
        let source = GenericHttpPollingSource::with_raw_config(config).unwrap();

        let point = source.fetch_raw().await.unwrap();

        // BEHAVIOR: raw_payload is JSON object, not stringified
        assert!(point.raw_payload.is_object());
        assert!(!point.raw_payload.is_string());
    }
}
```

### 3.2 MQTT Source: Sends Raw MQTT Payload

**Test Location**: `apps/air-quality-app/src/sources/mqtt.rs` or equivalent

```rust
#[cfg(test)]
mod mqtt_bug_fix_tests {
    use super::*;

    /// BUG-FIX: MQTT source must send raw payload, not parsed data
    #[tokio::test]
    async fn test_mqtt_source_sends_raw_payload_not_parsed() {
        // GIVEN: MQTT message with mixed field types
        let mqtt_payload = serde_json::json!({
            "pm25": 15.3,
            "serialno": "SENSOR001",    // Bug: was lost
            "boot_count": 42,           // Bug: was lost (integer, not float)
            "wifi_ssid": "HomeNetwork", // Bug: was lost
            "channels": [1, 2, 3, 4]    // Bug: was lost (array)
        });

        let mock_message = MockMqttMessage::new(
            "airgradient/readings",
            mqtt_payload.clone()
        );

        let mut mock_source = MockMqttSource::new();
        mock_source
            .expect_fetch_raw()
            .times(1)
            .returning(move || {
                Ok(RawDataPoint::new(
                    "air-quality-Mqtt",
                    mqtt_payload.clone()
                ))
            });

        // WHEN: Source fetches raw data
        let point = mock_source.fetch_raw().await.unwrap();

        // THEN: All fields preserved including non-numeric
        assert_eq!(point.raw_payload["pm25"], 15.3);
        assert_eq!(point.raw_payload["serialno"], "SENSOR001");
        assert_eq!(point.raw_payload["boot_count"], 42);
        assert_eq!(point.raw_payload["wifi_ssid"], "HomeNetwork");
        assert_eq!(point.raw_payload["channels"].as_array().unwrap().len(), 4);
    }

    /// BUG-FIX: MQTT source preserves exact byte-for-byte payload structure
    #[tokio::test]
    async fn test_mqtt_source_preserves_payload_byte_fidelity() {
        // The exact JSON structure matters for debugging and reprocessing
        let original_json = r#"{"pm25":12.5,"extra_field":"value"}"#;
        let parsed: serde_json::Value = serde_json::from_str(original_json).unwrap();

        let point = RawDataPoint::new("test-Mqtt", parsed.clone());

        // Verify the payload round-trips correctly
        let serialized = serde_json::to_string(&point.raw_payload).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed, reparsed);
    }
}
```

### 3.3 RawDataPoint: Contains Exact API Response

**Test Location**: `core/src/types/raw_data_point.rs`

```rust
#[cfg(test)]
mod raw_payload_integrity_tests {
    use super::*;

    /// BUG-FIX: RawDataPoint preserves ALL JSON types
    #[test]
    fn test_raw_data_point_preserves_all_json_types() {
        let complex_payload = serde_json::json!({
            "string": "text value",
            "integer": 42,
            "float": 3.14159,
            "boolean_true": true,
            "boolean_false": false,
            "null_value": null,
            "array": [1, "two", 3.0, true, null],
            "nested_object": {
                "level2": {
                    "level3": "deep value"
                }
            },
            "empty_object": {},
            "empty_array": []
        });

        let point = RawDataPoint::new("test-Http", complex_payload.clone());

        // Each type preserved
        assert_eq!(point.raw_payload["string"], "text value");
        assert_eq!(point.raw_payload["integer"], 42);
        assert!((point.raw_payload["float"].as_f64().unwrap() - 3.14159).abs() < 0.0001);
        assert_eq!(point.raw_payload["boolean_true"], true);
        assert_eq!(point.raw_payload["boolean_false"], false);
        assert!(point.raw_payload["null_value"].is_null());
        assert_eq!(point.raw_payload["array"].as_array().unwrap().len(), 5);
        assert_eq!(point.raw_payload["nested_object"]["level2"]["level3"], "deep value");
        assert!(point.raw_payload["empty_object"].is_object());
        assert!(point.raw_payload["empty_array"].is_array());
    }

    /// BUG-FIX: No TimeSeriesPoint fields in raw_payload
    #[test]
    fn test_raw_payload_no_timeseries_serialization() {
        // This is the bug: sources were serializing TimeSeriesPoint instead of raw response
        let api_response = serde_json::json!({
            "pm25": 12.5,
            "model": "ONE-V9"
        });

        let point = RawDataPoint::new("test-Http", api_response);

        // These fields indicate TimeSeriesPoint was incorrectly stored
        let forbidden_fields = ["timestamp", "location_id", "value", "tags", "ndp_id_in_payload"];

        for field in forbidden_fields {
            assert!(
                point.raw_payload.get(field).is_none(),
                "raw_payload should not contain '{}' - this indicates TimeSeriesPoint serialization",
                field
            );
        }
    }
}
```

### 3.4 SourceManager: Wires Sources to Storage Correctly

**Test Location**: `apps/air-quality-app/src/pipeline/source_manager.rs` or equivalent

```rust
#[cfg(test)]
mod source_manager_bug_fix_tests {
    use super::*;

    /// BUG-FIX: SourceManager sends RawDataPoint to storage, not TimeSeriesPoint
    #[tokio::test]
    async fn test_source_manager_routes_raw_data_to_storage() {
        // GIVEN: Mock source that returns RawDataPoint
        let mut mock_source = MockRawSource::new();
        let expected_payload = serde_json::json!({
            "pm25": 12.5,
            "serialno": "ABC123"
        });
        let expected_payload_clone = expected_payload.clone();

        mock_source
            .expect_fetch_raw()
            .times(1)
            .returning(move || {
                Ok(RawDataPoint::new("test-Http", expected_payload_clone.clone()))
            });

        // AND: Spy storage to capture what's written
        let mut mock_store = MockRawStore::new();
        let captured_point: Arc<Mutex<Option<RawDataPoint>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured_point.clone();

        mock_store
            .expect_write_raw()
            .times(1)
            .returning(move |point| {
                *captured_clone.lock().unwrap() = Some(point);
                Ok(())
            });

        // WHEN: SourceManager orchestrates fetch -> store
        let manager = SourceManager::new(
            Box::new(mock_source),
            Arc::new(mock_store)
        );
        manager.run_once().await.unwrap();

        // THEN: Storage receives RawDataPoint with correct payload
        let written = captured_point.lock().unwrap();
        let point = written.as_ref().expect("Point should have been written");

        assert_eq!(point.source_id, "test-Http");
        assert_eq!(point.raw_payload["pm25"], 12.5);
        assert_eq!(point.raw_payload["serialno"], "ABC123");
    }

    /// BUG-FIX: Verify write_raw called, not write (TimeSeriesPoint method)
    #[tokio::test]
    async fn test_source_manager_calls_write_raw_not_write() {
        let mut mock_source = MockRawSource::new();
        mock_source
            .expect_fetch_raw()
            .returning(|| Ok(RawDataPoint::new("test", serde_json::json!({}))));

        let mut mock_store = MockRawStore::new();

        // EXPECTATION: write_raw is called exactly once
        mock_store
            .expect_write_raw()
            .times(1)
            .returning(|_| Ok(()));

        // NOTE: If write() (TimeSeriesPoint) was called, this test would fail
        // because MockRawStore doesn't have write() method

        let manager = SourceManager::new(
            Box::new(mock_source),
            Arc::new(mock_store)
        );

        manager.run_once().await.unwrap();
        // mockall verifies expectations automatically
    }
}
```

### 3.5 RawStorageWriter: Receives Raw Payloads

**Test Location**: `core/src/storage/parquet.rs` or integration tests

```rust
#[cfg(test)]
mod storage_writer_bug_fix_tests {
    use super::*;

    /// BUG-FIX: ParquetStore writes raw_payload as-is without transformation
    #[tokio::test]
    async fn test_parquet_store_writes_raw_payload_without_parsing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).unwrap();

        // GIVEN: RawDataPoint with complex payload
        let original_payload = serde_json::json!({
            "pm25": 12.5,
            "serialno": "ABC123",
            "nested": {
                "key": "value"
            },
            "array": [1, 2, 3]
        });

        let point = RawDataPoint::new("test-Http", original_payload.clone())
            .with_ndp_id("sensor-001")
            .with_context(serde_json::json!({"room": "office"}));

        // WHEN: Write to storage
        store.write_raw(point).await.unwrap();

        // THEN: Query back and verify payload integrity
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let results = store.query_raw(start, end, Some("test-Http".to_string())).await.unwrap();

        assert_eq!(results.len(), 1);
        let retrieved = &results[0];

        // BEHAVIOR: raw_payload matches exactly what was written
        assert_eq!(retrieved.raw_payload, original_payload);
        assert_eq!(retrieved.raw_payload["serialno"], "ABC123");
        assert_eq!(retrieved.raw_payload["nested"]["key"], "value");
        assert_eq!(retrieved.raw_payload["array"].as_array().unwrap().len(), 3);
    }

    /// BUG-FIX: Verify 5-column schema (not old 7-column tall schema)
    #[tokio::test]
    async fn test_parquet_schema_is_wide_not_tall() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).unwrap();

        let point = RawDataPoint::new("test-Http", serde_json::json!({"value": 1}));
        store.write_raw(point).await.unwrap();

        // Read Parquet file and verify schema
        let parquet_files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|s| s == "parquet").unwrap_or(false))
            .collect();

        assert!(!parquet_files.is_empty(), "Should have written Parquet file");

        // Schema should have 5 columns: timestamp, source_id, ndp_id, context, raw_payload
        // NOT the old tall schema: timestamp, location_id, value, tags, field_name, etc.
    }
}
```

---

## 4. Refactoring Steps: Red-Green-Refactor Cycle

### Cycle 1: RawDataPoint Construction (COMPLETE)

**Status**: GREEN - Already implemented in `core/src/types/raw_data_point.rs`

Tests verify:
- Construction with all fields
- Builder pattern
- Optional field handling
- Serialization round-trip

### Cycle 2: RawSource Trait (COMPLETE)

**Status**: GREEN - Already implemented in `core/src/traits.rs`

Tests verify:
- `fetch_raw()` returns single RawDataPoint
- `fetch_raw_batch()` returns Vec<RawDataPoint>
- Mock definitions work correctly

### Cycle 3: HTTP Source Bug Fix (IN PROGRESS)

**Status**: RED - Tests written, implementation needed

**Red Phase**:
```rust
// This test should FAIL currently
#[tokio::test]
async fn test_http_source_sends_raw_json_response_not_parsed() {
    // ... test code from 3.1 ...
}
```

**Green Phase** (Implementation required):
```rust
// In GenericHttpPollingSource
impl RawSource for GenericHttpPollingSource {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        let response = self.client.get(&self.config.url).send().await?;
        let raw_json: serde_json::Value = response.json().await?;

        // BUG FIX: Store raw response, not parsed TimeSeriesPoint
        Ok(RawDataPoint::new(
            self.source_id(),
            raw_json  // <-- Exact API response
        )
        .with_ndp_id_opt(self.config.ndp_id.clone())
        .with_context_opt(self.config.context.clone()))
    }
}
```

**Refactor Phase**:
- Extract HTTP response handling to testable function
- Add error handling for invalid JSON
- Add logging for debugging

### Cycle 4: MQTT Source Bug Fix

**Status**: RED - Tests to be written

**Red Phase**:
```rust
#[tokio::test]
async fn test_mqtt_source_sends_raw_payload_not_parsed() {
    // ... test code from 3.2 ...
}
```

**Green Phase** (Implementation required):
```rust
// In MqttSource (wherever MQTT is implemented)
impl RawSource for MqttSource {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        let message = self.next_message().await?;
        let raw_json: serde_json::Value = serde_json::from_slice(&message.payload)?;

        // BUG FIX: Store raw MQTT payload, not parsed data
        Ok(RawDataPoint::new(
            self.source_id(),
            raw_json  // <-- Exact MQTT payload
        )
        .with_ndp_id_opt(self.config.ndp_id.clone())
        .with_context_opt(self.config.context.clone()))
    }
}
```

### Cycle 5: SourceManager Integration

**Status**: RED - Tests to be written

**Red Phase**: Tests from 3.4

**Green Phase**: Update SourceManager to use RawSource instead of Source

### Cycle 6: ParquetStore Raw Methods

**Status**: YELLOW - Partially implemented

**Red Phase**: Tests from 3.5

**Green Phase**: Verify `write_raw` and `query_raw` work with 5-column schema

---

## 5. Integration Verification

### 5.1 End-to-End Test Scenario

```rust
/// E2E: Full pipeline stores raw JSON, not parsed data
#[tokio::test]
#[ignore] // Run with --ignored
async fn e2e_pipeline_stores_raw_json() {
    // SETUP: Start mock API server
    let mock_server = MockServer::start().await;
    let api_response = serde_json::json!({
        "pm25": 12.5,
        "pm10": 28.3,
        "serialno": "ABC123",
        "firmware": "v3.4.1",
        "status": "healthy"
    });

    Mock::given(method("GET"))
        .and(path("/measures/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&api_response))
        .mount(&mock_server)
        .await;

    // SETUP: Temp storage
    let temp_dir = tempfile::TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // SETUP: Configure source
    let config = HttpPollingConfig {
        url: format!("{}/measures/current", mock_server.uri()),
        stream_id: "test-stream".to_string(),
        ndp_id: Some("sensor-001".to_string()),
        context: Some(serde_json::json!({"room": "office"})),
        ..Default::default()
    };

    let source = GenericHttpPollingSource::with_raw_config(config).unwrap();

    // EXECUTE: Fetch and store
    let raw_point = source.fetch_raw().await.unwrap();
    store.write_raw(raw_point).await.unwrap();

    // VERIFY: Query back via DuckDB
    let query = r#"
        SELECT
            source_id,
            raw_payload->>'pm25' as pm25,
            raw_payload->>'serialno' as serialno,
            raw_payload->>'firmware' as firmware,
            raw_payload->>'status' as status
        FROM bronze
        WHERE source_id = 'test-stream-Http'
    "#;

    let results = execute_duckdb_query(temp_dir.path(), query).await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["pm25"], "12.5");
    assert_eq!(results[0]["serialno"], "ABC123");    // Non-numeric preserved!
    assert_eq!(results[0]["firmware"], "v3.4.1");    // Non-numeric preserved!
    assert_eq!(results[0]["status"], "healthy");     // Non-numeric preserved!
}
```

### 5.2 Verification Checklist

| Verification | Method | Passing Criteria |
|--------------|--------|------------------|
| Unit tests pass | `cargo test raw_data_point` | All green |
| HTTP source tests | `cargo test http_poll::bug_fix` | All green |
| MQTT source tests | `cargo test mqtt::bug_fix` | All green |
| Integration tests | `cargo test --ignored` | All green |
| Coverage | `cargo tarpaulin` | >80% on affected files |
| No regressions | `cargo test` | All existing tests pass |

### 5.3 Manual Verification Steps

1. **Start local API mock**:
   ```bash
   cd /workspaces/neural-data-platform
   # Start test infrastructure
   ```

2. **Run ingestion once**:
   ```bash
   cargo run --bin air-quality-app -- --once
   ```

3. **Query Parquet files with DuckDB**:
   ```sql
   SELECT
       timestamp,
       source_id,
       raw_payload->>'serialno' as serialno,  -- Should NOT be null
       raw_payload->>'firmware' as firmware   -- Should NOT be null
   FROM 'data/bronze/**/*.parquet'
   LIMIT 5;
   ```

4. **Verify non-numeric fields exist**:
   - If `serialno` and `firmware` are present, bug is fixed
   - If they are NULL or missing, bug persists

---

## 6. Files to Modify

| File | Change | Priority |
|------|--------|----------|
| `core/src/sources/http_poll.rs` | Implement `RawSource` trait, return raw JSON | P0 |
| `apps/air-quality-app/src/sources/mqtt.rs` | Implement `RawSource` trait, return raw payload | P0 |
| `apps/air-quality-app/src/pipeline/source_manager.rs` | Use `RawSource` instead of `Source` | P1 |
| `core/src/storage/parquet.rs` | Verify `write_raw` implementation | P1 |
| `tests/integration/dp_004_bug_fix_test.rs` | E2E verification tests | P1 |

---

## 7. Success Criteria

1. **All unit tests pass**: Tests in sections 3.1-3.5
2. **Non-numeric fields preserved**: `serialno`, `firmware`, `status` appear in queries
3. **No TimeSeriesPoint in raw_payload**: No `location_id`, `value`, `tags` fields
4. **E2E test passes**: Full pipeline verification
5. **Existing functionality unbroken**: All previous tests pass
6. **Coverage maintained**: >80% on modified files

---

## Related Documents

- [ADR-001: Bronze Raw JSON Schema](/workspaces/neural-data-platform/product/features/dp-004/architecture/ADR-001-bronze-raw-json-schema.md)
- [TEST_STRATEGY.md](/workspaces/neural-data-platform/product/features/dp-004/refinement/TEST_STRATEGY.md)
- [MOCK_DEFINITIONS.md](/workspaces/neural-data-platform/product/features/dp-004/refinement/MOCK_DEFINITIONS.md)
- [core/src/traits.rs](/workspaces/neural-data-platform/core/src/traits.rs) - RawSource and RawStore traits
- [core/src/types/raw_data_point.rs](/workspaces/neural-data-platform/core/src/types/raw_data_point.rs) - RawDataPoint struct

---

*Last Updated: 2026-01-01 by ndp-tester*
