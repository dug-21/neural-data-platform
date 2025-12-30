use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during stream configuration validation
#[derive(Debug, Error, PartialEq)]
pub enum StreamConfigError {
    #[error("Invalid stream ID: {0}")]
    InvalidStreamId(String),

    #[error("Invalid field name: {0}")]
    InvalidFieldName(String),

    #[error("Stream must have at least one field")]
    NoFields,

    #[error("Stream must have at least one source")]
    NoSources,

    #[error("Invalid field type for {field}: {reason}")]
    InvalidFieldType { field: String, reason: String },

    #[error("Invalid range for field {field}: {reason}")]
    InvalidRange { field: String, reason: String },
}

/// Field data types supported in stream schemas
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Float,
    Int,
    String,
    Bool,
    Json,
}

/// Field definition in a stream schema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaField {
    /// Field name (snake_case)
    pub name: String,

    /// Field data type
    #[serde(rename = "type")]
    pub field_type: FieldType,

    /// Physical unit (informational)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Expected range [min, max] (informational, not enforced)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Vec<f64>>,

    /// Decimal places for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_precision: Option<u32>,

    /// Whether field can be null
    #[serde(default = "default_nullable")]
    pub nullable: bool,

    /// Default value if not provided (JSON value)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

fn default_nullable() -> bool {
    true
}

impl SchemaField {
    /// Create a new required field
    pub fn new(name: String, field_type: FieldType) -> Self {
        Self {
            name,
            field_type,
            unit: None,
            description: None,
            range: None,
            display_precision: None,
            nullable: true,
            default: None,
        }
    }

    /// Set field as non-nullable
    pub fn required(mut self) -> Self {
        self.nullable = false;
        self
    }

    /// Set field unit
    pub fn with_unit(mut self, unit: String) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Set field description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set expected range
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.range = Some(vec![min, max]);
        self
    }

    /// Set display precision
    pub fn with_precision(mut self, precision: u32) -> Self {
        self.display_precision = Some(precision);
        self
    }

    /// Validate field configuration
    pub fn validate(&self) -> Result<(), StreamConfigError> {
        // Validate field name format
        if !is_valid_field_name(&self.name) {
            return Err(StreamConfigError::InvalidFieldName(self.name.clone()));
        }

        // Type-specific validation
        match self.field_type {
            FieldType::String | FieldType::Bool | FieldType::Json => {
                if self.range.is_some() {
                    return Err(StreamConfigError::InvalidFieldType {
                        field: self.name.clone(),
                        reason: "String, Bool, and Json types cannot have range".to_string(),
                    });
                }
                if self.display_precision.is_some() {
                    return Err(StreamConfigError::InvalidFieldType {
                        field: self.name.clone(),
                        reason: "String, Bool, and Json types cannot have display_precision"
                            .to_string(),
                    });
                }
            }
            FieldType::Int => {
                if self.display_precision.is_some() {
                    return Err(StreamConfigError::InvalidFieldType {
                        field: self.name.clone(),
                        reason: "Int type cannot have display_precision".to_string(),
                    });
                }
            }
            FieldType::Float => {
                // Float can have both range and precision
            }
        }

        // Validate range if present
        if let Some(ref range) = self.range {
            if range.len() != 2 {
                return Err(StreamConfigError::InvalidRange {
                    field: self.name.clone(),
                    reason: "Range must have exactly 2 elements [min, max]".to_string(),
                });
            }
            if range[0] >= range[1] {
                return Err(StreamConfigError::InvalidRange {
                    field: self.name.clone(),
                    reason: format!("Min ({}) must be less than max ({})", range[0], range[1]),
                });
            }
        }

        Ok(())
    }
}

/// Source type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
}

/// Source configuration within a stream
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    /// Source type
    #[serde(rename = "type")]
    pub source_type: SourceType,

    /// Whether source is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Source-specific parameters
    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}

fn default_enabled() -> bool {
    true
}

/// Stream configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamConfig {
    /// Unique stream identifier (kebab-case)
    pub stream_id: String,

    /// Human-readable description
    pub description: String,

    /// Schema version (semver)
    #[serde(default = "default_version")]
    pub version: String,

    /// Whether stream is active
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Days to retain data (0 = infinite)
    #[serde(default)]
    pub retention_days: u32,

    /// Days before compression
    #[serde(default)]
    pub compression_after_days: u32,

    /// Partitioning strategy
    #[serde(default = "default_partitioning")]
    pub partitioning_strategy: String,

    /// Field definitions
    pub fields: Vec<SchemaField>,

    /// Source configurations
    pub sources: Vec<SourceConfig>,

    /// Storage configuration overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfig>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_partitioning() -> String {
    "daily".to_string()
}

/// Storage configuration for a stream
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    /// Batch size for writes
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Batch timeout in seconds
    #[serde(default = "default_batch_timeout")]
    pub batch_timeout_secs: u64,

    /// Buffer capacity for channel
    #[serde(default = "default_buffer_capacity")]
    pub buffer_capacity: usize,
}

