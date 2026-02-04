# FE-001 Phase A: Algorithm Overview

> **Created:** 2026-02-04
> **Phase:** A (Architecture Foundation)
> **Status:** Pseudocode Complete

---

## Executive Summary

This document provides an overview of the algorithmic designs for Phase A of the Gold Layer Feature Engineering initiative. These algorithms define the core logic that transforms declarative configuration into executable DDL.

**Design Principles Applied:**
1. **London TDD** - All interfaces designed for testability with dependency injection
2. **Config-Driven** - Algorithms consume configuration, not hardcoded values
3. **Idempotent** - All DDL generation supports sync/recreate actions
4. **SQL-First** - Computations happen in TimescaleDB, not Rust

---

## Algorithm Index

| ID | Algorithm | Purpose | Document |
|----|-----------|---------|----------|
| A01 | Gold DDL CLI | Command routing and orchestration | [ALGO-gold-ddl-cli.md](./ALGO-gold-ddl-cli.md) |
| A02 | Continuous Aggregate Generator | Per-stream hourly/daily aggregate SQL | [ALGO-continuous-aggregate.md](./ALGO-continuous-aggregate.md) |
| A03 | Alignment Interpreter | Cross-stream JOIN and aligned view SQL | [ALGO-alignment-interpreter.md](./ALGO-alignment-interpreter.md) |
| A04 | Feature Registry | Extensible feature type system | [ALGO-feature-registry.md](./ALGO-feature-registry.md) |

---

## Dependency Graph

```
                    ┌───────────────────────┐
                    │    Gold DDL CLI       │
                    │      (A01)            │
                    └───────────┬───────────┘
                                │
            ┌───────────────────┼───────────────────┐
            │                   │                   │
            ▼                   ▼                   ▼
┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐
│   Continuous      │  │    Alignment      │  │    Feature        │
│   Aggregate Gen   │  │    Interpreter    │  │    Registry       │
│      (A02)        │  │      (A03)        │  │      (A04)        │
└─────────┬─────────┘  └─────────┬─────────┘  └─────────┬─────────┘
          │                      │                      │
          │                      │                      │
          ▼                      ▼                      ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SQL Output Generator                         │
│           (Shared formatter for all generated DDL)               │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Data Types

These types are shared across all algorithms. Defined in `core/src/gold/config.rs`.

### GoldEtlConfig

```
STRUCT GoldEtlConfig:
    enabled: boolean
    aggregates: Option<AggregateConfig>
    features: Option<FeaturesConfig>
    transitions: Option<TransitionsConfig>
```

### AggregateConfig

```
STRUCT AggregateConfig:
    granularities: List<String>         // ["1 hour", "1 day"]
    entity_column: String               // "ndp_id"
    fields: Map<String, FieldMetrics>   // {"pm25": {metrics: [mean, std]}}
    refresh_interval: String            // "15 minutes"
    start_offset: String                // "4 hours"
    end_offset: String                  // "15 minutes"
```

### DomainConfig

```
STRUCT DomainConfig:
    id: String
    description: String
    streams: List<StreamReference>
    alignment: AlignmentConfig
    objectives: List<Objective>
    constraints: List<Constraint>
```

### AlignmentConfig

```
STRUCT AlignmentConfig:
    view_name: String
    granularity: String
    join_strategy: JoinStrategy         // full_outer | left | inner
    null_handling: NullHandling         // preserve | carry_forward | interpolate
```

---

## Shared Traits (London TDD)

### ConfigLoader Trait

```
TRAIT ConfigLoader:
    METHOD load_stream_config(stream_id: String) -> Result<StreamConfig, ConfigError>
    METHOD load_domain_config(domain_id: String) -> Result<DomainConfig, ConfigError>
    METHOD load_all_streams() -> Result<Map<String, StreamConfig>, ConfigError>
```

**Implementations:**
- `FileSystemConfigLoader` - Production: reads from config/base/streams/
- `EtcdConfigLoader` - Production: reads from etcd endpoint
- `MockConfigLoader` - Test: returns predefined configs

### SqlOutputWriter Trait

```
TRAIT SqlOutputWriter:
    METHOD write_comment(comment: String) -> Result<(), IoError>
    METHOD write_statement(sql: String) -> Result<(), IoError>
    METHOD write_blank_line() -> Result<(), IoError>
    METHOD finish() -> Result<String, IoError>
