# AIR-018 Pseudocode: Replace Polars with arrow-rs + parquet

> **Phase:** SPARC Pseudocode (P)
> **Feature:** air-018 (Eliminate Polars from Bronze Write Path)
> **Approach:** Option B -- full replacement of both write AND read paths in `core/src/storage/parquet.rs`
> **Author:** ndp-rust-dev agent
> **Date:** 2026-02-10

---

## Overview

This document provides method-by-method replacement pseudocode for every Polars usage in
`core/src/storage/parquet.rs` (1947 lines) and the `CoreError::Polars` variant in `core/src/error.rs`.

The replacement uses the `arrow` (arrow-rs) and `parquet` crates directly. All method signatures,
trait implementations, and Parquet file schemas remain identical. Output files are byte-compatible
with existing readers (Silver ETL, MCP server, Grafana).

### Crate Versions (Cargo.toml changes)

Workspace `Cargo.toml` additions:
```toml
# In [workspace.dependencies]
arrow = { version = "54", default-features = false, features = ["prettyprint"] }
parquet = { version = "54", default-features = false, features = ["arrow", "snap"] }
```

`core/Cargo.toml` changes:
```toml
# REMOVE:
# polars = { workspace = true }

# ADD:
arrow = { workspace = true }
parquet = { workspace = true }
```

> Note: `arrow` 54.x and `parquet` 54.x share the same arrow-rs release. The `snap` feature in
> `parquet` enables Snappy compression. The `default-features = false` avoids pulling in heavy
> optional dependencies (e.g., IPC, CSV, JSON readers).

---

## Schemas

### 6-Column TimeSeriesPoint Schema (write_parquet / append_to_parquet / query)

| Column       | Arrow DataType   | Nullable | Parquet Physical |
|-------------|-----------------|----------|-----------------|
| timestamp   | Int64           | false    | INT64           |
| location_id | Utf8            | false    | BYTE_ARRAY      |
| metric      | Utf8            | false    | BYTE_ARRAY      |
| value       | Float64         | false    | DOUBLE          |
| ndp_id      | Utf8            | true     | BYTE_ARRAY      |
| context     | Utf8            | true     | BYTE_ARRAY      |

### 5-Column RawDataPoint Schema (write_raw_parquet / append_to_raw_parquet / query_raw)

| Column       | Arrow DataType   | Nullable | Parquet Physical |
|-------------|-----------------|----------|-----------------|
| timestamp   | Int64           | false    | INT64           |
| source_id   | Utf8            | false    | BYTE_ARRAY      |
| ndp_id      | Utf8            | true     | BYTE_ARRAY      |
| context     | Utf8            | true     | BYTE_ARRAY      |
| raw_payload | Utf8            | false    | BYTE_ARRAY      |

---

## Import Changes

### Current (Polars)

```rust
use polars::prelude::*;
```

### New (arrow-rs + parquet)

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

## P-01: `write_parquet()` (6-column TimeSeriesPoint schema)

### Current Signature (unchanged)

```rust
async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()>
```

### Current Approach

Creates 6 Polars `Series` from Vecs, builds a `DataFrame`, writes via `ParquetWriter` with Snappy.

### New Pseudocode

