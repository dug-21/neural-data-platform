# MCP Architecture Patterns

**Feature**: dp-010 - Extend MCP to Silver Layer & Data Dictionary
**Status**: Architecture Analysis
**Created**: 2026-01-16
**Based On**: dp-005 Bronze MCP Implementation

---

## 1. Overview

This document captures the architectural patterns established in the dp-005 Bronze MCP implementation that **must be followed** when extending the MCP server to support Silver layer and Data Dictionary access. These patterns ensure consistency, testability, and maintainability across the NDP MCP server.

---

## 2. Core Architectural Patterns

### 2.1 Domain Adapter Pattern (Hexagonal Architecture)

The MCP server follows the NDP Domain Adapter pattern established in the platform architecture.

**Pattern Structure**:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        MCP Handler (Core)                            │
│                                                                      │
│   Routes requests to tools, independent of storage implementation    │
├─────────────────────────────────────────────────────────────────────┤
│                           PORTS (Traits)                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────┐  │
│  │  BronzeStorage  │  │   ConfigStore   │  │  SilverStorage (*)  │  │
│  │    (dp-005)     │  │    (dp-005)     │  │     (dp-010)        │  │
│  └────────┬────────┘  └────────┬────────┘  └──────────┬──────────┘  │
│           │                    │                       │             │
├───────────┼────────────────────┼───────────────────────┼─────────────┤
│           │          ADAPTERS (Implementations)        │             │
│  ┌────────▼────────┐  ┌────────▼────────┐  ┌──────────▼──────────┐  │
│  │LocalParquetStore│  │StreamRegistry   │  │TimescaleSilverStore│  │
│  │    (local fs)   │  │   Adapter       │  │  (PostgreSQL)       │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘

(*) New for dp-010
```

**Key Files**:
- Port definition: `/workspaces/neural-data-platform/core/ndp-mcp-server/src/storage/traits.rs`
- Adapter: `/workspaces/neural-data-platform/core/ndp-mcp-server/src/storage/local.rs`
- Config adapter: `/workspaces/neural-data-platform/core/ndp-mcp-server/src/etcd/registry_adapter.rs`

**Pattern Rules**:
1. **Traits define contracts** - All storage access goes through trait methods
2. **Adapters implement traits** - Different backends (local, S3, TimescaleDB) implement the same trait
3. **Dependency injection** - Handler receives `Arc<dyn Trait>` at construction
4. **Mock generation** - Use `#[cfg_attr(test, automock)]` for test mocks

**Example - Existing BronzeStorage trait**:

```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    async fn list_streams(&self) -> McpResult<Vec<StreamStorageInfo>>;
    async fn get_schema(&self, stream_id: &str) -> McpResult<ParquetSchemaInfo>;
    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>>;
    async fn latest_partition(&self, stream_id: &str) -> McpResult<Option<String>>;
}
```

**Pattern for dp-010 - SilverStorage trait**:

```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait SilverStorage: Send + Sync {
    async fn list_tables(&self) -> McpResult<Vec<SilverTableInfo>>;
    async fn describe_table(&self, name: &str) -> McpResult<SilverTableSchema>;
    async fn sample(&self, name: &str, limit: usize, filters: Option<Filters>) -> McpResult<Vec<Value>>;
    async fn stats(&self, name: &str) -> McpResult<TableStats>;
}
```

### 2.2 Tool Implementation Pattern

Each MCP tool follows a consistent implementation pattern.

**Tool Module Structure**:

```
src/mcp/tools/
├── mod.rs              # Re-exports all tools
├── list_streams.rs     # One file per tool
├── describe_schema.rs
├── sample_data.rs
└── validate_config.rs
```

**Tool File Structure**:

```rust
//! Tool documentation with response format examples

use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::BronzeStorage;

/// Input parameters struct
#[derive(Debug, Clone, Deserialize)]
pub struct ToolNameArgs {
    pub required_param: String,
    #[serde(default = "default_fn")]
    pub optional_param: String,
}

/// Response structs (mode-specific if needed)
#[derive(Debug, Clone, Serialize)]
pub struct ToolNameResponse {
    pub success: bool,
    // ... fields
}

/// Main execute function - generic over traits
pub async fn execute<S, C>(
    storage: &S,
    config_store: &C,
    args: Value,
) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
    C: ConfigStore + ?Sized,
{
    // 1. Parse and validate arguments
    let args: ToolNameArgs = serde_json::from_value(args)
        .map_err(|e| McpError::InvalidRequest(format!("Invalid arguments: {}", e)))?;

    // 2. Validate stream_id format (if applicable)
    validate_stream_id(&args.stream_id)?;

    // 3. Execute business logic
    let data = storage.some_method(&args.stream_id).await?;

    // 4. Build response
    let response = ToolNameResponse {
        success: true,
        // ... populate fields
    };

    // 5. Serialize and return
    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}
```

