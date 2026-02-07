# OPS-003 Phase 2 Refinement: v1.1.15 Validate Migration

> **Feature**: ops-003 (Unified Action Library)
> **Release**: v1.1.15
> **Date**: 2026-02-07
> **Status**: Refinement
> **Patterns Used**: ID 19 (release-workflow), ID 33 (integration-environment), ID 35 (integration-e2e-testing), ID 37 (ndp-config-dir), ID 43 (deploy-sh-safety-protocol), ID 44 (crate-module-migration), ID 45 (no-fallback-dispatch-policy)

---

## 1. Implementation Order

### Phase Diagram

```
                    +------------------+
                    | CHECKPOINT 0     |
                    | All 217 tests    |
                    | pass at origin   |
                    | cargo test       |
                    | -p ndp-validate  |
                    +--------+---------+
                             |
              +--------------+--------------+
              |                             |
    +---------v---------+         +---------v---------+
    | A. Create validate|         | B. Add deps to    |
    | module structure  |         | ndp-lib Cargo.toml|
    | in ndp-lib        |         | (jsonschema,      |
    | (empty mod.rs)    |         | schemars, etc.)   |
    +---------+---------+         +---------+---------+
              |                             |
              +--------------+--------------+
                             |
                    +--------v---------+
                    | C. Move source   |
                    | files in order:  |
                    | 1. error.rs      |
                    | 2. schema.rs     |
                    | 3. schema_gen.rs |
                    | 4. semantic/     |
                    |    mod.rs        |
                    |    sources.rs    |
                    |    source_path.rs|
                    |    dq_rules.rs   |
                    |    gold.rs       |
                    |    domain.rs     |
                    |    table_exists.rs|
                    +--------+---------+
                             |
                    +--------v---------+
                    | D. Split cli.rs  |
                    | into lib types   |
                    | vs CLI-only code |
                    | (CRITICAL STEP)  |
                    +--------+---------+
                             |
                    +--------v---------+
                    | CHECKPOINT 1     |
                    | cargo test       |
                    | -p ndp-lib       |
                    | (217 validate +  |
                    | 491 gold tests)  |
                    +--------+---------+
                             |
              +--------------+--------------+
              |                             |
    +---------v---------+         +---------v---------+
    | E. Update ndp-    |         | F. Add commands/  |
    | validate to thin  |         | validate.rs to    |
    | wrapper           |         | ndp-cli           |
    +---------+---------+         +---------+---------+
              |                             |
              +--------------+--------------+
                             |
                    +--------v---------+
                    | CHECKPOINT 2     |
                    | ndp validate     |
                    | output ==        |
                    | ndp-validate     |
                    | output           |
                    +--------+---------+
                             |
                    +--------v---------+
                    | G. Exit code     |
                    | handling (0/1/2) |
                    | in ndp-cli       |
                    +--------+---------+
                             |
                    +--------v---------+
                    | CHECKPOINT 3     |
                    | Exit codes match |
                    | for all modes    |
                    +--------+---------+
                             |
                    +--------v---------+   <-- POINT OF NO RETURN
                    | H. Update        |       (deploy.sh changes)
                    | deploy.sh        |
                    | (2 dispatch      |
                    | sites)           |
                    +--------+---------+
                             |
                    +--------v---------+
                    | CHECKPOINT 4     |
                    | Integration      |
                    | deploy succeeds  |
                    +--------+---------+
                             |
                    +--------v---------+
                    | I. Release       |
                    | v1.1.15          |
                    +--------+---------+
```

### Step Dependencies

| Step | Depends On | Can Parallel With | Estimated Effort |
|------|-----------|-------------------|------------------|
| A | None | B | Small (mkdir + mod.rs) |
| B | None | A | Small (Cargo.toml edits) |
| C | A + B | None | Medium (move 11 files, fix `use` paths) |
| D | C | None | **Large** (split 1370-line cli.rs) |
| E | D | F | Medium (rewrite lib.rs, update main.rs) |
| F | D | E | Medium (new commands/validate.rs) |
| G | F | None | Medium (ExitCode handling) |
| H | E + F + G | None | Small (2 dispatch sites) |
| I | H + all checkpoints | None | Small (3 artifacts) |

### What Can Be Done in Parallel

- **A + B**: Module structure and Cargo.toml changes are independent.
- **E + F**: Thin wrapper and CLI commands are independent (both depend on Checkpoint 1).

### What Has Dependencies

- **C depends on A + B**: Cannot move files until module structure and deps exist.
- **D depends on C**: Splitting cli.rs requires the validate module to exist in ndp-lib.
- **E, F depend on D**: Both need the split types to import correctly.
- **G depends on F**: Exit code handling applies to the CLI command.
- **H depends on E + F + G**: deploy.sh must not switch until all three are verified.

### Point of No Return

**Step H (deploy.sh modification)** is the point of no return. Before H, the original ndp-validate binary and deploy.sh are untouched. After H, deploy.sh expects `ndp validate` to exist.

H is easily revertable via a single `git checkout` of deploy.sh, so the true point of no return is the release tag + push.

---

## 2. Risk Register

### Risk 1: cli.rs Split Complexity (1370 lines)

**Likelihood**: High | **Impact**: Medium

**Root Cause**: ndp-validate's `cli.rs` contains 1370 lines mixing library types (`ValidationResult`, `BatchValidationResult`, `ValidationSummary`, output formatters, exit code logic) with CLI-only code (clap `Cli` struct, arg parsing, `parse_from` tests). The library types must move to ndp-lib; the CLI code stays in ndp-validate.

**Phase 1 Analogy**: Phase 1 did not face this -- ndp-gold-ddl had a clean lib/CLI split. ndp-validate has the types **inside** cli.rs.

**Mitigation**:
1. Create `ndp_lib::validate::types.rs` for `ValidationResult`, `BatchValidationResult`, `ValidationSummary`, `BatchSummary`, `OutputFormat`.
2. Create `ndp_lib::validate::output.rs` for `output_json`, `output_json_batch`, `output_human`, `output_human_batch`, `format_error_human`, `format_warning_human`.
3. Create `ndp_lib::validate::exit_codes.rs` for `exit_codes` module, `determine_exit_code`, `determine_batch_exit_code`.
4. Leave `Cli` struct and its 55 tests in ndp-validate's cli.rs (CLI-only).
5. ndp-validate's cli.rs re-imports types from `ndp_lib::validate::types`.

**Rollback**: If the split is too complex, keep all of cli.rs in ndp-validate and have ndp-cli's `commands/validate.rs` depend on `ndp_validate` as a library (fallback approach). This is less clean but functional.

### Risk 2: Validate Test Breakage (217 tests)

**Likelihood**: Low | **Impact**: High

