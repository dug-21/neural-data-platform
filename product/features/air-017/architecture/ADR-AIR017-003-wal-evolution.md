# ADR-AIR017-003: WAL Evolution (Delete-All vs Watermark)

## Status

Proposed

## Context

The current WAL (`core/src/storage/wal.rs`) has three operations:

- `append(entry: &[u8])` -- write a JSON line, flush (lines 24-31)
- `replay() -> Vec<Vec<u8>>` -- read all lines (lines 34-46)
- `commit()` -- delete the file, recreate empty (lines 49-60)

The `commit()` method destroys all WAL entries unconditionally. This works in the current
architecture because `write_raw_batch()` calls WAL append, Parquet write, and WAL commit
in a single method. All WAL entries correspond to the batch being written, so deleting
all entries after the Parquet write is correct.

In the AIR-017 architecture, WAL entries accumulate continuously (one per event receipt)
and Parquet snapshots happen periodically (every 30-60 minutes). Between snapshots, the
WAL contains entries for data that has NOT yet been written to Parquet. A `commit()` that
deletes everything would destroy un-snapshot data.

The WAL must evolve to support partial commits: "commit entries up to here, keep the rest."

### Option A: Sequence-Numbered Entries + Truncate-to-Watermark

Each WAL entry gets a monotonically increasing sequence number. On snapshot, the current
highest sequence number becomes the watermark. `commit(watermark)` truncates entries
with `seq <= watermark` and retains entries with `seq > watermark`.

**WAL file format**:
```
{"seq":1,"ts":"2026-02-08T10:00:00Z","sid":"air-quality-Mqtt","data":{...}}
{"seq":2,"ts":"2026-02-08T10:00:08Z","sid":"air-quality-Mqtt","data":{...}}
{"seq":3,"ts":"2026-02-08T10:00:16Z","sid":"air-quality-Mqtt","data":{...}}
```

**Commit operation**: Read all entries, filter out `seq <= watermark`, rewrite the file
with remaining entries. Store the watermark.

**Startup recovery**: Read the watermark. Replay entries with `seq > watermark` into the
accumulator (these are entries received after the last successful snapshot).

Pros:
- Clean semantic: watermark defines the durability boundary.
- Replay-since-watermark is efficient (read file, skip entries below watermark).
- Sequence numbers provide total ordering, useful for debugging and auditing.
- Watermark can be stored in a separate small file (1 line, 8 bytes), avoiding the need
  to parse the WAL to determine recovery state.

Cons:
- `commit(watermark)` requires rewriting the WAL file (read remaining entries, write to
  temp file, rename). This is a file rewrite, but the WAL between snapshots is small
  (~450 KB for 30 minutes of data) so the cost is negligible.
