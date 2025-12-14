# AIR-002 Test Plan: Ingestion Pipeline Testing Strategy

**Version:** 1.0.0
**Date:** December 14, 2025
**Status:** ACTIVE
**Test Philosophy:** London School TDD - Behavior verification through collaboration testing

---

## Executive Summary

This test plan addresses the **critical gap** in AIR-001: the ingestion pipeline exists as isolated components but is not wired together. We have 67 passing unit tests in the domain layer, but 5 failing integration tests in the app layer because the MQTT → Parser → Validator → Adapter → Storage chain is not connected.

### Current Test Status
- **Domain Tests (air-quality):** 67/67 passing (100%)
- **App Tests (air-quality-app):** 42/47 passing (89.4%)
- **MCP Integration:** 16/16 passing (100%)
- **Failing:** 5 API integration tests (storage not wired)

### Test Coverage Breakdown

| Layer | Component | Unit Tests | Integration Tests | E2E Tests | Status |
|-------|-----------|------------|-------------------|-----------|--------|
| Domain | Parser | 13 ✅ | N/A | N/A | COMPLETE |
| Domain | Validator | 27 ✅ | N/A | N/A | COMPLETE |
| Domain | Adapter | 18 ✅ | N/A | N/A | COMPLETE |
| Domain | Types | 9 ✅ | N/A | N/A | COMPLETE |
| Core | Storage | ✅ | ❌ | N/A | ISOLATED |
| Core | MQTT | ✅ | ❌ | N/A | ISOLATED |
| App | API Routes | 42 ✅ | 5 ❌ | N/A | **BLOCKED** |
| App | MCP Tools | 16 ✅ | N/A | N/A | COMPLETE |
| **Pipeline** | **End-to-End** | **N/A** | **0** ❌ | **0** ❌ | **MISSING** |

---

## 1. Unit Tests (EXISTING - Verify Coverage)

### 1.1 Parser Tests (`domains/air-quality/src/parser.rs`)
**Status:** ✅ 13 tests passing
**Coverage:** Complete - all edge cases covered

```rust
// Existing tests - NO NEW WORK NEEDED
✅ test_parse_mqtt_complete_payload_all_29_fields
✅ test_parse_mqtt_minimal_required_fields
✅ test_parse_mqtt_invalid_json_returns_error
✅ test_parse_mqtt_missing_required_field_returns_error
✅ test_parse_mqtt_handles_null_values
✅ test_parse_local_api_complete_payload_all_29_fields
✅ test_parse_local_api_reuses_mqtt_parser
✅ test_parse_local_api_invalid_json
✅ test_parse_mqtt_with_extra_fields (forward compatibility)
✅ test_parse_mqtt_unicode_serialno
✅ test_parse_mqtt_large_payload (performance)
... (13 total)
```

**Verification Required:**
- Run `cargo test -p air-quality parser::tests` to confirm all passing
- Check coverage includes all 29 AirGradient fields
- Verify error handling for malformed JSON

### 1.2 Validator Tests (`domains/air-quality/src/validation.rs`)
**Status:** ✅ 27 tests passing
**Coverage:** Complete - all sensor ranges validated

```rust
// Existing tests - NO NEW WORK NEEDED
✅ test_validate_co2_valid_range (380-10000 ppm)
✅ test_validate_co2_below_minimum
✅ test_validate_co2_above_maximum
✅ test_validate_pm25_valid_range (0-1000 µg/m³)
✅ test_validate_pm25_negative_value
✅ test_validate_temperature_valid_range (-40 to 125°C)
✅ test_validate_humidity_valid_range (0-100%)
✅ test_validate_wifi_signal_strength (-100 to 0 dBm)
✅ test_validate_reading_partial_data (Some fields missing)
✅ test_validate_reading_all_valid
✅ test_validate_reading_multiple_violations
... (27 total)
```

**Verification Required:**
- Confirm EPA PM2.5 limits (0-1000 µg/m³)
- Verify CO2 sensor range (380-10000 ppm)
- Check temperature compensated vs raw handling

### 1.3 Adapter Tests (`domains/air-quality/src/adapter.rs`)
**Status:** ✅ 18 tests passing
**Coverage:** Complete - all 29 fields mapped to time series

