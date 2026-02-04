//! Domain Configuration Semantic Validation
//!
//! Validates domain configuration semantics for cross-stream alignment and objectives.
//! Implements validation rules per FE-001 DECISIONS.md Decision 6.
//!
//! # Error Codes
//!
//! - `InvalidDomainStream` (404): domain references non-existent stream
//! - `CircularDomainDependency` (407): domain references itself
//! - `InvalidObjectiveCondition` (408): objective condition not supported
//! - `DuplicateName`: duplicate alias in domain

use crate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Valid objective conditions per DECISIONS.md
const VALID_CONDITIONS: &[&str] = &["<", ">", "<=", ">=", "==", "!="];

/// Valid stream roles per DECISIONS.md
const VALID_ROLES: &[&str] = &["primary", "context", "actuator", "constraint"];

/// Valid join strategies for alignment
const VALID_JOIN_STRATEGIES: &[&str] = &["full_outer", "left", "inner"];

/// Validate domain configuration semantics
///
/// # Arguments
///
/// * `domain_config` - The domain configuration JSON
/// * `available_streams` - Set of available stream IDs in the system
///
/// # Returns
///
/// Vector of validation errors (empty if valid)
pub fn validate_domain(
    domain_config: &Value,
    available_streams: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Extract domain section
    let domain = match domain_config.get("domain") {
        Some(d) => d,
        None => return errors,
    };

    // Validate stream references
    if let Some(streams) = domain.get("streams").and_then(|v| v.as_array()) {
        errors.extend(validate_domain_stream_references(
            streams,
            available_streams,
        ));
        errors.extend(validate_unique_aliases(streams));
        errors.extend(validate_has_primary(streams));
    }

    // Validate alignment configuration
    if let Some(alignment) = domain.get("alignment") {
        errors.extend(validate_alignment(alignment));
    }

    // Validate objectives
    if let Some(objectives) = domain.get("objectives").and_then(|v| v.as_array()) {
        // Build stream_id -> alias map for objective validation
        let stream_map: HashMap<String, String> = domain
            .get("streams")
            .and_then(|v| v.as_array())
            .map(|streams| {
                streams
                    .iter()
                    .filter_map(|s| {
                        let stream_id = s.get("stream_id").and_then(|v| v.as_str())?;
                        let alias = s.get("alias").and_then(|v| v.as_str()).unwrap_or(stream_id);
                        Some((stream_id.to_string(), alias.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        errors.extend(validate_objectives(objectives, &stream_map));
    }

    // Validate constraints
    if let Some(constraints) = domain.get("constraints").and_then(|v| v.as_array()) {
        let stream_ids: HashSet<String> = domain
            .get("streams")
            .and_then(|v| v.as_array())
            .map(|streams| {
                streams
                    .iter()
                    .filter_map(|s| s.get("stream_id").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        errors.extend(validate_constraints(constraints, &stream_ids));
    }

    errors
}

/// Validate that all stream_ids in domain.streams exist
fn validate_domain_stream_references(
    streams: &[Value],
    available_streams: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for (idx, stream) in streams.iter().enumerate() {
        if let Some(stream_id) = stream.get("stream_id").and_then(|v| v.as_str()) {
            if !available_streams.contains(stream_id) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidDomainStream,
                    path: format!("$.domain.streams[{}].stream_id", idx),
                    message: format!(
                        "Stream '{}' not found. Available streams: {}",
                        stream_id,
                        format_stream_list(available_streams)
                    ),
                    severity: Severity::Error,
                    suggestion: find_closest_stream(stream_id, available_streams),
                    context: None,
                });
            }

            // Validate role if present
            if let Some(role) = stream.get("role").and_then(|v| v.as_str()) {
                if !VALID_ROLES.contains(&role) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidDomainStream,
                        path: format!("$.domain.streams[{}].role", idx),
                        message: format!(
                            "Invalid role '{}'. Valid roles: {}",
                            role,
                            VALID_ROLES.join(", ")
                        ),
                        severity: Severity::Error,
                        suggestion: None,
                        context: None,
                    });
                }
            }
        }
    }

    errors
}

/// Validate no duplicate aliases in domain
fn validate_unique_aliases(streams: &[Value]) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut seen_aliases: HashSet<String> = HashSet::new();

    for (idx, stream) in streams.iter().enumerate() {
        let alias = stream
            .get("alias")
            .and_then(|v| v.as_str())
            .or_else(|| stream.get("stream_id").and_then(|v| v.as_str()))
            .unwrap_or("");

        if !alias.is_empty() {
            if seen_aliases.contains(alias) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::DuplicateName,
                    path: format!("$.domain.streams[{}].alias", idx),
                    message: format!("Duplicate alias '{}' in domain streams", alias),
                    severity: Severity::Error,
                    suggestion: Some("Each stream must have a unique alias".to_string()),
                    context: None,
                });
            } else {
                seen_aliases.insert(alias.to_string());
            }
        }
    }

    errors
}

