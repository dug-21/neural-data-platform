# BUG-004: Bronze Accumulator Memory Leak

> **Bug ID:** BUG-004
> **Feature:** AIR-017 Phase 1 (Bronze Write-Ahead Architecture)
> **Severity:** Critical (approaching OOM on production Pi)
> **Created:** 2026-02-09
> **Status:** Specification Complete

---

## 1. Specification

### Problem Statement

AIR-017 Phase 1 introduced a Bronze Write-Ahead Architecture with an in-memory `Accumulator` (`core/src/storage/accumulator.rs`) that holds **all of today's data** in a `HashMap<String, Vec<RawDataPoint>>`. This accumulator never clears within a calendar day. Each 30-minute snapshot clones all accumulated data to build Polars column vectors, and glibc's malloc does not return freed pages to the OS. The result is unbounded memory growth from ~196 MiB at startup to ~467 MiB overnight against a 512 MiB container limit.

The previous version (v1.1.17, no accumulator) ran stable at ~75 MiB.

### Root Cause Analysis

The memory growth has three contributing factors that compound throughout the day:

**Factor 1: Accumulator never clears.**
`BronzeSubscriber::handle_point()` (line 167, `core/src/subscribers/bronze.rs`) calls `self.accumulator.add(owned_point)` on every received data point. The accumulator is never cleared or drained within a day. By midnight, it holds every data point received since startup.

**Factor 2: Snapshot clones all data.**
`BronzeSubscriber::snapshot()` (line 203, `core/src/subscribers/bronze.rs`) calls `self.accumulator.all_points_by_source()` which returns a `&HashMap<String, Vec<RawDataPoint>>` (line 59, `core/src/storage/accumulator.rs`). The snapshot loop at line 217 then calls `points.clone()` on the full `Vec<RawDataPoint>` for every source_id. This means every snapshot allocates a complete copy of all accumulated data.

**Factor 3: Polars column construction creates string copies.**
`ParquetStore::write_raw_parquet()` (line 512, `core/src/storage/parquet.rs`) iterates over all points and calls `.clone()` on `source_id` (line 536), `.to_string()` on `raw_payload` (line 539), and `.to_string()` on `context` (line 538). These create fresh heap allocations proportional to the total data volume. When the Polars DataFrame and column vectors are dropped after the write, glibc's malloc retains the pages in its free list rather than returning them via `munmap`.

**Peak memory during a single snapshot:**
- Accumulator: all day's data (grows linearly)
- Cloned `Vec<RawDataPoint>` per source: same size as accumulator
- Polars column vectors (timestamps, source_ids, payloads): same data as strings
- Total: approximately 3x the accumulator's logical size at peak

**The core insight:** The WAL already contains every data point on disk. The accumulator is a redundant in-memory copy that exists only to avoid reading the WAL at snapshot time. Eliminating the accumulator and reading the WAL from disk during snapshot trades a trivial sequential disk read (~12 MB/day) for hundreds of megabytes of retained heap memory.

### Proposed Fix

Remove the in-memory accumulator entirely. Use the WAL as the single source of truth for unsnapshot data.

**`handle_point()`:** WAL append only. No accumulator add.

**`snapshot()`:** Replay the full WAL from disk (sequence > 0), group entries by `source_id`, write one Parquet file per source (full overwrite as before). Do NOT truncate the WAL within a day -- the WAL stays as-is until day rollover since the next snapshot needs the same data plus any new entries.

**Recovery on startup:** No-op. The WAL is already on disk. The next snapshot timer will read the WAL and write Parquet. No need to seed an in-memory accumulator.

**Day rollover:** Truncate the WAL (or delete and recreate). The previous day's Parquet files are finalized.

### Operational Impact

**Zero impact to downstream consumers:**

