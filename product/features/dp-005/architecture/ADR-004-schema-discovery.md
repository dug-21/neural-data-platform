# ADR-004: Dynamic Schema Discovery via Parquet Introspection

## Status

Accepted

## Date

2026-01-03

## Context

The dp-005 Bronze MCP Server needs to report the schema of Bronze layer data. There are two potential sources of schema information:

### Schema Sources

1. **Configuration (etcd)**
   - `entity_schemas[].attributes[]` defines expected target schema
   - `sources[].parser.field_mappings[]` defines transformations
   - Represents what we **expect** the data to look like

2. **Parquet Files (Bronze)**
   - Arrow schema embedded in Parquet file footer
   - Represents what the data **actually** looks like
   - Always matches reality

### The Bronze Schema Reality

Per [DP-004 ADR-001](../dp-004/architecture/ADR-001-bronze-raw-json-schema.md), Bronze stores a standard envelope:

```
timestamp    | source_id  | ndp_id    | context     | raw_payload
DateTime     | String     | String?   | JSON?       | JSON
```

The domain-specific fields (pm25, temperature, etc.) are **inside** `raw_payload` as JSON, not as separate Parquet columns.

### Requirements

| Requirement | Priority | Notes |
|-------------|----------|-------|
| Report actual Bronze structure | Must | Envelope columns |
| Extract raw_payload keys | Should | For source analysis |
| No hardcoded schemas | Must | Adapt to evolution |
| Handle missing files | Must | Graceful error |
| Performance | Should | Metadata-only reads |

## Decision

**Use dynamic Parquet introspection with no hardcoded schema expectations. The schema is always discovered from the actual data files.**

### Introspection Strategy

```rust
impl BronzeStorage for LocalParquetStorage {
    async fn schema(&self, stream_id: &str) -> McpResult<Schema> {
        // 1. Find latest partition
        let path = self.find_latest_partition(stream_id)?;

        // 2. Open Parquet file (metadata only)
        let file = File::open(&path)?;
        let reader = SerializedFileReader::new(file)?;

        // 3. Read Arrow schema from Parquet metadata
        let parquet_schema = reader.metadata().file_metadata().schema();
        let arrow_schema = parquet_to_arrow_schema(parquet_schema)?;

        Ok(arrow_schema)
    }
}
```

### Schema Response Structure

For `describe_schema` tool, the response includes multiple levels:

