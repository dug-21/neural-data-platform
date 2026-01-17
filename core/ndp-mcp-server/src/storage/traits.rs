//! Storage trait definitions with mockall support for London School TDD.
//!
//! This module defines the ports for Bronze, Silver, Dictionary, and ETL storage
//! access following the NDP Domain Adapter pattern (ADR-002).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[cfg(test)]
use mockall::automock;

use super::types::{
    ColumnDescription, DictionaryEntry, DqRuleInfo, EtlHistoryResult, EtlStreamStatus,
    FreshnessReport, LineageTrace, ParquetSchemaInfo, SampleFilters, SilverTableDescription,
    SilverTableInfo, SilverTableStats, StreamStorageInfo,
};
use crate::error::McpResult;

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

        mock.expect_list_streams().times(1).returning(move || {
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

        mock.expect_list_streams().times(1).returning(|| Ok(vec![]));

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
                    file_path: "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet"
                        .to_string(),
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
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(3),
            )
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
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(1),
            )
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
            .with(
                mockall::predicate::eq("empty-stream"),
                mockall::predicate::eq(10),
            )
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
                    file_path: "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet"
                        .to_string(),
                })
            });

        // Step 3: Sample data from stream
        mock.expect_sample()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(5),
            )
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| Ok(vec![json!({"timestamp": 1704067200000i64})]));

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
                Err(McpError::StorageError(
                    "Invalid Parquet magic bytes".to_string(),
                ))
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
            .returning(|_, _| Err(McpError::StorageError("Disk I/O error".to_string())));

        let result = mock.sample("any-stream", 10).await;
        assert!(result.is_err());
    }
}

// ============================================================================
// Silver Layer Storage Trait (dp-010)
// ============================================================================

/// Silver layer TimescaleDB storage abstraction (Port).
///
/// Defines the interface for accessing Silver layer data stored in TimescaleDB.
/// Implementations handle connection management, query execution, and result
/// mapping.
///
/// # Design Rationale (ADR-002)
///
/// Following the Domain Adapter pattern:
/// - This trait is the **port** (interface)
/// - `TimescaleStorage` will be the **adapter** for TimescaleDB
/// - Future adapters could support other time-series databases
///
/// # Methods
///
/// - `list_tables()`: Enumerate all Silver hypertables with metadata
/// - `describe_table()`: Get detailed schema for a specific table
/// - `sample()`: Read N rows with optional time filtering
/// - `get_stats()`: Get table statistics including DQ summary
///
/// # Example
///
/// ```ignore
/// use ndp_mcp_server::storage::SilverStorage;
///
/// let storage = TimescaleStorage::new("postgres://...");
/// let tables = storage.list_tables().await?;
/// let schema = storage.describe_table("air_quality_readings").await?;
/// let rows = storage.sample("air_quality_readings", 10, None).await?;
/// ```
#[cfg_attr(test, automock)]
#[async_trait]
pub trait SilverStorage: Send + Sync {
    /// List all Silver hypertables with metadata.
    ///
    /// Queries TimescaleDB metadata to enumerate all hypertables in the
    /// Silver schema. Returns information about each table including
    /// chunk configuration and row counts.
    ///
    /// # Returns
    ///
    /// Vector of `SilverTableInfo` with table names and metadata.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the database query fails.
    async fn list_tables(&self) -> McpResult<Vec<SilverTableInfo>>;

    /// Get detailed schema for a Silver table.
    ///
    /// Returns column definitions including data types, units, and
    /// descriptions from the data dictionary. Also includes hypertable
    /// metadata if applicable.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name (e.g., "air_quality_readings")
    ///
    /// # Returns
    ///
    /// `SilverTableDescription` with columns and hypertable info.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if table doesn't exist
    /// - `McpError::StorageError` if the query fails
    async fn describe_table(&self, table_name: &str) -> McpResult<SilverTableDescription>;

