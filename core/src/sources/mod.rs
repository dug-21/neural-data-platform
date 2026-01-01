//! Data source implementations for ingesting time series data
//!
//! This module provides different strategies for ingesting data:
//! - MQTT: Real-time streaming from MQTT brokers with multi-subscription support
//! - HTTP Polling: Periodic polling of HTTP endpoints
//! - Merge: Combining and deduplicating data from multiple sources
//! - Parsers: Parse external API responses into TimeSeriesPoint format

pub mod http_poll;
pub mod merge;
pub mod mqtt;
pub mod parsers;

pub use http_poll::{
    AuthMethod, EndpointConfig, ErrorClassification, GenericHttpPollingConfig,
    GenericHttpPollingSource, HttpPollingConfig, HttpPollingSource, ParserRegistry, PollingError,
    ResponseParser, RetryConfig, SensorConfig,
};
pub use merge::{MergeConfig, ReadingMerger};
pub use mqtt::{
    mqtt_pattern_to_regex, ConfigError, MqttConfig, MqttSource, RouteEntry, RouterError,
    SubscriptionConfig, SubscriptionError, TopicRouter,
};
pub use parsers::{AirPollutionParser, WeatherParser};

use crate::types::stream_config::SourceType;

// ========== DP-004: Source ID Generation ==========

/// Generate source_id from stream_id and source type.
///
/// Format: "{stream_id}-{source_type_suffix}"
///
/// # Examples
///
/// ```
/// use neural_core::sources::generate_source_id;
/// use neural_core::types::stream_config::SourceType;
///
/// let source_id = generate_source_id("air-quality", &SourceType::HttpPoll);
/// assert_eq!(source_id, "air-quality-Http");
///
/// let mqtt_id = generate_source_id("sensors", &SourceType::Mqtt);
/// assert_eq!(mqtt_id, "sensors-Mqtt");
/// ```
pub fn generate_source_id(stream_id: &str, source_type: &SourceType) -> String {
    let type_suffix = source_type_suffix(source_type);
    format!("{}-{}", stream_id, type_suffix)
}

/// Generate source_id with index for multi-source streams.
///
/// Format: "{stream_id}-{source_type_suffix}-{index}"
///
/// # Examples
///
/// ```
/// use neural_core::sources::generate_source_id_indexed;
/// use neural_core::types::stream_config::SourceType;
///
/// let source_id = generate_source_id_indexed("air-quality", &SourceType::Mqtt, 0);
/// assert_eq!(source_id, "air-quality-Mqtt-0");
/// ```
pub fn generate_source_id_indexed(
    stream_id: &str,
    source_type: &SourceType,
    index: usize,
) -> String {
    format!("{}-{}", generate_source_id(stream_id, source_type), index)
}

/// Get the type suffix for a SourceType.
///
/// Used internally by generate_source_id functions.
fn source_type_suffix(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::HttpPoll => "Http",
        SourceType::Mqtt => "Mqtt",
        SourceType::Webhook => "Webhook",
        SourceType::FileWatch => "FileWatch",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== TDD CYCLE 7: Source ID Generation Tests ==========

    #[test]
    fn test_generate_source_id_http() {
        let source_id = generate_source_id("air-quality", &SourceType::HttpPoll);
        assert_eq!(source_id, "air-quality-Http");
    }

    #[test]
    fn test_generate_source_id_mqtt() {
        let source_id = generate_source_id("sensors", &SourceType::Mqtt);
        assert_eq!(source_id, "sensors-Mqtt");
    }

    #[test]
    fn test_generate_source_id_webhook() {
        let source_id = generate_source_id("events", &SourceType::Webhook);
        assert_eq!(source_id, "events-Webhook");
    }

    #[test]
    fn test_generate_source_id_filewatch() {
        let source_id = generate_source_id("logs", &SourceType::FileWatch);
        assert_eq!(source_id, "logs-FileWatch");
    }

    #[test]
    fn test_generate_source_id_preserves_stream_id_format() {
        // kebab-case stream IDs should be preserved
        let source_id = generate_source_id("home-air-quality", &SourceType::HttpPoll);
        assert_eq!(source_id, "home-air-quality-Http");

        // Single word stream ID
        let source_id = generate_source_id("weather", &SourceType::Mqtt);
        assert_eq!(source_id, "weather-Mqtt");
    }

    #[test]
    fn test_generate_source_id_empty_stream_id() {
        // Edge case: empty stream_id
        let source_id = generate_source_id("", &SourceType::HttpPoll);
        assert_eq!(source_id, "-Http");
    }

    #[test]
    fn test_generate_source_id_indexed_basic() {
        let source_id = generate_source_id_indexed("air-quality", &SourceType::Mqtt, 0);
        assert_eq!(source_id, "air-quality-Mqtt-0");
    }

    #[test]
    fn test_generate_source_id_indexed_multiple_indices() {
        assert_eq!(
            generate_source_id_indexed("sensors", &SourceType::HttpPoll, 0),
            "sensors-Http-0"
        );
        assert_eq!(
            generate_source_id_indexed("sensors", &SourceType::HttpPoll, 1),
            "sensors-Http-1"
        );
        assert_eq!(
            generate_source_id_indexed("sensors", &SourceType::HttpPoll, 10),
            "sensors-Http-10"
        );
    }

    #[test]
    fn test_generate_source_id_indexed_large_index() {
        let source_id = generate_source_id_indexed("test", &SourceType::Mqtt, 999);
        assert_eq!(source_id, "test-Mqtt-999");
    }

    #[test]
    fn test_source_type_suffix() {
        assert_eq!(source_type_suffix(&SourceType::HttpPoll), "Http");
        assert_eq!(source_type_suffix(&SourceType::Mqtt), "Mqtt");
        assert_eq!(source_type_suffix(&SourceType::Webhook), "Webhook");
        assert_eq!(source_type_suffix(&SourceType::FileWatch), "FileWatch");
    }
}
