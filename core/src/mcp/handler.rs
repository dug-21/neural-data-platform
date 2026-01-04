//! MCP protocol handler for Bronze MCP Server (dp-005).
//!
//! Routes incoming MCP JSON-RPC requests to the appropriate handlers.
//! Implements the MCP specification with HTTP POST transport per ADR-001.
//!
//! # Supported Methods
//!
//! | Method | Description |
//! |--------|-------------|
//! | `initialize` | Server capability negotiation |
//! | `tools/list` | List available Bronze layer tools |
//! | `tools/call` | Invoke a tool with parameters |
//!
//! # Example
//!
//! ```ignore
//! use axum::extract::State;
//! use neural_core::mcp::handler::mcp_handler;
//!
//! let response = mcp_handler(State(state), Json(request)).await;
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info, instrument, warn};

use super::protocol::{
    JsonRpcError, McpRequest, McpResponse, McpRpcError, ToolCallParams, ToolDefinition,
    ToolResponse, ToolsListResult,
};

// =============================================================================
// Application State
// =============================================================================

/// Application state shared across request handlers.
///
/// Contains dependencies needed by tool implementations:
/// - Configuration store (etcd client)
/// - Bronze storage abstraction (Parquet reader)
///
/// # Thread Safety
///
/// All fields are `Send + Sync` to allow sharing across async tasks.
pub struct AppState {
    /// Server version from Cargo.toml
    pub version: String,

    /// Server name for initialization response
    pub server_name: String,

    /// Bronze data path (e.g., "/data/raw")
    pub raw_data_path: String,

    /// etcd client for configuration
    pub config_client: Option<Box<dyn ConfigStore + Send + Sync>>,

    /// Bronze storage for Parquet access
    pub storage: Option<Box<dyn BronzeStorage + Send + Sync>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            server_name: "ndp-bronze-mcp".to_string(),
            raw_data_path: "/data/raw".to_string(),
            config_client: None,
            storage: None,
        }
    }
}

// =============================================================================
// Storage Traits (Interface Definitions)
// =============================================================================

/// Configuration store interface for stream config access.
///
/// Abstraction over etcd to enable testing and alternative backends.
#[async_trait::async_trait]
pub trait ConfigStore: Send + Sync {
    /// List all stream IDs from configuration.
    async fn list_stream_ids(&self) -> Result<Vec<String>, ConfigError>;

    /// Get stream configuration by ID.
    async fn get_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;

    /// Check if configuration store is healthy.
    async fn health_check(&self) -> Result<(), ConfigError>;
}

/// Configuration error types.
#[derive(Debug, Clone)]
pub struct ConfigError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Stream configuration from etcd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub enabled: bool,
    pub sources: Vec<SourceConfig>,
    pub entity_schemas: Vec<EntitySchema>,
}

/// Source configuration for a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: String,
    pub parser: Option<ParserConfig>,
}

/// Parser configuration with field mappings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserConfig {
    #[serde(rename = "type")]
    pub parser_type: Option<String>,
    pub field_mappings: Vec<FieldMapping>,
}

/// Field mapping from source to target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source_path: String,
    pub target_field: String,
    pub unit: Option<String>,
}

/// Entity schema definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub schema_name: String,
    pub attributes: Vec<SchemaAttribute>,
}

/// Schema attribute definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaAttribute {
    pub name: String,
    #[serde(rename = "type")]
    pub attr_type: String,
    pub unit: Option<String>,
    pub nullable: Option<bool>,
    pub description: Option<String>,
}

/// Bronze storage interface for Parquet access.
///
/// Abstraction over local filesystem or cloud storage.
#[async_trait::async_trait]
pub trait BronzeStorage: Send + Sync {
    /// Get storage info for a stream (latest partition, file size, etc).
    async fn get_storage_info(&self, stream_id: &str) -> Result<Option<StorageInfo>, StorageError>;

    /// Get raw payload structure from a stream's Parquet file.
    async fn get_raw_payload_structure(
        &self,
        stream_id: &str,
    ) -> Result<RawPayloadStructure, StorageError>;

    /// Sample N rows from a stream.
    async fn sample_rows(&self, stream_id: &str, n: usize) -> Result<SampleResult, StorageError>;

    /// Check if storage is accessible.
    async fn health_check(&self) -> Result<(), StorageError>;
}

/// Storage error types.
#[derive(Debug, Clone)]
pub struct StorageError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for StorageError {}

/// Storage metadata for a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub latest_partition: String,
    pub file_size_bytes: u64,
    pub file_modified: String,
}

