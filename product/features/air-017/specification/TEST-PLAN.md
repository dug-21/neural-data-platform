# AIR-017 Test Plan: Bronze Write-Ahead Architecture

> **Feature:** air-017
> **Created:** 2026-02-08
> **Status:** Draft
> **Author:** ndp-architect (specification phase)
> **AgentDB Patterns Used:** ID 10 (specification:config-validation-pipeline), ID 29 (conventions:feature-directory-structure)

---

## 1. Test Strategy Overview

### 1.1 Testing Layers

| Layer | Purpose | Count Target | Speed |
|-------|---------|--------------|-------|
| **Unit Tests** | Verify individual functions and structs in isolation | ~55 | < 30s |
| **Integration Tests** | Verify component interactions (WAL + accumulator + Parquet) | ~20 | < 60s |
| **Property-Based Tests** | Fuzz invariants (WAL ordering, dedup correctness, eviction) | ~10 | < 120s |
| **Benchmark Tests** | Verify NFR memory/latency/throughput on target hardware | ~5 | < 300s |

### 1.2 Test Methodology

All tests follow London TDD (AgentDB pattern ID 16):
- **Arrange-Act-Assert** structure
- **Behavior verification** over implementation testing
- **Test naming**: `test_<component>_<scenario>_<expected>`
- **Mocks** for RawStore trait; real files for WAL and Parquet

### 1.3 Test Dependencies

| Dependency | Used For |
|------------|----------|
| `tempfile::TempDir` | Isolated filesystem for WAL and Parquet tests |
| `mockall` | MockRawStore for BronzeSubscriber behavior tests |
| `tokio::test` | Async test runtime |
| `proptest` | Property-based test generation (existing dev dependency or to be added) |

---

## 2. Unit Tests: WriteAheadLog

### 2.1 Construction and Lifecycle

**WAL-UNIT-01: WAL directory creation**
- Arrange: Non-existent directory path.
- Act: `WriteAheadLog::new(path)`.
- Assert: Directory is created, `streams()` returns empty vec.

**WAL-UNIT-02: WAL discovers existing stream files on construction**
- Arrange: Pre-create `{dir}/air-quality.wal` with 3 entries and `{dir}/air-quality.wal.seq` with value `3`.
- Act: `WriteAheadLog::new(dir)`.
- Assert: `streams()` returns `["air-quality"]`, `current_seq("air-quality")` returns `Some(3)`.

**WAL-UNIT-03: WAL resumes sequence from seq file**
- Arrange: Create WAL with 5 entries, seq file contains `5`.
- Act: Construct new WAL, append one entry.
- Assert: New entry has `seq=6`.

**WAL-UNIT-04: WAL recovers sequence from data when seq file is missing**
- Arrange: Create WAL with entries seq 1-5, delete the `.seq` file.
- Act: Construct new WAL, append one entry.
- Assert: New entry has `seq=6` (scanned from data file).

**WAL-UNIT-05: WAL recovers sequence from data when seq file is corrupt**
- Arrange: Create WAL with entries seq 1-5, write garbage to `.seq` file.
- Act: Construct new WAL, append one entry.
- Assert: New entry has `seq=6`.

### 2.2 Append Operations

**WAL-UNIT-06: Append single entry**
- Arrange: Empty WAL for stream "test-stream".
- Act: `wal.append("test-stream", &point)`.
- Assert: Returns `Ok(1)`, file contains 1 line, line is valid JSON with `seq=1`.

**WAL-UNIT-07: Append multiple entries to same stream**
- Arrange: Empty WAL.
- Act: Append 5 points to "air-quality".
- Assert: Returns seq 1-5, file contains 5 lines, all parseable as `WalEntry`.

**WAL-UNIT-08: Append to multiple streams**
- Arrange: Empty WAL.
- Act: Append 3 points to "air-quality", 2 to "outdoor-weather".
- Assert: Two separate files exist, correct entry counts in each.

**WAL-UNIT-09: Append preserves RawDataPoint fields**
- Arrange: Create a RawDataPoint with all fields populated (timestamp, source_id, ndp_id, context, raw_payload).
- Act: Append, then replay.
- Assert: Replayed entry's `data` field equals the original point (PartialEq check).

