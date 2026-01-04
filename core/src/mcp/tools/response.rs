//! MCP Tool Response Helpers (dp-005)
//!
//! Provides consistent response formatting following ADR-005.
//! All tool responses use the MCP content format with success/error flags.

use crate::mcp::{McpRpcError, ToolResponse, JsonRpcError};
use serde::Serialize;
use serde_json::Value;

/// Result type for tool operations
pub type ToolResult = Result<Value, McpRpcError>;

/// Inner response structure for tool data
#[derive(Debug, Clone, Serialize)]
pub struct SuccessResponse<T: Serialize> {
    pub success: bool,
    #[serde(flatten)]
    pub data: T,
}

/// Inner response structure for tool errors
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// Create a successful tool response with data
///
/// # Arguments
/// * `data` - The response data to serialize
///
/// # Returns
/// A JSON Value containing the MCP ToolResponse structure
///
/// # Example
///
/// ```ignore
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct ListResult { streams: Vec<StreamInfo> }
///
/// let result = ListResult { streams: vec![...] };
/// let response = create_tool_response(result)?;
/// ```
pub fn create_tool_response<T: Serialize>(data: T) -> ToolResult {
    let success_data = SuccessResponse {
        success: true,
        data,
    };

    let json_text = serde_json::to_string(&success_data)
        .map_err(|e| McpRpcError::new(JsonRpcError::INTERNAL_ERROR, format!("Serialization error: {}", e)))?;

    let tool_response = ToolResponse::success(json_text);

    serde_json::to_value(tool_response)
        .map_err(|e| McpRpcError::new(JsonRpcError::INTERNAL_ERROR, format!("Response encoding error: {}", e)))
}

/// Create an error tool response
///
/// # Arguments
/// * `code` - Error code (e.g., "STREAM_NOT_FOUND")
/// * `message` - Human-readable error message
/// * `details` - Optional additional context
///
/// # Returns
/// A JSON Value containing the MCP ToolResponse with isError flag
pub fn create_error_response(
    code: &str,
    message: &str,
    details: Option<Value>,
) -> ToolResult {
    let error_data = ErrorResponse {
        success: false,
        error: message.to_string(),
        code: code.to_string(),
        details,
    };

    let json_text = serde_json::to_string(&error_data)
        .map_err(|e| McpRpcError::new(JsonRpcError::INTERNAL_ERROR, format!("Serialization error: {}", e)))?;

    let tool_response = ToolResponse::error(json_text);

    serde_json::to_value(tool_response)
        .map_err(|e| McpRpcError::new(JsonRpcError::INTERNAL_ERROR, format!("Response encoding error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize)]
    struct TestData {
        name: String,
        count: i32,
    }

    #[test]
    fn test_create_tool_response_success() {
        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        let result = create_tool_response(data).unwrap();

        // Verify it's a ToolResponse
        let response: ToolResponse = serde_json::from_value(result).unwrap();
        assert!(!response.is_error());
        assert_eq!(response.content.len(), 1);

        // Verify inner content
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert_eq!(inner["success"], true);
        assert_eq!(inner["name"], "test");
        assert_eq!(inner["count"], 42);
    }

    #[test]
    fn test_create_error_response() {
        let result = create_error_response(
            "STREAM_NOT_FOUND",
            "Stream not found: invalid-stream",
            Some(serde_json::json!({"stream_id": "invalid-stream"})),
        ).unwrap();

        let response: ToolResponse = serde_json::from_value(result).unwrap();
        assert!(response.is_error());
        assert_eq!(response.is_error, Some(true));

        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert_eq!(inner["success"], false);
        assert_eq!(inner["code"], "STREAM_NOT_FOUND");
        assert!(inner["error"].as_str().unwrap().contains("invalid-stream"));
    }

    #[test]
    fn test_create_error_response_without_details() {
        let result = create_error_response(
            "INTERNAL_ERROR",
            "Something went wrong",
            None,
        ).unwrap();

        let response: ToolResponse = serde_json::from_value(result).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert!(inner.get("details").is_none());
    }

    #[test]
    fn test_success_response_serialization() {
        #[derive(Serialize)]
        struct Streams {
            streams: Vec<String>,
        }

        let data = Streams {
            streams: vec!["air-quality".to_string()],
        };

        let success = SuccessResponse {
            success: true,
            data,
        };

        let json = serde_json::to_string(&success).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("air-quality"));
    }
}
