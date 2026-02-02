//! Error codes for NDP validation errors.
//!
//! This module defines machine-readable error codes for all validation
//! errors that can occur in the NDP configuration pipeline.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

/// Machine-readable error codes for validation errors.
///
/// Error codes are grouped by category:
/// - 100-199: Syntax errors (JSON parsing)
/// - 200-299: Schema errors (JSON Schema validation)
/// - 300-319: Type validation errors
/// - 320-339: Cross-reference errors
/// - 340-359: External validation errors (database checks)
/// - 360-379: Source configuration errors
/// - 380-399: DQ rule errors
/// - 900-999: Warnings (non-blocking)
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    Display,
    EnumString,
    AsRefStr,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // =========================================================================
    // Syntax Errors (100-199)
    // =========================================================================
    /// JSON syntax error (malformed JSON)
    SyntaxError,

    // =========================================================================
    // Schema Errors (200-299)
    // =========================================================================
    /// Required field is missing
    MissingRequired,
    /// Field has wrong type
    InvalidType,
    /// Unknown field in object
    UnknownField,
    /// String does not match required pattern
    PatternMismatch,
    /// Value is not in allowed enum
    EnumViolation,
    /// Array length out of bounds
    ArrayBounds,

    // =========================================================================
    // Type Validation Errors (300-319)
    // =========================================================================
    /// Invalid field type specification
    InvalidFieldType,
    /// Invalid source type specification
    InvalidSourceType,
    /// Invalid numeric range (min >= max)
    InvalidRange,
    /// Invalid precision value
    InvalidPrecision,

    // =========================================================================
    // Cross-Reference Errors (320-339)
    // =========================================================================
    /// Source path references undefined field
    InvalidSourcePath,
    /// Duplicate name found
    DuplicateName,
    /// Constraint violation
    ConstraintViolation,

    // =========================================================================
    // External Validation Errors (340-359)
    // =========================================================================
    /// Referenced table does not exist
    TableNotFound,
    /// Referenced column does not exist
    ColumnNotFound,
    /// Type mismatch with database
    TypeMismatch,
    /// Table check failed
    TableCheckFailed,
    /// Column check failed
    ColumnCheckFailed,
    /// Invalid table name format
    InvalidTableFormat,

    // =========================================================================
    // Source Configuration Errors (360-379)
    // =========================================================================
    /// Missing required source configuration
    MissingSourceConfig,
    /// Invalid source configuration
    InvalidSourceConfig,

    // =========================================================================
    // DQ Rule Errors (380-399)
    // =========================================================================
    /// Invalid DQ rule type
    InvalidDqRuleType,
    /// Invalid DQ rule configuration
    InvalidDqRule,
    /// Invalid DQ action
    InvalidDqAction,
    /// Invalid DQ column reference
    InvalidDqColumn,
    /// Invalid DQ rule syntax
    InvalidDqSyntax,
    /// Invalid regex pattern
    InvalidRegex,
    /// Invalid interval/duration format
    InvalidInterval,
    /// Invalid transform configuration
    InvalidTransform,

    // =========================================================================
    // Warnings (900-999)
    // =========================================================================
    /// Unknown device class (non-blocking warning)
    UnknownDeviceClass,
    /// Deprecated field usage
    DeprecatedField,
    /// Recommended field missing
    RecommendedFieldMissing,
}

impl ErrorCode {
    /// Get the numeric code for this error.
    ///
    /// Codes follow the grouping defined in the enum documentation.
    pub fn numeric_code(&self) -> u16 {
        match self {
            // Syntax (100-199)
            ErrorCode::SyntaxError => 100,

            // Schema (200-299)
            ErrorCode::MissingRequired => 200,
            ErrorCode::InvalidType => 201,
            ErrorCode::UnknownField => 202,
            ErrorCode::PatternMismatch => 203,
            ErrorCode::EnumViolation => 204,
            ErrorCode::ArrayBounds => 205,

            // Type Validation (300-319)
            ErrorCode::InvalidFieldType => 300,
            ErrorCode::InvalidSourceType => 301,
            ErrorCode::InvalidRange => 302,
            ErrorCode::InvalidPrecision => 303,

            // Cross-Reference (320-339)
            ErrorCode::InvalidSourcePath => 320,
            ErrorCode::DuplicateName => 321,
            ErrorCode::ConstraintViolation => 322,

            // External (340-359)
            ErrorCode::TableNotFound => 340,
            ErrorCode::ColumnNotFound => 341,
            ErrorCode::TypeMismatch => 342,
            ErrorCode::TableCheckFailed => 343,
            ErrorCode::ColumnCheckFailed => 344,
            ErrorCode::InvalidTableFormat => 345,

            // Source Config (360-379)
            ErrorCode::MissingSourceConfig => 360,
            ErrorCode::InvalidSourceConfig => 361,

            // DQ Rules (380-399)
            ErrorCode::InvalidDqRuleType => 380,
            ErrorCode::InvalidDqRule => 381,
            ErrorCode::InvalidDqAction => 382,
            ErrorCode::InvalidDqColumn => 383,
            ErrorCode::InvalidDqSyntax => 384,
            ErrorCode::InvalidRegex => 385,
            ErrorCode::InvalidInterval => 386,
            ErrorCode::InvalidTransform => 387,

            // Warnings (900-999)
            ErrorCode::UnknownDeviceClass => 900,
            ErrorCode::DeprecatedField => 901,
            ErrorCode::RecommendedFieldMissing => 902,
        }
    }

    /// Check if this error code represents a warning (non-blocking).
    pub fn is_warning(&self) -> bool {
        self.numeric_code() >= 900
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_serialization() {
        let code = ErrorCode::InvalidSourceType;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"INVALID_SOURCE_TYPE\"");

        let parsed: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, code);
    }

    #[test]
    fn test_numeric_codes() {
        assert_eq!(ErrorCode::SyntaxError.numeric_code(), 100);
        assert_eq!(ErrorCode::MissingRequired.numeric_code(), 200);
        assert_eq!(ErrorCode::InvalidSourceType.numeric_code(), 301);
        assert_eq!(ErrorCode::InvalidDqRule.numeric_code(), 381);
        assert_eq!(ErrorCode::UnknownDeviceClass.numeric_code(), 900);
    }

    #[test]
    fn test_is_warning() {
        assert!(!ErrorCode::InvalidSourceType.is_warning());
        assert!(!ErrorCode::InvalidDqRule.is_warning());
        assert!(ErrorCode::UnknownDeviceClass.is_warning());
        assert!(ErrorCode::DeprecatedField.is_warning());
    }
}
