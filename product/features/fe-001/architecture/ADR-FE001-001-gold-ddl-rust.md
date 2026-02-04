# ADR-FE001-001: Gold DDL Generation in Rust

**Status**: Accepted
**Date**: 2026-02-04
**Decision Makers**: NDP Architecture Team
**Feature**: FE-001 Gold Layer Foundation
**Parent ADRs**: ADR-016-002 (Declarative Deploy), ADR-019-001 (Two-Layer Validation)

---

## Context

### The Problem

Silver DDL generation currently uses Bash (`deploy/pi/ddl-generator.sh`). This works for Silver because the patterns are predictable: CREATE TABLE, indexes, hypertable conversion, retention policies. The SQL is straightforward string templating.

Gold layer is significantly more complex:
- **Continuous aggregates** with computed expressions (`AVG()`, `STDDEV()`, `PERCENTILE_CONT()`)
- **Multiple granularities** generating multiple views per stream (hourly, daily)
- **Feature computations** (lag, rolling windows, trends)
- **Domain-aligned views** joining multiple streams with configurable join strategies
- **Expression validation** (does this column exist? is this metric valid?)

Bash string manipulation cannot safely handle this complexity:
- Testing DDL output is difficult in Bash
- Escaping in nested heredocs is error-prone
- Debugging nested string interpolation is painful
- No type safety for config-to-SQL transformation
- Cannot easily validate expressions before generating SQL

### Current Architecture

| Layer | DDL Generation | Reason |
|-------|----------------|--------|
| Silver | Bash (`ddl-generator.sh`) | Simple patterns, works well |
| Gold | ??? | Complex expressions, validation needed |

### Triggering Workflow

Gold DDL generation integrates with the existing declarative deployment flow:

```
Manifest declares "gold-table" -> deploy.sh -> handle_gold_table() -> ??? -> psql
```

The question is what generates the DDL between `handle_gold_table()` and `psql`.

---

## Decision

**Gold DDL generation will be a Rust CLI tool (`ndp-gold-ddl`), called from `deploy.sh`.**

### Tool Location

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

### CLI Interface

```bash
# Generate DDL for a stream's Gold layer
ndp-gold-ddl generate --stream air-quality --action sync

# Generate DDL for a domain (aligned view, unified events)
ndp-gold-ddl generate --domain indoor-air-quality

# Validate config without generating
ndp-gold-ddl validate --stream air-quality

# Schema evolution (recreate with DROP)
ndp-gold-ddl generate --stream air-quality --action recreate
```

### Integration with deploy.sh

```bash
# In deploy.sh
handle_gold_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Gold Table: $stream_id (action=$action)"

    # Rust tool generates validated SQL
    local ddl=$(ndp-gold-ddl generate --stream "$stream_id" --action "$action" 2>&1)
    if [ $? -ne 0 ]; then
        error "Gold DDL generation failed: $ddl"
        return 1
    fi
    log "  Applying Gold DDL to TimescaleDB..."
    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
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

### Idempotency Patterns

The tool generates idempotent SQL based on the `action` parameter:

**For `action: sync`** (default, first deploy or unchanged config):
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

**For `action: recreate`** (config changed, requires DROP/CREATE):
```sql
-- Explicitly drop and recreate
DROP MATERIALIZED VIEW IF EXISTS gold.outdoor_weather_hourly CASCADE;

CREATE MATERIALIZED VIEW gold.outdoor_weather_hourly
WITH (timescaledb.continuous) AS ...;

