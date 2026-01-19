# NDP Codebase Structure Analysis

**Date**: 2026-01-17
**Purpose**: Map the Neural Data Platform codebase to identify structure, data flows, and optimization targets.

---

## 1. Project Organization

### 1.1 Workspace Structure

The project is a Rust workspace with 6 active members:

```
/workspaces/neural-data-platform/
├── Cargo.toml                    # Workspace root
├── core/                         # Core library (platform-core)
│   ├── ndp-mcp-server/           # MCP server for Bronze/Silver exploration
│   └── src/                      # Core Rust library
├── config-client/                # etcd configuration client
├── domains/
│   └── air-quality/              # Domain-specific types
├── apps/
│   ├── air-quality-app/          # Main ingestion binary
│   └── silver-etl/               # Bronze->Silver ETL binary
└── config/
    └── base/streams/             # GitOps YAML configurations
```

### 1.2 Binary Artifacts

| Binary | Source | Purpose |
|--------|--------|---------|
| `air-quality-app` | `apps/air-quality-app/src/main.rs` | Main ingestion service (MQTT + HTTP polling) |
| `silver-etl` | `apps/silver-etl/src/main.rs` | ETL daemon (Bronze Parquet -> Silver TimescaleDB) |
| `ndp-mcp-server` | `core/ndp-mcp-server/src/main.rs` | MCP server for data exploration |

---

## 2. Core Library (`/core`)

### 2.1 Module Structure

```
core/src/
├── lib.rs                        # Re-exports all public API
├── error.rs                      # CoreError enum
├── traits.rs                     # Core trait definitions
├── types/
│   ├── mod.rs
│   ├── raw_data_point.rs         # Bronze layer record (5-column schema)
│   └── time_series_point.rs      # Legacy typed data point
├── coordinator/
│   ├── mod.rs
│   └── ingestion_coordinator.rs  # Manages sources and routing
├── sources/
│   ├── mod.rs
│   ├── mqtt.rs                   # MQTT source implementation
│   └── http_poll.rs              # HTTP polling source (~800 lines)
├── storage/
│   ├── mod.rs
│   └── parquet.rs                # ParquetStore for Bronze layer
├── parsers/
│   └── mod.rs                    # Response parsing (Parser trait)
└── config/
    └── mod.rs                    # Configuration types
```

### 2.2 Key Traits

Located in `/core/src/traits.rs`:

| Trait | Purpose | Key Methods |
|-------|---------|-------------|
| `Source` | Typed data fetching | `fetch() -> Vec<TimeSeriesPoint>`, `health_check()` |
| `RawSource` | Raw JSON ingestion | `subscribe() -> mpsc::Receiver<RawDataPoint>`, `connect()` |
| `Store` | Typed data storage | `write()`, `read()`, `query()` |
| `RawStore` | Raw Bronze storage | `write_raw()`, `read_raw()`, `query_raw()` |
| `Forecast` | ML predictions | `train()`, `predict()`, `evaluate()` |
| `ResponseParser` | JSON response parsing | `parse()` -> `TimeSeriesPoint` |
| `Parser` | Raw payload parsing | `parse()` -> `ParsedRecord` |

### 2.3 Error Handling

The `CoreError` enum (`/core/src/error.rs`) has 10 variants:

```rust
pub enum CoreError {
    Storage(String),
    Source(String),
    Forecast(String),
    Validation(String),
    Config(String),
    Io(std::io::Error),
    Polars(String),
    DatabaseError(String),
    PredictionError(String),
    Parser(String),
}
```

---

## 3. Data Flow Paths

### 3.1 Bronze Layer Ingestion (air-quality-app)

```
┌─────────────┐     ┌──────────────────┐     ┌───────────────────┐
│ MQTT Source │────>│ IngestionRouter  │────>│ RawStorageWriter  │
└─────────────┘     │ (stream routing) │     │ (batch + timeout) │
                    └──────────────────┘     └─────────┬─────────┘
┌─────────────┐              │                         │
│ HTTP Source │──────────────┘                         v
└─────────────┘                              ┌─────────────────┐
                                             │  ParquetStore   │
                                             │ (daily partitions)│
                                             └─────────────────┘
```

**Key async patterns**:
- `mpsc::channel<RawDataPoint>(1000)` - Backpressure via bounded channel
- `tokio::spawn` for background tasks
- `tokio::select!` for graceful shutdown
- `Arc<RwLock<SourceManager>>` for shared source state

### 3.2 Bronze -> Silver ETL (silver-etl)

```
┌─────────────────┐     ┌──────────────┐     ┌────────────────────┐
│  Bronze Parquet │────>│   DuckDB     │────>│   TimescaleDB      │
│  read_parquet() │     │  (in-memory) │     │ (postgres extension)│
└─────────────────┘     └──────────────┘     └────────────────────┘
                               │
                        ┌──────┴──────┐
                        │ Pre-transform│
                        │ (array explosion)
                        └─────────────┘
```

