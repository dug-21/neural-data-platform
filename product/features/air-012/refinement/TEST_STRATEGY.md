# air-012: Home Assistant Integration - Test Strategy

## Overview

This document defines the test strategy for air-012, the Home Assistant MQTT integration feature. Since this feature is primarily configuration-driven (stream config, dimension update, Silver schema) and leverages the **existing, well-tested MQTT source adapter**, the testing focus is on **integration testing** and **configuration validation**.

**Key Insight:** The MQTT source adapter (`core/src/sources/mqtt/`) has 40+ unit tests covering payload parsing, topic routing, subscription management, and connection handling. We do not need to re-test that functionality.

---

## 1. Test Pyramid for air-012

### Test Distribution

```
        /\
       /  \        End-to-End (5%)
      /____\       - Full pipeline verification
     /      \
    /________\     Integration (70%)
   /          \    - MQTT -> Bronze flow
  /__________  \   - Bronze -> Silver ETL
 /            \ \  - Pipeline health queries
/________________\
                   Config Validation (25%)
                   - Stream config parsing
                   - Dimension CSV validation
                   - Schema verification
```

### Rationale

| Level | Percentage | Focus |
|-------|------------|-------|
| **Unit Tests** | ~0% | MQTT adapter already tested; no new Rust code |
| **Config Validation** | ~25% | Stream config, dimension CSV, SQL schema |
| **Integration Tests** | ~70% | Data flow through Bronze -> Silver |
| **End-to-End** | ~5% | Full pipeline with real MQTT broker |

---

## 2. Unit Tests (If Needed)

### No New Unit Tests Required

The air-012 feature uses existing infrastructure:

- **MQTT Source Adapter**: 40+ tests in `core/src/sources/mqtt/mod.rs`
- **Topic Router**: Tests in `core/src/sources/mqtt/router.rs`
- **Subscription Config**: Tests in `core/src/sources/mqtt/subscription.rs`
- **Parquet Storage**: Tested via existing air-quality stream
- **Silver ETL**: Tested in `apps/silver-etl/tests/integration_tests.rs`

### Potential Unit Tests (Only If New Code Added)

If any new code is written, follow existing patterns:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_to_boolean_on() {
        // If we add a state parser function
        assert_eq!(parse_state("on"), Some(true));
        assert_eq!(parse_state("ON"), Some(true));
    }

    #[test]
    fn test_state_to_boolean_off() {
        assert_eq!(parse_state("off"), Some(false));
        assert_eq!(parse_state("OFF"), Some(false));
    }

    #[test]
    fn test_ndp_id_extraction_from_topic() {
        // homeassistant/binary_sensor/door_backslider/state -> door_backslider
        let topic = "homeassistant/binary_sensor/door_backslider/state";
        assert_eq!(extract_ndp_id(topic), "door_backslider");
    }
}
```

---

## 3. Configuration Validation Tests

### 3.1 Stream Config Validation

**Test File:** `tests/config/air_012_stream_config_test.rs` (or inline in existing test suite)

```rust
#[test]
fn test_home_assistant_stream_config_valid() {
    // Load and validate the stream config
    let config_path = "config/base/streams/home-assistant-state.yaml";
    let config = StreamConfig::from_file(config_path)
        .expect("Config should parse");

    assert_eq!(config.stream_id, "home-assistant-state");
    assert_eq!(config.source.source_type, SourceType::Mqtt);
}

#[test]
fn test_mqtt_subscription_topic_pattern() {
    let config = load_home_assistant_config();
    let mqtt_config = config.source.mqtt.expect("Should have MQTT config");

    let subs = mqtt_config.get_subscriptions();
    assert!(!subs.is_empty(), "Should have subscriptions");

    // Verify topic pattern
    let ha_sub = &subs[0];
    assert_eq!(ha_sub.topic_pattern, "homeassistant/binary_sensor/+/state");
}

#[test]
fn test_mqtt_broker_config() {
    let config = load_home_assistant_config();
    let mqtt_config = config.source.mqtt.unwrap();

    assert_eq!(mqtt_config.broker_url, "192.168.52.103");
    assert_eq!(mqtt_config.port, 1883);
}
```

### 3.2 Dimension CSV Validation

**Test File:** `tests/config/air_012_dimension_test.rs`

```rust
#[test]
fn test_entity_context_csv_has_new_sensors() {
    let csv_path = "data/dimensions/entity_context.csv";
    let contents = std::fs::read_to_string(csv_path)
        .expect("Should read CSV");

    // Verify required sensors are present
    assert!(contents.contains("door_backslider"), "Missing door_backslider");
    assert!(contents.contains("door_officewindow"), "Missing door_officewindow");
    assert!(contents.contains("door_dinettewindow"), "Missing door_dinettewindow");
}

