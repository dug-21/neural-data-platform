# ADR-001: Replace Polars with arrow-rs + parquet crate

**Status:** Proposed
**Date:** 2026-02-10
**Feature:** air-018
**Deciders:** [User], Architecture Agent

## Context

BUG-004 identified a memory leak in the Bronze layer's Parquet write path. On Raspberry Pi 5 running Ubuntu 25.x (kernel 6.14+) in a 512 MiB Docker container, the Polars DataFrame create/drop cycle leaks approximately 4.5 MiB per 30-minute snapshot cycle. At 48 cycles/day this adds ~216 MiB/day, causing an OOM kill within 24-36 hours.

The leak originates from glibc malloc's inability to return fragmented heap pages after Polars' internal Arrow buffer allocation and DataFrame drop. Calling `malloc_trim(0)` after each cycle reclaims ~91% of the spike but leaves a persistent 4.5 MiB residual per cycle that accumulates indefinitely.

Two alternative allocators were tested and both failed on this specific platform:

- **jemalloc** (`tikv-jemallocator 0.6`): Uses `MADV_FREE` on kernel 6.14+, which cgroup v2 still counts as RSS, triggering false OOM. Additionally, `background_thread:true` can deadlock during init in Docker containers with restricted `/sys` access. Result: data processing froze for 13+ minutes.
- **mimalloc** (`mimalloc 0.1`): Container exited silently during data processing (not during startup). Failure mode not fully understood. Result: 0B memory / 0 PIDs in Docker stats.

Both allocators work during the low-allocation startup phase; they fail during the high-allocation Polars write operations. The page size (4096 bytes) rules out the ARM64 large-page mismatch theory. **Do not retry alternative allocators without first verifying the specific kernel/cgroup interaction on the target Pi.**

The sole Polars usage in the `core` crate is confined to `core/src/storage/parquet.rs` and a `From<PolarsError>` impl in `core/src/error.rs`. No other core source file imports or depends on Polars directly.

## Decision

Replace all Polars usage in `core/src/storage/parquet.rs` with direct `arrow` (v54) and `parquet` (v54) crate usage. Remove `polars` from `core/Cargo.toml` `[dependencies]`. Remove the `From<PolarsError>` conversion in `core/src/error.rs` (the `CoreError::Polars` variant can remain for backward compatibility or be renamed to a general `Parquet` variant).

### What Changes

| Component | Before | After |
|-----------|--------|-------|
| Write path (`write_parquet`) | `Series::new` + `DataFrame::new` + `ParquetWriter` | `arrow::array::*` builders + `RecordBatch` + `parquet::arrow::ArrowWriter` |
| Write path (`write_raw_parquet`) | Same Polars pattern | Same arrow-rs pattern |
| Read path (`append_to_parquet`) | `ParquetReader::new(file).finish()` + `df.column().utf8()` | `parquet::arrow::arrow_reader::ParquetRecordBatchReader` + `arrow::array` downcasts |
| Read path (`query`) | `ParquetReader` + `df.lazy().filter().collect()` | `ParquetRecordBatchReader` + row-level `if` timestamp filter |
| Read path (`append_to_raw_parquet`) | `ParquetReader` + column extraction | `ParquetRecordBatchReader` + array downcasts |
| Read path (`query_raw`) | `ParquetReader` + column extraction | `ParquetRecordBatchReader` + array downcasts |
| Error handling (`error.rs`) | `From<polars::error::PolarsError>` | `From<parquet::errors::ParquetError>` + `From<arrow::error::ArrowError>` |
| Test assertions | `ParquetReader` for schema verification | `ParquetRecordBatchReader` for schema verification |
| `core/Cargo.toml` | `polars = { workspace = true }` | `arrow = { version = "54", features = ["chrono-tz"] }` + `parquet = { version = "54", features = ["snap"] }` |

### What Does NOT Change

- Parquet file schema (column names, types, nullability) -- must be bit-for-bit compatible
- Trait signatures: `Store`, `RawStore` (all public method signatures unchanged)
- `spawn_blocking` pattern in write methods
- WAL and accumulator architecture (air-017 Phase 1)
- `malloc_trim(0)` call in `bronze.rs` (harmless, useful for diagnostics)
- Workspace-level `polars` definition in root `Cargo.toml` (still used by `silver-etl` and `air-quality-app` dev-deps)
- Diagnostic logging in `bronze.rs` (RSS tracking stays for production verification)

