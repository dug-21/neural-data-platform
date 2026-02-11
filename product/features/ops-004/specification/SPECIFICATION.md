# OPS-004 Specification: BUG-005 Memory Instrumentation

> **Feature ID:** ops-004
> **SPARC Phase:** Specification
> **Author:** ndp-architect
> **Created:** 2026-02-11
> **Status:** Draft
> **Depends on:** air-018 (Polars removal, deployed v1.1.21)
> **Related:** BUG-005, GitHub Issue #16

---

## 1. Problem Summary

After air-018 eliminated the Polars DataFrame leak (BUG-004), the air-quality-app container RSS still grows from ~104 MiB at startup to ~229 MiB over 13.5 hours on Raspberry Pi 5 (glibc 2.36, cgroup v2, 512 MiB container limit). The growth rate decelerates: ~16 MiB/hr in the first 3 hours, ~9 MiB/hr average, ~5 MiB/hr by hour 12+. This deceleration pattern is consistent with glibc malloc fragmentation (large free blocks cannot be returned to OS) rather than a true unbounded leak.

Key evidence from production (2026-02-10, 13.5 hour run):

| Metric | Startup | After 13.5h | Delta |
|--------|---------|-------------|-------|
| RSS (VmRSS) | 104.3 MiB | 229.4 MiB | +125.1 MiB |
| accumulator_mib | 4.1 MiB | 8.0 MiB | +3.9 MiB |
| **Unattributed** | -- | -- | **+121.2 MiB** |

The existing heartbeat log reports only RSS, accumulator_mib, accumulator_count, wal_mib, and wal_errors. There is no visibility into where the other 121 MiB resides: glibc arena fragmentation, reqwest HTTP connection pools, rumqttc MQTT buffers, tokio runtime overhead, HashMap/Vec capacity overhead, or Silver subscriber state.

This specification defines instrumentation to attribute RSS to specific subsystems so that targeted mitigation (Phase 2) can be designed with data rather than guesses.

---

## 2. Functional Requirements

### FR-01: Enhanced Heartbeat Memory Report

Extend the existing heartbeat log (in `core/src/subscribers/bronze.rs`, line 440-449) with:

1. **Accumulator capacity breakdown:**
   - `accum_hashmap_capacity`: `self.accumulator.points.capacity()` (HashMap bucket count)
   - `accum_vec_capacity_total`: sum of `Vec::capacity()` across all source Vecs
   - `accum_vec_len_total`: sum of `Vec::len()` across all source Vecs (already reported as `accumulator_count`)
   - `accum_wasted_capacity_bytes`: estimated bytes in allocated-but-unused Vec slots

2. **Process-level allocator stats (glibc mallinfo2):**
   - `arena_mib`: total non-mmapped memory in arenas (mallinfo2.arena)
   - `hblkhd_mib`: total mmapped memory (mallinfo2.hblkhd)
   - `uordblks_mib`: total allocated space in arenas (mallinfo2.uordblks)
   - `fordblks_mib`: total free space in arenas -- this is the fragmentation metric (mallinfo2.fordblks)
   - `fragmentation_pct`: `fordblks / (arena + hblkhd) * 100` -- ratio of free-but-unreturnable memory

3. **RSS decomposition from /proc/self/smaps_rollup:**
   - `rss_anon_mib`: Anonymous RSS (heap + stack)
   - `rss_file_mib`: File-backed RSS (shared libs, mmap)
   - `rss_shmem_mib`: Shared memory RSS

These new fields are appended to the existing heartbeat `info!()` structured log. No new log lines are created.

### FR-02: Accumulator Capacity Introspection Methods

Add the following public methods to `core/src/storage/accumulator.rs`:

```rust
/// Total HashMap bucket capacity (not count of entries).
pub fn hashmap_capacity(&self) -> usize

/// Sum of Vec::capacity() for all source Vecs.
pub fn total_vec_capacity(&self) -> usize

/// Estimated bytes wasted in unused Vec capacity slots.
pub fn wasted_capacity_bytes(&self) -> usize
```

The `memory_estimate_bytes()` method already exists but does not account for Vec capacity vs len. These new methods expose the gap between allocated capacity and used length, which is the likely source of some of the unattributed RSS.

### FR-03: Allocator Stats Module

Create `core/src/diagnostics/allocator.rs` with:

