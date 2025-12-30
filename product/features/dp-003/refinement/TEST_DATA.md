# DP-003: Test Data and Fixtures

## Overview

This document defines test data, fixtures, and sample configurations for TDD implementation of the MQTT multi-subscription feature. All fixtures are designed to exercise edge cases and verify correct behavior.

---

## 1. Configuration Fixtures

### 1.1 Valid Multi-Subscription Configuration

**File**: `tests/fixtures/mqtt/config_multi_subscription.yaml`

```yaml
# Valid multi-subscription MQTT configuration
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "neural-data-platform-test"
      qos: 1
      reconnect_delay_ms: 1000
      max_reconnect_delay_ms: 30000
      buffer_capacity: 1000
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
          qos: 1
          parser:
            type: flat_json
            location_id_field: serialno
            default_location_id: unknown
            skip_fields:
              - serialno
              - firmware
              - model
              - ledMode

        - stream_id: homeassistant
          topic_pattern: "homeassistant/+/+/state"
          qos: 1
          parser:
            type: flat_json
            location_id_field: entity_id
            default_location_id: unknown_entity
            skip_fields:
              - entity_id
              - last_updated
              - last_changed
```

### 1.2 Legacy Single-Topic Configuration

**File**: `tests/fixtures/mqtt/config_legacy.yaml`

```yaml
# Legacy single-topic configuration (backward compatibility)
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "neural-data-platform-legacy"
      topic_pattern: "airgradient/readings/+"
      qos: 1
```

### 1.3 Invalid Configuration - Duplicate Stream IDs

**File**: `tests/fixtures/mqtt/config_invalid_duplicate_stream_id.yaml`

```yaml
# INVALID: Duplicate stream_id values
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "topic/a/+"

        - stream_id: air-quality  # DUPLICATE!
          topic_pattern: "topic/b/+"
```

### 1.4 Invalid Configuration - Missing Stream ID

**File**: `tests/fixtures/mqtt/config_invalid_missing_stream_id.yaml`

```yaml
# INVALID: Missing stream_id
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      subscriptions:
        - topic_pattern: "airgradient/+"  # Missing stream_id!
```

### 1.5 Invalid Configuration - Empty Topic Pattern

**File**: `tests/fixtures/mqtt/config_invalid_empty_topic.yaml`

```yaml
# INVALID: Empty topic_pattern
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      subscriptions:
        - stream_id: test-stream
          topic_pattern: ""  # Empty!
```

### 1.6 Invalid Configuration - Mixed Legacy and New Format

**File**: `tests/fixtures/mqtt/config_invalid_mixed_format.yaml`

```yaml
# INVALID: Cannot use both topic_pattern AND subscriptions
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      topic_pattern: "old/topic/+"  # Legacy format
      subscriptions:                 # AND new format!
        - stream_id: new-stream
          topic_pattern: "new/topic/+"
```

---

## 2. MQTT Message Fixtures

### 2.1 AirGradient Sensor Messages

**File**: `tests/fixtures/mqtt/airgradient_message_full.json`

```json
{
  "pm01": 0,
  "pm02": 2.17,
  "pm10": 2.33,
  "pm02Compensated": 1.27,
  "atmp": 22.1,
  "atmpCompensated": 22.1,
  "rhum": 65.13,
  "rhumCompensated": 65.13,
  "rco2": 396,
  "tvocIndex": 42,
  "tvocRaw": 31506.42,
  "noxIndex": 2,
  "noxRaw": 19013.92,
  "boot": 1568,
  "wifi": -29,
  "serialno": "d83bda1cd074",
  "firmware": "3.4.1",
  "model": "I-9PSL",
  "ledMode": "co2"
}
```

**File**: `tests/fixtures/mqtt/airgradient_message_minimal.json`

```json
{
  "pm02": 15.5,
  "serialno": "abc123"
}
```

**File**: `tests/fixtures/mqtt/airgradient_message_outdoor.json`

```json
{
  "pm01": 5,
  "pm02": 8.5,
  "pm10": 12.3,
  "atmp": 18.2,
  "rhum": 72.5,
  "serialno": "outdoor001",
  "firmware": "3.4.1",
  "model": "O-1PST"
}
```

