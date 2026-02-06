//! Phase C Test Fixtures
//!
//! Test helpers for Phase C (Cross-Stream + Alignment) tests.
//! These fixtures create consistent test data following project patterns.
//!
//! # Per TEST-PLAN.md
//!
//! These helpers support:
//! - v11-005: Cross-Stream Aligned View
//! - v11-006: State Transition Materializer
//! - v11-007: Objectives Storage

use ndp_gold_ddl::config::types::{
    AggregatesConfig, FieldConfig, FieldMetricsConfig, SilverEtlConfig,
};
use ndp_gold_ddl::{
    AlignedStream, AlignmentConfig, ConfigLoader, DomainConfig, GoldDdlError, GoldEtlConfig,
    JoinStrategy, NullHandling, ObjectiveConfig, Priority, Result, StreamConfig, StreamRef,
    StreamRole, StreamType, TargetConfig,
};
use std::collections::HashMap;

// ============================================================================
// Domain Configuration Fixtures
// ============================================================================

/// Create a three-stream domain configuration for testing cross-stream alignment.
///
/// Streams:
/// - `air-quality` (alias: aq) - Primary, Observation type
/// - `outdoor-weather` (alias: ow) - Context, Observation type
/// - `home-assistant-state` (alias: se) - Actuator, StateEvent type
///
/// This is the canonical test case for Phase C aligned view generation.
pub fn create_three_stream_domain() -> DomainConfig {
    DomainConfig {
        id: "indoor-air-quality".to_string(),
        description: "Indoor air quality domain for testing".to_string(),
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "aq".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "ow".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
            StreamRef {
                stream_id: "home-assistant-state".to_string(),
                alias: "se".to_string(),
                role: StreamRole::Actuator,
                null_handling: Some(NullHandling::CarryForward),
            },
        ],
        alignment: AlignmentConfig {
            view_name: "indoor_air_quality_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![
            create_objective_config("healthy_co2", "co2", "<", 800.0),
            create_objective_config("healthy_pm25", "pm25", "<", 12.0),
        ],
        events: None,
    }
}

/// Create a two-stream domain for simpler JOIN testing.
///
/// Streams:
/// - `air-quality` (alias: indoor) - Primary
/// - `outdoor-weather` (alias: outdoor) - Context
pub fn create_two_stream_domain() -> DomainConfig {
    DomainConfig {
        id: "test-domain".to_string(),
        description: "Two stream test domain".to_string(),
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "indoor".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "outdoor".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
        ],
        alignment: AlignmentConfig {
            view_name: "test_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![],
        events: None,
    }
}

/// Create a domain with forecast stream for LATERAL JOIN testing.
///
/// Streams:
/// - `air-quality` (alias: indoor) - Primary, Observation
/// - `nws-forecast-hourly` (alias: forecast) - Context, Forecast
pub fn create_domain_with_forecast() -> DomainConfig {
    DomainConfig {
        id: "test-forecast-domain".to_string(),
        description: "Domain with forecast stream".to_string(),
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "indoor".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
            StreamRef {
                stream_id: "nws-forecast-hourly".to_string(),
                alias: "forecast".to_string(),
                role: StreamRole::Context,
                null_handling: None,
            },
        ],
        alignment: AlignmentConfig {
            view_name: "forecast_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![],
        events: None,
    }
}

// ============================================================================
// Stream Configuration Fixtures
// ============================================================================

/// Create a typed stream reference for testing.
///
/// # Arguments
///
/// * `stream_id` - The stream identifier
/// * `alias` - The alias to use in SQL
/// * `role` - The stream's role in the domain
/// * `null_handling` - Optional NULL handling override
pub fn create_stream_ref(
    stream_id: &str,
    alias: &str,
    role: StreamRole,
    null_handling: Option<NullHandling>,
) -> StreamRef {
    StreamRef {
        stream_id: stream_id.to_string(),
        alias: alias.to_string(),
        role,
        null_handling,
    }
}

