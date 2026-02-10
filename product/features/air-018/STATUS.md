# AIR-018 Status

**Feature:** Eliminate Polars from Bronze Write Path
**Status:** Specification Complete
**Version Target:** v1.1.21
**Created:** 2026-02-09
**Last Updated:** 2026-02-10 by ndp-scrum-master

## Phase Tracking

| Phase | Status | Date | Artifacts |
|-------|--------|------|-----------|
| Scope | Complete | 2026-02-09 | SCOPE.md |
| Specification | Complete | 2026-02-10 | specification/SPECIFICATION.md, specification/TEST-STRATEGY.md |
| Pseudocode | Complete | 2026-02-10 | pseudocode/PSEUDOCODE.md |
| Architecture | Complete | 2026-02-10 | architecture/ADR-001-replace-polars-with-arrow-rs.md, architecture/DEPENDENCY-ANALYSIS.md |
| Refinement | Complete | 2026-02-10 | refinement/REFINEMENT.md |
| Completion | Complete | 2026-02-10 | completion/COMPLETION.md |
| Implementation | Not Started | -- | -- |
| Testing | Not Started | -- | -- |
| Release | Not Started | -- | v1.1.21 |
| Deployment | Not Started | -- | -- |

## Progress

- [x] SCOPE.md created
- [x] SPARC Specification complete
- [x] SPARC Pseudocode complete
- [x] SPARC Architecture complete
- [x] SPARC Refinement complete
- [x] SPARC Completion complete
- [ ] Implementation started
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Release manifest created
- [ ] Deployed to production

## Active Work

All SPARC planning artifacts complete (S, P, A, R, C). Ready for implementation.

## Files Changed (Planned)

| File | Change |
|------|--------|
| `core/src/storage/parquet.rs` | Replace all Polars API with arrow-rs + parquet crate |
| `core/src/error.rs` | CoreError::Polars -> CoreError::Arrow, new From impls |
| `core/Cargo.toml` | Remove polars, add arrow v57 + parquet v57 |
| `Cargo.toml` (workspace) | Add arrow + parquet to [workspace.dependencies] |

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| -- | -- | No bugs filed yet |

## Dependencies

| Dependency | Status |
|------------|--------|
| air-017 Phase 1 (WAL architecture) | Deployed (v1.1.18) |
| BUG-004 investigation | Complete (root cause: Polars DataFrame alloc) |
| ops-003 (shared constants) | Complete (v1.1.20) |

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-09 | Option B (full replacement) over Option A (write-only) | Maintenance burden of two styles, binary size, clean dep cut |
| 2026-02-09 | Alternative allocators rejected | jemalloc/mimalloc crash on Pi 5 + kernel 6.14 + cgroup v2 |
| 2026-02-10 | arrow v57 + parquet v57 | Matches transitive deps in Cargo.lock via Polars 0.35 |
| 2026-02-10 | CoreError::Polars -> CoreError::Arrow | Clean rename, no external crate matches on variant |
| 2026-02-10 | PATCH version bump (v1.1.21) | Bug fix (BUG-004 OOM), identical Parquet schema output |

## Branch

`feature/air-018`

## SPARC Artifacts Summary

| Document | Lines | Key Content |
|----------|-------|-------------|
| SCOPE.md | 97 | Problem statement, BUG-004 evidence, allocator failures, constraints |
| SPECIFICATION.md | 458 | 10 functional requirements, schema mapping, method-by-method plan, 7 acceptance criteria |
| TEST-STRATEGY.md | 383 | 35 existing test inventory, 6 new tests, regression prevention matrix |
| PSEUDOCODE.md | 1187 | 6 method pseudocodes (P-01 to P-06), helper functions, error handling |
| ADR-001 | 294 | Decision record with 4 alternatives considered, risk assessment |
| DEPENDENCY-ANALYSIS.md | 165 | Polars API surface (11 distinct APIs), transitive dep reduction (~40-50 crates) |
| COMPLETION.md | ~280 | Implementation checklist, release procedure, deployment plan, rollback |
