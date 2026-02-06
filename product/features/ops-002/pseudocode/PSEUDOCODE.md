# OPS-002 Pseudocode: Eliminate Hardcoded References from Gold Layer Generators

> **Feature:** ops-002
> **Phase:** SPARC Pseudocode
> **Date:** 2026-02-06

---

## 1. Data Flow Diagrams

### 1.1 Current (BEFORE) Data Flow

```
EventsGenerator::generate_detection_procedure()
    |
    v
domain_id --> snake_case("indoor_air_quality")
    |
    v
HARDCODED SQL TEMPLATE
    |-- "silver.state_events"          (should come from StreamConfig)
    |-- "s.ndp_id"                     (should come from TransitionConfig)
    |-- "s.state"                      (should come from TransitionConfig)
    |-- "'home-assistant-state'"       (should come from DomainConfig.streams)
    |-- "gold.air_quality_hourly"      (should come from StreamConfig)
    |-- "co2_mean", "pm25_mean"        (should come from gold_etl.aggregates)
    |-- 800.0, 12.0 thresholds        (should come from DomainConfig.objectives)
    |-- "'healthy_co2'", "'healthy_pm25'"  (should come from objectives[].id)
    |-- "'ppm'", "'ug/m3'" units       (should come from objectives[].target.unit)
    |-- 6 context column names          (should derive from aligned view columns)
    |
    v
  SQL string output (only works for air-quality domain)
```

### 1.2 Target (AFTER) Data Flow

```
EventsGenerator::generate_detection_procedure(domain_config, config_loader)
    |
    +---> domain_config.streams
    |       |-- Find actuator stream --> stream_id, silver_etl.target_table
    |       |-- Find primary stream  --> stream_id, gold table name
    |
    +---> config_loader.load_stream_config(actuator.stream_id)
    |       |-- silver_etl.target_table  --> "silver.state_events"
    |       |-- silver_etl.timestamp.target_field --> "event_time"
    |       |-- gold_etl.transitions.state_field  --> "state"
    |       |-- gold_etl.transitions.entity_field --> "ndp_id"
    |
    +---> config_loader.load_stream_config(primary.stream_id)
    |       |-- silver_etl.target_table  --> derive Gold CA table
    |       |-- gold_etl.aggregates.fields --> column names for hourly_obs
    |
    +---> domain_config.objectives[]
    |       |-- For each objective:
    |       |     target.metric    --> "co2", "pm25"
    |       |     target.threshold --> 800.0, 12.0
    |       |     target.condition --> "<"
    |       |     target.unit      --> "ppm", "ug/m3"
    |       |     id               --> "healthy_co2"
    |       |     target.stream    --> resolve to Gold CA table
    |
    +---> domain_config.alignment.view_name --> aligned view for context
    |
    +---> Derive context columns from aligned_streams[] column lists
    |
    v
  SQL string output (works for ANY domain)
```

### 1.3 StateTransitionGenerator Data Flow (AFTER)

```
StateTransitionGenerator::generate(transition_config, action)
    |
    +---> transition_config.direction_mapping  (if present)
    |       |-- Use custom state value pairs
    |       |-- No fallback to hardcoded 'on'/'off'
    |
    +---> transition_config.states[] (NEW: from gold_etl.transitions.states)
    |       |-- If no direction_mapping, build default from states[0]/states[1]
    |
    +---> device_type_mapping config (NEW field on TransitionConfig or StreamConfig)
    |       |-- Map entity_id LIKE patterns to device types
    |
    v
  SQL string output (no hardcoded on/off or door_%/window_%)
```

### 1.4 AlignedViewGenerator Data Flow (AFTER)

```
AlignedViewGenerator::determine_stream_type(stream_id)
    |
    v  BEFORE: string matching on stream_id
    |
    v  AFTER:
config_loader.load_stream_config(stream_id)
    |-- stream_config.stream_type --> "observation" | "state_event" | "forecast" | "dimension"
    |
    v
  StreamType enum (parsed directly from config)
```

---

## 2. Algorithm Design

### 2.1 EventsGenerator Refactoring

#### 2.1.1 New Struct Signature

```
STRUCT EventsGenerator:
    domain_id: String
    config: EventsConfig
    config_loader: Box<dyn ConfigLoader>   // NEW: needed to load stream configs
```

The generator needs access to a ConfigLoader because `generate_detection_procedure`
must resolve stream_id references in the domain config to actual Silver table names
and Gold column names. The DomainConfig alone is insufficient.

#### 2.1.2 Constructor Changes

```
ALGORITHM: EventsGenerator::from_domain_config
INPUT: domain (DomainConfig), config_loader (impl ConfigLoader)
OUTPUT: EventsGenerator

BEGIN
    config <- domain.events OR EventsConfig::new()
    RETURN EventsGenerator {
        domain_id: domain.id,
        config: config,
        config_loader: config_loader,
    }
END
```

#### 2.1.3 Core Algorithm: generate_detection_procedure

