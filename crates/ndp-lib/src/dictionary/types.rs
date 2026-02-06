//! Domain types for data dictionary sync.
//!
//! These structs represent the parsed config.json content that gets synced
//! to the `data_dictionary` schema in TimescaleDB. The caller is responsible
//! for loading and parsing configs; this module operates on pre-parsed structs.

use serde::{Deserialize, Serialize};

/// A single stream config entry ready for dictionary sync.
///
/// Maps to one row in `data_dictionary.streams` plus child rows in fields,
/// sources, entity_schemas, and optionally Silver layer tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDictionaryEntry {
    pub stream_id: String,
    pub description: Option<String>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_retention")]
    pub retention_days: i32,
    #[serde(default)]
    pub fields: Vec<FieldEntry>,
    #[serde(default)]
    pub sources: Vec<SourceEntry>,
    #[serde(default)]
    pub entity_schemas: Vec<EntitySchemaEntry>,
    pub silver_etl: Option<SilverEtlEntry>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}
fn default_true() -> bool {
    true
}
fn default_retention() -> i32 {
    90
}

/// A field in the Bronze schema.
///
/// Maps to one row in `data_dictionary.fields`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldEntry {
    pub name: String,
    pub field_type: String,
    #[serde(default = "default_true")]
    pub nullable: bool,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub validation_min: Option<f64>,
    pub validation_max: Option<f64>,
}

/// A data source feeding a stream.
///
/// Maps to one row in `data_dictionary.sources`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    pub source_id: String,
    pub source_type: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_json_object")]
    pub config: serde_json::Value,
    pub parser_type: Option<String>,
}

fn default_json_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// An entity schema definition (e.g. "AirGradient Indoor Monitor").
///
/// Maps to one row in `data_dictionary.entity_schemas`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchemaEntry {
    pub schema_name: String,
    pub description: Option<String>,
    pub device_class: Option<String>,
    #[serde(default)]
    pub attributes: Vec<EntitySchemaAttribute>,
}

/// An attribute within an entity schema.
///
/// Maps to one row in `data_dictionary.entity_schema_attributes`.
/// The `schema_id` FK is resolved via subselect at insert time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchemaAttribute {
    pub name: String,
    pub attribute_type: String,
    pub unit: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub nullable: bool,
    pub range_min: Option<f64>,
    pub range_max: Option<f64>,
}

/// Silver ETL configuration for a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverEtlEntry {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub target_table: String,
    pub description: Option<String>,
    pub grain: Option<String>,
    /// Extracted from `silver_etl.timestamp.target_field`.
    #[serde(default = "default_timestamp_col")]
    pub timestamp_column: String,
    #[serde(default)]
    pub field_mappings: Vec<SilverFieldMapping>,
    #[serde(default)]
    pub dq_rules: Vec<SilverTableDqRule>,
}

fn default_timestamp_col() -> String {
    "observation_time".to_string()
}

/// A field mapping from Bronze to Silver.
///
/// Maps to rows in `silver_columns`, `silver_lineage`, and optionally
/// `silver_dq_rules` (column-level).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverFieldMapping {
    pub source_path: String,
    pub target_column: String,
    /// Config type name: double_precision, smallint, text, etc.
    pub data_type: String,
    pub unit: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub nullable: bool,
    pub transform_type: Option<String>,
    #[serde(default)]
    pub dq_rules: Vec<SilverColumnDqRule>,
}

/// A column-level DQ rule (e.g. range_check).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverColumnDqRule {
    pub rule_name: String,
    #[serde(default = "default_json_object")]
    pub params: serde_json::Value,
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_action() -> String {
    "flag".to_string()
}

/// A table-level DQ rule (cross_field_check, freshness_check, etc.).
///
/// These are inserted into `silver_dq_rules` with `silver_column = NULL`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverTableDqRule {
    /// e.g. cross_field_check, freshness_check, rate_of_change, completeness_check
    pub rule_type: String,
    /// Derived name: for cross_field_check = the "name" field; for others = "{type}_{field}"
    pub rule_name: String,
    #[serde(default = "default_json_object")]
    pub params: serde_json::Value,
    #[serde(default = "default_action")]
    pub action: String,
}
