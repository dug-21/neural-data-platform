//! Domain configuration types for aligned views
//!
//! Defines structures for domain-centric configuration that joins
//! multiple streams into aligned views.

use serde::{Deserialize, Serialize};

use crate::generators::events::EventsConfig;

/// Domain configuration for cross-stream alignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfig {
    /// Domain identifier (e.g., "indoor-air-quality")
    pub id: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Streams to include in this domain
    pub streams: Vec<StreamRef>,

    /// Alignment configuration for the view
    pub alignment: AlignmentConfig,

    /// Optional objectives for pattern detection
    #[serde(default)]
    pub objectives: Vec<ObjectiveConfig>,

    /// Optional events infrastructure configuration
    #[serde(default)]
    pub events: Option<EventsConfig>,
}

/// Reference to a stream within a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRef {
    /// Stream identifier (matches stream_id in config)
    pub stream_id: String,

    /// Alias for this stream in the aligned view (e.g., "indoor", "outdoor")
    pub alias: String,

    /// Role of this stream in the domain
    #[serde(default)]
    pub role: StreamRole,

    /// Override null handling for this stream (optional)
    #[serde(default)]
    pub null_handling: Option<NullHandling>,
}

/// Role of a stream within a domain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamRole {
    /// Primary stream - always first in JOINs, dictates the base time range
    #[default]
    Primary,
    /// Context stream - provides environmental context
    Context,
    /// Actuator stream - records user/system actions
    Actuator,
    /// Constraint stream - provides boundaries or limits
    Constraint,
}

impl StreamRole {
    /// Get the sort order for this role (lower = earlier in join)
    pub fn sort_order(&self) -> u8 {
        match self {
            StreamRole::Primary => 0,
            StreamRole::Context => 1,
            StreamRole::Actuator => 2,
            StreamRole::Constraint => 3,
        }
    }
}

/// Stream type for determining join and null handling behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    /// Observation - direct measurement at a point in time
    #[default]
    Observation,
    /// State event - state changed at a point in time
    StateEvent,
    /// Forecast - prediction for a future time
    Forecast,
    /// Dimension - slow-changing reference data
    Dimension,
}

impl StreamType {
    /// Get the default null handling strategy for this stream type
    pub fn default_null_handling(&self) -> NullHandling {
        match self {
            StreamType::Observation => NullHandling::Preserve,
            StreamType::StateEvent => NullHandling::CarryForward,
            StreamType::Forecast => NullHandling::Preserve,
            StreamType::Dimension => NullHandling::CarryForward,
        }
    }

    /// Map stream type to correlation role for V1.2 pattern detection.
    ///
    /// Returns a static string representing the role this stream type
    /// plays in correlation analysis:
    /// - `"effect"` - Observation data (what changed)
    /// - `"cause"` - State events (what triggered)
    /// - `"context"` - Forecast data (predictive context)
    /// - `"metadata"` - Dimension data (reference information)
    pub fn correlation_role(&self) -> &'static str {
        match self {
            StreamType::Observation => "effect",
            StreamType::StateEvent => "cause",
            StreamType::Forecast => "context",
            StreamType::Dimension => "metadata",
        }
    }

    /// Get the null handling strategy as a string for SQL generation.
    ///
    /// Returns:
    /// - `"preserve"` for Observation and Forecast (keep NULLs)
    /// - `"carry_forward"` for StateEvent and Dimension (LOCF)
    pub fn null_handling(&self) -> &'static str {
        match self {
            StreamType::Observation | StreamType::Forecast => "preserve",
            StreamType::StateEvent | StreamType::Dimension => "carry_forward",
        }
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

/// Alignment configuration for the view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentConfig {
    /// Name of the generated view (e.g., "indoor_air_quality_aligned")
    pub view_name: String,

    /// Time granularity for alignment (e.g., "1 hour")
    pub granularity: String,

    /// Join strategy for combining streams
    #[serde(default)]
    pub join_strategy: JoinStrategy,

    /// Default null handling strategy
    #[serde(default)]
    pub null_handling: NullHandling,
}

