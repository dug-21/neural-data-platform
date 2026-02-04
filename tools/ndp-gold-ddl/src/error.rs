//! Error types for ndp-gold-ddl
//!
//! Structured error types for Gold DDL generation.

use thiserror::Error;

/// Error codes for Gold DDL generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Configuration file not found
    ConfigNotFound = 100,
    /// Configuration parse error
    ConfigParseError = 101,
    /// Gold ETL not enabled
    GoldEtlDisabled = 102,
    /// Missing required field in configuration
    MissingRequiredField = 103,

    /// Invalid aggregate metric
    InvalidMetric = 200,
    /// Invalid granularity format
    InvalidGranularity = 201,
    /// Invalid window format
    InvalidWindow = 202,

    /// Field not found in stream configuration
    FieldNotFound = 300,
    /// Invalid field type for operation
    InvalidFieldType = 301,

    /// Unknown feature type
    UnknownFeatureType = 400,
    /// Invalid feature configuration
    InvalidFeatureConfig = 401,

    /// Generation failed
    GenerationFailed = 500,
}

/// Main error type for ndp-gold-ddl
#[derive(Debug, Error)]
pub enum GoldDdlError {
    #[error("Configuration not found: {path}")]
    ConfigNotFound { path: String },

    #[error("Failed to parse configuration: {message}")]
    ConfigParseError { message: String },

    #[error("Gold ETL is not enabled for stream '{stream_id}'")]
    GoldEtlDisabled { stream_id: String },

    #[error("Missing required field '{field}' in {context}")]
    MissingRequiredField { field: String, context: String },

    #[error("Invalid metric '{metric}' for field '{field}'. Valid metrics: {valid:?}")]
    InvalidMetric {
        metric: String,
        field: String,
        valid: Vec<String>,
    },

    #[error("Invalid granularity '{granularity}'. Expected format: '<number> <unit>' (e.g., '1 hour', '1 day')")]
    InvalidGranularity { granularity: String },

    #[error("Invalid window '{window}'. Expected format: '<number> <unit>' (e.g., '4 hours', '24 hours')")]
    InvalidWindow { window: String },

    #[error("Field '{field}' not found in stream '{stream_id}'. Available fields: {available:?}")]
    FieldNotFound {
        field: String,
        stream_id: String,
        available: Vec<String>,
    },

    #[error("Invalid field type for '{field}': {reason}")]
    InvalidFieldType { field: String, reason: String },

    #[error("Unknown feature type '{feature_type}'. Available types: {available:?}")]
    UnknownFeatureType {
        feature_type: String,
        available: Vec<String>,
    },

    #[error("Invalid feature configuration for '{feature_type}': {message}")]
    InvalidFeatureConfig {
        feature_type: String,
        message: String,
    },

    #[error("DDL generation failed: {message}")]
    GenerationFailed { message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl GoldDdlError {
    /// Get the error code for this error
    pub fn code(&self) -> ErrorCode {
        match self {
            GoldDdlError::ConfigNotFound { .. } => ErrorCode::ConfigNotFound,
            GoldDdlError::ConfigParseError { .. } => ErrorCode::ConfigParseError,
            GoldDdlError::GoldEtlDisabled { .. } => ErrorCode::GoldEtlDisabled,
            GoldDdlError::MissingRequiredField { .. } => ErrorCode::MissingRequiredField,
            GoldDdlError::InvalidMetric { .. } => ErrorCode::InvalidMetric,
            GoldDdlError::InvalidGranularity { .. } => ErrorCode::InvalidGranularity,
            GoldDdlError::InvalidWindow { .. } => ErrorCode::InvalidWindow,
            GoldDdlError::FieldNotFound { .. } => ErrorCode::FieldNotFound,
            GoldDdlError::InvalidFieldType { .. } => ErrorCode::InvalidFieldType,
            GoldDdlError::UnknownFeatureType { .. } => ErrorCode::UnknownFeatureType,
            GoldDdlError::InvalidFeatureConfig { .. } => ErrorCode::InvalidFeatureConfig,
            GoldDdlError::GenerationFailed { .. } => ErrorCode::GenerationFailed,
            GoldDdlError::IoError(_) => ErrorCode::ConfigNotFound,
            GoldDdlError::JsonError(_) => ErrorCode::ConfigParseError,
        }
    }
}

/// Result type for Gold DDL operations
pub type Result<T> = std::result::Result<T, GoldDdlError>;
