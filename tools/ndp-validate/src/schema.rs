//! Layer 1: JSON Schema Validation
//!
//! Provides declarative validation against JSON Schema Draft 2020-12.
//! This layer handles:
//! - Structural validation (required fields, object structure)
//! - Type checking (string, number, boolean, array, object)
//! - Enum validation (allowed values)
//! - Pattern matching (regex for string formats)
//! - Additional property detection (unknown fields)
//!
//! # London School TDD Design
//!
//! This module follows behavior-driven design:
//! - Focus on WHAT the validator does (interactions/contracts)
//! - Tests verify behavior through mock expectations and output verification
//! - Error codes: SYNTAX_ERROR, MISSING_REQUIRED, INVALID_TYPE, UNKNOWN_FIELD,
//!   PATTERN_MISMATCH, ENUM_VIOLATION

use crate::error::{ErrorCode, SchemaValidatorError, Severity, ValidationError, ValidationLayer};
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;
use std::path::Path;

// =============================================================================
// Schema Validator
// =============================================================================

/// JSON Schema validator for NDP configurations
///
/// Supports both stream and domain configuration validation.
pub struct SchemaValidator {
    /// Compiled JSON Schema
    schema: JSONSchema,
}

/// Domain schema validator
pub struct DomainSchemaValidator {
    /// Compiled JSON Schema for domain configs
    schema: JSONSchema,
}

impl SchemaValidator {
    /// Create a new validator from a JSON Schema value
    pub fn new(schema_value: Value) -> Result<Self, SchemaValidatorError> {
        let schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .map_err(|e| SchemaValidatorError::SchemaCompileError(e.to_string()))?;

        Ok(Self { schema })
    }

    /// Create a new validator from a schema file path
    pub fn from_file(path: &Path) -> Result<Self, SchemaValidatorError> {
        let content = std::fs::read_to_string(path)?;
        let schema_value: Value = serde_json::from_str(&content)
            .map_err(|e| SchemaValidatorError::SchemaLoadError(e.to_string()))?;
        Self::new(schema_value)
    }

    /// Create a validator using the embedded default schema
    pub fn default_schema() -> Result<Self, SchemaValidatorError> {
        let schema_value = default_stream_schema();
        Self::new(schema_value)
    }

    /// Validate JSON syntax and return parsed Value or syntax error
    ///
    /// This method checks for malformed JSON and returns detailed line/column
    /// information for syntax errors.
    ///
    /// # Arguments
    /// * `content` - Raw JSON string to parse
    ///
    /// # Returns
    /// * `Ok(Value)` - Parsed JSON value if syntax is valid
    /// * `Err(ValidationError)` - Syntax error with line/column information
    ///
    /// # Example
    /// ```ignore
    /// use ndp_validate::SchemaValidator;
    ///
    /// let result = SchemaValidator::validate_json_syntax(r#"{"valid": true}"#);
    /// assert!(result.is_ok());
    ///
    /// let result = SchemaValidator::validate_json_syntax(r#"{"invalid": }"#);
    /// assert!(result.is_err());
    /// ```
    pub fn validate_json_syntax(content: &str) -> Result<Value, ValidationError> {
        serde_json::from_str(content).map_err(|e| {
            let line = e.line();
            let column = e.column();
            let message = e.to_string();

            // Extract a cleaner error message (remove "at line X, column Y" suffix)
            let clean_message = if let Some(pos) = message.find(" at line ") {
                message[..pos].to_string()
            } else {
                message
            };

            ValidationError::syntax_error(line, column, &clean_message)
        })
    }

    /// Validate a JSON value against the schema
    ///
    /// # Arguments
    /// * `instance` - Parsed JSON value to validate
    ///
    /// # Returns
    /// Vector of validation errors (empty if valid)
    pub fn validate_schema(&self, instance: &Value) -> Vec<ValidationError> {
        let result = self.schema.validate(instance);

        match result {
            Ok(_) => Vec::new(),
            Err(errors) => errors.map(|e| self.convert_error(&e)).collect(),
        }
    }

    /// Validate both syntax and schema in one call
    ///
    /// Convenience method that first validates JSON syntax, then validates
    /// against the schema if syntax is valid.
    ///
    /// # Arguments
    /// * `content` - Raw JSON string to validate
    ///
    /// # Returns
    /// Vector of validation errors (empty if valid)
    pub fn validate(&self, content: &str) -> Vec<ValidationError> {
        // First check syntax
        let value = match Self::validate_json_syntax(content) {
            Ok(v) => v,
            Err(e) => return vec![e],
        };

        // Then validate schema
        self.validate_schema(&value)
    }

    /// Validate a JSON value against the schema (alias for validate_schema)
    #[deprecated(since = "0.2.0", note = "Use validate_schema instead")]
    pub fn validate_value(&self, instance: &Value) -> Vec<ValidationError> {
        self.validate_schema(instance)
    }