```
ALGORITHM: generate_detection_procedure
INPUT: self (EventsGenerator), domain_config (DomainConfig)
OUTPUT: Result<String>

-- We need:
-- (a) The actuator stream's Silver table + fields for state transitions
-- (b) Per-objective: Gold CA table + metric column for threshold crossings
-- (c) The aligned view name + columns for context enrichment

BEGIN
    domain_id_snake <- self.domain_id.replace('-', '_')

    -- ============================================================
    -- PHASE 1: Resolve State Transition Sources
    -- ============================================================
    state_stream_ref <- find_stream_by_role(domain_config.streams, Actuator)
    IF state_stream_ref IS NONE THEN
        -- Domain has no actuator stream: skip state transition section
        state_section_sql <- ""
    ELSE
        state_stream_config <- self.config_loader
            .load_stream_config(state_stream_ref.stream_id)

        -- Read Silver table from stream config
        state_silver_table <- state_stream_config.silver_etl.target_table
            // e.g., "silver.state_events"

        -- Read timestamp field from stream config
        state_timestamp <- state_stream_config.silver_etl.timestamp.target_field
            // e.g., "event_time"

        -- Read entity and state fields from gold_etl.transitions
        transition_cfg <- resolve_transition_config(state_stream_config)
        entity_field <- transition_cfg.entity_field    // e.g., "ndp_id"
        state_field  <- transition_cfg.state_field     // e.g., "state"
        stream_id_literal <- state_stream_ref.stream_id  // e.g., "home-assistant-state"

        state_section_sql <- build_state_transitions_sql(
            state_silver_table,
            state_timestamp,
            entity_field,
            state_field,
            stream_id_literal,
            domain_id_snake,
            domain_config.alignment.view_name,
        )
    END IF

    -- ============================================================
    -- PHASE 2: Resolve Threshold Crossing Sources (per objective)
    -- ============================================================
    -- Group objectives by target.stream to minimize CTEs
    objectives_by_stream <- group_by(domain_config.objectives, |o| o.target.stream)

    crossing_cte_parts <- []
    all_crossing_cte_names <- []

    FOR EACH (stream_id, objectives) IN objectives_by_stream DO
        stream_config <- self.config_loader.load_stream_config(stream_id)

        -- Derive Gold CA table name
        gold_ca_table <- derive_gold_ca_table(stream_config, "1 hour")
            // e.g., "gold.air_quality_hourly"

        -- Find the stream_ref to get the stream_id literal
        stream_ref <- find_stream_ref(domain_config.streams, stream_id)
        stream_id_literal <- stream_ref.stream_id  // e.g., "air-quality"

        -- Determine the entity column for this Gold CA
        entity_column <- NDP_ENTITY_COLUMN  // "ndp_id" (constant)

        -- Build hourly_obs CTE for this stream (one CTE per stream)
        -- Columns needed: bucket, entity, and each metric referenced by objectives
        needed_metrics <- UNIQUE(objectives.map(|o| o.target.metric))
        metric_columns <- []
        FOR EACH metric IN needed_metrics DO
            col_name <- format!("{}_mean", metric)
            metric_columns.push((metric, col_name))
        END FOR

        hourly_obs_cte <- build_hourly_obs_cte(
            gold_ca_table,
            entity_column,
            stream_id_literal,
            metric_columns,
        )

        -- Build per-objective crossing CTEs
        FOR EACH objective IN objectives DO
            col_name <- format!("{}_mean", objective.target.metric)
            crossing_cte <- build_crossing_cte(
                objective.id,             // e.g., "healthy_co2"
                objective.target.metric,  // e.g., "co2"
                col_name,                 // e.g., "co2_mean"
                objective.target.threshold,  // e.g., 800.0
                objective.target.condition,  // e.g., "<"
                objective.target.unit,       // e.g., "ppm"
            )
            crossing_cte_parts.push(crossing_cte)
            all_crossing_cte_names.push(format!("{}_crossings", objective.id))
        END FOR
    END FOR

    -- UNION ALL crossing CTEs
    crossings_union_sql <- build_crossings_union(all_crossing_cte_names)

    -- ============================================================
    -- PHASE 3: Build Context Enrichment
    -- ============================================================
    aligned_view_name <- format!("gold.{}", domain_config.alignment.view_name)
        // e.g., "gold.indoor_air_quality_aligned"

    context_columns <- derive_context_columns(domain_config, self.config_loader)
        // Returns list of (json_key, column_name) pairs

    context_jsonb_sql <- build_context_jsonb(context_columns, aligned_view_name)

    -- ============================================================
    -- PHASE 4: Assemble Final Procedure
    -- ============================================================
    RETURN format_detection_procedure(
        state_section_sql,
        hourly_obs_cte,
        crossing_cte_parts,
        crossings_union_sql,
        context_jsonb_sql,
        aligned_view_name,
    )
END
```

#### 2.1.4 Subroutine: find_stream_by_role

```
ALGORITHM: find_stream_by_role
INPUT: streams (Vec<StreamRef>), role (StreamRole)
OUTPUT: Option<StreamRef>

BEGIN
    FOR EACH stream IN streams DO
        IF stream.role == role THEN
            RETURN Some(stream)
        END IF
    END FOR
    RETURN None
END
```

#### 2.1.5 Subroutine: resolve_transition_config

```
ALGORITHM: resolve_transition_config
INPUT: stream_config (StreamConfig)
OUTPUT: Result<TransitionResolvedConfig>

BEGIN
    -- First check gold_etl.transitions (full stream config on disk)
    IF stream_config.gold_etl.transitions IS SOME THEN
        t <- stream_config.gold_etl.transitions
        RETURN {
            entity_field: t.entity_field OR "ndp_id",
            state_field: t.state_field,
            states: t.states,
        }
    END IF

    -- Fallback: check gold_etl.features.transitions (ndp-gold-ddl simplified config)
    IF stream_config.gold_etl.features.transitions IS SOME THEN
        t <- stream_config.gold_etl.features.transitions
        RETURN {
            entity_field: "ndp_id",
            state_field: t.field,
            states: t.states,
        }
    END IF

    RETURN Error("No transition config found for stream")
END
```

**Note on config shape mismatch:** The `config/base/streams/home-assistant-state/config.json`
has `gold_etl.transitions` at the top level, while `tools/ndp-gold-ddl/src/config/types.rs`
models it as `gold_etl.features.transitions`. The resolver handles both shapes.

#### 2.1.6 Subroutine: derive_gold_ca_table

```
ALGORITHM: derive_gold_ca_table
INPUT: stream_config (StreamConfig), granularity (String)
OUTPUT: String

-- Convention: gold.{stream_id_snake}_{suffix}
-- This mirrors AlignedViewGenerator::derive_gold_table_name

BEGIN
    normalized_id <- stream_config.stream_id.replace('-', '_')
    suffix <- granularity_to_suffix(granularity)
        // "1 hour" -> "hourly", "1 day" -> "daily"
    RETURN format!("gold.{}_{}", normalized_id, suffix)
        // e.g., "gold.air_quality_hourly"
END
```

