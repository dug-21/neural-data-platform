# AIR-018 Specification: Eliminate Polars from parquet.rs

> **Feature ID:** air-018
> **SPARC Phase:** Specification
> **Author:** ndp-architect
> **Created:** 2026-02-10
> **Status:** Draft
> **Depends on:** air-017 Phase 1 (deployed v1.1.18)
> **Related:** BUG-004 (Bronze memory leak), ADR-001

---

## 1. Problem Summary

Polars 0.35 DataFrame create/drop cycles in `core/src/storage/parquet.rs` leak approximately 4.5 MiB per 30-minute snapshot cycle on Raspberry Pi 5 (kernel 6.14+, glibc malloc, cgroup v2). At 48 cycles/day this exhausts a 512 MiB Docker container within 24-36 hours. Alternative allocators (jemalloc, mimalloc) crash on this platform. The fix is to remove Polars entirely from the write and read paths in `parquet.rs`, replacing it with the lower-level `arrow` and `parquet` crates that Polars itself depends on transitively.

---

## 2. Functional Requirements

### FR-01: Replace `write_parquet()` (line 90)

**Current:** Builds 6 Polars `Series`, creates a `DataFrame`, writes via `ParquetWriter::new(file).with_compression(ParquetCompression::Snappy).finish(&mut df)`.

**New:** Build 6 Arrow arrays (`Int64Array`, `StringArray`, `Float64Array`), construct a `RecordBatch` from an `Arc<Schema>`, write via `parquet::arrow::ArrowWriter::new(file, schema, Some(props))` with Snappy compression properties, then `writer.write(&batch)` + `writer.close()`.

**Preserve:** `spawn_blocking` wrapper, pre-allocated `Vec::with_capacity(len)`, same column names and types.

### FR-02: Replace `write_raw_parquet()` (line 512)

**Current:** Builds 5 Polars `Series`, creates a `DataFrame`, writes via `ParquetWriter::new(file).with_compression(ParquetCompression::Snappy).finish(&mut df)`.

**New:** Build 5 Arrow arrays (`Int64Array`, `StringArray`), construct a `RecordBatch` from an `Arc<Schema>`, write via `ArrowWriter` with Snappy compression, then `writer.write(&batch)` + `writer.close()`.

**Preserve:** `spawn_blocking` wrapper, pre-allocated `Vec::with_capacity(len)`, same column names and types.

### FR-03: Replace `append_to_parquet()` (line 157)

**Current:** Opens existing Parquet file with Polars `ParquetReader::new(file).finish()`, extracts columns via `df.column("name")?.i64()?` / `.utf8()?` / `.f64()?`, iterates rows to reconstruct `TimeSeriesPoint` structs.

**New:** Open file with `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?.build()?`, iterate `RecordBatch` results, downcast columns via `batch.column(idx).as_any().downcast_ref::<Int64Array>()` (and similar for `StringArray`, `Float64Array`), reconstruct `TimeSeriesPoint` structs. Handle nullable columns (`ndp_id`, `context`) via `.is_null(i)` checks.

### FR-04: Replace `append_to_raw_parquet()` (line 581)

**Current:** Same Polars `ParquetReader` pattern as FR-03 but with 5-column raw schema.

**New:** Same `ParquetRecordBatchReaderBuilder` pattern as FR-03 but with 5-column raw schema. Handle nullable columns (`ndp_id`, `context`) via `.is_null(i)` checks.

**Note:** This method is `#[deprecated]` but still called by `write_raw()`. It must continue to function correctly.

### FR-05: Replace `query()` (line 278)

**Current:** Opens Parquet file with Polars `ParquetReader`, applies `.lazy().filter(col("timestamp").gt_eq(...).and(...)).collect()`, then extracts columns.

**New:** Open with `ParquetRecordBatchReaderBuilder`, iterate `RecordBatch` results, manually filter rows where `timestamp_array.value(i) >= start_micros && timestamp_array.value(i) <= end_micros`. No lazy evaluation -- direct iteration with conditional push.

**Rationale:** The Polars lazy filter adds DataFrame overhead for what is a simple range comparison. Direct iteration on Arrow arrays is both simpler and avoids DataFrame allocation.

