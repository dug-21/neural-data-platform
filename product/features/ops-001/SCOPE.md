# OPS-001: Deployment Tooling Foundation (V1.1.9)

> **Feature ID:** ops-001
> **Version:** V1.1.9
> **Created:** 2026-02-06
> **Status:** Scoping
> **Phase:** ops (Infrastructure / Deployment)

---

## Executive Summary

OPS-001 extracts the two largest DB-facing functions from `deploy.sh` (~780 lines of Bash) into Rust, establishing the `ndp` CLI and `ndp-lib` crate as the permanent home for all deployment operations that interact with TimescaleDB. This is the seed that all future V1.2+ ops features extend without refactoring.

### The OPS-001 Promise

| Capability | Description |
|------------|-------------|
| **ndp CLI** | Entity/verb command structure (`ndp <entity> <verb>`) designed for the full V1.1-V2.0 journey |
| **ndp-lib crate** | Shared library consumed by CLI today, MCP server tomorrow. Functions take parsed structs, not file paths |
| **dictionary sync** | `ndp dictionary sync` replaces `sync_to_data_dictionary()` (~460 lines of Bash SQL generation) |
| **dimension sync** | `ndp dimension sync` replaces `import_dimension_sql()` + `sync_dimension()` fallback (~85 lines) |
| **deploy.sh parity** | deploy.sh calls `ndp` instead of inline SQL. End-to-end `./deploy.sh apply` behavior is identical |
| **Integration tested** | Verified against `docker-compose.integration.yml` stack (etcd + TimescaleDB + apps) |
| **Architecture patterns** | Documented patterns all future agents must follow when adding commands |

### Success Test

> **Can `DEPLOY_ENV=integration ./deploy.sh apply <manifest>` complete identically with the Rust-backed dictionary and dimension sync, producing the same DB state as the current Bash implementation?**

If yes, the migration is safe and deploy.sh can be progressively hollowed out.

---

## Problem Statement

### Current State (V1.1.8)

`deploy.sh` is 2,868 lines, 43 functions, 11 deployment phases. It works. But:

- **~1,100 lines (38%) are DB-facing operations** - SQL generation via string concatenation in Bash
- **`sync_to_data_dictionary()`** (lines 375-838, ~460 lines) is the largest function. It parses YAML/JSON configs, generates SQL INSERT statements via echo/heredoc, and pipes them to `psql`. This is fragile, hard to test, and has no type safety.
- **Dimension sync** uses CSV import via `docker cp` + `\COPY` - works but is opaque
- **No unit tests** on any of these SQL-generating functions
- **SQL injection surface** - stream descriptions are sed-escaped (`s/'/''/g`) but not parameterized
- **Two config parsers** - `yaml_get()` has fallback chains (yq -> python -> grep/sed) because Bash has no native JSON/YAML support
- **Stale config reads (BUG)** - `sync_to_data_dictionary()` reads legacy `.yaml` stream configs (lines 411, 502, 589) while authoritative configs are `.json` since FE-002. The dictionary may not reflect current config state.
- **Dimension config never migrated** - `entity_context.yaml` was missed during FE-002 JSON standardization. Only dimension config still in YAML format.

### Target State (V1.1.9)

- `ndp dictionary sync` and `ndp dimension sync` replace the Bash implementations
- deploy.sh calls `ndp` with appropriate flags; falls back to Bash if binary not found
- `ndp-lib` crate contains the business logic, consumed by the thin CLI wrapper
- All functions take parsed config structs (source-agnostic: files today, etcd later)
- Unit tests cover all SQL generation and data transformation logic
- Integration tests verify against the real stack
- All configs read from `.json` (fixes stale-data bug in dictionary sync)
- Dimension config migrated from YAML to JSON (completes FE-002 standardization)

### Why Now (Before V1.2)

V1.2 (Pattern Detection Engine) will add new deployment operations:

| V1.2 Addition | Deploy Operation | Without OPS-001 | With OPS-001 |
|---------------|------------------|------------------|--------------|
| Correlation jobs | `ndp job create/sync` | More Bash SQL generation | Add command to ndp CLI |
| Pattern storage | `ndp pattern sync` | More Bash SQL generation | Add command to ndp CLI |
| Event detection config | `ndp event configure` | More Bash SQL generation | Add command to ndp CLI |

