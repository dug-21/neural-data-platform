//! Error types for NDP validation
//!
//! Defines structured error types following the dp-019 specification:
//! - SYNTAX_ERROR: Malformed JSON
//! - MISSING_REQUIRED: Required field not present
//! - INVALID_TYPE: Wrong JSON type
//! - UNKNOWN_FIELD: Unexpected field (additionalProperties violation)
//! - PATTERN_MISMATCH: String doesn't match regex pattern
//! - ENUM_VIOLATION: Value not in allowed enum

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Validation layer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationLayer {
    /// JSON syntax errors (malformed JSON)
    Syntax,
    /// JSON Schema validation errors
    Schema,
    /// Application-level semantic validation
    Semantic,
}

/// Error severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Must fix before deploy
    Error,
    /// Should review, may be intentional
    Warning,
}

/// Error code enumeration per dp-019 specification section 5.3
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Layer 1: Syntax
    SyntaxError,

    // Layer 1: Schema
    MissingRequired,
    InvalidType,
    UnknownField,
    PatternMismatch,
    EnumViolation,
    ArrayBounds,

    // Layer 2: Semantic - Types (300-319)
    InvalidFieldType,
    InvalidSourceType,
    InvalidRange,
    InvalidPrecision,

    // Layer 2: Semantic - Cross-Reference (320-339)
    InvalidSourcePath,
    DuplicateName,
    ConstraintViolation,

    // Layer 2: Semantic - External (340-359)
    TableNotFound,
    ColumnNotFound,
    TypeMismatch,
    TableCheckFailed,
    ColumnCheckFailed,
    InvalidTableFormat,

    // Layer 2: Semantic - Source Config (360-379)
    MissingSourceConfig,
    InvalidSourceConfig,

    // Layer 2: Semantic - DQ Rules (380-399)
    InvalidDqRuleType,
    InvalidDqRule,
    InvalidDqAction,
    InvalidDqColumn,
    InvalidDqSyntax,
    InvalidRegex,
    InvalidInterval,
    InvalidTransform,

    // Layer 2: Semantic - Gold Layer (400-408)
    InvalidGoldField,          // gold_etl references field not in stream
    InvalidStreamType,         // transitions on non-state_event stream
    UnknownAlignmentStream,    // alignment references unknown stream
    InvalidAggregateMetric,    // unknown metric type
    InvalidDomainStream,       // domain references non-existent stream
    InvalidFeatureType,        // unknown feature type
    InvalidGranularity,        // granularity format not recognized
    CircularDomainDependency,  // domain references itself
    InvalidObjectiveCondition, // objective condition not supported

    // Warnings (900-999)
    UnknownDeviceClass,
}

impl ErrorCode {
    /// Get the validation layer for this error code
    pub fn layer(&self) -> ValidationLayer {
        match self {
            ErrorCode::SyntaxError => ValidationLayer::Syntax,
            ErrorCode::MissingRequired
            | ErrorCode::InvalidType
            | ErrorCode::UnknownField
            | ErrorCode::PatternMismatch
            | ErrorCode::EnumViolation
            | ErrorCode::ArrayBounds => ValidationLayer::Schema,
            ErrorCode::InvalidFieldType
            | ErrorCode::InvalidSourceType
            | ErrorCode::InvalidRange
            | ErrorCode::InvalidPrecision
            | ErrorCode::InvalidSourcePath
            | ErrorCode::DuplicateName
            | ErrorCode::ConstraintViolation
            | ErrorCode::TableNotFound
            | ErrorCode::ColumnNotFound
            | ErrorCode::TypeMismatch
            | ErrorCode::TableCheckFailed
            | ErrorCode::ColumnCheckFailed
            | ErrorCode::InvalidTableFormat
            | ErrorCode::MissingSourceConfig
            | ErrorCode::InvalidSourceConfig
            | ErrorCode::InvalidDqRuleType
            | ErrorCode::InvalidDqRule
            | ErrorCode::InvalidDqAction
            | ErrorCode::InvalidDqColumn
            | ErrorCode::InvalidDqSyntax
            | ErrorCode::InvalidRegex
            | ErrorCode::InvalidInterval
            | ErrorCode::InvalidTransform
            | ErrorCode::InvalidGoldField
            | ErrorCode::InvalidStreamType
            | ErrorCode::UnknownAlignmentStream
            | ErrorCode::InvalidAggregateMetric
            | ErrorCode::InvalidDomainStream
            | ErrorCode::InvalidFeatureType
            | ErrorCode::InvalidGranularity
            | ErrorCode::CircularDomainDependency
            | ErrorCode::InvalidObjectiveCondition
            | ErrorCode::UnknownDeviceClass => ValidationLayer::Semantic,
        }
    }

