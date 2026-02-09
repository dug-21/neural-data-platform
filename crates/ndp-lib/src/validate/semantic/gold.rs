//! Gold Layer Semantic Validation
//!
//! Validates Gold ETL configuration semantics that cannot be expressed in JSON Schema.
//! Implements validation rules per FE-001 DECISIONS.md Decision 2.
//!
//! # Error Codes (400-408)
//!
//! - `InvalidGoldField` (400): gold_etl references field not in stream
//! - `InvalidStreamType` (401): transitions on non-state_event stream (warning)
//! - `UnknownAlignmentStream` (402): alignment references unknown stream
//! - `InvalidAggregateMetric` (403): unknown metric type
//! - `InvalidFeatureType` (405): unknown feature type
//! - `InvalidGranularity` (406): granularity format not recognized

use crate::constants::{VALID_METRICS, VALID_ROLLING_STATS};
use crate::validate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};
use serde_json::Value;
use std::collections::HashSet;
use strsim::levenshtein;

/// Maximum Levenshtein distance for "did you mean" suggestions
const MAX_SUGGESTION_DISTANCE: usize = 3;

/// Validate Gold ETL configuration semantics
///
/// # Arguments
///
/// * `config` - The complete stream configuration JSON
///
/// # Returns
///
/// Vector of validation errors (empty if valid)
pub fn validate_gold_etl(config: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Extract gold_etl section - if not present, no validation needed
    let gold_etl = match config.get("gold_etl") {
        Some(ge) => ge,
        None => return errors,
    };

    // If not enabled, skip detailed validation
    let enabled = gold_etl
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !enabled {
        return errors;
    }

    // Extract field names from config.fields[]
    let field_names: HashSet<String> = extract_field_names(config);

    // Get stream_type if present
    let stream_type = config
        .get("stream_type")
        .and_then(|v| v.as_str())
        .unwrap_or("observation");

    // Validate aggregates section
    if let Some(aggregates) = gold_etl.get("aggregates") {
        errors.extend(validate_aggregates(aggregates, &field_names));
    }

    // Validate features section
    if let Some(features) = gold_etl.get("features") {
        errors.extend(validate_features(features, &field_names));
    }

    // Validate transitions section
    if let Some(transitions) = gold_etl.get("transitions") {
        errors.extend(validate_transitions(transitions, stream_type, &field_names));
    }

    errors
}

/// Extract field names from config.fields[]
fn extract_field_names(config: &Value) -> HashSet<String> {
    config
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|fields| {
            fields
                .iter()
                .filter_map(|f| f.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Validate aggregates configuration
fn validate_aggregates(aggregates: &Value, field_names: &HashSet<String>) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Validate granularities format
    if let Some(granularities) = aggregates.get("granularities").and_then(|v| v.as_array()) {
        for (idx, granularity) in granularities.iter().enumerate() {
            if let Some(g) = granularity.as_str() {
                if !is_valid_granularity(g) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidGranularity,
                        path: format!("$.gold_etl.aggregates.granularities[{}]", idx),
                        message: format!(
                            "Invalid granularity format '{}'. Expected format: '<number> <unit>' where unit is minute(s), hour(s), or day(s)",
                            g
                        ),
                        severity: Severity::Error,
                        suggestion: Some("Examples: '1 hour', '15 minutes', '1 day', '7 days'".to_string()),
                        context: None,
                    });
                }
            }
        }
    }

    // Validate default_metrics
    if let Some(default_metrics) = aggregates.get("default_metrics").and_then(|v| v.as_array()) {
        for (idx, metric) in default_metrics.iter().enumerate() {
            if let Some(m) = metric.as_str() {
                if !VALID_METRICS.contains(&m) {
                    let suggestion = find_closest_metric(m);
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidAggregateMetric,
                        path: format!("$.gold_etl.aggregates.default_metrics[{}]", idx),
                        message: format!(
                            "Invalid metric '{}'. Valid metrics: {}",
                            m,
                            VALID_METRICS.join(", ")
                        ),
                        severity: Severity::Error,
                        suggestion,
                        context: None,
                    });
                }
            }
        }
    }

    // Validate fields section - check all referenced fields exist
    if let Some(fields_obj) = aggregates.get("fields").and_then(|v| v.as_object()) {
        for (field_name, field_config) in fields_obj {
            // Check field exists in stream
            if !field_names.contains(field_name) {
                let suggestion = find_closest_field(field_name, field_names);
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidGoldField,
                    path: format!("$.gold_etl.aggregates.fields.{}", field_name),
                    message: format!(
                        "Field '{}' not found in stream. Available fields: {}",
                        field_name,
                        format_field_list(field_names)
                    ),
                    severity: Severity::Error,
                    suggestion,
                    context: None,
                });
            }

            // Validate metrics for this field
            if let Some(metrics) = field_config.get("metrics").and_then(|v| v.as_array()) {
                for (idx, metric) in metrics.iter().enumerate() {
                    if let Some(m) = metric.as_str() {
                        if !VALID_METRICS.contains(&m) {
                            let suggestion = find_closest_metric(m);
                            errors.push(ValidationError {
                                layer: ValidationLayer::Semantic,
                                code: ErrorCode::InvalidAggregateMetric,
                                path: format!(
                                    "$.gold_etl.aggregates.fields.{}.metrics[{}]",
                                    field_name, idx
                                ),
                                message: format!(
                                    "Invalid metric '{}'. Valid metrics: {}",
                                    m,
                                    VALID_METRICS.join(", ")
                                ),
                                severity: Severity::Error,
                                suggestion,
                                context: None,
                            });
                        }
                    }
                }
            }
        }
    }

    errors
}

