//! Schema generation from ndp-types.
//!
//! This module generates JSON Schema from Rust types using schemars,
//! ensuring the schema is always in sync with runtime types.
//!
//! ## Features
//!
//! - `generate_schema()` - Generate JSON Schema to stdout or file
//! - `verify_schema()` - Compare committed schema against generated
//!
//! ## CLI Usage
//!
//! ```bash
//! # Generate schema to stdout
//! ndp-validate --generate-schema
//!
//! # Generate schema to file
//! ndp-validate --generate-schema --output schemas/stream-config.v1.2.schema.json
//!
//! # Verify committed schema matches (for CI)
//! ndp-validate --verify-schema schemas/stream-config.v1.2.schema.json
//! ```

use ndp_types::{SourceType, FieldType, DqRuleType, DqAction, MonotonicDirection};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Schema generation error
#[derive(Debug, thiserror::Error)]
pub enum SchemaGenError {
    #[error("Failed to serialize schema: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Failed to read schema file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Schema drift detected: {0}")]
    SchemaDrift(String),
}

/// Result type for schema generation operations.
pub type SchemaGenResult<T> = Result<T, SchemaGenError>;

/// Combined schema for NDP types.
///
/// This struct is used to generate a single schema containing all
/// NDP type definitions. It's not used at runtime.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "NDP Types Schema")]
struct NdpTypesSchema {
    /// Example source type field for schema generation
    #[schemars(description = "Data source type")]
    source_type: SourceType,

    /// Example field type field for schema generation
    #[schemars(description = "Field data type")]
    field_type: FieldType,

    /// Example DQ rule type field for schema generation
    #[schemars(description = "Data quality rule type")]
    dq_rule_type: DqRuleType,

    /// Example DQ action field for schema generation
    #[schemars(description = "Data quality action on failure")]
    dq_action: DqAction,

    /// Example monotonic direction for schema generation
    #[schemars(description = "Monotonic check direction")]
    monotonic_direction: MonotonicDirection,
}

/// Generate JSON Schema from ndp-types.
///
/// Returns a JSON Schema document containing definitions for all
/// NDP configuration types (SourceType, FieldType, DqRuleType, etc.).
///
/// # Returns
///
/// A pretty-printed JSON string containing the schema.
///
/// # Example
///
/// ```rust,ignore
/// use ndp_validate::schema_gen::generate_schema;
///
/// let schema_json = generate_schema()?;
/// println!("{}", schema_json);
/// ```
pub fn generate_schema() -> SchemaGenResult<String> {
    // Generate schema from the combined types struct
    let root_schema = schema_for!(NdpTypesSchema);

    // Convert to JSON Value for manipulation
    let mut schema_value = serde_json::to_value(&root_schema)?;

    // Add standard JSON Schema metadata
    if let Some(obj) = schema_value.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            serde_json::json!("https://json-schema.org/draft/2020-12/schema"),
        );
        obj.insert(
            "$id".to_string(),
            serde_json::json!("https://ndp.local/schemas/ndp-types.v1.0.json"),
        );
        obj.insert(
            "title".to_string(),
            serde_json::json!("NDP Configuration Types"),
        );
        obj.insert(
            "description".to_string(),
            serde_json::json!("JSON Schema for NDP configuration types. Generated from ndp-types crate."),
        );

        // Add generation metadata
        obj.insert(
            "x-ndp-version".to_string(),
            serde_json::json!("1.0.0"),
        );
        obj.insert(
            "x-generated-from".to_string(),
            serde_json::json!("ndp-types crate via schemars"),
        );
    }

    // Return pretty-printed JSON
    Ok(serde_json::to_string_pretty(&schema_value)?)
}

/// Generate schema for a specific type.
///
/// Returns a JSON Schema document for the specified type only.
pub fn generate_type_schema<T: JsonSchema>() -> SchemaGenResult<String> {
    let root_schema = schema_for!(T);
    Ok(serde_json::to_string_pretty(&root_schema)?)
}

