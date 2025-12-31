# AIR-009: Mock Definitions

## Overview

This document defines the mock objects (test doubles) needed for London School TDD implementation of AIR-009. Each mock isolates the System Under Test (SUT) from its collaborators.

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

The mock definitions reflect the **simple blob storage** decision:

| Component | What to Mock/Stub |
|-----------|-------------------|
| Parser | Produces records with `ndp_id` + `context` JSON string |
| ParquetWriter | Captures `ndp_id` + `context` columns |
| TimescaleClient | Verifies `ndp_id` TEXT + `context` JSONB |

**Key Simplification**: No `ProcessedContext`, no promoted fields, just simple JSON serialization.

---

## Mock Object Categories

| Category | Type | Purpose |
|----------|------|---------|
| **Mocks** | Behavior verification | Verify specific interactions occurred |
| **Stubs** | Canned responses | Provide predictable return values |
| **Fakes** | Simplified implementation | In-memory versions of external systems |
| **Spies** | Call recording | Record calls for later assertion |

---

## MockEtcdClient

### Purpose
Isolate ConfigSyncService and ConfigClient from real etcd cluster.

### Interface

```rust
use mockall::automock;
use async_trait::async_trait;

#[automock]
#[async_trait]
pub trait EtcdOperations {
    /// Put a key-value pair
    async fn put(&self, key: String, value: Vec<u8>) -> Result<(), EtcdError>;

    /// Get a value by key
    async fn get(&self, key: String) -> Result<Option<Vec<u8>>, EtcdError>;

    /// Get all keys with prefix
    async fn get_prefix(&self, prefix: String) -> Result<Vec<(String, Vec<u8>)>, EtcdError>;

    /// Delete a key
    async fn delete(&self, key: String) -> Result<(), EtcdError>;

    /// Watch for changes
    async fn watch(&self, prefix: String) -> Result<WatchStream, EtcdError>;
}
```

### Mock Implementation

```rust
use mockall::predicate::*;

pub fn create_mock_etcd_for_sync() -> MockEtcdOperations {
    let mut mock = MockEtcdOperations::new();

    // Expect ndp_id to be written
    mock.expect_put()
        .withf(|key, _| key.contains("/ndp_id"))
        .times(1)
        .returning(|_, _| Ok(()));

    // Expect context blob to be written (single key, not flattened)
    mock.expect_put()
        .withf(|key, val| {
            key.contains("/context") &&
            // Verify it's a JSON blob
            String::from_utf8_lossy(val).contains("{")
        })
        .times(1)
        .returning(|_, _| Ok(()));

    mock
}

pub fn create_mock_etcd_for_read(ndp_id: &str, context: serde_json::Value) -> MockEtcdOperations {
    let mut mock = MockEtcdOperations::new();

    let ndp_id_value = serde_json::to_vec(&ndp_id).unwrap();
    let context_value = serde_json::to_vec(&context).unwrap();

    mock.expect_get()
        .withf(|key| key.contains("/ndp_id"))
        .returning(move |_| Ok(Some(ndp_id_value.clone())));

    mock.expect_get()
        .withf(|key| key.contains("/context"))
        .returning(move |_| Ok(Some(context_value.clone())));

    mock
}
```

### Usage Example

```rust
#[tokio::test]
async fn test_config_sync_writes_ndp_id() {
    let mock = create_mock_etcd_for_sync();

    let sync_service = ConfigSyncService::new(mock);
    let config = create_test_config_with_ndp_id("test-001");

    sync_service.sync(config).await.unwrap();

    // mockall automatically verifies expectations on drop
}
```

---

## MockConfigStore

### Purpose
Provide stubbed configuration data for testing components that read configs.

### Interface

```rust
#[automock]
#[async_trait]
pub trait ConfigStore {
    /// Get stream configuration
    async fn get_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;

    /// List all stream IDs
    async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;

    /// Get source configuration by ndp_id
    async fn get_source_by_ndp_id(&self, ndp_id: &str) -> Result<SourceConfig, ConfigError>;
}
```

### Stub Factory