    /// Sample N rows from a Silver table.
    ///
    /// Reads up to N rows from the specified table. Supports optional
    /// time-based filtering for targeted exploration.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name
    /// * `n` - Maximum number of rows to return (1-100)
    /// * `filters` - Optional time range and ordering filters
    ///
    /// # Returns
    ///
    /// Vector of JSON objects representing rows.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if table doesn't exist
    /// - `McpError::StorageError` if the query fails
    async fn sample(
        &self,
        table_name: &str,
        n: usize,
        filters: Option<SampleFilters>,
    ) -> McpResult<Vec<Value>>;

    /// Get statistics for a Silver table.
    ///
    /// Returns row counts, time ranges, chunk information, and
    /// data quality summary for the specified table.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name
    ///
    /// # Returns
    ///
    /// `SilverTableStats` with counts and ranges.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if table doesn't exist
    /// - `McpError::StorageError` if the query fails
    async fn get_stats(&self, table_name: &str) -> McpResult<SilverTableStats>;
}

// ============================================================================
// Dictionary Store Trait (dp-010)
// ============================================================================

/// Data dictionary abstraction for cross-layer metadata (Port).
///
/// Defines the interface for accessing the unified data dictionary that
/// spans Bronze and Silver layers. Enables column discovery, lineage
/// tracing, and DQ rule lookup.
///
/// # Design Rationale (ADR-002)
///
/// Following the Domain Adapter pattern:
/// - This trait is the **port** (interface)
/// - `DictionaryClient` will be the **adapter** combining etcd and TimescaleDB
///
/// # Methods
///
/// - `search()`: Find columns by name or description
/// - `describe_column()`: Get detailed column metadata
/// - `trace_lineage()`: Trace Silver column to Bronze source
/// - `list_dq_rules()`: List data quality rules
///
/// # Example
///
/// ```ignore
/// use ndp_mcp_server::storage::DictionaryStore;
///
/// let dict = DictionaryClient::new(etcd, timescale);
/// let results = dict.search("temperature", Some("silver")).await?;
/// let lineage = dict.trace_lineage("air_quality_readings", "pm25").await?;
/// ```
#[cfg_attr(test, automock)]
#[async_trait]
pub trait DictionaryStore: Send + Sync {
    /// Search for columns matching a query.
    ///
    /// Searches column names and descriptions across Bronze and Silver
    /// layers. Supports partial matching and optional layer filtering.
    ///
    /// # Arguments
    ///
    /// * `query` - Search term (partial match on name or description)
    /// * `layer` - Optional filter: "bronze", "silver", or None for both
    ///
    /// # Returns
    ///
    /// Vector of matching `DictionaryEntry` records.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the query fails.
    async fn search(&self, query: &str, layer: Option<String>) -> McpResult<Vec<DictionaryEntry>>;

    /// Get detailed information about a specific column.
    ///
    /// Returns comprehensive column metadata including source information
    /// (for Silver), data quality rules, and validation ranges.
    ///
    /// # Arguments
    ///
    /// * `table_or_stream` - Table name (Silver) or stream ID (Bronze)
    /// * `column_name` - The column name
    ///
    /// # Returns
    ///
    /// `ColumnDescription` with full metadata.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if table/stream doesn't exist
    /// - `McpError::InvalidRequest` if column doesn't exist
    async fn describe_column(
        &self,
        table_or_stream: &str,
        column_name: &str,
    ) -> McpResult<ColumnDescription>;

    /// Trace lineage from Silver column back to Bronze source.
    ///
    /// Returns the complete lineage chain showing how a Silver column
    /// maps back to Bronze source fields, including any transformations.
    ///
    /// # Arguments
    ///
    /// * `silver_table` - The Silver table name
    /// * `silver_column` - The Silver column name
    ///
    /// # Returns
    ///
    /// `LineageTrace` with source chain and DQ rules.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if table doesn't exist
    /// - `McpError::InvalidRequest` if column doesn't exist
    async fn trace_lineage(
        &self,
        silver_table: &str,
        silver_column: &str,
    ) -> McpResult<LineageTrace>;

