# OPS-003 Phase 3 Test Plan: Shared Constants + Cross-cutting Validation

> **Feature:** ops-003 Phase 3 -- Shared constants, cross-cutting validation, NoOpDbClient dedup, YAML cleanup
> **Author:** ndp-architect (specification agent)
> **Date:** 2026-02-08
> **Status:** Draft
> **AgentDB Patterns Used:** ID 1 (development:crate-module-migration), ID 4 (procedure:crate-validate-migration)

---

## 1. Test Strategy Overview

### 1.1 Testing Layers

| Layer | Purpose | Count Target | Speed |
|-------|---------|-------------|-------|
| **Unit Tests** | Verify constants, cross-cutting validation, SyncOptions changes | ~35 new | < 15s |
| **Regression Tests** | Ensure all 740+ existing tests still pass | 740+ unchanged | < 120s |
| **Integration Tests** | Cross-cutting validation with real configs + integration stack | ~8 new | < 120s |
| **Static Analysis** | Grep-based verification of dedup and cleanup | ~20 checks | < 5s |

### 1.2 Test Methodology: London TDD (Outside-In)

All new tests follow London TDD (AgentDB pattern ID 16):

1. **Write failing test first** -- Define expected behavior before implementation
2. **Mock dependencies** -- Use trait-based mocking for DB interactions
3. **Outside-in** -- Start from CLI behavior, work inward to library functions
4. **Arrange-Act-Assert** structure
5. **Test naming**: `test_<function>_<scenario>_<expected>`

### 1.3 Key Principle: Phase 3 Is Internal Consolidation

Phase 3 has **zero deploy.sh changes** and **zero new CLI flags**. The `--no-validate` flag already exists but was not wired. Testing focuses on:

- Constants are defined once and used everywhere
- Cross-cutting validation fires by default and can be bypassed
- NoOpDbClient dedup produces same observable behavior
- YAML configs are properly retired

### 1.4 Existing Test Baseline

| Package | Tests (Phase 2 end) | Notes |
|---------|-------------------|-------|
| ndp-lib | 675 (validate) + ~491 (gold) + ~30 (other) | Main library |
| ndp-validate | 65 | Thin wrapper CLI tests |
| ndp-gold-ddl | 15 | Golden master tests |
| ndp-types | 88 | Foundation types |
| ndp-cli | 16 (doc) | CLI doc tests |
| **Total** | **~740+** | per STATUS.md Phase 2 report |

---

## 2. Unit Tests: Shared Constants (FR-01)

### 2.1 Constants Module Tests

These tests verify that `ndp_lib::constants` contains the correct values and that all consumers use them.

| Test ID | Function Under Test | Scenario | Expected |
|---------|-------------------|----------|----------|
| CONST-01 | `constants::VALID_METRICS` | Contains all 9 metrics | `["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]` |
| CONST-02 | `constants::VALID_METRICS` | Length is 9 | `VALID_METRICS.len() == 9` |
| CONST-03 | `constants::VALID_ROLLING_STATS` | Contains all 4 stats | `["mean", "std", "min", "max"]` |
| CONST-04 | `constants::VALID_ROLLING_STATS` | Length is 4 | `VALID_ROLLING_STATS.len() == 4` |
| CONST-05 | `constants::VALID_ROLLING_STATS` | Is subset of VALID_METRICS | Every stat in VALID_ROLLING_STATS is also in VALID_METRICS |
| CONST-06 | `constants::GOLD_SCHEMA` | Equals "gold" | `GOLD_SCHEMA == "gold"` |
| CONST-07 | `constants::SILVER_SCHEMA` | Equals "silver" | `SILVER_SCHEMA == "silver"` |
| CONST-08 | `constants::NDP_ENTITY_COLUMN` | Equals "ndp_id" | `NDP_ENTITY_COLUMN == "ndp_id"` |

### 2.2 Constants Consumption Tests

These tests verify that the gold and validate modules use the shared constants (not local copies).

