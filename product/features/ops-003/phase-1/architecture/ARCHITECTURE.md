# OPS-003 Phase 1 Architecture: Gold Migration (v1.1.14)

> **Author**: ndp-architect
> **Date**: 2026-02-07
> **Status**: Proposed
> **Scope**: ops-003-01 through ops-003-04

---

## 1. Module Layout

### 1.1 Target Directory Structure

```
crates/ndp-lib/src/
  lib.rs                          # Add: pub mod gold;
  db.rs                           # Add: NoOpDbClient
  error.rs                        # Add: Gold-specific error variants
  gold/
    mod.rs                        # Public API: generate(), sync(), recreate()
    config/
      mod.rs                      # Re-exports from types.rs, loader.rs, domain.rs
      types.rs                    # GoldEtlConfig, StreamConfig (Gold-specific), Action, etc.
      loader.rs                   # Gold ConfigLoader trait + FileSystemConfigLoader
      domain.rs                   # DomainConfig, StreamRef, AlignmentConfig, etc.
    generators/
      mod.rs                      # Re-exports all generators
      aligned_view.rs             # AlignedViewGenerator
      classification.rs           # ClassificationSyncer, generate_classification_sql
      column_builder.rs           # ColumnBuilder (internal helper)
      constants.rs                # NDP_ENTITY_COLUMN, GOLD_SCHEMA, SILVER_SCHEMA
      continuous_aggregate.rs     # ContinuousAggregateGenerator
      events.rs                   # EventsGenerator, EventsConfig, IEventsGenerator
      join_builder.rs             # JoinBuilder (internal helper)
      null_handler.rs             # NullHandler trait + implementations
      refresh_policy.rs           # RefreshPolicyGenerator
      state_transitions.rs        # StateTransitionGenerator, TransitionConfig
    planner/
      mod.rs                      # Re-exports from sync.rs
      sync.rs                     # SyncPlanner, SyncPlan, CaAction, CaPlan
    registry/
      mod.rs                      # FeatureRegistry + re-exports
      lag.rs                      # LagFeatureGenerator
      rolling.rs                  # RollingFeatureGenerator
      trait_def.rs                # FeatureGenerator trait, FeatureConfig, SqlColumn
      trend.rs                    # TrendFeatureGenerator
    validation/
      mod.rs                      # Re-exports from config_validator.rs
      config_validator.rs         # ConfigValidator, parse_granularity, parse_window, etc.
    error.rs                      # GoldDdlError, ErrorCode, Result (Gold-specific)
```

Total: 29 source files move from `tools/ndp-gold-ddl/src/` to `crates/ndp-lib/src/gold/`.

### 1.2 Source-to-Destination File Mapping

| ndp-gold-ddl source | ndp-lib destination |
|---|---|
| `src/config/mod.rs` | `src/gold/config/mod.rs` |
| `src/config/types.rs` | `src/gold/config/types.rs` |
| `src/config/loader.rs` | `src/gold/config/loader.rs` |
| `src/config/domain.rs` | `src/gold/config/domain.rs` |
| `src/generators/mod.rs` | `src/gold/generators/mod.rs` |
| `src/generators/aligned_view.rs` | `src/gold/generators/aligned_view.rs` |
| `src/generators/classification.rs` | `src/gold/generators/classification.rs` |
| `src/generators/column_builder.rs` | `src/gold/generators/column_builder.rs` |
| `src/generators/constants.rs` | `src/gold/generators/constants.rs` |
| `src/generators/continuous_aggregate.rs` | `src/gold/generators/continuous_aggregate.rs` |
| `src/generators/events.rs` | `src/gold/generators/events.rs` |
| `src/generators/join_builder.rs` | `src/gold/generators/join_builder.rs` |
| `src/generators/null_handler.rs` | `src/gold/generators/null_handler.rs` |
| `src/generators/refresh_policy.rs` | `src/gold/generators/refresh_policy.rs` |
| `src/generators/state_transitions.rs` | `src/gold/generators/state_transitions.rs` |
| `src/planner/mod.rs` | `src/gold/planner/mod.rs` |
| `src/planner/sync.rs` | `src/gold/planner/sync.rs` |
| `src/registry/mod.rs` | `src/gold/registry/mod.rs` |
| `src/registry/lag.rs` | `src/gold/registry/lag.rs` |
| `src/registry/rolling.rs` | `src/gold/registry/rolling.rs` |
| `src/registry/trait_def.rs` | `src/gold/registry/trait_def.rs` |
| `src/registry/trend.rs` | `src/gold/registry/trend.rs` |
| `src/validation/mod.rs` | `src/gold/validation/mod.rs` |
| `src/validation/config_validator.rs` | `src/gold/validation/config_validator.rs` |
| `src/error.rs` | `src/gold/error.rs` |
| `src/db/mod.rs` | NOT moved (replaced by ndp_lib::db) |
| `src/db/client.rs` | NOT moved (replaced by ndp_lib::db) |
| `src/db/queries.rs` | `src/gold/db.rs` (CaChecker, CaInfo, PostgresCaChecker only) |
| `src/lib.rs` | `src/gold/mod.rs` (rewritten as public API) |
| `src/main.rs` | NOT moved (stays in ndp-gold-ddl as thin wrapper) |

### 1.3 Integration Test Files

| ndp-gold-ddl test source | ndp-lib destination |
|---|---|
| `tests/aligned_view_tests.rs` | `tests/gold/aligned_view_tests.rs` |
| `tests/state_transitions_tests.rs` | `tests/gold/state_transitions_tests.rs` |
| `tests/objectives_tests.rs` | `tests/gold/objectives_tests.rs` |
| `tests/golden_master_test.rs` | `tests/gold/golden_master_test.rs` |
| `tests/ops002_config_driven_tests.rs` | `tests/gold/ops002_config_driven_tests.rs` |
| `tests/ops002_source_scan_tests.rs` | `tests/gold/ops002_source_scan_tests.rs` |
| `tests/ops002_hardcoding_tests.rs` | `tests/gold/ops002_hardcoding_tests.rs` |
| `tests/fixtures/mod.rs` | `tests/gold/fixtures/mod.rs` |
| `tests/fixtures/phase_c.rs` | `tests/gold/fixtures/phase_c.rs` |
| `tests/fixtures/energy_monitoring.rs` | `tests/gold/fixtures/energy_monitoring.rs` |