    /// List data quality rules with optional filters.
    ///
    /// Returns DQ rules defined for Silver tables. Supports filtering
    /// by table and/or column.
    ///
    /// # Arguments
    ///
    /// * `table` - Optional filter by table name
    /// * `column` - Optional filter by column name (requires table)
    ///
    /// # Returns
    ///
    /// Vector of `DqRuleInfo` matching the filters.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the query fails.
    async fn list_dq_rules(
        &self,
        table: Option<String>,
        column: Option<String>,
    ) -> McpResult<Vec<DqRuleInfo>>;
}

// ============================================================================
// ETL Run Store Trait (dp-010)
// ============================================================================

/// ETL run history storage abstraction (Port).
///
/// Defines the interface for accessing ETL run history and freshness
/// information. Enables monitoring of Bronze-to-Silver pipeline health.
///
/// # Design Rationale (ADR-002)
///
/// Following the Domain Adapter pattern:
/// - This trait is the **port** (interface)
/// - `EtlRunClient` will be the **adapter** for TimescaleDB etl_runs table
///
/// # Methods
///
/// - `get_status()`: Get current ETL status for streams
/// - `get_history()`: Get historical ETL runs with filtering
/// - `get_freshness()`: Check data freshness across layers
///
/// # Example
///
/// ```ignore
/// use ndp_mcp_server::storage::EtlRunStore;
///
/// let store = EtlRunClient::new(pool);
/// let status = store.get_status(Some("air-quality")).await?;
/// let history = store.get_history("air-quality", 50, None, None).await?;
/// let freshness = store.get_freshness(None).await?;
/// ```
#[cfg_attr(test, automock)]
#[async_trait]
pub trait EtlRunStore: Send + Sync {
    /// Get current ETL status for one or all streams.
    ///
    /// Returns the latest ETL run status and 24-hour statistics.
    /// Pass None for stream_id to get status for all streams.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - Optional filter by stream ID
    ///
    /// # Returns
    ///
    /// Vector of `EtlStreamStatus` for matching streams.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the query fails.
    async fn get_status(&self, stream_id: Option<String>) -> McpResult<Vec<EtlStreamStatus>>;

    /// Get historical ETL runs for a stream.
    ///
    /// Returns paginated ETL run history with optional filtering by
    /// time range and status.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream ID to query
    /// * `limit` - Maximum number of runs to return
    /// * `since` - Optional filter: only runs after this time
    /// * `status_filter` - Optional filter: "success", "failed", or None
    ///
    /// # Returns
    ///
    /// `EtlHistoryResult` with runs and summary.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if stream doesn't exist
    /// - `McpError::StorageError` if the query fails
    async fn get_history(
        &self,
        stream_id: &str,
        limit: usize,
        since: Option<DateTime<Utc>>,
        status_filter: Option<String>,
    ) -> McpResult<EtlHistoryResult>;

    /// Get data freshness report across layers.
    ///
    /// Checks data freshness for Bronze streams and Silver tables.
    /// Returns age information and staleness indicators.
    ///
    /// # Arguments
    ///
    /// * `layer` - Optional filter: "bronze", "silver", or None for both
    ///
    /// # Returns
    ///
    /// `FreshnessReport` with entries and summary.
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the query fails.
    async fn get_freshness(&self, layer: Option<String>) -> McpResult<FreshnessReport>;
}

// ============================================================================
// Silver Storage Tests (dp-010)
// ============================================================================

