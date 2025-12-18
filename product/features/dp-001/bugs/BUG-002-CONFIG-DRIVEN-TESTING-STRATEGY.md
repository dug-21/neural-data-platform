# BUG-002: Config-Driven Testing Strategy

**Status**: Draft
**Created**: 2025-12-18
**Purpose**: Prevent regression to hardcoded parsing logic

---

## Problem Statement

The SPARC documents cover unit and integration tests for the new parser implementations, but they don't address:

1. **How do we ensure config is ACTUALLY driving behavior?**
2. **How do we detect if someone introduces hardcoded logic?**
3. **How do we verify that unknown fields are captured (not dropped)?**

This document defines tests that would **FAIL if someone regresses to hardcoded parsing**.

---

## Test Categories

### 1. Config Binding Enforcement Tests

These tests verify that parsers are created FROM config, not bypassed.

```rust
//! tests/config_driven/parser_binding_tests.rs
//!
//! CONTRACT: Parsers MUST be created from config. Direct instantiation
//! with hardcoded values is a violation.

#[cfg(test)]
mod config_binding_tests {
    use neural_core::parsers::{Parser, ParserConfig, create_parser_from_config};
    use neural_core::sources::{MqttSource, HttpPollingSource};

    /// Test that MqttSource REQUIRES a parser to be injected
    /// This would fail if MqttSource has internal hardcoded parsing
    #[test]
    fn mqtt_source_requires_parser_injection() {
        // Attempting to create MqttSource without parser should not be possible
        // If this compiles with a default parser, it's a violation

        let config = MqttConfig::default();

        // This MUST require a parser parameter:
        // let source = MqttSource::new(config, parser); // Correct
        // let source = MqttSource::new(config);         // Should NOT compile

        // Verify at runtime that no default parser is used
        assert!(MqttSource::default_parser().is_none(),
            "MqttSource should not have a default parser - config must drive parsing");
    }

    /// Test that HttpPollingSource REQUIRES a parser registry
    #[test]
    fn http_source_requires_parser_registry() {
        let config = GenericHttpPollingConfig::default();

        // Parser registry must be provided, not created internally
        let result = GenericHttpPollingSource::new(config, ParserRegistry::new());
        assert!(result.is_ok());

        // If there's a with_default_parsers() method, it should be explicit opt-in
        // not automatic
    }

    /// Test that SourceManager creates parsers FROM stream config
    #[test]
    fn source_manager_creates_parser_from_config() {
        let yaml = r#"
            stream_id: test-stream
            sources:
              - source_type: mqtt
                parser:
                  parser_type: flat_json
                  location_id_field: device_id
                  skip_fields: [metadata]
        "#;

        let config = parse_stream_config(yaml);
        let source_config = &config.sources[0];

        // SourceManager MUST use the parser config from YAML
        let parser = create_parser_from_config(&source_config.parser).unwrap();

        // Verify parser was configured from YAML, not defaults
        assert_eq!(parser.config().location_id_field, "device_id");
        assert!(parser.config().skip_fields.contains("metadata"));
    }

    /// Test that missing parser config causes error, not fallback
    #[test]
    fn missing_parser_config_is_error_not_fallback() {
        let yaml = r#"
            stream_id: test-stream
            sources:
              - source_type: mqtt
                # NOTE: parser section is missing!
                params:
                  broker_url: localhost
        "#;

        let config = parse_stream_config(yaml);
        let source_config = &config.sources[0];

        // This MUST fail, not fall back to a default parser
        let result = SourceManager::spawn_source(source_config);
        assert!(result.is_err(),
            "Missing parser config should be an error, not use defaults");
    }
}
```

### 2. Field Extraction Contract Tests

These tests verify that field extraction is data-driven, not schema-driven.

