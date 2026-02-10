# AIR-018 Refinement: TDD Implementation Plan for Polars Replacement

> **Phase:** SPARC Refinement (R)
> **Feature:** air-018 (Eliminate Polars from Bronze Write Path)
> **Author:** ndp-rust-dev agent
> **Date:** 2026-02-10
> **Status:** Draft
> **Depends on:** Specification, Pseudocode (P-01 through P-06), ADR-001, TEST-STRATEGY

---

## 1. TDD Implementation Cycles

The replacement is divided into 9 ordered TDD cycles. Each cycle follows Red-Green-Refactor.
The implementor MUST complete cycles in order because later cycles depend on earlier ones
compiling. The Polars import (`use polars::prelude::*`) remains until Cycle 8 removes it;
during Cycles 1-7, both Polars and arrow-rs imports coexist temporarily.

### Cycle 1: Foundation -- Cargo.toml + error.rs + imports

**Goal:** Add arrow-rs and parquet crate dependencies. Migrate error types. Compilation must
succeed with both Polars and arrow-rs present temporarily.

**Red:**
```
cargo check -p platform-core
```
Must compile with `arrow` and `parquet` as dependencies and the new `CoreError::Arrow` variant.
At this stage all existing code still uses Polars, so the check should pass unchanged.

**Green:**

1. Add to workspace `Cargo.toml` (`[workspace.dependencies]`):
   ```toml
   arrow = { version = "54", default-features = false, features = ["prettyprint"] }
   parquet = { version = "54", default-features = false, features = ["arrow", "snap"] }
   ```

2. Add to `core/Cargo.toml` (`[dependencies]`):
   ```toml
   arrow = { workspace = true }
   parquet = { workspace = true }
   ```
   Keep `polars = { workspace = true }` for now (removed in Cycle 8).

3. Update `core/src/error.rs`:
   - Rename `CoreError::Polars(String)` to `CoreError::Arrow(String)`
   - Change `#[error("Polars error: {0}")]` to `#[error("Arrow error: {0}")]`
   - Remove `impl From<polars::error::PolarsError> for CoreError`
   - Add `impl From<arrow::error::ArrowError> for CoreError`
   - Add `impl From<parquet::errors::ParquetError> for CoreError`

4. Add new imports to `core/src/storage/parquet.rs` (below existing `use polars::prelude::*`):
   ```rust
   use arrow::array::{ArrayRef, Float64Array, Int64Array, StringArray};
   use arrow::datatypes::{DataType, Field, Schema};
   use arrow::record_batch::RecordBatch;
   use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
   use parquet::arrow::ArrowWriter;
   use parquet::basic::Compression;
   use parquet::file::properties::WriterProperties;
   ```

5. Add two private helper functions in `parquet.rs` (inside `impl ParquetStore`, above
   `write_parquet`):
   ```rust
   fn timeseries_schema() -> Arc<Schema> { ... }
   fn raw_data_schema() -> Arc<Schema> { ... }
   ```
   As defined in PSEUDOCODE.md schemas section.

6. Add two private helper functions for nullable column reads:
   ```rust
   fn read_nullable_string(col: Option<&StringArray>, i: usize) -> Option<String> { ... }
   fn read_nullable_json(col: Option<&StringArray>, i: usize) -> Option<serde_json::Value> { ... }
   ```

**Verify:** `cargo check -p platform-core` passes. No test runs yet -- just compilation.

**Refactor:** None needed.

**Risk note:** Removing `From<PolarsError>` will break any code that relies on `?` auto-converting
Polars errors. In `parquet.rs`, the `query()` method at lines 307-310 uses `df.column("timestamp")?.i64()?`
which auto-converts via `From<PolarsError>`. This is fine because those lines will be rewritten
in Cycle 5. During the transition, `cargo check` will fail on those lines. To fix this temporarily,
wrap those conversions in explicit `.map_err(|e| CoreError::Storage(e.to_string()))` calls,
OR defer the `From<PolarsError>` removal until Cycle 8 (recommended approach below).

**RECOMMENDED APPROACH:** Keep `From<PolarsError>` during Cycles 1-7. In Cycle 8, after all
methods are converted, remove it along with the Polars dependency. This avoids transient
compilation failures between cycles.

**Revised Cycle 1 error.rs changes:**
- ADD `CoreError::Arrow(String)` variant (new variant, alongside `Polars`)
- ADD `From<arrow::error::ArrowError>` and `From<parquet::errors::ParquetError>` impls
- KEEP `CoreError::Polars(String)` and `From<PolarsError>` until Cycle 8

---

### Cycle 2: `write_raw_parquet()` -- the hot path (BUG-004 fix)

**Goal:** Replace the most critical method first. This is called every 30-minute snapshot
cycle by `BronzeSubscriber`. It is the primary source of the memory leak.

**Red:**
```
cargo test -p platform-core storage::parquet::tests::test_write_raw_batch
cargo test -p platform-core storage::parquet::tests::test_write_raw_batch_empty_succeeds
```
These existing tests exercise `write_raw_batch` which calls `write_raw_parquet`. They should
pass before the change (baseline) and must pass after.