### 2.2 HomeAssistant State Messages

**File**: `tests/fixtures/mqtt/homeassistant_sensor_state.json`

```json
{
  "entity_id": "sensor.living_room_temperature",
  "state": "21.5",
  "attributes": {
    "unit_of_measurement": "C",
    "friendly_name": "Living Room Temperature",
    "device_class": "temperature"
  },
  "last_changed": "2024-01-15T10:30:00+00:00",
  "last_updated": "2024-01-15T10:30:00+00:00"
}
```

**File**: `tests/fixtures/mqtt/homeassistant_binary_sensor_state.json`

```json
{
  "entity_id": "binary_sensor.front_door",
  "state": "on",
  "attributes": {
    "friendly_name": "Front Door",
    "device_class": "door"
  },
  "last_changed": "2024-01-15T10:25:00+00:00",
  "last_updated": "2024-01-15T10:25:00+00:00"
}
```

**File**: `tests/fixtures/mqtt/homeassistant_numeric_state.json`

```json
{
  "entity_id": "sensor.power_consumption",
  "state": "1523.45",
  "attributes": {
    "unit_of_measurement": "W",
    "friendly_name": "Power Consumption",
    "device_class": "power"
  }
}
```

### 2.3 Malformed Messages

**File**: `tests/fixtures/mqtt/malformed_json.txt`

```
not valid json {
```

**File**: `tests/fixtures/mqtt/malformed_partial.json`

```json
{"pm02": 15.5, "serialno":
```

**File**: `tests/fixtures/mqtt/malformed_wrong_types.json`

```json
{
  "pm02": "not-a-number",
  "serialno": 12345,
  "atmp": null
}
```

### 2.4 Edge Case Messages

**File**: `tests/fixtures/mqtt/message_empty_object.json`

```json
{}
```

**File**: `tests/fixtures/mqtt/message_only_metadata.json`

```json
{
  "serialno": "abc123",
  "firmware": "3.4.1",
  "model": "I-9PSL"
}
```

**File**: `tests/fixtures/mqtt/message_negative_values.json`

```json
{
  "pm02": 0.0,
  "atmp": -5.5,
  "wifi": -85,
  "serialno": "freezer001"
}
```

**File**: `tests/fixtures/mqtt/message_large_values.json`

```json
{
  "pm02": 999999.99,
  "rco2": 50000,
  "tvocRaw": 999999.99,
  "serialno": "extreme001"
}
```

**File**: `tests/fixtures/mqtt/message_unicode.json`

```json
{
  "pm02": 15.5,
  "serialno": "sensor_cafe",
  "notes": "Temperature in cafe"
}
```

---

## 3. Expected Output Fixtures

### 3.1 TimeSeriesPoint Structures

**AirGradient Full Message Output**

```rust
// Expected output for airgradient_message_full.json
// 15 TimeSeriesPoint entries (one per numeric field)
vec![
    TimeSeriesPoint {
        timestamp: /* test timestamp */,
        location_id: "d83bda1cd074".to_string(),
        value: 0.0,
        tags: HashMap::from([
            ("metric".to_string(), "pm01".to_string()),
            ("source".to_string(), "mqtt".to_string()),
            ("stream_id".to_string(), "air-quality".to_string()),
        ]),
    },
    TimeSeriesPoint {
        timestamp: /* test timestamp */,
        location_id: "d83bda1cd074".to_string(),
        value: 2.17,
        tags: HashMap::from([
            ("metric".to_string(), "pm02".to_string()),
            ("source".to_string(), "mqtt".to_string()),
            ("stream_id".to_string(), "air-quality".to_string()),
        ]),
    },
    // ... 13 more points for remaining numeric fields
]
```

### 3.2 Parquet Schema Verification

```sql
-- Expected Bronze layer schema (DuckDB query)
SELECT
    timestamp,      -- TIMESTAMP
    location_id,    -- VARCHAR
    value,          -- DOUBLE
    metric,         -- VARCHAR (from tags)
    source,         -- VARCHAR (from tags)
    stream_id       -- VARCHAR (from tags)
FROM read_parquet('data/bronze/air-quality/2024-01-15/*.parquet')
LIMIT 1;
```

