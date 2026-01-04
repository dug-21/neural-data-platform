# BUG-001: Detailed Refactoring Plan - MCP Server config-client Migration

**Created:** 2026-01-04
**Author:** ndp-architect
**Status:** Ready for Implementation

## Executive Summary

This plan details the refactoring of `ndp-mcp-server` to use the existing `config-client` crate instead of its duplicate etcd implementation. The migration removes approximately 700 lines of redundant code while improving consistency with the main application.

---

## Pattern Research Summary

### Retrieved Patterns Used

| Pattern | Relevance | Applied |
|---------|-----------|---------|
| `config-etcd-pattern` | Key structure `/streams/{stream-id}/config` | Yes |
| `arch-mcp-etcd-config` | Decision to use etcd, fail-fast behavior | Yes |
| `config-stream-files` | StreamConfig structure understanding | Yes |
| `arch-mcp-http-transport` | Server architecture context | Background |
| `dp-005-deployment-strategy` | Container/deployment context | Background |

---

## Current Architecture Analysis

### Duplication Identified

```
+-------------------+       +-------------------+
|   ndp-mcp-server  |       |    Main App       |
+-------------------+       +-------------------+
        |                           |
        v                           v
+-------------------+       +-------------------+
| EtcdConfigStore   |       | ConfigClient      |
| (custom, ~350 LOC)|       | (config-client)   |
+-------------------+       +-------------------+
        |                           |
        v                           v
    etcd-client                 etcd-client
```

**Problem:** Two independent etcd implementations with different:
- Key parsing strategies
- Type definitions
- Error handling

### Files to Modify

| File | Lines | Action | Reason |
|------|-------|--------|--------|
| `core/ndp-mcp-server/Cargo.toml` | 58 | MODIFY | Add config-client dependency |
| `core/ndp-mcp-server/src/etcd/mod.rs` | 506 | MODIFY | Replace with adapter using config-client |
| `core/ndp-mcp-server/src/etcd/client.rs` | 346 | DELETE | Custom implementation no longer needed |
| `core/ndp-mcp-server/src/server.rs` | 230 | MODIFY | Update AppState initialization |
| `core/ndp-mcp-server/src/config.rs` | ~60 | MODIFY | Add etcd endpoint handling |
| `core/ndp-mcp-server/src/mcp/tools/list_streams.rs` | 207 | MODIFY | Adjust to new types |
| `core/ndp-mcp-server/src/mcp/tools/describe_schema.rs` | 480 | MODIFY | Adjust to new types |
| `core/ndp-mcp-server/src/mcp/tools/validate_config.rs` | ~200 | MODIFY | Adjust to new types |
| `core/ndp-mcp-server/src/mcp/tools/sample_data.rs` | ~150 | MINOR | No ConfigStore dependency |

### Files to Delete

| File | Lines | Reason |
|------|-------|--------|
| `core/src/mcp/etcd_config_store.rs` | 504 | Replaced by config-client |

### Files to Keep (No Changes)

| File | Reason |
|------|--------|
| `core/ndp-mcp-server/src/storage/` | BronzeStorage unchanged |
| `core/ndp-mcp-server/src/mcp/protocol.rs` | MCP protocol unchanged |
| `core/ndp-mcp-server/src/mcp/handler.rs` | Generic handler unchanged |
| `core/ndp-mcp-server/src/error.rs` | Error types mostly unchanged |

---

## New Dependency Structure

### Before

```toml
# core/ndp-mcp-server/Cargo.toml
[dependencies]
etcd-client = "0.14"  # Direct dependency
```

### After

```toml
# core/ndp-mcp-server/Cargo.toml
[dependencies]
# etcd-client = "0.14"  # REMOVE - provided by config-client
config-client = { path = "../../config-client" }
neural_core = { path = "../", package = "platform-core" }
```

### Dependency Graph

```
ndp-mcp-server
    |
    +-- config-client
    |       |
    |       +-- neural_core (StreamConfig)
    |       +-- etcd-client
    |
    +-- neural_core (shared types)
```

---

## Code Changes

### 1. Cargo.toml Update