```rust
async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
    if points.is_empty() {
        return Ok(());
    }

    let path = path.to_path_buf();

    // Move CPU-intensive work to blocking thread pool (AIR-010 P3-02)
    tokio::task::spawn_blocking(move || {
        let parent = path.parent().ok_or_else(|| {
            CoreError::Storage("Invalid path: no parent directory".to_string())
        })?;
        std::fs::create_dir_all(parent)?;

        // -- Build the Arrow schema (6 columns) --
        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("location_id", DataType::Utf8, false),
            Field::new("metric", DataType::Utf8, false),
            Field::new("value", DataType::Float64, false),
            Field::new("ndp_id", DataType::Utf8, true),   // nullable
            Field::new("context", DataType::Utf8, true),   // nullable
        ]));

        // -- Pre-allocate column Vecs (P2-02) --
        let len = points.len();
        let mut timestamps = Vec::with_capacity(len);
        let mut location_ids = Vec::with_capacity(len);
        let mut metrics = Vec::with_capacity(len);
        let mut values = Vec::with_capacity(len);
        // For nullable columns, use Vec<Option<String>> so Arrow builds a null bitmap
        let mut ndp_ids: Vec<Option<String>> = Vec::with_capacity(len);
        let mut contexts: Vec<Option<String>> = Vec::with_capacity(len);

        for p in &points {
            timestamps.push(p.timestamp.timestamp_micros());
            location_ids.push(p.location_id.as_str());
            metrics.push(
                p.tags
                    .get("metric")
                    .map(|s| s.as_str())
                    .unwrap_or("unknown"),
            );
            values.push(p.value);
            ndp_ids.push(p.ndp_id.clone());
            contexts.push(p.context.as_ref().map(|c| c.to_string()));
        }

        // -- Build Arrow arrays --
        let ts_array = Int64Array::from(timestamps);
        let loc_array = StringArray::from(
            location_ids.into_iter().collect::<Vec<&str>>(),
        );
        let metric_array = StringArray::from(
            metrics.into_iter().collect::<Vec<&str>>(),
        );
        let val_array = Float64Array::from(values);
        // Nullable StringArray from Vec<Option<String>>:
        let ndp_id_array = StringArray::from(
            ndp_ids
                .iter()
                .map(|opt| opt.as_deref())
                .collect::<Vec<Option<&str>>>(),
        );
        let context_array = StringArray::from(
            contexts
                .iter()
                .map(|opt| opt.as_deref())
                .collect::<Vec<Option<&str>>>(),
        );

        // -- Build RecordBatch --
        let batch = RecordBatch::try_new(schema.clone(), vec![
            Arc::new(ts_array) as ArrayRef,
            Arc::new(loc_array) as ArrayRef,
            Arc::new(metric_array) as ArrayRef,
            Arc::new(val_array) as ArrayRef,
            Arc::new(ndp_id_array) as ArrayRef,
            Arc::new(context_array) as ArrayRef,
        ])
        .map_err(|e| CoreError::Storage(format!("Failed to create RecordBatch: {}", e)))?;

        // -- Write Parquet with Snappy compression --
        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build();
        let file = std::fs::File::create(&path)?;
        let mut writer = ArrowWriter::try_new(file, schema, Some(props))
            .map_err(|e| CoreError::Storage(format!("Failed to create ArrowWriter: {}", e)))?;
        writer
            .write(&batch)
            .map_err(|e| CoreError::Storage(format!("Failed to write Parquet: {}", e)))?;
        writer
            .close()
            .map_err(|e| CoreError::Storage(format!("Failed to close Parquet writer: {}", e)))?;

        Ok::<_, CoreError>(())
    })
    .await
    .map_err(|e| CoreError::Storage(format!("Parquet write task panicked: {}", e)))??;

    Ok(())
}
```

### Key Differences from Polars

1. **No DataFrame** -- `RecordBatch` replaces `DataFrame`. One fewer allocation layer.
2. **Explicit schema** -- Schema is declared up front with nullable flags.
3. **Nullable columns** -- `StringArray::from(Vec<Option<&str>>)` automatically sets the null bitmap.
   In Polars, `Series::new("ndp_id", ndp_ids)` where `ndp_ids: Vec<Option<String>>` also handles
   nullability, but through Polars' internal ChunkedArray.
4. **Writer lifecycle** -- `ArrowWriter::try_new` + `.write()` + `.close()` instead of
   `ParquetWriter::new(file).with_compression(...).finish(&mut df)`. The `.close()` call is
   required to flush buffered row groups and write the Parquet footer.

---

## P-02: `write_raw_parquet()` (5-column RawDataPoint schema)

### Current Signature (unchanged)

```rust
pub async fn write_raw_parquet(&self, points: Vec<RawDataPoint>, path: &Path) -> CoreResult<()>
```

### Current Approach

Creates 5 Polars `Series` from Vecs, builds a `DataFrame`, writes via `ParquetWriter` with Snappy.

### New Pseudocode