| Test ID | Module Under Test | Scenario | Expected |
|---------|------------------|----------|----------|
| CONST-09 | `gold::validation::ConfigValidator` | Rejects metric not in VALID_METRICS | Error message lists the same 9 valid metrics as `constants::VALID_METRICS` |
| CONST-10 | `validate::semantic::gold::validate_gold_etl()` | Rejects metric not in VALID_METRICS | Error message lists the same 9 valid metrics |
| CONST-11 | `gold::validation::ConfigValidator` | Rejects stat not in VALID_ROLLING_STATS | Error message references VALID_ROLLING_STATS values |
| CONST-12 | `validate::semantic::gold::validate_gold_etl()` | Rejects stat not in VALID_ROLLING_STATS | Error message references the same 4 valid stats |
| CONST-13 | Both validators | Accept "p99" as valid metric | Both return no error for "p99" (edge case: p99 is valid in VALID_METRICS but not in VALID_ROLLING_STATS) |
| CONST-14 | Both validators | Reject "average" as metric | Both produce InvalidAggregateMetric/InvalidMetric error |

### 2.3 Implementation: Constants Unit Tests

```rust
// crates/ndp-lib/src/constants.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_metrics_contains_all_expected() {
        let expected = ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"];
        assert_eq!(VALID_METRICS.len(), expected.len());
        for metric in &expected {
            assert!(
                VALID_METRICS.contains(metric),
                "VALID_METRICS missing '{}'",
                metric
            );
        }
    }

    #[test]
    fn test_valid_rolling_stats_contains_all_expected() {
        let expected = ["mean", "std", "min", "max"];
        assert_eq!(VALID_ROLLING_STATS.len(), expected.len());
        for stat in &expected {
            assert!(
                VALID_ROLLING_STATS.contains(stat),
                "VALID_ROLLING_STATS missing '{}'",
                stat
            );
        }
    }

    #[test]
    fn test_rolling_stats_is_subset_of_metrics() {
        for stat in VALID_ROLLING_STATS {
            assert!(
                VALID_METRICS.contains(stat),
                "VALID_ROLLING_STATS contains '{}' which is not in VALID_METRICS",
                stat
            );
        }
    }

    #[test]
    fn test_gold_schema_value() {
        assert_eq!(GOLD_SCHEMA, "gold");
    }

    #[test]
    fn test_silver_schema_value() {
        assert_eq!(SILVER_SCHEMA, "silver");
    }

    #[test]
    fn test_ndp_entity_column_value() {
        assert_eq!(NDP_ENTITY_COLUMN, "ndp_id");
    }
}
```

---

## 3. Unit Tests: Cross-cutting Validation (FR-02)

### 3.1 SyncOptions Tests

| Test ID | Function Under Test | Scenario | Expected |
|---------|-------------------|----------|----------|
| XVAL-01 | `SyncOptions::default()` | Default construction | `validate == true`, `dry_run == false` |
| XVAL-02 | `SyncOptions` | Explicit `validate: false` | Field is `false` |
| XVAL-03 | `SyncOptions` | Explicit `validate: true, dry_run: true` | Both fields set |

### 3.2 Cross-cutting Validation in sync_stream() Tests

These tests verify that `gold::sync_stream()` calls validation when `opts.validate` is true and skips it when false. They use a mock `CaChecker`.

| Test ID | Function Under Test | Scenario | Expected |
|---------|-------------------|----------|----------|
| XVAL-04 | `gold::sync_stream()` | Valid config, `validate: true` | Returns Ok with DDL string |
| XVAL-05 | `gold::sync_stream()` | Config with invalid metric, `validate: true` | Returns Err containing "validation failed" |
| XVAL-06 | `gold::sync_stream()` | Config with invalid metric, `validate: false` | Returns Ok (validation bypassed, DDL generated even with invalid metric) |
| XVAL-07 | `gold::sync_stream()` | Config with nonexistent field, `validate: true` | Returns Err containing "FieldNotFound" or "validation failed" |
| XVAL-08 | `gold::sync_stream()` | Missing gold_etl section, `validate: true` | Returns Err (missing gold_etl is checked before validation) |
| XVAL-09 | `gold::sync_stream()` | gold_etl.enabled = false, `validate: true` | Returns Err (disabled check happens before validation) |

### 3.3 Cross-cutting Validation in CLI Tests

| Test ID | Function Under Test | Scenario | Expected |
|---------|-------------------|----------|----------|
| XVAL-10 | `ndp gold sync --stream X --no-validate` | Passes SyncOptions with validate=false | DDL generated without validation |
| XVAL-11 | `ndp gold sync --stream X` (no --no-validate) | Passes SyncOptions with validate=true | Validation runs before DDL generation |
| XVAL-12 | `ndp gold generate --stream X --no-validate` | no_validate flag is respected | No validation error even with bad config |
| XVAL-13 | `ndp gold recreate --stream X --no-validate` | no_validate flag is respected | Same as XVAL-12 |