#### 2.1.7 Subroutine: build_state_transitions_sql

```
ALGORITHM: build_state_transitions_sql
INPUT:
    silver_table (String),      // "silver.state_events"
    timestamp_field (String),   // "event_time"
    entity_field (String),      // "ndp_id"
    state_field (String),       // "state"
    stream_id_literal (String), // "home-assistant-state"
    domain_id_snake (String),   // "indoor_air_quality"
    aligned_view_name (String), // "indoor_air_quality_aligned"
OUTPUT: String (SQL fragment)

BEGIN
    context_sql <- build_state_context_enrichment(domain_id_snake, aligned_view_name)

    RETURN format!("""
    WITH new_transitions AS (
        SELECT
            s.{timestamp_field} AS event_time,
            '{stream_id_literal}' AS stream_id,
            s.{entity_field} AS entity_id,
            'state_transition' AS event_type,
            LAG(s.{state_field}) OVER (
                PARTITION BY s.{entity_field}
                ORDER BY s.{timestamp_field}
            ) AS from_state,
            s.{state_field} AS to_state,
            EXTRACT(EPOCH FROM (
                s.{timestamp_field} - LAG(s.{timestamp_field}) OVER (
                    PARTITION BY s.{entity_field}
                    ORDER BY s.{timestamp_field}
                )
            )) * 1000 AS duration_ms
        FROM {silver_table} s
        WHERE s.{timestamp_field} > last_run
    ),
    actual_transitions AS (
        SELECT * FROM new_transitions
        WHERE from_state IS NOT NULL
          AND from_state IS DISTINCT FROM to_state
    )
    INSERT INTO gold.events (...)
    SELECT
        t.event_time, t.stream_id, t.entity_id, t.event_type,
        t.from_state, t.to_state, t.duration_ms::BIGINT,
        {context_sql},
        '{{}}'::JSONB
    FROM actual_transitions t
    ON CONFLICT DO NOTHING;
    """)
END
```

#### 2.1.8 Subroutine: build_crossing_cte

```
ALGORITHM: build_crossing_cte
INPUT:
    objective_id (String),   // "healthy_co2"
    metric_name (String),    // "co2"
    col_name (String),       // "co2_mean" (from Gold CA)
    threshold (f64),         // 800.0
    condition (String),      // "<"
    unit (Option<String>),   // Some("ppm")
OUTPUT: String (SQL CTE fragment)

-- For a "<" condition: crossing rises when prev < threshold AND curr >= threshold
-- For a ">=" condition: crossing rises when prev >= threshold AND curr < threshold
-- We generate both rising and falling crossings for each objective.

BEGIN
    -- Determine rising/falling logic from condition
    (rising_test, falling_test) <- resolve_crossing_tests(threshold, condition)
        // For "<":
        //   rising:  "{col}_prev < {threshold} AND {col}_value >= {threshold}"
        //   falling: "{col}_prev >= {threshold} AND {col}_value < {threshold}"
        // For ">=":
        //   rising:  "{col}_prev < {threshold} AND {col}_value >= {threshold}"
        //   falling: "{col}_prev >= {threshold} AND {col}_value < {threshold}"
        // Note: rising/falling always means crossing the threshold boundary,
        //       direction label depends on whether the condition is "healthy above"
        //       or "healthy below"

    cte_name <- format!("{}_crossings", objective_id)

    RETURN format!("""
    {cte_name} AS (
        SELECT
            bucket AS event_time,
            stream_id,
            entity_id,
            'threshold_crossing' AS event_type,
            '{metric_name}' AS metric,
            {threshold} AS threshold_value,
            CASE
                WHEN {rising_test} THEN 'rising'
                WHEN {falling_test} THEN 'falling'
            END AS crossing_direction,
            {col_name}_value AS metric_value,
            {col_name}_prev AS previous_metric_value,
            '{objective_id}' AS objective_id
        FROM hourly_obs
        WHERE bucket > last_run
          AND {col_name}_prev IS NOT NULL
          AND {col_name}_value IS NOT NULL
          AND ({rising_test} OR {falling_test})
    )
    """)
END
```

#### 2.1.9 Subroutine: resolve_crossing_tests

```
ALGORITHM: resolve_crossing_tests
INPUT: threshold (f64), condition (String)
OUTPUT: (rising_sql, falling_sql)

-- A threshold crossing happens when the metric value crosses the threshold
-- boundary between consecutive observations, regardless of which direction
-- is "healthy". We always emit both rising and falling crossings.

BEGIN
    col_prev <- "{col}_prev"
    col_value <- "{col}_value"

    -- Rising: value crosses upward through threshold
    rising_sql  <- format!("{col_prev} < {threshold} AND {col_value} >= {threshold}")

    -- Falling: value crosses downward through threshold
    falling_sql <- format!("{col_prev} >= {threshold} AND {col_value} < {threshold}")

    RETURN (rising_sql, falling_sql)
END
```

#### 2.1.10 Subroutine: derive_context_columns

```
ALGORITHM: derive_context_columns
INPUT: domain_config (DomainConfig), config_loader (impl ConfigLoader)
OUTPUT: Vec<(String, String)>  -- (json_key, sql_column_expression)

-- Build context enrichment from the aligned view.
-- Include a representative subset of columns: the first N numeric columns
-- from each stream, plus any state columns.

BEGIN
    context_cols <- []

    FOR EACH stream_ref IN domain_config.streams DO
        stream_config <- config_loader.load_stream_config(stream_ref.stream_id)

        IF stream_config.gold_etl.aggregates IS SOME THEN
            -- Use aggregate column naming: {alias}_{field}_{metric}
            aggregates <- stream_config.gold_etl.aggregates
            sorted_fields <- aggregates.fields.keys().sorted()

            FOR EACH field_name IN sorted_fields DO
                field_cfg <- aggregates.fields[field_name]
                -- Use first metric (typically "mean") for context snapshot
                IF field_cfg.metrics contains "mean" THEN
                    json_key <- format!("{}_{}", stream_ref.alias, field_name)
                    sql_col  <- format!("a.{}_{}_mean", stream_ref.alias, field_name)
                    context_cols.push((json_key, sql_col))
                END IF
            END FOR
        END IF

        IF stream_config.gold_etl.transitions IS SOME THEN
            -- Include last-known state for actuator streams
            t <- stream_config.gold_etl.transitions
            json_key <- format!("{}_state", stream_ref.alias)
            sql_col  <- format!("a.{}_{}_last", stream_ref.alias, t.state_field)
            context_cols.push((json_key, sql_col))
        END IF
    END FOR

    RETURN context_cols
END
```

