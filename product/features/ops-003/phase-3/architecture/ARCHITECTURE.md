# OPS-003 Phase 3 Architecture: Shared Constants + Cross-cutting Validation (v1.1.17)

> **Author**: ndp-architect
> **Date**: 2026-02-08
> **Status**: Proposed
> **Scope**: ops-003-08 through ops-003-13
> **Predecessor**: Phase 2 (v1.1.15 Validate Migration) + Phase 1 (v1.1.14 Gold Migration)

---

## ADR-003-003: Shared Constants and Cross-cutting Validation

### Status

Proposed

### Context

After Phase 1 (Gold migration into `ndp_lib::gold`) and Phase 2 (Validate migration into `ndp_lib::validate`), both modules live as siblings in `ndp-lib`. However, they still operate as islands -- each carries its own copy of constants, its own validation logic, and the CLI carries three copies of `NoOpDbClient`. Specifically:

1. **Constants duplication**: `VALID_METRICS` is defined identically in two places (`gold::config::types` and `validate::semantic::gold`). `VALID_ROLLING_STATS` and `VALID_STATS` are defined in one place each but represent overlapping concepts. `GOLD_SCHEMA`, `SILVER_SCHEMA`, and `NDP_ENTITY_COLUMN` are scoped inside `gold::generators::constants` but represent platform-wide concepts needed by future modules (e.g., Silver ETL, MCP server).

2. **No cross-cutting validation**: `gold::sync()` does not call `validate::gold_config()` before generating DDL. The `--no-validate` flag exists in the CLI (captured by clap) but the value is discarded with `no_validate: _` in the gold command handler. `SyncOptions` lacks a `validate` field.

3. **Dual Gold validation**: `gold::validation::ConfigValidator` validates Gold config using typed `StreamConfig` structs. `validate::semantic::gold::validate_gold_etl()` validates the same Gold config using `serde_json::Value`. Both check metrics, granularity, and field references, but with different approaches, different error types, and slightly different coverage.

4. **NoOpDbClient proliferation**: Four implementations exist -- one canonical version in `ndp_lib::db` (which returns `Ok(empty)`) and three copies in ndp-cli command modules (`dictionary.rs`, `dimension.rs`, `domain.rs`) that use `unreachable!()` instead. The ndp-lib version is already re-exported via `pub use db::NoOpDbClient` but the CLI commands don't use it.

### Decision

1. Extract all platform-wide constants to `ndp_lib::constants`. Both `gold` and `validate` import from this single location.

2. Add a `validate` field to `SyncOptions`. Wire `gold::sync_stream()` and `gold::sync_domain()` to call `validate::semantic::gold::validate_gold_etl()` when `opts.validate` is true (default). Map the existing `--no-validate` CLI flag to `SyncOptions { validate: false }`.

3. Remove `gold::validation::ConfigValidator` as a standalone struct. The `validate_gold_config()` convenience function survives but delegates to shared constants. The generator-level validation in `ContinuousAggregateGenerator` (which also checks metrics) remains because it validates at generation time -- separate from pre-flight validation.

4. Consolidate all `NoOpDbClient` usage to the single `ndp_lib::db::NoOpDbClient`. Remove the three copies from ndp-cli.

### Consequences

**Easier:**
- Adding a new metric requires changing one line in one file (`constants.rs`).
- Every Gold mutation validates config by default -- invalid config never reaches DDL generation.
- Future modules (Silver ETL, MCP server) import schema names from `ndp_lib::constants` instead of hardcoding strings.
- CLI command modules shrink by ~25 lines each (removed NoOpDbClient boilerplate).

**Harder:**
- `constants.rs` becomes a coupling point: changes to VALID_METRICS affect both gold generation and validation. This is intentional -- they SHOULD be in sync.
- The `gold::validation` module shrinks significantly. Its parsing functions (`parse_granularity`, `parse_window`, `granularity_to_suffix`) remain because they are used by generators and registry modules, not just validation.
- Removing ConfigValidator requires verifying that `validate::semantic::gold::validate_gold_etl()` covers every check ConfigValidator performs (gap analysis in Section 6).

### Alternatives Considered

**A1: Keep constants in gold::generators::constants, re-export from lib.rs (rejected)**

This preserves the gold-centric ownership but forces validate to import from a gold submodule, creating a semantic dependency from validate to gold. Constants like `GOLD_SCHEMA` and `SILVER_SCHEMA` are platform concepts, not gold module concepts.

**A2: Create a separate `ndp-constants` crate (rejected)**

Over-engineering for 6 constants. A module in ndp-lib is sufficient. If constants grow to 50+ items or need independent versioning, revisit.

**A3: Keep both validation paths, just share constants (rejected)**

Sharing constants but keeping two validators means two places to update when validation rules change. The whole point of Phase 3 is unification.

---

## 1. Module Layout

### 1.1 New File: `crates/ndp-lib/src/constants.rs`

```
crates/ndp-lib/src/
  lib.rs                          # Add: pub mod constants;
  constants.rs                    # NEW -- platform-wide constants
  types.rs                        # MODIFIED -- add validate field to SyncOptions
  db.rs                           # UNCHANGED -- NoOpDbClient already here
  gold/
    config/
      types.rs                    # MODIFIED -- remove VALID_METRICS, VALID_ROLLING_STATS
      mod.rs                      # MODIFIED -- remove VALID_METRICS, VALID_ROLLING_STATS re-exports
    generators/
      constants.rs                # MODIFIED -- remove GOLD_SCHEMA, SILVER_SCHEMA, NDP_ENTITY_COLUMN
      continuous_aggregate.rs     # MODIFIED -- import from crate::constants
      events.rs                   # MODIFIED -- import from crate::constants
      state_transitions.rs        # MODIFIED -- import from crate::constants
      aligned_view.rs             # MODIFIED -- import from crate::constants
    validation/
      config_validator.rs         # MODIFIED -- import constants from crate::constants, simplify
      mod.rs                      # UNCHANGED -- still exports parse_granularity, etc.
    mod.rs                        # MODIFIED -- sync_stream/sync_domain call validation
  validate/
    semantic/
      gold.rs                     # MODIFIED -- import from crate::constants
      mod.rs                      # UNCHANGED
    mod.rs                        # UNCHANGED

tools/ndp-cli/src/commands/
  dictionary.rs                   # MODIFIED -- remove local NoOpDbClient
  dimension.rs                    # MODIFIED -- remove local NoOpDbClient
  domain.rs                       # MODIFIED -- remove local NoOpDbClient
  gold.rs                         # MODIFIED -- wire no_validate to SyncOptions
```

