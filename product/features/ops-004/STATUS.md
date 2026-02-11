# OPS-004: Memory Instrumentation for BUG-005

**Feature:** Memory diagnostics and instrumentation for air-quality-app RSS drift
**Status:** Implementation Complete
**Version Target:** v1.1.22
**Created:** 2026-02-11
**Last Updated:** 2026-02-11 by ndp-scrum-master

## Context

GitHub Issue #16 reports that after v1.1.21 (air-018 Polars removal) fixed BUG-004's DataFrame leak, a separate, slower RSS drift of ~5-16 MiB/hr persists. Over a 13.5-hour production run, RSS grew from 108 MiB to 216 MiB. The growth rate decelerates over time, suggesting fragmentation rather than a true linear leak.

This feature adds instrumentation to diagnose the root cause and guide mitigation.

## Related

- **GitHub Issue**: #16 (BUG-005: Slow RSS drift ~16 MiB/hr causes container OOM in ~24h)
- **BUG-004**: `product/features/air-017/bugs/BUG-004-accumulator-memory-leak.md` (fixed in v1.1.21)
- **air-018**: `product/features/air-018/` (Polars -> arrow-rs, deployed as v1.1.21)

## Phase Tracking

| Phase | Status | Date | Artifacts |
|-------|--------|------|-----------|
| Scope | Complete | 2026-02-11 | SCOPE.md |
| Specification | Complete | 2026-02-11 | specification/SPECIFICATION.md |
| Pseudocode | Complete | 2026-02-11 | pseudocode/PSEUDOCODE.md |
| Architecture | Complete | 2026-02-11 | architecture/ADR-001-memory-instrumentation.md |
| Refinement | Complete | 2026-02-11 | (London TDD iteration) |
| Completion | Complete | 2026-02-11 | (test suite verified) |
| Implementation | Complete | 2026-02-11 | 3 new files, 3 modified files |
| Testing | Complete | 2026-02-11 | 895 lib tests (32 new) |
| Release | Pending | -- | v1.1.22 manifest created |
| Deployment | Not Started | -- | -- |

## Progress

- [x] SCOPE.md created
- [x] SPARC Specification complete
- [x] SPARC Pseudocode complete
- [x] SPARC Architecture complete
- [x] SPARC Refinement complete (TDD iterations)
- [x] SPARC Completion complete
- [x] Implementation complete (diagnostics module, accumulator methods, bronze integration)
- [x] All tests passing (895 lib, 0 new failures)
- [x] Release manifest created (v1.1.22)
- [x] Changelog updated
- [ ] Deployed to production

## Implementation Summary

### Files Created
- `core/src/diagnostics/mod.rs` — module root with re-exports
- `core/src/diagnostics/memory.rs` — MemoryDiagnostics, SmapsSummary, MallocStats, MemoryTrend, parse/read functions, 27 unit tests

### Files Modified
- `core/src/lib.rs` — added `pub mod diagnostics;` and re-exports
- `core/src/storage/accumulator.rs` — added `hash_capacity()`, `vec_capacity()`, `vec_len()`, 5 unit tests; `clear()` now uses `HashMap::new()`
- `core/src/subscribers/bronze.rs` — enhanced heartbeat with diagnostics, enhanced snapshot with allocator stats, per-source delta tracking, subsystem memory attribution, trend summary, moved `read_process_rss_mib()` to diagnostics

### Test Results
- 27 new diagnostics module tests
- 5 new accumulator capacity tests
- 37 existing bronze tests pass (0 regressions)
- 4 integration tests pass (0 regressions)
- 895 total platform-core lib tests

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| -- | -- | No bugs filed |

## Dependencies

| Dependency | Status |
|------------|--------|
| air-018 (Polars removal) | Deployed (v1.1.21) |
| BUG-004 fix verification | Complete (Polars leak confirmed fixed) |
| ops-003 (shared constants) | Complete (v1.1.20) |

## Reports

| Date | Type | File |
|------|------|------|
| 2026-02-11 | Memory Analysis | `reports/memory-analysis-v1.1.21.md` |
