# BUG-002 Architecture: Domain Objectives Sync Migration to Rust

> **Bug:** BUG-002 (ops-002)
> **Status:** Proposed
> **Author:** ndp-architect
> **Date:** 2026-02-06
> **Scope:** `ndp domain sync` -- new entity/verb command in ndp CLI

---

## Problem Summary

`sync_domains_to_data_dictionary()` in `deploy/pi/deploy.sh` (lines 883-1086) is ~200 lines of Bash that generates raw SQL via string interpolation to populate `data_dictionary.domains`, `data_dictionary.domain_streams`, `data_dictionary.objectives`, and `data_dictionary.constraints`. It has been dead code since FE-002 standardized configs to JSON: the Bash only looks for `domain.yaml` and uses YAML key paths (`domain.id`, `domain.streams[0].stream_id`) that do not match the flat JSON structure.

The fix migrates this sync operation to the Rust ndp-lib/ndp-cli toolchain, following the exact pattern established by `ndp dictionary sync` (ops-001 Phase B) and `ndp dimension sync` (ops-001 Phase C).

---

## ADR-BUG002-001: Domain Sync Belongs in ndp-lib, Not ndp-gold-ddl

### Status

Proposed

### Context

An earlier analysis suggested adding `--objectives` and `--apply` flags to `ndp-gold-ddl`. This was incorrect for two reasons:

1. **ndp-gold-ddl generates DDL** (CREATE VIEW, CREATE PROCEDURE, CREATE MATERIALIZED VIEW). Domain sync is **metadata population** -- it writes rows to existing `data_dictionary.*` tables. These are fundamentally different operations.

2. **ndp-gold-ddl's `DbClient` trait** (in `tools/ndp-gold-ddl/src/db.rs`) only exposes `query()` for read-only sync planning. **ndp-lib's `DbClient` trait** (in `crates/ndp-lib/src/db.rs`) already provides `execute()` and `batch_execute()` for write operations.

The existing entity sync modules demonstrate the pattern:

| Module | What It Populates | Where It Lives |
|--------|-------------------|----------------|
| `dictionary` | `data_dictionary.streams`, `fields`, `sources`, `entity_schemas`, `silver_*` | `crates/ndp-lib/src/dictionary/` |
| `dimension` | `silver.entity_context` (dimension tables) | `crates/ndp-lib/src/dimension/` |
| **`domain`** | **`data_dictionary.domains`, `domain_streams`, `objectives`, `constraints`** | **`crates/ndp-lib/src/domain/`** |

### Decision

Domain sync is implemented as a new module in `ndp-lib` and exposed as `ndp domain sync` in the CLI. It follows the identical structural pattern as `dictionary` and `dimension`.

### Consequences

**Easier:**
- Reuses the full ops-001 infrastructure: `DbClient`, `ConfigLoader`, `SyncReport`, `SyncOptions`, `NdpLibError`, CLI routing, `command -v ndp` deploy.sh pattern.
- London TDD with `MockDbClient` -- no database needed for unit tests.
- deploy.sh integration is a copy of the proven `command -v ndp` pattern (already at lines 384-396 and 1218-1232).

**Harder:**
- Nothing. This is the same operation class as dictionary sync, using the same infrastructure.

### Alternatives Considered

**A. Add to ndp-gold-ddl.** Rejected. Wrong tool -- ndp-gold-ddl generates DDL, not metadata. Its `DbClient` lacks write methods.

**B. Fix the Bash in-place.** Rejected. The Bash uses string-interpolated SQL (injection-vulnerable), has no tests, and the ops-001 Rust infrastructure already exists for exactly this class of operation.

---

## Module Layout

```
crates/ndp-lib/src/
  lib.rs              # MODIFIED: add `pub mod domain;`
  config.rs           # MODIFIED: add `load_domain_configs()` to ConfigLoader trait
  domain/             # NEW
    mod.rs            # sync_domain() + London TDD tests
    types.rs          # DomainSyncEntry, ObjectiveSyncEntry, StreamMappingEntry, ConstraintSyncEntry
    sql.rs            # SQL constants (parameterized)

tools/ndp-cli/src/
  main.rs             # MODIFIED: add Domain to Commands enum
  commands/
    mod.rs            # MODIFIED: add `pub mod domain;`
    domain.rs         # NEW: ndp domain sync command
```

---

