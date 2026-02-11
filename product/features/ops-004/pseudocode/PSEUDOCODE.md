# ops-004 Pseudocode: BUG-005 Memory Instrumentation

**Feature**: Memory diagnostic instrumentation for air-quality-app
**Goal**: Instrument, not fix. Attribute the ~121 MiB RSS gap between `accumulator_mib` (4-8 MiB) and observed RSS (104-229 MiB) during 13.5h runs on Pi 5.

---

## P-01: MemoryDiagnostics Struct

Central struct for collecting all memory statistics from a single sample. Lives in a new module `core/src/diagnostics/memory.rs`.

```
// core/src/diagnostics/memory.rs

/// Snapshot of process memory state at a single point in time.
/// All sizes in bytes unless field name has _mib suffix.
pub struct MemoryDiagnostics {
    /// When this sample was taken
    sampled_at: DateTime<Utc>,

    // -- Process-level (from /proc/self/statm or /proc/self/status) --
    rss_bytes: Option<u64>,          // VmRSS from /proc/self/status

    // -- Allocator-level (from libc::mallinfo2 on Linux) --
    arena_bytes: Option<u64>,        // mallinfo2.arena: non-mmapped space allocated
    ordblks: Option<u64>,            // mallinfo2.ordblks: free chunks
    hblkhd_bytes: Option<u64>,       // mallinfo2.hblkhd: space in mmapped regions
    uordblks_bytes: Option<u64>,     // mallinfo2.uordblks: total allocated space
    fordblks_bytes: Option<u64>,     // mallinfo2.fordblks: total free space

    // -- /proc/self/smaps aggregates --
    heap_rss_bytes: Option<u64>,     // RSS of [heap] mapping
    stack_rss_bytes: Option<u64>,    // RSS of [stack] mapping
    anon_rss_bytes: Option<u64>,     // sum of all anonymous (non-file-backed) RSS

    // -- Accumulator-level --
    accumulator_count: usize,                // total points
    accumulator_source_count: usize,         // distinct source_ids
    accumulator_capacity: usize,             // HashMap allocated capacity (buckets)
    accumulator_vec_capacity_sum: usize,     // sum of capacity() for all inner Vecs
    accumulator_vec_len_sum: usize,          // sum of len() for all inner Vecs
    accumulator_estimate_bytes: usize,       // existing memory_estimate_bytes()
}

impl MemoryDiagnostics {
    /// Collect a full diagnostic snapshot.
    /// Called from heartbeat timer and snapshot boundaries.
    pub fn collect(accumulator: &Accumulator) -> Self {
        let sampled_at = Utc::now()

        // P-05: parse /proc/self/status for VmRSS
        let rss_bytes = read_proc_status_rss_bytes()

        // P-06: call mallinfo2() via FFI
        let alloc_stats = read_mallinfo2()

        // P-05: parse /proc/self/smaps for heap/stack/anon
        let smaps = read_proc_smaps_summary()

        // P-05b: accumulator capacity metrics
        let accumulator_count = accumulator.count()
        let accumulator_source_count = accumulator.source_count()
        let accumulator_capacity = accumulator.hash_capacity()        // NEW method
        let accumulator_vec_capacity_sum = accumulator.vec_capacity() // NEW method
        let accumulator_vec_len_sum = accumulator.vec_len()           // NEW method
        let accumulator_estimate_bytes = accumulator.memory_estimate_bytes()

        return Self { sampled_at, rss_bytes, ..alloc_stats, ..smaps,
                      accumulator_count, accumulator_source_count,
                      accumulator_capacity, accumulator_vec_capacity_sum,
                      accumulator_vec_len_sum, accumulator_estimate_bytes }
    }

    /// Convenience: RSS in MiB as formatted string, or "N/A".
    pub fn rss_mib_display(&self) -> String {
        match self.rss_bytes {
            Some(b) => format!("{:.1}", b as f64 / 1_048_576.0),
            None => "N/A".into(),
        }
    }

    /// Convenience: the unaccounted gap = RSS - accumulator_estimate.
    /// This is the value we are trying to attribute.
    pub fn unaccounted_bytes(&self) -> Option<i64> {
        self.rss_bytes.map(|rss|
            rss as i64 - self.accumulator_estimate_bytes as i64
        )
    }
}
```

