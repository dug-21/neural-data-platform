# OPS-002 Architecture: Eliminate Hardcoded References from Gold Layer Generators

> **Feature:** OPS-002
> **Status:** Proposed
> **Author:** ndp-architect
> **Date:** 2026-02-06

---

## Problem Summary

The `EventsGenerator.generate_detection_procedure()` method in `tools/ndp-gold-ddl/src/generators/events.rs` contains 50+ hardcoded domain-specific values that should be driven by configuration. This breaks the platform's config-driven design principle and caused production failures in v1.1.10 when the hardcoded table name `silver.home_assistant_state` did not match the actual table `silver.state_events`.

### Hardcoded References Inventory

| Line Range | Hardcoded Value | Config Source |
|-----------|----------------|---------------|
| 475 | `'home-assistant-state'` stream literal | `domain_config.streams[].stream_id` (actuator role) |
| 472-482 | `silver.state_events` table, `ndp_id`, `state`, `event_time` columns | `stream_config.silver_etl.target_table`, field_mappings |
| 530 | `'air-quality'::TEXT` stream literal | `domain_config.streams[].stream_id` (primary role) |
| 531-534 | `co2_mean`, `pm25_mean` column names | Derived from `gold_etl.aggregates.fields` keys + `_mean` suffix |
| 535 | `gold.air_quality_hourly` table name | Derived: `gold.{stream_id_snake}_{granularity_suffix}` |
| 544-552 | CO2 threshold 800, metric `'co2'`, objective `'healthy_co2'` | `domain_config.objectives[0]` |
| 568-576 | PM2.5 threshold 12, metric `'pm25'`, objective `'healthy_pm25'` | `domain_config.objectives[1]` |
| 505-510 | Context columns: `indoor_co2_mean`, `indoor_pm25_mean`, etc. | Derived from aligned view column catalog |
| 611-616 | Context columns repeated in threshold crossing INSERT | Same derivation as above |
| 621 | Unit derivation: `'ppm'` / `'ug/m3'` | `objective.target.unit` |

---

## ADR-OPS002-001: Config Resolution Strategy

### Status
Proposed

### Context

The `EventsGenerator` currently receives only a `DomainConfig` and internally hardcodes Silver/Gold table references. Other generators in the codebase demonstrate two patterns:

**Pattern A: Generator takes stream config directly.** `ContinuousAggregateGenerator.from_stream_config()` and `StateTransitionGenerator.from_stream_config()` read `silver_etl.target_table` from the config passed in. They never resolve config themselves.

**Pattern B: Generator takes config loader.** `AlignedViewGenerator<L: ConfigLoader>` stores a `ConfigLoader` and calls `load_stream_config()` during generation to look up each stream referenced in the domain.

The events generator needs data from *both* the `DomainConfig` (objectives, stream references, alignment view name) and multiple `StreamConfig` instances (Silver table names, Gold column names, timestamp fields). This is the same requirement as `AlignedViewGenerator`.

### Decision

**Follow Pattern B: `EventsGenerator` receives a `ConfigLoader` reference along with the `DomainConfig`.**

The generator signature becomes:

```rust
pub struct EventsGenerator<L: ConfigLoader> {
    domain_id: String,
    config: EventsConfig,
    config_loader: L,
}

impl<L: ConfigLoader> EventsGenerator<L> {
    pub fn new(domain_config: &DomainConfig, config_loader: L) -> Self { ... }

    pub fn generate(&self, domain_config: &DomainConfig, action: Action) -> Result<String> { ... }
}
```

The `generate_detection_procedure()` method:
1. Receives the full `DomainConfig` (objectives, streams, alignment).
2. Uses `self.config_loader.load_stream_config(stream_id)` for each stream referenced in the domain to resolve Silver table names, Gold column names, and timestamp fields.
3. Uses `domain_config.objectives` to generate threshold crossing SQL per objective.

### Consequences

**Easier:**
- Adding new domains requires zero Rust code changes.
- Adding new objectives to an existing domain automatically generates threshold crossing detection.
- The generator is testable with a `MockConfigLoader` (already exists at `tools/ndp-gold-ddl/src/config/mock_loader.rs`).

**Harder:**
- The `EventsGenerator` struct gains a type parameter `L: ConfigLoader`, changing its signature. Callers must supply a loader.
- Existing trait `IEventsGenerator` methods must be updated to pass `DomainConfig` and `ConfigLoader`.

### Alternatives Considered

**A. Pre-resolve all config into a "resolved" struct.** Build a `ResolvedEventsContext` that flattens all needed data before calling the generator. Rejected because it duplicates the work already done by `ConfigLoader` and creates a parallel resolution path that must be kept in sync. The `AlignedViewGenerator` already proves that the `ConfigLoader` pattern works cleanly.

