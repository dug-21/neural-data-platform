# air-018: Dependency Analysis -- Polars to arrow-rs Migration

**Date:** 2026-02-10
**Feature:** air-018
**Related ADR:** ADR-001-replace-polars-with-arrow-rs.md

## Current Polars Feature Set Used

The workspace-level Polars dependency is defined in the root `Cargo.toml`:

```toml
polars = { version = "0.35", features = ["parquet", "lazy", "dtype-datetime"] }
```

### Feature Mapping: What Each Feature Provides to `core`

| Polars Feature | What It Enables in `parquet.rs` | arrow-rs / parquet Replacement |
|----------------|-------------------------------|-------------------------------|
| `parquet` | `ParquetReader`, `ParquetWriter`, `ParquetCompression::Snappy` | `parquet::arrow::ArrowWriter`, `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder`, `parquet::basic::Compression::SNAPPY`, `parquet::file::properties::WriterProperties` |
| `lazy` | `df.lazy().filter(col("timestamp").gt_eq(...)).collect()` in `query()` | Row-level `if ts >= start && ts <= end` check during iteration (no lazy evaluation needed at current data volumes) |
| `dtype-datetime` | Not directly used in `parquet.rs` -- enables Polars datetime dtype support | Not needed. Timestamps are stored as `i64` microseconds, not as Polars datetime. `chrono` handles conversion. |

### Polars API Surface Used in `core/src/storage/parquet.rs`

| Polars API | Occurrences | arrow-rs Equivalent |
|------------|-------------|---------------------|
| `Series::new("name", vec)` | 11 (6 in `write_parquet`, 5 in `write_raw_parquet`) | `Arc::new(Int64Array::from(vec))`, `Arc::new(StringArray::from(vec))`, `Arc::new(Float64Array::from(vec))` |
| `DataFrame::new(vec![series...])` | 2 | `RecordBatch::try_new(schema, vec![arrays...])` |
| `ParquetWriter::new(file).with_compression(...).finish(&mut df)` | 2 | `ArrowWriter::try_new(file, schema, Some(props))` + `writer.write(&batch)` + `writer.close()` |
| `ParquetReader::new(file).finish()` | 5 (append_to_parquet, query, append_to_raw_parquet, query_raw, test) | `ParquetRecordBatchReaderBuilder::try_new(file)?.build()?` |
| `df.column("name")?.i64()?` | 4 | `batch.column_by_name("name")?.as_any().downcast_ref::<Int64Array>()` |
| `df.column("name")?.utf8()?` | 12 | `batch.column_by_name("name")?.as_any().downcast_ref::<StringArray>()` |
| `df.column("name")?.f64()?` | 2 | `batch.column_by_name("name")?.as_any().downcast_ref::<Float64Array>()` |
| `df.column("name").ok().and_then(\|c\| c.utf8().ok())` | 6 (nullable columns) | `batch.column_by_name("name").and_then(\|c\| c.as_any().downcast_ref::<StringArray>())` |
| `df.height()` | 4 | `batch.num_rows()` |
| `chunked_array.get(i)` | ~20 | `array.is_null(i)` + `array.value(i)` |
| `df.lazy().filter(...).collect()` | 1 (in `query()`) | Row-level `if` check during iteration |
| `df.get_column_names()` | 1 (in test) | `batch.schema().fields().iter().map(\|f\| f.name())` |
| `ParquetCompression::Snappy` | 2 | `Compression::SNAPPY` via `WriterProperties::builder().set_compression(...)` |

### Polars API Surface Used in `core/src/error.rs`

| Polars API | Occurrences | Replacement |
|------------|-------------|-------------|
| `polars::error::PolarsError` | 1 (`From` impl) | `parquet::errors::ParquetError` and `arrow::error::ArrowError` |
| `CoreError::Polars(String)` | 1 (variant definition) | Rename to `CoreError::Parquet(String)` |

## Transitive Dependency Reduction

### Polars 0.35 Transitive Dependency Tree (Relevant Crates)

When `polars` is a direct dependency of `core`, it pulls in these top-level Polars crates:

```
polars v0.35
  polars-core
  polars-io (includes parquet feature)
    arrow2 (Polars uses its own Arrow implementation, NOT apache arrow-rs)
    parquet2
  polars-lazy (lazy evaluation engine)
    polars-plan
    polars-pipe
    polars-ops
  polars-time (for dtype-datetime)
  polars-utils
```

Key observation: **Polars 0.35 uses `arrow2` and `parquet2`**, which are forks of the Apache `arrow` and `parquet` crates. This means the workspace currently compiles BOTH:
1. `arrow2` + `parquet2` (via Polars)
2. `arrow` + `parquet` (if any other dependency uses the Apache versions)

After removing Polars from core, only the standard Apache `arrow` + `parquet` crates are compiled for core.

### Estimated Crate Reduction for `core` Build

| Category | Crates Removed | Examples |
|----------|---------------|----------|
| Polars core | ~8 | `polars`, `polars-core`, `polars-io`, `polars-lazy`, `polars-plan`, `polars-pipe`, `polars-ops`, `polars-utils` |
| Polars time | ~2 | `polars-time`, `polars-arrow` |
| Arrow2 ecosystem | ~5 | `arrow2`, `parquet2`, `arrow2-convert`, plus format-specific sub-crates |
| Polars internal utilities | ~10 | `polars-json`, `polars-row`, `polars-ffi`, `polars-error`, `polars-compute`, etc. |
| Shared transitive deps (unique to Polars) | ~15-25 | Various utility crates only pulled in via Polars |
| **Total estimated** | **~40-50** | |

