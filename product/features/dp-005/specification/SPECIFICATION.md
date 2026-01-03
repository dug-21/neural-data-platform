# dp-005: Bronze MCP Server - Specification (SPARC S Phase)

**Document Type**: SPARC Specification - Executive Summary
**Version**: 1.0.0
**Last Updated**: 2026-01-03
**Status**: COMPLETE

---

## Executive Summary

The Bronze MCP Server is a Rust-based Model Context Protocol (MCP) server that exposes Bronze layer data exploration and configuration validation tools to development agents. Running on the Raspberry Pi edge deployment with cloud portability in mind, this server enables Claude Code and other MCP clients to programmatically discover, inspect, and validate NDP's Bronze layer data.

The server addresses a critical gap in the config-driven NDP platform: development agents currently lack standardized access to Bronze layer data structure and configuration. Without this capability, agents cannot verify that incoming data matches configuration expectations, explore actual data schemas for ETL development, or validate the full configuration sync pipeline from source YAML through etcd to runtime.

The MVP delivers four essential tools (list_streams, describe_schema, validate_config, sample_data) via HTTP transport, maintaining a minimal memory footprint (<50MB) suitable for edge deployment while following MCP specification standards for broad tooling compatibility.

---

## Scope Summary

**Primary Goals** (from SCOPE.md):

| Goal | Description |
|------|-------------|
| Agent Data Exploration | Enable agents to query stream structure and sample data |
| Config-Data Validation | Detect mismatches between etcd configuration and Bronze reality |
| Pipeline Validation | Validate full sync: source YAML -> config-client -> etcd -> MCP |
| Cloud Portability | Same server works on Pi today, cloud tomorrow |
| Minimal Footprint | <50MB memory overhead on edge deployment |
| Standards Compliance | Follow MCP specification for Claude Code integration |
| Deployment Ready | Dockerfile and docker-compose integration for Pi deployment |

**Non-Goals (MVP)**: SQL query execution, Silver layer access, authentication, write operations, type/value constraint validation.

---

## Functional Requirements Summary

| ID | Requirement | Priority | Details |
|----|-------------|----------|---------|
| FR-001 | MCP Server Startup and Discovery | Must Have | Server binds to configured address, responds to `tools/list` and `tools/call`, provides `/health` endpoint |
| FR-002 | Tool: list_streams | Must Have | Enumerates all streams from etcd with metadata (description, version, enabled, sources) and storage info from filesystem |
| FR-003 | Tool: describe_schema | Must Have | Returns schema info with modes: source (raw_payload structure + mappings), target (entity_schemas), all (combined + gap analysis) |
| FR-004 | Tool: validate_config | Must Have | Compares etcd config fields vs Parquet raw_payload fields, reports matching/missing/extra fields with explanatory notes |
| FR-005 | Tool: sample_data | Must Have | Returns N most recent rows (default 10, max 100) with full Bronze envelope structure |
| FR-006 | Error Handling | Must Have | Consistent error format with codes (STREAM_NOT_FOUND, ETCD_UNAVAILABLE, etc.), no internal stack trace exposure |

See: [requirements.md](requirements.md) for detailed acceptance criteria and response structures.

---

## Non-Functional Requirements Summary

| ID | Category | Requirement | Target |
|----|----------|-------------|--------|
| NFR-001 | Performance | Memory usage | <50MB idle, <100MB under load |
| NFR-001 | Performance | Response time | <100ms cached, <500ms cold |
| NFR-002 | Reliability | etcd unavailable | Fail fast, no stale data |
| NFR-002 | Reliability | Graceful shutdown | Complete in-flight requests on SIGTERM |
| NFR-003 | Portability | Configuration | All values from environment |
| NFR-003 | Portability | Storage abstraction | BronzeStorage trait for local/cloud |
| NFR-004 | Observability | Logging | Structured tracing with configurable levels |
| NFR-005 | Security | No sensitive exposure | Validate stream_id format, bind to configurable address |

See: [requirements.md](requirements.md) for complete NFR specifications.

---

## Interface Summary