```rust
pub async fn write_raw_parquet(&self, points: Vec<RawDataPoint>, path: &Path) -> CoreResult<()> {
    if points.is_empty() {
        return Ok(());
    }

    let path = path.to_path_buf();

    // Move CPU-intensive work to blocking thread pool (AIR-010 P3-02)
    tokio::task::spawn_blocking(move || {
        let parent = path.parent().ok_or_else(|| {
            CoreError::Storage("Invalid path: no parent directory".to_string())
        })?;
        std::fs::create_dir_all(parent)?;

        // -- Build the Arrow schema (5 columns) --
        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("ndp_id", DataType::Utf8, true),       // nullable
            Field::new("context", DataType::Utf8, true),       // nullable
            Field::new("raw_payload", DataType::Utf8, false),
        ]));

        // -- Pre-allocate column Vecs (P2-02) --
        let len = points.len();
        let mut timestamps = Vec::with_capacity(len);
        let mut source_ids = Vec::with_capacity(len);
        let mut ndp_ids: Vec<Option<String>> = Vec::with_capacity(len);
        let mut contexts: Vec<Option<String>> = Vec::with_capacity(len);
        let mut raw_payloads = Vec::with_capacity(len);

        for p in &points {
            timestamps.push(p.timestamp.timestamp_micros());
            source_ids.push(p.source_id.as_str());
            ndp_ids.push(p.ndp_id.clone());
            contexts.push(p.context.as_ref().map(|c| c.to_string()));
            raw_payloads.push(p.raw_payload.to_string());
        }

        // -- Build Arrow arrays --
        let ts_array = Int64Array::from(timestamps);
        let source_id_array = StringArray::from(
            source_ids.into_iter().collect::<Vec<&str>>(),
        );
        // Nullable arrays from Vec<Option<String>>
        let ndp_id_array = StringArray::from(
            ndp_ids
                .iter()
                .map(|opt| opt.as_deref())
                .collect::<Vec<Option<&str>>>(),
        );
        let context_array = StringArray::from(
            contexts
                .iter()
                .map(|opt| opt.as_deref())
                .collect::<Vec<Option<&str>>>(),
        );
        let payload_array = StringArray::from(
            raw_payloads.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
        );

        // -- Build RecordBatch --
        let batch = RecordBatch::try_new(schema.clone(), vec![
            Arc::new(ts_array) as ArrayRef,
            Arc::new(source_id_array) as ArrayRef,
            Arc::new(ndp_id_array) as ArrayRef,
            Arc::new(context_array) as ArrayRef,
            Arc::new(payload_array) as ArrayRef,
        ])
        .map_err(|e| CoreError::Storage(format!("Failed to create RecordBatch: {}", e)))?;

        // -- Write Parquet with Snappy compression --
        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build();
        let file = std::fs::File::create(&path)?;
        let mut writer = ArrowWriter::try_new(file, schema, Some(props))
            .map_err(|e| CoreError::Storage(format!("Failed to create ArrowWriter: {}", e)))?;
        writer
            .write(&batch)
            .map_err(|e| CoreError::Storage(format!("Failed to write Parquet: {}", e)))?;
        writer
            .close()
            .map_err(|e| CoreError::Storage(format!("Failed to close Parquet writer: {}", e)))?;

        Ok::<_, CoreError>(())
    })
    .await
    .map_err(|e| CoreError::Storage(format!("Parquet write task panicked: {}", e)))??;

    Ok(())
}
```

### Key Differences from P-01

- 5 columns instead of 6 (no `location_id`, `metric`, `value`; uses `source_id`, `raw_payload` instead).
- `raw_payload` is serialized to JSON string via `p.raw_payload.to_string()` -- same as current Polars code.
- `source_ids` uses `&str` references since RawDataPoint owns its source_id (avoids cloning into the Vec).

---

## P-03: `append_to_parquet()` -- Read existing Parquet into TimeSeriesPoint Vec

### Current Signature (unchanged)

```rust
async fn append_to_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()>
```

### Current Approach

Opens existing Parquet with `ParquetReader`, reads into `DataFrame`, extracts columns via
`df.column("x")?.i64()?` / `.utf8()?`, iterates rows, builds `TimeSeriesPoint` structs,
appends to new points, writes all via `write_parquet`.

### New Pseudocode

```rust
async fn append_to_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
    let mut all_points = points;

    if path.exists() {
        let file = std::fs::File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|e| CoreError::Storage(format!("Failed to read existing Parquet: {}", e)))?;
        let reader = builder.build()
            .map_err(|e| CoreError::Storage(format!("Failed to build Parquet reader: {}", e)))?;

        for batch_result in reader {
            let batch = batch_result
                .map_err(|e| CoreError::Storage(format!("Failed to read batch: {}", e)))?;

            let num_rows = batch.num_rows();

            // -- Downcast required columns --
            let timestamps = batch
                .column_by_name("timestamp")
                .ok_or_else(|| CoreError::Storage("Missing timestamp column".to_string()))?
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| CoreError::Storage("Invalid timestamp type".to_string()))?;

            let location_ids = batch
                .column_by_name("location_id")
                .ok_or_else(|| CoreError::Storage("Missing location_id column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| CoreError::Storage("Invalid location_id type".to_string()))?;

            let metrics = batch
                .column_by_name("metric")
                .ok_or_else(|| CoreError::Storage("Missing metric column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| CoreError::Storage("Invalid metric type".to_string()))?;

            let values = batch
                .column_by_name("value")
                .ok_or_else(|| CoreError::Storage("Missing value column".to_string()))?
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| CoreError::Storage("Invalid value type".to_string()))?;

            // -- Optional nullable columns (ndp_id, context) --
            // Use column_by_name which returns Option; if column missing, treat as all-null.
            let ndp_ids = batch
                .column_by_name("ndp_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let contexts = batch
                .column_by_name("context")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            // -- Iterate rows --
            for i in 0..num_rows {
                // Required columns: skip row if any is null (matches Polars behavior)
                if timestamps.is_null(i)
                    || location_ids.is_null(i)
                    || metrics.is_null(i)
                    || values.is_null(i)
                {
                    continue;
                }

                let ts = timestamps.value(i);
                let loc = location_ids.value(i);
                let metric = metrics.value(i);
                let val = values.value(i);

                let timestamp = DateTime::from_timestamp_micros(ts)
                    .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                let mut tags = HashMap::new();
                tags.insert("metric".to_string(), metric.to_string());

                // Nullable columns: check is_null before accessing value
                let ndp_id = ndp_ids.and_then(|col| {
                    if col.is_null(i) {
                        None
                    } else {
                        Some(col.value(i).to_string())
                    }
                });

                let context = contexts.and_then(|col| {
                    if col.is_null(i) {
                        None
                    } else {
                        serde_json::from_str(col.value(i)).ok()
                    }
                });

                all_points.push(TimeSeriesPoint {
                    timestamp,
                    location_id: loc.to_string(),
                    value: val,
                    tags,
                    ndp_id,
                    context,
                });
            }
        }
    }

    self.write_parquet(all_points, path).await
}
```

