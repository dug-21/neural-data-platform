# ADR-AIR017-004: Snapshot Strategy (Overwrite vs Append vs Multi-File)

## Status

Proposed

## Context

The current Bronze layer writes Parquet files using a read-modify-write pattern
(`append_to_raw_parquet()` at `core/src/storage/parquet.rs:563-622`). On every flush:

1. Open the existing daily Parquet file (e.g., `raw/air-quality/year=2026/month=02/day=08/data.parquet`).
2. Read and deserialize every row into `Vec<RawDataPoint>`.
3. Append the new batch to the Vec.
4. Rewrite the entire file with all rows.

This is O(file_size) per flush. At the end of a day with 11,000 points (~5.5 MB Parquet),
each flush reads 5.5 MB, deserializes ~11,000 rows, and rewrites 5.5 MB. With flushes
every 30 seconds, this is 2,880 full-file rewrites per day per stream.

AIR-017 replaces this with an in-memory accumulator that holds the full day's data. The
question is how the accumulator writes to Parquet.

### Option A: Full Overwrite from Accumulator

Write the entire day's data from the accumulator as a new Parquet file, overwriting the
existing file. No read required.

Pros:
- Simplest possible write path: serialize all points, write file, done.
- No read dependency -- write cost is O(day's data), independent of existing file.
- File is always internally consistent (no partial updates, no footer corruption).
- Idempotent: writing the same accumulator twice produces the same file.

Cons:
- Writes the full day's data on every snapshot, even if only a few new points arrived.
  At ~5.5 MB per stream and 4 streams, each snapshot writes ~22 MB.
  With 30-minute intervals, that is ~48 snapshots/day x 22 MB = ~1 GB/day of writes.
  On the Pi's SD card / USB SSD, this is acceptable (modern SSDs handle TB/day).
- If the write fails mid-way, the previous file is lost (overwrite). Mitigated by
  writing to a temp file and renaming atomically.

### Option B: Parquet Row Group Append

Open the existing Parquet file, append a new row group with only the new points, update
the footer. Existing data is not re-read or re-written.

Pros:
- Write cost is O(new points only).
- Disk I/O is minimal.

Cons:
- **SCOPE.md explicitly rejects this**: "Parquet row group append / footer rewrite
  (complexity and corruption risk on Pi)" is listed under Out of Scope.
- Parquet footer rewrite is complex and error-prone. A crash during footer rewrite
  corrupts the entire file.
- Multiple row groups per file increase read complexity for downstream consumers.
- Polars' `ParquetWriter` does not natively support appending row groups to existing files.
  Would require switching to `arrow-rs` with low-level Parquet APIs.

### Option C: Multi-File per Day (Delta/Iceberg style)

Instead of one Parquet file per day, write a new Parquet file for each snapshot. The day's
directory contains multiple files: `data-001.parquet`, `data-002.parquet`, etc. Reads
merge all files.

Pros:
- Each write is O(new points only).
- No risk of corrupting previous files.
- Natural append semantics.

Cons:
- **SCOPE.md explicitly rejects this**: "Sidecar files / multi-file-per-day approaches
  (explicitly rejected by this design)" is listed under Out of Scope.
- Read path must merge multiple files, increasing query complexity.
- Compaction needed to prevent file count from growing unboundedly.
- Changes the Bronze data format contract (one file per day per stream per data type).

### Option D: Differential Snapshot (Write Only New Points)

Track which points have been snapshot since the last write. Write only the delta.
The Parquet file is still a single file, so this is effectively Option A but with a
"last snapshot position" in the accumulator to identify the delta, followed by a
read-modify-write of only the delta.

Cons:
- Still requires reading the existing file to merge with the delta.
- Reintroduces the read-modify-write that AIR-017 exists to eliminate.

## Decision

**Option A: Full Overwrite from Accumulator.**

The SCOPE.md explicitly rejects Options B and C. Option D reintroduces read-modify-write.
Option A is the only approach that eliminates read-modify-write entirely while staying
within SCOPE constraints.

### Implementation

The snapshot write follows this sequence:

```
snapshot_write(stream_id):
  1. points = accumulator.get_stream(stream_id)    // Clone, ~22 MiB for the largest stream
  2. path = store.raw_partition_path(stream_id, today)
  3. tmp_path = path.with_extension("parquet.tmp")
  4. store.write_raw_parquet(points, &tmp_path)     // Existing method, writes full file
  5. std::fs::rename(tmp_path, path)                // Atomic on POSIX
```

Step 5 (atomic rename) ensures that a crash during the write does not corrupt the
existing Parquet file. The old file is only replaced after the new file is fully written
and fsynced. If the process crashes between steps 4 and 5, the `.parquet.tmp` file is
an orphan that will be overwritten on the next snapshot.

### Write Amplification Analysis

| Metric | Current (read-modify-write) | AIR-017 (full overwrite) |
|--------|-----------------------------|--------------------------|
| Frequency | Every 30s (2,880/day) | Every 30 min (48/day) |
| Per-write I/O | Read ~5.5 MB + Write ~5.5 MB = ~11 MB | Write ~5.5 MB (no read) |
| Daily total I/O | 2,880 x 11 MB = **~31.7 GB/day** | 48 x 5.5 MB = **~264 MB/day** |
| Reduction | -- | **120x less I/O** |

The AIR-017 approach writes 120x less data to disk per day despite writing the full file
on each snapshot. The reduction comes from the dramatically lower snapshot frequency
(48/day vs 2,880/day) and the elimination of the read phase.

### Crash Safety

| Failure Point | Consequence | Recovery |
|---------------|-------------|----------|
| Crash during `write_raw_parquet` (step 4) | `.parquet.tmp` is partial | Ignored on restart; accumulator rebuilt from Parquet + WAL replay |
| Crash after `write_raw_parquet` but before `rename` (between 4 and 5) | `.parquet.tmp` is complete but not visible | Same as above; overwritten on next snapshot |
| Crash after `rename` (step 5) but before WAL commit | Parquet is up-to-date; WAL replays some entries that are already in Parquet | Accumulator contains duplicates; next snapshot overwrites with clean data |
| Power loss during `rename` | On ext4 with default mount options, rename is atomic (metadata journaling) | File is either the old version or the new version, never corrupt |

## Consequences

**Positive:**
- Read-modify-write is completely eliminated. No Parquet file is ever opened for reading
  during a write operation.
- Daily disk I/O drops from ~31.7 GB to ~264 MB (120x reduction).
- Write path is dead simple: serialize, write, rename. No merge logic.
- Parquet file is always internally consistent -- never partially updated.
- Atomic rename provides crash safety without complex recovery logic.

**Negative:**
- Each snapshot writes the full day's data, even if only 1 new point arrived since the
  last snapshot. In the worst case (no events but snapshot timer fires), this writes an
  identical file. The write cost at ~5.5 MB is trivial.
- The Parquet file is stale by up to one snapshot interval (30-60 minutes). Data received
  between snapshots is in the WAL and accumulator but not in Parquet. This is the core
  trade-off of AIR-017 and is addressed in Phase 3 (read path integration).
- The temp file (`.parquet.tmp`) consumes additional disk space during the write (~5.5 MB
  transient). This is negligible on the Pi's storage.

**Neutral:**
- The `write_raw_parquet()` method in `ParquetStore` is reused as-is. It already writes
  a complete Parquet file from a `Vec<RawDataPoint>`. The only change is that it is
  called from `BronzeSubscriber` (via the `RawStore` trait) instead of from
  `append_to_raw_parquet()`.
- The `append_to_raw_parquet()` and `append_to_parquet()` methods are not deleted in
  Phase 1. They remain for backward compatibility and are candidates for removal in a
  future cleanup.
