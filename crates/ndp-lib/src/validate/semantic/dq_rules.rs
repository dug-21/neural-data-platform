//! DQ Rule Semantic Validation
//!
//! Validates data quality rules according to the NDP DQ Framework.
//! Supports all 11 rule types as defined in DQ-VALIDATION-RESEARCH.md.
//!
//! # Rule Types
//!
//! - Value-Level: range_check, null_check, enum_check, pattern_check
//! - Temporal: freshness_check, monotonic_check, rate_of_change
//! - Cross-Field: cross_field_check, conditional_check
//! - Batch-Level: completeness_check, cardinality_check
//!
//! # Error Codes
//!
//! - INVALID_DQ_RULE_TYPE: Unknown DQ rule type
//! - INVALID_DQ_RULE: Rule-specific validation failure
//! - INVALID_DQ_ACTION: Action not valid for rule type
//! - INVALID_DQ_COLUMN: DQ rule references unknown column
//! - INVALID_DQ_SYNTAX: Invalid SQL expression
//! - INVALID_REGEX: Invalid regex pattern
//! - INVALID_INTERVAL: Invalid interval format

use crate::validate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use std::collections::HashSet;

// BUG-001-fix: Import DQ types from ndp-types (single source of truth)
use ndp_types::{DqAction, DqRuleType};

// =============================================================================
// DQ Rule Types (serde-compatible for JSON config parsing)
// =============================================================================

/// Get supported DQ rule types from ndp-types (single source of truth)
///
/// This function returns the authoritative list of supported DQ rule types
/// from ndp-types, eliminating the risk of drift between validation and runtime.
fn supported_dq_rules() -> &'static [&'static str] {
    DqRuleType::all_names()
}

/// Get supported DQ actions from ndp-types (single source of truth)
///
/// This function returns the authoritative list of supported DQ actions
/// from ndp-types, eliminating the risk of drift between validation and runtime.
#[allow(dead_code)] // Reserved for future action validation
fn supported_actions() -> &'static [&'static str] {
    DqAction::all_names()
}

/// Action compatibility matrix - maps rule types to valid actions
pub fn get_valid_actions(rule_type: &str) -> &'static [&'static str] {
    match rule_type {
        "range_check" => &["flag", "reject", "clamp"],
        "null_check" => &["flag", "reject"],
        "enum_check" => &["flag", "reject"],
        "pattern_check" => &["flag", "reject"],
        "freshness_check" => &["flag", "reject"],
        "monotonic_check" => &["flag"],
        "rate_of_change" => &["flag"],
        "cross_field_check" => &["flag", "reject"],
        "conditional_check" => &["flag", "reject"],
        "completeness_check" => &["warn", "flag"],
        "cardinality_check" => &["warn", "flag"],
        _ => &[],
    }
}

// =============================================================================
// DQ Rule Representation
// =============================================================================

/// Generic DQ rule from JSON config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DqRule {
    /// Rule type (e.g., "range_check", "null_check")
    pub rule: String,

    /// Field this rule applies to (optional for cross-field rules)
    #[serde(default)]
    pub field: Option<String>,

    /// Action to take on violation
    #[serde(default)]
    pub action: Option<String>,

    // range_check specific
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub clamp_to_bounds: Option<bool>,

    // enum_check specific
    #[serde(default)]
    pub allowed_values: Option<Vec<String>>,
    #[serde(default)]
    pub case_sensitive: Option<bool>,

    // pattern_check specific
    #[serde(default)]
    pub pattern: Option<String>,

    // freshness_check specific
    #[serde(default)]
    pub max_age: Option<String>,
    #[serde(default)]
    pub max_future: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,

    // monotonic_check specific
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub partition_by: Option<Vec<String>>,
    #[serde(default)]
    pub allow_reset: Option<bool>,
    #[serde(default)]
    pub reset_threshold: Option<f64>,

    // rate_of_change specific
    #[serde(default)]
    pub max_change_per_minute: Option<f64>,

    // cross_field_check / conditional_check specific
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub expression: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub then_rule: Option<Box<DqRule>>,

    // completeness_check / cardinality_check specific
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub min_completeness: Option<f64>,
    #[serde(default)]
    pub expected_range: Option<(i32, i32)>,
}

// =============================================================================
// Validation Functions
// =============================================================================