```toml
# core/ndp-mcp-server/Cargo.toml

[dependencies]
# Web framework (unchanged)
axum = { version = "0.7", features = ["macros", "json"] }
tower = { version = "0.4", features = ["util", "timeout", "limit"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Async runtime (unchanged)
tokio = { workspace = true }
tokio-util = { version = "0.7", features = ["rt"] }

# Serialization (unchanged)
serde = { workspace = true }
serde_json = { workspace = true }

# Error handling (unchanged)
thiserror = { workspace = true }

# Logging and tracing (unchanged)
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

# === CHANGE: Replace direct etcd-client with config-client ===
# etcd-client = "0.14"  # REMOVE
config-client = { path = "../../config-client" }
neural_core = { path = "../", package = "platform-core" }

# Parquet/Arrow for Bronze layer reading (unchanged)
parquet = "57"
arrow = "57"

# Utilities (unchanged)
chrono = { workspace = true }
uuid = { workspace = true }
async-trait = "0.1"
```

### 2. ConfigStore Adapter

Create a new adapter that wraps `config_client::StreamRegistry`:

```rust
// core/ndp-mcp-server/src/etcd/mod.rs (REPLACE contents)

//! etcd Configuration Store Module - config-client Adapter
//!
//! Wraps the shared `config-client` crate to provide the ConfigStore trait
//! for MCP server tools. This replaces the previous custom implementation.

use async_trait::async_trait;
use config_client::{ConfigError, StreamRegistry};
use neural_core::StreamConfig as CoreStreamConfig;
use std::sync::Arc;

use crate::error::{McpError, McpResult};

#[cfg(test)]
use mockall::automock;

// Re-export types that tools need
pub use config_client::StreamRegistry;

/// Configuration converted from neural_core::StreamConfig for MCP tools
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub enabled: bool,
    pub source_type: String,
    pub field_mappings: Vec<FieldMapping>,
    pub entity_schema: EntitySchema,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FieldMapping {
    pub source: String,
    pub target: Option<String>,
    pub field_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct EntitySchema {
    pub name: String,
    pub version: String,
    pub attributes: Vec<SchemaAttribute>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaAttribute {
    pub name: String,
    #[serde(rename = "type")]
    pub attr_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default)]
    pub required: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            stream_id: String::new(),
            enabled: false,
            source_type: String::new(),
            field_mappings: Vec::new(),
            entity_schema: EntitySchema::default(),
        }
    }
}

/// Convert from neural_core::StreamConfig to our MCP StreamConfig
impl From<CoreStreamConfig> for StreamConfig {
    fn from(core: CoreStreamConfig) -> Self {
        // Extract source type from first source
        let source_type = core.sources.first()
            .map(|s| format!("{:?}", s.source_type).to_lowercase())
            .unwrap_or_default();

        // Convert fields to field mappings (simplified)
        let field_mappings = core.fields.iter()
            .map(|f| FieldMapping {
                source: f.name.clone(),
                target: Some(f.name.clone()),
                field_type: Some(format!("{:?}", f.field_type).to_lowercase()),
            })
            .collect();

        // Build entity schema from stream metadata
        let entity_schema = EntitySchema {
            name: core.description.clone(),
            version: core.version.clone(),
            attributes: core.fields.iter()
                .map(|f| SchemaAttribute {
                    name: f.name.clone(),
                    attr_type: format!("{:?}", f.field_type).to_lowercase(),
                    unit: f.unit.clone(),
                    required: f.required,
                })
                .collect(),
        };

        Self {
            stream_id: core.stream_id,
            enabled: core.enabled,
            source_type,
            field_mappings,
            entity_schema,
        }
    }
}

/// Configuration store abstraction (Port).
#[cfg_attr(test, automock)]
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// List all configured stream IDs.
    async fn list_streams(&self) -> McpResult<Vec<String>>;

    /// Get configuration for a specific stream.
    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig>;

    /// Get only enabled streams.
    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>>;

    /// Validate that the configuration store is accessible.
    async fn validate(&self) -> McpResult<()>;
}

/// Adapter wrapping config-client's StreamRegistry
pub struct ConfigClientStore {
    registry: Arc<StreamRegistry>,
}

impl ConfigClientStore {
    /// Create a new config store connected to etcd.
    ///
    /// # Arguments
    ///
    /// * `endpoints` - etcd server endpoints
    pub async fn new(endpoints: Vec<String>) -> Result<Self, McpError> {
        let endpoint_refs: Vec<&str> = endpoints.iter().map(|s| s.as_str()).collect();

        let registry = StreamRegistry::new(&endpoint_refs)
            .await
            .map_err(|e| McpError::EtcdUnavailable(format!("Failed to connect: {}", e)))?;

        Ok(Self {
            registry: Arc::new(registry),
        })
    }
}

#[async_trait]
impl ConfigStore for ConfigClientStore {
    async fn list_streams(&self) -> McpResult<Vec<String>> {
        self.registry
            .list_streams()
            .await
            .map_err(|e| McpError::EtcdUnavailable(format!("List failed: {}", e)))
    }

    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig> {
        let core_config = self.registry
            .load_stream(stream_id)
            .await
            .map_err(|e| match e {
                ConfigError::NotFound(_) => McpError::StreamNotFound(stream_id.to_string()),
                _ => McpError::EtcdUnavailable(format!("Get config failed: {}", e)),
            })?;

        Ok(StreamConfig::from(core_config))
    }

    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>> {
        let stream_ids = self.list_streams().await?;
        let mut enabled = Vec::new();

        for stream_id in stream_ids {
            match self.get_config(&stream_id).await {
                Ok(config) if config.enabled => enabled.push(config),
                Ok(_) => {} // Disabled, skip
                Err(e) => {
                    tracing::warn!(stream_id = %stream_id, error = %e, "Failed to get stream config");
                }
            }
        }

        Ok(enabled)
    }

    async fn validate(&self) -> McpResult<()> {
        // Try to list streams as a health check
        self.list_streams().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert!(config.stream_id.is_empty());
        assert!(!config.enabled);
    }

    // Mock tests for ConfigStore trait remain unchanged
}
```

