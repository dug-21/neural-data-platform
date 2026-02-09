# OPS-003 Phase 3 Specification: Shared Constants + Cross-cutting Validation

> **Feature:** ops-003 Phase 3
> **Release:** v1.1.18 (tentative)
> **Created:** 2026-02-08
> **Status:** Specification
> **Specification Agent:** ndp-architect
> **AgentDB Patterns Used:** ID 1 (development:crate-module-migration), ID 4 (procedure:crate-validate-migration)

---

## 1. Problem Statement

### 1.1 What Remains After Phase 2

Phase 1 (v1.1.14) migrated Gold DDL generation into `ndp_lib::gold`. Phase 2 (v1.1.17) migrated config validation into `ndp_lib::validate`. Both modules now live as siblings in `ndp-lib`, but they were extracted from separate codebases without cross-module wiring. Six categories of duplication and fragmentation remain:

| Category | Where | Impact |
|----------|-------|--------|
| **Duplicate VALID_METRICS** | `gold::config::types` (line 11) AND `validate::semantic::gold` (line 21) | If a new metric is added to one but not the other, validation diverges from generation |
| **Duplicate VALID_ROLLING_STATS / VALID_STATS** | `gold::config::types` (line 16) AND `validate::semantic::gold` (line 26, named `VALID_STATS`) | Same divergence risk; additionally the names differ (`VALID_ROLLING_STATS` vs `VALID_STATS`) |
| **GOLD_SCHEMA / SILVER_SCHEMA / NDP_ENTITY_COLUMN buried in generators** | `gold::generators::constants` (lines 7-13) | Not accessible to validate or other future modules; only used within generators |
| **Duplicate Gold config validation** | `gold::validation::ConfigValidator` (struct) AND `validate::semantic::gold::validate_gold_etl()` (function) | Two different validation pipelines for Gold config; ConfigValidator operates on typed structs, validate_gold_etl operates on JSON |
| **3 copies of NoOpDbClient** | `ndp-cli/commands/domain.rs` (line 146), `dictionary.rs` (line 116), `dimension.rs` (line 178) | All have `unreachable!()` for all methods; ndp-lib already has a proper `NoOpDbClient` at `db.rs` line 94 that returns `Ok(())` |
| **No cross-cutting validation** | `gold::sync_stream()` does NOT call `validate::gold_config()` before generating DDL | Invalid config can reach DDL generation, producing broken SQL |

### 1.2 Why It Matters

1. **Constant drift.** Two definitions of `VALID_METRICS` means a metric added to one will silently be missing from the other. The Gold generator will produce DDL for a metric that the validator rejects (or vice versa). This is a latent bug.

2. **Validation gap.** `ndp gold sync --stream X` generates DDL without any semantic validation. If the config has an invalid field reference, the DDL generator produces SQL referencing a nonexistent column. The error surfaces at `psql` execution time, not at validation time. Cross-cutting validation (calling `validate::gold_config()` before `generate()`) closes this gap.

3. **Three NoOpDbClient copies.** Each copy has `unreachable!()` implementations that will panic if called. The canonical `ndp_lib::NoOpDbClient` returns `Ok(())`, which is safer. The CLI commands should use the library version.

4. **Stale YAML configs.** Seven `config.yaml` files exist under `config/base/streams/` alongside their `config.json` replacements. All configs have been JSON since v1.1.8. The YAML files confuse agents and can cause incorrect file discovery if any code falls back to `.yaml`.

### 1.3 What Changes

Phase 3 is **internal consolidation only**. No deploy.sh changes. No new CLI flags. No public API additions. Existing behavior is preserved with one enhancement: `gold::sync_stream()` now validates config by default.

### 1.4 What Does NOT Change

- Gold DDL generation logic (same SQL output for same input)
- Validation logic (same errors/warnings for same input)
- deploy.sh (0 dispatch site changes)
- CLI commands (existing `--no-validate` flag already captured, now wired)
- `ndp-gold-ddl` standalone behavior (already a thin wrapper)
- `ndp-validate` standalone behavior (already a thin wrapper)
- Test assertions (same expected values)

---

## 2. Requirements

### 2.1 Functional Requirements

#### FR-01: Shared Constants Module (`ndp_lib::constants`)

Create a new module `crates/ndp-lib/src/constants.rs` containing all constants shared across `gold` and `validate` (and future modules).

**Constants to extract:**