    /// Get the default severity for this error code
    pub fn default_severity(&self) -> Severity {
        match self {
            ErrorCode::UnknownDeviceClass
            | ErrorCode::TableCheckFailed
            | ErrorCode::ColumnCheckFailed
            | ErrorCode::TypeMismatch
            | ErrorCode::InvalidStreamType => Severity::Warning, // Transitions on non-state_event is a warning
            _ => Severity::Error,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ErrorCode::SyntaxError => "SYNTAX_ERROR",
            ErrorCode::MissingRequired => "MISSING_REQUIRED",
            ErrorCode::InvalidType => "INVALID_TYPE",
            ErrorCode::UnknownField => "UNKNOWN_FIELD",
            ErrorCode::PatternMismatch => "PATTERN_MISMATCH",
            ErrorCode::EnumViolation => "ENUM_VIOLATION",
            ErrorCode::ArrayBounds => "ARRAY_BOUNDS",
            ErrorCode::InvalidFieldType => "INVALID_FIELD_TYPE",
            ErrorCode::InvalidSourceType => "INVALID_SOURCE_TYPE",
            ErrorCode::InvalidRange => "INVALID_RANGE",
            ErrorCode::InvalidPrecision => "INVALID_PRECISION",
            ErrorCode::InvalidSourcePath => "INVALID_SOURCE_PATH",
            ErrorCode::DuplicateName => "DUPLICATE_NAME",
            ErrorCode::ConstraintViolation => "CONSTRAINT_VIOLATION",
            ErrorCode::TableNotFound => "TABLE_NOT_FOUND",
            ErrorCode::ColumnNotFound => "COLUMN_NOT_FOUND",
            ErrorCode::TypeMismatch => "TYPE_MISMATCH",
            ErrorCode::TableCheckFailed => "TABLE_CHECK_FAILED",
            ErrorCode::ColumnCheckFailed => "COLUMN_CHECK_FAILED",
            ErrorCode::InvalidTableFormat => "INVALID_TABLE_FORMAT",
            ErrorCode::MissingSourceConfig => "MISSING_SOURCE_CONFIG",
            ErrorCode::InvalidSourceConfig => "INVALID_SOURCE_CONFIG",
            ErrorCode::InvalidDqRuleType => "INVALID_DQ_RULE_TYPE",
            ErrorCode::InvalidDqRule => "INVALID_DQ_RULE",
            ErrorCode::InvalidDqAction => "INVALID_DQ_ACTION",
            ErrorCode::InvalidDqColumn => "INVALID_DQ_COLUMN",
            ErrorCode::InvalidDqSyntax => "INVALID_DQ_SYNTAX",
            ErrorCode::InvalidRegex => "INVALID_REGEX",
            ErrorCode::InvalidInterval => "INVALID_INTERVAL",
            ErrorCode::InvalidTransform => "INVALID_TRANSFORM",
            ErrorCode::InvalidGoldField => "INVALID_GOLD_FIELD",
            ErrorCode::InvalidStreamType => "INVALID_STREAM_TYPE",
            ErrorCode::UnknownAlignmentStream => "UNKNOWN_ALIGNMENT_STREAM",
            ErrorCode::InvalidAggregateMetric => "INVALID_AGGREGATE_METRIC",
            ErrorCode::InvalidDomainStream => "INVALID_DOMAIN_STREAM",
            ErrorCode::InvalidFeatureType => "INVALID_FEATURE_TYPE",
            ErrorCode::InvalidGranularity => "INVALID_GRANULARITY",
            ErrorCode::CircularDomainDependency => "CIRCULAR_DOMAIN_DEPENDENCY",
            ErrorCode::InvalidObjectiveCondition => "INVALID_OBJECTIVE_CONDITION",
            ErrorCode::UnknownDeviceClass => "UNKNOWN_DEVICE_CLASS",
        };
        write!(f, "{}", s)
    }
}

/// A structured validation error per dp-019 specification section 5.1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Validation layer (syntax, schema, semantic)
    pub layer: ValidationLayer,

    /// Error code from standard list
    pub code: ErrorCode,

    /// JSONPath to error location (e.g., "$.fields[0].type")
    pub path: String,

    /// Human-readable error message
    pub message: String,

    /// Severity level
    pub severity: Severity,

    /// Optional actionable suggestion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,

    /// Optional additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

impl ValidationError {
    /// Create a new syntax error with line/column information
    pub fn syntax_error(line: usize, column: usize, message: &str) -> Self {
        Self {
            layer: ValidationLayer::Syntax,
            code: ErrorCode::SyntaxError,
            path: format!("line {}, column {}", line, column),
            message: message.to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: Some(serde_json::json!({
                "line": line,
                "column": column
            })),
        }
    }

