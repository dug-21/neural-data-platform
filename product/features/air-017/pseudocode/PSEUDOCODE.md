# AIR-017 Pseudocode: Bronze Write-Ahead Architecture

> **Feature:** AIR-017 (Phase 1 + Phase 2)
> **Scope:** Eliminate read-modify-write from Bronze Parquet layer
> **Files Evolved:** `core/src/storage/wal.rs`, `core/src/subscribers/bronze.rs`, `core/src/storage/parquet.rs`, `config/base/platform.yaml`

---

## 1. WriteAheadLog v2 (`core/src/storage/wal.rs`)

Evolves from 65-line append/replay/commit(=delete) to watermark-based WAL with sequence numbers.

### 1.1 Data Structures

```rust
// Each WAL entry is a single JSON line in the file.
// The sequence number is part of the serialized JSON, not a file-level index.
struct WalEntry {
    sequence: u64,                // monotonically increasing, assigned by WAL
    source_id: String,            // e.g. "air-quality-Mqtt"
    timestamp: DateTime<Utc>,     // ingestion timestamp from RawDataPoint
    point: RawDataPoint,          // full RawDataPoint (timestamp, source_id, ndp_id, context, raw_payload)
}

struct WriteAheadLog {
    path: PathBuf,                // e.g. data/bronze/wal/bronze.wal
    file: File,                   // opened in append mode
    next_sequence: u64,           // next sequence to assign (starts at 1 or loaded from file)
    committed_watermark: u64,     // all entries with seq <= this have been snapshot to Parquet
}
```

### 1.2 Constructor: `WriteAheadLog::new(path) -> CoreResult<Self>`

Replaces the current constructor at `wal.rs:12-22`. The new version scans the existing file to recover `next_sequence` and `committed_watermark`.

```
fn new(path):
    create parent directories if needed

    if file exists AND file size > 0:
        // Recovery: scan file to learn current state
        open file for read
        max_sequence = 0
        for each line in file:
            skip empty lines
            parse line as JSON -> WalEntry
            if parse fails:
                // Partial write from crash: skip corrupted trailing line
                warn!("Skipping corrupted WAL line at end of file")
                break   // only the last line can be partial; lines before it were flushed
            max_sequence = max(max_sequence, entry.sequence)

        next_sequence = max_sequence + 1

        // Read watermark from header line if present, else 0
        // Watermark is stored as first line: {"__watermark": N}
        // (see commit_to() for how it's written)
        committed_watermark = read_watermark_from_file(path)  // 0 if not found

        // Re-open in append mode for new writes
        file = open(path, append=true)
    else:
        // Fresh start
        file = open(path, create=true, append=true)
        next_sequence = 1
        committed_watermark = 0
        // Write watermark header
        writeln!(file, json!({"__watermark": 0}))
        file.flush()

    return Self { path, file, next_sequence, committed_watermark }
```

### 1.3 Append: `fn append(&mut self, point: &RawDataPoint) -> CoreResult<u64>`

Replaces `wal.rs:24-31`. Returns the assigned sequence number instead of `()`.

```
fn append(point: &RawDataPoint) -> CoreResult<u64>:
    let seq = self.next_sequence
    let entry = WalEntry {
        sequence: seq,
        source_id: point.source_id.clone(),
        timestamp: point.timestamp,
        point: point.clone(),
    }

    let json_line = serde_json::to_string(&entry)
        .map_err(|e| CoreError::Storage(format!("WAL serialize failed: {}", e)))

    writeln!(self.file, "{}", json_line)
        .map_err(|e| CoreError::Storage(format!("WAL write failed: {}", e)))

    self.file.flush()
        .map_err(|e| CoreError::Storage(format!("WAL flush failed: {}", e)))

    self.next_sequence = seq + 1

    return Ok(seq)
```

**Error contract:** If append returns `Err`, the caller MUST NOT add the point to the in-memory accumulator. Data that is not in the WAL is not durable and must not be counted as received.

### 1.4 Replay: `fn replay_since(&self, watermark: u64) -> CoreResult<Vec<WalEntry>>`

Replaces `wal.rs:34-46`. Filters by watermark instead of returning all entries.

```
fn replay_since(watermark: u64) -> CoreResult<Vec<WalEntry>>:
    let file = File::open(&self.path)
        .map_err(|e| CoreError::Storage(format!("WAL open for replay failed: {}", e)))
    let reader = BufReader::new(file)
    let mut entries = Vec::new()

    for line in reader.lines():
        let line = line?
        if line.trim().is_empty():
            continue

        // Skip watermark header line
        if line.contains("\"__watermark\""):
            continue

        match serde_json::from_str::<WalEntry>(&line):
            Ok(entry):
                if entry.sequence > watermark:
                    entries.push(entry)
            Err(e):
                // Corrupted line — only possible at end of file from crash mid-write.
                // Log and stop. All valid entries before this are intact.
                warn!(error = %e, "Skipping corrupted WAL entry, assuming crash tail")
                break

    // Sort by sequence to guarantee ordering (file order should already be correct,
    // but sort is cheap insurance after a crash recovery)
    entries.sort_by_key(|e| e.sequence)

    return Ok(entries)
```

### 1.5 Commit: `fn commit_to(&mut self, watermark: u64) -> CoreResult<()>`

Replaces `wal.rs:49-59`. Instead of deleting the entire file, rewrites it keeping only entries with `sequence > watermark`.

```
fn commit_to(watermark: u64) -> CoreResult<()>:
    if watermark <= self.committed_watermark:
        // Nothing to commit — watermark hasn't advanced
        return Ok(())

    // Step 1: Read all entries that survive the truncation
    let surviving = self.replay_since(watermark)?

    // Step 2: Write to temp file atomically
    let tmp_path = self.path.with_extension("wal.tmp")
    let mut tmp_file = File::create(&tmp_path)
        .map_err(|e| CoreError::Storage(format!("WAL temp file create failed: {}", e)))

    // Write watermark header
    writeln!(tmp_file, "{}", json!({"__watermark": watermark}))
        .map_err(|e| CoreError::Storage(format!("WAL watermark write failed: {}", e)))

    // Write surviving entries
    for entry in &surviving:
        let json_line = serde_json::to_string(&entry)
            .map_err(|e| CoreError::Storage(format!("WAL re-serialize failed: {}", e)))
        writeln!(tmp_file, "{}", json_line)?

    tmp_file.flush()?
    // fsync the temp file for crash safety
    tmp_file.sync_all()?

    // Step 3: Atomic rename (on same filesystem, this is atomic on Linux/ext4)
    std::fs::rename(&tmp_path, &self.path)
        .map_err(|e| CoreError::Storage(format!("WAL rename failed: {}", e)))

    // Step 4: Re-open the new file in append mode
    self.file = OpenOptions::new().append(true).open(&self.path)
        .map_err(|e| CoreError::Storage(format!("WAL re-open after commit failed: {}", e)))

    self.committed_watermark = watermark

    info!(
        watermark = watermark,
        surviving_entries = surviving.len(),
        "WAL committed to watermark"
    )

    return Ok(())
```

