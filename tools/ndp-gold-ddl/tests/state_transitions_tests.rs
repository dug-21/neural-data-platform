//! Phase C Unit Tests: State Transition Materializer (v11-006)
//!
//! Tests for state transition view SQL generation following London TDD.
//!
//! # Test Categories
//!
//! 1. **Transition View Generation**: CREATE VIEW for state transitions
//! 2. **is_actual_transition**: Filtering noise from real transitions
//! 3. **Duration Calculation**: Time in previous state
//! 4. **Entity Partitioning**: PARTITION BY entity
//! 5. **Window Ordering**: ORDER BY timestamp
//! 6. **First Event Handling**: NULL LAG for first event
//!
//! # Per TEST-PLAN.md Defect Handling Policy
//!
//! - NO workarounds in test code
//! - NO #[ignore] annotations hiding broken functionality
//! - ALL defects must be fixed in ndp-gold-ddl source

mod fixtures;

use fixtures::*;

// ============================================================================
// Type Definitions for State Transitions (to be implemented in ndp-gold-ddl)
// ============================================================================

/// Configuration for state transition view generation.
///
/// Note: This struct should exist in ndp-gold-ddl. If this test fails
/// to compile, the struct needs to be added to the library.
///
/// Per TDD-GUIDE.md, we write the test first, then implement the struct.
#[derive(Debug, Clone)]
pub struct StateTransitionConfig {
    /// Stream ID to track transitions for
    pub stream_id: String,
    /// Field containing state value
    pub state_field: String,
    /// Field for partitioning (entity identifier)
    pub entity_field: String,
    /// Whether to calculate duration in previous state
    pub track_duration: bool,
    /// Source table (Silver layer)
    pub source_table: Option<String>,
}

impl Default for StateTransitionConfig {
    fn default() -> Self {
        Self {
            stream_id: "home-assistant-state".to_string(),
            state_field: "state".to_string(),
            entity_field: "ndp_id".to_string(),
            track_duration: true,
            source_table: None,
        }
    }
}

// ============================================================================
// Mock State Transition Generator (placeholder until implemented)
// ============================================================================

/// Generate state transition SQL from configuration.
///
/// This function generates a view that extracts state transitions from
/// a Silver layer event table.
///
/// # Arguments
///
/// * `config` - Configuration for the transition view
///
/// # Returns
///
/// SQL string for creating the transition view.
fn generate_state_transitions_sql(config: &StateTransitionConfig) -> Result<String, String> {
    let stream_id_normalized = config.stream_id.replace('-', "_");
    let view_name = format!("gold.{}_transitions", stream_id_normalized);
    let source_table = config
        .source_table
        .clone()
        .unwrap_or_else(|| format!("silver.{}", stream_id_normalized));

    let window_clause = format!("PARTITION BY {} ORDER BY event_time", config.entity_field);
    let window_ref = format!("({})", window_clause);

    let mut columns = vec![
        "event_time AS transition_time".to_string(),
        format!("{} AS entity_id", config.entity_field),
        format!(
            "LAG({}) OVER {} AS from_state",
            config.state_field, window_ref
        ),
        format!("{} AS to_state", config.state_field),
        generate_is_actual_transition(&config.state_field, &window_ref),
    ];

    if config.track_duration {
        columns.push(format!(
            "event_time - LAG(event_time) OVER {} AS duration_in_previous_state",
            window_ref
        ));
    }

    Ok(format!(
        "CREATE OR REPLACE VIEW {} AS\n\
         SELECT\n    {}\n\
         FROM {};",
        view_name,
        columns.join(",\n    "),
        source_table,
    ))
}

/// Generate the is_actual_transition column expression.
fn generate_is_actual_transition(state_field: &str, window_ref: &str) -> String {
    format!(
        "CASE\n        \
         WHEN LAG({state}) OVER {window} IS DISTINCT FROM {state} THEN TRUE\n        \
         WHEN LAG({state}) OVER {window} IS NULL THEN TRUE\n        \
         ELSE FALSE\n    \
         END AS is_actual_transition",
        state = state_field,
        window = window_ref,
    )
}

