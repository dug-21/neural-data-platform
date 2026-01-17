//! Storage types for Bronze layer metadata.
//!
//! Defines data structures returned by `BronzeStorage` trait methods.
//! These types support the `BronzeStorage` trait (ADR-002) and are designed
//! for JSON serialization in MCP tool responses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Parquet Schema Types (for describe_schema tool)
// ============================================================================

/// Schema information for a Bronze stream's Parquet files.
///
/// Returned by `BronzeStorage::get_schema()`. Contains both the
/// Parquet column definitions and analyzed `raw_payload` structure.
///
/// # Example JSON
///
/// ```json
/// {
///   "stream_id": "air-quality",
///   "fields": [
///     {"name": "timestamp", "data_type": "INT64", "nullable": false},
///     {"name": "source_id", "data_type": "UTF8", "nullable": false},
///     {"name": "raw_payload", "data_type": "UTF8", "nullable": false}
///   ],
///   "raw_payload_structure": {
///     "keys": ["pm25", "temperature", "humidity"],
///     "nested": {"main": ["temp", "pressure"]}
///   },
///   "file_path": "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParquetSchemaInfo {
    /// Stream identifier this schema belongs to.
    pub stream_id: String,

    /// Column definitions from Parquet schema.
    pub fields: Vec<FieldInfo>,

    /// Analyzed structure of the `raw_payload` JSON column.
    ///
    /// None if raw_payload could not be analyzed (e.g., no data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_payload_structure: Option<RawPayloadStructure>,

    /// Absolute path to the analyzed Parquet file.
    pub file_path: String,
}

impl ParquetSchemaInfo {
    /// Create a new ParquetSchemaInfo with required fields.
    pub fn new(stream_id: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            fields: Vec::new(),
            raw_payload_structure: None,
            file_path: file_path.into(),
        }
    }

    /// Builder method to set fields.
    pub fn with_fields(mut self, fields: Vec<FieldInfo>) -> Self {
        self.fields = fields;
        self
    }

    /// Builder method to set raw_payload_structure.
    pub fn with_payload_structure(mut self, structure: RawPayloadStructure) -> Self {
        self.raw_payload_structure = Some(structure);
        self
    }
}

/// Information about a single Parquet column.
///
/// Maps to Arrow field definitions with optional metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldInfo {
    /// Column name.
    pub name: String,

    /// Arrow/Parquet data type as string.
    ///
    /// Common types: "INT64", "UTF8", "DOUBLE", "BOOLEAN", "TIMESTAMP"
    pub data_type: String,

    /// Whether the field can contain null values.
    #[serde(default)]
    pub nullable: bool,

    /// Optional description or unit information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FieldInfo {
    /// Create a new FieldInfo with name and data type.
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            nullable: true,
            description: None,
        }
    }

    /// Builder method to set nullable.
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Builder method to set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Structure of the `raw_payload` JSON column.
///
/// Used to help development agents understand the internal structure
/// of source data stored in Bronze. Extracted by sampling actual data.
///
/// # Example
///
/// For a raw_payload like:
/// ```json
/// {
///   "main": {"temp": 20.5, "humidity": 65},
///   "wind": {"speed": 5.2, "deg": 180}
/// }
/// ```
///
/// Returns:
/// ```json
/// {
///   "keys": ["main", "wind"],
///   "nested": {
///     "main": ["temp", "humidity"],
///     "wind": ["speed", "deg"]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RawPayloadStructure {
    /// Top-level keys found in raw_payload objects.
    pub keys: Vec<String>,

    /// Nested object keys for fields that contain objects.
    ///
    /// Key is the parent field name, value is list of child keys.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub nested: HashMap<String, Vec<String>>,
}

impl RawPayloadStructure {
    /// Create an empty RawPayloadStructure.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set keys.
    pub fn with_keys(mut self, keys: Vec<String>) -> Self {
        self.keys = keys;
        self
    }

    /// Builder method to add a nested structure.
    pub fn with_nested(mut self, parent: impl Into<String>, children: Vec<String>) -> Self {
        self.nested.insert(parent.into(), children);
        self
    }
}

// ============================================================================
// Stream Storage Types (for list_streams tool)
// ============================================================================

/// Metadata about a Bronze stream's storage.
///
/// Returned by `BronzeStorage::list()` for each discovered stream.
/// Provides information about the stream's storage location, latest
/// partition, and basic statistics.
///
/// # Example JSON
///
/// ```json
/// {
///   "stream_id": "air-quality",
///   "latest_partition": "year=2026/month=01/day=03",
///   "file_size_bytes": 1048576,
///   "file_modified": "2026-01-03T10:30:00Z",
///   "row_count": 1440
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamStorageInfo {
    /// Stream identifier (directory name under raw_path).
    ///
    /// Example: "air-quality", "outdoor-weather"
    pub stream_id: String,

    /// Latest partition path relative to stream directory.
    ///
    /// Hive-style format: "year=YYYY/month=MM/day=DD"
    /// None if no partitions exist yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_partition: Option<String>,

    /// Size of the latest partition file in bytes.
    ///
    /// None if no partitions exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<u64>,

    /// Last modification time of the latest partition file.
    ///
    /// None if no partitions exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_modified: Option<DateTime<Utc>>,

    /// Number of rows in the latest partition file.
    ///
    /// None if not yet scanned (requires reading Parquet metadata).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u64>,
}

impl StreamStorageInfo {
    /// Create a new StreamStorageInfo with just the stream_id.
    ///
    /// Other fields are set to None and can be populated later.
    pub fn new(stream_id: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            latest_partition: None,
            file_size_bytes: None,
            file_modified: None,
            row_count: None,
        }
    }

    /// Builder method to set the latest partition.
    pub fn with_latest_partition(mut self, partition: impl Into<String>) -> Self {
        self.latest_partition = Some(partition.into());
        self
    }

    /// Builder method to set the file size.
    pub fn with_file_size(mut self, size: u64) -> Self {
        self.file_size_bytes = Some(size);
        self
    }

    /// Builder method to set the file modified time.
    pub fn with_modified(mut self, modified: DateTime<Utc>) -> Self {
        self.file_modified = Some(modified);
        self
    }

    /// Builder method to set the row count.
    pub fn with_row_count(mut self, count: u64) -> Self {
        self.row_count = Some(count);
        self
    }
}

/// Structure of JSON fields found in raw_payload.
///
/// Used by `describe_schema` to help development agents understand
/// the internal structure of source data stored in the `raw_payload`
/// column.
///
/// # Field Types
///
/// - Top-level fields are listed with their JSON types
/// - Nested objects are recursively analyzed
/// - Arrays include element type information
///
/// # Example
///
/// For raw_payload like:
/// ```json
/// {
///   "pm25": 12.5,
///   "temperature": 23.4,
///   "status": "active",
///   "readings": [{"co2": 450}]
/// }
/// ```
///
/// Returns:
/// ```json
/// {
///   "fields": {
///     "pm25": "number",
///     "temperature": "number",
///     "status": "string",
///     "readings": "array"
///   },
///   "nested_objects": {
///     "readings": ["co2"]
///   },
///   "sample_count": 10
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonStructure {
    /// Top-level field names mapped to their JSON types.
    ///
    /// Types: "string", "number", "boolean", "null", "object", "array"
    pub fields: HashMap<String, String>,

    /// Nested object field names.
    ///
    /// For each field that is an object or contains objects,
    /// lists the keys found within.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub nested_objects: HashMap<String, Vec<String>>,

    /// Number of samples analyzed to determine structure.
    ///
    /// Multiple samples are merged to capture optional fields.
    pub sample_count: usize,
}

