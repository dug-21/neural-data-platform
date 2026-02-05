// FE-001 Phase B v11-001: Stream Type Classification
//
// This module provides stream type classification for correlation analysis.
// StreamType categorizes data streams to enable intelligent correlation detection
// between observation data (effects) and event data (causes).

use serde::{Deserialize, Serialize};

/// Classification of data stream types for correlation analysis.
///
/// Stream types are used to categorize data streams for intelligent
/// correlation detection. Each type maps to a correlation role:
/// - Observation → effect (what changed)
/// - StateEvent → cause (what triggered)
/// - Forecast → context (predictive context)
/// - Dimension → metadata (reference data)
///
/// # Example
///
/// ```
/// use neural_core::types::StreamType;
///
/// let stream_type: StreamType = serde_json::from_str(r#""observation""#).unwrap();
/// assert_eq!(stream_type, StreamType::Observation);
/// assert_eq!(stream_type.correlation_role(), "effect");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    /// Continuous numeric readings (PM2.5, temperature, etc.)
    ///
    /// Observations are time-series measurements that represent the "effect"
    /// in cause-effect relationships. They are what we're trying to explain
    /// or predict.
    Observation,

    /// Binary/discrete state changes (door open/close, HVAC on/off)
    ///
    /// State events represent the "cause" in cause-effect relationships.
    /// Changes in state often trigger changes in observation data.
    StateEvent,

    /// Future predictions from external source (NWS forecast)
    ///
    /// Forecasts provide predictive context for understanding future
    /// correlation opportunities and expected patterns.
    Forecast,

    /// Slowly changing reference data (device metadata, calibration)
    ///
    /// Dimensions are reference tables that provide metadata context
    /// for other streams but don't directly participate in correlations.
    Dimension,
}

impl StreamType {
    /// Map stream type to correlation role for V1.2 pattern detection.
    ///
    /// Returns a static string representing the role this stream type
    /// plays in correlation analysis:
    /// - `"effect"` - Observation data (what changed)
    /// - `"cause"` - State events (what triggered)
    /// - `"context"` - Forecast data (predictive context)
    /// - `"metadata"` - Dimension data (reference information)
    ///
    /// # Example
    ///
    /// ```
    /// use neural_core::types::StreamType;
    ///
    /// assert_eq!(StreamType::Observation.correlation_role(), "effect");
    /// assert_eq!(StreamType::StateEvent.correlation_role(), "cause");
    /// assert_eq!(StreamType::Forecast.correlation_role(), "context");
    /// assert_eq!(StreamType::Dimension.correlation_role(), "metadata");
    /// ```
    pub fn correlation_role(&self) -> &'static str {
        match self {
            StreamType::Observation => "effect",
            StreamType::StateEvent => "cause",
            StreamType::Forecast => "context",
            StreamType::Dimension => "metadata",
        }
    }
}

impl Default for StreamType {
    /// Default to Observation as the most common stream type.
    fn default() -> Self {
        StreamType::Observation
    }
}

impl std::fmt::Display for StreamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamType::Observation => write!(f, "observation"),
            StreamType::StateEvent => write!(f, "state_event"),
            StreamType::Forecast => write!(f, "forecast"),
            StreamType::Dimension => write!(f, "dimension"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========== TDD CYCLE 1: StreamType Enum Deserialization ==========

    #[test]
    fn test_stream_type_deserializes_observation() {
        // Arrange
        let json = json!("observation");

        // Act
        let result: Result<StreamType, _> = serde_json::from_value(json);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StreamType::Observation);
    }

    #[test]
    fn test_stream_type_deserializes_state_event() {
        // Arrange
        let json = json!("state_event");

        // Act
        let result: Result<StreamType, _> = serde_json::from_value(json);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StreamType::StateEvent);
    }

    #[test]
    fn test_stream_type_deserializes_forecast() {
        // Arrange
        let json = json!("forecast");

        // Act
        let result: Result<StreamType, _> = serde_json::from_value(json);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StreamType::Forecast);
    }

    #[test]
    fn test_stream_type_deserializes_dimension() {
        // Arrange
        let json = json!("dimension");

        // Act
        let result: Result<StreamType, _> = serde_json::from_value(json);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StreamType::Dimension);
    }

    #[test]
    fn test_stream_type_serializes_to_snake_case() {
        // Verify serialization produces snake_case
        assert_eq!(
            serde_json::to_string(&StreamType::Observation).unwrap(),
            "\"observation\""
        );
        assert_eq!(
            serde_json::to_string(&StreamType::StateEvent).unwrap(),
            "\"state_event\""
        );
        assert_eq!(
            serde_json::to_string(&StreamType::Forecast).unwrap(),
            "\"forecast\""
        );
        assert_eq!(
            serde_json::to_string(&StreamType::Dimension).unwrap(),
            "\"dimension\""
        );
    }

    #[test]
    fn test_stream_type_invalid_value_fails() {
        // Arrange
        let json = json!("invalid_type");

        // Act
        let result: Result<StreamType, _> = serde_json::from_value(json);

        // Assert
        assert!(result.is_err());
    }

    // ========== TDD CYCLE 3: Correlation Role Mapping ==========

    #[test]
    fn test_observation_type_is_effect_role() {
        assert_eq!(StreamType::Observation.correlation_role(), "effect");
    }

    #[test]
    fn test_state_event_type_is_cause_role() {
        assert_eq!(StreamType::StateEvent.correlation_role(), "cause");
    }

    #[test]
    fn test_forecast_type_is_context_role() {
        assert_eq!(StreamType::Forecast.correlation_role(), "context");
    }

    #[test]
    fn test_dimension_type_is_metadata_role() {
        assert_eq!(StreamType::Dimension.correlation_role(), "metadata");
    }

    // ========== Additional Tests: Display and Default ==========

    #[test]
    fn test_stream_type_display() {
        assert_eq!(StreamType::Observation.to_string(), "observation");
        assert_eq!(StreamType::StateEvent.to_string(), "state_event");
        assert_eq!(StreamType::Forecast.to_string(), "forecast");
        assert_eq!(StreamType::Dimension.to_string(), "dimension");
    }

    #[test]
    fn test_stream_type_default() {
        assert_eq!(StreamType::default(), StreamType::Observation);
    }

    #[test]
    fn test_stream_type_clone() {
        let original = StreamType::StateEvent;
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_stream_type_debug() {
        let debug_str = format!("{:?}", StreamType::Observation);
        assert_eq!(debug_str, "Observation");
    }
}
