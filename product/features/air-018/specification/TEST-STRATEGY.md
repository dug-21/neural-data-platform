# AIR-018 Test Strategy: Replace Polars with arrow-rs + parquet

> **Feature:** air-018
> **Author:** ndp-tester
> **Date:** 2026-02-10
> **Status:** Approved
> **Applies to:** `core/src/storage/parquet.rs`

---

## 1. Scope

This document defines the test strategy for replacing Polars with direct `arrow-rs` + `parquet` crate usage in `core/src/storage/parquet.rs`. The goal is to ensure schema-identical Parquet output, zero regressions in existing behavior, and coverage of new arrow-rs-specific edge cases.

---

## 2. Existing Test Inventory

### 2.1 Unit Tests in `parquet.rs` (`mod tests`, line 823)

| # | Test Name | What It Tests | Uses Polars Directly? | AIR-018 Impact |
|---|-----------|---------------|----------------------|----------------|
| 1 | `test_partition_path_generation` | Partition path format for TimeSeriesPoint | No | None |
| 2 | `test_write_single_point` | Single TimeSeriesPoint write via `store.write()` | No | None |
| 3 | `test_write_batch` | Batch TimeSeriesPoint write via `store.write_batch()` | No | None |
| 4 | `test_query_time_range` | Query with time range filtering | No | None |
| 5 | `test_query_with_filters` | Query with HashMap filters parameter | No | None |
| 6 | `test_aggregate_mean` | Mean aggregation over time buckets | No | None |
| 7 | `test_aggregate_percentile` | 95th percentile aggregation | No | None |
| 8 | `test_health_check` | HealthStatus from `store.health_check()` | No | None |
| 9 | `test_wal_write` | WAL entry created on write | No | None |
| 10 | `test_wal_replay_on_startup` | WAL replay restores points to Parquet | No | None |
| 11 | `test_partition_pruning` | Multi-day writes create separate partitions | No | None |
| 12 | `test_multiple_locations` | Writes to different location_ids are isolated | No | None |
| 13 | `test_metric_column_persistence` | Multiple metric names survive write/read | No | None |
| 14 | `test_metric_column_default_to_unknown` | Missing metric tag defaults to "unknown" | No | None |
| 15 | `test_partition_key_uses_stream_id_over_location_id` | stream_id tag overrides location_id for partition | No | None |
| 16 | `test_partition_key_falls_back_to_location_id` | No stream_id tag uses location_id | No | None |
| 17 | `test_mqtt_points_written_to_stream_directory` | MQTT-enriched point goes to stream dir | No | None |
| 18 | `test_get_partition_key_function` | Unit test for `get_partition_key` helper | No | None |
| 19 | `test_parquet_stores_ndp_id` | ndp_id round-trips through write/query | No | None |
| 20 | `test_parquet_stores_context` | context JSON round-trips through write/query | No | None |
| 21 | `test_parquet_stores_both_ndp_id_and_context` | Both nullable fields round-trip | No | None |
| 22 | `test_parquet_handles_none_ndp_id_and_context` | None values for ndp_id/context round-trip | No | None |
| 23 | `test_raw_partition_path_uses_stream_id` | Raw partition path extracts stream from source_id | No | None |
| 24 | `test_extract_stream_id_from_source_id` | `extract_stream_id` helper function | No | None |
| 25 | `test_raw_parquet_schema_has_5_columns` | Verifies 5-column raw schema using Polars `ParquetReader` + `df.get_column_names()` | **Yes** | **Rewrite** |
| 26 | `test_write_raw_single_point` | Single RawDataPoint write via `store.write_raw()` | No | None |
| 27 | `test_write_and_query_raw_round_trip` | RawDataPoint write/query round-trip | No | None |
| 28 | `test_raw_handles_nullable_fields` | None ndp_id/context in RawDataPoint | No | None |
| 29 | `test_write_raw_batch` | Batch RawDataPoint write | No | None |
| 30 | `test_write_raw_batch_empty_succeeds` | Empty batch returns Ok(()) | No | None |
| 31 | `test_write_raw_batch_multiple_sources` | Batch with multiple source_ids partitions correctly | No | None |
| 32 | `test_partition_path_structure` | Raw partition directory structure verified on disk | No | None |
| 33 | `test_source_filter_in_query` | query_raw with and without source filter | No | None |
| 34 | `test_raw_preserves_all_json_types` | Complex JSON types survive raw_payload round-trip | No | None |
| 35 | `test_raw_context_metadata_preserved` | Context JSON with nested objects round-trips | No | None |

