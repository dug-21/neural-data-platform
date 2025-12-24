//! Parser factory for creating parsers from configuration
//!
//! Provides a factory function that creates the appropriate parser
//! implementation based on configuration.

use crate::error::{CoreError, CoreResult};
use crate::parsers::array_iterator::ArrayIteratorParser;
use crate::parsers::column_oriented::ColumnOrientedParser;
use crate::parsers::config::{ParserConfig, ParserType};
use crate::parsers::flat_json::FlatJsonParser;
use crate::parsers::json_path::JsonPathParser;
use crate::parsers::traits::Parser;

/// Create a parser instance from configuration
///
/// This factory function inspects the parser type in the configuration
/// and creates the appropriate parser implementation.
///
/// # Arguments
/// * `config` - Parser configuration
///
/// # Returns
/// A boxed Parser trait object with Send + Sync bounds, or CoreError if the parser type is unknown
pub fn create_parser_from_config(
    config: ParserConfig,
) -> CoreResult<Box<dyn Parser + Send + Sync>> {
    match config.parser_type {
        ParserType::FlatJson => {
            let parser = FlatJsonParser::from_config(config)?;
            Ok(Box::new(parser))
        }
        ParserType::JsonPath => {
            let parser = JsonPathParser::from_config(config)?;
            Ok(Box::new(parser))
        }
        ParserType::ArrayIterator => {
            let parser = ArrayIteratorParser::from_config(config)?;
            Ok(Box::new(parser))
        }
        ParserType::ColumnOriented => {
            let parser = ColumnOrientedParser::from_config(config)?;
            Ok(Box::new(parser))
        }
        ParserType::Custom(ref name) => {
            Err(CoreError::Config(format!(
                "Custom parser type '{}' not registered. Only built-in parsers (flat_json, json_path, array_iterator, column_oriented) are supported.",
                name
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::config::{ColumnMapping, ColumnOrientedConfig, FieldMapping, TimestampFormat};
    use std::collections::HashMap;

    #[test]
    fn test_create_flat_json_parser() {
        let config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec!["firmware".to_string()],
            field_mappings: None,
            array_config: None,
            column_config: None,
            default_tags: HashMap::new(),
        };

        let parser = create_parser_from_config(config).unwrap();
        assert_eq!(parser.name(), "flat_json");
    }

    #[test]
    fn test_create_json_path_parser() {
        let mut default_tags = HashMap::new();
        default_tags.insert("source".to_string(), "http".to_string());

        let config = ParserConfig {
            parser_type: ParserType::JsonPath,
            location_id_field: "name".to_string(),
            default_location_id: Some("test".to_string()),
            skip_fields: vec![],
            field_mappings: Some(vec![FieldMapping {
                path: "main.temp".to_string(),
                metric_name: "temperature".to_string(),
                unit: Some("celsius".to_string()),
                transform: None,
            }]),
            array_config: None,
            column_config: None,
            default_tags,
        };

        let parser = create_parser_from_config(config).unwrap();
        assert_eq!(parser.name(), "json_path");
    }

    #[test]
    fn test_json_path_parser_missing_mappings_error() {
        let config = ParserConfig {
            parser_type: ParserType::JsonPath,
            location_id_field: "name".to_string(),
            default_location_id: Some("test".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            array_config: None,
            column_config: None,
            default_tags: HashMap::new(),
        };

        let result = create_parser_from_config(config);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("field_mappings"));
        }
    }

    #[test]
    fn test_create_column_oriented_parser() {
        let column_config = ColumnOrientedConfig {
            metrics_base_path: "properties".to_string(),
            columns: vec![
                ColumnMapping {
                    metric_path: "temperature".to_string(),
                    field_name: "temperature".to_string(),
                    values_path: None,
                    timestamp_path: None,
                    value_path: None,
                },
                ColumnMapping {
                    metric_path: "dewpoint".to_string(),
                    field_name: "dewpoint".to_string(),
                    values_path: None,
                    timestamp_path: None,
                    value_path: None,
                },
            ],
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions: std::collections::HashMap::new(),
        };

        let config = ParserConfig {
            parser_type: ParserType::ColumnOriented,
            location_id_field: "properties.gridId".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            array_config: None,
            column_config: Some(column_config),
            default_tags: HashMap::new(),
        };

        let parser = create_parser_from_config(config).unwrap();
        assert_eq!(parser.name(), "column_oriented");
    }
}