**ETL Components** (`/apps/silver-etl/src/`):
- `etl.rs` - EtlRunner with DuckDB connection
- `sql_gen.rs` - SQL generation from config
- `dq.rs` - Data quality SQL generation
- `pre_transform.rs` - Array explosion for NWS forecasts
- `daemon.rs` - Scheduled ETL execution
- `persistence.rs` - Run history in DuckDB

### 3.3 MCP Server (data exploration)

```
┌───────────────┐     ┌─────────────────┐     ┌──────────────────┐
│  MCP Client   │────>│  axum routes    │────>│ Tool Handlers    │
│ (Claude, etc) │     │  /call, /list   │     │ (15+ tools)      │
└───────────────┘     └─────────────────┘     └────────┬─────────┘
                                                       │
                      ┌────────────────────────────────┼────────────────────────────────┐
                      v                                v                                v
              ┌───────────────┐              ┌─────────────────┐              ┌─────────────────┐
              │ etcd Registry │              │ Bronze Parquet  │              │ Silver Timescale│
              │ (stream config)│             │ (local/S3)      │              │ (tokio-postgres)│
              └───────────────┘              └─────────────────┘              └─────────────────┘
```

**MCP Tools** (15 tools in `/core/ndp-mcp-server/src/mcp/tools/`):
- Bronze: `list_streams`, `describe_schema`, `validate_config`, `sample_data`
- Silver: `list_silver_tables`, `describe_silver_table`, `sample_silver_data`, `silver_stats`
- Dictionary: `query_dictionary`, `describe_column`, `trace_lineage`, `list_dq_rules`
- ETL: `etl_status`, `etl_history`, `data_freshness`

---

## 4. Configuration System

### 4.1 Configuration Hierarchy

```
Priority 1: Stream Registry (/streams/{id}/config in etcd)
Priority 2: Legacy etcd (/config/{app}/*)
Priority 3: YAML files (config/*.yaml)
Priority 4: Code defaults
```

### 4.2 Config Client (`/config-client`)

```rust
// Key types in config-client/src/
pub struct ConfigClient {
    client: etcd_client::Client,
    prefix: String,
}

pub struct StreamRegistry {
    client: ConfigClient,
}
```

**Methods**:
- `get<T>()`, `set<T>()`, `delete()` - Typed CRUD
- `list()`, `get_prefix_raw()`, `get_prefix_nested()` - Bulk operations
- `watch()` - Real-time change notifications
- `get_with_env()` - Environment variable override

### 4.3 Stream Configuration Schema

Example from `/config/base/streams/air-quality/config.yaml`:

```yaml
stream_id: "air-quality"
enabled: true
retention_days: 365
partitioning_strategy: "daily"

sources:
  - type: mqtt
    ndp_id: "aq_airgradient_1"
    broker_url: "mosquitto"
    topic_pattern: "airgradient/readings/+"
    parser:
      parser_type: flat_json

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  timestamp:
    source_field: timestamp
    transform: microseconds_to_timestamp
  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
```

---

## 5. Memory-Intensive Components

### 5.1 Identified Hotspots

| Component | Location | Memory Pattern | Notes |
|-----------|----------|----------------|-------|
| ParquetStore | `core/src/storage/parquet.rs` | Batch writes (100 rows default) | Uses Polars DataFrame |
| EtlRunner | `apps/silver-etl/src/etl.rs` | DuckDB in-memory | Loads all Parquet files |
| HttpPollingSource | `core/src/sources/http_poll.rs` | Response buffering | ~800 lines, retry logic |
| MqttSource | `core/src/sources/mqtt.rs` | Channel buffering | 1000 capacity default |
| RawStorageWriter | `apps/air-quality-app/` | Batch accumulation | 50 rows, 30s timeout |

### 5.2 Channel Buffer Sizes

```rust
// Storage pipeline
mpsc::channel::<RawDataPoint>(1000)

// Dead letter queue
mpsc::channel::<DeadLetterItem>(100)

// MQTT default
buffer_capacity: 1000

// Storage writer
batch_size: 50
batch_timeout: 30s
```

### 5.3 Dependencies with Memory Impact

From `Cargo.toml`:

| Crate | Version | Memory Concern |
|-------|---------|----------------|
| `polars` | 0.35 | DataFrame operations, lazy eval |
| `duckdb` | (via silver-etl) | In-memory database |
| `parquet` | 57 | Arrow arrays |
| `arrow` | 57 | Columnar buffers |
| `reqwest` | 0.12 | HTTP client with connection pool |
| `tokio-postgres` | 0.7 | Connection pool (bb8) |

---

## 6. Async/Concurrency Patterns

### 6.1 Runtime Configuration

