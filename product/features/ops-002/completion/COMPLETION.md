# OPS-002: Eliminate Hardcoded References from Gold Layer Generators - COMPLETION

> **Feature ID:** ops-002
> **Version:** v1.1.11
> **SPARC Phase:** Completion
> **Created:** 2026-02-06
> **Author:** ndp-scrum-master

---

## 1. Definition of Done

ALL criteria must pass before ops-002 is released:

### Functional Completeness

- [ ] Zero hardcoded domain-specific values in generator SQL output
- [ ] `events.rs` detection procedure reads streams, columns, thresholds from `DomainConfig`/`StreamConfig`
- [ ] `state_transitions.rs` default direction case reads state values from config (not hardcoded `'on'`/`'off'`)
- [ ] `state_transitions.rs` device type case reads mapping from config (not hardcoded `'door_%'`, `'window_%'`, etc.)
- [ ] `aligned_view.rs` `determine_stream_type()` reads `stream_type` from `StreamConfig` (not string-matching on stream ID)
- [ ] `ndp_id` entity column name sourced from shared constant or config
- [ ] `gold` schema name sourced from shared constant
- [ ] `issued_at` forecast timestamp column sourced from config

### Test Compliance

- [ ] All existing 556+ tests still pass (zero regressions)
- [ ] Hardcoding detection test suite passes with fictional "energy-monitoring" domain config
- [ ] Config-driven generation tests pass for air-quality domain (backwards compatibility)
- [ ] Integration tests pass against `docker-compose.integration.yml` TimescaleDB
- [ ] Source code scanning tests find zero domain-specific literals in generator source
- [ ] Generated SQL for air-quality domain produces semantically equivalent output to pre-refactor baseline
- [ ] Detection procedure (job 1026) works with production Pi config
- [ ] All new and changed code has London TDD tests
- [ ] Fictional domain test proves config-only domain addition (no Rust code changes)
- [ ] New test count >= 15

### Documentation and Process

- [ ] Architecture patterns saved to AgentDB for future reference
- [ ] All participating agents have recorded reflexion
- [ ] STATUS.md updated to "done"
- [ ] Release artifacts created (manifest, tag, changelog)

---

## 2. Implementation Phases

### Phase 1: Test Infrastructure (Write Tests First -- London TDD)

**Goal:** Create the test harness that defines "done" before writing any implementation code.

**Dependencies:** None (first phase)

**Deliverables:**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| T-001 | Fictional domain config | Create `energy-monitoring` domain config with streams, objectives, alignment -- entirely different from air-quality | Valid JSON config for a fictional energy domain with solar, grid-power, thermostat-state streams |
| T-002 | Hardcoding detection tests | Tests that scan generated SQL for domain-specific string literals | Tests FAIL initially; pass after refactoring |
| T-003 | Config-driven generation tests | Tests that generate SQL for energy-monitoring domain and verify correctness | Tests FAIL initially; pass after refactoring |
| T-004 | Source code scanning tests | Regex-based tests that scan Rust source files for hardcoded domain values | Tests FAIL initially; pass after refactoring |
| T-005 | Baseline capture | Capture current air-quality SQL output as golden master for semantic equivalence | Golden master files exist in `.test/ops-002/` |
| T-006 | Backwards compatibility tests | Tests that generate SQL for air-quality domain and compare to golden master | Tests PASS throughout (no regression) |

**Fictional Domain Config (`energy-monitoring`):**

```
domain_id: "energy-monitoring"
streams:
  - solar-output (observation, stream_type in config)
  - grid-power (observation)
  - thermostat-state (state_event)
objectives:
  - daily_cost < $5.00
  - solar_utilization > 80%
alignment:
  - all three streams, hourly
```

**Exit Criteria:** All detection tests exist and FAIL. All backwards-compatibility tests PASS.

---

### Phase 2: Shared Constants (P1 -- Quick Wins)

**Goal:** Extract repeated magic strings into shared constants.

**Dependencies:** Phase 1 (tests exist)

**Deliverables:**

