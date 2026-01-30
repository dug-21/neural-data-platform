//! Error types for dimension loading operations
//!
//! Following NDP's thiserror pattern from core/src/error.rs

use thiserror::Error;

/// Errors that can occur during dimension loading
#[derive(Error, Debug)]
pub enum DimensionError {
    /// CSV parsing error with line context
    #[error("CSV parsing error at line {line}: {message}")]
    CsvError { line: usize, message: String },

    /// Database operation error
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// IO error (file not found, permissions, etc.)
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Configuration validation error
    #[error("Config error: {0}")]
    ConfigError(String),

    /// Schema mismatch between CSV and config
    #[error("Schema mismatch: expected {expected} columns, got {actual}")]
    SchemaMismatch { expected: usize, actual: usize },

    /// Invalid field type or value conversion error
    #[error("Invalid field type for {field}: {reason}")]
    InvalidFieldType { field: String, reason: String },

    /// Missing required field
    #[error("Missing required field '{field}' at line {line}")]
    MissingField { field: String, line: usize },

    /// Primary key violation or constraint error
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    /// Connection pool error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Transaction error
    #[error("Transaction error: {0}")]
    TransactionError(String),
}

impl DimensionError {
    /// Create a CSV error with line number
    pub fn csv_parse(line: usize, message: impl Into<String>) -> Self {
        Self::CsvError {
            line,
            message: message.into(),
        }
    }

    /// Create a database error
    pub fn database(message: impl Into<String>) -> Self {
        Self::DatabaseError(message.into())
    }

    /// Create a config validation error
    pub fn config(message: impl Into<String>) -> Self {
        Self::ConfigError(message.into())
    }

    /// Create a missing field error
    pub fn missing_field(field: impl Into<String>, line: usize) -> Self {
        Self::MissingField {
            field: field.into(),
            line,
        }
    }

    /// Create a type conversion error
    pub fn invalid_type(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidFieldType {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

/// Convert DimensionError to CoreError for integration with existing error handling
impl From<DimensionError> for crate::error::CoreError {
    fn from(err: DimensionError) -> Self {
        match err {
            DimensionError::CsvError { .. } => crate::error::CoreError::Parser(err.to_string()),
            DimensionError::DatabaseError(_) => {
                crate::error::CoreError::DatabaseError(err.to_string())
            }
            DimensionError::IoError(_) => crate::error::CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                err.to_string(),
            )),
            DimensionError::ConfigError(_) => crate::error::CoreError::Config(err.to_string()),
            DimensionError::SchemaMismatch { .. } => {
                crate::error::CoreError::Validation(err.to_string())
            }
            DimensionError::InvalidFieldType { .. } => {
                crate::error::CoreError::Validation(err.to_string())
            }
            DimensionError::MissingField { .. } => {
                crate::error::CoreError::Validation(err.to_string())
            }
            DimensionError::ConstraintViolation(_) => {
                crate::error::CoreError::DatabaseError(err.to_string())
            }
            DimensionError::ConnectionError(_) => {
                crate::error::CoreError::DatabaseError(err.to_string())
            }
            DimensionError::TransactionError(_) => {
                crate::error::CoreError::DatabaseError(err.to_string())
            }
        }
    }
}

#[cfg(feature = "timescale")]
impl From<tokio_postgres::Error> for DimensionError {
    fn from(err: tokio_postgres::Error) -> Self {
        DimensionError::DatabaseError(err.to_string())
    }
}

#[cfg(feature = "timescale")]
impl<E: std::error::Error + 'static> From<bb8::RunError<E>> for DimensionError {
    fn from(err: bb8::RunError<E>) -> Self {
        DimensionError::ConnectionError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_error_display() {
        let err = DimensionError::csv_parse(42, "invalid character");
        assert_eq!(
            err.to_string(),
            "CSV parsing error at line 42: invalid character"
        );
    }

    #[test]
    fn test_schema_mismatch_display() {
        let err = DimensionError::SchemaMismatch {
            expected: 5,
            actual: 3,
        };
        assert_eq!(
            err.to_string(),
            "Schema mismatch: expected 5 columns, got 3"
        );
    }

    #[test]
    fn test_missing_field_display() {
        let err = DimensionError::missing_field("user_id", 10);
        assert_eq!(
            err.to_string(),
            "Missing required field 'user_id' at line 10"
        );
    }

    #[test]
    fn test_invalid_type_display() {
        let err = DimensionError::invalid_type("age", "expected integer");
        assert_eq!(
            err.to_string(),
            "Invalid field type for age: expected integer"
        );
    }

    #[test]
    fn test_convert_to_core_error() {
        let dim_err = DimensionError::database("connection timeout");
        let core_err: crate::error::CoreError = dim_err.into();

        match core_err {
            crate::error::CoreError::DatabaseError(msg) => {
                assert!(msg.contains("connection timeout"));
            }
            _ => panic!("Expected DatabaseError variant"),
        }
    }
}
