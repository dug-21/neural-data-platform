# ADR-001: Memory Instrumentation Strategy for BUG-005

## Status

Proposed

## Context

GitHub Issue #16 reports that air-quality-app RSS grows ~9 MiB/hr (decelerating) on Raspberry Pi 5 within a 512 MiB Docker container. BUG-004 (Polars DataFrame leak) was fixed in air-018 (v1.1.21) by replacing Polars with direct arrow-rs/parquet crate usage. After that fix, the FAST leak (~4.5 MiB per 30-min snapshot) is gone, but a SLOW growth persists.

**Production evidence (2026-02-10, 13.5 hour run):**

- RSS grew from 104.3 MiB to 229.4 MiB (+125.1 MiB)
- Accumulator only grew from 4.1 MiB to 8.0 MiB (+3.9 MiB)
- 121.2 MiB of RSS growth is unattributed to any measured subsystem
- Growth rate DECELERATES: ~16 MiB/hr early, ~5 MiB/hr by hour 12+
- Snapshot trim IS working: net deltas per cycle are often negative (-2.4, -6.5, -0.9 MiB)
- Snapshot peak spikes are 40-60 MiB above pre-snapshot RSS

The deceleration pattern and the snapshot spike/trim cycle strongly suggest glibc malloc arena fragmentation: repeated large allocations (RecordBatch construction during Parquet writes) fragment the heap into many small free blocks that glibc cannot return to the OS even after `malloc_trim(0)`. The RSS appears to grow but the memory is actually free-but-fragmented within the process address space.

However, we cannot PROVE this without allocator-level telemetry. The existing diagnostics only report RSS and accumulator size. We are blind to:

1. glibc arena state (fragmented free space vs. truly allocated space)
2. HashMap/Vec capacity overhead (Rust allocates more capacity than len)
3. Per-subsystem memory: HTTP sources (reqwest pool), MQTT (rumqttc buffers), Silver subscribers (TimescaleDB connection pool), tokio runtime
4. RSS composition: anonymous heap vs. file-backed (shared libs) vs. shared memory

**Subsystems running in the process (from `main.rs`):**

| Subsystem | Component | Potential Memory Holders |
|-----------|-----------|--------------------------|
| Bronze | BronzeSubscriber | Accumulator HashMap, WAL file handle, ParquetStore |
| Silver | 6x SilverSubscriber | Per-subscriber state, SilverOutput (TimescaleDB pool) |
| HTTP Sources | reqwest::Client | Connection pool, response buffers, TLS state |
| MQTT Source | rumqttc::AsyncClient | Receive buffer, in-flight messages, reconnect state |
| Config | ConfigClient, StreamRegistry | etcd connections, cached configs |
| Coordinator | IngestionCoordinator, SourceManager | EventBus broadcast channel, routing state |
| API | axum server | Connection state (minimal if few clients) |

## Decision

We adopt an **instrumentation-first** approach: measure before optimizing. Phase 1 adds memory attribution telemetry to production logs. Phase 2 applies targeted mitigation based on Phase 1 data. Phase 3 validates the fix with a 48-hour soak test.

### Phase 1: Instrument (this feature, ops-004)

Add three categories of memory telemetry to existing log infrastructure:

**1. glibc mallinfo2() via FFI**

```rust
// core/src/diagnostics/allocator.rs
#[cfg(target_os = "linux")]
extern "C" {
    fn mallinfo2() -> Mallinfo2;
}

#[repr(C)]
struct Mallinfo2 {
    arena: usize,     // Non-mmapped space allocated from system
    ordblks: usize,   // Number of free chunks
    smblks: usize,    // Number of fastbin blocks
    hblks: usize,     // Number of mmapped regions
    hblkhd: usize,    // Space in mmapped regions
    usmblks: usize,   // Unused (always 0)
    fsmblks: usize,   // Space in freed fastbin blocks
    uordblks: usize,  // Total allocated space
    fordblks: usize,  // Total free space (FRAGMENTATION METRIC)
    keepcost: usize,  // Top-most releasable space
}
```

The critical metric is `fordblks`: free space within arenas that glibc has NOT returned to the OS. If `fordblks` grows proportionally with RSS while `uordblks` stays flat, fragmentation is confirmed. If `uordblks` grows, there is a genuine leak (allocated memory not freed).

This FFI call is safe: mallinfo2 is a pure query, makes no allocations, and is present in glibc 2.33+ (Pi 5 ships glibc 2.36). The same FFI pattern is already used for `malloc_trim(0)` in `bronze.rs:248-251`.

**2. /proc/self/smaps_rollup parsing**

```rust
// core/src/diagnostics/smaps.rs
pub fn read_smaps_rollup() -> Option<SmapsRollup> {
    let content = std::fs::read_to_string("/proc/self/smaps_rollup").ok()?;
    // Parse Rss_Anon, Rss_File, Rss_Shmem lines (format: "Label: NNN kB")
    ...
}
```

This decomposes RSS into anonymous (heap+stack), file-backed (shared libraries, mmapped files), and shared memory. File-backed RSS is the baseline from loaded libraries and does not grow. Anonymous RSS is where the leak lives.

**3. Accumulator capacity introspection**

The existing `memory_estimate_bytes()` counts `Vec::len()` not `Vec::capacity()`. Rust's Vec doubles capacity on growth, so a Vec with 1000 elements may have capacity for 1024 or 2048. For the accumulator with ~2400 points across ~6 sources, this overhead could be several MiB of unused-but-allocated capacity.

New methods: `hashmap_capacity()`, `total_vec_capacity()`, `wasted_capacity_bytes()`.

**Where instrumentation is added (file locations):**