```rust
//! tests/config_driven/field_extraction_tests.rs
//!
//! CONTRACT: Parsers MUST extract fields based on data, not hardcoded lists.
//! Adding a new field to JSON should NOT require code changes.

#[cfg(test)]
mod field_extraction_tests {
    use chrono::Utc;
    use serde_json::json;

    /// Test that FlatJsonParser extracts ALL numeric fields, not a predefined set
    #[test]
    fn flat_parser_extracts_unknown_fields() {
        let config = ParserConfig {
            parser_type: "flat_json".to_string(),
            location_id_field: "device_id".to_string(),
            skip_fields: vec!["device_id".to_string()],
            ..Default::default()
        };

        let parser = FlatJsonParser::from_config(config).unwrap();

        // JSON payload with a NEW field that wasn't in original design
        let payload = json!({
            "device_id": "sensor-001",
            "pm25": 12.5,
            "temperature": 22.0,
            "BRAND_NEW_FIELD": 42.0,  // <-- Not in any schema!
            "another_future_field": 99.9
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // CRITICAL: Unknown fields MUST be extracted
        let metrics: Vec<&str> = points.iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        assert!(metrics.contains(&"pm25"), "Known field not extracted");
        assert!(metrics.contains(&"temperature"), "Known field not extracted");
        assert!(metrics.contains(&"BRAND_NEW_FIELD"),
            "Unknown field dropped! This indicates hardcoded field list.");
        assert!(metrics.contains(&"another_future_field"),
            "Unknown field dropped! This indicates hardcoded field list.");

        // Verify we got all 4 numeric fields
        assert_eq!(points.len(), 4,
            "Expected 4 fields, got {}. Some fields were dropped.", points.len());
    }

    /// Test that adding a field to AirGradient payload doesn't require code changes
    #[test]
    fn airgradient_future_proofing() {
        let config = air_quality_parser_config();
        let parser = FlatJsonParser::from_config(config).unwrap();

        // Simulate AirGradient firmware adding new sensors
        let future_payload = json!({
            "serialno": "d83bda1cd074",
            "pm02": 12.5,
            "rco2": 600,
            "atmp": 22.0,
            "rhum": 45.0,
            "tvocIndex": 120,
            "noxIndex": 5,
            // New fields from firmware v4.0
            "pm01Compensated": 8.5,
            "co2Compensated": 580,
            "vocRawValue": 2500,
            "noxRawValue": 150,
            "audioDb": 45.5
        });

        let points = parser.parse(&future_payload, Utc::now()).unwrap();

        // ALL numeric fields must be captured
        let expected_fields = [
            "pm02", "rco2", "atmp", "rhum", "tvocIndex", "noxIndex",
            "pm01Compensated", "co2Compensated", "vocRawValue", "noxRawValue", "audioDb"
        ];

        let extracted: HashSet<&str> = points.iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        for field in expected_fields {
            assert!(extracted.contains(field),
                "Field '{}' was not extracted. Parser may be using hardcoded field list.", field);
        }
    }

    /// Test that non-numeric fields are correctly skipped (not silently dropped)
    #[test]
    fn non_numeric_fields_are_skipped_not_dropped() {
        let config = ParserConfig {
            parser_type: "flat_json".to_string(),
            location_id_field: "id".to_string(),
            skip_fields: vec!["id".to_string()],
            ..Default::default()
        };

        let parser = FlatJsonParser::from_config(config).unwrap();

        let payload = json!({
            "id": "test",
            "numeric_field": 42.0,
            "string_field": "should be skipped",
            "boolean_field": true,
            "object_field": {"nested": "value"}
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Only numeric field should be extracted
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric"), Some(&"numeric_field".to_string()));
    }
}
```

### 3. Config Change Propagation Tests

These tests verify that changing config actually changes behavior.

