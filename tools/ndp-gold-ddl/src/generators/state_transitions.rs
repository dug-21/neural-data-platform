//! State transition DDL generator
//!
//! Generates SQL for state transition extraction from state_event streams.
//! Implements v11-006 per SPEC-C02.
//!
//! State transitions are extracted using LAG window functions to:
//! - Detect when state actually changed (vs. duplicate events)
//! - Calculate duration in previous state
//! - Track transition direction (from_state -> to_state)

use crate::config::{Action, StreamConfig};
use crate::error::{GoldDdlError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for state transition extraction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransitionConfig {
    /// Whether transitions are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Field containing the state value
    #[serde(default)]
    pub state_field: String,

    /// Field containing the entity identifier for partitioning
    #[serde(default)]
    pub entity_field: String,

    /// Whether to track duration in previous state
    #[serde(default)]
    pub track_duration: bool,

    /// Whether to include in aligned view aggregation
    #[serde(default)]
    pub include_in_alignment: bool,

    /// Optional mapping for transition direction (e.g., "off_to_on" -> "opening")
    #[serde(default)]
    pub direction_mapping: Option<HashMap<String, String>>,
}

impl TransitionConfig {
    /// Create a new transition config with required fields
    pub fn new(state_field: &str, entity_field: &str) -> Self {
        Self {
            enabled: true,
            state_field: state_field.to_string(),
            entity_field: entity_field.to_string(),
            track_duration: true,
            include_in_alignment: true,
            direction_mapping: None,
        }
    }

    /// Create from stream config's gold_etl.features.transitions section
    pub fn from_stream_config(config: &StreamConfig) -> Option<Self> {
        config
            .gold_etl
            .as_ref()
            .and_then(|g| g.features.as_ref())
            .and_then(|f| f.transitions.as_ref())
            .map(|t| TransitionConfig {
                enabled: t.enabled,
                state_field: t.field.clone(),
                entity_field: "ndp_id".to_string(), // Default entity field
                track_duration: true,
                include_in_alignment: true,
                direction_mapping: None,
            })
    }
}

/// Generator for state transition DDL
pub struct StateTransitionGenerator {
    /// Stream ID being processed
    stream_id: String,
    /// Source table (from silver_etl.target_table)
    source_table: String,
    /// Timestamp column name
    timestamp_column: String,
}

impl StateTransitionGenerator {
    /// Create a new generator from stream configuration
    pub fn from_stream_config(config: &StreamConfig) -> Result<Self> {
        let source_table = config
            .silver_etl
            .as_ref()
            .map(|s| s.target_table.clone())
            .ok_or_else(|| GoldDdlError::MissingRequiredField {
                field: "silver_etl.target_table".to_string(),
                context: format!("stream '{}'", config.stream_id),
            })?;

        let timestamp_column = config
            .silver_etl
            .as_ref()
            .and_then(|s| s.timestamp.as_ref())
            .map(|t| t.target_field.clone())
            .unwrap_or_else(|| "event_time".to_string());

        Ok(Self {
            stream_id: config.stream_id.clone(),
            source_table,
            timestamp_column,
        })
    }

    /// Create a new generator with explicit parameters (for testing)
    pub fn new(stream_id: &str, source_table: &str, timestamp_column: &str) -> Self {
        Self {
            stream_id: stream_id.to_string(),
            source_table: source_table.to_string(),
            timestamp_column: timestamp_column.to_string(),
        }
    }

    /// Generate complete DDL for state transitions view
    pub fn generate(&self, transition_config: &TransitionConfig, action: Action) -> Result<String> {
        if !transition_config.enabled {
            return Err(GoldDdlError::GenerationFailed {
                message: format!("Transitions not enabled for stream '{}'", self.stream_id),
            });
        }

        if transition_config.state_field.is_empty() {
            return Err(GoldDdlError::MissingRequiredField {
                field: "state_field".to_string(),
                context: format!("transitions config for stream '{}'", self.stream_id),
            });
        }

        if transition_config.entity_field.is_empty() {
            return Err(GoldDdlError::MissingRequiredField {
                field: "entity_field".to_string(),
                context: format!("transitions config for stream '{}'", self.stream_id),
            });
        }

        let view_name = self.get_view_name();
        let create_sql = self.generate_create_view(transition_config)?;

        match action {
            Action::Sync => self.wrap_sync_mode(&view_name, &create_sql),
            Action::Recreate => self.wrap_recreate_mode(&view_name, &create_sql),
        }
    }

    /// Get the transition view name
    fn get_view_name(&self) -> String {
        format!("gold.{}_transitions", self.stream_id.replace('-', "_"))
    }

    /// Generate the CREATE MATERIALIZED VIEW statement
    fn generate_create_view(&self, config: &TransitionConfig) -> Result<String> {
        let view_name = self.get_view_name();
        let window_def = format!(
            "PARTITION BY {} ORDER BY {}",
            config.entity_field, self.timestamp_column
        );

        let mut columns = vec![
            format!("{} AS transition_time", self.timestamp_column),
            format!("{} AS entity_id", config.entity_field),
            format!("'{}' AS stream_id", self.stream_id),
            String::new(), // Empty line for formatting
            "-- State transition details".to_string(),
            format!("LAG({}) OVER w AS from_state", config.state_field),
            format!("{} AS to_state", config.state_field),
            String::new(),
            "-- Is this an actual state change?".to_string(),
            self.generate_is_actual_transition(&config.state_field),
        ];

        // Add duration columns if enabled
        if config.track_duration {
            columns.push(String::new());
            columns.push("-- Duration in previous state".to_string());
            columns.push(self.generate_duration_columns());
        }

        // Add transition direction if mapping provided
        if let Some(ref mapping) = config.direction_mapping {
            columns.push(String::new());
            columns.push("-- Transition direction (for binary on/off states)".to_string());
            columns.push(self.generate_direction_case(&config.state_field, mapping));
        } else {
            // Default direction logic for on/off states
            columns.push(String::new());
            columns.push("-- Transition direction (for binary on/off states)".to_string());
            columns.push(self.generate_default_direction_case(&config.state_field));
        }

        // Add device type derivation from entity field
        columns.push(String::new());
        columns.push("-- Device type derived from entity_id".to_string());
        columns.push(self.generate_device_type_case(&config.entity_field));

        // Build the full SQL
        let column_sql = columns
            .iter()
            .filter(|c| !c.is_empty())
            .map(|c| format!("    {}", c))
            .collect::<Vec<_>>()
            .join(",\n");

        Ok(format!(
            r#"CREATE MATERIALIZED VIEW {view_name} AS
SELECT
{column_sql}
FROM {source_table}
WINDOW w AS ({window_def});

-- Index for efficient queries
CREATE INDEX IF NOT EXISTS idx_{view_name_short}_time
    ON {view_name} (transition_time DESC);

CREATE INDEX IF NOT EXISTS idx_{view_name_short}_entity
    ON {view_name} (entity_id, transition_time DESC);

-- Filtered view for only actual transitions
CREATE VIEW {view_name}_actual AS
SELECT * FROM {view_name}
WHERE is_actual_transition = TRUE;

COMMENT ON MATERIALIZED VIEW {view_name} IS
    'State transitions extracted from {stream_id} stream. Refresh with Gold layer.';"#,
            view_name = view_name,
            column_sql = column_sql,
            source_table = self.source_table,
            window_def = window_def,
            view_name_short = self.stream_id.replace('-', "_"),
            stream_id = self.stream_id,
        ))
    }

    /// Generate the is_actual_transition CASE expression
    fn generate_is_actual_transition(&self, state_field: &str) -> String {
        format!(
            r#"CASE
        WHEN LAG({state}) OVER w IS DISTINCT FROM {state} THEN TRUE
        WHEN LAG({state}) OVER w IS NULL THEN TRUE
        ELSE FALSE
    END AS is_actual_transition"#,
            state = state_field
        )
    }

    /// Generate duration columns
    fn generate_duration_columns(&self) -> String {
        format!(
            r#"{timestamp} - LAG({timestamp}) OVER w AS duration_in_previous_state,
    EXTRACT(EPOCH FROM ({timestamp} - LAG({timestamp}) OVER w)) * 1000 AS duration_ms"#,
            timestamp = self.timestamp_column
        )
    }

    /// Generate direction CASE with custom mapping
    fn generate_direction_case(
        &self,
        state_field: &str,
        mapping: &HashMap<String, String>,
    ) -> String {
        let mut cases = Vec::new();

        // Convert mapping to CASE WHEN clauses
        for (transition, direction) in mapping {
            // Parse "off_to_on" format
            if let Some((from, to)) = transition.split_once("_to_") {
                cases.push(format!(
                    "WHEN LAG({}) OVER w = '{}' AND {} = '{}' THEN '{}'",
                    state_field, from, state_field, to, direction
                ));
            }
        }

        // Add initial state and unknown cases
        cases.push(format!(
            "WHEN LAG({}) OVER w IS NULL THEN 'initial'",
            state_field
        ));
        cases.push("ELSE 'unknown'".to_string());

        format!(
            "CASE\n        {}\n    END AS transition_direction",
            cases.join("\n        ")
        )
    }

    /// Generate default direction CASE for on/off states
    fn generate_default_direction_case(&self, state_field: &str) -> String {
        format!(
            r#"CASE
        WHEN LAG({state}) OVER w = 'off' AND {state} = 'on' THEN 'opening'
        WHEN LAG({state}) OVER w = 'on' AND {state} = 'off' THEN 'closing'
        WHEN LAG({state}) OVER w IS NULL THEN 'initial'
        ELSE 'unknown'
    END AS transition_direction"#,
            state = state_field
        )
    }

    /// Generate device type derivation CASE
    fn generate_device_type_case(&self, entity_field: &str) -> String {
        format!(
            r#"CASE
        WHEN {entity} LIKE 'door_%' THEN 'door'
        WHEN {entity} LIKE 'window_%' THEN 'window'
        WHEN {entity} LIKE 'motion_%' THEN 'motion'
        WHEN {entity} LIKE 'light_%' THEN 'light'
        ELSE 'other'
    END AS device_type"#,
            entity = entity_field
        )
    }

    /// Wrap CREATE statement with sync mode (IF NOT EXISTS check)
    fn wrap_sync_mode(&self, view_name: &str, create_sql: &str) -> Result<String> {
        let parts: Vec<&str> = view_name.split('.').collect();
        let (schema, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("gold", view_name)
        };

        Ok(format!(
            r#"-- State transitions for stream: {stream_id}
-- Generated by ndp-gold-ddl
-- Mode: SYNC (create if not exists)

CREATE SCHEMA IF NOT EXISTS gold;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_matviews
        WHERE schemaname = '{schema}'
          AND matviewname = '{name}'
    ) THEN
        -- Create the materialized view
{create_sql_indented}

        RAISE NOTICE 'Created transition view: {view_name}';
    ELSE
        RAISE NOTICE '{view_name} already exists, skipping';
    END IF;
END $$;"#,
            stream_id = self.stream_id,
            schema = schema,
            name = name,
            view_name = view_name,
            create_sql_indented = create_sql
                .lines()
                .map(|l| format!("        {}", l))
                .collect::<Vec<_>>()
                .join("\n"),
        ))
    }

    /// Wrap CREATE statement with recreate mode (DROP CASCADE first)
    fn wrap_recreate_mode(&self, view_name: &str, create_sql: &str) -> Result<String> {
        // Get the _actual view name for dropping
        let actual_view_name = format!("{}_actual", view_name);

        Ok(format!(
            r#"-- State transitions for stream: {stream_id}
-- Generated by ndp-gold-ddl
-- Mode: RECREATE (drop and create)

CREATE SCHEMA IF NOT EXISTS gold;

-- Drop existing views
DROP VIEW IF EXISTS {actual_view_name} CASCADE;
DROP MATERIALIZED VIEW IF EXISTS {view_name} CASCADE;

-- Create the transition view
{create_sql}"#,
            stream_id = self.stream_id,
            view_name = view_name,
            actual_view_name = actual_view_name,
            create_sql = create_sql,
        ))
    }
}

