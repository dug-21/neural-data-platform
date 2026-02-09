# AIR-016: Parquet Append-Only Writes (Eliminate Read-Modify-Write)

> **Feature ID:** air-016
> **Created:** 2026-02-06
> **Status:** Scoping
> **Phase:** air (Foundation / Core)

---

## Problem Statement

The `ParquetStore` in `core/src/storage/parquet.rs` uses a read-modify-write pattern for appends. Every 30-second flush cycle reads the entire daily Parquet file into memory, deserializes every row back to Rust structs, appends new points, then rewrites the entire file. Data exists in memory 3x simultaneously during the write path.

The air-quality-app container is at 462 MiB / 512 MiB (90%) and will eventually OOM-kill.

### How append_to_parquet Works Today

```
append_to_parquet(new_points, path):
  1. Read existing file into Polars DataFrame        ← full file in memory
  2. Deserialize every row into Vec<TimeSeriesPoint>  ← 2nd copy
  3. Append new_points to the Vec
  4. Build column Vecs from all points                ← 3rd copy
  5. Build new DataFrame from column Vecs             ← 4th representation
  6. Overwrite file with new DataFrame
```

The same pattern exists in `append_to_raw_parquet` for raw data points.

### Memory Impact

With daily files growing throughout the day (thousands of rows from 4 MQTT streams at ~30s intervals), peak memory for a single flush spikes to 50-100+ MiB late in the day. This happens twice per cycle (parsed + raw files), every 30 seconds.

### Files Affected

| File | Lines | Function |
|------|-------|----------|
| `core/src/storage/parquet.rs` | 157-225 | `append_to_parquet()` — parsed TimeSeriesPoint |
| `core/src/storage/parquet.rs` | 563-619 | `append_to_raw_parquet()` — raw RawDataPoint |
| `core/src/storage/parquet.rs` | 90-154 | `write_parquet()` — builds DataFrame, writes file |
| `core/src/storage/parquet.rs` | 510-559 | `write_raw_parquet()` — same pattern for raw |

---

## Desired Outcome

Replace read-modify-write with append-only Parquet writes. New points are written as a new row group appended to the existing file. No existing data is read back into memory.

### Target Memory Profile

| Component | Before | After |
|-----------|--------|-------|
| Parquet flush (late day) | 50-100+ MiB | < 5 MiB |
| Peak container memory | ~460 MiB | ~200 MiB |
| Memory growth over day | Linear with file size | Constant |

---

## Approach

Parquet files support multiple row groups. Instead of read-deserialize-append-rewrite, open the file in append mode and write a new row group containing only the new points.

### Option A: arrow-rs / parquet crate (append row group)

The `parquet` crate (part of arrow-rs) supports `SerializedFileWriter` which can append row groups to an existing file. This is the standard Rust approach for Parquet append.

Trade-off: Requires replacing Polars `ParquetWriter`/`ParquetReader` with lower-level arrow-rs APIs for the write path. Read path (used by Silver ETL) can stay on Polars.

### Option B: Polars sink mode

Polars supports `ParquetWriter::new(file).with_row_group_size()` but does NOT support appending to an existing file — it always overwrites.

Not viable without a workaround (e.g., writing to a temp file then concatenating).

### Option C: Per-flush file, periodic compaction

Write each flush as a separate small Parquet file (e.g., `2026-02-06_001.parquet`, `2026-02-06_002.parquet`). Periodically compact into a single daily file (e.g., at midnight or on a background timer).

Trade-off: Simple implementation, no new dependencies. Read path needs to glob multiple files per partition. Compaction adds complexity.

---

## Constraints

- WAL (Write-Ahead Log) must continue to work as-is — it is separate from the Parquet write path
- The `Store` trait interface (`write`, `write_batch`, `query`, `query_raw`) must not change
- Read path (`query`, `query_raw`, `find_partitions`) must continue to work with the new file format
- Silver ETL reads Parquet files from the Bronze path — must remain compatible
- No new runtime dependencies (keep binary size reasonable for Pi)

---

## Out of Scope

- MQTT unbounded cache fix (separate issue, separate feature)
- EventBus capacity tuning
- Polars removal (may be a future feature, but not this one)
- Silver ETL changes (unless required for read compatibility)
