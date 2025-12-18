//! Parser implementations for external data sources
//!
//! This module provides parsers for converting external API responses
//! into TimeSeriesPoint format for ingestion into the platform.

mod air_pollution;
mod weather;

pub use air_pollution::AirPollutionParser;
pub use weather::WeatherParser;
