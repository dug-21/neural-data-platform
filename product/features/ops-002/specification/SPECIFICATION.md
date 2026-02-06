# OPS-002 Specification: Eliminate Hardcoded References from Gold Layer Generators

> **Feature ID:** ops-002
> **SPARC Phase:** Specification
> **Version:** 1.0
> **Date:** 2026-02-06
> **Status:** Draft

---

## 1. Golden Rule

**Zero hardcoded domain-specific values in generators.**

A generator must produce correct SQL for any domain using only configuration inputs. If a second domain (e.g., `energy-monitoring`) cannot produce working SQL without modifying Rust code, the feature is incomplete.

---

## 2. Functional Requirements

### 2.1 P0 Critical -- Detection Procedure (`events.rs::generate_detection_procedure`)

#### FR-001: State Transitions Read Silver Table from StreamConfig

| Attribute | Value |
|-----------|-------|
| **ID** | FR-001 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/events.rs` lines 472-517 |
| **Current behavior** | Hardcodes `silver.state_events` and `'home-assistant-state'` as literals |
| **Required behavior** | For each stream in `DomainConfig.streams` that has `StreamType::StateEvent`, read `silver_etl.target_table` from its `StreamConfig` to determine the Silver source table. Read the `stream_id` from the `StreamRef` for the literal. |
| **Config sources** | `DomainConfig.streams[].stream_id` (stream literal), `StreamConfig.silver_etl.target_table` (Silver table) |

**Acceptance Criteria:**
- AC-001a: Generated SQL references the Silver table from `StreamConfig.silver_etl.target_table`, not a hardcoded string.
- AC-001b: The stream_id literal in the INSERT uses `DomainConfig.streams[].stream_id`, not a hardcoded value.
- AC-001c: If no stream with `StreamType::StateEvent` exists in the domain, the state transitions section is omitted entirely.
- AC-001d: Works for a fictional `StreamConfig` with `target_table = "silver.device_events"` and `stream_id = "smart-home-devices"`.

#### FR-002: State Transitions Read Column Names from StreamConfig

| Attribute | Value |
|-----------|-------|
| **ID** | FR-002 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/events.rs` lines 478-480 |
| **Current behavior** | Hardcodes column names `ndp_id`, `state`, `event_time` |
| **Required behavior** | Read the entity column from `TransitionConfig.entity_field` (which comes from `StreamConfig.gold_etl.features.transitions`). Read the state field from `TransitionConfig.state_field`. Read the timestamp column from `StreamConfig.silver_etl.timestamp.target_field`. |
| **Config sources** | `TransitionConfig.entity_field`, `TransitionConfig.state_field`, `StreamConfig.silver_etl.timestamp.target_field` |

**Acceptance Criteria:**
- AC-002a: Generated SQL uses the entity field from config, not hardcoded `ndp_id`.
- AC-002b: Generated SQL uses the state field from config, not hardcoded `state`.
- AC-002c: Generated SQL uses the timestamp field from config, not hardcoded `event_time`.
- AC-002d: A fictional config with `entity_field = "device_id"`, `state_field = "power_state"`, `timestamp = "recorded_at"` produces valid SQL using those names.

#### FR-003: Threshold Crossings Read from Domain Objectives