    /// Convert a jsonschema error to our ValidationError format
    fn convert_error(&self, error: &jsonschema::ValidationError) -> ValidationError {
        let path = format_json_path(&error.instance_path);
        let message = error.to_string();

        // Map error kind to our error codes
        let code = match error.kind {
            jsonschema::error::ValidationErrorKind::Required { .. } => ErrorCode::MissingRequired,
            jsonschema::error::ValidationErrorKind::Type { .. } => ErrorCode::InvalidType,
            jsonschema::error::ValidationErrorKind::Enum { .. } => ErrorCode::EnumViolation,
            jsonschema::error::ValidationErrorKind::Pattern { .. } => ErrorCode::PatternMismatch,
            jsonschema::error::ValidationErrorKind::AdditionalProperties { .. } => {
                ErrorCode::UnknownField
            }
            jsonschema::error::ValidationErrorKind::MinItems { .. }
            | jsonschema::error::ValidationErrorKind::MaxItems { .. } => ErrorCode::ArrayBounds,
            _ => ErrorCode::InvalidType,
        };

        // Try to generate a helpful suggestion for unknown fields
        let suggestion = if code == ErrorCode::UnknownField {
            self.suggest_field_correction(error)
        } else {
            None
        };

        ValidationError {
            layer: ValidationLayer::Schema,
            code,
            path,
            message,
            severity: Severity::Error,
            suggestion,
            context: None,
        }
    }

    /// Suggest a correction for an unknown field based on common typos
    fn suggest_field_correction(&self, error: &jsonschema::ValidationError) -> Option<String> {
        let error_str = error.to_string();

        // Common typos and their corrections (dp-019 requirements)
        let corrections = [
            ("silver_elt", "silver_etl"),
            ("field_mapings", "field_mappings"),
            ("field_mapping", "field_mappings"),
            ("source_paths", "source_path"),
            ("target_columns", "target_column"),
            ("temperture", "temperature"),
            ("humidty", "humidity"),
            ("pressue", "pressure"),
            ("timestap", "timestamp"),
            ("discription", "description"),
            ("enbled", "enabled"),
        ];

        for (typo, correct) in corrections.iter() {
            if error_str.contains(typo) {
                return Some(format!("Did you mean '{}'?", correct));
            }
        }

        None
    }
}

/// Format a JSONPath from jsonschema's InstancePath
///
/// Converts the internal path representation to standard JSONPath notation
/// (e.g., "$.fields[0].name")
fn format_json_path(path: &jsonschema::paths::JSONPointer) -> String {
    let mut result = String::from("$");

    for segment in path.iter() {
        match segment {
            jsonschema::paths::PathChunk::Property(prop) => {
                result.push('.');
                result.push_str(prop.as_ref());
            }
            jsonschema::paths::PathChunk::Index(idx) => {
                result.push_str(&format!("[{}]", idx));
            }
            _ => {}
        }
    }

    result
}

// =============================================================================
// Default Schema
// =============================================================================

/// Returns the default JSON Schema for NDP stream configurations
///
/// This schema validates the structure of stream config YAML files.
pub fn default_stream_schema() -> Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://ndp.local/schemas/stream-config.json",
        "title": "NDP Stream Configuration",
        "description": "Configuration schema for NDP data streams",
        "type": "object",
        "required": ["info"],
        "additionalProperties": false,
        "properties": {
            "info": {
                "type": "object",
                "required": ["stream_id", "version"],
                "additionalProperties": false,
                "properties": {
                    "stream_id": {
                        "type": "string",
                        "pattern": "^[a-z][a-z0-9-]*$",
                        "description": "Unique stream identifier (kebab-case)"
                    },
                    "version": {
                        "type": "string",
                        "pattern": "^\\d+\\.\\d+\\.\\d+$",
                        "description": "Semantic version (e.g., 1.0.0)"
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description"
                    },
                    "enabled": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether the stream is active"
                    }
                }
            },
            "source": {
                "type": "object",
                "required": ["type"],
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["http_poll", "mqtt", "file_watch", "manual"],
                        "description": "Source type"
                    },
                    "config": {
                        "type": "object",
                        "description": "Source-specific configuration"
                    }
                }
            },
            "parser": {
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["json_path", "csv", "regex", "passthrough"],
                        "description": "Parser type"
                    },
                    "field_mappings": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["source_path", "target_field"],
                            "properties": {
                                "source_path": {
                                    "type": "string",
                                    "description": "JSONPath or source field name"
                                },
                                "target_field": {
                                    "type": "string",
                                    "pattern": "^[a-z][a-z0-9_]*$",
                                    "description": "Target field name (snake_case)"
                                },
                                "transform": {
                                    "type": "string",
                                    "description": "Optional transform expression"
                                }
                            }
                        }
                    }
                }
            },
            "entity_schemas": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["schema_name", "attributes"],
                    "properties": {
                        "schema_name": {
                            "type": "string",
                            "description": "Schema identifier"
                        },
                        "attributes": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["name", "data_type"],
                                "properties": {
                                    "name": {
                                        "type": "string",
                                        "pattern": "^[a-z][a-z0-9_]*$",
                                        "description": "Attribute name (snake_case)"
                                    },
                                    "data_type": {
                                        "type": "string",
                                        "enum": [
                                            "string", "text",
                                            "integer", "int", "bigint",
                                            "float", "double", "real",
                                            "boolean", "bool",
                                            "timestamp", "timestamptz",
                                            "json", "jsonb"
                                        ],
                                        "description": "SQL data type"
                                    },
                                    "nullable": {
                                        "type": "boolean",
                                        "default": true,
                                        "description": "Whether NULL values allowed"
                                    },
                                    "unit": {
                                        "type": "string",
                                        "description": "Unit of measurement"
                                    },
                                    "description": {
                                        "type": "string",
                                        "description": "Field description"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "silver_etl": {
                "type": "object",
                "properties": {
                    "target_table": {
                        "type": "string",
                        "pattern": "^[a-z][a-z0-9_]*$",
                        "description": "Target Silver table name"
                    },
                    "transforms": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string"
                                },
                                "sql": {
                                    "type": "string"
                                }
                            }
                        }
                    },
                    "dq_checks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["name", "expression"],
                            "properties": {
                                "name": {
                                    "type": "string"
                                },
                                "expression": {
                                    "type": "string",
                                    "description": "SQL WHERE clause for DQ check"
                                },
                                "severity": {
                                    "type": "string",
                                    "enum": ["error", "warning", "info"],
                                    "default": "error"
                                }
                            }
                        }
                    }
                }
            },
            "storage": {
                "type": "object",
                "properties": {
                    "bronze_path": {
                        "type": "string"
                    },
                    "retention_days": {
                        "type": "integer",
                        "minimum": 1
                    }
                }
            }
        }
    })
}