```rust
// Existing tests - NO NEW WORK NEEDED
✅ test_to_time_series_points_creates_29_points
✅ test_to_time_series_points_has_correct_tags (location, device, metric)
✅ test_to_time_series_points_handles_missing_data
✅ test_extract_metric_by_name
✅ test_available_metrics_lists_all_present
✅ test_timestamp_preservation
✅ test_serialno_in_all_points
✅ test_metric_specific_tags (pm02, co2, etc.)
... (18 total)
```

**Verification Required:**
- Confirm all 29 fields convert to `TimeSeriesPoint`
- Verify tags include: location, device, metric, unit
- Check timestamp consistency across all points

### 1.4 Storage Tests (Core - Existing)
**Location:** `/workspaces/neural-data-platform/core/src/storage/`
**Status:** ✅ Passing (isolated)
**Gap:** Not tested with air-quality domain types

**Verification Required:**
- Check if Parquet storage tests exist
- Verify WAL recovery tests
- Confirm time-range query tests
- **NEW:** Add tests for `AirQualityReading` → Parquet schema

---

## 2. Integration Tests (NEW - The Critical Gap)

### 2.1 Parser → Validator Integration
**Status:** ❌ MISSING
**Location:** `/workspaces/neural-data-platform/domains/air-quality/tests/integration/parser_validator_test.rs`
**Priority:** HIGH

**Test Cases:**

```rust
#[test]
fn test_parser_to_validator_chain_valid_payload() {
    // Arrange
    let payload = r#"{"serialno":"airgradient:test","rco2":450,"pm02":12.5}"#;

    // Act
    let reading = parse_mqtt_payload(payload).expect("Parse failed");
    let validation = validate_reading(&reading);

    // Assert
    assert!(validation.is_ok(), "Valid reading should pass validation");
}

#[test]
fn test_parser_to_validator_chain_invalid_co2() {
    // Arrange - CO2 outside valid range
    let payload = r#"{"serialno":"test","rco2":50000}"#;

    // Act
    let reading = parse_mqtt_payload(payload).expect("Parse succeeds");
    let validation = validate_reading(&reading);

    // Assert
    assert!(validation.is_err());
    assert!(matches!(
        validation.unwrap_err(),
        ValidationError::Co2OutOfRange(50000)
    ));
}

#[test]
fn test_parser_to_validator_handles_partial_data() {
    // Arrange - Only required fields
    let payload = r#"{"serialno":"test","rco2":400}"#;

    // Act
    let reading = parse_mqtt_payload(payload)?;
    let validation = validate_reading(&reading);

    // Assert - Partial data is valid (optional fields are None)
    assert!(validation.is_ok());
}
```

**Success Criteria:**
- Parse → Validate chain works for all 29 fields
- Invalid ranges rejected post-parse
- Partial payloads validated correctly

### 2.2 Validator → Adapter Integration
**Status:** ❌ MISSING
**Location:** `/workspaces/neural-data-platform/domains/air-quality/tests/integration/validator_adapter_test.rs`
**Priority:** HIGH

**Test Cases:**

```rust
#[test]
fn test_validated_reading_to_time_series_points() {
    // Arrange
    let reading = create_valid_reading();
    validate_reading(&reading).expect("Validation failed");

    // Act
    let points = TimeSeriesAdapter::to_time_series_points(&reading);

    // Assert
    assert_eq!(points.len(), 13); // Only non-null metrics

    // Verify CO2 point
    let co2_point = points.iter()
        .find(|p| p.tags.get("metric") == Some(&"co2".to_string()))
        .expect("CO2 point not found");
    assert_eq!(co2_point.value, 450.0);
    assert_eq!(co2_point.tags.get("unit"), Some(&"ppm".to_string()));
}

#[test]
fn test_adapter_preserves_validation_semantics() {
    // Arrange - Invalid reading that passes parser
    let mut reading = create_valid_reading();
    reading.metrics.rco2 = Some(50000); // Invalid CO2

    // Act - Should catch in validation before adapter
    let validation = validate_reading(&reading);

    // Assert
    assert!(validation.is_err(), "Adapter should not receive invalid data");
}
```