```rust
pub struct StubConfigStore {
    streams: HashMap<String, StreamConfig>,
}

impl StubConfigStore {
    pub fn new() -> Self {
        Self { streams: HashMap::new() }
    }

    pub fn with_stream(mut self, config: StreamConfig) -> Self {
        self.streams.insert(config.stream_id.clone(), config);
        self
    }

    /// Create stub with standard test fixtures
    pub fn with_test_fixtures() -> Self {
        Self::new()
            .with_stream(create_air_quality_stream())
            .with_stream(create_weather_stream())
    }
}

#[async_trait]
impl ConfigStore for StubConfigStore {
    async fn get_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError> {
        self.streams.get(stream_id)
            .cloned()
            .ok_or(ConfigError::NotFound(stream_id.into()))
    }

    async fn list_streams(&self) -> Result<Vec<String>, ConfigError> {
        Ok(self.streams.keys().cloned().collect())
    }

    async fn get_source_by_ndp_id(&self, ndp_id: &str) -> Result<SourceConfig, ConfigError> {
        for stream in self.streams.values() {
            for source in &stream.sources {
                if source.ndp_id.as_deref() == Some(ndp_id) {
                    return Ok(source.clone());
                }
            }
        }
        Err(ConfigError::NotFound(ndp_id.into()))
    }
}
```

### Usage Example

```rust
#[test]
fn test_lookup_source_by_ndp_id() {
    let store = StubConfigStore::with_test_fixtures();

    let source = store.get_source_by_ndp_id("airgradient-office-001")
        .await
        .unwrap();

    assert_eq!(source.ndp_id, Some("airgradient-office-001".into()));
    assert!(source.context.is_some());
}
```

---

## MockParquetWriter

### Purpose
Capture writes to Bronze layer for verification without actual file I/O.

### Interface

```rust
#[automock]
#[async_trait]
pub trait ParquetWriter {
    /// Write a single point
    async fn write(&self, point: TimeSeriesPoint) -> Result<(), StorageError>;

    /// Write a batch of points
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<(), StorageError>;
}
```

### Spy Implementation (Simple Blob Approach)

```rust
use std::sync::{Arc, Mutex};

pub struct SpyParquetWriter {
    written_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
}

impl SpyParquetWriter {
    pub fn new() -> Self {
        Self {
            written_points: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all points that were written
    pub fn get_written_points(&self) -> Vec<TimeSeriesPoint> {
        self.written_points.lock().unwrap().clone()
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

    /// Assert context blob was written and contains field
    pub fn assert_context_contains(&self, json_path: &str, expected_value: &str) {
        let points = self.written_points.lock().unwrap();
        let found = points.iter().any(|p| {
            if let Some(ref ctx) = p.context {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(ctx) {
                    if let Some(val) = parsed.pointer(json_path) {
                        return val.as_str() == Some(expected_value) ||
                               val.to_string().contains(expected_value);
                    }
                }
            }
            false
        });
        assert!(found, "Expected context to contain {}={}", json_path, expected_value);
    }

    /// Assert context blob was written with full structure
    pub fn assert_context_written(&self) {
        let points = self.written_points.lock().unwrap();
        assert!(
            points.iter().any(|p| p.context.is_some()),
            "Expected at least one point with context to be written"
        );
    }
}

#[async_trait]
impl ParquetWriter for SpyParquetWriter {
    async fn write(&self, point: TimeSeriesPoint) -> Result<(), StorageError> {
        self.written_points.lock().unwrap().push(point);
        Ok(())
    }

    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<(), StorageError> {
        self.written_points.lock().unwrap().extend(points);
        Ok(())
    }
}
```

### Usage Example (Simple Blob Verification)

```rust
#[tokio::test]
async fn test_pipeline_writes_context_blob_to_bronze() {
    let spy_writer = SpyParquetWriter::new();

    let pipeline = IngestPipeline::new(
        stub_config_store(),
        spy_writer.clone(),
    );

    pipeline.process(test_mqtt_message()).await.unwrap();

    // Verify ndp_id written
    spy_writer.assert_ndp_id_written("airgradient-office-001");

    // Verify context blob written with all fields
    spy_writer.assert_context_written();
    spy_writer.assert_context_contains("/device_type", "airgradient");
    spy_writer.assert_context_contains("/location/type", "indoor");
    spy_writer.assert_context_contains("/model", "ONE-V9");
}
```