## Alternatives Considered

### Alternative 1: Write path only (Option A)

Replace only `write_parquet()` and `write_raw_parquet()`. Keep read paths on Polars.

- **Pro**: Minimal blast radius. Only the hot write path changes.
- **Con**: Polars stays as a `[dependencies]` entry in core. Two serialization styles in one file (arrow-rs for writes, Polars for reads). Maintenance burden of keeping both idioms consistent.
- **Rejected**: The read paths use the same Polars patterns (DataFrame column extraction, `utf8()`, `i64()`, `f64()` downcasts). Having two styles in one file creates cognitive overhead. The dependency stays, so binary size and transitive dep count do not improve.

### Alternative 2: Full replacement in parquet.rs (Option B) -- CHOSEN

Replace both write AND read paths. Remove `polars` from `core/Cargo.toml` `[dependencies]` entirely.

- **Pro**: Clean cut. One less heavy dependency. Smaller binary (~15-20 MB less on ARM64). Consistent code style throughout `parquet.rs`. Simpler transitive dependency tree. The `arrow` and `parquet` crates are already pulled in transitively by Polars, so this is a strict reduction.
- **Con**: Larger changeset (all methods in `parquet.rs` touched). More test updates required. The arrow-rs API is more verbose than Polars for simple operations.
- **Chosen**: The read path rewrite is mechanical -- each Polars column extraction maps directly to an arrow-rs array downcast. The additional verbosity is acceptable given the benefits.

### Alternative 3: Full workspace elimination (Option C)

Also remove Polars from `apps/air-quality-app` and `apps/silver-etl` dev-dependencies.

- **Pro**: Polars entirely gone from the workspace.
- **Con**: Much larger scope. `silver-etl` test code uses Polars in test modules (lines 1211, 1473 in `etl.rs`). `air-quality-app` uses it in `dp004_pipeline_integration.rs` line 380. Reworking all test code is scope creep for a BUG-004 fix.
- **Rejected**: Those usages are in `[dev-dependencies]` only and do not affect the production binary. They can be addressed in a separate cleanup task.

### Alternative 4: Alternative allocator (jemalloc/mimalloc)

Both tested and failed on Pi 5 / kernel 6.14+ / cgroup v2.

- **jemalloc**: `MADV_FREE` + cgroup v2 = false OOM. `background_thread` deadlock in Docker.
- **mimalloc**: Silent container death during data processing.
- **Rejected**: Platform incompatibility documented in SCOPE.md. Do not retry without kernel/cgroup verification.

## Consequences

### Positive

- **BUG-004 memory leak fixed**: Direct `RecordBatch` construction uses a simpler allocation pattern than Polars DataFrames, producing less heap fragmentation. The create/write/drop cycle does not leave persistent residual allocations.
- **Binary size reduction**: Removing Polars as a direct dependency eliminates ~50+ transitive crates from the `core` build. Estimated ARM64 binary size reduction: 15-20 MB (Polars pulls in `arrow2`, `polars-core`, `polars-io`, `polars-lazy`, `polars-ops`, etc.).
- **Fewer transitive dependencies**: The `arrow` and `parquet` crates are already transitive dependencies of Polars. After this change, `core` depends on them directly with no intermediary. Dependency audit surface is smaller.
- **Consistent code style**: All Parquet I/O in `parquet.rs` uses one idiom (arrow-rs arrays and RecordBatch), not a mix of Polars DataFrame and raw arrow.
- **Pure Rust**: No new C library dependencies introduced. Both `arrow` and `parquet` crates are pure Rust.
- **Compile time improvement**: Fewer crates to compile for the `core` build.

### Negative

