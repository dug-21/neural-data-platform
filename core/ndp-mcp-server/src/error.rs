//! Error types for the Bronze MCP Server.
//!
//! Uses structured errors with thiserror for proper error propagation
//! and MCP-compliant error responses.

use thiserror::Error;

/// MCP Server error types.
///
/// These errors map to JSON-RPC error codes and MCP tool error responses.
/// Follows the NDP error handling pattern (AIR-001) with structured errors.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum McpError {
    /// Configuration error (missing env vars, invalid config)
    #[error("Configuration error: {0}")]
    Config(String),

    /// etcd connection or query error
    #[error("etcd unavailable: {0}")]
    EtcdUnavailable(String),

    /// Storage/Parquet read error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Stream not found in configuration or storage
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    /// Parse error (JSON, Parquet schema, etc.)
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Invalid parameters (alias for InvalidRequest for backward compatibility)
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl McpError {
    /// Convert to JSON-RPC error code.
    ///
    /// Standard JSON-RPC 2.0 error codes:
    /// - -32700: Parse error
    /// - -32600: Invalid Request
    /// - -32601: Method not found
    /// - -32602: Invalid params
    /// - -32603: Internal error
    /// - -32000 to -32099: Server error (reserved for implementation-defined)
    pub fn json_rpc_code(&self) -> i32 {
        match self {
            McpError::Config(_) => -32603,
            McpError::EtcdUnavailable(_) => -32000,
            McpError::StorageError(_) => -32001,
            McpError::StreamNotFound(_) => -32002,
            McpError::ParseError(_) => -32700,
            McpError::InvalidRequest(_) | McpError::InvalidParams(_) => -32602,
            McpError::Internal(_) => -32603,
        }
    }

    /// Convert to MCP error code string.
    ///
    /// These codes are used in MCP tool responses with isError: true.
    pub fn mcp_error_code(&self) -> &'static str {
        match self {
            McpError::Config(_) => "CONFIG_ERROR",
            McpError::EtcdUnavailable(_) => "ETCD_UNAVAILABLE",
            McpError::StorageError(_) => "STORAGE_ERROR",
            McpError::StreamNotFound(_) => "STREAM_NOT_FOUND",
            McpError::ParseError(_) => "PARSE_ERROR",
            McpError::InvalidRequest(_) | McpError::InvalidParams(_) => "INVALID_PARAMS",
            McpError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

// Implement From traits for common error types
impl From<std::io::Error> for McpError {
    fn from(err: std::io::Error) -> Self {
        McpError::StorageError(err.to_string())
    }
}

impl From<serde_json::Error> for McpError {
    fn from(err: serde_json::Error) -> Self {
        McpError::ParseError(err.to_string())
    }
}

/// Result type alias for MCP operations.
pub type McpResult<T> = Result<T, McpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_not_found_error_codes() {
        let err = McpError::StreamNotFound("test-stream".to_string());
        assert_eq!(err.json_rpc_code(), -32002);
        assert_eq!(err.mcp_error_code(), "STREAM_NOT_FOUND");
    }

    #[test]
    fn test_etcd_unavailable_error_codes() {
        let err = McpError::EtcdUnavailable("connection refused".to_string());
        assert_eq!(err.json_rpc_code(), -32000);
        assert_eq!(err.mcp_error_code(), "ETCD_UNAVAILABLE");
    }

    #[test]
    fn test_storage_error_codes() {
        let err = McpError::StorageError("file not found".to_string());
        assert_eq!(err.json_rpc_code(), -32001);
        assert_eq!(err.mcp_error_code(), "STORAGE_ERROR");
    }

    #[test]
    fn test_parse_error_codes() {
        let err = McpError::ParseError("invalid JSON".to_string());
        assert_eq!(err.json_rpc_code(), -32700);
        assert_eq!(err.mcp_error_code(), "PARSE_ERROR");
    }

    #[test]
    fn test_invalid_request_error_codes() {
        let err = McpError::InvalidRequest("missing stream_id".to_string());
        assert_eq!(err.json_rpc_code(), -32602);
        // InvalidRequest and InvalidParams both map to INVALID_PARAMS for MCP compatibility
        assert_eq!(err.mcp_error_code(), "INVALID_PARAMS");
    }

    #[test]
    fn test_config_error_display() {
        let err = McpError::Config("missing NDP_RAW_PATH".to_string());
        assert_eq!(
            err.to_string(),
            "Configuration error: missing NDP_RAW_PATH"
        );
    }

    #[test]
    fn test_etcd_error_display() {
        let err = McpError::EtcdUnavailable("connection timeout".to_string());
        assert_eq!(err.to_string(), "etcd unavailable: connection timeout");
    }

    #[test]
    fn test_storage_error_display() {
        let err = McpError::StorageError("parquet read failed".to_string());
        assert_eq!(err.to_string(), "Storage error: parquet read failed");
    }

    #[test]
    fn test_error_equality() {
        let err1 = McpError::StreamNotFound("stream-a".to_string());
        let err2 = McpError::StreamNotFound("stream-a".to_string());
        let err3 = McpError::StreamNotFound("stream-b".to_string());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_error_clone() {
        let err1 = McpError::StorageError("original".to_string());
        let err2 = err1.clone();

        assert_eq!(err1, err2);
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let mcp_err: McpError = io_err.into();

        assert!(matches!(mcp_err, McpError::StorageError(_)));
        assert!(mcp_err.to_string().contains("file not found"));
    }

    #[test]
    fn test_from_serde_error() {
        let json_result: Result<serde_json::Value, _> = serde_json::from_str("invalid json");
        let json_err = json_result.unwrap_err();
        let mcp_err: McpError = json_err.into();

        assert!(matches!(mcp_err, McpError::ParseError(_)));
    }

    #[test]
    fn test_invalid_params_error_codes() {
        let err = McpError::InvalidParams("missing stream_id".to_string());
        assert_eq!(err.json_rpc_code(), -32602);
        assert_eq!(err.mcp_error_code(), "INVALID_PARAMS");
    }

    #[test]
    fn test_all_error_variants_have_codes() {
        // Exhaustive test to ensure all variants have valid codes
        let errors = vec![
            McpError::Config("test".to_string()),
            McpError::EtcdUnavailable("test".to_string()),
            McpError::StorageError("test".to_string()),
            McpError::StreamNotFound("test".to_string()),
            McpError::ParseError("test".to_string()),
            McpError::InvalidRequest("test".to_string()),
            McpError::InvalidParams("test".to_string()),
            McpError::Internal("test".to_string()),
        ];

        for err in errors {
            // All JSON-RPC codes should be negative
            assert!(err.json_rpc_code() < 0);
            // All MCP codes should be non-empty
            assert!(!err.mcp_error_code().is_empty());
            // All errors should have display output
            assert!(!err.to_string().is_empty());
        }
    }
}
