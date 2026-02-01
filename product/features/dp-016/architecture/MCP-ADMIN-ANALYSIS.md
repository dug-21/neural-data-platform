# MCP-Enabled Configuration Administration Analysis

## Executive Summary

This document analyzes the feasibility of full MCP-enabled configuration administration for NDP. The goal is to allow adding streams, modifying configuration, and managing the platform entirely through MCP tools.

**Key Findings:**

| Capability | Current State | Gap | Effort |
|------------|---------------|-----|--------|
| Config CRUD APIs | **Full support** exists in config-client | None | - |
| MCP Read Tools | 15 tools for data exploration | None | - |
| MCP Write Tools | **None exist** | Complete gap | Medium |
| Validation Hook | StreamConfig.validate() exists | No schema validation for silver_etl | Low |
| Sync Mechanism | SourceManager.update_sources_for_stream() exists | No SilverSubscriber hot-reload | Medium |
| Schema Generation | Not implemented | Complete gap | High |

---

## 1. Current MCP State

### 1.1 Existing MCP Tools (Read-Only)

The `ndp-mcp-server` exposes 15 tools, all **read-only**:

**File:** `core/ndp-mcp-server/src/mcp/handler.rs:117-387`

| Category | Tool | Purpose |
|----------|------|---------|
| Bronze Layer | `list_streams` | List all Bronze streams with metadata |
| | `describe_schema` | Get schema info (source/target/all modes) |
| | `validate_config` | Compare etcd config vs actual Parquet schema |
| | `sample_data` | Retrieve sample rows from Bronze stream |
| Silver Layer | `list_silver_tables` | List all Silver hypertables |
| | `describe_silver_table` | Get Silver table schema details |
| | `sample_silver_data` | Sample rows with time filtering |
| | `silver_stats` | Table statistics, row counts, DQ summary |
| Dictionary | `query_dictionary` | Search columns by name/description |
| | `describe_column` | Column details including lineage, DQ rules |
| | `trace_lineage` | Trace Silver column to Bronze source(s) |
| | `list_dq_rules` | List DQ rules for tables/columns |
| ETL Observability | `etl_status` | Current ETL status for streams |
| | `etl_history` | Historical ETL runs for trend analysis |
| | `data_freshness` | Data freshness across layers |

**No write/mutate tools exist today.**

### 1.2 ConfigStore Trait (Port)

The `ConfigStore` trait is read-only for the MCP server:

**File:** `core/ndp-mcp-server/src/etcd/mod.rs:123-174`

```rust
#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn list_streams(&self) -> McpResult<Vec<String>>;
    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig>;
    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>>;
    async fn validate(&self) -> McpResult<()>;
}
```

**Missing Methods:**
- `create_config()` / `save_config()`
- `update_config()`
- `delete_config()`

---

## 2. CRUD APIs in config-client

### 2.1 ConfigClient (Low-Level)

**File:** `config-client/src/client.rs`

Full CRUD support exists:

| Method | Line | Purpose |
|--------|------|---------|
| `get<T>()` | 29-42 | Read typed config value |
| `get_raw()` | 45-47 | Read raw JSON value |
| `set<T>()` | 50-57 | Write typed config value |
| `set_raw()` | 60-62 | Write raw JSON value |
| `delete()` | 65-70 | Delete a config key |
| `list()` | 73-90 | List keys under prefix |
| `get_prefix_raw()` | 95-125 | Get all key-values under prefix |
| `get_prefix_nested()` | 130-145 | Get as nested JSON object |
| `watch()` | 148-154 | Watch for changes on prefix |

### 2.2 StreamRegistry (High-Level)

**File:** `config-client/src/stream/registry.rs`

Full lifecycle support:

| Method | Line | Purpose |
|--------|------|---------|
| `list_streams()` | 62-84 | List all stream IDs |
| `load_stream()` | 30-59 | Load and cache stream config |
| `load_all_streams()` | 87-109 | Load all stream configs |
| `stream_exists()` | 112-119 | Check if stream exists |
| `save_stream()` | 122-141 | **Create/Update** stream config |
| `delete_stream()` | 144-158 | **Delete** stream config |
| `clear_cache()` | 161-165 | Clear local cache |