/// Validate all DQ rules in a configuration
///
/// # Arguments
///
/// * `rules` - Array of DQ rules to validate
/// * `silver_columns` - Set of valid Silver column names
///
/// # Returns
///
/// Vector of validation errors (empty if all rules are valid)
pub fn validate_dq_rules(
    rules: &[DqRule],
    silver_columns: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut rule_names: HashSet<String> = HashSet::new();

    for (idx, rule) in rules.iter().enumerate() {
        let base_path = format!("$.silver_etl.dq_rules[{}]", idx);

        // Validate rule type
        if !supported_dq_rules().contains(&rule.rule.as_str()) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRuleType,
                path: format!("{}.rule", base_path),
                message: format!(
                    "DQ rule type '{}' is not supported. Must be one of: {}",
                    rule.rule,
                    supported_dq_rules().join(", ")
                ),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
            continue;
        }

        // Validate action compatibility
        if let Some(action) = &rule.action {
            let valid_actions = get_valid_actions(&rule.rule);
            if !valid_actions.contains(&action.as_str()) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidDqAction,
                    path: format!("{}.action", base_path),
                    message: format!(
                        "Action '{}' is not valid for rule type '{}'. Valid actions: {}",
                        action,
                        rule.rule,
                        valid_actions.join(", ")
                    ),
                    severity: Severity::Error,
                    suggestion: None,
                    context: None,
                });
            }
        }

        // Validate field reference (for field-based rules)
        if let Some(field) = &rule.field {
            if rule.rule != "cross_field_check"
                && rule.rule != "conditional_check"
                && !silver_columns.contains(field)
            {
                let suggestion = find_closest_match(field, silver_columns);
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidDqColumn,
                    path: format!("{}.field", base_path),
                    message: format!(
                        "DQ rule references unknown column '{}'. Available columns: {}",
                        field,
                        silver_columns
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    severity: Severity::Error,
                    suggestion,
                    context: None,
                });
            }
        }

        // Rule-specific validation
        let rule_errors = match rule.rule.as_str() {
            "range_check" => validate_range_check(rule, &base_path),
            "enum_check" => validate_enum_check(rule, &base_path),
            "pattern_check" => validate_pattern_check(rule, &base_path),
            "freshness_check" => validate_freshness_check(rule, &base_path),
            "monotonic_check" => validate_monotonic_check(rule, &base_path, silver_columns),
            "rate_of_change" => validate_rate_of_change(rule, &base_path, silver_columns),
            "cross_field_check" => {
                validate_cross_field_check(rule, &base_path, silver_columns, &mut rule_names)
            }
            "conditional_check" => {
                validate_conditional_check(rule, &base_path, silver_columns, &mut rule_names)
            }
            "completeness_check" => validate_completeness_check(rule, &base_path),
            "cardinality_check" => validate_cardinality_check(rule, &base_path),
            _ => vec![],
        };

        errors.extend(rule_errors);
    }

    errors
}

/// Validate DQ rules from JSON values
///
/// This wrapper function parses JSON values into DqRule structs and then
/// validates them using the main validate_dq_rules function.
///
/// # Arguments
///
/// * `rules` - Array of JSON values representing DQ rules
/// * `silver_columns` - Set of valid Silver column names
///
/// # Returns
///
/// Vector of validation errors (empty if all rules are valid)
pub fn validate_dq_rules_from_json(
    rules: &[Value],
    silver_columns: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut parsed_rules = Vec::new();

    for (idx, rule_value) in rules.iter().enumerate() {
        let base_path = format!("$.silver_etl.dq_rules[{}]", idx);

        match serde_json::from_value::<DqRule>(rule_value.clone()) {
            Ok(rule) => parsed_rules.push(rule),
            Err(e) => {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidDqRule,
                    path: base_path,
                    message: format!("Failed to parse DQ rule: {}", e),
                    severity: Severity::Error,
                    suggestion: None,
                    context: None,
                });
            }
        }
    }

    // If there were parsing errors, return them immediately
    if !errors.is_empty() {
        return errors;
    }

    // Validate the successfully parsed rules
    validate_dq_rules(&parsed_rules, silver_columns)
}

// =============================================================================
// Rule-Specific Validators
// =============================================================================

/// Validate range_check rule
fn validate_range_check(rule: &DqRule, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // At least one of min or max required
    if rule.min.is_none() && rule.max.is_none() {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: base_path.to_string(),
            message: "range_check requires at least one of 'min' or 'max'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
        return errors;
    }

    // If both specified, min must be less than max
    if let (Some(min), Some(max)) = (rule.min, rule.max) {
        if min >= max {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: base_path.to_string(),
                message: format!("range_check min ({}) must be less than max ({})", min, max),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
    }

    // clamp_to_bounds only valid with clamp action
    if rule.clamp_to_bounds == Some(true) && rule.action.as_deref() != Some("clamp") {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.clamp_to_bounds", base_path),
            message: "clamp_to_bounds is only valid when action is 'clamp'".to_string(),
            severity: Severity::Warning,
            suggestion: None,
            context: None,
        });
    }

    errors
}

