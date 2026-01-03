# ADR-001: MCP Transport Protocol Selection

## Status

Accepted

## Date

2026-01-03

## Context

The dp-005 Bronze MCP Server needs to communicate with MCP clients (primarily Claude Code on development machines). The Model Context Protocol (MCP) specification supports multiple transport mechanisms:

### Transport Options

1. **HTTP POST** - Standard request/response over HTTP
2. **Server-Sent Events (SSE)** - Unidirectional streaming from server
3. **stdio** - Standard input/output streams (local processes)
4. **WebSocket** - Bidirectional streaming

### Requirements

| Requirement | Priority | Notes |
|-------------|----------|-------|
| Cross-network communication | Must | Mac (dev) to Pi (edge) |
| Cloud portability | Must | Same server works on AWS/GCP |
| Request/response semantics | Must | Tool calls are synchronous |
| Streaming responses | Nice | Not needed for MVP tools |
| TLS support (future) | Should | Production security |
| Proxy traversal | Should | Corporate network compatibility |
| Connection pooling | Nice | Performance optimization |

### Constraint: Resource Budget

The MCP server runs on Raspberry Pi 5 with limited resources:
- Total memory: < 1GB available for server
- Target: < 50MB memory overhead
- CPU: Quad-core ARM Cortex-A76

## Decision

**Use HTTP POST as the primary transport for MVP, with axum as the HTTP framework.**

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Mac (Development)                                          │
│                                                              │
│   Claude Code ──► MCP Client                                │
│                      │                                       │
│                      │ HTTP POST (JSON-RPC)                 │
│                      ▼                                       │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │ Network (LAN or WAN)
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  Pi (Edge) - ndp-mcp-server                                 │
│                                                              │
│   axum HTTP Server                                          │
│   POST /mcp ──► JSON-RPC Handler ──► Tool Executor         │
│                                                              │
│   Routes:                                                    │
│   - POST /mcp           # MCP protocol endpoint             │
│   - GET  /health        # Health check                      │
│   - GET  /metrics       # Prometheus (future)               │
└─────────────────────────────────────────────────────────────┘
```

### Protocol Flow

```
Client                                    Server
  │                                          │
  │  POST /mcp                               │
  │  {"jsonrpc":"2.0","method":"tools/list"} │
  │ ─────────────────────────────────────────►
  │                                          │
  │  200 OK                                  │
  │  {"jsonrpc":"2.0","result":{...}}        │
  │ ◄─────────────────────────────────────────
  │                                          │
  │  POST /mcp                               │
  │  {"method":"tools/call","params":{...}}  │
  │ ─────────────────────────────────────────►
  │                                          │
  │  200 OK                                  │
  │  {"result":{"content":[...]}}            │
  │ ◄─────────────────────────────────────────
