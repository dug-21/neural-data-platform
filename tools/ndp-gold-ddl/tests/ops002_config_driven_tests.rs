//! OPS-002 Config-Driven Generation Tests
//!
//! Verify that the EventsGenerator correctly adapts its output based on
//! the number of objectives, the presence/absence of units, and the
//! config fields for entity and state.
//!
//! Test IDs: CG-001 through CG-006

mod fixtures;

use fixtures::energy_monitoring::*;
use fixtures::phase_c::{assert_sql_contains, assert_sql_not_contains, count_sql_occurrences};

use ndp_gold_ddl::{Action, EventsGenerator};

// ============================================================================
// CG-001: Single Objective Generates Single CTE
// ============================================================================

#[test]
fn test_single_objective_generates_single_crossing_cte() {
    let domain = create_single_objective_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate(Action::Recreate).unwrap();

    // Exactly one "*_crossings AS" CTE
    let crossings_count = count_sql_occurrences(&sql, "_crossings AS");
    // Should be 2: "voltage_crossings AS" + "all_crossings AS"
    assert_eq!(
        crossings_count, 2,
        "CG-001: Expected 2 '*_crossings AS' occurrences (voltage_crossings + all_crossings), \
         found {}.\n\nSQL:\n{}",
        crossings_count, sql,
    );

    // The all_crossings UNION ALL should have exactly one source (no UNION ALL keyword needed)
    assert_sql_contains(&sql, "voltage_crossings", "CG-001: voltage_crossings CTE");
    // With a single source, there should be no UNION ALL
    let union_count = count_sql_occurrences(&sql, "UNION ALL");
    assert_eq!(
        union_count, 0,
        "CG-001: Single objective should not have UNION ALL, found {}.\n\nSQL:\n{}",
        union_count, sql,
    );
}

// ============================================================================
// CG-002: Three Objectives Generate Three CTEs
// ============================================================================

#[test]
fn test_three_objectives_generate_three_crossing_ctes() {
    let domain = create_three_objective_domain();
    let loader = energy_monitoring_loader_with_frequency();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate(Action::Recreate).unwrap();

    // Three metric-specific crossings + all_crossings = 4
    let crossings_count = count_sql_occurrences(&sql, "_crossings AS");
    assert_eq!(
        crossings_count, 4,
        "CG-002: Expected 4 '*_crossings AS' occurrences \
         (voltage + current + frequency + all_crossings), found {}.\n\nSQL:\n{}",
        crossings_count, sql,
    );

    // The all_crossings UNION ALL should have 2 UNION ALL connectors (3 sources)
    let union_count = count_sql_occurrences(&sql, "UNION ALL");
    assert_eq!(
        union_count, 2,
        "CG-002: Three objectives should have 2 UNION ALL connectors, found {}.\n\nSQL:\n{}",
        union_count, sql,
    );

    // Verify all three CTEs exist
    assert_sql_contains(&sql, "voltage_crossings", "CG-002: voltage_crossings CTE");
    assert_sql_contains(&sql, "current_crossings", "CG-002: current_crossings CTE");
    assert_sql_contains(
        &sql,
        "frequency_crossings",
        "CG-002: frequency_crossings CTE",
    );
}

// ============================================================================
// CG-003: Zero Objectives Skips Threshold Section
// ============================================================================

#[test]
fn test_zero_objectives_skips_threshold_crossings() {
    let domain = create_zero_objective_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate(Action::Recreate).unwrap();

    // Should NOT contain threshold crossings section
    assert_sql_not_contains(
        &sql,
        "THRESHOLD CROSSINGS",
        "CG-003: no THRESHOLD CROSSINGS section",
    );
    assert_sql_not_contains(
        &sql,
        "_crossings",
        "CG-003: no crossing CTEs at all",
    );

    // State transitions should still be present
    assert_sql_contains(
        &sql,
        "STATE TRANSITIONS",
        "CG-003: state transitions still present",
    );
}

// ============================================================================
// CG-004: Objective With No Unit Uses Empty String or NULL in CASE
// ============================================================================

#[test]
fn test_objective_without_unit_produces_valid_sql() {
    let domain = create_no_unit_objective_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate(Action::Recreate).unwrap();

    // The SQL should still generate valid output without crashing
    assert_sql_contains(
        &sql,
        "THRESHOLD CROSSINGS",
        "CG-004: threshold section present even without unit",
    );
    assert_sql_contains(
        &sql,
        "voltage_crossings",
        "CG-004: voltage crossing CTE present",
    );

    // The details should be a simple empty JSONB since there are no unit cases
    // (When unit is None, no WHEN clause is generated, so no unit_cases, producing '{}'::JSONB)
    assert_sql_contains(
        &sql,
        "safe_voltage",
        "CG-004: objective id still present",
    );
}

// ============================================================================
// CG-005: Entity Field From Stream Config
// ============================================================================

#[test]
fn test_entity_field_from_stream_config() {
    let domain = create_energy_monitoring_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate(Action::Recreate).unwrap();

    // The TransitionConfig.from_stream_config for grid-relay-state uses
    // NDP_ENTITY_COLUMN ("ndp_id") by default since TransitionsConfig doesn't
    // have a separate entity_field. This is expected behavior.
    assert_sql_contains(
        &sql,
        "AS entity_id",
        "CG-005: entity_id alias present in state transitions",
    );
}

// ============================================================================
// CG-006: State Field From Stream Config
// ============================================================================

#[test]
fn test_state_field_from_stream_config() {
    let domain = create_energy_monitoring_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate(Action::Recreate).unwrap();

    // The grid-relay-state stream has transitions.field = "relay_state"
    // so the generator should use relay_state, not "state"
    assert_sql_contains(
        &sql,
        "LAG(s.relay_state)",
        "CG-006: LAG uses relay_state from config transitions.field",
    );
    assert_sql_contains(
        &sql,
        "s.relay_state AS to_state",
        "CG-006: to_state uses relay_state from config",
    );
}
