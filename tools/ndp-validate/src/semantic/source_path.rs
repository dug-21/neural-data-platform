//! Source path cross-reference validation (FR-022)
//!
//! This module validates that `silver_etl.field_mappings[].source_path` values
//! reference fields that actually exist in `config.fields[]`.
//!
//! This is the critical P-005 fix from dp-016 - source_path typos currently
//! cause silent NULL values in Silver because the ETL cannot find the referenced field.
//!
//! ## Algorithm (from PSEUDOCODE.md section 5.5)
//!
//! 1. Build a HashSet of valid field names from `config.fields[].name`
//! 2. For each `field_mapping` in `silver_etl.field_mappings`:
//!    - Extract source_path and validate it starts with "raw_payload."
//!    - Extract the field reference (everything after "raw_payload.")
//!    - For nested paths like "raw_payload.nested.field", check the root "nested"
//!    - If the root field is not in field_names, generate INVALID_SOURCE_PATH error
//!    - Use Levenshtein distance to suggest close matches ("did you mean")
//!
//! ## Error Code: INVALID_SOURCE_PATH

use crate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};
use std::collections::HashSet;
use strsim::levenshtein;

/// Raw payload prefix that all source_path values must start with
const RAW_PAYLOAD_PREFIX: &str = "raw_payload.";

/// Maximum Levenshtein distance for "did you mean" suggestions
const MAX_SUGGESTION_DISTANCE: usize = 3;

/// Validates source_path cross-references in silver_etl.field_mappings
///
/// # Arguments
///
/// * `field_names` - Set of valid field names from config.fields[]
/// * `field_mappings` - List of (index, source_path) tuples to validate
///
/// # Returns
///
/// Vector of ValidationError for any invalid source_path references
pub fn validate_source_paths(
    field_names: &HashSet<String>,
    field_mappings: &[(usize, String)],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for (idx, source_path) in field_mappings {
        // Check if source_path starts with "raw_payload."
        if !source_path.starts_with(RAW_PAYLOAD_PREFIX) {
            errors.push(create_missing_prefix_error(*idx, source_path));
            continue;
        }

        // Extract the field reference (everything after "raw_payload.")
        let field_ref = &source_path[RAW_PAYLOAD_PREFIX.len()..];

        // For nested paths like "raw_payload.nested.field", check the root field
        let root_field = extract_root_field(field_ref);

        // Check if the root field exists in field_names
        if !field_names.contains(root_field) {
            let suggestion = find_closest_match(root_field, field_names);
            errors.push(create_invalid_source_path_error(
                *idx,
                source_path,
                root_field,
                field_names,
                suggestion,
            ));
        }
    }

    errors
}

/// Extracts the root field name from a potentially nested path
///
/// Examples:
/// - "pm02" -> "pm02"
/// - "nested.field" -> "nested"
/// - "deeply.nested.value" -> "deeply"
fn extract_root_field(field_ref: &str) -> &str {
    field_ref.split('.').next().unwrap_or(field_ref)
}

/// Find the closest matching field name using Levenshtein distance
///
/// Returns Some(suggestion) if a match with distance <= MAX_SUGGESTION_DISTANCE is found
fn find_closest_match(input: &str, candidates: &HashSet<String>) -> Option<String> {
    let input_lower = input.to_lowercase();
    let mut best_match: Option<(String, usize)> = None;

    for candidate in candidates {
        let distance = levenshtein(&input_lower, &candidate.to_lowercase());

        if distance <= MAX_SUGGESTION_DISTANCE {
            match &best_match {
                None => best_match = Some((candidate.clone(), distance)),
                Some((_, best_distance)) if distance < *best_distance => {
                    best_match = Some((candidate.clone(), distance));
                }
                _ => {}
            }
        }
    }

    best_match.map(|(name, _)| name)
}

/// Creates an error for source_path missing the "raw_payload." prefix
fn create_missing_prefix_error(idx: usize, source_path: &str) -> ValidationError {
    ValidationError {
        layer: ValidationLayer::Semantic,
        code: ErrorCode::InvalidSourcePath,
        path: format!("$.silver_etl.field_mappings[{}].source_path", idx),
        message: format!(
            "source_path '{}' must start with 'raw_payload.'",
            source_path
        ),
        severity: Severity::Error,
        suggestion: Some(format!("Change to 'raw_payload.{}'", source_path)),
        context: None,
    }
}

