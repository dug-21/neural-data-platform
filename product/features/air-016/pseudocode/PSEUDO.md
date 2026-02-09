# AIR-016 Phase 1 Pseudocode: Allocator-Level Memory Management

> **Supersedes**: Previous sidecar-files approach (ADR-001)
> **Strategy**: Reduce peak memory by (1) switching to jemalloc, (2) dropping input
> data before building DataFrames, and (3) hinting the OS to reclaim freed pages.
> **Files touched**: 4 files, 0 trait changes, 0 read-path changes.

---

## Component 1: tikv-jemallocator Global Allocator

### Rationale

glibc malloc holds freed pages in thread-local caches indefinitely. jemalloc
returns them to the OS more aggressively and has lower fragmentation under the
repeated allocate-build-drop cycle that Parquet flushes produce.

### File: `apps/air-quality-app/Cargo.toml`

```toml
# ADD to [dependencies] section (after existing entries, line 55 area):
tikv-jemallocator = "0.6"
```

### File: `apps/air-quality-app/src/main.rs`

```rust
// ADD these 5 lines BEFORE the first `use` statement (before current line 1).
// They must appear at the top of the file, before any other code.

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

// --- existing code continues unchanged from here ---
// use air_quality_app::{...};
// use config_client::{...};
// ...
```

**Why `cfg(not(target_env = "msvc"))`**: jemalloc does not build on MSVC
(Windows). The Pi target is `aarch64-unknown-linux-gnu`, so the cfg is always
true in production but keeps the crate compilable on Windows dev machines.

**No other changes to main.rs.** The `#[tokio::main]` function and all
application logic remain untouched.

---

## Component 2: Explicit `drop(points)` in Write Functions

### Rationale

`write_parquet()` and `write_raw_parquet()` iterate over the input `Vec<T>` by
reference (`for p in &points`) to build column Vecs, then build a DataFrame
from those columns. The input Vec stays alive until the closure returns, meaning
the input data and the column Vecs coexist in memory simultaneously. An explicit
`drop(points)` after the iteration loop frees the input data before the
DataFrame is constructed, halving peak memory for the write path.

### File: `core/src/storage/parquet.rs`, function `write_parquet()`

Current code (lines 98-151, inside `spawn_blocking` closure):

```
 98:  tokio::task::spawn_blocking(move || {
...
113:      for p in &points {
114:          timestamps.push(p.timestamp.timestamp_micros());
115:          location_ids.push(p.location_id.clone());
...
124:          contexts.push(p.context.as_ref().map(|c| c.to_string()));
125:      }
126:                          // <--- INSERT drop(points) HERE
127:      let timestamp_series = Series::new("timestamp", timestamps);
```

**Change**: Insert one line after line 125 (after the `for p in &points` loop
closing brace), before line 127 (before `let timestamp_series`):

```rust
            drop(points); // AIR-016: Free input data before building DataFrame to halve peak memory
```

**No other changes to `write_parquet()`.** The `for p in &points` loop borrows
`points` immutably; once the loop ends, the borrow is released and `drop()` is
valid. The column Vecs (`timestamps`, `location_ids`, etc.) own their data
independently and are unaffected.

### File: `core/src/storage/parquet.rs`, function `write_raw_parquet()`

Current code (lines 510-555, inside `spawn_blocking` closure):

```
510:  tokio::task::spawn_blocking(move || {
...
524:      for p in &points {
525:          timestamps.push(p.timestamp.timestamp_micros());
526:          source_ids.push(p.source_id.clone());
527:          ndp_ids.push(p.ndp_id.clone());
528:          contexts.push(p.context.as_ref().map(|c| c.to_string()));
529:          raw_payloads.push(p.raw_payload.to_string());
530:      }
531:                          // <--- INSERT drop(points) HERE
532:      // Create Series for DataFrame
533:      let timestamp_series = Series::new("timestamp", timestamps);
```

**Change**: Insert one line after line 530 (after the `for p in &points` loop
closing brace), before line 532 (before `// Create Series for DataFrame`):

```rust
            drop(points); // AIR-016: Free input data before building DataFrame to halve peak memory
```

**No other changes to `write_raw_parquet()`.**

---

## Component 3: `malloc_trim` Fallback