**Root Cause**: Moving source files changes `use` paths. Tests reference internal modules.

**Phase 1 Experience**: Phase 1 had 3 files with stale `crate::config::` paths. All caught by `cargo check`.

**Mitigation**:
- Move files one submodule at a time: error -> schema -> schema_gen -> semantic.
- Run `cargo check -p ndp-lib` after each submodule.
- All 55 CLI-specific tests stay in ndp-validate (they test clap parsing, not library logic).
- The remaining 162 tests move with the source code.
- Do NOT delete originals until all 217 tests pass in new location.

**Rollback**:
```bash
git stash  # or git checkout -- crates/ndp-lib/src/validate/
# Original tests still pass:
cargo test -p ndp-validate
```

### Risk 3: deploy.sh Regression (Validate Dispatch)

**Likelihood**: Medium | **Impact**: High (validation is pre-flight check)

**Root Cause**: Two dispatch sites change from `ndp-validate` to `ndp validate`. Flags change. Exit code semantics change.

**Phase 1 Bugs That Apply**:
- **BUG-004 (config-dir path mismatch)**: ndp-validate defaults `--config-dir` to `config/base/streams`. ndp-cli global `--config-dir` points to `config/base`. DIFFERENT CONVENTIONS. Must resolve before deploy.sh switchover.
- **Silently ignored flags**: Phase 1 found `--events` + `--stream` was silently ignored. Must verify all flag combinations for validate.

**Mitigation**:
1. Exact output parity testing before deploy.sh switchover (Checkpoint 2).
2. Exit code parity testing (Checkpoint 3).
3. Integration deploy with `DEPLOY_ENV=integration` before release (Checkpoint 4).
4. deploy.sh grep verification (Section 6).

**Rollback**:
```bash
git checkout v1.1.14 -- deploy/pi/deploy.sh
# ndp-validate standalone still works:
cargo build -p ndp-validate
```

### Risk 4: Exit Code Mismatch (3-way exit codes)

**Likelihood**: High | **Impact**: Medium

**Root Cause**: ndp-validate uses 3 exit codes per dp-019:
- 0: Validation passed (may have warnings)
- 1: Validation failed (has errors)
- 2: System error (file not found, schema load failed)

ndp-cli currently uses `Result<(), Box<dyn Error>>` which maps to:
- 0: Success
- 1: Any error (no distinction between validation error and system error)

deploy.sh at Site 3 checks `if ! "$validate_tool" ...` which only distinguishes 0 vs non-zero. But preserving the 3-way exit code is important for scripting parity and future use.

**Mitigation**: See Critical Decision Point 5 (Section 3). The chosen approach must be implemented in Step G.

### Risk 5: Dependency Bloat (jsonschema + schemars + sqlparser)

**Likelihood**: Low | **Impact**: Low

**Root Cause**: ndp-validate brings `jsonschema` (0.17), `schemars` (0.8), `sqlparser` (0.50), `strsim` (0.11), `regex` (1), `serde_yaml` (0.9) into ndp-lib. These increase compile time and binary size.

**Measurement (pre-migration)**:
```bash
# Measure current ndp binary size:
ls -la target/release/ndp
# Measure after adding deps (Step B):
# Expected: ~2-4 MB increase (jsonschema is the heaviest)
```

**Mitigation**: See Critical Decision Point 1 (Section 3). Feature flag approach is an option but adds complexity for marginal benefit on a single-platform (Raspberry Pi) deployment.

### Risk 6: config-dir Convention Conflict

**Likelihood**: High | **Impact**: High

**Root Cause**: ndp-validate's `--config-dir` defaults to `config/base/streams` and is used directly as the streams directory. ndp-cli's `--config-dir` defaults to `config/base` and child commands call `.parent()` or append subdirectories.

deploy.sh Site 3 (`validate_domain_configs`) calls:
```bash
"$validate_tool" --domain "$config_file" --format human
```
No `--config-dir` is passed -- it relies on the default `config/base/streams`.

deploy.sh Site 4 (`handle_domain_declaration`) calls:
```bash
"$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
```
Where `$CONFIG_STREAMS_DIR` is `$REPO_ROOT/config/base/streams`.

**Phase 1 Lesson (BUG-004)**: deploy.sh passed `$REPO_ROOT/config` but ndp CLI expected `config/base`. Three deploy.sh sites had to be fixed. SAME CLASS OF BUG WILL HAPPEN HERE unless the config-dir convention is resolved before deploy.sh switchover.

**Mitigation**: See Critical Decision Point 4 (Section 3).

### Risk 7: Schema File Path Resolution

**Likelihood**: Medium | **Impact**: Medium

**Root Cause**: ndp-validate defaults `--schema-path` to `schemas/stream-config.v1.1.schema.json` relative to CWD. ndp-cli runs from `$REPO_ROOT`. On the Pi, CWD during deploy.sh is `$REPO_ROOT`, so relative paths work. But if the CLI is run from a different directory, schema files are not found.

**Mitigation**:
1. In `commands/validate.rs`, resolve schema path relative to config-dir parent (same as ndp-validate behavior).
2. Alternatively, embed the schema at compile time using `include_str!`. This eliminates file-not-found at runtime.
3. ndp-validate already has `SchemaValidator::default_schema()` which embeds the schema. Use this path in ndp-lib.

### Risk 8: Domain Schema Path Divergence

**Likelihood**: Low | **Impact**: Medium

**Root Cause**: ndp-validate has two schema paths: `--schema-path` (stream) and `--domain-schema-path` (domain). ndp-cli currently has no concept of domain schemas. Must ensure both schema paths are forwarded correctly.

**Mitigation**: ndp-validate's `SchemaValidator::default_schema()` and `DomainSchemaValidator::default_schema()` embed schemas at compile time via schemars generation. The CLI does not need to pass file paths if it uses the default schema methods. Only `--verify-schema` needs a file path, and that mode is for CI/dev, not deploy.sh.

---

## 3. Critical Decision Points

### Decision 1: Feature Flag vs Always-On for jsonschema/schemars Deps

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: Always-on** (recommended) | Simple build. Single binary. No conditional compilation. | Adds ~2-4 MB to ndp binary. Longer compile for non-validate work. |
| **B: Feature flag** `validate` | Smaller binary when not needed. Faster compile for gold-only work. | Cargo feature unification headaches. deploy.sh must build with `--features validate`. Feature-gated `pub mod validate` complicates imports. |

**Recommendation**: Option A (always-on). Justification:
1. Phase 1 (gold module) used always-on with zero issues.
2. Raspberry Pi deployment always builds the full `ndp` binary.
3. Feature flags add complexity to deploy.sh (`cargo build -p ndp-cli --features validate`).
4. jsonschema/schemars are pure Rust -- no system library linking issues on Pi.
5. Binary size is well under the 15 MB target even with all deps.

