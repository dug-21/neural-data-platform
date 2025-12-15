# AIR-004 Multi-Stream Data Platform - Test Strategy

## Document Information

- **Feature ID**: AIR-004
- **Version**: 1.0.0
- **Status**: Test Strategy - London School TDD Approach
- **Created**: 2025-12-15
- **Author**: SPARC Testing Agent
- **Testing Philosophy**: London School TDD (Mock-Driven, Behavior-Focused)

---

## Executive Summary

This document outlines the comprehensive test strategy for AIR-004, ensuring 90% minimum test coverage while maintaining backward compatibility with existing AIR-002/AIR-003 functionality. The strategy uses **London School TDD** (mock-driven development) to test behavior through collaborator interactions rather than implementation details.

### Critical Constraints

1. **MUST NOT break existing tests** - All current tests continue to pass
2. **Target Coverage**: 90% minimum for new code
3. **Test-First Approach**: Tests written BEFORE implementation
4. **Mock Dependencies**: Use mockall crate for all external collaborators
5. **Fast Execution**: Unit tests < 100ms, integration tests < 5s

---

## 1. Existing Test Baseline

### 1.1 Current Test Coverage (AIR-002/AIR-003)

**Passing Tests (MUST REMAIN GREEN)**:

#### `/apps/air-quality-app/tests/integration_test.rs` (786 lines)
- ✅ **ParquetStore Integration Tests**:
  - `test_parquet_write_and_query` - Basic write/read operations
  - `test_data_persistence_after_restart` - WAL replay verification
  - `test_storage_health_check` - Health endpoint validation
  - `test_multi_location_partitioning` - Multiple location support
  - `test_batch_write_performance` - 1000 points batch test

- ✅ **WAL (Write-Ahead Log) Tests**:
  - `test_wal_replay_correctness` - Data survives restart
  - `test_wal_replay_empty` - Empty WAL handling

- ✅ **Aggregation Tests**:
  - `test_aggregation_mean`, `test_aggregation_sum`, `test_aggregation_max`, `test_aggregation_min`

- ✅ **Time Range Filtering**:
  - `test_time_range_exact_boundaries` - Boundary condition testing
  - `test_time_range_no_data` - Empty result handling
  - `test_time_range_cross_day_boundaries` - Multi-day partitioning

- ✅ **Edge Cases**:
  - `test_invalid_empty_location_id`, `test_invalid_nan_values`, `test_invalid_infinity_values`
  - `test_invalid_reversed_time_range`, `test_invalid_empty_batch`

- ✅ **Concurrent Access**:
  - `test_concurrent_writes_different_locations` - 5 parallel writes
  - `test_concurrent_reads_same_location` - 10 parallel reads

- ✅ **Stress Tests**:
  - `test_air002_batch_size` - 100 points (AIR-002 batch size)
  - `test_multiple_sequential_batches` - 10 batches × 100 points

#### `/apps/air-quality-app/tests/etcd_config_test.rs` (98 lines)
- ✅ `test_load_config_from_etcd` - etcd configuration loading (requires etcd)
- ✅ `test_env_override` - Environment variable precedence
- ✅ `test_watch_config_changes` - Hot-reload via watch

#### `/apps/air-quality-app/tests/config_hierarchy_test.rs` (238 lines)
- ✅ Configuration hierarchy validation:
  - Priority order: etcd > env vars > config.yaml > defaults
  - DATA_DIR precedence over STORAGE_PATH
  - MQTT environment overrides
  - Path format validation

#### `/apps/air-quality-app/src/api/routes.rs` (505 lines, ~300 lines of tests)
- ✅ **API Endpoint Tests** (inline `#[cfg(test)]` module):
  - Health check endpoints
  - Readings endpoints
  - Alerts endpoints
  - Locations endpoints
  - Forecast endpoints

#### `/config-client/tests/integration_test.rs`
- ✅ ConfigClient library tests (etcd operations)

### 1.2 Test Execution Baseline

**Known Issues**:
- Build fails on memory-constrained environments (linker killed with signal 9)
- Solution: Run tests selectively or increase available memory

**Baseline Command**:
```bash
# Full test suite (requires sufficient memory)
cargo test --package air-quality-app --all-features

# Selective testing (for memory-constrained environments)
cargo test --package air-quality-app --lib
cargo test --package air-quality-app --test integration_test
```

---

## 2. AIR-004 Test Architecture

### 2.1 London School TDD Principles

**Core Philosophy**:
- Test **behavior** through collaborator interactions, not implementation
- Mock all dependencies to test in isolation
- Focus on **what** components do, not **how** they do it

**Example Pattern**:
```rust
// London TDD: Test behavior through mocked dependencies
#[test]
fn test_stream_registry_notifies_coordinator_on_new_stream() {
    // Arrange: Mock dependencies
    let mut mock_client = MockConfigClient::new();
    let mut mock_coordinator = MockIngestionCoordinator::new();

    // Expect: Behavior verification
    mock_client.expect_watch()
        .times(1)
        .returning(|_| Ok(watch_handle));

    mock_coordinator.expect_spawn_sources()
        .with(eq("weather"))
        .times(1)
        .returning(|_| Ok(()));

    // Act: Execute behavior
    let registry = StreamRegistry::new(mock_client);
    registry.register_stream("weather", stream_config);

    // Assert: Verify interactions happened
}
```

**vs. Classic TDD (Not Used Here)**:
```rust
// Classic TDD: Test internal state
#[test]
fn test_stream_registry_adds_to_map() {
    let registry = StreamRegistry::new_real();
    registry.register_stream("weather", config);

    // Direct state inspection
    assert_eq!(registry.streams.len(), 1);
    assert!(registry.streams.contains_key("weather"));
}
```

