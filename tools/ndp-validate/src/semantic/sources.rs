//! Source configuration validation for NDP stream configurations
//!
//! Validates source-specific required fields based on source type:
//! - `mqtt`: requires `broker_url` and `topics`
//! - `http_poll`: requires `endpoints` and positive `poll_interval_secs`
//! - `csv`: requires `path` and `timestamp_field`
//! - `webhook`: no additional requirements
//! - `file_watch`: no additional requirements

use crate::error::{ErrorCode, ValidationError};
use serde_json::Value;

// BUG-001-fix: Import SourceType from ndp-types (single source of truth)
use ndp_types::SourceType;

/// Get supported source types from ndp-types (single source of truth)
///
/// This function returns the authoritative list of supported source types
/// from ndp-types, eliminating the risk of drift between validation and runtime.
fn supported_source_types() -> &'static [&'static str] {
    SourceType::all_names()
}

/// Validate all sources in the configuration
///
/// Returns a vector of validation errors. Empty vector means validation passed.
pub fn validate_sources(sources: &[Value]) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for (idx, source) in sources.iter().enumerate() {
        let base_path = format!("$.sources[{}]", idx);
        errors.extend(validate_source(source, &base_path));
    }

    errors
}

/// Validate a single source configuration
fn validate_source(source: &Value, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Get source type
    let source_type = match source.get("type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            // Missing type is caught by JSON Schema, skip semantic validation
            return errors;
        }
    };

    // Validate source type is supported
    if !supported_source_types().contains(&source_type) {
        let suggestion = find_closest_match(source_type, supported_source_types());
        let mut error = ValidationError::semantic_error(
            ErrorCode::InvalidSourceType,
            &format!("{}.type", base_path),
            format!(
                "Source type '{}' is not supported. Must be one of: {}",
                source_type,
                supported_source_types().join(", ")
            ),
        );
        if let Some(suggestion) = suggestion {
            error = error.with_suggestion(&format!("Did you mean '{}'?", suggestion));
        }
        errors.push(error);
        return errors; // Skip further validation for unsupported type
    }

    // Validate source-specific required fields
    match source_type {
        "mqtt" => errors.extend(validate_mqtt_source(source, base_path)),
        "http_poll" => errors.extend(validate_http_poll_source(source, base_path)),
        "csv" => errors.extend(validate_csv_source(source, base_path)),
        "webhook" | "file_watch" => {
            // No additional required fields for these types
        }
        _ => {} // Already caught above
    }

    errors
}

/// Validate MQTT source configuration
///
/// Required fields:
/// - `broker_url`: The MQTT broker URL
/// - `topics`: Array of topics to subscribe to (at least one)
fn validate_mqtt_source(source: &Value, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check broker_url
    match source.get("broker_url") {
        Some(url) if url.as_str().is_some_and(|s| !s.is_empty()) => {}
        _ => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "MQTT source requires 'broker_url'",
            ));
        }
    }

    // Check topics
    match source.get("topics") {
        Some(Value::Array(topics)) if !topics.is_empty() => {}
        Some(Value::Array(_)) => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "MQTT source requires at least one topic",
            ));
        }
        _ => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "MQTT source requires 'topics' array",
            ));
        }
    }

    errors
}

/// Validate HTTP poll source configuration
///
/// Required fields:
/// - `endpoints`: Array of endpoint configurations (at least one)
/// - `poll_interval_secs`: Positive integer for polling interval
fn validate_http_poll_source(source: &Value, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check endpoints
    match source.get("endpoints") {
        Some(Value::Array(endpoints)) if !endpoints.is_empty() => {}
        Some(Value::Array(_)) => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "HTTP poll source requires at least one endpoint",
            ));
        }
        _ => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "HTTP poll source requires 'endpoints' array",
            ));
        }
    }

    // Check poll_interval_secs
    match source.get("poll_interval_secs") {
        Some(Value::Number(n)) => {
            if let Some(interval) = n.as_i64() {
                if interval <= 0 {
                    errors.push(ValidationError::semantic_error(
                        ErrorCode::InvalidSourceConfig,
                        &format!("{}.poll_interval_secs", base_path),
                        "HTTP poll source requires positive poll_interval_secs",
                    ));
                }
            } else if let Some(interval) = n.as_f64() {
                if interval <= 0.0 {
                    errors.push(ValidationError::semantic_error(
                        ErrorCode::InvalidSourceConfig,
                        &format!("{}.poll_interval_secs", base_path),
                        "HTTP poll source requires positive poll_interval_secs",
                    ));
                }
            }
        }
        None => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "HTTP poll source requires 'poll_interval_secs'",
            ));
        }
        _ => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::InvalidSourceConfig,
                &format!("{}.poll_interval_secs", base_path),
                "poll_interval_secs must be a number",
            ));
        }
    }

    errors
}