**Decision**: Always-on. No feature flag.

### Decision 2: Where ValidationResult/BatchResult Types Live

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: ndp_lib::validate::types** (recommended) | CLI and future MCP server share types. Serialization works across boundaries. | 1370-line cli.rs must be split. |
| **B: ndp_lib::validate re-exports from ndp-validate** | No split needed. | Circular: ndp-lib depends on ndp-validate. Violates layering. |
| **C: Leave in ndp-validate, CLI formats raw strings** | Simplest. No type sharing. | ndp-cli cannot produce structured JSON output. No parity with ndp-validate. |

**Recommendation**: Option A. The types are data structures with `Serialize` derive -- they belong in the library, not the CLI binary. The split is mechanical:

**Types that move to ndp_lib::validate**:
- `ValidationResult` (line 230-287 of cli.rs)
- `BatchValidationResult` (line 289-338 of cli.rs)
- `ValidationSummary` (line 222-228 of cli.rs)
- `BatchSummary` (line 299-306 of cli.rs)
- `OutputFormat` enum (line 45-52 of cli.rs)
- `exit_codes` module (line 29-38 of cli.rs)
- `determine_exit_code()` (line 454-460 of cli.rs)
- `determine_batch_exit_code()` (line 463-469 of cli.rs)
- `output_json()`, `output_json_batch()` (line 345-354)
- `output_human()`, `output_human_batch()` (line 357-411)
- `format_error_human()`, `format_warning_human()` (line 414-451)

**Types that stay in ndp-validate**:
- `Cli` struct (clap derive, line 82-161 of cli.rs)
- `ConfigType` enum (line 55-62)
- `Cli::validate_args()` (line 163-195)
- `Cli::is_schema_mode()`, `is_domain_mode()`, `config_type()` (line 197-215)
- All 55 CLI-parsing tests

**Decision**: Option A. Split cli.rs into lib types and CLI-only code.

### Decision 3: deploy.sh Policy -- No-Fallback vs Graceful Skip

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: No-fallback (error + return 1)** (recommended) | Consistent with Phase 1 gold dispatch. Problems surface immediately. | If ndp binary is missing, deployment halts entirely. |
| **B: Graceful skip (warn + return 0)** | Validation is optional; deployment can proceed without it. More forgiving. | Silently skips pre-flight checks. Masks build failures. Phase 1 deliberately moved away from this. |
| **C: Configurable** | Best of both worlds. `--require-validation` flag in deploy.sh. | Adds deploy.sh complexity. Another flag to document. |

**Recommendation**: Option A (no-fallback). Justification:
1. Phase 1 established the no-fallback pattern (pattern ID 45). Consistency.
2. Validation IS the pre-flight check. If it can be skipped, what is the point?
3. deploy.sh already checks `command -v ndp` for dictionary/dimension/domain/gold dispatch (5 existing sites). All use no-fallback. Validate should match.
4. If ndp is missing entirely, ALL dispatch sites fail, not just validate. So the graceful-skip scenario ("ndp missing but we skip validate and continue") cannot occur -- gold dispatch would also fail.

**However**: There is one nuance. The current deploy.sh at Site 4 (`handle_domain_declaration`) wraps validate in an `if [ -n "$validate_tool" ]` block. If ndp-validate is absent, it skips validation but continues with domain sync. After v1.1.15, validate and domain sync share the same `ndp_tool` variable (already resolved for gold dispatch at Site 2 in the same function). If `ndp_tool` exists, validate runs. If it does not exist, the function already failed at the gold dispatch. So the graceful-skip path is unreachable after v1.1.14.

**Decision**: No-fallback (error + return 1). Same pattern as Phase 1.

### Decision 4: config-dir Interaction Between ndp Global Flag and Validate's --config-dir

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: Validate subcommand has its own --config-dir** | Preserves ndp-validate behavior. deploy.sh passes same paths. | Two config-dir flags on one binary. Confusing UX. |
| **B: Use ndp global --config-dir, derive streams-dir** (recommended) | Single flag. Consistent with gold, dictionary, domain, dimension. | Must compute streams_dir as `config_dir.join("streams")`. deploy.sh must change from `$CONFIG_STREAMS_DIR` to `$REPO_ROOT/config/base`. |
| **C: Validate subcommand takes --streams-dir** | Explicit about what it needs. No ambiguity. | Different flag name from standalone. deploy.sh flag changes. |

**Recommendation**: Option B. Justification:
1. Pattern ID 37 (ndp-config-dir convention): `--config-dir` always points to the BASE directory (`config/base`). All existing commands follow this.
2. The validate module internally computes: `streams_dir = config_dir.join("streams")`, `domains_dir = config_dir.parent().unwrap().join("domains")`.
3. deploy.sh already passes `$REPO_ROOT/config/base` to gold (after BUG-004 fix). Validate should use the same value.
4. For `--schema-path` and `--domain-schema-path`, the library has embedded defaults. Only override if needed.

**Derived paths from `--config-dir config/base`**:
```
config_dir       = config/base
streams_dir      = config/base/streams          (for --all, single stream)
domains_dir      = config/domains               (for --domain-all)
schema_path      = embedded (SchemaValidator::default_schema())
domain_schema    = embedded (DomainSchemaValidator::default_schema())
```

**deploy.sh Site 3 change**:
```bash
# Before: "$validate_tool" --domain "$config_file" --format human
# After:  "$ndp_tool" validate --domain "$config_file" --config-dir "$REPO_ROOT/config/base" --format human
```

**deploy.sh Site 4 change**:
```bash
# Before: "$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
# After:  "$ndp_tool" validate --domain "$config_file" --config-dir "$REPO_ROOT/config/base" --format human
```

Note: `$CONFIG_STREAMS_DIR` is `$REPO_ROOT/config/base/streams`. The ndp-cli command receives `config/base` and derives `streams` internally. This is the BUG-004 lesson applied proactively.

**Decision**: Option B. Use ndp global `--config-dir`, derive streams-dir internally.

### Decision 5: Exit Code Handling

**Options**:

| Option | Pros | Cons |
|--------|------|------|
| **A: `std::process::ExitCode`** (recommended) | Clean Rust API. Supports arbitrary exit codes (0/1/2). main.rs returns ExitCode. | Requires changing ndp-cli's main.rs signature from `Result<(), Box<dyn Error>>` to `ExitCode`. Affects all commands. |
| **B: `std::process::exit(code)`** | Simple. Does not change main.rs signature. | Skips destructors. Bad practice in library code. Only acceptable in main.rs. |
| **C: Error mapping to exit 1** | No changes to main.rs. All errors map to exit 1. | Loses the 0/1/2 distinction. deploy.sh only checks 0 vs non-zero, so functional impact is zero. But scripting parity is lost. |
| **D: Custom error type with exit code** | Preserves exit codes without changing main.rs. Error carries code. | Over-engineering. The error type becomes a container for an int. |

