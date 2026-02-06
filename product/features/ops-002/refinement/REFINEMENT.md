# OPS-002 Refinement: London TDD Test Plan

> **Feature:** ops-002 - Eliminate Hardcoded References from Gold Layer Generators
> **Phase:** Refinement (SPARC R)
> **Approach:** London School TDD - outside-in, mock collaborators, test behavior
> **Created:** 2026-02-06

---

## 1. Test-First Overview

London TDD dictates that we write failing tests FIRST that describe the behavior we want, then refactor the generators to pass those tests. The tests define a contract: "given this fictional config, the generated SQL must contain ONLY values from that config."

### Test Execution Order

| Step | Category | Purpose | Run With |
|------|----------|---------|----------|
| 1 | Hardcoding Detection | Define the "no air-quality literals" contract | `cargo test -p ndp-gold-ddl` |
| 2 | Config-Driven Generation | Define what config-driven output looks like | `cargo test -p ndp-gold-ddl` |
| 3 | Cross-Cutting Constants | Enforce shared constants for `ndp_id`, `gold` schema | `cargo test -p ndp-gold-ddl` |
| 4 | Refactor Generators | Make tests pass by reading from config | (implementation) |
| 5 | Source Code Scan | Prevent re-introduction of hardcoded values | `cargo test -p ndp-gold-ddl` |
| 6 | Golden Master Update | Capture new baselines after refactor | `./scripts/capture-golden-master.sh` |
| 7 | Integration Tests | Verify generated SQL executes against TimescaleDB | `cargo test -- --ignored` |

---

## 2. Hardcoding Detection Test Suite (P0 - MOST CRITICAL)

These tests use a **fictional domain** ("energy-monitoring") to prove the generator reads from config. If ANY air-quality specific string leaks into the generated SQL, the test fails immediately.

### 2.1 Fictional Domain: Energy Monitoring