- **Larger initial changeset**: Every method in `parquet.rs` that touches Polars is rewritten. This is approximately 8 methods plus the test module (~1900 lines total, ~600 lines of production code, ~1300 lines of tests).
- **arrow-rs API verbosity**: Building a `RecordBatch` from vectors requires explicit `Arc<dyn Array>` construction and schema definition. Polars' `Series::new` + `DataFrame::new` is more concise. This is a one-time cost -- once written, the code is straightforward.
- **Test assertion updates**: Tests that use `ParquetReader` to verify output must switch to `ParquetRecordBatchReader`. The schema verification test (`test_raw_parquet_schema_has_5_columns`) uses `df.get_column_names()` which becomes `record_batch.schema().fields()`.
- **`CoreError::Polars` variant**: The error variant name becomes slightly misleading after removing Polars. Options: rename to `CoreError::Parquet`, or keep the variant name for backward compatibility and rename in a separate cleanup. The `From<PolarsError>` impl must be removed and replaced with `From<ParquetError>` and `From<ArrowError>`.

### Neutral

- `malloc_trim(0)` in `bronze.rs` remains. It is harmless and provides useful diagnostic logging. It can be removed in a future cleanup once BUG-004 is confirmed fixed in production.
- WAL and accumulator architecture unchanged. air-017 Phase 1 is orthogonal.
- Trait signatures (`Store`, `RawStore`) unchanged. No downstream API impact.

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Schema incompatibility breaks Silver ETL | Low | High | Schema compatibility test: write with new code, read with `ParquetRecordBatchReader`, verify identical column names, types, and nullability. Compare against a fixture file written by old code. |
| Performance regression in write path | Low | Medium | Benchmark before/after with identical data volume (1000 points). The write path is I/O-bound (Snappy compression + disk write), not CPU-bound on array construction. |
| Performance regression in read path | Low | Low | Read path is used only for WAL recovery and `query()` which is not the hot path (hot path is `write_raw_batch` via `BronzeSubscriber`). |
| Test failures from API changes | Medium | Low | Mechanical translation. Every Polars API call maps to a well-documented arrow-rs equivalent. Tests are comprehensive (1900+ lines). |
| Memory leak not fully resolved | Low | Medium | Keep BUG-004 RSS diagnostics in `bronze.rs`. Verify in production that per-cycle residual drops to near zero. If residual persists, the leak source is elsewhere (the 0.7 MiB/30min slow creep is already documented as a separate investigation). |
| `CoreError::Polars` variant breaks downstream callers | Low | Low | The variant is only constructed via `From<PolarsError>` which is internal to core. No external crate matches on `CoreError::Polars` by name. Rename to `CoreError::Parquet` in the same changeset. |

## Dependencies

### Added to `core/Cargo.toml`

```toml
arrow = { version = "54", default-features = false, features = ["chrono-tz"] }
parquet = { version = "54", default-features = false, features = ["snap", "arrow"] }
```

The `snap` feature enables Snappy compression (matching current Polars behavior). The `arrow` feature on the parquet crate enables `ArrowWriter` and `ParquetRecordBatchReader`. The `chrono-tz` feature on arrow enables timezone-aware timestamp conversion.

`default-features = false` is used to minimize the transitive dependency footprint on the resource-constrained Pi target.

### Removed from `core/Cargo.toml`

```toml
polars = { workspace = true }
```

### Unchanged

- Workspace-level `polars` definition in root `Cargo.toml` stays (used by `silver-etl` and `air-quality-app` in `[dev-dependencies]`)
- All other `core` dependencies unchanged

## Implementation Notes

### Write Path Translation

Both `write_parquet` (6-column TimeSeriesPoint schema) and `write_raw_parquet` (5-column RawDataPoint schema) follow the same pattern:

**Before (Polars):**
```rust
let timestamp_series = Series::new("timestamp", timestamps);
// ... more Series ...
let mut df = DataFrame::new(vec![series1, series2, ...])?;
let file = std::fs::File::create(&path)?;
ParquetWriter::new(file)
    .with_compression(ParquetCompression::Snappy)
    .finish(&mut df)?;
```

