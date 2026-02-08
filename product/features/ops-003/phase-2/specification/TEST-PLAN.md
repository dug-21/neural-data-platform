# OPS-003 Phase 2 (v1.1.15) Test Plan -- Validate Migration

> **Feature:** ops-003 Phase 2 -- ndp-validate consolidated into ndp-lib and ndp CLI
> **Author:** ndp-tester agent
> **Date:** 2026-02-07
> **Status:** Draft
> **AgentDB Patterns Used:** ID 16 (testing:ndp-types-london-tdd), ID 33 (procedure:integration-environment), ID 34 (implementation:ndp-entity-sync-module), ID 37 (convention:ndp-config-dir), ID 38 (testing:integration-e2e-checklist)

### Revision History

| Rev | Date | Author | Changes |
|-----|------|--------|---------|
| R1 | 2026-02-08 | ndp-tester | D1: CLI commands changed from entity/verb subcommands to flat flags per CLI UX Design (10-CLI-UX-DESIGN-REVISED.md lines 193-203, 388-395). All `ndp validate stream`, `ndp validate domain`, `ndp validate schema` subcommand forms replaced with `--stream`, `--domain`, `--schema` flags. D2: Confirmed no YAML-specific test cases exist (serde_yaml already absent from test plan; no changes needed). D3: Added UNIT-NEW-16 through UNIT-NEW-19 for `is_valid_granularity()` deduplication coverage. D4: Schema operations updated from `schema generate`/`schema verify` to `--schema --generate`/`--schema --verify`. New unit test count: 15 -> 19, total new/migrated: 322 -> 326. |

---

## 1. Test Strategy Overview

### 1.1 Testing Layers

| Layer | Purpose | Count Target | Speed |
|---|---|---|---|
| **Unit Tests** | Verify individual functions and modules in isolation | 222+ migrated, ~34 new | < 30s |
| **CLI Parity Tests** | Prove `ndp validate` produces identical output to `ndp-validate` | ~55 new | < 60s |
| **deploy.sh Integration** | Prove deploy.sh works with new `ndp` binary at both dispatch sites | ~12 new | < 30s |
| **E2E Integration** | Validate real configs against live TimescaleDB | ~10 new | < 120s |
| **Golden Master** | Frozen output comparison to catch silent regressions | ~8 new | < 30s |

### 1.2 Phase 1 Lessons Applied

Phase 1 (Gold migration, v1.1.14) uncovered three bugs during integration testing that unit tests alone missed. Every lesson below is codified as a specific test in this plan.

| Phase 1 Bug | Root Cause | Test That Prevents Recurrence |
|---|---|---|
| **BUG-004**: `--config-dir` path wrong | deploy.sh passed wrong base path to `ndp gold` | Section 4: DEPLOY-INT-01 through DEPLOY-INT-04 test real paths |
| **Parity gap**: `--events` + `--stream` silently ignored | Flag captured but not checked for conflicts | Section 3: PAR-INV-* tests EVERY invalid flag combination |
| **Parity gap**: `--validate-only` captured but unused | Flag parsed but never branched on | Section 3: PAR-FLAG-* tests verify each flag produces different behavior |
| **Golden master**: 15 DDL comparisons caught regressions | DDL output changed subtly | Section 6: GM-* tests for validate output |

### 1.3 Test Methodology

All tests follow London TDD (pattern ID 16):
- **Arrange-Act-Assert** structure
- **Behavior verification** over implementation testing
- **Test naming**: `test_<function>_<scenario>_<expected>`
- **Mocks** for external dependencies (filesystem, database, network)

---

## 2. Unit Test Migration (ops-003-05)

### 2.1 Current ndp-validate Test Inventory

Actual test count from `cargo test -p ndp-validate -- --list`: **222 tests** (including 5 doc tests).

#### File-by-File Breakdown

| Source File | Test Count | Category |
|---|---|---|
| `src/cli.rs` | 65 | CLI parsing, output formatting, exit codes |
| `src/schema.rs` | 45 | JSON Schema validation (stream + domain) |
| `src/semantic/dq_rules.rs` | 22 | DQ rule syntax, column refs, actions |
| `src/semantic/sources.rs` | 18 | Source config validation (mqtt, http, csv, etc.) |
| `src/semantic/source_path.rs` | 15 | Source path cross-reference, Levenshtein |
| `src/semantic/gold.rs` | 13 | Gold ETL config validation |
| `src/semantic/domain.rs` | 13 | Domain semantic validation |
| `src/semantic/table_exists.rs` | 11 | Table existence checks |
| `src/schema_gen.rs` | 10 | Schema generation and drift detection |
| `src/error.rs` | 5 | Error types, severity mapping |
| **Doc tests** | 5 | Inline code examples |
| **TOTAL** | **222** | |

### 2.2 Migration Path: Source to Destination

Unit tests are embedded via `#[cfg(test)]` and migrate with their source files.

| Source Path | Destination Path | Tests |
|---|---|---|
| `tools/ndp-validate/src/error.rs` | `crates/ndp-lib/src/validate/error.rs` | 5 |
| `tools/ndp-validate/src/schema.rs` | `crates/ndp-lib/src/validate/schema.rs` | 45 |
| `tools/ndp-validate/src/schema_gen.rs` | `crates/ndp-lib/src/validate/schema_gen.rs` | 10 |
| `tools/ndp-validate/src/semantic/mod.rs` | `crates/ndp-lib/src/validate/semantic/mod.rs` | 0 |
| `tools/ndp-validate/src/semantic/sources.rs` | `crates/ndp-lib/src/validate/semantic/sources.rs` | 18 |
| `tools/ndp-validate/src/semantic/source_path.rs` | `crates/ndp-lib/src/validate/semantic/source_path.rs` | 15 |
| `tools/ndp-validate/src/semantic/table_exists.rs` | `crates/ndp-lib/src/validate/semantic/table_exists.rs` | 11 |
| `tools/ndp-validate/src/semantic/dq_rules.rs` | `crates/ndp-lib/src/validate/semantic/dq_rules.rs` | 22 |
| `tools/ndp-validate/src/semantic/gold.rs` | `crates/ndp-lib/src/validate/semantic/gold.rs` | 13 |
| `tools/ndp-validate/src/semantic/domain.rs` | `crates/ndp-lib/src/validate/semantic/domain.rs` | 13 |