### 3.4 Implementation: Cross-cutting Validation Tests

```rust
// In crates/ndp-lib/src/gold/mod.rs or a new test file

#[cfg(test)]
mod cross_cutting_tests {
    use super::*;
    use crate::gold::config::{
        AggregatesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig, StreamConfig,
    };
    use crate::gold::db::{CaChecker, CaInfo};
    use std::collections::HashMap;

    // Mock CaChecker that returns empty (no existing CAs)
    struct MockCaChecker;

    #[async_trait::async_trait]
    impl CaChecker for MockCaChecker {
        async fn get_existing_cas(
            &self,
            _schema: &str,
        ) -> crate::gold::error::Result<Vec<CaInfo>> {
            Ok(vec![])
        }
    }

    fn valid_stream_config() -> StreamConfig {
        StreamConfig {
            stream_id: "test-stream".to_string(),
            stream_type: None,
            fields: vec![
                FieldConfig {
                    name: "pm25".to_string(),
                    field_type: "float".to_string(),
                },
            ],
            silver_etl: None,
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                aggregates: Some(AggregatesConfig {
                    granularities: vec!["1 hour".to_string()],
                    fields: {
                        let mut map = HashMap::new();
                        map.insert(
                            "pm25".to_string(),
                            FieldMetricsConfig {
                                metrics: vec!["mean".to_string()],
                            },
                        );
                        map
                    },
                }),
                features: None,
                refresh_policy: None,
            }),
        }
    }

    fn invalid_metric_config() -> StreamConfig {
        let mut config = valid_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        let agg = gold_etl.aggregates.as_mut().unwrap();
        agg.fields
            .get_mut("pm25")
            .unwrap()
            .metrics
            .push("invalid_metric".to_string());
        config
    }

    #[test]
    fn test_sync_options_default_validate_true() {
        let opts = crate::types::SyncOptions::default();
        assert!(opts.validate, "Default SyncOptions should have validate=true");
        assert!(!opts.dry_run);
    }

    #[test]
    fn test_sync_options_no_validate() {
        let opts = crate::types::SyncOptions {
            dry_run: false,
            validate: false,
        };
        assert!(!opts.validate);
    }
}
```

---

## 4. Unit Tests: NoOpDbClient Dedup (FR-04)

### 4.1 Test Cases

| Test ID | Function Under Test | Scenario | Expected |
|---------|-------------------|----------|----------|
| NOOP-01 | `ndp_lib::NoOpDbClient::query()` | Any query | Returns `Ok(vec![])` |
| NOOP-02 | `ndp_lib::NoOpDbClient::execute()` | Any statement | Returns `Ok(0)` |
| NOOP-03 | `ndp_lib::NoOpDbClient::batch_execute()` | Any SQL | Returns `Ok(())` |
| NOOP-04 | `ndp_lib::NoOpDbClient` | Implements `DbClient` trait | Compiles: `let _: &dyn DbClient = &NoOpDbClient;` |
| NOOP-05 | `ndp_lib::NoOpDbClient` | Implements `Send + Sync` | Compiles: `fn assert_send_sync<T: Send + Sync>() {} assert_send_sync::<NoOpDbClient>();` |

### 4.2 Note: Existing Tests in db.rs

The `ndp_lib::db` module already has 3 tests (`test_invalid_url_rejected`, `test_postgres_url_accepted`, `test_alternate_postgres_url_accepted`) but none for `NoOpDbClient`. Tests NOOP-01 through NOOP-05 are new additions.

### 4.3 Implementation

```rust
// In crates/ndp-lib/src/db.rs, add to existing #[cfg(test)] mod tests

#[test]
fn test_noop_db_client_query_returns_empty() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = NoOpDbClient;
        let result = client.query("SELECT 1", &[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    });
}

#[test]
fn test_noop_db_client_execute_returns_zero() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = NoOpDbClient;
        let result = client.execute("INSERT INTO t VALUES (1)", &[]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    });
}

#[test]
fn test_noop_db_client_batch_execute_returns_ok() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = NoOpDbClient;
        let result = client.batch_execute("CREATE TABLE t (id INT);").await;
        assert!(result.is_ok());
    });
}

#[test]
fn test_noop_db_client_implements_db_client_trait() {
    fn accepts_db_client(_: &dyn DbClient) {}
    accepts_db_client(&NoOpDbClient);
}

#[test]
fn test_noop_db_client_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NoOpDbClient>();
}
```

