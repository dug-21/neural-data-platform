# AIR-017 Refinement: Bronze Write-Ahead Architecture

> **Feature:** air-017
> **SPARC Phase:** Refinement
> **Date:** 2026-02-08
> **Status:** Complete

This document identifies risks, edge cases, performance concerns, and iterative improvements for the Bronze Write-Ahead Architecture. Every scenario has a concrete mitigation -- nothing is left as "TBD".

---

## 1. Edge Cases and Boundary Conditions

### 1.1 Empty WAL on Startup

**Scenario:** Process starts and the WAL file is empty (zero bytes) or contains only whitespace lines.

**What goes wrong:** Nothing. The current `WriteAheadLog::replay()` at `core/src/storage/wal.rs:34-46` already returns an empty `Vec<Vec<u8>>` when the file is empty, because the `BufReader::lines()` iterator produces no non-empty lines.

**Mitigation:** Accumulator is seeded from the existing Parquet file only. If Parquet exists, it becomes the full accumulator state. If no Parquet exists either, see 1.3.

### 1.2 Empty Parquet on Startup (First Day)

**Scenario:** The process starts on the first day of operation for a stream. No Parquet file exists for today. WAL may or may not have entries.

**What goes wrong:** `path.exists()` returns false in the Parquet read path (`core/src/storage/parquet.rs:160` for parsed, `core/src/storage/parquet.rs:570` for raw). The accumulator starts empty, which is correct.

**Mitigation:** Accumulator starts at zero points. WAL entries (if any) are replayed into the empty accumulator. First snapshot creates the Parquet file. No special handling required beyond the normal startup sequence:

```
1. Check for today's Parquet file -> not found -> accumulator = empty HashMap
2. Replay WAL entries after watermark -> append to accumulator
3. Resume normal operation
```

### 1.3 Both WAL and Parquet Empty (Fresh Install)

**Scenario:** Brand new deployment. No data directory exists. No WAL, no Parquet.

**What goes wrong:** Nothing, provided directory creation is handled. The current `ParquetStore::new()` at `core/src/storage/parquet.rs:21-32` calls `create_dir_all(&base_path)`. The WAL constructor at `core/src/storage/wal.rs:12-22` also calls `create_dir_all(parent)`.

**Mitigation:** Both paths already handle missing directories. Accumulator starts empty. First event appended to WAL creates the file. First snapshot creates the Parquet directory tree. No special case needed.

### 1.4 Process Crash During Snapshot Write (Parquet Corruption)

**Scenario:** The process is killed (SIGKILL, OOM, power loss) while writing a Parquet snapshot from the accumulator. The file on disk may be truncated, have an incomplete footer, or be zero bytes.

**What goes wrong:** On next startup, the Parquet read fails with a deserialization error (incomplete Parquet footer). The accumulator cannot be seeded from the corrupt file, so the process either crashes on startup or starts with an empty accumulator and loses all data that was only in Parquet.

**Mitigation:** Write snapshot to a temporary file, then atomic rename.

```rust
// Snapshot write procedure:
// 1. Write accumulator to: {partition_dir}/data.parquet.tmp
// 2. fsync the file
// 3. Rename data.parquet.tmp -> data.parquet (atomic on ext4, btrfs, overlayfs)
// 4. fsync the parent directory

// On startup:
// 1. If data.parquet.tmp exists and data.parquet does not:
//    -> Previous snapshot completed write but crashed before rename.
//       Try to read .tmp; if valid, rename to .parquet. If corrupt, delete .tmp.
// 2. If both data.parquet.tmp and data.parquet exist:
//    -> Crash between rename and cleanup. Delete .tmp, use .parquet.
// 3. If only data.parquet exists:
//    -> Normal case. Use it.
```

`rename(2)` is atomic on ext4/btrfs (the filesystems used in Raspberry Pi OS and Docker overlay). This is the standard pattern used by SQLite, RocksDB, and Parquet-based systems like Delta Lake.

**Implementation detail:** Use `std::fs::rename()` which maps to `rename(2)`. The parent directory fsync ensures durability across power loss. On Pi with SD card, fsync latency is 5-20ms -- acceptable for snapshot writes that happen every 30-60 minutes.

### 1.5 Process Crash During WAL Commit (Truncation)

**Scenario:** The current WAL commit (`core/src/storage/wal.rs:49-60`) deletes the file and recreates it. If the process crashes between `remove_file` and opening a new file, the WAL file is gone entirely. Under air-017, commit evolves to watermark-based truncation. If the process crashes mid-truncation, the WAL may be partially truncated.

**What goes wrong with current design:** WAL is deleted. On restart, replay finds no WAL. If the Parquet file was already written (commit happens after Parquet write), no data loss. If Parquet write failed and WAL was still committed (not possible in current code -- see `write_raw_batch` at `core/src/storage/parquet.rs:710-741` which writes Parquet before committing WAL), data would be lost. The current ordering is correct: Parquet write first, then WAL commit.

**What goes wrong with watermark-based truncation:** If the WAL is rewritten (copy entries after watermark to new file, rename), a crash during this process could lose the WAL.

**Mitigation:** Two-phase WAL commit:

1. Write the new watermark to a separate file (`wal.watermark`), fsync it.
2. On replay, ignore all WAL entries with sequence number <= watermark.
3. Periodically (or on day rollover) compact the WAL by rewriting only entries after the watermark.

This means a crash during step 1 leaves the old watermark intact (replay processes more entries than needed, which is safe because the accumulator deduplicates). A crash during step 3 leaves the full WAL intact (watermark already advanced, so extra entries are skipped on replay).

The watermark file is a single integer (8 bytes). Writing it is effectively atomic on any filesystem because it fits in a single disk sector.

### 1.6 Midnight Rollover During Snapshot

**Scenario:** The snapshot timer (every 30-60 min) and the day rollover timer (midnight UTC) fire at the same time. Both attempt to write Parquet and manipulate the accumulator concurrently.

**What goes wrong:** Double write, race condition on accumulator state.