#### CLI and output formatting stay in ndp-validate thin wrapper

| Source Path | Stays In | Tests |
|---|---|---|
| `tools/ndp-validate/src/cli.rs` | `tools/ndp-validate/src/cli.rs` (re-exports from ndp-lib) | 65 |
| `tools/ndp-validate/src/main.rs` | `tools/ndp-validate/src/main.rs` (thin binary) | 0 |

The 65 CLI tests remain in ndp-validate because they test clap argument parsing specific to the `ndp-validate` binary. The ndp CLI will have its own parallel set of clap tests (see Section 3).

### 2.3 Import Path Changes

Every `use` statement referencing the old crate path must be updated systematically.

| Old Import | New Import |
|---|---|
| `use crate::error::*` | `use crate::validate::error::*` |
| `use crate::schema::*` | `use crate::validate::schema::*` |
| `use crate::schema_gen::*` | `use crate::validate::schema_gen::*` |
| `use crate::semantic::*` | `use crate::validate::semantic::*` |
| `use ndp_validate::*` (in ndp-validate tests) | `use ndp_lib::validate::*` (re-exported) |
| `use ndp_validate::cli::*` (in main.rs) | Stays as-is (cli stays in ndp-validate) |
| `use ndp_validate::schema::*` (external) | `use ndp_lib::validate::schema::*` |
| `use ndp_validate::semantic::*` (external) | `use ndp_lib::validate::semantic::*` |

### 2.4 Expected Pass Rate After Migration

| Milestone | Expected | Acceptable |
|---|---|---|
| Immediate after file move | 0% (compilation errors) | Expected |
| After import path fixes | 95%+ | >= 90% |
| After dependency fixes | 100% | >= 99% (allow flaky doc tests) |
| Final | 222/222 (100%) | 100% -- no test regression allowed |

### 2.5 New Unit Tests for ndp-lib Validate API Surface

The migration creates new public API surface in `ndp_lib::validate::*` that needs coverage.

| Test ID | Function Under Test | Scenario | Expected |
|---|---|---|---|
| UNIT-NEW-01 | `ndp_lib::validate::validate_stream_config()` | Valid JSON config string | Returns `Ok(ValidationResult)` with `valid: true` |
| UNIT-NEW-02 | `ndp_lib::validate::validate_stream_config()` | Invalid JSON syntax | Returns `Ok(ValidationResult)` with syntax error |
| UNIT-NEW-03 | `ndp_lib::validate::validate_stream_config()` | Missing required fields | Returns `Ok(ValidationResult)` with schema errors |
| UNIT-NEW-04 | `ndp_lib::validate::validate_domain_config()` | Valid domain JSON | Returns `Ok(ValidationResult)` with `valid: true` |
| UNIT-NEW-05 | `ndp_lib::validate::validate_domain_config()` | Invalid role enum | Returns `Ok(ValidationResult)` with enum error |
| UNIT-NEW-06 | `ndp_lib::validate::validate_batch_streams()` | Directory with 3 configs (2 valid, 1 invalid) | Returns `BatchValidationResult` with correct counts |
| UNIT-NEW-07 | `ndp_lib::validate::validate_batch_domains()` | Directory with 2 valid domains | Returns `BatchValidationResult` all valid |
| UNIT-NEW-08 | `ndp_lib::validate::generate_schema()` | No arguments | Returns valid JSON schema string |
| UNIT-NEW-09 | `ndp_lib::validate::verify_schema()` | Matching schema file | Returns `Ok(true)` |
| UNIT-NEW-10 | `ndp_lib::validate::verify_schema()` | Drifted schema file | Returns `Ok(false)` |
| UNIT-NEW-11 | `ndp_lib::validate::validate_stream_config_semantic_only()` | Config with source errors | Returns semantic errors only |
| UNIT-NEW-12 | `ndp_lib::validate::format_json()` | Valid result | Returns valid JSON string |
| UNIT-NEW-13 | `ndp_lib::validate::format_human()` | Result with errors | Contains `[FAIL]` and error messages |
| UNIT-NEW-14 | `ndp_lib::validate::determine_exit_code()` | Strict mode with warnings | Returns exit code 1 |
| UNIT-NEW-15 | `ndp_lib::validate::determine_exit_code()` | Non-strict mode with warnings | Returns exit code 0 |
| UNIT-NEW-16 | `ndp_lib::validate::semantic::is_valid_granularity()` | Valid granularities: 1m, 5m, 15m, 30m, 1h, 4h, 1d | Returns `true` for each |
| UNIT-NEW-17 | `ndp_lib::validate::semantic::is_valid_granularity()` | Invalid granularities: 2m, 3h, 1w, "", "foo" | Returns `false` for each |
| UNIT-NEW-18 | `ndp_lib::validate::semantic::gold.rs` | `validate_gold_config()` calls shared `is_valid_granularity()` | No local `is_valid_granularity` definition in `gold.rs`; imports from `semantic::mod.rs` |
| UNIT-NEW-19 | `ndp_lib::validate::semantic::domain.rs` | `validate_domain_config()` calls shared `is_valid_granularity()` | No local `is_valid_granularity` definition in `domain.rs`; imports from `semantic::mod.rs` |

### 2.6 Test Fixtures Migration