#[cfg(test)]
mod silver_storage_tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::types::{
        DqSummary, HypertableInfo, SilverColumnInfo, SilverTableDescription, SilverTableInfo,
        SilverTableStats,
    };
    use chrono::TimeZone;
    use serde_json::json;

    #[tokio::test]
    async fn test_list_tables_returns_table_info() {
        let mut mock = MockSilverStorage::new();

        mock.expect_list_tables().times(1).returning(|| {
            Ok(vec![
                SilverTableInfo::new("air_quality_readings")
                    .with_description("Air quality sensor readings")
                    .with_hypertable(true, Some("1 day".to_string()))
                    .with_row_count(50000),
                SilverTableInfo::new("outdoor_weather_readings")
                    .with_hypertable(true, Some("1 day".to_string()))
                    .with_row_count(30000),
            ])
        });

        let result = mock.list_tables().await;
        assert!(result.is_ok());
        let tables = result.unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].table_name, "air_quality_readings");
        assert!(tables[0].is_hypertable);
    }

    #[tokio::test]
    async fn test_list_tables_returns_empty_when_no_tables() {
        let mut mock = MockSilverStorage::new();

        mock.expect_list_tables().times(1).returning(|| Ok(vec![]));

        let result = mock.list_tables().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_list_tables_propagates_storage_error() {
        let mut mock = MockSilverStorage::new();

        mock.expect_list_tables().times(1).returning(|| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let result = mock.list_tables().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_describe_table_returns_schema() {
        let mut mock = MockSilverStorage::new();

        mock.expect_describe_table()
            .with(mockall::predicate::eq("air_quality_readings"))
            .times(1)
            .returning(|_| {
                Ok(SilverTableDescription::new("air_quality_readings")
                    .with_description("Air quality sensor readings")
                    .with_columns(vec![
                        SilverColumnInfo::new("timestamp", "TIMESTAMPTZ")
                            .with_nullable(false)
                            .with_primary_key(true),
                        SilverColumnInfo::new("pm25", "DOUBLE PRECISION")
                            .with_unit("ug/m3")
                            .with_description("PM2.5 particulate matter"),
                    ])
                    .with_hypertable_info(
                        HypertableInfo::new("timestamp", "1 day")
                            .with_chunk_count(30)
                            .with_total_bytes(100 * 1024 * 1024),
                    ))
            });

        let result = mock.describe_table("air_quality_readings").await;
        assert!(result.is_ok());
        let desc = result.unwrap();
        assert_eq!(desc.table_name, "air_quality_readings");
        assert_eq!(desc.columns.len(), 2);
        assert!(desc.hypertable_info.is_some());
    }

    #[tokio::test]
    async fn test_describe_table_returns_not_found() {
        let mut mock = MockSilverStorage::new();

        mock.expect_describe_table()
            .with(mockall::predicate::eq("nonexistent_table"))
            .times(1)
            .returning(|table| Err(McpError::StreamNotFound(table.to_string())));

        let result = mock.describe_table("nonexistent_table").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_sample_returns_json_rows() {
        let mut mock = MockSilverStorage::new();

        mock.expect_sample()
            .with(
                mockall::predicate::eq("air_quality_readings"),
                mockall::predicate::eq(5),
                mockall::predicate::always(),
            )
            .times(1)
            .returning(|_, _, _| {
                Ok(vec![
                    json!({
                        "timestamp": "2026-01-17T10:00:00Z",
                        "pm25": 12.5,
                        "dq_flags": null
                    }),
                    json!({
                        "timestamp": "2026-01-17T10:01:00Z",
                        "pm25": 13.0,
                        "dq_flags": null
                    }),
                ])
            });

        let result = mock.sample("air_quality_readings", 5, None).await;
        assert!(result.is_ok());
        let rows = result.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["pm25"], 12.5);
    }

    #[tokio::test]
    async fn test_sample_with_filters() {
        let mut mock = MockSilverStorage::new();

        let since = Utc.with_ymd_and_hms(2026, 1, 17, 0, 0, 0).unwrap();
        let filters = SampleFilters::new().with_since(since);

        mock.expect_sample()
            .withf(|table, n, f| table == "air_quality_readings" && *n == 10 && f.is_some())
            .times(1)
            .returning(|_, _, _| Ok(vec![json!({"timestamp": "2026-01-17T10:00:00Z"})]));

        let result = mock.sample("air_quality_readings", 10, Some(filters)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_stats_returns_statistics() {
        let mut mock = MockSilverStorage::new();

        mock.expect_get_stats()
            .with(mockall::predicate::eq("air_quality_readings"))
            .times(1)
            .returning(|_| {
                let min = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
                let max = Utc.with_ymd_and_hms(2026, 1, 17, 23, 59, 59).unwrap();
                Ok(SilverTableStats::new("air_quality_readings")
                    .with_row_count(50000)
                    .with_time_range(min, max)
                    .with_chunk_count(17)
                    .with_total_bytes(50 * 1024 * 1024)
                    .with_dq_summary(DqSummary::new(5, 3)))
            });

        let result = mock.get_stats("air_quality_readings").await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.row_count, 50000);
        assert_eq!(stats.chunk_count, 17);
        assert!(stats.dq_summary.is_some());
    }

    #[tokio::test]
    async fn test_silver_storage_workflow() {
        let mut mock = MockSilverStorage::new();
        let mut seq = mockall::Sequence::new();

        // Step 1: List tables
        mock.expect_list_tables()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| {
                Ok(vec![SilverTableInfo::new("air_quality_readings")
                    .with_hypertable(true, Some("1 day".to_string()))])
            });

        // Step 2: Describe the discovered table
        mock.expect_describe_table()
            .with(mockall::predicate::eq("air_quality_readings"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                Ok(SilverTableDescription::new("air_quality_readings")
                    .with_columns(vec![SilverColumnInfo::new("pm25", "DOUBLE PRECISION")]))
            });

        // Step 3: Sample data
        mock.expect_sample()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _, _| Ok(vec![json!({"pm25": 12.5})]));

        // Execute workflow
        let tables = mock.list_tables().await.unwrap();
        assert_eq!(tables.len(), 1);

        let desc = mock.describe_table("air_quality_readings").await.unwrap();
        assert!(!desc.columns.is_empty());

        let rows = mock.sample("air_quality_readings", 5, None).await.unwrap();
        assert!(!rows.is_empty());
    }
}

// ============================================================================
// Dictionary Store Tests (dp-010)
// ============================================================================

#[cfg(test)]
mod dictionary_store_tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::types::{
        ColumnDescription, DictionaryEntry, DqRuleInfo, LineageSource, LineageTrace, SourceInfo,
        ValidationRange,
    };
    use serde_json::json;

    #[tokio::test]
    async fn test_search_returns_matching_columns() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .with(
                mockall::predicate::eq("pm25"),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DictionaryEntry::new("bronze", "air-quality", "pm25", "number")
                        .with_unit("ug/m3"),
                    DictionaryEntry::new(
                        "silver",
                        "air_quality_readings",
                        "pm25",
                        "DOUBLE PRECISION",
                    )
                    .with_unit("ug/m3"),
                ])
            });

        let result = mock.search("pm25", None).await;
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].layer, "bronze");
        assert_eq!(entries[1].layer, "silver");
    }

    #[tokio::test]
    async fn test_search_with_layer_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .with(
                mockall::predicate::eq("temperature"),
                mockall::predicate::eq(Some("silver".to_string())),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![DictionaryEntry::new(
                    "silver",
                    "outdoor_weather_readings",
                    "temperature",
                    "DOUBLE PRECISION",
                )
                .with_unit("celsius")])
            });

        let result = mock.search("temperature", Some("silver".to_string())).await;
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].layer, "silver");
    }

    #[tokio::test]
    async fn test_search_returns_empty_for_no_matches() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search().times(1).returning(|_, _| Ok(vec![]));

        let result = mock.search("nonexistent", None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_describe_column_returns_full_metadata() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .with(
                mockall::predicate::eq("air_quality_readings"),
                mockall::predicate::eq("pm25"),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ColumnDescription::new(
                    "silver",
                    "air_quality_readings",
                    "pm25",
                    "DOUBLE PRECISION",
                )
                .with_unit("ug/m3")
                .with_description("PM2.5 particulate matter concentration")
                .with_nullable(true)
                .with_source(SourceInfo::new("air-quality", "$.pm25"))
                .with_dq_rules(vec![DqRuleInfo::new(
                    "air_quality_readings",
                    "range_check",
                    "flag",
                    "column",
                )
                .with_silver_column("pm25")
                .with_rule_params(json!({"min": 0, "max": 500}))])
                .with_validation_range(ValidationRange::bounded(0.0, 500.0)))
            });

        let result = mock.describe_column("air_quality_readings", "pm25").await;
        assert!(result.is_ok());
        let desc = result.unwrap();
        assert_eq!(desc.column_name, "pm25");
        assert!(desc.source.is_some());
        assert!(!desc.dq_rules.is_empty());
        assert!(desc.validation_range.is_some());
    }

    #[tokio::test]
    async fn test_describe_column_returns_not_found_for_missing_table() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .times(1)
            .returning(|table, _| Err(McpError::StreamNotFound(table.to_string())));

        let result = mock.describe_column("nonexistent_table", "col").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_describe_column_returns_invalid_request_for_missing_column() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column().times(1).returning(|_, col| {
            Err(McpError::InvalidRequest(format!(
                "Column not found: {}",
                col
            )))
        });

        let result = mock
            .describe_column("air_quality_readings", "nonexistent")
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn test_trace_lineage_returns_full_chain() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
            .with(
                mockall::predicate::eq("air_quality_readings"),
                mockall::predicate::eq("pm25"),
            )
            .times(1)
            .returning(|_, _| {
                Ok(
                    LineageTrace::new("air_quality_readings", "pm25", "DOUBLE PRECISION")
                        .with_silver_unit("ug/m3")
                        .with_lineage(vec![LineageSource::new("air-quality", "$.pm25")
                            .with_bronze_type("number")
                            .with_bronze_unit("ug/m3")
                            .with_transformation("cast to double")])
                        .with_dq_rules(vec![DqRuleInfo::new(
                            "air_quality_readings",
                            "range_check",
                            "flag",
                            "column",
                        )
                        .with_silver_column("pm25")]),
                )
            });

        let result = mock.trace_lineage("air_quality_readings", "pm25").await;
        assert!(result.is_ok());
        let trace = result.unwrap();
        assert_eq!(trace.silver_column, "pm25");
        assert_eq!(trace.lineage.len(), 1);
        assert_eq!(trace.lineage[0].source_stream, "air-quality");
    }

    #[tokio::test]
    async fn test_list_dq_rules_returns_all_rules() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(None::<String>),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DqRuleInfo::new("air_quality_readings", "range_check", "flag", "column")
                        .with_silver_column("pm25"),
                    DqRuleInfo::new("air_quality_readings", "not_null", "reject", "column")
                        .with_silver_column("timestamp"),
                    DqRuleInfo::new("outdoor_weather_readings", "range_check", "flag", "column")
                        .with_silver_column("temperature"),
                ])
            });

        let result = mock.list_dq_rules(None, None).await;
        assert!(result.is_ok());
        let rules = result.unwrap();
        assert_eq!(rules.len(), 3);
    }

    #[tokio::test]
    async fn test_list_dq_rules_with_table_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(Some("air_quality_readings".to_string())),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DqRuleInfo::new("air_quality_readings", "range_check", "flag", "column")
                        .with_silver_column("pm25"),
                    DqRuleInfo::new("air_quality_readings", "not_null", "reject", "column")
                        .with_silver_column("timestamp"),
                ])
            });

        let result = mock
            .list_dq_rules(Some("air_quality_readings".to_string()), None)
            .await;
        assert!(result.is_ok());
        let rules = result.unwrap();
        assert_eq!(rules.len(), 2);
        assert!(rules
            .iter()
            .all(|r| r.silver_table == "air_quality_readings"));
    }

    #[tokio::test]
    async fn test_list_dq_rules_with_column_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(Some("air_quality_readings".to_string())),
                mockall::predicate::eq(Some("pm25".to_string())),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![DqRuleInfo::new(
                    "air_quality_readings",
                    "range_check",
                    "flag",
                    "column",
                )
                .with_silver_column("pm25")])
            });

        let result = mock
            .list_dq_rules(
                Some("air_quality_readings".to_string()),
                Some("pm25".to_string()),
            )
            .await;
        assert!(result.is_ok());
        let rules = result.unwrap();
        assert_eq!(rules.len(), 1);
    }
}

