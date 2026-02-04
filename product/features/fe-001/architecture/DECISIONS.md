# FE-001: Gold Layer Architecture Decisions

> **Created:** 2026-02-03
> **Status:** Draft for Review
> **Method:** Architecture Swarm Analysis

---

## Executive Summary

Five architecture agents analyzed the V1 codebase to inform Gold layer design. This document synthesizes their findings into **11 architectural decisions** (including 1 ADR and 1 constraint) and **2 explicitly deferred decisions**.

**Key Insight**: V1.0 has established strong, consistent patterns. Gold layer should **extend these patterns**, not invent new ones.

---

## Core Architectural Decisions

### Decision 1: Config Placement

**Question**: Where should `gold_etl` configuration live?

**Options Considered**:
| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A | Embed in StreamConfig | Follows silver_etl pattern; atomic | Larger config files |
| B | Separate gold/ directory | Clear separation | Breaks single-file-per-stream |
| C | Hybrid | Flexibility | Complexity |

**RECOMMENDATION: Option A - Embed `gold_etl` in StreamConfig**

**Rationale**:
- `silver_etl: Option<SilverEtlConfig>` pattern already exists and works well
- Single YAML file per stream enables atomic validation
- GitOps friendly (one file = one stream's complete definition)
- ConfigLoader trait already supports layer-specific extraction

**Implementation**:
```rust
// In core/src/types/stream_config.rs
pub struct StreamConfig {
    pub stream_id: String,
    pub description: String,
    pub fields: Vec<FieldConfig>,
    pub sources: Vec<SourceConfig>,
    pub storage: StorageConfig,
    pub silver_etl: Option<SilverEtlConfig>,  // Existing
    pub gold_etl: Option<GoldEtlConfig>,      // NEW
}
```

**Config Example**:
```yaml
stream_id: air-quality
stream_type: observation  # NEW - for classification
description: Indoor air quality measurements
fields: [...]
sources: [...]
storage: [...]
silver_etl:
  enabled: true
  # ... existing silver config
gold_etl:                  # NEW SECTION
  enabled: true
  aggregates:
    granularities: ["1 hour", "1 day"]
    fields:
      pm25: { metrics: [mean, std, min, max, p95] }
      co2: { metrics: [mean, std, min, max] }
  features:
    lag:
      enabled: true
      lags_hours: [1, 6, 24]
      fields: [pm25, co2]
    rolling:
      enabled: true
      windows: ["4 hours", "24 hours"]
      stats: [mean, std]
      fields: [pm25]
```

---

### Decision 2: Schema Validation

**Question**: How to validate Gold layer configuration?

**Current Pattern**: Two-layer validation (ADR-019-001)
1. Layer 1: JSON Schema (structure, types, enums)
2. Layer 2: Semantic validation (cross-references, DB checks)

**RECOMMENDATION: Follow existing pattern exactly**

**New Schemas to Create**:
| Schema | Location | Purpose |
|--------|----------|---------|
| `gold-etl.schema.json` | `config/schemas/` | Aggregates, features, transitions |
| `objectives.schema.json` | `config/schemas/` | Targets, constraints |
| `alignment.schema.json` | `config/schemas/` | Cross-stream alignment |

**Integration**:
```json
// In stream-config.v2.schema.json, add:
{
  "properties": {
    "gold_etl": { "$ref": "gold-etl.schema.json#/definitions/gold_etl" }
  }
}
```

**New Semantic Validation Rules**:
| Error Code | Rule |
|------------|------|
| 400 | `InvalidGoldField` - gold_etl references field not in stream |
| 401 | `InvalidStreamType` - transitions on non-state_event stream |
| 402 | `UnknownAlignmentStream` - alignment references unknown stream |
| 403 | `InvalidAggregateMetric` - unknown metric type |

---

### Decision 3: Data Dictionary Extension

**Question**: How to extend metadata for Gold layer?

**Current Pattern**: Single `data_dictionary` schema with layer prefixes:
- Bronze: `streams`, `fields`, `sources`
- Silver: `silver_tables`, `silver_columns`, `silver_lineage`

**RECOMMENDATION: Extend existing schema with `gold_*` tables**

**New Tables**:
```sql
-- Gold layer tables
CREATE TABLE data_dictionary.gold_tables (
    table_name TEXT PRIMARY KEY,
    object_type TEXT NOT NULL,  -- 'continuous_aggregate', 'view', 'materialized_view'
    source_silver_table TEXT REFERENCES data_dictionary.silver_tables(table_name),
    bucket_interval INTERVAL,
    refresh_interval INTERVAL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE data_dictionary.gold_columns (
    table_name TEXT REFERENCES data_dictionary.gold_tables(table_name),
    column_name TEXT NOT NULL,
    data_type TEXT NOT NULL,
    feature_type TEXT,  -- 'aggregate', 'lag', 'rolling', 'trend', 'raw'
    source_expression TEXT,  -- SQL expression that generates this column
    description TEXT,
    PRIMARY KEY (table_name, column_name)
);

CREATE TABLE data_dictionary.objectives (
    objective_id TEXT PRIMARY KEY,
    description TEXT,
    targets JSONB NOT NULL,  -- Array of {stream, metric, condition, threshold}
    constraints JSONB,
    priority TEXT DEFAULT 'medium',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE data_dictionary.stream_classification (
    stream_id TEXT PRIMARY KEY REFERENCES data_dictionary.streams(stream_id),
    stream_type TEXT NOT NULL,  -- 'observation', 'state_event', 'forecast', 'dimension'
    correlation_role TEXT,  -- 'cause', 'effect', 'context', 'metadata'
    description TEXT
);
```

**Population Mechanism**: Gold ETL Interpreter generates metadata atomically with DDL (not Bash script sync).

---

### Decision 4: Crate/Module Layout

**Question**: Where should Gold layer code live?

**Current Pattern**:
- `core/src/silver/` - Silver layer types and logic
- `apps/silver-etl/` - Silver ETL orchestration and SQL generation
- Heavy dependencies stay in app crates

**RECOMMENDATION: Add `core/src/gold/` module**

**Proposed Structure**:
```
core/src/gold/
├── mod.rs              # Public API
├── config.rs           # GoldEtlConfig, AlignmentConfig, ObjectivesConfig
├── interpreter/
│   ├── mod.rs
│   ├── aggregate.rs    # Continuous aggregate SQL generation
│   ├── alignment.rs    # Aligned view SQL generation
│   └── events.rs       # Unified events view generation
├── features/
│   ├── mod.rs
│   ├── registry.rs     # Feature type registry
│   ├── lag.rs          # Lag feature generator
│   ├── rolling.rs      # Rolling stats generator
│   └── trend.rs        # Trend computation
├── events/
│   ├── mod.rs
│   ├── transitions.rs  # State transition extraction
│   └── threshold.rs    # Threshold crossing detection
└── outputs/
    └── timescale.rs    # GoldTimescaleOutput adapter
```

**Feature Gate** (in core/Cargo.toml):
```toml
[features]
default = ["bronze", "silver"]
bronze = []
silver = ["bronze"]
gold = ["silver", "timescale"]  # Gold requires Silver
```

**Future App Crate**: `apps/gold-etl/` for orchestration (following silver-etl pattern)

---

### Decision 5: SQL Generation and Execution Pattern

**Question**: How to generate and apply Gold layer SQL?

**Current Architecture** (IMPORTANT):
- **Bronze**: Parquet files on disk (raw data)
- **Silver**: TimescaleDB tables (normalized timestamps, light DQ)
- **Silver ETL**: Event-driven subscriber to event bus (NOT batch ETL)
- **Database**: TimescaleDB is the ONLY database (DuckDB is deprecated/removed)
- **Silver DDL**: Generated by Bash (`ddl-generator.sh`)

**Gold Layer Pattern**:
- Gold reads directly from Silver tables in TimescaleDB
- Gold writes to TimescaleDB (continuous aggregates, materialized views)
- No intermediate ETL engine - pure SQL transformations
- Triggered by TimescaleDB continuous aggregate refresh policies

---

#### ADR-FE001-001: Gold DDL Generation in Rust

**Status**: Accepted (2026-02-04)

**Context**:
Silver DDL generation uses Bash (`deploy/pi/ddl-generator.sh`). This works for Silver because the patterns are predictable: CREATE TABLE, indexes, hypertable, policies. The SQL is straightforward string templating.

Gold layer is significantly more complex:
- Continuous aggregates with computed expressions (`AVG()`, `STDDEV()`, `PERCENTILE_CONT()`)
- Multiple granularities generating multiple views per stream
- Feature computations (lag, rolling windows, trends)
- Domain-aligned views joining multiple streams with configurable join strategies
- Expression validation (does this column exist? is this metric valid?)

Bash string manipulation cannot safely handle this complexity. Testing is difficult, escaping is error-prone, and debugging nested heredocs is painful.

**Decision**:
Gold DDL generation will be a **Rust CLI tool** (`ndp-gold-ddl`), called from `deploy.sh`.

**Consequences**:

| Aspect | Impact |
|--------|--------|
| **Silver DDL** | Unchanged - stays in Bash (don't fix what works) |
| **Gold DDL** | New Rust tool in `tools/ndp-gold-ddl/` |
| **deploy.sh** | Remains orchestrator; calls Rust tool |
| **Testing** | Unit tests for every SQL generation pattern |
| **Type Safety** | Rust validates config → SQL transformation |
| **Compilation** | Must cross-compile for Pi (existing pattern) |

**Implementation**:

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

**Integration with deploy.sh**:

```bash
# In deploy.sh
handle_gold_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Gold Table: $stream_id (action=$action)"

    case "$action" in
        sync)
            # Rust tool generates validated SQL
            local ddl=$(ndp-gold-ddl generate --stream "$stream_id" --mode full 2>&1)
            if [ $? -ne 0 ]; then
                error "Gold DDL generation failed: $ddl"
                return 1
            fi
            log "  Applying Gold DDL to TimescaleDB..."
            echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
            ;;
        validate-only)
            ndp-gold-ddl validate --stream "$stream_id"
            ;;
    esac
}

handle_domain() {
    local declaration="$1"
    local domain_id=$(echo "$declaration" | jq -r '.domain_id')

    log "Domain: $domain_id"

    # Generate aligned view and unified events for domain
    local ddl=$(ndp-gold-ddl generate --domain "$domain_id" 2>&1)
    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
}
```

**Idempotency Patterns** (same as Silver):
- `CREATE MATERIALIZED VIEW ... IF NOT EXISTS` (note: requires TimescaleDB 2.x)
- `DO $$ IF NOT EXISTS ... $$` for policies
- Check for existing objects before CREATE

---

---

### Decision 6: Domain-Centric Configuration

**Question**: Where should cross-stream configuration (alignment, objectives) live?

**Key Insight**: Streams are domain-agnostic building blocks. Domains are where intelligence/analytics happens.

**DECISION: Domain-centric configuration in `config/domains/`**

**Rationale**:
- Streams remain reusable (same stream can serve multiple domains)
- Domains are self-contained (alignment + objectives together)
- Preserves flexibility: can create one super-wide domain if needed
- Cannot easily go the other way (platform-wide → domain-centric is hard)
- Matches FEATURE-ROADMAP.md mental model exactly

**Config Structure**:
```
config/
├── base/streams/                    # Data building blocks (domain-agnostic)
│   ├── air-quality/config.yaml      # Includes gold_etl section
│   ├── outdoor-weather/config.yaml
│   └── home-assistant-state/config.yaml
│
└── domains/                         # Intelligence contexts
    ├── indoor-air-quality/
    │   └── domain.yaml              # Streams, alignment, objectives
    └── energy-efficiency/           # Future domain
        └── domain.yaml
```

**Domain Config Example** (`config/domains/indoor-air-quality/domain.yaml`):
```yaml
domain:
  id: indoor-air-quality
  description: "Maintain healthy indoor air quality"

  # Which streams this domain uses
  streams:
    - stream_id: air-quality
      alias: indoor
      role: primary           # What we're optimizing
    - stream_id: outdoor-weather
      alias: outdoor
      role: context
    - stream_id: home-assistant-state
      alias: state
      role: actuator          # Potential causes/actions
    - stream_id: outdoor-air-quality
      alias: outdoor_aqi
      role: constraint

  # Domain-specific aligned view
  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
    null_handling: preserve

  # What we're trying to achieve
  objectives:
    - id: healthy_co2
      target:
        stream: air-quality
        metric: co2
        condition: "<"
        threshold: 800
        unit: ppm
      priority: high

    - id: healthy_pm25
      target:
        stream: air-quality
        metric: pm25
        condition: "<"
        threshold: 12
        unit: µg/m³
      priority: high

  # When NOT to take action
  constraints:
    - id: outdoor_air_safe
      description: "Don't open window if outdoor air is bad"
      stream: outdoor-air-quality
      metric: pm25
      condition: "<"
      threshold: 35
```

---

### Decision 8: Forecast Streams Align on Issued Time, Not Valid Time

**Status**: Accepted (2026-02-04)

**Context**: Forecast streams (e.g., NWS weather forecasts) have two timestamps:
- `issued_at`: When the forecast was published/available
- `valid_time`: The future hour being predicted

When joining forecasts with observations for correlation analysis, the join key matters for causality.

**The Problem**:
```
NWS Forecast issued at 10:00 AM:
  - valid_time=14:00: temp=75°F
  - valid_time=15:00: temp=78°F

Observation at 14:00:
  - indoor CO2 = 850 ppm
  - user opened window at 14:15
```

**Wrong**: Join `forecast.valid_time = observation.time`
- Shows "what was predicted FOR 14:00"
- But that prediction was made hours earlier
- Cannot establish causality - user couldn't act on future information

**Correct**: Join `forecast.issued_at <= observation.time` (most recent)
- Shows "what forecast was AVAILABLE at 14:00"
- This is the information the user could have seen when making decisions
- Preserves causal validity for V1.2 correlation analysis

**Decision**: All `forecast` type streams align on `issued_at`, not `valid_time`.

**Implementation**:
```sql
-- In aligned view, forecast columns show "latest available forecast"
LEFT JOIN LATERAL (
    SELECT * FROM gold.nws_forecast_hourly f
    WHERE f.issued_at <= bucket
    ORDER BY f.issued_at DESC
    LIMIT 1
) forecast ON TRUE
```

**Applies To**: Any stream with `stream_type: forecast`
- NWS hourly forecasts
- Any future prediction data sources
- Model predictions (when added in V1.3+)

**Rationale**: Correlation analysis requires causal validity. You can only correlate observations with information that was *available* at the time, not information about that time that was generated earlier or later.

---

### Decision 10: NULL Handling in Aligned View by Stream Type

**Status**: Accepted (2026-02-04)

**Context**: FULL OUTER JOIN across streams produces NULLs where a stream has no data for a given hour. The handling strategy affects correlation validity.

**Decision**: NULL handling depends on `stream_type`:

| Stream Type | Strategy | Rationale |
|-------------|----------|-----------|
| `observation` | **Preserve NULL** | Missing sensor reading ≠ zero. Don't fabricate data. |
| `state_event` | **Carry Forward (LOCF)** | State persists until changed. Last known state IS current state. |
| `forecast` | **Preserve NULL** | If no forecast available, don't pretend there was one. |
| `dimension` | **Carry Forward** | Dimensions are slow-changing. Last value remains valid. |

**Implementation**:
```sql
SELECT
    bucket,

    -- Observations: preserve NULL (honest representation)
    aq.pm25_mean AS indoor_pm25,
    aq.co2_mean AS indoor_co2,
    ow.temp_mean AS outdoor_temp,

    -- State: carry forward (state persists until changed)
    COALESCE(
        se.window_state,
        LAG(se.window_state) IGNORE NULLS OVER (ORDER BY bucket)
    ) AS window_state

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se ON aq.bucket = se.bucket
```

**V1.2 Implications**:
- Correlation algorithms must be NULL-aware (skip NULL pairs, don't fail)
- Report "coverage %" to indicate how much overlapping data existed
- Granger causality and similar tests handle missing data natively

**Why Not Interpolate?**: Linear interpolation fabricates data points. For causal analysis, it's better to know "data was missing" than to use synthetic values that could create false correlations.

---

### Decision 9: Gold Schema Evolution Requires DROP/RECREATE

**Status**: Accepted (2026-02-04) - Constraint, not choice

**Context**: TimescaleDB continuous aggregates cannot have columns added via `ALTER TABLE ADD COLUMN`. This is a platform limitation.

**Constraint**: Adding a new metric to `gold_etl.aggregates.fields` requires:
1. DROP the existing continuous aggregate
2. CREATE new continuous aggregate with updated columns
3. Wait for refresh to repopulate data

**Implications**:
- Config changes that add Gold metrics are **breaking changes**
- Historical data will be recomputed on next refresh (not lost, just reprocessed)
- Refresh policies will repopulate from Silver (source of truth)

**Not a Decision**: This is a known limitation to document, not a choice between options.

---

### Decision 11: Idempotency via Manifest-Declared Actions

**Status**: Accepted (2026-02-04)

**Context**: Continuous aggregates need idempotent deployment. `CREATE MATERIALIZED VIEW` fails if view exists. Detection of "what changed" could happen at deploy time or manifest creation time.

**Decision**: Manifest explicitly declares `action` for each Gold table. Detection happens at manifest creation, not deploy time.

**Manifest Actions**:

| Action | When to Use | What `ndp-gold-ddl` Generates |
|--------|-------------|-------------------------------|
| `sync` | First deploy, or no config changes | Check exists → CREATE if not |
| `recreate` | `gold_etl` config changed (new metrics, granularities) | DROP IF EXISTS → CREATE |

**Manifest Example**:
```json
{
  "gold-tables": [
    { "stream_id": "air-quality", "action": "sync" },
    { "stream_id": "outdoor-weather", "action": "recreate" }
  ]
}
```

**`ndp-gold-ddl` Implementation**:

```bash
# sync mode - idempotent create
ndp-gold-ddl generate --stream air-quality --action sync

# recreate mode - DROP + CREATE
ndp-gold-ddl generate --stream outdoor-weather --action recreate
```

**Generated SQL for `sync`**:
```sql
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'air_quality_hourly'
    ) THEN
        CREATE MATERIALIZED VIEW gold.air_quality_hourly
        WITH (timescaledb.continuous) AS ...;
    ELSE
        RAISE NOTICE 'gold.air_quality_hourly already exists, skipping';
    END IF;
END $$;
```

**Generated SQL for `recreate`**:
```sql
-- Explicitly drop and recreate
DROP MATERIALIZED VIEW IF EXISTS gold.outdoor_weather_hourly CASCADE;

CREATE MATERIALIZED VIEW gold.outdoor_weather_hourly
WITH (timescaledb.continuous) AS ...;

-- Re-add policies (dropped with CASCADE)
SELECT add_continuous_aggregate_policy(...);
```

**⚠️ PROCEDURAL REQUIREMENT**:

> **ANY change to `gold_etl` config requires `action: recreate` in the manifest.**
>
> Unlike Silver tables (which support `ADD COLUMN`), Gold continuous aggregates cannot be altered in place. If you change metrics, granularities, or any `gold_etl` field, you MUST use `recreate`. Using `sync` with changed config will result in the old schema remaining in place.

**Action Selection Guide**:

| Scenario | Action | Why |
|----------|--------|-----|
| First deploy of Gold for stream | `sync` | Creates new aggregate |
| Re-deploy, `gold_etl` unchanged | `sync` | Idempotent, skips if exists |
| **ANY change to `gold_etl`** | **`recreate`** | **Required - cannot alter in place** |
| Remove Gold from stream | `drop` (future) | Explicit removal |

**Future Process** (automated):
- Compare new config vs etcd (deployed state)
- Auto-set action based on diff
- Automation will enforce this rule

**Rationale**: Manifest is explicit about intent. No runtime detection needed. deploy.sh executes what manifest declares. Safer and more predictable.

---

### Decision 7: Aligned Views Are Domain-Scoped

**Question**: One platform-wide aligned view or one per domain?

**DECISION: One aligned view per domain**

**Rationale**:
- Domain is self-contained unit
- Preserves flexibility (can create one super-wide domain if needed)
- Designing for domain-centric allows platform-wide as special case
- Designing for platform-wide prevents easy domain separation

**Performance Note**: If materialized view performance becomes critical on Pi, user can choose to consolidate into single domain. Architecture supports both patterns.

**What This Means**:
| Artifact | Scope | Example |
|----------|-------|---------|
| `gold.{stream}_hourly` | Per-stream | `gold.air_quality_hourly` |
| `gold.{domain}_aligned` | Per-domain | `gold.indoor_air_quality_aligned` |
| `gold.{domain}_events` | Per-domain | Unified events for domain |
| Threshold crossings | Per-domain | Derived from domain objectives |

---

## Decisions Summary Table

| # | Decision | Choice | Pattern Source |
|---|----------|--------|----------------|
| 1 | Per-stream Gold config | Embed `gold_etl` in StreamConfig | Follows silver_etl |
| 2 | Schema validation | Two-layer (JSON Schema + semantic) | ADR-019-001 |
| 3 | Data dictionary | Extend with `gold_*` tables | Follows silver pattern |
| 4 | Module layout | `core/src/gold/` with feature gate | Follows silver pattern |
| 5 | Gold DDL generation | **Rust CLI tool** (`ndp-gold-ddl`) | ADR-FE001-001 |
| 6 | Cross-stream config | **Domain-centric** in `config/domains/` | Flexibility principle |
| 7 | Aligned views | **One per domain** (not platform-wide) | Preserves optionality |
| 8 | Forecast alignment | Join on `issued_at`, not `valid_time` | Causal validity |
| 9 | Gold schema evolution | DROP/RECREATE (constraint) | TimescaleDB limitation |
| 10 | NULL handling | By stream_type (preserve/carry forward) | Causal validity |
| 11 | Idempotency | Manifest declares `sync` vs `recreate` | Explicit intent |
| D1 | Threshold crossing dedupe | **Deferred** - observe behavior first | Premature optimization |
| D2 | Backfill strategy | **Deferred** - Bronze→Silver concern | Architecture layers |

### Config Location Summary

| Config Type | Location | Rationale |
|-------------|----------|-----------|
| Stream aggregates/features | `config/base/streams/{id}/config.yaml` → gold_etl | Fields are right there |
| Domain alignment | `config/domains/{name}/domain.yaml` | Cross-stream |
| Domain objectives | `config/domains/{name}/domain.yaml` | Cross-stream |
| Constraints | `config/domains/{name}/domain.yaml` | Domain-scoped |

---

## Open Questions Requiring Further Discussion

### Q1: Statistical Computations - Rust vs TimescaleDB?

**Context**: Silver data sits in TimescaleDB with normalized timestamps and light DQ applied. Gold layer needs to compute aggregates, features, and eventually correlations.

**The Real Question**: Where should statistical calculations happen?

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A: TimescaleDB (SQL)** | Continuous aggregates, window functions | Native; automatic refresh; no data movement | Limited to SQL expressiveness |
| **B: Rust** | Pull from Silver, compute, write to Gold | Full algorithmic power; testable | Data movement; memory overhead |
| **C: Hybrid** | Simple stats in SQL, complex in Rust | Best of both | Two code paths |

**Recommendation**:

| Computation Type | Where | Rationale |
|------------------|-------|-----------|
| Aggregates (mean, std, min, max) | TimescaleDB | Native continuous aggregates |
| Lag features | TimescaleDB | Window functions are efficient |
| Rolling stats | TimescaleDB | Window functions |
| Trend (slope) | TimescaleDB | Approximation via window functions |
| Granger causality (V1.2) | Rust | Complex algorithm, not SQL-expressible |
| Anomaly detection (V1.2) | Rust | Statistical algorithms |

**V1.1**: Use TimescaleDB for all computations. Continuous aggregates handle the heavy lifting.
**V1.2+**: Add Rust computation layer for pattern detection algorithms.

### Q2: Gold Layer Trigger Mechanism?

**Context**: Silver ETL is event-driven (subscribes to event bus). How should Gold layer be triggered?

**Options**:
| Option | Description |
|--------|-------------|
| A | **Continuous Aggregates** | TimescaleDB auto-refreshes on policy schedule (e.g., every 15 min) |
| B | **Event subscription** | Gold subscribes to Silver write events |
| C | **Scheduled job** | Cron-like refresh (e.g., hourly) |

**Recommendation**: Option A for V1.1. Continuous aggregates have built-in refresh policies. No additional orchestration needed.

```sql
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

### Q3: Trend Computation Method?

**Context**: Linear regression slope in SQL is expensive. Pi 5 has limited resources.

**Options**:
| Option | Method | Performance |
|--------|--------|-------------|
| A | SQL window function | Simple approximation using (last - first) / window |
| B | Rust computation | Accurate linear regression |
| C | Defer to V1.2 | Simplify V1.1 scope |

**Recommendation**: Option A for V1.1 with simple slope approximation:

```sql
-- Simple trend: (last value - first value) / window hours
(LAST(co2, observation_time) - FIRST(co2, observation_time)) / 4.0 AS co2_trend_4h
```

Revisit accuracy needs in V1.2 when pattern detection requires it.

---

## Architecture Patterns Stored

The following patterns were stored in AgentDB for future reference:

| ID | Pattern | Tags |
|----|---------|------|
| 20 | `architecture:gold-layer-schema-design` | fe-001, gold-layer |
| 21 | `architecture:gold-crate-structure` | fe-001, gold-layer, crate-structure |
| 22 | `architecture:gold-data-dictionary` | fe-001, gold-layer, data-dictionary |
| 23 | `architecture:gold-etl-config-placement` | fe-001, gold-layer |
| 24 | `architecture:config-loader-extension` | fe-001, gold-layer |
| 25 | `architecture:gold-etl-config-structure` | fe-001, gold-layer |
| 26 | `architecture:gold-sql-generation` | fe-001, gold-layer, sql-generation |

---

## CRITICAL: Complete Config Lifecycle

> **Research Date:** 2026-02-04
> **Purpose:** Document the complete config flow that Gold layer must follow

Gold layer configuration MUST follow the exact same patterns as existing V1.0 infrastructure. Here's the complete lifecycle:

### The Config Lifecycle (6 Stages)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONFIG LIFECYCLE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. CONFIG FILES                          2. SYNC TO ETCD                   │
│  ┌─────────────────────┐                  ┌─────────────────────┐           │
│  │ config/base/streams │  ──────────────► │ scripts/sync-       │           │
│  │   /air-quality/     │     JSON blob    │ streams-to-etcd.sh  │           │
│  │     config.json     │                  │                     │           │
│  │                     │                  │ Key: /streams/      │           │
│  │ config/domains/     │  ──────────────► │   {stream_id}/      │           │
│  │   /indoor-air/      │    (NEW)         │   config            │           │
│  │     domain.yaml     │                  └─────────────────────┘           │
│  └─────────────────────┘                            │                       │
│           │                                         │                       │
│           ▼                                         ▼                       │
│  3. VALIDATION (Rust)                     4. RUNTIME LOADING                │
│  ┌─────────────────────┐                  ┌─────────────────────┐           │
│  │ tools/ndp-validate  │                  │ config-client       │           │
│  │                     │                  │                     │           │
│  │ Layer 1: JSON Schema│                  │ StreamRegistry      │           │
│  │ Layer 2: Semantic   │                  │  .load_stream()     │           │
│  │                     │                  │  .load_all_streams()│           │
│  │ New error codes:    │                  │                     │           │
│  │  400-403 for Gold   │                  │ Deserialize to Rust │           │
│  └─────────────────────┘                  │ StreamConfig struct │           │
│           │                               └─────────────────────┘           │
│           ▼                                         │                       │
│  5. DDL GENERATION                                  │                       │
│  ┌─────────────────────┐                            │                       │
│  │ deploy/pi/          │                            │                       │
│  │   ddl-generator.sh  │ ◄──────────────────────────┘                       │
│  │                     │     reads config                                   │
│  │ generate_silver_ddl │                                                    │
│  │ generate_gold_ddl   │ (NEW)                                              │
│  │                     │                                                    │
│  │ Generates:          │                                                    │
│  │  - CREATE TABLE     │                                                    │
│  │  - CREATE INDEX     │                                                    │
│  │  - Hypertable setup │                                                    │
│  │  - Policies         │                                                    │
│  │  - Continuous agg   │ (NEW for Gold)                                     │
│  └─────────────────────┘                                                    │
│           │                                                                 │
│           ▼                                                                 │
│  6. DEPLOYMENT                                                              │
│  ┌─────────────────────┐                                                    │
│  │ deploy/pi/deploy.sh │                                                    │
│  │                     │                                                    │
│  │ apply() function:   │                                                    │
│  │  - handle_silver_table()                                                 │
│  │  - handle_gold_table()  (NEW)                                            │
│  │                     │                                                    │
│  │ Pipes DDL to:       │                                                    │
│  │  timescaledb psql   │                                                    │
│  └─────────────────────┘                                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### What Gold Layer Must Implement

| Stage | Existing | Gold Layer Extension |
|-------|----------|----------------------|
| **1. Config Files** | `config/base/streams/{id}/config.json` | Add `gold_etl` section to stream config; NEW `config/domains/{id}/domain.yaml` |
| **2. Sync to etcd** | `scripts/sync-streams-to-etcd.sh` | May need `sync-domains-to-etcd.sh` for domain configs |
| **3. Validation** | `tools/ndp-validate` | Extend JSON schema for `gold_etl`; Add semantic rules 400-403 |
| **4. Runtime Loading** | `config-client/src/stream/registry.rs` | Extend `StreamConfig` struct with `gold_etl: Option<GoldEtlConfig>` |
| **5. DDL Generation** | `deploy/pi/ddl-generator.sh` (Bash) | **NEW Rust tool**: `tools/ndp-gold-ddl/` (ADR-FE001-001) |
| **6. Deployment** | `deploy/pi/deploy.sh` → `handle_silver_table()` | Add `handle_gold_table()`, `handle_domain()` calling Rust tool |

### Key Files to Modify/Create

**Config Files (Phase A):**
```
config/
├── base/streams/air-quality/config.json    # ADD gold_etl section
├── domains/                                 # NEW directory
│   └── indoor-air-quality/
│       └── domain.yaml                     # NEW domain config
└── schemas/
    ├── gold-etl.schema.json                # NEW
    ├── domain.schema.json                  # NEW
    └── stream-config.v2.schema.json        # EXTEND with gold_etl
```

**Sync Scripts:**
```
scripts/
├── sync-streams-to-etcd.sh                 # May need update for gold_etl in stream
└── sync-domains-to-etcd.sh                 # NEW script (follows existing pattern)
```

**Validation (Rust):**
```
tools/ndp-validate/
├── src/schema.rs                           # Extend default_stream_schema()
├── src/semantic/mod.rs                     # Add gold validation rules
└── src/semantic/gold.rs                    # NEW module for gold semantic validation
```

**Core Library (Rust):**
```
core/src/
├── types/stream_config.rs                  # ADD gold_etl: Option<GoldEtlConfig>
└── gold/                                   # NEW module
    ├── mod.rs
    └── config.rs                           # GoldEtlConfig, DomainConfig structs
```

**DDL Generation (Rust - ADR-FE001-001):**
```
tools/ndp-gold-ddl/                         # NEW Rust tool
├── Cargo.toml
├── src/
│   ├── main.rs                             # CLI: generate, validate, evolve
│   ├── generators/
│   │   ├── continuous_aggregate.rs
│   │   ├── aligned_view.rs
│   │   └── features.rs
│   └── validation/
│       └── expressions.rs

deploy/pi/
└── deploy.sh                               # ADD handle_gold_table(), handle_domain()
```

### etcd Key Patterns

| Config Type | etcd Key | Value |
|-------------|----------|-------|
| Stream Config | `/streams/{stream_id}/config` | Complete JSON blob including `gold_etl` |
| Domain Config | `/domains/{domain_id}/config` | Complete domain YAML/JSON (NEW) |

### Manifest Declaration Types

Current manifest supports:
```json
{
  "etcd-config": { "stream_id": "...", "path": "..." },
  "silver-table": { "stream_id": "...", "action": "sync" },
  "stream": { "stream_id": "...", "action": "enable" }
}
```

Gold layer adds:
```json
{
  "gold-table": { "stream_id": "...", "action": "sync" },
  "domain": { "domain_id": "...", "action": "sync" }
}
```

### Implementation Order (Phase A)

This is the correct implementation order based on dependencies:

1. **A01: Gold ETL JSON Schema** → Required before validation can work
2. **A03: Domain JSON Schema** → Required for cross-stream config
3. **A02: Update ndp-validate** → Extend schema.rs, add gold semantic rules
4. **A04: Extend StreamConfig struct** → core/src/types/stream_config.rs
5. **A05: Create ndp-gold-ddl tool** → NEW Rust tool in `tools/ndp-gold-ddl/` (ADR-FE001-001)
6. **A06: Deploy.sh handlers** → `handle_gold_table()`, `handle_domain()` calling Rust tool
7. **Sync scripts** → sync-domains-to-etcd.sh (if needed)

---

## Deferred Decisions

These questions were considered but explicitly deferred until we have more information:

### Deferred: Threshold Crossing Deduplication

**Question**: When a metric oscillates around a threshold, it generates many crossing events. Should we dedupe? Apply hysteresis?

**Risk if Unaddressed**: Event spam, noisy unified events view

**Decision**: **Deferred** - Wait until we observe the behavior in practice. Any decision now is premature guessing. If threshold crossings create noise, we'll directly observe the pattern and design an appropriate resolution.

**Revisit When**: After V1.1 Phase E (Unified Events View) is deployed and generating real threshold crossing events.

---

### Deferred: Backfill Strategy

**Question**: Pi reboots, misses 4 hours. How does Gold layer catch up?

**Risk if Unaddressed**: Data gaps in aggregates after outages

**Decision**: **Deferred** - This is primarily a Bronze→Silver concern, not Gold. The expected behavior:
1. Bronze ingests data (may have gaps during outage)
2. Silver uses upsert, should recover automatically when Bronze catches up
3. Gold continuous aggregates refresh from Silver (materialized view catchup)

**Note**: May need to validate upsert behavior with Silver subscriber model. If Silver recovery works as expected, Gold inherits that behavior automatically.

**Revisit When**: If we observe Gold data gaps after Pi reboots that aren't explained by Silver gaps.

---

## Next Steps

1. **Review these decisions** with stakeholders
2. **Resolve open questions** (Q1-Q3)
3. **Create ADRs** for significant decisions (config placement, module layout)
4. **Begin SPARC Specification** for Phase A features (v11-A01 through v11-A05)

---

## References

### Implementation Guides
- [Config Deployment Flow](./CONFIG-DEPLOYMENT-FLOW.md) - Complete 12-component, 9-phase deployment flow

### Analysis Documents
- [Config Patterns](./config-patterns.md)
- [Schema Validation Patterns](./schema-validation-patterns.md)
- [Data Dictionary Patterns](./data-dictionary-patterns.md)
- [Crate Layout Patterns](./crate-layout-patterns.md)
- [ETL Patterns](./etl-patterns.md)