**Key Evidence: Save with Validation**

```rust
// config-client/src/stream/registry.rs:122-141
pub async fn save_stream(&self, config: &StreamConfig) -> Result<(), ConfigError> {
    debug!("Saving stream configuration: {}", config.stream_id);

    // Validate before saving
    config
        .validate()
        .map_err(|e| ConfigError::EnvError(format!("Invalid stream config: {}", e)))?;

    let key = format!("/{}/config", config.stream_id);
    self.client.set(&key, config).await?;

    // Update cache
    {
        let mut cache = self.cache.write().await;
        cache.insert(config.stream_id.clone(), config.clone());
    }

    info!("Saved stream configuration: {}", config.stream_id);
    Ok(())
}
```

---

## 3. Validation Hook Architecture

### 3.1 Current Validation Points

| Validation Point | File:Line | What It Validates |
|------------------|-----------|-------------------|
| YAML Parsing | `apps/air-quality-app/src/config_sync/service.rs:129` | Syntax via serde_yaml |
| Config Conversion | `service.rs:132` | Field types, source types |
| StreamConfig.validate() | `core/src/types/stream_config.rs:368-405` | Stream ID format, fields, sources |
| Config before etcd save | `config-client/src/stream/registry.rs:126-128` | Same StreamConfig.validate() |

### 3.2 StreamConfig Validation

**File:** `core/src/types/stream_config.rs:6-25`

```rust
#[derive(Debug, Error, PartialEq)]
pub enum StreamConfigError {
    #[error("Invalid stream ID: {0}")]
    InvalidStreamId(String),

    #[error("Invalid field name: {0}")]
    InvalidFieldName(String),

    #[error("Stream must have at least one field")]
    NoFields,

    #[error("Stream must have at least one source")]
    NoSources,

    #[error("Invalid field type for {field}: {reason}")]
    InvalidFieldType { field: String, reason: String },

    #[error("Invalid range for field {field}: {reason}")]
    InvalidRange { field: String, reason: String },
}
```

**Validation Rules:**
- Stream ID: kebab-case, 3-64 characters
- Field names: snake_case, 1-64 characters
- Must have at least one field
- Must have at least one source
- Field types must be valid (float, int, string, bool, json)

### 3.3 Validation Gaps

| Area | Current State | Gap |
|------|---------------|-----|
| Bronze config | Fully validated | None |
| Silver ETL config | **Not validated** | No schema for silver_etl section |
| Field mappings | **Partially validated** | Source paths not checked against Bronze |
| DQ rules | **Not validated** | Rule syntax not checked |
| Target table | **Not validated** | Table existence not checked |

**Where MCP validation would hook in:**

```
MCP Tool (create_stream / update_stream)
    |
    v
+---------------------------+
| 1. Validate StreamConfig  |  <-- Exists: core/src/types/stream_config.rs
+---------------------------+
    |
    v
+---------------------------+
| 2. Validate Silver ETL    |  <-- GAP: Not implemented
+---------------------------+
    |
    v
+---------------------------+
| 3. Cross-reference check  |  <-- GAP: Source paths vs actual fields
+---------------------------+
    |
    v
+---------------------------+
| 4. Write to etcd          |  <-- Exists: registry.save_stream()
+---------------------------+
```

---

## 4. Sync Mechanism: Config to Running Apps

### 4.1 etcd Watch Capability

**File:** `config-client/src/watch.rs:12-80`

The `WatchHandle` provides real-time change notification:

```rust
// config-client/src/watch.rs:12-75
pub struct WatchHandle {
    cancel_tx: mpsc::Sender<()>,
}

impl WatchHandle {
    pub(crate) async fn new<F>(
        client: Client,
        prefix: &str,
        callback: F,
    ) -> Result<Self, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
    {
        // Spawns tokio task that:
        // 1. Watches etcd prefix for changes
        // 2. On Put: calls callback(key, Some(value))
        // 3. On Delete: calls callback(key, None)
    }
}
```