| ID | Task | Files Affected | Acceptance Criteria |
|----|------|---------------|---------------------|
| C-001 | `NDP_ENTITY_COLUMN` constant | `events.rs`, `state_transitions.rs`, `continuous_aggregate.rs`, `aligned_view.rs`, `column_builder.rs` | All 5+ files reference the constant; zero literal `"ndp_id"` in generators |
| C-002 | `GOLD_SCHEMA` constant | `events.rs`, `state_transitions.rs`, `aligned_view.rs`, `continuous_aggregate.rs`, `join_builder.rs`, `refresh_policy.rs` | All 6+ files reference the constant; zero literal `"gold"` schema strings in generators |
| C-003 | Constants module | New `generators/constants.rs` or addition to existing module | Constants are `pub const` with doc comments |
| C-004 | Unit tests for constants | Tests verify generated SQL uses correct values | All constant-related tests pass |

**Risk:** Low -- mechanical find-and-replace with test coverage.

**Exit Criteria:** Source scan tests for `"ndp_id"` and `"gold"` literals pass.

---

### Phase 3: EventsGenerator Refactoring (P0 -- Core)

**Goal:** Make `generate_detection_procedure()` fully config-driven.

**Dependencies:** Phase 2 (constants available)

**Deliverables:**

| ID | Task | Hardcoded Values Eliminated | Acceptance Criteria |
|----|------|---------------------------|---------------------|
| E-001 | State transition section reads from config | `'home-assistant-state'`, `silver.state_events`, `s.ndp_id`, `s.state`, `s.event_time` | Stream ID, source table, entity field, state field, timestamp field all read from `DomainConfig.streams` where role=Actuator and stream_type=state_event |
| E-002 | Threshold crossing section reads from objectives | `'air-quality'`, `co2_mean`, `pm25_mean`, `800.0`, `12.0`, `'healthy_co2'`, `'healthy_pm25'` | All metrics, thresholds, conditions, objective IDs read from `DomainConfig.objectives[].targets[]` |
| E-003 | Gold CA table name from config | `gold.air_quality_hourly` | Table name derived from stream config (`gold.{stream_id_snake}_hourly`) |
| E-004 | Context enrichment from aligned view columns | `indoor_co2_mean`, `indoor_pm25_mean`, `indoor_temperature_c_mean`, `outdoor_temperature_c_mean`, `outdoor_aqi_pm25_mean`, `state_state_last` | Context columns derived from domain's alignment config and stream field definitions |
| E-005 | Unit mapping from objectives | `'ppm'`, `'ug/m3'` | Unit strings read from `objectives[].targets[].unit` |
| E-006 | Generator signature update | N/A | `generate_detection_procedure()` accepts `&DomainConfig` (or already has it via `self`) and iterates over streams/objectives |

**Approach:**

The detection procedure has two independently refactorable sections:

1. **State Transitions block (lines 472-517):** Iterate `domain.streams` filtered to `stream_type == state_event`. For each, look up `silver_etl.target_table` via `ConfigLoader`. Generate the CTE per state-event stream.

2. **Threshold Crossings block (lines 526-622):** Iterate `domain.objectives[].targets[]`. For each target, look up the observation stream's Gold CA table name. Generate a crossing CTE per target metric.

**Critical Constraint:** The refactored procedure must produce SQL that executes identically on the current air-quality domain -- same events detected, same context captured.

**Exit Criteria:** Energy-monitoring domain generates valid detection procedure SQL. Air-quality domain produces semantically equivalent SQL to baseline.

---

### Phase 4: StateTransitionGenerator Refactoring (P0)

**Goal:** Eliminate hardcoded state values and device type patterns.

**Dependencies:** Phase 2 (constants available)

**Deliverables:**

| ID | Task | Hardcoded Values Eliminated | Acceptance Criteria |
|----|------|---------------------------|---------------------|
| S-001 | Default direction case from config | `'off'`, `'on'`, `'opening'`, `'closing'` | Direction mapping read from `TransitionConfig.direction_mapping` (which already exists) -- make it required, not optional fallback to hardcoded |
| S-002 | Device type case from config | `'door_%'`, `'window_%'`, `'motion_%'`, `'light_%'` | Device type derivation read from config or removed (entity naming conventions are domain-specific) |
| S-003 | Entity field from config | `"ndp_id"` default | Uses `NDP_ENTITY_COLUMN` constant from Phase 2, or reads from `TransitionConfig.entity_field` |

