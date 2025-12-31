//! Column-Oriented Parser Implementation
//!
//! Handles column-oriented JSON data structures where each metric has its own
//! dedicated values array. Used for NWS gridpoints and similar data sources.
//!
//! Features:
//! - ISO 8601 duration timestamp parsing ("2025-12-23T00:00:00+00:00/PT1H")
//! - Column-by-column metric extraction
//! - Flexible JSON path navigation
//! - Unit conversion support
//! - Graceful error handling with warnings

use crate::error::{CoreError, CoreResult};
use crate::parsers::config::{ColumnMapping, ColumnOrientedConfig, ParserConfig, TimestampFormat};
use crate::parsers::traits::Parser;
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tracing::{debug, warn};

/// Parser for column-oriented JSON data structures
///
/// Handles data where each metric is stored in a separate object with
/// its own values array. Each value includes a timestamp and numeric value.
#[derive(Debug)]
pub struct ColumnOrientedParser {
    config: ParserConfig,
    column_config: ColumnOrientedConfig,
}

impl ColumnOrientedParser {
    /// Create a new ColumnOrientedParser from configuration
    ///
    /// Extracts column_config from ParserConfig.column_config field.
    /// Returns error if column_config is not present.
    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        let column_config = config.column_config.clone().ok_or_else(|| {
            CoreError::Config(
                "ColumnOrientedParser requires 'column_config' in ParserConfig".to_string(),
            )
        })?;