```rust
/// glibc mallinfo2 result (Linux only, no-op on other platforms).
pub struct AllocatorStats {
    pub arena_bytes: usize,      // Non-mmapped arena space
    pub hblkhd_bytes: usize,     // Mmapped space
    pub uordblks_bytes: usize,   // Allocated in arenas
    pub fordblks_bytes: usize,   // Free in arenas (fragmentation)
}

/// Read glibc mallinfo2() via FFI. Returns None on non-Linux.
pub fn read_allocator_stats() -> Option<AllocatorStats>
```

Implementation uses `extern "C" { fn mallinfo2() -> mallinfo2; }` FFI call to glibc. The `mallinfo2` struct matches the glibc definition (all `size_t` fields). This is the same FFI pattern already used for `malloc_trim(0)` in `bronze.rs` line 248-251.

No new crate dependencies. The `libc` crate is NOT needed; raw `extern "C"` FFI is used directly as the existing code does for `malloc_trim`.

### FR-04: Smaps Rollup Module

Create `core/src/diagnostics/smaps.rs` with:

```rust
/// RSS decomposition from /proc/self/smaps_rollup (Linux only).
pub struct SmapsRollup {
    pub rss_anon_bytes: u64,   // Anonymous pages (heap, stack)
    pub rss_file_bytes: u64,   // File-backed pages (libs, mmap)
    pub rss_shmem_bytes: u64,  // Shared memory pages
}

/// Read /proc/self/smaps_rollup. Returns None on non-Linux or if unavailable.
pub fn read_smaps_rollup() -> Option<SmapsRollup>
```

Implementation parses `/proc/self/smaps_rollup` (single file read, no directory traversal). Fields are `Rss_Anon`, `Rss_File`, `Rss_Shmem` in kB. This is a lightweight operation (single file, ~20 lines to parse).

### FR-05: Per-Snapshot Memory Delta Tracking

Extend the snapshot diagnostic logging (in `bronze.rs`, line 213-274) to include allocator stats before and after Parquet writes:

```
Snapshot starting:  rss_before, arena_before, fordblks_before
Snapshot writes:    rss_after_writes, arena_after_writes, fordblks_after_writes
After malloc_trim:  rss_after_trim, arena_after_trim, fordblks_after_trim
Net deltas:         rss_delta, arena_delta, fordblks_delta (fragmentation change)
```

This reveals whether snapshot-induced Parquet/Arrow allocations are fragmenting the heap (fordblks grows) even when RSS appears stable (malloc_trim returns pages).

### FR-06: Diagnostics Module Structure

Create `core/src/diagnostics/mod.rs` to organize all diagnostic code:

```
core/src/diagnostics/
    mod.rs          -- re-exports
    allocator.rs    -- FR-03: mallinfo2 FFI
    smaps.rs        -- FR-04: /proc/self/smaps_rollup parser
```

Move the existing `read_process_rss_mib()` function from `bronze.rs` (line 545-558) into `core/src/diagnostics/mod.rs` as a public function. Update `bronze.rs` to import from the new module. This consolidates all memory diagnostic code in one place.

### FR-07: Subsystem Memory Attribution Log

Add a periodic (every 5 minutes, aligned with existing heartbeat timer) subsystem attribution log that reports estimated memory per category. This is a new structured log line separate from the heartbeat, emitted at `info` level:

```
memory_attribution:
  accumulator_mib: 8.0      (from memory_estimate_bytes)
  accum_overhead_mib: 2.1   (capacity - used estimate)
  wal_mib: 0.3              (from wal.file_size_bytes -- mmap'd)
  arena_fragmentation_mib: 45.2  (fordblks from mallinfo2)
  mmapped_mib: 12.0         (hblkhd from mallinfo2)
  file_backed_rss_mib: 18.0 (from smaps_rollup)
  unattributed_mib: 35.0    (rss - sum of above)
```

The `unattributed_mib` is the residual after subtracting known categories. Over time, watching which category grows identifies the leak source. If `arena_fragmentation_mib` grows monotonically, glibc fragmentation is confirmed. If `unattributed_mib` grows, a subsystem buffer not yet instrumented is the cause.

### FR-08: jemalloc Evaluation Criteria Documentation

Document in the ADR (not in code) the criteria for switching to jemalloc as a Phase 2 mitigation:

1. `arena_fragmentation_mib` exceeds 30% of RSS for 24+ hours
2. `unattributed_mib` is stable (ruling out subsystem leaks)
3. jemalloc must compile for aarch64-unknown-linux-gnu
4. jemalloc binary must not increase container image by more than 5 MiB
5. No prior crash history with jemalloc on this platform (air-018 documented crashes with mimalloc; jemalloc was not tested)