/// Verify that a committed schema file matches the generated schema.
///
/// # Arguments
///
/// * `path` - Path to the committed schema file
///
/// # Returns
///
/// - `Ok(true)` if schemas match
/// - `Ok(false)` if schemas differ (drift detected)
/// - `Err(...)` if file cannot be read or parsed
///
/// # Example
///
/// ```rust,ignore
/// use ndp_validate::schema_gen::verify_schema;
/// use std::path::Path;
///
/// match verify_schema(Path::new("schemas/ndp-types.json"))? {
///     true => println!("Schema matches!"),
///     false => eprintln!("Schema drift detected!"),
/// }
/// ```
pub fn verify_schema(path: &Path) -> SchemaGenResult<bool> {
    // Read existing schema from file
    let existing_content = std::fs::read_to_string(path)?;
    let existing_value: serde_json::Value = serde_json::from_str(&existing_content)?;

    // Generate fresh schema
    let generated_content = generate_schema()?;
    let generated_value: serde_json::Value = serde_json::from_str(&generated_content)?;

    // Normalize both for comparison (remove volatile fields)
    let existing_normalized = normalize_schema(&existing_value);
    let generated_normalized = normalize_schema(&generated_value);

    // Compare
    Ok(existing_normalized == generated_normalized)
}

/// Compare schemas and return list of differences.
///
/// # Arguments
///
/// * `path` - Path to the committed schema file
///
/// # Returns
///
/// A vector of difference descriptions. Empty if schemas match.
pub fn compare_schemas(path: &Path) -> SchemaGenResult<Vec<SchemaDifference>> {
    // Read existing schema from file
    let existing_content = std::fs::read_to_string(path)?;
    let existing_value: serde_json::Value = serde_json::from_str(&existing_content)?;

    // Generate fresh schema
    let generated_content = generate_schema()?;
    let generated_value: serde_json::Value = serde_json::from_str(&generated_content)?;

    // Normalize both for comparison
    let existing_normalized = normalize_schema(&existing_value);
    let generated_normalized = normalize_schema(&generated_value);

    // Find differences
    let mut differences = Vec::new();
    find_differences("$", &existing_normalized, &generated_normalized, &mut differences);

    Ok(differences)
}

/// A difference between two schema values.
#[derive(Debug, Clone)]
pub struct SchemaDifference {
    /// JSONPath to the difference
    pub path: String,
    /// Description of the difference
    pub description: String,
    /// Value in existing schema (if present)
    pub existing: Option<serde_json::Value>,
    /// Value in generated schema (if present)
    pub generated: Option<serde_json::Value>,
}

impl std::fmt::Display for SchemaDifference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.description)
    }
}

/// Normalize schema for comparison by removing volatile fields.
fn normalize_schema(schema: &serde_json::Value) -> serde_json::Value {
    let mut normalized = schema.clone();

    // Remove fields that may vary between generations
    if let Some(obj) = normalized.as_object_mut() {
        obj.remove("x-generated-at");
        // Keep x-ndp-version and x-generated-from for intentional change detection
    }

    // Sort object keys recursively for consistent comparison
    sort_keys_recursive(&mut normalized);

    normalized
}

/// Recursively sort object keys for consistent comparison.
fn sort_keys_recursive(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // serde_json::Map doesn't support sorting, so we rebuild it
            let sorted: BTreeMap<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| {
                    let mut v = v.clone();
                    sort_keys_recursive(&mut v);
                    (k.clone(), v)
                })
                .collect();

            *map = sorted.into_iter().collect();
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                sort_keys_recursive(item);
            }
        }
        _ => {}
    }
}

