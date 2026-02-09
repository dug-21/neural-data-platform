# OPS-003 Phase 3 Refinement: Shared Constants + Cross-cutting Validation

> **Feature**: ops-003 (Unified Action Library)
> **Release**: v1.1.18
> **Date**: 2026-02-08
> **Status**: Refinement
> **Patterns Used**: ID 1 (development:crate-module-migration), ID 3 (deployment:deploy-sh-ndp-dispatch), ID 4 (procedure:crate-validate-migration)

---

## 1. Implementation Order (Phase Diagram)

Phase 3 is internal consolidation. Zero deploy.sh changes. No new external behavior except:
`gold::sync_stream()` now validates config by default; `--no-validate` opt-out.

```
                    +---------------------+
                    | CHECKPOINT 0        |
                    | Baseline: 865 tests |
                    | cargo test          |
                    | --workspace         |
                    | (681 ndp-lib +      |
                    |  65 ndp-validate +  |
                    |  15 ndp-gold-ddl +  |
                    |  104 ndp-types)     |
                    +----------+----------+
                               |
                    +----------v----------+
                    | A. Create           |
                    | ndp_lib::constants   |
                    | module              |
                    | (London TDD: write  |
                    | 7 tests first)      |
                    +----------+----------+
                               |
                    +----------v----------+
                    | B. Wire gold +      |
                    | validate to use     |
                    | shared constants    |
                    | (replace local      |
                    | definitions with    |
                    | imports)            |
                    +----------+----------+
                               |
                    +----------v----------+
                    | CHECKPOINT 1        |
                    | All tests pass.     |
                    | Constants defined   |
                    | in exactly 1 place. |
                    | Values unchanged.   |
                    +----------+----------+
                               |
                    +----------v----------+
                    | C. Add validate     |
                    | field to            |
                    | SyncOptions +       |
                    | gold_config() fn    |
                    | (London TDD: test   |
                    | first)              |
                    +----------+----------+
                               |
                    +----------v----------+
                    | D. Wire gold::sync  |
                    | to call validate::  |
                    | gold_config()       |
                    | before generating   |
                    | DDL. Wire CLI       |
                    | --no-validate to    |
                    | SyncOptions.        |
                    +----------+----------+
                               |
                    +----------v----------+
                    | CHECKPOINT 2        |
                    | gold sync validates |
                    | by default.         |
                    | --no-validate works.|
                    +----------+----------+
                               |
                    +----------v----------+
                    | E. Remove gold::    |
                    | validation::        |
                    | ConfigValidator     |
                    | (gap analysis       |
                    | FIRST; verify       |
                    | validate module     |
                    | covers all checks)  |
                    +----------+----------+
                               |
                    +----------v----------+
                    | CHECKPOINT 3        |
                    | All gold tests pass |
                    | with unified        |
                    | validation.         |
                    | --validate-only     |
                    | still works.        |
                    +----------+----------+
                               |
                    +----------v----------+
                    | F. Consolidate      |
                    | NoOpDbClient        |
                    | (3 CLI copies -> 1  |
                    | import from         |
                    | ndp_lib::db)        |
                    +----------+----------+
                               |
                    +----------v----------+
                    | G. Retire YAML      |
                    | stream configs      |
                    | (7 files renamed    |
                    | to .yaml.bak)       |
                    +----------+----------+
                               |
                    +----------v----------+
                    | CHECKPOINT 4        |
                    | All tests pass.     |
                    | Integration env     |
                    | validates.          |
                    | cargo clippy clean. |
                    +----------+----------+
                               |
                    +----------v----------+
                    | H. Release          |
                    | v1.1.18             |
                    | (manifest, tag,     |
                    | CHANGELOG)          |
                    +----------+----------+
```

### Step Dependencies

| Step | Depends On | Can Parallel With | Estimated Effort |
|------|-----------|-------------------|------------------|
| A | None | None | Small (1 file, 7 tests) |
| B | A | None | Small (edit import paths in 4 files) |
| C | A | None | Small (1 struct change + 1 function) |
| D | B + C | None | Medium (wire gold::sync, wire CLI --no-validate) |
| E | D | None | Medium (gap analysis + removal) |
| F | None (independent) | A, B, C, D, E | Small (delete + import in 3 files) |
| G | None (independent) | A through F | Small (7 file renames) |
| H | All checkpoints | None | Small (3 artifacts) |

### What Can Be Done in Parallel

- **F (NoOpDbClient)** is independent of the constants/validation work (A-E).
- **G (YAML retirement)** is independent of all code changes.
- In practice, F and G can be done at any time after Checkpoint 0.

### Point of No Return

There is no true point of no return in Phase 3. All changes are internal to ndp-lib and ndp-cli. deploy.sh is untouched. Every step is individually revertable with `git checkout`.

The closest to a "no return" point is **Step E** (ConfigValidator removal): once removed, any code path that imported `ConfigValidator` directly must use the validate module instead. But since ndp-gold-ddl's `lib.rs` re-exports `validation::ConfigValidator` from ndp-lib, this affects the thin wrapper's public API. Mitigation: keep a deprecated `ConfigValidator` type alias if needed.

---

## 2. Risk Register

### Risk 1: Constants Value Drift During Extraction

**Likelihood**: Low | **Impact**: High

**Root Cause**: Copying the constant values to `constants.rs` and failing to exactly match the original arrays. Even a reordering would not break behavior (since `.contains()` is order-independent), but a missing element would.

**Current Values (canonical)**:
```
VALID_METRICS:        ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"]  (9 items)
VALID_ROLLING_STATS:  ["mean", "std", "min", "max"]  (4 items)
VALID_STATS:          ["mean", "std", "min", "max"]  (4 items -- identical to VALID_ROLLING_STATS)
GOLD_SCHEMA:          "gold"
SILVER_SCHEMA:        "silver"
NDP_ENTITY_COLUMN:    "ndp_id"
```

**Mitigation**:
1. London TDD: Write tests asserting exact values BEFORE creating the constants.
2. Tests explicitly check `VALID_METRICS.len() == 9` and enumerate all expected values.
3. After extraction, verify with `cargo test -p ndp-lib -- constants` before proceeding.
4. Use diff to verify old definition sites produce identical values.

**Rollback**: Revert `constants.rs` and restore original definitions in `gold/config/types.rs` and `validate/semantic/gold.rs`.