| Attribute | Value |
|-----------|-------|
| **ID** | FR-003 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/events.rs` lines 526-590 |
| **Current behavior** | Hardcodes stream `'air-quality'`, table `gold.air_quality_hourly`, columns `co2_mean`/`pm25_mean`, thresholds `800`/`12`, metric names `'co2'`/`'pm25'`, objective IDs `'healthy_co2'`/`'healthy_pm25'`, and units `'ppm'`/`'ug/m3'` |
| **Required behavior** | Iterate over `DomainConfig.objectives[]`. For each objective, resolve the target stream's Gold hourly CA table name from the stream's config. Use `objective.target.metric` to derive the column name pattern (`{metric}_mean`). Use `objective.target.threshold`, `objective.target.condition`, `objective.id`, and `objective.target.unit` directly from config. |
| **Config sources** | `DomainConfig.objectives[]`, `ObjectiveConfig.target.{stream, metric, condition, threshold, unit}`, `ObjectiveConfig.id` |

**Acceptance Criteria:**
- AC-003a: Generated SQL produces one threshold crossing CTE per objective.
- AC-003b: Threshold values come from `ObjectiveConfig.target.threshold`, not hardcoded numbers.
- AC-003c: Metric column names are derived from `ObjectiveConfig.target.metric` using the pattern `{metric}_mean` (matching Gold CA column naming).
- AC-003d: Objective IDs come from `ObjectiveConfig.id`.
- AC-003e: Stream IDs and Gold table names are resolved from config, not hardcoded.
- AC-003f: Units come from `ObjectiveConfig.target.unit`.
- AC-003g: The crossing direction logic (`rising`/`falling`) is generated for each objective based on `ObjectiveConfig.target.condition`.
- AC-003h: If `DomainConfig.objectives` is empty, the threshold crossings section is omitted entirely.
- AC-003i: Six objectives in `indoor-air-quality/domain.json` (co2, pm25, humidity min/max, temp min/max) all produce crossing CTEs.

#### FR-004: Context Enrichment Reads Fields from Aligned View Columns

| Attribute | Value |
|-----------|-------|
| **ID** | FR-004 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/events.rs` lines 503-514, 609-620 |
| **Current behavior** | Hardcodes context fields: `indoor_co2_mean`, `indoor_pm25_mean`, `indoor_temperature_c_mean`, `outdoor_temperature_c_mean`, `outdoor_aqi_pm25_mean`, `state_state_last` |
| **Required behavior** | Build the context `jsonb_build_object()` dynamically from the aligned view's known columns. These columns are derivable from `DomainConfig.streams[]` combined with each stream's `StreamConfig.gold_etl.aggregates.fields` and their aliases. |
| **Config sources** | `DomainConfig.streams[].alias`, `StreamConfig.gold_etl.aggregates.fields`, `DomainConfig.alignment.view_name` |

**Acceptance Criteria:**
- AC-004a: The context JSONB keys are derived from the aligned view column names, not hardcoded.
- AC-004b: The aligned view name comes from `DomainConfig.alignment.view_name`.
- AC-004c: Adding a new stream to the domain config automatically adds its fields to the context enrichment.
- AC-004d: Context enrichment works for a domain with different streams than `indoor-air-quality`.

### 2.2 P0 Critical -- State Transitions (`state_transitions.rs`)

#### FR-005: Device Type Patterns Read from Config

| Attribute | Value |
|-----------|-------|
| **ID** | FR-005 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/state_transitions.rs` lines 306-317 |
| **Current behavior** | Hardcodes device type CASE with patterns: `door_%`, `window_%`, `motion_%`, `light_%` |
| **Required behavior** | Accept device type patterns from configuration. The `TransitionConfig` (or a new `DeviceTypeConfig`) should define a list of `(pattern, device_type)` pairs. If no device type config is provided, omit the `device_type` column entirely rather than guessing. |
| **Config sources** | New field on `TransitionConfig`: `device_types: Option<Vec<DeviceTypeMapping>>` |

**Acceptance Criteria:**
- AC-005a: Device type patterns come from config, not hardcoded LIKE patterns.
- AC-005b: If no device type config is present, the `device_type` column is not generated.
- AC-005c: Custom patterns (e.g., `sensor_%` -> `sensor`, `plug_%` -> `smart_plug`) work correctly.

#### FR-006: Default Direction Logic Reads from Config

| Attribute | Value |
|-----------|-------|
| **ID** | FR-006 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/state_transitions.rs` lines 293-303 |
| **Current behavior** | `generate_default_direction_case()` hardcodes binary state values `'off'`/`'on'` and directions `'opening'`/`'closing'` |
| **Required behavior** | When no `direction_mapping` is provided, read the valid states from `TransitionsConfig.states` (already exists in config: `["on", "off"]`). Use the first two states as the binary pair. Or better: require a `direction_mapping` when direction is desired, and omit the direction column if neither mapping nor states are provided. |
| **Config sources** | `TransitionsConfig.states`, `TransitionConfig.direction_mapping` |

**Acceptance Criteria:**
- AC-006a: No hardcoded `'on'`/`'off'`/`'opening'`/`'closing'` in generator code.
- AC-006b: If `direction_mapping` is provided, it is used (this already works).
- AC-006c: If neither `direction_mapping` nor `states` are provided, the `transition_direction` column is omitted.
- AC-006d: States like `["open", "closed"]` with appropriate mapping produce `'opening'`/`'closing'` correctly.