**Success Criteria:**
- Only validated readings reach adapter
- All 29 fields correctly converted
- Tags and metadata preserved

### 2.3 MQTT → Parser Integration
**Status:** ❌ MISSING
**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/integration/mqtt_parser_test.rs`
**Priority:** HIGH
**Dependencies:** Mock MQTT broker (rumqttd or mosquitto)

**Test Cases:**

```rust
#[tokio::test]
async fn test_mqtt_message_to_parser_chain() {
    // Arrange - Mock MQTT broker
    let broker = MockMqttBroker::start().await;
    let client = create_test_mqtt_client(&broker).await;

    // Act - Publish AirGradient reading
    let payload = r#"{"serialno":"test","rco2":600,"pm02":15.2}"#;
    client.publish("airgradient/readings/test", QoS::AtLeastOnce, false, payload).await?;

    // Wait for message
    let received = broker.wait_for_message(Duration::from_secs(1)).await?;

    // Assert - Parser can handle MQTT bytes
    let reading = parse_mqtt_payload(&received.payload)?;
    assert_eq!(reading.device.serialno, "test");
    assert_eq!(reading.metrics.rco2, Some(600));
}

#[tokio::test]
async fn test_mqtt_wildcard_subscription() {
    // Arrange
    let broker = MockMqttBroker::start().await;
    let client = create_test_mqtt_client(&broker).await;

    // Act - Subscribe to airgradient/readings/+
    client.subscribe("airgradient/readings/+", QoS::AtLeastOnce).await?;

    // Publish to multiple serial numbers
    client.publish("airgradient/readings/device1", ...).await?;
    client.publish("airgradient/readings/device2", ...).await?;

    // Assert - Both messages received
    let messages = broker.received_messages(2, Duration::from_secs(2)).await?;
    assert_eq!(messages.len(), 2);
}

#[tokio::test]
async fn test_mqtt_connection_resilience() {
    // Arrange
    let broker = MockMqttBroker::start().await;
    let client = create_test_mqtt_client(&broker).await;

    // Act - Kill broker
    broker.shutdown().await;

    // Wait for reconnect attempt
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Restart broker
    let broker = MockMqttBroker::restart().await;

    // Assert - Client reconnects within 60s
    let connected = wait_for_connection(&client, Duration::from_secs(60)).await;
    assert!(connected, "Client should auto-reconnect");
}
```

**Success Criteria:**
- MQTT messages successfully parsed
- Wildcard subscriptions work (`+` placeholder)
- Reconnection logic handles broker restarts

### 2.4 Adapter → Storage Integration
**Status:** ❌ MISSING
**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/integration/adapter_storage_test.rs`
**Priority:** CRITICAL (Currently failing in app tests)

**Test Cases:**

```rust
#[tokio::test]
async fn test_time_series_points_to_parquet_storage() {
    // Arrange
    let reading = create_test_reading();
    let points = TimeSeriesAdapter::to_time_series_points(&reading);

    let storage = ParquetStore::new(StorageConfig {
        data_dir: PathBuf::from("/tmp/test-air-quality"),
        ..Default::default()
    }).await?;

    // Act - Write points to storage
    for point in &points {
        storage.write_point(point).await?;
    }
    storage.flush().await?;

    // Assert - Query returns written data
    let start = Utc::now() - chrono::Duration::minutes(5);
    let end = Utc::now();
    let result = storage.query_range("test-loc", start, end).await?;

    assert_eq!(result.len(), points.len());
    assert_eq!(result[0].tags.get("metric"), Some(&"co2".to_string()));
}

#[tokio::test]
async fn test_storage_wal_recovery_after_crash() {
    // Arrange
    let storage = create_test_storage().await;

    // Act - Write 100 points
    for i in 0..100 {
        storage.write_point(&create_test_point(i)).await?;
    }

    // Simulate crash (drop without flush)
    drop(storage);

    // Restart storage
    let storage = ParquetStore::new(test_config()).await?;

    // Assert - All 100 points recovered from WAL
    let result = storage.query_all().await?;
    assert_eq!(result.len(), 100, "WAL should recover all points");
}

#[tokio::test]
async fn test_storage_daily_partitioning() {
    // Arrange
    let storage = create_test_storage().await;

    // Act - Write readings across 3 days
    let day1 = Utc.with_ymd_and_hms(2025, 12, 14, 10, 0, 0).unwrap();
    let day2 = day1 + chrono::Duration::days(1);
    let day3 = day1 + chrono::Duration::days(2);

    storage.write_point(&create_point_at(day1)).await?;
    storage.write_point(&create_point_at(day2)).await?;
    storage.write_point(&create_point_at(day3)).await?;
    storage.flush().await?;

    // Assert - 3 partition directories created
    let partitions = storage.list_partitions().await?;
    assert_eq!(partitions.len(), 3);
    assert!(partitions.contains(&"year=2025/month=12/day=14"));
    assert!(partitions.contains(&"year=2025/month=12/day=15"));
    assert!(partitions.contains(&"year=2025/month=12/day=16"));
}
```