impl JsonStructure {
    /// Create an empty JsonStructure.
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a JSON value and merge its structure into this instance.
    ///
    /// Call multiple times on different samples to build a complete
    /// picture of the field structure (handling optional fields).
    ///
    /// # Arguments
    ///
    /// * `value` - A JSON value to analyze (typically from raw_payload)
    pub fn analyze(&mut self, value: &serde_json::Value) {
        self.sample_count += 1;

        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                // Record field type
                let type_name = Self::json_type_name(val);
                self.fields.insert(key.clone(), type_name);

                // Track nested object keys
                if let Some(nested_obj) = val.as_object() {
                    let nested_keys: Vec<String> = nested_obj.keys().cloned().collect();
                    self.nested_objects.insert(key.clone(), nested_keys);
                } else if let Some(arr) = val.as_array() {
                    // Check if array contains objects
                    for elem in arr {
                        if let Some(elem_obj) = elem.as_object() {
                            let elem_keys: Vec<String> = elem_obj.keys().cloned().collect();
                            // Merge with existing keys
                            let entry = self.nested_objects.entry(key.clone()).or_default();
                            for k in elem_keys {
                                if !entry.contains(&k) {
                                    entry.push(k);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get the JSON type name for a value.
    fn json_type_name(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(_) => "boolean".to_string(),
            serde_json::Value::Number(_) => "number".to_string(),
            serde_json::Value::String(_) => "string".to_string(),
            serde_json::Value::Array(_) => "array".to_string(),
            serde_json::Value::Object(_) => "object".to_string(),
        }
    }

    /// Merge another JsonStructure into this one.
    ///
    /// Used to combine analysis from multiple samples.
    pub fn merge(&mut self, other: &JsonStructure) {
        self.sample_count += other.sample_count;

        // Merge fields (later values override)
        for (key, value) in &other.fields {
            self.fields.insert(key.clone(), value.clone());
        }

        // Merge nested objects (combine keys)
        for (key, keys) in &other.nested_objects {
            let entry = self.nested_objects.entry(key.clone()).or_default();
            for k in keys {
                if !entry.contains(k) {
                    entry.push(k.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stream_storage_info_new() {
        let info = StreamStorageInfo::new("air-quality");
        assert_eq!(info.stream_id, "air-quality");
        assert!(info.latest_partition.is_none());
        assert!(info.file_size_bytes.is_none());
    }

    #[test]
    fn test_stream_storage_info_builder() {
        let info = StreamStorageInfo::new("air-quality")
            .with_latest_partition("year=2026/month=01/day=03")
            .with_file_size(1024)
            .with_row_count(100);

        assert_eq!(info.stream_id, "air-quality");
        assert_eq!(
            info.latest_partition,
            Some("year=2026/month=01/day=03".to_string())
        );
        assert_eq!(info.file_size_bytes, Some(1024));
        assert_eq!(info.row_count, Some(100));
    }

    #[test]
    fn test_json_structure_analyze_simple() {
        let mut structure = JsonStructure::new();
        structure.analyze(&json!({
            "pm25": 12.5,
            "temperature": 23.4,
            "status": "active"
        }));

        assert_eq!(structure.sample_count, 1);
        assert_eq!(structure.fields.get("pm25"), Some(&"number".to_string()));
        assert_eq!(
            structure.fields.get("temperature"),
            Some(&"number".to_string())
        );
        assert_eq!(structure.fields.get("status"), Some(&"string".to_string()));
    }

    #[test]
    fn test_json_structure_analyze_nested() {
        let mut structure = JsonStructure::new();
        structure.analyze(&json!({
            "sensor": {
                "id": "abc123",
                "model": "AirGradient"
            },
            "readings": [
                {"co2": 450, "tvoc": 100}
            ]
        }));

        assert_eq!(structure.fields.get("sensor"), Some(&"object".to_string()));
        assert_eq!(structure.fields.get("readings"), Some(&"array".to_string()));

        // Check nested keys
        let sensor_keys = structure.nested_objects.get("sensor").unwrap();
        assert!(sensor_keys.contains(&"id".to_string()));
        assert!(sensor_keys.contains(&"model".to_string()));

        let readings_keys = structure.nested_objects.get("readings").unwrap();
        assert!(readings_keys.contains(&"co2".to_string()));
        assert!(readings_keys.contains(&"tvoc".to_string()));
    }

    #[test]
    fn test_json_structure_merge() {
        let mut s1 = JsonStructure::new();
        s1.analyze(&json!({
            "field_a": 1,
            "field_b": "test"
        }));

        let mut s2 = JsonStructure::new();
        s2.analyze(&json!({
            "field_a": 2,
            "field_c": true
        }));

        s1.merge(&s2);

        assert_eq!(s1.sample_count, 2);
        assert!(s1.fields.contains_key("field_a"));
        assert!(s1.fields.contains_key("field_b"));
        assert!(s1.fields.contains_key("field_c"));
    }

    #[test]
    fn test_stream_storage_info_serialization() {
        let info = StreamStorageInfo::new("test-stream")
            .with_latest_partition("year=2026/month=01/day=01")
            .with_file_size(2048);

        let json_str = serde_json::to_string(&info).unwrap();
        assert!(json_str.contains("test-stream"));
        assert!(json_str.contains("year=2026"));
        assert!(json_str.contains("2048"));
    }

    // ========== ParquetSchemaInfo Tests ==========

    #[test]
    fn test_parquet_schema_info_new() {
        let schema = ParquetSchemaInfo::new("air-quality", "/data/raw/air-quality/data.parquet");
        assert_eq!(schema.stream_id, "air-quality");
        assert_eq!(schema.file_path, "/data/raw/air-quality/data.parquet");
        assert!(schema.fields.is_empty());
        assert!(schema.raw_payload_structure.is_none());
    }

    #[test]
    fn test_parquet_schema_info_builder() {
        let schema = ParquetSchemaInfo::new("test-stream", "/path/to/file.parquet")
            .with_fields(vec![
                FieldInfo::new("timestamp", "INT64"),
                FieldInfo::new("source_id", "UTF8"),
            ])
            .with_payload_structure(RawPayloadStructure::new().with_keys(vec!["pm25".to_string()]));

        assert_eq!(schema.fields.len(), 2);
        assert!(schema.raw_payload_structure.is_some());
        let payload_struct = schema.raw_payload_structure.unwrap();
        assert!(payload_struct.keys.contains(&"pm25".to_string()));
    }

    #[test]
    fn test_parquet_schema_info_serialization() {
        let schema =
            ParquetSchemaInfo::new("outdoor-weather", "/data/raw/outdoor-weather/data.parquet")
                .with_fields(vec![
                    FieldInfo::new("timestamp", "INT64").with_nullable(false),
                    FieldInfo::new("raw_payload", "UTF8"),
                ]);

        let json_str = serde_json::to_string(&schema).unwrap();
        assert!(json_str.contains("outdoor-weather"));
        assert!(json_str.contains("timestamp"));
        assert!(json_str.contains("INT64"));
    }

    // ========== FieldInfo Tests ==========

    #[test]
    fn test_field_info_new() {
        let field = FieldInfo::new("timestamp", "INT64");
        assert_eq!(field.name, "timestamp");
        assert_eq!(field.data_type, "INT64");
        assert!(field.nullable); // default is true
        assert!(field.description.is_none());
    }

    #[test]
    fn test_field_info_builder() {
        let field = FieldInfo::new("temperature", "DOUBLE")
            .with_nullable(false)
            .with_description("Temperature in Celsius");

        assert_eq!(field.name, "temperature");
        assert_eq!(field.data_type, "DOUBLE");
        assert!(!field.nullable);
        assert_eq!(
            field.description,
            Some("Temperature in Celsius".to_string())
        );
    }

    #[test]
    fn test_field_info_equality() {
        let f1 = FieldInfo::new("pm25", "DOUBLE");
        let f2 = FieldInfo::new("pm25", "DOUBLE");
        let f3 = FieldInfo::new("pm25", "INT64");

        assert_eq!(f1, f2);
        assert_ne!(f1, f3);
    }

    // ========== RawPayloadStructure Tests ==========

    #[test]
    fn test_raw_payload_structure_new() {
        let structure = RawPayloadStructure::new();
        assert!(structure.keys.is_empty());
        assert!(structure.nested.is_empty());
    }

    #[test]
    fn test_raw_payload_structure_builder() {
        let structure = RawPayloadStructure::new()
            .with_keys(vec!["main".to_string(), "wind".to_string()])
            .with_nested("main", vec!["temp".to_string(), "humidity".to_string()])
            .with_nested("wind", vec!["speed".to_string(), "deg".to_string()]);

        assert_eq!(structure.keys.len(), 2);
        assert!(structure.keys.contains(&"main".to_string()));
        assert!(structure.keys.contains(&"wind".to_string()));

        let main_nested = structure.nested.get("main").unwrap();
        assert!(main_nested.contains(&"temp".to_string()));
        assert!(main_nested.contains(&"humidity".to_string()));
    }

    #[test]
    fn test_raw_payload_structure_serialization() {
        let structure = RawPayloadStructure::new()
            .with_keys(vec!["pm25".to_string(), "temperature".to_string()]);

        let json_str = serde_json::to_string(&structure).unwrap();
        assert!(json_str.contains("pm25"));
        assert!(json_str.contains("temperature"));
        // nested should be omitted when empty
        assert!(!json_str.contains("nested"));
    }

    #[test]
    fn test_raw_payload_structure_serialization_with_nested() {
        let structure = RawPayloadStructure::new()
            .with_keys(vec!["main".to_string()])
            .with_nested("main", vec!["temp".to_string()]);

        let json_str = serde_json::to_string(&structure).unwrap();
        assert!(json_str.contains("nested"));
        assert!(json_str.contains("temp"));
    }

    // ========== Type Equality and Clone Tests ==========

    #[test]
    fn test_parquet_schema_info_clone() {
        let schema = ParquetSchemaInfo::new("stream", "/path")
            .with_fields(vec![FieldInfo::new("f1", "INT64")]);

        let cloned = schema.clone();
        assert_eq!(schema, cloned);
    }

    #[test]
    fn test_field_info_serialization_skip_none_description() {
        let field = FieldInfo::new("test", "INT64");
        let json_str = serde_json::to_string(&field).unwrap();
        // description should be omitted when None
        assert!(!json_str.contains("description"));
    }
}

// ============================================================================
// Silver Layer Types (dp-010)
// ============================================================================

/// Information about a Silver layer hypertable.
///
/// Returned by `SilverStorage::list_tables()`. Contains metadata about
/// each TimescaleDB hypertable in the Silver layer including chunk info.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SilverTableInfo {
    /// Table name in Silver layer (e.g., "air_quality_readings").
    pub table_name: String,

    /// Human-readable description of the table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Data grain (e.g., "per_reading", "hourly", "daily").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grain: Option<String>,

    /// Bronze streams that feed this table.
    pub source_streams: Vec<String>,

    /// Whether this is a TimescaleDB hypertable.
    pub is_hypertable: bool,

    /// Chunk interval for hypertables (e.g., "1 day", "1 hour").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_interval: Option<String>,

    /// Total row count in the table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<i64>,

    /// Total bytes used by the table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<i64>,
}

impl SilverTableInfo {
    /// Create a new SilverTableInfo with required fields.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            description: None,
            grain: None,
            source_streams: Vec::new(),
            is_hypertable: false,
            chunk_interval: None,
            row_count: None,
            total_bytes: None,
        }
    }

    /// Builder method to set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder method to set grain.
    pub fn with_grain(mut self, grain: impl Into<String>) -> Self {
        self.grain = Some(grain.into());
        self
    }

    /// Builder method to set source streams.
    pub fn with_source_streams(mut self, streams: Vec<String>) -> Self {
        self.source_streams = streams;
        self
    }

    /// Builder method to mark as hypertable.
    pub fn with_hypertable(mut self, is_hypertable: bool, chunk_interval: Option<String>) -> Self {
        self.is_hypertable = is_hypertable;
        self.chunk_interval = chunk_interval;
        self
    }

    /// Builder method to set row count.
    pub fn with_row_count(mut self, count: i64) -> Self {
        self.row_count = Some(count);
        self
    }

    /// Builder method to set total bytes.
    pub fn with_total_bytes(mut self, bytes: i64) -> Self {
        self.total_bytes = Some(bytes);
        self
    }
}

/// Detailed schema description for a Silver table.
///
/// Returned by `SilverStorage::describe_table()`. Includes column definitions
/// and TimescaleDB-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SilverTableDescription {
    /// Table name.
    pub table_name: String,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Column definitions.
    pub columns: Vec<SilverColumnInfo>,

    /// Hypertable metadata if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hypertable_info: Option<HypertableInfo>,
}

impl SilverTableDescription {
    /// Create a new SilverTableDescription.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            description: None,
            columns: Vec::new(),
            hypertable_info: None,
        }
    }

    /// Builder method to set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder method to set columns.
    pub fn with_columns(mut self, columns: Vec<SilverColumnInfo>) -> Self {
        self.columns = columns;
        self
    }

    /// Builder method to set hypertable info.
    pub fn with_hypertable_info(mut self, info: HypertableInfo) -> Self {
        self.hypertable_info = Some(info);
        self
    }
}

/// Information about a column in a Silver table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SilverColumnInfo {
    /// Column name.
    pub column_name: String,

    /// PostgreSQL data type.
    pub data_type: String,

    /// Unit of measurement (e.g., "celsius", "ug/m3").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the column can contain NULL values.
    pub nullable: bool,

    /// Whether this column is part of the primary key.
    pub is_primary_key: bool,
}

impl SilverColumnInfo {
    /// Create a new SilverColumnInfo.
    pub fn new(column_name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            column_name: column_name.into(),
            data_type: data_type.into(),
            unit: None,
            description: None,
            nullable: true,
            is_primary_key: false,
        }
    }

    /// Builder method to set unit.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Builder method to set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder method to set nullable.
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Builder method to set is_primary_key.
    pub fn with_primary_key(mut self, is_pk: bool) -> Self {
        self.is_primary_key = is_pk;
        self
    }
}

/// TimescaleDB hypertable metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HypertableInfo {
    /// Time column name used for partitioning.
    pub time_column: String,

    /// Chunk interval (e.g., "1 day", "1 hour").
    pub chunk_interval: String,

    /// Number of chunks in the hypertable.
    pub chunk_count: i64,

    /// Total bytes used by all chunks.
    pub total_bytes: i64,
}

impl HypertableInfo {
    /// Create a new HypertableInfo.
    pub fn new(time_column: impl Into<String>, chunk_interval: impl Into<String>) -> Self {
        Self {
            time_column: time_column.into(),
            chunk_interval: chunk_interval.into(),
            chunk_count: 0,
            total_bytes: 0,
        }
    }

