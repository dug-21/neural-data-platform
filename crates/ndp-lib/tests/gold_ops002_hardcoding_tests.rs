//! OPS-002 Hardcoding Detection Tests
//!
//! London TDD tests using a fictional "energy-monitoring" domain to prove
//! the EventsGenerator is truly config-driven. If ANY air-quality-specific
//! string leaks into SQL generated for this domain, the test fails.
//!
//! Test IDs: HD-001 through HD-008

mod gold_fixtures;

use gold_fixtures::energy_monitoring::*;
use gold_fixtures::phase_c::{assert_sql_contains, assert_sql_not_contains};

use ndp_lib::gold::{Action, EventsGenerator};

// ============================================================================
// Helper: generate detection procedure SQL for the energy-monitoring domain
// ============================================================================

fn generate_energy_monitoring_sql() -> String {
    let domain = create_energy_monitoring_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    generator.generate(Action::Recreate).unwrap()
}

// ============================================================================
// HD-001: Detection Procedure Contains No Air-Quality Literals
// ============================================================================

#[test]
fn test_detection_procedure_contains_no_air_quality_literals() {
    let sql = generate_energy_monitoring_sql();

    for forbidden in FORBIDDEN_AIR_QUALITY_LITERALS {
        assert!(
            !sql.contains(forbidden),
            "HD-001 FAILED: Found forbidden air-quality literal '{}' in generated SQL for \
             energy-monitoring domain.\n\nGenerated SQL:\n{}",
            forbidden,
            sql,
        );
    }
}

#[test]
fn test_detection_procedure_contains_energy_monitoring_values() {
    let sql = generate_energy_monitoring_sql();

    // Domain-specific values that MUST appear
    assert_sql_contains(&sql, "energy-monitoring", "domain id in header");
    assert_sql_contains(&sql, "smart-meter", "primary stream id");
    assert_sql_contains(&sql, "voltage", "voltage metric");
    assert_sql_contains(&sql, "240.0", "voltage threshold");
    assert_sql_contains(&sql, "safe_voltage", "voltage objective id");
    assert_sql_contains(&sql, "current", "current metric");
    assert_sql_contains(&sql, "30.0", "current threshold");
    assert_sql_contains(&sql, "efficient_current", "current objective id");
}

// ============================================================================
// HD-002: State Transitions Section Uses Config Stream IDs
// ============================================================================

#[test]
fn test_state_transitions_use_config_stream_id() {
    let sql = generate_energy_monitoring_sql();

    // Should reference grid-relay-state from config
    assert_sql_contains(
        &sql,
        "grid-relay-state",
        "HD-002: actuator stream_id from config",
    );

    // Should reference silver.grid_relay_state from stream config
    assert_sql_contains(
        &sql,
        "silver.grid_relay_state",
        "HD-002: silver table from stream config",
    );

    // Must NOT contain air-quality actuator references
    assert_sql_not_contains(
        &sql,
        "home_assistant_state",
        "HD-002: no hardcoded home_assistant_state",
    );
    assert_sql_not_contains(
        &sql,
        "home-assistant-state",
        "HD-002: no hardcoded home-assistant-state",
    );
    assert_sql_not_contains(
        &sql,
        "silver.state_events",
        "HD-002: no hardcoded silver.state_events",
    );
}

// ============================================================================
// HD-003: Threshold Crossings Section Uses Objectives Config
// ============================================================================

#[test]
fn test_threshold_crossings_use_objectives_config() {
    let sql = generate_energy_monitoring_sql();

    // Should contain energy-monitoring crossing CTEs
    assert_sql_contains(
        &sql,
        "voltage_crossings",
        "HD-003: voltage crossing CTE from config",
    );
    assert_sql_contains(
        &sql,
        "current_crossings",
        "HD-003: current crossing CTE from config",
    );

    // Should contain config thresholds
    assert_sql_contains(&sql, "240.0", "HD-003: voltage threshold from config");
    assert_sql_contains(&sql, "30.0", "HD-003: current threshold from config");

    // Should contain objective IDs
    assert_sql_contains(&sql, "'safe_voltage'", "HD-003: voltage objective_id");
    assert_sql_contains(&sql, "'efficient_current'", "HD-003: current objective_id");

    // Must NOT contain air-quality thresholds or objectives
    assert_sql_not_contains(&sql, "800", "HD-003: no hardcoded 800 threshold");
    assert_sql_not_contains(&sql, "12.0", "HD-003: no hardcoded 12.0 threshold");
    assert_sql_not_contains(&sql, "co2", "HD-003: no hardcoded co2");
    assert_sql_not_contains(&sql, "pm25", "HD-003: no hardcoded pm25");
}