---

## 5. Regression Tests

### 5.1 All Existing Tests Must Pass

The primary regression gate is `cargo test --workspace`. Phase 3 changes constants (same values) and adds a field to `SyncOptions`. No logic changes. All existing tests should pass without modification.

| Test ID | Scope | Command | Expected |
|---------|-------|---------|----------|
| REG-01 | ndp-lib gold tests | `cargo test -p ndp-lib -- gold` | 0 failures |
| REG-02 | ndp-lib validate tests | `cargo test -p ndp-lib -- validate` | 0 failures |
| REG-03 | ndp-lib db tests | `cargo test -p ndp-lib -- db` | 0 failures |
| REG-04 | ndp-gold-ddl golden masters | `cargo test -p ndp-gold-ddl` | 0 failures |
| REG-05 | ndp-validate thin wrapper | `cargo test -p ndp-validate` | 0 failures |
| REG-06 | ndp-types | `cargo test -p ndp-types` | 0 failures |
| REG-07 | Full workspace | `cargo test --workspace` | 0 failures |

### 5.2 Gold DDL Parity

Gold DDL generation must produce identical SQL before and after Phase 3.

| Test ID | Scenario | Verification |
|---------|----------|-------------|
| REG-08 | `ndp gold generate --stream air-quality` | Same DDL as before Phase 3 |
| REG-09 | `ndp gold generate --domain indoor-air-quality` | Same DDL as before Phase 3 |
| REG-10 | `ndp gold generate --stream air-quality --transitions` | Same DDL as before Phase 3 |
| REG-11 | `ndp gold generate --domain indoor-air-quality --events` | Same DDL as before Phase 3 |

### 5.3 Validation Output Parity

Validation must produce identical errors and warnings.

| Test ID | Scenario | Verification |
|---------|----------|-------------|
| REG-12 | `ndp validate --all` | Same error/warning counts as before Phase 3 |
| REG-13 | `ndp validate --domain-all` | Same output as before Phase 3 |

---

## 6. Static Analysis Tests

These are grep-based verification checks that ensure deduplication and cleanup are complete. They can be run as a shell script or as Rust tests using `std::process::Command`.

### 6.1 Constants Dedup Verification

| Test ID | Check | Command | Expected |
|---------|-------|---------|----------|
| STATIC-01 | No local VALID_METRICS in validate | `grep -n "const VALID_METRICS" crates/ndp-lib/src/validate/semantic/gold.rs` | 0 matches |
| STATIC-02 | No local VALID_STATS in validate | `grep -n "const VALID_STATS" crates/ndp-lib/src/validate/semantic/gold.rs` | 0 matches |
| STATIC-03 | No local VALID_METRICS in gold types | `grep -n "const VALID_METRICS" crates/ndp-lib/src/gold/config/types.rs` | 0 matches |
| STATIC-04 | No local VALID_ROLLING_STATS in gold types | `grep -n "const VALID_ROLLING_STATS" crates/ndp-lib/src/gold/config/types.rs` | 0 matches |
| STATIC-05 | No local constants in generators | `grep -n "^pub const" crates/ndp-lib/src/gold/generators/constants.rs` | 0 matches (only re-exports) |
| STATIC-06 | Single VALID_METRICS definition | `grep -rn "const VALID_METRICS" crates/ndp-lib/src/` | Exactly 1 match in constants.rs |
| STATIC-07 | Single VALID_ROLLING_STATS definition | `grep -rn "const VALID_ROLLING_STATS" crates/ndp-lib/src/` | Exactly 1 match in constants.rs |
| STATIC-08 | Single GOLD_SCHEMA definition | `grep -rn 'const GOLD_SCHEMA' crates/ndp-lib/src/` | Exactly 1 match in constants.rs |
| STATIC-09 | Single SILVER_SCHEMA definition | `grep -rn 'const SILVER_SCHEMA' crates/ndp-lib/src/` | Exactly 1 match in constants.rs |
| STATIC-10 | Single NDP_ENTITY_COLUMN definition | `grep -rn 'const NDP_ENTITY_COLUMN' crates/ndp-lib/src/` | Exactly 1 match in constants.rs |

### 6.2 NoOpDbClient Dedup Verification