**Mitigation:** Both timers run in the same `select!` loop in `BronzeSubscriber::start()` (see `core/src/subscribers/bronze.rs:223-268`). The `select!` macro is already `biased` (line 225), which means branches are evaluated in order. Place the rollover branch before the snapshot branch:

```rust
tokio::select! {
    biased;

    // Priority 1: Cancellation
    _ = cancellation_token.cancelled() => { break; }

    // Priority 2: Day rollover (includes forced final snapshot of yesterday)
    _ = rollover_timer.tick() => {
        self.force_snapshot().await;  // Snapshot yesterday's data
        self.rollover().await;         // Archive yesterday, start fresh accumulator
        self.recompute_rollover_timer();
    }

    // Priority 3: Periodic snapshot
    _ = snapshot_timer.tick() => {
        self.snapshot().await;
    }

    // Priority 4: Receive events
    result = receiver.recv() => { ... }
}
```

Because `biased` ensures rollover is checked before snapshot, the rollover branch "wins" when both are ready. The rollover handler performs a forced snapshot as its first step, so the snapshot timer's tick is effectively consumed (the accumulator was just snapshot). The snapshot branch, if it runs after rollover, finds the accumulator nearly empty (only events received since rollover) and writes a small or empty Parquet -- harmless.

### 1.7 WAL Grows Unbounded (Snapshot Failures)

**Scenario:** Snapshot writes keep failing (disk full, permissions issue, Parquet serialization bug). The WAL accumulates entries without ever being truncated. WAL file grows without bound.

**What goes wrong:** Disk fills up. Startup replay becomes slow (reading and deserializing the entire WAL). Memory spikes on replay (loading all entries at once).

**Mitigation:** Three-layer defense:

1. **WAL size cap** (configurable, default 50 MiB). When the WAL file exceeds the cap, log a `tracing::error!` and stop accepting new WAL writes. Events continue to flow into the in-memory accumulator (so no data loss for the current session), but durability is lost until the snapshot succeeds and the WAL can be truncated. The size cap prevents disk exhaustion.

2. **Consecutive failure counter**. Track how many snapshots have failed in a row. After N consecutive failures (configurable, default 5), emit a health check degradation via the existing `HealthStatus` mechanism (`core/src/subscribers/bronze.rs:304-331`). This surfaces in the `/health` endpoint for Docker health checks.

3. **Startup WAL size check**. On startup, if the WAL is larger than 2x the size cap, log a warning. Replay proceeds normally but the operator is alerted that something went wrong in the previous session.

### 1.8 Accumulator Memory Exceeds Budget

**Scenario:** Unexpected data volume (new streams added, polling frequency increased, data payloads larger than expected). The in-memory accumulator grows beyond the ~22 MiB estimate toward the 512 MiB container limit.

**What goes wrong:** OOM kill by Docker. Process restart loses the accumulator (but WAL recovers it). Repeated OOM-restart cycles if the volume stays high.

**Mitigation:**

1. **Memory estimation**. The accumulator tracks its approximate memory usage. For `RawDataPoint`: `timestamp` (12 bytes) + `source_id` (heap string ~30 bytes) + `ndp_id` (Option<String> ~30 bytes) + `context` (Option<Value> ~50 bytes) + `raw_payload` (Value ~200-500 bytes). Estimated ~500 bytes per point. The accumulator exposes `fn memory_estimate(&self) -> usize` which returns `point_count * 500 + overhead`.

2. **Emergency snapshot threshold** (configurable, default 100 MiB). When `memory_estimate()` exceeds this threshold, trigger an immediate snapshot regardless of the snapshot timer. This writes the current accumulator to Parquet, truncates the WAL, and resets the accumulator. The threshold of 100 MiB provides headroom: with ~80-100 MiB baseline runtime and 100 MiB accumulator, peak during snapshot is ~300 MiB, well under the 512 MiB limit.

3. **Point count hard limit** (configurable, default 500,000 points). A secondary safety valve. If `point_count > limit`, force snapshot. At ~500 bytes/point, 500K points = ~250 MiB.

### 1.9 Clock Skew / NTP Jump

**Scenario:** System clock jumps forward or backward due to NTP synchronization. This is common on Raspberry Pi which has no battery-backed RTC and synchronizes time on boot.

**What goes wrong:**
- **Clock jumps forward past midnight:** Rollover fires early. Yesterday's data is finalized prematurely, but any events with timestamps before midnight are already in the accumulator and get archived correctly. Events arriving "after midnight" (per the jumped clock) go into a new day's accumulator. When NTP corrects the clock backward, events may have timestamps in "yesterday" but the accumulator is for "today". These events end up in today's accumulator with yesterday's timestamps.
- **Clock jumps backward:** The rollover timer, computed as "time until next midnight", suddenly has hours to wait again. No premature rollover. Events continue accumulating normally.

**Mitigation:**
- Use wall-clock computation for rollover: `let next_midnight = (Utc::now().date_naive() + chrono::Duration::days(1)).and_hms_opt(rollover_hour, 0, 0)`. Recompute this after every rollover event, not using `interval(86400s)` which would drift.
- After rollover, recompute the timer: `tokio::time::sleep_until(tokio::time::Instant::now() + (next_midnight - Utc::now()).to_std().unwrap_or(Duration::from_secs(3600)))`. If the duration is negative (clock jumped forward past midnight), trigger rollover immediately.
- The accumulator is keyed by `(source_id, timestamp)`. Events with "wrong day" timestamps simply have inaccurate partition placement. This is acceptable for Bronze -- the Silver ETL layer uses the event timestamp for its own partitioning and does not rely on Bronze file placement.

### 1.10 Duplicate Entries After Recovery

**Scenario:** Process crashes after writing some events to both WAL and Parquet (via a snapshot), then recovers. WAL replay adds entries that already exist in the Parquet snapshot.

**What goes wrong:** Duplicate points in the accumulator. Subsequent Parquet snapshots contain duplicates. Silver catch-up reads duplicates.

**Mitigation:** Watermark-based deduplication. Each WAL entry gets a monotonically increasing sequence number. The snapshot records which sequence number it includes up to (the watermark). On recovery:

```
1. Read today's Parquet file -> seed accumulator (these are all entries up to watermark W)
2. Read watermark file -> W
3. Replay WAL, skipping entries with sequence <= W
4. Accumulator now has Parquet data + WAL entries after W = complete, no duplicates
```

The sequence number is a `u64` counter that starts at 0 when the WAL is first created and increments for every `append()`. It is written as part of the WAL entry (prepended or as a JSON field). The watermark file stores the sequence number of the last entry included in the most recent snapshot.

Alternative approach: dedup by `(source_id, timestamp)` during accumulator insertion. Since sensor readings have unique timestamps per source (microsecond precision), a `HashMap<(String, DateTime<Utc>), RawDataPoint>` naturally deduplicates. This is simpler than sequence numbers and handles the common case. The downside is that two genuinely different readings with the same timestamp from the same source would be collapsed -- acceptable for NDP where sensor readings are periodic, not sub-microsecond.

**Recommended approach:** Use the `HashMap` keyed by `(source_id, timestamp)` for the accumulator. This provides natural deduplication without requiring sequence numbers in Phase 1. Add sequence-based watermarking in Phase 2 when WAL truncation is implemented.

### 1.11 Source ID Changes Mid-Day (Stream Reconfiguration)

**Scenario:** An operator reconfigures a stream mid-day (changes source type from MQTT to HTTP, or renames the stream). The `source_id` in incoming events changes from `air-quality-Mqtt` to `air-quality-Http`.

**What goes wrong:** The accumulator has entries under the old source_id. New entries arrive under a different source_id. The Parquet partition path uses `extract_stream_id(source_id)` which strips the protocol suffix (`core/src/storage/parquet.rs:460-470`), so both `air-quality-Mqtt` and `air-quality-Http` map to the same directory `raw/air-quality/...`. No file split.

**Mitigation:** No action required for the common case. Since `extract_stream_id` normalizes away the protocol suffix, both old and new source_ids map to the same partition directory and Parquet file. The accumulator, keyed by `(source_id, timestamp)`, stores entries under both source_ids -- this is correct because they represent different source channels for the same stream.

If the stream is truly renamed (not just the protocol), the old stream's data stays in its Parquet file and the new stream starts fresh. This is correct behavior: stream renaming is a schema-level operation that should not retroactively rewrite existing data.

### 1.12 Partial WAL Line on Crash

**Scenario:** Process crashes mid-`writeln!()` in `WriteAheadLog::append()` (`core/src/storage/wal.rs:28`). The last line of the WAL file is truncated JSON.

**What goes wrong:** `serde_json::from_slice()` fails on the partial line during replay.

**Mitigation:** The current WAL replay at `core/src/storage/wal.rs:39-44` reads lines. A partial last line is a valid string (just incomplete JSON). The replay must handle deserialization failure on the last line gracefully:

```rust
for (i, line) in reader.lines().enumerate() {
    let line = line?;
    if line.trim().is_empty() {
        continue;
    }
    match serde_json::from_str::<RawDataPoint>(&line) {
        Ok(point) => entries.push(point),
        Err(e) => {
            // If this is the last line, it's likely a partial write from a crash.
            // Log a warning and skip it. If it's a middle line, something is more
            // seriously wrong, but we still skip and continue.
            warn!(line_number = i + 1, error = %e, "Skipping corrupt WAL entry");
        }
    }
}
```

Data loss from a partial last line: at most 1 event (the one being written when the crash occurred). This is the standard WAL guarantee: all events up to the last successful `flush()` are durable.

### 1.13 WAL Replay with Mixed Stream Data

**Scenario:** The WAL contains entries from multiple streams (air-quality, outdoor-weather, nws-forecast). On replay, entries must be routed to the correct accumulator bucket.

**What goes wrong:** If the accumulator is a flat `Vec<RawDataPoint>`, stream interleaving is fine for the accumulator but complicates Parquet snapshot writing (must partition by stream_id).

**Mitigation:** The accumulator is a `HashMap<String, Vec<RawDataPoint>>` keyed by stream_id (extracted from source_id). On event receipt and during WAL replay:

```rust
let stream_id = extract_stream_id(&point.source_id);
self.accumulator.entry(stream_id.to_string())
    .or_default()
    .push(point);
```

During snapshot, iterate the map and write one Parquet file per stream per day partition. This aligns with the current partitioning strategy (`raw/{stream_id}/year={Y}/month={M}/day={D}/data.parquet`).

### 1.14 Concurrent Snapshot and Event Receipt

**Scenario:** While a snapshot is being written (in `spawn_blocking`), new events arrive and are appended to the accumulator and WAL.

**What goes wrong:** The snapshot writes a frozen copy of the accumulator. Events arriving during the snapshot write are in the WAL but not in the Parquet snapshot. This is correct behavior -- those events will be included in the next snapshot.

**Mitigation:** Before starting the snapshot, clone or drain the accumulator into a snapshot buffer. Subsequent events go into a fresh accumulator. The WAL watermark is advanced to the last sequence number in the snapshot buffer, not the last event in the accumulator.

```rust
async fn snapshot(&mut self) {
    // 1. Freeze current accumulator
    let snapshot_data = std::mem::take(&mut self.accumulator);
    let snapshot_watermark = self.wal_sequence_counter;
    // From this point, new events go into the fresh (empty) accumulator

    // 2. Write snapshot to Parquet (blocking I/O)
    let result = self.write_snapshot(snapshot_data).await;

    match result {
        Ok(()) => {
            // 3. Advance WAL watermark
            self.write_watermark(snapshot_watermark);
        }
        Err(e) => {
            // 4. Snapshot failed -- merge frozen data back into accumulator
            // (Events received during the failed snapshot are already in self.accumulator)
            self.merge_back(snapshot_data);
            error!(error = %e, "Snapshot failed, data preserved in accumulator");
        }
    }
}
```

**Critical detail:** If the snapshot fails, the frozen data must be merged back. The merge is append-only (no dedup needed because the frozen data and new data have disjoint time ranges). The WAL watermark is NOT advanced on failure, so a crash after a failed snapshot replays all entries correctly.

