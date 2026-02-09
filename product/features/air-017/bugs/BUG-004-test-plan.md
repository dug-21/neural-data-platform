# BUG-004 Test Plan: Bronze Accumulator Memory Leak Fix

> **Bug:** BUG-004
> **Feature:** air-017
> **Created:** 2026-02-09
> **Fix Summary:** Remove in-memory Accumulator entirely. Snapshot reads from WAL replay instead of memory.

---

## Root Cause

The `Accumulator` (a `HashMap<String, Vec<RawDataPoint>>`) holds every data point for the entire day in memory and never clears. Over 24 hours on a Raspberry Pi with 4 streams at ~11K points/stream, this grows to ~22 MiB and stays resident forever. The fix eliminates the accumulator from the hot path entirely: `handle_point()` only appends to WAL, and `snapshot()` replays the WAL from disk to build the Parquet data on demand.

## New Design Summary

| Method | Before (accumulator) | After (WAL-only) |
|--------|---------------------|-------------------|
| `handle_point()` | WAL append + `accumulator.add()` | WAL append only. Increment `events_received`. |
| `snapshot()` | Read from `accumulator.all_points_by_source()` | Call `wal.replay_since(0)` or `wal.replay_all()`, group by `source_id`, write Parquet per source via `store.write_raw_snapshot()`. Do NOT truncate WAL. |
| `recover()` | Parquet seed + WAL replay into accumulator | No-op or minimal (log WAL existence/size). WAL is the source of truth. |
| `start()` heartbeat | Log `accumulator.count()` | Log WAL file size on disk. |
| Day rollover | Not yet implemented | Truncate WAL, start fresh for new day. |
| `health_check()` | Report `accumulator_count` | Report `wal_entry_count` or WAL file size. |
| `BronzeSubscriber` struct | Contains `accumulator: Accumulator` field | Field removed. |

---

## 1. Tests to REMOVE (Accumulator-Specific)

These tests exist in `core/src/storage/accumulator.rs` mod tests. The entire `accumulator.rs` file is deleted or deprecated by this fix. All 15 tests are removed.

| # | Test Name | File | Reason |
|---|-----------|------|--------|
| 1 | `test_new_accumulator_is_empty` | `accumulator.rs:213` | Tests Accumulator constructor. Type no longer exists. |
| 2 | `test_add_single_point` | `accumulator.rs:227` | Tests `Accumulator::add()`. Method removed. |
| 3 | `test_add_multiple_points_same_source` | `accumulator.rs:246` | Tests multi-point add to same source. No accumulator. |
| 4 | `test_add_multiple_points_different_sources` | `accumulator.rs:264` | Tests multi-source grouping. No accumulator. |
| 5 | `test_earliest_latest_in_order` | `accumulator.rs:284` | Tests timestamp tracking. Accumulator removed. |
| 6 | `test_earliest_latest_reverse_order` | `accumulator.rs:301` | Tests timestamp tracking with reverse insertion. Accumulator removed. |
| 7 | `test_clear_resets_everything_except_date` | `accumulator.rs:320` | Tests `Accumulator::clear()`. Method removed. |
| 8 | `test_drain_for_date_partitions_by_date` | `accumulator.rs:343` | Tests `drain_for_date()`. Method removed. |
| 9 | `test_drain_for_date_removes_empty_source_buckets` | `accumulator.rs:381` | Tests bucket cleanup on drain. Method removed. |
| 10 | `test_merge_wal_entries_no_duplicates` | `accumulator.rs:416` | Tests WAL merge into accumulator. No accumulator. |
| 11 | `test_merge_wal_entries_deduplicates` | `accumulator.rs:434` | Tests dedup during merge. No accumulator. |
| 12 | `test_merge_wal_entries_mixed_duplicate_and_new` | `accumulator.rs:455` | Tests mixed merge. No accumulator. |
| 13 | `test_memory_estimate_bytes` | `accumulator.rs:480` | Tests memory estimation. No accumulator. |
| 14 | `test_memory_estimate_empty` | `accumulator.rs:499` | Tests empty memory estimate. No accumulator. |
| 15 | `test_all_points_by_source_does_not_consume` | `accumulator.rs:512` | Tests reference semantics. No accumulator. |

**Action:** Delete `core/src/storage/accumulator.rs` entirely. Remove `pub mod accumulator;` and `pub use accumulator::Accumulator;` from `core/src/storage/mod.rs`.

