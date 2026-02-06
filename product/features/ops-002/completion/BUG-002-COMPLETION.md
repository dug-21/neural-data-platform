# BUG-002 Completion: Domain Objectives Sync (`ndp domain sync`)

> **Bug:** BUG-002 (objectives-sync-not-migrated)
> **Feature:** ops-002
> **Release:** v1.1.12 (PATCH)
> **SPARC Phase:** Completion
> **Created:** 2026-02-06
> **Author:** ndp-scrum-master

---

## 1. Definition of Done

ALL criteria must pass before v1.1.12 is released.

### 1.1 Functional Completeness

- [ ] `ndp domain sync` command exists and is functional
- [ ] `ndp domain sync --dry-run` prints planned operations without executing
- [ ] 4 target tables populated: `data_dictionary.domains`, `data_dictionary.domain_streams`, `data_dictionary.objectives`, `data_dictionary.constraints`
- [ ] All SQL uses parameterized queries (`$1`, `$2`, ...) -- no string concatenation
- [ ] Transaction-wrapped (`BEGIN` / `COMMIT`)
- [ ] UPSERT for `domains` table (idempotent re-runs)
- [ ] DELETE+INSERT per domain for child tables (`domain_streams`, `objectives`, `constraints`)
- [ ] `SyncReport` returned with accurate counts (domains processed, streams/objectives/constraints created)
- [ ] ConfigLoader trait extended with `load_domain_configs() -> Result<Vec<DomainConfig>>`
- [ ] `FileSystemConfigLoader` discovers `config/domains/*/domain.json`
- [ ] Conversion from `DomainConfig` -> `DomainSyncEntry` tested

### 1.2 Test Compliance

- [ ] 18+ new London TDD unit tests pass using `MockDbClient`
- [ ] `cargo test -p ndp-lib -- domain` passes all domain module tests
- [ ] `cargo test --workspace` passes (all existing 616 + new ~20 = ~636)
- [ ] `cargo clippy --workspace` clean (no new warnings)
- [ ] Integration test passes against `docker-compose.integration.yml`

### 1.3 deploy.sh Integration

- [ ] `sync_domains_to_data_dictionary()` replaced with `command -v ndp` pattern
- [ ] Fallback is no-op (same as current dead-code behavior)
- [ ] Pattern matches existing `command -v ndp` usage at deploy.sh lines 386, 1220

### 1.4 Release Artifacts

- [ ] Manifest created: `.deploy/releases/v1.1.12.manifest.json`
- [ ] `CHANGELOG.md` updated with v1.1.12 entry
- [ ] Git tag: `v1.1.12` (annotated)
- [ ] BUG-002 status in STATUS.md updated to FIXED
- [ ] ops-002 STATUS.md updated

---

## 2. Implementation Order

Implementation follows London TDD: write test first, then implementation to make it pass.

### Phase 1: Domain Types (`crates/ndp-lib/src/domain/types.rs`)

Define entry structs that mirror the 4 target tables. These are the "parsed structs, not file paths" that the sync function accepts.

```
DomainSyncEntry
  domain_id: String
  description: Option<String>
  stream_count: i32
  config_path: String
  streams: Vec<DomainStreamEntry>
  objectives: Vec<ObjectiveSyncEntry>
  constraints: Vec<ConstraintSyncEntry>

DomainStreamEntry
  stream_id: String
  alias: String
  role: String            // "primary" | "context" | "actuator" | "constraint"

ObjectiveSyncEntry
  objective_id: String
  description: Option<String>
  target_stream: String
  target_metric: String
  condition: String       // "<" | ">" | "<=" | ">=" | "==" | "!=" | "between"
  threshold: f64
  threshold_upper: Option<f64>
  unit: Option<String>
  priority: String        // "low" | "medium" | "high" | "critical"

ConstraintSyncEntry
  constraint_id: String
  description: Option<String>
  constraint_stream: String
  constraint_metric: String
  condition: String
  threshold: f64
  unit: Option<String>
```

### Phase 2: SQL Constants (`crates/ndp-lib/src/domain/sql.rs`)

All SQL as `pub const` string constants with parameterized placeholders.