| Source Path | Destination Path |
|---|---|
| `tools/ndp-validate/tests/fixtures/configs/valid/*.json` | `crates/ndp-lib/tests/validate_fixtures/configs/valid/*.json` |
| `tools/ndp-validate/tests/fixtures/configs/invalid/*.json` | `crates/ndp-lib/tests/validate_fixtures/configs/invalid/*.json` |

Fixture files: 3 valid configs (`valid_full.json`, `valid_minimal.json`, `valid_objectives.json`) and 7 invalid configs (`invalid_bad_granularity.json`, `invalid_bad_join_strategy.json`, `invalid_bad_role.json`, `invalid_duplicate_alias.json`, `invalid_missing_id.json`, `invalid_no_primary.json`, `invalid_single_stream.json`).

---

## 3. CLI Parity Tests (ops-003-06)

This is the CRITICAL section. Phase 1 taught us that flag combinations can silently break. Every mode, every flag, every invalid combination, and every exit code must be tested.

### 3.1 Flag Mapping Table

The `ndp validate` CLI uses flat flags (not entity/verb subcommands). Authoritative command signatures from CLI UX Design (10-CLI-UX-DESIGN-REVISED.md lines 193-203).

| ndp-validate Command | ndp validate Command | Behavior |
|---|---|---|
| `ndp-validate config.json` | `ndp validate --stream config.json` | Single stream validation |
| `ndp-validate --all` | `ndp validate --all` | Full platform validation (all streams + domains) |
| `ndp-validate --domain FILE` | `ndp validate --domain FILE` | Single domain validation |
| `ndp-validate --domain-all` | `ndp validate --domain --all` | Batch domain validation |
| `ndp-validate --generate-schema` | `ndp validate --schema --generate` | Schema generation |
| `ndp-validate --generate-schema --output F` | `ndp validate --schema --generate --output F` | Schema generation to file |
| `ndp-validate --verify-schema F` | `ndp validate --schema --verify F` | Schema drift detection |
| `--schema-only` | `--schema-only` | Skip semantic validation |
| `--check-tables` | `--check-tables` | Verify Silver tables exist |
| `--format json\|human` | `--format json\|human` | Output format |
| `--strict` | `--strict` | Treat warnings as errors |
| `--verbose` / `-v` | `--verbose` / `-v` | Progress output |
| `--config-dir DIR` | Inherits from global `--config-dir` | Config base directory |
| `--timescale-url URL` | Inherits from global `--db-url` | Database connection |
| `--schema-path PATH` | `--schema-path PATH` | Custom stream schema |
| `--domain-schema-path PATH` | `--domain-schema-path PATH` | Custom domain schema |
| `--domains-dir DIR` | `--domains-dir DIR` | Custom domains directory |

### 3.2 Mode Parity Tests

Each mode must produce identical output between old and new binaries.

| Test ID | Mode | Old Command | New Command | Assertion |
|---|---|---|---|---|
| PAR-MODE-01 | Single stream | `ndp-validate config/base/streams/air-quality/config.json` | `ndp validate --stream config/base/streams/air-quality/config.json` | JSON output identical |
| PAR-MODE-02 | Single stream (human) | `ndp-validate --format human config.json` | `ndp validate --stream config.json --format human` | Human output equivalent (ignore binary name in output) |
| PAR-MODE-03 | All streams + domains | `ndp-validate --all` | `ndp validate --all` | JSON output identical |
| PAR-MODE-04 | Single domain | `ndp-validate --domain config/domains/indoor-air-quality/domain.json` | `ndp validate --domain config/domains/indoor-air-quality/domain.json` | JSON output identical |
| PAR-MODE-05 | All domains | `ndp-validate --domain-all` | `ndp validate --domain --all` | JSON output identical |
| PAR-MODE-06 | Generate schema | `ndp-validate --generate-schema` | `ndp validate --schema --generate` | stdout JSON identical |
| PAR-MODE-07 | Generate schema to file | `ndp-validate --generate-schema --output /tmp/s.json` | `ndp validate --schema --generate --output /tmp/s.json` | File contents identical |
| PAR-MODE-08 | Verify schema (match) | `ndp-validate --verify-schema schemas/stream-config.v1.1.schema.json` | `ndp validate --schema --verify schemas/stream-config.v1.1.schema.json` | Exit code identical (0) |
| PAR-MODE-09 | Verify schema (drift) | `ndp-validate --verify-schema /tmp/wrong.json` | `ndp validate --schema --verify /tmp/wrong.json` | Exit code identical (1) |

### 3.3 Flag Behavior Tests

Each flag must produce measurably different behavior. Phase 1 BUG: `--validate-only` was captured but never branched on. EVERY flag below must demonstrate behavioral impact.

| Test ID | Flag | Test Method | Assertion |
|---|---|---|---|
| PAR-FLAG-01 | `--schema-only` | Run with and without on config with semantic errors | With flag: 0 semantic errors. Without: N semantic errors. |
| PAR-FLAG-02 | `--check-tables` + `--db-url` | Run with `--check-tables` and valid DB URL | Output includes table existence check results |
| PAR-FLAG-03 | `--check-tables` without `--db-url` | Run without DB URL | Exit code 2, error message mentions `--timescale-url` or `--db-url` |
| PAR-FLAG-04 | `--format json` | Default output | Valid JSON on stdout, parseable by `jq` |
| PAR-FLAG-05 | `--format human` | Run with `--format human` | Output contains `[PASS]` or `[FAIL]`, NO valid JSON on stdout |
| PAR-FLAG-06 | `--strict` with warnings | Run on config that produces warnings | Without `--strict`: exit 0. With `--strict`: exit 1. |
| PAR-FLAG-07 | `--strict` without warnings | Run on perfectly valid config | Both strict and non-strict: exit 0. |
| PAR-FLAG-08 | `--verbose` | Run with and without `--verbose` | With: stderr contains "Running Layer" messages. Without: no stderr. |
| PAR-FLAG-09 | `-v` (short) | Alias for `--verbose` | Identical behavior to PAR-FLAG-08 |
| PAR-FLAG-10 | `--config-dir` | Pass custom directory | Validation uses files from specified directory, NOT default |
| PAR-FLAG-11 | `--schema-path` | Pass custom schema file | Validation uses custom schema, NOT embedded default |
| PAR-FLAG-12 | `--domain-schema-path` | Pass custom domain schema | Domain validation uses custom schema |
| PAR-FLAG-13 | `--domains-dir` | Pass custom domains directory | `--all` scans specified directory, NOT default config/domains |

