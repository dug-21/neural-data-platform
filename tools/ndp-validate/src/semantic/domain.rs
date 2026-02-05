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
//!
//! # Usage
//!
//! ```ignore
//! use ndp_validate::semantic::domain::{validate_domain, validate_domain_semantic};
//!
//! // With known streams
//! let available_streams: HashSet<String> = /* ... */;
//! let errors = validate_domain(&domain_config, &available_streams);
//!
//! // Standalone validation (discovers streams from config dir)
//! let errors = validate_domain_semantic(&domain_config, Some(Path::new("config/base/streams")));
//! ```

use crate::error::{ErrorCode, Severity, ValidationError, ValidationLayer};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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
/// * `domain_config` - The domain configuration JSON (FLAT format, no wrapper)
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

    // FE-002 B0: FLAT format - process config directly, no wrapper extraction
    // The domain config should have id, streams, alignment at root level

    // Validate stream references
    if let Some(streams) = domain_config.get("streams").and_then(|v| v.as_array()) {
        errors.extend(validate_domain_stream_references(
            streams,
            available_streams,
        ));
        errors.extend(validate_unique_aliases(streams));
        errors.extend(validate_has_primary(streams));
    }

    // Validate alignment configuration
    if let Some(alignment) = domain_config.get("alignment") {
        errors.extend(validate_alignment(alignment));
    }

    // Validate objectives
    if let Some(objectives) = domain_config.get("objectives").and_then(|v| v.as_array()) {
        // Build stream_id -> alias map for objective validation
        let stream_map: HashMap<String, String> = domain_config
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
    if let Some(constraints) = domain_config.get("constraints").and_then(|v| v.as_array()) {
        let stream_ids: HashSet<String> = domain_config
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
                    path: format!("$.streams[{}].stream_id", idx),
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
                        path: format!("$.streams[{}].role", idx),
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
                    path: format!("$.streams[{}].alias", idx),
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
            path: "$.streams".to_string(),
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
                path: "$.alignment.join_strategy".to_string(),
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
                path: "$.alignment.granularity".to_string(),
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
                    path: format!("$.objectives[{}].id", idx),
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
                        path: format!("$.objectives[{}].target.stream", idx),
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
                        path: format!("$.objectives[{}].target.condition", idx),
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
                    path: format!("$.constraints[{}].id", idx),
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
                    path: format!("$.constraints[{}].stream", idx),
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
                    path: format!("$.constraints[{}].condition", idx),
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
// Standalone Semantic Validation (FE-002 Phase B)
// =============================================================================

/// Validate domain configuration semantics (standalone version)
///
/// This function discovers available streams from the config directory
/// and validates the domain configuration against them.
///
/// # Arguments
///
/// * `domain_config` - The domain configuration JSON (FLAT format)
/// * `streams_dir` - Optional path to streams config directory
///
/// # Returns
///
/// Vector of validation errors (empty if valid)
///
/// # Example
///
/// ```ignore
/// let config: Value = serde_json::from_str(&content)?;
/// let errors = validate_domain_semantic(&config, Some(Path::new("config/base/streams")));
/// ```
pub fn validate_domain_semantic(
    domain_config: &Value,
    streams_dir: Option<&Path>,
) -> Vec<ValidationError> {
    // Discover available streams from config directory
    let available_streams = match streams_dir {
        Some(dir) => discover_streams(dir),
        None => {
            // Try default locations
            let default_paths = [
                Path::new("config/base/streams"),
                Path::new("config/integration/base/streams"),
            ];

            default_paths
                .iter()
                .find(|p| p.exists())
                .map(|p| discover_streams(p))
                .unwrap_or_default()
        }
    };

    // Validate the domain
    validate_domain(domain_config, &available_streams)
}

/// Discover available stream IDs from the config directory
///
/// Searches for directories containing config.yaml or config.json files
/// and extracts stream_id from the info section or uses directory name.
fn discover_streams(streams_dir: &Path) -> HashSet<String> {
    let mut streams = HashSet::new();

    if !streams_dir.exists() || !streams_dir.is_dir() {
        return streams;
    }

    // Read directory entries
    if let Ok(entries) = std::fs::read_dir(streams_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Only process directories
            if !path.is_dir() {
                continue;
            }

            // Try to find config file
            let config_paths = [
                path.join("config.yaml"),
                path.join("config.yml"),
                path.join("config.json"),
            ];

            for config_path in &config_paths {
                if config_path.exists() {
                    // Try to extract stream_id from config
                    if let Some(stream_id) = extract_stream_id(config_path) {
                        streams.insert(stream_id);
                    } else {
                        // Fall back to directory name
                        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                            streams.insert(dir_name.to_string());
                        }
                    }
                    break;
                }
            }
        }
    }

    streams
}

