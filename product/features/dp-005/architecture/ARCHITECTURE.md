# dp-005: Bronze MCP Server - Architecture

**SPARC Phase**: Architecture (A)
**Status**: COMPLETE
**Date**: 2026-01-03

---

## 1. System Overview

The Bronze MCP Server exposes NDP Bronze layer data and configuration to development agents via the Model Context Protocol (MCP). It runs on the Raspberry Pi (edge) with cloud portability designed in from the start.

### Purpose

Enable AI development agents to:
1. **Discover streams** - What data exists in Bronze?
2. **Understand schemas** - What does the data look like?
3. **Validate configuration** - Does config match reality?
4. **Sample data** - What are actual values?

### High-Level Architecture

```
+-------------------------------------------------------------------------+
|                          MAC (Development)                               |
|                                                                          |
|   Claude Code --> MCP Client --> HTTP --> Pi MCP Server                 |
|                                                                          |
|   .claude/mcp.json:                                                      |
|   { "ndp-bronze": { "type": "http", "url": "http://pi:9100/mcp" } }    |
+-------------------------------------------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                          PI (Production) - ndp-mcp-server               |
|                                                                          |
|   +---------------------------------------------------------------+    |
|   |  HTTP Layer (axum)                                             |    |
|   |  POST /mcp --> JSON-RPC Router                                |    |
|   |  GET /health --> Health check                                 |    |
|   +---------------------------------------------------------------+    |
|                                     |                                    |
|   +---------------------------------------------------------------+    |
|   |  MCP Protocol Handler                                          |    |
|   |  tools/list --> Tool definitions                              |    |
|   |  tools/call --> Route to implementation                       |    |
|   +---------------------------------------------------------------+    |
|                                     |                                    |
|   +---------------------------------------------------------------+    |
|   |  Tool Implementations                                          |    |
|   |  list_streams ----> BronzeStorage.list()                      |    |
|   |  describe_schema -> BronzeStorage.schema()                    |    |
|   |  validate_config -> ConfigStore.get() + BronzeStorage.schema()|    |
|   |  sample_data -----> BronzeStorage.sample()                    |    |
|   +---------------------------------------------------------------+    |
|                          |                    |                          |
|   +----------------------+    +--------------------------------------+  |
|   |  ConfigStore (etcd)  |    |  BronzeStorage (trait)               |  |
|   |  - Read stream cfg   |    |  - LocalParquetStorage (today)       |  |
|   |  - Validate sync     |    |  - S3ParquetStorage (tomorrow)       |  |
|   +----------------------+    +--------------------------------------+  |
|                                                                          |
|   Data: /data/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet    |
+-------------------------------------------------------------------------+
```

---

## 2. Key Components

| Component | Responsibility | Technology |
|-----------|---------------|------------|
| **HTTP Server** | Accept connections, route requests | axum 0.7 |
| **MCP Handler** | JSON-RPC parsing, method dispatch | Custom |
| **Tool Registry** | Tool definitions, parameter validation | Custom |
| **BronzeStorage** | Parquet read, schema introspection | parquet/arrow 53 |
| **ConfigStore** | Stream config from etcd | etcd-client 0.14 |

---

## 3. Architectural Decisions Summary

| ADR | Decision | Status | Key Rationale |
|-----|----------|--------|---------------|
| [ADR-001](./ADR-001-mcp-transport.md) | HTTP POST transport | Accepted | Universal compatibility, cloud-ready, stateless |
| [ADR-002](./ADR-002-storage-abstraction.md) | BronzeStorage trait | Accepted | Storage-agnostic, testable, follows Domain Adapter pattern |
| [ADR-003](./ADR-003-config-source.md) | etcd as config source | Accepted | Validates sync pipeline, matches running apps |
| [ADR-004](./ADR-004-schema-discovery.md) | Parquet introspection | Accepted | Always accurate, zero maintenance, evolution-proof |
| [ADR-005](./ADR-005-response-format.md) | JSON with success flag | Accepted | MCP compliant, parseable, actionable errors |

---

## 4. Component Architecture

### 4.1 HTTP Layer (axum)

The server uses axum for HTTP handling with minimal middleware:

```
Routes:
  POST /mcp     -> mcp_handler (all MCP protocol messages)
  GET  /health  -> health_check (service status)
  GET  /metrics -> prometheus_metrics (future)
```