### 3.4 Invalid Combination Tests

Phase 1 lesson: flags that conflict must produce clear errors, NOT silent ignoring.

| Test ID | Invalid Combination | Expected |
|---|---|---|
| PAR-INV-01 | `ndp validate --stream config.json --all` | Error: conflicts -- cannot specify both `--stream` and `--all` |
| PAR-INV-02 | `ndp validate` (no flags at all) | Error: must specify `--stream`, `--domain`, `--all`, or `--schema` |
| PAR-INV-03 | `ndp validate --stream config.json --format xml` | Error: invalid format value |
| PAR-INV-04 | `ndp validate --stream config.json --check-tables` (no DB URL) | Exit 2: `--check-tables` requires `--db-url` or `TIMESCALE_URL` |
| PAR-INV-05 | `ndp validate --schema --generate config.json` | Error: `--schema --generate` does not accept positional args |
| PAR-INV-06 | `ndp validate --schema --verify` (no path) | Error: `--verify` requires a schema path argument |
| PAR-INV-07 | `ndp validate --stream config.json --schema-only --check-tables` | Valid (not conflicting -- schema-only skips semantic, check-tables is separate layer) OR error if they are designed to conflict |
| PAR-INV-08 | `ndp validate --domain config.json --check-tables` | Document expected behavior -- `--check-tables` may not apply to domains |
| PAR-INV-09 | `ndp validate --stream config.json --domain config.json` | Error: cannot specify both `--stream` and `--domain` |
| PAR-INV-10 | `ndp validate --stream nonexistent.json` | Exit 2: file not found |
| PAR-INV-11 | `ndp validate --domain nonexistent.json` | Exit 2: file not found |
| PAR-INV-12 | `ndp validate --schema --verify nonexistent.json` | Exit 2: schema file not found |

### 3.5 Exit Code Parity Tests

| Test ID | Scenario | Expected Exit Code | Assertion |
|---|---|---|---|
| PAR-EXIT-01 | Valid stream config | 0 | Both old and new binary return 0 |
| PAR-EXIT-02 | Invalid stream config (schema error) | 1 | Both return 1 |
| PAR-EXIT-03 | Nonexistent config file | 2 | Both return 2 |
| PAR-EXIT-04 | Valid config, `--strict`, no warnings | 0 | Both return 0 |
| PAR-EXIT-05 | Valid config, `--strict`, has warnings | 1 | Both return 1 |
| PAR-EXIT-06 | Valid domain config | 0 | Both return 0 |
| PAR-EXIT-07 | Invalid domain config | 1 | Both return 1 |
| PAR-EXIT-08 | Schema verify match | 0 | Both return 0 |
| PAR-EXIT-09 | Schema verify drift | 1 | Both return 1 |
| PAR-EXIT-10 | Schema generate success | 0 | Both return 0 |
| PAR-EXIT-11 | `--check-tables` without `--db-url` | 2 | Both return 2 |

### 3.6 Output Format Parity Tests

JSON output must be structurally identical. Human output must be semantically equivalent (allowing for binary name differences).

| Test ID | Test | Old Output | New Output | Comparison Method |
|---|---|---|---|---|
| PAR-OUT-01 | JSON: valid stream | JSON with `"valid": true` | Identical JSON | `jq -S` sort keys, diff |
| PAR-OUT-02 | JSON: invalid stream | JSON with errors array | Identical JSON | `jq -S` sort keys, diff |
| PAR-OUT-03 | JSON: batch (--all) | BatchValidationResult JSON | Identical JSON | `jq -S` sort keys, diff |
| PAR-OUT-04 | JSON: domain | JSON with `"valid": true` | Identical JSON | `jq -S` sort keys, diff |
| PAR-OUT-05 | Human: valid stream | `[PASS] config.json` | `[PASS] config.json` | String contains check (ignore ANSI codes) |
| PAR-OUT-06 | Human: invalid stream | `[FAIL]` + error list | Same error list | Strip ANSI, compare error messages |
| PAR-OUT-07 | Human: batch summary | `SUMMARY: N configs...` | Same summary line | Regex match on counts |
| PAR-OUT-08 | Schema generate | Schema JSON | Identical JSON | Byte-for-byte comparison |

### 3.7 Implementation: Parity Test Harness

The parity tests should be implemented as a Rust integration test that invokes both binaries and compares output.

```rust
// tests/validate_parity_tests.rs
use std::process::Command;

/// Run both old and new binary, compare output
fn assert_parity(old_args: &[&str], new_args: &[&str]) {
    let old_output = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--"])
        .args(old_args)
        .output()
        .expect("Failed to run ndp-validate");

    let new_output = Command::new("cargo")
        .args(["run", "-p", "ndp-cli", "--"])
        .args(new_args)
        .output()
        .expect("Failed to run ndp");

    assert_eq!(
        old_output.status.code(),
        new_output.status.code(),
        "Exit codes differ: old={:?}, new={:?}",
        old_output.status.code(),
        new_output.status.code()
    );

    // For JSON output: parse and compare structurally
    let old_json: serde_json::Value = serde_json::from_slice(&old_output.stdout)
        .unwrap_or_default();
    let new_json: serde_json::Value = serde_json::from_slice(&new_output.stdout)
        .unwrap_or_default();
    assert_eq!(old_json, new_json, "JSON output differs");
}
```