**Success Criteria:**
- `TimeSeriesPoint` → Parquet schema conversion works
- WAL recovery restores all uncommitted writes
- Daily partitioning creates correct directory structure
- Time-range queries return correct data

### 2.5 Full Pipeline Integration (MQTT → Storage)
**Status:** ❌ MISSING
**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/integration/pipeline_test.rs`
**Priority:** CRITICAL

**Test Cases:**

```rust
#[tokio::test]
async fn test_full_pipeline_mqtt_to_storage() {
    // Arrange
    let broker = MockMqttBroker::start().await;
    let storage = create_test_storage().await;

    // Create pipeline
    let pipeline = IngestionPipeline::new(
        MqttSource::new(mqtt_config(&broker)),
        storage.clone(),
    ).await?;

    // Start pipeline
    pipeline.start().await?;

    // Act - Publish sensor reading
    broker.publish_airgradient_reading(r#"{
        "serialno": "airgradient:e2e-test",
        "rco2": 850,
        "pm02": 12.5,
        "atmp": 22.0,
        "rhum": 45
    }"#).await?;

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Assert - Data in storage
    let result = storage.query_range(
        "airgradient:e2e-test",
        Utc::now() - Duration::from_secs(10),
        Utc::now(),
    ).await?;

    assert!(!result.is_empty(), "Pipeline should store data");
    assert_eq!(result.iter().find(|p| p.tags["metric"] == "co2").unwrap().value, 850.0);
    assert_eq!(result.iter().find(|p| p.tags["metric"] == "pm25").unwrap().value, 12.5);
}

#[tokio::test]
async fn test_pipeline_rejects_invalid_data() {
    // Arrange
    let pipeline = create_test_pipeline().await;
    pipeline.start().await?;

    // Act - Publish invalid CO2 reading
    broker.publish_airgradient_reading(r#"{
        "serialno": "test",
        "rco2": 50000
    }"#).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Assert - No data stored (validation blocked it)
    let result = storage.query_all().await?;
    assert!(result.is_empty(), "Invalid data should not be stored");

    // Check dead letter queue
    let dlq = storage.query_dead_letter_queue().await?;
    assert_eq!(dlq.len(), 1);
    assert!(dlq[0].error.contains("Co2OutOfRange"));
}