-- Re-add policies (dropped with CASCADE)
SELECT add_continuous_aggregate_policy(...);
```

---

## Consequences

### Positive

1. **Type Safety** - Rust validates config-to-SQL transformation at compile time
2. **Testability** - Unit tests verify every SQL generation pattern; impossible with Bash
3. **Expression Validation** - Can validate that metric expressions reference valid columns before generating SQL
4. **Error Messages** - Rust can provide clear, structured error messages with suggestions
5. **Cross-Platform** - Same tool works on development machines and Pi (cross-compile)
6. **Consistency** - Gold DDL generation follows established pattern (deploy.sh orchestrates, specialized tool generates)
7. **IDE Support** - Rust syntax highlighting and error checking for SQL generation code
8. **Future Features** - Easy to add schema diffing, migration scripts, dry-run mode

### Negative

1. **Compilation Required** - Must cross-compile for Pi (ARM64), adding to build process
2. **New Binary** - One more tool to maintain, version, and deploy
3. **Learning Curve** - Contributors need Rust knowledge for Gold DDL changes
4. **Startup Time** - Rust binary has ~50ms startup overhead vs Bash (negligible for deploy workflow)

### Neutral

1. **Silver DDL Unchanged** - Silver DDL stays in Bash; don't fix what works
2. **deploy.sh Unchanged** - deploy.sh remains the orchestrator; just calls a different tool
3. **Manifest Format** - Same declaration patterns as Silver (`action: sync/recreate`)

---

## Alternatives Considered

### Alternative 1: Extend Bash ddl-generator.sh

Add Gold DDL generation functions to existing Bash script.

**Rejected because:**
- Complex expressions require nested string interpolation with escaping
- Cannot unit test SQL output (would need integration tests for every pattern)
- No type checking - typos in config field names become runtime SQL errors
- Debugging nested heredocs is painful
- Bash has no JSON Schema or type validation

### Alternative 2: Python Script

Use Python with Jinja2 templates for SQL generation.

**Rejected because:**
- Adds Python runtime dependency to Pi deployment
- NDP is a Rust project; Python would be an outlier
- Would require maintaining Python templates alongside Rust config types
- No type safety between config struct and template variables

### Alternative 3: SQL Stored Procedures

Create stored procedures in TimescaleDB that generate DDL from config.

**Rejected because:**
- Moves business logic into database (violates separation of concerns)
- Harder to test and debug
- Cannot validate expressions before execution
- PostgreSQL PL/pgSQL is not suited for complex code generation

### Alternative 4: dbt (Data Build Tool)

Use dbt for Gold layer transformations.

**Rejected because:**
- Adds significant complexity (dbt server, Python runtime, dbt-specific patterns)
- Overkill for Pi deployment scenario
- NDP already has declarative deployment; dbt would duplicate functionality
- Would require learning dbt-specific Jinja macros

---

## Implementation

### Phase A05 Scope (from DECISIONS.md)

The `ndp-gold-ddl` tool is Phase A05 in the FE-001 implementation plan:

1. **A01**: Gold ETL JSON Schema - Required before tool can validate
2. **A02**: Update ndp-validate - Extend for Gold semantic rules
3. **A03**: Domain JSON Schema - Required for domain validation
4. **A04**: Extend StreamConfig struct - Required for config loading
5. **A05**: Create ndp-gold-ddl tool - This ADR
6. **A06**: Deploy.sh handlers - Integrate tool with deploy workflow

### Generator Modules

| Module | Input | Output |
|--------|-------|--------|
| `continuous_aggregate.rs` | `gold_etl.aggregates` | `CREATE MATERIALIZED VIEW ... WITH (timescaledb.continuous)` |
| `features.rs` | `gold_etl.features` | Lag, rolling, trend column expressions |
| `aligned_view.rs` | `domain.alignment` | `CREATE VIEW gold.{domain}_aligned` with JOINs |
| `events.rs` | `domain.streams`, `objectives` | State transitions, threshold crossings, unified events |
| `policies.rs` | `gold_etl.aggregates.granularities` | `add_continuous_aggregate_policy()` |

### Validation Before Generation

```rust
// In validation/expressions.rs
pub fn validate_metrics(
    config: &GoldEtlConfig,
    stream_fields: &HashSet<String>,
) -> Result<(), ValidationError> {
    for (field, metrics) in &config.aggregates.fields {
        // Check field exists in stream
        if !stream_fields.contains(field) {
            return Err(ValidationError::InvalidGoldField {
                field: field.clone(),
                available: stream_fields.iter().cloned().collect(),
            });
        }
        // Check metrics are valid
        for metric in &metrics.metrics {
            if !VALID_METRICS.contains(metric.as_str()) {
                return Err(ValidationError::InvalidAggregateMetric {
                    metric: metric.clone(),
                    field: field.clone(),
                });
            }
        }
    }
    Ok(())
}
```

---

## Related Decisions

- **Decision 5 (DECISIONS.md)**: SQL Generation and Execution Pattern - foundational decision
- **Decision 9 (DECISIONS.md)**: Gold Schema Evolution Requires DROP/RECREATE - constraint
- **Decision 11 (DECISIONS.md)**: Idempotency via Manifest-Declared Actions - action parameter design
- **ADR-016-002**: Declarative Deploy - deploy.sh orchestration pattern
- **ADR-019-001**: Two-Layer Validation - validation pattern to follow

---

## References

- `/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md` - Source decision
- `/workspaces/neural-data-platform/product/features/fe-001/architecture/CONFIG-DEPLOYMENT-FLOW.md` - Integration points
- `/workspaces/neural-data-platform/deploy/pi/ddl-generator.sh` - Current Silver DDL pattern
- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - Deployment orchestrator
- `/workspaces/neural-data-platform/tools/ndp-validate/` - Existing Rust validation tool pattern

---

*Architecture decision created: 2026-02-04*
*Feature: FE-001 Gold Layer Foundation*