/// Validate features configuration (lag, rolling, trend)
fn validate_features(features: &Value, field_names: &HashSet<String>) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Validate lag features
    if let Some(lag) = features.get("lag") {
        let enabled = lag
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enabled {
            // Validate lags_hours is non-empty
            if let Some(lags_hours) = lag.get("lags_hours").and_then(|v| v.as_array()) {
                if lags_hours.is_empty() {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidFeatureType,
                        path: "$.gold_etl.features.lag.lags_hours".to_string(),
                        message: "lags_hours cannot be empty when lag is enabled".to_string(),
                        severity: Severity::Error,
                        suggestion: Some("Add at least one lag hour, e.g. [1, 6, 24]".to_string()),
                        context: None,
                    });
                } else {
                    // Validate each hour is >= 1
                    for (idx, hour) in lags_hours.iter().enumerate() {
                        if let Some(h) = hour.as_i64() {
                            if h < 1 {
                                errors.push(ValidationError {
                                    layer: ValidationLayer::Semantic,
                                    code: ErrorCode::InvalidFeatureType,
                                    path: format!(
                                        "$.gold_etl.features.lag.lags_hours[{}]",
                                        idx
                                    ),
                                    message: format!(
                                        "Lag hours must be >= 1, got {}",
                                        h
                                    ),
                                    severity: Severity::Error,
                                    suggestion: None,
                                    context: None,
                                });
                            }
                        }
                    }
                }
            }

            // Validate lag fields exist
            if let Some(fields) = lag.get("fields").and_then(|v| v.as_array()) {
                for (idx, field) in fields.iter().enumerate() {
                    if let Some(f) = field.as_str() {
                        if !field_names.contains(f) {
                            let suggestion = find_closest_field(f, field_names);
                            errors.push(ValidationError {
                                layer: ValidationLayer::Semantic,
                                code: ErrorCode::InvalidGoldField,
                                path: format!("$.gold_etl.features.lag.fields[{}]", idx),
                                message: format!("Lag field '{}' not found in stream", f),
                                severity: Severity::Error,
                                suggestion,
                                context: None,
                            });
                        }
                    }
                }
            }
        }
    }

    // Validate rolling features
    if let Some(rolling) = features.get("rolling") {
        let enabled = rolling
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enabled {
            // Validate windows is non-empty
            if let Some(windows) = rolling.get("windows").and_then(|v| v.as_array()) {
                if windows.is_empty() {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidFeatureType,
                        path: "$.gold_etl.features.rolling.windows".to_string(),
                        message: "windows cannot be empty when rolling is enabled".to_string(),
                        severity: Severity::Error,
                        suggestion: Some(
                            "Add at least one window, e.g. [\"4 hours\", \"24 hours\"]".to_string(),
                        ),
                        context: None,
                    });
                }
            }

            // Validate windows format
            if let Some(windows) = rolling.get("windows").and_then(|v| v.as_array()) {
                for (idx, window) in windows.iter().enumerate() {
                    if let Some(w) = window.as_str() {
                        if !is_valid_granularity(w) {
                            errors.push(ValidationError {
                                layer: ValidationLayer::Semantic,
                                code: ErrorCode::InvalidGranularity,
                                path: format!("$.gold_etl.features.rolling.windows[{}]", idx),
                                message: format!(
                                    "Invalid window format '{}'. Expected format: '<number> <unit>'",
                                    w
                                ),
                                severity: Severity::Error,
                                suggestion: Some("Examples: '4 hours', '24 hours'".to_string()),
                                context: None,
                            });
                        }
                    }
                }
            }

            // Validate stats
            if let Some(stats) = rolling.get("stats").and_then(|v| v.as_array()) {
                for (idx, stat) in stats.iter().enumerate() {
                    if let Some(s) = stat.as_str() {
                        if !VALID_ROLLING_STATS.contains(&s) {
                            errors.push(ValidationError {
                                layer: ValidationLayer::Semantic,
                                code: ErrorCode::InvalidFeatureType,
                                path: format!("$.gold_etl.features.rolling.stats[{}]", idx),
                                message: format!(
                                    "Invalid stat '{}'. Valid stats: {}",
                                    s,
                                    VALID_ROLLING_STATS.join(", ")
                                ),
                                severity: Severity::Error,
                                suggestion: None,
                                context: None,
                            });
                        }
                    }
                }
            }

            // Validate fields exist
            if let Some(fields) = rolling.get("fields").and_then(|v| v.as_array()) {
                for (idx, field) in fields.iter().enumerate() {
                    if let Some(f) = field.as_str() {
                        if !field_names.contains(f) {
                            let suggestion = find_closest_field(f, field_names);
                            errors.push(ValidationError {
                                layer: ValidationLayer::Semantic,
                                code: ErrorCode::InvalidGoldField,
                                path: format!("$.gold_etl.features.rolling.fields[{}]", idx),
                                message: format!("Rolling field '{}' not found in stream", f),
                                severity: Severity::Error,
                                suggestion,
                                context: None,
                            });
                        }
                    }
                }
            }
        }
    }

    // Validate trend features
    if let Some(trend) = features.get("trend") {
        let enabled = trend
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enabled {
            // Validate window is non-empty
            if let Some(window) = trend.get("window").and_then(|v| v.as_str()) {
                if window.is_empty() {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidFeatureType,
                        path: "$.gold_etl.features.trend.window".to_string(),
                        message: "window cannot be empty when trend is enabled".to_string(),
                        severity: Severity::Error,
                        suggestion: Some("Set a window, e.g. '4 hours'".to_string()),
                        context: None,
                    });
                } else if !is_valid_granularity(window) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidGranularity,
                        path: "$.gold_etl.features.trend.window".to_string(),
                        message: format!(
                            "Invalid trend window format '{}'. Expected format: '<number> <unit>'",
                            window
                        ),
                        severity: Severity::Error,
                        suggestion: Some("Example: '4 hours'".to_string()),
                        context: None,
                    });
                }
            }

            // Validate fields exist
            if let Some(fields) = trend.get("fields").and_then(|v| v.as_array()) {
                for (idx, field) in fields.iter().enumerate() {
                    if let Some(f) = field.as_str() {
                        if !field_names.contains(f) {
                            let suggestion = find_closest_field(f, field_names);
                            errors.push(ValidationError {
                                layer: ValidationLayer::Semantic,
                                code: ErrorCode::InvalidGoldField,
                                path: format!("$.gold_etl.features.trend.fields[{}]", idx),
                                message: format!("Trend field '{}' not found in stream", f),
                                severity: Severity::Error,
                                suggestion,
                                context: None,
                            });
                        }
                    }
                }
            }
        }
    }

    errors
}

