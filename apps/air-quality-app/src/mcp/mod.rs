//! MCP (Model Context Protocol) module for Claude integration
//! Exposes air quality data and analytics through MCP tools

pub mod tools;
pub mod server;

#[cfg(test)]
mod test_types;

pub use server::McpServer;
