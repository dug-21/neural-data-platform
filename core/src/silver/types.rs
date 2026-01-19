//! Silver layer types for streaming transform.
//!
//! This module defines the core types for Bronze-to-Silver streaming transforms:
//! - `SilverRecord`: The transformed output record ready for TimescaleDB
//! - `TransformError`: Errors that can occur during transformation
//! - `DqViolation`: Individual data quality rule violations
//! - `DqResult`: Aggregated DQ evaluation result

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

// =============================================================================
// Silver Record
// =============================================================================

/// Transformed Silver layer record ready for TimescaleDB insertion.
///
/// Note: Column names are NOT stored in SilverRecord - they come from
/// SilverEtlConfig at write time. This keeps records data-only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SilverRecord {
    /// Stream identifier (e.g., "air-quality", "outdoor-weather")
    pub stream_id: String,

    /// Primary observation timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional secondary timestamp (e.g., forecast valid_time)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_timestamp: Option<DateTime<Utc>>,

    /// Device or source identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Transformed fields as key-value pairs
    pub fields: HashMap<String, Value>,

    /// Identity fields passed through unchanged
    #[serde(default)]
    pub identity_fields: HashMap<String, Value>,

    /// Data quality evaluation result
    pub dq_result: DqResult,
}

impl SilverRecord {
    pub fn new(stream_id: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self {
            stream_id: stream_id.into(),
            timestamp,
            valid_timestamp: None,
            device_id: None,
            fields: HashMap::new(),
            identity_fields: HashMap::new(),
            dq_result: DqResult::passed(),
        }
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn with_valid_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.valid_timestamp = Some(ts);
        self
    }

    pub fn with_field(mut self, name: impl Into<String>, value: Value) -> Self {
        self.fields.insert(name.into(), value);
        self
    }

    pub fn with_identity_field(mut self, name: impl Into<String>, value: Value) -> Self {
        self.identity_fields.insert(name.into(), value);
        self
    }

    pub fn with_dq_result(mut self, dq_result: DqResult) -> Self {
        self.dq_result = dq_result;
        self
    }

    pub fn should_drop(&self) -> bool {
        self.dq_result.should_drop
    }

    pub fn dq_flags(&self) -> Vec<String> {
        self.dq_result
            .violations
            .iter()
            .map(|v| v.flag_string())
            .collect()
    }

    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn get_field_as_f64(&self, name: &str) -> Option<f64> {
        self.fields.get(name).and_then(|v| v.as_f64())
    }
}

impl Default for SilverRecord {
    fn default() -> Self {
        Self::new("", Utc::now())
    }
}

// =============================================================================
// Transform Error
// =============================================================================

#[derive(Debug, Error, Clone, PartialEq)]
pub enum TransformError {
    #[error("Missing required field: {field} at path '{path}'")]
    MissingField { field: String, path: String },

    #[error("Invalid timestamp '{value}' for field '{field}': {reason}")]
    InvalidTimestamp {
        field: String,
        value: String,
        reason: String,
    },

    #[error("Type conversion failed for field '{field}': expected {expected}, got {actual}")]
    TypeConversion {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("JSON path extraction failed for '{path}': {reason}")]
    JsonPathError { path: String, reason: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Expression evaluation failed for '{field}': {reason}")]
    ExpressionError { field: String, reason: String },

    #[error("Record dropped due to DQ rule: {rule}")]
    DroppedByDq { rule: String },
}

// =============================================================================
// DQ Violation
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DqViolation {
    pub rule_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_value: Option<Value>,
    pub action: String,
    pub value_modified: bool,
}

impl DqViolation {
    pub fn new(
        rule_name: impl Into<String>,
        field: Option<String>,
        message: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            rule_name: rule_name.into(),
            field,
            message: message.into(),
            original_value: None,
            action: action.into(),
            value_modified: false,
        }
    }

    pub fn with_original_value(mut self, value: Value) -> Self {
        self.original_value = Some(value);
        self
    }