Alternative: Shell-based parity test script for rapid iteration before Rust tests are ready.

```bash
#!/bin/bash
# .test/validate-parity.sh
# Run from repository root

set -e
CONFIG="config/base/streams/air-quality/config.json"
OLD_BIN="cargo run -p ndp-validate --"
NEW_BIN="cargo run -p ndp-cli --"

echo "=== PAR-MODE-01: Single stream ==="
diff <($OLD_BIN "$CONFIG" 2>/dev/null | jq -S .) \
     <($NEW_BIN validate --stream "$CONFIG" 2>/dev/null | jq -S .) \
     && echo "PASS" || echo "FAIL"

echo "=== PAR-EXIT-01: Exit code (valid) ==="
$OLD_BIN "$CONFIG" >/dev/null 2>&1; OLD_RC=$?
$NEW_BIN validate --stream "$CONFIG" >/dev/null 2>&1; NEW_RC=$?
[ "$OLD_RC" -eq "$NEW_RC" ] && echo "PASS (both $OLD_RC)" || echo "FAIL ($OLD_RC != $NEW_RC)"
```

---

## 4. deploy.sh Integration Tests (ops-003-07)

### 4.1 Dispatch Sites

deploy.sh has two sites that invoke `ndp-validate`:

| Site | Line | Function | Purpose |
|---|---|---|---|
| **Site 1** | ~1530 | `validate_domain_configs()` | Validates domain configs from manifest before deployment |
| **Site 2** | ~2032 | `handle_domain_declaration()` | Validates individual domain config before Gold DDL sync |

Both sites use an identical tool-discovery cascade:
1. `command -v ndp-validate`
2. `/opt/ndp/bin/ndp-validate`
3. `$REPO_ROOT/target/release/ndp-validate`
4. `$REPO_ROOT/target/debug/ndp-validate`

### 4.2 Switchover Strategy

Phase 2 changes the cascade to prefer `ndp` over `ndp-validate`:
1. `command -v ndp` (new unified CLI)
2. `command -v ndp-validate` (legacy fallback)
3. `/opt/ndp/bin/ndp` (Pi install path)
4. `$REPO_ROOT/target/release/ndp` (dev build)
5. `$REPO_ROOT/target/debug/ndp` (debug build)

If neither `ndp` nor `ndp-validate` is found: **error + return 1** (no silent skip). This is a policy change from Phase 1 where the Gold tool discovery silently warned and returned 0.

### 4.3 deploy.sh Test Matrix

| Test ID | Dispatch Site | Scenario | Assertion |
|---|---|---|---|
| DEPLOY-INT-01 | Site 1 (validate_domain_configs) | `ndp` binary on PATH, valid manifest with domain | Exit 0, validation runs, "validated successfully" in log |
| DEPLOY-INT-02 | Site 1 | `ndp` binary on PATH, invalid domain config | Exit 1, "validation failed" in log |
| DEPLOY-INT-03 | Site 1 | Neither `ndp` nor `ndp-validate` on PATH | Exit 1, error message (NOT warning + return 0) |
| DEPLOY-INT-04 | Site 2 (handle_domain_declaration) | `ndp` binary on PATH, valid domain file | Validation passes, proceeds to Gold DDL |
| DEPLOY-INT-05 | Site 2 | `ndp` binary on PATH, invalid domain file | Exit 1, "Domain config validation failed" in log |
| DEPLOY-INT-06 | Site 2 | Neither binary available | Exit 1, error message |
| DEPLOY-INT-07 | Site 1 | `--config-dir` resolves to `config/base` (not `config/base/streams`) | Streams dir = `$config_dir/streams/`, domains dir = `$config_dir/../domains/` |
| DEPLOY-INT-08 | Both | `$REPO_ROOT/target/release/ndp` exists but not on PATH | Falls through to file path check, uses it |
| DEPLOY-INT-09 | Both | `DEPLOY_ENV=integration` | Config dir resolves to `config/integration/base` |
| DEPLOY-INT-10 | Site 2 | Invocation includes `--config-dir "$CONFIG_STREAMS_DIR"` | The `--config-dir` flag points to streams subdir (BUG-004 prevention) |
| DEPLOY-INT-11 | Site 2 | Domain JSON + `--format human` | Human output appears in deploy logs (not raw JSON) |
| DEPLOY-INT-12 | Both | Legacy fallback: only `ndp-validate` on PATH | Uses `ndp-validate` with old argument format |

### 4.4 BUG-004 Prevention: --config-dir Path Resolution

Phase 1 BUG-004 was caused by `--config-dir` receiving a path that was one level too deep or too shallow. This requires explicit tests.

```
# CORRECT for stream validation:
ndp validate --stream config.json --config-dir config/base/streams
# or via global --config-dir:
ndp --config-dir config/base validate --stream config.json

# CORRECT for domain validation:
ndp validate --domain domain.json --config-dir config/base/streams
# (domain validation uses --config-dir to find related streams, NOT the domain itself)

# deploy.sh Site 2 currently passes:
#   "$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
#
# For the new binary this becomes:
#   ndp validate --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
```

Test DEPLOY-INT-10 specifically verifies that when deploy.sh calls the new binary, the `--config-dir` value resolves correctly and validation can find related stream configs for cross-reference checks.

### 4.5 Implementation: deploy.sh Test Approach

deploy.sh tests must use real shell execution, NOT Rust unit tests. They are shell integration tests.

```bash
#!/bin/bash
# .test/deploy-validate-switchover.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Build the ndp binary
cargo build -p ndp-cli 2>/dev/null

# Source deploy.sh functions (not main)
export REPO_ROOT
export CONFIG_STREAMS_DIR="$REPO_ROOT/config/base/streams"
export DOMAINS_DIR="$REPO_ROOT/config/domains"
export PATH="$REPO_ROOT/target/debug:$PATH"

# Test: validate_domain_configs finds ndp binary
echo "=== DEPLOY-INT-01: ndp binary on PATH ==="
# Create test manifest with domain
cat > /tmp/test-manifest.json <<'EOF'
{
  "version": "1.0",
  "declarations": {
    "domains": [{"domain_id": "indoor-air-quality"}]
  }
}
EOF
# Source the function and test it
# (requires deploy.sh to be source-able, test via subprocess)
```