#[test]
fn test_entity_context_csv_columns() {
    let csv_path = "data/dimensions/entity_context.csv";
    let mut rdr = csv::Reader::from_path(csv_path)
        .expect("Should open CSV");

    let headers = rdr.headers().expect("Should have headers");

    // Required columns
    assert!(headers.iter().any(|h| h == "ndp_id"));
    assert!(headers.iter().any(|h| h == "category"));
    assert!(headers.iter().any(|h| h == "friendly_name"));
    assert!(headers.iter().any(|h| h == "location_path"));
}

#[test]
fn test_entity_context_categories_valid() {
    let csv_path = "data/dimensions/entity_context.csv";
    let mut rdr = csv::Reader::from_path(csv_path)
        .expect("Should open CSV");

    for result in rdr.records() {
        let record = result.expect("Valid row");
        let category = &record[1]; // category column

        // Valid categories for air-012
        let valid_categories = ["door", "window", "sensor", "weather"];
        assert!(
            valid_categories.contains(&category),
            "Invalid category: {}",
            category
        );
    }
}
```

### 3.3 Silver Schema Validation

**Test File:** `tests/schema/air_012_silver_schema_test.sql`

```sql
-- Run with: psql -f tests/schema/air_012_silver_schema_test.sql

-- Test 1: Table exists
SELECT count(*) = 1 AS table_exists
FROM information_schema.tables
WHERE table_schema = 'silver'
  AND table_name = 'state_events';

-- Test 2: Required columns exist
SELECT
    count(*) = 5 AS all_columns_present
FROM information_schema.columns
WHERE table_schema = 'silver'
  AND table_name = 'state_events'
  AND column_name IN ('event_time', 'ndp_id', 'source_entity_id', 'state', 'dq_flags');

-- Test 3: Is hypertable
SELECT count(*) = 1 AS is_hypertable
FROM timescaledb_information.hypertables
WHERE hypertable_schema = 'silver'
  AND hypertable_name = 'state_events';

-- Test 4: Primary key is correct
SELECT count(*) = 2 AS pk_correct
FROM information_schema.key_column_usage
WHERE table_schema = 'silver'
  AND table_name = 'state_events'
  AND constraint_name LIKE '%pkey%';
```

---

## 4. Integration Tests

### 4.1 MQTT Message to Bronze Flow

**Test File:** `tests/integration/air_012_mqtt_to_bronze_test.rs`

```rust
#[tokio::test]
#[ignore] // Requires: MQTT broker (mock or real)
async fn test_mqtt_message_stored_in_bronze() {
    // Setup
    let mock_broker = start_mock_mqtt_broker().await;
    let config = load_test_stream_config("home-assistant-state");

    // Start source
    let mut source = MqttSource::with_raw_config(
        config.source.mqtt.unwrap(),
        Box::new(FlatJsonParser::default()),
        Some("home-assistant-state".to_string()),
        None,
        None,
    );
    source.start().await.expect("Should start");

    // Publish test message
    mock_broker.publish(
        "homeassistant/binary_sensor/door_backslider/state",
        "on"
    ).await;

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Fetch raw points (Bronze layer)
    let raw_points = source.fetch_raw_batch().await.expect("Should fetch");

    // Verify
    assert!(!raw_points.is_empty(), "Should capture raw payload");
    let point = &raw_points[0];
    assert_eq!(point.source_id, "home-assistant-state-Mqtt");
}

#[tokio::test]
#[ignore]
async fn test_mqtt_topic_metadata_captured() {
    // Setup and publish...

    let points = source.fetch().await.expect("Should fetch");
    let point = &points[0];

    // Verify topic is captured in tags
    assert!(point.tags.contains_key("topic"));
    assert!(point.tags.get("topic").unwrap().contains("door_backslider"));

    // Verify stream_id is set
    assert_eq!(point.tags.get("stream_id"), Some(&"home-assistant-state".to_string()));
}