### 1.4 Module Hierarchy and Visibility

```rust
// crates/ndp-lib/src/lib.rs
pub mod gold;      // NEW in v1.1.14

// crates/ndp-lib/src/gold/mod.rs
pub mod config;       // Gold-specific config types and loader
pub mod generators;   // DDL generators
pub mod planner;      // SyncPlanner for idempotent deployment
pub mod registry;     // Feature type registry
pub mod validation;   // Gold config validation
pub mod error;        // GoldDdlError, ErrorCode

// Private: db module (CaChecker) is pub(crate) since it's implementation detail
// used only by planner. External callers use ndp_lib::DbClient.
pub mod db;           // CaChecker trait, CaInfo, PostgresCaChecker

// Re-exports at gold/mod.rs level for convenience
pub use config::{
    Action, AlignedStream, AlignmentConfig, ConfigLoader as GoldConfigLoader,
    DomainConfig as GoldDomainConfig, FileSystemConfigLoader as GoldFileSystemConfigLoader,
    GoldEtlConfig, JoinStrategy, NullHandling, ObjectiveConfig, Priority,
    StreamConfig as GoldStreamConfig, StreamRef, StreamRole, StreamType, TargetConfig,
};
pub use db::{CaChecker, CaInfo, PostgresCaChecker};
pub use error::{GoldDdlError, Result as GoldResult};
pub use generators::{
    AlignedViewGenerator, ContinuousAggregateGenerator, EventsGenerator,
    RefreshPolicyGenerator, StateTransitionGenerator, TransitionConfig,
};
pub use planner::{CaAction, SyncPlan, SyncPlanner};
pub use registry::{FeatureConfig, FeatureGenerator, FeatureRegistry, SqlColumn};
pub use validation::{validate_gold_config, ConfigValidator, granularity_to_suffix,
    parse_granularity, parse_window};
```

**Naming convention for re-exports**: Gold-specific types that collide with existing ndp-lib names use prefixed aliases (`GoldStreamConfig`, `GoldDomainConfig`, `GoldConfigLoader`, `GoldFileSystemConfigLoader`, `GoldResult`). This prevents ambiguity for consumers importing both `ndp_lib::config::*` and `ndp_lib::gold::*`.

---

## 2. Dependency Analysis

### 2.1 ndp-gold-ddl Current Dependencies

| Dependency | Version in ndp-gold-ddl | Workspace? | Moves to ndp-lib? | Rationale |
|---|---|---|---|---|
| `clap` | `4` (derive, env) | Yes | NO | CLI-only; stays in ndp-gold-ddl binary |
| `serde` | `1.0` (derive) | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `serde_json` | `1.0` | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `thiserror` | `1.0` | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `tracing` | `0.1` | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `tracing-subscriber` | `0.3` (env-filter) | Yes (workspace) | NO | Binary-only (logging init) |
| `tokio` | `1` (rt-multi-thread, macros) | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `tokio-postgres` | `0.7` (with-chrono-0_4) | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `async-trait` | `0.1` | Yes (workspace) | Already in ndp-lib | Already a dependency |

**Dev-dependencies:**

| Dependency | Version in ndp-gold-ddl | Moves to ndp-lib? | Rationale |
|---|---|---|---|
| `tempfile` | `3.8` | Already in ndp-lib as `3` | Pin to `3` (workspace compat) |
| `pretty_assertions` | `1.4` | YES (dev-dep) | Used by gold tests |
| `mockall` | `0.11` | YES (dev-dep) | Used by gold tests. Note: 0.11, not 0.12 |
| `sha2` | `0.10` | YES (dev-dep) | Used by golden_master_test |

### 2.2 Version Pinning Strategy

All dependencies in ndp-gold-ddl already align with workspace versions or are compatible. The key consideration is `mockall`:

- ndp-gold-ddl uses `mockall = "0.11"`.
- SCOPE.md suggests `mockall = "0.12"` for ndp-lib.
- **Decision**: Use `0.12` in ndp-lib. The mockall 0.11 to 0.12 API is compatible for our use cases (simple `#[automock]` on traits). If any tests break, they will need minor syntax adjustments during migration. Using the newer version prevents technical debt.

### 2.3 Exact ndp-lib Dependency Additions (v1.1.14)

```toml
# crates/ndp-lib/Cargo.toml — additions for v1.1.14

[dev-dependencies]
# Existing:
# tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
# tempfile = "3"

# NEW for gold module tests:
mockall = "0.12"
pretty_assertions = "1"
sha2 = "0.10"
```

No new runtime dependencies needed. All runtime dependencies used by gold generators (`serde`, `serde_json`, `thiserror`, `tracing`, `tokio`, `tokio-postgres`, `async-trait`) are already in ndp-lib's `[dependencies]`.

### 2.4 Feature Flags

No feature flags needed. The gold module is unconditionally compiled. Rationale:
- The gold module has no heavy native dependencies (unlike, e.g., linking to FANN).
- On Raspberry Pi, the single `ndp` binary includes all capabilities.
- Adding feature flags would complicate the build for no measurable benefit. The gold module adds negligible compile time and binary size.

---

## 3. DbClient Trait Unification

### 3.1 Current State: Two DbClient Traits

**ndp_lib::db::DbClient** (the authority):

```rust
#[async_trait]
pub trait DbClient: Send + Sync {
    async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>;
    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64>;
    async fn batch_execute(&self, sql: &str) -> Result<()>;
}
```

**ndp_gold_ddl::db::client::DbClient** (the duplicate):

