# OPS-003 Phase 2 Specification: Validate Migration (v1.1.15)

> **Feature:** ops-003 Phase 2
> **Release:** v1.1.15
> **Created:** 2026-02-07
> **Status:** Specification
> **Specification Agent:** ndp-architect

---

## 1. Problem Statement

### 1.1 What We Are Migrating

The `ndp-validate` tool (13 source files, 9,897 lines, 222 tests) is a standalone binary that validates NDP stream and domain configurations against JSON Schema (Layer 1) and semantic business rules (Layer 2). It lives in `tools/ndp-validate/` and is called directly from deploy.sh at 2 dispatch sites.

### 1.2 Why We Are Migrating

1. **Single library goal.** OPS-003 consolidates all NDP actions into `ndp-lib`. Phase 1 (v1.1.14) moved Gold DDL generation; Phase 2 moves validation. After this release, 5 of 7 deploy.sh dispatch sites use the `ndp` binary, with validation completing the second batch.

2. **Cross-module validation.** Once validation lives in `ndp-lib`, Phase 3 (v1.1.16) can wire `ndp_lib::gold::sync()` to call `ndp_lib::validate::gold_config()` directly as a sibling module call. This is impossible while validation is in a separate crate without adding a dependency tangle.

3. **Shared constants.** `ndp-validate`'s `semantic/gold.rs` defines `VALID_METRICS` and `VALID_STATS` locally. The same constants exist in `ndp-gold-ddl` (now `ndp_lib::gold`). Migration into sibling modules is the prerequisite for the Phase 3 deduplication.

4. **Agent navigation.** When deploy.sh validation fails, agents currently must distinguish between `ndp-validate` (standalone) and `ndp` (CLI). After this release, all agents investigate one codebase: `ndp-lib`.

5. **deploy.sh no-fallback completion.** Phase 1 switched Gold dispatch to `ndp` with no-fallback error semantics. Phase 2 does the same for validation, eliminating the last `warn + return 0` silent skip patterns in deploy.sh.

### 1.3 What Changes (Amendment 2026-02-08)

- **Drop YAML config parsing.** All stream/domain configs are JSON (migrated in v1.1.8). The YAML code paths in `main.rs:92-104` (format auto-detection) and `semantic/domain.rs:499-565` (stream discovery fallback to `config.yaml`/`config.yml`) are dead code. Do NOT add `serde_yaml` to ndp-lib. Strip these paths during migration. Stale `.yaml` stream config files will be renamed to `.yaml.bak` in Phase 3.

### 1.4 What Does NOT Change

- Validation logic itself (no new rules, no removed rules)
- Output format (JSON and human-readable formats are byte-identical)
- Exit codes (0/1/2 per dp-019 specification)
- Error codes (all `ErrorCode` variants preserved)
- Test behavior (222 tests migrate, assertions unchanged; YAML-specific tests removed or converted to JSON)

---

## 2. Requirements

### 2.1 Functional Requirements

#### FR-01: Module Migration (ops-003-05)

All validation logic from `tools/ndp-validate/src/` must be available under `ndp_lib::validate`. The public API must expose:

| Function / Type | Purpose |
|----------------|---------|
| `validate_stream(config: &Value, opts: &ValidateOptions) -> ValidationResult` | Single stream config validation |
| `validate_stream_file(path: &Path, opts: &ValidateOptions) -> Result<ValidationResult>` | Stream validation from file path |
| `validate_all_streams(config_dir: &Path, opts: &ValidateOptions) -> Result<BatchValidationResult>` | Batch stream validation |
| `validate_domain(config: &Value, streams_dir: Option<&Path>) -> ValidationResult` | Single domain config validation |
| `validate_domain_file(path: &Path, opts: &ValidateOptions) -> Result<ValidationResult>` | Domain validation from file path |
| `validate_all_domains(domains_dir: &Path, config_dir: &Path, opts: &ValidateOptions) -> Result<BatchValidationResult>` | Batch domain validation |
| `generate_schema() -> Result<String, SchemaGenError>` | Generate JSON Schema from ndp-types |
| `verify_schema(path: &Path) -> Result<bool, SchemaGenError>` | Verify committed schema matches generated |
| `compare_schemas(path: &Path) -> Result<Vec<String>, SchemaGenError>` | Show differences between committed and generated schema |
| `ValidationResult` | Single config result (valid, errors, warnings) |
| `BatchValidationResult` | Multiple config results with summary |
| `ValidationError` | Structured validation error |
| `ErrorCode` | Error code enumeration |
| `Severity` | Error/Warning severity |
| `ValidationLayer` | Syntax/Schema/Semantic layer |
| `OutputFormat` | Json/Human format enum |
| `SchemaValidator` | JSON Schema validator (Layer 1) |
| `DomainSchemaValidator` | Domain-specific schema validator |
| `SemanticValidator` | Semantic validator coordinator (Layer 2) |
| `ValidateOptions` | Options struct (schema_only, strict, verbose, check_tables, format) |

**FR-01a: Deduplicate `is_valid_granularity()`**

During migration, `is_valid_granularity()` must be extracted from its duplicate locations (`semantic/gold.rs:430` and `semantic/domain.rs:403`) into a single shared implementation in `validate/semantic/common.rs` (or `validate/semantic/mod.rs`). Both `gold.rs` and `domain.rs` must call the shared function. This is explicitly required by SCOPE.md v1.1.15.

#### FR-02: CLI Commands (ops-003-06)

The `ndp validate` subcommand must support all operations that `ndp-validate` supports. Flag mapping from standalone to unified CLI:

| ndp-validate (standalone) | ndp validate (unified) | Notes |
|---------------------------|----------------------|-------|
| `ndp-validate <path>` | `ndp validate --stream <path>` | Positional becomes named |
| `ndp-validate --all` | `ndp validate --all` | Unchanged |
| `ndp-validate --domain <path>` | `ndp validate --domain <path>` | Unchanged |
| `ndp-validate --domain-all` | `ndp validate --domain-all` | Unchanged |
| `ndp-validate --generate-schema` | `ndp validate --schema --generate` | Flat flags (not subcommand) |
| `ndp-validate --generate-schema --output FILE` | `ndp validate --schema --generate --output FILE` | Flat flags |
| `ndp-validate --verify-schema PATH` | `ndp validate --schema --verify <path>` | Flat flags (not subcommand) |
| `ndp-validate --schema-only <path>` | `ndp validate --stream <path> --schema-only` | Unchanged |
| `ndp-validate --check-tables --timescale-url URL <path>` | `ndp validate --stream <path> --check-tables` | Uses global `--db-url` |
| `ndp-validate --format json` | `ndp validate --format json` | Unchanged |
| `ndp-validate --format human` | `ndp validate --format human` | Unchanged |
| `ndp-validate --strict` | `ndp validate --strict` | Unchanged |
| `ndp-validate --verbose` | (global) `RUST_LOG=info` | Verbose via tracing, not a flag |
| `ndp-validate --config-dir DIR` | (global) `--config-dir DIR` | ndp-cli global flag |
| `ndp-validate --timescale-url URL` | (global) `--db-url URL` | Harmonized with ndp-cli convention |
| `ndp-validate --schema-path PATH` | `ndp validate --schema-path PATH` | Unchanged |
| `ndp-validate --domain-schema-path PATH` | `ndp validate --domain-schema-path PATH` | Unchanged |
| `ndp-validate --domains-dir DIR` | `ndp validate --domains-dir DIR` | Unchanged |

**Design decision: `--verbose` maps to tracing, not a dedicated flag.**

The standalone `ndp-validate` has a `--verbose` flag that calls `eprintln!()` at various progress points. The `ndp` CLI uses the `tracing` crate with `RUST_LOG` for verbosity control. Rather than maintaining a dedicated `--verbose` flag, Phase 2 replaces `eprintln!` verbose calls with `tracing::info!()` calls in the library, which the CLI controls via `RUST_LOG=info`. This is consistent with Phase 1 (Gold commands use the same pattern).

**Design decision: `--timescale-url` maps to `--db-url`.**

The standalone tool uses `--timescale-url` and `TIMESCALE_URL` env var. The ndp CLI already has `--db-url` as a global flag that reads `TIMESCALE_URL` env var. No new flag needed -- `--check-tables` uses the existing global `--db-url`.

**Design decision: Schema operations use flat flags `--schema --generate` / `--schema --verify`.**

`ndp-validate --generate-schema` and `--verify-schema` are not stream validation -- they are schema management operations. In the ndp CLI, they become `ndp validate --schema --generate` and `ndp validate --schema --verify <path>`. This uses flat flags consistent with the rest of the `ndp validate` interface (per CLI UX Design doc, Section "ndp validate"), rather than introducing a subcommand nesting level.

#### FR-03: deploy.sh Switchover (ops-003-07)

Both deploy.sh dispatch sites that call `ndp-validate` must be switched to `ndp validate` with no-fallback error semantics.

### 2.2 Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-01 | Zero logic changes to validation rules | Migration, not refactoring |
| NFR-02 | Output byte-identical for JSON format | Ensures deploy.sh grep patterns work |
| NFR-03 | Exit codes 0/1/2 preserved | dp-019 specification compliance |
| NFR-04 | All 222 tests pass under new module paths | No regression |
| NFR-05 | Binary size increase < 2MB | Raspberry Pi deployment constraint |
| NFR-06 | `ndp-validate` standalone remains buildable | D4 from SCOPE.md |

---

## 3. Module Structure

### 3.1 Target Layout in `crates/ndp-lib/src/validate/`

```
crates/ndp-lib/src/validate/
  mod.rs                  Public API: validate_stream(), validate_domain(), validate_all_*()
  error.rs                ValidationError, ErrorCode, Severity, ValidationLayer, SchemaValidatorError
  result.rs               ValidationResult, BatchValidationResult, ValidationSummary, BatchSummary
  format.rs               OutputFormat, output_json(), output_human(), exit_codes
  options.rs              ValidateOptions (schema_only, strict, check_tables, format)
  schema.rs               SchemaValidator, DomainSchemaValidator
  schema_gen.rs           generate_schema(), verify_schema(), compare_schemas(), SchemaGenError
  semantic/
    mod.rs                SemanticValidator coordinator
    common.rs             Shared helpers: is_valid_granularity() (FR-01a)
    sources.rs            Source config validation (FR-020)
    source_path.rs        Source path cross-reference (FR-022)
    dq_rules.rs           DQ rule syntax validation
    gold.rs               Gold ETL semantic validation (FE-001)
    domain.rs             Domain semantic validation (FE-001)
    table_exists.rs       Table existence checking (FR-023)
```

### 3.2 File Origin Mapping

| # | Source | Lines | Destination | Migration Notes |
|---|--------|-------|-------------|----------------|
| 1 | `cli.rs` | 1370 | Split into `result.rs` (types), `format.rs` (output), `options.rs` (options) | `Cli` struct stays in ndp-cli. Types and formatters move to lib. |
| 2 | `error.rs` | 432 | `validate/error.rs` | Unchanged. Does NOT merge with `ndp_lib::error::NdpLibError` -- these are validation-specific error types, not library errors. |
| 3 | `schema.rs` | 1656 | `validate/schema.rs` | `use crate::error::` becomes `use super::error::` |
| 4 | `schema_gen.rs` | 575 | `validate/schema_gen.rs` | `use ndp_types::` unchanged (ndp-lib already depends on ndp-types) |
| 5 | `semantic/mod.rs` | 147 | `validate/semantic/mod.rs` | `use crate::error::` becomes `use super::super::error::` (or `use crate::validate::error::`) |
| 6 | `semantic/sources.rs` | 602 | `validate/semantic/sources.rs` | Same import path change |
| 7 | `semantic/source_path.rs` | 624 | `validate/semantic/source_path.rs` | Same import path change |
| 8 | `semantic/dq_rules.rs` | 1999 | `validate/semantic/dq_rules.rs` | Same import path change |
| 9 | `semantic/gold.rs` | 882 | `validate/semantic/gold.rs` | Same import path change. Local VALID_METRICS stays for now (Phase 3 dedup) |
| 10 | `semantic/domain.rs` | 926 | `validate/semantic/domain.rs` | Same import path change |
| 11 | `semantic/table_exists.rs` | 236 | `validate/semantic/table_exists.rs` | Same import path change |
| 12 | `lib.rs` | 63 | Absorbed into `validate/mod.rs` | Re-exports become module-level pub use |
| 13 | `main.rs` | 385 | **Stays** in ndp-validate (thin wrapper) | Rewired to call `ndp_lib::validate::*` |

