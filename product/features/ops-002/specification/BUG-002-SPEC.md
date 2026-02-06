# BUG-002 SPARC Specification: Domain Objectives Sync Migration to Rust Toolchain

**Version:** 1.0
**Feature:** ops-002
**Release:** v1.1.12 (PATCH)
**Date:** 2026-02-06
**Status:** DRAFT

---

## 1. Problem Statement

The `sync_domains_to_data_dictionary()` function in `deploy/pi/deploy.sh` (lines 883-1086) is approximately 200 lines of dead Bash code that silently fails on every deployment. Three root causes make it non-functional:

1. **Wrong file extension:** Lines 908 and 932 glob for `*/domain.yaml`, but FE-002 standardized all configuration to JSON. The actual config file is `config/domains/indoor-air-quality/domain.json`.
2. **Wrong key paths:** The function calls `yaml_get "$config_file" "domain.id"`, expecting a nested YAML structure with a `domain:` top-level key. The JSON config is flat: `{"id": "indoor-air-quality", ...}`.
3. **Bypasses Rust toolchain:** The function parses config independently using `yaml_get`/`yaml_array_get` shell helpers instead of using the ops-001 infrastructure (`ndp-lib` + `ndp-cli`) that was purpose-built for this class of operation.

**Observable symptom on every deploy:**
```
[WARN] No domain.yaml files found in /home/doug/neural-data-platform/config/domains
```

**Impact:** The following `data_dictionary` tables are never populated (or contain stale data from the last YAML-era deployment):

| Table | Purpose |
|-------|---------|
| `data_dictionary.domains` | Domain metadata (description, stream count, config path) |
| `data_dictionary.domain_streams` | Stream-to-domain mapping with roles (primary, context, actuator, constraint) |
| `data_dictionary.objectives` | Target metrics for pattern detection and threshold crossing (CO2 < 800 ppm, PM2.5 < 12 ug/m3, etc.) |
| `data_dictionary.constraints` | Conditions for the V1.3+ action framework |

Any downstream tool, Grafana dashboard, or Gold layer generator that queries these tables gets empty or stale results. The Gold layer `EventsGenerator` (v1.1.10) reads objectives from these tables to generate threshold crossing detection procedures.

---

## 2. Requirements

### 2.1 Functional Requirements

| ID | Description | Priority |
|----|-------------|----------|
| FR-001 | System shall provide an `ndp domain sync` CLI command that reads domain configuration from `config/domains/*/domain.json` and writes to the 4 target `data_dictionary` tables. | High |
| FR-002 | The sync function shall accept parsed `DomainConfig` structs (not file paths), consistent with the ndp-lib design principle of parsed-structs-not-paths. | High |
| FR-003 | The `domains` table shall be populated via UPSERT (ON CONFLICT on `domain_id` DO UPDATE). | High |
| FR-004 | The `domain_streams` table shall be populated via DELETE+INSERT per domain (full sync within domain scope). | High |
| FR-005 | The `objectives` table shall be populated via DELETE+INSERT per domain (full sync within domain scope). | High |
| FR-006 | The `constraints` table shall be populated via DELETE+INSERT per domain (full sync within domain scope). FK cascade from `domains` ensures orphan cleanup. | High |
| FR-007 | All SQL shall use parameterized queries (`$1`, `$2`, ...). No string concatenation for values. | High |
| FR-008 | The entire sync operation shall be wrapped in a single transaction (BEGIN/COMMIT). | High |
| FR-009 | The command shall return a structured `SyncReport` with counts of domains processed, streams mapped, objectives synced, and constraints synced. | High |
| FR-010 | The command shall support `--dry-run` mode that reports what would be synced without executing SQL. | Medium |
| FR-011 | The command shall accept `--config-dir` to override the default domain config directory (`config/domains`). | Medium |
| FR-012 | The `ConfigLoader` trait shall be extended with a `load_domain_configs()` method. `FileSystemConfigLoader` shall discover `config/domains/*/domain.json` files. | High |
| FR-013 | `deploy.sh` shall be updated to replace the `sync_domains_to_data_dictionary()` body with the `command -v ndp` fallback pattern calling `ndp domain sync`. | High |
| FR-014 | The `stream_count` column in `data_dictionary.domains` shall be populated from the actual length of the `streams[]` array in the config, not from a separate count field. | Medium |
| FR-015 | The `config_path` column shall store the relative path `config/domains/<domain_id>/domain.json`. | Low |
| FR-016 | Objectives with `"between"` condition shall store `threshold` (lower) and `threshold_upper` (upper) from the threshold array `[min, max]`. Non-between conditions shall set `threshold_upper` to NULL. | High |
| FR-017 | Non-fatal errors (e.g., a single domain fails to parse) shall be captured in `SyncReport.errors` without aborting the entire sync. Fatal errors (DB connection, transaction failure) shall propagate immediately. | Medium |