## Type Hierarchy (domain/types.rs)

Types are modeled as "sync entry" structs that map directly to database tables. The domain config is parsed separately (by `ConfigLoader`), then converted to these entry types before being passed to the sync function. This follows the dictionary pattern where `StreamConfig` is converted to `StreamDictionaryEntry` via `convert.rs`.

For domain sync the JSON structure is simple enough that no separate conversion step is needed -- the entry types can deserialize directly from `domain.json`.

```rust
/// A single domain configuration ready for sync.
/// Maps to one row in `data_dictionary.domains` plus child rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSyncEntry {
    /// Domain identifier (e.g., "indoor-air-quality").
    pub id: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Streams referenced by this domain.
    #[serde(default)]
    pub streams: Vec<StreamMappingEntry>,

    /// Objectives (target metrics to optimize toward).
    #[serde(default)]
    pub objectives: Vec<ObjectiveSyncEntry>,

    /// Constraints (conditions that must be met for actions).
    /// Optional section -- many domains have none.
    #[serde(default)]
    pub constraints: Vec<ConstraintSyncEntry>,
}
```

### StreamMappingEntry

Maps to `data_dictionary.domain_streams`.

```rust
/// A stream-to-domain mapping with role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMappingEntry {
    /// Stream identifier (e.g., "air-quality").
    pub stream_id: String,

    /// Short alias for aligned view column prefixes.
    pub alias: String,

    /// Role of this stream in the domain.
    /// Valid: "primary", "context", "actuator", "constraint".
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "primary".to_string()
}
```

### ObjectiveSyncEntry

Maps to `data_dictionary.objectives`.

```rust
/// A domain objective (target metric to optimize).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveSyncEntry {
    /// Objective identifier (e.g., "healthy_co2").
    pub id: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Target specification.
    pub target: ObjectiveTarget,

    /// Priority: "low", "medium", "high", "critical".
    #[serde(default = "default_priority")]
    pub priority: String,
}

fn default_priority() -> String {
    "medium".to_string()
}

/// Target metric and condition for an objective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveTarget {
    /// Source stream (e.g., "air-quality").
    pub stream: String,

    /// Metric name (e.g., "co2", "pm25").
    pub metric: String,

    /// Comparison condition: "<", ">", "<=", ">=", "==", "!=", "between".
    pub condition: String,

    /// Threshold value (or lower bound for "between").
    pub threshold: f64,

    /// Upper threshold for "between" condition.
    #[serde(default)]
    pub threshold_upper: Option<f64>,

    /// Unit of measurement (e.g., "ppm", "ug/m3").
    #[serde(default)]
    pub unit: Option<String>,
}
```

### ConstraintSyncEntry

Maps to `data_dictionary.constraints`.

```rust
/// A constraint on domain actions (V1.3+ action framework).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintSyncEntry {
    /// Constraint identifier.
    pub id: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Constraint stream (source of constraint data).
    pub stream: String,

    /// Metric to check.
    pub metric: String,

    /// Comparison condition.
    pub condition: String,

    /// Threshold value.
    pub threshold: f64,

    /// Unit of measurement.
    #[serde(default)]
    pub unit: Option<String>,
}
```

### Deserialization Compatibility

The types above deserialize directly from the existing `config/domains/indoor-air-quality/domain.json` without any conversion layer. The JSON structure is:

```json
{
  "id": "indoor-air-quality",
  "description": "...",
  "streams": [{"stream_id": "air-quality", "alias": "indoor", "role": "primary"}],
  "objectives": [{"id": "healthy_co2", "target": {"stream": "...", ...}, "priority": "high"}],
  "constraints": []
}
```

This maps 1:1 to `DomainSyncEntry`. No `convert.rs` bridge is needed (unlike dictionary sync where `StreamConfig` has a different shape than `StreamDictionaryEntry`).

---

## ConfigLoader Extension (config.rs)

### Trait Extension

Add `load_domain_configs()` to the `ConfigLoader` trait.

```rust
pub trait ConfigLoader: Send + Sync {
    fn load_stream_configs(&self) -> Result<Vec<StreamConfig>>;
    fn load_dimension_config(&self, dimension_id: &str) -> Result<DimensionConfig>;

    // NEW
    fn load_domain_configs(&self) -> Result<Vec<DomainConfig>>;
}
```