### FR-06: Replace `query_raw()` (line 761)

**Current:** Opens Parquet files with Polars `ParquetReader`, extracts 5 columns, applies manual time and source filters in a loop.

**New:** Same `ParquetRecordBatchReaderBuilder` pattern with manual filtering in the loop. This method already does manual filtering -- only the reader changes.

### FR-07: Dependency Changes in `core/Cargo.toml`

**Remove:**
```toml
polars = { workspace = true }
```

**Add:**
```toml
arrow = { version = "57", default-features = false, features = ["chrono-tz"] }
parquet = { version = "57", default-features = false, features = ["snap"] }
```

**Version rationale:** Polars 0.35 transitively depends on `arrow` 57.1.0 and `parquet` 57.1.0 (confirmed in `Cargo.lock`). Using version 57 directly ensures schema compatibility and shrinks the dependency tree by removing the Polars intermediary. The `snap` feature enables Snappy compression. The `chrono-tz` feature enables chrono integration for timestamp handling. Using `default-features = false` minimizes the footprint.

**Workspace update:** Add `arrow` and `parquet` entries to `[workspace.dependencies]` in root `Cargo.toml`. Remove the `polars` entry if no other workspace members use it (check: `silver-etl` and `air-quality-app` dev-deps may still reference it).

### FR-08: Schema Compatibility

Parquet output files MUST be bit-for-bit schema-compatible with current output:
- Same column names, in the same order
- Same physical Parquet types (INT64, BYTE_ARRAY, DOUBLE)
- Same logical types (no logical timestamp annotation -- raw INT64 microseconds)
- Same compression: Snappy
- Same nullability for nullable columns (ndp_id, context)

Verification: A test must write data with the new code and read it back with the `parquet` crate's metadata API to confirm column names, types, and compression match the existing schema.

### FR-09: Preserve BUG-004 Diagnostics

The `malloc_trim(0)` call in `core/src/subscribers/bronze.rs` (line 249) and surrounding RSS diagnostic logging MUST remain untouched. These are needed to verify the fix eliminates the chunk leak and to monitor the separate slow creep leak.

### FR-10: Test Migration

All tests in `core/src/storage/parquet.rs` that currently use Polars `ParquetReader` for read-side assertions (e.g., `test_raw_parquet_schema_has_5_columns` at line 1583) MUST be rewritten to use the `parquet` crate's `ParquetRecordBatchReader` or metadata API instead. No test may import `polars::prelude::*`.

---

## 3. Schema Mapping

### 3.1 TimeSeriesPoint Schema (6 columns)

| # | Column | Polars Type | Arrow Type | Parquet Physical | Nullable | Notes |
|---|--------|-------------|------------|------------------|----------|-------|
| 0 | `timestamp` | `Series::new("timestamp", Vec<i64>)` | `Int64Array` | INT64 | No | Microseconds since epoch |
| 1 | `location_id` | `Series::new("location_id", Vec<String>)` | `StringArray` | BYTE_ARRAY (UTF8) | No | |
| 2 | `metric` | `Series::new("metric", Vec<String>)` | `StringArray` | BYTE_ARRAY (UTF8) | No | From `tags["metric"]`, defaults to `"unknown"` |
| 3 | `value` | `Series::new("value", Vec<f64>)` | `Float64Array` | DOUBLE | No | |
| 4 | `ndp_id` | `Series::new("ndp_id", Vec<Option<String>>)` | `StringArray` | BYTE_ARRAY (UTF8) | Yes | `Option<String>` maps to nullable `StringArray` |
| 5 | `context` | `Series::new("context", Vec<Option<String>>)` | `StringArray` | BYTE_ARRAY (UTF8) | Yes | JSON-serialized `serde_json::Value` |

**Arrow Schema definition:**
```rust
use arrow::datatypes::{DataType, Field, Schema};
use std::sync::Arc;

fn timeseries_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("timestamp", DataType::Int64, false),
        Field::new("location_id", DataType::Utf8, false),
        Field::new("metric", DataType::Utf8, false),
        Field::new("value", DataType::Float64, false),
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),
    ]))
}
```

