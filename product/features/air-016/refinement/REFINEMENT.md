# AIR-016 Phase 1 Refinement: Allocator Fix Validation Plan

> **Phase**: R (Refinement) -- Iterative improvement and validation
> **Scope**: Phase 1 only (jemalloc + explicit drop + malloc_trim)
> **Implementation size**: ~15 lines across 4 files
> **Risk level**: Low -- no logic changes, no trait changes, no file format changes

---

## 1. Cross-Compilation Verification

### Build Environment

The air-quality-app builds natively on the Raspberry Pi 5 (aarch64). There is
no cross-compilation step. The root `Dockerfile` uses `rust:1-bookworm` as the
builder stage and runs `cargo build --release -p air-quality-app` directly.
On the Pi, Docker runs natively on aarch64, so the Rust toolchain targets
`aarch64-unknown-linux-gnu` by default.

### jemalloc Build Requirements

`tikv-jemallocator` compiles jemalloc from C source as part of its build script.
This requires a C compiler and `make`. The `rust:1-bookworm` image includes
`gcc` and `make` by default (they are part of the Rust toolchain's build
dependencies). No additional `apt-get install` is needed in the Dockerfile.

The existing builder stage installs `pkg-config libssl-dev protobuf-compiler`.
None of these conflict with jemalloc's build. The `cc` crate (used by
tikv-jemallocator's build script) will find `gcc` automatically.

### Verification Steps

1. Build locally on dev machine (x86_64): `cargo build -p air-quality-app`
2. Build in Docker on Pi (aarch64): `docker build -t ndp/air-quality-app .`
3. Confirm binary starts: `docker run --rm ndp/air-quality-app --help`

### Fallback if jemalloc Fails on aarch64

If `tikv-jemallocator` fails to compile on aarch64 (unlikely -- it is tested
on aarch64-linux by the tikv project), the fallback is:

- Remove the 1 line from `apps/air-quality-app/Cargo.toml`
- Remove the 5 lines from `apps/air-quality-app/src/main.rs`
- Keep components 2 (explicit drop) and 3 (malloc_trim) -- these are
  independent of the allocator and still provide meaningful benefit

Components 2+3 alone reduce peak memory by ~50% per flush and force glibc to
return freed pages. This is a viable standalone improvement even without
jemalloc.

---

## 2. Feature Gating Consideration

### Should jemalloc be behind a cargo feature flag?

**Recommendation: NO.**

| Factor | Assessment |
|--------|------------|
| Implementation complexity | 3 lines in main.rs, 1 line in Cargo.toml |
| Dev build impact | jemalloc compiles on x86_64 Linux and macOS without issues |
| Windows compatibility | The `#[cfg(not(target_env = "msvc"))]` guard already handles MSVC |
| Maintenance cost of feature flag | Adds conditional compilation paths, doubles the test matrix |
| Benefit of feature flag | None -- jemalloc is always preferred for this workload |

A feature flag would be warranted if jemalloc caused test failures or
significantly slowed compile times on developer machines. Neither applies here.
The `tikv-jemallocator` crate adds ~10 seconds to a clean build and has zero
effect on incremental builds (the allocator is set once in main.rs).

The `#[cfg(not(target_env = "msvc"))]` annotation on the global allocator
declaration already provides the only platform gate that matters. On MSVC
(Windows), the default system allocator is used. On all other targets (Linux,
macOS), jemalloc is active. This is the same pattern used by the tikv project
itself.

---

## 3. malloc_trim Placement

### Where the SPEC Places It

The SPEC calls for `malloc_trim(0)` at the end of `append_to_parquet()` and
`append_to_raw_parquet()`, after the `self.write_parquet(...).await` and
`self.write_raw_parquet(...).await` calls return.

### Why This Placement Is Correct

The `write_parquet` and `write_raw_parquet` functions run the heavy allocation
work inside `tokio::task::spawn_blocking`. When `spawn_blocking` returns, the
blocking thread has already freed its allocations (including the DataFrame and
column Vecs). The `drop(points)` call inside the closure frees the input data
on the blocking thread as well.

However, the `append_to_parquet` function itself runs on a tokio async thread.
The read-modify-write cycle at the top of this function (reading the existing
file, deserializing rows, building the combined Vec) happens on the async
thread's heap. These allocations are freed before `write_parquet` is called,
but glibc's free-list holds the pages.