/// Validate enum_check rule
fn validate_enum_check(rule: &DqRule, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match &rule.allowed_values {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.allowed_values", base_path),
                message: "enum_check requires non-empty 'allowed_values' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(values) if values.is_empty() => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.allowed_values", base_path),
                message: "enum_check requires non-empty 'allowed_values' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        _ => {}
    }

    errors
}

/// Validate pattern_check rule
fn validate_pattern_check(rule: &DqRule, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let pattern_missing = match &rule.pattern {
        None => true,
        Some(s) if s.is_empty() => true,
        _ => false,
    };

    if pattern_missing {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.pattern", base_path),
            message: "pattern_check requires a 'pattern' regex".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    } else if let Some(pattern) = &rule.pattern {
        // Validate regex syntax
        if let Err(e) = Regex::new(pattern) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidRegex,
                path: format!("{}.pattern", base_path),
                message: format!("Invalid regex pattern: {}", e),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
    }

    errors
}

/// Validate freshness_check rule
fn validate_freshness_check(rule: &DqRule, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // At least one of max_age or max_future should be specified
    if rule.max_age.is_none() && rule.max_future.is_none() {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: base_path.to_string(),
            message: "freshness_check should have 'max_age' or 'max_future'".to_string(),
            severity: Severity::Warning,
            suggestion: None,
            context: None,
        });
    }

    // Validate interval format for max_age
    if let Some(interval) = &rule.max_age {
        if !is_valid_interval(interval) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidInterval,
                path: format!("{}.max_age", base_path),
                message: format!(
                    "Invalid interval '{}'. Examples: '2 hours', '30 minutes', '1 day'",
                    interval
                ),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
    }

    // Validate interval format for max_future
    if let Some(interval) = &rule.max_future {
        if !is_valid_interval(interval) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidInterval,
                path: format!("{}.max_future", base_path),
                message: format!(
                    "Invalid interval '{}'. Examples: '5 minutes', '1 hour'",
                    interval
                ),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
    }

    errors
}

/// Validate monotonic_check rule
fn validate_monotonic_check(
    rule: &DqRule,
    base_path: &str,
    silver_columns: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // direction is required
    if rule.direction.is_none() {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.direction", base_path),
            message:
                "monotonic_check requires 'direction' (increasing|decreasing|strict_increasing)"
                    .to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    } else if let Some(dir) = &rule.direction {
        if !["increasing", "decreasing", "strict_increasing"].contains(&dir.as_str()) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.direction", base_path),
                message: format!(
                    "Invalid direction '{}'. Must be one of: increasing, decreasing, strict_increasing",
                    dir
                ),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
    }

    // partition_by is required
    match &rule.partition_by {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.partition_by", base_path),
                message: "monotonic_check requires 'partition_by' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(cols) if cols.is_empty() => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.partition_by", base_path),
                message: "monotonic_check requires non-empty 'partition_by' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(cols) => {
            // Validate partition columns exist
            for col in cols {
                if !silver_columns.contains(col) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidDqColumn,
                        path: format!("{}.partition_by", base_path),
                        message: format!("Unknown partition column '{}'", col),
                        severity: Severity::Error,
                        suggestion: find_closest_match(col, silver_columns),
                        context: None,
                    });
                }
            }
        }
    }

    errors
}

/// Validate rate_of_change rule
fn validate_rate_of_change(
    rule: &DqRule,
    base_path: &str,
    silver_columns: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // max_change_per_minute is required and must be positive
    match rule.max_change_per_minute {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.max_change_per_minute", base_path),
                message: "rate_of_change requires 'max_change_per_minute'".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(val) if val <= 0.0 => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.max_change_per_minute", base_path),
                message: "max_change_per_minute must be positive".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        _ => {}
    }

    // partition_by is required
    match &rule.partition_by {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.partition_by", base_path),
                message: "rate_of_change requires 'partition_by' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(cols) if cols.is_empty() => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.partition_by", base_path),
                message: "rate_of_change requires non-empty 'partition_by' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(cols) => {
            // Validate partition columns exist
            for col in cols {
                if !silver_columns.contains(col) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidDqColumn,
                        path: format!("{}.partition_by", base_path),
                        message: format!("Unknown partition column '{}'", col),
                        severity: Severity::Error,
                        suggestion: find_closest_match(col, silver_columns),
                        context: None,
                    });
                }
            }
        }
    }

    errors
}