### Risk 2: Cross-cutting Validation Breaks Gold Sync

**Likelihood**: Medium | **Impact**: High

**Root Cause**: Wiring `validate::semantic::gold::validate_gold_etl()` into `gold::sync_stream()` means configs that previously generated DDL (because the generator tolerated them) now fail at validation time. This is a behavioral change.

**Known example**: The Phase 1 integration test found `FieldNotFound { field: "temperature_c" }` -- the gold_etl config references `temperature_c` but the stream raw field is `atmp`. The generator produces DDL with this field name (it does not cross-reference stream fields). The semantic validator DOES cross-reference and will flag this as an error.

**Mitigation**:
1. Default `SyncOptions.validate` to `true`, but ensure `--no-validate` is properly wired so deploy.sh can opt out if needed.
2. The `validate::semantic::gold::validate_gold_etl()` operates on `serde_json::Value`, but `gold::sync_stream()` has a typed `StreamConfig`. Two options:
   a. Re-serialize the typed config to JSON and call `validate_gold_etl()`.
   b. Create a new `validate::gold_config_typed()` that takes the typed struct.
   Decision D1 (Section 3) addresses this.
3. Integration test with known-invalid config to verify error surfaces at validation, not at psql.
4. Integration test with `--no-validate` to verify bypass works.

**Rollback**: Set `SyncOptions.validate` default to `false`. This restores pre-Phase-3 behavior.

### Risk 3: ConfigValidator Removal Gap

**Likelihood**: Medium | **Impact**: Medium

**Root Cause**: `gold::validation::ConfigValidator` validates typed `StreamConfig` structs. `validate::semantic::gold::validate_gold_etl()` validates `serde_json::Value`. They check overlapping but not identical things.

**Gap Analysis (performed from reading source)**:

| Check | ConfigValidator | validate_gold_etl | Notes |
|-------|:--------------:|:-----------------:|-------|
| gold_etl exists | Yes (MissingRequiredField) | No (returns empty if absent) | ConfigValidator is stricter |
| gold_etl.enabled | Yes (GoldEtlDisabled) | No (returns empty if disabled) | ConfigValidator is stricter |
| Granularity format | Yes (parse_granularity) | Yes (is_valid_granularity) | Equivalent |
| Aggregate field exists in stream | Yes (FieldNotFound) | Yes (InvalidGoldField) | Equivalent |
| Metric is valid | Yes (VALID_METRICS check) | Yes (VALID_METRICS check) | Equivalent |
| Lag enabled + empty lags_hours | Yes (InvalidFeatureConfig) | No (not checked) | ConfigValidator covers more |
| Lag hours >= 1 | Yes (InvalidFeatureConfig) | No (not checked) | ConfigValidator covers more |
| Rolling enabled + empty windows | Yes (InvalidFeatureConfig) | No (not checked) | ConfigValidator covers more |
| Rolling window format | Yes (parse_window) | Yes (is_valid_granularity) | Equivalent |
| Rolling stats valid | Yes (VALID_ROLLING_STATS) | Yes (VALID_STATS) | Equivalent |
| Trend enabled + empty window | Yes (InvalidFeatureConfig) | No (not checked) | ConfigValidator covers more |
| Trend window format | Yes (parse_window) | Yes (is_valid_granularity) | Equivalent |
| Transitions on non-state_event | No | Yes (Warning) | validate_gold_etl covers more |
| Transitions state_field exists | No | Yes (InvalidGoldField) | validate_gold_etl covers more |
| Transitions entity_field exists | No | Yes (InvalidGoldField) | validate_gold_etl covers more |
| Lag field references | No | Yes (InvalidGoldField) | validate_gold_etl covers more |
| Rolling field references | No | Yes (InvalidGoldField) | validate_gold_etl covers more |
| Trend field references | No | Yes (InvalidGoldField) | validate_gold_etl covers more |
| default_metrics validation | No | Yes (InvalidAggregateMetric) | validate_gold_etl covers more |
| "Did you mean" suggestions | No | Yes (Levenshtein) | validate_gold_etl richer |

**Conclusion**: ConfigValidator has 5 checks not in validate_gold_etl:
1. gold_etl existence check
2. gold_etl.enabled check
3. Lag lags_hours non-empty
4. Lag hours >= 1
5. Trend/Rolling window non-empty when enabled

These must be added to `validate_gold_etl()` BEFORE removing ConfigValidator.

**Mitigation**:
1. Before Step E, add the 5 missing checks to `validate::semantic::gold::validate_gold_etl()`.
2. Write tests for each gap (London TDD).
3. Run both validators in parallel on test configs and compare outputs.
4. Only remove ConfigValidator after all gap tests pass.

**Rollback**: Keep ConfigValidator. It still works. Simply do not delete it.

### Risk 4: NoOpDbClient Trait Incompatibility

**Likelihood**: Low | **Impact**: Low

**Root Cause**: The 3 CLI copies of `NoOpDbClient` use `unreachable!()` for all methods. The canonical `ndp_lib::db::NoOpDbClient` returns `Ok(vec![])`, `Ok(0)`, and `Ok(())`. The behavioral difference matters only if the dry-run code path actually calls these methods.

**Analysis**: The CLI commands use `NoOpDbClient` for dry-run mode. The sync functions (`dictionary::sync_dictionary`, `dimension::sync_dimension`, `domain::sync_domains`) accept `&impl DbClient` and the dry-run branch typically skips DB calls entirely. However, `sync_stream()` uses `CaChecker`, which calls `DbClient::query()` to check existing continuous aggregates. If `NoOpDbClient` returns `Ok(vec![])` for queries, `SyncPlanner` will think no CAs exist and generate full DDL. The `unreachable!()` version would panic.

The ndp-lib `NoOpDbClient` (returning empty results) is correct for gold sync dry-run: it generates DDL for all CAs (treating none as existing). The CLI gold command already handles dry-run by calling `generate_stream()` instead of `sync_stream()`, so `NoOpDbClient` is never passed to `sync_stream()`. For dictionary/dimension/domain, dry-run skips DB execution internally.

**Mitigation**: Replace CLI copies with `use ndp_lib::NoOpDbClient`. Verify all dry-run paths with `cargo test -p ndp-cli`.

**Rollback**: Keep local copies. They work.

### Risk 5: YAML Retirement Breaks Active Code

**Likelihood**: Low | **Impact**: Medium