| Concern | Before (accumulator) | After (WAL-only) |
|---------|---------------------|-------------------|
| Parquet file format | Unchanged | Unchanged |
| Parquet file paths | `{data_dir}/raw/{stream}/year=YYYY/month=MM/day=DD/data.parquet` | Identical |
| Parquet file content | All day's data for source_id | Identical |
| Silver ETL | Reads from Parquet | No change |
| Process RSS | ~196 MiB start, ~467 MiB overnight (growing) | ~75 MiB flat all day |
| WAL disk usage | Truncated every 30 min (~0.5 MB peak) | Grows to ~12 MB/day, cleared at rollover |
| Snapshot I/O | Clone from memory + Polars write | Sequential WAL read (~12 MB) + Polars write |
| Snapshot latency | Dominated by clone + Polars (~ms) | Sequential read adds microseconds for 12 MB |

### Acceptance Criteria

1. Process RSS remains below 100 MiB after 24 hours of continuous operation with 4 streams at current ingest rates.
2. Parquet files produced by the new snapshot path are byte-for-byte schema-compatible with the previous version (same columns, same types, same compression).
3. All existing unit and integration tests in `core/src/subscribers/bronze.rs` pass (with modifications to remove accumulator references).
4. WAL file size on disk does not exceed 20 MB at end of day (4 streams, ~11K points/stream, ~500 bytes/point serialized).
5. Startup after clean shutdown produces correct Parquet on first snapshot (WAL replayed from disk).
6. Startup after crash (WAL has entries, Parquet may be stale) produces correct Parquet on first snapshot (WAL replayed, Parquet overwritten).
7. Day rollover clears the WAL and the next day starts with an empty WAL.
8. Snapshot logging includes: WAL file size in bytes, number of entries replayed, number of Parquet files written, elapsed wall-clock time.

---

## 2. Pseudocode

### 2.1 BronzeSubscriber struct (remove accumulator field)

```
struct BronzeSubscriber {
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    wal: WriteAheadLog,
    // REMOVED: accumulator: Accumulator,
    data_dir: PathBuf,
    cancellation_token: CancellationToken,
    is_running: bool,
    // Metrics
    events_received: u64,
    events_written: u64,
    snapshots_written: u64,
    errors_total: u64,
    wal_errors: u64,
}
```

### 2.2 handle_point() -- WAL append only

```
fn handle_point(&mut self, point: Arc<RawDataPoint>) {
    self.events_received += 1

    if !self.accepts_stream(&point.source_id) {
        debug!("Skipping point: stream not in filter")
        return
    }

    let owned_point = (*point).clone()

    match self.wal.append_point(&owned_point) {
        Ok(_seq) => {
            // WAL is the single source of truth.
            // No accumulator add -- data is durable on disk.
        }
        Err(e) => {
            self.wal_errors += 1
            error!("WAL append failed -- point NOT durable: {}", e)
        }
    }
}
```

### 2.3 snapshot() -- replay WAL from disk

```
async fn snapshot(&mut self) -> Result<(), SubscriberError> {
    let snapshot_start = std::time::Instant::now()

    // Read WAL file size before replay for logging
    let wal_file_size = self.wal.file_size_bytes()

    // Replay all entries from WAL (sequence > 0 = everything)
    let entries = match self.wal.replay_since(0) {
        Ok(entries) => entries,
        Err(e) => {
            error!("Snapshot: WAL replay failed: {}", e)
            return Err(SubscriberError::StorageError(...))
        }
    }

    if entries.is_empty() {
        return Ok(())
    }

    let entry_count = entries.len()

    // Group by source_id
    let mut points_by_source: HashMap<String, Vec<RawDataPoint>> = HashMap::new()
    for entry in entries {
        points_by_source
            .entry(entry.source_id.clone())
            .or_default()
            .push(entry.point)
    }

    let source_count = points_by_source.len()
    let total_points: usize = points_by_source.values().map(|v| v.len()).sum()

    // Determine snapshot_time from latest point timestamp
    let snapshot_time = points_by_source.values()
        .flat_map(|pts| pts.iter())
        .map(|p| p.timestamp)
        .max()
        .unwrap_or_else(Utc::now)

    // Write one Parquet file per source_id (full overwrite)
    for (source_id, points) in points_by_source {
        let partition_path = self.partition_path(&source_id, snapshot_time)

        self.store
            .write_raw_snapshot(points, &partition_path)   // points moved, not cloned
            .await
            .map_err(|e| SubscriberError::StorageError(...))?
    }

    // DO NOT truncate WAL here.
    // The WAL stays intact because the next snapshot needs the same data
    // plus any new entries. WAL is only cleared at day rollover.

    self.events_written = total_points as u64
    self.snapshots_written += 1

    let elapsed = snapshot_start.elapsed()
    info!(
        subscriber_id = %self.id,
        sources = source_count,
        total_points = total_points,
        wal_entries_replayed = entry_count,
        wal_file_bytes = wal_file_size,
        elapsed_ms = elapsed.as_millis(),
        "Snapshot complete"
    )

    Ok(())
}
```

