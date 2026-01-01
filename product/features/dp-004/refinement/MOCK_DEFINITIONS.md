# DP-004: Mock Definitions

## Overview

This document defines the mock objects (test doubles) needed for London School TDD implementation of DP-004: Bronze Layer Raw JSON Schema. Each mock isolates the System Under Test (SUT) from its collaborators.

---

## Raw JSON Storage (ADR-001)

The mock definitions reflect the **raw JSON storage** decision:

| Component | What to Mock/Stub |
|-----------|-------------------|
| HTTP Source | MockServer returns raw JSON response |
| MQTT Source | MockMqttClient delivers raw message bytes |
| ParquetStore | SpyParquetStore captures RawDataPoint writes |
| Pipeline | MockChannel captures routed data |

**Key Simplification**: Sources produce `RawDataPoint` directly; no complex parsing mocks needed.

---

## Mock Object Categories

| Category | Type | Purpose |
|----------|------|---------|
| **Mocks** | Behavior verification | Verify specific interactions occurred |
| **Stubs** | Canned responses | Provide predictable return values |
| **Fakes** | Simplified implementation | In-memory versions of external systems |
| **Spies** | Call recording | Record calls for later assertion |

---

## MockHttpClient

### Purpose

Isolate HTTP sources from real network calls by providing canned responses.

### Interface

```rust
use async_trait::async_trait;

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &str) -> Result<HttpResponse, HttpError>;
    async fn post(&self, url: &str, body: &[u8]) -> Result<HttpResponse, HttpError>;
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: HashMap<String, String>,
}
```

### Mock Implementation

```rust
use mockall::automock;

#[automock]
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &str) -> Result<HttpResponse, HttpError>;
}

/// Create mock that returns JSON response
pub fn create_mock_http_json_response(json_body: serde_json::Value) -> MockHttpClient {
    let mut mock = MockHttpClient::new();
    let body = serde_json::to_vec(&json_body).unwrap();

    mock.expect_get()
        .returning(move |_| Ok(HttpResponse {
            status: 200,
            body: body.clone(),
            headers: HashMap::new(),
        }));

    mock
}

/// Create mock that returns error
pub fn create_mock_http_error() -> MockHttpClient {
    let mut mock = MockHttpClient::new();

    mock.expect_get()
        .returning(|_| Err(HttpError::ConnectionFailed("mock error".into())));

    mock
}

/// Create mock that returns non-JSON
pub fn create_mock_http_html_error() -> MockHttpClient {
    let mut mock = MockHttpClient::new();

    mock.expect_get()
        .returning(|_| Ok(HttpResponse {
            status: 500,
            body: b"<html><body>Error</body></html>".to_vec(),
            headers: HashMap::new(),
        }));

    mock
}
```

### Wiremock Alternative (Preferred for Integration)

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

/// Setup wiremock server with JSON response
pub async fn setup_mock_http_server(response: serde_json::Value) -> MockServer {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/current"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(response)
        )
        .mount(&mock_server)
        .await;

    mock_server
}

/// Example: Air quality sensor response
pub async fn setup_airgradient_mock() -> MockServer {
    setup_mock_http_server(json!({
        "wifi": -62,
        "serialno": "d83bda1cd074",
        "rco2": 458,
        "pm02": 14,
        "pm10": 18,
        "pm01": 11,
        "atmp": 28.7,
        "rhum": 38,
        "tvoc": 44,
        "nox": 1,
        "model": "I-9PSL",
        "firmware": "3.1.1"
    })).await
}

/// Example: OpenWeatherMap response
pub async fn setup_owm_mock() -> MockServer {
    setup_mock_http_server(json!({
        "main": {
            "temp": 295.15,
            "feels_like": 294.5,
            "pressure": 1013,
            "humidity": 65
        },
        "wind": {
            "speed": 3.5,
            "deg": 180
        },
        "weather": [
            {"id": 800, "main": "Clear", "description": "clear sky"}
        ]
    })).await
}
```

### Usage Example

```rust
#[tokio::test]
async fn test_http_source_with_mock() {
    let mock_server = setup_airgradient_mock().await;

    let source = HttpPollingSource::new(
        "air-quality",
        &mock_server.uri(),
        "/api/current",
    );

    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.source_id, "air-quality-Http");
    assert_eq!(result.raw_payload["pm02"], 14);
    assert_eq!(result.raw_payload["model"], "I-9PSL");
}
```

---

## MockMqttClient

### Purpose

Isolate MQTT sources from real broker connections by simulating message delivery.

### Interface

```rust
#[async_trait]
pub trait MqttClient: Send + Sync {
    async fn subscribe(&mut self, topic: &str) -> Result<(), MqttError>;
    async fn receive(&mut self) -> Result<MqttMessage, MqttError>;
    async fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), MqttError>;
}