/// Join strategy for combining streams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinStrategy {
    /// Full outer join - includes all rows from all streams
    #[default]
    FullOuter,
    /// Left join - preserves all rows from primary stream
    Left,
    /// Inner join - only rows present in all streams
    Inner,
}

impl JoinStrategy {
    /// Get the SQL keyword for this join strategy
    pub fn sql_keyword(&self) -> &'static str {
        match self {
            JoinStrategy::FullOuter => "FULL OUTER JOIN",
            JoinStrategy::Left => "LEFT JOIN",
            JoinStrategy::Inner => "INNER JOIN",
        }
    }
}

/// Null handling strategy for aligned views
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullHandling {
    /// Preserve nulls as-is (honest representation)
    #[default]
    Preserve,
    /// Carry forward last known value (LOCF)
    CarryForward,
    /// Linear interpolation between known values
    Interpolate,
}

/// Objective configuration for pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveConfig {
    /// Objective identifier
    pub id: String,

    /// Description of the objective
    #[serde(default)]
    pub description: String,

    /// Target definition
    pub target: TargetConfig,

    /// Priority level
    #[serde(default)]
    pub priority: Priority,
}

/// Target configuration for an objective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    /// Stream to monitor
    pub stream: String,

    /// Metric to evaluate
    pub metric: String,

    /// Comparison condition
    pub condition: String,

    /// Threshold value
    pub threshold: f64,

    /// Optional unit
    #[serde(default)]
    pub unit: Option<String>,
}

/// Priority level for objectives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

/// Metadata about an aligned stream for SQL generation
#[derive(Debug, Clone)]
pub struct AlignedStream {
    /// Stream identifier
    pub stream_id: String,

    /// Alias in the view
    pub alias: String,

    /// Role in the domain
    pub role: StreamRole,

    /// Stream type (observation, state_event, forecast, dimension)
    pub stream_type: StreamType,

    /// Gold table name (e.g., "gold.air_quality_hourly")
    pub gold_table: String,

    /// Available columns in the Gold layer
    pub columns: Vec<String>,

    /// Null handling strategy for this stream
    pub null_handling: NullHandling,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_role_sort_order() {
        assert_eq!(StreamRole::Primary.sort_order(), 0);
        assert_eq!(StreamRole::Context.sort_order(), 1);
        assert_eq!(StreamRole::Actuator.sort_order(), 2);
        assert_eq!(StreamRole::Constraint.sort_order(), 3);
    }

    #[test]
    fn test_stream_type_default_null_handling() {
        assert_eq!(
            StreamType::Observation.default_null_handling(),
            NullHandling::Preserve
        );
        assert_eq!(
            StreamType::StateEvent.default_null_handling(),
            NullHandling::CarryForward
        );
        assert_eq!(
            StreamType::Forecast.default_null_handling(),
            NullHandling::Preserve
        );
        assert_eq!(
            StreamType::Dimension.default_null_handling(),
            NullHandling::CarryForward
        );
    }

    #[test]
    fn test_join_strategy_sql_keyword() {
        assert_eq!(JoinStrategy::FullOuter.sql_keyword(), "FULL OUTER JOIN");
        assert_eq!(JoinStrategy::Left.sql_keyword(), "LEFT JOIN");
        assert_eq!(JoinStrategy::Inner.sql_keyword(), "INNER JOIN");
    }

    #[test]
    fn test_domain_config_deserialize() {
        let json = r#"{
            "id": "indoor-air-quality",
            "description": "Indoor air quality monitoring domain",
            "streams": [
                {
                    "stream_id": "air-quality",
                    "alias": "indoor",
                    "role": "primary"
                },
                {
                    "stream_id": "outdoor-weather",
                    "alias": "outdoor",
                    "role": "context"
                },
                {
                    "stream_id": "home-assistant-state",
                    "alias": "state",
                    "role": "actuator"
                }
            ],
            "alignment": {
                "view_name": "indoor_air_quality_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer",
                "null_handling": "preserve"
            }
        }"#;