**Root Cause**: Renaming `config.yaml` to `config.yaml.bak` in `config/base/streams/` could break code that discovers configs by looking for `*.yaml`.

**Analysis of .yaml references in Rust source**:
- `apps/air-quality-app/src/config_sync/service.rs` (line 252): DP-018 prefers `config.json` over `config.yaml`. It checks for `config.json` first; if found, skips `config.yaml`. Since all 7 streams have `config.json`, the YAML fallback is never exercised.
- `apps/air-quality-app/src/main.rs` (line 97): References `config.yaml` for AppConfig, NOT stream configs. This is `config/base/platform.yaml` (or app-level), which is NOT being retired.
- `archive/legacy-config-store/src/loaders/gitops.rs` (line 123): Archived code. Not in workspace build.
- `scripts/sync_config.rs` (line 31): Script, not compiled.
- `crates/ndp-lib/src/` -- Zero `.yaml` references. Confirmed via grep.

**Mitigation**:
1. Only rename stream config YAMLs under `config/base/streams/*/config.yaml`. Do NOT touch `config/base/platform.yaml`, `config/overlays/**/*.yaml`, `config/base/processors/*.yaml`, or Grafana configs.
2. Run integration test after rename to verify no code path fails.
3. Rename (not delete) so rollback is trivial.

**Rollback**: `rename *.yaml.bak -> *.yaml`. Single command.

### Risk 6: ndp-gold-ddl Thin Wrapper Public API Break

**Likelihood**: Medium | **Impact**: Low

**Root Cause**: `tools/ndp-gold-ddl/src/lib.rs` line 35 re-exports `ndp_lib::gold::validation::{validate_gold_config, ConfigValidator}`. If ConfigValidator is removed in Step E, this re-export breaks compilation.

**Mitigation**:
1. Update `ndp-gold-ddl/src/lib.rs` to remove the `ConfigValidator` re-export.
2. Keep `validate_gold_config` function (it remains; only the struct is removed).
3. If external consumers depend on `ConfigValidator`, provide a type alias: `pub type ConfigValidator = ()` with a deprecation warning. But since ndp-gold-ddl is only used as a thin wrapper and no external consumers exist, this is not needed.

**Rollback**: Restore the re-export line.

---

## 3. Decision Points

### Decision D1: How Does gold::sync() Call validate_gold_etl()?

**Context**: `gold::sync_stream()` has a typed `gold::config::StreamConfig`. `validate::semantic::gold::validate_gold_etl()` takes `&serde_json::Value`.

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: Re-serialize typed config to JSON, call validate_gold_etl()** | No new function. Single validation path. | Wasteful round-trip (serialize then parse). StreamConfig must implement Serialize. |
| **B: Create validate::gold_config_typed(config: &gold::StreamConfig)** | Type-safe. No serialization overhead. Direct struct access. | Duplicates validation logic for two input types (JSON vs typed). |
| **C: Keep ConfigValidator for typed validation, add cross-cutting call in sync** (recommended) | Minimal change. ConfigValidator already works on typed structs. validate_gold_etl stays for JSON validation. | Two validators remain (contradicts unification goal). |
| **D: Convert gold::sync to load raw JSON, validate first, then parse to typed** | Single validation path. Clean flow. | Requires restructuring gold::sync to read JSON first. Current flow uses ConfigLoader which returns typed structs. |

**Recommendation**: Option A, with a small twist. `gold::config::StreamConfig` already derives `Serialize` (via serde). The re-serialization cost is negligible (small JSON object, done once at sync start, not in a hot loop). This keeps a single validation path and lets us fully remove ConfigValidator.

```rust
// In gold/mod.rs sync_stream():
if opts.validate {
    let config_json = serde_json::to_value(&stream_config)?;
    let errors = crate::validate::semantic::gold::validate_gold_etl(&config_json);
    if !errors.is_empty() {
        return Err(...);
    }
}
```

**Decision**: Option A. Re-serialize and validate through the canonical path.

### Decision D2: Does --no-validate Apply to generate as Well as sync?

**Context**: The CLI already has `--no-validate` on Generate, Sync, and Recreate. Currently all are captured but discarded (`no_validate: _`).

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: Only wire for sync and recreate** (recommended) | Generate does not mutate. Validation before generation is defensive but not critical. | Inconsistency: flag exists on generate but does nothing different. |
| **B: Wire for all three** | Consistent. Every `--no-validate` flag has meaning. | Generate without validation means broken SQL could be produced silently. |
| **C: Remove --no-validate from generate, keep on sync/recreate** | Clean CLI. No meaningless flags. | Breaking change to CLI interface (flag removal). Minor since deploy.sh does not use --no-validate. |

**Recommendation**: Option A. Wire `--no-validate` for `sync` and `recreate` only. The `generate` command currently does not call validation anyway (it calls generators directly). The `--no-validate` flag on `generate` is a no-op today and remains a no-op. In a future release, we can add validation to `generate` and then `--no-validate` becomes meaningful. No flag removal avoids a breaking change.

**Decision**: Wire --no-validate for sync and recreate. Generate unchanged.

### Decision D3: Error Type for Cross-cutting Validation Failure

**Context**: `validate_gold_etl()` returns `Vec<ValidationError>`. `gold::sync_stream()` returns `Result<String, Box<dyn Error>>`. The validation errors need to be converted.

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: Format errors into a single String, wrap in Box\<dyn Error\>** (recommended) | Simple. Matches existing error handling. CLI prints the error message. | Loses structured error data (codes, paths, suggestions). |
| **B: Return a new NdpLibError::ValidationFailed(Vec\<ValidationError\>)** | Structured. CLI could format differently. | Adds a new error variant. NdpLibError would need to depend on validate::error types. Coupling. |
| **C: Return Vec\<ValidationError\> alongside DDL as a compound result** | Most flexible. | Requires changing the return type of sync_stream(). Breaking change to library API. |

**Recommendation**: Option A. The error message includes the validation error details in human-readable format. Example:

```
Gold configuration validation failed for stream 'air-quality':
  - [E400] $.gold_etl.aggregates.fields.temperature_c: Field 'temperature_c' not found in stream
  - [E403] $.gold_etl.aggregates.default_metrics[1]: Invalid metric 'avg'
```

**Decision**: Option A. Format validation errors into a descriptive string.

