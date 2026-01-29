# dp-013: CSV Source Type & Dimension Tables - Architecture

## System Context

This feature extends NDP's configuration language to support two new data types:
1. **CSV as a source type** - For batch/historical timeseries data
2. **Dimension table configs** - For reference/lookup data

Both capabilities follow NDP's established Domain Adapter pattern (Hexagonal Architecture), requiring minimal new abstractions while reusing existing infrastructure.

### Where This Fits

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           NDP Data Platform                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         SOURCE ADAPTERS                               │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────────┐  │   │
│  │  │ MqttSource │  │HttpPolling │  │  Webhook   │  │  CsvSource     │  │   │
│  │  │  (AIR-001) │  │  Source    │  │  (Future)  │  │  (dp-013) NEW  │  │   │
│  │  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └───────┬────────┘  │   │
│  └────────┼───────────────┼───────────────┼─────────────────┼───────────┘   │
│           │               │               │                 │               │
│           └───────────────┴───────────────┴─────────────────┘               │
│                                   │                                          │
│                                   ▼                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                    IngestionCoordinator (AIR-005)                     │   │
│  │                    mpsc channel → routing → storage                   │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                   │                                          │
│                                   ▼                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         BRONZE LAYER                                  │   │
│  │             Parquet files (same format for all sources)               │   │
│  │              Partitioned by stream_id and date                        │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                   │                                          │
│                              ETL (dp-006)                                    │
│                                   │                                          │
│                                   ▼                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         SILVER LAYER                                  │   │
│  │                        TimescaleDB                                    │   │
│  │  ┌─────────────────────────────┐  ┌─────────────────────────────┐    │   │
│  │  │   silver.* observations    │  │   silver.entity_context     │    │   │
│  │  │   (from Bronze ETL)        │  │   (dp-013) NEW              │    │   │
│  │  └─────────────────────────────┘  └─────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                    DIMENSION LOADER (dp-013) NEW                      │   │
│  │             Bypasses Bronze, loads directly to Silver                 │   │
│  │             Triggered via CLI or deploy.sh sync                       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Flow Distinction

| Data Type | Source | Path | Trigger |
|-----------|--------|------|---------|
| **Timeseries (streams)** | MQTT, HTTP, CSV | Bronze -> Silver | Continuous/On-demand |
| **Dimensions** | CSV (future: API) | Direct to Silver | Deploy/CLI |

---

## Component Design

### Part 1: CsvSourceAdapter

The CSV source adapter implements the existing `RawSource` trait (DP-004), following the same pattern as `MqttSource` and `HttpPollingSource`.

```
                    ┌─────────────────────────────────────────────┐
                    │              CsvSourceAdapter                │
                    │                                             │
   StreamConfig     │  ┌─────────────┐    ┌─────────────────────┐ │
  (source.type:csv) │  │ CsvReader   │    │ TimestampParser     │ │
 ─────────────────> │  │ (csv crate) │───>│ (iso8601/epoch/fmt) │ │
                    │  └─────────────┘    └─────────────────────┘ │
                    │         │                    │               │
                    │         ▼                    ▼               │
                    │  ┌─────────────────────────────────────────┐ │
                    │  │           RowToRawDataPoint             │ │
                    │  │   - Preserves full CSV row as JSON      │ │    RawDataPoint
                    │  │   - Adds timestamp, ndp_id, context     │──────────────────>
                    │  │   - Same format as MQTT/HTTP sources    │ │   (to Bronze)
                    │  └─────────────────────────────────────────┘ │
                    │                                             │
                    └─────────────────────────────────────────────┘
```

#### Source Type Extension

Extend `SourceType` enum in `/core/src/types/stream_config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    Csv,  // NEW: dp-013
}
```

#### CsvConfig Structure

```rust
/// CSV source configuration (parsed from stream config YAML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvConfig {
    /// Path to CSV file (absolute or relative to config root)
    pub path: PathBuf,

    /// Column name containing timestamps
    pub timestamp_field: String,

    /// Timestamp format: "iso8601", "epoch_seconds", "epoch_millis", or strftime format
    #[serde(default = "default_timestamp_format")]
    pub timestamp_format: String,

    /// Field delimiter (default: comma)
    #[serde(default = "default_delimiter")]
    pub delimiter: char,

    /// File encoding (default: utf-8)
    #[serde(default = "default_encoding")]
    pub encoding: String,

    /// Error handling: "skip" or "abort"
    #[serde(default = "default_on_error")]
    pub on_error: ErrorHandling,
}
```