### 3.2 RawDataPoint Schema (5 columns)

| # | Column | Polars Type | Arrow Type | Parquet Physical | Nullable | Notes |
|---|--------|-------------|------------|------------------|----------|-------|
| 0 | `timestamp` | `Series::new("timestamp", Vec<i64>)` | `Int64Array` | INT64 | No | Microseconds since epoch |
| 1 | `source_id` | `Series::new("source_id", Vec<String>)` | `StringArray` | BYTE_ARRAY (UTF8) | No | |
| 2 | `ndp_id` | `Series::new("ndp_id", Vec<Option<String>>)` | `StringArray` | BYTE_ARRAY (UTF8) | Yes | |
| 3 | `context` | `Series::new("context", Vec<Option<String>>)` | `StringArray` | BYTE_ARRAY (UTF8) | Yes | JSON-serialized |
| 4 | `raw_payload` | `Series::new("raw_payload", Vec<String>)` | `StringArray` | BYTE_ARRAY (UTF8) | No | JSON-serialized |

**Arrow Schema definition:**
```rust
fn raw_data_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("timestamp", DataType::Int64, false),
        Field::new("source_id", DataType::Utf8, false),
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),
        Field::new("raw_payload", DataType::Utf8, false),
    ]))
}
```

---

## 4. Method-by-Method Replacement Plan

### 4.1 `write_parquet()` -- Write Path (TimeSeriesPoint)

| Step | Current (Polars) | New (arrow-rs) |
|------|-----------------|----------------|
| Collect data | `Vec<i64>`, `Vec<String>`, `Vec<f64>`, `Vec<Option<String>>` | Same -- no change to collection loop |
| Build columns | `Series::new("timestamp", timestamps)` | `Arc::new(Int64Array::from(timestamps)) as ArrayRef` |
| Build columns | `Series::new("location_id", location_ids)` | `Arc::new(StringArray::from(location_ids)) as ArrayRef` |
| Build columns | `Series::new("metric", metrics)` | `Arc::new(StringArray::from(metrics)) as ArrayRef` |
| Build columns | `Series::new("value", values)` | `Arc::new(Float64Array::from(values)) as ArrayRef` |
| Build columns | `Series::new("ndp_id", ndp_ids)` (Option<String>) | `Arc::new(StringArray::from(ndp_ids)) as ArrayRef` (arrow handles Option natively) |
| Build columns | `Series::new("context", contexts)` (Option<String>) | `Arc::new(StringArray::from(contexts)) as ArrayRef` |
| Create container | `DataFrame::new(vec![...])` | `RecordBatch::try_new(schema, vec![...])` |
| Write file | `ParquetWriter::new(file).with_compression(Snappy).finish(&mut df)` | See write pattern below |

**Arrow write pattern:**
```rust
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

let props = WriterProperties::builder()
    .set_compression(Compression::SNAPPY)
    .build();

let file = std::fs::File::create(&path)?;
let mut writer = ArrowWriter::try_new(file, schema.clone(), Some(props))
    .map_err(|e| CoreError::Storage(format!("Failed to create ArrowWriter: {}", e)))?;
writer.write(&batch)
    .map_err(|e| CoreError::Storage(format!("Failed to write RecordBatch: {}", e)))?;
writer.close()
    .map_err(|e| CoreError::Storage(format!("Failed to close Parquet writer: {}", e)))?;
```

### 4.2 `write_raw_parquet()` -- Write Path (RawDataPoint)

Same pattern as 4.1 but with the 5-column raw schema. The collection loop is identical in structure.

### 4.3 `append_to_parquet()` -- Read Path (TimeSeriesPoint)