### 2.2 Integration Tests in `bronze.rs` (`mod integration_tests`, line 1486)

| # | Test Name | What It Tests | Uses Polars Directly? | AIR-018 Impact |
|---|-----------|---------------|----------------------|----------------|
| I-1 | `test_integration_full_ingest_snapshot_cycle` | Full BronzeSubscriber ingest, snapshot, verify via `query_raw` | No | None |
| I-2 | `test_integration_crash_recovery` | Crash simulation, WAL replay, accumulator rebuild | No | None |
| I-3 | `test_integration_snapshot_overwrites_previous` | Second snapshot overwrites (not appends) | No | None |
| I-4 | `test_integration_multiple_streams_isolation` | Multiple source_ids get separate Parquet files | No | None |

---

## 3. Test Categories

### Category 1: Tests that DO NOT use Polars directly -- NO CHANGE NEEDED (38 tests)

These tests call the `ParquetStore` trait methods (`write`, `write_batch`, `query`, `write_raw`, `write_raw_batch`, `query_raw`, `write_raw_snapshot`) and inspect results through the trait interface. They never import or call Polars APIs in the test body itself.

**Unit tests (34 tests):** Tests #1-24, #26-35 from the inventory above.

**Integration tests (4 tests):** I-1 through I-4 from `bronze.rs`.

These 38 tests should pass without any modifications after the arrow-rs replacement. If any fail, that failure indicates a behavioral regression in the new implementation.

### Category 2: Tests that use Polars for assertion/verification -- UPDATE REQUIRED (1 test)

**Test #25: `test_raw_parquet_schema_has_5_columns`** (line 1583)

This test writes a RawDataPoint, then opens the Parquet file directly with `polars::prelude::ParquetReader`, calls `df.get_column_names()`, and asserts column count and names.

**Current code (Polars):**
```rust
let file = std::fs::File::open(&path).unwrap();
let df = ParquetReader::new(file).finish().unwrap();
let column_names: Vec<&str> = df.get_column_names();
assert_eq!(column_names.len(), 5, "Should have exactly 5 columns");
assert!(column_names.contains(&"timestamp"));
assert!(column_names.contains(&"source_id"));
assert!(column_names.contains(&"ndp_id"));
assert!(column_names.contains(&"context"));
assert!(column_names.contains(&"raw_payload"));
```

**Required change (arrow-rs):**
```rust
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

let file = std::fs::File::open(&path).unwrap();
let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
let schema = builder.schema();
let column_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
assert_eq!(column_names.len(), 5, "Should have exactly 5 columns");
assert!(column_names.contains(&"timestamp"));
assert!(column_names.contains(&"source_id"));
assert!(column_names.contains(&"ndp_id"));
assert!(column_names.contains(&"context"));
assert!(column_names.contains(&"raw_payload"));
```

### Category 3: New tests to ADD for air-018

#### T-NEW-01: Schema compatibility -- TimeSeriesPoint (6-column)

**Purpose:** Verify the arrow-rs write path produces the exact Parquet schema expected by downstream consumers (Silver ETL, MCP server).

