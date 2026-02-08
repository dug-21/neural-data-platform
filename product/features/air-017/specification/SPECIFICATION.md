# AIR-017 Specification: Bronze Write-Ahead Architecture

> **Feature:** air-017
> **Created:** 2026-02-08
> **Status:** Specification
> **Specification Agent:** ndp-architect
> **AgentDB Patterns Used:** ID 10 (specification:config-validation-pipeline), ID 29 (conventions:feature-directory-structure), ID 30 (architecture:ndp-overview)

---

## 1. Problem Statement

The Bronze layer's write path performs a read-modify-write on every flush cycle. `append_to_raw_parquet()` (`core/src/storage/parquet.rs:563-622`) reads the entire daily Parquet file, deserializes all rows into a `Vec<RawDataPoint>`, appends the new batch, and rewrites the entire file. At 30-second flush intervals this produces approximately 2,880 full-file rewrites per day, each costing O(file_size) in both CPU and memory.

Separately, the WAL provides no durability until flush time. Events sit in the `BronzeSubscriber.buffer` (`core/src/subscribers/bronze.rs:87`) with no persistence for up to 30 seconds. A crash during that window loses all buffered data.

This specification defines the functional and non-functional requirements for eliminating read-modify-write, moving WAL writes to event receipt, and introducing periodic snapshot-based Parquet writes.

---

## 2. Functional Requirements

### 2.1 WAL-on-Receipt (Immediate Durability)

**FR-001: WAL append on event receipt**
When `BronzeSubscriber.handle_point()` receives a `RawDataPoint`, it must append a serialized WAL entry to disk before adding the point to the in-memory accumulator. The WAL write must include an `fsync` (via `File::flush()` as today, or `File::sync_data()` for stronger guarantees). The current call site is `bronze.rs:182-197`; today it only pushes to `self.buffer` with no persistence.

**FR-002: WAL entry format**
Each WAL entry must be a single JSON line containing:
- `seq`: monotonically increasing `u64` sequence number, assigned by the WAL on append
- `stream_id`: extracted from `source_id` using the existing `extract_stream_from_source_id()` logic (`bronze.rs:337-346`)
- `data`: the serialized `RawDataPoint`

Example:
```json
{"seq":42,"stream_id":"air-quality","data":{"timestamp":"2026-02-08T10:30:00Z","source_id":"air-quality-Mqtt","ndp_id":"sensor-001","raw_payload":{"pm25":12.5}}}
```

**FR-003: WAL sequence number persistence**
The WAL must track the next sequence number in a separate file (`wal.seq`) alongside the WAL data file. On startup, the sequence file is read to resume numbering. If the sequence file is missing or corrupt, the WAL must scan the data file to find the highest sequence number and continue from there. The sequence number must never go backward.

**FR-004: WAL file per stream**
Each stream must have its own WAL file, located at `{base_path}/wal/{stream_id}.wal`. This replaces the current single `wal.log` file (`core/src/storage/parquet.rs:25`). Per-stream WALs enable independent watermark advancement and avoid cross-stream interference during replay.

**FR-005: WAL write failure behavior**
If a WAL append fails (disk full, I/O error), `BronzeSubscriber` must:
1. Log the error at `error!` level with the stream_id and error detail.
2. Increment `errors_total` counter.
3. Still add the point to the in-memory accumulator (best-effort durability; the accumulator is the primary data path for the current process lifetime).
4. NOT drop the point silently.

The rationale: losing WAL durability is bad but losing the data entirely is worse. The point remains in the accumulator and will be snapshot to Parquet at the next snapshot interval.

### 2.2 In-Memory Accumulator

**FR-006: Accumulator data structure**
`BronzeSubscriber` must maintain a per-stream in-memory accumulator: `HashMap<String, Vec<RawDataPoint>>` keyed by stream_id (extracted from `source_id`). This replaces the current flat `buffer: Vec<RawDataPoint>` (`bronze.rs:87`). The accumulator persists across flush cycles; it is NOT drained on flush.

**FR-007: Accumulator population**
Every point that passes the stream filter (`accepts_stream()`) must be added to the appropriate stream's accumulator Vec. Points are appended in arrival order.

