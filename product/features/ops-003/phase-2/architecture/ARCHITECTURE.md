# OPS-003 Phase 2 Architecture: Validate Migration (v1.1.15)

> **Author**: ndp-architect
> **Date**: 2026-02-07
> **Status**: Proposed
> **Scope**: ops-003-05 through ops-003-07
> **Predecessor**: Phase 1 (v1.1.14 Gold Migration) -- follow the same patterns

---

## ADR-003-002: Validate Library Extraction

### Status

Proposed

### Context

ndp-validate (13 source files, 9,897 lines, 217 tests) is a standalone binary providing two-layer config validation for NDP stream and domain configurations. It implements the dp-019 specification with JSON Schema validation (Layer 1) and semantic cross-field validation (Layer 2).

After Phase 1 (v1.1.14), the Gold DDL generation lives in `ndp_lib::gold` and deploy.sh calls `ndp gold` instead of `ndp-gold-ddl`. However, config validation still requires a separate `ndp-validate` binary. This creates two problems:

1. **deploy.sh fragmentation**: 5 of 7 dispatch sites use `ndp`, but 2 still use `ndp-validate` with its own 4-way binary resolution and graceful-skip fallback pattern.
2. **Cross-module isolation**: Gold generation cannot call validation before generating DDL. Phase 3 (v1.1.16) needs `gold::sync()` to call `validate::gold_config()`, which requires both modules in the same crate.

The validate module has a clear library/CLI split: the validation engine (schema validators, semantic validators, error types) is pure library code, while CLI concerns (clap args, output formatters, exit code determination) are presentation layer. This split must be preserved in the migration.

### Decision

Extract the validation engine from `tools/ndp-validate/src/` into `crates/ndp-lib/src/validate/`. CLI-specific types and formatters move to `tools/ndp-cli/src/commands/validate.rs`. The `ndp-validate` binary becomes a thin wrapper re-exporting from `ndp_lib::validate`.

Key boundary rule: **ndp-lib contains validation logic and result types. The CLI contains output formatting, arg parsing, and process exit codes.**

### Consequences

**Easier:**
- `ndp_lib::gold::sync()` can call `ndp_lib::validate::gold_config()` directly (v1.1.16).
- deploy.sh has a single binary resolution pattern for all 7 dispatch sites.
- Future MCP server imports validation from ndp-lib without depending on CLI concerns.
- Validation result types (`ValidationResult`, `BatchValidationResult`) are usable by any consumer, not coupled to terminal output.

**Harder:**
- `cli.rs` must be surgically split: result types and exit codes move to ndp-lib, output formatters stay in ndp-cli.
- `jsonschema`, `schemars`, `serde_yaml`, `strsim`, `regex`, and `sqlparser` become ndp-lib dependencies (increases compile-time footprint).
- 217 tests must be migrated with exact path updates.
- The `Cli` struct tests (clap parsing) stay in ndp-validate since they test the standalone binary interface.

### Alternatives Considered

**A1: Move everything including CLI types to ndp-lib (rejected)**

Moving `OutputFormat`, `output_human()`, and `output_json()` into ndp-lib would couple the library to terminal presentation concerns. Libraries should return structured data, not formatted strings with ANSI escape codes. The CLI layer formats results for its audience.

**A2: Keep validation in ndp-validate, add ndp-lib facade (rejected)**

Same reasons as Phase 1 rejection: prevents cross-module calls, does not eliminate duplication, creates circular dependency risk.

---

## 1. Module Layout

### 1.1 Target Directory Structure

```
crates/ndp-lib/src/
  lib.rs                          # Add: pub mod validate;
  validate/
    mod.rs                        # Public API: stream(), domain(), all(), gold_config(), schema_gen()
    error.rs                      # ValidationError, ErrorCode, Severity, ValidationLayer, SchemaValidatorError
    results.rs                    # ValidationResult, BatchValidationResult, ValidationSummary, exit_codes
    schema.rs                     # SchemaValidator, DomainSchemaValidator (jsonschema)
    schema_gen.rs                 # generate_schema, verify_schema, compare_schemas (schemars)
    semantic/
      mod.rs                      # SemanticValidator coordinator
      sources.rs                  # Source configuration validation
      source_path.rs              # Source path cross-reference validation
      dq_rules.rs                 # DQ rule syntax validation
      gold.rs                     # Gold ETL semantic validation
      domain.rs                   # Domain semantic validation
```

Total: 11 source files move from `tools/ndp-validate/src/` to `crates/ndp-lib/src/validate/`.

### 1.2 What Does NOT Move

Two files from ndp-validate are NOT migrated to ndp-lib:

| File | Lines | Reason |
|---|---|---|
| `cli.rs` (partial) | ~600 | Clap `Cli` struct, `OutputFormat` enum, `output_human()`, `output_human_batch()`, `output_json()`, `output_json_batch()`, `format_error_human()`, `format_warning_human()` are presentation-layer code. They stay in ndp-cli or ndp-validate. |
| `main.rs` | 385 | Binary entry point. Stays in ndp-validate as thin wrapper. |
| `semantic/table_exists.rs` | 236 | Stub implementation. Currently returns warnings unconditionally. Migrates as-is but is marked for DbClient integration in Phase 3. |

The `cli.rs` split is the critical design decision of this phase. See Section 3 for details.

### 1.3 Source-to-Destination File Mapping

