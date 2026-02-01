# dp-020: Declarative Deploy

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

### Manifest and Orchestration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.1 | Define manifest schema | Pending | |
| 3.2 | Create manifest parser | Pending | |
| 3.9 | Create deploy.sh v2 | Pending | |
| 3.10 | Add device state tracking | Pending | |

### Action: Stream Sync

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.3 | Implement stream sync | Pending | |

### Action: Silver Table DDL (dp-015)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.4 | Implement silver-table action | Pending | |
| 3.4a | DDL generator: CREATE TABLE | Pending | |
| 3.4b | DDL generator: Indexes | Pending | |
| 3.4c | DDL generator: Hypertable | Pending | |
| 3.4d | DDL generator: Policies | Pending | |
| 3.4e | DDL generator: Permissions | Pending | |
| 3.4f | Idempotent execution | Pending | |

### Action: Other Declarations

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.5 | Implement migration action | Pending | |
| 3.6 | Implement dimensions action | Pending | |
| 3.7 | Implement dictionary action | Pending | |
| 3.8 | Implement reload logic | Pending | |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | Pending | JSON configs, ConfigLoader |
| dp-019 | Pending | Validation, type mapping |
| dp-017 | Complete | Integration environment ready |

---

## Absorbs

- **dp-015**: Config-Driven Silver Table Creation

---

## Branch
TBD

## Last Updated
2026-02-01
