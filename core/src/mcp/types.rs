//! MCP configuration types for Bronze MCP Server.
//!
//! These types represent stream configuration data retrieved from etcd
//! and are designed for MCP tool responses.
//!
//! # Type Hierarchy
//!
//! - [`StreamConfig`]: Top-level stream configuration
//!   - [`SourceConfig`]: Data source configuration
//!     - [`FieldMapping`]: Parser field mappings
//!   - [`EntitySchema`]: Target schema definition
//!     - [`Attribute`]: Schema attribute definition
//!
//! # Design Notes
//!
//! These types are intentionally separate from `core::types::stream_config`
//! to provide MCP-specific serialization and response formatting.

use serde::{Deserialize, Serialize};

// =============================================================================
// Stream Configuration
// =============================================================================

/// Stream configuration from etcd.
///
/// Represents the complete configuration for a data stream as stored
/// in etcd under `/streams/{stream_id}/`.
///
/// # Fields
///
/// - `stream_id`: Unique identifier (kebab-case)
/// - `description`: Human-readable description
/// - `version`: Configuration version (semver)
/// - `enabled`: Whether the stream is active
/// - `sources`: Data source configurations
/// - `entity_schemas`: Target schema definitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamConfig {
    /// Unique stream identifier (kebab-case, 1-64 chars).
    pub stream_id: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Configuration version (semver format).
    #[serde(default)]
    pub version: Option<String>,

    /// Whether the stream is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Data retention in days (0 = infinite).
    #[serde(default)]
    pub retention_days: Option<u32>,

    /// Data sources for this stream.
    #[serde(default)]
    pub sources: Vec<SourceConfig>,

    /// Target schema definitions.
    #[serde(default)]
    pub entity_schemas: Vec<EntitySchema>,
}

fn default_true() -> bool {
    true
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            stream_id: String::new(),
            description: None,
            version: None,
            enabled: true,
            retention_days: None,
            sources: Vec::new(),
            entity_schemas: Vec::new(),
        }
    }
}

// =============================================================================
// Source Configuration
// =============================================================================

/// Data source configuration within a stream.
///
/// Represents a single data source (MQTT, HTTP polling, etc.) that
/// feeds data into the stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    /// Source type identifier (mqtt, http_poll, webhook, file_watch).
    #[serde(rename = "type")]
    pub source_type: String,

    /// Whether this source is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Platform-assigned stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Parser configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserConfig>,

    /// Additional source-specific configuration as JSON.
    /// This captures broker_url, topic_pattern, etc.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            source_type: "unknown".to_string(),
            enabled: true,
            ndp_id: None,
            parser: None,
            extra: serde_json::Map::new(),
        }
    }
}

// =============================================================================
// Parser Configuration
// =============================================================================

/// Parser configuration for a data source.
///
/// Defines how raw data is parsed and field mappings are applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParserConfig {
    /// Parser type (flat_json, json_path, array_iterator).
    #[serde(default)]
    pub parser_type: Option<String>,

    /// Field mappings for extracting values from raw payload.
    #[serde(default)]
    pub field_mappings: Vec<FieldMapping>,

    /// Additional parser-specific configuration.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            parser_type: None,
            field_mappings: Vec::new(),
            extra: serde_json::Map::new(),
        }
    }
}

/// Field mapping for parser configuration.
///
/// Maps a path in the raw payload to a target metric name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldMapping {
    /// JSON path in raw payload (dot-separated).
    pub path: String,

    /// Target metric name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,

    /// Unit of measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

// =============================================================================
// Entity Schema
// =============================================================================

/// Entity schema definition for a stream.
///
/// Defines the target schema that data will be transformed into
/// in the Silver layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitySchema {
    /// Schema name identifier.
    pub schema_name: String,

    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Device class (e.g., air_quality, weather).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,

    /// Schema attributes.
    #[serde(default)]
    pub attributes: Vec<Attribute>,
}

impl Default for EntitySchema {
    fn default() -> Self {
        Self {
            schema_name: String::new(),
            description: None,
            device_class: None,
            attributes: Vec::new(),
        }
    }
}

/// Schema attribute definition.
///
/// Defines a single attribute/field in an entity schema with
/// type, unit, and validation constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attribute {
    /// Attribute name (snake_case).
    pub name: String,

    /// Data type (float, int, string, bool, json, timestamp).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub attr_type: Option<String>,

    /// Unit of measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether null values are allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,

    /// Valid value range [min, max] for numeric types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Vec<f64>>,
}

impl Default for Attribute {
    fn default() -> Self {
        Self {
            name: String::new(),
            attr_type: None,
            unit: None,
            description: None,
            nullable: None,
            range: None,
        }
    }
}

// =============================================================================
// Stream Info (for list_streams response)
// =============================================================================