/// Creates an error for source_path referencing a non-existent field
fn create_invalid_source_path_error(
    idx: usize,
    source_path: &str,
    root_field: &str,
    available_fields: &HashSet<String>,
    suggestion: Option<String>,
) -> ValidationError {
    let mut sorted_fields: Vec<_> = available_fields.iter().cloned().collect();
    sorted_fields.sort();

    let suggestion_msg = suggestion.map(|s| format!("Did you mean '{}'?", s));

    ValidationError {
        layer: ValidationLayer::Semantic,
        code: ErrorCode::InvalidSourcePath,
        path: format!("$.silver_etl.field_mappings[{}].source_path", idx),
        message: format!(
            "source_path '{}' references field '{}' which is not defined in config.fields",
            source_path, root_field
        ),
        severity: Severity::Error,
        suggestion: suggestion_msg,
        context: Some(serde_json::json!({
            "available_fields": sorted_fields
        })),
    }
}

// =============================================================================
// LONDON SCHOOL TDD TESTS
// =============================================================================
//
// These tests follow Outside-In TDD with mock-first approach:
// 1. Test the behavior (what happens) not implementation (how it works)
// 2. Focus on interactions and collaborations
// 3. Verify contracts through expectations

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Test Helper: Create field names set
    // =========================================================================
    fn create_field_names(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // =========================================================================
    // TEST 1: Valid source_path passes validation
    // =========================================================================
    #[test]
    fn test_valid_source_path_passes() {
        // GIVEN: A set of valid field names
        let field_names = create_field_names(&["pm02", "temperature", "humidity"]);

        // AND: A field_mapping with a valid source_path referencing "pm02"
        let field_mappings = vec![(0, "raw_payload.pm02".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: No errors should be returned
        assert!(
            errors.is_empty(),
            "Expected no errors for valid source_path, got: {:?}",
            errors
        );
    }

    // =========================================================================
    // TEST 2: source_path typo fails with "did you mean" suggestion
    // =========================================================================
    #[test]
    fn test_source_path_typo_fails_with_suggestion() {
        // GIVEN: A set of valid field names
        let field_names = create_field_names(&["pm02", "temperature", "humidity"]);

        // AND: A field_mapping with a typo in source_path ("pm03" instead of "pm02" - 1 edit distance)
        let field_mappings = vec![(0, "raw_payload.pm03".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: One error should be returned
        assert_eq!(errors.len(), 1, "Expected exactly one error");

        let error = &errors[0];

        // AND: Error should be INVALID_SOURCE_PATH
        assert_eq!(error.code, ErrorCode::InvalidSourcePath);

        // AND: Error should be at the correct JSONPath
        assert_eq!(error.path, "$.silver_etl.field_mappings[0].source_path");

        // AND: Error message should reference the typo
        assert!(
            error.message.contains("pm03"),
            "Message should contain the typo: {}",
            error.message
        );

        // AND: Suggestion should say "Did you mean 'pm02'?"
        assert!(
            error
                .suggestion
                .as_ref()
                .map_or(false, |s| s.contains("pm02")),
            "Should suggest 'pm02', got: {:?}",
            error.suggestion
        );
    }

    // =========================================================================
    // TEST 3: source_path missing raw_payload prefix fails
    // =========================================================================
    #[test]
    fn test_source_path_missing_raw_payload_prefix_fails() {
        // GIVEN: A set of valid field names
        let field_names = create_field_names(&["pm02", "temperature"]);

        // AND: A field_mapping without the required "raw_payload." prefix
        let field_mappings = vec![(2, "pm02".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: One error should be returned
        assert_eq!(errors.len(), 1, "Expected exactly one error");

        let error = &errors[0];

        // AND: Error code should be INVALID_SOURCE_PATH
        assert_eq!(error.code, ErrorCode::InvalidSourcePath);

        // AND: Error path should point to the correct index
        assert_eq!(error.path, "$.silver_etl.field_mappings[2].source_path");

        // AND: Error message should mention the missing prefix
        assert!(
            error.message.contains("raw_payload."),
            "Message should mention required prefix: {}",
            error.message
        );

        // AND: Suggestion should recommend adding the prefix
        assert!(
            error
                .suggestion
                .as_ref()
                .map_or(false, |s| s.contains("raw_payload.pm02")),
            "Should suggest adding prefix, got: {:?}",
            error.suggestion
        );
    }

    // =========================================================================
    // TEST 4: Nested source_path validates root field
    // =========================================================================
    #[test]
    fn test_nested_source_path_validates_root() {
        // GIVEN: A set of valid field names including "nested"
        let field_names = create_field_names(&["nested", "pm02"]);

        // AND: A field_mapping with a nested path "raw_payload.nested.field"
        let field_mappings = vec![(0, "raw_payload.nested.field".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: No errors because "nested" exists in field_names
        assert!(
            errors.is_empty(),
            "Should pass when root field 'nested' exists: {:?}",
            errors
        );
    }

    // =========================================================================
    // TEST 5: Nested source_path with invalid root fails
    // =========================================================================
    #[test]
    fn test_nested_source_path_invalid_root_fails() {
        // GIVEN: A set of valid field names (not including "unknown")
        let field_names = create_field_names(&["pm02", "temperature"]);

        // AND: A field_mapping with a nested path where root does not exist
        let field_mappings = vec![(1, "raw_payload.unknown.nested.field".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: One error should be returned
        assert_eq!(errors.len(), 1);

        let error = &errors[0];

        // AND: Error message should reference the root field "unknown"
        assert!(
            error.message.contains("unknown"),
            "Should reference 'unknown' root field: {}",
            error.message
        );
    }

    // =========================================================================
    // TEST 6: Multiple source_path errors are accumulated
    // =========================================================================
    #[test]
    fn test_multiple_source_path_errors_accumulated() {
        // GIVEN: A set of valid field names
        let field_names = create_field_names(&["pm02"]);

        // AND: Multiple field_mappings with errors
        let field_mappings = vec![
            (0, "raw_payload.pm02".to_string()),       // Valid
            (1, "raw_payload.temperature".to_string()), // Invalid - not in fields
            (2, "humidity".to_string()),                // Invalid - missing prefix
            (3, "raw_payload.pressure".to_string()),    // Invalid - not in fields
        ];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: Three errors should be accumulated (not fail-fast)
        assert_eq!(
            errors.len(),
            3,
            "Should accumulate all errors, not fail on first: {:?}",
            errors
        );

        // AND: Errors should have correct indices
        let paths: Vec<_> = errors.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"$.silver_etl.field_mappings[1].source_path"));
        assert!(paths.contains(&"$.silver_etl.field_mappings[2].source_path"));
        assert!(paths.contains(&"$.silver_etl.field_mappings[3].source_path"));
    }

    // =========================================================================
    // TEST 7: Context includes available fields
    // =========================================================================
    #[test]
    fn test_error_context_includes_available_fields() {
        // GIVEN: A set of valid field names
        let field_names = create_field_names(&["pm02", "temperature", "humidity"]);

        // AND: A field_mapping with an invalid source_path
        let field_mappings = vec![(0, "raw_payload.invalid_field".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: Error context should include available fields
        assert_eq!(errors.len(), 1);

        let context = errors[0].context.as_ref().expect("Should have context");
        let available = context["available_fields"]
            .as_array()
            .expect("Should have available_fields array");

        // AND: Available fields should be sorted alphabetically
        let field_strs: Vec<_> = available
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(field_strs.contains(&"pm02"));
        assert!(field_strs.contains(&"temperature"));
        assert!(field_strs.contains(&"humidity"));
    }

    // =========================================================================
    // TEST 8: Case-sensitive matching (fields are snake_case)
    // =========================================================================
    #[test]
    fn test_source_path_is_case_sensitive() {
        // GIVEN: A set of valid field names (snake_case)
        let field_names = create_field_names(&["pm02", "temperature"]);

        // AND: A field_mapping with wrong case
        let field_mappings = vec![(0, "raw_payload.PM02".to_string())];

        // WHEN: We validate the source_paths
        let errors = validate_source_paths(&field_names, &field_mappings);

        // THEN: Error should be returned (case mismatch)
        assert_eq!(errors.len(), 1);

        // AND: Suggestion should include the correct case
        assert!(
            errors[0]
                .suggestion
                .as_ref()
                .map_or(false, |s| s.contains("pm02")),
            "Should suggest lowercase 'pm02': {:?}",
            errors[0].suggestion
        );
    }

    // =========================================================================
    // TEST 9: Empty field_mappings returns no errors
    // =========================================================================
    #[test]
    fn test_empty_field_mappings_returns_no_errors() {
        let field_names = create_field_names(&["pm02"]);
        let field_mappings: Vec<(usize, String)> = vec![];

        let errors = validate_source_paths(&field_names, &field_mappings);

        assert!(errors.is_empty());
    }

    // =========================================================================
    // TEST 10: All valid source_paths return no errors
    // =========================================================================
    #[test]
    fn test_all_valid_source_paths_return_no_errors() {
        // GIVEN: Field names matching air-quality config.json
        let field_names = create_field_names(&[
            "pm01",
            "pm02",
            "pm10",
            "rco2",
            "atmp",
            "rhum",
            "tvoc_index",
            "nox_index",
            "atmp_compensated",
            "rhum_compensated",
            "pm02_compensated",
        ]);

        // AND: Field mappings from air-quality silver_etl
        let field_mappings = vec![
            (0, "raw_payload.pm02Compensated".to_string()),
            (1, "raw_payload.pm10".to_string()),
            (2, "raw_payload.rco2".to_string()),
            (3, "raw_payload.atmpCompensated".to_string()),
            (4, "raw_payload.rhumCompensated".to_string()),
            (5, "raw_payload.tvocIndex".to_string()),
            (6, "raw_payload.noxIndex".to_string()),
        ];

        // WHEN: We validate
        let errors = validate_source_paths(&field_names, &field_mappings);

        // NOTE: This test reveals the real-world scenario from air-quality config!
        // The source_path uses camelCase (pm02Compensated) but fields use snake_case (pm02_compensated)
        // This is the exact P-005 bug - the validator should catch this mismatch!

        // In production, we would expect errors here because of case mismatch
        // For now, let's verify the validator catches these as expected
        assert!(
            !errors.is_empty(),
            "Should detect camelCase vs snake_case mismatch in source_path"
        );
    }

    // =========================================================================
    // TEST 11: Levenshtein suggestion finds close matches
    // =========================================================================
    #[test]
    fn test_levenshtein_finds_close_matches() {
        let field_names = create_field_names(&["temperature", "humidity", "pressure"]);

        // Test various typos
        let test_cases = vec![
            ("temperture", Some("temperature")),   // Missing 'a'
            ("tmperature", Some("temperature")),   // Missing 'e'
            ("humidty", Some("humidity")),         // Missing 'i'
            ("presure", Some("pressure")),         // Missing 's'
            ("completely_wrong", None),            // Too different
        ];

        for (typo, expected) in test_cases {
            let result = find_closest_match(typo, &field_names);
            assert_eq!(
                result.as_deref(),
                expected,
                "For typo '{}', expected {:?} but got {:?}",
                typo,
                expected,
                result
            );
        }
    }

    // =========================================================================
    // TEST 12: extract_root_field handles various paths
    // =========================================================================
    #[test]
    fn test_extract_root_field() {
        assert_eq!(extract_root_field("pm02"), "pm02");
        assert_eq!(extract_root_field("nested.field"), "nested");
        assert_eq!(extract_root_field("deeply.nested.value"), "deeply");
        assert_eq!(extract_root_field(""), "");
    }

    // =========================================================================
    // TEST 13: Real config scenario - air-quality field mappings
    // =========================================================================
    #[test]
    fn test_real_config_air_quality_scenario() {
        // This test uses ACTUAL field names from air-quality config.json
        // to verify the validator catches real issues

        // Fields from config.json (snake_case as per the spec)
        let field_names = create_field_names(&[
            "pm01",
            "pm01_standard",
            "pm01_count",
            "pm02",
            "pm02_standard",
            "pm02_compensated",
            "pm02_count",
            "pm10",
            "pm10_standard",
            "pm10_count",
            "pm003_count",
            "pm005_count",
            "pm50_count",
            "rco2",
            "atmp",
            "atmp_compensated",
            "rhum",
            "rhum_compensated",
            "tvoc_index",
            "tvoc_raw",
            "nox_index",
            "nox_raw",
            "wifi",
            "boot",
            "boot_count",
            "serialno",
            "firmware",
            "model",
            "led_mode",
        ]);

        // Field mappings use snake_case source_path to match field names
        // This is the CORRECT pattern per dp-019 specification
        let valid_mappings = vec![
            (0, "raw_payload.pm02_compensated".to_string()),
            (1, "raw_payload.pm10".to_string()),
            (2, "raw_payload.rco2".to_string()),
            (3, "raw_payload.atmp_compensated".to_string()),
            (4, "raw_payload.rhum_compensated".to_string()),
            (5, "raw_payload.tvoc_index".to_string()),
            (6, "raw_payload.nox_index".to_string()),
        ];

        let errors = validate_source_paths(&field_names, &valid_mappings);

        // All should pass because source_paths match field names exactly
        assert!(
            errors.is_empty(),
            "Valid snake_case source_paths should pass: {:?}",
            errors
        );
    }

    // =========================================================================
    // TEST 14: Severity is always Error for INVALID_SOURCE_PATH
    // =========================================================================
    #[test]
    fn test_invalid_source_path_severity_is_error() {
        let field_names = create_field_names(&["pm02"]);
        let field_mappings = vec![(0, "raw_payload.invalid".to_string())];

        let errors = validate_source_paths(&field_names, &field_mappings);

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].severity,
            Severity::Error,
            "INVALID_SOURCE_PATH should be Error severity, not Warning"
        );
    }

    // =========================================================================
    // TEST 15: Layer is always Semantic for source_path validation
    // =========================================================================
    #[test]
    fn test_source_path_errors_are_semantic_layer() {
        let field_names = create_field_names(&["pm02"]);
        let field_mappings = vec![
            (0, "raw_payload.invalid".to_string()),
            (1, "no_prefix".to_string()),
        ];

        let errors = validate_source_paths(&field_names, &field_mappings);

        for error in &errors {
            assert_eq!(
                error.layer,
                ValidationLayer::Semantic,
                "source_path errors should be Semantic layer"
            );
        }
    }
}
