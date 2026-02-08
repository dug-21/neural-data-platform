# ADR-AIR017-002: In-Memory Accumulator Design

## Status

Proposed

## Context

AIR-017 introduces an in-memory accumulator that holds the current day's data points
across flush cycles. Currently, `BronzeSubscriber` has a `buffer: Vec<RawDataPoint>`
(line 87 of `core/src/subscribers/bronze.rs`) that is drained on every flush. After
AIR-017, the buffer is replaced by a persistent accumulator that survives across flushes
and is only cleared on day rollover.

The accumulator must support:

1. **Insert**: Add a `RawDataPoint` on event receipt (after WAL append). Must be fast --
   this is on the hot path.
2. **Drain for snapshot**: Produce a `Vec<RawDataPoint>` grouped by stream for Parquet
   writes. Must not lose data if the snapshot write fails.
3. **Count/size estimation**: Report point count and estimated memory for health checks.
4. **Day rollover**: Clear all points for the previous day, start fresh for today.
5. **Startup seeding**: Accept a `Vec<RawDataPoint>` loaded from an existing Parquet file
   during recovery.

### Option A: HashMap<String, Vec<RawDataPoint>>

Key is `stream_id` (extracted from `source_id`). Each stream's points are in a Vec,
appended in arrival order.

Pros:
- Simple, idiomatic Rust.
- O(1) insert (amortized Vec push).
- Natural grouping by stream for snapshot writes (ParquetStore partitions by stream).
- Low overhead (no ordering structure, no dedup index).

Cons:
- Points within a stream are in arrival order, not timestamp order. This is fine for
  Parquet writes (Parquet is not required to be sorted; queries handle ordering) but
  may be suboptimal for Phase 3 read queries.
- No deduplication by timestamp. Duplicate detection during startup recovery (Parquet +
  WAL replay) requires a separate pass.

### Option B: BTreeMap<(String, DateTime<Utc>), RawDataPoint>

Key is `(stream_id, timestamp)`. Points are sorted by stream and then by time.

Pros:
- Naturally sorted output for snapshot writes.
- Implicit deduplication by timestamp within a stream.
- Efficient range queries for Phase 3 (`range((stream, start)..(stream, end))`).

Cons:
- Higher memory overhead per entry (~80 bytes for BTreeMap node overhead vs ~0 for Vec).
  For 44,000 points: ~3.5 MiB additional overhead.
- O(log n) insert vs O(1) for HashMap+Vec.
- Timestamp-based dedup is incorrect: two different data points can share the same
  timestamp (e.g., two MQTT messages arriving in the same millisecond from the same
  stream). A `(stream_id, timestamp)` key would silently drop the second one.
- Over-engineering for Phase 1 where the accumulator is write-only (no reads until Phase 3).

### Option C: Custom ring buffer per stream

Fixed-capacity circular buffers, one per stream. Oldest entries are evicted when full.

Pros:
- Bounded memory.
- O(1) insert.

Cons:
- Data loss by design (eviction). Contradicts the durability goal -- the WAL retains
  everything, but the accumulator would not, creating an inconsistency.
- Snapshot must write all data, not just the ring buffer window.
- Complexity with no clear benefit over HashMap+Vec.

## Decision

**Option A: HashMap<String, Vec<RawDataPoint>>.**

The accumulator is a wrapper struct around `HashMap<String, Vec<RawDataPoint>>`:

```rust
pub struct BronzeAccumulator {
    /// Stream-grouped data points. Key is stream_id (e.g., "air-quality").
    points: HashMap<String, Vec<RawDataPoint>>,
    /// Total count across all streams (avoid summing Vec lengths on every query).
    count: usize,
}
```

Methods:

```rust
impl BronzeAccumulator {
    /// Create an empty accumulator.
    pub fn new() -> Self;

    /// Insert a point. Extracts stream_id from source_id.
    pub fn insert(&mut self, point: RawDataPoint);

    /// Seed the accumulator from recovered Parquet data.
    pub fn seed(&mut self, points: Vec<RawDataPoint>);

    /// Get all points for a stream (cloned). Used for snapshot writes.
    /// Returns an empty Vec if stream has no data.
    pub fn get_stream(&self, stream_id: &str) -> Vec<RawDataPoint>;

    /// Get all stream IDs with data.
    pub fn stream_ids(&self) -> Vec<String>;

    /// Total point count across all streams.
    pub fn count(&self) -> usize;

    /// Estimated memory usage in bytes.
    pub fn estimated_bytes(&self) -> usize;

    /// Clear all data (day rollover).
    pub fn clear(&mut self);

    /// Clear data for a specific stream.
    pub fn clear_stream(&mut self, stream_id: &str);
}
```

The `get_stream()` method clones the data rather than draining it. This is deliberate:
if the snapshot write fails, the accumulator still holds the data for the next attempt.
The memory cost of the clone (~22 MiB transient during snapshot) is acceptable and
explicitly budgeted in the SCOPE.md memory model.

### Thread Safety

In Phase 1, the accumulator lives inside `BronzeSubscriber`'s `start()` method and is
accessed only from the single tokio task running the `select!` loop. No `Arc`, `Mutex`,
or `RwLock` is needed.

In Phase 3, when the read path needs access, the accumulator will be wrapped in
`Arc<RwLock<BronzeAccumulator>>`. The writer (BronzeSubscriber) holds a write lock
briefly during `insert()`. Readers (query methods) hold a read lock during
`get_stream()`. Since `insert()` is O(1) and the write lock duration is microseconds,
contention will be negligible.

### Deduplication During Startup Recovery

When recovering from a crash, the accumulator is seeded from the Parquet file (last
snapshot) and then WAL entries after the watermark are replayed. To avoid duplicates:

- Each WAL entry has a sequence number (`seq: u64`).
- The Parquet snapshot records the WAL watermark at the time of writing (stored in a
  companion `.watermark` file or as Parquet metadata).
- On replay, only WAL entries with `seq > watermark` are inserted into the accumulator.
- This guarantees no duplicates without scanning the accumulator.

## Consequences

**Positive:**
- Simple, low-overhead design. HashMap+Vec is the most cache-friendly option for
  sequential insert followed by bulk read.
- Memory overhead is negligible beyond the data itself (~44,000 entries x 8 bytes
  Vec pointer = ~350 KB overhead).
- Grouping by stream matches the Parquet partitioning scheme
  (`raw/{stream_id}/year={Y}/month={M}/day={D}/data.parquet`), so snapshot writes
  iterate streams directly.
- No risk of silent dedup (BTreeMap Option B would have dropped same-timestamp points).

**Negative:**
- Points within a stream are unsorted. Phase 3 queries that need time-ordered results
  must sort after retrieval. For 11,000 points per stream, a sort is ~0.1ms -- negligible.
- No built-in dedup. If the same event is somehow inserted twice (code bug, not WAL
  replay), the accumulator stores both copies. The WAL sequence number mechanism prevents
  this during recovery; during normal operation, the EventBus broadcast channel prevents
  duplicates by design.

**Neutral:**
- The 500-byte estimate per `RawDataPoint` is conservative. Actual sizes vary by stream:
  air-quality payloads are ~300 bytes (JSON with ~15 fields), while NWS forecast payloads
  can be ~1-2 KB. The total accumulator size is bounded by the number of points per day,
  not the payload size variance. At worst (all large payloads), the accumulator reaches
  ~40-50 MiB -- still well within the 512 MiB Docker limit.
