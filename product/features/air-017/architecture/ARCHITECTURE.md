# AIR-017 Architecture: Bronze Write-Ahead Architecture

> **Author**: ndp-architect
> **Date**: 2026-02-08
> **Status**: Proposed
> **Feature**: air-017 (Bronze Write-Ahead Architecture -- Eliminate Read-Modify-Write)
> **Scope**: Phases 1-3 as defined in SCOPE.md

---

## 1. Overview

AIR-017 separates the durability concern (WAL) from the archival concern (Parquet) in the
Bronze layer. Today, durability and archival are tangled inside `ParquetStore.write_raw_batch()`:
the WAL is written only when the flush timer fires (up to 30s after event receipt), and
every flush reads the entire daily Parquet file, deserializes all rows, appends new points,
and rewrites the file. This is O(file_size) per flush -- 2,880 full-file rewrites per day.

After AIR-017, the WAL provides immediate durability (milliseconds after event receipt), an
in-memory accumulator holds the current day's data, and Parquet is written as a periodic
full-overwrite snapshot from the accumulator (no read required). Read-modify-write is
eliminated entirely.

### ADRs

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-AIR017-001](ADR-AIR017-001-wal-position.md) | WAL Position (Subscriber vs Store) | Proposed |
| [ADR-AIR017-002](ADR-AIR017-002-accumulator-design.md) | In-Memory Accumulator Design | Proposed |
| [ADR-AIR017-003](ADR-AIR017-003-wal-evolution.md) | WAL Evolution (Delete-All vs Watermark) | Proposed |
| [ADR-AIR017-004](ADR-AIR017-004-snapshot-strategy.md) | Snapshot Strategy (Overwrite vs Append) | Proposed |
| [ADR-AIR017-005](ADR-AIR017-005-polars-dependency.md) | Polars Dependency Impact | Proposed |

---

## 2. Component Diagram

### 2.1 Current Architecture (Being Replaced)

```
+--------------------+       +----------------------------+
|    EventBus        |       |     ParquetStore           |
|  broadcast channel |       |  (core/src/storage/        |
|                    |       |       parquet.rs)           |
+--------+-----------+       |                            |
         |                   |  write_raw_batch():        |
         v                   |    1. WAL.append()         |
+--------------------+       |    2. read Parquet file    |
| BronzeSubscriber   |       |    3. deserialize all rows |
| (core/src/         |------>|    4. append new batch     |
|  subscribers/      | flush |    5. rewrite entire file  |
|  bronze.rs)        |       |    6. WAL.commit() = del   |
|                    |       |                            |
| buffer: Vec<RDP>   |       |  wal: Arc<Mutex<WAL>>     |
| flush timer: 30s   |       |  base_path: PathBuf       |
+--------------------+       +----------------------------+
  No durability until              O(file_size) per flush
  flush timer fires                2,880 rewrites/day
```

### 2.2 Target Architecture (AIR-017)

```
+--------------------+
|     EventBus       |
|  broadcast channel |
+--------+-----------+
         |
         v
+------------------------------------------+
|  BronzeSubscriber                        |
|  (core/src/subscribers/bronze.rs)        |
|                                          |
|  On event receipt:                       |
|    1. WAL.append(seq, point)    <-- ms   |
|    2. accumulator.insert(point) <-- ms   |
|                                          |
|  +------------------+  +--------------+  |
|  | WriteAheadLog    |  | Accumulator  |  |
|  | (core/src/       |  | HashMap<     |  |
|  |  storage/wal.rs) |  |   StreamId,  |  |
|  |                  |  |   Vec<RDP>>  |  |
|  | seq_counter: u64 |  |              |  |
|  | watermark: u64   |  | ~22 MiB     |  |
|  +------------------+  +--------------+  |
|                              |           |
|  Snapshot timer (30-60 min): |           |
|    1. Write accumulator ---->| Parquet   |
|       full overwrite         | (no read) |
|    2. WAL.commit(watermark)  |           |
|                              |           |
|  Day rollover (midnight):    |           |
|    1. Final snapshot         |           |
|    2. Clear yesterday WAL    |           |
|    3. Reset accumulator      |           |
|                              |           |
|  Startup recovery:           |           |
|    1. Read today's Parquet   |           |
|    2. Replay WAL > watermark |           |
|    3. Merge into accumulator |           |
+------------------------------------------+
         |
         v
+------------------------------------------+
|  ParquetStore                            |
|  (core/src/storage/parquet.rs)           |
|                                          |
|  write_snapshot(points, path):           |
|    - Write all points as new file        |
|    - No read, no deserialize             |
|    - O(day's data) write only            |
|                                          |
|  query_raw(start, end, filter):          |
|    - Read from Parquet files (unchanged) |
|    - Phase 3: merge with accumulator     |
+------------------------------------------+
```