/// Raw payload structure from Parquet introspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPayloadStructure {
    pub keys: Vec<String>,
    pub nested: std::collections::HashMap<String, Vec<String>>,
    pub file_analyzed: String,
}

/// Sample data result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleResult {
    pub rows: Vec<serde_json::Value>,
    pub source_file: String,
}

// =============================================================================
// Main Handler
// =============================================================================

/// Main MCP request handler.
///
/// Routes incoming JSON-RPC requests to appropriate method handlers.
///
/// # Arguments
///
/// * `state` - Shared application state
/// * `request` - MCP JSON-RPC request
///
/// # Returns
///
/// MCP JSON-RPC response with result or error.
///
/// # Example
///
/// ```ignore
/// let response = mcp_handler(state, request).await;
/// ```
#[instrument(skip(state), fields(method = %request.method))]
pub async fn mcp_handler(state: Arc<AppState>, request: McpRequest) -> McpResponse {
    // Validate JSON-RPC version
    if !request.is_valid_version() {
        warn!(version = %request.jsonrpc, "Invalid JSON-RPC version");
        return McpResponse::invalid_request(request.id, "Invalid JSON-RPC version, expected 2.0");
    }

    info!(method = %request.method, "Processing MCP request");

    // Route to method handler
    let result = match request.method.as_str() {
        "initialize" => handle_initialize(state.as_ref()).await,
        "tools/list" => handle_tools_list().await,
        "tools/call" => handle_tools_call(state.as_ref(), request.params.clone()).await,
        _ => {
            warn!(method = %request.method, "Unknown method");
            Err(McpRpcError::new(
                JsonRpcError::METHOD_NOT_FOUND,
                format!("Method not found: {}", request.method),
            ))
        }
    };

    // Build response
    match result {
        Ok(value) => {
            debug!("Request successful");
            McpResponse::success(request.id, value)
        }
        Err(error) => {
            error!(code = error.code, message = %error.message, "Request failed");
            McpResponse::error(request.id, error)
        }
    }
}

// =============================================================================
// Method Handlers
// =============================================================================

/// Handle `initialize` method.
///
/// Returns server capabilities and version information per MCP specification.
async fn handle_initialize(state: &AppState) -> Result<serde_json::Value, McpRpcError> {
    info!(
        server = %state.server_name,
        version = %state.version,
        "Handling initialize request"
    );

    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": state.server_name,
            "version": state.version
        }
    }))
}

/// Handle `tools/list` method.
///
/// Returns definitions for all Bronze layer tools.
async fn handle_tools_list() -> Result<serde_json::Value, McpRpcError> {
    info!("Handling tools/list request");

    let tools = vec![
        // Tool 1: list_streams
        ToolDefinition::new(
            "list_streams",
            "List all available Bronze layer streams with metadata including description, enabled status, version, source types, and storage info (latest partition, file size, modification time).",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        // Tool 2: describe_schema
        ToolDefinition::new(
            "describe_schema",
            "Get schema information for a stream. Modes: 'source' shows raw_payload structure and field mappings from parser config, 'target' shows entity_schemas attributes, 'all' (default) shows complete ETL picture with gap analysis identifying unmapped source fields and target fields without mapping.",
            json!({
                "type": "object",
                "properties": {
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
                },
                "required": ["stream_id"]
            }),
        ),
        // Tool 3: validate_config
        ToolDefinition::new(
            "validate_config",
            "Compare stream configuration in etcd against actual Bronze Parquet schema. Returns validation status (match, mismatch, partial), lists config_fields from entity_schemas, raw_payload_fields from Parquet, and analysis showing fields in config but not payload, fields in payload but not config, and matching fields.",
            json!({
                "type": "object",
                "properties": {
                    "stream_id": {
                        "type": "string",
                        "description": "The stream identifier to validate"
                    }
                },
                "required": ["stream_id"]
            }),
        ),
        // Tool 4: sample_data
        ToolDefinition::new(
            "sample_data",
            "Retrieve sample rows from a Bronze stream for exploration. Returns full Bronze envelope structure (timestamp, source_id, ndp_id, context, raw_payload) for N most recent rows. Useful for understanding actual data structure for ETL development.",
            json!({
                "type": "object",
                "properties": {
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
                },
                "required": ["stream_id"]
            }),
        ),
    ];

    let result = ToolsListResult { tools };

    serde_json::to_value(result).map_err(|e| {
        McpRpcError::new(
            JsonRpcError::INTERNAL_ERROR,
            format!("Failed to serialize tools list: {}", e),
        )
    })
}

/// Handle `tools/call` method.
///
/// Routes to the appropriate tool implementation based on tool name.
async fn handle_tools_call(
    state: &AppState,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, McpRpcError> {
    // Extract tool call parameters
    let params = params
        .ok_or_else(|| McpRpcError::new(JsonRpcError::INVALID_PARAMS, "Missing params object"))?;

    let tool_params: ToolCallParams = serde_json::from_value(params).map_err(|e| {
        McpRpcError::new(
            JsonRpcError::INVALID_PARAMS,
            format!("Invalid params structure: {}", e),
        )
    })?;

    info!(tool = %tool_params.name, "Routing to tool implementation");

    // Get arguments (default to empty object)
    let arguments = tool_params.arguments.unwrap_or(json!({}));

    // Route to tool implementation
    let tool_response = match tool_params.name.as_str() {
        "list_streams" => execute_list_streams(state, arguments).await,
        "describe_schema" => execute_describe_schema(state, arguments).await,
        "validate_config" => execute_validate_config(state, arguments).await,
        "sample_data" => execute_sample_data(state, arguments).await,
        _ => {
            warn!(tool = %tool_params.name, "Unknown tool");
            ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Unknown tool: {}", tool_params.name),
                    "code": "UNKNOWN_TOOL"
                }))
                .unwrap_or_default(),
            )
        }
    };

    serde_json::to_value(tool_response).map_err(|e| {
        McpRpcError::new(
            JsonRpcError::INTERNAL_ERROR,
            format!("Failed to serialize tool response: {}", e),
        )
    })
}