**Expected Column Types**:

| Column | Type | Nullable | Notes |
|--------|------|----------|-------|
| timestamp | TIMESTAMP | No | UTC timestamp |
| location_id | VARCHAR | No | Sensor/entity ID |
| value | DOUBLE | No | Numeric value |
| metric | VARCHAR | No | Field name (pm02, atmp, etc.) |
| source | VARCHAR | No | Always "mqtt" |
| stream_id | VARCHAR | No | "air-quality" or "homeassistant" |

---

## 4. Topic Pattern Test Cases

### 4.1 Single-Level Wildcard (+) Test Data

```rust
// test_topic_routing_single_wildcard.rs

const PATTERN: &str = "airgradient/readings/+";

const SHOULD_MATCH: &[&str] = &[
    "airgradient/readings/abc123",
    "airgradient/readings/xyz789",
    "airgradient/readings/sensor-001",
    "airgradient/readings/d83bda1cd074",
];

const SHOULD_NOT_MATCH: &[&str] = &[
    "airgradient/readings/",           // Empty level
    "airgradient/readings",            // Missing level
    "airgradient/readings/a/b",        // Two levels
    "airgradient/other/abc123",        // Different path
    "other/readings/abc123",           // Different prefix
];
```

### 4.2 Multi-Level Wildcard (#) Test Data

```rust
// test_topic_routing_multi_wildcard.rs

const PATTERN: &str = "homeassistant/#";

const SHOULD_MATCH: &[&str] = &[
    "homeassistant/sensor/temp/state",
    "homeassistant/binary_sensor/door/state",
    "homeassistant/a/b/c/d/e/f",
    "homeassistant/light/living_room/state",
    "homeassistant",                   // Just prefix
];

const SHOULD_NOT_MATCH: &[&str] = &[
    "other/homeassistant/sensor",
    "home/assistant/sensor",
    "homeassistant2/sensor",
];
```

### 4.3 Mixed Wildcard Test Data

```rust
// test_topic_routing_mixed.rs

const PATTERN: &str = "homeassistant/+/+/state";

const SHOULD_MATCH: &[&str] = &[
    "homeassistant/sensor/temp/state",
    "homeassistant/binary_sensor/door/state",
    "homeassistant/light/living_room/state",
];

const SHOULD_NOT_MATCH: &[&str] = &[
    "homeassistant/sensor/state",           // Only 2 levels
    "homeassistant/sensor/temp/config",     // Different suffix
    "homeassistant/sensor/a/b/state",       // 3 levels in middle
];
```

---

## 5. Rust Test Data Constants

### 5.1 Test Constants Module

**File**: `core/src/sources/test_data.rs`

