# OPS-003 Phase 3 (v1.1.16) Pseudocode: Shared Constants + Cross-cutting

> **Feature:** ops-003 Release 3 -- Shared constants, cross-cutting validation, deduplication
> **Phase:** Pseudocode (SPARC P)
> **Date:** 2026-02-08
> **Scope:** Internal consolidation. 0 deploy.sh changes. No new external behavior.

### Revision History

| Date | Revision | Decisions Applied |
|------|----------|-------------------|
| 2026-02-08 | v1 | Initial pseudocode |

---

## Table of Contents

1. [Constants Module (ops-003-08)](#1-constants-module-ops-003-08)
2. [Cross-cutting Validation (ops-003-09)](#2-cross-cutting-validation-ops-003-09)
3. [Gold Validation Unification (ops-003-10)](#3-gold-validation-unification-ops-003-10)
4. [NoOpDbClient Dedup (ops-003-11)](#4-noopdbclient-dedup-ops-003-11)
5. [ndp-gold-ddl Thin Wrapper (ops-003-12)](#5-ndp-gold-ddl-thin-wrapper-ops-003-12)
6. [YAML Config Retirement (ops-003-13)](#6-yaml-config-retirement-ops-003-13)
7. [CLI Changes](#7-cli-changes)
8. [Cargo.toml Changes](#8-cargotoml-changes)
9. [Migration Procedure](#9-migration-procedure)
10. [Complexity Analysis](#10-complexity-analysis)

---

## 1. Constants Module (ops-003-08)

### 1.1 Current State: Constants Scattered Across 3 Locations

| Constant | Location 1 | Location 2 | Location 3 |
|----------|-----------|-----------|-----------|
| `VALID_METRICS` | `gold/config/types.rs:11` | `validate/semantic/gold.rs:21` | -- |
| `VALID_ROLLING_STATS` | `gold/config/types.rs:16` | -- | -- |
| `VALID_STATS` | -- | `validate/semantic/gold.rs:26` | -- |
| `GOLD_SCHEMA` | `gold/generators/constants.rs:10` | -- | -- |
| `SILVER_SCHEMA` | `gold/generators/constants.rs:13` | -- | -- |
| `NDP_ENTITY_COLUMN` | `gold/generators/constants.rs:7` | -- | -- |

**Note on naming divergence:** `validate/semantic/gold.rs` calls rolling stats
`VALID_STATS` while `gold/config/types.rs` calls them `VALID_ROLLING_STATS`.
Both contain `["mean", "std", "min", "max"]`. Phase 3 unifies under the name
`VALID_ROLLING_STATS` because it is more specific and avoids collision with
aggregate metrics.

### 1.2 London TDD: Tests FIRST

```rust
// FILE: crates/ndp-lib/src/constants.rs (tests at bottom)

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Test 1: VALID_METRICS contains all aggregate metrics
    // =========================================================================
    #[test]
    fn test_valid_metrics_contains_expected_values() {
        // These are the canonical metrics from SPEC-A01-gold-etl-schema.md
        let expected = ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"];
        for metric in &expected {
            assert!(
                VALID_METRICS.contains(metric),
                "VALID_METRICS should contain '{}'",
                metric
            );
        }
        assert_eq!(
            VALID_METRICS.len(),
            expected.len(),
            "VALID_METRICS should have exactly {} entries",
            expected.len()
        );
    }

    // =========================================================================
    // Test 2: VALID_ROLLING_STATS contains rolling stat types
    // =========================================================================
    #[test]
    fn test_valid_rolling_stats_contains_expected_values() {
        let expected = ["mean", "std", "min", "max"];
        for stat in &expected {
            assert!(
                VALID_ROLLING_STATS.contains(stat),
                "VALID_ROLLING_STATS should contain '{}'",
                stat
            );
        }
        assert_eq!(VALID_ROLLING_STATS.len(), expected.len());
    }

    // =========================================================================
    // Test 3: Schema constants are correct
    // =========================================================================
    #[test]
    fn test_schema_constants() {
        assert_eq!(GOLD_SCHEMA, "gold");
        assert_eq!(SILVER_SCHEMA, "silver");
    }

    // =========================================================================
    // Test 4: Entity column constant
    // =========================================================================
    #[test]
    fn test_ndp_entity_column() {
        assert_eq!(NDP_ENTITY_COLUMN, "ndp_id");
    }

    // =========================================================================
    // Test 5: VALID_ROLLING_STATS is a subset of VALID_METRICS
    // =========================================================================
    #[test]
    fn test_rolling_stats_subset_of_metrics() {
        for stat in VALID_ROLLING_STATS {
            assert!(
                VALID_METRICS.contains(stat),
                "Rolling stat '{}' should also be a valid metric",
                stat
            );
        }
    }
}
```

### 1.3 Implementation: `crates/ndp-lib/src/constants.rs`

```rust
//! Shared constants for NDP operations.
//!
//! Single source of truth for values used across the gold, validate,
//! and generator modules. Eliminates duplication between gold/config/types.rs,
//! gold/generators/constants.rs, and validate/semantic/gold.rs.

/// Valid aggregate metrics per SPEC-A01-gold-etl-schema.md.
///
/// Used by:
/// - `gold::validation::ConfigValidator` (validates stream configs)
/// - `gold::generators::ContinuousAggregateGenerator` (generates SQL)
/// - `validate::semantic::gold::validate_gold_etl` (semantic validation)
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];

/// Valid rolling feature statistics.
///
/// These are a subset of VALID_METRICS. Used by:
/// - `gold::validation::ConfigValidator` (validates rolling config)
/// - `validate::semantic::gold::validate_gold_etl` (semantic validation)
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

/// Gold schema name. All Gold layer objects are created in this schema.
///
/// Used by all Gold DDL generators (continuous aggregates, aligned views,
/// state transitions, events).
pub const GOLD_SCHEMA: &str = "gold";

/// Silver schema name. All Silver layer tables live here.
///
/// Used by events generator for source table references.
pub const SILVER_SCHEMA: &str = "silver";

/// Default entity identifier column used across NDP streams.
///
/// Used by state transition and events generators for entity tracking.
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";
```

### 1.4 Import Changes: Consuming Files

Each consuming file removes its local constant definition and imports from
`crate::constants` instead.

#### 1.4.1 `gold/config/types.rs` (lines 11-16 removed)

```rust
// BEFORE (crates/ndp-lib/src/gold/config/types.rs)
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

// AFTER
// (constants removed from this file entirely)
```

#### 1.4.2 `gold/config/mod.rs` (line 17 updated)

```rust
// BEFORE
pub use types::{
    Action, AggregatesConfig, FeaturesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig,
    LagConfig, RefreshPolicyConfig, RollingConfig, SilverEtlConfig, StreamConfig, TimestampConfig,
    TransitionsConfig, TrendConfig, VALID_METRICS, VALID_ROLLING_STATS,
};

// AFTER
pub use types::{
    Action, AggregatesConfig, FeaturesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig,
    LagConfig, RefreshPolicyConfig, RollingConfig, SilverEtlConfig, StreamConfig, TimestampConfig,
    TransitionsConfig, TrendConfig,
};
// Re-export constants for backward compatibility with gold::config::VALID_METRICS usage
pub use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};
```

#### 1.4.3 `gold/validation/config_validator.rs` (line 5 updated)

```rust
// BEFORE
use crate::gold::config::{FeaturesConfig, StreamConfig, VALID_METRICS, VALID_ROLLING_STATS};

// AFTER
use crate::gold::config::{FeaturesConfig, StreamConfig};
use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};
```

#### 1.4.4 `gold/generators/constants.rs` (entire file replaced)

```rust
// BEFORE (crates/ndp-lib/src/gold/generators/constants.rs)
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";
pub const GOLD_SCHEMA: &str = "gold";
pub const SILVER_SCHEMA: &str = "silver";

// AFTER
//! Generator constants -- re-exports from shared constants module.
//!
//! Preserved so that `use super::constants::GOLD_SCHEMA` in generator files
//! continues to compile without changing every generator import.
pub use crate::constants::{GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA};
```

#### 1.4.5 `gold/generators/continuous_aggregate.rs` (line 6 updated)

```rust
// BEFORE
use crate::gold::config::{
    Action, AggregatesConfig, GoldEtlConfig, RefreshPolicyConfig, StreamConfig, VALID_METRICS,
};

// AFTER
use crate::gold::config::{
    Action, AggregatesConfig, GoldEtlConfig, RefreshPolicyConfig, StreamConfig,
};
use crate::constants::VALID_METRICS;
```

#### 1.4.6 `validate/semantic/gold.rs` (lines 21-26 removed)

```rust
// BEFORE (crates/ndp-lib/src/validate/semantic/gold.rs)
const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];
const VALID_STATS: &[&str] = &["mean", "std", "min", "max"];

// AFTER
use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};
// VALID_STATS renamed to VALID_ROLLING_STATS (matches canonical name)
```

All references to `VALID_STATS` in `validate/semantic/gold.rs` (lines 269, 277)
change to `VALID_ROLLING_STATS`:

```rust
// BEFORE (line 269)
if !VALID_STATS.contains(&s) {
// ...
    VALID_STATS.join(", ")

// AFTER
if !VALID_ROLLING_STATS.contains(&s) {
// ...
    VALID_ROLLING_STATS.join(", ")
```

#### 1.4.7 `lib.rs` (add module declaration)

```rust
// BEFORE (crates/ndp-lib/src/lib.rs)
pub mod config;
pub mod convert;
pub mod db;
// ...

// AFTER
pub mod config;
pub mod constants;  // NEW
pub mod convert;
pub mod db;
// ...
```

### 1.5 Verification

```
GATE: After constants module extraction
    cargo test -p ndp-lib                    # All tests pass
    cargo test -p ndp-lib -- constants       # New constant tests pass
    cargo test -p ndp-lib -- gold            # Gold tests still pass
    cargo test -p ndp-lib -- validate        # Validate tests still pass

    # Verify no local constant definitions remain:
    grep -rn "^const VALID_METRICS" crates/ndp-lib/src/
    # Should match ONLY constants.rs

    grep -rn "^pub const VALID_METRICS" crates/ndp-lib/src/
    # Should match ONLY constants.rs

    grep -rn "^const VALID_STATS" crates/ndp-lib/src/
    # Should match NOTHING (renamed to VALID_ROLLING_STATS)
```

---

## 2. Cross-cutting Validation (ops-003-09)

### 2.1 Design

Gold mutating operations (`sync`, `recreate`) should validate configuration
by default before generating DDL. Validation is opt-out via `--no-validate`.

The wiring point is `gold::sync_stream()` and `gold::recreate_stream()`.
These call `crate::validate::validate_stream()` with the loaded config JSON
before proceeding to DDL generation.

### 2.2 SyncOptions Enhancement

```rust
// FILE: crates/ndp-lib/src/types.rs

// BEFORE
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
}

// AFTER
/// Options for sync operations.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// If true, generate SQL but do not execute against the database.
    pub dry_run: bool,
    /// If true, validate config before mutating. Default: true.
    pub validate: bool,
    /// Enable verbose diagnostic output.
    pub verbose: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            validate: true,
            verbose: false,
        }
    }
}
```

### 2.3 London TDD: Tests FIRST

```rust
// FILE: crates/ndp-lib/src/gold/mod.rs (new tests at bottom)

#[cfg(test)]
mod cross_cutting_tests {
    use super::*;
    use crate::types::SyncOptions;
    use std::path::Path;

    // Helper: create a FileSystemConfigLoader pointing at test fixtures
    fn test_loader() -> config::FileSystemConfigLoader {
        let config_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()   // crates/
            .parent().unwrap()   // repo root
            .join("config");
        config::FileSystemConfigLoader::new(&config_dir)
    }

    // =========================================================================
    // Test 1: sync validates config by default
    // =========================================================================
    //
    // STRATEGY: Call sync_stream with validate=true (default) on a stream
    // that has an intentionally invalid gold_etl config. The operation
    // should fail with a validation error BEFORE attempting any DDL generation
    // or database calls.
    //
    // This test verifies the cross-cutting wiring exists. It does NOT need
    // a database because validation runs before DB operations.
    //
    // NOTE: This requires a test fixture with an invalid gold_etl config.
    // We create one inline using a temporary directory.
    #[test]
    fn sync_validates_config_by_default() {
        let tmp = tempfile::tempdir().unwrap();

        // Create a config with an invalid metric
        let stream_dir = tmp.path().join("base").join("streams").join("test-invalid");
        std::fs::create_dir_all(&stream_dir).unwrap();
        std::fs::write(
            stream_dir.join("config.json"),
            serde_json::json!({
                "stream_id": "test-invalid",
                "fields": [
                    { "name": "pm25", "type": "float" }
                ],
                "gold_etl": {
                    "enabled": true,
                    "aggregates": {
                        "granularities": ["1 hour"],
                        "fields": {
                            "pm25": { "metrics": ["mean", "invalid_metric"] }
                        }
                    }
                }
            }).to_string(),
        ).unwrap();

        let loader = config::FileSystemConfigLoader::new(tmp.path());
        let opts = SyncOptions {
            dry_run: true,
            validate: true,   // default
            verbose: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let checker = crate::gold::db::NoOpCaChecker;
            sync_stream(&loader, "test-invalid", &checker, &opts).await
        });

        assert!(
            result.is_err(),
            "sync with validate=true should fail on invalid config"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid_metric") || err.contains("Invalid metric"),
            "Error should mention the invalid metric, got: {}",
            err
        );
    }

    // =========================================================================
    // Test 2: sync skips validation when validate=false
    // =========================================================================
    //
    // STRATEGY: Call sync_stream with validate=false on the SAME invalid
    // config. It should proceed past validation and attempt DDL generation.
    // The DDL generation itself may or may not fail (depends on the config),
    // but the point is that it should NOT fail with a validation error.
    #[test]
    fn sync_skips_validation_when_no_validate() {
        let tmp = tempfile::tempdir().unwrap();

        let stream_dir = tmp.path().join("base").join("streams").join("test-invalid");
        std::fs::create_dir_all(&stream_dir).unwrap();
        std::fs::write(
            stream_dir.join("config.json"),
            serde_json::json!({
                "stream_id": "test-invalid",
                "fields": [
                    { "name": "pm25", "type": "float" }
                ],
                "gold_etl": {
                    "enabled": true,
                    "aggregates": {
                        "granularities": ["1 hour"],
                        "fields": {
                            "pm25": { "metrics": ["mean", "invalid_metric"] }
                        }
                    }
                }
            }).to_string(),
        ).unwrap();

        let loader = config::FileSystemConfigLoader::new(tmp.path());
        let opts = SyncOptions {
            dry_run: true,
            validate: false,  // SKIP validation
            verbose: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let checker = crate::gold::db::NoOpCaChecker;
            sync_stream(&loader, "test-invalid", &checker, &opts).await
        });

        // Should NOT fail with a validation error.
        // It may fail with a DDL generation error (invalid_metric is not a
        // recognized SQL function), but the error should NOT be from the
        // validation module.
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                !msg.contains("validation") && !msg.contains("Validation"),
                "Should not fail with validation error when validate=false, got: {}",
                msg
            );
        }
        // If it succeeds (generator handles unknown metrics gracefully),
        // that's also fine -- the test only verifies validation was skipped.
    }

    // =========================================================================
    // Test 3: sync succeeds on valid config with validation enabled
    // =========================================================================
    #[test]
    fn sync_valid_config_passes_with_validation() {
        let tmp = tempfile::tempdir().unwrap();

        let stream_dir = tmp.path().join("base").join("streams").join("test-valid");
        std::fs::create_dir_all(&stream_dir).unwrap();
        std::fs::write(
            stream_dir.join("config.json"),
            serde_json::json!({
                "stream_id": "test-valid",
                "fields": [
                    { "name": "pm25", "type": "float" },
                    { "name": "co2", "type": "int" }
                ],
                "gold_etl": {
                    "enabled": true,
                    "aggregates": {
                        "granularities": ["1 hour"],
                        "fields": {
                            "pm25": { "metrics": ["mean", "std", "max"] }
                        }
                    }
                }
            }).to_string(),
        ).unwrap();

        let loader = config::FileSystemConfigLoader::new(tmp.path());
        let opts = SyncOptions {
            dry_run: true,
            validate: true,
            verbose: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let checker = crate::gold::db::NoOpCaChecker;
            sync_stream(&loader, "test-valid", &checker, &opts).await
        });

        assert!(
            result.is_ok(),
            "Valid config with validation should succeed: {:?}",
            result.unwrap_err()
        );
    }

    // =========================================================================
    // Test 4: SyncOptions defaults to validate=true
    // =========================================================================
    #[test]
    fn sync_options_defaults_validate_true() {
        let opts = SyncOptions::default();
        assert!(opts.validate, "SyncOptions should default validate to true");
        assert!(!opts.dry_run, "SyncOptions should default dry_run to false");
        assert!(!opts.verbose, "SyncOptions should default verbose to false");
    }

    // =========================================================================
    // Test 5: recreate validates config by default
    // =========================================================================
    #[test]
    fn recreate_validates_config_by_default() {
        let tmp = tempfile::tempdir().unwrap();

        let stream_dir = tmp.path().join("base").join("streams").join("test-invalid");
        std::fs::create_dir_all(&stream_dir).unwrap();
        std::fs::write(
            stream_dir.join("config.json"),
            serde_json::json!({
                "stream_id": "test-invalid",
                "fields": [
                    { "name": "pm25", "type": "float" }
                ],
                "gold_etl": {
                    "enabled": true,
                    "aggregates": {
                        "granularities": ["1 hour"],
                        "fields": {
                            "nonexistent_field": { "metrics": ["mean"] }
                        }
                    }
                }
            }).to_string(),
        ).unwrap();

        let loader = config::FileSystemConfigLoader::new(tmp.path());
        let opts = GenerateOptions {
            transitions: false,
            events: false,
            verbose: false,
        };

        let result = recreate_stream_validated(&loader, "test-invalid", &opts, true);

        assert!(
            result.is_err(),
            "recreate with validate=true should fail on invalid config"
        );
    }
}
```

### 2.4 Implementation: Gold Module Cross-cutting Wiring

#### 2.4.1 Validation convenience function

```rust
// FILE: crates/ndp-lib/src/validate/mod.rs (add new function)

/// Validate Gold ETL configuration using the semantic validator.
///
/// This is the cross-cutting entry point called by gold::sync_stream()
/// and gold::recreate_stream() before DDL generation.
///
/// Loads the config as JSON and runs validate_stream() with schema_only=false.
/// If errors are found, returns them as a formatted error string.
///
/// # Arguments
///
/// * `config` - Gold StreamConfig (typed, from gold::config)
///
/// # Returns
///
/// Ok(()) if validation passes, Err with formatted error list if it fails.
pub fn gold_config_check(
    config_json: &serde_json::Value,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let opts = ValidateOptions {
        schema_only: false,
        strict: false,
        check_tables: false,
        format: OutputFormat::Json,
        config_dir: std::path::PathBuf::from("."),
        schema_path: None,
        domain_schema_path: None,
        domains_dir: None,
        db_url: None,
    };
    let result = validate_stream(config_json, &opts);
    if result.valid {
        Ok(())
    } else {
        let error_msgs: Vec<String> = result
            .errors
            .iter()
            .map(|e| format!("[{}] {}: {}", e.code, e.path, e.message))
            .collect();
        Err(format!(
            "Config validation failed ({} error{}):\n  {}",
            error_msgs.len(),
            if error_msgs.len() == 1 { "" } else { "s" },
            error_msgs.join("\n  ")
        ).into())
    }
}
```

#### 2.4.2 Update `sync_stream()` to call validation

```rust
// FILE: crates/ndp-lib/src/gold/mod.rs

// BEFORE (sync_stream, line 137-157)
pub async fn sync_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    checker: &(impl CaChecker + Send + Sync),
    _opts: &crate::types::SyncOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
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

// AFTER
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

    // Cross-cutting validation (ops-003-09)
    if opts.validate {
        // Serialize to JSON Value for the validation layer
        let config_json = serde_json::to_value(&stream_config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        crate::validate::gold_config_check(&config_json)?;
    }

    let planner = SyncPlanner::new(checker, &stream_config);
    let plan = planner.plan(gold_etl).await?;

    Ok(plan.to_ddl())
}
```

#### 2.4.3 Update `recreate_stream()` to call validation

```rust
// FILE: crates/ndp-lib/src/gold/mod.rs

// BEFORE (recreate_stream, line 177-195)
pub fn recreate_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    _opts: &GenerateOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
    // ... existing code
}

// AFTER: Add validate parameter
/// Recreate Gold DDL for a stream (drop and create).
///
/// Optionally validates configuration before generating DDL.
/// Called by CLI with validate=!no_validate.
pub fn recreate_stream(
    loader: &impl ConfigLoader,
    stream_id: &str,
    opts: &GenerateOptions,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;
    let gold_etl = stream_config
        .gold_etl
        .as_ref()
        .ok_or_else(|| format!("Stream '{}' has no gold_etl configuration", stream_id))?;

    if !gold_etl.enabled {
        return Err(format!("Stream '{}' has gold_etl.enabled = false", stream_id).into());
    }

    let generator = ContinuousAggregateGenerator::from_stream_config(&stream_config)?;
    let sql = generator.generate(gold_etl, Action::Recreate)?;
    Ok(sql)
}

/// Recreate with optional validation gate.
///
/// This is the cross-cutting variant. The CLI calls this.
pub fn recreate_stream_validated(
    loader: &impl ConfigLoader,
    stream_id: &str,
    opts: &GenerateOptions,
    validate: bool,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let stream_config = loader.load_stream_config(stream_id)?;

    // Cross-cutting validation (ops-003-09)
    if validate {
        let config_json = serde_json::to_value(&stream_config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        crate::validate::gold_config_check(&config_json)?;
    }

    recreate_stream(loader, stream_id, opts)
}
```

**Design note:** `recreate_stream()` keeps its existing signature for backward
compatibility. A new `recreate_stream_validated()` adds the validation gate.
The CLI calls the validated variant. This avoids breaking existing callers.

### 2.5 Serialization Requirement

The cross-cutting validation requires `gold::config::StreamConfig` to implement
`Serialize` so it can be converted to `serde_json::Value` for the validation
layer. Checking the current definition:

```rust
// crates/ndp-lib/src/gold/config/types.rs line 19
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoldEtlConfig { ... }
```

`StreamConfig` already derives `Serialize`. This is confirmed by the
`Serialize` derive on `GoldEtlConfig` and related types. If `StreamConfig`
itself is missing `Serialize`, add it:

```rust
// ENSURE this derives Serialize (it likely already does)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub stream_type: Option<String>,
    pub fields: Vec<FieldConfig>,
    pub silver_etl: Option<SilverEtlConfig>,
    pub gold_etl: Option<GoldEtlConfig>,
}
```

---

## 3. Gold Validation Unification (ops-003-10)

### 3.1 Current State

There are TWO validation systems for Gold configs:

| System | Location | Input Type | Output Type | Consumers |
|--------|----------|-----------|-------------|-----------|
| `ConfigValidator` | `gold/validation/config_validator.rs` | `gold::config::StreamConfig` (typed) | `Result<(), GoldDdlError>` | `gold.rs` CLI `run_validate_only()` |
| `validate_gold_etl()` | `validate/semantic/gold.rs` | `serde_json::Value` (untyped) | `Vec<ValidationError>` | `SemanticValidator`, cross-cutting |

They validate overlapping concerns (metrics, fields, granularity) but:
- `ConfigValidator` operates on typed structs, fails-fast on first error
- `validate_gold_etl()` operates on JSON values, collects all errors
- `ConfigValidator` validates lag/rolling/trend feature config details
- `validate_gold_etl()` validates field existence with Levenshtein suggestions

### 3.2 Unification Strategy

**Keep both, make cross-cutting use `validate_gold_etl()`.**

Rationale: `validate_gold_etl()` (the semantic validator) provides better
error messages (Levenshtein suggestions, multiple error collection, structured
error codes). `ConfigValidator` is used internally by generators for fast
fail-on-first-error validation at DDL generation time.

The change is:
1. The CLI `run_validate_only()` function calls `validate_gold_etl()` via
   `validate::gold_config_check()` instead of `ConfigValidator::new().validate()`
2. Cross-cutting validation in `sync_stream()` uses `validate::gold_config_check()`
3. `ConfigValidator` remains available for internal generator use but is not
   the public validation API

### 3.3 London TDD: Tests FIRST

```rust
// FILE: crates/ndp-lib/src/gold/mod.rs (or a new test module)

#[cfg(test)]
mod validation_unification_tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // Test 1: validate_gold_etl catches all errors that ConfigValidator catches
    // =========================================================================
    #[test]
    fn semantic_validator_catches_invalid_metric() {
        let config = json!({
            "stream_id": "test",
            "fields": [{ "name": "pm25", "type": "float" }],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "pm25": { "metrics": ["mean", "average"] }
                    }
                }
            }
        });

        let errors = crate::validate::semantic::validate_gold_etl(&config);
        assert!(!errors.is_empty(), "Should catch invalid metric 'average'");
        assert!(
            errors.iter().any(|e| e.message.contains("average")),
            "Should mention 'average' in error"
        );
    }

    // =========================================================================
    // Test 2: validate_gold_etl catches invalid field reference
    // =========================================================================
    #[test]
    fn semantic_validator_catches_nonexistent_field() {
        let config = json!({
            "stream_id": "test",
            "fields": [{ "name": "pm25", "type": "float" }],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "nonexistent": { "metrics": ["mean"] }
                    }
                }
            }
        });

        let errors = crate::validate::semantic::validate_gold_etl(&config);
        assert!(!errors.is_empty(), "Should catch nonexistent field");
    }

    // =========================================================================
    // Test 3: validate_gold_etl catches invalid granularity
    // =========================================================================
    #[test]
    fn semantic_validator_catches_invalid_granularity() {
        let config = json!({
            "stream_id": "test",
            "fields": [{ "name": "pm25", "type": "float" }],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["hourly"],
                    "fields": {
                        "pm25": { "metrics": ["mean"] }
                    }
                }
            }
        });

        let errors = crate::validate::semantic::validate_gold_etl(&config);
        assert!(!errors.is_empty(), "Should catch invalid granularity 'hourly'");
    }

    // =========================================================================
    // Test 4: gold_config_check returns formatted error
    // =========================================================================
    #[test]
    fn gold_config_check_returns_error_on_invalid() {
        let config = json!({
            "stream_id": "test",
            "fields": [{ "name": "pm25", "type": "float" }],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "pm25": { "metrics": ["mean", "bogus_metric"] }
                    }
                }
            }
        });

        let result = crate::validate::gold_config_check(&config);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("bogus_metric"));
    }

    // =========================================================================
    // Test 5: gold_config_check passes on valid config
    // =========================================================================
    #[test]
    fn gold_config_check_passes_on_valid() {
        let config = json!({
            "stream_id": "test",
            "fields": [
                { "name": "pm25", "type": "float" },
                { "name": "co2", "type": "int" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "pm25": { "metrics": ["mean", "std", "max"] }
                    }
                }
            }
        });

        let result = crate::validate::gold_config_check(&config);
        assert!(result.is_ok(), "Valid config should pass: {:?}", result.unwrap_err());
    }
}
```

### 3.4 Implementation: CLI `run_validate_only` Update

```rust
// FILE: tools/ndp-cli/src/commands/gold.rs

// BEFORE (run_validate_only, line 187-216)
async fn run_validate_only(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use ndp_lib::gold::config::ConfigLoader;

    if let Some(domain_id) = domain {
        let config = ConfigLoader::load_domain_config(loader, &domain_id)?;
        println!("Domain '{}' configuration is valid", config.id);
        return Ok(());
    }

    if let Some(stream_id) = stream {
        let config = ConfigLoader::load_stream_config(loader, &stream_id)?;
        if let Some(ref gold_etl) = config.gold_etl {
            if !gold_etl.enabled {
                return Err(format!("Stream '{}' Gold ETL is disabled", stream_id).into());
            }
            // Run config validation
            ndp_lib::gold::validation::ConfigValidator::new().validate(&config)?;
            println!("Stream '{}' Gold ETL configuration is valid", stream_id);
        } else {
            return Err(format!("Stream '{}' has no gold_etl configuration", stream_id).into());
        }
        return Ok(());
    }

    Err("Must specify --stream or --domain".into())
}

// AFTER
async fn run_validate_only(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use ndp_lib::gold::config::ConfigLoader;

    if let Some(domain_id) = domain {
        let config = ConfigLoader::load_domain_config(loader, &domain_id)?;
        println!("Domain '{}' configuration is valid", config.id);
        return Ok(());
    }

    if let Some(stream_id) = stream {
        let config = ConfigLoader::load_stream_config(loader, &stream_id)?;
        if let Some(ref gold_etl) = config.gold_etl {
            if !gold_etl.enabled {
                return Err(format!("Stream '{}' Gold ETL is disabled", stream_id).into());
            }
            // Cross-cutting validation via validate module (ops-003-10)
            let config_json = serde_json::to_value(&config)
                .map_err(|e| format!("Failed to serialize config: {}", e))?;
            ndp_lib::validate::gold_config_check(&config_json)?;
            println!("Stream '{}' Gold ETL configuration is valid", stream_id);
        } else {
            return Err(format!("Stream '{}' has no gold_etl configuration", stream_id).into());
        }
        return Ok(());
    }

    Err("Must specify --stream or --domain".into())
}
```

### 3.5 What Gets Removed vs. Kept

| Item | Action | Reason |
|------|--------|--------|
| `gold::validation::ConfigValidator` | **Keep** | Used internally by generators for fast fail validation |
| `gold::validation::validate_gold_config()` | **Keep** | Same (convenience wrapper around ConfigValidator) |
| `gold::validation::parse_granularity()` | **Keep** | Used by generators for SQL generation |
| `gold::validation::parse_window()` | **Keep** | Used by generators for SQL generation |
| `gold::validation::granularity_to_suffix()` | **Keep** | Used by generators for view naming |
| CLI calling `ConfigValidator::new().validate()` | **Replace** | Use `validate::gold_config_check()` instead |

The `gold::validation` module is NOT removed. It stays as an internal module
used by generators. The public validation API shifts to `validate::gold_config_check()`.

---

## 4. NoOpDbClient Dedup (ops-003-11)

### 4.1 Current State: 4 Copies

| Location | Behavior | Used By |
|----------|----------|---------|
| `crates/ndp-lib/src/db.rs:94` | Returns `Ok(vec![])` / `Ok(0)` / `Ok(())` | Library consumers |
| `tools/ndp-cli/src/commands/dictionary.rs:116` | `unreachable!()` on all methods | `dictionary sync --dry-run` |
| `tools/ndp-cli/src/commands/dimension.rs:178` | `unreachable!()` on all methods | `dimension sync --dry-run` |
| `tools/ndp-cli/src/commands/domain.rs:146` | `unreachable!()` on all methods | `domain sync --dry-run` |

**Behavioral difference:** ndp-lib's `NoOpDbClient` returns empty results.
The CLI copies use `unreachable!()` which panics if called. This is because
the CLI dry-run paths never actually call the DB client -- they just need
a value that satisfies the `impl DbClient` constraint.

### 4.2 Decision: Use ndp-lib's NoOpDbClient Everywhere

ndp-lib's `NoOpDbClient` (returns empty results) is safer than `unreachable!()`
because if a code path accidentally reaches the DB client during dry-run,
it returns harmless empty data instead of panicking. This is the correct
behavior for a no-op implementation.

### 4.3 London TDD: Tests FIRST

```rust
// FILE: crates/ndp-lib/src/db.rs (add to existing tests module)

#[cfg(test)]
mod tests {
    use super::*;

    // ... existing tests ...

    // =========================================================================
    // Test: NoOpDbClient query returns empty vec
    // =========================================================================
    #[test]
    fn test_no_op_db_client_query_returns_empty() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = NoOpDbClient;
            let rows = client.query("SELECT 1", &[]).await.unwrap();
            assert!(rows.is_empty(), "NoOpDbClient should return empty rows");
        });
    }

    // =========================================================================
    // Test: NoOpDbClient execute returns zero rows affected
    // =========================================================================
    #[test]
    fn test_no_op_db_client_execute_returns_zero() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = NoOpDbClient;
            let count = client.execute("INSERT INTO t VALUES (1)", &[]).await.unwrap();
            assert_eq!(count, 0, "NoOpDbClient should return 0 rows affected");
        });
    }

    // =========================================================================
    // Test: NoOpDbClient batch_execute returns Ok
    // =========================================================================
    #[test]
    fn test_no_op_db_client_batch_execute_returns_ok() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = NoOpDbClient;
            let result = client.batch_execute("CREATE TABLE t (id INT)").await;
            assert!(result.is_ok(), "NoOpDbClient batch_execute should succeed");
        });
    }

    // =========================================================================
    // Test: NoOpDbClient is Send + Sync (required by trait)
    // =========================================================================
    #[test]
    fn test_no_op_db_client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NoOpDbClient>();
    }
}
```

### 4.4 Implementation: Remove CLI Copies

#### 4.4.1 `tools/ndp-cli/src/commands/dictionary.rs`

```rust
// BEFORE (lines 111-137 at end of file)
// NoOpDbClient for dry-run mode
// ...
use async_trait::async_trait;

struct NoOpDbClient;

#[async_trait]
impl ndp_lib::DbClient for NoOpDbClient {
    async fn query( ... ) -> ndp_lib::Result<Vec<tokio_postgres::Row>> {
        unreachable!("NoOpDbClient should not be called in dry_run mode")
    }
    async fn execute( ... ) -> ndp_lib::Result<u64> {
        unreachable!("NoOpDbClient should not be called in dry_run mode")
    }
    async fn batch_execute(&self, _sql: &str) -> ndp_lib::Result<()> {
        unreachable!("NoOpDbClient should not be called in dry_run mode")
    }
}

// AFTER
// (entire NoOpDbClient block removed)
// (remove `use async_trait::async_trait;` if no longer needed)
```

Usage site change (line 68):

```rust
// BEFORE
ndp_lib::dictionary::sync_dictionary(&entries, &NoOpDbClient, &options).await?;

// AFTER
ndp_lib::dictionary::sync_dictionary(&entries, &ndp_lib::NoOpDbClient, &options).await?;
```

#### 4.4.2 `tools/ndp-cli/src/commands/dimension.rs`

```rust
// BEFORE (lines 173-199 at end of file)
// (same pattern as dictionary.rs)

// AFTER
// (entire NoOpDbClient block removed)
```

Usage site change (line 124):

```rust
// BEFORE
                    &NoOpDbClient,

// AFTER
                    &ndp_lib::NoOpDbClient,
```

#### 4.4.3 `tools/ndp-cli/src/commands/domain.rs`

```rust
// BEFORE (lines 141-169 at end of file)
use async_trait::async_trait;

struct NoOpDbClient;

#[async_trait]
impl ndp_lib::DbClient for NoOpDbClient { ... }

// AFTER
// (entire NoOpDbClient block removed)
```

Usage site change (line 85):

```rust
// BEFORE
ndp_lib::domain::sync_domains(&entries, &NoOpDbClient, &options).await?;

// AFTER
ndp_lib::domain::sync_domains(&entries, &ndp_lib::NoOpDbClient, &options).await?;
```

### 4.5 Verification

```
GATE: After NoOpDbClient dedup
    cargo test -p ndp-cli                   # CLI still compiles
    cargo test -p ndp-lib -- db             # NoOpDbClient tests pass

    # Verify only ONE definition remains:
    grep -rn "struct NoOpDbClient" crates/ tools/
    # Should match ONLY crates/ndp-lib/src/db.rs

    # Verify CLI commands import from ndp_lib:
    grep -rn "ndp_lib::NoOpDbClient" tools/ndp-cli/src/
    # Should match dictionary.rs, dimension.rs, domain.rs
```

---

## 5. ndp-gold-ddl Thin Wrapper (ops-003-12)

### 5.1 Current State

`tools/ndp-gold-ddl/src/lib.rs` already re-exports from `ndp_lib::gold`.
This was done in Phase 1. The thin wrapper is already in place.

`tools/ndp-gold-ddl/src/main.rs` still contains its own CLI parsing and
direct calls to ndp-lib gold functions. This is acceptable -- main.rs is
the CLI entry point and needs its own Clap struct.

### 5.2 What Changes in Phase 3

The only change needed is updating `lib.rs` to also re-export the new
constants from `ndp_lib::constants`:

```rust
// FILE: tools/ndp-gold-ddl/src/lib.rs

// BEFORE (current, already a thin wrapper)
// ... (existing re-exports) ...

// AFTER (add constants re-export)
// Add at the end of the file:

/// Shared constants (VALID_METRICS, etc.) now live in ndp_lib::constants.
/// Re-exported here for backward compatibility.
pub mod constants {
    pub use ndp_lib::constants::{
        GOLD_SCHEMA, NDP_ENTITY_COLUMN, SILVER_SCHEMA, VALID_METRICS, VALID_ROLLING_STATS,
    };
}
```

### 5.3 ndp-validate Thin Wrapper

Already completed in Phase 2. `tools/ndp-validate/src/lib.rs` already
re-exports from `ndp_lib::validate`. No Phase 3 changes needed.

### 5.4 London TDD

```rust
// No new tests needed. Existing tests cover the thin wrapper:
// cargo test -p ndp-gold-ddl   # All tests pass via re-exports
// cargo test -p ndp-validate   # All tests pass via re-exports
```

---

## 6. YAML Config Retirement (ops-003-13)

### 6.1 Current State

7 stream directories have BOTH `config.yaml` and `config.json`:

| Stream | config.yaml | config.json |
|--------|:-----------:|:-----------:|
| `air-quality` | exists | exists |
| `home-assistant-state` | exists | exists |
| `nws-forecast-hourly` | exists | exists |
| `nws-gridpoints-forecast` | exists | exists |
| `nws-observations` | exists | exists |
| `outdoor-air-quality` | exists | exists |
| `outdoor-weather` | exists | exists |

No domain configs have `.yaml` files (only `domain.json`).

All active code paths read `config.json`. The `.yaml` files are stale
artifacts from before the JSON migration.

### 6.2 London TDD: Tests FIRST

```rust
// FILE: crates/ndp-lib/src/validate/mod.rs (add test)

#[cfg(test)]
mod yaml_retirement_tests {
    use std::path::Path;

    // =========================================================================
    // Test: No active .yaml stream configs exist
    // =========================================================================
    #[test]
    fn no_active_yaml_stream_configs() {
        let streams_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()   // crates/
            .parent().unwrap()   // repo root
            .join("config")
            .join("base")
            .join("streams");

        if !streams_dir.exists() {
            // Skip in CI environments without config dir
            return;
        }

        let mut yaml_files = Vec::new();

        for entry in std::fs::read_dir(&streams_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                let yaml_path = path.join("config.yaml");
                let yml_path = path.join("config.yml");
                if yaml_path.exists() {
                    yaml_files.push(yaml_path);
                }
                if yml_path.exists() {
                    yaml_files.push(yml_path);
                }
            }
        }

        assert!(
            yaml_files.is_empty(),
            "Found {} active .yaml/.yml stream configs that should have been \
             renamed to .yaml.bak:\n  {}",
            yaml_files.len(),
            yaml_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n  ")
        );
    }

    // =========================================================================
    // Test: No active .yaml domain configs exist
    // =========================================================================
    #[test]
    fn no_active_yaml_domain_configs() {
        let domains_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .join("config")
            .join("domains");

        if !domains_dir.exists() {
            return;
        }

        let mut yaml_files = Vec::new();

        for entry in std::fs::read_dir(&domains_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                let yaml_path = path.join("domain.yaml");
                let yml_path = path.join("domain.yml");
                if yaml_path.exists() {
                    yaml_files.push(yaml_path);
                }
                if yml_path.exists() {
                    yaml_files.push(yml_path);
                }
            }
        }

        assert!(
            yaml_files.is_empty(),
            "Found {} active .yaml/.yml domain configs:\n  {}",
            yaml_files.len(),
            yaml_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n  ")
        );
    }
}
```

### 6.3 Retirement Procedure

```bash
#!/bin/bash
# ops-003-13: Rename stale YAML stream configs to .yaml.bak
#
# These files are stale artifacts from before the JSON migration.
# All active code paths read config.json. The YAML files are renamed
# (not deleted) so they can be recovered if needed.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
STREAMS_DIR="$REPO_ROOT/config/base/streams"

echo "Retiring stale YAML stream configs..."

count=0
for yaml_file in "$STREAMS_DIR"/*/config.yaml "$STREAMS_DIR"/*/config.yml; do
    if [ -f "$yaml_file" ]; then
        # Only rename if the corresponding JSON exists
        json_file="${yaml_file%.*}.json"
        dir=$(dirname "$yaml_file")
        json_file="$dir/config.json"

        if [ -f "$json_file" ]; then
            echo "  Renaming: $yaml_file -> ${yaml_file}.bak"
            git mv "$yaml_file" "${yaml_file}.bak"
            count=$((count + 1))
        else
            echo "  WARNING: $yaml_file has no config.json counterpart, skipping"
        fi
    fi
done

echo "Retired $count YAML config files."
echo ""
echo "NOTE: platform.yaml is NOT affected by this change."
echo "      Only stream/domain config.yaml files are retired."
```

### 6.4 Files to Rename

```
git mv config/base/streams/air-quality/config.yaml         config/base/streams/air-quality/config.yaml.bak
git mv config/base/streams/home-assistant-state/config.yaml config/base/streams/home-assistant-state/config.yaml.bak
git mv config/base/streams/nws-forecast-hourly/config.yaml  config/base/streams/nws-forecast-hourly/config.yaml.bak
git mv config/base/streams/nws-gridpoints-forecast/config.yaml config/base/streams/nws-gridpoints-forecast/config.yaml.bak
git mv config/base/streams/nws-observations/config.yaml     config/base/streams/nws-observations/config.yaml.bak
git mv config/base/streams/outdoor-air-quality/config.yaml  config/base/streams/outdoor-air-quality/config.yaml.bak
git mv config/base/streams/outdoor-weather/config.yaml      config/base/streams/outdoor-weather/config.yaml.bak
```

### 6.5 What NOT to Rename

- `config/platform.yaml` -- This is the platform config, not a stream config.
  It serves a different purpose and is NOT retired.
- Any `.yaml` files outside `config/base/streams/` and `config/domains/`.

---

## 7. CLI Changes

### 7.1 `--no-validate` Wiring in Gold Commands

The `--no-validate` flag already exists in the Clap structs (added in Phase 1).
Currently it is destructured but ignored (`no_validate: _`). Phase 3 wires it.

```rust
// FILE: tools/ndp-cli/src/commands/gold.rs

// BEFORE: run() function, GoldCommands::Sync arm (line 131)
        GoldCommands::Sync {
            stream,
            domain,
            transitions: _,
            events: _,
            dry_run,
            no_validate: _,     // <-- IGNORED
        } => run_sync(&loader, stream, domain, db_url, db_timeout, dry_run).await,

// AFTER
        GoldCommands::Sync {
            stream,
            domain,
            transitions: _,
            events: _,
            dry_run,
            no_validate,        // <-- NOW USED
        } => run_sync(&loader, stream, domain, db_url, db_timeout, dry_run, !no_validate).await,
```

```rust
// BEFORE: run_sync signature (line 222)
async fn run_sync(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    db_url: Option<&str>,
    db_timeout: u64,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {

// AFTER
async fn run_sync(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    db_url: Option<&str>,
    db_timeout: u64,
    dry_run: bool,
    validate: bool,      // NEW
) -> Result<(), Box<dyn std::error::Error>> {
```

Inside `run_sync`, pass `validate` to `SyncOptions`:

```rust
// BEFORE (line 239)
        let opts = ndp_lib::types::SyncOptions { dry_run };

// AFTER
        let opts = ndp_lib::types::SyncOptions {
            dry_run,
            validate,
            verbose: false,
        };
```

Similarly for `GoldCommands::Recreate`:

```rust
// BEFORE (line 139-144)
        GoldCommands::Recreate {
            stream,
            domain,
            dry_run: _,
            no_validate: _,
        } => run_recreate(&loader, stream, domain, db_url, db_timeout).await,

// AFTER
        GoldCommands::Recreate {
            stream,
            domain,
            dry_run: _,
            no_validate,
        } => run_recreate(&loader, stream, domain, db_url, db_timeout, !no_validate).await,
```

```rust
// BEFORE: run_recreate signature
async fn run_recreate(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    db_url: Option<&str>,
    _db_timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {

// AFTER
async fn run_recreate(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream: Option<String>,
    domain: Option<String>,
    db_url: Option<&str>,
    _db_timeout: u64,
    validate: bool,      // NEW
) -> Result<(), Box<dyn std::error::Error>> {
```

Inside `run_recreate`, call the validated variant:

```rust
// BEFORE (line 292)
        let ddl = ndp_lib::gold::recreate_stream(loader, &stream_id, &opts)?;

// AFTER
        let ddl = ndp_lib::gold::recreate_stream_validated(loader, &stream_id, &opts, validate)?;
```

And for `GoldCommands::Generate`:

```rust
// BEFORE (line 117-129)
        GoldCommands::Generate {
            stream,
            domain,
            transitions,
            events,
            validate_only,
            no_validate: _,
        } => {

// AFTER
        GoldCommands::Generate {
            stream,
            domain,
            transitions,
            events,
            validate_only,
            no_validate,
        } => {
            if validate_only {
                run_validate_only(&loader, stream, domain).await
            } else if !no_validate {
                // Validate before generating
                if let Some(ref s) = stream {
                    validate_before_generate(&loader, s)?;
                }
                run_generate(&loader, stream, domain, transitions, events).await
            } else {
                run_generate(&loader, stream, domain, transitions, events).await
            }
        }
```

```rust
/// Validate config before generation (cross-cutting, ops-003-09).
fn validate_before_generate(
    loader: &ndp_lib::gold::config::FileSystemConfigLoader,
    stream_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use ndp_lib::gold::config::ConfigLoader;

    let config = ConfigLoader::load_stream_config(loader, stream_id)?;
    let config_json = serde_json::to_value(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    ndp_lib::validate::gold_config_check(&config_json)?;
    Ok(())
}
```

### 7.2 London TDD for CLI Changes

```rust
// FILE: tools/ndp-cli/src/commands/gold.rs (add tests)

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Helper to parse gold args
    fn parse_gold(args: &[&str]) -> Result<GoldArgs, clap::Error> {
        // Wrap in full CLI to test
        #[derive(Parser)]
        struct TestCli {
            #[command(subcommand)]
            command: TestCommands,
        }
        #[derive(clap::Subcommand)]
        enum TestCommands {
            Gold(GoldArgs),
        }
        let mut full_args = vec!["test", "gold"];
        full_args.extend_from_slice(args);
        let cli = TestCli::try_parse_from(full_args)?;
        match cli.command {
            TestCommands::Gold(args) => Ok(args),
        }
    }

    #[test]
    fn test_sync_no_validate_flag_parsed() {
        let args = parse_gold(&["sync", "--stream", "air-quality", "--no-validate"]).unwrap();
        match args.command {
            GoldCommands::Sync { no_validate, .. } => {
                assert!(no_validate, "--no-validate should be true");
            }
            _ => panic!("Expected Sync command"),
        }
    }

    #[test]
    fn test_sync_default_validate() {
        let args = parse_gold(&["sync", "--stream", "air-quality"]).unwrap();
        match args.command {
            GoldCommands::Sync { no_validate, .. } => {
                assert!(!no_validate, "default should be validate (no_validate=false)");
            }
            _ => panic!("Expected Sync command"),
        }
    }

    #[test]
    fn test_generate_no_validate_flag_parsed() {
        let args = parse_gold(&["generate", "--stream", "air-quality", "--no-validate"]).unwrap();
        match args.command {
            GoldCommands::Generate { no_validate, .. } => {
                assert!(no_validate);
            }
            _ => panic!("Expected Generate command"),
        }
    }

    #[test]
    fn test_recreate_no_validate_flag_parsed() {
        let args = parse_gold(&["recreate", "--stream", "air-quality", "--no-validate"]).unwrap();
        match args.command {
            GoldCommands::Recreate { no_validate, .. } => {
                assert!(no_validate);
            }
            _ => panic!("Expected Recreate command"),
        }
    }
}
```

---

## 8. Cargo.toml Changes

### 8.1 `crates/ndp-lib/Cargo.toml`

No new dependencies. Phase 3 is internal consolidation only.

The only Cargo.toml change is ensuring `serde` with `derive` feature is
available for the `Serialize` derive on `StreamConfig` (if not already):

```toml
# Already present:
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
```

### 8.2 `tools/ndp-cli/Cargo.toml`

No changes. `ndp-cli` already depends on `ndp-lib` and `serde_json`.

### 8.3 `tools/ndp-gold-ddl/Cargo.toml`

No changes. Already depends on `ndp-lib`.

### 8.4 `tools/ndp-validate/Cargo.toml`

No changes. Already depends on `ndp-lib`.

---

## 9. Migration Procedure

### Step-by-step with verification gates.

```
ALGORITHM: Phase 3 Migration
INPUT: codebase at v1.1.15 (Phase 2 complete)
OUTPUT: codebase at v1.1.16

PRE-FLIGHT:
    cargo test --workspace        # Record baseline test count
    cargo test -p ndp-lib         # Record ndp-lib test count
    cargo test -p ndp-cli         # Record ndp-cli test count

STEP 1: Create constants module (ops-003-08)
    1a. Create crates/ndp-lib/src/constants.rs with constants and tests
    1b. Add `pub mod constants;` to crates/ndp-lib/src/lib.rs
    1c. Update gold/config/types.rs (remove constants, keep types)
    1d. Update gold/config/mod.rs (re-export from crate::constants)
    1e. Update gold/generators/constants.rs (re-export from crate::constants)
    1f. Update gold/generators/continuous_aggregate.rs (import from crate::constants)
    1g. Update gold/validation/config_validator.rs (import from crate::constants)
    1h. Update validate/semantic/gold.rs (import, rename VALID_STATS)

    VERIFICATION GATE:
        cargo test -p ndp-lib -- constants    # New tests pass
        cargo test -p ndp-lib -- gold         # Gold tests still pass
        cargo test -p ndp-lib -- validate     # Validate tests still pass
        # Verify single source of truth:
        grep -rn "pub const VALID_METRICS" crates/ndp-lib/src/
        # Should match ONLY constants.rs

STEP 2: Enhance SyncOptions (ops-003-09, prep)
    2a. Add `validate: bool` and `verbose: bool` to SyncOptions
    2b. Update Default impl

    VERIFICATION GATE:
        cargo test -p ndp-lib -- types       # Compiles
        cargo test -p ndp-cli                # CLI still builds (uses SyncOptions)

STEP 3: Add gold_config_check() (ops-003-09)
    3a. Add gold_config_check() to crates/ndp-lib/src/validate/mod.rs
    3b. Add cross-cutting tests

    VERIFICATION GATE:
        cargo test -p ndp-lib -- gold_config_check  # New tests pass

STEP 4: Wire cross-cutting validation (ops-003-09)
    4a. Update sync_stream() to check opts.validate
    4b. Add recreate_stream_validated()
    4c. Add cross-cutting tests for sync/recreate

    VERIFICATION GATE:
        cargo test -p ndp-lib -- cross_cutting    # New tests pass
        cargo test -p ndp-lib -- gold             # Existing gold tests still pass

STEP 5: Update SyncOptions usage in CLI (ops-003-09)
    5a. Wire --no-validate in gold.rs run() function
    5b. Update run_sync() and run_recreate() signatures
    5c. Update run_validate_only() to use gold_config_check() (ops-003-10)
    5d. Wire validate_before_generate() for generate command

    VERIFICATION GATE:
        cargo test -p ndp-cli                    # CLI tests pass
        cargo build -p ndp-cli
        # Manual test:
        cargo run -p ndp-cli -- gold generate --stream air-quality \
            --config-dir config/base
        cargo run -p ndp-cli -- gold generate --stream air-quality \
            --config-dir config/base --no-validate

STEP 6: Deduplicate NoOpDbClient (ops-003-11)
    6a. Remove NoOpDbClient from dictionary.rs
    6b. Remove NoOpDbClient from dimension.rs
    6c. Remove NoOpDbClient from domain.rs
    6d. Replace all usages with ndp_lib::NoOpDbClient

    VERIFICATION GATE:
        cargo test -p ndp-cli                    # CLI tests pass
        grep -rn "struct NoOpDbClient" tools/    # No matches
        cargo run -p ndp-cli -- dictionary sync --dry-run \
            --db-url postgresql://localhost/test --config-dir config/base

STEP 7: Update ndp-gold-ddl thin wrapper (ops-003-12)
    7a. Add constants re-export to ndp-gold-ddl lib.rs

    VERIFICATION GATE:
        cargo test -p ndp-gold-ddl               # All tests pass

STEP 8: Retire YAML configs (ops-003-13)
    8a. Run retirement procedure (git mv *.yaml -> *.yaml.bak)
    8b. Add YAML retirement test

    VERIFICATION GATE:
        cargo test -p ndp-lib -- yaml_retirement  # Test passes
        ls config/base/streams/*/config.yaml 2>/dev/null
        # Should list nothing

STEP 9: Final verification
    cargo test --workspace             # ALL tests pass
    cargo clippy --workspace           # No warnings
    cargo fmt --check                  # Formatted

    # Verify acceptance criteria:
    # 1. VALID_METRICS defined in exactly one place
    grep -c "pub const VALID_METRICS" crates/ndp-lib/src/constants.rs  # 1
    grep -rn "const VALID_METRICS" crates/ndp-lib/src/ | grep -v constants.rs | grep -v ".bak"  # 0

    # 2. NoOpDbClient defined in exactly one place
    grep -rn "struct NoOpDbClient" crates/ tools/  # 1 match (db.rs)

    # 3. Cross-cutting validation works
    cargo run -p ndp-cli -- gold sync --stream air-quality \
        --config-dir config/base --dry-run
    # Should succeed (validates then generates)

    # 4. --no-validate skips validation
    cargo run -p ndp-cli -- gold sync --stream air-quality \
        --config-dir config/base --dry-run --no-validate
    # Should also succeed

    # 5. Standalone binaries still work
    cargo run -p ndp-gold-ddl -- --config-dir config \
        generate --stream air-quality
```

---

## 10. Complexity Analysis

### 10.1 Change Summary

```
Files CREATED:    1
    crates/ndp-lib/src/constants.rs              (~50 lines)

Files MODIFIED:  12
    crates/ndp-lib/src/lib.rs                    (add pub mod constants)
    crates/ndp-lib/src/types.rs                  (add validate/verbose to SyncOptions)
    crates/ndp-lib/src/validate/mod.rs           (add gold_config_check + tests)
    crates/ndp-lib/src/gold/mod.rs               (wire cross-cutting + tests)
    crates/ndp-lib/src/gold/config/types.rs      (remove constant definitions)
    crates/ndp-lib/src/gold/config/mod.rs        (update re-exports)
    crates/ndp-lib/src/gold/generators/constants.rs  (re-export from crate::constants)
    crates/ndp-lib/src/gold/generators/continuous_aggregate.rs  (update import)
    crates/ndp-lib/src/gold/validation/config_validator.rs  (update import)
    crates/ndp-lib/src/validate/semantic/gold.rs (import + rename VALID_STATS)
    tools/ndp-cli/src/commands/gold.rs           (wire --no-validate)
    tools/ndp-gold-ddl/src/lib.rs                (add constants re-export)

Files MODIFIED (deletions): 3
    tools/ndp-cli/src/commands/dictionary.rs     (remove NoOpDbClient copy)
    tools/ndp-cli/src/commands/dimension.rs      (remove NoOpDbClient copy)
    tools/ndp-cli/src/commands/domain.rs         (remove NoOpDbClient copy)

Files RENAMED: 7
    config/base/streams/*/config.yaml -> config.yaml.bak

New tests:  ~25
    constants.rs:        5 tests
    cross-cutting:       5 tests
    validation unify:    5 tests
    NoOpDbClient:        4 tests
    YAML retirement:     2 tests
    CLI --no-validate:   4 tests
```

### 10.2 Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| VALID_STATS rename breaks validate tests | Low | Low | Simple find-replace; cargo test catches immediately |
| SyncOptions new fields break existing callers | Low | Medium | Default impl provides backward-compatible defaults |
| gold::config::StreamConfig missing Serialize | Low | Low | Already has Serialize derive |
| Cross-cutting validation too strict | Medium | Medium | `--no-validate` opt-out available; validate only catches real errors |
| NoOpDbClient behavioral difference | Low | Low | ndp-lib version is safer (returns empty vs. panic) |
| YAML .bak files confuse git | Low | Low | `git mv` preserves history; .bak extension is obvious |

### 10.3 Dependency Impact

No new crate dependencies. No compile-time impact. No binary size change.

---

## Appendix A: Constant Location Audit (Complete)

After Phase 3, all constant definitions live in `crates/ndp-lib/src/constants.rs`.
All other files import via `use crate::constants::*` or re-export for backward
compatibility.

| Constant | Definition | Re-exports |
|----------|-----------|------------|
| `VALID_METRICS` | `constants.rs` | `gold/config/mod.rs`, `ndp-gold-ddl/lib.rs` |
| `VALID_ROLLING_STATS` | `constants.rs` | `gold/config/mod.rs`, `ndp-gold-ddl/lib.rs` |
| `GOLD_SCHEMA` | `constants.rs` | `gold/generators/constants.rs`, `ndp-gold-ddl/lib.rs` |
| `SILVER_SCHEMA` | `constants.rs` | `gold/generators/constants.rs`, `ndp-gold-ddl/lib.rs` |
| `NDP_ENTITY_COLUMN` | `constants.rs` | `gold/generators/constants.rs`, `ndp-gold-ddl/lib.rs` |

## Appendix B: NoOpDbClient Dedup Audit (Complete)

| Location | Before Phase 3 | After Phase 3 |
|----------|:-------------:|:-------------:|
| `crates/ndp-lib/src/db.rs` | Definition (returns empty) | Definition (unchanged) |
| `tools/ndp-cli/src/commands/dictionary.rs` | Copy (unreachable!) | **Removed**, uses `ndp_lib::NoOpDbClient` |
| `tools/ndp-cli/src/commands/dimension.rs` | Copy (unreachable!) | **Removed**, uses `ndp_lib::NoOpDbClient` |
| `tools/ndp-cli/src/commands/domain.rs` | Copy (unreachable!) | **Removed**, uses `ndp_lib::NoOpDbClient` |

## Appendix C: SyncOptions Callers

All callers of `SyncOptions` must be updated for the new fields:

| File | Current Usage | Update Needed |
|------|--------------|---------------|
| `gold/mod.rs:141` | `_opts: &SyncOptions` | Use `opts.validate` |
| `gold/mod.rs:167` | `_opts: &SyncOptions` | No change (domain sync, no validate) |
| `commands/gold.rs:239` | `SyncOptions { dry_run }` | Add `validate`, `verbose` |
| `commands/gold.rs:264` | `SyncOptions { dry_run }` | Add `validate`, `verbose` |
| `commands/dictionary.rs` | `SyncOptions { dry_run }` | Add `validate: false, verbose: false` |
| `commands/dimension.rs` | `SyncOptions { dry_run }` | Add `validate: false, verbose: false` |
| `commands/domain.rs` | `SyncOptions { dry_run }` | Add `validate: false, verbose: false` |

**Design note:** Dictionary, dimension, and domain sync do NOT validate by
default because their configs are not Gold configs. Cross-cutting validation
(ops-003-09) is specific to Gold operations. Non-gold callers set
`validate: false` explicitly.

**Alternative:** Use `SyncOptions::default()` and override only `dry_run`.
Since Default sets `validate: true`, non-gold callers would need to explicitly
set `validate: false`. This is the safer direction -- opt-out rather than opt-in.