**Why atomic rename:** If the process crashes between creating the temp file and renaming, the old WAL file remains intact. On next startup, `new()` reads the old file and replays from the old watermark. No data is lost. The worst case is replaying entries that were already snapshot — dedup in the accumulator handles this.

### 1.6 Accessors

```
fn current_watermark(&self) -> u64:
    return self.committed_watermark

fn next_sequence(&self) -> u64:
    return self.next_sequence

fn path(&self) -> &Path:
    return &self.path
```

### 1.7 Watermark Header Parsing (internal helper)

```
fn read_watermark_from_file(path: &Path) -> u64:
    let file = File::open(path)
    // if open fails, return 0 — no watermark on fresh file
    let reader = BufReader::new(file)

    if let Some(Ok(first_line)) = reader.lines().next():
        if let Ok(val) = serde_json::from_str::<Value>(&first_line):
            if let Some(wm) = val.get("__watermark").and_then(|v| v.as_u64()):
                return wm

    return 0  // no watermark found — treat as fresh
```

---

## 2. InMemoryAccumulator (new: `core/src/storage/accumulator.rs`)

A new module. The accumulator holds all of today's data in memory, organized by source_id. It serves two purposes: (a) feed the Parquet snapshot writer, (b) enable future read-path queries (Phase 3).

### 2.1 Data Structures

```rust
struct Accumulator {
    // source_id -> Vec of points for that source, sorted by timestamp
    points: HashMap<String, Vec<RawDataPoint>>,
    // Total point count across all sources (avoids iterating HashMap for len)
    count: usize,
    // Tracking bounds for diagnostics
    earliest: Option<DateTime<Utc>>,
    latest: Option<DateTime<Utc>>,
    // The date this accumulator covers (used for day rollover)
    active_date: NaiveDate,
}
```

### 2.2 Constructor

```
fn new(date: NaiveDate) -> Self:
    return Self {
        points: HashMap::new(),
        count: 0,
        earliest: None,
        latest: None,
        active_date: date,
    }
```

### 2.3 Add: `fn add(&mut self, point: RawDataPoint)`

```
fn add(&mut self, point: RawDataPoint):
    let ts = point.timestamp
    let source_id = point.source_id.clone()

    // Update bounds
    match self.earliest:
        None => self.earliest = Some(ts)
        Some(e) if ts < e => self.earliest = Some(ts)
        _ => ()
    match self.latest:
        None => self.latest = Some(ts)
        Some(l) if ts > l => self.latest = Some(ts)
        _ => ()

    // Insert into source bucket
    self.points
        .entry(source_id)
        .or_insert_with(Vec::new)
        .push(point)

    self.count += 1
```

### 2.4 Seed from Parquet: `fn seed_from_parquet(&mut self, path: &Path) -> CoreResult<usize>`

Used during startup recovery. Reads an existing daily Parquet file and populates the accumulator. Returns the number of points loaded.

```
fn seed_from_parquet(path: &Path) -> CoreResult<usize>:
    if !path.exists():
        debug!(path = %path.display(), "No Parquet file to seed from")
        return Ok(0)

    // Use spawn_blocking because Parquet read is CPU-intensive
    // (same pattern as parquet.rs:510 write_raw_parquet)
    let points = tokio::task::spawn_blocking(move || -> CoreResult<Vec<RawDataPoint>> {
        let file = File::open(path)
            .map_err(|e| CoreError::Storage(format!("Seed open failed: {}", e)))
        let df = ParquetReader::new(file).finish()
            .map_err(|e| CoreError::Storage(format!("Seed Parquet read failed: {}", e)))

        let mut points = Vec::with_capacity(df.height())

        // Extract columns — same pattern as parquet.rs:577-596 (append_to_raw_parquet)
        let timestamps = df.column("timestamp")?.i64()?
        let source_ids = df.column("source_id")?.utf8()?
        let ndp_ids = df.column("ndp_id").ok().and_then(|c| c.utf8().ok())
        let contexts = df.column("context").ok().and_then(|c| c.utf8().ok())
        let raw_payloads = df.column("raw_payload")?.utf8()?

        for i in 0..df.height():
            if let (Some(ts), Some(source_id), Some(payload_str)) =
                (timestamps.get(i), source_ids.get(i), raw_payloads.get(i)):

                let timestamp = DateTime::from_timestamp_micros(ts)
                    .ok_or_else(|| CoreError::Storage("Invalid timestamp in Parquet".into()))?
                let ndp_id = ndp_ids.and_then(|col| col.get(i).map(|s| s.to_string()))
                let context = contexts.and_then(|col| col.get(i).and_then(|s| serde_json::from_str(s).ok()))
                let raw_payload = serde_json::from_str(payload_str)
                    .map_err(|e| CoreError::Storage(format!("Invalid JSON in Parquet: {}", e)))?

                points.push(RawDataPoint {
                    timestamp, source_id: source_id.to_string(),
                    ndp_id, context, raw_payload,
                })

        return Ok(points)
    }).await??

    let loaded_count = points.len()
    for point in points:
        self.add(point)

    info!(
        count = loaded_count,
        path = %path.display(),
        "Seeded accumulator from Parquet"
    )

    return Ok(loaded_count)
```

### 2.5 Merge WAL Entries: `fn merge_wal_entries(&mut self, entries: Vec<WalEntry>)`

Used during startup recovery after seeding from Parquet. Dedup is critical: entries that were both WAL'd and snapshot'd must not produce duplicates.

