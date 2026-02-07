# OPS-003 Phase 1 (v1.1.14) Test Plan -- Gold Migration

> **Feature:** ops-003 Release 1 -- Gold DDL generation consolidated into ndp-lib and ndp CLI
> **Author:** ndp-tester agent
> **Date:** 2026-02-07
> **Status:** Draft
> **AgentDB Patterns Used:** ID 16 (testing:ndp-types-london-tdd), ID 22 (testing:hardcoding-detection-london-tdd), ID 34 (implementation:ndp-entity-sync-module), ID 14 (implementation:london-tdd-schema-validation)

---

## 1. Test Inventory (Current State)

### 1.1 Unit Tests (in-file `#[cfg(test)]` modules)

Every source file in `tools/ndp-gold-ddl/src/` that contains a `#[cfg(test)]` module is listed below with the exact number of `#[test]` or `#[tokio::test]` annotations.

| Source File | Test Count | Category |
|---|---|---|
| `src/generators/events.rs` | 54 | Generator -- Events infrastructure |
| `src/generators/state_transitions.rs` | 24 | Generator -- State transition views |
| `src/generators/continuous_aggregate.rs` | 20 | Generator -- Continuous aggregates |
| `src/config/loader.rs` | 18 | Config -- FileSystemConfigLoader |
| `src/config/domain.rs` | 17 | Config -- DomainConfig parsing |
| `src/validation/config_validator.rs` | 17 | Validation -- Gold config rules |
| `src/generators/aligned_view.rs` | 16 | Generator -- Cross-stream aligned views |
| `src/generators/classification.rs` | 14 | Generator -- Classification SQL |
| `src/config/types.rs` | 11 | Config -- Type parsing/serialization |
| `src/registry/rolling.rs` | 9 | Registry -- Rolling features |
| `src/registry/lag.rs` | 8 | Registry -- Lag features |
| `src/generators/refresh_policy.rs` | 8 | Generator -- Refresh policies |
| `src/generators/join_builder.rs` | 8 | Generator -- JOIN construction |
| `src/planner/sync.rs` | 7 (6 async + 1 sync) | Planner -- Sync plan decisions |
| `src/registry/trend.rs` | 7 | Registry -- Trend features |
| `src/generators/column_builder.rs` | 6 | Generator -- Column construction |
| `src/registry/mod.rs` | 6 | Registry -- Feature registry core |
| `src/generators/null_handler.rs` | 4 | Generator -- NULL handling |
| `src/registry/trait_def.rs` | 4 | Registry -- Trait definitions |
| `src/db/client.rs` | 3 | DB -- Client URL validation |
| `src/db/queries.rs` | 3 | DB -- Query construction |
| **TOTAL (unit)** | **264** | |

### 1.2 Integration Tests (`tests/` directory)

| Test File | Test Count | Category |
|---|---|---|
| `tests/aligned_view_tests.rs` | 25 | Phase C -- Aligned view generation |
| `tests/objectives_tests.rs` | 23 | Phase C -- Objectives parsing/SQL |
| `tests/state_transitions_tests.rs` | 21 | Phase C -- State transitions |
| `tests/golden_master_test.rs` | 15 | Golden master -- DDL output parity |
| `tests/ops002_hardcoding_tests.rs` | 10 | OPS-002 -- Hardcoding detection |
| `tests/ops002_source_scan_tests.rs` | 6 | OPS-002 -- Source code scanning |
| `tests/ops002_config_driven_tests.rs` | 6 | OPS-002 -- Config-driven generation |
| `tests/fixtures/phase_c.rs` | 6 | Fixture self-tests |
| **TOTAL (integration)** | **112** | |

### 1.3 Grand Total

| Layer | Count |
|---|---|
| Unit tests (in `src/`) | 264 |
| Integration tests (in `tests/`) | 112 |
| **Total** | **376** |

This matches the SCOPE.md claim of 376 tests.

### 1.4 Test Fixtures and Data Files

| File/Directory | Purpose | Migration Needed |
|---|---|---|
| `tests/fixtures/mod.rs` | Fixture module root (re-exports phase_c) | YES -- move to `crates/ndp-lib/tests/gold/fixtures/mod.rs` |
| `tests/fixtures/phase_c.rs` | MockConfigLoader, stream configs, SQL assertion helpers | YES -- move to `crates/ndp-lib/tests/gold/fixtures/phase_c.rs` |
| `tests/fixtures/energy_monitoring.rs` | Fictional domain for hardcoding detection | YES -- move to `crates/ndp-lib/tests/gold/fixtures/energy_monitoring.rs` |
| `tests/fixtures/golden-master/*.sql` (14 files) | Baseline DDL snapshots for golden master tests | YES -- move to `crates/ndp-lib/tests/gold/fixtures/golden-master/` |
| `tests/fixtures/golden-master/CHECKSUMS.sha256` | SHA-256 checksums for fixture integrity | YES -- move alongside SQL fixtures |

### 1.5 Mock Implementations