### 2.2 Non-Functional Requirements

| ID | Category | Description | Measurement |
|----|----------|-------------|-------------|
| NFR-001 | Consistency | Module structure shall follow the exact pattern established by `crates/ndp-lib/src/dictionary/` (mod.rs, types.rs, sql.rs). | Code review |
| NFR-002 | Testability | All sync logic shall be tested via London TDD with MockDbClient. No integration database required for unit tests. | `cargo test` passes without DB |
| NFR-003 | Security | No SQL injection vectors. All values passed via parameterized queries. | Code review: zero string interpolation in SQL |
| NFR-004 | Performance | Sync of a single domain with 6 objectives and 4 streams shall complete in < 100ms against a local TimescaleDB. | Integration test timing |
| NFR-005 | Backward Compatibility | If `ndp` CLI is not available on the deployment target, `deploy.sh` shall fall back to a warning (not an error) and skip domain sync, matching the existing pattern at lines 384-392. | Deploy without ndp binary |
| NFR-006 | Idempotency | Running `ndp domain sync` twice with the same config shall produce identical database state. | Integration test |
| NFR-007 | Observability | Sync shall emit structured tracing logs (info for start/complete, warn for skipped domains, error for failures). | Log inspection |

---

## 3. Acceptance Criteria

### AC-001: Domain metadata synced to `data_dictionary.domains`

```
GIVEN the domain config at config/domains/indoor-air-quality/domain.json
  AND a running TimescaleDB with the 005_domain_objectives.sql schema
WHEN I run `ndp domain sync`
THEN data_dictionary.domains contains exactly 1 row WHERE domain_id = 'indoor-air-quality'
  AND description = 'Maintain healthy indoor air quality'
  AND stream_count = 4
  AND config_path = 'config/domains/indoor-air-quality/domain.json'
```

### AC-002: Domain streams synced to `data_dictionary.domain_streams`

```
GIVEN the indoor-air-quality domain config with 4 streams
WHEN I run `ndp domain sync`
THEN data_dictionary.domain_streams contains exactly 4 rows for domain_id = 'indoor-air-quality'
  AND stream_id = 'air-quality' has alias = 'indoor', role = 'primary'
  AND stream_id = 'outdoor-weather' has alias = 'outdoor', role = 'context'
  AND stream_id = 'home-assistant-state' has alias = 'state', role = 'actuator'
  AND stream_id = 'outdoor-air-quality' has alias = 'outdoor_aqi', role = 'constraint'
```

### AC-003: Objectives synced to `data_dictionary.objectives`

```
GIVEN the indoor-air-quality domain config with 6 objectives
WHEN I run `ndp domain sync`
THEN data_dictionary.objectives contains exactly 6 rows for domain_id = 'indoor-air-quality'
  AND objective_id = 'healthy_co2' has:
      target_stream = 'air-quality', target_metric = 'co2',
      condition = '<', threshold = 800, threshold_upper = NULL,
      unit = 'ppm', priority = 'high'
  AND objective_id = 'healthy_pm25' has:
      target_stream = 'air-quality', target_metric = 'pm25',
      condition = '<', threshold = 12, threshold_upper = NULL,
      unit = 'ug/m3', priority = 'high'
  AND all 4 medium-priority objectives (humidity and temperature bounds) are present
```

### AC-004: Idempotent re-run