/// Validate CSV source configuration
///
/// Required fields:
/// - `path`: Path to the CSV file
/// - `timestamp_field`: Name of the timestamp column
fn validate_csv_source(source: &Value, base_path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check path
    match source.get("path") {
        Some(path) if path.as_str().is_some_and(|s| !s.is_empty()) => {}
        _ => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "CSV source requires 'path'",
            ));
        }
    }

    // Check timestamp_field
    match source.get("timestamp_field") {
        Some(field) if field.as_str().is_some_and(|s| !s.is_empty()) => {}
        _ => {
            errors.push(ValidationError::semantic_error(
                ErrorCode::MissingSourceConfig,
                base_path,
                "CSV source requires 'timestamp_field'",
            ));
        }
    }

    errors
}

/// Find closest matching string using Levenshtein distance
fn find_closest_match(input: &str, candidates: &[&str]) -> Option<String> {
    let input_lower = input.to_lowercase();
    let mut best_match = None;
    let mut best_distance = usize::MAX;

    for candidate in candidates {
        let distance = strsim::levenshtein(&input_lower, &candidate.to_lowercase());
        if distance < best_distance && distance <= 3 {
            best_distance = distance;
            best_match = Some(candidate.to_string());
        }
    }

    best_match
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ===========================================
    // MQTT Source Tests
    // ===========================================

    #[test]
    fn test_mqtt_source_requires_broker_url() {
        // Arrange: MQTT source without broker_url
        let sources = vec![json!({
            "type": "mqtt",
            "topics": ["sensors/temperature"]
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::MissingSourceConfig);
        assert!(errors[0].message.contains("broker_url"));
    }

    #[test]
    fn test_mqtt_source_requires_topics() {
        // Arrange: MQTT source without topics
        let sources = vec![json!({
            "type": "mqtt",
            "broker_url": "mqtt://localhost:1883"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::MissingSourceConfig);
        assert!(errors[0].message.contains("topic"));
    }

    #[test]
    fn test_mqtt_source_requires_at_least_one_topic() {
        // Arrange: MQTT source with empty topics array
        let sources = vec![json!({
            "type": "mqtt",
            "broker_url": "mqtt://localhost:1883",
            "topics": []
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::MissingSourceConfig);
        assert!(errors[0].message.contains("at least one topic"));
    }

    // ===========================================
    // HTTP Poll Source Tests
    // ===========================================

    #[test]
    fn test_http_poll_requires_endpoints() {
        // Arrange: HTTP poll source without endpoints
        let sources = vec![json!({
            "type": "http_poll",
            "poll_interval_secs": 60
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::MissingSourceConfig);
        assert!(errors[0].message.contains("endpoint"));
    }

    #[test]
    fn test_http_poll_requires_positive_interval() {
        // Arrange: HTTP poll source with zero interval
        let sources = vec![json!({
            "type": "http_poll",
            "endpoints": [{"url": "https://api.example.com"}],
            "poll_interval_secs": 0
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidSourceConfig);
        assert!(errors[0].message.contains("positive"));
    }

    #[test]
    fn test_http_poll_rejects_negative_interval() {
        // Arrange: HTTP poll source with negative interval
        let sources = vec![json!({
            "type": "http_poll",
            "endpoints": [{"url": "https://api.example.com"}],
            "poll_interval_secs": -5
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidSourceConfig);
        assert!(errors[0].message.contains("positive"));
    }

    // ===========================================
    // CSV Source Tests
    // ===========================================

    #[test]
    fn test_csv_source_requires_path_and_timestamp_field() {
        // Arrange: CSV source without path and timestamp_field
        let sources = vec![json!({
            "type": "csv"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 2);
        let codes: Vec<_> = errors.iter().map(|e| e.code).collect();
        assert!(codes.contains(&ErrorCode::MissingSourceConfig));
        let messages: String = errors.iter().map(|e| e.message.clone()).collect();
        assert!(messages.contains("path"));
        assert!(messages.contains("timestamp_field"));
    }

    #[test]
    fn test_csv_source_requires_path() {
        // Arrange: CSV source with timestamp_field but no path
        let sources = vec![json!({
            "type": "csv",
            "timestamp_field": "timestamp"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::MissingSourceConfig);
        assert!(errors[0].message.contains("path"));
    }

    #[test]
    fn test_csv_source_requires_timestamp_field() {
        // Arrange: CSV source with path but no timestamp_field
        let sources = vec![json!({
            "type": "csv",
            "path": "/data/sensors.csv"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::MissingSourceConfig);
        assert!(errors[0].message.contains("timestamp_field"));
    }

    // ===========================================
    // Valid Source Tests
    // ===========================================

    #[test]
    fn test_valid_mqtt_source_passes() {
        // Arrange: Complete valid MQTT source
        let sources = vec![json!({
            "type": "mqtt",
            "broker_url": "mqtt://localhost:1883",
            "topics": ["sensors/temperature", "sensors/humidity"]
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert!(
            errors.is_empty(),
            "Expected no errors but got: {:?}",
            errors
        );
    }

    #[test]
    fn test_valid_http_poll_source_passes() {
        // Arrange: Complete valid HTTP poll source
        let sources = vec![json!({
            "type": "http_poll",
            "endpoints": [
                {"url": "https://api.weather.gov/forecast"}
            ],
            "poll_interval_secs": 300
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert!(
            errors.is_empty(),
            "Expected no errors but got: {:?}",
            errors
        );
    }

    #[test]
    fn test_valid_csv_source_passes() {
        // Arrange: Complete valid CSV source
        let sources = vec![json!({
            "type": "csv",
            "path": "/data/sensor_data.csv",
            "timestamp_field": "recorded_at"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert!(
            errors.is_empty(),
            "Expected no errors but got: {:?}",
            errors
        );
    }

    #[test]
    fn test_valid_webhook_source_passes() {
        // Arrange: Webhook source (no required fields beyond type)
        let sources = vec![json!({
            "type": "webhook"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert!(
            errors.is_empty(),
            "Expected no errors but got: {:?}",
            errors
        );
    }

    #[test]
    fn test_valid_file_watch_source_passes() {
        // Arrange: File watch source (no required fields beyond type)
        let sources = vec![json!({
            "type": "file_watch"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert!(
            errors.is_empty(),
            "Expected no errors but got: {:?}",
            errors
        );
    }

    // ===========================================
    // Unsupported Source Type Tests
    // ===========================================

    #[test]
    fn test_unsupported_source_type_fails() {
        // Arrange: Source with unsupported type
        let sources = vec![json!({
            "type": "ftp"
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidSourceType);
        assert!(errors[0].message.contains("ftp"));
        assert!(errors[0].message.contains("not supported"));
    }

    #[test]
    fn test_unsupported_source_type_with_suggestion() {
        // Arrange: Source with typo that should suggest correct type
        let sources = vec![json!({
            "type": "mqt"  // typo for mqtt
        })];

        // Act
        let errors = validate_sources(&sources);

        // Assert
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidSourceType);
        assert!(errors[0].suggestion.as_ref().unwrap().contains("mqtt"));
    }

    // ===========================================
    // Multiple Sources Tests
    // ===========================================

    #[test]
    fn test_multiple_sources_validation() {
        // Arrange: Multiple sources with various issues
        let sources = vec![
            json!({
                "type": "mqtt",
                "broker_url": "mqtt://localhost:1883",
                "topics": ["sensors/+"]
            }),
            json!({
                "type": "http_poll"
                // Missing endpoints and poll_interval_secs
            }),
        ];

        // Act
        let errors = validate_sources(&sources);

        // Assert: First source valid, second should have 2 errors
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|e| e.path.contains("sources[1]")));
    }

    #[test]
    fn test_error_path_includes_index() {
        // Arrange: Source at index 2 with error
        let sources = vec![
            json!({"type": "webhook"}),
            json!({"type": "webhook"}),
            json!({"type": "mqtt"}), // Missing broker_url and topics
        ];

        // Act
        let errors = validate_sources(&sources);

        // Assert: Errors should reference sources[2]
        assert!(!errors.is_empty());
        assert!(errors.iter().all(|e| e.path.contains("sources[2]")));
    }
}
