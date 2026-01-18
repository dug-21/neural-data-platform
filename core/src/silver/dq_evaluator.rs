//! Data Quality evaluation for Silver layer streaming transform.

use regex::Regex;
use serde_json::Value;

use crate::config::{DqAction, DqRule};

use super::types::{DqResult, DqViolation, SilverRecord};

/// Evaluate DQ rules against a SilverRecord.
pub fn evaluate_dq_rules(record: &SilverRecord, rules: &[DqRule]) -> DqResult {
    let mut result = DqResult::passed();
    result.rules_evaluated = rules.len();

    for rule in rules {
        if let Some(violation) = evaluate_single_rule(record, rule) {
            result.add_violation(violation);
        }
    }

    result
}

/// Evaluate DQ rules and apply modifications to the record.
pub fn evaluate_and_apply_dq_rules(record: &mut SilverRecord, rules: &[DqRule]) -> DqResult {
    let mut result = DqResult::passed();
    result.rules_evaluated = rules.len();

    for rule in rules {
        if let Some(violation) = evaluate_and_apply_single_rule(record, rule) {
            result.add_violation(violation);
        }
    }

    result
}

fn evaluate_single_rule(record: &SilverRecord, rule: &DqRule) -> Option<DqViolation> {
    match rule {
        DqRule::RangeCheck {
            field,
            min,
            max,
            action,
            ..
        } => evaluate_range_check(record, field, *min, *max, action),

        DqRule::NullCheck { field, action } => evaluate_null_check(record, field, action),

        DqRule::EnumCheck {
            field,
            allowed_values,
            case_sensitive,
            action,
        } => evaluate_enum_check(record, field, allowed_values, *case_sensitive, action),

        DqRule::PatternCheck {
            field,
            pattern,
            action,
        } => evaluate_pattern_check(record, field, pattern, action),

        DqRule::CrossFieldCheck {
            name,
            expression,
            message,
            action,
        } => evaluate_cross_field_check(record, name, expression, message.as_deref(), action),

        // Temporal/batch rules - skip in streaming
        DqRule::FreshnessCheck { .. }
        | DqRule::MonotonicCheck { .. }
        | DqRule::RateOfChange { .. }
        | DqRule::CompletenessCheck { .. }
        | DqRule::CardinalityCheck { .. }
        | DqRule::ConditionalCheck { .. } => None,
    }
}

fn evaluate_and_apply_single_rule(record: &mut SilverRecord, rule: &DqRule) -> Option<DqViolation> {
    match rule {
        DqRule::RangeCheck {
            field,
            min,
            max,
            action,
            clamp_to_bounds,
        } => {
            if *clamp_to_bounds && matches!(action, DqAction::Clamp) {
                evaluate_and_clamp_range(record, field, *min, *max, action)
            } else {
                evaluate_range_check(record, field, *min, *max, action)
            }
        }
        _ => evaluate_single_rule(record, rule),
    }
}

fn evaluate_range_check(
    record: &SilverRecord,
    field: &str,
    min: Option<f64>,
    max: Option<f64>,
    action: &DqAction,
) -> Option<DqViolation> {
    let value = record.get_field(field)?;
    if value.is_null() {
        return None;
    }
    let num = value.as_f64()?;
    let mut violations = vec![];

    if let Some(min_val) = min {
        if num < min_val {
            violations.push(format!("value {} < min {}", num, min_val));
        }
    }
    if let Some(max_val) = max {
        if num > max_val {
            violations.push(format!("value {} > max {}", num, max_val));
        }
    }

    if violations.is_empty() {
        None
    } else {
        Some(
            DqViolation::new(
                "range_check",
                Some(field.to_string()),
                violations.join("; "),
                action_to_string(action),
            )
            .with_original_value(value.clone()),
        )
    }
}

