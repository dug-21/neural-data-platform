//! Raw Text Parser Implementation
//!
//! Parses plain text MQTT payloads (e.g., "on", "off", "42.5") from Home Assistant
//! and other systems that don't send JSON. The parser extracts ndp_id from the
//! MQTT topic using a configurable regex pattern.
//!
//! # AIR-012: Home Assistant Integration
//!
//! Home Assistant publishes state changes as plain text:
//! ```
//! Topic: homeassistant/binary_sensor/door_backslider/state
//! Payload: on
//! ```
//!
//! This parser handles such payloads by:
//! 1. Storing the raw payload text as the value
//! 2. Extracting ndp_id from the topic path using regex capture groups
//! 3. Generating ingestion timestamp
//!
//! # Configuration
//!
//! ```yaml
//! parser_type: raw_text
//! raw_text_config:
//!   # Regex with named capture group "ndp_id" to extract from topic
//!   ndp_id_regex: "homeassistant/[^/]+/(?P<ndp_id>[^/]+)/state"
//!   # Metric name for the extracted value
//!   metric_name: "state"
//!   # Whether to attempt numeric parsing (optional, default: false)
//!   parse_numeric: false
//! ```

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;

use super::{ParseContext, Parser, ParserConfig};

/// Configuration for raw text parsing
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RawTextConfig {
    /// Regex pattern with named capture group "ndp_id" to extract from topic
    /// Example: "homeassistant/[^/]+/(?P<ndp_id>[^/]+)/state"
    pub ndp_id_regex: Option<String>,

    /// Metric name for the extracted value (default: "value")
    #[serde(default = "default_metric_name")]
    pub metric_name: String,

    /// Whether to attempt parsing the payload as a numeric value
    /// If true and parsing succeeds, stores as f64
    /// If false or parsing fails, stores string in tags
    #[serde(default)]
    pub parse_numeric: bool,

    /// Value to use when payload is "on" (if parse_numeric is true)
    #[serde(default = "default_on_value")]
    pub on_value: f64,

    /// Value to use when payload is "off" (if parse_numeric is true)
    #[serde(default)]
    pub off_value: f64,
}

fn default_metric_name() -> String {
    "value".to_string()
}

fn default_on_value() -> f64 {
    1.0
}

impl Default for RawTextConfig {
    fn default() -> Self {
        Self {
            ndp_id_regex: None,
            metric_name: "value".to_string(),
            parse_numeric: false,
            on_value: 1.0,
            off_value: 0.0,
        }
    }
}

/// Parser for plain text MQTT payloads
///
/// Handles non-JSON payloads from Home Assistant and similar systems.
/// The MQTT source wraps raw text in a JSON structure before passing to this parser:
///
/// ```json
/// {
///   "_raw_text": "on",
///   "_topic": "homeassistant/binary_sensor/door_backslider/state"
/// }
/// ```
pub struct RawTextParser {
    config: ParserConfig,
    raw_text_config: RawTextConfig,
    ndp_id_regex: Option<Regex>,
}

impl RawTextParser {
    /// Create a new RawTextParser from configuration
    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        // Extract raw_text_config from the generic config
        // For now, use defaults if not provided
        let raw_text_config = RawTextConfig::default();

        let ndp_id_regex = if let Some(ref pattern) = raw_text_config.ndp_id_regex {
            Some(
                Regex::new(pattern).map_err(|e| {
                    CoreError::Config(format!("Invalid ndp_id_regex pattern: {}", e))
                })?,
            )
        } else {
            None
        };

        Ok(Self {
            config,
            raw_text_config,
            ndp_id_regex,
        })
    }

    /// Create a RawTextParser with explicit raw text configuration
    pub fn with_raw_text_config(
        config: ParserConfig,
        raw_text_config: RawTextConfig,
    ) -> CoreResult<Self> {
        let ndp_id_regex = if let Some(ref pattern) = raw_text_config.ndp_id_regex {
            Some(
                Regex::new(pattern).map_err(|e| {
                    CoreError::Config(format!("Invalid ndp_id_regex pattern: {}", e))
                })?,
            )
        } else {
            None
        };

        Ok(Self {
            config,
            raw_text_config,
            ndp_id_regex,
        })
    }

    /// Extract ndp_id from topic using configured regex
    fn extract_ndp_id_from_topic(&self, topic: &str) -> Option<String> {
        if let Some(ref regex) = self.ndp_id_regex {
            if let Some(caps) = regex.captures(topic) {
                if let Some(m) = caps.name("ndp_id") {
                    return Some(m.as_str().to_string());
                }
            }
        }
        None
    }

    /// Parse the raw text payload into a numeric value
    fn parse_value(&self, raw_text: &str) -> (f64, bool) {
        let trimmed = raw_text.trim();

        // Handle boolean-like values
        match trimmed.to_lowercase().as_str() {
            "on" | "true" | "yes" | "1" => return (self.raw_text_config.on_value, true),
            "off" | "false" | "no" | "0" => return (self.raw_text_config.off_value, true),
            _ => {}
        }

        // Try numeric parsing
        if self.raw_text_config.parse_numeric {
            if let Ok(num) = trimmed.parse::<f64>() {
                return (num, true);
            }
        }

        // Default: use 0.0 as value and store text in tags
        (0.0, false)
    }
}

