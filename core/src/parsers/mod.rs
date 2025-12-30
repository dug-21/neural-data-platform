//! Config-driven parser module for Neural Data Platform
//!
//! This module provides a flexible, configuration-driven approach to parsing
//! data from various sources. Instead of hardcoded structs, parsers are
//! configured via YAML and can dynamically extract fields from JSON payloads.

pub mod array_iterator;
pub mod column_oriented;
pub mod config;
pub mod factory;
pub mod flat_json;
pub mod json_path;
pub mod traits;

pub use array_iterator::{
    ArrayIteratorConfig, ArrayIteratorParser, ElementMapping, MetadataTagMapping, StringParseConfig,
};
pub use column_oriented::ColumnOrientedParser;
pub use config::{
    ColumnMapping, ColumnOrientedConfig, ConversionFormula, FieldMapping, ParserConfig, ParserType,
    TimestampFormat, UnitConversion,
};
pub use factory::create_parser_from_config;
pub use flat_json::FlatJsonParser;
pub use json_path::JsonPathParser;
pub use traits::Parser;