### Key Differences from Polars Read Path

1. **No DataFrame** -- `ParquetRecordBatchReaderBuilder` yields `RecordBatch` iterators directly.
2. **Column access** -- `batch.column_by_name("x")` returns `Option<&ArrayRef>` instead of
   `df.column("x")` returning `Result`. Downcast via `.as_any().downcast_ref::<T>()`.
3. **Null checking** -- Explicit `.is_null(i)` check before `.value(i)`. In Polars, `.get(i)`
   returns `Option<T>` which handles nulls implicitly. With Arrow, `.value(i)` on a null index
   returns the default value (0 for ints, "" for strings), so you MUST check `.is_null(i)` first.
4. **Batch iteration** -- Reader returns multiple batches (one per row group). The `for batch_result
   in reader` loop handles this. In Polars, `ParquetReader::new(file).finish()` loads the entire
   file into one DataFrame.

---

## P-04: `append_to_raw_parquet()` -- Read existing raw Parquet into RawDataPoint Vec

### Current Signature (unchanged)

```rust
#[deprecated(since = "1.2.0", note = "...")]
async fn append_to_raw_parquet(
    &self,
    points: Vec<RawDataPoint>,
    path: PathBuf,
) -> CoreResult<()>
```

### Current Approach

Opens existing Parquet, reads `DataFrame`, extracts 5 columns, iterates rows to build
`RawDataPoint` structs, appends to new points, writes all via `write_raw_parquet`.

### New Pseudocode

```rust
#[deprecated(
    since = "1.2.0",
    note = "Use write_raw_snapshot for full-overwrite snapshot writes (AIR-017). \
            This read-modify-write path is retained only for legacy write_raw() callers."
)]
async fn append_to_raw_parquet(
    &self,
    points: Vec<RawDataPoint>,
    path: PathBuf,
) -> CoreResult<()> {
    let mut all_points = points;

    if path.exists() {
        let file = std::fs::File::open(&path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|e| CoreError::Storage(format!("Failed to read existing Parquet: {}", e)))?;
        let reader = builder.build()
            .map_err(|e| CoreError::Storage(format!("Failed to build Parquet reader: {}", e)))?;

        for batch_result in reader {
            let batch = batch_result
                .map_err(|e| CoreError::Storage(format!("Failed to read batch: {}", e)))?;

            let num_rows = batch.num_rows();

            // -- Downcast required columns --
            let timestamps = batch
                .column_by_name("timestamp")
                .ok_or_else(|| CoreError::Storage("Missing timestamp column".to_string()))?
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| CoreError::Storage("Invalid timestamp type".to_string()))?;

            let source_ids = batch
                .column_by_name("source_id")
                .ok_or_else(|| CoreError::Storage("Missing source_id column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| CoreError::Storage("Invalid source_id type".to_string()))?;

            let raw_payloads = batch
                .column_by_name("raw_payload")
                .ok_or_else(|| CoreError::Storage("Missing raw_payload column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| CoreError::Storage("Invalid raw_payload type".to_string()))?;

            // -- Optional nullable columns --
            let ndp_ids = batch
                .column_by_name("ndp_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let contexts = batch
                .column_by_name("context")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            // -- Iterate rows --
            for i in 0..num_rows {
                if timestamps.is_null(i) || source_ids.is_null(i) || raw_payloads.is_null(i) {
                    continue;
                }

                let ts = timestamps.value(i);
                let source_id = source_ids.value(i);
                let payload_str = raw_payloads.value(i);

                let timestamp = DateTime::from_timestamp_micros(ts)
                    .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                let ndp_id = ndp_ids.and_then(|col| {
                    if col.is_null(i) {
                        None
                    } else {
                        Some(col.value(i).to_string())
                    }
                });

                let context = contexts.and_then(|col| {
                    if col.is_null(i) {
                        None
                    } else {
                        serde_json::from_str(col.value(i)).ok()
                    }
                });

                let raw_payload: serde_json::Value = serde_json::from_str(payload_str)
                    .map_err(|e| CoreError::Storage(format!("Invalid JSON payload: {}", e)))?;

                all_points.push(RawDataPoint {
                    timestamp,
                    source_id: source_id.to_string(),
                    ndp_id,
                    context,
                    raw_payload,
                });
            }
        }
    }

    self.write_raw_parquet(all_points, &path).await
}
```