**After (arrow-rs):**
```rust
use arrow::array::{Int64Array, Float64Array, StringArray, ArrayRef};
use arrow::datatypes::{Schema, Field, DataType};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

let schema = Arc::new(Schema::new(vec![
    Field::new("timestamp", DataType::Int64, false),
    // ... more fields ...
]));

let batch = RecordBatch::try_new(schema, vec![
    Arc::new(Int64Array::from(timestamps)) as ArrayRef,
    // ... more arrays ...
])?;

let file = std::fs::File::create(&path)?;
let props = WriterProperties::builder()
    .set_compression(Compression::SNAPPY)
    .build();
let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
writer.write(&batch)?;
writer.close()?;
```

### Read Path Translation

**Before (Polars):**
```rust
let file = std::fs::File::open(&path)?;
let df = ParquetReader::new(file).finish()?;
let timestamps = df.column("timestamp")?.i64()?;
let location_ids = df.column("location_id")?.utf8()?;
for i in 0..df.height() {
    if let Some(ts) = timestamps.get(i) { ... }
}
```

**After (arrow-rs):**
```rust
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use arrow::array::{Int64Array, StringArray, Float64Array};

let file = std::fs::File::open(&path)?;
let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
let reader = builder.build()?;

for batch in reader {
    let batch = batch?;
    let timestamps = batch.column_by_name("timestamp")
        .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
    let location_ids = batch.column_by_name("location_id")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>());
    // iterate rows ...
}
```

### Query Filter Translation

The `query()` method currently uses Polars lazy filter:
```rust
df = df.lazy()
    .filter(col("timestamp").gt_eq(lit(start)).and(col("timestamp").lt_eq(lit(end))))
    .collect()?;
```

This becomes a simple row-level check during iteration:
```rust
if let Some(ts) = timestamps.value(i) {
    if ts >= start_micros && ts <= end_micros {
        // include this row
    }
}
```

This is acceptable because:
1. Partition files are daily -- a single file contains at most ~2880 points (one per 30 seconds for 24 hours). Row-level filtering of 2880 rows is trivially fast.
2. The `query()` method is not the hot path. It is used for WAL recovery and ad-hoc queries.
3. For future optimization, Parquet row group statistics could be used for predicate pushdown, but this is not needed at current data volumes.

### Nullable Column Handling

The `ndp_id` and `context` columns are nullable. In Polars, these are represented as `Option<&str>` in `ChunkedArray<Utf8Type>`. In arrow-rs, nullable strings use the null bitmap in `StringArray`:

```rust
// Building nullable array
let ndp_ids: Vec<Option<&str>> = points.iter()
    .map(|p| p.ndp_id.as_deref())
    .collect();
let ndp_id_array = StringArray::from(ndp_ids);

// Reading nullable values
if ndp_id_array.is_null(i) {
    None
} else {
    Some(ndp_id_array.value(i).to_string())
}
```

### `spawn_blocking` Pattern Preserved

Both write methods use `tokio::task::spawn_blocking` to avoid blocking the async runtime. This pattern is preserved exactly as-is. The closure moves the path and points into the blocking thread, constructs the RecordBatch, writes it, and returns `CoreResult<()>`.

### Error Handling Changes

`core/src/error.rs` changes:

1. Remove `From<polars::error::PolarsError>` impl
2. Rename `CoreError::Polars(String)` to `CoreError::Parquet(String)` (or add a new `Parquet` variant and deprecate `Polars`)
3. Add `From<parquet::errors::ParquetError>` impl
4. Add `From<arrow::error::ArrowError>` impl

All existing `.map_err(|e| CoreError::Storage(...))` calls in `parquet.rs` can remain as-is since they already convert to the `Storage` variant.

### Test Code Changes

The test module in `parquet.rs` has one test (`test_raw_parquet_schema_has_5_columns`) that reads back a Parquet file using `ParquetReader` to verify the schema. This test switches to:

```rust
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

let file = std::fs::File::open(&path).unwrap();
let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
let reader = builder.build().unwrap();
let batch = reader.into_iter().next().unwrap().unwrap();
let field_names: Vec<&str> = batch.schema().fields().iter().map(|f| f.name().as_str()).collect();
assert_eq!(field_names.len(), 5);
```

All other tests exercise the public API (`write`, `write_batch`, `query`, `write_raw`, `query_raw`) and do not directly reference Polars types. They will work without changes once the underlying implementation is swapped.
