//! Validation trait and error types for NDP configuration.
//!
//! This module provides the `NdpValidate` trait that all configuration types
//! implement for semantic validation beyond JSON Schema structural validation.
//!
//! # Validation Layers
//!
//! - **Syntax**: JSON parsing (malformed JSON)
//! - **Schema**: JSON Schema validation (wrong types, missing fields)
//! - **Semantic**: Business rule validation (cross-references, constraints)
//!
//! # Example
//!
//! ```rust
//! use ndp_types::{NdpValidate, ValidationError, ErrorCode};
//!
//! struct MyConfig {
//!     min: f64,
//!     max: f64,
//! }
//!
//! impl NdpValidate for MyConfig {
//!     fn validate(&self) -> Vec<ValidationError> {
//!         let mut errors = Vec::new();
//!         if self.min > self.max {
//!             errors.push(ValidationError::semantic(
//!                 ErrorCode::InvalidRange,
//!                 "$.range",
//!                 "min must be less than max"
//!             ));
//!         }
//!         errors
//!     }
//! }
//!
//! let config = MyConfig { min: 100.0, max: 50.0 };
//! let errors = config.validate();
//! assert_eq!(errors.len(), 1);
//! ```

use crate::error::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Validation layer indicating where the error was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLayer {
    /// JSON syntax errors (malformed JSON)
    Syntax,
    /// JSON Schema validation errors (wrong types, missing fields)
    Schema,
    /// Semantic validation errors (invalid references, constraint violations)
    Semantic,
}

/// Error severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Validation fails; config cannot be used
    Error,
    /// Validation passes with warnings; config can be used
    Warning,
}

/// Unified validation error structure.
///
/// Provides machine-readable code, human-readable message,
/// and JSONPath location for precise error reporting.
///
/// # Example
///
/// ```rust
/// use ndp_types::{ValidationError, ValidationLayer, Severity, ErrorCode};
///
/// let error = ValidationError {
///     layer: ValidationLayer::Semantic,
///     code: ErrorCode::InvalidRange,
///     path: "$.silver_etl.dq_rules[0]".to_string(),
///     message: "min (100) must be less than max (50)".to_string(),
///     severity: Severity::Error,
///     suggestion: Some("Swap min and max values".to_string()),
/// };
///
/// assert_eq!(error.path, "$.silver_etl.dq_rules[0]");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Validation layer where error was detected
    pub layer: ValidationLayer,
    /// Machine-readable error code for programmatic handling
    pub code: ErrorCode,
    /// JSONPath to the error location (e.g., "$.sources[0].type")
    pub path: String,
    /// Human-readable error message
    pub message: String,
    /// Error severity
    pub severity: Severity,
    /// Optional suggestion for fixing the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl ValidationError {
    /// Create a semantic error with standard fields.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ndp_types::{ValidationError, ErrorCode};
    ///
    /// let error = ValidationError::semantic(
    ///     ErrorCode::InvalidDqRule,
    ///     "$.dq_rules[0]",
    ///     "range_check min must be less than max"
    /// );
    ///
    /// assert_eq!(error.code, ErrorCode::InvalidDqRule);
    /// ```
    pub fn semantic(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Semantic,
            code,
            path: path.into(),
            message: message.into(),
            severity: Severity::Error,
            suggestion: None,
        }
    }

    /// Create a schema validation error.
    pub fn schema(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Schema,
            code,
            path: path.into(),
            message: message.into(),
            severity: Severity::Error,
            suggestion: None,
        }
    }

    /// Create a warning (non-blocking).
    pub fn warning(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Semantic,
            code,
            path: path.into(),
            message: message.into(),
            severity: Severity::Warning,
            suggestion: None,
        }
    }

    /// Add a suggestion to this error.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Context for cross-reference validation.
///
/// Provides information about related configuration elements
/// that semantic validation may need to reference.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Field names defined in schema_fields
    pub field_names: HashSet<String>,
    /// Column names defined in Silver field_mappings
    pub silver_columns: HashSet<String>,
    /// Stream ID being validated
    pub stream_id: Option<String>,
    /// Database connection URL for table/column checks
    pub database_url: Option<String>,
}