### 1.2 Module Hierarchy After Phase 3

```rust
// crates/ndp-lib/src/lib.rs
pub mod config;
pub mod constants;    // NEW
pub mod convert;
pub mod db;
pub mod dictionary;
pub mod dimension;
pub mod domain;
pub mod error;
pub mod gold;
pub mod types;
pub mod validate;

// Re-exports
pub use db::{DbClient, NoOpDbClient};  // NoOpDbClient already exported
pub use error::{NdpLibError, Result};
pub use types::{SyncError, SyncOptions, SyncReport};
```

---

## 2. Constants Inventory

### 2.1 Complete Constant Audit

Every duplicated or platform-scope constant in the codebase, with exact locations:

| Constant | Type | Location 1 | Location 2 | Target |
|----------|------|------------|------------|--------|
| `VALID_METRICS` | `&[&str]` (9 items) | `crates/ndp-lib/src/gold/config/types.rs:11` | `crates/ndp-lib/src/validate/semantic/gold.rs:21` | `ndp_lib::constants` |
| `VALID_ROLLING_STATS` | `&[&str]` (4 items) | `crates/ndp-lib/src/gold/config/types.rs:16` | (none -- single location) | `ndp_lib::constants` |
| `VALID_STATS` | `&[&str]` (4 items) | `crates/ndp-lib/src/validate/semantic/gold.rs:26` | (none -- single location) | `ndp_lib::constants` (merge with VALID_ROLLING_STATS) |
| `GOLD_SCHEMA` | `&str` ("gold") | `crates/ndp-lib/src/gold/generators/constants.rs:10` | (none -- single location) | `ndp_lib::constants` |
| `SILVER_SCHEMA` | `&str` ("silver") | `crates/ndp-lib/src/gold/generators/constants.rs:13` | (none -- single location) | `ndp_lib::constants` |
| `NDP_ENTITY_COLUMN` | `&str` ("ndp_id") | `crates/ndp-lib/src/gold/generators/constants.rs:7` | (none -- single location) | `ndp_lib::constants` |

### 2.2 Constant Values

```
VALID_METRICS      = ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]
VALID_ROLLING_STATS = ["mean", "std", "min", "max"]
VALID_STATS        = ["mean", "std", "min", "max"]
GOLD_SCHEMA        = "gold"
SILVER_SCHEMA      = "silver"
NDP_ENTITY_COLUMN  = "ndp_id"
```

### 2.3 VALID_ROLLING_STATS vs VALID_STATS Merge

`VALID_ROLLING_STATS` (gold config types) and `VALID_STATS` (validate semantic gold) have identical values: `["mean", "std", "min", "max"]`. They represent the same concept -- valid statistics for rolling window features. In `constants.rs`, they become a single constant named `VALID_ROLLING_STATS`.

### 2.4 Target File: `crates/ndp-lib/src/constants.rs`

```rust
//! Platform-wide constants for NDP.
//!
//! Single source of truth for schema names, valid metric/stat lists,
//! and column naming conventions. Used by both `gold` and `validate` modules.

/// Valid aggregate metrics for Gold continuous aggregates.
///
/// These metrics can appear in `gold_etl.aggregates.fields.<name>.metrics`
/// and `gold_etl.aggregates.default_metrics`.
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];

/// Valid statistics for rolling window features.
///
/// A subset of VALID_METRICS. These stats can appear in
/// `gold_etl.features.rolling.stats`.
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

/// Gold schema name. All Gold layer database objects are created in this schema.
pub const GOLD_SCHEMA: &str = "gold";

/// Silver schema name. All Silver layer hypertables live in this schema.
pub const SILVER_SCHEMA: &str = "silver";

/// Default entity identifier column used across NDP streams.
///
/// Streams with entity-level data (e.g., multiple sensors) use this column
/// to identify each entity within the stream.
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";
```

### 2.5 Import Changes by Consumer Module

After extraction, each consumer changes its import:

| Consumer File | Old Import | New Import |
|---|---|---|
| `gold/config/types.rs` | (local `const VALID_METRICS`, `const VALID_ROLLING_STATS`) | `use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};` |
| `gold/config/mod.rs` | `pub use types::{..., VALID_METRICS, VALID_ROLLING_STATS}` | Remove from re-export list; consumers import from `crate::constants` directly |
| `gold/generators/constants.rs` | (local `const GOLD_SCHEMA`, etc.) | `pub use crate::constants::{GOLD_SCHEMA, SILVER_SCHEMA, NDP_ENTITY_COLUMN};` (re-export for backward compatibility) |
| `gold/generators/continuous_aggregate.rs` | `use crate::gold::config::{..., VALID_METRICS}` | `use crate::constants::VALID_METRICS;` |
| `gold/generators/events.rs` | `use super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA}` | `use crate::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA};` |
| `gold/generators/state_transitions.rs` | `use super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN}` | `use crate::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN};` |
| `gold/generators/aligned_view.rs` | `use crate::gold::generators::constants::GOLD_SCHEMA` | `use crate::constants::GOLD_SCHEMA;` |
| `gold/validation/config_validator.rs` | `use crate::gold::config::{..., VALID_METRICS, VALID_ROLLING_STATS}` | `use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};` |
| `validate/semantic/gold.rs` | (local `const VALID_METRICS`, `const VALID_STATS`) | `use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};` (rename `VALID_STATS` references to `VALID_ROLLING_STATS`) |

### 2.6 Backward Compatibility: `gold::generators::constants`

