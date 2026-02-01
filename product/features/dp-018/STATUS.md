# dp-018: JSON Config Foundation

## Current Phase
planning

## Progress
- [x] SCOPE.md created
- [ ] SPARC Specification
- [ ] SPARC Pseudocode
- [ ] SPARC Architecture
- [ ] SPARC Refinement
- [ ] SPARC Completion
- [ ] All tests passing
- [ ] Documentation updated

---

## Task Progress

### Phase 0: JSON Migration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 0.1 | Create JSON Schemas (v1.1) | Pending | |
| 0.2 | Create supporting schemas | Pending | |
| 0.3 | Build migration script | Pending | |
| 0.4 | Migrate stream configs | Pending | |
| 0.5 | Enrich fields with descriptions | Pending | |
| 0.6 | Migrate dimension configs | Pending | |
| 0.7 | Update .gitignore | Pending | |
| 0.8 | Update documentation | Pending | |

### Phase 1: Unified Config Loading

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 1.1 | Create ConfigLoader trait | Pending | |
| 1.2 | Implement EtcdConfigLoader | Pending | |
| 1.3 | Fix Silver streaming | Pending | Critical fix |
| 1.4 | Fix Silver batch | Pending | |
| 1.5 | Fix data dictionary sync | Pending | |
| 1.5a | Update dictionary loader | Pending | |
| 1.6 | Add config source logging | Pending | |
| 1.7 | Promote sync errors | Pending | Quick win |

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
TBD

## Last Updated
2026-02-01