```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "source",
  "envelope_schema": {
    "columns": [
      {"name": "timestamp", "type": "INT64", "logical": "TIMESTAMP_MICROS"},
      {"name": "source_id", "type": "UTF8", "nullable": false},
      {"name": "ndp_id", "type": "UTF8", "nullable": true},
      {"name": "context", "type": "UTF8", "logical": "JSON", "nullable": true},
      {"name": "raw_payload", "type": "UTF8", "logical": "JSON", "nullable": false}
    ]
  },
  "raw_payload_structure": {
    "keys": ["base", "clouds", "cod", "coord", "dt", "id", "main", "name"],
    "nested": {
      "main": ["feels_like", "humidity", "pressure", "temp", "temp_max", "temp_min"],
      "wind": ["deg", "gust", "speed"],
      "coord": ["lat", "lon"]
    }
  },
  "file_analyzed": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

### raw_payload Analysis

To discover the structure inside `raw_payload`:

```rust
async fn analyze_raw_payload_structure(
    &self,
    stream_id: &str,
    sample_size: usize,
) -> McpResult<RawPayloadStructure> {
    // 1. Sample N rows from latest partition
    let rows = self.sample(stream_id, sample_size).await?;

    // 2. Extract raw_payload from each row
    let payloads: Vec<Value> = rows.iter()
        .filter_map(|row| row.get("raw_payload").cloned())
        .collect();

    // 3. Aggregate keys across all payloads
    let mut top_level_keys: HashSet<String> = HashSet::new();
    let mut nested_keys: HashMap<String, HashSet<String>> = HashMap::new();

    for payload in payloads {
        if let Value::Object(map) = payload {
            for (key, value) in map {
                top_level_keys.insert(key.clone());
                if let Value::Object(nested) = value {
                    let nested_set = nested_keys.entry(key).or_default();
                    for nested_key in nested.keys() {
                        nested_set.insert(nested_key.clone());
                    }
                }
            }
        }
    }

    Ok(RawPayloadStructure {
        keys: top_level_keys.into_iter().collect(),
        nested: nested_keys.into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect(),
    })
}
```

### Why No Hardcoded Expectations

Hardcoding schema expectations creates problems:

1. **Synchronization burden**: Code must match config must match data
2. **Version drift**: Old code, new data format = errors
3. **False confidence**: Code says "pm25 exists" but file doesn't have it

Instead, introspection provides:

1. **Always accurate**: Schema comes from the actual file
2. **Self-documenting**: Agents see real structure, not expectations
3. **Evolution-friendly**: New columns automatically discovered
4. **Debug-friendly**: "What does the data actually look like?"

### Schema Evolution Handling

When Bronze schema evolves (e.g., new column added):

| Scenario | Behavior |
|----------|----------|
| New column in Parquet | Automatically appears in schema response |
| Column removed | Automatically disappears from schema response |
| Type change | New type reported from file |
| New stream | Discovered when files appear |

No code changes required for schema evolution.

## Consequences

### Positive

1. **Always accurate**: Schema matches reality, not expectations
2. **Zero maintenance**: No schema definitions to keep in sync
3. **Evolution-proof**: New columns, types, streams auto-discovered
4. **Debug-friendly**: See exactly what's in the data
5. **Trust**: Agents can rely on schema responses

### Negative

1. **No data-free discovery**: Need at least one file to report schema
   - Mitigation: Return "no data available" error for empty streams

2. **Sample-based payload analysis**: May miss rare keys
   - Mitigation: Sample multiple rows, document limitation

3. **Inconsistent schemas**: Different files may have different schemas
   - Mitigation: Use latest file, document that Bronze schema is stable

4. **Performance**: File read required (metadata only, fast)
   - Mitigation: Parquet footer is small, can cache

### Metadata-Only Performance

Parquet stores schema in file footer. Reading schema does NOT require:
- Reading row data
- Decompressing column data
- Scanning the full file

Typical schema read: < 10ms for local file.

## Alternatives Considered

### Alternative 1: Schema from Configuration

**How it works**: Read `entity_schemas` from etcd, report as Bronze schema.

```rust
async fn schema(&self, stream_id: &str) -> McpResult<Schema> {
    let config = self.config_store.get_stream(stream_id).await?;
    // Convert entity_schemas to Arrow schema
}
```

**Rejected because**:
- Config defines **target** (Silver) schema, not Bronze
- Bronze has different structure (envelope + raw_payload)
- Mismatch would confuse agents
- Doesn't help with raw_payload analysis

### Alternative 2: Hardcoded Bronze Envelope

**How it works**: Return static schema for Bronze envelope.

```rust
fn bronze_schema() -> Schema {
    Schema::new(vec![
        Field::new("timestamp", DataType::Int64, false),
        Field::new("source_id", DataType::Utf8, false),
        // ... hardcoded columns
    ])
}
```

**Rejected because**:
- Becomes stale if Bronze schema evolves
- Doesn't help with raw_payload content
- Adds maintenance burden

### Alternative 3: Schema Registry (External)

**How it works**: Use Confluent Schema Registry or similar.

**Rejected because**:
- Overkill for single-source Bronze schema
- Additional infrastructure
- Parquet already embeds schema

### Alternative 4: Both Sources + Comparison

**How it works**: Return config schema AND introspected schema, highlight differences.

```json
{
  "configured_schema": { ... },
  "actual_schema": { ... },
  "differences": [ ... ]
}
```

**Partially accepted**: This is what `validate_config` tool does, but `describe_schema` should return actual data structure for clarity.

## Implementation Notes

### Dependencies

```toml
[dependencies]
parquet = "53"
arrow = { version = "53", features = ["json"] }
```

### Schema Conversion

```rust
use parquet::arrow::parquet_to_arrow_schema;
use parquet::file::reader::SerializedFileReader;

fn get_arrow_schema(path: &Path) -> Result<Schema> {
    let file = File::open(path)?;
    let reader = SerializedFileReader::new(file)?;
    let parquet_schema = reader.metadata().file_metadata().schema_descr();
    let arrow_schema = parquet_to_arrow_schema(parquet_schema, None)?;
    Ok(arrow_schema)
}
```

### Handling Empty Streams

```rust
async fn schema(&self, stream_id: &str) -> McpResult<Schema> {
    match self.find_latest_partition(stream_id) {
        Some(path) => {
            // Read schema from file
        }
        None => Err(McpError::NoData(format!(
            "Stream '{}' has no data files. Schema cannot be determined.",
            stream_id
        ))),
    }
}
```

### Caching Consideration

For frequently accessed schemas:

```rust
struct CachedSchema {
    schema: Schema,
    raw_payload_structure: RawPayloadStructure,
    cached_at: Instant,
    file_path: PathBuf,
}

// Cache key: stream_id + file_path
// Invalidate: When file_path changes (new partition)
```

## Related Decisions

- [ADR-002: Storage Abstraction](./ADR-002-storage-abstraction.md) - BronzeStorage trait
- [ADR-005: Response Format](./ADR-005-response-format.md) - How schema is returned
- [DP-004 ADR-001: Bronze Schema](../dp-004/architecture/ADR-001-bronze-raw-json-schema.md) - Bronze envelope design

## References

- [Apache Parquet Metadata](https://parquet.apache.org/docs/file-format/metadata/)
- [Arrow Schema](https://arrow.apache.org/docs/format/Columnar.html#schema-message)
- [parquet-rs Schema Reading](https://docs.rs/parquet/latest/parquet/file/reader/trait.FileReader.html)
