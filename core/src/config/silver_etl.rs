//! Silver ETL configuration types
//!
//! Defines the configuration structures for Bronze-to-Silver ETL transformations.
//! This module extends the existing config-driven patterns from Bronze ingestion
//! to Silver layer ETL, including field mappings, transforms, and DQ rules.
//!
//! # Example
//!
//! ```yaml
//! silver_etl:
//!   enabled: true
//!   target_table: silver.air_quality_observations
//!   timestamp:
//!     source_field: timestamp
//!     target_field: observation_time
//!     transform: microseconds_to_timestamp
//!   field_mappings:
//!     - source_path: raw_payload.pm02
//!       target_column: pm25
//!       type: double_precision
//!       dq_rules:
//!         - rule: range_check
//!           min: 0.0
//!           max: 1000.0
//!           action: flag
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

/// Silver ETL configuration errors
#[derive(Debug, Error, PartialEq)]
pub enum SilverConfigError {
    #[error("Invalid column type '{column_type}' for field '{field}'")]
    InvalidColumnType { field: String, column_type: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid DQ rule: {0}")]
    InvalidDqRule(String),

    #[error("Invalid target table: {0}")]
    InvalidTargetTable(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

// =============================================================================
// Main Configuration Types
// =============================================================================

/// Silver ETL configuration for a stream
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SilverEtlConfig {
    /// Whether this Silver ETL is enabled
    pub enabled: bool,

    /// Target table in Silver layer (e.g., "silver.air_quality_observations")
    pub target_table: String,

    /// Optional target schema name for versioning
    #[serde(default)]
    pub target_schema: Option<String>,

    /// Timestamp field mapping
    pub timestamp: TimestampMapping,

    /// Identity fields that pass through unchanged
    #[serde(default)]
    pub identity_fields: Vec<IdentityField>,

    /// Field mappings with transforms and DQ rules
    #[serde(default)]
    pub field_mappings: Vec<SilverFieldMapping>,

    /// Global DQ rules applied to all fields
    #[serde(default)]
    pub dq_rules: Vec<DqRule>,

    /// DQ output configuration
    #[serde(default)]
    pub dq_output: DqOutputConfig,

    /// Deduplication configuration
    #[serde(default)]
    pub deduplication: DeduplicationConfig,

    /// Incremental load configuration
    #[serde(default)]
    pub incremental: IncrementalConfig,
}

impl SilverEtlConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), SilverConfigError> {
        // Validate target_table format (should start with "silver.")
        if !self.target_table.starts_with("silver.") {
            return Err(SilverConfigError::InvalidTargetTable(
                "target_table must start with 'silver.'".to_string(),
            ));
        }

        // Validate field mappings
        for mapping in &self.field_mappings {
            mapping.validate()?;
        }

        // Validate DQ rules
        for rule in &self.dq_rules {
            rule.validate()?;
        }

        Ok(())
    }

    /// Get all target column names
    pub fn get_target_columns(&self) -> Vec<&str> {
        self.field_mappings
            .iter()
            .map(|m| m.target_column.as_str())
            .collect()
    }
}

// =============================================================================
// Timestamp Configuration
// =============================================================================

/// Timestamp mapping configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TimestampMapping {
    /// Source field name in Bronze data
    pub source_field: String,

    /// Target field name in Silver table
    pub target_field: String,

    /// Transform to apply to timestamp
    pub transform: TimestampTransform,
}

/// Timestamp transform types
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampTransform {
    /// Convert microseconds since epoch to timestamp
    MicrosecondsToTimestamp,
    /// Parse ISO 8601 formatted string
    Iso8601,
    /// Convert Unix seconds to timestamp
    UnixSeconds,
    /// Parse NWS duration format (ISO 8601 with duration suffix)
    NwsDuration,
}

// =============================================================================
// Identity Fields
// =============================================================================

/// Identity field passthrough configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct IdentityField {
    /// Source path in Bronze data (can be JSON path like "context.location.path")
    pub source: String,

    /// Target column name in Silver table
    pub target: String,
}

// =============================================================================
// Field Mapping Configuration
// =============================================================================