```
GIVEN `ndp domain sync` has already been run successfully
WHEN I run `ndp domain sync` again with the same config
THEN all row counts remain identical
  AND no duplicate rows exist in any table
  AND updated_at on data_dictionary.domains is refreshed
```

### AC-005: Transaction safety

```
GIVEN a domain config with invalid data in the second domain (e.g., missing required field)
WHEN I run `ndp domain sync`
THEN the first domain is committed successfully
  AND the second domain's error is recorded in SyncReport.errors
  AND the overall command returns success with warnings
```

Note: Per FR-008 the entire operation is one transaction, but per FR-017 individual domain parse failures are non-fatal. The transaction wraps the DB operations, not the config parsing.

### AC-006: Dry run produces no side effects

```
GIVEN a valid domain config
WHEN I run `ndp domain sync --dry-run`
THEN no SQL is executed against the database
  AND the SyncReport contains the counts that would have been synced
  AND the command prints a summary to stdout
```

### AC-007: deploy.sh integration

```
GIVEN the ndp binary is available on PATH (or at /opt/ndp/bin/ndp or $REPO_ROOT/target/release/ndp)
WHEN deploy.sh calls sync_domains_to_data_dictionary()
THEN it executes `ndp domain sync --db-url <url> --config-dir <path>`
  AND the function logs the SyncReport summary

GIVEN the ndp binary is NOT available
WHEN deploy.sh calls sync_domains_to_data_dictionary()
THEN it logs a warning: "ndp CLI not available, skipping domain objectives sync"
  AND returns 0 (not a deployment failure)
```

### AC-008: Parameterized SQL (no injection)

```
GIVEN a domain config with description containing a single quote: "O'Reilly's domain"
WHEN I run `ndp domain sync`
THEN the sync succeeds without SQL syntax errors
  AND the description is stored correctly in the database
  (verified by: parameterized queries handle escaping at the driver level)
```

### AC-009: Constraints table (empty case)

```
GIVEN a domain config with no "constraints" array (like the current indoor-air-quality config)
WHEN I run `ndp domain sync`
THEN data_dictionary.constraints contains 0 rows for that domain
  AND no errors are reported
```

### AC-010: Constraints table (populated case)

```
GIVEN a domain config with a "constraints" array:
  [{"id": "outdoor_aqi_safe", "description": "Outdoor AQI < 100",
    "stream": "outdoor-air-quality", "metric": "aqi",
    "condition": "<", "threshold": 100, "unit": "AQI"}]
WHEN I run `ndp domain sync`
THEN data_dictionary.constraints contains 1 row for that domain
  AND constraint_id = 'outdoor_aqi_safe', constraint_stream = 'outdoor-air-quality',
      constraint_metric = 'aqi', condition = '<', threshold = 100, unit = 'AQI'
```

### AC-011: SyncReport output

```
WHEN I run `ndp domain sync` with 1 domain, 4 streams, 6 objectives, 0 constraints
THEN stdout displays:
  Domain sync complete:
    Domains synced:     1
    Streams mapped:     4
    Objectives synced:  6
    Constraints synced: 0
    Duration:           X.XXs
```

### AC-012: Unit tests pass without database

```
WHEN I run `cargo test -p ndp-lib`
THEN all domain sync unit tests pass using MockDbClient
  AND no real database connection is attempted
  AND tests verify: SQL statement content, parameter counts, execution order,
      transaction wrapping (BEGIN/COMMIT), FK-safe delete ordering, UPSERT syntax
```

---

## 4. Constraints and Assumptions

### 4.1 Technical Constraints

| Constraint | Rationale |
|------------|-----------|
| Must use `ndp-lib` crate for sync logic, `ndp-cli` for CLI command | ops-001 established this architecture; domain sync is the same class of operation as dictionary sync |
| Must use `DbClient` trait from `crates/ndp-lib/src/db.rs` | Enables London TDD with mocks; same trait used by dictionary and dimension sync |
| Must use parameterized SQL constants (not `format!()` with values) | Security requirement; matches dictionary/sql.rs pattern |
| Tables already exist via `005_domain_objectives.sql` | No DDL changes needed; this is a data population fix, not a schema change |
| `domain.json` config format is fixed (FE-002) | No changes to config schema; parse what exists |
| Rust edition 2021, async/await with tokio | Workspace-level constraint from Cargo.toml |
| `async-trait` crate required for `DbClient` impl | Existing workspace dependency |