// =============================================================================
// Tool Implementations
// =============================================================================

/// Execute `list_streams` tool.
///
/// Lists all Bronze layer streams with metadata.
#[instrument(skip(state))]
async fn execute_list_streams(state: &AppState, _arguments: serde_json::Value) -> ToolResponse {
    debug!("Executing list_streams");

    // Check if config client is available
    let config_client = match &state.config_client {
        Some(client) => client,
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Configuration store not available",
                    "code": "ETCD_UNAVAILABLE"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Get stream IDs from config
    let stream_ids = match config_client.list_stream_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Failed to list streams: {}", e),
                    "code": "ETCD_UNAVAILABLE"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Build stream info for each stream
    let mut streams = Vec::new();
    for stream_id in stream_ids {
        // Get config
        let config = match config_client.get_stream_config(&stream_id).await {
            Ok(config) => config,
            Err(_) => continue, // Skip streams with missing config
        };

        // Get storage info if available
        let storage_info = if let Some(storage) = &state.storage {
            storage.get_storage_info(&stream_id).await.ok().flatten()
        } else {
            None
        };

        // Extract source types
        let sources: Vec<String> = config
            .sources
            .iter()
            .map(|s| s.source_type.clone())
            .collect();

        streams.push(json!({
            "stream_id": stream_id,
            "description": config.description,
            "enabled": config.enabled,
            "version": config.version,
            "sources": sources,
            "storage": storage_info
        }));
    }

    ToolResponse::success(
        serde_json::to_string(&json!({
            "success": true,
            "streams": streams
        }))
        .unwrap_or_default(),
    )
}

/// Input parameters for describe_schema tool.
#[derive(Debug, Deserialize)]
struct DescribeSchemaInput {
    stream_id: String,
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String {
    "all".to_string()
}

/// Execute `describe_schema` tool.
///
/// Returns schema information based on mode: source, target, or all.
#[instrument(skip(state))]
async fn execute_describe_schema(state: &AppState, arguments: serde_json::Value) -> ToolResponse {
    // Parse input
    let input: DescribeSchemaInput = match serde_json::from_value(arguments) {
        Ok(input) => input,
        Err(e) => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Invalid parameters: {}", e),
                    "code": "INVALID_PARAMETER"
                }))
                .unwrap_or_default(),
            );
        }
    };

    debug!(stream_id = %input.stream_id, mode = %input.mode, "Executing describe_schema");

    // Validate mode
    if !["all", "source", "target"].contains(&input.mode.as_str()) {
        return ToolResponse::error(
            serde_json::to_string(&json!({
                "success": false,
                "error": format!("Invalid mode: {}. Expected: all, source, or target", input.mode),
                "code": "INVALID_PARAMETER"
            }))
            .unwrap_or_default(),
        );
    }

    // Get config
    let config = match &state.config_client {
        Some(client) => match client.get_stream_config(&input.stream_id).await {
            Ok(config) => config,
            Err(e) => {
                return ToolResponse::error(
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("Stream not found: {}", input.stream_id),
                        "code": "STREAM_NOT_FOUND",
                        "details": {
                            "stream_id": input.stream_id,
                            "cause": e.to_string()
                        }
                    }))
                    .unwrap_or_default(),
                );
            }
        },
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Configuration store not available",
                    "code": "ETCD_UNAVAILABLE"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Build response based on mode
    match input.mode.as_str() {
        "source" => build_source_schema_response(state, &input.stream_id, &config).await,
        "target" => build_target_schema_response(&input.stream_id, &config),
        "all" => build_all_schema_response(state, &input.stream_id, &config).await,
        _ => unreachable!(), // Already validated above
    }
}

