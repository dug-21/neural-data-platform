//! silver_stats Tool Implementation
//!
//! Get statistics for a Silver table including row counts, time ranges,
//! chunk information, and data quality summary.
//!
//! # Arguments
//!
//! ```json
//! { "table_name": "air_quality_observations" }
//! ```
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "table_name": "air_quality_observations",
//!   "row_count": 142857,
//!   "time_range": {
//!     "min": "2026-01-01T00:00:00Z",
//!     "max": "2026-01-17T23:59:59Z"
//!   },
//!   "chunk_count": 17,
//!   "total_bytes": 52428800,
//!   "dq_summary": {
//!     "total_rules": 5,
//!     "columns_with_rules": 3
//!   }
//! }
//! ```
//!
//! # Error Response
//!
//! ```json
//! {
//!   "success": false,
//!   "error": "Table not found: nonexistent_table",
//!   "code": "STREAM_NOT_FOUND"
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::SilverStorage;

/// Input arguments for silver_stats.
#[derive(Debug, Clone, Deserialize)]
pub struct SilverStatsArgs {
    /// Table name to get statistics for (required)
    pub table_name: String,
}

/// Response structure for silver_stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverStatsResponse {
    /// Success flag
    pub success: bool,

    /// Table name
    pub table_name: String,

    /// Total row count
    pub row_count: i64,

    /// Time range of data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRangeEntry>,

    /// Number of chunks (for hypertables)
    pub chunk_count: i64,

    /// Total bytes used
    pub total_bytes: i64,

    /// Data quality summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dq_summary: Option<DqSummaryEntry>,
}

/// Time range information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangeEntry {
    /// Minimum (earliest) timestamp
    pub min: DateTime<Utc>,

    /// Maximum (latest) timestamp
    pub max: DateTime<Utc>,
}

/// Data quality summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DqSummaryEntry {
    /// Total number of DQ rules
    pub total_rules: i32,

    /// Columns with at least one rule
    pub columns_with_rules: i32,
}

/// Execute the silver_stats tool.
///
/// # Arguments
///
/// * `storage` - Silver storage implementation
/// * `args` - Tool arguments containing table_name
///
/// # Returns
///
/// MCP tool result with table statistics
pub async fn execute<S>(storage: &S, args: SilverStatsArgs) -> McpResult<McpToolResult>
where
    S: SilverStorage + ?Sized,
{
    // Validate required argument
    if args.table_name.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "table_name is required and cannot be empty".to_string(),
        ));
    }

    // Get statistics from storage
    let stats = storage.get_stats(&args.table_name).await?;

    // Convert time range
    let time_range = stats.time_range.map(|tr| TimeRangeEntry {
        min: tr.min,
        max: tr.max,
    });

    // Convert DQ summary
    let dq_summary = stats.dq_summary.map(|dq| DqSummaryEntry {
        total_rules: dq.total_rules,
        columns_with_rules: dq.columns_with_rules,
    });

    let response = SilverStatsResponse {
        success: true,
        table_name: stats.table_name,
        row_count: stats.row_count,
        time_range,
        chunk_count: stats.chunk_count,
        total_bytes: stats.total_bytes,
        dq_summary,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Parse arguments from JSON value.
pub fn parse_args(args: Option<serde_json::Value>) -> McpResult<SilverStatsArgs> {
    match args {
        Some(value) => serde_json::from_value(value)
            .map_err(|e| McpError::InvalidParams(format!("Invalid arguments: {}", e))),
        None => Err(McpError::InvalidParams(
            "Missing required argument: table_name".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DqSummary, MockSilverStorage, SilverTableStats};
    use chrono::TimeZone;

    #[tokio::test]
    async fn test_silver_stats_success() {
        let mut storage = MockSilverStorage::new();

        let min_time = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let max_time = Utc.with_ymd_and_hms(2026, 1, 17, 23, 59, 59).unwrap();

        storage
            .expect_get_stats()
            .with(mockall::predicate::eq("air_quality_readings"))
            .returning(move |_| {
                Ok(SilverTableStats::new("air_quality_readings")
                    .with_row_count(50000)
                    .with_time_range(min_time, max_time)
                    .with_chunk_count(17)
                    .with_total_bytes(50 * 1024 * 1024)
                    .with_dq_summary(DqSummary::new(5, 3)))
            });

        let args = SilverStatsArgs {
            table_name: "air_quality_readings".to_string(),
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SilverStatsResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.table_name, "air_quality_readings");
        assert_eq!(response.row_count, 50000);
        assert_eq!(response.chunk_count, 17);
        assert_eq!(response.total_bytes, 50 * 1024 * 1024);

        // Check time range
        assert!(response.time_range.is_some());
        let tr = response.time_range.unwrap();
        assert_eq!(tr.min, min_time);
        assert_eq!(tr.max, max_time);

        // Check DQ summary
        assert!(response.dq_summary.is_some());
        let dq = response.dq_summary.unwrap();
        assert_eq!(dq.total_rules, 5);
        assert_eq!(dq.columns_with_rules, 3);
    }

    #[tokio::test]
    async fn test_silver_stats_table_not_found() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_get_stats()
            .with(mockall::predicate::eq("nonexistent_table"))
            .returning(|table| Err(McpError::StreamNotFound(table.to_string())));

        let args = SilverStatsArgs {
            table_name: "nonexistent_table".to_string(),
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
        assert!(err.to_string().contains("nonexistent_table"));
    }

    #[tokio::test]
    async fn test_silver_stats_empty_table_name() {
        let storage = MockSilverStorage::new();

        let args = SilverStatsArgs {
            table_name: "".to_string(),
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("table_name"));
    }

    #[tokio::test]
    async fn test_silver_stats_storage_error() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_get_stats()
            .returning(|_| Err(McpError::StorageError("Query timeout".to_string())));

        let args = SilverStatsArgs {
            table_name: "some_table".to_string(),
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_silver_stats_without_optional_fields() {
        let mut storage = MockSilverStorage::new();

        storage.expect_get_stats().returning(|_| {
            Ok(SilverTableStats::new("minimal_table")
                .with_row_count(100)
                .with_chunk_count(1)
                .with_total_bytes(1024))
        });

        let args = SilverStatsArgs {
            table_name: "minimal_table".to_string(),
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SilverStatsResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.row_count, 100);
        assert!(response.time_range.is_none());
        assert!(response.dq_summary.is_none());
    }

    #[test]
    fn test_parse_args_success() {
        let json = serde_json::json!({
            "table_name": "test_table"
        });

        let args = parse_args(Some(json)).unwrap();
        assert_eq!(args.table_name, "test_table");
    }

    #[test]
    fn test_parse_args_missing() {
        let result = parse_args(None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidParams(_)));
    }

    #[test]
    fn test_parse_args_invalid() {
        let json = serde_json::json!({
            "wrong_field": "value"
        });

        let result = parse_args(Some(json));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn test_silver_stats_with_zero_values() {
        let mut storage = MockSilverStorage::new();

        storage.expect_get_stats().returning(|_| {
            Ok(SilverTableStats::new("empty_table")
                .with_row_count(0)
                .with_chunk_count(0)
                .with_total_bytes(0))
        });

        let args = SilverStatsArgs {
            table_name: "empty_table".to_string(),
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SilverStatsResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.row_count, 0);
        assert_eq!(response.chunk_count, 0);
        assert_eq!(response.total_bytes, 0);
    }
}
