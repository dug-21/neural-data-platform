//! list_silver_tables Tool Implementation
//!
//! Lists all Silver hypertables with metadata from TimescaleDB.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "tables": [
//!     {
//!       "table_name": "air_quality_observations",
//!       "description": "Air quality sensor readings",
//!       "grain": "per_reading",
//!       "source_streams": ["air-quality"],
//!       "is_hypertable": true,
//!       "chunk_interval": "1 day",
//!       "row_count": 142857,
//!       "total_bytes": 52428800
//!     }
//!   ]
//! }
//! ```
//!
//! # Error Response
//!
//! ```json
//! {
//!   "success": false,
//!   "error": "Database connection failed",
//!   "code": "STORAGE_ERROR"
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::McpResult;
use crate::mcp::protocol::McpToolResult;
use crate::storage::SilverStorage;

/// Response structure for list_silver_tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSilverTablesResponse {
    /// Success flag
    pub success: bool,

    /// List of Silver tables
    pub tables: Vec<SilverTableEntry>,
}

/// Entry for a single Silver table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverTableEntry {
    /// Table name in Silver layer
    pub table_name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Data grain (e.g., "per_reading", "hourly")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grain: Option<String>,

    /// Bronze streams that feed this table
    pub source_streams: Vec<String>,

    /// Whether this is a TimescaleDB hypertable
    pub is_hypertable: bool,

    /// Chunk interval for hypertables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_interval: Option<String>,

    /// Total row count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<i64>,

    /// Total bytes used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<i64>,
}

/// Execute the list_silver_tables tool.
///
/// # Arguments
///
/// * `storage` - Silver storage implementation for TimescaleDB access
///
/// # Returns
///
/// MCP tool result with list of Silver tables
pub async fn execute<S>(storage: &S) -> McpResult<McpToolResult>
where
    S: SilverStorage + ?Sized,
{
    // Get all Silver tables from storage
    let tables = storage.list_tables().await?;

    // Convert to response format
    let entries: Vec<SilverTableEntry> = tables
        .into_iter()
        .map(|t| SilverTableEntry {
            table_name: t.table_name,
            description: t.description,
            grain: t.grain,
            source_streams: t.source_streams,
            is_hypertable: t.is_hypertable,
            chunk_interval: t.chunk_interval,
            row_count: t.row_count,
            total_bytes: t.total_bytes,
        })
        .collect();

    let response = ListSilverTablesResponse {
        success: true,
        tables: entries,
    };

    McpToolResult::success(&response)
        .map_err(|e| crate::error::McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::{MockSilverStorage, SilverTableInfo};

    #[tokio::test]
    async fn test_list_silver_tables_success() {
        let mut storage = MockSilverStorage::new();

        storage.expect_list_tables().returning(|| {
            Ok(vec![
                SilverTableInfo::new("air_quality_readings")
                    .with_description("Air quality sensor readings")
                    .with_grain("per_reading")
                    .with_source_streams(vec!["air-quality".to_string()])
                    .with_hypertable(true, Some("1 day".to_string()))
                    .with_row_count(50000)
                    .with_total_bytes(10 * 1024 * 1024),
                SilverTableInfo::new("outdoor_weather_readings")
                    .with_description("Weather observations")
                    .with_hypertable(true, Some("1 day".to_string()))
                    .with_row_count(30000),
            ])
        });

        let result = execute(&storage).await.unwrap();
        let text = &result.content[0].text;
        let response: ListSilverTablesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.tables.len(), 2);
        assert_eq!(response.tables[0].table_name, "air_quality_readings");
        assert_eq!(
            response.tables[0].description,
            Some("Air quality sensor readings".to_string())
        );
        assert!(response.tables[0].is_hypertable);
        assert_eq!(response.tables[0].row_count, Some(50000));
    }

    #[tokio::test]
    async fn test_list_silver_tables_empty() {
        let mut storage = MockSilverStorage::new();

        storage.expect_list_tables().returning(|| Ok(vec![]));

        let result = execute(&storage).await.unwrap();
        let text = &result.content[0].text;
        let response: ListSilverTablesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.tables.is_empty());
    }

    #[tokio::test]
    async fn test_list_silver_tables_storage_error() {
        let mut storage = MockSilverStorage::new();

        storage.expect_list_tables().returning(|| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let result = execute(&storage).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StorageError(_)));
        assert!(err.to_string().contains("Database connection failed"));
    }

    #[tokio::test]
    async fn test_list_silver_tables_preserves_all_fields() {
        let mut storage = MockSilverStorage::new();

        storage.expect_list_tables().returning(|| {
            Ok(vec![SilverTableInfo::new("test_table")
                .with_description("Test description")
                .with_grain("hourly")
                .with_source_streams(vec!["stream-a".to_string(), "stream-b".to_string()])
                .with_hypertable(true, Some("1 hour".to_string()))
                .with_row_count(1000)
                .with_total_bytes(2048)])
        });

        let result = execute(&storage).await.unwrap();
        let text = &result.content[0].text;
        let response: ListSilverTablesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.tables.len(), 1);
        let table = &response.tables[0];
        assert_eq!(table.table_name, "test_table");
        assert_eq!(table.description, Some("Test description".to_string()));
        assert_eq!(table.grain, Some("hourly".to_string()));
        assert_eq!(table.source_streams.len(), 2);
        assert!(table.source_streams.contains(&"stream-a".to_string()));
        assert!(table.source_streams.contains(&"stream-b".to_string()));
        assert!(table.is_hypertable);
        assert_eq!(table.chunk_interval, Some("1 hour".to_string()));
        assert_eq!(table.row_count, Some(1000));
        assert_eq!(table.total_bytes, Some(2048));
    }
}