#[tokio::test]
async fn test_pipeline_handles_mqtt_reconnect() {
    // Arrange
    let pipeline = create_test_pipeline().await;
    pipeline.start().await?;

    // Act - Kill broker
    broker.shutdown().await;

    // Wait for reconnect attempts
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Restart broker
    let broker = MockMqttBroker::restart().await;

    // Publish data
    broker.publish_airgradient_reading(valid_payload()).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Assert - Pipeline recovered and processed data
    let result = storage.query_all().await?;
    assert!(!result.is_empty(), "Pipeline should recover from disconnect");
}
```

**Success Criteria:**
- End-to-end data flow: MQTT → Parser → Validator → Adapter → Storage
- Invalid data rejected and sent to DLQ
- Pipeline survives MQTT broker restarts
- No data loss during reconnection

---

## 3. End-to-End Tests (Docker-based)

### 3.1 Test Infrastructure

**Docker Compose:** `/workspaces/neural-data-platform/docker/air-quality/docker-compose.test.yml`

```yaml
services:
  mosquitto:
    image: eclipse-mosquitto:2.0
    ports:
      - "1883:1883"
    volumes:
      - ./mosquitto.conf:/mosquitto/config/mosquitto.conf
    healthcheck:
      test: ["CMD", "mosquitto_pub", "-t", "test", "-m", "test"]
      interval: 10s
      timeout: 3s
      retries: 3

  air-quality-app:
    build:
      context: ../..
      dockerfile: apps/air-quality-app/Dockerfile
    depends_on:
      mosquitto:
        condition: service_healthy
    environment:
      - MQTT_BROKER=mosquitto
      - MQTT_PORT=1883
      - STORAGE_PATH=/data/air-quality
      - LOG_LEVEL=debug
    volumes:
      - test-data:/data/air-quality
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 3s
      retries: 5

  sensor-simulator:
    build:
      context: ../../tests/fixtures
      dockerfile: sensor-simulator.dockerfile
    depends_on:
      mosquitto:
        condition: service_healthy
    environment:
      - MQTT_BROKER=mosquitto
      - PUBLISH_INTERVAL=1s
      - SERIAL_NUMBER=airgradient:e2e-test

volumes:
  test-data:
```

### 3.2 E2E Test Scenarios

**Test Runner:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/e2e/`

```rust
#[tokio::test]
#[ignore] // Run with: cargo test --ignored
async fn e2e_full_system_smoke_test() {
    // Arrange - Start Docker stack
    let env = DockerTestEnv::start("docker/air-quality/docker-compose.test.yml").await?;

    // Wait for health checks
    env.wait_for_healthy("mosquitto", Duration::from_secs(30)).await?;
    env.wait_for_healthy("air-quality-app", Duration::from_secs(60)).await?;

    // Act - Trigger sensor simulator
    env.exec("sensor-simulator", "publish-batch", &["10"]).await?;

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Assert - Query API
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:8080/api/v1/readings/latest?location_id=airgradient:e2e-test")
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    let json: serde_json::Value = response.json().await?;
    assert_eq!(json["status"], "success");
    assert!(json["data"]["co2"].as_f64().unwrap() > 0.0);

    // Cleanup
    env.shutdown().await?;
}

#[tokio::test]
#[ignore]
async fn e2e_mqtt_broker_restart_resilience() {
    // Arrange
    let env = DockerTestEnv::start("docker-compose.test.yml").await?;

    // Publish initial data
    env.exec("sensor-simulator", "publish-batch", &["5"]).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Act - Restart MQTT broker
    env.restart_service("mosquitto").await?;

    // Wait for reconnect
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Publish more data
    env.exec("sensor-simulator", "publish-batch", &["5"]).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Assert - All 10 readings stored
    let response = client.get("http://localhost:8080/api/v1/readings?location_id=airgradient:e2e-test").await?;
    let json: serde_json::Value = response.json().await?;
    assert_eq!(json["data"].as_array().unwrap().len(), 10);
}

#[tokio::test]
#[ignore]
async fn e2e_performance_100_messages_per_second() {
    // Arrange
    let env = DockerTestEnv::start("docker-compose.test.yml").await?;

    // Act - Publish 100 msg/sec for 60 seconds (6000 readings)
    let start = Instant::now();
    env.exec("sensor-simulator", "publish-rate", &["100", "60"]).await?;
    let duration = start.elapsed();

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Assert - All messages processed
    let response = client.get("http://localhost:8080/api/v1/readings?location_id=airgradient:e2e-test").await?;
    let json: serde_json::Value = response.json().await?;
    let count = json["data"].as_array().unwrap().len();

    assert!(count >= 5900, "Should process at least 98% of messages ({})", count);
    assert!(duration.as_secs() <= 70, "Should complete within 70 seconds");
}
```

**Success Criteria:**
- Docker stack starts in <60 seconds
- 100% data flow from sensor simulator to API
- System survives broker restarts with no data loss
- Performance: >100 msg/sec ingestion rate
- Latency: <1 second from MQTT publish to API query

---

## 4. Test Infrastructure & Tooling

