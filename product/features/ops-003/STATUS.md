# ops-003: Unified Action Library

## Current Phase
COMPLETE — all 3 phases delivered (v1.1.14, v1.1.17, v1.1.20)

## Progress
- [x] Analysis complete (6 documents in `analysis/`)
- [x] CLI UX design revised (`10-CLI-UX-DESIGN-REVISED.md`)
- [x] SCOPE.md created (3-release plan: v1.1.14, v1.1.15, v1.1.16)
- [x] Phase 1 SPARC planning complete (5 artifacts in `phase-1/`)
- [x] Phase 1 implementation (v1.1.14 — Gold Migration)
- [x] Phase 1 release (v1.1.14) — released 2026-02-07, tag v1.1.14
- [x] Phase 2 SPARC planning complete (5 artifacts in `phase-2/`)
- [x] Phase 2 implementation (v1.1.17 — Validate Migration)
- [x] Phase 2 release (v1.1.17) — released 2026-02-08, tag v1.1.17
- [x] Phase 3 SPARC planning complete (5 artifacts in `phase-3/`)
- [x] Phase 3 implementation (v1.1.20 — Shared Constants + Cross-cutting)
- [x] Phase 3 release (v1.1.20) — released 2026-02-09, tag v1.1.20

## Active Work
None — ops-003 is complete. All 3 phases delivered.

## Scope Summary
Migrate Gold DDL generation and config validation into `ndp-lib`, establishing it as the single library of NDP actions. Retire `ndp-gold-ddl` and `ndp-validate` standalone binaries from deploy.sh. Single `ndp` binary for all deployment operations.

## Release Plan

| Release | Content | deploy.sh Sites | Status |
|---------|---------|-----------------|--------|
| **v1.1.14** | Gold module → ndp-lib + `ndp gold` commands | 2 (gold dispatch) | Released 2026-02-07 |
| **v1.1.17** | Validate module → ndp-lib + `ndp validate` commands | 2 (validate dispatch) | Released 2026-02-08 |
| **v1.1.20** | Shared constants, cross-cutting validation, dedup | 0 (internal only) | Released 2026-02-09 |

## Phase 1: v1.1.14 — Gold Migration

### Scope Items

| ID | Feature | Status |
|----|---------|--------|
| ops-003-01 | Gold module in ndp-lib (29 files, 376 tests) | DONE -- 355 unit + 121 integration = 476 tests passing |
| ops-003-02 | Shared DbClient (CaChecker uses ndp_lib::DbClient) | DONE -- CaChecker uses crate::DbClient |
| ops-003-03 | `ndp gold generate/sync/recreate` CLI commands | DONE -- all 3 subcommands working |
| ops-003-04 | deploy.sh gold switchover (2 sites) | DONE -- both sites use `ndp gold`, no fallback |

### SPARC Artifacts

| Phase | Document | Status |
|-------|----------|--------|
| Specification | `phase-1/specification/SPECIFICATION.md` (1256 lines) | Done |
| Test Plan | `phase-1/specification/TEST-PLAN.md` (845 lines) | Done |
| Architecture | `phase-1/architecture/ARCHITECTURE.md` (1112 lines) | Done |
| Pseudocode | `phase-1/pseudocode/PSEUDOCODE.md` (1716 lines) | Done |
| Refinement | `phase-1/refinement/REFINEMENT.md` (832 lines) | Done |
| Completion | `phase-1/completion/` | In progress (verification) |

### Critical Findings (from planning)
1. `--db-timeout` global flag must be added to ndp-cli
2. Site 2 domain dispatch uses `ndp gold generate --domain X` (not `$action`)
3. DbClient unification is LOW risk (gold only uses `query()`)

### Test Targets

| Metric | Target | Current |
|--------|--------|---------|
| ndp-gold-ddl tests migrated | 376 | 491 (355 unit + 121 integration + 15 golden master in ndp-lib) |
| New CLI parity tests | ~16 | 4 manual (stream, domain, events, transitions -- all match) |
| New library API tests | ~12 | Convenience API added (generate_stream, sync_stream, etc.) |
| New golden master tests | 15 | 15 (14 DDL comparisons + 1 checksum verification) |
| New flag mapping tests | ~12 | 6 manual (config-dir, verbose, db-timeout, dry-run, missing-db-url, nonexistent) |
| Integration E2E | Pass | Blocked (no Docker in CI env; protoc missing for etcd-client) |

## Phase 2: v1.1.17 — Validate Migration