```rust
#[async_trait]
pub trait DbClient: Send + Sync {
    async fn query(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<tokio_postgres::Row>, DbError>;
}
```

### 3.2 Differences

| Aspect | ndp_lib::DbClient | ndp_gold_ddl::DbClient |
|---|---|---|
| Methods | `query`, `execute`, `batch_execute` | `query` only |
| Error type | `NdpLibError` (via `Result`) | `DbError` (gold-local enum) |
| `ToSql` import | `tokio_postgres::types::ToSql` | `tokio_postgres::types::ToSql` |
| Row type | `tokio_postgres::Row` | `tokio_postgres::Row` |

ndp_lib's trait is a **strict superset** of ndp_gold_ddl's trait. The gold code only uses `query()`.

### 3.3 Unified Trait Design

**The unified trait is ndp_lib::db::DbClient as-is.** No changes to the trait itself.

The gold module's `CaChecker` and `PostgresCaChecker` need to be adapted to use `ndp_lib::db::DbClient` instead of the local `DbClient`. This requires two changes:

1. **Error mapping**: Gold's `CaChecker` trait returns `Result<_, DbError>`. After migration, it returns `Result<_, GoldDdlError>`, with the `DbClient.query()` error (`NdpLibError`) mapped via `GoldDdlError::DatabaseError(e.to_string())`.

2. **PostgresCaChecker generic bound**: Changes from `C: ndp_gold_ddl::db::DbClient` to `C: ndp_lib::DbClient`.

### 3.4 CaChecker Adaptation

**Before (in ndp-gold-ddl):**

```rust
use super::client::{DbClient, DbError};

pub struct PostgresCaChecker<C: DbClient> {
    client: C,
}

#[async_trait]
impl<C: DbClient + Send + Sync> CaChecker for PostgresCaChecker<C> {
    async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, DbError> {
        let rows = self.client.query(query, &[&schema, &name]).await?;
        // ...
    }
}
```

**After (in ndp_lib::gold::db):**

```rust
use crate::db::DbClient;  // ndp_lib::db::DbClient
use super::error::GoldDdlError;

pub struct PostgresCaChecker<C: DbClient> {
    client: C,
}

#[async_trait]
impl<C: DbClient + Send + Sync> CaChecker for PostgresCaChecker<C> {
    async fn ca_exists(&self, schema: &str, name: &str)
        -> Result<bool, GoldDdlError>
    {
        let rows = self.client.query(query, &[&schema, &name])
            .await
            .map_err(|e| GoldDdlError::DatabaseError(e.to_string()))?;
        // ...
    }
}
```

### 3.5 CaChecker Trait Error Type Change

The `CaChecker` trait currently returns `Result<_, DbError>`. In the migrated version, it returns `Result<_, GoldDdlError>`. This is the key signature change:

```rust
// Before: uses local DbError
#[async_trait]
pub trait CaChecker: Send + Sync {
    async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, DbError>;
    async fn get_ca_info(&self, schema: &str, name: &str) -> Result<Option<CaInfo>, DbError>;
    async fn list_cas_in_schema(&self, schema: &str) -> Result<Vec<CaInfo>, DbError>;
    async fn refresh_policy_exists(&self, schema: &str, name: &str) -> Result<bool, DbError>;
}

// After: uses GoldDdlError
#[async_trait]
pub trait CaChecker: Send + Sync {
    async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError>;
    async fn get_ca_info(&self, schema: &str, name: &str) -> Result<Option<CaInfo>, GoldDdlError>;
    async fn list_cas_in_schema(&self, schema: &str) -> Result<Vec<CaInfo>, GoldDdlError>;
    async fn refresh_policy_exists(&self, schema: &str, name: &str) -> Result<bool, GoldDdlError>;
}
```

This eliminates the separate `DbError` enum entirely. `GoldDdlError::DatabaseError(String)` absorbs its role.

### 3.6 PostgresClient: No Changes Needed

ndp_lib's `PostgresClient` already has `connect()`, `query()`, `execute()`, and `batch_execute()`. The gold code only calls `query()` through the `CaChecker`. Since `PostgresCaChecker<C: DbClient>` is generic over `DbClient`, it works with ndp_lib's `PostgresClient` directly.

### 3.7 NoOpDbClient Addition

Add to `crates/ndp-lib/src/db.rs`:

```rust
/// No-op database client for dry-run mode.
///
/// All methods return empty results. Used when database operations
/// should be skipped (e.g., `--dry-run` mode in CLI commands).
pub struct NoOpDbClient;

#[async_trait]
impl DbClient for NoOpDbClient {
    async fn query(&self, _query: &str, _params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        Ok(vec![])
    }

    async fn execute(&self, _query: &str, _params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
        Ok(0)
    }

    async fn batch_execute(&self, _sql: &str) -> Result<()> {
        Ok(())
    }
}
```

**Behavioral difference from current ndp-cli NoOpDbClient**: The current CLI NoOpDbClient uses `unreachable!()` (panics if called). The new shared version returns empty results instead. This is safer: if code accidentally calls DB methods in dry-run mode, it gets empty data rather than a panic. Any code that should never call DB in dry-run mode should enforce that at the call site, not in the client.

**Impact on existing ndp-cli code**: The 3 copies of `NoOpDbClient` in `commands/domain.rs`, `commands/dictionary.rs`, and `commands/dimension.rs` continue to work in v1.1.14. In v1.1.16, they are replaced with `use ndp_lib::db::NoOpDbClient`.

---

## 4. Config Types Strategy

### 4.1 Principle: Gold Config Types Stay Gold-Specific

Gold config types move to `ndp_lib::gold::config`, NOT merged with `ndp_lib::config::StreamConfig`. The types serve fundamentally different purposes:

| Type | Location After Migration | Purpose | Key Distinguishing Fields |
|---|---|---|---|
| `ndp_lib::config::StreamConfig` | Unchanged | Dictionary/dimension/domain sync | `description`, `version`, `enabled`, `retention_days`, `sources[]`, `entity_schemas` |
| `ndp_lib::gold::config::StreamConfig` | NEW | Gold DDL generation | `stream_type`, `fields` (simplified), `gold_etl`, `silver_etl` (simplified) |
| `platform_core::config::StreamConfig` | Unchanged (core crate) | Runtime ingestion | Everything (100+ fields) |

