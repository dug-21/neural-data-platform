# Agent Routing and Swarm Composition

## NDP Agent Preference

Always use NDP-specific agents over generic ones:

| Instead of | Use | Why |
|------------|-----|-----|
| `coder` | `ndp-rust-dev` | Knows Rust patterns, project structure |
| `system-architect` | `ndp-architect` | Knows Domain Adapter pattern, ADRs |
| `tester` | `ndp-tester` | Knows test patterns, mocking approach |
| `planner` | `ndp-scrum-master` | Knows feature lifecycle, reads protocols |
| `reviewer` | `ndp-validator` | Runs gate validations, glass box reports, trust recording |
| `researcher` | `ndp-researcher` | Knows project context, AgentDB patterns |

---

## Two Sessions, Two Leaders

Every feature follows a two-session lifecycle. The same coordinator agent (ndp-scrum-master) serves as leader in both sessions, reading different protocols.

| Session | Leader Role | Protocol | Ends When |
|---------|------------|----------|-----------|
| Session 1 | Design Leader | `design-protocol.md` | Returns artifacts to human for approval |
| Session 2 | Delivery Leader | `delivery-protocol.md` | All 3 gates pass and code is delivered |

The **Implementation Brief** is the handoff document between sessions.

---

## Every Swarm Has These Agents

| Agent | Role | When |
|-------|------|------|
| `ndp-scrum-master` | **Leader** — reads protocol, spawns workers, manages stages, updates GH Issues | Both sessions |
| `ndp-validator` | **Validation gate** — focused checks per gate, glass box reports, trust recording | Session 2 (spawned 3x) |

These are non-negotiable. No swarm runs without a leader and no delivery completes without validation gates.

---

## Complete Agent Roster

### Coordination (2 agents — mandatory)

| Agent | Type | What It Does |
|-------|------|-------------|
| `ndp-scrum-master` | coordinator | Design Leader (Session 1) or Delivery Leader (Session 2). Reads protocol, spawns workers, manages gates, GH Issue lifecycle |
| `ndp-validator` | gate | Gate 3a (design review), Gate 3b (code review), Gate 3c (risk coverage). One agent, three focused spawns |

### Research (1 agent — Phase 1)

| Agent | Type | What It Does |
|-------|------|-------------|
| `ndp-researcher` | specialist | Problem space exploration, codebase analysis, collaborative scope definition, writes SCOPE.md |

### Design (4 agents — Session 1, Phase 2)

| Agent | Type | What It Produces |
|-------|------|-----------------|
| `ndp-architect` | specialist | ARCHITECTURE.md with ADRs + Integration Surface, stores ADRs in AgentDB |
| `ndp-specification` | specialist | SPECIFICATION.md (requirements, acceptance criteria, domain models) |
| `ndp-risk-strategist` | specialist | RISK-TEST-STRATEGY.md (feature-level risks, test scenario mapping, priority) |
| `ndp-vision-guardian` | specialist | ALIGNMENT-REPORT.md (checks source docs against product vision) |

### Synthesis (1 agent — Session 1, Phase 2)

