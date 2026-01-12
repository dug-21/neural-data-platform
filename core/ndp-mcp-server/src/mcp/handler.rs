//! MCP Request Handler
//!
//! Routes incoming MCP JSON-RPC requests to appropriate method handlers
//! and tool implementations.

use std::sync::Arc;

use serde_json::Value;
use tracing::{debug, warn};

use crate::error::{McpError, McpResult};
use crate::etcd::ConfigStore;
use crate::storage::BronzeStorage;

use super::protocol::{
    error_codes, InitializeResult, JsonRpcRequest, JsonRpcResponse, McpToolResult, ToolDefinition,
    ToolInputSchema, ToolsCallParams, ToolsListResult,
};
use super::tools;

/// MCP request handler with storage and config dependencies.
pub struct McpHandler<S, C>
where
    S: BronzeStorage + Send + Sync,
    C: ConfigStore + Send + Sync,
{
    /// Bronze layer storage
    storage: Arc<S>,

    /// Configuration store (etcd)
    config_store: Arc<C>,
}

impl<S, C> McpHandler<S, C>
where
    S: BronzeStorage + Send + Sync,
    C: ConfigStore + Send + Sync,
{
    /// Create a new MCP handler with the given dependencies.
    pub fn new(storage: Arc<S>, config_store: Arc<C>) -> Self {
        Self {
            storage,
            config_store,
        }
    }

    /// Handle an MCP JSON-RPC request.
    ///
    /// Routes to the appropriate method handler based on the method name.
    pub async fn handle(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!(
            method = %request.method,
            id = ?request.id,
            "Handling MCP request"
        );

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id).await,
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,
            "notifications/initialized" => {
                // Notification, no response needed but we return success for HTTP
                JsonRpcResponse::success(request.id, serde_json::json!({}))
            }
            _ => {
                warn!(method = %request.method, "Unknown method");
                JsonRpcResponse::error(
                    request.id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("Method not found: {}", request.method),
                )
            }
        }
    }

    /// Handle initialize request.
    async fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        let result = InitializeResult::default();
        match serde_json::to_value(result) {
            Ok(value) => JsonRpcResponse::success(id, value),
            Err(e) => JsonRpcResponse::error(
                id,
                error_codes::INTERNAL_ERROR,
                format!("Serialization error: {}", e),
            ),
        }
    }

    /// Handle tools/list request.
    async fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let tools = vec![
            ToolDefinition {
                name: "list_streams".to_string(),
                description: "List all available Bronze layer streams with metadata".to_string(),
                input_schema: ToolInputSchema::empty(),
            },
            ToolDefinition {
                name: "describe_schema".to_string(),
                description: "Get schema information for a stream. Modes: source (raw_payload structure + field mappings), target (entity_schemas), all (complete ETL picture with gap analysis)".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "stream_id": {
                            "type": "string",
                            "description": "The stream identifier (e.g., 'air-quality', 'outdoor-weather')"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["all", "source", "target"],
                            "description": "Schema view mode (default: all)",
                            "default": "all"
                        }
                    }),
                    vec!["stream_id".to_string()],
                ),
            },
            ToolDefinition {
                name: "validate_config".to_string(),
                description: "Compare stream configuration in etcd against actual Bronze Parquet schema. Detects mismatches, missing fields, and extra fields.".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "stream_id": {
                            "type": "string",
                            "description": "The stream identifier to validate"
                        }
                    }),
                    vec!["stream_id".to_string()],
                ),
            },
            ToolDefinition {
                name: "sample_data".to_string(),
                description: "Retrieve sample rows from a Bronze stream for exploration".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "stream_id": {
                            "type": "string",
                            "description": "The stream identifier"
                        },
                        "n": {
                            "type": "integer",
                            "description": "Number of rows to return (default: 10, max: 100)",
                            "default": 10,
                            "minimum": 1,
                            "maximum": 100
                        }
                    }),
                    vec!["stream_id".to_string()],
                ),
            },
        ];

        let result = ToolsListResult { tools };
        match serde_json::to_value(result) {
            Ok(value) => JsonRpcResponse::success(id, value),
            Err(e) => JsonRpcResponse::error(
                id,
                error_codes::INTERNAL_ERROR,
                format!("Serialization error: {}", e),
            ),
        }
    }

    /// Handle tools/call request.
    async fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        // Parse the params
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing params");
            }
        };

        let call_params: ToolsCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        debug!(tool = %call_params.name, "Executing tool");

        // Route to the appropriate tool handler
        let result = match call_params.name.as_str() {
            "list_streams" => self.execute_list_streams().await,
            "describe_schema" => self.execute_describe_schema(call_params.arguments).await,
            "validate_config" => self.execute_validate_config(call_params.arguments).await,
            "sample_data" => self.execute_sample_data(call_params.arguments).await,
            _ => {
                warn!(tool = %call_params.name, "Unknown tool");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Unknown tool: {}", call_params.name),
                );
            }
        };

        match result {
            Ok(tool_result) => match serde_json::to_value(tool_result) {
                Ok(value) => JsonRpcResponse::success(id, value),
                Err(e) => JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Serialization error: {}", e),
                ),
            },
            Err(e) => {
                let (code, mcp_code) = match &e {
                    McpError::StreamNotFound(_) => {
                        (error_codes::STREAM_NOT_FOUND, "STREAM_NOT_FOUND")
                    }
                    McpError::EtcdUnavailable(_) => {
                        (error_codes::ETCD_UNAVAILABLE, "ETCD_UNAVAILABLE")
                    }
                    McpError::StorageError(_) => (error_codes::STORAGE_ERROR, "STORAGE_ERROR"),
                    McpError::InvalidRequest(_) => {
                        (error_codes::INVALID_PARAMS, "INVALID_PARAMETER")
                    }
                    _ => (error_codes::INTERNAL_ERROR, "INTERNAL_ERROR"),
                };

                let result = McpToolResult::error(e.to_string(), mcp_code);
                match serde_json::to_value(result) {
                    Ok(value) => JsonRpcResponse::success(id, value),
                    Err(ser_err) => JsonRpcResponse::error(id, code, format!("Error: {}", ser_err)),
                }
            }
        }
    }

    /// Execute list_streams tool.
    async fn execute_list_streams(&self) -> McpResult<McpToolResult> {
        let result =
            tools::list_streams::execute(self.storage.as_ref(), self.config_store.as_ref()).await?;
        Ok(result)
    }

    /// Execute describe_schema tool.
    async fn execute_describe_schema(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        let result = tools::describe_schema::execute(
            self.storage.as_ref(),
            self.config_store.as_ref(),
            args,
        )
        .await?;
        Ok(result)
    }

    /// Execute validate_config tool.
    async fn execute_validate_config(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        let result = tools::validate_config::execute(
            self.storage.as_ref(),
            self.config_store.as_ref(),
            args,
        )
        .await?;
        Ok(result)
    }

    /// Execute sample_data tool.
    async fn execute_sample_data(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        let result = tools::sample_data::execute(self.storage.as_ref(), args).await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etcd::MockConfigStore;
    use crate::storage::MockBronzeStorage;

    fn create_test_handler() -> McpHandler<MockBronzeStorage, MockConfigStore> {
        let storage = Arc::new(MockBronzeStorage::new());
        let config_store = Arc::new(MockConfigStore::new());
        McpHandler::new(storage, config_store)
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: None,
        };

        let response = handler.handle(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_handle_tools_list() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(request).await;
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 4);
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "unknown/method".to_string(),
            params: None,
        };

        let response = handler.handle(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }
}