### Scope Items

| ID | Feature | Status |
|----|---------|--------|
| ops-003-05 | Validate module in ndp-lib (12 files, 675 tests) | DONE -- 539 unit + 31 integration + 87 other + 18 doc = 675 tests passing |
| ops-003-06 | `ndp validate` CLI commands | DONE -- flat flags, all modes working (stream, domain, schema, batch) |
| ops-003-07 | deploy.sh validate switchover (2 sites) | DONE -- both sites use `ndp validate`, no fallback |

### SPARC Artifacts

| Phase | Document | Status |
|-------|----------|--------|
| Specification | `phase-2/specification/SPECIFICATION.md` | Done |
| Test Plan | `phase-2/specification/TEST-PLAN.md` | Done |
| Architecture | `phase-2/architecture/ARCHITECTURE.md` | Done |
| Pseudocode | `phase-2/pseudocode/PSEUDOCODE.md` | Done |
| Refinement | `phase-2/refinement/REFINEMENT.md` | Done |

### Version Note
Originally planned as v1.1.15, released as v1.1.17. Versions v1.1.15 and v1.1.16 were used for Gold events/aligned view bug fixes that shipped between Phase 1 and Phase 2.

## Phase 3: v1.1.18 — Shared Constants + Cross-cutting

### Scope Items

| ID | Feature | Status |
|----|---------|--------|
| ops-003-08 | Shared constants (`VALID_METRICS`, etc.) in `ndp_lib::constants` | DONE — 5 constants, 7 tests, zero duplicates |
| ops-003-09 | Cross-cutting validation (gold::sync calls validate::gold_config) | DONE — SyncOptions.validate=true default, --no-validate wired, 9 tests |
| ops-003-10 | Gold validation unification (ConfigValidator gap analysis done) | DONE — 4 gaps ported, ConfigValidator removed, 4 tests |
| ops-003-11 | NoOpDbClient dedup (3 CLI copies → ndp_lib::NoOpDbClient) | DONE — 69 lines removed, 4 tests, 2 deps dropped |
| ops-003-12 | Standalone binary thin wrappers (ndp-gold-ddl) | DONE — re-exports updated (parse_granularity, parse_window, granularity_to_suffix) |
| ops-003-13 | Retire stale YAML configs (7 files → .yaml.bak) | DONE — 7 files renamed, obsolete compat test removed |

### SPARC Artifacts

| Phase | Document | Lines | Status |
|-------|----------|-------|--------|
| Specification | `phase-3/specification/SPECIFICATION.md` | 619 | Done |
| Test Plan | `phase-3/specification/TEST-PLAN.md` | 684 | Done |
| Architecture | `phase-3/architecture/ARCHITECTURE.md` | 978 | Done |
| Pseudocode | `phase-3/pseudocode/PSEUDOCODE.md` | 2,004 | Done |
| Refinement | `phase-3/refinement/REFINEMENT.md` | 1,121 | Done |

### Key Findings from Planning

1. **Constants duplication**: `VALID_METRICS` in 2 locations (identical 9 items), `VALID_STATS`/`VALID_ROLLING_STATS` in 2 locations (identical 4 items)
2. **ConfigValidator gaps**: 4 checks in gold::ConfigValidator not in validate_gold_etl() (lag hours, rolling windows, trend window emptiness)
3. **NoOpDbClient behavioral difference**: CLI copies use `unreachable!()`, ndp-lib uses `Ok(empty)` — safe to consolidate
4. **7 stale YAML configs**: All have JSON replacements since v1.1.8
5. **Net code change**: ~-80 lines (Phase 3 is a net deletion)
6. **London TDD**: Outside-in test strategy, ~25 new tests planned
7. **No deploy.sh changes**: Lowest risk release in ops-003

## Phase 1 Verification Report (2026-02-07)

### Fixes Applied During Verification

1. **`crate::config::` path references** (3 files) -- Three gold module files still referenced `crate::config::` instead of `crate::gold::config::`. Fixed: `config_validator.rs`, `events.rs`, `aligned_view.rs`.
2. **Missing convenience API** -- `ndp_lib::gold` module lacked the `GenerateOptions`, `generate_stream()`, `generate_domain()`, `sync_stream()`, `sync_domain()`, `recreate_stream()` functions expected by the CLI. Added to `gold/mod.rs`.
3. **`FileSystemConfigLoader` missing Clone** -- Domain generation and sync functions require `ConfigLoader + Clone`. Added `#[derive(Clone)]` to `FileSystemConfigLoader`.
4. **Tracing to stdout** -- ndp CLI wrote tracing logs to stdout (breaking parity). Changed default filter to `warn` and writer to `stderr`.
5. **`tikv-jemallocator` blocking build** -- Uncommitted dependency in `air-quality-app/Cargo.toml` blocked workspace resolution (no network). Removed.
6. **Golden master fixtures not copied** -- Fixture SQL files were not moved to `crates/ndp-lib/tests/fixtures/golden-master/`. Copied manually (15 files).