/// Build source schema response.
async fn build_source_schema_response(
    state: &AppState,
    stream_id: &str,
    config: &StreamConfig,
) -> ToolResponse {
    // Get raw payload structure from storage
    let raw_payload_structure = match &state.storage {
        Some(storage) => match storage.get_raw_payload_structure(stream_id).await {
            Ok(structure) => structure,
            Err(e) => {
                return ToolResponse::error(
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("No data available for stream: {}", stream_id),
                        "code": "NO_DATA_AVAILABLE",
                        "details": {
                            "stream_id": stream_id,
                            "cause": e.to_string()
                        }
                    }))
                    .unwrap_or_default(),
                );
            }
        },
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Storage not available",
                    "code": "INTERNAL_ERROR"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Extract field mappings from parser config
    let (parser_type, field_mappings, field_mappings_json) = extract_field_mappings(config);

    // Compute unmapped source fields
    let mapped_source_paths: std::collections::HashSet<_> = field_mappings
        .iter()
        .filter_map(|m| m.source_path.split('.').next().map(|s| s.to_string()))
        .collect();

    let unmapped_source_fields: Vec<String> = raw_payload_structure
        .keys
        .iter()
        .filter(|k| !mapped_source_paths.contains(*k))
        .cloned()
        .collect();

    ToolResponse::success(
        serde_json::to_string(&json!({
            "success": true,
            "stream_id": stream_id,
            "mode": "source",
            "raw_payload_structure": {
                "keys": raw_payload_structure.keys,
                "nested": raw_payload_structure.nested
            },
            "parser_type": parser_type,
            "field_mappings": field_mappings_json,
            "unmapped_source_fields": unmapped_source_fields,
            "file_analyzed": raw_payload_structure.file_analyzed
        }))
        .unwrap_or_default(),
    )
}

/// Build target schema response.
fn build_target_schema_response(stream_id: &str, config: &StreamConfig) -> ToolResponse {
    // Get first entity schema (if available)
    let entity_schema = config.entity_schemas.first();

    match entity_schema {
        Some(schema) => {
            let attributes: Vec<serde_json::Value> = schema
                .attributes
                .iter()
                .map(|attr| {
                    json!({
                        "name": attr.name,
                        "type": attr.attr_type,
                        "unit": attr.unit,
                        "nullable": attr.nullable.unwrap_or(true),
                        "description": attr.description
                    })
                })
                .collect();

            ToolResponse::success(
                serde_json::to_string(&json!({
                    "success": true,
                    "stream_id": stream_id,
                    "mode": "target",
                    "entity_schema": schema.schema_name,
                    "attributes": attributes
                }))
                .unwrap_or_default(),
            )
        }
        None => ToolResponse::error(
            serde_json::to_string(&json!({
                "success": false,
                "error": format!("No entity schema defined for stream: {}", stream_id),
                "code": "NO_DATA_AVAILABLE"
            }))
            .unwrap_or_default(),
        ),
    }
}