```rust
//! Test data constants for MQTT multi-subscription tests
//! Only compiled with #[cfg(test)]

#[cfg(test)]
pub mod test_data {
    /// AirGradient full payload JSON
    pub const AIRGRADIENT_FULL_JSON: &str = r#"{
        "pm01": 0,
        "pm02": 2.17,
        "pm10": 2.33,
        "pm02Compensated": 1.27,
        "atmp": 22.1,
        "atmpCompensated": 22.1,
        "rhum": 65.13,
        "rhumCompensated": 65.13,
        "rco2": 396,
        "tvocIndex": 42,
        "tvocRaw": 31506.42,
        "noxIndex": 2,
        "noxRaw": 19013.92,
        "boot": 1568,
        "wifi": -29,
        "serialno": "d83bda1cd074",
        "firmware": "3.4.1",
        "model": "I-9PSL",
        "ledMode": "co2"
    }"#;

    /// AirGradient minimal payload JSON
    pub const AIRGRADIENT_MINIMAL_JSON: &str = r#"{
        "pm02": 15.5,
        "serialno": "abc123"
    }"#;

    /// HomeAssistant sensor state JSON
    pub const HOMEASSISTANT_SENSOR_JSON: &str = r#"{
        "entity_id": "sensor.living_room_temperature",
        "state": "21.5",
        "attributes": {
            "unit_of_measurement": "C",
            "friendly_name": "Living Room Temperature"
        }
    }"#;

    /// HomeAssistant numeric state JSON
    pub const HOMEASSISTANT_NUMERIC_JSON: &str = r#"{
        "entity_id": "sensor.power_consumption",
        "state": "1523.45"
    }"#;

    /// Malformed JSON (invalid syntax)
    pub const MALFORMED_JSON: &str = "not valid json {";

    /// Empty JSON object
    pub const EMPTY_JSON: &str = "{}";

    /// JSON with only metadata (no numeric fields)
    pub const METADATA_ONLY_JSON: &str = r#"{
        "serialno": "abc123",
        "firmware": "3.4.1",
        "model": "I-9PSL"
    }"#;

    /// JSON with negative values
    pub const NEGATIVE_VALUES_JSON: &str = r#"{
        "pm02": 0.0,
        "atmp": -5.5,
        "wifi": -85,
        "serialno": "freezer001"
    }"#;

    /// Multi-subscription YAML config
    pub const MULTI_SUBSCRIPTION_CONFIG_YAML: &str = r#"
        broker_url: "localhost"
        port: 11883
        subscriptions:
          - stream_id: air-quality
            topic_pattern: "airgradient/readings/+"
          - stream_id: homeassistant
            topic_pattern: "homeassistant/+/+/state"
    "#;

    /// Legacy single-topic YAML config
    pub const LEGACY_CONFIG_YAML: &str = r#"
        broker_url: "localhost"
        port: 11883
        topic_pattern: "airgradient/readings/+"
    "#;

    /// Expected numeric field count for full AirGradient message
    pub const AIRGRADIENT_FULL_NUMERIC_FIELDS: usize = 15;

    /// Expected numeric field count for minimal AirGradient message
    pub const AIRGRADIENT_MINIMAL_NUMERIC_FIELDS: usize = 1;

    /// Test stream IDs
    pub mod stream_ids {
        pub const AIR_QUALITY: &str = "air-quality";
        pub const HOMEASSISTANT: &str = "homeassistant";
        pub const TEST_STREAM: &str = "test-stream";
    }

    /// Test topics
    pub mod topics {
        pub const AIR_QUALITY_ABC123: &str = "airgradient/readings/abc123";
        pub const AIR_QUALITY_XYZ789: &str = "airgradient/readings/xyz789";
        pub const HA_SENSOR_TEMP: &str = "homeassistant/sensor/temp/state";
        pub const HA_BINARY_DOOR: &str = "homeassistant/binary_sensor/door/state";
        pub const UNKNOWN_TOPIC: &str = "unknown/topic/path";
    }

    /// Test serial numbers / location IDs
    pub mod location_ids {
        pub const AIRGRADIENT_FULL: &str = "d83bda1cd074";
        pub const AIRGRADIENT_MINIMAL: &str = "abc123";
        pub const HA_TEMP_SENSOR: &str = "sensor.living_room_temperature";
        pub const HA_POWER_SENSOR: &str = "sensor.power_consumption";
        pub const DEFAULT: &str = "unknown";
    }
}
```

### 5.2 Test Data Loaders

```rust
// core/src/sources/test_data.rs (continued)

#[cfg(test)]
pub mod loaders {
    use serde_json::Value;
    use std::fs;
    use std::path::Path;

    /// Load JSON fixture from file
    pub fn load_json_fixture(filename: &str) -> Value {
        let fixture_path = format!("tests/fixtures/mqtt/{}", filename);
        let content = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|_| panic!("Failed to load fixture: {}", fixture_path));
        serde_json::from_str(&content)
            .unwrap_or_else(|_| panic!("Failed to parse JSON fixture: {}", fixture_path))
    }

    /// Load YAML fixture from file
    pub fn load_yaml_fixture<T: serde::de::DeserializeOwned>(filename: &str) -> T {
        let fixture_path = format!("tests/fixtures/mqtt/{}", filename);
        let content = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|_| panic!("Failed to load fixture: {}", fixture_path));
        serde_yaml::from_str(&content)
            .unwrap_or_else(|_| panic!("Failed to parse YAML fixture: {}", fixture_path))
    }

    /// Load raw text fixture
    pub fn load_raw_fixture(filename: &str) -> String {
        let fixture_path = format!("tests/fixtures/mqtt/{}", filename);
        fs::read_to_string(&fixture_path)
            .unwrap_or_else(|_| panic!("Failed to load fixture: {}", fixture_path))
    }
}
```

