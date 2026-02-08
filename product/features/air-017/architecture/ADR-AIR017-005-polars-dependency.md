# ADR-AIR017-005: Polars Dependency Impact

## Status

Proposed

## Context

The `core` crate depends on `polars` for Parquet I/O. Polars is used in two distinct
patterns within `core/src/storage/parquet.rs`:

### Read Pattern (read-modify-write, lines 157-225 and 563-622)

`append_to_parquet()` and `append_to_raw_parquet()` use Polars to:

1. Open an existing Parquet file via `ParquetReader::new(file).finish()`.
2. Extract columns from the resulting `DataFrame` (`.column("timestamp")?.i64()?`, etc.).
3. Iterate rows to reconstruct `Vec<RawDataPoint>` (or `Vec<TimeSeriesPoint>`).
4. Append new points to the Vec.
5. Write the combined Vec back using the write pattern below.

This read-and-deserialize pattern is the O(file_size) bottleneck that AIR-017 eliminates.

### Write Pattern (full file write, lines 498-559)

`write_raw_parquet()` uses Polars to:

1. Build column vectors from `Vec<RawDataPoint>` (Vecs of i64 timestamps, Utf8 source_ids,
   Utf8 ndp_ids, Utf8 context JSON, Utf8 raw_payload JSON).
2. Construct a `DataFrame` from `Series` objects.
3. Write the DataFrame to Parquet via `ParquetWriter::new(file).finish(&mut df)`.

This write-only pattern is retained in AIR-017. The `write_raw_parquet()` method is called
by the snapshot write.

### Query Pattern (read path, lines 743-810)

`query_raw()` uses Polars to read Parquet files and extract `RawDataPoint` vectors for
Silver ETL catch-up and MCP server queries. This is unchanged by AIR-017.

### The Question

With AIR-017 eliminating the read-modify-write pattern, should Polars be replaced with
a lighter-weight library (e.g., `arrow-rs` + `parquet` crate) for the remaining
write-only and read-only operations?

### Option A: Keep Polars

Continue using Polars for both write and read operations. The read-modify-write code paths
(`append_to_parquet()`, `append_to_raw_parquet()`) are deprecated but not removed in
Phase 1. They can be removed in a future cleanup.

Pros:
- No dependency change. Zero risk of introducing new bugs.
- Polars API is ergonomic for DataFrame-style operations.
- Developers are already familiar with the Polars patterns in the codebase.
- Compile time is already paid.

Cons:
- Polars is a heavy dependency (~30 MB compiled, ~60s compile time). For the remaining
  write-only and read-only usage, it is overkill.
- Polars pulls in a large transitive dependency tree (arrow2, etc.).
- On the Pi, the compiled binary size includes Polars whether we use 10% or 100% of it.

### Option B: Replace Polars with arrow-rs + parquet crate

Use the `arrow` and `parquet` crates directly for Parquet I/O. These are the same crates
that Polars wraps internally (Polars uses its own `arrow2` fork, but the `parquet` crate
from the official Apache Arrow Rust project is an alternative).

Pros:
- Lighter dependency (arrow + parquet are ~10 MB compiled vs ~30 MB for Polars).
- More control over Parquet writer settings (compression, row group size, etc.).
- Aligns with the broader Rust ecosystem trend toward arrow-rs.

Cons:
- Requires rewriting all Parquet read and write code.
- Lower-level API: no DataFrame abstraction, manual RecordBatch construction.
- Significant effort for a marginal binary size improvement.
- Risk of introducing bugs in a critical data path.

### Option C: Replace Polars for writes only, keep for reads

Use `arrow-rs` for `write_raw_parquet()` (the hot path in AIR-017), keep Polars for
`query_raw()` (the read path).

Cons:
- Two Parquet libraries in the same crate. Both Polars and arrow-rs would be compiled.
- Increases total dependency size, not decreases it.
- Maintenance burden of two different API styles.

## Decision

**Option A: Keep Polars. Do not change the Parquet dependency in AIR-017.**

The rationale:

1. **Scope discipline**: AIR-017 is about eliminating read-modify-write and improving
   durability. It is not about optimizing dependencies. Mixing a dependency change into
   an architectural change increases risk without advancing the feature's goals.

2. **Polars usage simplifies**: After AIR-017, Polars is used in only two patterns:
   - Write: `write_raw_parquet()` -- build Series from Vecs, write DataFrame.
   - Read: `query_raw()` -- read DataFrame, extract columns.

   Both patterns are straightforward and well-tested. The read-modify-write pattern
   (the complex one) is eliminated, which actually makes Polars usage cleaner.

3. **Binary size is not a constraint**: The compiled binary runs on a Raspberry Pi 5
   with 8 GB RAM and 32+ GB storage. A 30 MB binary vs a 10 MB binary is irrelevant
   at this scale.

4. **Compile time is a developer experience issue, not a runtime issue**: Polars compile
   time (~60s) is already paid. Removing it would save time only on clean builds, which
   are rare during development.

5. **Future opportunity**: If a future feature (e.g., V2.0 rewrite, or a "slim edge" Pi
   Zero deployment) requires a smaller binary, Polars removal can be scoped as a
   standalone feature with its own testing and validation.

### Cleanup Opportunity (Not in AIR-017 Scope)

After AIR-017 Phase 1, the following methods become unused on the hot path:

- `append_to_raw_parquet()` (lines 563-622): No longer called by `write_raw_batch()`.
- `append_to_parquet()` (lines 157-225): No longer called by `write_batch()`.

These methods should be marked with `#[deprecated]` in AIR-017 and removed in a
subsequent cleanup feature. They may still be useful for one-off data repair scripts.

## Consequences

**Positive:**
- No dependency churn. The Parquet I/O code is well-tested and stable.
- AIR-017 scope stays focused on the architectural change (WAL position, accumulator,
  snapshot strategy) without conflating it with a dependency migration.
- Polars usage actually becomes simpler: the complex read-modify-write pattern is
  eliminated, leaving only straightforward write and read operations.

**Negative:**
- The Polars dependency remains, with its compile-time and binary size overhead.
  This is accepted as a non-issue for the current deployment target (Pi 5).
- The deprecated `append_to_*_parquet()` methods will exist alongside the new
  `write_raw_snapshot()` method until they are cleaned up. This creates temporary
  confusion about which method to use, mitigated by deprecation annotations and
  documentation.

**Neutral:**
- If `arrow-rs` replacement becomes desirable later, AIR-017's architectural changes
  (accumulator, snapshot overwrite, WAL in subscriber) do not depend on Polars.
  The `write_raw_parquet()` method is the only Polars integration point, and it can
  be reimplemented with arrow-rs without changing the architecture.