### HTTP Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/mcp` | POST | MCP JSON-RPC protocol messages |
| `/health` | GET | Health check with version and status |

### MCP Tools

| Tool | Input | Output Summary |
|------|-------|----------------|
| `list_streams` | none | Array of streams with metadata + storage info |
| `describe_schema` | `stream_id`, `mode` (all/source/target) | Schema structure based on mode |
| `validate_config` | `stream_id` | Validation status with field diff analysis |
| `sample_data` | `stream_id`, `n` (1-100) | Array of Bronze envelope rows |

### Response Format

All tool responses use MCP content format:
```json
{
  "content": [{"type": "text", "text": "{\"success\": true, ...}"}]
}
```

Error responses include `"isError": true` with structured error codes.

See: [interfaces.md](interfaces.md) for complete JSON-RPC formats, input schemas, and response structures.

---

## Data Contracts Summary

### Bronze Layer Structure

| Component | Format | Description |
|-----------|--------|-------------|
| File Organization | Hive-style | `/data/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet` |
| Parquet Schema | Arrow | timestamp, source_id, ndp_id, context, raw_payload, partition columns |
| raw_payload | JSON string | Exact source payload, untransformed |
| context | JSON string | Config-derived metadata snapshot |

### etcd Configuration

| Key Pattern | Example |
|-------------|---------|
| Stream metadata | `/streams/{stream_id}/stream_id`, `description`, `enabled`, `version` |
| Sources | `/streams/{stream_id}/sources/{index}/type`, `parser/field_mappings/*` |
| Entity schemas | `/streams/{stream_id}/entity_schemas/{index}/schema_name`, `attributes/*` |

### Source of Truth by Domain

| Domain | Source | Location |
|--------|--------|----------|
| Bronze structure | Parquet file | Introspected from `/data/raw/` |
| Field mappings | Parser config | `sources[].parser.field_mappings` |
| Silver/Target schema | Entity schemas | `entity_schemas[].attributes` |

See: [data-contracts.md](data-contracts.md) for complete etcd key patterns, Parquet schema definition, and entity schema format.

---

## Dependencies Summary

### Runtime Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| etcd | 3.5+ | Configuration storage |
| Rust | 1.75+ | Server implementation |
| Parquet files | Arrow 53 | Bronze layer data |

### Key Rust Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| axum | 0.7 | HTTP server |
| tokio | 1.x | Async runtime |
| etcd-client | 0.14 | etcd v3 API |
| parquet/arrow | 53 | Parquet reading |
| serde/serde_json | 1.x | Serialization |
| tracing | 0.1 | Observability |

### Feature Dependencies

| Feature | Status | Required For |
|---------|--------|--------------|
| DP-004 | Complete | Bronze raw JSON schema |
| AIR-011 | Complete | RawDataPoint structure |
| etcd sync | Complete | Config availability |

See: [dependencies.md](dependencies.md) for complete crate versions, infrastructure requirements, and build dependencies.

---

## Test Coverage Summary

### Test Strategy

- **London School TDD**: Outside-in development with mock-driven tests
- **Test Pyramid**: Unit (45+ tests) -> Integration (etcd + Parquet) -> System (Pi deployment)

### Coverage by Component

| Component | Test Cases | Priority |
|-----------|------------|----------|
| list_streams | TC-LS-001 to TC-LS-004 | 2 P0, 2 P1 |
| describe_schema | TC-DS-010 to TC-DS-015 | 3 P0, 2 P1, 1 P2 |
| validate_config | TC-VC-020 to TC-VC-024 | 4 P0, 1 P1 |
| sample_data | TC-SD-030 to TC-SD-035 | 2 P0, 3 P1, 1 P2 |
| MCP Protocol | TC-INT-001 to TC-INT-005 | 2 P0, 3 P1 |
| **Total** | **24 test cases** | **13 P0, 9 P1, 2 P2** |

### System Test Targets

| Metric | Target |
|--------|--------|
| Memory (idle) | <30MB RSS |
| Memory (active) | <50MB RSS |
| Response time p95 | <500ms |
| Startup time | <5s |