### 4.2 Gold Config Types Inventory

These types move as-is from `ndp_gold_ddl::config::*` to `ndp_lib::gold::config::*`:

**From `config/types.rs`:**
- `GoldEtlConfig` -- Gold ETL top-level config
- `AggregatesConfig` -- Time-bucket aggregation settings
- `FieldMetricsConfig` -- Per-field metric list
- `FeaturesConfig` -- Feature computation config (lag, rolling, trend, transitions)
- `LagConfig`, `RollingConfig`, `TrendConfig`, `TransitionsConfig` -- Feature sub-configs
- `RefreshPolicyConfig` -- Continuous aggregate refresh settings
- `StreamConfig` (Gold-specific) -- Simplified stream config for DDL generation
- `FieldConfig` -- Simplified field definition
- `SilverEtlConfig` (Gold-specific) -- Simplified Silver ETL reference
- `TimestampConfig` (Gold-specific) -- Timestamp field config
- `Action` -- Sync/Recreate enum
- `VALID_METRICS: &[&str]` -- Valid aggregate metric names
- `VALID_ROLLING_STATS: &[&str]` -- Valid rolling statistic names

**From `config/domain.rs`:**
- `DomainConfig` (Gold-specific) -- Domain alignment configuration
- `StreamRef` -- Stream reference within a domain
- `StreamRole` -- Primary/Context/Actuator enum
- `AlignmentConfig` -- View alignment settings
- `AlignedStream` -- Resolved stream in alignment
- `JoinStrategy` -- Full outer/Left/Inner enum
- `NullHandling` -- Carry forward/Interpolate/Preserve enum
- `ObjectiveConfig` -- Domain objective for pattern detection
- `TargetConfig` -- Objective target specification
- `Priority` -- High/Medium/Low enum
- `StreamType` -- Observation/StateEvent/Forecast/Dimension enum

**From `config/loader.rs`:**
- `ConfigLoader` trait (Gold-specific) -- `load_stream_config()`, `load_domain_config()`
- `FileSystemConfigLoader` -- File system implementation
- `default_loader()` -- Factory function
- `resolve_config_dir()` -- Path resolution helper

### 4.3 Collision Avoidance

The Gold `ConfigLoader` trait has a different signature from ndp_lib's `ConfigLoader`:

| Trait | Signature |
|---|---|
| `ndp_lib::config::ConfigLoader` | `load_stream_configs() -> Vec<StreamConfig>`, `load_dimension_config()`, `load_domain_configs()` |
| `ndp_lib::gold::config::ConfigLoader` | `load_stream_config(id) -> StreamConfig`, `load_domain_config(id) -> DomainConfig` |

These are different traits with different semantics (bulk load vs. single-entity load). They do not collide because they live in different modules. Re-exported from `gold/mod.rs` as `GoldConfigLoader` to prevent confusion when both are imported.

---

## 5. CLI Architecture

### 5.1 Clap Structure

**New file: `tools/ndp-cli/src/commands/gold.rs`**

```rust
use clap::{Args, Subcommand};
use std::path::Path;

/// Gold layer DDL operations.
#[derive(Args)]
pub struct GoldArgs {
    #[command(subcommand)]
    pub command: GoldCommands,
}

#[derive(Subcommand)]
pub enum GoldCommands {
    /// Generate Gold layer DDL without applying.
    Generate {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Include state transition view DDL.
        #[arg(long)]
        transitions: bool,

        /// Include events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,

        /// Validate config only, do not generate DDL.
        #[arg(long)]
        validate_only: bool,

        /// Skip pre-generation validation.
        #[arg(long)]
        no_validate: bool,
    },

    /// Sync Gold layer (idempotent create-if-not-exists).
    Sync {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Include state transition view DDL.
        #[arg(long)]
        transitions: bool,

        /// Include events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,

        /// Generate DDL without applying to database.
        #[arg(long)]
        dry_run: bool,

        /// Skip pre-sync validation.
        #[arg(long)]
        no_validate: bool,
    },

    /// Recreate Gold layer (drop and create).
    Recreate {
        /// Target stream ID.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Target domain ID.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Generate DDL without applying to database.
        #[arg(long)]
        dry_run: bool,

        /// Skip pre-recreate validation.
        #[arg(long)]
        no_validate: bool,
    },
}
```

### 5.2 Integration into main.rs

```rust
// tools/ndp-cli/src/main.rs

// Add to Cli struct:
#[arg(long, default_value = "10", global = true)]
db_timeout: u64,

// Add to Commands enum:
/// Gold layer DDL operations.
Gold(commands::gold::GoldArgs),

// Add to match:
Commands::Gold(args) => {
    commands::gold::run(args, &config_dir, &db_url, cli.db_timeout).await?;
}
```

### 5.3 Global Flag Flow

The existing global flags (`--db-url`, `--config-dir`, `--env`) are already defined on the `Cli` struct with `global = true`. A new `--db-timeout` global flag is added for gold operations.

```
ndp --db-url $URL --config-dir config/base --db-timeout 10 gold sync --stream air-quality
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                    Global flags (parsed by Cli)                 Subcommand (parsed by GoldArgs)
```

**How globals flow to library calls:**

```
main.rs
  cli.resolve_config_dir() --> config_dir: PathBuf
  cli.resolve_db_url()     --> db_url: String
  cli.db_timeout           --> db_timeout: u64
  |
  commands::gold::run(args, &config_dir, &db_url, db_timeout)
    |
    GoldFileSystemConfigLoader::new(&config_dir)    # config_dir used here
    PostgresClient::connect(&db_url, db_timeout)     # db_url + timeout used here
    PostgresCaChecker::new(postgres_client)
    SyncPlanner::new(&checker, &stream_config)
    generator.generate(gold_etl, action)
```