/// Field mapping for Silver ETL
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SilverFieldMapping {
    /// Source path in Bronze data (e.g., "raw_payload.pm02")
    pub source_path: String,

    /// Target column name in Silver table
    pub target_column: String,

    /// PostgreSQL column type
    #[serde(rename = "type")]
    pub column_type: String,

    /// Whether the column allows NULL values
    #[serde(default = "default_true")]
    pub nullable: bool,

    /// Optional transform to apply
    #[serde(default)]
    pub transform: Option<TransformConfig>,

    /// DQ rules specific to this field
    #[serde(default)]
    pub dq_rules: Vec<DqRule>,
}

impl SilverFieldMapping {
    /// Validate field mapping
    pub fn validate(&self) -> Result<(), SilverConfigError> {
        const VALID_TYPES: &[&str] = &[
            "double_precision",
            "real",
            "integer",
            "bigint",
            "smallint",
            "text",
            "varchar",
            "boolean",
            "timestamptz",
            "jsonb",
            "text[]",
        ];

        if !VALID_TYPES.contains(&self.column_type.as_str()) {
            return Err(SilverConfigError::InvalidColumnType {
                field: self.target_column.clone(),
                column_type: self.column_type.clone(),
            });
        }

        // Validate field-level DQ rules
        for rule in &self.dq_rules {
            rule.validate()?;
        }

        Ok(())
    }
}

fn default_true() -> bool {
    true
}

// =============================================================================
// Transform Configuration
// =============================================================================

/// Transform configuration for field mappings
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformConfig {
    /// Unit conversion (e.g., Kelvin to Celsius)
    UnitConversion {
        from: String,
        to: String,
        formula: ConversionFormula,
    },

    /// SQL expression transform
    Expression { expr: String },

    /// Lookup table for categorical mappings
    Lookup { table: HashMap<String, String> },

    /// JSON path extraction from nested payloads
    JsonExtract { path: String },

    /// Timestamp format conversion
    Timestamp { format: TimestampTransform },

    /// Computed field based on other columns
    Computed { depends_on: Vec<String>, expr: String },
}

/// Conversion formula types
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversionFormula {
    /// Linear transformation: (value * scale) + offset
    Linear { scale: f64, offset: f64 },

    /// Custom code expression (future enhancement)
    Custom { code: String },
}

impl ConversionFormula {
    /// Apply formula to a value
    pub fn apply(&self, value: f64) -> f64 {
        match self {
            ConversionFormula::Linear { scale, offset } => (value * scale) + offset,
            ConversionFormula::Custom { .. } => {
                // Future: evaluate custom expression
                value
            }
        }
    }
}

// =============================================================================
// Data Quality Rules
// =============================================================================

/// DQ rule configuration
///
/// Supports 11 rule types as defined in DQ-FRAMEWORK-DESIGN.md:
/// - Value-level: range_check, null_check, enum_check, pattern_check
/// - Temporal: freshness_check, monotonic_check, rate_of_change
/// - Cross-field: cross_field_check, conditional_check
/// - Batch-level: completeness_check, cardinality_check
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum DqRule {
    // =========================================================================
    // Value-Level Rules
    // =========================================================================
    /// Range check: validates numeric values fall within bounds
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    RangeCheck {
        #[serde(default)]
        field: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        action: DqAction,
        #[serde(default)]
        clamp_to_bounds: bool,
    },

    /// Null check: validates required fields are present
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    NullCheck {
        #[serde(default)]
        field: String,
        #[serde(default = "default_reject")]
        action: DqAction,
    },

    /// Enum check: validates value is in allowed set
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    EnumCheck {
        #[serde(default)]
        field: String,
        allowed_values: Vec<String>,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        action: DqAction,
    },

    /// Pattern check: validates string matches regex pattern
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    PatternCheck {
        #[serde(default)]
        field: String,
        pattern: String,
        #[serde(default)]
        action: DqAction,
    },

    // =========================================================================
    // Temporal Rules
    // =========================================================================
    /// Freshness check: validates timestamp is within expected window
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    FreshnessCheck {
        #[serde(default)]
        field: String,
        #[serde(default)]
        max_age: Option<String>,
        #[serde(default)]
        max_future: Option<String>,
        #[serde(default = "default_ingestion_time")]
        reference: String,
        #[serde(default)]
        action: DqAction,
    },

    /// Monotonic check: validates cumulative values increase/decrease monotonically
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    MonotonicCheck {
        #[serde(default)]
        field: String,
        direction: MonotonicDirection,
        partition_by: Vec<String>,
        #[serde(default)]
        allow_reset: bool,
        #[serde(default)]
        reset_threshold: Option<f64>,
        #[serde(default)]
        action: DqAction,
    },

    /// Rate of change: validates delta between consecutive values
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    RateOfChange {
        #[serde(default)]
        field: String,
        max_change_per_minute: f64,
        partition_by: Vec<String>,
        #[serde(default)]
        action: DqAction,
    },

    // =========================================================================
    // Cross-Field Rules
    // =========================================================================
    /// Cross-field check: validates relationships between multiple fields
    CrossFieldCheck {
        name: String,
        expression: String,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        action: DqAction,
    },

    /// Conditional check: validates a field based on another field's value
    ConditionalCheck {
        name: String,
        condition: String,
        then_rule: Box<DqRule>,
        #[serde(default)]
        action: DqAction,
    },

    // =========================================================================
    // Batch-Level Rules
    // =========================================================================
    /// Completeness check: validates batch-level completeness metrics
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    CompletenessCheck {
        #[serde(default = "default_batch")]
        level: String,
        #[serde(default)]
        field: String,
        min_completeness: f64,
        #[serde(default = "default_warn")]
        action: DqAction,
    },

    /// Cardinality check: validates expected distinct value count
    /// Note: `field` is optional when rule is embedded in a field_mapping (inherited from parent)
    CardinalityCheck {
        #[serde(default = "default_batch")]
        level: String,
        #[serde(default)]
        field: String,
        expected_range: (i32, i32),
        #[serde(default = "default_warn")]
        action: DqAction,
    },
}