**FR-008: Accumulator queryability (Phase 3 placeholder)**
The accumulator must be wrapped in `Arc<RwLock<HashMap<String, Vec<RawDataPoint>>>>` so that Phase 3 can expose a read handle to the Silver catch-up path and MCP server. In Phase 1, only `BronzeSubscriber` holds a write reference. No external readers are wired in Phase 1, but the data structure must support concurrent reads from the start.

**FR-009: Accumulator memory cap**
Each stream's accumulator Vec must have a configurable maximum point count (`max_accumulator_points`, default: 50,000). If a stream's Vec reaches this limit, the oldest points are evicted (FIFO) to make room for new arrivals. This prevents unbounded memory growth if a stream produces data faster than expected. Eviction must log a warning with the stream_id and number of evicted points.

**FR-010: Accumulator clear on day rollover**
At day rollover (see FR-016), the accumulator for the finalized day must be cleared completely. A fresh empty Vec is initialized for the new day.

### 2.3 Snapshot Timer and Parquet Writes

**FR-011: Snapshot timer**
`BronzeSubscriber` must maintain a configurable snapshot interval timer using `tokio::time::interval`. The interval is configured via `snapshot_interval_secs` in `BronzeSubscriberConfig` (default: 1800 seconds = 30 minutes). The timer runs as a branch in the existing `select!` loop (`bronze.rs:223-268`).

**FR-012: Snapshot write semantics**
When the snapshot timer fires, for each stream in the accumulator:
1. Clone the stream's Vec (snapshot isolation -- the accumulator continues accepting new points during the write).
2. Write the cloned Vec to the daily Parquet file using `write_raw_parquet()` (`parquet.rs:502-560`) which creates the file from scratch (overwrite, not append).
3. On success, record the snapshot watermark (the highest WAL sequence number included in this snapshot) in a sidecar file `{partition_dir}/snapshot.watermark`.
4. On failure, log at `error!` level and retry at the next snapshot interval. Do NOT advance the watermark.

This eliminates `append_to_raw_parquet()` (`parquet.rs:563-622`) entirely. The read-modify-write function is no longer called on the write path.

**FR-013: Snapshot write uses spawn_blocking**
The Parquet write during snapshot must use `tokio::task::spawn_blocking` as the existing `write_raw_parquet()` already does (`parquet.rs:510`). No change to the blocking strategy is required, but the caller must await the result and handle errors.

**FR-014: Snapshot watermark file**
Each stream partition directory (`{base_path}/raw/{stream_id}/year=YYYY/month=MM/day=DD/`) must contain a `snapshot.watermark` file. This file contains a single line: the WAL sequence number of the last entry included in the most recent Parquet snapshot for that day. Format: plain text integer (e.g., `4217\n`). On startup, this watermark tells the recovery logic which WAL entries have already been persisted to Parquet.

**FR-015: Flush timer repurposing**
The existing `flush_interval_secs` timer (`bronze.rs:218-219`) is repurposed. It no longer triggers a Parquet write. Instead, on each tick:
1. Batch-append all points in the accumulator's "pending WAL" buffer to WAL (if batch WAL writes are implemented for throughput).
2. Log accumulator statistics (point count per stream, WAL size).

If the implementation moves WAL writes to the individual `handle_point()` call (FR-001), then the flush timer becomes a periodic health-check / metrics emission point. It must NOT be removed -- its existence enables future use for batch WAL fsync optimization.

### 2.4 Day Rollover

**FR-016: Day rollover timer**
`BronzeSubscriber` must compute the Duration until the next day boundary (midnight UTC by default, configurable via `day_rollover_utc_hour` in `BronzeSubscriberConfig`, default: 0). The first rollover uses `tokio::time::sleep_until(next_midnight)`. Subsequent rollovers recompute the next midnight from wall clock time (not interval-based) to avoid timer drift.

**FR-017: Day rollover procedure**
When the day rollover timer fires:
1. Perform a final snapshot for yesterday's data (FR-012 procedure, targeting yesterday's partition path).
2. Clear yesterday's accumulator entries (FR-010).
3. Truncate WAL entries for yesterday's stream data up to the snapshot watermark (FR-019).
4. Initialize a fresh empty accumulator for today.
5. Log the rollover at `info!` level with stream_id and point counts.