pub struct MqttMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retain: bool,
}
```

### Mock Implementation

```rust
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

/// Mock MQTT client with queued messages
pub struct MockMqttClient {
    messages: Arc<Mutex<VecDeque<MqttMessage>>>,
    subscriptions: Arc<Mutex<Vec<String>>>,
}

impl MockMqttClient {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(VecDeque::new())),
            subscriptions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Queue a message for receive()
    pub fn queue_message(&self, topic: &str, payload: serde_json::Value) {
        self.messages.lock().unwrap().push_back(MqttMessage {
            topic: topic.to_string(),
            payload: serde_json::to_vec(&payload).unwrap(),
            qos: 0,
            retain: false,
        });
    }

    /// Queue raw bytes message
    pub fn queue_raw_message(&self, topic: &str, payload: &[u8]) {
        self.messages.lock().unwrap().push_back(MqttMessage {
            topic: topic.to_string(),
            payload: payload.to_vec(),
            qos: 0,
            retain: false,
        });
    }

    /// Get subscriptions for verification
    pub fn get_subscriptions(&self) -> Vec<String> {
        self.subscriptions.lock().unwrap().clone()
    }
}

#[async_trait]
impl MqttClient for MockMqttClient {
    async fn subscribe(&mut self, topic: &str) -> Result<(), MqttError> {
        self.subscriptions.lock().unwrap().push(topic.to_string());
        Ok(())
    }

    async fn receive(&mut self) -> Result<MqttMessage, MqttError> {
        self.messages
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(MqttError::NoMessage)
    }

    async fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), MqttError> {
        // Record for verification if needed
        Ok(())
    }
}
```

### Factory Functions

```rust
/// Create mock with AirGradient message
pub fn create_airgradient_mqtt_mock() -> MockMqttClient {
    let mock = MockMqttClient::new();
    mock.queue_message("airgradient/readings", json!({
        "wifi": -65,
        "serialno": "abc123",
        "pm02": 12,
        "rco2": 450,
        "atmp": 22.3,
        "rhum": 45
    }));
    mock
}

/// Create mock with multiple messages
pub fn create_multi_message_mqtt_mock() -> MockMqttClient {
    let mock = MockMqttClient::new();

    mock.queue_message("sensors/office/air", json!({"pm25": 10}));
    mock.queue_message("sensors/office/air", json!({"pm25": 11}));
    mock.queue_message("sensors/office/air", json!({"pm25": 12}));

    mock
}

/// Create mock with non-JSON message (edge case)
pub fn create_invalid_mqtt_mock() -> MockMqttClient {
    let mock = MockMqttClient::new();
    mock.queue_raw_message("sensors/broken", b"not json");
    mock
}
```

### Usage Example

```rust
#[tokio::test]
async fn test_mqtt_source_with_mock() {
    let mock_client = create_airgradient_mqtt_mock();

    let source = MqttSource::new("air-quality", mock_client);

    let result = source.receive_raw().await.unwrap();

    assert_eq!(result.source_id, "air-quality-Mqtt");
    assert_eq!(result.raw_payload["pm02"], 12);
    assert_eq!(result.raw_payload["serialno"], "abc123");
}
```

---

## SpyParquetStore

### Purpose

Capture writes to Bronze layer for verification without actual file I/O.

### Interface

```rust
#[async_trait]
pub trait RawDataStore: Send + Sync {
    async fn write_raw(&self, point: RawDataPoint) -> Result<(), StorageError>;
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> Result<(), StorageError>;
    async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, StorageError>;
}
```

### Spy Implementation

```rust
use std::sync::{Arc, Mutex};

