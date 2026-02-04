# FE-001: Gold Layer Foundation (V1.1)

> **Feature ID:** fe-001
> **Version:** V1.1
> **Created:** 2026-02-03
> **Status:** Scoping
> **Roadmap Reference:** [gold-001/FEATURE-ROADMAP.md](../gold-001/FEATURE-ROADMAP.md)
> **Architecture Decisions:** [architecture/DECISIONS.md](./architecture/DECISIONS.md)

---

## Executive Summary

V1.1 establishes the **Gold Layer Foundation**—the declarative, config-driven infrastructure that transforms Silver layer data into ML-ready features for pattern detection. This is not a fixed set of capabilities; it's an **extensible architecture** that enables future versions to add streams, features, and objectives without code changes.

### The V1.1 Promise

| Capability | Description |
|------------|-------------|
| **Declarative Gold Layer** | JSON config drives Silver → Gold transformation |
| **Stream Classification** | Metadata distinguishes state vs continuous vs forecast |
| **Time Alignment** | All streams joined on consistent hourly buckets |
| **Feature Computation** | Rolling stats, lag features, trends—config-driven |
| **Unified Events** | State transitions + threshold crossings in single view |
| **Objectives Framework** | Declarative target specification for pattern filtering |

### Success Test

> **Can we add a new stream (e.g., `outdoor-air-quality`) to the Gold layer by *only editing JSON config*?**

If yes, the architecture works. This is the "fast-follower test" that validates V1.1.

---

## Problem Statement

### Current State (V1.0)

V1.0 delivers:
- Working Bronze → Silver pipeline
- Config-driven MQTT & HTTP ingestion
- Config-driven Silver schema definition
- Config-driven data dictionary
- Multiple streams: air-quality, outdoor-weather, home-assistant-state, nws-forecast

**Gap**: Silver data exists but is not prepared for correlation analysis. Each stream is isolated. No hourly aggregates. No cross-stream alignment. No feature computation.

### Target State (V1.1)

V1.1 delivers:
- Gold layer continuous aggregates per stream (hourly)
- Cross-stream aligned view joining all streams
- Computed features (rolling mean, std, lag, trend)
- Unified event abstraction (state transitions + threshold crossings)
- Objectives stored and queryable
- All driven by JSON configuration with schema validation

### Why This Matters

V1.2 (Pattern Detection Engine) requires:
1. **Classified streams** to know which are causes vs effects
2. **Time-aligned data** for correlation scanning
3. **Consistent granularity** (hourly buckets) for statistical tests
4. **Events** to detect and correlate
5. **Objectives** to filter relevant relationships

V1.1 exists to enable V1.2. Every feature has a purpose in the capability chain.

---

## Scope Definition

### In Scope

#### Tier 1: Architecture (Must Have)

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **v11-A01** | Gold ETL JSON Schema | JSON Schema for `gold_etl` config section | Schema validates; helpful error messages |
| **v11-A02** | Gold DDL Tool | Rust CLI tool (`ndp-gold-ddl`) generates SQL from config | Generates valid TimescaleDB SQL; idempotent; called from deploy.sh |
| **v11-A03** | Alignment JSON Schema | JSON Schema for cross-stream alignment | Schema validates alignment configs |
| **v11-A04** | Alignment Interpreter | Rust module generates aligned view SQL | Generates valid JOIN SQL; NULL handling |
| **v11-A05** | Objectives JSON Schema | JSON Schema for objectives config | Schema validates; supports all condition types |
| **v11-A06** | Feature Type Registry | Extensible registry for feature generators | New feature types addable via trait impl |