/// Validate at least one stream has role=primary
fn validate_has_primary(streams: &[Value]) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let has_primary = streams.iter().any(|s| {
        s.get("role")
            .and_then(|v| v.as_str())
            .map(|r| r == "primary")
            .unwrap_or(false)
    });

    if !has_primary && !streams.is_empty() {
        errors.push(ValidationError {
            layer: ValidationLayer::Semantic,
            code: ErrorCode::InvalidDomainStream,
            path: "$.domain.streams".to_string(),
            message: "Domain must have at least one stream with role: primary".to_string(),
            severity: Severity::Warning,
            suggestion: Some("Add role: primary to the main stream being optimized".to_string()),
            context: None,
        });
    }

    errors
}

/// Validate alignment configuration
fn validate_alignment(alignment: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Validate join_strategy if present
    if let Some(strategy) = alignment.get("join_strategy").and_then(|v| v.as_str()) {
        if !VALID_JOIN_STRATEGIES.contains(&strategy) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidDomainStream,
                path: "$.domain.alignment.join_strategy".to_string(),
                message: format!(
                    "Invalid join_strategy '{}'. Valid strategies: {}",
                    strategy,
                    VALID_JOIN_STRATEGIES.join(", ")
                ),
                severity: Severity::Error,
                suggestion: None,
                context: None,
            });
        }
    }

    // Validate granularity format if present
    if let Some(granularity) = alignment.get("granularity").and_then(|v| v.as_str()) {
        if !is_valid_granularity(granularity) {
            errors.push(ValidationError {
                layer: ValidationLayer::Semantic,
                code: ErrorCode::InvalidGranularity,
                path: "$.domain.alignment.granularity".to_string(),
                message: format!(
                    "Invalid granularity format '{}'. Expected format: '<number> <unit>'",
                    granularity
                ),
                severity: Severity::Error,
                suggestion: Some("Examples: '1 hour', '15 minutes'".to_string()),
                context: None,
            });
        }
    }

    errors
}

/// Validate objectives configuration
fn validate_objectives(
    objectives: &[Value],
    stream_map: &HashMap<String, String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for (idx, objective) in objectives.iter().enumerate() {
        // Check for duplicate IDs
        if let Some(id) = objective.get("id").and_then(|v| v.as_str()) {
            if seen_ids.contains(id) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::DuplicateName,
                    path: format!("$.domain.objectives[{}].id", idx),
                    message: format!("Duplicate objective ID '{}'", id),
                    severity: Severity::Error,
                    suggestion: None,
                    context: None,
                });
            } else {
                seen_ids.insert(id.to_string());
            }
        }

        // Validate target stream exists
        if let Some(target) = objective.get("target") {
            if let Some(stream) = target.get("stream").and_then(|v| v.as_str()) {
                if !stream_map.contains_key(stream) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidDomainStream,
                        path: format!("$.domain.objectives[{}].target.stream", idx),
                        message: format!(
                            "Objective references stream '{}' which is not in this domain",
                            stream
                        ),
                        severity: Severity::Error,
                        suggestion: None,
                        context: None,
                    });
                }
            }

            // Validate condition
            if let Some(condition) = target.get("condition").and_then(|v| v.as_str()) {
                if !VALID_CONDITIONS.contains(&condition) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        code: ErrorCode::InvalidObjectiveCondition,
                        path: format!("$.domain.objectives[{}].target.condition", idx),
                        message: format!(
                            "Invalid condition '{}'. Valid conditions: {}",
                            condition,
                            VALID_CONDITIONS.join(", ")
                        ),
                        severity: Severity::Error,
                        suggestion: None,
                        context: None,
                    });
                }
            }
        }
    }

    errors
}