```
Write 3 TimeSeriesPoints using the new arrow-rs write_parquet().
Open the Parquet file with ParquetRecordBatchReaderBuilder.
Verify:
  - Column names: ["timestamp", "location_id", "metric", "value", "ndp_id", "context"]
  - Column types: Int64, Utf8, Utf8, Float64, Utf8 (nullable), Utf8 (nullable)
  - Compression: Snappy (check file metadata)
  - Row count: 3
```

**Priority:** CRITICAL. This is the primary regression gate.

#### T-NEW-02: Schema compatibility -- RawDataPoint (5-column)

**Purpose:** Verify the arrow-rs write_raw_parquet() produces the exact schema.

```
Write 3 RawDataPoints using the new arrow-rs write_raw_parquet().
Open the Parquet file with ParquetRecordBatchReaderBuilder.
Verify:
  - Column names: ["timestamp", "source_id", "ndp_id", "context", "raw_payload"]
  - Column types: Int64, Utf8, Utf8 (nullable), Utf8 (nullable), Utf8
  - Compression: Snappy
  - Row count: 3
```

**Priority:** CRITICAL.

#### T-NEW-03: Nullable column handling -- mixed Some/None

**Purpose:** Verify null bitmap is correctly constructed when ndp_id and context are Some for some rows and None for others.

```
Write 4 TimeSeriesPoints:
  - Point 1: ndp_id=Some("id-1"), context=Some({...})
  - Point 2: ndp_id=None, context=None
  - Point 3: ndp_id=Some("id-3"), context=None
  - Point 4: ndp_id=None, context=Some({...})

Read back with ParquetRecordBatchReaderBuilder.
For each RecordBatch, verify:
  - ndp_id column: is_null(1)==true, is_null(3)==true, value(0)=="id-1", value(2)=="id-3"
  - context column: is_null(1)==true, is_null(2)==true, value(0) and value(3) are non-null
  - Total null_count for ndp_id == 2
  - Total null_count for context == 2
```

Also run the same test for the 5-column RawDataPoint schema.

**Priority:** HIGH. Incorrect null handling silently corrupts data.

#### T-NEW-04: Empty batch handling

**Purpose:** Verify that empty input does not create files or return errors.

```
Call write_parquet(vec![], path) -> Ok(())
Verify: path does NOT exist

Call write_raw_parquet(vec![], path) -> Ok(())
Verify: path does NOT exist
```

**Priority:** MEDIUM. The existing code already guards this, but confirm arrow-rs path preserves it.

#### T-NEW-05: Large batch stress test (10,000 points)

**Purpose:** Validate no off-by-one errors in array construction and verify performance is acceptable.

```
Generate 10,000 TimeSeriesPoints with sequential timestamps and known values.
Write via write_parquet().
Read back via query().
Verify:
  - Count == 10,000
  - First point value matches
  - Last point value matches
  - No panics or OOM
```

Also run for RawDataPoint with 10,000 points.

**Priority:** MEDIUM. Catches allocation/capacity bugs that small tests miss.

#### T-NEW-06: Cross-read compatibility (manual verification step)

**Purpose:** Prove Parquet files written by arrow-rs can be read by any standard Parquet reader, and vice versa.

This test cannot be fully automated within a single test run because the old code (Polars) will be removed. Instead, document a manual verification procedure:

```
Manual verification steps:
1. BEFORE air-018 merge: On main branch, run test suite to produce Parquet files
   in a known temp directory. Copy those files aside.
2. AFTER air-018 implementation: Write a test that reads those saved Parquet files
   using the new arrow-rs read path. Verify all data round-trips correctly.
3. AFTER air-018 implementation: Write new Parquet files with arrow-rs, then
   read them with a standalone Polars script (or Python pyarrow) to verify
   the schema is standard-compliant.

Alternatively, keep polars as a dev-dependency temporarily and write:
  - test_cross_read_arrow_writes_polars_reads: Write with arrow-rs, read with Polars
  - test_cross_read_polars_writes_arrow_reads: Write with Polars, read with arrow-rs
  Remove these tests once compatibility is confirmed.
```

