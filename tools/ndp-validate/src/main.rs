//! ndp-validate CLI entry point
//!
//! Two-layer config validation tool for NDP stream and domain configurations.
//!
//! Exit codes per dp-019 specification:
//! - 0: Validation passed (may have warnings)
//! - 1: Validation failed (has errors)
//! - 2: System error (file not found, schema load failed, etc.)

use clap::Parser;
use std::path::Path;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

use ndp_validate::cli::{
    determine_batch_exit_code, determine_exit_code, exit_codes, output_human, output_human_batch,
    output_json, output_json_batch, BatchValidationResult, Cli, OutputFormat, ValidationResult,
};
use ndp_validate::error::ValidationError;
use ndp_validate::schema::DomainSchemaValidator;
use ndp_validate::schema_gen;
use ndp_validate::semantic::validate_domain_semantic;

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Validate CLI arguments
    if let Err(e) = cli.validate_args() {
        eprintln!("Error: {}", e);
        return ExitCode::from(exit_codes::SYSTEM_ERROR as u8);
    }

    // Handle schema generation mode
    if cli.generate_schema {
        return handle_generate_schema(&cli);
    }

    // Handle schema verification mode
    if let Some(ref schema_path) = cli.verify_schema {
        return handle_verify_schema(schema_path);
    }

    // Handle domain validation mode
    if cli.is_domain_mode() {
        return run_domain_validation(&cli).await;
    }

    // Run validation
    match run_validation(&cli).await {
        Ok(result) => {
            // Output results
            match cli.format {
                OutputFormat::Json => println!("{}", output_json(&result)),
                OutputFormat::Human => print!("{}", output_human(&result)),
            }

            // Determine exit code
            let code = determine_exit_code(&result, cli.strict);
            ExitCode::from(code as u8)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
        }
    }
}

/// Run validation on the specified config(s)
async fn run_validation(cli: &Cli) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    let config_path = cli.config_path.as_ref().ok_or("No config path specified")?;

    // Check file exists
    if !config_path.exists() {
        return Err(format!("Config file not found: {}", config_path.display()).into());
    }

    let mut result = ValidationResult::new(config_path.display().to_string());

    // Read and parse the file
    let content = std::fs::read_to_string(config_path)?;

    // Determine format (JSON or YAML) and parse
    let value: serde_json::Value = if config_path
        .extension()
        .map(|e| e == "yaml" || e == "yml")
        .unwrap_or(false)
    {
        serde_yaml::from_str(&content).map_err(|e| {
            result.add_error(ValidationError::syntax_error(
                e.location().map(|l| l.line()).unwrap_or(0),
                e.location().map(|l| l.column()).unwrap_or(0),
                &e.to_string(),
            ));
            format!("YAML parse error: {}", e)
        })?
    } else {
        serde_json::from_str(&content).map_err(|e| {
            result.add_error(ValidationError::syntax_error(
                e.line(),
                e.column(),
                &e.to_string(),
            ));
            format!("JSON parse error: {}", e)
        })?
    };

    // Layer 1: Schema validation
    if cli.verbose {
        eprintln!("Running Layer 1 (Schema) validation...");
    }

    let schema_validator = ndp_validate::schema::SchemaValidator::default_schema()?;
    let schema_errors = schema_validator.validate_schema(&value);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation (unless --schema-only)
    if !cli.schema_only {
        if cli.verbose {
            eprintln!("Running Layer 2 (Semantic) validation...");
        }

        // Use SemanticValidator to run all semantic validation rules
        let semantic_validator = ndp_validate::semantic::SemanticValidator::new();
        let semantic_errors = semantic_validator.validate(&value);
        for error in semantic_errors {
            if error.severity == ndp_validate::Severity::Warning {
                result.add_warning(error);
            } else {
                result.add_error(error);
            }
        }
    }

    // Layer 2b: Table existence check (if --check-tables)
    if cli.check_tables && cli.verbose {
        eprintln!("Checking table existence in TimescaleDB...");
    }
    // TODO: Implement table existence check
    // This requires database connection which is out of scope for initial implementation

    Ok(result)
}

/// Handle --generate-schema command
fn handle_generate_schema(cli: &Cli) -> ExitCode {
    match schema_gen::generate_schema() {
        Ok(schema_json) => {
            if let Some(ref output_path) = cli.output {
                // Write to file
                match std::fs::write(output_path, &schema_json) {
                    Ok(()) => {
                        eprintln!("Schema written to {}", output_path.display());
                        ExitCode::from(exit_codes::SUCCESS as u8)
                    }
                    Err(e) => {
                        eprintln!("Error writing schema to file: {}", e);
                        ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
                    }
                }
            } else {
                // Write to stdout
                println!("{}", schema_json);
                ExitCode::from(exit_codes::SUCCESS as u8)
            }
        }
        Err(e) => {
            eprintln!("Error generating schema: {}", e);
            ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
        }
    }
}

