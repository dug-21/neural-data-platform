//! MCP protocol types for JSON-RPC 2.0 communication.
//!
//! Implements the Model Context Protocol (MCP) JSON-RPC message format
//! as specified in the MCP specification. Designed for HTTP POST transport
//! following ADR-001 (dp-005).
//!
//! # JSON-RPC 2.0 Compliance
//!
//! All messages conform to JSON-RPC 2.0:
//! - `jsonrpc` field is always "2.0"
//! - `id` can be string, number, or null (for notifications)
//! - Errors use standard JSON-RPC error codes
//!
//! # MCP-Specific Extensions
//!
//! - `tools/list` returns tool definitions with input schemas
//! - `tools/call` invokes a tool and returns content array
//! - Tool responses use `isError` flag for error indication

use serde::{Deserialize, Serialize};

// =============================================================================
// JSON-RPC Error Codes (Standard + MCP)
// =============================================================================

/// Standard JSON-RPC 2.0 error codes and MCP-specific extensions.
///
/// # Standard Codes (-32700 to -32600)
/// - Parse error: Invalid JSON
/// - Invalid request: Missing required JSON-RPC fields
/// - Method not found: Unknown method name
/// - Invalid params: Malformed parameters
/// - Internal error: Server-side errors
///
/// # Reference
/// - [JSON-RPC 2.0 Spec](https://www.jsonrpc.org/specification#error_object)
pub struct JsonRpcError;

impl JsonRpcError {
    /// Invalid JSON was received (-32700)
    pub const PARSE_ERROR: i32 = -32700;

    /// The JSON sent is not a valid Request object (-32600)
    pub const INVALID_REQUEST: i32 = -32600;

    /// The method does not exist or is not available (-32601)
    pub const METHOD_NOT_FOUND: i32 = -32601;

    /// Invalid method parameter(s) (-32602)
    pub const INVALID_PARAMS: i32 = -32602;

    /// Internal JSON-RPC error (-32603)
    pub const INTERNAL_ERROR: i32 = -32603;

    /// Server error range start (-32099)
    /// Reserved for implementation-defined server-errors
    pub const SERVER_ERROR_START: i32 = -32099;

    /// Server error range end (-32000)
    pub const SERVER_ERROR_END: i32 = -32000;
}

// =============================================================================
// JSON-RPC Request Types
// =============================================================================

/// MCP JSON-RPC request envelope.
///
/// Represents an incoming MCP request following JSON-RPC 2.0 format.
///
/// # Fields
/// - `jsonrpc`: Must be "2.0"
/// - `id`: Request identifier (string, number, or null for notifications)
/// - `method`: MCP method name (e.g., "tools/list", "tools/call")
/// - `params`: Method-specific parameters (optional)
///
/// # Example
///
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": "req-123",
///   "method": "tools/call",
///   "params": {
///     "name": "list_streams",
///     "arguments": {}
///   }
/// }
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRequest {
    /// JSON-RPC version, must be "2.0"
    pub jsonrpc: String,

    /// Request identifier for correlation.
    /// Can be string, number, or null (for notifications).
    #[serde(default)]
    pub id: Option<serde_json::Value>,

    /// MCP method name (e.g., "tools/list", "tools/call")
    pub method: String,

    /// Method-specific parameters
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Validates the JSON-RPC version field.
    ///
    /// Returns `true` if jsonrpc is "2.0", `false` otherwise.
    pub fn is_valid_version(&self) -> bool {
        self.jsonrpc == "2.0"
    }

    /// Extracts parameters as a typed value.
    ///
    /// Returns `None` if params is missing or deserialization fails.
    pub fn params_as<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        self.params
            .as_ref()
            .and_then(|p| serde_json::from_value(p.clone()).ok())
    }
}

