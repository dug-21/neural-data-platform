//! Silver layer module for Bronze-to-Silver transformation
//!
//! This module contains types and utilities for transforming Bronze (raw Parquet)
//! data into Silver (TimescaleDB) format with data quality evaluation.
//!
//! # Architecture (DP-012)
//!
//! Bronze (Parquet) -> Silver (TimescaleDB)
//! Raw, append-only data -> Queryable, indexed data
//!
//! # Modules
//!
//! - `types`: Core Silver types (SilverRecord, TransformError, DqResult)
//! - `transform`: Bronze-to-Silver transformation logic
//! - `dq_evaluator`: Data quality rule evaluation
//! - `outputs`: Output sinks (TimescaleOutput, InMemorySilverOutput)

pub mod dq_evaluator;
pub mod outputs;
pub mod transform;
pub mod types;

// Re-export main types
pub use types::{DqResult, DqViolation, SilverRecord, TransformError};

// Re-export transform functions
pub use transform::transform_to_silver;

// Re-export DQ evaluation functions
pub use dq_evaluator::{evaluate_and_apply_dq_rules, evaluate_dq_rules};

// Re-export output types
pub use outputs::{
    InMemorySilverOutput, SilverOutput, SilverOutputError, TimescaleConfig, TimescaleOutput,
};
