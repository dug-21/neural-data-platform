//! MCP Tool Implementations
//!
//! This module contains the implementations for all MCP tools exposed by the
//! Bronze layer server.
//!
//! # Tools
//!
//! - [`list_streams`]: List all available Bronze streams with metadata
//! - [`describe_schema`]: Get schema information for a stream
//! - [`validate_config`]: Validate stream config against actual data
//! - [`sample_data`]: Get sample rows from a stream

pub mod describe_schema;
pub mod list_streams;
pub mod sample_data;
pub mod validate_config;