**Design Decision Required:**

The `generate_device_type_case()` function (lines 306-317) hardcodes Home Assistant entity patterns. Options:

- **Option A:** Add `device_type_mapping` to `TransitionConfig` (e.g., `{"door_": "door", "window_": "window"}`)
- **Option B:** Remove device_type column entirely (it is domain-specific and not used by V1.2 correlation engine)
- **Option C:** Make it optional -- only generate if mapping provided in config

Recommended: **Option C** -- backwards compatible, doesn't break existing SQL, but new domains aren't forced to provide a mapping.

**Exit Criteria:** Energy-monitoring domain's thermostat-state stream generates valid transition DDL without air-quality patterns.

---

### Phase 5: AlignedView Fix (P0)

**Goal:** Read `stream_type` from config instead of inferring from stream ID string matching.

**Dependencies:** Phase 2 (constants available)

**Deliverables:**

| ID | Task | Hardcoded Values Eliminated | Acceptance Criteria |
|----|------|---------------------------|---------------------|
| A-001 | Read stream_type from StreamConfig | String pattern matching on `"forecast"`, `"state"`, `"event"`, `"dimension"`, `"ref"` | `determine_stream_type()` reads `stream_config.stream_type` field; falls back to current heuristic with deprecation warning |
| A-002 | StreamConfig already has stream_type | Verify field exists | `stream_type` field exists in all stream configs (it was added in v11-001) |

**Risk:** Low -- `StreamConfig` already has `stream_type` field. This is a one-function fix.

**Exit Criteria:** A stream named `solar-output` (contains no stream-type keywords) is correctly classified as `observation` via its config's `stream_type` field.

---

### Phase 6: Integration Verification

**Goal:** Prove everything works end-to-end.

**Dependencies:** Phases 3, 4, 5 (all refactoring complete)

**Deliverables:**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| V-001 | Run full test suite | `cargo test --workspace` | All 556+ existing tests pass, plus 15+ new tests |
| V-002 | Semantic equivalence check | Compare generated air-quality SQL before/after | SQL output is semantically equivalent (same tables, columns, logic) |
| V-003 | Integration test against TimescaleDB | Run generated SQL against `docker-compose.integration.yml` | All SQL executes without error; events are detected |
| V-004 | Fictional domain end-to-end | Generate full Gold DDL for energy-monitoring domain | Valid SQL generated from config alone, no Rust code changes |
| V-005 | Pi config verification | Run detection procedure generation with production Pi domain config | Procedure SQL generates without error |
| V-006 | Hardcoding scan passes | Source code scan of all generator `.rs` files | Zero domain-specific literals found |

**Exit Criteria:** All verification tasks pass. Zero regressions.

---

### Phase 7: Release (v1.1.11)

**Goal:** Package and release following NDP release policy.

**Dependencies:** Phase 6 (verification complete)

**Deliverables:**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| R-001 | Version determination | PATCH bump: refactoring, no new features | v1.1.11 confirmed |
| R-002 | Manifest creation | `.deploy/releases/v1.1.11.manifest.json` | Valid manifest with container build declaration |
| R-003 | Changelog update | `CHANGELOG.md` entry for v1.1.11 | Entry describes all hardcoding eliminations |
| R-004 | Git commit and tag | Annotated tag `v1.1.11` | Tag matches manifest `release_version` |
| R-005 | Pattern documentation | Save architecture patterns to AgentDB | Patterns stored for future config-driven generator work |
| R-006 | Reflexion recording | All participating agents record reflexion | Reflexion episodes recorded |

**Release Checklist (per RELEASE-POLICY.md):**