### 4.1 Mock MQTT Broker

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/mocks/mqtt_broker.rs`

```rust
pub struct MockMqttBroker {
    server: rumqttd::Broker,
    port: u16,
    messages: Arc<Mutex<Vec<PublishMessage>>>,
}

impl MockMqttBroker {
    pub async fn start() -> Result<Self> {
        let config = rumqttd::Config {
            broker: BrokerConfig {
                port: 0, // Random port
                ..Default::default()
            },
        };

        let server = rumqttd::Broker::new(config)?;
        let port = server.local_addr()?.port();

        Ok(Self {
            server,
            port,
            messages: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn publish_airgradient_reading(&self, json: &str) -> Result<()> {
        let topic = "airgradient/readings/test";
        self.server.publish(topic, QoS::AtLeastOnce, false, json.as_bytes())?;
        Ok(())
    }

    pub async fn received_messages(&self, count: usize, timeout: Duration) -> Result<Vec<PublishMessage>> {
        let start = Instant::now();
        loop {
            let messages = self.messages.lock().await;
            if messages.len() >= count {
                return Ok(messages.clone());
            }

            if start.elapsed() > timeout {
                return Err(anyhow!("Timeout waiting for {} messages", count));
            }

            drop(messages);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
```

### 4.2 Test Data Fixtures

**Location:** `/workspaces/neural-data-platform/tests/fixtures/airgradient/`

**Files:**
- `complete-reading.json` - All 29 fields populated
- `minimal-reading.json` - Only required fields (serialno)
- `partial-reading.json` - Mix of Some/None fields
- `invalid-co2.json` - CO2 out of range (50000 ppm)
- `invalid-pm25.json` - PM2.5 negative value
- `malformed.json` - Invalid JSON syntax

```json
// complete-reading.json
{
  "wifi": -50,
  "serialno": "airgradient:fixture-complete",
  "rco2": 850,
  "pm01": 5.2,
  "pm02": 12.5,
  "pm10": 18.7,
  "pm003Count": 1500,
  "atmp": 22.5,
  "rhum": 45.0,
  "tvoc": 150,
  // ... all 29 fields
}

// invalid-co2.json
{
  "serialno": "airgradient:fixture-invalid",
  "rco2": 50000  // Outside 380-10000 range
}
```

### 4.3 Sensor Simulator

**Location:** `/workspaces/neural-data-platform/tests/fixtures/sensor-simulator/`

**Dockerfile:**
```dockerfile
FROM rust:1.75-alpine
WORKDIR /app
COPY sensor-simulator.rs .
RUN rustc sensor-simulator.rs -o sensor-simulator
CMD ["./sensor-simulator"]
```

**Features:**
- Publish readings at configurable rate (default: 1/sec)
- Random CO2: 400-2000 ppm
- Random PM2.5: 0-50 µg/m³
- Random temperature: 18-28°C
- Support batch publishing for load tests

### 4.4 Test Utilities

```rust
// /workspaces/neural-data-platform/apps/air-quality-app/tests/utils/mod.rs

pub fn create_test_reading() -> AirQualityReading {
    AirQualityReading {
        device: DeviceMetadata {
            serialno: "test-device".to_string(),
            wifi: Some(-50),
            // ...
        },
        metrics: QualityMetrics { rco2: Some(450) },
        // ... all fields
        timestamp: Some(Utc::now()),
    }
}

pub async fn create_test_storage() -> ParquetStore {
    let temp_dir = tempfile::tempdir().unwrap();
    ParquetStore::new(StorageConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    }).await.unwrap()
}

pub fn mqtt_test_config(port: u16) -> MqttConfig {
    MqttConfig {
        broker_url: "localhost".to_string(),
        port,
        client_id: format!("test-{}", uuid::Uuid::new_v4()),
        topic_pattern: "airgradient/readings/+".to_string(),
        ..Default::default()
    }
}
```

---

## 5. Acceptance Test Criteria

### 5.1 Functional Acceptance

| Requirement | Test | Pass Criteria |
|-------------|------|---------------|
| FR-1.1: MQTT Connection | `test_mqtt_connection_resilience` | Reconnects within 60s |
| FR-1.2: Message Parsing | `test_full_pipeline_mqtt_to_storage` | All 29 fields parsed |
| FR-1.3: Data Validation | `test_pipeline_rejects_invalid_data` | Invalid data → DLQ |
| FR-2.1: Parquet Storage | `test_time_series_points_to_parquet_storage` | Data persisted |
| FR-2.2: WAL Durability | `test_storage_wal_recovery_after_crash` | 100% recovery |
| FR-2.3: Time Queries | `test_storage_daily_partitioning` | Correct partitions |
| FR-3.1: API Queries | `e2e_full_system_smoke_test` | 200 OK with data |

### 5.2 Performance Acceptance

| Metric | Target | Test | Measurement |
|--------|--------|------|-------------|
| Ingestion Latency | <1s | `e2e_performance_100_messages_per_second` | MQTT publish → API query |
| Throughput | >100 msg/sec | `e2e_performance_100_messages_per_second` | 6000 msgs in 60s |
| Query Latency | <100ms | `test_readings_time_range_query` | 24h query |
| Storage Efficiency | <500MB/year | Calculate from daily partitions | 1440 readings/day |
| WAL Recovery | <30s | `test_storage_wal_recovery_after_crash` | App restart time |

### 5.3 Reliability Acceptance

| Scenario | Test | Pass Criteria |
|----------|------|---------------|
| MQTT Broker Restart | `e2e_mqtt_broker_restart_resilience` | Zero data loss |
| App Crash Recovery | `test_storage_wal_recovery_after_crash` | All WAL replayed |
| Invalid Data Handling | `test_pipeline_rejects_invalid_data` | DLQ capture |
| Partial Payload | `test_parser_to_validator_handles_partial_data` | Accepts None fields |
| Network Timeout | `test_mqtt_connection_resilience` | Exponential backoff |

---

## 6. Test Execution Plan

### 6.1 Test Phases

#### Phase 1: Fix Existing Integration Tests (Day 1)
**Goal:** Get 47/47 app tests passing

1. Wire `ParquetStore` to API routes
2. Fix `test_readings_time_range_query` (404 → 200)
3. Fix `test_aggregate_endpoint_mean`
4. Fix `test_forecast_endpoint`
5. Fix `test_alerts_endpoint`
6. Fix `test_latest_readings_endpoint_with_data`

**Verification:**
```bash
cargo test -p air-quality-app
# Expected: test result: ok. 47 passed; 0 failed
```

#### Phase 2: Integration Tests (Day 2)
**Goal:** Complete pipeline integration testing

1. Implement `parser_validator_test.rs`
2. Implement `validator_adapter_test.rs`
3. Implement `mqtt_parser_test.rs`
4. Implement `adapter_storage_test.rs`
5. Implement `pipeline_test.rs`

**Verification:**
```bash
cargo test -p air-quality-app --test integration
# Expected: 15+ integration tests passing
```

#### Phase 3: E2E Tests (Day 3)
**Goal:** Docker-based end-to-end validation

1. Create `docker-compose.test.yml`
2. Build sensor simulator image
3. Implement `e2e_full_system_smoke_test`
4. Implement `e2e_mqtt_broker_restart_resilience`
5. Implement `e2e_performance_100_messages_per_second`

**Verification:**
```bash
cargo test -p air-quality-app --ignored -- --test-threads=1
# Expected: E2E tests passing
```

### 6.2 Continuous Integration

**GitHub Actions:** `.github/workflows/air-quality-tests.yml`

```yaml
name: Air Quality Pipeline Tests

on:
  push:
    paths:
      - 'domains/air-quality/**'
      - 'apps/air-quality-app/**'
  pull_request:

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run unit tests
        run: cargo test -p air-quality -p air-quality-app

  integration-tests:
    runs-on: ubuntu-latest
    services:
      mosquitto:
        image: eclipse-mosquitto:2.0
        ports:
          - 1883:1883
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run integration tests
        run: cargo test -p air-quality-app --test integration

  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Start Docker stack
        run: docker-compose -f docker/air-quality/docker-compose.test.yml up -d
      - name: Wait for health
        run: ./scripts/wait-for-healthy.sh
      - name: Run E2E tests
        run: cargo test -p air-quality-app --ignored
```

---

## 7. Test Metrics & Reporting

### 7.1 Coverage Targets

| Layer | Target | Current | Gap |
|-------|--------|---------|-----|
| Domain Unit Tests | 100% | 100% ✅ | 0% |
| Integration Tests | 90% | 0% ❌ | **90%** |
| E2E Tests | 80% | 0% ❌ | **80%** |
| Overall | 90% | 60% ⚠️ | 30% |

### 7.2 Quality Gates

**All PRs must pass:**
- ✅ 100% unit tests passing
- ✅ 100% integration tests passing
- ✅ E2E smoke test passing
- ✅ Code coverage >90% for new code
- ✅ Zero clippy warnings
- ✅ Formatted with `cargo fmt`

### 7.3 Test Output Format

```bash
# Unit Tests
running 67 tests
test parser::tests::test_parse_mqtt_complete ... ok
test validator::tests::test_validate_co2_valid ... ok
test adapter::tests::test_to_time_series_points ... ok
...
test result: ok. 67 passed; 0 failed; 0 ignored

# Integration Tests
running 15 tests
test integration::parser_validator::test_chain_valid_payload ... ok
test integration::mqtt_parser::test_mqtt_to_parser_chain ... ok
test integration::pipeline::test_full_pipeline ... ok
...
test result: ok. 15 passed; 0 failed; 0 ignored

# E2E Tests (Docker)
running 3 tests
test e2e::test_full_system_smoke ... ok (30.2s)
test e2e::test_mqtt_restart_resilience ... ok (45.1s)
test e2e::test_performance_100msg_per_sec ... ok (75.3s)
...
test result: ok. 3 passed; 0 failed; 0 ignored
```

---

## 8. Risk Mitigation

### 8.1 Known Risks

| Risk | Impact | Mitigation | Test Coverage |
|------|--------|------------|---------------|
| MQTT broker unavailable | HIGH | Auto-reconnect, exponential backoff | `test_mqtt_connection_resilience` |
| Invalid sensor data | MEDIUM | Validation layer, DLQ | `test_pipeline_rejects_invalid_data` |
| WAL corruption | HIGH | Checksums, atomic writes | `test_storage_wal_recovery_after_crash` |
| Storage disk full | HIGH | Retention policy, alerts | Manual test (not automated) |
| High message rate | MEDIUM | Backpressure, bounded queues | `e2e_performance_100_messages_per_second` |

### 8.2 Test Environment Isolation

- Unit tests: In-memory, no I/O
- Integration tests: Mock MQTT broker (rumqttd), temp directories
- E2E tests: Docker Compose, isolated networks, separate ports

---

## 9. Next Steps

### Immediate Actions (Day 1)
1. ✅ **This document** - Test plan created
2. ⬜ Fix 5 failing app integration tests
3. ⬜ Wire `ParquetStore` to API routes
4. ⬜ Verify all 67 domain unit tests still pass

### Short-term (Days 2-3)
5. ⬜ Implement integration test suite (15+ tests)
6. ⬜ Create mock MQTT broker utility
7. ⬜ Build Docker test infrastructure
8. ⬜ Run E2E smoke test

### Medium-term (Week 2)
9. ⬜ Add performance benchmarks
10. ⬜ Set up CI/CD pipeline
11. ⬜ Document test fixtures
12. ⬜ Create test data generator

---

## 10. References

- [Test Coverage Summary](/workspaces/neural-data-platform/product/features/air-001/test-coverage-summary.md)
- [E2E Requirements](/workspaces/neural-data-platform/product/features/air-001/current-state/e2e-requirements/test-scenarios.md)
- [AIR-002 Specification](/workspaces/neural-data-platform/product/features/air-002/specs/01-specification.md)
- [London School TDD](https://www.codecademy.com/article/tdd-london-school)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)

---

**Document Status:** READY FOR IMPLEMENTATION
**Next Agent:** `specification` → `pseudocode` → `system-architect` → **`tester` (YOU ARE HERE)** → `coder`
**Approval Required:** Tech Lead sign-off on integration test strategy