    /// Create a schema validation error
    pub fn schema_error(code: ErrorCode, path: &str, message: &str) -> Self {
        Self {
            layer: ValidationLayer::Schema,
            code,
            path: path.to_string(),
            message: message.to_string(),
            severity: code.default_severity(),
            suggestion: None,
            context: None,
        }
    }

    /// Add a suggestion to this error
    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.to_string());
        self
    }

    /// Add context to this error
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Create a semantic validation error
    pub fn semantic_error(code: ErrorCode, path: &str, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Semantic,
            code,
            path: path.to_string(),
            message: message.into(),
            severity: code.default_severity(),
            suggestion: None,
            context: None,
        }
    }

    /// Create a semantic validation warning
    pub fn semantic_warning(code: ErrorCode, path: &str, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Semantic,
            code,
            path: path.to_string(),
            message: message.into(),
            severity: Severity::Warning,
            suggestion: None,
            context: None,
        }
    }
}

/// Schema validator error (internal errors, not validation errors)
#[derive(Debug, Error)]
pub enum SchemaValidatorError {
    #[error("Failed to load schema: {0}")]
    SchemaLoadError(String),

    #[error("Failed to compile schema: {0}")]
    SchemaCompileError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_layer_mapping() {
        assert_eq!(ErrorCode::SyntaxError.layer(), ValidationLayer::Syntax);
        assert_eq!(ErrorCode::MissingRequired.layer(), ValidationLayer::Schema);
        assert_eq!(ErrorCode::InvalidType.layer(), ValidationLayer::Schema);
        assert_eq!(ErrorCode::UnknownField.layer(), ValidationLayer::Schema);
        assert_eq!(
            ErrorCode::InvalidSourcePath.layer(),
            ValidationLayer::Semantic
        );
        // Gold layer error codes
        assert_eq!(
            ErrorCode::InvalidGoldField.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::InvalidStreamType.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::UnknownAlignmentStream.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::InvalidAggregateMetric.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::InvalidDomainStream.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::InvalidFeatureType.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::InvalidGranularity.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::CircularDomainDependency.layer(),
            ValidationLayer::Semantic
        );
        assert_eq!(
            ErrorCode::InvalidObjectiveCondition.layer(),
            ValidationLayer::Semantic
        );
    }

    #[test]
    fn test_error_code_default_severity() {
        assert_eq!(ErrorCode::SyntaxError.default_severity(), Severity::Error);
        assert_eq!(
            ErrorCode::UnknownDeviceClass.default_severity(),
            Severity::Warning
        );
        assert_eq!(
            ErrorCode::MissingRequired.default_severity(),
            Severity::Error
        );
        // Gold layer error codes
        assert_eq!(
            ErrorCode::InvalidGoldField.default_severity(),
            Severity::Error
        );
        assert_eq!(
            ErrorCode::InvalidStreamType.default_severity(),
            Severity::Warning
        );
        assert_eq!(
            ErrorCode::InvalidAggregateMetric.default_severity(),
            Severity::Error
        );
    }

    #[test]
    fn test_syntax_error_creation() {
        let err = ValidationError::syntax_error(5, 10, "Unexpected token");

        assert_eq!(err.layer, ValidationLayer::Syntax);
        assert_eq!(err.code, ErrorCode::SyntaxError);
        assert_eq!(err.path, "line 5, column 10");
        assert_eq!(err.message, "Unexpected token");
        assert_eq!(err.severity, Severity::Error);

        let ctx = err.context.unwrap();
        assert_eq!(ctx["line"], 5);
        assert_eq!(ctx["column"], 10);
    }

    #[test]
    fn test_schema_error_with_suggestion() {
        let err = ValidationError::schema_error(
            ErrorCode::UnknownField,
            "$.silver_elt",
            "Unknown field 'silver_elt'",
        )
        .with_suggestion("Did you mean 'silver_etl'?");

        assert_eq!(err.code, ErrorCode::UnknownField);
        assert_eq!(err.suggestion.unwrap(), "Did you mean 'silver_etl'?");
    }

    #[test]
    fn test_validation_error_serialization() {
        let err = ValidationError::syntax_error(1, 5, "Unexpected end of input");
        let json = serde_json::to_string(&err).unwrap();

        assert!(json.contains("\"layer\":\"syntax\""));
        assert!(json.contains("\"code\":\"SYNTAX_ERROR\""));
        assert!(json.contains("\"severity\":\"error\""));
    }
}