See: [test-plan.md](test-plan.md), [test-cases.md](test-cases.md), [test-fixtures.md](test-fixtures.md) for complete testing documentation.

---

## Deployment Requirements Summary

### Container Configuration

| Aspect | Specification |
|--------|---------------|
| Build | Multi-stage Dockerfile |
| Base (runtime) | debian:bookworm-slim |
| Port | 9100 |
| Memory limit | 64MB |
| Image size | < 50MB |

### Docker Compose Integration

| Service | ndp-mcp-server |
|---------|----------------|
| Dependencies | etcd (healthy) |
| Volume | air-quality-data:/data/raw:ro |
| Network | neural-network |
| Health check | GET /health |

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| NDP_RAW_PATH | /data/raw | Bronze data directory |
| NDP_ETCD_ENDPOINTS | http://etcd:2379 | Config store |
| NDP_MCP_LISTEN | 0.0.0.0:9100 | Server bind address |
| RUST_LOG | info | Log verbosity |

See: [deployment-requirements.md](deployment-requirements.md) for complete deployment specifications.

---

## Design Patterns

Key patterns adopted from MCP reference implementations:

1. **Tool Registry Pattern**: Trait-based tool abstraction for extensibility
2. **Consistent Response Format**: JSON with success flag, structured errors
3. **Health Endpoint**: Essential for monitoring and load balancer integration
4. **Environment Configuration**: No hardcoded values
5. **HTTP POST Transport**: SSE deferred to post-MVP

See: [mcp-design-patterns.md](mcp-design-patterns.md) for Rust translation of reference patterns.

---

## Acceptance Criteria Summary

### Must Have (MVP Exit Criteria)

- [ ] MCP server starts and responds to `tools/list`
- [ ] All 4 tools callable via `tools/call`
- [ ] `list_streams` returns all Bronze streams with metadata
- [ ] `describe_schema` returns accurate Parquet + config schema
- [ ] `validate_config` compares config vs data with diff
- [ ] `sample_data` returns N recent rows as JSON
- [ ] Health endpoint returns server status
- [ ] Server runs on Pi with <50MB memory idle
- [ ] Claude Code can connect and use tools via mcp.json
- [ ] Dockerfile created and builds on ARM64
- [ ] Docker Compose service defined
- [ ] deploy.sh status shows MCP server health
- [ ] Container runs within 64MB memory limit

### Should Have

- [ ] Structured logging with tracing
- [ ] Graceful shutdown
- [ ] Config validation on startup
- [ ] Actionable error messages

### Could Have (Post-MVP)

- [ ] Prometheus metrics endpoint
- [ ] SSE transport mode
- [ ] SQL query tool
- [ ] Authentication layer
- [ ] S3 storage backend

---

## Document Index

| Document | Purpose |
|----------|---------|
| [SCOPE.md](../SCOPE.md) | Feature scope definition and data landscape |
| [requirements.md](requirements.md) | Detailed functional and non-functional requirements |
| [interfaces.md](interfaces.md) | API contracts, MCP protocol, tool schemas |
| [data-contracts.md](data-contracts.md) | etcd keys, Parquet schema, entity schema format |
| [dependencies.md](dependencies.md) | Rust crates, infrastructure, build requirements |
| [test-plan.md](test-plan.md) | Testing strategy and execution |
| [test-cases.md](test-cases.md) | Detailed test case specifications |
| [test-fixtures.md](test-fixtures.md) | Test data and mock definitions |
| [mcp-design-patterns.md](mcp-design-patterns.md) | Reference implementation patterns |
| [deployment-requirements.md](deployment-requirements.md) | Container, Docker Compose, and deploy.sh integration requirements |

---

## Next Steps (SPARC Phases)

1. **Pseudocode (P)**: Algorithm design for tool implementations
2. **Architecture (A)**: System design, ADRs, component diagrams
3. **Refinement (R)**: TDD implementation following test cases
4. **Completion (C)**: Integration, deployment, verification

---

*This document consolidates the SPARC Specification phase for DP-005. All detailed requirements are cross-referenced in the Document Index above.*
