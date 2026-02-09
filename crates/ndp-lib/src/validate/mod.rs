//! NDP Configuration Validation Library
//!
//! Provides three-layer validation for NDP stream and domain configurations:
//!
//! - **Layer 0 (Syntax)**: JSON parsing errors
//! - **Layer 1 (Schema)**: JSON Schema validation (Draft 2020-12)
//! - **Layer 2 (Semantic)**: Application-level rules (source configs, field references,
//!   DQ rules, Gold ETL, domain alignment)
//!
//! Also provides validation result types and output formatting used by
//! both `ndp-validate` (standalone tool) and `ndp validate` (CLI subcommand).
//!
//! # Public API
//!
//! The main entry points for consumers are the convenience functions:
//!
//! - [`validate_stream`] - Validate a single stream config (parsed JSON)
//! - [`validate_stream_file`] - Validate a single stream config from a file path
//! - [`validate_all_streams`] - Validate all stream configs in a directory
//! - [`validate_domain_config`] - Validate a single domain config (parsed JSON)
//! - [`validate_domain_file`] - Validate a single domain config from a file path
//! - [`validate_all_domains`] - Validate all domain configs in a directory
//!
//! All convenience functions accept a [`ValidateOptions`] struct that controls
//! behavior (schema-only mode, strict mode, output format, etc.).

pub mod error;
pub mod result;
pub mod schema;
pub mod schema_gen;
pub mod semantic;

use std::path::{Path, PathBuf};

// Re-exports: error types
pub use error::{ErrorCode, SchemaValidatorError, Severity, ValidationError, ValidationLayer};

// Re-exports: result types
pub use result::{
    determine_batch_exit_code, determine_exit_code, exit_codes, output_human, output_human_batch,
    output_json, output_json_batch, BatchSummary, BatchValidationResult, OutputFormat,
    ValidationResult, ValidationSummary,
};

// Re-exports: schema validators
pub use schema::{DomainSchemaValidator, SchemaValidator};
pub use schema_gen::{compare_schemas, generate_schema, verify_schema, SchemaDifference};

// Re-exports: semantic validators
pub use semantic::table_exists::parse_table_reference;
pub use semantic::{
    validate_domain, validate_domain_semantic, validate_dq_rules, validate_gold_etl,
    validate_source_paths, validate_sources, validate_table_exists, Validator as SemanticValidator,
};

// =============================================================================
// ValidateOptions
// =============================================================================

/// Options controlling validation behavior.
///
/// Constructed from CLI args in ndp-cli or ndp-validate, or directly by
/// library consumers.
#[derive(Debug, Clone)]
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

// =============================================================================
// Public Convenience Functions — Stream Validation
// =============================================================================

/// Validate a single stream configuration (parsed JSON).
///
/// Runs Layer 1 (Schema) validation, and unless `opts.schema_only` is set,
/// also runs Layer 2 (Semantic) validation.
///
/// # Arguments
///
/// * `config` - Parsed JSON value of the stream configuration
/// * `opts` - Validation options controlling behavior
///
/// # Returns
///
/// A `ValidationResult` containing errors, warnings, and validity status.
pub fn validate_stream(config: &serde_json::Value, opts: &ValidateOptions) -> ValidationResult {
    let config_path = config
        .pointer("/info/stream_id")
        .or_else(|| config.get("stream_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("<inline>");

    let mut result = ValidationResult::new(config_path);

    // Layer 1: Schema validation
    let schema_validator = match resolve_stream_schema_validator(opts) {
        Ok(v) => v,
        Err(e) => {
            result.add_error(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                "$",
                format!("Failed to load schema: {}", e),
            ));
            return result;
        }
    };

    let schema_errors = schema_validator.validate_schema(config);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation (unless --schema-only)
    if !opts.schema_only {
        let semantic_validator = SemanticValidator::new();
        let semantic_errors = semantic_validator.validate(config);
        for error in semantic_errors {
            if error.severity == Severity::Warning {
                result.add_warning(error);
            } else {
                result.add_error(error);
            }
        }
    }

    result
}

/// Validate a single stream configuration from a file path.
///
/// Reads the file, parses as JSON, and delegates to [`validate_stream`].
///
/// # Arguments
///
/// * `path` - Path to the stream config JSON file
/// * `opts` - Validation options controlling behavior
///
/// # Returns
///
/// A `ValidationResult` or an error if the file cannot be read/parsed.
pub fn validate_stream_file(
    path: &Path,
    opts: &ValidateOptions,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("Config file not found: {}", path.display()).into());
    }

    let content = std::fs::read_to_string(path)?;

    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            let mut result = ValidationResult::new(path.display().to_string());
            result.add_error(ValidationError::syntax_error(
                e.line(),
                e.column(),
                &e.to_string(),
            ));
            return Ok(result);
        }
    };

    let mut result = validate_stream(&value, opts);
    // Override config_path with the actual file path
    result.config_path = path.display().to_string();
    Ok(result)
}