#### Tier 2: Capabilities (Enabled by Architecture)

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **v11-001** | Stream Type Classification | Add `stream_type` enum to stream config | All streams classified; validation rejects unknown |
| **v11-002** | Classification Propagation | Stream type in Silver metadata & dictionary | MCP tool can query stream types |
| **v11-003** | Per-Stream Continuous Aggregates | Hourly aggregates for each Silver table | Aggregates exist; query < 100ms for 30 days |
| **v11-004** | Aggregate Refresh Policy | Auto-refresh every 15 min, 4-hour lookback | Policy configured; resource usage within budget |
| **v11-005** | Cross-Stream Aligned View | Materialized view joining all streams hourly | View operational; NULLs handled correctly |
| **v11-006** | State Transition Materializer | Derive transition events from state streams | Transitions extracted; `is_actual_transition` works |
| **v11-007** | Objectives Storage | Store objectives in etcd, expose via MCP | Objectives queryable; sync from config works |
| **v11-008** | Basic Feature Computation | Rolling mean, std, min, max per metric | Features in aligned view; configurable windows |
| **v11-009** | Lag Feature Computation | Metric values at t-1h, t-6h, t-24h | Lag features computed; NULL handling for edges |
| **v11-010** | Gold Layer Data Dictionary | Metadata for Gold tables and views | All Gold objects documented; queryable |
| **v11-011** | Correlation-Ready Dashboard | Grafana showing aligned streams + objectives | Dashboard loads < 2s; shows objective thresholds |
| **v11-012** | Threshold Crossing Generator | Events when metrics cross objective thresholds | Crossings detected; rising/falling direction |
| **v11-013** | Unified Events View | Combine state transitions + threshold crossings | Single view; consistent schema; hourly aggregates |

#### Tier 3: Validation (Proves Architecture)

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **v11-V01** | Fast-Follower Stream Test | Add `outdoor-air-quality` via config only | Zero Rust code changes; stream in aligned view |
| **v11-V02** | New Feature Type Test | Add new feature type via registry | Trait impl only; no interpreter changes |

### Out of Scope (Deferred to V1.2+)

| Item | Reason | Target Version |
|------|--------|----------------|
| Granger causality scanning | Requires V1.1 aligned data | V1.2 |
| Correlation candidate ranking | Requires pattern detection | V1.2 |
| Anomaly event detection | Requires baseline learning | V1.2 |
| Trend change event detection | Requires trend features | V1.3 |
| Causal validation (PC algorithm) | Requires correlation candidates | V1.3 |
| Predictive models | Requires validated relationships | V1.3 |
| Action framework | Requires predictions | V1.3 |

---

## Technical Design Overview

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     V1.1 DECLARATIVE GOLD ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  CONFIG LAYER (JSON + JSON Schema Validation)                               │
│  ─────────────────────────────────────────────                               │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐                │
│  │ stream configs │  │ domain.yaml    │  │ objectives.json│                │
│  │ + gold_etl     │  │ (alignment)    │  │                │                │
│  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘                │
│          │                   │                   │                          │
│          └───────────────────┼───────────────────┘                          │
│                              │                                              │
│                              ▼                                              │
│  DDL GENERATION (Rust CLI Tool - ADR-FE001-001)                             │
│  ───────────────────────────────────────────────                             │
│  ┌────────────────────────────────────────────────────────────────────────┐│
│  │ ndp-gold-ddl (tools/ndp-gold-ddl/)                                      ││
│  │ • Reads stream config + gold_etl section                               ││
│  │ • Generates CREATE MATERIALIZED VIEW statements                        ││
│  │ • Generates ADD_CONTINUOUS_AGGREGATE_POLICY statements                 ││
│  │ • Called from deploy.sh → output piped to psql                         ││
│  └────────────────────────────────────────────────────────────────────────┘│
│                              │                                              │
│                              ▼                                              │
│  DEPLOYMENT (deploy.sh - existing orchestrator)                             │
│  ──────────────────────────────────────────────                              │
│  ┌────────────────────────────────────────────────────────────────────────┐│
│  │ deploy.sh apply                                                         ││
│  │ • handle_silver_table() → ddl-generator.sh (Bash - unchanged)          ││
│  │ • handle_gold_table()   → ndp-gold-ddl (Rust - NEW)                    ││
│  │ • handle_domain()       → ndp-gold-ddl --domain (Rust - NEW)           ││
│  └────────────────────────────────────────────────────────────────────────┘│
│                              │                                              │
│                              ▼                                              │
│  TIMESCALEDB LAYER (Generated from config)                                  │
│  ─────────────────────────────────────────                                   │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐                │
│  │ gold.stream_   │  │ gold.domain_   │  │ gold.events_   │                │
│  │ _hourly views  │  │ _aligned view  │  │ unified        │                │
│  └────────────────┘  └────────────────┘  └────────────────┘                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