| Constant | SQL Pattern | Params |
|----------|------------|--------|
| `UPSERT_DOMAIN` | `INSERT INTO data_dictionary.domains ... ON CONFLICT (domain_id) DO UPDATE SET ...` | `$1` domain_id, `$2` description, `$3` stream_count, `$4` config_path |
| `DELETE_DOMAIN_STREAMS` | `DELETE FROM data_dictionary.domain_streams WHERE domain_id = $1` | `$1` domain_id |
| `INSERT_DOMAIN_STREAM` | `INSERT INTO data_dictionary.domain_streams (domain_id, stream_id, alias, role) VALUES ($1, $2, $3, $4)` | `$1`-`$4` |
| `DELETE_OBJECTIVES` | `DELETE FROM data_dictionary.objectives WHERE domain_id = $1` | `$1` domain_id |
| `INSERT_OBJECTIVE` | `INSERT INTO data_dictionary.objectives (objective_id, domain_id, description, target_stream, target_metric, condition, threshold, threshold_upper, unit, priority) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)` | `$1`-`$10` |
| `DELETE_CONSTRAINTS` | `DELETE FROM data_dictionary.constraints WHERE domain_id = $1` | `$1` domain_id |
| `INSERT_CONSTRAINT` | `INSERT INTO data_dictionary.constraints (constraint_id, domain_id, description, constraint_stream, constraint_metric, condition, threshold, unit) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)` | `$1`-`$8` |

FK-safe delete order within a domain: constraints -> objectives -> domain_streams (then UPSERT domain).

### Phase 3: Sync Function (`crates/ndp-lib/src/domain/mod.rs`)

```
pub async fn sync_domain(
    entries: &[DomainSyncEntry],
    db: &impl DbClient,
    options: &SyncOptions,
) -> Result<SyncReport>
```

Algorithm:
1. If `options.dry_run`, return counts without executing SQL
2. `BEGIN` transaction
3. For each `DomainSyncEntry`:
   a. DELETE constraints for this domain_id
   b. DELETE objectives for this domain_id
   c. DELETE domain_streams for this domain_id
   d. UPSERT domain row
   e. INSERT domain_streams
   f. INSERT objectives
   g. INSERT constraints
4. `COMMIT` transaction
5. Return `SyncReport` with counts

Tests (London TDD, MockDbClient):
1. `test_sync_empty_domains` -- no entries, still BEGIN/COMMIT
2. `test_sync_single_domain_upsert` -- domains table uses ON CONFLICT
3. `test_sync_domain_streams_inserted` -- 4 stream entries for indoor-air-quality
4. `test_sync_objectives_inserted` -- 6 objectives for indoor-air-quality
5. `test_sync_constraints_empty` -- 0 constraints (none in current config)
6. `test_sync_deletes_before_inserts` -- FK-safe ordering verified
7. `test_sync_transaction_wrapping` -- BEGIN first, COMMIT last
8. `test_sync_parameterized_queries` -- all SQL uses `$N` placeholders, no string concat
9. `test_sync_report_counts` -- items_processed, items_created accurate
10. `test_dry_run_no_sql_executed` -- MockDbClient records zero calls
11. `test_dry_run_returns_counts` -- report has correct counts
12. `test_sync_multiple_domains` -- two domains processed independently
13. `test_sync_idempotent` -- running twice produces same result (UPSERT + DELETE+INSERT)
14. `test_sync_objective_with_threshold_upper` -- between condition uses threshold_upper
15. `test_sync_objective_all_conditions` -- each condition operator (`<`, `>`, `<=`, `>=`, `==`, `!=`, `between`) inserted correctly
16. `test_sync_constraint_inserted` -- constraint with all fields
17. `test_sync_domain_stream_roles` -- all 4 roles (primary, context, actuator, constraint) inserted
18. `test_domain_sync_entry_from_domain_config` -- conversion produces correct entries

### Phase 4: ConfigLoader Extension (`crates/ndp-lib/src/config.rs`)

Add to `ConfigLoader` trait:
```rust
fn load_domain_configs(&self) -> Result<Vec<DomainConfig>>;
```

Add `DomainConfig` struct (serde-deserializable from `domain.json`):
```rust
pub struct DomainConfig {
    pub id: String,
    pub description: Option<String>,
    pub streams: Vec<DomainStreamConfig>,
    pub alignment: Option<serde_json::Value>,
    pub events: Option<serde_json::Value>,
    pub objectives: Vec<ObjectiveConfig>,
    // constraints field: Vec<ConstraintConfig> (absent in current config, default empty)
}
```