#### 2.1.11 Subroutine: build_context_jsonb

```
ALGORITHM: build_context_jsonb
INPUT:
    context_columns (Vec<(String, String)>),
    aligned_view_name (String)  -- e.g., "gold.indoor_air_quality_aligned"
OUTPUT: String (SQL expression)

BEGIN
    IF context_columns IS EMPTY THEN
        RETURN "'{}'::JSONB"
    END IF

    pairs <- []
    FOR EACH (json_key, sql_col) IN context_columns DO
        pairs.push(format!("'{}', {}", json_key, sql_col))
    END FOR

    RETURN format!("""
        COALESCE(
            (SELECT jsonb_build_object({pairs_joined})
             FROM {aligned_view_name} a
             WHERE a.bucket = time_bucket('1 hour', t.event_time)),
            '{{}}'::JSONB
        )
    """, pairs_joined = pairs.join(", "))
END
```

#### 2.1.12 Subroutine: build_hourly_obs_cte

```
ALGORITHM: build_hourly_obs_cte
INPUT:
    gold_ca_table (String),        // "gold.air_quality_hourly"
    entity_column (String),        // "ndp_id"
    stream_id_literal (String),    // "air-quality"
    metric_columns (Vec<(String, String)>)  // [("co2", "co2_mean"), ("pm25", "pm25_mean")]
OUTPUT: String (SQL CTE)

BEGIN
    select_cols <- []
    select_cols.push("bucket")
    select_cols.push(format!("{} AS entity_id", entity_column))
    select_cols.push(format!("'{}'::TEXT AS stream_id", stream_id_literal))

    FOR EACH (metric, col_name) IN metric_columns DO
        select_cols.push(format!("{} AS {}_value", col_name, metric))
        select_cols.push(format!(
            "LAG({}) OVER (PARTITION BY {} ORDER BY bucket) AS {}_prev",
            col_name, entity_column, metric
        ))
    END FOR

    RETURN format!("""
    hourly_obs AS (
        SELECT
            {select_cols_joined}
        FROM {gold_ca_table}
        WHERE bucket > last_run - INTERVAL '1 hour'
    )
    """)
END
```

**Complexity Analysis - generate_detection_procedure:**

```
Time Complexity:
    - find_stream_by_role: O(s) where s = number of streams
    - load_stream_config: O(1) per call (file I/O, not algorithmically bound)
    - group objectives: O(n) where n = number of objectives
    - build CTEs: O(n) per objective
    - build context: O(s * f) where f = avg fields per stream
    - Total: O(n + s*f) -- dominated by context column derivation

Space Complexity:
    - Intermediate CTEs: O(n) for n objectives
    - Context columns: O(s * f)
    - Output SQL string: O(n * k) where k = avg CTE size
    - Total: O(n * k)
```

---

### 2.2 StateTransitionGenerator Refactoring

#### 2.2.1 Replace generate_default_direction_case

The current implementation hardcodes `'off'`/`'on'` -> `'opening'`/`'closing'`.

```
ALGORITHM: generate_default_direction_case (REFACTORED)
INPUT:
    state_field (String),
    states (Vec<String>)   // From TransitionConfig.states, e.g., ["on", "off"]
OUTPUT: String (SQL CASE expression)

BEGIN
    IF states.len() >= 2 THEN
        -- Use first two states as the binary pair
        state_a <- states[0]  // e.g., "on"
        state_b <- states[1]  // e.g., "off"

        RETURN format!("""
        CASE
            WHEN LAG({state_field}) OVER w = '{state_b}' AND {state_field} = '{state_a}'
                THEN 'activating'
            WHEN LAG({state_field}) OVER w = '{state_a}' AND {state_field} = '{state_b}'
                THEN 'deactivating'
            WHEN LAG({state_field}) OVER w IS NULL THEN 'initial'
            ELSE 'unknown'
        END AS transition_direction
        """)
    ELSE
        -- Non-binary states: just label as "changed"
        RETURN format!("""
        CASE
            WHEN LAG({state_field}) OVER w IS NULL THEN 'initial'
            WHEN LAG({state_field}) OVER w IS DISTINCT FROM {state_field} THEN 'changed'
            ELSE 'unchanged'
        END AS transition_direction
        """)
    END IF
END
```

**Change from current behavior:**
- BEFORE: Always uses `'off' -> 'on' = 'opening'` and `'on' -> 'off' = 'closing'`
- AFTER: Uses `states[0]`/`states[1]` from TransitionConfig. Labels become generic
  (`activating`/`deactivating`) unless a `direction_mapping` is supplied.

#### 2.2.2 Replace generate_device_type_case

The current implementation hardcodes `door_%`, `window_%`, `motion_%`, `light_%`.

```
ALGORITHM: generate_device_type_case (REFACTORED)
INPUT:
    entity_field (String),
    device_type_mapping (Option<Vec<DeviceTypeRule>>)
        // DeviceTypeRule: { pattern: String, device_type: String }
        // e.g., [{ pattern: "door_%", device_type: "door" },
        //        { pattern: "window_%", device_type: "window" }]
OUTPUT: String (SQL CASE expression)

BEGIN
    IF device_type_mapping IS SOME AND NOT EMPTY THEN
        cases <- []
        FOR EACH rule IN device_type_mapping DO
            cases.push(format!(
                "WHEN {} LIKE '{}' THEN '{}'",
                entity_field, rule.pattern, rule.device_type
            ))
        END FOR
        cases.push("ELSE 'other'")

        RETURN format!("CASE\n    {}\nEND AS device_type", cases.join("\n    "))
    ELSE
        -- No device type mapping: omit the column entirely
        RETURN ""
    END IF
END
```