```rust
// tests/ops002_hardcoding_tests.rs

mod fixtures;

use fixtures::*;
use ndp_gold_ddl::{
    DomainConfig, StreamRef, StreamRole, AlignmentConfig, JoinStrategy,
    NullHandling, ObjectiveConfig, TargetConfig, Priority, EventsConfig,
    Action, StreamConfig, FieldConfig, SilverEtlConfig, GoldEtlConfig,
    AggregatesConfig, FieldMetricsConfig,
};
use ndp_gold_ddl::generators::events::EventsGenerator;
use std::collections::HashMap;

/// The "blessed list" of domain-agnostic strings that are ALLOWED
/// in generated SQL regardless of domain. These are structural SQL
/// keywords and generic column names from the events schema.
const BLESSED_STRUCTURAL: &[&str] = &[
    "gold",               // schema name (P1 - to be constantified separately)
    "silver",             // schema name
    "events",             // generic table name (gold.events)
    "event_time",         // schema column
    "event_type",         // schema column
    "event_id",           // schema column
    "entity_id",          // schema column
    "stream_id",          // schema column
    "from_state",         // schema column
    "to_state",           // schema column
    "state_transition",   // event type constant
    "threshold_crossing", // event type constant
    "metric",             // schema column
    "threshold_value",    // schema column
    "crossing_direction", // schema column
    "metric_value",       // schema column
    "objective_id",       // schema column
    "context",            // schema column
    "details",            // schema column
    "detect_events",      // procedure name
    "events_unified",     // view name
    "events_hourly",      // CA name
    "rising",             // direction constant
    "falling",            // direction constant
    "bucket",             // time_bucket alias
    "ndp_id",             // entity column (P1)
];

/// Air-quality-specific strings that MUST NOT appear in generated SQL
/// when using a non-air-quality domain config.
const FORBIDDEN_AIR_QUALITY_LITERALS: &[&str] = &[
    // Stream identifiers
    "home-assistant-state",
    "home_assistant_state",
    "air-quality",
    "air_quality",
    // Table references
    "silver.state_events",
    "silver.home_assistant_state",
    "gold.air_quality_hourly",
    // Column names from air-quality domain
    "co2_mean",
    "pm25_mean",
    "co2_value",
    "pm25_value",
    "co2_prev",
    "pm25_prev",
    "indoor_co2",
    "indoor_pm25",
    "indoor_temperature_c",
    "outdoor_temperature_c",
    "outdoor_aqi_pm25",
    "state_state_last",
    "indoor_co2_mean",
    "indoor_pm25_mean",
    "indoor_temperature_c_mean",
    "outdoor_temperature_c_mean",
    "outdoor_aqi_pm25_mean",
    // Threshold values
    "800.0",
    "800",
    "12.0",
    // Objective identifiers
    "healthy_co2",
    "healthy_pm25",
    // Metric names
    "'co2'",
    "'pm25'",
    // Unit literals
    "'ppm'",
    "'ug/m3'",
    // CTE names
    "co2_crossings",
    "pm25_crossings",
];

/// Create a fictional "energy-monitoring" domain config.
/// This domain has ZERO overlap with air-quality.
fn create_energy_monitoring_domain() -> DomainConfig {
    DomainConfig {
        id: "energy-monitoring".to_string(),
        description: "Fictional energy monitoring domain for testing".to_string(),
        streams: vec![
            StreamRef {
                stream_id: "smart-meter".to_string(),
                alias: "meter".to_string(),
                role: StreamRole::Primary,
                null_handling: None,
            },
            StreamRef {
                stream_id: "grid-relay-state".to_string(),
                alias: "relay".to_string(),
                role: StreamRole::Actuator,
                null_handling: Some(NullHandling::CarryForward),
            },
        ],
        alignment: AlignmentConfig {
            view_name: "energy_monitoring_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        objectives: vec![
            ObjectiveConfig {
                id: "safe_voltage".to_string(),
                description: "Keep voltage below safe threshold".to_string(),
                target: TargetConfig {
                    stream: "smart-meter".to_string(),
                    metric: "voltage".to_string(),
                    condition: "<".to_string(),
                    threshold: 240.0,
                    unit: Some("volts".to_string()),
                },
                priority: Priority::High,
            },
            ObjectiveConfig {
                id: "efficient_current".to_string(),
                description: "Keep current draw below efficiency threshold".to_string(),
                target: TargetConfig {
                    stream: "smart-meter".to_string(),
                    metric: "current".to_string(),
                    condition: "<".to_string(),
                    threshold: 30.0,
                    unit: Some("amps".to_string()),
                },
                priority: Priority::Medium,
            },
        ],
        events: Some(EventsConfig {
            enabled: true,
            chunk_interval: "7 days".to_string(),
            retention: Some("1 year".to_string()),
            detection_schedule: "15 minutes".to_string(),
        }),
    }
}

/// Create a mock StreamConfig for "smart-meter" stream.
fn create_smart_meter_stream_config() -> StreamConfig {
    let mut fields_map = HashMap::new();
    fields_map.insert(
        "voltage".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string(), "max".to_string()],
        },
    );
    fields_map.insert(
        "current".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string()],
        },
    );
    fields_map.insert(
        "power_w".to_string(),
        FieldMetricsConfig {
            metrics: vec!["mean".to_string(), "sum".to_string()],
        },
    );

    StreamConfig {
        stream_id: "smart-meter".to_string(),
        fields: vec![
            FieldConfig { name: "voltage".to_string(), field_type: "float".to_string() },
            FieldConfig { name: "current".to_string(), field_type: "float".to_string() },
            FieldConfig { name: "power_w".to_string(), field_type: "float".to_string() },
        ],
        silver_etl: Some(SilverEtlConfig {
            target_table: "silver.smart_meter_observations".to_string(),
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

/// Create a mock StreamConfig for "grid-relay-state" stream.
fn create_relay_state_stream_config() -> StreamConfig {
    StreamConfig {
        stream_id: "grid-relay-state".to_string(),
        fields: vec![
            FieldConfig { name: "relay_state".to_string(), field_type: "string".to_string() },
            FieldConfig { name: "device_id".to_string(), field_type: "string".to_string() },
        ],
        silver_etl: Some(SilverEtlConfig {
            target_table: "silver.grid_relay_state".to_string(),
            timestamp: None,
        }),
        gold_etl: None,
    }
}
```

### 2.2 Test Specifications

#### HD-001: Detection Procedure Contains No Air-Quality Literals