**B. Add Silver/Gold table names to DomainConfig directly.** Extend `StreamRef` with `silver_table` and `gold_table` fields. Rejected because this duplicates data already present in stream configs (`silver_etl.target_table`) and would create two sources of truth for table names.

---

## ADR-OPS002-002: Dynamic SQL Generation from Objectives

### Status
Proposed

### Context

The detection procedure currently has two hardcoded CTEs: `co2_crossings` and `pm25_crossings`. Each duplicates the same SQL pattern with different metric names, thresholds, and objective IDs. The domain config already declares six objectives (CO2, PM2.5, humidity min/max, temperature min/max), but only two are implemented.

The pattern for each threshold crossing CTE is identical:

```sql
{metric}_crossings AS (
    SELECT
        bucket AS event_time,
        stream_id,
        entity_id,
        'threshold_crossing' AS event_type,
        '{metric}' AS metric,
        {threshold} AS threshold_value,
        CASE
            WHEN {metric}_prev {inverse_condition} {threshold} AND {metric}_value {condition} {threshold} THEN '{direction_1}'
            WHEN {metric}_prev {condition} {threshold} AND {metric}_value {inverse_condition} {threshold} THEN '{direction_2}'
        END AS crossing_direction,
        {metric}_value AS metric_value,
        {metric}_prev AS previous_metric_value,
        '{objective_id}' AS objective_id
    FROM hourly_obs
    WHERE ...
)
```

Every field in this template is available from `ObjectiveConfig`:
- `metric` from `objective.target.metric`
- `threshold` from `objective.target.threshold`
- `condition` from `objective.target.condition`
- `objective_id` from `objective.id`
- `stream` from `objective.target.stream` (to determine which Gold CA to query)
- `unit` from `objective.target.unit`

### Decision

**Generate one CTE per objective, iterating `domain_config.objectives`.**

The implementation:

1. **Group objectives by source stream.** Objectives targeting the same stream share a single `hourly_obs` CTE (avoiding duplicate table scans).

2. **For each stream group, generate a `hourly_obs_{stream_snake}` CTE** that selects the relevant metric columns and their LAG values from the stream's Gold hourly CA.

3. **For each objective, generate a `{objective_id}_crossings` CTE** using the template above, parameterized by the objective's target config.

4. **Union all crossing CTEs** into `all_crossings` and INSERT into `gold.events`.

The crossing direction is derived from the condition:
- `condition = "<"` means "rising" when value crosses above threshold, "falling" when below.
- `condition = ">="` means "falling" when value drops below threshold, "rising" when above.
- For complementary pairs (e.g., `comfortable_humidity_min` with `>=` 40 and `comfortable_humidity_max` with `<=` 60), each generates its own CTE independently.

```
Condition  | Rising trigger                    | Falling trigger
-----------|-----------------------------------|----------------------------------
"<"        | prev < threshold AND val >= threshold  | prev >= threshold AND val < threshold
"<="       | prev <= threshold AND val > threshold  | prev > threshold AND val <= threshold
">"        | prev > threshold AND val <= threshold   | prev <= threshold AND val > threshold
">="       | prev >= threshold AND val < threshold   | prev < threshold AND val >= threshold
```

### Consequences

**Easier:**
- Adding a new objective (e.g., `healthy_tvoc: tvoc_index < 200`) requires only a JSON config change.
- All six current objectives generate detection SQL automatically.
- The per-objective CTE structure is self-documenting in the generated SQL.

**Harder:**
- The generated SQL grows linearly with objective count. For the current six objectives, this produces six CTEs plus stream-grouped observation CTEs. This is acceptable given the procedure runs every 15 minutes on a Pi.
- Metric-to-column mapping must be resolved: objective `metric: "co2"` maps to Gold column `co2_mean`. The mapping convention is `{metric}_mean` from the stream's `gold_etl.aggregates.fields` config. The generator validates that the metric exists in the stream's aggregate config.

### Column Name Resolution

The objective target declares `metric: "co2"`. The Gold CA column is `co2_mean`. The mapping rule:

1. Look up `stream_config.gold_etl.aggregates.fields[metric]`.
2. Verify `"mean"` is in the field's metrics list (required for threshold comparison).
3. The Gold column name is `{metric}_mean`.

If the metric is not found in the stream's aggregate config, the generator returns an error. This prevents silent failures where an objective references a non-existent metric.

---

## ADR-OPS002-003: Stream-to-Table Mapping Convention

### Status
Proposed

### Context

The detection procedure needs to resolve three types of table/view references:

