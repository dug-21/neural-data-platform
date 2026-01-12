//! validate_config MCP Tool (dp-005)
//!
//! Compares stream configuration in etcd against actual Bronze Parquet schema.
//! Detects mismatches, missing fields, and extra fields.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "stream_id": "outdoor-weather",
//!   "entity_schema": "nws-weather",
//!   "validation": {
//!     "status": "mismatch",
//!     "config_fields": ["temperature", "humidity", "rain_1h"],
//!     "raw_payload_fields": ["main", "wind", "clouds"],
//!     "analysis": {
//!       "in_config_not_in_payload": ["rain_1h"],
//!       "in_payload_not_in_config": ["clouds"],
//!       "matching": ["temperature", "humidity"]
//!     },
//!     "notes": "..."
//!   }
//! }
//! ```

use crate::mcp::tools::{
    create_error_response, create_tool_response, error_codes,
    traits::{ConfigError, PayloadStructure},
    AppState,
};
use crate::mcp::{JsonRpcError, McpRpcError, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

// =============================================================================
// Input/Output Types
// =============================================================================

/// Input schema for validate_config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigInput {
    pub stream_id: String,
}

/// Validation analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationAnalysis {
    /// Fields in config but not in raw_payload
    pub in_config_not_in_payload: Vec<String>,
    /// Fields in raw_payload but not in config
    pub in_payload_not_in_config: Vec<String>,
    /// Fields present in both
    pub matching: Vec<String>,
    /// Mapped correctly (for nested payloads)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapped_correctly: Option<Vec<String>>,
    /// Unmapped nested fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmapped_nested_fields: Option<Vec<String>>,
}

/// Nested payload info for complex structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedPayloadInfo {
    #[serde(flatten)]
    pub nested: std::collections::HashMap<String, Vec<String>>,
}

/// Validation result details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Status: match, partial_match, mismatch, no_target_schema, mapped
    pub status: String,
    /// Config field names (target fields from entity_schemas)
    pub config_fields: Vec<String>,
    /// Raw payload field names (top-level keys)
    pub raw_payload_fields: Vec<String>,
    /// Nested structure info (if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_payload_nested: Option<std::collections::HashMap<String, Vec<String>>>,
    /// Analysis results
    pub analysis: ValidationAnalysis,
    /// Explanatory notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// validate_config response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigOutput {
    pub stream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_schema: Option<String>,
    pub validation: ValidationResult,
}

// =============================================================================
// Tool Definition
// =============================================================================

/// Get the MCP tool definition for validate_config
pub fn tool_definition() -> ToolDefinition {
    ToolDefinition::new(
        "validate_config",
        "Compare stream configuration in etcd against actual Bronze Parquet schema. Detects mismatches, missing fields, and extra fields.",
        json!({
            "type": "object",
            "properties": {
                "stream_id": {
                    "type": "string",
                    "description": "The stream identifier to validate"
                }
            },
            "required": ["stream_id"]
        }),
    )
}

// =============================================================================
// Tool Execution
// =============================================================================