---

## 2. Tests to KEEP Unchanged

### 2.1 WAL Tests (all 26 in `core/src/storage/wal.rs`)

Every WAL test remains valid. The WAL API does not change; only the caller changes (BronzeSubscriber no longer feeds accumulator after WAL append).

| # | Test Name | Lines | Reason to Keep |
|---|-----------|-------|----------------|
| 1 | `test_wal_entry_serialization_round_trip` | `wal.rs:332` | WalEntry format unchanged. |
| 2 | `test_wal_creation` | `wal.rs:353` | WAL constructor unchanged. |
| 3 | `test_wal_append_returns_incrementing_sequences` | `wal.rs:367` | append_point API unchanged. |
| 4 | `test_wal_replay_since_zero_returns_all` | `wal.rs:386` | replay_since(0) is now the primary snapshot data source. |
| 5 | `test_wal_replay_since_filters_by_watermark` | `wal.rs:410` | Watermark filtering still used. |
| 6 | `test_wal_commit_to_removes_committed_entries` | `wal.rs:431` | commit_to used for day rollover. |
| 7 | `test_wal_commit_to_noop_for_stale_watermark` | `wal.rs:456` | Edge case still relevant. |
| 8 | `test_wal_persistence_across_instances` | `wal.rs:486` | Critical for crash recovery. |
| 9 | `test_wal_watermark_persistence_across_instances` | `wal.rs:513` | Critical for crash recovery. |
| 10 | `test_wal_skips_corrupted_trailing_line` | `wal.rs:542` | Crash safety unchanged. |
| 11 | `test_wal_legacy_append_and_replay` | `wal.rs:571` | Legacy API for ParquetStore. |
| 12 | `test_wal_legacy_commit_clears_log` | `wal.rs:589` | Legacy API. |
| 13 | `test_wal_legacy_append_after_commit` | `wal.rs:608` | Legacy API. |
| 14 | `test_wal_legacy_empty_replay` | `wal.rs:629` | Legacy API. |
| 15 | `test_wal_legacy_invalid_utf8` | `wal.rs:640` | Legacy API error handling. |
| 16 | `test_wal_legacy_persistence_across_instances` | `wal.rs:656` | Legacy API durability. |
| 17 | `test_wal_empty_replay_since` | `wal.rs:678` | Empty WAL edge case for snapshot. |
| 18 | `test_wal_legacy_replay_skips_watermark_headers` | `wal.rs:691` | Header filtering. |
| 19 | `test_wal_append_after_commit_to` | `wal.rs:715` | Append after partial commit. |
| 20 | `test_wal_commit_to_all_entries` | `wal.rs:742` | Full commit (day rollover). |
| 21 | `test_wal_entry_preserves_source_id` | `wal.rs:764` | Source ID tracking for grouping. |

### 2.2 BronzeSubscriber Config Tests (kept unchanged, 7 tests)

| # | Test Name | Lines | Reason to Keep |
|---|-----------|-------|----------------|
| 1 | `test_config_default_values` | `bronze.rs:575` | Config defaults unchanged. |
| 2 | `test_config_deserialize_with_defaults` | `bronze.rs:586` | Deserialization unchanged. |
| 3 | `test_config_deserialize_full` | `bronze.rs:599` | Full deserialization unchanged. |
| 4 | `test_config_snapshot_interval_defaults_to_1800` | `bronze.rs:618` | Snapshot interval config unchanged. |
| 5 | `test_config_day_rollover_defaults_to_zero` | `bronze.rs:630` | Day rollover config unchanged. |
| 6 | `test_config_with_all_new_fields` | `bronze.rs:640` | All new fields config unchanged. |
| 7 | `test_config_backward_compatible_with_pre_air017_yaml` | `bronze.rs:655` | Backward compat unchanged. |

### 2.3 BronzeSubscriber Lifecycle Tests (kept unchanged, 3 tests)

| # | Test Name | Lines | Reason to Keep |
|---|-----------|-------|----------------|
| 1 | `test_subscriber_creation` | `bronze.rs:677` | Creation test. **Note:** Assert on `accumulator.count()` must be removed from the struct, but the rest (id, metrics, is_running) stays. See "Tests to Rewrite" for the needed changes. |
| 2 | `test_subscriber_creation_returns_core_result` | `bronze.rs:691` | Constructor returns CoreResult. Unchanged. |
| 3 | `test_subscriber_handles_lagged_error` | `bronze.rs:1067` | Lag handling unchanged. |

