//! Validate subcommand: `ndp validate [flags]`.
//!
//! Routes validation operations to `ndp_lib::validate::*` functions.
//! Uses flat flags (not subcommands) per specification.
//!
//! # Examples
//!
//! ```bash
//! ndp validate --stream config/base/streams/air-quality/config.json
//! ndp validate --all
//! ndp validate --domain config/domains/indoor-air-quality/domain.json
//! ndp validate --domain-all
//! ndp validate --schema --generate
//! ndp validate --schema --verify schemas/ndp-types.json
//! ```

use clap::Args;
use std::path::{Path, PathBuf};

use ndp_lib::validate::{
    self, determine_batch_exit_code, determine_exit_code, exit_codes, output_human,
    output_human_batch, output_json, output_json_batch, BatchValidationResult,
    DomainSchemaValidator, OutputFormat, SchemaValidator, SemanticValidator, ValidationResult,
};

/// Validate NDP stream and domain configurations.
#[derive(Args)]
pub struct ValidateArgs {
    /// Validate a single stream config file.
    #[arg(long)]
    pub stream: Option<PathBuf>,

    /// Validate all stream configs in the config directory.
    #[arg(long)]
    pub all: bool,

    /// Validate a single domain config file.
    #[arg(long)]
    pub domain: Option<PathBuf>,

    /// Validate all domain configs.
    #[arg(long, name = "domain-all")]
    pub domain_all: bool,

    /// Schema operations mode.
    #[arg(long)]
    pub schema: bool,

    /// Generate JSON Schema (requires --schema).
    #[arg(long)]
    pub generate: bool,

    /// Verify committed schema matches generated (requires --schema).
    #[arg(long)]
    pub verify: Option<PathBuf>,

    /// Output file for schema generation.
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Schema-only validation (skip semantic checks).
    #[arg(long)]
    pub schema_only: bool,

    /// Check table existence in TimescaleDB.
    #[arg(long)]
    pub check_tables: bool,

    /// Output format: human or json.
    #[arg(long, default_value = "json")]
    pub format: String,

    /// Strict mode (warnings become errors).
    #[arg(long)]
    pub strict: bool,

    /// Path to stream JSON Schema file.
    #[arg(long)]
    pub schema_path: Option<PathBuf>,

    /// Path to domain JSON Schema file.
    #[arg(long)]
    pub domain_schema_path: Option<PathBuf>,
}

/// Resolve the output format from the --format string.
fn resolve_format(format_str: &str) -> Result<OutputFormat, String> {
    match format_str.to_lowercase().as_str() {
        "human" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        _ => Err(format!(
            "Unknown output format '{}'. Expected 'human' or 'json'.",
            format_str
        )),
    }
}

/// Run the validate subcommand. Returns an exit code (0/1/2).
pub fn run(args: ValidateArgs, base_config_dir: &Path) -> i32 {
    // Parse output format
    let format = match resolve_format(&args.format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {}", e);
            return exit_codes::SYSTEM_ERROR;
        }
    };

    // Dispatch based on flags
    if args.schema && args.generate {
        return run_schema_generate(&args);
    }

    if args.schema {
        if let Some(ref verify_path) = args.verify {
            return run_schema_verify(verify_path);
        }
        eprintln!("Error: --schema requires --generate or --verify <path>.");
        return exit_codes::SYSTEM_ERROR;
    }

    if let Some(ref stream_path) = args.stream {
        return run_validate_stream(stream_path, &args, format, base_config_dir);
    }

    if args.all {
        return run_validate_all_streams(base_config_dir, &args, format);
    }

    if let Some(ref domain_path) = args.domain {
        return run_validate_domain(domain_path, &args, format, base_config_dir);
    }

    if args.domain_all {
        return run_validate_all_domains(base_config_dir, &args, format);
    }

    // No flags specified -- print help hint
    eprintln!("No validation target specified.");
    eprintln!("Use one of: --stream <path>, --all, --domain <path>, --domain-all, --schema --generate, --schema --verify <path>");
    eprintln!("Run 'ndp validate --help' for full usage.");
    exit_codes::SYSTEM_ERROR
}

// =============================================================================
// Schema generation / verification
// =============================================================================