    /// Builder method to set chunk count.
    pub fn with_chunk_count(mut self, count: i64) -> Self {
        self.chunk_count = count;
        self
    }

    /// Builder method to set total bytes.
    pub fn with_total_bytes(mut self, bytes: i64) -> Self {
        self.total_bytes = bytes;
        self
    }
}

/// Filters for sampling Silver table data.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SampleFilters {
    /// Only include rows after this timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,

    /// Only include rows before this timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<DateTime<Utc>>,

    /// Order by column (default: time column descending).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<String>,
}

impl SampleFilters {
    /// Create empty filters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set since timestamp.
    pub fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Builder method to set until timestamp.
    pub fn with_until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    /// Builder method to set order_by column.
    pub fn with_order_by(mut self, order_by: impl Into<String>) -> Self {
        self.order_by = Some(order_by.into());
        self
    }
}

/// Statistics for a Silver table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SilverTableStats {
    /// Table name.
    pub table_name: String,

    /// Total row count.
    pub row_count: i64,

    /// Time range of data in the table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,

    /// Number of chunks (for hypertables).
    pub chunk_count: i64,

    /// Total bytes used.
    pub total_bytes: i64,

    /// Data quality summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dq_summary: Option<DqSummary>,
}

impl SilverTableStats {
    /// Create new SilverTableStats.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            row_count: 0,
            time_range: None,
            chunk_count: 0,
            total_bytes: 0,
            dq_summary: None,
        }
    }

    /// Builder method to set row count.
    pub fn with_row_count(mut self, count: i64) -> Self {
        self.row_count = count;
        self
    }

    /// Builder method to set time range.
    pub fn with_time_range(mut self, min: DateTime<Utc>, max: DateTime<Utc>) -> Self {
        self.time_range = Some(TimeRange { min, max });
        self
    }

    /// Builder method to set chunk count.
    pub fn with_chunk_count(mut self, count: i64) -> Self {
        self.chunk_count = count;
        self
    }

    /// Builder method to set total bytes.
    pub fn with_total_bytes(mut self, bytes: i64) -> Self {
        self.total_bytes = bytes;
        self
    }

    /// Builder method to set DQ summary.
    pub fn with_dq_summary(mut self, summary: DqSummary) -> Self {
        self.dq_summary = Some(summary);
        self
    }
}