```
Name:     test_detection_procedure_contains_no_air_quality_literals
Behavior: When generating detect_events for "energy-monitoring" domain,
          NONE of the air-quality-specific strings appear in the output.
Setup:    Create energy-monitoring DomainConfig with smart-meter + grid-relay-state
          streams and voltage/current objectives.
          Provide MockConfigLoader with smart-meter and relay-state StreamConfigs.
Assert:   For each string in FORBIDDEN_AIR_QUALITY_LITERALS, assert it does NOT
          appear in the generated SQL.
          For each fictional value (energy-monitoring, smart-meter, voltage, 240,
          safe_voltage, current, 30, efficient_current), assert it DOES appear.
Why:      This is the PRIMARY regression guard. If a future developer adds a new
          hardcoded air-quality reference, this test catches it immediately with
          a clear message: "Found forbidden literal 'X' in generated SQL for
          non-air-quality domain."
```

#### HD-002: State Transitions Section Uses Config Stream IDs

```
Name:     test_state_transitions_use_config_stream_id
Behavior: The STATE TRANSITIONS section of detect_events references the
          actuator stream from config, not hardcoded 'home-assistant-state'.
Setup:    Energy-monitoring domain where actuator is "grid-relay-state".
Assert:   SQL contains "grid_relay_state" (normalized).
          SQL does NOT contain "home_assistant_state" or "state_events".
          SQL references silver.grid_relay_state (from stream config silver_etl.target_table).
Why:      The current code hardcodes 'home-assistant-state' at line 475 and
          silver.state_events at line 481. Both must come from config.
```

#### HD-003: Threshold Crossings Section Uses Objectives Config

```
Name:     test_threshold_crossings_use_objectives_config
Behavior: The THRESHOLD CROSSINGS section generates one CTE per objective,
          using metric names, thresholds, and IDs from domain.objectives[].
Setup:    Energy-monitoring domain with voltage (240V) and current (30A) objectives.
Assert:   SQL contains "voltage_crossings" CTE (derived from metric name).
          SQL contains 240.0 as threshold (not 800.0).
          SQL contains 30.0 as threshold (not 12.0).
          SQL contains 'safe_voltage' as objective_id.
          SQL contains 'efficient_current' as objective_id.
          SQL does NOT contain 800, 12, "co2", "pm25".
Why:      Lines 538-590 currently hardcode two specific CTEs for co2 and pm25.
          The refactored code must loop over domain.objectives[] and generate
          one CTE per objective.
```

#### HD-004: Context Enrichment Uses Config-Derived Fields

```
Name:     test_context_enrichment_uses_config_fields
Behavior: The jsonb_build_object for context snapshots uses field names
          derived from the domain's stream configs, not hardcoded column names.
Setup:    Energy-monitoring domain with smart-meter fields (voltage, current, power_w).
Assert:   SQL context jsonb contains "voltage_mean" or "meter_voltage_mean" (from config).
          SQL does NOT contain "indoor_co2", "indoor_pm25", "outdoor_temp", etc.
Why:      Lines 505-510 and 609-616 hardcode 6 air-quality-specific context fields.
          The refactored code must derive these from the domain's stream configs.
```

#### HD-005: Unit Mapping Uses Objective Config

```
Name:     test_unit_mapping_uses_objective_config
Behavior: The unit CASE expression in details jsonb uses objective.target.unit
          from config, not hardcoded 'ppm'/'ug/m3' mapping.
Setup:    Energy-monitoring domain with voltage (unit: "volts") and current (unit: "amps").
Assert:   SQL contains "'volts'" and "'amps'" for units.
          SQL does NOT contain "'ppm'" or "'ug/m3'".
Why:      Line 621 hardcodes `WHEN 'co2' THEN 'ppm' ELSE 'ug/m3'`.
          The refactored code must read unit from objective.target.unit.
```

#### HD-006: Gold Table Name Derived From Config

```
Name:     test_gold_table_name_from_config
Behavior: The hourly observations CTE reads from a Gold table name derived
          from the stream config, not hardcoded 'gold.air_quality_hourly'.
Setup:    Energy-monitoring domain where primary stream is "smart-meter".
Assert:   SQL references "gold.smart_meter_hourly" (derived from stream_id).
          SQL does NOT reference "gold.air_quality_hourly".
Why:      Line 535 hardcodes gold.air_quality_hourly.
```

#### HD-007: Silver Table Name From Stream Config

