# ops-003 Analysis: Current State of CLI Tooling

> **Date**: 2026-02-06
> **Context**: Post ops-001 (v1.1.9) and ops-002 (v1.1.11-v1.1.13)

---

## Binary Inventory (7 binaries in workspace)

| Binary | Crate | Purpose | deploy.sh sites |
|--------|-------|---------|-----------------|
| `ndp` | tools/ndp-cli | Unified CLI (3 commands) | 3 (`command -v ndp`) |
| `ndp-validate` | tools/ndp-validate | Config validation | 2 (`command -v ndp-validate`) |
| `ndp-gold-ddl` | tools/ndp-gold-ddl | Gold DDL generation | 2 (`command -v ndp-gold-ddl`) |
| `air-quality-server` | apps/air-quality-app | Data ingestion | N/A (Docker) |
| `silver-etl` | apps/silver-etl | Bronze-to-Silver ETL | N/A (Docker) |
| `ndp-mcp-server` | core/ndp-mcp-server | MCP tools for AI agents | N/A (Docker) |

## The Three Deployment Tools

### 1. `ndp` CLI (tools/ndp-cli) -- 5 source files
- **Commands**: `dictionary sync`, `dimension sync`, `domain sync`
- **Depends on**: `ndp-lib` (shared library)
- **Pattern**: Entity/verb, all logic delegated to ndp-lib
- **Tests**: Via ndp-lib (94 tests)

### 2. `ndp-validate` (tools/ndp-validate) -- 13 source files, 217 tests
- **Commands**: Flat CLI with `--all`, `--domain`, `--domain-all`, `--generate-schema`
- **Depends on**: `ndp-types` only (NOT ndp-lib)
- **Pattern**: Standalone, own error types, own validation logic
- **Unique capability**: Two-layer validation (JSON Schema + semantic), Levenshtein suggestions

### 3. `ndp-gold-ddl` (tools/ndp-gold-ddl) -- 29 source files, 376 tests
- **Commands**: `generate`, `validate`
- **Depends on**: Nothing in workspace (fully standalone)
- **Pattern**: Standalone, own DbClient, own ConfigLoader, own config types
- **Unique capability**: DDL generation for continuous aggregates, aligned views, state transitions, events

## Shared Library: ndp-lib (crates/ndp-lib) -- 15 source files, 94 tests

| Module | Functions | Used By |
|--------|-----------|---------|
| `db` | `DbClient` trait, `PostgresClient` | ndp-cli only |
| `config` | `ConfigLoader` trait, `FileSystemConfigLoader` | ndp-cli only |
| `dictionary` | `sync_dictionary()` | ndp-cli |
| `dimension` | `sync_dimension()` | ndp-cli |
| `domain` | `sync_domains()` | ndp-cli |
| `convert` | Config-to-sync-type bridges | ndp-cli |
| `types` | `SyncReport`, `SyncOptions` | ndp-cli |

**Critical observation**: ndp-lib is only consumed by ndp-cli. Neither ndp-validate nor ndp-gold-ddl use it.

## Dependency Graph

```
ndp-types (foundation)
    |
    +---> ndp-lib -----> ndp-cli (ndp binary)
    |
    +---> ndp-validate (standalone -- own error/validation types)
    |
    (nothing)
         ndp-gold-ddl (fully standalone -- own EVERYTHING)
```

## Test Distribution

| Crate | Tests | % of Total |
|-------|-------|------------|
| ndp-gold-ddl | 376 | 53% |
| ndp-validate | 217 | 31% |
| ndp-lib | 94 | 13% |
| ndp-cli | 0 (thin wrapper) | 0% |
| **Total** | **687** | 100% |