### 2.4 Stream Filter Tests (kept unchanged, 2 tests)

| # | Test Name | Lines | Reason to Keep |
|---|-----------|-------|----------------|
| 1 | `test_subscriber_accepts_all_streams_by_default` | `bronze.rs:715` | Filter logic unchanged. |
| 2 | `test_subscriber_filters_streams` | `bronze.rs:726` | Filter logic unchanged. |

### 2.5 Partition Path Tests (kept unchanged, 2 tests)

| # | Test Name | Lines | Reason to Keep |
|---|-----------|-------|----------------|
| 1 | `test_partition_path_computation` | `bronze.rs:886` | Path logic unchanged. |
| 2 | `test_partition_path_strips_protocol_suffix` | `bronze.rs:904` | Suffix stripping unchanged. |

### 2.6 Extract Stream ID Tests (kept unchanged, 1 test)

| # | Test Name | Lines | Reason to Keep |
|---|-----------|-------|----------------|
| 1 | `test_extract_stream_from_source_id` | `bronze.rs:950` | Helper function unchanged. |

---

## 3. Tests to REWRITE

These tests reference `accumulator.count()`, `accumulator.source_count()`, `accumulator.all_points_by_source()`, or `self.accumulator` in assertions. They must be rewritten to verify WAL-only behavior.

### 3.1 BronzeSubscriber Unit Tests (`bronze.rs` mod tests)

| # | Current Test Name | Lines | What Changes |
|---|-------------------|-------|-------------|
| 1 | `test_subscriber_creation` | `bronze.rs:677` | Remove assertion `assert_eq!(subscriber.accumulator.count(), 0)` (no accumulator field). Keep remaining assertions on id, metrics, is_running. May add assertion on WAL next_sequence == 1. |
| 2 | `test_handle_point_wal_then_accumulator` | `bronze.rs:742` | **Rename** to `test_handle_point_wal_append_only`. Remove `assert_eq!(subscriber.accumulator.count(), 1)`. Assert only: WAL `next_sequence == 2`, `events_received == 1`, `wal_errors == 0`. Verify WAL file contains 1 entry via `wal.replay_since(0).len() == 1`. |
| 3 | `test_handle_point_multiple_sources` | `bronze.rs:761` | **Rename** to `test_handle_point_multiple_sources_wal_only`. Remove `accumulator.count()` and `accumulator.source_count()` assertions. Assert: `wal.next_sequence() == 4`. Replay WAL and verify 3 entries with correct source_ids. |
| 4 | `test_filtered_points_not_in_wal_or_accumulator` | `bronze.rs:776` | Remove `assert_eq!(subscriber.accumulator.count(), 0)`. Assert: `wal.next_sequence() == 1` (WAL not advanced for filtered point), `events_received == 1`. |
| 5 | `test_snapshot_empty_is_noop` | `bronze.rs:798` | Rewrite: snapshot on empty WAL is a no-op. Assert `write_raw_snapshot` is NOT called, `snapshots_written == 0`. The check changes from `accumulator.count() == 0` to `wal.replay_since(0).is_empty()`. |
| 6 | `test_snapshot_writes_all_sources` | `bronze.rs:810` | Keep mock expectations (2 sources, `write_raw_snapshot` called twice). Remove accumulator interaction. handle_point feeds WAL only. Snapshot replays WAL, groups by source, writes Parquet. |
| 7 | `test_snapshot_advances_wal_watermark` | `bronze.rs:834` | Same logic applies. The snapshot still advances the watermark after successful writes. Remove accumulator references. **Note:** In the new design, snapshot does NOT truncate or advance watermark (WAL stays for next full-day write). This assertion may be REMOVED or changed depending on whether the fix keeps watermark advancement. See section 4 for the new snapshot contract. |
| 8 | `test_snapshot_failure_does_not_advance_watermark` | `bronze.rs:861` | Same structure. Remove accumulator references. Snapshot replays WAL, write fails, watermark stays at 0. |
| 9 | `test_health_check_not_running` | `bronze.rs:921` | Keep test. Change: `accumulator_count` key in health details is replaced with WAL-based metric (e.g., `wal_entry_count` or `wal_file_size_bytes`). |
| 10 | `test_health_check_includes_new_metrics` | `bronze.rs:933` | Change: Replace `assert!(health.details.contains_key("accumulator_count"))` with `assert!(health.details.contains_key("wal_entry_count"))` or similar. |