impl DqRule {
    /// Validate DQ rule configuration
    pub fn validate(&self) -> Result<(), SilverConfigError> {
        match self {
            DqRule::RangeCheck { min, max, field, .. } => {
                if min.is_none() && max.is_none() {
                    return Err(SilverConfigError::InvalidDqRule(format!(
                        "range_check for '{}' must have at least min or max",
                        field
                    )));
                }
                if let (Some(min_val), Some(max_val)) = (min, max) {
                    if min_val >= max_val {
                        return Err(SilverConfigError::InvalidDqRule(format!(
                            "range_check for '{}': min ({}) must be less than max ({})",
                            field, min_val, max_val
                        )));
                    }
                }
            }
            DqRule::CompletenessCheck {
                min_completeness,
                field,
                ..
            } => {
                if *min_completeness < 0.0 || *min_completeness > 1.0 {
                    return Err(SilverConfigError::InvalidDqRule(format!(
                        "completeness_check for '{}': min_completeness must be between 0.0 and 1.0",
                        field
                    )));
                }
            }
            DqRule::CardinalityCheck {
                expected_range,
                field,
                ..
            } => {
                if expected_range.0 > expected_range.1 {
                    return Err(SilverConfigError::InvalidDqRule(format!(
                        "cardinality_check for '{}': expected_range[0] must be <= expected_range[1]",
                        field
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the field this rule applies to (if applicable)
    pub fn field(&self) -> Option<&str> {
        match self {
            DqRule::RangeCheck { field, .. } => Some(field),
            DqRule::NullCheck { field, .. } => Some(field),
            DqRule::EnumCheck { field, .. } => Some(field),
            DqRule::PatternCheck { field, .. } => Some(field),
            DqRule::FreshnessCheck { field, .. } => Some(field),
            DqRule::MonotonicCheck { field, .. } => Some(field),
            DqRule::RateOfChange { field, .. } => Some(field),
            DqRule::CompletenessCheck { field, .. } => Some(field),
            DqRule::CardinalityCheck { field, .. } => Some(field),
            DqRule::CrossFieldCheck { .. } => None,
            DqRule::ConditionalCheck { .. } => None,
        }
    }

    /// Get the action for this rule
    pub fn action(&self) -> &DqAction {
        match self {
            DqRule::RangeCheck { action, .. } => action,
            DqRule::NullCheck { action, .. } => action,
            DqRule::EnumCheck { action, .. } => action,
            DqRule::PatternCheck { action, .. } => action,
            DqRule::FreshnessCheck { action, .. } => action,
            DqRule::MonotonicCheck { action, .. } => action,
            DqRule::RateOfChange { action, .. } => action,
            DqRule::CrossFieldCheck { action, .. } => action,
            DqRule::ConditionalCheck { action, .. } => action,
            DqRule::CompletenessCheck { action, .. } => action,
            DqRule::CardinalityCheck { action, .. } => action,
        }
    }
}

/// Monotonic direction for monotonic_check rule
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MonotonicDirection {
    Increasing,
    Decreasing,
    StrictIncreasing,
}

/// DQ action types
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DqAction {
    /// Keep original value, add to dq_flags
    #[default]
    Flag,
    /// Set to NULL, add to dq_flags
    Reject,
    /// Clamp to bounds, add to dq_flags
    Clamp,
    /// Drop entire row
    Drop,
    /// Log warning (typically for batch-level rules)
    Warn,
}

fn default_reject() -> DqAction {
    DqAction::Reject
}

fn default_warn() -> DqAction {
    DqAction::Warn
}

fn default_ingestion_time() -> String {
    "ingestion_time".to_string()
}

fn default_batch() -> String {
    "batch".to_string()
}

// =============================================================================
// DQ Output Configuration
// =============================================================================

/// DQ output configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DqOutputConfig {
    /// Whether DQ output is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Target column for DQ flags
    #[serde(default = "default_dq_flags")]
    pub target_column: String,

    /// Include rule names in flags
    #[serde(default = "default_true")]
    pub include_rules: bool,

    /// Include original values in flags (privacy consideration)
    #[serde(default)]
    pub include_values: bool,
}

impl Default for DqOutputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target_column: "dq_flags".to_string(),
            include_rules: true,
            include_values: false,
        }
    }
}

fn default_dq_flags() -> String {
    "dq_flags".to_string()
}

// =============================================================================
// Deduplication Configuration
// =============================================================================

/// Deduplication configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct DeduplicationConfig {
    /// Whether deduplication is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Key columns for identifying duplicates
    #[serde(default)]
    pub key_columns: Vec<String>,

    /// Deduplication strategy
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
}

/// Deduplication strategy
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    /// Update existing row with new values
    #[default]
    Upsert,
    /// Skip new row if key exists
    Skip,
    /// Replace existing row entirely
    Replace,
}

// =============================================================================
// Incremental Load Configuration
// =============================================================================

/// Incremental load configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct IncrementalConfig {
    /// Whether incremental loading is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Column to use as watermark
    #[serde(default)]
    pub watermark_column: String,