**Key Design Choices:**
- Single `/mcp` endpoint for all MCP methods (JSON-RPC routing)
- Stateless requests (no session state)
- CORS enabled for development flexibility
- Graceful shutdown with tokio CancellationToken

### 4.2 MCP Protocol Handler

Implements JSON-RPC 2.0 over HTTP per MCP specification:

```
Request Flow:
  POST /mcp
  {"jsonrpc":"2.0","method":"tools/list","id":1}
      |
      v
  mcp_handler() -> match method {
      "initialize"  -> protocol_info
      "tools/list"  -> tool_definitions
      "tools/call"  -> route_to_tool(params)
      _             -> error(-32601, "Method not found")
  }
      |
      v
  {"jsonrpc":"2.0","result":{...},"id":1}
```

**Supported Methods:**
- `initialize` - Protocol handshake
- `tools/list` - Return tool definitions with JSON Schema
- `tools/call` - Execute tool by name with parameters

### 4.3 Tool Implementations

Four MVP tools expose Bronze layer data:

| Tool | Input | Output | Data Source |
|------|-------|--------|-------------|
| `list_streams` | none | Stream metadata array | etcd + filesystem |
| `describe_schema` | stream_id, mode | Schema by mode | Parquet + etcd |
| `validate_config` | stream_id | Config vs data diff | etcd + Parquet |
| `sample_data` | stream_id, n | Row array | Parquet |

**describe_schema Modes:**
- `source` - Raw payload structure + field mappings (ETL development)
- `target` - Entity schemas from config (Silver target)
- `all` - Both + gap analysis (complete picture)

### 4.4 Storage Abstraction (BronzeStorage)

Follows the Domain Adapter pattern (hexagonal architecture):

```rust
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    async fn list(&self) -> McpResult<Vec<StreamStorageInfo>>;
    async fn schema(&self, stream_id: &str) -> McpResult<Schema>;
    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>>;
    async fn validate(&self) -> McpResult<()>;
}
```

**Implementations:**
- `LocalParquetStorage` - Filesystem access (current)
- `S3ParquetStorage` - Object storage (future)

**Partition Discovery:**
```
/data/raw/{stream_id}/
  year=2026/
    month=01/
      day=03/
        data.parquet  <- Latest partition found by walking tree
```

### 4.5 Config Client (etcd)

Reads stream configuration from etcd, not source YAML files:

```rust
#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn list_streams(&self) -> McpResult<Vec<String>>;
    async fn get_stream(&self, stream_id: &str) -> McpResult<StreamConfig>;
    async fn health_check(&self) -> McpResult<()>;
}
```

**etcd Key Structure:**
```
/streams/air-quality/stream_id        -> "air-quality"
/streams/air-quality/enabled          -> true
/streams/air-quality/entity_schemas/0/schema_name -> "airgradient"
```

**Startup Behavior:** Fail fast if etcd unavailable - no stale data tolerance.

---

## 5. Data Flow

### Typical Tool Call Sequence

```
Claude Code                          MCP Server                    Data Layer
    |                                    |                             |
    |  POST /mcp                         |                             |
    |  {"method":"tools/call",           |                             |
    |   "params":{"name":"describe_schema",                            |
    |             "arguments":{"stream_id":"outdoor-weather"}}}        |
    | ---------------------------------> |                             |
    |                                    |                             |
    |                                    |  ConfigStore.get_stream()   |
    |                                    | --------------------------> |
    |                                    | <-------------------------- |
    |                                    |  StreamConfig               |
    |                                    |                             |
    |                                    |  BronzeStorage.schema()     |
    |                                    | --------------------------> |
    |                                    |    (Parquet introspection)  |
    |                                    | <-------------------------- |
    |                                    |  Arrow Schema               |
    |                                    |                             |
    |                                    |  BronzeStorage.sample(5)    |
    |                                    | --------------------------> |
    |                                    |    (raw_payload analysis)   |
    |                                    | <-------------------------- |
    |                                    |  RawPayloadStructure        |
    |                                    |                             |
    |  200 OK                            |                             |
    |  {"content":[{"type":"text",       |                             |
    |    "text":"{\"success\":true,      |                             |
    |             \"stream_id\":\"outdoor-weather\",                   |
    |             \"mode\":\"all\",...}"}]}                            |
    | <--------------------------------- |                             |
```

### validate_config Flow

