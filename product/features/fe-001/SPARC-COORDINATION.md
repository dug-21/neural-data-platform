# FE-001: Gold Layer Foundation - SPARC Coordination

> **Created:** 2026-02-04
> **Author:** ndp-scrum-master
> **Status:** Planning
> **Feature Reference:** [SCOPE.md](./SCOPE.md)
> **Architecture Reference:** [DECISIONS.md](./architecture/DECISIONS.md)

---

## Executive Summary

This document coordinates the SPARC methodology for FE-001 Gold Layer Foundation. After analyzing the scope, architecture decisions, and dependencies, I recommend **5 separate SPARC cycles** aligned with the existing phase structure. Each phase has clear boundaries, measurable exit criteria, and integration points.

**Key Finding**: The phases are well-defined and cannot be safely combined because:
1. Phase B (First Stream) validates the architecture from Phase A
2. Phase C (Cross-Stream) requires Phase B's reference implementation
3. Phase D (Validation) is specifically designed to prove Phase A-C architecture
4. Phase E (Unified Events) builds on all previous phases

---

## Phase Breakdown Analysis

### Recommendation: 5 Separate SPARC Cycles

| SPARC Cycle | FE-001 Phase | Duration | Why Separate? |
|-------------|--------------|----------|---------------|
| **SPARC-A** | Phase A: Architecture Foundation | 2 weeks | Foundation must be solid before building on it |
| **SPARC-B** | Phase B: First Stream | 1 week | Validates architecture; reference implementation |
| **SPARC-C** | Phase C: Cross-Stream + Alignment | 1 week | Introduces JOIN complexity; must be validated |
| **SPARC-D** | Phase D: Validation + Dashboard | 1 week | Critical validation; proves extensibility |
| **SPARC-E** | Phase E: Unified Event Abstraction | 1 week | New event types; V1.2 handoff |

### Rationale for Separate Cycles

1. **Risk Isolation**: Each phase introduces new complexity. Validating incrementally prevents cascading failures.

2. **Clear Exit Gates**: Each SPARC cycle has measurable completion criteria that gate the next phase.

3. **Learning Integration**: Each phase can store patterns via `reflexion` before starting the next.

4. **Deployment Cadence**: Each phase produces deployable artifacts that can be tested on Pi.

---

## SPARC Cycle Details

### SPARC-A: Architecture Foundation

**Duration**: 2 weeks (Week 1-2)
**Team**: `ndp-architect`, `ndp-rust-dev`, `ndp-tester`

#### Scope

| Feature ID | Description | SPARC Owner |
|------------|-------------|-------------|
| v11-A01 | Gold ETL JSON Schema | ndp-architect |
| v11-A02 | Gold DDL Tool (ndp-gold-ddl) | ndp-rust-dev |
| v11-A03 | Alignment JSON Schema | ndp-architect |
| v11-A05 | Objectives JSON Schema | ndp-architect |
| v11-001 | Stream Type Classification | ndp-architect |

#### SPARC Deliverables

| Phase | Deliverable | Location |
|-------|-------------|----------|
| **S** | SPECIFICATION.md | `fe-001/specification/PHASE-A-SPEC.md` |
| **P** | PSEUDOCODE.md | `fe-001/pseudocode/PHASE-A-PSEUDO.md` |
| **A** | ARCHITECTURE.md | `fe-001/architecture/PHASE-A-ARCH.md` |
| **R** | Test results, fixes | `tools/ndp-gold-ddl/tests/` |
| **C** | Deployment verification | `fe-001/completion/PHASE-A-COMPLETE.md` |

#### Exit Criteria

- [ ] `gold-etl.schema.json` validates example configs
- [ ] `domain.schema.json` validates example configs
- [ ] `objectives.schema.json` validates example configs
- [ ] `ndp-gold-ddl validate --stream air-quality` passes
- [ ] `stream_type` field added to all stream configs
- [ ] All unit tests pass
- [ ] Architecture review completed

#### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Schema complexity exceeds expectations | Medium | High | Start with minimal schema, add features iteratively |
| Rust tool takes longer than estimated | Medium | High | Focus on core generation first, defer validation |
| Integration with deploy.sh breaks | Low | Medium | Test on dev Pi frequently |

---

### SPARC-B: First Stream (air-quality)

**Duration**: 1 week (Week 3)
**Team**: `ndp-rust-dev`, `ndp-timescale-dev`, `ndp-tester`