```rust
//! tests/config_driven/config_propagation_tests.rs
//!
//! CONTRACT: Changing config MUST change parser behavior.
//! If changing config doesn't change behavior, something is hardcoded.

#[cfg(test)]
mod config_propagation_tests {
    /// Test that skip_fields config actually affects extraction
    #[test]
    fn skip_fields_config_is_honored() {
        // Config WITHOUT skip_fields
        let config_no_skip = ParserConfig {
            parser_type: "flat_json".to_string(),
            location_id_field: "id".to_string(),
            skip_fields: vec!["id".to_string()], // Only skip id
            ..Default::default()
        };

        // Config WITH skip_fields
        let config_with_skip = ParserConfig {
            parser_type: "flat_json".to_string(),
            location_id_field: "id".to_string(),
            skip_fields: vec!["id".to_string(), "wifi".to_string(), "boot".to_string()],
            ..Default::default()
        };

        let parser_no_skip = FlatJsonParser::from_config(config_no_skip).unwrap();
        let parser_with_skip = FlatJsonParser::from_config(config_with_skip).unwrap();

        let payload = json!({
            "id": "sensor",
            "pm25": 12.5,
            "wifi": -67,
            "boot": 1234
        });

        let points_no_skip = parser_no_skip.parse(&payload, Utc::now()).unwrap();
        let points_with_skip = parser_with_skip.parse(&payload, Utc::now()).unwrap();

        // Without skip: 3 fields (pm25, wifi, boot)
        // With skip: 1 field (pm25 only)
        assert_eq!(points_no_skip.len(), 3,
            "Without skip config, should extract all 3 numeric fields");
        assert_eq!(points_with_skip.len(), 1,
            "With skip config, should only extract pm25");

        // Verify the difference in behavior
        assert_ne!(points_no_skip.len(), points_with_skip.len(),
            "Changing skip_fields config had no effect! Something is hardcoded.");
    }

    /// Test that location_id_field config is honored
    #[test]
    fn location_id_field_config_is_honored() {
        let config_serialno = ParserConfig {
            location_id_field: "serialno".to_string(),
            ..default_flat_config()
        };

        let config_device = ParserConfig {
            location_id_field: "device".to_string(),
            ..default_flat_config()
        };

        let parser_serialno = FlatJsonParser::from_config(config_serialno).unwrap();
        let parser_device = FlatJsonParser::from_config(config_device).unwrap();

        let payload = json!({
            "serialno": "ABC123",
            "device": "sensor-001",
            "value": 42.0
        });

        let points_serialno = parser_serialno.parse(&payload, Utc::now()).unwrap();
        let points_device = parser_device.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points_serialno[0].location_id, "ABC123");
        assert_eq!(points_device[0].location_id, "sensor-001");

        assert_ne!(points_serialno[0].location_id, points_device[0].location_id,
            "Changing location_id_field config had no effect! Location extraction is hardcoded.");
    }

    /// Test that JsonPathParser mappings are config-driven
    #[test]
    fn json_path_mappings_are_config_driven() {
        let config_temp_only = ParserConfig {
            parser_type: "json_path".to_string(),
            location_id: "test".to_string(),
            field_mappings: vec![
                FieldMapping { path: "$.main.temp".into(), metric: "temperature".into() }
            ],
            ..Default::default()
        };

        let config_all_fields = ParserConfig {
            parser_type: "json_path".to_string(),
            location_id: "test".to_string(),
            field_mappings: vec![
                FieldMapping { path: "$.main.temp".into(), metric: "temperature".into() },
                FieldMapping { path: "$.main.humidity".into(), metric: "humidity".into() },
                FieldMapping { path: "$.wind.speed".into(), metric: "wind_speed".into() },
            ],
            ..Default::default()
        };

        let parser_temp = JsonPathParser::from_config(config_temp_only).unwrap();
        let parser_all = JsonPathParser::from_config(config_all_fields).unwrap();

        let payload = json!({
            "main": {"temp": 22.5, "humidity": 65.0},
            "wind": {"speed": 3.5}
        });

        let points_temp = parser_temp.parse(&payload, Utc::now()).unwrap();
        let points_all = parser_all.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points_temp.len(), 1, "With one mapping, should get one point");
        assert_eq!(points_all.len(), 3, "With three mappings, should get three points");

        // Verify config change affected output
        assert_ne!(points_temp.len(), points_all.len(),
            "Changing field_mappings config had no effect! Extraction is hardcoded.");
    }
}
```

### 4. No Hardcoded Defaults Tests

These tests verify that there are no hidden default behaviors.