```
fn merge_wal_entries(&mut self, entries: Vec<WalEntry>):
    let mut added = 0
    let mut skipped = 0

    for entry in entries:
        let source_id = &entry.point.source_id
        let ts = entry.point.timestamp

        // Dedup check: does this exact (source_id, timestamp) already exist?
        // This is the primary correctness guarantee for crash recovery.
        //
        // Rationale: RawDataPoint has no unique ID field. The combination of
        // (source_id, timestamp) is unique because:
        //   - timestamp is set to Utc::now() at ingestion (nanosecond precision via chrono)
        //   - source_id identifies the specific source instance
        //   - Two events from the same source at the same nanosecond timestamp would
        //     already be a bug in the ingestion layer
        //
        // Implementation: linear scan of the source's Vec. For recovery this is
        // acceptable because:
        //   - It runs once at startup, not on the hot path
        //   - Typical WAL replay is 0 to ~1800 entries (30-60 min of data)
        //   - Building a HashSet<(String, i64)> from the Parquet-seeded data
        //     would use more memory than the scan saves in time

        let is_duplicate = self.points
            .get(source_id.as_str())
            .map(|existing| existing.iter().any(|p| p.timestamp == ts))
            .unwrap_or(false)

        if is_duplicate:
            skipped += 1
            continue

        self.add(entry.point)
        added += 1

    info!(
        added = added,
        skipped_duplicates = skipped,
        "Merged WAL entries into accumulator"
    )
```

### 2.6 Snapshot Access: `fn all_points_by_source(&self) -> &HashMap<String, Vec<RawDataPoint>>`

Returns a reference to all points. The snapshot logic iterates this to build Parquet files, one per source_id.

```
fn all_points_by_source(&self) -> &HashMap<String, Vec<RawDataPoint>>:
    return &self.points
```

### 2.7 Drain for Date: `fn drain_for_date(&mut self, date: NaiveDate) -> HashMap<String, Vec<RawDataPoint>>`

Used during day rollover. Takes all points whose timestamp falls on the given date. Points for other dates remain.

```
fn drain_for_date(&mut self, date: NaiveDate) -> HashMap<String, Vec<RawDataPoint>>:
    let mut drained: HashMap<String, Vec<RawDataPoint>> = HashMap::new()
    let mut remaining_count = 0

    for (source_id, points) in &mut self.points:
        let (for_date, keep): (Vec<_>, Vec<_>) = points
            .drain(..)
            .partition(|p| p.timestamp.date_naive() == date)

        if !for_date.is_empty():
            drained.insert(source_id.clone(), for_date)

        *points = keep
        remaining_count += points.len()

    // Recalculate count
    self.count = remaining_count

    // Recalculate bounds from remaining points
    self.earliest = None
    self.latest = None
    for points in self.points.values():
        for p in points:
            match self.earliest:
                None => self.earliest = Some(p.timestamp)
                Some(e) if p.timestamp < e => self.earliest = Some(p.timestamp)
                _ => ()
            match self.latest:
                None => self.latest = Some(p.timestamp)
                Some(l) if p.timestamp > l => self.latest = Some(p.timestamp)
                _ => ()

    // Remove empty source_id entries
    self.points.retain(|_, v| !v.is_empty())

    return drained
```

### 2.8 Memory Estimation: `fn memory_estimate_bytes(&self) -> usize`

Approximate memory usage for monitoring. Used by health checks to report accumulator size.

```
fn memory_estimate_bytes(&self) -> usize:
    let mut total = 0

    // HashMap overhead: ~48 bytes per entry + key String heap allocation
    total += self.points.len() * 48

    for (source_id, points) in &self.points:
        // Key: String on heap
        total += source_id.len()
        // Vec overhead: 24 bytes (ptr + len + cap) + element storage
        total += 24 + points.capacity() * std::mem::size_of::<RawDataPoint>()
        // RawDataPoint: the serde_json::Value in raw_payload is the big variable.
        // Estimate ~200 bytes per Value (typical sensor JSON).
        // source_id ~20 bytes on heap, ndp_id ~30 bytes on heap.
        for p in points:
            total += p.source_id.len()
            total += p.ndp_id.as_ref().map(|s| s.len()).unwrap_or(0)
            total += estimate_json_heap_size(&p.raw_payload)  // recursive Value size
            total += p.context.as_ref().map(|v| estimate_json_heap_size(v)).unwrap_or(0)

    return total
```

Helper for JSON heap estimation:

```
fn estimate_json_heap_size(value: &serde_json::Value) -> usize:
    match value:
        Value::Null | Value::Bool(_) | Value::Number(_) => 0  // inline in enum
        Value::String(s) => s.len()
        Value::Array(arr) => 24 + arr.iter().map(estimate_json_heap_size).sum::<usize>()
                             + arr.len() * std::mem::size_of::<Value>()
        Value::Object(map) => 48 + map.iter()
            .map(|(k, v)| k.len() + estimate_json_heap_size(v) + 48)
            .sum::<usize>()
```

### 2.9 Diagnostics

```
fn count(&self) -> usize:
    return self.count

fn source_count(&self) -> usize:
    return self.points.len()

fn earliest(&self) -> Option<DateTime<Utc>>:
    return self.earliest

fn latest(&self) -> Option<DateTime<Utc>>:
    return self.latest

fn active_date(&self) -> NaiveDate:
    return self.active_date
```

---

## 3. BronzeSubscriber v2 (`core/src/subscribers/bronze.rs`)

Modifies the existing BronzeSubscriber. The `select!` loop at line 223-268 gains two new timer branches and the event-handling logic changes from buffer-then-flush to WAL-then-accumulate.

### 3.1 Updated Config (`BronzeSubscriberConfig`)

Extends the struct at `bronze.rs:39-56`:

```rust
struct BronzeSubscriberConfig {
    // Existing fields (bronze.rs:42-55)
    batch_size: usize,              // kept for backpressure on WAL writes
    flush_interval_secs: u64,       // WAL batch flush interval (now less critical)
    max_retries: u32,               // retry count for snapshot writes
    stream_filter: Vec<String>,     // stream filter (unchanged)

    // New fields
    snapshot_interval_secs: u64,    // Parquet snapshot interval (default: 1800 = 30 min)
    day_rollover_utc_hour: u8,      // hour to finalize daily file (default: 0 = midnight UTC)
}

fn default_snapshot_interval_secs() -> u64 { 1800 }
fn default_day_rollover_utc_hour() -> u8 { 0 }
```