```
Name:     test_silver_table_from_stream_config
Behavior: The state transitions CTE reads from the silver table defined in
          the actuator stream's silver_etl.target_table, not hardcoded.
Setup:    Energy-monitoring domain where actuator silver_etl.target_table
          = "silver.grid_relay_state".
Assert:   SQL contains "silver.grid_relay_state".
          SQL does NOT contain "silver.state_events" or "silver.home_assistant_state".
Why:      Line 481 hardcodes silver.state_events.
```

#### HD-008: Aligned View Name From Domain Config

```
Name:     test_aligned_view_name_from_domain_config
Behavior: The context enrichment subquery references the aligned view name
          from domain.alignment.view_name (already partially config-driven
          via domain_id_snake, but should use the exact config value).
Setup:    Energy-monitoring domain with alignment.view_name = "energy_monitoring_aligned".
Assert:   SQL contains "gold.energy_monitoring_aligned".
          SQL does NOT contain "indoor_air_quality_aligned".
Why:      Lines 511-512 already derive this from domain_id, but should verify
          it uses the canonical config view_name.
```

---

## 3. Config-Driven Generation Tests

These tests verify the positive behavior: that config values flow through correctly.

#### CG-001: Single Objective Generates Single CTE

```
Name:     test_single_objective_generates_single_crossing_cte
Behavior: A domain with one objective generates exactly one threshold crossing CTE.
Setup:    Domain with one objective (safe_voltage, threshold 240).
Assert:   Exactly one "*_crossings AS" CTE appears.
          The "all_crossings" UNION ALL has exactly one source.
Why:      The current code hardcodes exactly two CTEs. The refactored code
          must generate N CTEs for N objectives.
```

#### CG-002: Three Objectives Generate Three CTEs

```
Name:     test_three_objectives_generate_three_crossing_ctes
Behavior: A domain with three objectives generates three threshold crossing CTEs.
Setup:    Domain with voltage, current, and frequency objectives.
Assert:   Three "*_crossings AS" CTEs appear.
          The "all_crossings" UNION ALL has three sources.
Why:      Proves the loop-based generation works for arbitrary N.
```

#### CG-003: Zero Objectives Skips Threshold Section

```
Name:     test_zero_objectives_skips_threshold_crossings
Behavior: A domain with no objectives omits the THRESHOLD CROSSINGS section entirely.
Setup:    Domain with empty objectives vec.
Assert:   SQL does NOT contain "THRESHOLD CROSSINGS".
          SQL does NOT contain "*_crossings".
          State transitions section still present.
Why:      Edge case: not every domain needs threshold detection.
```

#### CG-004: Objective With No Unit Uses NULL

```
Name:     test_objective_without_unit_uses_null
Behavior: When an objective has unit: None, the details jsonb uses NULL for unit.
Setup:    Domain with one objective where unit is None.
Assert:   SQL contains "NULL" for the unit field in details jsonb.
Why:      Current code assumes all metrics have units.
```

#### CG-005: Entity Field From Stream Config

```
Name:     test_entity_field_from_stream_config
Behavior: The state transitions section uses the entity field name from the
          actuator stream config, not hardcoded "ndp_id".
Setup:    Actuator stream where entity field is "device_id" (not "ndp_id").
Assert:   SQL contains "s.device_id AS entity_id" (or equivalent).
          SQL does NOT contain "s.ndp_id" in the state transitions CTE.
Why:      Line 476 hardcodes s.ndp_id. Must read from config.
```

#### CG-006: State Field From Stream Config

```
Name:     test_state_field_from_stream_config
Behavior: LAG() in state transitions uses the state field name from config.
Setup:    Actuator stream where state field is "relay_state" (not "state").
Assert:   SQL contains "LAG(s.relay_state)".
          SQL does NOT contain "LAG(s.state)" in the state transitions CTE.
Why:      Line 478 hardcodes s.state. Must read from config.
```

---

## 4. Cross-Cutting Constant Tests

These tests enforce that shared constants are used consistently.

#### CC-001: ndp_id Column Name Consistency

```
Name:     test_ndp_id_uses_shared_constant_or_config
Behavior: The string "ndp_id" appears either:
          (a) via a shared constant (NDP_ENTITY_COLUMN), or
          (b) via stream config entity_field.
          It never appears as a raw string literal in generator logic.
Setup:    Source code scan (see Section 6).
Assert:   The string literal "ndp_id" does NOT appear in generator .rs files
          outside of: constant definitions, test code, or config default values.
Why:      "ndp_id" is scattered across 5 files as a magic string.
```