**Config schema extension needed (minimal):**

```json
// In config/base/streams/home-assistant-state/config.json
// gold_etl.transitions (existing section, add new field):
{
  "transitions": {
    "enabled": true,
    "state_field": "state",
    "entity_field": "ndp_id",
    "states": ["on", "off"],
    "track_duration": true,
    "include_in_alignment": true,
    "device_type_mapping": [
      { "pattern": "door_%", "device_type": "door" },
      { "pattern": "window_%", "device_type": "window" },
      { "pattern": "motion_%", "device_type": "motion" },
      { "pattern": "light_%", "device_type": "light" }
    ]
  }
}
```

#### 2.2.3 Signature Changes

```
STRUCT TransitionConfig (UPDATED):
    enabled: bool
    state_field: String
    entity_field: String
    track_duration: bool
    include_in_alignment: bool
    direction_mapping: Option<HashMap<String, String>>  // existing
    states: Vec<String>                                  // ALREADY EXISTS in types.rs
    device_type_mapping: Option<Vec<DeviceTypeRule>>     // NEW

STRUCT DeviceTypeRule:                                   // NEW
    pattern: String      // SQL LIKE pattern, e.g., "door_%"
    device_type: String  // label, e.g., "door"
```

**Complexity Analysis - StateTransitionGenerator changes:**

```
Time Complexity:
    - generate_default_direction_case: O(k) where k = states.len() (trivially small)
    - generate_device_type_case: O(m) where m = device_type_mapping.len()
    - No change to overall O(1) generation

Space Complexity:
    - O(m) for device type rules
    - O(1) for direction case
```

---

### 2.3 AlignedViewGenerator: Stream Type Fix

#### 2.3.1 Replace String-Matching determine_stream_type

```
ALGORITHM: determine_stream_type (REFACTORED)
INPUT: stream_id (String)
OUTPUT: StreamType

-- BEFORE: String matching on stream_id ("forecast", "state", "event", "dimension")
-- AFTER: Read from loaded StreamConfig

BEGIN
    stream_config <- self.config_loader.load_stream_config(stream_id)

    -- StreamConfig from disk has "stream_type" field
    -- The ndp-gold-ddl StreamConfig type needs this field added
    IF stream_config.stream_type IS SOME THEN
        RETURN parse_stream_type(stream_config.stream_type)
    ELSE
        -- Fallback: preserve current string matching for backward compat
        RETURN infer_stream_type_from_id(stream_id)
    END IF
END
```

**Config schema extension needed:**

The `StreamConfig` struct in `tools/ndp-gold-ddl/src/config/types.rs` must add:

```rust
pub struct StreamConfig {
    pub stream_id: String,
    pub stream_type: Option<StreamType>,   // NEW - reads "stream_type" from JSON
    pub fields: Vec<FieldConfig>,
    pub silver_etl: Option<SilverEtlConfig>,
    pub gold_etl: Option<GoldEtlConfig>,
}
```

This field already exists in the on-disk config files (e.g., `"stream_type": "observation"`
in air-quality config, `"stream_type": "state_event"` in home-assistant-state config).
The ndp-gold-ddl `StreamConfig` struct simply doesn't read it yet.

---

## 3. Config Schema Extensions

### 3.1 Summary of Changes

| Change | Type | Location |
|--------|------|----------|
| `StreamConfig.stream_type` | Add field | `tools/ndp-gold-ddl/src/config/types.rs` |
| `DeviceTypeRule` struct | New struct | `tools/ndp-gold-ddl/src/generators/state_transitions.rs` |
| `TransitionConfig.device_type_mapping` | Add field | `tools/ndp-gold-ddl/src/generators/state_transitions.rs` |
| `device_type_mapping` in JSON | Add section | `config/base/streams/home-assistant-state/config.json` |

### 3.2 Rust Type Additions

```rust
// In types.rs - StreamConfig gets stream_type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,

    #[serde(default)]
    pub stream_type: Option<StreamType>,  // NEW

    #[serde(default)]
    pub fields: Vec<FieldConfig>,

    #[serde(default)]
    pub silver_etl: Option<SilverEtlConfig>,

    #[serde(default)]
    pub gold_etl: Option<GoldEtlConfig>,
}

// In state_transitions.rs - DeviceTypeRule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTypeRule {
    pub pattern: String,
    pub device_type: String,
}

// In state_transitions.rs - TransitionConfig gets device_type_mapping
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransitionConfig {
    // ... existing fields ...

    #[serde(default)]
    pub device_type_mapping: Option<Vec<DeviceTypeRule>>,  // NEW
}
```

### 3.3 What We Do NOT Add

The following items already exist and should be used as-is:

- `ObjectiveConfig.target.stream` -- already maps objective to stream
- `ObjectiveConfig.target.metric` -- already names the metric
- `ObjectiveConfig.target.threshold` -- already has the threshold value
- `ObjectiveConfig.target.condition` -- already has the comparison operator
- `ObjectiveConfig.target.unit` -- already has the unit string
- `StreamConfig.silver_etl.target_table` -- already has the Silver table
- `StreamConfig.gold_etl.aggregates.fields` -- already has metric columns
- `TransitionConfig.states` -- already in `types.rs::TransitionsConfig`

---

## 4. Test Pseudocode (London TDD)

### 4.1 Hardcoding Detection Test (Regression Guard)

This is the most important test. It guarantees that NO future change can
reintroduce air-quality-specific values into the generated SQL.