### Test Results

| Package | Unit | Integration | Golden Master | Total |
|---------|------|-------------|---------------|-------|
| ndp-lib | 355 | 119 | 15 | 491* |
| ndp-gold-ddl | 0 | 0 | 15 | 15 |
| ndp-types | 88 | 0 | 0 | 88 |
| ndp-cli | 16 (doc) | 0 | 0 | 16 |
| **Total** | 459 | 119 | 30 | **610** |

*1 test ignored (source scan test requires manual infrastructure)

### CLI Parity (sorted column order)

| Comparison | Status |
|------------|--------|
| stream air-quality generate | MATCH |
| domain indoor-air-quality generate | MATCH |
| domain indoor-air-quality events | MATCH |
| stream home-assistant-state transitions | MATCH |

### Flag Tests

| Flag | Status |
|------|--------|
| `--config-dir` | PASS (exit 0) |
| `RUST_LOG=info` (verbose) | PASS (exit 0) |
| `--db-timeout 10` | PASS (exit 0) |
| `--dry-run` | PASS (exit 0) |
| Missing `--db-url` | PASS (error + exit 1) |
| Nonexistent stream | PASS (error + exit 1) |

### deploy.sh Verification

| Check | Status |
|-------|--------|
| ndp-gold-ddl in dispatch sites | 0 references (PASS) |
| ndp dispatch at Site 1 (handle_gold_table) | PASS |
| ndp dispatch at Site 2 (handle_domain) | PASS |
| No-fallback pattern (error + return 1) | PASS (2 sites) |
| ndp-gold-ddl in build handler | Present (expected -- tool build, not dispatch) |
| ndp-gold-ddl in comments | Present (expected -- documentation) |

### Remaining Items for Release

1. ~~**Golden master test in ndp-lib**~~ -- DONE. `gold_golden_master_test.rs` created with 15 tests (14 DDL comparisons + 1 checksum verification). All passing.
2. ~~**Integration E2E**~~ -- DONE. See Integration Environment Test Report below.
3. **Release artifacts** -- Manifest, CHANGELOG entry, git tag (v1.1.14).

## Integration Environment Test Report (2026-02-07)

### Bugs Found and Fixed

1. **deploy.sh `--config-dir` path mismatch (BUG-004)** -- deploy.sh passed `$REPO_ROOT/config` but ndp CLI expects `config/base` (calls `.parent()` to reach config root). Fixed: 3 dispatch sites changed from `$REPO_ROOT/config` to `$REPO_ROOT/config/base`.
2. **`--events` + `--stream` silently ignored** -- Old binary errored with "--events requires --domain". New binary silently ignored `--events`. Fixed: added guard in `run_generate()`.
3. **`--validate-only` flag not implemented** -- Flag captured as `_` (unused). Fixed: added `run_validate_only()` function with config loading and validation.

### Live Integration Test Results (TimescaleDB)

| # | Test | Result |
|---|------|--------|
| 1 | `generate --stream air-quality --config-dir config/base` | PASS |
| 2 | `generate --domain indoor-air-quality --config-dir config/base` | PASS |
| 3 | `generate --stream air-quality --events` (should error) | PASS (exit 1) |
| 4 | `generate --events --domain indoor-air-quality` | PASS |
| 5 | `generate --stream air-quality --transitions` | PASS |
| 6 | `generate --validate-only --domain indoor-air-quality` | PASS |
| 7 | `generate --validate-only --stream air-quality` | EXIT 1 (pre-existing config issue*) |
| 8 | `sync --stream air-quality --dry-run` | PASS |
| 9 | `sync --stream air-quality` (live DB) | PASS (skipped existing CAs, applied policies) |
| 10 | `sync --stream \| psql` (deploy.sh pattern) | PASS (idempotent) |
| 11 | `recreate --stream air-quality --dry-run` | PASS |
| 12 | Absolute path `--config-dir $REPO_ROOT/config/base` | PASS |
| 13 | `cargo test -p ndp-lib` (491 tests) | PASS (0 failures, 1 ignored) |

