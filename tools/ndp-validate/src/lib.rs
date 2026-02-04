//! ndp-validate - Two-layer config validation for NDP streams
//!
//! This tool implements the dp-019 Config Validation Pipeline specification:
//!
//! ## Layer 1: JSON Schema Validation
//! - Structural validation against JSON Schema
//! - Type checking, required fields, enum values
//! - Pattern matching for string formats
//! - Unknown field detection (additionalProperties: false)
//!
//! ## Layer 2: Semantic Validation
//! - Cross-field consistency checks (source_path references)
//! - NDP-specific business rules (valid types, transforms)
//! - DQ rule syntax validation
//! - Levenshtein distance suggestions for typos
//!
//! ## Exit Codes
//!
//! - 0: Validation passed (may have warnings)
//! - 1: Validation failed (has errors)
//! - 2: System error (file not found, schema load failed, etc.)
//!
//! ## Usage
//!
//! ```bash
//! # Validate a single config file
//! ndp-validate config/base/streams/air-quality/config.json
//!
//! # Validate all configs
//! ndp-validate --all
//!
//! # Schema-only validation (fast, no DB needed)
//! ndp-validate --schema-only config/base/streams/air-quality/config.json
//!
//! # Full validation with table existence check
//! ndp-validate --check-tables --timescale-url postgresql://localhost/ndp config.json
//!
//! # Human-readable output
//! ndp-validate --format human --all
//! ```

pub mod cli;
pub mod error;
pub mod schema;
pub mod schema_gen;
pub mod semantic;

// Re-export CLI types
pub use cli::{
    exit_codes, BatchValidationResult, Cli, OutputFormat, ValidationResult, ValidationSummary,
};

// Re-export error types
pub use error::{ErrorCode, Severity, ValidationError, ValidationLayer};

// Re-export schema types
pub use schema::SchemaValidator;

// Re-export schema generation
pub use schema_gen::{compare_schemas, generate_schema, verify_schema, SchemaGenError};
