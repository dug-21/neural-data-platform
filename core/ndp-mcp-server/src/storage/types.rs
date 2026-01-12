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