The file `crates/ndp-lib/src/gold/generators/constants.rs` currently exports `GOLD_SCHEMA`, `SILVER_SCHEMA`, and `NDP_ENTITY_COLUMN`. After extraction, this file becomes a re-export shim to avoid breaking any consumers that reference the old path:

```rust
//! Shared constants for Gold DDL generators (re-exported from crate::constants).
//!
//! Prefer importing from `ndp_lib::constants` directly.

pub use crate::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA};
```

This allows a gradual migration: existing `use super::constants::GOLD_SCHEMA` paths continue to work while new code uses `crate::constants::GOLD_SCHEMA`.

---

## 3. Cross-cutting Validation Architecture

### 3.1 SyncOptions.validate Field

Add a `validate` field to `SyncOptions` in `crates/ndp-lib/src/types.rs:37`:

```rust
/// Options for sync operations.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// If true, generate SQL but do not execute against the database.
    pub dry_run: bool,
    /// If true, validate config before mutating (default: true).
    pub validate: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            validate: true,
        }
    }
}
```

This is a breaking change to `SyncOptions` because it no longer derives `Default` automatically (it uses a manual impl). All existing construction sites use `SyncOptions { dry_run: <bool> }` which will fail to compile with a missing `validate` field. This is desirable -- it forces every consumer to make an explicit choice about validation.

### 3.2 Construction Site Updates

Every place that constructs `SyncOptions` must be updated:

| File | Line | Current | After |
|---|---|---|---|
| `tools/ndp-cli/src/commands/gold.rs` | 239 | `SyncOptions { dry_run }` | `SyncOptions { dry_run, validate: !no_validate }` |
| `tools/ndp-cli/src/commands/gold.rs` | 264 | `SyncOptions { dry_run }` | `SyncOptions { dry_run, validate: !no_validate }` |
| `tools/ndp-cli/src/commands/dictionary.rs` | (multiple) | `SyncOptions { dry_run: true/false }` | `SyncOptions { dry_run: true/false, validate: true }` |
| `tools/ndp-cli/src/commands/dimension.rs` | (multiple) | `SyncOptions { dry_run: true/false }` | `SyncOptions { dry_run: true/false, validate: true }` |
| `tools/ndp-cli/src/commands/domain.rs` | (multiple) | `SyncOptions { dry_run: true/false }` | `SyncOptions { dry_run: true/false, validate: true }` |
| `crates/ndp-lib/src/gold/mod.rs` | 141, 167 | `SyncOptions` parameter (not constructed) | No change (receives opts from caller) |

### 3.3 Validation Wiring in gold::sync_stream()

Currently, `gold::sync_stream()` at `crates/ndp-lib/src/gold/mod.rs:137` does no validation. After Phase 3:

```rust
/// Sync Gold DDL for a stream against a real database (idempotent).
pub async fn sync_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    checker: &(impl CaChecker + Send + Sync),
    opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;

    // Cross-cutting validation: validate config before mutating
    if opts.validate {
        let config_json = serde_json::to_value(&stream_config)?;
        let errors = crate::validate::validate_gold_etl(&config_json);
        let blocking_errors: Vec<_> = errors
            .into_iter()
            .filter(|e| e.severity == crate::validate::Severity::Error)
            .collect();
        if !blocking_errors.is_empty() {
            let messages: Vec<String> = blocking_errors
                .iter()
                .map(|e| format!("[{}] {}: {}", e.code, e.path, e.message))
                .collect();
            return Err(format!(
                "Validation failed for stream '{}': {}",
                stream_id,
                messages.join("; ")
            )
            .into());
        }
    }

    let gold_etl = stream_config
        .gold_etl
        .as_ref()
        .ok_or_else(|| format!("Stream '{}' has no gold_etl configuration", stream_id))?;

    if !gold_etl.enabled {
        return Err(format!("Stream '{}' has gold_etl.enabled = false", stream_id).into());
    }

    let planner = SyncPlanner::new(checker, &stream_config);
    let plan = planner.plan(gold_etl).await?;

    Ok(plan.to_ddl())
}
```

### 3.4 Validation Wiring in gold::sync_domain()

`gold::sync_domain()` at `crates/ndp-lib/src/gold/mod.rs:163` generates aligned view DDL. Domain config validation uses `validate::validate_domain_semantic()`:

```rust
/// Sync Gold DDL for a domain.
pub fn sync_domain(
    loader: &(impl ConfigLoader + Clone + 'static),
    domain_id: &str,
    opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let domain_config = loader.load_domain_config(domain_id)?;

    // Cross-cutting validation: validate domain config before generating DDL
    if opts.validate {
        let config_json = serde_json::to_value(&domain_config)?;
        let errors = crate::validate::validate_domain_semantic(&config_json, None);
        let blocking_errors: Vec<_> = errors
            .into_iter()
            .filter(|e| e.severity == crate::validate::Severity::Error)
            .collect();
        if !blocking_errors.is_empty() {
            let messages: Vec<String> = blocking_errors
                .iter()
                .map(|e| format!("[{}] {}: {}", e.code, e.path, e.message))
                .collect();
            return Err(format!(
                "Validation failed for domain '{}': {}",
                domain_id,
                messages.join("; ")
            )
            .into());
        }
    }

    let generator = AlignedViewGenerator::new(loader.clone());
    let sql = generator.generate(&domain_config, Action::Sync)?;
    Ok(sql)
}
```

### 3.5 Sequence Diagram: Cross-cutting Validation

```
CLI (ndp gold sync --stream air-quality)
  |
  v
commands/gold.rs::run_sync()
  | Constructs SyncOptions { validate: !no_validate, dry_run }
  |
  v
ndp_lib::gold::sync_stream(loader, stream_id, checker, opts)
  |
  |-- opts.validate == true ?
  |     |
  |     v
  |   serde_json::to_value(&stream_config)
  |     |
  |     v
  |   ndp_lib::validate::validate_gold_etl(&config_json)
  |     |-- Uses crate::constants::VALID_METRICS
  |     |-- Uses crate::constants::VALID_ROLLING_STATS
  |     |-- Checks field references, granularity, transitions
  |     |
  |     v
  |   Any Severity::Error? --> return Err("Validation failed...")
  |     |
  |     v (no errors)
  |
  |-- Load gold_etl, check enabled
  |
  v
SyncPlanner::plan(gold_etl)
  | Checks existing CAs in DB
  v
SyncPlan::to_ddl()
  | Generates CREATE/SKIP DDL
  v
Return DDL string
```