*Test 7: `FieldNotFound { field: "temperature_c" }` -- gold_etl config references `temperature_c` but stream raw field is `atmp`. Pre-existing config validation gap, not an ops-003 regression. Both old and new binaries generate DDL with this field name.

### deploy.sh Config-Dir Verification

| Check | Before Fix | After Fix |
|-------|-----------|-----------|
| `--config-dir "$REPO_ROOT/config"` | Config root = `$REPO_ROOT` (WRONG) | N/A |
| `--config-dir "$REPO_ROOT/config/base"` | N/A | Config root = `$REPO_ROOT/config` (CORRECT) |
| Site 1 (handle_gold_table) | `$REPO_ROOT/config` | `$REPO_ROOT/config/base` |
| Site 2 (handle_domain aligned view) | `$REPO_ROOT/config` | `$REPO_ROOT/config/base` |
| Site 3 (handle_domain events) | `$REPO_ROOT/config` | `$REPO_ROOT/config/base` |

## Bugs

### BUG-004: deploy.sh config-dir mismatch
- **Severity**: Critical (would break all Gold operations on Pi)
- **Found**: Integration testing 2026-02-07
- **Fixed**: Same session. 3 deploy.sh sites updated.
- **Root cause**: ndp CLI convention is `--config-dir = config/base`, uses `.parent()` to reach config root. deploy.sh was passing `config/` directly.

## Phase 2 Verification Report (2026-02-08)

### Implementation Steps

| Step | Description | Status |
|------|-------------|--------|
| A+B | Module structure + deps (jsonschema, schemars, sqlparser, regex, strsim) | DONE |
| C | 11 source files migrated, `crate::` → `crate::validate::` import rewrite | DONE |
| C.1 | `is_valid_granularity()` deduplicated to `semantic/mod.rs` | DONE |
| C.2 | YAML code paths stripped (all configs JSON since v1.1.8) | DONE |
| D | `cli.rs` split — `result.rs` (library types) extracted to ndp-lib | DONE |
| D.2 | `ValidateOptions` struct + 6 public convenience functions | DONE |
| E | `ndp-validate` → thin wrapper (`pub use ndp_lib::validate::*`) | DONE |
| F | `ndp validate` CLI command with flat flags | DONE |
| H | deploy.sh switchover — 2 dispatch sites, no-fallback policy | DONE |
| I | Parity testing, exit codes, integration env validation | DONE |

### Test Results

| Package | Tests | Status |
|---------|-------|--------|
| ndp-lib (validate) | 675 | PASS |
| ndp-validate | 65 | PASS |
| ndp-cli | 0 | PASS (compile-only) |
| **Total** | **740** | **0 failures** |

### Output Parity

| Scenario | Status | Notes |
|----------|--------|-------|
| Single stream (JSON) | PASS | Same error/warning counts, codes, severity |
| Single stream (human) | PASS | Same line count, format |
| Single domain (JSON) | PASS | Byte-identical |
| All domains (JSON) | PASS | Both exit 0 |
| Schema generate | PASS | Byte-identical |
| All streams (JSON) | IMPROVED | Old `--all` was broken; new works correctly |

### Exit Codes

| Scenario | Expected | Actual |
|----------|----------|--------|
| Validation errors | 1 | 1 |
| System error (missing file) | 2 | 2 |
| Schema generate success | 0 | 0 |
| Valid domain config | 0 | 0 |

### deploy.sh Verification

| Check | Status |
|-------|--------|
| `ndp-validate` in dispatch sites | 0 references (PASS) |
| `ndp validate` dispatch at Site 1 (validate_domain_configs) | PASS |
| `ndp validate` dispatch at Site 2 (handle_domain) | PASS |
| No-fallback pattern (error + return 1) | PASS (2 sites) |
| `bash -n` syntax check | PASS |

### Integration Environment

| Test | Result |
|------|--------|
| `ndp validate --all` (4 streams) | PASS (62 errors, 4 warnings -- pre-existing config issues) |
| `ndp validate --domain-all` (1 domain) | PASS (0 errors, 0 warnings) |
| `ndp validate --schema --generate` | PASS (exit 0) |

### Key Decisions Made During Implementation