### 3.2 Updated Struct Fields

Replaces `bronze.rs:83-95`:

```rust
struct BronzeSubscriber {
    // Existing (unchanged)
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    cancellation_token: CancellationToken,
    is_running: bool,

    // Removed: buffer: Vec<RawDataPoint>  (replaced by accumulator)

    // New: WAL and Accumulator
    wal: WriteAheadLog,
    accumulator: Accumulator,

    // Metrics (expanded)
    events_received: u64,
    events_persisted: u64,          // renamed from events_written — counts WAL appends
    snapshots_written: u64,         // renamed from batches_written — counts Parquet snapshots
    snapshot_failures: u64,         // consecutive snapshot failures
    errors_total: u64,
    wal_sequence: u64,              // latest WAL sequence for diagnostics
}
```

### 3.3 Constructor

Replaces `bronze.rs:104-122`. Now requires a WAL path.

```
fn new(id, config, store, wal_path) -> CoreResult<Self>:
    let wal = WriteAheadLog::new(wal_path)?
    let today = Utc::now().date_naive()
    let accumulator = Accumulator::new(today)

    return Ok(Self {
        id: id.into(),
        config,
        store,
        wal,
        accumulator,
        cancellation_token: CancellationToken::new(),
        is_running: false,
        events_received: 0,
        events_persisted: 0,
        snapshots_written: 0,
        snapshot_failures: 0,
        errors_total: 0,
        wal_sequence: 0,
    })
```

### 3.4 Updated `start()` Method

Replaces the `select!` loop at `bronze.rs:211-287`. The core change: three timers instead of one, and events go to WAL immediately instead of a buffer.

```
async fn start(&mut self, mut receiver) -> Result<(), SubscriberError>:
    info!(subscriber_id = %self.id, "Starting BronzeSubscriber v2")
    self.is_running = true

    // --- Startup Recovery ---
    self.recover().await?

    // --- Timer Setup ---

    // 1. Snapshot timer: write accumulator to Parquet
    let snapshot_interval = Duration::from_secs(self.config.snapshot_interval_secs)
    let mut snapshot_timer = tokio::time::interval(snapshot_interval)
    snapshot_timer.tick().await   // skip immediate first tick

    // 2. Day rollover timer: finalize yesterday, start new accumulator
    let mut day_rollover_sleep = self.compute_next_rollover_sleep()
    // day_rollover_sleep is a Pin<Box<tokio::time::Sleep>> that fires at the
    // configured UTC hour. After each fire, we recompute for the next day.

    // 3. Flush timer is kept for periodic WAL stats logging, NOT for Parquet writes.
    //    In v2, every event is WAL-appended immediately, so there is no buffer to flush.
    //    The timer now serves only to log accumulator stats at regular intervals.
    let stats_interval = Duration::from_secs(self.config.flush_interval_secs)
    let mut stats_timer = tokio::time::interval(stats_interval)
    stats_timer.tick().await   // skip immediate first tick

    // --- Main Loop ---
    loop {
        tokio::select! {
            biased;

            // Priority 1: Cancellation (graceful shutdown)
            _ = self.cancellation_token.cancelled() => {
                info!(subscriber_id = %self.id, "Received cancellation signal")
                break
            }

            // Priority 2: Day rollover
            _ = &mut day_rollover_sleep => {
                match self.finalize_day().await {
                    Ok(()) => {
                        info!(subscriber_id = %self.id, "Day rollover complete")
                    }
                    Err(e) => {
                        error!(
                            subscriber_id = %self.id,
                            error = %e,
                            "Day rollover failed — will retry next tick"
                        )
                        // Do NOT clear accumulator or advance watermark.
                        // finalize_day() is idempotent; retrying is safe.
                    }
                }
                // Recompute sleep for next day (or retry in 5 min on failure)
                day_rollover_sleep = self.compute_next_rollover_sleep()
            }

            // Priority 3: Parquet snapshot
            _ = snapshot_timer.tick() => {
                match self.snapshot_to_parquet().await {
                    Ok(count) => {
                        debug!(
                            subscriber_id = %self.id,
                            points_snapshot = count,
                            "Parquet snapshot complete"
                        )
                        self.snapshot_failures = 0
                    }
                    Err(e) => {
                        self.snapshot_failures += 1
                        error!(
                            subscriber_id = %self.id,
                            error = %e,
                            consecutive_failures = self.snapshot_failures,
                            "Parquet snapshot failed — WAL retains all entries"
                        )
                        // WAL is NOT committed. All entries survive for next attempt.
                        // If failures accumulate, WAL grows but data is safe.
                    }
                }
            }

            // Priority 4: Stats logging (replaces old flush timer purpose)
            _ = stats_timer.tick() => {
                debug!(
                    subscriber_id = %self.id,
                    accumulator_count = self.accumulator.count(),
                    accumulator_sources = self.accumulator.source_count(),
                    accumulator_memory_bytes = self.accumulator.memory_estimate_bytes(),
                    wal_sequence = self.wal_sequence,
                    wal_watermark = self.wal.current_watermark(),
                    "Bronze stats"
                )
            }

            // Priority 5: Receive events from EventBus
            result = receiver.recv() => {
                match result {
                    Ok(point) => {
                        self.handle_point_v2(point).await
                    }
                    Err(RecvError::Lagged(n)) => {
                        warn!(
                            subscriber_id = %self.id,
                            lagged_count = n,
                            "Subscriber lagged — events lost from EventBus"
                        )
                        // Continue. Lagged events are gone. The WAL + accumulator
                        // for prior events are intact.
                    }
                    Err(RecvError::Closed) => {
                        info!(subscriber_id = %self.id, "Event bus channel closed")
                        break
                    }
                }
            }
        }
    }

    // --- Shutdown ---
    info!(subscriber_id = %self.id, "Performing final snapshot before shutdown")
    if let Err(e) = self.snapshot_to_parquet().await {
        error!(
            subscriber_id = %self.id,
            error = %e,
            "Final snapshot failed — WAL preserves data for next startup"
        )
        // NOT fatal. WAL has all data. Next startup will recover.
    }

    self.is_running = false
    info!(
        subscriber_id = %self.id,
        events_received = self.events_received,
        events_persisted = self.events_persisted,
        snapshots_written = self.snapshots_written,
        errors_total = self.errors_total,
        "BronzeSubscriber stopped"
    )

    return Ok(())
```

