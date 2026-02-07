//! Validation module for Gold DDL generation
//!
//! Validates configuration before generating DDL.

mod config_validator;

pub use config_validator::{
    granularity_to_suffix, parse_granularity, parse_window, validate_gold_config, ConfigValidator,
};
