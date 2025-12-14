//! Air Quality Domain - AirGradient device data models and parsers
//!
//! This crate provides comprehensive support for AirGradient ONE devices,
//! supporting all 29 fields from both MQTT and Local API data sources.

pub mod adapter;
pub mod parser;
pub mod types;
pub mod validation;

pub use adapter::AirQualityAdapter;
pub use parser::{parse_local_api_payload, parse_mqtt_payload, ParserError};
pub use types::{
    AirQualityReading, DeviceMetadata, EnvironmentalData, GasData, ParticleData, QualityMetrics,
};
pub use validation::{validate_reading, ValidationError};