> **Key Architecture Decision**: Gold DDL generation uses a Rust CLI tool (`ndp-gold-ddl`)
> rather than Bash. See [ADR-FE001-001](./architecture/DECISIONS.md#adr-fe001-001-gold-ddl-generation-in-rust) for rationale.

### Config-Driven Pattern

Following V1.0's proven approach:

```json
{
  "gold_etl": {
    "enabled": true,
    "description": "Gold layer transformation for air-quality stream",

    "aggregates": {
      "granularities": ["1 hour", "1 day"],
      "default_metrics": ["mean", "std", "min", "max", "count"],
      "fields": {
        "pm25": { "metrics": ["mean", "std", "min", "max", "p95"] },
        "co2": { "metrics": ["mean", "std", "min", "max"] },
        "temperature_c": { "metrics": ["mean", "min", "max"] }
      }
    },

    "features": {
      "lag": {
        "enabled": true,
        "lags_hours": [1, 6, 24],
        "fields": ["pm25", "co2"]
      },
      "rolling": {
        "enabled": true,
        "windows": ["4 hours", "24 hours"],
        "stats": ["mean", "std"],
        "fields": ["pm25"]
      },
      "trend": {
        "enabled": true,
        "window": "4 hours",
        "fields": ["pm25", "co2"]
      }
    },

    "transitions": {
      "enabled": true,
      "description": "For state_event streams only"
    }
  }
}
```

### Stream Type Classification

| Type | Description | Correlation Role | Examples |
|------|-------------|------------------|----------|
| `observation` | Continuous numeric readings | **Effect** (target) | PM2.5, CO2, temperature |
| `state_event` | Binary/discrete state changes | **Cause** (source) | Window open/close, door |
| `forecast` | Future predictions from external source | **Context** | NWS weather forecast |
| `dimension` | Slowly changing reference data | **Metadata** | Entity context, locations |

### Unified Event Abstraction

V1.1 implements two event types in a unified schema:

```sql
EVENT = {
    event_id,            -- Unique identifier
    event_time,          -- When the event occurred
    stream_id,           -- Source stream
    entity_id,           -- Which entity (ndp_id)
    event_type,          -- "state_transition" | "threshold_crossing"
    details: JSONB       -- Type-specific payload
}
```

| Event Type | Source | Detection |
|------------|--------|-----------|
| `state_transition` | state_event streams | State field value changes |
| `threshold_crossing` | objectives + observations | Metric crosses declared threshold |

Future event types (V1.2+): `anomaly`, `trend_change`

---

## Implementation Phases

### Phase A: Architecture Foundation (Target: Week 1-2)

**Focus**: Build the extensible architecture before implementing capabilities.

| Feature | Priority | Dependencies |
|---------|----------|--------------|
| v11-A01: Gold ETL JSON Schema | Critical | V1.0 schema validation |
| v11-A02: Gold ETL Interpreter (basic) | Critical | v11-A01 |
| v11-A03: Alignment JSON Schema | Critical | V1.0 schema validation |
| v11-A05: Objectives JSON Schema | High | V1.0 schema validation |
| v11-001: Stream Type Classification | High | V1.0 stream config |

**Exit Criteria**:
- [ ] JSON Schemas defined and validated
- [ ] Basic interpreter can generate SQL from config
- [ ] Stream types added to existing configs
- [ ] Architecture review completed

### Phase B: First Stream (Target: Week 3)

**Focus**: Apply architecture to `air-quality` stream as reference implementation.

| Feature | Priority | Dependencies |
|---------|----------|--------------|
| v11-002: Classification Propagation | High | v11-001 |
| v11-003: Per-Stream Continuous Aggregates (air-quality) | Critical | v11-A02 |
| v11-004: Aggregate Refresh Policy | High | v11-003 |
| v11-A06: Feature Type Registry (basic) | High | v11-A02 |
| v11-008: Basic Feature Computation (air-quality) | Medium | v11-A06 |

**Exit Criteria**:
- [ ] `gold.air_quality_hourly` generated from config
- [ ] Refresh policy operational
- [ ] At least one feature type (lag or rolling) working
- [ ] **Config-only change can modify aggregate fields**

### Phase C: Cross-Stream + Alignment (Target: Week 4)

**Focus**: Extend to remaining streams, build alignment view.

| Feature | Priority | Dependencies |
|---------|----------|--------------|
| v11-003: Continuous Aggregates (outdoor-weather, state-events) | Critical | v11-A02 |
| v11-A04: Alignment Interpreter | Critical | v11-A03 |
| v11-005: Cross-Stream Aligned View (3 streams) | Critical | v11-A04 |
| v11-006: State Transition Materializer | High | v11-001 |
| v11-007: Objectives Storage | Medium | v11-A05 |

**Exit Criteria**:
- [ ] 3 streams in Gold layer (air-quality, outdoor-weather, home-assistant-state)
- [ ] Aligned view operational
- [ ] State transitions extractable
- [ ] Objectives stored in etcd

**Deliberately excluded**: `outdoor-air-quality` (saved for fast-follower test)

### Phase D: Validation + Dashboard (Target: Week 5)

**Focus**: Prove the architecture with fast-follower test.

| Feature | Priority | Dependencies |
|---------|----------|--------------|
| v11-V01: Fast-Follower Test (outdoor-air-quality) | Critical | v11-A02, v11-A04 |
| v11-009: Lag Feature Computation | Medium | v11-A06 |
| v11-010: Gold Layer Data Dictionary | Medium | v11-003, v11-005 |
| v11-011: Correlation-Ready Dashboard | High | v11-005, v11-007 |

**Exit Criteria**:
- [ ] `outdoor-air-quality` added to Gold layer via **config change only**
- [ ] No Rust code changes required for fast-follower
- [ ] Dashboard demonstrates all capabilities
- [ ] Architecture validated for V1.2

### Phase E: Unified Event Abstraction (Extended)

**Focus**: Complete the unified event abstraction for V1.2 handoff.

| Feature | Priority | Dependencies |
|---------|----------|--------------|
| v11-012: Threshold Crossing Generator | Critical | v11-007 |
| v11-013: Unified Events View | Critical | v11-006, v11-012 |
| v11-V02: New Feature Type Test | Medium | v11-A06 |

**Exit Criteria**:
- [ ] Threshold crossing events generated from objectives config
- [ ] Unified events view combines state + threshold events
- [ ] Hourly event aggregates in aligned view
- [ ] V1.2 can query unified events for pattern detection

---

## Feature Specifications

### v11-A01: Gold ETL JSON Schema

**Location**: `config/schemas/gold-etl.schema.json`

**Schema Structure**:
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "gold-etl.schema.json",
  "type": "object",
  "properties": {
    "gold_etl": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "aggregates": { "$ref": "#/definitions/aggregates" },
        "features": { "$ref": "#/definitions/features" },
        "transitions": { "$ref": "#/definitions/transitions" }
      }
    }
  },
  "definitions": {
    "aggregates": {
      "type": "object",
      "properties": {
        "granularities": {
          "type": "array",
          "items": { "type": "string", "pattern": "^\\d+ (hour|day|minute)s?$" }
        },
        "fields": {
          "type": "object",
          "additionalProperties": {
            "type": "object",
            "properties": {
              "metrics": {
                "type": "array",
                "items": { "enum": ["mean", "std", "min", "max", "count", "p95", "p99"] }
              }
            }
          }
        }
      }
    }
  }
}
```

**Acceptance Criteria**:
- [ ] Schema defined and documented
- [ ] Validates against example gold_etl configs
- [ ] Integrated with existing schema validation pipeline
- [ ] Error messages are helpful for config authors

---

### v11-A02: Gold DDL Tool (ndp-gold-ddl)

**Location**: `tools/ndp-gold-ddl/` (new Rust CLI tool)

**Rationale**: See [ADR-FE001-001](./architecture/DECISIONS.md#adr-fe001-001-gold-ddl-generation-in-rust). Gold DDL is too complex for Bash string templating (continuous aggregates with computed expressions, multiple granularities, domain-aligned joins).

**CLI Interface**:
```bash
# Generate DDL for a stream's Gold layer
ndp-gold-ddl generate --stream air-quality --mode full