### 3. Server.rs AppState Changes

```rust
// core/ndp-mcp-server/src/server.rs

use crate::config::AppConfig;
use crate::etcd::{ConfigStore, ConfigClientStore};  // Changed import
use crate::mcp::{JsonRpcRequest, McpHandler};
use crate::storage::{BronzeStorage, LocalParquetStorage};

// ... (struct AppState unchanged)

impl AppState<LocalParquetStorage, ConfigClientStore> {  // Type changed
    /// Create new application state with real implementations.
    pub async fn new(config: AppConfig) -> Result<Self, McpError> {  // Now async!
        let storage = Arc::new(LocalParquetStorage::new(&config.raw_path));

        // Use config-client instead of custom EtcdConfigStore
        let config_store = Arc::new(
            ConfigClientStore::new(config.etcd_endpoints.clone())
                .await?  // Async connection
        );

        let handler = Arc::new(McpHandler::new(storage, config_store));

        Ok(Self { config, handler })
    }
}
```

### 4. Main.rs Initialization Update

```rust
// core/ndp-mcp-server/src/main.rs

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = AppConfig::from_env();

    info!(
        listen_addr = %config.listen_addr,
        raw_path = %config.raw_path,
        etcd_endpoints = ?config.etcd_endpoints,
        "Starting ndp-mcp-server"
    );

    // Create application state (now async)
    let state = Arc::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to initialize application state")
    );

    // Validate etcd connection
    state.handler.validate_config().await
        .expect("etcd health check failed - ensure etcd is running");

    info!("Connected to etcd, config store validated");

    // Create router and start server
    let router = create_router(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!("Listening on {}", config.listen_addr);

    axum::serve(listener, router).await?;

    Ok(())
}
```

### 5. Delete etcd/client.rs

The file `core/ndp-mcp-server/src/etcd/client.rs` (346 lines) should be deleted entirely as its functionality is now provided by `config-client`.

---

## Type Mapping

### neural_core::StreamConfig to MCP StreamConfig

| Core Type | MCP Type | Notes |
|-----------|----------|-------|
| `stream_id: String` | `stream_id: String` | Direct mapping |
| `enabled: bool` | `enabled: bool` | Direct mapping |
| `sources: Vec<SourceConfig>` | `source_type: String` | First source type only |
| `fields: Vec<SchemaField>` | `field_mappings: Vec<FieldMapping>` | Simplified |
| `description: String` | `entity_schema.name: String` | Mapped |
| `version: String` | `entity_schema.version: String` | Mapped |
| `fields: Vec<SchemaField>` | `entity_schema.attributes` | Converted |