### 5.4 Flag Mapping from Standalone to Subcommand

| Standalone (ndp-gold-ddl) | Subcommand (ndp gold) | Notes |
|---|---|---|
| `ndp-gold-ddl --config-dir DIR generate --stream X` | `ndp --config-dir DIR gold generate --stream X` | `--config-dir` becomes global |
| `ndp-gold-ddl --database-url URL generate --stream X --action sync` | `ndp --db-url URL gold sync --stream X` | `--database-url` becomes `--db-url` (global), `--action sync` becomes `sync` verb |
| `ndp-gold-ddl --database-url URL generate --stream X --action recreate` | `ndp --db-url URL gold recreate --stream X` | `--action recreate` becomes `recreate` verb |
| `ndp-gold-ddl generate --stream X --transitions` | `ndp gold generate --stream X --transitions` | Unchanged |
| `ndp-gold-ddl generate --domain X --events` | `ndp gold generate --domain X --events` | Unchanged |
| `ndp-gold-ddl validate --stream X` | `ndp gold generate --stream X --validate-only` | `validate` subcommand absorbed into `--validate-only` flag |
| `ndp-gold-ddl --verbose` | `RUST_LOG=debug` or future `--verbose` flag | Verbose via tracing env filter |

### 5.5 Command-to-Library Mapping

```
ndp gold generate --stream X
  --> loader = GoldFileSystemConfigLoader::new(config_dir)
  --> stream_config = loader.load_stream_config(X)
  --> gold_etl = stream_config.gold_etl
  --> generator = ContinuousAggregateGenerator::from_stream_config(&stream_config)
  --> ddl = generator.generate(gold_etl, Action::Sync)
  --> println!("{}", ddl)

ndp gold sync --stream X --db-url URL
  --> loader = GoldFileSystemConfigLoader::new(config_dir)
  --> stream_config = loader.load_stream_config(X)
  --> gold_etl = stream_config.gold_etl
  --> db = PostgresClient::connect(URL, timeout)
  --> checker = PostgresCaChecker::new(db)
  --> planner = SyncPlanner::new(&checker, &stream_config)
  --> plan = planner.plan(gold_etl)
  --> ddl = plan.to_ddl()
  --> println!("{}", ddl)

ndp gold recreate --stream X --db-url URL
  --> (same as sync but with Action::Recreate, no planner needed)
  --> generator = ContinuousAggregateGenerator::from_stream_config(&stream_config)
  --> ddl = generator.generate(gold_etl, Action::Recreate)
  --> println!("{}", ddl)

ndp gold generate --domain X
  --> loader = GoldFileSystemConfigLoader::new(config_dir)
  --> domain_config = loader.load_domain_config(X)
  --> generator = AlignedViewGenerator::new(loader)
  --> ddl = generator.generate(&domain_config, Action::Sync)
  --> println!("{}", ddl)

ndp gold generate --domain X --events
  --> loader = GoldFileSystemConfigLoader::new(config_dir)
  --> domain_config = loader.load_domain_config(X)
  --> generator = EventsGenerator::from_domain_config(&domain_config, Box::new(loader))
  --> ddl = generator.generate(Action::Sync)
  --> println!("{}", ddl)
```

### 5.6 Exit Code Alignment

The existing ndp-cli uses `Result<(), Box<dyn std::error::Error>>` which maps to exit code 0 (success) or 1 (any error). ndp-gold-ddl has more granular exit codes (0, 1, 3). For v1.1.14, the ndp-cli gold command uses the same coarse-grained exit code model as the rest of the CLI. The standalone ndp-gold-ddl thin wrapper preserves the original exit codes for backward compatibility.

---

## 6. Error Handling Strategy

### 6.1 Gold Error Types Remain Separate

`GoldDdlError` moves to `ndp_lib::gold::error` and retains its full richness (ErrorCode enum, structured variants with field/stream_id context). It is NOT merged with `NdpLibError`.

Rationale:
- `GoldDdlError` has domain-specific variants (`InvalidMetric`, `InvalidGranularity`, `FieldNotFound`) that are meaningless to dictionary or dimension sync.
- Merging would either bloat `NdpLibError` with irrelevant variants or lose error fidelity.
- The gold module has an internal `Result<T>` alias for `Result<T, GoldDdlError>`.

### 6.2 Bridging for CLI

The CLI command (`commands/gold.rs`) maps `GoldDdlError` to `Box<dyn std::error::Error>` at the boundary, same as the existing domain/dictionary/dimension commands:

```rust
pub async fn run(
    args: GoldArgs,
    config_dir: &Path,
    db_url: &str,
    db_timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // ... calls ndp_lib::gold::* which return GoldDdlError
    // GoldDdlError implements std::error::Error, so ? auto-converts
}
```

### 6.3 Future: NdpLibError Integration (v1.1.16)

When cross-cutting validation is wired in v1.1.16, `gold::sync()` will call `validate::gold_config()`. If the validate module returns a different error type, `NdpLibError` may gain a `Gold(GoldDdlError)` variant. This is a v1.1.16 concern, not v1.1.14.

---

## 7. Cargo.toml Changes

### 7.1 crates/ndp-lib/Cargo.toml

```toml
# crates/ndp-lib/Cargo.toml
# EXISTING dependencies -- no changes
[dependencies]
ndp-types = { path = "../ndp-types" }
tokio-postgres = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
csv = { workspace = true }

# EXISTING dev-dependencies
[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
tempfile = "3"

# ADD for gold module tests:
mockall = "0.12"
pretty_assertions = "1"
sha2 = "0.10"
```

### 7.2 tools/ndp-gold-ddl/Cargo.toml (After Thin Wrapper Conversion)

