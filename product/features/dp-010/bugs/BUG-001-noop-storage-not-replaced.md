# BUG-001: NoOp Storage Implementations Not Replaced with TimescaleDB Adapters

**Bug ID**: BUG-001
**Feature**: dp-010 (Silver MCP Server Extension)
**Severity**: Critical
**Status**: Open
**Discovered**: 2026-01-17
**Reporter**: Claude Code Testing

---

## Summary

The dp-010 feature was marked as "IMPLEMENTATION COMPLETE" but only the tool interfaces and NoOp stubs were implemented. The actual TimescaleDB storage adapters were never created, causing 11 of 15 MCP tools to return "not configured" errors in production.

---

## Symptoms

When calling Silver, Dictionary, or ETL MCP tools:

```json
{"code":"STORAGE_ERROR","error":"Storage error: Silver layer not configured","success":false}
{"code":"STORAGE_ERROR","error":"Storage error: Dictionary not configured","success":false}
{"code":"STORAGE_ERROR","error":"Storage error: ETL store not configured","success":false}
```

### Affected Tools (11 of 15)

| Category | Tool | Error |
|----------|------|-------|
| Silver | `list_silver_tables` | Silver layer not configured |
| Silver | `describe_silver_table` | Silver layer not configured |
| Silver | `sample_silver_data` | Silver layer not configured |
| Silver | `silver_stats` | Silver layer not configured |
| Dictionary | `query_dictionary` | Dictionary not configured |
| Dictionary | `describe_column` | Dictionary not configured |
| Dictionary | `trace_lineage` | Dictionary not configured |
| Dictionary | `list_dq_rules` | Dictionary not configured |
| ETL | `etl_status` | ETL store not configured |
| ETL | `etl_history` | ETL store not configured |
| ETL | `data_freshness` | ETL store not configured |

### Working Tools (4 of 15)

| Category | Tool | Status |
|----------|------|--------|
| Bronze | `list_streams` | Working |
| Bronze | `describe_schema` | Working |
| Bronze | `validate_config` | Working |
| Bronze | `sample_data` | Working |

---

## Root Cause Analysis

### What Was Completed

1. **Traits defined** - `SilverStorage`, `DictionaryStore`, `EtlRunStore` in `storage/traits.rs`
2. **Types defined** - 26 structs for request/response types in `storage/types.rs`
3. **Tool modules created** - 11 tool files in `mcp/tools/`
4. **Handler extended** - `McpHandler<B,C,S,D,E>` with 5 generic parameters
5. **NoOp stubs created** - `NoOpSilverStorage`, `NoOpDictionaryStore`, `NoOpEtlRunStore` in `server.rs`
6. **Unit tests passing** - 279 tests with mocked storage

### What Was NOT Completed

The **Completion Phase** items from STATUS.md were never implemented:

| Item | Status | Impact |
|------|--------|--------|
| `TimescaleSilverStorage` impl | NOT DONE | Silver tools return errors |
| `TimescaleDictionaryStore` impl | NOT DONE | Dictionary tools return errors |
| `TimescaleEtlRunStore` impl | NOT DONE | ETL tools return errors |
| Integration testing | NOT DONE | No verification against live DB |
| docker-compose updates | NOT DONE | No `NDP_TIMESCALE_URL` configured |

### Code Evidence

In `core/ndp-mcp-server/src/server.rs` (lines 250-265):

```rust
pub fn with_registry(config: AppConfig, registry: StreamRegistry) -> Self {
    let storage = Arc::new(LocalParquetStorage::new(&config.raw_path));  // Real
    let config_store = Arc::new(StreamRegistryAdapter::new(registry));    // Real
    let silver_storage = Arc::new(NoOpSilverStorage);     // ← STUB - always errors
    let dictionary_store = Arc::new(NoOpDictionaryStore); // ← STUB - always errors
    let etl_store = Arc::new(NoOpEtlRunStore);            // ← STUB - always errors
    ...
}
```

The `NoOp*` implementations always return `McpError::StorageError("... not configured")`.

---

## Required Fixes

### 1. Create TimescaleDB Adapter Implementations

#### 1.1 TimescaleSilverStorage

**File**: `core/ndp-mcp-server/src/storage/timescale_silver.rs`