**Priority:** CRITICAL for confidence, but can be done as a manual step or temporary dev-dependency test.

---

## 4. Integration Tests Impact

### 4.1 `bronze.rs` Integration Tests (4 tests)

All four integration tests in `bronze.rs` (`mod integration_tests`, starting at line 1486) interact with `ParquetStore` exclusively through the `RawStore` trait interface:

- `test_integration_full_ingest_snapshot_cycle` -- uses `store.query_raw()`
- `test_integration_crash_recovery` -- uses `handle_point()`, `snapshot()`, `recover()`, `query_raw()`
- `test_integration_snapshot_overwrites_previous` -- uses `handle_point()`, `snapshot()`, `query_raw()`
- `test_integration_multiple_streams_isolation` -- uses `handle_point()`, `snapshot()`, `query_raw()`

**Impact: NONE.** These tests do not import or use Polars APIs. They should pass unchanged.

### 4.2 `bronze.rs` Unit Tests (30+ tests)

The unit tests in `bronze.rs` (`mod tests`, starting at line 574) use `MockRawStore` and never touch `ParquetStore` or Polars. They are completely unaffected by air-018.

### 4.3 Production Import Changes

The production code in `parquet.rs` currently has `use polars::prelude::*;` at line 9. After air-018, this import changes to:

```rust
use arrow::array::{
    Float64Array, Int64Array, StringArray, ArrayRef,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
```

The test module (`mod tests`) currently inherits `use super::*;` which brings in the Polars import. After air-018, `super::*` will bring in arrow/parquet imports instead. Test #25 needs explicit arrow-rs reader imports as described in Category 2.

---

## 5. Test Execution Order

During implementation, tests should be run in this order:

### Phase 1: Schema compatibility (prove output format is identical)

1. **T-NEW-01** -- TimeSeriesPoint 6-column schema verification
2. **T-NEW-02** -- RawDataPoint 5-column schema verification
3. **T-NEW-03** -- Nullable column handling with mixed Some/None

These must pass before proceeding. If the schema is wrong, all downstream tests will fail for the wrong reason.

### Phase 2: Existing unit tests through trait interface (should pass without changes)

4. Run all 34 Category 1 unit tests from `parquet.rs`:
   ```
   cargo test --package neural-core storage::parquet::tests
   ```
   Exclude `test_raw_parquet_schema_has_5_columns` temporarily (it uses Polars directly).

### Phase 3: Updated assertion tests

5. **Test #25 updated** -- `test_raw_parquet_schema_has_5_columns` rewritten with arrow-rs reader

### Phase 4: Integration tests

6. Run all 4 integration tests from `bronze.rs`:
   ```
   cargo test --package neural-core subscribers::bronze::integration_tests
   ```

### Phase 5: New edge case and stress tests

7. **T-NEW-04** -- Empty batch handling
8. **T-NEW-05** -- Large batch stress test (10,000 points)

### Phase 6: Cross-read compatibility (manual or temporary)

9. **T-NEW-06** -- Manual or dev-dependency cross-read verification

### Final: Full suite

10. Run the complete test suite:
    ```
    cargo test --workspace
    ```

---

## 6. Regression Prevention