### 3.2 BronzeSubscriber Recovery Tests (`bronze.rs` mod tests)

All recovery tests currently seed/assert on the accumulator. In the new design, recovery is a no-op (or just logs WAL status), so these tests must change fundamentally.

| # | Current Test Name | Lines | What Changes |
|---|-------------------|-------|-------------|
| 11 | `test_recovery_empty_start` | `bronze.rs:1176` | Recovery is now a no-op. Test verifies: recover() returns Ok, no Parquet read attempted (`query_raw` expectation removed or times(0)). |
| 12 | `test_recovery_parquet_only` | `bronze.rs:1196` | **REMOVE entirely.** Recovery no longer reads Parquet into an accumulator. The WAL is the sole source of truth for snapshot. |
| 13 | `test_recovery_wal_only` | `bronze.rs:1226` | Simplify to: recover() returns Ok and logs WAL size. No accumulator assertions. The WAL entries are not loaded into memory; they will be replayed during next snapshot(). |
| 14 | `test_recovery_parquet_plus_wal` | `bronze.rs:1263` | **REMOVE entirely.** No Parquet + WAL merge logic exists in the new design. |
| 15 | `test_recovery_parquet_failure_falls_back_to_wal` | `bronze.rs:1317` | **REMOVE entirely.** Recovery does not read Parquet at all. |
| 16 | `test_recovery_called_before_select_loop` | `bronze.rs:1358` | Simplify: recovery still runs before the select loop but does minimal work (logs WAL info). Remove Parquet seed expectations and accumulator assertions. |

### 3.3 BronzeSubscriber Integration-Style Tests (`bronze.rs` mod tests)

| # | Current Test Name | Lines | What Changes |
|---|-------------------|-------|-------------|
| 17 | `test_subscriber_receives_and_processes_events` | `bronze.rs:976` | Remove `query_raw` recovery expectation (recovery no longer reads Parquet). Keep `write_raw_snapshot` expectation. Snapshot now replays WAL to build Parquet data. |
| 18 | `test_snapshot_timer_fires` | `bronze.rs:1023` | Same as above: remove recovery query_raw mock. Keep write_raw_snapshot mock. |
| 19 | `test_subscriber_final_snapshot_on_shutdown` | `bronze.rs:1087` | Remove recovery query_raw mock. Final snapshot replays WAL and writes Parquet. |

### 3.4 P1-10 Integration Tests (`bronze.rs` mod integration_tests)

| # | Current Test Name | Lines | What Changes |
|---|-------------------|-------|-------------|
| 20 | `test_integration_full_ingest_snapshot_cycle` | `bronze.rs:1492` | No accumulator references in assertions. Test already uses real ParquetStore. Only change: verify the snapshot is WAL-replay-driven (functionally equivalent output, implementation differs). May need to adjust recovery expectations if subscriber.start() no longer calls query_raw for recovery. |
| 21 | `test_integration_crash_recovery` | `bronze.rs:1572` | Major rewrite. Phase 2 (recovery): Instead of asserting `accumulator.count() == 15`, verify that after recovery + snapshot, Parquet contains all 15 points. The recovery no longer populates an accumulator; it ensures WAL integrity. The subsequent snapshot replays WAL and writes correct Parquet. |
| 22 | `test_integration_snapshot_overwrites_previous` | `bronze.rs:1680` | Remove `assert_eq!(subscriber.accumulator.count(), 20)`. Instead, verify WAL has 20 entries. Second snapshot replays full WAL (all 20 entries) and writes Parquet with 20 points. |
| 23 | `test_integration_multiple_streams_isolation` | `bronze.rs:1754` | Remove `accumulator.count()` and `accumulator.source_count()` assertions. Verify WAL has 16 total entries. Snapshot writes 3 separate Parquet files with correct counts (8, 5, 3). |

---

## 4. New Tests to ADD

### 4.1 WAL-Only Snapshot Flow