1. **Silver table** for state transition queries (e.g., `silver.state_events`)
2. **Gold CA** for threshold crossing queries (e.g., `gold.air_quality_hourly`)
3. **Gold aligned view** for context enrichment (e.g., `gold.indoor_air_quality_aligned`)

Currently, the Silver table name is hardcoded. The Gold CA name is hardcoded. The aligned view name IS already config-driven (via `domain_id_snake`).

### Decision

**All table references are resolved from existing config fields. No new config fields are needed.**

| Reference | Resolution Source | Example |
|-----------|------------------|---------|
| Silver table | `stream_config.silver_etl.target_table` | `"silver.state_events"` |
| Gold CA | `gold.{stream_id_snake}_{granularity_suffix(alignment.granularity)}` | `"gold.air_quality_hourly"` |
| Gold aligned view | `gold.{alignment.view_name}` | `"gold.indoor_air_quality_aligned"` |
| Timestamp column | `stream_config.silver_etl.timestamp.target_field` | `"event_time"` |
| Entity column | `stream_config.silver_etl.identity_fields[0].target` | `"ndp_id"` |

The Gold CA naming convention already exists in:
- `ContinuousAggregateGenerator.generate_for_granularity()` (line 96): `gold.{stream_id_snake}_{suffix}`
- `AlignedViewGenerator.derive_gold_table_name()` (line 134): same pattern

**Extract this as a shared function** in a new `naming.rs` module to avoid duplication:

```rust
// tools/ndp-gold-ddl/src/naming.rs

/// Derive the Gold CA table name for a stream at a given granularity.
///
/// Convention: gold.{stream_id_with_underscores}_{granularity_suffix}
/// Example: stream_id="air-quality", granularity="1 hour" -> "gold.air_quality_hourly"
pub fn derive_gold_ca_name(stream_id: &str, granularity: &str) -> String {
    let normalized_id = stream_id.replace('-', "_");
    let suffix = granularity_to_suffix(granularity);
    format!("gold.{}_{}", normalized_id, suffix)
}
```

### State Transition Stream Resolution

The detection procedure needs to find "the stream with role=actuator" (for state transitions). The resolution path:

1. Find `stream_ref` in `domain_config.streams` where `role == StreamRole::Actuator`.
2. Load its `StreamConfig` via `config_loader.load_stream_config(stream_ref.stream_id)`.
3. Read `stream_config.silver_etl.target_table` for the Silver table name.
4. Read `stream_config.silver_etl.timestamp.target_field` for the timestamp column.
5. Read `stream_config.silver_etl.identity_fields[0].target` for the entity column.
6. Read `stream_config.silver_etl.field_mappings` for the state column name.

This replaces the hardcoded `silver.state_events`, `event_time`, `ndp_id`, and `state` references.

### Consequences

**Easier:**
- No new config fields to maintain. Everything is already declared.
- The naming function is shared across generators, preventing drift.
- Adding a new stream type automatically gets correct table name resolution.

**Harder:**
- `silver_etl.identity_fields` is currently used only by the Silver ETL. The events generator now depends on it. This coupling is acceptable because the identity field IS the correct source of truth for "which column identifies the entity."
- The `field_mappings` array must be searched to find the state column. This is a linear scan over a small array (2-7 entries).

---

## ADR-OPS002-004: Hardcoding Prevention Architecture

### Status
Proposed

### Context

After fixing the current hardcoded references, we need to prevent future regressions. Domain-specific literals in generator code should be caught before they reach production.

### Decision

**Implement a three-layer prevention strategy: compile-time, test-time, and review-time.**

#### Layer 1: Domain Literal Detection Tests

Create a test module `tests/hardcoding_guard.rs` that:

1. Generates SQL for the `indoor-air-quality` domain using real config files.
2. Scans the generated SQL for domain-specific literals that should come from config.
3. Fails if any hardcoded domain literal is found in the Rust source (not the generated SQL).

```rust
/// Test that generator source code contains no domain-specific literals.
///
/// This test reads the Rust source files in generators/ and asserts
/// that none of them contain string literals matching known domain values.
#[test]
fn test_no_domain_literals_in_generator_source() {
    let forbidden_literals = [
        "air-quality",         // stream IDs belong in config
        "home-assistant",      // stream IDs belong in config
        "state_events",        // Silver table names belong in config
        "air_quality_hourly",  // Gold CA names are derived
        "co2_mean",            // Column names are derived from config
        "pm25_mean",           // Column names are derived from config
        "indoor_co2",          // Aligned view columns are derived
        "800",                 // Thresholds belong in objectives
        "healthy_co2",         // Objective IDs belong in config
    ];

    let source_files = glob("src/generators/*.rs");
    for file in source_files {
        let content = read_to_string(file);
        for literal in &forbidden_literals {
            assert!(
                !content.contains(literal),
                "Found domain literal '{}' in {}. Use config instead.",
                literal, file
            );
        }
    }
}
```