#[tokio::test]
#[ignore]
async fn test_multiple_sensors_same_stream() {
    // Publish messages for 3 different sensors
    let topics = [
        "homeassistant/binary_sensor/door_backslider/state",
        "homeassistant/binary_sensor/door_officewindow/state",
        "homeassistant/binary_sensor/door_dinettewindow/state",
    ];

    for topic in topics {
        mock_broker.publish(topic, "off").await;
    }

    // Wait and fetch
    let points = source.fetch().await.expect("Should fetch");

    // All should have same stream_id
    for point in &points {
        assert_eq!(
            point.tags.get("stream_id"),
            Some(&"home-assistant-state".to_string())
        );
    }
}
```

### 4.2 Bronze to Silver ETL Flow

**Test File:** `tests/integration/air_012_silver_etl_test.rs`

```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB
async fn test_state_events_etl_basic() {
    // Setup: Bronze fixture with state event data
    let bronze_fixture = setup_state_events_bronze_fixture().await;
    let pg_conn = test_postgres_connection().await;

    // Execute ETL
    let etl_result = run_state_events_etl(&bronze_fixture, &pg_conn).await;
    assert!(etl_result.is_ok());

    // Verify Silver table has data
    let row_count: i64 = pg_conn
        .query_one("SELECT count(*) FROM silver.state_events", &[])
        .await
        .expect("Query")
        .get(0);

    assert!(row_count > 0, "Should have rows in Silver");
}

#[tokio::test]
#[ignore]
async fn test_state_events_schema_correct() {
    // Setup and ETL...

    // Query a row and verify structure
    let row = pg_conn
        .query_one(
            "SELECT event_time, ndp_id, source_entity_id, state, dq_flags
             FROM silver.state_events LIMIT 1",
            &[]
        )
        .await
        .expect("Query");

    // Verify types
    let _event_time: chrono::DateTime<chrono::Utc> = row.get("event_time");
    let ndp_id: String = row.get("ndp_id");
    let state: String = row.get("state");

    assert!(!ndp_id.is_empty());
    assert!(state == "on" || state == "off");
}

#[tokio::test]
#[ignore]
async fn test_source_entity_id_extracted() {
    // ETL with topic: homeassistant/binary_sensor/door_backslider/state
    // Expected source_entity_id: binary_sensor.door_backslider

    let row = pg_conn
        .query_one(
            "SELECT source_entity_id FROM silver.state_events
             WHERE ndp_id = 'door_backslider'",
            &[]
        )
        .await
        .expect("Query");

    let source_entity_id: String = row.get("source_entity_id");
    assert_eq!(source_entity_id, "binary_sensor.door_backslider");
}
```

### 4.3 Dimension Table Integration

**Test File:** `tests/integration/air_012_dimension_test.rs`

```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB
async fn test_dimension_sync_loads_sensors() {
    let pg_conn = test_postgres_connection().await;

    // Run dimension sync
    let result = std::process::Command::new("./deploy.sh")
        .arg("sync-dimensions")
        .output()
        .expect("Should run sync");

    assert!(result.status.success());

    // Verify dimension has the 3 sensors
    let count: i64 = pg_conn
        .query_one(
            "SELECT count(*) FROM silver.entity_context
             WHERE ndp_id IN ('door_backslider', 'door_officewindow', 'door_dinettewindow')",
            &[]
        )
        .await
        .expect("Query")
        .get(0);

    assert_eq!(count, 3, "Should have all 3 sensors");
}

#[tokio::test]
#[ignore]
async fn test_dimension_join_with_events() {
    // Verify JOIN works between state_events and entity_context
    let row = pg_conn
        .query_one(
            "SELECT e.ndp_id, e.state, ec.category, ec.friendly_name
             FROM silver.state_events e
             JOIN silver.entity_context ec ON e.ndp_id = ec.ndp_id
             WHERE e.ndp_id = 'door_backslider'",
            &[]
        )
        .await
        .expect("Query");

    let category: String = row.get("category");
    assert_eq!(category, "door");
}
```

### 4.4 Pipeline Health Integration

**Test File:** `tests/integration/air_012_pipeline_health_test.rs`

```rust
#[tokio::test]
#[ignore] // Requires: TimescaleDB + Grafana datasource
async fn test_pipeline_health_query_returns_freshness() {
    let pg_conn = test_postgres_connection().await;

    // Insert test data
    pg_conn.execute(
        "INSERT INTO silver.state_events (event_time, ndp_id, state)
         VALUES (NOW() - INTERVAL '1 hour', 'door_backslider', 'on')",
        &[]
    ).await.expect("Insert");

    // Run pipeline health query
    let freshness: f64 = pg_conn
        .query_one(
            "SELECT EXTRACT(EPOCH FROM (NOW() - MAX(event_time)))
             FROM silver.state_events
             WHERE ndp_id = 'door_backslider'",
            &[]
        )
        .await
        .expect("Query")
        .get(0);

    // Should be ~1 hour (3600 seconds) with some tolerance
    assert!(freshness >= 3500.0 && freshness <= 3700.0);
}