### 4.2 SourceManager Hot-Reload

**File:** `apps/air-quality-app/src/coordinator/source_manager.rs:1067-1099`

```rust
pub async fn update_sources_for_stream(
    &mut self,
    stream_id: &str,
) -> Result<(), SourceManagerError> {
    info!("Updating sources for stream: {}", stream_id);

    // 1. Load new configuration from etcd
    let config = self.registry.load_stream(stream_id).await
        .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

    // 2. Stop existing sources for this stream
    let source_ids: Vec<String> = { /* find sources by stream_id */ };
    for source_id in source_ids {
        self.stop_source(&source_id).await?;
    }

    // 3. Start new sources with updated config
    self.start_sources_for_stream(&config).await?;

    info!("Sources updated for stream: {}", stream_id);
    Ok(())
}
```

**Bronze layer hot-reload EXISTS but is not wired to etcd watch.**

### 4.3 Sync Gaps

| Component | Hot-Reload Method | Gap |
|-----------|-------------------|-----|
| SourceManager (Bronze) | `update_sources_for_stream()` | Not wired to etcd watch |
| BronzeSubscriber | None | No hot-reload - uses static config |
| SilverSubscriber | None | **No hot-reload** - reads YAML on startup |
| Silver ETL | None | **Complete gap** - reads YAML, not etcd |

### 4.4 Proposed Sync Architecture

```
                         etcd
                           |
                           | watch("/streams/")
                           v
                 +-------------------+
                 |  Watch Dispatcher |
                 +-------------------+
                           |
         +-----------------+-----------------+
         |                 |                 |
         v                 v                 v
+----------------+ +----------------+ +----------------+
| SourceManager  | |   (future)     | |   (future)     |
| (Bronze)       | | SilverManager  | | AlertManager   |
+----------------+ +----------------+ +----------------+
         |                 |                 |
         v                 v                 v
+----------------+ +----------------+ +----------------+
| MqttSource     | | SilverETL      | | AlertTriggers  |
| HttpSource     | | Jobs           | | Notifications  |
+----------------+ +----------------+ +----------------+
```

---

## 5. Required Changes for Full MCP Config Administration

### 5.1 New MCP Tools Needed

| Tool | Purpose | Priority |
|------|---------|----------|
| `create_stream` | Create new stream config in etcd | P0 |
| `update_stream` | Update existing stream config | P0 |
| `delete_stream` | Delete stream config | P1 |
| `validate_stream_config` | Dry-run validation without saving | P0 |
| `reload_stream` | Trigger hot-reload for running apps | P1 |
| `create_silver_table` | Generate and execute Silver DDL | P2 |
| `list_pending_changes` | Show config changes not yet applied | P2 |

### 5.2 ConfigStore Trait Extensions

**File to modify:** `core/ndp-mcp-server/src/etcd/mod.rs`

```rust
#[async_trait]
pub trait ConfigStore: Send + Sync {
    // Existing (read-only)
    async fn list_streams(&self) -> McpResult<Vec<String>>;
    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig>;
    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>>;
    async fn validate(&self) -> McpResult<()>;

    // NEW: Write operations
    async fn save_config(&self, config: &StreamConfig) -> McpResult<()>;
    async fn delete_config(&self, stream_id: &str) -> McpResult<()>;

    // NEW: Validation (dry-run)
    async fn validate_config(&self, config: &StreamConfig) -> McpResult<ValidationResult>;
}
```

### 5.3 Enhanced Validation

**New file:** `core/src/validation/silver_etl.rs`

```rust
pub enum SilverEtlValidationError {
    MissingTargetTable(String),
    InvalidSourcePath { path: String, reason: String },
    InvalidDqRule { rule: String, reason: String },
    InvalidTimestampTransform(String),
    MissingIdentityFields,
}

pub fn validate_silver_etl(
    config: &SilverEtlConfig,
    bronze_schema: &BronzeSchemaInfo,
) -> Result<(), SilverEtlValidationError> {
    // 1. Check target_table format
    // 2. Validate source_path exists in bronze_schema
    // 3. Validate DQ rule syntax
    // 4. Check timestamp transform is known
    // 5. Verify identity fields exist
}
```