| Risk | Source | Mitigation |
|------|--------|------------|
| Silver ETL reads Parquet files with different schema | `silver-etl` crate reads Bronze Parquet via its own reader | T-NEW-01 and T-NEW-02 verify schema identity. Integration env test with Silver ETL reading new Parquet files. |
| MCP server `query_raw` returns wrong data | `ndp-mcp-server` calls `ParquetStore::query_raw()` | Tests #27, #33, #34, #35 verify query_raw round-trips. Integration tests I-1, I-4 use real query_raw. |
| BronzeSubscriber snapshot cycle produces corrupt files | `bronze.rs` calls `store.write_raw_snapshot()` | Integration tests I-1, I-2, I-3, I-4 exercise the full snapshot cycle with real ParquetStore. |
| WAL replay recovery reads Parquet with wrong schema | `ParquetStore::replay_wal()` and `bronze.rs::recover()` | Test #10 (WAL replay) and integration test I-2 (crash recovery) both verify this path. |
| Nullable columns silently drop data or produce wrong nulls | arrow-rs nullable array construction differs from Polars | T-NEW-03 explicitly tests mixed null/non-null patterns. Tests #19-22 verify ndp_id and context round-trip. |
| Parquet compression changes or is omitted | ArrowWriter properties must specify Snappy | T-NEW-01 and T-NEW-02 verify compression metadata from file. |
| Large batches cause OOM or off-by-one | Array builder capacity mismatch | T-NEW-05 tests 10,000 points to catch allocation issues. |
| `append_to_parquet` (read-modify-write) breaks for TimeSeriesPoint | Legacy path reads existing Parquet, adds points, rewrites | Tests #2 (write + verify), #11 (partition pruning multi-write), #12 (multiple locations) all exercise append. |
| `append_to_raw_parquet` (deprecated read-modify-write) breaks for RawDataPoint | Used by `write_raw()` single-point path | Tests #26, #27, #28 exercise `write_raw()` which calls `append_to_raw_parquet`. |

---

## 7. Test Dependencies

### Current test imports (before air-018)

The `mod tests` block in `parquet.rs` uses `use super::*;` which brings in `polars::prelude::*` from the module-level import. Test #25 is the only test that directly uses Polars APIs (`ParquetReader`, `df.get_column_names()`).

### After air-018

- Module-level import changes from `polars::prelude::*` to `arrow` + `parquet` crate imports
- `use super::*;` in test module will bring in the new arrow/parquet imports
- Test #25 rewrite uses `ParquetRecordBatchReaderBuilder` from `parquet::arrow`
- New tests (T-NEW-01 through T-NEW-05) use `ParquetRecordBatchReaderBuilder` and arrow schema inspection
- **No new test-only dependencies needed** -- `arrow` and `parquet` crates are production dependencies

### Polars in dev-dependencies (optional, temporary)

The SCOPE.md allows keeping `polars` in `core`'s `[dev-dependencies]` for read-side test assertions. This is ONLY needed if T-NEW-06 (cross-read compatibility) is implemented as an automated test. Once compatibility is confirmed, `polars` can be removed from dev-dependencies entirely.

---

## 8. Test File Summary

| File | Test Count | AIR-018 Changes |
|------|-----------|----------------|
| `core/src/storage/parquet.rs` (`mod tests`) | 35 | 1 test rewritten (#25), 5 new tests added (T-NEW-01 through T-NEW-05) |
| `core/src/subscribers/bronze.rs` (`mod tests`) | 30+ | None |
| `core/src/subscribers/bronze.rs` (`mod integration_tests`) | 4 | None |

**Total tests affected by code changes:** 1 (rewrite)
**Total new tests added:** 5-6
**Total tests expected to pass unchanged:** 38 (Category 1)

---

## 9. Success Criteria

Air-018 is test-complete when:

1. All 35 existing unit tests in `parquet.rs` pass (34 unchanged + 1 rewritten)
2. All 4 integration tests in `bronze.rs` pass unchanged
3. All 30+ unit tests in `bronze.rs` pass unchanged
4. T-NEW-01 and T-NEW-02 verify schema identity (column names, types, compression)
5. T-NEW-03 verifies correct null bitmap handling
6. T-NEW-04 verifies empty batch behavior
7. T-NEW-05 verifies large batch (10,000 points) without errors
8. Cross-read compatibility verified (T-NEW-06, manual or automated)
9. `cargo test --workspace` passes with zero failures
10. `polars` is removed from `core/Cargo.toml` `[dependencies]` (may remain in `[dev-dependencies]` temporarily)
