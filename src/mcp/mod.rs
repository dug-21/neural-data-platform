//! MCP (Model Context Protocol) Integration Module
//! 
//! This module provides MCP tools for interacting with the Neural Trader platform

pub mod trading_tools;
pub mod registration;

pub use trading_tools::TradingMcpTools;
pub use registration::register_mcp_tools;

/// MCP Tool Definitions
pub const MCP_TOOLS: &[&str] = &[
    "query_market_data",
    "get_cache_data",
    "request_prediction",
    "agent_decision",
    "system_status",
];