/// Create an AlignedStream for testing column/join builders directly.
///
/// # Arguments
///
/// * `stream_id` - The stream identifier
/// * `alias` - The alias for SQL generation
/// * `role` - Stream role
/// * `stream_type` - Type (Observation, StateEvent, etc.)
/// * `columns` - Columns available in Gold layer
/// * `null_handling` - NULL handling strategy
pub fn create_aligned_stream(
    stream_id: &str,
    alias: &str,
    role: StreamRole,
    stream_type: StreamType,
    columns: Vec<&str>,
    null_handling: NullHandling,
) -> AlignedStream {
    AlignedStream {
        stream_id: stream_id.to_string(),
        alias: alias.to_string(),
        role,
        stream_type,
        gold_table: format!("gold.{}_hourly", stream_id.replace('-', "_")),
        columns: columns.iter().map(|s| s.to_string()).collect(),
        null_handling,
    }
}

/// Create a standard observation stream for testing.
pub fn create_observation_stream(stream_id: &str, alias: &str, role: StreamRole) -> AlignedStream {
    create_aligned_stream(
        stream_id,
        alias,
        role,
        StreamType::Observation,
        vec![
            "bucket",
            "pm25_mean",
            "pm25_std",
            "co2_mean",
            "sample_count",
        ],
        NullHandling::Preserve,
    )
}

/// Create a state event stream for testing LOCF NULL handling.
pub fn create_state_event_stream(stream_id: &str, alias: &str) -> AlignedStream {
    create_aligned_stream(
        stream_id,
        alias,
        StreamRole::Actuator,
        StreamType::StateEvent,
        vec!["bucket", "window_state", "hvac_mode", "sample_count"],
        NullHandling::CarryForward,
    )
}

/// Create a forecast stream for testing LATERAL JOIN.
pub fn create_forecast_stream(stream_id: &str, alias: &str) -> AlignedStream {
    create_aligned_stream(
        stream_id,
        alias,
        StreamRole::Context,
        StreamType::Forecast,
        vec![
            "bucket",
            "temperature_c_mean",
            "humidity_pct_mean",
            "sample_count",
        ],
        NullHandling::Preserve,
    )
}

// ============================================================================
// State Transition Configuration Fixtures
// ============================================================================

/// Configuration for state transition view generation.
///
/// This is used for testing v11-006: State Transition Materializer.
#[derive(Debug, Clone)]
pub struct TransitionConfig {
    /// Stream ID to track transitions for
    pub stream_id: String,
    /// Field containing state value
    pub state_field: String,
    /// Field for partitioning (entity identifier)
    pub entity_field: String,
    /// Whether to calculate duration in previous state
    pub track_duration: bool,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            stream_id: "home-assistant-state".to_string(),
            state_field: "state".to_string(),
            entity_field: "ndp_id".to_string(),
            track_duration: true,
        }
    }
}

/// Create a transition config for testing state transitions.
///
/// # Arguments
///
/// * `stream_id` - The stream to track transitions for
pub fn create_transition_config(stream_id: &str) -> TransitionConfig {
    TransitionConfig {
        stream_id: stream_id.to_string(),
        state_field: "state".to_string(),
        entity_field: "ndp_id".to_string(),
        track_duration: true,
    }
}

/// Create a transition config with custom fields.
pub fn create_custom_transition_config(
    stream_id: &str,
    state_field: &str,
    entity_field: &str,
    track_duration: bool,
) -> TransitionConfig {
    TransitionConfig {
        stream_id: stream_id.to_string(),
        state_field: state_field.to_string(),
        entity_field: entity_field.to_string(),
        track_duration,
    }
}

// ============================================================================
// Objective Configuration Fixtures
// ============================================================================

/// Create an objective configuration for testing.
///
/// # Arguments
///
/// * `id` - Objective identifier
/// * `metric` - The metric to monitor
/// * `condition` - Comparison operator (<, >, <=, >=, ==, !=)
/// * `threshold` - Threshold value
pub fn create_objective(
    id: &str,
    metric: &str,
    condition: &str,
    threshold: f64,
) -> ObjectiveConfig {
    create_objective_config(id, metric, condition, threshold)
}