| Location | What | Why |
|----------|------|-----|
| `core/src/diagnostics/mod.rs` | New module; moved `read_process_rss_mib()` here | Centralize diagnostic code |
| `core/src/diagnostics/allocator.rs` | mallinfo2 FFI | Allocator-level telemetry |
| `core/src/diagnostics/smaps.rs` | smaps_rollup parser | RSS decomposition |
| `core/src/storage/accumulator.rs` | Capacity methods | Reveal Vec overhead |
| `core/src/subscribers/bronze.rs` L440-449 | Enhanced heartbeat | Add allocator stats to existing log |
| `core/src/subscribers/bronze.rs` L213-274 | Enhanced snapshot | Add before/after allocator stats |
| `core/src/subscribers/bronze.rs` (new) | Attribution log | Periodic memory category breakdown |

### Phase 2: Mitigate (separate feature, data-driven)

Based on Phase 1 data, apply ONE of these mitigations:

| If Phase 1 Shows | Mitigation | Approach |
|-------------------|------------|----------|
| `fordblks` >> `uordblks` growth (fragmentation) | jemalloc allocator | Add `tikv-jemallocator` crate; jemalloc has better arena management and returns pages more aggressively |
| `accum_wasted_capacity` growing unbounded | Vec shrink_to_fit | Call `shrink_to_fit()` on accumulator Vecs after each snapshot |
| `unattributed_mib` growing steadily | Subsystem buffer leak | Instrument reqwest pool size, rumqttc buffer, then fix the leaking subsystem |
| `uordblks` growing steadily (true leak) | Targeted fix | Bisect code paths to find the allocation site |

### Phase 3: Validate (separate feature)

After Phase 2 mitigation is deployed:

- 48-hour soak test on Pi 5 with production workload
- Success criteria: RSS stays below 256 MiB for entire 48h period
- Monitoring: attribution logs confirm stability of all categories

### Phased Approach Rationale

Jumping directly to an allocator swap (jemalloc) without data is risky because:

1. The air-018 experience showed mimalloc CRASHED on this platform -- alternative allocators are not guaranteed safe on aarch64 Pi 5
2. If the root cause is a genuine leak (not fragmentation), jemalloc would not help
3. If the root cause is Vec capacity overhead, shrink_to_fit is simpler and safer than an allocator change
4. Instrumentation has zero risk -- it adds logging, nothing else changes

## Consequences

### What becomes easier

- **Root cause identification**: Production logs will show exactly which memory category grows, eliminating guesswork
- **Future memory debugging**: The diagnostics module is reusable for any future memory issue
- **Mitigation design**: Phase 2 will be a targeted, data-driven fix rather than a shotgun approach
- **Regression detection**: Attribution logs can be monitored long-term to catch new memory issues early

### What becomes harder

- **Log volume**: Each heartbeat adds ~200 bytes of new structured fields. At 30-second intervals, this is ~576 KB/day. Negligible.
- **Code surface**: Three new files in `core/src/diagnostics/`. Maintenance cost is low since the code is pure reads with no side effects.
- **Platform coupling**: mallinfo2 and /proc/self/smaps_rollup are Linux-specific. Non-Linux platforms get `None` values. This is acceptable since the production target is exclusively Linux on Pi 5.

## Alternatives Considered

### Alternative 1: Blind jemalloc Swap

**Approach:** Replace glibc malloc with jemalloc (via `tikv-jemallocator`) without instrumentation.

**Rejected because:**
- mimalloc crashed on this platform (air-018 documented). jemalloc may also crash on aarch64 Pi 5 -- we have no data.
- If the root cause is NOT fragmentation (e.g., genuine leak), jemalloc would not help and we would have wasted a release cycle.
- Adding a system allocator affects ALL allocations process-wide. This is a high-blast-radius change that should be preceded by confirming fragmentation is the actual problem.

### Alternative 2: Arbitrary shrink_to_fit Everywhere

**Approach:** Call `shrink_to_fit()` on all accumulator Vecs after each snapshot.

**Rejected because:**
- Without data, we do not know if Vec capacity is the problem. If fragmentation is the cause, shrink_to_fit would actually WORSEN it by freeing and immediately reallocating memory (causing more fragmentation).
- shrink_to_fit after snapshot is pointless if the accumulator is growing (new data arrives immediately after snapshot; the Vec will just grow back).
- May need to be combined with HashMap::shrink_to_fit, which could cause rehashing overhead.

### Alternative 3: Prometheus/OpenTelemetry Metrics

**Approach:** Export memory metrics via Prometheus endpoint for Grafana dashboards.

**Rejected for Phase 1 because:**
- Adds `prometheus` or `opentelemetry` crate dependencies (NFR-02 violation)
- Requires Grafana infrastructure (not yet deployed, planned for db-* features)
- Structured logging achieves the same observability for diagnosis
- Can be added in a future ops feature once the leak is fixed and Grafana is available

### Alternative 4: Heap Profiler (valgrind/heaptrack)

**Approach:** Run the application under valgrind or heaptrack on the Pi to identify allocation sites.

**Rejected because:**
- Valgrind is not available for aarch64 Docker containers on Pi 5 without significant setup
- Heaptrack requires LD_PRELOAD which conflicts with the Docker deployment
- Both add 10-100x overhead, making the 30-minute snapshot cycle impractical to observe
- Production data is already available; we just need to ATTRIBUTE it, not profile it

### Alternative 5: Custom Global Allocator Wrapper

**Approach:** Implement a custom Rust GlobalAlloc that tracks allocations per call site.

**Rejected because:**
- Extremely invasive (wraps every allocation in the process)
- Significant performance overhead (hash lookup or counter update per alloc/dealloc)
- mallinfo2 provides aggregate stats sufficient for the current diagnosis at zero overhead
- Can be reconsidered in Phase 2 if mallinfo2 data is insufficient