#[tokio::test]
#[ignore]
async fn test_sparse_data_threshold_no_false_alarm() {
    let pg_conn = test_postgres_connection().await;

    // Insert data from 12 hours ago (should be FRESH for sparse data)
    pg_conn.execute(
        "INSERT INTO silver.state_events (event_time, ndp_id, state)
         VALUES (NOW() - INTERVAL '12 hours', 'door_backslider', 'off')",
        &[]
    ).await.expect("Insert");

    // Check status using sparse data thresholds (18 hours = fresh)
    let hours_since_last: f64 = pg_conn
        .query_one(
            "SELECT EXTRACT(EPOCH FROM (NOW() - MAX(event_time))) / 3600
             FROM silver.state_events
             WHERE ndp_id = 'door_backslider'",
            &[]
        )
        .await
        .expect("Query")
        .get(0);

    // 12 hours < 18 hours threshold = FRESH (green)
    assert!(hours_since_last < 18.0, "Should be within fresh threshold");
}
```

---

## 5. Mock Strategy for MQTT Broker

### 5.1 Using rumqttd for Local Testing

```rust
// tests/helpers/mock_mqtt.rs

use rumqttd::{Broker, Config};
use std::net::SocketAddr;

pub struct MockMqttBroker {
    broker: Broker,
    client: AsyncClient,
}

impl MockMqttBroker {
    pub async fn start() -> Self {
        let config = Config {
            id: 0,
            router: rumqttd::RouterConfig::default(),
            v4: Some(vec![rumqttd::ServerSettings {
                listen: SocketAddr::from(([127, 0, 0, 1], 0)), // Random port
                ..Default::default()
            }]),
            ..Default::default()
        };

        let broker = Broker::new(config);
        let port = broker.v4_servers()[0].local_addr().port();

        // Create client to publish test messages
        let mqtt_options = MqttOptions::new("test-publisher", "127.0.0.1", port);
        let (client, _) = AsyncClient::new(mqtt_options, 10);

        Self { broker, client }
    }

    pub async fn publish(&self, topic: &str, payload: &str) {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload.as_bytes())
            .await
            .expect("Should publish");
    }

    pub fn port(&self) -> u16 {
        self.broker.v4_servers()[0].local_addr().port()
    }
}
```

### 5.2 Using Test Fixtures (No Broker)

For most tests, use pre-recorded fixtures instead of a live broker:

```rust
// tests/fixtures/home_assistant_messages.rs

pub fn door_open_message() -> RawDataPoint {
    RawDataPoint::new(
        "home-assistant-state-Mqtt".to_string(),
        serde_json::json!("on"),
    )
    .with_timestamp(Utc::now())
    .with_metadata(json!({
        "topic": "homeassistant/binary_sensor/door_backslider/state"
    }))
}

pub fn door_closed_message() -> RawDataPoint {
    RawDataPoint::new(
        "home-assistant-state-Mqtt".to_string(),
        serde_json::json!("off"),
    )
    .with_timestamp(Utc::now())
    .with_metadata(json!({
        "topic": "homeassistant/binary_sensor/door_backslider/state"
    }))
}

pub fn all_sensors_closed() -> Vec<RawDataPoint> {
    vec![
        door_closed_message_for("door_backslider"),
        door_closed_message_for("door_officewindow"),
        door_closed_message_for("door_dinettewindow"),
    ]
}
```

### 5.3 Integration Test with Real Broker

For CI/CD, optionally test against real Home Assistant broker:

```yaml
# .github/workflows/air-012-integration.yml
jobs:
  integration-test:
    runs-on: self-hosted  # Pi runner with Home Assistant access
    steps:
      - uses: actions/checkout@v4
      - name: Test MQTT connectivity
        run: |
          mosquitto_sub -h 192.168.52.103 -p 1883 \
            -t "homeassistant/binary_sensor/+/state" \
            -C 1 --timeout 5
      - name: Run integration tests
        env:
          MQTT_BROKER: "192.168.52.103"
          MQTT_PORT: "1883"
        run: cargo test -p air-quality-app air_012 -- --ignored