```toml
# tools/ndp-gold-ddl/Cargo.toml
[package]
name = "ndp-gold-ddl"
version = "0.1.0"
edition = "2021"
authors = ["Neural Data Platform Team"]
description = "Gold layer DDL generation tool for NDP stream configurations"

[[bin]]
name = "ndp-gold-ddl"
path = "src/main.rs"

[dependencies]
# Retained: CLI and binary infrastructure
clap = { version = "4", features = ["derive", "env"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

# ADD: ndp-lib for all gold logic
ndp-lib = { path = "../../crates/ndp-lib" }

# REMOVE (moved to ndp-lib):
# serde = { version = "1.0", features = ["derive"] }
# serde_json = "1.0"
# thiserror = "1.0"
# tokio-postgres = { version = "0.7", features = ["with-chrono-0_4"] }
# async-trait = "0.1"

[dev-dependencies]
# REMOVE (moved to ndp-lib):
# tempfile = "3.8"
# pretty_assertions = "1.4"
# mockall = "0.11"
# sha2 = "0.10"
```

Note: `serde`, `serde_json`, `thiserror`, `tokio-postgres`, and `async-trait` are removed because the thin wrapper `main.rs` does not use them directly. All type handling flows through `ndp_lib::gold::*`.

### 7.3 tools/ndp-cli/Cargo.toml

```toml
# tools/ndp-cli/Cargo.toml
# NO CHANGES for v1.1.14
# ndp-cli already depends on ndp-lib, which now includes gold module.
# The gold command module only imports from ndp_lib::gold::*.
```

The `ndp-cli` binary does not need direct dependencies on `tokio-postgres` types for the gold command because:
- `PostgresClient::connect()` returns an ndp_lib type.
- `PostgresCaChecker::new()` takes the generic `C: DbClient`.
- All error types come from ndp_lib.

However, `tokio-postgres` is already in ndp-cli's Cargo.toml (used by existing commands), so it stays.

---

## 8. `use` Path Migration

### 8.1 Internal Path Changes

Every file moving from `ndp_gold_ddl` to `ndp_lib::gold` needs its `use crate::` paths updated:

| Old path | New path |
|---|---|
| `use crate::config::*` | `use crate::gold::config::*` |
| `use crate::error::*` | `use crate::gold::error::*` |
| `use crate::generators::*` | `use crate::gold::generators::*` |
| `use crate::planner::*` | `use crate::gold::planner::*` |
| `use crate::registry::*` | `use crate::gold::registry::*` |
| `use crate::validation::*` | `use crate::gold::validation::*` |
| `use crate::db::{DbClient, DbError}` | `use crate::db::DbClient; use crate::gold::error::GoldDdlError` |

### 8.2 Cross-Module References

Within the gold module, references between submodules stay relative:

```rust
// In gold/generators/continuous_aggregate.rs:
use super::constants::GOLD_SCHEMA;           // gold::generators::constants
use crate::gold::config::StreamConfig;       // gold::config
use crate::gold::error::{GoldDdlError, Result}; // gold::error
use crate::gold::registry::FeatureRegistry;  // gold::registry
```

### 8.3 Integration Test Path Changes

Integration tests in `tests/gold/` import from ndp_lib:

```rust
// Before (in ndp-gold-ddl tests):
use ndp_gold_ddl::config::*;
use ndp_gold_ddl::generators::*;

// After (in ndp-lib tests):
use ndp_lib::gold::config::*;
use ndp_lib::gold::generators::*;
```

---

## 9. ndp-gold-ddl Thin Wrapper Design

### 9.1 main.rs After Migration

The standalone binary retains its CLI interface but delegates all logic to ndp_lib::gold:

```rust
// tools/ndp-gold-ddl/src/main.rs (after migration)

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use ndp_lib::gold::{
    Action, GoldConfigLoader, GoldFileSystemConfigLoader, GoldStreamConfig,
    AlignedViewGenerator, ContinuousAggregateGenerator, EventsGenerator,
    StateTransitionGenerator, TransitionConfig,
    PostgresCaChecker, SyncPlanner,
};
use ndp_lib::db::PostgresClient;

// ... same Cli struct and Commands enum ...
// ... same run() function but calling ndp_lib::gold::* ...
```

### 9.2 lib.rs After Migration

```rust
// tools/ndp-gold-ddl/src/lib.rs (after migration)
//! ndp-gold-ddl - Gold layer DDL generation for NDP
//!
//! This crate re-exports from ndp_lib::gold for backward compatibility.
//! New code should depend on ndp-lib directly.

pub use ndp_lib::gold::*;
```

This ensures any external code that depends on `ndp_gold_ddl` as a library (unlikely, but possible) continues to work.

---

## 10. ADR: Library Extraction over Facade

### ADR-003-001: Move Gold Generators into ndp-lib as Library Extraction

#### Status

Proposed

#### Context

After ops-001 and ops-002, NDP has three deployment binaries (`ndp`, `ndp-gold-ddl`, `ndp-validate`) with duplicated infrastructure (DbClient, ConfigLoader, constants). The gold DDL generation logic in `ndp-gold-ddl` needs to be accessible to other modules (e.g., validation calling gold config validation, future MCP server calling gold generation).

Two approaches were considered:
1. **Facade pattern**: Keep logic in ndp-gold-ddl, add ndp-lib facades that call into it.
2. **Library extraction**: Move logic into ndp-lib, make ndp-gold-ddl a thin wrapper.

#### Decision

Use library extraction. The gold generators, planner, registry, config types, validation, and error types move from `tools/ndp-gold-ddl/src/` to `crates/ndp-lib/src/gold/`. The ndp-gold-ddl binary becomes a thin CLI wrapper that imports from ndp-lib.

#### Consequences

**Easier:**
- Cross-module calls within ndp-lib (e.g., `gold::sync()` calling `validate::gold_config()` in v1.1.16).
- Single crate to test for all library logic (`cargo test -p ndp-lib`).
- MCP server and future API server import from one crate.
- Agent debugging: one codebase to investigate when deploy.sh fails.
- Shared infrastructure (DbClient, ConfigLoader) used directly, no adapters needed.