    /// Lag interval for late arrivals (e.g., "5 minutes")
    #[serde(default)]
    pub lag_interval: String,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Test 1: Parse minimal valid config (RED -> GREEN)
    // ============================================================
    #[test]
    fn test_parse_minimal_silver_etl_config() {
        let yaml = r#"
enabled: true
target_table: silver.air_quality_observations
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
field_mappings: []
"#;

        let config: SilverEtlConfig =
            serde_yaml::from_str(yaml).expect("Should parse minimal config");

        assert!(config.enabled);
        assert_eq!(config.target_table, "silver.air_quality_observations");
        assert_eq!(config.timestamp.source_field, "timestamp");
        assert_eq!(config.timestamp.target_field, "observation_time");
        assert!(matches!(
            config.timestamp.transform,
            TimestampTransform::MicrosecondsToTimestamp
        ));
    }

    // ============================================================
    // Test 2: Parse complete config with all optional fields
    // ============================================================
    #[test]
    fn test_parse_complete_silver_etl_config() {
        let yaml = r#"
enabled: true
target_table: silver.air_quality_observations
target_schema: air_quality_observations_v1
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
identity_fields:
  - source: ndp_id
    target: ndp_id
  - source: context.location.path
    target: location_path
field_mappings:
  - source_path: raw_payload.pm02
    target_column: pm25
    type: double_precision
    nullable: false
dq_output:
  enabled: true
  target_column: dq_flags
  include_rules: true
  include_values: false
deduplication:
  enabled: true
  key_columns: [observation_time, ndp_id]
  strategy: upsert
incremental:
  enabled: true
  watermark_column: observation_time
  lag_interval: 5 minutes
"#;

        let config: SilverEtlConfig =
            serde_yaml::from_str(yaml).expect("Should parse complete config");

        assert!(config.enabled);
        assert_eq!(
            config.target_schema,
            Some("air_quality_observations_v1".to_string())
        );
        assert_eq!(config.identity_fields.len(), 2);
        assert_eq!(config.field_mappings.len(), 1);
        assert!(config.dq_output.enabled);
        assert!(config.deduplication.enabled);
        assert!(config.incremental.enabled);
        assert_eq!(config.incremental.lag_interval, "5 minutes");
    }