Where `DomainConfig` is a config-layer struct that deserializes `domain.json`. Since the domain JSON maps directly to `DomainSyncEntry`, the `DomainConfig` type alias can simply be `DomainSyncEntry` re-exported, or a separate thin struct. Given the direct mapping, use the sync entry types directly as the config types -- there is no structural mismatch to bridge.

**Design decision:** The `ConfigLoader` method returns `Vec<DomainSyncEntry>` directly, avoiding a separate `DomainConfig` struct. This is justified because:
1. The JSON deserializes directly into `DomainSyncEntry` without transformation.
2. The dictionary module needed separate config vs. entry types because `StreamConfig` has a fundamentally different shape than `StreamDictionaryEntry`. Domain config does not have this mismatch.
3. Adding an unnecessary intermediate type creates boilerplate with zero value.

### FileSystemConfigLoader Extension

```rust
pub struct FileSystemConfigLoader {
    streams_dir: PathBuf,
    dimensions_dir: PathBuf,
    domains_dir: PathBuf,   // NEW
}
```

#### Constructor Changes

**`new()` -- add `domains_dir` parameter:**

```rust
pub fn new(
    streams_dir: impl Into<PathBuf>,
    dimensions_dir: impl Into<PathBuf>,
    domains_dir: impl Into<PathBuf>,
) -> Self
```

This is a breaking change to the constructor signature. All callers (CLI commands) must be updated. There are exactly 2 callers:
- `tools/ndp-cli/src/commands/dictionary.rs` line 49
- `tools/ndp-cli/src/commands/dimension.rs` line 49 (indirectly -- it does not use the loader)

**`from_base_dir()` -- add domains convention:**

```rust
pub fn from_base_dir(base_dir: impl Into<PathBuf>) -> Self {
    let base: PathBuf = base_dir.into();
    Self {
        streams_dir: base.join("streams"),
        dimensions_dir: base.join("dimensions"),
        domains_dir: base.join("../domains"),  // domains at peer level
    }
}
```

**Important:** The domains directory is NOT under `config/base/` -- it is at `config/domains/`. The stream configs are at `config/base/streams/`. This means `from_base_dir("config/base")` cannot derive the domains directory as a simple subdirectory.

Two options:

**Option A: `from_base_dir` uses parent directory to resolve domains.**

```rust
// config/base -> parent is config/ -> config/domains/
domains_dir: base.join("..").join("domains"),
```

**Option B: Add a separate `set_domains_dir()` method.**

```rust
pub fn with_domains_dir(mut self, dir: impl Into<PathBuf>) -> Self {
    self.domains_dir = dir.into();
    self
}
```

**Decision: Option B (builder pattern).** The domains directory path is already known at the call site (`CONFIG_DOMAINS_DIR` in deploy.sh, `config/domains` in CLI). Hardcoding a parent-directory traversal is fragile and assumes a specific directory layout. The builder pattern keeps each path explicit.

For backwards compatibility, the `new()` constructor keeps two parameters and `domains_dir` defaults to an empty `PathBuf`. When `load_domain_configs()` is called on a loader with no domains_dir set, it returns `NdpLibError::ConfigNotFound`.

**Revised constructor approach:**

```rust
pub struct FileSystemConfigLoader {
    streams_dir: PathBuf,
    dimensions_dir: PathBuf,
    domains_dir: Option<PathBuf>,   // Optional, set via builder
}

impl FileSystemConfigLoader {
    /// Existing constructor (unchanged signature for backwards compat)
    pub fn new(streams_dir: impl Into<PathBuf>, dimensions_dir: impl Into<PathBuf>) -> Self {
        Self {
            streams_dir: streams_dir.into(),
            dimensions_dir: dimensions_dir.into(),
            domains_dir: None,
        }
    }

    /// Set the domains directory.
    pub fn with_domains_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.domains_dir = Some(dir.into());
        self
    }

    /// Existing from_base_dir (unchanged)
    pub fn from_base_dir(base_dir: impl Into<PathBuf>) -> Self { ... }
}
```

### Domain Config Discovery

Domain configs live at `config/domains/*/domain.json`. Discovery follows the same pattern as `discover_stream_ids()`:

```rust
fn discover_domain_ids(&self) -> Result<Vec<String>> {
    let domains_dir = self.domains_dir.as_ref().ok_or_else(|| {
        NdpLibError::ConfigNotFound {
            path: "domains_dir not configured".to_string(),
        }
    })?;

    if !domains_dir.exists() {
        return Err(NdpLibError::ConfigNotFound {
            path: domains_dir.display().to_string(),
        });
    }

    let mut ids = Vec::new();
    for entry in std::fs::read_dir(domains_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let config_path = path.join("domain.json");
            if config_path.exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    ids.push(name.to_string());
                }
            }
        }
    }
    ids.sort();
    Ok(ids)
}
```

---

## SQL Constants (domain/sql.rs)

All SQL uses parameterized queries (`$1`, `$2`, ...). No string concatenation.

### UPSERT Domain

```rust
pub const UPSERT_DOMAIN: &str = "\
INSERT INTO data_dictionary.domains (domain_id, description, stream_count, config_path, updated_at) \
VALUES ($1, $2, $3, $4, NOW()) \
ON CONFLICT (domain_id) DO UPDATE SET \
description = EXCLUDED.description, \
stream_count = EXCLUDED.stream_count, \
config_path = EXCLUDED.config_path, \
updated_at = NOW()";
```

### DELETE + INSERT Domain Streams

```rust
pub const DELETE_DOMAIN_STREAMS: &str = "\
DELETE FROM data_dictionary.domain_streams WHERE domain_id = $1";

pub const INSERT_DOMAIN_STREAM: &str = "\
INSERT INTO data_dictionary.domain_streams (domain_id, stream_id, alias, role) \
VALUES ($1, $2, $3, $4)";
```

### DELETE + INSERT Objectives

```rust
pub const DELETE_OBJECTIVES: &str = "\
DELETE FROM data_dictionary.objectives WHERE domain_id = $1";

pub const INSERT_OBJECTIVE: &str = "\
INSERT INTO data_dictionary.objectives \
(objective_id, domain_id, description, target_stream, target_metric, \
condition, threshold, threshold_upper, unit, priority) \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)";
```

### DELETE + INSERT Constraints

```rust
pub const DELETE_CONSTRAINTS: &str = "\
DELETE FROM data_dictionary.constraints WHERE domain_id = $1";

pub const INSERT_CONSTRAINT: &str = "\
INSERT INTO data_dictionary.constraints \
(constraint_id, domain_id, description, constraint_stream, constraint_metric, \
condition, threshold, unit) \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)";
```

### Sync Status Update

```rust
pub const UPDATE_SYNC_STATUS_DOMAIN_SUCCESS: &str = "\
UPDATE data_dictionary.sync_status \
SET completed_at = NOW(), \
    status = 'success', \
    domains_synced = $1, \
    objectives_synced = $2 \
WHERE status = 'running' AND completed_at IS NULL";
```

---

## Sync Strategy (domain/mod.rs)

### Function Signature

```rust
pub async fn sync_domain(
    domains: &[DomainSyncEntry],
    db: &impl DbClient,
    options: &SyncOptions,
) -> Result<SyncReport>
```

This follows the dictionary pattern exactly: takes parsed structs (not file paths), uses `&impl DbClient` for testability, returns `SyncReport`.

### Transaction Flow

```
BEGIN
  INSERT sync_status (running)
  FOR EACH domain:
    UPSERT data_dictionary.domains           (parent row)
    DELETE data_dictionary.domain_streams     (clear children)
    INSERT data_dictionary.domain_streams     (re-insert children)
    DELETE data_dictionary.objectives         (clear children)
    INSERT data_dictionary.objectives         (re-insert children)
    DELETE data_dictionary.constraints        (clear children)
    INSERT data_dictionary.constraints        (re-insert children)
  UPDATE sync_status (success, counts)
COMMIT
```

### Why UPSERT Parent + DELETE/INSERT Children

The parent `domains` table uses UPSERT because the domain_id is stable and we want to preserve `created_at`. The child tables (`domain_streams`, `objectives`, `constraints`) use DELETE+INSERT (full refresh per domain) because:

1. Child rows do not have independent `created_at` timestamps worth preserving.
2. Full refresh is simpler and matches the Bash behavior.
3. The CASCADE foreign keys ensure FK integrity during DELETE.
4. The operation is per-domain (not global), so it does not affect other domains.

This is the same strategy the Bash implementation uses (UPSERT domain, DELETE+INSERT children).

### FK Ordering

The `domains` parent must be inserted BEFORE children. Children can be deleted in any order because they all have ON DELETE CASCADE from the parent. The sequence for each domain is:

1. UPSERT `domains` (ensures parent exists)
2. DELETE `domain_streams` WHERE domain_id = $1
3. INSERT `domain_streams` rows
4. DELETE `objectives` WHERE domain_id = $1
5. INSERT `objectives` rows
6. DELETE `constraints` WHERE domain_id = $1
7. INSERT `constraints` rows

### Error Handling

Per-domain errors are collected in `SyncReport.errors` as non-fatal `SyncError` entries. If one domain fails (e.g., a parse error), other domains still sync. This matches the dictionary sync pattern.

Fatal errors (transaction failure, database connection loss) propagate as `NdpLibError::Database` and abort the entire sync.

### Dry Run

When `options.dry_run` is true, return a `SyncReport` with counts computed from the input data without executing any SQL. Same pattern as `dictionary::build_dry_run_report()`.

### Collected Counts

```rust
#[derive(Debug, Default)]
struct DomainSyncCounts {
    domains: i32,
    streams: i32,
    objectives: i32,
    constraints: i32,
}
```

The `SyncReport` maps these as:
- `entity`: `"domain"`
- `items_processed`: number of domains
- `items_created`: domains + streams + objectives + constraints inserted
- `items_updated`: 0 (UPSERT domain counted in created for simplicity)
- `items_deleted`: 0 (DELETEs are internal to the full refresh, not reported)

---

## CLI Command (commands/domain.rs)

### Structure

```rust
/// Domain operations.
#[derive(Args)]
pub struct DomainArgs {
    #[command(subcommand)]
    pub command: DomainCommands,
}

#[derive(Subcommand)]
pub enum DomainCommands {
    /// Sync domain configs to the data_dictionary tables.
    Sync {
        /// Domain config directory containing domain subdirectories.
        /// Defaults to config/domains (production) or config/integration/domains.
        #[arg(long)]
        config_dir: Option<PathBuf>,

        /// Print what would be synced without executing.
        #[arg(long)]
        dry_run: bool,
    },
}
```

### Run Function

```rust
pub async fn run(
    args: DomainArgs,
    base_config_dir: &Path,
    db_url: &str,
) -> Result<(), Box<dyn std::error::Error>>
```

The run function:

1. Resolves the domains directory (explicit `--config-dir`, or convention-based from `base_config_dir`).
2. Creates a `FileSystemConfigLoader` with `.with_domains_dir(domains_dir)`.
3. Calls `loader.load_domain_configs()` to get `Vec<DomainSyncEntry>`.
4. If `--dry-run`: calls `sync_domain()` with `SyncOptions { dry_run: true }` and prints the report.
5. If live: connects to DB via `PostgresClient::connect()`, calls `sync_domain()`, prints the report.

### Domains Directory Resolution

```rust
let domains_dir = config_dir.unwrap_or_else(|| {
    // Domains are at config/domains (production) or config/integration/domains
    match env.as_str() {
        "integration" => PathBuf::from("config/integration/domains"),
        _ => PathBuf::from("config/domains"),
    }
});
```

This matches the `CONFIG_DOMAINS_DIR` variable in deploy.sh (lines 67, 75).

### Main.rs Changes

```rust
#[derive(Subcommand)]
enum Commands {
    /// Data dictionary operations.
    Dictionary(commands::dictionary::DictionaryArgs),

    /// Dimension table operations.
    Dimension(commands::dimension::DimensionArgs),

    /// Domain configuration operations.
    Domain(commands::domain::DomainArgs),       // NEW
}
```

And in the match:

```rust
Commands::Domain(args) => {
    commands::domain::run(args, &config_dir, &db_url).await?;
}
```

### NoOpDbClient

The domain CLI command defines a local `NoOpDbClient` for dry-run mode, identical to the one in `commands/dictionary.rs`. This duplication is acceptable for now; a future refactor (ops-003 or later) could extract it to a shared module.

---

## deploy.sh Integration

### Current State (Broken)

Lines 883-1086: `sync_domains_to_data_dictionary()` generates raw SQL via string interpolation from `domain.yaml` files. This is dead code -- no `domain.yaml` files exist.

### Target State

Replace the function body with the `command -v ndp` fallback pattern, identical to lines 384-396 (dictionary sync) and 1218-1232 (dimension sync).

