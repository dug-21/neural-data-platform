# dp-005: Bronze MCP Server - Success Criteria

## Overview

This document defines measurable success criteria for the Bronze MCP Server MVP. All metrics are designed to be verifiable through automated testing and manual validation.

---

## Performance Criteria

### Memory Footprint

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Resident memory (idle) | < 30 MB | `ps aux` on Pi after 1 hour idle |
| Resident memory (under load) | < 50 MB | `ps aux` during concurrent requests |
| Memory growth (24h) | < 5 MB | Compare RSS at start vs 24h later |

**Rationale**: Pi has 4GB RAM shared with TimescaleDB, Parquet writer, and other services. MCP server should be lightweight.

### Response Times

| Tool | Target | Conditions |
|------|--------|------------|
| `list_streams` | < 100 ms | Cold cache, 10 streams |
| `describe_schema` (source) | < 100 ms | Single stream, Parquet file < 10 MB |
| `describe_schema` (target) | < 50 ms | etcd lookup only |
| `describe_schema` (all) | < 150 ms | Combined source + target + gap analysis |
| `validate_config` | < 200 ms | Single stream |
| `sample_data` (10 rows) | < 500 ms | Single stream, Parquet file < 10 MB |
| `sample_data` (100 rows) | < 1000 ms | Single stream, Parquet file < 10 MB |
| Health check (`/health`) | < 10 ms | Always |

**Measurement Method**: P95 latency over 100 requests using `wrk` or similar.

### Startup Time

| Metric | Target | Notes |
|--------|--------|-------|
| Server ready | < 5 seconds | From process start to accepting requests |
| First request | < 500 ms | Additional latency acceptable for first request |

---

## Correctness Criteria

### Schema Accuracy

| Requirement | Verification |
|-------------|--------------|
| Parquet schema matches `describe_schema` output | Compare introspected schema vs tool output for all Bronze streams |
| All Bronze envelope fields present | `timestamp`, `source_id`, `ndp_id`, `context`, `raw_payload` |
| Nested JSON keys correctly extracted from `raw_payload` | Sample 10 rows, verify keys match |
| etcd config reflected in tool responses | Modify etcd value, verify tool reflects change |

### Data Integrity

| Requirement | Verification |
|-------------|--------------|
| `sample_data` returns exact Bronze envelope | Compare row-by-row with direct Parquet read |
| Zero data transformation in MCP layer | Raw JSON preserved exactly |
| Partition columns included when requested | `year`, `month`, `day` available |

### Config Validation Accuracy

| Requirement | Verification |
|-------------|--------------|
| `validate_config` detects all field mismatches | Create test config with known differences |
| Missing fields identified correctly | Remove field from config, verify detection |
| Extra fields identified correctly | Add field to config not in data, verify detection |
| Gap analysis shows unmapped source fields | Verify against manual inspection |

---

## Reliability Criteria

### Error Handling

| Scenario | Expected Behavior | Target |
|----------|-------------------|--------|
| etcd unavailable | Return error in < 1 second | Fail fast, no stale data |
| Stream not found | Structured error with stream ID | Immediate response |
| Parquet file missing | Structured error, suggest partition | Immediate response |
| Parquet file corrupted | Structured error, log details | < 1 second |
| Invalid JSON in `raw_payload` | Partial results + warning | Tool completes |
| Request timeout | Clean abort, log context | 30 second default |

### Stability

| Requirement | Verification |
|-------------|--------------|
| No panics | All error paths return `Result<T, E>` |
| No memory leaks | 24-hour soak test, RSS stable |
| Graceful shutdown | `SIGTERM` handled, connections drained |
| Connection limits respected | Max 100 concurrent connections |

### Error Response Format

All errors must follow this structure:

```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\": false, \"error\": \"...\", \"code\": \"...\", \"details\": {...}}"
  }],
  "isError": true
}
```

**Error Codes**:
| Code | Meaning |
|------|---------|
| `STREAM_NOT_FOUND` | Stream ID does not exist in etcd |
| `STORAGE_UNAVAILABLE` | Cannot access Bronze storage path |
| `CONFIG_UNAVAILABLE` | etcd unreachable or timeout |
| `PARSE_ERROR` | Failed to parse Parquet or JSON |
| `INTERNAL_ERROR` | Unexpected server error |

---

## Portability Criteria

### Environment Configuration

| Requirement | Verification |
|-------------|--------------|
| All config from environment variables | No hardcoded paths, hosts, or ports |
| `NDP_RAW_PATH` configurable | Test with `/tmp/test-raw` |
| `NDP_ETCD_ENDPOINTS` configurable | Test with different etcd host |
| `NDP_MCP_LISTEN` configurable | Test with different port |

### Storage Abstraction

