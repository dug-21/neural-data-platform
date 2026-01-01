# AIR-010 Test Cleanup Analysis

## Summary

During AIR-010 optimization, several components and dependencies were removed. This document identifies test files that reference removed components and recommends cleanup actions.

## Components Removed During AIR-010

| Component | Location | Reason |
|-----------|----------|--------|
| `config-store` crate | Moved to `archive/legacy-config-store/` | Not used by NDP air quality features |
| `sqlx` dependency | Removed from workspace | Not used by current Bronze layer |
| `duckdb` dependency | Never added (commented out) | Optional Silver layer feature |
| `parquet` crate | Commented out in dev-deps | Using `polars` for Parquet I/O instead |

---

## Test Files Requiring Cleanup

### Category 1: Legacy `autonomous_platform` Tests (REMOVE ENTIRE DIRECTORY)

The `/tests/` root directory contains **87+ test files** from a previous project iteration (trading platform). These reference `autonomous_platform` module which does not exist in the current workspace.

**Recommendation: Archive entire `/tests/` directory to `/archive/legacy-tests/`**

Files affected include:
- `tests/unit/*.rs` (all files)
- `tests/integration/*.rs` (all files)
- `tests/performance/*.rs` (all files)
- `tests/mcp_integration/*.rs` (all files)
- `tests/orchestrator/*.rs` (all files)
- `tests/phase3/*.rs` (all files)
- `tests/common/*.rs`
- `tests/*.rs` (root level test files)

These tests are not part of the NDP air quality project and reference non-existent modules.

---

### Category 2: Config-Store Tests (REMOVE)

**Path:** `tests/components/config_store/`

**Status:** Standalone test package with own Cargo.toml. References config-store which was archived.

**Files:**
- `tests/components/config_store/mod.rs`
- `tests/components/config_store/test_config_api.rs`
- `tests/components/config_store/test_distributed_sync.rs`
- `tests/components/config_store/test_hot_reload.rs`
- `tests/components/config_store/test_model_storage.rs`
- `tests/components/config_store/test_security.rs`
- `tests/components/config_store/run_tests.rs`

**Recommendation:** Archive to `archive/legacy-config-store/tests/component-tests/`

---

### Category 3: DuckDB Tests (FEATURE-GATE or REMOVE)

**Path:** `apps/air-quality-app/tests/`

**Status:** Requires `duckdb` crate which is not installed.

**Files:**
- `apps/air-quality-app/tests/duckdb_views_test.rs` - Partially feature-gated
- `apps/air-quality-app/tests/silver_layer_integration_test.rs` - NOT gated, uses duckdb+parquet

**Options:**
1. **Feature-gate** with `#[cfg(feature = "duckdb-tests")]` - Allows tests when DuckDB is installed
2. **Remove** - If Silver layer DuckDB implementation is not planned

**Recommendation:** Add complete feature gates. These tests will be useful when DP-001 Silver Layer is implemented.

---

### Category 4: SQLx Tests (REMOVE)

**Path:** `tests/unit/` and `tests/`

**Status:** Reference `sqlx` which was removed from workspace dependencies.

**Files:**
- `tests/unit/timescale_adapter_test.rs` - Uses `sqlx::{postgres::PgPoolOptions, Pool, Postgres}`
- `tests/timescale_adapter_standalone_test.rs` - Uses `sqlx::{postgres::PgPoolOptions, Pool, Postgres}`

**Recommendation:** Archive to `archive/legacy-tests/`. These tests are for TimescaleDB adapter which is not in current scope.

---

### Category 5: Emergency Tests (KEEP - STANDALONE)

**Path:** `tests/emergency/`

**Status:** Standalone package with own `Cargo.toml` defining its own dependencies including `sqlx`.

**Files:**
- `tests/emergency/test_*.rs` (all files)
- `tests/emergency/Cargo.toml`

**Recommendation:** KEEP as-is. This is a standalone test package for emergency/production validation that manages its own dependencies.

---

## Current Workspace Tests (KEEP)

These test files are part of the current NDP air quality project and should be kept:

### Core Library Tests
- `core/tests/config_driven_suite.rs` - Config-driven parsing tests
- `core/tests/nws_integration.rs` - NWS weather integration
- `core/tests/nws_config_compatibility_test.rs` - NWS config tests
- `core/tests/parser_integration_test.rs` - Parser tests
- `core/tests/weather_polling_integration.rs` - Weather polling tests
- `core/tests/fixtures/*.rs` - Test fixtures

### Air Quality App Tests
- `apps/air-quality-app/tests/integration_test.rs` - App integration tests
- `apps/air-quality-app/tests/mqtt_routing_integration_test.rs` - MQTT routing tests
- `apps/air-quality-app/src/**/*_test.rs` - Inline unit tests

### Config Client Tests
- `config-client/tests/integration_test.rs` - etcd client tests

### Domain Tests
- `domains/air-quality/examples/test_aqi_alerts.rs` - AQI alert examples

---

## Recommended Cleanup Actions

### Immediate (Required for Build)

1. **Feature-gate DuckDB tests** in `apps/air-quality-app/tests/`:
   ```rust
   #![cfg(feature = "duckdb-tests")]
   ```

### Short-term (Recommended)

2. **Archive legacy tests directory:**
   ```bash
   mkdir -p archive/legacy-tests
   mv tests/* archive/legacy-tests/
   # Keep only: tests/emergency/ (move back after)
   ```

3. **Archive config-store component tests:**
   ```bash
   mv tests/components/config_store archive/legacy-config-store/component-tests/
   ```

### Verification Commands

After cleanup, verify build and tests pass:
```bash
cargo check
cargo test --workspace
```

---

## Test Count Summary

| Category | Files | Action |
|----------|-------|--------|
| Legacy `autonomous_platform` tests | 87+ | Archive |
| Config-store component tests | 7 | Archive |
| DuckDB/Parquet tests | 2 | Feature-gate |
| SQLx TimescaleDB tests | 2 | Archive |
| Emergency standalone tests | 7 | Keep |
| **Current NDP tests** | **~15** | **Keep** |

---

## Notes

- The legacy tests directory structure suggests this repo was previously used for a trading/financial platform project
- The current NDP (Neural Data Platform) focuses on air quality monitoring with a different architecture
- Test cleanup will significantly reduce build times and eliminate false compilation errors
- Feature-gated tests allow future reactivation when dependencies are added

---

*Generated during AIR-010 optimization phase*
*Date: 2026-01-01*