### 2.2 Test Pyramid for AIR-004

```
         /\
        /E2E\      <- 5% - Full pipeline (MQTT → Storage)
       /------\
      /Integr.\   <- 15% - Component integration (real dependencies)
     /----------\
    /   Unit     \ <- 80% - London TDD mocked unit tests
   /--------------\
```

**Distribution**:
- **Unit Tests (80%)**: All new components with mocked dependencies
- **Integration Tests (15%)**: Cross-component interactions (e.g., StreamRegistry + ConfigClient)
- **E2E Tests (5%)**: Full multi-stream pipeline validation

---

## 3. Test Plan for New AIR-004 Components

### 3.1 StreamRegistry (London TDD)

**Component**: `/apps/air-quality-app/src/registry/stream_registry.rs`

**Dependencies to Mock**:
```rust
trait ConfigClient {
    async fn get<T>(&self, key: &str) -> Result<T>;
    async fn put<T>(&self, key: &str, value: &T) -> Result<()>;
    async fn watch(&self, prefix: &str) -> Result<WatchHandle>;
    async fn get_keys_under(&self, prefix: &str) -> Result<Vec<String>>;
}

trait IngestionCoordinator {
    async fn spawn_sources(&mut self, stream_id: &str, sources: Vec<SourceConfig>) -> Result<()>;
    async fn stop_sources(&mut self, stream_id: &str) -> Result<()>;
}
```

**Test Cases** (using mockall):

```rust
// File: apps/air-quality-app/tests/unit/stream_registry_test.rs

use mockall::predicate::*;
use mockall::mock;

mock! {
    ConfigClient {}
    #[async_trait]
    impl ConfigClient for ConfigClient {
        async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T>;
        async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
        async fn watch(&self, prefix: &str) -> Result<WatchHandle>;
        async fn get_keys_under(&self, prefix: &str) -> Result<Vec<String>>;
    }
}

mock! {
    IngestionCoordinator {}
    #[async_trait]
    impl IngestionCoordinator for IngestionCoordinator {
        async fn spawn_sources(&mut self, stream_id: &str, sources: Vec<SourceConfig>) -> Result<()>;
        async fn stop_sources(&mut self, stream_id: &str) -> Result<()>;
    }
}

#[tokio::test]
async fn test_registry_initializes_with_existing_streams() {
    // Arrange
    let mut mock_client = MockConfigClient::new();
    mock_client.expect_get_keys_under()
        .with(eq("/"))
        .times(1)
        .returning(|_| Ok(vec!["air-quality".to_string(), "weather".to_string()]));

    mock_client.expect_get::<StreamConfig>()
        .withf(|key| key.contains("config"))
        .times(2)
        .returning(|key| {
            if key.contains("air-quality") {
                Ok(create_air_quality_config())
            } else {
                Ok(create_weather_config())
            }
        });

    mock_client.expect_get::<Schema>()
        .times(2)
        .returning(|_| Ok(create_valid_schema()));

    // Act
    let registry = StreamRegistry::new(mock_client).await.unwrap();

    // Assert
    assert_eq!(registry.stream_count(), 2);
}

#[tokio::test]
async fn test_registry_validates_schema_on_registration() {
    // Arrange
    let mut mock_client = MockConfigClient::new();
    mock_client.expect_put()
        .times(0); // Should NOT call put if validation fails

    let invalid_config = StreamConfig {
        schema: Schema {
            fields: vec![] // Invalid: empty schema
        },
        ..Default::default()
    };

    // Act
    let registry = StreamRegistry::new(mock_client).await.unwrap();
    let result = registry.register_stream("invalid", invalid_config).await;

    // Assert
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("schema"));
}

#[tokio::test]
async fn test_registry_notifies_coordinator_on_stream_addition() {
    // Arrange
    let mut mock_client = MockConfigClient::new();
    let mut mock_coordinator = MockIngestionCoordinator::new();

    mock_client.expect_put()
        .times(3) // config, schema, sources
        .returning(|_, _| Ok(()));

    mock_coordinator.expect_spawn_sources()
        .with(eq("weather"), always())
        .times(1)
        .returning(|_, _| Ok(()));

    // Act
    let mut registry = StreamRegistry::with_coordinator(mock_client, mock_coordinator).await.unwrap();
    registry.register_stream("weather", create_weather_config()).await.unwrap();

    // Assert: expectations verified by mockall
}

#[tokio::test]
async fn test_registry_hot_reload_on_etcd_watch_event() {
    // Arrange
    let (watch_tx, watch_rx) = mpsc::channel(10);
    let mut mock_client = MockConfigClient::new();

    mock_client.expect_watch()
        .times(1)
        .returning(move |_| Ok(watch_rx));

    mock_client.expect_get::<StreamConfig>()
        .times(1)
        .returning(|_| Ok(create_updated_config()));

    // Act
    let registry = StreamRegistry::new(mock_client).await.unwrap();
    watch_tx.send(WatchEvent::Put { key: "/weather/config", value: "..." }).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await; // Allow watch processing

    // Assert
    assert!(registry.has_stream("weather"));
}

// Additional tests:
// - test_registry_handles_concurrent_registrations
// - test_registry_prevents_duplicate_stream_ids
// - test_registry_graceful_shutdown_cancels_watch
// - test_registry_error_handling_etcd_unavailable
```

### 3.2 HttpPoller Source (London TDD)

**Component**: `/apps/air-quality-app/src/sources/http_poller.rs`

