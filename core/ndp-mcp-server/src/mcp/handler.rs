//! MCP Request Handler
//!
//! Routes incoming MCP JSON-RPC requests to appropriate method handlers
//! and tool implementations.

use std::sync::Arc;

use serde_json::Value;
use tracing::{debug, warn};

use crate::error::{McpError, McpResult};
use crate::etcd::ConfigStore;
use crate::storage::{BronzeStorage, DictionaryStore, EtlRunStore, SilverStorage};

use super::protocol::{
    error_codes, InitializeResult, JsonRpcRequest, JsonRpcResponse, McpToolResult, ToolDefinition,
    ToolInputSchema, ToolsCallParams, ToolsListResult,
};
use super::tools;

/// MCP request handler with storage and config dependencies.
///
/// Supports Bronze, Silver, Dictionary, and ETL storage layers following
/// the Domain Adapter pattern (ADR-002).
pub struct McpHandler<B, C, S, D, E>
where
    B: BronzeStorage + Send + Sync,
    C: ConfigStore + Send + Sync,
    S: SilverStorage + Send + Sync,
    D: DictionaryStore + Send + Sync,
    E: EtlRunStore + Send + Sync,
{
    /// Bronze layer storage
    storage: Arc<B>,

    /// Configuration store (etcd)
    config_store: Arc<C>,

    /// Silver layer storage
    silver_storage: Arc<S>,

    /// Data dictionary store
    dictionary_store: Arc<D>,

    /// ETL run store
    etl_store: Arc<E>,
}

