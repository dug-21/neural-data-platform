# dp-014: Status Tracker

**Feature**: Config-Driven Gold Layer
**Current Phase**: Draft Scope
**Started**: 2026-01-30
**Last Updated**: 2026-01-30

---

## Current Status: Draft Scope Captured

Scope captured from air-012 design discussion. Will be revised before implementation.

**Origin:** During air-012 scoping, decided to:
- Keep Silver as simple fact storage
- Compute SCD/features in config-driven Gold layer
- Establish pattern for ML feature engineering

---

## Phase Status

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| Scope Definition | **Draft** | 2026-01-30 | - |
| Specification (SPARC-S) | Not Started | - | - |
| Pseudocode (SPARC-P) | Not Started | - | - |
| Architecture (SPARC-A) | Not Started | - | - |
| Refinement (SPARC-R) | Not Started | - | - |
| Completion (SPARC-C) | Not Started | - | - |

---

## Draft Scope Summary

### Part 1: Gold View Configuration Schema
- YAML schema for Gold layer artifacts
- Support materialized views, views, continuous aggregates
- Column definitions with transforms
- Refresh strategies

### Part 2: SCD for State Events (First Use Case)
- `gold.state_periods` materialized view
- Computed valid_from/valid_to from event log
- Point-in-time lookup support

### Part 3: DDL Generator Extension
- Extend dp-013's DdlGenerator pattern
- Generate CREATE MATERIALIZED VIEW
- Generate indexes and refresh commands

### Part 4: Deploy Integration
- `./deploy.sh sync-gold-views`
- `./deploy.sh refresh-gold-view <id>`

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| air-012 state_events | Pending | Source table for first use case |
| dp-013 DdlGenerator | ✅ Ready | Pattern to extend |
| deploy.sh infrastructure | ✅ Ready | Add gold commands |

---

## Related Features

| Feature | Relationship |
|---------|--------------|
| air-012 | Provides first use case (state_events) |
| dp-013 | Provides DdlGenerator pattern |
| ml-??? | Will consume Gold layer features |

---

## Notes

This is a draft scope. Will be revised when:
1. air-012 is implemented and we learn from it
2. ML research clarifies feature requirements
3. We understand continuous aggregate needs better

---

*Status last updated: 2026-01-30*
