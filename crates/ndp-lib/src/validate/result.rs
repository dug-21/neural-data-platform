//! Validation result types for NDP validation
//!
//! Contains library-side types extracted from the CLI module:
//! - `ValidationResult` / `BatchValidationResult` for structured output
//! - `ValidationSummary` / `BatchSummary` for aggregate counts
//! - `OutputFormat` for selecting JSON vs human output
//! - `exit_codes` module for standard process exit codes
//!
//! These types are consumed by both `ndp-validate` (standalone) and `ndp validate` (CLI subcommand).

use crate::validate::error::{ValidationError, ValidationLayer};
use serde::Serialize;
use std::collections::HashMap;

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
// Output Format
// =============================================================================

/// Output format for validation results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Structured JSON output (default, for scripting)
    #[default]
    Json,
    /// Human-readable terminal output with colors
    Human,
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
        *self
            .summary
            .by_layer
            .entry(layer_name.to_string())
            .or_insert(0) += 1;
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
    serde_json::to_string_pretty(result)
        .unwrap_or_else(|e| format!(r#"{{"error": "JSON serialization failed: {}"}}"#, e))
}

/// Format batch validation results as JSON
pub fn output_json_batch(results: &BatchValidationResult) -> String {
    serde_json::to_string_pretty(results)
        .unwrap_or_else(|e| format!(r#"{{"error": "JSON serialization failed: {}"}}"#, e))
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
    if !result.valid || (strict && !result.warnings.is_empty()) {
        exit_codes::VALIDATION_ERROR
    } else {
        exit_codes::SUCCESS
    }
}

/// Determine exit code for batch results
pub fn determine_batch_exit_code(results: &BatchValidationResult, strict: bool) -> i32 {
    if !results.all_valid() || (strict && results.has_warnings()) {
        exit_codes::VALIDATION_ERROR
    } else {
        exit_codes::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::error::{ErrorCode, Severity};

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
}
