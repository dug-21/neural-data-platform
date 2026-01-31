//! Config-driven parser module for Neural Data Platform
//!
//! This module provides a flexible, configuration-driven approach to parsing
//! data from various sources. Instead of hardcoded structs, parsers are
//! configured via YAML and can dynamically extract fields from JSON payloads.
//!
//! # Deprecation Notice
//!
//! This module is deprecated as of v0.2.0. For Bronze-to-Silver field extraction
//! and transformation, use [`crate::silver::transform`] instead. The Silver
//! transform module provides:
//! - Configuration-driven field mappings from stream YAML
//! - Data quality rule evaluation
//! - Type coercion and unit conversions
//!
//! Parsers will be removed in v0.3.0.
#![deprecated(
    since = "0.2.0",
    note = "Use silver/transform.rs for field extraction. Parsers will be removed in 0.3.0"
)]

pub mod array_iterator;
pub mod column_oriented;
pub mod config;
pub mod factory;
pub mod flat_json;
pub mod json_path;
pub mod raw_text;
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
pub use raw_text::{RawTextConfig, RawTextParser};
pub use traits::{ParseContext, Parser};