### New Accumulator Methods (P-01b)

Add three methods to `Accumulator` to expose capacity vs len:

```
// core/src/storage/accumulator.rs  (add to existing impl block)

/// Number of allocated HashMap buckets (capacity, not len).
/// This reveals over-allocation from HashMap growth.
pub fn hash_capacity(&self) -> usize {
    self.points.capacity()
}

/// Sum of capacity() across all inner Vecs.
/// Reveals allocation headroom that Vec::push reserves.
pub fn vec_capacity(&self) -> usize {
    self.points.values().map(|v| v.capacity()).sum()
}

/// Sum of len() across all inner Vecs (should equal self.count).
pub fn vec_len(&self) -> usize {
    self.points.values().map(|v| v.len()).sum()
}
```

---

## P-02: Enhanced Heartbeat Logging

Modify the existing heartbeat in `BronzeSubscriber::start()` flush_timer branch. Currently emits:
- `accumulator_count`, `accumulator_mib`, `wal_mib`, `rss_mib`, `wal_errors`, `events_received`

Enhanced heartbeat adds capacity metrics and allocator stats:

```
// In BronzeSubscriber::start(), flush_timer tick branch:

_ = flush_timer.tick() => {
    let diag = MemoryDiagnostics::collect(&self.accumulator)

    info!(
        subscriber_id = %self.id,

        // Existing fields (backward compatible)
        accumulator_count = diag.accumulator_count,
        accumulator_mib = format!("{:.1}", diag.accumulator_estimate_bytes as f64 / 1_048_576.0),
        wal_mib = format!("{:.1}", self.wal.file_size_bytes() as f64 / 1_048_576.0),
        rss_mib = diag.rss_mib_display(),
        wal_errors = self.wal_errors,
        events_received = self.events_received,

        // NEW capacity fields
        accum_hash_capacity = diag.accumulator_capacity,
        accum_vec_capacity = diag.accumulator_vec_capacity_sum,
        accum_vec_len = diag.accumulator_vec_len_sum,
        accum_waste_ratio = format!("{:.2}",
            if diag.accumulator_vec_len_sum > 0 {
                diag.accumulator_vec_capacity_sum as f64 / diag.accumulator_vec_len_sum as f64
            } else { 0.0 }
        ),

        // NEW allocator fields (Linux only, "N/A" elsewhere)
        arena_mib = format_opt_mib(diag.arena_bytes),
        uordblks_mib = format_opt_mib(diag.uordblks_bytes),
        fordblks_mib = format_opt_mib(diag.fordblks_bytes),
        hblkhd_mib = format_opt_mib(diag.hblkhd_bytes),

        // NEW smaps fields
        heap_rss_mib = format_opt_mib(diag.heap_rss_bytes),
        anon_rss_mib = format_opt_mib(diag.anon_rss_bytes),

        // NEW gap field -- the thing we want to shrink
        unaccounted_mib = diag.unaccounted_bytes()
            .map(|b| format!("{:.1}", b as f64 / 1_048_576.0))
            .unwrap_or_else(|| "N/A".into()),

        "Heartbeat"
    )
}

/// Helper: format Option<u64> bytes as MiB string.
fn format_opt_mib(bytes: Option<u64>) -> String {
    match bytes {
        Some(b) => format!("{:.1}", b as f64 / 1_048_576.0),
        None => "N/A".into(),
    }
}
```

### Backward Compatibility

All existing fields remain at the same positions with the same keys. New fields are appended. Log parsers that read `accumulator_count`, `rss_mib`, etc. continue to work unmodified.

---

## P-03: Per-Source Snapshot Memory

Wrap each source's Parquet write in `snapshot()` with RSS before/after to attribute memory to specific sources. Currently the snapshot loop is:

```
for (source_id, points) in points_by_source {
    store.write_raw_snapshot(points.clone(), &partition_path).await?
}
```

Enhanced version:

```
// In BronzeSubscriber::snapshot():

let mut per_source_deltas: Vec<(String, usize, f64)> = Vec::new()  // (source_id, point_count, delta_mib)

for (source_id, points) in &points_by_source {
    let point_count = points.len()
    let rss_before = read_rss_bytes()
    let partition_path = self.partition_path(source_id, snapshot_time)

    self.store.write_raw_snapshot(points.clone(), &partition_path).await?

    let rss_after = read_rss_bytes()
    let delta_mib = match (rss_before, rss_after) {
        (Some(b), Some(a)) => (a as f64 - b as f64) / 1_048_576.0,
        _ => 0.0,
    }

    per_source_deltas.push((source_id.clone(), point_count, delta_mib))

    debug!(
        subscriber_id = %self.id,
        source_id = source_id,
        points = point_count,
        rss_delta_mib = format!("{:+.1}", delta_mib),
        "Snapshot source write complete"
    )
}

// Summary log at info level
for (source_id, count, delta) in &per_source_deltas {
    info!(
        subscriber_id = %self.id,
        source_id = source_id,
        points = count,
        rss_delta_mib = format!("{:+.1}", delta),
        "Snapshot per-source memory delta"
    )
}
```

### Why per-source matters

With 8 sources (production), one source may dominate the RSS spike. Per-source tracking reveals which source (by payload size, cardinality) drives the most temporary allocation.

---

## P-04: Subsystem Memory Attribution

Tag RSS samples by active subsystem. This uses a lightweight "phase marker" approach -- sample RSS at subsystem boundaries.

```
/// Active subsystem phases for memory attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subsystem {
    Idle,           // between events, waiting on select!
    BronzeIngest,   // handle_point: WAL + accumulator
    BronzeSnapshot, // Parquet write cycle
    SilverEtl,      // Silver subscriber transform + insert
    HttpPoll,       // HTTP source fetch cycle
    MqttRecv,       // MQTT message receipt + parse
}

/// Lightweight subsystem memory sample.
pub struct SubsystemSample {
    pub subsystem: Subsystem,
    pub rss_bytes_enter: Option<u64>,
    pub rss_bytes_exit: Option<u64>,
    pub duration_us: u64,
}

impl SubsystemSample {
    pub fn delta_bytes(&self) -> Option<i64> {
        match (self.rss_bytes_enter, self.rss_bytes_exit) {
            (Some(enter), Some(exit)) => Some(exit as i64 - enter as i64),
            _ => None,
        }
    }
}
```

### Usage in BronzeSubscriber

```
// In the select! loop, wrap event handling:

result = receiver.recv() => {
    match result {
        Ok(point) => {
            let rss_enter = read_rss_bytes()
            self.handle_point(point)
            let rss_exit = read_rss_bytes()
            // Only log if delta exceeds threshold (avoid noise)
            if let (Some(e), Some(x)) = (rss_enter, rss_exit) {
                let delta = x as i64 - e as i64
                if delta.unsigned_abs() > 1_048_576 {  // > 1 MiB change
                    debug!(
                        subscriber_id = %self.id,
                        subsystem = "BronzeIngest",
                        rss_delta_mib = format!("{:+.1}", delta as f64 / 1_048_576.0),
                        "Subsystem memory delta > 1 MiB"
                    )
                }
            }
        }
        // ... existing error handling unchanged
    }
}
```

### Design Decision: Sampling, Not Wrapping

Reading `/proc/self/status` costs ~5-10us per call. At 1-2 events/min this is negligible. At snapshot time (8 sources), it adds ~80us total. No measurable overhead on Pi 5.

We do NOT wrap every subsystem in a guard struct. Instead, we sample at key boundaries only:
1. Before/after each event handle_point (BronzeIngest)
2. Before/after each source write in snapshot (BronzeSnapshot)
3. Before/after the overall snapshot() call (already exists from BUG-004)

Silver, HTTP, and MQTT subsystem attribution is deferred to a follow-up if Bronze instrumentation does not explain the gap.

---

## P-05: /proc/self/smaps Parser

Parse `/proc/self/smaps` to extract heap, stack, and anonymous mapping RSS.

