//! describe_schema Tool Implementation
//!
//! Returns schema information for a stream with different view modes:
//! - `source`: Raw payload structure and field mappings
//! - `target`: Entity schemas (Silver layer target)
//! - `all`: Combined view with gap analysis
//!
//! # Response Format (mode: all)
//!
//! ```json
//! {
//!   "success": true,
//!   "stream_id": "outdoor-weather",
//!   "mode": "all",
//!   "source": {
//!     "raw_payload_structure": {...},
//!     "parser_type": "json_path",
//!     "field_mappings": [...]
//!   },
//!   "target": {
//!     "entity_schema": "nws-weather",
//!     "attributes": [...]
//!   },
//!   "gap_analysis": {
//!     "unmapped_source_fields": [...],
//!     "target_fields_without_mapping": [...]
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{McpError, McpResult};
use crate::etcd::ConfigStore;
use crate::mcp::protocol::McpToolResult;
use crate::storage::BronzeStorage;

/// Input parameters for describe_schema tool.
#[derive(Debug, Clone, Deserialize)]
pub struct DescribeSchemaArgs {
    /// Stream identifier (required)
    pub stream_id: String,

    /// Schema view mode (default: all)
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "all".to_string()
}

/// Field mapping from source to target.
#[derive(Debug, Clone, Serialize)]
pub struct FieldMapping {
    /// JSON path in raw_payload
    pub source_path: String,

    /// Target field name
    pub target_field: String,

    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// Source schema information.
#[derive(Debug, Clone, Serialize)]
pub struct SourceSchema {
    /// Structure of raw_payload from Parquet sample
    pub raw_payload_structure: RawPayloadStructure,

    /// Parser type (flat_json, json_path, etc.)
    pub parser_type: String,

    /// Field mappings from config
    pub field_mappings: Vec<FieldMapping>,
}

/// Structure of raw_payload keys.
#[derive(Debug, Clone, Serialize)]
pub struct RawPayloadStructure {
    /// Top-level keys
    pub keys: Vec<String>,

    /// Nested object keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested: Option<Value>,
}

/// Target schema attribute.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaAttribute {
    /// Attribute name
    pub name: String,

    /// Data type
    #[serde(rename = "type")]
    pub attr_type: String,

    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Whether nullable
    pub nullable: bool,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Valid range for numeric types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Vec<f64>>,
}

/// Target schema information.
#[derive(Debug, Clone, Serialize)]
pub struct TargetSchema {
    /// Entity schema name
    pub entity_schema: String,

    /// Schema attributes
    pub attributes: Vec<SchemaAttribute>,
}

/// Gap analysis between source and target.
#[derive(Debug, Clone, Serialize)]
pub struct GapAnalysis {
    /// Source fields not mapped to any target field
    pub unmapped_source_fields: Vec<String>,

    /// Target fields without a source mapping
    pub target_fields_without_mapping: Vec<String>,
}

/// Response for mode: source
#[derive(Debug, Clone, Serialize)]
pub struct SourceModeResponse {
    pub success: bool,
    pub stream_id: String,
    pub mode: String,
    pub raw_payload_structure: RawPayloadStructure,
    pub parser_type: String,
    pub field_mappings: Vec<FieldMapping>,
    pub unmapped_source_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_analyzed: Option<String>,
}

/// Response for mode: target
#[derive(Debug, Clone, Serialize)]
pub struct TargetModeResponse {
    pub success: bool,
    pub stream_id: String,
    pub mode: String,
    pub entity_schema: String,
    pub attributes: Vec<SchemaAttribute>,
}

/// Response for mode: all
#[derive(Debug, Clone, Serialize)]
pub struct AllModeResponse {
    pub success: bool,
    pub stream_id: String,
    pub mode: String,
    pub source: SourceSchema,
    pub target: TargetSchema,
    pub gap_analysis: GapAnalysis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_analyzed: Option<String>,
}

