# FE-002: Domain Configuration Standardization - Status

> **Last Updated:** 2026-02-05
> **Current Phase:** Refinement Complete (Ready for Implementation)
> **Overall Progress:** 15% (SPARC Refinement Complete)

---

## SPARC Phase Status

| SPARC Phase | Status | Deliverables |
|-------------|--------|--------------|
| **Specification** | Complete | SCOPE.md |
| **Pseudocode** | N/A | Reuses existing patterns |
| **Architecture** | N/A | Reuses existing ADRs |
| **Refinement** | Complete | TEST-STRATEGY.md, TDD-GUIDE.md, TEST-PLAN.md, GOLDEN-MASTER-FIXTURES.md |
| **Completion** | Pending | Implementation + Deployment |

---

## Implementation Phase Status

| Phase | Status | Progress | Notes |
|-------|--------|----------|-------|
| **0: Baseline Capture** | Complete | 100% | 12 fixtures captured |
| **A: YAML to JSON Migration** | Pending | 0% | GAP-001 resolution |
| **B0: Schema Format Fix** | Pending | 0% | GAP-004 - FLAT format consistency |
| **B: Schema Validation Integration** | Pending | 0% | GAP-003 resolution |

---

## Feature Checklist

### Phase A: YAML to JSON Migration (GAP-001)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| A1 | Convert domain.yaml to domain.json | Pending | 107 lines → ~120 lines JSON |
| A2 | Update loader path extension | Pending | loader.rs:46-47 |
| A3 | Update parser to serde_json | Pending | loader.rs:80 |
| A4 | Convert test fixtures to JSON | Pending | 3 inline tests in domain.rs |
| A5 | Remove serde_yaml dependency | Pending | Cargo.toml cleanup |
| A6 | Run test suite | Pending | `cargo test -p ndp-gold-ddl` |
| A7 | Manual CLI validation | Pending | End-to-end verification |

### Phase B0: Schema Format Standardization (GAP-004)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| B0-1 | Fix domain.schema.json to FLAT format | Pending | Remove wrapper requirement |
| B0-2 | Fix semantic validator to FLAT format | Pending | domain.rs changes |
| B0-3 | Update semantic validator tests | Pending | Test flat format |
| B0-4 | Verify schema validates domain.json | Pending | Manual verification |

### Phase B: Schema Validation Integration (GAP-003)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| B1 | Add `--domain` CLI flag | Pending | cli.rs modifications |
| B2 | Add config type enum | Pending | Stream vs Domain |
| B3 | Load domain schema for Layer 1 | Pending | schema.rs, main.rs |
| B4 | Wire semantic validation | Pending | semantic/mod.rs |
| B5 | Add domain validation flow | Pending | main.rs branching |
| B6 | Write unit tests | Pending | 30-40 new tests |
| B7 | Integration testing | Pending | End-to-end validation |
| B8 | Add to deploy.sh workflow | Pending | Pre-deployment validation |

---

## Blockers

| Blocker | Impact | Owner | Resolution |
|---------|--------|-------|------------|
| None | - | - | Ready to start |

---

## GitHub Issues Addressed

| Issue | Title | Status |
|-------|-------|--------|
| [#11](https://github.com/dug-21/neural-data-platform/issues/11) | GAP-001: Domain config uses YAML instead of JSON | Open → Resolved by Phase A |
| [#13](https://github.com/dug-21/neural-data-platform/issues/13) | GAP-003: No JSON Schema validation for domain configs | Open → Resolved by Phase B |

---

## Decisions Made

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-05 | Combine GAP-001 and GAP-003 into single feature | Sequential dependency, shared code, avoids intermediate state |
| 2026-02-05 | Feature ID: FE-002 | Next in FE- sequence after FE-001 |
| 2026-02-05 | Phase A before Phase B | JSON files must exist before JSON Schema validation can work |
| 2026-02-05 | **FLAT format consistency principle** | All NDP configs use flat format (no wrappers) - domain schema must be fixed |
| 2026-02-05 | Add Phase B0 for schema format fix | Schema defines target format; must be fixed before validation wiring |

---

## Refinement Documents

| Document | Purpose | Status |
|----------|---------|--------|
| [TEST-STRATEGY.md](./refinement/TEST-STRATEGY.md) | Golden master testing methodology | Complete |
| [TDD-GUIDE.md](./refinement/TDD-GUIDE.md) | London TDD implementation guide | Complete |
| [TEST-PLAN.md](./refinement/TEST-PLAN.md) | Detailed test cases (106 tests) | Complete |
| [GOLDEN-MASTER-FIXTURES.md](./refinement/GOLDEN-MASTER-FIXTURES.md) | Fixture management reference | Complete |

---

## Recent Activity

| Date | Activity | Outcome |
|------|----------|---------|
| 2026-02-05 | Swarm analysis of GAP-001 + GAP-003 | Dependency analysis, scope estimation |
| 2026-02-05 | Created SCOPE.md | Feature scoped, ready for implementation |
| 2026-02-05 | Created STATUS.md | Tracking document initialized |
| 2026-02-05 | Created SPARC Refinement docs | TEST-STRATEGY.md, TDD-GUIDE.md, TEST-PLAN.md, GOLDEN-MASTER-FIXTURES.md |
| 2026-02-05 | SPARC planning swarm (5 agents) | Specification, Pseudocode, Completion docs created |
| 2026-02-05 | Golden master baseline captured | 12 SQL fixtures + SHA256 manifest |
| 2026-02-05 | Discovered GAP-004 (schema format) | Domain schema uses WRAPPED format, should be FLAT |
| 2026-02-05 | Added Phase B0 to scope | Schema format fix before validation wiring |

---

## Next Actions

1. [x] Review SCOPE.md with stakeholders
2. [x] Create refinement test strategy documents
3. [x] Run baseline capture (12 fixtures captured)
4. [ ] Commit golden master fixtures to repo
5. [ ] **Phase A:** Convert domain.yaml to domain.json
6. [ ] Run Phase A checkpoint (golden master comparison)
7. [ ] **Phase B0:** Fix domain.schema.json to FLAT format
8. [ ] **Phase B0:** Fix semantic/domain.rs to FLAT format
9. [ ] **Phase B:** Add `--domain` flag to ndp-validate
10. [ ] Complete Phase B unit tests (30-40 new)
11. [ ] Integration testing
12. [ ] Close GitHub issues #11, #13

---

## Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Domain configs in JSON | 0 | 1 (indoor-air-quality) |
| Layer 1 validation for domains | No | Yes |
| Layer 2 validation for domains | Yes (exists) | Yes (integrated) |
| New test coverage | 0 | 30-40 tests |

---

*Feature: FE-002 Domain Configuration Standardization*
*Related: FE-001 (Gold Layer Foundation)*