**Pattern Rules**:
1. **Separate args struct** - Input parameters in a Deserialize struct with defaults
2. **Generic over traits** - Functions accept `&S where S: Trait + ?Sized` for mock injection
3. **Validate early** - Check arguments before storage calls
4. **Response structs** - Typed Serialize structs for consistent output
5. **Single return point** - `McpToolResult::success(&response)`

### 2.3 Response Format Pattern

All tool responses follow ADR-005 response format.

**Success Response Structure**:

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":true,\"stream_id\":\"...\",\"data\":{...}}"
  }]
}
```

**Error Response Structure**:

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\":false,\"error\":\"Error message\",\"code\":\"ERROR_CODE\",\"details\":{...}}"
  }],
  "isError": true
}
```

**Response Builder (from protocol.rs)**:

```rust
impl McpToolResult {
    pub fn success<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            content: vec![McpContent::json(value)?],
            is_error: None,
        })
    }

    pub fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        let error_obj = serde_json::json!({
            "success": false,
            "error": message.into(),
            "code": code.into()
        });
        Self {
            content: vec![McpContent::text(error_obj.to_string())],
            is_error: Some(true),
        }
    }
}
```

**Pattern Rules**:
1. **Always include `success` field** - Boolean at top level of inner JSON
2. **Include context** - stream_id, table_name, mode in success responses
3. **Actionable errors** - Include suggestions and available options in error details
4. **MCP `isError` flag** - Set to `true` only for errors

---

## 3. Error Handling Pattern

### 3.1 Error Type Hierarchy

**Error enum (from error.rs)**:

```rust
#[derive(Error, Debug, Clone, PartialEq)]
pub enum McpError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("etcd unavailable: {0}")]
    EtcdUnavailable(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
```

### 3.2 Error Code Mapping

| Error Type | JSON-RPC Code | MCP Code | HTTP Status |
|------------|---------------|----------|-------------|
| Config | -32603 | CONFIG_ERROR | 200 |
| EtcdUnavailable | -32000 | ETCD_UNAVAILABLE | 200 |
| StorageError | -32001 | STORAGE_ERROR | 200 |
| StreamNotFound | -32002 | STREAM_NOT_FOUND | 200 |
| ParseError | -32700 | PARSE_ERROR | 200 |
| InvalidRequest | -32602 | INVALID_PARAMS | 200 |
| Internal | -32603 | INTERNAL_ERROR | 200 |

**Note**: All tool errors return HTTP 200 with error in JSON body (MCP spec).

### 3.3 New Error Types for dp-010

```rust
// Add to McpError enum:
#[error("Table not found: {0}")]
TableNotFound(String),

#[error("Database unavailable: {0}")]
DatabaseUnavailable(String),

#[error("Column not found: {0}.{1}")]
ColumnNotFound(String, String),  // (table, column)
```

### 3.4 Error Conversion Pattern

```rust
// In adapter implementations:
impl From<tokio_postgres::Error> for McpError {
    fn from(err: tokio_postgres::Error) -> Self {
        if err.to_string().contains("does not exist") {
            McpError::TableNotFound(err.to_string())
        } else {
            McpError::DatabaseUnavailable(err.to_string())
        }
    }
}
```

---

## 4. Handler Pattern

### 4.1 McpHandler Structure

```rust
pub struct McpHandler<S, C>
where
    S: BronzeStorage + Send + Sync,
    C: ConfigStore + Send + Sync,
{
    storage: Arc<S>,
    config_store: Arc<C>,
}
```

**Extended for dp-010**:

```rust
pub struct McpHandler<B, C, S, D>
where
    B: BronzeStorage + Send + Sync,
    C: ConfigStore + Send + Sync,
    S: SilverStorage + Send + Sync,
    D: DictionaryStore + Send + Sync,
{
    bronze_storage: Arc<B>,
    config_store: Arc<C>,
    silver_storage: Arc<S>,
    dictionary_store: Arc<D>,
}
```