fn evaluate_and_clamp_range(
    record: &mut SilverRecord,
    field: &str,
    min: Option<f64>,
    max: Option<f64>,
    action: &DqAction,
) -> Option<DqViolation> {
    let value = record.get_field(field).cloned()?;
    if value.is_null() {
        return None;
    }
    let num = value.as_f64()?;
    let mut clamped = num;
    let mut was_clamped = false;

    if let Some(min_val) = min {
        if num < min_val {
            clamped = min_val;
            was_clamped = true;
        }
    }
    if let Some(max_val) = max {
        if num > max_val {
            clamped = max_val;
            was_clamped = true;
        }
    }

    if was_clamped {
        record.fields.insert(
            field.to_string(),
            Value::Number(
                serde_json::Number::from_f64(clamped)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
        Some(
            DqViolation::new(
                "range_check",
                Some(field.to_string()),
                format!("value {} clamped to {}", num, clamped),
                action_to_string(action),
            )
            .with_original_value(value)
            .with_value_modified(),
        )
    } else {
        None
    }
}

fn evaluate_null_check(
    record: &SilverRecord,
    field: &str,
    action: &DqAction,
) -> Option<DqViolation> {
    let value = record.get_field(field);
    let is_null = match value {
        None => true,
        Some(v) => v.is_null(),
    };

    if is_null {
        Some(DqViolation::new(
            "null_check",
            Some(field.to_string()),
            "required field is null or missing",
            action_to_string(action),
        ))
    } else {
        None
    }
}

fn evaluate_enum_check(
    record: &SilverRecord,
    field: &str,
    allowed_values: &[String],
    case_sensitive: bool,
    action: &DqAction,
) -> Option<DqViolation> {
    let value = record.get_field(field)?;
    if value.is_null() {
        return None;
    }

    let str_value = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };

    let is_valid = if case_sensitive {
        allowed_values.contains(&str_value)
    } else {
        let lower = str_value.to_lowercase();
        allowed_values.iter().any(|v| v.to_lowercase() == lower)
    };

    if is_valid {
        None
    } else {
        Some(
            DqViolation::new(
                "enum_check",
                Some(field.to_string()),
                format!("value '{}' not in allowed set", str_value),
                action_to_string(action),
            )
            .with_original_value(value.clone()),
        )
    }
}

fn evaluate_pattern_check(
    record: &SilverRecord,
    field: &str,
    pattern: &str,
    action: &DqAction,
) -> Option<DqViolation> {
    let value = record.get_field(field)?;
    if value.is_null() {
        return None;
    }
    let str_value = value.as_str()?;

    let regex = match Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return None,
    };

    if regex.is_match(str_value) {
        None
    } else {
        Some(
            DqViolation::new(
                "pattern_check",
                Some(field.to_string()),
                format!("value '{}' does not match pattern", str_value),
                action_to_string(action),
            )
            .with_original_value(value.clone()),
        )
    }
}

fn evaluate_cross_field_check(
    record: &SilverRecord,
    name: &str,
    expression: &str,
    message: Option<&str>,
    action: &DqAction,
) -> Option<DqViolation> {
    let result = evaluate_expression(record, expression);
    if result {
        None
    } else {
        Some(DqViolation::new(
            name,
            None,
            message.unwrap_or(&format!("cross-field check failed: {}", expression)),
            action_to_string(action),
        ))
    }
}

fn evaluate_expression(record: &SilverRecord, expression: &str) -> bool {
    let expr = expression.trim();

    // Handle OR
    if let Some(idx) = find_top_level_operator(expr, " OR ") {
        let left = &expr[..idx];
        let right = &expr[idx + 4..];
        return evaluate_expression(record, left) || evaluate_expression(record, right);
    }

    // Handle AND
    if let Some(idx) = find_top_level_operator(expr, " AND ") {
        let left = &expr[..idx];
        let right = &expr[idx + 5..];
        return evaluate_expression(record, left) && evaluate_expression(record, right);
    }

    // Handle IS NULL
    if expr.to_uppercase().ends_with(" IS NULL") {
        let field = expr[..expr.len() - 8].trim();
        return is_null_or_missing(record, field);
    }

    // Handle comparisons
    for (op, evaluator) in [
        (">=", compare_gte as fn(f64, f64) -> bool),
        ("<=", compare_lte as fn(f64, f64) -> bool),
        ("!=", compare_ne as fn(f64, f64) -> bool),
        ("=", compare_eq as fn(f64, f64) -> bool),
        (">", compare_gt as fn(f64, f64) -> bool),
        ("<", compare_lt as fn(f64, f64) -> bool),
    ] {
        if let Some(idx) = expr.find(op) {
            let left = expr[..idx].trim();
            let right = expr[idx + op.len()..].trim();
            let left_val = get_numeric_value(record, left);
            let right_val = get_numeric_value(record, right);
            if let (Some(l), Some(r)) = (left_val, right_val) {
                return evaluator(l, r);
            }
            return false;
        }
    }

    true
}

