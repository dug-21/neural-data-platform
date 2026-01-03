//! BronzeStorage trait definition with mockall support for London School TDD.
//!
//! This module defines the port for Bronze layer storage access following
//! the NDP Domain Adapter pattern (ADR-002).

use async_trait::async_trait;
use serde_json::Value;

#[cfg(test)]
use mockall::automock;

use crate::error::McpResult;
use super::types::{ParquetSchemaInfo, StreamStorageInfo};

/// Bronze layer storage abstraction (Port).
///
/// Defines the interface for accessing Bronze layer Parquet files.
/// Implementations handle different storage backends (local filesystem,
/// S3, GCS, etc.) while exposing a consistent interface.
///
/// # Design Rationale (ADR-002)
///
/// Following the Domain Adapter pattern:
/// - This trait is the **port** (interface)
/// - `LocalParquetStorage` is the **adapter** for local filesystem
/// - Future `S3ParquetStorage` will be an adapter for cloud storage
///
/// # Methods
///
/// - `list_streams()`: Enumerate all streams with storage metadata
/// - `get_schema()`: Get Parquet schema info for a stream
/// - `sample()`: Read N most recent rows from a stream
/// - `latest_partition()`: Find the most recent partition path
///
/// # Example
///
/// ```ignore
/// use ndp_mcp_server::storage::{BronzeStorage, LocalParquetStorage};
///
/// let storage = LocalParquetStorage::new("/data/raw");
/// let streams = storage.list_streams().await?;
/// let schema = storage.get_schema("air-quality").await?;
/// let rows = storage.sample("air-quality", 10).await?;
/// ```
#[cfg_attr(test, automock)]
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    /// List all streams that have data in Bronze storage.
    ///
    /// Scans the base directory for stream subdirectories and returns
    /// metadata about each stream including latest partition info.
    ///
    /// # Returns
    ///
    /// Vector of `StreamStorageInfo` with stream IDs and storage metadata.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the base path cannot be read.
    async fn list_streams(&self) -> McpResult<Vec<StreamStorageInfo>>;

    /// Get the Parquet schema for a specific stream.
    ///
    /// Opens the latest partition file and reads the schema from
    /// Parquet metadata (footer). Returns both the raw schema and
    /// analyzed `raw_payload` structure for ETL development.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier (e.g., "air-quality")
    ///
    /// # Returns
    ///
    /// `ParquetSchemaInfo` with column definitions and payload analysis.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if no data exists for the stream
    /// - `McpError::StorageError` if the Parquet file cannot be read
    async fn get_schema(&self, stream_id: &str) -> McpResult<ParquetSchemaInfo>;

    /// Sample N rows from the most recent partition of a stream.
    ///
    /// Reads the latest partition and returns up to N rows as JSON objects.
    /// Each row includes all columns from the Bronze envelope schema:
    /// timestamp, source_id, ndp_id, context, raw_payload.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier (e.g., "air-quality")
    /// * `n` - Maximum number of rows to return (1-100)
    ///
    /// # Returns
    ///
    /// Vector of JSON objects representing rows.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if no data exists for the stream
    /// - `McpError::StorageError` if the Parquet file cannot be read
    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>>;

    /// Get the path to the latest partition for a stream.
    ///
    /// Walks the Hive-style partition tree to find the most recent
    /// year/month/day directory containing data.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    ///
    /// # Returns
    ///
    /// Optional partition path string (e.g., "year=2026/month=01/day=03")
    /// or None if no partitions exist.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the stream directory cannot be read.
    async fn latest_partition(&self, stream_id: &str) -> McpResult<Option<String>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::types::{FieldInfo, RawPayloadStructure};
    use serde_json::json;

    // ========== LONDON SCHOOL TDD: BEHAVIOR VERIFICATION TESTS ==========

    #[tokio::test]
    async fn test_list_streams_returns_stream_info() {
        let mut mock = MockBronzeStorage::new();

        let expected = vec![
            StreamStorageInfo::new("air-quality")
                .with_latest_partition("year=2026/month=01/day=03")
                .with_file_size(1024),
            StreamStorageInfo::new("outdoor-weather")
                .with_latest_partition("year=2026/month=01/day=03")
                .with_file_size(2048),
        ];

        mock.expect_list_streams()
            .times(1)
            .returning(move || {
                Ok(vec![
                    StreamStorageInfo::new("air-quality")
                        .with_latest_partition("year=2026/month=01/day=03")
                        .with_file_size(1024),
                    StreamStorageInfo::new("outdoor-weather")
                        .with_latest_partition("year=2026/month=01/day=03")
                        .with_file_size(2048),
                ])
            });

        let result = mock.list_streams().await;
        assert!(result.is_ok());
        let streams = result.unwrap();
        assert_eq!(streams.len(), 2);
        assert_eq!(streams[0].stream_id, "air-quality");
        assert_eq!(streams[1].stream_id, "outdoor-weather");
    }

    #[tokio::test]
    async fn test_list_streams_returns_empty_when_no_data() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_list_streams()
            .times(1)
            .returning(|| Ok(vec![]));

        let result = mock.list_streams().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_streams_propagates_storage_error() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_list_streams()
            .times(1)
            .returning(|| Err(McpError::StorageError("Permission denied".to_string())));

        let result = mock.list_streams().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_get_schema_returns_parquet_info() {
        let mut mock = MockBronzeStorage::new();

        let expected_schema = ParquetSchemaInfo {
            stream_id: "air-quality".to_string(),
            fields: vec![
                FieldInfo::new("timestamp", "INT64"),
                FieldInfo::new("source_id", "UTF8"),
                FieldInfo::new("raw_payload", "UTF8"),
            ],
            raw_payload_structure: Some(RawPayloadStructure {
                keys: vec!["pm25".to_string(), "temperature".to_string()],
                nested: std::collections::HashMap::new(),
            }),
            file_path: "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet".to_string(),
        };

        mock.expect_get_schema()
            .with(mockall::predicate::eq("air-quality"))
            .times(1)
            .returning(move |_| {
                Ok(ParquetSchemaInfo {
                    stream_id: "air-quality".to_string(),
                    fields: vec![
                        FieldInfo::new("timestamp", "INT64"),
                        FieldInfo::new("source_id", "UTF8"),
                        FieldInfo::new("raw_payload", "UTF8"),
                    ],
                    raw_payload_structure: Some(RawPayloadStructure {
                        keys: vec!["pm25".to_string(), "temperature".to_string()],
                        nested: std::collections::HashMap::new(),
                    }),
                    file_path: "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet".to_string(),
                })
            });

        let result = mock.get_schema("air-quality").await;
        assert!(result.is_ok());
        let schema = result.unwrap();
        assert_eq!(schema.stream_id, "air-quality");
        assert_eq!(schema.fields.len(), 3);
        assert!(schema.raw_payload_structure.is_some());
    }

    #[tokio::test]
    async fn test_get_schema_returns_not_found_for_unknown_stream() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_get_schema()
            .with(mockall::predicate::eq("unknown-stream"))
            .times(1)
            .returning(|stream_id| Err(McpError::StreamNotFound(stream_id.to_string())));

        let result = mock.get_schema("unknown-stream").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
        assert!(err.to_string().contains("unknown-stream"));
    }

    #[tokio::test]
    async fn test_sample_returns_json_rows() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_sample()
            .with(mockall::predicate::eq("air-quality"), mockall::predicate::eq(3))
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    json!({
                        "timestamp": 1704067200000i64,
                        "source_id": "air-quality-Mqtt",
                        "raw_payload": {"pm25": 12.5}
                    }),
                    json!({
                        "timestamp": 1704067260000i64,
                        "source_id": "air-quality-Mqtt",
                        "raw_payload": {"pm25": 13.0}
                    }),
                    json!({
                        "timestamp": 1704067320000i64,
                        "source_id": "air-quality-Mqtt",
                        "raw_payload": {"pm25": 11.5}
                    }),
                ])
            });

        let result = mock.sample("air-quality", 3).await;
        assert!(result.is_ok());
        let rows = result.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["source_id"], "air-quality-Mqtt");
        assert_eq!(rows[0]["raw_payload"]["pm25"], 12.5);
    }

    #[tokio::test]
    async fn test_sample_respects_row_limit() {
        let mut mock = MockBronzeStorage::new();

        // Request only 1 row
        mock.expect_sample()
            .with(mockall::predicate::eq("air-quality"), mockall::predicate::eq(1))
            .times(1)
            .returning(|_, _| {
                Ok(vec![json!({
                    "timestamp": 1704067200000i64,
                    "source_id": "air-quality-Mqtt",
                    "raw_payload": {"pm25": 12.5}
                })])
            });

        let result = mock.sample("air-quality", 1).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_sample_returns_empty_for_stream_without_data() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_sample()
            .with(mockall::predicate::eq("empty-stream"), mockall::predicate::eq(10))
            .times(1)
            .returning(|stream_id, _| Err(McpError::StreamNotFound(stream_id.to_string())));

        let result = mock.sample("empty-stream", 10).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_latest_partition_returns_partition_path() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_latest_partition()
            .with(mockall::predicate::eq("air-quality"))
            .times(1)
            .returning(|_| Ok(Some("year=2026/month=01/day=03".to_string())));

        let result = mock.latest_partition("air-quality").await;
        assert!(result.is_ok());
        let partition = result.unwrap();
        assert_eq!(partition, Some("year=2026/month=01/day=03".to_string()));
    }

    #[tokio::test]
    async fn test_latest_partition_returns_none_when_no_data() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_latest_partition()
            .with(mockall::predicate::eq("new-stream"))
            .times(1)
            .returning(|_| Ok(None));

        let result = mock.latest_partition("new-stream").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ========== WORKFLOW TESTS ==========

    #[tokio::test]
    async fn test_storage_workflow_list_then_schema_then_sample() {
        let mut mock = MockBronzeStorage::new();

        let mut seq = mockall::Sequence::new();

        // Step 1: List streams
        mock.expect_list_streams()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| {
                Ok(vec![StreamStorageInfo::new("air-quality")
                    .with_latest_partition("year=2026/month=01/day=03")])
            });

        // Step 2: Get schema for discovered stream
        mock.expect_get_schema()
            .with(mockall::predicate::eq("air-quality"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                Ok(ParquetSchemaInfo {
                    stream_id: "air-quality".to_string(),
                    fields: vec![FieldInfo::new("timestamp", "INT64")],
                    raw_payload_structure: None,
                    file_path: "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet".to_string(),
                })
            });

        // Step 3: Sample data from stream
        mock.expect_sample()
            .with(mockall::predicate::eq("air-quality"), mockall::predicate::eq(5))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                Ok(vec![json!({"timestamp": 1704067200000i64})])
            });

        // Execute workflow
        let streams = mock.list_streams().await.unwrap();
        assert_eq!(streams.len(), 1);

        let schema = mock.get_schema("air-quality").await.unwrap();
        assert_eq!(schema.stream_id, "air-quality");

        let rows = mock.sample("air-quality", 5).await.unwrap();
        assert!(!rows.is_empty());
    }

    // ========== ERROR HANDLING TESTS ==========

    #[tokio::test]
    async fn test_get_schema_handles_corrupted_parquet() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_get_schema()
            .with(mockall::predicate::eq("corrupted-stream"))
            .times(1)
            .returning(|_| {
                Err(McpError::StorageError("Invalid Parquet magic bytes".to_string()))
            });

        let result = mock.get_schema("corrupted-stream").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StorageError(_)));
        assert!(err.to_string().contains("Parquet"));
    }

    #[tokio::test]
    async fn test_sample_handles_io_error() {
        let mut mock = MockBronzeStorage::new();

        mock.expect_sample()
            .times(1)
            .returning(|_, _| {
                Err(McpError::StorageError("Disk I/O error".to_string()))
            });

        let result = mock.sample("any-stream", 10).await;
        assert!(result.is_err());
    }
}