---

## MockTimescaleClient

### Purpose
Verify SQL statements sent to TimescaleDB without real database.

### Interface

```rust
#[automock]
#[async_trait]
pub trait TimescaleOperations {
    /// Execute a SQL statement
    async fn execute(&self, sql: &str) -> Result<u64, DbError>;

    /// Execute a query and return rows
    async fn query(&self, sql: &str) -> Result<Vec<Row>, DbError>;

    /// Execute with parameters
    async fn execute_params(&self, sql: &str, params: &[&dyn ToSql]) -> Result<u64, DbError>;

    /// Insert a record
    async fn insert(&self, table: &str, record: &Record) -> Result<(), DbError>;
}
```

### Mock for Migration Testing (Simple Blob Schema)

```rust
pub fn create_mock_for_simple_blob_migration() -> MockTimescaleOperations {
    let mut mock = MockTimescaleOperations::new();

    // Expect ndp_id column creation
    mock.expect_execute()
        .withf(|sql| {
            sql.to_uppercase().contains("ALTER TABLE") &&
            sql.to_uppercase().contains("NDP_ID") &&
            sql.to_uppercase().contains("TEXT")
        })
        .times(1)
        .returning(|_| Ok(0));

    // Expect ndp_id index creation
    mock.expect_execute()
        .withf(|sql| {
            sql.to_uppercase().contains("CREATE INDEX") &&
            sql.contains("ndp_id")
        })
        .times(1)
        .returning(|_| Ok(0));

    // Expect context JSONB column
    mock.expect_execute()
        .withf(|sql| {
            sql.to_uppercase().contains("CONTEXT") &&
            sql.to_uppercase().contains("JSONB")
        })
        .times(1)
        .returning(|_| Ok(0));

    // Expect GIN index on JSONB context
    mock.expect_execute()
        .withf(|sql| {
            sql.to_uppercase().contains("CREATE INDEX") &&
            sql.to_uppercase().contains("GIN") &&
            sql.contains("context")
        })
        .times(1)
        .returning(|_| Ok(0));

    mock
}
```

### Mock for Insert Testing

```rust
pub fn create_mock_for_insert() -> MockTimescaleOperations {
    let mut mock = MockTimescaleOperations::new();

    mock.expect_insert()
        .withf(|table, record| {
            table == "sensor_readings" &&
            record.has_field("ndp_id") &&
            record.has_field("context")
        })
        .times(1..)
        .returning(|_, _| Ok(()));

    mock
}
```

### Mock for Query Testing

```rust
pub fn create_mock_for_query(expected_rows: Vec<Row>) -> MockTimescaleOperations {
    let mut mock = MockTimescaleOperations::new();

    mock.expect_query()
        .withf(|sql| sql.contains("ndp_id"))
        .returning(move |_| Ok(expected_rows.clone()));

    mock
}
```

### Usage Example

```rust
#[tokio::test]
async fn test_etl_inserts_with_ndp_id_and_context() {
    let mock = create_mock_for_insert();

    let etl = BronzeToSilverETL::new(mock);

    let bronze_record = create_test_bronze_record();
    etl.transform_and_load(bronze_record).await.unwrap();

    // mockall verifies expectations on drop
}
```

---

## MockContextProvider (Simple Blob Approach)

### Purpose
Provide context for parser testing without full config stack.

### Interface

```rust
pub trait ContextProvider {
    /// Get ndp_id for current source
    fn ndp_id(&self) -> Option<&str>;

    /// Get context as JSON string (simple blob)
    fn context(&self) -> Option<&str>;
}
```

### Stub Implementation

```rust
pub struct StubContextProvider {
    ndp_id: Option<String>,
    context: Option<String>,
}

impl StubContextProvider {
    pub fn new() -> Self {
        Self {
            ndp_id: None,
            context: None,
        }
    }

    pub fn with_ndp_id(mut self, id: &str) -> Self {
        self.ndp_id = Some(id.into());
        self
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(serde_json::to_string(&context).unwrap());
        self
    }
}

impl ContextProvider for StubContextProvider {
    fn ndp_id(&self) -> Option<&str> {
        self.ndp_id.as_deref()
    }

    fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }
}
```

