# OPS-003 Phase 1 Specification: Gold Migration (v1.1.14)

> **Feature:** ops-003 Phase 1
> **Release:** v1.1.14
> **Created:** 2026-02-07
> **Status:** Specification
> **Specification Agent:** SPARC Specification Phase

---

## 1. Executive Summary

This specification defines the exact contract for migrating Gold DDL generation from `tools/ndp-gold-ddl/src/` into `crates/ndp-lib/src/gold/`, exposing it via `ndp gold` CLI subcommands, and switching 2 deploy.sh dispatch sites from `ndp-gold-ddl` to `ndp`. After v1.1.14, `ndp gold generate|sync|recreate` is the sole path for Gold DDL in deploy.sh.

---

## 2. Source Inventory

### 2.1 Files to Migrate from `tools/ndp-gold-ddl/src/`

Every `.rs` file below moves into `crates/ndp-lib/src/gold/` unless marked "stays".

| # | Source Path | Lines | Destination in ndp-lib | Notes |
|---|-------------|-------|----------------------|-------|
| 1 | `config/mod.rs` | 18 | `gold/config/mod.rs` | Re-exports |
| 2 | `config/types.rs` | 492 | `gold/config/types.rs` | StreamConfig, GoldEtlConfig, Action, VALID_METRICS, VALID_ROLLING_STATS |
| 3 | `config/domain.rs` | 492 | `gold/config/domain.rs` | DomainConfig, AlignmentConfig, ObjectiveConfig, StreamRef |
| 4 | `config/loader.rs` | 614 | `gold/config/loader.rs` | ConfigLoader trait, FileSystemConfigLoader |
| 5 | `db/mod.rs` | 10 | **stays** (already in ndp-lib) | ndp-lib has its own db module |
| 6 | `db/client.rs` | 132 | **stays** (duplicate) | ndp-lib already has DbClient. Wire CaChecker to ndp-lib's trait. |
| 7 | `db/queries.rs` | 178 | `gold/db.rs` | CaChecker, CaInfo, PostgresCaChecker (uses `ndp_lib::DbClient`) |
| 8 | `error.rs` | 130 | `gold/error.rs` | GoldDdlError, ErrorCode, Result |
| 9 | `generators/mod.rs` | 37 | `gold/generators/mod.rs` | Re-exports |
| 10 | `generators/continuous_aggregate.rs` | 614 | `gold/generators/continuous_aggregate.rs` | ContinuousAggregateGenerator |
| 11 | `generators/aligned_view.rs` | 912 | `gold/generators/aligned_view.rs` | AlignedViewGenerator |
| 12 | `generators/state_transitions.rs` | 998 | `gold/generators/state_transitions.rs` | StateTransitionGenerator |
| 13 | `generators/events.rs` | 2358 | `gold/generators/events.rs` | EventsGenerator |
| 14 | `generators/refresh_policy.rs` | 175 | `gold/generators/refresh_policy.rs` | RefreshPolicyGenerator |
| 15 | `generators/column_builder.rs` | 267 | `gold/generators/column_builder.rs` | Column SQL builder |
| 16 | `generators/join_builder.rs` | 353 | `gold/generators/join_builder.rs` | JOIN clause builder |
| 17 | `generators/null_handler.rs` | 182 | `gold/generators/null_handler.rs` | NULL handling SQL |
| 18 | `generators/constants.rs` | 13 | `gold/generators/constants.rs` | Generator constants |
| 19 | `generators/classification.rs` | 372 | `gold/generators/classification.rs` | Classification syncer |
| 20 | `planner/mod.rs` | 8 | `gold/planner/mod.rs` | Re-exports |
| 21 | `planner/sync.rs` | 500 | `gold/planner/sync.rs` | SyncPlanner, SyncPlan, CaAction, CaPlan |
| 22 | `registry/mod.rs` | 222 | `gold/registry/mod.rs` | FeatureRegistry |
| 23 | `registry/trait_def.rs` | 165 | `gold/registry/trait_def.rs` | FeatureGenerator trait |
| 24 | `registry/lag.rs` | 174 | `gold/registry/lag.rs` | LagFeature |
| 25 | `registry/rolling.rs` | 240 | `gold/registry/rolling.rs` | RollingFeature |
| 26 | `registry/trend.rs` | 183 | `gold/registry/trend.rs` | TrendFeature |
| 27 | `validation/mod.rs` | 9 | `gold/validation/mod.rs` | Re-exports |
| 28 | `validation/config_validator.rs` | 555 | `gold/validation/config_validator.rs` | ConfigValidator, parse_granularity, parse_window, granularity_to_suffix |
| 29 | `lib.rs` | 57 | `gold/mod.rs` | Becomes the gold module root with public API |
| 30 | `main.rs` | 330 | **stays** in ndp-gold-ddl (thin wrapper) | Rewired to call `ndp_lib::gold::*` |