#### CC-002: Gold Schema Name Consistency

```
Name:     test_gold_schema_uses_constant
Behavior: The schema name "gold" is defined as a constant and used everywhere.
Setup:    Source code scan.
Assert:   Raw string "gold." or "'gold'" does NOT appear in generator .rs files
          outside of: constant definitions, test code, format strings using the constant.
Why:      "gold" appears in 6+ locations as a raw string.
```

---

## 5. Mock Strategy

### 5.1 Mock Boundaries

| Component | Mocked? | Why |
|-----------|---------|-----|
| `DomainConfig` | Real struct, fictional data | Config is a plain struct - no I/O |
| `StreamConfig` | Real struct, fictional data | Same |
| `ObjectiveConfig` | Real struct, fictional data | Same |
| `ConfigLoader` | MockConfigLoader | Avoids file system dependency |
| `EventsGenerator` | Real (SUT) | This is what we are testing |
| `SQL string output` | Assertion target | We inspect the output, not a mock |
| `Database connection` | Not needed for unit tests | SQL generation is pure string output |

### 5.2 Mock ConfigLoader Setup

```rust
// Reuse existing MockConfigLoader from tests/fixtures/phase_c.rs
let loader = MockConfigLoader::new()
    .with_stream("smart-meter", create_smart_meter_stream_config())
    .with_stream("grid-relay-state", create_relay_state_stream_config())
    .with_domain("energy-monitoring", create_energy_monitoring_domain());
```

### 5.3 What Stays Real

- `EventsGenerator` - the system under test
- `DomainConfig`, `StreamConfig`, `ObjectiveConfig` - plain data structs
- SQL string output - we assert against the actual generated string
- `Action::Sync` / `Action::Recreate` - enum values

### 5.4 Fictional Domain Fixtures

Two fictional domains are needed:

| Domain | Streams | Objectives | Purpose |
|--------|---------|------------|---------|
| `energy-monitoring` | smart-meter (Primary), grid-relay-state (Actuator) | safe_voltage (240V), efficient_current (30A) | Primary hardcoding detection |
| `greenhouse-control` | soil-sensors (Primary), irrigation-valve-state (Actuator) | optimal_moisture (0.6), safe_temperature (35C) | Secondary validation, different naming |

The greenhouse-control domain provides a second fictional domain to verify the generator is truly generic, not just accidentally passing with one particular domain.

---

## 6. Regression Prevention: Source Code Scan Tests

These tests scan the Rust source files directly to catch hardcoded patterns.

#### RS-001: No Domain-Specific Literals in Generator Source

```
Name:     test_no_air_quality_literals_in_generator_source
Behavior: The Rust source files in generators/ contain no air-quality-specific
          string literals (excluding test modules and comments).
Setup:    Read each .rs file in src/generators/.
          Strip out #[cfg(test)] modules and line comments.
Assert:   None of the following appear as string literals:
          "home-assistant-state", "air-quality", "co2", "pm25",
          "800", "12.0", "state_events", "air_quality_hourly",
          "healthy_co2", "healthy_pm25", "ppm", "ug/m3",
          "indoor_co2", "indoor_pm25", "outdoor_temperature",
          "window_state", "hvac_mode", "door_%", "window_%", "motion_%", "light_%"
Why:      This catches hardcoded values at the source level, not just in output.
          A developer cannot add a new hardcoded value without this test failing.
```

#### RS-002: Blessed Constant List

```
Name:     test_generator_source_uses_only_blessed_literals
Behavior: Any string literal in generators/ that looks like a SQL identifier
          or domain value must be in the blessed list OR come from a const/config.
Setup:    Parse string literals from generator source.
Assert:   Every string literal is either:
          (a) in the BLESSED_STRUCTURAL list, or
          (b) a SQL keyword (SELECT, FROM, INSERT, etc.), or
          (c) a format placeholder ({}, {field}), or
          (d) a comment string
Why:      Proactive defense against new hardcoding.
```

#### RS-003: state_transitions.rs No Hardcoded State Values