        let config: DomainConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.id, "indoor-air-quality");
        assert_eq!(config.streams.len(), 3);
        assert_eq!(config.streams[0].alias, "indoor");
        assert_eq!(config.streams[0].role, StreamRole::Primary);
        assert_eq!(config.alignment.view_name, "indoor_air_quality_aligned");
        assert_eq!(config.alignment.join_strategy, JoinStrategy::FullOuter);
    }

    #[test]
    fn test_stream_ref_with_null_handling_override() {
        let json = r#"{
            "stream_id": "home-assistant-state",
            "alias": "state",
            "role": "actuator",
            "null_handling": "carry_forward"
        }"#;

        let stream_ref: StreamRef = serde_json::from_str(json).unwrap();
        assert_eq!(stream_ref.null_handling, Some(NullHandling::CarryForward));
    }

    #[test]
    fn test_objective_config_deserialize() {
        let json = r#"{
            "id": "healthy_co2",
            "description": "Keep CO2 below healthy threshold",
            "target": {
                "stream": "air-quality",
                "metric": "co2",
                "condition": "<",
                "threshold": 800,
                "unit": "ppm"
            },
            "priority": "high"
        }"#;

        let objective: ObjectiveConfig = serde_json::from_str(json).unwrap();
        assert_eq!(objective.id, "healthy_co2");
        assert_eq!(objective.target.threshold, 800.0);
        assert_eq!(objective.priority, Priority::High);
    }

    // ========== v11-014: Events Config Deserialization Tests ==========

    #[test]
    fn test_domain_config_with_events_deserialize() {
        let json = r#"{
            "id": "indoor-air-quality",
            "description": "Test domain",
            "streams": [
                { "stream_id": "air-quality", "alias": "indoor", "role": "primary" },
                { "stream_id": "outdoor-weather", "alias": "outdoor", "role": "context" }
            ],
            "alignment": {
                "view_name": "indoor_air_quality_aligned",
                "granularity": "1 hour"
            },
            "events": {
                "enabled": true,
                "chunk_interval": "14 days",
                "retention": "2 years",
                "detection_schedule": "30 minutes"
            }
        }"#;

        let config: DomainConfig = serde_json::from_str(json).unwrap();
        assert!(config.events.is_some());
        let events = config.events.unwrap();
        assert!(events.enabled);
        assert_eq!(events.chunk_interval, "14 days");
        assert_eq!(events.retention, Some("2 years".to_string()));
        assert_eq!(events.detection_schedule, "30 minutes");
    }

    #[test]
    fn test_domain_config_without_events_defaults_to_none() {
        let json = r#"{
            "id": "indoor-air-quality",
            "description": "Test domain",
            "streams": [
                { "stream_id": "air-quality", "alias": "indoor", "role": "primary" },
                { "stream_id": "outdoor-weather", "alias": "outdoor", "role": "context" }
            ],
            "alignment": {
                "view_name": "indoor_air_quality_aligned",
                "granularity": "1 hour"
            }
        }"#;

        let config: DomainConfig = serde_json::from_str(json).unwrap();
        assert!(config.events.is_none());
    }

    // ========== v11-002: Correlation Role Mapping Tests ==========

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

    // ========== v11-002: Null Handling Mapping Tests ==========

    #[test]
    fn test_observation_has_preserve_null_handling() {
        assert_eq!(StreamType::Observation.null_handling(), "preserve");
    }

    #[test]
    fn test_state_event_has_carry_forward_null_handling() {
        assert_eq!(StreamType::StateEvent.null_handling(), "carry_forward");
    }

    #[test]
    fn test_forecast_has_preserve_null_handling() {
        assert_eq!(StreamType::Forecast.null_handling(), "preserve");
    }

    #[test]
    fn test_dimension_has_carry_forward_null_handling() {
        assert_eq!(StreamType::Dimension.null_handling(), "carry_forward");
    }

    // ========== v11-002: Display Trait Tests ==========

    #[test]
    fn test_stream_type_display() {
        assert_eq!(StreamType::Observation.to_string(), "observation");
        assert_eq!(StreamType::StateEvent.to_string(), "state_event");
        assert_eq!(StreamType::Forecast.to_string(), "forecast");
        assert_eq!(StreamType::Dimension.to_string(), "dimension");
    }
}