```rust
pub struct TimescaleSilverStorage {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl TimescaleSilverStorage {
    pub async fn new(database_url: &str) -> Result<Self, McpError> {
        // Create bb8 connection pool with tokio-postgres
    }
}

#[async_trait]
impl SilverStorage for TimescaleSilverStorage {
    async fn list_tables(&self) -> McpResult<Vec<SilverTableInfo>> {
        // Query timescaledb_information.hypertables + data_dictionary.silver_tables
    }

    async fn describe_table(&self, name: &str) -> McpResult<SilverTableDescription> {
        // Query data_dictionary.silver_columns + information_schema
    }

    async fn sample(&self, name: &str, n: usize, filters: Option<SampleFilters>)
        -> McpResult<Vec<Value>> {
        // Dynamic query with parameterized filters
    }

    async fn get_stats(&self, name: &str) -> McpResult<SilverTableStats> {
        // Aggregate queries for counts, nulls, DQ flags
    }
}
```

#### 1.2 TimescaleDictionaryStore

**File**: `core/ndp-mcp-server/src/storage/timescale_dictionary.rs`

```rust
pub struct TimescaleDictionaryStore {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

#[async_trait]
impl DictionaryStore for TimescaleDictionaryStore {
    async fn search(&self, query: &str, layer: Option<String>)
        -> McpResult<Vec<DictionaryEntry>> {
        // Query data_dictionary.silver_columns with ILIKE
    }

    async fn describe_column(&self, table: &str, column: &str)
        -> McpResult<ColumnDescription> {
        // Query data_dictionary.silver_columns + lineage
    }

    async fn trace_lineage(&self, table: &str, column: &str)
        -> McpResult<LineageTrace> {
        // Query data_dictionary.silver_lineage
    }

    async fn list_dq_rules(&self, table: Option<String>, column: Option<String>)
        -> McpResult<Vec<DqRuleInfo>> {
        // Query data_dictionary.silver_dq_rules
    }
}
```

#### 1.3 TimescaleEtlRunStore

**File**: `core/ndp-mcp-server/src/storage/timescale_etl.rs`

```rust
pub struct TimescaleEtlRunStore {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

#[async_trait]
impl EtlRunStore for TimescaleEtlRunStore {
    async fn get_status(&self, stream_id: Option<String>)
        -> McpResult<Vec<EtlStreamStatus>> {
        // Query silver.etl_runs with latest per stream
    }

    async fn get_history(&self, stream_id: &str, limit: usize,
        since: Option<DateTime<Utc>>, status: Option<String>)
        -> McpResult<EtlHistoryResult> {
        // Query silver.etl_runs with filters
    }

    async fn get_freshness(&self, layer: Option<String>)
        -> McpResult<FreshnessReport> {
        // Query max timestamps from Bronze + Silver
    }
}
```

### 2. Add Dependencies

**File**: `core/ndp-mcp-server/Cargo.toml`

```toml
[dependencies]
tokio-postgres = { version = "0.7", features = ["with-chrono-0_4", "with-serde_json-1"] }
bb8 = "0.8"
bb8-postgres = "0.8"
```

### 3. Update Configuration

**File**: `core/ndp-mcp-server/src/config.rs`

```rust
pub struct AppConfig {
    // Existing
    pub listen_addr: String,
    pub etcd_endpoints: Vec<String>,
    pub raw_path: String,

    // NEW: TimescaleDB connection
    pub timescale_url: Option<String>,
    pub timescale_max_connections: u32,  // default: 5
    pub timescale_connect_timeout_secs: u64,  // default: 10
}
```

### 4. Update Server Initialization

**File**: `core/ndp-mcp-server/src/server.rs`

Add new constructor that uses real implementations:

```rust
impl AppState<LocalParquetStorage, StreamRegistryAdapter,
              TimescaleSilverStorage, TimescaleDictionaryStore, TimescaleEtlRunStore> {

    /// Create application state with full Silver layer support.
    pub async fn with_timescale(
        config: AppConfig,
        registry: StreamRegistry,
    ) -> Result<Self, McpError> {
        let storage = Arc::new(LocalParquetStorage::new(&config.raw_path));
        let config_store = Arc::new(StreamRegistryAdapter::new(registry));

        // Real TimescaleDB implementations
        let timescale_url = config.timescale_url
            .ok_or_else(|| McpError::ConfigError("NDP_TIMESCALE_URL required".into()))?;

        let silver_storage = Arc::new(
            TimescaleSilverStorage::new(&timescale_url).await?
        );
        let dictionary_store = Arc::new(
            TimescaleDictionaryStore::new(&timescale_url).await?
        );
        let etl_store = Arc::new(
            TimescaleEtlRunStore::new(&timescale_url).await?
        );

        let handler = Arc::new(McpHandler::new(
            storage, config_store, silver_storage, dictionary_store, etl_store
        ));

        Ok(Self { config, handler })
    }
}
```