/// Recursively find differences between two JSON values.
fn find_differences(
    path: &str,
    existing: &serde_json::Value,
    generated: &serde_json::Value,
    differences: &mut Vec<SchemaDifference>,
) {
    if existing == generated {
        return;
    }

    match (existing, generated) {
        (serde_json::Value::Object(e_map), serde_json::Value::Object(g_map)) => {
            // Check for missing keys in existing
            for key in g_map.keys() {
                if !e_map.contains_key(key) {
                    differences.push(SchemaDifference {
                        path: format!("{}.{}", path, key),
                        description: format!("Missing key '{}' in existing schema", key),
                        existing: None,
                        generated: Some(g_map[key].clone()),
                    });
                }
            }

            // Check for extra keys in existing
            for key in e_map.keys() {
                if !g_map.contains_key(key) {
                    differences.push(SchemaDifference {
                        path: format!("{}.{}", path, key),
                        description: format!("Extra key '{}' in existing schema (not in generated)", key),
                        existing: Some(e_map[key].clone()),
                        generated: None,
                    });
                }
            }

            // Recurse into shared keys
            for key in g_map.keys() {
                if let Some(e_val) = e_map.get(key) {
                    let g_val = &g_map[key];
                    find_differences(
                        &format!("{}.{}", path, key),
                        e_val,
                        g_val,
                        differences,
                    );
                }
            }
        }
        (serde_json::Value::Array(e_arr), serde_json::Value::Array(g_arr)) => {
            // Check if this is an enum array (special handling)
            if path.ends_with(".enum") {
                // Compare as sets for enum arrays
                let e_set: std::collections::HashSet<_> = e_arr.iter().collect();
                let g_set: std::collections::HashSet<_> = g_arr.iter().collect();

                let missing: Vec<serde_json::Value> =
                    g_set.difference(&e_set).map(|v| (*v).clone()).collect();
                let extra: Vec<serde_json::Value> =
                    e_set.difference(&g_set).map(|v| (*v).clone()).collect();

                if !missing.is_empty() {
                    differences.push(SchemaDifference {
                        path: path.to_string(),
                        description: format!("Missing enum values: {:?}", missing),
                        existing: None,
                        generated: Some(serde_json::Value::Array(missing)),
                    });
                }

                if !extra.is_empty() {
                    differences.push(SchemaDifference {
                        path: path.to_string(),
                        description: format!("Extra enum values not in generated: {:?}", extra),
                        existing: Some(serde_json::Value::Array(extra)),
                        generated: None,
                    });
                }
            } else {
                // Compare arrays element by element
                let max_len = e_arr.len().max(g_arr.len());
                for i in 0..max_len {
                    let e_val = e_arr.get(i);
                    let g_val = g_arr.get(i);

                    match (e_val, g_val) {
                        (Some(e), Some(g)) => {
                            find_differences(&format!("{}[{}]", path, i), e, g, differences);
                        }
                        (Some(e), None) => {
                            differences.push(SchemaDifference {
                                path: format!("{}[{}]", path, i),
                                description: "Extra array element in existing".to_string(),
                                existing: Some(e.clone()),
                                generated: None,
                            });
                        }
                        (None, Some(g)) => {
                            differences.push(SchemaDifference {
                                path: format!("{}[{}]", path, i),
                                description: "Missing array element in existing".to_string(),
                                existing: None,
                                generated: Some(g.clone()),
                            });
                        }
                        (None, None) => {}
                    }
                }
            }
        }
        _ => {
            // Primitive value mismatch
            differences.push(SchemaDifference {
                path: path.to_string(),
                description: format!(
                    "Value mismatch: existing={:?}, generated={:?}",
                    existing, generated
                ),
                existing: Some(existing.clone()),
                generated: Some(generated.clone()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    // TC-401: Generate Schema Produces Valid JSON
    #[test]
    fn test_generate_schema_produces_valid_json() {
        let schema_json = generate_schema().expect("Should generate schema");

        // Parse to verify it's valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&schema_json).expect("Should be valid JSON");

        // Check required fields
        assert!(parsed.get("$schema").is_some(), "Should have $schema");
        assert!(parsed.get("title").is_some(), "Should have title");
    }

    // TC-402: Schema Includes All SourceType Variants
    #[test]
    fn test_schema_includes_all_source_types() {
        let schema_json = generate_schema().expect("Should generate schema");
        let schema: serde_json::Value = serde_json::from_str(&schema_json).unwrap();

        // Find SourceType definition in $defs or definitions
        let _source_type_def = schema
            .pointer("/definitions/SourceType")
            .or_else(|| schema.pointer("/$defs/SourceType"));

        // SourceType should be present somewhere in the schema
        // (it may be inlined or in definitions)
        assert!(
            schema_json.contains("mqtt"),
            "Schema should contain 'mqtt'"
        );
        assert!(
            schema_json.contains("http_poll"),
            "Schema should contain 'http_poll'"
        );
        assert!(
            schema_json.contains("webhook"),
            "Schema should contain 'webhook'"
        );
        assert!(
            schema_json.contains("file_watch"),
            "Schema should contain 'file_watch'"
        );
        assert!(
            schema_json.contains("csv"),
            "Schema should contain 'csv'"
        );
    }

    #[test]
    fn test_schema_includes_all_dq_rule_types() {
        let schema_json = generate_schema().expect("Should generate schema");

        // Verify all DQ rule types are in the schema
        assert!(schema_json.contains("range_check"));
        assert!(schema_json.contains("null_check"));
        assert!(schema_json.contains("enum_check"));
        assert!(schema_json.contains("pattern_check"));
        assert!(schema_json.contains("freshness_check"));
        assert!(schema_json.contains("monotonic_check"));
        assert!(schema_json.contains("rate_of_change"));
        assert!(schema_json.contains("cross_field_check"));
        assert!(schema_json.contains("conditional_check"));
        assert!(schema_json.contains("completeness_check"));
        assert!(schema_json.contains("cardinality_check"));
    }

    #[test]
    fn test_schema_includes_all_dq_actions() {
        let schema_json = generate_schema().expect("Should generate schema");

        // Verify all DQ actions are in the schema
        assert!(schema_json.contains("flag"));
        assert!(schema_json.contains("reject"));
        assert!(schema_json.contains("clamp"));
        assert!(schema_json.contains("drop"));
        assert!(schema_json.contains("warn"));
    }

    // TC-501: Verify Schema Returns True When Matching
    #[test]
    fn test_verify_schema_returns_true_when_matching() {
        // Generate schema
        let schema_json = generate_schema().expect("Should generate schema");

        // Write to temp file
        let mut temp_file = NamedTempFile::new().expect("Should create temp file");
        write!(temp_file, "{}", schema_json).expect("Should write schema");

        // Verify against itself
        let matches = verify_schema(temp_file.path()).expect("Should verify");
        assert!(matches, "Schema should match itself");
    }

    // TC-502: Verify Schema Returns False When Drift
    #[test]
    fn test_verify_schema_returns_false_when_drift() {
        // Create outdated schema (missing 'csv' in source types)
        let outdated_schema = r#"{
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Outdated Schema",
            "definitions": {
                "SourceType": {
                    "type": "string",
                    "enum": ["mqtt", "http_poll", "webhook", "file_watch"]
                }
            }
        }"#;

        // Write to temp file
        let mut temp_file = NamedTempFile::new().expect("Should create temp file");
        write!(temp_file, "{}", outdated_schema).expect("Should write schema");

        // Verify - should detect drift
        let matches = verify_schema(temp_file.path()).expect("Should verify");
        assert!(!matches, "Should detect schema drift");
    }

    #[test]
    fn test_compare_schemas_finds_differences() {
        // Create schema with wrong enum values
        let wrong_schema = r#"{
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Wrong Schema",
            "properties": {
                "source_type": {
                    "type": "string",
                    "enum": ["mqtt", "ftp"]
                }
            }
        }"#;

        // Write to temp file
        let mut temp_file = NamedTempFile::new().expect("Should create temp file");
        write!(temp_file, "{}", wrong_schema).expect("Should write schema");

        // Compare
        let differences = compare_schemas(temp_file.path()).expect("Should compare");
        assert!(!differences.is_empty(), "Should find differences");
    }

    #[test]
    fn test_generate_type_schema() {
        // Generate schema for SourceType only
        let schema_json = generate_type_schema::<SourceType>().expect("Should generate");
        let schema: serde_json::Value = serde_json::from_str(&schema_json).unwrap();

        // schemars generates oneOf with string enums for strum-derived enums
        // Check that the schema has oneOf array (strum enum pattern)
        assert!(
            schema["oneOf"].is_array(),
            "Expected oneOf array in schema, got: {}",
            serde_json::to_string_pretty(&schema).unwrap()
        );

        // Each variant should be a string enum
        let variants = schema["oneOf"].as_array().unwrap();
        assert!(!variants.is_empty(), "Should have variants");

        // Check first variant has expected structure
        let first_variant = &variants[0];
        assert_eq!(first_variant["type"], "string");
        assert!(first_variant["enum"].is_array());
    }

    #[test]
    fn test_normalize_schema_removes_volatile_fields() {
        let schema_with_timestamp = serde_json::json!({
            "title": "Test",
            "x-generated-at": "2026-02-02T12:00:00Z",
            "x-ndp-version": "1.0.0"
        });

        let normalized = normalize_schema(&schema_with_timestamp);

        assert!(normalized.get("x-generated-at").is_none());
        assert!(normalized.get("x-ndp-version").is_some());
    }
}
