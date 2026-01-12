//! MCP Tools for Bronze Layer Data Exploration (dp-005)
//!
//! This module implements the 4 core MCP tools for the Bronze MCP Server:
//! - `list_streams`: Enumerate all Bronze layer streams with metadata
//! - `describe_schema`: Get schema information with source/target/all modes
//! - `validate_config`: Compare etcd config against actual Parquet schema
//! - `sample_data`: Retrieve sample rows from Bronze streams
//!
//! # Design Principles
//!
//! Following London School TDD (Outside-In, Mock-Driven):
//! - Tools depend on trait abstractions (BronzeStorage, ConfigStore)
//! - All external dependencies are injectable and mockable
//! - Tests verify behavior through mock expectations
//!
//! # Example
//!
//! ```ignore
//! use neural_core::mcp::tools::{list_streams, AppState};
//!
//! let state = AppState::new(storage, config_store);
//! let result = list_streams::execute(&state, json!({})).await?;
//! ```

pub mod describe_schema;
pub mod list_streams;
mod response;
pub mod sample_data;
pub mod traits;
pub mod validate_config;

pub use response::{create_error_response, create_tool_response, ToolResult};
pub use traits::{BronzeStorage, ConfigStore, StreamConfigInfo, StreamStorageInfo};

use std::sync::Arc;

/// Shared application state for MCP tools
///
/// Contains injected dependencies for storage and configuration access.
/// This follows the Domain Adapter pattern for testability.
#[derive(Clone)]
pub struct AppState {
    /// Bronze layer storage abstraction
    pub storage: Arc<dyn BronzeStorage>,
    /// Configuration store abstraction (etcd)
    pub config: Arc<dyn ConfigStore>,
}

impl AppState {
    /// Create new application state with injected dependencies
    pub fn new(storage: Arc<dyn BronzeStorage>, config: Arc<dyn ConfigStore>) -> Self {
        Self { storage, config }
    }
}

/// Error codes for MCP tool responses
///
/// Following ADR-005 response format specification
pub mod error_codes {
    pub const STREAM_NOT_FOUND: &str = "STREAM_NOT_FOUND";
    pub const ETCD_UNAVAILABLE: &str = "ETCD_UNAVAILABLE";
    pub const NO_DATA_AVAILABLE: &str = "NO_DATA_AVAILABLE";
    pub const INVALID_PARAMETER: &str = "INVALID_PARAMETER";
    pub const PARSE_ERROR: &str = "PARSE_ERROR";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
    pub const UNKNOWN_TOOL: &str = "UNKNOWN_TOOL";
}
