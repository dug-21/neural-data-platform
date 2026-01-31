//! Flat JSON Parser Implementation
//!
//! Extracts ALL numeric fields from a flat JSON object, preserving
//! original field names. This is the default parser for IoT sensors
//! that report multiple metrics in a single message.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;

use super::{Parser, ParserConfig};

pub struct FlatJsonParser {
    config: ParserConfig,
}

impl FlatJsonParser {
    pub fn new(config: ParserConfig) -> Self {
        Self { config }
    }

    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        Ok(Self::new(config))
    }

    fn extract_location_id(&self, obj: &serde_json::Map<String, Value>) -> CoreResult<String> {
        // Try to extract from configured field
        if let Some(value) = obj.get(&self.config.location_id_field) {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
        }

        // Fall back to default
        self.config.default_location_id.clone().ok_or_else(|| {
            CoreError::Parser(format!(
                "Location ID field '{}' not found and no default configured",
                self.config.location_id_field
            ))
        })
    }

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
}

impl Parser for FlatJsonParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        let obj = payload
            .as_object()
            .ok_or_else(|| CoreError::Parser("Payload is not a JSON object".to_string()))?;

        // Extract location ID from configured field
        let location_id = self.extract_location_id(obj)?;

        let mut points = Vec::new();

        for (key, value) in obj {
            // Skip non-metric fields
            if self.config.skip_fields.contains(key) {
                continue;
            }

            // Extract numeric values (f64, i64, u64)
            let numeric_value = Self::extract_numeric(value);

            if let Some(num) = numeric_value {
                let mut tags = self.config.default_tags.clone();
                tags.insert("metric".to_string(), key.clone());

                points.push(TimeSeriesPoint {
                    timestamp,
                    location_id: location_id.clone(),
                    value: num,
                    tags,
                    ndp_id: None,
                    context: None,
                });
            }
        }

        Ok(points)
    }

    fn name(&self) -> &str {
        "flat_json"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
    // parse_with_context() uses default trait implementation (AIR-009)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_flat_json_parser_extracts_all_numeric_fields() {
        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: None,
            skip_fields: vec!["serialno".to_string(), "firmware".to_string()],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({
            "serialno": "d83bda1cd074",
            "firmware": "3.4.1",
            "pm01": 1.0,
            "pm02": 2.17,
            "rco2": 396,
            "atmp": 22.1,
            "tvocIndex": 42
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should extract 5 numeric fields (skip serialno, firmware)
        assert_eq!(points.len(), 5);

        let metrics: Vec<&str> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        assert!(metrics.contains(&"pm01"));
        assert!(metrics.contains(&"pm02"));
        assert!(metrics.contains(&"rco2"));
        assert!(metrics.contains(&"atmp"));
        assert!(metrics.contains(&"tvocIndex"));

        // serialno and firmware should NOT be extracted
        assert!(!metrics.contains(&"serialno"));
        assert!(!metrics.contains(&"firmware"));
    }

    #[test]
    fn test_flat_json_parser_preserves_original_field_names() {
        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec!["serialno".to_string()],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({
            "serialno": "test",
            "rco2": 400.0,
            "atmp": 22.0,
            "rhum": 50.0
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        let metrics: Vec<&str> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // MUST preserve original names
        assert!(metrics.contains(&"rco2"));
        assert!(metrics.contains(&"atmp"));
        assert!(metrics.contains(&"rhum"));

        // Should NOT have renamed versions
        assert!(!metrics.contains(&"co2"));
        assert!(!metrics.contains(&"temperature"));
        assert!(!metrics.contains(&"humidity"));
    }

    #[test]
    fn test_flat_json_parser_default_tags() {
        let mut default_tags = HashMap::new();
        default_tags.insert("source".to_string(), "http".to_string());
        default_tags.insert("stream_id".to_string(), "test-stream".to_string());

        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("test".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags,
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({
            "serialno": "abc123",
            "pm02": 12.5
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("source"), Some(&"http".to_string()));
        assert_eq!(
            points[0].tags.get("stream_id"),
            Some(&"test-stream".to_string())
        );
        assert_eq!(points[0].tags.get("metric"), Some(&"pm02".to_string()));
    }

    // ========== AIR-009: parse_with_context TESTS (TDD Cycle 5-6) ==========

    #[test]
    fn test_flat_json_parser_injects_ndp_id_and_context() {
        use super::super::ParseContext;

        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: None,
            skip_fields: vec!["serialno".to_string()],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({
            "serialno": "d83bda1cd074",
            "pm02": 12.5
        });

        let context = ParseContext::new(
            Some("air-quality-office-001".to_string()),
            Some(json!({"room": "office", "floor": 2})),
        );

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert!(!points.is_empty());
        assert_eq!(points[0].ndp_id, Some("air-quality-office-001".to_string()));
        assert!(points[0].context.is_some());

        // Verify context content
        let ctx = points[0].context.as_ref().unwrap();
        assert_eq!(ctx["room"], "office");
        assert_eq!(ctx["floor"], 2);
    }

    #[test]
    fn test_flat_json_parser_injects_ndp_id_only() {
        use super::super::ParseContext;

        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("fallback".to_string()),
            skip_fields: vec!["serialno".to_string()],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({
            "serialno": "test123",
            "temperature": 22.5
        });

        let context = ParseContext::new(Some("sensor-123".to_string()), None);

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert!(!points.is_empty());
        assert_eq!(points[0].ndp_id, Some("sensor-123".to_string()));
        assert!(points[0].context.is_none());
    }

    #[test]
    fn test_flat_json_parser_injects_context_only() {
        use super::super::ParseContext;

        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("fallback".to_string()),
            skip_fields: vec!["serialno".to_string()],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({
            "serialno": "test123",
            "humidity": 65.0
        });

        let context = ParseContext::new(None, Some(json!({"location": "basement"})));

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert!(!points.is_empty());
        assert!(points[0].ndp_id.is_none());
        assert!(points[0].context.is_some());
        assert_eq!(points[0].context.as_ref().unwrap()["location"], "basement");
    }

    #[test]
    fn test_flat_json_parser_empty_context_passthrough() {
        use super::super::ParseContext;

        let config = ParserConfig {
            parser_type: super::super::ParserType::FlatJson,
            location_id_field: "id".to_string(),
            default_location_id: Some("test".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = json!({"value": 100.0});

        // Empty context should result in None values
        let context = ParseContext::default();

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert!(!points.is_empty());
        assert!(points[0].ndp_id.is_none());
        assert!(points[0].context.is_none());
    }
}