fn default_batch_size() -> usize {
    100
}

fn default_batch_timeout() -> u64 {
    5
}

fn default_buffer_capacity() -> usize {
    1000
}

impl StreamConfig {
    /// Validate stream configuration
    pub fn validate(&self) -> Result<(), StreamConfigError> {
        // Validate stream ID format
        if !is_valid_stream_id(&self.stream_id) {
            return Err(StreamConfigError::InvalidStreamId(self.stream_id.clone()));
        }

        // Must have at least one field
        if self.fields.is_empty() {
            return Err(StreamConfigError::NoFields);
        }

        // Must have at least one source
        if self.sources.is_empty() {
            return Err(StreamConfigError::NoSources);
        }

        // Validate each field
        for field in &self.fields {
            field.validate()?;
        }

        Ok(())
    }

    /// Get field by name
    pub fn get_field(&self, name: &str) -> Option<&SchemaField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Check if field exists
    pub fn has_field(&self, name: &str) -> bool {
        self.get_field(name).is_some()
    }
}

/// Validate stream ID format (kebab-case, 3-64 chars)
fn is_valid_stream_id(id: &str) -> bool {
    let len = id.len();
    if len < 3 || len > 64 {
        return false;
    }

    // Must start with lowercase letter
    if !id.chars().next().map_or(false, |c| c.is_ascii_lowercase()) {
        return false;
    }

    // Only lowercase letters, digits, and hyphens
    id.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Validate field name format (snake_case, 1-64 chars)
fn is_valid_field_name(name: &str) -> bool {
    let len = name.len();
    if len < 1 || len > 64 {
        return false;
    }

    // Must start with lowercase letter
    if !name
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_lowercase())
    {
        return false;
    }

    // Only lowercase letters, digits, and underscores
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== LONDON SCHOOL TDD: FIELD VALIDATION TESTS ==========

    #[test]
    fn test_schema_field_new_creates_nullable_field() {
        let field = SchemaField::new("test_field".to_string(), FieldType::Float);

        assert_eq!(field.name, "test_field");
        assert_eq!(field.field_type, FieldType::Float);
        assert!(field.nullable);
        assert!(field.unit.is_none());
        assert!(field.description.is_none());
    }

    #[test]
    fn test_schema_field_required_makes_non_nullable() {
        let field = SchemaField::new("test_field".to_string(), FieldType::Float).required();

        assert!(!field.nullable);
    }

    #[test]
    fn test_schema_field_builder_pattern() {
        let field = SchemaField::new("pm25".to_string(), FieldType::Float)
            .required()
            .with_unit("µg/m³".to_string())
            .with_description("Particulate Matter 2.5".to_string())
            .with_range(0.0, 500.0)
            .with_precision(1);

        assert_eq!(field.name, "pm25");
        assert!(!field.nullable);
        assert_eq!(field.unit, Some("µg/m³".to_string()));
        assert_eq!(field.range, Some(vec![0.0, 500.0]));
        assert_eq!(field.display_precision, Some(1));
    }

    #[test]
    fn test_field_name_validation_valid_names() {
        let valid_names = vec!["pm25", "temperature", "co2", "event_type", "sensor_id"];

        for name in valid_names {
            assert!(is_valid_field_name(name), "Should accept: {}", name);
        }
    }

    #[test]
    fn test_field_name_validation_invalid_names() {
        let invalid_names = vec![
            "PM25",           // uppercase
            "temp-c",         // hyphen
            "2temp",          // starts with digit
            "_private",       // starts with underscore
            "",               // empty
            "field_name_that_is_way_too_long_and_exceeds_the_maximum_allowed_length_for_field_names", // too long
        ];

        for name in &invalid_names {
            assert!(!is_valid_field_name(name), "Should reject: {}", name);
        }
    }

    #[test]
    fn test_field_validate_string_type_cannot_have_range() {
        let field =
            SchemaField::new("event_type".to_string(), FieldType::String).with_range(0.0, 100.0);

        let result = field.validate();
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(StreamConfigError::InvalidFieldType { .. })
        ));
    }

    #[test]
    fn test_field_validate_int_type_cannot_have_precision() {
        let field = SchemaField::new("count".to_string(), FieldType::Int).with_precision(2);

        let result = field.validate();
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(StreamConfigError::InvalidFieldType { .. })
        ));
    }

    #[test]
    fn test_field_validate_float_can_have_range_and_precision() {
        let field = SchemaField::new("temperature".to_string(), FieldType::Float)
            .with_range(-40.0, 60.0)
            .with_precision(1);

        assert!(field.validate().is_ok());
    }

    #[test]
    fn test_field_validate_range_must_have_two_elements() {
        let mut field = SchemaField::new("test".to_string(), FieldType::Float);
        field.range = Some(vec![0.0, 50.0, 100.0]); // Three elements

        let result = field.validate();
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(StreamConfigError::InvalidRange { .. })
        ));
    }

    #[test]
    fn test_field_validate_range_min_must_be_less_than_max() {
        let field = SchemaField::new("test".to_string(), FieldType::Float).with_range(100.0, 50.0); // min > max

        let result = field.validate();
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(StreamConfigError::InvalidRange { .. })
        ));
    }

    #[test]
    fn test_field_validate_invalid_field_name() {
        let field = SchemaField::new("Invalid-Name".to_string(), FieldType::Float);

        let result = field.validate();
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(StreamConfigError::InvalidFieldName(_))
        ));
    }

    // ========== STREAM CONFIG VALIDATION TESTS ==========

    #[test]
    fn test_stream_id_validation_valid_ids() {
        let valid_ids = vec!["air-quality", "home-events", "weather", "power-usage"];

        for id in valid_ids {
            assert!(is_valid_stream_id(id), "Should accept: {}", id);
        }
    }

    #[test]
    fn test_stream_id_validation_invalid_ids() {
        let invalid_ids = vec![
            "AirQuality",  // uppercase
            "air_quality", // underscore
            "ab",          // too short
            "2stream",     // starts with digit
            "stream-id-that-is-way-too-long-and-exceeds-the-maximum-allowed-length",
        ];

        for id in invalid_ids {
            assert!(!is_valid_stream_id(id), "Should reject: {}", id);
        }
    }

    #[test]
    fn test_stream_config_validate_invalid_stream_id() {
        let config = create_test_stream_config();
        let mut invalid_config = config.clone();
        invalid_config.stream_id = "Invalid_ID".to_string();

        let result = invalid_config.validate();
        assert!(result.is_err());
        assert!(matches!(result, Err(StreamConfigError::InvalidStreamId(_))));
    }

    #[test]
    fn test_stream_config_validate_no_fields() {
        let mut config = create_test_stream_config();
        config.fields.clear();

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StreamConfigError::NoFields);
    }

    #[test]
    fn test_stream_config_validate_no_sources() {
        let mut config = create_test_stream_config();
        config.sources.clear();

        let result = config.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StreamConfigError::NoSources);
    }

    #[test]
    fn test_stream_config_validate_valid_config() {
        let config = create_test_stream_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_stream_config_get_field_existing() {
        let config = create_test_stream_config();
        let field = config.get_field("pm25");

        assert!(field.is_some());
        assert_eq!(field.unwrap().name, "pm25");
    }

    #[test]
    fn test_stream_config_get_field_missing() {
        let config = create_test_stream_config();
        let field = config.get_field("nonexistent");

        assert!(field.is_none());
    }

    #[test]
    fn test_stream_config_has_field() {
        let config = create_test_stream_config();

        assert!(config.has_field("pm25"));
        assert!(config.has_field("temperature"));
        assert!(!config.has_field("nonexistent"));
    }

    // ========== SERIALIZATION TESTS ==========

    #[test]
    fn test_schema_field_serialization() {
        let field = SchemaField::new("pm25".to_string(), FieldType::Float)
            .required()
            .with_unit("µg/m³".to_string())
            .with_range(0.0, 500.0)
            .with_precision(1);

        let json = serde_json::to_string(&field).expect("Serialization should succeed");
        let deserialized: SchemaField =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(deserialized, field);
    }

    #[test]
    fn test_stream_config_serialization() {
        let config = create_test_stream_config();

        let json = serde_json::to_string(&config).expect("Serialization should succeed");
        let deserialized: StreamConfig =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(deserialized.stream_id, config.stream_id);
        assert_eq!(deserialized.fields.len(), config.fields.len());
        assert_eq!(deserialized.sources.len(), config.sources.len());
    }

    #[test]
    fn test_field_type_serialization() {
        let types = vec![
            (FieldType::Float, "\"float\""),
            (FieldType::Int, "\"int\""),
            (FieldType::String, "\"string\""),
            (FieldType::Bool, "\"bool\""),
            (FieldType::Json, "\"json\""),
        ];

        for (field_type, expected_json) in types {
            let json = serde_json::to_string(&field_type).unwrap();
            assert_eq!(json, expected_json);

            let deserialized: FieldType = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, field_type);
        }
    }

    // ========== HELPER FUNCTIONS ==========

    fn create_test_stream_config() -> StreamConfig {
        StreamConfig {
            stream_id: "test-stream".to_string(),
            description: "Test stream".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 365,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![
                SchemaField::new("pm25".to_string(), FieldType::Float)
                    .required()
                    .with_unit("µg/m³".to_string())
                    .with_range(0.0, 500.0)
                    .with_precision(1),
                SchemaField::new("temperature".to_string(), FieldType::Float)
                    .with_unit("celsius".to_string())
                    .with_range(-40.0, 60.0)
                    .with_precision(1),
            ],
            sources: vec![SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                params: HashMap::new(),
            }],
            storage: None,
        }
    }
}