/// Trait for generating state transition DDL
pub trait ITransitionGenerator {
    /// Generate transition view DDL for a state_event stream
    fn generate_transitions_ddl(&self, stream_config: &StreamConfig) -> Result<String>;

    /// Generate filtered view for actual transitions only
    fn generate_actual_transitions_view(&self, base_view: &str) -> Result<String>;
}

impl ITransitionGenerator for StateTransitionGenerator {
    fn generate_transitions_ddl(&self, stream_config: &StreamConfig) -> Result<String> {
        let transition_config =
            TransitionConfig::from_stream_config(stream_config).ok_or_else(|| {
                GoldDdlError::MissingRequiredField {
                    field: "gold_etl.features.transitions".to_string(),
                    context: format!("stream '{}'", stream_config.stream_id),
                }
            })?;

        self.generate(&transition_config, Action::Sync)
    }

    fn generate_actual_transitions_view(&self, base_view: &str) -> Result<String> {
        Ok(format!(
            r#"CREATE VIEW {base_view}_actual AS
SELECT * FROM {base_view}
WHERE is_actual_transition = TRUE;"#,
            base_view = base_view
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        FeaturesConfig, GoldEtlConfig, SilverEtlConfig, TimestampConfig,
        TransitionsConfig as TypesTransitionsConfig,
    };

    fn create_test_transition_config() -> TransitionConfig {
        TransitionConfig {
            enabled: true,
            state_field: "state".to_string(),
            entity_field: "ndp_id".to_string(),
            track_duration: true,
            include_in_alignment: true,
            direction_mapping: None,
        }
    }

    fn create_test_stream_config() -> StreamConfig {
        StreamConfig {
            stream_id: "home-assistant-state".to_string(),
            fields: vec![],
            silver_etl: Some(SilverEtlConfig {
                target_table: "silver.state_events".to_string(),
                timestamp: Some(TimestampConfig {
                    target_field: "event_time".to_string(),
                }),
            }),
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                aggregates: None,
                features: Some(FeaturesConfig {
                    lag: None,
                    rolling: None,
                    trend: None,
                    transitions: Some(TypesTransitionsConfig {
                        enabled: true,
                        field: "state".to_string(),
                        states: vec!["on".to_string(), "off".to_string()],
                    }),
                }),
                refresh_policy: None,
            }),
        }
    }