### 4.2 Assumptions

| Assumption | Impact if Wrong |
|------------|----------------|
| Only one domain directory currently exists (`indoor-air-quality`) | If multiple domains exist, the sync must handle all of them. The design already iterates `config/domains/*/domain.json`. |
| The `constraints` section is optional in `domain.json` | If a domain has constraints, they will be synced. If absent, no rows are written. The current config has no constraints. |
| The `between` condition requires a threshold array `[min, max]` | If a between-condition objective appears, both `threshold` and `threshold_upper` must be extracted. The current config has no between-conditions. |
| `deploy.sh` can tolerate the `ndp` binary not being available | The fallback-to-warning pattern is already established at lines 384-392 and 1218-1226. |
| The `sync_status` table tracking (domains_synced, objectives_synced columns added by 005_domain_objectives.sql) is optional | The domain sync may update sync_status for observability but failure to do so is non-fatal, matching dictionary sync behavior. |

---

## 5. Dependencies on Existing Infrastructure

### 5.1 Direct Dependencies (Must Exist, Already Verified)

| Dependency | Location | Status |
|------------|----------|--------|
| `DbClient` trait (`query`, `execute`, `batch_execute`) | `crates/ndp-lib/src/db.rs` | Exists, lines 17-26 |
| `PostgresClient::connect()` | `crates/ndp-lib/src/db.rs` | Exists, lines 39-63 |
| `ConfigLoader` trait | `crates/ndp-lib/src/config.rs` | Exists, lines 18-24 |
| `FileSystemConfigLoader` | `crates/ndp-lib/src/config.rs` | Exists, lines 291-398 |
| `SyncReport`, `SyncOptions`, `SyncError` | `crates/ndp-lib/src/types.rs` | Exists |
| `NdpLibError` (Database, ConfigNotFound, ConfigParse variants) | `crates/ndp-lib/src/error.rs` | Exists |
| Entity/verb CLI routing (`Commands` enum) | `tools/ndp-cli/src/main.rs` | Exists, lines 42-49 |
| `command -v ndp` pattern in deploy.sh | `deploy/pi/deploy.sh` | Exists, lines 386, 1220 |
| Target tables (`005_domain_objectives.sql`) | `deploy/pi/init-scripts/005_domain_objectives.sql` | Exists, tables created idempotently |
| `domain.json` config file | `config/domains/indoor-air-quality/domain.json` | Exists, 112 lines, all fields present |

### 5.2 Reference Implementation

The `ndp dictionary sync` command is the exact pattern to follow:

| Layer | Dictionary (reference) | Domain (to build) |
|-------|----------------------|-------------------|
| Types | `dictionary/types.rs` (StreamDictionaryEntry, etc.) | `domain/types.rs` (DomainSyncEntry, ObjectiveSyncEntry, etc.) |
| SQL | `dictionary/sql.rs` (parameterized constants) | `domain/sql.rs` (UPSERT domains, DELETE+INSERT for children) |
| Logic | `dictionary/mod.rs` (sync_dictionary function) | `domain/mod.rs` (sync_domain function) |
| Convert | `convert.rs` (stream_config_to_dictionary_entry) | Not needed -- domain config maps directly to sync entries |
| CLI | `commands/dictionary.rs` (DictionaryArgs, run()) | `commands/domain.rs` (DomainArgs, run()) |
| Tests | `dictionary/mod.rs` tests (20 tests, MockDbClient) | `domain/mod.rs` tests (same pattern) |

---

## 6. Data Model

### 6.1 Config Source: `domain.json`

Path pattern: `config/domains/<domain_id>/domain.json`