- [ ] All changes tested in integration environment
- [ ] Stream configs validated
- [ ] DDL generation tested with `--dry-run`
- [ ] No uncommitted changes
- [ ] Manifest created: `.deploy/releases/v1.1.11.manifest.json`
- [ ] CHANGELOG.md updated
- [ ] Commit: `git commit -m "release: v1.1.11"`
- [ ] Tag: `git tag -a v1.1.11 -m "Release v1.1.11: Eliminate hardcoded references from Gold generators"`
- [ ] Push code and tag

---

## 3. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| SQL semantic changes break production events | Medium | High | Golden master comparison: capture before/after SQL, verify identical event detection on same input data |
| Missing config field causes runtime panic | Medium | High | Validate config completeness in tests; use `Option<>` with sensible defaults where appropriate |
| Performance regression from dynamic SQL generation | Low | Medium | Benchmark SQL generation time; detection procedure uses same SQL patterns, just parameterized differently |
| Future hardcoding reintroduction | High | High | Source code scanning tests in CI catch any new domain literals in generators |
| Backwards-incompatible config change | Low | High | Air-quality config is unchanged; new fields are additive (optional with defaults) |
| `determine_stream_type()` fallback breaks existing behavior | Low | Medium | Keep heuristic as fallback with deprecation warning; only remove when all configs have `stream_type` |
| Device type column removal breaks downstream | Low | Medium | Use Option C (optional) -- existing SQL unchanged, new domains just omit the mapping |

---

## 4. Hardcoded Values Inventory

Complete inventory of domain-specific values to eliminate:

### events.rs -- `generate_detection_procedure()`

| Line | Hardcoded Value | Source Should Be | Priority |
|------|----------------|-----------------|----------|
| 475 | `'home-assistant-state'` | `DomainConfig.streams[role=Actuator].stream_id` | P0 |
| 481 | `silver.state_events` | `StreamConfig.silver_etl.target_table` (via ConfigLoader) | P0 |
| 478 | `s.ndp_id` | `NDP_ENTITY_COLUMN` constant or stream config entity field | P1 |
| 478 | `s.state` | `TransitionConfig.state_field` | P0 |
| 478 | `s.event_time` | `StreamConfig.silver_etl.timestamp.target_field` | P0 |
| 505 | `indoor_co2_mean` | Aligned view column from stream config fields | P0 |
| 506 | `indoor_pm25_mean` | Aligned view column from stream config fields | P0 |
| 507 | `indoor_temperature_c_mean` | Aligned view column from stream config fields | P0 |
| 508 | `outdoor_temperature_c_mean` | Aligned view column from stream config fields | P0 |
| 509 | `outdoor_aqi_pm25_mean` | Aligned view column from stream config fields | P0 |
| 510 | `state_state_last` | Aligned view column from stream config fields | P0 |
| 530 | `'air-quality'::TEXT` | `DomainConfig.objectives[].targets[].stream` | P0 |
| 531 | `co2_mean` | Gold CA column from stream config field + `_mean` suffix | P0 |
| 533 | `pm25_mean` | Gold CA column from stream config field + `_mean` suffix | P0 |
| 535 | `gold.air_quality_hourly` | `gold.{stream_id_snake}_hourly` derived from stream config | P0 |
| 545 | `800.0` | `DomainConfig.objectives[].targets[metric=co2].threshold` | P0 |
| 547-548 | `800` (repeated 4x) | Same threshold from objectives | P0 |
| 552 | `'healthy_co2'` | `DomainConfig.objectives[].id` or target-derived ID | P0 |
| 569 | `12.0` | `DomainConfig.objectives[].targets[metric=pm25].threshold` | P0 |
| 571-572 | `12` (repeated 4x) | Same threshold from objectives | P0 |
| 576 | `'healthy_pm25'` | `DomainConfig.objectives[].id` or target-derived ID | P0 |
| 621 | `'ppm'`, `'ug/m3'` | `objectives[].targets[].unit` | P1 |

**Total: 25+ hardcoded domain values in events.rs alone**

### state_transitions.rs