/// Execute the validate_config tool
///
/// # Arguments
/// * `state` - Application state with injected dependencies
/// * `args` - Input arguments containing stream_id
///
/// # Returns
/// MCP ToolResponse as JSON Value
pub async fn execute(state: &AppState, args: Value) -> Result<Value, McpRpcError> {
    // Parse input
    let input: ValidateConfigInput = serde_json::from_value(args).map_err(|e| {
        McpRpcError::new(
            JsonRpcError::INVALID_PARAMS,
            format!("Invalid input: {}", e),
        )
    })?;

    // Get full config from etcd
    let full_config = match state.config.get_full_config(&input.stream_id).await {
        Ok(config) => config,
        Err(e) => match e {
            ConfigError::StreamNotFound(id) => {
                return create_error_response(
                    error_codes::STREAM_NOT_FOUND,
                    &format!("Stream not found: {}", id),
                    Some(json!({"stream_id": id})),
                );
            }
            ConfigError::ConnectionFailed(msg) | ConfigError::Unavailable(msg) => {
                return create_error_response(
                    error_codes::ETCD_UNAVAILABLE,
                    &format!("Configuration unavailable: {}", msg),
                    None,
                );
            }
            _ => return Err(McpRpcError::new(-32603, format!("Config error: {}", e))),
        },
    };

    // Check if we have entity schemas
    if full_config.entity_schemas.is_empty() {
        return handle_no_entity_schema(&input.stream_id, state).await;
    }

    // Get payload structure from storage
    let payload_structure = state.storage.analyze_payload(&input.stream_id).await.ok();

    // Get field mappings and target fields
    let field_mappings = full_config
        .parser
        .as_ref()
        .map(|p| p.field_mappings.clone())
        .unwrap_or_default();

    let entity_schema = full_config
        .entity_schemas
        .first()
        .map(|es| es.schema_name.clone())
        .unwrap_or_else(|| "undefined".to_string());

    let config_fields: Vec<String> = full_config
        .entity_schemas
        .first()
        .map(|es| es.attributes.iter().map(|a| a.name.clone()).collect())
        .unwrap_or_default();

    // Analyze based on whether we have payload structure
    let (status, analysis, notes, raw_payload_fields, raw_payload_nested) = match payload_structure
    {
        Some(ps) => analyze_with_payload(&config_fields, &field_mappings, &ps),
        None => analyze_without_payload(&config_fields),
    };

    let output = ValidateConfigOutput {
        stream_id: input.stream_id,
        entity_schema: Some(entity_schema),
        validation: ValidationResult {
            status,
            config_fields: config_fields.clone(),
            raw_payload_fields,
            raw_payload_nested,
            analysis,
            notes,
        },
    };

    create_tool_response(output)
}

/// Handle case where stream has no entity_schemas
async fn handle_no_entity_schema(stream_id: &str, state: &AppState) -> Result<Value, McpRpcError> {
    let payload_structure = state.storage.analyze_payload(stream_id).await.ok();

    let raw_payload_fields = payload_structure
        .as_ref()
        .map(|ps| ps.keys.clone())
        .unwrap_or_default();

    let output = ValidateConfigOutput {
        stream_id: stream_id.to_string(),
        entity_schema: None,
        validation: ValidationResult {
            status: "no_target_schema".to_string(),
            config_fields: vec![],
            raw_payload_fields,
            raw_payload_nested: None,
            analysis: ValidationAnalysis {
                in_config_not_in_payload: vec![],
                in_payload_not_in_config: vec![],
                matching: vec![],
                mapped_correctly: None,
                unmapped_nested_fields: None,
            },
            notes: Some("No entity_schemas defined for this stream. Add entity_schemas to config for Silver layer mapping.".to_string()),
        },
    };

    create_tool_response(output)
}

