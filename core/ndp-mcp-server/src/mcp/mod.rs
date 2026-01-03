//! MCP Protocol Implementation
//!
//! This module implements the Model Context Protocol (MCP) for the Bronze layer
//! server. It includes JSON-RPC 2.0 protocol types, request handlers, and tool
//! implementations.
//!
//! # Module Structure
//!
//! - [`protocol`]: JSON-RPC 2.0 types and MCP-specific structures
//! - [`handler`]: Request routing and method dispatch
//! - [`tools`]: Tool implementations (list_streams, describe_schema, etc.)
//!
//! # MCP Methods Supported
//!
//! - `initialize`: Server capability negotiation
//! - `tools/list`: Return available tools with input schemas
//! - `tools/call`: Execute a tool with provided arguments

pub mod handler;
pub mod protocol;
pub mod tools;

pub use handler::McpHandler;
pub use protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpContent, McpToolResult, ToolDefinition,
    ToolInputSchema,
};
