//! Array Iterator Parser Implementation
//!
//! Iterates over JSON arrays to produce multiple TimeSeriesPoints per element.
//! Used for parsing forecast data where each period produces multiple metrics.
//!
//! Features:
//! - Array path navigation (JSONPath-like)
//! - Element iteration producing multiple TimeSeriesPoints
//! - String parsing with regex patterns (e.g., "15 mph" → 15.0)
//! - Enum mapping (e.g., cardinal directions N→0, NE→45)
//! - Metadata tags from shared response fields

use crate::error::{CoreError, CoreResult};
use crate::parsers::config::ParserConfig;
use crate::parsers::traits::Parser;
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Configuration for array iterator parser
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ArrayIteratorConfig {
    /// Path to the array in the JSON (e.g., "properties.periods")
    pub array_path: String,

    /// Field in each array element to use as timestamp (e.g., "startTime")
    pub timestamp_field: String,

    /// Metadata tags to extract from root/shared fields
    #[serde(default)]
    pub metadata_tags: Vec<MetadataTagMapping>,

    /// Metadata metrics to emit as metric rows (e.g., issue_time)
    #[serde(default)]
    pub metadata_metrics: Vec<MetadataMetricMapping>,

    /// Element field mappings
    pub element_mappings: Vec<ElementMapping>,
}

/// Metadata tag extracted from shared response fields
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MetadataTagMapping {
    /// JSON path to the metadata value (e.g., "properties.generatedAt")
    pub path: String,

    /// Tag name to store the value under
    pub tag_name: String,
}

/// Metadata metric extracted from shared response fields (emitted as a metric row)
///
/// Unlike metadata_tags which are stored in tags (and may not be persisted),
/// metadata_metrics emit actual metric rows with the value stored in the value column.
/// This is useful for timestamps that need to be queryable for analytics.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MetadataMetricMapping {
    /// JSON path to the metadata value (e.g., "properties.updateTime")
    pub path: String,

    /// Metric name for the emitted TimeSeriesPoint
    pub metric_name: String,

    /// Value type determines how to extract the numeric value
    #[serde(default)]
    pub value_type: MetadataValueType,

    /// Optional unit for the metric
    #[serde(default)]
    pub unit: Option<String>,
}

/// How to extract a numeric value from a metadata field
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataValueType {
    /// Parse as ISO 8601 timestamp, emit epoch seconds
    #[default]
    Timestamp,
    /// Parse as direct numeric value
    Numeric,
}

/// Mapping configuration for extracting a field from each array element
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ElementMapping {
    /// JSON path within the element (e.g., "temperature", "windSpeed")
    pub path: String,

    /// Metric name for the TimeSeriesPoint
    pub metric_name: String,

    /// Optional unit for the metric
    #[serde(default)]
    pub unit: Option<String>,

    /// Optional string parse pattern (regex with capture group)
    #[serde(default)]
    pub string_parse: Option<StringParseConfig>,

    /// Optional enum mapping (string → numeric)
    #[serde(default)]
    pub enum_map: Option<HashMap<String, f64>>,

    /// Whether this field is optional (skip if missing)
    #[serde(default)]
    pub optional: bool,
}

/// String parsing configuration using regex
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StringParseConfig {
    /// Regex pattern to match (e.g., r"^(\d+)\s*(?:to\s*(\d+)\s*)?mph$")
    pub pattern: String,

    /// Capture group index to extract (1-based)
    pub capture_group: usize,

    /// Optional second capture group for range averaging
    #[serde(default)]
    pub capture_group_high: Option<usize>,
}

/// Parser that iterates over JSON arrays to produce multiple TimeSeriesPoints
#[derive(Debug)]
pub struct ArrayIteratorParser {
    config: ParserConfig,
    array_config: ArrayIteratorConfig,
}

impl ArrayIteratorParser {
    /// Create a new ArrayIteratorParser from configuration
    ///
    /// Extracts array_config from ParserConfig.array_config field.
    /// Returns error if array_config is not present.
    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        let array_config = config.array_config.clone().ok_or_else(|| {
            CoreError::Config(
                "ArrayIteratorParser requires 'array_config' in ParserConfig".to_string(),
            )
        })?;

