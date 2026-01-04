# BUG-001: MCP Server Should Use config-client Instead of Duplicate etcd Implementation

**Status:** ✓ FULLY RESOLVED (Implementation + Legacy Cleanup Complete)
**Severity:** Medium
**Resolution Date:** 2026-01-04
**Component:** core/ndp-mcp-server
**Created:** 2026-01-04
**Reporter:** Development Team

## Summary

The Bronze MCP Server implements its own etcd configuration access (`EtcdConfigStore`) instead of using the existing `config-client` crate that the main application uses. This creates:
1. Code duplication
2. Potential inconsistency in configuration parsing
3. Maintenance burden of two separate implementations

## Symptoms

- `list_streams` works (reads from etcd)
- `describe_schema` fails with "Stream not found"
- `sample_data` fails with "Stream not found"
- Parquet files exist at correct path (`/data/raw/{stream_id}/year=.../data.parquet`)

## Root Cause Analysis

### Issue 1: Duplicate etcd Implementation

| Component | etcd Access Method | Location |
|-----------|-------------------|----------|
| MCP Server | `EtcdConfigStore` (custom) | `core/src/mcp/etcd_config_store.rs` |
| Main App | `config-client::ConfigClient` | `config-client/` crate |

The MCP server has ~500 lines of custom etcd parsing code that duplicates functionality already available in `config-client`.

### Issue 2: Configuration Not Shared

The MCP server does NOT read storage path from etcd configuration. The main app reads `/storage/base_path` from etcd (see `apps/air-quality-app/src/config_etcd.rs:86-107`), but the MCP server only uses `NDP_RAW_PATH` environment variable.

### Issue 3: Files to Remove/Refactor

```
core/src/mcp/etcd_config_store.rs    - DELETE (use config-client)
core/src/mcp/config_store.rs         - DELETE (use config-client)
core/src/mcp/types.rs                - REFACTOR (use config-client types)
core/ndp-mcp-server/src/config.rs    - REFACTOR (use config-client for etcd)
```

## Evidence

Data exists in container:
```bash
docker exec air-quality-app ls -laR /data/raw/outdoor-weather
# Shows: year=2026/month=01/day=01-04/data.parquet files
```

MCP Server config shows default path matches:
```rust
// core/ndp-mcp-server/src/config.rs:52
raw_path: std::env::var("NDP_RAW_PATH")
    .unwrap_or_else(|_| "/data/raw".to_string()),
```

But MCP server runs in separate container without shared volume access to the data.

## Acceptance Criteria

1. [x] MCP server uses `config-client` crate for all etcd access
2. [x] Remove duplicate `EtcdConfigStore` implementation - **~1,062 lines deleted**
3. [x] Storage base path read from etcd `/storage/base_path` (with env fallback)
4. [x] All existing MCP tools work: `list_streams`, `describe_schema`, `validate_config`, `sample_data`
5. [x] All tests updated and passing
6. [x] No backward compatibility concerns (alpha software)

## Resolution

See [BUG-001-outcome-report.md](./BUG-001-outcome-report.md) for full implementation details.

## Implementation Plan

### Phase 1: Add config-client Dependency
- Add `config-client` to `core/ndp-mcp-server/Cargo.toml`
- Update imports

### Phase 2: Replace EtcdConfigStore
- Use `StreamRegistry` from config-client for stream listing
- Use `ConfigClient` for configuration access
- Read storage path from etcd with env fallback

### Phase 3: Cleanup
- Delete `core/src/mcp/etcd_config_store.rs`
- Delete `core/src/mcp/config_store.rs`
- Simplify `core/src/mcp/types.rs`
- Update all affected tests

### Phase 4: Validation
- Test all 4 MCP tools against real data
- Verify Parquet file access works
- Update integration tests

## Related Files

- `core/ndp-mcp-server/Cargo.toml`
- `core/ndp-mcp-server/src/config.rs`
- `core/ndp-mcp-server/src/server.rs`
- `core/src/mcp/etcd_config_store.rs` (to delete)
- `core/src/mcp/config_store.rs` (to delete)
- `core/src/mcp/handler.rs`
- `core/src/mcp/mod.rs`
- `config-client/src/lib.rs`
- `config-client/src/stream/registry.rs`

## Notes

- This is alpha software - no backward compatibility required
- Main app already proves config-client works correctly
- Swarm coordination will be used for implementation
