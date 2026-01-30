//! Dimension configuration types for reference/lookup data
//!
//! Dimensions are reference tables that load directly to Silver layer,
//! bypassing Bronze. They contain metadata that enriches timeseries observations.
//!
//! # Architecture (DP-013)
//!
//! Dimensions bypass Bronze because:
//! - They are metadata, not observations
//! - Small, rarely-changing data
//! - Need TRUNCATE/UPSERT patterns, not append-only
//!
//! ```text
//! TIMESERIES: MQTT/HTTP -> Bronze (Parquet) -> ETL -> Silver
//! DIMENSIONS: CSV -> DimensionLoader -> Silver (direct)
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for a dimension table
///
/// Example YAML:
/// ```yaml
/// dimension_id: entity-context
/// target:
///   table: entity_context
///   schema: silver
/// source:
///   type: csv
///   path: config/dimensions/entity_context.csv
/// schema:
///   fields:
///     - name: ndp_id
///       type: text
///       nullable: false
///     - name: category
///       type: text
///   primary_key: [ndp_id]
/// load:
///   strategy: truncate_and_load
///   batch_size: 1000
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionConfig {
    /// Unique identifier for the dimension (kebab-case)
    pub dimension_id: String,

    /// Target table configuration
    pub target: DimensionTarget,

    /// Source data configuration
    pub source: DimensionSource,

    /// Schema definition for the dimension table
    pub schema: DimensionSchema,

    /// Load behavior configuration
    #[serde(default)]
    pub load: LoadConfig,
}

/// Target table specification for dimension
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionTarget {
    /// Table name (without schema prefix)
    pub table: String,

    /// Database schema name (default: "silver")
    #[serde(default = "default_schema")]
    pub schema: String,
}

impl DimensionTarget {
    /// Get fully qualified table name (schema.table)
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

/// Source configuration for dimension data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionSource {
    /// Source type (currently only CSV supported)
    #[serde(rename = "type")]
    pub source_type: DimensionSourceType,

    /// Path to source file (relative to config root or absolute)
    pub path: PathBuf,

    /// Field delimiter character (default: ',')
    #[serde(default = "default_delimiter")]
    pub delimiter: char,
}

/// Supported dimension source types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DimensionSourceType {
    /// CSV file source
    Csv,
    // Future: Api, Database, etc.
}

/// Schema definition for dimension table
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionSchema {
    /// Field definitions
    pub fields: Vec<DimensionField>,

    /// Primary key columns for UPSERT operations
    #[serde(default)]
    pub primary_key: Vec<String>,

    /// Index definitions for the table
    #[serde(default)]
    pub indexes: Vec<IndexConfig>,
}

/// Configuration for a database index
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexConfig {
    /// Index name (used in CREATE INDEX statement)
    pub name: String,

    /// Columns to include in the index
    pub columns: Vec<String>,

    /// Whether this is a unique index (default: false)
    #[serde(default)]
    pub unique: bool,
}

impl DimensionSchema {
    /// Get field by name
    pub fn get_field(&self, name: &str) -> Option<&DimensionField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get column names as a vector
    pub fn column_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    /// Validate that all primary key fields exist in schema
    pub fn validate_primary_key(&self) -> Result<(), String> {
        for pk_col in &self.primary_key {
            if self.get_field(pk_col).is_none() {
                return Err(format!(
                    "Primary key column '{}' not found in schema fields",
                    pk_col
                ));
            }
        }
        Ok(())
    }

    /// Validate that all index columns exist in schema
    pub fn validate_indexes(&self) -> Result<(), String> {
        for index in &self.indexes {
            for col in &index.columns {
                if self.get_field(col).is_none() {
                    return Err(format!(
                        "Index '{}' references column '{}' not found in schema fields",
                        index.name, col
                    ));
                }
            }
        }
        Ok(())
    }

    /// Full validation of schema (primary key and indexes)
    pub fn validate(&self) -> Result<(), String> {
        self.validate_primary_key()?;
        self.validate_indexes()?;
        Ok(())
    }
}