/// Validate all stream configurations in a directory.
///
/// Discovers stream config files (looking for `config.json` in subdirectories),
/// validates each one, and aggregates results.
///
/// # Arguments
///
/// * `config_dir` - Path to the streams config directory (e.g., `config/base/streams`)
/// * `opts` - Validation options controlling behavior
///
/// # Returns
///
/// A `BatchValidationResult` or an error if the directory cannot be read.
pub fn validate_all_streams(
    config_dir: &Path,
    opts: &ValidateOptions,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>> {
    if !config_dir.exists() {
        return Err(format!("Config directory not found: {}", config_dir.display()).into());
    }

    let mut results = Vec::new();

    for entry in std::fs::read_dir(config_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let config_json = path.join("config.json");
            if config_json.exists() {
                match validate_stream_file(&config_json, opts) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        let mut result = ValidationResult::new(config_json.display().to_string());
                        result.add_error(ValidationError::semantic_error(
                            ErrorCode::MissingSourceConfig,
                            "$",
                            format!("Failed to validate: {}", e),
                        ));
                        results.push(result);
                    }
                }
            }
        }
    }

    if results.is_empty() {
        return Err(format!("No config.json files found in {}", config_dir.display()).into());
    }

    Ok(BatchValidationResult::from_results(results))
}

// =============================================================================
// Public Convenience Functions — Domain Validation
// =============================================================================

/// Validate a single domain configuration (parsed JSON).
///
/// Runs Layer 1 (Schema) validation, and unless `opts.schema_only` is set,
/// also runs Layer 2 (Semantic) validation with stream cross-referencing.
///
/// # Arguments
///
/// * `config` - Parsed JSON value of the domain configuration
/// * `streams_dir` - Optional path to streams config directory for cross-referencing
///
/// # Returns
///
/// A `ValidationResult` containing errors, warnings, and validity status.
pub fn validate_domain_config(
    config: &serde_json::Value,
    streams_dir: Option<&Path>,
) -> ValidationResult {
    let config_id = config
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("<inline>");

    let mut result = ValidationResult::new(config_id);

    // Layer 1: Schema validation
    let schema_validator = match DomainSchemaValidator::default_schema() {
        Ok(v) => v,
        Err(e) => {
            result.add_error(ValidationError::semantic_error(
                ErrorCode::InvalidDomainStream,
                "$",
                format!("Failed to load domain schema: {}", e),
            ));
            return result;
        }
    };

    let schema_errors = schema_validator.validate_schema(config);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation
    let semantic_errors = validate_domain_semantic(config, streams_dir);
    for error in semantic_errors {
        if error.severity == Severity::Warning {
            result.add_warning(error);
        } else {
            result.add_error(error);
        }
    }

    result
}

/// Validate a single domain configuration from a file path.
///
/// Reads the file, parses as JSON, and runs schema + semantic validation.
///
/// # Arguments
///
/// * `path` - Path to the domain config JSON file
/// * `opts` - Validation options controlling behavior
///
/// # Returns
///
/// A `ValidationResult` or an error if the file cannot be read/parsed.
pub fn validate_domain_file(
    path: &Path,
    opts: &ValidateOptions,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("Domain config file not found: {}", path.display()).into());
    }

    let content = std::fs::read_to_string(path)?;

    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            let mut result = ValidationResult::new(path.display().to_string());
            result.add_error(ValidationError::syntax_error(
                e.line(),
                e.column(),
                &e.to_string(),
            ));
            return Ok(result);
        }
    };

    let mut result = ValidationResult::new(path.display().to_string());

    // Layer 1: Schema validation
    let schema_validator = resolve_domain_schema_validator(opts)?;
    let schema_errors = schema_validator.validate_schema(&value);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation (unless --schema-only)
    if !opts.schema_only {
        let streams_dir = &opts.config_dir;
        let semantic_errors = validate_domain_semantic(&value, Some(streams_dir));
        for error in semantic_errors {
            if error.severity == Severity::Warning {
                result.add_warning(error);
            } else {
                result.add_error(error);
            }
        }
    }

    Ok(result)
}

/// Validate all domain configurations in a directory.
///
/// Discovers domain config files (looking for `domain.json` in subdirectories),
/// validates each one, and aggregates results.
///
/// # Arguments
///
/// * `domains_dir` - Path to the domains directory (e.g., `config/domains`)
/// * `config_dir` - Path to streams config directory for cross-referencing
/// * `opts` - Validation options controlling behavior
///
/// # Returns
///
/// A `BatchValidationResult` or an error if the directory cannot be read.
pub fn validate_all_domains(
    domains_dir: &Path,
    config_dir: &Path,
    opts: &ValidateOptions,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>> {
    if !domains_dir.exists() {
        return Err(format!("Domains directory not found: {}", domains_dir.display()).into());
    }

    // Build opts with the provided config_dir for stream cross-referencing
    let domain_opts = ValidateOptions {
        config_dir: config_dir.to_path_buf(),
        ..opts.clone()
    };

    let mut results = Vec::new();

    for entry in std::fs::read_dir(domains_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let domain_json = path.join("domain.json");
            if domain_json.exists() {
                match validate_domain_file(&domain_json, &domain_opts) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        let mut result = ValidationResult::new(domain_json.display().to_string());
                        result.add_error(ValidationError::semantic_error(
                            ErrorCode::InvalidDomainStream,
                            "$",
                            format!("Failed to validate: {}", e),
                        ));
                        results.push(result);
                    }
                }
            }
        }
    }

    if results.is_empty() {
        return Err(format!("No domain.json files found in {}", domains_dir.display()).into());
    }

    Ok(BatchValidationResult::from_results(results))
}