| Test ID | Check | Command | Expected |
|---------|-------|---------|----------|
| STATIC-11 | No NoOpDbClient in ndp-cli | `grep -rn "struct NoOpDbClient" tools/ndp-cli/src/` | 0 matches |
| STATIC-12 | No unreachable!() in ndp-cli commands | `grep -rn 'unreachable!.*NoOpDbClient' tools/ndp-cli/src/` | 0 matches |
| STATIC-13 | CLI commands import from ndp_lib | `grep -rn "ndp_lib::NoOpDbClient\|use ndp_lib::db::NoOpDbClient" tools/ndp-cli/src/` | 3 matches (domain, dictionary, dimension) |

### 6.3 YAML Cleanup Verification

| Test ID | Check | Command | Expected |
|---------|-------|---------|----------|
| STATIC-14 | No config.yaml in base streams | `find config/base/streams -name "config.yaml" -type f` | 0 results |
| STATIC-15 | No config.yaml in integration streams | `find config/integration/base/streams -name "config.yaml" -type f` | 0 results |
| STATIC-16 | Backup files exist (base) | `find config/base/streams -name "config.yaml.bak" -type f \| wc -l` | 7 |
| STATIC-17 | Backup files exist (integration) | `find config/integration/base/streams -name "config.yaml.bak" -type f \| wc -l` | 3 |
| STATIC-18 | platform.yaml preserved | `test -f config/base/platform.yaml` | Exit 0 |
| STATIC-19 | No code references config.yaml for streams | `grep -rn '"config.yaml"' crates/ndp-lib/src/` | 0 matches |
| STATIC-20 | No code references config.yml | `grep -rn '"config.yml"' crates/ndp-lib/src/` | 0 matches |

### 6.4 SyncOptions Construction Verification

| Test ID | Check | Command | Expected |
|---------|-------|---------|----------|
| STATIC-21 | All SyncOptions have validate field | `grep -rn "SyncOptions {" tools/ndp-cli/src/ crates/ndp-lib/src/` | Every match includes `validate:` |
| STATIC-22 | No TODO/unimplemented in changed files | `grep -rn 'TODO\|unimplemented!\|todo!' crates/ndp-lib/src/constants.rs crates/ndp-lib/src/types.rs tools/ndp-cli/src/commands/gold.rs` | 0 matches |

---

## 7. Integration Tests (Requires Integration Stack)

### 7.1 Prerequisites

```bash
docker compose -f docker-compose.integration.yml up -d
# Wait for TimescaleDB:
docker exec integration-timescaledb pg_isready -U postgres -d ndp
# Build ndp CLI:
cargo build -p ndp-cli
```

### 7.2 Cross-cutting Validation E2E Tests

These tests use the integration environment to verify cross-cutting validation with real configs and a real database.

| Test ID | Scenario | Command | Expected |
|---------|----------|---------|----------|
| INT-01 | Gold sync with validation (happy path) | `ndp gold sync --stream air-quality --db-url postgresql://postgres:postgres@localhost:5432/ndp --config-dir config/base --dry-run` | Exit 0, DDL output |
| INT-02 | Gold sync with --no-validate (happy path) | `ndp gold sync --stream air-quality --db-url postgresql://postgres:postgres@localhost:5432/ndp --config-dir config/base --dry-run --no-validate` | Exit 0, DDL output |
| INT-03 | Gold generate with validation (happy path) | `ndp gold generate --stream air-quality --config-dir config/base` | Exit 0, DDL output |
| INT-04 | Gold generate with --no-validate | `ndp gold generate --stream air-quality --config-dir config/base --no-validate` | Exit 0, DDL output (same as INT-03 for valid config) |
| INT-05 | Full deploy.sh apply (regression) | `DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.17.manifest.json` | All phases complete (Gold sync uses validation by default now) |
| INT-06 | Validate then sync pipeline | `ndp validate --all --config-dir config/base && ndp gold sync --stream air-quality --db-url ... --dry-run` | Both commands succeed |
| INT-07 | DDL parity with previous version | `diff <(ndp gold generate --stream air-quality --config-dir config/base) <(expected_ddl)` | Identical DDL output |
| INT-08 | NoOpDbClient dedup (dry-run commands) | `ndp dictionary sync --dry-run --config-dir config/base && ndp dimension sync --dry-run --config-dir config/base && ndp domain sync --dry-run --config-dir config/base` | All 3 succeed (using ndp_lib::NoOpDbClient) |