### 2.3 P0 Critical -- Aligned View (`aligned_view.rs`)

#### FR-007: Stream Type Read from StreamConfig, Not Inferred by String Matching

| Attribute | Value |
|-----------|-------|
| **ID** | FR-007 |
| **Priority** | P0 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/aligned_view.rs` lines 121-131 |
| **Current behavior** | `determine_stream_type()` uses string matching: `if stream_id.contains("forecast")` -> Forecast, `contains("state")` -> StateEvent, etc. |
| **Required behavior** | Read `stream_type` from `StreamConfig`. The `StreamConfig` struct already supports deserialization of `StreamType`. The `ConfigLoader` can provide this. If `stream_type` is not set in config, fall back to `StreamType::Observation` as the default. |
| **Config sources** | `StreamConfig.stream_type` (new field, or inferred from `StreamConfig.silver_etl` structure) |

**Acceptance Criteria:**
- AC-007a: `determine_stream_type()` reads `stream_type` from the loaded `StreamConfig`, not from string matching on `stream_id`.
- AC-007b: A stream named `"temperature-readings"` with `stream_type: "forecast"` in config is treated as a Forecast stream.
- AC-007c: A stream named `"forecast-data"` with `stream_type: "observation"` in config is treated as an Observation stream (config wins over naming).
- AC-007d: When `stream_type` is absent from config, defaults to `Observation`.

### 2.4 P1 Should Fix -- Entity Column Constant

#### FR-008: Entity Column Name as Configurable Default

| Attribute | Value |
|-----------|-------|
| **ID** | FR-008 |
| **Priority** | P1 |
| **Source file** | `state_transitions.rs:67`, `continuous_aggregate.rs:46`, `main.rs:313` |
| **Current behavior** | `"ndp_id"` hardcoded in 5 locations as the entity column |
| **Required behavior** | Define `ndp_id` as a named constant `DEFAULT_ENTITY_COLUMN` in a shared location (e.g., `ndp-gold-ddl/src/config/mod.rs` or `ndp-types`). All generators reference this constant. The constant is overridable per-stream via `StreamConfig` if a future stream uses a different entity column name. |

**Acceptance Criteria:**
- AC-008a: Zero raw `"ndp_id"` string literals in generator code (only the constant definition).
- AC-008b: All generators reference `DEFAULT_ENTITY_COLUMN` or a config-provided override.
- AC-008c: Grep for `"ndp_id"` in `src/generators/` returns zero results (excluding test fixtures).

### 2.5 P1 Should Fix -- Schema Name Constant

#### FR-009: Gold Schema Name as Named Constant

| Attribute | Value |
|-----------|-------|
| **ID** | FR-009 |
| **Priority** | P1 |
| **Source file** | Multiple files: `events.rs`, `continuous_aggregate.rs`, `state_transitions.rs`, `aligned_view.rs`, `planner/sync.rs`, `refresh_policy.rs` |
| **Current behavior** | `"gold"` schema name hardcoded in 6+ source files as string literals in SQL templates |
| **Required behavior** | Define `GOLD_SCHEMA` as a named constant. All generators reference the constant. This is not about making the schema name runtime-configurable (Gold layer is always `gold`), but about having a single source of truth. |

**Acceptance Criteria:**
- AC-009a: A single constant `GOLD_SCHEMA` is defined.
- AC-009b: All SQL generation references the constant, not a raw `"gold"` string in schema positions.
- AC-009c: Changing the constant to `"gold_v2"` and recompiling produces SQL with `gold_v2.events` instead of `gold.events`.

### 2.6 P1 Should Fix -- Forecast Timestamp Column

#### FR-010: Forecast Alignment Column from Config

| Attribute | Value |
|-----------|-------|
| **ID** | FR-010 |
| **Priority** | P1 |
| **Source file** | `tools/ndp-gold-ddl/src/generators/join_builder.rs` lines 122-124 |
| **Current behavior** | Hardcodes `f.issued_at` as the forecast alignment column |
| **Required behavior** | Read the forecast timestamp column from the stream's `StreamConfig` (e.g., `silver_etl.timestamp.target_field` or a new `forecast_alignment_column` field). Default to `"issued_at"` if not specified. |

**Acceptance Criteria:**
- AC-010a: Forecast join uses a configurable column name, not hardcoded `issued_at`.
- AC-010b: Default behavior is unchanged when config does not specify the column.
- AC-010c: A forecast stream with `alignment_column: "forecast_time"` uses `f.forecast_time` in the LATERAL join.

---

## 3. Non-Functional Requirements

### NFR-001: Backward Compatibility

The generated SQL for the `indoor-air-quality` domain MUST be byte-identical (or semantically identical) to the current output when using the existing config files. Existing deployments must not break.

**Validation:** Capture current output as golden files. Run generator with updated code + same config. Diff must be empty or contain only whitespace/comment changes.

### NFR-002: Performance

Config loading adds negligible overhead. The generator already loads configs via `ConfigLoader`; this feature adds at most 1-2 additional `load_stream_config()` calls per domain.

**Target:** Total generation time for a domain < 100ms (currently ~10ms).

### NFR-003: Error Messages

When config is missing a required field (e.g., `silver_etl.target_table` for a state event stream referenced in objectives), the error message must:
- Name the missing field
- Name the stream_id that is missing it
- Suggest what config file to check

### NFR-004: No Regression in Test Count

The existing 339 passing tests in ndp-gold-ddl must continue passing. New tests are additive.

---

## 4. Test Strategy (London TDD)

### 4.1 Philosophy

London School TDD: mock collaborators, test behavior through interfaces. Each generator test injects a `MockConfigLoader` and verifies the SQL output contains expected config-derived values.

### 4.2 Unit Tests with Mocks

For each FR, write tests following this pattern:

```
Given: A MockConfigLoader returning a fictional StreamConfig
When:  Generator produces SQL
Then:  SQL contains values from the mock config, not hardcoded defaults
```

**FR-001/FR-002 tests (state transitions in detection procedure):**
- Mock a StateEvent stream with `target_table = "silver.device_states"`, `stream_id = "smart-home"`, `entity_field = "device_id"`, `state_field = "power_state"`
- Assert generated SQL references `silver.device_states`, `'smart-home'`, `device_id`, `power_state`
- Assert SQL does NOT contain `silver.state_events`, `'home-assistant-state'`, or raw `ndp_id`

**FR-003 tests (threshold crossings):**
- Mock a domain with 2 objectives: `{metric: "voltage", threshold: 240, unit: "V"}` and `{metric: "current", threshold: 15, unit: "A"}`
- Assert generated SQL contains `voltage_mean`, `240.0`, `current_mean`, `15.0`
- Assert SQL does NOT contain `co2_mean`, `pm25_mean`, `800`, `12`

**FR-004 tests (context enrichment):**
- Mock aligned view columns derived from a 2-stream domain with aliases `"power"` and `"grid"`
- Assert context JSONB keys are `power_*` and `grid_*`, not `indoor_*` or `outdoor_*`

**FR-005 tests (device types):**
- Mock `TransitionConfig` with `device_types: [("pump_%", "pump"), ("valve_%", "valve")]`
- Assert generated SQL contains `LIKE 'pump_%'` and `LIKE 'valve_%'`
- Assert SQL does NOT contain `door_%` or `window_%`

**FR-006 tests (default direction):**
- Mock `TransitionConfig` with no `direction_mapping` and no `states`
- Assert `transition_direction` column is not in the output

**FR-007 tests (stream type from config):**
- Mock a stream with `stream_id = "my-predictions"` and `stream_type = Observation`
- Assert it is NOT treated as Forecast despite the name

**FR-008 tests (entity column constant):**
- Grep-based test: scan all `.rs` files in `src/generators/` for raw `"ndp_id"` string literals; assert count is zero (excluding constant definition and test fixtures)

**FR-009 tests (schema constant):**
- Change `GOLD_SCHEMA` to `"test_gold"` in test context; assert generated SQL uses `test_gold.events`

**FR-010 tests (forecast column):**
- Mock a forecast stream with `alignment_column = "prediction_time"`; assert LATERAL join uses `f.prediction_time`

### 4.3 Hardcoding Detection Tests

Create a dedicated test module: `tests/no_hardcoded_domain_values.rs`

These tests scan the generated SQL output for known domain-specific literals that should not appear when generating for a non-air-quality domain:

```rust
// Forbidden domain-specific literals when generating for "energy-monitoring" domain
const FORBIDDEN_LITERALS: &[&str] = &[
    "silver.state_events",       // Silver table name
    "home-assistant-state",       // Stream ID
    "air-quality",               // Stream ID
    "'co2'",                     // Metric name
    "'pm25'",                    // Metric name
    "co2_mean",                  // Column name
    "pm25_mean",                 // Column name
    "800",                       // CO2 threshold
    "'ppm'",                     // Unit
    "'ug/m3'",                   // Unit
    "healthy_co2",               // Objective ID
    "healthy_pm25",              // Objective ID
    "indoor_co2",                // Context field
    "indoor_pm25",               // Context field
    "indoor_temperature_c",      // Context field
    "outdoor_temperature_c",     // Context field
    "outdoor_aqi_pm25",          // Context field
    "state_state_last",          // Context field
    "door_%",                    // Device type pattern
    "window_%",                  // Device type pattern
    "motion_%",                  // Device type pattern
    "light_%",                   // Device type pattern
    "'on'",                      // Binary state
    "'off'",                     // Binary state
    "'opening'",                 // Direction
    "'closing'",                 // Direction
];
```

Each test generates SQL for a fictional domain and asserts none of the forbidden literals appear.

### 4.4 Regression Guard: Fictional Domain Tests

Create an `energy-monitoring` test fixture:

```json
{
  "id": "energy-monitoring",
  "streams": [
    {"stream_id": "power-meter", "alias": "power", "role": "primary"},
    {"stream_id": "grid-status", "alias": "grid", "role": "context"},
    {"stream_id": "smart-switch-state", "alias": "switch", "role": "actuator"}
  ],
  "alignment": {
    "view_name": "energy_monitoring_aligned",
    "granularity": "1 hour"
  },
  "events": {"enabled": true},
  "objectives": [
    {
      "id": "peak_power",
      "target": {"stream": "power-meter", "metric": "watts", "condition": "<", "threshold": 3000, "unit": "W"}
    },
    {
      "id": "min_voltage",
      "target": {"stream": "power-meter", "metric": "voltage", "condition": ">=", "threshold": 210, "unit": "V"}
    }
  ]
}
```

With corresponding mock `StreamConfig` entries for each stream. The test verifies:
1. SQL generates without errors.
2. SQL references `silver.power_meter_observations`, `silver.smart_switch_states` (from mock StreamConfigs).
3. SQL contains `watts_mean`, `3000`, `voltage_mean`, `210`.
4. SQL does NOT contain any air-quality-specific literals (run against FORBIDDEN_LITERALS).
5. SQL is syntactically valid (parses without error if we add a SQL parser, or at minimum balanced parentheses and semicolons).

### 4.5 Integration Tests

Using `docker-compose.integration.yml` with TimescaleDB:

1. **Schema creation test:** Execute the generated DDL against a real TimescaleDB instance. Verify tables, views, continuous aggregates, and procedures are created.
2. **Detection procedure test:** Insert test data into Silver tables, run the detection procedure, verify events appear in `gold.events`.
3. **Aligned view test:** Verify the generated aligned view query executes without errors against real tables.
4. **Idempotency test:** Run the same DDL twice in `sync` mode. Verify no errors on second run.

Integration tests live in `tools/ndp-gold-ddl/tests/integration/` and are gated behind `#[cfg(feature = "integration")]` or a `--ignored` flag.