#### Scope

| Feature ID | Description | SPARC Owner |
|------------|-------------|-------------|
| v11-002 | Classification Propagation | ndp-timescale-dev |
| v11-003 | Continuous Aggregates (air-quality) | ndp-rust-dev |
| v11-004 | Aggregate Refresh Policy | ndp-timescale-dev |
| v11-A06 | Feature Type Registry (basic) | ndp-rust-dev |
| v11-008 | Basic Feature Computation (air-quality) | ndp-rust-dev |

#### SPARC Deliverables

| Phase | Deliverable | Location |
|-------|-------------|----------|
| **S** | SPECIFICATION.md | `fe-001/specification/PHASE-B-SPEC.md` |
| **P** | PSEUDOCODE.md | `fe-001/pseudocode/PHASE-B-PSEUDO.md` |
| **A** | ARCHITECTURE.md | `fe-001/architecture/PHASE-B-ARCH.md` |
| **R** | Test results | `tools/ndp-gold-ddl/tests/air_quality_test.rs` |
| **C** | Pi deployment verification | `fe-001/completion/PHASE-B-COMPLETE.md` |

#### Exit Criteria

- [ ] `gold.air_quality_hourly` continuous aggregate exists
- [ ] Refresh policy operational (every 15 min)
- [ ] At least one feature type (lag or rolling) working
- [ ] Query < 100ms for 30-day range on Pi
- [ ] **Config-only change can modify aggregate fields** (architecture validation)
- [ ] `data_dictionary.gold_tables` populated

#### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Continuous aggregates too expensive on Pi | Medium | High | Profile refresh cost; adjust window sizes |
| Feature registry becomes over-engineered | Medium | Medium | Start with 2 feature types only |
| Config change doesn't regenerate DDL | Low | High | Explicit test for config-change-only scenario |

---

### SPARC-C: Cross-Stream + Alignment

**Duration**: 1 week (Week 4)
**Team**: `ndp-rust-dev`, `ndp-timescale-dev`, `ndp-analytics-engineer`, `ndp-tester`

#### Scope

| Feature ID | Description | SPARC Owner |
|------------|-------------|-------------|
| v11-003 | Continuous Aggregates (outdoor-weather, state-events) | ndp-rust-dev |
| v11-A04 | Alignment Interpreter | ndp-rust-dev |
| v11-005 | Cross-Stream Aligned View (3 streams) | ndp-analytics-engineer |
| v11-006 | State Transition Materializer | ndp-timescale-dev |
| v11-007 | Objectives Storage | ndp-rust-dev |

#### SPARC Deliverables

| Phase | Deliverable | Location |
|-------|-------------|----------|
| **S** | SPECIFICATION.md | `fe-001/specification/PHASE-C-SPEC.md` |
| **P** | PSEUDOCODE.md | `fe-001/pseudocode/PHASE-C-PSEUDO.md` |
| **A** | ARCHITECTURE.md | `fe-001/architecture/PHASE-C-ARCH.md` |
| **R** | Integration tests | `tools/ndp-gold-ddl/tests/integration/` |
| **C** | Pi deployment verification | `fe-001/completion/PHASE-C-COMPLETE.md` |

#### Exit Criteria

- [ ] 3 streams in Gold layer (air-quality, outdoor-weather, home-assistant-state)
- [ ] `gold.indoor_air_quality_aligned` view operational
- [ ] State transitions extractable from `gold.state_events_transitions`
- [ ] Objectives stored in etcd at `/domains/indoor-air-quality/objectives`
- [ ] NULL handling correct (observations preserve NULL, states carry forward)
- [ ] Forecast streams align on `issued_at`, not `valid_time`

#### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| FULL OUTER JOIN performance poor | Medium | High | Start with LEFT JOIN; profile; consider UNION ALL |
| Alignment gaps cause NULL rows | Medium | Medium | Document expected NULL patterns; test edge cases |
| Domain config structure unclear | Low | Medium | Use example from DECISIONS.md verbatim |

**Note**: `outdoor-air-quality` deliberately excluded (reserved for Phase D fast-follower test)

---

### SPARC-D: Validation + Dashboard

**Duration**: 1 week (Week 5)
**Team**: `ndp-rust-dev`, `ndp-grafana-dev`, `ndp-tester`

#### Scope