```
// core/src/diagnostics/memory.rs

/// Aggregated RSS from /proc/self/smaps by mapping type.
pub struct SmapsSummary {
    pub heap_rss_bytes: u64,     // [heap] mapping
    pub stack_rss_bytes: u64,    // [stack] mapping
    pub anon_rss_bytes: u64,     // all anonymous (non-file-backed) mappings
    pub file_rss_bytes: u64,     // all file-backed mappings (shared libs, etc.)
    pub total_rss_bytes: u64,    // sum of all Rss: lines
}

/// Parse /proc/self/smaps and aggregate RSS by mapping type.
///
/// smaps format (repeated per mapping):
///   7f1234000-7f1235000 rw-p 00000000 00:00 0    [heap]
///   Size:     4096 kB
///   Rss:      4096 kB
///   ...
///
/// Strategy:
/// 1. Read the file line by line
/// 2. Track the current mapping name (last token of header line)
/// 3. For each "Rss:" line, add to the appropriate bucket
pub fn read_proc_smaps_summary() -> Option<SmapsSummary> {
    let content = std::fs::read_to_string("/proc/self/smaps").ok()?

    let mut summary = SmapsSummary { heap: 0, stack: 0, anon: 0, file: 0, total: 0 }
    let mut current_name: Option<String> = None
    let mut is_file_backed = false

    for line in content.lines() {
        if line.contains('-') && !line.starts_with(' ') {
            // This is a mapping header line
            // Format: addr-addr perms offset dev inode [name]
            let parts: Vec<&str> = line.split_whitespace().collect()
            current_name = parts.last().map(|s| s.to_string())
            // File-backed if the inode field (5th) is not "0"
            is_file_backed = parts.get(4).map(|s| *s != "0").unwrap_or(false)
        } else if line.starts_with("Rss:") {
            // Parse "Rss:    1234 kB"
            let kb = parse_kb_value(line)?
            let bytes = kb * 1024

            summary.total_rss_bytes += bytes

            match current_name.as_deref() {
                Some("[heap]") => summary.heap_rss_bytes += bytes,
                Some("[stack]") => summary.stack_rss_bytes += bytes,
                _ if is_file_backed => summary.file_rss_bytes += bytes,
                _ => summary.anon_rss_bytes += bytes,
            }
        }
    }

    Some(summary)
}

/// Parse "Key:    1234 kB" -> 1234u64
fn parse_kb_value(line: &str) -> Option<u64> {
    line.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<u64>().ok())
}
```

### Existing /proc/self/status parser (refactored)

The existing `read_process_rss_mib()` in bronze.rs reads VmRSS from `/proc/self/status`. Refactor to:

```
/// Read VmRSS from /proc/self/status. Returns bytes.
pub fn read_proc_status_rss_bytes() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("VmRSS:"))
                .and_then(|line| parse_kb_value(line))
                .map(|kb| kb * 1024)
        })
}
```

The old `read_process_rss_mib()` becomes a thin wrapper:

```
fn read_process_rss_mib() -> Option<f64> {
    read_proc_status_rss_bytes().map(|b| b as f64 / 1_048_576.0)
}
```

---

## P-06: Allocator Stats via libc FFI (mallinfo2)

Call `mallinfo2()` on Linux to get allocator-level statistics. This reveals fragmentation and free-list bloat that RSS alone cannot show.

```
// core/src/diagnostics/memory.rs

/// Allocator statistics from glibc mallinfo2().
/// All values in bytes.
pub struct MallocStats {
    pub arena: u64,      // Non-mmapped space allocated from system
    pub ordblks: u64,    // Number of free chunks
    pub hblkhd: u64,     // Space in mmapped regions
    pub uordblks: u64,   // Total allocated space
    pub fordblks: u64,   // Total free space
    pub keepcost: u64,   // Topmost releasable block
}

/// Read allocator stats via mallinfo2() FFI.
/// Returns None on non-Linux platforms.
pub fn read_mallinfo2() -> Option<MallocStats> {
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct Mallinfo2 {
            arena: usize,
            ordblks: usize,
            smblks: usize,
            hblks: usize,
            hblkhd: usize,
            usmblks: usize,
            fsmblks: usize,
            uordblks: usize,
            fordblks: usize,
            keepcost: usize,
        }

        extern "C" {
            fn mallinfo2() -> Mallinfo2;
        }

        let info = unsafe { mallinfo2() };

        Some(MallocStats {
            arena: info.arena as u64,
            ordblks: info.ordblks as u64,
            hblkhd: info.hblkhd as u64,
            uordblks: info.uordblks as u64,
            fordblks: info.fordblks as u64,
            keepcost: info.keepcost as u64,
        })
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
```