/// Time range with min and max timestamps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeRange {
    /// Minimum (earliest) timestamp.
    pub min: DateTime<Utc>,

    /// Maximum (latest) timestamp.
    pub max: DateTime<Utc>,
}

impl TimeRange {
    /// Create a new TimeRange.
    pub fn new(min: DateTime<Utc>, max: DateTime<Utc>) -> Self {
        Self { min, max }
    }
}

/// Data quality summary for a table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DqSummary {
    /// Total number of DQ rules defined.
    pub total_rules: i32,

    /// Number of columns with at least one DQ rule.
    pub columns_with_rules: i32,
}

impl DqSummary {
    /// Create a new DqSummary.
    pub fn new(total_rules: i32, columns_with_rules: i32) -> Self {
        Self {
            total_rules,
            columns_with_rules,
        }
    }
}

// ============================================================================
// Dictionary Types (dp-010)
// ============================================================================

/// Entry in the data dictionary.
///
/// Returned by `DictionaryStore::search()`. Represents a single column
/// in either Bronze or Silver layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DictionaryEntry {
    /// Layer: "bronze" or "silver".
    pub layer: String,

    /// Entity name (stream_id for Bronze, table_name for Silver).
    pub entity: String,

    /// Column name.
    pub column_name: String,

    /// Data type.
    pub data_type: String,

    /// Unit of measurement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl DictionaryEntry {
    /// Create a new DictionaryEntry.
    pub fn new(
        layer: impl Into<String>,
        entity: impl Into<String>,
        column_name: impl Into<String>,
        data_type: impl Into<String>,
    ) -> Self {
        Self {
            layer: layer.into(),
            entity: entity.into(),
            column_name: column_name.into(),
            data_type: data_type.into(),
            unit: None,
            description: None,
        }
    }

    /// Builder method to set unit.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Builder method to set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Detailed column description from the data dictionary.
///
/// Returned by `DictionaryStore::describe_column()`. Includes lineage
/// information and DQ rules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnDescription {
    /// Layer: "bronze" or "silver".
    pub layer: String,

    /// Table or stream name.
    pub table_or_stream: String,

    /// Column name.
    pub column_name: String,

    /// Data type.
    pub data_type: String,

    /// Unit of measurement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the column can contain NULL values.
    pub nullable: bool,

    /// Source information for Silver columns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceInfo>,

    /// Data quality rules applied to this column.
    pub dq_rules: Vec<DqRuleInfo>,

    /// Validation range for numeric columns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_range: Option<ValidationRange>,
}

impl ColumnDescription {
    /// Create a new ColumnDescription.
    pub fn new(
        layer: impl Into<String>,
        table_or_stream: impl Into<String>,
        column_name: impl Into<String>,
        data_type: impl Into<String>,
    ) -> Self {
        Self {
            layer: layer.into(),
            table_or_stream: table_or_stream.into(),
            column_name: column_name.into(),
            data_type: data_type.into(),
            unit: None,
            description: None,
            nullable: true,
            source: None,
            dq_rules: Vec::new(),
            validation_range: None,
        }
    }

    /// Builder method to set unit.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Builder method to set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder method to set nullable.
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Builder method to set source.
    pub fn with_source(mut self, source: SourceInfo) -> Self {
        self.source = Some(source);
        self
    }

    /// Builder method to set DQ rules.
    pub fn with_dq_rules(mut self, rules: Vec<DqRuleInfo>) -> Self {
        self.dq_rules = rules;
        self
    }

