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
| [ADR-006](./ADR-006-deployment-strategy.md) | Docker Compose integration | Accepted | Pi resource constraints, stateless, minimal footprint |

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

### 6.1 Docker Compose Service Definition

The MCP server integrates into the existing NDP Docker Compose stack:

```yaml
ndp-mcp-server:
  build:
    context: ../..
    dockerfile: core/ndp-mcp-server/Dockerfile
  image: neural-data-platform/ndp-mcp-server:latest
  container_name: ndp-mcp-server
  ports:
    - "9100:9100"
  volumes:
    - air-quality-data:/data/raw:ro
  environment:
    - RUST_LOG=info
    - NDP_RAW_PATH=/data/raw
    - NDP_ETCD_ENDPOINTS=http://etcd:2379
    - NDP_MCP_LISTEN=0.0.0.0:9100
  depends_on:
    etcd:
      condition: service_healthy
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:9100/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 30s
  deploy:
    resources:
      limits:
        memory: 64M
  networks:
    - neural-network
```

**Key Configuration Notes:**
- **Read-only volume mount** (`ro`) - MCP server only reads Bronze data
- **etcd dependency** - Waits for etcd health check before starting
- **Memory limit** - 64MB hard cap for Pi resource constraints
- **Stateless** - No persistent state, safe to restart/replace

### 6.2 deploy.sh Integration

The MCP server integrates into the existing `deploy/pi/deploy.sh` script:

**Startup Sequence:**
```
1. etcd (config store) - must be healthy first
2. mosquitto (MQTT broker)
3. air-quality-app (ingestion)
4. timescaledb (Silver layer)
5. ndp-mcp-server (Bronze access) <- NEW
6. grafana (dashboards)
```

**Health Check Addition to status() Function:**
```bash
status() {
    echo "=== NDP Service Status ==="

    # ... existing service checks ...

    # MCP Server health
    echo -n "ndp-mcp-server: "
    if curl -sf http://localhost:9100/health > /dev/null 2>&1; then
        echo "healthy"
    else
        echo "unhealthy"
    fi
}
```

**No Special Init Scripts Needed:**
- MCP server is stateless (no database migrations)
- Configuration comes from environment variables
- Bronze data already exists from air-quality-app
- etcd configuration already synced by existing pipeline

### 6.3 Dockerfile Requirements

Multi-stage build optimized for ARM64 (Pi) deployment:

```dockerfile
# Stage 1: Build
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /build
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY core/ core/

# Build release binary
RUN cargo build --release --package ndp-mcp-server

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -u 1000 -s /sbin/nologin ndp
USER ndp

COPY --from=builder /build/target/release/ndp-mcp-server /usr/local/bin/

EXPOSE 9100
ENTRYPOINT ["/usr/local/bin/ndp-mcp-server"]
```

**Runtime Dependencies:**
- `libssl3` - TLS support for future HTTPS
- `ca-certificates` - Certificate validation
- `curl` - Health check command

**Build Targets:**
- Development: `x86_64-unknown-linux-gnu`
- Production (Pi): `aarch64-unknown-linux-gnu`

### 6.4 Resource Budget

Updated total memory allocation with MCP server:

| Service | Memory Limit | Purpose |
|---------|-------------|---------|
| mosquitto | 128 MB | MQTT broker |
| etcd | 256 MB | Configuration store |
| air-quality-app | 512 MB | Ingestion, Parquet writes |
| timescaledb | 256 MB | Silver layer storage |
| grafana | 256 MB | Dashboards |
| **ndp-mcp-server** | **64 MB** | **Bronze MCP access** |
| **TOTAL** | **1472 MB** | |

**Pi 5 Capacity:** 8GB RAM available, 1.5GB allocated (18% utilization)

**MCP Server Memory Profile:**
- Base process: ~10 MB
- Per-request overhead: ~1-5 MB (Parquet reads)
- Peak (large sample): ~30 MB
- 64 MB limit provides 2x headroom

### 6.5 Network Topology

All services communicate via Docker internal network:

```
+------------------------------------------------------------------+
|                     neural-network (bridge)                       |
|                                                                    |
|  +------------+     +------------+     +------------------+       |
|  | mosquitto  |     |    etcd    |     | air-quality-app  |       |
|  | :1883      |     | :2379      |     | (ingestion)      |       |
|  +------------+     +------------+     +------------------+       |
|        |                  |                    |                   |
|        |                  +--------------------+                   |
|        |                  |                                        |
|  +------------+     +------------------+     +----------------+   |
|  | timescaledb|     | ndp-mcp-server   |     |    grafana     |   |
|  | :5432      |     | :9100            |     | :3000          |   |
|  +------------+     +------------------+     +----------------+   |
+------------------------------------------------------------------+
                              |
                         Host Network
                              |
                    +------------------+
                    | External Access  |
                    | pi:9100 -> MCP   |
                    | pi:3000 -> Grafana|
                    +------------------+
```

**Service Discovery:**
- All services use Docker DNS (`etcd:2379`, `timescaledb:5432`)
- Host port mapping only for external access
- MCP server accessible from development Mac via `http://pi:9100/mcp`

### 6.6 Cloud Deployment (Future)

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
| [ADR-006-deployment-strategy.md](./ADR-006-deployment-strategy.md) | Deployment Strategy | Accepted |

---

## References

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [NDP Platform Architecture](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [Domain Adapter Pattern](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md#domain-adapter-pattern)
- [dp-005 SCOPE.md](../SCOPE.md)