/// Handle --verify-schema command
fn handle_verify_schema(schema_path: &std::path::Path) -> ExitCode {
    // Check file exists
    if !schema_path.exists() {
        eprintln!("Schema file not found: {}", schema_path.display());
        return ExitCode::from(exit_codes::SYSTEM_ERROR as u8);
    }

    match schema_gen::verify_schema(schema_path) {
        Ok(true) => {
            eprintln!("Schema verification PASSED");
            eprintln!("Committed schema matches generated schema");
            ExitCode::from(exit_codes::SUCCESS as u8)
        }
        Ok(false) => {
            eprintln!("Schema verification FAILED - drift detected!");
            eprintln!();

            // Show differences
            match schema_gen::compare_schemas(schema_path) {
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
                "  ndp-validate --generate-schema --output {}",
                schema_path.display()
            );

            ExitCode::from(exit_codes::VALIDATION_ERROR as u8)
        }
        Err(e) => {
            eprintln!("Error verifying schema: {}", e);
            ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
        }
    }
}

// =============================================================================
// Domain Validation (FE-002 Phase B)
// =============================================================================

/// Run domain validation based on CLI arguments
async fn run_domain_validation(cli: &Cli) -> ExitCode {
    if let Some(ref domain_path) = cli.domain {
        // Single domain validation
        match validate_single_domain(cli, domain_path).await {
            Ok(result) => {
                // Output results
                match cli.format {
                    OutputFormat::Json => println!("{}", output_json(&result)),
                    OutputFormat::Human => print!("{}", output_human(&result)),
                }
                // Determine exit code
                let code = determine_exit_code(&result, cli.strict);
                ExitCode::from(code as u8)
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
            }
        }
    } else if cli.domain_all {
        // Batch domain validation
        match validate_all_domains(cli).await {
            Ok(batch_result) => {
                // Output results
                match cli.format {
                    OutputFormat::Json => println!("{}", output_json_batch(&batch_result)),
                    OutputFormat::Human => print!("{}", output_human_batch(&batch_result)),
                }
                // Determine exit code
                let code = determine_batch_exit_code(&batch_result, cli.strict);
                ExitCode::from(code as u8)
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
            }
        }
    } else {
        eprintln!("Error: Domain validation requires --domain <path> or --domain-all");
        ExitCode::from(exit_codes::SYSTEM_ERROR as u8)
    }
}

/// Validate a single domain configuration file
async fn validate_single_domain(
    cli: &Cli,
    domain_path: &Path,
) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    // Check file exists
    if !domain_path.exists() {
        return Err(format!("Domain config file not found: {}", domain_path.display()).into());
    }

    let mut result = ValidationResult::new(domain_path.display().to_string());

    // Read and parse the file
    let content = std::fs::read_to_string(domain_path)?;

    // Parse as JSON
    let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        result.add_error(ValidationError::syntax_error(
            e.line(),
            e.column(),
            &e.to_string(),
        ));
        format!("JSON parse error: {}", e)
    })?;

    // Layer 1: Schema validation
    if cli.verbose {
        eprintln!("Running Layer 1 (Schema) validation on domain config...");
    }

    let schema_validator = if cli.domain_schema_path.exists() {
        DomainSchemaValidator::from_file(&cli.domain_schema_path)?
    } else {
        DomainSchemaValidator::default_schema()?
    };

    let schema_errors = schema_validator.validate_schema(&value);
    for error in schema_errors {
        result.add_error(error);
    }

    // Layer 2: Semantic validation (unless --schema-only)
    if !cli.schema_only {
        if cli.verbose {
            eprintln!("Running Layer 2 (Semantic) validation on domain config...");
        }

        let streams_dir = &cli.config_dir;
        let semantic_errors = validate_domain_semantic(&value, Some(streams_dir));
        for error in semantic_errors {
            if error.severity == ndp_validate::Severity::Warning {
                result.add_warning(error);
            } else {
                result.add_error(error);
            }
        }
    }

    Ok(result)
}

/// Validate all domain configurations in the domains directory
async fn validate_all_domains(
    cli: &Cli,
) -> Result<BatchValidationResult, Box<dyn std::error::Error>> {
    let domains_dir = &cli.domains_dir;

    if !domains_dir.exists() {
        return Err(format!("Domains directory not found: {}", domains_dir.display()).into());
    }

    let mut results = Vec::new();

    // Find all domain.json files
    for entry in std::fs::read_dir(domains_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let domain_json = path.join("domain.json");
            if domain_json.exists() {
                if cli.verbose {
                    eprintln!("Validating: {}", domain_json.display());
                }
                match validate_single_domain(cli, &domain_json).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        // Create an error result for this domain
                        let mut result = ValidationResult::new(domain_json.display().to_string());
                        result.add_error(ValidationError::semantic_error(
                            ndp_validate::ErrorCode::InvalidDomainStream,
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