Without OPS-001, every V1.2 feature adds more untestable Bash SQL. With OPS-001, they extend a tested Rust codebase.

---

## Scope Definition

### In Scope

#### Tier 1: Foundation (Must Have)

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **ops-001-01** | ndp-lib crate | Shared library crate in `crates/ndp-lib/` with DbClient, ConfigLoader traits | Crate compiles; traits defined; consumed by CLI |
| **ops-001-02** | ndp CLI scaffold | `tools/ndp-cli/` with clap-based entity/verb routing | `ndp --help` shows entity list; `ndp dictionary --help` shows verbs |
| **ops-001-03** | dictionary sync | `ndp dictionary sync --config-dir <path>` replaces `sync_to_data_dictionary()` | Same DB state as Bash; unit + integration tested |
| **ops-001-04** | dimension sync | `ndp dimension sync <id> --config <path> --source <path>` replaces `import_dimension_sql()` | Same DB state as Bash; unit + integration tested |
| **ops-001-05** | deploy.sh integration | deploy.sh calls `ndp` for dictionary + dimension sync with Bash fallback | `deploy.sh apply` works with and without `ndp` binary |
| **ops-001-06** | Integration tests | End-to-end tests against `docker-compose.integration.yml` | Tests pass in CI-equivalent environment |
| **ops-001-12** | JSON-only configs | Dictionary sync reads `.json` stream configs (not legacy `.yaml`). Migrate `entity_context.yaml` → `.json` | All configs JSON; legacy YAML ignored |
| **ops-001-13** | Fix stale dictionary sync | Current Bash `sync_to_data_dictionary()` reads legacy `.yaml`; Rust reads authoritative `.json` | Dictionary reflects current JSON config state |

#### Tier 2: Architecture (Must Have)

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **ops-001-07** | Command map (proposed) | Document the full entity/verb map from V1.1 through V2.0 | Document exists; reviewed; referenced by CLAUDE.md |
| **ops-001-08** | Architecture patterns doc | Patterns all future agents must follow when adding ndp commands | Document exists; covers traits, testing, error handling |
| **ops-001-09** | DEPLOY_ENV support | ndp CLI respects environment (integration vs pi) for connection params | `--env integration` or env var selects correct DB |

#### Tier 3: Quality

| ID | Feature | Description | Acceptance Criteria |
|----|---------|-------------|---------------------|
| **ops-001-10** | Golden master tests | Capture current Bash SQL output; verify Rust produces equivalent | Golden master fixtures exist; CI comparison passes |
| **ops-001-11** | Dry-run mode | `ndp dictionary sync --dry-run` prints SQL without executing | Matches deploy.sh `--dry-run` behavior |

### Out of Scope (Deferred)

| Item | Reason | Target |
|------|--------|--------|
| Migrating `handle_gold_table()` | ndp-gold-ddl already handles this | N/A |
| Migrating `handle_domain()` | ndp-gold-ddl already handles this | N/A |
| Migrating `handle_silver_table()` | Lower value; ddl-generator.sh works | ops-002 |
| Migrating `sync_config()` (etcd sync) | Not DB-facing; Bash is adequate | ops-002 |
| Reading config from etcd | Architect for it now; implement later | V1.3 |
| CI/CD pipeline for Rust builds | No proven time savings on current setup | Deferred |
| MCP server consuming ndp-lib | Future work; architecture supports it | V1.3+ |
| Migrating `handle_stream()` | etcd-facing, not DB-facing | ops-002 |

---

## Technical Design

### Crate Architecture

```
Cargo.toml (workspace - add new members)
├── crates/
│   ├── ndp-types/          # Existing: shared type definitions
│   └── ndp-lib/            # NEW: shared operational logic
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs       # Re-exports
│           ├── db.rs        # DbClient trait + PostgresClient (extracted from ndp-gold-ddl)
│           ├── config.rs    # ConfigLoader trait + FileSystemLoader
│           ├── dictionary/
│           │   ├── mod.rs   # sync_dictionary(), SyncReport
│           │   └── sql.rs   # SQL generation (parameterized)
│           ├── dimension/
│           │   ├── mod.rs   # sync_dimension(), DimensionSyncReport
│           │   └── csv.rs   # CSV parsing and import
│           └── error.rs     # NdpLibError enum
└── tools/
    ├── ndp-cli/             # NEW: thin CLI wrapper
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs      # Clap routing + env handling
    │       └── commands/
    │           ├── mod.rs
    │           ├── dictionary.rs  # dictionary subcommand
    │           └── dimension.rs   # dimension subcommand
    ├── ndp-gold-ddl/        # Existing: Gold DDL generation
    └── ndp-validate/        # Existing: config validation
```