```rust
//! tests/config_driven/no_hardcoded_defaults_tests.rs
//!
//! CONTRACT: No hardcoded defaults should override config.

#[cfg(test)]
mod no_hardcoded_defaults_tests {
    /// Test that there are no hardcoded skip fields
    #[test]
    fn no_hardcoded_skip_fields() {
        // Create parser with EMPTY skip list
        let config = ParserConfig {
            parser_type: "flat_json".to_string(),
            location_id_field: "id".to_string(),
            skip_fields: vec![], // Explicitly empty!
            ..Default::default()
        };

        let parser = FlatJsonParser::from_config(config).unwrap();

        // AirGradient payload with typical "skip" fields
        let payload = json!({
            "id": "test",
            "wifi": -67,
            "boot": 1234,
            "firmware": "3.4.1",  // String, should be skipped anyway
            "model": "I-9PSL",    // String
            "ledMode": "co2",     // String
            "bootCount": 42,
            "pm02": 12.5
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Since skip_fields is empty, ALL numeric fields should be extracted
        let metrics: Vec<&str> = points.iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // If these are NOT extracted, there are hardcoded skip defaults
        assert!(metrics.contains(&"wifi"),
            "'wifi' was skipped despite empty skip_fields. Hardcoded skip list detected!");
        assert!(metrics.contains(&"boot"),
            "'boot' was skipped despite empty skip_fields. Hardcoded skip list detected!");
        assert!(metrics.contains(&"bootCount"),
            "'bootCount' was skipped despite empty skip_fields. Hardcoded skip list detected!");
        assert!(metrics.contains(&"pm02"),
            "'pm02' was not extracted. Something is wrong.");
    }

    /// Test that there are no hardcoded field name transformations
    #[test]
    fn no_hardcoded_field_transformations() {
        let config = default_flat_config();
        let parser = FlatJsonParser::from_config(config).unwrap();

        // Payload with AirGradient's ORIGINAL field names
        let payload = json!({
            "id": "test",
            "rco2": 600,      // NOT "co2"
            "atmp": 22.0,     // NOT "temperature"
            "rhum": 45.0,     // NOT "humidity"
            "tvocIndex": 120  // NOT "tvoc"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        let metrics: Vec<&str> = points.iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // Fields MUST preserve original names
        assert!(metrics.contains(&"rco2"),
            "Field 'rco2' was renamed! Parser is doing hardcoded field name transformation.");
        assert!(metrics.contains(&"atmp"),
            "Field 'atmp' was renamed! Parser is doing hardcoded field name transformation.");
        assert!(metrics.contains(&"rhum"),
            "Field 'rhum' was renamed! Parser is doing hardcoded field name transformation.");
        assert!(metrics.contains(&"tvocIndex"),
            "Field 'tvocIndex' was renamed! Parser is doing hardcoded field name transformation.");

        // These should NOT exist (they would indicate hardcoded renaming)
        assert!(!metrics.contains(&"co2"),
            "Field 'co2' found! Parser is doing hardcoded renaming from 'rco2'.");
        assert!(!metrics.contains(&"temperature"),
            "Field 'temperature' found! Parser is doing hardcoded renaming from 'atmp'.");
        assert!(!metrics.contains(&"humidity"),
            "Field 'humidity' found! Parser is doing hardcoded renaming from 'rhum'.");
    }
}
```

### 5. End-to-End Config Integration Tests

These tests verify the full flow from YAML config to stored data.

```rust
//! tests/config_driven/e2e_config_tests.rs
//!
//! CONTRACT: YAML config drives the entire ingestion pipeline.

#[cfg(test)]
mod e2e_config_tests {
    use std::fs;
    use tempfile::TempDir;

    /// Test that stream YAML config drives field extraction end-to-end
    #[tokio::test]
    async fn yaml_config_drives_field_extraction() {
        // Create temp config file
        let yaml = r#"
            stream_id: test-stream
            sources:
              - source_type: mqtt
                parser:
                  parser_type: flat_json
                  location_id_field: device_id
                  skip_fields:
                    - device_id
                    - metadata_field
                  source_tag: mqtt
                params:
                  broker_url: test
        "#;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-stream.yaml");
        fs::write(&config_path, yaml).unwrap();

        // Load config and create source
        let config = load_stream_config(&config_path).await.unwrap();
        let parser = create_parser_from_source_config(&config.sources[0]).unwrap();

        // Parse test payload
        let payload = json!({
            "device_id": "sensor-001",
            "metadata_field": 999,  // Should be skipped
            "pm25": 12.5,
            "temperature": 22.0
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Verify config was applied
        assert_eq!(points.len(), 2, "Should extract pm25 and temperature");
        assert_eq!(points[0].location_id, "sensor-001", "location_id_field should be honored");

        let metrics: Vec<&str> = points.iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        assert!(!metrics.contains(&"metadata_field"),
            "skip_fields config was not honored");
    }

    /// Test that changing YAML config changes stored data
    #[tokio::test]
    async fn yaml_config_change_affects_stored_data() {
        // This test would:
        // 1. Start with config that skips 'wifi'
        // 2. Send payload with wifi=-67
        // 3. Verify wifi is NOT in Parquet
        // 4. Update config to NOT skip 'wifi'
        // 5. Send same payload
        // 6. Verify wifi IS in Parquet

        // If changing config doesn't change stored data, something is hardcoded
    }
}
```