```

### MCP Endpoint Specification

```rust
// axum route definition
async fn mcp_handler(
    State(app_state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request).await,
        "tools/list" => handle_tools_list(request).await,
        "tools/call" => handle_tools_call(&app_state, request).await,
        _ => error_response(request.id, -32601, "Method not found"),
    }
}
```

### Key Design Choices

1. **Single endpoint `/mcp`**: All MCP methods go to one endpoint (JSON-RPC routing)
2. **Stateless requests**: No session state between requests (simplifies scaling)
3. **Synchronous responses**: Tool results returned in same HTTP response
4. **Standard HTTP semantics**: 200 for success, 4xx/5xx for errors

### Cloud Portability

The HTTP transport enables seamless cloud migration:

| Aspect | Pi (Today) | Cloud (Tomorrow) |
|--------|------------|------------------|
| URL | `http://pi:9100/mcp` | `https://ndp-api.example.com/mcp` |
| TLS | Disabled | Enabled (Let's Encrypt) |
| Auth | None | Bearer token / OAuth2 |
| Load Balancer | N/A | AWS ALB / GCP LB |
| DNS | mDNS / static IP | Route53 / Cloud DNS |

No code changes required - only environment configuration.

## Consequences

### Positive

1. **Universal compatibility**: HTTP works everywhere (proxies, firewalls, load balancers)
2. **Simple implementation**: axum provides excellent async HTTP handling
3. **Cloud-ready**: No transport changes needed for cloud deployment
4. **Debuggable**: Standard tools (curl, Postman) work out of the box
5. **TLS-ready**: HTTPS is trivial to add with rustls
6. **Stateless**: Easy to scale horizontally if needed
7. **Low memory**: No persistent connections to manage

### Negative

1. **No streaming**: Large responses must complete before sending
   - Mitigation: MVP tools return bounded result sets
   - Future: Add SSE endpoint if needed for streaming queries

2. **Connection overhead**: New TCP connection per request
   - Mitigation: HTTP/2 with connection pooling (axum supports this)
   - Impact: Minimal for tool call frequency (seconds between calls)

3. **No server push**: Server cannot notify client of changes
   - Mitigation: Not needed for MVP (tools are client-initiated)
   - Future: Add WebSocket for file change notifications

### Performance Characteristics

| Metric | Expected | Actual (TBD) |
|--------|----------|--------------|
| `tools/list` latency | < 10ms | - |
| `list_streams` latency | < 50ms | - |
| `sample_data(n=10)` latency | < 200ms | - |
| Memory per request | < 1MB | - |
| Concurrent requests | 10+ | - |

## Alternatives Considered

### Alternative 1: stdio Transport

**How it works**: MCP server runs as subprocess, communicates via stdin/stdout.

```bash
# Client spawns server process
./ndp-mcp-server --transport stdio

# Communication via pipes
echo '{"method":"tools/list"}' | ./ndp-mcp-server
```

**Rejected because**:
- Cannot communicate across network (Mac to Pi)
- Requires server binary on client machine
- No cloud deployment path
- Process lifecycle management complexity

### Alternative 2: SSE (Server-Sent Events)

**How it works**: HTTP long-polling with server push capability.

```
Client ──► GET /mcp/events (SSE connection)
           ◄── event: tool_result
           ◄── event: tool_result
```

**Rejected for MVP because**:
- More complex client implementation
- Not needed for request/response tools
- Added connection management overhead
- Can be added later as secondary transport

### Alternative 3: WebSocket

**How it works**: Full-duplex persistent connection.

```
Client ◄──► WS /mcp (bidirectional)
```

**Rejected for MVP because**:
- Overkill for simple tool calls
- Connection management complexity
- Proxy traversal issues
- Higher memory per connection
- Can be added later if needed

### Alternative 4: gRPC

**How it works**: Protocol Buffers over HTTP/2.

**Rejected because**:
- MCP specification uses JSON-RPC, not gRPC
- Additional protobuf compilation step
- Heavier dependency footprint
- Overkill for the use case

## Implementation Notes

### Dependencies

```toml
[dependencies]
axum = "0.7"                    # HTTP framework
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }
serde_json = "1"               # JSON-RPC serialization
```

### Error Handling

HTTP status codes map to error types:

| Error Type | HTTP Status | JSON-RPC Code |
|------------|-------------|---------------|
| Tool success | 200 | N/A |
| Tool error (expected) | 200 | N/A (isError: true) |
| Method not found | 200 | -32601 |
| Invalid params | 200 | -32602 |
| Internal error | 200 | -32603 |
| Server unavailable | 503 | N/A |

### CORS Configuration

For development from different origins:

```rust
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods([Method::POST, Method::GET])
    .allow_headers([CONTENT_TYPE]);
```

## Related Decisions

- [ADR-002: Storage Abstraction](./ADR-002-storage-abstraction.md) - How storage is accessed
- [ADR-003: Config Source](./ADR-003-config-source.md) - etcd as config source
- [ADR-005: Response Format](./ADR-005-response-format.md) - MCP response structure

## References

- [MCP Specification - Transports](https://modelcontextprotocol.io/specification/2025-11-25#transports)
- [axum Documentation](https://docs.rs/axum/latest/axum/)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [NDP Resource Constraints](../../dp-001/architecture/) - Pi memory budgets