**Dependencies to Mock**:
```rust
trait HttpClient {
    async fn request(&self, url: &str, method: HttpMethod) -> Result<Response>;
}

trait ResponseParser {
    fn parse(&self, body: &[u8], format: ResponseFormat) -> Result<Vec<TimeSeriesPoint>>;
}
```

**Test Cases**:

```rust
// File: apps/air-quality-app/tests/unit/http_poller_test.rs

mock! {
    HttpClient {}
    #[async_trait]
    impl HttpClient for HttpClient {
        async fn request(&self, url: &str, method: HttpMethod) -> Result<Response>;
    }
}

mock! {
    ResponseParser {}
    impl ResponseParser for ResponseParser {
        fn parse(&self, body: &[u8], format: ResponseFormat) -> Result<Vec<TimeSeriesPoint>>;
    }
}

#[tokio::test]
async fn test_poller_fetches_data_at_configured_interval() {
    // Arrange
    let mut mock_http = MockHttpClient::new();
    mock_http.expect_request()
        .times(3) // Expect 3 polls
        .returning(|_, _| Ok(Response { status: 200, body: b"[]" }));

    let config = HttpPollerConfig {
        interval: Duration::from_millis(100),
        ..Default::default()
    };

    // Act
    let (tx, mut rx) = mpsc::channel(10);
    let poller = HttpPoller::new(config, mock_http, tx);
    let handle = tokio::spawn(poller.run());

    tokio::time::sleep(Duration::from_millis(350)).await;
    handle.abort();

    // Assert: 3 fetch cycles occurred
}

#[tokio::test]
async fn test_poller_authenticates_with_bearer_token() {
    // Arrange
    let mut mock_http = MockHttpClient::new();
    mock_http.expect_request()
        .withf(|url, _| url.contains("Authorization: Bearer test-token"))
        .times(1)
        .returning(|_, _| Ok(Response::ok()));

    let config = HttpPollerConfig {
        auth: Some(AuthConfig::Bearer("test-token".to_string())),
        ..Default::default()
    };

    // Act
    let (tx, _rx) = mpsc::channel(10);
    let poller = HttpPoller::new(config, mock_http, tx);
    poller.poll_once().await.unwrap();

    // Assert: mock expectation verified
}

#[tokio::test]
async fn test_poller_retries_on_http_error_with_backoff() {
    // Arrange
    let mut mock_http = MockHttpClient::new();
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    mock_http.expect_request()
        .times(3) // Initial + 2 retries
        .returning(move |_, _| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Err(anyhow::anyhow!("Network timeout"))
        });

    // Act
    let (tx, _rx) = mpsc::channel(10);
    let poller = HttpPoller::new(config, mock_http, tx);
    let result = poller.poll_once_with_retry().await;

    // Assert
    assert!(result.is_err());
    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_poller_parses_json_response_to_points() {
    // Arrange
    let mut mock_http = MockHttpClient::new();
    let mut mock_parser = MockResponseParser::new();

    mock_http.expect_request()
        .times(1)
        .returning(|_, _| Ok(Response {
            status: 200,
            body: br#"[{"timestamp": "2024-01-01T10:00:00Z", "value": 25.5}]"#
        }));

    mock_parser.expect_parse()
        .times(1)
        .returning(|_, _| Ok(vec![create_test_point()]));

    // Act
    let (tx, mut rx) = mpsc::channel(10);
    let poller = HttpPoller::with_parser(config, mock_http, mock_parser, tx);
    poller.poll_once().await.unwrap();

    // Assert
    let point = rx.recv().await.unwrap();
    assert_eq!(point.value, 25.5);
}

#[tokio::test]
async fn test_poller_health_check_returns_endpoint_status() {
    // Arrange
    let mut mock_http = MockHttpClient::new();
    mock_http.expect_request()
        .times(1)
        .returning(|_, _| Ok(Response { status: 200, body: b"" }));

    // Act
    let (tx, _rx) = mpsc::channel(10);
    let poller = HttpPoller::new(config, mock_http, tx);
    let health = poller.health_check().await.unwrap();

    // Assert
    assert!(health.healthy);
    assert_eq!(health.details.get("status_code"), Some(&"200".to_string()));
}

// Additional tests:
// - test_poller_timeout_enforcement
// - test_poller_rate_limiting_compliance
// - test_poller_graceful_shutdown
// - test_poller_csv_parsing_support
// - test_poller_invalid_json_handling
```

### 3.3 WebhookHandler (London TDD with Axum Testing)

**Component**: `/apps/air-quality-app/src/sources/webhook_handler.rs`

**Dependencies to Mock**:
```rust
trait Authenticator {
    fn validate_token(&self, token: &str) -> Result<bool>;
}

trait SchemaValidator {
    fn validate(&self, data: &serde_json::Value, schema: &Schema) -> Result<()>;
}
```

**Test Cases**:

```rust
// File: apps/air-quality-app/tests/unit/webhook_handler_test.rs

use axum_test::TestServer;

mock! {
    Authenticator {}
    impl Authenticator for Authenticator {
        fn validate_token(&self, token: &str) -> Result<bool>;
    }
}

mock! {
    SchemaValidator {}
    impl SchemaValidator for SchemaValidator {
        fn validate(&self, data: &serde_json::Value, schema: &Schema) -> Result<()>;
    }
}

#[tokio::test]
async fn test_webhook_accepts_valid_event() {
    // Arrange
    let mut mock_auth = MockAuthenticator::new();
    mock_auth.expect_validate_token()
        .with(eq("valid-token"))
        .times(1)
        .returning(|_| Ok(true));

    let mut mock_validator = MockSchemaValidator::new();
    mock_validator.expect_validate()
        .times(1)
        .returning(|_, _| Ok(()));

    let (tx, mut rx) = mpsc::channel(10);
    let server = WebhookServer::with_mocks(mock_auth, mock_validator, tx);
    let test_server = TestServer::new(server.router()).unwrap();

    // Act
    let response = test_server.post("/api/streams/home-events/events")
        .add_header("Authorization", "Bearer valid-token")
        .json(&json!({"event_type": "cooking_start", "room": "kitchen"}))
        .await;

    // Assert
    response.assert_status(StatusCode::ACCEPTED);
    let point = rx.recv().await.unwrap();
    assert_eq!(point.tags.get("event_type"), Some(&"cooking_start".to_string()));
}

#[tokio::test]
async fn test_webhook_rejects_invalid_token() {
    // Arrange
    let mut mock_auth = MockAuthenticator::new();
    mock_auth.expect_validate_token()
        .with(eq("invalid-token"))
        .times(1)
        .returning(|_| Ok(false));

    let (tx, _rx) = mpsc::channel(10);
    let server = WebhookServer::with_mocks(mock_auth, MockSchemaValidator::new(), tx);
    let test_server = TestServer::new(server.router()).unwrap();

    // Act
    let response = test_server.post("/api/streams/home-events/events")
        .add_header("Authorization", "Bearer invalid-token")
        .json(&json!({"event_type": "test"}))
        .await;

    // Assert
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_webhook_validates_schema_conformance() {
    // Arrange
    let mut mock_validator = MockSchemaValidator::new();
    mock_validator.expect_validate()
        .times(1)
        .returning(|_, _| Err(anyhow::anyhow!("Field 'required_field' missing")));

    let (tx, _rx) = mpsc::channel(10);
    let server = WebhookServer::with_mocks(always_valid_auth(), mock_validator, tx);
    let test_server = TestServer::new(server.router()).unwrap();

    // Act
    let response = test_server.post("/api/streams/home-events/events")
        .add_header("Authorization", "Bearer valid-token")
        .json(&json!({"incomplete": "data"}))
        .await;

    // Assert
    response.assert_status(StatusCode::BAD_REQUEST);
    response.assert_text_contains("required_field");
}

#[tokio::test]
async fn test_webhook_rate_limiting() {
    // Arrange
    let (tx, _rx) = mpsc::channel(10);
    let server = WebhookServer::with_rate_limit(100, Duration::from_secs(60), tx); // 100 req/min
    let test_server = TestServer::new(server.router()).unwrap();

    // Act: Send 101 requests rapidly
    let mut responses = vec![];
    for _ in 0..101 {
        let response = test_server.post("/api/streams/test/events")
            .add_header("Authorization", "Bearer token")
            .json(&json!({"test": "data"}))
            .await;
        responses.push(response.status_code());
    }

    // Assert: Last request should be rate-limited
    assert_eq!(responses[100], StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_webhook_health_endpoint() {
    // Arrange
    let (tx, _rx) = mpsc::channel(10);
    let server = WebhookServer::new(tx);
    let test_server = TestServer::new(server.router()).unwrap();

    // Act
    let response = test_server.get("/health").await;

    // Assert
    response.assert_status_ok();
    response.assert_json(&json!({"healthy": true, "server": "running"}));
}

// Additional tests:
// - test_webhook_concurrent_requests
// - test_webhook_malformed_json_handling
// - test_webhook_content_type_validation
// - test_webhook_cors_headers
```

### 3.4 StreamRecord Serialization (Pure Unit Tests)

**Component**: `/apps/air-quality-app/src/models/stream_record.rs`

**Test Cases** (no mocks needed - pure data structure):

```rust
// File: apps/air-quality-app/tests/unit/stream_record_test.rs

#[test]
fn test_stream_record_serializes_to_json() {
    // Arrange
    let record = StreamRecord {
        stream_id: "air-quality".to_string(),
        timestamp: Utc.with_ymd_and_hms(2025, 12, 15, 10, 30, 0).unwrap(),
        data: json!({"pm25": 12.5, "co2": 450}),
    };

    // Act
    let json = serde_json::to_string(&record).unwrap();

    // Assert
    assert!(json.contains("air-quality"));
    assert!(json.contains("12.5"));
    assert!(json.contains("450"));
}

#[test]
fn test_stream_record_deserializes_from_json() {
    // Arrange
    let json = r#"{
        "stream_id": "weather",
        "timestamp": "2025-12-15T10:30:00Z",
        "data": {"temperature": 22.5}
    }"#;

    // Act
    let record: StreamRecord = serde_json::from_str(json).unwrap();

    // Assert
    assert_eq!(record.stream_id, "weather");
    assert_eq!(record.data["temperature"], 22.5);
}

#[test]
fn test_stream_record_validates_stream_id_format() {
    // Valid formats
    assert!(is_valid_stream_id("air-quality"));
    assert!(is_valid_stream_id("home-events"));
    assert!(is_valid_stream_id("weather-123"));

    // Invalid formats
    assert!(!is_valid_stream_id("Air-Quality")); // Uppercase
    assert!(!is_valid_stream_id("air quality")); // Space
    assert!(!is_valid_stream_id("air_quality")); // Underscore
}

#[test]
fn test_stream_record_handles_nested_json() {
    // Arrange
    let record = StreamRecord {
        stream_id: "complex".to_string(),
        timestamp: Utc::now(),
        data: json!({
            "sensors": {
                "indoor": {"pm25": 10.0},
                "outdoor": {"pm25": 25.0}
            }
        }),
    };

    // Act
    let serialized = serde_json::to_string(&record).unwrap();
    let deserialized: StreamRecord = serde_json::from_str(&serialized).unwrap();

    // Assert
    assert_eq!(deserialized.data["sensors"]["indoor"]["pm25"], 10.0);
}
```