/// Execute the describe_schema tool.
///
/// # Arguments
///
/// * `storage` - Bronze storage for Parquet schema inspection
/// * `config_store` - etcd config store for field mappings and entity schemas
/// * `args` - Tool arguments (stream_id, mode)
pub async fn execute<S, C>(storage: &S, config_store: &C, args: Value) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
    C: ConfigStore + ?Sized,
{
    let args: DescribeSchemaArgs = serde_json::from_value(args)
        .map_err(|e| McpError::InvalidRequest(format!("Invalid arguments: {}", e)))?;

    // Validate stream_id format
    validate_stream_id(&args.stream_id)?;

    // Validate mode
    if !["all", "source", "target"].contains(&args.mode.as_str()) {
        return Err(McpError::InvalidRequest(format!(
            "Invalid mode '{}'. Must be one of: all, source, target",
            args.mode
        )));
    }

    // Verify stream exists by trying to get config (will return StreamNotFound if not exists)
    let _config = config_store.get_config(&args.stream_id).await?;

    match args.mode.as_str() {
        "source" => execute_source_mode(storage, config_store, &args.stream_id).await,
        "target" => execute_target_mode(config_store, &args.stream_id).await,
        "all" | _ => execute_all_mode(storage, config_store, &args.stream_id).await,
    }
}