// ============================================================================
// ETL Run Store Tests (dp-010)
// ============================================================================

#[cfg(test)]
mod etl_run_store_tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::types::{
        EtlHistoryResult, EtlRunDetail, EtlRunInfo, EtlStreamStatus, FreshnessEntry,
        FreshnessReport, FreshnessSummary, HistorySummary, RunStats,
    };
    use chrono::TimeZone;

    #[tokio::test]
    async fn test_get_status_returns_all_streams() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(None::<String>))
            .times(1)
            .returning(|_| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
                let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();
                Ok(vec![
                    EtlStreamStatus::new("air-quality", "healthy")
                        .with_last_run(
                            EtlRunInfo::new("run-001", started)
                                .with_completed_at(completed)
                                .with_row_counts(1000, 5, 2),
                        )
                        .with_runs_last_24h(RunStats::new(24, 23, 1)),
                    EtlStreamStatus::new("outdoor-weather", "healthy")
                        .with_runs_last_24h(RunStats::new(24, 24, 0)),
                ])
            });

        let result = mock.get_status(None).await;
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].stream_id, "air-quality");
        assert_eq!(statuses[0].status, "healthy");
    }

    #[tokio::test]
    async fn test_get_status_with_stream_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(Some("air-quality".to_string())))
            .times(1)
            .returning(|_| {
                Ok(vec![EtlStreamStatus::new("air-quality", "healthy")
                    .with_runs_last_24h(RunStats::new(24, 24, 0))])
            });

        let result = mock.get_status(Some("air-quality".to_string())).await;
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].stream_id, "air-quality");
    }

    #[tokio::test]
    async fn test_get_status_returns_empty_for_unknown_stream() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(Some("nonexistent".to_string())))
            .times(1)
            .returning(|_| Ok(vec![]));

        let result = mock.get_status(Some("nonexistent".to_string())).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_history_returns_paginated_runs() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(50),
                mockall::predicate::eq(None::<DateTime<Utc>>),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|stream_id, _, _, _| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
                let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![
                        EtlRunDetail::new("run-001", started, "success", "incremental")
                            .with_completed_at(completed)
                            .with_row_counts(1000, 5, 2),
                        EtlRunDetail::new(
                            "run-002",
                            started - chrono::Duration::hours(1),
                            "success",
                            "incremental",
                        )
                        .with_completed_at(completed - chrono::Duration::hours(1))
                        .with_row_counts(950, 3, 1),
                    ])
                    .with_summary(HistorySummary::new(2, 100)))
            });

        let result = mock.get_history("air-quality", 50, None, None).await;
        assert!(result.is_ok());
        let history = result.unwrap();
        assert_eq!(history.stream_id, "air-quality");
        assert_eq!(history.runs.len(), 2);
        assert_eq!(history.summary.total_returned, 2);
        assert_eq!(history.summary.total_available, 100);
    }

    #[tokio::test]
    async fn test_get_history_with_since_filter() {
        let mut mock = MockEtlRunStore::new();

        let since = Utc.with_ymd_and_hms(2026, 1, 17, 0, 0, 0).unwrap();

        mock.expect_get_history()
            .withf(move |stream, limit, since_opt, status| {
                stream == "air-quality" && *limit == 50 && since_opt.is_some() && status.is_none()
            })
            .times(1)
            .returning(|stream_id, _, _, _| {
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![])
                    .with_summary(HistorySummary::new(0, 0)))
            });

        let result = mock.get_history("air-quality", 50, Some(since), None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_history_with_status_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(50),
                mockall::predicate::eq(None::<DateTime<Utc>>),
                mockall::predicate::eq(Some("failed".to_string())),
            )
            .times(1)
            .returning(|stream_id, _, _, _| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 5, 0, 0).unwrap();
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![EtlRunDetail::new(
                        "run-003",
                        started,
                        "failed",
                        "incremental",
                    )
                    .with_error("Connection timeout", None)])
                    .with_summary(HistorySummary::new(1, 1)))
            });

        let result = mock
            .get_history("air-quality", 50, None, Some("failed".to_string()))
            .await;
        assert!(result.is_ok());
        let history = result.unwrap();
        assert_eq!(history.runs.len(), 1);
        assert_eq!(history.runs[0].status, "failed");
    }

    #[tokio::test]
    async fn test_get_history_returns_not_found_for_unknown_stream() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .times(1)
            .returning(|stream_id, _, _, _| Err(McpError::StreamNotFound(stream_id.to_string())));

        let result = mock.get_history("nonexistent", 50, None, None).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_get_freshness_returns_all_layers() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .with(mockall::predicate::eq(None::<String>))
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                let latest = now - chrono::Duration::minutes(5);
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![
                        FreshnessEntry::new("bronze", "air-quality", "fresh")
                            .with_latest_timestamp(latest, now)
                            .with_row_count(50000),
                        FreshnessEntry::new("silver", "air_quality_readings", "fresh")
                            .with_latest_timestamp(latest, now)
                            .with_row_count(50000)
                            .with_last_etl_run(now - chrono::Duration::minutes(1)),
                    ])
                    .with_summary(FreshnessSummary::new(1, 1, 0, 0)))
            });

        let result = mock.get_freshness(None).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.freshness.len(), 2);
        assert_eq!(report.summary.bronze_streams, 1);
        assert_eq!(report.summary.silver_tables, 1);
    }

    #[tokio::test]
    async fn test_get_freshness_with_layer_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .with(mockall::predicate::eq(Some("bronze".to_string())))
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![FreshnessEntry::new("bronze", "air-quality", "fresh")])
                    .with_summary(FreshnessSummary::new(1, 0, 0, 0)))
            });

        let result = mock.get_freshness(Some("bronze".to_string())).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.freshness.len(), 1);
        assert_eq!(report.freshness[0].layer, "bronze");
    }

    #[tokio::test]
    async fn test_get_freshness_detects_stale_data() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness().times(1).returning(|_| {
            let now = Utc::now();
            let stale_timestamp = now - chrono::Duration::hours(2);
            Ok(FreshnessReport::new(now)
                .with_freshness(vec![FreshnessEntry::new("bronze", "air-quality", "stale")
                    .with_latest_timestamp(stale_timestamp, now)])
                .with_summary(FreshnessSummary::new(1, 0, 1, 0)))
        });

        let result = mock.get_freshness(None).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.summary.stale_count, 1);
        assert_eq!(report.freshness[0].freshness_status, "stale");
    }

    #[tokio::test]
    async fn test_etl_store_workflow() {
        let mut mock = MockEtlRunStore::new();
        let mut seq = mockall::Sequence::new();

        // Step 1: Check overall status
        mock.expect_get_status()
            .with(mockall::predicate::eq(None::<String>))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                Ok(vec![EtlStreamStatus::new("air-quality", "warning")
                    .with_runs_last_24h(RunStats::new(24, 20, 4))])
            });

        // Step 2: Get history for problematic stream
        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(10),
                mockall::predicate::always(),
                mockall::predicate::eq(Some("failed".to_string())),
            )
            .times(1)
            .in_sequence(&mut seq)
            .returning(|stream_id, _, _, _| {
                let started = Utc::now();
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![EtlRunDetail::new(
                        "run-fail",
                        started,
                        "failed",
                        "incremental",
                    )
                    .with_error("Connection refused", None)])
                    .with_summary(HistorySummary::new(1, 4)))
            });

        // Step 3: Check freshness
        mock.expect_get_freshness()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                let now = Utc::now();
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![FreshnessEntry::new(
                        "silver",
                        "air_quality_readings",
                        "stale",
                    )])
                    .with_summary(FreshnessSummary::new(0, 1, 1, 0)))
            });

        // Execute workflow
        let statuses = mock.get_status(None).await.unwrap();
        assert_eq!(statuses[0].status, "warning");

        let history = mock
            .get_history("air-quality", 10, None, Some("failed".to_string()))
            .await
            .unwrap();
        assert!(!history.runs.is_empty());

        let freshness = mock.get_freshness(None).await.unwrap();
        assert_eq!(freshness.summary.stale_count, 1);
    }
}