**Harder:**
- Initial migration requires updating 29 files' `use crate::` paths.
- Integration tests must move with the code.
- Temporarily larger PR (29 files + 10 test files + Cargo.toml changes).
- Must verify 376 tests pass after path changes.

#### Alternatives Considered

**A1: Facade pattern (rejected)**

ndp-lib would define thin wrapper functions that call ndp-gold-ddl. This avoids moving code but:
- Creates a circular dependency risk (ndp-lib depends on ndp-gold-ddl).
- Prevents cross-module calls (gold and validate cannot call each other).
- Doubles the API surface (ndp-gold-ddl and ndp-lib both export the same functions).
- Does not eliminate DbClient duplication.

**A2: Depend on ndp-gold-ddl as library (rejected)**

ndp-cli would depend on ndp-gold-ddl directly. This:
- Does not solve the DbClient duplication (ndp-gold-ddl still has its own).
- Makes ndp-gold-ddl both a library and a binary, complicating its Cargo.toml.
- Does not enable cross-module calls with validate.

**A3: Shared traits crate (rejected)**

Extract DbClient and ConfigLoader into a new `ndp-traits` crate. This:
- Adds a crate to the workspace for 2 traits.
- Does not address the gold/validate cross-module call requirement.
- Over-engineering for the current scale.

---

## 11. Migration Sequence

The recommended implementation order within v1.1.14:

### Step 1: Prepare ndp-lib

1. Add `pub mod gold;` to `crates/ndp-lib/src/lib.rs`.
2. Add `NoOpDbClient` to `crates/ndp-lib/src/db.rs`.
3. Add dev-dependencies to `crates/ndp-lib/Cargo.toml` (`mockall`, `pretty_assertions`, `sha2`).
4. Create the empty module structure under `crates/ndp-lib/src/gold/`.

### Step 2: Move Source Files

Use `git mv` for each file to preserve git history:

```bash
# Create directories
mkdir -p crates/ndp-lib/src/gold/{config,generators,planner,registry,validation}
mkdir -p crates/ndp-lib/tests/gold/fixtures

# Move config module
git mv tools/ndp-gold-ddl/src/config/types.rs crates/ndp-lib/src/gold/config/types.rs
git mv tools/ndp-gold-ddl/src/config/loader.rs crates/ndp-lib/src/gold/config/loader.rs
git mv tools/ndp-gold-ddl/src/config/domain.rs crates/ndp-lib/src/gold/config/domain.rs
# (config/mod.rs is rewritten, not moved)

# Move generators
git mv tools/ndp-gold-ddl/src/generators/aligned_view.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/classification.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/column_builder.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/constants.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/events.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/join_builder.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/null_handler.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/refresh_policy.rs crates/ndp-lib/src/gold/generators/
git mv tools/ndp-gold-ddl/src/generators/state_transitions.rs crates/ndp-lib/src/gold/generators/
# (generators/mod.rs is rewritten)

# Move planner
git mv tools/ndp-gold-ddl/src/planner/sync.rs crates/ndp-lib/src/gold/planner/sync.rs
# (planner/mod.rs is rewritten)

# Move registry
git mv tools/ndp-gold-ddl/src/registry/lag.rs crates/ndp-lib/src/gold/registry/
git mv tools/ndp-gold-ddl/src/registry/rolling.rs crates/ndp-lib/src/gold/registry/
git mv tools/ndp-gold-ddl/src/registry/trait_def.rs crates/ndp-lib/src/gold/registry/
git mv tools/ndp-gold-ddl/src/registry/trend.rs crates/ndp-lib/src/gold/registry/
# (registry/mod.rs is rewritten)

# Move validation
git mv tools/ndp-gold-ddl/src/validation/config_validator.rs crates/ndp-lib/src/gold/validation/
# (validation/mod.rs is rewritten)

# Move error
git mv tools/ndp-gold-ddl/src/error.rs crates/ndp-lib/src/gold/error.rs

# Move db queries (CaChecker) -- renamed to db.rs
cp tools/ndp-gold-ddl/src/db/queries.rs crates/ndp-lib/src/gold/db.rs
# (not git mv because queries.rs is combined/adapted, not moved as-is)

# Move integration tests
git mv tools/ndp-gold-ddl/tests/aligned_view_tests.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/state_transitions_tests.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/objectives_tests.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/golden_master_test.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/ops002_config_driven_tests.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/ops002_source_scan_tests.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/ops002_hardcoding_tests.rs crates/ndp-lib/tests/gold/
git mv tools/ndp-gold-ddl/tests/fixtures/ crates/ndp-lib/tests/gold/fixtures/
```

### Step 3: Update `use` Paths

For each moved file, find and replace:
- `use crate::config` with `use crate::gold::config`
- `use crate::error` with `use crate::gold::error`
- `use crate::generators` with `use crate::gold::generators`
- `use crate::planner` with `use crate::gold::planner`
- `use crate::registry` with `use crate::gold::registry`
- `use crate::validation` with `use crate::gold::validation`
- `use crate::db::{DbClient, DbError}` with `use crate::db::DbClient` + `use crate::gold::error::GoldDdlError`

For integration tests, replace `use ndp_gold_ddl::` with `use ndp_lib::gold::`.

### Step 4: Wire CaChecker to ndp_lib::DbClient

Adapt `gold/db.rs` (formerly `queries.rs`):
- Remove `use super::client::{DbClient, DbError}`.
- Add `use crate::db::DbClient`.
- Change `CaChecker` trait methods to return `Result<_, GoldDdlError>`.
- Update `PostgresCaChecker` to use `.map_err(|e| GoldDdlError::DatabaseError(e.to_string()))`.

### Step 5: Verify Tests

```bash
# All gold unit tests should pass
cargo test -p ndp-lib -- gold

# All gold integration tests should pass
cargo test -p ndp-lib --test '*'

# Standalone binary should still build (temporarily broken until thin wrapper is done)
cargo build -p ndp-gold-ddl
```

### Step 6: Write gold/mod.rs Module Files