### 3.5 Multi-Stream ParquetStore (Integration Test)

**Component**: `/apps/air-quality-app/src/storage/multi_stream_store.rs`

**Test Strategy**: Integration test with REAL ParquetStore (not mocked)

```rust
// File: apps/air-quality-app/tests/integration/multi_stream_storage_test.rs

#[tokio::test]
async fn test_multi_stream_store_isolates_streams() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let store = MultiStreamStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Act: Write to two different streams
    store.write_batch("air-quality", vec![
        create_point(now, "sensor-1", 25.5)
    ]).await.unwrap();

    store.write_batch("weather", vec![
        create_point(now, "station-1", 22.0)
    ]).await.unwrap();

    // Assert: Queries don't cross-contaminate
    let air_results = store.query("air-quality", "sensor-1", now - Duration::hours(1), now + Duration::hours(1)).await.unwrap();
    let weather_results = store.query("weather", "station-1", now - Duration::hours(1), now + Duration::hours(1)).await.unwrap();

    assert_eq!(air_results.len(), 1);
    assert_eq!(weather_results.len(), 1);
    assert_eq!(air_results[0].value, 25.5);
    assert_eq!(weather_results[0].value, 22.0);
}

#[tokio::test]
async fn test_multi_stream_store_partition_structure() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let store = MultiStreamStore::new(temp_dir.path()).unwrap();

    // Act
    store.write_batch("test-stream", vec![create_point(Utc::now(), "loc-1", 42.0)]).await.unwrap();

    // Assert: Verify directory structure
    let expected_path = temp_dir.path().join("streams/test-stream/data/loc-1");
    assert!(expected_path.exists());
    assert!(expected_path.join(format!("year={}/month={}/day={}",
        Utc::now().year(), Utc::now().month(), Utc::now().day())).exists());
}

#[tokio::test]
async fn test_multi_stream_store_concurrent_writes() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(MultiStreamStore::new(temp_dir.path()).unwrap());
    let now = Utc::now();

    // Act: Concurrent writes to different streams
    let handles = (0..5).map(|i| {
        let store_clone = store.clone();
        tokio::spawn(async move {
            store_clone.write_batch(
                &format!("stream-{}", i),
                vec![create_point(now, "sensor-1", i as f64)]
            ).await
        })
    }).collect::<Vec<_>>();

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Assert: All streams have data
    for i in 0..5 {
        let results = store.query(&format!("stream-{}", i), "sensor-1", now - Duration::hours(1), now + Duration::hours(1)).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, i as f64);
    }
}

#[tokio::test]
async fn test_backward_compatibility_with_air002_format() {
    // Arrange: Use existing AIR-002 Parquet data
    let temp_dir = TempDir::new().unwrap();

    // Simulate AIR-002 data structure: data/{location_id}/year=.../readings.parquet
    let air002_path = temp_dir.path().join("data/sensor-001/year=2025/month=12/day=15");
    std::fs::create_dir_all(&air002_path).unwrap();

    // Write data using OLD ParquetStore
    let old_store = ParquetStore::new(temp_dir.path()).unwrap();
    old_store.write_batch(vec![create_point(Utc::now(), "sensor-001", 25.5)]).await.unwrap();

    // Act: Read using NEW MultiStreamStore
    let new_store = MultiStreamStore::new(temp_dir.path()).unwrap();
    let results = new_store.query_legacy("sensor-001", Utc::now() - Duration::hours(1), Utc::now() + Duration::hours(1)).await.unwrap();

    // Assert: Old data still readable
    assert!(!results.is_empty());
}
```

### 3.6 Cross-Stream Query (Integration Test)

```rust
// File: apps/air-quality-app/tests/integration/cross_stream_query_test.rs

#[tokio::test]
async fn test_asof_join_correlates_streams() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let store = MultiStreamStore::new(temp_dir.path()).unwrap();
    let base_time = Utc::now();

    // Write air-quality data
    store.write_batch("air-quality", vec![
        create_point(base_time, "sensor-1", 10.0),
        create_point(base_time + Duration::minutes(5), "sensor-1", 15.0),
        create_point(base_time + Duration::minutes(10), "sensor-1", 20.0),
    ]).await.unwrap();

    // Write home-events data
    store.write_batch("home-events", vec![
        create_event(base_time + Duration::minutes(1), "cooking_start"),
    ]).await.unwrap();

    // Act: ASOF JOIN query
    let aligned = store.query_cross_stream(
        vec!["air-quality", "home-events"],
        base_time,
        base_time + Duration::minutes(15),
        AlignmentStrategy::Asof { tolerance: Duration::minutes(5) }
    ).await.unwrap();

    // Assert: Air quality points aligned with cooking event
    assert_eq!(aligned.len(), 3);
    assert!(aligned[1].streams.contains_key("home-events")); // 5-minute mark
    assert_eq!(aligned[1].streams["air-quality"], 15.0);
}
```

---

## 4. Test Naming Conventions

### 4.1 Unit Test Naming

**Pattern**: `test_[component]_[behavior]_[condition]`

**Examples**:
- `test_registry_validates_schema_on_registration`
- `test_poller_retries_on_http_error_with_backoff`
- `test_webhook_rejects_invalid_token`

### 4.2 Integration Test Naming

**Pattern**: `test_[feature]_[integration_point]`