fn compare_gte(l: f64, r: f64) -> bool {
    l >= r
}
fn compare_lte(l: f64, r: f64) -> bool {
    l <= r
}
fn compare_eq(l: f64, r: f64) -> bool {
    (l - r).abs() < f64::EPSILON
}
fn compare_ne(l: f64, r: f64) -> bool {
    (l - r).abs() >= f64::EPSILON
}
fn compare_gt(l: f64, r: f64) -> bool {
    l > r
}
fn compare_lt(l: f64, r: f64) -> bool {
    l < r
}

fn find_top_level_operator(expr: &str, op: &str) -> Option<usize> {
    let mut depth = 0;
    let op_upper = op.to_uppercase();
    let expr_upper = expr.to_uppercase();
    for (i, c) in expr.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && i + op.len() <= expr.len() && &expr_upper[i..i + op.len()] == op_upper {
            return Some(i);
        }
    }
    None
}

fn is_null_or_missing(record: &SilverRecord, field: &str) -> bool {
    match record.get_field(field) {
        None => true,
        Some(v) => v.is_null(),
    }
}

fn get_numeric_value(record: &SilverRecord, expr: &str) -> Option<f64> {
    if let Ok(num) = expr.parse::<f64>() {
        return Some(num);
    }
    record.get_field_as_f64(expr)
}

fn action_to_string(action: &DqAction) -> String {
    match action {
        DqAction::Flag => "flag".to_string(),
        DqAction::Reject => "reject".to_string(),
        DqAction::Clamp => "clamp".to_string(),
        DqAction::Drop => "drop".to_string(),
        DqAction::Warn => "warn".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    fn test_record() -> SilverRecord {
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        SilverRecord::new("air-quality", ts)
            .with_field("pm25", json!(25.5))
            .with_field("pm10", json!(45.0))
            .with_field("co2", json!(420))
    }

    #[test]
    fn test_range_check_within_bounds() {
        let record = test_record();
        let rule = DqRule::RangeCheck {
            field: "pm25".to_string(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };
        assert!(evaluate_single_rule(&record, &rule).is_none());
    }

    #[test]
    fn test_range_check_out_of_bounds() {
        let record = SilverRecord::new("test", Utc::now()).with_field("value", json!(1500.0));
        let rule = DqRule::RangeCheck {
            field: "value".to_string(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };
        let violation = evaluate_single_rule(&record, &rule).unwrap();
        assert_eq!(violation.rule_name, "range_check");
    }

    #[test]
    fn test_null_check() {
        let record = SilverRecord::new("test", Utc::now());
        let rule = DqRule::NullCheck {
            field: "missing".to_string(),
            action: DqAction::Reject,
        };
        assert!(evaluate_single_rule(&record, &rule).is_some());
    }

    #[test]
    fn test_cross_field_check() {
        let record = test_record();
        let rule = DqRule::CrossFieldCheck {
            name: "pm10_gte_pm25".to_string(),
            expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25".to_string(),
            message: None,
            action: DqAction::Flag,
        };
        assert!(evaluate_single_rule(&record, &rule).is_none());
    }

    #[test]
    fn test_evaluate_dq_rules() {
        let record = test_record();
        let rules = vec![DqRule::RangeCheck {
            field: "pm25".to_string(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        }];
        let result = evaluate_dq_rules(&record, &rules);
        assert!(result.passed);
        assert_eq!(result.rules_evaluated, 1);
    }
}