```rust
// Workspace tokio features
tokio = {
    version = "1.40",
    features = ["rt-multi-thread", "macros", "sync", "time", "fs", "signal", "net", "io-util"]
}
```

### 6.2 Patterns Used

| Pattern | Location | Purpose |
|---------|----------|---------|
| `mpsc::channel` | Coordinator, Storage | Backpressure, decoupling |
| `Arc<RwLock<T>>` | SourceManager | Shared mutable state |
| `Arc<Mutex<T>>` | HttpPollingSource | Per-endpoint state |
| `tokio::spawn` | Background tasks | Non-blocking operations |
| `tokio::select!` | Main loop | Graceful shutdown |
| `CancellationToken` | (Planned) | Clean shutdown |
| `async_trait` | All traits | Async interface definitions |

### 6.3 Shutdown Handling

```rust
// Current pattern in air-quality-app/src/main.rs
let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

tokio::spawn(async move {
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    let _ = shutdown_tx.send(());
});

tokio::select! {
    result = axum::serve(listener, app) => { ... }
    _ = &mut shutdown_rx => {
        // Wait for background tasks
        if let Some(task) = coordinator_task {
            let _ = task.await;
        }
    }
}
```

---

## 7. Configuration Touchpoints for Enhancement

### 7.1 Hardcoded Values

| Location | Value | Purpose |
|----------|-------|---------|
| `main.rs:281` | `1000` | Storage channel capacity |
| `main.rs:289` | `50` | Batch size |
| `main.rs:290` | `30s` | Batch timeout |
| `main.rs:259` | `100` | Dead letter channel |
| `main.rs:308` | `1000` | Coordinator buffer |

### 7.2 Configuration via Environment

| Variable | Default | Used By |
|----------|---------|---------|
| `ETCD_ENDPOINT` | `http://localhost:2379` | All services |
| `STREAM_CONFIG_DIR` | `/workspaces/.../config/base/streams` | air-quality-app |
| `TIMESCALE_URL` | Required | silver-etl |
| `NDP_TIMESCALE_*` | Various | silver-etl (fallback) |
| `BRONZE_PATH` | `/data/raw` | MCP server |

### 7.3 Missing Configuration Options

- No memory limits configuration
- No per-stream buffer sizing
- No dynamic resource adjustment
- No runtime config reload (except etcd watch)

---

## 8. Module Dependencies

### 8.1 Internal Dependencies

```
air-quality-app
├── neural_core (platform-core)
└── config-client

silver-etl
├── neural_core (platform-core)
└── config-client

ndp-mcp-server
├── neural_core (platform-core)
└── config-client
```

### 8.2 Feature Flags

Currently no feature flags are used for conditional compilation. All dependencies are always compiled.

---

## 9. Optimization Targets Summary

### 9.1 Memory Optimization Opportunities

1. **Channel sizing**: Currently hardcoded, should be configurable per-stream
2. **Batch accumulation**: Fixed sizes regardless of memory pressure
3. **DuckDB memory**: No limit configuration for ETL
4. **Parquet reading**: Full file loads, no streaming
5. **Polars DataFrames**: Created for each write batch

### 9.2 Performance Optimization Opportunities

1. **HTTP polling**: Single-threaded per endpoint, no connection reuse
2. **ETL watermark**: Scans all partitions for each run
3. **MCP tools**: Synchronous Parquet reads
4. **Config loading**: Full reload on each request

### 9.3 Code Reduction Opportunities

1. **http_poll.rs**: ~800 lines, complex retry logic
2. **etl.rs**: ~1500 lines, SQL generation mixed with execution
3. **Duplicate types**: TimeSeriesPoint vs RawDataPoint
4. **Feature conditionals**: All features always compiled

---

## 10. File Reference Summary

| Path | Lines | Purpose |
|------|-------|---------|
| `/core/src/lib.rs` | ~150 | Core re-exports |
| `/core/src/traits.rs` | ~100 | Trait definitions |
| `/core/src/error.rs` | ~43 | Error types |
| `/core/src/storage/parquet.rs` | ~400 | Bronze storage |
| `/core/src/sources/http_poll.rs` | ~800 | HTTP polling |
| `/core/src/coordinator/ingestion_coordinator.rs` | ~300 | Source coordination |
| `/apps/silver-etl/src/etl.rs` | ~1500 | ETL execution |
| `/apps/silver-etl/src/sql_gen.rs` | ~500 | SQL generation |
| `/apps/air-quality-app/src/main.rs` | ~410 | Main binary |
| `/config-client/src/client.rs` | ~340 | etcd client |
| `/core/ndp-mcp-server/src/` | ~2500 | MCP server (15+ tools) |

---

*This analysis provides the foundation for identifying specific optimization targets. Further investigation should focus on profiling runtime memory usage and identifying hot paths.*