```
Name:     test_state_transitions_no_hardcoded_states
Behavior: state_transitions.rs does not contain hardcoded 'off'/'on' strings
          in generator logic (outside of test code).
Setup:    Read state_transitions.rs, strip test modules.
Assert:   "'off'" and "'on'" do not appear in non-test, non-comment code.
          "'door_%'", "'window_%'", "'motion_%'", "'light_%'" do not appear
          in non-test, non-comment code.
Why:      Lines 296-297 hardcode 'off'/'on', lines 309-312 hardcode device patterns.
```

#### RS-004: aligned_view.rs No String-Based Type Inference

```
Name:     test_aligned_view_no_string_type_inference
Behavior: aligned_view.rs does not use string matching on stream_id to
          determine StreamType.
Setup:    Read aligned_view.rs, strip test modules.
Assert:   The pattern `contains("forecast")`, `contains("state")`,
          `contains("event")`, `contains("dimension")`, `contains("ref")`
          does not appear in determine_stream_type or equivalent.
Why:      Lines 122-127 use string matching instead of reading stream config.
          StreamType should come from config, not inference.
```

---

## 7. Integration Test Plan

Integration tests verify generated SQL executes correctly against a real TimescaleDB.

### 7.1 Prerequisites

```bash
# Start integration environment
docker compose -f docker-compose.integration.yml up -d timescaledb

# Wait for health
docker compose -f docker-compose.integration.yml exec timescaledb \
    pg_isready -U postgres -d ndp
```

### 7.2 Connection Details

```
Host: localhost
Port: 5432
Database: ndp
User: postgres
Password: postgres
URL: postgresql://postgres:postgres@localhost:5432/ndp
```

### 7.3 Test Database Setup/Teardown

```rust
// tests/integration/events_integration_tests.rs

use tokio_postgres::{Client, NoTls};

/// Connect to integration TimescaleDB.
async fn connect() -> Client {
    let (client, connection) = tokio_postgres::connect(
        "host=localhost port=5432 user=postgres password=postgres dbname=ndp",
        NoTls,
    ).await.expect("Failed to connect to integration TimescaleDB");

    tokio::spawn(async move { connection.await.unwrap(); });
    client
}

/// Create prerequisite schemas and tables for events tests.
async fn setup_prerequisites(client: &Client) {
    // Create schemas
    client.batch_execute("
        CREATE SCHEMA IF NOT EXISTS gold;
        CREATE SCHEMA IF NOT EXISTS silver;
        CREATE EXTENSION IF NOT EXISTS timescaledb;
    ").await.unwrap();

    // Create a minimal Silver table for state events
    client.batch_execute("
        CREATE TABLE IF NOT EXISTS silver.grid_relay_state (
            event_time TIMESTAMPTZ NOT NULL,
            ndp_id TEXT NOT NULL,
            relay_state TEXT NOT NULL
        );
    ").await.unwrap();

    // Create a minimal Gold hourly table for threshold crossings
    client.batch_execute("
        CREATE TABLE IF NOT EXISTS gold.smart_meter_hourly (
            bucket TIMESTAMPTZ NOT NULL,
            ndp_id TEXT NOT NULL,
            voltage_mean DOUBLE PRECISION,
            current_mean DOUBLE PRECISION,
            power_w_mean DOUBLE PRECISION,
            sample_count BIGINT
        );
    ").await.unwrap();

    // Create the aligned view stub
    client.batch_execute("
        CREATE OR REPLACE VIEW gold.energy_monitoring_aligned AS
        SELECT
            NOW() AS bucket,
            0.0::DOUBLE PRECISION AS meter_voltage_mean,
            0.0::DOUBLE PRECISION AS meter_current_mean,
            'off'::TEXT AS relay_relay_state_last;
    ").await.unwrap();
}

/// Clean up after tests.
async fn teardown(client: &Client) {
    client.batch_execute("
        DROP SCHEMA IF EXISTS gold CASCADE;
        DROP TABLE IF EXISTS silver.grid_relay_state CASCADE;
    ").await.unwrap();
}
```

### 7.4 Integration Test Specifications

#### IT-001: Generated Events DDL Executes Without Error