### 3.6 --no-validate CLI Flag Mapping

The `--no-validate` flag already exists in all three gold subcommands (generate, sync, recreate) in `tools/ndp-cli/src/commands/gold.rs`. Currently the value is captured by clap but discarded:

```rust
// CURRENT (lines 123-124, 135-138):
no_validate: _,  // discarded
```

After Phase 3:

```rust
// In run_sync():
let opts = ndp_lib::types::SyncOptions {
    dry_run,
    validate: !no_validate,
};
```

For `generate` and `recreate`, the `no_validate` flag is also wired to skip pre-generation validation:

```rust
// In run_generate(), before generating DDL:
if !no_validate {
    // Run validation (same pattern as sync)
}

// In run_recreate(), before generating DDL:
if !no_validate {
    // Run validation (same pattern as sync)
}
```

### 3.7 Error Propagation

When validation fails, the error is a formatted `Box<dyn std::error::Error>` containing all blocking validation errors. The CLI surfaces this as a non-zero exit code. This is consistent with how other sync errors (missing gold_etl, disabled gold_etl) are propagated.

Validation warnings (Severity::Warning) do NOT block execution. They are filtered out of the blocking check. If verbose mode is enabled in the future, warnings could be logged via `tracing::warn!()`.

---

## 4. NoOpDbClient Consolidation

### 4.1 Current State: Four Implementations

| Location | Lines | Behavior on call | Used by |
|---|---|---|---|
| `crates/ndp-lib/src/db.rs:94-109` | 16 | Returns `Ok(vec![])` / `Ok(0)` / `Ok(())` | ndp-lib tests, re-exported as `ndp_lib::NoOpDbClient` |
| `tools/ndp-cli/src/commands/dictionary.rs:116-139` | 24 | `unreachable!()` | dictionary dry-run mode |
| `tools/ndp-cli/src/commands/dimension.rs:178-199` | 22 | `unreachable!()` | dimension dry-run mode |
| `tools/ndp-cli/src/commands/domain.rs:146-167` | 22 | `unreachable!()` | domain dry-run mode |

### 4.2 Behavioral Difference

The ndp-lib version returns `Ok(empty)` on all methods -- safe to call, produces no side effects.

The CLI versions panic with `unreachable!()` -- they assume NoOpDbClient methods are never actually invoked during dry-run. This is a stricter contract but creates a runtime panic risk if the assumption breaks.

### 4.3 Decision: Use ndp-lib's Version Everywhere

The ndp-lib `NoOpDbClient` (returns Ok) is the correct choice:

1. It is already the canonical implementation, exported in the library's public API.
2. It is safer: if a code path does accidentally call `query()` during dry-run, it returns empty results instead of panicking.
3. The `unreachable!()` contract is fragile -- if `sync_dictionary()` adds a new query before the dry-run check, the CLI crashes.

### 4.4 Migration: Three Files

Each CLI command file has an identical pattern to update:

**Before** (example from `dictionary.rs:111-139`):
```rust
// ---------------------------------------------------------------------------
// NoOpDbClient for dry-run mode
// ---------------------------------------------------------------------------

use async_trait::async_trait;

struct NoOpDbClient;

#[async_trait]
impl ndp_lib::DbClient for NoOpDbClient {
    // ... 20 lines of unreachable!() implementations
}
```

**After:**
```rust
// (deleted -- use ndp_lib::NoOpDbClient instead)
```

And the usage site changes from:

```rust
ndp_lib::dictionary::sync_dictionary(&entries, &NoOpDbClient, &options).await?;
```

to:

```rust
ndp_lib::dictionary::sync_dictionary(&entries, &ndp_lib::NoOpDbClient, &options).await?;
```

Since `ndp_lib::NoOpDbClient` is already a public re-export from `lib.rs:36`, no new export is needed.

### 4.5 Files Changed

| File | Change | Lines Removed |
|---|---|---|
| `tools/ndp-cli/src/commands/dictionary.rs` | Delete lines 110-139, update usage at line 68 | ~30 |
| `tools/ndp-cli/src/commands/dimension.rs` | Delete lines 173-199, update usage at line 124 | ~27 |
| `tools/ndp-cli/src/commands/domain.rs` | Delete lines 141-167, update usage at line 85 | ~27 |

Total: ~84 lines removed, 3 import paths updated.

### 4.6 async_trait Dependency Cleanup

Each CLI command file imports `use async_trait::async_trait;` solely for the local NoOpDbClient impl. After removal, this import becomes unused and should be deleted. Check that `async_trait` is not used for anything else in these files.

---

## 5. Gold Validation Unification

### 5.1 What ConfigValidator Does Today

`ConfigValidator` at `crates/ndp-lib/src/gold/validation/config_validator.rs:10-168` performs these checks on a typed `gold::config::StreamConfig`:

| Check | Method | Lines | Error Type |
|---|---|---|---|
| gold_etl present | `validate()` | 21-28 | `GoldDdlError::MissingRequiredField` |
| gold_etl.enabled | `validate()` | 30-34 | `GoldDdlError::GoldEtlDisabled` |
| Granularity format | `validate_granularities()` | 54-59 | `GoldDdlError::InvalidGranularity` |
| Aggregate field exists in stream | `validate_aggregate_fields()` | 62-76 | `GoldDdlError::FieldNotFound` |
| Aggregate metric is valid | `validate_aggregate_fields()` | 78-87 | `GoldDdlError::InvalidMetric` |
| Lag hours non-empty when enabled | `validate_features()` | 101-108 | `GoldDdlError::InvalidFeatureConfig` |
| Lag hours >= 1 | `validate_features()` | 110-117 | `GoldDdlError::InvalidFeatureConfig` |
| Rolling windows non-empty when enabled | `validate_features()` | 127-132 | `GoldDdlError::InvalidFeatureConfig` |
| Rolling window format valid | `validate_features()` | 134-136 | `GoldDdlError::InvalidWindow` |
| Rolling stat is valid | `validate_features()` | 138-148 | `GoldDdlError::InvalidFeatureConfig` |
| Trend window non-empty when enabled | `validate_features()` | 155-160 | `GoldDdlError::InvalidFeatureConfig` |
| Trend window format valid | `validate_features()` | 162 | `GoldDdlError::InvalidWindow` |