---

## 2. Performance Analysis

### 2.1 WAL Write Latency

**Current path:** Events arrive via EventBus broadcast channel -> `BronzeSubscriber::handle_point()` adds to in-memory buffer -> flush timer fires every 30s -> `store.write_raw_batch()` -> WAL append (batch) -> Parquet read-modify-write -> WAL commit (delete).

**Proposed path:** Events arrive -> WAL append immediately -> accumulator insert -> (separate) snapshot timer fires -> Parquet write from accumulator -> WAL watermark advance.

**Per-event WAL cost:**
- `serde_json::to_vec()`: ~1-5 us for a `RawDataPoint` with ~500 bytes of JSON payload.
- `writeln!(file, "{}", json_str)`: ~10-50 us on Pi SD card for a sequential write.
- `file.flush()`: This calls `fflush()` (userspace buffer flush to kernel), NOT `fsync()`. The current WAL implementation at `core/src/storage/wal.rs:29` uses `flush()`. On Linux, `flush()` does NOT guarantee disk durability -- only that userspace buffers are handed to the kernel's page cache. Actual disk write happens asynchronously.

**Trade-off: flush() vs fsync():**
- `flush()` only (current): ~10-50 us per event. Durability gap: kernel may buffer up to 30 seconds (dirty page writeback interval) before writing to SD card. A power loss could lose up to 30s of WAL entries.
- `fsync()` per event: ~1-5 ms on Pi SD card (forces page cache to disk). At 4 streams with ~1 event per stream per 10 minutes, that is ~4 fsyncs per 10 minutes -- negligible. But during catch-up or burst replay, fsync per event would be a bottleneck.
- **Recommendation: Use `flush()` for normal operation (current behavior). Add `fsync()` only on snapshot watermark writes and day rollover.** The rationale: the WAL's purpose is crash recovery, not power-loss recovery. For power loss, the last Parquet snapshot (30-60 min old) is the recovery point. The WAL recovers from process crashes (SIGSEGV, OOM kill) where the kernel's page cache is preserved.

**Batch WAL writes:** If future data volume increases (more streams, higher frequency), batch N events into a single WAL write+flush. The current architecture already batches in BronzeSubscriber's buffer. Moving WAL writes to the per-event path means each event gets its own writeln+flush. For the current 4 streams at ~0.8 events/min total, this is 0.8 writes/min -- zero concern. If volume grows to 100 events/sec, batch WAL writes every 100ms (10 events per write).

### 2.2 Snapshot Write Time

**Data volume:** 4 streams x ~11,000 points/day x ~500 bytes = ~22 MiB of data.

**Parquet write pipeline:**
1. Build column vectors from accumulator: O(N) where N = point count. ~44K points, ~1-2 ms.
2. Create DataFrame: O(N), ~1 ms.
3. Snappy compression + Parquet serialization: Snappy is fast (~250 MB/s on ARM Cortex-A76). For 22 MiB: ~90ms compression time.
4. File write: 22 MiB compressed (Snappy typically achieves 2-4x on JSON-heavy data, so ~6-11 MiB on disk). SD card sequential write: ~20-40 MB/s. Write time: ~0.3-0.6s.
5. fsync: ~5-20ms.

**Total estimated snapshot time: 500ms - 2 seconds.** This is well within the 30-60 minute snapshot interval. Even if snapshot takes 5 seconds in pathological cases, it blocks only the `spawn_blocking` thread, not the async runtime. Event receipt continues in the `select!` loop.

**Comparison to current read-modify-write:**
- Current: Read 22 MiB Parquet + deserialize all rows + append + rewrite. The read+deserialize phase takes ~1-3 seconds. Total per flush: 2-6 seconds, happening every 30 seconds = 7-20% of time spent on I/O.
- Proposed: Write 22 MiB from memory (no read). Total per snapshot: 0.5-2 seconds, happening every 30-60 minutes = 0.03-0.1% of time spent on I/O.

### 2.3 Memory Profile Through the Day

```
Time     Accumulator    WAL file     Parquet (latest)    Total RAM
00:00    ~0 MiB         ~0 MiB       N/A (new day)       ~80 MiB (baseline)
06:00    ~5.5 MiB       ~5.5 MiB     ~5.5 MiB (snap)     ~86 MiB
12:00    ~11 MiB        ~11 MiB*     ~11 MiB (snap)       ~92 MiB
18:00    ~16.5 MiB      ~16.5 MiB*   ~16.5 MiB (snap)     ~98 MiB
23:59    ~22 MiB        ~22 MiB*     ~22 MiB (snap)       ~104 MiB

Snapshot  +22 MiB spike (column vecs)    duration: <2s    peak: ~126 MiB
Rollover  accumulator reset to ~0         WAL truncated    drops to ~80 MiB

* WAL size only reflects entries since last snapshot (watermark-based truncation
  removes entries already in Parquet). Actual WAL size between snapshots: ~0.7 MiB
  (30 min of data at current volume).
```

**Key difference from current architecture:** No spikes from reading existing Parquet into memory. The current read-modify-write temporarily holds 2x the file in memory (existing data + new data). At end of day, this is ~44 MiB per flush. The new architecture's snapshot spike is also ~44 MiB (accumulator + column vecs) but happens every 30-60 minutes instead of every 30 seconds.

**Headroom:** Peak ~126 MiB vs. 512 MiB container limit = 75% headroom. Even with 2x current data volume (8 streams, ~44 MiB accumulator), peak would be ~210 MiB -- still under the limit.

### 2.4 SD Card Wear Analysis

**Current architecture:**
- Parquet writes: ~2,880/day (every 30 seconds). Each write rewrites the full file (6-11 MiB compressed by end of day). Daily write volume: ~2,880 * ~5 MiB average = ~14.4 GB/day.
- WAL writes: Same count, but WAL is small (one batch of JSON lines, then deleted).