**Key change:** `points` is moved into `write_raw_snapshot()` rather than cloned. The `HashMap` owns the data, each iteration moves the `Vec<RawDataPoint>` out of the map. No `.clone()` needed. This eliminates Factor 2 from the root cause analysis. Factor 3 (Polars string allocation) still occurs but is transient -- the column vectors are dropped after the Parquet write, and since no accumulator is holding a persistent copy, RSS returns to baseline.

### 2.4 start() select loop -- remove accumulator references

```
async fn start(&mut self, mut receiver: broadcast::Receiver<Arc<RawDataPoint>>)
    -> Result<(), SubscriberError>
{
    info!("Starting BronzeSubscriber")
    self.is_running = true

    // NO recovery step needed.
    // WAL is already on disk. Next snapshot reads it.
    // Log WAL state for observability.
    let wal_size = self.wal.file_size_bytes()
    let wal_entries = self.wal.replay_since(0).map(|e| e.len()).unwrap_or(0)
    info!(
        subscriber_id = %self.id,
        wal_file_bytes = wal_size,
        wal_entry_count = wal_entries,
        "Startup: WAL state"
    )

    let snapshot_interval = Duration::from_secs(self.config.snapshot_interval_secs)
    let mut snapshot_timer = tokio::time::interval(snapshot_interval)
    snapshot_timer.tick().await   // skip immediate first tick

    let flush_interval = Duration::from_secs(self.config.flush_interval_secs)
    let mut flush_timer = tokio::time::interval(flush_interval)
    flush_timer.tick().await   // skip immediate first tick

    loop {
        tokio::select! {
            biased;

            _ = self.cancellation_token.cancelled() => {
                info!("Received cancellation signal")
                break
            }

            _ = snapshot_timer.tick() => {
                if let Err(e) = self.snapshot().await {
                    error!("Snapshot failed on timer: {}", e)
                }
            }

            _ = flush_timer.tick() => {
                let wal_size = self.wal.file_size_bytes()
                debug!(
                    subscriber_id = %self.id,
                    wal_file_bytes = wal_size,
                    wal_errors = self.wal_errors,
                    "Heartbeat"
                )
            }

            result = receiver.recv() => {
                match result {
                    Ok(point) => self.handle_point(point),
                    Err(RecvError::Lagged(n)) => {
                        warn!("Subscriber lagged - {} events may be lost", n)
                    }
                    Err(RecvError::Closed) => {
                        info!("Event bus channel closed")
                        break
                    }
                }
            }
        }
    }

    // Final snapshot on exit
    info!("Performing final snapshot before shutdown")
    if let Err(e) = self.snapshot().await {
        error!("Final snapshot failed: {}", e)
    }

    self.is_running = false
    info!(
        subscriber_id = %self.id,
        events_received = self.events_received,
        events_written = self.events_written,
        snapshots_written = self.snapshots_written,
        errors_total = self.errors_total,
        wal_errors = self.wal_errors,
        "BronzeSubscriber stopped"
    )

    Ok(())
}
```