**FR-018: Day rollover must not block event ingestion**
The rollover snapshot write must run in a background task (`tokio::spawn`). New events arriving during rollover are added to today's accumulator immediately. Yesterday's data is immutable at this point -- no new events can have yesterday's date unless clock skew exceeds the configurable `day_rollover_utc_hour`.

### 2.5 WAL Watermark-Based Commit

**FR-019: Watermark-based WAL truncation**
`WriteAheadLog` must replace the current `commit()` method (`wal.rs:49-60`, which deletes the entire file and recreates it) with `truncate_before(watermark: u64)`. This method:
1. Reads all entries from the WAL file.
2. Filters out entries with `seq <= watermark`.
3. Rewrites the WAL file with only the surviving entries.
4. Updates the sequence file to reflect the surviving range.

Entries with `seq > watermark` survive because they arrived after the last Parquet snapshot and are not yet persisted.

**FR-020: WAL truncation atomicity**
WAL truncation must write surviving entries to a temporary file (`{stream_id}.wal.tmp`), then atomically rename it to `{stream_id}.wal` using `std::fs::rename()`. This prevents data loss if the process crashes during truncation.

### 2.6 Startup Recovery

**FR-021: Recovery procedure on startup**
When `BronzeSubscriber` starts (or `ParquetStore` is constructed):
1. For each stream directory found under `{base_path}/raw/`:
   a. Read today's Parquet file (if it exists) into a `Vec<RawDataPoint>`. This represents data up to the last snapshot.
   b. Read the `snapshot.watermark` file to get the last persisted WAL sequence number.
   c. Replay the stream's WAL file (`{base_path}/wal/{stream_id}.wal`), filtering to only entries with `seq > watermark`.
   d. Deserialize each replayed WAL entry into a `RawDataPoint`.
   e. Merge Parquet data + replayed WAL data into the stream's accumulator.
2. Resume normal operation with the populated accumulator.

**FR-022: Deduplication during recovery**
Duplicate points can occur if a crash happens after a WAL write but before the snapshot watermark is updated. Deduplication uses the tuple `(timestamp_micros, source_id)` as a natural key. During recovery merge (FR-021 step 1e), if a point from the WAL replay matches a point already in the Parquet-seeded accumulator by this key, the WAL point is skipped. This is O(n) with a HashSet lookup.

**FR-023: Recovery from corrupt WAL**
If a WAL file contains a line that fails JSON deserialization, recovery must:
1. Log a warning with the line number and error.
2. Skip the corrupt entry.
3. Continue processing remaining entries.
This matches the existing WAL behavior where partial last lines from crash-interrupted writes are naturally handled by line-by-line JSON parsing.

**FR-024: Recovery from missing snapshot watermark**
If the `snapshot.watermark` file is missing (first run, or file was lost):
- If a Parquet file exists for today: replay ALL WAL entries, deduplicating against the Parquet data.
- If no Parquet file exists: replay ALL WAL entries into an empty accumulator (no dedup needed).

### 2.7 Configuration Schema