### Why mallinfo2, not mallinfo

`mallinfo()` uses `int` fields which overflow at 2 GiB. `mallinfo2()` uses `size_t` fields. On the Pi 5 with 512 MiB container limit this is not strictly necessary, but `mallinfo2()` is available in glibc >= 2.33 (Raspberry Pi OS Bookworm ships glibc 2.36) and is the correct modern API.

---

## P-07: Memory Trend Log

Periodic summary (every N snapshots) showing growth rate. Helps detect slow leaks without requiring external tooling to parse individual heartbeats.

```
// core/src/diagnostics/memory.rs

/// Rolling window of RSS samples for trend computation.
pub struct MemoryTrend {
    samples: VecDeque<(DateTime<Utc>, u64)>,  // (timestamp, rss_bytes)
    max_samples: usize,                        // ring buffer size
}

impl MemoryTrend {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Record an RSS sample. Evicts oldest if buffer full.
    pub fn record(&mut self, rss_bytes: u64) {
        let now = Utc::now()
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front()
        }
        self.samples.push_back((now, rss_bytes))
    }

    /// Compute growth rate in bytes/hour using linear regression on samples.
    /// Returns None if fewer than 2 samples.
    pub fn growth_rate_bytes_per_hour(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None
        }

        let first = self.samples.front()?
        let last = self.samples.back()?
        let hours = (last.0 - first.0).num_seconds() as f64 / 3600.0

        if hours < 0.01 {
            return None  // too short to compute meaningful rate
        }

        let delta_bytes = last.1 as f64 - first.1 as f64
        Some(delta_bytes / hours)
    }

    /// Number of recorded samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }
}
```

### Usage in BronzeSubscriber

```
// Add to BronzeSubscriber struct:
memory_trend: MemoryTrend,

// In new():
memory_trend: MemoryTrend::new(100),  // last 100 heartbeat samples

// In heartbeat (flush_timer tick):
if let Some(rss) = diag.rss_bytes {
    self.memory_trend.record(rss)
}

// Every 10 snapshots, emit trend summary:
// In snapshot(), after successful write:
self.snapshots_written += 1
if self.snapshots_written % 10 == 0 {
    let rate = self.memory_trend.growth_rate_bytes_per_hour()
    info!(
        subscriber_id = %self.id,
        snapshots = self.snapshots_written,
        trend_samples = self.memory_trend.len(),
        rss_growth_mib_per_hour = rate
            .map(|r| format!("{:.2}", r / 1_048_576.0))
            .unwrap_or_else(|| "N/A".into()),
        rss_current_mib = diag.rss_mib_display(),
        "Memory trend summary (every 10 snapshots)"
    )
}
```

### Production Cadence

- Heartbeat fires every `flush_interval_secs` (default 5s) -- trend records RSS each time
- Snapshot fires every `snapshot_interval_secs` (default 1800s = 30 min)
- Trend summary every 10 snapshots = every 5 hours
- With 100 sample ring buffer at 5s intervals = last ~8.3 minutes of RSS history per trend report

---

## Module Structure

```
core/src/diagnostics/
    mod.rs          // pub mod memory;
    memory.rs       // MemoryDiagnostics, SmapsSummary, MallocStats, MemoryTrend
                    // read_proc_status_rss_bytes(), read_proc_smaps_summary()
                    // read_mallinfo2(), format_opt_mib()
```

### Changes to Existing Files

| File | Change |
|------|--------|
| `core/src/lib.rs` | Add `pub mod diagnostics;` |
| `core/src/storage/accumulator.rs` | Add `hash_capacity()`, `vec_capacity()`, `vec_len()` |
| `core/src/subscribers/bronze.rs` | Import `MemoryDiagnostics`, `MemoryTrend`; enhance heartbeat and snapshot; refactor `read_process_rss_mib()` to delegate to shared function |

### No New Dependencies

All instrumentation uses:
- `std::fs::read_to_string` for `/proc` files
- `extern "C"` FFI for `mallinfo2` (libc, no crate needed)
- `std::collections::VecDeque` for trend buffer
- `chrono` (already a dependency) for timestamps