1. **Released as v1.1.17** (not v1.1.15) — v1.1.15/v1.1.16 used for Gold bug fixes
2. **Default format `json`** — aligned with standalone `ndp-validate` default
3. **`serde_yaml` removed** from ndp-validate — all configs JSON since v1.1.8
4. **Exit codes via `process::exit()`** in main.rs match arm — avoids changing return type for all commands
5. **`is_valid_granularity` dedup** — thin wrapper delegation pattern (local fn calls `super::`)

## Knowledge Captured
14 new AgentDB patterns (IDs 37-50) stored during Phase 1 planning:
- ID 37: specification:library-extraction-migration
- ID 38: testing:cli-parity
- ID 39: architecture:library-extraction-pattern
- ID 40: testing:crate-migration-test-strategy
- ID 41: architecture:dbclient-trait-unification
- ID 42: architecture:adr-003-001-gold-library-extraction
- ID 43: procedure:deploy-sh-safety-protocol
- ID 44: procedure:crate-module-migration
- ID 45: architecture:no-fallback-dispatch-policy
- ID 46-50: pseudocode patterns (crate-migration, cli-command-addition, dbclient-adaptation, deploy-sh-dispatch, gold-migration-v1.1.14)

Phase 2 patterns (IDs 1-4 in new AgentDB session):
- ID 1: development:crate-module-migration
- ID 2: development:ndp-cli-subcommand
- ID 3: deployment:deploy-sh-ndp-dispatch
- ID 4: procedure:crate-validate-migration

## Key References
- SCOPE: `product/features/ops-003/SCOPE.md`
- CLI UX: `product/research/deployment/10-CLI-UX-DESIGN-REVISED.md`
- Analysis: `product/features/ops-003/analysis/`

## Phase 3 Verification Report (2026-02-09)

### Implementation Steps (3-wave swarm, 5 agents)

| Wave | Step | Description | Status |
|------|------|-------------|--------|
| 1 | A+B | Constants module + wire 3 consumers | DONE — 7 tests, zero duplicates |
| 1 | F | NoOpDbClient dedup (3 copies → 1 import) | DONE — -69 lines, -2 deps |
| 1 | G | YAML retirement (7 files → .yaml.bak) | DONE — obsolete compat test removed |
| 2 | C+D | SyncOptions.validate + gold::sync wiring + --no-validate | DONE — 9 tests |
| 3 | E | ConfigValidator gap analysis + removal | DONE — 4 gaps ported, 4 tests |

### Gap Analysis (ConfigValidator vs validate_gold_etl)

| Check | ConfigValidator | validate_gold_etl | Action |
|-------|----------------|-------------------|--------|
| Lag lags_hours non-empty | Yes | No → Yes | Ported |
| Lag hours >= 1 | Yes | No → Yes | Ported |
| Rolling windows non-empty | Yes | No → Yes | Ported |
| Trend window non-empty | Yes | No → Yes | Ported |
| gold_etl exists/enabled | Yes | Skips (empty vec) | Handled upstream |
| Granularity format | Yes | Yes | Already covered |
| Field/metric validation | Yes | Yes | Already covered |

### Test Results

| Package | Tests | Status |
|---------|-------|--------|
| ndp-lib | 554 | PASS |
| ndp-validate | 65 | PASS |
| ndp-gold-ddl | 15 | PASS |
| ndp-cli | 4+ | PASS |
| ndp-types | 88+ | PASS |
| platform-core | 861 | PASS |
| **Total** | **874+** | **0 failures** |

### Clippy

`cargo clippy -p ndp-lib -p ndp-cli -p ndp-gold-ddl -p ndp-validate -- -D warnings` — **clean** (4 pre-existing issues also fixed)

### Net Impact

- **-2,568 lines** net (consolidation)
- **+24 new tests** (7 constants + 9 cross-cutting + 4 gap + 4 NoOpDbClient)
- **-9 ConfigValidator tests** (replaced by unified validation)
- **-2 ndp-cli dependencies** (async-trait, tokio-postgres)
- **0 deploy.sh changes**
- **0 new CLI flags**

### Design Decisions

1. **Re-serialization approach** (struct → JSON → validate) avoids dual validation paths
2. **Errors block, warnings pass** through gold sync
3. **Utility functions kept** in gold::validation (parse_granularity, parse_window, granularity_to_suffix — used by generators)
4. **SyncOptions::default() has validate=true** — all existing callers get validation automatically

## Last Updated
2026-02-09 — Phase 3 implementation complete, released as v1.1.20. ops-003 COMPLETE.