### Decision D4: Should YAML Retirement Be a Script or Manual?

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: Manual rename** (recommended) | Simple. Explicit. Visible in git diff. | 7 commands to type. |
| **B: Bash script** | Repeatable. Could be used in CI. | Over-engineering for a one-time operation. |

**Recommendation**: Option A. Manual `git mv` for each of the 7 files. This ensures each rename appears in git history and the diff is reviewable.

**Decision**: Manual rename via `git mv`.

### Decision D5: Where Does validate::gold_config() Bridge Function Live?

**Context**: We need a function that takes the output of `validate_gold_etl()` (a `Vec<ValidationError>`) and converts it to a `Result<(), Box<dyn Error>>` suitable for gold::sync's error handling.

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: In gold/mod.rs as a private helper** (recommended) | Close to usage. No new public API. | Logic in gold module references validate types. |
| **B: In validate/mod.rs as gold_config_check()** | Clean separation. Validate module owns validation. | gold module must call validate module explicitly. Already the plan. |
| **C: In a new cross/ module** | Clear cross-cutting concern. | Over-engineering for one function. |

**Recommendation**: Option A. A 10-line private function in `gold/mod.rs`:

```rust
fn check_gold_config(config: &config::StreamConfig) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_value(config)?;
    let errors = crate::validate::semantic::gold::validate_gold_etl(&json);
    if errors.is_empty() {
        Ok(())
    } else {
        let msg = format_validation_errors(&errors, &config.stream_id);
        Err(msg.into())
    }
}
```

**Decision**: Private helper in `gold/mod.rs`.

---

## 4. London TDD Strategy

### Outside-In Testing Order

Phase 3 tests are written BEFORE implementation, from the outside in:

```
Layer 1 (Acceptance):  CLI behavior tests
    |
    v
Layer 2 (Module):      gold::sync with mock validator
    |
    v
Layer 3 (Unit):        Constants values, NoOpDbClient conformance
```

### Step A: Constants Module (7 unit tests)

Write these tests FIRST in `crates/ndp-lib/src/constants.rs`:

```rust
#[test] fn test_valid_metrics_contains_expected_values()     // 9 exact values
#[test] fn test_valid_metrics_count()                        // len == 9
#[test] fn test_valid_rolling_stats_contains_expected()      // 4 exact values
#[test] fn test_valid_rolling_stats_count()                  // len == 4
#[test] fn test_gold_schema_value()                          // "gold"
#[test] fn test_silver_schema_value()                        // "silver"
#[test] fn test_ndp_entity_column_value()                    // "ndp_id"
```

Red: Tests fail (module does not exist). Green: Add `constants.rs`. Refactor: Remove old definitions from `gold/config/types.rs` and `validate/semantic/gold.rs`.

### Step B: Import Wiring (no new tests -- existing tests prove correctness)

After replacing local definitions with imports from `crate::constants`, run:

```bash
cargo test -p ndp-lib -- gold::validation     # ConfigValidator still works
cargo test -p ndp-lib -- validate::semantic::gold  # validate still works
cargo test -p ndp-lib -- gold::generators      # CA generator still works
```

All existing tests must pass with zero changes to test assertions.

### Step C: SyncOptions + gold_config() (3 new tests)

```rust
// In types.rs test module:
#[test] fn test_sync_options_default_validate_true()

// In gold/mod.rs test module:
#[test] fn test_sync_stream_with_invalid_config_returns_error()
#[test] fn test_sync_stream_with_no_validate_skips_validation()
```

Red: `SyncOptions` has no `validate` field. Green: Add field, default `true`. Wire in gold::sync.

### Step D: CLI --no-validate Wiring (2 integration-level tests)

```bash
# Manual CLI tests (not automated unit tests -- requires built binary):
ndp gold sync --stream air-quality --no-validate --dry-run --config-dir config/base --db-url "postgresql://..."
# Expected: generates DDL even if config has issues

ndp gold sync --stream air-quality --dry-run --config-dir config/base --db-url "postgresql://..."
# Expected: validation runs first; if errors, prints them and exits 1
```

### Step E: ConfigValidator Removal (gap tests first)

Write 5 tests for the gap checks BEFORE adding them to `validate_gold_etl()`:

```rust
#[test] fn test_gold_etl_missing_returns_no_errors()          // Already passes (existing behavior)
#[test] fn test_gold_etl_disabled_returns_no_errors()          // Already passes
#[test] fn test_lag_empty_hours_flagged()                      // NEW -- currently not checked
#[test] fn test_lag_zero_hours_flagged()                       // NEW
#[test] fn test_rolling_empty_windows_flagged()                // NEW
#[test] fn test_trend_empty_window_flagged()                   // NEW
```

Red: 4 of 6 fail (validate_gold_etl does not check these). Green: Add checks.

Then remove ConfigValidator and verify the existing 19 ConfigValidator tests either:
- Have equivalents in `validate::semantic::gold::tests`, or
- Are migrated to validate_gold_etl test suite.

### Step F: NoOpDbClient Consolidation (0 new tests)

Replace 3 local definitions with `use ndp_lib::NoOpDbClient`. All existing tests pass unchanged because:
- Dictionary dry-run: `sync_dictionary` with `NoOpDbClient` never calls DB methods.
- Dimension dry-run: `sync_dimension` with `NoOpDbClient` never calls DB methods.
- Domain dry-run: `sync_domains` with `NoOpDbClient` never calls DB methods.

Verify: `cargo build -p ndp-cli`.

### Step G: YAML Retirement (0 tests)

No tests needed. Git tracks the rename. Verify with:
```bash
ls config/base/streams/*/config.yaml      # Should return nothing
ls config/base/streams/*/config.yaml.bak  # Should return 7 files
```

### Red-Green-Refactor Summary

| Step | Red (write test) | Green (make pass) | Refactor |
|------|-------------------|-------------------|----------|
| A | 7 constant value tests | Create constants.rs | -- |
| B | (existing tests) | Replace local defs with imports | Remove old constant definitions |
| C | 3 SyncOptions + validation tests | Add validate field, wire sync | -- |
| D | (manual CLI tests) | Wire --no-validate in gold.rs | -- |
| E | 4 gap tests + 2 existing | Add missing checks to validate_gold_etl | Remove ConfigValidator struct |
| F | (existing build) | Replace CLI NoOpDbClient with import | Delete 75 lines of boilerplate |
| G | (manual ls check) | git mv 7 files | -- |

