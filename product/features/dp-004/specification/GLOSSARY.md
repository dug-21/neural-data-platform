# DP-004: Bronze Raw JSON Schema - Glossary

## Overview

This glossary defines key terms used in dp-004 documentation. Terms are organized by domain and include their context within the Neural Data Platform.

---

## Core Data Structures

### RawDataPoint

**Definition**: A Rust struct representing a single ingested message with its complete, unmodified JSON payload and platform metadata.

**Structure**:
```rust
pub struct RawDataPoint {
    pub timestamp: DateTime<Utc>,    // When NDP received the message
    pub source_id: String,           // Stream configuration identifier
    pub ndp_id: Option<String>,      // Stable platform-owned identifier
    pub context: Option<Value>,      // Config-derived metadata snapshot
    pub raw_payload: Value,          // Exact source payload as JSON
}
```

**Usage**: Primary data structure for Bronze layer storage. Replaces `TimeSeriesPoint` for raw data ingestion.

**Location**: `core/src/traits.rs`

---

### TimeSeriesPoint

**Definition**: The legacy Rust struct representing a single metric observation with a numeric value.

**Structure**:
```rust
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
    pub ndp_id: Option<String>,
    pub context: Option<Value>,
}
```

**Usage**: Will be deprecated for Bronze storage but may remain for Silver layer ETL output where typed, extracted metrics are needed.

**Status**: Deprecated for Bronze path in dp-004

**Location**: `core/src/traits.rs`

---

### raw_payload

**Definition**: A JSON value containing the exact, unmodified payload received from a data source.

**Characteristics**:
- **Sacred**: Must never be transformed, filtered, or modified
- **Type**: `serde_json::Value` (can represent any valid JSON)
- **Content**: Exactly what the source sent (numbers, strings, booleans, objects, arrays)
- **Purpose**: Enables replay, debugging, and future schema evolution

**Example**:
```json
{
  "pm02": 12.5,
  "rco2": 450,
  "atmp": 22.1,
  "serialno": "abc123",
  "firmware": "3.4.1"
}
```

**Contrast with**: Previous approach where parsers extracted only numeric `value` fields.

---

### source_id

**Definition**: A string identifier from stream configuration that identifies which data pipeline produced a `RawDataPoint`.

**Format**: Typically `{stream_name}-{source_type}`, e.g., `air-quality-Mqtt`, `weather-Http`

**Purpose**:
- Partition key for Parquet storage
- Traceability for debugging
- Routing decisions in pipeline

**Derived from**: Stream configuration file (`config/base/streams/*.yaml`)

---

### ndp_id

**Definition**: A stable, platform-owned identifier for a logical data source that persists across configuration changes.

**Characteristics**:
- Optional (nullable in storage)
- Human-readable (e.g., `airgradient-office-001`)
- Defined in stream configuration
- Does not change when device MAC or IP changes

**Purpose**: Provides stable identity for analytics and dashboards when underlying device identifiers change.

**Introduced in**: AIR-009

---

### context

**Definition**: A JSON object containing configuration-derived metadata captured at ingestion time.

**Characteristics**:
- Optional (nullable in storage)
- Snapshot of config metadata when message was ingested
- Immutable after ingestion (even if config changes later)

**Example**:
```json
{
  "room": "office",
  "floor": 2,
  "building": "HQ",
  "sensor_model": "AirGradient ONE"
}
```

**Purpose**: Enriches data with business context without modifying raw payload.

---

## Data Layer Architecture

### Bronze Layer

**Definition**: The first tier in the Lakehouse architecture, storing raw, immutable data exactly as received from sources.

**Characteristics**:
- Format: Apache Parquet files
- Schema: Wide (one row per message)
- Content: Raw JSON payloads
- Purpose: Archive, replay, audit, debugging

**Storage Path**: `{base_path}/data/{source_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet`

**Query Tool**: DuckDB (via Grafana plugin for ad-hoc queries)

---

### Silver Layer

**Definition**: The second tier in the Lakehouse architecture, storing cleansed, typed, and structured data extracted from Bronze.

**Characteristics**:
- Format: TimescaleDB hypertables
- Schema: Tall (one row per metric per timestamp)
- Content: Typed values (floats, strings, booleans)
- Purpose: Analytics, dashboards, continuous aggregates

**Transformation**: ETL process extracts metrics from `raw_payload` using JSON path expressions.

**Status**: Not part of dp-004 (covered by dp-003)

---

### Gold Layer

**Definition**: The third tier in the Lakehouse architecture, storing aggregated, feature-engineered data for ML and advanced analytics.

**Characteristics**:
- Format: Feature Store (TBD)
- Content: Pre-computed aggregations, rolling windows, derived features
- Purpose: ML training, real-time prediction