/// Execute source mode - raw_payload structure and field mappings.
async fn execute_source_mode<S, C>(
    storage: &S,
    config_store: &C,
    stream_id: &str,
) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
    C: ConfigStore + ?Sized,
{
    // Get schema info from storage (includes raw_payload_structure)
    let schema_info = storage.get_schema(stream_id).await?;

    // Get config with field mappings from etcd
    let config = config_store.get_config(stream_id).await?;

    // Convert field mappings to our response type
    let field_mappings: Vec<FieldMapping> = config
        .field_mappings
        .iter()
        .map(|m| FieldMapping {
            source_path: m.source.clone(),
            target_field: m.target.clone().unwrap_or_else(|| m.source.clone()),
            unit: None,
        })
        .collect();

    // Build raw_payload structure from schema info
    let structure = schema_info
        .raw_payload_structure
        .map(|s| RawPayloadStructure {
            keys: s.keys,
            nested: s
                .nested
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        serde_json::Value::Array(
                            v.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    )
                })
                .next()
                .map(|(_, v)| v),
        })
        .unwrap_or_else(|| RawPayloadStructure {
            keys: vec![],
            nested: None,
        });

    // Calculate unmapped fields
    let mapped_fields: Vec<String> = field_mappings
        .iter()
        .map(|m| m.source_path.split('.').next().unwrap_or("").to_string())
        .collect();

    let unmapped: Vec<String> = structure
        .keys
        .iter()
        .filter(|k| !mapped_fields.contains(k))
        .cloned()
        .collect();

    let response = SourceModeResponse {
        success: true,
        stream_id: stream_id.to_string(),
        mode: "source".to_string(),
        raw_payload_structure: structure,
        parser_type: config.source_type.clone(),
        field_mappings,
        unmapped_source_fields: unmapped,
        file_analyzed: Some(schema_info.file_path),
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Execute target mode - entity schemas.
async fn execute_target_mode<C>(config_store: &C, stream_id: &str) -> McpResult<McpToolResult>
where
    C: ConfigStore + ?Sized,
{
    let config = config_store.get_config(stream_id).await?;

    // Convert entity schema attributes to our response type
    let attributes: Vec<SchemaAttribute> = config
        .entity_schema
        .attributes
        .iter()
        .map(|a| SchemaAttribute {
            name: a.name.clone(),
            attr_type: a.attr_type.clone(),
            unit: a.unit.clone(),
            nullable: !a.required,
            description: None,
            range: None,
        })
        .collect();

    let response = TargetModeResponse {
        success: true,
        stream_id: stream_id.to_string(),
        mode: "target".to_string(),
        entity_schema: config.entity_schema.name.clone(),
        attributes,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Execute all mode - combined view with gap analysis.
async fn execute_all_mode<S, C>(
    storage: &S,
    config_store: &C,
    stream_id: &str,
) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
    C: ConfigStore + ?Sized,
{
    // Get schema info from storage
    let schema_info = storage.get_schema(stream_id).await?;

    // Get config from etcd
    let config = config_store.get_config(stream_id).await?;

    // Build raw_payload structure from schema info
    let structure = schema_info
        .raw_payload_structure
        .map(|s| RawPayloadStructure {
            keys: s.keys,
            nested: s
                .nested
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        serde_json::Value::Array(
                            v.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    )
                })
                .next()
                .map(|(_, v)| v),
        })
        .unwrap_or_else(|| RawPayloadStructure {
            keys: vec![],
            nested: None,
        });

    // Convert field mappings to our response type
    let field_mappings: Vec<FieldMapping> = config
        .field_mappings
        .iter()
        .map(|m| FieldMapping {
            source_path: m.source.clone(),
            target_field: m.target.clone().unwrap_or_else(|| m.source.clone()),
            unit: None,
        })
        .collect();

    // Convert entity schema attributes to our response type
    let attributes: Vec<SchemaAttribute> = config
        .entity_schema
        .attributes
        .iter()
        .map(|a| SchemaAttribute {
            name: a.name.clone(),
            attr_type: a.attr_type.clone(),
            unit: a.unit.clone(),
            nullable: !a.required,
            description: None,
            range: None,
        })
        .collect();

    // Calculate gap analysis
    let mapped_source_paths: Vec<String> = field_mappings
        .iter()
        .map(|m| m.source_path.split('.').next().unwrap_or("").to_string())
        .collect();

    let mapped_target_fields: Vec<String> = field_mappings
        .iter()
        .map(|m| m.target_field.clone())
        .collect();

    let unmapped_source: Vec<String> = structure
        .keys
        .iter()
        .filter(|k| !mapped_source_paths.contains(k))
        .cloned()
        .collect();

    let unmapped_target: Vec<String> = attributes
        .iter()
        .map(|a| a.name.clone())
        .filter(|name| !mapped_target_fields.contains(name))
        .collect();

    let response = AllModeResponse {
        success: true,
        stream_id: stream_id.to_string(),
        mode: "all".to_string(),
        source: SourceSchema {
            raw_payload_structure: structure,
            parser_type: config.source_type.clone(),
            field_mappings,
        },
        target: TargetSchema {
            entity_schema: config.entity_schema.name.clone(),
            attributes,
        },
        gap_analysis: GapAnalysis {
            unmapped_source_fields: unmapped_source,
            target_fields_without_mapping: unmapped_target,
        },
        file_analyzed: Some(schema_info.file_path),
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Validate stream_id format (kebab-case).
pub(crate) fn validate_stream_id(stream_id: &str) -> McpResult<()> {
    if stream_id.is_empty() {
        return Err(McpError::InvalidParams(
            "stream_id cannot be empty".to_string(),
        ));
    }

    if stream_id.len() > 64 {
        return Err(McpError::InvalidParams(
            "stream_id cannot exceed 64 characters".to_string(),
        ));
    }

    // Must start with lowercase letter
    let first_char = stream_id.chars().next().unwrap();
    if !first_char.is_ascii_lowercase() {
        return Err(McpError::InvalidParams(
            "stream_id must start with a lowercase letter".to_string(),
        ));
    }

    // Must contain only lowercase letters, digits, and hyphens
    for c in stream_id.chars() {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' {
            return Err(McpError::InvalidParams(format!(
                "stream_id contains invalid character '{}'. Only lowercase letters, digits, and hyphens are allowed",
                c
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_stream_id_valid() {
        assert!(validate_stream_id("air-quality").is_ok());
        assert!(validate_stream_id("outdoor-weather").is_ok());
        assert!(validate_stream_id("nws-forecast-hourly").is_ok());
        assert!(validate_stream_id("stream123").is_ok());
    }

    #[test]
    fn test_validate_stream_id_invalid() {
        assert!(validate_stream_id("").is_err());
        assert!(validate_stream_id("Air-Quality").is_err());
        assert!(validate_stream_id("123-stream").is_err());
        assert!(validate_stream_id("stream_name").is_err());
        assert!(validate_stream_id("stream name").is_err());
    }
}