Calling `malloc_trim(0)` after the `.await` returns means:

- All allocations from both the async thread (read-modify-write) and the
  blocking thread (column build + DataFrame) have been freed
- `malloc_trim` runs on the async thread, trimming that thread's heap
- The blocking thread's heap is trimmed by jemalloc automatically (or on its
  next reuse by the blocking pool)

### Alternative: Inside spawn_blocking

Placing `malloc_trim` inside the `spawn_blocking` closure would only trim the
blocking thread's heap, missing the async thread's read-modify-write
allocations. This is strictly worse.

### Alternative: Separate spawn_blocking for malloc_trim

Since `malloc_trim` is a syscall that may briefly block, one could argue it
should be in its own `spawn_blocking` call. However:

- `malloc_trim(0)` typically completes in <1ms
- The 30-second flush interval is orders of magnitude larger
- Adding a second `spawn_blocking` introduces unnecessary scheduling overhead
- The async thread is already blocked waiting for the write to finish; a <1ms
  syscall is negligible

**Conclusion**: The SPEC placement (after the `.await` returns, on the async
thread) is correct and sufficient.

---

## 4. Safety of drop(points)

### write_parquet Analysis

```rust
// Inside spawn_blocking closure:
for p in &points {                    // immutable borrow of `points`
    timestamps.push(p.timestamp...);  // clones scalar/string data into column Vecs
    location_ids.push(p.location_id.clone());
    // ... more column Vecs ...
}
// Borrow of `points` ends here (loop scope closes)

drop(points);  // <-- INSERTED HERE: points is no longer borrowed

let timestamp_series = Series::new("timestamp", timestamps);
// ... column Vecs are independent, own their data ...
```

**Safety**: The `for p in &points` loop borrows `points` immutably. The borrow
is scoped to the loop body. After the loop's closing brace, `points` is no
longer borrowed. Each column Vec (timestamps, location_ids, etc.) owns its
data independently -- values were `.clone()`'d or copied (scalars like i64,
f64 are Copy). The `drop(points)` call consumes the owned Vec, freeing the
`TimeSeriesPoint` structs and their String fields. No column Vec references
anything inside `points`.

The Rust borrow checker enforces this at compile time. If `drop(points)` were
placed inside the loop or while a reference to `points` existed, it would be a
compile error, not a runtime bug.

### write_raw_parquet Analysis

Identical structure. The `for p in &points` loop builds column Vecs from
`RawDataPoint` fields using `.clone()` and `.to_string()`. After the loop,
`points` is no longer borrowed. `drop(points)` is safe for the same reasons.

### What drop(points) Achieves

Without `drop(points)`, the input Vec lives until the closure returns. This
means during DataFrame construction and Parquet serialization, both the input
data and the output column data exist simultaneously. For a file with N rows,
this is approximately 2x the string memory.

With `drop(points)`, the input data is freed before DataFrame construction
begins. Peak memory for the write path drops from ~2x to ~1x the column data
size.

---

## 5. Testing Refinement

### Existing Test Coverage

Phase 1 makes zero logic changes. The same data goes in, the same data comes
out. The allocator, explicit drop, and malloc_trim are invisible to the
application's observable behavior.

| Test Suite | Impact | Action |
|------------|--------|--------|
| `cargo test -p platform-core` (Parquet tests) | None -- same read/write behavior | Run, confirm pass |
| `cargo test -p air-quality-app` | None -- no app logic changes | Run, confirm pass |
| `cargo test -p ndp-gold-ddl` (339 tests) | None -- unrelated crate | Run, confirm pass |
| `cargo test -p ndp-validate` (217 tests) | None -- unrelated crate | Run, confirm pass |

### No New Tests Needed

There is no new behavior to test. Adding a test that "verifies memory usage"
would be flaky and platform-dependent (RSS measurements vary by OS, allocator
state, and system load). Memory improvement is validated by observation on the
Pi, not by unit tests.

### Build Verification (New)

The one verification that existing tests do NOT cover is whether the project
compiles with the new dependencies. This is covered by `cargo build` and
`cargo clippy` in the verification plan. The `unsafe` block for `malloc_trim`
will trigger a clippy warning if not annotated. The `#[cfg(target_os = "linux")]`
guard ensures it only compiles on Linux, avoiding issues on macOS or Windows
dev machines.