### Notes

- Structurally identical to P-03 but with 5-column raw schema.
- `raw_payload` is read as a string and deserialized via `serde_json::from_str` -- unchanged from Polars.
- The `#[deprecated]` attribute is preserved exactly.

---

## P-05: `query()` -- Read with lazy filter replacement

### Current Signature (unchanged)

```rust
async fn query(
    &self,
    location_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    _filters: Option<HashMap<String, String>>,
) -> CoreResult<Vec<TimeSeriesPoint>>
```

### Current Approach

Opens Parquet file, uses `df.lazy().filter(col("timestamp").gt_eq(lit(start))...collect()` for
timestamp filtering, then iterates rows to build points.

### New Pseudocode

```rust
async fn query(
    &self,
    location_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    _filters: Option<HashMap<String, String>>,
) -> CoreResult<Vec<TimeSeriesPoint>> {
    let mut all_points = Vec::new();

    let start_micros = start.timestamp_micros();
    let end_micros = end.timestamp_micros();

    let mut current = start;
    while current <= end {
        let path = self.partition_path(location_id, current);

        if path.exists() {
            let file = std::fs::File::open(&path)?;
            let builder = ParquetRecordBatchReaderBuilder::try_new(file)
                .map_err(|e| CoreError::Storage(format!("Failed to read Parquet: {}", e)))?;
            let reader = builder.build()
                .map_err(|e| CoreError::Storage(format!("Failed to build Parquet reader: {}", e)))?;

            for batch_result in reader {
                let batch = batch_result
                    .map_err(|e| CoreError::Storage(format!("Failed to read batch: {}", e)))?;

                let num_rows = batch.num_rows();

                // -- Downcast columns --
                let timestamps = batch
                    .column_by_name("timestamp")
                    .ok_or_else(|| CoreError::Storage("Missing timestamp column".to_string()))?
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .ok_or_else(|| CoreError::Storage("Invalid timestamp type".to_string()))?;

                let location_ids = batch
                    .column_by_name("location_id")
                    .ok_or_else(|| CoreError::Storage("Missing location_id column".to_string()))?
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| CoreError::Storage("Invalid location_id type".to_string()))?;

                let metrics_col = batch
                    .column_by_name("metric")
                    .ok_or_else(|| CoreError::Storage("Missing metric column".to_string()))?
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| CoreError::Storage("Invalid metric type".to_string()))?;

                let values = batch
                    .column_by_name("value")
                    .ok_or_else(|| CoreError::Storage("Missing value column".to_string()))?
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| CoreError::Storage("Invalid value type".to_string()))?;

                let ndp_ids = batch
                    .column_by_name("ndp_id")
                    .and_then(|c| c.as_any().downcast_ref::<StringArray>());

                let contexts = batch
                    .column_by_name("context")
                    .and_then(|c| c.as_any().downcast_ref::<StringArray>());

                // -- Iterate rows with timestamp filter (replaces Polars lazy filter) --
                for i in 0..num_rows {
                    if timestamps.is_null(i)
                        || location_ids.is_null(i)
                        || metrics_col.is_null(i)
                        || values.is_null(i)
                    {
                        continue;
                    }

                    let ts = timestamps.value(i);

                    // FILTER: replaces df.lazy().filter(col("timestamp").gt_eq/lt_eq)
                    if ts < start_micros || ts > end_micros {
                        continue;
                    }

                    let timestamp = DateTime::from_timestamp_micros(ts)
                        .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                    let loc = location_ids.value(i);
                    let metric = metrics_col.value(i);
                    let val = values.value(i);

                    let mut tags = HashMap::new();
                    tags.insert("metric".to_string(), metric.to_string());

                    let ndp_id = ndp_ids.and_then(|col| {
                        if col.is_null(i) {
                            None
                        } else {
                            Some(col.value(i).to_string())
                        }
                    });

                    let context = contexts.and_then(|col| {
                        if col.is_null(i) {
                            None
                        } else {
                            serde_json::from_str(col.value(i)).ok()
                        }
                    });

                    all_points.push(TimeSeriesPoint {
                        timestamp,
                        location_id: loc.to_string(),
                        value: val,
                        tags,
                        ndp_id,
                        context,
                    });
                }
            }
        }

        current = current + chrono::Duration::days(1);
    }

    Ok(all_points)
}
```