```
ALGORITHM: test_no_hardcoded_domain_values
PURPOSE: Generate SQL for a FICTIONAL domain and assert zero air-quality strings

BEGIN
    -- Create a completely fictional domain config
    fictional_domain <- DomainConfig {
        id: "factory-noise",
        streams: [
            StreamRef { stream_id: "decibel-monitor", alias: "noise", role: Primary },
            StreamRef { stream_id: "valve-controller", alias: "valve", role: Actuator },
        ],
        alignment: AlignmentConfig {
            view_name: "factory_noise_aligned",
            granularity: "1 hour",
        },
        objectives: [
            ObjectiveConfig {
                id: "safe_noise_level",
                target: TargetConfig {
                    stream: "decibel-monitor",
                    metric: "db_level",
                    condition: "<",
                    threshold: 85.0,
                    unit: Some("dB"),
                },
            },
        ],
        events: Some(EventsConfig { enabled: true, ... }),
    }

    -- Create mock config loader with fictional stream configs
    mock_loader <- MockConfigLoader::new()
        .with_stream("decibel-monitor", StreamConfig {
            stream_id: "decibel-monitor",
            stream_type: Some(StreamType::Observation),
            silver_etl: Some(SilverEtlConfig {
                target_table: "silver.noise_observations",
                timestamp: Some(TimestampConfig { target_field: "measurement_time" }),
            }),
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                aggregates: Some(AggregatesConfig {
                    fields: { "db_level": { metrics: ["mean", "max"] } },
                }),
            }),
        })
        .with_stream("valve-controller", StreamConfig {
            stream_id: "valve-controller",
            stream_type: Some(StreamType::StateEvent),
            silver_etl: Some(SilverEtlConfig {
                target_table: "silver.valve_events",
                timestamp: Some(TimestampConfig { target_field: "event_time" }),
            }),
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                transitions: Some(TransitionsConfig {
                    enabled: true,
                    state_field: "position",
                    entity_field: "valve_id",
                    states: ["open", "closed"],
                }),
            }),
        })

    -- Generate the SQL
    generator <- EventsGenerator::from_domain_config(&fictional_domain, mock_loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    -- ASSERT: No air-quality-specific strings appear
    ASSERT NOT sql.contains("air-quality")
    ASSERT NOT sql.contains("air_quality")
    ASSERT NOT sql.contains("home-assistant")
    ASSERT NOT sql.contains("home_assistant")
    ASSERT NOT sql.contains("co2")
    ASSERT NOT sql.contains("pm25")
    ASSERT NOT sql.contains("indoor_co2")
    ASSERT NOT sql.contains("indoor_pm25")
    ASSERT NOT sql.contains("800")
    ASSERT NOT sql.contains("12.0")
    ASSERT NOT sql.contains("ppm")
    ASSERT NOT sql.contains("ug/m3")
    ASSERT NOT sql.contains("healthy_co2")
    ASSERT NOT sql.contains("healthy_pm25")
    ASSERT NOT sql.contains("state_events")    -- Silver table for home-assistant
    ASSERT NOT sql.contains("ndp_id")          -- After P1 constant extraction

    -- ASSERT: Fictional domain values DO appear
    ASSERT sql.contains("factory_noise")
    ASSERT sql.contains("silver.noise_observations")
    ASSERT sql.contains("silver.valve_events")
    ASSERT sql.contains("db_level")
    ASSERT sql.contains("85")                   -- threshold
    ASSERT sql.contains("safe_noise_level")     -- objective ID
    ASSERT sql.contains("dB")                   -- unit
    ASSERT sql.contains("valve-controller")     -- stream_id literal
    ASSERT sql.contains("decibel-monitor")      -- stream_id literal
    ASSERT sql.contains("position")             -- state_field
    ASSERT sql.contains("valve_id")             -- entity_field
END
```

### 4.2 Mock-Based Unit Tests

#### 4.2.1 Detection Procedure Reads Objectives from Config

```
ALGORITHM: test_detection_procedure_uses_config_objectives
PURPOSE: Verify threshold values come from domain config, not hardcoded

BEGIN
    domain <- create_domain_with_objectives([
        ObjectiveConfig {
            id: "safe_temp",
            target: { stream: "sensors", metric: "temperature", condition: "<", threshold: 42.5, unit: "celsius" },
        },
        ObjectiveConfig {
            id: "min_pressure",
            target: { stream: "sensors", metric: "pressure", condition: ">=", threshold: 1013.0, unit: "hPa" },
        },
    ])

    mock_loader <- create_mock_loader_for(domain)
    generator <- EventsGenerator::from_domain_config(&domain, mock_loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    -- Threshold values from config
    ASSERT sql.contains("42.5")           -- temperature threshold
    ASSERT sql.contains("1013")           -- pressure threshold
    ASSERT sql.contains("'safe_temp'")    -- objective ID
    ASSERT sql.contains("'min_pressure'") -- objective ID
    ASSERT sql.contains("'temperature'")  -- metric name
    ASSERT sql.contains("'pressure'")     -- metric name
    ASSERT sql.contains("'celsius'")      -- unit
    ASSERT sql.contains("'hPa'")          -- unit
END
```

#### 4.2.2 Detection Procedure Reads Silver Table from Stream Config

```
ALGORITHM: test_detection_procedure_uses_silver_table_from_config
PURPOSE: Verify Silver table name comes from stream config

BEGIN
    domain <- create_domain_with_actuator_stream("my-actuator")

    mock_loader <- MockConfigLoader::new()
        .with_stream("my-actuator", StreamConfig {
            silver_etl: Some(SilverEtlConfig {
                target_table: "silver.custom_events_table",
                ...
            }),
            ...
        })

    generator <- EventsGenerator::from_domain_config(&domain, mock_loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    ASSERT sql.contains("silver.custom_events_table")
    ASSERT NOT sql.contains("silver.state_events")
END
```

#### 4.2.3 Detection Procedure Reads Entity/State Fields from Config

