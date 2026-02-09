//! Parsing utilities for Gold DDL generation
//!
//! Provides granularity and window parsing used by generators and registry modules.
//! Semantic validation of Gold ETL config is handled by `crate::validate::semantic::gold`.

mod config_validator;

pub use config_validator::{granularity_to_suffix, parse_granularity, parse_window};