| Step | Current (Polars) | New (arrow-rs) |
|------|-----------------|----------------|
| Open file | `ParquetReader::new(file).finish()` returns `DataFrame` | `ParquetRecordBatchReaderBuilder::try_new(file)?.build()?` returns `impl Iterator<Item = Result<RecordBatch>>` |
| Get column | `df.column("timestamp")?.i64()?` | `batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap()` |
| Get column | `df.column("location_id")?.utf8()?` | `batch.column(1).as_any().downcast_ref::<StringArray>().unwrap()` |
| Get column | `df.column("metric")?.utf8()?` | `batch.column(2).as_any().downcast_ref::<StringArray>().unwrap()` |
| Get column | `df.column("value")?.f64()?` | `batch.column(3).as_any().downcast_ref::<Float64Array>().unwrap()` |
| Get nullable | `df.column("ndp_id").ok().and_then(\|c\| c.utf8().ok())` | `batch.column(4).as_any().downcast_ref::<StringArray>()` (nullable via `.is_null(i)`) |
| Get nullable | `df.column("context").ok().and_then(\|c\| c.utf8().ok())` | `batch.column(5).as_any().downcast_ref::<StringArray>()` (nullable via `.is_null(i)`) |
| Row count | `df.height()` | `batch.num_rows()` |
| Get value | `timestamps.get(i)` returns `Option<i64>` | `ts_array.value(i)` returns `i64` (non-nullable) |
| Get value | `location_ids.get(i)` returns `Option<&str>` | `loc_array.value(i)` returns `&str` (non-nullable) |
| Get nullable | `ndp_ids.and_then(\|col\| col.get(i).map(...))` | `if !ndp_id_array.is_null(i) { Some(ndp_id_array.value(i).to_string()) } else { None }` |

**Arrow read pattern:**
```rust
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use arrow::array::{Int64Array, Float64Array, StringArray};

let file = std::fs::File::open(path)?;
let reader = ParquetRecordBatchReaderBuilder::try_new(file)
    .map_err(|e| CoreError::Storage(format!("Failed to open Parquet reader: {}", e)))?
    .build()
    .map_err(|e| CoreError::Storage(format!("Failed to build Parquet reader: {}", e)))?;

for batch_result in reader {
    let batch = batch_result
        .map_err(|e| CoreError::Storage(format!("Failed to read batch: {}", e)))?;

    let ts_array = batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap();
    let loc_array = batch.column(1).as_any().downcast_ref::<StringArray>().unwrap();
    // ... etc

    for i in 0..batch.num_rows() {
        let ts = ts_array.value(i);
        let loc = loc_array.value(i);
        // ... reconstruct TimeSeriesPoint
    }
}
```

### 4.4 `append_to_raw_parquet()` -- Read Path (RawDataPoint)

Same pattern as 4.3 but with 5-column raw schema. Column indices:
- 0: timestamp (Int64Array)
- 1: source_id (StringArray)
- 2: ndp_id (StringArray, nullable)
- 3: context (StringArray, nullable)
- 4: raw_payload (StringArray)

### 4.5 `query()` -- Filtered Read Path (TimeSeriesPoint)

**Current filter logic (Polars lazy):**
```rust
df = df.lazy()
    .filter(col("timestamp").gt_eq(lit(start_micros)).and(col("timestamp").lt_eq(lit(end_micros))))
    .collect()?;
```

**New filter logic (manual on RecordBatch):**
```rust
let start_micros = start.timestamp_micros();
let end_micros = end.timestamp_micros();

for batch_result in reader {
    let batch = batch_result?;
    let ts_array = batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap();
    // ... other columns

    for i in 0..batch.num_rows() {
        let ts = ts_array.value(i);
        if ts >= start_micros && ts <= end_micros {
            // reconstruct and push TimeSeriesPoint
        }
    }
}
```

This is simpler and avoids DataFrame creation entirely.

### 4.6 `query_raw()` -- Filtered Read Path (RawDataPoint)

The current code already applies manual time and source filters in a loop. The only change is replacing `ParquetReader::new(file).finish()` with `ParquetRecordBatchReaderBuilder` and changing column access from Polars `.column()?.i64()?` to Arrow `downcast_ref`.

---

## 5. Import Changes

### Remove
```rust
use polars::prelude::*;
```

### Add
```rust
use arrow::array::{ArrayRef, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
```

---

## 6. Files Changed

