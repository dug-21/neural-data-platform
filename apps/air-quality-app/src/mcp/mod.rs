//! MCP (Model Context Protocol) module for Claude integration
//! Exposes air quality data and analytics through MCP tools

pub mod server;
pub mod tools;

#[cfg(test)]
mod test_types;

pub use server::McpServer;