### Key Differences

1. **No lazy filter** -- Polars' `df.lazy().filter(col("timestamp").gt_eq(lit(start)))...collect()`
   is replaced by a simple `if ts < start_micros || ts > end_micros { continue; }` check in the
   row loop. This is functionally identical for our workload size (daily partitions with hundreds
   to low thousands of rows). Polars' lazy filter evaluates the same predicate but adds DataFrame
   materialization overhead.
2. **Pre-computed micros** -- `start_micros` and `end_micros` are computed once before the loop
   instead of on every comparison. The Polars approach used `lit(start.timestamp_micros())` which
   also only evaluates once, so this is equivalent.
3. **Error context** -- The Polars version used `?` on `df.column("timestamp")?.i64()?` which
   auto-converted `PolarsError` to `CoreError::Polars` via `From`. The new version uses explicit
   `.ok_or_else(|| CoreError::Storage(...))` which is more descriptive.

---

## P-06: `query_raw()` -- Read raw Parquet with filters

### Current Signature (unchanged)

```rust
async fn query_raw(
    &self,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    source_filter: Option<String>,
) -> CoreResult<Vec<RawDataPoint>>
```

### Current Approach

Finds partition files, opens each with `ParquetReader`, extracts columns via `df.column("x")?.utf8()?`,
iterates rows with manual time and source filters.

### New Pseudocode

```rust
async fn query_raw(
    &self,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    source_filter: Option<String>,
) -> CoreResult<Vec<RawDataPoint>> {
    let partition_files = self.find_raw_partitions(start, end, source_filter.as_deref())?;
    // M-005: Pre-allocate based on partition count (estimate ~100 points per file)
    let mut all_points = Vec::with_capacity(partition_files.len() * 100);

    for path in partition_files {
        let file = std::fs::File::open(&path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|e| CoreError::Storage(format!("Failed to read Parquet: {}", e)))?;
        let reader = builder.build()
            .map_err(|e| CoreError::Storage(format!("Failed to build Parquet reader: {}", e)))?;

        for batch_result in reader {
            let batch = batch_result
                .map_err(|e| CoreError::Storage(format!("Failed to read batch: {}", e)))?;

            let num_rows = batch.num_rows();

            // -- Downcast columns --
            let timestamps = batch
                .column_by_name("timestamp")
                .ok_or_else(|| CoreError::Storage("Missing timestamp column".to_string()))?
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| CoreError::Storage("Invalid timestamp type".to_string()))?;

            let source_ids = batch
                .column_by_name("source_id")
                .ok_or_else(|| CoreError::Storage("Missing source_id column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| CoreError::Storage("Invalid source_id type".to_string()))?;

            let ndp_ids = batch
                .column_by_name("ndp_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let contexts = batch
                .column_by_name("context")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let raw_payloads = batch
                .column_by_name("raw_payload")
                .ok_or_else(|| CoreError::Storage("Missing raw_payload column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| CoreError::Storage("Invalid raw_payload type".to_string()))?;

            // -- Iterate rows with filters --
            for i in 0..num_rows {
                if timestamps.is_null(i) || source_ids.is_null(i) || raw_payloads.is_null(i) {
                    continue;
                }

                let ts = timestamps.value(i);
                let source_id = source_ids.value(i);
                let payload_str = raw_payloads.value(i);

                let timestamp = DateTime::from_timestamp_micros(ts)
                    .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                // Apply time filter
                if timestamp < start || timestamp > end {
                    continue;
                }

                // Apply source filter
                if let Some(ref filter) = source_filter {
                    if source_id != filter {
                        continue;
                    }
                }

                let ndp_id = ndp_ids.and_then(|col| {
                    if col.is_null(i) {
                        None
                    } else {
                        Some(col.value(i).to_string())
                    }
                });

                let context = contexts.and_then(|col| {
                    if col.is_null(i) {
                        None
                    } else {
                        serde_json::from_str(col.value(i)).ok()
                    }
                });

                let raw_payload: serde_json::Value = serde_json::from_str(payload_str)
                    .map_err(|e| CoreError::Storage(format!("Invalid JSON payload: {}", e)))?;

                all_points.push(RawDataPoint {
                    timestamp,
                    source_id: source_id.to_string(),
                    ndp_id,
                    context,
                    raw_payload,
                });
            }
        }
    }

    Ok(all_points)
}
```