    /// Builder method to set validation range.
    pub fn with_validation_range(mut self, range: ValidationRange) -> Self {
        self.validation_range = Some(range);
        self
    }
}

/// Source information for a Silver column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceInfo {
    /// Bronze stream ID.
    pub stream: String,

    /// JSON path within raw_payload.
    pub path: String,

    /// Transformation applied (e.g., "cast", "unit_conversion").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation: Option<String>,
}

impl SourceInfo {
    /// Create a new SourceInfo.
    pub fn new(stream: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            stream: stream.into(),
            path: path.into(),
            transformation: None,
        }
    }

    /// Builder method to set transformation.
    pub fn with_transformation(mut self, transformation: impl Into<String>) -> Self {
        self.transformation = Some(transformation.into());
        self
    }
}

/// Lineage trace from Silver column back to Bronze source.
///
/// Returned by `DictionaryStore::trace_lineage()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineageTrace {
    /// Silver table name.
    pub silver_table: String,

    /// Silver column name.
    pub silver_column: String,

    /// Silver column data type.
    pub silver_type: String,

    /// Silver column unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silver_unit: Option<String>,

    /// Source lineage chain.
    pub lineage: Vec<LineageSource>,

    /// DQ rules applied at Silver layer.
    pub dq_rules: Vec<DqRuleInfo>,
}

impl LineageTrace {
    /// Create a new LineageTrace.
    pub fn new(
        silver_table: impl Into<String>,
        silver_column: impl Into<String>,
        silver_type: impl Into<String>,
    ) -> Self {
        Self {
            silver_table: silver_table.into(),
            silver_column: silver_column.into(),
            silver_type: silver_type.into(),
            silver_unit: None,
            lineage: Vec::new(),
            dq_rules: Vec::new(),
        }
    }

    /// Builder method to set silver unit.
    pub fn with_silver_unit(mut self, unit: impl Into<String>) -> Self {
        self.silver_unit = Some(unit.into());
        self
    }

    /// Builder method to set lineage sources.
    pub fn with_lineage(mut self, lineage: Vec<LineageSource>) -> Self {
        self.lineage = lineage;
        self
    }

    /// Builder method to set DQ rules.
    pub fn with_dq_rules(mut self, rules: Vec<DqRuleInfo>) -> Self {
        self.dq_rules = rules;
        self
    }
}

/// A single source in the lineage chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineageSource {
    /// Bronze stream ID.
    pub source_stream: String,

    /// JSON path in raw_payload.
    pub source_path: String,

    /// Transformation applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation: Option<String>,

    /// Bronze field data type (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bronze_type: Option<String>,

    /// Bronze field unit (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bronze_unit: Option<String>,
}

impl LineageSource {
    /// Create a new LineageSource.
    pub fn new(source_stream: impl Into<String>, source_path: impl Into<String>) -> Self {
        Self {
            source_stream: source_stream.into(),
            source_path: source_path.into(),
            transformation: None,
            bronze_type: None,
            bronze_unit: None,
        }
    }

    /// Builder method to set transformation.
    pub fn with_transformation(mut self, transformation: impl Into<String>) -> Self {
        self.transformation = Some(transformation.into());
        self
    }

    /// Builder method to set bronze type.
    pub fn with_bronze_type(mut self, bronze_type: impl Into<String>) -> Self {
        self.bronze_type = Some(bronze_type.into());
        self
    }

    /// Builder method to set bronze unit.
    pub fn with_bronze_unit(mut self, bronze_unit: impl Into<String>) -> Self {
        self.bronze_unit = Some(bronze_unit.into());
        self
    }
}

/// Data quality rule information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DqRuleInfo {
    /// Silver table this rule applies to.
    pub silver_table: String,

    /// Column this rule applies to (None for table-level rules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silver_column: Option<String>,

    /// Rule name (e.g., "range_check", "not_null").
    pub rule_name: String,

    /// Rule parameters as JSON.
    pub rule_params: serde_json::Value,

    /// Action on rule violation: "flag", "reject", "warn".
    pub action: String,

    /// Scope: "column" or "table".
    pub scope: String,
}

impl DqRuleInfo {
    /// Create a new DqRuleInfo.
    pub fn new(
        silver_table: impl Into<String>,
        rule_name: impl Into<String>,
        action: impl Into<String>,
        scope: impl Into<String>,
    ) -> Self {
        Self {
            silver_table: silver_table.into(),
            silver_column: None,
            rule_name: rule_name.into(),
            rule_params: serde_json::Value::Object(serde_json::Map::new()),
            action: action.into(),
            scope: scope.into(),
        }
    }

    /// Builder method to set silver column.
    pub fn with_silver_column(mut self, column: impl Into<String>) -> Self {
        self.silver_column = Some(column.into());
        self
    }

    /// Builder method to set rule params.
    pub fn with_rule_params(mut self, params: serde_json::Value) -> Self {
        self.rule_params = params;
        self
    }
}

/// Validation range for numeric columns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationRange {
    /// Minimum allowed value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum allowed value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl ValidationRange {
    /// Create a new ValidationRange.
    pub fn new(min: Option<f64>, max: Option<f64>) -> Self {
        Self { min, max }
    }

    /// Create a range with both bounds.
    pub fn bounded(min: f64, max: f64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Create a range with only minimum.
    pub fn min_only(min: f64) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    /// Create a range with only maximum.
    pub fn max_only(max: f64) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }
}

// ============================================================================
// ETL Types (dp-010)
// ============================================================================

/// ETL status for a single stream.
///
/// Returned by `EtlRunStore::get_status()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EtlStreamStatus {
    /// Stream identifier.
    pub stream_id: String,

    /// Current status: "healthy", "warning", "error", "unknown".
    pub status: String,

    /// Information about the last ETL run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<EtlRunInfo>,

    /// Statistics for runs in the last 24 hours.
    pub runs_last_24h: RunStats,
}

impl EtlStreamStatus {
    /// Create a new EtlStreamStatus.
    pub fn new(stream_id: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            status: status.into(),
            last_run: None,
            runs_last_24h: RunStats::default(),
        }
    }

    /// Builder method to set last run info.
    pub fn with_last_run(mut self, run: EtlRunInfo) -> Self {
        self.last_run = Some(run);
        self
    }

    /// Builder method to set runs_last_24h.
    pub fn with_runs_last_24h(mut self, stats: RunStats) -> Self {
        self.runs_last_24h = stats;
        self
    }
}