**FR-025: New BronzeSubscriberConfig fields**
The following fields must be added to `BronzeSubscriberConfig` (`bronze.rs:39-56`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `snapshot_interval_secs` | `u64` | `1800` | Seconds between Parquet snapshot writes |
| `day_rollover_utc_hour` | `u8` | `0` | UTC hour for day rollover (0-23) |
| `max_accumulator_points` | `usize` | `50000` | Maximum points per stream in accumulator |
| `wal_dir` | `Option<String>` | `None` | Override WAL directory (default: `{base_path}/wal/`) |
| `snapshot_on_shutdown` | `bool` | `true` | Whether to perform a snapshot during graceful shutdown |

Existing fields (`batch_size`, `flush_interval_secs`, `max_retries`, `stream_filter`) remain unchanged. `batch_size` retains its meaning as the event count threshold for triggering a WAL batch flush if batching is used; `flush_interval_secs` becomes the periodic health-check/metrics interval.

**FR-026: platform.yaml schema update**
The `subscribers.bronze` section in `config/base/platform.yaml` must accept the new fields:

```yaml
subscribers:
  bronze:
    enabled: true
    batch_size: 100
    flush_interval_secs: 30
    snapshot_interval_secs: 1800
    day_rollover_utc_hour: 0
    max_accumulator_points: 50000
    snapshot_on_shutdown: true
    max_retries: 3
```

### 2.8 Error Handling

**FR-027: Snapshot failure handling**
If a Parquet snapshot write fails:
1. Log at `error!` level with stream_id and error detail.
2. Increment a `snapshot_failures` counter.
3. Do NOT advance the WAL watermark.
4. Retry at the next snapshot interval.
5. If `snapshot_failures` exceeds `max_retries` consecutive failures for a stream, log at `error!` level with a message indicating the WAL is growing unbounded for that stream.

**FR-028: WAL truncation failure handling**
If WAL truncation fails:
1. Log at `error!` level.
2. Do NOT retry immediately (the next snapshot cycle will attempt truncation again).
3. The WAL continues to grow. This is safe because WAL replay uses the watermark to skip already-persisted entries.

**FR-029: Graceful shutdown behavior**
When `BronzeSubscriber` receives a cancellation signal (`bronze.rs:228-230`):
1. If `snapshot_on_shutdown` is true (FR-025), perform a snapshot for all streams.
2. Flush any pending WAL writes.
3. Log final metrics (events_received, events_written, accumulator sizes, WAL sizes).
4. If the shutdown snapshot fails, log the error but proceed with shutdown (do not block indefinitely).

### 2.9 Removed Code Paths

**FR-030: Remove append_to_parquet() from write path**
The `append_to_parquet()` method (`parquet.rs:157-225`) must no longer be called from `Store::write()` or `Store::write_batch()`. The method may be retained for backward compatibility with the parsed `TimeSeriesPoint` path (non-raw data), but the raw data path (`append_to_raw_parquet()` at `parquet.rs:563-622`) must be removed from the write path entirely.

**FR-031: Remove WAL commit-as-delete**
The current `WriteAheadLog::commit()` method (`wal.rs:49-60`) that deletes the entire file and recreates it must be replaced by `truncate_before(watermark)` (FR-019). The `commit()` method must be removed or deprecated. Callers that previously called `commit()` (in `parquet.rs:272-273` and `parquet.rs:737-738`) must be updated to call `truncate_before()` with the appropriate watermark.

---

## 3. Non-Functional Requirements

**NFR-001: Memory budget**
Peak RSS with 4 active streams must not exceed 150 MiB. Breakdown:
- Accumulator: 4 streams x 11,000 points/day x ~500 bytes = ~22 MiB
- Snapshot transient (column Vecs during Parquet write): ~22 MiB
- Runtime baseline (tokio, MQTT, EventBus, HTTP): ~80-100 MiB
- Total peak (during snapshot): ~130-150 MiB

**NFR-002: Durability latency**
Time from event receipt in `handle_point()` to WAL entry persisted on disk must be less than 10 milliseconds under normal I/O conditions. This is measured as the elapsed time of `wal.append()` including the `flush()` call.

**NFR-003: Parquet write frequency**
At the default 30-minute snapshot interval, each stream produces at most 48 Parquet writes per day (24 hours / 0.5 hours = 48). At 60-minute intervals, 24 writes per day. This is a reduction from the current ~2,880 writes/day.

**NFR-004: Crash recovery time**
Recovery on startup (FR-021) must complete within 5 seconds on a Raspberry Pi 5 for a typical day's data (4 streams, ~44,000 total points). The bottleneck is reading the Parquet files; WAL replay is negligible for the expected WAL sizes (at most 30 minutes of data per stream at default snapshot interval).

**NFR-005: WAL disk usage**
WAL disk usage per stream must not exceed 10 MiB under normal operation. At default 30-minute snapshot intervals with 4 streams producing 1 point every ~8 seconds, each stream accumulates approximately 225 WAL entries between snapshots. At ~500 bytes per entry, this is approximately 112 KiB per stream. The 10 MiB cap provides a 90x safety margin.

**NFR-006: No new runtime dependencies**
No new crate dependencies may be added beyond what air-016 Phase 1 introduces. The implementation uses only `std`, `tokio`, `chrono`, `serde_json`, `polars`, and `tracing` -- all already in the dependency tree.

**NFR-007: Backward compatibility**
The Parquet file format, directory structure (`{base_path}/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet`), and schema (5 columns: timestamp, source_id, ndp_id, context, raw_payload) must remain unchanged. Existing Parquet files from before air-017 must be readable without migration.

**NFR-008: Configuration backward compatibility**
A `platform.yaml` without the new air-017 fields (`snapshot_interval_secs`, `day_rollover_utc_hour`, `max_accumulator_points`, `wal_dir`, `snapshot_on_shutdown`) must work with defaults. Deserialization must use `#[serde(default)]` for all new fields.

---

## 4. Acceptance Criteria

| ID | Requirement | Testable Condition |
|----|-------------|--------------------|
| AC-001 | FR-001 | Given an event arrives at BronzeSubscriber, when handle_point() returns, then the WAL file contains a new entry with the event's data. |
| AC-002 | FR-002 | Given a WAL entry is written, when the entry is read back, then it contains `seq`, `stream_id`, and `data` fields with correct values. |
| AC-003 | FR-003 | Given the process restarts, when WriteAheadLog is constructed, then the sequence number resumes from the last persisted value without gaps or repeats. |
| AC-004 | FR-004 | Given events arrive for streams "air-quality" and "outdoor-weather", when WAL files are inspected, then `wal/air-quality.wal` and `wal/outdoor-weather.wal` exist as separate files. |
| AC-005 | FR-005 | Given WAL append fails (simulated I/O error), when handle_point() returns, then the point is still present in the accumulator AND errors_total is incremented AND an error is logged. |
| AC-006 | FR-006 | Given events arrive for 2 streams, when the accumulator is inspected, then it contains 2 keys, each mapping to a Vec of the correct points for that stream. |
| AC-007 | FR-009 | Given a stream accumulator reaches max_accumulator_points, when a new point arrives, then the oldest point is evicted and the Vec length remains at max_accumulator_points. |
| AC-008 | FR-011 | Given snapshot_interval_secs=10 (test value), when 15 seconds elapse with events arriving, then exactly 1 snapshot write occurs. |
| AC-009 | FR-012 | Given the snapshot timer fires, when the Parquet file is read back, then it contains ALL points from the accumulator for that stream, and the file was created by overwrite (not append). |
| AC-010 | FR-012 | Given a snapshot completes, when the snapshot.watermark file is read, then it contains the highest WAL sequence number that was included in the snapshot. |
| AC-011 | FR-014 | Given a snapshot completes for stream "air-quality" on 2026-02-08, then the file `{base}/raw/air-quality/year=2026/month=02/day=08/snapshot.watermark` exists and contains a valid integer. |
| AC-012 | FR-016 | Given day_rollover_utc_hour=0, when the UTC clock reaches 00:00:00, then a rollover procedure is triggered within 1 second. |
| AC-013 | FR-017 | Given day rollover fires, when the procedure completes, then: (a) yesterday's Parquet file is finalized, (b) yesterday's accumulator is empty, (c) yesterday's WAL entries up to the watermark are truncated. |
| AC-014 | FR-019 | Given a WAL contains entries with seq 1-100, when truncate_before(50) is called, then the WAL contains only entries with seq 51-100. |
| AC-015 | FR-020 | Given a crash occurs during WAL truncation, when the process restarts, then the WAL is either in the pre-truncation state OR the post-truncation state (never partially truncated). |
| AC-016 | FR-021 | Given a process crash after WAL writes but before snapshot, when the process restarts, then the accumulator contains all points from both the last Parquet snapshot AND the replayed WAL entries. |
| AC-017 | FR-022 | Given a crash occurs after snapshot + WAL write but before watermark update, when recovery runs, then duplicate points (present in both Parquet and WAL) appear only once in the accumulator. |
| AC-018 | FR-023 | Given a WAL file contains a corrupt line, when recovery runs, then the corrupt line is skipped with a warning and all other entries are recovered. |
| AC-019 | FR-025 | Given a platform.yaml without snapshot_interval_secs, when BronzeSubscriberConfig is deserialized, then snapshot_interval_secs defaults to 1800. |
| AC-020 | FR-027 | Given a snapshot write fails 4 times consecutively (max_retries=3), then an error log message indicates unbounded WAL growth for that stream. |
| AC-021 | FR-029 | Given snapshot_on_shutdown=true, when BronzeSubscriber receives cancellation, then a snapshot is performed before shutdown completes. |
| AC-022 | FR-030 | Given air-017 is deployed, when write_raw_batch() is called during normal operation, then append_to_raw_parquet() is never invoked (verified by removing the method or adding a panic guard in tests). |
| AC-023 | NFR-001 | Given 4 streams with 11,000 points each in the accumulator, when a snapshot is in progress, then peak RSS (measured via `/proc/self/status` VmRSS) does not exceed 150 MiB. |
| AC-024 | NFR-002 | Given normal I/O conditions, when handle_point() is called 1,000 times, then the average WAL append latency is below 10ms (measured with std::time::Instant). |
| AC-025 | NFR-004 | Given a cold start with 4 streams and 44,000 total points in Parquet + WAL, when recovery completes, then elapsed time is below 5 seconds on Raspberry Pi 5 hardware. |
| AC-026 | NFR-007 | Given a Parquet file written by the pre-air-017 code, when the post-air-017 recovery reads it, then all points are correctly loaded into the accumulator. |
| AC-027 | NFR-008 | Given a platform.yaml from before air-017 (no new fields), when the config is loaded, then BronzeSubscriber starts with all default values and operates correctly. |

---

## 5. Interface Contracts

### 5.1 WriteAheadLog Evolution

**Current** (`core/src/storage/wal.rs:6-65`):

```rust
pub struct WriteAheadLog {
    path: PathBuf,
    file: File,
}

impl WriteAheadLog {
    pub fn new<P: AsRef<Path>>(path: P) -> CoreResult<Self>;
    pub fn append(&mut self, entry: &[u8]) -> CoreResult<()>;
    pub fn replay(&self) -> CoreResult<Vec<Vec<u8>>>;
    pub fn commit(&mut self) -> CoreResult<()>;     // DELETE ALL
    pub fn path(&self) -> &Path;
}
```

**Proposed**:

```rust
/// A WAL entry with a monotonic sequence number for watermark-based truncation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub seq: u64,
    pub stream_id: String,
    pub data: RawDataPoint,
}

pub struct WriteAheadLog {
    dir: PathBuf,
    /// Per-stream file handles, lazily opened.
    streams: HashMap<String, WalStream>,
}

/// Per-stream WAL state.
struct WalStream {
    path: PathBuf,
    file: File,
    next_seq: u64,
}

impl WriteAheadLog {
    /// Construct WAL manager for a base directory.
    /// Creates `{dir}/` if it does not exist.
    /// Discovers existing stream WAL files and loads sequence counters.
    pub fn new<P: AsRef<Path>>(dir: P) -> CoreResult<Self>;

    /// Append a point to the appropriate stream's WAL.
    /// Returns the assigned sequence number.
    pub fn append(&mut self, stream_id: &str, point: &RawDataPoint) -> CoreResult<u64>;

    /// Replay all entries for a stream with seq > after_watermark.
    /// If after_watermark is 0, replays all entries.
    pub fn replay_stream(
        &self,
        stream_id: &str,
        after_watermark: u64,
    ) -> CoreResult<Vec<WalEntry>>;

    /// Truncate entries with seq <= watermark for a stream.
    /// Uses atomic rename for crash safety.
    pub fn truncate_before(
        &mut self,
        stream_id: &str,
        watermark: u64,
    ) -> CoreResult<()>;

    /// List all stream_ids that have WAL files.
    pub fn streams(&self) -> Vec<String>;

    /// Get the current highest sequence number for a stream.
    pub fn current_seq(&self, stream_id: &str) -> Option<u64>;
}
```

### 5.2 BronzeSubscriber Changes

**Current** (`core/src/subscribers/bronze.rs:83-95`):

```rust
pub struct BronzeSubscriber {
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    buffer: Vec<RawDataPoint>,          // REMOVED
    cancellation_token: CancellationToken,
    is_running: bool,
    events_received: u64,
    events_written: u64,
    batches_written: u64,
    errors_total: u64,
}
```

**Proposed**:

```rust
pub struct BronzeSubscriber {
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    wal: WriteAheadLog,
    /// Per-stream in-memory accumulator. Persists across flush cycles.
    /// Arc<RwLock<>> for Phase 3 read-path sharing.
    accumulator: Arc<RwLock<HashMap<String, Vec<RawDataPoint>>>>,
    /// Per-stream WAL watermarks from last successful snapshot.
    snapshot_watermarks: HashMap<String, u64>,
    cancellation_token: CancellationToken,
    is_running: bool,
    // Metrics
    events_received: u64,
    events_written: u64,
    snapshots_written: u64,
    snapshot_failures: HashMap<String, u32>,
    errors_total: u64,
}
```

The `buffer` field is replaced by `accumulator`. The `batches_written` counter is replaced by `snapshots_written`. A `wal` field is added for direct WAL management. The `store` field is retained for Parquet writes (snapshot path only).

### 5.3 BronzeSubscriberConfig Changes

**Current** (`core/src/subscribers/bronze.rs:39-56`):

```rust
pub struct BronzeSubscriberConfig {
    pub batch_size: usize,
    pub flush_interval_secs: u64,
    pub max_retries: u32,
    pub stream_filter: Vec<String>,
}
```

**Proposed** (additions only; existing fields unchanged):

```rust
pub struct BronzeSubscriberConfig {
    // --- existing ---
    pub batch_size: usize,
    pub flush_interval_secs: u64,
    pub max_retries: u32,
    pub stream_filter: Vec<String>,

    // --- air-017 additions ---
    #[serde(default = "default_snapshot_interval_secs")]
    pub snapshot_interval_secs: u64,          // default: 1800

    #[serde(default)]
    pub day_rollover_utc_hour: u8,            // default: 0

    #[serde(default = "default_max_accumulator_points")]
    pub max_accumulator_points: usize,        // default: 50_000

    #[serde(default)]
    pub wal_dir: Option<String>,              // default: None ({base_path}/wal/)

    #[serde(default = "default_snapshot_on_shutdown")]
    pub snapshot_on_shutdown: bool,            // default: true
}
```

### 5.4 ParquetStore Changes

The `ParquetStore` struct (`parquet.rs:15-18`) retains its current fields but the following methods are affected:

**Removed from write path:**
- `append_to_raw_parquet()` (`parquet.rs:563-622`) -- no longer called by `write_raw_batch()`
- `append_to_parquet()` (`parquet.rs:157-225`) -- no longer called by `write_batch()`

**Retained (used by snapshot):**
- `write_raw_parquet()` (`parquet.rs:502-560`) -- called by BronzeSubscriber snapshot logic
- `raw_partition_path()` (`parquet.rs:486-496`) -- called to determine where snapshots go

**Modified:**
- `RawStore::write_raw_batch()` implementation (`parquet.rs:710-741`) -- simplified to delegate to BronzeSubscriber (or removed if BronzeSubscriber handles writes directly without going through the RawStore trait)

The relationship between `BronzeSubscriber` and `ParquetStore` changes from "subscriber calls store.write_raw_batch() which does WAL + read-modify-write" to "subscriber owns the WAL and accumulator directly, and uses store.write_raw_parquet() only for snapshot writes."

### 5.5 New Files

| File | Purpose |
|------|---------|
| `core/src/storage/wal.rs` | Rewritten: per-stream WAL with sequence numbers and watermark truncation |
| `core/src/storage/accumulator.rs` | In-memory accumulator with eviction and RwLock wrapper (may be inlined in bronze.rs if small) |

### 5.6 Removed/Deprecated Code

| Location | What | Action |
|----------|------|--------|
| `parquet.rs:563-622` | `append_to_raw_parquet()` | Remove entirely |
| `parquet.rs:157-225` | `append_to_parquet()` | Remove from raw write path (keep for parsed TimeSeriesPoint if still used) |
| `wal.rs:49-60` | `commit()` | Remove; replaced by `truncate_before()` |
| `parquet.rs:34-60` | `replay_wal()` | Rewrite: recovery moves to BronzeSubscriber |
| `parquet.rs:25-26` | Single `wal.log` path | Replaced by per-stream WAL directory |

---

## 6. Data Flow Diagrams

### 6.1 Normal Operation (Event Receipt)

```
MQTT/HTTP Event
     |
     v
  EventBus (broadcast channel)
     |
     v
  BronzeSubscriber.handle_point(point)
     |
     +---> WAL.append(stream_id, point) --> disk (fsync)
     |         Returns seq number
     |
     +---> accumulator[stream_id].push(point)
     |         (FIFO eviction if at max_accumulator_points)
     |
     +---> events_received += 1
     |
     v
  Return (point is durable in WAL + queryable in accumulator)
```

### 6.2 Snapshot Flow (Periodic Timer)

```
  snapshot_timer fires
     |
     v
  For each stream_id in accumulator:
     |
     +---> Clone accumulator[stream_id]  (snapshot isolation)
     |
     +---> Compute partition_path for today
     |
     +---> spawn_blocking {
     |         write_raw_parquet(cloned_points, path)  // overwrite
     |     }
     |
     +---> On success:
     |         watermark = WAL.current_seq(stream_id)
     |         Write watermark to {partition_dir}/snapshot.watermark
     |         WAL.truncate_before(stream_id, watermark)
     |         snapshot_failures[stream_id] = 0
     |         snapshots_written += 1
     |
     +---> On failure:
     |         snapshot_failures[stream_id] += 1
     |         Log error (do NOT advance watermark)
     |         If failures > max_retries: log unbounded WAL warning
     |
     v
  Resume normal operation
```

### 6.3 Day Rollover Flow

```
  day_rollover_timer fires (midnight UTC)
     |
     v
  For each stream_id with yesterday's data:
     |
     +---> Snapshot yesterday's accumulator to yesterday's partition
     |         (Same as 6.2 but targeting yesterday's date)
     |
     +---> WAL.truncate_before(stream_id, snapshot_watermark)
     |
     +---> Clear accumulator[stream_id] for yesterday
     |
     +---> Log rollover metrics
     |
     v
  Today's accumulator is already receiving new events
  (started empty at midnight or seeded from events that arrived
   after midnight during the rollover procedure)
```

### 6.4 Crash Recovery Flow (Startup)

```
  Process starts
     |
     v
  BronzeSubscriber::new() / recovery()
     |
     v
  For each stream_id found in WAL directory:
     |
     +---> Read today's Parquet file (if exists)
     |         parquet_points = Vec<RawDataPoint>
     |
     +---> Read snapshot.watermark (if exists)
     |         watermark = u64 (or 0 if missing)
     |
     +---> WAL.replay_stream(stream_id, after_watermark=watermark)
     |         wal_points = Vec<WalEntry>
     |
     +---> Build dedup set from parquet_points:
     |         seen = HashSet<(i64, String)>  // (timestamp_micros, source_id)
     |
     +---> Merge:
     |         accumulator[stream_id] = parquet_points
     |         for entry in wal_points:
     |             key = (entry.data.timestamp_micros, entry.data.source_id)
     |             if key not in seen:
     |                 accumulator[stream_id].push(entry.data)
     |                 seen.insert(key)
     |
     +---> Log recovery stats (parquet count, WAL replayed count, dedup count)
     |
     v
  Start select! loop (normal operation)
```

---

## 7. Phase Boundaries

### Phase 1 (This Specification)
- FR-001 through FR-015 (WAL-on-receipt, accumulator, snapshot)
- FR-025 through FR-031 (config, error handling, code removal)
- All NFR requirements
- Recovery: FR-021 through FR-024

### Phase 2 (Subsequent)
- FR-016 through FR-020 (day rollover, WAL watermark truncation)
- Day rollover timer and procedure

### Phase 3 (Future)
- FR-008 Phase 3 integration (expose accumulator to Silver/MCP read path)
- Silver catch-up path reads from accumulator instead of stale Parquet

This phasing means Phase 1 delivers the core value (eliminate read-modify-write, immediate WAL durability) without the complexity of day rollover. In Phase 1, the WAL grows for the full day and is truncated only on snapshot. Day rollover in Phase 2 adds the daily finalization boundary.

---

## 8. Migration Path

1. Deploy air-017 with the new WAL and accumulator architecture.
2. Existing Parquet files are read normally during recovery (FR-021, NFR-007).
3. The old single `wal.log` file, if present, is replayed one final time using the legacy `replay()` method, then removed.
4. New per-stream WAL files are created in `{base_path}/wal/`.
5. No data migration is required. The first snapshot after deployment writes a fresh Parquet file from the accumulator.