### Notes

- The current Polars code already uses manual time/source filters in the row loop (no lazy filter).
  The arrow-rs version is structurally identical except `ParquetReader` is replaced by
  `ParquetRecordBatchReaderBuilder` and column access uses downcast instead of `.utf8()?.get(i)`.

---

## Helper: Nullable Column Reader Pattern

The following pattern appears in every read method. Extract as a helper to reduce repetition
during implementation:

```rust
/// Read a nullable string value from a StringArray at index i.
/// Returns None if the column reference is None or the value at i is null.
fn read_nullable_string(col: Option<&StringArray>, i: usize) -> Option<String> {
    col.and_then(|c| {
        if c.is_null(i) {
            None
        } else {
            Some(c.value(i).to_string())
        }
    })
}

/// Read a nullable JSON value from a StringArray at index i.
/// Returns None if the column reference is None, the value is null,
/// or JSON deserialization fails.
fn read_nullable_json(col: Option<&StringArray>, i: usize) -> Option<serde_json::Value> {
    col.and_then(|c| {
        if c.is_null(i) {
            None
        } else {
            serde_json::from_str(c.value(i)).ok()
        }
    })
}
```

Usage in row loops:
```rust
let ndp_id = read_nullable_string(ndp_ids, i);
let context = read_nullable_json(contexts, i);
```

---

## Error Handling Changes

### `core/src/error.rs`

The `CoreError::Polars` variant and its `From<PolarsError>` impl must be updated.

**Option A (Recommended):** Replace `Polars` variant with `Arrow` variant:

```rust
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Source error: {0}")]
    Source(String),

    #[error("Forecast error: {0}")]
    Forecast(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // CHANGED: Was CoreError::Polars, now CoreError::Arrow
    #[error("Arrow error: {0}")]
    Arrow(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Prediction error: {0}")]
    PredictionError(String),

    #[error("Parser error: {0}")]
    Parser(String),
}

// CHANGED: Was From<polars::error::PolarsError>
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

pub type CoreResult<T> = Result<T, CoreError>;
```

**Impact analysis:** Search the codebase for any code matching on `CoreError::Polars`. If found
outside `core/`, those patterns must be updated to `CoreError::Arrow`. This is checked during
refinement phase.

**Option B (Conservative):** Keep `CoreError::Polars` name but change the `From` impl. This avoids
any match-arm changes elsewhere but leaves a misleading name. Not recommended.

---

## Unchanged Methods

The following methods/sections in `parquet.rs` require NO changes because they contain no Polars
usage:

| Method/Section | Lines | Reason |
|----------------|-------|--------|
| `ParquetStore::new()` | 21-32 | Filesystem + WAL only |
| `replay_wal()` | 34-60 | JSON deserialization + write_batch call |
| `partition_path()` | 66-75 | PathBuf construction only |
| `get_partition_key()` | 78-84 | HashMap lookup only |
| `raw_partition_path()` | 486-496 | PathBuf construction only |
| `base_path()` | 501-503 | Getter only |
| `extract_stream_id()` | 460-470 | String parsing only |
| `Store::write()` | 230-240 | WAL + calls append_to_parquet |
| `Store::write_batch()` | 246-276 | WAL + groups + calls append_to_parquet |
| `Store::aggregate()` | 352-417 | Pure computation on Vec, no Parquet I/O |
| `Store::health_check()` | 419-444 | Filesystem checks only |
| `RawStore::write_raw()` | 713-724 | WAL + calls append_to_raw_parquet |
| `RawStore::write_raw_batch()` | 733-751 | Groups + calls write_raw_parquet |
| `RawStore::write_raw_snapshot()` | 753-759 | Delegates to write_raw_parquet |
| `find_raw_partitions()` | 647-687 | Filesystem walk only |
| `collect_partition_files()` | 690-707 | Filesystem walk only |

---

## Test Updates

### Current Test Verification Pattern (Polars)

Tests currently verify Parquet output using Polars:
```rust
// In test_raw_parquet_schema_has_5_columns (line 1583):
let file = std::fs::File::open(&path).unwrap();
let df = ParquetReader::new(file).finish().unwrap();
let column_names: Vec<&str> = df.get_column_names();
assert_eq!(column_names.len(), 5);
```

### New Test Verification Pattern (arrow-rs)

