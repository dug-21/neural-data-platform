//! MCP Protocol Types
//!
//! JSON-RPC 2.0 and MCP-specific type definitions for the Bronze layer server.
//!
//! # JSON-RPC 2.0 Specification
//!
//! All MCP communication follows JSON-RPC 2.0:
//! - Request: `{"jsonrpc": "2.0", "id": "...", "method": "...", "params": {...}}`
//! - Response: `{"jsonrpc": "2.0", "id": "...", "result": {...}}` or `{"error": {...}}`
//!
//! # MCP Content Format
//!
//! Tool results use MCP content format:
//! ```json
//! {
//!   "content": [{"type": "text", "text": "{...}"}],
//!   "isError": false
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

// =============================================================================
// JSON-RPC 2.0 Types
// =============================================================================

/// JSON-RPC 2.0 request structure.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    /// Protocol version, must be "2.0"
    pub jsonrpc: String,

    /// Request identifier (optional for notifications)
    #[serde(default)]
    pub id: Option<Value>,

    /// Method name to invoke
    pub method: String,

    /// Method parameters (optional)
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response structure.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    /// Protocol version, always "2.0"
    pub jsonrpc: String,

    /// Echoed from request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,

    /// Success result (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// Error (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code (see JSON-RPC spec and MCP extensions)
    pub code: i32,

    /// Human-readable error message
    pub message: String,

    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Create an error response with additional data.
    pub fn error_with_data(
        id: Option<Value>,
        code: i32,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }
}

// =============================================================================
// JSON-RPC Error Codes
// =============================================================================

/// Standard JSON-RPC 2.0 error codes.
pub mod error_codes {
    /// Invalid JSON was received by the server
    pub const PARSE_ERROR: i32 = -32700;

    /// The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;

    /// The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;

    /// Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;

    /// Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    // MCP-specific error codes (-32000 to -32099 reserved for implementation)

    /// etcd unavailable
    pub const ETCD_UNAVAILABLE: i32 = -32000;

    /// Storage/Parquet error
    pub const STORAGE_ERROR: i32 = -32001;

    /// Stream not found
    pub const STREAM_NOT_FOUND: i32 = -32002;

    /// No data available for stream
    pub const NO_DATA_AVAILABLE: i32 = -32003;
}

// =============================================================================
// MCP-Specific Types
// =============================================================================

/// MCP content block in tool responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    /// Content type (always "text" for now)
    #[serde(rename = "type")]
    pub content_type: String,

    /// Text content (JSON-encoded tool result)
    pub text: String,
}

impl McpContent {
    /// Create a text content block.
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: content.into(),
        }
    }

    /// Create a text content block from a serializable value.
    pub fn json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            content_type: "text".to_string(),
            text: serde_json::to_string(value)?,
        })
    }
}

/// MCP tool result structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Content blocks
    pub content: Vec<McpContent>,

    /// Whether this is an error response
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl McpToolResult {
    /// Create a success result with JSON content.
    pub fn success<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            content: vec![McpContent::json(value)?],
            is_error: None,
        })
    }

    /// Create an error result.
    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        let error_obj = serde_json::json!({
            "success": false,
            "error": message.into(),
            "code": code.into()
        });
        Self {
            content: vec![McpContent::text(error_obj.to_string())],
            is_error: Some(true),
        }
    }

    /// Create an error result with additional details.
    pub fn error_with_details(
        message: impl Into<String>,
        code: impl Into<String>,
        details: Value,
    ) -> Self {
        let error_obj = serde_json::json!({
            "success": false,
            "error": message.into(),
            "code": code.into(),
            "details": details
        });
        Self {
            content: vec![McpContent::text(error_obj.to_string())],
            is_error: Some(true),
        }
    }
}

// =============================================================================
// Tool Definition Types
// =============================================================================

/// MCP tool definition for tools/list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g., "list_streams")
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// JSON Schema for input parameters
    #[serde(rename = "inputSchema")]
    pub input_schema: ToolInputSchema,
}

/// JSON Schema for tool input parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInputSchema {
    /// Always "object"
    #[serde(rename = "type")]
    pub schema_type: String,

    /// Property definitions
    pub properties: Value,

    /// Required property names
    pub required: Vec<String>,

    /// Whether additional properties are allowed
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<bool>,
}

impl ToolInputSchema {
    /// Create an empty input schema (no parameters).
    pub fn empty() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: serde_json::json!({}),
            required: vec![],
            additional_properties: Some(false),
        }
    }

    /// Create an input schema with given properties.
    pub fn with_properties(properties: Value, required: Vec<String>) -> Self {
        Self {
            schema_type: "object".to_string(),
            properties,
            required,
            additional_properties: Some(false),
        }
    }
}

// =============================================================================
// Initialize Response Types
// =============================================================================

/// Server information for initialize response.
#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,
}

/// Server capabilities for initialize response.
#[derive(Debug, Clone, Serialize)]
pub struct ServerCapabilities {
    /// Tools capability (empty object = tools supported)
    pub tools: Value,
}

/// Initialize response payload.
#[derive(Debug, Clone, Serialize)]
pub struct InitializeResult {
    /// Protocol version supported
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,

    /// Server information
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,

    /// Server capabilities
    pub capabilities: ServerCapabilities,
}

impl Default for InitializeResult {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".to_string(),
            server_info: ServerInfo {
                name: "ndp-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ServerCapabilities {
                tools: serde_json::json!({}),
            },
        }
    }
}

// =============================================================================
// Tools List Response Types
// =============================================================================

/// tools/list response payload.
#[derive(Debug, Clone, Serialize)]
pub struct ToolsListResult {
    /// Available tools
    pub tools: Vec<ToolDefinition>,
}

// =============================================================================
// Tools Call Request Types
// =============================================================================

/// tools/call request parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsCallParams {
    /// Tool name to invoke
    pub name: String,

    /// Tool arguments
    #[serde(default)]
    pub arguments: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse::success(
            Some(serde_json::json!(1)),
            serde_json::json!({"test": "value"}),
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse::error(
            Some(serde_json::json!(1)),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn test_mcp_tool_result_success() {
        #[derive(Serialize)]
        struct TestResult {
            success: bool,
            value: i32,
        }

        let result = McpToolResult::success(&TestResult {
            success: true,
            value: 42,
        })
        .unwrap();

        assert!(result.is_error.is_none());
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].content_type, "text");
    }

    #[test]
    fn test_mcp_tool_result_error() {
        let result = McpToolResult::error("Stream not found", "STREAM_NOT_FOUND");
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("STREAM_NOT_FOUND"));
    }

    #[test]
    fn test_tool_input_schema_empty() {
        let schema = ToolInputSchema::empty();
        assert_eq!(schema.schema_type, "object");
        assert!(schema.required.is_empty());
    }
}