**Recommendation**: Option A for the validate command; Option C for other commands (no change). Justification:
1. deploy.sh only checks `if !` (zero vs non-zero). The 0/1/2 distinction has no operational impact on deploy.sh today.
2. However, `ndp validate` should preserve exit code parity with `ndp-validate` for scripting use outside deploy.sh.
3. The cleanest approach: `commands/validate.rs` returns `ExitCode` instead of `Result`. The `main.rs` dispatch handles this:

```rust
Commands::Validate(args) => {
    return commands::validate::run(args, &config_dir).await;
}
// Other commands continue to return Result<(), Box<dyn Error>>
```

This requires `main` to return `ExitCode` instead of `Result`. The conversion:
```rust
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(1)
        }
    }
}
```

All existing commands (`dictionary`, `dimension`, `domain`, `gold`) return `Result<(), Box<dyn Error>>` which maps to `ExitCode::SUCCESS` on Ok or `ExitCode::from(1)` on Err. No behavioral change.

**Decision**: Option A. Change ndp-cli main.rs to return `ExitCode`. Validate command returns proper 0/1/2.

---

## 4. Definition of Done

### ops-003-05: Validate Module in ndp-lib

| Criterion | Verification Command |
|-----------|---------------------|
| All 217 ndp-validate unit tests pass in ndp-lib | `cargo test -p ndp-lib -- validate` |
| ndp-validate standalone still builds | `cargo build -p ndp-validate` |
| ndp-validate standalone tests still pass | `cargo test -p ndp-validate` |
| `cargo test --workspace` passes (no regressions) | `cargo test --workspace` |
| No compilation warnings in moved code | `cargo build -p ndp-lib 2>&1 \| grep -c warning` returns 0 |
| ValidationResult, BatchValidationResult live in ndp_lib::validate | `grep -r "pub struct ValidationResult" crates/ndp-lib/src/validate/` |
| Error types live in ndp_lib::validate::error | `grep -r "pub struct ValidationError" crates/ndp-lib/src/validate/` |
| Schema validator embeds schema at compile time | `grep "include_str\|default_schema" crates/ndp-lib/src/validate/schema.rs` |

### ops-003-06: `ndp validate` Subcommands

| Criterion | Verification Command |
|-----------|---------------------|
| `ndp validate --all` produces identical output to `ndp-validate --all` | `diff <(ndp-validate --all --config-dir config/base/streams) <(ndp validate --all --config-dir config/base)` |
| `ndp validate --domain <path>` produces identical output | `diff <(ndp-validate --domain <path> --config-dir config/base/streams) <(ndp validate --domain <path> --config-dir config/base)` |
| `ndp validate --domain-all` produces identical output | `diff <(ndp-validate --domain-all) <(ndp validate --domain-all --config-dir config/base)` |
| `ndp validate --generate-schema` produces identical JSON | `diff <(ndp-validate --generate-schema) <(ndp validate --generate-schema)` |
| `ndp validate --verify-schema <path>` produces identical result | Compare exit codes and stderr output |
| Exit code 0 on valid config | `ndp validate config/base/streams/air-quality/config.json; echo $?` returns 0 |
| Exit code 1 on invalid config | Create invalid config, verify exit 1 |
| Exit code 2 on system error (file not found) | `ndp validate /nonexistent/config.json; echo $?` returns 2 |
| `--format json` produces valid JSON | `ndp validate --all --format json \| python3 -m json.tool` |
| `--format human` produces colored output | Visual inspection |
| `--strict` treats warnings as errors | `ndp validate --all --strict; echo $?` returns 1 if warnings exist |
| `--schema-only` skips semantic validation | Compare error counts with and without flag |

### ops-003-07: deploy.sh Validate Switchover

| Criterion | Verification Command |
|-----------|---------------------|
| Zero calls to `ndp-validate` remain in deploy.sh dispatch sites | See Section 6 (grep verification) |
| `validate_domain_configs()` calls `ndp validate` | `grep 'ndp validate' deploy/pi/deploy.sh` |
| `handle_domain_declaration()` validate part calls `ndp validate` | Same grep |
| Missing `ndp` binary causes `error` + `return 1` | Already established by v1.1.14 gold dispatch |
| Integration deploy passes | `DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json` |
| Domain validation runs during deploy | Check deploy.sh output for "Validating domain config" |

---

## 5. Integration Test Checklist

### Pre-Integration (Unit/Parity Tests)

```bash
# 1. Baseline: verify ndp-validate still passes
cargo test -p ndp-validate
# Expected: 217 passed, 0 failed

# 2. Verify ndp-lib includes validate tests
cargo test -p ndp-lib -- validate
# Expected: 162+ passed (unit tests from moved source files)

# 3. Full workspace test
cargo test --workspace
# Expected: 0 failures across all crates

# 4. Build both binaries
cargo build -p ndp-cli -p ndp-validate

# 5. Parity: single stream validation
diff <(./target/debug/ndp-validate config/base/streams/air-quality/config.json --format json 2>/dev/null) \
     <(./target/debug/ndp validate config/base/streams/air-quality/config.json --config-dir config/base --format json 2>/dev/null)

# 6. Parity: all streams validation
diff <(./target/debug/ndp-validate --all --config-dir config/base/streams --format json 2>/dev/null) \
     <(./target/debug/ndp validate --all --config-dir config/base --format json 2>/dev/null)

# 7. Parity: domain validation
diff <(./target/debug/ndp-validate --domain config/domains/indoor-air-quality/domain.json --config-dir config/base/streams --format json 2>/dev/null) \
     <(./target/debug/ndp validate --domain config/domains/indoor-air-quality/domain.json --config-dir config/base --format json 2>/dev/null)

# 8. Parity: domain-all validation
diff <(./target/debug/ndp-validate --domain-all --format json 2>/dev/null) \
     <(./target/debug/ndp validate --domain-all --config-dir config/base --format json 2>/dev/null)

# 9. Parity: schema generation
diff <(./target/debug/ndp-validate --generate-schema 2>/dev/null) \
     <(./target/debug/ndp validate --generate-schema 2>/dev/null)

# 10. Exit code: valid config
./target/debug/ndp validate config/base/streams/air-quality/config.json --config-dir config/base --format json > /dev/null 2>&1
echo "Exit: $?"
# Expected: 0

# 11. Exit code: nonexistent file
./target/debug/ndp validate /nonexistent/config.json --config-dir config/base --format json > /dev/null 2>&1
echo "Exit: $?"
# Expected: 2

# 12. Exit code: strict with warnings
./target/debug/ndp validate --all --config-dir config/base --strict --format json > /dev/null 2>&1
echo "Exit: $?"
# Expected: 1 (if any warnings in current configs)

# 13. Human format output
./target/debug/ndp validate --all --config-dir config/base --format human
# Expected: colored output with [PASS] / [FAIL] per stream
```