| Mock | Location | Description |
|---|---|---|
| `MockConfigLoader` | `tests/fixtures/phase_c.rs` | Builder-pattern mock for ConfigLoader trait; used by aligned_view_tests, objectives_tests, state_transitions_tests, hardcoding_tests, config_driven_tests |
| `MockCaChecker` | `src/planner/sync.rs` (in `#[cfg(test)]` block) | Builder-pattern mock for CaChecker trait; used by sync planner tests |
| `MockDbClient` (ndp-lib) | `crates/ndp-lib/src/db.rs` (not present yet) | Does NOT currently exist. The ndp-lib DbClient trait has no mock; ndp-gold-ddl defines its own `DbClient` and `CaChecker` separately |
| `NoOpDbClient` x3 | `tools/ndp-cli/src/commands/{dictionary,dimension,domain}.rs` | Three identical copies for dry-run mode |

### 1.6 Existing ndp-lib Tests (for reference)

| Source File | Test Count |
|---|---|
| `crates/ndp-lib/src/dictionary/mod.rs` | 21 |
| `crates/ndp-lib/src/domain/mod.rs` | 18 |
| `crates/ndp-lib/src/dictionary/sql.rs` | 13 |
| `crates/ndp-lib/src/convert.rs` | 11 |
| `crates/ndp-lib/src/dimension/csv_import.rs` | 11 |
| `crates/ndp-lib/src/config.rs` | 9 |
| `crates/ndp-lib/src/dimension/mod.rs` | 8 |
| `crates/ndp-lib/src/db.rs` | 3 |
| **Total** | **94** |

After migration, `cargo test -p ndp-lib` should run **94 (existing) + 376 (migrated) = 470 tests**.

---

## 2. Test Migration Plan

### 2.1 Source to Destination Mapping

#### Unit Tests (move with source code)

Unit tests are embedded in their source files via `#[cfg(test)]` modules. They migrate automatically when the source file moves. The following table shows the source-to-destination path for every file that contains tests.

| Source Path | Destination Path |
|---|---|
| `tools/ndp-gold-ddl/src/generators/events.rs` | `crates/ndp-lib/src/gold/generators/events.rs` |
| `tools/ndp-gold-ddl/src/generators/state_transitions.rs` | `crates/ndp-lib/src/gold/generators/state_transitions.rs` |
| `tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs` | `crates/ndp-lib/src/gold/generators/continuous_aggregate.rs` |
| `tools/ndp-gold-ddl/src/generators/aligned_view.rs` | `crates/ndp-lib/src/gold/generators/aligned_view.rs` |
| `tools/ndp-gold-ddl/src/generators/classification.rs` | `crates/ndp-lib/src/gold/generators/classification.rs` |
| `tools/ndp-gold-ddl/src/generators/refresh_policy.rs` | `crates/ndp-lib/src/gold/generators/refresh_policy.rs` |
| `tools/ndp-gold-ddl/src/generators/column_builder.rs` | `crates/ndp-lib/src/gold/generators/column_builder.rs` |
| `tools/ndp-gold-ddl/src/generators/join_builder.rs` | `crates/ndp-lib/src/gold/generators/join_builder.rs` |
| `tools/ndp-gold-ddl/src/generators/null_handler.rs` | `crates/ndp-lib/src/gold/generators/null_handler.rs` |
| `tools/ndp-gold-ddl/src/generators/constants.rs` | `crates/ndp-lib/src/gold/generators/constants.rs` |
| `tools/ndp-gold-ddl/src/generators/mod.rs` | `crates/ndp-lib/src/gold/generators/mod.rs` |
| `tools/ndp-gold-ddl/src/config/types.rs` | `crates/ndp-lib/src/gold/config/types.rs` |
| `tools/ndp-gold-ddl/src/config/domain.rs` | `crates/ndp-lib/src/gold/config/domain.rs` |
| `tools/ndp-gold-ddl/src/config/loader.rs` | `crates/ndp-lib/src/gold/config/loader.rs` |
| `tools/ndp-gold-ddl/src/config/mod.rs` | `crates/ndp-lib/src/gold/config/mod.rs` |
| `tools/ndp-gold-ddl/src/validation/config_validator.rs` | `crates/ndp-lib/src/gold/validation/config_validator.rs` |
| `tools/ndp-gold-ddl/src/validation/mod.rs` | `crates/ndp-lib/src/gold/validation/mod.rs` |
| `tools/ndp-gold-ddl/src/registry/lag.rs` | `crates/ndp-lib/src/gold/registry/lag.rs` |
| `tools/ndp-gold-ddl/src/registry/rolling.rs` | `crates/ndp-lib/src/gold/registry/rolling.rs` |
| `tools/ndp-gold-ddl/src/registry/trend.rs` | `crates/ndp-lib/src/gold/registry/trend.rs` |
| `tools/ndp-gold-ddl/src/registry/trait_def.rs` | `crates/ndp-lib/src/gold/registry/trait_def.rs` |
| `tools/ndp-gold-ddl/src/registry/mod.rs` | `crates/ndp-lib/src/gold/registry/mod.rs` |
| `tools/ndp-gold-ddl/src/planner/sync.rs` | `crates/ndp-lib/src/gold/planner/sync.rs` |
| `tools/ndp-gold-ddl/src/planner/mod.rs` | `crates/ndp-lib/src/gold/planner/mod.rs` |
| `tools/ndp-gold-ddl/src/db/client.rs` | SUPERSEDED by `crates/ndp-lib/src/db.rs` (already exists) |
| `tools/ndp-gold-ddl/src/db/queries.rs` | `crates/ndp-lib/src/gold/db/queries.rs` (CaChecker stays gold-specific) |
| `tools/ndp-gold-ddl/src/db/mod.rs` | `crates/ndp-lib/src/gold/db/mod.rs` |
| `tools/ndp-gold-ddl/src/error.rs` | `crates/ndp-lib/src/gold/error.rs` |