### Builder Pattern

```rust
pub struct ContextProviderBuilder {
    provider: StubContextProvider,
}

impl ContextProviderBuilder {
    pub fn new() -> Self {
        Self { provider: StubContextProvider::new() }
    }

    pub fn ndp_id(mut self, id: &str) -> Self {
        self.provider = self.provider.with_ndp_id(id);
        self
    }

    pub fn location_indoor(self, path: &str) -> Self {
        self.context(json!({
            "location": {
                "type": "indoor",
                "path": path
            }
        }))
    }

    pub fn location_outdoor(self, coords: [f64; 2]) -> Self {
        self.context(json!({
            "location": {
                "type": "outdoor",
                "coordinates": coords
            }
        }))
    }

    pub fn with_device_info(self, device_type: &str, model: &str) -> Self {
        // Merge with existing context
        let current = self.provider.context
            .as_ref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or(json!({}));

        let mut merged = current.as_object().unwrap().clone();
        merged.insert("device_type".into(), json!(device_type));
        merged.insert("model".into(), json!(model));

        self.context(serde_json::Value::Object(merged))
    }

    pub fn context(mut self, ctx: serde_json::Value) -> Self {
        self.provider = self.provider.with_context(ctx);
        self
    }

    pub fn build(self) -> StubContextProvider {
        self.provider
    }
}
```

### Usage Example

```rust
#[test]
fn test_parser_uses_context_provider() {
    let context = ContextProviderBuilder::new()
        .ndp_id("airgradient-office-001")
        .location_indoor("home/upstairs/office")
        .with_device_info("airgradient", "ONE-V9")
        .build();

    let parser = FlatJsonParser::with_context(context);
    let record = parser.parse(test_payload()).unwrap();

    // ndp_id attached
    assert_eq!(record.ndp_id, Some("airgradient-office-001".into()));

    // Full context as JSON blob
    let ctx: serde_json::Value = serde_json::from_str(&record.context.unwrap()).unwrap();
    assert_eq!(ctx["location"]["type"], "indoor");
    assert_eq!(ctx["device_type"], "airgradient");
    assert_eq!(ctx["model"], "ONE-V9");
}
```

---

## FakeEtcdClient

### Purpose
In-memory etcd for integration tests requiring round-trip behavior.

### Implementation

```rust
use std::collections::BTreeMap;
use std::sync::RwLock;

pub struct FakeEtcdClient {
    data: RwLock<BTreeMap<String, Vec<u8>>>,
}

impl FakeEtcdClient {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(BTreeMap::new()),
        }
    }

    /// Seed initial data for testing
    pub fn seed(&self, key: &str, value: &[u8]) {
        self.data.write().unwrap().insert(key.into(), value.to_vec());
    }

    /// Dump all data for debugging
    pub fn dump(&self) -> BTreeMap<String, Vec<u8>> {
        self.data.read().unwrap().clone()
    }
}

#[async_trait]
impl EtcdOperations for FakeEtcdClient {
    async fn put(&self, key: String, value: Vec<u8>) -> Result<(), EtcdError> {
        self.data.write().unwrap().insert(key, value);
        Ok(())
    }

    async fn get(&self, key: String) -> Result<Option<Vec<u8>>, EtcdError> {
        Ok(self.data.read().unwrap().get(&key).cloned())
    }

    async fn get_prefix(&self, prefix: String) -> Result<Vec<(String, Vec<u8>)>, EtcdError> {
        let data = self.data.read().unwrap();
        Ok(data
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    async fn delete(&self, key: String) -> Result<(), EtcdError> {
        self.data.write().unwrap().remove(&key);
        Ok(())
    }

    async fn watch(&self, _prefix: String) -> Result<WatchStream, EtcdError> {
        // Return a dummy watch stream for testing
        Ok(WatchStream::empty())
    }
}
```

### Usage Example

```rust
#[tokio::test]
async fn test_config_round_trip() {
    let fake = FakeEtcdClient::new();

    // Sync config
    let sync = ConfigSyncService::new(fake.clone());
    sync.sync(test_config()).await.unwrap();

    // Read back
    let client = ConfigClient::new(fake);
    let retrieved = client.get_stream("air-quality").await.unwrap();

    assert_eq!(
        retrieved.sources[0].ndp_id,
        Some("airgradient-office-001".into())
    );
}
```