| ndp-validate source | ndp-lib destination | Notes |
|---|---|---|
| `src/error.rs` | `src/validate/error.rs` | Moves as-is |
| `src/cli.rs` (result types) | `src/validate/results.rs` | Extract: `ValidationResult`, `BatchValidationResult`, `ValidationSummary`, `BatchSummary`, `exit_codes`, `determine_exit_code()`, `determine_batch_exit_code()` |
| `src/cli.rs` (CLI + formatters) | `tools/ndp-cli/src/commands/validate.rs` | Extract: `OutputFormat`, `output_human()`, `output_json()`, formatters |
| `src/schema.rs` | `src/validate/schema.rs` | Moves as-is |
| `src/schema_gen.rs` | `src/validate/schema_gen.rs` | Moves as-is |
| `src/semantic/mod.rs` | `src/validate/semantic/mod.rs` | Moves as-is |
| `src/semantic/sources.rs` | `src/validate/semantic/sources.rs` | Moves as-is |
| `src/semantic/source_path.rs` | `src/validate/semantic/source_path.rs` | Moves as-is |
| `src/semantic/dq_rules.rs` | `src/validate/semantic/dq_rules.rs` | Moves as-is |
| `src/semantic/gold.rs` | `src/validate/semantic/gold.rs` | Moves as-is |
| `src/semantic/domain.rs` | `src/validate/semantic/domain.rs` | Moves as-is |
| `src/semantic/table_exists.rs` | `src/validate/semantic/table_exists.rs` | Moves as-is (stub) |
| `src/lib.rs` | `src/validate/mod.rs` | Rewritten as public API |
| `src/main.rs` | stays in ndp-validate | Thin wrapper |

### 1.4 Module Hierarchy and Visibility

```rust
// crates/ndp-lib/src/lib.rs
pub mod validate;      // NEW in v1.1.15

// crates/ndp-lib/src/validate/mod.rs
pub mod error;         // ValidationError, ErrorCode, Severity, ValidationLayer
pub mod results;       // ValidationResult, BatchValidationResult, exit_codes
pub mod schema;        // SchemaValidator, DomainSchemaValidator
pub mod schema_gen;    // Schema generation and verification
pub mod semantic;      // SemanticValidator and all sub-validators

// Re-exports at validate/mod.rs level for convenience
pub use error::{ErrorCode, Severity, ValidationError, ValidationLayer, SchemaValidatorError};
pub use results::{
    exit_codes, determine_exit_code, determine_batch_exit_code,
    BatchSummary, BatchValidationResult, ValidationResult, ValidationSummary,
};
pub use schema::{DomainSchemaValidator, SchemaValidator};
pub use schema_gen::{compare_schemas, generate_schema, verify_schema, SchemaGenError};
pub use semantic::SemanticValidator;
pub use semantic::domain::validate_domain_semantic;
```

---

## 2. Dependency Analysis

### 2.1 ndp-validate Current Dependencies

| Dependency | Version | Workspace? | Moves to ndp-lib? | Rationale |
|---|---|---|---|---|
| `ndp-types` | workspace | Yes | Already in ndp-lib | Schema generation references ndp-types |
| `jsonschema` | `0.17` | No | YES | Core Layer 1 validation engine |
| `schemars` | `0.8` | No | YES | Schema generation from Rust types |
| `serde` | `1.0` (derive) | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `serde_json` | `1.0` | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `serde_yaml` | `0.9` | No | YES | YAML config parsing in validation |
| `thiserror` | `1.0` | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `sqlparser` | `0.50` (visitor) | No | YES | DQ rule SQL syntax validation |
| `regex` | `1.0` | No | YES | Pattern validation in DQ rules |
| `strsim` | `0.11` | No | YES | Levenshtein distance for suggestions |
| `tokio` | `1` (full) | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `clap` | `4` (derive, env) | Yes | NO | CLI-only; stays in ndp-cli |
| `tracing` | `0.1` | Yes (workspace) | Already in ndp-lib | Already a dependency |
| `tracing-subscriber` | `0.3` | Yes (workspace) | NO | Binary-only (logging init) |

**Dev-dependencies:**

| Dependency | Version | Moves to ndp-lib? | Rationale |
|---|---|---|---|
| `tempfile` | `3.8` | Already in ndp-lib as `3` | Used by validate tests |
| `tokio-test` | `0.4` | NO | Not used by migrated tests (validate tests are synchronous) |

### 2.2 New ndp-lib Runtime Dependencies (v1.1.15)

```toml
# crates/ndp-lib/Cargo.toml -- additions for v1.1.15

[dependencies]
# EXISTING: ndp-types, tokio-postgres, tokio, async-trait, serde, serde_json,
#           thiserror, tracing, chrono, csv

# NEW for validate module:
jsonschema = "0.17"
schemars = "0.8"
serde_yaml = "0.9"
sqlparser = { version = "0.50", features = ["visitor"] }
regex = "1"
strsim = "0.11"
```

### 2.3 Dependency Weight Assessment

| Dependency | Compile Impact | Binary Size Impact | Pi Memory Impact |
|---|---|---|---|
| `jsonschema` | Moderate (JSON Schema engine) | ~200KB | Negligible |
| `schemars` | Light (derive macros) | ~50KB | Negligible |
| `serde_yaml` | Light | ~80KB | Negligible |
| `sqlparser` | Moderate (SQL parser) | ~300KB | Negligible |
| `regex` | Light (already a transitive dep) | ~0 | Negligible |
| `strsim` | Trivial | ~5KB | Negligible |

Total estimated binary size increase: ~635KB. Combined with Phase 1, the `ndp` binary stays well under the 15MB target.

### 2.4 Feature Flags

No feature flags needed. Same rationale as Phase 1: the validate module has no heavy native dependencies and all capabilities should be available on the Pi.

---

## 3. What Migrates vs What Stays: The cli.rs Split

This is the most architecturally significant decision in Phase 2. The current `cli.rs` (1,370 lines) contains both library-grade types and CLI-specific presentation code. They must be separated.

### 3.1 Types That Move to ndp-lib (validate/results.rs)

These types are part of the validation engine's output contract. Any consumer (CLI, MCP server, test harness) needs them to interpret validation results.