| Constant | Current Location | Current Name | Target Name |
|----------|-----------------|--------------|-------------|
| Valid aggregate metrics | `gold::config::types` line 11 | `VALID_METRICS` | `VALID_METRICS` |
| Valid aggregate metrics | `validate::semantic::gold` line 21 | `VALID_METRICS` | (removed -- uses constants module) |
| Valid rolling stats | `gold::config::types` line 16 | `VALID_ROLLING_STATS` | `VALID_ROLLING_STATS` |
| Valid rolling stats | `validate::semantic::gold` line 26 | `VALID_STATS` | (removed -- uses constants module) |
| Gold schema name | `gold::generators::constants` line 10 | `GOLD_SCHEMA` | `GOLD_SCHEMA` |
| Silver schema name | `gold::generators::constants` line 13 | `SILVER_SCHEMA` | `SILVER_SCHEMA` |
| Entity column name | `gold::generators::constants` line 7 | `NDP_ENTITY_COLUMN` | `NDP_ENTITY_COLUMN` |

**Values (canonical, from `gold::config::types`):**

```rust
// crates/ndp-lib/src/constants.rs

/// Valid aggregate metrics for Gold layer continuous aggregates.
///
/// Used by both `gold::validation::ConfigValidator` (typed struct validation)
/// and `validate::semantic::gold` (JSON-level validation).
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];

/// Valid statistics for rolling window features.
///
/// A subset of VALID_METRICS applicable to rolling windows.
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

/// Gold schema name. All Gold layer objects are created in this schema.
pub const GOLD_SCHEMA: &str = "gold";

/// Silver schema name. All Silver layer tables live here.
pub const SILVER_SCHEMA: &str = "silver";

/// Default entity identifier column used across NDP streams.
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";
```

**Import path changes after extraction:**

| Consumer | Old Import | New Import |
|----------|-----------|------------|
| `gold::config::types` | Local `const VALID_METRICS` | `use crate::constants::VALID_METRICS;` |
| `gold::config::types` | Local `const VALID_ROLLING_STATS` | `use crate::constants::VALID_ROLLING_STATS;` |
| `gold::config::mod` | `use types::{... VALID_METRICS, VALID_ROLLING_STATS}` | `pub use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};` (re-export for backward compat) |
| `gold::validation::config_validator` | `use crate::gold::config::{... VALID_METRICS, VALID_ROLLING_STATS}` | Unchanged (re-export chain) |
| `gold::generators::continuous_aggregate` | `use crate::gold::config::{... VALID_METRICS}` | Unchanged (re-export chain) |
| `gold::generators::constants` | Local `const GOLD_SCHEMA` etc. | `pub use crate::constants::{GOLD_SCHEMA, SILVER_SCHEMA, NDP_ENTITY_COLUMN};` |
| `gold::generators::events` | `use super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA}` | Unchanged (re-export chain) |
| `gold::generators::state_transitions` | `use super::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN}` | Unchanged (re-export chain) |
| `gold::generators::aligned_view` | `use crate::gold::generators::constants::GOLD_SCHEMA` | Unchanged (re-export chain) |
| `validate::semantic::gold` | Local `const VALID_METRICS` (line 21) | `use crate::constants::VALID_METRICS;` |
| `validate::semantic::gold` | Local `const VALID_STATS` (line 26) | `use crate::constants::VALID_ROLLING_STATS;` (rename) |

**Backward compatibility strategy:** The `gold::config::mod` module currently re-exports `VALID_METRICS` and `VALID_ROLLING_STATS` from `types`. After extraction, `gold::config::mod` will re-export from `crate::constants` instead. External consumers (like `gold::validation::config_validator`) use `crate::gold::config::{VALID_METRICS, VALID_ROLLING_STATS}` which continues to work through the re-export. Similarly, `gold::generators::constants` becomes a re-export module. No downstream import changes are required.

**Name harmonization:** `validate::semantic::gold` uses `VALID_STATS` (line 26) where `gold::config::types` uses `VALID_ROLLING_STATS` (line 16). Both contain `["mean", "std", "min", "max"]`. The canonical name is `VALID_ROLLING_STATS`. All references to `VALID_STATS` in `validate::semantic::gold` must be renamed to `VALID_ROLLING_STATS`.

#### FR-02: Cross-cutting Validation (`gold::sync` calls `validate::gold_config`)

Wire `ndp_lib::gold::sync_stream()` to call `ndp_lib::validate::semantic::validate_gold_etl()` before generating DDL. The `--no-validate` flag (already parsed in ndp-cli `GoldCommands::Sync`) controls this behavior.

**Changes to `SyncOptions`:**

```rust
// crates/ndp-lib/src/types.rs
pub struct SyncOptions {
    /// If true, generate SQL but do not execute against the database.
    pub dry_run: bool,
    /// If true, skip config validation before sync. Default: false.
    pub validate: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            validate: true,  // Validate by default
        }
    }
}
```

**Changes to `gold::sync_stream()`:**