**Status**: Future scope (phase: ML)

---

### Silver Layer ETL

**Definition**: The Extract-Transform-Load process that converts Bronze raw JSON into Silver typed metrics.

**Process**:
1. **Extract**: Read Parquet files from Bronze
2. **Transform**: Parse `raw_payload` JSON, extract fields, convert types
3. **Load**: Write to TimescaleDB Silver tables

**Example SQL** (conceptual):
```sql
INSERT INTO silver.readings (timestamp, ndp_id, metric, value)
SELECT
    timestamp,
    ndp_id,
    'pm02',
    CAST(raw_payload->>'pm02' AS FLOAT)
FROM bronze.readings
WHERE source_id = 'air-quality-Mqtt';
```

**Responsibility**: Handles schema evolution - if source adds new field, only ETL changes, not ingestion.

---

## Storage Concepts

### Partition Key

**Definition**: The value used to organize Parquet files into directory structures for efficient querying.

**Current (dp-004)**: `source_id` (from RawDataPoint)

**Previous**: `stream_id` tag or `location_id` fallback

**Directory Structure**:
```
data/
  air-quality-Mqtt/
    year=2026/
      month=01/
        day=15/
          readings.parquet
  weather-Http/
    year=2026/
      month=01/
        day=15/
          readings.parquet
```

---

### Wide Format

**Definition**: A data layout where each row represents a complete message/event with all its fields.

**Example (RawDataPoint)**:
```
timestamp           | source_id        | ndp_id          | context              | raw_payload
2026-01-01 12:00:00 | air-quality-Mqtt | airgradient-001 | {"room":"office"}    | {"pm02":12.5,"rco2":450}
```

**Contrast with**: Tall format (one row per metric)

---

### Tall Format

**Definition**: A data layout where each row represents a single metric observation.

**Example (TimeSeriesPoint)**:
```
timestamp           | location_id | metric | value
2026-01-01 12:00:00 | sensor-001  | pm02   | 12.5
2026-01-01 12:00:00 | sensor-001  | rco2   | 450.0
```

**Usage**: Silver layer, analytics queries, time-series databases

---

### Schema-on-Read

**Definition**: An approach where data schema is applied at query time rather than at write time.

**Applied in Bronze**:
- Write: Store raw JSON without interpretation
- Read: Parse JSON fields as needed during queries

**Benefits**: Flexible, handles schema evolution, no data loss

**Trade-off**: Query-time parsing overhead (acceptable for Bronze use cases)

---

### Schema-on-Write

**Definition**: An approach where data schema is enforced at write time.

**Applied in Silver**:
- ETL validates and types data
- TimescaleDB enforces column types
- Invalid data rejected or handled explicitly

**Benefits**: Query performance, type safety, compression efficiency

---

## Traits and Interfaces

### RawStore Trait

**Definition**: A Rust trait defining the interface for storage backends that handle `RawDataPoint`.

**Methods**:
```rust
#[async_trait]
pub trait RawStore: Send + Sync {
    async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>;
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>;
}
```

**Implementors**: `ParquetStore`

---

### RawSource Trait

**Definition**: A Rust trait defining the interface for data sources that emit `RawDataPoint`.

**Methods**:
```rust
#[async_trait]
pub trait RawSource: Send + Sync {
    async fn fetch_raw(&self) -> CoreResult<Vec<RawDataPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

**Implementors**: `HttpPollingSource`, `GenericHttpPollingSource`

---

### ParseContext

**Definition**: A struct carrying metadata from stream configuration through the parsing pipeline.

**Fields**:
```rust
pub struct ParseContext {
    pub source_id: String,           // NEW in dp-004
    pub ndp_id: Option<String>,
    pub context: Option<Value>,
}
```

**Purpose**: Transfers configuration metadata to data points without coupling parsers to config format.

---

## Acronyms

| Acronym | Expansion | Definition |
|---------|-----------|------------|
| ADR | Architecture Decision Record | Document capturing an important architectural decision |
| ETL | Extract-Transform-Load | Process of moving data between layers with transformations |
| JSON | JavaScript Object Notation | Text-based data interchange format |
| NDP | Neural Data Platform | This project's platform name |
| WAL | Write-Ahead Log | Durability mechanism that logs writes before committing |

---

## References

- [ADR-001: Bronze Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
- [AIR-009: ndp_id Design](../../air-009/architecture/ADR-001-ndp-id-design.md)
- [Databricks Lakehouse Architecture](https://docs.databricks.com/lakehouse-architecture/index.html)
- [DuckDB JSON Functions](https://duckdb.org/docs/extensions/json.html)
