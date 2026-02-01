# dp-021: Config Lifecycle & MCP Administration

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

### Phase 4: Hot-Reload

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 4.1 | Wire etcd watch | Pending | |
| 4.2 | Implement source update | Pending | |
| 4.3 | Handle MQTT reconnect | Pending | |
| 4.4 | Handle HTTP polling change | Pending | |
| 4.5 | Add reload endpoint | Pending | |
| 4.6 | Integration test | Pending | |

### Phase 5: Schema Migration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 5.1 | Create migration framework | Pending | |
| 5.2 | Create v2.0 JSON Schema | Pending | |
| 5.3 | Implement v1.1→v2.0 migration | Pending | |
| 5.4 | Remove entity_schemas fallback | Pending | |
| 5.5 | Create migration CLI | Pending | |
| 5.6 | Add dry-run mode | Pending | |
| 5.7 | Update validator | Pending | |
| 5.8 | Update sync scripts | Pending | |
| 5.9 | Remove deprecated structs | Pending | |

### Phase 6: MCP Write Tools

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.1 | create_stream MCP tool | Pending | |
| 6.2 | update_stream MCP tool | Pending | |
| 6.3 | delete_stream MCP tool | Pending | |
| 6.4 | validate_stream MCP tool | Pending | |
| 6.5 | create_silver_table MCP tool | Pending | |
| 6.6 | reload_stream MCP tool | Pending | |

---

## Phasing

| Option | Phases | Effort | Status |
|--------|--------|--------|--------|
| Minimal | 4 only | 2-3 days | Not started |
| Core | 4 + 5 | 5-7 days | Not started |
| Full | 4 + 5 + 6 | 10-14 days | Not started |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | Pending | JSON configs with v1.1 schema |
| dp-019 | Pending | Validation pipeline |
| dp-020 | Pending | Declarative deploy |
| dp-017 | Complete | Integration environment ready |

---

## Branch
TBD

## Last Updated
2026-02-01
