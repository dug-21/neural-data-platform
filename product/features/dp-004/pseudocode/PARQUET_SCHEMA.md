# Pseudocode: New Parquet Schema

## Overview

Defines the new Bronze layer Parquet schema for storing raw JSON payloads. This replaces the current "tall" schema with a "wide" raw JSON schema.

## Related ADR

- [ADR-001: Bronze Layer Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)

---

## Schema Comparison

### Current Schema (7 columns, tall format)

```
timestamp | location_id | metric | value | ndp_id | context | [tags]
INT64     | UTF8        | UTF8   | FLOAT64 | UTF8 | UTF8    | ...
```

### New Schema (5 columns, wide format)

```
timestamp | source_id | ndp_id   | context  | raw_payload
INT64     | UTF8      | UTF8?    | UTF8?    | UTF8
```

---

## Arrow Schema Definition

```pseudocode
FUNCTION create_raw_data_schema() -> ArrowSchema:
    fields = [
        // Timestamp as microseconds since epoch (NOT NULL)
        Field::new(
            name: "timestamp",
            data_type: DataType::Timestamp(TimeUnit::Microsecond, Some("UTC")),
            nullable: false
        ),

        // Source identifier from config (NOT NULL)
        Field::new(
            name: "source_id",
            data_type: DataType::Utf8,
            nullable: false
        ),

        // Stable platform-owned identifier (NULLABLE)
        Field::new(
            name: "ndp_id",
            data_type: DataType::Utf8,
            nullable: true
        ),

        // Config-derived metadata as JSON string (NULLABLE)
        Field::new(
            name: "context",
            data_type: DataType::Utf8,
            nullable: true
        ),

        // Raw payload as JSON string (NOT NULL)
        Field::new(
            name: "raw_payload",
            data_type: DataType::Utf8,
            nullable: false
        ),
    ]

    RETURN ArrowSchema::new(fields)
END FUNCTION
```

---

## Record Batch Construction

```pseudocode
FUNCTION build_record_batch(points: Vec<RawDataPoint>) -> Result<RecordBatch>:
    IF points.is_empty():
        RETURN Error("Cannot build batch from empty points")
    END IF

    // Pre-allocate arrays
    count = points.len()

    // Create column builders
    timestamp_builder = TimestampMicrosecondBuilder::with_capacity(count)
    source_id_builder = StringBuilder::with_capacity(count)
    ndp_id_builder = StringBuilder::with_capacity(count)      // Nullable
    context_builder = StringBuilder::with_capacity(count)     // Nullable
    raw_payload_builder = StringBuilder::with_capacity(count)

    // Populate builders from points
    FOR point IN points:
        // Timestamp (required)
        timestamp_builder.append_value(point.timestamp.timestamp_micros())

        // Source ID (required)
        source_id_builder.append_value(point.source_id)

        // NDP ID (optional)
        IF point.ndp_id IS Some(value):
            ndp_id_builder.append_value(value)
        ELSE:
            ndp_id_builder.append_null()
        END IF

        // Context (optional, serialize to JSON string)
        IF point.context IS Some(json_value):
            context_builder.append_value(json_value.to_string())
        ELSE:
            context_builder.append_null()
        END IF

        // Raw payload (required, serialize to JSON string)
        raw_payload_builder.append_value(point.raw_payload.to_string())
    END FOR

    // Build arrays
    timestamp_array = timestamp_builder.finish()
    source_id_array = source_id_builder.finish()
    ndp_id_array = ndp_id_builder.finish()
    context_array = context_builder.finish()
    raw_payload_array = raw_payload_builder.finish()

    // Create schema
    schema = create_raw_data_schema()

    // Build record batch
    record_batch = TRY RecordBatch::try_new(
        schema,
        [
            Arc::new(timestamp_array),
            Arc::new(source_id_array),
            Arc::new(ndp_id_array),
            Arc::new(context_array),
            Arc::new(raw_payload_array),
        ]
    )

    RETURN Ok(record_batch)
END FUNCTION
```

---

## Polars DataFrame Construction