/// Extract stream_id from a config file
fn extract_stream_id(config_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(config_path).ok()?;

    // Try JSON first
    if config_path
        .extension()
        .map(|e| e == "json")
        .unwrap_or(false)
    {
        let value: Value = serde_json::from_str(&content).ok()?;
        // Try info.stream_id (wrapped) or stream_id (flat)
        value
            .get("info")
            .and_then(|i| i.get("stream_id"))
            .or_else(|| value.get("stream_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        // Try YAML
        let value: Value = serde_yaml::from_str(&content).ok()?;
        value
            .get("info")
            .and_then(|i| i.get("stream_id"))
            .or_else(|| value.get("stream_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
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
    // Test 1: Valid domain configuration passes (FLAT format)
    // =========================================================================
    #[test]
    fn test_valid_domain_passes() {
        let available = make_streams(&["air-quality", "outdoor-weather", "home-assistant-state"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
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
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // =========================================================================
    // Test 2: Unknown stream fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_domain_unknown_stream_fails() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "nonexistent-stream", "role": "primary" }
            ]
        });

        let errors = validate_domain(&config, &available);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::InvalidDomainStream);
        assert!(errors[0].message.contains("nonexistent-stream"));
    }

    // =========================================================================
    // Test 3: Duplicate alias fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_duplicate_alias_fails() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "air-quality", "alias": "data", "role": "primary" },
                { "stream_id": "outdoor-weather", "alias": "data", "role": "context" }
            ]
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(|e| e.code == ErrorCode::DuplicateName));
        assert!(errors.iter().any(|e| e.message.contains("Duplicate alias")));
    }

    // =========================================================================
    // Test 4: Missing primary stream warns (FLAT format)
    // =========================================================================
    #[test]
    fn test_missing_primary_warns() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "air-quality", "role": "context" },
                { "stream_id": "outdoor-weather", "role": "context" }
            ]
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.message.contains("role: primary") && e.severity == Severity::Warning));
    }

    // =========================================================================
    // Test 5: Invalid objective condition fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_invalid_objective_condition_fails() {
        let available = make_streams(&["air-quality"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
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
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidObjectiveCondition));
    }

    // =========================================================================
    // Test 6: Objective referencing non-domain stream fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_objective_unknown_stream_fails() {
        let available = make_streams(&["air-quality", "outdoor-weather"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
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
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidDomainStream
                && e.message.contains("outdoor-weather")));
    }

    // =========================================================================
    // Test 7: Invalid join strategy fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_invalid_join_strategy_fails() {
        let available = make_streams(&["air-quality"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "air-quality", "role": "primary" }
            ],
            "alignment": {
                "join_strategy": "cross_join"
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(|e| e.message.contains("join_strategy")));
    }

    // =========================================================================
    // Test 8: Invalid alignment granularity fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_invalid_alignment_granularity_fails() {
        let available = make_streams(&["air-quality"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "air-quality", "role": "primary" }
            ],
            "alignment": {
                "granularity": "hourly"
            }
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidGranularity));
    }

    // =========================================================================
    // Test 9: Invalid stream role fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_invalid_stream_role_fails() {
        let available = make_streams(&["air-quality"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "air-quality", "role": "unknown_role" }
            ]
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(
            |e| e.code == ErrorCode::InvalidDomainStream && e.message.contains("Invalid role")
        ));
    }

    // =========================================================================
    // Test 10: Duplicate objective ID fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_duplicate_objective_id_fails() {
        let available = make_streams(&["air-quality"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
            "id": "test",
            "streams": [
                { "stream_id": "air-quality", "role": "primary" }
            ],
            "objectives": [
                { "id": "same_id", "target": { "stream": "air-quality", "condition": "<" } },
                { "id": "same_id", "target": { "stream": "air-quality", "condition": ">" } }
            ]
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.iter().any(
            |e| e.code == ErrorCode::DuplicateName && e.message.contains("Duplicate objective")
        ));
    }

    // =========================================================================
    // Test 11: Empty config with no streams returns no errors (FLAT format)
    // =========================================================================
    #[test]
    fn test_empty_config_returns_no_errors() {
        let available = make_streams(&["air-quality"]);
        // FE-002 B0: FLAT format - config with unrelated fields
        // The validator gracefully handles configs without streams
        let config = json!({
            "some_other_field": true
        });

        let errors = validate_domain(&config, &available);
        assert!(errors.is_empty());
    }

    // =========================================================================
    // Test 12: Constraint with invalid stream fails (FLAT format)
    // =========================================================================
    #[test]
    fn test_constraint_invalid_stream_fails() {
        let available = make_streams(&["air-quality", "outdoor-air-quality"]);
        // FE-002 B0: FLAT format - no "domain" wrapper
        let config = json!({
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
        });

        let errors = validate_domain(&config, &available);
        assert!(errors
            .iter()
            .any(|e| e.code == ErrorCode::InvalidDomainStream
                && e.message.contains("outdoor-air-quality")));
    }

    // =========================================================================
    // Test 13: FLAT format matches actual domain.json structure (FE-002 B0)
    // =========================================================================
    #[test]
    fn test_flat_format_matches_domain_json() {
        // This test validates against the exact structure in
        // config/domains/indoor-air-quality/domain.json
        let available = make_streams(&[
            "air-quality",
            "outdoor-weather",
            "home-assistant-state",
            "outdoor-air-quality",
        ]);

        // Real domain.json structure (FLAT format)
        let config = json!({
            "id": "indoor-air-quality",
            "description": "Maintain healthy indoor air quality",
            "streams": [
                { "stream_id": "air-quality", "alias": "indoor", "role": "primary" },
                { "stream_id": "outdoor-weather", "alias": "outdoor", "role": "context" },
                { "stream_id": "home-assistant-state", "alias": "state", "role": "actuator", "null_handling": "carry_forward" },
                { "stream_id": "outdoor-air-quality", "alias": "outdoor_aqi", "role": "constraint" }
            ],
            "alignment": {
                "view_name": "indoor_air_quality_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            },
            "objectives": [
                {
                    "id": "healthy_co2",
                    "description": "Keep CO2 below 800 ppm for cognitive performance",
                    "target": {
                        "stream": "air-quality",
                        "metric": "co2",
                        "condition": "<",
                        "threshold": 800,
                        "unit": "ppm"
                    },
                    "priority": "high"
                }
            ]
        });

        let errors = validate_domain(&config, &available);
        assert!(
            errors.is_empty(),
            "FLAT format validation failed: {:?}",
            errors
        );
    }
}