`FileSystemConfigLoader` implementation:
- Constructor gains `domains_dir: PathBuf` (add field, update `from_base_dir` to derive `base.join("domains")`)
- `load_domain_configs()` discovers `<domains_dir>/*/domain.json` (same glob pattern as streams)
- Note: `FileSystemConfigLoader` currently has `streams_dir` and `dimensions_dir`. Adding `domains_dir` requires updating the constructor. Use `from_base_dir` which already computes paths from a base -- add `base.join("domains")`.

Backward compatibility: the new `load_domain_configs()` method must have a default implementation on the trait that returns `Err(NdpLibError::ConfigNotFound)` or an empty vec, so existing implementors (mocks) are not broken. Alternatively, add it as a new trait method with a default impl.

### Phase 5: Conversion (`crates/ndp-lib/src/convert.rs` or `domain/convert.rs`)

```rust
pub fn domain_config_to_sync_entry(config: &DomainConfig, config_path: &str) -> DomainSyncEntry
```

Maps:
- `config.id` -> `domain_id`
- `config.description` -> `description`
- `config.streams.len()` -> `stream_count`
- `config_path` -> `config_path`
- `config.streams[*]` -> `DomainStreamEntry` (direct field mapping)
- `config.objectives[*]` -> `ObjectiveSyncEntry` (flatten `target` sub-object)
- `config.constraints[*]` -> `ConstraintSyncEntry` (if present, default empty vec)

### Phase 6: CLI Command (`tools/ndp-cli/src/commands/domain.rs`)

Follow the `dictionary.rs` reference implementation exactly:

```rust
#[derive(Args)]
pub struct DomainArgs {
    #[command(subcommand)]
    pub command: DomainCommands,
}

#[derive(Subcommand)]
pub enum DomainCommands {
    Sync {
        #[arg(long)]
        config_dir: Option<PathBuf>,  // override domains dir
        #[arg(long)]
        dry_run: bool,
    },
}
```

Handler:
1. Construct `FileSystemConfigLoader` from `base_config_dir`
2. Call `loader.load_domain_configs()`
3. Convert each `DomainConfig` -> `DomainSyncEntry` via `domain_config_to_sync_entry()`
4. If dry_run: print planned operations, return
5. Connect to DB via `PostgresClient::connect()`
6. Call `ndp_lib::domain::sync_domain(&entries, &db, &options)`
7. Print `SyncReport`

### Phase 7: CLI Wiring (`tools/ndp-cli/src/main.rs` + `commands/mod.rs`)

Add `Domain` variant to `Commands` enum:
```rust
enum Commands {
    Dictionary(commands::dictionary::DictionaryArgs),
    Dimension(commands::dimension::DimensionArgs),
    Domain(commands::domain::DomainArgs),  // NEW
}
```

Add match arm in `main()`:
```rust
Commands::Domain(args) => {
    commands::domain::run(args, &config_dir, &db_url).await?;
}
```

Add `pub mod domain;` to `commands/mod.rs`.

### Phase 8: deploy.sh Integration

Replace `sync_domains_to_data_dictionary()` call (lines 883-1086) with the `command -v ndp` pattern:

```bash
if command -v ndp &>/dev/null; then
    log_info "Syncing domain configs via ndp domain sync..."
    ndp domain sync --db-url "$TIMESCALE_URL" --config-dir "$CONFIG_DIR" || {
        log_warn "ndp domain sync failed (non-fatal)"
    }
else
    log_warn "ndp not found, skipping domain sync"
fi
```

This replaces ~200 lines of dead Bash code with 7 lines. The fallback (ndp not found) is a no-op, which is identical to the current behavior since the Bash function never executed successfully.

### Phase 9: Integration Test

Run against `docker-compose.integration.yml` to verify end-to-end.

### Phase 10: Release Artifacts (v1.1.12)

Manifest, CHANGELOG, git tag.

---

## 3. Verification Procedure

### 3.1 Unit Tests

```bash
cargo test -p ndp-lib -- domain --nocapture
```

**Expected:** 18+ tests pass. All use `MockDbClient` with no database required.

### 3.2 Workspace Tests

```bash
cargo test --workspace
```

**Expected:** All existing 616 tests + ~20 new domain tests = ~636 total, zero failures.

### 3.3 Clippy

```bash
cargo clippy -p ndp-lib -p ndp-cli -- -D warnings
```

**Expected:** Zero new warnings.

### 3.4 Integration Test