### 3.5 Event Handling: `handle_point_v2`

Replaces `bronze.rs:182-197`. The critical change: WAL write happens synchronously on event receipt, not deferred to a flush timer.

```
async fn handle_point_v2(&mut self, point: Arc<RawDataPoint>):
    self.events_received += 1

    // Stream filter check (unchanged from bronze.rs:186-193)
    if !self.accepts_stream(&point.source_id):
        debug!(
            subscriber_id = %self.id,
            source_id = %point.source_id,
            "Skipping point: stream not in filter"
        )
        return

    let owned_point: RawDataPoint = (*point).clone()

    // Step 1: WAL append — this is the durability guarantee
    match self.wal.append(&owned_point):
        Ok(seq) => {
            self.wal_sequence = seq
            self.events_persisted += 1

            // Step 2: Add to accumulator — ONLY if WAL succeeded
            // This ensures accumulator is always a subset of what WAL contains.
            self.accumulator.add(owned_point)
        }
        Err(e) => {
            // WAL write failed. Point is NOT durable. Do NOT add to accumulator.
            self.errors_total += 1
            error!(
                subscriber_id = %self.id,
                source_id = %point.source_id,
                error = %e,
                "WAL append failed — point dropped (not durable)"
            )
            // This is a serious error. If WAL disk is full or filesystem is
            // read-only, every subsequent event will also fail. The health check
            // will report unhealthy via errors_total > 0.
        }
    }
```

**Key invariant:** The accumulator is always a subset of WAL. If WAL append fails, the point never enters the accumulator. If the process crashes, WAL replay + dedup rebuilds the accumulator exactly.

### 3.6 Day Rollover Timer Computation

```
fn compute_next_rollover_sleep(&self) -> Pin<Box<tokio::time::Sleep>>:
    let now = Utc::now()
    let rollover_hour = self.config.day_rollover_utc_hour as u32

    // Compute the next occurrence of rollover_hour:00:00 UTC
    let today_rollover = now.date_naive()
        .and_hms_opt(rollover_hour, 0, 0)
        .unwrap()
        .and_utc()

    let next_rollover = if now >= today_rollover:
        // Already past today's rollover time — schedule for tomorrow
        today_rollover + chrono::Duration::days(1)
    else:
        today_rollover

    let duration_until = (next_rollover - now)
        .to_std()
        .unwrap_or(Duration::from_secs(60))  // fallback: 1 min if clock is weird

    debug!(
        subscriber_id = %self.id,
        next_rollover = %next_rollover,
        secs_until = duration_until.as_secs(),
        "Computed next day rollover"
    )

    return Box::pin(tokio::time::sleep(duration_until))
```

**Edge case: midnight exactly.** If the process starts at exactly midnight UTC, `now >= today_rollover` is true, so rollover schedules for tomorrow. This is correct because the accumulator was just initialized with today's date and there is no yesterday to finalize.

---

## 4. Snapshot Logic

### 4.1 `snapshot_to_parquet() -> CoreResult<usize>`

Writes the full contents of the accumulator to Parquet for each source_id. Returns total points written.

```
async fn snapshot_to_parquet(&mut self) -> CoreResult<usize>:
    let all_points = self.accumulator.all_points_by_source()

    if all_points.is_empty():
        debug!(subscriber_id = %self.id, "Snapshot skipped — accumulator empty")
        return Ok(0)

    let mut total_written = 0

    // Write one Parquet file per source_id (matches current partition layout).
    // Each write is a full overwrite — NO read-modify-write.
    for (source_id, points) in all_points:
        if points.is_empty():
            continue

        // Compute the partition path for this source's data.
        // Use the first point's timestamp for the day partition.
        // All points for a given source_id on a given day go to the same file.
        //
        // NOTE: Points spanning midnight (e.g., accumulator has points from 23:59
        // and 00:01) will all go to one file based on the first point's date.
        // Day rollover (section 5) handles this by draining only yesterday's points
        // before the snapshot writes today's file.
        let representative_ts = points[0].timestamp
        let path = self.store.raw_partition_path(source_id, representative_ts)
        // ^^ Uses ParquetStore::raw_partition_path (parquet.rs:486-496)
        //    e.g. data/bronze/raw/air-quality/year=2026/month=02/day=08/data.parquet

        // Clone points for the write (accumulator retains originals).
        // This is the "peak memory" moment: accumulator + cloned write batch.
        let write_batch: Vec<RawDataPoint> = points.clone()

        // Write using ParquetStore::write_raw_parquet (NOT append_to_raw_parquet).
        // This is a FULL OVERWRITE — the entire file is written from scratch.
        // The read-modify-write path (parquet.rs:563-622) is bypassed entirely.
        match self.store.write_raw_parquet_overwrite(write_batch, &path).await:
            Ok(()) => {
                total_written += points.len()
                debug!(
                    subscriber_id = %self.id,
                    source_id = %source_id,
                    count = points.len(),
                    path = %path.display(),
                    "Snapshot wrote Parquet"
                )
            }
            Err(e) => {
                // Fail the entire snapshot if any source fails.
                // WAL is NOT committed. Next snapshot attempt will retry all sources.
                error!(
                    subscriber_id = %self.id,
                    source_id = %source_id,
                    error = %e,
                    "Snapshot Parquet write failed"
                )
                return Err(e.into())
            }

    // All sources written successfully. Advance WAL watermark.
    let current_seq = self.wal_sequence
    self.wal.commit_to(current_seq)
        .map_err(|e| {
            // WAL commit failed AFTER Parquet wrote successfully.
            // This is NOT data loss — WAL entries will be replayed on next startup
            // and deduped against the Parquet data. It just means the WAL file is
            // larger than it needs to be. Log and continue.
            error!(
                subscriber_id = %self.id,
                error = %e,
                "WAL commit failed after successful snapshot — WAL will be larger than needed"
            )
            e
        })?

    self.snapshots_written += 1

    info!(
        subscriber_id = %self.id,
        total_written = total_written,
        wal_watermark = current_seq,
        "Snapshot complete"
    )

    return Ok(total_written)
```

### 4.2 New ParquetStore Method: `write_raw_parquet_overwrite`