### 2.3 Data Flow Summary

```
Event --> WAL (durable, ms) --> Accumulator (in-memory)
                                     |
                            Snapshot timer fires
                                     |
                                     v
                              Parquet overwrite (no read)
                                     |
                              WAL truncate to watermark
```

---

## 3. Module Boundaries

### 3.1 Files Changed

| File | Change Type | Description |
|------|-------------|-------------|
| `core/src/subscribers/bronze.rs` | Major rewrite | Owns WAL + accumulator; adds snapshot timer, day rollover timer, startup recovery |
| `core/src/storage/wal.rs` | Major rewrite | Sequence-numbered entries, watermark-based commit, replay-since |
| `core/src/storage/parquet.rs` | Minor change | Add `write_snapshot()` method (full overwrite, no read); keep `append_to_parquet()` and `append_to_raw_parquet()` for backward compat until all callers migrate |
| `core/src/traits.rs` | Minor change | Add `write_raw_snapshot(&self, points: Vec<RawDataPoint>, path: &Path)` to `RawStore` trait (Phase 1); add `query_raw_with_memory()` (Phase 3) |
| `config/base/platform.yaml` | Add fields | `snapshot_interval_secs: 1800`, `day_rollover_utc_hour: 0` |

### 3.2 Files NOT Changed

| File | Reason |
|------|--------|
| `core/src/subscribers/silver.rs` | Silver receives events directly from EventBus, not from Bronze |
| `core/src/types/raw_data_point.rs` | RawDataPoint struct is unchanged |
| `core/src/storage/parquet.rs` read path | `query_raw()` is unchanged in Phases 1-2; Phase 3 adds accumulator merge |
| Silver ETL logic | Out of scope per SCOPE.md |

### 3.3 New Types

```rust
// In core/src/storage/wal.rs
pub struct WalEntry {
    pub seq: u64,                    // Monotonically increasing sequence number
    pub point: RawDataPoint,         // The data point (serialized as JSON line)
}

// In core/src/subscribers/bronze.rs
pub struct BronzeAccumulator {
    points: HashMap<String, Vec<RawDataPoint>>,  // stream_id -> points
    count: usize,                                 // Total point count across all streams
}

// In core/src/subscribers/bronze.rs (config extension)
pub struct BronzeSubscriberConfig {
    pub batch_size: usize,                // Kept: WAL batch threshold for backpressure
    pub flush_interval_secs: u64,         // Repurposed: WAL flush interval
    pub snapshot_interval_secs: u64,      // New: Parquet snapshot interval (default 1800)
    pub day_rollover_utc_hour: u8,        // New: UTC hour for day finalization (default 0)
    pub max_retries: u32,                 // Kept: retry count for Parquet writes
    pub stream_filter: Vec<String>,       // Kept: optional stream filtering
}
```

---

## 4. Dependency Analysis

### 4.1 Crate-Level Dependencies

No new crate dependencies are introduced. All functionality uses existing dependencies:

| Dependency | Used For | Status |
|------------|----------|--------|
| `serde` / `serde_json` | WAL entry serialization | Existing |
| `polars` | Parquet write (via `write_raw_parquet`) | Existing; simplified usage (write-only, no read on flush) |
| `tokio` | `select!` loop, `interval`, `sleep_until`, `spawn_blocking` | Existing |
| `chrono` | Timestamp handling, day rollover computation | Existing |
| `tracing` | Structured logging | Existing |
| `uuid` | ndp_id generation | Existing |

### 4.2 Trait-Level Dependencies

```
BronzeSubscriber
  |-- owns WriteAheadLog (direct, not through trait)
  |-- owns BronzeAccumulator (direct, not through trait)
  |-- uses Arc<dyn RawStore> (existing trait, for snapshot writes)
  |-- receives broadcast::Receiver<Arc<RawDataPoint>> (existing EventBus pattern)
  |-- uses CancellationToken (existing shutdown pattern)

WriteAheadLog
  |-- no trait dependencies (concrete struct, file I/O only)

ParquetStore (implements RawStore)
  |-- removes: WAL ownership (moved to BronzeSubscriber)
  |-- adds: write_raw_snapshot() method (trait method on RawStore)
  |-- keeps: query_raw(), write_raw_parquet() (existing)
```

### 4.3 Breaking Change Analysis

The move of WAL from `ParquetStore` to `BronzeSubscriber` changes the `ParquetStore::new()` signature.
Today `ParquetStore` creates a WAL at `{base_path}/wal.log`. After AIR-017 Phase 1:

- `ParquetStore::new()` no longer creates a WAL.
- `ParquetStore.wal` field is removed.
- `write_raw_batch()` no longer calls WAL append/commit (BronzeSubscriber does this).
- `write_raw_batch()` is kept for backward compatibility but becomes a thin wrapper over
  `write_raw_snapshot()` (or is deprecated).

The `RawStore` trait gains `write_raw_snapshot()`. Existing implementors (only `ParquetStore`
and `MockRawStore`) must implement it. The mock gains a trivial implementation.

---

## 5. Memory Model

### 5.1 Accumulator Sizing

Based on current production volumes (4 streams, ~11,000 points/stream/day):

| Component | Calculation | Size |
|-----------|-------------|------|
| `RawDataPoint` in-memory size | ~500 bytes (source_id ~30B + ndp_id ~30B + context ~100B + raw_payload ~300B + timestamp 8B + String overhead) | 500 B |
| Points per stream per day | ~11,000 (one every ~8 seconds) | -- |
| Streams | 4 (air-quality, outdoor-air-quality, outdoor-weather, nws-observations) | -- |
| Total points in accumulator | 4 x 11,000 = 44,000 | -- |
| **Accumulator memory** | 44,000 x 500 B | **~22 MiB** |
| HashMap overhead | 4 entries + Vec overhead | negligible |

### 5.2 Peak RSS During Snapshot

During a Parquet snapshot, the accumulator data must be converted to column vectors
(Polars Series). This creates a transient copy:

| Component | Size |
|-----------|------|
| Accumulator (retained during snapshot) | ~22 MiB |
| Column vectors for Parquet write | ~22 MiB (transient) |
| Polars DataFrame overhead | ~2-5 MiB |
| **Peak during snapshot** | **~46-49 MiB** |

The column vectors are freed after the `spawn_blocking` write completes.

### 5.3 Total Application RSS Budget

| Component | Baseline | Peak |
|-----------|----------|------|
| Runtime (tokio, tracing, allocator) | ~20 MiB | ~20 MiB |
| MQTT client + buffers | ~10 MiB | ~15 MiB |
| HTTP clients (reqwest) | ~10 MiB | ~15 MiB |
| EventBus channels | ~5 MiB | ~10 MiB |
| Bronze accumulator | ~22 MiB | ~22 MiB |
| Snapshot write (transient) | 0 | ~27 MiB |
| Silver subscriber | ~5 MiB | ~10 MiB |
| WAL file (memory-mapped reads) | ~1 MiB | ~5 MiB |
| **Total** | **~73 MiB** | **~124 MiB** |