---

## 3. Non-Functional Requirements

### NFR-01: Instrumentation Overhead

All diagnostic code MUST add less than 1% CPU overhead to the 30-second heartbeat cycle. Specifically:

- `mallinfo2()` FFI: <1ms per call (kernel syscall, no userspace computation)
- `/proc/self/smaps_rollup` read: <1ms per call (single procfs file)
- Accumulator capacity iteration: O(n) where n = source_count (currently ~6), <0.1ms
- No allocation in the diagnostic code paths themselves (stack-only computation)

### NFR-02: No New Crate Dependencies

All instrumentation uses:
- Raw `extern "C"` FFI for glibc (pattern already in codebase at `bronze.rs:248`)
- `std::fs::read_to_string` for procfs (pattern already in codebase at `bronze.rs:546`)
- `#[cfg(target_os = "linux")]` guards with no-op fallbacks on other platforms

### NFR-03: Pi-Compatible

All code must compile and run on:
- Target: aarch64-unknown-linux-gnu
- Kernel: 6.14+ (Pi 5)
- glibc: 2.36+ (mallinfo2 available since glibc 2.33)
- Docker: cgroup v2 with 512 MiB memory limit

### NFR-04: No Behavioral Changes

Instrumentation MUST NOT change:
- Data flow (ingestion, accumulation, snapshot, WAL)
- Timing (snapshot interval, flush interval)
- Memory allocation patterns (no new long-lived allocations)
- Error handling or recovery behavior

The only observable change is additional structured fields in existing log lines and one new periodic attribution log line.

---

## 4. Files Changed

| File | Change Type | Description |
|------|-------------|-------------|
| `core/src/diagnostics/mod.rs` | **New** | Diagnostics module root; moved `read_process_rss_mib()` here |
| `core/src/diagnostics/allocator.rs` | **New** | FR-03: glibc mallinfo2 FFI wrapper |
| `core/src/diagnostics/smaps.rs` | **New** | FR-04: /proc/self/smaps_rollup parser |
| `core/src/lib.rs` | Modified | Add `pub mod diagnostics;` |
| `core/src/storage/accumulator.rs` | Modified | FR-02: Add capacity introspection methods |
| `core/src/subscribers/bronze.rs` | Modified | FR-01, FR-05, FR-06, FR-07: Enhanced heartbeat and snapshot logs; import diagnostics module; remove local `read_process_rss_mib()` |

---

## 5. Acceptance Criteria

### AC-01: All Existing Tests Pass

All 874+ tests across the workspace continue to pass. No test regressions. The new diagnostic code is behind `#[cfg(target_os = "linux")]` guards and has no-op fallbacks, so tests on all platforms work.

### AC-02: Heartbeat Logs Include Allocator Stats

On Linux, the heartbeat log line includes `arena_mib`, `fordblks_mib`, `uordblks_mib`, and `fragmentation_pct` fields. On non-Linux, these fields show "N/A".

### AC-03: Heartbeat Logs Include Accumulator Capacity

The heartbeat log line includes `accum_vec_capacity_total` and `accum_wasted_capacity_bytes` fields.

### AC-04: Snapshot Logs Include Before/After Allocator Stats

The snapshot diagnostic logging includes `fordblks_before_mib`, `fordblks_after_writes_mib`, and `fordblks_after_trim_mib` fields showing heap fragmentation changes through the snapshot cycle.

### AC-05: Smaps Rollup Parsed Correctly

Unit test: given a mock `/proc/self/smaps_rollup` content string, `parse_smaps_rollup()` extracts `Rss_Anon`, `Rss_File`, and `Rss_Shmem` correctly. Integration test (Linux only): `read_smaps_rollup()` returns `Some(...)` with plausible values.

### AC-06: Attribution Log Emitted

The `memory_attribution` log line is emitted at the heartbeat interval with all categorized fields and a computed `unattributed_mib` residual.

### AC-07: read_process_rss_mib Moved

`read_process_rss_mib()` is no longer defined in `bronze.rs`. It is imported from `core::diagnostics`. All callers updated.

### AC-08: Accumulator Capacity Methods Unit Tested

Unit tests for `hashmap_capacity()`, `total_vec_capacity()`, and `wasted_capacity_bytes()` verify correct behavior with empty accumulator, single-source, and multi-source data.

