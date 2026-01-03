# dp-005: Bronze MCP Server - Acceptance Checklist

## Overview

This checklist defines the complete set of acceptance criteria that must be satisfied before dp-005 can be considered complete. Each item must be verified and signed off.

---

## MCP Protocol Compliance

### Server Capabilities

- [ ] Server responds to `initialize` request with capabilities
- [ ] `tools/list` returns valid tool definitions array
- [ ] `tools/call` routes to correct tool implementation
- [ ] JSON-RPC 2.0 format followed (jsonrpc, id, method, params)
- [ ] Error responses include `isError: true` flag

### Tool Definitions

- [ ] All 4 tools have `name` field
- [ ] All 4 tools have `description` field
- [ ] All 4 tools have valid `inputSchema` (JSON Schema)
- [ ] Required parameters marked in `required` array
- [ ] Default values specified for optional parameters

---

## Tool Functionality

### list_streams

- [ ] Returns all streams from etcd
- [ ] Includes `stream_id` for each stream
- [ ] Includes `description` from config
- [ ] Includes `enabled` status
- [ ] Includes `version` from config
- [ ] Includes `sources` array (mqtt, http_poll, etc.)
- [ ] Includes `storage` object with:
  - [ ] `latest_partition` path
  - [ ] `file_size_bytes`
  - [ ] `file_modified` timestamp
- [ ] `storage` is `null` for streams with no data
- [ ] Returns empty array when no streams configured
- [ ] Response time < 100 ms

### describe_schema

**Mode: source**
- [ ] Returns `raw_payload_structure` with keys
- [ ] Returns nested structure (main.temp, wind.speed)
- [ ] Returns `parser_type` from config
- [ ] Returns `field_mappings` array
- [ ] Each mapping has `source_path`, `target_field`, `unit`
- [ ] Returns `unmapped_source_fields`
- [ ] Returns `file_analyzed` path
- [ ] Response time < 100 ms

**Mode: target**
- [ ] Returns `entity_schema` name
- [ ] Returns `attributes` array
- [ ] Each attribute has `name`, `type`, `unit`, `nullable`
- [ ] Response time < 50 ms

**Mode: all**
- [ ] Returns combined `source` and `target` objects
- [ ] Returns `gap_analysis` object with:
  - [ ] `unmapped_source_fields`
  - [ ] `target_fields_without_mapping`
- [ ] Response time < 150 ms

**General**
- [ ] Default mode is `all` when not specified
- [ ] Returns error for unknown stream_id
- [ ] Handles streams with no Parquet data

### validate_config

- [ ] Compares entity_schema attributes vs raw_payload keys
- [ ] Returns `validation.status` (match/mismatch)
- [ ] Returns `config_fields` array
- [ ] Returns `raw_payload_fields` array
- [ ] Returns `analysis` object with:
  - [ ] `in_config_not_in_payload`
  - [ ] `in_payload_not_in_config`
  - [ ] `matching`
- [ ] Includes contextual `notes` explaining differences
- [ ] Response time < 200 ms
- [ ] Returns error for unknown stream_id

### sample_data

- [ ] Returns `row_count` matching actual rows
- [ ] Returns `rows` array with Bronze envelope:
  - [ ] `timestamp` (INT64 milliseconds)
  - [ ] `source_id`
  - [ ] `ndp_id` (nullable)
  - [ ] `context` (JSON object)
  - [ ] `raw_payload` (JSON object)
- [ ] Returns `source_file` path
- [ ] Default n=10 when not specified
- [ ] Respects n parameter up to max 100
- [ ] Caps at 100 rows even if higher requested
- [ ] Returns most recent rows from latest partition
- [ ] Response time < 500 ms for 10 rows
- [ ] Response time < 1000 ms for 100 rows
- [ ] Returns error for unknown stream_id

---

## Error Handling

### Error Response Format

- [ ] All errors include `success: false`
- [ ] All errors include `error` message
- [ ] All errors include `code` identifier
- [ ] Response wrapper includes `isError: true`

### Error Scenarios

- [ ] Stream not found returns `STREAM_NOT_FOUND`
- [ ] etcd unavailable returns `CONFIG_UNAVAILABLE` in < 1 second
- [ ] Parquet file missing returns `STORAGE_UNAVAILABLE`
- [ ] Parquet file corrupted returns `PARSE_ERROR`
- [ ] Invalid tool name returns `Unknown tool: {name}`
- [ ] Missing required parameter returns descriptive error

### Stability

- [ ] No panics in any error path
- [ ] No `unwrap()` or `expect()` in production code
- [ ] All Results properly propagated
- [ ] Graceful handling of malformed JSON