/// Validate cross_field_check rule
fn validate_cross_field_check(
    rule: &DqRule,
    base_path: &str,
    silver_columns: &HashSet<String>,
    rule_names: &mut HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Name is required and must be unique
    let name_missing = match &rule.name {
        None => true,
        Some(s) if s.is_empty() => true,
        _ => false,
    };

    if name_missing {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.name", base_path),
            message: "cross_field_check requires a 'name'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    } else if let Some(name) = &rule.name {
        if rule_names.contains(name) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::DuplicateName,
                path: format!("{}.name", base_path),
                message: format!("Duplicate DQ rule name '{}'", name),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        } else {
            rule_names.insert(name.clone());
        }
    }

    // Expression is required
    let expr_missing = match &rule.expression {
        None => true,
        Some(s) if s.is_empty() => true,
        _ => false,
    };

    if expr_missing {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.expression", base_path),
            message: "cross_field_check requires an 'expression'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    } else if let Some(expression) = &rule.expression {
        // Parse and validate SQL expression
        errors.extend(validate_sql_expression(
            expression,
            silver_columns,
            &format!("{}.expression", base_path),
        ));
    }

    errors
}

/// Validate conditional_check rule
fn validate_conditional_check(
    rule: &DqRule,
    base_path: &str,
    silver_columns: &HashSet<String>,
    rule_names: &mut HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Name is required and must be unique
    let name_missing = match &rule.name {
        None => true,
        Some(s) if s.is_empty() => true,
        _ => false,
    };

    if name_missing {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.name", base_path),
            message: "conditional_check requires a 'name'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    } else if let Some(name) = &rule.name {
        if rule_names.contains(name) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::DuplicateName,
                path: format!("{}.name", base_path),
                message: format!("Duplicate DQ rule name '{}'", name),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        } else {
            rule_names.insert(name.clone());
        }
    }

    // Condition is required
    let cond_missing = match &rule.condition {
        None => true,
        Some(s) if s.is_empty() => true,
        _ => false,
    };

    if cond_missing {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.condition", base_path),
            message: "conditional_check requires a 'condition'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    } else if let Some(condition) = &rule.condition {
        // Validate condition as SQL expression
        errors.extend(validate_sql_expression(
            condition,
            silver_columns,
            &format!("{}.condition", base_path),
        ));
    }

    // then_rule is required and must be recursively validated
    match &rule.then_rule {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.then_rule", base_path),
                message: "conditional_check requires a 'then_rule'".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(then_rule) => {
            // Recursively validate the nested rule
            let nested_errors = validate_dq_rules(&[*then_rule.clone()], silver_columns);
            // Update paths for nested errors
            for mut err in nested_errors {
                err.path = err.path.replace(
                    "$.silver_etl.dq_rules[0]",
                    &format!("{}.then_rule", base_path),
                );
                errors.push(err);
            }
        }
    }

    errors
}

/// Validate completeness_check rule
fn validate_completeness_check(rule: &DqRule, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // level must be "batch"
    if rule.level.as_deref() != Some("batch") {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.level", base_path),
            message: "completeness_check requires level: 'batch'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    }

    // min_completeness must be 0.0-1.0
    match rule.min_completeness {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.min_completeness", base_path),
                message: "completeness_check requires 'min_completeness'".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some(val) if !(0.0..=1.0).contains(&val) => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.min_completeness", base_path),
                message: format!("min_completeness must be between 0.0 and 1.0, got {}", val),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        _ => {}
    }

    errors
}

/// Validate cardinality_check rule
fn validate_cardinality_check(rule: &DqRule, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // level must be "batch"
    if rule.level.as_deref() != Some("batch") {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDqRule,
            path: format!("{}.level", base_path),
            message: "cardinality_check requires level: 'batch'".to_string(),
            severity: Severity::Error,
            suggestion: None,
            context: None,
        });
    }

    // expected_range must be [min, max] with min <= max
    match rule.expected_range {
        None => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.expected_range", base_path),
                message: "cardinality_check requires 'expected_range' array".to_string(),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Some((min, max)) if min > max => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqRule,
                path: format!("{}.expected_range", base_path),
                message: format!(
                    "expected_range[0] ({}) must be <= expected_range[1] ({})",
                    min, max
                ),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        _ => {}
    }

    errors
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Validate SQL expression syntax and column references
fn validate_sql_expression(
    expression: &str,
    valid_columns: &HashSet<String>,
    path: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Wrap expression in SELECT to make it valid SQL
    let sql = format!("SELECT {} AS result", expression);

    // Parse SQL
    let dialect = PostgreSqlDialect {};
    match Parser::parse_sql(&dialect, &sql) {
        Err(e) => {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDqSyntax,
                path: path.to_string(),
                message: format!("Invalid SQL expression: {}", e),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
        Ok(ast) => {
            // Extract column references from AST
            let referenced_columns = extract_column_references(&ast);

            // Validate all referenced columns exist
            for col in referenced_columns {
                // Skip known SQL keywords/functions that might be parsed as identifiers
                let lower_col = col.to_lowercase();
                if ["null", "true", "false", "and", "or", "not", "is"].contains(&lower_col.as_str())
                {
                    continue;
                }

                if !valid_columns.contains(&col) {
                    let suggestion = find_closest_match(&col, valid_columns);
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidDqColumn,
                        path: path.to_string(),
                        message: format!("Unknown column '{}' in expression", col),
                        severity: Severity::Error,
                        suggestion,
                        context: None,
                    });
                }
            }
        }
    }

    errors
}