| File | Change Type | Description |
|------|-------------|-------------|
| `core/src/storage/parquet.rs` | Modified | Replace all Polars API calls with arrow-rs + parquet crate equivalents |
| `core/Cargo.toml` | Modified | Remove `polars = { workspace = true }`, add `arrow` + `parquet` |
| `Cargo.toml` (workspace root) | Modified | Add `arrow` + `parquet` to `[workspace.dependencies]`; remove `polars` if unused elsewhere |
| `core/src/subscribers/bronze.rs` | Not changed | `malloc_trim(0)` and diagnostic logging remain as-is |
| `core/src/traits.rs` | Not changed | Store and RawStore trait signatures are untouched |
| `core/src/types/raw_data_point.rs` | Not changed | RawDataPoint struct is untouched |

---

## 7. Constraints

| Constraint | Rationale |
|------------|-----------|
| Parquet file schema identical to current output | Silver ETL, MCP server, and Grafana read these files. Schema drift would break downstream consumers. |
| ARM64 compatible (aarch64-unknown-linux-gnu) | Target is Raspberry Pi 5. `arrow` and `parquet` crates are pure Rust. |
| No new C library dependencies | Prevents cross-compilation issues. Snappy compression via the `snap` crate (pure Rust). |
| `spawn_blocking` preserved for all write and heavy read operations | Parquet serialization/deserialization is CPU-intensive; must not block the tokio async runtime. |
| No changes to trait signatures (`Store`, `RawStore`) | Other crates depend on these traits. Changing them would trigger cross-crate propagation (see pattern ID 24). |
| No changes to `bronze.rs` behavior | BUG-004 diagnostic logging stays for post-fix verification. |
| Docker memory limit remains 512 MiB | The fix must work within the existing constraint, not raise the limit. |

---

## 8. Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Arrow nullable handling differs from Polars | Medium | Parquet schema mismatch | FR-08 compatibility test compares schemas byte-for-byte |
| RecordBatch column order differs from DataFrame column order | Low | Wrong data in wrong columns | Use named schema construction; validate column names in tests |
| Existing Parquet files written by Polars cannot be read by arrow reader | Very Low | Data loss on upgrade | arrow-rs reads standard Parquet; add migration test reading Polars-written files |
| Performance regression on read path (no lazy filter pushdown) | Low | Slower queries | Current queries scan full partition files anyway; lazy filter was DataFrame-level, not predicate-pushdown |
| Polars still needed by other workspace members | Medium | Cannot remove from workspace deps | Check silver-etl and air-quality-app; keep workspace dep if needed, remove only from core |

---

## 9. Acceptance Criteria

### AC-01: All existing tests pass
All 874+ tests across the workspace continue to pass. No test regressions.

### AC-02: Schema compatibility verified
New test: write a TimeSeriesPoint batch with the new code, read the Parquet file metadata, and assert:
- Column count is 6
- Column names are `["timestamp", "location_id", "metric", "value", "ndp_id", "context"]` in that order
- Column types match expected Parquet physical types
- Compression is Snappy

New test: same verification for the 5-column RawDataPoint schema.

### AC-03: Polars removed from core dependencies
`polars` does not appear in `core/Cargo.toml` `[dependencies]`. It may remain in workspace-level deps if other crates need it.

### AC-04: Binary size reduction measured
Measure the `air-quality-app` binary size before and after. Document the delta. Expected: significant reduction since Polars pulls in substantial transitive dependencies.

### AC-05: Round-trip data integrity
New tests write data with the new code and read it back, verifying:
- All TimeSeriesPoint fields (timestamp, location_id, value, tags/metric, ndp_id, context)
- All RawDataPoint fields (timestamp, source_id, ndp_id, context, raw_payload)
- Nullable fields (None values survive round-trip)
- JSON context survives serialization round-trip

### AC-06: BUG-004 memory leak eliminated
After deployment to Pi, the BUG-004 diagnostic logging in `bronze.rs` should show:
- `polars_delta` metric drops to near-zero (no DataFrame allocation overhead)
- RSS growth per snapshot cycle reduces from +4.5 MiB to near-zero
- Container no longer OOMs within 36 hours

This is verified post-deployment, not in CI. The diagnostic logging remains in place for verification.

### AC-07: Cross-read compatibility
New test: create a Parquet file with the current Polars-based code (in a dev-dependency test helper), then read it with the new arrow-based code. This verifies that existing on-disk Parquet files produced by previous versions are readable after the upgrade.