This test is intentionally aggressive. When a developer adds a new generator method, the test forces them to parameterize domain values from config rather than hardcoding.

#### Layer 2: Config Completeness Validation

The generator validates that all required config fields exist before generating SQL. Missing fields produce clear error messages:

```
Error: MissingRequiredField { field: "silver_etl.target_table", context: "stream 'home-assistant-state'" }
```

This already exists in `StateTransitionGenerator` and `ContinuousAggregateGenerator`. The refactored `EventsGenerator` follows the same pattern.

#### Layer 3: Generated SQL Smoke Test

An integration test generates SQL for the real config and executes it against the integration TimescaleDB (via `docker-compose.integration.yml`). This catches runtime mismatches between generated SQL and actual database schema.

### Consequences

**Easier:**
- Domain literal detection catches regressions at `cargo test` time.
- Clear error messages tell developers exactly which config field to add.
- Integration tests catch runtime issues before deployment.

**Harder:**
- The forbidden literals list must be maintained. When a new domain is added, its values should be added to the list. However, the test can be made more generic by reading the config files and extracting domain-specific values automatically.
- Integration tests require Docker to be running (only in CI/CD or explicit local testing).

---

## Component Diagram

### Current Flow (Hardcoded)

```
DomainConfig ──> EventsGenerator ──> SQL
                      │
                      ├── hardcoded "silver.state_events"
                      ├── hardcoded "gold.air_quality_hourly"
                      ├── hardcoded "co2_mean", "pm25_mean"
                      ├── hardcoded thresholds 800, 12
                      ├── hardcoded stream IDs
                      └── hardcoded context columns
```

### Refactored Flow (Config-Driven)

```
                   ┌─────────────────────────┐
                   │      ConfigLoader        │
                   │  (FileSystem or Mock)     │
                   └──────────┬──────────────┘
                              │
              load_stream_config(stream_id)
                              │
                   ┌──────────▼──────────────┐
                   │     StreamConfig(s)       │
                   │  ┌────────────────────┐  │
                   │  │ silver_etl:        │  │
                   │  │   target_table ────┼──┼──> Silver table name
                   │  │   timestamp ───────┼──┼──> Timestamp column
                   │  │   identity_fields ─┼──┼──> Entity column
                   │  │   field_mappings ──┼──┼──> State field name
                   │  ├────────────────────┤  │
                   │  │ gold_etl:          │  │
                   │  │   aggregates ──────┼──┼──> Metric column names
                   │  └────────────────────┘  │
                   └──────────────────────────┘

┌──────────────────┐        ┌─────────────────────────────────┐
│   DomainConfig   │        │   EventsGenerator<L: ConfigLoader> │
│                  │        │                                 │
│  objectives[] ───┼───────>│   generate_detection_procedure()│
│    .id ──────────┼────┐   │     │                           │
│    .target:      │    │   │     ├── State Transitions CTE   │
│      .stream ────┼──┐ │   │     │   stream: actuator role   │
│      .metric ────┼──┤ │   │     │   table: from StreamConfig│
│      .threshold ─┼──┤ │   │     │   columns: from config    │
│      .condition ─┼──┤ │   │     │                           │
│      .unit ──────┼──┤ │   │     ├── Per-Objective CTEs      │
│                  │  │ │   │     │   loop objectives[]       │
│  streams[] ──────┼──┤ │   │     │   threshold from config   │
│    .stream_id ───┼──┤ │   │     │   metric from config      │
│    .role ────────┼──┤ │   │     │   Gold CA from naming fn  │
│                  │  │ │   │     │                           │
│  alignment:      │  │ │   │     ├── Context Enrichment      │
│    .view_name ───┼──┘ │   │     │   columns from aligned    │
│    .granularity ─┼────┘   │     │   view column catalog     │
│                  │        │     │                           │
└──────────────────┘        │     └── UNION ALL + INSERT      │
                            └─────────────────────────────────┘
                                          │
                                          ▼
                                    Generated SQL
                                  (no domain literals)
```

### Data Flow for Detection Procedure Generation