---

## 5. E2E Integration Tests (Against Live TimescaleDB)

### 5.1 Prerequisites

- Docker Compose integration stack running: `docker compose -f docker-compose.integration.yml up -d`
- TimescaleDB accessible at `localhost:5432`, database `ndp`, user `postgres`/`postgres`
- psql NOT available in dev container; use `docker exec -i integration-timescaledb psql`
- Silver tables must be created before `--check-tables` tests (deploy Silver ETL first)

### 5.2 E2E Test Matrix

| Test ID | Config | Command | Assertion |
|---|---|---|---|
| E2E-01 | `config/base/streams/air-quality/config.json` | `ndp validate --stream config.json --format json` | Exit 0, `"valid": true` |
| E2E-02 | `config/base/streams/air-quality/config.json` | `ndp validate --stream config.json --schema-only --format json` | Exit 0, no semantic errors |
| E2E-03 | ALL stream configs | `ndp validate --all --config-dir config/base/streams` | Exit 0, all valid |
| E2E-04 | `config/domains/indoor-air-quality/domain.json` | `ndp validate --domain config/domains/indoor-air-quality/domain.json` | Exit 0 |
| E2E-05 | ALL domain configs | `ndp validate --domain --all --domains-dir config/domains` | Exit 0 |
| E2E-06 | Air quality config | `ndp validate --stream config.json --check-tables --db-url postgresql://postgres:postgres@localhost:5432/ndp` | Exit 0 if Silver tables exist, exit 1 with TABLE_NOT_FOUND if not |
| E2E-07 | Schema verify | `ndp validate --schema --verify schemas/stream-config.v1.1.schema.json` | Exit 0 (schema in sync) |
| E2E-08 | Schema generate | `ndp validate --schema --generate \| jq .title` | Outputs `"NDP Configuration Types"` |
| E2E-09 | Piped JSON (deploy.sh pattern) | `ndp validate --all --format json 2>/dev/null \| jq '.summary.invalid_configs'` | Outputs `0` (all valid) |
| E2E-10 | Verbose + human (deploy.sh pattern) | `ndp validate --domain FILE --format human --verbose 2>&1` | stderr has "Running Layer", stdout has `[PASS]` |

### 5.3 Known Limitations (NOT Regressions)

These are known issues from Phase 1 and the existing ndp-validate that must NOT block Phase 2.

| Limitation | Impact | Resolution |
|---|---|---|
| Domain semantic validation fails if Silver tables not deployed | `--domain-all` may report errors for streams without Silver tables | Document: run Silver ETL before domain validation |
| `--check-tables` requires live DB | Cannot run in unit test environment | Mark E2E-06 with `#[ignore]` |
| Schema drift detection is sensitive to schemars version | `verify-schema` may fail after schemars update | Pin schemars version in Cargo.toml |

---

## 6. Regression Prevention

### 6.1 Golden Master Test Design

Following the Phase 1 approach (15 golden master DDL tests), Phase 2 creates golden master snapshots of `ndp-validate` output before migration.

#### Capture Process

```bash
#!/bin/bash
# .test/capture-validate-golden-masters.sh
# Run BEFORE migration to capture baseline output

FIXTURES_DIR="crates/ndp-lib/tests/validate_fixtures/golden-master"
mkdir -p "$FIXTURES_DIR"

# GM-01: Valid stream config (JSON output)
ndp-validate config/base/streams/air-quality/config.json \
  > "$FIXTURES_DIR/gm-01-valid-stream.json" 2>/dev/null

# GM-02: Invalid fixture (schema errors)
ndp-validate tools/ndp-validate/tests/fixtures/configs/invalid/invalid_missing_id.json \
  > "$FIXTURES_DIR/gm-02-invalid-missing-id.json" 2>/dev/null

# GM-03: Domain validation
ndp-validate --domain config/domains/indoor-air-quality/domain.json \
  > "$FIXTURES_DIR/gm-03-valid-domain.json" 2>/dev/null

# GM-04: Batch stream validation
ndp-validate --all --config-dir config/base/streams \
  > "$FIXTURES_DIR/gm-04-batch-streams.json" 2>/dev/null

# GM-05: Schema generation
ndp-validate --generate-schema \
  > "$FIXTURES_DIR/gm-05-schema-generate.json" 2>/dev/null

# GM-06: Human output (strip ANSI for stable comparison)
ndp-validate --format human config/base/streams/air-quality/config.json 2>/dev/null \
  | sed 's/\x1b\[[0-9;]*m//g' \
  > "$FIXTURES_DIR/gm-06-human-output.txt"

# GM-07: Human batch output
ndp-validate --format human --all --config-dir config/base/streams 2>/dev/null \
  | sed 's/\x1b\[[0-9;]*m//g' \
  > "$FIXTURES_DIR/gm-07-human-batch.txt"

# GM-08: Schema-only mode (fewer errors than full)
ndp-validate --schema-only tools/ndp-validate/tests/fixtures/configs/invalid/invalid_bad_role.json \
  > "$FIXTURES_DIR/gm-08-schema-only.json" 2>/dev/null

# Generate checksums
cd "$FIXTURES_DIR"
sha256sum *.json *.txt > CHECKSUMS.sha256
```

#### Golden Master Tests