### 5. Update main.rs

**File**: `core/ndp-mcp-server/src/main.rs`

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;
    let registry = config.create_stream_registry().await?;

    // Use full TimescaleDB support if configured
    let state = if config.timescale_url.is_some() {
        Arc::new(AppState::with_timescale(config.clone(), registry).await?)
    } else {
        // Fallback to Bronze-only (NoOp stubs)
        Arc::new(AppState::with_registry(config.clone(), registry))
    };

    // ... rest of server startup
}
```

### 6. Update Docker Compose

**File**: `deploy/pi/docker-compose.yml`

```yaml
services:
  ndp-mcp-server:
    environment:
      - NDP_MCP_LISTEN=0.0.0.0:9100
      - NDP_ETCD_ENDPOINTS=http://etcd:2379
      - NDP_RAW_PATH=/data/raw
      # NEW: TimescaleDB connection
      - NDP_TIMESCALE_URL=postgresql://ndp:${NDP_DB_PASSWORD}@timescaledb:5432/ndp
      - NDP_TIMESCALE_MAX_CONNECTIONS=5
    depends_on:
      etcd:
        condition: service_healthy
      timescaledb:  # NEW dependency
        condition: service_healthy
```

### 7. Add Integration Tests

**File**: `core/ndp-mcp-server/tests/integration/timescale_storage_test.rs`

```rust
#[tokio::test]
#[ignore] // Requires running TimescaleDB
async fn test_silver_storage_list_tables() {
    let storage = TimescaleSilverStorage::new(&test_db_url()).await.unwrap();
    let tables = storage.list_tables().await.unwrap();
    assert!(tables.len() >= 4); // 4 Silver tables expected
}

#[tokio::test]
#[ignore]
async fn test_dictionary_search() {
    let store = TimescaleDictionaryStore::new(&test_db_url()).await.unwrap();
    let results = store.search("temperature", None).await.unwrap();
    assert!(!results.is_empty());
}
```

---

## Acceptance Criteria

| Criterion | Validation |
|-----------|------------|
| `list_silver_tables` returns tables | Returns 4+ Silver hypertables |
| `describe_silver_table` returns columns | Returns typed columns with units |
| `sample_silver_data` returns rows | Returns JSON rows from TimescaleDB |
| `query_dictionary` searches | Returns matching columns |
| `trace_lineage` works | Shows Bronze→Silver mapping |
| `etl_status` shows runs | Returns ETL run history |
| No "not configured" errors | All 15 tools functional |
| Integration tests pass | Tests against live TimescaleDB |

---

## Implementation Plan

### Phase 1: Core Adapters (Priority: Critical)

1. Add tokio-postgres + bb8 dependencies
2. Implement `TimescaleSilverStorage`
3. Implement `TimescaleDictionaryStore`
4. Implement `TimescaleEtlRunStore`
5. Add connection pool configuration

### Phase 2: Integration (Priority: High)

1. Update `AppConfig` with TimescaleDB settings
2. Add `AppState::with_timescale()` constructor
3. Update `main.rs` for conditional initialization
4. Update docker-compose.yml

### Phase 3: Verification (Priority: High)

1. Write integration tests
2. Test against Pi deployment
3. Verify all 15 MCP tools work via Claude Code

---

## Patterns to Follow

From AgentDB skill search:

| Pattern | Description |
|---------|-------------|
| `mcp-silver-storage-pattern` | Trait design with automock, tokio-postgres + bb8 pooling |
| `dp-011-hybrid-connection-pattern` | Pool config: max_size=2, min_idle=1, connection_timeout=5s |
| `arch-domain-adapter-pattern` | Traits as ports, implementations as adapters |
| `config-client-adapter-pattern` | Error mapping and type conversion |

---

## Related Documents

- [dp-010 STATUS.md](../STATUS.md) - Shows "Completion - PENDING" items
- [dp-010 SCOPE.md](../SCOPE.md) - Original feature scope
- [SILVER-TOOLS-SPEC.md](../specification/SILVER-TOOLS-SPEC.md) - SQL queries and response schemas
- [DICTIONARY-TOOLS-SPEC.md](../specification/DICTIONARY-TOOLS-SPEC.md) - Dictionary tool specs
- [ETL-STATUS-SPEC.md](../specification/ETL-STATUS-SPEC.md) - ETL tool specs

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-17 | Claude Code | Initial bug report |

---

*Bug documented: 2026-01-17*