---

## 6. Measurement Plan

### Baseline (Before Deployment)

Record RSS from the current production container over a 24-hour period.

```bash
# On the Pi, run every 5 minutes via cron or a simple loop:
docker stats --no-stream --format \
  "{{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}" \
  air-quality-app >> /data/memory-baseline.tsv
```

Capture for a full 24 hours. Expected pattern: RSS climbs from ~96 MiB after
restart to ~350-490 MiB by end of day, never decreasing.

### Post-Deployment (After Phase 1)

Deploy the Phase 1 build and record RSS using the same method for 24 hours.

```bash
docker stats --no-stream --format \
  "{{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}" \
  air-quality-app >> /data/memory-phase1.tsv
```

### Success Criteria

| Metric | Threshold | Rationale |
|--------|-----------|-----------|
| RSS after 1 hour | < 150 MiB | Proves jemalloc returns pages promptly |
| RSS after 12 hours | < 200 MiB | Proves no monotonic growth trend |
| RSS after 24 hours | < 200 MiB | Proves sustained stability |
| RSS growth rate | < 1 MiB/hour | Vs current ~16 MiB/hour |
| Data correctness | Identical | Silver ETL reads produce same row counts |

The 200 MiB threshold is conservative. The SPEC estimates 100-120 MiB stable.
If RSS stabilizes anywhere under 200 MiB, Phase 1 is a success and Phase 2
(sidecar files) is not needed.

### If Phase 1 Falls Short

If RSS exceeds 200 MiB after 24 hours but is lower than the 490 MiB baseline,
Phase 1 is a partial success. Document the observed RSS curve and proceed to
Phase 2 (sidecar files) to eliminate the read-modify-write pattern entirely.

If RSS matches the baseline (no improvement), investigate whether jemalloc is
actually active by checking:

```bash
# Inside the container:
cat /proc/$(pidof air-quality-server)/maps | grep jemalloc
```

If jemalloc mappings are absent, the `#[global_allocator]` may not have taken
effect (e.g., statically linked glibc overriding it). This would be
investigated as a build configuration issue.

---

## 7. Rollback Plan

### Full Rollback (All 3 Components)

Revert the 4 changed files to their pre-Phase 1 state. This is a single
`git revert` of the Phase 1 commit.

| Component | Files to Revert | Risk of Revert |
|-----------|----------------|----------------|
| jemalloc allocator | `apps/air-quality-app/Cargo.toml`, `apps/air-quality-app/src/main.rs` | None -- returns to glibc default |
| drop(points) | `core/src/storage/parquet.rs` (2 locations) | None -- points are dropped at end of closure anyway |
| malloc_trim | `core/src/storage/parquet.rs` (2 locations), `core/Cargo.toml` | None -- removes a no-op hint |

### Partial Rollback (jemalloc Only)

If jemalloc causes unexpected behavior (unlikely) but the other components are
fine:

- Remove `tikv-jemallocator = "0.6"` from `apps/air-quality-app/Cargo.toml`
- Remove the 5-line allocator block from `apps/air-quality-app/src/main.rs`
- Keep `drop(points)` and `malloc_trim` -- these are independent and beneficial
  under glibc as well

### Components That Are Always Safe to Keep

| Component | Why It's Safe |
|-----------|--------------|
| `drop(points)` | Explicitly does what Rust would do implicitly at end of scope. Zero behavioral change. Reduces peak memory under any allocator. |
| `malloc_trim(0)` | No-op under jemalloc. Under glibc, it returns free pages to OS. Worst case: a <1ms syscall every 30 seconds. |

---

## Refinement Checklist

Before moving to Completion phase:

- [ ] `cargo build -p air-quality-app` succeeds on dev machine
- [ ] `cargo clippy -p air-quality-app -p platform-core` -- zero warnings
- [ ] `cargo test -p platform-core` -- all Parquet tests pass
- [ ] `cargo test -p air-quality-app` -- all app tests pass
- [ ] Docker build succeeds on Pi (aarch64)
- [ ] Container starts and passes health check
- [ ] 24-hour baseline RSS captured from current production
- [ ] Phase 1 deployed to Pi
- [ ] 24-hour post-deployment RSS captured
- [ ] RSS under 200 MiB sustained (success) OR Phase 2 triggered (partial)