```
ALGORITHM: test_detection_procedure_uses_entity_field_from_config
PURPOSE: Verify entity_field and state_field come from transition config

BEGIN
    domain <- create_domain_with_actuator_stream("valve-controller")

    mock_loader <- MockConfigLoader::new()
        .with_stream("valve-controller", StreamConfig {
            gold_etl: Some(GoldEtlConfig {
                transitions: Some({
                    state_field: "position",
                    entity_field: "valve_id",
                    states: ["open", "closed"],
                }),
            }),
            silver_etl: Some(SilverEtlConfig {
                target_table: "silver.valve_events",
                timestamp: Some({ target_field: "recorded_at" }),
            }),
            ...
        })

    generator <- EventsGenerator::from_domain_config(&domain, mock_loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    ASSERT sql.contains("s.valve_id")       -- entity_field from config
    ASSERT sql.contains("s.position")       -- state_field from config
    ASSERT sql.contains("s.recorded_at")    -- timestamp from config
    ASSERT NOT sql.contains("s.ndp_id")     -- not hardcoded
    ASSERT NOT sql.contains("s.state ")     -- not hardcoded (note trailing space)
END
```

#### 4.2.4 State Transitions Uses Config States

```
ALGORITHM: test_state_transitions_uses_config_states
PURPOSE: Verify on/off is not hardcoded

BEGIN
    config <- TransitionConfig {
        states: ["open", "closed"],
        direction_mapping: None,
        ...
    }

    generator <- StateTransitionGenerator::new("test-stream", "silver.test", "event_time")
    sql <- generator.generate(&config, Action::Recreate).unwrap()

    -- Should use configured states
    ASSERT sql.contains("'open'")
    ASSERT sql.contains("'closed'")

    -- Should NOT contain hardcoded on/off
    ASSERT NOT sql.contains("= 'off'")
    ASSERT NOT sql.contains("= 'on'")
    ASSERT NOT sql.contains("'opening'")
    ASSERT NOT sql.contains("'closing'")
END
```

#### 4.2.5 Device Type Reads from Config

```
ALGORITHM: test_device_type_uses_config_mapping
PURPOSE: Verify device type patterns come from config

BEGIN
    config <- TransitionConfig {
        device_type_mapping: Some([
            DeviceTypeRule { pattern: "sensor_%", device_type: "sensor" },
            DeviceTypeRule { pattern: "actuator_%", device_type: "actuator" },
        ]),
        ...
    }

    generator <- StateTransitionGenerator::new("test-stream", "silver.test", "event_time")
    sql <- generator.generate(&config, Action::Recreate).unwrap()

    ASSERT sql.contains("LIKE 'sensor_%'")
    ASSERT sql.contains("LIKE 'actuator_%'")
    ASSERT sql.contains("'sensor'")
    ASSERT sql.contains("'actuator'")

    -- Should NOT contain hardcoded patterns
    ASSERT NOT sql.contains("'door_%'")
    ASSERT NOT sql.contains("'window_%'")
    ASSERT NOT sql.contains("'motion_%'")
    ASSERT NOT sql.contains("'light_%'")
END
```

#### 4.2.6 Device Type Omitted When No Mapping

```
ALGORITHM: test_device_type_omitted_when_no_mapping
PURPOSE: Verify device_type column is absent when config has no mapping

BEGIN
    config <- TransitionConfig {
        device_type_mapping: None,
        ...
    }

    generator <- StateTransitionGenerator::new("test-stream", "silver.test", "event_time")
    sql <- generator.generate(&config, Action::Recreate).unwrap()

    ASSERT NOT sql.contains("device_type")
    ASSERT NOT sql.contains("LIKE")
END
```

#### 4.2.7 Stream Type from Config

```
ALGORITHM: test_stream_type_read_from_config
PURPOSE: Verify stream type is read from config, not inferred from stream_id

BEGIN
    -- Create a stream with a misleading name but explicit stream_type
    mock_loader <- MockConfigLoader::new()
        .with_stream("my-forecast-stream", StreamConfig {
            stream_id: "my-forecast-stream",
            stream_type: Some(StreamType::Observation),  -- Despite "forecast" in name
            ...
        })

    generator <- AlignedViewGenerator::new(mock_loader)

    -- The stream type should be Observation (from config), not Forecast (from name)
    stream_type <- generator.determine_stream_type("my-forecast-stream")
    ASSERT stream_type == StreamType::Observation
END
```

#### 4.2.8 No Objectives Skips Threshold Section

```
ALGORITHM: test_no_objectives_generates_no_threshold_section
PURPOSE: Domains with no objectives should still generate valid SQL

BEGIN
    domain <- DomainConfig {
        objectives: [],  -- empty
        streams: [ actuator_stream, primary_stream ],
        ...
    }

    mock_loader <- create_mock_loader_for(domain)
    generator <- EventsGenerator::from_domain_config(&domain, mock_loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    -- State transitions still present
    ASSERT sql.contains("STATE TRANSITIONS")

    -- Threshold section absent or empty
    ASSERT NOT sql.contains("threshold_crossing")
    ASSERT NOT sql.contains("hourly_obs")
END
```

#### 4.2.9 No Actuator Stream Skips State Transition Section

```
ALGORITHM: test_no_actuator_stream_skips_state_transitions
PURPOSE: Domains with only observation streams should skip state transitions

BEGIN
    domain <- DomainConfig {
        streams: [
            StreamRef { role: Primary, ... },
            StreamRef { role: Context, ... },
        ],
        objectives: [ some_objective ],
        ...
    }

    mock_loader <- create_mock_loader_for(domain)
    generator <- EventsGenerator::from_domain_config(&domain, mock_loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    ASSERT NOT sql.contains("state_transition")
    ASSERT NOT sql.contains("from_state")
    ASSERT sql.contains("threshold_crossing")  -- still generates this
END
```

### 4.3 Integration Test

```
ALGORITHM: test_generated_sql_executes_against_timescaledb
PURPOSE: End-to-end verification that generated SQL is syntactically valid

PRECONDITION: Integration TimescaleDB available (DEPLOY_ENV=integration)

BEGIN
    -- Load real config from disk
    config_dir <- "/workspaces/neural-data-platform/config"
    loader <- FileSystemConfigLoader::new(config_dir)
    domain <- loader.load_domain_config("indoor-air-quality")

    -- Generate SQL
    generator <- EventsGenerator::from_domain_config(&domain, loader)
    sql <- generator.generate(Action::Recreate).unwrap()

    -- Connect to integration TimescaleDB
    conn <- connect_to_integration_db()

    -- Execute within a transaction that we roll back
    conn.execute("BEGIN")
    result <- conn.execute(sql)
    conn.execute("ROLLBACK")

    -- Assert SQL executed without error
    ASSERT result.is_ok(), format!("SQL execution failed: {}", result.err())

    -- Optionally verify objects were created (before rollback)
    -- ASSERT conn.table_exists("gold.events")
    -- ASSERT conn.procedure_exists("gold.detect_events")
END
```

