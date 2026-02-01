# dp-018: JSON Config Foundation

## Current Phase
refinement (ready for implementation)

## Progress
- [x] SCOPE.md created
- [x] SPARC Specification complete
- [x] SPARC Pseudocode complete
- [x] SPARC Architecture complete
- [ ] SPARC Refinement (TDD implementation)
- [ ] SPARC Completion
- [ ] All tests passing
- [ ] Documentation updated

---

## SPARC Artifacts

| Phase | Document | Lines | Status |
|-------|----------|-------|--------|
| Specification | `specification/CURRENT-STATE-RESEARCH.md` | 500+ | Complete |
| Specification | `specification/SPECIFICATION.md` | 499 | Complete |
| Specification | `specification/TEST-STRATEGY.md` | 800+ | Complete |
| Pseudocode | `pseudocode/PSEUDOCODE.md` | 900+ | Complete |
| Architecture | `architecture/ADR-018-001-config-loader-design.md` | 500+ | Complete |
| Architecture | `architecture/JSON-SCHEMA-DESIGN.md` | 400+ | Complete |

### Key Findings from Research

**Root Cause**: `ConfigSyncService.to_stream_config()` parses `silver_etl` section but discards it. Silver ETL loads config from YAML files (not etcd) in `load_silver_etl_config()` at `main.rs:602-629`.

**Fix**: Add `silver_etl: Option<SilverEtlConfig>` to StreamConfig, fix ConfigSyncService to sync it, Silver uses existing StreamRegistry. No new traits or abstractions needed - consolidate on existing config-client.

### AgentDB Patterns Saved

| ID | Pattern | Tags |
|----|---------|------|
| 100 | `architecture:config-format` | dp-018, json, standard |
| 101 | `architecture:config-loading` | dp-018, config-loader, etcd |
| 102 | `architecture:silver-etl` | dp-018, subscriber, deprecated-batch |
| 103 | `procedure:config-migration` | dp-018, migration, schema-version |
| 104 | `architecture:field-metadata` | dp-018, fields, entity-schemas |
| 105 | `architecture:config-loader` | dp-018, trait, design |
| 106 | `testing:config-loader-tdd` | dp-018, london-tdd, mock |
| 107 | `architecture:json-schema-versioning` | dp-018, schema, versioning |

---

## Task Progress

### Phase 0: JSON Migration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 0.1 | Create JSON Schemas (v1.1) | Pending | Schema design in JSON-SCHEMA-DESIGN.md |
| 0.2 | Create supporting schemas | Pending | |
| 0.3 | Build migration script | Pending | Algorithm in PSEUDOCODE.md |
| 0.4 | Migrate stream configs | Pending | |
| 0.5 | Enrich fields with descriptions | Pending | Algorithm in PSEUDOCODE.md |
| 0.6 | Migrate dimension configs | Pending | |
| 0.7 | Update .gitignore | Pending | |
| 0.8 | Update documentation | Pending | |

### Phase 1: Unified Config Loading

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 1.1 | Add silver_etl to StreamConfig | Pending | Design in ADR-018-001 |
| 1.2 | Define SilverEtlConfig struct | Pending | In core/src/types/ |
| 1.3 | Fix ConfigSyncService | Pending | Include silver_etl in sync |
| 1.4 | Fix Silver subscriber | Pending | Use StreamRegistry.load_stream() |
| 1.5 | Fix data dictionary sync | Pending | Use fields.description |
| 1.5a | Update dictionary loader | Pending | Fallback algorithm in PSEUDOCODE.md |
| 1.6 | Add config source logging | Pending | Format in PSEUDOCODE.md |
| 1.7 | Promote sync errors | Pending | Quick win |

---

## Test Strategy

**Approach**: London TDD (outside-in, mock collaborators)

| Test Category | Infrastructure | Purpose |
|---------------|----------------|---------|
| Unit Tests | None | MockConfigLoader for isolation |
| Integration | None (mocked) | Component interactions |
| Contract | None | JSON Schema validation |
| Full Integration | Docker | Real etcd/TimescaleDB |

**Key Mock**: `MockConfigLoader` with builder pattern (`with_stream`, `with_error`)

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-017 | Complete | Integration environment ready |
| dp-016 | Complete | Architecture decisions made |

---

## Absorbs

- **air-013**: Unified Config Source for Silver ETL

---

## Branch
TBD (create when starting refinement)

## Last Updated
2026-02-01 (SPARC S/P/A phases complete)