/// Validate transitions configuration
fn validate_transitions(
    transitions: &Value,
    stream_type: &str,
    field_names: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let enabled = transitions
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if enabled {
        // Transitions only valid for state_event streams
        if stream_type != "state_event" {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidStreamType,
                path: "$.gold_etl.transitions".to_string(),
                message: format!(
                    "Transitions config only applies to state_event streams, but stream_type is '{}'",
                    stream_type
                ),
                severity: Severity::Warning,
                suggestion: Some("Set stream_type: state_event or remove transitions config".to_string()),
                context: None,
            });
        }

        // Validate state_field exists if specified
        if let Some(state_field) = transitions.get("state_field").and_then(|v| v.as_str()) {
            if !field_names.contains(state_field) && state_field != "state" {
                let suggestion = find_closest_field(state_field, field_names);
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidGoldField,
                    path: "$.gold_etl.transitions.state_field".to_string(),
                    message: format!("State field '{}' not found in stream", state_field),
                    severity: Severity::Error,
                    suggestion,
                    context: None,
                });
            }
        }

        // Validate entity_field exists if specified
        if let Some(entity_field) = transitions.get("entity_field").and_then(|v| v.as_str()) {
            // ndp_id is a system field, always valid
            if !field_names.contains(entity_field) && entity_field != "ndp_id" {
                let suggestion = find_closest_field(entity_field, field_names);
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidGoldField,
                    path: "$.gold_etl.transitions.entity_field".to_string(),
                    message: format!("Entity field '{}' not found in stream", entity_field),
                    severity: Severity::Error,
                    suggestion,
                    context: None,
                });
            }
        }
    }

    errors
}

