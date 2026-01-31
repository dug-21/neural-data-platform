//! MQTT subscription configuration types
//!
//! Provides configuration structures for MQTT topic subscriptions,
//! enabling multi-subscription support with per-subscription parser configuration.

use crate::parsers::ParserConfig;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during subscription validation
#[derive(Error, Debug, Clone, PartialEq)]
pub enum SubscriptionError {
    /// Stream ID cannot be empty
    #[error("stream_id cannot be empty")]
    EmptyStreamId,

    /// Topic pattern cannot be empty
    #[error("topic_pattern cannot be empty")]
    EmptyTopicPattern,

    /// Invalid topic pattern syntax
    #[error("invalid topic pattern: {0}")]
    InvalidTopicPattern(String),
}

/// Helper function for serde default for enabled field
fn default_true() -> bool {
    true
}

/// Configuration for a single MQTT subscription
///
/// Each subscription maps an MQTT topic pattern to a stream, with optional
/// parser configuration for customizing how messages are parsed.
///
/// # Example
///
/// ```rust
/// use neural_core::sources::mqtt::SubscriptionConfig;
///
/// let sub = SubscriptionConfig::new("air-quality", "airgradient/readings/+");
/// assert_eq!(sub.stream_id, "air-quality");
/// assert!(sub.enabled);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionConfig {
    /// Unique identifier for the stream this subscription feeds
    pub stream_id: String,

    /// MQTT topic pattern (supports + and # wildcards)
    pub topic_pattern: String,

    /// Optional parser configuration override for this subscription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserConfig>,

    /// Whether this subscription is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// AIR-012: Topic segment index to extract as ndp_id (0-indexed)
    ///
    /// For event-oriented streams where each device should have its own ndp_id,
    /// this allows dynamic extraction from the topic path.
    ///
    /// Example: For topic "homeassistant/binary_sensor/door_backslider/state"
    /// with ndp_id_topic_segment: 2, the ndp_id becomes "door_backslider"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id_topic_segment: Option<usize>,
}