| Requirement | Verification |
|-------------|--------------|
| `BronzeStorage` trait defined | Interface supports local + cloud |
| Local implementation complete | `LocalParquetStorage` passes all tests |
| S3/GCS pluggable (design only) | Trait allows future `S3ParquetStorage` |
| No filesystem paths in business logic | All storage access through trait |

### Environment Variables

```bash
# Required
NDP_RAW_PATH=/data/raw              # Bronze storage root
NDP_ETCD_ENDPOINTS=http://localhost:2379

# Optional (with defaults)
NDP_MCP_LISTEN=0.0.0.0:9100
NDP_MCP_LOG_LEVEL=info
NDP_MCP_REQUEST_TIMEOUT=30          # seconds
NDP_ETCD_PREFIX=/config/streams
NDP_AUTH_ENABLED=false              # Future: enable auth
```

---

## MCP Protocol Compliance

### Required Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/mcp` | POST | MCP JSON-RPC handler |
| `/health` | GET | Health check |

### MCP Methods

| Method | Status | Verification |
|--------|--------|--------------|
| `initialize` | Implemented | Returns capabilities |
| `tools/list` | Implemented | Returns 4 tool definitions |
| `tools/call` | Implemented | Routes to tool implementations |

### Tool Definition Compliance

Each tool must include:
- `name`: Unique identifier
- `description`: Clear purpose description
- `inputSchema`: Valid JSON Schema with `type`, `properties`, `required`

---

## Integration Criteria

### Claude Code Connectivity

| Requirement | Verification |
|-------------|--------------|
| MCP client config works | Add to `.claude/mcp.json`, verify connection |
| Tool discovery succeeds | `tools/list` returns all 4 tools |
| Tool invocation works | Each tool callable with valid params |
| Error responses understood | Claude interprets `isError: true` correctly |

### Pi Deployment

| Requirement | Verification |
|-------------|--------------|
| Docker container builds | `docker build` succeeds on Pi |
| Container starts successfully | `docker-compose up` works |
| Persists across restarts | Data accessible after container restart |
| Logs accessible | `docker logs ndp-mcp-server` shows output |

---

## Test Coverage Criteria

### Unit Tests

| Component | Coverage Target |
|-----------|-----------------|
| Tool implementations | 90% |
| MCP protocol handling | 80% |
| Storage layer | 85% |
| Config parsing | 90% |

### Integration Tests

| Test | Description |
|------|-------------|
| `test_list_streams_empty` | No streams, returns empty array |
| `test_list_streams_multiple` | Multiple streams, correct metadata |
| `test_describe_schema_source` | Source mode returns raw_payload structure |
| `test_describe_schema_target` | Target mode returns entity_schemas |
| `test_describe_schema_all` | All mode includes gap analysis |
| `test_validate_config_match` | Config matches data, status=match |
| `test_validate_config_mismatch` | Config differs, shows differences |
| `test_sample_data_default` | Returns 10 rows by default |
| `test_sample_data_limit` | Respects n parameter |
| `test_etcd_unavailable` | Graceful error handling |
| `test_stream_not_found` | Correct error response |

### End-to-End Tests

| Test | Description |
|------|-------------|
| `test_claude_code_flow` | Full workflow from Claude Code |
| `test_pi_deployment` | Verify deployment on actual Pi |

---

## Acceptance Checklist

### MVP Completion

- [ ] MCP server starts and responds to `tools/list`
- [ ] `list_streams` returns all Bronze streams with metadata
- [ ] `describe_schema` works in all 3 modes (source, target, all)
- [ ] `validate_config` compares config vs data correctly
- [ ] `sample_data` returns requested number of rows
- [ ] Health endpoint returns server status
- [ ] Memory usage < 50 MB on Pi
- [ ] All response times within targets
- [ ] Integration tests passing (90%+ coverage)
- [ ] Claude Code can connect and use all tools
- [ ] Pi deployment successful

### Quality Gates

- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt --check` passes
- [ ] `cargo test` passes (all unit + integration tests)
- [ ] `cargo doc` generates without errors
- [ ] No `unwrap()` or `expect()` in production code paths
- [ ] All public APIs documented

---

## Monitoring (Post-MVP)

### Metrics to Track

| Metric | Type | Purpose |
|--------|------|---------|
| `mcp_request_duration_seconds` | Histogram | Response time distribution |
| `mcp_requests_total` | Counter | Request volume |
| `mcp_errors_total` | Counter | Error rate |
| `mcp_active_connections` | Gauge | Connection pool usage |
| `mcp_memory_bytes` | Gauge | Memory consumption |

### Alerting Thresholds (Future)

| Alert | Condition |
|-------|-----------|
| High latency | P95 > 2x target for 5 minutes |
| Error spike | Error rate > 5% for 5 minutes |
| Memory pressure | RSS > 100 MB |
| Connection exhaustion | Active > 80 connections |

---

*Success criteria defined for dp-005 MVP*