// =============================================================================
// Domain Schema Validator
// =============================================================================

impl DomainSchemaValidator {
    /// Create a new domain validator from a JSON Schema value
    pub fn new(schema_value: Value) -> Result<Self, SchemaValidatorError> {
        let schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .map_err(|e| SchemaValidatorError::SchemaCompileError(e.to_string()))?;

        Ok(Self { schema })
    }

    /// Create a new domain validator from a schema file path
    pub fn from_file(path: &Path) -> Result<Self, SchemaValidatorError> {
        let content = std::fs::read_to_string(path)?;
        let schema_value: Value = serde_json::from_str(&content)
            .map_err(|e| SchemaValidatorError::SchemaLoadError(e.to_string()))?;
        Self::new(schema_value)
    }

    /// Create a validator using the embedded default domain schema
    pub fn default_schema() -> Result<Self, SchemaValidatorError> {
        let schema_value = default_domain_schema();
        Self::new(schema_value)
    }

    /// Validate a domain JSON value against the schema
    ///
    /// # Arguments
    /// * `instance` - Parsed JSON value to validate
    ///
    /// # Returns
    /// Vector of validation errors (empty if valid)
    pub fn validate_schema(&self, instance: &Value) -> Vec<ValidationError> {
        let result = self.schema.validate(instance);

        match result {
            Ok(_) => Vec::new(),
            Err(errors) => errors.map(|e| self.convert_error(&e)).collect(),
        }
    }

    /// Validate both syntax and schema in one call
    pub fn validate(&self, content: &str) -> Vec<ValidationError> {
        // First check syntax
        let value = match SchemaValidator::validate_json_syntax(content) {
            Ok(v) => v,
            Err(e) => return vec![e],
        };

        // Then validate schema
        self.validate_schema(&value)
    }

    /// Convert a jsonschema error to our ValidationError format
    fn convert_error(&self, error: &jsonschema::ValidationError) -> ValidationError {
        let path = format_json_path(&error.instance_path);
        let message = error.to_string();

        // Map error kind to our error codes
        let code = match error.kind {
            jsonschema::error::ValidationErrorKind::Required { .. } => ErrorCode::MissingRequired,
            jsonschema::error::ValidationErrorKind::Type { .. } => ErrorCode::InvalidType,
            jsonschema::error::ValidationErrorKind::Enum { .. } => ErrorCode::EnumViolation,
            jsonschema::error::ValidationErrorKind::Pattern { .. } => ErrorCode::PatternMismatch,
            jsonschema::error::ValidationErrorKind::AdditionalProperties { .. } => {
                ErrorCode::UnknownField
            }
            jsonschema::error::ValidationErrorKind::MinItems { .. }
            | jsonschema::error::ValidationErrorKind::MaxItems { .. } => ErrorCode::ArrayBounds,
            _ => ErrorCode::InvalidType,
        };

        // Try to generate a helpful suggestion for unknown fields
        let suggestion = if code == ErrorCode::UnknownField {
            self.suggest_field_correction(error)
        } else {
            None
        };

        ValidationError {
            layer: ValidationLayer::Schema,
            code,
            path,
            message,
            severity: Severity::Error,
            suggestion,
            context: None,
        }
    }