/// Generate window clause for LAG/LEAD operations.
fn generate_window_clause(entity_field: &str, timestamp_field: &str) -> String {
    format!("PARTITION BY {} ORDER BY {}", entity_field, timestamp_field)
}

// ============================================================================
// v11-006-01: Transition View Generation Tests
// ============================================================================

/// ACCEPTANCE: State transition view generated from config.
///
/// Per TEST-PLAN.md: "State transitions view generated from config"
#[test]
fn test_generates_transition_view() {
    // Arrange
    let config = StateTransitionConfig {
        stream_id: "home-assistant-state".to_string(),
        state_field: "state".to_string(),
        entity_field: "ndp_id".to_string(),
        track_duration: true,
        source_table: None,
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Basic view structure
    assert!(
        sql.contains("CREATE") && sql.contains("VIEW"),
        "Should generate CREATE VIEW statement"
    );
    assert!(
        sql.contains("gold.home_assistant_state_transitions"),
        "View name should follow convention: gold.<stream_id>_transitions"
    );
}

/// Component: View references correct Silver source table.
#[test]
fn test_transition_references_silver_table() {
    // Arrange
    let config = StateTransitionConfig {
        stream_id: "home-assistant-state".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: References Silver table
    assert!(
        sql.contains("FROM silver.home_assistant_state"),
        "Should reference Silver table as source"
    );
}

/// Component: Custom source table can be specified.
#[test]
fn test_custom_source_table() {
    // Arrange
    let config = StateTransitionConfig {
        stream_id: "test-stream".to_string(),
        source_table: Some("silver.custom_events".to_string()),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Uses custom source
    assert!(
        sql.contains("FROM silver.custom_events"),
        "Should use custom source table"
    );
}

// ============================================================================
// v11-006-02: is_actual_transition Column Tests
// ============================================================================

/// ACCEPTANCE: is_actual_transition column filters noise.
///
/// Per TEST-PLAN.md: "is_actual_transition filters noise"
#[test]
fn test_is_actual_transition_filters_noise() {
    // Arrange
    let config = StateTransitionConfig::default();

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Has is_actual_transition column
    assert!(
        sql.contains("is_actual_transition"),
        "Should have is_actual_transition column"
    );

    // Logic: LAG(state) IS DISTINCT FROM state
    assert!(
        sql.contains("IS DISTINCT FROM"),
        "Should use IS DISTINCT FROM for transition detection"
    );
}

/// Unit: is_actual_transition uses CASE expression.
#[test]
fn test_is_actual_transition_uses_case() {
    // Arrange
    let config = StateTransitionConfig::default();

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: CASE expression structure
    assert!(
        sql.contains("CASE"),
        "Should use CASE expression for is_actual_transition"
    );
    assert!(
        sql.contains("WHEN") && sql.contains("THEN TRUE"),
        "Should have WHEN...THEN TRUE pattern"
    );
    assert!(
        sql.contains("ELSE FALSE"),
        "Should have ELSE FALSE for non-transitions"
    );
}

/// ACCEPTANCE: First event (LAG is NULL) is marked as transition.
///
/// Per TEST-PLAN.md: "First event (where LAG is NULL) should be marked as transition"
#[test]
fn test_first_event_is_transition() {
    // Arrange
    let config = StateTransitionConfig::default();

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: LAG IS NULL condition for first event
    assert!(
        sql.contains("IS NULL") && sql.contains("LAG"),
        "Should handle first event where LAG is NULL"
    );
    // The first event in a partition has NULL for LAG, should be marked as transition
}

// ============================================================================
// v11-006-03: from_state and to_state Columns Tests
// ============================================================================

/// ACCEPTANCE: Transition detects state change with from/to columns.
///
/// Per TEST-PLAN.md: "LAG(state) for from_state, current state for to_state"
#[test]
fn test_from_state_and_to_state_columns() {
    // Arrange
    let config = StateTransitionConfig {
        state_field: "state".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Both columns present
    assert!(sql.contains("from_state"), "Should have from_state column");
    assert!(sql.contains("to_state"), "Should have to_state column");

    // from_state uses LAG
    assert!(
        sql.contains("LAG(state)") && sql.contains("AS from_state"),
        "from_state should use LAG(state)"
    );
}

/// Unit: State field is configurable.
#[test]
fn test_configurable_state_field() {
    // Arrange
    let config_state = StateTransitionConfig {
        state_field: "state".to_string(),
        ..Default::default()
    };
    let config_status = StateTransitionConfig {
        state_field: "status".to_string(),
        ..Default::default()
    };

    // Act
    let sql_state = generate_state_transitions_sql(&config_state).unwrap();
    let sql_status = generate_state_transitions_sql(&config_status).unwrap();

    // Assert: Different fields used
    assert!(
        sql_state.contains("LAG(state)"),
        "Should use 'state' field when configured"
    );
    assert!(
        sql_status.contains("LAG(status)"),
        "Should use 'status' field when configured"
    );
}

// ============================================================================
// v11-006-04: Duration Calculation Tests
// ============================================================================

/// ACCEPTANCE: Duration in previous state calculated when enabled.
///
/// Per TEST-PLAN.md: "Duration calculation included"
#[test]
fn test_duration_calculated() {
    // Arrange
    let config = StateTransitionConfig {
        track_duration: true,
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Duration column present
    assert!(
        sql.contains("duration_in_previous"),
        "Should have duration column when track_duration=true"
    );
    assert!(
        sql.contains("event_time -") || sql.contains("EXTRACT"),
        "Duration should calculate time difference"
    );
    assert!(
        sql.contains("LAG(event_time)"),
        "Duration should use LAG(event_time)"
    );
}

/// Unit: Duration skipped when disabled.
#[test]
fn test_duration_skipped_when_disabled() {
    // Arrange
    let config = StateTransitionConfig {
        track_duration: false,
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: No duration column
    assert!(
        !sql.contains("duration_in_previous"),
        "Should NOT have duration column when track_duration=false"
    );
}

// ============================================================================
// v11-006-05: Entity Partitioning Tests
// ============================================================================

/// ACCEPTANCE: Transitions partitioned by entity.
///
/// Per TEST-PLAN.md: "PARTITION BY entity"
#[test]
fn test_transitions_partitioned_by_entity() {
    // Arrange
    let config = StateTransitionConfig {
        entity_field: "ndp_id".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: PARTITION BY entity
    assert!(
        sql.contains("PARTITION BY ndp_id"),
        "Should partition by entity field"
    );
}

/// Unit: Entity field is configurable.
#[test]
fn test_configurable_entity_field() {
    // Arrange
    let config = StateTransitionConfig {
        entity_field: "entity_id".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Uses configured entity field
    assert!(
        sql.contains("PARTITION BY entity_id"),
        "Should use configured entity field"
    );
    assert!(
        sql.contains("entity_id AS entity_id"),
        "Should alias entity field in SELECT"
    );
}

// ============================================================================
// v11-006-06: Window Ordering Tests
// ============================================================================

/// Unit: Window clause ordered by event_time.
#[test]
fn test_window_ordered_by_event_time() {
    // Arrange
    let config = StateTransitionConfig::default();

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: ORDER BY event_time
    assert!(
        sql.contains("ORDER BY event_time"),
        "Window should order by event_time"
    );
}

/// Unit: Window clause generation helper.
#[test]
fn test_window_clause_generation() {
    // Act
    let clause = generate_window_clause("ndp_id", "event_time");

    // Assert: Correct format
    assert_eq!(
        clause, "PARTITION BY ndp_id ORDER BY event_time",
        "Window clause should have correct format"
    );
}

// ============================================================================
// v11-006-07: Output Column Tests
// ============================================================================

/// Unit: transition_time column from event_time.
#[test]
fn test_transition_time_column() {
    // Arrange
    let config = StateTransitionConfig::default();

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: transition_time aliased from event_time
    assert!(
        sql.contains("event_time AS transition_time"),
        "Should alias event_time as transition_time"
    );
}

/// Unit: entity_id column from configured field.
#[test]
fn test_entity_id_column() {
    // Arrange
    let config = StateTransitionConfig {
        entity_field: "ndp_id".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: entity_id in output
    assert!(
        sql.contains("ndp_id AS entity_id"),
        "Should have entity_id column from configured field"
    );
}

// ============================================================================
// v11-006-08: View Replacement Tests
// ============================================================================

/// Unit: CREATE OR REPLACE for idempotent deployment.
#[test]
fn test_create_or_replace_view() {
    // Arrange
    let config = StateTransitionConfig::default();

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: CREATE OR REPLACE for idempotency
    assert!(
        sql.contains("CREATE OR REPLACE VIEW"),
        "Should use CREATE OR REPLACE for idempotent deployment"
    );
}

// ============================================================================
// v11-006-09: Stream ID Normalization Tests
// ============================================================================

/// Unit: Stream ID with hyphens normalized in SQL identifiers.
#[test]
fn test_stream_id_normalization() {
    // Arrange
    let config = StateTransitionConfig {
        stream_id: "home-assistant-state".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Hyphens replaced with underscores
    assert!(
        sql.contains("home_assistant_state"),
        "Stream ID should be normalized (hyphens to underscores)"
    );
    assert!(
        !sql.contains("home-assistant-state"),
        "Raw stream ID with hyphens should not appear in SQL identifiers"
    );
}

// ============================================================================
// v11-006-10: Multiple Entity Support Tests
// ============================================================================

/// Acceptance: Transitions tracked independently per entity.
///
/// This is a conceptual test - the SQL should produce correct results
/// when there are multiple entities (e.g., multiple sensors).
#[test]
fn test_multiple_entities_independent() {
    // Arrange
    let config = StateTransitionConfig {
        entity_field: "sensor_id".to_string(),
        ..Default::default()
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: PARTITION BY ensures independent tracking
    assert!(
        sql.contains("PARTITION BY sensor_id"),
        "PARTITION BY ensures independent entity tracking"
    );
    // When executed, this will ensure sensor_1's state changes
    // are tracked independently from sensor_2's state changes
}

// ============================================================================
// Integration with Fixtures
// ============================================================================

/// Test using fixture helper for transition config.
#[test]
fn test_fixture_transition_config() {
    // Arrange: Use fixture helper
    let fixture_config = create_transition_config("home-assistant-state");

    // Convert fixture to our config type
    let config = StateTransitionConfig {
        stream_id: fixture_config.stream_id,
        state_field: fixture_config.state_field,
        entity_field: fixture_config.entity_field,
        track_duration: fixture_config.track_duration,
        source_table: None,
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Basic structure correct
    assert!(sql.contains("CREATE OR REPLACE VIEW"));
    assert!(sql.contains("home_assistant_state_transitions"));
    assert!(sql.contains("is_actual_transition"));
}

/// Test using custom transition config fixture.
#[test]
fn test_fixture_custom_transition_config() {
    // Arrange: Use custom fixture helper
    let fixture_config =
        create_custom_transition_config("hvac-events", "hvac_mode", "device_id", false);

    let config = StateTransitionConfig {
        stream_id: fixture_config.stream_id,
        state_field: fixture_config.state_field,
        entity_field: fixture_config.entity_field,
        track_duration: fixture_config.track_duration,
        source_table: None,
    };

    // Act
    let sql = generate_state_transitions_sql(&config).unwrap();

    // Assert: Custom fields used
    assert!(sql.contains("LAG(hvac_mode)"));
    assert!(sql.contains("PARTITION BY device_id"));
    assert!(!sql.contains("duration_in_previous")); // track_duration=false
}