This is the existing `write_raw_parquet` (parquet.rs:502-560) exposed with a new name to clarify intent. It takes a `Vec<RawDataPoint>` and a path, creates parent directories, and writes a fresh Parquet file (no read step). The existing `write_raw_parquet` already does exactly this — the rename makes the intent explicit and allows the old `append_to_raw_parquet` to remain for any code that still needs it during migration.

```
// In ParquetStore (or directly on RawStore trait):
async fn write_raw_parquet_overwrite(
    &self,
    points: Vec<RawDataPoint>,
    path: &Path,
) -> CoreResult<()>:
    // Identical to existing write_raw_parquet (parquet.rs:502-560).
    // The key difference from append_to_raw_parquet (parquet.rs:563-622) is:
    //   - NO "if path.exists() { read file }" block
    //   - Writes points directly to file, overwriting any existing content
    //   - This is the ONLY Parquet write path used by BronzeSubscriber v2
    self.write_raw_parquet(points, path).await
```

**Why not modify RawStore trait directly?** The RawStore trait is used by other callers (Silver catch-up, MCP server). Changing `write_raw_batch` semantics would break those consumers. A separate method avoids coupling. The trait extension can happen in Phase 3 when read-path integration is addressed.

---

## 5. Day Rollover Logic

### 5.1 `finalize_day() -> CoreResult<()>`

Called when the day rollover timer fires. Finalizes yesterday's Parquet file and starts a fresh accumulator for today.

```
async fn finalize_day(&mut self) -> CoreResult<()>:
    let yesterday = self.accumulator.active_date()
    let today = Utc::now().date_naive()

    // Guard: don't rollover if we're still on the same day.
    // This can happen if the timer fires early due to clock adjustment.
    if yesterday == today:
        debug!(
            subscriber_id = %self.id,
            date = %today,
            "Day rollover skipped — still same day"
        )
        return Ok(())

    info!(
        subscriber_id = %self.id,
        finalizing_date = %yesterday,
        new_date = %today,
        "Starting day rollover"
    )

    // Step 1: Drain yesterday's points from accumulator
    let yesterday_points = self.accumulator.drain_for_date(yesterday)

    if yesterday_points.is_empty():
        info!(
            subscriber_id = %self.id,
            date = %yesterday,
            "No points for yesterday — nothing to finalize"
        )
        // Still update active_date even if no data
    else:
        // Step 2: Write final Parquet snapshot for yesterday
        for (source_id, points) in &yesterday_points:
            let representative_ts = points[0].timestamp
            let path = self.store.raw_partition_path(source_id, representative_ts)

            self.store.write_raw_parquet_overwrite(points.clone(), &path).await
                .map_err(|e| {
                    // CRITICAL: Do not lose yesterday's data.
                    // Re-insert the drained points back into the accumulator.
                    error!(
                        subscriber_id = %self.id,
                        source_id = %source_id,
                        error = %e,
                        "Day rollover Parquet write failed — returning points to accumulator"
                    )
                    e
                })?
            // NOTE: If this fails, the `?` propagates the error. The caller in the
            // select! loop will NOT clear the accumulator and will retry next tick.
            // The drain_for_date already removed the points from the accumulator, so
            // on failure we need to re-insert them. See error recovery below.

    // Step 3: Advance WAL watermark past all yesterday's entries.
    // Since all yesterday's entries have been snapshot, we can commit up to the
    // last sequence number that was assigned before today started.
    // We use the current wal_sequence — any entries added during this function
    // call (between drain and here) are for today and will have higher sequences.
    let commit_watermark = self.wal_sequence
    self.wal.commit_to(commit_watermark)?

    // Step 4: Update accumulator's active date
    // Points for today that arrived during the rollover window remain in the accumulator.
    // The drain_for_date only removed yesterday's points.
    self.accumulator.active_date = today

    info!(
        subscriber_id = %self.id,
        finalized_date = %yesterday,
        new_active_date = %today,
        wal_watermark = commit_watermark,
        "Day rollover complete"
    )

    return Ok(())
```

### 5.2 Error Recovery for Failed Day Rollover

If the Parquet write fails mid-rollover (e.g., disk full), we need to restore the drained points.

```
// The pattern for safe day rollover with restore-on-failure:

async fn finalize_day_safe(&mut self) -> CoreResult<()>:
    let yesterday = self.accumulator.active_date()
    let today = Utc::now().date_naive()

    if yesterday == today:
        return Ok(())

    let yesterday_points = self.accumulator.drain_for_date(yesterday)

    match self.write_day_snapshot(&yesterday_points).await:
        Ok(()) => {
            // Success — commit WAL and update date
            let commit_watermark = self.wal_sequence
            self.wal.commit_to(commit_watermark)?
            self.accumulator.active_date = today
            Ok(())
        }
        Err(e) => {
            // Failure — restore drained points back into accumulator
            warn!(
                subscriber_id = %self.id,
                error = %e,
                "Day rollover failed — restoring points to accumulator"
            )
            for (source_id, points) in yesterday_points:
                for point in points:
                    self.accumulator.add(point)
            // Do NOT update active_date. Do NOT commit WAL.
            // Next rollover attempt will retry.
            Err(e.into())
        }

async fn write_day_snapshot(
    &self,
    points_by_source: &HashMap<String, Vec<RawDataPoint>>,
) -> CoreResult<()>:
    for (source_id, points) in points_by_source:
        if points.is_empty():
            continue
        let path = self.store.raw_partition_path(source_id, points[0].timestamp)
        self.store.write_raw_parquet_overwrite(points.clone(), &path).await?
    Ok(())
```

---

## 6. Startup Recovery

### 6.1 `recover() -> CoreResult<()>`

Called at the beginning of `start()`, before the main `select!` loop. Rebuilds the in-memory accumulator from the last Parquet snapshot and WAL replay.

