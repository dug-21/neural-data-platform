//! NDP library error types

use thiserror::Error;

/// Errors produced by ndp-lib operations.
#[derive(Error, Debug)]
pub enum NdpLibError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Config not found: {path}")]
    ConfigNotFound { path: String },

    #[error("Config parse error: {message}")]
    ConfigParse { message: String },

    #[error("Sync failed for {entity}: {reason}")]
    SyncFailed { entity: String, reason: String },

    #[error("CSV error: {0}")]
    Csv(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<tokio_postgres::Error> for NdpLibError {
    fn from(e: tokio_postgres::Error) -> Self {
        NdpLibError::Database(e.to_string())
    }
}

impl From<serde_json::Error> for NdpLibError {
    fn from(e: serde_json::Error) -> Self {
        NdpLibError::ConfigParse {
            message: e.to_string(),
        }
    }
}

impl From<csv::Error> for NdpLibError {
    fn from(e: csv::Error) -> Self {
        NdpLibError::Csv(e.to_string())
    }
}

/// Convenience Result type for ndp-lib operations.
pub type Result<T> = std::result::Result<T, NdpLibError>;