- Sequence counter must be persisted or recovered from the WAL file on startup (read last
  entry's seq number).

### Option B: Per-Day WAL Files

Separate WAL files per day: `wal-2026-02-08.log`, `wal-2026-02-09.log`. Commit deletes
the previous day's WAL file entirely. Current day's WAL is always retained.

Pros:
- Simple commit: delete yesterday's file.
- No need to parse or rewrite WAL files.

Cons:
- Does not solve the within-day problem: a snapshot at 14:00 should commit entries
  from 00:00-14:00 but retain entries from 14:00 onward. Per-day files cannot represent
  this boundary.
- Day rollover edge cases with timezone/UTC alignment.
- Multi-file management adds complexity.
- SCOPE.md does not explicitly reject this, but the watermark approach from SCOPE.md
  section "WAL Evolution Required" describes within-day partial commits.

### Option C: Checkpoint File Alongside WAL

WAL entries are never deleted. A separate checkpoint file records the last committed
position (byte offset or line number). On replay, only entries after the checkpoint
position are loaded.

Pros:
- WAL file is append-only, never rewritten (simplest file I/O pattern).
- Checkpoint update is a single small write.

Cons:
- WAL file grows unboundedly through the day. At 500 bytes/entry and ~44,000 entries/day,
  this is ~22 MB/day. Acceptable for a single day, but the file must eventually be
  cleaned up.
- On day rollover, the WAL file must be deleted or rotated. This reintroduces file
  management complexity.
- Reading the full file and skipping entries is O(file_size) regardless of how many
  entries are relevant.
- The checkpoint position (byte offset) is fragile: if the WAL file is modified (e.g.,
  by a failed partial write creating a truncated last line), the byte offset may point
  to the middle of an entry.

## Decision

**Option A: Sequence-Numbered Entries + Truncate-to-Watermark, with Option C's
append-only characteristic during normal operation.**

The design combines the strengths of both approaches:

1. **Normal operation**: WAL is append-only. Each entry includes a `seq` number. No file
   rewrites during event receipt. This gives Option C's simple I/O.

2. **On snapshot commit**: Read all entries with `seq > watermark`, write to a new temp
   file, rename over the original. This truncates committed entries. The watermark is
   written to a companion file (`wal.watermark`). This gives Option A's clean semantics.

3. **On day rollover**: Final snapshot commits all entries for yesterday. WAL file is
   deleted and recreated (today's entries start from seq 1 or continue the global counter).

### WAL Entry Format

```
{"s":1,"d":{"timestamp":"2026-02-08T10:00:00Z","source_id":"air-quality-Mqtt","raw_payload":{...}}}
```

Fields:
- `s`: sequence number (u64, monotonically increasing)
- `d`: the full `RawDataPoint` serialized as JSON

The entry is a single JSON line, terminated by `\n`. The `writeln!` + `flush()` pattern
from the current WAL is retained for crash safety (partial last line is skipped on replay).

Short field names (`s`, `d`) minimize per-entry overhead. At ~10 bytes overhead per entry
(vs the ~500 byte payload), this is negligible.

### Watermark File

`wal.watermark` contains a single line:

```
42
```

This is the sequence number of the last committed entry. On startup:
1. Read `wal.watermark` to get the watermark (default 0 if file missing).
2. Replay WAL entries with `seq > watermark` into the accumulator.
3. Set the internal sequence counter to `max(seq from WAL entries)` or `watermark` if
   WAL is empty.

### Revised WriteAheadLog API

```rust
pub struct WriteAheadLog {
    path: PathBuf,
    watermark_path: PathBuf,
    file: File,
    seq_counter: u64,
    watermark: u64,
}

impl WriteAheadLog {
    /// Create or open a WAL. Reads watermark from companion file.
    pub fn new<P: AsRef<Path>>(path: P) -> CoreResult<Self>;

    /// Append an entry with the next sequence number. Returns the assigned seq.
    pub fn append(&mut self, point: &RawDataPoint) -> CoreResult<u64>;

    /// Replay all entries with seq > self.watermark.
    pub fn replay_since_watermark(&self) -> CoreResult<Vec<(u64, RawDataPoint)>>;

    /// Replay all entries (ignoring watermark). Used for emergency recovery.
    pub fn replay_all(&self) -> CoreResult<Vec<(u64, RawDataPoint)>>;

    /// Commit: advance watermark to the given seq, truncate committed entries.
    pub fn commit(&mut self, watermark: u64) -> CoreResult<()>;

    /// Current watermark value.
    pub fn watermark(&self) -> u64;

    /// Current sequence counter (next entry will get seq_counter + 1).
    pub fn seq_counter(&self) -> u64;

    /// WAL file size in bytes.
    pub fn file_size(&self) -> CoreResult<u64>;

    /// File path.
    pub fn path(&self) -> &Path;
}
```

### Commit Implementation

```
commit(new_watermark):
  1. Read WAL file line by line.
  2. Collect entries with seq > new_watermark into a Vec.
  3. Write collected entries to "{wal_path}.tmp".
  4. fsync the temp file.
  5. Rename "{wal_path}.tmp" -> "{wal_path}" (atomic on POSIX).
  6. Write new_watermark to "{watermark_path}.tmp".
  7. fsync the watermark temp file.
  8. Rename "{watermark_path}.tmp" -> "{watermark_path}".
  9. Reopen WAL file in append mode.
  10. Update self.watermark = new_watermark.
```

The rename operations are atomic on Linux (ext4, the Pi's filesystem). If the process
crashes between steps 5 and 8, the WAL has been truncated but the watermark has not been
updated. On restart, `replay_since_watermark()` will replay entries that were already
committed, causing duplicates in the accumulator. This is handled by the snapshot write
being idempotent (full overwrite, so re-inserting a few points into the accumulator and
then writing a snapshot produces the correct result).

## Consequences

**Positive:**
- WAL supports partial commits, enabling the snapshot-based architecture.
- Sequence numbers provide a total ordering for debugging, auditing, and dedup.
- Watermark file is trivially small and recoverable.
- Normal operation (event receipt) is append-only, no file rewrites.
- Commit truncation keeps the WAL file small (~450 KB between snapshots).

**Negative:**
- Commit requires a file rewrite (read + filter + write + rename). At ~450 KB per
  snapshot interval, this takes <1ms and is negligible.
- Two files to manage (WAL + watermark) instead of one. Both are in the same directory.
- The sequence counter starts at 0 on fresh start and recovers from WAL on restart.
  If the WAL is empty and the watermark file is missing, the counter starts at 0.
  This is correct but means sequence numbers are not globally unique across process
  lifetimes -- they reset on day rollover.

**Neutral:**
- The WAL entry format changes from bare JSON (`{"timestamp":...,"source_id":...}`) to
  wrapped JSON (`{"s":1,"d":{...}}`). This is a breaking change for the WAL file format,
  but WAL files are transient (deleted on commit) and not a stable API. On upgrade, any
  existing WAL file from the old format will fail to parse the `s` field. The safe upgrade
  path is: shut down cleanly (which commits the WAL), then deploy the new version.
- Existing tests for `WriteAheadLog` (11 tests in `core/src/storage/wal.rs:67-236`) will
  need to be rewritten for the new API.