---

## 5. Definition of Done

### ops-003-08: Shared Constants

- [ ] `VALID_METRICS` defined in exactly one place: `crates/ndp-lib/src/constants.rs`
- [ ] `VALID_ROLLING_STATS` defined in exactly one place: `crates/ndp-lib/src/constants.rs`
- [ ] `GOLD_SCHEMA` defined in exactly one place: `crates/ndp-lib/src/constants.rs`
- [ ] `SILVER_SCHEMA` defined in exactly one place: `crates/ndp-lib/src/constants.rs`
- [ ] `NDP_ENTITY_COLUMN` defined in exactly one place: `crates/ndp-lib/src/constants.rs`
- [ ] No `const VALID_METRICS` in `gold/config/types.rs`
- [ ] No `const VALID_METRICS` in `validate/semantic/gold.rs`
- [ ] No `const VALID_STATS` in `validate/semantic/gold.rs`
- [ ] `gold/generators/constants.rs` either removed or re-exports from `crate::constants`
- [ ] 7 unit tests for constant values passing

Verification:
```bash
grep -rn "const VALID_METRICS" crates/ndp-lib/src/
# Expected: exactly 1 result (constants.rs)

grep -rn "const VALID_ROLLING_STATS" crates/ndp-lib/src/
# Expected: exactly 1 result (constants.rs)

grep -rn "const VALID_STATS" crates/ndp-lib/src/
# Expected: 0 results (renamed to VALID_ROLLING_STATS)

grep -rn "const GOLD_SCHEMA" crates/ndp-lib/src/
# Expected: exactly 1 result (constants.rs)
```

### ops-003-09: Cross-cutting Validation

- [ ] `SyncOptions` has `validate: bool` field, default `true`
- [ ] `gold::sync_stream()` calls validation before generating DDL when `opts.validate == true`
- [ ] `gold::sync_stream()` skips validation when `opts.validate == false`
- [ ] CLI `ndp gold sync --no-validate` maps to `SyncOptions { validate: false, .. }`
- [ ] CLI `ndp gold recreate --no-validate` maps to `SyncOptions { validate: false, .. }`
- [ ] Invalid config + `gold sync` (without --no-validate) returns validation error, not DDL
- [ ] Invalid config + `gold sync --no-validate` returns DDL (bypasses validation)

Verification:
```bash
grep -n "opts.validate" crates/ndp-lib/src/gold/mod.rs
# Expected: conditional check before DDL generation

grep -n "no_validate: _" tools/ndp-cli/src/commands/gold.rs
# Expected: 0 results (no longer discarded)
```

### ops-003-10: Gold Validation Unification

- [ ] `gold::validation::ConfigValidator` struct removed
- [ ] `gold::validation::validate_gold_config()` function kept (delegates to shared constants)
- [ ] `validate::semantic::gold::validate_gold_etl()` has 5 additional checks (gap from ConfigValidator)
- [ ] All 19 ConfigValidator tests pass through validate_gold_etl or equivalent
- [ ] `gold::generators::ContinuousAggregateGenerator` metric check unchanged (uses constants)
- [ ] `ndp gold generate --validate-only` still works (uses validate_gold_etl path)

Verification:
```bash
grep -rn "ConfigValidator" crates/ndp-lib/src/gold/
# Expected: 0 results (struct removed)

cargo test -p ndp-lib -- gold::validation
# Expected: all tests pass (parsing functions remain)
```

### ops-003-11: NoOpDbClient Dedup

- [ ] `NoOpDbClient` defined in exactly one place: `crates/ndp-lib/src/db.rs`
- [ ] `tools/ndp-cli/src/commands/dictionary.rs` uses `ndp_lib::NoOpDbClient`
- [ ] `tools/ndp-cli/src/commands/dimension.rs` uses `ndp_lib::NoOpDbClient`
- [ ] `tools/ndp-cli/src/commands/domain.rs` uses `ndp_lib::NoOpDbClient`
- [ ] No `struct NoOpDbClient` in ndp-cli source
- [ ] No `unreachable!()` in ndp-cli DbClient implementations

Verification:
```bash
grep -rn "struct NoOpDbClient" tools/ndp-cli/src/
# Expected: 0 results

grep -rn "unreachable.*NoOpDbClient" tools/ndp-cli/src/
# Expected: 0 results
```

### ops-003-13: Retire YAML Stream Configs

- [ ] 7 files renamed from `config.yaml` to `config.yaml.bak` under `config/base/streams/`
- [ ] `config/base/platform.yaml` unchanged (NOT retired)
- [ ] `config/overlays/**/*.yaml` unchanged (NOT retired)
- [ ] `config/base/processors/*.yaml` unchanged (NOT retired)
- [ ] `config/grafana/**/*.yaml` unchanged (NOT retired)
- [ ] No Rust source code references `config.yaml` in ndp-lib or ndp-cli

Files to rename:
```
config/base/streams/air-quality/config.yaml          -> config.yaml.bak
config/base/streams/outdoor-weather/config.yaml      -> config.yaml.bak
config/base/streams/outdoor-air-quality/config.yaml  -> config.yaml.bak
config/base/streams/home-assistant-state/config.yaml -> config.yaml.bak
config/base/streams/nws-forecast-hourly/config.yaml  -> config.yaml.bak
config/base/streams/nws-observations/config.yaml     -> config.yaml.bak
config/base/streams/nws-gridpoints-forecast/config.yaml -> config.yaml.bak
```

Verification:
```bash
ls config/base/streams/*/config.yaml 2>/dev/null
# Expected: ls: cannot access ...: No such file or directory

ls config/base/streams/*/config.yaml.bak | wc -l
# Expected: 7
```

### Global Phase 3 Completion

- [ ] `cargo test --workspace` passes (target: 865+ tests, 0 failures)
- [ ] `cargo clippy --workspace` clean (0 warnings)
- [ ] `cargo build -p ndp-cli` succeeds
- [ ] `cargo build -p ndp-gold-ddl` succeeds (thin wrapper still compiles)
- [ ] `cargo build -p ndp-validate` succeeds (thin wrapper still compiles)
- [ ] Integration environment validates (Section 6)
- [ ] Release artifacts created (manifest, CHANGELOG, tag)

---

## 6. Integration Test Checklist

### Pre-Integration (Unit/Parity Tests)