| Agent | Type | What It Produces |
|-------|------|-----------------|
| `ndp-synthesizer` | synthesizer | IMPLEMENTATION-BRIEF.md (coordinator's operating doc), ACCEPTANCE-MAP.md, GH Issue |

### Component Design (2 agents — Session 2, Stage 3a)

| Agent | Type | What It Produces |
|-------|------|-----------------|
| `ndp-pseudocode` | specialist | pseudocode/OVERVIEW.md + per-component pseudocode files |
| `ndp-tester` | specialist | test-plan/OVERVIEW.md + per-component test plan files (rooted in risk strategy) |

### Implementation (variable — Session 2, Stage 3b)

| Agent | Type | When to Include |
|-------|------|----------------|
| `ndp-rust-dev` | general | Any Rust code changes — the default implementation agent |
| `ndp-tester` | specialized | When new tests are needed or test strategy changes |
| `ndp-parquet-dev` | narrow | WAL, Parquet files, snapshot logic, `core/src/bronze/` |
| `ndp-timescale-dev` | narrow | Hypertables, continuous aggregates, ETL |
| `ndp-analytics-engineer` | specialized | Materialized views, domain transforms, `tools/ndp-gold-ddl/` |
| `ndp-dq-engineer` | specialized | Data quality rules, transparency tables, schema validation |
| `ndp-meteorologist` | specialized | NWS data, forecast schemas, atmospheric science |
| `ndp-air-quality-specialist` | specialized | AQI calculations, EPA standards, sensor calibration |
| `ndp-feature-engineer` | narrow | Time-series features, windowing, aggregations |
| `ndp-ml-engineer` | narrow | ruv-FANN models, training pipelines, inference |
| `ndp-grafana-dev` | narrow | Grafana dashboards, panels, data sources |
| `ndp-alert-engineer` | narrow | Rust-based triggers, thresholds, notifications |

### Testing (1 agent — Session 2, Stage 3c)

| Agent | Type | What It Produces |
|-------|------|-----------------|
| `ndp-tester` | execution | Runs all tests, produces RISK-COVERAGE-REPORT.md |

**Total: 19 agents** (2 coordination + 1 research + 4 design + 1 synthesis + 2 component design + variable implementation + 1 testing)

---

## Swarm Composition Templates

### Design Swarm (Session 1)

```
Leader:   ndp-scrum-master (Design Leader)
Wave 1:   ndp-architect, ndp-specification                         (parallel)
          ndp-architect stores ADRs in AgentDB via /save-pattern
Wave 2:   ndp-risk-strategist (reads arch + spec + vision)         (sequential)
Wave 3:   ndp-vision-guardian (reads all 3 source docs)            (sequential)
Wave 4:   ndp-synthesizer (brief + acceptance map + GH Issue)      (sequential, fresh context)
```

Produces: ARCHITECTURE.md, SPECIFICATION.md, RISK-TEST-STRATEGY.md, ALIGNMENT-REPORT.md, IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, GH Issue.

### Delivery Swarm (Session 2)

```
Leader:     ndp-scrum-master (Delivery Leader)
Stage 3a:   ndp-pseudocode, ndp-tester (test plans)               (parallel)
Gate 3a:    ndp-validator (component design validation)
Stage 3b:   ndp-rust-dev, specialists                             (parallel)
Gate 3b:    ndp-validator (code review validation)
Stage 3c:   ndp-tester (test execution)
Gate 3c:    ndp-validator (risk coverage validation)
```

### Stage 3b Composition by Feature Type

Use these as starting points for Stage 3b agent selection. The Delivery Leader picks agents based on the Component Map in the implementation brief.

#### General Rust Feature

```
Stage 3b:  ndp-rust-dev, ndp-tester
```

The baseline. Most features start here.

#### Data Pipeline (Bronze → Silver → Gold)

```
Stage 3b:  ndp-parquet-dev, ndp-timescale-dev, ndp-analytics-engineer, ndp-dq-engineer
```

Add domain scientist if pipeline involves domain-specific logic.

#### Schema / ETL Change

```
Stage 3b:  ndp-timescale-dev, ndp-dq-engineer
```

Architecture decisions already captured in Phase 2 source docs.

#### New Data Source

```
Stage 3b:  ndp-rust-dev, ndp-parquet-dev, {domain-scientist}
```

Domain scientist validates data interpretation.

#### ML / Predictions

```
Stage 3b:  ndp-feature-engineer, ndp-ml-engineer, ndp-rust-dev
```

#### Dashboard / Visualization

```
Stage 3b:  ndp-grafana-dev, ndp-analytics-engineer
```

#### Alerts / Triggers

```
Stage 3b:  ndp-alert-engineer, ndp-rust-dev, {domain-scientist}
```

#### Bug Fix

```
Stage 3b:  ndp-rust-dev, ndp-tester
```

For single-file bugs, skip the swarm entirely — just fix and validate.

---

## Composition Rules

1. **Every swarm**: ndp-scrum-master (leader) + ndp-validator (gates). No exceptions.
2. **Session 1**: Always ndp-architect + ndp-specification + ndp-risk-strategist + ndp-vision-guardian + ndp-synthesizer.
3. **Session 2 Stage 3a**: Always ndp-pseudocode + ndp-tester (test plans).
4. **Session 2 Stage 3b**: Varies by feature type — pick from implementation roster.
5. **Session 2 Stage 3c**: Always ndp-tester (execution mode).
6. **Domain data work**: Include the relevant domain scientist (meteorologist or air-quality-specialist).
7. **Schema/ETL changes**: Include ndp-dq-engineer for data quality impact.
8. **Skip swarm for**: single-file edits, 1-2 line fixes, config changes, docs, exploration.
9. **Max stage size**: 5 workers per stage. Split into waves within a stage if more agents needed.
