//! Error types for the configuration store

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ConfigError {
    #[error("Configuration key not found: {0}")]
    NotFound(String),

    #[error("Invalid configuration path: {0}")]
    InvalidPath(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Custom error: {0}")]
    Custom(String),
}