---

## 5. Scope Boundaries

### 5.1 IN Scope

| Item | Detail |
|------|--------|
| `events.rs::generate_detection_procedure()` | Full refactor to read all values from config (FR-001 through FR-004) |
| `state_transitions.rs` device type and direction | Remove hardcoded patterns (FR-005, FR-006) |
| `aligned_view.rs::determine_stream_type()` | Read from config instead of string matching (FR-007) |
| `ndp_id` entity column | Extract to constant (FR-008) |
| `gold` schema name | Extract to constant (FR-009) |
| `join_builder.rs` forecast column | Make configurable (FR-010) |
| New config fields | `stream_type` on StreamConfig, `device_types` on TransitionConfig, `alignment_column` on forecast config |
| Test infrastructure | Fictional domain test fixtures, hardcoding detection tests |
| JSON Schema updates | Update JSON Schema for any new config fields |
| Golden file tests | Capture current output for backward compatibility |

### 5.2 OUT of Scope

| Item | Rationale |
|------|-----------|
| New domain configs | We are making the generators generic, not adding new domains. New domains are a separate feature. |
| etcd config loading | Config source abstraction is ops-001 / V1.3 concern. We use `ConfigLoader` trait (already abstract). |
| V1.2 Pattern Detection Engine | Events infrastructure is V1.1; pattern detection uses it but is V1.2 scope. |
| Changes to Silver layer | Silver table names, columns, etc. are not changing -- we are reading them from config. |
| CI/CD pipeline changes | No build pipeline changes needed. |
| `ndp-validate` changes | Validation crate may need updated test fixtures but no logic changes. |
| Deploy.sh changes | Deployment script uses ndp-gold-ddl as a binary; no changes needed. |