| Feature ID | Description | SPARC Owner |
|------------|-------------|-------------|
| v11-V01 | Fast-Follower Test (outdoor-air-quality) | ndp-tester |
| v11-009 | Lag Feature Computation | ndp-rust-dev |
| v11-010 | Gold Layer Data Dictionary | ndp-analytics-engineer |
| v11-011 | Correlation-Ready Dashboard | ndp-grafana-dev |

#### SPARC Deliverables

| Phase | Deliverable | Location |
|-------|-------------|----------|
| **S** | SPECIFICATION.md | `fe-001/specification/PHASE-D-SPEC.md` |
| **P** | PSEUDOCODE.md (dashboard wireframes) | `fe-001/pseudocode/PHASE-D-PSEUDO.md` |
| **A** | Dashboard design | `fe-001/architecture/PHASE-D-ARCH.md` |
| **R** | Fast-follower test report | `fe-001/refinement/FAST-FOLLOWER-REPORT.md` |
| **C** | Deployment verification | `fe-001/completion/PHASE-D-COMPLETE.md` |

#### Exit Criteria

- [ ] **CRITICAL**: `outdoor-air-quality` added to Gold layer via **config change only**
- [ ] **CRITICAL**: Zero Rust code changes required for fast-follower
- [ ] Fast-follower time measured and documented (target: < 1 hour)
- [ ] Lag features (t-1h, t-6h, t-24h) working for all streams
- [ ] `data_dictionary.gold_*` tables populated and queryable
- [ ] Correlation-ready dashboard loads < 2s
- [ ] Dashboard shows objective thresholds as reference lines

#### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Fast-follower test reveals architecture gap | Medium | Critical | Treat as learning; fix architecture before Phase E |
| Dashboard performance poor on Pi | Low | Medium | Use continuous aggregate queries only |
| Data dictionary sync incomplete | Low | Low | Manual verification query |

**Note**: This phase is the **architecture proof point**. If fast-follower fails, stop and fix architecture before continuing.

---

### SPARC-E: Unified Event Abstraction

**Duration**: 1 week (Week 6)
**Team**: `ndp-rust-dev`, `ndp-timescale-dev`, `ndp-tester`

#### Scope

| Feature ID | Description | SPARC Owner |
|------------|-------------|-------------|
| v11-012 | Threshold Crossing Generator | ndp-rust-dev |
| v11-013 | Unified Events View | ndp-timescale-dev |
| v11-V02 | New Feature Type Test | ndp-tester |

#### SPARC Deliverables

| Phase | Deliverable | Location |
|-------|-------------|----------|
| **S** | SPECIFICATION.md | `fe-001/specification/PHASE-E-SPEC.md` |
| **P** | PSEUDOCODE.md | `fe-001/pseudocode/PHASE-E-PSEUDO.md` |
| **A** | Event schema design | `fe-001/architecture/PHASE-E-ARCH.md` |
| **R** | Event generation tests | `tools/ndp-gold-ddl/tests/events_test.rs` |
| **C** | V1.2 handoff verification | `fe-001/completion/PHASE-E-COMPLETE.md` |

#### Exit Criteria

- [ ] Threshold crossing events generated from objectives config
- [ ] `gold.events_unified` combines state transitions + threshold crossings
- [ ] Unified event schema: `(event_id, event_time, stream_id, entity_id, event_type, details)`
- [ ] Hourly event aggregates available in aligned view
- [ ] New feature type addable via trait implementation only (v11-V02)
- [ ] V1.2 can query unified events for pattern detection

#### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Threshold crossing chattering (too many events) | Medium | Medium | Document; defer hysteresis to V1.2 if needed |
| Event schema doesn't support V1.2 needs | Low | High | Review with V1.2 requirements before finalizing |
| Feature type registry too rigid | Low | Medium | Design for extension, not modification |

---

## Implementation Order and Dependencies

```
                    SPARC-A (Architecture)
                           |
                           v
                    SPARC-B (First Stream)
                           |
                           v
                    SPARC-C (Cross-Stream)
                           |
                           v
                    SPARC-D (Validation)
                           |
                           v
                    SPARC-E (Unified Events)
                           |
                           v
                    V1.2 Handoff Ready
```

### Dependency Matrix