**Green:**

Replace the body of `write_raw_parquet()` (lines 512-570) with the arrow-rs implementation
from PSEUDOCODE P-02. Key changes:
- Remove `Series::new(...)` and `DataFrame::new(...)` calls
- Build `Int64Array`, `StringArray` arrays from pre-allocated Vecs
- Build `RecordBatch::try_new(raw_data_schema(), vec![...])`
- Write via `ArrowWriter::try_new(file, schema, Some(props))` + `.write(&batch)` + `.close()`
- Preserve `spawn_blocking` wrapper exactly
- Preserve empty-guard `if points.is_empty() { return Ok(()); }`

**Verify:**
```
cargo test -p platform-core storage::parquet::tests::test_write_raw_batch
cargo test -p platform-core storage::parquet::tests::test_write_raw_batch_empty_succeeds
cargo test -p platform-core storage::parquet::tests::test_write_raw_batch_multiple_sources
```

**Refactor:** Verify `source_ids` collection does not clone unnecessarily. The current code
clones `p.source_id` into owned Strings for Polars Series. With arrow-rs, use `&str` references
where possible:
```rust
source_ids.push(p.source_id.as_str());
```
This requires `source_ids: Vec<&str>` instead of `Vec<String>`.

---

### Cycle 3: `write_parquet()` -- 6-column TimeSeriesPoint write

**Goal:** Replace the TimeSeriesPoint write path. This is used by `append_to_parquet` (and
indirectly by `write`, `write_batch`, WAL replay).

**Red:**
```
cargo test -p platform-core storage::parquet::tests::test_write_batch
cargo test -p platform-core storage::parquet::tests::test_write_single_point
```

**Green:**

Replace the body of `write_parquet()` (lines 90-155) with the arrow-rs implementation from
PSEUDOCODE P-01. Same structural pattern as Cycle 2 but with 6 columns.

Key difference from Cycle 2:
- `location_ids` can use `&str` references: `p.location_id.as_str()`
- `metrics` collects `&str` from tag lookup with default
- `ndp_ids` and `contexts` use `Vec<Option<String>>` for nullable arrays

**Verify:**
```
cargo test -p platform-core storage::parquet::tests::test_write_batch
cargo test -p platform-core storage::parquet::tests::test_write_single_point
cargo test -p platform-core storage::parquet::tests::test_metric_column_persistence
cargo test -p platform-core storage::parquet::tests::test_metric_column_default_to_unknown
```

**Refactor:** Extract the Snappy `WriterProperties` builder into a shared constant or function
since it is identical in both write methods:
```rust
fn snappy_writer_properties() -> WriterProperties {
    WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build()
}
```

---

### Cycle 4: Read paths -- `append_to_parquet()` + `append_to_raw_parquet()`

**Goal:** Replace both read-modify-write methods. These are used by `Store::write()` (single
point writes) and the deprecated `write_raw()`.

**Red:**
```
cargo test -p platform-core storage::parquet::tests::test_write_single_point
cargo test -p platform-core storage::parquet::tests::test_write_and_query_raw_round_trip
cargo test -p platform-core storage::parquet::tests::test_raw_handles_nullable_fields
cargo test -p platform-core storage::parquet::tests::test_wal_replay_on_startup
```
These tests exercise the append path (write single point, then query back). They must pass
before and after.

**Green:**

Replace `append_to_parquet()` (lines 157-225) with PSEUDOCODE P-03.
Replace `append_to_raw_parquet()` (lines 581-640) with PSEUDOCODE P-04.

Both methods share the same pattern:
1. If file exists, open with `ParquetRecordBatchReaderBuilder::try_new(file)?.build()?`
2. Iterate `RecordBatch` results
3. Downcast required columns via `.column_by_name("x")?.as_any().downcast_ref::<T>()`
4. Handle nullable columns with `.is_null(i)` checks
5. Reconstruct point structs
6. Append to new points vector
7. Call `write_parquet` / `write_raw_parquet` with merged vector

Use the `read_nullable_string()` and `read_nullable_json()` helpers from Cycle 1.

**Critical correctness note on nullable columns:**

In Polars, `col.get(i)` returns `None` for null AND for out-of-bounds. In arrow-rs,
`.value(i)` on a null index returns the default value (empty string `""` for StringArray).
You MUST check `.is_null(i)` before `.value(i)` for nullable columns. The helpers enforce
this pattern.

**Verify:**
```
cargo test -p platform-core storage::parquet::tests::test_write_single_point
cargo test -p platform-core storage::parquet::tests::test_write_and_query_raw_round_trip
cargo test -p platform-core storage::parquet::tests::test_raw_handles_nullable_fields
cargo test -p platform-core storage::parquet::tests::test_wal_replay_on_startup
cargo test -p platform-core storage::parquet::tests::test_partition_pruning
cargo test -p platform-core storage::parquet::tests::test_multiple_locations
```