### 5.3 Deferred

| Item | Defer To | Reason |
|------|----------|--------|
| `observation_time` default timestamp | Future cleanup | Low impact; already configurable via `silver_etl.timestamp.target_field` |
| Making `GOLD_SCHEMA` runtime-configurable | V2.0 multi-tenant | Current scope is single constant; runtime config adds complexity |
| Context enrichment field selection | V1.2 | Currently includes all aligned view columns; V1.2 may want selective inclusion |
| `EventsConfig` per-stream overrides | V1.2 | Currently one EventsConfig per domain; may need per-stream in future |

---

## 6. Risk Assessment

### Risk 1: Detection Procedure SQL Complexity

| Attribute | Value |
|-----------|-------|
| **Risk** | The detection procedure is ~170 lines of SQL generated from a single Rust method. Refactoring to be config-driven while maintaining exact SQL semantics is error-prone. |
| **Likelihood** | High |
| **Impact** | High -- broken detection = no events on production Pi |
| **Mitigation** | Golden file tests capture current output. Integration tests verify procedure executes against real TimescaleDB. Refactor incrementally: state transitions first, then threshold crossings, then context. Each step has its own passing test suite before the next step begins. |

### Risk 2: Config Schema Changes Break Existing Deployments

| Attribute | Value |
|-----------|-------|
| **Risk** | Adding new fields to StreamConfig (e.g., `stream_type`) could break JSON Schema validation or require config file updates on production Pi. |
| **Likelihood** | Medium |
| **Impact** | Medium -- deployment blocked until config updated |
| **Mitigation** | All new fields are optional with sensible defaults. `stream_type` defaults to `Observation`. `device_types` defaults to `None` (omit column). No existing config file needs modification for the refactor to work. |