| Phase | Depends On | Enables |
|-------|------------|---------|
| SPARC-A | V1.0 complete | SPARC-B (JSON schemas, DDL tool) |
| SPARC-B | SPARC-A | SPARC-C (proven aggregate pattern) |
| SPARC-C | SPARC-B | SPARC-D (3 streams aligned) |
| SPARC-D | SPARC-C | SPARC-E (architecture proven) |
| SPARC-E | SPARC-D | V1.2 Pattern Detection |

### Critical Path

1. **v11-A01 (Gold ETL Schema)** - Blocks all Gold config work
2. **v11-A02 (ndp-gold-ddl tool)** - Blocks all Gold DDL generation
3. **v11-003 (Continuous Aggregates)** - Blocks aligned view
4. **v11-V01 (Fast-Follower Test)** - Gates SPARC-E start

---

## Integration Test Strategy

### deploy/pi/deploy.sh Integration

Each SPARC phase MUST include a deploy.sh integration test:

| Phase | Test | Command |
|-------|------|---------|
| SPARC-A | Schema validation | `ndp-gold-ddl validate --stream air-quality` |
| SPARC-B | Single stream deploy | `deploy.sh apply test-manifest-b.json` |
| SPARC-C | Multi-stream deploy | `deploy.sh apply test-manifest-c.json` |
| SPARC-D | Fast-follower | `deploy.sh apply test-manifest-d.json` (config change only) |
| SPARC-E | Events deploy | `deploy.sh apply test-manifest-e.json` |

### Test Manifests

Create test manifests for each phase in `.deploy/test/`:

```
.deploy/test/
├── phase-a-validate.manifest.json    # Schema validation only
├── phase-b-air-quality.manifest.json # Single stream Gold
├── phase-c-multi-stream.manifest.json # 3 streams + alignment
├── phase-d-fast-follower.manifest.json # Add outdoor-air-quality
└── phase-e-events.manifest.json      # Unified events
```

### Pi Deployment Cadence

| Week | Deploy to Pi | Verification |
|------|--------------|--------------|
| Week 2 (end SPARC-A) | Schema validation only | `ndp-gold-ddl validate` passes |
| Week 3 (end SPARC-B) | air-quality Gold | Query `gold.air_quality_hourly` |
| Week 4 (end SPARC-C) | 3 streams + aligned view | Query `gold.indoor_air_quality_aligned` |
| Week 5 (end SPARC-D) | fast-follower | outdoor-air-quality in aligned view |
| Week 6 (end SPARC-E) | Unified events | Query `gold.events_unified` |

---

## London TDD Approach

### Core Principle

Following the NDP London TDD pattern established in dp-012:
- **Outside-In Development**: Start from behavior, work inward
- **Behavior Verification**: Mock collaborators, verify interactions
- **Interface Discovery**: Tests drive interface design
- **Fast, Isolated Tests**: Mocks eliminate external dependencies

### Mock Definitions for FE-001

#### 1. ConfigLoader Mock (existing pattern)

```rust
// Already exists in core/src/config/mock_loader.rs
// Extend for Gold layer config loading

mock! {
    pub GoldConfigLoader {}

    impl GoldConfigLoader for GoldConfigLoader {
        fn load_gold_etl(&self, stream_id: &str) -> Result<GoldEtlConfig, ConfigError>;
        fn load_domain(&self, domain_id: &str) -> Result<DomainConfig, ConfigError>;
        fn load_objectives(&self, domain_id: &str) -> Result<Vec<Objective>, ConfigError>;
    }
}
```

#### 2. SqlGenerator Mock (new)

```rust
// tools/ndp-gold-ddl/src/mocks.rs

mock! {
    pub SqlGenerator {}

    impl SqlGenerator for SqlGenerator {
        fn generate_continuous_aggregate(
            &self,
            config: &GoldEtlConfig,
            stream_id: &str
        ) -> Result<String, GeneratorError>;

        fn generate_aligned_view(
            &self,
            domain: &DomainConfig
        ) -> Result<String, GeneratorError>;

        fn generate_unified_events(
            &self,
            domain: &DomainConfig
        ) -> Result<String, GeneratorError>;
    }
}
```

#### 3. TimescaleDb Mock (existing pattern)

```rust
// Already exists in core/src/silver/mod.rs tests
// Reuse for Gold layer tests

mock! {
    pub TimescaleDb {}

    #[async_trait]
    impl TimescaleDb for TimescaleDb {
        async fn execute_ddl(&self, sql: &str) -> Result<(), DbError>;
        async fn query(&self, sql: &str) -> Result<Vec<Row>, DbError>;
        async fn view_exists(&self, schema: &str, view: &str) -> Result<bool, DbError>;
    }
}
```