/// Validate constraints configuration
fn validate_constraints(
    constraints: &[Value],
    stream_ids: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for (idx, constraint) in constraints.iter().enumerate() {
        // Check for duplicate IDs
        if let Some(id) = constraint.get("id").and_then(|v| v.as_str()) {
            if seen_ids.contains(id) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::DuplicateName,
                    path: format!("$.domain.constraints[{}].id", idx),
                    message: format!("Duplicate constraint ID '{}'", id),
                    severity: Severity::Error,
                    suggestion: None,
                    context: None,
                });
            } else {
                seen_ids.insert(id.to_string());
            }
        }

        // Validate stream reference
        if let Some(stream) = constraint.get("stream").and_then(|v| v.as_str()) {
            if !stream_ids.contains(stream) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidDomainStream,
                    path: format!("$.domain.constraints[{}].stream", idx),
                    message: format!(
                        "Constraint references stream '{}' which is not in this domain",
                        stream
                    ),
                    severity: Severity::Error,
                    suggestion: None,
                    context: None,
                });
            }
        }

        // Validate condition
        if let Some(condition) = constraint.get("condition").and_then(|v| v.as_str()) {
            if !VALID_CONDITIONS.contains(&condition) {
                errors.push(ValidationError {
                    layer: ValidationLayer::Semantic,
                    code: ErrorCode::InvalidObjectiveCondition,
                    path: format!("$.domain.constraints[{}].condition", idx),
                    message: format!(
                        "Invalid condition '{}'. Valid conditions: {}",
                        condition,
                        VALID_CONDITIONS.join(", ")
                    ),
                    severity: Severity::Error,
                    suggestion: None,
                    context: None,
                });
            }
        }
    }

    errors
}

/// Check if a granularity string is valid
fn is_valid_granularity(granularity: &str) -> bool {
    let pattern = regex::Regex::new(r"^\d+\s+(minute|hour|day)s?$").unwrap();
    pattern.is_match(granularity)
}

/// Find the closest matching stream using Levenshtein distance
fn find_closest_stream(input: &str, candidates: &HashSet<String>) -> Option<String> {
    use strsim::levenshtein;

    let input_lower = input.to_lowercase();
    let mut best_match: Option<(String, usize)> = None;

    for candidate in candidates {
        let distance = levenshtein(&input_lower, &candidate.to_lowercase());

        if distance <= 3 {
            match &best_match {
                None => best_match = Some((candidate.clone(), distance)),
                Some((_, best_distance)) if distance < *best_distance => {
                    best_match = Some((candidate.clone(), distance));
                }
                _ => {}
            }
        }
    }

    best_match.map(|(name, _)| format!("Did you mean '{}'?", name))
}

