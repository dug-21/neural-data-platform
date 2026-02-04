# FE-001 Phase A: Architecture Foundation - Overview

> **Created:** 2026-02-04
> **Phase:** A (Architecture Foundation)
> **Target:** Week 1-2
> **Status:** Specification Complete

---

## Executive Summary

Phase A establishes the declarative architecture foundation that enables all subsequent V1.1 capabilities. This phase focuses on schemas, tools, and registries - the infrastructure that makes "config-only stream addition" possible.

**Exit Criteria**: JSON Schemas defined, Gold DDL tool operational, Feature Type Registry extensible.

---

## Phase A Features

| ID | Feature | Priority | Specification |
|----|---------|----------|---------------|
| v11-A01 | Gold ETL JSON Schema | Critical | [SPEC-A01](./SPEC-A01-gold-etl-schema.md) |
| v11-A02 | Gold DDL Tool (ndp-gold-ddl) | Critical | [SPEC-A02](./SPEC-A02-gold-ddl-tool.md) |
| v11-A03 | Alignment JSON Schema | Critical | [SPEC-A03](./SPEC-A03-alignment-schema.md) |
| v11-A04 | Alignment Interpreter | Critical | [SPEC-A04](./SPEC-A04-alignment-interpreter.md) |
| v11-A05 | Objectives JSON Schema | High | [SPEC-A05](./SPEC-A05-objectives-schema.md) |
| v11-A06 | Feature Type Registry | High | [SPEC-A06](./SPEC-A06-feature-registry.md) |

---

## Dependency Graph

```
                    ┌─────────────────┐
                    │ v11-A01         │
                    │ Gold ETL Schema │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
    ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
    │ v11-A02         │ │ v11-A03         │ │ v11-A05         │
    │ Gold DDL Tool   │ │ Alignment Schema│ │ Objectives      │
    │ (continuous agg)│ │                 │ │ Schema          │
    └────────┬────────┘ └────────┬────────┘ └─────────────────┘
             │                   │
             │                   ▼
             │          ┌─────────────────┐
             │          │ v11-A04         │
             │          │ Alignment       │
             │          │ Interpreter     │
             └──────────┴─────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ v11-A06         │
                    │ Feature Type    │
                    │ Registry        │
                    └─────────────────┘
```

### Dependency Details

| Feature | Depends On | Blocking For |
|---------|------------|--------------|
| v11-A01 | V1.0 schema validation | v11-A02, v11-A03, ndp-validate extension |
| v11-A02 | v11-A01 | Phase B (continuous aggregates) |
| v11-A03 | V1.0 schema validation | v11-A04 |
| v11-A04 | v11-A03, v11-A02 | Phase C (aligned views) |
| v11-A05 | V1.0 schema validation | Phase C (objectives storage) |
| v11-A06 | v11-A02 | Phase B (feature computation) |

---

## Implementation Order

Based on dependencies, the recommended implementation order is:

### Week 1: Core Schemas

1. **v11-A01: Gold ETL JSON Schema** (Day 1-2)
   - Defines the structure for per-stream Gold configuration
   - Enables validation before any DDL generation

2. **v11-A03: Alignment JSON Schema** (Day 2-3)
   - Defines domain-level cross-stream alignment
   - Standalone - can parallel v11-A02

3. **v11-A05: Objectives JSON Schema** (Day 3)
   - Defines objectives within domain config
   - Standalone - can parallel v11-A02

### Week 2: Tools and Interpreters

4. **v11-A02: Gold DDL Tool** (Day 4-7)
   - Rust CLI tool for DDL generation
   - Most complex feature - requires v11-A01 complete

5. **v11-A04: Alignment Interpreter** (Day 7-8)
   - Module within ndp-gold-ddl for aligned views
   - Requires v11-A03 complete

6. **v11-A06: Feature Type Registry** (Day 8-9)
   - Trait-based extensibility for feature generators
   - Can be developed alongside v11-A02

---

## Shared Interfaces

### Config Types (core/src/gold/config.rs)

```rust
/// Gold ETL configuration embedded in stream config
pub struct GoldEtlConfig {
    pub enabled: bool,
    pub aggregates: Option<AggregateConfig>,
    pub features: Option<FeaturesConfig>,
    pub transitions: Option<TransitionsConfig>,
}

/// Domain alignment configuration
pub struct AlignmentConfig {
    pub view_name: String,
    pub granularity: String,
    pub join_strategy: JoinStrategy,
    pub null_handling: NullHandling,
}

/// Objectives configuration
pub struct ObjectivesConfig {
    pub objectives: Vec<Objective>,
    pub constraints: Vec<Constraint>,
}
```

### Validation Error Codes (400-499 reserved for Gold)