```rust
// crates/ndp-lib/src/gold/mod.rs
pub async fn sync_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    checker: &(impl CaChecker + Send + Sync),
    opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
    let gold_etl = stream_config
        .gold_etl
        .as_ref()
        .ok_or_else(|| format!("Stream '{}' has no gold_etl configuration", stream_id))?;

    if !gold_etl.enabled {
        return Err(format!("Stream '{}' has gold_etl.enabled = false", stream_id).into());
    }

    // Cross-cutting validation: validate config before generating DDL
    if opts.validate {
        // Load the raw JSON for semantic validation
        let config_path = loader.stream_config_path(stream_id)?;
        let content = std::fs::read_to_string(&config_path)?;
        let json_value: serde_json::Value = serde_json::from_str(&content)?;
        let errors = crate::validate::semantic::validate_gold_etl(&json_value);
        if !errors.is_empty() {
            let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(format!(
                "Gold config validation failed for stream '{}': {}",
                stream_id,
                error_msgs.join("; ")
            ).into());
        }
    }

    let planner = SyncPlanner::new(checker, &stream_config);
    let plan = planner.plan(gold_etl).await?;

    Ok(plan.to_ddl())
}
```

**Note:** The `validate_gold_etl()` function operates on `serde_json::Value`, while `sync_stream()` works with `gold::config::StreamConfig` (a typed struct). The cross-cutting validation must load the raw JSON to call the semantic validator. This requires adding a `stream_config_path()` method to `ConfigLoader` (or reading the file that was already loaded). Since `FileSystemConfigLoader` already knows the path, this is a small addition.

**Alternative approach (simpler):** Instead of re-reading the file, call `gold::validation::validate_gold_config()` which operates on the typed `StreamConfig` struct. This uses the `ConfigValidator` that Phase 3 plans to unify (FR-03). The typed validator catches field-not-found, invalid-metric, invalid-granularity, and invalid-rolling-stat errors -- the same categories as the semantic validator. The advantage is no re-reading of files.

```rust
// Simpler approach: use typed ConfigValidator
if opts.validate {
    crate::gold::validation::validate_gold_config(&stream_config)?;
}
```

**Decision:** Use the simpler approach (typed ConfigValidator) for cross-cutting validation in sync. The ConfigValidator already exists, validates the same concerns, and operates on the struct that sync already has. The semantic validator (`validate::semantic::validate_gold_etl()`) remains for JSON-level validation in the `ndp validate` pipeline. Unification of these two is addressed in FR-03 below but is optional -- both can coexist serving different consumers.

**CLI wiring (`--no-validate`):**

The `GoldCommands::Sync` variant already captures `no_validate: bool` (line 74 of `gold.rs`). Currently it is discarded with `no_validate: _` (line 138). Phase 3 wires it:

```rust
// tools/ndp-cli/src/commands/gold.rs, in run_sync()
let opts = ndp_lib::types::SyncOptions {
    dry_run,
    validate: !no_validate,
};
```

Similarly for `GoldCommands::Generate` (line 47, `no_validate`) and `GoldCommands::Recreate` (line 93, `no_validate`).

#### FR-03: Gold Validation Unification

**Current state:** Two separate validation implementations exist:

1. `gold::validation::ConfigValidator` - Validates typed `gold::config::StreamConfig`. Used by `ndp gold generate --validate-only`.
2. `validate::semantic::gold::validate_gold_etl()` - Validates raw `serde_json::Value`. Used by `ndp validate --stream`.

Both check the same concerns (field references, metric validity, granularity format, rolling stats) but against different data representations. They can diverge.

**Decision: Keep both, ensure they use shared constants.**

Full unification would require one of:
- Converting the JSON validator to use typed structs (breaks the schema/semantic validation pipeline)
- Converting the typed validator to use JSON (loses type safety for Gold DDL generation)

Neither is worthwhile. Instead, Phase 3 ensures both validators use `crate::constants::VALID_METRICS` and `crate::constants::VALID_ROLLING_STATS`. This guarantees the same valid values regardless of which validation path runs. The structural logic (checking field existence, granularity format) is inherently different between JSON and typed struct approaches.

**FR-03a: Remove ConfigValidator re-export from gold::mod.rs?**

No. `ConfigValidator` is a public type used by ndp-gold-ddl standalone (`main.rs` line 207). It stays exported. The unification scope is narrowed to: both validators use shared constants.

#### FR-04: NoOpDbClient Deduplication

**Current state:** Four `NoOpDbClient` definitions exist:

| Location | Behavior | Used By |
|----------|----------|---------|
| `crates/ndp-lib/src/db.rs` line 94 | Returns `Ok(vec![])` / `Ok(0)` / `Ok(())` | Not currently used by CLI commands |
| `tools/ndp-cli/src/commands/domain.rs` line 146 | `unreachable!()` for all methods | `domain sync --dry-run` |
| `tools/ndp-cli/src/commands/dictionary.rs` line 116 | `unreachable!()` for all methods | `dictionary sync --dry-run` |
| `tools/ndp-cli/src/commands/dimension.rs` line 178 | `unreachable!()` for all methods | `dimension sync --dry-run` |

**Decision:** The three CLI copies are replaced with `ndp_lib::NoOpDbClient`. The library version returns `Ok(...)` instead of panicking, which is strictly safer.

**Behavioral difference:** The CLI copies use `unreachable!()` because dry-run mode should never call the database. The library version returns empty results. In practice, this difference is immaterial -- dry-run code paths do not call `query()`, `execute()`, or `batch_execute()`. If a bug causes a dry-run path to call the DB, the library version silently succeeds (producing incorrect output), while the CLI version panics. The silent success is preferred because:
1. The DDL output is the important artifact (printed to stdout)
2. DB calls during dry-run are already guarded by sync logic
3. Panics in production deployment are worse than empty DB results

**Changes per file:**

- `domain.rs`: Remove lines 140-169 (NoOpDbClient struct + impl). Change line 85 to `use ndp_lib::NoOpDbClient;`.
- `dictionary.rs`: Remove lines 111-137 (NoOpDbClient struct + impl). Change line 68 to `use ndp_lib::NoOpDbClient;`.
- `dimension.rs`: Remove lines 173-199 (NoOpDbClient struct + impl). Change line 124 to `use ndp_lib::NoOpDbClient;`.
- Remove `use async_trait::async_trait;` from each file if it becomes unused.

#### FR-05: Standalone Binary Thin Wrappers

**ndp-gold-ddl:** Already a thin wrapper (Phase 1 made it so). `lib.rs` re-exports from `ndp_lib::gold`. `main.rs` still uses the re-exported types to call generation directly. No changes needed.

**ndp-validate:** Already a thin wrapper (Phase 2 made it so). `lib.rs` re-exports from `ndp_lib::validate`. No changes needed.

This scope item is complete from Phase 1 and Phase 2. No Phase 3 work required.

#### FR-06: Retire Stale YAML Stream Configs

**Scope:** Rename `config.yaml` files under `config/base/streams/` to `config.yaml.bak`. These are legacy YAML configs that were replaced by `config.json` in v1.1.8 (FE-002 Domain Config Standardization). All code paths now use JSON.

**Files to rename:**

| Current Path | New Path |
|-------------|----------|
| `config/base/streams/air-quality/config.yaml` | `config/base/streams/air-quality/config.yaml.bak` |
| `config/base/streams/home-assistant-state/config.yaml` | `config/base/streams/home-assistant-state/config.yaml.bak` |
| `config/base/streams/outdoor-weather/config.yaml` | `config/base/streams/outdoor-weather/config.yaml.bak` |
| `config/base/streams/outdoor-air-quality/config.yaml` | `config/base/streams/outdoor-air-quality/config.yaml.bak` |
| `config/base/streams/nws-forecast-hourly/config.yaml` | `config/base/streams/nws-forecast-hourly/config.yaml.bak` |
| `config/base/streams/nws-observations/config.yaml` | `config/base/streams/nws-observations/config.yaml.bak` |
| `config/base/streams/nws-gridpoints-forecast/config.yaml` | `config/base/streams/nws-gridpoints-forecast/config.yaml.bak` |

**Files NOT renamed (intentionally kept as .yaml):**

| Path | Reason |
|------|--------|
| `config/base/platform.yaml` | Platform config, not a stream config. Different format. |
| `config/base/processors/threshold-alerts.yaml` | Processor config, not a stream config. |
| `config/base/air-quality.yaml` | Legacy single-file config (pre-directory structure). Not active. |
| `config/base/air-quality/config.yaml` | Legacy nested config (pre-v1.1.8). Not a stream directory. |
| `config/overlays/*/config.yaml` | Overlay configs. Different lifecycle. |
| `config/samples/*.yaml` | Sample configs for documentation. |
| `config/schemas/homeassistant/config.yaml` | Home Assistant schema definition. |
| `config/grafana/**/*.yaml` | Grafana provisioning. Different system entirely. |
| `config/integration/base/streams/*/config.yaml` | Integration environment stream configs. These ARE stale YAML that should be renamed too. |

**Integration environment YAML configs to rename:**