/// Information about a single ETL run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EtlRunInfo {
    /// Unique run identifier.
    pub id: String,

    /// When the run started.
    pub started_at: DateTime<Utc>,

    /// When the run completed (None if still running).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,

    /// Number of rows processed.
    pub rows_processed: i64,

    /// Number of rows flagged by DQ rules.
    pub rows_flagged: i64,

    /// Number of rows rejected by DQ rules.
    pub rows_rejected: i64,

    /// High watermark before this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_before: Option<DateTime<Utc>>,

    /// High watermark after this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_after: Option<DateTime<Utc>>,

    /// Error message if the run failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl EtlRunInfo {
    /// Create a new EtlRunInfo.
    pub fn new(id: impl Into<String>, started_at: DateTime<Utc>) -> Self {
        Self {
            id: id.into(),
            started_at,
            completed_at: None,
            duration_ms: None,
            rows_processed: 0,
            rows_flagged: 0,
            rows_rejected: 0,
            watermark_before: None,
            watermark_after: None,
            error_message: None,
        }
    }

    /// Builder method to set completed_at and calculate duration.
    pub fn with_completed_at(mut self, completed_at: DateTime<Utc>) -> Self {
        self.duration_ms = Some((completed_at - self.started_at).num_milliseconds());
        self.completed_at = Some(completed_at);
        self
    }

    /// Builder method to set row counts.
    pub fn with_row_counts(mut self, processed: i64, flagged: i64, rejected: i64) -> Self {
        self.rows_processed = processed;
        self.rows_flagged = flagged;
        self.rows_rejected = rejected;
        self
    }

    /// Builder method to set watermarks.
    pub fn with_watermarks(
        mut self,
        before: Option<DateTime<Utc>>,
        after: Option<DateTime<Utc>>,
    ) -> Self {
        self.watermark_before = before;
        self.watermark_after = after;
        self
    }

    /// Builder method to set error message.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error_message = Some(error.into());
        self
    }
}

/// Statistics for ETL runs over a time period.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RunStats {
    /// Total number of runs.
    pub total: i32,

    /// Number of successful runs.
    pub succeeded: i32,

    /// Number of failed runs.
    pub failed: i32,
}

impl RunStats {
    /// Create new RunStats.
    pub fn new(total: i32, succeeded: i32, failed: i32) -> Self {
        Self {
            total,
            succeeded,
            failed,
        }
    }
}

/// ETL history for a stream.
///
/// Returned by `EtlRunStore::get_history()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EtlHistoryResult {
    /// Stream identifier.
    pub stream_id: String,

    /// List of ETL runs.
    pub runs: Vec<EtlRunDetail>,

    /// Summary of the history.
    pub summary: HistorySummary,
}

impl EtlHistoryResult {
    /// Create a new EtlHistoryResult.
    pub fn new(stream_id: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            runs: Vec::new(),
            summary: HistorySummary::default(),
        }
    }

    /// Builder method to set runs.
    pub fn with_runs(mut self, runs: Vec<EtlRunDetail>) -> Self {
        self.runs = runs;
        self
    }

    /// Builder method to set summary.
    pub fn with_summary(mut self, summary: HistorySummary) -> Self {
        self.summary = summary;
        self
    }
}

/// Detailed information about an ETL run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EtlRunDetail {
    /// Unique run identifier.
    pub id: String,

    /// When the run started.
    pub started_at: DateTime<Utc>,

    /// When the run completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,

    /// Run status: "success", "failed", "running".
    pub status: String,

    /// Number of rows processed.
    pub rows_processed: i64,

    /// Number of rows flagged.
    pub rows_flagged: i64,

    /// Number of rows rejected.
    pub rows_rejected: i64,

    /// High watermark before this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_before: Option<DateTime<Utc>>,

    /// High watermark after this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_after: Option<DateTime<Utc>>,

    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    /// Additional error context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_context: Option<serde_json::Value>,

    /// Run mode: "incremental", "full", "backfill".
    pub run_mode: String,
}

impl EtlRunDetail {
    /// Create a new EtlRunDetail.
    pub fn new(
        id: impl Into<String>,
        started_at: DateTime<Utc>,
        status: impl Into<String>,
        run_mode: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            started_at,
            completed_at: None,
            duration_ms: None,
            status: status.into(),
            rows_processed: 0,
            rows_flagged: 0,
            rows_rejected: 0,
            watermark_before: None,
            watermark_after: None,
            error_message: None,
            error_context: None,
            run_mode: run_mode.into(),
        }
    }

    /// Builder method to set completed_at and calculate duration.
    pub fn with_completed_at(mut self, completed_at: DateTime<Utc>) -> Self {
        self.duration_ms = Some((completed_at - self.started_at).num_milliseconds());
        self.completed_at = Some(completed_at);
        self
    }

    /// Builder method to set row counts.
    pub fn with_row_counts(mut self, processed: i64, flagged: i64, rejected: i64) -> Self {
        self.rows_processed = processed;
        self.rows_flagged = flagged;
        self.rows_rejected = rejected;
        self
    }

    /// Builder method to set watermarks.
    pub fn with_watermarks(
        mut self,
        before: Option<DateTime<Utc>>,
        after: Option<DateTime<Utc>>,
    ) -> Self {
        self.watermark_before = before;
        self.watermark_after = after;
        self
    }

    /// Builder method to set error.
    pub fn with_error(
        mut self,
        message: impl Into<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        self.error_message = Some(message.into());
        self.error_context = context;
        self
    }
}

/// Summary of ETL history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HistorySummary {
    /// Number of runs returned.
    pub total_returned: i32,

    /// Total runs available (may be more than returned due to limit).
    pub total_available: i32,

    /// Time range of returned runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
}

impl HistorySummary {
    /// Create a new HistorySummary.
    pub fn new(total_returned: i32, total_available: i32) -> Self {
        Self {
            total_returned,
            total_available,
            time_range: None,
        }
    }

    /// Builder method to set time range.
    pub fn with_time_range(mut self, min: DateTime<Utc>, max: DateTime<Utc>) -> Self {
        self.time_range = Some(TimeRange { min, max });
        self
    }
}

/// Data freshness report across layers.
///
/// Returned by `EtlRunStore::get_freshness()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessReport {
    /// When this report was generated.
    pub checked_at: DateTime<Utc>,

    /// Freshness entries for each stream/table.
    pub freshness: Vec<FreshnessEntry>,

    /// Summary of freshness across layers.
    pub summary: FreshnessSummary,
}

impl FreshnessReport {
    /// Create a new FreshnessReport.
    pub fn new(checked_at: DateTime<Utc>) -> Self {
        Self {
            checked_at,
            freshness: Vec::new(),
            summary: FreshnessSummary::default(),
        }
    }

    /// Builder method to set freshness entries.
    pub fn with_freshness(mut self, freshness: Vec<FreshnessEntry>) -> Self {
        self.freshness = freshness;
        self
    }

    /// Builder method to set summary.
    pub fn with_summary(mut self, summary: FreshnessSummary) -> Self {
        self.summary = summary;
        self
    }
}

/// Freshness information for a single stream or table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessEntry {
    /// Layer: "bronze" or "silver".
    pub layer: String,

    /// Identifier (stream_id for Bronze, table_name for Silver).
    pub identifier: String,

    /// Latest timestamp in the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<DateTime<Utc>>,

    /// Age in seconds since latest timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<i64>,

    /// Status: "fresh", "stale", "critical", "unknown".
    pub freshness_status: String,

    /// Row count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<i64>,

    /// When the last ETL run completed (Silver only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_etl_run: Option<DateTime<Utc>>,
}