```rust
// crates/ndp-lib/src/validate/results.rs

/// Exit codes per dp-019 specification
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const VALIDATION_ERROR: i32 = 1;
    pub const SYSTEM_ERROR: i32 = 2;
}

/// Summary of validation counts by layer
pub struct ValidationSummary { ... }

/// Complete validation result for a single config file
pub struct ValidationResult { ... }

/// Batch validation result for multiple configs
pub struct BatchValidationResult { ... }

/// Summary for batch validation
pub struct BatchSummary { ... }

/// Determine exit code from result and strict mode
pub fn determine_exit_code(result: &ValidationResult, strict: bool) -> i32 { ... }

/// Determine exit code for batch results
pub fn determine_batch_exit_code(results: &BatchValidationResult, strict: bool) -> i32 { ... }
```

**Why these move**: `ValidationResult` is the structured return type of every validation function. It is not a CLI concept -- it is the validation engine's output. Exit code constants are part of the dp-019 specification, not a CLI implementation detail. The `determine_exit_code()` functions encode business logic (strict mode semantics), not presentation.

### 3.2 Types That Stay in ndp-cli (commands/validate.rs)

These are terminal presentation concerns. They format `ValidationResult` for human or machine consumption via stdout.

```rust
// tools/ndp-cli/src/commands/validate.rs (or a validate/output.rs submodule)

/// Output format for validation results
pub enum OutputFormat { Json, Human }

/// Format a single validation result as JSON
pub fn output_json(result: &ValidationResult) -> String { ... }

/// Format batch validation results as JSON
pub fn output_json_batch(results: &BatchValidationResult) -> String { ... }

/// Format a single validation result for human-readable terminal output
pub fn output_human(result: &ValidationResult) -> String { ... }

/// Format batch validation results for human-readable output
pub fn output_human_batch(results: &BatchValidationResult) -> String { ... }

// Internal formatting helpers (private)
fn format_error_human(error: &ValidationError, indent: &str) -> String { ... }
fn format_warning_human(warning: &ValidationError, indent: &str) -> String { ... }
```

**Why these stay**: They contain ANSI escape codes, terminal formatting logic, and assume stdout output. A library should not embed terminal assumptions. The MCP server would format ValidationResult as JSON-RPC, not ANSI-colored text.

### 3.3 The Clap Struct

The `Cli` struct from ndp-validate does NOT move at all. ndp-cli defines its own clap structure (see Section 7). The old `Cli` struct stays in ndp-validate for the thin wrapper binary.

### 3.4 Test Split