### 4.4 Backward Compatibility Test

```
ALGORITHM: test_air_quality_domain_generates_equivalent_sql
PURPOSE: Ensure the refactored generator produces functionally equivalent SQL
         for the existing air-quality domain

BEGIN
    -- Generate SQL with the OLD hardcoded approach (captured as golden file)
    expected_sql <- read_file("tests/fixtures/events_detection_v1.sql")

    -- Generate SQL with the NEW config-driven approach
    domain <- load_air_quality_domain_config()
    loader <- create_loader_with_real_stream_configs()
    generator <- EventsGenerator::from_domain_config(&domain, loader)
    actual_sql <- generator.generate(Action::Recreate).unwrap()

    -- Normalize whitespace for comparison
    expected_normalized <- normalize_sql(expected_sql)
    actual_normalized   <- normalize_sql(actual_sql)

    -- The SQL should be semantically equivalent
    -- (exact string match after normalization, or key fragment matching)
    ASSERT actual_normalized.contains("silver.state_events")
    ASSERT actual_normalized.contains("gold.air_quality_hourly")
    ASSERT actual_normalized.contains("800")   -- CO2 threshold from config
    ASSERT actual_normalized.contains("12")    -- PM2.5 threshold from config
    ASSERT actual_normalized.contains("healthy_co2")
    ASSERT actual_normalized.contains("healthy_pm25")
END
```

---

## 5. Constants Definition (P1 Items)

### 5.1 Shared Constants

These constants eliminate magic strings repeated across multiple files.
They should live in a shared module imported by all generators.

```rust
// tools/ndp-gold-ddl/src/constants.rs

/// Default entity identifier column used across NDP streams.
/// All Silver tables use "ndp_id" as the entity column unless
/// overridden in stream config's identity_fields.
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";

/// Gold schema name. All Gold layer objects are created in this schema.
pub const GOLD_SCHEMA: &str = "gold";

/// Silver schema name. All Silver layer tables live here.
pub const SILVER_SCHEMA: &str = "silver";

/// Default timestamp column for observation streams.
pub const DEFAULT_OBSERVATION_TIMESTAMP: &str = "observation_time";

/// Default timestamp column for event streams.
pub const DEFAULT_EVENT_TIMESTAMP: &str = "event_time";
```

### 5.2 Usage

```
BEFORE: "gold.events"                          --> format!("{}.events", GOLD_SCHEMA)
BEFORE: "ndp_id"                               --> NDP_ENTITY_COLUMN
BEFORE: "observation_time"                     --> DEFAULT_OBSERVATION_TIMESTAMP
BEFORE: format!("gold.{}_transitions", ...)    --> format!("{}.{}_transitions", GOLD_SCHEMA, ...)
```

### 5.3 Files Affected by P1 Constants

| File | Current Usage | Replace With |
|------|---------------|--------------|
| `events.rs` (6 locations) | `"gold."` prefix | `GOLD_SCHEMA` |
| `continuous_aggregate.rs` | `"gold."` prefix, `"ndp_id"` | `GOLD_SCHEMA`, `NDP_ENTITY_COLUMN` |
| `state_transitions.rs` | `"gold."` prefix, `"ndp_id"` default | `GOLD_SCHEMA`, `NDP_ENTITY_COLUMN` |
| `aligned_view.rs` | `"gold."` prefix | `GOLD_SCHEMA` |
| `sync.rs` | `schemaname = 'gold'` | `GOLD_SCHEMA` |
| `main.rs` | `"ndp_id"` | `NDP_ENTITY_COLUMN` |

---

## 6. Implementation Order

The algorithms above should be implemented in this order to maintain
green tests at every step:

```
Phase 1: P1 Constants (low risk, no behavior change)
    1a. Create constants.rs with NDP_ENTITY_COLUMN, GOLD_SCHEMA
    1b. Replace magic strings across all files
    1c. Run existing tests -- all must pass unchanged

Phase 2: AlignedView stream_type fix (isolated, smallest P0 change)
    2a. Add stream_type field to ndp-gold-ddl StreamConfig
    2b. Update determine_stream_type to read from config with fallback
    2c. Write test_stream_type_read_from_config
    2d. Run existing tests -- all must pass (fallback preserves behavior)

Phase 3: StateTransition dehard-coding (moderate, config extension)
    3a. Add DeviceTypeRule struct and device_type_mapping to TransitionConfig
    3b. Refactor generate_default_direction_case to use states[]
    3c. Refactor generate_device_type_case to use config mapping
    3d. Write test_state_transitions_uses_config_states
    3e. Write test_device_type_uses_config_mapping
    3f. Update home-assistant-state config.json with device_type_mapping
    3g. Run all tests

Phase 4: EventsGenerator refactoring (largest change, core P0)
    4a. Add config_loader field to EventsGenerator
    4b. Update constructor to accept ConfigLoader
    4c. Implement helper subroutines (find_stream_by_role, etc.)
    4d. Refactor generate_detection_procedure step-by-step:
        - First: Replace Silver table reference (test with mock)
        - Second: Replace entity/state field references
        - Third: Replace threshold crossings with objective iteration
        - Fourth: Replace context enrichment with derived columns
    4e. Write test_no_hardcoded_domain_values (regression guard)
    4f. Write backward compatibility test
    4g. Run all tests

Phase 5: Integration verification
    5a. Write integration test (requires TimescaleDB)
    5b. Generate SQL for air-quality domain
    5c. Compare against current production SQL
    5d. Deploy to integration environment
```
