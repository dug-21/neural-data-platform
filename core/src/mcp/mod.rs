//! MCP (Model Context Protocol) module for JSON-RPC communication.
//!
//! This module provides:
//! - Protocol types following MCP specification and JSON-RPC 2.0 standard
//! - Request handler for routing MCP methods
//! - Tool implementations for Bronze layer data exploration
//!
//! # Architecture
//!
//! ```text
//! HTTP POST /mcp
//!     |
//!     v
//! mcp_handler() - Routes by method name
//!     |
//!     +-- initialize -> Server capabilities
//!     +-- tools/list -> Tool definitions
//!     +-- tools/call -> Tool router
//!             |
//!             +-- list_streams
//!             +-- describe_schema
//!             +-- validate_config
//!             +-- sample_data
//! ```
//!
//! # Example
//!
//! ```rust
//! use neural_core::mcp::{McpRequest, McpResponse, McpResult, ToolContent, ToolResponse};
//!
//! // Parse incoming request
//! let request: McpRequest = serde_json::from_str(r#"{
//!     "jsonrpc": "2.0",
//!     "id": "1",
//!     "method": "tools/list",
//!     "params": {}
//! }"#).unwrap();
//!
//! // Build tool response
//! let tool_response = ToolResponse::success("Sample data here".to_string());
//!
//! // Wrap in MCP response
//! let mcp_response = McpResponse::success(request.id.clone(), tool_response);
//! ```

// Core protocol types
mod protocol;

// Error types
mod error;
mod types;

// Handler and tools
pub mod handler;

// MCP Tools (dp-005 Bronze layer exploration)
pub mod tools;

// Protocol exports
pub use protocol::{
    JsonRpcError, McpRequest, McpResponse, McpResult, McpRpcError, ToolCallParams, ToolContent,
    ToolDefinition, ToolResponse, ToolsListResult,
};

// Error and type exports
pub use error::McpError;
pub use types::{
    Attribute, EntitySchema as EntitySchemaV2, FieldMapping as FieldMappingV2,
    ParserConfig as ParserConfigV2, SourceConfig as SourceConfigV2, StreamConfig as StreamConfigV2,
    StreamInfo,
};

// Re-export handler types
pub use handler::{
    mcp_handler, AppState, BronzeStorage, ConfigError, ConfigStore, EntitySchema, FieldMapping,
    ParserConfig, RawPayloadStructure, SampleResult, SchemaAttribute, SourceConfig, StorageError,
    StorageInfo, StreamConfig,
};
