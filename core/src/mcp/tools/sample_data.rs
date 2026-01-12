//! sample_data MCP Tool (dp-005)
//!
//! Retrieves sample rows from a Bronze stream for exploration.
//! Returns rows in Bronze envelope format with timestamp, source_id,
//! ndp_id, context, and raw_payload.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "stream_id": "air-quality",
//!   "row_count": 5,
//!   "rows": [
//!     {
//!       "timestamp": 1767452639760716,
//!       "source_id": "air-quality-Mqtt",
//!       "ndp_id": "sensor-001",
//!       "context": {"location": {...}},
//!       "raw_payload": {"pm25": 12.5, ...}
//!     }
//!   ],
//!   "source_file": "/data/raw/air-quality/.../data.parquet"
//! }
//! ```

use crate::mcp::tools::{
    create_error_response, create_tool_response, error_codes,
    traits::{BronzeRow, ConfigError, StorageError},
    AppState,
};
use crate::mcp::{JsonRpcError, McpRpcError, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// =============================================================================
// Constants
// =============================================================================

/// Default number of rows to return
const DEFAULT_N: usize = 10;

/// Maximum number of rows allowed
const MAX_N: usize = 100;

// =============================================================================
// Input/Output Types
// =============================================================================

/// Input schema for sample_data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleDataInput {
    pub stream_id: String,
    #[serde(default = "default_n")]
    pub n: usize,
}

fn default_n() -> usize {
    DEFAULT_N
}

/// sample_data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleDataOutput {
    pub stream_id: String,
    pub row_count: usize,
    pub rows: Vec<BronzeRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

// =============================================================================
// Tool Definition
// =============================================================================