    // =========================================================================
    // TDD Cycle 1: Basic Transition Detection
    // =========================================================================

    #[test]
    fn test_generates_transition_view() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("CREATE") && sql.contains("MATERIALIZED VIEW"));
        assert!(sql.contains("LAG(state)"));
        assert!(sql.contains("from_state"));
        assert!(sql.contains("to_state"));
    }

    #[test]
    fn test_view_name_follows_pattern() {
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let view_name = generator.get_view_name();

        assert_eq!(view_name, "gold.home_assistant_state_transitions");
    }

    #[test]
    fn test_generator_creates_from_stream_config() {
        let config = create_test_stream_config();
        let generator = StateTransitionGenerator::from_stream_config(&config);

        assert!(generator.is_ok());
        let gen = generator.unwrap();
        assert_eq!(gen.stream_id, "home-assistant-state");
        assert_eq!(gen.source_table, "silver.state_events");
    }

    // =========================================================================
    // TDD Cycle 2: is_actual_transition Column
    // =========================================================================

    #[test]
    fn test_is_actual_transition_filters_noise() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        // Should have boolean column for actual transitions
        assert!(sql.contains("is_actual_transition"));

        // Logic: LAG(state) IS DISTINCT FROM state (or IS NULL for first)
        assert!(sql.contains("IS DISTINCT FROM"));
    }

    #[test]
    fn test_first_event_is_transition() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        // First event (LAG is NULL) should be marked as transition
        assert!(sql.contains("LAG(state) OVER w IS NULL THEN TRUE"));
    }

    // =========================================================================
    // TDD Cycle 3: Duration in Previous State
    // =========================================================================

    #[test]
    fn test_duration_calculated() {
        let config = TransitionConfig {
            track_duration: true,
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("duration_in_previous_state"));
        assert!(sql.contains("duration_ms"));
        assert!(sql.contains("EXTRACT(EPOCH FROM"));
        assert!(sql.contains("LAG(event_time)"));
    }

    #[test]
    fn test_duration_skipped_when_disabled() {
        let config = TransitionConfig {
            track_duration: false,
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(!sql.contains("duration_in_previous_state"));
        assert!(!sql.contains("duration_ms"));
    }

    // =========================================================================
    // TDD Cycle 4: Entity Partitioning
    // =========================================================================

    #[test]
    fn test_entity_partitioning() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("PARTITION BY ndp_id"));
        assert!(sql.contains("ORDER BY event_time"));
    }

    #[test]
    fn test_entity_field_is_configurable() {
        let config = TransitionConfig {
            entity_field: "device_id".to_string(),
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("PARTITION BY device_id"));
        assert!(sql.contains("device_id AS entity_id"));
    }

    // =========================================================================
    // TDD Cycle 5: Transition Direction
    // =========================================================================

    #[test]
    fn test_default_direction_logic() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("transition_direction"));
        assert!(sql.contains("'opening'"));
        assert!(sql.contains("'closing'"));
        assert!(sql.contains("'initial'"));
        assert!(sql.contains("'unknown'"));
    }

    #[test]
    fn test_custom_direction_mapping() {
        let mut mapping = HashMap::new();
        mapping.insert("closed_to_open".to_string(), "opening".to_string());
        mapping.insert("open_to_closed".to_string(), "closing".to_string());

        let config = TransitionConfig {
            direction_mapping: Some(mapping),
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("'closed'") && sql.contains("'open'"));
        assert!(sql.contains("'opening'"));
        assert!(sql.contains("'closing'"));
    }

    // =========================================================================
    // TDD Cycle 6: Device Type Derivation
    // =========================================================================

    #[test]
    fn test_device_type_derived_from_entity() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("device_type"));
        assert!(sql.contains("LIKE 'door_%'"));
        assert!(sql.contains("LIKE 'window_%'"));
    }

    // =========================================================================
    // Idempotency Tests
    // =========================================================================

    #[test]
    fn test_sync_mode_checks_existence() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Sync).unwrap();

        assert!(sql.contains("IF NOT EXISTS"));
        assert!(sql.contains("pg_matviews"));
        assert!(sql.contains("schemaname = 'gold'"));
    }

    #[test]
    fn test_recreate_mode_drops_first() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("DROP MATERIALIZED VIEW IF EXISTS"));
        assert!(sql.contains("CASCADE"));
    }

    // =========================================================================
    // Index Generation Tests
    // =========================================================================

    #[test]
    fn test_generates_indexes() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("CREATE INDEX IF NOT EXISTS"));
        assert!(sql.contains("transition_time DESC"));
        assert!(sql.contains("entity_id, transition_time"));
    }

    // =========================================================================
    // Filtered View Tests
    // =========================================================================

    #[test]
    fn test_generates_actual_transitions_view() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("CREATE VIEW"));
        assert!(sql.contains("_actual AS"));
        assert!(sql.contains("is_actual_transition = TRUE"));
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_disabled_transitions_returns_error() {
        let config = TransitionConfig {
            enabled: false,
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let result = generator.generate(&config, Action::Recreate);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not enabled"));
    }

    #[test]
    fn test_missing_state_field_returns_error() {
        let config = TransitionConfig {
            state_field: String::new(),
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let result = generator.generate(&config, Action::Recreate);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("state_field"));
    }

    #[test]
    fn test_missing_entity_field_returns_error() {
        let config = TransitionConfig {
            entity_field: String::new(),
            ..create_test_transition_config()
        };
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let result = generator.generate(&config, Action::Recreate);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("entity_field"));
    }

    #[test]
    fn test_missing_silver_etl_returns_error() {
        let mut config = create_test_stream_config();
        config.silver_etl = None;

        let result = StateTransitionGenerator::from_stream_config(&config);

        assert!(result.is_err());
    }

    // =========================================================================
    // Stream ID Normalization Tests
    // =========================================================================

    #[test]
    fn test_stream_id_normalized_in_view_name() {
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let view_name = generator.get_view_name();

        // Hyphens should be replaced with underscores
        assert!(!view_name.contains('-'));
        assert!(view_name.contains("home_assistant_state"));
    }

    // =========================================================================
    // Comment Generation Tests
    // =========================================================================

    #[test]
    fn test_generates_view_comment() {
        let config = create_test_transition_config();
        let generator = StateTransitionGenerator::new(
            "home-assistant-state",
            "silver.state_events",
            "event_time",
        );

        let sql = generator.generate(&config, Action::Recreate).unwrap();

        assert!(sql.contains("COMMENT ON MATERIALIZED VIEW"));
        assert!(sql.contains("home-assistant-state"));
    }
}
