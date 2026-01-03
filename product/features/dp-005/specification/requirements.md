# DP-005: Bronze MCP Server - Requirements Specification

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2026-01-03
**Status**: Draft

---

## Overview

This document defines the functional and non-functional requirements for the Bronze MCP Server - a Rust-based Model Context Protocol server that exposes Bronze layer data exploration and validation tools to development agents.

---

## Functional Requirements

### FR-001: MCP Server Startup and Discovery

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-001.1 | Server MUST start and bind to configured listen address | Must Have | Server starts without error when `NDP_MCP_LISTEN` is valid |
| FR-001.2 | Server MUST respond to `tools/list` MCP method | Must Have | Returns array of 4 tool definitions |
| FR-001.3 | Server MUST respond to `tools/call` MCP method | Must Have | Routes to correct tool implementation |
| FR-001.4 | Server MUST provide `/health` HTTP endpoint | Must Have | Returns `{"status": "ok", "version": "..."}` |
| FR-001.5 | Server MUST validate environment configuration on startup | Should Have | Fails fast with descriptive error if config invalid |

### FR-002: Tool - list_streams

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-002.1 | Tool MUST enumerate all stream IDs from etcd | Must Have | Returns streams matching `/streams/{stream_id}/stream_id` keys |
| FR-002.2 | Tool MUST include stream metadata: description, version, enabled | Must Have | Fields extracted from etcd config |
| FR-002.3 | Tool MUST include source types array | Must Have | Extracted from `sources[].type` in config |
| FR-002.4 | Tool MUST include storage info from filesystem | Must Have | latest_partition, file_size_bytes, file_modified |
| FR-002.5 | Tool MUST return null storage if no Parquet files exist | Must Have | `"storage": null` for streams without data |
| FR-002.6 | Tool MUST accept no input parameters | Must Have | Empty inputSchema properties |

**Response Structure:**
```json
{
  "success": true,
  "streams": [
    {
      "stream_id": "string",
      "description": "string",
      "enabled": "boolean",
      "version": "string",
      "sources": ["mqtt", "http_poll"],
      "storage": {
        "latest_partition": "year=YYYY/month=MM/day=DD",
        "file_size_bytes": "integer",
        "file_modified": "ISO8601 timestamp"
      } | null
    }
  ]
}
```

### FR-003: Tool - describe_schema

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-003.1 | Tool MUST accept required `stream_id` parameter | Must Have | Validates stream exists in etcd |
| FR-003.2 | Tool MUST accept optional `mode` parameter with values: all, source, target | Must Have | Defaults to "all" |
| FR-003.3 | Mode "source" MUST return raw_payload structure from Parquet | Must Have | Introspects actual `raw_payload` JSON keys |
| FR-003.4 | Mode "source" MUST return field_mappings from parser config | Must Have | Extracted from `sources[].parser.field_mappings` |
| FR-003.5 | Mode "source" MUST return unmapped_source_fields | Should Have | Fields in raw_payload not in any mapping |
| FR-003.6 | Mode "target" MUST return entity_schemas from config | Must Have | Full attributes array with name, type, unit, nullable |
| FR-003.7 | Mode "all" MUST return combined source + target + gap_analysis | Must Have | Identifies mapping gaps |
| FR-003.8 | Tool MUST return error if stream_id not found | Must Have | `{"success": false, "error": "Stream not found: ..."}` |
| FR-003.9 | Tool MUST include file_analyzed path in response | Should Have | Shows which Parquet file was inspected |

**Response Structure (mode: source):**
```json
{
  "success": true,
  "stream_id": "string",
  "mode": "source",
  "raw_payload_structure": {
    "keys": ["string"],
    "nested": {
      "parent_key": ["child_keys"]
    }
  },
  "parser_type": "json_path | flat_json | ...",
  "field_mappings": [
    {
      "source_path": "main.temp",
      "target_field": "temperature",
      "unit": "celsius"
    }
  ],
  "unmapped_source_fields": ["string"],
  "file_analyzed": "/data/raw/.../data.parquet"
}
```

**Response Structure (mode: target):**
```json
{
  "success": true,
  "stream_id": "string",
  "mode": "target",
  "entity_schema": "schema_name",
  "attributes": [
    {
      "name": "string",
      "type": "float | int | string | bool | json | timestamp",
      "unit": "string | null",
      "nullable": "boolean"
    }
  ]
}
```

**Response Structure (mode: all):**
```json
{
  "success": true,
  "stream_id": "string",
  "mode": "all",
  "source": { /* raw_payload_structure, field_mappings */ },
  "target": { /* entity_schema, attributes */ },
  "gap_analysis": {
    "unmapped_source_fields": ["string"],
    "target_fields_without_mapping": ["string"]
  }
}
```