/// Build combined schema response with gap analysis.
async fn build_all_schema_response(
    state: &AppState,
    stream_id: &str,
    config: &StreamConfig,
) -> ToolResponse {
    // Get raw payload structure
    let raw_payload_structure = match &state.storage {
        Some(storage) => match storage.get_raw_payload_structure(stream_id).await {
            Ok(structure) => structure,
            Err(e) => {
                return ToolResponse::error(
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("No data available for stream: {}", stream_id),
                        "code": "NO_DATA_AVAILABLE",
                        "details": {
                            "stream_id": stream_id,
                            "cause": e.to_string()
                        }
                    }))
                    .unwrap_or_default(),
                );
            }
        },
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Storage not available",
                    "code": "INTERNAL_ERROR"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Extract field mappings
    let (parser_type, field_mappings, field_mappings_json) = extract_field_mappings(config);

    // Get target fields
    let entity_schema = config.entity_schemas.first();
    let target_fields: Vec<String> = entity_schema
        .map(|s| s.attributes.iter().map(|a| a.name.clone()).collect())
        .unwrap_or_default();

    // Compute gap analysis
    let mapped_source_paths: std::collections::HashSet<_> = field_mappings
        .iter()
        .filter_map(|m| m.source_path.split('.').next().map(|s| s.to_string()))
        .collect();

    let mapped_target_fields: std::collections::HashSet<_> = field_mappings
        .iter()
        .map(|m| m.target_field.clone())
        .collect();

    let unmapped_source_fields: Vec<String> = raw_payload_structure
        .keys
        .iter()
        .filter(|k| !mapped_source_paths.contains(*k))
        .cloned()
        .collect();

    let target_fields_without_mapping: Vec<String> = target_fields
        .iter()
        .filter(|f| !mapped_target_fields.contains(*f))
        .cloned()
        .collect();

    // Build target info
    let target_info = entity_schema.map(|schema| {
        let attributes: Vec<serde_json::Value> = schema
            .attributes
            .iter()
            .map(|attr| {
                json!({
                    "name": attr.name,
                    "type": attr.attr_type,
                    "unit": attr.unit,
                    "nullable": attr.nullable.unwrap_or(true)
                })
            })
            .collect();

        json!({
            "entity_schema": schema.schema_name,
            "attributes": attributes
        })
    });

    ToolResponse::success(
        serde_json::to_string(&json!({
            "success": true,
            "stream_id": stream_id,
            "mode": "all",
            "source": {
                "raw_payload_structure": {
                    "keys": raw_payload_structure.keys,
                    "nested": raw_payload_structure.nested
                },
                "parser_type": parser_type,
                "field_mappings": field_mappings_json
            },
            "target": target_info,
            "gap_analysis": {
                "unmapped_source_fields": unmapped_source_fields,
                "target_fields_without_mapping": target_fields_without_mapping
            },
            "file_analyzed": raw_payload_structure.file_analyzed
        }))
        .unwrap_or_default(),
    )
}

/// Simplified field mapping for internal processing.
#[derive(Debug, Clone)]
struct SimplifiedFieldMapping {
    source_path: String,
    target_field: String,
}

/// Extract field mappings from stream config.
///
/// Returns (parser_type, typed_mappings, json_mappings).
fn extract_field_mappings(
    config: &StreamConfig,
) -> (
    Option<String>,
    Vec<SimplifiedFieldMapping>,
    Vec<serde_json::Value>,
) {
    let mut parser_type = None;
    let mut typed_mappings = Vec::new();
    let mut json_mappings = Vec::new();

    for source in &config.sources {
        if let Some(parser) = &source.parser {
            parser_type = parser.parser_type.clone();
            for mapping in &parser.field_mappings {
                typed_mappings.push(SimplifiedFieldMapping {
                    source_path: mapping.source_path.clone(),
                    target_field: mapping.target_field.clone(),
                });
                json_mappings.push(json!({
                    "source_path": mapping.source_path,
                    "target_field": mapping.target_field,
                    "unit": mapping.unit
                }));
            }
        }
    }

    (parser_type, typed_mappings, json_mappings)
}

/// Input parameters for validate_config tool.
#[derive(Debug, Deserialize)]
struct ValidateConfigInput {
    stream_id: String,
}