1. Load `entity_schemas` from etcd (target schema)
2. Sample Parquet rows and parse `raw_payload` JSON (source structure)
3. Compare field sets (config fields vs raw_payload keys)
4. Generate gap analysis (in_config_not_in_payload, in_payload_not_in_config)
5. Return structured diff with notes

---

## 6. Deployment Architecture

### Pi Deployment (Current)

```
Docker Compose Stack:
  ndp-mcp-server (port 9100)
    |
    +-- /data/raw (volume mount)
    +-- etcd:2379 (network)

Resource Budget:
  Memory: < 50MB
  CPU: Minimal (I/O bound)
```

### Cloud Deployment (Future)

| Aspect | Pi (Today) | Cloud (Tomorrow) |
|--------|------------|------------------|
| URL | `http://pi:9100/mcp` | `https://ndp-api.example.com/mcp` |
| TLS | Disabled | Let's Encrypt |
| Auth | None | Bearer token / OAuth2 |
| Storage | Local filesystem | S3 via object_store crate |
| Config | etcd (local) | etcd (managed) or env vars |

**No code changes required** - only environment configuration.

---

## 7. Cross-Cutting Concerns

### Error Handling

Error types map to MCP responses:

| Error Category | HTTP Status | Response Pattern |
|----------------|-------------|------------------|
| Tool success | 200 | `{"success": true, ...}` |
| Tool error | 200 | `{"success": false, "error": "...", "details": {...}}`, `isError: true` |
| Method not found | 200 | JSON-RPC error -32601 |
| Invalid params | 200 | JSON-RPC error -32602 |
| Server unavailable | 503 | Plain error response |

### Logging (tracing)

Structured logging with context:

```rust
info!(stream_id = %id, tool = "describe_schema", "Tool invoked");
warn!(error = %e, "etcd connection failed, retrying");
error!(path = %path, "Parquet file corrupted");
```

### Observability

- Health endpoint: `GET /health`
- Metrics (future): Prometheus at `GET /metrics`
- Request tracing: tower-http trace layer

---

## 8. Future Considerations

### Phase 2 Enhancements

| Enhancement | Description | Dependency |
|-------------|-------------|------------|
| S3 Backend | `S3ParquetStorage` implementation | object_store crate |
| Authentication | Bearer token / OAuth2 | HTTPS deployment |
| SSE Transport | Server-Sent Events for streaming | axum-extra |
| SQL Queries | Raw SQL against Bronze | DataFusion / DuckDB |
| Type Validation | Validate types in raw_payload | Phase 2 scope |

### Schema Evolution

The introspection approach handles schema evolution automatically:
- New columns appear in responses
- Removed columns disappear
- Type changes reflected immediately

---

## 9. Project Structure

```
/core/ndp-mcp-server/
  Cargo.toml
  src/
    main.rs                 # Entry, config loading, server start
    server.rs               # Axum routes, middleware
    mcp/
      mod.rs
      protocol.rs           # JSON-RPC types, MCP messages
      handler.rs            # Request routing (tools/list, tools/call)
      tools/
        mod.rs              # Tool registry
        list_streams.rs
        describe_schema.rs
        validate_config.rs
        sample_data.rs
    storage/
      mod.rs
      traits.rs             # BronzeStorage trait
      local.rs              # LocalParquetStorage
    config/
      mod.rs
      etcd.rs               # etcd client, config types
  tests/
    integration/
      mcp_protocol_test.rs
      tool_tests.rs
    fixtures/
      sample.parquet
```

---

## 10. ADR Index

| Document | Title | Status |
|----------|-------|--------|
| [ADR-001-mcp-transport.md](./ADR-001-mcp-transport.md) | MCP Transport Protocol Selection | Accepted |
| [ADR-002-storage-abstraction.md](./ADR-002-storage-abstraction.md) | BronzeStorage Trait Abstraction | Accepted |
| [ADR-003-config-source.md](./ADR-003-config-source.md) | etcd as Configuration Source | Accepted |
| [ADR-004-schema-discovery.md](./ADR-004-schema-discovery.md) | Dynamic Schema Discovery via Parquet Introspection | Accepted |
| [ADR-005-response-format.md](./ADR-005-response-format.md) | MCP Response Format | Accepted |

---

## References

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [NDP Platform Architecture](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [Domain Adapter Pattern](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md#domain-adapter-pattern)
- [dp-005 SCOPE.md](../SCOPE.md)
