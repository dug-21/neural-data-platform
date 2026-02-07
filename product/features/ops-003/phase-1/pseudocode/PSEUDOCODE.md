# OPS-003 Phase 1 (v1.1.14) Pseudocode: Gold Migration

> **Feature:** ops-003 Release 1 -- Gold DDL generation consolidated into ndp-lib and ndp CLI
> **Phase:** Pseudocode (SPARC P)
> **Date:** 2026-02-07
> **Scope:** 29 source files, 376 tests, 2 deploy.sh dispatch sites

---

## Table of Contents

1. [ndp_lib::gold Module Public API](#1-ndp_libgold-module-public-api)
2. [ndp_lib::gold::config Types](#2-ndp_libgoldconfig-types)
3. [Generator Pseudocode](#3-generator-pseudocode)
4. [SyncPlanner Pseudocode](#4-syncplanner-pseudocode)
5. [CLI Command Pseudocode](#5-cli-command-pseudocode-commandsgoldrs)
6. [ndp-gold-ddl main.rs Thin Wrapper](#6-ndp-gold-ddl-mainrs-thin-wrapper)
7. [deploy.sh Change Pseudocode](#7-deploysh-change-pseudocode)
8. [Migration Script Pseudocode](#8-migration-script-pseudocode)
9. [Error Type Integration](#9-error-type-integration)
10. [Cargo.toml Changes](#10-cargotoml-changes)
11. [Complexity Analysis](#11-complexity-analysis)

---

## 1. ndp_lib::gold Module Public API

### 1.1 Module Structure (`crates/ndp-lib/src/gold/mod.rs`)

```
ALGORITHM: GoldModulePublicAPI
PURPOSE: Define the public API surface for ndp_lib::gold

MODULE STRUCTURE:
    gold/
    +-- mod.rs              Public API: generate(), sync(), recreate()
    +-- config.rs            Gold-specific config types
    +-- error.rs             GoldDdlError (moved from ndp-gold-ddl)
    +-- generators/
    |   +-- mod.rs
    |   +-- aligned_view.rs
    |   +-- classification.rs
    |   +-- column_builder.rs
    |   +-- constants.rs
    |   +-- continuous_aggregate.rs
    |   +-- events.rs
    |   +-- join_builder.rs
    |   +-- null_handler.rs
    |   +-- refresh_policy.rs
    |   +-- state_transitions.rs
    +-- planner/
    |   +-- mod.rs
    |   +-- sync.rs
    +-- registry/
    |   +-- mod.rs
    |   +-- lag.rs
    |   +-- rolling.rs
    |   +-- trait_def.rs
    |   +-- trend.rs
    +-- validation/
        +-- mod.rs
        +-- config_validator.rs
```

### 1.2 Public API Signatures (`gold/mod.rs`)

```rust
//! Gold layer DDL generation for NDP.
//!
//! Migrated from tools/ndp-gold-ddl. Provides DDL generation for:
//! - TimescaleDB continuous aggregates for individual streams
//! - Aligned materialized views for cross-stream correlation
//! - State transition views for state_event streams
//! - Events infrastructure for unified event storage

pub mod config;
pub mod error;
pub mod generators;
pub mod planner;
pub mod registry;
pub mod validation;

// Re-exports for convenient access (same surface as ndp_gold_ddl::lib.rs)
pub use config::{
    Action, AlignedStream, AlignmentConfig, ConfigLoader, DomainConfig,
    FileSystemConfigLoader, GoldEtlConfig, JoinStrategy, NullHandling,
    ObjectiveConfig, Priority, StreamConfig, StreamRef, StreamRole,
    StreamType, TargetConfig,
};

pub use error::{GoldDdlError, Result};

pub use generators::{
    generate_classification_sql, generate_gold_table_sql,
    AlignedViewGenerator, ClassificationSyncer, ContinuousAggregateGenerator,
    DefaultClassificationSyncer, EventsConfig, EventsGenerator,
    IEventsGenerator, ITransitionGenerator, RefreshPolicyGenerator,
    StateTransitionGenerator, TransitionConfig,
};

pub use registry::{FeatureConfig, FeatureGenerator, FeatureRegistry, SqlColumn};

pub use validation::{validate_gold_config, ConfigValidator};

pub use planner::{CaAction, SyncPlan, SyncPlanner};

// ---------------------------------------------------------------------------
// Top-level convenience functions (NEW for v1.1.14)
// ---------------------------------------------------------------------------

/// Generate Gold DDL for a stream without applying.
///
/// This is the primary entry point for `ndp gold generate --stream X`.
/// Returns the DDL string. Does NOT connect to database.
///
/// ALGORITHM:
///   1. Load stream config from config_loader
///   2. Validate gold_etl is present and enabled
///   3. Dispatch to appropriate generator based on options
///   4. Return DDL string
pub fn generate_stream(
    config_loader: &impl ConfigLoader,
    stream_id: &str,
    opts: &GenerateOptions,
) -> Result<String>

/// Generate Gold DDL for a domain (aligned view) without applying.
///
/// This is the entry point for `ndp gold generate --domain X`.
pub fn generate_domain(
    config_loader: &impl ConfigLoader,
    domain_id: &str,
    opts: &GenerateOptions,
) -> Result<String>

/// Sync Gold DDL for a stream (idempotent apply with DB checks).
///
/// This is the entry point for `ndp gold sync --stream X`.
/// Connects to database, checks what exists, generates only needed DDL.
///
/// ALGORITHM:
///   1. Load stream config
///   2. Validate gold_etl
///   3. Connect to DB via CaChecker
///   4. Run SyncPlanner to determine create/skip/recreate per CA
///   5. Return plan.to_ddl() string
pub async fn sync_stream(
    config_loader: &impl ConfigLoader,
    stream_id: &str,
    db: &(impl crate::DbClient + Send + Sync),
    opts: &SyncOptions,
) -> Result<String>

/// Sync Gold DDL for a domain (aligned view apply).
///
/// This is the entry point for `ndp gold sync --domain X`.
pub fn sync_domain(
    config_loader: &impl ConfigLoader,
    domain_id: &str,
    _opts: &SyncOptions,
) -> Result<String>

/// Recreate Gold DDL for a stream (drop + create).
///
/// This is the entry point for `ndp gold recreate --stream X`.
pub fn recreate_stream(
    config_loader: &impl ConfigLoader,
    stream_id: &str,
    opts: &GenerateOptions,
) -> Result<String>

/// Options for generate operations (no DB needed).
pub struct GenerateOptions {
    /// Generate state transition views instead of CAs
    pub transitions: bool,
    /// Generate events infrastructure DDL
    pub events: bool,
    /// Verbose output to stderr
    pub verbose: bool,
}

/// Extended SyncOptions (augments ndp_lib::types::SyncOptions).
///
/// NOTE: In v1.1.14, SyncOptions from ndp_lib::types is reused.
/// The existing SyncOptions { dry_run: bool } is extended:
pub struct GoldSyncOptions {
    /// Base sync options
    pub base: crate::types::SyncOptions,
    /// Verbose output
    pub verbose: bool,
    /// Database timeout in seconds
    pub db_timeout: u64,
}
```

### 1.3 Top-Level Function Pseudocode

```
ALGORITHM: generate_stream
INPUT: config_loader (impl ConfigLoader), stream_id (string), opts (GenerateOptions)
OUTPUT: DDL string or error

BEGIN
    stream_config <- config_loader.load_stream_config(stream_id)?

    gold_etl <- stream_config.gold_etl
        .ok_or(MissingRequiredField("gold_etl", stream_id))?

    IF NOT gold_etl.enabled THEN
        RETURN error("Gold ETL disabled for stream")
    END IF

    IF opts.transitions THEN
        transition_config <- TransitionConfig::from_stream_config(&stream_config)
            .unwrap_or_else(TransitionConfig::new("state", "ndp_id"))
        generator <- StateTransitionGenerator::from_stream_config(&stream_config)?
        RETURN generator.generate(&transition_config, Action::Sync)
    END IF

    IF opts.events THEN
        RETURN error("--events requires --domain")
    END IF

    // Default: continuous aggregate generation
    generator <- ContinuousAggregateGenerator::from_stream_config(&stream_config)?
    ddl <- generator.generate(&gold_etl, Action::Sync)?
    RETURN ddl
END


ALGORITHM: sync_stream
INPUT: config_loader, stream_id, db (impl DbClient), opts (SyncOptions)
OUTPUT: DDL string with sync plan or error

BEGIN
    stream_config <- config_loader.load_stream_config(stream_id)?

    gold_etl <- stream_config.gold_etl
        .ok_or(MissingRequiredField("gold_etl", stream_id))?

    IF NOT gold_etl.enabled THEN
        RETURN error("Gold ETL disabled for stream")
    END IF

    // Create CaChecker using ndp_lib::DbClient
    // KEY CHANGE: CaChecker wraps ndp_lib::DbClient, not ndp-gold-ddl::DbClient
    checker <- CaCheckerAdapter::new(db)
    planner <- SyncPlanner::new(&checker, &stream_config)
    plan <- planner.plan(&gold_etl).await?

    IF opts.verbose THEN
        eprintln(plan.summary())
    END IF

    RETURN plan.to_ddl()
END


ALGORITHM: generate_domain
INPUT: config_loader, domain_id, opts (GenerateOptions)
OUTPUT: DDL string or error

BEGIN
    domain_config <- config_loader.load_domain_config(domain_id)?

    IF opts.events THEN
        events_loader <- config_loader.clone()  // needs Box<dyn ConfigLoader>
        generator <- EventsGenerator::from_domain_config(&domain_config, events_loader)
        RETURN generator.generate(Action::Sync)
    END IF

    // Default: aligned view generation
    generator <- AlignedViewGenerator::new(config_loader)
    ddl <- generator.generate(&domain_config, Action::Sync)?
    RETURN ddl
END


ALGORITHM: recreate_stream
INPUT: config_loader, stream_id, opts (GenerateOptions)
OUTPUT: DDL string or error

BEGIN
    stream_config <- config_loader.load_stream_config(stream_id)?

    gold_etl <- stream_config.gold_etl
        .ok_or(MissingRequiredField("gold_etl", stream_id))?

    IF NOT gold_etl.enabled THEN
        RETURN error("Gold ETL disabled for stream")
    END IF

    IF opts.transitions THEN
        transition_config <- TransitionConfig::from_stream_config(&stream_config)
            .unwrap_or_else(TransitionConfig::new("state", "ndp_id"))
        generator <- StateTransitionGenerator::from_stream_config(&stream_config)?
        RETURN generator.generate(&transition_config, Action::Recreate)
    END IF

    generator <- ContinuousAggregateGenerator::from_stream_config(&stream_config)?
    ddl <- generator.generate(&gold_etl, Action::Recreate)?
    RETURN ddl
END
```

---

## 2. ndp_lib::gold::config Types

### 2.1 Types That Move (`gold/config.rs` -- formerly `config/types.rs`)

All types move with ZERO logic changes. Only `use crate::` paths change.

```rust
// crates/ndp-lib/src/gold/config.rs
//
// BEFORE (in ndp-gold-ddl):  use crate::config::domain::StreamType;
// AFTER  (in ndp-lib):        use crate::gold::config::domain::StreamType;
//                              (or, since domain.rs is in same module: use super::domain::StreamType)

// --- types.rs content moves here ---

/// Valid aggregate metrics (SHARED -- will move to constants.rs in v1.1.16)
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];

/// Valid rolling statistics (SHARED -- will move to constants.rs in v1.1.16)
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoldEtlConfig {
    pub enabled: bool,
    pub aggregates: Option<AggregatesConfig>,
    pub features: Option<FeaturesConfig>,
    pub refresh_policy: Option<RefreshPolicyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregatesConfig {
    pub granularities: Vec<String>,
    pub fields: HashMap<String, FieldMetricsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldMetricsConfig {
    pub metrics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeaturesConfig {
    pub lag: Option<LagConfig>,
    pub rolling: Option<RollingConfig>,
    pub trend: Option<TrendConfig>,
    pub transitions: Option<TransitionsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LagConfig {
    pub enabled: bool,
    pub lags_hours: Vec<u32>,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RollingConfig {
    pub enabled: bool,
    pub windows: Vec<String>,
    pub stats: Vec<String>,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrendConfig {
    pub enabled: bool,
    pub window: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransitionsConfig {
    pub enabled: bool,
    pub field: String,
    pub states: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshPolicyConfig {
    pub start_offset: String,
    pub end_offset: String,
    pub schedule_interval: String,
}

// impl Default, for_granularity, default_hourly, default_daily -- all move unchanged

/// Simplified stream config for Gold DDL generation
/// NOTE: This is NOT ndp_lib::config::StreamConfig (which is sync-focused)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub stream_type: Option<StreamType>,
    pub fields: Vec<FieldConfig>,
    pub silver_etl: Option<SilverEtlConfig>,
    pub gold_etl: Option<GoldEtlConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    pub name: String,
    #[serde(rename = "type", default)]
    pub field_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SilverEtlConfig {
    pub target_table: String,
    pub timestamp: Option<TimestampConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimestampConfig {
    #[serde(default = "default_timestamp_field")]
    pub target_field: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    #[default]
    Sync,
    Recreate,
}

// impl FromStr, Display -- move unchanged
```

### 2.2 Domain Types (`gold/config/domain.rs` -- formerly `config/domain.rs`)

All domain types move unchanged:
- `DomainConfig`, `StreamRef`, `StreamRole`, `StreamType`
- `AlignmentConfig`, `JoinStrategy`, `NullHandling`
- `ObjectiveConfig`, `TargetConfig`, `Priority`
- `AlignedStream`

### 2.3 Loader Types (`gold/config/loader.rs` -- formerly `config/loader.rs`)

```rust
// ConfigLoader trait and FileSystemConfigLoader move unchanged
// Path references:
//   BEFORE: use crate::config::domain::DomainConfig;
//   AFTER:  use super::domain::DomainConfig;  (within gold::config module)

pub trait ConfigLoader: Send + Sync {
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig>;
    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig>;
}

pub struct FileSystemConfigLoader {
    config_dir: PathBuf,
}

// All methods and tests move unchanged
```

### 2.4 Config Module Entry (`gold/config/mod.rs`)

```rust
pub mod domain;
pub mod loader;
pub mod types;

pub use domain::{ /* all domain types */ };
pub use loader::{ ConfigLoader, FileSystemConfigLoader, default_loader, resolve_config_dir };
pub use types::{ Action, AggregatesConfig, /* ... all types ... */, VALID_METRICS, VALID_ROLLING_STATS };
```

---

## 3. Generator Pseudocode

### 3.1 Generators Module Overview

Each generator file moves with the following `use` path changes:

| Generator File | Old `use crate::` Path | New `use crate::gold::` Path |
|---------------|------------------------|------------------------------|
| `continuous_aggregate.rs` | `crate::config::{Action, AggregatesConfig, ...}` | `crate::gold::config::{Action, AggregatesConfig, ...}` |
| `continuous_aggregate.rs` | `crate::error::{GoldDdlError, Result}` | `crate::gold::error::{GoldDdlError, Result}` |
| `continuous_aggregate.rs` | `crate::validation::granularity_to_suffix` | `crate::gold::validation::granularity_to_suffix` |
| `aligned_view.rs` | `crate::config::{Action, AlignedStream, ...}` | `crate::gold::config::{...}` |
| `aligned_view.rs` | `crate::generators::column_builder` | `super::column_builder` |
| `aligned_view.rs` | `crate::generators::constants::GOLD_SCHEMA` | `super::constants::GOLD_SCHEMA` |
| `state_transitions.rs` | `crate::config::{Action, StreamConfig}` | `crate::gold::config::{...}` |
| `state_transitions.rs` | `super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN}` | unchanged (relative) |
| `events.rs` | `crate::config::{Action, ConfigLoader, DomainConfig, StreamRole}` | `crate::gold::config::{...}` |
| `events.rs` | `super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA}` | unchanged (relative) |
| `events.rs` | `super::state_transitions::TransitionConfig` | unchanged (relative) |
| `refresh_policy.rs` | `crate::config::RefreshPolicyConfig` | `crate::gold::config::RefreshPolicyConfig` |
| `refresh_policy.rs` | `crate::validation::granularity_to_suffix` | `crate::gold::validation::granularity_to_suffix` |
| `classification.rs` | `crate::config::{...}` | `crate::gold::config::{...}` |
| `column_builder.rs` | `crate::config::*` | `crate::gold::config::*` |
| `join_builder.rs` | `crate::config::*` | `crate::gold::config::*` |
| `null_handler.rs` | `crate::config::*` | `crate::gold::config::*` |
| `constants.rs` | (no crate imports) | unchanged |

### 3.2 Generator Module Entry (`gold/generators/mod.rs`)

```rust
// Identical to current tools/ndp-gold-ddl/src/generators/mod.rs
// All submodule declarations and re-exports stay the same

pub mod aligned_view;
pub mod classification;
pub mod column_builder;
pub mod constants;
pub mod continuous_aggregate;
pub mod events;
pub mod join_builder;
pub mod null_handler;
pub mod refresh_policy;
pub mod state_transitions;

pub use aligned_view::AlignedViewGenerator;
pub use classification::{
    generate_classification_sql, generate_gold_table_sql,
    ClassificationSyncer, DefaultClassificationSyncer,
};
pub use column_builder::ColumnBuilder;
pub use continuous_aggregate::ContinuousAggregateGenerator;
pub use events::{EventsConfig, EventsGenerator, IEventsGenerator};
pub use join_builder::JoinBuilder;
pub use null_handler::{
    CarryForwardNullHandler, InterpolateNullHandler, NullHandler, PreserveNullHandler,
};
pub use refresh_policy::RefreshPolicyGenerator;
pub use state_transitions::{
    DeviceTypeRule, ITransitionGenerator, StateTransitionGenerator, TransitionConfig,
};
```

### 3.3 Path Transformation Rule

```
ALGORITHM: TransformUsePaths
INPUT: source_file (Rust source), old_crate_root ("crate"), module_depth (int)
OUTPUT: transformed source with updated use paths

RULE: Every `use crate::` that references gold-ddl-internal modules changes:
    crate::config::*     -> crate::gold::config::*
    crate::error::*      -> crate::gold::error::*
    crate::generators::* -> crate::gold::generators::*
    crate::planner::*    -> crate::gold::planner::*
    crate::registry::*   -> crate::gold::registry::*
    crate::validation::* -> crate::gold::validation::*
    crate::db::*          -> (see Section 4 -- CaChecker adapter)

RULE: Intra-module `super::` paths do NOT change.
    super::constants::GOLD_SCHEMA stays super::constants::GOLD_SCHEMA

RULE: External crate imports do NOT change.
    use serde::{Deserialize, Serialize} stays the same
    use async_trait::async_trait stays the same
```

---

## 4. SyncPlanner Pseudocode

### 4.1 SyncPlanner Decision Logic

The SyncPlanner determines create/skip/recreate for each continuous aggregate.

```
ALGORITHM: SyncPlanner::plan
INPUT: gold_etl (GoldEtlConfig), stream_config (StreamConfig), checker (CaChecker)
OUTPUT: SyncPlan with per-CA actions

BEGIN
    aggregates <- gold_etl.aggregates
        .ok_or(MissingRequiredField("gold_etl.aggregates"))?

    generator <- ContinuousAggregateGenerator::from_stream_config(stream_config)?

    ca_plans <- []

    FOR EACH granularity IN aggregates.granularities DO
        plan <- plan_for_granularity(generator, aggregates, granularity, gold_etl.refresh_policy)
        ca_plans.append(plan)
    END FOR

    RETURN SyncPlan {
        stream_id: stream_config.stream_id,
        schema_ddl: "CREATE SCHEMA IF NOT EXISTS gold;",
        ca_plans: ca_plans,
    }
END


ALGORITHM: plan_for_granularity
INPUT: generator, aggregates, granularity, refresh_policy
OUTPUT: CaPlan (single CA decision)

BEGIN
    suffix <- granularity_to_suffix(granularity)
    view_name <- "{stream_id_normalized}_{suffix}"
    schema <- "gold"

    // Check DB state via CaChecker trait
    ca_exists <- checker.ca_exists(schema, view_name).await?
    policy_exists <- IF ca_exists THEN
        checker.refresh_policy_exists(schema, view_name).await?
    ELSE
        false
    END IF

    // Generate DDL (needed for create or recreate)
    create_ddl <- generator.generate_ca_ddl_only(aggregates, granularity)?
    policy_ddl <- generator.generate_policy_ddl_only(granularity, refresh_policy)?

    // Decision
    action <- IF ca_exists THEN CaAction::Skip ELSE CaAction::Create

    RETURN CaPlan {
        schema, name: view_name, granularity,
        action,
        create_ddl: Some(create_ddl),
        policy_ddl: Some(policy_ddl),
        needs_policy: NOT policy_exists,
    }
END
```

### 4.2 CaChecker Adaptation: From Local DbClient to ndp_lib::DbClient

**Problem:** ndp-gold-ddl defines its own `DbClient` trait (in `db/client.rs`) with signature:
```rust
async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, DbError>
```

ndp-lib defines `DbClient` (in `db.rs`) with signature:
```rust
async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>  // NdpLibError
```

**Solution:** The `CaChecker` trait stays in `gold/` but `PostgresCaChecker` wraps `ndp_lib::DbClient` instead of the local `DbClient`.

```
ALGORITHM: CaCheckerAdapter
PURPOSE: Bridge gold::CaChecker to use ndp_lib::DbClient

APPROACH A (PREFERRED): CaChecker keeps its own error type (DbError).
PostgresCaChecker<C: ndp_lib::DbClient> adapts errors:

    impl<C: ndp_lib::DbClient> CaChecker for PostgresCaChecker<C> {
        async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, DbError> {
            let rows = self.client.query(SQL, &[&schema, &name])
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
            // ... same logic ...
        }
    }

CHANGES REQUIRED:
    1. gold/db/client.rs: Remove PostgresClient (use ndp_lib::db::PostgresClient)
    2. gold/db/client.rs: Keep DbError enum (gold-specific error type)
    3. gold/db/queries.rs: Change `super::client::DbClient` -> `crate::DbClient`
       Change PostgresCaChecker<C: DbClient> -> PostgresCaChecker<C: crate::DbClient>
    4. gold/db/mod.rs: Remove DbClient re-export, keep CaChecker/CaInfo/PostgresCaChecker

NEW gold/db/mod.rs:
    pub mod client;   // Only DbError now (no DbClient, no PostgresClient)
    pub mod queries;

    pub use client::DbError;
    pub use queries::{CaChecker, CaInfo, PostgresCaChecker};

NEW gold/db/client.rs:
    // ONLY the error type remains here. DbClient and PostgresClient are gone.
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum DbError {
        #[error("Connection failed: {0}")]
        ConnectionFailed(String),
        #[error("Query failed: {0}")]
        QueryFailed(String),
        #[error("Invalid database URL: {0}")]
        InvalidUrl(String),
        #[error("Connection timeout after {0} seconds")]
        Timeout(u64),
    }

NEW gold/db/queries.rs:
    use super::client::DbError;

    // CaChecker trait stays the same
    #[async_trait]
    pub trait CaChecker: Send + Sync {
        async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, DbError>;
        async fn get_ca_info(&self, schema: &str, name: &str) -> Result<Option<CaInfo>, DbError>;
        async fn list_cas_in_schema(&self, schema: &str) -> Result<Vec<CaInfo>, DbError>;
        async fn refresh_policy_exists(&self, schema: &str, name: &str) -> Result<bool, DbError>;
    }

    // PostgresCaChecker now takes crate::DbClient (ndp_lib's trait)
    pub struct PostgresCaChecker<C: crate::DbClient> {
        client: C,
    }

    #[async_trait]
    impl<C: crate::DbClient + Send + Sync> CaChecker for PostgresCaChecker<C> {
        async fn ca_exists(&self, schema: &str, name: &str) -> Result<bool, DbError> {
            let rows = self.client.query(QUERY, &[&schema, &name])
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
            Ok(rows.first().map(|r| r.get::<_, bool>("exists")).unwrap_or(false))
        }
        // ... remaining methods follow same pattern ...
    }
```

### 4.3 CaInfo (Unchanged)

```rust
// Moves as-is. No changes.
#[derive(Debug, Clone, PartialEq)]
pub struct CaInfo {
    pub schema: String,
    pub name: String,
    pub view_definition: Option<String>,
}
```

---

## 5. CLI Command Pseudocode (`commands/gold.rs`)

### 5.1 Clap Structs

Following the exact patterns from `dictionary.rs` and `domain.rs`:

```rust
// tools/ndp-cli/src/commands/gold.rs

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
    /// Generate Gold DDL without applying.
    Generate {
        /// Stream ID for single-stream continuous aggregate.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Domain ID for cross-stream aligned view.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Generate state transitions view instead of continuous aggregate.
        #[arg(long)]
        transitions: bool,

        /// Generate events infrastructure DDL (requires --domain).
        #[arg(long)]
        events: bool,
    },

    /// Sync Gold DDL (idempotent apply -- create if not exists).
    Sync {
        /// Stream ID for single-stream sync.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Domain ID for cross-stream sync.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Database connection timeout in seconds.
        #[arg(long, default_value = "10")]
        db_timeout: u64,
    },

    /// Recreate Gold DDL (drop and create).
    Recreate {
        /// Stream ID for single-stream recreate.
        #[arg(long, conflicts_with = "domain")]
        stream: Option<String>,

        /// Domain ID for cross-stream recreate.
        #[arg(long, conflicts_with = "stream")]
        domain: Option<String>,

        /// Database connection timeout in seconds.
        #[arg(long, default_value = "10")]
        db_timeout: u64,
    },
}
```

### 5.2 Execute Function

```
ALGORITHM: gold::run
INPUT:
    args (GoldArgs) - parsed Clap arguments
    base_config_dir (Path) - resolved config base directory (from global --config-dir)
    db_url (str) - resolved database URL (from global --db-url)
OUTPUT: Result<(), Box<dyn Error>>

BEGIN
    // Create Gold-specific config loader
    // Gold configs use a different directory structure than sync configs:
    //   config/base/streams/<stream>/config.json  for streams
    //   config/domains/<domain>/domain.json       for domains
    // The Gold ConfigLoader (from ndp_lib::gold::config) resolves these paths.
    //
    // base_config_dir points to "config/base" (or "config/integration/base")
    // Gold ConfigLoader needs the parent: "config" (or "config/integration")
    config_dir <- base_config_dir.parent().unwrap_or(base_config_dir)
    loader <- ndp_lib::gold::FileSystemConfigLoader::new(config_dir)

    MATCH args.command:
        GoldCommands::Generate { stream, domain, transitions, events } =>
            run_generate(loader, stream, domain, transitions, events)

        GoldCommands::Sync { stream, domain, db_timeout } =>
            run_sync(loader, stream, domain, db_url, db_timeout).await

        GoldCommands::Recreate { stream, domain, db_timeout } =>
            run_recreate(loader, stream, domain, db_url, db_timeout).await
    END MATCH
END


ALGORITHM: run_generate
INPUT: loader, stream (Option<String>), domain (Option<String>), transitions (bool), events (bool)
OUTPUT: prints DDL to stdout

BEGIN
    opts <- GenerateOptions { transitions, events, verbose: false }

    IF domain IS SOME THEN
        ddl <- ndp_lib::gold::generate_domain(&loader, domain_id, &opts)?
        println(ddl)
    ELSE IF stream IS SOME THEN
        ddl <- ndp_lib::gold::generate_stream(&loader, stream_id, &opts)?
        println(ddl)
    ELSE
        RETURN error("Must specify --stream or --domain")
    END IF
END


ALGORITHM: run_sync
INPUT: loader, stream, domain, db_url, db_timeout
OUTPUT: prints DDL to stdout

BEGIN
    IF stream IS SOME THEN
        // Stream sync requires DB connection for CA existence checks
        db <- ndp_lib::db::PostgresClient::connect(db_url, db_timeout).await?

        opts <- SyncOptions { dry_run: false }
        ddl <- ndp_lib::gold::sync_stream(&loader, stream_id, &db, &opts).await?
        println(ddl)

    ELSE IF domain IS SOME THEN
        // Domain sync does not use DB checks (aligned views use DO $$ IF NOT EXISTS)
        opts <- SyncOptions { dry_run: false }
        ddl <- ndp_lib::gold::sync_domain(&loader, domain_id, &opts)?
        println(ddl)

    ELSE
        RETURN error("Must specify --stream or --domain")
    END IF
END


ALGORITHM: run_recreate
INPUT: loader, stream, domain, db_url, db_timeout
OUTPUT: prints DDL to stdout

BEGIN
    opts <- GenerateOptions { transitions: false, events: false, verbose: false }

    IF stream IS SOME THEN
        ddl <- ndp_lib::gold::recreate_stream(&loader, stream_id, &opts)?
        println(ddl)
    ELSE IF domain IS SOME THEN
        // Domain recreate: generate with Action::Recreate
        domain_config <- loader.load_domain_config(domain_id)?
        generator <- AlignedViewGenerator::new(loader)
        ddl <- generator.generate(&domain_config, Action::Recreate)?
        println(ddl)
    ELSE
        RETURN error("Must specify --stream or --domain")
    END IF
END
```

### 5.3 Error Handling and Exit Codes

```
ALGORITHM: Error Handling in gold.rs
PURPOSE: Match existing ndp-cli patterns (dictionary.rs, domain.rs)

The run() function returns Result<(), Box<dyn Error>>.
main.rs propagates this error, which prints to stderr via Display trait.
Exit code is 1 on any error (consistent with other commands).

No special exit code mapping in the CLI layer -- that complexity
stays in ndp-gold-ddl's main.rs for backward compatibility.
The ndp CLI uses a single error path: print and exit(1).
```

### 5.4 Module Registration (`commands/mod.rs`)

```rust
// tools/ndp-cli/src/commands/mod.rs

pub mod dictionary;
pub mod dimension;
pub mod domain;
pub mod gold;     // NEW
```

### 5.5 Main.rs Integration

```rust
// tools/ndp-cli/src/main.rs changes:

#[derive(Subcommand)]
enum Commands {
    /// Data dictionary operations.
    Dictionary(commands::dictionary::DictionaryArgs),

    /// Dimension table operations.
    Dimension(commands::dimension::DimensionArgs),

    /// Domain configuration operations.
    Domain(commands::domain::DomainArgs),

    /// Gold layer DDL operations.             // NEW
    Gold(commands::gold::GoldArgs),            // NEW
}

// In main():
match cli.command {
    Commands::Dictionary(args) => {
        commands::dictionary::run(args, &config_dir, &db_url).await?;
    }
    Commands::Dimension(args) => {
        commands::dimension::run(args, &config_dir, &db_url).await?;
    }
    Commands::Domain(args) => {
        commands::domain::run(args, &config_dir, &db_url).await?;
    }
    Commands::Gold(args) => {                  // NEW
        commands::gold::run(args, &config_dir, &db_url).await?;
    }
}
```

### 5.6 Global Flag Flow

```
DIAGRAM: Global Flag Flow from CLI to Library

User invocation:
    ndp gold sync --stream air-quality --db-url postgresql://... --config-dir config/base

Clap parsing (main.rs):
    cli.db_url       = Some("postgresql://...")
    cli.config_dir   = Some("config/base")
    cli.command      = Commands::Gold(GoldArgs { command: GoldCommands::Sync { ... } })

Resolution (main.rs):
    config_dir = cli.resolve_config_dir()      -> PathBuf("config/base")
    db_url     = cli.resolve_db_url()           -> "postgresql://..."

Dispatch to gold::run():
    args        = GoldArgs (from Clap)
    config_dir  = &PathBuf("config/base")      -> gold::run computes parent -> "config"
    db_url      = "postgresql://..."

Library call:
    loader = FileSystemConfigLoader::new("config")
    db     = PostgresClient::connect(db_url, db_timeout).await?
    ddl    = ndp_lib::gold::sync_stream(&loader, "air-quality", &db, &opts).await?

NOTE: --db-url maps to global Cli.db_url (harmonized from standalone --database-url).
NOTE: --db-timeout maps to per-subcommand arg (not global, since only gold uses it).
NOTE: --config-dir maps to the same global Cli.config_dir that dictionary/domain use.
      The gold config loader needs the config ROOT (not base/), so gold::run goes
      up one directory from base_config_dir.
```

---

## 6. ndp-gold-ddl main.rs Thin Wrapper

After migration, `tools/ndp-gold-ddl/src/main.rs` becomes a thin wrapper that delegates to `ndp_lib::gold::*`. It preserves backward compatibility for anyone who still calls `ndp-gold-ddl` directly.

```rust
// tools/ndp-gold-ddl/src/main.rs (AFTER migration)
// Preserves existing CLI interface but delegates to ndp_lib::gold

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

// Import from ndp_lib instead of local modules
use ndp_lib::gold::config::{Action, FileSystemConfigLoader};
use ndp_lib::gold::{generate_stream, generate_domain, sync_stream, recreate_stream};
use ndp_lib::gold::{GenerateOptions};
use ndp_lib::db::PostgresClient;

mod exit_codes {
    pub const SUCCESS: u8 = 0;
    pub const GENERATION_ERROR: u8 = 1;
    pub const DATABASE_ERROR: u8 = 3;
}

// Clap struct stays IDENTICAL to current -- backward compatibility
#[derive(Parser, Debug)]
#[command(name = "ndp-gold-ddl")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, env = "NDP_CONFIG_DIR", default_value = "./config")]
    config_dir: PathBuf,
    #[arg(long, env = "TIMESCALE_URL")]
    database_url: Option<String>,
    #[arg(long, default_value = "10")]
    db_timeout: u64,
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Generate { /* same fields as current */ },
    Validate { /* same fields as current */ },
}

#[tokio::main]
async fn main() -> ExitCode {
    // Same tracing init
    let cli = Cli::parse();

    match run(&cli).await {
        Ok(output) => {
            println!("{}", output);
            ExitCode::from(exit_codes::SUCCESS)
        }
        Err(e) => {
            let exit_code = if e.to_string().contains("Database") {
                exit_codes::DATABASE_ERROR
            } else {
                exit_codes::GENERATION_ERROR
            };
            eprintln!("Error: {}", e);
            ExitCode::from(exit_code)
        }
    }
}

async fn run(cli: &Cli) -> Result<String, Box<dyn std::error::Error>> {
    let loader = FileSystemConfigLoader::new(&cli.config_dir);

    match &cli.command {
        Commands::Generate { stream, domain, action, transitions, events } => {
            let action: Action = action.parse()?;

            if let Some(domain_id) = domain {
                let opts = GenerateOptions {
                    transitions: false,
                    events: *events,
                    verbose: cli.verbose,
                };
                return generate_domain(&loader, domain_id, &opts)
                    .map_err(|e| e.into());
            }

            if let Some(stream_id) = stream {
                match action {
                    Action::Sync => {
                        if let Some(db_url) = &cli.database_url {
                            let db = PostgresClient::connect(db_url, cli.db_timeout).await?;
                            let opts = ndp_lib::types::SyncOptions { dry_run: false };
                            return sync_stream(&loader, stream_id, &db, &opts).await
                                .map_err(|e| e.into());
                        }
                        // No DB URL: generate all DDL (dry-run)
                        let opts = GenerateOptions {
                            transitions: *transitions,
                            events: false,
                            verbose: cli.verbose,
                        };
                        return generate_stream(&loader, stream_id, &opts)
                            .map_err(|e| e.into());
                    }
                    Action::Recreate => {
                        let opts = GenerateOptions {
                            transitions: *transitions,
                            events: false,
                            verbose: cli.verbose,
                        };
                        return recreate_stream(&loader, stream_id, &opts)
                            .map_err(|e| e.into());
                    }
                }
            }

            Err("Must specify --stream or --domain".into())
        }
        // Validate command delegates too...
        Commands::Validate { stream, domain } => {
            // Same validation logic, calls ndp_lib::gold::validate_gold_config
            // ...
        }
    }
}
```

### 6.1 Cargo.toml Change for ndp-gold-ddl

```toml
# tools/ndp-gold-ddl/Cargo.toml CHANGES:

[dependencies]
# ADD:
ndp-lib = { path = "../../crates/ndp-lib" }

# KEEP (needed by main.rs for CLI parsing):
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# REMOVE (now provided by ndp-lib):
# serde, serde_json -- provided transitively via ndp-lib
# async-trait -- provided transitively
# thiserror -- provided transitively
# tokio-postgres -- provided transitively
```

---

## 7. deploy.sh Change Pseudocode

### 7.1 Site 1: `handle_gold_tables()` (~line 1936)

```bash
ALGORITHM: handle_gold_tables() Gold Dispatch Switchover

# BEFORE: 4-way lookup for ndp-gold-ddl binary
# AFTER:  4-way lookup for ndp binary (same pattern as lines 386, 894, 1063)

PSEUDOCODE:

handle_gold_tables() {
    local stream_id="$1"
    local action="$2"
    local db_url="$3"

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
        return 1   # FAIL LOUDLY -- no fallback (D5 from SCOPE)
    fi

    # Map action to verb:
    #   "sync"     -> ndp gold sync
    #   "recreate" -> ndp gold recreate
    #
    # NOTE: deploy.sh currently calls ndp-gold-ddl generate --action $action.
    # With ndp CLI, action IS the verb: ndp gold $action.
    local ddl
    ddl=$("$ndp_tool" gold "$action" --stream "$stream_id" \
        --config-dir "$REPO_ROOT/config" \
        --db-url "$db_url" \
        --db-timeout 10 2>&1)

    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        error "Gold DDL generation failed for stream '$stream_id': $ddl"
        return 1
    fi

    # Apply DDL if non-empty
    if [ -n "$ddl" ] && [ "$ddl" != "-- No work needed" ]; then
        info "Applying Gold DDL for stream '$stream_id'..."
        echo "$ddl" | psql "$db_url" -v ON_ERROR_STOP=1
        local psql_exit=$?
        if [ $psql_exit -ne 0 ]; then
            error "Failed to apply Gold DDL for stream '$stream_id'"
            return 1
        fi
    else
        info "Gold layer up to date for stream '$stream_id'"
    fi
}

KEY DIFFERENCES FROM BEFORE:
    1. error + return 1 instead of warn + return 0
    2. "ndp" binary instead of "ndp-gold-ddl"
    3. "$ndp_tool" gold "$action" instead of "$gold_ddl_tool" --action "$action" generate
    4. --db-url instead of --database-url
    5. --config-dir "$REPO_ROOT/config" instead of --config-dir "$REPO_ROOT/config"
       (same value, but flag name is --config-dir in both cases -- no change)
```

### 7.2 Site 2: `handle_domain_declaration()` Gold Section (~line 2069)

```bash
ALGORITHM: handle_domain_declaration() Gold Section Switchover

PSEUDOCODE:

# Inside handle_domain_declaration(), the gold dispatch section:

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

    # Generate and apply domain aligned view DDL
    local ddl
    ddl=$("$ndp_tool" gold "$action" --domain "$domain_id" \
        --config-dir "$REPO_ROOT/config" 2>&1)

    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        error "Gold DDL generation failed for domain '$domain_id': $ddl"
        return 1
    fi

    # Apply DDL...

KEY DIFFERENCES FROM BEFORE:
    1. No --database-url needed for domain (domain generation does not use DB checks)
    2. error + return 1 instead of warn + return 0
    3. "ndp gold" instead of "ndp-gold-ddl generate"
    4. $action is the verb (sync or recreate), mapped to ndp gold $action
```

### 7.3 ndp Tool Resolution Extraction

```bash
# OPTIMIZATION: Since handle_domain_declaration() now resolves ndp_tool
# for BOTH gold and validate (in v1.1.15), extract to a shared function:

resolve_ndp_tool() {
    if command -v ndp &> /dev/null; then
        echo "ndp"
    elif [ -x "/opt/ndp/bin/ndp" ]; then
        echo "/opt/ndp/bin/ndp"
    elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
        echo "$REPO_ROOT/target/release/ndp"
    elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
        echo "$REPO_ROOT/target/debug/ndp"
    else
        error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
        return 1
    fi
}

# Usage:
local ndp_tool
ndp_tool=$(resolve_ndp_tool) || return 1

NOTE: This optimization is OPTIONAL for v1.1.14. The ndp tool resolution
pattern already exists at 3 sites (lines 386, 894, 1063) with the same
4-way lookup. Adding 2 more sites (gold dispatch) follows the established
pattern. The extraction to a function can be done in v1.1.15 or later.
```

---

## 8. Migration Script Pseudocode

### Step-by-step migration procedure with verification gates.

```
ALGORITHM: Gold Module Migration
INPUT: current codebase at v1.1.13
OUTPUT: codebase at v1.1.14 with gold module in ndp-lib

PRE-FLIGHT:
    cargo test -p ndp-gold-ddl    # Record baseline: 376 tests passing
    cargo test -p ndp-lib          # Record baseline: existing tests passing
    cargo test -p ndp-cli          # Record baseline: existing tests passing

STEP 1: Create module structure in ndp-lib
    mkdir -p crates/ndp-lib/src/gold/config
    mkdir -p crates/ndp-lib/src/gold/generators
    mkdir -p crates/ndp-lib/src/gold/planner
    mkdir -p crates/ndp-lib/src/gold/registry
    mkdir -p crates/ndp-lib/src/gold/validation
    mkdir -p crates/ndp-lib/src/gold/db

STEP 2: Copy source files (NOT git mv yet -- keep originals for comparison)
    # Config module (4 files)
    cp tools/ndp-gold-ddl/src/config/mod.rs     crates/ndp-lib/src/gold/config/mod.rs
    cp tools/ndp-gold-ddl/src/config/types.rs   crates/ndp-lib/src/gold/config/types.rs
    cp tools/ndp-gold-ddl/src/config/domain.rs  crates/ndp-lib/src/gold/config/domain.rs
    cp tools/ndp-gold-ddl/src/config/loader.rs  crates/ndp-lib/src/gold/config/loader.rs

    # Error module (1 file)
    cp tools/ndp-gold-ddl/src/error.rs          crates/ndp-lib/src/gold/error.rs

    # Generators module (11 files)
    cp tools/ndp-gold-ddl/src/generators/mod.rs                crates/ndp-lib/src/gold/generators/mod.rs
    cp tools/ndp-gold-ddl/src/generators/aligned_view.rs       crates/ndp-lib/src/gold/generators/aligned_view.rs
    cp tools/ndp-gold-ddl/src/generators/classification.rs     crates/ndp-lib/src/gold/generators/classification.rs
    cp tools/ndp-gold-ddl/src/generators/column_builder.rs     crates/ndp-lib/src/gold/generators/column_builder.rs
    cp tools/ndp-gold-ddl/src/generators/constants.rs          crates/ndp-lib/src/gold/generators/constants.rs
    cp tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs crates/ndp-lib/src/gold/generators/continuous_aggregate.rs
    cp tools/ndp-gold-ddl/src/generators/events.rs             crates/ndp-lib/src/gold/generators/events.rs
    cp tools/ndp-gold-ddl/src/generators/join_builder.rs       crates/ndp-lib/src/gold/generators/join_builder.rs
    cp tools/ndp-gold-ddl/src/generators/null_handler.rs       crates/ndp-lib/src/gold/generators/null_handler.rs
    cp tools/ndp-gold-ddl/src/generators/refresh_policy.rs     crates/ndp-lib/src/gold/generators/refresh_policy.rs
    cp tools/ndp-gold-ddl/src/generators/state_transitions.rs  crates/ndp-lib/src/gold/generators/state_transitions.rs

    # Planner module (2 files)
    cp tools/ndp-gold-ddl/src/planner/mod.rs    crates/ndp-lib/src/gold/planner/mod.rs
    cp tools/ndp-gold-ddl/src/planner/sync.rs   crates/ndp-lib/src/gold/planner/sync.rs

    # Registry module (5 files)
    cp tools/ndp-gold-ddl/src/registry/mod.rs       crates/ndp-lib/src/gold/registry/mod.rs
    cp tools/ndp-gold-ddl/src/registry/lag.rs        crates/ndp-lib/src/gold/registry/lag.rs
    cp tools/ndp-gold-ddl/src/registry/rolling.rs    crates/ndp-lib/src/gold/registry/rolling.rs
    cp tools/ndp-gold-ddl/src/registry/trait_def.rs  crates/ndp-lib/src/gold/registry/trait_def.rs
    cp tools/ndp-gold-ddl/src/registry/trend.rs      crates/ndp-lib/src/gold/registry/trend.rs

    # Validation module (2 files)
    cp tools/ndp-gold-ddl/src/validation/mod.rs              crates/ndp-lib/src/gold/validation/mod.rs
    cp tools/ndp-gold-ddl/src/validation/config_validator.rs crates/ndp-lib/src/gold/validation/config_validator.rs

    # DB module (3 files -- but only DbError and CaChecker/CaInfo/PostgresCaChecker)
    cp tools/ndp-gold-ddl/src/db/mod.rs      crates/ndp-lib/src/gold/db/mod.rs
    cp tools/ndp-gold-ddl/src/db/client.rs   crates/ndp-lib/src/gold/db/client.rs
    cp tools/ndp-gold-ddl/src/db/queries.rs  crates/ndp-lib/src/gold/db/queries.rs

    # Total: 29 files copied

STEP 3: Update use paths in copied files
    # Systematic find-and-replace in crates/ndp-lib/src/gold/**/*.rs:
    #
    # Pattern: crate::config::  -> crate::gold::config::
    # Pattern: crate::error::   -> crate::gold::error::
    # Pattern: crate::generators:: -> crate::gold::generators::
    # Pattern: crate::planner:: -> crate::gold::planner::
    # Pattern: crate::registry:: -> crate::gold::registry::
    # Pattern: crate::validation:: -> crate::gold::validation::
    # Pattern: crate::db::      -> crate::gold::db::    (for CaChecker, CaInfo)
    #
    # EXCEPTION: gold/db/queries.rs references DbClient
    #   OLD: use super::client::DbClient
    #   NEW: use crate::DbClient    (ndp_lib's DbClient trait)
    #
    # EXCEPTION: gold/db/client.rs
    #   Remove PostgresClient struct and impl (use crate::db::PostgresClient)
    #   Keep DbError enum

STEP 4: Wire gold module into ndp-lib
    # crates/ndp-lib/src/lib.rs -- add:
    pub mod gold;

STEP 5: Create gold/mod.rs with top-level convenience functions
    # Write the mod.rs with generate_stream(), sync_stream(), etc.
    # (See Section 1.2 above)

STEP 6: Update Cargo.toml dependencies
    # crates/ndp-lib/Cargo.toml -- add dev-dependencies:
    [dev-dependencies]
    mockall = "0.12"
    pretty_assertions = "1"

    # crates/ndp-lib/Cargo.toml -- add dependencies (if not already present):
    async-trait = "0.1"   # already present
    tokio = { version = "1", features = ["full"] }  # check if already present
    tempfile = "3"        # dev-dependency for config loader tests

STEP 7: Wire CaChecker to use ndp_lib::DbClient
    # Modify gold/db/queries.rs:
    #   PostgresCaChecker<C: crate::DbClient> instead of <C: super::client::DbClient>
    #   Error mapping: crate::NdpLibError -> DbError
    # Modify gold/db/client.rs:
    #   Remove PostgresClient (duplicated from crate::db::PostgresClient)
    #   Keep only DbError enum
    # Modify gold/db/mod.rs:
    #   Remove DbClient re-export
    #   Keep CaChecker, CaInfo, PostgresCaChecker, DbError

    VERIFICATION GATE:
        cargo test -p ndp-lib           # All existing + new gold tests pass
        cargo test -p ndp-lib -- gold   # Focus on gold module tests

STEP 8: Update ndp-gold-ddl to depend on ndp-lib
    # tools/ndp-gold-ddl/Cargo.toml:
    #   Add: ndp-lib = { path = "../../crates/ndp-lib" }
    #
    # tools/ndp-gold-ddl/src/lib.rs:
    #   Re-export from ndp_lib::gold instead of local modules
    #
    # tools/ndp-gold-ddl/src/main.rs:
    #   Update imports to use ndp_lib::gold::* (thin wrapper)

    VERIFICATION GATE:
        cargo test -p ndp-gold-ddl       # Standalone binary still works
        # Compare output:
        diff <(cargo run -p ndp-gold-ddl -- generate --stream air-quality --config-dir config) \
             <(echo "baseline output from before migration")

STEP 9: Add commands/gold.rs to ndp-cli
    # tools/ndp-cli/src/commands/gold.rs  (new file, see Section 5)
    # tools/ndp-cli/src/commands/mod.rs   (add: pub mod gold;)
    # tools/ndp-cli/src/main.rs           (add Gold variant to Commands enum)

    VERIFICATION GATE:
        cargo build -p ndp-cli
        # Test parity:
        diff <(cargo run -p ndp-gold-ddl -- generate --stream air-quality --config-dir config) \
             <(cargo run -p ndp-cli -- gold generate --stream air-quality --config-dir config)

STEP 10: Update deploy.sh
    # Site 1: handle_gold_tables()          -- switch to ndp gold
    # Site 2: handle_domain_declaration()   -- switch to ndp gold
    # (See Section 7 for exact pseudocode)

STEP 11: Integration test
    docker compose -f docker-compose.integration.yml up -d
    cargo build -p ndp-cli
    DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
    # Verify: Gold phases complete without error

    VERIFICATION GATE:
        All phases pass. deploy.sh exit code 0.

STEP 12: Final verification
    cargo test --workspace     # ALL workspace tests pass
    cargo test -p ndp-lib      # 376+ gold tests in ndp-lib
    cargo test -p ndp-gold-ddl # Standalone tests still pass
    cargo test -p ndp-cli      # CLI tests pass
```

---

## 9. Error Type Integration

### 9.1 GoldDdlError in ndp-lib

`GoldDdlError` moves to `crates/ndp-lib/src/gold/error.rs` unchanged. It does NOT merge with `NdpLibError`. The gold module maintains its own error type for domain-specific error codes.

```
RELATIONSHIP:
    ndp_lib::error::NdpLibError       -- library-wide errors (Database, ConfigNotFound, etc.)
    ndp_lib::gold::error::GoldDdlError -- gold-specific errors (InvalidMetric, InvalidGranularity, etc.)

CONVERSION:
    The top-level functions (generate_stream, sync_stream) return gold::Result<String>
    which uses GoldDdlError. The CLI layer converts to Box<dyn Error>.

    In v1.1.16, cross-cutting validation will need GoldDdlError -> NdpLibError conversion.
    For v1.1.14, they remain independent.
```

### 9.2 Error Flow

```
DIAGRAM: Error Flow

    ndp gold sync --stream X
        |
        v
    commands/gold.rs::run()
        | returns Result<(), Box<dyn Error>>
        |
        v
    ndp_lib::gold::sync_stream()
        | returns gold::Result<String>  (GoldDdlError)
        |
        +---> CaChecker::ca_exists()
        |     | returns Result<bool, DbError>
        |     | DbError -> GoldDdlError::DatabaseError(e.to_string())
        |
        +---> ContinuousAggregateGenerator::generate()
        |     | returns gold::Result<String>
        |
        +---> SyncPlanner::plan()
              | returns gold::Result<SyncPlan>

    At CLI boundary:
        gold::Result<String> -> Result<(), Box<dyn Error>>
        via .map_err(|e| e.into())   (GoldDdlError implements std::error::Error)
```

---

## 10. Cargo.toml Changes

### 10.1 crates/ndp-lib/Cargo.toml

```toml
# v1.1.14 additions to [dependencies]:
# (Most are already present -- verify each)
serde = { version = "1", features = ["derive"] }      # likely already present
serde_json = "1"                                        # likely already present
async-trait = "0.1"                                     # likely already present
thiserror = "1"                                         # likely already present
tokio = { version = "1", features = ["full"] }          # likely already present
tokio-postgres = { version = "0.7", features = ["with-serde_json-1"] }  # likely already present
tracing = "0.1"                                         # likely already present

# v1.1.14 additions to [dev-dependencies]:
mockall = "0.12"            # for gold planner mock tests (CaChecker mocks)
pretty_assertions = "1"     # for gold integration tests
tempfile = "3"              # for config loader tests (already present?)
```

### 10.2 tools/ndp-gold-ddl/Cargo.toml

```toml
# ADD to [dependencies]:
ndp-lib = { path = "../../crates/ndp-lib" }

# REMOVE from [dependencies] (now provided by ndp-lib):
# NOTE: Only remove after verifying ndp-lib re-exports everything needed.
# In v1.1.14, KEEP all deps to avoid breakage. Remove in v1.1.16 cleanup.
```

### 10.3 tools/ndp-cli/Cargo.toml

```toml
# No Cargo.toml changes needed for ndp-cli.
# ndp-cli already depends on ndp-lib.
# Gold commands use ndp_lib::gold::* which is in the same ndp-lib crate.
```

---

## 11. Complexity Analysis

### 11.1 Migration Complexity

```
ANALYSIS: Migration File Counts

Source files to copy:     29
    config/       4 files (mod.rs, types.rs, domain.rs, loader.rs)
    error.rs      1 file
    generators/  11 files (mod.rs + 10 generators)
    planner/      2 files (mod.rs, sync.rs)
    registry/     5 files (mod.rs + 4 generators)
    validation/   2 files (mod.rs, config_validator.rs)
    db/           3 files (mod.rs, client.rs, queries.rs)
    gold/mod.rs   1 file  (NEW -- top-level API)

New files to create:      2
    crates/ndp-lib/src/gold/mod.rs     (top-level API)
    tools/ndp-cli/src/commands/gold.rs (CLI command)

Files to modify:          5
    crates/ndp-lib/src/lib.rs           (add pub mod gold)
    tools/ndp-cli/src/commands/mod.rs   (add pub mod gold)
    tools/ndp-cli/src/main.rs           (add Gold variant)
    tools/ndp-gold-ddl/Cargo.toml       (add ndp-lib dep)
    deploy/pi/deploy.sh                 (2 dispatch sites)

Tests to migrate:       376
    Unit tests:         ~340 (inline in source files)
    Integration tests:  ~36  (in tools/ndp-gold-ddl/tests/)
```

### 11.2 Algorithmic Complexity (Unchanged by Migration)

```
ANALYSIS: Gold DDL Generation

generate_stream():
    Time:  O(G * F * M) where G=granularities, F=fields, M=metrics per field
    Space: O(G * F * M) for DDL string assembly
    Typical: G=2, F=5, M=3 -> O(30) -> constant time

sync_stream():
    Time:  O(G * (DB_QUERY + F * M))
           DB_QUERY = network round trip per granularity (ca_exists + policy_exists)
    Space: O(G * F * M) for SyncPlan
    Typical: 2-4 DB queries + O(30) generation -> dominated by DB queries

generate_domain():
    Time:  O(S * F) where S=streams in domain, F=fields per stream
    Space: O(S * F) for column list and join SQL
    Typical: S=3, F=5 -> O(15)

All operations are I/O-bound (DB queries, file reads), not compute-bound.
Migration does not change algorithmic complexity.
```

### 11.3 Risk Assessment

```
RISK MATRIX:

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Use path find-replace misses a reference | Medium | Low | cargo check catches immediately |
| CaChecker DbClient adapter breaks mocks | Low | Medium | Mock tests in planner/sync.rs verify |
| Gold ConfigLoader path resolution wrong | Medium | Medium | Test with both config/base and config/ |
| deploy.sh action->verb mapping wrong | Low | High | Integration test catches |
| ndp-gold-ddl standalone breaks | Low | Medium | Dedicated test: cargo test -p ndp-gold-ddl |
| Circular dependency introduced | None | -- | ndp-lib has no workspace deps except ndp-types |
```

---

## Appendix A: File Inventory

Complete list of files involved in the v1.1.14 migration:

```
MOVED (29 files, tools/ndp-gold-ddl/src/ -> crates/ndp-lib/src/gold/):
  config/mod.rs
  config/types.rs
  config/domain.rs
  config/loader.rs
  error.rs
  generators/mod.rs
  generators/aligned_view.rs
  generators/classification.rs
  generators/column_builder.rs
  generators/constants.rs
  generators/continuous_aggregate.rs
  generators/events.rs
  generators/join_builder.rs
  generators/null_handler.rs
  generators/refresh_policy.rs
  generators/state_transitions.rs
  planner/mod.rs
  planner/sync.rs
  registry/mod.rs
  registry/lag.rs
  registry/rolling.rs
  registry/trait_def.rs
  registry/trend.rs
  validation/mod.rs
  validation/config_validator.rs
  db/mod.rs
  db/client.rs
  db/queries.rs

CREATED (2 files):
  crates/ndp-lib/src/gold/mod.rs
  tools/ndp-cli/src/commands/gold.rs

MODIFIED (5 files):
  crates/ndp-lib/src/lib.rs
  tools/ndp-cli/src/commands/mod.rs
  tools/ndp-cli/src/main.rs
  tools/ndp-gold-ddl/Cargo.toml
  deploy/pi/deploy.sh

MODIFIED (after migration, thin wrapper):
  tools/ndp-gold-ddl/src/lib.rs
  tools/ndp-gold-ddl/src/main.rs
```

---

## Appendix B: Flag Mapping Quick Reference

```
STANDALONE -> SUBCOMMAND:

ndp-gold-ddl generate --stream X                -> ndp gold generate --stream X
ndp-gold-ddl generate --domain X                -> ndp gold generate --domain X
ndp-gold-ddl generate --stream X --transitions  -> ndp gold generate --stream X --transitions
ndp-gold-ddl generate --domain X --events       -> ndp gold generate --domain X --events
ndp-gold-ddl generate --stream X --action sync --database-url U
                                                -> ndp gold sync --stream X --db-url U
ndp-gold-ddl generate --stream X --action recreate --database-url U
                                                -> ndp gold recreate --stream X --db-url U
ndp-gold-ddl validate --stream X                -> ndp gold generate --stream X --validate-only
--config-dir                                    -> --config-dir (global)
--database-url                                  -> --db-url (global, harmonized)
--db-timeout                                    -> --db-timeout (per-subcommand)
--verbose                                       -> --verbose (global, future)
```