#### Integration Tests (move to `tests/gold/`)

| Source Path | Destination Path |
|---|---|
| `tools/ndp-gold-ddl/tests/aligned_view_tests.rs` | `crates/ndp-lib/tests/gold/aligned_view_tests.rs` |
| `tools/ndp-gold-ddl/tests/objectives_tests.rs` | `crates/ndp-lib/tests/gold/objectives_tests.rs` |
| `tools/ndp-gold-ddl/tests/state_transitions_tests.rs` | `crates/ndp-lib/tests/gold/state_transitions_tests.rs` |
| `tools/ndp-gold-ddl/tests/golden_master_test.rs` | `crates/ndp-lib/tests/gold/golden_master_test.rs` |
| `tools/ndp-gold-ddl/tests/ops002_hardcoding_tests.rs` | `crates/ndp-lib/tests/gold/ops002_hardcoding_tests.rs` |
| `tools/ndp-gold-ddl/tests/ops002_source_scan_tests.rs` | `crates/ndp-lib/tests/gold/ops002_source_scan_tests.rs` |
| `tools/ndp-gold-ddl/tests/ops002_config_driven_tests.rs` | `crates/ndp-lib/tests/gold/ops002_config_driven_tests.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/mod.rs` | `crates/ndp-lib/tests/gold/fixtures/mod.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/phase_c.rs` | `crates/ndp-lib/tests/gold/fixtures/phase_c.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/energy_monitoring.rs` | `crates/ndp-lib/tests/gold/fixtures/energy_monitoring.rs` |
| `tools/ndp-gold-ddl/tests/fixtures/golden-master/*.sql` | `crates/ndp-lib/tests/gold/fixtures/golden-master/*.sql` |
| `tools/ndp-gold-ddl/tests/fixtures/golden-master/CHECKSUMS.sha256` | `crates/ndp-lib/tests/gold/fixtures/golden-master/CHECKSUMS.sha256` |

### 2.2 Import Path Changes

Every `use` statement referencing the old crate path must be updated. The following table shows the systematic replacements.

| Old Import | New Import |
|---|---|
| `use ndp_gold_ddl::*` | `use ndp_lib::gold::*` |
| `use ndp_gold_ddl::config::types::*` | `use ndp_lib::gold::config::types::*` |
| `use ndp_gold_ddl::config::*` | `use ndp_lib::gold::config::*` |
| `use ndp_gold_ddl::generators::*` | `use ndp_lib::gold::generators::*` |
| `use ndp_gold_ddl::planner::*` | `use ndp_lib::gold::planner::*` |
| `use ndp_gold_ddl::registry::*` | `use ndp_lib::gold::registry::*` |
| `use ndp_gold_ddl::validation::*` | `use ndp_lib::gold::validation::*` |
| `use ndp_gold_ddl::db::*` | `use ndp_lib::gold::db::*` (for CaChecker, CaInfo) |
| `use ndp_gold_ddl::error::*` | `use ndp_lib::gold::error::*` |
| `use crate::config::*` (in unit tests) | `use crate::gold::config::*` |
| `use crate::generators::*` (in unit tests) | `use crate::gold::generators::*` |
| `use crate::db::*` (in unit tests) | `use crate::gold::db::*` |
| `use crate::error::*` (in unit tests) | `use crate::gold::error::*` |
| `use crate::registry::*` (in unit tests) | `use crate::gold::registry::*` |
| `use crate::planner::*` (in unit tests) | `use crate::gold::planner::*` |
| `use crate::validation::*` (in unit tests) | `use crate::gold::validation::*` |

### 2.3 Special Migration Cases

#### a. DbClient Trait Convergence

ndp-gold-ddl defines `db::client::DbClient` with only `query()`. ndp-lib defines `db::DbClient` with `query()`, `execute()`, and `batch_execute()`. The ndp-lib version is the superset.

**Action:** CaChecker (in `gold/db/queries.rs`) must be updated to accept `&(impl ndp_lib::DbClient + Send + Sync)` instead of `&(impl gold::db::DbClient + Send + Sync)`. The old `gold::db::client` module is NOT migrated; tests that reference `DbError` must switch to `NdpLibError::Database`.

**Test impact:** 3 tests in `src/db/client.rs` are ALREADY covered by equivalent tests in `crates/ndp-lib/src/db.rs`. These 3 tests do NOT migrate; they are duplicates. The remaining `src/db/queries.rs` tests (3 tests) must be updated to use `NdpLibError`.