        Ok(Self {
            config,
            array_config,
        })
    }

    /// Create from explicit configs (for testing)
    #[cfg(test)]
    pub fn from_configs(
        config: ParserConfig,
        array_config: ArrayIteratorConfig,
    ) -> CoreResult<Self> {
        Ok(Self {
            config,
            array_config,
        })
    }

    /// Extract value at JSON path (supports dot notation and array access)
    fn extract_at_path<'a>(&self, root: &'a Value, path: &str) -> Option<&'a Value> {
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

        Some(current)
    }

    /// Extract array from JSON at specified path
    fn extract_array<'a>(&self, payload: &'a Value) -> CoreResult<&'a Vec<Value>> {
        let array_value = self
            .extract_at_path(payload, &self.array_config.array_path)
            .ok_or_else(|| {
                CoreError::Source(format!(
                    "Array not found at path: {}",
                    self.array_config.array_path
                ))
            })?;

        array_value.as_array().ok_or_else(|| {
            CoreError::Source(format!(
                "Value at path '{}' is not an array",
                self.array_config.array_path
            ))
        })
    }

    /// Extract timestamp from element
    fn extract_element_timestamp(&self, element: &Value) -> CoreResult<DateTime<Utc>> {
        let timestamp_value = self
            .extract_at_path(element, &self.array_config.timestamp_field)
            .ok_or_else(|| {
                CoreError::Source(format!(
                    "Timestamp field '{}' not found in element",
                    self.array_config.timestamp_field
                ))
            })?;

        let timestamp_str = timestamp_value
            .as_str()
            .ok_or_else(|| CoreError::Source("Timestamp value is not a string".to_string()))?;

        DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| CoreError::Source(format!("Invalid RFC3339 timestamp: {}", e)))
    }

    /// Extract metadata tags from shared fields
    fn extract_metadata_tags(&self, payload: &Value) -> HashMap<String, String> {
        let mut tags = HashMap::new();

        for metadata_mapping in &self.array_config.metadata_tags {
            if let Some(value) = self.extract_at_path(payload, &metadata_mapping.path) {
                let value_str = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => format!("{}", value),
                };
                tags.insert(metadata_mapping.tag_name.clone(), value_str);
            } else {
                warn!(
                    "Metadata path '{}' not found, skipping tag '{}'",
                    metadata_mapping.path, metadata_mapping.tag_name
                );
            }
        }

        tags
    }

    /// Extract metadata metrics from shared fields
    ///
    /// Returns Vec of (metric_name, value, unit) tuples.
    /// - For Timestamp type: parses ISO 8601 string, converts to epoch seconds (f64)
    /// - For Numeric type: extracts numeric value directly
    fn extract_metadata_metrics(&self, payload: &Value) -> Vec<(String, f64, Option<String>)> {
        let mut metrics = Vec::new();

        for metadata_metric in &self.array_config.metadata_metrics {
            if let Some(value) = self.extract_at_path(payload, &metadata_metric.path) {
                let numeric_value = match metadata_metric.value_type {
                    MetadataValueType::Timestamp => {
                        // Parse ISO 8601 timestamp and convert to epoch seconds
                        if let Some(timestamp_str) = value.as_str() {
                            match DateTime::parse_from_rfc3339(timestamp_str) {
                                Ok(dt) => Some(dt.timestamp() as f64),
                                Err(e) => {
                                    warn!(
                                        "Failed to parse timestamp '{}' for metadata metric '{}': {}",
                                        timestamp_str, metadata_metric.metric_name, e
                                    );
                                    None
                                }
                            }
                        } else {
                            warn!(
                                "Metadata metric '{}' value is not a string (expected ISO 8601 timestamp)",
                                metadata_metric.metric_name
                            );
                            None
                        }
                    }
                    MetadataValueType::Numeric => {
                        // Extract numeric value directly
                        if let Some(num) = value.as_f64() {
                            Some(num)
                        } else if let Some(num) = value.as_i64() {
                            Some(num as f64)
                        } else if let Some(num) = value.as_u64() {
                            Some(num as f64)
                        } else {
                            warn!(
                                "Metadata metric '{}' value is not numeric: {:?}",
                                metadata_metric.metric_name, value
                            );
                            None
                        }
                    }
                };

                if let Some(value) = numeric_value {
                    metrics.push((
                        metadata_metric.metric_name.clone(),
                        value,
                        metadata_metric.unit.clone(),
                    ));
                }
            } else {
                warn!(
                    "Metadata path '{}' not found, skipping metric '{}'",
                    metadata_metric.path, metadata_metric.metric_name
                );
            }
        }

        metrics
    }

    /// Parse numeric value from string using regex
    fn parse_string_value(
        &self,
        string_val: &str,
        parse_config: &StringParseConfig,
    ) -> Option<f64> {
        // Compile regex (cached via lazy_static pattern)
        let re = Regex::new(&parse_config.pattern).ok()?;
        let caps = re.captures(string_val.trim())?;

        // Extract primary capture group
        let low = caps
            .get(parse_config.capture_group)
            .and_then(|m| m.as_str().parse::<f64>().ok())?;

        // If there's a high capture group (for ranges), average them
        if let Some(high_group) = parse_config.capture_group_high {
            if let Some(high) = caps
                .get(high_group)
                .and_then(|m| m.as_str().parse::<f64>().ok())
            {
                return Some((low + high) / 2.0);
            }
        }

        Some(low)
    }

    /// Extract numeric value from element field
    fn extract_element_value(&self, element: &Value, mapping: &ElementMapping) -> Option<f64> {
        let field_value = self.extract_at_path(element, &mapping.path)?;

        // Try direct numeric extraction
        if let Some(num) = field_value.as_f64() {
            return Some(num);
        }
        if let Some(num) = field_value.as_i64() {
            return Some(num as f64);
        }
        if let Some(num) = field_value.as_u64() {
            return Some(num as f64);
        }

        // Try string parsing with regex
        if let Some(parse_config) = &mapping.string_parse {
            if let Some(s) = field_value.as_str() {
                return self.parse_string_value(s, parse_config);
            }
        }

        // Try enum mapping
        if let Some(enum_map) = &mapping.enum_map {
            if let Some(s) = field_value.as_str() {
                return enum_map.get(&s.to_uppercase()).copied();
            }
        }

        None
    }

    /// Extract location ID from payload
    fn extract_location_id(&self, payload: &Value) -> CoreResult<String> {
        // Try to extract using path
        if let Some(value) = self.extract_at_path(payload, &self.config.location_id_field) {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
            if let Some(num) = value.as_f64() {
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

impl Parser for ArrayIteratorParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Extract location ID
        let location_id = self.extract_location_id(payload)?;

        // Extract shared metadata tags
        let mut metadata_tags = self.extract_metadata_tags(payload);
        metadata_tags.extend(self.config.default_tags.clone());

        // Extract metadata metrics (to emit as metric rows)
        let metadata_metrics = self.extract_metadata_metrics(payload);

        // Extract array
        let array = self.extract_array(payload)?;
        let element_count = array.len();

        debug!(
            "Processing {} array elements from path '{}'",
            element_count, self.array_config.array_path
        );

        // Estimate capacity: elements × (element_mappings + metadata_metrics)
        let mut points = Vec::with_capacity(
            element_count * (self.array_config.element_mappings.len() + metadata_metrics.len()),
        );

        // Iterate over array elements
        for (idx, element) in array.iter().enumerate() {
            // Extract element timestamp
            let element_timestamp = match self.extract_element_timestamp(element) {
                Ok(ts) => ts,
                Err(e) => {
                    warn!("Skipping element {}: {}", idx, e);
                    continue;
                }
            };

            // Process each mapping for this element
            for mapping in &self.array_config.element_mappings {
                match self.extract_element_value(element, mapping) {
                    Some(value) => {
                        // Build tags
                        let mut tags = metadata_tags.clone();
                        tags.insert("metric".to_string(), mapping.metric_name.clone());

                        if let Some(unit) = &mapping.unit {
                            tags.insert("unit".to_string(), unit.clone());
                        }

                        // Add element timestamp as forecast_valid_time
                        tags.insert(
                            "forecast_valid_time".to_string(),
                            element_timestamp.timestamp().to_string(),
                        );

                        // Create point
                        points.push(TimeSeriesPoint {
                            timestamp,
                            location_id: location_id.clone(),
                            value,
                            tags,
                            ndp_id: None,
                            context: None,
                        });

                        debug!(
                            "Element {}: extracted {} = {}",
                            idx, mapping.metric_name, value
                        );
                    }
                    None => {
                        if !mapping.optional {
                            return Err(CoreError::Source(format!(
                                "Required field '{}' not found or invalid in element {}",
                                mapping.path, idx
                            )));
                        } else {
                            debug!(
                                "Element {}: optional field '{}' not found, skipping",
                                idx, mapping.path
                            );
                        }
                    }
                }
            }

            // Emit metadata metrics for this element
            // These use the same element_timestamp, location_id, and metadata_tags
            for (metric_name, value, unit) in &metadata_metrics {
                let mut tags = metadata_tags.clone();
                tags.insert("metric".to_string(), metric_name.clone());

                if let Some(unit) = unit {
                    tags.insert("unit".to_string(), unit.clone());
                }

                // Add element timestamp as forecast_valid_time
                tags.insert(
                    "forecast_valid_time".to_string(),
                    element_timestamp.timestamp().to_string(),
                );

                points.push(TimeSeriesPoint {
                    timestamp,
                    location_id: location_id.clone(),
                    value: *value,
                    tags,
                    ndp_id: None,
                    context: None,
                });

                debug!(
                    "Element {}: emitted metadata metric {} = {}",
                    idx, metric_name, value
                );
            }
        }

        if points.is_empty() {
            warn!("No points extracted from array");
        } else {
            debug!(
                "Extracted {} points from {} elements",
                points.len(),
                element_count
            );
        }

        Ok(points)
    }

    fn name(&self) -> &str {
        "array_iterator"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
    // parse_with_context() uses default trait implementation (AIR-009)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::config::ParserType;
    use serde_json::json;

    fn create_test_parser(
        array_path: &str,
        timestamp_field: &str,
        mappings: Vec<ElementMapping>,
    ) -> ArrayIteratorParser {
        let array_config = ArrayIteratorConfig {
            array_path: array_path.to_string(),
            timestamp_field: timestamp_field.to_string(),
            metadata_tags: vec![],
            metadata_metrics: vec![],
            element_mappings: mappings,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("array_iterator".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: Some(array_config),
            column_config: None,
        };

        ArrayIteratorParser::from_config(base_config).unwrap()
    }

    #[test]
    fn test_array_iteration_produces_correct_point_count() {
        let mappings = vec![
            ElementMapping {
                path: "temperature".to_string(),
                metric_name: "temp".to_string(),
                unit: Some("celsius".to_string()),
                string_parse: None,
                enum_map: None,
                optional: false,
            },
            ElementMapping {
                path: "humidity".to_string(),
                metric_name: "humid".to_string(),
                unit: Some("percent".to_string()),
                string_parse: None,
                enum_map: None,
                optional: false,
            },
        ];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "temperature": 20.5, "humidity": 65.0},
                {"time": "2025-12-21T13:00:00Z", "temperature": 21.0, "humidity": 63.0},
                {"time": "2025-12-21T14:00:00Z", "temperature": 22.0, "humidity": 60.0}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // 3 elements × 2 metrics = 6 points
        assert_eq!(points.len(), 6);

        // Verify we have both metrics for each element
        let temp_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"temp".to_string()))
            .collect();
        let humid_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"humid".to_string()))
            .collect();

        assert_eq!(temp_points.len(), 3);
        assert_eq!(humid_points.len(), 3);
    }

    #[test]
    fn test_timestamp_extraction_from_elements() {
        let mappings = vec![ElementMapping {
            path: "value".to_string(),
            metric_name: "test".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("data", "timestamp", mappings);

        let payload = json!({
            "location": "test",
            "data": [
                {"timestamp": "2025-12-21T12:00:00Z", "value": 10.0},
                {"timestamp": "2025-12-21T13:00:00Z", "value": 20.0}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 2);

        // Verify forecast_valid_time tags are different
        let time1 = points[0].tags.get("forecast_valid_time").unwrap();
        let time2 = points[1].tags.get("forecast_valid_time").unwrap();
        assert_ne!(time1, time2);

        // Verify they correspond to the element timestamps
        let expected_time1 = DateTime::parse_from_rfc3339("2025-12-21T12:00:00Z")
            .unwrap()
            .timestamp()
            .to_string();
        assert_eq!(time1, &expected_time1);
    }

    #[test]
    fn test_string_parsing_with_regex() {
        let mappings = vec![ElementMapping {
            path: "windSpeed".to_string(),
            metric_name: "wind_speed".to_string(),
            unit: Some("m/s".to_string()),
            string_parse: Some(StringParseConfig {
                pattern: r"^(\d+)\s*mph$".to_string(),
                capture_group: 1,
                capture_group_high: None,
            }),
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "windSpeed": "15 mph"}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 15.0);
        assert_eq!(points[0].tags.get("metric").unwrap(), "wind_speed");
    }

    #[test]
    fn test_string_parsing_with_range_averaging() {
        let mappings = vec![ElementMapping {
            path: "windSpeed".to_string(),
            metric_name: "wind_speed".to_string(),
            unit: Some("m/s".to_string()),
            string_parse: Some(StringParseConfig {
                pattern: r"^(\d+)\s*to\s*(\d+)\s*mph$".to_string(),
                capture_group: 1,
                capture_group_high: Some(2),
            }),
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "windSpeed": "5 to 10 mph"}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 7.5); // Average of 5 and 10
    }

    #[test]
    fn test_enum_mapping_for_wind_direction() {
        let mut enum_map = HashMap::new();
        enum_map.insert("N".to_string(), 0.0);
        enum_map.insert("NE".to_string(), 45.0);
        enum_map.insert("E".to_string(), 90.0);
        enum_map.insert("SE".to_string(), 135.0);
        enum_map.insert("S".to_string(), 180.0);
        enum_map.insert("SW".to_string(), 225.0);
        enum_map.insert("W".to_string(), 270.0);
        enum_map.insert("NW".to_string(), 315.0);

        let mappings = vec![ElementMapping {
            path: "windDirection".to_string(),
            metric_name: "wind_dir".to_string(),
            unit: Some("degrees".to_string()),
            string_parse: None,
            enum_map: Some(enum_map),
            optional: false,
        }];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "windDirection": "N"},
                {"time": "2025-12-21T13:00:00Z", "windDirection": "NE"},
                {"time": "2025-12-21T14:00:00Z", "windDirection": "S"}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value, 0.0); // N
        assert_eq!(points[1].value, 45.0); // NE
        assert_eq!(points[2].value, 180.0); // S
    }

    #[test]
    fn test_metadata_tags_propagation() {
        let mappings = vec![ElementMapping {
            path: "value".to_string(),
            metric_name: "test".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let array_config = ArrayIteratorConfig {
            array_path: "periods".to_string(),
            timestamp_field: "time".to_string(),
            metadata_tags: vec![MetadataTagMapping {
                path: "generatedAt".to_string(),
                tag_name: "issue_time".to_string(),
            }],
            metadata_metrics: vec![],
            element_mappings: mappings,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("array_iterator".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: {
                let mut tags = HashMap::new();
                tags.insert("source".to_string(), "nws".to_string());
                tags
            },
            array_config: Some(array_config),
            column_config: None,
        };

        let parser = ArrayIteratorParser::from_config(base_config).unwrap();

        let payload = json!({
            "location": "test",
            "generatedAt": "2025-12-21T10:00:00Z",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "value": 10.0}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);

        // Verify default tags
        assert_eq!(points[0].tags.get("source").unwrap(), "nws");

        // Verify metadata tags
        assert_eq!(
            points[0].tags.get("issue_time").unwrap(),
            "2025-12-21T10:00:00Z"
        );

        // Verify metric tag
        assert_eq!(points[0].tags.get("metric").unwrap(), "test");
    }

    #[test]
    fn test_optional_fields_gracefully_skipped() {
        let mappings = vec![
            ElementMapping {
                path: "required".to_string(),
                metric_name: "req".to_string(),
                unit: None,
                string_parse: None,
                enum_map: None,
                optional: false,
            },
            ElementMapping {
                path: "optional".to_string(),
                metric_name: "opt".to_string(),
                unit: None,
                string_parse: None,
                enum_map: None,
                optional: true,
            },
        ];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "required": 10.0}
                // optional field missing
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should only get 1 point (the required field)
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric").unwrap(), "req");
    }

    #[test]
    fn test_missing_required_field_returns_error() {
        let mappings = vec![ElementMapping {
            path: "required".to_string(),
            metric_name: "req".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test",
            "periods": [
                {"time": "2025-12-21T12:00:00Z"}
                // required field missing
            ]
        });

        let result = parser.parse(&payload, Utc::now());
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("Required field"));
        }
    }

    #[test]
    fn test_nested_array_path_navigation() {
        let mappings = vec![ElementMapping {
            path: "temp".to_string(),
            metric_name: "temperature".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("data.forecast.periods", "timestamp", mappings);

        let payload = json!({
            "location": "test",
            "data": {
                "forecast": {
                    "periods": [
                        {"timestamp": "2025-12-21T12:00:00Z", "temp": 20.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 20.5);
    }

    #[test]
    fn test_metadata_metrics_timestamp_extraction() {
        let mappings = vec![ElementMapping {
            path: "temperature".to_string(),
            metric_name: "temp".to_string(),
            unit: Some("celsius".to_string()),
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let array_config = ArrayIteratorConfig {
            array_path: "periods".to_string(),
            timestamp_field: "time".to_string(),
            metadata_tags: vec![],
            metadata_metrics: vec![MetadataMetricMapping {
                path: "updateTime".to_string(),
                metric_name: "issue_time".to_string(),
                value_type: MetadataValueType::Timestamp,
                unit: Some("epoch_seconds".to_string()),
            }],
            element_mappings: mappings,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("array_iterator".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: Some(array_config),
            column_config: None,
        };

        let parser = ArrayIteratorParser::from_config(base_config).unwrap();

        let payload = json!({
            "location": "test",
            "updateTime": "2025-12-21T10:00:00Z",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "temperature": 20.5},
                {"time": "2025-12-21T13:00:00Z", "temperature": 21.0}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should have 2 elements × 2 metrics (temp + issue_time) = 4 points
        assert_eq!(points.len(), 4);

        // Find issue_time points
        let issue_time_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"issue_time".to_string()))
            .collect();

        assert_eq!(issue_time_points.len(), 2);

        // Verify the timestamp was converted to epoch seconds
        let expected_epoch = DateTime::parse_from_rfc3339("2025-12-21T10:00:00Z")
            .unwrap()
            .timestamp() as f64;

        for point in issue_time_points {
            assert_eq!(point.value, expected_epoch);
            assert_eq!(point.tags.get("unit").unwrap(), "epoch_seconds");
        }

        // Verify temperature points still work
        let temp_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"temp".to_string()))
            .collect();

        assert_eq!(temp_points.len(), 2);
        assert_eq!(temp_points[0].value, 20.5);
        assert_eq!(temp_points[1].value, 21.0);
    }

    #[test]
    fn test_metadata_metrics_numeric_extraction() {
        let mappings = vec![ElementMapping {
            path: "value".to_string(),
            metric_name: "measurement".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let array_config = ArrayIteratorConfig {
            array_path: "data".to_string(),
            timestamp_field: "time".to_string(),
            metadata_tags: vec![],
            metadata_metrics: vec![MetadataMetricMapping {
                path: "confidence".to_string(),
                metric_name: "confidence_score".to_string(),
                value_type: MetadataValueType::Numeric,
                unit: Some("percent".to_string()),
            }],
            element_mappings: mappings,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("array_iterator".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: Some(array_config),
            column_config: None,
        };

        let parser = ArrayIteratorParser::from_config(base_config).unwrap();

        let payload = json!({
            "location": "test",
            "confidence": 95.5,
            "data": [
                {"time": "2025-12-21T12:00:00Z", "value": 10.0}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should have 1 element × 2 metrics = 2 points
        assert_eq!(points.len(), 2);

        // Find confidence_score point
        let confidence_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"confidence_score".to_string()))
            .unwrap();

        assert_eq!(confidence_point.value, 95.5);
        assert_eq!(confidence_point.tags.get("unit").unwrap(), "percent");
    }

    #[test]
    fn test_metadata_metrics_emitted_per_element() {
        // Verify that metadata metrics are emitted once per array element,
        // not once per document
        let mappings = vec![ElementMapping {
            path: "temp".to_string(),
            metric_name: "temperature".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let array_config = ArrayIteratorConfig {
            array_path: "periods".to_string(),
            timestamp_field: "time".to_string(),
            metadata_tags: vec![],
            metadata_metrics: vec![MetadataMetricMapping {
                path: "issued".to_string(),
                metric_name: "issue_time".to_string(),
                value_type: MetadataValueType::Timestamp,
                unit: None,
            }],
            element_mappings: mappings,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("array_iterator".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: Some(array_config),
            column_config: None,
        };

        let parser = ArrayIteratorParser::from_config(base_config).unwrap();

        let payload = json!({
            "location": "test",
            "issued": "2025-12-21T10:00:00Z",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "temp": 20.0},
                {"time": "2025-12-21T13:00:00Z", "temp": 21.0},
                {"time": "2025-12-21T14:00:00Z", "temp": 22.0}
            ]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // 3 elements × 2 metrics (temp + issue_time) = 6 points
        assert_eq!(points.len(), 6);

        // Verify we have 3 issue_time points (one per element)
        let issue_time_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"issue_time".to_string()))
            .collect();

        assert_eq!(issue_time_points.len(), 3);

        // Each should have different forecast_valid_time tags
        let valid_times: Vec<_> = issue_time_points
            .iter()
            .map(|p| p.tags.get("forecast_valid_time").unwrap().as_str())
            .collect();

        assert_eq!(valid_times.len(), 3);
        // Verify they're all different (converted to set should still have 3 items)
        let unique_times: std::collections::HashSet<_> = valid_times.iter().collect();
        assert_eq!(unique_times.len(), 3);
    }

    #[test]
    fn test_metadata_metrics_missing_optional() {
        // Test graceful handling when metadata path doesn't exist
        let mappings = vec![ElementMapping {
            path: "value".to_string(),
            metric_name: "measurement".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let array_config = ArrayIteratorConfig {
            array_path: "data".to_string(),
            timestamp_field: "time".to_string(),
            metadata_tags: vec![],
            metadata_metrics: vec![MetadataMetricMapping {
                path: "nonexistent.field".to_string(),
                metric_name: "missing_metric".to_string(),
                value_type: MetadataValueType::Numeric,
                unit: None,
            }],
            element_mappings: mappings,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("array_iterator".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: Some(array_config),
            column_config: None,
        };

        let parser = ArrayIteratorParser::from_config(base_config).unwrap();

        let payload = json!({
            "location": "test",
            "data": [
                {"time": "2025-12-21T12:00:00Z", "value": 10.0}
            ]
        });

        // Should not error, just warn and skip the missing metadata metric
        let result = parser.parse(&payload, Utc::now());
        assert!(result.is_ok());

        let points = result.unwrap();

        // Should only have 1 point from element mapping (metadata metric skipped)
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric").unwrap(), "measurement");
        assert_eq!(points[0].value, 10.0);
    }

    // ========== AIR-009: parse_with_context TESTS (TDD Cycle 6) ==========

    #[test]
    fn test_array_iterator_parser_injects_ndp_id_and_context() {
        use super::super::ParseContext;

        let mappings = vec![ElementMapping {
            path: "temperature".to_string(),
            metric_name: "temp".to_string(),
            unit: Some("celsius".to_string()),
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("periods", "time", mappings);

        let payload = json!({
            "location": "test-sensor",
            "periods": [
                {"time": "2025-12-21T12:00:00Z", "temperature": 20.5},
                {"time": "2025-12-21T13:00:00Z", "temperature": 21.0}
            ]
        });

        let context = ParseContext::new(
            Some("weather-station-001".to_string()),
            Some(json!({"region": "north", "station_type": "outdoor"})),
        );

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert_eq!(points.len(), 2);

        // All points should have ndp_id and context injected
        for point in &points {
            assert_eq!(point.ndp_id, Some("weather-station-001".to_string()));
            assert!(point.context.is_some());
            let ctx = point.context.as_ref().unwrap();
            assert_eq!(ctx["region"], "north");
            assert_eq!(ctx["station_type"], "outdoor");
        }
    }

    #[test]
    fn test_array_iterator_parser_empty_context_passthrough() {
        use super::super::ParseContext;

        let mappings = vec![ElementMapping {
            path: "value".to_string(),
            metric_name: "measurement".to_string(),
            unit: None,
            string_parse: None,
            enum_map: None,
            optional: false,
        }];

        let parser = create_test_parser("data", "time", mappings);

        let payload = json!({
            "location": "test",
            "data": [
                {"time": "2025-12-21T12:00:00Z", "value": 42.0}
            ]
        });

        let context = ParseContext::default();

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert_eq!(points.len(), 1);
        assert!(points[0].ndp_id.is_none());
        assert!(points[0].context.is_none());
    }
}