### Risk 3: Dynamic CTE Generation for N Objectives

| Attribute | Value |
|-----------|-------|
| **Risk** | Current code generates exactly 2 hardcoded CTEs (co2, pm25). Refactoring to N objectives from config means generating a variable number of CTEs and a UNION ALL. Edge cases: 0 objectives, 1 objective, 10+ objectives. |
| **Likelihood** | Medium |
| **Impact** | Medium -- incorrect SQL generation |
| **Mitigation** | Test with 0, 1, 2, 6 (current indoor-air-quality count), and 10 objectives. Verify UNION ALL correctness. The `generate_detection_procedure` method will iterate `DomainConfig.objectives` and produce one CTE per objective. |

### Risk 4: Backward Compatibility of Generated SQL

| Attribute | Value |
|-----------|-------|
| **Risk** | Even semantically equivalent SQL changes could cause issues if downstream tooling expects exact SQL text. |
| **Likelihood** | Low |
| **Impact** | Low -- no downstream tooling parses the SQL text |
| **Mitigation** | Golden file tests verify output stability. The `indoor-air-quality` domain should produce functionally identical SQL (same tables, columns, thresholds, logic). Minor whitespace/formatting differences are acceptable. |

### Risk 5: Test Maintenance Burden

| Attribute | Value |
|-----------|-------|
| **Risk** | Adding fictional domain test fixtures and hardcoding detection tests creates more test code to maintain. |
| **Likelihood** | Low |
| **Impact** | Low -- tests are stable once written |
| **Mitigation** | Fictional domain fixtures are self-contained test helpers. FORBIDDEN_LITERALS list is a compile-time constant. Low ongoing maintenance. |

