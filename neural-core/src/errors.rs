//! Core error types shared across binaries
//! Module size: <200 lines as per requirements

use thiserror::Error;

/// Core error type for neural-trader
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Data error: {0}")]
    DataError(String),
    
    #[error("Prediction error: {0}")]
    PredictionError(String),
    
    #[error("Trading error: {0}")]
    TradingError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Event bus error: {0}")]
    EventError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Model error: {0}")]
    ModelError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self, 
            CoreError::NetworkError(_) | 
            CoreError::StorageError(_) |
            CoreError::IoError(_)
        )
    }
    
    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            CoreError::Validation(_) => "validation",
            CoreError::DataError(_) => "data",
            CoreError::PredictionError(_) => "prediction",
            CoreError::TradingError(_) => "trading",
            CoreError::StorageError(_) => "storage",
            CoreError::EventError(_) => "event",
            CoreError::ConfigError(_) => "config",
            CoreError::NetworkError(_) => "network",
            CoreError::ModelError(_) => "model",
            CoreError::SerializationError(_) => "serialization",
            CoreError::IoError(_) => "io",
            CoreError::JsonError(_) => "json",
            CoreError::Unknown(_) => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_retryable() {
        let network_err = CoreError::NetworkError("timeout".to_string());
        assert!(network_err.is_retryable());
        
        let validation_err = CoreError::Validation("invalid input".to_string());
        assert!(!validation_err.is_retryable());
    }
    
    #[test]
    fn test_error_categories() {
        let err = CoreError::PredictionError("model failed".to_string());
        assert_eq!(err.category(), "prediction");
    }
}