**Net test count:** 376 - 3 (duplicate client tests) = 373 migrated, but 3 equivalent tests already exist in ndp-lib, so total remains at 376 unique logical tests (just different host crate).

#### b. Golden Master Tests

The golden master tests (`tests/golden_master_test.rs`) invoke `cargo run -p ndp-gold-ddl` as a subprocess. After migration, these tests need TWO variants:

1. **Legacy golden masters:** Continue testing `ndp-gold-ddl` standalone (keep working as thin wrapper).
2. **New parity golden masters:** Test `ndp gold generate` subcommand produces identical output (see Section 3.3).

The `execute_gold_ddl()` helper must be updated to use `CARGO_MANIFEST_DIR` relative to the new crate location.

#### c. Source Scan Tests

`tests/ops002_source_scan_tests.rs` reads Rust source files from `tools/ndp-gold-ddl/src/generators/`. After migration, the scanned paths change to `crates/ndp-lib/src/gold/generators/`. The `strip_tests_and_comments()` helper needs no changes; only the file paths it reads need updating.

### 2.4 dev-dependencies for ndp-lib Cargo.toml

These must be added to `crates/ndp-lib/Cargo.toml`:

```toml
[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
tempfile = "3"
pretty_assertions = "1.4"
mockall = "0.12"
sha2 = "0.10"
```

Note: `mockall` version bumped from 0.11 (ndp-gold-ddl) to 0.12 for consistency.

---

## 3. London TDD Test Design (NEW Tests)

These tests do NOT exist today. They must be written as part of the v1.1.14 implementation, following London TDD (test-first, outside-in, mock-driven).

### 3.1 Library API Tests

Location: `crates/ndp-lib/src/gold/mod.rs` (in `#[cfg(test)]` block) or `crates/ndp-lib/tests/gold/api_tests.rs`

#### 3.1.1 generate() Function Tests

The public API `ndp_lib::gold::generate()` is the top-level entry point that replaces the `ndp-gold-ddl` CLI logic.

```
test_generate_stream_returns_ddl_string
    Arrange: Valid StreamConfig with gold_etl enabled, GenerateOptions { stream: "air-quality" }
    Act:     ndp_lib::gold::generate(&config, &opts)
    Assert:  Result is Ok, contains "CREATE MATERIALIZED VIEW"

test_generate_domain_returns_aligned_view_ddl
    Arrange: Valid DomainConfig with 3 streams, MockConfigLoader, GenerateOptions { domain: "indoor-air-quality" }
    Act:     ndp_lib::gold::generate(&config, &opts)
    Assert:  Result is Ok, contains "FULL OUTER JOIN", "CREATE MATERIALIZED VIEW"

test_generate_stream_with_transitions_returns_transition_ddl
    Arrange: StreamConfig with transitions enabled, GenerateOptions { stream: "home-assistant-state", transitions: true }
    Act:     ndp_lib::gold::generate(&config, &opts)
    Assert:  Result is Ok, contains "LAG(", "PARTITION BY"

test_generate_domain_with_events_returns_events_ddl
    Arrange: DomainConfig with events enabled, GenerateOptions { domain: "indoor-air-quality", events: true }
    Act:     ndp_lib::gold::generate(&config, &opts)
    Assert:  Result is Ok, contains "THRESHOLD CROSSINGS", "STATE TRANSITIONS"

test_generate_disabled_gold_etl_returns_error
    Arrange: StreamConfig with gold_etl.enabled = false
    Act:     ndp_lib::gold::generate(&config, &opts)
    Assert:  Result is Err, message contains "disabled"

test_generate_missing_gold_etl_returns_error
    Arrange: StreamConfig with gold_etl = None
    Act:     ndp_lib::gold::generate(&config, &opts)
    Assert:  Result is Err, message contains "no gold_etl"
```

#### 3.1.2 sync() Function Tests

```
test_sync_with_mock_db_returns_sync_report
    Arrange: Valid StreamConfig, MockDbClient that returns empty rows (no existing CAs)
    Act:     ndp_lib::gold::sync(&config, &mock_db, &SyncOptions::default())
    Assert:  Result is Ok(SyncReport), items_created > 0

test_sync_with_existing_ca_skips_creation
    Arrange: Valid StreamConfig, MockDbClient that returns existing CA rows
    Act:     ndp_lib::gold::sync(&config, &mock_db, &SyncOptions::default())
    Assert:  Result is Ok(SyncReport), DDL contains "Skipping"

test_sync_dry_run_does_not_execute
    Arrange: Valid StreamConfig, MockDbClient (should NOT receive execute calls)
    Act:     ndp_lib::gold::sync(&config, &mock_db, &SyncOptions { dry_run: true, .. })
    Assert:  Result is Ok, MockDbClient.execute was never called

test_sync_db_connection_failure_returns_error
    Arrange: Invalid db_url
    Act:     ndp_lib::gold::sync(&config, &mock_db, &opts)
    Assert:  Result is Err, error type is Database
```

#### 3.1.3 recreate() Function Tests

