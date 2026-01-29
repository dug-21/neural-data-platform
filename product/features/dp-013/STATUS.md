# dp-013: Status Tracker

**Feature**: CSV Source Type & Dimension Tables
**Current Phase**: Ready for Implementation (SPARC-R Refinement)
**Started**: 2026-01-29
**Last Updated**: 2026-01-29

---

## Current Status: SPARC Planning Complete - Ready for Implementation

All SPARC planning phases (Specification, Pseudocode, Architecture) have been completed. The feature is now ready for TDD implementation in the Refinement phase.

Feature extends NDP configuration language to support:
1. CSV as a source type for stream configs (timeseries batch data)
2. Dimension table configs for reference/lookup data (new config type)

Initial deliverable: Entity Context dimension for air-012 Home Assistant integration.

---

## Phase Status

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| Scope Definition | **Complete** | 2026-01-29 | 2026-01-29 |
| Specification (SPARC-S) | **Complete** | 2026-01-29 | 2026-01-29 |
| Pseudocode (SPARC-P) | **Complete** | 2026-01-29 | 2026-01-29 |
| Architecture (SPARC-A) | **Complete** | 2026-01-29 | 2026-01-29 |
| Refinement (SPARC-R) | **Pending** | - | - |
| Completion (SPARC-C) | Pending | - | - |

---

## SPARC Phase Checklist

### Specification Phase - COMPLETE

- [x] Functional requirements documented
- [x] Non-functional requirements documented
- [x] CSV source type config schema defined
- [x] Dimension config schema defined
- [x] Interface contracts (traits) specified
- [x] Test scenarios documented
- [x] London TDD strategy defined

**Deliverable**: `specification/SPECIFICATION.md`

### Pseudocode Phase - COMPLETE

- [x] CSV adapter algorithm design
- [x] Dimension loader algorithm design
- [x] Error handling flows documented
- [x] Data flow pseudocode

**Deliverable**: `pseudocode/PSEUDOCODE.md`

### Architecture Phase - COMPLETE

- [x] ADR: CSV source integration approach
- [x] ADR: Dimension table loading strategy
- [x] Component diagram
- [x] Integration points documented
- [x] SQL patterns documented

**Deliverables**:
- `architecture/ARCHITECTURE.md`
- `architecture/ADR-001-csv-source-type.md`
- `architecture/ADR-002-dimension-tables.md`
- `architecture/SQL_PATTERNS.md`

### Refinement Phase - PENDING (Ready to Start)

- [ ] Unit tests for CSV adapter
- [ ] Unit tests for dimension loader
- [ ] Integration tests for Bronze ingest
- [ ] Integration tests for dimension sync
- [ ] Implementation following TDD

**Deliverables**:
- `refinement/TEST_STRATEGY.md` (complete - defines test approach)
- `refinement/RUST_IMPLEMENTATION.md` (complete - implementation guidance)
- `refinement/DATA_QUALITY.md` (complete - DQ integration)

### Completion Phase - PENDING

- [ ] CLI commands implemented (`ndp dimension list/sync`)
- [ ] `deploy.sh sync` integration
- [ ] Documentation updated
- [ ] Entity Context dimension deployed
- [ ] Acceptance criteria verified

**Deliverable**: `completion/ACCEPTANCE_CRITERIA.md` (complete - defines done criteria)

---

## Key Decisions Made

| Decision | Choice | ADR | Notes |
|----------|--------|-----|-------|
| CSV adapter location | `core/src/sources/csv.rs` | ADR-001 | Follow domain adapter pattern |
| Dimension storage | Direct to Silver (TimescaleDB) | ADR-002 | Dimensions bypass Bronze |
| Load strategy default | `truncate_and_load` | ADR-002 | Clean replace for dimensions |
| Timestamp parsing | chrono crate | ADR-001 | Align with existing sources |
| Config directory | `config/base/dimensions/` | SCOPE | Per initial scope |

---

## Dependencies

| Dependency | Status | Feature | Notes |
|------------|--------|---------|-------|
| Stream config system | Ready | air-001+ | Existing pattern to extend |
| Bronze layer (Parquet) | Ready | air-002 | CSV data lands here first |
| etcd config sync | Ready | dp-001 | `deploy.sh sync` to extend |
| Silver ETL | Ready | dp-006+ | Promotes CSV-sourced data |
| Entity schemas pattern | Ready | air-003 | Field mapping approach |
| Home Assistant streams | In Progress | air-012 | Consumer for entity_context |