**WAL-UNIT-10: Append flushes to disk**
- Arrange: Empty WAL.
- Act: Append one entry. Do NOT close the WAL. Open the file directly with `File::open` and read.
- Assert: File contains the entry (proves flush happened).

**WAL-UNIT-11: Sequence numbers are monotonically increasing across restarts**
- Arrange: Append 5 entries (seq 1-5). Drop WAL. Reconstruct.
- Act: Append 3 more entries.
- Assert: New entries have seq 6, 7, 8.

### 2.3 Replay Operations

**WAL-UNIT-12: Replay all entries (watermark=0)**
- Arrange: WAL with 5 entries (seq 1-5).
- Act: `replay_stream("stream", after_watermark=0)`.
- Assert: Returns 5 entries in seq order.

**WAL-UNIT-13: Replay after watermark**
- Arrange: WAL with 10 entries (seq 1-10).
- Act: `replay_stream("stream", after_watermark=7)`.
- Assert: Returns 3 entries with seq 8, 9, 10.

**WAL-UNIT-14: Replay empty WAL**
- Arrange: Empty WAL (no file for stream).
- Act: `replay_stream("nonexistent", 0)`.
- Assert: Returns empty Vec (not an error).

**WAL-UNIT-15: Replay skips corrupt lines**
- Arrange: WAL file with 3 valid entries and 1 line of garbage ("NOT JSON") between entries 2 and 3.
- Act: `replay_stream("stream", 0)`.
- Assert: Returns 3 entries (the corrupt line is skipped), warning is logged.

**WAL-UNIT-16: Replay with watermark beyond all entries**
- Arrange: WAL with entries seq 1-5.
- Act: `replay_stream("stream", after_watermark=100)`.
- Assert: Returns empty Vec.

### 2.4 Truncation Operations

**WAL-UNIT-17: Truncate removes entries at or below watermark**
- Arrange: WAL with entries seq 1-10.
- Act: `truncate_before("stream", watermark=5)`.
- Assert: `replay_stream("stream", 0)` returns 5 entries (seq 6-10).

**WAL-UNIT-18: Truncate all entries**
- Arrange: WAL with entries seq 1-5.
- Act: `truncate_before("stream", watermark=5)`.
- Assert: `replay_stream("stream", 0)` returns empty Vec. File exists but is empty.

**WAL-UNIT-19: Truncate with watermark=0 (no-op)**
- Arrange: WAL with entries seq 1-5.
- Act: `truncate_before("stream", watermark=0)`.
- Assert: All 5 entries still present.

**WAL-UNIT-20: Truncate uses atomic rename**
- Arrange: WAL with entries seq 1-10.
- Act: During truncate, verify that a `.wal.tmp` file is created then renamed.
- Assert: No `.wal.tmp` file remains after truncation completes.

**WAL-UNIT-21: Truncate updates sequence counter correctly**
- Arrange: WAL with entries seq 1-10. Truncate to watermark=7.
- Act: Append a new entry.
- Assert: New entry has seq=11 (not seq=4, which would be wrong if counter reset).

**WAL-UNIT-22: Truncate nonexistent stream is no-op**
- Arrange: Empty WAL.
- Act: `truncate_before("nonexistent", 50)`.
- Assert: Returns Ok(()).

---

## 3. Unit Tests: In-Memory Accumulator

### 3.1 Basic Operations

**ACC-UNIT-01: Empty accumulator**
- Arrange: New accumulator.
- Act: Read lock, check size.
- Assert: HashMap is empty.

**ACC-UNIT-02: Push single point to stream**
- Arrange: Empty accumulator.
- Act: Push one RawDataPoint for "air-quality".
- Assert: accumulator["air-quality"].len() == 1.

**ACC-UNIT-03: Push to multiple streams**
- Arrange: Empty accumulator.
- Act: Push 3 points to "air-quality", 2 to "outdoor-weather".
- Assert: Two keys, correct lengths.