pub struct SpyParquetStore {
    written_points: Arc<Mutex<Vec<RawDataPoint>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl SpyParquetStore {
    pub fn new() -> Self {
        Self {
            written_points: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    /// Get all points that were written
    pub fn get_written_points(&self) -> Vec<RawDataPoint> {
        self.written_points.lock().unwrap().clone()
    }

    /// Set to return error on next write
    pub fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }

    /// Assert specific source_id was written
    pub fn assert_source_written(&self, source_id: &str) {
        let points = self.written_points.lock().unwrap();
        assert!(
            points.iter().any(|p| p.source_id == source_id),
            "Expected point with source_id={} to be written",
            source_id
        );
    }

    /// Assert raw_payload contains field
    pub fn assert_payload_contains(&self, field: &str, expected: serde_json::Value) {
        let points = self.written_points.lock().unwrap();
        let found = points.iter().any(|p| p.raw_payload.get(field) == Some(&expected));
        assert!(found, "Expected payload field {}={}", field, expected);
    }

    /// Assert ndp_id was written
    pub fn assert_ndp_id_written(&self, ndp_id: &str) {
        let points = self.written_points.lock().unwrap();
        assert!(
            points.iter().any(|p| p.ndp_id.as_deref() == Some(ndp_id)),
            "Expected point with ndp_id={} to be written",
            ndp_id
        );
    }

    /// Assert context contains field
    pub fn assert_context_contains(&self, path: &str, expected: &str) {
        let points = self.written_points.lock().unwrap();
        let found = points.iter().any(|p| {
            p.context.as_ref().map_or(false, |ctx| {
                ctx.pointer(path)
                    .map_or(false, |v| v.as_str() == Some(expected))
            })
        });
        assert!(found, "Expected context to contain {}={}", path, expected);
    }

    /// Get count of written points
    pub fn get_write_count(&self) -> usize {
        self.written_points.lock().unwrap().len()
    }

    /// Clear written points
    pub fn clear(&self) {
        self.written_points.lock().unwrap().clear();
    }
}

#[async_trait]
impl RawDataStore for SpyParquetStore {
    async fn write_raw(&self, point: RawDataPoint) -> Result<(), StorageError> {
        if *self.should_fail.lock().unwrap() {
            return Err(StorageError::WriteError("mock failure".into()));
        }
        self.written_points.lock().unwrap().push(point);
        Ok(())
    }

    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> Result<(), StorageError> {
        if *self.should_fail.lock().unwrap() {
            return Err(StorageError::WriteError("mock failure".into()));
        }
        self.written_points.lock().unwrap().extend(points);
        Ok(())
    }

    async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, StorageError> {
        let points = self.written_points.lock().unwrap();
        Ok(points
            .iter()
            .filter(|p| p.timestamp >= start && p.timestamp <= end)
            .filter(|p| source_filter.map_or(true, |s| p.source_id == s))
            .cloned()
            .collect())
    }
}
```

### Usage Example

```rust
#[tokio::test]
async fn test_pipeline_writes_to_storage() {
    let spy_store = SpyParquetStore::new();
    let pipeline = create_pipeline_with_store(spy_store.clone());

    pipeline.start().await.unwrap();

    // Simulate source data
    pipeline.ingest_raw(RawDataPoint {
        timestamp: Utc::now(),
        source_id: "test-Http".into(),
        ndp_id: Some("device-001".into()),
        context: Some(json!({"room": "lab"})),
        raw_payload: json!({"pm25": 12.5}),
    }).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify writes
    spy_store.assert_source_written("test-Http");
    spy_store.assert_ndp_id_written("device-001");
    spy_store.assert_payload_contains("pm25", json!(12.5));
    spy_store.assert_context_contains("/room", "lab");
}
```

---

## MockChannel

### Purpose

Capture messages sent through pipeline channels for verification.

### Implementation

```rust
use tokio::sync::mpsc;

pub struct MockChannelReceiver {
    received: Arc<Mutex<Vec<RawDataPoint>>>,
    rx: mpsc::Receiver<RawDataPoint>,
}

pub struct MockChannelSender {
    tx: mpsc::Sender<RawDataPoint>,
}

pub fn create_mock_channel(buffer: usize) -> (MockChannelSender, MockChannelReceiver) {
    let (tx, rx) = mpsc::channel(buffer);
    (
        MockChannelSender { tx },
        MockChannelReceiver {
            received: Arc::new(Mutex::new(Vec::new())),
            rx,
        },
    )
}

impl MockChannelReceiver {
    /// Collect all received messages
    pub async fn collect_all(&mut self, timeout: Duration) -> Vec<RawDataPoint> {
        let mut collected = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            tokio::select! {
                Some(point) = self.rx.recv() => {
                    collected.push(point);
                }
                _ = tokio::time::sleep_until(deadline) => {
                    break;
                }
            }
        }

        collected
    }

    /// Assert specific message was received
    pub async fn assert_received(&mut self, source_id: &str, timeout: Duration) {
        let messages = self.collect_all(timeout).await;
        assert!(
            messages.iter().any(|m| m.source_id == source_id),
            "Expected message from source_id={}",
            source_id
        );
    }
}

impl MockChannelSender {
    pub async fn send(&self, point: RawDataPoint) -> Result<(), mpsc::error::SendError<RawDataPoint>> {
        self.tx.send(point).await
    }
}
```

---

## StubSourceConfig

### Purpose

Provide predictable source configuration for testing.

### Implementation

```rust
pub struct StubSourceConfig {
    pub source_type: SourceType,
    pub source_id: String,
    pub ndp_id: Option<String>,
    pub context: Option<serde_json::Value>,
    pub enabled: bool,
}

impl StubSourceConfig {
    pub fn http(stream_id: &str) -> Self {
        Self {
            source_type: SourceType::HttpPolling,
            source_id: format!("{}-Http", stream_id),
            ndp_id: None,
            context: None,
            enabled: true,
        }
    }