```bash
# 1. Baseline: all workspace tests pass
cargo test --workspace
# Expected: 865+ passed, 0 failed

# 2. Constants are singular
grep -rn "const VALID_METRICS" crates/ndp-lib/src/ | wc -l
# Expected: 1

# 3. NoOpDbClient is singular
grep -rn "struct NoOpDbClient" tools/ndp-cli/src/ | wc -l
# Expected: 0

# 4. ConfigValidator is removed
grep -rn "pub struct ConfigValidator" crates/ndp-lib/src/ | wc -l
# Expected: 0

# 5. --no-validate is wired
grep -n "no_validate: _" tools/ndp-cli/src/commands/gold.rs | wc -l
# Expected: 0

# 6. Build all binaries
cargo build -p ndp-cli -p ndp-gold-ddl -p ndp-validate

# 7. Clippy clean
cargo clippy --workspace -- -D warnings
```

### Integration Environment Tests (Live TimescaleDB)

```bash
# 1. Start integration stack
docker compose -f docker-compose.integration.yml up -d

# 2. Wait for TimescaleDB
docker compose -f docker-compose.integration.yml exec timescaledb \
  pg_isready -U postgres -d ndp

# 3. Build ndp binary
cargo build -p ndp-cli

# 4. Gold sync with VALID config -> succeeds
./target/debug/ndp gold sync --stream air-quality \
  --config-dir config/base \
  --db-url "postgresql://postgres:postgres@localhost:5432/ndp" \
  --db-timeout 10
# Expected: DDL output (may skip existing CAs), exit 0

# 5. Gold sync with INVALID config -> validation error
# Create a temp config with invalid metric
TMPDIR=$(mktemp -d)
mkdir -p "$TMPDIR/base/streams/test-invalid"
cat > "$TMPDIR/base/streams/test-invalid/config.json" << 'EOF'
{
  "stream_id": "test-invalid",
  "fields": [{"name": "pm25", "type": "float"}],
  "gold_etl": {
    "enabled": true,
    "aggregates": {
      "granularities": ["1 hour"],
      "fields": { "nonexistent": { "metrics": ["avg"] } }
    }
  }
}
EOF
./target/debug/ndp gold sync --stream test-invalid \
  --config-dir "$TMPDIR/base" \
  --db-url "postgresql://postgres:postgres@localhost:5432/ndp" \
  --db-timeout 10
# Expected: validation error message, exit 1
echo "Exit: $?"

# 6. Gold sync --no-validate with INVALID config -> DDL generated (bypasses validation)
./target/debug/ndp gold sync --stream test-invalid \
  --config-dir "$TMPDIR/base" \
  --db-url "postgresql://postgres:postgres@localhost:5432/ndp" \
  --db-timeout 10 \
  --no-validate
# Expected: DDL output (may contain broken SQL), exit 0
echo "Exit: $?"

# 7. Full deploy.sh apply with manifest -> all phases complete
DEPLOY_ENV=integration \
  ./deploy.sh apply .deploy/releases/v1.1.18.manifest.json
# Expected: all phases complete (Phase 3 has no deploy.sh changes)

# 8. Validate all streams still pass
./target/debug/ndp validate --all --config-dir config/base --format human
# Expected: same results as pre-Phase-3 (validation logic unchanged)

# 9. Validate domain still passes
./target/debug/ndp validate --domain-all --config-dir config/base --format human
# Expected: 0 errors for indoor-air-quality domain

# 10. Verify Gold tables still exist
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c \
  "SELECT view_schema, view_name FROM timescaledb_information.continuous_aggregates WHERE view_schema = 'gold'"
# Expected: existing CAs present

# 11. Tear down
docker compose -f docker-compose.integration.yml down
rm -rf "$TMPDIR"
```

---

## 7. Lessons Applied from Phase 1 and Phase 2

### Lesson 1: BUG-004 -- config-dir Path Mismatch

**Phase 1 Bug**: deploy.sh passed `$REPO_ROOT/config` but ndp CLI expected `config/base`. Three dispatch sites had to be fixed.

**Phase 3 Application**: Phase 3 has zero deploy.sh changes, so this class of bug cannot recur. However, the cross-cutting validation in `gold::sync_stream()` needs access to the config as JSON. The config is loaded by `FileSystemConfigLoader` from the config-dir path. The validation path does not need to re-resolve config-dir because the typed config is already loaded and gets re-serialized. No path handling is involved in the validation bridge.

**Verdict**: Not applicable to Phase 3. No action needed.

### Lesson 2: Silently Ignored Flags

**Phase 1 Bug**: `--events` + `--stream` was silently ignored instead of erroring.
**Phase 2 Bug**: `--no-validate` captured but discarded with `_` in destructuring.

**Phase 3 Application**: This IS the primary fix in Phase 3. The `no_validate` field is currently captured as `no_validate: _` in `tools/ndp-cli/src/commands/gold.rs` at lines 123, 134, 143. Step D wires it to `SyncOptions { validate: !no_validate }`.

**Verification**: After Step D, confirm:
```bash
grep -n "no_validate: _" tools/ndp-cli/src/commands/gold.rs
# Expected: 0 results
```

### Lesson 3: Unimplemented Flag Captured as `_`

**Phase 1 Bug**: `--validate-only` flag was declared but not wired.

**Phase 3 Application**: Phase 3 does not add new flags. The `--no-validate` flag already exists and just needs wiring. However, `run_validate_only()` in gold.rs (line 207) currently calls `ndp_lib::gold::validation::ConfigValidator::new().validate(&config)?`. After Step E removes ConfigValidator, this must be updated to call the validate module instead.

**Mitigation**: In Step E, update `run_validate_only()` to:
```rust
let json = serde_json::to_value(&config)?;
let errors = ndp_lib::validate::semantic::gold::validate_gold_etl(&json);
if errors.is_empty() {
    println!("Stream '{}' Gold ETL configuration is valid", stream_id);
} else {
    // Format and return errors
}
```

### Lesson 4: Tracing to stdout

**Phase 1 Fix**: ndp CLI wrote tracing logs to stdout, breaking parity.

**Phase 3 Application**: Already fixed in v1.1.14. No additional work needed.

### Lesson 5: Integration Environment Required

**Phase 1 Finding**: Integration E2E testing found 3 bugs that unit tests did not catch.
**Phase 2 Finding**: Integration E2E confirmed parity. Zero bugs found.