/// Create an ObjectiveConfig (internal helper).
fn create_objective_config(
    id: &str,
    metric: &str,
    condition: &str,
    threshold: f64,
) -> ObjectiveConfig {
    ObjectiveConfig {
        id: id.to_string(),
        description: format!("Test objective for {}", metric),
        target: TargetConfig {
            stream: "air-quality".to_string(),
            metric: metric.to_string(),
            condition: condition.to_string(),
            threshold,
            unit: None,
        },
        priority: Priority::High,
    }
}

/// Create an objective with full customization.
pub fn create_full_objective(
    id: &str,
    stream: &str,
    metric: &str,
    condition: &str,
    threshold: f64,
    unit: Option<&str>,
    priority: Priority,
) -> ObjectiveConfig {
    ObjectiveConfig {
        id: id.to_string(),
        description: format!("Objective: {} {} {}", metric, condition, threshold),
        target: TargetConfig {
            stream: stream.to_string(),
            metric: metric.to_string(),
            condition: condition.to_string(),
            threshold,
            unit: unit.map(|s| s.to_string()),
        },
        priority,
    }
}

// ============================================================================
// Mock ConfigLoader for Testing
// ============================================================================

/// Mock ConfigLoader for unit testing without file system or database.
///
/// This mock implements the ConfigLoader trait and returns predefined
/// configurations. Use the builder pattern to set up expected responses.
///
/// # Example
///
/// ```rust
/// let loader = MockConfigLoader::new()
///     .with_stream("air-quality", create_gold_stream_config("air-quality"))
///     .with_stream("outdoor-weather", create_gold_stream_config("outdoor-weather"));
///
/// let config = loader.load_stream_config("air-quality").unwrap();
/// ```
pub struct MockConfigLoader {
    stream_configs: HashMap<String, StreamConfig>,
    domain_configs: HashMap<String, DomainConfig>,
}

impl Default for MockConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl MockConfigLoader {
    /// Create a new empty mock loader.
    pub fn new() -> Self {
        Self {
            stream_configs: HashMap::new(),
            domain_configs: HashMap::new(),
        }
    }

    /// Add a stream config to the mock (builder pattern).
    pub fn with_stream(mut self, stream_id: &str, config: StreamConfig) -> Self {
        self.stream_configs.insert(stream_id.to_string(), config);
        self
    }

    /// Add a domain config to the mock (builder pattern).
    pub fn with_domain(mut self, domain_id: &str, config: DomainConfig) -> Self {
        self.domain_configs.insert(domain_id.to_string(), config);
        self
    }

    /// Add a Gold-enabled stream with default configuration.
    pub fn with_gold_stream(self, stream_id: &str) -> Self {
        let config = create_gold_stream_config(stream_id);
        self.with_stream(stream_id, config)
    }
}

impl ConfigLoader for MockConfigLoader {
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
        self.stream_configs
            .get(stream_id)
            .cloned()
            .ok_or_else(|| GoldDdlError::ConfigNotFound {
                path: format!("mock:{}", stream_id),
            })
    }

    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
        self.domain_configs
            .get(domain_id)
            .cloned()
            .ok_or_else(|| GoldDdlError::ConfigNotFound {
                path: format!("mock-domain:{}", domain_id),
            })
    }
}

/// Create a StreamConfig with Gold ETL enabled for testing.
pub fn create_gold_stream_config(stream_id: &str) -> StreamConfig {
    let mut fields_map = HashMap::new();
    fields_map.insert(
        "pm25".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string(), "std".to_string()],
        },
    );
    fields_map.insert(
        "co2".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string()],
        },
    );

    StreamConfig {
        stream_id: stream_id.to_string(),
        fields: vec![
            FieldConfig {
                name: "pm25".to_string(),
                field_type: "float".to_string(),
            },
            FieldConfig {
                name: "co2".to_string(),
                field_type: "int".to_string(),
            },
        ],
        silver_etl: Some(SilverEtlConfig {
            target_table: format!("silver.{}_observations", stream_id.replace('-', "_")),
            timestamp: None,
        }),
        gold_etl: Some(GoldEtlConfig {
            enabled: true,
            aggregates: Some(AggregatesConfig {
                granularities: vec!["1 hour".to_string()],
                fields: fields_map,
            }),
            features: None,
            refresh_policy: None,
        }),
    }
}