**Refactor:** The column downcast + error handling pattern is verbose. Consider a macro or
helper function, but only if it does not obscure the logic. The explicit form is acceptable
for 6 methods.

---

### Cycle 5: `query()` -- read with timestamp filter

**Goal:** Replace the Polars lazy filter in the TimeSeriesPoint query path with direct
row-level filtering on Arrow arrays.

**Red:**
```
cargo test -p platform-core storage::parquet::tests::test_query_time_range
cargo test -p platform-core storage::parquet::tests::test_query_with_filters
cargo test -p platform-core storage::parquet::tests::test_aggregate_mean
cargo test -p platform-core storage::parquet::tests::test_aggregate_percentile
```
`aggregate` calls `query` internally, so aggregation tests also validate this cycle.

**Green:**

Replace `query()` (lines 278-350) with PSEUDOCODE P-05. Key changes:
- Remove `ParquetReader::new(file).finish()` + `df.lazy().filter(...).collect()`
- Replace with `ParquetRecordBatchReaderBuilder` + row-level `if ts < start_micros || ts > end_micros { continue; }`
- Pre-compute `start_micros` and `end_micros` once before the loop

**Performance rationale:** Partition files contain at most ~2880 rows (one per 30 seconds
for 24 hours). Row-level filtering is trivially fast at this scale. The Polars lazy filter
added DataFrame materialization overhead without predicate pushdown benefit.

**Verify:**
```
cargo test -p platform-core storage::parquet::tests::test_query_time_range
cargo test -p platform-core storage::parquet::tests::test_query_with_filters
cargo test -p platform-core storage::parquet::tests::test_aggregate_mean
cargo test -p platform-core storage::parquet::tests::test_aggregate_percentile
cargo test -p platform-core storage::parquet::tests::test_parquet_stores_ndp_id
cargo test -p platform-core storage::parquet::tests::test_parquet_stores_context
cargo test -p platform-core storage::parquet::tests::test_parquet_stores_both_ndp_id_and_context
cargo test -p platform-core storage::parquet::tests::test_parquet_handles_none_ndp_id_and_context
```

**Refactor:** None needed. The row-level filter is clearer than the Polars lazy approach.

---

### Cycle 6: `query_raw()` -- raw read with filters

**Goal:** Replace the last Polars usage in production code.

**Red:**
```
cargo test -p platform-core storage::parquet::tests::test_source_filter_in_query
cargo test -p platform-core storage::parquet::tests::test_write_and_query_raw_round_trip
cargo test -p platform-core storage::parquet::tests::test_raw_preserves_all_json_types
cargo test -p platform-core storage::parquet::tests::test_raw_context_metadata_preserved
```

**Green:**

Replace `query_raw()` (lines 761-820) with PSEUDOCODE P-06. Key changes:
- Replace `ParquetReader::new(file).finish()` with `ParquetRecordBatchReaderBuilder`
- Replace `df.column("x")?.i64()?` / `.utf8()?` with arrow-rs downcast pattern
- Keep existing time and source filter logic (already manual in current code)
- Preserve `Vec::with_capacity(partition_files.len() * 100)` pre-allocation (M-005)

**Verify:**
```
cargo test -p platform-core storage::parquet::tests::test_source_filter_in_query
cargo test -p platform-core storage::parquet::tests::test_write_and_query_raw_round_trip
cargo test -p platform-core storage::parquet::tests::test_raw_preserves_all_json_types
cargo test -p platform-core storage::parquet::tests::test_raw_context_metadata_preserved
cargo test -p platform-core storage::parquet::tests::test_write_raw_single_point
cargo test -p platform-core storage::parquet::tests::test_raw_handles_nullable_fields
```

**Refactor:** At this point, ALL production code is converted. No Polars API calls remain in
production methods. The only remaining Polars usage is in test #25 and the import line.

---

### Cycle 7: New schema compatibility and edge case tests (T-NEW-01 through T-NEW-05)

**Goal:** Add the new tests defined in TEST-STRATEGY.md sections T-NEW-01 through T-NEW-05.
These tests verify schema correctness, nullable handling, and large batch behavior.

**Red:** Write all new tests first. They should pass immediately since the underlying
implementation was converted in Cycles 2-6.

**Tests to add:**

**T-NEW-01: `test_timeseries_parquet_schema_metadata`**
```
Write 3 TimeSeriesPoints.
Open the Parquet file with ParquetRecordBatchReaderBuilder.
Verify:
  - schema.fields().len() == 6
  - Column names: ["timestamp", "location_id", "metric", "value", "ndp_id", "context"]
  - Column types: Int64, Utf8, Utf8, Float64, Utf8 (nullable), Utf8 (nullable)
  - Verify Snappy compression from file metadata
  - Row count: 3
```

**T-NEW-02: `test_raw_parquet_schema_metadata`**
```
Write 3 RawDataPoints.
Open the Parquet file with ParquetRecordBatchReaderBuilder.
Verify:
  - schema.fields().len() == 5
  - Column names: ["timestamp", "source_id", "ndp_id", "context", "raw_payload"]
  - Column types: Int64, Utf8, Utf8 (nullable), Utf8 (nullable), Utf8
  - Verify Snappy compression from file metadata
  - Row count: 3
```