---

## 6. Performance Test Data

### 6.1 Bulk Message Generator

```rust
// tests/performance/mqtt_data_generator.rs

/// Generate bulk test messages for throughput testing
pub fn generate_airgradient_messages(count: usize, base_serial: &str) -> Vec<String> {
    (0..count)
        .map(|i| {
            format!(
                r#"{{
                    "pm02": {:.2},
                    "atmp": {:.1},
                    "rhum": {:.1},
                    "rco2": {},
                    "serialno": "{}-{}"
                }}"#,
                (i as f64 * 0.1) % 100.0,        // pm02: 0-100
                20.0 + (i as f64 * 0.01) % 10.0, // atmp: 20-30
                40.0 + (i as f64 * 0.1) % 40.0,  // rhum: 40-80
                400 + (i % 1000),                 // rco2: 400-1400
                base_serial,
                i
            )
        })
        .collect()
}

/// Generate bulk HomeAssistant messages
pub fn generate_homeassistant_messages(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| {
            format!(
                r#"{{
                    "entity_id": "sensor.test_{}",
                    "state": "{:.2}"
                }}"#,
                i,
                (i as f64 * 0.1) % 100.0
            )
        })
        .collect()
}
```

### 6.2 Large Payload Test

**File**: `tests/fixtures/mqtt/large_payload.json`

```json
{
  "pm01": 1,
  "pm02": 2.17,
  "pm10": 2.33,
  "pm02Compensated": 1.27,
  "pm01Raw": 0.5,
  "pm02Raw": 1.8,
  "pm10Raw": 2.1,
  "atmp": 22.1,
  "atmpCompensated": 22.1,
  "rhum": 65.13,
  "rhumCompensated": 65.13,
  "rco2": 396,
  "tvocIndex": 42,
  "tvocRaw": 31506.42,
  "noxIndex": 2,
  "noxRaw": 19013.92,
  "boot": 1568,
  "wifi": -29,
  "rssi": -45,
  "voltage": 3.3,
  "uptime": 86400,
  "freeHeap": 50000,
  "minFreeHeap": 40000,
  "pressure": 1013.25,
  "altitude": 100.5,
  "serialno": "stress-test-001",
  "firmware": "3.4.1",
  "model": "I-9PSL-EXTENDED",
  "ledMode": "co2",
  "timestamp": 1705312200,
  "extra_field_1": 1.1,
  "extra_field_2": 2.2,
  "extra_field_3": 3.3,
  "extra_field_4": 4.4,
  "extra_field_5": 5.5
}
```

---

## 7. Integration Test Scenarios

### 7.1 Multi-Stream Routing Scenario

```rust
/// Test scenario: Messages from two different sources routed to correct streams
pub struct MultiStreamRoutingScenario {
    pub air_quality_topic: String,
    pub air_quality_payload: String,
    pub homeassistant_topic: String,
    pub homeassistant_payload: String,
}

impl Default for MultiStreamRoutingScenario {
    fn default() -> Self {
        Self {
            air_quality_topic: "airgradient/readings/test123".to_string(),
            air_quality_payload: r#"{"pm02": 15.5, "serialno": "test123"}"#.to_string(),
            homeassistant_topic: "homeassistant/sensor/temp/state".to_string(),
            homeassistant_payload: r#"{"entity_id": "sensor.temp", "state": "21.5"}"#.to_string(),
        }
    }
}
```

### 7.2 Reconnection Scenario