| Test ID | Baseline File | New Command | Comparison |
|---|---|---|---|
| GM-01 | `gm-01-valid-stream.json` | `ndp validate --stream air-quality/config.json` | JSON structural equality |
| GM-02 | `gm-02-invalid-missing-id.json` | `ndp validate --stream invalid_missing_id.json` | JSON structural equality |
| GM-03 | `gm-03-valid-domain.json` | `ndp validate --domain domain.json` | JSON structural equality |
| GM-04 | `gm-04-batch-streams.json` | `ndp validate --all` | JSON structural equality (sort results by config_path) |
| GM-05 | `gm-05-schema-generate.json` | `ndp validate --schema --generate` | JSON structural equality |
| GM-06 | `gm-06-human-output.txt` | `ndp validate --stream config.json --format human` (strip ANSI) | Line-by-line text comparison |
| GM-07 | `gm-07-human-batch.txt` | `ndp validate --all --format human` (strip ANSI) | Line-by-line text comparison |
| GM-08 | `gm-08-schema-only.json` | `ndp validate --stream invalid.json --schema-only` | JSON structural equality |

### 6.2 Checksum Verification

```rust
#[test]
fn test_golden_master_checksums_valid() {
    let checksums_path = Path::new("tests/validate_fixtures/golden-master/CHECKSUMS.sha256");
    let content = std::fs::read_to_string(checksums_path)
        .expect("CHECKSUMS.sha256 must exist");

    for line in content.lines() {
        let parts: Vec<&str> = line.splitn(2, "  ").collect();
        assert_eq!(parts.len(), 2, "Invalid checksum line: {}", line);
        let expected_hash = parts[0];
        let filename = parts[1];

        let file_path = checksums_path.parent().unwrap().join(filename);
        let file_content = std::fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Missing golden master file: {}", filename));

        let actual_hash = sha256_hex(&file_content);
        assert_eq!(
            expected_hash, actual_hash,
            "Checksum mismatch for {}: expected {}, got {}",
            filename, expected_hash, actual_hash
        );
    }
}
```

### 6.3 JSON Comparison Approach

JSON outputs may differ in key ordering. Use normalized comparison.

```rust
fn assert_json_equivalent(old: &str, new: &str) {
    let old_val: serde_json::Value = serde_json::from_str(old)
        .expect("Old output is not valid JSON");
    let new_val: serde_json::Value = serde_json::from_str(new)
        .expect("New output is not valid JSON");

    // Normalize: sort keys recursively
    let old_sorted = sort_json_keys(&old_val);
    let new_sorted = sort_json_keys(&new_val);

    assert_eq!(
        old_sorted, new_sorted,
        "JSON outputs differ.\nOld: {}\nNew: {}",
        serde_json::to_string_pretty(&old_sorted).unwrap(),
        serde_json::to_string_pretty(&new_sorted).unwrap()
    );
}
```

---

## 7. Acceptance Criteria Verification Matrix

### ops-003-05: Validate module in ndp-lib

| Criterion | Test(s) | Pass Condition |
|---|---|---|
| All 222 ndp-validate tests pass under new module paths | `cargo test -p ndp-lib -- validate` | 222/222 pass (0 failures, 0 ignored) |
| ndp-lib test count increases by at least 222 | `cargo test -p ndp-lib -- --list \| wc -l` | Count >= 316 (94 existing + 222 migrated) |
| ndp-validate thin wrapper still compiles and passes its own tests | `cargo test -p ndp-validate` | All tests pass (65 CLI tests remain in wrapper) |
| New convenience functions have unit tests | UNIT-NEW-01 through UNIT-NEW-19 | 19/19 pass |
| Import path changes are complete (no references to old paths) | `grep -r "ndp_validate::" crates/ndp-lib/` returns empty | 0 occurrences |
| Test fixtures migrated correctly | `ls crates/ndp-lib/tests/validate_fixtures/configs/{valid,invalid}/` | 3 valid + 7 invalid config files present |

### ops-003-06: `ndp validate` flat-flag CLI

| Criterion | Test(s) | Pass Condition |
|---|---|---|
| Every mode produces identical output | PAR-MODE-01 through PAR-MODE-09 | 9/9 pass |
| Every flag produces measurably different behavior | PAR-FLAG-01 through PAR-FLAG-13 | 13/13 pass |
| Every invalid combination produces clear error | PAR-INV-01 through PAR-INV-12 | 12/12 pass |
| Exit codes match old binary for all scenarios | PAR-EXIT-01 through PAR-EXIT-11 | 11/11 pass |
| JSON output structurally identical | PAR-OUT-01 through PAR-OUT-04, PAR-OUT-08 | 5/5 pass |
| Human output semantically equivalent | PAR-OUT-05 through PAR-OUT-07 | 3/3 pass |
| Golden master tests pass | GM-01 through GM-08 | 8/8 pass |

### ops-003-07: deploy.sh validate switchover

| Criterion | Test(s) | Pass Condition |
|---|---|---|
| deploy.sh Site 1 works with `ndp` binary | DEPLOY-INT-01, DEPLOY-INT-02 | Pass |
| deploy.sh Site 2 works with `ndp` binary | DEPLOY-INT-04, DEPLOY-INT-05, DEPLOY-INT-11 | Pass |
| No-fallback policy enforced | DEPLOY-INT-03, DEPLOY-INT-06 | Exit 1 when no binary found |
| `--config-dir` path resolution correct | DEPLOY-INT-07, DEPLOY-INT-10 | Streams dir resolves correctly |
| Integration env path resolves | DEPLOY-INT-09 | `config/integration/base` used |
| Legacy fallback works | DEPLOY-INT-12 | `ndp-validate` invoked with old arg format |
| E2E against real configs | E2E-01 through E2E-10 | All pass (E2E-06 conditional on Silver tables) |

---

## 8. Test Execution Checklist

### 8.1 Pre-Migration (Run Once, Before Any Code Changes)