fn run_schema_generate(args: &ValidateArgs) -> i32 {
    match validate::generate_schema() {
        Ok(schema_json) => {
            if let Some(ref output_path) = args.output {
                match std::fs::write(output_path, &schema_json) {
                    Ok(()) => {
                        eprintln!("Schema written to {}", output_path.display());
                        exit_codes::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("Error writing schema to file: {}", e);
                        exit_codes::SYSTEM_ERROR
                    }
                }
            } else {
                println!("{}", schema_json);
                exit_codes::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Error generating schema: {}", e);
            exit_codes::SYSTEM_ERROR
        }
    }
}

fn run_schema_verify(verify_path: &Path) -> i32 {
    if !verify_path.exists() {
        eprintln!("Schema file not found: {}", verify_path.display());
        return exit_codes::SYSTEM_ERROR;
    }

    match validate::verify_schema(verify_path) {
        Ok(true) => {
            eprintln!("Schema verification PASSED");
            eprintln!("Committed schema matches generated schema.");
            exit_codes::SUCCESS
        }
        Ok(false) => {
            eprintln!("Schema verification FAILED - drift detected!");
            // Show differences
            match validate::compare_schemas(verify_path) {
                Ok(differences) => {
                    eprintln!("Found {} difference(s):", differences.len());
                    for diff in &differences {
                        eprintln!("  - {}", diff);
                    }
                }
                Err(e) => {
                    eprintln!("Could not compute differences: {}", e);
                }
            }
            eprintln!();
            eprintln!("To fix, regenerate the schema:");
            eprintln!(
                "  ndp validate --schema --generate --output {}",
                verify_path.display()
            );
            exit_codes::VALIDATION_ERROR
        }
        Err(e) => {
            eprintln!("Error verifying schema: {}", e);
            exit_codes::SYSTEM_ERROR
        }
    }
}

// =============================================================================
// Stream validation
// =============================================================================

fn run_validate_stream(
    stream_path: &Path,
    args: &ValidateArgs,
    format: OutputFormat,
    _base_config_dir: &Path,
) -> i32 {
    match validate_stream_file(stream_path, args) {
        Ok(result) => {
            output_result(&result, format);
            determine_exit_code(&result, args.strict)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            exit_codes::SYSTEM_ERROR
        }
    }
}

fn run_validate_all_streams(
    base_config_dir: &Path,
    args: &ValidateArgs,
    format: OutputFormat,
) -> i32 {
    let streams_dir = base_config_dir.join("streams");
    if !streams_dir.exists() {
        eprintln!("Streams directory not found: {}", streams_dir.display());
        return exit_codes::SYSTEM_ERROR;
    }

    let mut results = Vec::new();

    // Walk subdirectories looking for config.json files
    match std::fs::read_dir(&streams_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let config_json = path.join("config.json");
                    if config_json.exists() {
                        match validate_stream_file(&config_json, args) {
                            Ok(result) => results.push(result),
                            Err(e) => {
                                let mut result =
                                    ValidationResult::new(config_json.display().to_string());
                                result.add_error(
                                    ndp_lib::validate::ValidationError::semantic_error(
                                        ndp_lib::validate::ErrorCode::InvalidSourceConfig,
                                        "$",
                                        format!("Failed to validate: {}", e),
                                    ),
                                );
                                results.push(result);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error reading streams directory: {}", e);
            return exit_codes::SYSTEM_ERROR;
        }
    }

    if results.is_empty() {
        eprintln!("No config.json files found in {}", streams_dir.display());
        return exit_codes::SYSTEM_ERROR;
    }

    let batch = BatchValidationResult::from_results(results);
    output_batch_result(&batch, format);
    determine_batch_exit_code(&batch, args.strict)
}

/// Validate a single stream configuration file.
fn validate_stream_file(
    stream_path: &Path,
    args: &ValidateArgs,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    if !stream_path.exists() {
        return Err(format!("Config file not found: {}", stream_path.display()).into());
    }

    let mut result = ValidationResult::new(stream_path.display().to_string());

    // Read file content
    let content = std::fs::read_to_string(stream_path)?;

    // Parse as JSON
    let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        result.add_error(ndp_lib::validate::ValidationError::syntax_error(
            e.line(),
            e.column(),
            &e.to_string(),
        ));
        format!("JSON parse error: {}", e)
    })?;

    // Layer 1: Schema validation
    let schema_validator = if let Some(ref schema_path) = args.schema_path {
        if schema_path.exists() {
            SchemaValidator::from_file(schema_path)?
        } else {
            SchemaValidator::default_schema()?
        }
    } else {
        SchemaValidator::default_schema()?
    };

    let schema_errors = schema_validator.validate_schema(&value);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation (unless --schema-only)
    if !args.schema_only {
        let semantic_validator = SemanticValidator::new();
        let semantic_errors = semantic_validator.validate(&value);
        for error in semantic_errors {
            if error.severity == ndp_lib::validate::Severity::Warning {
                result.add_warning(error);
            } else {
                result.add_error(error);
            }
        }
    }

    Ok(result)
}

// =============================================================================
// Domain validation
// =============================================================================

fn run_validate_domain(
    domain_path: &Path,
    args: &ValidateArgs,
    format: OutputFormat,
    base_config_dir: &Path,
) -> i32 {
    match validate_domain_file(domain_path, args, base_config_dir) {
        Ok(result) => {
            output_result(&result, format);
            determine_exit_code(&result, args.strict)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            exit_codes::SYSTEM_ERROR
        }
    }
}

fn run_validate_all_domains(
    base_config_dir: &Path,
    args: &ValidateArgs,
    format: OutputFormat,
) -> i32 {
    // Domains live at config/domains, which is a sibling of config/base
    let domains_dir = base_config_dir
        .parent()
        .unwrap_or(base_config_dir)
        .join("domains");

    if !domains_dir.exists() {
        eprintln!("Domains directory not found: {}", domains_dir.display());
        return exit_codes::SYSTEM_ERROR;
    }

    let mut results = Vec::new();

    match std::fs::read_dir(&domains_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let domain_json = path.join("domain.json");
                    if domain_json.exists() {
                        match validate_domain_file(&domain_json, args, base_config_dir) {
                            Ok(result) => results.push(result),
                            Err(e) => {
                                let mut result =
                                    ValidationResult::new(domain_json.display().to_string());
                                result.add_error(
                                    ndp_lib::validate::ValidationError::semantic_error(
                                        ndp_lib::validate::ErrorCode::InvalidDomainStream,
                                        "$",
                                        format!("Failed to validate: {}", e),
                                    ),
                                );
                                results.push(result);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error reading domains directory: {}", e);
            return exit_codes::SYSTEM_ERROR;
        }
    }

    if results.is_empty() {
        eprintln!("No domain.json files found in {}", domains_dir.display());
        return exit_codes::SYSTEM_ERROR;
    }

    let batch = BatchValidationResult::from_results(results);
    output_batch_result(&batch, format);
    determine_batch_exit_code(&batch, args.strict)
}

/// Validate a single domain configuration file.
fn validate_domain_file(
    domain_path: &Path,
    args: &ValidateArgs,
    base_config_dir: &Path,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    if !domain_path.exists() {
        return Err(format!("Domain config file not found: {}", domain_path.display()).into());
    }

    let mut result = ValidationResult::new(domain_path.display().to_string());

    // Read file content
    let content = std::fs::read_to_string(domain_path)?;

    // Parse as JSON
    let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        result.add_error(ndp_lib::validate::ValidationError::syntax_error(
            e.line(),
            e.column(),
            &e.to_string(),
        ));
        format!("JSON parse error: {}", e)
    })?;

    // Layer 1: Schema validation
    // When no explicit --domain-schema-path is given, try loading from disk first
    // (config/schemas/domain.schema.json), falling back to the embedded default.
    let schema_validator = if let Some(ref schema_path) = args.domain_schema_path {
        if schema_path.exists() {
            DomainSchemaValidator::from_file(schema_path)?
        } else {
            DomainSchemaValidator::default_schema()?
        }
    } else {
        let default_path = std::path::Path::new("config/schemas/domain.schema.json");
        if default_path.exists() {
            DomainSchemaValidator::from_file(default_path)?
        } else {
            DomainSchemaValidator::default_schema()?
        }
    };

    let schema_errors = schema_validator.validate_schema(&value);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation (unless --schema-only)
    if !args.schema_only {
        let streams_dir = base_config_dir.join("streams");
        let streams_dir_ref = if streams_dir.exists() {
            Some(streams_dir.as_path())
        } else {
            None
        };

        let semantic_errors = validate::validate_domain_semantic(&value, streams_dir_ref);
        for error in semantic_errors {
            if error.severity == ndp_lib::validate::Severity::Warning {
                result.add_warning(error);
            } else {
                result.add_error(error);
            }
        }
    }

    Ok(result)
}

// =============================================================================
// Output helpers
// =============================================================================

fn output_result(result: &ValidationResult, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!("{}", output_json(result)),
        OutputFormat::Human => print!("{}", output_human(result)),
    }
}

fn output_batch_result(batch: &BatchValidationResult, format: OutputFormat) {
    match format {
        OutputFormat::Json => println!("{}", output_json_batch(batch)),
        OutputFormat::Human => print!("{}", output_human_batch(batch)),
    }
}