```
Step 1: Resolve actuator stream
  domain_config.streams.find(role == Actuator)
  -> stream_ref { stream_id: "home-assistant-state", ... }
  -> config_loader.load_stream_config("home-assistant-state")
  -> StreamConfig { silver_etl: { target_table: "silver.state_events", ... } }

Step 2: Generate state transitions CTE
  silver_table:      stream_config.silver_etl.target_table
  timestamp_col:     stream_config.silver_etl.timestamp.target_field
  entity_col:        stream_config.silver_etl.identity_fields[0].target
  state_col:         find_state_field(stream_config.silver_etl.field_mappings)
  stream_id_literal: stream_ref.stream_id

Step 3: Group objectives by source stream
  objectives.group_by(|o| o.target.stream)
  -> { "air-quality": [healthy_co2, healthy_pm25, ...] }

Step 4: For each stream group, resolve Gold CA
  config_loader.load_stream_config("air-quality")
  gold_ca_name = derive_gold_ca_name("air-quality", domain_config.alignment.granularity)
  -> "gold.air_quality_hourly"

Step 5: Generate hourly_obs CTE per stream group
  SELECT bucket, ndp_id, {metrics from objectives}_mean, LAG(...)
  FROM {gold_ca_name}

Step 6: Generate crossing CTE per objective
  For each objective in group:
    threshold = objective.target.threshold
    metric_col = format!("{}_mean", objective.target.metric)
    condition = objective.target.condition
    unit = objective.target.unit

Step 7: Generate context enrichment
  aligned_view = format!("gold.{}", domain_config.alignment.view_name)
  context_columns = derive from aligned view column catalog (via config_loader)

Step 8: UNION ALL crossing CTEs + INSERT with context
```

---

## Interface Changes

### 1. EventsGenerator Struct (Breaking Change)

**Before:**
```rust
pub struct EventsGenerator {
    domain_id: String,
    config: EventsConfig,
}

impl EventsGenerator {
    pub fn from_domain_config(domain: &DomainConfig) -> Self;
    pub fn new(domain_id: &str, config: EventsConfig) -> Self;
    pub fn generate(&self, action: Action) -> Result<String>;
}
```

**After:**
```rust
pub struct EventsGenerator<L: ConfigLoader> {
    domain_id: String,
    config: EventsConfig,
    config_loader: L,
}

impl<L: ConfigLoader> EventsGenerator<L> {
    pub fn from_domain_config(domain: &DomainConfig, config_loader: L) -> Self;
    pub fn new(domain_id: &str, config: EventsConfig, config_loader: L) -> Self;
    pub fn generate(&self, domain_config: &DomainConfig, action: Action) -> Result<String>;
}
```

Note: `generate()` now takes `&DomainConfig` because it needs access to `objectives` and `streams` during SQL generation. Previously these were not needed because everything was hardcoded.

### 2. IEventsGenerator Trait (Breaking Change)

**Before:**
```rust
pub trait IEventsGenerator {
    fn generate_events_hypertable(&self, domain: &DomainConfig) -> Result<String>;
    fn generate_unified_view(&self) -> Result<String>;
    fn generate_hourly_aggregate(&self) -> Result<String>;
    fn generate_detection_procedure(&self, domain: &DomainConfig) -> Result<String>;
    fn generate_detection_job(&self, schedule: &str) -> Result<String>;
}
```

**After:**
```rust
pub trait IEventsGenerator {
    fn generate_events_hypertable(&self, domain: &DomainConfig) -> Result<String>;
    fn generate_unified_view(&self) -> Result<String>;
    fn generate_hourly_aggregate(&self) -> Result<String>;
    fn generate_detection_procedure(&self, domain: &DomainConfig) -> Result<String>;
    fn generate_detection_job(&self, schedule: &str) -> Result<String>;
}
```

The trait signature for `generate_detection_procedure` already takes `&DomainConfig`. The trait itself does not change. Only the concrete implementation changes internally to use the config loader.

### 3. New Shared Module: naming.rs

```rust
// tools/ndp-gold-ddl/src/naming.rs

use crate::validation::granularity_to_suffix;

/// Derive the Gold continuous aggregate name for a stream at a given granularity.
pub fn derive_gold_ca_name(stream_id: &str, granularity: &str) -> String {
    let normalized_id = stream_id.replace('-', "_");
    let suffix = granularity_to_suffix(granularity);
    format!("gold.{}_{}", normalized_id, suffix)
}

/// Derive the Gold aligned view name (already in DomainConfig, but validated here).
pub fn derive_gold_aligned_view_name(view_name: &str) -> String {
    format!("gold.{}", view_name)
}
```

### 4. Config Struct Additions (Minimal)

**No new config structs are required.** All needed data already exists:

