# dp-014: Config-Driven Gold Layer

## Overview

Extend NDP's configuration language to support Gold layer artifacts (materialized views, feature views, aggregations). This establishes the foundation for ML feature engineering.

**Origin:** Discussion during air-012 scoping (2026-01-30). Decided to keep Silver as simple fact storage and compute SCD/features in a config-driven Gold layer.

**Status:** Draft scope - will be revised before implementation.

---

## Motivation

### The Problem

Today, Gold layer artifacts (views, aggregates) are created via SQL scripts. This breaks the config-driven pattern established for Bronze (stream configs) and Silver (silver_etl configs).

```
Bronze: Config-driven ✅ (stream configs)
Silver: Config-driven ✅ (silver_etl in stream configs)
Gold:   SQL scripts ❌ (inconsistent)
```

### The Vision

```yaml
# In stream config or separate gold config
gold_views:
  - view_id: state_periods
    type: materialized_view
    source_table: silver.state_events
    refresh: on_demand  # or: continuous, scheduled
    definition:
      columns:
        - name: ndp_id
          source: ndp_id
        - name: state
          source: state
        - name: valid_from
          source: event_time
        - name: valid_to
          transform: LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time)
```

---

## Goals

1. **Config-driven Gold artifacts** - Materialized views, feature views defined in YAML
2. **First use case: SCD for state_events** - Point-in-time queries for ML
3. **Pattern for feature engineering** - Repeatable across domains
4. **Foundation for ML consumption** - Clean interface between Gold and ML layers

---

## Scope

### Part 1: Gold View Configuration Schema

Define the YAML schema for Gold layer artifacts:

```yaml
gold_views:
  - view_id: <unique identifier>
    type: materialized_view | view | continuous_aggregate
    description: <human readable>

    # Source configuration
    source:
      table: silver.state_events
      # OR
      query: |
        SELECT ... FROM silver.state_events
        JOIN silver.entity_context USING (ndp_id)

    # Refresh strategy (for materialized views)
    refresh:
      strategy: on_demand | scheduled | continuous
      schedule: "0 */6 * * *"  # cron for scheduled

    # Column definitions
    columns:
      - name: ndp_id
        type: text
        source: ndp_id  # direct mapping
      - name: valid_to
        type: timestamptz
        transform: LEAD(event_time) OVER (...)  # SQL transform

    # Indexes
    indexes:
      - columns: [ndp_id, valid_from]
        unique: false
```

### Part 2: SCD Materialized View for State Events

First use case - compute SCD semantics from air-012's simple event log:

```yaml
# config/base/gold/state_periods.yaml
gold_view_id: state_periods
description: "SCD Type 2 view of state events with valid_from/valid_to"
type: materialized_view

source:
  table: silver.state_events

refresh:
  strategy: on_demand
  # Can be refreshed after batch ETL or on schedule

columns:
  - name: ndp_id
    type: text
    source: ndp_id

  - name: state
    type: text
    source: state

  - name: valid_from
    type: timestamptz
    source: event_time

  - name: valid_to
    type: timestamptz
    transform: LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time)

  - name: duration_seconds
    type: integer
    transform: |
      EXTRACT(EPOCH FROM (
        LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time) - event_time
      ))

indexes:
  - name: idx_state_periods_lookup
    columns: [ndp_id, valid_from]

  - name: idx_state_periods_validity
    columns: [ndp_id, valid_from, valid_to]
```

### Part 3: DDL Generator Extension

Extend existing DdlGenerator (from dp-013) to support Gold views:

```rust
impl GoldDdlGenerator {
    pub fn generate_materialized_view(config: &GoldViewConfig) -> String;
    pub fn generate_refresh_command(config: &GoldViewConfig) -> String;
    pub fn generate_indexes(config: &GoldViewConfig) -> Vec<String>;
}
```

### Part 4: CLI/Deploy Integration

```bash
# List configured gold views
./deploy.sh list-gold-views

# Sync gold views (create/replace)
./deploy.sh sync-gold-views

# Refresh a specific materialized view
./deploy.sh refresh-gold-view state_periods

# Future: ndp gold list, ndp gold sync, ndp gold refresh
```

---

## Generated SQL (Example)

From the config above, generate:

```sql
-- Create materialized view
CREATE MATERIALIZED VIEW gold.state_periods AS
SELECT
    ndp_id,
    state,
    event_time AS valid_from,
    LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time) AS valid_to,
    EXTRACT(EPOCH FROM (
        LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time) - event_time
    ))::INTEGER AS duration_seconds
FROM silver.state_events;

-- Create indexes
CREATE INDEX idx_state_periods_lookup
ON gold.state_periods (ndp_id, valid_from);

CREATE INDEX idx_state_periods_validity
ON gold.state_periods (ndp_id, valid_from, valid_to);

-- Refresh command (for manual/scheduled refresh)
REFRESH MATERIALIZED VIEW gold.state_periods;
```