impl<B, C, S, D, E> McpHandler<B, C, S, D, E>
where
    B: BronzeStorage + Send + Sync,
    C: ConfigStore + Send + Sync,
    S: SilverStorage + Send + Sync,
    D: DictionaryStore + Send + Sync,
    E: EtlRunStore + Send + Sync,
{
    /// Create a new MCP handler with the given dependencies.
    pub fn new(
        storage: Arc<B>,
        config_store: Arc<C>,
        silver_storage: Arc<S>,
        dictionary_store: Arc<D>,
        etl_store: Arc<E>,
    ) -> Self {
        Self {
            storage,
            config_store,
            silver_storage,
            dictionary_store,
            etl_store,
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
            // =============================================
            // Bronze Layer Tools (4)
            // =============================================
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
            // =============================================
            // Silver Layer Tools (4) - dp-010
            // =============================================
            ToolDefinition {
                name: "list_silver_tables".to_string(),
                description: "List all Silver hypertables with metadata from TimescaleDB"
                    .to_string(),
                input_schema: ToolInputSchema::empty(),
            },
            ToolDefinition {
                name: "describe_silver_table".to_string(),
                description:
                    "Get detailed schema for a Silver table including columns and hypertable info"
                        .to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "table_name": {
                            "type": "string",
                            "description": "The Silver table name (e.g., 'air_quality_readings')"
                        }
                    }),
                    vec!["table_name".to_string()],
                ),
            },
            ToolDefinition {
                name: "sample_silver_data".to_string(),
                description: "Sample rows from a Silver table with optional time filtering"
                    .to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "table_name": {
                            "type": "string",
                            "description": "The Silver table name"
                        },
                        "n": {
                            "type": "integer",
                            "description": "Number of rows to return (default: 10, max: 100)",
                            "default": 10,
                            "minimum": 1,
                            "maximum": 100
                        },
                        "since": {
                            "type": "string",
                            "description": "Only include rows after this timestamp (ISO 8601)"
                        },
                        "until": {
                            "type": "string",
                            "description": "Only include rows before this timestamp (ISO 8601)"
                        },
                        "order_by": {
                            "type": "string",
                            "description": "Order by clause (e.g., 'timestamp DESC')"
                        }
                    }),
                    vec!["table_name".to_string()],
                ),
            },
            ToolDefinition {
                name: "silver_stats".to_string(),
                description:
                    "Get statistics for a Silver table including row counts, time ranges, and DQ summary"
                        .to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "table_name": {
                            "type": "string",
                            "description": "The Silver table name"
                        }
                    }),
                    vec!["table_name".to_string()],
                ),
            },
            // =============================================
            // Data Dictionary Tools (4) - dp-010
            // =============================================
            ToolDefinition {
                name: "query_dictionary".to_string(),
                description: "Search the data dictionary for columns matching a query".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "query": {
                            "type": "string",
                            "description": "Search term for partial matching on column names/descriptions"
                        },
                        "layer": {
                            "type": "string",
                            "enum": ["bronze", "silver", "all"],
                            "description": "Filter by layer (default: all)",
                            "default": "all"
                        }
                    }),
                    vec!["query".to_string()],
                ),
            },
            ToolDefinition {
                name: "describe_column".to_string(),
                description:
                    "Get comprehensive details for a specific column including lineage and DQ rules"
                        .to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "table_or_stream": {
                            "type": "string",
                            "description": "Table name (Silver) or stream ID (Bronze)"
                        },
                        "column_name": {
                            "type": "string",
                            "description": "The column name to describe"
                        }
                    }),
                    vec!["table_or_stream".to_string(), "column_name".to_string()],
                ),
            },
            ToolDefinition {
                name: "trace_lineage".to_string(),
                description: "Trace a Silver column back to its Bronze source(s)".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "silver_table": {
                            "type": "string",
                            "description": "The Silver table name"
                        },
                        "silver_column": {
                            "type": "string",
                            "description": "The Silver column name to trace"
                        }
                    }),
                    vec!["silver_table".to_string(), "silver_column".to_string()],
                ),
            },
            ToolDefinition {
                name: "list_dq_rules".to_string(),
                description: "List data quality rules applied to Silver tables/columns".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "table": {
                            "type": "string",
                            "description": "Filter by Silver table name (optional)"
                        },
                        "column": {
                            "type": "string",
                            "description": "Filter by column name (optional, requires table)"
                        }
                    }),
                    vec![],
                ),
            },
            // =============================================
            // ETL Observability Tools (3) - dp-010
            // =============================================
            ToolDefinition {
                name: "etl_status".to_string(),
                description: "Get current ETL status for one or all streams".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "stream_id": {
                            "type": "string",
                            "description": "Optional stream ID to filter by. If omitted, returns all streams."
                        }
                    }),
                    vec![],
                ),
            },
            ToolDefinition {
                name: "etl_history".to_string(),
                description: "Retrieve historical ETL runs for trend analysis and debugging"
                    .to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "stream_id": {
                            "type": "string",
                            "description": "The stream ID to query history for (required)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of runs to return (default: 10, max: 100)",
                            "default": 10,
                            "minimum": 1,
                            "maximum": 100
                        },
                        "since": {
                            "type": "string",
                            "description": "ISO 8601 timestamp to filter runs after"
                        },
                        "status": {
                            "type": "string",
                            "enum": ["running", "success", "failed", "partial"],
                            "description": "Filter by status"
                        }
                    }),
                    vec!["stream_id".to_string()],
                ),
            },
            ToolDefinition {
                name: "data_freshness".to_string(),
                description: "Report data freshness across Bronze and Silver layers".to_string(),
                input_schema: ToolInputSchema::with_properties(
                    serde_json::json!({
                        "layer": {
                            "type": "string",
                            "enum": ["bronze", "silver", "all"],
                            "description": "Filter by layer (default: all)",
                            "default": "all"
                        }
                    }),
                    vec![],
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
            // Bronze layer tools
            "list_streams" => self.execute_list_streams().await,
            "describe_schema" => self.execute_describe_schema(call_params.arguments).await,
            "validate_config" => self.execute_validate_config(call_params.arguments).await,
            "sample_data" => self.execute_sample_data(call_params.arguments).await,

            // Silver layer tools (dp-010)
            "list_silver_tables" => self.execute_list_silver_tables().await,
            "describe_silver_table" => {
                self.execute_describe_silver_table(call_params.arguments)
                    .await
            }
            "sample_silver_data" => self.execute_sample_silver_data(call_params.arguments).await,
            "silver_stats" => self.execute_silver_stats(call_params.arguments).await,

            // Dictionary tools (dp-010)
            "query_dictionary" => self.execute_query_dictionary(call_params.arguments).await,
            "describe_column" => self.execute_describe_column(call_params.arguments).await,
            "trace_lineage" => self.execute_trace_lineage(call_params.arguments).await,
            "list_dq_rules" => self.execute_list_dq_rules(call_params.arguments).await,

            // ETL tools (dp-010)
            "etl_status" => self.execute_etl_status(call_params.arguments).await,
            "etl_history" => self.execute_etl_history(call_params.arguments).await,
            "data_freshness" => self.execute_data_freshness(call_params.arguments).await,

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

    // =========================================================================
    // Bronze Layer Tool Executors
    // =========================================================================

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

    // =========================================================================
    // Silver Layer Tool Executors (dp-010)
    // =========================================================================

    /// Execute list_silver_tables tool.
    async fn execute_list_silver_tables(&self) -> McpResult<McpToolResult> {
        tools::list_silver_tables::execute(self.silver_storage.as_ref()).await
    }

    /// Execute describe_silver_table tool.
    async fn execute_describe_silver_table(
        &self,
        args: Option<Value>,
    ) -> McpResult<McpToolResult> {
        let parsed_args = tools::describe_silver_table::parse_args(args)?;
        tools::describe_silver_table::execute(self.silver_storage.as_ref(), parsed_args).await
    }

    /// Execute sample_silver_data tool.
    async fn execute_sample_silver_data(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let parsed_args = tools::sample_silver_data::parse_args(args)?;
        tools::sample_silver_data::execute(self.silver_storage.as_ref(), parsed_args).await
    }

    /// Execute silver_stats tool.
    async fn execute_silver_stats(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let parsed_args = tools::silver_stats::parse_args(args)?;
        tools::silver_stats::execute(self.silver_storage.as_ref(), parsed_args).await
    }

    // =========================================================================
    // Dictionary Tool Executors (dp-010)
    // =========================================================================

    /// Execute query_dictionary tool.
    async fn execute_query_dictionary(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        tools::query_dictionary::execute(self.dictionary_store.as_ref(), args).await
    }

    /// Execute describe_column tool.
    async fn execute_describe_column(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        tools::describe_column::execute(self.dictionary_store.as_ref(), args).await
    }

    /// Execute trace_lineage tool.
    async fn execute_trace_lineage(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        tools::trace_lineage::execute(self.dictionary_store.as_ref(), args).await
    }

    /// Execute list_dq_rules tool.
    async fn execute_list_dq_rules(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let args = args.unwrap_or(serde_json::json!({}));
        tools::list_dq_rules::execute(self.dictionary_store.as_ref(), args).await
    }

    // =========================================================================
    // ETL Tool Executors (dp-010)
    // =========================================================================

    /// Execute etl_status tool.
    async fn execute_etl_status(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let parsed_args: tools::etl_status::EtlStatusArgs = match args {
            Some(v) => serde_json::from_value(v)
                .map_err(|e| McpError::InvalidParams(format!("Invalid arguments: {}", e)))?,
            None => tools::etl_status::EtlStatusArgs::default(),
        };
        tools::etl_status::execute(self.etl_store.as_ref(), parsed_args).await
    }

    /// Execute etl_history tool.
    async fn execute_etl_history(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let parsed_args: tools::etl_history::EtlHistoryArgs = match args {
            Some(v) => serde_json::from_value(v)
                .map_err(|e| McpError::InvalidParams(format!("Invalid arguments: {}", e)))?,
            None => tools::etl_history::EtlHistoryArgs::default(),
        };
        tools::etl_history::execute(self.etl_store.as_ref(), parsed_args).await
    }

    /// Execute data_freshness tool.
    async fn execute_data_freshness(&self, args: Option<Value>) -> McpResult<McpToolResult> {
        let parsed_args: tools::data_freshness::DataFreshnessArgs = match args {
            Some(v) => serde_json::from_value(v)
                .map_err(|e| McpError::InvalidParams(format!("Invalid arguments: {}", e)))?,
            None => tools::data_freshness::DataFreshnessArgs::default(),
        };
        tools::data_freshness::execute(self.etl_store.as_ref(), parsed_args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etcd::MockConfigStore;
    use crate::storage::{MockBronzeStorage, MockDictionaryStore, MockEtlRunStore, MockSilverStorage};

    fn create_test_handler() -> McpHandler<
        MockBronzeStorage,
        MockConfigStore,
        MockSilverStorage,
        MockDictionaryStore,
        MockEtlRunStore,
    > {
        let storage = Arc::new(MockBronzeStorage::new());
        let config_store = Arc::new(MockConfigStore::new());
        let silver_storage = Arc::new(MockSilverStorage::new());
        let dictionary_store = Arc::new(MockDictionaryStore::new());
        let etl_store = Arc::new(MockEtlRunStore::new());
        McpHandler::new(
            storage,
            config_store,
            silver_storage,
            dictionary_store,
            etl_store,
        )
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
        // 4 Bronze + 4 Silver + 4 Dictionary + 3 ETL = 15 tools
        assert_eq!(tools.len(), 15);
    }

    #[tokio::test]
    async fn test_handle_tools_list_contains_silver_tools() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(request).await;
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();

        let tool_names: Vec<&str> = tools
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();

        // Verify Silver tools are present
        assert!(tool_names.contains(&"list_silver_tables"));
        assert!(tool_names.contains(&"describe_silver_table"));
        assert!(tool_names.contains(&"sample_silver_data"));
        assert!(tool_names.contains(&"silver_stats"));
    }

    #[tokio::test]
    async fn test_handle_tools_list_contains_dictionary_tools() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(request).await;
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();

        let tool_names: Vec<&str> = tools
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();

        // Verify Dictionary tools are present
        assert!(tool_names.contains(&"query_dictionary"));
        assert!(tool_names.contains(&"describe_column"));
        assert!(tool_names.contains(&"trace_lineage"));
        assert!(tool_names.contains(&"list_dq_rules"));
    }

    #[tokio::test]
    async fn test_handle_tools_list_contains_etl_tools() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle(request).await;
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();

        let tool_names: Vec<&str> = tools
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();

        // Verify ETL tools are present
        assert!(tool_names.contains(&"etl_status"));
        assert!(tool_names.contains(&"etl_history"));
        assert!(tool_names.contains(&"data_freshness"));
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

    #[tokio::test]
    async fn test_handle_unknown_tool() {
        let handler = create_test_handler();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "unknown_tool",
                "arguments": {}
            })),
        };

        let response = handler.handle(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, error_codes::INVALID_PARAMS);
    }
}