impl FreshnessEntry {
    /// Create a new FreshnessEntry.
    pub fn new(
        layer: impl Into<String>,
        identifier: impl Into<String>,
        freshness_status: impl Into<String>,
    ) -> Self {
        Self {
            layer: layer.into(),
            identifier: identifier.into(),
            latest_timestamp: None,
            age_seconds: None,
            freshness_status: freshness_status.into(),
            row_count: None,
            last_etl_run: None,
        }
    }

    /// Builder method to set latest timestamp and calculate age.
    pub fn with_latest_timestamp(mut self, timestamp: DateTime<Utc>, now: DateTime<Utc>) -> Self {
        self.latest_timestamp = Some(timestamp);
        self.age_seconds = Some((now - timestamp).num_seconds());
        self
    }

    /// Builder method to set row count.
    pub fn with_row_count(mut self, count: i64) -> Self {
        self.row_count = Some(count);
        self
    }

    /// Builder method to set last ETL run.
    pub fn with_last_etl_run(mut self, run: DateTime<Utc>) -> Self {
        self.last_etl_run = Some(run);
        self
    }
}

/// Summary of freshness across layers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FreshnessSummary {
    /// Number of Bronze streams checked.
    pub bronze_streams: i32,

    /// Number of Silver tables checked.
    pub silver_tables: i32,

    /// Number of stale entries.
    pub stale_count: i32,

    /// Number of critical entries.
    pub critical_count: i32,
}

impl FreshnessSummary {
    /// Create a new FreshnessSummary.
    pub fn new(
        bronze_streams: i32,
        silver_tables: i32,
        stale_count: i32,
        critical_count: i32,
    ) -> Self {
        Self {
            bronze_streams,
            silver_tables,
            stale_count,
            critical_count,
        }
    }
}

// ============================================================================
// Silver/Dictionary/ETL Types Tests (dp-010)
// ============================================================================

#[cfg(test)]
mod silver_tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_silver_table_info_new() {
        let info = SilverTableInfo::new("air_quality_readings");
        assert_eq!(info.table_name, "air_quality_readings");
        assert!(info.description.is_none());
        assert!(!info.is_hypertable);
    }

    #[test]
    fn test_silver_table_info_builder() {
        let info = SilverTableInfo::new("air_quality_readings")
            .with_description("Air quality sensor readings")
            .with_grain("per_reading")
            .with_source_streams(vec!["air-quality".to_string()])
            .with_hypertable(true, Some("1 day".to_string()))
            .with_row_count(10000)
            .with_total_bytes(1024 * 1024);

        assert_eq!(info.table_name, "air_quality_readings");
        assert_eq!(
            info.description,
            Some("Air quality sensor readings".to_string())
        );
        assert_eq!(info.grain, Some("per_reading".to_string()));
        assert!(info.is_hypertable);
        assert_eq!(info.chunk_interval, Some("1 day".to_string()));
        assert_eq!(info.row_count, Some(10000));
    }

    #[test]
    fn test_silver_column_info_new() {
        let col = SilverColumnInfo::new("pm25", "DOUBLE PRECISION");
        assert_eq!(col.column_name, "pm25");
        assert_eq!(col.data_type, "DOUBLE PRECISION");
        assert!(col.nullable);
        assert!(!col.is_primary_key);
    }

    #[test]
    fn test_silver_column_info_builder() {
        let col = SilverColumnInfo::new("timestamp", "TIMESTAMPTZ")
            .with_nullable(false)
            .with_primary_key(true)
            .with_description("Reading timestamp");

        assert!(!col.nullable);
        assert!(col.is_primary_key);
        assert_eq!(col.description, Some("Reading timestamp".to_string()));
    }

    #[test]
    fn test_hypertable_info() {
        let info = HypertableInfo::new("timestamp", "1 day")
            .with_chunk_count(30)
            .with_total_bytes(100 * 1024 * 1024);

        assert_eq!(info.time_column, "timestamp");
        assert_eq!(info.chunk_interval, "1 day");
        assert_eq!(info.chunk_count, 30);
    }

    #[test]
    fn test_sample_filters() {
        let now = Utc::now();
        let filters = SampleFilters::new()
            .with_since(now)
            .with_order_by("timestamp DESC");

        assert_eq!(filters.since, Some(now));
        assert_eq!(filters.order_by, Some("timestamp DESC".to_string()));
    }

    #[test]
    fn test_silver_table_stats() {
        let now = Utc::now();
        let stats = SilverTableStats::new("air_quality_readings")
            .with_row_count(50000)
            .with_time_range(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(), now)
            .with_chunk_count(15)
            .with_dq_summary(DqSummary::new(5, 3));

        assert_eq!(stats.row_count, 50000);
        assert!(stats.time_range.is_some());
        assert_eq!(stats.dq_summary.as_ref().unwrap().total_rules, 5);
    }

    #[test]
    fn test_time_range() {
        let min = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let max = Utc.with_ymd_and_hms(2026, 1, 15, 23, 59, 59).unwrap();
        let range = TimeRange::new(min, max);

        assert_eq!(range.min, min);
        assert_eq!(range.max, max);
    }

    #[test]
    fn test_silver_table_description_serialization() {
        let desc = SilverTableDescription::new("test_table")
            .with_description("Test table")
            .with_columns(vec![SilverColumnInfo::new("id", "INTEGER")]);

        let json_str = serde_json::to_string(&desc).unwrap();
        assert!(json_str.contains("test_table"));
        assert!(json_str.contains("INTEGER"));
    }
}