**Phase 3 Application**: Checkpoints 2 and 4 require integration testing. Specifically:
- Checkpoint 2: Cross-cutting validation must be tested against a real config that would previously pass generation but fail validation. The `temperature_c` field reference in air-quality config is such a case.
- Checkpoint 4: Full deploy.sh apply to verify Phase 3 did not regress Phase 1/2 deploy behavior.

### Lesson 6: Golden Master Fixtures Must Be Copied

**Phase 1 Bug**: SQL fixtures not moved to new test location.

**Phase 3 Application**: Not applicable. Phase 3 does not move files between crates.

### Lesson 7: Move Order Matters

**Phase 1 Lesson**: Files should be moved in dependency order (leaves first).
**Phase 2 Lesson**: Same -- error.rs first, then schema, then semantic.

**Phase 3 Application**: Phase 3 does not move files. The implementation order (A-G) is ordered by dependency, not file location. Constants (A) must be created before imports are wired (B). SyncOptions change (C) must exist before gold::sync wiring (D). Gap tests (E) must pass before ConfigValidator removal.

### Lesson 8: Convenience API Added Late

**Phase 1 Finding**: ndp_lib::gold lacked convenience functions expected by CLI.

**Phase 3 Application**: Phase 3 does not add new public convenience functions. The cross-cutting validation is internal to `sync_stream()`. The `gold_config()` bridge is a private helper. No new public API surface.

### Lesson 9: YAML Dead Code (Phase 2 Discovery)

**Phase 2 Action**: Stripped `serde_yaml` and YAML code paths from ndp-validate since all configs have been JSON since v1.1.8.

**Phase 3 Application**: Step G retires the actual YAML files, completing the cleanup started in Phase 2. This eliminates the source of confusion that Phase 2's YAML code removal addressed from the code side.

### Lesson 10: is_valid_granularity Dedup (Phase 2 Resolution)

**Phase 2 Action**: Extracted `is_valid_granularity()` to `semantic/mod.rs` so both `gold.rs` and `domain.rs` call the shared implementation.

**Phase 3 Application**: This pattern (extract shared logic to parent module) is the same pattern used in Step A (extract shared constants to `constants.rs`). The approach is proven.

---

## 8. Release Preparation

### Version Number

**v1.1.18** -- next available after v1.1.17 (Phase 2 release).

SCOPE.md originally labeled Phase 3 as v1.1.16, but v1.1.15 and v1.1.16 were consumed by Gold bug fixes shipped between Phase 1 and Phase 2. v1.1.17 was Phase 2. v1.1.18 is Phase 3.

Per RELEASE-POLICY.md: internal restructuring without new user-facing features = PATCH bump.

### Manifest Template

Location: `.deploy/releases/v1.1.18.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.1.18",
  "description": "Release v1.1.18: Shared constants, cross-cutting validation, deduplication (ops-003 Phase 3)",
  "changes": [
    {
      "type": "tool",
      "id": "ndp-cli",
      "action": "build",
      "profile": "release"
    }
  ]
}
```

Note: Only `ndp-cli` rebuild is declared. No gold-tables, domain, or stream declarations -- Phase 3 changes are internal to the library. The deploy.sh behavior is unchanged.

### CHANGELOG Entry Template

```markdown
## [1.1.18] - 2026-02-XX

Shared constants, cross-cutting validation, and deduplication (ops-003 Phase 3).

### Changed

- **Shared constants module** -- `VALID_METRICS`, `VALID_ROLLING_STATS`, `GOLD_SCHEMA`, `SILVER_SCHEMA`, `NDP_ENTITY_COLUMN` defined once in `ndp_lib::constants`; all consumers import from this single source
- **Cross-cutting validation** -- `ndp gold sync` validates config before generating DDL by default; `--no-validate` skips validation
- **Gold validation unified** -- `gold::validation::ConfigValidator` removed; all validation routes through `validate::semantic::gold::validate_gold_etl()` with 5 additional checks (lag hours, rolling windows, trend window emptiness)
- **NoOpDbClient consolidated** -- 3 copies in ndp-cli replaced with single import from `ndp_lib::db::NoOpDbClient`

### Removed

- `gold::validation::ConfigValidator` struct (replaced by validate module)
- `VALID_METRICS` duplicate in `validate::semantic::gold` (now imports from constants)
- `VALID_STATS` constant in `validate::semantic::gold` (renamed to `VALID_ROLLING_STATS` in constants)
- 3 `NoOpDbClient` definitions in ndp-cli command modules
- 7 stale `config.yaml` files under `config/base/streams/` (renamed to `.yaml.bak`)

### Technical Notes

- Zero deploy.sh changes (internal consolidation only)
- 865+ tests, 0 failures
- cargo clippy clean
- Integration environment validated
```

### Git Tag Procedure

```bash
# 1. Verify clean state on main branch
git status
git branch

# 2. Verify all tests pass
cargo test --workspace

# 3. Verify clippy clean
cargo clippy --workspace -- -D warnings

# 4. Create annotated tag
git tag -a v1.1.18 -m "Release v1.1.18: Shared constants, cross-cutting validation, deduplication (ops-003 Phase 3)"

# 5. Verify tag
git tag -l v1.1.18
git show v1.1.18 --stat
```

### Rollback Plan

Phase 3 is the lowest-risk release of the three ops-003 releases. It has zero deploy.sh changes and zero new public API. Rollback scenarios:

**If v1.1.18 fails to build on Pi:**
```bash
# Revert to v1.1.17
git checkout v1.1.17
cargo build -p ndp-cli --release
# All deploy.sh dispatch sites still work (they target ndp binary, which is unchanged)
```

**If cross-cutting validation causes unexpected deploy failures:**
```bash
# Option 1: Add --no-validate to deploy.sh gold sync call
# Edit deploy/pi/deploy.sh, add --no-validate to ndp gold sync invocation
# This is a 1-line change

# Option 2: Revert to v1.1.17
git checkout v1.1.17
cargo build -p ndp-cli --release
```

**If YAML retirement breaks something:**
```bash
# Rename back
for f in config/base/streams/*/config.yaml.bak; do
  mv "$f" "${f%.bak}"
done
```

---

## Appendix A: File Change Manifest

### Files Created

| File | Content | Lines (est.) |
|------|---------|-------------|
| `crates/ndp-lib/src/constants.rs` | Platform-wide constants + 7 unit tests | ~60 |