---

## Test Fixtures Module (Simple Blob Approach)

Centralized test data creation:

```rust
// tests/fixtures/mod.rs

pub mod configs {
    pub fn minimal_source_config() -> SourceConfig {
        SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        }
    }

    pub fn source_with_ndp_id(id: &str) -> SourceConfig {
        SourceConfig {
            ndp_id: Some(id.into()),
            ..minimal_source_config()
        }
    }

    /// Full context as JSON blob
    pub fn source_with_full_context() -> SourceConfig {
        SourceConfig {
            ndp_id: Some("airgradient-office-001".into()),
            context: Some(json!({
                "location": {
                    "coordinates": [29.958, -81.308],
                    "type": "indoor",
                    "path": "home/upstairs/office"
                },
                "device_type": "airgradient",
                "model": "ONE-V9",
                "tags": ["primary", "calibrated"]
            })),
            ..minimal_source_config()
        }
    }
}

pub mod payloads {
    pub fn mqtt_air_quality() -> Vec<u8> {
        serde_json::to_vec(&json!({
            "serial_number": "d83bda1cd074",
            "pm25": 12.5,
            "pm10": 18.2,
            "temperature": 22.3,
            "humidity": 45.0,
            "co2": 650
        })).unwrap()
    }
}

pub mod points {
    /// Create point with ndp_id
    pub fn with_ndp_id(id: &str) -> TimeSeriesPoint {
        let mut point = create_basic_point();
        point.ndp_id = Some(id.into());
        point
    }

    /// Create point with ndp_id and context blob
    pub fn with_context(ndp_id: &str, context: serde_json::Value) -> TimeSeriesPoint {
        let mut point = with_ndp_id(ndp_id);
        point.context = Some(serde_json::to_string(&context).unwrap());
        point
    }

    /// Create point with full context blob
    pub fn with_full_context(ndp_id: &str) -> TimeSeriesPoint {
        with_context(ndp_id, json!({
            "location": {
                "type": "indoor",
                "path": "home/office",
                "coordinates": [29.958, -81.308]
            },
            "device_type": "airgradient",
            "model": "ONE-V9"
        }))
    }
}
```

---

## Mock Verification Helpers (Simple Blob Approach)

```rust
pub mod assertions {
    /// Assert point has expected ndp_id
    pub fn assert_has_ndp_id(point: &TimeSeriesPoint, expected: &str) {
        assert_eq!(
            point.ndp_id.as_deref(),
            Some(expected),
            "Expected ndp_id={}, got {:?}",
            expected,
            point.ndp_id
        );
    }

    /// Assert point has context blob
    pub fn assert_has_context(point: &TimeSeriesPoint) {
        assert!(
            point.context.is_some(),
            "Expected context to be present"
        );
    }

    /// Assert context blob contains expected field
    pub fn assert_context_has(point: &TimeSeriesPoint, json_path: &str, expected: &str) {
        let ctx = point.context.as_ref()
            .expect("Expected context to be present");
        let parsed: serde_json::Value = serde_json::from_str(ctx)
            .expect("context should be valid JSON");
        let actual = parsed.pointer(json_path)
            .expect(&format!("Expected path {} to exist in context", json_path));
        assert!(
            actual.as_str() == Some(expected) || actual.to_string().contains(expected),
            "Expected context{} = {}, got {}",
            json_path, expected, actual
        );
    }

    /// Assert SQL contains expected clauses
    pub fn assert_sql_contains(sql: &str, clauses: &[&str]) {
        for clause in clauses {
            assert!(
                sql.to_uppercase().contains(&clause.to_uppercase()),
                "Expected SQL to contain '{}', got: {}",
                clause, sql
            );
        }
    }

    /// Assert SQL creates simple blob schema (ndp_id + context JSONB)
    pub fn assert_sql_creates_simple_blob_schema(sql: &str) {
        assert_sql_contains(sql, &[
            "ndp_id",
            "context",
            "JSONB"
        ]);
    }
}
```