impl IndexConfig {
    /// Create a new index configuration
    pub fn new(name: impl Into<String>, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            columns,
            unique: false,
        }
    }

    /// Set index as unique
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }
}

/// Field definition within dimension schema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DimensionField {
    /// Column name
    pub name: String,

    /// Data type for the column
    #[serde(rename = "type")]
    pub field_type: FieldType,

    /// Whether field can be null (default: true)
    #[serde(default = "default_nullable")]
    pub nullable: bool,
}

impl DimensionField {
    /// Create a new required (non-nullable) field
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            nullable: true,
        }
    }

    /// Set field as non-nullable (required)
    pub fn required(mut self) -> Self {
        self.nullable = false;
        self
    }
}

/// Data types supported for dimension fields
///
/// Maps to PostgreSQL types in Silver layer (TimescaleDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// TEXT - Variable-length string
    Text,
    /// BIGINT - 64-bit integer
    Integer,
    /// DOUBLE PRECISION - 64-bit floating point
    Float,
    /// BOOLEAN - true/false
    Boolean,
    /// TIMESTAMPTZ - Timestamp with timezone
    Timestamp,
    /// JSONB - Binary JSON
    Jsonb,
}

impl FieldType {
    /// Convert to PostgreSQL type string
    pub fn to_pg_type(&self) -> &'static str {
        match self {
            FieldType::Text => "TEXT",
            FieldType::Integer => "BIGINT",
            FieldType::Float => "DOUBLE PRECISION",
            FieldType::Boolean => "BOOLEAN",
            FieldType::Timestamp => "TIMESTAMPTZ",
            FieldType::Jsonb => "JSONB",
        }
    }

    /// Parse a string value according to this field type
    pub fn parse_value(&self, value: &str) -> Result<serde_json::Value, String> {
        if value.is_empty() {
            return Ok(serde_json::Value::Null);
        }

        match self {
            FieldType::Text => Ok(serde_json::Value::String(value.to_string())),
            FieldType::Integer => value
                .parse::<i64>()
                .map(|v| serde_json::Value::Number(v.into()))
                .map_err(|e| format!("Invalid integer '{}': {}", value, e)),
            FieldType::Float => value
                .parse::<f64>()
                .map(|v| {
                    serde_json::Number::from_f64(v)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                })
                .map_err(|e| format!("Invalid float '{}': {}", value, e)),
            FieldType::Boolean => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "t" | "y" => Ok(serde_json::Value::Bool(true)),
                "false" | "0" | "no" | "f" | "n" => Ok(serde_json::Value::Bool(false)),
                _ => Err(format!("Invalid boolean '{}'", value)),
            },
            FieldType::Timestamp => {
                // Validate timestamp format (basic check)
                if chrono::DateTime::parse_from_rfc3339(value).is_ok() {
                    Ok(serde_json::Value::String(value.to_string()))
                } else if chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").is_ok()
                {
                    Ok(serde_json::Value::String(value.to_string()))
                } else {
                    Err(format!(
                        "Invalid timestamp '{}': expected ISO 8601 format",
                        value
                    ))
                }
            }
            FieldType::Jsonb => {
                serde_json::from_str(value).map_err(|e| format!("Invalid JSON '{}': {}", value, e))
            }
        }
    }
}

/// Load behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadConfig {
    /// Load strategy (default: truncate_and_load)
    #[serde(default)]
    pub strategy: LoadStrategy,

    /// Batch size for INSERT operations (default: 1000)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            strategy: LoadStrategy::default(),
            batch_size: default_batch_size(),
        }
    }
}

/// Load strategy for dimension updates
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadStrategy {
    /// Delete all existing data, insert new (default for dimensions)
    /// Uses DELETE + INSERT within a transaction
    #[default]
    TruncateAndLoad,

    /// Insert new rows, update existing based on primary_key
    /// Uses INSERT ON CONFLICT DO UPDATE
    Upsert,
}