### 2.5 Day rollover logic

Day rollover is a separate concern from this bug fix but is mentioned here for completeness since WAL truncation moves from snapshot-time to rollover-time.

```
// In the select! loop, add a day rollover branch:

_ = day_rollover_timer => {
    info!("Day rollover: truncating WAL")

    // Final snapshot for the ending day
    if let Err(e) = self.snapshot().await {
        error!("Day rollover snapshot failed: {}", e)
    }

    // Truncate WAL: all entries are now in Parquet
    match self.wal.truncate() {
        Ok(()) => info!("WAL truncated for new day"),
        Err(e) => error!("WAL truncate failed: {}", e),
    }
}
```

The `truncate()` method can be implemented as the existing `commit()` (line 286, `core/src/storage/wal.rs`) which deletes and recreates the file. Alternatively, rename `commit()` to `truncate()` for clarity.

### 2.6 WAL helper methods for logging

```
impl WriteAheadLog {
    /// Return the WAL file size in bytes, or 0 if file does not exist.
    pub fn file_size_bytes(&self) -> u64 {
        std::fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Return the count of entries currently in the WAL (requires file scan).
    /// This is O(file_size) -- use sparingly (e.g., startup logging only).
    pub fn entry_count(&self) -> CoreResult<usize> {
        self.replay_since(0).map(|entries| entries.len())
    }
}
```

---

## 3. Architecture

### ADR-017-BUG-004: Remove In-Memory Accumulator, WAL-Only Bronze Snapshot

#### Status

Proposed

#### Context

AIR-017 Phase 1 introduced a `HashMap<String, Vec<RawDataPoint>>` accumulator in `BronzeSubscriber` to hold all of today's data in memory. The rationale was to avoid reading the WAL from disk at snapshot time. In practice:

1. The accumulator holds a complete copy of the day's data in memory, never clearing it.
2. Each 30-minute snapshot clones the entire accumulator contents to build Polars column vectors.
3. glibc's malloc does not return freed pages to the OS (it uses `brk`/`sbrk` for small allocations and retains `mmap`'d regions in free lists).
4. Memory grows from ~196 MiB at startup to ~467 MiB overnight, approaching the 512 MiB container limit.

The WAL already contains every data point on disk in a sequential, line-delimited JSON format. Reading ~12 MB of sequential data from disk takes microseconds on the Pi's SD card or SSD. The accumulator is a premature optimization that creates a critical resource problem.

#### Decision

Remove the `Accumulator` struct from `BronzeSubscriber`. Use the WAL file on disk as the single source of truth for snapshot data.

Specifically:

- **`handle_point()`**: WAL append only. No in-memory buffering.
- **`snapshot()`**: `wal.replay_since(0)` to read all entries, group by `source_id`, write Parquet per source. Move (not clone) data into `write_raw_snapshot()`.
- **Recovery**: Eliminated. WAL is on disk; next snapshot reads it.
- **WAL lifecycle**: WAL is NOT truncated at snapshot time (the next snapshot needs the same data plus new entries). WAL is truncated at day rollover only.
- **`accumulator.rs`**: Removed from compilation. File can be deleted or `#[cfg(test)]`-gated if any test utilities reference it.

#### Consequences

**What becomes easier:**
- Memory profile is flat at ~75 MiB regardless of data volume or time of day.
- No risk of OOM from accumulated data.
- Recovery logic is eliminated (no need to seed accumulator from Parquet + WAL).
- Snapshot code is simpler: read WAL, group, write. No clone step.
- `BronzeSubscriber::new()` no longer needs to construct an `Accumulator` with today's date.