    /// Suggest a correction for an unknown field based on common typos
    fn suggest_field_correction(&self, error: &jsonschema::ValidationError) -> Option<String> {
        let error_str = error.to_string();

        // Common domain config typos
        let corrections = [
            ("objective", "objectives"),
            ("stream", "streams"),
            ("allignment", "alignment"),
            ("alignement", "alignment"),
            ("contraint", "constraint"),
            ("constraints", "constraints"),
            ("granulatiry", "granularity"),
            ("join_stratgy", "join_strategy"),
            ("null_handeling", "null_handling"),
            ("view_nam", "view_name"),
        ];

        for (typo, correct) in corrections.iter() {
            if error_str.contains(typo) && !error_str.contains(correct) {
                return Some(format!("Did you mean '{}'?", correct));
            }
        }

        None
    }
}

/// Returns the default JSON Schema for NDP domain configurations
///
/// This is a simplified embedded schema for basic validation.
/// For full validation, use the schema file at config/schemas/domain.schema.json
pub fn default_domain_schema() -> Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://neural-data-platform.local/schemas/domain.schema.json",
        "title": "NDP Domain Configuration",
        "description": "Schema for domain configurations combining streams, alignment, and objectives.",
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "streams", "alignment"],
        "properties": {
            "id": {
                "type": "string",
                "pattern": "^[a-z][a-z0-9-]*$",
                "description": "Unique domain identifier (kebab-case)"
            },
            "description": {
                "type": "string",
                "description": "Human-readable domain description"
            },
            "streams": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["stream_id", "role"],
                    "properties": {
                        "stream_id": {
                            "type": "string",
                            "pattern": "^[a-z][a-z0-9-]*$"
                        },
                        "alias": {
                            "type": "string",
                            "pattern": "^[a-z][a-z0-9_]*$",
                            "maxLength": 20
                        },
                        "role": {
                            "type": "string",
                            "enum": ["primary", "context", "actuator", "constraint"]
                        },
                        "null_handling": {
                            "type": "string",
                            "enum": ["preserve", "carry_forward", "interpolate"]
                        }
                    }
                },
                "minItems": 1
            },
            "alignment": {
                "type": "object",
                "additionalProperties": false,
                "required": ["view_name", "granularity"],
                "properties": {
                    "view_name": {
                        "type": "string",
                        "pattern": "^[a-z][a-z0-9_]*$",
                        "maxLength": 63
                    },
                    "granularity": {
                        "type": "string",
                        "pattern": "^\\d+\\s+(minute|hour|day)s?$"
                    },
                    "join_strategy": {
                        "type": "string",
                        "enum": ["full_outer", "left", "inner"],
                        "default": "full_outer"
                    },
                    "null_handling": {
                        "type": "string",
                        "enum": ["preserve", "carry_forward", "interpolate"]
                    },
                    "timestamp_alignment": {
                        "type": "string",
                        "enum": ["bucket_start", "bucket_end"]
                    }
                }
            },
            "objectives": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "target"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "pattern": "^[a-z][a-z0-9_]*$",
                            "maxLength": 50
                        },
                        "description": { "type": "string" },
                        "target": {
                            "type": "object",
                            "required": ["stream", "metric", "condition", "threshold"],
                            "properties": {
                                "stream": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" },
                                "metric": { "type": "string", "pattern": "^[a-z][a-z0-9_]*$" },
                                "condition": {
                                    "type": "string",
                                    "enum": ["<", "<=", ">", ">=", "==", "!=", "between"]
                                },
                                "threshold": {},
                                "unit": { "type": "string" }
                            }
                        },
                        "priority": {
                            "type": "string",
                            "enum": ["critical", "high", "medium", "low"]
                        },
                        "time_window": { "type": "object" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "depends_on": { "type": "array", "items": { "type": "string" } },
                        "aggregation": {
                            "type": "string",
                            "enum": ["all", "any"]
                        }
                    }
                }
            },
            "events": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "chunk_interval": { "type": "string" },
                    "retention": { "type": "string" },
                    "detection_schedule": { "type": "string" },
                    "refresh_start_offset_days": { "type": "integer", "minimum": 1 }
                }
            },
            "constraints": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "condition"],
                    "properties": {
                        "id": {
                            "type": "string",
                            "pattern": "^[a-z][a-z0-9_]*$",
                            "maxLength": 50
                        },
                        "description": { "type": "string" },
                        "condition": {
                            "type": "object",
                            "required": ["stream", "metric", "operator", "threshold"],
                            "properties": {
                                "stream": { "type": "string" },
                                "metric": { "type": "string" },
                                "operator": {
                                    "type": "string",
                                    "enum": ["<", "<=", ">", ">=", "==", "!=", "between"]
                                },
                                "threshold": {}
                            }
                        },
                        "applies_to": { "type": "array", "items": { "type": "string" } }
                    }
                }
            }
        }
    })
}

