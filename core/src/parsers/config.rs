//! Parser configuration types
//!
//! Defines the configuration structures for creating parsers from YAML/JSON config.

use crate::parsers::array_iterator::ArrayIteratorConfig;
use crate::parsers::raw_text::RawTextConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a parser instance
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

    /// For ColumnOrientedParser: column-specific configuration
    #[serde(default)]
    pub column_config: Option<ColumnOrientedConfig>,

    /// For RawTextParser: raw text parsing configuration (AIR-012)
    #[serde(default)]
    pub raw_text_config: Option<RawTextConfig>,
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
            column_config: None,
            raw_text_config: None,
        }
    }
}

/// Parser type enumeration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParserType {
    /// Extract all numeric fields from flat JSON object
    FlatJson,
    /// Extract specific fields using JSON path expressions
    JsonPath,
    /// Iterate over JSON arrays to produce multiple TimeSeriesPoints
    ArrayIterator,
    /// Extract metrics from column-oriented data structures
    ColumnOriented,
    /// Parse plain text payloads (e.g., "on", "off", "42.5") from Home Assistant
    /// AIR-012: Home Assistant Integration
    RawText,
    /// Custom parser (must be registered in code)
    Custom(String),
}

/// Field mapping for JsonPathParser
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

/// Mapping for a single column/metric in ColumnOrientedParser
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ColumnMapping {
    /// Path within metrics base (e.g., "temperature" for NWS)
    pub metric_path: String,

    /// Output field name in TimeSeriesPoint
    pub field_name: String,

    /// Path to values array within metric (default: "values")
    #[serde(default)]
    pub values_path: Option<String>,

    /// Path to timestamp within value entry (default: "validTime")
    #[serde(default)]
    pub timestamp_path: Option<String>,

    /// Path to value within entry (default: "value")
    #[serde(default)]
    pub value_path: Option<String>,
}

/// Configuration for column-oriented parser
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ColumnOrientedConfig {
    /// Base path to metrics container (e.g., "properties" for NWS)
    pub metrics_base_path: String,

    /// Column mappings: metric_path -> field_name
    pub columns: Vec<ColumnMapping>,

    /// Timestamp format variant
    pub timestamp_format: TimestampFormat,

    /// Unit conversions
    #[serde(default)]
    pub unit_conversions: HashMap<String, UnitConversion>,
}

/// Timestamp format variants for column-oriented parser
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimestampFormat {
    /// NWS format: "2025-12-23T00:00:00+00:00/PT1H"
    /// Split on "/" and parse first component
    Iso8601Duration,

    /// Open-Meteo format: Separate time array
    /// Time values are in a parallel array at specified path
    ParallelArray {
        /// Path to time array (e.g., "hourly.time")
        time_path: String,
    },
}

/// Unit conversion configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct UnitConversion {
    /// Source unit identifier
    pub from: String,

    /// Target unit identifier
    pub to: String,

    /// Optional conversion factor (for simple multiplication)
    #[serde(default)]
    pub factor: Option<f64>,

    /// Optional conversion formula (for complex conversions)
    #[serde(default)]
    pub formula: Option<ConversionFormula>,
}

impl UnitConversion {
    /// Apply conversion to value
    pub fn convert(&self, value: f64) -> f64 {
        if let Some(factor) = self.factor {
            value * factor
        } else if let Some(formula) = &self.formula {
            formula.apply(value)
        } else {
            value // No conversion
        }
    }
}

/// Conversion formula types
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversionFormula {
    /// Linear: (value * scale) + offset
    Linear { scale: f64, offset: f64 },

    /// Custom Rust code (future enhancement)
    Custom { code: String },
}

impl ConversionFormula {
    /// Apply formula to value
    pub fn apply(&self, value: f64) -> f64 {
        match self {
            ConversionFormula::Linear { scale, offset } => (value * scale) + offset,
            ConversionFormula::Custom { .. } => {
                // Future: compile and execute custom code
                value
            }
        }
    }
}