### Integration Environment Tests (Live TimescaleDB)

```bash
# 1. Start integration stack
docker compose -f docker-compose.integration.yml up -d

# 2. Wait for TimescaleDB
docker compose -f docker-compose.integration.yml exec timescaledb \
  pg_isready -U postgres -d ndp

# 3. Build ndp binary with validate module
cargo build -p ndp-cli

# 4. Dry-run deploy
DEPLOY_ENV=integration DRY_RUN=true \
  ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json

# 5. Full integration deploy
DEPLOY_ENV=integration \
  ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json

# 6. Verify validation ran (check deploy output for validate messages)
# Expected: "Validating domain config" appears in output
# Expected: "[PASS]" appears for each domain

# 7. Verify Gold tables still exist (Phase 1 not regressed)
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c \
  "SELECT view_schema, view_name FROM timescaledb_information.continuous_aggregates WHERE view_schema = 'gold'"

# 8. Verify dictionary/dimension/domain sync still works
docker compose -f docker-compose.integration.yml exec timescaledb \
  psql -U postgres -d ndp -c \
  "SELECT COUNT(*) FROM ndp.data_dictionary"

# 9. Tear down
docker compose -f docker-compose.integration.yml down
```

---

## 6. deploy.sh Verification Checklist

### After v1.1.15, run these grep commands:

```bash
# 1. No ndp-validate references in dispatch sites
grep -n 'ndp-validate' deploy/pi/deploy.sh | grep -v '#' | grep -v 'ndp-validate)' | grep -v 'cargo_package\|binary_name'
# Expected: zero results from dispatch code
# NOTE: ndp-validate may still appear in the build handler (tool build)
#        and in comments. That is expected and acceptable.

# 2. No validate_tool variable usage in dispatch
grep -n 'validate_tool' deploy/pi/deploy.sh
# Expected: zero results (variable completely eliminated)

# 3. Verify ndp validate calls exist
grep -n 'ndp validate' deploy/pi/deploy.sh
# Expected: 2 lines (Site 3: validate_domain_configs, Site 4: handle_domain_declaration)

# 4. Verify ndp dispatch sites count
grep -n 'command -v ndp' deploy/pi/deploy.sh | grep -v 'ndp-validate\|ndp-gold-ddl'
# Expected: 5+ lines (dictionary, domain, dimension, gold-site-1, gold-site-2)
# NOTE: After v1.1.15, validate sites reuse ndp_tool already resolved by
# another dispatch in the same function. They may not have their own
# "command -v ndp" check.

# 5. No fallback pattern for validate
grep -n 'skipping.*validation\|skipping domain validation' deploy/pi/deploy.sh
# Expected: zero results (no graceful-skip messages remain)

# 6. Verify error + return 1 for missing ndp at validate sites
grep -B5 -A2 'ndp validate' deploy/pi/deploy.sh
# Expected: error handling visible for missing binary case
```

### Exact Expected deploy.sh Changes

**Site 3: `validate_domain_configs()` (line ~1530)**

BEFORE:
```bash
local validate_tool=""
if command -v ndp-validate &> /dev/null; then
    validate_tool="ndp-validate"
elif [ -x "/opt/ndp/bin/ndp-validate" ]; then
    validate_tool="/opt/ndp/bin/ndp-validate"
elif [ -x "$REPO_ROOT/target/release/ndp-validate" ]; then
    validate_tool="$REPO_ROOT/target/release/ndp-validate"
elif [ -x "$REPO_ROOT/target/debug/ndp-validate" ]; then
    validate_tool="$REPO_ROOT/target/debug/ndp-validate"
fi

if [ -z "$validate_tool" ]; then
    warn "ndp-validate not available, skipping domain validation"
    warn "Build with: cargo build -p ndp-validate --release"
    return 0
fi
# ...
"$validate_tool" --domain "$config_file" --format human
```

AFTER:
```bash
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
# ...
"$ndp_tool" validate --domain "$config_file" --config-dir "$REPO_ROOT/config/base" --format human
```

**Site 4: `handle_domain_declaration()` validate dispatch (line ~2032)**

BEFORE:
```bash
local validate_tool=""
if command -v ndp-validate &> /dev/null; then
    validate_tool="ndp-validate"
# ... 4-way lookup ...
fi

if [ -n "$validate_tool" ]; then
    "$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
else
    warn "  ndp-validate not available, skipping domain validation"
fi
```

AFTER:
```bash
# ndp_tool already resolved earlier in this function (from gold dispatch, v1.1.14)
if [ -z "$ndp_tool" ]; then
    error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
    return 1
fi

"$ndp_tool" validate --domain "$config_file" --config-dir "$REPO_ROOT/config/base" --format human
```

**Key differences from ndp-validate invocation**:
- `--config-dir` changes from `$CONFIG_STREAMS_DIR` (`config/base/streams`) to `$REPO_ROOT/config/base` (ndp-cli convention).
- `validate_tool` variable eliminated; reuses `ndp_tool` from gold dispatch.
- `warn` + skip replaced with `error` + `return 1`.

---

## 7. Rollback Plan

### If v1.1.15 Fails in Production

**Symptoms**: deploy.sh validation phases fail, domain validation does not run, incorrect exit codes cause deployment to halt.

**Step 1: Revert deploy.sh immediately**
```bash
# On Pi:
git checkout v1.1.14 -- deploy/pi/deploy.sh
```

This restores the `command -v ndp-validate` dispatch. Since ndp-validate standalone was NOT removed in v1.1.15, it still works.

**Step 2: Verify standalone binary still available**
```bash
command -v ndp-validate || ls /opt/ndp/bin/ndp-validate || ls target/release/ndp-validate
```

If not available (unlikely -- v1.1.15 does not remove it):
```bash
cargo build -p ndp-validate --release
```

**Step 3: Re-deploy using v1.1.14 manifest**
```bash
./deploy.sh apply .deploy/releases/v1.1.14.manifest.json
```

**Step 4: Verify validation works**
```bash
# On Pi:
ndp-validate --all --config-dir config/base/streams --format human
ndp-validate --domain config/domains/indoor-air-quality/domain.json --format human
```

**Step 5: Record the failure**
- Create bug file: `product/features/ops-003/bugs/BUG-00N-{slug}.md`
- Update `product/features/ops-003/STATUS.md`
- Record reflexion with low reward for the pattern that failed

