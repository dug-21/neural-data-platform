# AIR-017: Bronze Write-Ahead Architecture (Eliminate Read-Modify-Write)

> **Feature ID:** air-017
> **Created:** 2026-02-07
> **Status:** Scoping
> **Phase:** air (Foundation / Core)
> **Depends on:** air-016 Phase 1 (buys time, not a prerequisite)

---

## Problem Statement

The Bronze layer treats Parquet as a hot write target. Every flush cycle (currently every 30 seconds after air-016 config change) reads the entire daily Parquet file into memory, deserializes every row, appends new points, and rewrites the entire file. This is O(file_size) per flush and fundamentally wrong for a columnar format designed for batch analytics.

No mature data platform appends to existing Parquet files. InfluxDB, TimescaleDB, ClickHouse, Delta Lake, and Apache Iceberg all separate the hot write path (WAL / memtable) from the cold storage path (immutable columnar files). NDP's Bronze layer conflates both.

### Current Architecture (Broken)

```
Event arrives → EventBus
  → BronzeSubscriber buffer (in-memory, no durability)
  → flush timer fires (every 30s)
  → store.write_raw_batch()
      → WAL append (first durability point — up to 30s after event)
      → read entire daily Parquet file
      → deserialize every row into Vec<RawDataPoint>
      → append new batch
      → rewrite entire file
      → WAL commit (delete)
```

Problems:
1. **WAL is too late** — data sits in BronzeSubscriber buffer with no durability for up to 30 seconds
2. **Read-modify-write scales as O(file_size)** — memory cost grows linearly through the day
3. **Parquet rewrite frequency is excessive** — even at 30s intervals, ~2,880 full-file rewrites per day
4. **WAL and Parquet are coupled** — WAL commit deletes everything, can't represent "written up to here"

### Files Affected

| File | Lines | Role |
|------|-------|------|
| `core/src/subscribers/bronze.rs` | 1-795 | BronzeSubscriber — event buffering, flush logic |
| `core/src/storage/parquet.rs` | 157-225 | `append_to_parquet()` — read-modify-write for parsed |
| `core/src/storage/parquet.rs` | 563-622 | `append_to_raw_parquet()` — read-modify-write for raw |
| `core/src/storage/wal.rs` | 1-65 | WriteAheadLog — append-only file, commit = delete |
| `config/base/platform.yaml` | 25-31 | Bronze subscriber config |

---

## Desired Outcome

Separate the durability concern (WAL) from the archival concern (Parquet). WAL provides immediate durability on event receipt. Parquet is a periodic snapshot written from an in-memory accumulator. Read-modify-write is eliminated entirely.

### Target Architecture

```
Event arrives → EventBus
  → BronzeSubscriber
      → WAL append immediately (durability within milliseconds)
      → In-memory accumulator (today's points, queryable)

Snapshot timer fires (every 30-60 min, configurable)
  → Write full day's data from accumulator → readings.parquet (overwrite, not append)
  → Advance WAL watermark (truncate only committed entries)

Day rollover (midnight UTC, configurable)
  → Final snapshot of yesterday's data (file is now immutable, never touched again)
  → Clear WAL entries for yesterday
  → Start fresh accumulator for today

Startup recovery
  → Read today's Parquet file (if exists) → seed accumulator
  → Replay WAL entries after last snapshot → complete the accumulator
  → Resume normal operation
```

### Target Properties

