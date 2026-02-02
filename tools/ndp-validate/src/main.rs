//! ndp-validate CLI entry point
//!
//! Two-layer config validation tool for NDP stream configurations.
//!
//! Exit codes per dp-019 specification:
//! - 0: Validation passed (may have warnings)
//! - 1: Validation failed (has errors)
//! - 2: System error (file not found, schema load failed, etc.)

use clap::Parser;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

use ndp_validate::cli::{
    determine_exit_code, exit_codes, output_human, output_json, Cli, OutputFormat,
    ValidationResult,
};
use ndp_validate::error::ValidationError;
use ndp_validate::schema_gen;

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
    let config_path = cli
        .config_path
        .as_ref()
        .ok_or("No config path specified")?;

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
    if cli.check_tables
        && cli.verbose {
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