    // ============================================================
    // Test 3: Parse field mapping with unit conversion transform
    // ============================================================
    #[test]
    fn test_parse_field_mapping_with_transform() {
        let yaml = r#"
source_path: raw_payload.main.temp
target_column: temperature_c
type: double_precision
nullable: true
transform:
  type: unit_conversion
  from: kelvin
  to: celsius
  formula:
    type: linear
    scale: 1.0
    offset: -273.15
"#;

        let mapping: SilverFieldMapping =
            serde_yaml::from_str(yaml).expect("Should parse field mapping with transform");

        assert_eq!(mapping.source_path, "raw_payload.main.temp");
        assert_eq!(mapping.target_column, "temperature_c");
        assert_eq!(mapping.column_type, "double_precision");
        assert!(mapping.nullable);

        match mapping.transform {
            Some(TransformConfig::UnitConversion { from, to, formula }) => {
                assert_eq!(from, "kelvin");
                assert_eq!(to, "celsius");
                match formula {
                    ConversionFormula::Linear { scale, offset } => {
                        assert!((scale - 1.0).abs() < f64::EPSILON);
                        assert!((offset - (-273.15)).abs() < f64::EPSILON);
                    }
                    _ => panic!("Expected Linear formula"),
                }
            }
            _ => panic!("Expected UnitConversion transform"),
        }
    }

    // ============================================================
    // Test 4: Parse DQ rule - range_check with flag action
    // ============================================================
    #[test]
    fn test_parse_dq_rule_range_check() {
        let yaml = r#"
rule: range_check
field: pm25
min: 0.0
max: 1000.0
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse range_check rule");

        match rule {
            DqRule::RangeCheck {
                field,
                min,
                max,
                action,
                ..
            } => {
                assert_eq!(field, "pm25");
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(1000.0));
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected RangeCheck rule"),
        }
    }

    // ============================================================
    // Test 5: Parse DQ rule - range_check with clamp action
    // ============================================================
    #[test]
    fn test_parse_dq_rule_range_check_clamp() {
        let yaml = r#"
rule: range_check
field: humidity_pct
min: 0.0
max: 100.0
action: clamp
clamp_to_bounds: true
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse range_check with clamp");

        match rule {
            DqRule::RangeCheck {
                field,
                min,
                max,
                action,
                clamp_to_bounds,
            } => {
                assert_eq!(field, "humidity_pct");
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(100.0));
                assert!(matches!(action, DqAction::Clamp));
                assert!(clamp_to_bounds);
            }
            _ => panic!("Expected RangeCheck rule"),
        }
    }

    // ============================================================
    // Test 6: Parse DQ rule - null_check with reject action
    // ============================================================
    #[test]
    fn test_parse_dq_rule_null_check() {
        let yaml = r#"
rule: null_check
field: observation_time
action: reject
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse null_check rule");

        match rule {
            DqRule::NullCheck { field, action } => {
                assert_eq!(field, "observation_time");
                assert!(matches!(action, DqAction::Reject));
            }
            _ => panic!("Expected NullCheck rule"),
        }
    }

    // ============================================================
    // Test 7: Parse DQ rule - cross_field_check
    // ============================================================
    #[test]
    fn test_parse_dq_rule_cross_field_check() {
        let yaml = r#"
rule: cross_field_check
name: pm10_gte_pm25
expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
message: pm10_less_than_pm25
action: flag
"#;

        let rule: DqRule =
            serde_yaml::from_str(yaml).expect("Should parse cross_field_check rule");

        match rule {
            DqRule::CrossFieldCheck {
                name,
                expression,
                message,
                action,
            } => {
                assert_eq!(name, "pm10_gte_pm25");
                assert!(expression.contains("pm10 >= pm25"));
                assert_eq!(message, Some("pm10_less_than_pm25".to_string()));
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected CrossFieldCheck rule"),
        }
    }

    // ============================================================
    // Test 8: Validation rejects config without target_table
    // ============================================================
    #[test]
    fn test_validate_rejects_missing_target_table() {
        let yaml = r#"
enabled: true
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
field_mappings: []
"#;

        let result: Result<SilverEtlConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Should fail without target_table");
    }

    // ============================================================
    // Test 9: Validation rejects invalid column type
    // ============================================================
    #[test]
    fn test_validate_rejects_invalid_column_type() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.test".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![],
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.pm02".to_string(),
                target_column: "pm25".to_string(),
                column_type: "invalid_type".to_string(), // Invalid!
                nullable: true,
                transform: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid_type"));
    }

    // ============================================================
    // Test 10: Default values applied when not specified
    // ============================================================
    #[test]
    fn test_default_values_applied() {
        let yaml = r#"
enabled: true
target_table: silver.test
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
field_mappings: []
"#;

        let config: SilverEtlConfig =
            serde_yaml::from_str(yaml).expect("Should parse with defaults");

        // DQ output defaults
        assert!(!config.dq_output.enabled); // Default false
        assert_eq!(config.dq_output.target_column, "dq_flags");

        // Deduplication defaults
        assert!(!config.deduplication.enabled);
        assert!(matches!(
            config.deduplication.strategy,
            DeduplicationStrategy::Upsert
        ));

        // Incremental defaults
        assert!(!config.incremental.enabled);
    }

    // ============================================================
    // Test 11: Parse enum_check DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_enum_check() {
        let yaml = r#"
