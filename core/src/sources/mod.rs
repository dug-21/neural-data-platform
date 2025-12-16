//! Data source implementations for ingesting time series data
//!
//! This module provides different strategies for ingesting data:
//! - MQTT: Real-time streaming from MQTT brokers
//! - HTTP Polling: Periodic polling of HTTP endpoints
//! - Merge: Combining and deduplicating data from multiple sources
//! - Parsers: Parse external API responses into TimeSeriesPoint format

pub mod http_poll;
pub mod merge;
pub mod mqtt;
pub mod parsers;

pub use http_poll::{
    AuthMethod, EndpointConfig, ErrorClassification, GenericHttpPollingConfig,
    GenericHttpPollingSource, HttpPollingConfig, HttpPollingSource, ParserRegistry,
    PollingError, ResponseParser, RetryConfig, SensorConfig,
};
pub use merge::{MergeConfig, ReadingMerger};
pub use mqtt::{MqttConfig, MqttSource};
pub use parsers::{AirPollutionParser, WeatherParser};
