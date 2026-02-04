# FE-001 Architecture Decision Record Index

**Feature**: FE-001 Gold Layer Foundation
**Created**: 2026-02-04
**Status**: Active

---

## Overview

This document indexes all Architecture Decision Records (ADRs) for the FE-001 Gold Layer Foundation feature. These decisions establish the architectural patterns for the Gold layer of the Neural Data Platform's Bronze -> Silver -> Gold data lake architecture.

---

## ADR Summary

| ADR ID | Title | Status | Key Decision |
|--------|-------|--------|--------------|
| [ADR-FE001-001](./ADR-FE001-001-gold-ddl-rust.md) | Gold DDL Generation in Rust | Accepted | Rust CLI tool for complex Gold SQL generation |
| [ADR-FE001-002](./ADR-FE001-002-domain-centric-config.md) | Domain-Centric Configuration | Accepted | Cross-stream config in `config/domains/` |
| [ADR-FE001-003](./ADR-FE001-003-forecast-alignment.md) | Forecast Streams Align on issued_at | Accepted | Forecasts join on availability, not prediction time |
| [ADR-FE001-004](./ADR-FE001-004-null-handling.md) | NULL Handling by Stream Type | Accepted | Preserve vs carry-forward based on stream_type |
| [ADR-FE001-005](./ADR-FE001-005-manifest-idempotency.md) | Manifest-Declared Idempotency | Accepted | Explicit sync/recreate action in manifest |

---

## Decision Categories

### Configuration Architecture

- **ADR-FE001-002**: Establishes domain-centric configuration pattern
  - Streams remain domain-agnostic building blocks
  - Domains contain alignment, objectives, constraints
  - Location: `config/domains/{domain_id}/domain.yaml`

### Code Generation

- **ADR-FE001-001**: Establishes Rust-based DDL generation
  - Tool: `tools/ndp-gold-ddl/`
  - Replaces Bash for complex Gold SQL patterns
  - Integrated with deploy.sh

### Data Alignment

- **ADR-FE001-003**: Defines forecast stream alignment strategy
  - Forecasts align on `issued_at` (when available) not `valid_time` (what predicted)
  - Preserves causal validity for correlation analysis

- **ADR-FE001-004**: Defines NULL handling strategy
  - Observations: Preserve NULL (don't fabricate data)
  - State events: Carry forward (state persists until changed)
  - Forecasts: Preserve NULL

### Deployment

- **ADR-FE001-005**: Defines idempotency strategy
  - Manifest declares `action: sync` or `action: recreate`
  - Detection at manifest creation, not deploy time
  - Addresses TimescaleDB continuous aggregate limitations

---

## Related ADRs (Other Features)

These ADRs from other features are referenced by FE-001 decisions:

| ADR | Feature | Relationship |
|-----|---------|--------------|
| ADR-016-001 | dp-016 | Config Source of Truth (JSON standard) |
| ADR-016-002 | dp-016 | Declarative Deploy (manifest pattern) |
| ADR-018-001 | dp-018 | Config Loader Design (pass-through architecture) |
| ADR-019-001 | dp-019 | Two-Layer Validation (validation pattern) |

---

## Implementation Phases

These ADRs inform the FE-001 implementation phases:

### Phase A: Config & Validation (Weeks 1-2)

| Phase | ADRs Applied |
|-------|--------------|
| A01: Gold ETL JSON Schema | ADR-019-001 (validation pattern) |
| A02: Update ndp-validate | ADR-019-001, ADR-FE001-002 |
| A03: Domain JSON Schema | ADR-FE001-002 |
| A04: Extend StreamConfig struct | ADR-FE001-002, ADR-FE001-003 |
| A05: Create ndp-gold-ddl tool | **ADR-FE001-001** |
| A06: Deploy.sh handlers | ADR-FE001-001, **ADR-FE001-005** |

### Phase B-E: Implementation

| Phase | ADRs Applied |
|-------|--------------|
| B: Continuous Aggregates | ADR-FE001-001, ADR-FE001-005 |
| C: Feature Views | ADR-FE001-001 |
| D: Aligned Views | ADR-FE001-002, **ADR-FE001-003**, **ADR-FE001-004** |
| E: Unified Events | ADR-FE001-002 |

---

## Supersession History

No FE-001 ADRs have been superseded. All decisions are current and active.

---

## Decision Principles

The FE-001 ADRs follow these guiding principles:

1. **Extend Existing Patterns** - Gold layer follows Silver layer patterns where applicable
2. **Explicit Over Implicit** - Manifest declares intent; no runtime detection
3. **Causal Validity** - Data alignment preserves ability to analyze cause and effect
4. **Semantic Correctness** - NULL handling matches data semantics (observation vs state)
5. **Flexibility Preserved** - Domain-centric design allows future platform-wide consolidation

---

## How to Use This Index

### For Implementers

1. Read relevant ADRs before implementing a phase
2. Follow patterns established in referenced parent ADRs
3. Validate implementation against consequences listed in each ADR

### For Reviewers

1. Check PRs against applicable ADRs
2. Verify `action: recreate` is used when config changes (ADR-FE001-005)
3. Confirm stream_type is set correctly for new streams (ADR-FE001-003, ADR-FE001-004)

### For Future Decisions

1. Check this index for existing decisions before creating new ADRs
2. Reference relevant FE-001 ADRs when designing Gold layer extensions
3. Update this index when adding new ADRs

---

## Document History

| Date | Change |
|------|--------|
| 2026-02-04 | Initial creation with 5 ADRs |

---

*Index maintained by NDP Architecture Team*