**Totals:**
- Source files moved: 28 (files #1-4, #7-29)
- Source files staying/duplicated: 2 (files #5, #6)
- Source lines moved: ~9,919 (total 10,273 minus 354 for db/mod.rs, db/client.rs, main.rs which stay)
- main.rs rewritten: 1 (thin wrapper, ~50 lines)

### 2.2 Integration Test Files to Migrate

| # | Source Path | Lines | Destination | Notes |
|---|-------------|-------|-------------|-------|
| 1 | `tests/aligned_view_tests.rs` | 811 | `crates/ndp-lib/tests/gold/aligned_view_tests.rs` | |
| 2 | `tests/golden_master_test.rs` | 668 | `crates/ndp-lib/tests/gold/golden_master_test.rs` | |
| 3 | `tests/objectives_tests.rs` | 709 | `crates/ndp-lib/tests/gold/objectives_tests.rs` | |
| 4 | `tests/state_transitions_tests.rs` | 629 | `crates/ndp-lib/tests/gold/state_transitions_tests.rs` | |
| 5 | `tests/ops002_config_driven_tests.rs` | 193 | `crates/ndp-lib/tests/gold/ops002_config_driven_tests.rs` | |
| 6 | `tests/ops002_hardcoding_tests.rs` | 299 | `crates/ndp-lib/tests/gold/ops002_hardcoding_tests.rs` | |
| 7 | `tests/ops002_source_scan_tests.rs` | 197 | `crates/ndp-lib/tests/gold/ops002_source_scan_tests.rs` | |
| 8 | `tests/fixtures/mod.rs` | 21 | `crates/ndp-lib/tests/gold/fixtures/mod.rs` | |
| 9 | `tests/fixtures/energy_monitoring.rs` | 334 | `crates/ndp-lib/tests/gold/fixtures/energy_monitoring.rs` | |
| 10 | `tests/fixtures/phase_c.rs` | 627 | `crates/ndp-lib/tests/gold/fixtures/phase_c.rs` | |

**Integration test lines:** 4,488
**Integration test count:** 112

### 2.3 Test Counts

| Category | Count | Source |
|----------|-------|--------|
| Unit tests (in `src/**/*.rs`) | 264 | `#[test]` and `#[tokio::test]` in source files |
| Integration tests (in `tests/**/*.rs`) | 112 | `#[test]` and `#[tokio::test]` in test files |
| **Total** | **376** | All must pass under `cargo test -p ndp-lib` after migration |

### 2.4 ndp-gold-ddl Dependencies (Cargo.toml)

Dependencies that must be added to `crates/ndp-lib/Cargo.toml`:

| Dependency | Version | Type | Already in ndp-lib? | Action |
|------------|---------|------|---------------------|--------|
| `clap` | 4 (derive, env) | runtime | No | NOT needed (CLI stays in ndp-gold-ddl/ndp-cli) |
| `serde` | 1.0 (derive) | runtime | Yes (workspace) | No change |
| `serde_json` | 1.0 | runtime | Yes (workspace) | No change |
| `thiserror` | 1.0 | runtime | Yes (workspace) | No change |
| `tracing` | 0.1 | runtime | Yes (workspace) | No change |
| `tracing-subscriber` | 0.3 | runtime | No | NOT needed (logging stays in binaries) |
| `tokio` | 1 (rt-multi-thread, macros) | runtime | Yes (workspace) | No change |
| `tokio-postgres` | 0.7 (with-chrono-0_4) | runtime | Yes (workspace) | No change |
| `async-trait` | 0.1 | runtime | Yes (workspace) | No change |
| `tempfile` | 3.8 | dev | Yes | No change |
| `pretty_assertions` | 1.4 | dev | **No** | **ADD** |
| `mockall` | 0.11 | dev | **No** | **ADD** |
| `sha2` | 0.10 | dev | **No** | **ADD** (golden master tests) |

**Cargo.toml additions for ndp-lib:**

```toml
[dev-dependencies]
pretty_assertions = "1.4"
mockall = "0.11"
sha2 = "0.10"
```

### 2.5 Current ndp-lib Structure

```
crates/ndp-lib/src/
  lib.rs           (36 lines)  -- Re-exports: DbClient, NdpLibError, Result, SyncError, SyncOptions, SyncReport
  db.rs            (140 lines) -- DbClient trait, PostgresClient
  config.rs        (exists)    -- ConfigLoader trait, FileSystemConfigLoader
  convert.rs       (exists)    -- Config -> sync-type bridges
  error.rs         (49 lines)  -- NdpLibError enum
  types.rs         (42 lines)  -- SyncReport, SyncError, SyncOptions
  dictionary/      (exists)    -- mod.rs, sql.rs, types.rs
  dimension/       (exists)    -- mod.rs, csv_import.rs, types.rs
  domain/          (exists)    -- mod.rs, sql.rs, types.rs
```

### 2.6 Current ndp-cli Structure

```
tools/ndp-cli/src/
  main.rs           (110 lines) -- Cli struct, Commands enum, resolve_config_dir, resolve_db_url
  commands/
    mod.rs          (8 lines)   -- pub mod dictionary; dimension; domain;
    dictionary.rs   (exists)
    dimension.rs    (exists)
    domain.rs       (148 lines) -- DomainArgs, DomainCommands::Sync, NoOpDbClient
```

---

## 3. Functional Requirements

### FR-001: Gold Module in ndp-lib

**ID:** ops-003-01
**Priority:** Critical

Create `crates/ndp-lib/src/gold/` module with the following structure:

```
crates/ndp-lib/src/gold/
  mod.rs                               -- Public API: generate(), sync(), recreate()
  error.rs                             -- GoldDdlError, ErrorCode
  db.rs                                -- CaChecker trait, CaInfo, PostgresCaChecker
  config/
    mod.rs                             -- Re-exports
    types.rs                           -- StreamConfig (Gold), GoldEtlConfig, Action, etc.
    domain.rs                          -- DomainConfig, AlignmentConfig, etc.
    loader.rs                          -- ConfigLoader trait, FileSystemConfigLoader
  generators/
    mod.rs                             -- Re-exports
    continuous_aggregate.rs
    aligned_view.rs
    state_transitions.rs
    events.rs
    refresh_policy.rs
    column_builder.rs
    join_builder.rs
    null_handler.rs
    constants.rs
    classification.rs
  planner/
    mod.rs                             -- Re-exports
    sync.rs                            -- SyncPlanner, SyncPlan, CaAction, CaPlan
  registry/
    mod.rs                             -- FeatureRegistry
    trait_def.rs                       -- FeatureGenerator trait
    lag.rs
    rolling.rs
    trend.rs
  validation/
    mod.rs                             -- Re-exports
    config_validator.rs                -- ConfigValidator, parse_granularity, parse_window
```

**Acceptance Criteria:**
- `cargo test -p ndp-lib` passes with all 376 gold tests (264 unit + 112 integration)
- `use ndp_lib::gold::*` compiles and exports the full public API
- No `todo!()`, `unimplemented!()`, or placeholder functions

### FR-002: Shared DbClient

**ID:** ops-003-02
**Priority:** Critical

The `CaChecker` trait in `gold/db.rs` must use `ndp_lib::DbClient` (from `crates/ndp-lib/src/db.rs`), NOT the local `ndp_gold_ddl::db::DbClient`.

**Current ndp-gold-ddl DbClient trait:**
```rust
// tools/ndp-gold-ddl/src/db/client.rs
#[async_trait]
pub trait DbClient: Send + Sync {
    async fn query(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<tokio_postgres::Row>, DbError>;
}
```

**Current ndp-lib DbClient trait:**
```rust
// crates/ndp-lib/src/db.rs
#[async_trait]
pub trait DbClient: Send + Sync {
    async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>;
    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64>;
    async fn batch_execute(&self, sql: &str) -> Result<()>;
}
```

**Reconciliation:** ndp-lib's `DbClient` is a superset (has `execute` and `batch_execute`). The `CaChecker` implementation only calls `query()`. Therefore:

1. `PostgresCaChecker<C: DbClient>` changes to use `ndp_lib::DbClient` instead of `ndp_gold_ddl::db::DbClient`.
2. Error mapping changes: `DbError` in queries must map to `GoldDdlError::DatabaseError(String)` via `.map_err(|e| GoldDdlError::DatabaseError(e.to_string()))`. This already works because `ndp_lib::NdpLibError::Database(String)` has the same shape.
3. The `PostgresCaChecker` generic changes from `<C: ndp_gold_ddl::db::DbClient>` to `<C: ndp_lib::DbClient>`.

**Acceptance Criteria:**
- `CaChecker` and `PostgresCaChecker` use `crate::DbClient` (which is `ndp_lib::DbClient`)
- `ndp_gold_ddl::db::DbClient` is not imported anywhere in the gold module
- `SyncPlanner` compiles with `ndp_lib::db::PostgresClient`

### FR-003: `ndp gold` Subcommands

**ID:** ops-003-03
**Priority:** Critical

Add `tools/ndp-cli/src/commands/gold.rs` implementing the following commands, aligned to the CLI UX Design (doc 10):

#### 3.1 Command: `ndp gold generate`

```
ndp gold generate --stream <id> [flags]
ndp gold generate --domain <id> [flags]
```

| Flag | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `--stream <id>` | String | One of stream/domain | - | Target stream for CA DDL |
| `--domain <id>` | String | One of stream/domain | - | Target domain for aligned view DDL |
| `--transitions` | bool | No | false | Generate state transition views instead of CAs |
| `--events` | bool | No | false | Generate events infrastructure (requires `--domain`) |
| `--validate-only` | bool | No | false | Validate config without generating DDL |

**Output:** Prints DDL to stdout. Errors to stderr.

**Exit codes:**
- 0: Success
- 1: Generation error (validation, config)
- 2: System error (file not found)
- 3: Database error

#### 3.2 Command: `ndp gold sync`

```
ndp gold sync --stream <id> [flags]
ndp gold sync --domain <id> [flags]
```

| Flag | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `--stream <id>` | String | One of stream/domain | - | Target stream |
| `--domain <id>` | String | One of stream/domain | - | Target domain |
| `--db-url <url>` | String | Yes (or env TIMESCALE_URL) | - | Database connection URL |
| `--db-timeout <secs>` | u64 | No | 10 | Connection timeout |
| `--dry-run` | bool | No | false | Generate DDL without applying |
| `--no-validate` | bool | No | false | Skip config validation (v1.1.16 behavior) |
| `--verbose` | bool | No | false | Detailed output |
| `--transitions` | bool | No | false | Sync state transition views |
| `--events` | bool | No | false | Sync events infrastructure (requires `--domain`) |

**Behavior:**
1. Connects to DB at `--db-url`
2. Uses `SyncPlanner` to check existing CAs
3. Generates only missing DDL
4. Prints DDL to stdout (deploy.sh pipes to `psql`)

**Output:** Same DDL format as `ndp gold generate`, but filtered by database state (idempotent).

#### 3.3 Command: `ndp gold recreate`

```
ndp gold recreate --stream <id> [flags]
```

| Flag | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `--stream <id>` | String | Yes | - | Target stream |
| `--db-url <url>` | String | Yes (or env) | - | Database connection URL |
| `--db-timeout <secs>` | u64 | No | 10 | Connection timeout |
| `--verbose` | bool | No | false | Detailed output |

**Behavior:** Generates `DROP MATERIALIZED VIEW IF EXISTS ... CASCADE` followed by full `CREATE` DDL for all CAs.

### FR-004: Flag Mapping (Old to New)

Every old invocation must have an exact new equivalent:

| Old (ndp-gold-ddl) | New (ndp gold) | Semantic |
|---------------------|----------------|----------|
| `ndp-gold-ddl generate --stream X` | `ndp gold generate --stream X` | Generate CA DDL |
| `ndp-gold-ddl generate --domain X` | `ndp gold generate --domain X` | Generate aligned view DDL |
| `ndp-gold-ddl generate --stream X --transitions` | `ndp gold generate --stream X --transitions` | State transition DDL |
| `ndp-gold-ddl generate --domain X --events` | `ndp gold generate --domain X --events` | Events DDL |
| `ndp-gold-ddl generate --stream X --action sync --database-url U` | `ndp gold sync --stream X --db-url U` | Idempotent sync |
| `ndp-gold-ddl generate --stream X --action recreate --database-url U` | `ndp gold recreate --stream X --db-url U` | Drop and recreate |
| `ndp-gold-ddl generate --domain X --action sync` | `ndp gold sync --domain X` | Domain sync (no DB needed) |
| `ndp-gold-ddl generate --domain X --events --action sync` | `ndp gold sync --domain X --events` | Events sync |
| `ndp-gold-ddl validate --stream X` | `ndp gold generate --stream X --validate-only` | Validate only |
| `ndp-gold-ddl validate --domain X` | `ndp gold generate --domain X --validate-only` | Validate only |
| `ndp-gold-ddl --config-dir DIR` | `ndp --config-dir DIR gold ...` | Config directory (global) |
| `ndp-gold-ddl --database-url URL` | `ndp --db-url URL gold ...` | Database URL (global, harmonized) |
| `ndp-gold-ddl --db-timeout N` | `ndp gold sync --db-timeout N` | Connection timeout |
| `ndp-gold-ddl --verbose` | `ndp gold sync --verbose` | Verbose output |

**Key differences:**
- `--database-url` becomes `--db-url` (ndp-cli's existing convention)
- `--action sync` is promoted to the verb `sync` (separate subcommand)
- `--action recreate` is promoted to the verb `recreate` (separate subcommand)
- `--config-dir` becomes a global flag (already exists in ndp-cli)

### FR-005: deploy.sh Gold Dispatch Switchover

**ID:** ops-003-04
**Priority:** Critical

#### Site 1: `handle_gold_tables()` (lines 1936-1988)

**BEFORE (current):**
```bash
# Check if ndp-gold-ddl tool is available
local gold_ddl_tool=""
if command -v ndp-gold-ddl &> /dev/null; then
    gold_ddl_tool="ndp-gold-ddl"
elif [ -x "/opt/ndp/bin/ndp-gold-ddl" ]; then
    gold_ddl_tool="/opt/ndp/bin/ndp-gold-ddl"
elif [ -x "$REPO_ROOT/target/release/ndp-gold-ddl" ]; then
    gold_ddl_tool="$REPO_ROOT/target/release/ndp-gold-ddl"
elif [ -x "$REPO_ROOT/target/debug/ndp-gold-ddl" ]; then
    gold_ddl_tool="$REPO_ROOT/target/debug/ndp-gold-ddl"
else
    warn "  ndp-gold-ddl tool not found, skipping Gold DDL generation"
    warn "  Build the tool with: cargo build --release -p ndp-gold-ddl"
    return 0
fi

ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" \
    --database-url "$db_url" \
    --db-timeout 10 \
    generate --stream "$stream_id" --action "$action" 2>&1)
```

**AFTER (v1.1.14):**
```bash
# Resolve ndp tool (required -- no fallback)
local ndp_tool=""
if command -v ndp &> /dev/null; then
    ndp_tool="ndp"
elif [ -x "/opt/ndp/bin/ndp" ]; then
    ndp_tool="/opt/ndp/bin/ndp"
elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/release/ndp"
elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/debug/ndp"
else
    error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
    return 1
fi

ddl=$("$ndp_tool" gold "$action" --stream "$stream_id" \
    --config-dir "$REPO_ROOT/config" \
    --db-url "$db_url" \
    --db-timeout 10 2>&1)
```

**Key changes:**
1. `ndp-gold-ddl` -> `ndp` (binary name)
2. `warn` + `return 0` -> `error` + `return 1` (no fallback)
3. `--database-url` -> `--db-url` (harmonized)
4. `generate --stream X --action Y` -> `gold "$action" --stream X` (verb promoted to subcommand)
5. `--config-dir` moves before `gold` subcommand (global flag)

**The `$action` variable maps directly to the verb:**
- `$action = "sync"` -> `ndp gold sync --stream X`
- `$action = "recreate"` -> `ndp gold recreate --stream X`

#### Site 2: `handle_domain()` gold dispatch (lines 2069-2129)

**BEFORE (current):**
```bash
# Check if ndp-gold-ddl tool is available for aligned view generation
local gold_ddl_tool=""
if command -v ndp-gold-ddl &> /dev/null; then
    gold_ddl_tool="ndp-gold-ddl"
elif [ -x "/opt/ndp/bin/ndp-gold-ddl" ]; then
    gold_ddl_tool="/opt/ndp/bin/ndp-gold-ddl"
elif [ -x "$REPO_ROOT/target/release/ndp-gold-ddl" ]; then
    gold_ddl_tool="$REPO_ROOT/target/release/ndp-gold-ddl"
elif [ -x "$REPO_ROOT/target/debug/ndp-gold-ddl" ]; then
    gold_ddl_tool="$REPO_ROOT/target/debug/ndp-gold-ddl"
fi

if [ -z "$gold_ddl_tool" ]; then
    warn "  ndp-gold-ddl tool not found, skipping aligned view DDL generation"
    warn "  Build the tool with: cargo build --release -p ndp-gold-ddl"
    return 0
fi

# Aligned view DDL
ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" generate \
    --domain "$domain_id" --action "$action" 2>&1)

# Events DDL
events_ddl=$("$gold_ddl_tool" --config-dir "$REPO_ROOT/config" generate \
    --domain "$domain_id" --events --action "$action" 2>&1)
```

**AFTER (v1.1.14):**
```bash
# Resolve ndp tool (required -- no fallback)
local ndp_tool=""
if command -v ndp &> /dev/null; then
    ndp_tool="ndp"
elif [ -x "/opt/ndp/bin/ndp" ]; then
    ndp_tool="/opt/ndp/bin/ndp"
elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/release/ndp"
elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
    ndp_tool="$REPO_ROOT/target/debug/ndp"
else
    error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
    return 1
fi

# Aligned view DDL
ddl=$("$ndp_tool" gold "$action" --domain "$domain_id" \
    --config-dir "$REPO_ROOT/config" 2>&1)

# Events DDL
events_ddl=$("$ndp_tool" gold "$action" --domain "$domain_id" \
    --events --config-dir "$REPO_ROOT/config" 2>&1)
```

**Key changes:** Same as Site 1 plus:
- Domain DDL does NOT use `--db-url` (no DB check for aligned views)
- Events DDL adds `--events` flag
- The two separate invocations (aligned view + events) remain separate

### FR-006: Output Format Parity

#### `ndp gold generate --stream X` Output

Must produce identical DDL to `ndp-gold-ddl generate --stream X`:

```sql
-- Gold layer DDL for stream: air-quality
-- Generated by ndp-gold-ddl

CREATE SCHEMA IF NOT EXISTS gold;

CREATE MATERIALIZED VIEW IF NOT EXISTS gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    AVG(pm25) AS pm25_mean,
    ...
FROM silver.air_quality_observations
GROUP BY bucket
WITH NO DATA;

SELECT add_continuous_aggregate_policy('gold.air_quality_hourly', ...);
```

**Parity contract:** The `-- Generated by` comment may say `ndp-gold-ddl` or `ndp` -- this is not a semantic difference. All SQL statements must be character-identical.

#### `ndp gold sync --stream X --db-url U` Output

Same format as generate, but filtered by database state:

```sql
-- Gold layer DDL for stream: air-quality
-- Generated by ndp-gold-ddl

CREATE SCHEMA IF NOT EXISTS gold;

-- Skipping gold.air_quality_hourly (already exists)
-- Creating gold.air_quality_daily (does not exist)
CREATE MATERIALIZED VIEW IF NOT EXISTS gold.air_quality_daily ...
```

---

## 4. Interface Contracts

### 4.1 `ndp_lib::gold` Public API

The `gold/mod.rs` file exposes these public functions:

#### `generate()`

```rust
/// Generate Gold DDL for a stream (continuous aggregates) or domain (aligned views).
///
/// Pure function -- no database access. Generates all DDL unconditionally.
///
/// # Arguments
/// * `config` - Gold-specific stream configuration
/// * `opts` - Generation options
///
/// # Returns
/// DDL string ready to pipe to psql.
pub fn generate_stream_ddl(
    config: &StreamConfig,
    opts: &GenerateOptions,
) -> Result<String, GoldDdlError>
```

```rust
/// Generate aligned view DDL for a domain.
pub fn generate_domain_ddl(
    domain_config: &DomainConfig,
    loader: Box<dyn ConfigLoader>,
    action: Action,
) -> Result<String, GoldDdlError>
```

```rust
/// Generate events infrastructure DDL for a domain.
pub fn generate_events_ddl(
    domain_config: &DomainConfig,
    loader: Box<dyn ConfigLoader>,
    action: Action,
) -> Result<String, GoldDdlError>
```

```rust
/// Generate state transitions DDL for a stream.
pub fn generate_transitions_ddl(
    config: &StreamConfig,
    action: Action,
) -> Result<String, GoldDdlError>
```

#### `sync()`

```rust
/// Sync Gold DDL with database state (idempotent).
///
/// Connects to the database, checks which CAs exist, generates only
/// the DDL for missing objects. Returns DDL string; caller applies it.
///
/// # Arguments
/// * `config` - Gold-specific stream configuration
/// * `checker` - CaChecker implementation (real or mock)
/// * `opts` - Sync options (verbose)
///
/// # Returns
/// DDL string filtered by database state.
pub async fn sync_stream_ddl(
    config: &StreamConfig,
    checker: &(impl CaChecker + Send + Sync),
    opts: &SyncOptions,
) -> Result<String, GoldDdlError>
```

#### `recreate()`

```rust
/// Generate DDL to drop and recreate all CAs for a stream.
///
/// # Arguments
/// * `config` - Gold-specific stream configuration
/// * `opts` - Generation options
///
/// # Returns
/// DDL with DROP + CREATE statements.
pub fn recreate_stream_ddl(
    config: &StreamConfig,
    opts: &GenerateOptions,
) -> Result<String, GoldDdlError>
```

#### `validate()`

```rust
/// Validate Gold ETL configuration for a stream.
///
/// # Returns
/// Ok(()) if valid, Err(GoldDdlError) with specific validation error.
pub fn validate_gold_config(config: &StreamConfig) -> Result<(), GoldDdlError>
```

### 4.2 Supporting Types

#### `GenerateOptions`

```rust
/// Options for DDL generation (read-only operations).
pub struct GenerateOptions {
    /// Action: Sync (IF NOT EXISTS) or Recreate (DROP + CREATE)
    pub action: Action,
    /// Show verbose output
    pub verbose: bool,
}
```

#### `SyncOptions` (extended)

The existing `ndp_lib::types::SyncOptions` gains `validate` and `verbose` fields in v1.1.16. For v1.1.14, we use `GenerateOptions` for the gold module since gold sync needs different options than dictionary/dimension sync.

#### `CaChecker` trait

```rust
/// Trait for checking continuous aggregate existence in TimescaleDB.
#[async_trait]
pub trait CaChecker: Send + Sync {
    async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError>;
    async fn get_ca_info(&self, schema: &str, name: &str) -> Result<Option<CaInfo>, GoldDdlError>;
    async fn list_cas_in_schema(&self, schema: &str) -> Result<Vec<CaInfo>, GoldDdlError>;
    async fn refresh_policy_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError>;
}
```

Note: The `CaChecker` error type changes from `DbError` to `GoldDdlError` (simplified mapping). `PostgresCaChecker` now takes `ndp_lib::DbClient` and maps errors.

#### `PostgresCaChecker`

```rust
/// PostgreSQL/TimescaleDB implementation of CaChecker.
/// Uses ndp_lib::DbClient for database access.
pub struct PostgresCaChecker<C: ndp_lib::DbClient> {
    client: C,
}

impl<C: ndp_lib::DbClient> PostgresCaChecker<C> {
    pub fn new(client: C) -> Self { ... }
}
```

### 4.3 Error Types

`GoldDdlError` moves as-is from `tools/ndp-gold-ddl/src/error.rs` to `crates/ndp-lib/src/gold/error.rs`. No changes to variants. The `DbError` type from `ndp_gold_ddl::db::client` is NOT migrated; `GoldDdlError::DatabaseError(String)` handles all DB errors.

### 4.4 Re-exports from `gold/mod.rs`

```rust
pub mod config;
pub mod db;
pub mod error;
pub mod generators;
pub mod planner;
pub mod registry;
pub mod validation;

// Public API functions
pub use self::generate::{
    generate_stream_ddl, generate_domain_ddl,
    generate_events_ddl, generate_transitions_ddl,
};
pub use self::sync::sync_stream_ddl;
pub use self::recreate::recreate_stream_ddl;
pub use self::validation::validate_gold_config;

// Re-exports for consumer convenience
pub use config::{
    Action, StreamConfig, GoldEtlConfig, DomainConfig, ConfigLoader,
    FileSystemConfigLoader,
};
pub use db::{CaChecker, CaInfo, PostgresCaChecker};
pub use error::{GoldDdlError, ErrorCode};
pub use generators::{
    ContinuousAggregateGenerator, AlignedViewGenerator,
    StateTransitionGenerator, EventsGenerator, RefreshPolicyGenerator,
};
pub use planner::{SyncPlanner, SyncPlan, CaAction, CaPlan};
pub use registry::{FeatureRegistry, FeatureGenerator, FeatureConfig, SqlColumn};
```

### 4.5 ndp-lib `lib.rs` Changes

Add the gold module:

```rust
pub mod config;
pub mod convert;
pub mod db;
pub mod dictionary;
pub mod dimension;
pub mod domain;
pub mod error;
pub mod gold;       // NEW
pub mod types;
```

---

## 5. Non-Functional Requirements

### NFR-001: Test Parity

All 376 ndp-gold-ddl tests must pass under `cargo test -p ndp-lib`.

**Verification:**
```bash
# Before migration (baseline)
cargo test -p ndp-gold-ddl 2>&1 | tail -1
# Expected: test result: ok. 376 passed; 0 failed

# After migration
cargo test -p ndp-lib 2>&1 | grep 'test result'
# Must show >= 376 passed (existing ndp-lib tests + gold tests)
```

### NFR-002: Output Parity

`ndp gold generate --stream X --config-dir DIR` must produce character-identical DDL to `ndp-gold-ddl --config-dir DIR generate --stream X` for all stream and domain configs.

**Verification:**
```bash
diff <(ndp-gold-ddl --config-dir config generate --stream air-quality) \
     <(ndp gold generate --stream air-quality --config-dir config)
# Expected: no diff (empty output)

diff <(ndp-gold-ddl --config-dir config generate --domain indoor-air-quality) \
     <(ndp gold generate --domain indoor-air-quality --config-dir config)
# Expected: no diff
```

Exception: The `-- Generated by` comment line may differ (`ndp-gold-ddl` vs `ndp`). This is acceptable.

### NFR-003: deploy.sh No-Regression

deploy.sh Gold phases must complete successfully using only the `ndp` binary.

**Verification:**
```bash
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
# Phase 5 (Gold tables) must complete successfully
# Phase 9 (Domain declarations) must complete successfully
```

### NFR-004: No Fallback in deploy.sh

If `ndp` binary is not found at any of the 4 lookup locations, deploy.sh must `error` and `return 1`. It must NOT `warn` and `return 0`.

**Verification:** Remove the `ndp` binary and run deploy.sh. Gold phases must fail loudly, not silently skip.

### NFR-005: Binary Size

The `ndp` binary (ndp-cli) must stay under 15MB after adding gold module code.

**Verification:**
```bash
cargo build --release -p ndp-cli
ls -la target/release/ndp | awk '{print $5}'
# Must be < 15728640 (15MB)
```

### NFR-006: Standalone Binary Still Builds

`ndp-gold-ddl` must still compile and produce correct output after the migration. It becomes a thin wrapper that calls `ndp_lib::gold::*`.

**Verification:**
```bash
cargo build -p ndp-gold-ddl
ndp-gold-ddl --config-dir config generate --stream air-quality
# Must produce same DDL as before
```

---

## 6. Clap Structure for `ndp gold`

### 6.1 GoldArgs (top-level)

```rust
/// Gold layer DDL operations.
#[derive(Args)]
pub struct GoldArgs {
    #[command(subcommand)]
    pub command: GoldCommands,
}
```

### 6.2 GoldCommands

```rust
#[derive(Subcommand)]
pub enum GoldCommands {
    /// Generate Gold DDL without applying.
    Generate {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Generate state transition views instead of CAs.
        #[arg(long)]
        transitions: bool,

        /// Generate events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,

        /// Validate config only, do not generate DDL.
        #[arg(long)]
        validate_only: bool,
    },

    /// Sync Gold DDL with database state (idempotent create-if-not-exists).
    Sync {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Database connection timeout in seconds.
        #[arg(long, default_value = "10")]
        db_timeout: u64,

        /// Generate state transition views instead of CAs.
        #[arg(long)]
        transitions: bool,

        /// Generate events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,

        /// Print DDL without applying (dry-run).
        #[arg(long)]
        dry_run: bool,

        /// Verbose output.
        #[arg(long)]
        verbose: bool,
    },

    /// Drop and recreate Gold DDL objects.
    Recreate {
        /// Target stream ID.
        #[arg(long)]
        stream: String,

        /// Database connection timeout in seconds.
        #[arg(long, default_value = "10")]
        db_timeout: u64,

        /// Verbose output.
        #[arg(long)]
        verbose: bool,
    },
}
```

### 6.3 ndp-cli Commands Enum Update

```rust
#[derive(Subcommand)]
enum Commands {
    /// Data dictionary operations.
    Dictionary(commands::dictionary::DictionaryArgs),
    /// Dimension table operations.
    Dimension(commands::dimension::DimensionArgs),
    /// Domain configuration operations.
    Domain(commands::domain::DomainArgs),
    /// Gold layer DDL operations.        // NEW
    Gold(commands::gold::GoldArgs),       // NEW
}
```

### 6.4 commands/mod.rs Update

```rust
pub mod dictionary;
pub mod dimension;
pub mod domain;
pub mod gold;    // NEW
```

---

## 7. `use` Path Changes

All moved files must update their `use` paths from `crate::` (ndp-gold-ddl) to `crate::gold::` (ndp-lib gold module) or to top-level `crate::` for shared ndp-lib infrastructure.

### 7.1 Internal gold module references

| Old (in ndp-gold-ddl) | New (in ndp-lib gold module) |
|------------------------|------------------------------|
| `crate::config::*` | `crate::gold::config::*` |
| `crate::db::*` | `crate::gold::db::*` (for CaChecker) |
| `crate::db::DbClient` | `crate::DbClient` (ndp-lib's DbClient) |
| `crate::error::*` | `crate::gold::error::*` |
| `crate::generators::*` | `crate::gold::generators::*` |
| `crate::planner::*` | `crate::gold::planner::*` |
| `crate::registry::*` | `crate::gold::registry::*` |
| `crate::validation::*` | `crate::gold::validation::*` |

### 7.2 External references from ndp-gold-ddl thin wrapper

```rust
// tools/ndp-gold-ddl/src/main.rs (after migration)
use ndp_lib::gold::config::{Action, ConfigLoader, FileSystemConfigLoader};
use ndp_lib::gold::db::PostgresCaChecker;
use ndp_lib::gold::generators::{...};
use ndp_lib::gold::planner::SyncPlanner;
use ndp_lib::db::PostgresClient;
```

### 7.3 Test file references

All test files change `use ndp_gold_ddl::` to `use ndp_lib::gold::`:

| Old | New |
|-----|-----|
| `use ndp_gold_ddl::StreamConfig` | `use ndp_lib::gold::StreamConfig` |
| `use ndp_gold_ddl::generators::*` | `use ndp_lib::gold::generators::*` |
| `use ndp_gold_ddl::config::*` | `use ndp_lib::gold::config::*` |

---

## 8. deploy.sh Contract Summary

### 8.1 Current State (v1.1.13)

| Dispatch Site | Function | Binary | Lines |
|---------------|----------|--------|-------|
| 1 | `handle_gold_tables()` | ndp-gold-ddl | 1936-1988 |
| 2 | `handle_domain()` gold part | ndp-gold-ddl | 2069-2129 |
| 3 | `validate_domain_configs()` | ndp-validate | ~1535 |
| 4 | `handle_domain()` validate part | ndp-validate | ~2033-2055 |
| 5 | dictionary sync | ndp | ~386 |
| 6 | dimension sync | ndp | ~894 |
| 7 | domain sync | ndp | ~1063 |

### 8.2 After v1.1.14

| Dispatch Site | Function | Binary | Change |
|---------------|----------|--------|--------|
| 1 | `handle_gold_tables()` | **ndp** | **CHANGED** (was ndp-gold-ddl) |
| 2 | `handle_domain()` gold part | **ndp** | **CHANGED** (was ndp-gold-ddl) |
| 3 | `validate_domain_configs()` | ndp-validate | Unchanged (v1.1.15) |
| 4 | `handle_domain()` validate part | ndp-validate | Unchanged (v1.1.15) |
| 5 | dictionary sync | ndp | Unchanged |
| 6 | dimension sync | ndp | Unchanged |
| 7 | domain sync | ndp | Unchanged |

**Sites 1 and 2 change. Sites 3-7 unchanged.**

### 8.3 Invocation Pattern Changes

**Site 1 (`handle_gold_tables`):**

| Aspect | Before | After |
|--------|--------|-------|
| Binary | `ndp-gold-ddl` | `ndp` |
| Not-found behavior | `warn` + `return 0` | `error` + `return 1` |
| Config flag | `--config-dir "$REPO_ROOT/config"` | `--config-dir "$REPO_ROOT/config"` |
| DB flag | `--database-url "$db_url"` | `--db-url "$db_url"` |
| Timeout flag | `--db-timeout 10` | `--db-timeout 10` |
| Command | `generate --stream "$stream_id" --action "$action"` | `gold "$action" --stream "$stream_id"` |

**Site 2 (`handle_domain` gold part):**

| Aspect | Before | After |
|--------|--------|-------|
| Binary | `ndp-gold-ddl` | `ndp` |
| Not-found behavior | `warn` + `return 0` | `error` + `return 1` |
| Aligned view cmd | `generate --domain "$domain_id" --action "$action"` | `gold "$action" --domain "$domain_id"` |
| Events cmd | `generate --domain "$domain_id" --events --action "$action"` | `gold "$action" --domain "$domain_id" --events` |
| DB flag | Not used for domain | Not used for domain |

---

## 9. Migration Procedure (Implementation Guide)

### Step 1: Create gold module directory structure

```bash
mkdir -p crates/ndp-lib/src/gold/{config,generators,planner,registry,validation}
mkdir -p crates/ndp-lib/tests/gold/fixtures
```

### Step 2: Move source files with `git mv`

Use `git mv` (not `cp`) to preserve history. Update `use` paths in every moved file.

### Step 3: Wire CaChecker to ndp-lib DbClient

In `gold/db.rs`:
- Replace `use super::client::{DbClient, DbError}` with `use crate::DbClient`
- Replace `Result<..., DbError>` with `Result<..., GoldDdlError>` in CaChecker trait
- Map ndp-lib errors: `.map_err(|e| GoldDdlError::DatabaseError(e.to_string()))`

### Step 4: Update ndp-lib Cargo.toml

Add dev-dependencies: `pretty_assertions`, `mockall`, `sha2`.

### Step 5: Update ndp-lib lib.rs

Add `pub mod gold;` to the module list.

### Step 6: Verify all 376 tests pass

```bash
cargo test -p ndp-lib 2>&1 | grep 'test result'
```

### Step 7: Update ndp-gold-ddl to thin wrapper

- `Cargo.toml`: Add `ndp-lib = { path = "../../crates/ndp-lib" }` as dependency
- `lib.rs`: Re-export from `ndp_lib::gold`
- `main.rs`: Change `use ndp_gold_ddl::` to `use ndp_lib::gold::`

### Step 8: Verify ndp-gold-ddl standalone still works

```bash
cargo build -p ndp-gold-ddl
ndp-gold-ddl --config-dir config generate --stream air-quality
```

### Step 9: Add commands/gold.rs to ndp-cli

Implement the GoldArgs/GoldCommands clap structure. Wire to `ndp_lib::gold::*`.

### Step 10: Verify output parity

```bash
diff <(ndp-gold-ddl --config-dir config generate --stream air-quality) \
     <(ndp gold generate --stream air-quality --config-dir config)
```

### Step 11: Update deploy.sh

Change Sites 1 and 2 as specified in FR-005.

### Step 12: Integration test

```bash
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
```

---

## 10. Acceptance Criteria (Gherkin)

### Scenario: Gold DDL generation via ndp CLI

```gherkin
Feature: Gold DDL generation via ndp CLI

  Scenario: Generate stream CA DDL
    Given the air-quality stream config exists at config/base/streams/air-quality/config.json
    And the stream has gold_etl.enabled = true
    When I run "ndp gold generate --stream air-quality --config-dir config"
    Then the exit code should be 0
    And stdout should contain "CREATE MATERIALIZED VIEW"
    And stdout should contain "gold.air_quality_hourly"

  Scenario: Generate domain aligned view DDL
    Given the indoor-air-quality domain config exists at config/domains/indoor-air-quality/domain.json
    When I run "ndp gold generate --domain indoor-air-quality --config-dir config"
    Then the exit code should be 0
    And stdout should contain "CREATE MATERIALIZED VIEW"

  Scenario: Sync stream CAs with database
    Given the air-quality stream config exists
    And gold.air_quality_hourly already exists in the database
    And gold.air_quality_daily does not exist
    When I run "ndp gold sync --stream air-quality --config-dir config --db-url postgresql://..."
    Then the exit code should be 0
    And stdout should contain "Skipping gold.air_quality_hourly"
    And stdout should contain "Creating gold.air_quality_daily"

  Scenario: Output parity with ndp-gold-ddl
    Given the air-quality stream config exists
    When I run "ndp-gold-ddl --config-dir config generate --stream air-quality"
    And capture stdout as OLD_DDL
    And I run "ndp gold generate --stream air-quality --config-dir config"
    And capture stdout as NEW_DDL
    Then OLD_DDL and NEW_DDL should be identical

  Scenario: deploy.sh fails when ndp not found
    Given the ndp binary is not in PATH or any fallback location
    When deploy.sh calls handle_gold_tables()
    Then the function should return exit code 1
    And stderr should contain "ndp tool not found"

  Scenario: deploy.sh Gold phase succeeds via ndp
    Given the ndp binary is available
    And the integration environment is running
    When I run "DEPLOY_ENV=integration ./deploy.sh apply manifest.json"
    Then the Gold tables phase should complete successfully
    And the domain Gold DDL phase should complete successfully
```

### Scenario: All gold tests pass in ndp-lib

```gherkin
  Scenario: Migrated tests pass
    When I run "cargo test -p ndp-lib"
    Then at least 376 gold-related tests should pass
    And 0 tests should fail
```

---

## 11. Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `use` path errors during migration | High | Low | Compiler catches all errors. Systematic find-and-replace. |
| CaChecker error type mismatch | Medium | Medium | Map `NdpLibError` to `GoldDdlError::DatabaseError(String)` in every CaChecker method |
| Integration test fixture paths break | Medium | Medium | Tests use `tempfile::TempDir`; no hardcoded paths. Fixtures embedded in test files. |
| deploy.sh regression | Medium | High | Integration test before release. Only 2 sites change. |
| Golden master tests reference old crate | Low | Low | Change `ndp_gold_ddl::` to `ndp_lib::gold::` in all test imports |
| Binary size exceeds 15MB | Low | Low | Code moves, doesn't grow. Single binary replaces separate builds. |

---

## 12. Out of Scope for v1.1.14

These are explicitly deferred to later releases:

| Item | Deferred To | Reason |
|------|-------------|--------|
| ndp-validate migration | v1.1.15 | Separate release |
| Shared constants (`VALID_METRICS` in ndp-lib root) | v1.1.16 | Cross-module concern |
| Cross-cutting validation (`gold::sync()` calls `validate::gold_config()`) | v1.1.16 | validate module not yet in ndp-lib |
| NoOpDbClient dedup | v1.1.16 | Low priority, works as-is |
| `--no-validate` flag behavior | v1.1.16 | Needs validate module |
| deploy.sh ndp-validate dispatch sites | v1.1.15 | Separate release |
| Unifying StreamConfig types | v1.3 | Premature optimization |

---

## 13. Release Artifacts

Per RELEASE-POLICY.md:

### Manifest: `.deploy/releases/v1.1.14.manifest.json`

```json
{
  "version": "1.0",
  "release_version": "1.1.14",
  "description": "Release v1.1.14: Gold DDL generation consolidated into ndp-lib and ndp CLI",
  "changes": [
    {"type": "tool", "id": "ndp-cli", "action": "build", "profile": "release"}
  ]
}
```

### Git Tag: `v1.1.14` (annotated)

### Changelog Entry

```
## [1.1.14] - 2026-02-XX

### Changed
- Gold DDL generation migrated from ndp-gold-ddl standalone to ndp-lib::gold module
- deploy.sh Gold dispatch sites (handle_gold_tables, handle_domain) now call `ndp gold` instead of `ndp-gold-ddl`
- deploy.sh fails loudly (error + return 1) if ndp binary not found for Gold operations

### Added
- `ndp gold generate` subcommand (replaces `ndp-gold-ddl generate`)
- `ndp gold sync` subcommand (replaces `ndp-gold-ddl generate --action sync`)
- `ndp gold recreate` subcommand (replaces `ndp-gold-ddl generate --action recreate`)
- 376 Gold DDL tests in ndp-lib

### Deprecated
- `ndp-gold-ddl` standalone binary (still builds but no longer called by deploy.sh)
```

---

## 14. Verification Commands (Complete Checklist)

```bash
# 1. All gold tests pass in ndp-lib
cargo test -p ndp-lib 2>&1 | grep 'test result'

# 2. ndp-gold-ddl standalone still works
cargo build -p ndp-gold-ddl
ndp-gold-ddl --config-dir config generate --stream air-quality > /dev/null && echo "OK"

# 3. ndp gold commands work
cargo build -p ndp-cli
ndp gold generate --stream air-quality --config-dir config > /dev/null && echo "OK"
ndp gold generate --domain indoor-air-quality --config-dir config > /dev/null && echo "OK"

# 4. Output parity (stream)
diff <(ndp-gold-ddl --config-dir config generate --stream air-quality) \
     <(ndp gold generate --stream air-quality --config-dir config)

# 5. Output parity (domain)
diff <(ndp-gold-ddl --config-dir config generate --domain indoor-air-quality) \
     <(ndp gold generate --domain indoor-air-quality --config-dir config)

# 6. Binary size check
ls -la target/release/ndp | awk '{print $5}'

# 7. No ndp-gold-ddl references in deploy.sh (gold sites only)
grep -n 'ndp-gold-ddl' deploy/pi/deploy.sh | grep -v '#'
# Should return ZERO non-comment lines in the handle_gold_tables and handle_domain functions

# 8. deploy.sh integration test
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

# 9. Zero calls to ndp-gold-ddl in deploy.sh gold sites
grep -c 'gold_ddl_tool' deploy/pi/deploy.sh
# Should be 0 (variable eliminated from both sites)
```

---

## 15. Dependencies and Prerequisites

| Dependency | Status | Required For |
|------------|--------|-------------|
| ndp-lib crate (`crates/ndp-lib/`) | Exists (ops-001) | Adding gold module |
| ndp-cli crate (`tools/ndp-cli/`) | Exists (ops-001) | Adding gold command |
| ndp-gold-ddl crate (`tools/ndp-gold-ddl/`) | Exists | Source of migration |
| ndp-lib DbClient trait | Exists (ops-001) | CaChecker wiring |
| ndp-lib PostgresClient | Exists (ops-001) | DB connectivity |
| docker-compose.integration.yml | Exists | Integration testing |
| deploy.sh | Exists | Dispatch site changes |
| All 376 ndp-gold-ddl tests passing | Precondition | Baseline verification |

**Precondition check:**
```bash
cargo test -p ndp-gold-ddl 2>&1 | tail -1
# Must show: test result: ok. 376 passed; 0 failed
```
