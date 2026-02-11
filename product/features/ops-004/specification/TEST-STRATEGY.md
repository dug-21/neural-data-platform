# ops-004 Test Strategy: BUG-005 Memory Instrumentation

**Feature**: Memory diagnostic instrumentation for air-quality-app
**Test Approach**: Unit tests for all new code; integration tests for log output; soak test criteria for acceptance.

---

## Test Categories Overview

| Category | Count | Target | Run With |
|----------|-------|--------|----------|
| Unit: MemoryDiagnostics | 8 | `core/src/diagnostics/memory.rs` | `cargo test` |
| Unit: /proc parsers | 7 | `core/src/diagnostics/memory.rs` | `cargo test` |
| Unit: mallinfo2 FFI | 3 | `core/src/diagnostics/memory.rs` | `cargo test` |
| Unit: Accumulator capacity | 5 | `core/src/storage/accumulator.rs` | `cargo test` |
| Unit: MemoryTrend | 6 | `core/src/diagnostics/memory.rs` | `cargo test` |
| Integration: heartbeat format | 3 | `core/src/subscribers/bronze.rs` | `cargo test` |
| Integration: per-source delta | 2 | `core/src/subscribers/bronze.rs` | `cargo test` |
| Acceptance: soak test criteria | - | Manual / CI | 48h run on Pi |

**Total unit/integration tests**: ~34

---

## 1. Unit Tests: MemoryDiagnostics Struct

Location: `core/src/diagnostics/memory.rs` (inline `#[cfg(test)] mod tests`)

### T-01: MemoryDiagnostics::collect with real accumulator

```
#[test]
fn test_diagnostics_collect_empty_accumulator() {
    // Arrange: empty Accumulator
    // Act: MemoryDiagnostics::collect(&accumulator)
    // Assert:
    //   - accumulator_count == 0
    //   - accumulator_source_count == 0
    //   - accumulator_vec_capacity_sum == 0
    //   - accumulator_vec_len_sum == 0
    //   - accumulator_estimate_bytes > 0 (HashMap overhead)
    //   - sampled_at is recent (within 1 second)
}
```

### T-02: MemoryDiagnostics::collect with populated accumulator

```
#[test]
fn test_diagnostics_collect_populated_accumulator() {
    // Arrange: Accumulator with 3 sources, 10 points each
    // Act: MemoryDiagnostics::collect(&accumulator)
    // Assert:
    //   - accumulator_count == 30
    //   - accumulator_source_count == 3
    //   - accumulator_vec_len_sum == 30
    //   - accumulator_vec_capacity_sum >= 30 (Vec over-allocates)
    //   - accumulator_capacity >= 3
    //   - accumulator_estimate_bytes > 0
}
```

### T-03: rss_mib_display formats correctly

```
#[test]
fn test_rss_mib_display_some() {
    // Arrange: MemoryDiagnostics with rss_bytes = Some(104_857_600) (100 MiB)
    // Assert: rss_mib_display() == "100.0"
}

#[test]
fn test_rss_mib_display_none() {
    // Arrange: MemoryDiagnostics with rss_bytes = None
    // Assert: rss_mib_display() == "N/A"
}
```

### T-04: unaccounted_bytes computation

```
#[test]
fn test_unaccounted_bytes_positive_gap() {
    // Arrange: rss_bytes = Some(200 * 1024 * 1024), accumulator_estimate = 5 * 1024 * 1024
    // Assert: unaccounted_bytes() == Some(195 * 1024 * 1024)
}

#[test]
fn test_unaccounted_bytes_none_rss() {
    // Arrange: rss_bytes = None
    // Assert: unaccounted_bytes() == None
}

#[test]
fn test_unaccounted_bytes_accumulator_larger_than_rss() {
    // Edge case: should produce negative value (theoretically impossible but safe)
    // Arrange: rss_bytes = Some(1024), accumulator_estimate = 2048
    // Assert: unaccounted_bytes() == Some(-1024)
}
```

### T-05: format_opt_mib helper

```
#[test]
fn test_format_opt_mib_some() {
    assert_eq!(format_opt_mib(Some(10_485_760)), "10.0")
}

#[test]
fn test_format_opt_mib_none() {
    assert_eq!(format_opt_mib(None), "N/A")
}
```

---