    pub fn mqtt(stream_id: &str) -> Self {
        Self {
            source_type: SourceType::Mqtt,
            source_id: format!("{}-Mqtt", stream_id),
            ndp_id: None,
            context: None,
            enabled: true,
        }
    }

    pub fn with_ndp_id(mut self, ndp_id: &str) -> Self {
        self.ndp_id = Some(ndp_id.to_string());
        self
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn to_source_config(&self) -> SourceConfig {
        SourceConfig {
            source_type: self.source_type.clone(),
            enabled: self.enabled,
            ndp_id: self.ndp_id.clone(),
            context: self.context.clone(),
            ..Default::default()
        }
    }
}
```

### Usage Example

```rust
#[test]
fn test_source_config_stub() {
    let config = StubSourceConfig::http("air-quality")
        .with_ndp_id("airgradient-001")
        .with_context(json!({
            "room": "office",
            "device_type": "airgradient"
        }))
        .to_source_config();

    assert_eq!(config.ndp_id, Some("airgradient-001".into()));
    assert_eq!(config.context.unwrap()["room"], "office");
}
```

---

## Test Fixtures Module

Centralized test data creation:

```rust
// tests/fixtures/mod.rs

pub mod raw_data {
    use super::*;

    /// Minimal RawDataPoint
    pub fn minimal() -> RawDataPoint {
        RawDataPoint {
            timestamp: Utc::now(),
            source_id: "test-source".to_string(),
            ndp_id: None,
            context: None,
            raw_payload: json!({"value": 1}),
        }
    }

    /// Full RawDataPoint (air quality)
    pub fn air_quality() -> RawDataPoint {
        RawDataPoint {
            timestamp: Utc::now(),
            source_id: "air-quality-Http".to_string(),
            ndp_id: Some("airgradient-office-001".to_string()),
            context: Some(json!({
                "room": "office",
                "floor": 2,
                "device_type": "airgradient"
            })),
            raw_payload: json!({
                "pm02": 12,
                "pm10": 18,
                "rco2": 450,
                "atmp": 22.3,
                "rhum": 45,
                "wifi": -65,
                "serialno": "d83bda1cd074",
                "firmware": "3.1.1",
                "model": "I-9PSL"
            }),
        }
    }

    /// Full RawDataPoint (weather)
    pub fn weather() -> RawDataPoint {
        RawDataPoint {
            timestamp: Utc::now(),
            source_id: "outdoor-weather-Http".to_string(),
            ndp_id: Some("owm-home-001".to_string()),
            context: Some(json!({
                "provider": "openweathermap",
                "location": {"lat": 29.95, "lon": -81.31}
            })),
            raw_payload: json!({
                "main": {
                    "temp": 295.15,
                    "feels_like": 294.5,
                    "pressure": 1013,
                    "humidity": 65
                },
                "wind": {"speed": 3.5, "deg": 180},
                "weather": [{"id": 800, "main": "Clear"}]
            }),
        }
    }

    /// RawDataPoint with non-numeric values
    pub fn with_non_numeric() -> RawDataPoint {
        RawDataPoint {
            timestamp: Utc::now(),
            source_id: "status-source".to_string(),
            ndp_id: Some("device-001".to_string()),
            context: None,
            raw_payload: json!({
                "status": "online",
                "connected": true,
                "error": null,
                "tags": ["primary", "calibrated"],
                "meta": {"version": "1.0.0"}
            }),
        }
    }
}

pub mod http_responses {
    use super::*;

    pub fn airgradient() -> serde_json::Value {
        json!({
            "wifi": -62,
            "serialno": "d83bda1cd074",
            "rco2": 458,
            "pm02": 14,
            "pm10": 18,
            "atmp": 28.7,
            "rhum": 38,
            "model": "I-9PSL",
            "firmware": "3.1.1"
        })
    }