// Default value functions for serde

fn default_schema() -> String {
    "silver".to_string()
}

fn default_delimiter() -> char {
    ','
}

fn default_nullable() -> bool {
    true
}

fn default_batch_size() -> usize {
    1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_config_deserialize() {
        let yaml = r#"
dimension_id: entity-context
target:
  table: entity_context
  schema: silver
source:
  type: csv
  path: config/dimensions/entity_context.csv
schema:
  fields:
    - name: ndp_id
      type: text
      nullable: false
    - name: category
      type: text
  primary_key:
    - ndp_id
load:
  strategy: truncate_and_load
  batch_size: 500
"#;

        let config: DimensionConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.dimension_id, "entity-context");
        assert_eq!(config.target.table, "entity_context");
        assert_eq!(config.target.schema, "silver");
        assert_eq!(config.target.qualified_name(), "silver.entity_context");
        assert_eq!(config.source.source_type, DimensionSourceType::Csv);
        assert_eq!(config.schema.fields.len(), 2);
        assert_eq!(config.schema.primary_key, vec!["ndp_id"]);
        assert!(!config.schema.fields[0].nullable);
        assert!(config.schema.fields[1].nullable);
        assert_eq!(config.load.strategy, LoadStrategy::TruncateAndLoad);
        assert_eq!(config.load.batch_size, 500);
    }

    #[test]
    fn test_dimension_config_defaults() {
        let yaml = r#"
dimension_id: test-dim
target:
  table: test_table
source:
  type: csv
  path: test.csv
schema:
  fields:
    - name: id
      type: text
"#;

        let config: DimensionConfig = serde_yaml::from_str(yaml).unwrap();

        // Check defaults
        assert_eq!(config.target.schema, "silver");
        assert_eq!(config.source.delimiter, ',');
        assert!(config.schema.fields[0].nullable);
        assert_eq!(config.load.strategy, LoadStrategy::TruncateAndLoad);
        assert_eq!(config.load.batch_size, 1000);
    }

    #[test]
    fn test_field_type_to_pg_type() {
        assert_eq!(FieldType::Text.to_pg_type(), "TEXT");
        assert_eq!(FieldType::Integer.to_pg_type(), "BIGINT");
        assert_eq!(FieldType::Float.to_pg_type(), "DOUBLE PRECISION");
        assert_eq!(FieldType::Boolean.to_pg_type(), "BOOLEAN");
        assert_eq!(FieldType::Timestamp.to_pg_type(), "TIMESTAMPTZ");
        assert_eq!(FieldType::Jsonb.to_pg_type(), "JSONB");
    }

    #[test]
    fn test_field_type_parse_text() {
        let result = FieldType::Text.parse_value("hello world");
        assert_eq!(
            result.unwrap(),
            serde_json::Value::String("hello world".to_string())
        );
    }

    #[test]
    fn test_field_type_parse_integer() {
        let result = FieldType::Integer.parse_value("42");
        assert_eq!(result.unwrap(), serde_json::json!(42));

        let result = FieldType::Integer.parse_value("not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_field_type_parse_float() {
        let result = FieldType::Float.parse_value("3.14");
        assert_eq!(result.unwrap(), serde_json::json!(3.14));
    }

    #[test]
    fn test_field_type_parse_boolean() {
        assert_eq!(
            FieldType::Boolean.parse_value("true").unwrap(),
            serde_json::json!(true)
        );
        assert_eq!(
            FieldType::Boolean.parse_value("1").unwrap(),
            serde_json::json!(true)
        );
        assert_eq!(
            FieldType::Boolean.parse_value("yes").unwrap(),
            serde_json::json!(true)
        );
        assert_eq!(
            FieldType::Boolean.parse_value("false").unwrap(),
            serde_json::json!(false)
        );
        assert_eq!(
            FieldType::Boolean.parse_value("0").unwrap(),
            serde_json::json!(false)
        );
        assert!(FieldType::Boolean.parse_value("maybe").is_err());
    }

    #[test]
    fn test_field_type_parse_empty_is_null() {
        let result = FieldType::Text.parse_value("");
        assert_eq!(result.unwrap(), serde_json::Value::Null);

        let result = FieldType::Integer.parse_value("");
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn test_schema_validate_primary_key() {
        let schema = DimensionSchema {
            fields: vec![
                DimensionField::new("id", FieldType::Text),
                DimensionField::new("name", FieldType::Text),
            ],
            primary_key: vec!["id".to_string()],
            indexes: vec![],
        };

        assert!(schema.validate_primary_key().is_ok());

        let bad_schema = DimensionSchema {
            fields: vec![DimensionField::new("id", FieldType::Text)],
            primary_key: vec!["nonexistent".to_string()],
            indexes: vec![],
        };

        assert!(bad_schema.validate_primary_key().is_err());
    }

    #[test]
    fn test_schema_column_names() {
        let schema = DimensionSchema {
            fields: vec![
                DimensionField::new("a", FieldType::Text),
                DimensionField::new("b", FieldType::Integer),
                DimensionField::new("c", FieldType::Float),
            ],
            primary_key: vec![],
            indexes: vec![],
        };

        assert_eq!(schema.column_names(), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_schema_validate_indexes() {
        let schema = DimensionSchema {
            fields: vec![
                DimensionField::new("id", FieldType::Text),
                DimensionField::new("category", FieldType::Text),
            ],
            primary_key: vec!["id".to_string()],
            indexes: vec![IndexConfig::new(
                "idx_category",
                vec!["category".to_string()],
            )],
        };

        assert!(schema.validate_indexes().is_ok());
        assert!(schema.validate().is_ok());

        let bad_schema = DimensionSchema {
            fields: vec![DimensionField::new("id", FieldType::Text)],
            primary_key: vec![],
            indexes: vec![IndexConfig::new(
                "idx_missing",
                vec!["nonexistent".to_string()],
            )],
        };

        assert!(bad_schema.validate_indexes().is_err());
    }

    #[test]
    fn test_index_config_with_indexes() {
        let yaml = r#"
dimension_id: test-with-indexes
target:
  table: test_table
source:
  type: csv
  path: test.csv
schema:
  fields:
    - name: id
      type: text
      nullable: false
    - name: category
      type: text
    - name: location
      type: text
  primary_key:
    - id
  indexes:
    - name: idx_category
      columns: [category]
    - name: idx_location_unique
      columns: [location]
      unique: true
"#;

        let config: DimensionConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.schema.indexes.len(), 2);
        assert_eq!(config.schema.indexes[0].name, "idx_category");
        assert_eq!(config.schema.indexes[0].columns, vec!["category"]);
        assert!(!config.schema.indexes[0].unique);
        assert_eq!(config.schema.indexes[1].name, "idx_location_unique");
        assert!(config.schema.indexes[1].unique);
    }

    #[test]
    fn test_dimension_field_builder() {
        let field = DimensionField::new("user_id", FieldType::Text).required();

        assert_eq!(field.name, "user_id");
        assert_eq!(field.field_type, FieldType::Text);
        assert!(!field.nullable);
    }

    #[test]
    fn test_load_strategy_serialization() {
        let truncate = LoadStrategy::TruncateAndLoad;
        let upsert = LoadStrategy::Upsert;

        let truncate_json = serde_json::to_string(&truncate).unwrap();
        let upsert_json = serde_json::to_string(&upsert).unwrap();

        assert_eq!(truncate_json, "\"truncate_and_load\"");
        assert_eq!(upsert_json, "\"upsert\"");

        let restored_truncate: LoadStrategy = serde_json::from_str(&truncate_json).unwrap();
        let restored_upsert: LoadStrategy = serde_json::from_str(&upsert_json).unwrap();

        assert_eq!(restored_truncate, LoadStrategy::TruncateAndLoad);
        assert_eq!(restored_upsert, LoadStrategy::Upsert);
    }
}