**ACC-UNIT-04: Points are appended in order**
- Arrange: Empty accumulator.
- Act: Push points with timestamps T1, T2, T3 (in order).
- Assert: Vec contains points in [T1, T2, T3] order.

### 3.2 Eviction

**ACC-UNIT-05: No eviction below limit**
- Arrange: max_accumulator_points=100.
- Act: Push 99 points to one stream.
- Assert: All 99 present, no eviction.

**ACC-UNIT-06: Eviction at limit**
- Arrange: max_accumulator_points=5.
- Act: Push 7 points with values [1,2,3,4,5,6,7].
- Assert: Vec contains [3,4,5,6,7] (oldest 2 evicted, FIFO).

**ACC-UNIT-07: Eviction logs warning**
- Arrange: max_accumulator_points=3, tracing subscriber captured.
- Act: Push 5 points.
- Assert: Warning log emitted with stream_id and eviction count (2).

**ACC-UNIT-08: Eviction is per-stream**
- Arrange: max_accumulator_points=3.
- Act: Push 5 points to "stream-a", 2 to "stream-b".
- Assert: "stream-a" has 3 points (2 evicted), "stream-b" has 2 points (none evicted).

### 3.3 Clear Operations

**ACC-UNIT-09: Clear single stream**
- Arrange: Accumulator with 3 streams.
- Act: Clear "air-quality".
- Assert: "air-quality" key removed (or Vec empty), other 2 streams untouched.

**ACC-UNIT-10: Clear all streams**
- Arrange: Accumulator with 3 streams.
- Act: Clear all.
- Assert: HashMap is empty.

---

## 4. Unit Tests: BronzeSubscriber

### 4.1 Config Deserialization

**BSUB-UNIT-01: Default config includes new fields**
- Arrange: `BronzeSubscriberConfig::default()`.
- Assert: `snapshot_interval_secs == 1800`, `day_rollover_utc_hour == 0`, `max_accumulator_points == 50_000`, `wal_dir == None`, `snapshot_on_shutdown == true`.

**BSUB-UNIT-02: Deserialize YAML with new fields**
- Arrange: YAML with `snapshot_interval_secs: 900`.
- Act: Deserialize.
- Assert: `snapshot_interval_secs == 900`, other new fields at defaults.

**BSUB-UNIT-03: Deserialize YAML without new fields (backward compat)**
- Arrange: YAML with only `batch_size: 50` (pre-air-017 config).
- Act: Deserialize.
- Assert: All new fields at defaults, `batch_size == 50`.

### 4.2 handle_point() Behavior

**BSUB-UNIT-04: handle_point writes to WAL then accumulator**
- Arrange: BronzeSubscriber with mock store and real TempDir WAL.
- Act: `handle_point(point)`.
- Assert: WAL file contains 1 entry AND accumulator["stream"] contains 1 point AND events_received == 1.

**BSUB-UNIT-05: handle_point increments events_received**
- Arrange: BronzeSubscriber.
- Act: Call handle_point 5 times.
- Assert: events_received == 5.

**BSUB-UNIT-06: handle_point respects stream filter**
- Arrange: stream_filter = ["air-quality"].
- Act: handle_point with source_id="outdoor-weather-Http".
- Assert: WAL is empty, accumulator is empty, events_received == 1 (counted but filtered).

**BSUB-UNIT-07: handle_point survives WAL failure**
- Arrange: WAL directory set to read-only path (simulated I/O error).
- Act: handle_point(point).
- Assert: Point IS in accumulator (best-effort), errors_total == 1, error is logged.

### 4.3 Snapshot Behavior

**BSUB-UNIT-08: Snapshot writes Parquet from accumulator**
- Arrange: BronzeSubscriber with 100 points in accumulator for "air-quality".
- Act: Trigger snapshot (call internal snapshot method directly).
- Assert: Parquet file exists at expected partition path, contains 100 rows.

**BSUB-UNIT-09: Snapshot overwrites existing Parquet (no append)**
- Arrange: Pre-create Parquet with 50 points. Add 100 points to accumulator.
- Act: Trigger snapshot.
- Assert: Parquet file contains exactly 100 rows (not 150).