### Key Design Decisions

#### D1: ndp-lib functions take parsed structs, not file paths

```rust
// ndp-lib/src/dictionary/mod.rs

/// Sync stream configs to data_dictionary tables.
/// Caller decides where configs come from (files, etcd, test fixtures).
pub async fn sync_dictionary(
    streams: &[StreamDictionaryEntry],
    db: &(impl DbClient + Send + Sync),
    options: &SyncOptions,
) -> Result<SyncReport, NdpLibError> { ... }
```

**Rationale**: Enables CLI to load from files today, MCP server to load from etcd later, tests to use fixtures always. Zero changes to ndp-lib when config source changes.

#### D2: DbClient trait shared across crates

```rust
// ndp-lib/src/db.rs

#[async_trait]
pub trait DbClient: Send + Sync {
    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64>;
    async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>;
    async fn batch_execute(&self, sql: &str) -> Result<()>;
}

pub struct PostgresClient { ... }

impl PostgresClient {
    pub async fn connect(url: &str, timeout_secs: u64) -> Result<Self> { ... }
}
```

ndp-gold-ddl currently has its own `DbClient` trait. OPS-001 extracts it to ndp-lib. ndp-gold-ddl becomes a consumer.

#### D3: Parameterized SQL, not string concatenation

Current Bash:
```bash
echo "INSERT INTO data_dictionary.streams (stream_id, description) VALUES ('$stream_id', '$description');"
```

OPS-001 Rust:
```rust
db.execute(
    "INSERT INTO data_dictionary.streams (stream_id, description, version, enabled, retention_days)
     VALUES ($1, $2, $3, $4, $5)
     ON CONFLICT (stream_id) DO UPDATE SET description = $2, version = $3, enabled = $4, retention_days = $5",
    &[&entry.stream_id, &entry.description, &entry.version, &entry.enabled, &entry.retention_days],
).await?;
```

#### D4: deploy.sh integration via fallback pattern

deploy.sh already has this pattern at line 1177:

```bash
sync_dimension() {
    if command -v ndp &> /dev/null; then
        ndp dimension sync "$dimension_id" --config "$config_file" --source "$source_file"
    else
        # Fallback to direct SQL import
        import_dimension_sql "$config_file" "$source_file" "$strategy"
    fi
}
```

OPS-001 extends this pattern to `sync_to_data_dictionary()`:

```bash
sync_to_data_dictionary() {
    if command -v ndp &> /dev/null; then
        ndp dictionary sync --config-dir "$CONFIG_STREAMS_DIR" \
            --db-url "$TIMESCALE_URL" \
            ${DRY_RUN:+--dry-run}
    else
        # Existing Bash implementation (preserved as fallback)
        _sync_to_data_dictionary_bash
    fi
}
```

#### D5: Environment-aware connection

```rust
// ndp-cli/src/main.rs
#[derive(Parser)]
struct Cli {
    /// Database URL (or set TIMESCALE_URL / NDP_TIMESCALE_URL)
    #[arg(long, env = "TIMESCALE_URL")]
    db_url: Option<String>,

    /// Environment: integration or pi
    #[arg(long, env = "DEPLOY_ENV", default_value = "pi")]
    env: String,

    /// Config directory (defaults based on env)
    #[arg(long)]
    config_dir: Option<PathBuf>,
}

impl Cli {
    fn resolve_config_dir(&self) -> PathBuf {
        self.config_dir.clone().unwrap_or_else(|| {
            match self.env.as_str() {
                "integration" => PathBuf::from("config/integration/base/streams"),
                _ => PathBuf::from("config/base/streams"),
            }
        })
    }

    fn resolve_db_url(&self) -> String {
        self.db_url.clone().unwrap_or_else(|| {
            match self.env.as_str() {
                "integration" => "postgresql://postgres:postgres@localhost:5432/ndp".into(),
                _ => "postgresql://postgres:postgres@timescaledb:5432/ndp".into(),
            }
        })
    }
}
```