**Step 6: Root cause analysis**
- Check deploy.sh logs for error message
- Compare `ndp validate` output on Pi vs dev machine
- Check `--config-dir` path (most likely culprit, per BUG-004 precedent)
- Check exit code behavior (`echo $?` after `ndp validate`)
- Verify `--format human` output parsing in deploy.sh (grep patterns)

### Binary Availability After Rollback

| Binary | Available? | Notes |
|--------|-----------|-------|
| ndp-validate | YES | Standalone binary not deleted in v1.1.15 |
| ndp (with validate) | YES | But validate subcommand may have issues |
| ndp (with gold) | YES | Unchanged from v1.1.14 |
| ndp-gold-ddl | YES | Thin wrapper, unchanged from v1.1.14 |

### If Rollback Occurs AFTER Release Tag

```bash
# Revert to v1.1.14
git checkout v1.1.14 -- deploy/pi/deploy.sh
git commit -m "revert: deploy.sh validate dispatch back to ndp-validate (v1.1.15 regression)"
# Do NOT retag v1.1.15 -- create v1.1.15.1 or bump to v1.1.16
```

---

## 8. Release Preparation

### Manifest Template

Location: `.deploy/releases/v1.1.15.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.1.15",
  "description": "Release v1.1.15: Config validation consolidated into ndp-lib and ndp CLI (ops-003 Phase 2)",
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

Note: No `gold-tables` or `domain` declarations needed -- those are unchanged from v1.1.14. The manifest only declares the ndp-cli rebuild (which now includes the validate module).

### CHANGELOG Template

```markdown
## [1.1.15] - 2026-02-XX

Config validation consolidated into ndp-lib and ndp CLI (ops-003 Phase 2).

### Changed

- **Validate module migrated to ndp-lib** -- 11 source files and 217 tests moved from `tools/ndp-validate/src/` to `crates/ndp-lib/src/validate/`
- **`ndp validate` subcommands** -- `ndp validate --all`, `--stream`, `--domain`, `--domain-all`, `--generate-schema`, `--verify-schema` replace standalone `ndp-validate` binary
- **deploy.sh validate dispatch** -- 2 dispatch sites switched from `command -v ndp-validate` to `ndp validate`
  - `validate_domain_configs()`: now calls `ndp validate --domain <path> --config-dir <base>`
  - `handle_domain_declaration()` validate part: now calls `ndp validate --domain <path> --config-dir <base>`
- **No-fallback policy** -- deploy.sh now errors (`return 1`) instead of warning and skipping when `ndp` is not found for validation
- **config-dir harmonization** -- `--config-dir` points to `config/base` (ndp-cli convention); validate derives `streams/` internally
- **Exit code preservation** -- `ndp validate` returns 0/1/2 per dp-019 specification (0=pass, 1=validation error, 2=system error)

### Added

- `crates/ndp-lib/src/validate/` module with full config validation capability
- `tools/ndp-cli/src/commands/validate.rs` -- CLI routing for validate subcommands
- `ndp-cli` main.rs returns `ExitCode` (supports 3-way exit codes)

### Technical Notes

- 217 validate tests migrated, all passing under `cargo test -p ndp-lib`
- ndp-validate standalone remains buildable as a thin wrapper over ndp-lib
- cli.rs (1370 lines) split: library types to ndp-lib, CLI parsing stays in ndp-validate
- All 7 deploy.sh dispatch sites now use `ndp` (dictionary, dimension, domain, gold x2, validate x2)
- Integration verified: `DEPLOY_ENV=integration ./deploy.sh apply v1.1.15.manifest.json`
```

### Git Tag Procedure

```bash
# 1. Verify on main branch with clean status
git status  # Must show clean working tree
git branch  # Must be on main

# 2. Verify all tests pass
cargo test --workspace

# 3. Create annotated tag
git tag -a v1.1.15 -m "Release v1.1.15: Config validation consolidated into ndp-lib and ndp CLI (ops-003 Phase 2)"

