# DP-004: Bronze Layer Raw JSON Schema

## Current Phase

**Refinement** - Implementation ready to begin

## Progress

| Phase | Status | Notes |
|-------|--------|-------|
| Specification | Complete | See SCOPE.md |
| Architecture | Complete | ADR-001 approved 2026-01-01 |
| Pseudocode | Skipped | Implementation defined in ADR |
| Refinement | In Progress | Implementation phase |
| Completion | Not Started | - |

## SPARC Checklist

- [x] SCOPE.md created
- [x] SPARC Specification complete
  - [x] REQUIREMENTS.md - Functional and non-functional requirements
  - [x] ACCEPTANCE_CRITERIA.md - Given/When/Then test scenarios
  - [x] USER_STORIES.md - Developer-focused implementation stories
  - [x] GLOSSARY.md - Term definitions (RawDataPoint, Bronze/Silver layers)
- [x] SPARC Architecture complete (ADR-001)
- [x] SPARC Completion planning complete
  - [x] IMPLEMENTATION_CHECKLIST.md - 4-phase implementation plan
  - [x] DEPLOYMENT_PLAN.md - Rollback strategy and monitoring
  - [x] INTEGRATION_TESTS.md - E2E test scenarios and benchmarks
  - [x] FUTURE_WORK.md - dp-005 Silver ETL and beyond
- [ ] Implementation: RawDataPoint struct
- [ ] Implementation: Parquet storage update
- [ ] Implementation: Source updates
- [ ] Tests passing
- [ ] Documentation updated
- [ ] Deployed to production

## Key Decisions

- [x] Bronze stores raw JSON payloads (ADR-001)
- [x] Context stored as separate column, not in payload
- [x] Tall format moves to Silver layer ETL
- [ ] Silver ETL approach (future feature dp-005)

## Implementation Tasks

| Task | Priority | Agent | Status |
|------|----------|-------|--------|
| Add `RawDataPoint` struct to `core/src/traits.rs` | P0 | ndp-rust-dev | Pending |
| Update `ParquetStore` schema (5 columns) | P0 | ndp-parquet-dev | Pending |
| Add `RawStore` trait for raw data path | P1 | ndp-rust-dev | Pending |
| Update sources to emit `RawDataPoint` | P1 | ndp-rust-dev | Pending |
| Unit tests for `RawDataPoint` serde | P0 | ndp-tester | Pending |
| Integration tests for Parquet roundtrip | P1 | ndp-tester | Pending |

> **Note**: Backward compatibility NOT required. Platform is <1 week old; existing data can be retired.

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| AIR-009: ndp_id/context fields | Complete | Already in TimeSeriesPoint |
| Parquet storage module | Operational | `core/src/storage/parquet.rs` |
| Current tall schema working | Verified | 6 columns currently |

## Blockers

**CRITICAL: Build Broken** (2026-01-01)

The codebase currently does not compile due to 3 errors in `core/src/sources/http_poll.rs`:

1. **Lifetime/async issue at line 674** - `fetch_raw_batch()` function has closure lifetime mismatch
2. **Lifetime/async issue at line 792** - `tokio::spawn` has `Send` bound not satisfied
3. **FnOnce implementation not general enough** - Related to sensor config iteration

**Root Cause**: Recent changes (likely dp-004 implementation) introduced async lifetime issues when iterating over `&SensorConfig` references within async closures.

**Impact**: All development blocked until resolved.

**Resolution Path**:
- Clone sensor configs before async boundary
- Fix lifetime bounds on iterator closures
- Ensure `Mutex<bool>` reference doesn't cross await points

**Assigned**: ndp-rust-dev (P0)

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Storage increase | Low | Parquet compression mitigates |

> **Simplified**: No backward compatibility needed. Clean cutover to new schema.

## Related Features

- **AIR-009**: Provides ndp_id/context fields (completed)
- **DP-003**: MQTT multi-subscription (completed)
- **DP-005** (future): Silver layer ETL pipeline

## Branch

`feature/dp-004` (to be created)

## Completion Phase Documents

| Document | Description |
|----------|-------------|
| [IMPLEMENTATION_CHECKLIST.md](./completion/IMPLEMENTATION_CHECKLIST.md) | 4-phase implementation plan with detailed tasks |
| [DEPLOYMENT_PLAN.md](./completion/DEPLOYMENT_PLAN.md) | Dual-write strategy, rollback procedures, monitoring |
| [INTEGRATION_TESTS.md](./completion/INTEGRATION_TESTS.md) | E2E test scenarios, performance benchmarks |
| [FUTURE_WORK.md](./completion/FUTURE_WORK.md) | dp-005 Silver ETL, Grafana migration, roadmap |

## Last Updated

2026-01-01 15:30 by ndp-scrum-master (Build blocker identified, STATUS updated)