```bash
# Start integration environment
docker compose -f docker-compose.integration.yml up -d

# Wait for TimescaleDB readiness
until docker compose -f docker-compose.integration.yml exec timescaledb \
  pg_isready -U postgres -d ndp; do sleep 2; done

# Run init scripts (ensure domain tables exist)
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -f /docker-entrypoint-initdb.d/005_domain_objectives.sql

# Run domain sync
cargo run -p ndp -- domain sync \
  --env integration \
  --db-url "postgresql://postgres:postgres@localhost:5432/ndp"

# Verify domains table
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c "SELECT domain_id, description, stream_count FROM data_dictionary.domains;"
```

**Expected output:**
```
      domain_id       |            description            | stream_count
----------------------+-----------------------------------+--------------
 indoor-air-quality   | Maintain healthy indoor air quality |            4
(1 row)
```

```bash
# Verify domain_streams table
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c "SELECT domain_id, stream_id, alias, role FROM data_dictionary.domain_streams ORDER BY stream_id;"
```

**Expected output:**
```
      domain_id       |      stream_id       |    alias    |   role
----------------------+----------------------+-------------+-----------
 indoor-air-quality   | air-quality          | indoor      | primary
 indoor-air-quality   | home-assistant-state | state       | actuator
 indoor-air-quality   | outdoor-air-quality  | outdoor_aqi | constraint
 indoor-air-quality   | outdoor-weather      | outdoor     | context
(4 rows)
```

```bash
# Verify objectives table
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c "SELECT objective_id, target_stream, target_metric, condition, threshold, unit, priority FROM data_dictionary.objectives ORDER BY objective_id;"
```

**Expected output:**
```
        objective_id         | target_stream | target_metric | condition | threshold | unit    | priority
-----------------------------+---------------+---------------+-----------+-----------+---------+----------
 comfortable_humidity_max    | air-quality   | humidity_pct  | <=        |        60 | percent | medium
 comfortable_humidity_min    | air-quality   | humidity_pct  | >=        |        40 | percent | medium
 comfortable_temperature_max | air-quality   | temperature_c | <=        |        24 | celsius | medium
 comfortable_temperature_min | air-quality   | temperature_c | >=        |        20 | celsius | medium
 healthy_co2                 | air-quality   | co2           | <         |       800 | ppm     | high
 healthy_pm25                | air-quality   | pm25          | <         |        12 | ug/m3   | high
(6 rows)
```

```bash
# Verify constraints table (should be empty -- none defined in current config)
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c "SELECT COUNT(*) FROM data_dictionary.constraints;"
```

**Expected output:**
```
 count
-------
     0
(1 row)
```

### 3.5 Dry Run Verification

```bash
cargo run -p ndp -- domain sync --dry-run
```

**Expected:** Prints planned operations (1 domain, 4 streams, 6 objectives, 0 constraints) without touching the database.

### 3.6 Idempotency Verification

```bash
# Run sync twice -- second run should succeed with same results
cargo run -p ndp -- domain sync \
  --db-url "postgresql://postgres:postgres@localhost:5432/ndp"
cargo run -p ndp -- domain sync \
  --db-url "postgresql://postgres:postgres@localhost:5432/ndp"

# Verify counts unchanged
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c "SELECT * FROM data_dictionary.v_domain_overview;"
```

**Expected:** 1 domain, 4 streams, 6 objectives, 0 constraints after both runs.

### 3.7 deploy.sh Verification

```bash
grep -A10 "sync_domains" deploy/pi/deploy.sh
```

**Expected:** Shows `command -v ndp` pattern, not the old YAML-parsing Bash function.

---

## 4. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| ConfigLoader trait change breaks existing code | Low | Medium | Add `load_domain_configs()` with a default impl returning empty vec; existing trait impls unaffected |
| FileSystemConfigLoader constructor change breaks callers | Low | Medium | Add `domains_dir` field with backward-compatible `from_base_dir()` that auto-derives `base.join("domains")` |
| deploy.sh change breaks deploy | Low | Low | Current function is dead code (searches for `domain.yaml`, files are `domain.json`). Replacing dead code with working code is net-positive. Fallback is no-op. |
| Schema changes needed | None | N/A | Tables already exist from `005_domain_objectives.sql`. No DDL changes. |
| `domain.json` format changes | None | N/A | Config already has all fields (`id`, `streams`, `objectives`). No config changes needed. |
| Constraint sync untested in integration | Low | Low | Unit tests cover constraint insertion. Integration returns 0 rows as expected since no constraints are defined in current config. |

---

## 5. File Manifest

New files:

| File | Purpose |
|------|---------|
| `crates/ndp-lib/src/domain/mod.rs` | `sync_domain()` function + London TDD tests |
| `crates/ndp-lib/src/domain/types.rs` | `DomainSyncEntry`, `ObjectiveSyncEntry`, `ConstraintSyncEntry`, `DomainStreamEntry` |
| `crates/ndp-lib/src/domain/sql.rs` | Parameterized SQL constants (UPSERT_DOMAIN, INSERT_OBJECTIVE, etc.) |
| `tools/ndp-cli/src/commands/domain.rs` | CLI command handler for `ndp domain sync` |

Modified files:

| File | Change |
|------|--------|
| `crates/ndp-lib/src/lib.rs` | Add `pub mod domain;` |
| `crates/ndp-lib/src/config.rs` | Add `DomainConfig` struct, `load_domain_configs()` to `ConfigLoader` trait, `FileSystemConfigLoader` domains discovery |
| `crates/ndp-lib/src/convert.rs` | Add `domain_config_to_sync_entry()` |
| `tools/ndp-cli/src/main.rs` | Add `Domain` variant to `Commands` enum and match arm |
| `tools/ndp-cli/src/commands/mod.rs` | Add `pub mod domain;` |
| `deploy/pi/deploy.sh` | Replace `sync_domains_to_data_dictionary()` with `command -v ndp` pattern |

Unchanged files (verified compatible):

| File | Reason |
|------|--------|
| `deploy/pi/init-scripts/005_domain_objectives.sql` | Tables already exist with correct schema |
| `config/domains/indoor-air-quality/domain.json` | Config already has all required fields |

---

## 6. Release Checklist (v1.1.12)

- [ ] All tests pass (existing 616 + new ~20 = ~636)
- [ ] Clippy clean on modified crates
- [ ] Integration test verified against `docker-compose.integration.yml`
- [ ] Manifest created: `.deploy/releases/v1.1.12.manifest.json`
- [ ] `CHANGELOG.md` updated with v1.1.12 entry
- [ ] Git tag: `v1.1.12` (annotated): `git tag -a v1.1.12 -m "fix(v1.1.12): domain objectives sync via ndp domain sync (BUG-002)"`
- [ ] BUG-002 status updated to FIXED in `product/features/ops-002/bugs/BUG-002-objectives-sync-not-migrated.md`
- [ ] ops-002 STATUS.md bug table updated: BUG-002 -> FIXED
- [ ] Reflexion recorded for all participating agents

---

## 7. Reference Implementation

The `ndp dictionary sync` implementation is the direct reference for this work:

| Component | Dictionary (reference) | Domain (this bug) |
|-----------|----------------------|-------------------|
| Entry types | `crates/ndp-lib/src/dictionary/types.rs` | `crates/ndp-lib/src/domain/types.rs` |
| SQL constants | `crates/ndp-lib/src/dictionary/sql.rs` | `crates/ndp-lib/src/domain/sql.rs` |
| Sync function | `crates/ndp-lib/src/dictionary/mod.rs` | `crates/ndp-lib/src/domain/mod.rs` |
| CLI command | `tools/ndp-cli/src/commands/dictionary.rs` | `tools/ndp-cli/src/commands/domain.rs` |
| Conversion | `crates/ndp-lib/src/convert.rs` | `crates/ndp-lib/src/convert.rs` (extend) |
| Test pattern | `MockDbClient` + London TDD (20 tests) | `MockDbClient` + London TDD (18+ tests) |
| deploy.sh | `command -v ndp` at line 1220 | `command -v ndp` replacing lines 883-1086 |

The sync function signature, error handling, transaction wrapping, and `SyncReport` return follow the dictionary sync pattern exactly.

---

## 8. Expected Data Counts (indoor-air-quality domain)

Derived from `config/domains/indoor-air-quality/domain.json`:

| Table | Count | Details |
|-------|-------|---------|
| `data_dictionary.domains` | 1 | `indoor-air-quality` |
| `data_dictionary.domain_streams` | 4 | air-quality (primary), outdoor-weather (context), home-assistant-state (actuator), outdoor-air-quality (constraint) |
| `data_dictionary.objectives` | 6 | healthy_co2, healthy_pm25, comfortable_humidity_min, comfortable_humidity_max, comfortable_temperature_min, comfortable_temperature_max |
| `data_dictionary.constraints` | 0 | None defined in current config |
| **Total rows** | **11** | |
