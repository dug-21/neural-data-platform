# AIR-016 Status

| Field | Value |
|-------|-------|
| Feature | AIR-016: Parquet Memory Efficiency |
| Status | Config mitigation deployed; code changes on hold |
| Version | v1.1.13 (config change only) |
| Next | Monitor RSS; permanent fix via air-017 |

## What Shipped

**Config change only** (`cc33b5b`): increased `flush_interval_secs` from 5 to 30 in `config/base/platform.yaml`. Reduces read-modify-write frequency by 6x. No code changes.

## What Did NOT Ship

The planned Phase 1 allocator fix (jemalloc + drop + malloc_trim) has not been implemented. Swarm review identified:

- `malloc_trim(0)` is dead code under jemalloc — should be removed from the plan
- The config change buys time with zero risk
- The real fix is architectural (eliminate read-modify-write entirely), now scoped as **air-017**

## Relationship to air-017

air-017 (Bronze Write-Ahead Architecture) replaces the read-modify-write pattern with WAL-on-receipt + in-memory accumulator + periodic Parquet snapshot. This makes the allocator fix unnecessary — if air-017 ships, there is no read-modify-write to fragment the heap.

If RSS grows unacceptably before air-017 is ready, the jemalloc + drop fix from Phase 1 remains a valid stopgap.

## SPARC Progress

- [x] Specification — `specification/SPEC.md`
- [x] Pseudocode — `pseudocode/PSEUDO.md`
- [x] Architecture — `architecture/ADR-001-sidecar-files.md`, `architecture/ADR-002-allocator-fix.md`
- [x] Refinement — `refinement/REFINEMENT.md`
- [x] Completion — `completion/COMPLETION.md`

## Swarm Analysis (2026-02-07)

**Round 1**: Three architect agents analyzed file-level options (A, C, D) in parallel. All converged on per-flush sidecar files.

**Round 2**: Re-analysis identified that the architecture works correctly — the root cause is glibc malloc not returning freed heap pages to the OS. Allocator-level fix is simpler, lower risk, and preserves existing file format.

**Round 3**: Human review identified that increasing the flush interval (Option D — zero code, config only) should be tried first. Accepted and deployed. Long-term architectural fix scoped as air-017.