// ============================================================================
// HD-004: Context Enrichment Uses Config-Derived Fields
// ============================================================================

#[test]
fn test_context_enrichment_uses_config_fields() {
    let sql = generate_energy_monitoring_sql();

    // Context JSONB should include fields from smart-meter (aliased as "meter")
    // The build_context_columns method produces {alias}_{field} labels
    assert_sql_contains(
        &sql,
        "meter_voltage",
        "HD-004: meter voltage in context from config",
    );
    assert_sql_contains(
        &sql,
        "meter_current",
        "HD-004: meter current in context from config",
    );
    assert_sql_contains(
        &sql,
        "meter_power_w",
        "HD-004: meter power_w in context from config",
    );

    // Must NOT contain air-quality context fields
    assert_sql_not_contains(
        &sql,
        "indoor_co2",
        "HD-004: no hardcoded indoor_co2 context",
    );
    assert_sql_not_contains(
        &sql,
        "indoor_pm25",
        "HD-004: no hardcoded indoor_pm25 context",
    );
    assert_sql_not_contains(
        &sql,
        "outdoor_temperature",
        "HD-004: no hardcoded outdoor_temperature context",
    );
}

// ============================================================================
// HD-005: Unit Mapping Uses Objective Config
// ============================================================================

#[test]
fn test_unit_mapping_uses_objective_config() {
    let sql = generate_energy_monitoring_sql();

    // Should contain energy-monitoring units
    assert_sql_contains(&sql, "'volts'", "HD-005: volts unit from objective config");
    assert_sql_contains(&sql, "'amps'", "HD-005: amps unit from objective config");

    // Must NOT contain air-quality units
    assert_sql_not_contains(&sql, "'ppm'", "HD-005: no hardcoded ppm unit");
    assert_sql_not_contains(&sql, "'ug/m3'", "HD-005: no hardcoded ug/m3 unit");
}

// ============================================================================
// HD-006: Gold Table Name Derived From Config
// ============================================================================

#[test]
fn test_gold_table_name_from_config() {
    let sql = generate_energy_monitoring_sql();

    // Gold CA table should be derived from stream_id "smart-meter"
    // -> gold.smart_meter_hourly (matching continuous_aggregate.rs naming)
    assert_sql_contains(
        &sql,
        "gold.smart_meter_hourly",
        "HD-006: gold table derived from smart-meter stream_id",
    );

    // Must NOT contain air-quality gold table
    assert_sql_not_contains(
        &sql,
        "gold.air_quality_hourly",
        "HD-006: no hardcoded gold.air_quality_hourly",
    );
}

// ============================================================================
// HD-007: Silver Table Name From Stream Config
// ============================================================================

#[test]
fn test_silver_table_from_stream_config() {
    let sql = generate_energy_monitoring_sql();

    // Silver table for the actuator (grid-relay-state) should come from config
    assert_sql_contains(
        &sql,
        "silver.grid_relay_state",
        "HD-007: silver table from stream config",
    );

    // Must NOT contain air-quality silver tables
    assert_sql_not_contains(
        &sql,
        "silver.state_events",
        "HD-007: no hardcoded silver.state_events",
    );
    assert_sql_not_contains(
        &sql,
        "silver.home_assistant_state",
        "HD-007: no hardcoded silver.home_assistant_state",
    );
}

// ============================================================================
// HD-008: Aligned View Name From Domain Config
// ============================================================================

#[test]
fn test_aligned_view_name_from_domain_config() {
    let sql = generate_energy_monitoring_sql();

    // Should use the alignment.view_name from config
    assert_sql_contains(
        &sql,
        "gold.energy_monitoring_aligned",
        "HD-008: aligned view name from domain config",
    );

    // Must NOT contain air-quality aligned view
    assert_sql_not_contains(
        &sql,
        "indoor_air_quality_aligned",
        "HD-008: no hardcoded indoor_air_quality_aligned",
    );
}

// ============================================================================
// Comprehensive Forbidden Scan (belt-and-suspenders with HD-001)
// ============================================================================

#[test]
fn test_forbidden_scan_on_detection_procedure_only() {
    let domain = create_energy_monitoring_domain();
    let loader = energy_monitoring_loader();
    let generator = EventsGenerator::from_domain_config(&domain, Box::new(loader));
    let sql = generator.generate_detection_procedure().unwrap();

    for forbidden in FORBIDDEN_AIR_QUALITY_LITERALS {
        assert!(
            !sql.contains(forbidden),
            "Forbidden literal '{}' found in detection procedure SQL.\n\nSQL:\n{}",
            forbidden,
            sql,
        );
    }
}
