//! Parser implementations for external data sources
//!
//! This module provides parsers for converting external API responses
//! into TimeSeriesPoint format for ingestion into the platform.

mod weather;
mod air_pollution;

pub use weather::WeatherParser;
pub use air_pollution::AirPollutionParser;