```pseudocode
FUNCTION build_dataframe(points: Vec<RawDataPoint>) -> Result<DataFrame>:
    IF points.is_empty():
        RETURN Error("Cannot build DataFrame from empty points")
    END IF

    // Extract columns as vectors
    timestamps: Vec<i64> = points.iter()
        .map(|p| p.timestamp.timestamp_micros())
        .collect()

    source_ids: Vec<String> = points.iter()
        .map(|p| p.source_id.clone())
        .collect()

    ndp_ids: Vec<Option<String>> = points.iter()
        .map(|p| p.ndp_id.clone())
        .collect()

    contexts: Vec<Option<String>> = points.iter()
        .map(|p| p.context.as_ref().map(|c| c.to_string()))
        .collect()

    raw_payloads: Vec<String> = points.iter()
        .map(|p| p.raw_payload.to_string())
        .collect()

    // Create Series
    timestamp_series = Series::new("timestamp", timestamps)
    source_id_series = Series::new("source_id", source_ids)
    ndp_id_series = Series::new("ndp_id", ndp_ids)
    context_series = Series::new("context", contexts)
    raw_payload_series = Series::new("raw_payload", raw_payloads)

    // Create DataFrame
    df = TRY DataFrame::new([
        timestamp_series,
        source_id_series,
        ndp_id_series,
        context_series,
        raw_payload_series,
    ])

    RETURN Ok(df)
END FUNCTION
```

---

## Write Flow

```pseudocode
FUNCTION RawParquetStore::write_raw_batch(
    self,
    points: Vec<RawDataPoint>
) -> Result<()>:

    IF points.is_empty():
        RETURN Ok(())
    END IF

    // Step 1: Validate all points
    FOR point IN points:
        TRY point.validate()
    END FOR

    // Step 2: Group points by partition key
    // Partition key = source_id + date
    grouped: HashMap<PathBuf, Vec<RawDataPoint>> = HashMap::new()

    FOR point IN points:
        partition_path = self.partition_path_raw(
            source_id: point.source_id,
            timestamp: point.timestamp
        )
        grouped.entry(partition_path)
            .or_insert_with(Vec::new)
            .push(point)
    END FOR

    // Step 3: Write each partition
    FOR (path, partition_points) IN grouped:
        TRY self.append_raw_to_parquet(partition_points, path)
    END FOR

    RETURN Ok(())
END FUNCTION

FUNCTION RawParquetStore::partition_path_raw(
    self,
    source_id: String,
    timestamp: DateTime<Utc>
) -> PathBuf:
    // Structure: /data/raw/{source_id}/year=YYYY/month=MM/day=DD/readings.parquet

    RETURN self.base_path
        .join("data")
        .join("raw")                          // New "raw" subdirectory
        .join(source_id)
        .join(format!("year={}", timestamp.year()))
        .join(format!("month={:02}", timestamp.month()))
        .join(format!("day={:02}", timestamp.day()))
        .join("readings.parquet")
END FUNCTION

FUNCTION RawParquetStore::append_raw_to_parquet(
    self,
    points: Vec<RawDataPoint>,
    path: PathBuf
) -> Result<()>:

    // Create parent directories if needed
    parent = path.parent()
    IF parent IS Some(dir):
        fs::create_dir_all(dir)?
    END IF

    // Load existing points if file exists
    all_points = points

    IF path.exists():
        existing_points = TRY self.read_raw_parquet(path)
        all_points = existing_points.extend(points)
    END IF

    // Build DataFrame
    df = TRY build_dataframe(all_points)

    // Write with Snappy compression
    file = TRY File::create(path)
    writer = ParquetWriter::new(file)
        .with_compression(ParquetCompression::Snappy)

    TRY writer.finish(df)

    RETURN Ok(())
END FUNCTION
```

---

## Read Flow