This is well within the Docker 512 MiB limit. The peak is lower than the current
architecture because read-modify-write loads the entire Parquet file (~22 MiB at end of day)
plus the new batch, resulting in similar or higher peaks.

### 5.4 WAL File Size

WAL entries are JSON lines, approximately 500 bytes per entry. Between snapshots
(30 minutes at ~0.5 points/second across all streams):

- Points per snapshot interval: ~900 points
- WAL file size between snapshots: ~450 KB
- Maximum WAL size (24 hours without snapshot, failure case): ~22 MB

WAL size is bounded by the snapshot interval under normal operation.

---

## 6. Concurrency Model

### 6.1 Single-Task Ownership (Phase 1)

All mutable state lives inside `BronzeSubscriber`'s `start()` method, which runs in a
single tokio task. The `select!` loop processes events sequentially:

```
tokio::select! {
    biased;

    // Priority 1: Cancellation
    _ = cancellation_token.cancelled() => { ... }

    // Priority 2: Day rollover (Phase 2)
    _ = day_rollover_timer => { ... }

    // Priority 3: Snapshot timer
    _ = snapshot_timer.tick() => { ... }

    // Priority 4: Flush timer (WAL batch flush)
    _ = flush_timer.tick() => { ... }

    // Priority 5: Event receipt
    result = receiver.recv() => { ... }
}
```

There is no shared mutable state. The accumulator, WAL handle, and flush buffer are all
owned by the single task. No `Arc<Mutex<>>` is needed for the accumulator in Phase 1.

### 6.2 Snapshot Write Blocking

Parquet writes use `tokio::task::spawn_blocking` (existing pattern from `write_raw_parquet`).
During the snapshot write:

1. The accumulator is cloned (or drained into column vectors) on the async task.
2. Column vectors are moved into the `spawn_blocking` closure.
3. The async task awaits the blocking write.
4. During the await, events still arrive via the broadcast channel but are buffered
   by tokio's channel (capacity is bounded by EventBus channel size, currently 1024).

If the snapshot write takes longer than the event arrival rate can sustain in the channel
buffer, events will be lagged. At ~0.5 events/second and a channel capacity of 1024,
the snapshot write has ~2,000 seconds of headroom -- far exceeding any realistic write time.

### 6.3 Phase 3 Shared Access

Phase 3 requires the accumulator to be readable by query methods (Silver catch-up, MCP).
This will require either:

- **Option A**: `Arc<RwLock<BronzeAccumulator>>` shared between BronzeSubscriber and query callers.
  BronzeSubscriber holds write lock during insert (brief), readers hold read lock during query.
- **Option B**: A snapshot of the accumulator published to a shared `Arc<ArcSwap<...>>` on each
  WAL flush, providing lock-free reads at the cost of slight staleness.

Phase 3 design is deferred. Phase 1 accumulator is private to BronzeSubscriber.

---

## 7. Phase Boundaries

### 7.1 Phase 1: WAL on Receipt + Accumulator + Periodic Snapshot

**Goal**: Eliminate read-modify-write. Achieve millisecond durability.

**Changes**:
- `WriteAheadLog` gains sequence numbers (`seq: u64` per entry).
- `BronzeSubscriber` creates and owns the WAL (moved from ParquetStore).
- `BronzeSubscriber` gains `BronzeAccumulator` (HashMap<String, Vec<RawDataPoint>>).
- On event receipt: WAL.append(seq, point), then accumulator.insert(point).
- Flush timer repurposed: no longer calls `store.write_raw_batch()`. Instead, a no-op
  in Phase 1 (WAL is written per-event or per-batch; accumulator is already in memory).
  The flush timer may be kept for WAL fsync batching if per-event fsync is too costly.
- New snapshot timer (default 1800s): calls `store.write_raw_snapshot(accumulator.drain_stream(stream_id), path)`.
- `ParquetStore` gains `write_raw_snapshot()`: writes all points to Parquet, full overwrite.
- `ParquetStore.write_raw_batch()` is simplified to call `write_raw_snapshot()` internally
  (removes WAL logic from ParquetStore).
