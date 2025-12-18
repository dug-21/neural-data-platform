//! JSON Path Parser Implementation
//!
//! Extracts specific fields from nested JSON structures using path
//! expressions. This parser is used for external APIs with complex
//! response formats (e.g., OpenWeatherMap).

use crate::error::{CoreError, CoreResult};
use crate::parsers::config::ParserConfig;
use crate::parsers::traits::Parser;
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tracing::{debug, warn};

/// Parser that extracts specific fields using JSON path expressions
#[derive(Debug)]
pub struct JsonPathParser {
    config: ParserConfig,
}

impl JsonPathParser {
    /// Create a new JsonPathParser from configuration
    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        // Validate that field mappings are provided
        if config.field_mappings.is_none() {
            return Err(CoreError::Config(
                "JsonPathParser requires field_mappings in configuration".into(),
            ));
        }

        Ok(Self { config })
    }

    /// Extract value at JSON path (e.g., "main.temp", "list[0].components.pm2_5")
    fn extract_at_path(&self, root: &Value, path: &str) -> Option<Value> {
        let mut current = root;

        for segment in path.split('.') {
            // Handle array access: field[0]
            if let Some(bracket_pos) = segment.find('[') {
                let field_name = &segment[..bracket_pos];
                let index_str = &segment[bracket_pos + 1..segment.len() - 1];
                let index: usize = index_str.parse().ok()?;

                current = current.get(field_name)?;
                current = current.get(index)?;
            } else {
                current = current.get(segment)?;
            }
        }

        Some(current.clone())
    }

    /// Extract numeric value from JSON value
    fn extract_numeric(value: &Value) -> Option<f64> {
        if let Some(num) = value.as_f64() {
            Some(num)
        } else if let Some(num) = value.as_i64() {
            Some(num as f64)
        } else if let Some(num) = value.as_u64() {
            Some(num as f64)
        } else {
            None
        }
    }

    /// Apply transformation to value (e.g., unit conversion)
    fn apply_transform(&self, value: f64, transform: &str) -> f64 {
        match transform {
            "kelvin_to_celsius" => value - 273.15,
            "kelvin_to_fahrenheit" => (value - 273.15) * 9.0 / 5.0 + 32.0,
            "mps_to_mph" => value * 2.237,
            "mps_to_kmh" => value * 3.6,
            _ => {
                warn!(
                    "Unknown transformation: {}, returning value unchanged",
                    transform
                );
                value
            }
        }
    }

    /// Extract location ID from payload
    fn extract_location_id(&self, payload: &Value) -> CoreResult<String> {
        // Try to extract using path
        if let Some(value) = self.extract_at_path(payload, &self.config.location_id_field) {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
            // If numeric, convert to string
            if let Some(num) = Self::extract_numeric(&value) {
                return Ok(num.to_string());
            }
        }

        // Try direct field access
        if let Some(value) = payload.get(&self.config.location_id_field) {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
        }

        // Fall back to default
        self.config
            .default_location_id
            .clone()
            .ok_or_else(|| CoreError::Source("Could not extract location ID".into()))
    }
}

impl Parser for JsonPathParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mappings = self
            .config
            .field_mappings
            .as_ref()
            .ok_or_else(|| CoreError::Source("No field mappings configured".into()))?;

        // Extract location ID
        let location_id = self.extract_location_id(payload)?;

        let mut points = Vec::new();

        // Process each mapping
        for mapping in mappings {
            // Extract value at path
            if let Some(value) = self.extract_at_path(payload, &mapping.path) {
                // Convert to numeric
                if let Some(mut numeric_value) = Self::extract_numeric(&value) {
                    // Apply transformation if configured
                    if let Some(transform) = &mapping.transform {
                        numeric_value = self.apply_transform(numeric_value, transform);
                    }

                    let mut tags = self.config.default_tags.clone();
                    tags.insert("metric".to_string(), mapping.metric_name.clone());

                    if let Some(unit) = &mapping.unit {
                        tags.insert("unit".to_string(), unit.clone());
                    }

                    points.push(TimeSeriesPoint {
                        timestamp,
                        location_id: location_id.clone(),
                        value: numeric_value,
                        tags,
                    });

                    debug!("Extracted {}: {}", mapping.metric_name, numeric_value);
                }
            }
        }

        if points.is_empty() {
            warn!("No fields extracted from payload");
        } else {
            debug!("Extracted {} fields", points.len());
        }

        Ok(points)
    }

    fn name(&self) -> &str {
        "json_path"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::config::{FieldMapping, ParserType};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_config_with_mappings(mappings: Vec<FieldMapping>) -> ParserConfig {
        let mut default_tags = HashMap::new();
        default_tags.insert("source".to_string(), "http".to_string());

        ParserConfig {
            parser_type: ParserType::JsonPath,
            location_id_field: "name".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: Some(mappings),
            default_tags,
        }
    }

    #[test]
    fn test_json_path_parser_extracts_nested_fields() {
        let mappings = vec![
            FieldMapping {
                path: "main.temp".to_string(),
                metric_name: "temperature".to_string(),
                unit: Some("celsius".to_string()),
                transform: None,
            },
            FieldMapping {
                path: "wind.speed".to_string(),
                metric_name: "wind_speed".to_string(),
                unit: Some("m/s".to_string()),
                transform: None,
            },
        ];

        let config = create_test_config_with_mappings(mappings);
        let parser = JsonPathParser::from_config(config).unwrap();

        let payload = json!({
            "name": "London",
            "main": {"temp": 20.5},
            "wind": {"speed": 3.5}
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 2);
    }

    #[test]
    fn test_json_path_parser_handles_array_access() {
        let mappings = vec![FieldMapping {
            path: "list[0].main.aqi".to_string(),
            metric_name: "aqi".to_string(),
            unit: Some("1-5_scale".to_string()),
            transform: None,
        }];

        let config = create_test_config_with_mappings(mappings);
        let parser = JsonPathParser::from_config(config).unwrap();

        let payload = json!({
            "name": "test",
            "list": [{"main": {"aqi": 2}}]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 2.0);
    }

    #[test]
    fn test_json_path_parser_applies_transformations() {
        let mappings = vec![FieldMapping {
            path: "main.temp".to_string(),
            metric_name: "temperature".to_string(),
            unit: Some("celsius".to_string()),
            transform: Some("kelvin_to_celsius".to_string()),
        }];

        let config = create_test_config_with_mappings(mappings);
        let parser = JsonPathParser::from_config(config).unwrap();

        let payload = json!({
            "name": "test",
            "main": {"temp": 293.15}
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert!((points[0].value - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_json_path_parser_name() {
        let config = create_test_config_with_mappings(vec![]);
        let parser = JsonPathParser::from_config(config).unwrap();
        assert_eq!(parser.name(), "json_path");
    }

    #[test]
    fn test_json_path_parser_no_mappings_error() {
        let mut config = create_test_config_with_mappings(vec![]);
        config.field_mappings = None;

        let result = JsonPathParser::from_config(config);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("field_mappings"));
        }
    }
}