| Current Path | New Path |
|-------------|----------|
| `config/integration/base/streams/outdoor-weather/config.yaml` | `config/integration/base/streams/outdoor-weather/config.yaml.bak` |
| `config/integration/base/streams/home-assistant-state/config.yaml` | `config/integration/base/streams/home-assistant-state/config.yaml.bak` |
| `config/integration/base/streams/outdoor-air-quality/config.yaml` | `config/integration/base/streams/outdoor-air-quality/config.yaml.bak` |

**Total: 10 files renamed** (7 base + 3 integration).

**Verification:** After renaming, confirm no `.yaml` stream config files remain:
```bash
find config/base/streams -name "config.yaml" -type f   # Should return 0 results
find config/integration/base/streams -name "config.yaml" -type f  # Should return 0 results
```

---

## 3. Module Structure Changes

### 3.1 New File

```
crates/ndp-lib/src/constants.rs     Shared constants (VALID_METRICS, VALID_ROLLING_STATS, GOLD_SCHEMA, SILVER_SCHEMA, NDP_ENTITY_COLUMN)
```

### 3.2 Modified Files

| File | Change |
|------|--------|
| `crates/ndp-lib/src/lib.rs` | Add `pub mod constants;` |
| `crates/ndp-lib/src/types.rs` | Add `validate: bool` field to `SyncOptions` |
| `crates/ndp-lib/src/gold/config/types.rs` | Remove local `VALID_METRICS` and `VALID_ROLLING_STATS` constants; import from `crate::constants` |
| `crates/ndp-lib/src/gold/config/mod.rs` | Re-export constants from `crate::constants` instead of from `types` |
| `crates/ndp-lib/src/gold/generators/constants.rs` | Remove local constants; re-export from `crate::constants` |
| `crates/ndp-lib/src/gold/mod.rs` | Add validation call in `sync_stream()` |
| `crates/ndp-lib/src/validate/semantic/gold.rs` | Remove local `VALID_METRICS` and `VALID_STATS`; import from `crate::constants` |
| `tools/ndp-cli/src/commands/gold.rs` | Wire `no_validate` to `SyncOptions.validate` |
| `tools/ndp-cli/src/commands/domain.rs` | Remove local NoOpDbClient; use `ndp_lib::NoOpDbClient` |
| `tools/ndp-cli/src/commands/dictionary.rs` | Remove local NoOpDbClient; use `ndp_lib::NoOpDbClient` |
| `tools/ndp-cli/src/commands/dimension.rs` | Remove local NoOpDbClient; use `ndp_lib::NoOpDbClient` |

### 3.3 Renamed Files (git mv)

10 YAML config files renamed to `.yaml.bak` (see FR-06 above).

### 3.4 Deleted Code (Not Files)

| Location | Lines | Content Removed |
|----------|-------|----------------|
| `gold::config::types` | 11-16 | 2 `const` declarations (moved to `constants.rs`) |
| `gold::generators::constants` | 7-13 | 3 `const` declarations (moved to `constants.rs`) |
| `validate::semantic::gold` | 21-26 | 2 `const` declarations (moved to `constants.rs`) |
| `ndp-cli commands/domain.rs` | 140-169 | Local `NoOpDbClient` struct + impl |
| `ndp-cli commands/dictionary.rs` | 111-137 | Local `NoOpDbClient` struct + impl |
| `ndp-cli commands/dimension.rs` | 173-199 | Local `NoOpDbClient` struct + impl |

---

## 4. Public API Changes

### 4.1 New Public API

```rust
// crates/ndp-lib/src/constants.rs
pub const VALID_METRICS: &[&str];
pub const VALID_ROLLING_STATS: &[&str];
pub const GOLD_SCHEMA: &str;
pub const SILVER_SCHEMA: &str;
pub const NDP_ENTITY_COLUMN: &str;
```

### 4.2 Changed Public API

```rust
// crates/ndp-lib/src/types.rs
pub struct SyncOptions {
    pub dry_run: bool,
    pub validate: bool,  // NEW FIELD
}
```

**Breaking change analysis:** Adding `validate: bool` to `SyncOptions` is a breaking change for code that constructs `SyncOptions` without `Default`. Currently, `SyncOptions` derives `Default`, and the only external consumers are:
- `ndp-cli/commands/gold.rs` line 239: `SyncOptions { dry_run }` -- BREAKS (missing `validate` field)
- `ndp-cli/commands/gold.rs` line 264: `SyncOptions { dry_run }` -- BREAKS
- `ndp-cli/commands/domain.rs` line 76: `SyncOptions { dry_run: false }` -- BREAKS
- `ndp-cli/commands/dictionary.rs` line 59: `SyncOptions { dry_run: false }` -- BREAKS
- `ndp-cli/commands/dimension.rs` line 115: `SyncOptions { dry_run: false }` -- BREAKS

