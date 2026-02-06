# BUG-002: Domain objectives sync not migrated to Rust toolchain

## Status
OPEN — fix scoped as v1.1.12

## Severity
High — objectives are silently not synced to data dictionary on every deploy

## Discovered
2026-02-06, during v1.1.11 production deployment

## Symptom
```
[WARN] No domain.yaml files found in /home/doug/neural-data-platform/config/domains
```
Domain objectives, streams, and domain metadata are never written to `data_dictionary.objectives`, `data_dictionary.domain_streams`, or `data_dictionary.domains`.

## Root Cause

`sync_domains_to_data_dictionary()` in `deploy/pi/deploy.sh` (lines 883-1086) is ~200 lines of bash that:

1. **Only looks for `domain.yaml`** (line 908, 932) — config is `domain.json`
2. **Uses wrong key paths** — `yaml_get "$config_file" "domain.id"` expects nested YAML structure, but JSON is flat (`"id": "indoor-air-quality"`)
3. **Bypasses Rust toolchain** — parses config independently with `yaml_get`/`yaml_array_get` instead of using the ops-001 infrastructure

This function has been dead code since FE-002 standardized all config to JSON. Every deploy silently skips it.

## Impact

- `data_dictionary.objectives` table is empty (or stale from last YAML-era deploy)
- `data_dictionary.domains` table not populated
- `data_dictionary.domain_streams` table not populated
- `data_dictionary.constraints` table not populated
- Any downstream tool or query relying on these tables gets no data

## Affected tables

Per `deploy/pi/init-scripts/005_domain_objectives.sql`:

| Table | Columns |
|-------|---------|
| `data_dictionary.domains` | domain_id, description, stream_count, config_path |
| `data_dictionary.domain_streams` | domain_id, stream_id, alias, role |
| `data_dictionary.objectives` | objective_id, domain_id, description, target_stream, target_metric, condition, threshold, threshold_upper, unit, priority |
| `data_dictionary.constraints` | constraint_id, domain_id, description, constraint_stream, constraint_metric, condition, threshold, unit |

## Fix: `ndp domain sync` (v1.1.12)

Domain objectives sync is a **data dictionary population** operation — it writes metadata to `data_dictionary.*` tables. It belongs alongside `ndp dictionary sync` (which populates `data_dictionary.streams/*`), not in `ndp-gold-ddl` (which generates DDL for database objects like views, aggregates, procedures).

The ops-001 infrastructure already provides everything needed:

| Exists | Where | What |
|--------|-------|------|
| `DbClient` trait with `execute()`, `batch_execute()` | `crates/ndp-lib/src/db.rs` | DB abstraction with full write capability |
| `ConfigLoader` trait + `FileSystemConfigLoader` | `crates/ndp-lib/src/config.rs` | Config loading abstraction |
| `SyncReport`, `SyncOptions`, `NdpLibError` | `crates/ndp-lib/src/types.rs`, `error.rs` | Structured output and error patterns |
| Entity/verb CLI routing | `tools/ndp-cli/src/main.rs` | `ndp <entity> <verb>` pattern |
| `ndp dictionary sync` as reference impl | `tools/ndp-cli/src/commands/dictionary.rs` | End-to-end working example |
| `command -v ndp` fallback in deploy.sh | Lines 386, 1220 | Proven integration pattern |
| `domain.json` with all needed fields | `config/domains/indoor-air-quality/domain.json` | Config already complete |

### Deliverables

| # | Item | Details |
|---|------|---------|
| 1 | `ndp-lib/src/domain/` module | `types.rs` (DomainSyncEntry, ObjectiveSyncEntry, etc.), `sql.rs` (parameterized SQL), `mod.rs` (sync_domain function) |
| 2 | Extend `ConfigLoader` trait | Add `load_domain_configs()` to discover and parse `config/domains/*/domain.json` |
| 3 | `ndp domain sync` CLI command | `tools/ndp-cli/src/commands/domain.rs` — load configs, convert to entries, sync to DB, print report |
| 4 | deploy.sh integration | Replace `sync_domains_to_data_dictionary()` with `command -v ndp` pattern calling `ndp domain sync` |
| 5 | Unit tests (mock DbClient) | London TDD covering all 4 tables, same pattern as dictionary tests |
| 6 | Integration tests | Verify against `docker-compose.integration.yml` stack |

### Sync behavior

- Transaction-wrapped (BEGIN/COMMIT)
- Per domain: UPSERT `domains`, DELETE+INSERT `domain_streams`, DELETE+INSERT `objectives`, DELETE+INSERT `constraints`
- Parameterized SQL (no string concatenation)
- Returns structured `SyncReport` with counts

### Out of scope

| Item | Why |
|------|-----|
| `--apply` flag on ndp-gold-ddl | Separate enhancement; ndp-gold-ddl generates DDL, not metadata sync |
| `--objectives` flag on ndp-gold-ddl | Wrong tool — domain sync is data dictionary population, not DDL generation |
| Changes to `domain.json` config | Config already has all needed fields (objectives, streams, alignment) |
| Changes to init-scripts | Tables already exist via `005_domain_objectives.sql` |

### Prior incorrect analysis

The original bug report suggested adding `--objectives` and `--apply` flags to `ndp-gold-ddl`. This was wrong:
- `ndp-gold-ddl`'s `DbClient` only has `query()` (read-only for sync planning). `ndp-lib`'s `DbClient` already has `execute()` and `batch_execute()`.
- Domain sync is the same class of operation as dictionary sync (metadata population), not DDL generation.
- The ops-001 infrastructure (`ndp-lib` + `ndp-cli`) was built precisely for this pattern.

## Related

- BUG-001: Duplicate CTE names in detection procedure (FIXED in v1.1.11)
- ops-001 SCOPE.md: ndp-lib + ndp-cli foundation (implemented)
- ops-002 SCOPE.md: config-driven generators (implemented)
- `ndp dictionary sync`: reference implementation for the exact same pattern
