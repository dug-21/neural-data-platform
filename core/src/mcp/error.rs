//! MCP-specific error types for the Bronze MCP Server.
//!
//! These errors are designed for MCP tool responses and cover configuration
//! store, storage access, and validation failures.
//!
//! # Error Categories
//!
//! - **Configuration Errors**: etcd unavailable, stream not found
//! - **Storage Errors**: Parquet file access issues
//! - **Validation Errors**: Config/data mismatches
//!
//! # Example
//!
//! ```rust
//! use neural_core::mcp::McpError;
//!
//! fn get_stream(id: &str) -> Result<(), McpError> {
//!     if id.is_empty() {
//!         return Err(McpError::InvalidStreamId("Stream ID cannot be empty".into()));
//!     }
//!     // ...
//!     Ok(())
//! }
//! ```

use thiserror::Error;

/// MCP server errors with structured error codes for client handling.
///
/// Each variant maps to a specific error code that can be used in
/// MCP tool responses for programmatic error handling.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum McpError {
    // =========================================================================
    // Configuration Store Errors
    // =========================================================================
    /// etcd is unavailable or connection failed.
    ///
    /// Fail-fast error - the server should not cache stale data.
    #[error("etcd unavailable: {0}")]
    EtcdUnavailable(String),

    /// etcd returned an error during operation.
    #[error("etcd error: {0}")]
    EtcdError(String),

    /// Stream configuration not found in etcd.
    #[error("stream not found: {0}")]
    StreamNotFound(String),

    /// Configuration key not found.
    #[error("config key not found: {0}")]
    ConfigKeyNotFound(String),

    // =========================================================================
    // Validation Errors
    // =========================================================================
    /// Invalid stream ID format.
    ///
    /// Stream IDs must be kebab-case, 1-64 characters, starting with
    /// a lowercase letter.
    #[error("invalid stream ID: {0}")]
    InvalidStreamId(String),

    /// Configuration parsing failed.
    #[error("config parse error: {0}")]
    ConfigParseError(String),

    /// Configuration validation failed.
    #[error("config validation error: {0}")]
    ConfigValidationError(String),

    // =========================================================================
    // Storage Errors
    // =========================================================================
    /// Bronze storage not accessible.
    #[error("storage unavailable: {0}")]
    StorageUnavailable(String),

    /// Parquet file read error.
    #[error("parquet read error: {0}")]
    ParquetReadError(String),

    /// No data files found for stream.
    #[error("no data found for stream: {0}")]
    NoDataFound(String),

    // =========================================================================
    // Internal Errors
    // =========================================================================
    /// Internal server error.
    #[error("internal error: {0}")]
    InternalError(String),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl McpError {
    /// Returns the error code for MCP tool responses.
    ///
    /// These codes allow programmatic error handling by MCP clients.
    pub fn code(&self) -> &'static str {
        match self {
            McpError::EtcdUnavailable(_) => "ETCD_UNAVAILABLE",
            McpError::EtcdError(_) => "ETCD_ERROR",
            McpError::StreamNotFound(_) => "STREAM_NOT_FOUND",
            McpError::ConfigKeyNotFound(_) => "CONFIG_KEY_NOT_FOUND",
            McpError::InvalidStreamId(_) => "INVALID_STREAM_ID",
            McpError::ConfigParseError(_) => "CONFIG_PARSE_ERROR",
            McpError::ConfigValidationError(_) => "CONFIG_VALIDATION_ERROR",
            McpError::StorageUnavailable(_) => "STORAGE_UNAVAILABLE",
            McpError::ParquetReadError(_) => "PARQUET_READ_ERROR",
            McpError::NoDataFound(_) => "NO_DATA_FOUND",
            McpError::InternalError(_) => "INTERNAL_ERROR",
            McpError::SerializationError(_) => "SERIALIZATION_ERROR",
        }
    }

    /// Returns whether this error should trigger a retry.
    ///
    /// Transient errors (like temporary etcd unavailability) may be retried,
    /// while permanent errors (like invalid stream ID) should not.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            McpError::EtcdUnavailable(_) | McpError::EtcdError(_) | McpError::StorageUnavailable(_)
        )
    }

    /// Creates an error response structure for MCP tool responses.
    ///
    /// Returns a JSON-serializable structure with error code and message.
    pub fn to_error_response(&self) -> serde_json::Value {
        serde_json::json!({
            "success": false,
            "error": {
                "code": self.code(),
                "message": self.to_string()
            }
        })
    }
}

// =============================================================================
// Conversions from external error types
// =============================================================================

impl From<serde_json::Error> for McpError {
    fn from(e: serde_json::Error) -> Self {
        McpError::SerializationError(e.to_string())
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(
            McpError::EtcdUnavailable("test".into()).code(),
            "ETCD_UNAVAILABLE"
        );
        assert_eq!(
            McpError::StreamNotFound("air-quality".into()).code(),
            "STREAM_NOT_FOUND"
        );
        assert_eq!(
            McpError::InvalidStreamId("Bad_ID".into()).code(),
            "INVALID_STREAM_ID"
        );
    }

    #[test]
    fn test_error_display() {
        let err = McpError::StreamNotFound("air-quality".into());
        assert_eq!(err.to_string(), "stream not found: air-quality");
    }

    #[test]
    fn test_is_retryable() {
        assert!(McpError::EtcdUnavailable("test".into()).is_retryable());
        assert!(McpError::EtcdError("timeout".into()).is_retryable());
        assert!(McpError::StorageUnavailable("disk full".into()).is_retryable());

        assert!(!McpError::StreamNotFound("air-quality".into()).is_retryable());
        assert!(!McpError::InvalidStreamId("Bad_ID".into()).is_retryable());
    }

    #[test]
    fn test_to_error_response() {
        let err = McpError::StreamNotFound("test-stream".into());
        let response = err.to_error_response();

        assert_eq!(response["success"], false);
        assert_eq!(response["error"]["code"], "STREAM_NOT_FOUND");
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("test-stream"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json")
            .err()
            .unwrap();
        let mcp_err: McpError = json_err.into();

        assert!(matches!(mcp_err, McpError::SerializationError(_)));
    }

    #[test]
    fn test_error_equality() {
        let err1 = McpError::StreamNotFound("test".into());
        let err2 = McpError::StreamNotFound("test".into());
        let err3 = McpError::StreamNotFound("other".into());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