```
Name:     test_generated_events_ddl_executes_successfully
Behavior: SQL generated from energy-monitoring config executes against
          real TimescaleDB without errors.
Setup:    Connect to integration DB.
          Run setup_prerequisites().
          Generate DDL using EventsGenerator with energy-monitoring config.
Assert:   client.batch_execute(&sql).await is Ok.
          gold.events table exists (query pg_tables).
          gold.events_unified view exists.
          gold.events_hourly materialized view exists.
          gold.events_hourly_by_entity materialized view exists.
          gold.detect_events procedure exists.
Teardown: teardown().
Mark:     #[tokio::test] #[ignore] (requires running infrastructure).
Why:      Proves the generated SQL is syntactically and semantically valid.
```

#### IT-002: Detection Procedure Runs Without Error

```
Name:     test_detection_procedure_runs_without_error
Behavior: The generated detect_events procedure can be called and completes
          without SQL errors.
Setup:    Execute full events DDL from IT-001.
          Insert test data into silver.grid_relay_state.
          Insert test data into gold.smart_meter_hourly.
Assert:   CALL gold.detect_events(0, '{}'::JSONB) succeeds.
          RAISE NOTICE output contains event counts.
Teardown: teardown().
Mark:     #[tokio::test] #[ignore]
Why:      Validates the procedure logic is executable, not just parseable.
```

#### IT-003: Threshold Crossing Detection Produces Events

```
Name:     test_threshold_crossing_produces_events
Behavior: When Gold hourly data crosses the voltage threshold (240V),
          the procedure inserts a threshold_crossing event.
Setup:    Execute full events DDL.
          Insert two consecutive hourly rows into gold.smart_meter_hourly:
            bucket=T1, voltage_mean=230 (below 240)
            bucket=T2, voltage_mean=250 (above 240)
          Call detect_events.
Assert:   gold.events contains a row with:
            event_type = 'threshold_crossing'
            metric = 'voltage'
            threshold_value = 240.0
            crossing_direction = 'rising'
            objective_id = 'safe_voltage'
Teardown: teardown().
Mark:     #[tokio::test] #[ignore]
Why:      End-to-end validation that config-driven threshold detection works.
```

#### IT-004: State Transition Detection Produces Events

```
Name:     test_state_transition_produces_events
Behavior: When Silver state data shows a state change, the procedure
          inserts a state_transition event.
Setup:    Execute full events DDL.
          Insert two rows into silver.grid_relay_state:
            event_time=T1, ndp_id='relay_1', relay_state='closed'
            event_time=T2, ndp_id='relay_1', relay_state='open'
          Schedule job for detect_events and call it.
Assert:   gold.events contains a row with:
            event_type = 'state_transition'
            from_state = 'closed'
            to_state = 'open'
            entity_id = 'relay_1'
Teardown: teardown().
Mark:     #[tokio::test] #[ignore]
Why:      End-to-end validation that config-driven state detection works.
```

#### IT-005: Sync Mode Is Idempotent

```
Name:     test_sync_mode_idempotent
Behavior: Running DDL in sync mode twice does not error.
Setup:    Generate DDL with Action::Sync.
          Execute DDL once.
          Execute DDL again.
Assert:   Both executions succeed without error.
          gold.events table has the same structure after both runs.
Teardown: teardown().
Mark:     #[tokio::test] #[ignore]
Why:      Sync mode must be safe to run repeatedly (production deployment pattern).
```

---

## 8. Test File Organization

```
tools/ndp-gold-ddl/
  tests/
    fixtures/
      mod.rs              # (existing) re-exports
      phase_c.rs          # (existing) Phase C fixtures
      energy_monitoring.rs # (NEW) fictional domain fixtures for OPS-002
    ops002_hardcoding_tests.rs        # (NEW) HD-001 through HD-008
    ops002_config_driven_tests.rs     # (NEW) CG-001 through CG-006
    ops002_source_scan_tests.rs       # (NEW) RS-001 through RS-004
    ops002_constant_tests.rs          # (NEW) CC-001, CC-002
    integration/
      events_integration_tests.rs     # (NEW) IT-001 through IT-005
```

All new test files are prefixed `ops002_` for clear identification and to group them in test output.

---

## 9. Completion Criteria

### 9.1 Hardcoding Detection (Must Pass)

- [ ] HD-001: Zero air-quality literals in energy-monitoring SQL
- [ ] HD-002: State transitions use config stream IDs
- [ ] HD-003: Threshold crossings use objectives config
- [ ] HD-004: Context enrichment uses config-derived fields
- [ ] HD-005: Unit mapping uses objective config
- [ ] HD-006: Gold table name derived from config
- [ ] HD-007: Silver table name from stream config
- [ ] HD-008: Aligned view name from domain config