```
async fn recover(&mut self) -> CoreResult<()>:
    info!(subscriber_id = %self.id, "Starting Bronze recovery")

    let today = Utc::now().date_naive()
    self.accumulator = Accumulator::new(today)

    // Step 1: Find and seed from today's Parquet files (one per source_id).
    //
    // We need to discover which source_ids have Parquet files for today.
    // Use the raw partition directory structure:
    //   data/bronze/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet
    //
    // Walk the raw/ directory for today's date.
    let raw_base = self.store.base_path().join("raw")
    let mut total_seeded = 0

    if raw_base.exists():
        for stream_entry in std::fs::read_dir(&raw_base)?:
            let stream_entry = stream_entry?
            let stream_path = stream_entry.path()
            if !stream_path.is_dir():
                continue

            let today_parquet = stream_path
                .join(format!("year={}", today.year()))
                .join(format!("month={:02}", today.month()))
                .join(format!("day={:02}", today.day()))
                .join("data.parquet")

            if today_parquet.exists():
                match self.accumulator.seed_from_parquet(&today_parquet).await:
                    Ok(count) => {
                        total_seeded += count
                        debug!(
                            subscriber_id = %self.id,
                            path = %today_parquet.display(),
                            count = count,
                            "Seeded from Parquet"
                        )
                    }
                    Err(e) => {
                        // Non-fatal: Parquet file might be corrupt. WAL will fill gaps.
                        warn!(
                            subscriber_id = %self.id,
                            path = %today_parquet.display(),
                            error = %e,
                            "Failed to seed from Parquet — WAL replay will provide data"
                        )
                    }

    // Step 2: Replay WAL entries after the committed watermark.
    let watermark = self.wal.current_watermark()
    let wal_entries = self.wal.replay_since(watermark)?

    let wal_count = wal_entries.len()
    info!(
        subscriber_id = %self.id,
        parquet_seeded = total_seeded,
        wal_entries = wal_count,
        wal_watermark = watermark,
        "Recovery: replaying WAL"
    )

    // Step 3: Merge WAL entries into accumulator with dedup.
    self.accumulator.merge_wal_entries(wal_entries)

    // Step 4: Sync WAL sequence counter.
    // The WAL's next_sequence was already set during WriteAheadLog::new(),
    // but update our local tracking field.
    self.wal_sequence = self.wal.next_sequence() - 1  // last assigned, not next

    info!(
        subscriber_id = %self.id,
        total_points = self.accumulator.count(),
        sources = self.accumulator.source_count(),
        wal_sequence = self.wal_sequence,
        "Recovery complete"
    )

    return Ok(())
```

### 6.2 Edge Cases

**Empty WAL + Empty Parquet (fresh start):**
- `seed_from_parquet` finds no files, returns 0.
- `replay_since(0)` returns empty vec.
- Accumulator starts empty. Normal operation begins.

**Empty WAL + Existing Parquet (clean shutdown, no new data):**
- `seed_from_parquet` loads all today's points from Parquet.
- `replay_since(watermark)` returns empty vec (all entries were committed).
- Accumulator has exactly what Parquet has. Normal operation resumes.

**Non-empty WAL + Existing Parquet (crash after some events, before snapshot):**
- `seed_from_parquet` loads the last snapshot.
- `replay_since(watermark)` returns entries received after last snapshot.
- `merge_wal_entries` adds new entries, deduplicates any overlapping entries near the snapshot boundary.
- Accumulator has snapshot data + post-snapshot events. No data lost.

**Non-empty WAL + No Parquet (crash before first snapshot):**
- `seed_from_parquet` finds no file, returns 0.
- `replay_since(0)` returns all WAL entries.
- `merge_wal_entries` adds everything (no duplication because accumulator is empty).
- Accumulator rebuilt from WAL alone.

**WAL has entries for yesterday AND today (crash during day rollover):**
- `seed_from_parquet` loads today's Parquet (if any) and yesterday's Parquet.
- Actually, `seed_from_parquet` only loads today's date. Yesterday's data is in WAL.
- `replay_since(watermark)` returns all uncommitted entries including yesterday's.
- `merge_wal_entries` adds them. Accumulator now has mixed dates.
- On the next rollover timer tick, `finalize_day` will drain yesterday's points and write them.
- This is correct: no data lost, just a delayed rollover.

**Corrupted WAL trailing line (crash mid-write):**
- `replay_since` parses lines until it hits a corrupted one, then stops.
- All entries before the corrupted line are recovered.
- The single corrupted entry (partial write) is lost. This is at most one event.
- This matches the durability guarantee: WAL append returns Ok only after flush. If the process crashed before flush returned, the caller never got Ok, so the event was never "confirmed durable."

---

## 7. Error Handling Summary

### 7.1 Error Classification

| Error | Severity | Response | Data Impact |
|-------|----------|----------|-------------|
| WAL append fails | HIGH | Log error, skip point, increment errors_total | Single point lost (not durable) |
| WAL commit_to fails | MEDIUM | Log error, continue | WAL grows larger than needed; no data loss |
| Snapshot Parquet write fails | MEDIUM | Log error, retry next interval | WAL retains all entries; no data loss |
| Snapshot all-sources fail | MEDIUM | Do not commit WAL watermark | Full retry next interval |
| Day rollover Parquet write fails | HIGH | Restore drained points, retry next tick | No data loss |
| Day rollover WAL commit fails | MEDIUM | Log, continue; WAL larger than needed | No data loss |
| Recovery Parquet seed fails | LOW | Warn, continue; WAL replay fills gaps | No data loss |
| Recovery WAL replay fails | FATAL | Return error; cannot start | Process does not start |
| EventBus lagged | LOW | Log warning, continue | Lagged events lost (pre-existing behavior) |

### 7.2 Error Propagation Pattern

All methods use `CoreError` variants with `map_err` context:

```
// WAL errors → CoreError::Storage
wal.append(&point)
    .map_err(|e| CoreError::Storage(format!("WAL append for {} failed: {}", source_id, e)))

// Parquet errors → CoreError::Storage (existing pattern from parquet.rs)
store.write_raw_parquet_overwrite(points, &path).await
    .map_err(|e| CoreError::Storage(format!("Snapshot write for {} failed: {}", source_id, e)))

// SubscriberError wraps CoreError for the select! loop
// CoreError → SubscriberError::StorageError (existing pattern from bronze.rs:178)
```

### 7.3 Structured Tracing

All error paths use `tracing` macros with structured fields. The key fields for observability:

```
// Every WAL operation logs:
info!(subscriber_id, wal_sequence, wal_watermark, ...)

// Every snapshot logs:
info!(subscriber_id, points_snapshot, wal_watermark, source_id, path, ...)

// Every error logs:
error!(subscriber_id, error, source_id, ...)
```

---

## 8. Configuration