---

## Performance

### Memory

- [ ] Idle memory < 30 MB
- [ ] Under load memory < 50 MB
- [ ] No memory growth over 24 hours (< 5 MB increase)

### Response Times

- [ ] list_streams < 100 ms (P95)
- [ ] describe_schema < 150 ms (P95)
- [ ] validate_config < 200 ms (P95)
- [ ] sample_data (10 rows) < 500 ms (P95)
- [ ] sample_data (100 rows) < 1000 ms (P95)
- [ ] Health check < 10 ms (P95)

### Startup

- [ ] Server ready in < 5 seconds
- [ ] First request completes in < 500 ms

---

## Portability

### Environment Configuration

- [ ] `NDP_RAW_PATH` configurable (default: /data/raw)
- [ ] `NDP_ETCD_ENDPOINTS` configurable
- [ ] `NDP_MCP_LISTEN` configurable (default: 0.0.0.0:9100)
- [ ] `NDP_MCP_LOG_LEVEL` configurable (default: info)
- [ ] `NDP_ETCD_PREFIX` configurable (default: /config/streams)
- [ ] No hardcoded paths in source code
- [ ] No hardcoded hosts or ports in source code

### Storage Abstraction

- [ ] `BronzeStorage` trait defined
- [ ] `LocalParquetStorage` implements trait
- [ ] Trait supports future S3/GCS implementations
- [ ] All storage access through trait interface

---

## Integration

### Claude Code Connectivity

- [ ] MCP client config documented
- [ ] Server discoverable via `tools/list`
- [ ] All 4 tools invocable from Claude Code
- [ ] Error responses understood by Claude
- [ ] Example `.claude/mcp.json` provided

### Pi Deployment

- [ ] Dockerfile created
- [ ] docker-compose service defined
- [ ] Container builds on Pi (ARM64)
- [ ] Container starts successfully
- [ ] Logs accessible via `docker logs`
- [ ] Persists across container restarts
- [ ] Health check in docker-compose

### etcd Integration

- [ ] Connects to configured etcd endpoints
- [ ] Reads stream configs from correct prefix
- [ ] Handles etcd connection failures gracefully
- [ ] Reconnects after transient failures

---

## Code Quality

### Rust Standards

- [ ] `cargo build --release` succeeds
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt --check` passes
- [ ] `cargo test` all tests pass
- [ ] `cargo doc` generates without errors

### Test Coverage

- [ ] Unit tests: 80%+ coverage
- [ ] Integration tests for all tools
- [ ] Error scenario tests
- [ ] Performance benchmark tests

### Documentation

- [ ] All public APIs documented with `///` comments
- [ ] README.md in ndp-mcp-server directory
- [ ] API documentation in docs/dp-005/
- [ ] Deployment guide complete

---

## Observability

### Logging

- [ ] Structured JSON logging enabled
- [ ] Request/response logging (configurable)
- [ ] Error logging with context
- [ ] Log levels configurable at runtime

### Health Check

- [ ] GET /health responds with status
- [ ] Includes server version
- [ ] Includes uptime
- [ ] Includes etcd connectivity status

---

## Security

### Input Validation

- [ ] stream_id validated (alphanumeric + hyphen)
- [ ] n parameter bounded (1-100)
- [ ] mode parameter validated (all/source/target)
- [ ] Invalid JSON rejected gracefully

### Information Disclosure

- [ ] Stack traces not exposed in responses
- [ ] Internal paths not exposed in errors
- [ ] Sensitive config not logged

---

## Final Verification

### End-to-End Tests

- [ ] Claude Code can list all streams
- [ ] Claude Code can describe any stream schema
- [ ] Claude Code can validate any stream config
- [ ] Claude Code can sample data from any stream
- [ ] Error scenarios handled gracefully in Claude

### Production Readiness

- [ ] 24-hour soak test completed
- [ ] Memory stable under sustained load
- [ ] No error rate spikes
- [ ] Deployment documentation reviewed

---

## Sign-Off

| Phase | Verified By | Date | Notes |
|-------|-------------|------|-------|
| MCP Protocol Compliance | | | |
| Tool Functionality | | | |
| Error Handling | | | |
| Performance | | | |
| Portability | | | |
| Integration | | | |
| Code Quality | | | |
| Security | | | |
| End-to-End | | | |

### Final Approval

- [ ] **MVP Complete** - All checklist items verified
- [ ] **Deployed to Pi** - Running in production
- [ ] **Claude Code Integrated** - Used successfully by agents

---

*Acceptance checklist for dp-005 Bronze MCP Server*