To verify Snappy compression from file metadata, use:
```rust
use parquet::file::reader::SerializedFileReader;
use parquet::file::reader::FileReader;

let file = std::fs::File::open(&path).unwrap();
let parquet_reader = SerializedFileReader::new(file).unwrap();
let metadata = parquet_reader.metadata();
let row_group = metadata.row_group(0);
for i in 0..row_group.num_columns() {
    let col_meta = row_group.column(i);
    assert_eq!(col_meta.compression(), parquet::basic::Compression::SNAPPY);
}
```

**T-NEW-03: `test_nullable_column_mixed_some_none`**
```
Write 4 TimeSeriesPoints with mixed nullable patterns:
  - Point 1: ndp_id=Some("id-1"), context=Some({...})
  - Point 2: ndp_id=None, context=None
  - Point 3: ndp_id=Some("id-3"), context=None
  - Point 4: ndp_id=None, context=Some({...})

Read back with ParquetRecordBatchReaderBuilder.
For each RecordBatch, verify null bitmap:
  - ndp_id column: is_null(1)==true, is_null(3)==true, value(0)=="id-1", value(2)=="id-3"
  - context column: is_null(1)==true, is_null(2)==true, value(0) and value(3) are non-null
  - ndp_id null_count == 2
  - context null_count == 2
```

Also run the same pattern for RawDataPoint (`test_raw_nullable_column_mixed_some_none`).

**T-NEW-04: `test_empty_batch_creates_no_file`**
```
Call write_parquet(vec![], path) -> Ok(())
Verify: path does NOT exist

Call write_raw_parquet(vec![], path) -> Ok(())
Verify: path does NOT exist
```

**T-NEW-05: `test_large_batch_stress_10000_points`**
```
Generate 10,000 TimeSeriesPoints with sequential timestamps and value = index as f64.
Write via write_batch().
Read back via query().
Verify:
  - Count == 10,000
  - First point value == 0.0
  - Last point value == 9999.0
  - No panics or OOM
```

Also run for RawDataPoint (`test_large_raw_batch_stress_10000_points`).

**Green:** All tests should pass immediately from Cycles 2-6 implementation.

**Verify:**
```
cargo test -p platform-core storage::parquet::tests
```
Run the full test module to confirm everything passes.

---

### Cycle 8: Remove Polars dependency -- clean cut

**Goal:** Remove all Polars traces from `core`.

**Steps:**

1. Remove `use polars::prelude::*;` from `core/src/storage/parquet.rs` (line 9)