impl ValidationContext {
    /// Create a new empty validation context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with field names.
    pub fn with_fields(field_names: impl IntoIterator<Item = String>) -> Self {
        Self {
            field_names: field_names.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Add a field name to the context.
    pub fn add_field(&mut self, name: String) {
        self.field_names.insert(name);
    }

    /// Check if a field name exists.
    pub fn has_field(&self, name: &str) -> bool {
        self.field_names.contains(name)
    }
}

/// Trait for NDP configuration validation.
///
/// Implementors provide semantic validation logic that goes beyond
/// JSON Schema structural validation. This includes:
/// - Cross-reference checks (field names exist)
/// - Constraint validation (min < max)
/// - Domain-specific rules (valid regex patterns)
///
/// # Example
///
/// ```rust
/// use ndp_types::{NdpValidate, ValidationError, ValidationContext, ErrorCode};
///
/// struct RangeConfig {
///     min: f64,
///     max: f64,
/// }
///
/// impl NdpValidate for RangeConfig {
///     fn validate(&self) -> Vec<ValidationError> {
///         let mut errors = Vec::new();
///         if self.min >= self.max {
///             errors.push(ValidationError::semantic(
///                 ErrorCode::InvalidRange,
///                 "$",
///                 format!("min ({}) must be less than max ({})", self.min, self.max)
///             ));
///         }
///         errors
///     }
/// }
///
/// let config = RangeConfig { min: 10.0, max: 5.0 };
/// assert!(!config.is_valid());
/// ```
pub trait NdpValidate {
    /// Validate this configuration, returning all errors.
    ///
    /// # Returns
    /// Vector of validation errors (empty if valid)
    fn validate(&self) -> Vec<ValidationError>;

    /// Validate with additional context.
    ///
    /// Default implementation ignores context and calls validate().
    /// Override for validations that need cross-reference information.
    fn validate_with_context(&self, _ctx: &ValidationContext) -> Vec<ValidationError> {
        self.validate()
    }

    /// Check if configuration is valid (no errors).
    fn is_valid(&self) -> bool {
        self.validate()
            .iter()
            .all(|e| e.severity != Severity::Error)
    }
}

// =============================================================================
// LONDON SCHOOL TDD TESTS - TC-200 Series: NdpValidate Trait
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    // =========================================================================
    // TC-201: ValidationError construction
    // Description: Verify ValidationError can be constructed with all fields
    // Priority: High
    // =========================================================================
    #[test]
    fn test_validation_error_construction() {
        // Arrange & Act
        let error = ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidSourceType,
            path: "$.sources[0].type".to_string(),
            message: "Invalid source type 'ftp'".to_string(),
            severity: Severity::Error,
            suggestion: Some("Did you mean 'http_poll'?".to_string()),
        };

        // Assert
        assert_eq!(error.layer, ValidationLayer::Semantic);
        assert_eq!(error.path, "$.sources[0].type");
        assert!(error.suggestion.is_some());
        assert_eq!(error.suggestion.unwrap(), "Did you mean 'http_poll'?");
    }

    #[test]
    fn test_validation_error_semantic_helper() {
        let error = ValidationError::semantic(
            ErrorCode::InvalidDqRule,
            "$.silver_etl.dq_rules[0]",
            "range_check min must be less than max",
        );

        assert_eq!(error.layer, ValidationLayer::Semantic);
        assert_eq!(error.code, ErrorCode::InvalidDqRule);
        assert_eq!(error.path, "$.silver_etl.dq_rules[0]");
        assert_eq!(error.severity, Severity::Error);
        assert!(error.suggestion.is_none());
    }

    #[test]
    fn test_validation_error_schema_helper() {
        let error = ValidationError::schema(
            ErrorCode::MissingRequired,
            "$.stream_id",
            "required field missing",
        );

        assert_eq!(error.layer, ValidationLayer::Schema);
        assert_eq!(error.code, ErrorCode::MissingRequired);
    }

    #[test]
    fn test_validation_error_warning_helper() {
        let error = ValidationError::warning(
            ErrorCode::DeprecatedField,
            "$.entity_schemas",
            "entity_schemas is deprecated, use fields instead",
        );

        assert_eq!(error.severity, Severity::Warning);
    }

    #[test]
    fn test_validation_error_with_suggestion() {
        let error = ValidationError::semantic(
            ErrorCode::InvalidSourceType,
            "$.sources[0].type",
            "Unknown source type 'ftp'",
        )
        .with_suggestion("Did you mean 'http_poll'?");

        assert_eq!(
            error.suggestion,
            Some("Did you mean 'http_poll'?".to_string())
        );
    }

    // =========================================================================
    // TC-202: ErrorCode Display implementation
    // Description: Verify ErrorCode serializes to SCREAMING_SNAKE_CASE
    // Priority: High
    // =========================================================================
    #[test]
    fn test_validation_error_json_format() {
        let error = ValidationError::semantic(
            ErrorCode::InvalidDqRule,
            "$.silver_etl.dq_rules[0]",
            "range_check min must be less than max",
        );

        let json = serde_json::to_value(&error).unwrap();

        assert_eq!(json["layer"], "semantic");
        assert_eq!(json["code"], "INVALID_DQ_RULE");
        assert_eq!(json["severity"], "error");
    }

    // =========================================================================
    // TC-203: Severity levels (Error, Warning)
    // Priority: High
    // =========================================================================
    #[test]
    fn test_severity_error_blocks_validation() {
        let error = ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidRange,
            path: "$.test".to_string(),
            message: "test error".to_string(),
            severity: Severity::Error,
            suggestion: None,
        };