### 4.2 Request Routing Pattern

```rust
pub async fn handle(&self, request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => self.handle_initialize(request.id).await,
        "tools/list" => self.handle_tools_list(request.id).await,
        "tools/call" => self.handle_tools_call(request.id, request.params).await,
        _ => JsonRpcResponse::error(
            request.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", request.method),
        ),
    }
}
```

### 4.3 Tool Dispatch Pattern

```rust
let result = match call_params.name.as_str() {
    // Bronze tools (existing)
    "list_streams" => self.execute_list_streams().await,
    "describe_schema" => self.execute_describe_schema(call_params.arguments).await,

    // Silver tools (dp-010)
    "list_silver_tables" => self.execute_list_silver_tables().await,
    "describe_silver_table" => self.execute_describe_silver_table(call_params.arguments).await,

    // Dictionary tools (dp-010)
    "query_dictionary" => self.execute_query_dictionary(call_params.arguments).await,

    _ => return JsonRpcResponse::error(...),
};
```

---

## 5. Tool Definition Pattern

### 5.1 ToolDefinition Structure

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
}

pub struct ToolInputSchema {
    pub schema_type: String,       // Always "object"
    pub properties: Value,         // JSON Schema properties
    pub required: Vec<String>,     // Required property names
    pub additional_properties: Option<bool>,  // Usually false
}
```

### 5.2 Tool Registration Pattern

Tools are registered in `handle_tools_list`:

```rust
async fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
    let tools = vec![
        // Existing Bronze tools
        ToolDefinition {
            name: "list_streams".to_string(),
            description: "List all available Bronze layer streams with metadata".to_string(),
            input_schema: ToolInputSchema::empty(),
        },

        // New Silver tools (dp-010)
        ToolDefinition {
            name: "list_silver_tables".to_string(),
            description: "List all Silver layer hypertables with metadata".to_string(),
            input_schema: ToolInputSchema::empty(),
        },
        ToolDefinition {
            name: "describe_silver_table".to_string(),
            description: "Get schema information for a Silver table including columns, types, and units".to_string(),
            input_schema: ToolInputSchema::with_properties(
                serde_json::json!({
                    "table_name": {
                        "type": "string",
                        "description": "The Silver table name (e.g., 'air_quality_observations')"
                    }
                }),
                vec!["table_name".to_string()],
            ),
        },
        // ... more tools
    ];

    let result = ToolsListResult { tools };
    // ...
}
```

### 5.3 Input Schema Patterns

**No Parameters**:
```rust
ToolInputSchema::empty()
```

**Required Parameter**:
```rust
ToolInputSchema::with_properties(
    serde_json::json!({
        "stream_id": {
            "type": "string",
            "description": "The stream identifier"
        }
    }),
    vec!["stream_id".to_string()],
)
```

**Required + Optional Parameters**:
```rust
ToolInputSchema::with_properties(
    serde_json::json!({
        "table_name": {
            "type": "string",
            "description": "The table name"
        },
        "n": {
            "type": "integer",
            "description": "Number of rows (default: 10)",
            "default": 10,
            "minimum": 1,
            "maximum": 100
        }
    }),
    vec!["table_name".to_string()],  // Only required params
)
```

**Enum Parameter**:
```rust
"mode": {
    "type": "string",
    "enum": ["all", "source", "target"],
    "description": "Schema view mode (default: all)",
    "default": "all"
}
```

---

## 6. Testing Pattern

### 6.1 London School TDD with mockall

**Trait mocking setup**:
```rust
#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    // ...
}
```

**Test structure**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;

    #[tokio::test]
    async fn test_tool_returns_expected_data() {
        // 1. Create mock
        let mut mock = MockBronzeStorage::new();

        // 2. Set expectations
        mock.expect_list_streams()
            .times(1)
            .returning(|| Ok(vec![
                StreamStorageInfo::new("stream-a"),
            ]));

        // 3. Execute
        let result = execute(&mock, &mock_config).await;

        // 4. Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_propagates_error() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_get_schema()
            .with(eq("unknown"))
            .returning(|id| Err(McpError::StreamNotFound(id.to_string())));

        let result = execute(&mock, &mock_config, args).await;

        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }
}
```

### 6.2 Workflow Tests