| Property | Before | After |
|----------|--------|-------|
| Durability latency | Up to 30s (buffer → WAL on flush) | Milliseconds (WAL on event receipt) |
| Parquet write frequency | ~2,880/day (every 30s) | ~24-48/day (every 30-60 min) |
| Parquet write cost | O(file_size) read + O(file_size) write | O(day's data) write only (from memory, no read) |
| Memory model | Transient spikes from read-modify-write | Stable O(day's data) in accumulator |
| Files per day | 1 Parquet file | 1 Parquet file (unchanged) |
| Crash recovery | Replay WAL → write to Parquet | Read Parquet + replay WAL → rebuild accumulator |

### Memory Budget (Estimated)

| Component | Size |
|-----------|------|
| In-memory accumulator (4 streams x 11K points x ~500 bytes) | ~22 MiB |
| Parquet snapshot write (column Vecs from accumulator) | ~22 MiB transient |
| Peak during snapshot | ~44 MiB (accumulator + column Vecs) |
| Baseline (runtime, tokio, MQTT, EventBus, HTTP) | ~80-100 MiB |
| **Total peak RSS** | **~130-150 MiB** |

---

## Approach

### Phase 1: WAL on receipt + accumulator + periodic snapshot

Move WAL writes from ParquetStore to BronzeSubscriber. Add an in-memory accumulator that persists across flushes. Replace the flush action from "call store.write_raw_batch()" to "write full snapshot from accumulator."

### Phase 2: Day rollover + WAL watermarking

Add a midnight timer that finalizes yesterday's Parquet file and starts a fresh accumulator. Evolve WAL from "commit = delete all" to "commit up to watermark" so WAL entries for today survive yesterday's commit.

### Phase 3: Read path integration + Silver resilience

Expose the in-memory accumulator to the read path (Silver catch-up, MCP server) so queries can access data that hasn't been snapshot to Parquet yet.

More critically, this phase must address a **pre-existing Silver data-loss bug** that air-017 makes worse. Today, SilverSubscriber has no retry or re-queue mechanism. When TimescaleDB is down:

1. Silver continues receiving events from EventBus
2. Every `output.write()` call fails → error logged → **event dropped forever**
3. `high_water_mark` stays frozen at last successful write
4. `catch_up()` only runs once at startup (`silver.rs:536`), never re-triggers
5. **Recovery requires a full process restart** to re-enter `start()` → `catch_up()`

Under air-017, the catch-up path reads from Bronze Parquet, which is now 30-60 minutes stale. So even a restart leaves a gap equal to the snapshot interval.

**Options (to be decided during SPARC Architecture phase):**

- **A. Accumulator-backed BronzeReader**: `read_since()` merges Parquet + accumulator data. Closes the staleness gap but couples Silver to Bronze internals.
- **B. Silver-side retry buffer**: Silver buffers failed writes and replays them when TimescaleDB recovers. Simpler, doesn't require Bronze changes, but duplicates buffering logic.
- **C. Periodic re-catch-up**: Silver detects sustained write failures and re-triggers `catch_up()` after recovery. Lightweight but still limited by Parquet staleness unless combined with (A).
- **D. Accept the gap**: Snapshot interval is 30-60 min. On Pi, TimescaleDB downtime triggers a process restart anyway (Docker restart policy). Catch-up on restart covers most of the gap. Document as known limitation.

The pre-existing bug (Silver drops events on write failure with no recovery path other than restart) exists today regardless of air-017. Fixing it properly is valuable but may warrant its own feature scope.

---

## Scheduling

The app has no cron or external scheduler. All timers use `tokio::time::interval`. The BronzeSubscriber already has a configurable `flush_interval_secs` timer in its `select!` loop (`bronze.rs:219`). The snapshot timer and day-rollover timer will use the same pattern.

### Snapshot timer
New config field: `snapshot_interval_secs` (default: 1800 = 30 min). Uses `tokio::time::interval` in the BronzeSubscriber `select!` loop alongside the existing flush timer.

### Day rollover timer
Compute time until next midnight UTC. Use `tokio::time::sleep_until` for the first trigger, then `tokio::time::interval(Duration::from_secs(86400))` for subsequent days. Runs as a branch in the same `select!` loop.

### Configurable via platform.yaml

```yaml
subscribers:
  bronze:
    enabled: true
    batch_size: 100               # events before WAL flush (kept for backpressure)
    flush_interval_secs: 30       # WAL batch interval (fast, just WAL + accumulator)
    snapshot_interval_secs: 1800  # Parquet snapshot interval (slow, full write)
    day_rollover_utc_hour: 0      # Hour to finalize daily file (0 = midnight UTC)
    max_retries: 3
```

---

## Constraints

- One Parquet file per day per stream per data type (parsed + raw) — unchanged
- Parquet file naming (`readings.parquet`, `data.parquet`) — unchanged
- Parquet schema — unchanged
- Silver ETL real-time path (EventBus subscriber) — unchanged (receives events directly, not from Bronze)
- Silver catch-up path (`BronzeReader.read_since()`) — reads from Parquet today, must read from accumulator+Parquet in Phase 3
- Store trait interface (`write`, `write_batch`, `query`, `query_raw`) — may need to merge in-memory data for reads (Phase 3)
- WAL must survive process crashes and be replayable on startup
- No new runtime dependencies beyond what air-016 Phase 1 adds
- Docker memory limit 512 MiB — must stay well under with full day's accumulator

---

## Out of Scope

- Sidecar files / multi-file-per-day approaches (explicitly rejected by this design)
- Parquet row group append / footer rewrite (complexity and corruption risk on Pi)
- Silver retry/re-queue mechanism (pre-existing bug, may warrant own feature scope)
- Silver ETL transform logic changes (read-path integration in Phase 3 is in scope)
- Polars removal (may happen naturally since write path becomes simpler)
- Compaction (not needed — single file per day, overwritten periodically)
- MQTT unbounded cache fix (separate issue)
- EventBus capacity tuning (separate issue)

---

## WAL Evolution Required

The current WAL (`core/src/storage/wal.rs`) is minimal:
- `append()` — write JSON line to file, flush
- `replay()` — read all lines
- `commit()` — delete file, recreate empty

This needs to evolve for air-017:
- **Watermark-based commit**: Instead of deleting all entries, truncate up to a sequence number. Entries after the watermark (received since last Parquet snapshot) survive.
- **Per-stream WAL or sequence tagging**: The current single WAL file mixes all streams. For efficient replay, entries need a stream identifier (already present in the data as `source_id`).
- **Startup replay with dedup**: On startup, read Parquet (data up to last snapshot) + replay WAL (data after last snapshot). Need to avoid duplicating points that were both WAL'd and snapshot'd. A monotonic sequence number or timestamp watermark handles this.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| In-memory accumulator uses too much memory | Low | Medium | 22 MiB for current volumes; monitor; add eviction if needed |
| Day rollover timer drift | Low | Low | Recompute next midnight on each tick; use wall clock not interval |
| WAL grows unbounded if snapshots fail | Medium | Medium | Cap WAL size; alert on snapshot failure; retry logic |
| Read path returns stale data (Phase 3 deferred) | Medium | Low | Silver catch-up is rare; MCP can tolerate minutes of staleness |
| TimescaleDB downtime + Silver data loss | Medium | Medium | Pre-existing bug: Silver drops events on write failure. air-017 makes catch-up gap wider (snapshot interval vs near-real-time). Docker restart policy is current mitigation. Phase 3 or separate feature needed for proper fix. |
| Silver catch-up reads stale Parquet after restart | High (given air-017) | Medium | Parquet is up to 1 snapshot interval behind. Phase 3 option A (accumulator-backed BronzeReader) or forced snapshot before process exit mitigates. |
| Process crash loses in-memory accumulator | N/A | None | WAL replay rebuilds it; this is the design |
| Power loss corrupts WAL mid-write | Low | Low | WAL uses line-delimited JSON + flush(); partial last line is skipped on replay |