rule: enum_check
field: wind_direction
allowed_values: [N, NE, E, SE, S, SW, W, NW]
case_sensitive: false
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse enum_check rule");

        match rule {
            DqRule::EnumCheck {
                field,
                allowed_values,
                case_sensitive,
                action,
            } => {
                assert_eq!(field, "wind_direction");
                assert_eq!(allowed_values.len(), 8);
                assert!(!case_sensitive);
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected EnumCheck rule"),
        }
    }

    // ============================================================
    // Test 12: Parse pattern_check DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_pattern_check() {
        let yaml = r#"
rule: pattern_check
field: device_serial
pattern: "^[A-Z0-9]{8,12}$"
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse pattern_check rule");

        match rule {
            DqRule::PatternCheck {
                field,
                pattern,
                action,
            } => {
                assert_eq!(field, "device_serial");
                assert_eq!(pattern, "^[A-Z0-9]{8,12}$");
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected PatternCheck rule"),
        }
    }

    // ============================================================
    // Test 13: Parse freshness_check DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_freshness_check() {
        let yaml = r#"
rule: freshness_check
field: observation_time
max_age: "2 hours"
max_future: "10 minutes"
reference: ingestion_time
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse freshness_check rule");

        match rule {
            DqRule::FreshnessCheck {
                field,
                max_age,
                max_future,
                reference,
                action,
            } => {
                assert_eq!(field, "observation_time");
                assert_eq!(max_age, Some("2 hours".to_string()));
                assert_eq!(max_future, Some("10 minutes".to_string()));
                assert_eq!(reference, "ingestion_time");
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected FreshnessCheck rule"),
        }
    }

    // ============================================================
    // Test 14: Parse rate_of_change DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_rate_of_change() {
        let yaml = r#"
rule: rate_of_change
field: temperature_c
max_change_per_minute: 2.0
partition_by: [ndp_id]
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse rate_of_change rule");

        match rule {
            DqRule::RateOfChange {
                field,
                max_change_per_minute,
                partition_by,
                action,
            } => {
                assert_eq!(field, "temperature_c");
                assert!((max_change_per_minute - 2.0).abs() < f64::EPSILON);
                assert_eq!(partition_by, vec!["ndp_id".to_string()]);
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected RateOfChange rule"),
        }
    }

    // ============================================================
    // Test 15: Parse completeness_check batch-level DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_completeness_check() {
        let yaml = r#"
rule: completeness_check
level: batch
field: pm25
min_completeness: 0.95
action: warn
"#;

        let rule: DqRule =
            serde_yaml::from_str(yaml).expect("Should parse completeness_check rule");

        match rule {
            DqRule::CompletenessCheck {
                level,
                field,
                min_completeness,
                action,
            } => {
                assert_eq!(level, "batch");
                assert_eq!(field, "pm25");
                assert!((min_completeness - 0.95).abs() < f64::EPSILON);
                assert!(matches!(action, DqAction::Warn));
            }
            _ => panic!("Expected CompletenessCheck rule"),
        }
    }

    // ============================================================
    // Test 16: Parse cardinality_check batch-level DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_cardinality_check() {
        let yaml = r#"