/// Extract column references from parsed SQL AST
fn extract_column_references(ast: &[sqlparser::ast::Statement]) -> Vec<String> {
    use sqlparser::ast::{SelectItem, SetExpr, Statement};

    let mut columns = Vec::new();

    for stmt in ast {
        if let Statement::Query(query) = stmt {
            if let SetExpr::Select(select) = query.body.as_ref() {
                for item in &select.projection {
                    if let SelectItem::ExprWithAlias { expr, .. } = item {
                        extract_columns_from_expr(expr, &mut columns);
                    }
                }
            }
        }
    }

    columns
}

/// Recursively extract column names from an expression
fn extract_columns_from_expr(expr: &sqlparser::ast::Expr, columns: &mut Vec<String>) {
    use sqlparser::ast::Expr;

    match expr {
        Expr::Identifier(ident) => {
            columns.push(ident.value.clone());
        }
        Expr::CompoundIdentifier(idents) => {
            // Take the last part as the column name
            if let Some(last) = idents.last() {
                columns.push(last.value.clone());
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            extract_columns_from_expr(left, columns);
            extract_columns_from_expr(right, columns);
        }
        Expr::UnaryOp { expr, .. } => {
            extract_columns_from_expr(expr, columns);
        }
        Expr::Nested(inner) => {
            extract_columns_from_expr(inner, columns);
        }
        Expr::IsNull(inner) | Expr::IsNotNull(inner) => {
            extract_columns_from_expr(inner, columns);
        }
        Expr::Function(func) => {
            if let sqlparser::ast::FunctionArguments::List(args_list) = &func.args {
                for arg in &args_list.args {
                    if let sqlparser::ast::FunctionArg::Unnamed(
                        sqlparser::ast::FunctionArgExpr::Expr(e),
                    ) = arg
                    {
                        extract_columns_from_expr(e, columns);
                    }
                }
            }
        }
        Expr::Case {
            operand,
            conditions,
            results,
            else_result,
        } => {
            if let Some(op) = operand {
                extract_columns_from_expr(op, columns);
            }
            for cond in conditions {
                extract_columns_from_expr(cond, columns);
            }
            for result in results {
                extract_columns_from_expr(result, columns);
            }
            if let Some(else_r) = else_result {
                extract_columns_from_expr(else_r, columns);
            }
        }
        _ => {}
    }
}

/// Validate PostgreSQL interval format
fn is_valid_interval(interval: &str) -> bool {
    // Valid patterns: "N unit", "N unit N unit"
    // Units: seconds, minutes, hours, days, weeks, months, years
    let pattern = Regex::new(
        r"(?i)^\d+\s+(seconds?|sec|s|minutes?|min|m|hours?|h|days?|d|weeks?|w|months?|years?)(\s+\d+\s+(seconds?|sec|minutes?|min|m|hours?|h|days?|d))?$"
    ).unwrap();

    pattern.is_match(interval)
}

/// Find the closest matching string using Levenshtein distance
fn find_closest_match(input: &str, candidates: &HashSet<String>) -> Option<String> {
    let input_lower = input.to_lowercase();
    let mut min_distance = usize::MAX;
    let mut closest = None;

    for candidate in candidates {
        let distance = levenshtein_distance(&input_lower, &candidate.to_lowercase());
        if distance < min_distance && distance <= 3 {
            // Max 3 edits
            min_distance = distance;
            closest = Some(format!("Did you mean '{}'?", candidate));
        }
    }

    closest
}

/// Calculate Levenshtein distance between two strings
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b_len + 1) {
        *cell = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

// =============================================================================
// Tests - London School TDD (Tests FIRST)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_columns(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // =========================================================================
    // Test 1: Valid range_check with min and max
    // =========================================================================
    #[test]
    fn test_range_check_valid_min_max() {
        let columns = make_columns(&["pm25", "pm10", "temperature_c"]);
        let rules = vec![DqRule {
            rule: "range_check".to_string(),
            field: Some("pm25".to_string()),
            action: Some("flag".to_string()),
            min: Some(0.0),
            max: Some(1000.0),
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(
            errors.is_empty(),
            "Valid range_check should have no errors: {:?}",
            errors
        );
    }

    // =========================================================================
    // Test 2: range_check with min greater than max fails
    // =========================================================================
    #[test]
    fn test_range_check_min_greater_than_max_fails() {
        let columns = make_columns(&["pm25"]);
        let rules = vec![DqRule {
            rule: "range_check".to_string(),
            field: Some("pm25".to_string()),
            action: Some("flag".to_string()),
            min: Some(1000.0), // min > max
            max: Some(0.0),
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDqRule);
        assert!(errors[0].message.contains("must be less than max"));
    }

    // =========================================================================
    // Test 3: range_check missing both min and max fails
    // =========================================================================
    #[test]
    fn test_range_check_missing_both_min_max_fails() {
        let columns = make_columns(&["pm25"]);
        let rules = vec![DqRule {
            rule: "range_check".to_string(),
            field: Some("pm25".to_string()),
            action: Some("flag".to_string()),
            min: None, // Neither min nor max
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDqRule);
        assert!(errors[0].message.contains("at least one of 'min' or 'max'"));
    }

    // =========================================================================
    // Test 4: enum_check requires allowed_values
    // =========================================================================
    #[test]
    fn test_enum_check_requires_allowed_values() {
        let columns = make_columns(&["wind_direction"]);
        let rules = vec![DqRule {
            rule: "enum_check".to_string(),
            field: Some("wind_direction".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: Some(vec![]), // Empty array fails
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDqRule);
        assert!(errors[0].message.contains("non-empty 'allowed_values'"));
    }

    // =========================================================================
    // Test 5: pattern_check with invalid regex fails
    // =========================================================================
    #[test]
    fn test_pattern_check_invalid_regex_fails() {
        let columns = make_columns(&["device_serial"]);
        let rules = vec![DqRule {
            rule: "pattern_check".to_string(),
            field: Some("device_serial".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: Some("[invalid(regex".to_string()), // Unclosed bracket
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidRegex);
        assert!(errors[0].message.contains("Invalid regex pattern"));
    }

    // =========================================================================
    // Test 6: freshness_check with invalid interval fails
    // =========================================================================
    #[test]
    fn test_freshness_check_invalid_interval_fails() {
        let columns = make_columns(&["observation_time"]);
        let rules = vec![DqRule {
            rule: "freshness_check".to_string(),
            field: Some("observation_time".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: Some("2 hoursss".to_string()), // Typo
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidInterval);
        assert!(errors[0].message.contains("Invalid interval"));
    }

    // =========================================================================
    // Test 7: cross_field_check with invalid SQL fails
    // =========================================================================
    #[test]
    fn test_cross_field_check_invalid_sql_fails() {
        let columns = make_columns(&["pm25", "pm10"]);
        let rules = vec![DqRule {
            rule: "cross_field_check".to_string(),
            field: None,
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: Some("bad_syntax".to_string()),
            expression: Some("pm25 >= AND pm10".to_string()), // Invalid SQL
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(errors.iter().any(|e| e.code == ErrorCode::InvalidDqSyntax));
    }

    // =========================================================================
    // Test 8: cross_field_check with unknown column fails
    // =========================================================================
    #[test]
    fn test_cross_field_check_unknown_column_fails() {
        let columns = make_columns(&["pm25", "pm10"]);
        let rules = vec![DqRule {
            rule: "cross_field_check".to_string(),
            field: None,
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: Some("bad_col".to_string()),
            expression: Some("typo_col >= 0".to_string()), // Unknown column
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(errors.iter().any(|e| e.code == ErrorCode::InvalidDqColumn));
        assert!(errors.iter().any(|e| e.message.contains("typo_col")));
    }

    // =========================================================================
    // Test 9: completeness_check min_completeness out of range fails
    // =========================================================================
    #[test]
    fn test_completeness_check_min_out_of_range_fails() {
        let columns = make_columns(&["pm25"]);
        let rules = vec![DqRule {
            rule: "completeness_check".to_string(),
            field: Some("pm25".to_string()),
            action: Some("warn".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: Some("batch".to_string()),
            min_completeness: Some(1.5), // > 1.0
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(errors.iter().any(|e| e.code == ErrorCode::InvalidDqRule));
        assert!(errors
            .iter()
            .any(|e| e.message.contains("between 0.0 and 1.0")));
    }

    // =========================================================================
    // Test 10: action compatibility matrix - clamp only for range_check
    // =========================================================================
    #[test]
    fn test_action_compatibility_matrix() {
        let columns = make_columns(&["pm25"]);

        // clamp action valid for range_check
        let rules_valid = vec![DqRule {
            rule: "range_check".to_string(),
            field: Some("pm25".to_string()),
            action: Some("clamp".to_string()),
            min: Some(0.0),
            max: Some(100.0),
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];
        let errors = validate_dq_rules(&rules_valid, &columns);
        assert!(errors.is_empty(), "clamp should be valid for range_check");

        // clamp action invalid for null_check
        let rules_invalid = vec![DqRule {
            rule: "null_check".to_string(),
            field: Some("pm25".to_string()),
            action: Some("clamp".to_string()), // Invalid for null_check
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];
        let errors = validate_dq_rules(&rules_invalid, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDqAction);
        assert!(errors[0].message.contains("not valid for rule type"));
    }

    // =========================================================================
    // Test 11: Unknown rule type fails
    // =========================================================================
    #[test]
    fn test_unknown_rule_type_fails() {
        let columns = make_columns(&["pm25"]);
        let rules = vec![DqRule {
            rule: "unknown_check".to_string(), // Invalid rule type
            field: Some("pm25".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDqRuleType);
    }

    // =========================================================================
    // Test 12: Unknown field reference fails
    // =========================================================================
    #[test]
    fn test_unknown_field_reference_fails() {
        let columns = make_columns(&["pm25", "pm10"]);
        let rules = vec![DqRule {
            rule: "range_check".to_string(),
            field: Some("typo_field".to_string()), // Unknown field
            action: Some("flag".to_string()),
            min: Some(0.0),
            max: Some(100.0),
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDqColumn);
        assert!(errors[0].message.contains("typo_field"));
    }

    // =========================================================================
    // Test 13: Valid freshness_check with proper intervals
    // =========================================================================
    #[test]
    fn test_freshness_check_valid_intervals() {
        let columns = make_columns(&["observation_time"]);
        let rules = vec![DqRule {
            rule: "freshness_check".to_string(),
            field: Some("observation_time".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: Some("2 hours".to_string()),
            max_future: Some("10 minutes".to_string()),
            reference: Some("ingestion_time".to_string()),
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(
            errors.is_empty(),
            "Valid freshness_check should have no errors: {:?}",
            errors
        );
    }

    // =========================================================================
    // Test 14: Valid cross_field_check with null-safe expression
    // =========================================================================
    #[test]
    fn test_cross_field_check_valid_expression() {
        let columns = make_columns(&["pm25", "pm10"]);
        let rules = vec![DqRule {
            rule: "cross_field_check".to_string(),
            field: None,
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: Some("pm10_gte_pm25".to_string()),
            expression: Some("pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25".to_string()),
            message: Some("pm10_less_than_pm25".to_string()),
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(
            errors.is_empty(),
            "Valid cross_field_check should have no errors: {:?}",
            errors
        );
    }

    // =========================================================================
    // Test 15: Duplicate rule names fail
    // =========================================================================
    #[test]
    fn test_duplicate_rule_names_fail() {
        let columns = make_columns(&["pm25", "pm10"]);
        let rules = vec![
            DqRule {
                rule: "cross_field_check".to_string(),
                field: None,
                action: Some("flag".to_string()),
                min: None,
                max: None,
                clamp_to_bounds: None,
                allowed_values: None,
                case_sensitive: None,
                pattern: None,
                max_age: None,
                max_future: None,
                reference: None,
                direction: None,
                partition_by: None,
                allow_reset: None,
                reset_threshold: None,
                max_change_per_minute: None,
                name: Some("duplicate_name".to_string()),
                expression: Some("pm25 > 0".to_string()),
                message: None,
                condition: None,
                then_rule: None,
                level: None,
                min_completeness: None,
                expected_range: None,
            },
            DqRule {
                rule: "cross_field_check".to_string(),
                field: None,
                action: Some("flag".to_string()),
                min: None,
                max: None,
                clamp_to_bounds: None,
                allowed_values: None,
                case_sensitive: None,
                pattern: None,
                max_age: None,
                max_future: None,
                reference: None,
                direction: None,
                partition_by: None,
                allow_reset: None,
                reset_threshold: None,
                max_change_per_minute: None,
                name: Some("duplicate_name".to_string()), // Same name
                expression: Some("pm10 > 0".to_string()),
                message: None,
                condition: None,
                then_rule: None,
                level: None,
                min_completeness: None,
                expected_range: None,
            },
        ];

        let errors = validate_dq_rules(&rules, &columns);
        assert!(errors.iter().any(|e| e.message.contains("Duplicate")));
    }

    // =========================================================================
    // Test 16: Interval validation helper
    // =========================================================================
    #[test]
    fn test_interval_validation() {
        // Valid intervals
        assert!(is_valid_interval("2 hours"));
        assert!(is_valid_interval("30 minutes"));
        assert!(is_valid_interval("1 day"));
        assert!(is_valid_interval("5 minutes"));
        assert!(is_valid_interval("1 hour 30 minutes"));
        assert!(is_valid_interval("10 seconds"));
        assert!(is_valid_interval("1 week"));

        // Invalid intervals
        assert!(!is_valid_interval("2 hoursss")); // Typo
        assert!(!is_valid_interval("hours 2")); // Wrong order
        assert!(!is_valid_interval("2hours")); // Missing space
        assert!(!is_valid_interval("two hours")); // Word instead of number
        assert!(!is_valid_interval("")); // Empty
    }

    // =========================================================================
    // Test 17: Levenshtein distance helper
    // =========================================================================
    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("pm25", "pm25"), 0);
        assert_eq!(levenshtein_distance("pm25", "pm52"), 2);
        assert_eq!(levenshtein_distance("pm25", "pm"), 2);
        assert_eq!(levenshtein_distance("temperature", "temprature"), 1);
    }

    // =========================================================================
    // Test 18: Find closest match helper
    // =========================================================================
    #[test]
    fn test_find_closest_match() {
        let columns = make_columns(&["pm25", "pm10", "temperature_c"]);

        let result = find_closest_match("pm52", &columns);
        assert!(result.is_some());
        let matched = result.unwrap();
        assert!(matched.contains("pm25") || matched.contains("pm10"));

        let result = find_closest_match("completelydifferent", &columns);
        assert!(result.is_none()); // Too different
    }

    // =========================================================================
    // Test 19: Cardinality check expected_range validation
    // =========================================================================
    #[test]
    fn test_cardinality_check_expected_range() {
        let columns = make_columns(&["ndp_id"]);

        // Valid range
        let rules_valid = vec![DqRule {
            rule: "cardinality_check".to_string(),
            field: Some("ndp_id".to_string()),
            action: Some("warn".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: Some("batch".to_string()),
            min_completeness: None,
            expected_range: Some((1, 10)),
        }];
        let errors = validate_dq_rules(&rules_valid, &columns);
        assert!(errors.is_empty());

        // Invalid range (min > max)
        let rules_invalid = vec![DqRule {
            rule: "cardinality_check".to_string(),
            field: Some("ndp_id".to_string()),
            action: Some("warn".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: None,
            partition_by: None,
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: Some("batch".to_string()),
            min_completeness: None,
            expected_range: Some((10, 1)), // Invalid: min > max
        }];
        let errors = validate_dq_rules(&rules_invalid, &columns);
        assert!(errors.iter().any(|e| e.code == ErrorCode::InvalidDqRule));
    }

    // =========================================================================
    // Test 20: Monotonic check direction validation
    // =========================================================================
    #[test]
    fn test_monotonic_check_direction_validation() {
        let columns = make_columns(&["cumulative_rainfall", "ndp_id"]);

        // Valid direction
        let rules_valid = vec![DqRule {
            rule: "monotonic_check".to_string(),
            field: Some("cumulative_rainfall".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: Some("increasing".to_string()),
            partition_by: Some(vec!["ndp_id".to_string()]),
            allow_reset: Some(true),
            reset_threshold: Some(1000.0),
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];
        let errors = validate_dq_rules(&rules_valid, &columns);
        assert!(errors.is_empty());

        // Invalid direction
        let rules_invalid = vec![DqRule {
            rule: "monotonic_check".to_string(),
            field: Some("cumulative_rainfall".to_string()),
            action: Some("flag".to_string()),
            min: None,
            max: None,
            clamp_to_bounds: None,
            allowed_values: None,
            case_sensitive: None,
            pattern: None,
            max_age: None,
            max_future: None,
            reference: None,
            direction: Some("invalid_direction".to_string()),
            partition_by: Some(vec!["ndp_id".to_string()]),
            allow_reset: None,
            reset_threshold: None,
            max_change_per_minute: None,
            name: None,
            expression: None,
            message: None,
            condition: None,
            then_rule: None,
            level: None,
            min_completeness: None,
            expected_range: None,
        }];
        let errors = validate_dq_rules(&rules_invalid, &columns);
        assert!(errors.iter().any(|e| e.code == ErrorCode::InvalidDqRule));
    }
}