| Needed Data | Already In |
|-------------|-----------|
| Silver table name | `StreamConfig.silver_etl.target_table` |
| Timestamp column | `StreamConfig.silver_etl.timestamp.target_field` |
| Entity column | `StreamConfig.silver_etl.identity_fields[0].target` |
| State field | `StreamConfig.silver_etl.field_mappings[].target_column` (where type = "text" for state) |
| Metric columns | `StreamConfig.gold_etl.aggregates.fields` keys |
| Thresholds | `DomainConfig.objectives[].target.threshold` |
| Conditions | `DomainConfig.objectives[].target.condition` |
| Units | `DomainConfig.objectives[].target.unit` |
| Objective IDs | `DomainConfig.objectives[].id` |
| Aligned view columns | Derivable from stream configs via `AlignedViewGenerator.derive_gold_columns()` |

**One optional addition to `StreamConfig` for explicit state field identification:**

The `gold_etl.transitions` section (in `home-assistant-state/config.json`) already has `state_field: "state"`. This can be read to identify the state column unambiguously, rather than searching `field_mappings`. This field already exists:

```json
"gold_etl": {
  "transitions": {
    "enabled": true,
    "state_field": "state",
    "entity_field": "ndp_id"
  }
}
```

### 5. Context Column Resolution

The context enrichment block in the detection procedure builds a JSONB object from aligned view columns. Currently hardcoded as:

```sql
jsonb_build_object(
    'indoor_co2', a.indoor_co2_mean,
    'indoor_pm25', a.indoor_pm25_mean,
    'indoor_temp', a.indoor_temperature_c_mean,
    'outdoor_temp', a.outdoor_temperature_c_mean,
    'outdoor_pm25', a.outdoor_aqi_pm25_mean,
    'window_state', a.state_state_last
)
```

**Resolution:** Derive context columns by iterating `domain_config.streams` and building the JSONB object from each stream's Gold columns with their alias prefix. The `AlignedViewGenerator` already computes these columns via `derive_gold_columns()`. The events generator reuses this logic:

For each stream in the domain:
1. Load stream config.
2. Get Gold columns from `gold_etl.aggregates.fields`.
3. Prefix with `{alias}_` (matching the aligned view column naming convention).
4. Build JSONB key-value pairs: key = `"{alias}_{metric}"`, value = `a.{alias}_{metric}_{aggregate}`.

Select a representative subset (e.g., the first metric from each stream with `_mean` aggregate) to keep the context JSONB manageable. Alternatively, include ALL columns -- the JSONB context is for correlation analysis and more data is better.

**Decision:** Include all columns from the aligned view. The JSONB context exists for V1.2 pattern detection and completeness outweighs compactness. The aligned view already selects only the configured columns, so this is inherently bounded by config.

---

## Test Architecture (London TDD)

### Mock Boundaries

| Component | Mocked? | Justification |
|-----------|---------|---------------|
| `ConfigLoader` | YES (mock) | Isolates generator logic from filesystem. `MockConfigLoader` already exists. |
| `DomainConfig` | NO (real struct) | In-memory struct, no side effects. Tests construct it directly. |
| `StreamConfig` | NO (real struct) | In-memory struct, no side effects. Tests construct it directly. |
| `TimescaleDB` | YES (mock/absent) | Unit tests validate SQL string output, not DB execution. |
| `naming.rs` functions | NO (real) | Pure functions with no dependencies. Test directly. |
| File system | YES (via mock loader) | Unit tests never touch the filesystem. |

### Test Hierarchy

```
Unit Tests (cargo test)
├── naming_tests.rs
│   ├── test_derive_gold_ca_name_standard
│   ├── test_derive_gold_ca_name_with_hyphens
│   ├── test_derive_gold_ca_name_daily
│   └── test_derive_gold_ca_name_custom_granularity
│
├── events_tests.rs (refactored from existing)
│   ├── TDD Cycle: Config resolution
│   │   ├── test_generator_requires_config_loader
│   │   ├── test_generator_loads_stream_configs_from_domain
│   │   └── test_generator_errors_on_missing_stream_config
│   │
│   ├── TDD Cycle: State transitions from config
│   │   ├── test_state_transitions_use_silver_table_from_config
│   │   ├── test_state_transitions_use_timestamp_from_config
│   │   ├── test_state_transitions_use_entity_field_from_config
│   │   ├── test_state_transitions_use_state_field_from_config
│   │   └── test_state_transitions_use_stream_id_from_config
│   │
│   ├── TDD Cycle: Threshold crossings from objectives
│   │   ├── test_generates_one_cte_per_objective
│   │   ├── test_threshold_from_objective_config
│   │   ├── test_metric_column_derived_from_gold_etl
│   │   ├── test_condition_determines_crossing_direction
│   │   ├── test_objective_id_from_config
│   │   ├── test_unit_from_objective_config
│   │   ├── test_objectives_grouped_by_stream
│   │   └── test_errors_on_metric_not_in_aggregates
│   │
│   ├── TDD Cycle: Context enrichment from config
│   │   ├── test_context_columns_from_aligned_view
│   │   ├── test_context_uses_stream_alias_prefix
│   │   └── test_context_handles_multiple_streams
│   │
│   └── TDD Cycle: No hardcoded literals
│       ├── test_no_hardcoded_silver_table
│       ├── test_no_hardcoded_gold_ca
│       ├── test_no_hardcoded_thresholds
│       └── test_no_hardcoded_stream_ids
│
├── hardcoding_guard_tests.rs (NEW)
│   ├── test_no_domain_literals_in_events_generator_source
│   ├── test_no_domain_literals_in_state_transitions_source
│   └── test_no_domain_literals_in_detection_procedure_source
│
Integration Tests (cargo test --features integration)
├── test_generated_sql_executes_against_timescaledb
├── test_detection_procedure_creates_successfully
└── test_detection_procedure_detects_threshold_crossings
```