Note: The exact count depends on which crates are shared with other workspace members. Since `silver-etl` and `air-quality-app` still have Polars in `[dev-dependencies]`, those crates continue to compile Polars for their test builds. However, the **production binary** for `air-quality-app` (which is what runs on the Pi) will no longer link against Polars since `core` is the only runtime dependency that used it.

### Crates Added

| Crate | Purpose | Transitive Deps |
|-------|---------|-----------------|
| `arrow v54` (direct) | `RecordBatch`, typed arrays (`Int64Array`, `StringArray`, `Float64Array`), `Schema`, `Field`, `DataType` | Minimal -- `arrow-array`, `arrow-buffer`, `arrow-data`, `arrow-schema`, `arrow-cast` (most are zero or few transitive deps) |
| `parquet v54` (direct) | `ArrowWriter`, `ParquetRecordBatchReaderBuilder`, `WriterProperties`, `Compression` | `arrow` (shared), `snap` (for Snappy), `thrift` (for Parquet metadata) |

The Apache `arrow` and `parquet` crates have a much smaller dependency footprint than Polars because they are focused libraries rather than a full DataFrame engine.

## Binary Size Impact

### Estimated Savings

| Metric | Before (with Polars) | After (arrow-rs only) | Delta |
|--------|---------------------|----------------------|-------|
| ARM64 release binary size | ~45-55 MB | ~30-40 MB | -15 to -20 MB |
| Compile time (core crate) | ~120-180s | ~60-90s | -40% to -50% |
| Number of crates compiled (core) | ~180-220 | ~130-160 | -40-60 crates |

These are estimates based on typical Polars overhead. Actual measurements should be taken before and after the change using:

```bash
# Binary size
ls -la target/aarch64-unknown-linux-gnu/release/air-quality-app

# Crate count
cargo build --release -p platform-core 2>&1 | grep Compiling | wc -l

# Compile time
cargo build --release -p platform-core --timings
```

### Why the Reduction Is Significant on Pi

The Raspberry Pi 5 has:
- 4 GB or 8 GB RAM (shared with GPU)
- SD card or NVMe storage (slower than desktop SSD)
- ARM Cortex-A76 cores (capable but not x86-class)

A 15-20 MB binary size reduction:
- Reduces Docker image layer size (faster `git pull` + rebuild on Pi)
- Reduces memory-mapped binary footprint at runtime
- Reduces cold-start time (less binary to load from storage)

## Polars Features NOT Used by `core`

The following Polars capabilities are enabled by the workspace features but not used anywhere in `core/src/storage/parquet.rs`:

| Feature/Capability | Available via Polars | Used in core? | Notes |
|--------------------|---------------------|---------------|-------|
| `dtype-datetime` | Polars datetime column type | No | Timestamps are `i64` micros, not Polars datetime |
| Lazy join/groupby | `df.lazy().groupby()` | No | Aggregation is done manually in `aggregate()` |
| String operations | `str.contains()`, `str.replace()` | No | |
| Window functions | `over()`, `rolling_mean()` | No | |
| CSV/JSON I/O | `CsvReader`, `JsonReader` | No | Only Parquet I/O is used |
| Null handling | `fill_null()`, `drop_nulls()` | No | Null handling is manual via `Option` |
| Sorting | `df.sort()` | No | Sorting is done on `Vec<AggregatedPoint>` |
| Pivot/melt | `df.pivot()`, `df.melt()` | No | |

This confirms that Polars is dramatically over-provisioned for the actual usage in `core`. The crate acts as a simple Parquet I/O library with column extraction -- exactly what the `arrow` + `parquet` crates provide natively.

## Migration Checklist for Implementor

1. Add `arrow` and `parquet` to `core/Cargo.toml` `[dependencies]`
2. Remove `polars = { workspace = true }` from `core/Cargo.toml` `[dependencies]`
3. Replace `use polars::prelude::*` with specific arrow-rs and parquet imports in `parquet.rs`
4. Rewrite `write_parquet()` -- Series/DataFrame to RecordBatch/ArrowWriter
5. Rewrite `write_raw_parquet()` -- same pattern as above
6. Rewrite `append_to_parquet()` read path -- ParquetReader to ParquetRecordBatchReader
7. Rewrite `query()` read path -- ParquetReader + lazy filter to ParquetRecordBatchReader + row filter
8. Rewrite `append_to_raw_parquet()` read path
9. Rewrite `query_raw()` read path
10. Update `error.rs` -- remove `From<PolarsError>`, add `From<ParquetError>` and `From<ArrowError>`
11. Update test module -- replace `ParquetReader` in `test_raw_parquet_schema_has_5_columns` and any test using Polars to read back files
12. Run full test suite: `cargo test -p platform-core`
13. Verify schema compatibility: write with new code, read with old code (or compare column metadata)
14. Measure binary size before/after
15. Measure RSS behavior in integration environment