### 9.2 Config-Driven Generation (Must Pass)

- [ ] CG-001: Single objective generates single CTE
- [ ] CG-002: Three objectives generate three CTEs
- [ ] CG-003: Zero objectives skips threshold section
- [ ] CG-004: Objective without unit uses NULL
- [ ] CG-005: Entity field from stream config
- [ ] CG-006: State field from stream config

### 9.3 Cross-Cutting Constants (Must Pass)

- [ ] CC-001: ndp_id uses shared constant or config
- [ ] CC-002: Gold schema uses constant

### 9.4 Source Code Scan (Must Pass)

- [ ] RS-001: No air-quality literals in generator source
- [ ] RS-002: Generator uses only blessed literals
- [ ] RS-003: state_transitions.rs no hardcoded states
- [ ] RS-004: aligned_view.rs no string-based type inference

### 9.5 Integration Tests (Must Pass When Infrastructure Available)

- [ ] IT-001: Generated DDL executes without error
- [ ] IT-002: Detection procedure runs without error
- [ ] IT-003: Threshold crossing produces events
- [ ] IT-004: State transition produces events
- [ ] IT-005: Sync mode is idempotent

### 9.6 Existing Tests (Must Not Regress)

- [ ] All 339 existing ndp-gold-ddl tests pass
- [ ] Golden master tests updated with new baselines after refactor
- [ ] All 217 existing ndp-validate tests pass

---

## 10. Refactoring Strategy (What Changes in the Generator)

This section outlines what the implementation agent must change to make the tests pass. The tests are written first; these changes come second.

### 10.1 generate_detection_procedure() Signature Change

Current: `fn generate_detection_procedure(&self) -> Result<String>`

Proposed: The method must accept (or the `EventsGenerator` must hold) a reference to:
- `&[ObjectiveConfig]` - domain objectives for threshold crossings
- Stream config for the actuator stream (silver table, entity field, state field)
- Stream config for the primary stream (gold table name, metric columns)

This can be achieved by expanding `EventsGenerator::from_domain_config()` to also accept a `ConfigLoader`, or by adding config fields to the struct.

### 10.2 Threshold Crossings: Loop Over Objectives

Replace the hardcoded `co2_crossings` / `pm25_crossings` CTEs with a loop:

```
for each objective in domain.objectives:
    generate CTE named "{metric}_crossings"
    use objective.target.threshold
    use objective.target.metric + "_mean" as the column (from Gold CA)
    use objective.id as objective_id
    use objective.target.unit for details jsonb
```

### 10.3 State Transitions: Read From Config

Replace hardcoded references with:
- Silver table: `stream_config.silver_etl.target_table`
- Entity field: from config (new field in TransitionsConfig or StreamConfig)
- State field: from config (new field in TransitionsConfig or StreamConfig)
- Stream ID literal: `domain.streams[role=actuator].stream_id`

### 10.4 Context Enrichment: Derive From Streams

Replace hardcoded `jsonb_build_object(...)` with dynamic field list derived from:
- Each stream's Gold CA columns (from `stream_config.gold_etl.aggregates.fields`)
- Prefixed with the stream alias from `domain.streams[].alias`

### 10.5 Golden Master Baseline Update

After refactoring, the generated SQL for the existing air-quality domain will change format (dynamic CTEs instead of hardcoded ones). The golden master baselines must be re-captured:

```bash
./scripts/capture-golden-master.sh
```

The golden master tests then verify the NEW output format is stable.

---

## 11. Risk Assessment

| Risk | Mitigation |
|------|------------|
| Existing 339 tests break during refactor | Run full suite after each change; golden master catches drift |
| Fictional domain config incomplete | Thorough fixture with all required fields; compiler errors catch missing fields |
| Integration tests flaky due to Docker timing | Use health checks, retry logic, and `start_period` in compose |
| New hardcoded values introduced post-fix | RS-001 source scan test runs in CI; catches any new literals |
| Config structure needs new fields | Keep backward compatible; new fields use `#[serde(default)]` |
| Threshold crossing CTE generation order | Generate deterministically (sort objectives by id) |