## 2. Unit Tests: /proc Parsers

Location: `core/src/diagnostics/memory.rs` (inline tests)

These tests use synthetic file content, not actual `/proc` reads, to ensure deterministic behavior across platforms.

### T-06: parse_kb_value

```
#[test]
fn test_parse_kb_value_standard() {
    assert_eq!(parse_kb_value("Rss:       1024 kB"), Some(1024))
}

#[test]
fn test_parse_kb_value_zero() {
    assert_eq!(parse_kb_value("Rss:          0 kB"), Some(0))
}

#[test]
fn test_parse_kb_value_large() {
    assert_eq!(parse_kb_value("Rss:     524288 kB"), Some(524288))
}

#[test]
fn test_parse_kb_value_malformed() {
    assert_eq!(parse_kb_value("Rss: invalid kB"), None)
}
```

### T-07: read_proc_status_rss_bytes with synthetic content

Refactor the parser to accept a string (for testability), with a public function that reads the file:

```
// Production code structure:
fn parse_proc_status_rss_bytes(content: &str) -> Option<u64> { ... }
pub fn read_proc_status_rss_bytes() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_proc_status_rss_bytes(&content)
}

// Tests:
#[test]
fn test_parse_proc_status_typical() {
    let content = "\
VmPeak:   300000 kB
VmSize:   280000 kB
VmRSS:    104000 kB
VmData:   200000 kB";

    assert_eq!(parse_proc_status_rss_bytes(content), Some(104000 * 1024))
}

#[test]
fn test_parse_proc_status_no_vmrss() {
    let content = "VmPeak: 300000 kB\nVmSize: 280000 kB\n";
    assert_eq!(parse_proc_status_rss_bytes(content), None)
}
```

### T-08: read_proc_smaps_summary with synthetic content

```
// Production code structure:
fn parse_proc_smaps_summary(content: &str) -> Option<SmapsSummary> { ... }

#[test]
fn test_parse_smaps_heap_mapping() {
    let content = "\
55a000000000-55a000100000 rw-p 00000000 00:00 0          [heap]
Size:               1024 kB
Rss:                 512 kB
Pss:                 512 kB
";
    let summary = parse_proc_smaps_summary(content).unwrap();
    assert_eq!(summary.heap_rss_bytes, 512 * 1024);
}

#[test]
fn test_parse_smaps_stack_mapping() {
    let content = "\
7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0          [stack]
Size:                132 kB
Rss:                  20 kB
";
    let summary = parse_proc_smaps_summary(content).unwrap();
    assert_eq!(summary.stack_rss_bytes, 20 * 1024);
}

#[test]
fn test_parse_smaps_mixed_mappings() {
    let content = "\
55a000000000-55a000100000 rw-p 00000000 00:00 0          [heap]
Size:               1024 kB
Rss:                 512 kB
7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0          [stack]
Size:                132 kB
Rss:                  20 kB
7f0000000000-7f0000010000 rw-p 00000000 00:00 0
Size:                 64 kB
Rss:                  32 kB
7f0000100000-7f0000200000 r--p 00000000 08:01 12345      /usr/lib/libc.so.6
Size:               1024 kB
Rss:                 800 kB
";
    let summary = parse_proc_smaps_summary(content).unwrap();
    assert_eq!(summary.heap_rss_bytes, 512 * 1024);
    assert_eq!(summary.stack_rss_bytes, 20 * 1024);
    assert_eq!(summary.anon_rss_bytes, 32 * 1024);    // unnamed, non-file
    assert_eq!(summary.file_rss_bytes, 800 * 1024);   // libc.so
    assert_eq!(summary.total_rss_bytes, (512 + 20 + 32 + 800) * 1024);
}

#[test]
fn test_parse_smaps_empty() {
    let summary = parse_proc_smaps_summary("").unwrap();
    assert_eq!(summary.total_rss_bytes, 0);
}
```

---

## 3. Unit Tests: mallinfo2 FFI Wrapper

Location: `core/src/diagnostics/memory.rs` (inline tests)

### T-09: mallinfo2 on Linux returns Some

