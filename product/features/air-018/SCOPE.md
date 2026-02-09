# AIR-018: Eliminate Polars from Bronze Write Path

> **Feature ID:** air-018
> **Created:** 2026-02-09
> **Status:** Scoping
> **Phase:** air (Foundation / Core)
> **Depends on:** air-017 Phase 1 (deployed v1.1.18)
> **Related:** BUG-004 (Bronze memory leak)

---

## Problem Statement

The Bronze layer uses Polars DataFrames to write Parquet files. On Raspberry Pi 5 running Ubuntu 25.x (kernel 6.14+) in a 512 MiB Docker container, Polars' allocation pattern causes a memory leak that will OOM the container within ~36 hours.

### Observed Behavior (BUG-004 Investigation, 2026-02-09)

Diagnostic logging in v1.1.19 confirmed two leaks:

| Leak | Rate | Source | Evidence |
|------|------|--------|----------|
| Chunk leak | +4.5 MiB per 30-min snapshot cycle | Polars DataFrame create/drop | RSS jumps +46.3 MiB during `write_raw_parquet`, `malloc_trim(0)` reclaims 41.9 MiB, 4.5 MiB stuck |
| Slow creep | +0.7 MiB per 30-min interval | Unknown (between snapshots) | Heartbeat RSS tracking shows steady growth |

At 48 snapshot cycles/day, the chunk leak alone adds ~216 MiB/day. Container starts at ~90-140 MiB and hits the 512 MiB limit within 24-36 hours.

### Alternative Allocators Tested and Failed

Both jemalloc and mimalloc were tested as drop-in replacements for glibc malloc. **Both caused the container to crash on Pi 5 / Ubuntu 25.x.** These are not viable on this platform.

| Allocator | Crate | Result | Failure Mode |
|-----------|-------|--------|--------------|
| **jemalloc** | `tikv-jemallocator 0.6` | Container hung | App started normally but data processing froze for 13+ minutes. MQTT and Silver stopped receiving data. Reverted. |
| **mimalloc** | `mimalloc 0.1` | Container died | App started normally, then exited silently. Docker stats showed 0B memory / 0 PIDs. Reverted. |
| **glibc + malloc_trim** | (current) | Partial | `malloc_trim(0)` reclaims 91% per cycle but 4.5 MiB/cycle still leaks. Not sustainable. |

**Root cause analysis for allocator failures:**

- `getconf PAGE_SIZE` returns 4096 on the Pi 5, ruling out the ARM64 page size mismatch theory
- jemalloc on kernel 6.14+ uses `MADV_FREE` (lazily reclaimed pages) instead of `MADV_DONTNEED`. Cgroup v2 memory accounting still counts `MADV_FREE` pages as RSS, so the OOM killer fires before the kernel reclaims them. Additionally, jemalloc's default `background_thread:true` can deadlock during init in Docker containers with restricted `/sys` access
- mimalloc failure mode is less understood but the container silently exits during data processing (not during startup)
- Both allocators work fine during the low-allocation startup phase; they fail during high-allocation Polars write operations

**Platform details:** Raspberry Pi 5, Ubuntu 25.x, kernel 6.14+, Docker with cgroup v2, 512 MiB container memory limit, ARM64 (Cortex-A76), 4KB pages.

**Do not retry alternative allocators without first verifying the specific kernel/cgroup interaction on the target Pi.**

---

## Recommendation

Replace Polars with direct `arrow-rs` + `parquet` crate usage in `core/src/storage/parquet.rs`.

### Why This Fixes the Problem

The memory leak comes from Polars' DataFrame create/drop cycle. Each snapshot:
1. Creates a DataFrame from in-memory data (allocates Arrow buffers internally)
2. Writes Parquet via `ParquetWriter`
3. Drops the DataFrame

glibc malloc does not return the fragmented heap pages from step 1-3 to the OS. The `arrow` + `parquet` crates use simpler allocation patterns (direct `RecordBatch` construction, no DataFrame overhead) that produce less fragmentation.

### Scope

- Replace `polars::prelude::*` usage in `core/src/storage/parquet.rs` with `arrow::array` + `parquet::arrow::ArrowWriter`
- Remove `polars` from `core`'s `[dependencies]` in `Cargo.toml`
- Preserve identical Parquet file schema and output (no downstream impact)
- Keep `polars` in `[dev-dependencies]` for read-side test assertions if needed
- Keep BUG-004 diagnostic logging in `bronze.rs` to verify the fix

### Out of Scope

- Removing Polars from `apps/air-quality-app` dev-dependencies or test code
- Removing Polars from Silver ETL or other crates
- Alternative allocator investigation (documented above as failed)
- Fixing the slow creep leak (0.7 MiB/30min) — separate investigation after chunk leak is resolved
- WAL or accumulator architecture changes (covered by air-017 Phases 2-3)

---

## Files Affected

| File | Change |
|------|--------|
| `core/src/storage/parquet.rs` | Replace Polars DataFrame writes with arrow-rs RecordBatch + ArrowWriter |
| `core/Cargo.toml` | Remove `polars` from `[dependencies]`, add `arrow` + `parquet` crates |
| `core/src/subscribers/bronze.rs` | No change (diagnostic logging stays, malloc_trim becomes unnecessary but harmless) |

---

## Constraints

- Parquet file schema must be identical (no downstream impact to Silver ETL, MCP server, Grafana)
- Must compile and run on ARM64 (aarch64-unknown-linux-gnu)
- No new C library dependencies (arrow-rs and parquet are pure Rust)
- Docker memory limit remains 512 MiB