**BSUB-UNIT-10: Snapshot writes watermark file**
- Arrange: WAL has entries up to seq=42. Accumulator has matching points.
- Act: Trigger snapshot.
- Assert: `snapshot.watermark` file exists and contains "42".

**BSUB-UNIT-11: Snapshot failure does not advance watermark**
- Arrange: Mock store's write_raw_parquet to return error.
- Act: Trigger snapshot.
- Assert: No `snapshot.watermark` file is written (or old watermark unchanged), snapshot_failures incremented.

**BSUB-UNIT-12: Snapshot handles multiple streams independently**
- Arrange: Accumulator with "air-quality" (50 points) and "outdoor-weather" (30 points).
- Act: Trigger snapshot.
- Assert: Two separate Parquet files, correct point counts in each, two separate watermark files.

**BSUB-UNIT-13: Snapshot clones accumulator (does not drain)**
- Arrange: Accumulator with 100 points.
- Act: Trigger snapshot. Then check accumulator.
- Assert: Accumulator still contains 100 points (snapshot did not drain it).

### 4.4 Shutdown Behavior

**BSUB-UNIT-14: Graceful shutdown performs snapshot when configured**
- Arrange: snapshot_on_shutdown=true, accumulator has 50 points.
- Act: Cancel the subscriber via cancellation_token.
- Assert: Parquet file is written before shutdown completes.

**BSUB-UNIT-15: Graceful shutdown skips snapshot when configured**
- Arrange: snapshot_on_shutdown=false, accumulator has 50 points.
- Act: Cancel the subscriber.
- Assert: No new Parquet file is written.

---

## 5. Unit Tests: Recovery

**REC-UNIT-01: Recovery with Parquet only (no WAL)**
- Arrange: Today's Parquet file with 100 points. No WAL file. No watermark file.
- Act: Run recovery.
- Assert: Accumulator contains 100 points.

**REC-UNIT-02: Recovery with WAL only (no Parquet)**
- Arrange: WAL with 20 entries. No Parquet file. No watermark file.
- Act: Run recovery.
- Assert: Accumulator contains 20 points.

**REC-UNIT-03: Recovery with Parquet + WAL (no overlap)**
- Arrange: Parquet with 100 points. Watermark=50. WAL entries seq 51-70.
- Act: Run recovery.
- Assert: Accumulator contains 120 points (100 from Parquet + 20 from WAL).

**REC-UNIT-04: Recovery with Parquet + WAL (overlap / dedup)**
- Arrange: Parquet with 100 points (including points from WAL seq 45-50). Watermark=44. WAL entries seq 45-60.
- Act: Run recovery.
- Assert: Accumulator contains 116 points (100 from Parquet + 16 new from WAL; 6 duplicates from seq 45-50 are deduped).

**REC-UNIT-05: Recovery dedup uses (timestamp_micros, source_id) key**
- Arrange: Parquet point A with timestamp T1, source_id S1. WAL point B with same T1 and S1 but different raw_payload.
- Act: Run recovery.
- Assert: Only point A appears (Parquet takes precedence, WAL duplicate is skipped).

**REC-UNIT-06: Recovery with missing watermark file**
- Arrange: Parquet with 100 points. WAL with 120 entries (some overlap). No watermark file.
- Act: Run recovery.
- Assert: All WAL entries are replayed, duplicates against Parquet are deduped. Final count < 220 (overlapping entries removed).

**REC-UNIT-07: Recovery skips corrupt WAL entries**
- Arrange: WAL with 10 valid entries and 2 corrupt lines.
- Act: Run recovery.
- Assert: Accumulator contains 10 points (corrupt lines skipped with warnings).

**REC-UNIT-08: Recovery with empty WAL and empty Parquet**
- Arrange: Neither WAL nor Parquet exists for a stream.
- Act: Run recovery.
- Assert: Accumulator for that stream is empty (not an error).

**REC-UNIT-09: Recovery reads pre-air-017 Parquet files correctly**
- Arrange: Parquet file written by pre-air-017 code (same schema, no watermark file).
- Act: Run recovery.
- Assert: All points are loaded correctly (NFR-007 backward compat).