```pseudocode
FUNCTION RawParquetStore::read_raw_parquet(
    self,
    path: PathBuf
) -> Result<Vec<RawDataPoint>>:

    IF NOT path.exists():
        RETURN Ok(Vec::new())
    END IF

    // Read Parquet file
    file = TRY File::open(path)
    df = TRY ParquetReader::new(file).finish()

    // Extract columns
    timestamps = df.column("timestamp")?.i64()?
    source_ids = df.column("source_id")?.utf8()?
    ndp_ids = df.column("ndp_id").ok().and_then(|c| c.utf8().ok())
    contexts = df.column("context").ok().and_then(|c| c.utf8().ok())
    raw_payloads = df.column("raw_payload")?.utf8()?

    // Reconstruct points
    points = Vec::with_capacity(df.height())

    FOR i IN 0..df.height():
        ts_micros = timestamps.get(i) OR CONTINUE
        source_id = source_ids.get(i) OR CONTINUE
        raw_payload_str = raw_payloads.get(i) OR CONTINUE

        // Parse timestamp
        timestamp = DateTime::from_timestamp_micros(ts_micros)
            OR RETURN Error("Invalid timestamp")

        // Parse optional fields
        ndp_id = ndp_ids.and_then(|col| col.get(i).map(String::from))

        context = contexts.and_then(|col|
            col.get(i).and_then(|s| serde_json::from_str(s).ok())
        )

        raw_payload = TRY serde_json::from_str(raw_payload_str)

        point = RawDataPoint {
            timestamp: timestamp,
            source_id: source_id.to_string(),
            ndp_id: ndp_id,
            context: context,
            raw_payload: raw_payload,
        }

        points.push(point)
    END FOR

    RETURN Ok(points)
END FUNCTION
```

---

## Query Flow (DuckDB Compatible)

```pseudocode
// Example DuckDB query against new schema
FUNCTION query_raw_data_with_duckdb(
    parquet_path: String,
    source_id: String,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>
) -> SQL:

    RETURN """
    SELECT
        timestamp,
        source_id,
        ndp_id,
        context,
        raw_payload,
        -- Extract specific fields from raw_payload
        raw_payload->>'$.pm02' as pm25,
        raw_payload->>'$.rco2' as co2,
        raw_payload->>'$.atmp' as temperature
    FROM read_parquet('{parquet_path}/**/*.parquet')
    WHERE source_id = '{source_id}'
      AND timestamp >= '{start_time}'
      AND timestamp <= '{end_time}'
    ORDER BY timestamp DESC
    """
END FUNCTION
```

---

## Compression Settings

```pseudocode
CONST PARQUET_COMPRESSION = ParquetCompression::Snappy

// Rationale:
// - Snappy provides good compression/speed tradeoff
// - JSON column benefits from dictionary encoding in Parquet
// - Expected compression ratio: 3-5x for JSON data
```

---

## File Organization

```
/data/
├── raw/                           # New raw data directory
│   ├── air-quality-Http/
│   │   ├── year=2026/
│   │   │   ├── month=01/
│   │   │   │   ├── day=01/
│   │   │   │   │   └── readings.parquet
│   │   │   │   ├── day=02/
│   │   │   │   │   └── readings.parquet
│   ├── outdoor-weather-Http/
│   │   └── ...
│   └── outdoor-air-quality-Http/
│       └── ...
├── air-quality/                   # Legacy parsed data (deprecated)
│   └── ...
```

---

## Rust Implementation Signature

```rust
use crate::error::CoreResult;
use crate::traits::RawDataPoint;
use chrono::{DateTime, Datelike, Utc};
use polars::prelude::*;
use std::path::{Path, PathBuf};

impl ParquetStore {
    /// Build partition path for raw data
    fn partition_path_raw(&self, source_id: &str, timestamp: DateTime<Utc>) -> PathBuf {
        self.base_path
            .join("data")
            .join("raw")
            .join(source_id)
            .join(format!("year={}", timestamp.year()))
            .join(format!("month={:02}", timestamp.month()))
            .join(format!("day={:02}", timestamp.day()))
            .join("readings.parquet")
    }

    /// Write a batch of raw data points
    pub async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()> {
        // Implementation
        todo!()
    }

    /// Read raw data points from a parquet file
    fn read_raw_parquet(&self, path: &Path) -> CoreResult<Vec<RawDataPoint>> {
        // Implementation
        todo!()
    }
}
```

---

## File Location

**Target**: `core/src/storage/parquet.rs` (extend existing `ParquetStore`)

## Related Files

| File | Change |
|------|--------|
| `core/src/storage/parquet.rs` | Add raw data write/read methods |
| `core/src/storage/mod.rs` | Export new types |
| `core/src/traits.rs` | Add `RawDataPoint` struct |
