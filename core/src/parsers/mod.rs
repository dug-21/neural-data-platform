//! Config-driven parser module for Neural Data Platform
//!
//! This module provides a flexible, configuration-driven approach to parsing
//! data from various sources. Instead of hardcoded structs, parsers are
//! configured via YAML and can dynamically extract fields from JSON payloads.

pub mod config;
pub mod factory;
pub mod flat_json;
pub mod json_path;
pub mod traits;

pub use config::{FieldMapping, ParserConfig, ParserType};
pub use factory::create_parser_from_config;
pub use flat_json::FlatJsonParser;
pub use json_path::JsonPathParser;
pub use traits::Parser;