### 5.2 What validate::semantic::gold::validate_gold_etl() Does Today

`validate_gold_etl()` at `crates/ndp-lib/src/validate/semantic/gold.rs:40-83` performs these checks on a `serde_json::Value`:

| Check | Function | Lines | Error Type |
|---|---|---|---|
| gold_etl.enabled check | `validate_gold_etl()` | 44-56 | (skip if not enabled) |
| Granularity format | `validate_aggregates()` | 106-123 | `ValidationError::InvalidGranularity` |
| Default metrics valid | `validate_aggregates()` | 127-148 | `ValidationError::InvalidAggregateMetric` |
| Aggregate field exists in stream | `validate_aggregates()` | 151-169 | `ValidationError::InvalidGoldField` |
| Aggregate metric is valid | `validate_aggregates()` | 172-196 | `ValidationError::InvalidAggregateMetric` |
| Lag field exists in stream | `validate_features()` | 215-232 | `ValidationError::InvalidGoldField` |
| Rolling window format valid | `validate_features()` | 244-263 | `ValidationError::InvalidGranularity` |
| Rolling stat is valid | `validate_features()` | 266-286 | `ValidationError::InvalidFeatureType` |
| Rolling field exists in stream | `validate_features()` | 289-306 | `ValidationError::InvalidGoldField` |
| Trend window format valid | `validate_features()` | 318-332 | `ValidationError::InvalidGranularity` |
| Trend field exists in stream | `validate_features()` | 336-354 | `ValidationError::InvalidGoldField` |
| Transitions on state_event | `validate_transitions()` | 374-388 | `ValidationError::InvalidStreamType` (Warning) |
| Transitions state_field exists | `validate_transitions()` | 391-404 | `ValidationError::InvalidGoldField` |
| Transitions entity_field exists | `validate_transitions()` | 407-421 | `ValidationError::InvalidGoldField` |

### 5.3 Gap Analysis

| Check | ConfigValidator | validate_gold_etl | Gap |
|---|---|---|---|
| gold_etl present | YES (error) | NO (skips) | **Gap A**: validate_gold_etl silently returns empty; ConfigValidator returns error |
| gold_etl.enabled | YES (error) | YES (skips) | Different semantics: ConfigValidator errors, validate_gold_etl silently skips |
| Granularity format | YES | YES | Covered (different granularity parsers -- see 5.4) |
| Default metrics valid | NO | YES | **Gap B**: ConfigValidator doesn't check default_metrics |
| Aggregate field exists | YES | YES | Covered |
| Aggregate metric valid | YES | YES | Covered |
| Lag hours non-empty | YES | NO | **Gap C**: ConfigValidator checks, validate_gold_etl doesn't |
| Lag hours >= 1 | YES | NO | **Gap D**: ConfigValidator checks, validate_gold_etl doesn't |
| Lag field exists | NO (skip, see note) | YES | Opposite gap: validate_gold_etl checks against stream fields; ConfigValidator notes lag.fields reference aggregate output columns |
| Rolling windows non-empty | YES | NO | **Gap E**: ConfigValidator checks, validate_gold_etl doesn't |
| Rolling window format | YES | YES | Covered |
| Rolling stat valid | YES | YES | Covered |
| Rolling field exists | NO | YES | validate_gold_etl checks against stream fields |
| Trend window non-empty | YES | NO | **Gap F**: ConfigValidator checks, validate_gold_etl doesn't |
| Trend window format | YES | YES | Covered |
| Trend field exists | NO | YES | validate_gold_etl checks against stream fields |
| Transitions on state_event | NO | YES | validate_gold_etl has it; ConfigValidator doesn't |
| Transitions state_field exists | NO | YES | validate_gold_etl has it; ConfigValidator doesn't |
| Transitions entity_field exists | NO | YES | validate_gold_etl has it; ConfigValidator doesn't |
| "Did you mean" suggestions | NO | YES | validate_gold_etl provides Levenshtein suggestions |

### 5.4 Granularity Parsing: Two Implementations

There are two distinct granularity validators:

1. **`validate::semantic::mod::is_valid_granularity()`** (line 60): Uses a regex `^\d+\s+(minute|hour|day)s?$`. Does NOT accept "week".

2. **`gold::validation::config_validator::parse_granularity()`** (line 183): Parses to `(u32, String)`. Accepts "minute", "hour", "day", AND "week" units.

These have a semantic difference: the validate module rejects "1 week" while the gold module accepts it. Since `parse_granularity()` is also used by generators and registry modules (for DDL generation), and TimescaleDB continuous aggregates support weekly buckets, the gold module's behavior is correct. The validate module's regex is too restrictive.

**Resolution**: The `is_valid_granularity()` function in `validate::semantic::mod.rs` should be updated to accept "week(s)" to match the gold module. This is a bug fix, not a design change. The regex becomes `^\d+\s+(minute|hour|day|week)s?$`.

### 5.5 Unification Strategy

1. **validate_gold_etl() becomes the canonical pre-flight validator.** It operates on `serde_json::Value`, which means it can be called from `gold::sync_stream()` by serializing the typed config to JSON.

2. **Add missing checks from ConfigValidator into validate_gold_etl():**
   - Gap C: Lag hours non-empty when enabled
   - Gap D: Lag hours >= 1
   - Gap E: Rolling windows non-empty when enabled
   - Gap F: Trend window non-empty when enabled

