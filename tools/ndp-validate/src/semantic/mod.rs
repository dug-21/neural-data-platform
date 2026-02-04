//! Semantic validation layer for NDP stream configurations
//!
//! This module provides Layer 2 (semantic) validation that verifies
//! application-level rules that cannot be expressed in JSON Schema.
//!
//! ## Semantic Validation Rules
//!
//! - **sources** (FR-020): Validates source type and source-specific required fields:
//!   - `mqtt`: requires `broker_url` and `topics`
//!   - `http_poll`: requires `endpoints` and positive `poll_interval_secs`
//!   - `csv`: requires `path` and `timestamp_field`
//!   - `webhook`, `file_watch`: no additional requirements
//!
//! - **source_path** (FR-022): Validates that silver_etl.field_mappings[].source_path
//!   references fields that exist in config.fields[]. Uses Levenshtein distance
//!   for "did you mean" suggestions. This is the P-005 fix from dp-016.
//!
//! - **table_exists** (FR-023): Validates that Silver target tables exist in TimescaleDB.
//!   Supports graceful degradation when database connection is unavailable.
//!
//! - **dq_rules**: Validates DQ rule syntax, column references, and action compatibility.
//!
//! - **gold** (FE-001): Validates Gold ETL configuration semantics:
//!   - Field references in aggregates/features
//!   - Metric and stat types
//!   - Granularity format
//!   - Transitions on appropriate stream types
//!
//! - **domain** (FE-001): Validates domain configuration semantics:
//!   - Stream references exist
//!   - Unique aliases
//!   - Objective conditions
//!   - Alignment configuration

pub mod domain;
pub mod dq_rules;
pub mod gold;
pub mod source_path;
pub mod sources;
pub mod table_exists;

pub use domain::validate_domain;
pub use dq_rules::validate_dq_rules;
pub use gold::validate_gold_etl;
pub use source_path::validate_source_paths;
pub use sources::validate_sources;
pub use table_exists::{parse_table_reference, validate_table_exists};

// Re-export SemanticValidator for public use
pub use self::SemanticValidator as Validator;

use crate::error::ValidationError;
use serde_json::Value;
use std::collections::HashSet;

/// Semantic validator that coordinates all Layer 2 validation rules
#[derive(Debug, Default)]
pub struct SemanticValidator;

impl SemanticValidator {
    /// Create a new semantic validator
    pub fn new() -> Self {
        Self
    }

    /// Validate a stream configuration JSON value
    ///
    /// Runs all semantic validation rules:
    /// - FR-020: Source configuration validation
    /// - FR-022: Source path cross-reference validation
    /// - FR-023: Table existence validation (graceful degradation)
    /// - DQ rules validation
    /// - FE-001: Gold ETL validation
    pub fn validate(&self, config: &Value) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // FR-020: Validate sources
        if let Some(sources) = config.get("sources").and_then(|v| v.as_array()) {
            errors.extend(validate_sources(sources));
        }

        // FR-022: Validate source_path references
        // Extract field names from config.fields[]
        let field_names: HashSet<String> = config
            .get("fields")
            .and_then(|v| v.as_array())
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|f| f.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // Extract field_mappings from silver_etl
        if let Some(silver_etl) = config.get("silver_etl") {
            if let Some(mappings) = silver_etl.get("field_mappings").and_then(|v| v.as_array()) {
                let field_mappings: Vec<(usize, String)> = mappings
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, m)| {
                        m.get("source_path")
                            .and_then(|v| v.as_str())
                            .map(|s| (idx, s.to_string()))
                    })
                    .collect();

                errors.extend(validate_source_paths(&field_names, &field_mappings));
            }

            // FR-023: Validate target table exists (graceful degradation without DB)
            if let Some(target_table) = silver_etl.get("target_table").and_then(|v| v.as_str()) {
                errors.extend(validate_table_exists(target_table, None));
            }

            // Validate DQ rules
            // Extract silver column names from field_mappings
            let silver_columns: HashSet<String> = silver_etl
                .get("field_mappings")
                .and_then(|v| v.as_array())
                .map(|mappings| {
                    mappings
                        .iter()
                        .filter_map(|m| m.get("target_column").and_then(|n| n.as_str()))
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

            if let Some(dq_rules_json) = silver_etl.get("dq_rules").and_then(|v| v.as_array()) {
                // Parse DQ rules from JSON into typed DqRule structs
                let dq_rules: Vec<dq_rules::DqRule> = dq_rules_json
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();

                errors.extend(validate_dq_rules(&dq_rules, &silver_columns));
            }
        }

        // FE-001: Validate Gold ETL configuration
        errors.extend(validate_gold_etl(config));

        errors
    }
}
