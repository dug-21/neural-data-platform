# dp-019: Config Validation Pipeline - Status

## Current Phase: Specification -> Pseudocode

**Last Updated**: 2026-02-02 15:30
**Phase**: SPARC Planning

---

## SPARC Progress

| Phase | Status | Artifacts |
|-------|--------|-----------|
| Specification | Complete | SPECIFICATION.md, *-RESEARCH.md files (4 documents) |
| Pseudocode | In Progress | PSEUDOCODE.md |
| Architecture | Complete | VALIDATION-ARCHITECTURE.md |
| Refinement | Not Started | - |
| Completion | Not Started | - |

---

## Research Artifacts (Completed)

| Document | Purpose | Key Findings |
|----------|---------|--------------|
| `SPECIFICATION.md` | Full requirements | 34 functional requirements, 8 NFRs, 8 acceptance criteria |
| `SUPPORTED-VALUES-RESEARCH.md` | NDP enum catalog | 15 enum categories, Rust/Schema discrepancies documented |
| `DQ-VALIDATION-RESEARCH.md` | DQ rule validation | 11 rule types, expression grammar, validation matrix |
| `SILVER-VALIDATION-RESEARCH.md` | Table existence checks | SQL patterns, type mapping, graceful degradation |
| `CURRENT-CONFIG-ANALYSIS.md` | Codebase analysis | Existing validation gaps, recommended crates |

---

## Key Decisions

### Architecture Decisions

1. **Two-Layer Validation Architecture**
   - Layer 1: JSON Schema (declarative, offline, fast)
   - Layer 2: Semantic validation (Rust code, may require DB)

2. **Schema vs Code Split**
   | What | Where | Rationale |
   |------|-------|-----------|
   | Field types | JSON Schema enum | Small, stable set |
   | Source types | JSON Schema enum | Limited types (mqtt, http_poll, etc.) |
   | device_class | Rust code (warning) | Freeform for HA compatibility |
   | source_path references | Rust code | Cross-section reference |
   | Table existence | Rust code | Requires DB query |
   | DQ expression syntax | Rust code | Requires parser |

3. **Graceful DB Degradation**
   - Schema validation works offline (no DB)
   - Silver table checks are optional (`--check-tables` flag)
   - Clear "skipped" status when DB unavailable

### Research Discoveries

1. **Rust/Schema Discrepancies Found** (from SUPPORTED-VALUES-RESEARCH.md)
   - `webhook` vs `http_push` in source types
   - `reject` vs `nullify` in DQ actions
   - Missing: `csv` source type, `array_iterator` parser in schema

2. **DQ Validation Complexity** (from DQ-VALIDATION-RESEARCH.md)
   - 11 distinct rule types with different validation needs
   - Cross-field expressions require SQL parsing
   - Field inheritance from parent mapping context

3. **Recommended Crates**
   - `jsonschema` (v0.17+) for Layer 1
   - `sqlparser` for DQ expression validation
   - `regex` for pattern_check validation

---

## Progress Checklist

### Research Tasks (Complete)

- [x] Research NDP-supported values (SUPPORTED-VALUES-RESEARCH.md)
- [x] Research DDL generation requirements (in SILVER-VALIDATION-RESEARCH.md)
- [x] Research DQ rule syntax and operators (DQ-VALIDATION-RESEARCH.md)
- [x] Analyze current config handling (CURRENT-CONFIG-ANALYSIS.md)

### SPARC Phases

- [x] SCOPE.md created
- [x] SPARC Specification complete
- [ ] SPARC Pseudocode complete
- [x] SPARC Architecture complete
- [ ] SPARC Refinement complete
- [ ] SPARC Completion complete

### Implementation (Not Started)

- [ ] Layer 1: Schema validation with `jsonschema` crate
- [ ] Layer 2: Semantic validation (source_path, table existence)
- [ ] Layer 2: DQ rule syntax validation
- [ ] ndp-validate CLI binary
- [ ] deploy.sh integration
- [ ] Runtime startup validation
- [ ] All tests passing
- [ ] Documentation updated

---

## Task Progress

### Layer 1: Schema Validation

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.1 | Create Validator component | Not Started | Rust binary |
| 2.2 | JSON syntax validation | Not Started | serde_json with line numbers |
| 2.3 | JSON Schema validation | Not Started | `jsonschema` crate |
| 2.4 | Unknown field detection | Not Started | `additionalProperties: false` |

### Layer 2: Semantic Validation

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.5 | Valid `type` values | Research Complete | Can be JSON Schema enum |
| 2.6 | Valid `device_class` values | Research Complete | Freeform with warnings |
| 2.7 | Cross-reference validation | Not Started | source_path -> fields |
| 2.8 | Silver table existence check | Research Complete | Optional DB check |
| 2.9 | DQ rule syntax validation | Research Complete | 11 rule types documented |
| 2.10 | Source config validation | Not Started | Per-type required fields |

### Integration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.11 | Integrate into deploy.sh | Not Started | Gates deployment |
| 2.12 | Runtime startup validation | Not Started | Defense in depth |
| 2.13 | Decide: Schema vs Code | Complete | Documented in SPECIFICATION |

---

## Risks & Blockers

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| dp-018 not complete | Medium | High | Coordinate with dp-018 team |
| False positives | Low | High | Comprehensive testing, escape hatch |
| Performance on large configs | Low | Medium | Benchmark early |
| Rust/Schema sync maintenance | Medium | Low | Document authoritative source |

### Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018: JSON Config Foundation | Required | JSON configs + ConfigLoader trait |
| dp-017: Integration Environment | Complete | Integration environment ready |
| JSON Schema v1.1 | Available | Already in `schemas/` |

---

## Next Steps

1. **Complete PSEUDOCODE.md** - Algorithm design for validation pipeline
2. **Begin TDD implementation** (Refinement phase)
   - Start with Layer 1 schema validation tests
   - Use existing `schemas/stream-config.v1.1.schema.json`
3. **Coordinate with dp-018** for JSON config availability
4. **Create test fixtures** for invalid configs

---

## File Locations

| Artifact | Path |
|----------|------|
| Scope | `product/features/dp-019/SCOPE.md` |
| Specification | `product/features/dp-019/specification/SPECIFICATION.md` |
| Architecture | `product/features/dp-019/architecture/VALIDATION-ARCHITECTURE.md` |
| Supported Values Research | `product/features/dp-019/specification/SUPPORTED-VALUES-RESEARCH.md` |
| DQ Validation Research | `product/features/dp-019/specification/DQ-VALIDATION-RESEARCH.md` |
| Silver Validation Research | `product/features/dp-019/specification/SILVER-VALIDATION-RESEARCH.md` |
| Current Config Analysis | `product/features/dp-019/specification/CURRENT-CONFIG-ANALYSIS.md` |

---

## Branch

TBD (feature branch will be created during Refinement phase)

---

*Status updated: 2026-02-02*
*Phase: SPARC Planning*
*Parent: dp-016 Configuration Architecture Review*