- Startup: no recovery logic yet (accumulator starts empty, WAL is committed after each
  snapshot). Acceptable because Phase 1 WAL commit still deletes the full file.

**Config changes**:
```yaml
subscribers:
  bronze:
    enabled: true
    batch_size: 100
    flush_interval_secs: 30
    snapshot_interval_secs: 1800    # NEW
    max_retries: 3
```

**Test plan**:
- Unit: WAL append with sequence numbers, accumulator insert/drain, snapshot write.
- Integration: Event receipt -> WAL durable -> snapshot creates correct Parquet.
- Property: Snapshot Parquet contains exactly the same points as accumulator.

### 7.2 Phase 2: Day Rollover + WAL Watermarking

**Goal**: Handle multi-day operation. WAL survives across snapshots.

**Changes**:
- `WriteAheadLog.commit(watermark: u64)`: truncates entries with `seq <= watermark`,
  retains entries with `seq > watermark`.
- Day rollover timer: computes next midnight UTC, triggers final snapshot for yesterday,
  clears yesterday's entries from accumulator, resets WAL watermark.
- Startup recovery: read today's Parquet (if exists) to seed accumulator, replay WAL
  entries with `seq > last_snapshot_watermark` to fill the gap.
- `WriteAheadLog` stores the current watermark in a header line or companion `.watermark` file.

**Config changes**:
```yaml
subscribers:
  bronze:
    day_rollover_utc_hour: 0    # NEW
```

**Test plan**:
- Unit: WAL watermark-based truncation, replay-since-watermark, day rollover logic.
- Integration: Multi-day simulation (fast-forward clock), crash recovery with WAL replay.
- Property: After recovery, accumulator matches pre-crash state.

### 7.3 Phase 3: Read Path Integration

**Goal**: Queries see in-memory data, not just stale Parquet.

**Changes**:
- Accumulator becomes shared (`Arc<RwLock<BronzeAccumulator>>`).
- `RawStore.query_raw_with_memory()` merges Parquet results with accumulator data.
- Silver catch-up (`BronzeReader.read_since()`) uses the merged query.
- Decision on Silver resilience approach (Options A-D from SCOPE.md) is made here.

**Test plan**:
- Unit: Merged query returns union of Parquet + accumulator data, deduped by timestamp+source.
- Integration: Silver catch-up after Bronze restart returns complete data including recent events.

---

## 8. Error Handling

### 8.1 WAL Append Failure

If `WAL.append()` fails (disk full, I/O error):

- Log error via `tracing::error!`.
- The event is NOT added to the accumulator (durability-first: no accumulator without WAL).
- The event is effectively dropped from Bronze persistence.
- Silver subscriber still receives the event via EventBus (independent path).
- Metric `wal_errors_total` incremented.
- If errors are sustained, health check reports unhealthy.

### 8.2 Snapshot Write Failure

If `store.write_raw_snapshot()` fails:

- Log error via `tracing::error!`.
- WAL is NOT committed (watermark not advanced).
- Accumulator is NOT cleared.
- Next snapshot timer tick will retry with the same data plus any new events.
- WAL grows until the next successful snapshot.
- If WAL exceeds a configurable size limit (e.g., 50 MB), log a warning. Do not drop data.

### 8.3 Day Rollover Failure

If the final snapshot for the previous day fails during day rollover:

- Retry up to `max_retries` times.
- If all retries fail, log critical error but continue. The WAL retains yesterday's entries.
- On the next snapshot tick, the snapshot will include yesterday's data alongside today's
  (the accumulator has not been cleared for yesterday).
- This is a degraded state but prevents data loss.

### 8.4 Startup Recovery Failure

If Parquet read fails during startup recovery:

- Log error, start with empty accumulator.
- Replay full WAL (all entries, ignoring watermark) to rebuild what is possible.
- If WAL replay also fails, start fresh (data since last successful snapshot is lost).
- This matches the current behavior where a corrupted Parquet file causes data loss
  for that day.