### Files Modified

| File | Change | Lines Changed (est.) |
|------|--------|---------------------|
| `crates/ndp-lib/src/lib.rs` | Add `pub mod constants;` | 1 |
| `crates/ndp-lib/src/types.rs` | Add `validate: bool` to SyncOptions + Default impl | 5 |
| `crates/ndp-lib/src/gold/mod.rs` | Add `check_gold_config()` helper, wire in `sync_stream()` | 25 |
| `crates/ndp-lib/src/gold/config/types.rs` | Remove `VALID_METRICS`, `VALID_ROLLING_STATS` consts; add `use crate::constants::*` | 5 |
| `crates/ndp-lib/src/gold/config/mod.rs` | Update re-exports (remove VALID_METRICS, VALID_ROLLING_STATS) | 2 |
| `crates/ndp-lib/src/gold/generators/constants.rs` | Replace local definitions with re-exports from `crate::constants` | 6 |
| `crates/ndp-lib/src/gold/generators/continuous_aggregate.rs` | Update VALID_METRICS import path | 2 |
| `crates/ndp-lib/src/gold/validation/config_validator.rs` | Remove ConfigValidator struct; keep parse_granularity, parse_window, granularity_to_suffix | ~-100 (net deletion) |
| `crates/ndp-lib/src/gold/validation/mod.rs` | Remove ConfigValidator re-export | 2 |
| `crates/ndp-lib/src/validate/semantic/gold.rs` | Remove local VALID_METRICS/VALID_STATS; import from constants; add 5 gap checks | 30 |
| `tools/ndp-cli/src/commands/gold.rs` | Wire `no_validate` to SyncOptions instead of discarding | 15 |
| `tools/ndp-cli/src/commands/dictionary.rs` | Remove local NoOpDbClient; use `ndp_lib::NoOpDbClient` | -25 |
| `tools/ndp-cli/src/commands/dimension.rs` | Remove local NoOpDbClient; use `ndp_lib::NoOpDbClient` | -25 |
| `tools/ndp-cli/src/commands/domain.rs` | Remove local NoOpDbClient; use `ndp_lib::NoOpDbClient` | -25 |
| `tools/ndp-gold-ddl/src/lib.rs` | Remove ConfigValidator re-export | 2 |

### Files Renamed

| From | To |
|------|-----|
| `config/base/streams/air-quality/config.yaml` | `config/base/streams/air-quality/config.yaml.bak` |
| `config/base/streams/outdoor-weather/config.yaml` | `config/base/streams/outdoor-weather/config.yaml.bak` |
| `config/base/streams/outdoor-air-quality/config.yaml` | `config/base/streams/outdoor-air-quality/config.yaml.bak` |
| `config/base/streams/home-assistant-state/config.yaml` | `config/base/streams/home-assistant-state/config.yaml.bak` |
| `config/base/streams/nws-forecast-hourly/config.yaml` | `config/base/streams/nws-forecast-hourly/config.yaml.bak` |
| `config/base/streams/nws-observations/config.yaml` | `config/base/streams/nws-observations/config.yaml.bak` |
| `config/base/streams/nws-gridpoints-forecast/config.yaml` | `config/base/streams/nws-gridpoints-forecast/config.yaml.bak` |

### Net Line Count

| Category | Lines |
|----------|-------|
| New code (constants.rs, gap checks, validation bridge, wiring) | ~120 |
| Deleted code (ConfigValidator, 3 NoOpDbClient copies) | ~200 |
| **Net change** | **~-80 lines** |

Phase 3 is a net deletion of code. The codebase gets smaller while gaining cross-cutting validation.

---

## Appendix B: Validation Gap Resolution Detail

The 5 checks in ConfigValidator not covered by validate_gold_etl() are added to `validate::semantic::gold::validate_gold_etl()` in Step E:

### Gap 1: gold_etl existence

ConfigValidator returns `MissingRequiredField` when `gold_etl` is `None`. validate_gold_etl returns empty errors when `gold_etl` is absent (treating it as "nothing to validate").

**Resolution**: No change needed. The absence of `gold_etl` is not an error at the semantic validation level -- it means the stream does not participate in Gold. The generator already checks for this and returns a clear error. Cross-cutting validation via `gold::sync_stream()` loads the config, checks `gold_etl.is_some()`, and returns an error before reaching validation. No gap in practice.

### Gap 2: gold_etl.enabled check

Same logic. validate_gold_etl returns empty when disabled. The generator rejects disabled configs. No practical gap.

### Gap 3: Lag lags_hours non-empty

Add to `validate_gold_etl()`:
```rust
if let Some(lags) = lag.get("lags_hours").and_then(|v| v.as_array()) {
    if lags.is_empty() {
        errors.push(ValidationError { ... ErrorCode::InvalidFeatureType ... "lags_hours cannot be empty when enabled" });
    }
}
```

### Gap 4: Lag hours >= 1

Add to `validate_gold_etl()`:
```rust
for (idx, h) in lags.iter().enumerate() {
    if let Some(val) = h.as_i64() {
        if val < 1 {
            errors.push(ValidationError { ... "lag hours must be >= 1" });
        }
    }
}
```

### Gap 5: Rolling windows / Trend window non-empty

Add to `validate_gold_etl()` in rolling and trend blocks:
```rust
if windows.is_empty() {
    errors.push(ValidationError { ... "windows cannot be empty when enabled" });
}
```

---

## Appendix C: SyncOptions Change Detail

### Current (v1.1.17)

```rust
// crates/ndp-lib/src/types.rs
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub dry_run: bool,
}
```

### After Phase 3 (v1.1.18)

```rust
// crates/ndp-lib/src/types.rs
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub dry_run: bool,
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

The `validate: true` default means all sync operations validate by default. The CLI maps `--no-validate` to `validate: false`.

### CLI Wiring (gold.rs)

Before (v1.1.17):
```rust
GoldCommands::Sync { stream, domain, transitions: _, events: _, dry_run, no_validate: _ } => {
    ...
    let opts = ndp_lib::types::SyncOptions { dry_run };
```

After (v1.1.18):
```rust
GoldCommands::Sync { stream, domain, transitions: _, events: _, dry_run, no_validate } => {
    ...
    let opts = ndp_lib::types::SyncOptions { dry_run, validate: !no_validate };
```

Same pattern for `Recreate`.