/// Execute `validate_config` tool.
///
/// Compares config in etcd against actual Bronze Parquet schema.
#[instrument(skip(state))]
async fn execute_validate_config(state: &AppState, arguments: serde_json::Value) -> ToolResponse {
    // Parse input
    let input: ValidateConfigInput = match serde_json::from_value(arguments) {
        Ok(input) => input,
        Err(e) => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Invalid parameters: {}", e),
                    "code": "INVALID_PARAMETER"
                }))
                .unwrap_or_default(),
            );
        }
    };

    debug!(stream_id = %input.stream_id, "Executing validate_config");

    // Get config
    let config = match &state.config_client {
        Some(client) => match client.get_stream_config(&input.stream_id).await {
            Ok(config) => config,
            Err(e) => {
                return ToolResponse::error(
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("Stream not found: {}", input.stream_id),
                        "code": "STREAM_NOT_FOUND",
                        "details": {
                            "stream_id": input.stream_id,
                            "cause": e.to_string()
                        }
                    }))
                    .unwrap_or_default(),
                );
            }
        },
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Configuration store not available",
                    "code": "ETCD_UNAVAILABLE"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Get raw payload structure
    let raw_payload_structure = match &state.storage {
        Some(storage) => match storage.get_raw_payload_structure(&input.stream_id).await {
            Ok(structure) => structure,
            Err(e) => {
                return ToolResponse::error(
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("No data available for stream: {}", input.stream_id),
                        "code": "NO_DATA_AVAILABLE",
                        "details": {
                            "stream_id": input.stream_id,
                            "cause": e.to_string(),
                            "suggestion": "Stream may be disabled or not yet ingesting data"
                        }
                    }))
                    .unwrap_or_default(),
                );
            }
        },
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Storage not available",
                    "code": "INTERNAL_ERROR"
                }))
                .unwrap_or_default(),
            );
        }
    };

    // Get config fields from entity schema
    let entity_schema = config.entity_schemas.first();
    let (schema_name, config_fields): (Option<String>, Vec<String>) = match entity_schema {
        Some(schema) => (
            Some(schema.schema_name.clone()),
            schema.attributes.iter().map(|a| a.name.clone()).collect(),
        ),
        None => (None, Vec::new()),
    };

    // Get raw payload fields
    let raw_payload_fields: Vec<String> = raw_payload_structure.keys.clone();

    // Compute set operations
    let config_set: std::collections::HashSet<_> = config_fields.iter().cloned().collect();
    let payload_set: std::collections::HashSet<_> = raw_payload_fields.iter().cloned().collect();

    let in_config_not_in_payload: Vec<String> =
        config_set.difference(&payload_set).cloned().collect();
    let in_payload_not_in_config: Vec<String> =
        payload_set.difference(&config_set).cloned().collect();
    let matching: Vec<String> = config_set.intersection(&payload_set).cloned().collect();

    // Determine validation status
    let status = if in_config_not_in_payload.is_empty() && in_payload_not_in_config.is_empty() {
        "match"
    } else if matching.is_empty() {
        "mismatch"
    } else {
        "partial"
    };

    ToolResponse::success(
        serde_json::to_string(&json!({
            "success": true,
            "stream_id": input.stream_id,
            "entity_schema": schema_name,
            "validation": {
                "status": status,
                "config_fields": config_fields,
                "raw_payload_fields": raw_payload_fields,
                "analysis": {
                    "in_config_not_in_payload": in_config_not_in_payload,
                    "in_payload_not_in_config": in_payload_not_in_config,
                    "matching": matching
                },
                "notes": "Config uses flattened field names; raw_payload preserves source structure. Mapping happens in Silver layer via parser field_mappings."
            }
        }))
        .unwrap_or_default(),
    )
}

/// Input parameters for sample_data tool.
#[derive(Debug, Deserialize)]
struct SampleDataInput {
    stream_id: String,
    #[serde(default = "default_n")]
    n: usize,
}

fn default_n() -> usize {
    10
}