---

## 9. Configuration

### 9.1 New Configuration Fields

Added to `config/base/platform.yaml` under `subscribers.bronze`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `snapshot_interval_secs` | u64 | 1800 | Seconds between Parquet snapshot writes |
| `day_rollover_utc_hour` | u8 | 0 | UTC hour to finalize the daily file (0 = midnight) |

### 9.2 Repurposed Fields

| Field | Before | After |
|-------|--------|-------|
| `batch_size` | Events before flush to Parquet | Events before WAL batch write (backpressure threshold) |
| `flush_interval_secs` | Seconds between Parquet rewrites | Seconds between WAL batch flushes (kept for WAL batching) |

### 9.3 Configuration Hierarchy

Per NDP conventions, configuration follows the priority chain:

1. Stream Registry (`/streams/{id}/config` in etcd) -- future
2. Legacy etcd (`/config/{app}/*`) -- future
3. YAML files (`config/base/platform.yaml`) -- current
4. Code defaults (in `BronzeSubscriberConfig::default()`) -- current

---

## 10. Observability

### 10.1 Metrics (tracing structured fields)

| Metric | Type | Description |
|--------|------|-------------|
| `events_received` | counter | Total events received from EventBus |
| `wal_entries_written` | counter | Total WAL entries written |
| `wal_errors_total` | counter | WAL append failures |
| `accumulator_points` | gauge | Current points in accumulator |
| `accumulator_bytes_estimate` | gauge | Estimated memory usage of accumulator |
| `snapshots_written` | counter | Successful Parquet snapshots |
| `snapshot_errors_total` | counter | Failed Parquet snapshots |
| `snapshot_duration_ms` | histogram | Time to write a snapshot |
| `snapshot_points` | histogram | Points written per snapshot |
| `day_rollovers` | counter | Successful day rollovers |
| `wal_size_bytes` | gauge | Current WAL file size |
| `wal_watermark` | gauge | Current WAL watermark (last committed seq) |

### 10.2 Health Check

`BronzeSubscriber.health_check()` reports:

- `healthy: true` if running and no sustained errors.
- `accumulator_points`: current count.
- `last_snapshot_time`: timestamp of last successful snapshot.
- `wal_size_bytes`: current WAL file size.
- `wal_watermark`: current committed sequence number.

---

## 11. Migration Strategy

### 11.1 Backward Compatibility

The file format (Parquet schema, file naming, directory structure) is unchanged.
Existing Parquet files written by the old architecture are readable by the new one.
The only difference is write frequency (every 30s vs every 30 min) and write method
(full overwrite vs read-modify-write).

### 11.2 Rollback Plan

If AIR-017 Phase 1 causes issues:

1. Revert the BronzeSubscriber to the pre-AIR-017 version.
2. ParquetStore's `write_raw_batch()` still exists (backward compat).
3. Delete the new WAL file format (watermarked WAL vs plain WAL).
4. No data migration needed -- Parquet files are the same format.

### 11.3 Feature Flag

No feature flag is needed. The architecture change is internal to BronzeSubscriber
and ParquetStore. The external interface (EventBus events in, Parquet files out,
RawStore trait methods) is unchanged or additive (new trait method).

---

## 12. Relationship to Other Features

| Feature | Relationship |
|---------|-------------|
| air-016 | air-016 Phase 1 changes flush_interval_secs from 5s to 30s. AIR-017 builds on this by making the flush interval a WAL concern (fast) and adding a separate snapshot interval (slow). air-016 is not a prerequisite. |
| dp-012 | dp-012 established the BronzeSubscriber/SilverSubscriber pattern. AIR-017 extends BronzeSubscriber but does not change dp-012's EventBus architecture. |
| Silver catch-up | Silver's `BronzeReader.read_since()` reads Parquet files. After AIR-017 Phase 1-2, Parquet is up to 1 snapshot interval stale. Phase 3 addresses this. |
| ops-003 | ndp-validate validates stream configs and domain configs. No impact from AIR-017. |