```

---

## 6. Manual Test Checklist

### 6.1 Pre-Deployment Verification

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Verify MQTT broker access | `mosquitto_sub` receives messages |
| 2 | Load stream config | `ndp stream list` shows `home-assistant-state` |
| 3 | Start data collection | Stream shows "running" status |
| 4 | Trigger sensor event | Open/close a door/window |
| 5 | Check Bronze layer | Parquet file created with event |
| 6 | Run Silver ETL | `state_events` table has row |
| 7 | Query with dimension | JOIN returns category/name |
| 8 | Check dashboard | Freshness widget shows green |

### 6.2 Sparse Data Threshold Verification

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Ensure no events for 12 hours | (Wait or use test data) |
| 2 | Check pipeline health | Status = FRESH (green, < 18h) |
| 3 | Wait/simulate 20 hours | Status = STALE (yellow, 18-36h) |
| 4 | Wait/simulate 40 hours | Status = CRITICAL (red, > 36h) |
| 5 | Trigger new event | Status returns to FRESH |

### 6.3 Error Handling Verification

| Scenario | Action | Expected Behavior |
|----------|--------|-------------------|
| Broker unavailable | Stop MQTT broker | Reconnect attempts with backoff |
| Invalid message | Publish non-JSON | Error logged, no crash |
| Unknown topic | Publish to random topic | Dead letter warning |
| DB unavailable | Stop TimescaleDB | ETL fails gracefully, retries |

---

## 7. Test Execution Commands

### Quick Validation (No Infrastructure)

```bash
# Config validation only
cargo test -p air-quality-app config::air_012

# Schema validation (requires psql)
psql -f tests/schema/air_012_silver_schema_test.sql
```

### Integration Tests (Requires Docker)

```bash
# Start test infrastructure
docker compose -f deploy/docker-compose.test.yml up -d

# Run all air-012 integration tests
cargo test air_012 -- --ignored

# Run specific test
cargo test test_state_events_etl_basic -- --ignored

# Cleanup
docker compose -f deploy/docker-compose.test.yml down -v
```

### End-to-End (Requires Pi + Home Assistant)

```bash
# From Pi with Home Assistant access
MQTT_BROKER=192.168.52.103 MQTT_PORT=1883 \
  cargo test air_012_e2e -- --ignored
```

---

## 8. Test Coverage Summary

### Components and Test Types

| Component | Config Validation | Integration | E2E |
|-----------|-------------------|-------------|-----|
| Stream config (`home-assistant-state.yaml`) | Yes | Yes | Yes |
| MQTT subscription (`binary_sensor/+/state`) | Yes | Yes | Yes |
| Bronze storage (Parquet) | No (existing) | Yes | Yes |
| Silver table (`state_events`) | Yes (SQL) | Yes | Yes |
| Dimension (`entity_context`) | Yes | Yes | Yes |
| Pipeline health (sparse thresholds) | Yes (SQL) | Yes | Yes |

### Existing Test Coverage Leveraged

| Component | Test Location | Status |
|-----------|---------------|--------|
| MQTT Source Adapter | `core/src/sources/mqtt/mod.rs` | 40+ tests |
| Topic Router | `core/src/sources/mqtt/router.rs` | 15+ tests |
| Subscription Config | `core/src/sources/mqtt/subscription.rs` | 10+ tests |
| MQTT Routing Integration | `apps/air-quality-app/tests/mqtt_routing_integration_test.rs` | 6 tests |
| Silver ETL | `apps/silver-etl/tests/integration_tests.rs` | 10 tests |

---

## 9. Test Checklist

Before marking air-012 testing complete:

- [ ] Stream config validates successfully
- [ ] MQTT topic pattern matches expected sensors
- [ ] Dimension CSV has all 3 sensors with correct columns
- [ ] Silver schema SQL executes without errors
- [ ] Integration: MQTT message reaches Bronze
- [ ] Integration: Bronze data promotes to Silver
- [ ] Integration: Dimension JOIN works correctly
- [ ] Integration: Pipeline health query returns data
- [ ] Manual: Real sensor event flows through pipeline
- [ ] Manual: Sparse data threshold works (no false alarms)
- [ ] CI configuration updated for air-012 tests

---

## 10. Related Documentation

- **SCOPE.md**: Feature requirements and acceptance criteria
- **ACCEPTANCE_TESTS.md**: Acceptance test scenarios mapped to criteria
- **dp-013 TEST_STRATEGY.md**: Pattern reference for CSV/dimension testing
- **AIR-005-TEST-DESIGN.md**: London School TDD principles
- **MQTT Source Tests**: `core/src/sources/mqtt/mod.rs`