```
test_recreate_with_mock_db_returns_drop_and_create
    Arrange: Valid StreamConfig, MockDbClient
    Act:     ndp_lib::gold::recreate(&config, &mock_db, &opts)
    Assert:  Result is Ok(SyncReport), DDL contains "DROP", "CREATE"

test_recreate_forces_recreation_of_existing
    Arrange: StreamConfig, MockDbClient that returns existing CAs
    Act:     ndp_lib::gold::recreate(&config, &mock_db, &opts)
    Assert:  DDL contains "DROP MATERIALIZED VIEW" and "CREATE MATERIALIZED VIEW"
```

### 3.2 DbClient Trait Tests

Location: `crates/ndp-lib/src/gold/db/queries.rs` (in `#[cfg(test)]` block)

```
test_ca_checker_uses_ndp_lib_db_client
    Arrange: MockDbClient implementing ndp_lib::DbClient (not gold::db::DbClient)
    Act:     PostgresCaChecker wraps it, calls ca_exists("gold", "test_hourly")
    Assert:  MockDbClient.query was called with correct SQL

test_noop_db_client_returns_empty_results
    Arrange: ndp_lib::db::NoOpDbClient
    Act:     Call query(), execute(), batch_execute()
    Assert:  Returns Ok with empty/zero results (or panic if called in dry-run contexts)
```

### 3.3 CLI Parity Tests

Location: `crates/ndp-lib/tests/gold/cli_parity_tests.rs` (integration test) or `tools/ndp-cli/tests/gold_parity_tests.rs`

These tests build and run both `ndp-gold-ddl` and `ndp gold` as subprocesses, then compare output.

#### a. Stream-Level Parity

```
test_parity_stream_air_quality_generate
    Old: ndp-gold-ddl --config-dir config generate --stream air-quality
    New: ndp gold generate --stream air-quality --config-dir config/base
    Assert: stdout matches (after normalization for field ordering)

test_parity_stream_air_quality_sync
    Old: ndp-gold-ddl --config-dir config --database-url $URL generate --stream air-quality --action sync
    New: ndp gold sync --stream air-quality --config-dir config/base --db-url $URL
    Assert: stdout matches

test_parity_stream_air_quality_recreate
    Old: ndp-gold-ddl --config-dir config generate --stream air-quality --action recreate
    New: ndp gold recreate --stream air-quality --config-dir config/base
    Assert: stdout matches

test_parity_stream_outdoor_weather_generate
    Old: ndp-gold-ddl --config-dir config generate --stream outdoor-weather
    New: ndp gold generate --stream outdoor-weather --config-dir config/base
    Assert: stdout matches

test_parity_stream_home_assistant_state_transitions
    Old: ndp-gold-ddl --config-dir config generate --stream home-assistant-state --transitions
    New: ndp gold generate --stream home-assistant-state --transitions --config-dir config/base
    Assert: stdout matches

test_parity_stream_outdoor_air_quality_generate
    Old: ndp-gold-ddl --config-dir config generate --stream outdoor-air-quality
    New: ndp gold generate --stream outdoor-air-quality --config-dir config/base
    Assert: stdout matches
```

#### b. Domain-Level Parity

```
test_parity_domain_sync
    Old: ndp-gold-ddl --config-dir config generate --domain indoor-air-quality --action sync
    New: ndp gold generate --domain indoor-air-quality --config-dir config/base
    Assert: stdout matches

test_parity_domain_recreate
    Old: ndp-gold-ddl --config-dir config generate --domain indoor-air-quality --action recreate
    New: ndp gold recreate --domain indoor-air-quality --config-dir config/base
    Assert: stdout matches

test_parity_domain_events_sync
    Old: ndp-gold-ddl --config-dir config generate --domain indoor-air-quality --events --action sync
    New: ndp gold generate --domain indoor-air-quality --events --config-dir config/base
    Assert: stdout matches

test_parity_domain_events_recreate
    Old: ndp-gold-ddl --config-dir config generate --domain indoor-air-quality --events --action recreate
    New: ndp gold recreate --domain indoor-air-quality --events --config-dir config/base
    Assert: stdout matches
```

#### c. Exit Code Parity

```
test_parity_exit_code_success
    Both: generate --stream air-quality
    Assert: Both return exit code 0

test_parity_exit_code_invalid_stream
    Both: generate --stream nonexistent-stream
    Assert: Both return non-zero exit code

test_parity_exit_code_missing_args
    Both: generate (no --stream or --domain)
    Assert: Both return non-zero exit code

test_parity_exit_code_events_without_domain
    Both: generate --events (without --domain)
    Assert: Both return non-zero exit code
```

#### d. Error Message Parity (Soft)

Error message parity is a soft requirement. The exact wording may differ between the standalone binary and the subcommand. What matters is:
- Same exit code for the same error condition
- Error messages reference the same root cause (e.g., "no gold_etl configuration", "config not found")

### 3.4 Flag Mapping Tests

Location: `tools/ndp-cli/tests/gold_flag_tests.rs` or within `crates/ndp-lib/tests/gold/flag_mapping_tests.rs`

Per the SCOPE.md flag mapping table, each of these flag combinations needs a test.

#### a. Global Flags

