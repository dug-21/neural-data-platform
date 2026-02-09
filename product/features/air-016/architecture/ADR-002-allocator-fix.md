# ADR-002: Allocator-Level Memory Fix (Phase 1)

## Status: Accepted

## Context

The air-quality-app's RSS grew from 96 MiB to 490 MiB over several days. Initial analysis (ADR-001) proposed per-flush sidecar files to eliminate the read-modify-write pattern.

Re-analysis revealed the root cause is not the architecture but the **memory allocator**: glibc's malloc never returns freed heap pages to the OS. The read-modify-write pattern creates bursts of String/Vec allocations that fragment the heap. All allocations ARE freed (no Rust-level leak), but glibc's heap high-water mark only recedes from the top — interleaved small/large freed blocks prevent this.

## Decision

Fix the memory retention at the allocator level:

1. **tikv-jemallocator** as global allocator — jemalloc uses per-thread arenas with `mmap`/`munmap` and aggressively returns pages via `madvise(MADV_DONTNEED)`
2. **Explicit `drop(points)`** in write functions — halves peak memory by freeing input data before building the output DataFrame
3. **`malloc_trim(0)`** after each flush — belt-and-suspenders for any allocation path that bypasses jemalloc

## Why This Over Sidecar Files (ADR-001)

| | ADR-001 (Sidecar Files) | ADR-002 (Allocator Fix) |
|---|---|---|
| Lines changed | ~60-80 | ~15 |
| File format change | Yes (multiple files/day) | No |
| Read path change | Yes (glob multiple files) | No |
| New dependencies | None | tikv-jemallocator, libc |
| Silver ETL impact | Verify glob compat | None |
| MCP server impact | Change hard-coded filename | None |
| Risk | Medium (read path regression) | Low (allocator swap) |

ADR-001 is preserved as Phase 2 fallback if this fix proves insufficient.

## Consequences

### Positive
- Minimal code change (~15 lines)
- Zero architectural changes — file format, read path, write path logic all unchanged
- jemalloc is battle-tested (used by Firefox, Redis, ripgrep, InfluxDB IOx)
- Polars project explicitly recommends jemalloc for long-running processes

### Negative
- Binary size increases ~200KB (jemalloc linked statically)
- Adds two new Cargo dependencies
- `malloc_trim` requires `unsafe` block (well-known FFI call, minimal risk)

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| jemalloc build fails on aarch64 | Low | Medium | tikv-jemallocator supports aarch64-linux-gnu; fall back to drop+malloc_trim only |
| Binary size too large for Pi | Very Low | Low | 200KB is negligible on a 64GB SD card |
| jemalloc performance regression | Very Low | Low | jemalloc is typically faster than glibc malloc for this workload |
| Phase 1 insufficient (RSS still grows) | Medium | Medium | Trigger Phase 2 (sidecar files) per completion criteria |