---

## Proposed Command Map (V1.1 - V2.0)

This is the entity/verb structure designed as a journey. V1.1.9 builds the first two entities. Future versions add entities and verbs as extensions, never refactors.

### Entity/Verb Matrix

| Entity | Verbs | V1.1.9 (ops-001) | V1.2 | V1.3 | V2.0 |
|--------|-------|:-:|:-:|:-:|:-:|
| **dictionary** | `sync`, `status`, `diff` | sync | | diff | |
| **dimension** | `sync`, `list`, `status` | sync | | | |
| **stream** | `sync`, `list`, `validate`, `status` | | sync | | |
| **domain** | `sync`, `list`, `validate` | | | sync | |
| **gold** | `generate`, `apply`, `diff`, `status` | | | | |
| **job** | `create`, `list`, `status`, `pause`, `resume` | | create, list | | |
| **pattern** | `scan`, `list`, `status`, `export` | | scan | | |
| **event** | `detect`, `list`, `status`, `configure` | | | configure | |
| **config** | `validate`, `diff`, `push`, `pull` | | | push, pull | |
| **migration** | `run`, `status`, `plan` | | | | run |

### Verb Semantics (Standard)

| Verb | Meaning | Idempotent | Side Effects |
|------|---------|:----------:|:------------:|
| `sync` | Ensure target matches source (upsert) | Yes | DB writes |
| `list` | Display entities from DB or config | Yes | None (read-only) |
| `status` | Show operational health/state | Yes | None (read-only) |
| `validate` | Check config correctness without applying | Yes | None |
| `diff` | Show what would change (dry-run variant) | Yes | None |
| `generate` | Produce SQL/DDL without executing | Yes | None |
| `apply` | Execute generated SQL/DDL | No | DB writes |
| `create` | Create a new entity | No | DB writes |
| `pause` / `resume` | Toggle entity state | No | DB writes |
| `push` / `pull` | Transfer config to/from etcd | No | etcd writes |

### Command Examples (Full Journey)

```bash
# V1.1.9 (ops-001) - what we build now
ndp dictionary sync --config-dir config/base/streams
ndp dictionary sync --config-dir config/base/streams --dry-run
ndp dimension sync sensor-locations --config config/base/dimensions/sensor-locations/config.json --source data.csv

# V1.2 - add-on (no refactor)
ndp job create correlation-scan --domain indoor-air-quality
ndp job list
ndp job status correlation-scan
ndp pattern scan --domain indoor-air-quality --window 30d
ndp pattern list --min-confidence 0.7

# V1.3 - add-on (no refactor)
ndp config push --config-dir config/base/streams  # files -> etcd
ndp config pull --output-dir /tmp/config-snapshot  # etcd -> files
ndp dictionary sync --from-etcd                    # etcd as source
ndp event configure --domain indoor-air-quality

# V2.0 - add-on (no refactor)
ndp migration run --plan v2-schema-evolution
ndp migration status
```

---

## Architecture Patterns (Agent Reference)

All agents adding commands to `ndp` CLI MUST follow these patterns.

### Pattern 1: Library-First

All operational logic lives in `ndp-lib`. The CLI and MCP server are thin wrappers.

```
ndp-lib (business logic)
  ├── consumed by: ndp-cli (user-facing CLI)
  ├── consumed by: ndp-mcp-server (MCP tools, future)
  └── consumed by: tests (unit + integration)
```

**Rule**: If you can't unit test a function without a running database, it's in the wrong layer. SQL generation and config parsing are pure functions in ndp-lib. Only `DbClient::execute()` touches the database.

### Pattern 2: Trait-Based Dependencies

```rust
// CORRECT: function takes trait references
pub async fn sync_dictionary(
    streams: &[StreamDictionaryEntry],
    db: &(impl DbClient + Send + Sync),
) -> Result<SyncReport> { ... }

// WRONG: function takes concrete types or file paths
pub async fn sync_dictionary(
    config_dir: &Path,        // Ties to filesystem
    db_url: &str,             // Ties to specific DB
) -> Result<SyncReport> { ... }
```

### Pattern 3: Structured Output

All sync/mutation operations return a report struct:

```rust
pub struct SyncReport {
    pub entity: String,           // "dictionary", "dimension", etc.
    pub items_processed: usize,
    pub items_created: usize,
    pub items_updated: usize,
    pub items_deleted: usize,
    pub errors: Vec<SyncError>,
    pub duration: Duration,
}
```

CLI formats this for human output. MCP server returns it as JSON.

### Pattern 4: Error Handling

```rust
#[derive(thiserror::Error, Debug)]
pub enum NdpLibError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),

    #[error("Config not found: {path}")]
    ConfigNotFound { path: String },

    #[error("Config parse error: {message}")]
    ConfigParse { message: String },

    #[error("Sync failed for {entity}: {reason}")]
    SyncFailed { entity: String, reason: String },
}
```

### Pattern 5: Adding a New Command (Checklist)

When adding `ndp <entity> <verb>`:

1. **ndp-lib**: Add `src/<entity>/mod.rs` with the core function
2. **ndp-lib**: Function takes parsed structs + `&impl DbClient`
3. **ndp-lib**: Function returns `Result<SyncReport>` (or appropriate report type)
4. **ndp-lib**: Unit tests with mock DbClient
5. **ndp-cli**: Add `src/commands/<entity>.rs` with clap subcommand
6. **ndp-cli**: Wire config loading (FileSystemLoader) and DB connection
7. **Integration test**: Verify against docker-compose.integration.yml
8. **deploy.sh**: Add fallback pattern (`if command -v ndp`)
9. **Command map**: Update the entity/verb matrix in this document

### Pattern 6: Config Source Abstraction

```rust
// The ConfigLoader trait makes config source swappable
pub trait ConfigLoader: Send + Sync {
    fn load_stream_configs(&self) -> Result<Vec<StreamConfig>>;
    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig>;
    fn load_dimension_config(&self, dim_id: &str) -> Result<DimensionConfig>;
}

// V1.1.9: Files
pub struct FileSystemConfigLoader { config_dir: PathBuf }

// V1.3 (future): etcd
// pub struct EtcdConfigLoader { client: ConfigClient }

// Tests: In-memory
// pub struct MockConfigLoader { configs: HashMap<String, StreamConfig> }
```

---

## Integration Environment

OPS-001 is tested against the existing integration environment (`docker-compose.integration.yml`):

| Service | Container | Port | Role |
|---------|-----------|------|------|
| etcd | integration-etcd | 2379 | Config store |
| TimescaleDB | integration-timescaledb | 5432 | Silver + Gold + Dictionary |
| mosquitto | integration-mosquitto | 1883 | MQTT broker |
| air-quality-app | integration-air-quality | 8080 | Bronze + Silver ETL |
| ndp-mcp-server | integration-mcp-server | 9100 | MCP interface |
| grafana | integration-grafana | 3000 | Dashboards |

### Testing Strategy

```bash
# 1. Start integration stack
docker compose -f docker-compose.integration.yml up -d

# 2. Wait for health
docker compose -f docker-compose.integration.yml ps  # all healthy

# 3. Run current Bash sync (capture baseline)
DEPLOY_ENV=integration ./deploy.sh sync-dictionary
# Capture: psql dump of data_dictionary tables

# 4. Run new Rust sync
DEPLOY_ENV=integration ndp dictionary sync --config-dir config/base/streams --db-url postgresql://postgres:postgres@localhost:5432/ndp
# Compare: psql dump matches baseline

# 5. Run full deploy cycle
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json
# Verify: all phases complete, DB state correct
```

### Golden Master Approach

Capture current Bash output as golden masters (same pattern as FE-002):

```
.test/golden-master/ops-001/
├── dictionary_sync_air-quality.sql     # Expected SQL for air-quality stream
├── dictionary_sync_outdoor-weather.sql # Expected SQL for outdoor-weather stream
├── dictionary_sync_full.sql            # Expected full dictionary sync SQL
└── dimension_sync_sensor-locations.sql # Expected dimension import SQL
```

Rust implementation must produce SQL that, when executed, results in identical DB state. (Exact SQL may differ due to parameterization; DB state is the contract.)

---

## deploy.sh Functions Affected