**Examples**:
- `test_multi_stream_store_isolates_streams`
- `test_asof_join_correlates_streams`
- `test_backward_compatibility_with_air002_format`

---

## 5. Test Execution Plan

### 5.1 Development Workflow

**TDD Cycle** (Red-Green-Refactor):
1. **Write failing test** (Red)
   ```bash
   cargo test test_registry_validates_schema --package air-quality-app
   # Expected: Test compilation fails or test fails
   ```

2. **Implement minimum code** (Green)
   ```rust
   // Add just enough to pass
   impl StreamRegistry {
       fn validate_schema(&self, schema: &Schema) -> Result<()> {
           if schema.fields.is_empty() {
               return Err(anyhow::anyhow!("Schema cannot be empty"));
           }
           Ok(())
       }
   }
   ```

3. **Run test again**
   ```bash
   cargo test test_registry_validates_schema --package air-quality-app
   # Expected: Test passes
   ```

4. **Refactor** (improve design without changing behavior)

5. **Run ALL tests** to ensure no regressions
   ```bash
   cargo test --package air-quality-app --lib
   ```

### 5.2 CI/CD Pipeline

**Pre-commit Hook**:
```bash
#!/bin/bash
# Run fast unit tests only
cargo test --package air-quality-app --lib -- --test-threads=1
```

**Pull Request Checks**:
```bash
# Full test suite
cargo test --package air-quality-app --all-features

# Coverage check
cargo tarpaulin --package air-quality-app --out Lcov --output-dir ./coverage
# Enforce 90% minimum
```

**Nightly Regression Tests**:
```bash
# Stress tests + long-running integration tests
cargo test --package air-quality-app --all-features --release -- --ignored
```

---

## 6. Coverage Targets

### 6.1 Component-Level Coverage Goals

| Component | Target Coverage | Strategy |
|-----------|----------------|----------|
| StreamRegistry | 95% | Mock ConfigClient, test all state transitions |
| HttpPoller | 90% | Mock HTTP client, test retry logic |
| WebhookHandler | 90% | Axum TestServer, mock auth/validation |
| StreamRecord | 100% | Pure functions, no mocks needed |
| MultiStreamStore | 85% | Integration tests with real ParquetStore |
| Cross-Stream Query | 80% | Integration tests with real data |
| Schema Validator | 95% | Pure validation logic |

### 6.2 Coverage Verification

**Tool**: `cargo-tarpaulin`

```bash
# Install
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin \
    --package air-quality-app \
    --out Html \
    --output-dir ./coverage \
    --exclude-files "tests/*" "mocks/*"

# Open report
xdg-open coverage/index.html
```

**Coverage Report Structure**:
```
coverage/
├── index.html                    # Overall summary
├── registry/
│   └── stream_registry.rs.html  # Line-by-line coverage
├── sources/
│   ├── http_poller.rs.html
│   └── webhook_handler.rs.html
└── storage/
    └── multi_stream_store.rs.html
```

---

## 7. Mock Strategy with mockall

### 7.1 Creating Mocks

**Step 1**: Define trait for external dependency
```rust
#[async_trait]
trait ConfigClient {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T>;
    async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
}
```

**Step 2**: Use mockall to generate mock
```rust
#[cfg(test)]
use mockall::{automock, predicate::*};

#[cfg_attr(test, automock)]
#[async_trait]
trait ConfigClient {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T>;
    async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
}
```

**Step 3**: Use mock in tests
```rust
#[tokio::test]
async fn test_with_mock() {
    let mut mock = MockConfigClient::new();
    mock.expect_get()
        .with(eq("/test/key"))
        .times(1)
        .returning(|_| Ok("test-value".to_string()));

    // Use mock as dependency
    let component = MyComponent::new(mock);
    component.do_something().await;
}
```

### 7.2 Common Mock Patterns

**Pattern 1: Returning Static Values**
```rust
mock.expect_get()
    .returning(|_| Ok(StreamConfig::default()));
```

**Pattern 2: Dynamic Responses Based on Input**
```rust
mock.expect_get()
    .withf(|key| key.contains("air-quality"))
    .returning(|key| {
        if key.ends_with("config") {
            Ok(create_air_quality_config())
        } else {
            Ok(create_weather_config())
        }
    });
```

**Pattern 3: Simulating Errors**
```rust
mock.expect_request()
    .times(3)
    .returning(|_, _| Err(anyhow::anyhow!("Network timeout")));
```

**Pattern 4: Stateful Mocks**
```rust
let call_count = Arc::new(AtomicUsize::new(0));
let call_count_clone = call_count.clone();

mock.expect_poll()
    .returning(move || {
        let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
        if count < 2 {
            Err(anyhow::anyhow!("Retry"))
        } else {
            Ok(Response::ok())
        }
    });
```

---

## 8. Backward Compatibility Testing

### 8.1 AIR-002 Data Format Compatibility

**Test Goal**: Ensure AIR-004 can read AIR-002 Parquet files

```rust
#[tokio::test]
async fn test_read_legacy_air002_parquet_files() {
    // Setup: Create AIR-002 format data
    let temp_dir = TempDir::new().unwrap();
    let old_store = ParquetStore::new(temp_dir.path()).unwrap();

    let air002_point = TimeSeriesPoint {
        timestamp: Utc::now(),
        location_id: "sensor-001".to_string(),
        value: 25.5,
        tags: HashMap::from([("metric".to_string(), "pm25".to_string())]),
    };

    old_store.write_batch(vec![air002_point.clone()]).await.unwrap();

    // Test: Read using AIR-004 MultiStreamStore
    let new_store = MultiStreamStore::new(temp_dir.path()).unwrap();
    let results = new_store.query_legacy(
        "sensor-001",
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::hours(1)
    ).await.unwrap();

    // Assert: Data readable without migration
    assert!(!results.is_empty());
    assert_eq!(results[0].value, 25.5);
}
```