### 5.4 Hot-Reload Wiring

**Option A: Push Model (Recommended)**

```
MCP Tool (update_stream)
    |
    +---> 1. Validate config
    |
    +---> 2. Save to etcd
    |
    +---> 3. Publish to internal channel
           |
           +---> SourceManager.update_sources_for_stream()
           |
           +---> (future) SilverManager.update_etl_for_stream()
```

**Option B: Watch Model (Current capability, unused)**

```
etcd watch callback
    |
    +---> Identify changed stream
    |
    +---> Route to appropriate manager
```

### 5.5 Implementation Phases

| Phase | Scope | Effort | Dependencies |
|-------|-------|--------|--------------|
| **Phase 1** | Add write tools (create/update/delete) | 3 days | None |
| **Phase 2** | Enhanced validation (silver_etl) | 2 days | Phase 1 |
| **Phase 3** | Wire hot-reload to etcd watch | 3 days | Phase 1 |
| **Phase 4** | Silver DDL generation via MCP | 5 days | Phase 2 |
| **Phase 5** | End-to-end stream lifecycle | 2 days | Phase 3, 4 |

---

## 6. Security Considerations

### 6.1 Current State

No authentication or authorization on MCP tools.

### 6.2 Recommendations for Write Tools

| Concern | Mitigation |
|---------|------------|
| Unauthorized access | MCP server should require API key/token |
| Malicious config | Validation MUST run before save |
| Cascading failures | Dry-run validation tool before apply |
| Audit trail | Log all config changes with user/timestamp |
| Rollback | Store previous config version before overwrite |

---

## 7. Alternative Approaches

### 7.1 Option A: MCP as Primary Admin Interface (Recommended)

**Pros:**
- Single interface for all administration
- AI-assisted configuration (Claude can suggest, validate, apply)
- Consistent with MCP-first architecture

**Cons:**
- Requires trust in MCP client
- No web UI for human operators

### 7.2 Option B: Hybrid (MCP + CLI)

**Pros:**
- CLI for scripting/automation
- MCP for interactive AI-assisted admin

**Cons:**
- Two interfaces to maintain
- Potential for drift

### 7.3 Option C: Config API Gateway

**Pros:**
- Single API serves both MCP and CLI
- Rate limiting, auth, audit in one place

**Cons:**
- Additional service to deploy
- More infrastructure

---

## 8. Summary

**What Exists Today:**
- Full CRUD APIs in config-client (`save_stream()`, `delete_stream()`)
- Bronze hot-reload capability (`SourceManager.update_sources_for_stream()`)
- etcd watch infrastructure (`WatchHandle`)
- StreamConfig validation

**What's Missing:**
1. MCP write tools (create_stream, update_stream, delete_stream)
2. ConfigStore trait extension for writes
3. Silver ETL validation
4. Watch dispatcher wiring hot-reload to etcd changes
5. Silver layer hot-reload

**Recommended First Step:**
Implement `create_stream` MCP tool that wraps `StreamRegistry.save_stream()` with enhanced validation. This proves the pattern before adding update/delete.

---

## References

| Document | Location |
|----------|----------|
| ConfigClient source | `config-client/src/client.rs` |
| StreamRegistry source | `config-client/src/stream/registry.rs` |
| ConfigStore trait | `core/ndp-mcp-server/src/etcd/mod.rs` |
| MCP Handler | `core/ndp-mcp-server/src/mcp/handler.rs` |
| SourceManager | `apps/air-quality-app/src/coordinator/source_manager.rs` |
| WatchHandle | `config-client/src/watch.rs` |
| StreamConfig validation | `core/src/types/stream_config.rs` |
| Bronze config research | `product/features/dp-016/specification/BRONZE-CONFIG-RESEARCH.md` |

---

*Analysis created: 2026-02-01*
*Feature: dp-016 Configuration Architecture Review*