**REC-UNIT-10: Recovery from legacy single wal.log**
- Arrange: Old-style `wal.log` file at `{base_path}/wal.log` (pre-air-017 format, no seq numbers).
- Act: Run recovery.
- Assert: Legacy entries are replayed (best-effort, dedup by timestamp+source_id), then the legacy file is renamed to `wal.log.migrated`.

---

## 6. Integration Tests

### 6.1 End-to-End Write Path

**INT-01: Event receipt through to Parquet snapshot**
- Arrange: Full BronzeSubscriber with real WAL and real ParquetStore, snapshot_interval_secs=2 (fast test).
- Act: Send 50 events via broadcast channel. Wait 3 seconds.
- Assert: WAL contains entries, Parquet file exists with 50 rows, watermark file exists.

**INT-02: Multiple snapshots accumulate data**
- Arrange: snapshot_interval_secs=1.
- Act: Send 20 events, wait 1.5s (snapshot 1), send 20 more events, wait 1.5s (snapshot 2).
- Assert: Final Parquet contains 40 rows (snapshot 2 overwrites with full accumulator).

**INT-03: WAL truncation after snapshot**
- Arrange: snapshot_interval_secs=1.
- Act: Send 20 events, wait for snapshot + truncation.
- Assert: WAL file is empty or contains only entries after the watermark.

**INT-04: Crash simulation and recovery**
- Arrange: Send 50 events with snapshot_interval_secs=3600 (no snapshot during test). Kill subscriber (drop).
- Act: Construct new BronzeSubscriber pointing at same directory. Run recovery.
- Assert: Accumulator contains 50 points recovered from WAL.

**INT-05: Crash after partial snapshot**
- Arrange: Send 100 events. Force a snapshot (writes Parquet, writes watermark). Send 20 more events (in WAL only). Drop subscriber.
- Act: Construct new subscriber. Run recovery.
- Assert: Accumulator contains 120 points (100 from Parquet + 20 from WAL replay).

**INT-06: Multi-stream isolation**
- Arrange: Send events for 3 streams simultaneously.
- Act: Wait for snapshot.
- Assert: 3 separate Parquet files, 3 separate WAL files, 3 separate watermark files. Correct point counts in each.

### 6.2 BronzeSubscriber select! Loop

**INT-07: Snapshot timer fires at configured interval**
- Arrange: snapshot_interval_secs=1. Start subscriber with broadcast channel.
- Act: Send 5 events. Wait 2.5 seconds.
- Assert: At least 2 snapshots have been written (verified by Parquet file mtime or snapshots_written counter).

**INT-08: Events during snapshot are not lost**
- Arrange: Start subscriber. Configure slow snapshot (mock store with 500ms delay).
- Act: Send 10 events before snapshot. Trigger snapshot. Send 10 more during snapshot.
- Assert: After snapshot completes, accumulator contains 20 events. Next snapshot writes 20 rows.

**INT-09: Cancellation stops subscriber cleanly**
- Arrange: Start subscriber with snapshot_on_shutdown=true.
- Act: Send 30 events. Cancel via token.
- Assert: Parquet file exists (shutdown snapshot), subscriber task completes without error.

### 6.3 Configuration

**INT-10: platform.yaml round-trip**
- Arrange: Write a platform.yaml with all air-017 fields.
- Act: Parse via serde_yaml, serialize back to YAML.
- Assert: All fields preserved with correct values.

**INT-11: Old platform.yaml compatibility**
- Arrange: Use the current `config/base/platform.yaml` (no air-017 fields).
- Act: Deserialize BronzeSubscriberConfig from `subscribers.bronze` section.
- Assert: New fields have defaults, subscriber starts normally.

---

## 7. Property-Based Tests

### 7.1 WAL Invariants

**PROP-01: WAL sequence numbers are strictly monotonically increasing**
- Generator: Random sequence of append and truncate operations.
- Property: For any two consecutive entries in a WAL file, `entry[i].seq < entry[i+1].seq`.

**PROP-02: WAL replay after truncate returns correct subset**
- Generator: Random N entries (1-1000), random watermark (0-N).
- Property: `replay_stream(after_watermark=W)` returns exactly the entries with seq > W, in seq order.

