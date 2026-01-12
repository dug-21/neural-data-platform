//! describe_schema MCP Tool (dp-005)
//!
//! Returns schema information for a stream with three modes:
//! - `source`: raw_payload structure + field mappings
//! - `target`: entity_schemas attributes
//! - `all`: combined view with gap_analysis
//!
//! # Response Format (mode=all)
//!
//! ```json
//! {
//!   "success": true,
//!   "stream_id": "outdoor-weather",
//!   "mode": "all",
//!   "source": {
//!     "raw_payload_structure": { "keys": [...], "nested": {...} },
//!     "field_mappings": [...]
//!   },
//!   "target": {
//!     "entity_schema": "nws-weather",
//!     "attributes": [...]
//!   },
//!   "gap_analysis": {
//!     "unmapped_source_fields": [...],
//!     "target_fields_without_mapping": [...]
//!   },
//!   "file_analyzed": "/data/raw/..."
//! }
//! ```

use crate::mcp::tools::{
    create_error_response, create_tool_response, error_codes,
    traits::{ConfigError, EntityAttribute, FieldMapping, PayloadStructure},
    AppState,
};
use crate::mcp::{JsonRpcError, McpRpcError, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

// =============================================================================
// Input/Output Types
// =============================================================================

/// Input schema for describe_schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeSchemaInput {
    pub stream_id: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "all".to_string()
}

/// Source schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_payload_structure: Option<PayloadStructure>,
    pub parser_type: String,
    pub field_mappings: Vec<FieldMapping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmapped_source_fields: Option<Vec<String>>,
}

/// Target schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetSchema {
    pub entity_schema: String,
    pub attributes: Vec<EntityAttribute>,
}

/// Gap analysis between source and target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapAnalysis {
    pub unmapped_source_fields: Vec<String>,
    pub target_fields_without_mapping: Vec<String>,
}

/// Output for mode=source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceModeOutput {
    pub stream_id: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_payload_structure: Option<PayloadStructure>,
    pub parser_type: String,
    pub field_mappings: Vec<FieldMapping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmapped_source_fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_analyzed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Output for mode=target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetModeOutput {
    pub stream_id: String,
    pub mode: String,
    pub entity_schema: String,
    pub attributes: Vec<EntityAttribute>,
}

/// Output for mode=all
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllModeOutput {
    pub stream_id: String,
    pub mode: String,
    pub source: SourceSchema,
    pub target: TargetSchema,
    pub gap_analysis: GapAnalysis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_analyzed: Option<String>,
}

// =============================================================================
// Tool Definition
// =============================================================================

/// Get the MCP tool definition for describe_schema
pub fn tool_definition() -> ToolDefinition {
    ToolDefinition::new(
        "describe_schema",
        "Get schema information for a stream. Modes: source (raw_payload structure + field mappings), target (entity_schemas), all (complete ETL picture with gap analysis)",
        json!({
            "type": "object",
            "properties": {
                "stream_id": {
                    "type": "string",
                    "description": "The stream identifier (e.g., 'air-quality', 'outdoor-weather')"
                },
                "mode": {
                    "type": "string",
                    "enum": ["all", "source", "target"],
                    "description": "Schema view mode (default: all)",
                    "default": "all"
                }
            },
            "required": ["stream_id"]
        }),
    )
}

// =============================================================================
// Tool Execution
// =============================================================================