# 4. Verify tag
git tag -l v1.1.15
git show v1.1.15 --stat
```

---

## 9. Lessons Applied from Phase 1

### Lesson 1: BUG-004 -- config-dir Path Mismatch

**Phase 1 Bug**: deploy.sh passed `$REPO_ROOT/config` but ndp CLI expected `config/base`. Three deploy.sh sites needed fixing.

**Phase 2 Application**: Critical Decision Point 4 (Section 3) addresses this proactively. The validate module will use ndp global `--config-dir` pointing to `config/base`, and derive `streams/` internally. deploy.sh Site 3 and Site 4 will pass `$REPO_ROOT/config/base`, NOT `$CONFIG_STREAMS_DIR`.

**Verification**: Parity test #5-#8 in Section 5 explicitly check that paths resolve correctly.

### Lesson 2: Silently Ignored Flags

**Phase 1 Bug**: `--events` + `--stream` was silently ignored by the new binary instead of erroring.

**Phase 2 Application**: All clap `conflicts_with` declarations from ndp-validate's Cli struct must be replicated in ndp-cli's validate command. Specifically:
- `--all` conflicts with positional `config_path`
- `--domain` conflicts with `--all`, positional `config_path`
- `--domain-all` conflicts with `--all`, `--domain`, positional `config_path`
- `--generate-schema` conflicts with `--all`, positional `config_path`, `--verify-schema`

**Verification**: Parity tests include invalid flag combinations to verify proper error messages.

### Lesson 3: Unimplemented Flag Captured as `_` (Unused)

**Phase 1 Bug**: `--validate-only` flag was captured but not wired to any logic.

**Phase 2 Application**: Every flag in the `ndp validate` clap struct must have a corresponding code path. The implementation uses ndp-validate's existing `main.rs` logic as reference. No flag should be declared without a handler. Review checklist:
- `--all` -> runs batch validation
- `--domain <path>` -> runs single domain validation
- `--domain-all` -> runs all domain validation
- `--generate-schema` -> generates schema to stdout
- `--verify-schema <path>` -> verifies schema matches generated
- `--schema-only` -> skips semantic validation
- `--strict` -> treats warnings as errors
- `--format` -> json or human output
- `--check-tables` -> (deferred, prints warning that DB check not yet implemented)

### Lesson 4: Tracing to stdout

**Phase 1 Fix**: ndp CLI wrote tracing logs to stdout, breaking parity. Changed default filter to `warn` and writer to `stderr`.

**Phase 2 Application**: Already fixed in v1.1.14. No additional work needed -- ndp-cli's tracing configuration applies to all commands including `validate`.

### Lesson 5: Integration Environment Required

**Phase 1 Finding**: Integration E2E testing found 3 bugs that unit tests did not catch. All 3 were deploy.sh interaction issues (config-dir, flag handling, unimplemented paths).

**Phase 2 Application**: Checkpoints 2, 3, and 4 are mandatory. Do not proceed to deploy.sh switchover (Step H) without passing all parity tests. Do not release without integration deploy pass.

### Lesson 6: Golden Master Fixtures Must Be Copied

**Phase 1 Bug**: Fixture SQL files were not moved to `crates/ndp-lib/tests/fixtures/golden-master/`. Had to copy manually.

**Phase 2 Application**: ndp-validate does not use fixture SQL files. Its test data is inline or uses `tempfile`. However, the `schemas/` directory contains `stream-config.v1.1.schema.json` which is referenced by the standalone binary. Verify that the embedded schema approach (`SchemaValidator::default_schema()`) eliminates the need for file-based schema lookups in ndp-lib.

### Lesson 7: Move Order Matters

**Phase 1 Order**: error -> config -> db -> registry -> validation -> generators -> planner.

**Phase 2 Order**: error -> schema -> schema_gen -> semantic (mod, sources, source_path, dq_rules, gold, domain, table_exists). Then split cli.rs.

Rationale: error.rs has no internal deps. schema.rs depends on error. schema_gen depends on ndp-types (external). semantic/* depends on error. The semantic submodules have limited cross-deps (gold.rs and domain.rs are independent validators).

### Lesson 8: Convenience API Added Late

**Phase 1 Finding**: ndp_lib::gold module lacked `generate_stream()`, `sync_stream()` convenience functions expected by CLI. Had to add them during verification.

**Phase 2 Application**: Design the `ndp_lib::validate` public API upfront:
```rust
// ndp_lib::validate::mod.rs -- planned public API
pub fn validate_stream(config_path: &Path, options: &ValidateOptions) -> ValidationResult;
pub fn validate_all_streams(config_dir: &Path, options: &ValidateOptions) -> BatchValidationResult;
pub fn validate_domain(config_path: &Path, streams_dir: &Path, options: &ValidateOptions) -> ValidationResult;
pub fn validate_all_domains(domains_dir: &Path, streams_dir: &Path, options: &ValidateOptions) -> BatchValidationResult;
pub fn generate_schema() -> Result<String, SchemaGenError>;
pub fn verify_schema(schema_path: &Path) -> Result<bool, SchemaGenError>;
```

Where `ValidateOptions` consolidates the flags:
```rust
pub struct ValidateOptions {
    pub schema_only: bool,
    pub strict: bool,
    pub verbose: bool,
    pub format: OutputFormat,
}
```

This API should exist BEFORE `commands/validate.rs` is written.

---

## 10. Estimated Scope

### File Count

| Category | Files | Notes |
|----------|-------|-------|
| Source files to move | 11 | error.rs, schema.rs, schema_gen.rs, semantic/* (7 files), lib.rs rewrite |
| New files in ndp-lib | 3 | validate/mod.rs, validate/types.rs, validate/output.rs |
| Modified in ndp-lib | 2 | lib.rs (`pub mod validate`), Cargo.toml (new deps) |
| New files in ndp-cli | 1 | commands/validate.rs |
| Modified in ndp-cli | 2 | commands/mod.rs, main.rs |
| Modified in ndp-validate | 3 | Cargo.toml, lib.rs, main.rs (thin wrapper) |
| Modified deploy.sh | 1 | 2 dispatch sites |
| New release artifacts | 2 | manifest, CHANGELOG entry |
| **Total files touched** | **~25** | |

### Line Count Estimates

| Component | Lines | Notes |
|-----------|-------|-------|
| Source migration (11 files) | ~8,500 | 9,897 total minus cli.rs (1,370) minus main.rs (385) |
| cli.rs split: types to ndp-lib | ~350 | ValidationResult, BatchResult, output formatters, exit codes |
| cli.rs remaining in ndp-validate | ~1,020 | Cli struct, parsing tests (55 tests) |
| commands/validate.rs (new) | ~250 | Clap routing, library calls, exit code mapping |
| validate/mod.rs (new) | ~80 | Public API: validate_stream, validate_all, etc. |
| validate/types.rs (new) | ~200 | Moved from cli.rs |
| validate/output.rs (new) | ~120 | Moved from cli.rs |
| deploy.sh changes | ~30 | 2 dispatch sites (~15 lines each) |
| **Total new/modified lines** | **~1,050** | (excluding moved code which is ~8,500 lines unchanged) |

### Test Count

| Category | Tests | Notes |
|----------|-------|-------|
| Existing ndp-validate unit tests | 217 | 162 move to ndp-lib, 55 stay in ndp-validate (CLI parsing) |
| Existing ndp-validate doc tests | 5 (ignored) | Stay in ndp-validate |
| New parity tests | ~13 | Section 5 pre-integration tests |
| New exit code tests | ~6 | 0/1/2 for each mode |
| New integration E2E | ~9 | Section 5 integration environment tests |
| **Total expected** | **~245** | (217 migrated + ~28 new) |

### Cargo.toml Additions to ndp-lib

```toml
# crates/ndp-lib/Cargo.toml -- v1.1.15 additions

[dependencies]
# NEW for validate module:
jsonschema = "0.17"
sqlparser = { version = "0.50", features = ["visitor"] }
schemars = "0.8"
strsim = "0.11"
serde_yaml = "0.9"
regex = "1"

