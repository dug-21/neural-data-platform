//! Config-Driven Parser Test Suite for BUG-002
//!
//! This test suite ensures parsers remain config-driven and prevents regression
//! to hardcoded parsing logic.
//!
//! See: product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-TESTING-STRATEGY.md
//!
//! ## IMPORTANT: Constructor Architecture
//!
//! As of BUG-002 fix, sources NO LONGER create default parsers automatically.
//! The old `MqttSource::new()` and `HttpPollingSource::new()` constructors
//! that created default parsers have been removed.
//!
//! **Required pattern:**
//! 1. Create parsers explicitly via `FlatJsonParser::from_config()` or `JsonPathParser::from_config()`
//! 2. Pass parsers to sources via `with_parsers()` or use `with_default_parsers()` helper
//!
//! This enforces the contract that ALL parsing behavior comes from configuration,
//! not hardcoded defaults.

mod fixtures;

use chrono::Utc;
use fixtures::payloads;
use platform_core::parsers::{
    config::{FieldMapping, ParserConfig, ParserType},
    flat_json::FlatJsonParser,
    json_path::JsonPathParser,
    traits::Parser,
};
use std::collections::HashMap;

// ============================================================================
// PARSER BINDING TESTS
// Contract: Parsers must be created FROM configuration, not with hardcoded defaults
// ============================================================================

/// CONTRACT: FlatJsonParser requires explicit configuration
/// BREAKS IF: Someone adds a Default impl that bypasses config
#[test]
fn flat_parser_requires_config() {
    // This test verifies that FlatJsonParser is constructed from config
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "device_id".to_string(),
        default_location_id: None,
        skip_fields: vec!["device_id".to_string(), "timestamp".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config);
    assert!(
        parser.is_ok(),
        "FlatJsonParser should be created from config"
    );
}

/// CONTRACT: JsonPathParser requires explicit field mappings in config
/// BREAKS IF: Someone adds default mappings for specific APIs
#[test]
fn json_path_parser_requires_mappings() {
    let config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "name".to_string(),
        default_location_id: Some("test-location".to_string()),
        skip_fields: vec![],
        field_mappings: Some(vec![FieldMapping {
            path: "main.temp".to_string(),
            metric_name: "temperature".to_string(),
            unit: Some("celsius".to_string()),
            transform: None,
        }]),
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = JsonPathParser::from_config(config);
    assert!(
        parser.is_ok(),
        "JsonPathParser should be created from config with mappings"
    );
}

// ============================================================================
// FIELD EXTRACTION TESTS
// Contract: Parsers must extract fields based on config, not hardcoded lists
// ============================================================================

/// CONTRACT: FlatJsonParser extracts ALL numeric fields from payload
/// BREAKS IF: Someone adds a hardcoded list of expected fields
#[test]
fn flat_parser_extracts_all_numeric_fields() {
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "id".to_string(),
        default_location_id: Some("test".to_string()),
        skip_fields: vec!["id".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config).unwrap();
    let payload = payloads::numeric_types();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();

    // Should extract all 7 numeric fields (all except "id" which is skipped)
    // integer_field, float_field, scientific_notation, large_number, negative, zero, zero_float
    assert!(
        points.len() >= 7,
        "Should extract ALL numeric fields. Got {} but expected at least 7",
        points.len()
    );
}

/// CONTRACT: FlatJsonParser extracts FUTURE fields without code changes
/// BREAKS IF: Parser only extracts known/expected fields
#[test]
fn flat_parser_extracts_unknown_future_fields() {
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config).unwrap();
    let payload = payloads::airgradient_future();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();
    let metrics: Vec<&str> = points
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    // These are "future" fields that don't exist in current firmware
    // A truly config-driven parser should extract them
    assert!(
        metrics.contains(&"soilMoisture"),
        "Parser should extract unknown field 'soilMoisture'"
    );
    assert!(
        metrics.contains(&"uvIndex"),
        "Parser should extract unknown field 'uvIndex'"
    );
    assert!(
        metrics.contains(&"pm01Compensated"),
        "Parser should extract unknown field 'pm01Compensated'"
    );
}