```
#[test]
#[cfg(target_os = "linux")]
fn test_mallinfo2_returns_some_on_linux() {
    let stats = read_mallinfo2();
    assert!(stats.is_some());
    let stats = stats.unwrap();
    // arena should be > 0 (process has allocated memory)
    assert!(stats.arena > 0);
    // uordblks should be > 0 (some memory is in use)
    assert!(stats.uordblks > 0);
    // Sanity: arena >= uordblks (can't use more than allocated)
    assert!(stats.arena >= stats.uordblks);
}
```

### T-10: mallinfo2 on non-Linux returns None

```
#[test]
#[cfg(not(target_os = "linux"))]
fn test_mallinfo2_returns_none_on_non_linux() {
    let stats = read_mallinfo2();
    assert!(stats.is_none());
}
```

### T-11: MallocStats sanity after allocation

```
#[test]
#[cfg(target_os = "linux")]
fn test_mallinfo2_reflects_allocation() {
    let before = read_mallinfo2().unwrap();

    // Allocate a known chunk
    let _blob: Vec<u8> = vec![0u8; 1_048_576]; // 1 MiB

    let after = read_mallinfo2().unwrap();

    // uordblks should grow by approximately 1 MiB (may not be exact due to
    // allocator overhead and concurrent allocations)
    assert!(
        after.uordblks > before.uordblks,
        "uordblks should grow after 1 MiB allocation: before={}, after={}",
        before.uordblks, after.uordblks
    );
}
```

---

## 4. Unit Tests: Accumulator Capacity Methods

Location: `core/src/storage/accumulator.rs` (add to existing `#[cfg(test)] mod tests`)

### T-12: hash_capacity on empty accumulator

```
#[test]
fn test_hash_capacity_empty() {
    let acc = Accumulator::new(Utc::now().date_naive());
    // Empty HashMap has capacity 0
    assert_eq!(acc.hash_capacity(), 0);
}
```

### T-13: hash_capacity grows with sources

```
#[test]
fn test_hash_capacity_grows_with_sources() {
    let mut acc = Accumulator::new(Utc::now().date_naive());
    let ts = Utc::now();

    for i in 0..10 {
        acc.add(make_point(&format!("source-{}", i), ts));
    }

    // HashMap capacity >= number of keys (10)
    assert!(acc.hash_capacity() >= 10);
    // HashMap typically over-allocates (power of 2)
    // With 10 keys, capacity is likely 16 or more
}
```

### T-14: vec_capacity vs vec_len

```
#[test]
fn test_vec_capacity_ge_vec_len() {
    let mut acc = Accumulator::new(Utc::now().date_naive());
    let ts = Utc::now();

    // Add 50 points to single source (Vec will over-allocate)
    for i in 0..50 {
        acc.add(make_point("source-a", ts + chrono::Duration::seconds(i)));
    }

    assert_eq!(acc.vec_len(), 50);
    assert!(acc.vec_capacity() >= 50);
    // Vec typically doubles: capacity is likely 64
}
```

### T-15: vec_capacity across multiple sources

```
#[test]
fn test_vec_capacity_multiple_sources() {
    let mut acc = Accumulator::new(Utc::now().date_naive());
    let ts = Utc::now();

    // 3 sources, 10 points each
    for s in 0..3 {
        for i in 0..10 {
            acc.add(make_point(&format!("source-{}", s), ts + chrono::Duration::seconds(i)));
        }
    }

    assert_eq!(acc.vec_len(), 30);
    // Each Vec has capacity >= 10, so total >= 30
    assert!(acc.vec_capacity() >= 30);
    // The waste is capacity - len
    let waste = acc.vec_capacity() - acc.vec_len();
    // Waste exists because Vec growth is exponential
    // (not asserting specific waste, just that it compiles and is >= 0)
    assert!(waste < acc.vec_capacity()); // tautological but documents intent
}
```

### T-16: vec_capacity after clear

```
#[test]
fn test_vec_capacity_after_clear() {
    let mut acc = Accumulator::new(Utc::now().date_naive());
    let ts = Utc::now();

    for i in 0..100 {
        acc.add(make_point("src", ts + chrono::Duration::seconds(i)));
    }

    assert!(acc.vec_capacity() >= 100);

    acc.clear();

    // After clear, HashMap is empty, so vec capacity sum is 0
    assert_eq!(acc.vec_capacity(), 0);
    assert_eq!(acc.vec_len(), 0);
    assert_eq!(acc.hash_capacity(), 0);
}
```