```rust
// Replace ParquetReader with arrow-rs reader
let file = std::fs::File::open(&path).unwrap();
let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
let schema = builder.schema().clone();
let reader = builder.build().unwrap();

// Verify schema column count and names
assert_eq!(schema.fields().len(), 5, "Should have exactly 5 columns");
assert!(schema.field_with_name("timestamp").is_ok());
assert!(schema.field_with_name("source_id").is_ok());
assert!(schema.field_with_name("ndp_id").is_ok());
assert!(schema.field_with_name("context").is_ok());
assert!(schema.field_with_name("raw_payload").is_ok());

// Verify data by reading batches
let batches: Vec<RecordBatch> = reader.collect::<Result<Vec<_>, _>>().unwrap();
assert!(!batches.is_empty());

// Access specific column values for verification
let batch = &batches[0];
let source_ids = batch
    .column_by_name("source_id")
    .unwrap()
    .as_any()
    .downcast_ref::<StringArray>()
    .unwrap();
assert_eq!(source_ids.value(0), "test-Http");
```

### Tests That Need Updating

The following tests read Parquet files using `ParquetReader` (Polars) and must switch to arrow-rs:

| Test | Line | What Changes |
|------|------|-------------|
| `test_raw_parquet_schema_has_5_columns` | 1583 | `ParquetReader::new(file).finish()` -> `ParquetRecordBatchReaderBuilder::try_new(file)` |

All other tests use the `Store` and `RawStore` trait methods (write/query) for round-trip
verification and do not directly read Parquet files. Those tests require NO changes because
the trait signatures are unchanged.

---

## Cargo.toml Dependency Changes

### Workspace `Cargo.toml` (root)

```toml
[workspace.dependencies]
# ADD these (polars stays for other crates):
arrow = { version = "54", default-features = false, features = ["prettyprint"] }
parquet = { version = "54", default-features = false, features = ["arrow", "snap"] }
```

### `core/Cargo.toml`

```toml
[dependencies]
# REMOVE:
# polars = { workspace = true }

# ADD:
arrow = { workspace = true }
parquet = { workspace = true }
```

### Other Crates (NO CHANGE)

`apps/air-quality-app/Cargo.toml` and `apps/silver-etl/Cargo.toml` retain their `polars`
dependency. Those crates use Polars for their own purposes (Silver ETL, app-level reading) and
are out of scope for air-018.

---

## Build Verification Checklist

After implementation, verify:

- [ ] `cargo build -p platform-core` -- core builds without polars
- [ ] `cargo build -p platform-core --features timescale` -- timescale feature still works
- [ ] `cargo test -p platform-core` -- all existing tests pass
- [ ] `cargo clippy -p platform-core` -- no new warnings
- [ ] `cargo build` -- full workspace builds (other crates keep polars)
- [ ] Write a test Parquet file with new code, read it with Polars (in air-quality-app tests or
  manually) to confirm schema compatibility

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Schema mismatch breaks Silver ETL | Verify column names, types, and null flags match exactly. Silver ETL reads the same Parquet files. |
| Arrow crate version mismatch with parquet crate | Pin both to same major version (54.x). They share the same arrow-rs release. |
| Performance regression on Pi | `arrow-rs` + `parquet` should be faster than Polars for simple write operations since there is no DataFrame overhead. Benchmark on Pi if concerned. |
| Test breakage from `CoreError::Polars` removal | Search all crates for `CoreError::Polars` match arms before renaming to `CoreError::Arrow`. |
| Nullable column handling bugs | Explicit `.is_null(i)` checks at every read site. Extract helper functions to prevent copy-paste errors. |

---

## Implementation Order

1. **Add `arrow` + `parquet` to Cargo.toml** (workspace + core)
2. **Update `core/src/error.rs`** (replace Polars variant with Arrow)
3. **Update imports in `core/src/storage/parquet.rs`** (remove `polars::prelude::*`, add arrow/parquet)
4. **Implement P-01: `write_parquet()`** (most critical -- this is the BUG-004 hot path)
5. **Implement P-02: `write_raw_parquet()`** (second hot path)
6. **Implement P-03: `append_to_parquet()`** (read path)
7. **Implement P-04: `append_to_raw_parquet()`** (read path, deprecated)
8. **Implement P-05: `query()`** (read path with filter)
9. **Implement P-06: `query_raw()`** (read path with filters)
10. **Extract helper functions** (`read_nullable_string`, `read_nullable_json`)
11. **Update tests** (only `test_raw_parquet_schema_has_5_columns` needs direct update)
12. **Remove `polars` from `core/Cargo.toml`**
13. **Run full test suite** and verify