3. **Keep gaps A/B as-is:**
   - Gap A (gold_etl present): Not relevant for pre-flight validation. If gold_etl is missing, the caller (sync_stream) already handles this with an explicit error.
   - Gap B (default_metrics): Already covered by validate_gold_etl.

4. **Keep ConfigValidator, but simplify it.** ConfigValidator's `validate()` method is called by `gold::generate_stream()` and `run_validate_only()` in the CLI. It provides typed error reporting via `GoldDdlError`. Instead of deleting it, it should be reduced to:
   - Check gold_etl exists and is enabled (essential for generators)
   - Validate granularities (via parse_granularity -- needed by generators)
   - Delegate metric/field/feature checks to the constants in `crate::constants`
   - Remove duplicate checks now handled by validate_gold_etl

   The reason to keep a minimal ConfigValidator: generators call `parse_granularity()` and `parse_window()` for DDL generation, not just validation. These parsing functions must remain in `gold::validation` because they return parsed values `(u32, String)` that generators use. The semantic validator only does boolean checks.

5. **Remove the `validate_gold_config()` convenience wrapper** in `gold::validation::config_validator.rs:177-179`. This function just calls `ConfigValidator::new().validate(config)`. After Phase 3, pre-flight validation should use `validate::validate_gold_etl()` instead. The explicit `ConfigValidator::new().validate()` pattern remains available for generator-level validation.

### 5.6 ConfigValidator After Simplification

```rust
// gold::validation::config_validator.rs (simplified)

/// Validates Gold ETL configuration for DDL generation.
///
/// This validator checks config preconditions needed by the generators:
/// - gold_etl exists and is enabled
/// - Granularity strings parse to valid (value, unit) pairs
///
/// For comprehensive semantic validation (field references, metric validity,
/// feature configuration), use `validate::validate_gold_etl()` instead.
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn new() -> Self { Self }

    pub fn validate(&self, config: &StreamConfig) -> Result<()> {
        let gold_etl = config.gold_etl.as_ref()
            .ok_or_else(|| GoldDdlError::MissingRequiredField { ... })?;

        if !gold_etl.enabled {
            return Err(GoldDdlError::GoldEtlDisabled { ... });
        }

        // Validate granularities parse correctly (generators need parsed values)
        if let Some(ref aggregates) = gold_etl.aggregates {
            for granularity in &aggregates.granularities {
                parse_granularity(granularity)?;
            }
        }

        // Feature window parsing (generators need parsed values)
        if let Some(ref features) = gold_etl.features {
            if let Some(ref rolling) = features.rolling {
                if rolling.enabled {
                    for window in &rolling.windows {
                        parse_window(window)?;
                    }
                }
            }
            if let Some(ref trend) = features.trend {
                if trend.enabled && !trend.window.is_empty() {
                    parse_window(&trend.window)?;
                }
            }
        }

        Ok(())
    }
}
```

The metric validation, field existence checks, and feature config completeness checks are removed from ConfigValidator because they are now handled by `validate::validate_gold_etl()` in the pre-flight validation step.

---

## 6. Standalone Binary Thin Wrappers (ops-003-12)

### 6.1 ndp-gold-ddl

`tools/ndp-gold-ddl/` should already be a thin wrapper after Phase 1. Verify that its `lib.rs` re-exports from `ndp_lib::gold` and its `main.rs` calls `ndp_lib::gold::*` functions. No Phase 3 changes needed unless Phase 1 did not complete this.

### 6.2 ndp-validate

`tools/ndp-validate/` should already be a thin wrapper after Phase 2. Same verification applies.

### 6.3 Verification

```bash
# Both should build cleanly
cargo build -p ndp-gold-ddl -p ndp-validate

# Both should produce identical output to ndp CLI
diff <(target/debug/ndp-gold-ddl generate --stream air-quality --config-dir config) \
     <(target/debug/ndp gold generate --stream air-quality --config-dir config/base)

diff <(target/debug/ndp-validate --all --config-dir config/base/streams --format json) \
     <(target/debug/ndp validate --all --config-dir config/base --format json)
```

---

## 7. Retire Stale YAML Configs (ops-003-13)

### 7.1 Scope

Rename any `config.yaml` files in `config/` subdirectories to `config.yaml.bak`. The platform has used JSON configs exclusively since v1.1.8. Any remaining YAML files are stale and could confuse agents or tools that discover configs by extension.

### 7.2 Discovery

```bash
find config/ -name "config.yaml" -o -name "config.yml" | sort
```

### 7.3 Exclusion

`config/platform.yaml` is NOT renamed. It is actively used for platform-level configuration (not stream/domain config).

### 7.4 Verification

After renaming, verify:
- `ndp validate --all` still passes (should not be affected -- it looks for `config.json`)
- `ndp gold generate --stream air-quality` still works
- No Rust code references `.yaml` stream config paths (already stripped in Phase 2)

---

## 8. Dependency Analysis

### 8.1 New Dependencies

None. Phase 3 adds no new crate dependencies. All constants extraction and cross-cutting wiring use existing types (`serde_json::Value`, `ValidationError`).

### 8.2 Compile Time Impact

| Change | Impact |
|---|---|
| New `constants.rs` file | Negligible -- 6 const definitions, no derive macros |
| Cross-cutting validation in gold::sync | Marginal -- `serde_json::to_value()` call adds serialization at sync time |
| Removing 84 lines of NoOpDbClient from CLI | Minor improvement -- 3 fewer `async_trait` expansions |

### 8.3 Binary Size Impact

| Change | Impact |
|---|---|
| Remove 3 NoOpDbClient impls | -~2KB (trivial) |
| Add serde_json::to_value call in sync | ~0 (serde_json already linked) |
| Net | Slight decrease |

### 8.4 Runtime Performance Impact

The cross-cutting validation adds `serde_json::to_value(&stream_config)` before each sync operation. For a typical stream config (~2KB JSON), this takes microseconds. The validation itself iterates over fields and metrics -- also negligible relative to the database operations that follow.

The `--no-validate` flag provides an escape hatch for performance-critical pipelines.

---

## 9. Risk Assessment