        assert_eq!(error.severity, Severity::Error);
    }

    #[test]
    fn test_severity_warning_allows_validation() {
        let warning =
            ValidationError::warning(ErrorCode::DeprecatedField, "$.old_field", "deprecated");

        assert_eq!(warning.severity, Severity::Warning);
    }

    // =========================================================================
    // TC-204: ValidationContext field lookup
    // Priority: High
    // =========================================================================
    #[test]
    fn test_validation_context_field_lookup() {
        let mut ctx = ValidationContext::new();
        ctx.add_field("pm25".to_string());
        ctx.add_field("temperature".to_string());

        assert!(ctx.has_field("pm25"));
        assert!(ctx.has_field("temperature"));
        assert!(!ctx.has_field("unknown"));
    }

    #[test]
    fn test_validation_context_with_fields() {
        let ctx = ValidationContext::with_fields(vec!["field1".to_string(), "field2".to_string()]);

        assert!(ctx.has_field("field1"));
        assert!(ctx.has_field("field2"));
        assert!(!ctx.has_field("field3"));
    }

    // =========================================================================
    // TC-205: validate() returns empty vec for valid input
    // Priority: Critical
    // =========================================================================
    #[test]
    fn test_ndp_validate_trait_is_valid() {
        // Create a simple test struct that implements NdpValidate
        struct ValidConfig;

        impl NdpValidate for ValidConfig {
            fn validate(&self) -> Vec<ValidationError> {
                Vec::new() // Valid config has no errors
            }
        }

        let config = ValidConfig;
        assert!(config.is_valid());
        assert!(config.validate().is_empty());
    }

    #[test]
    fn test_ndp_validate_trait_is_invalid() {
        struct InvalidConfig;

        impl NdpValidate for InvalidConfig {
            fn validate(&self) -> Vec<ValidationError> {
                vec![ValidationError::semantic(
                    ErrorCode::InvalidRange,
                    "$",
                    "always invalid",
                )]
            }
        }

        let config = InvalidConfig;
        assert!(!config.is_valid());
        assert_eq!(config.validate().len(), 1);
    }

    #[test]
    fn test_ndp_validate_with_warnings_is_valid() {
        struct WarningConfig;

        impl NdpValidate for WarningConfig {
            fn validate(&self) -> Vec<ValidationError> {
                vec![ValidationError::warning(
                    ErrorCode::DeprecatedField,
                    "$",
                    "using deprecated field",
                )]
            }
        }

        let config = WarningConfig;
        // Warnings don't make config invalid
        assert!(config.is_valid());
        assert_eq!(config.validate().len(), 1);
    }

    // =========================================================================
    // Serialization tests
    // =========================================================================
    #[test]
    fn test_validation_layer_serialization() {
        let layers = vec![
            (ValidationLayer::Syntax, "\"syntax\""),
            (ValidationLayer::Schema, "\"schema\""),
            (ValidationLayer::Semantic, "\"semantic\""),
        ];

        for (layer, expected) in layers {
            let json = serde_json::to_string(&layer).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn test_severity_serialization() {
        let severities = vec![
            (Severity::Error, "\"error\""),
            (Severity::Warning, "\"warning\""),
        ];

        for (severity, expected) in severities {
            let json = serde_json::to_string(&severity).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn test_validation_error_round_trip() {
        let error = ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidSourcePath,
            path: "$.silver_etl.field_mappings[0].source_path".to_string(),
            message: "source_path references undefined field".to_string(),
            severity: Severity::Error,
            suggestion: Some("Define the field in schema_fields".to_string()),
        };

        let json = serde_json::to_string(&error).unwrap();
        let restored: ValidationError = serde_json::from_str(&json).unwrap();

        assert_eq!(error.layer, restored.layer);
        assert_eq!(error.code, restored.code);
        assert_eq!(error.path, restored.path);
        assert_eq!(error.message, restored.message);
        assert_eq!(error.severity, restored.severity);
        assert_eq!(error.suggestion, restored.suggestion);
    }

    // =========================================================================
    // ValidationContext tests
    // =========================================================================
    #[test]
    fn test_validation_context_default() {
        let ctx = ValidationContext::default();
        assert!(ctx.field_names.is_empty());
        assert!(ctx.silver_columns.is_empty());
        assert!(ctx.stream_id.is_none());
        assert!(ctx.database_url.is_none());
    }

    #[test]
    fn test_validation_context_with_all_fields() {
        let ctx = ValidationContext {
            field_names: vec!["a".to_string(), "b".to_string()].into_iter().collect(),
            silver_columns: vec!["col1".to_string()].into_iter().collect(),
            stream_id: Some("test-stream".to_string()),
            database_url: Some("postgres://localhost/test".to_string()),
        };

        assert!(ctx.has_field("a"));
        assert!(ctx.has_field("b"));
        assert!(!ctx.has_field("c"));
        assert_eq!(ctx.stream_id, Some("test-stream".to_string()));
    }
}