**PROP-03: WAL truncate + replay is idempotent**
- Generator: Random entries, random watermark.
- Property: `truncate_before(W); replay(0)` equals `replay(W)` on the pre-truncated WAL.

**PROP-04: WAL round-trip preserves RawDataPoint**
- Generator: Arbitrary RawDataPoint (random strings, nested JSON up to depth 5).
- Property: `append(point); replay()` returns a point where `replayed.data == original`.

### 7.2 Accumulator Invariants

**PROP-05: Accumulator length never exceeds max_accumulator_points**
- Generator: Random sequence of push operations (1-100,000), max_accumulator_points=100.
- Property: After every push, `accumulator[stream].len() <= max_accumulator_points`.

**PROP-06: FIFO eviction preserves newest points**
- Generator: N pushes with sequential IDs, max=M (M < N).
- Property: After all pushes, accumulator contains the last M points by insertion order.

### 7.3 Recovery Invariants

**PROP-07: Recovery dedup produces no duplicate (timestamp, source_id) pairs**
- Generator: Random Parquet points (P) and WAL entries (W) with some overlap.
- Property: After recovery merge, no two points in the accumulator share the same `(timestamp_micros, source_id)`.

**PROP-08: Recovery never loses data that was in either Parquet or WAL**
- Generator: Disjoint Parquet points (P) and WAL entries (W).
- Property: `accumulator.len() == P.len() + W.len()`.

**PROP-09: Recovery with overlapping data has correct count**
- Generator: P points in Parquet, W entries in WAL, O of which overlap (present in both).
- Property: `accumulator.len() == P.len() + W.len() - O`.

### 7.4 Snapshot Invariants

**PROP-10: Snapshot Parquet contains exactly accumulator contents**
- Generator: Random accumulator contents (1-10,000 points).
- Property: After snapshot, reading back the Parquet yields the same points (by value, ignoring order).

---

## 8. Benchmark Tests

**BENCH-01: WAL append latency**
- Setup: WAL on tmpfs (to isolate from disk variance in CI, run on real disk for Pi benchmarks).
- Action: Append 10,000 entries, measure per-append latency.
- Target: p99 < 10ms (NFR-002).

**BENCH-02: Snapshot write throughput**
- Setup: Accumulator with 11,000 points per stream, 4 streams.
- Action: Snapshot all 4 streams sequentially, measure total time.
- Target: < 2 seconds for all 4 streams.

**BENCH-03: Recovery time**
- Setup: 4 Parquet files (11,000 points each) + 4 WAL files (500 entries each).
- Action: Run full recovery procedure, measure elapsed time.
- Target: < 5 seconds on Raspberry Pi 5 (NFR-004).

**BENCH-04: Memory usage during snapshot**
- Setup: 4 streams, 11,000 points each in accumulator.
- Action: Trigger snapshot, measure peak RSS via `/proc/self/status` VmRSS.
- Target: < 150 MiB (NFR-001).

**BENCH-05: WAL disk usage**
- Setup: Append 30 minutes of data at 1 point / 8 seconds for 4 streams.
- Action: Measure total WAL size on disk.
- Target: < 500 KiB per stream, < 2 MiB total (well under NFR-005 10 MiB cap).

---

## 9. Test Matrix vs. Requirements