# Generate DDL for a domain (aligned view, unified events)
ndp-gold-ddl generate --domain indoor-air-quality

# Validate config without generating
ndp-gold-ddl validate --stream air-quality

# Schema evolution (add columns to existing)
ndp-gold-ddl evolve --stream air-quality
```

**Tool Structure**:
```
tools/ndp-gold-ddl/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library for testing
│   ├── generators/
│   │   ├── mod.rs
│   │   ├── continuous_aggregate.rs
│   │   ├── aligned_view.rs
│   │   ├── features.rs
│   │   └── events.rs
│   └── validation/
│       ├── mod.rs
│       └── expressions.rs   # Validate metric expressions
└── tests/
    ├── continuous_aggregate_test.rs
    └── aligned_view_test.rs
```

**Integration with deploy.sh**:
```bash
handle_gold_table() {
    local stream_id="$1"
    local ddl=$(ndp-gold-ddl generate --stream "$stream_id" --mode full)
    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
}
```
```

**Generated SQL Examples**:

```sql
-- Continuous aggregate for air-quality
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MIN(pm25) AS pm25_min,
    MAX(pm25) AS pm25_max,
    AVG(co2) AS co2_mean,
    STDDEV(co2) AS co2_std,
    COUNT(*) AS sample_count
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;

-- Refresh policy
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

**Acceptance Criteria**:
- [ ] Generates valid TimescaleDB SQL
- [ ] Idempotent (can re-run safely)
- [ ] Logs generated SQL for debugging
- [ ] Validates config before execution

---

### v11-005: Cross-Stream Aligned View

**Purpose**: Single materialized view joining all streams on hourly buckets. Primary input to V1.2 pattern detection.

**Generated SQL**:
```sql
CREATE MATERIALIZED VIEW gold.aligned_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', COALESCE(
        aq.bucket,
        ow.bucket,
        se.bucket
    )) AS bucket,

    -- Indoor Air Quality (observation - potential effects)
    aq.pm25_mean AS indoor_pm25,
    aq.co2_mean AS indoor_co2,
    aq.temp_mean AS indoor_temp,
    aq.humidity_mean AS indoor_humidity,

    -- Outdoor Weather (observation - context/causes)
    ow.temp_mean AS outdoor_temp,
    ow.humidity_mean AS outdoor_humidity,
    ow.wind_speed_mean AS wind_speed,
    ow.pressure_mean AS pressure,

    -- State Events (state_event - potential causes)
    se.transition_to_on_count AS window_opens,
    se.transition_to_off_count AS window_closes,
    se.state_end_of_hour AS last_window_state,

    -- Event counts from unified events
    ev.total_events,
    ev.state_transition_count,
    ev.threshold_crossing_count

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se ON aq.bucket = se.bucket
LEFT JOIN gold.events_hourly ev ON aq.bucket = ev.bucket;
```

**Design Decisions**:
1. **FULL OUTER JOIN**: Preserves rows even when some streams have no data
2. **Hourly granularity**: Balances detail vs noise for correlation
3. **NULL handling**: V1.2 must handle sparse data gracefully

**Acceptance Criteria**:
- [ ] View created with all current streams
- [ ] Query returns data for last 30 days
- [ ] NULLs handled correctly (no dropped rows)
- [ ] Query performance < 100ms for 30-day range

---

### v11-006: State Transition Materializer

**Purpose**: Convert raw state events into explicit transition records. Generic building block for any `state_event` stream.

**Config**:
```json
{
  "gold_etl": {
    "transitions": {
      "enabled": true,
      "state_field": "state",
      "entity_field": "ndp_id",
      "track_duration": true,
      "include_in_alignment": true
    }
  }
}
```

**Generated SQL**:
```sql
CREATE VIEW gold.{stream_id}_transitions AS
SELECT
    event_time AS transition_time,
    ndp_id AS entity_id,
    LAG(state) OVER w AS from_state,
    state AS to_state,
    CASE
        WHEN LAG(state) OVER w IS DISTINCT FROM state THEN TRUE
        WHEN LAG(state) OVER w IS NULL THEN TRUE
        ELSE FALSE
    END AS is_actual_transition,
    event_time - LAG(event_time) OVER w AS duration_in_previous_state