```bash
sync_domains_to_data_dictionary() {
    log "Syncing Domain Objectives to Data Dictionary..."

    # Check if TimescaleDB is running
    until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
        warn "Waiting for TimescaleDB to be ready..."
        sleep 2
    done

    # Use Rust CLI if available (BUG-002 fix)
    local ndp_tool=""
    if command -v ndp &> /dev/null; then
        ndp_tool="ndp"
    elif [ -x "/opt/ndp/bin/ndp" ]; then
        ndp_tool="/opt/ndp/bin/ndp"
    elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
        ndp_tool="$REPO_ROOT/target/release/ndp"
    elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
        ndp_tool="$REPO_ROOT/target/debug/ndp"
    fi

    if [ -n "$ndp_tool" ]; then
        log "Using Rust CLI for domain sync: $ndp_tool"
        if $ndp_tool domain sync \
            --config-dir "$CONFIG_DOMAINS_DIR" \
            --db-url "$TIMESCALE_URL"; then
            log "Domain objectives sync successful (Rust CLI)"
        else
            error "Domain objectives sync failed (Rust CLI)"
            return 1
        fi
    else
        warn "ndp CLI not found. Build with: cargo build --release -p ndp-cli"
        warn "Skipping domain objectives sync."
        return 0
    fi
}
```

**Key design decisions:**
- The Bash fallback (old YAML-based SQL generation) is removed entirely, not preserved. It was broken and will never work with JSON configs.
- If `ndp` is not available, the function logs a warning and returns 0 (non-fatal). This allows deploys to succeed even if the Rust CLI is not built yet.
- `CONFIG_DOMAINS_DIR` is already defined at lines 67/75 of deploy.sh for integration and production environments.

---

## Error Handling

### Error Mapping

| Scenario | NdpLibError Variant | Behavior |
|----------|---------------------|----------|
| Domains directory not found | `ConfigNotFound { path }` | Abort sync, return error |
| No domain.json in subdirectory | Skipped silently (same as stream discovery) | Warning logged, not counted |
| JSON parse error | `ConfigParse { message }` | Logged as `SyncError`, domain skipped |
| DB connection failure | `Database(String)` | Fatal, abort entire sync |
| INSERT fails for one domain | `Database(String)` | Logged as `SyncError`, domain skipped, other domains continue |
| Transaction COMMIT fails | `Database(String)` | Fatal, abort |

### Error Propagation in Sync

```rust
for domain in domains {
    match sync_single_domain(db, domain, &mut counts).await {
        Ok(()) => {}
        Err(e) => {
            tracing::error!(domain_id = %domain.id, error = %e, "Failed to sync domain");
            errors.push(SyncError {
                item: domain.id.clone(),
                message: format!("Domain sync failed: {}", e),
            });
        }
    }
}
```

This matches the dictionary pattern where per-stream errors are collected but do not abort the sync.

---

## Data Flow Diagram

```
                       config/domains/
                           |
          +----------------+----------------+
          |                                 |
  indoor-air-quality/              (future domains)/
    domain.json                      domain.json
          |                                 |
          v                                 v
  +-------------------------------------------------+
  |         FileSystemConfigLoader                   |
  |   .with_domains_dir("config/domains")            |
  |   .load_domain_configs()                         |
  |     discover_domain_ids()                        |
  |     for each: read + parse domain.json           |
  +-------------------------------------------------+
                       |
                       v
           Vec<DomainSyncEntry>
                       |
                       v
  +-------------------------------------------------+
  |            sync_domain()                         |
  |   &[DomainSyncEntry], &impl DbClient, &options  |
  |                                                  |
  |   BEGIN                                          |
  |   for each domain:                               |
  |     UPSERT domains (parent)                      |
  |     DELETE + INSERT domain_streams (children)    |
  |     DELETE + INSERT objectives (children)        |
  |     DELETE + INSERT constraints (children)       |
  |   COMMIT                                         |
  +-------------------------------------------------+
                       |
                       v
                 SyncReport
                       |
                       v
  +-------------------------------------------------+
  |         CLI Output (commands/domain.rs)           |
  |                                                  |
  |   Domain sync complete:                          |
  |     Domains synced: 1                            |
  |     Streams mapped: 4                            |
  |     Objectives:     6                            |
  |     Constraints:    0                            |
  |     Duration:       0.03s                        |
  +-------------------------------------------------+
```

---