All are within the workspace. Fix: add `validate: true` (or use `..Default::default()`) to each construction site.

### 4.3 Removed Public API

None. `VALID_METRICS` and `VALID_ROLLING_STATS` remain accessible from `gold::config` through re-exports. `GOLD_SCHEMA`, `SILVER_SCHEMA`, and `NDP_ENTITY_COLUMN` remain accessible from `gold::generators::constants` through re-exports.

---

## 5. Constants Inventory

### 5.1 VALID_METRICS Audit

| Location | Value | Status |
|----------|-------|--------|
| `gold::config::types` line 11 | `["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]` | **Source of truth** -- used by DDL generator |
| `validate::semantic::gold` line 21 | `["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]` | **Duplicate** -- same values, used by JSON validator |

**Values match.** Both have 9 identical metrics. Extract to `constants.rs`.

### 5.2 VALID_ROLLING_STATS Audit

| Location | Name | Value | Status |
|----------|------|-------|--------|
| `gold::config::types` line 16 | `VALID_ROLLING_STATS` | `["mean", "std", "min", "max"]` | **Source of truth** |
| `validate::semantic::gold` line 26 | `VALID_STATS` | `["mean", "std", "min", "max"]` | **Duplicate with different name** |

**Values match.** Canonical name: `VALID_ROLLING_STATS`. Rename `VALID_STATS` references in `validate::semantic::gold`.

### 5.3 Schema/Column Constants Audit

| Constant | Current Location | Used By |
|----------|-----------------|---------|
| `GOLD_SCHEMA = "gold"` | `gold::generators::constants` line 10 | aligned_view.rs, events.rs, state_transitions.rs, continuous_aggregate.rs (via generate_gold_table_sql) |
| `SILVER_SCHEMA = "silver"` | `gold::generators::constants` line 13 | events.rs |
| `NDP_ENTITY_COLUMN = "ndp_id"` | `gold::generators::constants` line 7 | events.rs, state_transitions.rs |

These are currently only used within `gold::generators::*`. Moving them to `crate::constants` makes them available to future modules (e.g., if `validate::semantic::domain` needs to validate Gold schema references).

---

## 6. NoOpDbClient Audit

### 6.1 All Definitions

| # | Location | Lines | Behavior | Dependencies |
|---|----------|-------|----------|-------------|
| 1 | `crates/ndp-lib/src/db.rs` | 94-109 | `Ok(vec![])`, `Ok(0)`, `Ok(())` | `async_trait`, `tokio_postgres::types::ToSql`, `tokio_postgres::Row` |
| 2 | `tools/ndp-cli/src/commands/domain.rs` | 146-169 | `unreachable!()` for all 3 methods | `async_trait`, `tokio_postgres::types::ToSql`, `tokio_postgres::Row` |
| 3 | `tools/ndp-cli/src/commands/dictionary.rs` | 116-137 | `unreachable!()` for all 3 methods | Same |
| 4 | `tools/ndp-cli/src/commands/dimension.rs` | 178-199 | `unreachable!()` for all 3 methods | Same |

### 6.2 Resolution

Copies 2, 3, 4 are deleted. All CLI commands use `ndp_lib::NoOpDbClient` (copy 1), which is already re-exported from `ndp_lib` root: `pub use db::{DbClient, NoOpDbClient};` (lib.rs line 36).

### 6.3 Impact on Dependencies

After removing the local `NoOpDbClient` definitions:
- `async_trait` import may become unused in each CLI command file (remove if so)
- `tokio_postgres::types::ToSql` and `tokio_postgres::Row` may become unused (check, remove if so)

---

## 7. YAML Config Audit

### 7.1 Stream Config Files

Every stream directory under `config/base/streams/` was checked:

| Stream | Has config.json? | Has config.yaml? | YAML Status |
|--------|-----------------|-------------------|-------------|
| air-quality | Yes | Yes | Stale -- rename to .bak |
| home-assistant-state | Yes | Yes | Stale -- rename to .bak |
| outdoor-weather | Yes | Yes | Stale -- rename to .bak |
| outdoor-air-quality | Yes | Yes | Stale -- rename to .bak |
| nws-forecast-hourly | Yes | Yes | Stale -- rename to .bak |
| nws-observations | Yes | Yes | Stale -- rename to .bak |
| nws-gridpoints-forecast | Yes | Yes | Stale -- rename to .bak |

### 7.2 Integration Stream Config Files

| Stream | Has config.json? | Has config.yaml? | YAML Status |
|--------|-----------------|-------------------|-------------|
| outdoor-weather | Needs check | Yes | Stale -- rename to .bak |
| home-assistant-state | Needs check | Yes | Stale -- rename to .bak |
| outdoor-air-quality | Needs check | Yes | Stale -- rename to .bak |