---

## 5. Unit Tests: MemoryTrend

Location: `core/src/diagnostics/memory.rs` (inline tests)

### T-17: empty trend returns None

```
#[test]
fn test_trend_empty_returns_none() {
    let trend = MemoryTrend::new(10);
    assert_eq!(trend.growth_rate_bytes_per_hour(), None);
    assert_eq!(trend.len(), 0);
}
```

### T-18: single sample returns None

```
#[test]
fn test_trend_single_sample_returns_none() {
    let mut trend = MemoryTrend::new(10);
    trend.record(100_000_000);
    assert_eq!(trend.growth_rate_bytes_per_hour(), None);
    assert_eq!(trend.len(), 1);
}
```

### T-19: two samples computes rate

```
#[test]
fn test_trend_two_samples_computes_rate() {
    let mut trend = MemoryTrend::new(10);

    // Manually inject samples with known timestamps for determinism.
    // Use the internal push method or refactor to accept (DateTime, u64).
    // For testability, add: pub fn record_at(&mut self, ts: DateTime<Utc>, rss: u64)

    let t0 = Utc::now();
    let t1 = t0 + chrono::Duration::hours(1);

    trend.record_at(t0, 100_000_000);   // 100 MB
    trend.record_at(t1, 110_000_000);   // 110 MB, 1 hour later

    let rate = trend.growth_rate_bytes_per_hour().unwrap();
    // Expected: 10_000_000 bytes/hour
    assert!((rate - 10_000_000.0).abs() < 100.0,
        "Expected ~10MB/hr, got {:.0}", rate);
}
```

### T-20: ring buffer eviction

```
#[test]
fn test_trend_ring_buffer_evicts_oldest() {
    let mut trend = MemoryTrend::new(3); // max 3 samples

    let t0 = Utc::now();
    trend.record_at(t0, 100);
    trend.record_at(t0 + chrono::Duration::seconds(1), 200);
    trend.record_at(t0 + chrono::Duration::seconds(2), 300);
    assert_eq!(trend.len(), 3);

    // Fourth sample evicts the first
    trend.record_at(t0 + chrono::Duration::seconds(3), 400);
    assert_eq!(trend.len(), 3);

    // Rate should be based on samples at t+1..t+3 (200, 300, 400)
    // Not t+0..t+3
}
```

### T-21: negative growth rate (RSS decreasing)

```
#[test]
fn test_trend_negative_growth() {
    let mut trend = MemoryTrend::new(10);

    let t0 = Utc::now();
    trend.record_at(t0, 200_000_000);
    trend.record_at(t0 + chrono::Duration::hours(1), 150_000_000);

    let rate = trend.growth_rate_bytes_per_hour().unwrap();
    assert!(rate < 0.0, "Rate should be negative when RSS decreases");
    assert!((rate + 50_000_000.0).abs() < 100.0);
}
```

### T-22: zero-duration samples returns None

```
#[test]
fn test_trend_zero_duration_returns_none() {
    let mut trend = MemoryTrend::new(10);
    let t0 = Utc::now();
    // Two samples at the exact same timestamp
    trend.record_at(t0, 100);
    trend.record_at(t0, 200);
    // Duration is 0 -> cannot compute rate
    assert_eq!(trend.growth_rate_bytes_per_hour(), None);
}
```

---

## 6. Integration Tests: Heartbeat Format

Location: `core/src/subscribers/bronze.rs` (add to existing `#[cfg(test)] mod tests`)

These tests verify the enhanced heartbeat emits the expected tracing fields. They use `tracing_subscriber` with an in-memory layer to capture log output.

### T-23: heartbeat contains new capacity fields

```
#[tokio::test]
async fn test_heartbeat_contains_capacity_fields() {
    // Arrange:
    //   - Create BronzeSubscriber with MockRawStore
    //   - Configure flush_interval_secs = 1
    //   - Set up tracing capture layer
    //   - Feed 5 events from 2 sources
    //
    // Act:
    //   - Start subscriber, wait >1s for heartbeat, cancel
    //
    // Assert captured log contains:
    //   - "Heartbeat" message
    //   - accum_hash_capacity field present
    //   - accum_vec_capacity field present
    //   - accum_vec_len field present
    //   - accum_waste_ratio field present
    //   - All existing fields still present (backward compat)
}
```

