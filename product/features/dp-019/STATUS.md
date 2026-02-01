# dp-019: Config Validation Pipeline

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

### Research Tasks

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.0 | Research NDP-supported values | Pending | Outputs SUPPORTED-VALUES.md |
| 2.0a | Research DDL generation | Pending | Outputs DDL-GENERATION.md |

### Layer 1: Schema Validation

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.1 | Create Validator component | Pending | |
| 2.2 | JSON syntax validation | Pending | |
| 2.3 | JSON Schema validation | Pending | |
| 2.4 | Unknown field detection | Pending | |

### Layer 2: Semantic Validation

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.5 | Valid `type` values | Pending | |
| 2.6 | Valid `device_class` values | Pending | |
| 2.7 | Cross-reference validation | Pending | |
| 2.8 | Silver table existence check | Pending | |
| 2.9 | DQ rule syntax validation | Pending | |
| 2.10 | Source config validation | Pending | |

### Integration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 2.11 | Integrate into deploy.sh | Pending | |
| 2.12 | Runtime startup validation | Pending | |
| 2.13 | Decide: Schema vs Code | Pending | |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | Pending | JSON configs required |
| dp-017 | Complete | Integration environment ready |

---

## Branch
TBD

## Last Updated
2026-02-01