/// Execute `sample_data` tool.
///
/// Returns N sample rows from a Bronze stream.
#[instrument(skip(state))]
async fn execute_sample_data(state: &AppState, arguments: serde_json::Value) -> ToolResponse {
    // Parse input
    let input: SampleDataInput = match serde_json::from_value(arguments) {
        Ok(input) => input,
        Err(e) => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Invalid parameters: {}", e),
                    "code": "INVALID_PARAMETER"
                }))
                .unwrap_or_default(),
            );
        }
    };

    debug!(stream_id = %input.stream_id, n = input.n, "Executing sample_data");

    // Validate n
    if input.n > 100 {
        return ToolResponse::error(
            serde_json::to_string(&json!({
                "success": false,
                "error": "Parameter 'n' exceeds maximum value of 100",
                "code": "INVALID_PARAMETER",
                "details": {
                    "parameter": "n",
                    "value": input.n,
                    "constraint": "maximum: 100"
                }
            }))
            .unwrap_or_default(),
        );
    }

    if input.n == 0 {
        return ToolResponse::error(
            serde_json::to_string(&json!({
                "success": false,
                "error": "Parameter 'n' must be at least 1",
                "code": "INVALID_PARAMETER",
                "details": {
                    "parameter": "n",
                    "value": input.n,
                    "constraint": "minimum: 1"
                }
            }))
            .unwrap_or_default(),
        );
    }

    // Verify stream exists in config
    if let Some(client) = &state.config_client {
        if let Err(e) = client.get_stream_config(&input.stream_id).await {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": format!("Stream not found: {}", input.stream_id),
                    "code": "STREAM_NOT_FOUND",
                    "details": {
                        "stream_id": input.stream_id,
                        "cause": e.to_string()
                    }
                }))
                .unwrap_or_default(),
            );
        }
    }

    // Get sample data from storage
    let sample_result = match &state.storage {
        Some(storage) => match storage.sample_rows(&input.stream_id, input.n).await {
            Ok(result) => result,
            Err(e) => {
                return ToolResponse::error(
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("Failed to sample data: {}", e),
                        "code": "NO_DATA_AVAILABLE",
                        "details": {
                            "stream_id": input.stream_id,
                            "cause": e.to_string(),
                            "suggestion": "Stream may be disabled or not yet ingesting data"
                        }
                    }))
                    .unwrap_or_default(),
                );
            }
        },
        None => {
            return ToolResponse::error(
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Storage not available",
                    "code": "INTERNAL_ERROR"
                }))
                .unwrap_or_default(),
            );
        }
    };

    ToolResponse::success(
        serde_json::to_string(&json!({
            "success": true,
            "stream_id": input.stream_id,
            "row_count": sample_result.rows.len(),
            "rows": sample_result.rows,
            "source_file": sample_result.source_file
        }))
        .unwrap_or_default(),
    )
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -------------------------------------------------------------------------
    // Test State Creation
    // -------------------------------------------------------------------------

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState::default())
    }

    // -------------------------------------------------------------------------
    // Handler Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_mcp_handler_invalid_version() {
        let state = test_state();
        let request = McpRequest {
            jsonrpc: "1.0".to_string(),
            id: Some(json!("test-1")),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = mcp_handler(state, request).await;
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("-32600")); // Invalid request
        assert!(json.contains("Invalid JSON-RPC version"));
    }

    #[tokio::test]
    async fn test_mcp_handler_unknown_method() {
        let state = test_state();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("test-1")),
            method: "unknown/method".to_string(),
            params: None,
        };

        let response = mcp_handler(state, request).await;
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("-32601")); // Method not found
        assert!(json.contains("unknown/method"));
    }

    // -------------------------------------------------------------------------
    // Initialize Handler Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_handle_initialize() {
        let state = AppState::default();
        let result = handle_initialize(&state).await.unwrap();

        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "ndp-bronze-mcp");
    }

    // -------------------------------------------------------------------------
    // Tools List Handler Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_handle_tools_list() {
        let result = handle_tools_list().await.unwrap();

        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 4);

        // Verify tool names
        let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

        assert!(tool_names.contains(&"list_streams"));
        assert!(tool_names.contains(&"describe_schema"));
        assert!(tool_names.contains(&"validate_config"));
        assert!(tool_names.contains(&"sample_data"));
    }

    #[tokio::test]
    async fn test_tools_list_has_input_schemas() {
        let result = handle_tools_list().await.unwrap();
        let tools = result["tools"].as_array().unwrap();

        for tool in tools {
            assert!(tool["inputSchema"].is_object());
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[tokio::test]
    async fn test_tools_list_describe_schema_has_required_stream_id() {
        let result = handle_tools_list().await.unwrap();
        let tools = result["tools"].as_array().unwrap();

        let describe_schema = tools
            .iter()
            .find(|t| t["name"] == "describe_schema")
            .unwrap();

        let required = describe_schema["inputSchema"]["required"]
            .as_array()
            .unwrap();
        assert!(required.iter().any(|v| v == "stream_id"));
    }

    #[tokio::test]
    async fn test_tools_list_sample_data_has_n_constraints() {
        let result = handle_tools_list().await.unwrap();
        let tools = result["tools"].as_array().unwrap();

        let sample_data = tools.iter().find(|t| t["name"] == "sample_data").unwrap();

        let n_prop = &sample_data["inputSchema"]["properties"]["n"];
        assert_eq!(n_prop["default"], 10);
        assert_eq!(n_prop["minimum"], 1);
        assert_eq!(n_prop["maximum"], 100);
    }

    // -------------------------------------------------------------------------
    // Tools Call Handler Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_handle_tools_call_missing_params() {
        let state = AppState::default();
        let result = handle_tools_call(&state, None).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, JsonRpcError::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_handle_tools_call_invalid_params() {
        let state = AppState::default();
        let result = handle_tools_call(&state, Some(json!("not an object"))).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, JsonRpcError::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_handle_tools_call_unknown_tool() {
        let state = AppState::default();
        let result = handle_tools_call(
            &state,
            Some(json!({
                "name": "unknown_tool",
                "arguments": {}
            })),
        )
        .await;

        assert!(result.is_ok());
        let value = result.unwrap();

        // Should be a ToolResponse with isError
        assert!(value["isError"].as_bool().unwrap_or(false));
        assert!(value["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("UNKNOWN_TOOL"));
    }

    // -------------------------------------------------------------------------
    // List Streams Tool Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_execute_list_streams_no_config_client() {
        let state = AppState::default();
        let response = execute_list_streams(&state, json!({})).await;

        assert!(response.is_error());
        assert!(response.content[0].text.contains("ETCD_UNAVAILABLE"));
    }

    // -------------------------------------------------------------------------
    // Describe Schema Tool Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_execute_describe_schema_invalid_mode() {
        let state = AppState::default();
        let response = execute_describe_schema(
            &state,
            json!({
                "stream_id": "test",
                "mode": "invalid"
            }),
        )
        .await;

        assert!(response.is_error());
        assert!(response.content[0].text.contains("INVALID_PARAMETER"));
        assert!(response.content[0].text.contains("Invalid mode"));
    }

    #[tokio::test]
    async fn test_execute_describe_schema_no_config_client() {
        let state = AppState::default();
        let response = execute_describe_schema(
            &state,
            json!({
                "stream_id": "test"
            }),
        )
        .await;

        assert!(response.is_error());
        assert!(response.content[0].text.contains("ETCD_UNAVAILABLE"));
    }

    // -------------------------------------------------------------------------
    // Validate Config Tool Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_execute_validate_config_no_config_client() {
        let state = AppState::default();
        let response = execute_validate_config(
            &state,
            json!({
                "stream_id": "test"
            }),
        )
        .await;

        assert!(response.is_error());
        assert!(response.content[0].text.contains("ETCD_UNAVAILABLE"));
    }

    // -------------------------------------------------------------------------
    // Sample Data Tool Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_execute_sample_data_n_exceeds_max() {
        let state = AppState::default();
        let response = execute_sample_data(
            &state,
            json!({
                "stream_id": "test",
                "n": 200
            }),
        )
        .await;

        assert!(response.is_error());
        assert!(response.content[0].text.contains("INVALID_PARAMETER"));
        assert!(response.content[0].text.contains("maximum"));
    }

    #[tokio::test]
    async fn test_execute_sample_data_n_zero() {
        let state = AppState::default();
        let response = execute_sample_data(
            &state,
            json!({
                "stream_id": "test",
                "n": 0
            }),
        )
        .await;

        assert!(response.is_error());
        assert!(response.content[0].text.contains("INVALID_PARAMETER"));
        assert!(response.content[0].text.contains("minimum"));
    }

    #[tokio::test]
    async fn test_execute_sample_data_default_n() {
        // Verify default_n function
        assert_eq!(default_n(), 10);
    }

    // -------------------------------------------------------------------------
    // Input Deserialization Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_describe_schema_input_defaults() {
        let input: DescribeSchemaInput = serde_json::from_value(json!({
            "stream_id": "test"
        }))
        .unwrap();

        assert_eq!(input.stream_id, "test");
        assert_eq!(input.mode, "all");
    }

    #[test]
    fn test_sample_data_input_defaults() {
        let input: SampleDataInput = serde_json::from_value(json!({
            "stream_id": "test"
        }))
        .unwrap();

        assert_eq!(input.stream_id, "test");
        assert_eq!(input.n, 10);
    }

    #[test]
    fn test_sample_data_input_custom_n() {
        let input: SampleDataInput = serde_json::from_value(json!({
            "stream_id": "test",
            "n": 50
        }))
        .unwrap();

        assert_eq!(input.n, 50);
    }

    // -------------------------------------------------------------------------
    // Integration-style Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_full_initialize_flow() {
        let state = test_state();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("init-1")),
            method: "initialize".to_string(),
            params: None,
        };

        let response = mcp_handler(state, request).await;
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("2.0"));
        assert!(json.contains("init-1"));
        assert!(json.contains("protocolVersion"));
        assert!(json.contains("ndp-bronze-mcp"));
    }

    #[tokio::test]
    async fn test_full_tools_list_flow() {
        let state = test_state();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("list-1")),
            method: "tools/list".to_string(),
            params: Some(json!({})),
        };

        let response = mcp_handler(state, request).await;
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("list-1"));
        assert!(json.contains("list_streams"));
        assert!(json.contains("describe_schema"));
        assert!(json.contains("validate_config"));
        assert!(json.contains("sample_data"));
    }
}