/// Analyze validation with available payload structure
fn analyze_with_payload(
    config_fields: &[String],
    field_mappings: &[crate::mcp::tools::traits::FieldMapping],
    payload_structure: &PayloadStructure,
) -> (
    String,
    ValidationAnalysis,
    Option<String>,
    Vec<String>,
    Option<std::collections::HashMap<String, Vec<String>>>,
) {
    let config_set: HashSet<_> = config_fields.iter().cloned().collect();
    let payload_set: HashSet<_> = payload_structure.keys.iter().cloned().collect();

    // Build mapping lookup: target_field -> source_path
    let mapping_lookup: std::collections::HashMap<_, _> = field_mappings
        .iter()
        .map(|m| (m.target_field.clone(), m.source_path.clone()))
        .collect();

    // Check which config fields have valid mappings to payload
    let mut matching = Vec::new();
    let mut in_config_not_in_payload = Vec::new();
    let mut mapped_correctly = Vec::new();

    for field in config_fields {
        if let Some(source_path) = mapping_lookup.get(field) {
            // This config field has a mapping - check if source exists
            let source_root = source_path.split('.').next().unwrap_or(source_path);

            if payload_set.contains(source_root) {
                // Source root exists in payload
                matching.push(field.clone());
                mapped_correctly.push(format!("{} -> {}", field, source_path));
            } else {
                // Mapping exists but source not found
                in_config_not_in_payload.push(field.clone());
            }
        } else if payload_set.contains(field) {
            // Direct match (field name matches payload key)
            matching.push(field.clone());
        } else {
            // Not mapped and not a direct match
            in_config_not_in_payload.push(field.clone());
        }
    }

    // Find payload fields not in config (using source roots from mappings)
    let mapped_source_roots: HashSet<_> = field_mappings
        .iter()
        .map(|m| {
            m.source_path
                .split('.')
                .next()
                .unwrap_or(&m.source_path)
                .to_string()
        })
        .collect();

    let in_payload_not_in_config: Vec<String> = payload_structure
        .keys
        .iter()
        .filter(|k| !config_set.contains(*k) && !mapped_source_roots.contains(*k))
        .cloned()
        .collect();

    // Calculate unmapped nested fields
    let unmapped_nested: Vec<String> = payload_structure
        .nested
        .iter()
        .flat_map(|(parent, children)| {
            children
                .iter()
                .map(|child| format!("{}.{}", parent, child))
                .filter(|path| !field_mappings.iter().any(|m| m.source_path == *path))
                .collect::<Vec<_>>()
        })
        .collect();

    // Determine status
    let status = if in_config_not_in_payload.is_empty() && in_payload_not_in_config.is_empty() {
        "match".to_string()
    } else if !mapped_correctly.is_empty() && in_config_not_in_payload.is_empty() {
        "mapped".to_string()
    } else if in_config_not_in_payload.is_empty() {
        "partial_match".to_string()
    } else {
        "mismatch".to_string()
    };

    // Generate notes
    let notes = if !in_config_not_in_payload.is_empty() {
        Some(format!(
            "Field(s) '{}' defined in config but not present in raw_payload. Verify field_mappings or source data.",
            in_config_not_in_payload.join("', '")
        ))
    } else if !in_payload_not_in_config.is_empty() {
        Some("Fields in raw_payload not defined in entity_schemas. These are available for future ETL development.".to_string())
    } else if !mapped_correctly.is_empty() {
        Some("Config uses flattened field names; raw_payload preserves source structure. Mapping verified via field_mappings.".to_string())
    } else {
        None
    };

    let analysis = ValidationAnalysis {
        in_config_not_in_payload,
        in_payload_not_in_config,
        matching,
        mapped_correctly: if mapped_correctly.is_empty() {
            None
        } else {
            Some(mapped_correctly)
        },
        unmapped_nested_fields: if unmapped_nested.is_empty() {
            None
        } else {
            Some(unmapped_nested)
        },
    };

    let raw_payload_nested = if payload_structure.nested.is_empty() {
        None
    } else {
        Some(payload_structure.nested.clone())
    };

    (
        status,
        analysis,
        notes,
        payload_structure.keys.clone(),
        raw_payload_nested,
    )
}

/// Analyze validation without payload structure (no Bronze data)
fn analyze_without_payload(
    config_fields: &[String],
) -> (
    String,
    ValidationAnalysis,
    Option<String>,
    Vec<String>,
    Option<std::collections::HashMap<String, Vec<String>>>,
) {
    let analysis = ValidationAnalysis {
        in_config_not_in_payload: config_fields.to_vec(),
        in_payload_not_in_config: vec![],
        matching: vec![],
        mapped_correctly: None,
        unmapped_nested_fields: None,
    };

    let notes = Some(
        "No Bronze data available. Cannot validate config against actual payload structure."
            .to_string(),
    );

    ("no_data".to_string(), analysis, notes, vec![], None)
}