## Test Architecture (London TDD)

### Mock Boundary

The `MockDbClient` records all SQL calls (query string + debug-formatted params). Tests assert on the recorded calls without requiring a database. This is the same mock used by dictionary and dimension tests.

### Test Plan

```
domain/mod.rs tests
|
+-- Basic Sync
|   +-- test_sync_empty_domains             (0 domains -> BEGIN/COMMIT only)
|   +-- test_sync_single_domain             (1 domain -> UPSERT domains row)
|   +-- test_sync_domain_with_streams       (4 stream mappings inserted)
|   +-- test_sync_domain_with_objectives    (6 objectives inserted)
|   +-- test_sync_domain_with_constraints   (constraints inserted when present)
|
+-- SQL Correctness
|   +-- test_upsert_domain_uses_on_conflict (UPSERT uses ON CONFLICT)
|   +-- test_parameterized_sql              (all queries use $N placeholders)
|   +-- test_objective_threshold_types      (threshold as NUMERIC, threshold_upper nullable)
|
+-- Ordering
|   +-- test_parent_before_children         (UPSERT domain before INSERT streams/objectives)
|   +-- test_delete_before_insert           (DELETE children before INSERT)
|   +-- test_transaction_wrapping           (first=BEGIN, last=COMMIT)
|
+-- Error Handling
|   +-- test_domain_error_non_fatal         (one domain fails, others succeed)
|   +-- test_sync_report_counts             (verify items_processed, items_created)
|
+-- Dry Run
|   +-- test_dry_run_no_sql_executed        (db.calls() is empty)
|   +-- test_dry_run_report_has_counts      (report reflects what would happen)
|
+-- Multi-Domain
|   +-- test_sync_multiple_domains          (2+ domains, each independent)
|
+-- Real Config Parsing
    +-- test_parse_real_domain_config        (include_str! on indoor-air-quality/domain.json)
    +-- test_domain_config_roundtrip         (deserialize + verify all fields)
```

### Test Helpers

```rust
fn make_minimal_domain(id: &str) -> DomainSyncEntry {
    DomainSyncEntry {
        id: id.to_string(),
        description: Some("Test domain".to_string()),
        streams: vec![],
        objectives: vec![],
        constraints: vec![],
    }
}

fn make_stream_mapping(stream_id: &str, alias: &str, role: &str) -> StreamMappingEntry {
    StreamMappingEntry {
        stream_id: stream_id.to_string(),
        alias: alias.to_string(),
        role: role.to_string(),
    }
}

fn make_objective(id: &str, metric: &str, condition: &str, threshold: f64) -> ObjectiveSyncEntry {
    ObjectiveSyncEntry {
        id: id.to_string(),
        description: Some(format!("Test objective {}", id)),
        target: ObjectiveTarget {
            stream: "air-quality".to_string(),
            metric: metric.to_string(),
            condition: condition.to_string(),
            threshold,
            threshold_upper: None,
            unit: Some("ppm".to_string()),
        },
        priority: "medium".to_string(),
    }
}
```

---

## Integration Test Plan

Integration tests verify the full pipeline against the Docker Compose stack (`docker-compose.integration.yml`).

### Prerequisites

- TimescaleDB running with `005_domain_objectives.sql` applied.
- The `data_dictionary` schema and all four tables exist.

### Tests

```
integration/
+-- test_domain_sync_creates_rows           (sync -> query tables -> verify row counts)
+-- test_domain_sync_upsert_idempotent      (sync twice -> same row counts)
+-- test_domain_sync_cascade_delete         (remove objective from config -> re-sync -> gone)
+-- test_domain_sync_preserves_other_domains (sync domain A -> sync domain B -> A untouched)
```

These tests are gated behind `#[cfg(feature = "integration")]` and use the `TIMESCALE_URL` environment variable.

---

## Backwards Compatibility

| Concern | Guarantee |
|---------|-----------|
| Config files | Zero changes to `domain.json`. The types deserialize from the existing format. |
| Database schema | Zero changes to `005_domain_objectives.sql`. Tables already exist with all needed columns. |
| deploy.sh interface | `sync_domains_to_data_dictionary` function name unchanged. Internal body replaced. |
| Other CLI commands | `ndp dictionary sync` and `ndp dimension sync` work identically. The `new()` constructor signature is unchanged. |
| ndp-gold-ddl | Zero changes. Domain sync is entirely within ndp-lib/ndp-cli. |