| Line | Hardcoded Value | Source Should Be | Priority |
|------|----------------|-----------------|----------|
| 67 | `"ndp_id"` default | `NDP_ENTITY_COLUMN` constant | P1 |
| 296 | `'off'`, `'on'` | `TransitionConfig.direction_mapping` (make required) | P0 |
| 297 | `'on'`, `'off'` | Same config | P0 |
| 296 | `'opening'`, `'closing'` | Direction labels from config | P0 |
| 309 | `'door_%'` | Config-driven or optional | P0 |
| 310 | `'window_%'` | Config-driven or optional | P0 |
| 311 | `'motion_%'` | Config-driven or optional | P0 |
| 312 | `'light_%'` | Config-driven or optional | P0 |

**Total: 6+ hardcoded domain values in state_transitions.rs**

### aligned_view.rs

| Line | Hardcoded Value | Source Should Be | Priority |
|------|----------------|-----------------|----------|
| 122 | `"forecast"` string check | `StreamConfig.stream_type` field | P0 |
| 124 | `"state"`, `"event"` string check | `StreamConfig.stream_type` field | P0 |
| 126 | `"dimension"`, `"ref"` string check | `StreamConfig.stream_type` field | P0 |

**Total: 3 hardcoded heuristic patterns in aligned_view.rs**

### Cross-Cutting

| File(s) | Hardcoded Value | Source Should Be | Priority |
|---------|----------------|-----------------|----------|
| 5+ files | `"ndp_id"` literal | `NDP_ENTITY_COLUMN` constant | P1 |
| 6+ files | `"gold"` schema literal | `GOLD_SCHEMA` constant | P1 |

**Grand Total: 50+ hardcoded domain-specific values across generators**

---

## 5. Success Metrics

| Metric | Target | How Measured |
|--------|--------|-------------|
| Hardcoded domain values in generators | 0 | Source code scan + detection test suite |
| Test regression count | 0 | All 556+ existing tests pass |
| New test coverage | >= 15 new tests | Test count delta |
| Fictional domain test | PASS | energy-monitoring config generates valid SQL without code changes |
| Integration test | PASS | Generated SQL executes on TimescaleDB |
| Semantic equivalence | PASS | Air-quality SQL output unchanged in behavior |
| Config-only domain addition | Verified | Fictional domain proves the architecture |
| Source scan | 0 domain literals | Regex scan of generator Rust source files |

---

## 6. Agent Assignments

| Phase | Primary Agent | Supporting Agents | Model |
|-------|--------------|-------------------|-------|
| Phase 1: Test Infrastructure | `ndp-tester` | `ndp-architect` (fictional config design) | opus |
| Phase 2: Shared Constants | `ndp-rust-dev` | `ndp-tester` (test updates) | opus |
| Phase 3: Events Refactoring | `ndp-rust-dev` | `ndp-architect` (config schema), `ndp-tester` | opus |
| Phase 4: State Transitions | `ndp-rust-dev` | `ndp-tester` | opus |
| Phase 5: Aligned View | `ndp-rust-dev` | `ndp-tester` | opus |
| Phase 6: Integration | `ndp-tester` | `ndp-rust-dev` | opus |
| Phase 7: Release | `ndp-scrum-master` | all | opus |

---

## 7. Dependency Graph

```
Phase 1 (Tests)
    |
    v
Phase 2 (Constants) ----+
    |                    |
    v                    v
Phase 3 (Events)    Phase 4 (State Trans)    Phase 5 (Aligned View)
    |                    |                        |
    +--------------------+------------------------+
    |
    v
Phase 6 (Integration Verification)
    |
    v
Phase 7 (Release v1.1.11)
```

Phases 3, 4, and 5 can run in parallel after Phase 2 completes.

---

## 8. References

- `/workspaces/neural-data-platform/product/features/ops-002/SCOPE.md` -- Problem statement
- `/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md` -- V1.1-V2.0 vision (config-driven principle)
- `/workspaces/neural-data-platform/docs/procedures/RELEASE-POLICY.md` -- Release process
- `/workspaces/neural-data-platform/product/features/ops-001/SCOPE.md` -- Previous ops feature (patterns)
- `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/generators/events.rs` -- Primary refactoring target
- `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/generators/state_transitions.rs` -- Secondary target
- `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/generators/aligned_view.rs` -- Tertiary target