### Key Test Patterns

**Pattern: Mock ConfigLoader returns deterministic configs**

```rust
fn mock_loader_with_air_quality() -> MockConfigLoader {
    let mut loader = MockConfigLoader::new();
    loader.add_stream_config("air-quality", StreamConfig {
        stream_id: "air-quality".to_string(),
        silver_etl: Some(SilverEtlConfig {
            target_table: "silver.air_quality_observations".to_string(),
            timestamp: Some(TimestampConfig {
                target_field: "observation_time".to_string(),
            }),
            ..Default::default()
        }),
        gold_etl: Some(GoldEtlConfig {
            enabled: true,
            aggregates: Some(AggregatesConfig {
                fields: hashmap! {
                    "co2" => FieldMetricsConfig { metrics: vec!["mean".into()] },
                    "pm25" => FieldMetricsConfig { metrics: vec!["mean".into()] },
                },
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    });
    loader
}
```

**Pattern: Assert config-driven values appear in SQL**

```rust
#[test]
fn test_threshold_from_objective_config() {
    let domain = create_domain_with_objective("test_metric", "<", 42.5);
    let loader = mock_loader_with_stream("test-stream", "silver.test_table");
    let generator = EventsGenerator::from_domain_config(&domain, loader);

    let sql = generator.generate(&domain, Action::Recreate).unwrap();

    assert!(sql.contains("42.5"), "Threshold should come from objective config");
    assert!(sql.contains("test_metric"), "Metric should come from objective config");
    assert!(!sql.contains("800"), "Should not contain hardcoded threshold");
}
```

### Integration Environment

Integration tests use `docker-compose.integration.yml` which provides:
- TimescaleDB on port 5432 with `DEPLOY_ENV=integration`
- Tests create the Gold schema, generate DDL, execute it, and verify objects exist.
- Tests are gated behind `#[cfg(feature = "integration")]` to avoid requiring Docker for unit tests.

---

## Migration Strategy

### Phase 1: Extract Shared Naming (Non-Breaking)

1. Create `tools/ndp-gold-ddl/src/naming.rs` with `derive_gold_ca_name()`.
2. Update `ContinuousAggregateGenerator` and `AlignedViewGenerator` to use the shared function.
3. All existing tests pass unchanged.
4. Commit.

### Phase 2: Refactor EventsGenerator Signature (Breaking Internal)

1. Add `L: ConfigLoader` type parameter to `EventsGenerator`.
2. Update `from_domain_config()` to accept `config_loader: L`.
3. Update `generate()` to accept `&DomainConfig`.
4. Update `IEventsGenerator` trait impl.
5. Update all callers (CLI, integration points).
6. All existing tests updated to use `MockConfigLoader`.
7. Commit.

### Phase 3: Replace Hardcoded State Transitions

1. Refactor `generate_detection_procedure()` state transitions CTE to resolve:
   - Silver table from config
   - Timestamp, entity, state columns from config
   - Stream ID literal from config
2. Write new tests that assert config-driven values.
3. Run existing tests to verify identical SQL output for current config.
4. Commit.

### Phase 4: Replace Hardcoded Threshold Crossings

1. Refactor threshold crossing CTEs to iterate `domain_config.objectives`.
2. Generate per-objective CTEs with config-driven thresholds, metrics, conditions.
3. Group objectives by stream for shared `hourly_obs` CTEs.
4. Write new tests for multi-objective generation.
5. Commit.

### Phase 5: Replace Hardcoded Context Enrichment

1. Derive context JSONB columns from aligned view column catalog.
2. Replace hardcoded `indoor_co2`, `indoor_pm25`, etc. with derived columns.
3. Write tests asserting context columns match domain streams.
4. Commit.