### FR-004: Tool - validate_config

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-004.1 | Tool MUST accept required `stream_id` parameter | Must Have | Validates stream exists |
| FR-004.2 | Tool MUST extract config_fields from entity_schemas attributes | Must Have | All attribute names from first entity_schema |
| FR-004.3 | Tool MUST extract raw_payload_fields from Parquet sample | Must Have | Top-level keys from parsed raw_payload JSON |
| FR-004.4 | Tool MUST compute set difference: in_config_not_in_payload | Must Have | Fields expected but not in raw data |
| FR-004.5 | Tool MUST compute set difference: in_payload_not_in_config | Must Have | Fields in raw data but not configured |
| FR-004.6 | Tool MUST compute intersection: matching fields | Must Have | Fields present in both |
| FR-004.7 | Tool MUST return validation status: match, mismatch, partial | Must Have | Based on differences |
| FR-004.8 | Tool MUST include explanatory notes | Should Have | Explain why mismatches are expected |
| FR-004.9 | Tool MUST return error if no Parquet data exists | Must Have | Cannot validate without data |

**Response Structure:**
```json
{
  "success": true,
  "stream_id": "string",
  "entity_schema": "string",
  "validation": {
    "status": "match | mismatch | partial",
    "config_fields": ["string"],
    "raw_payload_fields": ["string"],
    "analysis": {
      "in_config_not_in_payload": ["string"],
      "in_payload_not_in_config": ["string"],
      "matching": ["string"]
    },
    "notes": "string"
  }
}
```

### FR-005: Tool - sample_data

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-005.1 | Tool MUST accept required `stream_id` parameter | Must Have | Validates stream exists |
| FR-005.2 | Tool MUST accept optional `n` parameter (default: 10) | Must Have | Number of rows to return |
| FR-005.3 | Tool MUST enforce maximum `n` of 100 | Must Have | Prevents excessive data transfer |
| FR-005.4 | Tool MUST return most recent N rows | Must Have | Ordered by timestamp descending |
| FR-005.5 | Tool MUST return full Bronze envelope structure | Must Have | timestamp, source_id, ndp_id, context, raw_payload |
| FR-005.6 | Tool MUST include source_file path in response | Should Have | Which Parquet file was sampled |
| FR-005.7 | Tool MUST return actual row_count | Must Have | May be less than requested if insufficient data |
| FR-005.8 | Tool MUST return error if no data exists | Must Have | Cannot sample empty stream |

**Response Structure:**
```json
{
  "success": true,
  "stream_id": "string",
  "row_count": "integer",
  "rows": [
    {
      "timestamp": "integer (microseconds)",
      "source_id": "string",
      "ndp_id": "string | null",
      "context": "object | null",
      "raw_payload": "object"
    }
  ],
  "source_file": "/data/raw/.../data.parquet"
}
```

### FR-006: Error Handling

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-006.1 | All errors MUST use consistent response format | Must Have | `{"success": false, "error": "...", "code": "..."}` |
| FR-006.2 | Errors MUST NOT expose internal stack traces | Must Have | User-friendly messages only |
| FR-006.3 | Errors MUST include error code for programmatic handling | Should Have | e.g., STREAM_NOT_FOUND, ETCD_UNAVAILABLE |
| FR-006.4 | Unknown tool names MUST return appropriate error | Must Have | `{"error": "Unknown tool: xyz"}` |
| FR-006.5 | Missing required parameters MUST return validation error | Must Have | Clear indication of missing field |

**Error Codes:**
| Code | Description |
|------|-------------|
| `STREAM_NOT_FOUND` | Requested stream_id does not exist in etcd |
| `ETCD_UNAVAILABLE` | Cannot connect to etcd cluster |
| `NO_DATA_AVAILABLE` | Stream exists but no Parquet files found |
| `INVALID_PARAMETER` | Parameter validation failed |
| `INTERNAL_ERROR` | Unexpected server error |
| `UNKNOWN_TOOL` | Requested tool name not registered |

---

## Non-Functional Requirements

### NFR-001: Performance

| ID | Requirement | Priority | Measurement |
|----|-------------|----------|-------------|
| NFR-001.1 | Server MUST use less than 50MB memory at idle | Must Have | RSS memory via `/proc/self/status` |
| NFR-001.2 | Server MUST use less than 100MB memory under load | Should Have | With 10 concurrent requests |
| NFR-001.3 | Tool responses MUST complete in <100ms for cached data | Must Have | p95 latency for hot requests |
| NFR-001.4 | Tool responses MUST complete in <500ms for cold data | Should Have | p95 latency including Parquet read |
| NFR-001.5 | list_streams MUST complete in <200ms | Must Have | Includes etcd + filesystem scan |
| NFR-001.6 | sample_data MUST stream results without full materialization | Should Have | Avoid loading all rows into memory |

