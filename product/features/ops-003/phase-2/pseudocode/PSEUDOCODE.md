# OPS-003 Phase 2 (v1.1.15) Pseudocode: Validate Migration

> **Feature:** ops-003 Release 2 -- Config validation consolidated into ndp-lib and ndp CLI
> **Phase:** Pseudocode (SPARC P)
> **Date:** 2026-02-07
> **Scope:** 13 source files, 9,897 lines, 217 tests, 2 deploy.sh dispatch sites

---

## Table of Contents

1. [ndp_lib::validate Module Public API](#1-ndp_libvalidate-module-public-api)
2. [File Migration Pseudocode](#2-file-migration-pseudocode)
3. [CLI Command Pseudocode](#3-cli-command-pseudocode-commandsvalidaters)
4. [deploy.sh Dispatch Pseudocode](#4-deploysh-dispatch-pseudocode)
5. [ndp-validate Thin Wrapper](#5-ndp-validate-thin-wrapper)
6. [Test Migration Pseudocode](#6-test-migration-pseudocode)
7. [Cargo.toml Changes](#7-cargotoml-changes)
8. [Migration Script Pseudocode](#8-migration-script-pseudocode)
9. [Error Type Strategy](#9-error-type-strategy)
10. [Complexity Analysis](#10-complexity-analysis)

---

## 1. ndp_lib::validate Module Public API

### 1.1 Module Structure (`crates/ndp-lib/src/validate/`)

```
validate/
+-- mod.rs              Public API: validate_stream(), validate_all_streams(), etc.
+-- error.rs            ValidationError, ErrorCode, Severity, ValidationLayer
+-- result.rs           ValidationResult, BatchValidationResult (extracted from cli.rs)
+-- schema.rs           SchemaValidator, DomainSchemaValidator, default schemas
+-- schema_gen.rs       generate_schema(), verify_schema(), compare_schemas()
+-- semantic/
    +-- mod.rs          SemanticValidator coordinator
    +-- sources.rs      FR-020: Source config validation
    +-- source_path.rs  FR-022: Source path cross-reference
    +-- dq_rules.rs     DQ rule syntax, column refs, action validation
    +-- gold.rs         Gold ETL config semantic validation
    +-- domain.rs       Domain config semantic validation
    +-- table_exists.rs FR-023: Silver table existence check
```

### 1.2 Key Design: cli.rs Split

The current `cli.rs` (1,370 lines) contains two concerns:
1. **Library types** (ValidationResult, BatchValidationResult, ValidationSummary, exit_codes, OutputFormat) -- move to ndp-lib
2. **CLI output formatting** (output_json, output_human, format_error_human, etc.) -- stay in ndp-cli

Split strategy:

```
BEFORE (ndp-validate):
    cli.rs -> everything (types + formatting + Clap struct)

AFTER:
    crates/ndp-lib/src/validate/result.rs   -> ValidationResult, BatchValidationResult,
                                                ValidationSummary, BatchSummary,
                                                exit_codes, OutputFormat, ConfigType,
                                                determine_exit_code(), determine_batch_exit_code()

    tools/ndp-cli/src/commands/validate.rs  -> Clap structs, output_json(), output_human(),
                                                output_json_batch(), output_human_batch(),
                                                format_error_human(), format_warning_human()
```

### 1.3 Public API Signatures (`validate/mod.rs`)

```rust
//! Config validation for NDP stream and domain configurations.
//!
//! Migrated from tools/ndp-validate. Provides two-layer validation:
//! - Layer 1: JSON Schema structural validation
//! - Layer 2: Semantic cross-field validation
//!
//! ## Design
//!
//! All validation functions accept parsed data or paths. The CLI layer
//! handles filesystem discovery, output formatting, and exit codes.

pub mod error;
pub mod result;
pub mod schema;
pub mod schema_gen;
pub mod semantic;

// Re-export error types
pub use error::{ErrorCode, SchemaValidatorError, Severity, ValidationError, ValidationLayer};

// Re-export result types
pub use result::{
    exit_codes, BatchSummary, BatchValidationResult, ConfigType, OutputFormat,
    ValidationResult, ValidationSummary,
    determine_exit_code, determine_batch_exit_code,
};

// Re-export schema types
pub use schema::{
    default_domain_schema, default_stream_schema,
    DomainSchemaValidator, SchemaValidator,
};

// Re-export schema generation
pub use schema_gen::{
    compare_schemas, generate_schema, generate_type_schema,
    verify_schema, SchemaDifference, SchemaGenError, SchemaGenResult,
};

// Re-export semantic validators
pub use semantic::{
    validate_domain, validate_domain_semantic, validate_dq_rules,
    validate_gold_etl, validate_source_paths, validate_sources,
    validate_table_exists, parse_table_reference,
    SemanticValidator,
};

// ---------------------------------------------------------------------------
// Top-level convenience functions (NEW for v1.1.15)
// ---------------------------------------------------------------------------

/// Options controlling validation behavior.
///
/// Used by all top-level convenience functions.
pub struct ValidateOptions {
    /// Skip Layer 2 (semantic) validation; only run Layer 1 (schema).
    pub schema_only: bool,
    /// Treat warnings as errors.
    pub strict: bool,
    /// Output format (for exit code determination).
    pub format: OutputFormat,
    /// Check Silver table existence in TimescaleDB (requires db_url).
    pub check_tables: bool,
    /// TimescaleDB URL (required when check_tables is true).
    pub timescale_url: Option<String>,
    /// Path to custom JSON Schema for stream configs.
    pub schema_path: Option<std::path::PathBuf>,
    /// Path to custom JSON Schema for domain configs.
    pub domain_schema_path: Option<std::path::PathBuf>,
}

impl Default for ValidateOptions {
    fn default() -> Self {
        Self {
            schema_only: false,
            strict: false,
            format: OutputFormat::Json,
            check_tables: false,
            timescale_url: None,
            schema_path: None,
            domain_schema_path: None,
        }
    }
}

/// Validate a single stream config file.
///
/// Reads the file, parses JSON/YAML, runs Layer 1 (schema) and optionally
/// Layer 2 (semantic) validation.
///
/// ALGORITHM:
///   1. Read file content
///   2. Detect format (JSON vs YAML) by extension
///   3. Parse into serde_json::Value
///   4. Run SchemaValidator (Layer 1)
///   5. If !schema_only, run SemanticValidator (Layer 2)
///   6. Return ValidationResult
pub fn validate_stream(
    config_path: &std::path::Path,
    opts: &ValidateOptions,
) -> Result<ValidationResult, Box<dyn std::error::Error>>

/// Validate all stream configs in a directory.
///
/// Discovers all config.json files in subdirectories of config_dir,
/// validates each, returns BatchValidationResult.
///
/// ALGORITHM:
///   1. Read entries in config_dir
///   2. For each subdirectory, look for config.json
///   3. Validate each with validate_stream()
///   4. Collect into BatchValidationResult
pub fn validate_all_streams(
    config_dir: &std::path::Path,
    opts: &ValidateOptions,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>>

/// Validate a single domain config file.
///
/// ALGORITHM:
///   1. Read file content
///   2. Parse JSON
///   3. Run DomainSchemaValidator (Layer 1)
///   4. If !schema_only, run validate_domain_semantic() (Layer 2)
///   5. Return ValidationResult
pub fn validate_domain(
    domain_path: &std::path::Path,
    streams_dir: Option<&std::path::Path>,
    opts: &ValidateOptions,
) -> Result<ValidationResult, Box<dyn std::error::Error>>

/// Validate all domain configs in a directory.
///
/// ALGORITHM:
///   1. Read entries in domains_dir
///   2. For each subdirectory, look for domain.json
///   3. Validate each with validate_domain()
///   4. Collect into BatchValidationResult
pub fn validate_all_domains(
    domains_dir: &std::path::Path,
    streams_dir: Option<&std::path::Path>,
    opts: &ValidateOptions,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>>

/// Generate JSON Schema from ndp-types to stdout.
///
/// Delegates to schema_gen::generate_schema().
pub fn generate_schema_string() -> Result<String, SchemaGenError>

/// Verify committed schema matches generated schema.
///
/// Delegates to schema_gen::verify_schema().
pub fn verify_schema_file(
    schema_path: &std::path::Path,
) -> Result<bool, SchemaGenError>
```

### 1.4 Top-Level Function Pseudocode

```
ALGORITHM: validate_stream
INPUT: config_path (Path), opts (ValidateOptions)
OUTPUT: ValidationResult or error

BEGIN
    // Check file exists
    IF NOT config_path.exists() THEN
        RETURN error("Config file not found: {config_path}")
    END IF

    result <- ValidationResult::new(config_path.display())

    // Read and parse
    content <- fs::read_to_string(config_path)?

    // Detect format by extension
    value <- IF extension is "yaml" or "yml" THEN
        serde_yaml::from_str(content)
            .map_err(|e| result.add_error(syntax_error(e)))
    ELSE
        serde_json::from_str(content)
            .map_err(|e| result.add_error(syntax_error(e)))
    END IF

    IF value failed to parse THEN
        RETURN Ok(result)   // Already has syntax error
    END IF

    // Layer 1: Schema validation
    schema_validator <- IF opts.schema_path IS SOME THEN
        SchemaValidator::from_file(opts.schema_path)?
    ELSE
        SchemaValidator::default_schema()?
    END IF

    schema_errors <- schema_validator.validate_schema(&value)
    FOR EACH error IN schema_errors DO
        result.add_error(error)
    END FOR

    // Layer 2: Semantic validation (unless schema_only)
    IF NOT opts.schema_only THEN
        semantic_validator <- SemanticValidator::new()
        semantic_errors <- semantic_validator.validate(&value)
        FOR EACH error IN semantic_errors DO
            IF error.severity == Warning THEN
                result.add_warning(error)
            ELSE
                result.add_error(error)
            END IF
        END FOR
    END IF

    RETURN Ok(result)
END


ALGORITHM: validate_all_streams
INPUT: config_dir (Path), opts (ValidateOptions)
OUTPUT: BatchValidationResult or error

BEGIN
    IF NOT config_dir.exists() THEN
        RETURN error("Config directory not found: {config_dir}")
    END IF

    results <- []

    FOR EACH entry IN fs::read_dir(config_dir) DO
        IF entry.is_dir() THEN
            config_json <- entry.path().join("config.json")
            IF config_json.exists() THEN
                MATCH validate_stream(&config_json, opts):
                    Ok(result) => results.push(result)
                    Err(e) => {
                        error_result <- ValidationResult::new(config_json)
                        error_result.add_error(semantic_error(e))
                        results.push(error_result)
                    }
            END IF
        END IF
    END FOR

    IF results.is_empty() THEN
        RETURN error("No config.json files found in {config_dir}")
    END IF

    RETURN Ok(BatchValidationResult::from_results(results))
END


ALGORITHM: validate_domain
INPUT: domain_path (Path), streams_dir (Option<Path>), opts (ValidateOptions)
OUTPUT: ValidationResult or error

BEGIN
    IF NOT domain_path.exists() THEN
        RETURN error("Domain config file not found: {domain_path}")
    END IF

    result <- ValidationResult::new(domain_path.display())

    content <- fs::read_to_string(domain_path)?

    // Parse JSON (domain configs are always JSON)
    value <- serde_json::from_str(content)
        .map_err(|e| result.add_error(syntax_error(e)))?

    // Layer 1: Schema validation
    schema_validator <- IF opts.domain_schema_path IS SOME THEN
        DomainSchemaValidator::from_file(opts.domain_schema_path)?
    ELSE
        DomainSchemaValidator::default_schema()?
    END IF

    schema_errors <- schema_validator.validate_schema(&value)
    FOR EACH error IN schema_errors DO
        result.add_error(error)
    END FOR

    // Layer 2: Semantic validation (unless schema_only)
    IF NOT opts.schema_only THEN
        semantic_errors <- validate_domain_semantic(&value, streams_dir)
        FOR EACH error IN semantic_errors DO
            IF error.severity == Warning THEN
                result.add_warning(error)
            ELSE
                result.add_error(error)
            END IF
        END FOR
    END IF

    RETURN Ok(result)
END


ALGORITHM: validate_all_domains
INPUT: domains_dir (Path), streams_dir (Option<Path>), opts (ValidateOptions)
OUTPUT: BatchValidationResult or error

BEGIN
    IF NOT domains_dir.exists() THEN
        RETURN error("Domains directory not found: {domains_dir}")
    END IF

    results <- []

    FOR EACH entry IN fs::read_dir(domains_dir) DO
        IF entry.is_dir() THEN
            domain_json <- entry.path().join("domain.json")
            IF domain_json.exists() THEN
                MATCH validate_domain(&domain_json, streams_dir, opts):
                    Ok(result) => results.push(result)
                    Err(e) => {
                        error_result <- ValidationResult::new(domain_json)
                        error_result.add_error(semantic_error(e))
                        results.push(error_result)
                    }
            END IF
        END IF
    END FOR

    IF results.is_empty() THEN
        RETURN error("No domain.json files found in {domains_dir}")
    END IF

    RETURN Ok(BatchValidationResult::from_results(results))
END
```

---

## 2. File Migration Pseudocode

### 2.1 File Mapping Table

| Source (ndp-validate) | Destination (ndp-lib) | Import Changes |
|---|---|---|
| `src/error.rs` | `src/validate/error.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/cli.rs` (types only) | `src/validate/result.rs` | Extract types; `crate::error::` -> `super::error::` |
| `src/schema.rs` | `src/validate/schema.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/schema_gen.rs` | `src/validate/schema_gen.rs` | No `crate::` imports to change |
| `src/semantic/mod.rs` | `src/validate/semantic/mod.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/semantic/sources.rs` | `src/validate/semantic/sources.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/semantic/source_path.rs` | `src/validate/semantic/source_path.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/semantic/dq_rules.rs` | `src/validate/semantic/dq_rules.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/semantic/gold.rs` | `src/validate/semantic/gold.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/semantic/domain.rs` | `src/validate/semantic/domain.rs` | `crate::error::` -> `crate::validate::error::` |
| `src/semantic/table_exists.rs` | `src/validate/semantic/table_exists.rs` | `crate::error::` -> `crate::validate::error::` |

### 2.2 cli.rs Split Detail

```
ALGORITHM: SplitCliModule
INPUT: tools/ndp-validate/src/cli.rs (1,370 lines)
OUTPUT: Two files

FILE 1: crates/ndp-lib/src/validate/result.rs
    EXTRACT:
        - exit_codes module (lines 29-38)
        - OutputFormat enum (lines 45-52)
        - ConfigType enum (lines 55-62)
        - ValidationSummary struct (lines 222-227)
        - ValidationResult struct + impl (lines 230-287)
        - BatchSummary struct (lines 299-306)
        - BatchValidationResult struct + impl (lines 290-338)
        - determine_exit_code() function (lines 454-460)
        - determine_batch_exit_code() function (lines 463-469)

    IMPORT CHANGES:
        - `use crate::error::{ValidationError, ValidationLayer};`
          -> `use super::error::{ValidationError, ValidationLayer};`

    TESTS TO MOVE:
        - test_exit_code_0_on_success
        - test_exit_code_1_on_validation_error
        - test_exit_code_2_on_system_error
        - test_determine_exit_code_success
        - test_determine_exit_code_validation_error
        - test_determine_exit_code_strict_with_warnings
        - test_output_format_default
        - test_validation_result_new
        - test_validation_result_add_error
        - test_validation_result_add_warning
        - test_batch_validation_result

FILE 2: tools/ndp-cli/src/commands/validate.rs
    KEEP (as private helpers):
        - output_json() function
        - output_json_batch() function
        - output_human() function
        - output_human_batch() function
        - format_error_human() function
        - format_warning_human() function

    IMPORT FROM ndp_lib:
        - use ndp_lib::validate::{ValidationResult, BatchValidationResult, OutputFormat, ...};

    DO NOT MOVE:
        - Cli struct (clap) -- replaced by new ValidateArgs in commands/validate.rs
        - Cli::validate_args() -- argument validation moves into new Clap structs
        - Cli::is_schema_mode() -- replaced by match on subcommand
        - Cli::is_domain_mode() -- replaced by match on subcommand
        - All Cli-specific tests -- replaced by new Clap-based tests
```

### 2.3 Path Transformation Rule

```
ALGORITHM: TransformValidateUsePaths
INPUT: source_file (Rust source from ndp-validate)
OUTPUT: transformed source for ndp-lib location

RULE: Every `use crate::` that references ndp-validate-internal modules changes:
    crate::error::*     -> crate::validate::error::*
    crate::cli::*       -> crate::validate::result::*    (for types)
    crate::schema::*    -> crate::validate::schema::*
    crate::schema_gen::*-> crate::validate::schema_gen::*
    crate::semantic::*  -> crate::validate::semantic::*

RULE: Intra-module `super::` paths do NOT change.
    super::domain:: stays super::domain::

RULE: External crate imports do NOT change.
    use serde::{Deserialize, Serialize} stays the same
    use jsonschema::{Draft, JSONSchema} stays the same
    use ndp_types::{...} stays the same

RULE: Within validate submodules, prefer `super::` over `crate::validate::`:
    In semantic/gold.rs:
        OLD: use crate::error::{ErrorCode, ValidationError};
        NEW: use crate::validate::error::{ErrorCode, ValidationError};
        ALT: use super::super::error::{ErrorCode, ValidationError};
    Prefer the `crate::validate::` form for clarity.
```

### 2.4 Per-File Migration Detail

#### 2.4.1 error.rs

```
SOURCE: tools/ndp-validate/src/error.rs (432 lines)
DEST:   crates/ndp-lib/src/validate/error.rs

CHANGES:
    NONE. This file has no `use crate::` imports.
    All imports are from external crates (serde, thiserror, serde_json).
    Move as-is.

TYPES MOVED:
    ValidationLayer, Severity, ErrorCode (+ impls),
    ValidationError (+ impls), SchemaValidatorError

TESTS MOVED: 6 tests (all inline)
    test_error_code_layer_mapping
    test_error_code_default_severity
    test_syntax_error_creation
    test_schema_error_with_suggestion
    test_validation_error_serialization
```

#### 2.4.2 result.rs (extracted from cli.rs)

```
SOURCE: tools/ndp-validate/src/cli.rs (lines 24-469, selective)
DEST:   crates/ndp-lib/src/validate/result.rs

NEW FILE containing:
    - exit_codes module
    - OutputFormat enum (with ValueEnum derive for clap re-use)
    - ConfigType enum
    - ValidationSummary, ValidationResult, BatchSummary, BatchValidationResult
    - determine_exit_code(), determine_batch_exit_code()

IMPORT CHANGES:
    OLD: use crate::error::{ValidationError, ValidationLayer};
    NEW: use super::error::{ValidationError, ValidationLayer};

NOTE: OutputFormat needs `clap::ValueEnum` derive. This requires clap
as a dependency of ndp-lib. ALTERNATIVE: derive ValueEnum only in the
CLI layer, and in ndp-lib define OutputFormat without the clap derive.

PREFERRED APPROACH: ndp-lib defines OutputFormat as a plain enum with
Serialize/Default derives. The CLI layer uses a separate local enum
that maps to it, or uses `clap::ValueEnum` on a wrapper. This avoids
adding clap as a dependency to ndp-lib.

    // crates/ndp-lib/src/validate/result.rs
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
    pub enum OutputFormat {
        #[default]
        Json,
        Human,
    }

    // tools/ndp-cli/src/commands/validate.rs
    #[derive(Debug, Clone, Copy, ValueEnum)]
    pub enum CliOutputFormat {
        Json,
        Human,
    }

    impl From<CliOutputFormat> for ndp_lib::validate::OutputFormat {
        fn from(f: CliOutputFormat) -> Self {
            match f {
                CliOutputFormat::Json => ndp_lib::validate::OutputFormat::Json,
                CliOutputFormat::Human => ndp_lib::validate::OutputFormat::Human,
            }
        }
    }

TESTS MOVED: 11 tests
    (see Section 2.2 FILE 1 above)
```

#### 2.4.3 schema.rs

```
SOURCE: tools/ndp-validate/src/schema.rs (1,656 lines)
DEST:   crates/ndp-lib/src/validate/schema.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, SchemaValidatorError, Severity, ValidationError, ValidationLayer};
    NEW: use crate::validate::error::{ErrorCode, SchemaValidatorError, Severity, ValidationError, ValidationLayer};

TYPES MOVED:
    SchemaValidator, DomainSchemaValidator,
    default_stream_schema(), default_domain_schema(),
    format_json_path() (private)

TESTS MOVED: ~50 tests (all inline)
    All SchemaValidator tests (TC-SV-001 through TC-SV-012)
    All DomainSchemaValidator tests (TC-DSV-001 through TC-DSV-020)
```

#### 2.4.4 schema_gen.rs

```
SOURCE: tools/ndp-validate/src/schema_gen.rs (575 lines)
DEST:   crates/ndp-lib/src/validate/schema_gen.rs

IMPORT CHANGES:
    NONE. This file imports from ndp_types and schemars only.
    No `use crate::` references.
    Move as-is.

TYPES MOVED:
    SchemaGenError, SchemaGenResult, SchemaDifference,
    generate_schema(), generate_type_schema(), verify_schema(),
    compare_schemas(), normalize_schema(), sort_keys_recursive(),
    find_differences()

TESTS MOVED: 8 tests (all inline)
    test_generate_schema_produces_valid_json
    test_schema_includes_all_source_types
    test_schema_includes_all_dq_rule_types
    test_schema_includes_all_dq_actions
    test_verify_schema_returns_true_when_matching
    test_verify_schema_returns_false_when_drift
    test_compare_schemas_finds_differences
    test_generate_type_schema
    test_normalize_schema_removes_volatile_fields
```

#### 2.4.5 semantic/mod.rs

```
SOURCE: tools/ndp-validate/src/semantic/mod.rs (147 lines)
DEST:   crates/ndp-lib/src/validate/semantic/mod.rs

IMPORT CHANGES:
    OLD: use crate::error::ValidationError;
    NEW: use crate::validate::error::ValidationError;

TYPES MOVED:
    SemanticValidator struct + impl,
    re-exports of sub-module functions

TESTS: none (this file has no tests)
```

#### 2.4.6 semantic/sources.rs

```
SOURCE: tools/ndp-validate/src/semantic/sources.rs (602 lines)
DEST:   crates/ndp-lib/src/validate/semantic/sources.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, ValidationError};
    NEW: use crate::validate::error::{ErrorCode, ValidationError};

TESTS MOVED: ~20 tests (inline)
```

#### 2.4.7 semantic/source_path.rs

```
SOURCE: tools/ndp-validate/src/semantic/source_path.rs (624 lines)
DEST:   crates/ndp-lib/src/validate/semantic/source_path.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, ValidationError};
    NEW: use crate::validate::error::{ErrorCode, ValidationError};

NOTE: Uses `strsim` crate for Levenshtein distance. Ensure strsim
      is added to ndp-lib dependencies.

TESTS MOVED: ~15 tests (inline)
```

#### 2.4.8 semantic/dq_rules.rs

```
SOURCE: tools/ndp-validate/src/semantic/dq_rules.rs (1,999 lines)
DEST:   crates/ndp-lib/src/validate/semantic/dq_rules.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, ValidationError};
    NEW: use crate::validate::error::{ErrorCode, ValidationError};

NOTE: Uses `sqlparser`, `regex`, `strsim` crates. All must be in
      ndp-lib dependencies.

TESTS MOVED: ~50 tests (inline, largest test file)
```

#### 2.4.9 semantic/gold.rs

```
SOURCE: tools/ndp-validate/src/semantic/gold.rs (882 lines)
DEST:   crates/ndp-lib/src/validate/semantic/gold.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, Severity, ValidationError};
    NEW: use crate::validate::error::{ErrorCode, Severity, ValidationError};

NOTE: Contains hardcoded VALID_METRICS and VALID_ROLLING_STATS lists
      that duplicate the ones in gold::config::types. In v1.1.16 these
      will be unified into ndp_lib::constants. For v1.1.15, move as-is.

TESTS MOVED: ~30 tests (inline)
```

#### 2.4.10 semantic/domain.rs

```
SOURCE: tools/ndp-validate/src/semantic/domain.rs (926 lines)
DEST:   crates/ndp-lib/src/validate/semantic/domain.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, Severity, ValidationError};
    NEW: use crate::validate::error::{ErrorCode, Severity, ValidationError};

TESTS MOVED: ~25 tests (inline)
```

#### 2.4.11 semantic/table_exists.rs

```
SOURCE: tools/ndp-validate/src/semantic/table_exists.rs (236 lines)
DEST:   crates/ndp-lib/src/validate/semantic/table_exists.rs

IMPORT CHANGES:
    OLD: use crate::error::{ErrorCode, ValidationError};
    NEW: use crate::validate::error::{ErrorCode, ValidationError};

TESTS MOVED: ~8 tests (inline)
```

---

## 3. CLI Command Pseudocode (`commands/validate.rs`)

### 3.1 Clap Structs

```rust
// tools/ndp-cli/src/commands/validate.rs

use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Config validation operations.
#[derive(Args)]
pub struct ValidateArgs {
    #[command(subcommand)]
    pub command: ValidateCommands,
}

#[derive(Subcommand)]
pub enum ValidateCommands {
    /// Validate stream configuration(s).
    Stream {
        /// Path to a single stream config file.
        #[arg(value_name = "CONFIG_PATH", conflicts_with = "all")]
        config_path: Option<PathBuf>,

        /// Validate all stream configs in the config directory.
        #[arg(long)]
        all: bool,

        /// Skip semantic validation (Layer 2), only run schema validation.
        #[arg(long)]
        schema_only: bool,

        /// Check that Silver tables exist in TimescaleDB (requires --timescale-url).
        #[arg(long)]
        check_tables: bool,

        /// Output format.
        #[arg(long, value_enum, default_value = "json")]
        format: CliOutputFormat,

        /// Treat warnings as errors.
        #[arg(long)]
        strict: bool,

        /// TimescaleDB connection string (required for --check-tables).
        #[arg(long, env = "TIMESCALE_URL")]
        timescale_url: Option<String>,

        /// Show validation progress.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Validate domain configuration(s).
    Domain {
        /// Path to a single domain config file.
        #[arg(value_name = "DOMAIN_PATH", conflicts_with = "all")]
        domain_path: Option<PathBuf>,

        /// Validate all domain configs in the domains directory.
        #[arg(long)]
        all: bool,

        /// Skip semantic validation (Layer 2), only run schema validation.
        #[arg(long)]
        schema_only: bool,

        /// Output format.
        #[arg(long, value_enum, default_value = "json")]
        format: CliOutputFormat,

        /// Treat warnings as errors.
        #[arg(long)]
        strict: bool,

        /// Show validation progress.
        #[arg(short, long)]
        verbose: bool,

        /// Directory containing domain configs (for --all).
        #[arg(long, default_value = "config/domains")]
        domains_dir: PathBuf,
    },

    /// JSON Schema generation and verification.
    Schema {
        #[command(subcommand)]
        command: SchemaCommands,
    },
}

#[derive(Subcommand)]
pub enum SchemaCommands {
    /// Generate JSON Schema from ndp-types to stdout.
    Generate {
        /// Write schema to file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Verify committed schema matches generated schema (for CI).
    Verify {
        /// Path to committed schema file.
        path: PathBuf,
    },
}

/// CLI output format (maps to ndp_lib::validate::OutputFormat).
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliOutputFormat {
    Json,
    Human,
}
```

### 3.2 Execute Function

```
ALGORITHM: validate::run
INPUT:
    args (ValidateArgs) - parsed Clap arguments
    base_config_dir (Path) - resolved config base directory (from global --config-dir)
OUTPUT: Result<(), Box<dyn Error>>

BEGIN
    MATCH args.command:
        ValidateCommands::Stream { config_path, all, schema_only, check_tables,
                                   format, strict, timescale_url, verbose } =>
            opts <- ValidateOptions {
                schema_only,
                strict,
                format: format.into(),
                check_tables,
                timescale_url,
                schema_path: None,
                domain_schema_path: None,
            }

            IF all THEN
                result <- ndp_lib::validate::validate_all_streams(base_config_dir, &opts)?
                output_batch_result(&result, format, strict)
            ELSE IF config_path IS SOME THEN
                result <- ndp_lib::validate::validate_stream(config_path, &opts)?
                output_single_result(&result, format, strict)
            ELSE
                RETURN error("Must specify a config path or --all")
            END IF

        ValidateCommands::Domain { domain_path, all, schema_only, format,
                                    strict, verbose, domains_dir } =>
            opts <- ValidateOptions {
                schema_only,
                strict,
                format: format.into(),
                ..Default::default()
            }

            // Streams dir for semantic cross-reference validation
            streams_dir <- base_config_dir

            IF all THEN
                result <- ndp_lib::validate::validate_all_domains(
                    &domains_dir, Some(streams_dir), &opts
                )?
                output_batch_result(&result, format, strict)
            ELSE IF domain_path IS SOME THEN
                result <- ndp_lib::validate::validate_domain(
                    domain_path, Some(streams_dir), &opts
                )?
                output_single_result(&result, format, strict)
            ELSE
                RETURN error("Must specify a domain path or --all")
            END IF

        ValidateCommands::Schema { command } =>
            MATCH command:
                SchemaCommands::Generate { output } =>
                    run_generate_schema(output)
                SchemaCommands::Verify { path } =>
                    run_verify_schema(&path)
            END MATCH
    END MATCH
END


ALGORITHM: output_single_result
INPUT: result (ValidationResult), format (CliOutputFormat), strict (bool)
OUTPUT: prints to stdout, returns exit code handling

BEGIN
    MATCH format:
        Json => println(output_json(&result))
        Human => print(output_human(&result))
    END MATCH

    exit_code <- determine_exit_code(&result, strict)
    IF exit_code != 0 THEN
        std::process::exit(exit_code)
    END IF
END


ALGORITHM: output_batch_result
INPUT: result (BatchValidationResult), format (CliOutputFormat), strict (bool)
OUTPUT: prints to stdout, returns exit code handling

BEGIN
    MATCH format:
        Json => println(output_json_batch(&result))
        Human => print(output_human_batch(&result))
    END MATCH

    exit_code <- determine_batch_exit_code(&result, strict)
    IF exit_code != 0 THEN
        std::process::exit(exit_code)
    END IF
END


ALGORITHM: run_generate_schema
INPUT: output (Option<PathBuf>)
OUTPUT: prints schema to stdout or writes to file

BEGIN
    schema_json <- ndp_lib::validate::generate_schema_string()?

    IF output IS SOME THEN
        fs::write(output, &schema_json)?
        eprintln("Schema written to {output}")
    ELSE
        println(schema_json)
    END IF
END


ALGORITHM: run_verify_schema
INPUT: path (Path)
OUTPUT: exit code 0 if match, 1 if drift, 2 if error

BEGIN
    IF NOT path.exists() THEN
        eprintln("Schema file not found: {path}")
        std::process::exit(2)
    END IF

    MATCH ndp_lib::validate::verify_schema_file(path):
        Ok(true) =>
            eprintln("Schema verification PASSED")

        Ok(false) =>
            eprintln("Schema verification FAILED - drift detected!")

            // Show differences
            MATCH ndp_lib::validate::compare_schemas(path):
                Ok(diffs) =>
                    FOR EACH diff IN diffs DO
                        eprintln("  - {diff}")
                    END FOR
                Err(e) => eprintln("Could not compute differences: {e}")
            END MATCH

            eprintln("To fix: ndp validate schema generate --output {path}")
            std::process::exit(1)

        Err(e) =>
            eprintln("Error verifying schema: {e}")
            std::process::exit(2)
    END MATCH
END
```

### 3.3 Output Formatting Functions (stay in CLI layer)

```rust
// These functions move from ndp-validate::cli to ndp-cli::commands::validate
// They are presentation-only -- they use ANSI colors and are terminal-specific.
// They do NOT belong in the library.

/// Format a single validation result as JSON
fn output_json(result: &ValidationResult) -> String { /* same as current */ }

/// Format batch validation results as JSON
fn output_json_batch(results: &BatchValidationResult) -> String { /* same as current */ }

/// Format a single validation result for human-readable terminal output
fn output_human(result: &ValidationResult) -> String { /* same as current */ }

/// Format batch validation results for human-readable output
fn output_human_batch(results: &BatchValidationResult) -> String { /* same as current */ }

/// Format a single error for human-readable output (private)
fn format_error_human(error: &ValidationError, indent: &str) -> String { /* same as current */ }

/// Format a single warning for human-readable output (private)
fn format_warning_human(warning: &ValidationError, indent: &str) -> String { /* same as current */ }
```

### 3.4 Module Registration (`commands/mod.rs`)

```rust
// tools/ndp-cli/src/commands/mod.rs

pub mod dictionary;
pub mod dimension;
pub mod domain;
pub mod gold;
pub mod validate;   // NEW
```

### 3.5 Main.rs Integration

```rust
// tools/ndp-cli/src/main.rs changes:

#[derive(Subcommand)]
enum Commands {
    /// Data dictionary operations.
    Dictionary(commands::dictionary::DictionaryArgs),

    /// Dimension table operations.
    Dimension(commands::dimension::DimensionArgs),

    /// Domain configuration operations.
    Domain(commands::domain::DomainArgs),

    /// Gold layer DDL operations.
    Gold(commands::gold::GoldArgs),

    /// Config validation operations.                     // NEW
    Validate(commands::validate::ValidateArgs),           // NEW
}

// In main():
match cli.command {
    Commands::Dictionary(args) => { /* existing */ }
    Commands::Dimension(args) => { /* existing */ }
    Commands::Domain(args) => { /* existing */ }
    Commands::Gold(args) => { /* existing */ }
    Commands::Validate(args) => {                         // NEW
        commands::validate::run(args, &config_dir).await?;
    }
}
```

### 3.6 Global Flag Flow

```
DIAGRAM: Global Flag Flow from CLI to Library

User invocation:
    ndp validate stream --all --config-dir config/base/streams --format human --strict

Clap parsing (main.rs):
    cli.config_dir   = Some("config/base/streams")
    cli.command      = Commands::Validate(ValidateArgs {
        command: ValidateCommands::Stream {
            config_path: None,
            all: true,
            format: CliOutputFormat::Human,
            strict: true,
            ...
        }
    })

Resolution (main.rs):
    config_dir = cli.resolve_config_dir()   -> PathBuf("config/base/streams")

Dispatch to validate::run():
    args       = ValidateArgs (from Clap)
    config_dir = &PathBuf("config/base/streams")

Library call:
    opts = ValidateOptions { schema_only: false, strict: true, format: Human, ... }
    result = ndp_lib::validate::validate_all_streams(&config_dir, &opts)?

NOTE: --config-dir for validate points directly to the streams directory
      (e.g., config/base/streams), NOT the config root. This differs from
      gold which takes config/base and goes up one level.
      The ndp-validate standalone uses config/base/streams as default.
      The ndp CLI validate command follows the same convention.

NOTE: Validate does not require --db-url (except for --check-tables).
      The Validate command is NOT gated behind require_db_url() in main.rs.
```

---

## 4. deploy.sh Dispatch Pseudocode

### 4.1 Site 1: `validate_domain_configs()` (~line 1530)

```bash
ALGORITHM: validate_domain_configs() Switchover
PURPOSE: Replace ndp-validate with ndp validate domain

# BEFORE: 4-way lookup for ndp-validate binary, warn+return 0 on missing
# AFTER:  4-way lookup for ndp binary, error+return 1 on missing

PSEUDOCODE:

validate_domain_configs() {
    local manifest_file="$1"

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
        return 1   # FAIL LOUDLY -- no fallback (D5 from SCOPE)
    fi

    # Extract domain IDs from manifest
    local domain_ids=$(jq -r '
        [
            (.changes // [])[] | select(.type == "domain") | .domain_id,
            (.declarations.domains // [])[] | .domain_id
        ] | unique | .[]
    ' "$manifest_file" 2>/dev/null)

    local validation_failed=false

    for domain_id in $domain_ids; do
        if [ -z "$domain_id" ] || [ "$domain_id" = "null" ]; then
            continue
        fi

        # Find domain config file (same 4-way lookup as before)
        local config_file=""
        if [ -f "$CONFIG_DOMAINS_DIR/$domain_id/domain.json" ]; then
            config_file="$CONFIG_DOMAINS_DIR/$domain_id/domain.json"
        elif [ -f "$REPO_ROOT/config/domains/$domain_id/domain.json" ]; then
            config_file="$REPO_ROOT/config/domains/$domain_id/domain.json"
        elif [ -f "$CONFIG_DOMAINS_DIR/$domain_id/domain.yaml" ]; then
            config_file="$CONFIG_DOMAINS_DIR/$domain_id/domain.yaml"
        elif [ -f "$REPO_ROOT/config/domains/$domain_id/domain.yaml" ]; then
            config_file="$REPO_ROOT/config/domains/$domain_id/domain.yaml"
        fi

        if [ -z "$config_file" ]; then
            warn "  Domain config not found for: $domain_id"
            continue
        fi

        log "  Validating: $domain_id"
        # NEW: ndp validate domain <path> --format human
        if ! "$ndp_tool" validate domain "$config_file" --format human 2>&1 | \
            grep -v '^\[PASS\]'; then
            : # Validation passed
        else
            validation_failed=true
        fi
    done

    if [ "$validation_failed" = true ]; then
        return 1
    fi

    return 0
}

KEY DIFFERENCES FROM BEFORE:
    1. error + return 1 instead of warn + return 0 when ndp missing
    2. "ndp" binary instead of "ndp-validate"
    3. "$ndp_tool" validate domain "$config_file" --format human
       instead of "$validate_tool" --domain "$config_file" --format human
    4. ndp tool resolution uses same 4-way pattern as gold dispatch
```

### 4.2 Site 2: `handle_domain_declaration()` validate section (~line 2032)

```bash
ALGORITHM: handle_domain_declaration() Validate Section Switchover
PURPOSE: Replace ndp-validate with ndp validate domain

# BEFORE: Separate 4-way lookup for validate_tool, warn+skip on missing
# AFTER:  Reuse ndp_tool already resolved earlier in function (v1.1.14 gold dispatch)

PSEUDOCODE:

    # Phase B (FE-002): Validate domain config using ndp validate
    #
    # NOTE: ndp_tool is already resolved by the gold dispatch section above
    # (added in v1.1.14). If it was not resolved there, we need to resolve it.
    # After v1.1.14, ndp_tool is guaranteed to exist at this point because
    # the gold section already error+return 1 if ndp is missing.

    log "  Validating domain config..."
    if ! "$ndp_tool" validate domain "$config_file" \
        --config-dir "$CONFIG_STREAMS_DIR" --format human; then
        error "Domain config validation failed: $config_file"
        return 1
    fi
    log "  Domain config validation passed"

KEY DIFFERENCES FROM BEFORE:
    1. No separate validate_tool resolution -- reuse ndp_tool from gold section
    2. "$ndp_tool" validate domain instead of "$validate_tool" --domain
    3. No warn+skip path -- if ndp is missing, function already failed at gold dispatch
    4. --config-dir flag same value ($CONFIG_STREAMS_DIR) -- streams dir for cross-ref
```

### 4.3 Flag Mapping: Standalone to Subcommand

```
STANDALONE -> SUBCOMMAND (deploy.sh invocations):

validate_domain_configs() context:
    ndp-validate --domain "$config_file" --format human
    -> ndp validate domain "$config_file" --format human

handle_domain_declaration() context:
    ndp-validate --domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human
    -> ndp validate domain "$config_file" --config-dir "$CONFIG_STREAMS_DIR" --format human

FULL FLAG MAPPING:
    ndp-validate <path>                    -> ndp validate stream <path>
    ndp-validate --all                     -> ndp validate stream --all
    ndp-validate --domain <path>           -> ndp validate domain <path>
    ndp-validate --domain-all              -> ndp validate domain --all
    ndp-validate --generate-schema         -> ndp validate schema generate
    ndp-validate --verify-schema <path>    -> ndp validate schema verify <path>
    --schema-only                          -> --schema-only (same)
    --format json|human                    -> --format json|human (same)
    --strict                               -> --strict (same)
    --verbose / -v                         -> --verbose / -v (same)
    --config-dir                           -> --config-dir (global, same)
    --check-tables                         -> --check-tables (same)
    --timescale-url                        -> --timescale-url (same)
```

---

## 5. ndp-validate Thin Wrapper

After migration, `tools/ndp-validate/` becomes a thin wrapper that delegates to `ndp_lib::validate::*`.

### 5.1 lib.rs (re-exports)

```rust
// tools/ndp-validate/src/lib.rs (AFTER migration)
//
// All types and functions re-exported from ndp_lib::validate.
// This preserves backward compatibility for anyone importing ndp_validate::.

// Re-export everything from ndp_lib::validate
pub use ndp_lib::validate::error;
pub use ndp_lib::validate::result as cli;  // cli module name preserved
pub use ndp_lib::validate::schema;
pub use ndp_lib::validate::schema_gen;
pub use ndp_lib::validate::semantic;

// Re-export top-level types (same surface as current lib.rs)
pub use ndp_lib::validate::{
    // CLI types (from result.rs)
    exit_codes, BatchValidationResult, OutputFormat, ValidationResult, ValidationSummary,
    // Error types
    ErrorCode, Severity, ValidationError, ValidationLayer,
    // Schema types
    DomainSchemaValidator, SchemaValidator,
    // Schema generation
    compare_schemas, generate_schema, verify_schema, SchemaGenError,
    // Semantic domain validation
    validate_domain_semantic,
};
```

### 5.2 main.rs (delegates to library)

```rust
// tools/ndp-validate/src/main.rs (AFTER migration)
//
// Preserves existing CLI interface but delegates to ndp_lib::validate
// for all logic. Output formatting stays here for backward compat.

use clap::Parser;
use std::path::Path;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

// Import from ndp_lib instead of local modules
use ndp_lib::validate::{
    determine_batch_exit_code, determine_exit_code, exit_codes,
    BatchValidationResult, ValidationResult, ValidateOptions, OutputFormat,
    validate_stream, validate_domain, validate_all_domains,
    SchemaGenError,
};
use ndp_lib::validate::error::ValidationError;
use ndp_lib::validate::schema::DomainSchemaValidator;
use ndp_lib::validate::schema_gen;
use ndp_lib::validate::semantic::validate_domain_semantic;

// Clap struct stays IDENTICAL to current -- backward compatibility
// (same Cli struct from current cli.rs, but now just for parsing)

#[tokio::main]
async fn main() -> ExitCode {
    // Same implementation as current main.rs,
    // but calling ndp_lib::validate::* functions instead of local ones
    // ...
}
```

### 5.3 Cargo.toml Change for ndp-validate

```toml
# tools/ndp-validate/Cargo.toml CHANGES:

[dependencies]
# ADD:
ndp-lib = { path = "../../crates/ndp-lib" }

# KEEP (needed by main.rs for CLI parsing and output):
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
serde_json = "1.0"     # for output formatting in main.rs
serde_yaml = "0.9"     # for YAML parsing in main.rs

# REMOVE (now provided by ndp-lib):
# jsonschema, schemars, serde, thiserror, sqlparser, regex, strsim
# These are transitively available via ndp-lib.
# NOTE: Only remove after verifying ndp-lib re-exports everything.
# In v1.1.15, KEEP all deps to avoid breakage. Remove in v1.1.16.
ndp-types = { workspace = true }  # KEEP (used by main.rs for schema gen types)
```

---

## 6. Test Migration Pseudocode

### 6.1 Test Distribution

All 217 tests are inline `#[cfg(test)]` modules within source files. There are no integration test files in `tools/ndp-validate/tests/`.

| Source File | Approx Tests | Destination |
|---|---|---|
| `error.rs` | 5 | `validate/error.rs` (move with file) |
| `cli.rs` (result types) | 11 | `validate/result.rs` (move with types) |
| `cli.rs` (Clap/format) | ~50 | `commands/validate.rs` (rewrite for new Clap) |
| `schema.rs` | ~50 | `validate/schema.rs` (move with file) |
| `schema_gen.rs` | 9 | `validate/schema_gen.rs` (move with file) |
| `semantic/sources.rs` | ~20 | `validate/semantic/sources.rs` (move with file) |
| `semantic/source_path.rs` | ~15 | `validate/semantic/source_path.rs` (move with file) |
| `semantic/dq_rules.rs` | ~50 | `validate/semantic/dq_rules.rs` (move with file) |
| `semantic/gold.rs` | ~30 | `validate/semantic/gold.rs` (move with file) |
| `semantic/domain.rs` | ~25 | `validate/semantic/domain.rs` (move with file) |
| `semantic/table_exists.rs` | ~8 | `validate/semantic/table_exists.rs` (move with file) |

### 6.2 Import Path Changes in Tests

Tests that move with their source files need only the same `crate::` path updates:

```
RULE: In all #[cfg(test)] mod tests {} blocks:

    OLD: use crate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};
    NEW: use crate::validate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};

    OLD: use crate::cli::{ValidationResult, OutputFormat, ...};
    NEW: use crate::validate::result::{ValidationResult, OutputFormat, ...};

    OLD: use super::*;  (within same file)
    NEW: use super::*;  (UNCHANGED -- super still refers to parent module)
```

### 6.3 CLI Tests That Need Rewriting

The Clap-specific tests in `cli.rs` (approximately 50 tests) reference the `Cli` struct which is being replaced by `ValidateArgs` with a different structure (entity/verb vs flat flags). These tests must be **rewritten**, not moved.

```
TESTS TO REWRITE (new Clap structure):

    test_cli_structure_is_valid          -> test_validate_args_structure_is_valid
    test_parse_single_config_path        -> test_parse_stream_config_path
    test_parse_config_path_with_spaces   -> test_parse_stream_path_with_spaces
    test_parse_all_flag                  -> test_parse_stream_all_flag
    test_parse_all_flag_short            -> (no short flag in entity/verb)
    test_parse_schema_only_flag          -> test_parse_stream_schema_only
    test_parse_check_tables_flag         -> test_parse_stream_check_tables
    test_parse_format_json               -> test_parse_stream_format_json
    test_parse_format_human              -> test_parse_stream_format_human
    test_parse_strict_flag               -> test_parse_stream_strict
    test_parse_verbose_flag              -> test_parse_stream_verbose
    test_parse_generate_schema_flag      -> test_parse_schema_generate
    test_parse_verify_schema_flag        -> test_parse_schema_verify
    test_cli_accepts_domain_flag         -> test_parse_domain_config_path
    test_cli_accepts_domain_all_flag     -> test_parse_domain_all

TESTS THAT BECOME IRRELEVANT (conflicts resolved by subcommand):
    test_all_and_config_path_conflict    -> inherent in subcommand structure
    test_generate_schema_conflicts_with_config_path -> different subcommands
    test_generate_schema_conflicts_with_verify_schema -> different subcommands
    test_cli_domain_conflicts_with_all   -> different subcommands
```

### 6.4 Verification Gates

```
GATE 1: After moving error.rs, result.rs, schema.rs, schema_gen.rs
    cargo test -p ndp-lib -- validate   # types compile and pass

GATE 2: After moving all semantic/ files
    cargo test -p ndp-lib -- validate   # all 217 migrated tests pass

GATE 3: After wiring ndp-validate thin wrapper
    cargo test -p ndp-validate          # standalone still works

GATE 4: After adding commands/validate.rs
    cargo test -p ndp-cli               # CLI compiles and new tests pass
```

---

## 7. Cargo.toml Changes

### 7.1 crates/ndp-lib/Cargo.toml

```toml
[dependencies]
# EXISTING (already present):
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

# NEW for v1.1.15 (validate module dependencies):
jsonschema = "0.17"       # Layer 1 schema validation
schemars = "0.8"          # Schema generation from Rust types
serde_yaml = "0.9"        # YAML config parsing
sqlparser = { version = "0.50", features = ["visitor"] }  # DQ rule SQL validation
regex = "1"               # Pattern validation in semantic layer
strsim = "0.11"           # Levenshtein distance for typo suggestions

[dev-dependencies]
# EXISTING:
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
tempfile = "3"
pretty_assertions = "1.4"
mockall = "0.11"
sha2 = "0.10"

# NEW for v1.1.15:
# (none -- tempfile already present for schema_gen tests)
```

### 7.2 Feature Flag Design

Consider a `validate` feature flag to keep validation-specific dependencies optional:

```toml
[features]
default = ["validate"]
validate = ["dep:jsonschema", "dep:schemars", "dep:serde_yaml",
            "dep:sqlparser", "dep:regex", "dep:strsim"]

[dependencies]
jsonschema = { version = "0.17", optional = true }
schemars = { version = "0.8", optional = true }
serde_yaml = { version = "0.9", optional = true }
sqlparser = { version = "0.50", features = ["visitor"], optional = true }
regex = { version = "1", optional = true }
strsim = { version = "0.11", optional = true }
```

**Decision: Feature flag is OPTIONAL for v1.1.15.** All current consumers (ndp-cli, ndp-validate) need validation. The feature flag is a nice-to-have for hypothetical consumers that only need gold. Implement if easy, defer if it complicates the migration.

### 7.3 tools/ndp-validate/Cargo.toml

```toml
[dependencies]
# ADD:
ndp-lib = { path = "../../crates/ndp-lib" }

# KEEP all existing dependencies in v1.1.15 for safety.
# Remove in v1.1.16 cleanup after verifying re-exports work.
```

### 7.4 tools/ndp-cli/Cargo.toml

```toml
# No Cargo.toml changes needed for ndp-cli.
# ndp-cli already depends on ndp-lib.
# Validate commands use ndp_lib::validate::* which is in ndp-lib.
#
# HOWEVER: ndp-cli needs serde_json for output formatting.
# Verify serde_json is already in ndp-cli's dependencies.
# If not, add it.
```

---

## 8. Migration Script Pseudocode

### Step-by-step migration procedure with verification gates.

```
ALGORITHM: Validate Module Migration
INPUT: current codebase at v1.1.14
OUTPUT: codebase at v1.1.15 with validate module in ndp-lib

PRE-FLIGHT:
    cargo test -p ndp-validate    # Record baseline: 217 tests passing
    cargo test -p ndp-lib         # Record baseline: existing tests passing
    cargo test -p ndp-cli         # Record baseline: existing tests passing

STEP 1: Create module structure in ndp-lib
    mkdir -p crates/ndp-lib/src/validate/semantic

STEP 2: Add validate dependencies to ndp-lib Cargo.toml
    # Add jsonschema, schemars, serde_yaml, sqlparser, regex, strsim
    # (See Section 7.1)

    VERIFICATION GATE:
        cargo check -p ndp-lib    # Dependencies resolve

STEP 3: Copy source files (NOT git mv yet -- keep originals for comparison)
    # Error types (1 file)
    cp tools/ndp-validate/src/error.rs        crates/ndp-lib/src/validate/error.rs

    # Schema validators (2 files)
    cp tools/ndp-validate/src/schema.rs       crates/ndp-lib/src/validate/schema.rs
    cp tools/ndp-validate/src/schema_gen.rs   crates/ndp-lib/src/validate/schema_gen.rs

    # Semantic validators (7 files)
    cp tools/ndp-validate/src/semantic/mod.rs         crates/ndp-lib/src/validate/semantic/mod.rs
    cp tools/ndp-validate/src/semantic/sources.rs     crates/ndp-lib/src/validate/semantic/sources.rs
    cp tools/ndp-validate/src/semantic/source_path.rs crates/ndp-lib/src/validate/semantic/source_path.rs
    cp tools/ndp-validate/src/semantic/dq_rules.rs    crates/ndp-lib/src/validate/semantic/dq_rules.rs
    cp tools/ndp-validate/src/semantic/gold.rs        crates/ndp-lib/src/validate/semantic/gold.rs
    cp tools/ndp-validate/src/semantic/domain.rs      crates/ndp-lib/src/validate/semantic/domain.rs
    cp tools/ndp-validate/src/semantic/table_exists.rs crates/ndp-lib/src/validate/semantic/table_exists.rs

    # Total: 10 files copied

STEP 4: Create result.rs by extracting from cli.rs
    # Extract types and functions from cli.rs into new result.rs
    # (See Section 2.2 for exact extraction list)

STEP 5: Update use paths in copied files
    # Systematic find-and-replace in crates/ndp-lib/src/validate/**/*.rs:
    #
    # Pattern: use crate::error::   -> use crate::validate::error::
    # Pattern: use crate::cli::     -> use crate::validate::result::
    # Pattern: use crate::schema::  -> use crate::validate::schema::
    # Pattern: use crate::semantic:: -> use crate::validate::semantic::
    #
    # EXCEPTION: schema_gen.rs has no crate:: imports
    # EXCEPTION: error.rs has no crate:: imports
    # EXCEPTION: result.rs uses super::error:: (within validate module)

STEP 6: Create validate/mod.rs with top-level convenience functions
    # Write the mod.rs with validate_stream(), validate_all_streams(), etc.
    # (See Section 1.3 above)

STEP 7: Wire validate module into ndp-lib
    # crates/ndp-lib/src/lib.rs -- add:
    pub mod validate;

    VERIFICATION GATE:
        cargo test -p ndp-lib           # All existing + new validate tests pass
        cargo test -p ndp-lib -- validate  # Focus on validate module tests

STEP 8: Update ndp-validate to depend on ndp-lib
    # tools/ndp-validate/Cargo.toml:
    #   Add: ndp-lib = { path = "../../crates/ndp-lib" }
    #
    # tools/ndp-validate/src/lib.rs:
    #   Re-export from ndp_lib::validate instead of local modules
    #
    # tools/ndp-validate/src/main.rs:
    #   Update imports to use ndp_lib::validate::* (thin wrapper)

    VERIFICATION GATE:
        cargo test -p ndp-validate       # Standalone binary still works
        # Compare output:
        diff <(cargo run -p ndp-validate -- --all --config-dir config/base/streams --format json) \
             <(echo "baseline output from before migration")

STEP 9: Add commands/validate.rs to ndp-cli
    # tools/ndp-cli/src/commands/validate.rs  (new file, see Section 3)
    # tools/ndp-cli/src/commands/mod.rs       (add: pub mod validate;)
    # tools/ndp-cli/src/main.rs               (add Validate variant to Commands enum)

    VERIFICATION GATE:
        cargo build -p ndp-cli
        # Test parity for stream validation:
        diff <(cargo run -p ndp-validate -- --all --config-dir config/base/streams --format json) \
             <(cargo run -p ndp-cli -- validate stream --all --config-dir config/base/streams --format json)

        # Test parity for domain validation:
        diff <(cargo run -p ndp-validate -- --domain config/domains/indoor-air-quality/domain.json --format json) \
             <(cargo run -p ndp-cli -- validate domain config/domains/indoor-air-quality/domain.json --format json)

        # Test schema generation parity:
        diff <(cargo run -p ndp-validate -- --generate-schema) \
             <(cargo run -p ndp-cli -- validate schema generate)

STEP 10: Update deploy.sh
    # Site 1: validate_domain_configs()         -- switch to ndp validate domain
    # Site 2: handle_domain_declaration()       -- switch to ndp validate domain
    # (See Section 4 for exact pseudocode)

STEP 11: Integration test
    docker compose -f docker-compose.integration.yml up -d
    cargo build -p ndp-cli --release
    DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/v1.1.15.manifest.json
    # Verify: All phases complete without error
    # Specifically: domain validation phases pass via ndp binary

    VERIFICATION GATE:
        All phases pass. deploy.sh exit code 0.

STEP 12: Final verification
    cargo test --workspace            # ALL workspace tests pass
    cargo test -p ndp-lib -- validate # validate tests in ndp-lib
    cargo test -p ndp-validate        # Standalone tests still pass
    cargo test -p ndp-cli             # CLI tests pass
    # Verify no ndp-validate references remain in deploy.sh dispatch sites:
    grep -n 'ndp-validate' deploy/pi/deploy.sh
    # Should only appear in comments, tool build handler, and package names
```

---

## 9. Error Type Strategy

### 9.1 Error Types in ndp-lib

ndp-validate defines two error hierarchies:
1. `ValidationError` -- individual validation findings (Serialize, structured)
2. `SchemaValidatorError` -- internal errors loading/compiling schemas (thiserror)
3. `SchemaGenError` -- internal errors generating/comparing schemas (thiserror)

These all move to `ndp_lib::validate::error` and `ndp_lib::validate::schema_gen`.

### 9.2 Relationship to Existing ndp-lib Errors

```
RELATIONSHIP:
    ndp_lib::error::NdpLibError          -- library-wide errors (Database, ConfigNotFound)
    ndp_lib::gold::error::GoldDdlError   -- gold-specific errors (InvalidMetric, etc.)
    ndp_lib::validate::error::ValidationError  -- validation findings (structured, serializable)
    ndp_lib::validate::error::SchemaValidatorError -- schema load/compile failures

CONVERSION:
    The top-level functions (validate_stream, validate_domain) return
    Result<ValidationResult, Box<dyn Error>>. ValidationResult contains
    Vec<ValidationError> for findings. SchemaValidatorError and IO errors
    are propagated as Box<dyn Error>.

    In v1.1.16, cross-cutting validation will need:
    GoldDdlError -> ValidationError conversion (or gold::sync calls validate directly).
    For v1.1.15, they remain independent.

NOTE: ValidationError is NOT a Rust error type (does not impl std::error::Error).
It is a structured finding with Serialize. This is correct -- validation
"errors" are expected results, not exceptional conditions.
```

### 9.3 Error Flow

```
DIAGRAM: Error Flow for ndp validate stream <path>

    ndp validate stream config.json
        |
        v
    commands/validate.rs::run()
        | returns Result<(), Box<dyn Error>>
        |
        v
    ndp_lib::validate::validate_stream()
        | returns Result<ValidationResult, Box<dyn Error>>
        |
        +---> SchemaValidator::new()
        |     | may return SchemaValidatorError (Box<dyn Error>)
        |
        +---> SchemaValidator::validate_schema()
        |     | returns Vec<ValidationError> (findings, not Rust errors)
        |
        +---> SemanticValidator::validate()
              | returns Vec<ValidationError> (findings, not Rust errors)

    At CLI boundary:
        ValidationResult is printed (JSON or human format)
        Exit code determined by determine_exit_code()
        Box<dyn Error> from schema load failures -> eprintln + exit(2)
```

---

## 10. Complexity Analysis

### 10.1 Migration Complexity

```
ANALYSIS: Migration File Counts

Source files to copy/create:     12
    validate/error.rs         1 file  (copy)
    validate/result.rs        1 file  (EXTRACT from cli.rs)
    validate/schema.rs        1 file  (copy)
    validate/schema_gen.rs    1 file  (copy)
    validate/semantic/mod.rs  1 file  (copy)
    validate/semantic/        6 files (copy: sources, source_path, dq_rules, gold, domain, table_exists)
    validate/mod.rs           1 file  (NEW -- top-level API)

New files to create:           2
    crates/ndp-lib/src/validate/mod.rs        (top-level API)
    tools/ndp-cli/src/commands/validate.rs    (CLI command)

Files to modify:               5
    crates/ndp-lib/src/lib.rs              (add pub mod validate)
    crates/ndp-lib/Cargo.toml              (add 6 dependencies)
    tools/ndp-cli/src/commands/mod.rs      (add pub mod validate)
    tools/ndp-cli/src/main.rs              (add Validate variant)
    deploy/pi/deploy.sh                    (2 dispatch sites)

Tests:
    Unit tests to migrate:  ~167 (inline in source files, move with code)
    CLI tests to rewrite:   ~50  (Clap struct changed, new tests needed)
    New CLI parity tests:   ~10  (validate stream/domain parity with standalone)
```

### 10.2 Line Count Breakdown

```
ANALYSIS: Source Lines per File

error.rs:              432 lines (move as-is)
result.rs (from cli):  ~250 lines (extracted from 1,370-line cli.rs)
schema.rs:           1,656 lines (move as-is, largest file)
schema_gen.rs:         575 lines (move as-is)
semantic/mod.rs:       147 lines (move with import fix)
semantic/sources.rs:   602 lines (move with import fix)
semantic/source_path:  624 lines (move with import fix)
semantic/dq_rules.rs:1,999 lines (move with import fix, second largest)
semantic/gold.rs:      882 lines (move with import fix)
semantic/domain.rs:    926 lines (move with import fix)
semantic/table_exists: 236 lines (move with import fix)
validate/mod.rs:      ~200 lines (NEW convenience API)

Total library code: ~8,529 lines migrated + ~200 new
CLI command:         ~400 lines (new)
```

### 10.3 Dependency Impact

```
ANALYSIS: New Dependencies Added to ndp-lib

jsonschema  0.17  -- JSON Schema validation engine
schemars    0.8   -- Derive JSON Schema from Rust types
serde_yaml  0.9   -- YAML config parsing
sqlparser   0.50  -- SQL WHERE clause parsing for DQ rules
regex       1     -- Pattern validation
strsim      0.11  -- Levenshtein distance for suggestions

Build impact: ~15-20 second compile time increase for ndp-lib
              (these are non-trivial dependencies, especially sqlparser)

Binary size: ~2-3 MB increase for ndp binary
             (well within 15 MB target)
```

### 10.4 Risk Assessment

```
RISK MATRIX:

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Use path find-replace misses a reference | Medium | Low | cargo check catches immediately |
| cli.rs split loses test coverage | Medium | Medium | Count tests before/after; rewrite CLI tests |
| OutputFormat needs clap derive in lib | Low | Low | Wrapper enum in CLI layer |
| schema_gen.rs ndp-types import breaks | Low | Medium | ndp-types already in ndp-lib deps |
| deploy.sh flag mapping wrong | Medium | High | Integration test catches |
| ndp-validate standalone breaks | Low | Medium | Dedicated test: cargo test -p ndp-validate |
| serde_yaml version conflict | Low | Low | Same version as ndp-validate uses |
| sqlparser version conflict | Low | Low | Same version as ndp-validate uses |
| Circular dependency introduced | None | -- | ndp-lib has no workspace deps except ndp-types |
```

---

## Appendix A: File Inventory

Complete list of files involved in the v1.1.15 migration:

```
MOVED (11 files, tools/ndp-validate/src/ -> crates/ndp-lib/src/validate/):
  error.rs
  schema.rs
  schema_gen.rs
  semantic/mod.rs
  semantic/sources.rs
  semantic/source_path.rs
  semantic/dq_rules.rs
  semantic/gold.rs
  semantic/domain.rs
  semantic/table_exists.rs

EXTRACTED (1 file):
  cli.rs (types portion) -> validate/result.rs

CREATED (2 files):
  crates/ndp-lib/src/validate/mod.rs
  tools/ndp-cli/src/commands/validate.rs

MODIFIED (5 files):
  crates/ndp-lib/src/lib.rs
  crates/ndp-lib/Cargo.toml
  tools/ndp-cli/src/commands/mod.rs
  tools/ndp-cli/src/main.rs
  deploy/pi/deploy.sh

MODIFIED (after migration, thin wrapper):
  tools/ndp-validate/Cargo.toml
  tools/ndp-validate/src/lib.rs
  tools/ndp-validate/src/main.rs
```

---

## Appendix B: Comparison with Phase 1 Approach

| Aspect | Phase 1 (Gold) | Phase 2 (Validate) |
|---|---|---|
| Files moved | 29 | 12 (11 copy + 1 extract) |
| Lines | ~12,000 | ~9,900 |
| Tests | 376 | 217 |
| deploy.sh sites | 2 | 2 |
| Module split | None (1:1 file map) | cli.rs splits into result.rs + CLI formatting |
| New dependencies | mockall (dev) | jsonschema, schemars, serde_yaml, sqlparser, regex, strsim |
| DB trait unification | CaChecker -> DbClient | None needed |
| Config loader | Gold ConfigLoader moves | No config loader (reads files directly) |
| Error types | GoldDdlError (independent) | ValidationError (structured findings, not Rust errors) |
| CLI complexity | 3 subcommands (generate/sync/recreate) | 3 subcommands (stream/domain/schema) |

### Key Differences from Phase 1

1. **cli.rs split is the hardest part.** Phase 1 had no split -- every file mapped 1:1. Phase 2 must extract library types from CLI presentation code. This is the highest-risk step.

2. **No database trait work.** Phase 1 required CaChecker -> DbClient adaptation. Phase 2 has no DB interaction (except optional --check-tables which is not yet implemented).

3. **More new dependencies.** Phase 1 added only mockall. Phase 2 adds 6 runtime dependencies to ndp-lib. This increases compile time and binary size.

4. **Test rewriting required.** Phase 1 tests moved 1:1. Phase 2 has ~50 Clap-specific tests that must be rewritten for the new entity/verb structure.

5. **No config loader pattern.** Phase 1 migrated a ConfigLoader trait with FileSystemConfigLoader. Phase 2 validate functions take file paths directly -- no loader abstraction.