/// Get the MCP tool definition for sample_data
pub fn tool_definition() -> ToolDefinition {
    ToolDefinition::new(
        "sample_data",
        "Retrieve sample rows from a Bronze stream for exploration. Returns rows in Bronze envelope format with timestamp, source_id, ndp_id, context, and raw_payload.",
        json!({
            "type": "object",
            "properties": {
                "stream_id": {
                    "type": "string",
                    "description": "The stream identifier"
                },
                "n": {
                    "type": "integer",
                    "description": "Number of rows to return (default: 10, max: 100)",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": ["stream_id"]
        }),
    )
}

// =============================================================================
// Tool Execution
// =============================================================================

/// Execute the sample_data tool
///
/// # Arguments
/// * `state` - Application state with injected dependencies
/// * `args` - Input arguments containing stream_id and optional n
///
/// # Returns
/// MCP ToolResponse as JSON Value
///
/// # Behavior
/// - Returns up to n rows (default 10, max 100)
/// - Rows ordered by timestamp descending (most recent first)
/// - If n > available rows, returns all available with a note
/// - If n > MAX_N, clamps to MAX_N with a note
pub async fn execute(state: &AppState, args: Value) -> Result<Value, McpRpcError> {
    // Parse input
    let input: SampleDataInput = serde_json::from_value(args).map_err(|e| {
        McpRpcError::new(
            JsonRpcError::INVALID_PARAMS,
            format!("Invalid input: {}", e),
        )
    })?;

    // Validate stream exists in config (fail fast for invalid stream_id)
    match state.config.get_stream_config(&input.stream_id).await {
        Ok(_) => {} // Stream exists, continue
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

    // Clamp n to MAX_N
    let (requested_n, actual_n, clamped) = if input.n > MAX_N {
        (input.n, MAX_N, true)
    } else {
        (input.n, input.n, false)
    };

    // Get sample rows from storage
    let rows = match state.storage.sample(&input.stream_id, actual_n).await {
        Ok(rows) => rows,
        Err(StorageError::StreamNotFound(_)) | Err(StorageError::NoDataAvailable(_)) => {
            // Stream exists in config but no data - return empty result
            return create_tool_response(SampleDataOutput {
                stream_id: input.stream_id,
                row_count: 0,
                rows: vec![],
                source_file: None,
                note: Some("No data available for this stream".to_string()),
            });
        }
        Err(e) => {
            return create_error_response(
                error_codes::INTERNAL_ERROR,
                &format!("Storage error: {}", e),
                None,
            );
        }
    };

    // Get source file path
    let source_file = state
        .storage
        .get_latest_file_path(&input.stream_id)
        .await
        .ok()
        .flatten();

    // Build response with appropriate notes
    let row_count = rows.len();
    let note = if clamped && row_count == MAX_N {
        Some(format!(
            "Requested {} rows but maximum is {}. Returning {}.",
            requested_n, MAX_N, MAX_N
        ))
    } else if row_count < actual_n && row_count > 0 {
        Some(format!(
            "Requested {} rows but only {} available",
            actual_n, row_count
        ))
    } else {
        None
    };

    let output = SampleDataOutput {
        stream_id: input.stream_id,
        row_count,
        rows,
        source_file,
        note,
    };

    create_tool_response(output)
}

// =============================================================================
// Tests - London School TDD (Mock-Driven, Behavior Verification)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::traits::{MockBronzeStorage, MockConfigStore, StreamConfigInfo};
    use crate::mcp::ToolResponse;
    use std::sync::Arc;

    fn create_test_state(
        mock_storage: MockBronzeStorage,
        mock_config: MockConfigStore,
    ) -> AppState {
        AppState::new(Arc::new(mock_storage), Arc::new(mock_config))
    }

    fn sample_config_info() -> StreamConfigInfo {
        StreamConfigInfo {
            stream_id: "air-quality".to_string(),
            description: "Test stream".to_string(),
            enabled: true,
            version: "1.0.0".to_string(),
            sources: vec!["mqtt".to_string()],
        }
    }

    fn sample_rows(count: usize) -> Vec<BronzeRow> {
        (0..count)
            .map(|i| BronzeRow {
                timestamp: 1767452639760716 - (i as i64 * 60_000_000), // 1 minute apart
                source_id: "air-quality-Mqtt".to_string(),
                ndp_id: Some(format!("sensor-{:03}", i)),
                context: Some(json!({"location": {"room": "office"}})),
                raw_payload: json!({"pm25": 12.5 + i as f64, "co2": 800 + i as i64}),
            })
            .collect()
    }

    // -------------------------------------------------------------------------
    // TC-SD-030: Returns N most recent rows
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_returns_n_rows() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|_| Ok(sample_config_info()));

        mock_storage
            .expect_sample()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(5),
            )
            .returning(|_, n| Ok(sample_rows(n)));

        mock_storage.expect_get_latest_file_path().returning(|_| {
            Ok(Some(
                "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet".to_string(),
            ))
        });

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "air-quality",
                "n": 5
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert_eq!(inner["stream_id"], "air-quality");
        assert_eq!(inner["row_count"], 5);

        let rows = inner["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 5);

        // Verify Bronze envelope structure
        let first_row = &rows[0];
        assert!(first_row["timestamp"].is_i64());
        assert_eq!(first_row["source_id"], "air-quality-Mqtt");
        assert!(first_row["raw_payload"].is_object());

        // Verify source_file
        assert!(inner["source_file"]
            .as_str()
            .unwrap()
            .contains("air-quality"));
    }

    // -------------------------------------------------------------------------
    // TC-SD-031: Handles n > available rows
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_n_greater_than_available() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|_| Ok(sample_config_info()));

        // Storage only has 3 rows
        mock_storage
            .expect_sample()
            .returning(|_, _| Ok(sample_rows(3)));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(Some("/data/raw/sparse-stream/data.parquet".to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "sparse-stream",
                "n": 100
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["row_count"], 3);
        assert!(inner["note"]
            .as_str()
            .unwrap()
            .contains("Requested 100 rows but only 3 available"));
    }

    // -------------------------------------------------------------------------
    // TC-SD-032: Returns proper Bronze envelope structure (tested in TC-SD-030)
    // -------------------------------------------------------------------------

    // -------------------------------------------------------------------------
    // TC-SD-033: Handles stream with no data
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_no_data_available() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|_| Ok(sample_config_info()));

        // No data available
        mock_storage
            .expect_sample()
            .returning(|id, _| Err(StorageError::NoDataAvailable(id.to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "empty-stream",
                "n": 10
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        assert_eq!(inner["row_count"], 0);
        assert_eq!(inner["rows"].as_array().unwrap().len(), 0);
        assert!(inner["source_file"].is_null());
        assert!(inner["note"]
            .as_str()
            .unwrap()
            .contains("No data available"));
    }

    // -------------------------------------------------------------------------
    // TC-SD-034: Default n value is 10
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_default_n() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|_| Ok(sample_config_info()));

        // Verify sample is called with default n=10
        mock_storage
            .expect_sample()
            .with(mockall::predicate::always(), mockall::predicate::eq(10))
            .returning(|_, n| Ok(sample_rows(n)));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(None));

        let state = create_test_state(mock_storage, mock_config);

        // Act - no n specified
        let result = execute(
            &state,
            json!({
                "stream_id": "air-quality"
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["row_count"], 10);
    }

    // -------------------------------------------------------------------------
    // TC-SD-035: Maximum n is 100
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_max_n_clamped() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|_| Ok(sample_config_info()));

        // Should be called with MAX_N (100), not 1000
        mock_storage
            .expect_sample()
            .with(mockall::predicate::always(), mockall::predicate::eq(100))
            .returning(|_, n| Ok(sample_rows(n)));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(None));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "air-quality",
                "n": 1000
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["row_count"], 100);
        assert!(inner["note"]
            .as_str()
            .unwrap()
            .contains("Requested 1000 rows but maximum is 100"));
    }

    // -------------------------------------------------------------------------
    // Stream not found error
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_stream_not_found() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|id| Err(ConfigError::StreamNotFound(id.to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "nonexistent",
                "n": 10
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
        assert_eq!(def.name, "sample_data");
        assert!(def.description.contains("sample rows"));
        assert!(def.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("stream_id")));
        assert_eq!(def.input_schema["properties"]["n"]["default"], 10);
        assert_eq!(def.input_schema["properties"]["n"]["maximum"], 100);
    }

    // -------------------------------------------------------------------------
    // Verify rows are ordered by timestamp descending
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_sample_data_rows_ordered_descending() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config
            .expect_get_stream_config()
            .returning(|_| Ok(sample_config_info()));

        mock_storage
            .expect_sample()
            .returning(|_, n| Ok(sample_rows(n)));

        mock_storage
            .expect_get_latest_file_path()
            .returning(|_| Ok(None));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(
            &state,
            json!({
                "stream_id": "air-quality",
                "n": 3
            }),
        )
        .await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        let rows = inner["rows"].as_array().unwrap();
        let ts0 = rows[0]["timestamp"].as_i64().unwrap();
        let ts1 = rows[1]["timestamp"].as_i64().unwrap();
        let ts2 = rows[2]["timestamp"].as_i64().unwrap();

        // Most recent first (descending)
        assert!(ts0 > ts1);
        assert!(ts1 > ts2);
    }
}