FROM silver.state_events
WHERE stream_id = '{stream_id}'
WINDOW w AS (PARTITION BY ndp_id ORDER BY event_time);
```

**Acceptance Criteria**:
- [ ] Transition view generated from config (no hardcoded stream names)
- [ ] Works for ANY state_event stream
- [ ] `is_actual_transition` correctly filters noise
- [ ] Duration computed accurately

---

### v11-012: Threshold Crossing Generator

**Purpose**: Generate events when observation metrics cross objective thresholds.

**Key Insight**: Objectives define thresholds. Crossings of those thresholds ARE meaningful events.

**Config**:
```json
{
  "gold_etl": {
    "threshold_crossings": {
      "enabled": true,
      "source": "objectives",
      "include_in_unified": true
    }
  }
}
```

**Generated SQL**:
```sql
CREATE VIEW gold.threshold_crossings AS
WITH with_lag AS (
    SELECT
        observation_time,
        stream_id,
        ndp_id,
        metric_name,
        threshold_value,
        condition,
        objective_id,
        metric_value,
        LAG(metric_value) OVER (
            PARTITION BY stream_id, ndp_id, metric_name
            ORDER BY observation_time
        ) AS prev_value
    FROM observation_with_thresholds
)
SELECT
    observation_time AS event_time,
    stream_id,
    ndp_id AS entity_id,
    'threshold_crossing' AS event_type,
    metric_name,
    threshold_value,
    objective_id,
    metric_value AS current_value,
    prev_value AS previous_value,
    CASE
        WHEN condition = '<' AND metric_value >= threshold_value
             AND prev_value < threshold_value THEN 'rising'
        WHEN condition = '<' AND metric_value < threshold_value
             AND prev_value >= threshold_value THEN 'falling'
    END AS crossing_direction