### 8.2 Configuration Backward Compatibility

**Test Goal**: `/air-quality/*` etcd keys continue working

```rust
#[tokio::test]
async fn test_legacy_etcd_config_keys_still_work() {
    // Setup: Populate OLD etcd keys
    let client = ConfigClient::with_prefix(&["http://localhost:2379"], "/air-quality").await.unwrap();
    client.set("/mqtt/broker_url", &json!("mosquitto")).await.unwrap();
    client.set("/storage/base_path", &json!("/app/data")).await.unwrap();

    // Test: Load config using AIR-004 loader (with backward compatibility layer)
    let config = load_config_with_backward_compatibility().await.unwrap();

    // Assert: Old keys still work
    assert_eq!(config.mqtt.broker_url, "mosquitto");
    assert_eq!(config.storage.base_path, "/app/data");
}
```

---

## 9. Performance Testing

### 9.1 Throughput Baseline

**Goal**: Ensure AIR-004 matches AIR-002 performance

```rust
#[tokio::test]
async fn test_ingestion_throughput_no_regression() {
    // Baseline: AIR-002 performance
    let air002_throughput = benchmark_air002_ingestion().await;

    // New: AIR-004 performance
    let air004_throughput = benchmark_air004_ingestion().await;

    // Assert: No more than 10% regression
    let regression_percent = (air002_throughput - air004_throughput) / air002_throughput * 100.0;
    assert!(regression_percent < 10.0,
        "Performance regression detected: {}% (AIR-002: {}/s, AIR-004: {}/s)",
        regression_percent, air002_throughput, air004_throughput);
}

async fn benchmark_air002_ingestion() -> f64 {
    let store = ParquetStore::new(temp_dir()).unwrap();
    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        store.write_batch(vec![create_test_point()]).await.unwrap();
    }

    let elapsed = start.elapsed().as_secs_f64();
    10_000.0 / elapsed // records/second
}
```

### 9.2 Config Read Latency

**Goal**: Config reads remain < 10ms

```rust
#[tokio::test]
async fn test_config_read_latency_under_10ms() {
    let registry = StreamRegistry::new(config_client).await.unwrap();

    let mut durations = vec![];
    for _ in 0..100 {
        let start = std::time::Instant::now();
        let _ = registry.get_stream("air-quality").await;
        durations.push(start.elapsed());
    }

    let p95 = percentile(&durations, 0.95);
    assert!(p95.as_millis() < 10, "p95 latency {} ms exceeds 10ms threshold", p95.as_millis());
}
```

---

## 10. Test Data Builders

### 10.1 Helper Functions

```rust
// File: apps/air-quality-app/tests/common/test_data.rs

pub fn create_air_quality_config() -> StreamConfig {
    StreamConfig {
        id: "air-quality".to_string(),
        description: "Air quality monitoring".to_string(),
        retention_days: 365,
        schema: Schema {
            fields: vec![
                Field { name: "pm25".to_string(), field_type: FieldType::Float, nullable: false },
                Field { name: "co2".to_string(), field_type: FieldType::Int, nullable: false },
            ]
        },
        sources: vec![
            SourceConfig {
                source_type: SourceType::Mqtt,
                params: HashMap::from([
                    ("broker_url".to_string(), "mosquitto".to_string()),
                    ("topic".to_string(), "airgradient/readings/+".to_string()),
                ]),
            }
        ],
    }
}

pub fn create_weather_config() -> StreamConfig {
    StreamConfig {
        id: "weather".to_string(),
        description: "Weather station data".to_string(),
        retention_days: 365,
        schema: Schema {
            fields: vec![
                Field { name: "temperature".to_string(), field_type: FieldType::Float, nullable: false },
                Field { name: "humidity".to_string(), field_type: FieldType::Float, nullable: false },
            ]
        },
        sources: vec![
            SourceConfig {
                source_type: SourceType::HttpPoll,
                params: HashMap::from([
                    ("url".to_string(), "https://api.weather.com/data".to_string()),
                    ("interval".to_string(), "5m".to_string()),
                ]),
            }
        ],
    }
}

pub fn create_test_point(timestamp: DateTime<Utc>, location: &str, value: f64) -> TimeSeriesPoint {
    TimeSeriesPoint {
        timestamp,
        location_id: location.to_string(),
        value,
        tags: HashMap::new(),
    }
}
```

---

## 11. Success Criteria

### 11.1 Test Quality Metrics

- ✅ **Coverage**: 90% minimum for new AIR-004 code
- ✅ **Test Speed**: Unit tests < 100ms each, integration tests < 5s each
- ✅ **Failure Rate**: < 1% flaky tests
- ✅ **Backward Compatibility**: All AIR-002/AIR-003 tests continue passing
- ✅ **Documentation**: Every component has test strategy section in doc comments

### 11.2 Code Review Checklist

Before merging AIR-004 implementation:

- [ ] All new components have corresponding test files
- [ ] Test coverage report shows 90% minimum
- [ ] All existing integration tests pass
- [ ] Backward compatibility tests pass (AIR-002 data readable)
- [ ] Performance tests show no regression (< 10% slowdown)
- [ ] Mock strategy consistently applied (London TDD)
- [ ] Test data builders in common module
- [ ] No hardcoded test data (use builders)
- [ ] CI/CD pipeline includes all tests

---

## 12. Implementation Order

