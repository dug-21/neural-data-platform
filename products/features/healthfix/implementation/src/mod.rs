//! Health monitoring implementation for Neural Trader
//! 
//! This module provides comprehensive health monitoring with:
//! - Non-blocking async health monitoring
//! - Standalone HTTP health server on port 8080
//! - Real health checks for Database, Redis, Neural, and DAA components
//! - Circuit breaker pattern for fault tolerance
//! - Prometheus metrics export

pub mod health;
pub mod mcp_server_config;
pub mod mcp_server_enhanced;

pub use health::*;
pub use mcp_server_config::*;
pub use mcp_server_enhanced::*;