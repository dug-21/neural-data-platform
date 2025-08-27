//! Error types for Data-Staging service

use thiserror::Error;

/// Data-Staging service error types
#[derive(Error, Debug)]
pub enum DataStagingError {
    #[error("Service not initialized")]
    NotInitialized,
    
    #[error("Redis connection error: {0}")]
    RedisError(#[from] redis::RedisError),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Protocol buffer serialization error: {0}")]
    ProtoSerialization(#[from] prost::EncodeError),
    
    #[error("Protocol buffer deserialization error: {0}")]
    ProtoDeserialization(#[from] prost::DecodeError),
    
    #[error("Data validation failed: {message}")]
    ValidationError { message: String },
    
    #[error("Data quality insufficient: score={score}, threshold={threshold}")]
    QualityError { score: f32, threshold: f32 },
    
    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },
    
    #[error("Invalid data format: {message}")]
    InvalidFormat { message: String },
    
    #[error("EventBus integration error: {0}")]
    EventBusError(#[from] neural_core::eventbus::EventBusError),
    
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
    
    #[error("DLQ operation failed: {message}")]
    DlqError { message: String },
    
    #[error("Metrics error: {0}")]
    MetricsError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl DataStagingError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RedisError(_) => true,
            Self::EventBusError(_) => true,
            Self::IoError(_) => true,
            Self::Internal(_) => true,
            _ => false,
        }
    }
    
    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::NotInitialized => "initialization",
            Self::RedisError(_) => "redis",
            Self::JsonError(_) => "json",
            Self::ProtoSerialization(_) | Self::ProtoDeserialization(_) => "proto",
            Self::ValidationError { .. } => "validation",
            Self::QualityError { .. } => "quality",
            Self::MissingRequiredField { .. } => "missing_field",
            Self::InvalidFormat { .. } => "format",
            Self::EventBusError(_) => "eventbus",
            Self::ConfigError { .. } => "config",
            Self::DlqError { .. } => "dlq",
            Self::MetricsError(_) => "metrics",
            Self::IoError(_) => "io",
            Self::Internal(_) => "internal",
        }
    }
}

/// Result type alias for Data-Staging operations
pub type DataStagingResult<T> = Result<T, DataStagingError>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_retryable() {
        let redis_error = DataStagingError::RedisError(
            redis::RedisError::from(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test"))
        );
        assert!(redis_error.is_retryable());
        
        let validation_error = DataStagingError::ValidationError {
            message: "Invalid data".to_string()
        };
        assert!(!validation_error.is_retryable());
    }
    
    #[test]
    fn test_error_category() {
        let json_error = DataStagingError::JsonError(
            serde_json::Error::custom("test")
        );
        assert_eq!(json_error.category(), "json");
        
        let validation_error = DataStagingError::ValidationError {
            message: "test".to_string()
        };
        assert_eq!(validation_error.category(), "validation");
    }
}