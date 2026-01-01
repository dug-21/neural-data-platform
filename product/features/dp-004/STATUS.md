# DP-004: Status

## Current Phase

**Architecture** - ADR approved, ready for implementation

## Progress

| Phase | Status | Notes |
|-------|--------|-------|
| Specification | Complete | See SCOPE.md |
| Architecture | Complete | ADR-001 approved |
| Pseudocode | Not Started | - |
| Refinement | Not Started | - |
| Completion | Not Started | - |

## Key Decisions

- [x] Bronze stores raw JSON payloads (ADR-001)
- [x] Context stored as separate column, not in payload
- [x] Tall format moves to Silver layer ETL
- [ ] Silver ETL approach (future ADR)

## Blockers

None

## Next Steps

1. Implement `RawDataPoint` struct in `core/src/traits.rs`
2. Update Parquet storage for new schema
3. Update sources to emit `RawDataPoint`
4. Add integration tests
5. Plan Silver ETL feature (dp-005?)