| Test category | Lines (approx) | Destination |
|---|---|---|
| Clap parsing tests (`test_parse_*`, `test_cli_*`) | ~500 | Stay in ndp-validate (test the standalone binary's CLI) |
| ValidationResult tests (`test_validation_result_*`) | ~100 | Move to ndp-lib (test the library type) |
| Exit code tests (`test_exit_code_*`, `test_determine_*`) | ~80 | Move to ndp-lib (test the library function) |
| Output format tests (`test_output_json_*`, `test_output_human_*`) | ~100 | Move to ndp-cli (test the formatter) |
| Batch validation tests (`test_batch_*`) | ~30 | Move to ndp-lib (test the library type) |
| OutputFormat default test | ~5 | Move to ndp-cli |

---

## 4. Public API Surface

### 4.1 ndp_lib::validate Public API

The `validate/mod.rs` module provides high-level convenience functions matching the pattern established by `gold/mod.rs`:

```rust
// crates/ndp-lib/src/validate/mod.rs

/// Validate a single stream config file.
///
/// Runs Layer 1 (schema) and Layer 2 (semantic) validation.
/// Returns a ValidationResult with all errors and warnings.
pub fn validate_stream(
    config_path: &Path,
    schema_path: Option<&Path>,
    schema_only: bool,
) -> Result<ValidationResult, Box<dyn std::error::Error>> { ... }

/// Validate all stream configs in a directory.
///
/// Discovers config.json files under `config_dir/<stream_id>/config.json`.
pub fn validate_all_streams(
    config_dir: &Path,
    schema_path: Option<&Path>,
    schema_only: bool,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>> { ... }

/// Validate a single domain config file.
///
/// Runs Layer 1 (schema) and Layer 2 (semantic) domain validation.
pub fn validate_domain(
    domain_path: &Path,
    domain_schema_path: Option<&Path>,
    streams_dir: Option<&Path>,
    schema_only: bool,
) -> Result<ValidationResult, Box<dyn std::error::Error>> { ... }

/// Validate all domain configs in a directory.
///
/// Discovers domain.json files under `domains_dir/<domain_id>/domain.json`.
pub fn validate_all_domains(
    domains_dir: &Path,
    domain_schema_path: Option<&Path>,
    streams_dir: Option<&Path>,
    schema_only: bool,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>> { ... }

/// Validate a Gold ETL configuration (for cross-cutting validation in v1.1.16).
///
/// Called by `gold::sync()` before generating DDL.
/// Accepts a serde_json::Value to avoid coupling to Gold config types.
pub fn gold_config(config: &serde_json::Value) -> Vec<ValidationError> { ... }

/// Generate JSON Schema from ndp-types.
pub fn generate_schema() -> Result<String, SchemaGenError> { ... }

/// Verify committed schema matches generated schema.
pub fn verify_schema(schema_path: &Path) -> Result<bool, SchemaGenError> { ... }

/// Compare committed schema with generated schema, returning differences.
pub fn compare_schemas(schema_path: &Path) -> Result<Vec<String>, SchemaGenError> { ... }
```

### 4.2 API Design Rationale

**Paths not parsed structs**: Unlike Gold's `ConfigLoader` pattern, the validate API takes file paths. This is intentional. Validation must read files, parse JSON/YAML, and detect syntax errors. The parse step IS part of validation. Gold's `ConfigLoader` can assume valid config because it runs after validation.

**Optional schema path**: The schema path parameter is optional. When `None`, the embedded default schema is used. This allows both CLI usage (user specifies path) and library usage (embedded default).

**`gold_config()` takes `serde_json::Value`**: The cross-cutting validation function accepts raw JSON to avoid importing Gold config types. This prevents a dependency from `validate` back to `gold`. The Gold module parses config into its types and then passes the raw JSON to validation.

---

## 5. Import Path Migration

### 5.1 Internal Path Changes (within moved files)

Every file moving from `ndp_validate` to `ndp_lib::validate` needs its `use crate::` paths updated:

| Old path | New path |
|---|---|
| `use crate::error::*` | `use crate::validate::error::*` |
| `use crate::cli::*` | `use crate::validate::results::*` |
| `use crate::schema::*` | `use crate::validate::schema::*` |
| `use crate::schema_gen::*` | `use crate::validate::schema_gen::*` |
| `use crate::semantic::*` | `use crate::validate::semantic::*` |

### 5.2 External Consumer Path Changes

| Consumer | Old import | New import |
|---|---|---|
| ndp-validate main.rs | `use ndp_validate::cli::*` | `use ndp_lib::validate::*` |
| ndp-validate main.rs | `use ndp_validate::error::*` | `use ndp_lib::validate::*` |
| ndp-validate main.rs | `use ndp_validate::schema::*` | `use ndp_lib::validate::*` |
| ndp-validate main.rs | `use ndp_validate::schema_gen` | `use ndp_lib::validate::schema_gen` |
| ndp-validate main.rs | `use ndp_validate::semantic::*` | `use ndp_lib::validate::semantic::*` |
| ndp-cli commands/validate.rs | (new file) | `use ndp_lib::validate::*` |

### 5.3 Full Type Re-export Map

| Type | Old path (ndp_validate) | New path (ndp_lib) |
|---|---|---|
| `ValidationError` | `ndp_validate::error::ValidationError` | `ndp_lib::validate::ValidationError` |
| `ErrorCode` | `ndp_validate::error::ErrorCode` | `ndp_lib::validate::ErrorCode` |
| `Severity` | `ndp_validate::error::Severity` | `ndp_lib::validate::Severity` |
| `ValidationLayer` | `ndp_validate::error::ValidationLayer` | `ndp_lib::validate::ValidationLayer` |
| `SchemaValidatorError` | `ndp_validate::error::SchemaValidatorError` | `ndp_lib::validate::SchemaValidatorError` |
| `SchemaValidator` | `ndp_validate::schema::SchemaValidator` | `ndp_lib::validate::SchemaValidator` |
| `DomainSchemaValidator` | `ndp_validate::schema::DomainSchemaValidator` | `ndp_lib::validate::DomainSchemaValidator` |
| `ValidationResult` | `ndp_validate::cli::ValidationResult` | `ndp_lib::validate::ValidationResult` |
| `BatchValidationResult` | `ndp_validate::cli::BatchValidationResult` | `ndp_lib::validate::BatchValidationResult` |
| `ValidationSummary` | `ndp_validate::cli::ValidationSummary` | `ndp_lib::validate::ValidationSummary` |
| `exit_codes` | `ndp_validate::cli::exit_codes` | `ndp_lib::validate::exit_codes` |
| `determine_exit_code` | `ndp_validate::cli::determine_exit_code` | `ndp_lib::validate::determine_exit_code` |
| `determine_batch_exit_code` | `ndp_validate::cli::determine_batch_exit_code` | `ndp_lib::validate::determine_batch_exit_code` |
| `SchemaGenError` | `ndp_validate::schema_gen::SchemaGenError` | `ndp_lib::validate::SchemaGenError` |
| `generate_schema` | `ndp_validate::schema_gen::generate_schema` | `ndp_lib::validate::generate_schema` |
| `verify_schema` | `ndp_validate::schema_gen::verify_schema` | `ndp_lib::validate::verify_schema` |
| `compare_schemas` | `ndp_validate::schema_gen::compare_schemas` | `ndp_lib::validate::compare_schemas` |
| `SemanticValidator` | `ndp_validate::semantic::SemanticValidator` | `ndp_lib::validate::SemanticValidator` |
| `validate_domain_semantic` | `ndp_validate::semantic::domain::validate_domain_semantic` | `ndp_lib::validate::validate_domain_semantic` |
| `OutputFormat` | `ndp_validate::cli::OutputFormat` | (stays in ndp-cli) |
| `output_json` | `ndp_validate::cli::output_json` | (stays in ndp-cli) |
| `output_human` | `ndp_validate::cli::output_human` | (stays in ndp-cli) |
| `Cli` (clap struct) | `ndp_validate::cli::Cli` | (stays in ndp-validate) |

---

## 6. Error Handling Strategy

### 6.1 Validate Error Types Remain Separate

`ValidationError` and `ErrorCode` move to `ndp_lib::validate::error` and retain their full richness (40+ error codes across syntax, schema, and semantic layers). They are NOT merged with `NdpLibError`.

Rationale identical to Phase 1's `GoldDdlError` decision:
- `ValidationError` has domain-specific fields (`layer`, `code`, `path`, `severity`, `suggestion`, `context`) that are meaningless to dictionary or dimension sync.
- `ValidationError` is a *data structure* (serializable, not an error trait impl), while `NdpLibError` is an error enum.
- Merging would lose the structured dp-019-compliant error format.

### 6.2 Bridging for Public API

The convenience functions in `validate/mod.rs` return `Result<ValidationResult, Box<dyn std::error::Error>>` at the boundary, same as Gold's convenience functions. Internal validation errors are collected into `ValidationResult.errors` and `ValidationResult.warnings` -- they are NOT propagated as `Err`. Only system-level failures (file not found, schema load failure) return `Err`.

```rust
// Validation errors go INTO the result (not as Err):
let mut result = ValidationResult::new(config_path);
result.add_error(ValidationError::schema_error(...));

// System errors propagate as Err:
let content = std::fs::read_to_string(config_path)?; // Err if file not found
```

### 6.3 SchemaValidatorError

`SchemaValidatorError` is an internal error type for schema loading/compilation failures. It stays in `validate/error.rs` and is NOT confused with `ValidationError`. The distinction:

| Type | Purpose | Example |
|---|---|---|
| `ValidationError` | A validation finding about the config | "Required field 'stream_id' missing" |
| `SchemaValidatorError` | A failure of the validation infrastructure itself | "Failed to compile JSON Schema" |

---

## 7. CLI Command Design

### 7.1 Clap Structure: `ndp validate`

```rust
// tools/ndp-cli/src/commands/validate.rs

use clap::Args;
use std::path::PathBuf;

/// Config validation operations.
#[derive(Args)]
pub struct ValidateArgs {
    /// Config file path to validate (stream config).
    #[arg(value_name = "CONFIG_PATH", conflicts_with_all = ["all", "domain", "domain_all", "generate_schema", "verify_schema"])]
    pub config_path: Option<PathBuf>,

    /// Validate all stream configs in the config directory.
    #[arg(short, long, conflicts_with_all = ["config_path", "domain", "domain_all", "generate_schema", "verify_schema"])]
    pub all: bool,

    /// Validate a domain configuration file.
    #[arg(long, value_name = "FILE", conflicts_with_all = ["config_path", "all", "domain_all", "generate_schema", "verify_schema"])]
    pub domain: Option<PathBuf>,

    /// Validate all domain configs in config/domains/.
    #[arg(long, conflicts_with_all = ["config_path", "all", "domain", "generate_schema", "verify_schema"])]
    pub domain_all: bool,

    /// Generate JSON Schema from ndp-types to stdout.
    #[arg(long, conflicts_with_all = ["config_path", "all", "domain", "domain_all", "verify_schema"])]
    pub generate_schema: bool,

    /// Write generated schema to file (requires --generate-schema).
    #[arg(long, requires = "generate_schema")]
    pub output: Option<PathBuf>,

    /// Verify committed schema matches generated schema (for CI).
    #[arg(long, value_name = "PATH", conflicts_with_all = ["config_path", "all", "domain", "domain_all", "generate_schema"])]
    pub verify_schema: Option<PathBuf>,

    /// Skip semantic validation (Layer 2), only run schema validation.
    #[arg(long)]
    pub schema_only: bool,

    /// Output format.
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,

    /// Treat warnings as errors (exit code 1 if any warnings).
    #[arg(long)]
    pub strict: bool,

    /// Directory containing domain configs (for --domain-all).
    #[arg(long, default_value = "config/domains")]
    pub domains_dir: PathBuf,

    /// JSON Schema file path override for stream configs.
    #[arg(long)]
    pub schema_path: Option<PathBuf>,

    /// JSON Schema file path override for domain configs.
    #[arg(long)]
    pub domain_schema_path: Option<PathBuf>,
}
```

### 7.2 Integration into main.rs

```rust
// tools/ndp-cli/src/main.rs

// Add to Commands enum:
/// Config validation operations.
Validate(commands::validate::ValidateArgs),

// Add to match:
Commands::Validate(args) => {
    let exit_code = commands::validate::run(args, &config_dir).await;
    std::process::exit(exit_code);
}
```

Note: The validate command uses `process::exit()` with a specific exit code rather than `Result<(), Box<dyn Error>>`. This is because validation has a 3-way exit code convention (0/1/2) that must be preserved for deploy.sh compatibility, while the existing `main()` maps any `Err` to exit code 1.

### 7.3 Flag Mapping: Standalone to Subcommand

| Standalone (ndp-validate) | Subcommand (ndp validate) | Notes |
|---|---|---|
| `ndp-validate config.json` | `ndp validate config.json` | Positional arg preserved |
| `ndp-validate --all` | `ndp validate --all` | Identical |
| `ndp-validate -a` | `ndp validate -a` | Short flag preserved |
| `ndp-validate --domain FILE` | `ndp validate --domain FILE` | Identical |
| `ndp-validate --domain-all` | `ndp validate --domain-all` | Identical |
| `ndp-validate --schema-only FILE` | `ndp validate --schema-only FILE` | Identical |
| `ndp-validate --format human` | `ndp validate --format human` | Identical |
| `ndp-validate --strict` | `ndp validate --strict` | Identical |
| `ndp-validate --generate-schema` | `ndp validate --generate-schema` | Identical |
| `ndp-validate --generate-schema --output FILE` | `ndp validate --generate-schema --output FILE` | Identical |
| `ndp-validate --verify-schema FILE` | `ndp validate --verify-schema FILE` | Identical |
| `ndp-validate --config-dir DIR` | `ndp --config-dir DIR validate --all` | `--config-dir` is global in ndp-cli |
| `ndp-validate --check-tables --timescale-url URL` | `ndp --db-url URL validate --check-tables` | Deferred: table checking not yet wired to DbClient |
| `ndp-validate --verbose` | `RUST_LOG=debug` | Verbose via tracing env filter |

### 7.4 Config-Dir Resolution

The `--config-dir` flag in ndp-validate defaults to `config/base/streams`. In ndp-cli, `--config-dir` is a global flag that defaults to `config/base` (the parent, containing both `streams/` and `dimensions/`).

The validate command resolves this by appending `streams/` when discovering stream configs:

```rust
// In commands/validate.rs run():
let streams_dir = base_config_dir.join("streams");
let domains_dir = args.domains_dir.clone();

// For --all: discover streams under streams_dir/<id>/config.json
// For --domain-all: discover domains under domains_dir/<id>/domain.json
// For --domain FILE --config-dir DIR: pass DIR/streams as streams_dir to semantic validation
```

This matches the ndp-cli convention established in Phase 1 where `gold.rs` resolves `config_dir.parent()` for the Gold loader.

---

## 8. deploy.sh Dispatch Architecture

### 8.1 Site 3: `validate_domain_configs()` (line ~1533)

**BEFORE (current):**

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

# ...
"$validate_tool" --domain "$config_file" --format human
```

**AFTER (v1.1.15):**

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
"$ndp_tool" validate --domain "$config_file" --format human
```

**Key changes:**
- `validate_tool` variable eliminated. Uses `ndp_tool`.
- `ndp-validate` replaced with `ndp validate`.
- `warn` + `return 0` replaced with `error` + `return 1`. No fallback.
- Build instruction updated: `cargo build --release -p ndp-cli`.

### 8.2 Site 4: `handle_domain_declaration()` validate dispatch (line ~2032)

**BEFORE (current):**

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

**AFTER (v1.1.15):**

```bash
# ndp_tool already resolved earlier in this function (from v1.1.14 gold switchover at line ~2068)
# Use the same variable. If the function hasn't resolved it yet, resolve it:
if [ -z "$ndp_tool" ]; then
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
if ! "$ndp_tool" --config-dir "$CONFIG_STREAMS_DIR" validate --domain "$config_file" --format human; then
    error "Domain config validation failed: $config_file"
    return 1
fi
log "  Domain config validation passed"
```

**Key changes:**
- `validate_tool` eliminated. Reuses `ndp_tool` from the gold dispatch earlier in the same function.
- `--config-dir` is a global flag, placed before the `validate` subcommand.
- The `if [ -n "$validate_tool" ]` conditional eliminated: validation is now mandatory. If ndp is missing, the function already failed at the gold dispatch.
- `warn` + skip eliminated: no fallback.

### 8.3 ndp_tool Resolution: Extract to Helper

After v1.1.15, all 7 deploy.sh dispatch sites use the same ndp tool resolution pattern. To reduce duplication, this should be extracted into a shell function:

```bash
# Resolve the ndp binary path. Exits with error if not found.
# Sets the global NDP_TOOL variable.
resolve_ndp_tool() {
    if [ -n "$NDP_TOOL" ]; then
        return 0  # Already resolved
    fi

    if command -v ndp &> /dev/null; then
        NDP_TOOL="ndp"
    elif [ -x "/opt/ndp/bin/ndp" ]; then
        NDP_TOOL="/opt/ndp/bin/ndp"
    elif [ -x "$REPO_ROOT/target/release/ndp" ]; then
        NDP_TOOL="$REPO_ROOT/target/release/ndp"
    elif [ -x "$REPO_ROOT/target/debug/ndp" ]; then
        NDP_TOOL="$REPO_ROOT/target/debug/ndp"
    else
        error "ndp tool not found. Build with: cargo build --release -p ndp-cli"
        return 1
    fi
}
```

This is a cleanup step, not a structural change. The individual dispatch sites can be refactored to use `resolve_ndp_tool || return 1` in v1.1.15 or deferred to v1.1.16.

---

## 9. Schema File Resolution

### 9.1 Embedded Default Schemas

Both `SchemaValidator` and `DomainSchemaValidator` have `default_schema()` constructors that use embedded JSON schema values (compiled into the binary). This means:

- **No file system dependency for default validation.** The `ndp validate --all` command works without `schemas/stream-config.v1.1.schema.json` being present on disk.
- **Schema overrides via file path** are opt-in: `--schema-path` and `--domain-schema-path` load from disk when specified.

### 9.2 Schema Path Resolution Strategy

```
User specifies --schema-path?
  YES --> load from specified path
  NO  --> use SchemaValidator::default_schema() (embedded)

User specifies --domain-schema-path?
  YES --> load from specified path
  NO  --> use DomainSchemaValidator::default_schema() (embedded)
```

### 9.3 Schema Generation Output Path

The `--generate-schema --output FILE` and `--verify-schema FILE` commands work with absolute or relative paths. No config-dir resolution needed -- these are file operations, not config discovery.

---

## 10. Config Path Resolution

### 10.1 Stream Config Discovery

When `ndp validate --all` is invoked:

```
base_config_dir = cli.resolve_config_dir()  // e.g., config/base
streams_dir = base_config_dir / "streams"   // config/base/streams

For each dir in streams_dir:
  config_path = dir / "config.json"
  if config_path exists:
    validate_stream(config_path, ...)
```

### 10.2 Domain Config Discovery

When `ndp validate --domain-all` is invoked:

```
domains_dir = args.domains_dir            // default: config/domains
                                          // override: --domains-dir

For each dir in domains_dir:
  domain_path = dir / "domain.json"
  if domain_path exists:
    validate_domain(domain_path, streams_dir=base_config_dir/"streams", ...)
```

### 10.3 deploy.sh Config Path Conventions

deploy.sh passes explicit file paths to the validate command. It does NOT rely on the default config-dir:

```bash
# Site 3: validate_domain_configs()
"$ndp_tool" validate --domain "$config_file" --format human
# $config_file is an absolute path resolved by deploy.sh

# Site 4: handle_domain_declaration()
"$ndp_tool" --config-dir "$CONFIG_STREAMS_DIR" validate --domain "$config_file" --format human
# $CONFIG_STREAMS_DIR passed so domain semantic validation can resolve stream references
```

---

## 11. table_exists.rs: DbClient Integration Path

### 11.1 Current State

`table_exists.rs` accepts `pool: Option<()>` -- a stub parameter. When `None` (always, currently), it returns a warning. When `Some(())`, it returns empty results. No actual database query is implemented.

### 11.2 Phase 2 Decision: Move As-Is

The stub moves to `ndp_lib::validate::semantic::table_exists.rs` unchanged. Wiring it to `ndp_lib::DbClient` is deferred to Phase 3 (v1.1.16) when the `--check-tables` / `--db-url` integration is designed.

### 11.3 Phase 3 Integration Sketch

```rust
// Future (v1.1.16): table_exists.rs with real DbClient
pub async fn validate_table_exists(
    target_table: &str,
    db: Option<&(dyn DbClient + Send + Sync)>,
) -> Vec<ValidationError> {
    // ...
    if let Some(client) = db {
        let rows = client.query(
            "SELECT 1 FROM information_schema.tables WHERE table_schema = $1 AND table_name = $2",
            &[&schema, &table],
        ).await;
        // ...
    }
}
```

---

## 12. Migration Sequence

### Step 1: Prepare ndp-lib

1. Add `pub mod validate;` to `crates/ndp-lib/src/lib.rs`.
2. Add dependencies to `crates/ndp-lib/Cargo.toml`: `jsonschema`, `schemars`, `serde_yaml`, `sqlparser`, `regex`, `strsim`.
3. Create empty module structure under `crates/ndp-lib/src/validate/`.

### Step 2: Create results.rs from cli.rs Extract

1. Create `crates/ndp-lib/src/validate/results.rs`.
2. Extract from `tools/ndp-validate/src/cli.rs`: `exit_codes`, `ValidationSummary`, `ValidationResult`, `BatchSummary`, `BatchValidationResult`, `determine_exit_code()`, `determine_batch_exit_code()`.
3. Update imports to reference `crate::validate::error::*` instead of `crate::error::*`.

### Step 3: Move Source Files

```bash
# Create directories
mkdir -p crates/ndp-lib/src/validate/semantic

# Move error types
git mv tools/ndp-validate/src/error.rs crates/ndp-lib/src/validate/error.rs

# Move schema validator
git mv tools/ndp-validate/src/schema.rs crates/ndp-lib/src/validate/schema.rs

# Move schema generation
git mv tools/ndp-validate/src/schema_gen.rs crates/ndp-lib/src/validate/schema_gen.rs

# Move semantic validators
git mv tools/ndp-validate/src/semantic/mod.rs crates/ndp-lib/src/validate/semantic/mod.rs
git mv tools/ndp-validate/src/semantic/sources.rs crates/ndp-lib/src/validate/semantic/sources.rs
git mv tools/ndp-validate/src/semantic/source_path.rs crates/ndp-lib/src/validate/semantic/source_path.rs
git mv tools/ndp-validate/src/semantic/dq_rules.rs crates/ndp-lib/src/validate/semantic/dq_rules.rs
git mv tools/ndp-validate/src/semantic/gold.rs crates/ndp-lib/src/validate/semantic/gold.rs
git mv tools/ndp-validate/src/semantic/domain.rs crates/ndp-lib/src/validate/semantic/domain.rs
git mv tools/ndp-validate/src/semantic/table_exists.rs crates/ndp-lib/src/validate/semantic/table_exists.rs
```

### Step 4: Update `use` Paths

For each moved file, find and replace:
- `use crate::error` with `use crate::validate::error`
- `use crate::cli::{ValidationResult, ...}` with `use crate::validate::results::{ValidationResult, ...}`
- `use crate::schema` with `use crate::validate::schema`
- `use crate::schema_gen` with `use crate::validate::schema_gen`
- `use crate::semantic` with `use crate::validate::semantic`

### Step 5: Write validate/mod.rs

Create the public API module with convenience functions that orchestrate schema + semantic validation, following the patterns in main.rs.

### Step 6: Verify Tests

```bash
# All validate unit tests should pass
cargo test -p ndp-lib -- validate

# Full workspace build
cargo check --workspace
```

### Step 7: Add commands/validate.rs to ndp-cli

1. Create `tools/ndp-cli/src/commands/validate.rs` with:
   - Clap structure (Section 7.1)
   - `OutputFormat` enum and formatter functions (extracted from cli.rs)
   - `run()` function routing to `ndp_lib::validate::*`
2. Add `pub mod validate;` to `tools/ndp-cli/src/commands/mod.rs`.
3. Add `Validate` variant to `Commands` enum in `main.rs`.

### Step 8: Update ndp-validate to Thin Wrapper

1. Update `tools/ndp-validate/Cargo.toml`: add `ndp-lib`, keep `clap`.
2. Rewrite `tools/ndp-validate/src/lib.rs` to re-export from `ndp_lib::validate`.
3. Update `tools/ndp-validate/src/main.rs` imports from `ndp_validate::*` to `ndp_lib::validate::*`.
4. Keep `cli.rs` in ndp-validate for the `Cli` struct (standalone binary interface) and output formatters.

### Step 9: Verify Parity

```bash
# Build both binaries
cargo build -p ndp-cli -p ndp-validate

# Compare output for stream validation
diff <(target/debug/ndp-validate --all --config-dir config/base/streams --format json) \
     <(target/debug/ndp --config-dir config/base validate --all --format json)

# Compare output for domain validation
diff <(target/debug/ndp-validate --domain-all --format json) \
     <(target/debug/ndp validate --domain-all --format json)

# Compare exit codes
target/debug/ndp-validate --all --config-dir config/base/streams; echo "Exit: $?"
target/debug/ndp --config-dir config/base validate --all; echo "Exit: $?"
```

### Step 10: deploy.sh Switchover

Update 2 dispatch sites as specified in Section 8.

### Step 11: Integration Test

```bash
docker compose -f docker-compose.integration.yml up -d
cargo build -p ndp-cli
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json
```

Verify all 7 deploy.sh dispatch sites now use `ndp`:
- Sites 1-2: `ndp gold` (from v1.1.14)
- Sites 3-4: `ndp validate` (new in v1.1.15)
- Sites 5-7: `ndp dictionary/dimension/domain` (from ops-001/ops-002)

---

## 13. Cargo.toml Changes

### 13.1 crates/ndp-lib/Cargo.toml

```toml
[dependencies]
# EXISTING (no changes):
ndp-types = { path = "../ndp-types" }
tokio-postgres = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
csv = { workspace = true }

# NEW for validate module:
jsonschema = "0.17"
schemars = "0.8"
serde_yaml = "0.9"
sqlparser = { version = "0.50", features = ["visitor"] }
regex = "1"
strsim = "0.11"

[dev-dependencies]
# EXISTING (no changes):
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
tempfile = "3"
pretty_assertions = "1.4"
mockall = "0.11"
sha2 = "0.10"
```

### 13.2 tools/ndp-validate/Cargo.toml (After Thin Wrapper Conversion)

```toml
[package]
name = "ndp-validate"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ndp-validate"
path = "src/main.rs"

[dependencies]
# Retained: CLI and binary infrastructure
clap = { version = "4", features = ["derive", "env"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1.0"

# ADD: ndp-lib for all validation logic
ndp-lib = { path = "../../crates/ndp-lib" }

# REMOVE (moved to ndp-lib):
# ndp-types = { workspace = true }
# jsonschema = "0.17"
# schemars = "0.8"
# serde = { version = "1.0", features = ["derive"] }
# serde_yaml = "0.9"
# thiserror = "1.0"
# sqlparser = { version = "0.50", features = ["visitor"] }
# regex = "1.0"
# strsim = "0.11"
```

Note: `serde_json` is retained because the thin wrapper's `main.rs` may still format output. If it delegates all formatting to imported functions, this can also be removed.

### 13.3 tools/ndp-cli/Cargo.toml

```toml
# No Cargo.toml changes needed.
# ndp-cli already depends on ndp-lib, which now includes the validate module.
# The validate command module only imports from ndp_lib::validate::*.
# serde_json is already in ndp-cli's deps (used by existing commands).
```

---

## 14. Risks and Mitigations

### 14.1 Phase 1 Lessons Applied

| Phase 1 Lesson | Application to Phase 2 |
|---|---|
| Move files incrementally, compile between each submodule | Same approach: error.rs first, then schema.rs, then semantic/ |
| Integration tests catch path convention mismatches | Run `ndp validate --all` vs `ndp-validate --all` diff test |
| Config-dir convention differences between standalone and ndp-cli | Document in Section 10; handle in commands/validate.rs |
| Re-exports in mod.rs prevent consumer breakage | validate/mod.rs re-exports all public types |

### 14.2 Risk Registry

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| cli.rs split introduces bug in result type behavior | Medium | High | Extract tests with types. Run both ndp-validate and ndp validate against same configs. |
| jsonschema/schemars add transitive dependency conflicts | Low | Medium | Both are already pinned to specific versions. Run `cargo tree -d` to check for duplicates. |
| Embedded schema JSON strings break during move | Low | High | Schema is embedded via `include_str!` or inline JSON. Move the string literal with the file. Verify with `ndp validate --verify-schema`. |
| DQ rule validator's regex crate conflicts with existing transitive dep | Low | Low | `regex` is already a transitive dependency of multiple workspace crates. Adding it explicitly is safe. |
| deploy.sh exit code mismatch (ndp-validate exits 0/1/2, ndp-cli exits 0/1) | Medium | High | Validate command uses `process::exit()` with dp-019 exit codes, bypassing the `Result`-based exit. See Section 7.2. |
| `--config-dir` meaning differs between standalone and ndp-cli | Medium | Medium | Document in Section 10. Validate command appends `/streams` to the global config-dir. Test in integration env. |
| Clap test breakage (standalone Cli struct tests reference ndp_validate types) | Low | Low | Clap tests stay in ndp-validate; they test the standalone CLI interface which is preserved. |
| Binary size growth from new deps | Low | Low | Estimated ~635KB increase. Total stays under 15MB target. |
| Compile time increase from sqlparser + jsonschema | Medium | Low | Single build replaces 2 builds (ndp-lib + ndp-validate). Net neutral or improvement. |
| Phase 3 blocked if validate API surface is wrong | Medium | High | Design `gold_config()` API now even though it is not wired until v1.1.16. Ensure the function signature works for the cross-cutting validation use case. |

### 14.3 Incremental Compilation Order

Move files in dependency order to minimize cascading errors:

1. `error.rs` (no internal deps)
2. `results.rs` (depends on error)
3. `schema.rs` (depends on error)
4. `schema_gen.rs` (depends on ndp-types, schemars)
5. `semantic/sources.rs` (depends on error)
6. `semantic/source_path.rs` (depends on error)
7. `semantic/table_exists.rs` (depends on error)
8. `semantic/dq_rules.rs` (depends on error)
9. `semantic/gold.rs` (depends on error)
10. `semantic/domain.rs` (depends on error, may reference stream config paths)
11. `semantic/mod.rs` (depends on all semantic sub-validators)
12. `validate/mod.rs` (depends on everything above)

Run `cargo check -p ndp-lib` after each group (1-2, 3-4, 5-11, 12).

---

## 15. Summary of Deliverables

| Deliverable | Description |
|---|---|
| `crates/ndp-lib/src/validate/` | 11 source files (error, results, schema, schema_gen, semantic/*) |
| `crates/ndp-lib/src/lib.rs` | Add `pub mod validate;` |
| `crates/ndp-lib/Cargo.toml` | Add 6 runtime dependencies |
| `tools/ndp-cli/src/commands/validate.rs` | New file: validate CLI command + output formatters |
| `tools/ndp-cli/src/commands/mod.rs` | Add `pub mod validate;` |
| `tools/ndp-cli/src/main.rs` | Add `Validate` variant |
| `tools/ndp-validate/Cargo.toml` | Add ndp-lib, remove moved deps |
| `tools/ndp-validate/src/lib.rs` | Rewrite as re-export from ndp_lib::validate |
| `tools/ndp-validate/src/main.rs` | Update imports to use ndp_lib::validate |
| `deploy/pi/deploy.sh` | Update 2 dispatch sites (Sites 3 and 4) |
| All 217 validate tests passing | In `cargo test -p ndp-lib` |
| Validation output parity verified | `ndp validate --all` matches `ndp-validate --all` |
| All 7 deploy.sh dispatch sites use `ndp` | Zero references to `ndp-validate` or `ndp-gold-ddl` |