---

## Point-in-Time Query Pattern

Once `gold.state_periods` exists, point-in-time lookups are efficient:

```sql
-- "What was the window state at this observation time?"
SELECT state, valid_from, duration_seconds
FROM gold.state_periods
WHERE ndp_id = 'door_officewindow'
  AND valid_from <= '2026-01-30 10:00:00'
  AND (valid_to IS NULL OR valid_to > '2026-01-30 10:00:00');
```

This enables feature engineering for ML:

```sql
-- Feature vector: observations with state at observation time
SELECT
    o.observation_time,
    o.pm25,
    o.co2,
    o.humidity_pct,
    s.state AS window_state,
    s.duration_seconds AS window_state_duration
FROM silver.air_quality_observations o
LEFT JOIN gold.state_periods s
    ON s.ndp_id = 'door_officewindow'
    AND s.valid_from <= o.observation_time
    AND (s.valid_to IS NULL OR s.valid_to > o.observation_time)
WHERE o.ndp_id = 'aq_airgradient_1';
```

---

## Future Extensions (Not in Initial Scope)

### Continuous Aggregates (TimescaleDB)

```yaml
gold_views:
  - view_id: hourly_air_quality
    type: continuous_aggregate
    source:
      table: silver.air_quality_observations
    time_column: observation_time
    bucket: 1 hour
    aggregations:
      - name: avg_pm25
        function: AVG(pm25)
      - name: max_co2
        function: MAX(co2)
```

### Feature Views (ML-Oriented)

```yaml
gold_views:
  - view_id: ventilation_features
    type: feature_view
    description: "ML features for ventilation correlation"
    entity_key: observation_time
    features:
      - name: pm25_current
        source: silver.air_quality_observations.pm25
      - name: window_open
        source: gold.state_periods.state
        transform: "CASE WHEN state = 'on' THEN 1 ELSE 0 END"
      - name: window_open_minutes
        source: gold.state_periods.duration_seconds
        transform: "duration_seconds / 60"
```

### Scheduled Refresh

```yaml
refresh:
  strategy: scheduled
  schedule: "0 */6 * * *"  # Every 6 hours
  # Integrates with daemon background workers
```

---

## Acceptance Criteria

### Part 1: Config Schema
- [ ] Gold view config schema defined
- [ ] Schema supports: materialized_view, view types
- [ ] Column definitions with source mapping and transforms
- [ ] Index definitions
- [ ] Refresh strategy configuration

### Part 2: SCD for State Events
- [ ] `state_periods` config created
- [ ] Materialized view generates correct SQL
- [ ] Point-in-time lookups work efficiently
- [ ] Duration computed correctly

### Part 3: DDL Generator
- [ ] `GoldDdlGenerator` implemented
- [ ] Generates CREATE MATERIALIZED VIEW
- [ ] Generates indexes
- [ ] Generates REFRESH command

### Part 4: Deploy Integration
- [ ] `./deploy.sh sync-gold-views` works
- [ ] `./deploy.sh refresh-gold-view <id>` works
- [ ] Gold views listed in deployment status

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Silver state_events (air-012) | Pending | Source table for first use case |
| DdlGenerator (dp-013) | ✅ Ready | Pattern to extend |
| deploy.sh infrastructure | ✅ Ready | Add gold commands |

---

## Architectural Decisions

### Why Config-Driven?

1. **Consistency** - Same pattern as Bronze/Silver
2. **GitOps** - Gold artifacts versioned with code
3. **Repeatability** - New domains get Gold views via config
4. **Documentation** - Config is self-documenting

### Why Materialized View (not regular view)?

1. **Performance** - Point-in-time lookups need indexes
2. **ML workloads** - Batch feature computation benefits from materialization
3. **Refresh control** - Can refresh after ETL, not on every query

### Why Gold Layer (not in Silver)?

1. **Separation of concerns** - Silver = facts, Gold = features
2. **Flexibility** - Can iterate on feature logic without migrating fact data
3. **Multiple Gold views** - Same Silver facts can feed different feature views

---

## Relationship to ML Foundation

This feature establishes the **feature layer** that ML will consume:

```
Silver (Facts)           Gold (Features)              ML (Learning)
────────────────         ──────────────────           ─────────────
state_events      →      state_periods          →     Feature vectors
observations      →      ventilation_features   →     for clustering,
                                                       anomaly detection
```

Future ML features (ml-001+) will:
1. Read from Gold views
2. Export to training format
3. Train models (ruv-FANN, linfa)
4. Store model artifacts
5. Run inference

dp-014 provides the clean interface between data platform and ML.

---

## Notes

*This scope is a draft capturing our 2026-01-30 discussion. Will be revised before implementation based on air-012 learnings and further ML research.*