        Ok(Self {
            config,
            column_config,
        })
    }

    /// Create from explicit configs (for testing)
    #[cfg(test)]
    pub fn from_configs(
        config: ParserConfig,
        column_config: ColumnOrientedConfig,
    ) -> CoreResult<Self> {
        Ok(Self {
            config,
            column_config,
        })
    }

    /// Extract value at JSON path (supports dot notation)
    ///
    /// Navigates JSON structure using dot-separated paths.
    /// Does not support array indexing (e.g., "field[0]").
    fn extract_at_path<'a>(&self, root: &'a Value, path: &str) -> Option<&'a Value> {
        let mut current = root;

        for segment in path.split('.') {
            current = current.get(segment)?;
        }

        Some(current)
    }

    /// Parse ISO 8601 duration timestamp
    ///
    /// Handles NWS format: "2025-12-23T00:00:00+00:00/PT1H"
    /// Returns the datetime component (before the "/" separator).
    /// The duration component (e.g., "PT1H") indicates validity period
    /// but is not used for timestamp extraction.
    fn parse_iso8601_duration(&self, timestamp_str: &str) -> CoreResult<DateTime<Utc>> {
        let parts: Vec<&str> = timestamp_str.split('/').collect();
        if parts.is_empty() {
            return Err(CoreError::Source(format!(
                "Invalid ISO 8601 duration format: {}",
                timestamp_str
            )));
        }

        DateTime::parse_from_rfc3339(parts[0])
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                CoreError::Source(format!("Failed to parse timestamp '{}': {}", parts[0], e))
            })
    }

    /// Extract timestamp from value entry
    fn extract_timestamp(
        &self,
        value_entry: &Value,
        mapping: &ColumnMapping,
    ) -> CoreResult<DateTime<Utc>> {
        let timestamp_path = mapping.timestamp_path.as_deref().unwrap_or("validTime");

        let timestamp_value = self
            .extract_at_path(value_entry, timestamp_path)
            .ok_or_else(|| {
                CoreError::Source(format!(
                    "Timestamp field '{}' not found in value entry",
                    timestamp_path
                ))
            })?;

        let timestamp_str = timestamp_value
            .as_str()
            .ok_or_else(|| CoreError::Source("Timestamp value is not a string".to_string()))?;

        match &self.column_config.timestamp_format {
            TimestampFormat::Iso8601Duration => self.parse_iso8601_duration(timestamp_str),
            TimestampFormat::ParallelArray { .. } => {
                // For parallel array format, timestamps come from separate array
                DateTime::parse_from_rfc3339(timestamp_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| CoreError::Source(format!("Invalid RFC3339 timestamp: {}", e)))
            }
        }
    }

    /// Extract numeric value from value entry
    fn extract_value(&self, value_entry: &Value, mapping: &ColumnMapping) -> Option<f64> {
        let value_path = mapping.value_path.as_deref().unwrap_or("value");

        let value = self.extract_at_path(value_entry, value_path)?;

        // Try numeric extraction
        if let Some(num) = value.as_f64() {
            return Some(num);
        }
        if let Some(num) = value.as_i64() {
            return Some(num as f64);
        }
        if let Some(num) = value.as_u64() {
            return Some(num as f64);
        }

        None
    }

    /// Apply unit conversion if configured
    fn apply_unit_conversion(&self, value: f64, field_name: &str) -> f64 {
        if let Some(conversion) = self.column_config.unit_conversions.get(field_name) {
            conversion.convert(value)
        } else {
            value
        }
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

impl Parser for ColumnOrientedParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Extract location ID
        let location_id = self.extract_location_id(payload)?;

        // Get base metadata tags
        let default_tags = self.config.default_tags.clone();

        // Navigate to metrics base path
        let metrics_base = self
            .extract_at_path(payload, &self.column_config.metrics_base_path)
            .ok_or_else(|| {
                CoreError::Source(format!(
                    "Metrics base path '{}' not found",
                    self.column_config.metrics_base_path
                ))
            })?;

        // Pre-allocate point vector (estimate: columns × ~150 values)
        let estimated_capacity = self.column_config.columns.len() * 150;
        let mut all_points = Vec::with_capacity(estimated_capacity);

        // Iterate over each column mapping
        for mapping in &self.column_config.columns {
            // Navigate to metric object
            let metric_obj = match self.extract_at_path(metrics_base, &mapping.metric_path) {
                Some(obj) => obj,
                None => {
                    warn!(
                        "Metric path '{}' not found, skipping column '{}'",
                        mapping.metric_path, mapping.field_name
                    );
                    continue;
                }
            };

            // Navigate to values array
            let values_path = mapping.values_path.as_deref().unwrap_or("values");
            let values_array = match self.extract_at_path(metric_obj, values_path) {
                Some(val) => val.as_array(),
                None => {
                    warn!(
                        "Values path '{}' not found in metric '{}', skipping",
                        values_path, mapping.metric_path
                    );
                    continue;
                }
            };

            let values = match values_array {
                Some(arr) => arr,
                None => {
                    warn!(
                        "Values at path '{}' is not an array in metric '{}', skipping",
                        values_path, mapping.metric_path
                    );
                    continue;
                }
            };

            debug!(
                "Processing {} values for metric '{}'",
                values.len(),
                mapping.field_name
            );

            // Process each value entry
            for (idx, value_entry) in values.iter().enumerate() {
                // Extract timestamp
                let element_timestamp = match self.extract_timestamp(value_entry, mapping) {
                    Ok(ts) => ts,
                    Err(e) => {
                        warn!(
                            "Skipping value {} in metric '{}': {}",
                            idx, mapping.field_name, e
                        );
                        continue;
                    }
                };

                // Extract numeric value
                let raw_value = match self.extract_value(value_entry, mapping) {
                    Some(v) => v,
                    None => {
                        warn!(
                            "Could not extract value {} in metric '{}', skipping",
                            idx, mapping.field_name
                        );
                        continue;
                    }
                };

                // Apply unit conversion
                let converted_value = self.apply_unit_conversion(raw_value, &mapping.field_name);

                // Build tags
                let mut tags = default_tags.clone();
                tags.insert("metric".to_string(), mapping.field_name.clone());
                tags.insert(
                    "forecast_valid_time".to_string(),
                    element_timestamp.timestamp().to_string(),
                );

                // Create point
                all_points.push(TimeSeriesPoint {
                    timestamp, // Ingestion timestamp
                    location_id: location_id.clone(),
                    value: converted_value,
                    tags,
                    ndp_id: None,
                    context: None,
                });

                debug!(
                    "Extracted {} = {} (converted from {}) at {}",
                    mapping.field_name, converted_value, raw_value, element_timestamp
                );
            }
        }

        if all_points.is_empty() {
            warn!("No points extracted from column-oriented data");
        } else {
            debug!(
                "Extracted {} total points from {} columns",
                all_points.len(),
                self.column_config.columns.len()
            );
        }

        Ok(all_points)
    }

    fn name(&self) -> &str {
        "column_oriented"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
    // parse_with_context() uses default trait implementation (AIR-009)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::config::{ConversionFormula, ParserType, UnitConversion};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_parser(
        metrics_base_path: &str,
        columns: Vec<ColumnMapping>,
    ) -> ColumnOrientedParser {
        let column_config = ColumnOrientedConfig {
            metrics_base_path: metrics_base_path.to_string(),
            columns,
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions: HashMap::new(),
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("column_oriented".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: None,
            column_config: Some(column_config.clone()),
        };

        ColumnOrientedParser::from_configs(base_config, column_config).unwrap()
    }

    #[test]
    fn test_iso8601_duration_parsing() {
        let parser = create_test_parser("properties", vec![]);

        // Test valid NWS format
        let result = parser.parse_iso8601_duration("2025-12-24T12:00:00+00:00/PT1H");
        assert!(result.is_ok());

        let dt = result.unwrap();
        assert_eq!(dt.timestamp(), 1766577600); // 2025-12-24T12:00:00Z

        // Test without duration component
        let result = parser.parse_iso8601_duration("2025-12-24T12:00:00+00:00");
        assert!(result.is_ok());

        // Test invalid format
        let result = parser.parse_iso8601_duration("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_column_extraction_produces_correct_points() {
        let columns = vec![
            ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temp_c".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
            ColumnMapping {
                metric_path: "humidity".to_string(),
                field_name: "humidity_pct".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
        ];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5},
                        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 14.8}
                    ]
                },
                "humidity": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 68}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // 2 temperature values + 1 humidity value = 3 points
        assert_eq!(points.len(), 3);

        // Verify we have both metrics
        let temp_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"temp_c".to_string()))
            .collect();
        let humidity_points: Vec<_> = points
            .iter()
            .filter(|p| p.tags.get("metric") == Some(&"humidity_pct".to_string()))
            .collect();

        assert_eq!(temp_points.len(), 2);
        assert_eq!(humidity_points.len(), 1);

        // Verify values
        assert_eq!(temp_points[0].value, 15.5);
        assert_eq!(temp_points[1].value, 14.8);
        assert_eq!(humidity_points[0].value, 68.0);
    }

    #[test]
    fn test_missing_column_gracefully_skipped() {
        let columns = vec![
            ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temp_c".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
            ColumnMapping {
                metric_path: "nonexistent".to_string(),
                field_name: "missing".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            },
        ];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should only get temperature point, missing column skipped
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric").unwrap(), "temp_c");
    }

    #[test]
    fn test_invalid_timestamp_skips_entry() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "invalid-timestamp", "value": 15.5},
                        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 14.8}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should skip invalid timestamp, only get the valid one
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 14.8);
    }

    #[test]
    fn test_unit_conversion_applied() {
        let mut unit_conversions = HashMap::new();
        unit_conversions.insert(
            "temp_f".to_string(),
            UnitConversion {
                from: "fahrenheit".to_string(),
                to: "celsius".to_string(),
                factor: None,
                formula: Some(ConversionFormula::Linear {
                    scale: 5.0 / 9.0,
                    offset: -32.0 * 5.0 / 9.0,
                }),
            },
        );

        let column_config = ColumnOrientedConfig {
            metrics_base_path: "properties".to_string(),
            columns: vec![ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temp_f".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            }],
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("column_oriented".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: None,
            column_config: Some(column_config.clone()),
        };

        let parser = ColumnOrientedParser::from_configs(base_config, column_config).unwrap();

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 32.0}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        // 32°F = 0°C
        assert!((points[0].value - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_nested_path_navigation() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("data.forecast.properties", columns);

        let payload = json!({
            "location": "test",
            "data": {
                "forecast": {
                    "properties": {
                        "temperature": {
                            "values": [
                                {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                            ]
                        }
                    }
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 15.5);
    }

    #[test]
    fn test_default_paths_work() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,    // Should default to "values"
            timestamp_path: None, // Should default to "validTime"
            value_path: None,     // Should default to "value"
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 15.5);
    }

    #[test]
    fn test_custom_paths_override() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: Some("data".to_string()),
            timestamp_path: Some("time".to_string()),
            value_path: Some("measurement".to_string()),
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "data": [
                        {"time": "2025-12-24T00:00:00+00:00/PT1H", "measurement": 15.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 15.5);
    }

    #[test]
    fn test_forecast_valid_time_tag_present() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T12:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);

        let valid_time = points[0].tags.get("forecast_valid_time").unwrap();
        let expected_time = DateTime::parse_from_rfc3339("2025-12-24T12:00:00+00:00")
            .unwrap()
            .timestamp()
            .to_string();

        assert_eq!(valid_time, &expected_time);
    }

    #[test]
    fn test_location_id_extraction() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "station_123",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].location_id, "station_123");
    }

    #[test]
    fn test_default_tags_propagation() {
        let column_config = ColumnOrientedConfig {
            metrics_base_path: "properties".to_string(),
            columns: vec![ColumnMapping {
                metric_path: "temperature".to_string(),
                field_name: "temp_c".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            }],
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions: HashMap::new(),
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("column_oriented".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: {
                let mut tags = HashMap::new();
                tags.insert("source".to_string(), "nws".to_string());
                tags.insert("data_type".to_string(), "gridpoint".to_string());
                tags
            },
            array_config: None,
            column_config: Some(column_config.clone()),
        };

        let parser = ColumnOrientedParser::from_configs(base_config, column_config).unwrap();

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("source").unwrap(), "nws");
        assert_eq!(points[0].tags.get("data_type").unwrap(), "gridpoint");
        assert_eq!(points[0].tags.get("metric").unwrap(), "temp_c");
    }

    #[test]
    fn test_multiple_values_per_column() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5},
                        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 14.8},
                        {"validTime": "2025-12-24T02:00:00+00:00/PT1H", "value": 14.2},
                        {"validTime": "2025-12-24T03:00:00+00:00/PT1H", "value": 13.9}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 4);

        // Verify values are in order
        assert_eq!(points[0].value, 15.5);
        assert_eq!(points[1].value, 14.8);
        assert_eq!(points[2].value, 14.2);
        assert_eq!(points[3].value, 13.9);

        // Verify forecast_valid_time tags are different
        let time0 = points[0].tags.get("forecast_valid_time").unwrap();
        let time1 = points[1].tags.get("forecast_valid_time").unwrap();
        let time2 = points[2].tags.get("forecast_valid_time").unwrap();
        let time3 = points[3].tags.get("forecast_valid_time").unwrap();

        assert_ne!(time0, time1);
        assert_ne!(time1, time2);
        assert_ne!(time2, time3);
    }

    #[test]
    fn test_missing_metrics_base_path_returns_error() {
        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temp_c".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("nonexistent", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5}
                    ]
                }
            }
        });

        let result = parser.parse(&payload, Utc::now());
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("Metrics base path"));
        }
    }

    #[test]
    fn test_unit_conversion_factor() {
        let mut unit_conversions = HashMap::new();
        unit_conversions.insert(
            "wind_speed_ms".to_string(),
            UnitConversion {
                from: "kmh".to_string(),
                to: "ms".to_string(),
                factor: Some(1.0 / 3.6), // km/h to m/s
                formula: None,
            },
        );

        let column_config = ColumnOrientedConfig {
            metrics_base_path: "properties".to_string(),
            columns: vec![ColumnMapping {
                metric_path: "windSpeed".to_string(),
                field_name: "wind_speed_ms".to_string(),
                values_path: None,
                timestamp_path: None,
                value_path: None,
            }],
            timestamp_format: TimestampFormat::Iso8601Duration,
            unit_conversions,
        };

        let base_config = ParserConfig {
            parser_type: ParserType::Custom("column_oriented".to_string()),
            location_id_field: "location".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: None,
            column_config: Some(column_config.clone()),
        };

        let parser = ColumnOrientedParser::from_configs(base_config, column_config).unwrap();

        let payload = json!({
            "location": "test",
            "properties": {
                "windSpeed": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 36.0}
                    ]
                }
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 1);
        // 36 km/h = 10 m/s
        assert!((points[0].value - 10.0).abs() < 0.001);
    }

    // ========== AIR-009: parse_with_context TESTS (TDD Cycle 6) ==========

    #[test]
    fn test_column_oriented_parser_injects_ndp_id_and_context() {
        use super::super::ParseContext;

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "grid-station",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 22.5},
                        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 23.0}
                    ]
                }
            }
        });

        let context = ParseContext::new(
            Some("nws-grid-001".to_string()),
            Some(json!({"grid_x": 50, "grid_y": 75, "office": "SFO"})),
        );

        let points = parser
            .parse_with_context(&payload, Utc::now(), &context)
            .unwrap();

        assert_eq!(points.len(), 2);

        // All points should have ndp_id and context injected
        for point in &points {
            assert_eq!(point.ndp_id, Some("nws-grid-001".to_string()));
            assert!(point.context.is_some());
            let ctx = point.context.as_ref().unwrap();
            assert_eq!(ctx["grid_x"], 50);
            assert_eq!(ctx["grid_y"], 75);
            assert_eq!(ctx["office"], "SFO");
        }
    }

    #[test]
    fn test_column_oriented_parser_empty_context_passthrough() {
        use super::super::ParseContext;

        let columns = vec![ColumnMapping {
            metric_path: "temperature".to_string(),
            field_name: "temperature".to_string(),
            values_path: None,
            timestamp_path: None,
            value_path: None,
        }];

        let parser = create_test_parser("properties", columns);

        let payload = json!({
            "location": "test",
            "properties": {
                "temperature": {
                    "values": [
                        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 20.0}
                    ]
                }
            }
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