| Function | Lines | Action | Replacement |
|----------|-------|--------|-------------|
| `sync_to_data_dictionary()` | 375-838 (~460) | Replace with ndp call + Bash fallback | `ndp dictionary sync` |
| `import_dimension_sql()` | 1107-1163 (~55) | Becomes Bash fallback only | `ndp dimension sync` |
| `sync_dimension()` | 1166-1196 (~30) | Already has `command -v ndp` pattern | Update ndp call args |
| `sync_dimensions()` | 1198-1260 (~60) | Thin loop; calls sync_dimension | Unchanged (delegates) |
| `sync_domains_to_data_dictionary()` | 841-1040 (~200) | Bash fallback; ndp call in future | **Not in ops-001 scope** |
| `yaml_get()` and helpers | 120-370 (~250) | Still needed for remaining Bash functions | Unchanged |
| `handle_dictionary()` (apply phase 9) | ~20 | Calls `sync_to_data_dictionary()` | Unchanged (delegates) |
| `handle_dimensions()` (apply phase 8) | ~20 | Calls `sync_dimensions()` | Unchanged (delegates) |

**Lines removed from active Bash codepath**: ~545 (dictionary) + ~85 (dimension fallback) = ~630
**Lines retained as fallback**: Same code, wrapped in `else` branch of `command -v ndp` check

---

## Dependencies

### From Existing Codebase

| Dependency | Status | Notes |
|------------|--------|-------|
| ndp-gold-ddl DbClient pattern | Exists | Extract to ndp-lib |
| ndp-gold-ddl ConfigLoader trait | Exists | Inform ndp-lib design |
| ndp-types crate | Exists | Shared types |
| docker-compose.integration.yml | Exists | Integration test target |
| deploy.sh `sync_dimension()` fallback pattern | Exists (line 1177) | Extend to dictionary |
| tokio-postgres | workspace dep | DB connectivity |
| clap | To add | CLI framework |
| csv (crate) | To add | Dimension CSV parsing |

### Cargo Workspace Changes

```toml
# Add to Cargo.toml [workspace].members:
"crates/ndp-lib",
"tools/ndp-cli"
```

---

## Implementation Phases

### Phase A: Crate Scaffolding (Foundation)

| Task | Description |
|------|-------------|
| Create `crates/ndp-lib/` with Cargo.toml | Workspace member, depends on ndp-types, tokio-postgres |
| Define `DbClient` trait in ndp-lib | Extract from ndp-gold-ddl pattern |
| Define `ConfigLoader` trait in ndp-lib | Inform from ndp-gold-ddl pattern |
| Define `SyncReport` struct | Standardized operation output |
| Define `NdpLibError` enum | Standardized error types |
| Create `tools/ndp-cli/` with Cargo.toml | Depends on ndp-lib, clap |
| Implement clap entity/verb routing | `ndp <entity> <verb> [args]` |
| Verify `ndp --help` works | Scaffold complete |

**Exit Criteria**: `cargo build` succeeds for both crates. `ndp --help` shows entities.

### Phase B: Dictionary Sync

| Task | Description |
|------|-------------|
| Analyze `sync_to_data_dictionary()` line by line | Document every SQL statement and config field used |
| Implement `FileSystemConfigLoader` for streams | Load + parse all stream configs from directory |
| Implement `ndp_lib::dictionary::sync_dictionary()` | Parameterized SQL, transactional, returns SyncReport |
| Implement `ndp_lib::dictionary::generate_sql()` | Pure function for dry-run mode |
| Unit tests with mock DbClient | Cover all stream config variations |
| Golden master: capture current Bash SQL output | Baseline for comparison |
| Integration test against docker-compose stack | DB state matches Bash baseline |
| Wire into `ndp-cli` `dictionary sync` command | CLI flags: --config-dir, --db-url, --dry-run |

**Exit Criteria**: `ndp dictionary sync` produces identical DB state to `sync_to_data_dictionary()`.

### Phase C: Dimension Sync

| Task | Description |
|------|-------------|
| Analyze `import_dimension_sql()` and `sync_dimension()` | Document SQL + CSV import behavior |
| Implement `ndp_lib::dimension::sync_dimension()` | CSV parse + DB import, parameterized |
| Support truncate_and_load strategy | Match existing behavior |
| Unit tests | Cover CSV parsing, column mapping |
| Integration test | Dimension data matches Bash baseline |
| Wire into `ndp-cli` `dimension sync` command | CLI flags: --config, --source, --strategy |

