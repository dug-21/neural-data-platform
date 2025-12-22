//! NWS Configuration Compatibility Test
//!
//! Verifies that NWS stream YAML configurations can be deserialized
//! into ParserConfig structs correctly.

use platform_core::parsers::config::{ParserConfig, ParserType};
use platform_core::parsers::traits::Parser;
use std::fs;

#[test]
fn test_nws_observations_config_deserializes() {
    // Read the actual NWS observations config
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/base/streams/nws-observations/config.yaml"
    );

    let yaml_content = fs::read_to_string(yaml_path)
        .expect("Failed to read nws-observations config.yaml");

    let config: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
        .expect("Failed to parse YAML");

    // Extract the parser section
    let parser_yaml = config
        .get("sources")
        .and_then(|s| s.as_sequence())
        .and_then(|arr| arr.get(0))
        .and_then(|src| src.get("parser"))
        .expect("Could not find parser config in YAML");

    // Attempt to deserialize into ParserConfig
    let result: Result<ParserConfig, _> = serde_yaml::from_value(parser_yaml.clone());

    match result {
        Ok(parser_config) => {
            // Verify it parsed correctly
            assert!(matches!(parser_config.parser_type, ParserType::JsonPath));
            assert_eq!(parser_config.location_id_field, "properties.station");
            assert_eq!(parser_config.default_location_id, Some("ksgj".to_string()));
            assert!(parser_config.field_mappings.is_some());

            println!("✅ NWS observations config deserialized successfully");
        }
        Err(e) => {
            panic!("❌ Failed to deserialize nws-observations parser config: {}\n\nYAML:\n{}", e, serde_yaml::to_string(parser_yaml).unwrap());
        }
    }
}

#[test]
fn test_nws_forecast_hourly_config_deserializes() {
    // Read the actual NWS forecast hourly config
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/base/streams/nws-forecast-hourly/config.yaml"
    );

    let yaml_content = fs::read_to_string(yaml_path)
        .expect("Failed to read nws-forecast-hourly config.yaml");

    let config: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
        .expect("Failed to parse YAML");

    // Extract the parser section
    let parser_yaml = config
        .get("sources")
        .and_then(|s| s.as_sequence())
        .and_then(|arr| arr.get(0))
        .and_then(|src| src.get("parser"))
        .expect("Could not find parser config in YAML");

    // Attempt to deserialize into ParserConfig
    let result: Result<ParserConfig, _> = serde_yaml::from_value(parser_yaml.clone());

    match result {
        Ok(parser_config) => {
            // Verify it parsed correctly
            assert!(matches!(parser_config.parser_type, ParserType::ArrayIterator));
            assert_eq!(parser_config.location_id_field, "properties.gridId");
            assert_eq!(parser_config.default_location_id, Some("ksgj".to_string()));

            // Check if array_config is present
            if let Some(array_config) = parser_config.array_config {
                assert_eq!(array_config.array_path, "properties.periods");
                assert_eq!(array_config.timestamp_field, "startTime");
                assert!(!array_config.element_mappings.is_empty());
                assert!(!array_config.metadata_tags.is_empty());

                println!("✅ NWS forecast hourly config deserialized successfully");
                println!("   Array path: {}", array_config.array_path);
                println!("   Timestamp field: {}", array_config.timestamp_field);
                println!("   Element mappings: {}", array_config.element_mappings.len());
                println!("   Metadata tags: {}", array_config.metadata_tags.len());
            } else {
                // This is the problem - array_config is None
                println!("❌ COMPATIBILITY ISSUE:");
                println!("   ParserConfig deserialized but array_config is None");
                println!("   YAML has flat structure:");
                println!("{}", serde_yaml::to_string(parser_yaml).unwrap());
                println!("\n   But Rust expects nested structure:");
                println!("   parser:");
                println!("     parser_type: array_iterator");
                println!("     array_config:           # <-- NESTED HERE");
                println!("       array_path: ...");
                println!("       timestamp_field: ...");
                println!("       element_mappings: ...");
                panic!("array_config should be present for ArrayIterator parser");
            }
        }
        Err(e) => {
            panic!("❌ Failed to deserialize nws-forecast-hourly parser config: {}\n\nYAML:\n{}", e, serde_yaml::to_string(parser_yaml).unwrap());
        }
    }
}

#[test]
#[ignore] // Only run when build passes
fn test_create_array_iterator_parser_from_nws_config() {
    use platform_core::parsers::array_iterator::ArrayIteratorParser;

    // Read NWS forecast config
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/base/streams/nws-forecast-hourly/config.yaml"
    );

    let yaml_content = fs::read_to_string(yaml_path)
        .expect("Failed to read config");

    let config: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
        .expect("Failed to parse YAML");

    let parser_yaml = config
        .get("sources")
        .and_then(|s| s.as_sequence())
        .and_then(|arr| arr.get(0))
        .and_then(|src| src.get("parser"))
        .expect("Parser config not found");

    let parser_config: ParserConfig = serde_yaml::from_value(parser_yaml.clone())
        .expect("Failed to deserialize ParserConfig");

    // Try to create ArrayIteratorParser
    let parser = ArrayIteratorParser::from_config(parser_config)
        .expect("Failed to create ArrayIteratorParser from config");

    println!("✅ Successfully created ArrayIteratorParser from NWS config");
    println!("   Parser name: {}", parser.name());
}