/// Check if a granularity string is valid (delegates to shared implementation)
fn is_valid_granularity(granularity: &str) -> bool {
    super::is_valid_granularity(granularity)
}

/// Find the closest matching metric using Levenshtein distance
fn find_closest_metric(input: &str) -> Option<String> {
    find_closest_in_list(input, VALID_METRICS)
}

/// Find the closest matching field using Levenshtein distance
fn find_closest_field(input: &str, candidates: &HashSet<String>) -> Option<String> {
    let candidates_vec: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
    find_closest_in_list(input, &candidates_vec)
}

/// Generic closest match finder
fn find_closest_in_list(input: &str, candidates: &[&str]) -> Option<String> {
    let input_lower = input.to_lowercase();
    let mut best_match: Option<(String, usize)> = None;

    for candidate in candidates {
        let distance = levenshtein(&input_lower, &candidate.to_lowercase());

        if distance <= MAX_SUGGESTION_DISTANCE {
            match &best_match {
                None => best_match = Some((candidate.to_string(), distance)),
                Some((_, best_distance)) if distance < *best_distance => {
                    best_match = Some((candidate.to_string(), distance));
                }
                _ => {}
            }
        }
    }

    best_match.map(|(name, _)| format!("Did you mean '{}'?", name))
}

/// Format field list for error messages (sorted, truncated if too long)
fn format_field_list(fields: &HashSet<String>) -> String {
    let mut sorted: Vec<_> = fields.iter().cloned().collect();
    sorted.sort();
    if sorted.len() > 10 {
        format!(
            "{}, ... ({} more)",
            sorted[..10].join(", "),
            sorted.len() - 10
        )
    } else {
        sorted.join(", ")
    }
}