```rust
/// Test scenario: Reconnection after broker restart
pub struct ReconnectionScenario {
    pub initial_messages: Vec<(String, String)>,
    pub post_reconnect_messages: Vec<(String, String)>,
}

impl Default for ReconnectionScenario {
    fn default() -> Self {
        Self {
            initial_messages: vec![
                (
                    "airgradient/readings/before".to_string(),
                    r#"{"pm02": 10.0, "serialno": "before"}"#.to_string(),
                ),
            ],
            post_reconnect_messages: vec![
                (
                    "airgradient/readings/after".to_string(),
                    r#"{"pm02": 20.0, "serialno": "after"}"#.to_string(),
                ),
            ],
        }
    }
}
```

---

## 8. Fixture Directory Structure

```
tests/fixtures/mqtt/
  # Configuration fixtures
  config_multi_subscription.yaml
  config_legacy.yaml
  config_invalid_duplicate_stream_id.yaml
  config_invalid_missing_stream_id.yaml
  config_invalid_empty_topic.yaml
  config_invalid_mixed_format.yaml

  # AirGradient message fixtures
  airgradient_message_full.json
  airgradient_message_minimal.json
  airgradient_message_outdoor.json

  # HomeAssistant message fixtures
  homeassistant_sensor_state.json
  homeassistant_binary_sensor_state.json
  homeassistant_numeric_state.json

  # Malformed message fixtures
  malformed_json.txt
  malformed_partial.json
  malformed_wrong_types.json

  # Edge case fixtures
  message_empty_object.json
  message_only_metadata.json
  message_negative_values.json
  message_large_values.json
  message_unicode.json

  # Performance fixtures
  large_payload.json

  # Infrastructure config
  mosquitto.conf
  passwd
```

---

## 9. Expected Test Results

### 9.1 Config Parsing Results

| Config File | Expected Result | Error Message (if any) |
|-------------|-----------------|------------------------|
| `config_multi_subscription.yaml` | Success | - |
| `config_legacy.yaml` | Success (auto-converts) | - |
| `config_invalid_duplicate_stream_id.yaml` | Error | "duplicate stream_id: air-quality" |
| `config_invalid_missing_stream_id.yaml` | Error | "stream_id is required" |
| `config_invalid_empty_topic.yaml` | Error | "topic_pattern cannot be empty" |
| `config_invalid_mixed_format.yaml` | Error | "cannot use both topic_pattern and subscriptions" |

### 9.2 Message Parsing Results

| Message File | Expected Points | Location ID |
|--------------|-----------------|-------------|
| `airgradient_message_full.json` | 15 | d83bda1cd074 |
| `airgradient_message_minimal.json` | 1 | abc123 |
| `homeassistant_sensor_state.json` | 1 | sensor.living_room_temperature |
| `malformed_json.txt` | Error | - |
| `message_empty_object.json` | 0 | - |
| `message_only_metadata.json` | 0 | - |
| `message_negative_values.json` | 3 | freezer001 |

### 9.3 Topic Routing Results

| Topic | Expected Stream ID |
|-------|-------------------|
| `airgradient/readings/abc123` | air-quality |
| `airgradient/readings/xyz789` | air-quality |
| `homeassistant/sensor/temp/state` | homeassistant |
| `homeassistant/binary_sensor/door/state` | homeassistant |
| `unknown/topic/path` | None (unmatched) |

---

## 10. Summary

### Fixture Categories

| Category | File Count | Purpose |
|----------|------------|---------|
| Valid configs | 2 | Happy path testing |
| Invalid configs | 4 | Error handling |
| AirGradient messages | 3 | Parser testing |
| HomeAssistant messages | 3 | Multi-format testing |
| Malformed messages | 3 | Error recovery |
| Edge cases | 5 | Boundary testing |
| Performance | 1 | Load testing |
| Infrastructure | 2 | Test broker setup |
| **Total** | **23** | |

### Test Data Constants

| Constant Category | Count | Notes |
|-------------------|-------|-------|
| JSON payloads | 8 | Inline constants |
| Config YAML | 2 | Inline constants |
| Stream IDs | 3 | Named constants |
| Topics | 5 | Named constants |
| Location IDs | 5 | Named constants |

---

## References

- TEST_SCAFFOLDING.md - Test module organization
- MOCK_SETUP.md - Mock implementations
- TEST_CASES.md - Detailed test cases
- `core/src/sources/mqtt.rs` - Existing parser implementation
