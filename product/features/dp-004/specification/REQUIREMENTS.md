# DP-004: Bronze Raw JSON Schema - Requirements

## Overview

This document defines the functional and non-functional requirements for redesigning the Bronze layer to store raw JSON payloads instead of parsed, typed metrics. These requirements are derived from ADR-001 and the dp-004 SCOPE.

---

## Functional Requirements

### FR-1: New RawDataPoint Data Structure

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.1 | Create a new `RawDataPoint` struct in `core/src/traits.rs` | Must Have |
| FR-1.2 | `RawDataPoint` must contain `timestamp` field of type `DateTime<Utc>` | Must Have |
| FR-1.3 | `RawDataPoint` must contain `source_id` field of type `String` | Must Have |
| FR-1.4 | `RawDataPoint` must contain `ndp_id` field of type `Option<String>` | Must Have |
| FR-1.5 | `RawDataPoint` must contain `context` field of type `Option<Value>` (serde_json) | Must Have |
| FR-1.6 | `RawDataPoint` must contain `raw_payload` field of type `Value` (serde_json) | Must Have |
| FR-1.7 | `RawDataPoint` must derive `Debug`, `Clone`, `Serialize`, `Deserialize` | Must Have |

### FR-2: Parquet Storage Schema Update

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.1 | `ParquetStore` must write a 5-column schema: timestamp, source_id, ndp_id, context, raw_payload | Must Have |
| FR-2.2 | `timestamp` column must be stored as microsecond-precision i64 | Must Have |
| FR-2.3 | `source_id` column must be stored as UTF-8 String | Must Have |
| FR-2.4 | `ndp_id` column must be stored as nullable UTF-8 String | Must Have |
| FR-2.5 | `context` column must be stored as nullable UTF-8 String (JSON-serialized) | Must Have |
| FR-2.6 | `raw_payload` column must be stored as UTF-8 String (JSON-serialized) | Must Have |
| FR-2.7 | Parquet files must use Snappy compression | Should Have |
| FR-2.8 | Partition path must use `source_id` as the partition key | Must Have |

### FR-3: Store Trait Modifications

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.1 | Create new `RawStore` trait with `write_raw(point: RawDataPoint)` method | Must Have |
| FR-3.2 | Create new `RawStore` trait with `write_raw_batch(points: Vec<RawDataPoint>)` method | Must Have |
| FR-3.3 | Implement `RawStore` for `ParquetStore` | Must Have |
| ~~FR-3.4~~ | ~~Preserve existing `Store` trait for backward compatibility during migration~~ | Removed |

### FR-4: Source Trait Modifications

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.1 | Create new `RawSource` trait with `fetch_raw()` returning `CoreResult<Vec<RawDataPoint>>` | Must Have |
| FR-4.2 | Sources must emit `RawDataPoint` containing the complete, unmodified JSON payload | Must Have |
| FR-4.3 | Sources must populate `source_id` from stream configuration | Must Have |
| FR-4.4 | Sources must populate `ndp_id` from stream configuration | Must Have |
| FR-4.5 | Sources must populate `context` from stream configuration metadata | Must Have |
| FR-4.6 | Sources must set `timestamp` to the ingestion time (when NDP received the message) | Must Have |

### FR-5: Parser Role Simplification

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.1 | Parsers must no longer extract individual metric fields | Must Have |
| FR-5.2 | Parsers must preserve the raw JSON payload exactly as received | Must Have |
| FR-5.3 | Parsers may validate JSON structure but must not transform it | Should Have |
| FR-5.4 | Remove dependency on `TimeSeriesPoint` from parser output path | Must Have |

### FR-6: Pipeline Integration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.1 | IngestionCoordinator must route `RawDataPoint` to the storage layer | Must Have |
| FR-6.2 | Router must attach `stream_id` as `source_id` in `RawDataPoint` | Must Have |
| FR-6.3 | WAL (Write-Ahead Log) must serialize `RawDataPoint` for crash recovery | Must Have |

---

## Non-Functional Requirements

### NFR-1: Performance

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-1.1 | Raw payload storage must not degrade ingestion throughput by more than 10% | Must Have |
| NFR-1.2 | Parquet file sizes must remain within 2x of current sizes due to JSON overhead | Should Have |
| NFR-1.3 | Memory usage for buffering `RawDataPoint` must not exceed 256 MB under normal load | Should Have |

### NFR-2: Compatibility

> **Simplified**: Platform is <1 week old. No backward compatibility required.
> Existing data can be retired. Clean cutover to new schema.

| ID | Requirement | Priority |
|----|-------------|----------|
| ~~NFR-2.1~~ | ~~Existing Parquet files (old schema) must remain readable~~ | Removed |
| ~~NFR-2.2~~ | ~~New schema must coexist with old schema during migration period~~ | Removed |
| NFR-2.3 | DuckDB/Grafana queries must be able to parse `raw_payload` column | Must Have |

### NFR-3: Resource Constraints

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-3.1 | Solution must run on Raspberry Pi 5 (8GB RAM) | Must Have |
| NFR-3.2 | CPU overhead from JSON serialization must be acceptable on ARM64 | Must Have |

### NFR-4: Observability

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-4.1 | Log messages must indicate raw payload size in bytes | Should Have |
| NFR-4.2 | Health check must report raw storage write latency | Should Have |

---

## Patterns That Must Change

Based on pattern search and codebase analysis, the following existing patterns will be affected:

### Pattern: TimeSeriesPoint Struct