---

## File Change Map

### New Files

| File | Purpose |
|------|---------|
| `crates/ndp-lib/src/domain/mod.rs` | `sync_domain()` function + London TDD tests |
| `crates/ndp-lib/src/domain/types.rs` | `DomainSyncEntry`, `ObjectiveSyncEntry`, `StreamMappingEntry`, `ConstraintSyncEntry` |
| `crates/ndp-lib/src/domain/sql.rs` | SQL constants (UPSERT_DOMAIN, DELETE/INSERT children) |
| `tools/ndp-cli/src/commands/domain.rs` | `ndp domain sync` CLI command |

### Modified Files

| File | Changes |
|------|---------|
| `crates/ndp-lib/src/lib.rs` | Add `pub mod domain;` (1 line) |
| `crates/ndp-lib/src/config.rs` | Add `load_domain_configs()` to `ConfigLoader` trait, add `domains_dir` field to `FileSystemConfigLoader`, add `with_domains_dir()` builder, add `discover_domain_ids()` private method |
| `tools/ndp-cli/src/commands/mod.rs` | Add `pub mod domain;` (1 line) |
| `tools/ndp-cli/src/main.rs` | Add `Domain` variant to `Commands` enum, add match arm |
| `deploy/pi/deploy.sh` | Replace `sync_domains_to_data_dictionary()` body (~200 lines of Bash SQL generation replaced with ~30 lines of `command -v ndp` pattern) |

### Unchanged Files (Verified)

| File | Why Unchanged |
|------|---------------|
| `config/domains/indoor-air-quality/domain.json` | Config already has all needed fields |
| `deploy/pi/init-scripts/005_domain_objectives.sql` | Tables already exist |
| `crates/ndp-lib/src/dictionary/` | Separate module, no coupling |
| `crates/ndp-lib/src/dimension/` | Separate module, no coupling |
| `tools/ndp-gold-ddl/` | Different tool, different purpose |

### Estimated Scope

| Metric | Estimate |
|--------|----------|
| New files | 4 |
| Modified files | 5 |
| Lines added | ~450 (types ~80, sql ~40, sync logic ~150, tests ~120, CLI ~60) |
| Lines removed | ~170 (deploy.sh Bash SQL generation) |
| Net change | +~280 lines |
| New tests | ~16 unit tests + 4 integration tests |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `DomainSyncEntry` does not parse existing `domain.json` | Low | High | Test with `include_str!` on the real config file. The types match the JSON structure by design. |
| `FileSystemConfigLoader` breaking change | Low | Medium | `new()` signature unchanged. `domains_dir` added via builder pattern. Existing callers unaffected. |
| Missing `domains_synced` column in `sync_status` | Low | Low | Column added by `005_domain_objectives.sql`. If missing, sync_status update is non-fatal. |
| deploy.sh regression when `ndp` not available | Low | Medium | Function returns 0 with warning when CLI not found. No deploy failure. |
| `threshold` as `f64` vs `NUMERIC` | Low | Low | `tokio_postgres` handles `f64` -> `NUMERIC` conversion. The Bash used string interpolation of numeric values. |

---

## Implementation Order

### Phase 1: Types and SQL (No Logic)

Create `domain/types.rs` and `domain/sql.rs`. Write the `test_parse_real_domain_config` test to verify deserialization against the real `domain.json`.

### Phase 2: Sync Logic with Tests

Implement `sync_domain()` in `domain/mod.rs` with full London TDD test suite. Write the `MockDbClient` tests that verify SQL call ordering and parameterization.

### Phase 3: ConfigLoader Extension

Add `load_domain_configs()` to `ConfigLoader` trait and `FileSystemConfigLoader`. Add `with_domains_dir()` builder method. Write config discovery tests.

### Phase 4: CLI Command

Create `commands/domain.rs`, wire into `main.rs`. Test manually with `--dry-run`.

### Phase 5: deploy.sh Integration

Replace `sync_domains_to_data_dictionary()` body with `command -v ndp` pattern.

### Phase 6: Integration Testing

Run against `docker-compose.integration.yml` to verify end-to-end behavior.

Each phase is a separate commit. Phases 1-4 can be combined into a single commit if preferred (they are tightly coupled). Phase 5 (deploy.sh) is always a separate commit for clean rollback.