impl Parser for RawTextParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // The MQTT source wraps raw text in this structure:
        // { "_raw_text": "on", "_topic": "homeassistant/..." }
        let raw_text = payload
            .get("_raw_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CoreError::Parser(
                    "RawTextParser expects payload with '_raw_text' field".to_string(),
                )
            })?;

        let topic = payload.get("_topic").and_then(|v| v.as_str());

        // Extract ndp_id from topic if regex is configured
        let topic_ndp_id = topic.and_then(|t| self.extract_ndp_id_from_topic(t));

        // Get location_id - prefer topic extraction, fall back to config default
        let location_id = topic_ndp_id
            .clone()
            .or_else(|| self.config.default_location_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Parse the value
        let (value, parsed_as_numeric) = self.parse_value(raw_text);

        // Build tags
        let mut tags = self.config.default_tags.clone();
        tags.insert(
            "metric".to_string(),
            self.raw_text_config.metric_name.clone(),
        );

        // If not parsed as numeric, store raw text in tags
        if !parsed_as_numeric {
            tags.insert("raw_value".to_string(), raw_text.to_string());
        }

        // Add topic as tag for debugging
        if let Some(t) = topic {
            tags.insert("topic".to_string(), t.to_string());
        }

        let point = TimeSeriesPoint {
            timestamp,
            location_id,
            value,
            tags,
            ndp_id: topic_ndp_id,
            context: None,
        };

        Ok(vec![point])
    }

    fn name(&self) -> &str {
        "raw_text"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }

    // parse_with_context() uses default trait implementation
    // The ndp_id from topic extraction takes precedence unless context provides one
    fn parse_with_context(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
        context: &ParseContext,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mut points = self.parse(payload, timestamp)?;

        // Context ndp_id takes precedence over topic extraction
        for point in &mut points {
            if context.ndp_id.is_some() {
                point.ndp_id = context.ndp_id.clone();
            }
            if context.context.is_some() {
                point.context = context.context.clone();
            }
        }

        Ok(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::ParserType;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_default_config() -> ParserConfig {
        ParserConfig {
            parser_type: ParserType::RawText,
            location_id_field: "topic".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            array_config: None,
            column_config: None,
            raw_text_config: None,
            default_tags: HashMap::new(),
        }
    }

    #[test]
    fn test_parse_on_off_payload() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            metric_name: "state".to_string(),
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        // Test "on" payload
        let payload = json!({
            "_raw_text": "on",
            "_topic": "homeassistant/binary_sensor/door/state"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 1.0);
        assert_eq!(points[0].tags.get("metric"), Some(&"state".to_string()));
    }

    #[test]
    fn test_parse_off_payload() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            metric_name: "state".to_string(),
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({
            "_raw_text": "off",
            "_topic": "homeassistant/binary_sensor/door/state"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 0.0);
    }

    #[test]
    fn test_extract_ndp_id_from_topic() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            ndp_id_regex: Some("homeassistant/[^/]+/(?P<ndp_id>[^/]+)/state".to_string()),
            metric_name: "state".to_string(),
            parse_numeric: true,
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({
            "_raw_text": "on",
            "_topic": "homeassistant/binary_sensor/door_backslider/state"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].location_id, "door_backslider");
        assert_eq!(points[0].ndp_id, Some("door_backslider".to_string()));
    }

    #[test]
    fn test_parse_numeric_string() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            metric_name: "temperature".to_string(),
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({
            "_raw_text": "22.5",
            "_topic": "sensors/temp/living_room"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 22.5);
    }

    #[test]
    fn test_non_numeric_stored_in_tags() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: false,
            metric_name: "status".to_string(),
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({
            "_raw_text": "unavailable",
            "_topic": "sensors/status"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 0.0);
        assert_eq!(
            points[0].tags.get("raw_value"),
            Some(&"unavailable".to_string())
        );
    }

    #[test]
    fn test_missing_raw_text_field_error() {
        let config = create_default_config();
        let parser = RawTextParser::from_config(config).unwrap();

        let payload = json!({
            "some_field": "value"
        });

        let result = parser.parse(&payload, Utc::now());
        assert!(result.is_err());
    }

    #[test]
    fn test_topic_stored_in_tags() {
        let config = create_default_config();
        let raw_config = RawTextConfig::default();
        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({
            "_raw_text": "on",
            "_topic": "homeassistant/switch/light/state"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(
            points[0].tags.get("topic"),
            Some(&"homeassistant/switch/light/state".to_string())
        );
    }

    #[test]
    fn test_default_location_id_fallback() {
        let mut config = create_default_config();
        config.default_location_id = Some("fallback-sensor".to_string());
        let raw_config = RawTextConfig::default();

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        // No ndp_id regex, so falls back to default
        let payload = json!({
            "_raw_text": "on",
            "_topic": "some/topic"
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points[0].location_id, "fallback-sensor");
    }

    #[test]
    fn test_context_ndp_id_takes_precedence() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            ndp_id_regex: Some("homeassistant/[^/]+/(?P<ndp_id>[^/]+)/state".to_string()),
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({
            "_raw_text": "on",
            "_topic": "homeassistant/binary_sensor/door_backslider/state"
        });

        let context = ParseContext::new(
            Some("override-ndp-id".to_string()),
            Some(json!({"room": "living_room"})),
        );

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        // Context ndp_id takes precedence
        assert_eq!(points[0].ndp_id, Some("override-ndp-id".to_string()));
        assert!(points[0].context.is_some());
    }

    #[test]
    fn test_true_false_values() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let true_payload = json!({ "_raw_text": "true" });
        let false_payload = json!({ "_raw_text": "false" });

        let true_points = parser.parse(&true_payload, Utc::now()).unwrap();
        let false_points = parser.parse(&false_payload, Utc::now()).unwrap();

        assert_eq!(true_points[0].value, 1.0);
        assert_eq!(false_points[0].value, 0.0);
    }

    #[test]
    fn test_yes_no_values() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let yes_payload = json!({ "_raw_text": "yes" });
        let no_payload = json!({ "_raw_text": "no" });

        let yes_points = parser.parse(&yes_payload, Utc::now()).unwrap();
        let no_points = parser.parse(&no_payload, Utc::now()).unwrap();

        assert_eq!(yes_points[0].value, 1.0);
        assert_eq!(no_points[0].value, 0.0);
    }

    #[test]
    fn test_custom_on_off_values() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            on_value: 100.0,
            off_value: -1.0,
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let on_payload = json!({ "_raw_text": "on" });
        let off_payload = json!({ "_raw_text": "off" });

        let on_points = parser.parse(&on_payload, Utc::now()).unwrap();
        let off_points = parser.parse(&off_payload, Utc::now()).unwrap();

        assert_eq!(on_points[0].value, 100.0);
        assert_eq!(off_points[0].value, -1.0);
    }

    #[test]
    fn test_whitespace_trimming() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let payload = json!({ "_raw_text": "  on  " });

        let points = parser.parse(&payload, Utc::now()).unwrap();
        assert_eq!(points[0].value, 1.0);
    }

    #[test]
    fn test_case_insensitive_boolean() {
        let config = create_default_config();
        let raw_config = RawTextConfig {
            parse_numeric: true,
            ..Default::default()
        };

        let parser = RawTextParser::with_raw_text_config(config, raw_config).unwrap();

        let on_upper = json!({ "_raw_text": "ON" });
        let on_mixed = json!({ "_raw_text": "On" });
        let true_upper = json!({ "_raw_text": "TRUE" });

        let on_upper_points = parser.parse(&on_upper, Utc::now()).unwrap();
        let on_mixed_points = parser.parse(&on_mixed, Utc::now()).unwrap();
        let true_upper_points = parser.parse(&true_upper, Utc::now()).unwrap();

        assert_eq!(on_upper_points[0].value, 1.0);
        assert_eq!(on_mixed_points[0].value, 1.0);
        assert_eq!(true_upper_points[0].value, 1.0);
    }
}
