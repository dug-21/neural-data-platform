//! Source type definitions for NDP data ingestion.
//!
//! This module defines the supported data source types for the Bronze layer.
//! Each variant corresponds to a specific data ingestion pattern.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

/// Data source types supported by NDP.
///
/// Each variant corresponds to a specific data ingestion pattern
/// for the Bronze layer. Source types determine how data is collected
/// and the required configuration parameters.
///
/// # Variants
///
/// - `Mqtt`: Subscribe to MQTT broker topics for real-time sensor data
/// - `HttpPoll`: Poll HTTP endpoints at configured intervals
/// - `Webhook`: Receive push notifications via HTTP webhook
/// - `FileWatch`: Monitor filesystem for new/modified files
/// - `Csv`: Import data from CSV files (batch processing)
///
/// # Example
///
/// ```rust
/// use ndp_types::SourceType;
///
/// let source_type = SourceType::Mqtt;
/// assert_eq!(source_type.as_ref(), "mqtt");
///
/// // Parse from string
/// let parsed: SourceType = "http_poll".parse().unwrap();
/// assert_eq!(parsed, SourceType::HttpPoll);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    EnumIter,
    EnumString,
    Display,
    AsRefStr,
    VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SourceType {
    /// MQTT broker subscription for real-time sensor data.
    ///
    /// Requires configuration: broker URL, topic pattern, QoS level.
    Mqtt,

    /// HTTP endpoint polling for periodic data fetches.
    ///
    /// Requires configuration: URL, poll interval, optional auth headers.
    HttpPoll,

    /// HTTP webhook receiver for push-based data delivery.
    ///
    /// Requires configuration: endpoint path, optional auth validation.
    Webhook,

    /// File system watcher for local file ingestion.
    ///
    /// Requires configuration: watch path, file pattern, poll interval.
    FileWatch,

    /// CSV file import for batch data loading.
    ///
    /// Requires configuration: file path, delimiter, timestamp column.
    /// Added in dp-013.
    Csv,
}

impl SourceType {
    /// Returns all source type names as a static slice.
    ///
    /// This is the single source of truth for supported source types.
    /// Use this method instead of hardcoded string arrays.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ndp_types::SourceType;
    ///
    /// let names = SourceType::all_names();
    /// assert!(names.contains(&"mqtt"));
    /// assert!(names.contains(&"csv"));
    /// ```
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all source type variants.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ndp_types::SourceType;
    ///
    /// for source_type in SourceType::all() {
    ///     println!("Supported: {}", source_type);
    /// }
    /// ```
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Check if this source type supports real-time streaming.
    pub fn is_streaming(&self) -> bool {
        matches!(self, SourceType::Mqtt | SourceType::Webhook)
    }

    /// Check if this source type is poll-based.
    pub fn is_polling(&self) -> bool {
        matches!(self, SourceType::HttpPoll | SourceType::FileWatch)
    }

    /// Check if this source type is batch-oriented.
    pub fn is_batch(&self) -> bool {
        matches!(self, SourceType::Csv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_serialization_roundtrip() {
        for variant in SourceType::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: SourceType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_all_names_count() {
        assert_eq!(SourceType::all_names().len(), 5);
    }

    #[test]
    fn test_all_names_contains_expected() {
        let names = SourceType::all_names();
        assert!(names.contains(&"mqtt"));
        assert!(names.contains(&"http_poll"));
        assert!(names.contains(&"webhook"));
        assert!(names.contains(&"file_watch"));
        assert!(names.contains(&"csv"));
    }

    #[test]
    fn test_parse_from_string() {
        assert_eq!("mqtt".parse::<SourceType>().unwrap(), SourceType::Mqtt);
        assert_eq!(
            "http_poll".parse::<SourceType>().unwrap(),
            SourceType::HttpPoll
        );
        assert_eq!(
            "webhook".parse::<SourceType>().unwrap(),
            SourceType::Webhook
        );
        assert_eq!(
            "file_watch".parse::<SourceType>().unwrap(),
            SourceType::FileWatch
        );
        assert_eq!("csv".parse::<SourceType>().unwrap(), SourceType::Csv);
    }

    #[test]
    fn test_parse_invalid_returns_error() {
        assert!("invalid".parse::<SourceType>().is_err());
        assert!("MQTT".parse::<SourceType>().is_err()); // Case-sensitive
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", SourceType::Mqtt), "mqtt");
        assert_eq!(format!("{}", SourceType::HttpPoll), "http_poll");
    }

    #[test]
    fn test_as_ref() {
        assert_eq!(SourceType::Mqtt.as_ref(), "mqtt");
        assert_eq!(SourceType::HttpPoll.as_ref(), "http_poll");
    }

    #[test]
    fn test_streaming_vs_polling() {
        assert!(SourceType::Mqtt.is_streaming());
        assert!(SourceType::Webhook.is_streaming());
        assert!(!SourceType::HttpPoll.is_streaming());

        assert!(SourceType::HttpPoll.is_polling());
        assert!(SourceType::FileWatch.is_polling());
        assert!(!SourceType::Mqtt.is_polling());

        assert!(SourceType::Csv.is_batch());
        assert!(!SourceType::Mqtt.is_batch());
    }

    #[test]
    fn test_json_schema_generated() {
        let schema = schemars::schema_for!(SourceType);
        let schema_json = serde_json::to_value(&schema).unwrap();

        // schemars generates JSON Schema with $schema, title, and other metadata
        // The actual enum definition might be nested differently based on version
        // Check that it's a valid schema with expected content
        assert!(schema_json.is_object(), "Schema should be an object");

        // For simple enums, schemars may use oneOf or enum at the top level
        // or within a definition. Let's check for the enum values somewhere in the schema.
        let schema_str = serde_json::to_string(&schema_json).unwrap();
        assert!(
            schema_str.contains("\"mqtt\""),
            "Schema should contain mqtt variant"
        );
        assert!(
            schema_str.contains("\"csv\""),
            "Schema should contain csv variant"
        );
        assert!(
            schema_str.contains("\"http_poll\""),
            "Schema should contain http_poll variant"
        );
        assert!(
            schema_str.contains("\"webhook\""),
            "Schema should contain webhook variant"
        );
        assert!(
            schema_str.contains("\"file_watch\""),
            "Schema should contain file_watch variant"
        );
    }
}