---

## 10. Test Plan

### 10.1 Existing Tests to Migrate (Polars imports removed)

These tests in `core/src/storage/parquet.rs` currently use `polars::prelude::ParquetReader` for assertions:

| Test | Line | Change Required |
|------|------|-----------------|
| `test_raw_parquet_schema_has_5_columns` | 1583 | Replace `ParquetReader::new(file).finish()` with `ParquetRecordBatchReaderBuilder`; assert column names from schema metadata |

All other tests use the `Store`/`RawStore` trait methods for assertions (write then query), so they do not directly import Polars and will work unchanged once the implementation is swapped.

### 10.2 New Tests to Add

| Test | Purpose |
|------|---------|
| `test_timeseries_parquet_schema_metadata` | Write TimeSeriesPoint batch, open file with `parquet::file::reader::SerializedFileReader`, verify 6 columns, names, types, Snappy compression |
| `test_raw_parquet_schema_metadata` | Same for 5-column RawDataPoint schema |
| `test_timeseries_nullable_round_trip` | Write mix of Some/None ndp_id and context, read back, verify nulls preserved |
| `test_raw_nullable_round_trip` | Same for RawDataPoint |
| `test_empty_batch_write_no_file` | Verify `write_parquet` and `write_raw_parquet` with empty Vec create no file |
| `test_cross_read_compatibility` | (If feasible) Write with Polars in dev-dep, read with arrow-rs, verify data matches |

### 10.3 Integration Verification (Post-Deploy)

On the Pi deployment (not CI):
1. Deploy the new binary
2. Monitor `bronze.rs` RSS diagnostic logs for 2+ hours
3. Verify `polars_delta` drops to near-zero per snapshot cycle
4. Verify container RSS stays stable (no unbounded growth)
5. Verify Silver ETL continues to ingest from Bronze Parquet files without error

---

## 11. Implementation Order

| Step | Description | Estimated Effort |
|------|-------------|------------------|
| 1 | Add `arrow` + `parquet` to workspace and core Cargo.toml | Small |
| 2 | Define `timeseries_schema()` and `raw_data_schema()` helper functions | Small |
| 3 | Replace `write_parquet()` (FR-01) | Medium |
| 4 | Replace `write_raw_parquet()` (FR-02) | Medium |
| 5 | Replace `append_to_parquet()` read path (FR-03) | Medium |
| 6 | Replace `append_to_raw_parquet()` read path (FR-04) | Medium |
| 7 | Replace `query()` with manual filter (FR-05) | Medium |
| 8 | Replace `query_raw()` reader (FR-06) | Medium |
| 9 | Migrate `test_raw_parquet_schema_has_5_columns` (FR-10) | Small |
| 10 | Add new schema compatibility tests (AC-02) | Small |
| 11 | Remove `polars` from `core/Cargo.toml` and fix any remaining imports (FR-07) | Small |
| 12 | Update workspace `Cargo.toml` if polars is unused elsewhere | Small |
| 13 | Run full test suite, measure binary size delta (AC-04) | Small |

Steps 3-8 can be done incrementally -- each method replacement is independently testable via the existing test suite. The Polars import stays until all methods are converted (step 11).

---

## 12. Out of Scope

Per SCOPE.md:
- Removing Polars from `apps/air-quality-app` dev-dependencies or test code
- Removing Polars from Silver ETL or other crates
- Alternative allocator investigation (documented as failed in SCOPE.md)
- Fixing the slow creep leak (0.7 MiB/30min) -- separate investigation
- WAL or accumulator architecture changes (air-017 Phases 2-3)

---

## 13. Patterns Applied

| Pattern ID | Name | How Applied |
|------------|------|-------------|
| 26 | `architecture:bug-fix-wal-only-bronze` | Confirmed BUG-004 root cause is Polars DataFrame alloc, not WAL. This spec replaces the alloc source. |
| 23 | `testing:bronze-integration-with-parquet-wal` | Test strategy follows existing pattern: tempdir, write, read-back, assert. |
| 24 | `troubleshoot:struct-field-missing-across-crates` | Constraint: no trait signature changes to avoid cross-crate propagation. |