// =============================================================================
// Tests - London School TDD (Behavior Verification)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Test Fixtures
    // -------------------------------------------------------------------------

    fn create_validator() -> SchemaValidator {
        SchemaValidator::default_schema().expect("Failed to create default schema validator")
    }

    /// Create a validator with the v1.1 schema structure for testing
    fn create_v1_1_validator() -> SchemaValidator {
        let schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft-07/schema#",
            "type": "object",
            "required": ["stream_id", "description", "fields", "sources"],
            "additionalProperties": false,
            "properties": {
                "stream_id": {
                    "type": "string",
                    "pattern": "^[a-z][a-z0-9-]{2,63}$"
                },
                "description": {
                    "type": "string",
                    "minLength": 1
                },
                "version": {
                    "type": "string",
                    "pattern": "^\\d+\\.\\d+\\.\\d+$"
                },
                "enabled": { "type": "boolean" },
                "retention_days": { "type": "integer", "minimum": 0 },
                "partitioning_strategy": {
                    "type": "string",
                    "enum": ["daily", "hourly", "monthly"]
                },
                "fields": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "required": ["name", "type"],
                        "additionalProperties": false,
                        "properties": {
                            "name": { "type": "string", "pattern": "^[a-z][a-z0-9_]{0,63}$" },
                            "type": { "type": "string", "enum": ["float", "int", "string", "bool", "json"] },
                            "unit": { "type": "string" },
                            "description": { "type": "string" }
                        }
                    }
                },
                "sources": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "required": ["type", "enabled"],
                        "properties": {
                            "type": { "type": "string", "enum": ["mqtt", "http_poll", "http_push", "file_watch"] },
                            "enabled": { "type": "boolean" }
                        }
                    }
                },
                "silver_etl": {
                    "type": "object",
                    "required": ["enabled", "target_table"],
                    "additionalProperties": false,
                    "properties": {
                        "enabled": { "type": "boolean" },
                        "target_table": { "type": "string", "pattern": "^silver\\.[a-z_][a-z0-9_]*$" },
                        "field_mappings": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["source_path", "target_column", "type"],
                                "additionalProperties": false,
                                "properties": {
                                    "source_path": { "type": "string" },
                                    "target_column": { "type": "string" },
                                    "type": { "type": "string" }
                                }
                            }
                        }
                    }
                }
            }
        });
        SchemaValidator::new(schema).expect("Failed to create v1.1 validator")
    }

    /// Minimal valid v1.1 config for testing
    fn valid_v1_1_config() -> Value {
        serde_json::json!({
            "stream_id": "air-quality",
            "description": "Air quality measurements",
            "fields": [{ "name": "pm25", "type": "float", "unit": "ug/m3" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        })
    }

    // -------------------------------------------------------------------------
    // TC-SV-001: test_valid_json_parses_successfully
    // -------------------------------------------------------------------------

    #[test]
    fn test_valid_json_parses_successfully() {
        let valid_json = r#"{"stream_id": "test-stream", "value": 42}"#;
        let result = SchemaValidator::validate_json_syntax(valid_json);
        assert!(result.is_ok(), "Valid JSON should parse successfully");
        let value = result.unwrap();
        assert_eq!(value["stream_id"], "test-stream");
        assert_eq!(value["value"], 42);
    }

    // -------------------------------------------------------------------------
    // TC-SV-002: test_malformed_json_returns_syntax_error_with_line_number
    // -------------------------------------------------------------------------

    #[test]
    fn test_malformed_json_returns_syntax_error_with_line_number() {
        let malformed_json = "{\n    \"stream_id\": \"test\",\n    \"invalid\"\n}";
        let result = SchemaValidator::validate_json_syntax(malformed_json);
        assert!(result.is_err(), "Malformed JSON should return error");
        let error = result.unwrap_err();
        assert_eq!(error.layer, ValidationLayer::Syntax);
        assert_eq!(error.code, ErrorCode::SyntaxError);
        assert_eq!(error.severity, Severity::Error);
        assert!(
            error.path.contains("line"),
            "Error path should contain line number"
        );
        let ctx = error.context.expect("Should have context");
        assert!(ctx.get("line").is_some(), "Context should have line");
        assert!(ctx.get("column").is_some(), "Context should have column");
    }

    #[test]
    fn test_malformed_json_trailing_comma() {
        let malformed_json = r#"{"stream_id": "test",}"#;
        let result = SchemaValidator::validate_json_syntax(malformed_json);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::SyntaxError);
    }

    #[test]
    fn test_malformed_json_unclosed_string() {
        let malformed_json = r#"{"stream_id": "test"#;
        let result = SchemaValidator::validate_json_syntax(malformed_json);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::SyntaxError);
        assert!(!error.message.is_empty());
    }

    // -------------------------------------------------------------------------
    // TC-SV-003: test_missing_required_field_returns_schema_error
    // -------------------------------------------------------------------------

    #[test]
    fn test_missing_required_field_returns_schema_error() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "description": "Test stream",
            "fields": [{ "name": "temp", "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty(), "Should have validation errors");
        let missing_error = errors
            .iter()
            .find(|e| e.code == ErrorCode::MissingRequired)
            .expect("Should have MISSING_REQUIRED error");
        assert_eq!(missing_error.layer, ValidationLayer::Schema);
    }

    #[test]
    fn test_missing_nested_required_field() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [{ "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        let error = errors
            .iter()
            .find(|e| e.code == ErrorCode::MissingRequired)
            .expect("Should have MISSING_REQUIRED error");
        assert!(error.path.contains("fields"));
    }

    // -------------------------------------------------------------------------
    // TC-SV-004: test_unknown_field_returns_schema_error
    // -------------------------------------------------------------------------

    #[test]
    fn test_unknown_field_returns_schema_error() {
        let validator = create_v1_1_validator();
        let mut config = valid_v1_1_config();
        config["silver_elt"] = serde_json::json!({ "enabled": true });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty(), "Should detect unknown field");
        let unknown_error = errors
            .iter()
            .find(|e| e.code == ErrorCode::UnknownField)
            .expect("Should have UNKNOWN_FIELD error");
        assert_eq!(unknown_error.layer, ValidationLayer::Schema);
        if let Some(suggestion) = &unknown_error.suggestion {
            assert!(suggestion.contains("silver_etl"));
        }
    }

    #[test]
    fn test_unknown_nested_field_returns_schema_error() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [{ "name": "temp", "type": "float", "unknown_attr": true }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        let error = errors
            .iter()
            .find(|e| e.code == ErrorCode::UnknownField)
            .expect("Should have UNKNOWN_FIELD error");
        assert!(error.path.contains("fields") && error.path.contains("[0]"));
    }

    // -------------------------------------------------------------------------
    // TC-SV-005: test_invalid_type_returns_schema_error
    // -------------------------------------------------------------------------

    #[test]
    fn test_invalid_type_returns_schema_error() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "retention_days": "30",
            "fields": [{ "name": "temp", "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty(), "Should detect type error");
        let type_error = errors
            .iter()
            .find(|e| e.code == ErrorCode::InvalidType)
            .expect("Should have INVALID_TYPE error");
        assert_eq!(type_error.layer, ValidationLayer::Schema);
        assert!(type_error.path.contains("retention_days"));
    }

    // -------------------------------------------------------------------------
    // TC-SV-006: test_enum_violation_returns_schema_error
    // -------------------------------------------------------------------------

    #[test]
    fn test_enum_violation_returns_schema_error() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "partitioning_strategy": "weekly",
            "fields": [{ "name": "temp", "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty(), "Should detect enum violation");
        let enum_error = errors
            .iter()
            .find(|e| e.code == ErrorCode::EnumViolation)
            .expect("Should have ENUM_VIOLATION error");
        assert_eq!(enum_error.layer, ValidationLayer::Schema);
        assert!(enum_error.path.contains("partitioning_strategy"));
    }

    #[test]
    fn test_invalid_field_type_enum() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [{ "name": "temp", "type": "decimal" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
    }

    // -------------------------------------------------------------------------
    // TC-SV-007: Pattern Mismatch Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_pattern_mismatch_stream_id() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "123-invalid",
            "description": "Test",
            "fields": [{ "name": "temp", "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::PatternMismatch));
    }

    // -------------------------------------------------------------------------
    // TC-SV-008: Valid Config Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_valid_config_produces_no_errors() {
        let validator = create_v1_1_validator();
        let errors = validator.validate_schema(&valid_v1_1_config());
        assert!(
            errors.is_empty(),
            "Valid config should have no errors: {:?}",
            errors
        );
    }

    #[test]
    fn test_valid_config_with_all_fields() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "air-quality",
            "description": "Air quality measurements from sensors",
            "version": "1.0.0",
            "enabled": true,
            "retention_days": 365,
            "partitioning_strategy": "daily",
            "fields": [
                { "name": "pm25", "type": "float", "unit": "ug/m3", "description": "PM2.5" },
                { "name": "temperature", "type": "float", "unit": "celsius" }
            ],
            "sources": [
                { "type": "mqtt", "enabled": true },
                { "type": "http_poll", "enabled": false }
            ],
            "silver_etl": {
                "enabled": true,
                "target_table": "silver.air_quality_readings",
                "field_mappings": [
                    { "source_path": "raw_payload.pm25", "target_column": "pm25", "type": "double_precision" }
                ]
            }
        });
        let errors = validator.validate_schema(&config);
        assert!(
            errors.is_empty(),
            "Valid config should have no errors: {:?}",
            errors
        );
    }

    // -------------------------------------------------------------------------
    // TC-SV-009: Combined validate() method
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_catches_syntax_error() {
        let validator = create_v1_1_validator();
        let errors = validator.validate(r#"{"stream_id": "test", invalid}"#);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::SyntaxError);
    }

    #[test]
    fn test_validate_catches_schema_error() {
        let validator = create_v1_1_validator();
        let json_str = serde_json::to_string(&serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [{ "name": "temp", "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }],
            "unknown_field": true
        }))
        .unwrap();
        let errors = validator.validate(&json_str);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::UnknownField));
    }

    // -------------------------------------------------------------------------
    // TC-SV-010: Array bounds validation
    // -------------------------------------------------------------------------

    #[test]
    fn test_empty_fields_array_returns_error() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
    }

    // -------------------------------------------------------------------------
    // TC-SV-011: JSONPath formatting
    // -------------------------------------------------------------------------

    #[test]
    fn test_json_path_formatting() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [
                { "name": "temp", "type": "float" },
                { "name": "Humidity", "type": "float" }
            ],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        let path_error = errors.iter().find(|e| e.path.contains("fields")).unwrap();
        assert!(path_error.path.starts_with("$"));
        assert!(path_error.path.contains("[1]"));
    }

    // -------------------------------------------------------------------------
    // TC-SV-012: Multiple errors in one validation
    // -------------------------------------------------------------------------

    #[test]
    fn test_multiple_errors_in_one_validation() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "123-bad",
            "description": "Test",
            "unknown_top_field": true,
            "fields": [{ "name": "temp", "type": "invalid_type" }],
            "sources": [{ "type": "mqtt", "enabled": true }]
        });
        let errors = validator.validate_schema(&config);
        assert!(
            errors.len() >= 2,
            "Should have multiple errors: {:?}",
            errors
        );
    }

    // -------------------------------------------------------------------------
    // Error serialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_validation_error_json_serialization() {
        let validator = create_v1_1_validator();
        let config = serde_json::json!({
            "stream_id": "test-stream",
            "description": "Test",
            "fields": [{ "name": "temp", "type": "float" }],
            "sources": [{ "type": "mqtt", "enabled": true }],
            "silver_elt": { "enabled": true }
        });
        let errors = validator.validate_schema(&config);
        let json = serde_json::to_string_pretty(&errors).unwrap();
        assert!(json.contains("\"layer\": \"schema\""));
        assert!(json.contains("\"code\": \"UNKNOWN_FIELD\""));
        assert!(json.contains("\"severity\": \"error\""));
    }

    // -------------------------------------------------------------------------
    // Original tests (default schema)
    // -------------------------------------------------------------------------

    #[test]
    fn test_valid_minimal_config() {
        let validator = create_validator();
        let config =
            serde_json::json!({ "info": { "stream_id": "test-stream", "version": "1.0.0" } });
        let errors = validator.validate_schema(&config);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_missing_required_field() {
        let validator = create_validator();
        let config = serde_json::json!({ "info": { "stream_id": "test-stream" } });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::MissingRequired));
    }

    #[test]
    fn test_invalid_stream_id_pattern() {
        let validator = create_validator();
        let config =
            serde_json::json!({ "info": { "stream_id": "TestStream", "version": "1.0.0" } });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::PatternMismatch));
    }

    #[test]
    fn test_invalid_source_type() {
        let validator = create_validator();
        let config = serde_json::json!({
            "info": { "stream_id": "test-stream", "version": "1.0.0" },
            "source": { "type": "invalid_source" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
    }

    #[test]
    fn test_unknown_field() {
        let validator = create_validator();
        let config = serde_json::json!({
            "info": { "stream_id": "test-stream", "version": "1.0.0", "unknown_field": "value" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::UnknownField));
    }

    #[test]
    fn test_valid_full_config() {
        let validator = create_validator();
        let config = serde_json::json!({
            "info": { "stream_id": "air-quality", "version": "1.0.0", "description": "Air quality sensor data", "enabled": true },
            "source": { "type": "mqtt", "config": { "topic": "sensors/air-quality" } },
            "parser": { "type": "json_path", "field_mappings": [{ "source_path": "$.pm25", "target_field": "pm25_value" }] },
            "entity_schemas": [{ "schema_name": "air-quality-reading", "attributes": [{ "name": "pm25_value", "data_type": "float", "nullable": false, "unit": "ug/m3" }] }],
            "silver_etl": { "target_table": "air_quality_readings", "dq_checks": [{ "name": "pm25_range", "expression": "pm25_value >= 0 AND pm25_value <= 500" }] }
        });
        let errors = validator.validate_schema(&config);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_invalid_data_type() {
        let validator = create_validator();
        let config = serde_json::json!({
            "info": { "stream_id": "test-stream", "version": "1.0.0" },
            "entity_schemas": [{ "schema_name": "test", "attributes": [{ "name": "test_field", "data_type": "invalid_type" }] }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
    }

    // =========================================================================
    // Domain Schema Validator Tests
    // =========================================================================

    fn create_domain_validator() -> DomainSchemaValidator {
        DomainSchemaValidator::default_schema().expect("Failed to create domain schema validator")
    }

    fn valid_domain_config() -> Value {
        serde_json::json!({
            "id": "indoor-air-quality",
            "description": "Indoor air quality optimization",
            "streams": [
                { "stream_id": "air-quality", "alias": "indoor", "role": "primary" },
                { "stream_id": "outdoor-weather", "alias": "outdoor", "role": "context" }
            ],
            "alignment": {
                "view_name": "indoor_air_quality_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            }
        })
    }

    // TC-DSV-001: Valid domain config passes schema validation
    #[test]
    fn test_valid_domain_passes_schema() {
        let validator = create_domain_validator();
        let errors = validator.validate_schema(&valid_domain_config());
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // TC-DSV-002: Missing id fails schema validation
    #[test]
    fn test_missing_id_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::MissingRequired));
        assert!(errors.iter().any(|e| e.path == "$"));
    }

    // TC-DSV-003: Missing streams fails schema validation
    #[test]
    fn test_missing_streams_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::MissingRequired));
    }

    // TC-DSV-004: Missing alignment fails schema validation
    #[test]
    fn test_missing_alignment_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::MissingRequired));
    }

    // TC-DSV-005: Invalid role enum fails
    #[test]
    fn test_invalid_role_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "unknown_role" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
        assert!(errors.iter().any(|e| e.path.contains("role")));
    }

    // TC-DSV-006: Invalid join strategy enum fails
    #[test]
    fn test_invalid_join_strategy_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": {
                "view_name": "test_view",
                "granularity": "1 hour",
                "join_strategy": "cross_join"
            }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
    }

    // TC-DSV-007: Invalid granularity pattern fails
    #[test]
    fn test_invalid_granularity_pattern_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": {
                "view_name": "test_view",
                "granularity": "hourly"
            }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::PatternMismatch));
    }

    // TC-DSV-008: Invalid domain id pattern fails
    #[test]
    fn test_invalid_domain_id_pattern_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "Invalid_Domain_ID",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::PatternMismatch));
    }

    // TC-DSV-009: Unknown field fails schema validation
    #[test]
    fn test_domain_unknown_field_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" },
            "unknown_top_level_field": true
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::UnknownField));
    }

    // TC-DSV-010: Schema error includes JSONPath
    #[test]
    fn test_domain_schema_error_includes_json_path() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [
                { "stream_id": "air-quality", "role": "primary" },
                { "stream_id": "invalid", "role": "bad_role" }
            ],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        let role_error = errors.iter().find(|e| e.path.contains("streams[1]"));
        assert!(
            role_error.is_some(),
            "Error should include path with array index"
        );
    }

    // TC-DSV-011: Empty streams array fails
    #[test]
    fn test_empty_streams_array_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::ArrayBounds));
    }

    // TC-DSV-012: Valid domain with objectives passes
    #[test]
    fn test_valid_domain_with_objectives_passes_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" },
            "objectives": [
                {
                    "id": "healthy_co2",
                    "description": "Keep CO2 below 800 ppm",
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
        let errors = validator.validate_schema(&config);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // TC-DSV-013: Invalid objective condition fails
    #[test]
    fn test_invalid_objective_condition_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" },
            "objectives": [{
                "id": "test_obj",
                "target": {
                    "stream": "air-quality",
                    "metric": "co2",
                    "condition": "approximately",
                    "threshold": 800
                }
            }]
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
    }

    // TC-DSV-014: Valid domain with constraints passes
    #[test]
    fn test_valid_domain_with_constraints_passes_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" },
            "constraints": [{
                "id": "outdoor_safe",
                "condition": {
                    "stream": "outdoor-air-quality",
                    "metric": "aqi",
                    "operator": "<",
                    "threshold": 100
                }
            }]
        });
        let errors = validator.validate_schema(&config);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    // TC-DSV-015: View name with invalid characters fails
    #[test]
    fn test_invalid_view_name_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "role": "primary" }],
            "alignment": {
                "view_name": "Invalid-View-Name",
                "granularity": "1 hour"
            }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::PatternMismatch));
    }

    // TC-DSV-016: Stream with missing stream_id fails
    #[test]
    fn test_stream_missing_stream_id_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "alias": "indoor", "role": "primary" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::MissingRequired));
    }

    // TC-DSV-017: Stream with missing role fails
    #[test]
    fn test_stream_missing_role_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{ "stream_id": "air-quality", "alias": "indoor" }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::MissingRequired));
    }

    // TC-DSV-018: All valid null_handling values pass
    #[test]
    fn test_valid_null_handling_values_pass_schema() {
        let validator = create_domain_validator();
        for null_handling in &["preserve", "carry_forward", "interpolate"] {
            let config = serde_json::json!({
                "id": "test-domain",
                "streams": [{
                    "stream_id": "air-quality",
                    "role": "primary",
                    "null_handling": null_handling
                }],
                "alignment": {
                    "view_name": "test_view",
                    "granularity": "1 hour",
                    "null_handling": null_handling
                }
            });
            let errors = validator.validate_schema(&config);
            assert!(
                errors.is_empty(),
                "null_handling '{}' should be valid, got: {:?}",
                null_handling,
                errors
            );
        }
    }

    // TC-DSV-019: Invalid null_handling fails
    #[test]
    fn test_invalid_null_handling_fails_schema() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "test-domain",
            "streams": [{
                "stream_id": "air-quality",
                "role": "primary",
                "null_handling": "drop"
            }],
            "alignment": { "view_name": "test_view", "granularity": "1 hour" }
        });
        let errors = validator.validate_schema(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
    }

    // TC-DSV-020: Multiple schema errors reported
    #[test]
    fn test_multiple_domain_schema_errors_reported() {
        let validator = create_domain_validator();
        let config = serde_json::json!({
            "id": "Invalid_ID",
            "streams": [{ "stream_id": "air-quality", "role": "bad_role" }],
            "alignment": {
                "view_name": "Invalid-Name",
                "granularity": "hourly"
            }
        });
        let errors = validator.validate_schema(&config);
        assert!(
            errors.len() >= 3,
            "Expected at least 3 errors, got: {:?}",
            errors
        );
    }
}