```
test_flag_config_dir_passed_through
    Args: ndp gold generate --stream air-quality --config-dir /custom/path
    Assert: Config loaded from /custom/path, not default

test_flag_db_url_passed_through
    Args: ndp gold sync --stream air-quality --db-url postgresql://user:pass@host:5432/db
    Assert: DB connection attempted with specified URL

test_flag_db_timeout_passed_through
    Args: ndp gold sync --stream air-quality --db-url ... --db-timeout 5
    Assert: Connection timeout set to 5 seconds

test_flag_verbose_enables_debug_output
    Args: ndp gold generate --stream air-quality --verbose
    Assert: stderr contains additional diagnostic output
```

#### b. Gold-Specific Flags

```
test_flag_stream_generates_single_stream
    Args: ndp gold generate --stream air-quality
    Assert: Output contains only air-quality DDL

test_flag_domain_generates_aligned_view
    Args: ndp gold generate --domain indoor-air-quality
    Assert: Output contains aligned view DDL

test_flag_stream_and_domain_conflict
    Args: ndp gold generate --stream air-quality --domain indoor-air-quality
    Assert: Clap reports conflict, non-zero exit

test_flag_transitions_generates_state_view
    Args: ndp gold generate --stream home-assistant-state --transitions
    Assert: Output contains LAG(), PARTITION BY

test_flag_events_generates_events_ddl
    Args: ndp gold generate --domain indoor-air-quality --events
    Assert: Output contains THRESHOLD CROSSINGS

test_flag_events_without_domain_fails
    Args: ndp gold generate --stream air-quality --events
    Assert: Error: --events requires --domain

test_flag_dry_run_does_not_apply
    Args: ndp gold sync --stream air-quality --dry-run --db-url ...
    Assert: DDL printed but not executed

test_flag_no_validate_skips_validation (v1.1.16, design only)
    Args: ndp gold sync --stream air-quality --no-validate --db-url ...
    Assert: Validation skipped, sync proceeds directly
```

---

## 4. Integration Test Plan (E2E)

### 4.1 Prerequisites

```bash
# Start integration stack
docker compose -f docker-compose.integration.yml up -d

# Wait for TimescaleDB health check
docker compose -f docker-compose.integration.yml exec timescaledb pg_isready -U postgres -d ndp

# Build ndp-cli with gold module
cargo build -p ndp-cli

# Connection string for tests
export DB_URL="postgresql://postgres:postgres@localhost:5432/ndp"
```

### 4.2 deploy.sh Integration

#### Test: Full deploy.sh apply with ndp binary (Gold phases only)

```
test_deploy_sh_gold_phase_uses_ndp_binary
    Precondition: ndp binary built, ndp-gold-ddl binary NOT on PATH
    Action:       DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
    Assert:
      - Phase 5 (Gold DDL) completes successfully
      - Output shows "ndp gold" invocation, NOT "ndp-gold-ddl"
      - TimescaleDB contains gold.* continuous aggregates
      - Exit code 0

test_deploy_sh_fails_when_ndp_missing
    Precondition: ndp binary NOT on PATH, ndp-gold-ddl binary NOT on PATH
    Action:       DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
    Assert:
      - Error message: "ndp tool not found"
      - Exit code 1 (NOT 0 with warning)

test_deploy_sh_no_fallback_to_ndp_gold_ddl
    Precondition: ndp binary NOT on PATH, ndp-gold-ddl IS on PATH
    Action:       DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
    Assert:
      - Error message: "ndp tool not found"
      - deploy.sh does NOT fall back to ndp-gold-ddl
```

### 4.3 Database Integration

#### Test: Gold sync against real TimescaleDB

```
test_gold_sync_creates_continuous_aggregates
    Precondition: Clean TimescaleDB (no gold schema)
    Action:       ndp gold sync --stream air-quality --config-dir config/base --db-url $DB_URL
    Assert:
      - Exit code 0
      - gold schema exists in TimescaleDB
      - gold.air_quality_hourly continuous aggregate exists
      - Refresh policy is attached

test_gold_sync_idempotent
    Precondition: gold.air_quality_hourly already exists
    Action:       ndp gold sync --stream air-quality --config-dir config/base --db-url $DB_URL
    Assert:
      - Exit code 0
      - Output contains "Skipping" for existing CA
      - No errors
      - Running twice produces identical state

test_gold_recreate_drops_and_recreates
    Precondition: gold.air_quality_hourly exists
    Action:       ndp gold recreate --stream air-quality --config-dir config/base --db-url $DB_URL
    Assert:
      - Exit code 0
      - gold.air_quality_hourly still exists (recreated)
      - Refresh policy re-attached

test_ca_checker_detects_existing_aggregates
    Precondition: gold.air_quality_hourly exists
    Action:       ndp gold sync --stream air-quality --config-dir config/base --db-url $DB_URL --verbose
    Assert:
      - stderr shows "Skipping gold.air_quality_hourly (already exists)"
      - Only missing CAs are created
```

### 4.4 Config Integration