**BUG004-01: Snapshot replays WAL and writes Parquet (happy path)**
```
Arrange: BronzeSubscriber with MockRawStore. Feed 10 points from "air-quality-Mqtt"
         via handle_point (WAL only, no accumulator).
Act:     Call snapshot().
Assert:  write_raw_snapshot called once with Vec of 10 points and correct partition path.
         snapshots_written == 1, events_written == 10.
```

**BUG004-02: Empty WAL produces no-op snapshot**
```
Arrange: Fresh BronzeSubscriber, no points ingested.
Act:     Call snapshot().
Assert:  write_raw_snapshot NOT called. snapshots_written == 0.
```

**BUG004-03: WAL with multiple sources produces correct grouping**
```
Arrange: Feed 5 points from "air-quality-Mqtt", 3 from "outdoor-weather-Http",
         2 from "nws-forecast-HttpPoll" via handle_point.
Act:     Call snapshot().
Assert:  write_raw_snapshot called 3 times (once per source).
         Captured point vecs have lengths [5, 3, 2] respectively.
         Partition paths are distinct per stream.
```

**BUG004-04: Snapshot logs WAL size and entry count**
```
Arrange: Feed 20 points via handle_point. Capture tracing output.
Act:     Call snapshot().
Assert:  Log message contains: entry count (20), source count, elapsed time.
         (Use tracing_test or tracing-subscriber with in-memory writer.)
```

**BUG004-05: Snapshot does NOT truncate WAL (full-day retention)**
```
Arrange: Feed 10 points. WAL next_sequence == 11, watermark == 0.
Act:     Call snapshot().
Assert:  wal.replay_since(0).len() == 10 (entries still present).
         wal.current_watermark() == 0 (unchanged -- no commit_to called).
         This is the key behavioral change: the fix removes watermark advancement
         from snapshot so the WAL retains all day's data for subsequent snapshots.
```

**BUG004-06: Second snapshot replays full WAL (overwrites Parquet with all data)**
```
Arrange: Feed 10 points. Snapshot. Feed 10 more points (total 20 in WAL).
Act:     Call snapshot() again.
Assert:  Second write_raw_snapshot called with 20 points (not 10).
         Parquet is overwritten with the full day's data each time.
```

### 4.2 handle_point Is WAL-Only (No Memory Growth)

**BUG004-07: handle_point only writes to WAL, no memory accumulation**
```
Arrange: Fresh BronzeSubscriber.
Act:     Call handle_point 1000 times.
Assert:  WAL next_sequence == 1001.
         events_received == 1000.
         BronzeSubscriber struct size has NOT grown (no Vec/HashMap accumulating data).
         (The struct has no accumulator field to check; this is verified by compilation.)
```

**BUG004-08: handle_point with WAL failure increments wal_errors**
```
Arrange: BronzeSubscriber. Make WAL path read-only after construction (simulate I/O error).
         Note: This may require a test-only WAL stub or closing/reopening the file.
Act:     Call handle_point.
Assert:  wal_errors == 1. events_received == 1. No panic.
```

### 4.3 Day Rollover

**BUG004-09: Day rollover truncates WAL**
```
Arrange: Feed 50 points across the day. WAL has 50 entries.
Act:     Trigger day rollover (however the implementation exposes this).
Assert:  WAL is truncated (replay_since(0) returns empty or only new-day entries).
         Next snapshot for the new day starts from a clean WAL.
```

### 4.4 Recovery Is Minimal

**BUG004-10: Recovery is no-op (no Parquet read, no accumulator build)**
```
Arrange: Pre-populate WAL with 20 entries. Create BronzeSubscriber.
Act:     Call recover().
Assert:  recover() returns Ok(()).
         MockRawStore.query_raw is NOT called (no Parquet seeding).
         No in-memory data structure populated.
```

**BUG004-11: Recovery logs WAL existence and size**
```
Arrange: Pre-populate WAL with 20 entries. Capture tracing output.
Act:     Call recover().
Assert:  Log message contains WAL entry count or file size.
```

### 4.5 Crash Recovery End-to-End