```rust
#[tokio::test]
async fn test_workflow_list_then_describe_then_sample() {
    let mut mock = MockBronzeStorage::new();
    let mut seq = mockall::Sequence::new();

    // Step 1: List
    mock.expect_list_streams()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|| Ok(vec![...]));

    // Step 2: Describe
    mock.expect_get_schema()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_| Ok(...));

    // Step 3: Sample
    mock.expect_sample()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(vec![...]));

    // Execute workflow in order
    mock.list_streams().await.unwrap();
    mock.get_schema("stream").await.unwrap();
    mock.sample("stream", 5).await.unwrap();
}
```

---

## 7. Configuration Pattern

### 7.1 Environment Variables

**Existing (dp-005)**:
```bash
NDP_MCP_LISTEN=0.0.0.0:9100
NDP_ETCD_ENDPOINTS=http://localhost:2379
NDP_RAW_PATH=/data/raw
```

**New (dp-010)**:
```bash
NDP_TIMESCALE_URL=postgresql://user:pass@localhost:5432/ndp
NDP_DICTIONARY_SCHEMA=data_dictionary
NDP_SILVER_SCHEMA=silver
```

### 7.2 Config Struct Pattern

```rust
#[derive(Debug, Clone)]
pub struct McpConfig {
    pub listen_addr: String,
    pub etcd_endpoints: Vec<String>,
    pub raw_path: PathBuf,
    // dp-010 additions:
    pub timescale_url: Option<String>,
    pub dictionary_schema: String,
    pub silver_schema: String,
}

impl McpConfig {
    pub fn from_env() -> McpResult<Self> {
        Ok(Self {
            listen_addr: env::var("NDP_MCP_LISTEN")
                .unwrap_or_else(|_| "0.0.0.0:9100".to_string()),
            // ... other fields
        })
    }
}
```

---

## 8. Recommended Implementation Order for dp-010

### Phase 1: Storage Traits

1. **Create SilverStorage trait** in `src/storage/silver_traits.rs`
   - Mirror BronzeStorage pattern
   - Include mockall support

2. **Create DictionaryStore trait** in `src/dictionary/traits.rs`
   - Mirror ConfigStore pattern

### Phase 2: Adapters

3. **Implement TimescaleSilverStorage** in `src/storage/timescale.rs`
   - Use tokio-postgres or sqlx
   - Connection pooling (bb8 or deadpool)

4. **Implement PostgresDictionaryStore** in `src/dictionary/postgres.rs`

### Phase 3: Tools

5. **Silver tools** in `src/mcp/tools/silver/`
   - list_silver_tables.rs
   - describe_silver_table.rs
   - sample_silver_data.rs
   - silver_stats.rs

6. **Dictionary tools** in `src/mcp/tools/dictionary/`
   - query_dictionary.rs
   - describe_column.rs
   - trace_lineage.rs
   - list_dq_rules.rs

### Phase 4: Integration

7. **Extend McpHandler** with new generic parameters
8. **Update handle_tools_list** with new tool definitions
9. **Add tool dispatch routes** in handle_tools_call

---

## 9. ADR References

| ADR | Status | Summary |
|-----|--------|---------|
| [ADR-001-mcp-transport](./ADR-001-mcp-transport.md) | Accepted | HTTP POST with axum |
| [ADR-002-storage-abstraction](./ADR-002-storage-abstraction.md) | Accepted | BronzeStorage trait pattern |
| [ADR-003-config-source](./ADR-003-config-source.md) | Accepted | etcd via config-client |
| [ADR-004-schema-discovery](./ADR-004-schema-discovery.md) | Accepted | Parquet introspection |
| [ADR-005-response-format](./ADR-005-response-format.md) | Accepted | JSON with success flag |
| [ADR-006-deployment-strategy](./ADR-006-deployment-strategy.md) | Accepted | Docker deployment |

---

## 10. Patterns Stored in AgentDB

The following patterns have been stored in AgentDB and can be retrieved with `get-pattern`:

| Pattern Name | Domain | Tags |
|--------------|--------|------|
| arch-mcp-http-transport | dp-005 | mcp, http, transport, axum |
| arch-bronze-storage-trait | dp-005 | mcp, storage, traits, parquet |
| arch-mcp-response-format | dp-005 | mcp, response, json, error-handling |
| mcp-tool-testing-pattern | dp-005 | testing, mcp, mockall, london-school-tdd |
| mcp-config-adapter-pattern | dp-005 | mcp, config-client, domain-adapter |

---

*Document created: 2026-01-16*
*Based on: dp-005 implementation analysis*