Create `mod.rs` files for each submodule with the correct `pub mod` and `pub use` declarations as specified in Section 1.4.

### Step 7: Convert ndp-gold-ddl to Thin Wrapper

1. Update `tools/ndp-gold-ddl/Cargo.toml` (add ndp-lib, remove moved deps).
2. Rewrite `tools/ndp-gold-ddl/src/lib.rs` to re-export from ndp_lib::gold.
3. Update `tools/ndp-gold-ddl/src/main.rs` imports from `ndp_gold_ddl::` to `ndp_lib::gold::`.
4. Remove moved source files from `tools/ndp-gold-ddl/src/` (already moved by git mv).
5. Remove moved test files from `tools/ndp-gold-ddl/tests/` (already moved by git mv).

### Step 8: Add `commands/gold.rs` to ndp-cli

1. Create `tools/ndp-cli/src/commands/gold.rs` with Clap structure from Section 5.1.
2. Add `pub mod gold;` to `tools/ndp-cli/src/commands/mod.rs`.
3. Add `Gold(commands::gold::GoldArgs)` variant to `Commands` enum in `main.rs`.
4. Add `--db-timeout` global flag to `Cli` struct.
5. Implement `commands::gold::run()` following the command-to-library mapping in Section 5.5.

### Step 9: Verify Parity

```bash
# Build both binaries
cargo build -p ndp-cli -p ndp-gold-ddl

# Compare output for stream generation
diff <(target/debug/ndp-gold-ddl --config-dir config generate --stream air-quality) \
     <(target/debug/ndp --config-dir config/base gold generate --stream air-quality)

# Compare output for domain generation
diff <(target/debug/ndp-gold-ddl --config-dir config generate --domain indoor-air-quality) \
     <(target/debug/ndp --config-dir config/base gold generate --domain indoor-air-quality)
```

Note: config-dir paths differ because ndp-gold-ddl's `FileSystemConfigLoader` adds `base/streams/` internally, while ndp-cli passes the base dir directly. The parity test must account for this path convention difference.

### Step 10: deploy.sh Switchover

Update 2 dispatch sites in deploy.sh as specified in SCOPE.md Section "v1.1.14 deploy.sh Changes".

### Step 11: Integration Test

```bash
docker compose -f docker-compose.integration.yml up -d
cargo build -p ndp-cli
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
```

---

## 12. Risk Mitigations

### 12.1 Compile-Test-Compile Loop

After moving each submodule (config, generators, planner, registry, validation), run `cargo check -p ndp-lib` immediately. Do not move all 29 files and then fix errors. The dependency graph between submodules means errors cascade. Suggested order:

1. `error.rs` (no internal deps)
2. `config/` (depends on error, generators::events for EventsConfig)
3. `generators/constants.rs` (no internal deps)
4. `generators/` (depends on config, error, registry)
5. `registry/` (depends on config, error)
6. `validation/` (depends on config, error)
7. `db.rs` (depends on error, uses ndp_lib::DbClient)
8. `planner/` (depends on config, error, generators, db, validation)

### 12.2 Circular Dependency: config <-> generators::events

`config/domain.rs` imports `generators::events::EventsConfig` for the `DomainConfig.events` field. This creates a circular reference between `gold::config` and `gold::generators`. Resolution options:

**Option A (recommended): Move `EventsConfig` to `gold::config::domain.rs`.**
`EventsConfig` is a configuration type (deserializable from JSON), not a generator. It logically belongs in the config module. The `EventsGenerator` in `generators/events.rs` would import it from `gold::config::domain::EventsConfig`.

**Option B: Use `serde_json::Value` for events field.**
Replace `pub events: Option<EventsConfig>` with `pub events: Option<serde_json::Value>` in `DomainConfig`. Parse it later in the generator. This is more fragile.

Decision: **Option A**. Move `EventsConfig` to `gold::config::domain.rs`.

### 12.3 Config-Dir Path Convention Difference

ndp-gold-ddl's `FileSystemConfigLoader::new()` takes the top-level `config/` directory and internally appends `base/streams/` and `domains/`. The ndp-cli convention is to take the base directory (e.g., `config/base`) directly.

For v1.1.14, the gold module keeps its own `FileSystemConfigLoader` as-is. The CLI command adapts:

```rust
// In commands/gold.rs:
// config_dir from CLI is "config/base" or "config/integration/base"
// Gold's loader expects "config" (it adds "base/streams/" internally)
let gold_config_dir = config_dir.parent().unwrap_or(config_dir);
let loader = GoldFileSystemConfigLoader::new(gold_config_dir);
```

This is a temporary bridge. In v1.1.16, unification of ConfigLoader will normalize the path convention.

---

## 13. Summary of Deliverables

| Deliverable | Description |
|---|---|
| `crates/ndp-lib/src/gold/` | 25 source files (config, generators, planner, registry, validation, error, db) |
| `crates/ndp-lib/tests/gold/` | 10 integration test files |
| `crates/ndp-lib/src/db.rs` | Add `NoOpDbClient` struct |
| `crates/ndp-lib/Cargo.toml` | Add 3 dev-dependencies |
| `tools/ndp-gold-ddl/Cargo.toml` | Add ndp-lib, remove moved deps |
| `tools/ndp-gold-ddl/src/lib.rs` | Rewrite as re-export from ndp_lib::gold |
| `tools/ndp-gold-ddl/src/main.rs` | Update imports to use ndp_lib::gold |
| `tools/ndp-cli/src/commands/gold.rs` | New file: gold CLI commands |
| `tools/ndp-cli/src/commands/mod.rs` | Add `pub mod gold;` |
| `tools/ndp-cli/src/main.rs` | Add `Gold` variant, `--db-timeout` flag |
| `deploy/pi/deploy.sh` | Update 2 dispatch sites |
| All 376 gold tests passing | In `cargo test -p ndp-lib` |
| DDL output parity verified | `ndp gold generate` matches `ndp-gold-ddl generate` |
