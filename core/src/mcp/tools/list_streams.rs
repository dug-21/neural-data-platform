//! list_streams MCP Tool (dp-005)
//!
//! Enumerates all Bronze layer streams with metadata from both
//! etcd configuration and Parquet storage.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "streams": [
//!     {
//!       "stream_id": "air-quality",
//!       "description": "AirGradient sensor readings",
//!       "enabled": true,
//!       "version": "1.0.0",
//!       "sources": ["mqtt"],
//!       "storage": {
//!         "latest_partition": "year=2026/month=01/day=03",
//!         "file_size_bytes": 7310,
//!         "file_modified": "2026-01-03T14:54:00Z"
//!       }
//!     }
//!   ]
//! }
//! ```

use crate::mcp::tools::{
    AppState,
    create_tool_response,
    create_error_response,
    error_codes,
    traits::ConfigError,
};
use crate::mcp::{McpRpcError, ToolDefinition};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// =============================================================================
// Input/Output Types
// =============================================================================

/// Input schema for list_streams (no parameters)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListStreamsInput {}

/// Storage metadata in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    pub latest_partition: String,
    pub file_size_bytes: u64,
    pub file_modified: DateTime<Utc>,
}

/// Stream entry in list_streams response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    pub stream_id: String,
    pub description: String,
    pub enabled: bool,
    pub version: String,
    pub sources: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageMetadata>,
}

/// list_streams response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListStreamsOutput {
    pub streams: Vec<StreamEntry>,
}

// =============================================================================
// Tool Definition
// =============================================================================

/// Get the MCP tool definition for list_streams
pub fn tool_definition() -> ToolDefinition {
    ToolDefinition::no_params(
        "list_streams",
        "List all available Bronze layer streams with metadata. Returns stream configuration from etcd and storage metadata from Parquet files.",
    )
}

// =============================================================================
// Tool Execution
// =============================================================================

