//! CLI interface for ndp-validate
//!
//! Provides command-line argument parsing using clap derive macros.
//! Supports the following options per dp-019 specification:
//!
//! - Positional config path or --all flag
//! - --schema-only to skip semantic validation
//! - --check-tables to verify Silver table existence
//! - --format json|human for output format
//! - --strict to treat warnings as errors
//! - --verbose for progress output
//!
//! Exit codes:
//! - 0: Validation passed (may have warnings)
//! - 1: Validation failed (has errors)
//! - 2: System error (file not found, schema load failed, etc.)

use crate::error::{ValidationError, ValidationLayer};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

// =============================================================================
// Exit Codes (dp-019 Section 9.2)
// =============================================================================

/// Exit codes per dp-019 specification
pub mod exit_codes {
    /// Validation passed (may have warnings)
    pub const SUCCESS: i32 = 0;

    /// Validation failed (has errors)
    pub const VALIDATION_ERROR: i32 = 1;

    /// System error (file not found, DB connection failed, etc.)
    pub const SYSTEM_ERROR: i32 = 2;
}

// =============================================================================
// CLI Arguments (dp-019 Section 6.3)
// =============================================================================

/// Output format for validation results
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    /// Structured JSON output (default, for scripting)
    #[default]
    Json,
    /// Human-readable terminal output with colors
    Human,
}

/// ndp-validate - Two-layer config validation for NDP stream configurations
///
/// Validates JSON configuration files against the NDP stream config schema (Layer 1)
/// and application-level semantic rules (Layer 2).
///
/// Exit codes:
///   0 - Validation passed (may have warnings)
///   1 - Validation failed (has errors)
///   2 - System error (file not found, schema load failed, etc.)
///
/// Schema Generation:
///   --generate-schema          Generate JSON Schema from ndp-types to stdout
///   --generate-schema --output Generate to file
///   --verify-schema PATH       Check committed schema matches generated (for CI)
#[derive(Parser, Debug)]
#[command(name = "ndp-validate")]
#[command(author = "Neural Data Platform Team")]
#[command(version)]
#[command(about = "Validate NDP stream configurations", long_about = None)]
pub struct Cli {
    /// Config file path to validate (mutually exclusive with --all, --generate-schema, --verify-schema)
    #[arg(value_name = "CONFIG_PATH")]
    pub config_path: Option<PathBuf>,

    /// Validate all configs in the base config directory
    #[arg(short, long, conflicts_with_all = ["config_path", "generate_schema", "verify_schema"])]
    pub all: bool,

    /// Generate JSON Schema from ndp-types to stdout
    ///
    /// Uses schemars to derive JSON Schema from Rust types. This ensures
    /// the schema is always in sync with the runtime types.
    #[arg(long, conflicts_with_all = ["config_path", "all", "verify_schema"])]
    pub generate_schema: bool,

    /// Write generated schema to file (requires --generate-schema)
    #[arg(long, requires = "generate_schema")]
    pub output: Option<PathBuf>,

    /// Verify committed schema matches generated schema
    ///
    /// Compares the file at PATH against freshly generated schema.
    /// Exits 0 if they match, 1 if drift is detected. Use in CI.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["config_path", "all", "generate_schema"])]
    pub verify_schema: Option<PathBuf>,

    /// Skip semantic validation (Layer 2), only run schema validation
    #[arg(long)]
    pub schema_only: bool,

    /// Check that Silver tables exist in TimescaleDB (requires DB connection)
    #[arg(long)]
    pub check_tables: bool,

    /// Output format
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,

    /// Treat warnings as errors (exit code 1 if any warnings)
    #[arg(long)]
    pub strict: bool,

    /// Show validation progress
    #[arg(short, long)]
    pub verbose: bool,

    /// Base config directory (default: config/base/streams)
    #[arg(long, env = "NDP_CONFIG_DIR", default_value = "config/base/streams")]
    pub config_dir: PathBuf,

    /// TimescaleDB connection string (required for --check-tables)
    #[arg(long, env = "TIMESCALE_URL")]
    pub timescale_url: Option<String>,

    /// JSON Schema file path
    #[arg(long, default_value = "schemas/stream-config.v1.1.schema.json")]
    pub schema_path: PathBuf,
}