/// CONTRACT: FlatJsonParser extracts brand new fields from generic payloads
/// BREAKS IF: Parser has any built-in field expectations
#[test]
fn flat_parser_extracts_generic_unknown_fields() {
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "device_id".to_string(),
        default_location_id: None,
        skip_fields: vec!["device_id".to_string(), "timestamp".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config).unwrap();
    let payload = payloads::generic_unknown_fields();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();
    let metrics: Vec<&str> = points
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    // These fields have arbitrary names - not from any known API
    assert!(
        metrics.contains(&"brand_new_field"),
        "Parser should extract arbitrary field 'brand_new_field'"
    );
    assert!(
        metrics.contains(&"future_sensor_reading"),
        "Parser should extract arbitrary field 'future_sensor_reading'"
    );
    assert!(
        metrics.contains(&"experimental_metric"),
        "Parser should extract arbitrary field 'experimental_metric'"
    );
}

// ============================================================================
// CONFIG PROPAGATION TESTS
// Contract: Changing configuration must change parser behavior
// ============================================================================

/// CONTRACT: Changing skip_fields config actually affects extraction
/// BREAKS IF: skip_fields is ignored or has hidden defaults
#[test]
fn config_skip_fields_affects_extraction() {
    let config_with_skip = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec![
            "serialno".to_string(),
            "pm02".to_string(), // Skip a metric field
        ],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let config_without_skip = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec!["serialno".to_string()], // Only skip ID
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser_with_skip = FlatJsonParser::from_config(config_with_skip).unwrap();
    let parser_without_skip = FlatJsonParser::from_config(config_without_skip).unwrap();

    let payload = payloads::airgradient_current();
    let timestamp = Utc::now();

    let points_with_skip = parser_with_skip.parse(&payload, timestamp).unwrap();
    let points_without_skip = parser_without_skip.parse(&payload, timestamp).unwrap();

    let metrics_with_skip: Vec<&str> = points_with_skip
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();
    let metrics_without_skip: Vec<&str> = points_without_skip
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    assert!(
        !metrics_with_skip.contains(&"pm02"),
        "pm02 should be skipped when in skip_fields"
    );
    assert!(
        metrics_without_skip.contains(&"pm02"),
        "pm02 should be extracted when NOT in skip_fields"
    );
}

/// CONTRACT: Changing location_id_field config is honored
/// BREAKS IF: Location ID extraction is hardcoded
#[test]
fn config_location_id_field_is_honored() {
    let config_serialno = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config_serialno).unwrap();
    let payload = payloads::airgradient_current();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();

    // All points should have the location_id from the "serialno" field
    assert!(!points.is_empty(), "Should have parsed points");
    for point in &points {
        assert_eq!(
            point.location_id, "d83bda1cd074",
            "Location ID should come from configured field"
        );
    }
}

/// CONTRACT: default_tags config propagates to all points
/// BREAKS IF: Tags are hardcoded or ignored
#[test]
fn config_default_tags_propagate() {
    let mut tags = HashMap::new();
    tags.insert("source".to_string(), "test-source".to_string());
    tags.insert("stream_id".to_string(), "test-stream".to_string());

    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: tags,
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config).unwrap();
    let payload = payloads::airgradient_current();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();

    assert!(!points.is_empty(), "Should have parsed points");
    for point in &points {
        assert_eq!(
            point.tags.get("source"),
            Some(&"test-source".to_string()),
            "source tag should come from config"
        );
        assert_eq!(
            point.tags.get("stream_id"),
            Some(&"test-stream".to_string()),
            "stream_id tag should come from config"
        );
    }
}

// ============================================================================
// NO HARDCODED DEFAULTS TESTS
// Contract: Empty/minimal config should not activate hidden defaults
// ============================================================================

/// CONTRACT: Empty skip_fields extracts ALL fields (no hidden defaults)
/// BREAKS IF: Parser has hardcoded skip fields
#[test]
fn empty_skip_fields_extracts_all() {
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "id".to_string(),
        default_location_id: Some("test".to_string()),
        skip_fields: vec![], // EMPTY - should skip nothing
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config).unwrap();
    let payload = payloads::numeric_types();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();
    let metrics: Vec<&str> = points
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    // With empty skip_fields, ALL numeric fields should be extracted
    // including "id" if it were numeric (it's not in this case)
    assert!(
        points.len() >= 7,
        "Empty skip_fields should extract all numeric fields"
    );
}