---

## Test Fixtures

### Sample Payloads

```rust
// tests/fixtures/payloads.rs

/// AirGradient payload with ALL current fields
pub const AIRGRADIENT_CURRENT: &str = r#"{
    "serialno": "d83bda1cd074",
    "wifi": -67,
    "boot": 0,
    "firmware": "3.4.1",
    "model": "I-9PSL",
    "ledMode": "co2",
    "bootCount": 123,
    "pm01": 1,
    "pm02": 2.17,
    "pm10": 2.33,
    "rco2": 396,
    "atmp": 22.1,
    "rhum": 65.13,
    "tvocIndex": 42,
    "noxIndex": 2,
    "tvocRaw": 25420,
    "noxRaw": 16325
}"#;

/// AirGradient payload with FUTURE fields (firmware v4.0)
pub const AIRGRADIENT_FUTURE: &str = r#"{
    "serialno": "d83bda1cd074",
    "pm01": 1,
    "pm02": 2.17,
    "pm10": 2.33,
    "pm01Compensated": 0.8,
    "pm02Compensated": 1.9,
    "pm10Compensated": 2.1,
    "rco2": 396,
    "atmp": 22.1,
    "rhum": 65.13,
    "tvocIndex": 42,
    "noxIndex": 2,
    "vocRawValue": 2500,
    "noxRawValue": 150,
    "ambientLight": 128,
    "audioDbPeak": 45.5
}"#;

/// OpenWeatherMap with additional fields
pub const OPENWEATHERMAP_EXTENDED: &str = r#"{
    "main": {
        "temp": 22.5,
        "feels_like": 21.8,
        "temp_min": 20.0,
        "temp_max": 25.0,
        "pressure": 1013,
        "humidity": 65,
        "sea_level": 1013,
        "grnd_level": 1010
    },
    "wind": {
        "speed": 3.5,
        "deg": 180,
        "gust": 5.2
    },
    "clouds": {"all": 20},
    "visibility": 10000,
    "dt": 1702900000
}"#;
```

---

## CI Integration

### Test Commands

```bash
# Run all config-driven tests
cargo test --test config_driven -- --test-threads=1

# Run specific category
cargo test config_binding
cargo test field_extraction
cargo test config_propagation
cargo test no_hardcoded

# Run with verbose output for debugging
cargo test field_extraction -- --nocapture
```

### CI Pipeline Addition

```yaml
# .github/workflows/ci.yml

jobs:
  config-drift-tests:
    name: Config-Driven Architecture Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run config-driven tests
        run: cargo test --test config_driven

      - name: Verify no hardcoded field lists
        run: |
          # Grep for suspicious patterns that indicate hardcoding
          ! grep -r "skip_fields.*=.*vec!\[.*\"wifi\"" core/src/
          ! grep -r "skip_fields.*=.*vec!\[.*\"boot\"" core/src/
          ! grep -r "\"rco2\".*=>.*\"co2\"" core/src/
          ! grep -r "\"atmp\".*=>.*\"temperature\"" core/src/
```

---

## Summary

| Test Category | Purpose | Detects |
|--------------|---------|---------|
| Config Binding | Verify parsers come from config | Hidden default parsers |
| Field Extraction | Verify dynamic field capture | Hardcoded field lists |
| Config Propagation | Verify config changes behavior | Ignored config values |
| No Hardcoded Defaults | Verify no hidden overrides | Secret default behaviors |
| E2E Config | Verify YAML → storage flow | Broken config pipeline |

**Key Principle**: These tests should **FAIL immediately** if someone introduces hardcoded parsing logic. The test failures will explain exactly what violation occurred.

---

## Files to Create

| Path | Purpose |
|------|---------|
| `tests/config_driven/mod.rs` | Module root |
| `tests/config_driven/parser_binding_tests.rs` | Config binding enforcement |
| `tests/config_driven/field_extraction_tests.rs` | Dynamic field extraction |
| `tests/config_driven/config_propagation_tests.rs` | Config change verification |
| `tests/config_driven/no_hardcoded_defaults_tests.rs` | No hidden defaults |
| `tests/config_driven/e2e_config_tests.rs` | End-to-end validation |
| `tests/fixtures/payloads.rs` | Sample test payloads |