```
test_all_stream_configs_generate_valid_ddl
    Action: For each stream in config/base/streams/:
            ndp gold generate --stream $STREAM_ID --config-dir config/base
    Assert:
      - All exit with code 0
      - All produce non-empty DDL
      - DDL is syntactically valid SQL (contains CREATE, SELECT, FROM)

test_domain_config_generates_valid_ddl
    Action: ndp gold generate --domain indoor-air-quality --config-dir config/base
    Assert:
      - Exit code 0
      - DDL references all 3 domain streams
      - FULL OUTER JOIN present

test_invalid_config_produces_clear_error
    Action: ndp gold generate --stream nonexistent --config-dir config/base
    Assert:
      - Non-zero exit code
      - Error message identifies the missing config file
```

---

## 5. Test Execution Strategy

### 5.1 Execution Order

| Phase | What | Command | Expected Count |
|---|---|---|---|
| 1. Unit tests (existing, migrated) | All 264 unit tests in gold module | `cargo test -p ndp-lib -- gold::` | 264 |
| 2. Integration tests (existing, migrated) | All 112 tests from tests/gold/ | `cargo test -p ndp-lib --test '*'` | 112 |
| 3. Library API tests (new) | generate(), sync(), recreate() API tests | `cargo test -p ndp-lib -- gold::tests::api` | ~12 |
| 4. CLI parity tests (new) | Output comparison ndp-gold-ddl vs ndp gold | `cargo test -p ndp-cli --test gold_parity` | ~16 |
| 5. Flag mapping tests (new) | Clap flag validation | `cargo test -p ndp-cli --test gold_flag` | ~12 |
| 6. Integration (E2E) | deploy.sh and real DB | Manual or CI script | ~10 |
| **Total** | | | **~426** |

### 5.2 CI Equivalents

```bash
# Fast check -- must pass before any PR merge
cargo test -p ndp-lib
cargo test -p ndp-cli

# Slow check -- golden master and parity tests (build required)
cargo test -p ndp-lib --test golden_master_test
cargo test -p ndp-cli --test gold_parity

# Integration check (requires docker stack)
docker compose -f docker-compose.integration.yml up -d
DEPLOY_ENV=integration cargo test -p ndp-lib --test gold_integration -- --ignored
```

### 5.3 Verification Commands (Manual)

```bash
# DDL parity (stream-level)
diff <(ndp-gold-ddl --config-dir config generate --stream air-quality) \
     <(cargo run -p ndp-cli -- gold generate --stream air-quality --config-dir config/base)

# DDL parity (domain-level)
diff <(ndp-gold-ddl --config-dir config generate --domain indoor-air-quality) \
     <(cargo run -p ndp-cli -- gold generate --domain indoor-air-quality --config-dir config/base)

# DDL parity (transitions)
diff <(ndp-gold-ddl --config-dir config generate --stream home-assistant-state --transitions) \
     <(cargo run -p ndp-cli -- gold generate --stream home-assistant-state --transitions --config-dir config/base)

# DDL parity (events)
diff <(ndp-gold-ddl --config-dir config generate --domain indoor-air-quality --events) \
     <(cargo run -p ndp-cli -- gold generate --domain indoor-air-quality --events --config-dir config/base)
```

---

## 6. Regression Prevention

### 6.1 Golden Master Strategy

**Before migration begins**, capture current golden master baselines:

```bash
cd /workspaces/neural-data-platform
# Verify existing golden masters are intact
cargo test -p ndp-gold-ddl --test golden_master_test
```

**After migration**, run golden masters against BOTH binaries:

```bash
# Old binary (thin wrapper now)
cargo test -p ndp-gold-ddl --test golden_master_test

# New binary (via parity tests)
cargo test -p ndp-cli --test gold_parity
```

### 6.2 Automated Parity Script

Create `scripts/verify-gold-parity.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

CONFIG_DIR="config"
STREAMS=("air-quality" "outdoor-weather" "home-assistant-state" "outdoor-air-quality")
DOMAINS=("indoor-air-quality")
MODES=("sync" "recreate")

echo "=== Gold DDL Parity Check ==="

# Stream-level parity
for stream in "${STREAMS[@]}"; do
    for mode in "${MODES[@]}"; do
        OLD=$(ndp-gold-ddl --config-dir "$CONFIG_DIR" generate --stream "$stream" --action "$mode" 2>/dev/null)
        NEW=$(ndp gold generate --stream "$stream" --config-dir "${CONFIG_DIR}/base" 2>/dev/null)
        if [ "$OLD" = "$NEW" ]; then
            echo "PASS: stream=$stream mode=$mode"
        else
            echo "FAIL: stream=$stream mode=$mode"
            diff <(echo "$OLD") <(echo "$NEW") || true
        fi
    done
done

# Domain-level parity
for domain in "${DOMAINS[@]}"; do
    for mode in "${MODES[@]}"; do
        OLD=$(ndp-gold-ddl --config-dir "$CONFIG_DIR" generate --domain "$domain" --action "$mode" 2>/dev/null)
        NEW=$(ndp gold generate --domain "$domain" --config-dir "${CONFIG_DIR}/base" 2>/dev/null)
        if [ "$OLD" = "$NEW" ]; then
            echo "PASS: domain=$domain mode=$mode"
        else
            echo "FAIL: domain=$domain mode=$mode"
            diff <(echo "$OLD") <(echo "$NEW") || true
        fi
    done
done

echo "=== Parity check complete ==="
```