/// CONTRACT: Field names are NOT transformed (preserved as-is)
/// BREAKS IF: Parser renames fields (e.g., rco2 -> co2)
#[test]
fn field_names_not_transformed() {
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: None,
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(config).unwrap();
    let payload = payloads::airgradient_current();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();
    let metrics: Vec<&str> = points
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    // Fields should use ORIGINAL names from payload
    assert!(
        metrics.contains(&"rco2"),
        "Field should be 'rco2' not 'co2'"
    );
    assert!(
        metrics.contains(&"atmp"),
        "Field should be 'atmp' not 'temperature'"
    );
    assert!(
        metrics.contains(&"rhum"),
        "Field should be 'rhum' not 'humidity'"
    );

    // These transformed names should NOT exist
    assert!(
        !metrics.contains(&"co2"),
        "Transformed name 'co2' should not exist"
    );
    assert!(
        !metrics.contains(&"temperature"),
        "Transformed name 'temperature' should not exist"
    );
    assert!(
        !metrics.contains(&"humidity"),
        "Transformed name 'humidity' should not exist"
    );
}

// ============================================================================
// JSON PATH PARSER TESTS
// Contract: JsonPathParser extracts fields based on configured paths
// ============================================================================

/// CONTRACT: JsonPathParser extracts nested fields via path config
/// BREAKS IF: Paths are hardcoded or ignored
#[test]
fn json_path_extracts_nested_fields() {
    let config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "name".to_string(),
        default_location_id: Some("test-location".to_string()),
        skip_fields: vec![],
        field_mappings: Some(vec![
            FieldMapping {
                path: "main.temp".to_string(),
                metric_name: "temperature".to_string(),
                unit: Some("celsius".to_string()),
                transform: None,
            },
            FieldMapping {
                path: "main.humidity".to_string(),
                metric_name: "humidity".to_string(),
                unit: Some("percent".to_string()),
                transform: None,
            },
            FieldMapping {
                path: "wind.speed".to_string(),
                metric_name: "wind_speed".to_string(),
                unit: Some("m/s".to_string()),
                transform: None,
            },
        ]),
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = JsonPathParser::from_config(config).unwrap();
    let payload = payloads::openweathermap_weather_full();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();
    let metrics: Vec<&str> = points
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    assert!(
        metrics.contains(&"temperature"),
        "Should extract temperature via path main.temp"
    );
    assert!(
        metrics.contains(&"humidity"),
        "Should extract humidity via path main.humidity"
    );
    assert!(
        metrics.contains(&"wind_speed"),
        "Should extract wind_speed via path wind.speed"
    );
}

/// CONTRACT: JsonPathParser extracts array elements via path config
/// BREAKS IF: Array indexing is not supported
#[test]
fn json_path_extracts_array_elements() {
    let config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "".to_string(),
        default_location_id: Some("test-location".to_string()),
        skip_fields: vec![],
        field_mappings: Some(vec![
            FieldMapping {
                path: "list[0].main.aqi".to_string(),
                metric_name: "aqi".to_string(),
                unit: None,
                transform: None,
            },
            FieldMapping {
                path: "list[0].components.pm2_5".to_string(),
                metric_name: "pm2_5".to_string(),
                unit: Some("ug/m3".to_string()),
                transform: None,
            },
        ]),
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = JsonPathParser::from_config(config).unwrap();
    let payload = payloads::openweathermap_air_pollution();
    let timestamp = Utc::now();

    let points = parser.parse(&payload, timestamp).unwrap();
    let metrics: Vec<&str> = points
        .iter()
        .filter_map(|p| p.tags.get("metric").map(|s| s.as_str()))
        .collect();

    assert!(
        metrics.contains(&"aqi"),
        "Should extract aqi via path list[0].main.aqi"
    );
    assert!(
        metrics.contains(&"pm2_5"),
        "Should extract pm2_5 via path list[0].components.pm2_5"
    );
}
