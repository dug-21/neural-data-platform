//! HTTP server setup using axum framework.
//!
//! Implements the HTTP transport layer for MCP protocol as defined in ADR-001.
//! Routes:
//! - POST /mcp: MCP JSON-RPC endpoint
//! - GET /health: Health check endpoint

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;

/// Application state shared across all request handlers.
///
/// Contains references to configuration and storage/config clients.
/// Wrapped in Arc for thread-safe sharing.
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: AppConfig,
    // TODO: Add ConfigStore (etcd client) in Phase 1
    // pub config_store: ConfigStore,
    // TODO: Add BronzeStorage in Phase 1
    // pub storage: Arc<dyn BronzeStorage>,
}

impl AppState {
    /// Create new application state.
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}

/// Create the axum router with all routes configured.
///
/// # Routes
///
/// - `POST /mcp`: MCP JSON-RPC protocol endpoint
/// - `GET /health`: Health check endpoint
///
/// # Middleware
///
/// - TraceLayer: Request/response tracing for observability
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/mcp", post(mcp_handler))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// =============================================================================
// MCP Protocol Types
// =============================================================================

/// JSON-RPC 2.0 request structure for MCP protocol.
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Request ID (optional for notifications)
    pub id: Option<serde_json::Value>,
    /// Method name (e.g., "initialize", "tools/list", "tools/call")
    pub method: String,
    /// Method parameters (optional)
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response structure for MCP protocol.
#[derive(Debug, Serialize)]
pub struct McpResponse {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Echoed from request
    pub id: Option<serde_json::Value>,
    /// Result (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpResponse {
    /// Create a success response.
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

// =============================================================================
// MCP Protocol Handler
// =============================================================================

/// MCP JSON-RPC endpoint handler.
///
/// Routes MCP methods to appropriate handlers:
/// - `initialize`: Return server capabilities
/// - `tools/list`: Return available tools
/// - `tools/call`: Execute a tool
///
/// Returns JSON-RPC 2.0 formatted responses.
async fn mcp_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    tracing::debug!(
        method = %request.method,
        id = ?request.id,
        "MCP request received"
    );

    let response = match request.method.as_str() {
        "initialize" => handle_initialize(&state, &request).await,
        "tools/list" => handle_tools_list(&state, &request).await,
        "tools/call" => handle_tools_call(&state, &request).await,
        _ => McpResponse::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        ),
    };

    Json(response)
}

/// Handle MCP initialize request.
///
/// Returns server capabilities and protocol version.
async fn handle_initialize(
    _state: &AppState,
    request: &McpRequest,
) -> McpResponse {
    let capabilities = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "ndp-mcp-server",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
            "tools": {}
        }
    });

    McpResponse::success(request.id.clone(), capabilities)
}

/// Handle MCP tools/list request.
///
/// Returns list of available tools with their schemas.
async fn handle_tools_list(
    _state: &AppState,
    request: &McpRequest,
) -> McpResponse {
    // Tool definitions following MCP specification
    let tools = serde_json::json!({
        "tools": [
            {
                "name": "list_streams",
                "description": "List all Bronze layer streams with metadata",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "describe_schema",
                "description": "Get schema information for a stream",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "stream_id": {
                            "type": "string",
                            "description": "Stream identifier"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["source", "target", "all"],
                            "default": "all",
                            "description": "Schema view mode"
                        }
                    },
                    "required": ["stream_id"]
                }
            },
            {
                "name": "validate_config",
                "description": "Validate stream configuration against actual data",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "stream_id": {
                            "type": "string",
                            "description": "Stream identifier"
                        }
                    },
                    "required": ["stream_id"]
                }
            },
            {
                "name": "sample_data",
                "description": "Get sample rows from a stream",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "stream_id": {
                            "type": "string",
                            "description": "Stream identifier"
                        },
                        "n": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 100,
                            "default": 10,
                            "description": "Number of rows to return"
                        }
                    },
                    "required": ["stream_id"]
                }
            }
        ]
    });

    McpResponse::success(request.id.clone(), tools)
}

/// Handle MCP tools/call request.
///
/// Routes to specific tool implementations.
async fn handle_tools_call(
    _state: &AppState,
    request: &McpRequest,
) -> McpResponse {
    let params = match &request.params {
        Some(p) => p,
        None => {
            return McpResponse::error(
                request.id.clone(),
                -32602,
                "Missing params".to_string(),
            );
        }
    };

    let tool_name = params
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("");

    // Tool implementations will be added in subsequent phases
    // For now, return placeholder responses
    let result = match tool_name {
        "list_streams" => {
            // TODO: Implement in Phase 1
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": "{\"streams\": [], \"note\": \"Tool implementation pending\"}"
                }]
            })
        }
        "describe_schema" => {
            // TODO: Implement in Phase 2
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": "{\"error\": \"Not implemented yet\"}"
                }],
                "isError": true
            })
        }
        "validate_config" => {
            // TODO: Implement in Phase 3
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": "{\"error\": \"Not implemented yet\"}"
                }],
                "isError": true
            })
        }
        "sample_data" => {
            // TODO: Implement in Phase 1
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": "{\"rows\": [], \"note\": \"Tool implementation pending\"}"
                }]
            })
        }
        _ => {
            return McpResponse::error(
                request.id.clone(),
                -32602,
                format!("Unknown tool: {}", tool_name),
            );
        }
    };

    McpResponse::success(request.id.clone(), result)
}

// =============================================================================
// Health Check
// =============================================================================

/// Health check response structure.
#[derive(Serialize)]
pub struct HealthResponse {
    /// Server health status
    pub status: String,
    /// Server version
    pub version: String,
    /// Component health (optional details)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<HealthComponents>,
}

/// Component health details.
#[derive(Serialize)]
pub struct HealthComponents {
    /// etcd connection status
    pub etcd: ComponentStatus,
    /// Storage layer status
    pub storage: ComponentStatus,
}

/// Individual component status.
#[derive(Serialize)]
pub struct ComponentStatus {
    /// Whether component is healthy
    pub healthy: bool,
    /// Status message
    pub message: String,
}

/// Health check endpoint handler.
///
/// Returns server health status with version information.
/// Used by load balancers and orchestrators for health monitoring.
async fn health_check(
    State(_state): State<Arc<AppState>>,
) -> (StatusCode, Json<HealthResponse>) {
    // TODO: Add actual component health checks in Phase 4
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        components: None, // Will be populated when etcd/storage clients are added
    };

    (StatusCode::OK, Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_response_success() {
        let response = McpResponse::success(
            Some(serde_json::json!(1)),
            serde_json::json!({"test": "value"}),
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_mcp_response_error() {
        let response = McpResponse::error(
            Some(serde_json::json!(1)),
            -32601,
            "Method not found".to_string(),
        );
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, -32601);
    }
}