```json
{
  "id": "indoor-air-quality",
  "description": "Maintain healthy indoor air quality",
  "streams": [
    {"stream_id": "air-quality", "alias": "indoor", "role": "primary"},
    {"stream_id": "outdoor-weather", "alias": "outdoor", "role": "context"},
    {"stream_id": "home-assistant-state", "alias": "state", "role": "actuator"},
    {"stream_id": "outdoor-air-quality", "alias": "outdoor_aqi", "role": "constraint"}
  ],
  "objectives": [
    {
      "id": "healthy_co2",
      "description": "Keep CO2 below 800 ppm for cognitive performance",
      "target": {
        "stream": "air-quality",
        "metric": "co2",
        "condition": "<",
        "threshold": 800,
        "unit": "ppm"
      },
      "priority": "high"
    }
  ],
  "constraints": []
}
```

Note: The `alignment` and `events` sections of `domain.json` are not consumed by domain sync. They are consumed by the Gold layer generators (`ndp-gold-ddl`).

### 6.2 Target Tables

Source: `deploy/pi/init-scripts/005_domain_objectives.sql`

**Table: `data_dictionary.domains`**

| Column | Type | Constraint | Source |
|--------|------|------------|--------|
| domain_id | TEXT | PRIMARY KEY | `config.id` |
| description | TEXT | | `config.description` |
| stream_count | INTEGER | | `config.streams.length` |
| config_path | TEXT | | Computed: `config/domains/<id>/domain.json` |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | Auto |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | Auto |

Sync strategy: UPSERT (ON CONFLICT (domain_id) DO UPDATE SET description, stream_count, config_path, updated_at).

**Table: `data_dictionary.domain_streams`**

| Column | Type | Constraint | Source |
|--------|------|------------|--------|
| domain_id | TEXT | FK -> domains, NOT NULL | Parent domain |
| stream_id | TEXT | NOT NULL | `streams[i].stream_id` |
| alias | TEXT | NOT NULL | `streams[i].alias` |
| role | TEXT | CHECK IN (primary, context, actuator, constraint), NOT NULL | `streams[i].role` |

PK: (domain_id, stream_id). Sync strategy: DELETE WHERE domain_id = $1, then INSERT.

**Table: `data_dictionary.objectives`**

| Column | Type | Constraint | Source |
|--------|------|------------|--------|
| objective_id | TEXT | NOT NULL | `objectives[i].id` |
| domain_id | TEXT | FK -> domains, NOT NULL | Parent domain |
| description | TEXT | | `objectives[i].description` |
| target_stream | TEXT | NOT NULL | `objectives[i].target.stream` |
| target_metric | TEXT | NOT NULL | `objectives[i].target.metric` |
| condition | TEXT | CHECK IN (<, >, <=, >=, ==, !=, between), NOT NULL | `objectives[i].target.condition` |
| threshold | NUMERIC | NOT NULL | `objectives[i].target.threshold` (or `threshold[0]` for between) |
| threshold_upper | NUMERIC | | `objectives[i].target.threshold[1]` for between, NULL otherwise |
| unit | TEXT | | `objectives[i].target.unit` |
| priority | TEXT | CHECK IN (low, medium, high, critical), DEFAULT 'medium', NOT NULL | `objectives[i].priority` |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | Auto |
| updated_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | Auto |

PK: (domain_id, objective_id). Sync strategy: DELETE WHERE domain_id = $1, then INSERT.

**Table: `data_dictionary.constraints`**

| Column | Type | Constraint | Source |
|--------|------|------------|--------|
| constraint_id | TEXT | NOT NULL | `constraints[i].id` |
| domain_id | TEXT | FK -> domains, NOT NULL | Parent domain |
| description | TEXT | | `constraints[i].description` |
| constraint_stream | TEXT | NOT NULL | `constraints[i].stream` |
| constraint_metric | TEXT | NOT NULL | `constraints[i].metric` |
| condition | TEXT | CHECK IN (<, >, <=, >=, ==, !=), NOT NULL | `constraints[i].condition` |
| threshold | NUMERIC | NOT NULL | `constraints[i].threshold` |
| unit | TEXT | | `constraints[i].unit` |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | Auto |

PK: (domain_id, constraint_id). Sync strategy: DELETE WHERE domain_id = $1, then INSERT.

### 6.3 Sync Execution Order

Within a single BEGIN/COMMIT transaction:

1. BEGIN
2. For each domain config:
   a. UPSERT into `data_dictionary.domains`
   b. DELETE FROM `data_dictionary.domain_streams` WHERE domain_id = $1
   c. INSERT INTO `data_dictionary.domain_streams` (one per stream)
   d. DELETE FROM `data_dictionary.objectives` WHERE domain_id = $1
   e. INSERT INTO `data_dictionary.objectives` (one per objective)
   f. DELETE FROM `data_dictionary.constraints` WHERE domain_id = $1
   g. INSERT INTO `data_dictionary.constraints` (one per constraint)
3. COMMIT

Note: `domain_streams`, `objectives`, and `constraints` all have FK CASCADE from `domains`, so the per-domain DELETE+INSERT within the same transaction is safe. The UPSERT on `domains` ensures the parent row exists before child inserts.

---

## 7. Deliverables

### 7.1 New Files

| # | File | Purpose |
|---|------|---------|
| 1 | `crates/ndp-lib/src/domain/mod.rs` | `sync_domain()` function + London TDD tests with MockDbClient |
| 2 | `crates/ndp-lib/src/domain/types.rs` | `DomainSyncEntry`, `DomainStreamEntry`, `ObjectiveSyncEntry`, `ConstraintSyncEntry` structs |
| 3 | `crates/ndp-lib/src/domain/sql.rs` | Parameterized SQL constants (UPSERT_DOMAIN, DELETE_DOMAIN_STREAMS, INSERT_DOMAIN_STREAM, DELETE_OBJECTIVES, INSERT_OBJECTIVE, DELETE_CONSTRAINTS, INSERT_CONSTRAINT) |
| 4 | `tools/ndp-cli/src/commands/domain.rs` | CLI command: load configs, convert, sync, print SyncReport |

### 7.2 Modified Files

| # | File | Change |
|---|------|--------|
| 5 | `crates/ndp-lib/src/lib.rs` | Add `pub mod domain;` |
| 6 | `crates/ndp-lib/src/config.rs` | Add `load_domain_configs() -> Result<Vec<DomainConfig>>` to `ConfigLoader` trait + `DomainConfig` struct for domain.json + `FileSystemConfigLoader` impl |
| 7 | `tools/ndp-cli/src/commands/mod.rs` | Add `pub mod domain;` |
| 8 | `tools/ndp-cli/src/main.rs` | Add `Domain(commands::domain::DomainArgs)` variant to `Commands` enum + match arm |
| 9 | `deploy/pi/deploy.sh` | Replace body of `sync_domains_to_data_dictionary()` with `command -v ndp` pattern calling `ndp domain sync` |

---

## 8. Out of Scope

| Item | Rationale |
|------|-----------|
| `--apply` flag on `ndp-gold-ddl` | `ndp-gold-ddl` generates DDL (CREATE VIEW, CREATE PROCEDURE). Domain sync is data population (INSERT/UPSERT into metadata tables). Different tool, different concern. |
| `--objectives` flag on `ndp-gold-ddl` | Wrong tool. `ndp-gold-ddl`'s `DbClient` only has `query()` (read-only). `ndp-lib`'s `DbClient` has `execute()` for writes. |
| Changes to `domain.json` config format | The config already contains all fields needed (id, description, streams, objectives). No schema changes. |
| Changes to `005_domain_objectives.sql` init script | Tables already exist with correct schemas. This is a data population fix, not a schema migration. |
| Multi-domain discovery beyond `config/domains/*/domain.json` | Only one domain currently exists. The glob pattern handles multiple domains naturally. |
| etcd-based config loading | V1.3 scope. The `ConfigLoader` trait already abstracts the source; `FileSystemConfigLoader` is the V1.1 implementation. |
| Sync of `alignment` or `events` config sections | These sections are consumed by `ndp-gold-ddl` generators, not by the data dictionary sync. |
| Removal of the old Bash function body | The function body will be replaced with the `command -v ndp` pattern. The old YAML-based code will be deleted as part of the replacement (not preserved behind a flag). |
| `sync_status` table update for domain sync counts | Optional enhancement. The `domains_synced` and `objectives_synced` columns exist (added by 005_domain_objectives.sql) but updating them is non-critical and can be added as a follow-up. |

---