### Phase 1: Foundation (Week 1)
1. Set up test infrastructure:
   - Add mockall to dev-dependencies
   - Create `tests/common/test_data.rs` builder module
   - Set up coverage tooling (cargo-tarpaulin)

2. Baseline verification tests:
   - Run all existing tests and document results
   - Create regression test suite snapshot

### Phase 2: Core Components (Week 2-3)
1. StreamRegistry tests (London TDD with MockConfigClient)
2. StreamRecord serialization tests (pure unit tests)
3. Schema validator tests (pure unit tests)

### Phase 3: Source Implementations (Week 4-5)
1. HttpPoller tests (MockHttpClient)
2. WebhookHandler tests (Axum TestServer + MockAuthenticator)
3. MqttSource wrapper tests (reuse existing MqttSource)

### Phase 4: Storage & Integration (Week 6-7)
1. MultiStreamStore integration tests (real ParquetStore)
2. Cross-stream query tests (real data)
3. Backward compatibility tests (AIR-002 format)

### Phase 5: E2E & Performance (Week 8)
1. Full pipeline E2E tests
2. Performance regression tests
3. Load testing (10k records/sec baseline)

---

## 13. Risk Mitigation

### 13.1 Identified Risks

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Mock drift (mocks diverge from real implementation) | High | Regular integration tests with real dependencies |
| Flaky tests due to timing issues | Medium | Use tokio::time::pause() for deterministic async tests |
| Test slowness (linking large binaries) | Medium | Split tests into focused feature sets |
| Backward compatibility breaks | High | Dedicated compatibility test suite runs nightly |

### 13.2 Contingency Plans

**If coverage target not met**:
- Identify uncovered branches with `cargo-tarpaulin --show-missing-lines`
- Prioritize critical paths (error handling, validation)
- Accept lower coverage for defensive code (edge cases)

**If performance regression detected**:
- Profile with `cargo-flamegraph`
- Compare hot paths between AIR-002 and AIR-004
- Optimize critical sections before merge

**If tests become unmaintainable**:
- Refactor test data builders
- Extract common setup into fixtures
- Consider property-based testing with `proptest` for complex validation

---

## 14. References

### 14.1 Testing Resources

- **London School TDD**: [Growing Object-Oriented Software, Guided by Tests](http://www.growing-object-oriented-software.com/)
- **Mockall Documentation**: [https://docs.rs/mockall/](https://docs.rs/mockall/)
- **Rust Testing Book**: [https://rust-lang.github.io/book/ch11-00-testing.html](https://rust-lang.github.io/book/ch11-00-testing.html)
- **Axum Testing**: [https://docs.rs/axum-test/](https://docs.rs/axum-test/)

### 14.2 Internal Documents

- [AIR-004 Specification](/workspaces/neural-data-platform/product/features/air-004/specification/SPECIFICATION.md)
- [AIR-004 Pseudocode](/workspaces/neural-data-platform/product/features/air-004/pseudocode/PSEUDOCODE.md)
- [AIR-002 Implementation Baseline](/workspaces/neural-data-platform/apps/air-quality-app/)

---

## Appendix A: Test File Structure

```
apps/air-quality-app/
├── tests/
│   ├── common/
│   │   ├── mod.rs
│   │   └── test_data.rs              # Builders for test data
│   ├── unit/
│   │   ├── stream_registry_test.rs   # London TDD with mocks
│   │   ├── http_poller_test.rs       # London TDD with mocks
│   │   ├── webhook_handler_test.rs   # Axum TestServer tests
│   │   └── stream_record_test.rs     # Pure unit tests
│   ├── integration/
│   │   ├── multi_stream_storage_test.rs
│   │   ├── cross_stream_query_test.rs
│   │   └── backward_compatibility_test.rs
│   ├── e2e/
│   │   └── full_pipeline_test.rs
│   ├── performance/
│   │   └── throughput_test.rs
│   ├── integration_test.rs           # EXISTING - MUST PASS
│   ├── etcd_config_test.rs           # EXISTING - MUST PASS
│   └── config_hierarchy_test.rs      # EXISTING - MUST PASS
└── src/
    └── */                             # Inline #[cfg(test)] modules
```

---

## Appendix B: Example Test Output

```
$ cargo test --package air-quality-app --lib

running 58 tests
test unit::stream_registry_test::test_registry_initializes_with_existing_streams ... ok
test unit::stream_registry_test::test_registry_validates_schema_on_registration ... ok
test unit::stream_registry_test::test_registry_notifies_coordinator_on_stream_addition ... ok
test unit::http_poller_test::test_poller_fetches_data_at_configured_interval ... ok
test unit::http_poller_test::test_poller_authenticates_with_bearer_token ... ok
test unit::http_poller_test::test_poller_retries_on_http_error_with_backoff ... ok
test unit::webhook_handler_test::test_webhook_accepts_valid_event ... ok
test unit::webhook_handler_test::test_webhook_rejects_invalid_token ... ok
test unit::webhook_handler_test::test_webhook_validates_schema_conformance ... ok
test integration::multi_stream_storage_test::test_multi_stream_store_isolates_streams ... ok
test integration::multi_stream_storage_test::test_backward_compatibility_with_air002_format ... ok
test integration::cross_stream_query_test::test_asof_join_correlates_streams ... ok

test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.34s
```

---

**Status**: STRATEGY COMPLETE
**Next Steps**:
1. Review strategy with stakeholders
2. Begin Phase 1 implementation (test infrastructure setup)
3. Execute TDD cycle for StreamRegistry (first component)
4. Monitor coverage reports after each component

**Memory Update**: Save to swarm memory at `/swarm/tester/air004-test-strategy`