**BUG004-12: Crash recovery -- WAL survives restart, next snapshot writes correct Parquet**
```
Arrange: Phase 1 -- Create BronzeSubscriber, feed 10 points via handle_point,
         call snapshot() (Parquet has 10 points), feed 5 more points.
         Drop subscriber (simulates crash -- 5 unsnapshot'd entries in WAL).
         Phase 2 -- Create new BronzeSubscriber on same WAL path and data_dir.
Act:     Call recover(), then feed 0 new points, call snapshot().
Assert:  Parquet file now contains 15 points (WAL had all 15 entries because
         snapshot did not truncate; new subscriber replays full WAL).
```

**BUG004-13: Crash recovery -- WAL with corrupted trailing entry**
```
Arrange: Feed 10 points. Manually append garbage to WAL file (simulates crash
         mid-write). Create new BronzeSubscriber on same WAL path.
Act:     Call snapshot().
Assert:  Parquet contains 10 valid points. Corrupted entry skipped with warning.
```

### 4.6 Health Check Updates

**BUG004-14: Health check reports WAL metrics instead of accumulator count**
```
Arrange: Feed 15 points via handle_point.
Act:     Call health_check().
Assert:  details does NOT contain "accumulator_count".
         details contains WAL-based metric (e.g., "wal_entry_count" or "wal_file_size_bytes").
         events_received == "15".
```

### 4.7 Struct Compilation Checks

**BUG004-15: BronzeSubscriber does not have an accumulator field**
```
This is a compile-time check, not a runtime test. Verified by:
1. Removing `use crate::storage::accumulator::Accumulator;` from bronze.rs.
2. Removing the `accumulator: Accumulator` field from BronzeSubscriber struct.
3. Successful compilation proves the field is gone.
```

---

## 5. Integration Test Updates (P1-10 Tests)

The 4 integration tests in `bronze.rs mod integration_tests` need these changes:

### INT-01: `test_integration_full_ingest_snapshot_cycle`

**Current behavior:** Feeds events, waits for snapshot timer, verifies Parquet files exist and contain correct data.
**Change needed:** Recovery phase (inside `subscriber.start()`) will no longer call `store.query_raw`. Since this test uses real ParquetStore, there is no mock to adjust -- but the test must verify that snapshot correctly replays WAL data. The assertions on Parquet content (20 points, correct source_id, correct payload fields) remain valid.
**Verdict:** Likely works as-is once the implementation compiles. Verify.

### INT-04: `test_integration_crash_recovery`

**Current behavior:** Phase 1 writes 10 events + snapshot + 5 more events + crash. Phase 2 constructs new subscriber, calls `recover()`, asserts `accumulator.count() == 15`, then snapshots and verifies Parquet has 15 points.
**Change needed:**
- Remove `assert_eq!(subscriber.accumulator.count(), 15)` and `subscriber.accumulator.source_count()` assertions.
- After recovery, trigger a snapshot. Verify Parquet contains 15 points.
- Recovery itself does nothing visible; the WAL contains all 15 entries (because snapshot no longer truncates WAL). The snapshot replays all 15 entries and writes them to Parquet.

### INT-02: `test_integration_snapshot_overwrites_previous`

**Current behavior:** Feeds 10 events, snapshots, feeds 10 more, snapshots again. Asserts `accumulator.count() == 20` between snapshots. Asserts Parquet has 20 points after second snapshot.
**Change needed:**
- Remove `assert_eq!(subscriber.accumulator.count(), 20)`.
- Verify WAL has 20 entries (via `wal.next_sequence() == 21`).
- Second snapshot replays full WAL (20 entries) and writes Parquet with 20 points. Assertions on Parquet content stay.

### INT-06: `test_integration_multiple_streams_isolation`

**Current behavior:** Feeds 16 events across 3 sources. Asserts `accumulator.count() == 16` and `accumulator.source_count() == 3`. Snapshots and verifies 3 separate Parquet files.
**Change needed:**
- Remove `accumulator.count()` and `accumulator.source_count()` assertions.
- Verify WAL next_sequence == 17 (16 entries + initial 1).
- Keep all Parquet verification assertions (file counts, point counts, source_id correctness).

---

## 6. Memory Verification

### 6.1 Goal

Prove that RSS does not grow linearly with ingested data volume after the fix.

### 6.2 Approach: RSS Tracking Test