## 9. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ndp` binary not available on Pi at deploy time | Medium | Medium | Use established `command -v ndp` fallback pattern with warning. Deploy.sh already handles this for dictionary and dimension sync. |
| Domain config has unexpected JSON shape | Low | Medium | Parse with `serde_json::from_str` with explicit struct fields + `#[serde(default)]` for optional sections. Unit test against real `indoor-air-quality/domain.json` via `include_str!()`. |
| `between` condition objective never tested with real data | Medium | Low | Current config has no between-conditions. Add unit test with synthetic between-condition config. The Bash code (lines 1006-1012) already had this logic, so the schema supports it. |
| FK constraint violation if `domains` row doesn't exist before child inserts | Low | High | Execution order ensures UPSERT on `domains` runs before child DELETE+INSERT. Transaction wrapping ensures atomicity. |
| Future domains added without `objectives` or `constraints` sections | Medium | Low | Both sections are `#[serde(default)]` (empty Vec). Sync handles empty arrays gracefully (DELETE existing, INSERT nothing). |
| deploy.sh regression -- existing callers of `sync_domains_to_data_dictionary()` break | Low | Medium | Function signature is unchanged. Only the body changes. Two call sites: line 2290 (during domain deployment) and line 2817 (sync-domains subcommand). Both pass no arguments. |
| Concurrent deploys cause transaction conflicts | Very Low | Low | Single-Pi deployment model. Only one deploy runs at a time. Transaction wrapping ensures consistency. |

---

## 10. Test Plan Summary

### 10.1 Unit Tests (MockDbClient, no DB required)

| Test | Validates |
|------|-----------|
| test_sync_empty_domains | Empty input produces BEGIN/COMMIT, zero counts |
| test_sync_single_domain_upsert | `data_dictionary.domains` INSERT uses ON CONFLICT |
| test_sync_domain_streams_delete_insert | DELETE before INSERT for domain_streams, per domain |
| test_sync_objectives_delete_insert | DELETE before INSERT for objectives, per domain |
| test_sync_constraints_delete_insert | DELETE before INSERT for constraints, per domain |
| test_sync_objectives_between_condition | threshold and threshold_upper both populated for between |
| test_sync_objectives_single_condition | threshold_upper is NULL for non-between conditions |
| test_sync_constraints_empty | Empty constraints array produces DELETE but no INSERT |
| test_sync_parameterized_sql | All INSERT/UPSERT SQL contains $1, $2, etc. (no string interpolation) |
| test_sync_transaction_wrapping | First query is BEGIN, last query is COMMIT |
| test_sync_execution_order | UPSERT domain before DELETE+INSERT children; children in correct FK order |
| test_sync_report_counts | SyncReport.items_processed, items_created, items_updated match expected |
| test_dry_run_no_sql | Dry run mode executes zero SQL statements |
| test_multi_domain_sync | Two domains produce correct per-domain DELETE+INSERT sequences |
| test_real_config_parse | `include_str!()` on real `indoor-air-quality/domain.json` parses correctly |

### 10.2 Integration Tests (requires docker-compose.integration.yml)

| Test | Validates |
|------|-----------|
| Sync against live TimescaleDB | All 4 tables populated with correct data |
| Idempotent re-run | Second sync produces identical state |
| Query `v_domain_overview` view | View returns correct counts after sync |
| Query `get_objectives_for_stream()` function | Returns objectives for air-quality stream |

---

## 11. Success Metrics

| Metric | Target |
|--------|--------|
| All existing tests pass (`cargo test` workspace-wide) | 616+ tests green |
| New unit tests added | >= 15 tests in `domain/mod.rs` |
| New unit tests for config loading | >= 2 tests in `config.rs` |
| `data_dictionary.domains` populated after deploy | 1 row (indoor-air-quality) |
| `data_dictionary.domain_streams` populated after deploy | 4 rows |
| `data_dictionary.objectives` populated after deploy | 6 rows |
| `data_dictionary.constraints` populated after deploy | 0 rows (current config) |
| Bash dead code removed | ~200 lines of YAML-based sync replaced with ~15 lines of `command -v ndp` pattern |
| deploy.sh warning eliminated | No more "No domain.yaml files found" on deploy |
