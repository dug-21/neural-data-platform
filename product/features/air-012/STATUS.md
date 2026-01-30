# air-012: Status Tracker

**Feature**: Home Assistant Integration (Window/Door Sensors + State Events Pattern)
**Current Phase**: Architecture (SPARC-A)
**Started**: 2026-01-29
**Last Updated**: 2026-01-29 by ndp-scrum-master

---

## Current Status: Architecture Documentation Complete

SCOPE.md has been updated with detailed integration requirements. Architecture documents have been created covering MQTT integration patterns, data models, and feature engineering. Ready to proceed to Refinement phase.

**Dependency**: Requires dp-013 (CSV Source & Dimension Tables) for entity_context dimension loading.

---

## Phase Status

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| Scope Definition | **Complete** | 2026-01-29 | 2026-01-29 |
| Specification (SPARC-S) | Pending | - | - |
| Pseudocode (SPARC-P) | Pending | - | - |
| Architecture (SPARC-A) | **Complete** | 2026-01-29 | 2026-01-29 |
| Refinement (SPARC-R) | Pending | - | - |
| Completion (SPARC-C) | Pending | - | - |

---

## SPARC Phase Checklist

### Scope Phase - COMPLETE

- [x] Initial scope documented in SCOPE.md
- [x] MQTT integration verified (mosquitto_sub working)
- [x] Initial sensors identified (3 sensors)
- [x] Data flow defined (MQTT -> Bronze -> Silver)

### Architecture Phase - COMPLETE

- [x] INTEGRATION_PATTERNS.md - MQTT vs HTTP polling ADRs
- [x] DATA_MODEL.md - Silver/Gold schema design
- [x] FEATURE_ENGINEERING.md - ML feature definitions
- [x] AIR_QUALITY_DOMAIN.md - Ventilation thresholds for Florida
- [x] RECOMMENDATIONS_SUMMARY.md - Consolidated decisions

### Specification Phase - PENDING

- [ ] Functional requirements documented
- [ ] Non-functional requirements documented
- [ ] Interface contracts defined
- [ ] Test scenarios documented

### Refinement Phase - PENDING (Blocked by dp-013)

- [ ] Stream config created for `home-assistant-state`
- [ ] MQTT source adapter implemented
- [ ] Bronze storage for state events
- [ ] Silver ETL for `silver.state_events`
- [ ] Entity context dimension loaded (requires dp-013)

### Completion Phase - PENDING

- [ ] All 3 sensors integrated
- [ ] Dimension table populated
- [ ] Query: "Air quality when window open" working
- [ ] Dashboard updated

---

## Key Decisions Made

| Decision | Choice | Notes |
|----------|--------|-------|
| Timestamp source | Ingestion time | MQTT latency <100ms, acceptable for correlation |
| Message format | Simple state value | "on"/"off" payload from topic |
| Metadata storage | Dimension table | Context in `silver.entity_context` (dp-013) |
| Stream type | `state_events` | New generic pattern for boolean state changes |

---

## Dependencies

| Dependency | Status | Feature | Notes |
|------------|--------|---------|-------|
| dp-013 CSV Dimension Loader | **In Progress** | dp-013 | Required for entity_context |
| dp-012 Event Bus | In Progress | dp-012 | MQTT subscriber pattern |
| Bronze layer | Ready | air-002 | Parquet storage ready |
| Silver ETL | Ready | dp-006+ | Transform pattern established |

---

## Team

| Role | Agent | Focus |
|------|-------|-------|
| Coordinator | ndp-scrum-master | Feature lifecycle, STATUS.md |
| Architect | ndp-architect | MQTT patterns, state events schema |
| Rust Implementation | ndp-rust-dev | MQTT source adapter |
| Domain | ndp-air-quality-specialist | Ventilation thresholds |
| Testing | ndp-tester | Integration tests |

---

## Scope Summary

### Part 1: Home Assistant Window/Door Sensor Integration

Integrate 3 binary sensors via MQTT:
- `door_backslider` - Back door slider
- `door_officewindow` - Office window
- `door_dinettewindow` - Dinette window

### Part 2: State Events Platform Pattern

Generalize to reusable `stream_type: state_events` pattern:
- Generic schema: `silver.state_events`
- Applicable across domains (IoT, finance, operations)

---

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| - | - | No bugs tracked yet |

---

## Architecture Documents

| Document | Location | Status |
|----------|----------|--------|
| SCOPE.md | `product/features/air-012/SCOPE.md` | Complete |
| INTEGRATION_PATTERNS.md | `architecture/INTEGRATION_PATTERNS.md` | Complete |
| DATA_MODEL.md | `architecture/DATA_MODEL.md` | Complete |
| FEATURE_ENGINEERING.md | `architecture/FEATURE_ENGINEERING.md` | Complete |
| AIR_QUALITY_DOMAIN.md | `architecture/AIR_QUALITY_DOMAIN.md` | Complete |
| RECOMMENDATIONS_SUMMARY.md | `architecture/RECOMMENDATIONS_SUMMARY.md` | Complete |

---

## Related Patterns

| Pattern | Relevance |
|---------|-----------|
| `arch-domain-adapter-pattern` | MQTT adapter follows Source trait |
| `arch-data-lake-layers` | Bronze (raw MQTT) -> Silver (state_events) |
| `config-silver-metadata-fields` | Schema definition approach |

---

## Next Steps

1. **Complete Specification Phase**: Document functional requirements and test scenarios
2. **Wait for dp-013**: Dimension loader required for entity_context
3. **Implement MQTT Source**: After dp-013, implement MqttSource adapter
4. **Create Silver Schema**: `silver.state_events` table

---

*Status last updated: 2026-01-29 by ndp-scrum-master*