**Exit Criteria**: `ndp dimension sync` produces identical DB state to `import_dimension_sql()`.

### Phase D: deploy.sh Integration + Documentation

| Task | Description |
|------|-------------|
| Update `sync_to_data_dictionary()` in deploy.sh | Add `command -v ndp` check with Bash fallback |
| Update `sync_dimension()` in deploy.sh | Update ndp call arguments to match CLI |
| Verify `DEPLOY_ENV=integration ./deploy.sh apply` | Full end-to-end deploy cycle |
| Verify `DEPLOY_ENV=pi ./deploy.sh apply` | Pi deploy cycle (Docker build) |
| Add ndp-cli to Docker build | Include in same build container as other tools |
| Write architecture patterns document | ops-001-08: Agent reference patterns |
| Write command map document | ops-001-07: Full V1.1-V2.0 entity/verb map |
| Update CLAUDE.md | Reference new patterns and command map |

**Exit Criteria**: deploy.sh works identically with and without ndp binary. Documentation complete.

---

## Resource Constraints

### Build

- ndp-cli is built in the same Docker container as ndp-gold-ddl and ndp-validate
- No new build infrastructure required
- Shares workspace dependencies (tokio-postgres, serde, etc.)

### Runtime

- ndp CLI is invoked during deploy only (not a daemon)
- Memory: < 50 MB peak during sync
- Duration: Dictionary sync < 10s for all streams; dimension sync < 5s per dimension

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| SQL output differs subtly from Bash | Medium | High | Golden master tests; compare DB state, not SQL text |
| Config parsing differences (YAML edge cases) | Low | Medium | Test with all existing configs; Bash fallback retained |
| Docker build adds compile time | Low | Low | Shares workspace; incremental builds |
| ndp binary not found on Pi | Low | Low | Bash fallback preserved; deploy.sh works either way |
| Scope creep into other deploy.sh functions | Medium | Medium | Strict scope: dictionary + dimension only |

---

## V1.2 Handoff Requirements

For OPS-001 to be complete, V1.2 agents must be able to:

- [ ] Add a new entity to ndp CLI by following Pattern 5 (checklist)
- [ ] Implement the business logic in ndp-lib using Pattern 1 (library-first) and Pattern 2 (trait-based)
- [ ] Test against the integration environment without modifying infrastructure
- [ ] Reference the command map to know where their feature fits
- [ ] Use the DbClient and ConfigLoader traits without modification

**Contract**: The `DbClient` trait, `ConfigLoader` trait, `SyncReport` struct, and entity/verb routing in clap are the interface contract between OPS-001 and all future ops features.

---

## References

- [Deployment Declaratives](../../docs/procedures/DEPLOYMENT-DECLARATIVES.md) - Manifest format
- [Release Policy](../../docs/procedures/RELEASE-POLICY.md) - Versioning standard
- [FE-001 SCOPE.md](../fe-001/SCOPE.md) - Prior feature scope (pattern reference)
- [ndp-gold-ddl lib.rs](../../tools/ndp-gold-ddl/src/lib.rs) - Library-first pattern
- [ndp-gold-ddl ConfigLoader](../../tools/ndp-gold-ddl/src/config/loader.rs) - Trait pattern
- [ndp-gold-ddl DbClient](../../tools/ndp-gold-ddl/src/db/client.rs) - DB trait pattern
- [MCP ConfigStore](../../core/ndp-mcp-server/src/etcd/mod.rs) - etcd trait pattern
- [deploy.sh](../../deploy/pi/deploy.sh) - Current deployment script
- [docker-compose.integration.yml](../../docker-compose.integration.yml) - Integration environment
- [Deployment Research](../../product/research/deployment/) - 10-document analysis

### Research Documents Informing This Scope

- [00-SYNTHESIS-AND-RECOMMENDATIONS.md](../../product/research/deployment/00-SYNTHESIS-AND-RECOMMENDATIONS.md)
- [08-STANDARD-VERBS-AND-MCP-INTEGRATION.md](../../product/research/deployment/08-STANDARD-VERBS-AND-MCP-INTEGRATION.md)
- [09-STEPWISE-MIGRATION-PLAN.md](../../product/research/deployment/09-STEPWISE-MIGRATION-PLAN.md)
