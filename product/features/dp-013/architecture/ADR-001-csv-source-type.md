# ADR-001: CSV as Source Type

## Status

Proposed

## Context

NDP needs to support batch/historical data ingestion from CSV files. Use cases include:

1. **Historical backfill** - Loading past data when first deploying NDP
2. **Batch imports** - Periodic data dumps from external systems
3. **Manual corrections** - Uploading corrected datasets
4. **Migration** - Moving data from other platforms

The question is: should CSV be treated as a special "loader" system, or as another source type within the existing architecture?

### Current Source Architecture

NDP follows the Domain Adapter pattern (Hexagonal Architecture) where:
- **Ports** are defined as traits (`Source`, `RawSource`, `Store`)
- **Adapters** implement these traits (`MqttSource`, `HttpPollingSource`)
- **Configuration** drives behavior (stream YAML -> SourceConfig -> adapter)

The `SourceType` enum currently supports:
```rust
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
}
```

### Design Tension

| Approach | Pros | Cons |
|----------|------|------|
| **CSV as source type** | Reuses existing patterns, config-driven, same Bronze format | "Batch" vs "streaming" semantics differ |
| **Separate loader system** | Clear separation of batch vs stream | New abstraction, duplicate config patterns |

## Decision

**Implement CSV as a source type (`SourceType::Csv`)**, extending the existing Domain Adapter pattern.

### Rationale

1. **Architectural consistency**: All timeseries data should flow through the same pipeline (Source -> Coordinator -> Bronze -> Silver). CSV is just another transport.

2. **Config-driven**: NDP's strength is config-driven behavior. A CSV stream config looks nearly identical to an HTTP stream config - only the `source.type` differs.

3. **Minimal new code**: CsvSourceAdapter implements `RawSource` trait, reusing:
   - Same `RawDataPoint` output format
   - Same Bronze Parquet storage
   - Same ETL pipeline to Silver
   - Same field_mappings in silver_etl config

4. **Precedent**: `FileWatch` source type already exists (though not implemented). CSV is a specific case of file-based ingestion.

### Implementation

Extend `SourceType` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    Csv,  // NEW
}
```

Create `CsvSourceAdapter` implementing `RawSource`:

```rust
pub struct CsvSourceAdapter {
    config: CsvConfig,
    stream_id: String,
}

#[async_trait]
impl RawSource for CsvSourceAdapter {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        // Read single row, return as RawDataPoint
        // (Batch operation uses fetch_raw_batch)
    }

    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>> {
        // Read entire CSV, convert each row to RawDataPoint
        // Preserve full row as raw_payload JSON
    }
}
```

### Stream Config Example

```yaml
stream_id: historical-aq
enabled: true
source:
  type: csv                              # NEW source type
  path: data/imports/historical_readings.csv
  timestamp_field: timestamp
  timestamp_format: iso8601

entity_schemas:
  - entity_type: air_quality
    fields:
      - name: pm25
        source_field: pm25
        data_type: float

silver_etl:                              # Same ETL config as any other stream
  enabled: true
  target_table: silver.air_quality_observations
  field_mappings:
    - source_path: raw_payload.pm25
      target_column: pm25
      type: double_precision
```

### Trigger Mechanism

Unlike MQTT/HTTP (continuous), CSV is triggered on-demand:

```bash
ndp stream ingest <stream_id>
```

The CLI:
1. Loads stream config
2. Validates source.type == csv
3. Instantiates CsvSourceAdapter
4. Calls fetch_raw_batch()
5. Sends to IngestionCoordinator
6. Data lands in Bronze

## Consequences

### Positive

- **No new abstractions**: Reuses Source/RawSource traits, RawDataPoint, Bronze storage
- **Unified pipeline**: CSV data gets same treatment as streaming data
- **Config-driven**: Stream YAML controls behavior, no code changes for new CSV streams
- **ETL compatibility**: Existing silver_etl field_mappings work unchanged
- **Testability**: Can test CsvSourceAdapter in isolation, same as other sources

### Negative

- **Semantic mismatch**: Source trait implies "streaming"; CSV is batch. Mitigated by:
  - fetch_raw_batch() handles full file
  - CLI trigger (not continuous polling)
- **Memory for large files**: Entire CSV loaded into memory. Mitigations:
  - Chunked reading (future enhancement)
  - Document size limits
  - Streaming iterator pattern

### Neutral

- **FileWatch overlap**: FileWatch could watch a directory for new CSVs. CSV source type handles single-file imports. Both can coexist.

## Alternatives Considered

### 1. Separate CsvLoader Binary

A standalone `ndp-csv-loader` binary that reads CSV and writes directly to Bronze/Silver.

**Rejected because:**
- Duplicates config parsing logic
- Bypasses IngestionCoordinator (loses centralized routing)
- Different testing pattern than core adapters

### 2. Database COPY Command

Use TimescaleDB's COPY command to load CSV directly to Silver.

**Rejected because:**
- Bypasses Bronze (loses raw data archive)
- No DQ rule evaluation
- Schema must match exactly (no field_mappings)

### 3. FileWatch with CSV Detection

Extend FileWatch to auto-detect CSV files in a watch directory.

**Rejected for Phase 1 because:**
- More complex (directory watching, file locking)
- CSV source type handles explicit imports better
- FileWatch can be added later for auto-import scenarios

## References

- [DP-004: Bronze Raw JSON Schema](../../dp-004/architecture/ADR-001-bronze-raw-json-schema.md) - RawDataPoint format
- [AIR-005: Channel Ownership](../../air-005/) - IngestionCoordinator pattern
- [DP-006: Silver ETL](../../dp-006/) - Field mappings and ETL pipeline