- [ ] Run `cargo test -p ndp-validate -- --list | wc -l` and record count (expect 222)
- [ ] Run `cargo test -p ndp-validate` and confirm all 222 pass
- [ ] Run golden master capture script (`.test/capture-validate-golden-masters.sh`)
- [ ] Verify golden master checksums file exists
- [ ] Record `cargo test -p ndp-lib -- --list | wc -l` (expect ~94 or current count with gold migration)

### 8.2 During Migration (After Each File Move)

- [ ] After moving `error.rs`: `cargo test -p ndp-lib -- validate::error` (5 tests)
- [ ] After moving `schema.rs`: `cargo test -p ndp-lib -- validate::schema` (45 tests)
- [ ] After moving `schema_gen.rs`: `cargo test -p ndp-lib -- validate::schema_gen` (10 tests)
- [ ] After moving `semantic/sources.rs`: `cargo test -p ndp-lib -- validate::semantic::sources` (18 tests)
- [ ] After moving `semantic/source_path.rs`: `cargo test -p ndp-lib -- validate::semantic::source_path` (15 tests)
- [ ] After moving `semantic/table_exists.rs`: `cargo test -p ndp-lib -- validate::semantic::table_exists` (11 tests)
- [ ] After moving `semantic/dq_rules.rs`: `cargo test -p ndp-lib -- validate::semantic::dq_rules` (22 tests)
- [ ] After moving `semantic/gold.rs`: `cargo test -p ndp-lib -- validate::semantic::gold` (13 tests)
- [ ] After moving `semantic/domain.rs`: `cargo test -p ndp-lib -- validate::semantic::domain` (13 tests)
- [ ] After all moves: `cargo test -p ndp-lib` -- ALL tests pass (existing + migrated)

### 8.3 After CLI Implementation (After `ndp validate` Works)

- [ ] Run `cargo test -p ndp-cli` -- all tests pass
- [ ] Run parity test script (`.test/validate-parity.sh`) -- all modes match
- [ ] Run parity test suite: `cargo test -p ndp-lib -- validate_parity` (if Rust-based)
- [ ] Verify new unit tests: `cargo test -p ndp-lib -- validate::tests::unit_new` (UNIT-NEW-01 through UNIT-NEW-19)

### 8.4 After deploy.sh Changes

- [ ] Run deploy.sh integration tests: `.test/deploy-validate-switchover.sh`
- [ ] Test with `DEPLOY_ENV=integration` if integration stack is running
- [ ] Verify both dispatch sites with valid domain config
- [ ] Verify both dispatch sites with invalid domain config
- [ ] Verify error when neither binary is available
- [ ] Verify legacy fallback when only `ndp-validate` is available

### 8.5 E2E (Requires Integration Stack)

- [ ] Start integration stack: `docker compose -f docker-compose.integration.yml up -d`
- [ ] Wait for TimescaleDB to be ready: `docker exec integration-timescaledb pg_isready`
- [ ] Run E2E tests: E2E-01 through E2E-10
- [ ] If Silver tables exist: run E2E-06 (`--check-tables`)
- [ ] Stop integration stack: `docker compose -f docker-compose.integration.yml down`

### 8.6 Final Verification

- [ ] `cargo test -p ndp-lib -- --list | wc -l` >= 316 (94 existing + 222 migrated)
- [ ] `cargo test -p ndp-lib` -- 0 failures
- [ ] `cargo test -p ndp-validate` -- 0 failures (thin wrapper tests)
- [ ] `cargo test -p ndp-cli` -- 0 failures
- [ ] Golden master tests: GM-01 through GM-08 all pass
- [ ] `grep -r "ndp_validate::" crates/ndp-lib/src/` returns 0 results
- [ ] `cargo build -p ndp-cli --release` succeeds
- [ ] `.test/validate-parity.sh` all PASS
- [ ] No `TODO`, `unimplemented!()`, or `todo!()` in new code

---

## Appendix A: Test Count Summary

| Category | Count |
|---|---|
| Migrated unit tests (from ndp-validate) | 222 |
| New ndp-lib API surface tests (UNIT-NEW-*) | 19 |
| CLI parity tests (PAR-*) | 55 |
| deploy.sh integration tests (DEPLOY-INT-*) | 12 |
| E2E integration tests (E2E-*) | 10 |
| Golden master tests (GM-*) | 8 |
| **Total new/migrated tests** | **326** |

After Phase 2, `cargo test -p ndp-lib` should run approximately **335+ tests** (94 pre-existing + 222 migrated + 19 new). The PAR-*, DEPLOY-INT-*, E2E-*, and GM-* tests are integration tests that run separately.

## Appendix B: Dependency on Phase 1

Phase 2 assumes Phase 1 (Gold migration) is complete. Specifically:

- `crates/ndp-lib/src/gold/` module exists and all 376 Gold tests pass
- `tools/ndp-cli/src/commands/gold.rs` exists and works
- deploy.sh Gold dispatch has been switched to `ndp gold`
- `ndp_lib::db::DbClient` trait is stable

If Phase 1 is not complete, Phase 2 migration can still proceed for the validate module, but the ndp-cli integration (adding `validate` to the Commands enum) requires the existing CLI infrastructure from Phase 1.

## Appendix C: Risk Areas

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `jsonschema` crate version conflict between ndp-validate and ndp-lib | Medium | Tests fail with different validation behavior | Pin jsonschema version in workspace Cargo.toml |
| `schemars` output changes between builds | Low | Golden master tests fail | Use normalized JSON comparison, pin schemars |
| deploy.sh path resolution differs between Pi and dev container | High | DEPLOY-INT tests pass in dev but fail on Pi | Test with `DEPLOY_ENV=pi` explicitly, use absolute paths |
| `tempfile` crate dependency for schema_gen tests | Low | Compilation error if tempfile not in ndp-lib deps | Add tempfile to `[dev-dependencies]` |
| CLI argument parsing differences (clap version) | Low | Parity tests fail for edge cases | Both crates use workspace clap version |