/// Create a StreamConfig for state events (e.g., home-assistant-state).
pub fn create_state_event_stream_config(stream_id: &str) -> StreamConfig {
    StreamConfig {
        stream_id: stream_id.to_string(),
        fields: vec![
            FieldConfig {
                name: "state".to_string(),
                field_type: "string".to_string(),
            },
            FieldConfig {
                name: "entity_id".to_string(),
                field_type: "string".to_string(),
            },
        ],
        silver_etl: Some(SilverEtlConfig {
            target_table: format!("silver.{}", stream_id.replace('-', "_")),
            timestamp: None,
        }),
        gold_etl: None, // State events may not have Gold aggregates
    }
}

// ============================================================================
// SQL Assertion Helpers
// ============================================================================

/// Assert that SQL contains a specific clause (case-insensitive for keywords).
pub fn assert_sql_contains(sql: &str, expected: &str, msg: &str) {
    assert!(
        sql.contains(expected),
        "{}: Expected SQL to contain '{}'\n\nActual SQL:\n{}",
        msg,
        expected,
        sql
    );
}

/// Assert that SQL does NOT contain a specific clause.
pub fn assert_sql_not_contains(sql: &str, unexpected: &str, msg: &str) {
    assert!(
        !sql.contains(unexpected),
        "{}: Expected SQL to NOT contain '{}'\n\nActual SQL:\n{}",
        msg,
        unexpected,
        sql
    );
}

/// Count occurrences of a pattern in SQL.
pub fn count_sql_occurrences(sql: &str, pattern: &str) -> usize {
    sql.matches(pattern).count()
}

/// Assert the number of occurrences of a pattern in SQL.
pub fn assert_sql_count(sql: &str, pattern: &str, expected_count: usize, msg: &str) {
    let actual = count_sql_occurrences(sql, pattern);
    assert_eq!(
        actual, expected_count,
        "{}: Expected {} occurrences of '{}', found {}\n\nActual SQL:\n{}",
        msg, expected_count, pattern, actual, sql
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_three_stream_domain_has_correct_structure() {
        let domain = create_three_stream_domain();

        assert_eq!(domain.id, "indoor-air-quality");
        assert_eq!(domain.streams.len(), 3);

        // Verify primary stream
        let primary = domain
            .streams
            .iter()
            .find(|s| s.role == StreamRole::Primary);
        assert!(primary.is_some());
        assert_eq!(primary.unwrap().stream_id, "air-quality");

        // Verify alignment config
        assert_eq!(domain.alignment.join_strategy, JoinStrategy::FullOuter);
        assert_eq!(domain.alignment.granularity, "1 hour");
    }

    #[test]
    fn test_create_transition_config_defaults() {
        let config = create_transition_config("test-stream");

        assert_eq!(config.stream_id, "test-stream");
        assert_eq!(config.state_field, "state");
        assert_eq!(config.entity_field, "ndp_id");
        assert!(config.track_duration);
    }

    #[test]
    fn test_create_objective_structure() {
        let obj = create_objective("test_obj", "co2", "<", 800.0);

        assert_eq!(obj.id, "test_obj");
        assert_eq!(obj.target.metric, "co2");
        assert_eq!(obj.target.condition, "<");
        assert_eq!(obj.target.threshold, 800.0);
        assert_eq!(obj.priority, Priority::High);
    }

    #[test]
    fn test_mock_config_loader_returns_stream() {
        let loader = MockConfigLoader::new().with_gold_stream("air-quality");

        let result = loader.load_stream_config("air-quality");
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.stream_id, "air-quality");
        assert!(config.gold_etl.is_some());
    }

    #[test]
    fn test_mock_config_loader_not_found() {
        let loader = MockConfigLoader::new();

        let result = loader.load_stream_config("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_sql_assertion_helpers() {
        let sql = "SELECT * FROM gold.test FULL OUTER JOIN gold.other ON a.bucket = b.bucket";

        assert_sql_contains(sql, "FULL OUTER JOIN", "Should find join");
        assert_sql_not_contains(sql, "LEFT JOIN", "Should not find left join");
        assert_sql_count(sql, "bucket", 2, "Should have 2 bucket references");
    }
}