**Proposed architecture:**
- Parquet writes: 24-48/day (every 30-60 minutes). Each write is a full overwrite (6-11 MiB). Daily write volume: ~48 * ~5 MiB = ~240 MB/day.
- WAL writes: ~40,000 lines/day (4 streams x 11K points, each written immediately). Each line is ~500 bytes. Daily WAL write volume: ~20 MB/day (sequential append, very SD-card friendly).

**Reduction: ~14.4 GB/day to ~260 MB/day = 55x reduction in write volume.** This significantly extends SD card lifespan. Consumer SD cards are typically rated for 10-100 TBW (terabytes written). At 14.4 GB/day, a 10 TBW card lasts ~690 days (~1.9 years). At 260 MB/day, the same card lasts ~38,500 days (~105 years).

---

## 3. Security Considerations

### 3.1 WAL File Permissions

The WAL contains raw sensor data. For the air-quality domain, this includes PM2.5, CO2, temperature, humidity readings and location identifiers (indoor sensor placement descriptions). While not credentials or PII in the strict sense, location data could reveal occupancy patterns.

**Mitigation:**
- WAL files: created with `0600` permissions (owner read/write only). The current `OpenOptions::new().create(true).append(true).open(&path)` at `core/src/storage/wal.rs:19` uses the process umask (typically `0022` on Linux, resulting in `0644`). This should be tightened. On Linux, use `std::os::unix::fs::OpenOptionsExt::mode(0o600)`.
- Parquet files: same treatment. Currently use default permissions via `std::fs::File::create()`.
- Watermark file: contains only a sequence number. No sensitive data. Default permissions acceptable.

### 3.2 No New Network Surfaces

Air-017 introduces no new network listeners, API endpoints, or inter-process communication channels. All changes are local filesystem I/O within the existing container. The WAL, Parquet, and watermark files are on a Docker volume (`air-quality-data`) mounted at `/data`.

### 3.3 Docker Volume Isolation

The air-quality-app container mounts `air-quality-data` at `/data` with read-write access. The `ndp-mcp-server` container mounts the same volume at `/data:ro` (read-only). Air-017 does not change this arrangement. The MCP server reads Parquet files that may be up to 30-60 minutes stale (instead of up to 30 seconds stale), which is acceptable for its query use case.

---

## 4. Observability and Monitoring

### 4.1 Metrics to Expose

All metrics should be exposed via the existing `HealthStatus` mechanism (`core/src/subscribers/bronze.rs:304-331`) and through `tracing` structured logging.

| Metric | Type | Description |
|--------|------|-------------|
| `bronze_wal_entries_total` | Counter | Total WAL entries since last commit/truncation |
| `bronze_wal_size_bytes` | Gauge | Current WAL file size in bytes |
| `bronze_wal_sequence` | Counter | Current WAL sequence number (monotonically increasing) |
| `bronze_accumulator_points` | Gauge | Total points across all streams in accumulator |
| `bronze_accumulator_bytes` | Gauge | Estimated memory usage of accumulator |
| `bronze_accumulator_streams` | Gauge | Number of distinct stream_ids in accumulator |
| `bronze_snapshot_duration_seconds` | Histogram | Time taken for last snapshot write |
| `bronze_snapshot_last_success_epoch` | Gauge | Unix timestamp of last successful snapshot |
| `bronze_snapshot_failures_consecutive` | Gauge | Number of consecutive snapshot failures (resets to 0 on success) |
| `bronze_day_rollover_last_epoch` | Gauge | Unix timestamp of last day rollover |
| `bronze_events_received_total` | Counter | Total events received (exists: `events_received` at line 91) |
| `bronze_events_wal_written_total` | Counter | Total events written to WAL |

### 4.2 Logging

Add structured tracing events at these points:

```rust
// On snapshot success
info!(
    duration_ms = elapsed.as_millis(),
    points = snapshot_point_count,
    streams = snapshot_stream_count,
    parquet_bytes = file_size,
    watermark = new_watermark,
    "Bronze snapshot completed"
);

// On snapshot failure
error!(
    error = %e,
    consecutive_failures = self.snapshot_failures,
    accumulator_points = self.accumulator_point_count(),
    wal_size_bytes = self.wal_size(),
    "Bronze snapshot failed"
);

// On day rollover
info!(
    previous_day = %yesterday,
    final_snapshot_points = final_count,
    "Bronze day rollover completed"
);

// On WAL size warning
warn!(
    wal_size_bytes = wal_size,
    wal_cap_bytes = self.config.wal_size_cap,
    "WAL approaching size cap, snapshots may be failing"
);
```

### 4.3 Alerting Conditions

These should surface through Docker health checks via the existing `/health` endpoint:

| Condition | Severity | Detection |
|-----------|----------|-----------|
| WAL size > 80% of cap | Warning | Health check returns degraded |
| Consecutive snapshot failures > 3 | Warning | Health check returns degraded |
| Consecutive snapshot failures > 5 | Critical | Health check returns unhealthy (triggers Docker restart) |
| Accumulator memory > 80% of emergency threshold | Warning | Health check returns degraded |
| Accumulator memory > emergency threshold | Critical | Force snapshot + log error |
| No successful snapshot in > 2x snapshot_interval | Warning | Health check returns degraded |

### 4.4 Health Check Integration

Update the existing `BronzeSubscriber::health_check()` at `core/src/subscribers/bronze.rs:304-331`:

```rust
async fn health_check(&self) -> HealthStatus {
    let healthy = self.is_running
        && self.snapshot_failures_consecutive < 5
        && self.accumulator_memory_estimate() < self.config.emergency_snapshot_bytes;

    let mut details = HashMap::new();
    details.insert("accumulator_points".into(), self.accumulator.total_points().to_string());
    details.insert("accumulator_bytes".into(), self.accumulator_memory_estimate().to_string());
    details.insert("wal_size_bytes".into(), self.wal_size().to_string());
    details.insert("snapshot_failures".into(), self.snapshot_failures_consecutive.to_string());
    details.insert("last_snapshot".into(), self.last_snapshot_time.map_or("never".into(), |t| t.to_rfc3339()));
    // ... existing metrics ...

    HealthStatus { healthy, message, details }
}
```