/// Summary information for a stream.
///
/// Used in `list_streams` tool responses to provide an overview
/// of available streams without full configuration details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Stream identifier.
    pub stream_id: String,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Configuration version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Whether the stream is enabled.
    pub enabled: bool,

    /// Number of configured sources.
    pub source_count: usize,

    /// Source types (e.g., ["mqtt", "http_poll"]).
    pub source_types: Vec<String>,
}

impl From<&StreamConfig> for StreamInfo {
    fn from(config: &StreamConfig) -> Self {
        Self {
            stream_id: config.stream_id.clone(),
            description: config.description.clone(),
            version: config.version.clone(),
            enabled: config.enabled,
            source_count: config.sources.len(),
            source_types: config
                .sources
                .iter()
                .map(|s| s.source_type.clone())
                .collect(),
        }
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert!(config.stream_id.is_empty());
        assert!(config.enabled);
        assert!(config.sources.is_empty());
        assert!(config.entity_schemas.is_empty());
    }

    #[test]
    fn test_stream_config_serialization() {
        let config = StreamConfig {
            stream_id: "air-quality".to_string(),
            description: Some("Air quality sensors".to_string()),
            version: Some("1.0.0".to_string()),
            enabled: true,
            retention_days: Some(365),
            sources: vec![],
            entity_schemas: vec![],
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("air-quality"));
        assert!(json.contains("1.0.0"));

        let deserialized: StreamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stream_id, "air-quality");
    }

    #[test]
    fn test_source_config_with_extra_fields() {
        let json = json!({
            "type": "mqtt",
            "enabled": true,
            "ndp_id": "sensor-001",
            "broker_url": "mosquitto:1883",
            "topic_pattern": "airgradient/+"
        });

        let config: SourceConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.source_type, "mqtt");
        assert_eq!(config.ndp_id, Some("sensor-001".to_string()));
        assert_eq!(config.extra.get("broker_url").unwrap(), "mosquitto:1883");
    }

    #[test]
    fn test_entity_schema_serialization() {
        let schema = EntitySchema {
            schema_name: "airgradient".to_string(),
            description: Some("AirGradient sensors".to_string()),
            device_class: Some("air_quality".to_string()),
            attributes: vec![Attribute {
                name: "pm25".to_string(),
                attr_type: Some("float".to_string()),
                unit: Some("ug/m3".to_string()),
                description: Some("Particulate Matter 2.5".to_string()),
                nullable: Some(false),
                range: Some(vec![0.0, 1000.0]),
            }],
        };

        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("airgradient"));
        assert!(json.contains("pm25"));
        assert!(json.contains("ug/m3"));

        let deserialized: EntitySchema = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.schema_name, "airgradient");
        assert_eq!(deserialized.attributes.len(), 1);
    }

    #[test]
    fn test_attribute_with_range() {
        let json = json!({
            "name": "temperature",
            "type": "float",
            "unit": "celsius",
            "nullable": true,
            "range": [-40.0, 85.0]
        });

        let attr: Attribute = serde_json::from_value(json).unwrap();
        assert_eq!(attr.name, "temperature");
        assert_eq!(attr.range, Some(vec![-40.0, 85.0]));
    }

    #[test]
    fn test_stream_info_from_config() {
        let config = StreamConfig {
            stream_id: "test-stream".to_string(),
            description: Some("Test".to_string()),
            version: Some("1.0.0".to_string()),
            enabled: true,
            retention_days: None,
            sources: vec![
                SourceConfig {
                    source_type: "mqtt".to_string(),
                    ..Default::default()
                },
                SourceConfig {
                    source_type: "http_poll".to_string(),
                    ..Default::default()
                },
            ],
            entity_schemas: vec![],
        };

        let info = StreamInfo::from(&config);
        assert_eq!(info.stream_id, "test-stream");
        assert_eq!(info.source_count, 2);
        assert_eq!(info.source_types, vec!["mqtt", "http_poll"]);
    }

    #[test]
    fn test_field_mapping_serialization() {
        let mapping = FieldMapping {
            path: "main.temp".to_string(),
            metric_name: Some("temperature".to_string()),
            unit: Some("celsius".to_string()),
        };

        let json = serde_json::to_string(&mapping).unwrap();
        assert!(json.contains("main.temp"));
        assert!(json.contains("temperature"));

        let deserialized: FieldMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path, "main.temp");
    }

    #[test]
    fn test_parser_config_with_mappings() {
        let json = json!({
            "parser_type": "json_path",
            "field_mappings": [
                {"path": "main.temp", "metric_name": "temperature", "unit": "celsius"},
                {"path": "main.humidity", "metric_name": "humidity", "unit": "percent"}
            ],
            "location_id_field": "id"
        });

        let config: ParserConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.parser_type, Some("json_path".to_string()));
        assert_eq!(config.field_mappings.len(), 2);
        assert!(config.extra.contains_key("location_id_field"));
    }
}