### 7.3 Code References to `.yaml`

The YAML code paths were stripped from `ndp-validate` during Phase 2. The only remaining `.yaml` reference in stream validation is in `validate::semantic::domain.rs` where stream config discovery tries `.json` then `.yaml` -- but this was removed during Phase 2 migration. Verify with:

```bash
grep -rn "config.yaml\|config.yml" crates/ndp-lib/src/  # Should return 0 results
```

---

## 8. Acceptance Criteria

### 8.1 FR-01: Shared Constants

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-01.1 | `ndp_lib::constants::VALID_METRICS` exists and contains 9 metrics | `cargo test -p ndp-lib -- constants` |
| AC-01.2 | `ndp_lib::constants::VALID_ROLLING_STATS` exists and contains 4 stats | `cargo test -p ndp-lib -- constants` |
| AC-01.3 | `ndp_lib::constants::GOLD_SCHEMA` equals `"gold"` | `cargo test -p ndp-lib -- constants` |
| AC-01.4 | `ndp_lib::constants::SILVER_SCHEMA` equals `"silver"` | `cargo test -p ndp-lib -- constants` |
| AC-01.5 | `ndp_lib::constants::NDP_ENTITY_COLUMN` equals `"ndp_id"` | `cargo test -p ndp-lib -- constants` |
| AC-01.6 | No local `VALID_METRICS` definition in `validate::semantic::gold` | `grep -n "const VALID_METRICS" crates/ndp-lib/src/validate/semantic/gold.rs` returns 0 |
| AC-01.7 | No local `VALID_STATS` definition in `validate::semantic::gold` | `grep -n "const VALID_STATS" crates/ndp-lib/src/validate/semantic/gold.rs` returns 0 |
| AC-01.8 | No local `VALID_METRICS` definition in `gold::config::types` | `grep -n "const VALID_METRICS" crates/ndp-lib/src/gold/config/types.rs` returns 0 |
| AC-01.9 | No local `VALID_ROLLING_STATS` definition in `gold::config::types` | `grep -n "const VALID_ROLLING_STATS" crates/ndp-lib/src/gold/config/types.rs` returns 0 |
| AC-01.10 | No local constants in `gold::generators::constants` | File only contains re-exports |
| AC-01.11 | `gold::config::VALID_METRICS` still works (re-export) | Existing gold tests pass |
| AC-01.12 | All 740+ workspace tests pass | `cargo test --workspace` |

### 8.2 FR-02: Cross-cutting Validation

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-02.1 | `SyncOptions` has `validate: bool` field, default `true` | `cargo test -p ndp-lib -- types` |
| AC-02.2 | `gold::sync_stream()` with valid config and `validate: true` produces DDL | Unit test with mock CaChecker |
| AC-02.3 | `gold::sync_stream()` with invalid config and `validate: true` returns error | Unit test with config missing required field |
| AC-02.4 | `gold::sync_stream()` with invalid config and `validate: false` produces DDL (bypasses validation) | Unit test |
| AC-02.5 | `ndp gold sync --no-validate` passes `validate: false` to SyncOptions | CLI test |
| AC-02.6 | `ndp gold sync` (without --no-validate) passes `validate: true` to SyncOptions | CLI test |
| AC-02.7 | `ndp gold generate --no-validate` skips validation in validate-only mode | CLI behavior test |
| AC-02.8 | `ndp gold recreate --no-validate` passes `validate: false` | CLI test |

### 8.3 FR-03: Gold Validation (Shared Constants)

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-03.1 | `gold::validation::ConfigValidator` uses `crate::constants::VALID_METRICS` | Grep verification |
| AC-03.2 | `gold::validation::ConfigValidator` uses `crate::constants::VALID_ROLLING_STATS` | Grep verification |
| AC-03.3 | `validate::semantic::gold` uses `crate::constants::VALID_METRICS` | Grep verification |
| AC-03.4 | `validate::semantic::gold` uses `crate::constants::VALID_ROLLING_STATS` (not VALID_STATS) | Grep verification |
| AC-03.5 | Both validators accept the same set of metrics | Test: pass each metric from VALID_METRICS to both validators |
| AC-03.6 | Both validators reject the same invalid metric | Test: pass "average" to both, both reject |

### 8.4 FR-04: NoOpDbClient Dedup

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-04.1 | Zero `struct NoOpDbClient` definitions in ndp-cli | `grep -rn "struct NoOpDbClient" tools/ndp-cli/src/` returns 0 |
| AC-04.2 | `ndp domain sync --dry-run` still works | Manual test |
| AC-04.3 | `ndp dictionary sync --dry-run` still works | Manual test |
| AC-04.4 | `ndp dimension sync --dry-run` still works | Manual test |
| AC-04.5 | CLI commands import `ndp_lib::NoOpDbClient` | Grep verification |