---

## 5. Migration Strategy

### 5.1 Compatibility

- **Parquet files:** The schema (timestamp, source_id, ndp_id, context, raw_payload) is unchanged. Air-017 changes how files are written (full overwrite from accumulator instead of read-modify-write), not what they contain. Existing Parquet files are fully compatible.
- **WAL format:** The current WAL (`core/src/storage/wal.rs`) stores JSON lines. Air-017 adds a sequence number to each entry (either prepended to the line or as a JSON field). The new WAL format is a superset: old WAL entries without sequence numbers can be assigned sequence 0 on replay. Forward-compatible.
- **Configuration:** New config fields (`snapshot_interval_secs`, `day_rollover_utc_hour`, `wal_size_cap_bytes`, `emergency_snapshot_bytes`) have sensible defaults. Existing `platform.yaml` works unchanged; new fields are optional.

### 5.2 Deployment Sequence

1. Deploy new binary. On startup:
   - Existing WAL (if any) is replayed using the old format (no sequence numbers, assigned seq 0).
   - Existing Parquet files are read to seed the accumulator.
   - New WAL entries get sequence numbers starting from 1.
   - First snapshot overwrites Parquet from accumulator (same data, new write path).
2. No manual data migration steps.
3. No multi-step deployment (old and new binaries do not run concurrently).

### 5.3 Rollback Plan

Revert to the previous Docker image (previous binary). On startup:
- The old binary's `replay_wal()` reads WAL entries. If entries contain sequence numbers as a JSON field, `serde_json::from_slice::<TimeSeriesPoint>()` ignores unknown fields (serde default). If sequence numbers are prepended to lines, the old parser fails on the first line and falls back to an empty replay -- data loss for WAL entries, but Parquet files are intact.
- **To ensure clean rollback:** Include sequence numbers as a JSON field (`"_seq": 42`), not as a line prefix. Serde's default `#[serde(deny_unknown_fields)]` is NOT set on `TimeSeriesPoint` or `RawDataPoint`, so unknown fields are silently ignored. This makes the new WAL format backward-compatible with the old `replay()` implementation.
- Parquet files written by the new binary are identical in schema to those written by the old binary. No rollback issue.

---

## 6. Phase 3 Pre-Analysis: Silver Catch-Up Data-Loss Bug

### 6.1 The Pre-Existing Bug

The `SilverSubscriber` at `core/src/subscribers/silver.rs` has a data-loss path when TimescaleDB is unavailable:

1. **Event processing** (`process_event`, line 362): Calls `self.output.write(&record, &etl_config)` at line 399-401. On failure, returns `Err(SubscriberError::StorageError(...))`. The caller in the `select!` loop (line 560) logs the error and continues: `error!(error = %e, "Error processing event")`. The event is never retried. It is dropped.

2. **High water mark stalls** (`process_event`, lines 407-412): The `high_water_mark` only advances on successful writes. When writes fail, the watermark stays frozen. This is actually correct for catch-up purposes (it records the last successfully persisted timestamp). But since catch-up only runs once...

3. **Catch-up runs once** (`start`, line 536): `self.catch_up().await` runs at the beginning of `start()`. There is no mechanism to re-trigger catch-up during the subscriber's lifetime. If TimescaleDB goes down for 30 minutes and comes back, the events from those 30 minutes are gone from Silver. The watermark is frozen at the pre-outage timestamp.

4. **No retry buffer**: Unlike BronzeSubscriber which has a retry loop (lines 140-168), SilverSubscriber has no retry mechanism for individual write failures.

5. **Recovery requires restart**: The only way to re-process dropped events is a full process restart, which triggers `catch_up()` again at line 536. Docker's `restart: unless-stopped` policy handles crash recovery but not "Silver lost events while running."

### 6.2 How Air-017 Makes This Worse

**Before air-017:** Bronze Parquet is rewritten every 30 seconds. When the process restarts and Silver's `catch_up()` reads from Bronze (`BronzeReader::read_since()`), the Parquet file is at most 30 seconds stale. The gap between the last Parquet write and the crash is at most 30 seconds of data.

**After air-017:** Bronze Parquet is snapshot every 30-60 minutes. The gap widens to 30-60 minutes. On restart, `catch_up()` reads a Parquet file that may be 30-60 minutes behind the WAL. Events in the WAL but not yet in Parquet are invisible to Silver's catch-up.

**Concrete worst case:**
- Snapshot at 14:00. Parquet reflects data up to 14:00.
- TimescaleDB goes down at 14:01. Silver drops all events from 14:01 onward.
- TimescaleDB comes back at 14:20. Silver resumes writing. Events from 14:01-14:20 are lost.
- Process crashes at 14:25. Bronze WAL has events from 14:00-14:25. Bronze Parquet has data up to 14:00 (last snapshot).
- Process restarts. Silver catch-up reads Parquet (up to 14:00). Silver missed 14:00-14:25.
- Net loss: 25 minutes of Silver data. Under the old architecture, the loss would be at most 30 seconds (last Parquet rewrite at 14:24:30).

### 6.3 Options Analysis

#### Option A: Accumulator-backed BronzeReader

**Description:** Modify `BronzeReader::read_since()` to merge data from Parquet files AND the in-memory accumulator. Silver catch-up gets real-time data on restart.

**Pros:**
- Closes the staleness gap entirely. Catch-up reads all data including events received seconds ago.
- Single source of truth for Bronze read path.
- MCP server also benefits (queries return fresh data).

**Cons:**
- Couples Silver to Bronze internals. `BronzeReader` must hold a reference to the accumulator (e.g., via `Arc<RwLock<Accumulator>>`).
- The accumulator is owned by `BronzeSubscriber`. Exposing it requires refactoring `BronzeSubscriber` to share state, or introducing a new `AccumulatorReader` trait.
- Thread safety: The accumulator is mutated by the subscriber (event inserts) and read by the BronzeReader (Silver catch-up, MCP queries). Requires `RwLock` or a copy-on-read snapshot.
- Complexity: Merging Parquet data with accumulator data requires deduplication (events in both).