### TDD Cycle Examples

#### Cycle 1: Schema Validation

```rust
#[test]
fn test_gold_etl_config_validates() {
    // RED: GoldEtlConfig::validate doesn't exist
    let config = GoldEtlConfig {
        enabled: true,
        aggregates: Aggregates {
            granularities: vec!["1 hour".into()],
            fields: HashMap::from([
                ("pm25".into(), FieldAggregates { metrics: vec!["mean", "std"] })
            ])
        },
        ..Default::default()
    };

    assert!(config.validate().is_ok());
}

// GREEN: Implement minimal validate()
// REFACTOR: Add error details
```

#### Cycle 2: DDL Generation

```rust
#[test]
fn test_generates_continuous_aggregate_sql() {
    // RED: generate_continuous_aggregate doesn't produce valid SQL
    let config = test_gold_etl_config();
    let generator = ContinuousAggregateGenerator::new();

    let sql = generator.generate(&config, "air-quality").unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW"));
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("time_bucket('1 hour'"));
    assert!(sql.contains("AVG(pm25) AS pm25_mean"));
}

// GREEN: Implement generate() with string templating
// REFACTOR: Use proper SQL builder
```

#### Cycle 3: Alignment View

```rust
#[test]
fn test_generates_aligned_view_sql() {
    // RED: alignment interpreter doesn't exist
    let domain = test_domain_config();
    let generator = AlignedViewGenerator::new();

    let sql = generator.generate(&domain).unwrap();

    assert!(sql.contains("FULL OUTER JOIN"));
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("gold.outdoor_weather_hourly"));
}

// GREEN: Implement JOIN generation
// REFACTOR: Handle NULL strategies per stream type
```

### Test Organization

```
tools/ndp-gold-ddl/
├── src/
│   ├── generators/
│   │   ├── continuous_aggregate.rs  # + tests mod
│   │   ├── aligned_view.rs          # + tests mod
│   │   ├── features.rs              # + tests mod
│   │   └── events.rs                # + tests mod
│   └── validation/
│       └── expressions.rs           # + tests mod
└── tests/
    ├── integration/
    │   ├── air_quality_test.rs      # Full air-quality flow
    │   ├── multi_stream_test.rs     # 3-stream alignment
    │   └── fast_follower_test.rs    # Config-change-only test
    └── fixtures/
        ├── air-quality-gold-etl.json
        └── indoor-air-quality-domain.yaml
```

### Coverage Goals

| Component | Line Coverage | Branch Coverage |
|-----------|---------------|-----------------|
| ContinuousAggregateGenerator | > 90% | > 85% |
| AlignedViewGenerator | > 90% | > 85% |
| FeatureGenerator | > 85% | > 80% |
| EventsGenerator | > 85% | > 80% |
| GoldEtlConfig validation | > 95% | > 90% |
| DomainConfig validation | > 95% | > 90% |

---

## Success Criteria for FE-001

### Architecture Success (Primary)

| Criterion | Target | How to Verify |
|-----------|--------|---------------|
| **Extensibility** | Add new stream via config only | Fast-follower test (Phase D) |
| **Fast-follower time** | < 1 hour to add stream to Gold | Timed exercise |
| **Config-driven** | Zero Rust changes for new stream | Code review of Phase D |

### Performance Success (Secondary)

| Criterion | Target | How to Verify |
|-----------|--------|---------------|
| Aligned view query | < 100ms for 30-day range | pg_stat_statements on Pi |
| Refresh policy | < 5% sustained CPU | Pi monitoring during refresh |
| Resource usage | < 200 MB peak | Pi memory monitoring |

### Completeness Success (V1.2 Handoff)

| Criterion | Target | How to Verify |
|-----------|--------|---------------|
| Stream classification | 100% of streams classified | Config audit |
| Unified events | State + threshold in single view | Query `gold.events_unified` |
| Objectives | Queryable via MCP | MCP tool test |
| Data dictionary | All Gold objects documented | Query `data_dictionary.gold_*` |

### Definition of Done

FE-001 is complete when:

1. [ ] All 5 SPARC cycles completed (A-E)
2. [ ] Fast-follower test passes (zero code changes)
3. [ ] All exit criteria met for each phase
4. [ ] STATUS.md updated to "done"
5. [ ] All participating agents recorded reflexion feedback
6. [ ] Patterns stored in AgentDB for future reference
7. [ ] V1.2 team confirms Gold layer meets their requirements

---

## Agent Coordination

### Primary Agents by Phase

| Phase | Lead Agent | Supporting Agents |
|-------|------------|-------------------|
| SPARC-A | `ndp-architect` | `ndp-rust-dev`, `ndp-tester` |
| SPARC-B | `ndp-rust-dev` | `ndp-timescale-dev`, `ndp-tester` |
| SPARC-C | `ndp-analytics-engineer` | `ndp-rust-dev`, `ndp-timescale-dev` |
| SPARC-D | `ndp-tester` | `ndp-grafana-dev`, `ndp-rust-dev` |
| SPARC-E | `ndp-rust-dev` | `ndp-timescale-dev`, `ndp-tester` |

### Reflexion Requirements

At the end of each SPARC cycle, agents MUST record reflexion:

| Agent | Reflexion Focus |
|-------|-----------------|
| `ndp-architect` | Architecture patterns (schemas, decisions) |
| `ndp-rust-dev` | Implementation patterns (DDL generation, Rust idioms) |
| `ndp-timescale-dev` | TimescaleDB patterns (continuous aggregates, policies) |
| `ndp-analytics-engineer` | SQL patterns (joins, null handling, window functions) |
| `ndp-grafana-dev` | Dashboard patterns (Gold layer queries, visualization) |
| `ndp-tester` | Testing patterns (TDD cycles, integration tests) |

---

## Open Questions

### Resolved in DECISIONS.md

1. **Gold DDL generation**: Rust CLI tool (`ndp-gold-ddl`) - ADR-FE001-001
2. **Config placement**: Embed `gold_etl` in StreamConfig
3. **Domain structure**: Domain-centric in `config/domains/`
4. **NULL handling**: By stream type (preserve for observations, carry forward for state)
5. **Idempotency**: Manifest declares `sync` vs `recreate`

### Open for SPARC-A

1. **Trend computation method**: SQL window function (simple) vs Rust (accurate)?
   - **Recommendation**: Start with SQL approximation, revisit in V1.2 if needed

2. **Percentile computation**: Verify p95/p99 performance on Pi
   - **Action**: Test in SPARC-B, adjust if needed

3. **Feature naming convention**: Need consistent `{stream}_{field}_{stat}_{window}` pattern
   - **Action**: Define in SPARC-A specification

### Deferred to V1.2

1. **Threshold crossing deduplication/hysteresis**
2. **Backfill strategy** (Bronze->Silver concern)

---

## References

### FE-001 Documents

- [SCOPE.md](./SCOPE.md) - Full V1.1 scope definition
- [DECISIONS.md](./architecture/DECISIONS.md) - 11 architecture decisions
- [CONFIG-DEPLOYMENT-FLOW.md](./architecture/CONFIG-DEPLOYMENT-FLOW.md) - 12-component deployment flow
- [STATUS.md](./STATUS.md) - Current progress

### NDP Patterns

- [dp-012 London TDD Strategy](../dp-012/specification/LONDON-TDD-STRATEGY.md) - TDD reference
- [dp-012 SPARC-S Specification](../dp-012/specification/SPARC-S-SPECIFICATION.md) - Specification template
- [Release Policy](../../docs/procedures/RELEASE-POLICY.md) - Versioning standard

### External

- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)

---

## Appendix: Pattern Search Results

The following patterns were retrieved from AgentDB to inform this coordination:

| Pattern ID | Name | Relevance |
|------------|------|-----------|
| 10 | `specification:config-validation-pipeline` | SPARC specification pattern |
| 25 | `architecture:gold-etl-config-structure` | Gold config structure |
| 32 | `architecture:config-deployment-flow` | Deployment flow reference |
| 29 | `architecture:domain-centric-gold-config` | Domain config pattern |
| 30 | `architecture:ndp-config-lifecycle` | Config lifecycle stages |
| 16 | `testing:ndp-types-london-tdd` | London TDD pattern |
| 14 | `implementation:london-tdd-schema-validation` | TDD for schema validation |

---

*SPARC Coordination created: 2026-02-04 by ndp-scrum-master*