### T-24: heartbeat allocator fields present on Linux

```
#[tokio::test]
#[cfg(target_os = "linux")]
async fn test_heartbeat_allocator_fields_on_linux() {
    // Same setup as T-23
    // Assert captured log contains:
    //   - arena_mib field (not "N/A")
    //   - uordblks_mib field (not "N/A")
    //   - fordblks_mib field (not "N/A")
    //   - heap_rss_mib field (not "N/A")
    //   - unaccounted_mib field (not "N/A")
}
```

### T-25: heartbeat backward compatible with existing fields

```
#[tokio::test]
async fn test_heartbeat_backward_compatible() {
    // Assert that the heartbeat log line still contains these exact field names:
    //   - accumulator_count
    //   - accumulator_mib
    //   - wal_mib
    //   - rss_mib
    //   - wal_errors
    //   - events_received
    //
    // These field names MUST NOT change to avoid breaking log parsers.
}
```

---

## 7. Integration Tests: Per-Source Snapshot Delta

Location: `core/src/subscribers/bronze.rs` integration_tests module

### T-26: per-source delta logged for each source

```
#[tokio::test]
async fn test_snapshot_per_source_delta_logged() {
    // Arrange:
    //   - Create BronzeSubscriber with real ParquetStore (tempdir)
    //   - Feed events from 3 sources
    //
    // Act:
    //   - Call snapshot()
    //
    // Assert captured log contains:
    //   - 3 "Snapshot per-source memory delta" messages
    //   - Each message has source_id, points, rss_delta_mib fields
    //   - Source IDs match the 3 expected sources
}
```

### T-27: per-source delta includes point counts

```
#[tokio::test]
async fn test_snapshot_per_source_point_counts() {
    // Arrange:
    //   - Source A: 10 points
    //   - Source B: 5 points
    //   - Source C: 3 points
    //
    // Act: snapshot()
    //
    // Assert:
    //   - Source A delta log shows points=10
    //   - Source B delta log shows points=5
    //   - Source C delta log shows points=3
}
```

---

## 8. Mocking Strategy

### /proc filesystem mocking

All `/proc` parsers are split into two functions:
1. `parse_*()` -- accepts `&str`, fully unit-testable with synthetic content
2. `read_*()` -- reads the file, calls `parse_*()`, tested only on Linux CI

This avoids needing filesystem mocking while keeping parser logic fully tested.

### mallinfo2 mocking

`read_mallinfo2()` is not mocked. Instead:
- On Linux: tests call the real function and assert sanity (arena > 0)
- On non-Linux: tests assert the function returns `None`
- The `#[cfg(target_os)]` attribute handles platform dispatch at compile time

### MockRawStore for snapshot tests

Existing `MockRawStore` (from `mockall`) is used for all Bronze subscriber tests:
- `expect_write_raw_snapshot()` -- verify writes happen
- `expect_query_raw()` -- control recovery behavior

No new mock traits are needed.

### Tracing capture for log assertion

Use `tracing_subscriber::fmt::TestWriter` or a custom `Layer` that captures events into a `Vec<String>`:

```rust
use tracing_subscriber::layer::SubscriberExt;
use std::sync::{Arc, Mutex};

struct CaptureLayer {
    events: Arc<Mutex<Vec<String>>>,
}

// Implement tracing_subscriber::Layer to capture event messages
// Assert on captured events after test execution
```

This approach is non-invasive -- no production code changes needed for testability.

---

## 9. Acceptance Criteria: Soak Test

These are NOT automated tests. They define criteria for manual validation on the Pi 5 deployment.

### Soak-01: 48-hour RSS stability

| Metric | Threshold | Source |
|--------|-----------|--------|
| RSS at 48h | < 256 MiB | Heartbeat log `rss_mib` |
| Container memory at 48h | < 300 MiB | `docker stats` |
| RSS growth rate | < 5 MiB/hour | Trend summary log `rss_growth_mib_per_hour` |
| Accumulator waste ratio | < 3.0 | Heartbeat log `accum_waste_ratio` |

### Soak-02: Instrumentation overhead

