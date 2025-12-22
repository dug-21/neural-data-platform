//! Integration tests for Parser trait injection into HTTP sources
//!
//! Tests FR-012: GenericHttpPollingSource uses Parser trait

use platform_core::parsers::{
    create_parser_from_config, FlatJsonParser, JsonPathParser, ParserConfig, ParserType,
};
use platform_core::sources::{GenericHttpPollingConfig, GenericHttpPollingSource};
use platform_core::traits::Source;
use std::collections::HashMap;

/// Test that parser factory creates correct parser types
#[test]
fn test_parser_factory_creates_flat_json() {
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: Some("unknown".to_string()),
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = create_parser_from_config(config);
    assert!(parser.is_ok());

    let parser = parser.unwrap();
    assert_eq!(parser.name(), "flat_json");
}

#[test]
fn test_parser_factory_creates_json_path() {
    let config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "name".to_string(),
        default_location_id: Some("test".to_string()),
        skip_fields: vec![],
        field_mappings: Some(vec![platform_core::parsers::FieldMapping {
            path: "main.temp".to_string(),
            metric_name: "temperature".to_string(),
            unit: Some("celsius".to_string()),
            transform: None,
        }]),
        default_tags: HashMap::new(),
        array_config: None,
    };

    let parser = create_parser_from_config(config);
    assert!(parser.is_ok());

    let parser = parser.unwrap();
    assert_eq!(parser.name(), "json_path");
}

#[test]
fn test_parser_factory_unknown_parser_error() {
    let config = ParserConfig {
        parser_type: ParserType::Custom("unknown_parser".to_string()),
        location_id_field: "id".to_string(),
        default_location_id: None,
        skip_fields: vec![],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: None,
    };

    let result = create_parser_from_config(config);
    assert!(result.is_err());

    if let Err(e) = result {
        let error_msg = e.to_string();
        assert!(error_msg.contains("unknown_parser"));
        assert!(error_msg.contains("not registered"));
    }
}

/// Test that GenericHttpPollingSource accepts injected parser
#[tokio::test]
async fn test_generic_http_source_accepts_parser_injection() {
    // Create parser
    let parser_config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: Some("test_sensor".to_string()),
        skip_fields: vec!["serialno".to_string(), "firmware".to_string()],
        field_mappings: None,
        default_tags: [("source".to_string(), "http".to_string())].into(),
        array_config: None,
    };

    let parser = FlatJsonParser::from_config(parser_config).unwrap();

    // Create source with injected parser
    let http_config = GenericHttpPollingConfig::default();
    let source = GenericHttpPollingSource::new(http_config, Box::new(parser));

    assert!(source.is_ok());
    // Source creation successful - parser was injected properly
}

/// Test that different parser types can be injected
#[tokio::test]
async fn test_generic_http_source_accepts_json_path_parser() {
    // Create JsonPath parser
    let parser_config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "station_id".to_string(),
        default_location_id: Some("station1".to_string()),
        skip_fields: vec![],
        field_mappings: Some(vec![
            platform_core::parsers::FieldMapping {
                path: "temperature".to_string(),
                metric_name: "temp".to_string(),
                unit: Some("celsius".to_string()),
                transform: None,
            },
            platform_core::parsers::FieldMapping {
                path: "humidity".to_string(),
                metric_name: "hum".to_string(),
                unit: Some("percent".to_string()),
                transform: None,
            },
        ]),
        default_tags: [("source".to_string(), "http".to_string())].into(),
        array_config: None,
    };

    let parser = JsonPathParser::from_config(parser_config).unwrap();

    // Create source with JsonPath parser
    let http_config = GenericHttpPollingConfig::default();
    let source = GenericHttpPollingSource::new(http_config, Box::new(parser));

    assert!(source.is_ok());
    // Source creation successful with JsonPath parser
}

/// Test that parser factory integration works end-to-end
#[tokio::test]
async fn test_parser_factory_to_source_integration() {
    // Step 1: Create parser from config using factory
    let parser_config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: Some("test123".to_string()),
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: [("source".to_string(), "http".to_string())].into(),
        array_config: None,
    };

    let parser = create_parser_from_config(parser_config).expect("Parser creation failed");

    // Step 2: Inject parser into source
    let http_config = GenericHttpPollingConfig::default();
    let source = GenericHttpPollingSource::new(http_config, parser);

    assert!(source.is_ok());
    let source = source.unwrap();

    // Step 3: Verify source can be used normally
    let health = source.health_check().await.unwrap();
    assert!(!health.healthy); // Not running yet, but shouldn't error
}

/// Test backward compatibility with deprecated with_default_parsers
#[tokio::test]
#[allow(deprecated)]
async fn test_backward_compatibility_with_default_parsers() {
    let http_config = GenericHttpPollingConfig::default();
    let source = GenericHttpPollingSource::with_default_parsers(http_config);

    assert!(source.is_ok());
    // Deprecated method still works for backward compatibility
}