### 7.3 Integration Test Notes

- **INT-05** is the OPS-003 success test from SCOPE.md: "Can deploy.sh apply complete identically using only the ndp binary?"
- **INT-07** requires capturing DDL output before Phase 3 implementation as a golden master
- **INT-08** verifies that the NoOpDbClient swap does not break dry-run behavior for dictionary, dimension, and domain sync

---

## 8. Test Execution Order (London TDD: Outside-In)

### 8.1 Phase A: Write Failing Tests First

Before any implementation, write the following test files:

1. **`crates/ndp-lib/src/constants.rs`** -- Write test module (CONST-01 through CONST-08). Tests fail because the file does not exist yet.

2. **`crates/ndp-lib/src/types.rs`** -- Add tests for `SyncOptions.validate` (XVAL-01 through XVAL-03). Tests fail because the field does not exist yet.

3. **`crates/ndp-lib/src/db.rs`** -- Add NoOpDbClient tests (NOOP-01 through NOOP-05). These pass immediately (NoOpDbClient already exists) -- they serve as regression documentation.

### 8.2 Phase B: Implement to Green

Implement in this order, running tests after each step:

| Step | Implementation | Tests to Run | Expected |
|------|---------------|-------------|----------|
| B1 | Create `constants.rs`, register in `lib.rs` | `cargo test -p ndp-lib -- constants` | CONST-01 through CONST-08 pass |
| B2 | Update `gold::config::types` -- remove local constants | `cargo test -p ndp-lib -- gold::config` | Existing gold config tests pass |
| B3 | Update `gold::config::mod` -- re-export chain | `cargo test -p ndp-lib -- gold` | All gold tests pass |
| B4 | Update `gold::generators::constants` -- re-export chain | `cargo test -p ndp-lib -- gold::generators` | All generator tests pass |
| B5 | Update `validate::semantic::gold` -- import shared, rename VALID_STATS | `cargo test -p ndp-lib -- validate::semantic::gold` | All semantic gold tests pass |
| B6 | Full regression checkpoint | `cargo test --workspace` | All 740+ tests pass |
| B7 | Add `validate` field to `SyncOptions` | `cargo test -p ndp-lib -- types` | XVAL-01 through XVAL-03 pass |
| B8 | Fix all `SyncOptions` construction sites | `cargo build --workspace` | Compiles |
| B9 | Wire cross-cutting validation in `sync_stream()` | `cargo test -p ndp-lib -- gold::cross_cutting` | XVAL-04 through XVAL-09 pass |
| B10 | Wire `--no-validate` in CLI gold commands | `cargo build -p ndp-cli` | Compiles |
| B11 | Remove NoOpDbClient from CLI commands | `cargo build -p ndp-cli` | Compiles |
| B12 | Rename YAML configs | Static analysis checks | STATIC-14 through STATIC-20 pass |
| B13 | Final regression | `cargo test --workspace` | All tests pass |

### 8.3 Phase C: Integration Verification

After all unit tests pass:

1. Start integration stack
2. Run INT-01 through INT-08
3. Run deploy.sh end-to-end (INT-05)
4. Run static analysis (STATIC-01 through STATIC-22)

---

## 9. Test Matrix Summary

### 9.1 All Test Cases

| ID Range | Category | Count | Type |
|----------|----------|-------|------|
| CONST-01 to CONST-14 | Shared constants | 14 | Unit |
| XVAL-01 to XVAL-13 | Cross-cutting validation | 13 | Unit + CLI |
| NOOP-01 to NOOP-05 | NoOpDbClient | 5 | Unit |
| REG-01 to REG-13 | Regression | 13 | Regression |
| STATIC-01 to STATIC-22 | Static analysis | 22 | Grep |
| INT-01 to INT-08 | Integration | 8 | E2E |
| **Total** | | **75** | |

### 9.2 New Automated Tests (in Rust)

| Location | New Tests | Details |
|----------|-----------|---------|
| `crates/ndp-lib/src/constants.rs` | 8 | CONST-01 through CONST-08 |
| `crates/ndp-lib/src/db.rs` | 5 | NOOP-01 through NOOP-05 |
| `crates/ndp-lib/src/types.rs` | 3 | XVAL-01 through XVAL-03 |
| `crates/ndp-lib/src/gold/mod.rs` (or test file) | 6 | XVAL-04 through XVAL-09 |
| **Total new Rust tests** | **22** | |