### 8.1 Updated `config/base/platform.yaml`

```yaml
subscribers:
  bronze:
    enabled: true
    batch_size: 100               # events before WAL batch flush (backpressure)
    flush_interval_secs: 30       # stats logging interval (was: Parquet flush)
    snapshot_interval_secs: 1800  # Parquet snapshot interval (default: 30 min)
    day_rollover_utc_hour: 0      # hour to finalize daily file (0 = midnight UTC)
    max_retries: 3                # retry count for snapshot Parquet writes
```

### 8.2 Default Values

| Field | Default | Rationale |
|-------|---------|-----------|
| `snapshot_interval_secs` | 1800 | 30 min balances Parquet freshness vs. write cost. At 4 streams, this is ~48 Parquet writes/day instead of ~2,880. |
| `day_rollover_utc_hour` | 0 | Midnight UTC. On a Pi in US timezones, this is 7-8 PM local, during low-activity hours. |
| `batch_size` | 100 | Kept for potential future use as WAL batch-flush threshold. Currently, WAL appends on every event. |

### 8.3 Serde Implementation

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct BronzeSubscriberConfig {
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    #[serde(default = "default_flush_interval_secs")]
    pub flush_interval_secs: u64,

    #[serde(default = "default_snapshot_interval_secs")]
    pub snapshot_interval_secs: u64,

    #[serde(default = "default_day_rollover_utc_hour")]
    pub day_rollover_utc_hour: u8,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    #[serde(default)]
    pub stream_filter: Vec<String>,
}

fn default_snapshot_interval_secs() -> u64 { 1800 }
fn default_day_rollover_utc_hour() -> u8 { 0 }
// existing: default_batch_size() -> 100, default_flush_interval_secs() -> 5,
//           default_max_retries() -> 3
```

---

## 9. Data Flow Diagrams

### 9.1 Hot Path (Event Receipt)

```
EventBus ─broadcast─> BronzeSubscriber.handle_point_v2()
                           │
                           ├─ stream filter check
                           │
                           ├─ WAL.append(point)        ← durability in milliseconds
                           │   │
                           │   ├─ Ok(seq) ──> accumulator.add(point)
                           │   │
                           │   └─ Err ──> log error, point dropped
                           │
                           └─ (return to select! loop)
```

### 9.2 Snapshot Path (Timer)

```
snapshot_timer.tick() ──> snapshot_to_parquet()
                              │
                              ├─ for each source_id in accumulator:
                              │     write_raw_parquet_overwrite(points, path)  ← full file write
                              │
                              ├─ wal.commit_to(current_sequence)  ← truncate committed entries
                              │
                              └─ (return to select! loop)
```

### 9.3 Day Rollover Path (Timer)

```
day_rollover_sleep ──> finalize_day()
                           │
                           ├─ accumulator.drain_for_date(yesterday)
                           │
                           ├─ write_day_snapshot(yesterday_points)  ← final Parquet for yesterday
                           │     │
                           │     ├─ Ok ──> wal.commit_to(watermark)
                           │     │         accumulator.active_date = today
                           │     │
                           │     └─ Err ──> restore drained points to accumulator
                           │                retry next tick
                           │
                           └─ compute_next_rollover_sleep()
```

### 9.4 Startup Recovery Path

```
start() ──> recover()
                │
                ├─ Accumulator::new(today)
                │
                ├─ for each stream in data/bronze/raw/*/today/:
                │     accumulator.seed_from_parquet(data.parquet)
                │
                ├─ wal.replay_since(watermark)
                │
                ├─ accumulator.merge_wal_entries(entries)  ← dedup by (source_id, timestamp)
                │
                └─ (enter select! loop)
```

---

## 10. Files Changed Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `core/src/storage/wal.rs` | **Rewrite** | WalEntry struct, sequence numbers, watermark header, `append()` returns `u64`, `replay_since(watermark)`, `commit_to(watermark)` with atomic rename |
| `core/src/storage/accumulator.rs` | **New file** | InMemoryAccumulator: add, seed_from_parquet, merge_wal_entries, drain_for_date, memory_estimate |
| `core/src/subscribers/bronze.rs` | **Major modify** | BronzeSubscriberConfig gains 2 fields, struct gains WAL+accumulator, `start()` gains 2 timer branches, `handle_point` replaced by `handle_point_v2`, new `snapshot_to_parquet()`, `finalize_day()`, `recover()`, `compute_next_rollover_sleep()` |
| `core/src/storage/parquet.rs` | **Minor modify** | Expose `write_raw_parquet_overwrite()` (alias for existing `write_raw_parquet`). Old `append_to_raw_parquet` remains but is no longer called by BronzeSubscriber. |
| `core/src/storage/mod.rs` | **Minor modify** | Add `pub mod accumulator;` |
| `config/base/platform.yaml` | **Minor modify** | Add `snapshot_interval_secs: 1800` and `day_rollover_utc_hour: 0` to bronze config |

---

## 11. Invariants and Correctness Properties

These properties must hold at all times during normal operation and after crash recovery:

1. **WAL superset invariant:** Every point in the accumulator has a corresponding WAL entry with sequence > committed_watermark. The reverse is also true: every WAL entry with sequence > committed_watermark is either in the accumulator or will be after merge_wal_entries.

2. **Parquet snapshot completeness:** After a successful `snapshot_to_parquet()`, the Parquet file for each source_id contains all points that were in the accumulator at the start of the snapshot. Points added during the snapshot are in the WAL but not yet in Parquet.

3. **WAL watermark monotonicity:** `committed_watermark` only increases. It never decreases. `commit_to(w)` is a no-op if `w <= committed_watermark`.

4. **Dedup correctness:** `merge_wal_entries` uses `(source_id, timestamp)` as the dedup key. This works because `chrono::Utc::now()` provides microsecond-resolution timestamps, and two events from the same source_id at the same microsecond is physically impossible given MQTT/HTTP latencies.

5. **Day rollover atomicity:** Either all of yesterday's points are written to Parquet and the WAL watermark advances, or none of them are (points restored to accumulator). There is no state where yesterday's data is partially written.

6. **Crash safety:** After any crash at any point in the code, the startup recovery sequence (seed from Parquet + replay WAL + dedup) produces an accumulator that contains all durably-written data (all points for which `wal.append()` returned `Ok`).