// =============================================================================
// Tests - London School TDD (Mock-Driven, Behavior Verification)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::traits::{
        EntityAttribute, EntitySchema, FieldMapping, FullStreamConfig, MockBronzeStorage,
        MockConfigStore, ParserInfo, StreamConfigInfo,
    };
    use crate::mcp::ToolResponse;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_test_state(
        mock_storage: MockBronzeStorage,
        mock_config: MockConfigStore,
    ) -> AppState {
        AppState::new(Arc::new(mock_storage), Arc::new(mock_config))
    }

    fn simple_weather_config() -> FullStreamConfig {
        FullStreamConfig {
            info: StreamConfigInfo {
                stream_id: "simple-weather".to_string(),
                description: "Simple weather".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["http_poll".to_string()],
            },
            parser: None,
            entity_schemas: vec![EntitySchema {
                schema_name: "simple-weather".to_string(),
                attributes: vec![
                    EntityAttribute {
                        name: "temperature".to_string(),
                        data_type: "float".to_string(),
                        unit: None,
                        nullable: false,
                        description: None,
                    },
                    EntityAttribute {
                        name: "humidity".to_string(),
                        data_type: "float".to_string(),
                        unit: None,
                        nullable: false,
                        description: None,
                    },
                    EntityAttribute {
                        name: "pressure".to_string(),
                        data_type: "float".to_string(),
                        unit: None,
                        nullable: false,
                        description: None,
                    },
                ],
            }],
        }
    }

    fn nested_weather_config() -> FullStreamConfig {
        FullStreamConfig {
            info: StreamConfigInfo {
                stream_id: "nested-weather".to_string(),
                description: "Nested weather".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["http_poll".to_string()],
            },
            parser: Some(ParserInfo {
                parser_type: "json_path".to_string(),
                field_mappings: vec![FieldMapping {
                    source_path: "main.temp".to_string(),
                    target_field: "temperature".to_string(),
                    unit: None,
                }],
            }),
            entity_schemas: vec![EntitySchema {
                schema_name: "nested-weather".to_string(),
                attributes: vec![EntityAttribute {
                    name: "temperature".to_string(),
                    data_type: "float".to_string(),
                    unit: None,
                    nullable: false,
                    description: None,
                }],
            }],
        }
    }

    // -------------------------------------------------------------------------
    // TC-VC-020: Detects matching fields
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_config_matching_fields() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(simple_weather_config()));

        mock_storage.expect_analyze_payload().returning(|_| {
            Ok(PayloadStructure {
                keys: vec![
                    "temperature".to_string(),
                    "humidity".to_string(),
                    "pressure".to_string(),
                    "extra".to_string(),
                ],
                nested: HashMap::new(),
            })
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "simple-weather"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert_eq!(inner["validation"]["status"], "partial_match");

        let analysis = &inner["validation"]["analysis"];
        let matching = analysis["matching"].as_array().unwrap();
        assert_eq!(matching.len(), 3); // temperature, humidity, pressure

        let in_payload_not_config = analysis["in_payload_not_in_config"].as_array().unwrap();
        assert!(in_payload_not_config.iter().any(|v| v == "extra"));
    }

    // -------------------------------------------------------------------------
    // TC-VC-021: Reports fields in config but not in payload
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_config_missing_in_payload() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        // Config has rain_1h but payload doesn't
        let mut config = simple_weather_config();
        config.entity_schemas[0].attributes.push(EntityAttribute {
            name: "rain_1h".to_string(),
            data_type: "float".to_string(),
            unit: None,
            nullable: true,
            description: None,
        });

        mock_config
            .expect_get_full_config()
            .returning(move |_| Ok(config.clone()));

        mock_storage.expect_analyze_payload().returning(|_| {
            Ok(PayloadStructure {
                keys: vec!["temperature".to_string(), "humidity".to_string()],
                nested: HashMap::new(),
            })
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "weather-missing"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["validation"]["status"], "mismatch");

        let in_config_not_payload = inner["validation"]["analysis"]["in_config_not_in_payload"]
            .as_array()
            .unwrap();
        assert!(in_config_not_payload.iter().any(|v| v == "rain_1h"));
        assert!(in_config_not_payload.iter().any(|v| v == "pressure"));

        // Notes should mention missing fields
        assert!(inner["validation"]["notes"]
            .as_str()
            .unwrap()
            .contains("rain_1h"));
    }

    // -------------------------------------------------------------------------
    // TC-VC-022: Reports fields in payload but not in config
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_config_extra_in_payload() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        // Config only has temperature
        let mut config = simple_weather_config();
        config.entity_schemas[0].attributes = vec![EntityAttribute {
            name: "temperature".to_string(),
            data_type: "float".to_string(),
            unit: None,
            nullable: false,
            description: None,
        }];

        mock_config
            .expect_get_full_config()
            .returning(move |_| Ok(config.clone()));

        // Payload has more fields
        mock_storage.expect_analyze_payload().returning(|_| {
            Ok(PayloadStructure {
                keys: vec![
                    "temperature".to_string(),
                    "humidity".to_string(),
                    "pressure".to_string(),
                ],
                nested: HashMap::new(),
            })
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "weather-extra"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["validation"]["status"], "partial_match");

        let in_payload_not_config = inner["validation"]["analysis"]["in_payload_not_in_config"]
            .as_array()
            .unwrap();
        assert!(in_payload_not_config.iter().any(|v| v == "humidity"));
        assert!(in_payload_not_config.iter().any(|v| v == "pressure"));

        let in_config_not_payload = inner["validation"]["analysis"]["in_config_not_in_payload"]
            .as_array()
            .unwrap();
        assert!(in_config_not_payload.is_empty());
    }

    // -------------------------------------------------------------------------
    // TC-VC-023: Handles nested raw_payload structure
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_config_nested_payload() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(nested_weather_config()));

        let mut nested = HashMap::new();
        nested.insert(
            "main".to_string(),
            vec!["temp".to_string(), "humidity".to_string()],
        );
        nested.insert(
            "wind".to_string(),
            vec!["speed".to_string(), "deg".to_string()],
        );

        mock_storage.expect_analyze_payload().returning(move |_| {
            Ok(PayloadStructure {
                keys: vec!["main".to_string(), "wind".to_string()],
                nested: nested.clone(),
            })
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "nested-weather"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["validation"]["status"], "mapped");

        // Nested structure should be present
        assert!(inner["validation"]["raw_payload_nested"].is_object());

        // Mapped correctly should show the path
        let mapped = inner["validation"]["analysis"]["mapped_correctly"]
            .as_array()
            .unwrap();
        assert!(mapped
            .iter()
            .any(|v| v.as_str().unwrap().contains("temperature -> main.temp")));
    }

    // -------------------------------------------------------------------------
    // TC-VC-024: Handles stream with no entity_schemas
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_config_no_entity_schemas() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        // Config with empty entity_schemas
        let mut config = simple_weather_config();
        config.entity_schemas = vec![];

        mock_config
            .expect_get_full_config()
            .returning(move |_| Ok(config.clone()));

        mock_storage.expect_analyze_payload().returning(|_| {
            Ok(PayloadStructure {
                keys: vec!["temp".to_string(), "humidity".to_string()],
                nested: HashMap::new(),
            })
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "legacy-stream"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["validation"]["status"], "no_target_schema");
        assert!(inner["validation"]["notes"]
            .as_str()
            .unwrap()
            .contains("No entity_schemas"));
    }

    // -------------------------------------------------------------------------
    // Stream not found error
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_config_stream_not_found() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|id| Err(ConfigError::StreamNotFound(id.to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "nonexistent"
            }),
        )
        .await;

        // Assert - returns error response (ToolResponse with isError=true)
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        assert!(response.is_error());

        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert_eq!(inner["success"], false);
        assert_eq!(inner["code"], "STREAM_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_tool_definition_is_correct() {
        let def = tool_definition();
        assert_eq!(def.name, "validate_config");
        assert!(def.description.contains("Compare"));
        assert!(def.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("stream_id")));
    }
}