### 9.3 Post-Phase 3 Test Counts

| Package | Before Phase 3 | New Tests | After Phase 3 |
|---------|---------------|-----------|---------------|
| ndp-lib | ~740 | 22 | ~762 |
| ndp-validate | 65 | 0 | 65 |
| ndp-gold-ddl | 15 | 0 | 15 |
| ndp-types | 88 | 0 | 88 |
| ndp-cli | 16 | 0 | 16 |
| **Total** | **~740** | **22** | **~762** |

---

## 10. Static Analysis Test Script

```bash
#!/bin/bash
# .test/phase3-static-analysis.sh
# Run from repository root after Phase 3 implementation

set -e
PASS=0
FAIL=0

check() {
    local id="$1"
    local desc="$2"
    local expected="$3"
    local actual="$4"

    if [ "$actual" = "$expected" ]; then
        echo "  PASS  $id: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $id: $desc (expected: $expected, got: $actual)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== STATIC-01 to STATIC-10: Constants Dedup ==="
check "STATIC-01" "No local VALID_METRICS in validate/semantic/gold" "0" \
    "$(grep -c 'const VALID_METRICS' crates/ndp-lib/src/validate/semantic/gold.rs 2>/dev/null || echo 0)"
check "STATIC-02" "No local VALID_STATS in validate/semantic/gold" "0" \
    "$(grep -c 'const VALID_STATS' crates/ndp-lib/src/validate/semantic/gold.rs 2>/dev/null || echo 0)"
check "STATIC-03" "No local VALID_METRICS in gold/config/types" "0" \
    "$(grep -c 'const VALID_METRICS' crates/ndp-lib/src/gold/config/types.rs 2>/dev/null || echo 0)"
check "STATIC-04" "No local VALID_ROLLING_STATS in gold/config/types" "0" \
    "$(grep -c 'const VALID_ROLLING_STATS' crates/ndp-lib/src/gold/config/types.rs 2>/dev/null || echo 0)"
check "STATIC-06" "Single VALID_METRICS definition" "1" \
    "$(grep -rn 'const VALID_METRICS' crates/ndp-lib/src/ | grep -c '=')"
check "STATIC-07" "Single VALID_ROLLING_STATS definition" "1" \
    "$(grep -rn 'const VALID_ROLLING_STATS' crates/ndp-lib/src/ | grep -c '=')"

echo ""
echo "=== STATIC-11 to STATIC-13: NoOpDbClient Dedup ==="
check "STATIC-11" "No NoOpDbClient struct in ndp-cli" "0" \
    "$(grep -rc 'struct NoOpDbClient' tools/ndp-cli/src/ 2>/dev/null || echo 0)"
check "STATIC-12" "No unreachable NoOpDbClient in ndp-cli" "0" \
    "$(grep -rc 'unreachable.*NoOpDbClient' tools/ndp-cli/src/ 2>/dev/null || echo 0)"

echo ""
echo "=== STATIC-14 to STATIC-20: YAML Cleanup ==="
check "STATIC-14" "No config.yaml in base streams" "0" \
    "$(find config/base/streams -name 'config.yaml' -type f 2>/dev/null | wc -l | tr -d ' ')"
check "STATIC-15" "No config.yaml in integration streams" "0" \
    "$(find config/integration/base/streams -name 'config.yaml' -type f 2>/dev/null | wc -l | tr -d ' ')"
check "STATIC-16" "7 backup files in base streams" "7" \
    "$(find config/base/streams -name 'config.yaml.bak' -type f 2>/dev/null | wc -l | tr -d ' ')"
check "STATIC-17" "3 backup files in integration streams" "3" \
    "$(find config/integration/base/streams -name 'config.yaml.bak' -type f 2>/dev/null | wc -l | tr -d ' ')"
check "STATIC-18" "platform.yaml preserved" "0" \
    "$(test -f config/base/platform.yaml && echo 0 || echo 1)"
check "STATIC-19" "No config.yaml references in ndp-lib" "0" \
    "$(grep -rc '"config.yaml"' crates/ndp-lib/src/ 2>/dev/null || echo 0)"

echo ""
echo "=== Results ==="
echo "PASS: $PASS  FAIL: $FAIL"
[ "$FAIL" -eq 0 ] && echo "All static checks passed." || echo "FAILURES DETECTED."
exit $FAIL
```

---

## 11. Acceptance Criteria Verification Matrix