### 9.1 Risk Registry

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Constants extraction breaks import paths | Low | Medium | Backward-compatible re-exports in `gold::generators::constants`. Compiler catches all missing imports. |
| VALID_STATS/VALID_ROLLING_STATS merge introduces subtle behavior change | Low | Low | Values are identical (`["mean", "std", "min", "max"]`). Rename-only change. |
| Cross-cutting validation rejects configs that previously worked | Medium | High | validate_gold_etl already runs during `ndp validate --all` -- configs that pass standalone validation will pass cross-cutting validation. The only new rejections come from gaps C-F (empty lag hours, etc.) which are genuine config errors. |
| `serde_json::to_value(&stream_config)` loses data or produces different JSON than original config file | Low | High | Gold StreamConfig is `#[derive(Serialize)]` -- round-trip is safe. Test by comparing serialized output to original file. |
| Removing ConfigValidator checks that validate_gold_etl doesn't cover | Medium | High | Gap analysis in Section 5.3 identifies all gaps. Gaps C-F are added to validate_gold_etl before removing from ConfigValidator. |
| NoOpDbClient behavioral change (unreachable! -> Ok) causes silent failures | Low | Medium | The Ok behavior is strictly safer. If a dry-run code path accidentally queries the DB, returning empty results is better than crashing. No sync function should change behavior based on empty query results during dry-run. |
| Granularity regex update (adding "week") changes validation behavior | Low | Low | "1 week" was previously rejected by validate but accepted by gold generators. Making them consistent is a fix. No configs currently use weekly granularity. |

### 9.2 Rollback Strategy

Phase 3 contains no deploy.sh changes. If any issue is discovered:

1. **Constants extraction**: Revert `constants.rs`, restore local constants. No runtime behavior change.
2. **Cross-cutting validation**: Set `validate: false` in SyncOptions construction (or revert the validate field). Gold sync returns to pre-Phase 3 behavior instantly.
3. **NoOpDbClient consolidation**: Restore local copies. No runtime behavior change.
4. **ConfigValidator simplification**: Restore full ConfigValidator. No runtime behavior change.

All rollbacks are source-level. No database migrations, no config file changes, no deploy.sh modifications.

---

## 10. Migration Sequence

### Step 1: Create constants.rs

1. Create `crates/ndp-lib/src/constants.rs` with all 6 constants.
2. Add `pub mod constants;` to `crates/ndp-lib/src/lib.rs`.
3. `cargo check -p ndp-lib` -- should pass (new module, no consumers yet).

### Step 2: Migrate Constants Consumers (gold module)

1. Update `gold/config/types.rs`:
   - Remove `VALID_METRICS` and `VALID_ROLLING_STATS` const definitions (lines 11-16).
   - Add `use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};` for any local usage. (types.rs does not use them locally -- they are only re-exported via mod.rs.)

2. Update `gold/config/mod.rs`:
   - Remove `VALID_METRICS` and `VALID_ROLLING_STATS` from the `pub use types::{...}` re-export list (line 17).

3. Update `gold/generators/constants.rs`:
   - Replace local const definitions with re-exports:
     ```rust
     pub use crate::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA};
     ```

4. Update `gold/generators/continuous_aggregate.rs`:
   - Change `use crate::gold::config::{..., VALID_METRICS}` to import VALID_METRICS from `crate::constants` instead.

5. Update `gold/generators/events.rs`, `state_transitions.rs`, `aligned_view.rs`:
   - Change `use super::constants::{GOLD_SCHEMA, ...}` to `use crate::constants::{GOLD_SCHEMA, ...}`.

6. Update `gold/validation/config_validator.rs`:
   - Change `use crate::gold::config::{..., VALID_METRICS, VALID_ROLLING_STATS}` to import from `crate::constants`.

7. `cargo check -p ndp-lib` after each file. All gold tests should still pass.

### Step 3: Migrate Constants Consumers (validate module)

1. Update `validate/semantic/gold.rs`:
   - Remove local `const VALID_METRICS` definition (line 21).
   - Remove local `const VALID_STATS` definition (line 26).
   - Add `use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};`.
   - Replace all `VALID_STATS` references with `VALID_ROLLING_STATS`.

2. Fix granularity regex in `validate/semantic/mod.rs`:
   - Change regex from `r"^\d+\s+(minute|hour|day)s?$"` to `r"^\d+\s+(minute|hour|day|week)s?$"`.

3. `cargo test -p ndp-lib -- validate` -- all validate tests should pass.

### Step 4: Add validate field to SyncOptions

1. Update `crates/ndp-lib/src/types.rs`:
   - Remove `#[derive(Default)]` from `SyncOptions`.
   - Add `pub validate: bool` field.
   - Add manual `impl Default`.

2. Update all SyncOptions construction sites (see Section 3.2).

3. `cargo check --workspace` -- compiler tells you every missed site.

### Step 5: Wire Cross-cutting Validation

1. Update `gold::sync_stream()` in `crates/ndp-lib/src/gold/mod.rs`:
   - Add validation block per Section 3.3.

2. Update `gold::sync_domain()` in `crates/ndp-lib/src/gold/mod.rs`:
   - Add validation block per Section 3.4.

3. Update `commands/gold.rs` in ndp-cli:
   - Wire `no_validate` from clap to `SyncOptions { validate: !no_validate }`.

4. `cargo test -p ndp-lib` -- verify existing gold tests pass (they use SyncOptions with validate: true by default).

### Step 6: Add Missing Checks to validate_gold_etl()

1. In `validate/semantic/gold.rs`, add to `validate_features()`:
   - Check lag.lags_hours non-empty when enabled (Gap C)
   - Check lag.lags_hours >= 1 (Gap D)
   - Check rolling.windows non-empty when enabled (Gap E)
   - Check trend.window non-empty when enabled (Gap F)

2. Add tests for each new check.

3. `cargo test -p ndp-lib -- validate::semantic::gold` -- verify new tests pass.

### Step 7: Simplify ConfigValidator