/// Parameters for the `tools/call` method.
///
/// # Fields
/// - `name`: The tool name to invoke
/// - `arguments`: Tool-specific input arguments
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallParams {
    /// Name of the tool to invoke
    pub name: String,

    /// Tool-specific arguments
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

// =============================================================================
// JSON-RPC Response Types
// =============================================================================

/// MCP JSON-RPC response envelope.
///
/// Wraps either a successful result or an error following JSON-RPC 2.0.
///
/// # Success Response
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": "req-123",
///   "result": { ... }
/// }
/// ```
///
/// # Error Response
/// ```json
/// {
///   "jsonrpc": "2.0",
///   "id": "req-123",
///   "error": {
///     "code": -32601,
///     "message": "Method not found"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct McpResponse {
    /// JSON-RPC version, always "2.0"
    pub jsonrpc: String,

    /// Request identifier (mirrors the request id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,

    /// Response payload (result or error)
    #[serde(flatten)]
    pub result: McpResult,
}

impl McpResponse {
    /// JSON-RPC version string
    const JSONRPC_VERSION: &'static str = "2.0";

    /// Creates a successful response with result data.
    ///
    /// # Arguments
    /// * `id` - Request identifier to mirror
    /// * `result` - Success payload to serialize
    pub fn success<T: Serialize>(id: Option<serde_json::Value>, result: T) -> Self {
        Self {
            jsonrpc: Self::JSONRPC_VERSION.to_string(),
            id,
            result: McpResult::Success {
                result: serde_json::to_value(result).unwrap_or(serde_json::Value::Null),
            },
        }
    }

    /// Creates an error response.
    ///
    /// # Arguments
    /// * `id` - Request identifier to mirror (can be None for parse errors)
    /// * `error` - Error details
    pub fn error(id: Option<serde_json::Value>, error: McpRpcError) -> Self {
        Self {
            jsonrpc: Self::JSONRPC_VERSION.to_string(),
            id,
            result: McpResult::Error { error },
        }
    }

    /// Creates a "method not found" error response.
    pub fn method_not_found(id: Option<serde_json::Value>, method: &str) -> Self {
        Self::error(
            id,
            McpRpcError {
                code: JsonRpcError::METHOD_NOT_FOUND,
                message: format!("Method not found: {}", method),
                data: None,
            },
        )
    }

    /// Creates an "invalid params" error response.
    pub fn invalid_params(id: Option<serde_json::Value>, message: impl Into<String>) -> Self {
        Self::error(
            id,
            McpRpcError {
                code: JsonRpcError::INVALID_PARAMS,
                message: message.into(),
                data: None,
            },
        )
    }

    /// Creates an "internal error" response.
    pub fn internal_error(id: Option<serde_json::Value>, message: impl Into<String>) -> Self {
        Self::error(
            id,
            McpRpcError {
                code: JsonRpcError::INTERNAL_ERROR,
                message: message.into(),
                data: None,
            },
        )
    }

    /// Creates a "parse error" response (for malformed JSON).
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::error(
            None,
            McpRpcError {
                code: JsonRpcError::PARSE_ERROR,
                message: message.into(),
                data: None,
            },
        )
    }

    /// Creates an "invalid request" error response.
    pub fn invalid_request(id: Option<serde_json::Value>, message: impl Into<String>) -> Self {
        Self::error(
            id,
            McpRpcError {
                code: JsonRpcError::INVALID_REQUEST,
                message: message.into(),
                data: None,
            },
        )
    }
}

/// MCP response result: either success or error.
///
/// Uses serde's `untagged` enum to produce clean JSON output
/// without type discriminator fields.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum McpResult {
    /// Successful response with result payload
    Success {
        /// The method-specific result data
        result: serde_json::Value,
    },

    /// Error response with structured error
    Error {
        /// Structured error information
        error: McpRpcError,
    },
}

/// JSON-RPC error object.
///
/// Contains error code, message, and optional additional data.
///
/// # Standard Error Codes
/// | Code | Meaning |
/// |------|---------|
/// | -32700 | Parse error |
/// | -32600 | Invalid request |
/// | -32601 | Method not found |
/// | -32602 | Invalid params |
/// | -32603 | Internal error |
#[derive(Debug, Clone, Serialize)]
pub struct McpRpcError {
    /// Numeric error code (see [`JsonRpcError`])
    pub code: i32,

    /// Human-readable error message
    pub message: String,

    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpRpcError {
    /// Creates a new error with code and message.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Creates a new error with additional data.
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

// =============================================================================
// Tool Definition Types (for tools/list)
// =============================================================================

/// Tool definition for `tools/list` response.
///
/// Describes a tool's name, description, and input schema for
/// LLM consumption.
///
/// # Example
///
/// ```json
/// {
///   "name": "list_streams",
///   "description": "List all available Bronze layer streams",
///   "inputSchema": {
///     "type": "object",
///     "properties": {},
///     "required": []
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool identifier
    pub name: String,

    /// Human-readable description for LLM context
    pub description: String,

    /// JSON Schema for tool input validation
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Creates a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }

    /// Creates a tool definition with no input parameters.
    pub fn no_params(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(
            name,
            description,
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }
}

/// Result payload for `tools/list` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListResult {
    /// List of available tools
    pub tools: Vec<ToolDefinition>,
}

// =============================================================================
// Tool Response Types (for tools/call)
// =============================================================================

/// Content item in a tool response.
///
/// MCP tool responses contain an array of content items.
/// Currently only text type is supported for Bronze MCP Server.
///
/// # Example
///
/// ```json
/// {
///   "type": "text",
///   "text": "{\"success\": true, \"data\": {...}}"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    /// Content type identifier (always "text" for this server)
    #[serde(rename = "type")]
    pub content_type: String,

    /// The text content (JSON-encoded for structured data)
    pub text: String,
}

impl ToolContent {
    /// Creates a text content item.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: text.into(),
        }
    }

    /// Creates a text content item from a serializable value.
    ///
    /// Serializes the value to compact JSON.
    pub fn json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self::text(serde_json::to_string(value)?))
    }
}

/// Tool invocation response for `tools/call`.
///
/// Contains content array and optional error flag.
///
/// # Success Response
/// ```json
/// {
///   "content": [
///     {"type": "text", "text": "{\"success\": true, ...}"}
///   ]
/// }
/// ```
///
/// # Error Response
/// ```json
/// {
///   "content": [
///     {"type": "text", "text": "{\"success\": false, \"error\": \"...\"}"}
///   ],
///   "isError": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Content items (typically one text item with JSON)
    pub content: Vec<ToolContent>,

    /// Error flag (present only on errors per MCP spec)
    #[serde(rename = "isError")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResponse {
    /// Creates a successful tool response with text content.
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: None,
        }
    }

    /// Creates a successful tool response from a serializable value.
    ///
    /// The value is serialized to JSON and wrapped in a text content item.
    pub fn success_json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            content: vec![ToolContent::json(value)?],
            is_error: None,
        })
    }

    /// Creates an error tool response.
    ///
    /// Sets `isError: true` per MCP specification.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: Some(true),
        }
    }

    /// Creates an error tool response from a serializable value.
    ///
    /// The value is serialized to JSON and `isError` is set to true.
    pub fn error_json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            content: vec![ToolContent::json(value)?],
            is_error: Some(true),
        })
    }

    /// Checks if this is an error response.
    pub fn is_error(&self) -> bool {
        self.is_error.unwrap_or(false)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -------------------------------------------------------------------------
    // McpRequest Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mcp_request_deserialization() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": "req-123",
            "method": "tools/list",
            "params": {}
        }"#;

        let request: McpRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, Some(json!("req-123")));
        assert_eq!(request.method, "tools/list");
        assert!(request.params.is_some());
    }

    #[test]
    fn test_mcp_request_with_numeric_id() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 42,
            "method": "tools/call",
            "params": {"name": "list_streams"}
        }"#;

        let request: McpRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.id, Some(json!(42)));
        assert_eq!(request.method, "tools/call");
    }

    #[test]
    fn test_mcp_request_with_null_id() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": null,
            "method": "tools/list"
        }"#;

        let request: McpRequest = serde_json::from_str(json).unwrap();

        // Explicit null id in JSON deserializes to None with default serde behavior.
        // This is acceptable for JSON-RPC 2.0 as null id typically indicates a notification.
        // If we need to distinguish "missing id" from "null id", use a custom deserializer.
        assert_eq!(request.id, None);
    }

    #[test]
    fn test_mcp_request_without_id() {
        let json = r#"{
            "jsonrpc": "2.0",
            "method": "tools/list"
        }"#;

        let request: McpRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.id, None);
    }

    #[test]
    fn test_mcp_request_without_params() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": "1",
            "method": "tools/list"
        }"#;

        let request: McpRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.params, None);
    }

    #[test]
    fn test_mcp_request_is_valid_version() {
        let valid = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "tools/list".to_string(),
            params: None,
        };
        assert!(valid.is_valid_version());

        let invalid = McpRequest {
            jsonrpc: "1.0".to_string(),
            id: None,
            method: "tools/list".to_string(),
            params: None,
        };
        assert!(!invalid.is_valid_version());
    }

    #[test]
    fn test_mcp_request_params_as() {
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("1")),
            method: "tools/call".to_string(),
            params: Some(json!({"name": "list_streams", "arguments": {}})),
        };

        let params: Option<ToolCallParams> = request.params_as();
        assert!(params.is_some());

        let params = params.unwrap();
        assert_eq!(params.name, "list_streams");
    }

    // -------------------------------------------------------------------------
    // McpResponse Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mcp_response_success_serialization() {
        let response = McpResponse::success(Some(json!("req-123")), json!({"data": "test"}));

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""jsonrpc":"2.0""#));
        assert!(json.contains(r#""id":"req-123""#));
        assert!(json.contains(r#""result""#));
        assert!(!json.contains(r#""error""#));
    }

    #[test]
    fn test_mcp_response_error_serialization() {
        let response = McpResponse::error(
            Some(json!("req-123")),
            McpRpcError::new(JsonRpcError::METHOD_NOT_FOUND, "Method not found"),
        );

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""jsonrpc":"2.0""#));
        assert!(json.contains(r#""id":"req-123""#));
        assert!(json.contains(r#""error""#));
        assert!(json.contains(r#""code":-32601"#));
        assert!(!json.contains(r#""result""#));
    }

    #[test]
    fn test_mcp_response_method_not_found() {
        let response = McpResponse::method_not_found(Some(json!("1")), "unknown/method");

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("-32601"));
        assert!(json.contains("unknown/method"));
    }

    #[test]
    fn test_mcp_response_invalid_params() {
        let response = McpResponse::invalid_params(Some(json!("1")), "Missing required field");

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("-32602"));
        assert!(json.contains("Missing required field"));
    }

    #[test]
    fn test_mcp_response_internal_error() {
        let response = McpResponse::internal_error(Some(json!("1")), "Database connection failed");

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("-32603"));
        assert!(json.contains("Database connection failed"));
    }

    #[test]
    fn test_mcp_response_parse_error() {
        let response = McpResponse::parse_error("Unexpected token at position 42");

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("-32700"));
        assert!(!json.contains(r#""id""#)); // Parse errors don't have id
    }

    #[test]
    fn test_mcp_response_with_null_id() {
        let response = McpResponse::success(Some(serde_json::Value::Null), json!({}));

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""id":null"#));
    }

    // -------------------------------------------------------------------------
    // McpRpcError Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mcp_rpc_error_new() {
        let error = McpRpcError::new(-32601, "Method not found");

        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
        assert!(error.data.is_none());
    }

    #[test]
    fn test_mcp_rpc_error_with_data() {
        let error = McpRpcError::with_data(-32602, "Invalid params", json!({"field": "stream_id"}));

        assert_eq!(error.code, -32602);
        assert_eq!(error.data, Some(json!({"field": "stream_id"})));
    }

    #[test]
    fn test_mcp_rpc_error_serialization_without_data() {
        let error = McpRpcError::new(-32601, "Method not found");
        let json = serde_json::to_string(&error).unwrap();

        assert!(!json.contains("data"));
    }

    #[test]
    fn test_mcp_rpc_error_serialization_with_data() {
        let error = McpRpcError::with_data(-32602, "Invalid", json!({"hint": "use string"}));
        let json = serde_json::to_string(&error).unwrap();

        assert!(json.contains(r#""data""#));
        assert!(json.contains(r#""hint""#));
    }

    // -------------------------------------------------------------------------
    // JsonRpcError Constants Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_json_rpc_error_codes() {
        assert_eq!(JsonRpcError::PARSE_ERROR, -32700);
        assert_eq!(JsonRpcError::INVALID_REQUEST, -32600);
        assert_eq!(JsonRpcError::METHOD_NOT_FOUND, -32601);
        assert_eq!(JsonRpcError::INVALID_PARAMS, -32602);
        assert_eq!(JsonRpcError::INTERNAL_ERROR, -32603);
        assert_eq!(JsonRpcError::SERVER_ERROR_START, -32099);
        assert_eq!(JsonRpcError::SERVER_ERROR_END, -32000);
    }

    // -------------------------------------------------------------------------
    // ToolDefinition Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tool_definition_serialization() {
        let tool = ToolDefinition::new(
            "list_streams",
            "List all available Bronze layer streams",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        );

        let json = serde_json::to_string(&tool).unwrap();

        assert!(json.contains(r#""name":"list_streams""#));
        assert!(json.contains(r#""inputSchema""#));
    }

    #[test]
    fn test_tool_definition_no_params() {
        let tool = ToolDefinition::no_params("list_streams", "List all streams");

        assert_eq!(tool.input_schema["type"], "object");
        assert_eq!(tool.input_schema["properties"], json!({}));
    }

    #[test]
    fn test_tools_list_result_serialization() {
        let result = ToolsListResult {
            tools: vec![
                ToolDefinition::no_params("list_streams", "List streams"),
                ToolDefinition::new(
                    "sample_data",
                    "Get sample rows",
                    json!({
                        "type": "object",
                        "properties": {
                            "stream_id": {"type": "string"},
                            "n": {"type": "integer", "default": 10}
                        },
                        "required": ["stream_id"]
                    }),
                ),
            ],
        };

        let json = serde_json::to_string(&result).unwrap();

        assert!(json.contains(r#""tools""#));
        assert!(json.contains("list_streams"));
        assert!(json.contains("sample_data"));
    }

    // -------------------------------------------------------------------------
    // ToolContent Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tool_content_text() {
        let content = ToolContent::text("Hello, world!");

        assert_eq!(content.content_type, "text");
        assert_eq!(content.text, "Hello, world!");
    }

    #[test]
    fn test_tool_content_json() {
        #[derive(Serialize)]
        struct Data {
            success: bool,
            count: i32,
        }

        let data = Data {
            success: true,
            count: 42,
        };
        let content = ToolContent::json(&data).unwrap();

        assert_eq!(content.content_type, "text");
        assert!(content.text.contains("success"));
        assert!(content.text.contains("42"));
    }

    #[test]
    fn test_tool_content_serialization() {
        let content = ToolContent::text("test");
        let json = serde_json::to_string(&content).unwrap();

        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"test""#));
    }

    // -------------------------------------------------------------------------
    // ToolResponse Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tool_response_success() {
        let response = ToolResponse::success(r#"{"success": true, "data": "test"}"#);

        assert_eq!(response.content.len(), 1);
        assert_eq!(response.content[0].content_type, "text");
        assert!(!response.is_error());
        assert!(response.is_error.is_none());
    }

    #[test]
    fn test_tool_response_success_json() {
        #[derive(Serialize)]
        struct Result {
            success: bool,
            streams: Vec<String>,
        }

        let result = Result {
            success: true,
            streams: vec!["air-quality".to_string()],
        };
        let response = ToolResponse::success_json(&result).unwrap();

        assert!(!response.is_error());
        assert!(response.content[0].text.contains("air-quality"));
    }

    #[test]
    fn test_tool_response_error() {
        let response = ToolResponse::error(r#"{"success": false, "error": "Not found"}"#);

        assert_eq!(response.content.len(), 1);
        assert!(response.is_error());
        assert_eq!(response.is_error, Some(true));
    }

    #[test]
    fn test_tool_response_error_json() {
        #[derive(Serialize)]
        struct Error {
            success: bool,
            error: String,
        }

        let error = Error {
            success: false,
            error: "Stream not found".to_string(),
        };
        let response = ToolResponse::error_json(&error).unwrap();

        assert!(response.is_error());
        assert!(response.content[0].text.contains("Stream not found"));
    }

    #[test]
    fn test_tool_response_success_serialization_no_is_error() {
        let response = ToolResponse::success("test");
        let json = serde_json::to_string(&response).unwrap();

        // isError should not appear in success responses
        assert!(!json.contains("isError"));
    }

    #[test]
    fn test_tool_response_error_serialization_has_is_error() {
        let response = ToolResponse::error("error message");
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains(r#""isError":true"#));
    }

    // -------------------------------------------------------------------------
    // ToolCallParams Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tool_call_params_deserialization() {
        let json = r#"{
            "name": "sample_data",
            "arguments": {"stream_id": "air-quality", "n": 5}
        }"#;

        let params: ToolCallParams = serde_json::from_str(json).unwrap();

        assert_eq!(params.name, "sample_data");
        assert!(params.arguments.is_some());

        let args = params.arguments.unwrap();
        assert_eq!(args["stream_id"], "air-quality");
        assert_eq!(args["n"], 5);
    }

    #[test]
    fn test_tool_call_params_without_arguments() {
        let json = r#"{"name": "list_streams"}"#;

        let params: ToolCallParams = serde_json::from_str(json).unwrap();

        assert_eq!(params.name, "list_streams");
        assert!(params.arguments.is_none());
    }

    // -------------------------------------------------------------------------
    // Integration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_full_tools_list_flow() {
        // Simulate incoming request
        let request_json = r#"{
            "jsonrpc": "2.0",
            "id": "list-1",
            "method": "tools/list",
            "params": {}
        }"#;

        let request: McpRequest = serde_json::from_str(request_json).unwrap();
        assert!(request.is_valid_version());
        assert_eq!(request.method, "tools/list");

        // Simulate building response
        let tools = ToolsListResult {
            tools: vec![
                ToolDefinition::no_params("list_streams", "List Bronze streams"),
                ToolDefinition::new(
                    "sample_data",
                    "Get sample rows",
                    json!({"type": "object", "properties": {"stream_id": {"type": "string"}}}),
                ),
            ],
        };

        let response = McpResponse::success(request.id.clone(), tools);
        let response_json = serde_json::to_string_pretty(&response).unwrap();

        // Verify response structure
        assert!(response_json.contains(r#""jsonrpc": "2.0""#));
        assert!(response_json.contains(r#""id": "list-1""#));
        assert!(response_json.contains(r#""result""#));
        assert!(response_json.contains(r#""tools""#));
        assert!(response_json.contains("list_streams"));
        assert!(response_json.contains("sample_data"));
    }

    #[test]
    fn test_full_tools_call_flow() {
        // Simulate incoming tools/call request
        let request_json = r#"{
            "jsonrpc": "2.0",
            "id": "call-1",
            "method": "tools/call",
            "params": {
                "name": "sample_data",
                "arguments": {"stream_id": "air-quality", "n": 3}
            }
        }"#;

        let request: McpRequest = serde_json::from_str(request_json).unwrap();
        assert_eq!(request.method, "tools/call");

        let params: ToolCallParams = request.params_as().unwrap();
        assert_eq!(params.name, "sample_data");
        assert_eq!(params.arguments.as_ref().unwrap()["n"], 3);

        // Simulate tool execution result
        #[derive(Serialize)]
        struct SampleResult {
            success: bool,
            stream_id: String,
            row_count: usize,
            rows: Vec<serde_json::Value>,
        }

        let result = SampleResult {
            success: true,
            stream_id: "air-quality".to_string(),
            row_count: 3,
            rows: vec![json!({"timestamp": 123, "data": "sample"})],
        };

        let tool_response = ToolResponse::success_json(&result).unwrap();
        let mcp_response = McpResponse::success(request.id.clone(), tool_response);

        let response_json = serde_json::to_string(&mcp_response).unwrap();

        assert!(response_json.contains(r#""id":"call-1""#));
        assert!(response_json.contains("air-quality"));
        assert!(response_json.contains("row_count"));
    }

    #[test]
    fn test_full_error_flow() {
        // Simulate request for unknown tool
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("err-1")),
            method: "tools/call".to_string(),
            params: Some(json!({"name": "unknown_tool", "arguments": {}})),
        };

        // Build error response
        let tool_error = ToolResponse::error(
            serde_json::to_string(&json!({
                "success": false,
                "error": "Tool 'unknown_tool' not found",
                "code": "UNKNOWN_TOOL"
            }))
            .unwrap(),
        );

        let response = McpResponse::success(request.id.clone(), tool_error);
        let response_json = serde_json::to_string(&response).unwrap();

        assert!(response_json.contains(r#""id":"err-1""#));
        assert!(response_json.contains("isError"));
        assert!(response_json.contains("unknown_tool"));
    }
}
