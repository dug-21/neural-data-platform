//! MCP Tool Implementations
//!
//! This module contains the implementations for all MCP tools exposed by the
//! Bronze layer server.
//!
//! # Bronze Layer Tools
//!
//! - [`list_streams`]: List all available Bronze streams with metadata
//! - [`describe_schema`]: Get schema information for a stream
//! - [`validate_config`]: Validate stream config against actual data
//! - [`sample_data`]: Get sample rows from a stream
//!
//! # Silver Layer Tools (dp-010)
//!
//! - [`list_silver_tables`]: List all Silver hypertables with metadata
//! - [`describe_silver_table`]: Get detailed schema for a Silver table
//! - [`sample_silver_data`]: Sample rows from a Silver table
//! - [`silver_stats`]: Get statistics for a Silver table
//!
//! # ETL Observability Tools (dp-010)
//!
//! - [`etl_status`]: Get current ETL status for one or all streams
//! - [`etl_history`]: Retrieve historical ETL runs for trend analysis
//! - [`data_freshness`]: Report data freshness across Bronze and Silver layers
//!
//! # Data Dictionary Tools (dp-010)
//!
//! - [`query_dictionary`]: Search the data dictionary for columns matching a query
//! - [`describe_column`]: Get comprehensive details for a specific column
//! - [`trace_lineage`]: Trace a Silver column back to its Bronze source(s)
//! - [`list_dq_rules`]: List DQ rules applied to Silver tables/columns

// Bronze layer tools
pub mod describe_schema;
pub mod list_streams;
pub mod sample_data;
pub mod validate_config;

// Silver layer tools (dp-010)
pub mod describe_silver_table;
pub mod list_silver_tables;
pub mod sample_silver_data;
pub mod silver_stats;

// ETL observability tools (dp-010)
pub mod data_freshness;
pub mod etl_history;
pub mod etl_status;

// Data Dictionary tools (dp-010)
pub mod describe_column;
pub mod list_dq_rules;
pub mod query_dictionary;
pub mod trace_lineage;