**What becomes harder:**
- Snapshot must read the WAL from disk every time. For ~12 MB at current volumes this is negligible. If data volumes grow 10x, the WAL could reach ~120 MB, and sequential read time would still be under a second.
- WAL file grows throughout the day (~12 MB) instead of being truncated every 30 minutes (~0.5 MB peak). This is acceptable: 12 MB on a 32+ GB SD card is negligible.
- Health check can no longer report `accumulator_count`. Replace with `wal_entry_count` or `wal_file_bytes` (cheaper to compute from file metadata).
- Day rollover becomes the WAL truncation point instead of each snapshot. If day rollover fails, the WAL continues growing into the next day (but the next day's snapshot still produces correct Parquet because `replay_since(0)` returns all entries, and Parquet is partitioned by date in the file path).

#### Alternatives Considered

**A. Clear accumulator after snapshot (keep accumulator, just clear it).**
Rejected. Clearing the accumulator means the next snapshot would only write new-since-last-snapshot data. But the snapshot writes a full-day Parquet file (overwrite semantics). So either: (a) the accumulator must be rebuilt from Parquet on the next snapshot, reintroducing the read-modify-write we eliminated in AIR-017, or (b) the snapshot switches to append semantics, which changes the Parquet file format and breaks downstream consumers. Neither option is viable.

**B. Use `jemalloc` or `mimalloc` instead of glibc malloc.**
These allocators are better at returning pages to the OS. This would reduce the symptom but not the cause. The accumulator still holds a redundant copy of all data. A 22 MiB accumulator + 22 MiB clone + Polars strings is still ~66 MiB of unnecessary allocation per snapshot. With jemalloc, RSS might settle at ~200 MiB instead of ~467 MiB, but it would still be 3x the pre-AIR-017 baseline. Additionally, switching allocators introduces a new dependency and may have other performance implications.

**C. Drain accumulator into snapshot (move instead of clone).**
Change `all_points_by_source()` to return owned data (`HashMap<String, Vec<RawDataPoint>>`) and drain the accumulator. This eliminates the clone but requires rebuilding the accumulator from WAL before the next snapshot. This is essentially the WAL-only design but with an extra step (rebuild accumulator) that adds complexity for no benefit. If you are going to read the WAL anyway, there is no reason to maintain the accumulator.

**D. Snapshot only new data since last snapshot.**
Change snapshot from full-overwrite to incremental append. This requires a different Parquet strategy (multiple files per day, or Parquet row group append). It changes the downstream contract, adds compaction complexity, and violates the AIR-017 design principle of "one Parquet file per day per stream, full overwrite."

### Logging Additions

Add structured logging at these points:

| Event | Fields | Log Level |
|-------|--------|-----------|
| Startup WAL state | `wal_file_bytes`, `wal_entry_count` | `info` |
| Snapshot complete | `sources`, `total_points`, `wal_entries_replayed`, `wal_file_bytes`, `elapsed_ms` | `info` |
| Heartbeat (flush timer) | `wal_file_bytes`, `wal_errors` | `debug` |
| Day rollover | `wal_truncated` | `info` |
| WAL replay failure | `error` detail | `error` |

### WAL API Additions

Add to `WriteAheadLog` (`core/src/storage/wal.rs`):

- `file_size_bytes() -> u64`: Returns file size from `std::fs::metadata`. O(1) syscall. Used in heartbeat and snapshot logging.
- `entry_count() -> CoreResult<usize>`: Calls `replay_since(0).map(|e| e.len())`. O(file_size) -- use only at startup.
- Consider renaming `commit()` to `truncate()` for semantic clarity, preserving backward compatibility via a deprecated alias if `ParquetStore` still calls `commit()`.

---

## 4. Refinement

### Implementation Waves

**Wave 1: WAL helper methods** (low risk, no behavior change)
- Add `file_size_bytes()` and `entry_count()` to `WriteAheadLog` in `core/src/storage/wal.rs`.
- Add unit tests for both methods.
- This wave can be deployed independently as it adds API surface without changing behavior.

**Wave 2: Remove accumulator from BronzeSubscriber** (core fix)
- Remove `accumulator` field from `BronzeSubscriber` struct (line 111, `core/src/subscribers/bronze.rs`).
- Remove `use crate::storage::accumulator::Accumulator` import (line 29).
- Remove `use std::collections::HashMap` import (line 37) -- wait, `HashMap` is still needed for `health_check()` details and for grouping in `snapshot()`.
- Rewrite `handle_point()` (line 167): remove `self.accumulator.add(owned_point)` call.
- Rewrite `snapshot()` (line 203): replace `self.accumulator.all_points_by_source()` with `self.wal.replay_since(0)` + grouping logic. Remove `self.accumulator.count()` check, replace with entries check. Remove `self.accumulator.latest()`, compute from entries. Remove WAL `commit_to()` call. Add logging fields.
- Rewrite `recover()` (line 255): either remove entirely or replace with a startup log of WAL state. The `start()` method currently calls `self.recover().await?` at line 357 -- replace with WAL state logging.
- Update `health_check()` (line 454): replace `accumulator_count` detail with `wal_file_bytes`.
- Rewrite `new()` (line 135): remove `Accumulator::new(today)` construction.
- Update all unit tests that reference `subscriber.accumulator`.

**Wave 3: Remove accumulator module** (cleanup)
- Remove or gate `core/src/storage/accumulator.rs`.
- Remove `pub mod accumulator;` and `pub use accumulator::Accumulator;` from `core/src/storage/mod.rs` (lines 1, 5).
- Verify no other code references `Accumulator`. Grep the workspace.

**Wave 4: Update integration tests** (validation)
- Update integration tests in `core/src/subscribers/bronze.rs` (the `integration_tests` module starting at line 1415) to verify correct behavior without accumulator.
- Tests that assert `subscriber.accumulator.count()` must be rewritten to assert on WAL state or Parquet output.

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| WAL read at snapshot time is slower than memory access | Very Low | Negligible | 12 MB sequential read on any storage medium is under 100ms. Benchmark in integration test. |
| WAL grows unbounded if day rollover fails | Low | Medium | Day rollover failure is logged as error. WAL growth is bounded by ingest rate (~12 MB/day). Even without rollover, a week of WAL is ~84 MB. Add WAL size monitoring in heartbeat. |
| Tests that assert accumulator state break | Certain | Low | Expected. Rewrite tests to assert on WAL state and Parquet output instead. |
| Other code depends on `Accumulator` | Low | Low | Grep workspace for `Accumulator` references. Currently only `bronze.rs` and `storage/mod.rs` import it. |
| Snapshot sees partial WAL if `handle_point` races with `snapshot` | None | N/A | `BronzeSubscriber` is single-threaded (`&mut self`). The `select!` loop ensures `handle_point` and `snapshot` never run concurrently. |
| `write_raw_snapshot` signature takes `Vec<RawDataPoint>` by value | None | N/A | Data is moved from the grouped `HashMap`, not cloned. This is already the correct signature. |

### Rollback Plan

If the WAL-only approach causes unexpected issues in production:

1. Revert the commit that removes the accumulator.
2. The accumulator-based code is the current `main` branch.
3. No data migration needed -- WAL and Parquet formats are unchanged.
4. Alternative fast mitigation: switch to `jemalloc` as a stopgap while investigating (see Alternative B in ADR above). Add `[profile.release] opt-level = 3` and the `jemallocator` crate. This buys time but does not fix the root cause.

---

## 5. Completion

### Deployment Checklist

- [ ] All existing tests pass with accumulator removed (unit + integration).
- [ ] New tests cover: WAL-only snapshot, WAL helper methods, startup logging, health check without accumulator.
- [ ] `cargo clippy -- -D warnings` clean.
- [ ] `cargo build --release` succeeds.
- [ ] Manual verification on integration environment (`deploy/pi/deploy.sh`):
  - [ ] Start app, ingest data for 10 minutes across multiple streams.
  - [ ] Verify Parquet files written at snapshot interval.
  - [ ] Kill app (simulate crash), restart, verify first snapshot produces correct Parquet.
  - [ ] Monitor RSS via `docker stats` -- confirm flat memory profile.
- [ ] Update `product/features/air-017/STATUS.md` to reflect BUG-004 fix.

### Verification Steps

**1. Memory stability (primary acceptance criterion):**
```bash
# On Pi, after deploying the fix:
docker stats --no-stream air-quality-app
# Record RSS at startup, after 1 hour, after 6 hours, after 24 hours.
# Expected: RSS stays within 75-100 MiB range throughout.
```

**2. Parquet correctness:**
```bash
# After at least one snapshot interval, verify Parquet contents:
python3 -c "
import pyarrow.parquet as pq
t = pq.read_table('/data/raw/air-quality/year=2026/month=02/day=09/data.parquet')
print(f'Rows: {t.num_rows}, Columns: {t.column_names}')
print(t.schema)
"
# Expected: same schema as pre-fix (timestamp, source_id, ndp_id, context, raw_payload)
```

**3. WAL lifecycle:**
```bash
# Check WAL file size grows throughout the day:
ls -la /data/wal.log
# Expected: grows linearly, resets at day rollover.
```

**4. Log verification:**
```bash
# Check structured logs for new snapshot fields:
docker logs air-quality-app 2>&1 | grep "Snapshot complete"
# Expected: logs include wal_entries_replayed, wal_file_bytes, elapsed_ms
```

**5. Crash recovery:**
```bash
# Kill the container mid-day:
docker kill air-quality-app
# Restart:
docker start air-quality-app
# Check logs for startup WAL state:
docker logs air-quality-app 2>&1 | grep "Startup: WAL state"
# Wait for snapshot timer, verify Parquet has all data from before crash.
```

### Files Modified Summary

| File | Change |
|------|--------|
| `core/src/subscribers/bronze.rs` | Remove `accumulator` field, rewrite `handle_point()`, `snapshot()`, `recover()`/startup, `health_check()`, `new()`. Update all tests. |
| `core/src/storage/wal.rs` | Add `file_size_bytes()`, `entry_count()` methods. |
| `core/src/storage/accumulator.rs` | Remove file (or mark dead code). |
| `core/src/storage/mod.rs` | Remove `pub mod accumulator` and `pub use accumulator::Accumulator`. |

### Code References (current line numbers)

| Location | Line(s) | What |
|----------|---------|------|
| `core/src/subscribers/bronze.rs:29` | Import | `use crate::storage::accumulator::Accumulator` |
| `core/src/subscribers/bronze.rs:111` | Field | `accumulator: Accumulator` |
| `core/src/subscribers/bronze.rs:144` | Constructor | `Accumulator::new(today)` |
| `core/src/subscribers/bronze.rs:167-197` | Method | `handle_point()` -- WAL append + accumulator add |
| `core/src/subscribers/bronze.rs:203-243` | Method | `snapshot()` -- reads accumulator, clones data, writes Parquet |
| `core/src/subscribers/bronze.rs:255-325` | Method | `recover()` -- seeds accumulator from Parquet + WAL |
| `core/src/subscribers/bronze.rs:357` | Startup | `self.recover().await?` call |
| `core/src/subscribers/bronze.rs:391-395` | Heartbeat | Logs `accumulator_count` |
| `core/src/subscribers/bronze.rs:470-473` | Health | Reports `accumulator_count` in details |
| `core/src/storage/wal.rs:129-176` | Method | `replay_since()` -- reads WAL entries from disk |
| `core/src/storage/wal.rs:183-227` | Method | `commit_to()` -- truncates WAL (no longer called at snapshot) |
| `core/src/storage/wal.rs:286-299` | Method | `commit()` -- legacy full truncate (usable for day rollover) |
| `core/src/storage/accumulator.rs:1-191` | Full file | `Accumulator` struct and methods |
| `core/src/storage/mod.rs:1,5` | Module | `pub mod accumulator` and `pub use` |
| `core/src/storage/parquet.rs:512-562` | Method | `write_raw_parquet()` -- builds Polars DataFrame from points |