---

## 7. Success Metrics

### 7.1 Quantitative

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Hardcoded domain literals in generators | 0 | FORBIDDEN_LITERALS test passes for fictional domain |
| Existing tests passing | 339+ (ndp-gold-ddl) | `cargo test -p ndp-gold-ddl` |
| New test count | 25+ new tests | `cargo test -p ndp-gold-ddl -- --list \| wc -l` delta |
| Raw `"ndp_id"` in generators | 0 (excluding constant def) | Grep-based test |
| Raw `"gold"` schema strings in generators | 0 (excluding constant def) | Grep-based test |
| Backward compatibility | Identical SQL for indoor-air-quality domain | Golden file diff |
| Fictional domain generates without error | 1 domain (energy-monitoring) | Test assertion |

### 7.2 Qualitative

| Metric | Target |
|--------|--------|
| A new developer can add a domain by writing JSON config only | No Rust code changes required |
| Error messages point to specific config files | Actionable error messages with file paths |
| Code review confirms no domain-specific logic in generators | PR review checklist item |

### 7.3 Definition of Done

OPS-002 is complete when ALL of the following are true:

1. All 10 functional requirements (FR-001 through FR-010) have passing tests.
2. FORBIDDEN_LITERALS test passes for the `energy-monitoring` fictional domain.
3. Golden file test passes for the `indoor-air-quality` domain (backward compatibility).
4. All 339+ existing ndp-gold-ddl tests still pass.
5. Integration tests pass against TimescaleDB (detection procedure executes successfully).
6. PR review confirms zero hardcoded domain-specific values in generator code.
7. JSON Schema updated for any new config fields.

---

## 8. Implementation Phases

The refactoring should be done incrementally, each phase independently testable and deployable:

### Phase 1: Constants and StreamConfig.stream_type (FR-008, FR-009, FR-010, FR-007)

- Extract `DEFAULT_ENTITY_COLUMN` and `GOLD_SCHEMA` constants
- Add `stream_type` field to `StreamConfig` (optional, defaults to Observation)
- Refactor `determine_stream_type()` to read from config
- Add `alignment_column` support to forecast join
- Estimated scope: ~4 files, ~50 lines changed

### Phase 2: State Transitions Cleanup (FR-005, FR-006)

- Add `device_types` to `TransitionConfig`
- Refactor `generate_device_type_case()` to read from config
- Refactor `generate_default_direction_case()` to use config states
- Estimated scope: ~2 files, ~80 lines changed

### Phase 3: Detection Procedure -- State Transitions Section (FR-001, FR-002)

- Modify `EventsGenerator` to accept `ConfigLoader` dependency
- Refactor state transitions CTE to read Silver table, columns, stream_id from config
- Estimated scope: ~2 files, ~100 lines changed

### Phase 4: Detection Procedure -- Threshold Crossings Section (FR-003)

- Refactor threshold crossings to iterate `DomainConfig.objectives`
- Generate one CTE per objective with config-derived values
- UNION ALL the CTEs
- Estimated scope: ~1 file, ~150 lines changed

### Phase 5: Context Enrichment (FR-004) and Hardcoding Detection Tests

- Refactor context `jsonb_build_object()` to be dynamic
- Write FORBIDDEN_LITERALS test suite
- Write fictional domain (energy-monitoring) regression guard
- Golden file capture for backward compatibility
- Estimated scope: ~3 files, ~200 lines of test code

---

## 9. Glossary

| Term | Definition |
|------|-----------|
| **DomainConfig** | JSON config defining a cross-stream domain (e.g., `indoor-air-quality`). Lives in `config/domains/{id}/domain.json`. |
| **StreamConfig** | JSON config defining a single data stream. Lives in `config/base/streams/{id}/config.json`. |
| **ObjectiveConfig** | A threshold-based target within a domain (e.g., "CO2 < 800 ppm"). Embedded in DomainConfig. |
| **TransitionConfig** | Configuration for state transition extraction. Embedded in StreamConfig under `gold_etl.features.transitions`. |
| **Gold CA** | Gold Continuous Aggregate -- a TimescaleDB materialized view that aggregates Silver data. |
| **Detection Procedure** | A PL/pgSQL stored procedure that scans Silver/Gold data for events (state transitions, threshold crossings). |
| **FORBIDDEN_LITERALS** | A compile-time list of domain-specific strings that must not appear in generator output for non-matching domains. |
| **Golden File** | A captured reference copy of generated SQL used for backward compatibility testing. |
| **London TDD** | Test-Driven Development style where collaborators are mocked and behavior is tested through interfaces. |