rule: cardinality_check
level: batch
field: ndp_id
expected_range: [1, 10]
action: warn
"#;

        let rule: DqRule =
            serde_yaml::from_str(yaml).expect("Should parse cardinality_check rule");

        match rule {
            DqRule::CardinalityCheck {
                level,
                field,
                expected_range,
                action,
            } => {
                assert_eq!(level, "batch");
                assert_eq!(field, "ndp_id");
                assert_eq!(expected_range, (1, 10));
                assert!(matches!(action, DqAction::Warn));
            }
            _ => panic!("Expected CardinalityCheck rule"),
        }
    }

    // ============================================================
    // Test 17: Parse monotonic_check DQ rule
    // ============================================================
    #[test]
    fn test_parse_dq_rule_monotonic_check() {
        let yaml = r#"
rule: monotonic_check
field: cumulative_rainfall
direction: increasing
partition_by: [ndp_id]
allow_reset: true
reset_threshold: 1000.0
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml).expect("Should parse monotonic_check rule");

        match rule {
            DqRule::MonotonicCheck {
                field,
                direction,
                partition_by,
                allow_reset,
                reset_threshold,
                action,
            } => {
                assert_eq!(field, "cumulative_rainfall");
                assert!(matches!(direction, MonotonicDirection::Increasing));
                assert_eq!(partition_by, vec!["ndp_id".to_string()]);
                assert!(allow_reset);
                assert_eq!(reset_threshold, Some(1000.0));
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected MonotonicCheck rule"),
        }
    }

    // ============================================================
    // Test 18: Validate rejects invalid target_table prefix
    // ============================================================
    #[test]
    fn test_validate_rejects_invalid_target_table_prefix() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "bronze.test".to_string(), // Wrong prefix!
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![],
            field_mappings: vec![],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(SilverConfigError::InvalidTargetTable(msg)) => {
                assert!(msg.contains("silver."));
            }
            _ => panic!("Expected InvalidTargetTable error"),
        }
    }

    // ============================================================
    // Test 19: Validate DQ rule with invalid range
    // ============================================================
    #[test]
    fn test_validate_dq_rule_invalid_range() {
        let rule = DqRule::RangeCheck {
            field: "test".to_string(),
            min: Some(100.0),
            max: Some(50.0), // min > max is invalid
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };

        let result = rule.validate();
        assert!(result.is_err());
    }

    // ============================================================
    // Test 20: Parse all timestamp transform types
    // ============================================================
    #[test]
    fn test_parse_all_timestamp_transforms() {
        let transforms = vec![
            ("microseconds_to_timestamp", TimestampTransform::MicrosecondsToTimestamp),
            ("iso8601", TimestampTransform::Iso8601),
            ("unix_seconds", TimestampTransform::UnixSeconds),
            ("nws_duration", TimestampTransform::NwsDuration),
        ];

        for (yaml_val, expected) in transforms {
            let yaml = format!(
                r#"
source_field: ts
target_field: observation_time
transform: {}
"#,
                yaml_val
            );

            let mapping: TimestampMapping =
                serde_yaml::from_str(&yaml).expect("Should parse timestamp mapping");
            assert_eq!(mapping.transform, expected);
        }
    }

    // ============================================================
    // Test 21: Parse all transform types
    // ============================================================
    #[test]
    fn test_parse_all_transform_types() {
        // Expression transform
        let yaml = r#"
type: expression
expr: "(value - 32) * 5 / 9"
"#;
        let transform: TransformConfig = serde_yaml::from_str(yaml).unwrap();
        match transform {
            TransformConfig::Expression { expr } => {
                assert!(expr.contains("value"));
            }
            _ => panic!("Expected Expression transform"),
        }

        // Lookup transform
        let yaml = r#"
type: lookup
table:
  "1": "Good"
  "2": "Fair"
"#;
        let transform: TransformConfig = serde_yaml::from_str(yaml).unwrap();
        match transform {
            TransformConfig::Lookup { table } => {
                assert_eq!(table.get("1"), Some(&"Good".to_string()));
            }
            _ => panic!("Expected Lookup transform"),
        }

        // JsonExtract transform
        let yaml = r#"
type: json_extract
path: "$.list[0].main.aqi"
"#;
        let transform: TransformConfig = serde_yaml::from_str(yaml).unwrap();
        match transform {
            TransformConfig::JsonExtract { path } => {
                assert!(path.contains("aqi"));
            }
            _ => panic!("Expected JsonExtract transform"),
        }

        // Computed transform
        let yaml = r#"
type: computed
depends_on: [issue_time, valid_time]
expr: "EXTRACT(EPOCH FROM valid_time - issue_time) / 3600"
"#;
        let transform: TransformConfig = serde_yaml::from_str(yaml).unwrap();
        match transform {
            TransformConfig::Computed { depends_on, expr } => {
                assert_eq!(depends_on.len(), 2);
                assert!(expr.contains("EPOCH"));
            }
            _ => panic!("Expected Computed transform"),
        }
    }

    // ============================================================
    // Test 22: ConversionFormula apply method
    // ============================================================
    #[test]
    fn test_conversion_formula_apply() {
        // Linear conversion: Kelvin to Celsius
        let formula = ConversionFormula::Linear {
            scale: 1.0,
            offset: -273.15,
        };
        let result = formula.apply(300.0);
        assert!((result - 26.85).abs() < 0.01);

        // Custom formula (currently returns value unchanged)
        let custom = ConversionFormula::Custom {
            code: "value * 2".to_string(),
        };
        let result = custom.apply(100.0);
        assert!((result - 100.0).abs() < f64::EPSILON);
    }

    // ============================================================
    // Test 23: DqRule field() method
    // ============================================================
    #[test]
    fn test_dq_rule_field_method() {
        let rule = DqRule::RangeCheck {
            field: "pm25".to_string(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };
        assert_eq!(rule.field(), Some("pm25"));

        let rule = DqRule::CrossFieldCheck {
            name: "test".to_string(),
            expression: "a > b".to_string(),
            message: None,
            action: DqAction::Flag,
        };
        assert_eq!(rule.field(), None);
    }

    // ============================================================
    // Test 24: DqRule action() method
    // ============================================================
    #[test]
    fn test_dq_rule_action_method() {
        let rule = DqRule::RangeCheck {
            field: "test".to_string(),
            min: Some(0.0),
            max: Some(100.0),
            action: DqAction::Clamp,
            clamp_to_bounds: true,
        };
        assert!(matches!(rule.action(), DqAction::Clamp));
    }

    // ============================================================
    // Test 25: Identity field parsing
    // ============================================================
    #[test]
    fn test_parse_identity_fields() {
        let yaml = r#"
- source: ndp_id
  target: ndp_id
- source: context.location.path
  target: location_path
- source: raw_payload.serialno
  target: device_serial
"#;

        let fields: Vec<IdentityField> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].source, "ndp_id");
        assert_eq!(fields[1].source, "context.location.path");
        assert_eq!(fields[2].target, "device_serial");
    }

    // ============================================================
    // Test 26: Serialization round-trip
    // ============================================================
    #[test]
    fn test_serialization_round_trip() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.test".to_string(),
            target_schema: Some("test_v1".to_string()),
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![IdentityField {
                source: "ndp_id".to_string(),
                target: "ndp_id".to_string(),
            }],
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.pm02".to_string(),
                target_column: "pm25".to_string(),
                column_type: "double_precision".to_string(),
                nullable: false,
                transform: Some(TransformConfig::UnitConversion {
                    from: "raw".to_string(),
                    to: "ug_m3".to_string(),
                    formula: ConversionFormula::Linear {
                        scale: 1.0,
                        offset: 0.0,
                    },
                }),
                dq_rules: vec![DqRule::RangeCheck {
                    field: "pm25".to_string(),
                    min: Some(0.0),
                    max: Some(1000.0),
                    action: DqAction::Flag,
                    clamp_to_bounds: false,
                }],
            }],
            dq_rules: vec![],
            dq_output: DqOutputConfig {
                enabled: true,
                target_column: "dq_flags".to_string(),
                include_rules: true,
                include_values: false,
            },
            deduplication: DeduplicationConfig {
                enabled: true,
                key_columns: vec![
                    "observation_time".to_string(),
                    "ndp_id".to_string(),
                ],
                strategy: DeduplicationStrategy::Upsert,
            },
            incremental: IncrementalConfig {
                enabled: true,
                watermark_column: "observation_time".to_string(),
                lag_interval: "5 minutes".to_string(),
            },
        };

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&config).expect("Serialization should succeed");

        // Deserialize back
        let restored: SilverEtlConfig =
            serde_yaml::from_str(&yaml).expect("Deserialization should succeed");

        assert_eq!(config, restored);
    }
}