#### Integration with SourceManager

The `SourceManager` dispatches to `CsvSourceAdapter` when `source.type: csv`:

```rust
// In source_manager.rs spawn_source()
match config.source_type {
    SourceType::Mqtt => self.spawn_mqtt_source(source_id, config).await,
    SourceType::HttpPoll => self.spawn_http_poll_source(source_id, config).await,
    SourceType::Csv => self.spawn_csv_source(source_id, config).await,  // NEW
    // ...
}
```

---

### Part 2: DimensionLoader

Dimensions are **not** timeseries - they are reference data that enriches observations. They bypass Bronze and load directly to Silver.

```
┌────────────────────────────────────────────────────────────────────────┐
│                         DimensionLoader                                 │
│                                                                         │
│  config/base/dimensions/*.yaml      ┌───────────────────────────────┐  │
│  ─────────────────────────────>     │     DimensionConfig           │  │
│                                     │  - dimension_id               │  │
│                                     │  - target (table, pk)         │  │
│  config/dimensions/*.csv            │  - source (type, path)        │  │
│  ─────────────────────────────>     │  - schema (fields, types)     │  │
│                                     │  - load (strategy)            │  │
│                                     └───────────────┬───────────────┘  │
│                                                     │                   │
│                                                     ▼                   │
│                                     ┌───────────────────────────────┐  │
│                                     │      LoadStrategy             │  │
│                                     │  ┌─────────────────────────┐  │  │
│                                     │  │  truncate_and_load      │  │  │
│                                     │  │  DELETE * + INSERT      │  │  │
│                                     │  └─────────────────────────┘  │  │
│                                     │  ┌─────────────────────────┐  │  │
│                                     │  │  upsert                 │  │  │
│                                     │  │  ON CONFLICT UPDATE     │  │  │
│                                     │  └─────────────────────────┘  │  │
│                                     └───────────────┬───────────────┘  │
│                                                     │                   │
│                                                     ▼                   │
│                                     ┌───────────────────────────────┐  │
│                                     │     TimescaleDB (Silver)      │  │
│                                     │     silver.entity_context     │  │
│                                     │     silver.{other_dims}       │  │
│                                     └───────────────────────────────┘  │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

#### DimensionConfig Structure

New config type at `/core/src/types/dimension_config.rs`:

```rust
/// Dimension table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionConfig {
    /// Unique dimension identifier (kebab-case)
    pub dimension_id: String,

    /// Target table in Silver
    pub target: DimensionTarget,

    /// Data source (currently CSV only)
    pub source: DimensionSource,

    /// Schema definition
    pub schema: DimensionSchema,

    /// Load strategy
    #[serde(default)]
    pub load: LoadConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionTarget {
    /// Target table (e.g., "silver.entity_context")
    pub table: String,

    /// Primary key columns
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionSource {
    /// Source type (currently only "csv")
    #[serde(rename = "type")]
    pub source_type: String,

    /// Path to source file
    pub path: PathBuf,

    /// Delimiter (default: comma)
    #[serde(default)]
    pub delimiter: Option<char>,

    /// Encoding (default: utf-8)
    #[serde(default)]
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoadConfig {
    /// Load strategy: "truncate_and_load" or "upsert"
    #[serde(default = "default_strategy")]
    pub strategy: LoadStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadStrategy {
    #[default]
    TruncateAndLoad,
    Upsert,
}
```

#### File Organization

```
config/
├── base/
│   ├── streams/           # Existing stream configs
│   │   └── historical-aq/
│   │       └── config.yaml
│   └── dimensions/        # NEW: dimension configs
│       └── entity_context.yaml
└── dimensions/            # NEW: dimension CSV data
    └── entity_context.csv
```

---

## Integration Points

### 1. Configuration System (etcd)

Dimension configs follow the existing config hierarchy:

```
Priority 1: etcd (/dimensions/{id}/config)  - if hot reload needed
Priority 2: YAML files (config/base/dimensions/*.yaml)
```

For Phase 1, dimensions are managed as files only (no etcd watch). The `deploy.sh sync` command processes them alongside stream configs.

### 2. CLI Integration

New subcommands in the NDP CLI:

```
ndp dimension list                 # List configured dimensions
ndp dimension sync <id>            # Sync specific dimension
ndp dimension sync --all           # Sync all dimensions
ndp dimension sync <id> --dry-run  # Validate without loading
ndp stream ingest <stream_id>      # Trigger CSV stream ingest
```

### 3. Deploy Script

Extend `deploy.sh sync` to process dimensions:

```bash
# In deploy.sh sync
sync_streams()    # Existing
sync_dimensions() # NEW: Process config/base/dimensions/*.yaml
```

### 4. Silver Layer Schema

Auto-create dimension tables if they don't exist:

```sql
-- Generated from dimension_config.schema
CREATE TABLE IF NOT EXISTS silver.entity_context (
    ndp_id TEXT NOT NULL,
    category TEXT NOT NULL,
    friendly_name TEXT,
    location_path TEXT,
    correlates_with TEXT,
    orientation TEXT,
    PRIMARY KEY (ndp_id)
);
```

---

## Schema Evolution Strategy

### Streams (CSV Source Type)

CSV streams follow the existing schema evolution pattern:
- **Bronze**: Raw JSON blob preserves all columns (schema-agnostic)
- **Silver ETL**: `field_mappings` in `silver_etl` config control extraction
- **Adding fields**: Add mapping, re-run ETL on historical Bronze data
- **Removing fields**: Remove mapping, field stays in Bronze

### Dimensions

Dimension tables support two evolution patterns:

1. **Additive changes** (add nullable column):
   - Add field to `schema.fields` in YAML
   - Add column to CSV
   - Re-sync: table auto-alters, new values populated

2. **Breaking changes** (rename/remove column):
   - Update config and CSV
   - Drop and recreate table (truncate_and_load handles this)
   - Downstream views may need updates

---

## Error Handling

### CSV Source Errors

| Error | Behavior | Configurable |
|-------|----------|--------------|
| File not found | Abort with clear path | No |
| Parse error (malformed row) | Skip or abort | `on_error` |
| Type conversion failure | Skip or abort | `on_error` |
| Timestamp parse failure | Skip row, log warning | `on_error` |
| Empty file | No-op, log warning | No |

### Dimension Load Errors

| Error | Behavior |
|-------|----------|
| CSV parse error | Abort, rollback transaction |
| Missing required column | Abort before load |
| Type conversion failure | Abort, rollback |
| Table creation failure | Abort with DDL error |
| Primary key violation (upsert) | Update existing row |

---

## Testing Strategy

### Unit Tests

1. **CsvSourceAdapter**
   - Timestamp parsing (iso8601, epoch, custom format)
   - Row to RawDataPoint conversion
   - Error handling modes (skip vs abort)
   - Delimiter handling

2. **DimensionLoader**
   - Config validation
   - Schema to DDL generation
   - Load strategy execution

### Integration Tests

1. **CSV -> Bronze -> Silver Pipeline**
   - Create CSV stream config
   - Ingest CSV to Bronze
   - Run ETL to Silver
   - Verify data integrity

2. **Dimension Sync**
   - Create dimension config + CSV
   - Run sync (truncate_and_load)
   - Verify Silver table
   - Run sync again with updated CSV
   - Verify upsert behavior

3. **deploy.sh Integration**
   - Full sync with streams + dimensions
   - Dry-run validation

---

## Related ADRs

- [ADR-001: CSV Source Type](./ADR-001-csv-source-type.md) - CSV as a source adapter
- [ADR-002: Dimension Tables](./ADR-002-dimension-tables.md) - Dimension loading approach

## Related Features

- **DP-004**: Bronze raw JSON schema (CsvSource outputs same format)
- **DP-006**: Silver ETL (processes CSV-sourced Bronze data)
- **AIR-005**: Channel ownership (CsvSource sends to coordinator)
- **AIR-009**: ndp_id pattern (CSV rows can have ndp_id)
- **AIR-012**: Home Assistant (entity_context enriches events)
