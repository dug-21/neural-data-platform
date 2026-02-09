# AIR-016 Specification: Parquet Memory Efficiency

## Phased Approach

### Phase 1 (This Release): Allocator Fix + Explicit Memory Management
Fix the memory retention problem at the allocator level. The read-modify-write architecture works correctly — data is allocated, used, and freed — but glibc's malloc never returns freed heap pages to the OS. Fix this with jemalloc + explicit `drop()` scoping + `malloc_trim` fallback.

### Phase 2 (Future, if needed): Per-Flush Sidecar Files
If Phase 1 doesn't bring RSS under ~200 MiB sustained, replace read-modify-write with per-flush sidecar files. Architecture analysis and ADR preserved in `architecture/ADR-001-sidecar-files.md`.

---

## Phase 1 Specification (ACTIVE)

### Root Cause

The air-quality-app uses glibc's default malloc. Every 30s flush cycle in `append_to_parquet()` and `append_to_raw_parquet()` creates a burst of allocations:

```
Per-flush memory lifecycle:
  1. Read file → Polars DataFrame (Arrow buffers)           ~X MB via brk()
  2. Deserialize rows → Vec<RawDataPoint> (String clones)   ~X MB via brk()
  3. df dropped                                              "freed" to glibc free-list
  4. write_raw_parquet(all_points) in spawn_blocking:
     a. Build column Vecs (clone strings AGAIN)              ~X MB via brk()
     b. points still alive during column build               peak: ~2X MB
     c. Build DataFrame, write Parquet, drop everything      "freed" to glibc free-list
```

All allocations ARE freed (no Rust-level leak). But glibc malloc:
- Uses `brk()`/`sbrk()` for allocations < 128KB (the MMAP_THRESHOLD)
- The heap high-water mark NEVER recedes unless the TOP of the heap is free
- Fragmentation from interleaved small String allocations + larger Vec/Arrow buffers prevents the heap top from being free
- `malloc_trim()` is never called

Over hundreds of 30s cycles, RSS ratchets up monotonically: 96 MiB → 490 MiB in days.

### Fix Components

#### 1. tikv-jemallocator as global allocator

jemalloc uses per-thread arenas with `mmap`/`munmap` and aggressively returns pages via `madvise(MADV_DONTNEED)`. This is the #1 recommendation from the Polars project for long-running processes.

**Files changed:**
- `apps/air-quality-app/Cargo.toml` — add `tikv-jemallocator` dependency
- `apps/air-quality-app/src/main.rs` — 3-line global allocator declaration

#### 2. Explicit `drop(points)` in write functions

Inside `write_parquet()` and `write_raw_parquet()` spawn_blocking closures, the input `points` Vec stays alive while column Vecs are built from it. Both hold cloned string data simultaneously, doubling peak memory. Explicitly dropping `points` after column extraction halves the peak.

**Files changed:**
- `core/src/storage/parquet.rs` — add `drop(points)` after column Vec construction in both `write_parquet()` (after line 125) and `write_raw_parquet()` (after line 530)

#### 3. malloc_trim fallback (belt-and-suspenders)

After each flush cycle completes, call `libc::malloc_trim(0)` to force glibc to release free pages. This is a no-op under jemalloc (harmless) but provides a safety net if jemalloc is ever removed or on platforms where it's unavailable.

**Files changed:**
- `core/src/storage/parquet.rs` — add `malloc_trim(0)` call at end of `append_to_parquet()` and `append_to_raw_parquet()`
- `core/Cargo.toml` — add `libc` dependency (if not already present)

### Changes Summary

| File | Change | Lines |
|------|--------|-------|
| `apps/air-quality-app/Cargo.toml` | Add `tikv-jemallocator` dep | +1 |
| `apps/air-quality-app/src/main.rs` | Global allocator declaration | +3 |
| `core/Cargo.toml` | Add `libc` dep (if needed) | +1 |
| `core/src/storage/parquet.rs` | `drop(points)` in write_parquet | +1 |
| `core/src/storage/parquet.rs` | `drop(points)` in write_raw_parquet | +1 |
| `core/src/storage/parquet.rs` | `malloc_trim(0)` after append_to_parquet | +4 |
| `core/src/storage/parquet.rs` | `malloc_trim(0)` after append_to_raw_parquet | +4 |

**Total: ~15 lines added. Zero architectural changes. Zero file format changes.**

### What Does NOT Change

- File naming: `readings.parquet` / `data.parquet` — unchanged
- File format: single daily Parquet file — unchanged
- Read path: `query()`, `query_raw()` — unchanged
- Write path logic: read-modify-write — unchanged (just memory management around it)
- Store trait interface — unchanged
- Silver ETL — unchanged
- MCP server — unchanged
- WAL — unchanged

### Expected Memory Profile

| Metric | Before (glibc malloc) | After (jemalloc + drop) |
|--------|----------------------|------------------------|
| RSS after 1 hour | ~150 MiB (growing) | ~100-120 MiB (stable) |
| RSS after 12 hours | ~350 MiB (growing) | ~100-120 MiB (stable) |
| RSS after 24 hours | ~490 MiB (OOM risk) | ~100-120 MiB (stable) |
| Per-flush peak | ~2X file size | ~1X file size (with drop) |
| Memory return to OS | Never (glibc fragmentation) | Prompt (jemalloc arenas) |

### Testing Strategy

1. **Build verification**: Cargo build succeeds with jemalloc on aarch64 (Pi target)
2. **Existing tests pass**: All 556+ tests in ndp-gold-ddl and ndp-validate unchanged
3. **Parquet tests pass**: All ~40 tests in `parquet.rs` unchanged (no logic changes)
4. **Memory monitoring**: Deploy to integration env, track RSS over 24h via `docker stats`
5. **Correctness**: Write + read cycle produces identical data (no behavioral change)

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| jemalloc cross-compile fails for aarch64 | Low | High | tikv-jemallocator supports aarch64-linux; fall back to malloc_trim only |
| Binary size increase | Low | Low | jemalloc adds ~200KB to binary |
| Performance regression from jemalloc | Very Low | Low | jemalloc is typically faster than glibc malloc for this workload |
| malloc_trim latency spike | Low | Low | malloc_trim(0) is typically <1ms |

### Constraints Verified

| Constraint (from SCOPE) | Status |
|--------------------------|--------|
| WAL continues to work as-is | Yes — no write path logic changes |
| Store trait interface unchanged | Yes — no signature changes |
| Read path works unchanged | Yes — no read path changes at all |
| Silver ETL remains compatible | Yes — no file format changes |
| Parquet files same naming | Yes — `readings.parquet` / `data.parquet` unchanged |

### Out of Scope

- Sidecar files (Phase 2 — deferred pending Phase 1 measurement)
- Polars removal from write path
- MQTT unbounded cache fix (separate issue)
- EventBus capacity tuning (separate issue)
- Compaction (not needed — single daily file preserved)