2. Rewrite `test_raw_parquet_schema_has_5_columns` (test #25, line 1583):
   - Remove `ParquetReader::new(file).finish().unwrap()` and `df.get_column_names()`
   - Replace with `ParquetRecordBatchReaderBuilder::try_new(file).unwrap()` and
     `builder.schema().fields().iter().map(|f| f.name().as_str())`
   - (This may be redundant with T-NEW-02 from Cycle 7, but keep both for regression coverage)

3. Update `core/src/error.rs`:
   - Remove `CoreError::Polars(String)` variant (renamed to `Arrow` in Cycle 1)
   - Remove `impl From<polars::error::PolarsError> for CoreError`
   - If the RECOMMENDED approach from Cycle 1 was followed (kept both variants), now is the
     time to remove `Polars` and keep only `Arrow`

4. Remove from `core/Cargo.toml`:
   ```toml
   # REMOVE this line:
   polars = { workspace = true }
   ```

5. Keep `polars` in workspace `Cargo.toml` `[workspace.dependencies]` (still used by
   `silver-etl` and `air-quality-app` in their dev-dependencies).

**Verify:**
```
cargo check -p platform-core
cargo test -p platform-core
```
Both must pass with zero Polars references in `core/`.

**Grep verification:**
```
grep -r "polars" core/src/
# Expected: zero results
```

---

### Cycle 9: Full suite verification + integration + cleanup

**Goal:** Confirm zero regressions across the entire workspace. Update comments. Measure
binary size delta.

**Steps:**

1. Run full workspace test suite:
   ```
   cargo test --workspace
   ```
   All 874+ tests must pass.

2. Run clippy:
   ```
   cargo clippy -p platform-core -- -D warnings
   ```

3. Run formatter:
   ```
   cargo fmt --check -p platform-core
   ```

4. Update comments in `bronze.rs` that reference "Polars":
   - Line 249 area: `malloc_trim(0)` comment can be updated to note "Post-AIR-018: Polars
     removed; malloc_trim retained for slow-creep monitoring"
   - Any diagnostic comments that say "Polars DataFrame allocation" can be updated

5. Measure binary size:
   ```
   # Before (record from current main):
   ls -la target/release/air-quality-app
   # After:
   cargo build --release -p air-quality-app
   ls -la target/release/air-quality-app
   ```
   Document the delta in the completion report.

6. Run integration tests from `bronze.rs`:
   ```
   cargo test -p platform-core subscribers::bronze::integration_tests
   ```

7. Verify no `TODO`, `unimplemented!()`, `todo!()`, or placeholder functions exist in
   changed files.

---

## 2. Edge Cases and Error Handling

The implementor MUST handle every edge case listed below. Each is annotated with the
cycle where it is addressed.

### 2.1 Empty points vector (Cycles 2, 3)

Both `write_parquet` and `write_raw_parquet` have an early return:
```rust
if points.is_empty() {
    return Ok(());
}
```
This guard is preserved exactly. No file is created for empty input. Verified by
`test_write_raw_batch_empty_succeeds` and T-NEW-04.

### 2.2 All-null ndp_id column (Cycles 2, 3)

When every row has `ndp_id = None`, the `StringArray` will have a null bitmap with all bits
set to null. Arrow handles this natively:
```rust
let ndp_ids: Vec<Option<&str>> = vec![None, None, None];
let array = StringArray::from(ndp_ids); // All nulls, null_count == 3
```
Verified by `test_raw_handles_nullable_fields` and `test_parquet_handles_none_ndp_id_and_context`.

### 2.3 Mixed null/non-null in same column (Cycles 2, 3, 7)

When some rows have `Some("value")` and others have `None`, Arrow's null bitmap correctly
tracks which indices are null:
```rust
let ndp_ids: Vec<Option<&str>> = vec![Some("id-1"), None, Some("id-3"), None];
let array = StringArray::from(ndp_ids); // null_count == 2, is_null(1)==true, is_null(3)==true
```
Verified by T-NEW-03 (`test_nullable_column_mixed_some_none`).

### 2.4 Very large batches (10,000+ points) (Cycle 7)

`Vec::with_capacity(len)` pre-allocation (P2-02 optimization) is preserved from current code.
Arrow arrays are built from these Vecs in a single pass. `RecordBatch::try_new` does NOT copy
the data -- it wraps the existing buffers. The `ArrowWriter` writes a single row group.
Verified by T-NEW-05 (`test_large_batch_stress_10000_points`).

### 2.5 Corrupt Parquet file on read (Cycles 4, 5, 6)

If a Parquet file is corrupt (truncated, invalid footer), `ParquetRecordBatchReaderBuilder::try_new(file)`
returns `Err(ParquetError)`. This is propagated via:
```rust
.map_err(|e| CoreError::Storage(format!("Failed to read existing Parquet: {}", e)))?
```
The caller (`write_raw`, `query`, etc.) receives `Err(CoreError::Storage(...))` and can
decide recovery strategy. This matches the current Polars behavior where `ParquetReader::new(file).finish()`
returns `Err(PolarsError)`.

### 2.6 Missing columns on read -- backward compatibility (Cycles 4, 5, 6)

Nullable columns (`ndp_id`, `context`) use `batch.column_by_name("ndp_id")` which returns
`Option<&ArrayRef>`. If the column does not exist (e.g., reading a Parquet file written before
AIR-009 added these columns), the result is `None`, and the helper returns `None` for every row.
This matches the current Polars behavior: `df.column("ndp_id").ok()`.

Required columns (`timestamp`, `location_id`, `source_id`, etc.) use `.ok_or_else(|| CoreError::Storage(...))`.
If a required column is missing, the error propagates immediately. This is stricter than the
current Polars code (which would also fail, but with a `PolarsError`).

### 2.7 `ArrowWriter::close()` failure -- file corruption risk (Cycles 2, 3)

`ArrowWriter::close()` flushes buffered row groups and writes the Parquet footer. If `close()`
fails (e.g., disk full, permission error), the Parquet file is corrupt (no footer). The error
MUST be propagated:
```rust
writer.close()
    .map_err(|e| CoreError::Storage(format!("Failed to close Parquet writer: {}", e)))?;
```
Do NOT use `drop(writer)` -- the `Drop` impl for `ArrowWriter` does NOT call `close()`. An
unclosed writer produces a corrupt file with no error indication.

**This is the most critical correctness requirement in the entire implementation.**

### 2.8 `spawn_blocking` panic handling (Cycles 2, 3)

The existing pattern wraps the blocking closure result in a `JoinHandle`:
```rust
tokio::task::spawn_blocking(move || { ... })
    .await
    .map_err(|e| CoreError::Storage(format!("Parquet write task panicked: {}", e)))??;
```
The outer `?` handles `JoinError` (panic in the blocking task). The inner `?` handles
`CoreError` from the closure. This pattern is preserved exactly.

### 2.9 Timestamp conversion edge case (Cycles 4, 5, 6)

`DateTime::from_timestamp_micros(ts)` returns `None` for out-of-range microsecond values.
The current code handles this with `.ok_or_else(|| CoreError::Storage("Invalid timestamp"))`.
This is preserved in all read paths.

### 2.10 JSON deserialization failure in context column (Cycles 4, 5, 6)

The `context` column stores JSON-serialized `serde_json::Value`. If deserialization fails
(e.g., corrupt data), the current code silently returns `None`:
```rust
serde_json::from_str(col.value(i)).ok()
```
This behavior is preserved via `read_nullable_json()` which uses `.ok()` to swallow errors.

### 2.11 JSON deserialization failure in raw_payload column (Cycles 4, 6)

Unlike `context`, `raw_payload` deserialization failure is an error (the payload is required):
```rust
let raw_payload: serde_json::Value = serde_json::from_str(payload_str)
    .map_err(|e| CoreError::Storage(format!("Invalid JSON payload: {}", e)))?;
```
This is preserved from the current code.

---

## 3. Performance Considerations

### 3.1 Arrow RecordBatch vs Polars DataFrame allocation

| Aspect | Polars DataFrame | Arrow RecordBatch |
|--------|-----------------|-------------------|
| Column creation | `Series::new` allocates Polars ChunkedArray (wraps internal arrow buffers) | `Int64Array::from(vec)` / `StringArray::from(vec)` wraps Vec directly |
| Container creation | `DataFrame::new(vec![...])` validates and stores Series | `RecordBatch::try_new(schema, vec![...])` validates schema match |
| Overhead | ChunkedArray adds a Vec of Arrow arrays (even for single chunk) + metadata | Direct array reference, no extra wrapping |
| Drop behavior | DataFrame drops all Series, each drops ChunkedArray, each drops Arrow buffers | RecordBatch drops Arc references to arrays |

The key difference is that Polars adds TWO layers of indirection (Series -> ChunkedArray -> Arrow buffer)
compared to Arrow's ONE layer (ArrayRef -> buffer). This produces more heap fragmentation under
glibc malloc because the intermediate allocations are different sizes and lifetimes.

### 3.2 Pre-allocation patterns preserved

All `Vec::with_capacity(len)` calls from the P2-02 optimization are preserved:
```rust
let len = points.len();
let mut timestamps = Vec::with_capacity(len);
let mut location_ids = Vec::with_capacity(len);
// ... etc
```
Arrow array construction from `Vec<T>` does NOT copy the data for primitive types (`Int64Array`,
`Float64Array`). For `StringArray`, the data is copied into Arrow's offset+buffer format, but
this is a single allocation rather than per-element allocation.

### 3.3 Single row-group per file

The current code writes one DataFrame per file, producing one row group. The new code writes
one `RecordBatch` per `ArrowWriter`, also producing one row group. This is preserved.

### 3.4 Snappy compression

`WriterProperties::builder().set_compression(Compression::SNAPPY)` matches the current
`ParquetCompression::Snappy`. The `snap` crate (pure Rust Snappy) is used by both Polars and
the direct `parquet` crate, so compression ratio is identical.

### 3.5 Read path performance

The read path replaces `ParquetReader::new(file).finish()` (which loads the entire file into a
DataFrame) with `ParquetRecordBatchReaderBuilder::try_new(file)?.build()?` (which returns an
iterator over row groups). For single-row-group files (all our files), this is equivalent.

The Polars lazy filter in `query()`:
```rust
df.lazy().filter(col("timestamp").gt_eq(lit(start))...).collect()
```
is replaced by row-level comparison:
```rust
if ts < start_micros || ts > end_micros { continue; }
```
At our data volumes (< 3000 rows per partition file), row-level filtering is negligible.
The Polars lazy filter added DataFrame materialization overhead (creating a new filtered
DataFrame) that the row-level approach avoids entirely.

---

## 4. Error Type Migration

### 4.1 Exact changes to `core/src/error.rs`

**Current state (43 lines):**
```rust
#[derive(Error, Debug)]
pub enum CoreError {
    // ... other variants ...
    #[error("Polars error: {0}")]
    Polars(String),
    // ... other variants ...
}

impl From<polars::error::PolarsError> for CoreError {
    fn from(err: polars::error::PolarsError) -> Self {
        CoreError::Polars(err.to_string())
    }
}
```

**Final state after Cycle 8:**
```rust
#[derive(Error, Debug)]
pub enum CoreError {
    // ... other variants ...
    #[error("Arrow error: {0}")]
    Arrow(String),
    // ... other variants ...
}

impl From<arrow::error::ArrowError> for CoreError {
    fn from(err: arrow::error::ArrowError) -> Self {
        CoreError::Arrow(err.to_string())
    }
}

impl From<parquet::errors::ParquetError> for CoreError {
    fn from(err: parquet::errors::ParquetError) -> Self {
        CoreError::Arrow(err.to_string())
    }
}
```

### 4.2 Impact analysis

`CoreError::Polars` is referenced in exactly ONE location outside of documentation:
- `core/src/error.rs` line 38: the `From<PolarsError>` impl

No code in `core/`, `apps/`, `tools/`, or `crates/` matches on `CoreError::Polars` by name.
The variant is only constructed via the `From` impl. Renaming it to `CoreError::Arrow` has
zero external impact.

### 4.3 Why both `From<ArrowError>` and `From<ParquetError>`

The `parquet` crate has its own error type (`parquet::errors::ParquetError`) separate from
`arrow::error::ArrowError`. Both can occur:
- `ArrowError` from `RecordBatch::try_new()` (schema validation)
- `ParquetError` from `ArrowWriter::try_new()`, `.write()`, `.close()`, and
  `ParquetRecordBatchReaderBuilder::try_new()`

Both map to `CoreError::Arrow(String)` for simplicity. The error message string contains
enough context to distinguish the source.

### 4.4 Transition strategy

| Cycle | CoreError state |
|-------|----------------|
| 1 | Add `Arrow` variant + `From<ArrowError>` + `From<ParquetError>`. Keep `Polars` + `From<PolarsError>` |
| 2-7 | Both variants coexist. New code uses `Arrow`/`Storage`. Old code still compiles with `Polars` |
| 8 | Remove `Polars` variant + `From<PolarsError>`. Remove `polars` dependency |

---

## 5. Code Review Checklist

The implementor and reviewer MUST verify every item before merging.

### Dependencies
- [ ] `polars` NOT in `core/Cargo.toml` `[dependencies]`
- [ ] `arrow` added to `core/Cargo.toml` via workspace reference
- [ ] `parquet` added to `core/Cargo.toml` via workspace reference
- [ ] `arrow` and `parquet` entries added to workspace `Cargo.toml` `[workspace.dependencies]`
- [ ] Arrow and parquet versions use `"54"` with `default-features = false`
- [ ] `parquet` features include `"arrow"` and `"snap"`

### Imports
- [ ] No `use polars::` anywhere in `core/src/`
- [ ] Arrow and parquet imports are specific (not glob `use arrow::*`)

### Error handling
- [ ] `CoreError::Polars` variant removed (replaced by `CoreError::Arrow`)
- [ ] `From<polars::error::PolarsError>` impl removed
- [ ] `From<arrow::error::ArrowError>` impl added
- [ ] `From<parquet::errors::ParquetError>` impl added

### Write path
- [ ] `write_parquet()` uses `ArrowWriter` + `RecordBatch` (no DataFrame)
- [ ] `write_raw_parquet()` uses `ArrowWriter` + `RecordBatch` (no DataFrame)
- [ ] `ArrowWriter::close()` is called explicitly (NOT just `drop`)
- [ ] `WriterProperties` specifies `Compression::SNAPPY`
- [ ] Schema field order matches: timestamp, location_id, metric, value, ndp_id, context (6-col)
- [ ] Schema field order matches: timestamp, source_id, ndp_id, context, raw_payload (5-col)
- [ ] Nullable columns (`ndp_id`, `context`) use `Field::new("name", DataType::Utf8, true)`
- [ ] Non-nullable columns use `Field::new("name", DataType::T, false)`
- [ ] `spawn_blocking` pattern preserved for write methods
- [ ] Pre-allocated `Vec::with_capacity(len)` preserved (P2-02 optimization)
- [ ] Empty points guard (`if points.is_empty() { return Ok(()); }`) preserved

### Read path
- [ ] `append_to_parquet()` uses `ParquetRecordBatchReaderBuilder`
- [ ] `append_to_raw_parquet()` uses `ParquetRecordBatchReaderBuilder`
- [ ] `query()` uses `ParquetRecordBatchReaderBuilder` + row-level timestamp filter
- [ ] `query_raw()` uses `ParquetRecordBatchReaderBuilder` + manual time/source filter
- [ ] Nullable columns checked with `.is_null(i)` before `.value(i)`
- [ ] Missing nullable columns handled via `batch.column_by_name("x")` returning `None`
- [ ] `context` column deserialization uses `serde_json::from_str(col.value(i)).ok()` (silent failure)
- [ ] `raw_payload` deserialization uses `.map_err()` (error propagation)

### Tests
- [ ] All 874+ existing tests pass (`cargo test --workspace`)
- [ ] `test_raw_parquet_schema_has_5_columns` rewritten with arrow-rs reader
- [ ] T-NEW-01 added: `test_timeseries_parquet_schema_metadata` (6-column schema verification)
- [ ] T-NEW-02 added: `test_raw_parquet_schema_metadata` (5-column schema verification)
- [ ] T-NEW-03 added: `test_nullable_column_mixed_some_none` (null bitmap verification)
- [ ] T-NEW-04 added: `test_empty_batch_creates_no_file`
- [ ] T-NEW-05 added: `test_large_batch_stress_10000_points`
- [ ] New tests verify Snappy compression from file metadata
- [ ] No test imports `polars::prelude::*`

### Code quality
- [ ] `cargo fmt --check -p platform-core` passes
- [ ] `cargo clippy -p platform-core -- -D warnings` passes
- [ ] No `TODO`, `unimplemented!()`, `todo!()`, or placeholder functions
- [ ] No hardcoded secrets or paths
- [ ] `bronze.rs` comments updated to reflect Polars removal (diagnostic logging stays)
- [ ] `#[deprecated]` attribute preserved on `append_to_raw_parquet()`

---

## 6. Rollback Plan

If the change causes issues after deployment to Pi:

### Step 1: Revert the commit
```bash
git revert <commit-hash>
```
The entire replacement is expected to be a single commit. Reverting restores the Polars-based
implementation completely.

### Step 2: Rebuild with no cache (Pattern ID 21)
```bash
cd /home/pi/neural-data-platform
docker build --no-cache -t ndp-air-quality -f deploy/pi/Dockerfile .
```
The `--no-cache` flag is REQUIRED because cargo's incremental compilation cache can retain
stale object files from the arrow-rs build. Without `--no-cache`, the reverted binary may
still link against arrow-rs artifacts.

### Step 3: Redeploy
```bash
docker-compose -f docker-compose.yml down
docker-compose -f docker-compose.yml up -d
```

### Step 4: Verify via pre-deploy baseline sampling (Pattern ID 22)
```bash
# Use MCP ndp tools to sample current data
# Compare against pre-deploy baseline
# Verify RSS behavior returns to pre-change levels
```

### Step 5: Monitor
Watch BUG-004 diagnostic logging in `bronze.rs` for 2+ hours to confirm the reverted binary
behaves as expected (known memory leak resumes, but container is stable short-term).

### Rollback decision criteria
- Container OOMs within the first hour after deployment (should not happen -- the fix reduces
  memory usage)
- Silver ETL reports read errors on Bronze Parquet files (schema incompatibility)
- `query_raw` returns incorrect data (wrong column mapping)
- Any panic in the Parquet write or read path

---

## 7. Version Discrepancy Resolution

### Observed issue

The SPECIFICATION.md and PSEUDOCODE.md reference arrow/parquet version `"54"`, while the
ADR-001 mentions version `"54"`. However, the `Cargo.lock` shows:
- `arrow 56.2.0` and `arrow 57.1.0` already present
- `parquet 57.1.0` already present

The SPECIFICATION.md FR-07 section references version `"57"` with a rationale about matching
Polars 0.35's transitive dependency.

### Root cause

Polars 0.35.4 uses `polars-arrow` (its own fork), NOT the standard Apache `arrow` crate.
The `arrow 57.1.0` in `Cargo.lock` is pulled by another workspace member. The `arrow 56.2.0`
may be pulled by a different dependency.

### Resolution

The implementor should use version `"54"` as specified in the PSEUDOCODE.md, with the
understanding that Cargo's semver resolution will pick the latest compatible version from
the lockfile. Since `"54"` is a minimum version specifier (semver-compatible with 54.x, 55.x,
56.x, 57.x under `version = "54"`), Cargo will resolve to whatever version is already in the
lockfile or the latest available.

**Alternatively**, if the implementor wants to pin to the exact version already in the lockfile,
use `"57"` to match `parquet 57.1.0`. Both approaches are valid. The key constraint is that
`arrow` and `parquet` must use the SAME major version (they are released together).

### Recommendation

Use `"54"` as specified in the pseudocode. This provides maximum forward compatibility and lets
Cargo handle version resolution. Run `cargo update -p arrow -p parquet` after adding the
dependencies to ensure a consistent lockfile.

---

## 8. Dependency Graph Impact

### Before (core depends on polars)
```
platform-core
  -> polars 0.35.4
       -> polars-core -> polars-arrow (fork) -> arrow-format, ethnum, ...
       -> polars-io -> polars-parquet (fork) -> snap, streaming-decompression, ...
       -> polars-lazy -> polars-plan -> polars-pipe -> polars-ops
       -> polars-time
       -> polars-utils
       ~15 polars-* crates, each with their own deps
```

### After (core depends on arrow + parquet directly)
```
platform-core
  -> arrow (selected version)
       -> arrow-array, arrow-buffer, arrow-data, arrow-schema
       ~5-6 sub-crates
  -> parquet (selected version)
       -> arrow (shared), snap, thrift
       ~3 additional deps
```

The net reduction is approximately 10-15 crates that are unique to Polars and no longer needed
by `core`. Other workspace members (`silver-etl`, `air-quality-app`) still pull in Polars via
their `[dev-dependencies]`, so the workspace-level lockfile retains Polars crates. The
production binary for `air-quality-app` does NOT link against Polars since `core` is its only
dependency that used it.

---

## 9. Patterns Applied

| Pattern ID | Name | How Applied |
|------------|------|-------------|
| 27 | `architecture:polars-to-arrow-replacement` | Primary pattern. Guided the overall replacement strategy. |
| 29 | `testing:storage-engine-replacement-strategy` | Test inventory, category classification, and execution order. |
| 30 | `architecture:polars-to-arrow-migration` | BUG-004 root cause analysis, memory leak fix rationale. |
| 23 | `testing:bronze-integration-with-parquet-wal` | Integration test patterns preserved for Cycle 9. |
| 31 | `architecture:deprecated-approaches` | Confirmed Polars is deprecated for core. |
| 21 | (docker-cache-verification) | Rollback plan: `--no-cache` required on Pi rebuild. |
| 22 | (pre-deploy-baseline-sampling) | Rollback plan: baseline sampling for verification. |