### Phase 6: Add Hardcoding Guard Tests

1. Create `hardcoding_guard_tests.rs`.
2. Add forbidden literal detection for generator source files.
3. Verify all guards pass.
4. Commit.

### Backwards Compatibility

| Concern | Guarantee |
|---------|-----------|
| Generated SQL output | For the `indoor-air-quality` domain with current config, the generated SQL is functionally equivalent. Column order in JSONB may differ. |
| Config files | Zero changes to any JSON config file. |
| CLI interface | `ndp-gold-ddl events --domain indoor-air-quality --action sync` works identically. The `--config-dir` flag is already supported. |
| Test suite | All 339 existing ndp-gold-ddl tests pass after refactoring (some with updated mock setup). |

### Rollback Plan

Each phase is a separate commit. To rollback:
1. `git revert` the specific phase commit.
2. The previous phase's code compiles and passes all tests independently.

The phased approach means a failure in Phase 4 (threshold crossings) does not affect Phase 3 (state transitions), allowing partial delivery.

---

## File Change Map

### New Files

| File | Purpose |
|------|---------|
| `tools/ndp-gold-ddl/src/naming.rs` | Shared naming conventions: `derive_gold_ca_name()`, `derive_gold_aligned_view_name()` |
| `tools/ndp-gold-ddl/tests/hardcoding_guard.rs` | Source-level detection of domain literals in generator code |

### Modified Files

| File | Changes |
|------|---------|
| `tools/ndp-gold-ddl/src/lib.rs` | Add `pub mod naming;` |
| `tools/ndp-gold-ddl/src/generators/events.rs` | **Major refactor.** Add `L: ConfigLoader` type parameter. Refactor `generate_detection_procedure()` to resolve all references from config. Refactor `generate()` to accept `&DomainConfig`. Update all test methods to use `MockConfigLoader`. |
| `tools/ndp-gold-ddl/src/generators/mod.rs` | Update `EventsGenerator` export to include type parameter. |
| `tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs` | Replace inline `gold.{}_{}` format with `naming::derive_gold_ca_name()`. ~3 lines changed. |
| `tools/ndp-gold-ddl/src/generators/aligned_view.rs` | Replace `derive_gold_table_name()` with `naming::derive_gold_ca_name()`. ~5 lines changed. |
| `tools/ndp-gold-ddl/src/generators/state_transitions.rs` | No changes required. Already config-driven. (Validates our target pattern.) |
| `tools/ndp-gold-ddl/src/config/loader.rs` | No changes. `ConfigLoader` trait already sufficient. |
| `tools/ndp-gold-ddl/src/config/types.rs` | No changes. All needed fields already exist. |
| `tools/ndp-gold-ddl/src/config/domain.rs` | No changes. `DomainConfig`, `ObjectiveConfig`, `TargetConfig` already have all needed fields. |
| CLI entrypoint (e.g., `src/main.rs` or `src/cli.rs`) | Update `EventsGenerator` construction to pass `ConfigLoader`. ~5 lines changed. |

### Unchanged Files (Verified)

| File | Why Unchanged |
|------|--------------|
| `config/domains/indoor-air-quality/domain.json` | Zero config changes. Config already has all needed data. |
| `config/base/streams/air-quality/config.json` | Zero config changes. |
| `config/base/streams/home-assistant-state/config.json` | Zero config changes. |
| `tools/ndp-gold-ddl/src/config/mock_loader.rs` | Already supports mock stream and domain configs. May need minor additions for new test scenarios. |

### Estimated Scope

| Metric | Estimate |
|--------|----------|
| New files | 2 |
| Modified files | 6-7 |
| Lines added | ~300 (new naming module, new tests, refactored detection procedure) |
| Lines removed | ~120 (hardcoded SQL blocks replaced with config-driven generation) |
| Lines modified | ~80 (test setup, constructor calls) |
| Net change | +~200 lines |
| Test count change | +15-20 new tests, ~30 existing tests updated |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Generated SQL semantically different after refactor | Medium | High | Golden file test: capture current SQL output, diff against refactored output. |
| Performance regression from config loading in procedure generation | Low | Low | Config loading is file I/O done once at generation time, not at query time. The procedure itself is static SQL. |
| Missing config field causes generation failure | Medium | Medium | Phase 2 adds validation with clear error messages. Unit tests cover all paths. |
| Type parameter `L: ConfigLoader` makes the API more complex | Low | Low | Same pattern already used by `AlignedViewGenerator<L>`. Developers are familiar with it. |
| New objectives (humidity, temperature) generate more SQL than expected | Low | Low | Review generated SQL in Phase 4 tests. The procedure runs every 15 minutes; more CTEs are acceptable. |
