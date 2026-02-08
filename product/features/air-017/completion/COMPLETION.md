# AIR-017: Bronze Write-Ahead Architecture -- Completion Plan

> Work breakdown structure for implementation swarm.
> Each item includes: description, affected files, size estimate (S/M/L), and dependency chain.
> Reference: `docs/procedures/RELEASE-POLICY.md` for release artifacts.

---

## Phase 1: WAL on Receipt + Accumulator + Periodic Snapshot

**Goal**: Separate durability (WAL) from archival (Parquet). WAL provides millisecond durability on event receipt. Parquet is a periodic snapshot from an in-memory accumulator. Read-modify-write is eliminated.

### P1-01: Evolve WriteAheadLog struct -- sequence numbers and watermark

**Files**: `core/src/storage/wal.rs`
**Size**: M
**Depends on**: nothing

Add to the existing `WriteAheadLog` (65 lines today):

- `WalEntry` struct wrapping each line: `{ seq: u64, source_id: String, data: RawDataPoint }`
- Monotonic `next_seq: u64` counter, persisted as the first field of each JSON line
- `watermark: u64` field tracking the last sequence number included in a Parquet snapshot
- `append()` returns the assigned sequence number
- `replay_after(seq: u64)` returns only entries with `seq > watermark`
- `commit_up_to(seq: u64)` rewrites the WAL file retaining only entries with `seq > watermark` (replaces today's `commit()` which deletes everything)
- Backward compatibility: if a WAL file has no `seq` field (pre-air-017), treat every entry as `seq = 0` so `replay()` returns all of them (graceful migration)

**Acceptance criteria**:
- `append()` assigns sequential numbers starting from 1 (or continuation after restart via `replay()`)
- `replay_after(0)` returns all entries (equivalent to today's `replay()`)
- `commit_up_to(N)` retains only entries with `seq > N`
- Old-format WAL files (no `seq` field) are replayed correctly
- Existing tests in `wal.rs` continue to pass (backward compat)
- New unit tests for sequence numbering, watermark commit, and mixed-format replay

---

### P1-02: Create InMemoryAccumulator

**Files**: `core/src/subscribers/bronze.rs` (new submodule or section)
**Size**: M
**Depends on**: nothing (independent of P1-01)

New struct that holds all of today's data points in memory, grouped by source_id:

```
pub struct InMemoryAccumulator {
    /// Points grouped by source_id for efficient partition writes
    points: HashMap<String, Vec<RawDataPoint>>,
    /// Total point count across all sources
    count: usize,
    /// Highest WAL sequence number reflected in this accumulator
    watermark: u64,
}
```

Methods:
- `new() -> Self`
- `add(&mut self, point: RawDataPoint, seq: u64)` -- inserts point, updates watermark
- `seed_from_parquet(&mut self, points: Vec<RawDataPoint>)` -- populate from existing Parquet on startup (watermark stays at 0 since these are pre-WAL)
- `points_by_source(&self) -> &HashMap<String, Vec<RawDataPoint>>` -- read access for snapshot writer
- `total_count(&self) -> usize`
- `watermark(&self) -> u64`
- `clear(&mut self)` -- reset for day rollover (Phase 2)
- `estimated_memory_bytes(&self) -> usize` -- rough memory estimate for monitoring

**Acceptance criteria**:
- Adding N points results in `total_count() == N`
- Points are grouped correctly by source_id
- Watermark advances monotonically
- `seed_from_parquet` populates without changing watermark
- Memory estimate is within 2x of actual allocation (rough is acceptable)

---

### P1-03: Move WAL append to BronzeSubscriber event receipt

**Files**: `core/src/subscribers/bronze.rs`, `core/src/storage/parquet.rs`
**Size**: L
**Depends on**: P1-01, P1-02

This is the core behavioral change. Today:
1. Event arrives -> buffered in `BronzeSubscriber.buffer` (no durability)
2. Flush timer fires -> `store.write_raw_batch()` -> WAL append -> Parquet read-modify-write -> WAL commit

After this change:
1. Event arrives -> WAL append immediately (durability in ms) -> add to accumulator
2. Flush timer is now the **batch WAL flush** interval (keeps existing `flush_interval_secs` semantics for backpressure)

Changes to `BronzeSubscriber`:
- Add `wal: WriteAheadLog` field (owned, not via store)
- Add `accumulator: InMemoryAccumulator` field
- In `handle_point()`: call `wal.append()` then `accumulator.add()`
- Remove `buffer: Vec<RawDataPoint>` field (replaced by accumulator)
- Remove or repurpose `flush()` -- the current flush calls `store.write_raw_batch()` which does WAL+Parquet. The new flush is just a no-op or a periodic WAL fsync if batching WAL writes

Changes to `BronzeSubscriber::new()`:
- Accept WAL path parameter (derived from store's data directory)
- Construct `WriteAheadLog` and `InMemoryAccumulator`

Changes to `ParquetStore`:
- `write_raw_batch()` no longer owns the WAL. Remove WAL append and WAL commit from this method
- `write_raw_batch()` may become unused in Phase 1 (BronzeSubscriber calls snapshot instead)

**Acceptance criteria**:
- WAL entry written within the `handle_point` call (not deferred to flush)
- Accumulator contains all received points after processing
- `BronzeSubscriber.buffer` field removed
- No read-modify-write occurs on event receipt
- All existing BronzeSubscriber tests updated to reflect new behavior
- BronzeSubscriber metric counters (`events_received`, `events_written`) still accurate

---

### P1-04: Add snapshot timer to select! loop

**Files**: `core/src/subscribers/bronze.rs`, `config/base/platform.yaml`
**Size**: M
**Depends on**: P1-03

Add a new `tokio::time::interval` branch to the existing `select!` loop in `BronzeSubscriber::start()`:

```rust
let snapshot_interval = Duration::from_secs(self.config.snapshot_interval_secs);
let mut snapshot_timer = tokio::time::interval(snapshot_interval);
snapshot_timer.tick().await; // skip immediate first tick

// In the select! loop:
_ = snapshot_timer.tick() => {
    if let Err(e) = self.snapshot().await {
        error!(..., "Snapshot failed");
    }
}
```

Add `snapshot_interval_secs` to `BronzeSubscriberConfig`:
```rust
#[serde(default = "default_snapshot_interval_secs")]
pub snapshot_interval_secs: u64,  // default: 1800 (30 min)
```

Update `config/base/platform.yaml` bronze section:
```yaml
subscribers:
  bronze:
    enabled: true
    batch_size: 100
    flush_interval_secs: 30
    snapshot_interval_secs: 1800
    max_retries: 3
```

**Acceptance criteria**:
- Snapshot timer fires independently of flush timer
- Config defaults to 1800 seconds if not specified (backward compatible)
- `platform.yaml` includes `snapshot_interval_secs` field with comment
- Timer can be verified in integration test by setting interval to 1 second

---

### P1-05: Implement snapshot_to_parquet()

**Files**: `core/src/subscribers/bronze.rs`, `core/src/storage/parquet.rs`
**Size**: M
**Depends on**: P1-02, P1-03

New method on `BronzeSubscriber`:

```rust
async fn snapshot(&mut self) -> Result<(), SubscriberError> {
    // For each source in the accumulator:
    //   1. Build Vec<RawDataPoint> from accumulator
    //   2. Compute partition path (same as today's raw_partition_path)
    //   3. Write full Parquet file (overwrite, not append)
    //   4. Advance WAL watermark to accumulator's watermark
    //   5. Commit WAL up to watermark (truncate old entries)
}
```

The Parquet write uses an existing or new method that writes `Vec<RawDataPoint>` directly to a file path **without reading the existing file first**. This is the key difference from `append_to_raw_parquet()` which reads-then-appends.

Options for the write method:
- **Preferred**: Use `ParquetStore::write_raw_parquet()` (already exists, writes from Vec without reading). The BronzeSubscriber needs access to the store's `raw_partition_path()` and `write_raw_parquet()`.
- **Alternative**: Extract the partition path logic and Parquet write logic into standalone functions callable from BronzeSubscriber.

The snapshot writes the **full day's data** from the accumulator. Since the accumulator holds everything, the written file is a complete replacement, not an append.

**Acceptance criteria**:
- Snapshot produces a valid Parquet file identical in schema to today's files
- Snapshot does NOT read the existing Parquet file (no `File::open` for reading)
- WAL watermark advances after successful snapshot
- WAL entries before watermark are truncated
- Snapshot of empty accumulator is a no-op (no empty Parquet files created)
- Written file is byte-for-byte queryable by `query_raw()` (schema compatibility)

---

### P1-06: Implement startup recovery

**Files**: `core/src/subscribers/bronze.rs`
**Size**: L
**Depends on**: P1-01, P1-02, P1-05

On startup, before entering the `select!` loop, BronzeSubscriber must rebuild its accumulator:

```
1. Find today's Parquet file for each known stream (if exists)
2. Read Parquet -> seed accumulator (this is the data up to last snapshot)
3. Replay WAL entries after watermark -> add to accumulator (these are post-snapshot)
4. Resume normal operation with full day's data in memory
```

Step 1 requires knowing which streams exist. Options:
- **A**: Scan the Bronze data directory for today's date directories
- **B**: Use the stream configs to enumerate known streams

Step 2 uses `InMemoryAccumulator::seed_from_parquet()` from P1-02.

Step 3 uses `WriteAheadLog::replay_after(watermark)` from P1-01. The watermark is stored in the WAL or derived from the Parquet file's content (latest timestamp or explicit metadata).

Dedup: If a point was both WAL'd and snapshot'd (crash between WAL append and snapshot), it appears in both Parquet and WAL replay. Dedup by `(source_id, timestamp)` pair or by WAL sequence number. The preferred approach is sequence-number-based: Parquet snapshot records the watermark, WAL replay only returns entries after that watermark, so no overlap exists by construction.

Recovery edge cases:
- No Parquet file exists (first run of the day): accumulator starts empty, all WAL entries are replayed
- Parquet exists but WAL is empty (clean shutdown after snapshot): accumulator seeded from Parquet only
- Parquet exists and WAL has entries (crash after receiving events but before snapshot): seed from Parquet, replay WAL
- No Parquet and no WAL (fresh start): empty accumulator, normal operation

**Acceptance criteria**:
- After recovery, accumulator contains the union of Parquet + post-watermark WAL entries
- No duplicate points in the accumulator after recovery
- Recovery handles all four edge cases above
- Recovery completes before the subscriber starts processing new events
- Integration test: write events -> snapshot -> write more events -> simulate crash -> recover -> verify all events present

---

### P1-07: Update BronzeSubscriberConfig

**Files**: `core/src/subscribers/bronze.rs`, `config/base/platform.yaml`
**Size**: S
**Depends on**: nothing (can be done early, but values consumed by P1-04, P1-08)

Add new fields to `BronzeSubscriberConfig`:

```rust
/// Parquet snapshot interval in seconds (default: 1800 = 30 min)
#[serde(default = "default_snapshot_interval_secs")]
pub snapshot_interval_secs: u64,

/// UTC hour for day rollover (default: 0 = midnight UTC)
/// Phase 2 uses this; included now for config compatibility
#[serde(default)]
pub day_rollover_utc_hour: u8,
```

Add defaults:
```rust
fn default_snapshot_interval_secs() -> u64 { 1800 }
```

Ensure `BronzeSubscriberConfig::default()` includes the new fields with sensible defaults so that existing configurations without these fields continue to work (serde `default` attribute handles this).

**Acceptance criteria**:
- Old `platform.yaml` files without `snapshot_interval_secs` parse correctly (defaults to 1800)
- New `platform.yaml` includes the field with a comment
- `day_rollover_utc_hour` defaults to 0 and is accepted but unused until Phase 2
- Config deserialization tests pass with and without the new fields

---

### P1-08: Remove or simplify read-modify-write path

**Files**: `core/src/storage/parquet.rs`
**Size**: M
**Depends on**: P1-03, P1-05

With BronzeSubscriber owning the WAL and using snapshot writes, the following methods in `ParquetStore` need review:

- `append_to_raw_parquet()` (lines 562-622): The read-modify-write method. After air-017 Phase 1, BronzeSubscriber calls `write_raw_parquet()` directly (overwrite from accumulator). **However**, `append_to_raw_parquet()` is also called by `write_raw()` (single-point write, line 700-705). If `write_raw()` is still used anywhere, the method must remain.

Check callers of:
- `write_raw()` -- used by single-point ingestion paths (if any remain)
- `append_to_parquet()` (lines 157-225) -- used by parsed data path (TimeSeriesPoint)
- `write_raw_batch()` -- called by BronzeSubscriber flush (will be replaced)

Action plan:
1. If `write_raw()` has no callers outside tests: mark `append_to_raw_parquet()` as `#[deprecated]` with a note pointing to the snapshot path
2. If `write_raw()` still has callers: leave it for now, address in Phase 3 or separate cleanup
3. Remove WAL logic from `write_raw_batch()` (WAL is now owned by BronzeSubscriber)
4. `write_raw_batch()` becomes a simple "group by partition + write each" without WAL

The parsed data path (`append_to_parquet` for `TimeSeriesPoint`) is **out of scope** for air-017. It still uses read-modify-write. This is acceptable because the parsed path is less frequently used and may be eliminated entirely when Polars is removed.

**Acceptance criteria**:
- `write_raw_batch()` no longer touches the WAL
- All callers of the modified methods compile and pass tests
- No behavioral change for the parsed data path (`append_to_parquet`)
- If deprecated, method has `#[deprecated(since = "air-017", note = "...")]` annotation

---

### P1-09: Unit tests -- WAL v2, accumulator, snapshot, recovery

**Files**: `core/src/storage/wal.rs` (test module), `core/src/subscribers/bronze.rs` (test module)
**Size**: L
**Depends on**: P1-01 through P1-08

WAL tests (extend existing `mod tests` in `wal.rs`):
- `test_wal_sequence_numbers` -- verify monotonic assignment
- `test_wal_replay_after_watermark` -- only returns entries after given seq
- `test_wal_commit_up_to` -- retains entries after watermark, removes before
- `test_wal_backward_compat_no_seq_field` -- old-format entries replay as seq=0
- `test_wal_persistence_with_sequences` -- survive process restart, sequences resume

Accumulator tests (new section in `bronze.rs` tests or dedicated test module):
- `test_accumulator_add_and_count` -- basic insertion
- `test_accumulator_grouping_by_source` -- points routed to correct source buckets
- `test_accumulator_watermark_advances` -- watermark tracks highest seq
- `test_accumulator_seed_from_parquet` -- populate from Vec without changing watermark
- `test_accumulator_clear` -- reset for day rollover
- `test_accumulator_memory_estimate` -- rough correctness

BronzeSubscriber tests (update existing mocks and tests):
- `test_bronze_wal_on_receipt` -- WAL entry written in handle_point, not deferred
- `test_bronze_snapshot_writes_full_day` -- snapshot writes all accumulated points
- `test_bronze_snapshot_no_read` -- verify no file read during snapshot (mock assertion)
- `test_bronze_recovery_parquet_plus_wal` -- seed + replay produces correct state
- `test_bronze_recovery_no_parquet` -- WAL-only recovery
- `test_bronze_recovery_no_wal` -- Parquet-only recovery
- `test_bronze_recovery_dedup` -- no duplicates after recovery

**Acceptance criteria**:
- All tests pass with `cargo test -p neural-core`
- No `todo!()`, `unimplemented!()`, or `#[ignore]` annotations
- Mock-based tests use `MockRawStore` (already exists in test module)
- Test coverage for all recovery edge cases from P1-06

---

### P1-10: Integration tests -- full cycle

**Files**: new test file or extend existing integration tests
**Size**: L
**Depends on**: P1-01 through P1-09

End-to-end tests using a real `ParquetStore` (temp directory, no mocks):

- `test_ingest_wal_accumulate_snapshot_cycle`
  1. Create BronzeSubscriber with real WAL and ParquetStore (temp dir)
  2. Send N events via broadcast channel
  3. Wait for WAL entries (verify count)
  4. Trigger snapshot (set snapshot_interval to 1s or call snapshot() directly)
  5. Verify Parquet file exists with N rows
  6. Verify WAL is truncated (entries committed up to watermark)

- `test_crash_recovery_cycle`
  1. Send N events, snapshot, send M more events
  2. Drop BronzeSubscriber (simulate crash)
  3. Create new BronzeSubscriber with same data directory
  4. Verify accumulator has N + M points after recovery
  5. Trigger another snapshot, verify Parquet has N + M rows

- `test_memory_stays_within_budget`
  1. Send a realistic day's volume of events (e.g., 11K points x 4 streams)
  2. Check `InMemoryAccumulator::estimated_memory_bytes()` is under 50 MiB (leaving headroom)
  3. Trigger snapshot, verify memory does not spike above 100 MiB transient

- `test_concurrent_receive_and_snapshot`
  1. Start BronzeSubscriber with short snapshot interval
  2. Continuously send events
  3. Verify no data loss or corruption after multiple snapshot cycles
  4. Verify final Parquet file count matches expected

**Acceptance criteria**:
- All integration tests pass
- Tests use temp directories (cleaned up after)
- Tests complete within 30 seconds each (no real-time waits for 30-min intervals)
- Memory test uses realistic data volumes from SCOPE.md estimates

---

## Phase 2: Day Rollover + WAL Watermarking

**Goal**: Handle day boundaries cleanly. Finalize yesterday's Parquet file as immutable. Clear WAL entries for yesterday. Start fresh accumulator for today.

**Depends on**: Phase 1 complete and tested.

### P2-01: Day rollover timer

**Files**: `core/src/subscribers/bronze.rs`
**Size**: M
**Depends on**: Phase 1 complete

Add a third timer branch to the `select!` loop:

```rust
// Compute duration until next rollover hour
let next_rollover = compute_next_rollover(self.config.day_rollover_utc_hour);
let mut rollover_timer = tokio::time::sleep_until(next_rollover);
```

On rollover:
1. Take a final snapshot of the current accumulator (yesterday's file is now complete)
2. Clear the accumulator
3. Reset WAL (commit all entries -- yesterday's data is fully in Parquet)
4. Start fresh accumulator for today
5. Recompute next rollover time (use `tokio::time::sleep_until` again, not `interval`, to avoid drift)

Helper function:
```rust
fn compute_next_rollover(utc_hour: u8) -> tokio::time::Instant {
    // Calculate wall-clock time to next occurrence of utc_hour:00:00
    // Convert to tokio::time::Instant
}
```

**Acceptance criteria**:
- Rollover fires within 1 second of target time
- After rollover, accumulator is empty and WAL is empty
- Yesterday's Parquet file is complete (all data from the day)
- New events after rollover go to today's date directory
- Timer recomputes correctly (no interval drift over multiple days)

---

### P2-02: WAL watermark persistence

**Files**: `core/src/storage/wal.rs`
**Size**: S
**Depends on**: P1-01

Ensure the WAL watermark is durable:
- Option A: Store watermark in a sidecar file (`wal.watermark`) next to the WAL file
- Option B: First line of WAL is a metadata line `{"_watermark": N}` updated on each `commit_up_to()`
- Option C: Derive watermark from Parquet file metadata (latest timestamp)

Preferred: Option A (sidecar file). Simple, atomic write, no WAL format complexity.

**Acceptance criteria**:
- Watermark survives process restart
- Recovery reads watermark before replaying WAL
- Watermark file updated atomically (write temp + rename)

---

### P2-03: Cross-day event handling

**Files**: `core/src/subscribers/bronze.rs`
**Size**: S
**Depends on**: P2-01

Handle events that arrive for "yesterday" after rollover (clock skew, delayed MQTT delivery):
- Events with a timestamp before today's rollover boundary should be appended to today's accumulator (not yesterday's frozen file)
- Log a warning for late-arriving events
- This is a pragmatic choice: yesterday's file is immutable after rollover, and late events are rare enough to accept in today's file

**Acceptance criteria**:
- Late events are added to the current accumulator (not dropped)
- Warning log emitted for each late event
- No attempt to reopen yesterday's Parquet file

---

### P2-04: Phase 2 tests

**Files**: test modules
**Size**: M
**Depends on**: P2-01, P2-02, P2-03

- `test_day_rollover_finalizes_file` -- snapshot written, accumulator cleared
- `test_rollover_timer_accuracy` -- fires within tolerance
- `test_watermark_survives_restart` -- persist and reload
- `test_late_event_after_rollover` -- goes to today, warning logged
- `test_multi_day_operation` -- run through 2+ simulated rollovers

---

## Phase 3: Read Path Integration + Silver Resilience

**Goal**: Expose the in-memory accumulator to the read path so queries return fresh data. Address the Silver catch-up staleness gap introduced by less-frequent Parquet writes.

**Depends on**: Phase 2 complete. Architecture ADR for read path approach must be decided first.

### P3-01: Architecture decision -- read path approach

**Files**: `product/features/air-017/architecture/`
**Size**: S (decision, not code)
**Depends on**: nothing

Choose between options from SCOPE.md:
- **A. Accumulator-backed BronzeReader**: `read_since()` merges Parquet + accumulator
- **B. Silver-side retry buffer**: Silver buffers failed writes
- **C. Periodic re-catch-up**: Silver re-triggers catch_up
- **D. Accept the gap**: Document as known limitation

Record as ADR in `product/features/air-017/architecture/`.

---

### P3-02: Implement chosen read path

**Files**: depends on ADR decision
**Size**: L
**Depends on**: P3-01

If option A:
- Expose accumulator behind a trait (e.g., `AccumulatorReader`)
- `BronzeReader::read_since()` queries Parquet for data up to watermark, then accumulator for data after watermark
- Merge results, dedup, return

If option B:
- Changes in `core/src/subscribers/silver.rs`
- Add retry buffer for failed writes
- Re-attempt on timer

If option C:
- Detect sustained Silver write failures
- Re-trigger `catch_up()` on recovery

If option D:
- Document limitation in SCOPE.md and STATUS.md
- No code changes

---

### P3-03: Silver catch-up integration

**Files**: `core/src/subscribers/silver.rs`
**Size**: M
**Depends on**: P3-02

Regardless of read path choice, Silver's `catch_up()` must handle the new Bronze snapshot frequency:
- `catch_up()` reads from `BronzeReader.read_since(high_water_mark)`
- With 30-60 min snapshot intervals, the Parquet file may be up to 1 interval behind
- If option A chosen: `read_since` merges accumulator data, gap is closed
- If option D chosen: gap is documented, Docker restart is the recovery path

**Acceptance criteria**:
- Silver catch-up produces the same or better data freshness as before air-017
- If option A: catch-up returns data up to the latest event (not just last snapshot)
- No data loss in the catch-up path

---

### P3-04: Phase 3 tests

**Files**: test modules
**Size**: M
**Depends on**: P3-02, P3-03

- Read path returns Parquet + accumulator data (if option A)
- Silver catch-up integration test with new Bronze architecture
- Data freshness verification (accumulator data is included)
- Recovery scenario: Silver down, Bronze accumulating, Silver recovers, catch-up succeeds

---

## Release Checklist

Per `docs/procedures/RELEASE-POLICY.md`, each phase release requires:

### Per-Phase Release (Phase 1, 2, 3 may each be a MINOR release)

- [ ] Version bump determined (MINOR -- new functionality, backward compatible)
- [ ] All unit tests pass: `cargo test -p neural-core`
- [ ] All integration tests pass
- [ ] Memory profiling on Pi: peak RSS < 150 MiB with full day's accumulator
- [ ] No `todo!()`, `unimplemented!()`, or stub functions (anti-stub rule)
- [ ] Stream configs validated: `./tools/ndp-validate/ndp-validate.sh --all`
- [ ] Manifest created: `.deploy/releases/vX.Y.Z.manifest.json`
- [ ] `CHANGELOG.md` updated with Added/Changed/Fixed sections
- [ ] Commit: `git commit -m "release: vX.Y.Z"`
- [ ] Git tag (annotated): `git tag -a vX.Y.Z -m "Release vX.Y.Z: {description}"`
- [ ] Push code and tag
- [ ] Deploy to Pi: `./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json`
- [ ] Verify: `cat /var/ndp/deployed-version`
- [ ] Smoke test: data flows through WAL -> accumulator -> snapshot -> Parquet
- [ ] Monitor logs for errors: `./deploy.sh logs`
- [ ] `product/features/air-017/STATUS.md` updated

### Manifest Changes Array (Phase 1 example)

```json
{
  "changes": [
    {"type": "container", "target": "air-quality-app", "action": "build"},
    {"type": "container", "target": "air-quality-app", "action": "restart"}
  ]
}
```

No database migrations. No config format breaking changes (new fields have defaults).
Container rebuild required because `core` crate changes affect the app binary.

---

## Definition of Done

### Phase 1 Done

- WAL append happens on event receipt in BronzeSubscriber (not in ParquetStore flush)
- InMemoryAccumulator holds the full day's data, persists across flush intervals
- Snapshot writes Parquet from accumulator without reading existing file
- WAL watermark tracks last snapshot position
- Startup recovery: Parquet seed + WAL replay rebuilds accumulator
- All existing functionality preserved (data flows, Silver catch-up, MCP queries)
- Peak memory on Pi < 150 MiB with realistic data volumes
- No read-modify-write in the raw data write path

### Phase 2 Done

- Day rollover finalizes yesterday's Parquet file (immutable after rollover)
- WAL entries for yesterday are truncated after rollover
- Fresh accumulator starts for each new day
- Multi-day operation stable with no timer drift
- Late-arriving events handled gracefully

### Phase 3 Done

- Read path returns data up to latest event (not just last snapshot) -- if option A chosen
- Silver catch-up works correctly with new Bronze architecture
- Data freshness is equal to or better than pre-air-017
- Architecture decision recorded as ADR

---

## Dependency Graph

```
P1-01 (WAL v2) ─────────────────┐
                                 ├──► P1-03 (WAL in BronzeSubscriber) ──► P1-04 (snapshot timer)
P1-02 (Accumulator) ────────────┤                                         │
                                 │                                         ▼
P1-07 (Config fields) ──────────┘                              P1-05 (snapshot_to_parquet)
                                                                          │
                                                                          ▼
                                                               P1-06 (startup recovery)
                                                                          │
                                                               P1-08 (remove read-modify-write)
                                                                          │
                                                                          ▼
                                                               P1-09 (unit tests)
                                                                          │
                                                                          ▼
                                                               P1-10 (integration tests)
                                                                          │
                                                                          ▼
                                                               Phase 2 ──► Phase 3
```

Items P1-01, P1-02, and P1-07 can be developed in parallel (no interdependencies).
P1-03 is the critical path and requires both P1-01 and P1-02.
P1-09 and P1-10 are naturally last but test development can start alongside implementation.

---

## Sizing Summary

| Item | Size | Est. Effort |
|------|------|-------------|
| P1-01 WAL v2 | M | ~200 lines new/modified |
| P1-02 Accumulator | M | ~150 lines new |
| P1-03 WAL in BronzeSubscriber | L | ~300 lines modified |
| P1-04 Snapshot timer | M | ~50 lines new |
| P1-05 snapshot_to_parquet | M | ~100 lines new |
| P1-06 Startup recovery | L | ~150 lines new |
| P1-07 Config fields | S | ~30 lines new |
| P1-08 Remove read-modify-write | M | ~50 lines modified |
| P1-09 Unit tests | L | ~400 lines new |
| P1-10 Integration tests | L | ~300 lines new |
| **Phase 1 total** | | **~1,700 lines** |
| P2-01 through P2-04 | M | ~500 lines |
| P3-01 through P3-04 | L | ~600 lines (depends on ADR) |
| **Feature total** | | **~2,800 lines** |