    pub fn with_value_modified(mut self) -> Self {
        self.value_modified = true;
        self
    }

    pub fn flag_string(&self) -> String {
        match &self.field {
            Some(f) => format!("{}:{}", self.rule_name, f),
            None => self.rule_name.clone(),
        }
    }
}

// =============================================================================
// DQ Result
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DqResult {
    pub passed: bool,
    pub violations: Vec<DqViolation>,
    pub quality_score: f64,
    pub should_drop: bool,
    pub rules_evaluated: usize,
}

impl DqResult {
    pub fn passed() -> Self {
        Self {
            passed: true,
            violations: Vec::new(),
            quality_score: 1.0,
            should_drop: false,
            rules_evaluated: 0,
        }
    }

    pub fn with_violations(violations: Vec<DqViolation>, rules_evaluated: usize) -> Self {
        let should_drop = violations.iter().any(|v| v.action == "drop");
        let passed = violations.is_empty()
            || violations
                .iter()
                .all(|v| v.action == "flag" || v.action == "warn");

        let quality_score = if rules_evaluated > 0 {
            1.0 - (violations.len() as f64 / rules_evaluated as f64)
        } else {
            1.0
        };

        Self {
            passed,
            violations,
            quality_score: quality_score.max(0.0),
            should_drop,
            rules_evaluated,
        }
    }

    pub fn add_violation(&mut self, violation: DqViolation) {
        if violation.action == "drop" {
            self.should_drop = true;
            self.passed = false;
        } else if violation.action == "reject" {
            self.passed = false;
        }
        self.violations.push(violation);
        self.recalculate_score();
    }

    pub fn merge(&mut self, other: DqResult) {
        self.violations.extend(other.violations);
        self.rules_evaluated += other.rules_evaluated;
        self.should_drop = self.should_drop || other.should_drop;
        self.recalculate_score();
    }

    fn recalculate_score(&mut self) {
        self.passed = self.violations.is_empty()
            || self
                .violations
                .iter()
                .all(|v| v.action == "flag" || v.action == "warn");

        self.quality_score = if self.rules_evaluated > 0 {
            (1.0 - (self.violations.len() as f64 / self.rules_evaluated as f64)).max(0.0)
        } else {
            1.0
        };
    }

    pub fn count_by_action(&self, action: &str) -> usize {
        self.violations
            .iter()
            .filter(|v| v.action == action)
            .count()
    }
}

impl Default for DqResult {
    fn default() -> Self {
        Self::passed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn test_silver_record_new() {
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        let record = SilverRecord::new("air-quality", ts);

        assert_eq!(record.stream_id, "air-quality");
        assert_eq!(record.timestamp, ts);
        assert!(record.dq_result.passed);
    }

    #[test]
    fn test_silver_record_builder() {
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        let record = SilverRecord::new("air-quality", ts)
            .with_device_id("device-001")
            .with_field("pm25", json!(12.5));

        assert_eq!(record.device_id, Some("device-001".to_string()));
        assert_eq!(record.fields["pm25"], json!(12.5));
    }

    #[test]
    fn test_dq_result_passed() {
        let result = DqResult::passed();
        assert!(result.passed);
        assert_eq!(result.quality_score, 1.0);
    }

    #[test]
    fn test_dq_result_with_violations() {
        let violations = vec![DqViolation::new(
            "range_check",
            Some("pm25".to_string()),
            "out of range",
            "flag",
        )];
        let result = DqResult::with_violations(violations, 5);

        assert!(result.passed); // Flag only is still passed
        assert_eq!(result.quality_score, 0.8); // 1 - 1/5
    }

    #[test]
    fn test_dq_violation_flag_string() {
        let v1 = DqViolation::new("range_check", Some("pm25".to_string()), "test", "flag");
        assert_eq!(v1.flag_string(), "range_check:pm25");

        let v2 = DqViolation::new("cross_check", None, "test", "flag");
        assert_eq!(v2.flag_string(), "cross_check");
    }
}
