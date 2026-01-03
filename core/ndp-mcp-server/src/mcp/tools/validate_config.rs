//! validate_config Tool Implementation
//!
//! Compares stream configuration in etcd against actual Bronze Parquet schema.
//! Detects mismatches between configured field mappings and actual data fields.
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
//!     "config_fields": [...],
//!     "raw_payload_fields": [...],
//!     "analysis": {
//!       "in_config_not_in_payload": [...],
//!       "in_payload_not_in_config": [...],
//!       "matching": [...]
//!     },
//!     "notes": "..."
//!   }
//! }
//! ```

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{McpError, McpResult};
use crate::etcd::ConfigStore;
use crate::mcp::protocol::McpToolResult;
use crate::storage::BronzeStorage;

/// Input parameters for validate_config tool.
#[derive(Debug, Clone, Deserialize)]
pub struct ValidateConfigArgs {
    /// Stream identifier (required)
    pub stream_id: String,
}

/// Validation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    /// All config fields found in data
    Match,
    /// Some fields match, some don't
    Partial,
    /// Significant mismatch between config and data
    Mismatch,
}

/// Field analysis breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct FieldAnalysis {
    /// Config fields not found in raw_payload
    pub in_config_not_in_payload: Vec<String>,

    /// raw_payload fields not mapped in config
    pub in_payload_not_in_config: Vec<String>,

    /// Fields that match between config and data
    pub matching: Vec<String>,
}

/// Validation details.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationDetails {
    /// Overall validation status
    pub status: ValidationStatus,

    /// Fields defined in config (mapped field names)
    pub config_fields: Vec<String>,

    /// Fields found in raw_payload
    pub raw_payload_fields: Vec<String>,

    /// Detailed analysis
    pub analysis: FieldAnalysis,

    /// Human-readable notes explaining the validation
    pub notes: String,
}

/// Response structure for validate_config.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateConfigResponse {
    /// Success flag
    pub success: bool,

    /// Stream identifier
    pub stream_id: String,

    /// Entity schema name
    pub entity_schema: String,

    /// Validation details
    pub validation: ValidationDetails,
}

/// Execute the validate_config tool.
///
/// # Arguments
///
/// * `storage` - Bronze storage for Parquet inspection
/// * `config_store` - etcd config store for field mappings
/// * `args` - Tool arguments (stream_id)
pub async fn execute<S, C>(storage: &S, config_store: &C, args: Value) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
    C: ConfigStore + ?Sized,
{
    let args: ValidateConfigArgs = serde_json::from_value(args)
        .map_err(|e| McpError::InvalidRequest(format!("Invalid arguments: {}", e)))?;

    // Validate stream_id format
    super::describe_schema::validate_stream_id(&args.stream_id)?;

    // Get stream config (will return StreamNotFound if not exists)
    let config = config_store.get_config(&args.stream_id).await?;

    // Get schema info from storage (includes raw_payload_structure)
    let schema_info = storage.get_schema(&args.stream_id).await?;

    // Extract config fields (target field names from mappings)
    let config_fields: Vec<String> = config
        .field_mappings
        .iter()
        .map(|m| m.target.clone().unwrap_or_else(|| m.source.clone()))
        .collect();

    // Extract source paths from mappings for comparison
    let mapped_source_paths: HashSet<String> = config
        .field_mappings
        .iter()
        .map(|m| {
            // Get top-level key from path like "main.temp" -> "main"
            m.source.split('.').next().unwrap_or("").to_string()
        })
        .collect();

    // Raw payload fields (top-level keys) from schema info
    let raw_payload_keys: Vec<String> = schema_info
        .raw_payload_structure
        .map(|s| s.keys)
        .unwrap_or_default();

    let raw_payload_fields: HashSet<String> = raw_payload_keys.iter().cloned().collect();

    // Calculate analysis
    let in_config_not_in_payload: Vec<String> = mapped_source_paths
        .iter()
        .filter(|f| !f.is_empty() && !raw_payload_fields.contains(*f))
        .cloned()
        .collect();

    let in_payload_not_in_config: Vec<String> = raw_payload_fields
        .iter()
        .filter(|f| !mapped_source_paths.contains(*f))
        .cloned()
        .collect();

    let matching: Vec<String> = mapped_source_paths
        .iter()
        .filter(|f| !f.is_empty() && raw_payload_fields.contains(*f))
        .cloned()
        .collect();

    // Determine status
    let status = if in_config_not_in_payload.is_empty() && in_payload_not_in_config.is_empty() {
        ValidationStatus::Match
    } else if !matching.is_empty()
        && (in_config_not_in_payload.len() + in_payload_not_in_config.len()) <= matching.len()
    {
        ValidationStatus::Partial
    } else {
        ValidationStatus::Mismatch
    };

    // Generate notes
    let notes = generate_notes(
        &config.source_type,
        &in_config_not_in_payload,
        &in_payload_not_in_config,
    );

    let response = ValidateConfigResponse {
        success: true,
        stream_id: args.stream_id,
        entity_schema: config.entity_schema.name.clone(),
        validation: ValidationDetails {
            status,
            config_fields,
            raw_payload_fields: raw_payload_keys,
            analysis: FieldAnalysis {
                in_config_not_in_payload,
                in_payload_not_in_config,
                matching,
            },
            notes,
        },
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Generate human-readable notes explaining the validation result.
fn generate_notes(
    parser_type: &str,
    in_config_not_payload: &[String],
    in_payload_not_config: &[String],
) -> String {
    let mut notes = Vec::new();

    if parser_type == "json_path" {
        notes.push(
            "Config uses flattened field names; raw_payload preserves source structure \
             (e.g., main.temp, wind.speed). Mapping happens in Silver layer via parser field_mappings."
                .to_string(),
        );
    }

    if !in_config_not_payload.is_empty() {
        notes.push(format!(
            "Fields in config but not in raw_payload: {}. These may be nested fields or optional fields not present in the sample.",
            in_config_not_payload.join(", ")
        ));
    }

    if !in_payload_not_config.is_empty() {
        notes.push(format!(
            "Fields in raw_payload but not mapped in config: {}. These fields are preserved in Bronze but won't be extracted to Silver.",
            in_payload_not_config.join(", ")
        ));
    }

    if notes.is_empty() {
        "All mapped fields are present in raw_payload.".to_string()
    } else {
        notes.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ValidationStatus::Match).unwrap(),
            "\"match\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationStatus::Partial).unwrap(),
            "\"partial\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationStatus::Mismatch).unwrap(),
            "\"mismatch\""
        );
    }

    #[test]
    fn test_generate_notes_json_path() {
        let notes = generate_notes("json_path", &[], &[]);
        assert!(notes.contains("flattened field names"));
    }

    #[test]
    fn test_generate_notes_missing_fields() {
        let notes = generate_notes("flat_json", &["missing_field".to_string()], &[]);
        assert!(notes.contains("missing_field"));
        assert!(notes.contains("not in raw_payload"));
    }
}