### FR-01: Shared Constants

| Criterion | Test(s) | Pass Condition |
|-----------|---------|----------------|
| Constants defined once in `constants.rs` | STATIC-06 through STATIC-10, CONST-01 through CONST-08 | Single definition, correct values |
| No duplicate definitions | STATIC-01 through STATIC-05 | 0 local const declarations in old locations |
| Re-exports work (backward compat) | REG-01, REG-02 | All existing gold and validate tests pass |
| Both validators use shared constants | CONST-09 through CONST-14 | Same valid/invalid metrics accepted/rejected |

### FR-02: Cross-cutting Validation

| Criterion | Test(s) | Pass Condition |
|-----------|---------|----------------|
| SyncOptions default has validate=true | XVAL-01 | validate is true |
| sync_stream validates by default | XVAL-04, XVAL-05 | Valid config succeeds, invalid fails |
| --no-validate bypasses validation | XVAL-06, XVAL-10, XVAL-12, XVAL-13 | Invalid config generates DDL when validation off |
| Integration E2E | INT-01 through INT-05 | Deploy.sh apply succeeds |

### FR-04: NoOpDbClient Dedup

| Criterion | Test(s) | Pass Condition |
|-----------|---------|----------------|
| Single definition in ndp-lib | STATIC-11, STATIC-12 | 0 local definitions in ndp-cli |
| Trait conformance | NOOP-04, NOOP-05 | Implements DbClient + Send + Sync |
| Safe behavior | NOOP-01 through NOOP-03 | Returns Ok (not unreachable) |
| CLI dry-run works | INT-08 | All 3 dry-run commands succeed |

### FR-06: Retire Stale YAML

| Criterion | Test(s) | Pass Condition |
|-----------|---------|----------------|
| No active YAML stream configs | STATIC-14, STATIC-15 | 0 config.yaml files |
| Backups created | STATIC-16, STATIC-17 | 7 + 3 .yaml.bak files |
| platform.yaml preserved | STATIC-18 | File exists |
| No code references | STATIC-19, STATIC-20 | 0 code references |

---

## Appendix A: Risk Areas

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Re-export chain breaks `gold::config::VALID_METRICS` | Low | High (compilation failure) | Step B3 verifies with `cargo test -p ndp-lib -- gold` |
| `VALID_STATS` rename in validate tests | Low | Medium (test assertion text changes) | Tests reference constant by name; rename all at once |
| SyncOptions field addition breaks builds | Certain | Low (workspace-only) | Fix all 5 construction sites in step B8 |
| Cross-cutting validation rejects valid Pi configs | Medium | Medium | INT-05 (deploy.sh E2E) catches this |
| YAML rename misses integration env | Low | Low | STATIC-15 and STATIC-17 verify explicitly |

## Appendix B: Test File Locations

| Test Category | File Location |
|---------------|--------------|
| Constants unit tests | `crates/ndp-lib/src/constants.rs` (embedded `#[cfg(test)]`) |
| SyncOptions tests | `crates/ndp-lib/src/types.rs` (embedded `#[cfg(test)]`) |
| NoOpDbClient tests | `crates/ndp-lib/src/db.rs` (existing `#[cfg(test)]` module) |
| Cross-cutting validation tests | `crates/ndp-lib/src/gold/mod.rs` (new `#[cfg(test)]` section or `tests/gold_cross_cutting.rs`) |
| Static analysis | `.test/phase3-static-analysis.sh` (new script) |
| Integration tests | Manual execution per Section 7 |

## Appendix C: Dependency on Phase 1 + Phase 2

Phase 3 assumes both Phase 1 (v1.1.14) and Phase 2 (v1.1.17) are complete:

- `crates/ndp-lib/src/gold/` module exists with all Gold DDL code
- `crates/ndp-lib/src/validate/` module exists with all validation code
- `tools/ndp-gold-ddl/src/lib.rs` is a thin wrapper re-exporting from `ndp_lib::gold`
- `tools/ndp-validate/src/lib.rs` is a thin wrapper re-exporting from `ndp_lib::validate`
- deploy.sh uses `ndp` binary for all 7 dispatch sites
- `ndp_lib::db::NoOpDbClient` exists (added in Phase 1)
- `validate::semantic::is_valid_granularity()` exists in `semantic/mod.rs` (deduplicated in Phase 2)

If either phase is incomplete, Phase 3 cannot proceed.