```

**Implementations:**
- `StdoutWriter` - Production: writes to stdout
- `StringWriter` - Test: collects to String for assertion

### DdlGenerator Trait

```
TRAIT DdlGenerator:
    METHOD generate(config: &GeneratorInput, action: Action) -> Result<String, GeneratorError>
    METHOD validate(config: &GeneratorInput) -> Result<(), ValidationError>
```

**Implementations:**
- `ContinuousAggregateGenerator`
- `AlignedViewGenerator`
- `FeatureDdlGenerator`

---

## Action Semantics

The CLI supports two actions for idempotent deployment:

| Action | When to Use | SQL Pattern |
|--------|-------------|-------------|
| `sync` | First deploy, no config changes | `IF NOT EXISTS` check, skip if exists |
| `recreate` | Any change to gold_etl config | `DROP ... CASCADE` then `CREATE` |

**Key Constraint**: TimescaleDB continuous aggregates cannot be altered. Adding a new metric REQUIRES recreate.

---

## Error Codes

Gold layer validation uses error codes 400-408:

| Code | Name | Algorithm | Description |
|------|------|-----------|-------------|
| 400 | InvalidGoldField | A02 | gold_etl references field not in stream |
| 401 | InvalidStreamType | A02 | transitions config on non-state_event stream |
| 402 | UnknownAlignmentStream | A03 | alignment references unknown stream |
| 403 | InvalidAggregateMetric | A02 | unknown metric type in aggregates |
| 404 | InvalidDomainStream | A03 | domain references non-existent stream |
| 405 | InvalidFeatureType | A04 | unknown feature type in features config |
| 406 | InvalidGranularity | A02 | granularity format not recognized |
| 407 | CircularDomainDependency | A03 | domain references itself |
| 408 | InvalidObjectiveCondition | A03 | objective condition not supported |

---

## SQL Templates

### Continuous Aggregate Template (A02)

```sql
CREATE MATERIALIZED VIEW gold.{stream_id}_{granularity_suffix}
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('{granularity}', {timestamp_column}) AS bucket,
    {entity_column},
    {aggregate_expressions}
FROM silver.{source_table}
GROUP BY bucket, {entity_column};
```

### Refresh Policy Template (A02)

```sql
SELECT add_continuous_aggregate_policy('gold.{view_name}',
    start_offset => INTERVAL '{start_offset}',
    end_offset => INTERVAL '{end_offset}',
    schedule_interval => INTERVAL '{refresh_interval}'
);
```

### Aligned View Template (A03)

```sql
CREATE MATERIALIZED VIEW gold.{view_name} AS
SELECT
    COALESCE({bucket_list}) AS bucket,
    {column_expressions}
FROM gold.{primary_stream}_{granularity_suffix} {primary_alias}
{join_clauses}
WHERE COALESCE({bucket_list}) >= NOW() - INTERVAL '90 days';
```

---

## Complexity Analysis Summary

| Algorithm | Time Complexity | Space Complexity | Notes |
|-----------|-----------------|------------------|-------|
| CLI Command Router | O(1) | O(1) | Direct dispatch |
| Config Loading | O(n) | O(n) | n = config size |
| Aggregate SQL Gen | O(f * m) | O(f * m) | f = fields, m = metrics |
| Aligned View Gen | O(s * c) | O(s * c) | s = streams, c = columns |
| Feature Generation | O(f * t) | O(f * t) | f = fields, t = feature types |
| Validation | O(f + r) | O(1) | f = fields, r = rules |

All algorithms complete in < 500ms for typical configurations.

---

## References

- [SPEC-A02](../specification/SPEC-A02-gold-ddl-tool.md) - Gold DDL Tool specification
- [SPEC-A04](../specification/SPEC-A04-alignment-interpreter.md) - Alignment Interpreter specification
- [SPEC-A06](../specification/SPEC-A06-feature-registry.md) - Feature Registry specification
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [PATTERN-RESEARCH.md](../../architecture/PATTERN-RESEARCH.md) - Existing NDP patterns