impl SubscriptionConfig {
    /// Create a new subscription configuration
    ///
    /// Creates a subscription with the specified stream ID and topic pattern.
    /// The subscription is enabled by default with no custom parser.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - Unique identifier for the stream
    /// * `topic_pattern` - MQTT topic pattern (supports + and # wildcards)
    ///
    /// # Example
    ///
    /// ```rust
    /// use neural_core::sources::mqtt::SubscriptionConfig;
    ///
    /// let sub = SubscriptionConfig::new("sensors", "home/+/temperature");
    /// ```
    pub fn new(stream_id: impl Into<String>, topic_pattern: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            topic_pattern: topic_pattern.into(),
            parser: None,
            enabled: true,
            ndp_id_topic_segment: None,
        }
    }

    /// Set whether this subscription is enabled
    ///
    /// Builder method that returns self for method chaining.
    ///
    /// # Example
    ///
    /// ```rust
    /// use neural_core::sources::mqtt::SubscriptionConfig;
    ///
    /// let sub = SubscriptionConfig::new("test", "test/+")
    ///     .with_enabled(false);
    /// assert!(!sub.enabled);
    /// ```
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set a custom parser configuration for this subscription
    ///
    /// Builder method that returns self for method chaining.
    ///
    /// # Example
    ///
    /// ```rust
    /// use neural_core::sources::mqtt::SubscriptionConfig;
    /// use neural_core::parsers::{ParserConfig, ParserType};
    ///
    /// let parser = ParserConfig {
    ///     parser_type: ParserType::FlatJson,
    ///     location_id_field: "id".to_string(),
    ///     ..Default::default()
    /// };
    ///
    /// let sub = SubscriptionConfig::new("test", "test/+")
    ///     .with_parser(parser);
    /// assert!(sub.parser.is_some());
    /// ```
    pub fn with_parser(mut self, parser: ParserConfig) -> Self {
        self.parser = Some(parser);
        self
    }

    /// Set the topic segment index to extract as ndp_id (AIR-012)
    ///
    /// For event-oriented streams where each device should have its own ndp_id,
    /// this allows dynamic extraction from the topic path.
    ///
    /// # Example
    ///
    /// ```rust
    /// use neural_core::sources::mqtt::SubscriptionConfig;
    ///
    /// // For topic "homeassistant/binary_sensor/door_backslider/state"
    /// // segment 2 extracts "door_backslider" as ndp_id
    /// let sub = SubscriptionConfig::new("ha-state", "homeassistant/binary_sensor/+/state")
    ///     .with_ndp_id_topic_segment(2);
    /// ```
    pub fn with_ndp_id_topic_segment(mut self, segment: usize) -> Self {
        self.ndp_id_topic_segment = Some(segment);
        self
    }

    /// Validate the subscription configuration
    ///
    /// Checks that required fields are present and valid.
    ///
    /// # Errors
    ///
    /// Returns `SubscriptionError` if:
    /// - `stream_id` is empty
    /// - `topic_pattern` is empty
    ///
    /// # Example
    ///
    /// ```rust
    /// use neural_core::sources::mqtt::SubscriptionConfig;
    ///
    /// let valid_sub = SubscriptionConfig::new("test", "test/+");
    /// assert!(valid_sub.validate().is_ok());
    ///
    /// let invalid_sub = SubscriptionConfig {
    ///     stream_id: "".to_string(),
    ///     topic_pattern: "test/+".to_string(),
    ///     parser: None,
    ///     enabled: true,
    /// };
    /// assert!(invalid_sub.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), SubscriptionError> {
        if self.stream_id.is_empty() {
            return Err(SubscriptionError::EmptyStreamId);
        }
        if self.topic_pattern.is_empty() {
            return Err(SubscriptionError::EmptyTopicPattern);
        }
        Ok(())
    }
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            stream_id: String::new(),
            topic_pattern: String::new(),
            parser: None,
            enabled: true,
            ndp_id_topic_segment: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::ParserType;

    // ==========================================================================
    // Test Sequence 1.1: Basic Struct Creation
    // ==========================================================================

    #[test]
    fn test_subscription_config_new_creates_valid_struct() {
        let sub = SubscriptionConfig::new("air-quality", "airgradient/readings/+");

        assert_eq!(sub.stream_id, "air-quality");
        assert_eq!(sub.topic_pattern, "airgradient/readings/+");
        assert!(sub.enabled);
        assert!(sub.parser.is_none());
    }

    #[test]
    fn test_subscription_config_new_accepts_string_types() {
        // Test with String
        let sub1 = SubscriptionConfig::new(String::from("stream1"), String::from("topic/+"));
        assert_eq!(sub1.stream_id, "stream1");

        // Test with &str
        let sub2 = SubscriptionConfig::new("stream2", "topic/#");
        assert_eq!(sub2.stream_id, "stream2");
    }

    #[test]
    fn test_subscription_config_new_preserves_special_characters() {
        let sub =
            SubscriptionConfig::new("home-assistant_climate", "homeassistant/climate/+/state");

        assert_eq!(sub.stream_id, "home-assistant_climate");
        assert_eq!(sub.topic_pattern, "homeassistant/climate/+/state");
    }

    // ==========================================================================
    // Test Sequence 1.2: Default Trait
    // ==========================================================================

    #[test]
    fn test_subscription_config_default_has_enabled_true() {
        let sub = SubscriptionConfig::default();

        assert!(sub.enabled);
        assert!(sub.stream_id.is_empty());
        assert!(sub.topic_pattern.is_empty());
        assert!(sub.parser.is_none());
    }

    #[test]
    fn test_subscription_config_default_can_be_modified() {
        let mut sub = SubscriptionConfig::default();
        sub.stream_id = "modified".to_string();
        sub.topic_pattern = "modified/+".to_string();

        assert_eq!(sub.stream_id, "modified");
        assert_eq!(sub.topic_pattern, "modified/+");
    }

    // ==========================================================================
    // Test Sequence 1.3: Builder Methods
    // ==========================================================================

    #[test]
    fn test_subscription_config_with_enabled_sets_value() {
        let sub = SubscriptionConfig::new("test", "test/+").with_enabled(false);

        assert!(!sub.enabled);
    }

    #[test]
    fn test_subscription_config_with_enabled_true() {
        let sub = SubscriptionConfig::new("test", "test/+")
            .with_enabled(false)
            .with_enabled(true);

        assert!(sub.enabled);
    }

    #[test]
    fn test_subscription_config_with_parser_sets_config() {
        let parser = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "id".to_string(),
            ..Default::default()
        };
        let sub = SubscriptionConfig::new("test", "test/+").with_parser(parser.clone());

        assert_eq!(sub.parser, Some(parser));
    }

    #[test]
    fn test_subscription_config_builder_chaining() {
        let parser = ParserConfig {
            parser_type: ParserType::JsonPath,
            location_id_field: "sensor_id".to_string(),
            ..Default::default()
        };

        let sub = SubscriptionConfig::new("sensors", "sensors/+/data")
            .with_enabled(true)
            .with_parser(parser.clone());

        assert_eq!(sub.stream_id, "sensors");
        assert_eq!(sub.topic_pattern, "sensors/+/data");
        assert!(sub.enabled);
        assert_eq!(sub.parser, Some(parser));
    }

    // ==========================================================================
    // Test Sequence 1.4: Serde Deserialization
    // ==========================================================================

    #[test]
    fn test_subscription_serde_deserialize_minimal() {
        let yaml = r#"
stream_id: test
topic_pattern: "test/+"
"#;
        let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(sub.stream_id, "test");
        assert_eq!(sub.topic_pattern, "test/+");
        assert!(sub.enabled); // Default true
        assert!(sub.parser.is_none());
    }

    #[test]
    fn test_subscription_serde_deserialize_with_enabled_false() {
        let yaml = r#"
stream_id: test
topic_pattern: "test/+"
enabled: false
"#;
        let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(!sub.enabled);
    }

    #[test]
    fn test_subscription_serde_deserialize_with_enabled_true() {
        let yaml = r#"
stream_id: test
topic_pattern: "test/+"
enabled: true
"#;
        let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(sub.enabled);
    }

    #[test]
    fn test_subscription_serde_serialize_roundtrip() {
        let original =
            SubscriptionConfig::new("air-quality", "airgradient/readings/+").with_enabled(false);

        let yaml = serde_yaml::to_string(&original).unwrap();
        let deserialized: SubscriptionConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_subscription_serde_deserialize_with_parser() {
        let yaml = r#"
stream_id: custom
topic_pattern: "custom/+"
parser:
  parser_type: flat_json
  location_id_field: device_id
enabled: true
"#;
        let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(sub.stream_id, "custom");
        assert!(sub.parser.is_some());
        let parser = sub.parser.unwrap();
        assert_eq!(parser.location_id_field, "device_id");
    }

    // ==========================================================================
    // Test Sequence 1.5: Validation
    // ==========================================================================

    #[test]
    fn test_validate_empty_stream_id_error() {
        let sub = SubscriptionConfig {
            stream_id: "".to_string(),
            topic_pattern: "test/+".to_string(),
            parser: None,
            enabled: true,
            ..Default::default()
        };

        assert!(matches!(
            sub.validate(),
            Err(SubscriptionError::EmptyStreamId)
        ));
    }

    #[test]
    fn test_validate_empty_topic_pattern_error() {
        let sub = SubscriptionConfig {
            stream_id: "test".to_string(),
            topic_pattern: "".to_string(),
            parser: None,
            enabled: true,
            ..Default::default()
        };

        assert!(matches!(
            sub.validate(),
            Err(SubscriptionError::EmptyTopicPattern)
        ));
    }

    #[test]
    fn test_validate_valid_subscription_ok() {
        let sub = SubscriptionConfig::new("test", "test/+");

        assert!(sub.validate().is_ok());
    }

    #[test]
    fn test_validate_disabled_subscription_still_validates() {
        let sub = SubscriptionConfig::new("test", "test/+").with_enabled(false);

        // Disabled subscriptions should still validate their fields
        assert!(sub.validate().is_ok());
    }

    #[test]
    fn test_validate_with_wildcards_ok() {
        let sub_single = SubscriptionConfig::new("single", "sensors/+/temp");
        let sub_multi = SubscriptionConfig::new("multi", "sensors/#");
        let sub_combined = SubscriptionConfig::new("combined", "home/+/devices/#");

        assert!(sub_single.validate().is_ok());
        assert!(sub_multi.validate().is_ok());
        assert!(sub_combined.validate().is_ok());
    }

    // ==========================================================================
    // Additional Tests: Edge Cases and Error Display
    // ==========================================================================

    #[test]
    fn test_subscription_error_display() {
        let empty_stream = SubscriptionError::EmptyStreamId;
        let empty_topic = SubscriptionError::EmptyTopicPattern;
        let invalid_pattern = SubscriptionError::InvalidTopicPattern("bad pattern".to_string());

        assert_eq!(empty_stream.to_string(), "stream_id cannot be empty");
        assert_eq!(empty_topic.to_string(), "topic_pattern cannot be empty");
        assert_eq!(
            invalid_pattern.to_string(),
            "invalid topic pattern: bad pattern"
        );
    }

    #[test]
    fn test_subscription_config_debug() {
        let sub = SubscriptionConfig::new("test", "test/+");
        let debug_str = format!("{:?}", sub);

        assert!(debug_str.contains("SubscriptionConfig"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_subscription_config_clone() {
        let original = SubscriptionConfig::new("test", "test/+").with_enabled(false);
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_subscription_error_clone() {
        let error = SubscriptionError::EmptyStreamId;
        let cloned = error.clone();

        assert_eq!(error, cloned);
    }

    #[test]
    fn test_subscription_config_partial_eq() {
        let sub1 = SubscriptionConfig::new("test", "test/+");
        let sub2 = SubscriptionConfig::new("test", "test/+");
        let sub3 = SubscriptionConfig::new("different", "test/+");

        assert_eq!(sub1, sub2);
        assert_ne!(sub1, sub3);
    }
}
