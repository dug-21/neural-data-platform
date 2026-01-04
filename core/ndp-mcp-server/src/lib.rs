//! NDP Bronze MCP Server Library
//!
//! This library implements a Model Context Protocol (MCP) server that exposes
//! Bronze layer data exploration and configuration validation tools.
//!
//! # Architecture
//!
//! The server follows the NDP Domain Adapter pattern (hexagonal architecture):
//!
//! - **Core**: MCP protocol handling, tool orchestration
//! - **Ports (Traits)**: `BronzeStorage`, `ConfigStore`
//! - **Adapters**: `LocalParquetStorage`, `StreamRegistryAdapter` (via config-client)
//!
//! # Modules
//!
//! - [`config`]: Environment-based configuration
//! - [`error`]: Error types with MCP error codes
//! - [`server`]: HTTP server with axum
//! - [`etcd`]: etcd configuration via config-client
//! - [`mcp`]: MCP protocol types and handlers
//! - [`storage`]: Bronze layer storage abstraction

pub mod config;
pub mod error;
pub mod etcd;
pub mod mcp;
pub mod server;
pub mod storage;

// Re-export commonly used types
pub use config::AppConfig;
pub use error::{McpError, McpResult};
pub use etcd::{ConfigStore, StreamRegistryAdapter};
pub use mcp::McpHandler;
pub use server::{create_router, AppState};
pub use storage::{BronzeStorage, LocalParquetStorage};