impl Cli {
    /// Validate that CLI arguments are consistent
    ///
    /// Returns an error message if arguments are invalid
    pub fn validate_args(&self) -> Result<(), String> {
        // --output requires --generate-schema (enforce at runtime since clap
        // does not catch all cases when config_path is provided)
        if self.output.is_some() && !self.generate_schema {
            return Err("--output requires --generate-schema".to_string());
        }

        // Schema generation/verification modes don't need config path
        if self.generate_schema || self.verify_schema.is_some() {
            return Ok(());
        }

        // Must specify either config_path or --all
        if self.config_path.is_none() && !self.all {
            return Err("Must specify a config path or use --all".to_string());
        }

        // --check-tables requires --timescale-url
        if self.check_tables && self.timescale_url.is_none() {
            return Err("--check-tables requires --timescale-url to be set".to_string());
        }

        Ok(())
    }

    /// Check if running in schema generation mode
    pub fn is_schema_mode(&self) -> bool {
        self.generate_schema || self.verify_schema.is_some()
    }
}

// =============================================================================
// Validation Result (dp-019 Section 5.1)
// =============================================================================

/// Summary of validation counts by layer
#[derive(Debug, Clone, Serialize, Default)]
pub struct ValidationSummary {
    pub total_errors: usize,
    pub total_warnings: usize,
    pub by_layer: HashMap<String, usize>,
}

/// Complete validation result for a single config file (dp-019 Section 5.1)
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    /// Whether all validations passed (no errors)
    pub valid: bool,

    /// Path to validated config
    pub config_path: String,

    /// Summary statistics
    pub summary: ValidationSummary,

    /// List of validation errors
    pub errors: Vec<ValidationError>,

    /// List of validation warnings
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    /// Create a new empty result for a config path
    pub fn new(config_path: impl Into<String>) -> Self {
        Self {
            valid: true,
            config_path: config_path.into(),
            summary: ValidationSummary::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Add an error to the result
    pub fn add_error(&mut self, error: ValidationError) {
        self.valid = false;
        self.summary.total_errors += 1;
        let layer_name = match error.layer {
            ValidationLayer::Syntax => "syntax",
            ValidationLayer::Schema => "schema",
            ValidationLayer::Semantic => "semantic",
        };
        *self.summary.by_layer.entry(layer_name.to_string()).or_insert(0) += 1;
        self.errors.push(error);
    }

    /// Add a warning to the result
    pub fn add_warning(&mut self, warning: ValidationError) {
        self.summary.total_warnings += 1;
        self.warnings.push(warning);
    }

    /// Check if result has any issues (errors or warnings)
    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }
}

/// Batch validation result for multiple configs
#[derive(Debug, Clone, Serialize)]
pub struct BatchValidationResult {
    /// Summary across all configs
    pub summary: BatchSummary,
    /// Individual results
    pub results: Vec<ValidationResult>,
}

/// Summary for batch validation
#[derive(Debug, Clone, Serialize)]
pub struct BatchSummary {
    pub total_configs: usize,
    pub valid_configs: usize,
    pub invalid_configs: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
}

impl BatchValidationResult {
    /// Create from a list of individual results
    pub fn from_results(results: Vec<ValidationResult>) -> Self {
        let total_configs = results.len();
        let valid_configs = results.iter().filter(|r| r.valid).count();
        let invalid_configs = total_configs - valid_configs;
        let total_errors: usize = results.iter().map(|r| r.summary.total_errors).sum();
        let total_warnings: usize = results.iter().map(|r| r.summary.total_warnings).sum();

        Self {
            summary: BatchSummary {
                total_configs,
                valid_configs,
                invalid_configs,
                total_errors,
                total_warnings,
            },
            results,
        }
    }

    /// Check if all configs are valid
    pub fn all_valid(&self) -> bool {
        self.summary.invalid_configs == 0
    }

    /// Check if any warnings exist
    pub fn has_warnings(&self) -> bool {
        self.summary.total_warnings > 0
    }
}

// =============================================================================
// Output Formatting (dp-019 Section 7.3)
// =============================================================================