FROM with_lag
WHERE crossing_direction IS NOT NULL;
```

**Acceptance Criteria**:
- [ ] Generates threshold crossing events from objectives
- [ ] Detects both rising and falling crossings
- [ ] Works for all condition types (<, >, <=, >=)
- [ ] Indexed for V1.2 query patterns

---

### v11-013: Unified Events View

**Purpose**: Combine state transitions and threshold crossings into single queryable view.

**SQL**:
```sql
CREATE VIEW gold.events_unified AS

-- State transition events
SELECT
    transition_time AS event_time,
    stream_id,
    entity_id,
    'state_transition'::text AS event_type,
    jsonb_build_object(
        'from_state', from_state,
        'to_state', to_state,
        'duration_in_previous_ms',
            EXTRACT(EPOCH FROM duration_in_previous_state) * 1000
    ) AS details
FROM gold.state_transitions
WHERE is_actual_transition = TRUE

UNION ALL

-- Threshold crossing events
SELECT
    event_time,
    stream_id,
    entity_id,
    'threshold_crossing'::text AS event_type,
    jsonb_build_object(
        'metric', metric_name,
        'threshold', threshold_value,
        'direction', crossing_direction,
        'value', current_value,
        'objective_id', objective_id
    ) AS details
FROM gold.threshold_crossings;
```

**Acceptance Criteria**:
- [ ] View combines state transitions and threshold crossings
- [ ] Consistent event schema (event_time, stream_id, entity_id, event_type, details)
- [ ] Hourly aggregation continuous aggregate created
- [ ] Query performance < 100ms for 30-day range

---

## Dependencies

### From V1.0 (Prerequisites)

| Dependency | Status | Notes |
|------------|--------|-------|
| Bronze → Silver pipeline | Complete | Working for all streams |
| Config-driven stream ingestion | Complete | MQTT & HTTP polling |
| Config-driven Silver schema | Complete | Schema definitions in JSON |
| Config-driven DQ basis | Complete | DQ rules in config |
| TimescaleDB operational | Complete | Hypertables running |
| Schema validation pipeline | Complete | JSON Schema validation |
| etcd configuration store | Complete | Config sync working |
| MCP tools for data dictionary | Complete | Query existing metadata |

### External Dependencies

| Dependency | Required For | Risk |
|------------|--------------|------|
| TimescaleDB continuous aggregates | v11-003, v11-005 | Low - already using |
| tokio-postgres | v11-A02 interpreter | Low - existing dep |
| serde_json | Config parsing | Low - existing dep |
| jsonschema | Schema validation | Low - existing pattern |

---

## Resource Constraints (Pi 5)

### Memory Budget

| Component | Allocation | Notes |
|-----------|------------|-------|
| Continuous aggregate refresh | < 100 MB peak | Per-stream, sequential |
| Aligned view query | < 50 MB | For 30-day range |
| Feature computation | < 25 MB | Window functions |
| **Total Gold layer** | < 200 MB | During peak operations |

### CPU Budget

| Operation | Target | Frequency |
|-----------|--------|-----------|
| Aggregate refresh | < 5% sustained | Every 15 min |
| Aligned view refresh | < 10% peak | Every 15 min |
| Dashboard queries | < 5% | On demand |

### Storage Estimates

| Object | Size (30 days) | Growth Rate |
|--------|----------------|-------------|
| Per-stream hourly aggregate | ~5 MB | ~0.2 MB/day |
| Aligned view | ~10 MB | ~0.3 MB/day |
| Unified events | ~2 MB | ~0.1 MB/day |
| **Total Gold layer** | ~50 MB | ~2 MB/day |

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Architecture extensibility** | Add new stream via config only | Fast-follower test |
| **Gold layer query performance** | < 100ms for 30-day aligned query | pg_stat_statements |
| **Stream classification coverage** | 100% of streams classified | Config audit |
| **Objective declarability** | All targets expressible in config | User testing |
| **Fast-follower time** | < 1 hour to add new stream to Gold | Timed exercise |
| **Unified event coverage** | State + threshold events in single view | Query validation |
| **Resource compliance** | Within Pi 5 budget | Monitoring |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Interpreter complexity explodes | Medium | High | Start simple, add cases incrementally |
| Continuous aggregates too expensive | Low | High | Test on Pi early; adjust refresh policies |
| Config schema doesn't cover edge cases | Medium | Medium | Design for 80%, allow escape hatches |
| Cross-stream alignment gaps | Medium | Medium | NULL handling; interpolation if needed |
| Feature computation too slow | Low | Medium | Limit initial features; profile carefully |

---

## V1.2 Handoff Requirements

For V1.1 to be complete, V1.2 must be able to:

- [ ] Query `gold.aligned_hourly` for all streams
- [ ] Query `gold.events_unified` for all event types
- [ ] Get stream classifications via MCP
- [ ] Get objectives via MCP
- [ ] Access lag features for correlation testing
- [ ] Access rolling statistics for baseline computation

**Contract**: The Gold layer schema (views, columns, event types) is the interface contract between V1.1 and V1.2. Changes require coordination.

---

## Open Questions

1. **Trend feature computation**: Linear regression slope in SQL is expensive. Do we compute in Rust instead?

2. **Percentile computation**: p95/p99 requires ordered aggregation. TimescaleDB supports this, but verify Pi performance.

3. **Forecast stream handling**: NWS forecasts are already hourly. Pass through or aggregate differently?

4. **Hysteresis for threshold crossings**: Should we add hysteresis to prevent chattering around thresholds? (Deferred to V1.2 if needed)

5. **Feature naming convention**: Need consistent naming for generated columns (e.g., `{stream}_{field}_{stat}_{window}`).

---

## Appendix: Existing Data Availability

V1.1 can leverage existing Bronze data for immediate testing:

| Stream | Bronze Data Available | Notes |
|--------|----------------------|-------|
| air-quality | ~30 days | PM2.5, CO2, temp, humidity |
| outdoor-weather | ~30 days | From Open-Meteo |
| home-assistant-state | ~14 days | Window/door sensors |
| nws-forecast-hourly | ~7 days | Weather forecasts |

This enables V1.2 to begin pattern detection immediately after V1.1 completes, without waiting for data accumulation.

---

## References

- [Architecture Decisions](./architecture/DECISIONS.md) - ADRs and architectural analysis
- [Gold Layer Feature Roadmap](../gold-001/FEATURE-ROADMAP.md) - Full V1.1 → V2.0 roadmap
- [V1.0 Release Manifest](.deploy/releases/v1.0.0.manifest.json) - Current platform state
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/) - External docs
- [Stream Configuration Schema](config/schemas/stream.schema.json) - Existing V1.0 schema

### Architecture Analysis Documents

The following analysis documents inform the decisions in this scope:

- [Config Patterns](./architecture/config-patterns.md) - V1 config architecture analysis
- [Schema Validation Patterns](./architecture/schema-validation-patterns.md) - Two-layer validation
- [Data Dictionary Patterns](./architecture/data-dictionary-patterns.md) - Metadata strategy
- [Crate Layout Patterns](./architecture/crate-layout-patterns.md) - Module organization
- [ETL Patterns](./architecture/etl-patterns.md) - Event-driven ETL architecture