### Rationale

Even with jemalloc as the global allocator, the Polars library may use its own
internal allocation strategies. `malloc_trim(0)` is a Linux-specific hint that
tells glibc to release any freed pages back to the OS. On jemalloc, this is a
no-op (jemalloc does not implement it), so it acts as a safety net: if any
allocation path bypasses jemalloc and uses glibc, this call reclaims those pages.

The call is placed at the end of `append_to_parquet()` and
`append_to_raw_parquet()` -- after the entire write-and-drop cycle completes --
to maximize the amount of reclaimable memory.

### File: `core/Cargo.toml`

```toml
# ADD to [dependencies] section (after existing entries, line 43 area):
libc = "0.2"
```

### File: `core/src/storage/parquet.rs`, function `append_to_parquet()`

Current code (lines 157-225):

```
157:  async fn append_to_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
158:      let mut all_points = points;
...
224:      self.write_parquet(all_points, path).await
225:  }
```

**Change**: Replace line 224 with three lines that call `write_parquet` then
`malloc_trim`:

```rust
        self.write_parquet(all_points, path).await?;

        // AIR-016: Hint to allocator to release freed pages back to OS
        #[cfg(target_os = "linux")]
        unsafe { libc::malloc_trim(0); }

        Ok(())
```

Note: The original line 224 was a tail expression (`self.write_parquet(...).await`).
The replacement adds `?` to propagate errors, adds the `malloc_trim` call, then
returns `Ok(())` explicitly.

### File: `core/src/storage/parquet.rs`, function `append_to_raw_parquet()`

Current code (lines 563-622):

```
563:  async fn append_to_raw_parquet(&self, points: Vec<RawDataPoint>, path: PathBuf) -> CoreResult<()> {
568:      let mut all_points = points;
...
621:      self.write_raw_parquet(all_points, &path).await
622:  }
```

**Change**: Replace line 621 with three lines that call `write_raw_parquet`
then `malloc_trim`:

```rust
        self.write_raw_parquet(all_points, &path).await?;

        // AIR-016: Hint to allocator to release freed pages back to OS
        #[cfg(target_os = "linux")]
        unsafe { libc::malloc_trim(0); }

        Ok(())
```

### No import needed

`libc::malloc_trim` is called via its full path. No `use` statement is required
in `parquet.rs`.

---

## Summary of All Changes

| File | Change | Lines Affected |
|------|--------|----------------|
| `apps/air-quality-app/Cargo.toml` | Add `tikv-jemallocator = "0.6"` to `[dependencies]` | +1 line |
| `apps/air-quality-app/src/main.rs` | Add jemalloc global allocator block before first `use` | +5 lines (top of file) |
| `core/Cargo.toml` | Add `libc = "0.2"` to `[dependencies]` | +1 line |
| `core/src/storage/parquet.rs` `write_parquet()` | Add `drop(points);` after line 125 | +1 line |
| `core/src/storage/parquet.rs` `write_raw_parquet()` | Add `drop(points);` after line 530 | +1 line |
| `core/src/storage/parquet.rs` `append_to_parquet()` | Add `malloc_trim` after `write_parquet` call at line 224 | +4 lines, -1 line |
| `core/src/storage/parquet.rs` `append_to_raw_parquet()` | Add `malloc_trim` after `write_raw_parquet` call at line 621 | +4 lines, -1 line |

**Total**: 4 files changed, ~15 lines added, 2 lines modified. Zero trait
changes. Zero read-path changes. Zero new runtime behavior beyond memory
management.

---

## Verification Plan

1. `cargo build` -- confirms `tikv-jemallocator` and `libc` resolve and the
   `#[global_allocator]` compiles on the dev machine.
2. `cargo clippy` -- no new warnings (the `unsafe` block for `malloc_trim`
   should not trigger clippy because it is a well-known FFI call).
3. `cargo test -p platform-core` -- existing Parquet read/write tests pass
   (the `drop(points)` does not change observable behavior).
4. `cargo test -p air-quality-app` -- existing app tests pass.
5. Deploy to Pi, observe RSS via `docker stats` over a full day. Expect peak
   memory to drop from ~460 MiB to ~350 MiB (jemalloc + drop) or lower.