/// Execute the describe_schema tool
///
/// # Arguments
/// * `state` - Application state with injected dependencies
/// * `args` - Input arguments containing stream_id and mode
///
/// # Returns
/// MCP ToolResponse as JSON Value
pub async fn execute(state: &AppState, args: Value) -> Result<Value, McpRpcError> {
    // Parse input
    let input: DescribeSchemaInput = serde_json::from_value(args).map_err(|e| {
        McpRpcError::new(
            JsonRpcError::INVALID_PARAMS,
            format!("Invalid input: {}", e),
        )
    })?;

    let mode = input.mode.to_lowercase();
    if !["all", "source", "target"].contains(&mode.as_str()) {
        return create_error_response(
            error_codes::INVALID_PARAMETER,
            &format!(
                "Invalid mode: '{}'. Must be one of: all, source, target",
                mode
            ),
            Some(json!({"parameter": "mode", "value": mode})),
        );
    }

    // Get full config from etcd
    let full_config = match state.config.get_full_config(&input.stream_id).await {
        Ok(config) => config,
        Err(e) => match e {
            ConfigError::StreamNotFound(id) => {
                return create_error_response(
                    error_codes::STREAM_NOT_FOUND,
                    &format!("Stream not found: {}", id),
                    None,
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

    // Get storage analysis (best effort)
    let payload_structure = state.storage.analyze_payload(&input.stream_id).await.ok();
    let file_path = state
        .storage
        .get_latest_file_path(&input.stream_id)
        .await
        .ok()
        .flatten();

    // Execute based on mode
    match mode.as_str() {
        "source" => {
            execute_source_mode(&input.stream_id, &full_config, payload_structure, file_path)
        }
        "target" => execute_target_mode(&input.stream_id, &full_config),
        "all" => execute_all_mode(&input.stream_id, &full_config, payload_structure, file_path),
        _ => unreachable!(),
    }
}

/// Execute source mode - raw_payload structure + mappings
fn execute_source_mode(
    stream_id: &str,
    config: &crate::mcp::tools::traits::FullStreamConfig,
    payload_structure: Option<PayloadStructure>,
    file_path: Option<String>,
) -> Result<Value, McpRpcError> {
    let parser = config.parser.as_ref();
    let parser_type = parser
        .map(|p| p.parser_type.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let field_mappings = parser.map(|p| p.field_mappings.clone()).unwrap_or_default();

    // Calculate unmapped source fields
    let unmapped = payload_structure.as_ref().map(|ps| {
        let mapped_sources: HashSet<_> = field_mappings
            .iter()
            .map(|m| m.source_path.split('.').next().unwrap_or(&m.source_path))
            .collect();

        ps.keys
            .iter()
            .filter(|k| !mapped_sources.contains(k.as_str()))
            .cloned()
            .collect::<Vec<_>>()
    });

    let note = if payload_structure.is_none() {
        Some(
            "No Bronze data available for this stream. Schema derived from config only."
                .to_string(),
        )
    } else {
        None
    };

    let output = SourceModeOutput {
        stream_id: stream_id.to_string(),
        mode: "source".to_string(),
        raw_payload_structure: payload_structure,
        parser_type,
        field_mappings,
        unmapped_source_fields: unmapped,
        file_analyzed: file_path,
        note,
    };

    create_tool_response(output)
}

/// Execute target mode - entity_schemas
fn execute_target_mode(
    stream_id: &str,
    config: &crate::mcp::tools::traits::FullStreamConfig,
) -> Result<Value, McpRpcError> {
    let entity_schema = config
        .entity_schemas
        .first()
        .map(|es| es.schema_name.clone())
        .unwrap_or_else(|| "undefined".to_string());

    let attributes = config
        .entity_schemas
        .first()
        .map(|es| es.attributes.clone())
        .unwrap_or_default();

    let output = TargetModeOutput {
        stream_id: stream_id.to_string(),
        mode: "target".to_string(),
        entity_schema,
        attributes,
    };

    create_tool_response(output)
}

/// Execute all mode - combined with gap analysis
fn execute_all_mode(
    stream_id: &str,
    config: &crate::mcp::tools::traits::FullStreamConfig,
    payload_structure: Option<PayloadStructure>,
    file_path: Option<String>,
) -> Result<Value, McpRpcError> {
    let parser = config.parser.as_ref();
    let parser_type = parser
        .map(|p| p.parser_type.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let field_mappings = parser.map(|p| p.field_mappings.clone()).unwrap_or_default();

    // Calculate unmapped source fields
    let mapped_source_fields: HashSet<_> = field_mappings
        .iter()
        .map(|m| {
            m.source_path
                .split('.')
                .next()
                .unwrap_or(&m.source_path)
                .to_string()
        })
        .collect();

    let unmapped_source: Vec<String> = payload_structure
        .as_ref()
        .map(|ps| {
            ps.keys
                .iter()
                .filter(|k| !mapped_source_fields.contains(*k))
                .cloned()
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    // Calculate target fields without mapping
    let mapped_target_fields: HashSet<_> = field_mappings
        .iter()
        .map(|m| m.target_field.clone())
        .collect();

    let entity_schema = config
        .entity_schemas
        .first()
        .map(|es| es.schema_name.clone())
        .unwrap_or_else(|| "undefined".to_string());

    let attributes = config
        .entity_schemas
        .first()
        .map(|es| es.attributes.clone())
        .unwrap_or_default();

    let target_without_mapping: Vec<String> = attributes
        .iter()
        .filter(|a| !mapped_target_fields.contains(&a.name))
        .map(|a| a.name.clone())
        .collect();

    let source = SourceSchema {
        raw_payload_structure: payload_structure,
        parser_type,
        field_mappings,
        unmapped_source_fields: if unmapped_source.is_empty() {
            None
        } else {
            Some(unmapped_source.clone())
        },
    };

    let target = TargetSchema {
        entity_schema,
        attributes,
    };

    let gap_analysis = GapAnalysis {
        unmapped_source_fields: unmapped_source,
        target_fields_without_mapping: target_without_mapping,
    };

    let output = AllModeOutput {
        stream_id: stream_id.to_string(),
        mode: "all".to_string(),
        source,
        target,
        gap_analysis,
        file_analyzed: file_path,
    };

    create_tool_response(output)
}

// =============================================================================
// Tests - London School TDD (Mock-Driven, Behavior Verification)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::traits::{
        EntitySchema, FullStreamConfig, MockBronzeStorage, MockConfigStore, ParserInfo,
        StorageError, StreamConfigInfo,
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

    fn sample_full_config() -> FullStreamConfig {
        FullStreamConfig {
            info: StreamConfigInfo {
                stream_id: "outdoor-weather".to_string(),
                description: "Outdoor weather data".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["http_poll".to_string()],
            },
            parser: Some(ParserInfo {
                parser_type: "json_path".to_string(),
                field_mappings: vec![
                    FieldMapping {
                        source_path: "main.temp".to_string(),
                        target_field: "temperature".to_string(),
                        unit: Some("celsius".to_string()),
                    },
                    FieldMapping {
                        source_path: "main.humidity".to_string(),
                        target_field: "humidity".to_string(),
                        unit: Some("percent".to_string()),
                    },
                ],
            }),
            entity_schemas: vec![EntitySchema {
                schema_name: "nws-weather".to_string(),
                attributes: vec![
                    EntityAttribute {
                        name: "temperature".to_string(),
                        data_type: "float".to_string(),
                        unit: Some("celsius".to_string()),
                        nullable: false,
                        description: Some("Current temperature".to_string()),
                    },
                    EntityAttribute {
                        name: "humidity".to_string(),
                        data_type: "float".to_string(),
                        unit: Some("percent".to_string()),
                        nullable: true,
                        description: None,
                    },
                    EntityAttribute {
                        name: "rain_1h".to_string(),
                        data_type: "float".to_string(),
                        unit: Some("mm".to_string()),
                        nullable: true,
                        description: None,
                    },
                ],
            }],
        }
    }

    fn sample_payload_structure() -> PayloadStructure {
        let mut nested = HashMap::new();
        nested.insert(
            "main".to_string(),
            vec![
                "temp".to_string(),
                "humidity".to_string(),
                "pressure".to_string(),
            ],
        );
        nested.insert(
            "wind".to_string(),
            vec!["speed".to_string(), "deg".to_string()],
        );

        PayloadStructure {
            keys: vec![
                "main".to_string(),
                "wind".to_string(),
                "clouds".to_string(),
                "base".to_string(),
            ],
            nested,
        }
    }

    // -------------------------------------------------------------------------
    // TC-DS-010: Mode=source returns raw_payload structure + mappings
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_describe_schema_source_mode() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .with(mockall::predicate::eq("outdoor-weather"))
            .returning(|_| Ok(sample_full_config()));

        mock_storage
            .expect_analyze_payload()
            .returning(|_| Ok(sample_payload_structure()));

        mock_storage.expect_get_latest_file_path().returning(|_| {
            Ok(Some(
                "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet".to_string(),
            ))
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "outdoor-weather",
                "mode": "source"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert_eq!(inner["mode"], "source");
        assert_eq!(inner["parser_type"], "json_path");

        // raw_payload_structure present
        assert!(inner["raw_payload_structure"]["keys"].is_array());

        // field_mappings present
        let mappings = inner["field_mappings"].as_array().unwrap();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0]["source_path"], "main.temp");

        // unmapped_source_fields calculated
        let unmapped = inner["unmapped_source_fields"].as_array().unwrap();
        assert!(unmapped.iter().any(|v| v == "clouds"));
        assert!(unmapped.iter().any(|v| v == "base"));

        // file_analyzed present
        assert!(inner["file_analyzed"]
            .as_str()
            .unwrap()
            .contains("outdoor-weather"));
    }

    // -------------------------------------------------------------------------
    // TC-DS-011: Mode=target returns entity_schemas
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_describe_schema_target_mode() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(sample_full_config()));

        // Target mode still calls these but doesn't use them
        mock_storage
            .expect_analyze_payload()
            .returning(|_| Ok(sample_payload_structure()));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(None));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "outdoor-weather",
                "mode": "target"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert_eq!(inner["mode"], "target");
        assert_eq!(inner["entity_schema"], "nws-weather");

        let attributes = inner["attributes"].as_array().unwrap();
        assert_eq!(attributes.len(), 3);
        assert_eq!(attributes[0]["name"], "temperature");
        assert_eq!(attributes[0]["type"], "float");
    }

    // -------------------------------------------------------------------------
    // TC-DS-012: Mode=all includes gap_analysis
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_describe_schema_all_mode_with_gap_analysis() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(sample_full_config()));

        mock_storage
            .expect_analyze_payload()
            .returning(|_| Ok(sample_payload_structure()));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(Some("/data/raw/test/data.parquet".to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "outdoor-weather",
                "mode": "all"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert_eq!(inner["mode"], "all");

        // source section
        assert!(inner["source"]["raw_payload_structure"].is_object());
        assert!(inner["source"]["field_mappings"].is_array());

        // target section
        assert_eq!(inner["target"]["entity_schema"], "nws-weather");

        // gap_analysis section
        let gap = &inner["gap_analysis"];
        assert!(gap["unmapped_source_fields"].is_array());

        // rain_1h is in target but not mapped
        let target_without = gap["target_fields_without_mapping"].as_array().unwrap();
        assert!(target_without.iter().any(|v| v == "rain_1h"));
    }

    // -------------------------------------------------------------------------
    // TC-DS-013: Handles stream without data file
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_describe_schema_no_data_file() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(sample_full_config()));

        // No data available
        mock_storage
            .expect_analyze_payload()
            .returning(|_| Err(StorageError::NoDataAvailable("test".to_string())));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(None));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "nws-forecast-hourly",
                "mode": "source"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert!(inner["raw_payload_structure"].is_null());
        assert!(inner["note"].as_str().unwrap().contains("No Bronze data"));
    }

    // -------------------------------------------------------------------------
    // TC-DS-014: Mode defaults to 'all'
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_describe_schema_default_mode() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(sample_full_config()));

        mock_storage
            .expect_analyze_payload()
            .returning(|_| Ok(sample_payload_structure()));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(None));

        let state = create_test_state(mock_storage, mock_config);

        // Act - no mode specified
        let result = execute(
            &state,
            json!({
                "stream_id": "air-quality"
            }),
        )
        .await;

        // Assert - should use "all" mode
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["mode"], "all");
        assert!(inner["source"].is_object());
        assert!(inner["target"].is_object());
        assert!(inner["gap_analysis"].is_object());
    }

    // -------------------------------------------------------------------------
    // TC-DS-015: Invalid stream_id returns error
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_describe_schema_stream_not_found() {
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
                "stream_id": "nonexistent-stream",
                "mode": "all"
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
    async fn test_describe_schema_invalid_mode() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_full_config()
            .returning(|_| Ok(sample_full_config()));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "air-quality",
                "mode": "invalid"
            }),
        )
        .await;

        // Assert - returns error response, not Err
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        assert!(response.is_error());

        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert_eq!(inner["success"], false);
        assert_eq!(inner["code"], "INVALID_PARAMETER");
    }

    #[tokio::test]
    async fn test_tool_definition_is_correct() {
        let def = tool_definition();
        assert_eq!(def.name, "describe_schema");
        assert!(def.description.contains("schema"));
        assert!(def.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("stream_id")));
    }
}