**Location**: `core/src/traits.rs`

**Current Definition**:
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

**Impact**: A new `RawDataPoint` struct will be created. `TimeSeriesPoint` will eventually be deprecated for Bronze storage but may remain for Silver layer ETL output.

**Migration Path**:
1. Add `RawDataPoint` alongside `TimeSeriesPoint`
2. Implement parallel write paths during transition
3. Deprecate `TimeSeriesPoint` in Bronze path after validation

---

### Pattern: Parquet Storage Schema

**Location**: `core/src/storage/parquet.rs`

**Current Schema** (7 columns):
- timestamp (i64)
- location_id (String)
- metric (String)
- value (f64)
- ndp_id (String, nullable)
- context (String, nullable)
- tags (not persisted, derived from metric)

**New Schema** (5 columns):
- timestamp (i64)
- source_id (String)
- ndp_id (String, nullable)
- context (String, nullable)
- raw_payload (String)

**Impact**: Complete rewrite of `write_parquet()` and `append_to_parquet()` methods. The `query()` method will need separate implementation for old vs new schema files.

**Key Changes**:
- Remove value extraction logic
- Remove metric column construction
- Add raw_payload serialization
- Update partition key from `location_id`/`stream_id` tag to `source_id` field

---

### Pattern: Source Trait Implementation

**Location**: `core/src/traits.rs`, `core/src/sources/*.rs`

**Current Interface**:
```rust
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;
    async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}
```

**Impact**: A new `RawSource` trait will be added. Sources will need to implement both traits during migration, eventually migrating to `RawSource` only.

**Affected Sources**:
- `HttpPollingSource` (`core/src/sources/http_poll.rs`)
- `GenericHttpPollingSource` (`core/src/sources/http_poll.rs`)
- `MergeSource` (`core/src/sources/merge.rs`)

---

### Pattern: ResponseParser / Parser Trait

**Location**: `core/src/sources/http_poll.rs`, `core/src/parsers/traits.rs`

**Current Interface**:
```rust
// ResponseParser (legacy, in http_poll.rs)
pub trait ResponseParser: Send + Sync + 'static {
    fn parse(&self, response_body: &str, location_id: &str, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &'static str;
}

// Parser (new, in parsers/traits.rs)
pub trait Parser: Send + Sync {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>;
    fn parse_with_context(&self, payload: &Value, timestamp: DateTime<Utc>, context: &ParseContext)
        -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &str;
    fn config(&self) -> &ParserConfig;
}
```

**Impact**: Parsers will be simplified dramatically. Instead of extracting metrics, they will:
1. Validate JSON structure (optional)
2. Pass through raw payload unchanged
3. Attach metadata (ndp_id, context) from ParseContext

**Affected Parsers**:
- `FlatJsonParser` (`core/src/parsers/flat_json.rs`)
- `ArrayIteratorParser` (`core/src/parsers/array_iterator.rs`)
- `ColumnOrientedParser` (`core/src/parsers/column_oriented.rs`)
- `JsonPathParser` (`core/src/parsers/json_path.rs`)
- `WeatherParser` / `AirPollutionParser` (`core/src/sources/parsers.rs`)

---

### Pattern: ParseContext

**Location**: `core/src/parsers/traits.rs`

**Current Definition**:
```rust
pub struct ParseContext {
    pub ndp_id: Option<String>,
    pub context: Option<Value>,
}
```

**Impact**: ParseContext will be enhanced to include `source_id`. This struct will become the primary carrier of metadata from sources to storage.

**New Fields**:
- `source_id: String` (required)

---

### Pattern: Channel Data Flow

**Location**: `apps/air-quality-app/src/pipeline/`

**Current Flow**:
```
Source::fetch() -> Vec<TimeSeriesPoint> -> mpsc::channel -> Router -> Store::write()
```

**New Flow**:
```
RawSource::fetch_raw() -> Vec<RawDataPoint> -> mpsc::channel -> Router -> RawStore::write_raw()
```

**Impact**: Channel type changes from `TimeSeriesPoint` to `RawDataPoint`. Router enrichment logic changes from tag injection to field population.

---

### Pattern: Partition Path Generation

**Location**: `core/src/storage/parquet.rs`

**Current Logic**:
```rust
fn get_partition_key(point: &TimeSeriesPoint) -> String {
    point.tags.get("stream_id").cloned()
        .unwrap_or_else(|| point.location_id.clone())
}
```

**New Logic**:
```rust
fn get_partition_key(point: &RawDataPoint) -> &str {
    &point.source_id  // Always use source_id directly
}
```

**Impact**: Simpler, more predictable partition key derivation. No fallback logic needed.

---

## Dependencies

| Dependency | Description | Status |
|------------|-------------|--------|
| AIR-009 | ndp_id and context injection | Completed |
| DP-002 | TimescaleDB schema design | In Progress |
| DP-003 | Silver layer ETL pipeline | Not Started |

---

## Out of Scope

The following items are explicitly excluded from dp-004:

1. **Silver Layer ETL Implementation**: Covered by dp-003
2. **Migration of Existing Parquet Files**: Schema version detection allows coexistence
3. **Grafana Dashboard Query Updates**: Deferred until Silver layer available
4. **Query Interface for RawDataPoint**: Read path remains for debug/audit only

---

## References

- [ADR-001: Bronze Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
- [dp-004 SCOPE](../SCOPE.md)
- [AIR-009: ndp_id Design](../../air-009/architecture/ADR-001-ndp-id-design.md)