// =============================================================================
// Internal Helpers
// =============================================================================

/// Resolve the stream schema validator from options.
///
/// Uses `opts.schema_path` if provided, otherwise falls back to the
/// embedded default schema.
fn resolve_stream_schema_validator(
    opts: &ValidateOptions,
) -> Result<SchemaValidator, SchemaValidatorError> {
    match &opts.schema_path {
        Some(path) if path.exists() => SchemaValidator::from_file(path),
        _ => SchemaValidator::default_schema(),
    }
}

/// Resolve the domain schema validator from options.
///
/// Uses `opts.domain_schema_path` if provided, otherwise falls back to the
/// embedded default schema.
fn resolve_domain_schema_validator(
    opts: &ValidateOptions,
) -> Result<DomainSchemaValidator, SchemaValidatorError> {
    match &opts.domain_schema_path {
        Some(path) if path.exists() => DomainSchemaValidator::from_file(path),
        _ => DomainSchemaValidator::default_schema(),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_options_default() {
        let opts = ValidateOptions::default();
        assert!(!opts.schema_only);
        assert!(!opts.strict);
        assert!(!opts.check_tables);
        assert_eq!(opts.format, OutputFormat::Json);
        assert_eq!(opts.config_dir, PathBuf::from("config/base/streams"));
        assert!(opts.schema_path.is_none());
        assert!(opts.domain_schema_path.is_none());
        assert!(opts.domains_dir.is_none());
        assert!(opts.db_url.is_none());
    }

    #[test]
    fn test_validate_stream_valid_config() {
        let config = serde_json::json!({
            "info": { "stream_id": "test-stream", "version": "1.0.0" }
        });
        let opts = ValidateOptions::default();
        let result = validate_stream(&config, &opts);
        assert!(
            result.valid,
            "Valid config should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_validate_stream_invalid_config() {
        let config = serde_json::json!({
            "info": { "stream_id": "TestStream", "version": "1.0.0" }
        });
        let opts = ValidateOptions::default();
        let result = validate_stream(&config, &opts);
        assert!(!result.valid, "Invalid stream_id pattern should fail");
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validate_stream_schema_only() {
        // A config that would fail semantic validation but not schema
        let config = serde_json::json!({
            "info": { "stream_id": "test-stream", "version": "1.0.0" }
        });
        let opts = ValidateOptions {
            schema_only: true,
            ..Default::default()
        };
        let result = validate_stream(&config, &opts);
        assert!(result.valid);
    }

    #[test]
    fn test_validate_stream_file_not_found() {
        let opts = ValidateOptions::default();
        let result = validate_stream_file(Path::new("/nonexistent/config.json"), &opts);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_domain_config_schema_valid() {
        // validate_domain_config runs both schema and semantic validation.
        // Semantic validation discovers streams from streams_dir. When no
        // streams_dir is provided and no default paths exist, the stream
        // reference "air-quality" will fail semantic validation even though
        // the schema is valid. Use a tempdir with a config to prove full validation.
        let tmp = tempfile::tempdir().unwrap();
        let stream_dir = tmp.path().join("air-quality");
        std::fs::create_dir_all(&stream_dir).unwrap();
        std::fs::write(
            stream_dir.join("config.json"),
            r#"{"info":{"stream_id":"air-quality","version":"1.0.0"}}"#,
        )
        .unwrap();

        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let result = validate_domain_config(&config, Some(tmp.path()));
        assert!(
            result.valid,
            "Valid domain should pass: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_validate_domain_config_schema_invalid() {
        let config = serde_json::json!({
            "streams": [{ "stream_id": "air-quality", "role": "primary" }]
        });
        let result = validate_domain_config(&config, None);
        assert!(!result.valid, "Missing id should fail schema validation");
    }

    #[test]
    fn test_validate_domain_file_not_found() {
        let opts = ValidateOptions::default();
        let result = validate_domain_file(Path::new("/nonexistent/domain.json"), &opts);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_all_streams_dir_not_found() {
        let opts = ValidateOptions::default();
        let result = validate_all_streams(Path::new("/nonexistent/dir"), &opts);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_all_domains_dir_not_found() {
        let opts = ValidateOptions::default();
        let result = validate_all_domains(
            Path::new("/nonexistent/dir"),
            Path::new("/nonexistent/streams"),
            &opts,
        );
        assert!(result.is_err());
    }
}