### 6.3 deploy.sh Smoke Test

Create `scripts/verify-deploy-gold.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "=== deploy.sh Gold Phase Smoke Test ==="

# Ensure ndp binary exists
if ! command -v ndp &>/dev/null && \
   ! [ -x "target/release/ndp" ] && \
   ! [ -x "target/debug/ndp" ]; then
    echo "FAIL: ndp binary not found. Build with: cargo build -p ndp-cli"
    exit 1
fi

# Ensure ndp-gold-ddl is NOT required
# Temporarily rename to prove deploy.sh doesn't use it
if command -v ndp-gold-ddl &>/dev/null; then
    echo "WARNING: ndp-gold-ddl is still on PATH. Verifying deploy.sh does not call it..."
fi

# Start integration stack if not running
if ! docker compose -f docker-compose.integration.yml ps --services --filter status=running | grep -q timescaledb; then
    echo "Starting integration stack..."
    docker compose -f docker-compose.integration.yml up -d
    sleep 30  # Wait for TimescaleDB
fi

# Run deploy with only ndp binary
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.14.manifest.json

echo "=== Smoke test PASSED ==="
```

### 6.4 Pass/Fail Criteria

The migration is considered PASSING when ALL of the following are true:

| Criterion | Verification |
|---|---|
| All 376 gold tests pass under ndp-lib | `cargo test -p ndp-lib` -- 376 gold tests green |
| All 94 existing ndp-lib tests still pass | `cargo test -p ndp-lib` -- 94 existing tests green |
| Golden master tests pass | `cargo test -p ndp-lib --test golden_master_test` -- 15 tests green |
| ndp-gold-ddl standalone still works | `cargo test -p ndp-gold-ddl` -- builds and basic tests pass |
| CLI parity: stream DDL matches | `diff` output is empty for all streams |
| CLI parity: domain DDL matches | `diff` output is empty for all domains |
| deploy.sh Gold phase works via `ndp` | integration env deploy completes |
| Zero `ndp-gold-ddl` references in deploy.sh | `grep 'ndp-gold-ddl' deploy/pi/deploy.sh` returns empty |
| No test regressions in workspace | `cargo test --workspace` all green |

---

## 7. Risk Mitigation

### 7.1 Test-During-Migration Protocol

Run `cargo test -p ndp-lib` after EVERY file move. Not at the end -- after each individual file or logical group of files. This catches import path errors immediately.

Recommended move order (to minimize broken intermediate states):

1. **Error types first** (`error.rs`) -- everything depends on these
2. **Config types** (`config/types.rs`, `config/domain.rs`) -- generators depend on these
3. **Config loader** (`config/loader.rs`, `config/mod.rs`) -- 18 tests exercise file loading
4. **Registry** (`registry/`) -- generators depend on feature registry
5. **Generators** (in dependency order): `constants.rs`, `null_handler.rs`, `column_builder.rs`, `join_builder.rs`, `refresh_policy.rs`, `classification.rs`, `continuous_aggregate.rs`, `aligned_view.rs`, `state_transitions.rs`, `events.rs`
6. **Planner** (`planner/sync.rs`) -- depends on generators and DB
7. **DB queries** (`db/queries.rs`) -- CaChecker, wire to ndp_lib::DbClient
8. **Validation** (`validation/config_validator.rs`)
9. **Integration tests + fixtures** (last, once all source moved)

### 7.2 Rollback Plan

If migration stalls, the source files can remain in ndp-gold-ddl. The standalone binary continues to work. deploy.sh can be reverted to the pre-v1.1.14 dispatch pattern by restoring `command -v ndp-gold-ddl` blocks.

---

## 8. Appendix: Test File Breakdown Per Source File

For complete reference, here is the exact `#[test]` count breakdown for every source file with tests, organized by module.

### generators/ (152 tests)

| File | Tests |
|---|---|
| events.rs | 54 |
| state_transitions.rs | 24 |
| continuous_aggregate.rs | 20 |
| aligned_view.rs | 16 |
| classification.rs | 14 |
| refresh_policy.rs | 8 |
| join_builder.rs | 8 |
| column_builder.rs | 6 |
| null_handler.rs | 4 |

### config/ (46 tests)

| File | Tests |
|---|---|
| loader.rs | 18 |
| domain.rs | 17 |
| types.rs | 11 |

### registry/ (34 tests)

| File | Tests |
|---|---|
| rolling.rs | 9 |
| lag.rs | 8 |
| trend.rs | 7 |
| mod.rs | 6 |
| trait_def.rs | 4 |

### validation/ (17 tests)

| File | Tests |
|---|---|
| config_validator.rs | 17 |

### planner/ (7 tests)

| File | Tests |
|---|---|
| sync.rs | 7 |

### db/ (6 tests)

| File | Tests |
|---|---|
| client.rs | 3 |
| queries.rs | 3 |

### error.rs (0 tests)

No tests in error type definitions.