---

## 10. Appendix: Hardcoded Value Inventory

Complete inventory of every hardcoded domain-specific value found in the audit:

### events.rs (30+ values)

| Line(s) | Hardcoded Value | Should Come From |
|---------|----------------|------------------|
| 475 | `'home-assistant-state'` | `DomainConfig.streams[].stream_id` |
| 481 | `silver.state_events` | `StreamConfig.silver_etl.target_table` |
| 478 | `s.ndp_id` | `TransitionConfig.entity_field` |
| 478 | `s.state` | `TransitionConfig.state_field` |
| 475,480 | `s.event_time` | `StreamConfig.silver_etl.timestamp.target_field` |
| 530 | `'air-quality'::TEXT` | `DomainConfig.streams[].stream_id` |
| 531 | `co2_mean` | `ObjectiveConfig.target.metric` + `_mean` |
| 533 | `pm25_mean` | `ObjectiveConfig.target.metric` + `_mean` |
| 535 | `gold.air_quality_hourly` | Derived from stream's Gold CA table name |
| 545 | `800.0` | `ObjectiveConfig.target.threshold` |
| 547-549 | `800` (4 occurrences) | `ObjectiveConfig.target.threshold` |
| 552 | `'healthy_co2'` | `ObjectiveConfig.id` |
| 569 | `12.0` | `ObjectiveConfig.target.threshold` |
| 571-573 | `12` (4 occurrences) | `ObjectiveConfig.target.threshold` |
| 576 | `'healthy_pm25'` | `ObjectiveConfig.id` |
| 538 | `'co2'` metric name | `ObjectiveConfig.target.metric` |
| 568 | `'pm25'` metric name | `ObjectiveConfig.target.metric` |
| 621 | `'ppm'`, `'ug/m3'` | `ObjectiveConfig.target.unit` |
| 505-510 | `indoor_co2_mean`, etc. (6 context fields) | Derived from aligned view columns |
| 611-616 | Same 6 context fields (duplicated) | Derived from aligned view columns |

### state_transitions.rs (8 values)

| Line(s) | Hardcoded Value | Should Come From |
|---------|----------------|------------------|
| 67 | `"ndp_id"` | Config or `DEFAULT_ENTITY_COLUMN` constant |
| 296 | `'off'` | `TransitionsConfig.states` or `direction_mapping` |
| 296 | `'on'` | `TransitionsConfig.states` or `direction_mapping` |
| 296 | `'opening'` | `direction_mapping` |
| 297 | `'closing'` | `direction_mapping` |
| 309 | `'door_%'` | `TransitionConfig.device_types` |
| 310 | `'window_%'` | `TransitionConfig.device_types` |
| 311-312 | `'motion_%'`, `'light_%'` | `TransitionConfig.device_types` |

### aligned_view.rs (4 values)

| Line(s) | Hardcoded Value | Should Come From |
|---------|----------------|------------------|
| 122 | `contains("forecast")` | `StreamConfig.stream_type` |
| 124 | `contains("state")`, `contains("event")` | `StreamConfig.stream_type` |
| 126 | `contains("dimension")`, `contains("ref")` | `StreamConfig.stream_type` |

### join_builder.rs (1 value)

| Line(s) | Hardcoded Value | Should Come From |
|---------|----------------|------------------|
| 123 | `f.issued_at` | `StreamConfig.forecast_alignment_column` or default |

### continuous_aggregate.rs (1 value)

| Line(s) | Hardcoded Value | Should Come From |
|---------|----------------|------------------|
| 46 | `"ndp_id"` | `DEFAULT_ENTITY_COLUMN` constant |

### Cross-cutting: schema name "gold" (~15 occurrences)

Hardcoded in `events.rs`, `continuous_aggregate.rs`, `state_transitions.rs`, `aligned_view.rs`, `planner/sync.rs`, `refresh_policy.rs`. Should reference `GOLD_SCHEMA` constant.
