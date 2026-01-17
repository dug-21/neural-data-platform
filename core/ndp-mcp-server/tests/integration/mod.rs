//! Integration tests for the NDP MCP Server.
//!
//! These tests verify the HTTP endpoints and MCP protocol handlers work correctly.
//!
//! # Test Categories
//!
//! ## Always Run (cargo test)
//! - `health_test` - Health endpoint verification
//! - `mcp_protocol_test` - MCP protocol and tool listing
//!
//! ## Require TimescaleDB (cargo test -- --ignored)
//! - `timescale_storage_test` - Silver, Dictionary, and ETL storage adapters
//!
//! # Running TimescaleDB Integration Tests
//!
//! ```bash
//! export TEST_DATABASE_URL="postgresql://ndp:password@localhost:5432/ndp"
//! cargo test --test integration -- --ignored
//! ```

mod health_test;
mod mcp_protocol_test;
mod timescale_storage_test;