### 8.5 FR-05: Standalone Thin Wrappers

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-05.1 | `cargo build -p ndp-gold-ddl` succeeds | Build test |
| AC-05.2 | `cargo build -p ndp-validate` succeeds | Build test |

### 8.6 FR-06: Retire Stale YAML

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-06.1 | Zero `config.yaml` files under `config/base/streams/` | `find config/base/streams -name "config.yaml" -type f` returns empty |
| AC-06.2 | Zero `config.yaml` files under `config/integration/base/streams/` | `find config/integration/base/streams -name "config.yaml" -type f` returns empty |
| AC-06.3 | 7 `.yaml.bak` files exist under `config/base/streams/` | `find config/base/streams -name "config.yaml.bak" -type f` returns 7 |
| AC-06.4 | 3 `.yaml.bak` files exist under `config/integration/base/streams/` | `find config/integration/base/streams -name "config.yaml.bak" -type f` returns 3 |
| AC-06.5 | `platform.yaml` is NOT renamed | `ls config/base/platform.yaml` succeeds |
| AC-06.6 | No code references `config.yaml` for stream discovery | `grep -rn "config.yaml" crates/ndp-lib/src/` returns 0 |

### 8.7 Cross-cutting Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-X.1 | `cargo test --workspace` passes | 0 failures |
| AC-X.2 | `cargo clippy --workspace` clean | 0 warnings |
| AC-X.3 | No `TODO`, `unimplemented!()`, or `todo!()` in changed code | Grep verification |
| AC-X.4 | All `SyncOptions` construction sites include `validate` field | Grep verification |

---

## 9. Implementation Order

Phase 3 has no deploy.sh changes and no external API changes. The implementation order is:

1. **Create `constants.rs`** and register in `lib.rs`
2. **Update `gold::config::types`** -- remove local constants, import from `crate::constants`
3. **Update `gold::config::mod`** -- re-export from `crate::constants`
4. **Update `gold::generators::constants`** -- remove local constants, re-export from `crate::constants`
5. **Update `validate::semantic::gold`** -- remove local constants, import from `crate::constants`, rename `VALID_STATS` to `VALID_ROLLING_STATS`
6. **Run `cargo test --workspace`** -- all tests should pass (constants are the same values)
7. **Add `validate` field to `SyncOptions`** -- update all construction sites
8. **Wire cross-cutting validation** in `gold::sync_stream()`, `generate_stream()`, `recreate_stream()`
9. **Wire `--no-validate`** in ndp-cli gold commands
10. **Remove NoOpDbClient** copies from ndp-cli commands
11. **Rename YAML configs** to `.yaml.bak`
12. **Run `cargo test --workspace`** -- final verification

---

## 10. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Re-export chain breaks downstream imports | Low | Medium | Re-exports from `gold::config::mod` and `gold::generators::constants` preserve all existing import paths |
| `SyncOptions { dry_run }` without `validate` causes compile error | Certain | Low | All 5 construction sites are in-workspace; fix immediately |
| Cross-cutting validation rejects previously accepted configs | Medium | Medium | Configs that pass `ndp validate --stream` today are correct; only configs that bypass `ndp validate` (direct `ndp gold sync`) could be affected. Integration test catches this. |
| `VALID_STATS` rename breaks test assertions | Low | Low | Tests reference the constant name, not the variable; `VALID_ROLLING_STATS` is the canonical name |
| YAML rename breaks some undiscovered code path | Low | Low | All YAML paths were stripped in Phase 2; verify with grep |
| NoOpDbClient behavioral difference (Ok vs unreachable) causes silent incorrect output | Very Low | Medium | Dry-run paths do not call DB methods; guard exists in sync logic |

---

## Appendix A: Dependency Graph After Phase 3

No new dependencies. No removed dependencies. The only structural change is `constants.rs` being added to `ndp-lib`.

```
ndp-types (no workspace deps)
    |
    v
ndp-lib (depends on ndp-types)
    |-- constants.rs   (NEW: shared constants)
    |-- db.rs          (NoOpDbClient: canonical)
    |-- types.rs       (SyncOptions: +validate field)
    |-- gold/          (uses crate::constants, cross-cutting validation)
    |-- validate/      (uses crate::constants)
    |
    +---> ndp-cli      (uses ndp_lib::NoOpDbClient, wires --no-validate)
    +---> ndp-gold-ddl (thin wrapper, unchanged)
    +---> ndp-validate  (thin wrapper, unchanged)
```
