//! list_streams Tool Implementation
//!
//! Lists all available Bronze layer streams with metadata from etcd and
//! storage information from the filesystem.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "streams": [
//!     {
//!       "stream_id": "air-quality",
//!       "description": "AirGradient sensor readings from MQTT",
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

use serde::{Deserialize, Serialize};

use crate::error::McpResult;
use crate::etcd::ConfigStore;
use crate::mcp::protocol::McpToolResult;
use crate::storage::BronzeStorage;

/// Stream metadata with storage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Stream identifier
    pub stream_id: String,

    /// Human-readable description
    pub description: String,

    /// Whether stream is enabled
    pub enabled: bool,

    /// Configuration version
    pub version: String,

    /// Source types (mqtt, http_poll, etc.)
    pub sources: Vec<String>,

    /// Storage information (null if no data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageInfo>,
}

/// Storage information for a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    /// Most recent partition path
    pub latest_partition: String,

    /// Size of data.parquet file in bytes
    pub file_size_bytes: u64,

    /// File modification timestamp (ISO 8601)
    pub file_modified: String,
}

/// Response structure for list_streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListStreamsResponse {
    /// Success flag
    pub success: bool,

    /// List of streams
    pub streams: Vec<StreamInfo>,
}

/// Execute the list_streams tool.
///
/// # Arguments
///
/// * `storage` - Bronze storage for filesystem info
/// * `config_store` - etcd config store for stream metadata
///
/// # Returns
///
/// MCP tool result with stream listing
pub async fn execute<S, C>(storage: &S, config_store: &C) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
    C: ConfigStore + ?Sized,
{
    // Get enabled streams from config store (returns full StreamConfig)
    let enabled_configs = config_store.get_enabled_streams().await?;

    // Get storage info for all streams
    let storage_infos = storage.list_streams().await?;

    // Build a map of stream_id -> storage info for efficient lookup
    let storage_map: std::collections::HashMap<String, _> = storage_infos
        .into_iter()
        .map(|s| (s.stream_id.clone(), s))
        .collect();

    let mut streams = Vec::with_capacity(enabled_configs.len());

    for config in enabled_configs {
        // Get storage info from the map
        let storage_info = storage_map.get(&config.stream_id);

        // Build storage info if partition exists
        let storage = storage_info.and_then(|s| {
            // Only include storage info if we have partition data
            s.latest_partition.as_ref().map(|partition| StorageInfo {
                latest_partition: partition.clone(),
                file_size_bytes: s.file_size_bytes.unwrap_or(0),
                file_modified: s
                    .file_modified
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            })
        });

        streams.push(StreamInfo {
            stream_id: config.stream_id.clone(),
            description: config.entity_schema.name.clone(),
            enabled: config.enabled,
            version: config.entity_schema.version.clone(),
            sources: vec![config.source_type.clone()],
            storage,
        });
    }

    let response = ListStreamsResponse {
        success: true,
        streams,
    };

    McpToolResult::success(&response)
        .map_err(|e| crate::error::McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etcd::{EntitySchema, MockConfigStore, StreamConfig};
    use crate::storage::{MockBronzeStorage, StreamStorageInfo};

    #[tokio::test]
    async fn test_list_streams_empty() {
        let mut storage = MockBronzeStorage::new();
        storage.expect_list_streams().returning(|| Ok(vec![]));

        let mut config_store = MockConfigStore::new();
        config_store
            .expect_get_enabled_streams()
            .returning(|| Ok(vec![]));

        let result = execute(&storage, &config_store).await.unwrap();
        let text = &result.content[0].text;
        let response: ListStreamsResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.streams.is_empty());
    }

    #[tokio::test]
    async fn test_list_streams_with_data() {
        let mut config_store = MockConfigStore::new();
        config_store.expect_get_enabled_streams().returning(|| {
            Ok(vec![StreamConfig {
                stream_id: "test-stream".to_string(),
                enabled: true,
                source_type: "mqtt".to_string(),
                field_mappings: vec![],
                entity_schema: EntitySchema {
                    name: "Test stream".to_string(),
                    version: "1.0.0".to_string(),
                    attributes: vec![],
                },
                raw_config: std::collections::HashMap::new(),
            }])
        });

        let mut storage = MockBronzeStorage::new();
        storage.expect_list_streams().returning(|| {
            Ok(vec![StreamStorageInfo::new("test-stream")
                .with_latest_partition("year=2026/month=01/day=03")
                .with_file_size(1234)])
        });

        let result = execute(&storage, &config_store).await.unwrap();
        let text = &result.content[0].text;
        let response: ListStreamsResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.streams.len(), 1);
        assert_eq!(response.streams[0].stream_id, "test-stream");
        assert!(response.streams[0].storage.is_some());
    }
}