**Effort:** Medium-high. Requires trait changes, state sharing, dedup logic.

**Risk:** Moderate. RwLock contention could slow down event ingestion if catch-up reads are large.

#### Option B: Silver-side Retry Buffer

**Description:** SilverSubscriber buffers failed writes in a local queue and replays them when TimescaleDB recovers. No Bronze changes needed.

**Pros:**
- Fixes the root cause: Silver drops events on write failure.
- Decoupled from Bronze. No state sharing.
- Simpler to implement: a `VecDeque<(SilverRecord, SilverEtlConfig)>` with periodic drain.

**Cons:**
- Duplicates buffering logic (Bronze has WAL+accumulator, Silver would have its own buffer).
- Buffer is in-memory: lost on process crash. After crash, catch-up from Bronze is still needed, and Bronze Parquet is still stale.
- Memory pressure: if TimescaleDB is down for a long time, the buffer grows. Needs its own size cap.
- Does not help the MCP server (MCP reads from Bronze Parquet, not Silver).

**Effort:** Low-medium. Contained within SilverSubscriber.

**Risk:** Low for the retry buffer itself. Does not address the catch-up staleness gap.

#### Option C: Periodic Re-Catch-Up

**Description:** SilverSubscriber detects sustained write failures (N consecutive failures or M seconds without a successful write) and re-triggers `catch_up()` when writes start succeeding again.

**Pros:**
- Simple: reuse the existing `catch_up()` logic.
- No new data structures or state sharing.
- Handles the "TimescaleDB comes back" scenario without a restart.

**Cons:**
- Still limited by Parquet staleness. If catch-up reads from Parquet and Parquet is 30-60 min stale, the re-catch-up has the same gap.
- Without Option A, re-catch-up can only recover data up to the last Bronze snapshot.
- The `catch_up()` method reads ALL data since the watermark, which could be hours of data. This is a burst of writes to TimescaleDB that could be problematic.
- Need to pause live event processing during re-catch-up, or handle concurrent writes carefully.

**Effort:** Low. Add a failure counter and a re-catch-up trigger in the `select!` loop.

**Risk:** Low for the mechanism. Medium for effectiveness (gap still exists without Option A).

#### Option D: Accept the Gap

**Description:** Document the 30-60 minute staleness gap as a known limitation. Rely on Docker restart policy for crash recovery. The catch-up on restart covers most scenarios.

**Pros:**
- Zero implementation effort.
- The gap only matters when BOTH (a) TimescaleDB is down AND (b) the process doesn't restart. On Pi, Docker's `restart: unless-stopped` means the process restarts on crash. Deliberate TimescaleDB downtime (maintenance) is rare and short.
- The pre-existing bug (Silver drops events on write failure) exists with or without air-017. Air-017 makes the catch-up gap wider but does not introduce the drop behavior.

**Cons:**
- Does not fix the root cause.
- The 30-60 minute catch-up gap is real and visible: Silver data will have a hole equal to the snapshot interval after any process restart.
- As data volume grows, the gap becomes more significant (more data lost per hour of staleness).

**Effort:** None.

**Risk:** Low for current deployment. Medium for future growth.

### 6.4 Recommendation

**Phase 3 should implement Option B (Silver-side retry buffer) combined with Option C (periodic re-catch-up).** Rationale:

1. **Option B fixes the root cause** -- Silver should not silently drop events on write failure. A bounded in-memory retry buffer with configurable size cap (default: 10,000 records, ~5 MiB) handles transient TimescaleDB outages up to ~30 minutes at current volume.

2. **Option C provides defense in depth** -- if the retry buffer fills up (long outage), re-catch-up after recovery reads whatever is in Bronze Parquet. Combined with B, this covers: (a) short outages fully (buffer replays), (b) long outages partially (buffer fills -> some drops -> re-catch-up recovers from Parquet).

3. **Option A should be deferred to a separate feature.** It provides the most complete solution but requires significant refactoring of BronzeSubscriber state ownership. The benefit (closing the 30-60 min gap for catch-up) is only relevant when both the retry buffer is exhausted AND the process restarts -- a narrow failure mode.

4. **Option D is not acceptable as the final state** but is acceptable for Phase 1 and Phase 2 of air-017, where the focus is on the Bronze write path.

**Phase 3 gate criteria:** Option B + C must be implemented and tested before air-017 Phase 3 is considered complete. Option A can be a separate feature (e.g., `dp-NNN: Accumulator-Backed Bronze Read Path`).

---

## 7. Iterative Improvement Roadmap

### 7.1 Phase 1: WAL on Receipt + Accumulator + Periodic Snapshot

**Scope:** Move WAL writes from ParquetStore to BronzeSubscriber. Add in-memory accumulator. Replace the flush action from "call store.write_raw_batch()" to "write full snapshot from accumulator."

**Deliverables:**
1. New WAL format: JSON lines with sequence numbers as a field (`"_seq": N`).
2. In-memory accumulator: `HashMap<String, Vec<RawDataPoint>>` keyed by stream_id.
3. Snapshot writer: Builds Parquet from accumulator, writes via temp file + atomic rename.
4. Watermark file: Persists last-snapshot sequence number.
5. Startup recovery: Read Parquet + replay WAL after watermark.
6. Config: `snapshot_interval_secs` (default 1800), `wal_size_cap_bytes` (default 50 MiB), `emergency_snapshot_bytes` (default 100 MiB).
7. Metrics: All items from Section 4.1.

**Files changed:**
- `core/src/storage/wal.rs` -- Add sequence number to entries, watermark-aware replay.
- `core/src/subscribers/bronze.rs` -- Add accumulator, snapshot timer, WAL-on-receipt, health metrics.
- `core/src/storage/parquet.rs` -- Add `write_snapshot()` method that writes from owned data (no read-modify-write). Keep `append_to_raw_parquet()` for backward compatibility until Phase 2 removes it.
- `config/base/platform.yaml` -- Add `snapshot_interval_secs` to bronze subscriber config.