---

## Team

| Role | Agent | Focus |
|------|-------|-------|
| Coordinator | ndp-scrum-master | Feature lifecycle, STATUS.md |
| Architect | ndp-architect | Config schema design, ADRs |
| Rust Implementation | ndp-rust-dev | CSV adapter, dimension loader |
| Config Integration | ndp-parquet-dev | Bronze layer updates |
| Silver Integration | ndp-timescale-dev | Dimension table DDL |
| Testing | ndp-tester | TDD strategy, integration tests |

---

## Scope Summary

### Part 1: CSV Source Type

Extend `source.type` in stream configs to support `csv`:
- `path`: CSV file location
- `timestamp_field`: Column containing timestamps
- `timestamp_format`: iso8601, epoch_seconds, or custom
- Data flows through Bronze (Parquet) like HTTP/MQTT sources

### Part 2: Dimension Table Configs

New config type in `config/base/dimensions/`:
- `dimension_id`: Unique identifier
- `target.table`: Silver table name
- `source.type: csv` with path
- `schema.fields[]`: Column definitions
- `load.strategy`: truncate_and_load or upsert

### Part 3: CLI Integration

- `ndp dimension list` - Show configured dimensions
- `ndp dimension sync <id>` - Load specific dimension
- `ndp dimension sync --all` - Load all dimensions
- `deploy.sh sync` processes dimensions automatically

---

## Initial Deliverable

**Entity Context** dimension for air-012:
- Table: `silver.entity_context`
- Fields: ndp_id, category, friendly_name, location_path, correlates_with, orientation
- Use case: Enrich Home Assistant state events with human-readable context

---

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| - | - | No bugs tracked yet |

---

## SPARC Documents

| Document | Location | Status |
|----------|----------|--------|
| SCOPE.md | `product/features/dp-013/SCOPE.md` | Complete |
| SPECIFICATION.md | `specification/SPECIFICATION.md` | Complete |
| PSEUDOCODE.md | `pseudocode/PSEUDOCODE.md` | Complete |
| ARCHITECTURE.md | `architecture/ARCHITECTURE.md` | Complete |
| ADR-001 | `architecture/ADR-001-csv-source-type.md` | Complete |
| ADR-002 | `architecture/ADR-002-dimension-tables.md` | Complete |
| SQL_PATTERNS.md | `architecture/SQL_PATTERNS.md` | Complete |
| TEST_STRATEGY.md | `refinement/TEST_STRATEGY.md` | Complete |
| RUST_IMPLEMENTATION.md | `refinement/RUST_IMPLEMENTATION.md` | Complete |
| DATA_QUALITY.md | `refinement/DATA_QUALITY.md` | Complete |
| ACCEPTANCE_CRITERIA.md | `completion/ACCEPTANCE_CRITERIA.md` | Complete |

---

## Related Patterns

| Pattern | Relevance |
|---------|-----------|
| `arch-domain-adapter-pattern` | CSV adapter follows Source trait |
| `config-gitops-pattern` | Dimension configs sync via deploy.sh |
| `arch-data-lake-layers` | Bronze (batch CSV) -> Silver (queryable) |
| `config-silver-metadata-fields` | Schema definition approach |

---

## Next Steps

1. **Start Refinement Phase**: Begin TDD implementation
   - Create test fixtures for CSV parsing
   - Write unit tests for CsvSource adapter
   - Implement CsvSource following tests

2. **Implement Core Types**: Add to neural-core
   - SourceType::Csv variant
   - DimensionConfig struct
   - LoadStrategy enum

3. **Implement Dimension Loader**:
   - CSV parsing for dimensions
   - truncate_and_load strategy
   - upsert strategy

4. **CLI Commands**:
   - `ndp dimension list`
   - `ndp dimension sync`
   - `ndp stream ingest`

5. **deploy.sh Integration**:
   - Add sync_dimensions() function
   - Add to sync workflow

---

*Status last updated: 2026-01-29 by ndp-scrum-master*