// =============================================================================
// Tests - London School TDD
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =========================================================================
    // Test 1: Valid gold_etl configuration passes
    // =========================================================================
    #[test]
    fn test_gold_field_validation_passes_for_valid() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" },
                { "name": "co2", "type": "int" },
                { "name": "temperature", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour", "1 day"],
                    "default_metrics": ["mean", "std"],
                    "fields": {
                        "pm25": { "metrics": ["mean", "std", "max", "p95"] },
                        "co2": { "metrics": ["mean", "count"] }
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // =========================================================================
    // Test 2: Invalid field reference fails
    // =========================================================================
    #[test]
    fn test_gold_field_validation_fails_for_nonexistent() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" },
                { "name": "co2", "type": "int" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "nonexistent_field": { "metrics": ["mean"] }
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidGoldField);
        assert!(errors[0].message.contains("nonexistent_field"));
        assert!(errors[0]
            .path
            .contains("$.gold_etl.aggregates.fields.nonexistent_field"));
    }

    // =========================================================================
    // Test 3: Transitions on observation stream raises warning
    // =========================================================================
    #[test]
    fn test_transitions_on_observation_stream_warns() {
        let config = json!({
            "stream_id": "air-quality",
            "stream_type": "observation",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "transitions": {
                    "enabled": true,
                    "state_field": "state"
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidStreamType);
        assert_eq!(errors[0].severity, Severity::Warning);
        assert!(errors[0].message.contains("state_event"));
    }

    // =========================================================================
    // Test 4: Invalid metric fails
    // =========================================================================
    #[test]
    fn test_invalid_metric_fails() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "pm25": { "metrics": ["mean", "maxs"] }
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidAggregateMetric);
        assert!(errors[0].message.contains("maxs"));
        // Should suggest "max" as closest match (Levenshtein distance 1)
        assert!(errors[0]
            .suggestion
            .as_ref()
            .map_or(false, |s| s.contains("max")));
    }

    // =========================================================================
    // Test 5: Invalid granularity format fails
    // =========================================================================
    #[test]
    fn test_invalid_granularity_format_fails() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour", "hourly"]
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidGranularity);
        assert!(errors[0].message.contains("hourly"));
    }

    // =========================================================================
    // Test 6: Valid transitions on state_event stream passes
    // =========================================================================
    #[test]
    fn test_transitions_on_state_event_stream_passes() {
        let config = json!({
            "stream_id": "home-assistant-state",
            "stream_type": "state_event",
            "fields": [
                { "name": "state", "type": "string" },
                { "name": "entity_id", "type": "string" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "transitions": {
                    "enabled": true,
                    "state_field": "state",
                    "entity_field": "entity_id"
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // =========================================================================
    // Test 7: Disabled gold_etl skips validation
    // =========================================================================
    #[test]
    fn test_disabled_gold_etl_skips_validation() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [],
            "gold_etl": {
                "enabled": false,
                "aggregates": {
                    "granularities": ["invalid"],
                    "fields": {
                        "nonexistent": { "metrics": ["invalid_metric"] }
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(
            errors.is_empty(),
            "Disabled gold_etl should skip validation"
        );
    }

    // =========================================================================
    // Test 8: Missing gold_etl section returns no errors
    // =========================================================================
    #[test]
    fn test_missing_gold_etl_returns_no_errors() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ]
        });

        let errors = validate_gold_etl(&config);
        assert!(errors.is_empty());
    }

    // =========================================================================
    // Test 9: Lag feature with invalid field fails
    // =========================================================================
    #[test]
    fn test_lag_feature_invalid_field_fails() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "lag": {
                        "enabled": true,
                        "lags_hours": [1, 6, 24],
                        "fields": ["pm25", "nonexistent"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidGoldField);
        assert!(errors[0].path.contains("$.gold_etl.features.lag.fields"));
    }

    // =========================================================================
    // Test 10: Rolling feature with invalid stat fails
    // =========================================================================
    #[test]
    fn test_rolling_feature_invalid_stat_fails() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "rolling": {
                        "enabled": true,
                        "windows": ["4 hours"],
                        "stats": ["mean", "p95"],
                        "fields": ["pm25"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidFeatureType);
        assert!(errors[0].message.contains("p95"));
    }

    // =========================================================================
    // Test 11: Granularity pattern validation
    // =========================================================================
    #[test]
    fn test_granularity_patterns() {
        // Valid patterns
        assert!(is_valid_granularity("1 hour"));
        assert!(is_valid_granularity("15 minutes"));
        assert!(is_valid_granularity("1 day"));
        assert!(is_valid_granularity("7 days"));
        assert!(is_valid_granularity("30 minute"));

        // Invalid patterns
        assert!(!is_valid_granularity("1hr"));
        assert!(!is_valid_granularity("hourly"));
        assert!(!is_valid_granularity("60m"));
        assert!(!is_valid_granularity("1 week"));
        assert!(!is_valid_granularity(""));
    }

    // =========================================================================
    // Test 12: Default metrics validation
    // =========================================================================
    #[test]
    fn test_default_metrics_validation() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "default_metrics": ["mean", "avg"]
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidAggregateMetric);
        assert!(errors[0].message.contains("avg"));
    }

    // =========================================================================
    // Test 13: Trend feature with invalid window fails
    // =========================================================================
    #[test]
    fn test_trend_feature_invalid_window_fails() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "trend": {
                        "enabled": true,
                        "window": "4h",
                        "fields": ["pm25"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidGranularity);
        assert!(errors[0].path.contains("$.gold_etl.features.trend.window"));
    }

    // =========================================================================
    // Test 15: Lag lags_hours non-empty when enabled
    // =========================================================================
    #[test]
    fn test_validate_gold_etl_lag_hours_non_empty() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "lag": {
                        "enabled": true,
                        "lags_hours": [],
                        "fields": ["pm25"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(
            !errors.is_empty(),
            "Expected error for empty lags_hours, got none"
        );
        let lag_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.path.contains("lag") && e.message.contains("lags_hours"))
            .collect();
        assert_eq!(
            lag_errors.len(),
            1,
            "Expected exactly 1 lag lags_hours error, got: {:?}",
            lag_errors
        );
        assert_eq!(lag_errors[0].code, ErrorCode::InvalidFeatureType);
    }

    // =========================================================================
    // Test 16: Lag hours must be >= 1
    // =========================================================================
    #[test]
    fn test_validate_gold_etl_lag_hours_minimum_one() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "lag": {
                        "enabled": true,
                        "lags_hours": [1, 0, 24],
                        "fields": ["pm25"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(
            !errors.is_empty(),
            "Expected error for lag hours < 1, got none"
        );
        let hour_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.path.contains("lags_hours") && e.message.contains("must be >= 1"))
            .collect();
        assert_eq!(
            hour_errors.len(),
            1,
            "Expected exactly 1 lag hours minimum error, got: {:?}",
            hour_errors
        );
        assert_eq!(hour_errors[0].code, ErrorCode::InvalidFeatureType);
    }

    // =========================================================================
    // Test 17: Rolling windows non-empty when enabled
    // =========================================================================
    #[test]
    fn test_validate_gold_etl_rolling_windows_non_empty() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "rolling": {
                        "enabled": true,
                        "windows": [],
                        "stats": ["mean"],
                        "fields": ["pm25"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(
            !errors.is_empty(),
            "Expected error for empty rolling windows, got none"
        );
        let window_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.path.contains("rolling") && e.message.contains("windows"))
            .collect();
        assert_eq!(
            window_errors.len(),
            1,
            "Expected exactly 1 rolling windows error, got: {:?}",
            window_errors
        );
        assert_eq!(window_errors[0].code, ErrorCode::InvalidFeatureType);
    }

    // =========================================================================
    // Test 18: Trend window non-empty when enabled
    // =========================================================================
    #[test]
    fn test_validate_gold_etl_trend_window_non_empty() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"]
                },
                "features": {
                    "trend": {
                        "enabled": true,
                        "window": "",
                        "fields": ["pm25"]
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert!(
            !errors.is_empty(),
            "Expected error for empty trend window, got none"
        );
        let window_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.path.contains("trend.window") && e.message.contains("window"))
            .collect();
        assert_eq!(
            window_errors.len(),
            1,
            "Expected exactly 1 trend window error, got: {:?}",
            window_errors
        );
        assert_eq!(window_errors[0].code, ErrorCode::InvalidFeatureType);
    }

    // =========================================================================
    // Test 14: Field suggestion on typo
    // =========================================================================
    #[test]
    fn test_field_suggestion_on_typo() {
        let config = json!({
            "stream_id": "air-quality",
            "fields": [
                { "name": "pm25", "type": "float" },
                { "name": "temperature", "type": "float" }
            ],
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": {
                        "pm52": { "metrics": ["mean"] }
                    }
                }
            }
        });

        let errors = validate_gold_etl(&config);
        assert_eq!(errors.len(), 1);
        // Should suggest pm25 as closest match
        assert!(
            errors[0]
                .suggestion
                .as_ref()
                .map_or(false, |s| s.contains("pm25")),
            "Should suggest 'pm25', got: {:?}",
            errors[0].suggestion
        );
    }
}
