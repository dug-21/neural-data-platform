//! ndp-validate - Thin wrapper around ndp_lib::validate
//!
//! All validation logic lives in `ndp_lib::validate`. This crate re-exports
//! those types for backward compatibility and provides the CLI binary.
//!
//! ## Exit Codes
//!
//! - 0: Validation passed (may have warnings)
//! - 1: Validation failed (has errors)
//! - 2: System error (file not found, schema load failed, etc.)

// CLI module stays here (Clap struct is CLI-specific)
pub mod cli;

// Re-export all validation types from ndp-lib
pub use ndp_lib::validate::error;
pub use ndp_lib::validate::schema;
pub use ndp_lib::validate::schema_gen;
pub use ndp_lib::validate::semantic;

// Re-export commonly used types at crate root for backward compatibility
pub use ndp_lib::validate::{
    determine_batch_exit_code, determine_exit_code, exit_codes, output_human, output_human_batch,
    output_json, output_json_batch, validate_all_domains, validate_all_streams,
    validate_domain_config, validate_domain_file, validate_stream, validate_stream_file,
    BatchSummary, BatchValidationResult, DomainSchemaValidator, ErrorCode, OutputFormat,
    SchemaValidator, SchemaValidatorError, SemanticValidator, Severity, ValidateOptions,
    ValidationError, ValidationLayer, ValidationResult, ValidationSummary,
};