/// Format stream list for error messages
fn format_stream_list(streams: &HashSet<String>) -> String {
    let mut sorted: Vec<_> = streams.iter().cloned().collect();
    sorted.sort();
    if sorted.len() > 5 {
        format!(
            "{}, ... ({} more)",
            sorted[..5].join(", "),
            sorted.len() - 5
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

    fn make_streams(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // =========================================================================
    // Test 1: Valid domain configuration passes
    // =========================================================================
    #[test]
    fn test_valid_domain_passes() {
        let available = make_streams(&["air-quality", "outdoor-weather", "home-assistant-state"]);
        let config = json!({
            "domain": {
                "id": "indoor-air-quality",
                "streams": [
                    { "stream_id": "air-quality", "alias": "indoor", "role": "primary" },
                    { "stream_id": "outdoor-weather", "alias": "outdoor", "role": "context" }
                ],
                "alignment": {
                    "granularity": "1 hour",
                    "join_strategy": "full_outer"
                },
                "objectives": [
                    {
                        "id": "healthy_co2",
                        "target": {
                            "stream": "air-quality",
                            "metric": "co2",
                            "condition": "<",
                            "threshold": 800
                        }
                    }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // =========================================================================
    // Test 2: Unknown stream fails
    // =========================================================================
    #[test]
    fn test_domain_unknown_stream_fails() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "nonexistent-stream", "role": "primary" }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDomainStream);
        assert!(errors[0].message.contains("nonexistent-stream"));
    }

    // =========================================================================
    // Test 3: Duplicate alias fails
    // =========================================================================
    #[test]
    fn test_duplicate_alias_fails() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "alias": "data", "role": "primary" },
                    { "stream_id": "outdoor-weather", "alias": "data", "role": "context" }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(|e| e.code == ErrorCode::DuplicateName));
        assert!(errors.iter().any(|e| e.message.contains("Duplicate alias")));
    }

    // =========================================================================
    // Test 4: Missing primary stream warns
    // =========================================================================
    #[test]
    fn test_missing_primary_warns() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "context" },
                    { "stream_id": "outdoor-weather", "role": "context" }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.message.contains("role: primary") && e.severity == Severity::Warning));
    }

    // =========================================================================
    // Test 5: Invalid objective condition fails
    // =========================================================================
    #[test]
    fn test_invalid_objective_condition_fails() {
        let available = make_streams(&["air-quality"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "primary" }
                ],
                "objectives": [
                    {
                        "id": "test",
                        "target": {
                            "stream": "air-quality",
                            "condition": "approx"
                        }
                    }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidObjectiveCondition));
    }

    // =========================================================================
    // Test 6: Objective referencing non-domain stream fails
    // =========================================================================
    #[test]
    fn test_objective_unknown_stream_fails() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "primary" }
                ],
                "objectives": [
                    {
                        "id": "test",
                        "target": {
                            "stream": "outdoor-weather",
                            "condition": "<"
                        }
                    }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidDomainStream
                && e.message.contains("outdoor-weather")));
    }

    // =========================================================================
    // Test 7: Invalid join strategy fails
    // =========================================================================
    #[test]
    fn test_invalid_join_strategy_fails() {
        let available = make_streams(&["air-quality"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "primary" }
                ],
                "alignment": {
                    "join_strategy": "cross_join"
                }
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(|e| e.message.contains("join_strategy")));
    }

    // =========================================================================
    // Test 8: Invalid alignment granularity fails
    // =========================================================================
    #[test]
    fn test_invalid_alignment_granularity_fails() {
        let available = make_streams(&["air-quality"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "primary" }
                ],
                "alignment": {
                    "granularity": "hourly"
                }
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidGranularity));
    }

    // =========================================================================
    // Test 9: Invalid stream role fails
    // =========================================================================
    #[test]
    fn test_invalid_stream_role_fails() {
        let available = make_streams(&["air-quality"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "unknown_role" }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(
            |e| e.code == ErrorCode::InvalidDomainStream && e.message.contains("Invalid role")
        ));
    }

    // =========================================================================
    // Test 10: Duplicate objective ID fails
    // =========================================================================
    #[test]
    fn test_duplicate_objective_id_fails() {
        let available = make_streams(&["air-quality"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "primary" }
                ],
                "objectives": [
                    { "id": "same_id", "target": { "stream": "air-quality", "condition": "<" } },
                    { "id": "same_id", "target": { "stream": "air-quality", "condition": ">" } }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(
            |e| e.code == ErrorCode::DuplicateName && e.message.contains("Duplicate objective")
        ));
    }

    // =========================================================================
    // Test 11: Missing domain section returns no errors
    // =========================================================================
    #[test]
    fn test_missing_domain_returns_no_errors() {
        let available = make_streams(&["air-quality"]);
        let config = json!({
            "some_other_field": true
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.is_empty());
    }

    // =========================================================================
    // Test 12: Constraint with invalid stream fails
    // =========================================================================
    #[test]
    fn test_constraint_invalid_stream_fails() {
        let available = make_streams(&["air-quality", "outdoor-air-quality"]);
        let config = json!({
            "domain": {
                "id": "test",
                "streams": [
                    { "stream_id": "air-quality", "role": "primary" }
                ],
                "constraints": [
                    {
                        "id": "outdoor_safe",
                        "stream": "outdoor-air-quality",
                        "condition": "<"
                    }
                ]
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidDomainStream
                && e.message.contains("outdoor-air-quality")));
    }
}