| Metric | Threshold | How to measure |
|--------|-----------|----------------|
| Heartbeat latency | < 1 ms | Timestamp delta around diagnostic collection |
| Snapshot overhead from per-source RSS reads | < 100 ms total | 8 sources * ~10us per `/proc` read |
| No new allocations from instrumentation | 0 heap growth | mallinfo2 before/after diagnostic collection |

### Soak-03: Log volume

| Metric | Threshold | Rationale |
|--------|-----------|-----------|
| Heartbeat log rate | 1 per 5 seconds (existing) | No change from current |
| Per-source delta logs | 8 per snapshot (debug level) | Only at debug, not info |
| Trend summary | 1 per 5 hours | Every 10 snapshots |
| Subsystem delta logs (>1 MiB) | < 5 per hour | Threshold filters noise |

### Soak-04: Data attribution target

After 48-hour soak, the instrumentation should attribute at least 80% of RSS to known categories:

| Category | Expected Range | Source Field |
|----------|---------------|--------------|
| Accumulator (len-based) | 4-8 MiB | `accumulator_mib` |
| Accumulator waste (capacity - len) | 1-4 MiB | `accum_vec_capacity - accum_vec_len` |
| Heap (allocator arena) | 20-100 MiB | `arena_mib` |
| Allocator free lists | 5-50 MiB | `fordblks_mib` |
| File-backed mappings | 10-30 MiB | smaps `file_rss` |
| Anonymous non-heap | variable | smaps `anon_rss` |
| **Unattributed** | **< 20% of RSS** | `unaccounted_mib` |

If unattributed exceeds 20% at 48h, the next phase (ops-005 or BUG-005 Phase 2) will add deeper instrumentation (e.g., jemalloc profiling, per-allocation tracking).

---

## 10. Test Execution Plan

### Phase 1: Unit tests (before merge)

```bash
# Run all unit tests including new diagnostics module
cargo test -p platform-core

# Run specific test groups
cargo test -p platform-core diagnostics::memory
cargo test -p platform-core storage::accumulator::tests::test_hash_capacity
cargo test -p platform-core storage::accumulator::tests::test_vec_capacity
```

### Phase 2: Integration tests (before merge)

```bash
# Run bronze subscriber integration tests
cargo test -p platform-core integration_tests

# Run with log output to verify format
cargo test -p platform-core test_heartbeat -- --nocapture
```

### Phase 3: Linux-specific tests (CI or Pi)

```bash
# mallinfo2 and /proc tests only work on Linux
cargo test -p platform-core test_mallinfo2 -- --nocapture
cargo test -p platform-core test_parse_smaps -- --nocapture
```

### Phase 4: Soak test (post-deploy)

```bash
# Deploy to Pi, run for 48h, then analyze logs:
docker logs air-quality-app 2>&1 | grep "Heartbeat" | tail -20
docker logs air-quality-app 2>&1 | grep "Memory trend summary"
docker stats --no-stream air-quality-app
```

---

## 11. Coverage Targets

| Module | Target | Rationale |
|--------|--------|-----------|
| `diagnostics::memory` | 90% | Core instrumentation, must be correct |
| `storage::accumulator` (new methods) | 100% | 3 simple methods, easy to cover |
| `subscribers::bronze` (enhanced heartbeat) | 70% | Integration-heavy, harder to unit test |
| `subscribers::bronze` (per-source delta) | 70% | Integration-heavy |

### Lines excluded from coverage

- `#[cfg(not(target_os = "linux"))]` branches on Linux CI (dead code on the platform)
- `read_*()` functions that read `/proc` files (tested via `parse_*()` split)
- `unsafe { mallinfo2() }` block (tested by sanity assertions, not line coverage)

---

## 12. Risk Matrix

| Risk | Impact | Mitigation |
|------|--------|------------|
| `/proc` reads add overhead | Low (5-10us each) | Benchmark in soak test |
| `mallinfo2` not available on older glibc | Medium | `#[cfg]` guard + runtime fallback to `None` |
| Tracing capture flaky in async tests | Low | Use deterministic tracing subscriber, avoid timing |
| smaps parser incorrect for unusual mappings | Low | Test with multiple synthetic layouts |
| Accumulator `capacity()` returns 0 before first insert | None | Documented in T-12, matches HashMap semantics |
| New heartbeat fields break log ingestion | Low | Existing fields unchanged; new fields are additive |