| Requirement | Unit Tests | Integration Tests | Property Tests | Benchmarks |
|-------------|-----------|------------------|---------------|-----------|
| FR-001 (WAL on receipt) | BSUB-UNIT-04 | INT-01 | - | BENCH-01 |
| FR-002 (WAL entry format) | WAL-UNIT-06, -07, -09 | - | PROP-04 | - |
| FR-003 (Sequence persistence) | WAL-UNIT-02, -03, -04, -05, -11 | - | PROP-01 | - |
| FR-004 (Per-stream WAL) | WAL-UNIT-08 | INT-06 | - | - |
| FR-005 (WAL failure) | BSUB-UNIT-07 | - | - | - |
| FR-006 (Accumulator struct) | ACC-UNIT-01, -02, -03, -04 | - | - | - |
| FR-007 (Accumulator population) | BSUB-UNIT-04, -05 | INT-01 | - | - |
| FR-008 (RwLock wrapper) | ACC-UNIT-02 (via RwLock) | INT-08 | - | - |
| FR-009 (Memory cap) | ACC-UNIT-05, -06, -07, -08 | - | PROP-05, PROP-06 | - |
| FR-010 (Clear on rollover) | ACC-UNIT-09, -10 | - | - | - |
| FR-011 (Snapshot timer) | - | INT-07 | - | - |
| FR-012 (Snapshot write) | BSUB-UNIT-08, -09, -10, -12, -13 | INT-01, INT-02 | PROP-10 | BENCH-02 |
| FR-014 (Watermark file) | BSUB-UNIT-10 | INT-01 | - | - |
| FR-019 (Watermark truncation) | WAL-UNIT-17, -18, -19, -21, -22 | INT-03 | PROP-02, PROP-03 | - |
| FR-020 (Atomic rename) | WAL-UNIT-20 | - | - | - |
| FR-021 (Recovery) | REC-UNIT-01 through -10 | INT-04, INT-05 | PROP-07, -08, -09 | BENCH-03 |
| FR-022 (Dedup) | REC-UNIT-04, -05 | INT-05 | PROP-07, PROP-09 | - |
| FR-023 (Corrupt WAL) | WAL-UNIT-15, REC-UNIT-07 | - | - | - |
| FR-024 (Missing watermark) | REC-UNIT-06 | - | - | - |
| FR-025 (Config fields) | BSUB-UNIT-01, -02, -03 | INT-10, INT-11 | - | - |
| FR-027 (Snapshot failure) | BSUB-UNIT-11 | - | - | - |
| FR-029 (Shutdown) | BSUB-UNIT-14, -15 | INT-09 | - | - |
| NFR-001 (Memory) | - | - | - | BENCH-04 |
| NFR-002 (Durability latency) | - | - | - | BENCH-01 |
| NFR-004 (Recovery time) | - | - | - | BENCH-03 |
| NFR-005 (WAL disk) | - | - | - | BENCH-05 |
| NFR-007 (Backward compat) | REC-UNIT-09 | - | - | - |
| NFR-008 (Config compat) | BSUB-UNIT-03 | INT-11 | - | - |

---

## 10. Test Data Generators

### 10.1 RawDataPoint Generator

```rust
fn gen_raw_point(stream_id: &str, index: usize) -> RawDataPoint {
    let source_id = format!("{}-Http", stream_id);
    RawDataPoint::new(
        &source_id,
        serde_json::json!({
            "index": index,
            "pm25": 10.0 + (index as f64 * 0.1),
            "co2": 400 + index,
        }),
    )
    .with_timestamp(Utc::now() + chrono::Duration::seconds(index as i64))
    .with_ndp_id(format!("{}-sensor-001", stream_id))
}
```

### 10.2 Multi-Stream Point Batch

```rust
fn gen_multi_stream_batch(streams: &[&str], points_per_stream: usize) -> Vec<RawDataPoint> {
    streams.iter().flat_map(|stream| {
        (0..points_per_stream).map(move |i| gen_raw_point(stream, i))
    }).collect()
}
```

### 10.3 Pre-Populated WAL for Recovery Tests

```rust
fn create_test_wal(dir: &Path, stream_id: &str, count: usize) -> u64 {
    let mut wal = WriteAheadLog::new(dir).unwrap();
    let mut last_seq = 0;
    for i in 0..count {
        last_seq = wal.append(stream_id, &gen_raw_point(stream_id, i)).unwrap();
    }
    last_seq
}
```

### 10.4 Pre-Populated Parquet for Recovery Tests

```rust
async fn create_test_parquet(store: &ParquetStore, stream_id: &str, count: usize) {
    let points: Vec<RawDataPoint> = (0..count)
        .map(|i| gen_raw_point(stream_id, i))
        .collect();
    let path = store.raw_partition_path(
        &format!("{}-Http", stream_id),
        points[0].timestamp,
    );
    store.write_raw_parquet(points, &path).await.unwrap();
}
```