#[cfg(test)]
mod dictionary_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_dictionary_entry_new() {
        let entry =
            DictionaryEntry::new("silver", "air_quality_readings", "pm25", "DOUBLE PRECISION");
        assert_eq!(entry.layer, "silver");
        assert_eq!(entry.entity, "air_quality_readings");
        assert_eq!(entry.column_name, "pm25");
    }

    #[test]
    fn test_dictionary_entry_builder() {
        let entry =
            DictionaryEntry::new("silver", "air_quality_readings", "pm25", "DOUBLE PRECISION")
                .with_unit("ug/m3")
                .with_description("PM2.5 particulate matter");

        assert_eq!(entry.unit, Some("ug/m3".to_string()));
        assert_eq!(
            entry.description,
            Some("PM2.5 particulate matter".to_string())
        );
    }

    #[test]
    fn test_column_description() {
        let col =
            ColumnDescription::new("silver", "air_quality_readings", "pm25", "DOUBLE PRECISION")
                .with_unit("ug/m3")
                .with_nullable(false)
                .with_source(SourceInfo::new("air-quality", "$.pm25"))
                .with_validation_range(ValidationRange::bounded(0.0, 500.0));

        assert!(!col.nullable);
        assert!(col.source.is_some());
        assert!(col.validation_range.is_some());
    }

    #[test]
    fn test_source_info() {
        let source = SourceInfo::new("air-quality", "$.raw_payload.pm25")
            .with_transformation("cast to double");

        assert_eq!(source.stream, "air-quality");
        assert_eq!(source.path, "$.raw_payload.pm25");
        assert_eq!(source.transformation, Some("cast to double".to_string()));
    }

    #[test]
    fn test_lineage_trace() {
        let trace = LineageTrace::new("air_quality_readings", "pm25", "DOUBLE PRECISION")
            .with_silver_unit("ug/m3")
            .with_lineage(vec![LineageSource::new("air-quality", "$.pm25")
                .with_bronze_type("number")
                .with_transformation("cast")])
            .with_dq_rules(vec![DqRuleInfo::new(
                "air_quality_readings",
                "range_check",
                "flag",
                "column",
            )
            .with_silver_column("pm25")
            .with_rule_params(json!({"min": 0, "max": 500}))]);

        assert_eq!(trace.lineage.len(), 1);
        assert_eq!(trace.dq_rules.len(), 1);
    }

    #[test]
    fn test_dq_rule_info() {
        let rule = DqRuleInfo::new("air_quality_readings", "not_null", "reject", "column")
            .with_silver_column("timestamp")
            .with_rule_params(json!({}));

        assert_eq!(rule.rule_name, "not_null");
        assert_eq!(rule.action, "reject");
        assert_eq!(rule.silver_column, Some("timestamp".to_string()));
    }

    #[test]
    fn test_validation_range() {
        let bounded = ValidationRange::bounded(0.0, 100.0);
        assert_eq!(bounded.min, Some(0.0));
        assert_eq!(bounded.max, Some(100.0));

        let min_only = ValidationRange::min_only(0.0);
        assert_eq!(min_only.min, Some(0.0));
        assert!(min_only.max.is_none());

        let max_only = ValidationRange::max_only(500.0);
        assert!(max_only.min.is_none());
        assert_eq!(max_only.max, Some(500.0));
    }

    #[test]
    fn test_lineage_source_builder() {
        let source = LineageSource::new("outdoor-weather", "$.main.temp")
            .with_transformation("kelvin_to_celsius")
            .with_bronze_type("number")
            .with_bronze_unit("kelvin");

        assert_eq!(source.transformation, Some("kelvin_to_celsius".to_string()));
        assert_eq!(source.bronze_unit, Some("kelvin".to_string()));
    }
}

#[cfg(test)]
mod etl_tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_etl_stream_status_new() {
        let status = EtlStreamStatus::new("air-quality", "healthy");
        assert_eq!(status.stream_id, "air-quality");
        assert_eq!(status.status, "healthy");
        assert!(status.last_run.is_none());
    }

    #[test]
    fn test_etl_stream_status_builder() {
        let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
        let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();

        let run = EtlRunInfo::new("run-001", started)
            .with_completed_at(completed)
            .with_row_counts(1000, 5, 2);

        let status = EtlStreamStatus::new("air-quality", "healthy")
            .with_last_run(run)
            .with_runs_last_24h(RunStats::new(24, 23, 1));

        assert!(status.last_run.is_some());
        assert_eq!(status.runs_last_24h.total, 24);
        assert_eq!(status.runs_last_24h.failed, 1);
    }

    #[test]
    fn test_etl_run_info() {
        let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
        let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();
        let watermark_before = Utc.with_ymd_and_hms(2026, 1, 17, 9, 0, 0).unwrap();
        let watermark_after = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();

        let run = EtlRunInfo::new("run-001", started)
            .with_completed_at(completed)
            .with_row_counts(1000, 5, 2)
            .with_watermarks(Some(watermark_before), Some(watermark_after));

        assert_eq!(run.duration_ms, Some(5000));
        assert_eq!(run.rows_processed, 1000);
        assert_eq!(run.rows_flagged, 5);
        assert_eq!(run.rows_rejected, 2);
    }

    #[test]
    fn test_etl_run_info_with_error() {
        let started = Utc::now();
        let run = EtlRunInfo::new("run-002", started).with_error("Connection timeout");

        assert_eq!(run.error_message, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_run_stats() {
        let stats = RunStats::new(100, 95, 5);
        assert_eq!(stats.total, 100);
        assert_eq!(stats.succeeded, 95);
        assert_eq!(stats.failed, 5);
    }

    #[test]
    fn test_etl_history_result() {
        let started = Utc::now();
        let history = EtlHistoryResult::new("air-quality")
            .with_runs(vec![EtlRunDetail::new(
                "run-001",
                started,
                "success",
                "incremental",
            )
            .with_row_counts(500, 2, 0)])
            .with_summary(HistorySummary::new(1, 100));

        assert_eq!(history.stream_id, "air-quality");
        assert_eq!(history.runs.len(), 1);
        assert_eq!(history.summary.total_available, 100);
    }

    #[test]
    fn test_etl_run_detail() {
        let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
        let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 10).unwrap();

        let detail = EtlRunDetail::new("run-003", started, "success", "incremental")
            .with_completed_at(completed)
            .with_row_counts(2000, 10, 3);

        assert_eq!(detail.status, "success");
        assert_eq!(detail.run_mode, "incremental");
        assert_eq!(detail.duration_ms, Some(10000));
    }

    #[test]
    fn test_etl_run_detail_with_error() {
        let started = Utc::now();
        let detail = EtlRunDetail::new("run-004", started, "failed", "incremental").with_error(
            "Database connection failed",
            Some(serde_json::json!({"code": "ECONNREFUSED"})),
        );

        assert_eq!(detail.status, "failed");
        assert!(detail.error_message.is_some());
        assert!(detail.error_context.is_some());
    }

    #[test]
    fn test_freshness_report() {
        let now = Utc::now();
        let report = FreshnessReport::new(now)
            .with_freshness(vec![
                FreshnessEntry::new("bronze", "air-quality", "fresh").with_row_count(50000),
                FreshnessEntry::new("silver", "air_quality_readings", "fresh")
                    .with_row_count(50000)
                    .with_last_etl_run(now),
            ])
            .with_summary(FreshnessSummary::new(1, 1, 0, 0));

        assert_eq!(report.freshness.len(), 2);
        assert_eq!(report.summary.bronze_streams, 1);
        assert_eq!(report.summary.silver_tables, 1);
    }

    #[test]
    fn test_freshness_entry() {
        let now = Utc::now();
        let latest = now - chrono::Duration::minutes(5);

        let entry = FreshnessEntry::new("bronze", "air-quality", "fresh")
            .with_latest_timestamp(latest, now)
            .with_row_count(10000);

        assert_eq!(entry.layer, "bronze");
        assert!(entry.age_seconds.is_some());
        assert!(entry.age_seconds.unwrap() >= 300); // at least 5 minutes
    }

    #[test]
    fn test_freshness_summary() {
        let summary = FreshnessSummary::new(3, 5, 1, 0);
        assert_eq!(summary.bronze_streams, 3);
        assert_eq!(summary.silver_tables, 5);
        assert_eq!(summary.stale_count, 1);
        assert_eq!(summary.critical_count, 0);
    }

    #[test]
    fn test_history_summary() {
        let min = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let max = Utc.with_ymd_and_hms(2026, 1, 17, 23, 59, 59).unwrap();

        let summary = HistorySummary::new(50, 1000).with_time_range(min, max);

        assert_eq!(summary.total_returned, 50);
        assert_eq!(summary.total_available, 1000);
        assert!(summary.time_range.is_some());
    }
}
