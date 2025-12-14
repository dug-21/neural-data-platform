//! Data source implementations for ingesting time series data
//!
//! This module provides different strategies for ingesting data:
//! - MQTT: Real-time streaming from MQTT brokers
//! - HTTP Polling: Periodic polling of HTTP endpoints
//! - Merge: Combining and deduplicating data from multiple sources

pub mod http_poll;
pub mod merge;
pub mod mqtt;

pub use http_poll::{HttpPollingConfig, HttpPollingSource, SensorConfig};
pub use merge::{MergeConfig, ReadingMerger};
pub use mqtt::{MqttConfig, MqttSource};