---

## Testing Strategy

### Unit Tests

1. **Mock Tests** - ConfigStore trait mocking unchanged
2. **Type Conversion Tests** - New tests for `From<CoreStreamConfig>`

### Integration Tests

1. **etcd Connection** - Test `ConfigClientStore::new()` against etcd
2. **Stream Listing** - Test `list_streams()` returns expected IDs
3. **Config Retrieval** - Test `get_config()` returns valid StreamConfig

### Test Data

Use existing etcd test data synced by `deploy.sh sync`:
- `air-quality` stream
- `outdoor-weather` stream
- `nws-forecast` stream

---

## Migration Steps

### Phase 1: Dependency Addition (Minimal Risk)

1. Add `config-client` dependency to Cargo.toml
2. Add `neural_core` dependency to Cargo.toml
3. Run `cargo check` to verify compilation

### Phase 2: Adapter Implementation

1. Create new `ConfigClientStore` adapter
2. Implement `From<CoreStreamConfig>` conversion
3. Keep old `EtcdConfigStore` temporarily for comparison
4. Run tests to verify adapter works

### Phase 3: Wire Up New Implementation

1. Update `AppState::new()` to use `ConfigClientStore`
2. Make initialization async
3. Update main.rs for async state creation
4. Run all tests

### Phase 4: Cleanup

1. Delete `core/ndp-mcp-server/src/etcd/client.rs`
2. Delete `core/src/mcp/etcd_config_store.rs`
3. Remove old type definitions from etcd/mod.rs
4. Remove unused imports
5. Run final test suite

### Phase 5: Validation

1. Deploy to Pi environment
2. Test all 4 MCP tools manually
3. Verify Claude Desktop can use MCP server
4. Update deployment documentation

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Type conversion loses data | Medium | Medium | Thorough unit tests for conversion |
| Async initialization breaks startup | Low | High | Test in dev before Pi deploy |
| etcd connection pooling issues | Low | Medium | config-client already tested in main app |
| Test failures from type changes | High | Low | Update tests incrementally |

---

## Success Criteria

1. All existing MCP tools work: `list_streams`, `describe_schema`, `validate_config`, `sample_data`
2. All unit tests pass
3. All integration tests pass
4. `cargo clippy` passes with no warnings
5. Memory usage unchanged or reduced
6. Startup time unchanged or improved
7. No duplicate etcd implementations remain

---

## Estimated Effort

| Phase | Time | Complexity |
|-------|------|------------|
| Phase 1: Dependencies | 15 min | Low |
| Phase 2: Adapter | 2 hours | Medium |
| Phase 3: Wire Up | 1 hour | Medium |
| Phase 4: Cleanup | 30 min | Low |
| Phase 5: Validation | 1 hour | Low |
| **Total** | **~5 hours** | Medium |

---

## Appendix: Pattern Feedback

### Patterns That Helped

1. **config-etcd-pattern** - Confirmed key structure `/streams/{stream-id}/config`
2. **arch-mcp-etcd-config** - Validated fail-fast approach for etcd unavailability
3. **config-stream-files** - Understood StreamConfig structure for type conversion

### Patterns That Were Missing

1. **config-client-adapter-pattern** - No existing pattern for wrapping config-client in another crate
2. **mcp-server-initialization** - No pattern for async server state initialization

### Recommendation

After implementation, save a new pattern:
- **Name:** `config-client-adapter-pattern`
- **Domain:** architecture
- **Tags:** `dp-005`, `config-client`, `adapter`, `mcp-server`
- **Description:** How to wrap config-client's StreamRegistry for use in other crates that need ConfigStore trait behavior

---

## Related Documents

- [BUG-001-mcp-config-client-refactor.md](./BUG-001-mcp-config-client-refactor.md) - Original bug report
- [ADR-003-config-source.md](../architecture/ADR-003-config-source.md) - Decision to use etcd
- [config-client README](../../../../config-client/README.md) - config-client usage