**Totals:**
- Source files moved: 11 (#1-11, split into 14 destination files due to cli.rs decomposition)
- Source files staying: 1 (#13, main.rs rewired as thin wrapper)
- Source lines moved: ~9,449 (9,897 total minus ~448 for main.rs which stays/is rewritten)

### 3.3 The `cli.rs` Decomposition

The standalone `cli.rs` (1370 lines) contains 4 concerns that must be separated for library extraction:

1. **`Cli` struct + `validate_args()`** (lines 82-215) -- CLI argument parsing via clap `#[derive(Parser)]`. This is CLI-specific and does NOT move to ndp-lib. It becomes `tools/ndp-cli/src/commands/validate.rs`.

2. **Result types** (lines 217-338) -- `ValidationResult`, `BatchValidationResult`, `ValidationSummary`, `BatchSummary`. These are library types. Move to `validate/result.rs`.

3. **Output formatting** (lines 340-469) -- `OutputFormat`, `output_json()`, `output_human()`, `output_json_batch()`, `output_human_batch()`, `determine_exit_code()`, `determine_batch_exit_code()`, `exit_codes` module. Move to `validate/format.rs`.

4. **Tests** (lines 476-1370) -- 65 tests. Tests for `Cli` struct stay in ndp-cli or are removed (they test clap parsing, which changes for the unified CLI). Tests for result types and formatting move with their code.

**Test redistribution from cli.rs (65 tests):**

| Test Category | Count | Destination | Rationale |
|---------------|-------|-------------|-----------|
| CLI structure/parsing | 35 | New tests in `commands/validate.rs` | Tests need to verify new ndp validate arg parsing instead |
| Result types (ValidationResult, Batch) | 7 | `validate/result.rs` tests | Logic unchanged, only import paths change |
| Output formatting (JSON, human) | 10 | `validate/format.rs` tests | Logic unchanged |
| Exit code determination | 8 | `validate/format.rs` tests | Logic unchanged |
| OutputFormat default | 1 | `validate/format.rs` tests | Logic unchanged |
| Argument validation (validate_args) | 4 | New tests in `commands/validate.rs` | Different arg structure in ndp CLI |

Net: ~26 tests migrate directly (result + formatting + exit code). ~35 CLI parsing tests must be rewritten for the new `ndp validate` arg structure. ~4 arg validation tests are rewritten.

### 3.4 `validate/options.rs` -- New Type

The standalone `Cli` struct mixes CLI parsing with validation options. The library needs a CLI-agnostic options struct:

```rust
/// Options controlling validation behavior.
///
/// Constructed from CLI args in ndp-cli, or directly in library consumers.
pub struct ValidateOptions {
    /// Skip semantic validation (Layer 2), only run schema validation
    pub schema_only: bool,
    /// Treat warnings as errors (exit code 1 if any warnings)
    pub strict: bool,
    /// Check that Silver tables exist in TimescaleDB
    pub check_tables: bool,
    /// Output format
    pub format: OutputFormat,
    /// Base config directory for resolving sibling configs
    pub config_dir: PathBuf,
    /// JSON Schema file path for stream configs
    pub schema_path: Option<PathBuf>,
    /// JSON Schema file path for domain configs
    pub domain_schema_path: Option<PathBuf>,
    /// Directory containing domain configs
    pub domains_dir: Option<PathBuf>,
    /// Database URL for table existence checks
    pub db_url: Option<String>,
}

impl Default for ValidateOptions {
    fn default() -> Self {
        Self {
            schema_only: false,
            strict: false,
            check_tables: false,
            format: OutputFormat::Json,
            config_dir: PathBuf::from("config/base/streams"),
            schema_path: None,
            domain_schema_path: None,
            domains_dir: None,
            db_url: None,
        }
    }
}
```

---

## 4. CLI Command Design

### 4.1 ndp-cli Changes

#### 4.1.1 New `Commands` Variant

```rust
// tools/ndp-cli/src/main.rs
#[derive(Subcommand)]
enum Commands {
    Dictionary(commands::dictionary::DictionaryArgs),
    Dimension(commands::dimension::DimensionArgs),
    Domain(commands::domain::DomainArgs),
    Gold(commands::gold::GoldArgs),
    /// Config validation operations.
    Validate(commands::validate::ValidateArgs),  // NEW
}
```

#### 4.1.2 `commands/validate.rs` Structure

```rust
use clap::Args;
use std::path::PathBuf;

/// Configuration validation operations.
#[derive(Args)]
pub struct ValidateArgs {
    /// Validate a single stream config file.
    #[arg(long, conflicts_with_all = ["all", "domain", "domain_all", "schema"])]
    pub stream: Option<PathBuf>,

    /// Validate all stream configs in config directory.
    #[arg(short, long, conflicts_with_all = ["stream", "domain", "domain_all", "schema"])]
    pub all: bool,

    /// Validate a single domain config file.
    #[arg(long, conflicts_with_all = ["stream", "all", "domain_all", "schema"])]
    pub domain: Option<PathBuf>,

    /// Validate all domain configs.
    #[arg(long, conflicts_with_all = ["stream", "all", "domain", "schema"])]
    pub domain_all: bool,

    /// Schema management mode (combine with --generate or --verify).
    #[arg(long, conflicts_with_all = ["stream", "all", "domain", "domain_all"])]
    pub schema: bool,

    /// Generate JSON Schema from ndp-types (requires --schema).
    #[arg(long, requires = "schema", conflicts_with = "verify")]
    pub generate: bool,

    /// Verify committed schema matches generated (requires --schema).
    #[arg(long, requires = "schema", conflicts_with = "generate", value_name = "PATH")]
    pub verify: Option<PathBuf>,

    /// Write generated schema to file instead of stdout (with --schema --generate).
    #[arg(long, requires = "generate")]
    pub output: Option<PathBuf>,

    /// Skip semantic validation (Layer 2), only run schema validation.
    #[arg(long)]
    pub schema_only: bool,

    /// Check that Silver tables exist in TimescaleDB (requires --db-url).
    #[arg(long)]
    pub check_tables: bool,

    /// Output format (json or human).
    #[arg(long, value_enum, default_value = "json")]
    pub format: ndp_lib::validate::OutputFormat,

    /// Treat warnings as errors.
    #[arg(long)]
    pub strict: bool,

    /// JSON Schema file path for stream configs.
    #[arg(long)]
    pub schema_path: Option<PathBuf>,

    /// JSON Schema file path for domain configs.
    #[arg(long)]
    pub domain_schema_path: Option<PathBuf>,

    /// Directory containing domain configs.
    #[arg(long)]
    pub domains_dir: Option<PathBuf>,
}
```

#### 4.1.3 Routing Logic

The `validate` command does NOT require a database URL (unless `--check-tables` is used). This matches `gold generate` -- the main.rs routing must not call `require_db_url()` unconditionally.

```rust
Commands::Validate(args) => {
    commands::validate::run(args, &config_dir, db_url.as_deref()).await?;
}
```

#### 4.1.4 Argument Validation

The `run()` function must validate that:
- At least one of `--stream`, `--all`, `--domain`, `--domain-all`, or `--schema` is provided
- `--schema` requires either `--generate` or `--verify <path>` (error if `--schema` alone)
- `--check-tables` requires `--db-url` (global flag)
- `--stream` and `--domain` paths exist as files
- When `--all` is used, the config directory exists

These checks mirror `cli.rs::validate_args()` but adapted for the new flat-flag structure.

### 4.2 deploy.sh Invocation Mapping

After switchover, deploy.sh calls `ndp validate` like this:

| Operation | Standalone | Unified |
|-----------|-----------|---------|
| Validate domain config | `"$validate_tool" --domain "$config_file" --format human` | `"$ndp_tool" validate --domain "$config_file" --format human` |
| Validate domain with config dir | `"$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human` | `"$ndp_tool" validate --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human` |

Note: deploy.sh config variables (`$CONFIG_STREAMS_DIR`, `$CONFIG_DOMAINS_DIR`) are authoritative and unchanged. `ndp validate --config-dir` accepts the streams directory directly, matching standalone behavior.

---

## 5. deploy.sh Changes

### 5.1 Site 3: `validate_domain_configs()` (lines 1533-1596)

**Current code (lines 1533-1548):**

```bash
# Find ndp-validate tool
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
```

**Replacement:**

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
```

**Usage change (line 1584):**

Before: `"$validate_tool" --domain "$config_file" --format human`
After: `"$ndp_tool" validate --domain "$config_file" --format human`

**Key differences:**
- `error` + `return 1` instead of `warn` + `return 0`
- Resolves `ndp`, not `ndp-validate`
- Command becomes `ndp validate --domain` instead of `ndp-validate --domain`

### 5.2 Site 4: `handle_domain_declaration()` (lines 2032-2054)

**Current code (lines 2032-2053):**

```bash
# Phase B (FE-002): Validate domain config using ndp-validate
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

if [ -n "$validate_tool" ]; then
    log "  Validating domain config..."
    if ! "$validate_tool" --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human; then
        error "Domain config validation failed: $config_file"
        return 1
    fi
    log "  Domain config validation passed"
else
    warn "  ndp-validate not available, skipping domain validation"
    warn "  Build with: cargo build -p ndp-validate --release"
fi
```

**Replacement:**

```bash
# Validate domain config using ndp (required -- no fallback)
# ndp_tool already resolved earlier in this function (from v1.1.14 gold switchover at line ~2068)
if [ -z "$ndp_tool" ]; then
    # Resolve if not already done (defensive -- should already be resolved)
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
fi

log "  Validating domain config..."
if ! "$ndp_tool" validate --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human; then
    error "Domain config validation failed: $config_file"
    return 1
fi
log "  Domain config validation passed"
```

**Key differences:**
- Reuses `ndp_tool` variable from gold dispatch in the same function
- Eliminates `warn + skip` path -- validation failure is always an error
- `--config-dir "$CONFIG_STREAMS_DIR"` preserved (deploy.sh config vars are authoritative)

### 5.3 Safety Principles (per SCOPE.md)

1. **No fallback.** `error` + `return 1` if `ndp` is not found. Never `warn` + `return 0`.
2. **Same 4-way resolution.** `command -v`, `/opt/ndp/bin/`, `target/release/`, `target/debug/`.
3. **Atomic switchover.** Both validate dispatch sites switch in the same release.
4. **Integration test before release.** `DEPLOY_ENV=integration ./deploy.sh apply` must complete.
5. **Variable reuse.** Site 4 reuses `$ndp_tool` from the gold dispatch block that precedes it in `handle_domain_declaration()`.

---

## 6. Config Path Convention

### 6.1 The Problem

The standalone `ndp-validate` has a `--config-dir` flag that defaults to `config/base/streams` -- i.e., it points directly to the streams directory. The ndp CLI has a global `--config-dir` that defaults to `config/base` -- the base directory that **contains** `streams/`, `dimensions/`, etc.

These are different directories:
- `ndp-validate --config-dir config/base/streams` (streams dir)
- `ndp --config-dir config/base` (base dir, one level up)

### 6.2 Resolution Strategy

The validate command handler in ndp-cli must derive the streams directory from the base config dir:

```rust
// tools/ndp-cli/src/commands/validate.rs
pub async fn run(
    args: ValidateArgs,
    base_config_dir: &Path,  // e.g., config/base
    db_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Streams live under base/streams/
    let streams_dir = base_config_dir.join("streams");

    // Config root is the parent of base (e.g., config/)
    let config_root = base_config_dir
        .parent()
        .unwrap_or(base_config_dir);

    // Domains live under config_root/domains/
    let domains_dir = args.domains_dir
        .unwrap_or_else(|| config_root.join("domains"));

    // Build ValidateOptions
    let opts = ndp_lib::validate::ValidateOptions {
        config_dir: streams_dir,
        domains_dir: Some(domains_dir),
        // ...
    };
}
```

**deploy.sh config-dir convention.** deploy.sh defines `CONFIG_STREAMS_DIR="$REPO_ROOT/config/base/streams"` and `CONFIG_DOMAINS_DIR="$REPO_ROOT/config/domains"` at lines 73-74. These variables are authoritative. `ndp validate --config-dir` accepts the streams directory directly, matching standalone `ndp-validate` behavior. The BUG-004 config-dir issue was resolved in Phase 1; no deploy.sh path changes are needed for Phase 2.

### 6.3 Path Resolution Table

| deploy.sh passes | `ndp validate --config-dir` receives | Used for |
|------------------|--------------------------------------|----------|
| `$CONFIG_STREAMS_DIR` (`config/base/streams`) | Streams directory directly | Domain semantic validation (cross-ref stream configs) |
| (none) | Not passed at Site 3 | Site 3 doesn't need streams cross-ref |
| (none, default) | `config/base/streams` | Default stream path for `--all` mode |

### 6.4 Schema File Resolution

The standalone tool defaults to `schemas/stream-config.v1.1.schema.json` and `config/schemas/domain.schema.json`. These are relative paths that work from the repo root. The ndp CLI runs from the same repo root, so these defaults work unchanged. The validate module accepts optional overrides via `ValidateOptions.schema_path` and `ValidateOptions.domain_schema_path`.

---

## 7. Exit Code Mapping

### 7.1 dp-019 Exit Codes

| Code | Meaning | When |
|------|---------|------|
| **0** | Success | Validation passed (may have warnings unless `--strict`) |
| **1** | Validation error | At least one error found, or warnings with `--strict` |
| **2** | System error | File not found, schema load failed, parse error, DB connection failed |

### 7.2 Preservation Strategy

The `exit_codes` module and `determine_exit_code()` / `determine_batch_exit_code()` functions move into `ndp_lib::validate::format`. The ndp-cli `commands/validate.rs` calls these functions and converts the result to `std::process::ExitCode`.

```rust
// commands/validate.rs
let code = ndp_lib::validate::determine_exit_code(&result, args.strict);
std::process::exit(code);
```

The ndp-cli main.rs currently returns `Result<(), Box<dyn std::error::Error>>`. For validate commands that need non-zero exit codes, the handler must call `std::process::exit()` directly, since returning `Err` from main would print the error and exit with code 1 (losing the distinction between exit code 1 and 2).

This matches the standalone `ndp-validate` which uses `ExitCode::from()`.

### 7.3 Error Handling Flow

```
Library function fails (file not found, schema load)
  -> Returns Result::Err(...)
  -> CLI catches, prints to stderr, exits with code 2

Validation finds errors
  -> Returns ValidationResult { valid: false, errors: [...] }
  -> CLI formats output (JSON/human), exits with code 1

Validation passes (maybe warnings)
  -> Returns ValidationResult { valid: true, warnings: [...] }
  -> CLI formats output, exits with code 0 (or 1 if --strict and warnings)
```

---

## 8. Dependencies

### 8.1 Crate Dependencies to Add to `ndp-lib`

The following dependencies are currently in `tools/ndp-validate/Cargo.toml` but not in `crates/ndp-lib/Cargo.toml`:

| Dependency | Version | Current in ndp-lib? | Purpose |
|------------|---------|---------------------|---------|
| `jsonschema` | `0.17` | No | Layer 1 schema validation |
| `schemars` | `0.8` | No | JSON Schema generation from Rust types |
| `sqlparser` | `0.50` (features: `["visitor"]`) | No | DQ rule SQL syntax validation |
| `strsim` | `0.11` | No | Levenshtein distance for suggestions |
| `regex` | `1.0` | No | Pattern validation in gold.rs |
| ~~`serde_yaml`~~ | ~~`0.9`~~ | ~~No~~ | ~~YAML config file parsing~~ — **DROPPED** (Amendment 1.3: all configs are JSON) |
| `clap` | `4` (features: `["derive", "env"]`) | No | Only `ValueEnum` derive for `OutputFormat` |

**Note on clap:** The `OutputFormat` enum uses `#[derive(ValueEnum)]` for clap integration. Two options:
1. Add `clap` as a dependency of ndp-lib (pulls in the whole clap tree).
2. Move the `ValueEnum` derive to the CLI side and use a plain enum in ndp-lib.

**Decision:** Option 2. The `OutputFormat` enum in `ndp_lib::validate::format` is a plain enum without clap derives. The CLI `commands/validate.rs` has its own `OutputFormat` enum with `ValueEnum` that converts to the library enum. This keeps ndp-lib free of CLI framework dependencies.

### 8.2 Updated `crates/ndp-lib/Cargo.toml` (additions for v1.1.15)

```toml
[dependencies]
# ... existing ...

# Validation dependencies (v1.1.15)
jsonschema = "0.17"
sqlparser = { version = "0.50", features = ["visitor"] }
schemars = "0.8"
strsim = "0.11"
# serde_yaml removed — all NDP configs are JSON (Amendment 1.3)
regex = "1"

[dev-dependencies]
# ... existing ...
tempfile = "3"     # already present
tokio-test = "0.4" # needed for validate async tests (if any)
```

### 8.3 Dependency Note: `clap` NOT Added to ndp-lib

ndp-lib must remain CLI-framework-agnostic. The `OutputFormat` enum is defined as a plain Rust enum in the library. The CLI wraps it with clap's `ValueEnum`. This is the same pattern used for `SyncOptions` -- the library type has no CLI annotations.

### 8.4 Dependency Compatibility

All added dependencies are already used in the workspace (ndp-validate currently depends on them). No version conflicts expected. The workspace-level `Cargo.toml` should be checked for workspace-level dependency declarations; if these are not already declared there, they need individual version specifications in ndp-lib's Cargo.toml.

---

## 9. Migration Checklist

### 9.1 File-by-File Migration Plan

Each step below is an atomic, testable operation. Run `cargo check -p ndp-lib` after each step.

#### Step 1: Create Module Skeleton

Create empty module files and register `validate` in `ndp_lib::lib.rs`:

```
crates/ndp-lib/src/validate/mod.rs      (empty pub mod declarations)
crates/ndp-lib/src/validate/error.rs    (empty)
crates/ndp-lib/src/validate/result.rs   (empty)
crates/ndp-lib/src/validate/format.rs   (empty)
crates/ndp-lib/src/validate/options.rs  (empty)
crates/ndp-lib/src/validate/schema.rs   (empty)
crates/ndp-lib/src/validate/schema_gen.rs (empty)
crates/ndp-lib/src/validate/semantic/mod.rs      (empty)
crates/ndp-lib/src/validate/semantic/common.rs    (empty — will hold is_valid_granularity)
crates/ndp-lib/src/validate/semantic/sources.rs   (empty)
crates/ndp-lib/src/validate/semantic/source_path.rs (empty)
crates/ndp-lib/src/validate/semantic/dq_rules.rs (empty)
crates/ndp-lib/src/validate/semantic/gold.rs     (empty)
crates/ndp-lib/src/validate/semantic/domain.rs   (empty)
crates/ndp-lib/src/validate/semantic/table_exists.rs (empty)
```

Add `pub mod validate;` to `crates/ndp-lib/src/lib.rs`.

Verify: `cargo check -p ndp-lib`

#### Step 2: Add Dependencies

Add validation dependencies to `crates/ndp-lib/Cargo.toml` (see Section 8.2).

Verify: `cargo check -p ndp-lib`

#### Step 3: Migrate Error Types

Copy `tools/ndp-validate/src/error.rs` to `crates/ndp-lib/src/validate/error.rs`.

Import path changes:
- None (error.rs only depends on `serde`, `thiserror`, `serde_json` -- all already in ndp-lib)

Verify: `cargo check -p ndp-lib` and `cargo test -p ndp-lib -- validate::error`

#### Step 4: Migrate Result Types

Extract from `tools/ndp-validate/src/cli.rs` lines 217-338:
- `ValidationSummary`
- `ValidationResult`
- `BatchValidationResult`
- `BatchSummary`

Write to `crates/ndp-lib/src/validate/result.rs`.

Import path changes:
- `use crate::error::` -> `use super::error::`
- Add `use super::error::{ValidationError, ValidationLayer};`

Move associated tests (7 tests: `test_validation_result_new`, `test_validation_result_add_error`, `test_validation_result_add_warning`, `test_batch_validation_result`, and `has_issues`).

Verify: `cargo test -p ndp-lib -- validate::result`

#### Step 5: Migrate Output Formatting

Extract from `tools/ndp-validate/src/cli.rs` lines 28-38 and 340-469:
- `exit_codes` module
- `OutputFormat` enum (WITHOUT clap derives)
- `output_json()`, `output_json_batch()`
- `output_human()`, `output_human_batch()`
- `format_error_human()`, `format_warning_human()`
- `determine_exit_code()`, `determine_batch_exit_code()`

Write to `crates/ndp-lib/src/validate/format.rs`.

Import path changes:
- `use crate::error::` -> `use super::error::`
- Add `use super::result::{ValidationResult, BatchValidationResult};`

Remove `#[derive(ValueEnum)]` from `OutputFormat` -- library enum is plain.

Move associated tests (19 tests for output formatting and exit codes).

Verify: `cargo test -p ndp-lib -- validate::format`

#### Step 6: Create ValidateOptions

Write new `crates/ndp-lib/src/validate/options.rs` with the `ValidateOptions` struct from Section 3.4.

Verify: `cargo check -p ndp-lib`

#### Step 7: Migrate Schema Validator

Copy `tools/ndp-validate/src/schema.rs` to `crates/ndp-lib/src/validate/schema.rs`.

Import path changes:
- `use crate::error::` -> `use super::error::`

Verify: `cargo test -p ndp-lib -- validate::schema` (47 tests)

#### Step 8: Migrate Schema Generation

Copy `tools/ndp-validate/src/schema_gen.rs` to `crates/ndp-lib/src/validate/schema_gen.rs`.

Import path changes:
- `use ndp_types::` stays unchanged (ndp-lib depends on ndp-types)

Verify: `cargo test -p ndp-lib -- validate::schema_gen` (9 tests)

#### Step 9: Migrate Semantic Validators

Copy all files from `tools/ndp-validate/src/semantic/` to `crates/ndp-lib/src/validate/semantic/`.

Import path changes for each file:
- `use crate::error::` -> `use crate::validate::error::`

During this step, deduplicate `is_valid_granularity()`:
1. Create `validate/semantic/common.rs` with the single `pub(crate) fn is_valid_granularity()` implementation (taken from either `gold.rs:430` or `domain.rs:403` -- they are identical).
2. Remove the local `fn is_valid_granularity()` from both `gold.rs` and `domain.rs`.
3. Replace calls with `use super::common::is_valid_granularity;` in both files.
4. Move the `is_valid_granularity` unit tests to `common.rs`.

Verify: `cargo test -p ndp-lib -- validate::semantic` (91 tests)

#### Step 10: Wire Public API

Write `crates/ndp-lib/src/validate/mod.rs` with:
- `pub mod` declarations for all submodules
- Public API functions: `validate_stream()`, `validate_stream_file()`, `validate_all_streams()`, `validate_domain()`, `validate_domain_file()`, `validate_all_domains()`
- Re-exports for key types

The convenience functions extract the validation orchestration logic from `main.rs`:

```rust
/// Validate a single stream config from a file path.
pub fn validate_stream_file(
    path: &Path,
    opts: &ValidateOptions,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    // Read file, detect format (JSON/YAML), parse, run schema + semantic
    // This is the logic currently in main.rs::run_validation()
}

/// Validate all stream configs in a directory.
pub fn validate_all_streams(
    config_dir: &Path,
    opts: &ValidateOptions,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>> {
    // Discover configs, validate each, aggregate
    // This is the logic currently in main.rs::run_validation() with --all
}
```

Verify: `cargo test -p ndp-lib -- validate::` (all 222 tests, minus CLI parsing tests, plus convenience API tests)

#### Step 11: Add CLI Command

Create `tools/ndp-cli/src/commands/validate.rs` with `ValidateArgs` and `run()`.

Register in `tools/ndp-cli/src/commands/mod.rs` and `tools/ndp-cli/src/main.rs`.

Write new CLI parsing tests for the ndp validate flag structure.

Verify: `cargo build -p ndp-cli` and manual `ndp validate --all` test.

#### Step 12: CLI Parity Testing

Compare output of `ndp-validate` and `ndp validate` for all test cases:

```bash
# Stream validation
diff <(ndp-validate --all --config-dir config/base/streams --format json) \
     <(ndp validate --all --config-dir config/base --format json)

# Domain validation
diff <(ndp-validate --domain config/domains/indoor-air-quality/domain.json --format json) \
     <(ndp validate --domain config/domains/indoor-air-quality/domain.json --config-dir config/base --format json)

# Schema generation
diff <(ndp-validate --generate-schema) \
     <(ndp validate --schema --generate)

# Schema verification
diff <(ndp-validate --verify-schema schemas/stream-config.v1.1.schema.json 2>&1; echo "exit:$?") \
     <(ndp validate --schema --verify schemas/stream-config.v1.1.schema.json 2>&1; echo "exit:$?")
```

#### Step 13: Update ndp-validate Thin Wrapper

Rewrite `tools/ndp-validate/src/lib.rs` and `tools/ndp-validate/src/main.rs` to re-export from `ndp_lib::validate`.

Update `tools/ndp-validate/Cargo.toml` to depend on `ndp-lib` instead of duplicating dependencies.

Verify: `cargo build -p ndp-validate` and `ndp-validate --all` still works.

#### Step 14: deploy.sh Switchover

Apply the deploy.sh changes from Section 5.

Verify: Search for remaining `ndp-validate` references in deploy.sh dispatch code (comments and build references are acceptable).

#### Step 15: Integration Test

```bash
docker compose -f docker-compose.integration.yml up -d
cargo build -p ndp-cli
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json
```

Verify all phases complete, specifically validation phases.

---

## 10. Acceptance Criteria

### 10.1 ops-003-05: Validate Module in ndp-lib

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-05.1 | All 222 ndp-validate tests pass under `cargo test -p ndp-lib` | `cargo test -p ndp-lib -- validate::` reports 0 failures |
| AC-05.2 | `ValidationResult`, `BatchValidationResult` are public from `ndp_lib::validate` | Compile test: `use ndp_lib::validate::{ValidationResult, BatchValidationResult};` |
| AC-05.3 | `ValidationError`, `ErrorCode`, `Severity`, `ValidationLayer` are public | Compile test: `use ndp_lib::validate::{ValidationError, ErrorCode, Severity, ValidationLayer};` |
| AC-05.4 | `SchemaValidator`, `DomainSchemaValidator` are public | Compile test |
| AC-05.5 | `SemanticValidator` is public | Compile test |
| AC-05.6 | `validate_stream_file()` function exists and works | Unit test with real config file |
| AC-05.7 | `validate_all_streams()` function exists and works | Unit test with config directory |
| AC-05.8 | `validate_domain_file()` function exists and works | Unit test with real domain config |
| AC-05.9 | `validate_all_domains()` function exists and works | Unit test with domains directory |
| AC-05.10 | `generate_schema()` returns valid JSON Schema | Unit test |
| AC-05.11 | `verify_schema()` correctly detects drift | Unit test with modified schema |
| AC-05.12 | `OutputFormat::Json` and `OutputFormat::Human` produce identical output to standalone | Diff test |
| AC-05.13 | `determine_exit_code()` returns 0/1 correctly | Unit test with various result states |
| AC-05.14 | No `crate::` references remain pointing to ndp-validate internals | `grep -r "use crate::" crates/ndp-lib/src/validate/` only shows `crate::validate::` paths |
| AC-05.15 | `ndp-validate` standalone still builds and passes its tests when re-exporting from ndp-lib | `cargo build -p ndp-validate` succeeds |
| AC-05.16 | Single `is_valid_granularity()` implementation exists; `gold.rs` and `domain.rs` both call it | `grep -rn "fn is_valid_granularity" crates/ndp-lib/src/validate/semantic/` returns exactly 1 result |

### 10.2 ops-003-06: `ndp validate` Subcommands

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-06.1 | `ndp validate --all` produces identical JSON output to `ndp-validate --all` | `diff <(old) <(new)` is empty |
| AC-06.2 | `ndp validate --stream <path>` validates a single stream config | Manual test with air-quality config |
| AC-06.3 | `ndp validate --domain <path>` validates a single domain config | Manual test with indoor-air-quality domain |
| AC-06.4 | `ndp validate --domain-all` validates all domain configs | Manual test |
| AC-06.5 | `ndp validate --schema --generate` outputs JSON Schema to stdout | `ndp validate --schema --generate \| python3 -m json.tool` succeeds |
| AC-06.6 | `ndp validate --schema --generate --output FILE` writes schema to file | File exists and is valid JSON |
| AC-06.7 | `ndp validate --schema --verify PATH` exits 0 for matching schema | Exit code test |
| AC-06.8 | `ndp validate --format human` produces colored terminal output | Manual inspection |
| AC-06.9 | `ndp validate --format json` is the default | Output is valid JSON without `--format` flag |
| AC-06.10 | `ndp validate --strict` treats warnings as errors (exit 1) | Test with config that has warnings |
| AC-06.11 | `ndp validate --schema-only` skips semantic validation | Test: config with semantic errors but valid schema passes |
| AC-06.12 | `ndp validate --check-tables` requires `--db-url` | Error message if `--db-url` not set |
| AC-06.13 | Exit code 0 on success, 1 on validation error, 2 on system error | Test each case |
| AC-06.14 | Missing arguments produce helpful error message | `ndp validate` with no flags shows usage |
| AC-06.15 | `ndp validate --stream nonexistent.json` exits with code 2 | File not found is system error |

### 10.3 ops-003-07: deploy.sh Validate Switchover

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-07.1 | Zero `command -v ndp-validate` calls in deploy.sh dispatch code | `grep "command -v ndp-validate" deploy/pi/deploy.sh` returns only comments/build |
| AC-07.2 | Zero `validate_tool=` assignments in deploy.sh | `grep "validate_tool=" deploy/pi/deploy.sh` returns 0 lines |
| AC-07.3 | Site 3 (`validate_domain_configs`) uses `ndp validate` | Inspect function body |
| AC-07.4 | Site 4 (`handle_domain_declaration`) uses `ndp validate` | Inspect function body |
| AC-07.5 | Both sites use `error` + `return 1` for missing binary | No `warn` + `return 0` patterns |
| AC-07.6 | All 7 deploy.sh dispatch sites now use `ndp` | Sites 1-2 (gold, v1.1.14), Sites 3-4 (validate, v1.1.15), Sites 5-7 (dictionary/dimension/domain, already ndp) |
| AC-07.7 | `DEPLOY_ENV=integration ./deploy.sh apply` completes | Full integration test passes |
| AC-07.8 | deploy.sh `--config-dir` passes correct path to `ndp validate` | Validate receives base dir, derives streams dir internally |

### 10.4 Cross-Cutting Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-X.1 | `cargo test --workspace` passes | Zero failures across all crates |
| AC-X.2 | Binary size of `ndp` < 15MB | `ls -la target/release/ndp` |
| AC-X.3 | `cargo build -p ndp-validate` still succeeds | Standalone remains buildable |
| AC-X.4 | No new warnings in `cargo clippy -p ndp-lib` | Clean clippy |
| AC-X.5 | All `use` paths in validate module use `crate::validate::` prefix | Grep verification |

---

## Appendix A: Test Count Reconciliation

### A.1 Current ndp-validate Test Distribution (222 tests)

| Module | Unit Tests | Notes |
|--------|-----------|-------|
| `cli` | 65 | CLI parsing, result types, formatting, exit codes |
| `semantic` | 91 | Sources, source_path, dq_rules, gold, domain |
| `schema` | 47 | Schema validation (embedded schema, validation results) |
| `schema_gen` | 9 | Schema generation, verification, comparison |
| `error` | 5 | Error code mapping, severity, serialization |
| `semantic::domain` (integration) | 2 | Full domain validation with config discovery |
| `schema_gen` (integration) | 2 | Schema generation round-trip |
| `schema` (integration) | 1 | Full schema validation with file |
| **Total** | **222** | |

### A.2 Post-Migration Target Distribution

| ndp-lib Module | Tests | Source |
|----------------|-------|--------|
| `validate::error` | 5 | Direct copy |
| `validate::result` | 7 | Extracted from cli tests |
| `validate::format` | 19 | Extracted from cli tests (output + exit code) |
| `validate::schema` | 48 | Direct copy + 1 integration |
| `validate::schema_gen` | 11 | Direct copy + 2 integration |
| `validate::semantic` | 93 | Direct copy + 2 integration |
| `validate::options` | ~3 | New (Default trait, builder) |
| `validate` (convenience API) | ~6 | New (validate_stream_file, validate_all_streams, etc.) |
| **ndp-lib subtotal** | **~192** | |
| ndp-cli `commands/validate` | **~35** | New CLI parsing tests (replacing old Cli struct tests) |
| **Grand total** | **~227** | 222 original + ~5 new convenience/options tests |

The ~35 old `Cli` struct parsing tests must be REWRITTEN, not copied, because the argument structure changes (positional `CONFIG_PATH` becomes `--stream`, `--generate-schema` becomes `--schema --generate`, etc.). The rewritten tests verify the new `ValidateArgs` struct.

---

## Appendix B: Phase 1 Lessons Applied

These lessons from v1.1.14 directly inform Phase 2 decisions:

| # | Phase 1 Lesson | Phase 2 Application |
|---|---------------|---------------------|
| 1 | deploy.sh `--config-dir` path must match ndp CLI convention | Section 6: explicit path resolution strategy. deploy.sh passes `config/base`, validate handler appends `streams/`. |
| 2 | `--events requires --domain` guard was missing | Validate all flag combinations that should error: `--check-tables` without `--db-url`, `--stream` with `--all`, no args at all. |
| 3 | `--validate-only` flag was captured but not implemented | Test every `ValidateArgs` flag actually routes to library code. No `_` discards in match arms. |
| 4 | Golden master tests caught DDL regression | Create golden master tests for validate output: capture current `ndp-validate --all` JSON output as fixture, verify `ndp validate --all` matches. |
| 5 | psql not available in dev container | Table existence checking (`--check-tables`) requires `docker exec` for DB tests in integration environment. |
| 6 | Tracing to stdout broke parity | Library verbose output uses `tracing::info!()` to stderr (ndp CLI configures tracing to stderr). Validation JSON output goes to stdout only. |
| 7 | Missing convenience API | Define convenience functions (validate_stream_file, etc.) in mod.rs BEFORE CLI integration, not after. |
| 8 | FileSystemConfigLoader needed Clone | Check if any validate types need Clone for domain validation workflows. |

---

## Appendix C: Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Schema validator has embedded JSON (default_schema) | Medium | Medium | Move embedded schema string literals unchanged. Verify with schema validation tests. |
| `schemars` version conflict with ndp-types | Low | High | ndp-types already uses schemars 0.8. Workspace-level resolution handles it. |
| `OutputFormat` clap dependency in lib | None | — | Decision: plain enum in lib, clap wrapper in CLI (Section 8.1). |
| Path resolution mismatch in deploy.sh | Medium | High | Explicit path resolution table (Section 6.3). Integration test verifies. |
| CLI parsing test migration (35 tests rewritten) | Medium | Medium | New tests, not copied. Test the actual new arg structure. |
| ~~`serde_yaml` parse paths differ~~ | — | — | **ELIMINATED** (Amendment 1.3: YAML code paths stripped, serde_yaml not added) |
| Exit code 2 handling in ndp-cli main.rs | Medium | Medium | Validate handler calls `std::process::exit()` directly for system errors. Document in Section 7. |
| `--verbose` behavior change (eprintln -> tracing) | Low | Low | Deploy.sh does not parse verbose output. Only human-visible change. |

---

## Appendix D: Glossary

| Term | Definition |
|------|-----------|
| Layer 1 | JSON Schema validation (structural, type checking) |
| Layer 2 | Semantic validation (cross-field, business rules) |
| dp-019 | Config Validation Pipeline specification |
| FE-001 | Gold layer feature (introduced gold/domain semantic validators) |
| FE-002 | Domain Config Standardization (introduced domain validation in deploy.sh) |
| Thin wrapper | A binary whose main.rs delegates all logic to ndp-lib |
| No-fallback | deploy.sh fails with error if binary not found, never silently skips |

---

## Amendment 2026-02-08-b: Compliance Audit Corrections

**Date:** 2026-02-08
**Trigger:** Compliance audit found contradictions between SPECIFICATION, SCOPE.md, and CLI UX Design doc.

### Changes Applied

**D1 -- CLI Command Structure Consistency**

Verified flat-flag usage throughout. The SPECIFICATION already used flat flags for `--stream`, `--domain`, `--domain-all`, `--all`. No changes needed for those flags. Schema operations were the exception (see D4).

**D2 -- serde_yaml: DROP confirmed**

No changes needed. Amendment 1.3 already clearly eliminates serde_yaml from Section 1.3, the dependency table (Section 8.1), Cargo.toml additions (Section 8.2), and the risk table (Appendix C). The document is consistent.

**D3 -- `is_valid_granularity()` deduplication**

SCOPE.md (v1.1.15 task table) explicitly requires deduplication of `is_valid_granularity()`, which is duplicated at `semantic/gold.rs:430` and `semantic/domain.rs:403`. This was missing from the SPECIFICATION. Changes:

- Added FR-01a requirement under FR-01 specifying extraction to `validate/semantic/common.rs`
- Added `common.rs` to Section 3.1 (target layout) and Step 1 (module skeleton)
- Added deduplication instructions to Step 9 (migrate semantic validators)
- Added AC-05.16: Single `is_valid_granularity()` implementation; gold.rs and domain.rs both call it

**D4 -- Schema operations: flat flags instead of subcommands**

The authoritative CLI UX Design doc (`product/research/deployment/10-CLI-UX-DESIGN-REVISED.md`, lines 202-203) specifies `ndp validate --schema --generate` and `ndp validate --schema --verify <path>`. The SPECIFICATION previously used subcommand form (`ndp validate schema generate` / `ndp validate schema verify`). Changes:

- FR-02 flag mapping table: Updated 3 rows (lines 84-86) from subcommand to flat flags
- Design decision text: Rewritten to explain flat-flag rationale
- Section 4.1.2: Replaced `ValidateCommands`/`SchemaCommands` subcommand enums with `--schema`, `--generate`, `--verify`, `--output` flat flags in `ValidateArgs`
- Section 4.1.4: Added `--schema` requires `--generate` or `--verify` validation rule
- Step 12 (CLI parity testing): Updated diff commands
- AC-06.5/AC-06.6/AC-06.7: Updated to flat-flag syntax
- Appendix A.2 footnote: Updated `--generate-schema becomes --schema --generate`