[dev-dependencies]
# NEW for validate module tests:
# (tokio-test is not needed -- validate tests are sync)
# sha2 already present from v1.1.14
```

### Risk-Adjusted Timeline

Based on Phase 1 experience (which took one session for migration + one session for integration testing and bug fixes):

| Phase | Effort | Risk Level |
|-------|--------|-----------|
| Steps A+B (setup) | Small | Low |
| Step C (file moves) | Medium | Low (proven pattern) |
| Step D (cli.rs split) | **Large** | **High** (no Phase 1 precedent) |
| Steps E+F (thin wrapper + CLI) | Medium | Medium |
| Step G (exit codes) | Medium | Medium |
| Step H (deploy.sh) | Small | Medium (BUG-004 class) |
| Step I (release) | Small | Low |
| Integration testing + bug fixes | Medium | High (expect 1-3 bugs per Phase 1) |

**Critical path**: Step D (cli.rs split) is the highest-risk, highest-effort step with no Phase 1 precedent. Budget extra time for this.

---

## Appendix A: Source File Migration Manifest

### Files Moving to ndp-lib (11 files)

| Source | Destination | Lines | Notes |
|--------|-------------|-------|-------|
| `tools/ndp-validate/src/error.rs` | `crates/ndp-lib/src/validate/error.rs` | 432 | ValidationError, ErrorCode, Severity, SchemaValidatorError |
| `tools/ndp-validate/src/schema.rs` | `crates/ndp-lib/src/validate/schema.rs` | 1,656 | SchemaValidator, DomainSchemaValidator |
| `tools/ndp-validate/src/schema_gen.rs` | `crates/ndp-lib/src/validate/schema_gen.rs` | 575 | generate_schema, verify_schema, compare_schemas |
| `tools/ndp-validate/src/semantic/mod.rs` | `crates/ndp-lib/src/validate/semantic/mod.rs` | 147 | SemanticValidator coordinator |
| `tools/ndp-validate/src/semantic/sources.rs` | `crates/ndp-lib/src/validate/semantic/sources.rs` | 602 | Source type validation |
| `tools/ndp-validate/src/semantic/source_path.rs` | `crates/ndp-lib/src/validate/semantic/source_path.rs` | 624 | Source path cross-reference |
| `tools/ndp-validate/src/semantic/dq_rules.rs` | `crates/ndp-lib/src/validate/semantic/dq_rules.rs` | 1,999 | DQ rule syntax validation |
| `tools/ndp-validate/src/semantic/gold.rs` | `crates/ndp-lib/src/validate/semantic/gold.rs` | 882 | Gold config semantic validation |
| `tools/ndp-validate/src/semantic/domain.rs` | `crates/ndp-lib/src/validate/semantic/domain.rs` | 926 | Domain config semantic validation |
| `tools/ndp-validate/src/semantic/table_exists.rs` | `crates/ndp-lib/src/validate/semantic/table_exists.rs` | 236 | Table existence check stub |

Total: 8,079 lines of library code moving.

### Files Created in ndp-lib (3 files)

| File | Content | Extracted From |
|------|---------|---------------|
| `crates/ndp-lib/src/validate/mod.rs` | Public API + re-exports | New (wraps moved modules) |
| `crates/ndp-lib/src/validate/types.rs` | ValidationResult, BatchValidationResult, ValidationSummary, BatchSummary, OutputFormat | cli.rs lines 29-338, 345-469 |
| `crates/ndp-lib/src/validate/output.rs` | output_json, output_human, format_error_human, format_warning_human, exit_codes, determine_exit_code | cli.rs lines 345-469 |

### Files Staying in ndp-validate (modified)

| File | Change |
|------|--------|
| `tools/ndp-validate/Cargo.toml` | Add `ndp-lib` dependency, remove moved deps |
| `tools/ndp-validate/src/lib.rs` | Rewrite as re-export from ndp_lib::validate |
| `tools/ndp-validate/src/main.rs` | Update imports to use ndp_lib::validate |
| `tools/ndp-validate/src/cli.rs` | Remove moved types, import from ndp_lib::validate::types |

### `use` Path Migration

| Old path | New path |
|----------|----------|
| `use crate::error::*` | `use crate::validate::error::*` (within ndp-lib) |
| `use crate::cli::ValidationResult` | `use crate::validate::types::ValidationResult` (within ndp-lib) |
| `use crate::schema::*` | `use crate::validate::schema::*` (within ndp-lib) |
| `use crate::semantic::*` | `use crate::validate::semantic::*` (within ndp-lib) |
| `use ndp_validate::*` | `use ndp_lib::validate::*` (in ndp-cli) |

---

## Appendix B: Flag Mapping Reference

| deploy.sh (v1.1.14) | deploy.sh (v1.1.15) | Notes |
|---------------------|---------------------|-------|
| `"$validate_tool" --domain "$config_file" --format human` | `"$ndp_tool" validate --domain "$config_file" --config-dir "$REPO_ROOT/config/base" --format human` | Site 3: validate_domain_configs. Added --config-dir. |
| `"$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human` | `"$ndp_tool" validate --domain "$config_file" --config-dir "$REPO_ROOT/config/base" --format human` | Site 4: handle_domain_declaration. Changed config-dir from streams to base. |

---

## Appendix C: ndp validate CLI Design

```
ndp validate [OPTIONS] [CONFIG_PATH]

OPTIONS:
    --all                    Validate all stream configs in config-dir/streams/
    --domain <PATH>          Validate a domain configuration file
    --domain-all             Validate all domain configs in config-dir/../domains/
    --generate-schema        Generate JSON Schema from ndp-types to stdout
    --verify-schema <PATH>   Verify committed schema matches generated
    --schema-only            Skip semantic validation (Layer 2)
    --strict                 Treat warnings as errors (exit 1)
    --format <FORMAT>        Output format: json (default) or human
    --config-dir <DIR>       Base config directory (default: config/base)
                             Streams resolved as <DIR>/streams/
                             Domains resolved as <DIR>/../domains/

ARGS:
    [CONFIG_PATH]            Single config file to validate

EXIT CODES:
    0  Validation passed (may have warnings)
    1  Validation failed (has errors, or --strict with warnings)
    2  System error (file not found, schema load failed)
```

### Subcommand-Free Design Rationale

Unlike `ndp gold` which uses subcommands (`generate`, `sync`, `recreate`) because those represent different **actions**, `ndp validate` uses flags because all modes represent the same action (validation) applied to different targets. This matches the standalone `ndp-validate` UX and avoids unnecessary nesting:

```
# Good: flat, matches standalone behavior
ndp validate --all
ndp validate --domain config/domains/indoor-air-quality/domain.json
ndp validate config/base/streams/air-quality/config.json

# Avoided: unnecessary nesting
ndp validate stream --all           # too much nesting
ndp validate domain indoor-air-quality  # breaks path convention
```

---

## Appendix D: Temporary State Between v1.1.15 and v1.1.16

After v1.1.15 deploys but before v1.1.16:

| Component | State | Notes |
|-----------|-------|-------|
| deploy.sh Gold dispatch | Uses `ndp gold` | From v1.1.14, unchanged |
| deploy.sh Validate dispatch | Uses `ndp validate` | **New in v1.1.15** |
| deploy.sh Dictionary/Dimension/Domain | Uses `ndp` | Unchanged |
| All 7 deploy.sh dispatch sites | Use `ndp` | **Goal achieved** |
| ndp-validate binary | Thin wrapper around ndp-lib | Still works standalone |
| ndp-gold-ddl binary | Thin wrapper around ndp-lib | Unchanged from v1.1.14 |
| Gold config validation | In BOTH `ndp_lib::gold::validation` AND `ndp_lib::validate::semantic::gold` | Duplication persists until v1.1.16 |
| `VALID_METRICS` constants | Defined in BOTH `ndp_lib::gold::generators::constants` AND `ndp_lib::validate::semantic::gold` | Duplication persists until v1.1.16 |
| NoOpDbClient | 3 copies in ndp-cli + 1 in ndp-lib | Dedup deferred to v1.1.16 |

**Key Point**: This temporary state is SAFE. Both gold validation and validate semantic gold are independently correct. They can diverge only if someone modifies one without the other -- unlikely given they are in the same crate (ndp-lib) and v1.1.16 follows immediately.