### NFR-002: Reliability

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| NFR-002.1 | Server MUST fail fast if etcd is unavailable at startup | Must Have | Exit with error code, don't retry |
| NFR-002.2 | Server MUST timeout etcd operations after 5 seconds | Must Have | Prevent hanging requests |
| NFR-002.3 | Server MUST handle Parquet read errors gracefully | Must Have | Return error response, don't crash |
| NFR-002.4 | Server MUST implement graceful shutdown on SIGTERM | Should Have | Complete in-flight requests |
| NFR-002.5 | Server MUST recover from transient etcd disconnects | Should Have | Reconnect on next request |

### NFR-003: Portability

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| NFR-003.1 | All configuration MUST be environment-driven | Must Have | No hardcoded values |
| NFR-003.2 | Storage backend MUST be abstracted via trait | Must Have | BronzeStorage trait |
| NFR-003.3 | Local filesystem MUST be default storage implementation | Must Have | LocalParquetStorage |
| NFR-003.4 | Server MUST compile for aarch64 (Raspberry Pi) | Must Have | ARM64 cross-compilation |
| NFR-003.5 | Server MUST compile for x86_64 (cloud) | Must Have | Standard Linux deployment |
| NFR-003.6 | Config paths MUST support both local and S3-style URIs | Should Have | Future cloud portability |

### NFR-004: Observability

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| NFR-004.1 | Server MUST log requests with tracing | Must Have | Method, duration, success/error |
| NFR-004.2 | Server MUST support configurable log levels | Must Have | via NDP_MCP_LOG_LEVEL |
| NFR-004.3 | Server MUST log startup configuration | Must Have | Listen address, etcd endpoints, data path |
| NFR-004.4 | Server SHOULD include request IDs in logs | Should Have | For request tracing |
| NFR-004.5 | Health endpoint SHOULD include uptime | Should Have | Seconds since start |

### NFR-005: Security

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| NFR-005.1 | Server MUST NOT expose sensitive config values in responses | Must Have | No API keys, passwords |
| NFR-005.2 | Server MUST NOT allow path traversal in stream_id | Must Have | Validate stream_id format |
| NFR-005.3 | Server MUST bind to configurable address (not hardcoded) | Must Have | Allow localhost-only binding |
| NFR-005.4 | Server SHOULD support TLS termination (defer to proxy) | Could Have | For cloud deployment |
| NFR-005.5 | Authentication MUST be disabled for MVP | Must Have | Design hook, don't implement |

---

## Constraints

### Technical Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| TC-001 | Must use existing etcd for configuration | Architecture decision: GitOps config sync |
| TC-002 | Must use existing Parquet file structure | Compatibility with current Bronze layer |
| TC-003 | Must run on Raspberry Pi 5 (8GB RAM) | Edge deployment target |
| TC-004 | Must not require additional infrastructure | Minimize Pi resource usage |
| TC-005 | Must use Rust for implementation | Memory efficiency, type safety |

### Business Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| BC-001 | MVP scope: 4 tools only | Time-boxed delivery |
| BC-002 | Read-only operations only | Data integrity, simplicity |
| BC-003 | HTTP transport only (no SSE for MVP) | Simpler implementation |
| BC-004 | No authentication for MVP | Development convenience |

### Regulatory Constraints

None for MVP. Future considerations:
- Data privacy when exposing raw payloads
- Access logging for audit trails

---

## Dependencies

### Upstream Dependencies

| Dependency | Version | Purpose | Required By |
|------------|---------|---------|-------------|
| etcd | 3.5+ | Configuration storage | All tools |
| Parquet files | - | Bronze layer data | describe_schema, validate_config, sample_data |
| Stream configs | - | Source YAML synced to etcd | All tools |

### Feature Dependencies

| Feature | Status | Required For |
|---------|--------|--------------|
| DP-004 | Complete | Bronze raw JSON schema |
| AIR-011 | Complete | RawDataPoint structure |
| etcd sync | Complete | Config availability |

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

### Should Have

- [ ] Structured logging with tracing
- [ ] Graceful shutdown
- [ ] Config validation on startup
- [ ] Actionable error messages
- [ ] Request tracing IDs

### Could Have (Post-MVP)

- [ ] Prometheus metrics endpoint
- [ ] SSE transport mode
- [ ] SQL query tool
- [ ] Authentication layer
- [ ] S3 storage backend

---

## References

- [DP-005 SCOPE](/workspaces/neural-data-platform/product/features/dp-005/SCOPE.md)
- [MCP Design Patterns](/workspaces/neural-data-platform/product/features/dp-005/specification/mcp-design-patterns.md)
- [DP-004 Bronze Schema](/workspaces/neural-data-platform/product/features/dp-004/specification/REQUIREMENTS.md)
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)

---

*This document is part of the SPARC Specification phase for DP-005.*