/// Execute the list_streams tool
///
/// # Arguments
/// * `state` - Application state with injected dependencies
/// * `_args` - Input arguments (empty for this tool)
///
/// # Returns
/// MCP ToolResponse as JSON Value
///
/// # Behavior
/// 1. Gets all stream IDs from etcd config
/// 2. For each stream, gets config metadata
/// 3. Enriches with storage metadata from Bronze layer
/// 4. Returns combined list
pub async fn execute(state: &AppState, _args: Value) -> Result<Value, McpRpcError> {
    // 1. Get stream IDs from config store (fails fast if etcd unavailable)
    let stream_ids = match state.config.list_stream_ids().await {
        Ok(ids) => ids,
        Err(e) => match e {
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

    // 2. Get storage info (best effort - may not have data for all streams)
    let storage_info = state.storage.list().await.unwrap_or_default();

    // 3. Build response by combining config and storage
    let mut streams = Vec::new();

    for stream_id in &stream_ids {
        // Get config for this stream
        let config = match state.config.get_stream_config(stream_id).await {
            Ok(c) => c,
            Err(_) => continue, // Skip streams with config errors
        };

        // Find matching storage info
        let storage = storage_info.iter()
            .find(|s| s.stream_id == *stream_id)
            .and_then(|s| {
                // Only include storage if we have the required fields
                match (s.latest_partition.as_ref(), s.file_size_bytes, s.file_modified) {
                    (Some(partition), Some(size), Some(modified)) => {
                        Some(StorageMetadata {
                            latest_partition: partition.clone(),
                            file_size_bytes: size,
                            file_modified: modified,
                        })
                    }
                    _ => None,
                }
            });

        streams.push(StreamEntry {
            stream_id: config.stream_id,
            description: config.description,
            enabled: config.enabled,
            version: config.version,
            sources: config.sources,
            storage,
        });
    }

    // 4. Build successful response
    let output = ListStreamsOutput { streams };
    create_tool_response(output)
}

// =============================================================================
// Tests - London School TDD (Mock-Driven, Behavior Verification)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::traits::{
        MockBronzeStorage, MockConfigStore,
        StreamStorageInfo, StreamConfigInfo, StorageError,
    };
    use crate::mcp::ToolResponse;
    use mockall::predicate::*;
    use std::sync::Arc;

    /// Create test app state with mocked dependencies
    fn create_test_state(
        mock_storage: MockBronzeStorage,
        mock_config: MockConfigStore,
    ) -> AppState {
        AppState::new(
            Arc::new(mock_storage),
            Arc::new(mock_config),
        )
    }

    // -------------------------------------------------------------------------
    // TC-LS-001: Returns all configured streams
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_streams_returns_all_configured_streams() {
        // Arrange: Set up mocks with expectations
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        // Config store returns 3 stream IDs
        mock_config.expect_list_stream_ids()
            .times(1)
            .returning(|| Ok(vec![
                "air-quality".to_string(),
                "outdoor-weather".to_string(),
                "nws-forecast-hourly".to_string(),
            ]));

        // Config store returns details for each stream
        mock_config.expect_get_stream_config()
            .with(eq("air-quality"))
            .times(1)
            .returning(|_| Ok(StreamConfigInfo {
                stream_id: "air-quality".to_string(),
                description: "AirGradient sensor readings from MQTT".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["mqtt".to_string()],
            }));

        mock_config.expect_get_stream_config()
            .with(eq("outdoor-weather"))
            .times(1)
            .returning(|_| Ok(StreamConfigInfo {
                stream_id: "outdoor-weather".to_string(),
                description: "Outdoor weather data from OpenWeatherMap".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["http_poll".to_string()],
            }));

        mock_config.expect_get_stream_config()
            .with(eq("nws-forecast-hourly"))
            .times(1)
            .returning(|_| Ok(StreamConfigInfo {
                stream_id: "nws-forecast-hourly".to_string(),
                description: "NWS hourly forecast data".to_string(),
                enabled: false,
                version: "1.0.0".to_string(),
                sources: vec!["http_poll".to_string()],
            }));

        // Storage returns info for 2 streams (no data for nws-forecast-hourly)
        mock_storage.expect_list()
            .times(1)
            .returning(|| Ok(vec![
                StreamStorageInfo {
                    stream_id: "air-quality".to_string(),
                    latest_partition: Some("year=2026/month=01/day=03".to_string()),
                    file_size_bytes: Some(7310),
                    file_modified: Some(Utc::now()),
                    row_count: Some(100),
                },
                StreamStorageInfo {
                    stream_id: "outdoor-weather".to_string(),
                    latest_partition: Some("year=2026/month=01/day=03".to_string()),
                    file_size_bytes: Some(12480),
                    file_modified: Some(Utc::now()),
                    row_count: Some(50),
                },
            ]));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(&state, serde_json::json!({})).await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        assert!(!response.is_error());

        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert_eq!(inner["success"], true);

        let streams = inner["streams"].as_array().unwrap();
        assert_eq!(streams.len(), 3);

        // Verify air-quality stream
        let air_quality = &streams[0];
        assert_eq!(air_quality["stream_id"], "air-quality");
        assert_eq!(air_quality["enabled"], true);
        assert!(air_quality["storage"].is_object());
        assert_eq!(air_quality["storage"]["file_size_bytes"], 7310);

        // Verify disabled stream has null storage
        let nws = &streams[2];
        assert_eq!(nws["stream_id"], "nws-forecast-hourly");
        assert_eq!(nws["enabled"], false);
        assert!(nws["storage"].is_null());
    }

    // -------------------------------------------------------------------------
    // TC-LS-002: Handles empty Bronze directory
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_streams_handles_empty_storage() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config.expect_list_stream_ids()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_config.expect_get_stream_config()
            .returning(|_| Ok(StreamConfigInfo {
                stream_id: "air-quality".to_string(),
                description: "Test stream".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["mqtt".to_string()],
            }));

        // Empty storage - no files
        mock_storage.expect_list()
            .returning(|| Ok(vec![]));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(&state, serde_json::json!({})).await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        let streams = inner["streams"].as_array().unwrap();
        assert_eq!(streams.len(), 1);
        assert!(streams[0]["storage"].is_null());
    }

    // -------------------------------------------------------------------------
    // TC-LS-003: Handles etcd unavailable (fail fast)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_streams_handles_etcd_unavailable() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mock_storage = MockBronzeStorage::new();

        // etcd connection fails
        mock_config.expect_list_stream_ids()
            .times(1)
            .returning(|| Err(ConfigError::ConnectionFailed("connection refused".to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(&state, serde_json::json!({})).await;

        // Assert - returns error response (ToolResponse with isError=true)
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        assert!(response.is_error());

        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert_eq!(inner["success"], false);
        assert_eq!(inner["code"], "ETCD_UNAVAILABLE");
        assert!(inner["error"].as_str().unwrap().contains("connection refused"));
    }

    // -------------------------------------------------------------------------
    // TC-LS-004: Storage metadata accuracy (storage info populated)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_streams_includes_storage_metadata() {
        // Arrange
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        let expected_modified = Utc::now();

        mock_config.expect_list_stream_ids()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_config.expect_get_stream_config()
            .returning(|_| Ok(StreamConfigInfo {
                stream_id: "air-quality".to_string(),
                description: "Test".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec!["mqtt".to_string()],
            }));

        let modified_clone = expected_modified;
        mock_storage.expect_list()
            .returning(move || Ok(vec![
                StreamStorageInfo {
                    stream_id: "air-quality".to_string(),
                    latest_partition: Some("year=2026/month=01/day=03".to_string()),
                    file_size_bytes: Some(7310),
                    file_modified: Some(modified_clone),
                    row_count: Some(100),
                },
            ]));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(&state, serde_json::json!({})).await;

        // Assert
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        let storage = &inner["streams"][0]["storage"];
        assert_eq!(storage["latest_partition"], "year=2026/month=01/day=03");
        assert_eq!(storage["file_size_bytes"], 7310);
        // file_modified is present
        assert!(storage["file_modified"].is_string());
    }

    // -------------------------------------------------------------------------
    // Additional behavior tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_streams_storage_failure_gracefully_handled() {
        // Arrange: Storage fails but config works
        let mut mock_config = MockConfigStore::new();
        let mut mock_storage = MockBronzeStorage::new();

        mock_config.expect_list_stream_ids()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_config.expect_get_stream_config()
            .returning(|_| Ok(StreamConfigInfo {
                stream_id: "air-quality".to_string(),
                description: "Test".to_string(),
                enabled: true,
                version: "1.0.0".to_string(),
                sources: vec![],
            }));

        // Storage fails
        mock_storage.expect_list()
            .returning(|| Err(StorageError::Unavailable("disk error".to_string())));

        let state = create_test_state(mock_storage, mock_config);

        // Act
        let result = execute(&state, serde_json::json!({})).await;

        // Assert: Still succeeds but without storage info
        assert!(result.is_ok());
        let response: ToolResponse = serde_json::from_value(result.unwrap()).unwrap();
        let inner: Value = serde_json::from_str(&response.content[0].text).unwrap();

        assert_eq!(inner["success"], true);
        let streams = inner["streams"].as_array().unwrap();
        assert_eq!(streams.len(), 1);
        // Storage is null due to error
        assert!(streams[0]["storage"].is_null());
    }

    #[tokio::test]
    async fn test_tool_definition_is_correct() {
        let def = tool_definition();
        assert_eq!(def.name, "list_streams");
        assert!(def.description.contains("Bronze layer streams"));
        assert_eq!(def.input_schema["properties"], serde_json::json!({}));
    }
}