    pub fn openweathermap() -> serde_json::Value {
        json!({
            "main": {"temp": 295.15, "humidity": 65},
            "wind": {"speed": 3.5},
            "weather": [{"main": "Clear"}]
        })
    }

    pub fn nws_observation() -> serde_json::Value {
        json!({
            "properties": {
                "timestamp": "2026-01-01T12:00:00Z",
                "temperature": {"value": 22.5, "unitCode": "wmoUnit:degC"},
                "relativeHumidity": {"value": 65}
            }
        })
    }
}

pub mod source_configs {
    use super::*;

    pub fn air_quality_http() -> SourceConfig {
        StubSourceConfig::http("air-quality")
            .with_ndp_id("airgradient-office-001")
            .with_context(json!({"room": "office"}))
            .to_source_config()
    }

    pub fn weather_http() -> SourceConfig {
        StubSourceConfig::http("outdoor-weather")
            .with_ndp_id("owm-home-001")
            .with_context(json!({"provider": "openweathermap"}))
            .to_source_config()
    }

    pub fn mqtt_sensor() -> SourceConfig {
        StubSourceConfig::mqtt("mqtt-sensors")
            .with_ndp_id("mqtt-sensor-001")
            .to_source_config()
    }
}
```

---

## Mock Verification Helpers

```rust
pub mod assertions {
    use super::*;

    /// Assert RawDataPoint has expected source_id
    pub fn assert_source_id(point: &RawDataPoint, expected: &str) {
        assert_eq!(
            point.source_id, expected,
            "Expected source_id={}, got {}",
            expected, point.source_id
        );
    }

    /// Assert RawDataPoint has expected ndp_id
    pub fn assert_ndp_id(point: &RawDataPoint, expected: &str) {
        assert_eq!(
            point.ndp_id.as_deref(),
            Some(expected),
            "Expected ndp_id={}, got {:?}",
            expected, point.ndp_id
        );
    }

    /// Assert raw_payload contains field with value
    pub fn assert_payload_field(point: &RawDataPoint, field: &str, expected: serde_json::Value) {
        let actual = point.raw_payload.get(field);
        assert_eq!(
            actual,
            Some(&expected),
            "Expected payload[{}]={}, got {:?}",
            field, expected, actual
        );
    }

    /// Assert raw_payload contains nested field
    pub fn assert_payload_path(point: &RawDataPoint, path: &str, expected: serde_json::Value) {
        let actual = point.raw_payload.pointer(path);
        assert_eq!(
            actual,
            Some(&expected),
            "Expected payload at {}={}, got {:?}",
            path, expected, actual
        );
    }

    /// Assert context contains field
    pub fn assert_context_field(point: &RawDataPoint, field: &str, expected: &str) {
        let ctx = point.context.as_ref().expect("Expected context to be present");
        let actual = ctx.get(field).and_then(|v| v.as_str());
        assert_eq!(
            actual,
            Some(expected),
            "Expected context[{}]={}, got {:?}",
            field, expected, actual
        );
    }

    /// Assert all non-numeric types preserved
    pub fn assert_preserves_types(point: &RawDataPoint) {
        let payload = &point.raw_payload;

        // Check that JSON value types are preserved correctly
        for (key, value) in payload.as_object().unwrap_or(&serde_json::Map::new()) {
            match value {
                serde_json::Value::String(_) => {}
                serde_json::Value::Number(_) => {}
                serde_json::Value::Bool(_) => {}
                serde_json::Value::Null => {}
                serde_json::Value::Array(_) => {}
                serde_json::Value::Object(_) => {}
            }
        }
    }
}
```

---

## Integration with Real Components

For true end-to-end tests, use `tempfile` for filesystem isolation:

```rust
use tempfile::TempDir;

pub struct TestEnvironment {
    temp_dir: TempDir,
    store: ParquetStore,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).unwrap();

        Self { temp_dir, store }
    }

    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    pub async fn write_raw(&self, point: RawDataPoint) -> Result<(), StorageError> {
        self.store.write_raw(point).await
    }

    pub async fn query_parquet(&self, sql: &str) -> Result<Vec<serde_json::Value>, QueryError> {
        // Use DuckDB to query Parquet files
        let conn = duckdb::Connection::open_in_memory()?;
        let results = conn.prepare(sql)?.query_map([], |row| {
            // Convert to JSON...
        })?;
        Ok(results.collect())
    }
}
```