**Phase 1 exit criteria (must be true before moving to Phase 2):**
- All existing BronzeSubscriber tests pass (backward compatibility).
- New unit tests for: accumulator insert, accumulator memory estimation, WAL with sequence numbers, snapshot write (atomic rename), startup recovery (Parquet + WAL replay), emergency snapshot trigger.
- Integration test: ingest 1000 events, verify Parquet file contains all events after snapshot.
- Integration test: kill process mid-operation, restart, verify no data loss.
- Memory profiling: confirm peak RSS under 200 MiB with 1 day of 4-stream data.
- SD card write volume measured and compared to baseline (target: >10x reduction).

### 7.2 Phase 2: Day Rollover + WAL Watermark Truncation

**Scope:** Add midnight rollover timer. Evolve WAL from watermark-only to watermark + periodic compaction (rewrite WAL without entries before watermark).

**Deliverables:**
1. Day rollover timer: Computes next midnight UTC, fires forced snapshot + accumulator reset.
2. WAL compaction: After snapshot, rewrite WAL file containing only entries after watermark (via temp file + rename).
3. Yesterday's Parquet finalization: On rollover, yesterday's file becomes immutable.
4. Config: `day_rollover_utc_hour` (default 0).
5. Timezone/NTP resilience: Recompute rollover timer using wall clock.

**Files changed:**
- `core/src/subscribers/bronze.rs` -- Add rollover timer branch to `select!` loop, rollover logic, WAL compaction.
- `core/src/storage/wal.rs` -- Add `compact(watermark)` method (rewrite entries after watermark to new file).
- `config/base/platform.yaml` -- Add `day_rollover_utc_hour`.

**Phase 2 exit criteria:**
- All Phase 1 tests still pass.
- New tests for: rollover at midnight, rollover timer recomputation after NTP jump, WAL compaction correctness, concurrent rollover+snapshot handling.
- Integration test: simulate multi-day operation, verify Parquet files per day, verify WAL compaction.
- WAL file size stays bounded (never exceeds 2x snapshot interval worth of data).

### 7.3 Phase 3: Read Path Integration + Silver Resilience

**Scope:** Implement Silver-side retry buffer (Option B) and periodic re-catch-up (Option C). Defer accumulator-backed BronzeReader (Option A) to a separate feature.

**Deliverables:**
1. Silver retry buffer: `VecDeque<(SilverRecord, SilverEtlConfig)>` with configurable max size.
2. Retry drain: On successful write, drain buffer entries before processing new events.
3. Re-catch-up trigger: After N consecutive write failures resolved, re-run `catch_up()`.
4. Catch-up staleness documentation: Explicitly document the 30-60 min gap in Silver catch-up.

**Files changed:**
- `core/src/subscribers/silver.rs` -- Add retry buffer, failure counter, re-catch-up trigger.
- `config/base/platform.yaml` -- Add `silver.retry_buffer_size` (default 10000), `silver.re_catchup_after_failures` (default 10).

**Phase 3 exit criteria:**
- All Phase 1 and Phase 2 tests still pass.
- New tests for: Silver retry buffer fill and drain, re-catch-up after TimescaleDB recovery, buffer overflow behavior (oldest events dropped).
- Integration test: stop TimescaleDB for 5 minutes, restart, verify Silver data loss is limited to buffer overflow (if any).
- End-to-end test: ingest -> Bronze snapshot -> Silver catch-up -> verify data integrity across restart.

---

## Appendix A: File Reference

| File | Lines | Relevance to air-017 |
|------|-------|---------------------|
| `core/src/storage/wal.rs` | 1-65 | Current WAL implementation. Needs sequence numbers, watermark, compaction. |
| `core/src/subscribers/bronze.rs` | 1-795 | BronzeSubscriber. Primary refactoring target. Needs accumulator, snapshot timer, rollover timer. |
| `core/src/storage/parquet.rs:157-225` | `append_to_parquet()` | Read-modify-write for parsed data. Will be replaced by direct write from accumulator. |
| `core/src/storage/parquet.rs:563-622` | `append_to_raw_parquet()` | Read-modify-write for raw data. Will be replaced by direct write from accumulator. |
| `core/src/storage/parquet.rs:710-741` | `write_raw_batch()` | Current WAL-then-Parquet flow. WAL append moves to BronzeSubscriber. |
| `core/src/subscribers/silver.rs:247-325` | `catch_up()` | Silver catch-up from Bronze. Runs once on startup. Phase 3 adds re-catch-up. |
| `core/src/subscribers/silver.rs:362-415` | `process_event()` | Silver event processing. Drops events on write failure. Phase 3 adds retry buffer. |
| `core/src/subscribers/silver.rs:536` | `start()` | Where `catch_up()` is called once. Phase 3 adds re-trigger mechanism. |
| `core/src/subscribers/mod.rs:90-103` | `BronzeReader` trait | Read interface for Silver catch-up. Phase 3 (Option A, deferred) would extend this. |
| `config/base/platform.yaml:25-31` | Bronze subscriber config | Needs new fields: snapshot_interval_secs, day_rollover_utc_hour, wal_size_cap_bytes. |
| `deploy/pi/docker-compose.yml:117` | Memory limit | air-quality-app container: 512M. Target peak RSS: <200 MiB. |

## Appendix B: Configuration Defaults

```yaml
subscribers:
  bronze:
    enabled: true
    batch_size: 100                   # Events buffered before WAL batch write (kept for backpressure)
    flush_interval_secs: 30           # WAL batch flush interval (existing, now just WAL+accumulator)
    snapshot_interval_secs: 1800      # Parquet snapshot from accumulator (new, 30 min)
    day_rollover_utc_hour: 0          # Hour to finalize daily Parquet file (new, midnight UTC)
    max_retries: 3                    # Retry count for snapshot writes (existing)
    wal_size_cap_bytes: 52428800      # 50 MiB WAL size cap (new)
    emergency_snapshot_bytes: 104857600  # 100 MiB accumulator triggers forced snapshot (new)
    emergency_snapshot_points: 500000    # 500K point hard limit (new)
```