| Code | Name | Description |
|------|------|-------------|
| 400 | InvalidGoldField | gold_etl references field not in stream |
| 401 | InvalidStreamType | transitions config on non-state_event stream |
| 402 | UnknownAlignmentStream | alignment references unknown stream |
| 403 | InvalidAggregateMetric | unknown metric type in aggregates |
| 404 | InvalidDomainStream | domain references non-existent stream |
| 405 | InvalidFeatureType | unknown feature type in features config |
| 406 | InvalidGranularity | granularity format not recognized |
| 407 | CircularDomainDependency | domain references itself |
| 408 | InvalidObjectiveCondition | objective condition not supported |

---

## Test Strategy

### Unit Tests (per feature)

Each feature specification includes integration test requirements. Summary:

| Feature | Test File | Key Test Cases |
|---------|-----------|----------------|
| v11-A01 | `schema_validation_test.rs` | Valid config passes, invalid rejects with helpful errors |
| v11-A02 | `continuous_aggregate_test.rs` | SQL generation, idempotency, policy generation |
| v11-A03 | `alignment_schema_test.rs` | Domain config validation, stream references |
| v11-A04 | `aligned_view_test.rs` | JOIN generation, NULL handling, forecast alignment |
| v11-A05 | `objectives_schema_test.rs` | Condition types, threshold validation |
| v11-A06 | `feature_registry_test.rs` | Registration, lookup, trait implementation |

### Integration Tests

| Test | Description | Features Covered |
|------|-------------|------------------|
| `end_to_end_gold_deploy.rs` | Deploy Gold layer from config | A01, A02, A06 |
| `domain_aligned_view.rs` | Generate and execute aligned view | A03, A04 |
| `validation_pipeline.rs` | Full validation of Gold configs | A01, A03, A05 |

---

## File Inventory

### New Files (Phase A creates)

```
config/schemas/
├── gold-etl.schema.json          # v11-A01
├── domain.schema.json            # v11-A03 + v11-A05 (combined)
└── alignment.schema.json         # v11-A03 (referenced by domain.schema)

tools/ndp-gold-ddl/               # v11-A02
├── Cargo.toml
├── src/
│   ├── main.rs                   # CLI entry point
│   ├── lib.rs                    # Library for testing
│   ├── generators/
│   │   ├── mod.rs
│   │   ├── continuous_aggregate.rs
│   │   ├── aligned_view.rs       # v11-A04
│   │   ├── features.rs
│   │   └── events.rs
│   ├── registry/                 # v11-A06
│   │   ├── mod.rs
│   │   └── feature_types.rs
│   └── validation/
│       ├── mod.rs
│       └── expressions.rs
└── tests/
    ├── continuous_aggregate_test.rs
    ├── aligned_view_test.rs
    └── feature_registry_test.rs

core/src/gold/                    # Shared types
├── mod.rs
└── config.rs

tools/ndp-validate/src/semantic/
├── gold.rs                       # Gold-specific validation
└── domain.rs                     # Domain validation
```

### Modified Files (Phase A extends)

```
config/base/streams/*/config.json  # Add gold_etl section
config/domains/*/domain.yaml       # NEW directory, domain configs
deploy/pi/deploy.sh                # Add handle_gold_table(), handle_domain()
tools/ndp-validate/src/error.rs    # Add error codes 400-408
tools/ndp-validate/src/semantic/mod.rs  # Import gold.rs, domain.rs
core/src/types/stream_config.rs    # Add gold_etl: Option<GoldEtlConfig>
```

---

## Exit Criteria Checklist

### Architecture Foundation Complete When:

- [ ] **v11-A01**: gold-etl.schema.json validates example configs
- [ ] **v11-A02**: `ndp-gold-ddl generate --stream air-quality` produces valid SQL
- [ ] **v11-A03**: domain.schema.json validates indoor-air-quality domain
- [ ] **v11-A04**: `ndp-gold-ddl generate --domain indoor-air-quality` produces aligned view SQL
- [ ] **v11-A05**: Objectives section validates within domain config
- [ ] **v11-A06**: New feature type can be added via trait implementation

### Architecture Review Checklist:

- [ ] All schemas integrated with ndp-validate two-layer validation
- [ ] Error codes 400-408 implemented with helpful messages
- [ ] Gold DDL tool called from deploy.sh handlers
- [ ] Feature registry supports lag, rolling, trend feature types
- [ ] All unit tests passing
- [ ] Integration test demonstrates end-to-end flow

---

## References

- [SCOPE.md](../../SCOPE.md) - Full V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Complete deployment flow
- [ADR-FE001-001](../../architecture/DECISIONS.md#adr-fe001-001-gold-ddl-generation-in-rust) - Gold DDL in Rust decision