### AC-09: No New Dependencies in Cargo.toml

`core/Cargo.toml` has no new entries in `[dependencies]` or `[dev-dependencies]` compared to before this change.

---

## 6. Test Plan

### 6.1 Unit Tests (New)

| Test | File | Purpose |
|------|------|---------|
| `test_allocator_stats_struct_fields` | `diagnostics/allocator.rs` | Verify `AllocatorStats` struct has all expected fields |
| `test_smaps_rollup_parse` | `diagnostics/smaps.rs` | Parse mock smaps_rollup content, verify extracted values |
| `test_smaps_rollup_missing_fields` | `diagnostics/smaps.rs` | Handle missing fields gracefully (return None or 0) |
| `test_accumulator_hashmap_capacity` | `storage/accumulator.rs` | After adding 100 points to 3 sources, hashmap_capacity() >= 3 |
| `test_accumulator_total_vec_capacity` | `storage/accumulator.rs` | total_vec_capacity() >= total point count |
| `test_accumulator_wasted_capacity_nonzero` | `storage/accumulator.rs` | After Vec doubling, wasted_capacity_bytes() > 0 |
| `test_accumulator_wasted_capacity_empty` | `storage/accumulator.rs` | Empty accumulator has wasted_capacity_bytes() == 0 |

### 6.2 Integration Tests (Linux Only)

| Test | File | Purpose |
|------|------|---------|
| `test_read_allocator_stats_linux` | `diagnostics/allocator.rs` | `read_allocator_stats()` returns Some with arena > 0 |
| `test_read_smaps_rollup_linux` | `diagnostics/smaps.rs` | `read_smaps_rollup()` returns Some with rss_anon > 0 |
| `test_read_process_rss_moved` | `diagnostics/mod.rs` | `read_process_rss_mib()` works from new location |

### 6.3 Production Verification (Post-Deploy)

On Pi deployment with production workload:

1. Confirm heartbeat logs contain new allocator and capacity fields
2. Confirm snapshot logs contain before/after fordblks delta
3. Confirm memory_attribution log line emitted every heartbeat interval
4. Run for 4+ hours and visually inspect which category grows
5. Compute: does `arena_fragmentation_mib + accumulator + wal + file_backed` account for >80% of RSS? If yes, instrumentation is sufficient. If not, additional subsystem instrumentation needed in Phase 2.

---

## 7. Implementation Order

| Step | Description | Effort |
|------|-------------|--------|
| 1 | Create `core/src/diagnostics/mod.rs`; move `read_process_rss_mib()` from bronze.rs | Small |
| 2 | Create `core/src/diagnostics/allocator.rs` with mallinfo2 FFI (FR-03) | Small |
| 3 | Create `core/src/diagnostics/smaps.rs` with smaps_rollup parser (FR-04) | Small |
| 4 | Add accumulator capacity methods (FR-02) | Small |
| 5 | Enhance heartbeat logging with allocator + capacity stats (FR-01) | Medium |
| 6 | Enhance snapshot logging with before/after allocator stats (FR-05) | Medium |
| 7 | Add memory attribution log line (FR-07) | Small |
| 8 | Add unit tests for all new code | Medium |
| 9 | Run full test suite, verify no regressions | Small |

Steps 1-4 are independent and can be done in any order. Steps 5-7 depend on steps 1-4. Step 8 can be done alongside steps 5-7.

---

## 8. Out of Scope

- Switching to jemalloc or any alternative allocator (Phase 2, pending Phase 1 data)
- Adding `shrink_to_fit()` calls to accumulator Vecs (Phase 2, if data confirms Vec overhead)
- Instrumenting reqwest HTTP connection pool or rumqttc MQTT buffers (Phase 2, if unattributed_mib grows)
- Grafana dashboard for memory metrics (future ops feature)
- Prometheus/OpenTelemetry metrics export (future ops feature)
- Any changes to the Silver subscriber or TimescaleDB connection pool

---

## 9. Patterns Applied

| Pattern ID | Name | How Applied |
|------------|------|-------------|
| 26 | `architecture:bug-fix-wal-only-bronze` | Confirmed BUG-004 fix scope; this feature addresses the REMAINING growth not covered by air-018 |
| 31 | `architecture:deprecated-approaches` | Verified mimalloc/jemalloc were not previously tested successfully; instrumentation-first approach avoids repeating allocator swap failures |