/// Format a single validation result as JSON
pub fn output_json(result: &ValidationResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|e| {
        format!(r#"{{"error": "JSON serialization failed: {}"}}"#, e)
    })
}

/// Format batch validation results as JSON
pub fn output_json_batch(results: &BatchValidationResult) -> String {
    serde_json::to_string_pretty(results).unwrap_or_else(|e| {
        format!(r#"{{"error": "JSON serialization failed: {}"}}"#, e)
    })
}

/// Format a single validation result for human-readable terminal output
pub fn output_human(result: &ValidationResult) -> String {
    let mut output = String::new();

    // Header with pass/fail status
    if result.valid {
        output.push_str(&format!("\x1b[32m[PASS]\x1b[0m {}\n", result.config_path));
    } else {
        output.push_str(&format!("\x1b[31m[FAIL]\x1b[0m {}\n", result.config_path));
    }

    // Errors
    if !result.errors.is_empty() {
        output.push_str("\n  ERRORS:\n");
        for error in &result.errors {
            output.push_str(&format_error_human(error, "    "));
        }
    }

    // Warnings
    if !result.warnings.is_empty() {
        output.push_str("\n  WARNINGS:\n");
        for warning in &result.warnings {
            output.push_str(&format_warning_human(warning, "    "));
        }
    }

    output
}

/// Format batch validation results for human-readable output
pub fn output_human_batch(results: &BatchValidationResult) -> String {
    let mut output = String::new();

    // Individual results
    for result in &results.results {
        output.push_str(&output_human(result));
        output.push('\n');
    }

    // Summary
    output.push_str(&"=".repeat(60));
    output.push('\n');
    output.push_str(&format!(
        "SUMMARY: {} configs validated, {} passed, {} failed\n",
        results.summary.total_configs,
        results.summary.valid_configs,
        results.summary.invalid_configs
    ));
    output.push_str(&format!(
        "         {} errors, {} warnings\n",
        results.summary.total_errors, results.summary.total_warnings
    ));

    output
}

/// Format a single error for human-readable output
fn format_error_human(error: &ValidationError, indent: &str) -> String {
    let mut output = String::new();

    // Layer and path
    output.push_str(&format!(
        "{}\x1b[31m[{:?}]\x1b[0m {}\n",
        indent, error.layer, error.path
    ));

    // Message
    output.push_str(&format!("{}  {}\n", indent, error.message));

    // Suggestion (yellow)
    if let Some(ref suggestion) = error.suggestion {
        output.push_str(&format!(
            "{}\x1b[33m  Suggestion: {}\x1b[0m\n",
            indent, suggestion
        ));
    }

    output
}

/// Format a single warning for human-readable output
fn format_warning_human(warning: &ValidationError, indent: &str) -> String {
    let mut output = String::new();

    // Layer and path (yellow)
    output.push_str(&format!(
        "{}\x1b[33m[{:?}]\x1b[0m {}\n",
        indent, warning.layer, warning.path
    ));

    // Message
    output.push_str(&format!("{}  {}\n", indent, warning.message));

    output
}

/// Determine exit code based on validation results and strict mode
pub fn determine_exit_code(result: &ValidationResult, strict: bool) -> i32 {
    if !result.valid {
        exit_codes::VALIDATION_ERROR
    } else if strict && !result.warnings.is_empty() {
        exit_codes::VALIDATION_ERROR
    } else {
        exit_codes::SUCCESS
    }
}

/// Determine exit code for batch results
pub fn determine_batch_exit_code(results: &BatchValidationResult, strict: bool) -> i32 {
    if !results.all_valid() {
        exit_codes::VALIDATION_ERROR
    } else if strict && results.has_warnings() {
        exit_codes::VALIDATION_ERROR
    } else {
        exit_codes::SUCCESS
    }
}

// =============================================================================
// Tests - Following London TDD Methodology
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, Severity};
    use clap::CommandFactory;

    // =========================================================================
    // CLI Structure Tests
    // =========================================================================

    #[test]
    fn test_cli_structure_is_valid() {
        // Verify CLI structure is valid (clap internal validation)
        Cli::command().debug_assert();
    }

    // =========================================================================
    // Positional Argument Tests
    // =========================================================================

    #[test]
    fn test_parse_single_config_path() {
        let cli = Cli::parse_from([
            "ndp-validate",
            "config/base/streams/air-quality/config.json",
        ]);

        assert_eq!(
            cli.config_path,
            Some(PathBuf::from("config/base/streams/air-quality/config.json"))
        );
        assert!(!cli.all);
    }

    #[test]
    fn test_parse_config_path_with_spaces() {
        let cli = Cli::parse_from(["ndp-validate", "/path/to/my config/file.json"]);

        assert_eq!(
            cli.config_path,
            Some(PathBuf::from("/path/to/my config/file.json"))
        );
    }

    // =========================================================================
    // --all Flag Tests
    // =========================================================================

    #[test]
    fn test_parse_all_flag() {
        let cli = Cli::parse_from(["ndp-validate", "--all"]);

        assert!(cli.all);
        assert!(cli.config_path.is_none());
    }

    #[test]
    fn test_parse_all_flag_short() {
        let cli = Cli::parse_from(["ndp-validate", "-a"]);

        assert!(cli.all);
        assert!(cli.config_path.is_none());
    }

    // =========================================================================
    // --schema-only Flag Tests
    // =========================================================================

    #[test]
    fn test_parse_schema_only_flag() {
        let cli = Cli::parse_from(["ndp-validate", "--schema-only", "config.json"]);

        assert!(cli.schema_only);
    }

    #[test]
    fn test_parse_without_schema_only_flag() {
        let cli = Cli::parse_from(["ndp-validate", "config.json"]);

        assert!(!cli.schema_only);
    }

    // =========================================================================
    // --check-tables Flag Tests
    // =========================================================================

    #[test]
    fn test_parse_check_tables_flag() {
        let cli = Cli::parse_from([
            "ndp-validate",
            "--check-tables",
            "--timescale-url",
            "postgresql://localhost/ndp",
            "config.json",
        ]);

        assert!(cli.check_tables);
        assert_eq!(
            cli.timescale_url,
            Some("postgresql://localhost/ndp".to_string())
        );
    }

    #[test]
    fn test_parse_without_check_tables() {
        let cli = Cli::parse_from(["ndp-validate", "config.json"]);

        assert!(!cli.check_tables);
    }

    // =========================================================================
    // --format Flag Tests
    // =========================================================================

    #[test]
    fn test_parse_format_json() {
        let cli = Cli::parse_from(["ndp-validate", "--format", "json", "config.json"]);

        assert_eq!(cli.format, OutputFormat::Json);
    }

    #[test]
    fn test_parse_format_human() {
        let cli = Cli::parse_from(["ndp-validate", "--format", "human", "config.json"]);

        assert_eq!(cli.format, OutputFormat::Human);
    }

    #[test]
    fn test_parse_format_default_is_json() {
        let cli = Cli::parse_from(["ndp-validate", "config.json"]);

        // JSON is the default for scripting
        assert_eq!(cli.format, OutputFormat::Json);
    }

    // =========================================================================
    // --strict Flag Tests
    // =========================================================================

    #[test]
    fn test_parse_strict_flag() {
        let cli = Cli::parse_from(["ndp-validate", "--strict", "config.json"]);

        assert!(cli.strict);
    }

    #[test]
    fn test_parse_without_strict() {
        let cli = Cli::parse_from(["ndp-validate", "config.json"]);

        assert!(!cli.strict);
    }

    // =========================================================================
    // --verbose Flag Tests
    // =========================================================================

    #[test]
    fn test_parse_verbose_flag() {
        let cli = Cli::parse_from(["ndp-validate", "--verbose", "config.json"]);

        assert!(cli.verbose);
    }

    #[test]
    fn test_parse_verbose_short() {
        let cli = Cli::parse_from(["ndp-validate", "-v", "config.json"]);

        assert!(cli.verbose);
    }

    // =========================================================================
    // Combined Options Tests
    // =========================================================================

    #[test]
    fn test_parse_all_options_combined() {
        let cli = Cli::parse_from([
            "ndp-validate",
            "--all",
            "--schema-only",
            "--format",
            "human",
            "--strict",
            "--verbose",
            "--config-dir",
            "/custom/config/dir",
        ]);

        assert!(cli.all);
        assert!(cli.schema_only);
        assert_eq!(cli.format, OutputFormat::Human);
        assert!(cli.strict);
        assert!(cli.verbose);
        assert_eq!(cli.config_dir, PathBuf::from("/custom/config/dir"));
    }

    // =========================================================================
    // Argument Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_args_requires_config_or_all() {
        let cli = Cli {
            config_path: None,
            all: false,
            schema_only: false,
            check_tables: false,
            format: OutputFormat::Json,
            strict: false,
            verbose: false,
            config_dir: PathBuf::from("config/base/streams"),
            timescale_url: None,
            schema_path: PathBuf::from("schemas/stream-config.v1.1.schema.json"),
            generate_schema: false,
            output: None,
            verify_schema: None,
        };

        let result = cli.validate_args();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Must specify a config path or use --all"));
    }

    #[test]
    fn test_validate_args_check_tables_requires_db_url() {
        let cli = Cli {
            config_path: Some(PathBuf::from("config.json")),
            all: false,
            schema_only: false,
            check_tables: true,
            format: OutputFormat::Json,
            strict: false,
            verbose: false,
            config_dir: PathBuf::from("config/base/streams"),
            timescale_url: None, // Missing!
            schema_path: PathBuf::from("schemas/stream-config.v1.1.schema.json"),
            generate_schema: false,
            output: None,
            verify_schema: None,
        };

        let result = cli.validate_args();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("--check-tables requires --timescale-url"));
    }

    #[test]
    fn test_validate_args_success_with_config_path() {
        let cli = Cli {
            config_path: Some(PathBuf::from("config.json")),
            all: false,
            schema_only: false,
            check_tables: false,
            format: OutputFormat::Json,
            strict: false,
            verbose: false,
            config_dir: PathBuf::from("config/base/streams"),
            timescale_url: None,
            schema_path: PathBuf::from("schemas/stream-config.v1.1.schema.json"),
            generate_schema: false,
            output: None,
            verify_schema: None,
        };

        assert!(cli.validate_args().is_ok());
    }

    // =========================================================================
    // Exit Code Tests
    // =========================================================================

    #[test]
    fn test_exit_code_0_on_success() {
        assert_eq!(exit_codes::SUCCESS, 0);
    }

    #[test]
    fn test_exit_code_1_on_validation_error() {
        assert_eq!(exit_codes::VALIDATION_ERROR, 1);
    }

    #[test]
    fn test_exit_code_2_on_system_error() {
        assert_eq!(exit_codes::SYSTEM_ERROR, 2);
    }

    #[test]
    fn test_determine_exit_code_success() {
        let result = ValidationResult::new("config.json");
        assert_eq!(determine_exit_code(&result, false), exit_codes::SUCCESS);
    }

    #[test]
    fn test_determine_exit_code_validation_error() {
        let mut result = ValidationResult::new("config.json");
        result.add_error(ValidationError::schema_error(
            ErrorCode::MissingRequired,
            "$.stream_id",
            "Required field missing",
        ));
        assert_eq!(
            determine_exit_code(&result, false),
            exit_codes::VALIDATION_ERROR
        );
    }

    #[test]
    fn test_determine_exit_code_strict_with_warnings() {
        let mut result = ValidationResult::new("config.json");
        result.add_warning(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::UnknownDeviceClass,
            path: "$.fields[0].device_class".to_string(),
            message: "Unknown device class".to_string(),
            severity: Severity::Warning,
            suggestion: None,
            context: None,
        });
        // Without strict: success (warnings don't fail)
        assert_eq!(determine_exit_code(&result, false), exit_codes::SUCCESS);
        // With strict: validation error (warnings treated as errors)
        assert_eq!(
            determine_exit_code(&result, true),
            exit_codes::VALIDATION_ERROR
        );
    }

    // =========================================================================
    // OutputFormat Tests
    // =========================================================================

    #[test]
    fn test_output_format_default() {
        assert_eq!(OutputFormat::default(), OutputFormat::Json);
    }

    // =========================================================================
    // ValidationResult Tests
    // =========================================================================

    #[test]
    fn test_validation_result_new() {
        let result = ValidationResult::new("config.json");
        assert!(result.valid);
        assert_eq!(result.config_path, "config.json");
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
        assert_eq!(result.summary.total_errors, 0);
        assert_eq!(result.summary.total_warnings, 0);
    }

    #[test]
    fn test_validation_result_add_error() {
        let mut result = ValidationResult::new("config.json");
        result.add_error(ValidationError::schema_error(
            ErrorCode::MissingRequired,
            "$.stream_id",
            "Required field 'stream_id' is missing",
        ));

        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.summary.total_errors, 1);
        assert_eq!(result.summary.by_layer.get("schema"), Some(&1));
    }

    #[test]
    fn test_validation_result_add_warning() {
        let mut result = ValidationResult::new("config.json");
        result.add_warning(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::UnknownDeviceClass,
            path: "$.fields[0].device_class".to_string(),
            message: "Unknown device class".to_string(),
            severity: Severity::Warning,
            suggestion: None,
            context: None,
        });

        assert!(result.valid); // Warnings don't make it invalid
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.summary.total_warnings, 1);
    }

    // =========================================================================
    // JSON Output Tests
    // =========================================================================

    #[test]
    fn test_output_json_format() {
        let result = ValidationResult::new("config.json");
        let output = output_json(&result);

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["config_path"], "config.json");
    }

    #[test]
    fn test_output_json_with_errors() {
        let mut result = ValidationResult::new("config.json");
        result.add_error(ValidationError::schema_error(
            ErrorCode::MissingRequired,
            "$.stream_id",
            "Required field missing",
        ));

        let output = output_json(&result);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["errors"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["summary"]["total_errors"], 1);
    }

    // =========================================================================
    // Human-Readable Output Tests
    // =========================================================================

    #[test]
    fn test_output_human_format_pass() {
        let result = ValidationResult::new("config.json");
        let output = output_human(&result);

        assert!(output.contains("[PASS]"));
        assert!(output.contains("config.json"));
    }

    #[test]
    fn test_output_human_format_fail() {
        let mut result = ValidationResult::new("config.json");
        result.add_error(ValidationError::schema_error(
            ErrorCode::MissingRequired,
            "$.stream_id",
            "Required field missing",
        ));

        let output = output_human(&result);

        assert!(output.contains("[FAIL]"));
        assert!(output.contains("ERRORS:"));
        assert!(output.contains("$.stream_id"));
        assert!(output.contains("Required field missing"));
    }

    #[test]
    fn test_output_human_format_with_suggestions() {
        let mut result = ValidationResult::new("config.json");
        let error = ValidationError::schema_error(
            ErrorCode::UnknownField,
            "$.silver_elt",
            "Unknown field 'silver_elt'",
        )
        .with_suggestion("Did you mean 'silver_etl'?");
        result.add_error(error);

        let output = output_human(&result);

        assert!(output.contains("Suggestion:"));
        assert!(output.contains("silver_etl"));
    }

    // =========================================================================
    // Batch Validation Tests
    // =========================================================================

    #[test]
    fn test_batch_validation_result() {
        let result1 = ValidationResult::new("config1.json");
        let mut result2 = ValidationResult::new("config2.json");
        result2.add_error(ValidationError::schema_error(
            ErrorCode::MissingRequired,
            "$.stream_id",
            "Missing",
        ));

        let batch = BatchValidationResult::from_results(vec![result1, result2]);

        assert_eq!(batch.summary.total_configs, 2);
        assert_eq!(batch.summary.valid_configs, 1);
        assert_eq!(batch.summary.invalid_configs, 1);
        assert_eq!(batch.summary.total_errors, 1);
        assert!(!batch.all_valid());
    }

    // =========================================================================
    // Error Case Tests (clap should reject invalid combinations)
    // =========================================================================

    #[test]
    fn test_all_and_config_path_conflict() {
        // clap should reject this combination due to conflicts_with
        let result = Cli::try_parse_from(["ndp-validate", "--all", "config.json"]);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format_rejected() {
        let result = Cli::try_parse_from([
            "ndp-validate",
            "--format",
            "xml", // Invalid format
            "config.json",
        ]);

        assert!(result.is_err());
    }

    // =========================================================================
    // Schema Generation Tests
    // =========================================================================

    #[test]
    fn test_parse_generate_schema_flag() {
        let cli = Cli::parse_from(["ndp-validate", "--generate-schema"]);

        assert!(cli.generate_schema);
        assert!(cli.config_path.is_none());
        assert!(!cli.all);
    }

    #[test]
    fn test_parse_generate_schema_with_output() {
        let cli = Cli::parse_from([
            "ndp-validate",
            "--generate-schema",
            "--output",
            "schemas/output.json",
        ]);

        assert!(cli.generate_schema);
        assert_eq!(cli.output, Some(PathBuf::from("schemas/output.json")));
    }

    #[test]
    fn test_output_requires_generate_schema() {
        // --output without --generate-schema should fail in validate_args
        // Note: clap does not catch this case when config_path is provided,
        // so we validate at runtime
        let cli = Cli {
            config_path: Some(PathBuf::from("config.json")),
            all: false,
            schema_only: false,
            check_tables: false,
            format: OutputFormat::Json,
            strict: false,
            verbose: false,
            config_dir: PathBuf::from("config/base/streams"),
            timescale_url: None,
            schema_path: PathBuf::from("schemas/stream-config.v1.1.schema.json"),
            generate_schema: false,
            output: Some(PathBuf::from("output.json")), // output without generate_schema
            verify_schema: None,
        };

        let result = cli.validate_args();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--output requires --generate-schema"));
    }

    #[test]
    fn test_parse_verify_schema_flag() {
        let cli = Cli::parse_from([
            "ndp-validate",
            "--verify-schema",
            "schemas/existing.json",
        ]);

        assert_eq!(cli.verify_schema, Some(PathBuf::from("schemas/existing.json")));
        assert!(cli.config_path.is_none());
        assert!(!cli.all);
        assert!(!cli.generate_schema);
    }

    #[test]
    fn test_generate_schema_conflicts_with_config_path() {
        let result = Cli::try_parse_from([
            "ndp-validate",
            "--generate-schema",
            "config.json",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn test_generate_schema_conflicts_with_verify_schema() {
        let result = Cli::try_parse_from([
            "ndp-validate",
            "--generate-schema",
            "--verify-schema",
            "schema.json",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_schema_conflicts_with_all() {
        let result = Cli::try_parse_from([
            "ndp-validate",
            "--verify-schema",
            "schema.json",
            "--all",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_args_passes_for_generate_schema() {
        let cli = Cli {
            config_path: None,
            all: false,
            generate_schema: true,
            output: None,
            verify_schema: None,
            schema_only: false,
            check_tables: false,
            format: OutputFormat::Json,
            strict: false,
            verbose: false,
            config_dir: PathBuf::from("config/base/streams"),
            timescale_url: None,
            schema_path: PathBuf::from("schemas/stream-config.v1.1.schema.json"),
        };

        assert!(cli.validate_args().is_ok());
        assert!(cli.is_schema_mode());
    }

    #[test]
    fn test_validate_args_passes_for_verify_schema() {
        let cli = Cli {
            config_path: None,
            all: false,
            generate_schema: false,
            output: None,
            verify_schema: Some(PathBuf::from("schema.json")),
            schema_only: false,
            check_tables: false,
            format: OutputFormat::Json,
            strict: false,
            verbose: false,
            config_dir: PathBuf::from("config/base/streams"),
            timescale_url: None,
            schema_path: PathBuf::from("schemas/stream-config.v1.1.schema.json"),
        };

        assert!(cli.validate_args().is_ok());
        assert!(cli.is_schema_mode());
    }

    #[test]
    fn test_is_schema_mode() {
        // Not schema mode
        let cli = Cli::parse_from(["ndp-validate", "config.json"]);
        assert!(!cli.is_schema_mode());

        // Generate schema mode
        let cli = Cli::parse_from(["ndp-validate", "--generate-schema"]);
        assert!(cli.is_schema_mode());

        // Verify schema mode
        let cli = Cli::parse_from(["ndp-validate", "--verify-schema", "schema.json"]);
        assert!(cli.is_schema_mode());
    }
}