1. Reduce `gold/validation/config_validator.rs` per Section 5.6.
2. Remove `validate_gold_config()` wrapper function.
3. Update `gold/mod.rs` re-export: remove `validate_gold_config` from `pub use validation::`.
4. Update `gold::mod.rs` re-exports for ConfigValidator.
5. Update any callers of `validate_gold_config()`:
   - `commands/gold.rs:207`: `ndp_lib::gold::validation::ConfigValidator::new().validate(&config)?;` -- keep this call, it uses the simplified ConfigValidator.

6. `cargo test -p ndp-lib` -- all tests pass.

### Step 8: Consolidate NoOpDbClient

1. In `tools/ndp-cli/src/commands/dictionary.rs`:
   - Delete lines 110-139 (NoOpDbClient definition).
   - Delete `use async_trait::async_trait;` if unused.
   - Change `&NoOpDbClient` to `&ndp_lib::NoOpDbClient` at line 68.

2. Same for `dimension.rs` (lines 173-199, usage at line 124).

3. Same for `domain.rs` (lines 141-167, usage at line 85).

4. `cargo build -p ndp-cli` -- verify build succeeds.

### Step 9: Retire Stale YAML Configs

1. Find and rename:
   ```bash
   find config/ -name "config.yaml" -o -name "config.yml" | while read f; do
       mv "$f" "${f}.bak"
   done
   ```

2. Do NOT rename `config/platform.yaml`.

3. Verify `ndp validate --all` and `ndp gold generate` still work.

### Step 10: Verify Standalone Wrappers

1. `cargo build -p ndp-gold-ddl -p ndp-validate`
2. Run parity checks per Section 6.3.

### Step 11: Full Test Suite

```bash
cargo test --workspace
```

All 740+ tests must pass.

---

## 11. Test Strategy

### 11.1 New Tests (Phase 3)

| Test | Location | Description |
|---|---|---|
| `test_sync_stream_validates_by_default` | `gold/mod.rs` or integration test | sync_stream with invalid metrics returns validation error |
| `test_sync_stream_no_validate_skips` | `gold/mod.rs` or integration test | sync_stream with validate: false proceeds despite invalid config |
| `test_sync_domain_validates_by_default` | `gold/mod.rs` or integration test | sync_domain with invalid domain config returns validation error |
| `test_constants_valid_metrics_complete` | `constants.rs` | VALID_METRICS contains exactly 9 expected values |
| `test_constants_valid_rolling_stats_subset` | `constants.rs` | VALID_ROLLING_STATS is a subset of VALID_METRICS |
| `test_lag_empty_hours_fails` | `validate/semantic/gold.rs` | Lag with enabled=true and empty lags_hours fails (Gap C) |
| `test_lag_zero_hours_fails` | `validate/semantic/gold.rs` | Lag with hours < 1 fails (Gap D) |
| `test_rolling_empty_windows_fails` | `validate/semantic/gold.rs` | Rolling with enabled=true and empty windows fails (Gap E) |
| `test_trend_empty_window_fails` | `validate/semantic/gold.rs` | Trend with enabled=true and empty window fails (Gap F) |
| `test_weekly_granularity_valid` | `validate/semantic/gold.rs` | "1 week" is accepted as valid granularity |
| `test_noop_dbclient_from_lib` | `tools/ndp-cli` integration | ndp_lib::NoOpDbClient used in CLI dry-run path |

### 11.2 Existing Tests (Must Continue Passing)

| Test Suite | Count | Notes |
|---|---|---|
| `ndp-lib::gold` | ~376 | Constants import paths change; tests themselves unchanged |
| `ndp-lib::validate` | ~217 | VALID_STATS renamed to VALID_ROLLING_STATS in test assertions |
| `ndp-lib::dictionary` | ~20 | Unaffected (SyncOptions gains validate field but dictionary doesn't use it) |
| `ndp-lib::dimension` | ~20 | Same |
| `ndp-lib::domain` | ~20 | Same |
| `ndp-gold-ddl` | ~36 | Integration tests; import paths may change |
| `ndp-cli` | ~50 | NoOpDbClient cleanup; SyncOptions construction |

---

## 12. Summary of Deliverables

| Deliverable | Description |
|---|---|
| `crates/ndp-lib/src/constants.rs` | New file: 6 platform-wide constants |
| `crates/ndp-lib/src/lib.rs` | Add `pub mod constants;` |
| `crates/ndp-lib/src/types.rs` | Add `validate: bool` to SyncOptions, manual Default impl |
| `crates/ndp-lib/src/gold/mod.rs` | Add validation blocks to sync_stream(), sync_domain() |
| `crates/ndp-lib/src/gold/config/types.rs` | Remove VALID_METRICS, VALID_ROLLING_STATS definitions |
| `crates/ndp-lib/src/gold/config/mod.rs` | Remove constants from re-export list |
| `crates/ndp-lib/src/gold/generators/constants.rs` | Replace definitions with re-exports from crate::constants |
| `crates/ndp-lib/src/gold/generators/*.rs` | Update import paths (4 files) |
| `crates/ndp-lib/src/gold/validation/config_validator.rs` | Simplify: keep parse functions, remove duplicate validation |
| `crates/ndp-lib/src/validate/semantic/gold.rs` | Import from crate::constants, add Gap C-F checks |
| `crates/ndp-lib/src/validate/semantic/mod.rs` | Fix granularity regex to accept "week" |
| `tools/ndp-cli/src/commands/dictionary.rs` | Remove local NoOpDbClient, use ndp_lib::NoOpDbClient |
| `tools/ndp-cli/src/commands/dimension.rs` | Remove local NoOpDbClient, use ndp_lib::NoOpDbClient |
| `tools/ndp-cli/src/commands/domain.rs` | Remove local NoOpDbClient, use ndp_lib::NoOpDbClient |
| `tools/ndp-cli/src/commands/gold.rs` | Wire no_validate to SyncOptions.validate |
| `config/**/*.yaml` (stale) | Renamed to `.yaml.bak` (except platform.yaml) |
| 11 new tests | Cross-cutting validation, constants, gap checks |
| 740+ existing tests passing | `cargo test --workspace` |
