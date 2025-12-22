//! Parser configuration types
//!
//! Defines the configuration structures for creating parsers from YAML/JSON config.

use crate::parsers::array_iterator::ArrayIteratorConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a parser instance
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParserConfig {
    /// Parser type identifier
    pub parser_type: ParserType,

    /// Field to use as location/sensor ID (JSON path)
    pub location_id_field: String,

    /// Default location ID if field not found
    #[serde(default)]
    pub default_location_id: Option<String>,

    /// Fields to skip during extraction (metadata fields)
    #[serde(default)]
    pub skip_fields: Vec<String>,

    /// For JsonPathParser: explicit field mappings
    #[serde(default)]
    pub field_mappings: Option<Vec<FieldMapping>>,

    /// Tags to add to all extracted points
    #[serde(default)]
    pub default_tags: HashMap<String, String>,

    /// For ArrayIteratorParser: array-specific configuration
    #[serde(default)]
    pub array_config: Option<ArrayIteratorConfig>,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            parser_type: ParserType::FlatJson,
            location_id_field: "location_id".to_string(),
            default_location_id: None,
            skip_fields: Vec::new(),
            field_mappings: None,
            default_tags: HashMap::new(),
            array_config: None,
        }
    }
}

/// Parser type enumeration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserType {
    /// Extract all numeric fields from flat JSON object
    FlatJson,
    /// Extract specific fields using JSON path expressions
    JsonPath,
    /// Iterate over JSON arrays to produce multiple TimeSeriesPoints
    ArrayIterator,
    /// Custom parser (must be registered in code)
    Custom(String),
}

/// Field mapping for JsonPathParser
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldMapping {
    /// JSON path to extract value (e.g., "main.temp", "list[0].components.pm2_5")
    pub path: String,
    /// Metric name for the extracted value
    pub metric_name: String,
    /// Optional unit for the metric
    #[serde(default)]
    pub unit: Option<String>,
    /// Optional transformation (e.g., kelvin_to_celsius)
    #[serde(default)]
    pub transform: Option<String>,
}