```rust
#[tokio::test]
#[ignore] // Requires real timing, run with --ignored
async fn test_bug004_no_memory_leak() {
    // Read RSS before ingestion
    let rss_before = read_current_rss_kb();

    // Feed 50,000 points (simulating a full day's load)
    for i in 0..50_000 {
        subscriber.handle_point(Arc::new(gen_point("air-quality-Mqtt", base, i)));
    }

    // Read RSS after ingestion
    let rss_after = read_current_rss_kb();

    // RSS growth should be minimal (WAL is on disk, not memory)
    // Allow 5 MiB growth for WAL file handles, buffers, etc.
    let growth_kb = rss_after.saturating_sub(rss_before);
    assert!(
        growth_kb < 5_000,
        "RSS grew by {} KiB after 50K points -- possible memory leak",
        growth_kb
    );
}

fn read_current_rss_kb() -> u64 {
    // Linux: parse /proc/self/status VmRSS
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            return parts[1].parse().unwrap_or(0);
        }
    }
    0
}
```

### 6.3 Before/After Comparison Matrix

| Metric | Before Fix (accumulator) | After Fix (WAL-only) | How to Measure |
|--------|-------------------------|---------------------|----------------|
| RSS after 10K points | ~10-15 MiB above baseline | ~0-2 MiB above baseline | `/proc/self/status` VmRSS |
| RSS after 50K points | ~50-60 MiB above baseline | ~0-2 MiB above baseline | `/proc/self/status` VmRSS |
| RSS growth rate | Linear O(n) | Constant O(1) | Plot RSS vs point count |
| Disk usage growth | WAL + accumulator mirror | WAL only (expected, bounded) | `du -h` on WAL file |
| Snapshot memory spike | ~2x accumulator (clone) | WAL replay Vec (transient) | Peak VmRSS during snapshot |

### 6.4 CI Integration

Add a benchmark test (marked `#[ignore]`) that:
1. Feeds N points with periodic snapshots.
2. Asserts RSS < threshold at end of run.
3. Run on Pi hardware or Linux CI with `/proc/self/status` available.

---

## 7. Test Execution Summary

| Category | Count Before | Count After | Net Change |
|----------|-------------|-------------|------------|
| Accumulator unit tests (REMOVED) | 15 | 0 | -15 |
| WAL unit tests (KEPT) | 21 | 21 | 0 |
| Bronze config tests (KEPT) | 7 | 7 | 0 |
| Bronze lifecycle tests (KEPT/MINOR EDIT) | 3 | 3 | 0 |
| Bronze stream filter tests (KEPT) | 2 | 2 | 0 |
| Bronze partition path tests (KEPT) | 2 | 2 | 0 |
| Bronze extract stream id tests (KEPT) | 1 | 1 | 0 |
| Bronze handle_point tests (REWRITTEN) | 3 | 3 | 0 |
| Bronze snapshot tests (REWRITTEN) | 4 | 4 | 0 |
| Bronze health check tests (REWRITTEN) | 2 | 2 | 0 |
| Bronze recovery tests (REWRITTEN/REMOVED) | 6 | 2 | -4 |
| Bronze integration-style tests (REWRITTEN) | 3 | 3 | 0 |
| P1-10 integration tests (REWRITTEN) | 4 | 4 | 0 |
| **New BUG-004 tests (ADDED)** | 0 | 15 | +15 |
| **TOTAL** | 73 | 69 | -4 |

The net loss of 4 tests comes from removing 6 recovery tests that tested Parquet-seed-into-accumulator logic (no longer applicable) and removing 15 accumulator tests, offset by adding 15 new WAL-only-snapshot tests.

---

## 8. Test Priority Order

Implement in this order during the fix:

1. **Compile check** -- Remove accumulator from struct, verify compilation.
2. **BUG004-07** -- handle_point is WAL-only (core behavioral change).
3. **BUG004-01** -- Snapshot replays WAL and writes Parquet (core behavioral change).
4. **BUG004-02** -- Empty WAL no-op snapshot.
5. **BUG004-05** -- Snapshot does NOT truncate WAL.
6. **BUG004-03** -- Multi-source grouping from WAL replay.
7. **BUG004-06** -- Second snapshot replays full WAL.
8. **BUG004-10** -- Recovery is no-op.
9. **BUG004-12** -- Crash recovery end-to-end.
10. **BUG004-14** -- Health check updates.
11. Rewrite existing tests (section 3).
12. Update integration tests (section 5).
13. **BUG004-09** -- Day rollover truncation (if implementing in this fix).
14. Memory verification test (section 6, `#[ignore]`).
