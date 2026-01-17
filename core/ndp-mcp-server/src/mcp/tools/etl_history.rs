//! etl_history Tool Implementation
//!
//! Retrieve historical ETL runs for trend analysis and debugging.
//!
//! # Arguments
//!
//! * `stream_id` - Required: The stream ID to query history for.
//! * `limit` - Optional: Maximum number of runs to return (default: 10, max: 100).
//! * `since` - Optional: ISO 8601 timestamp to filter runs after.
//! * `status` - Optional: Filter by status ("running", "success", "failed", "partial").
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "stream_id": "air-quality",
//!   "runs": [
//!     {
//!       "id": "550e8400-e29b-41d4-a716-446655440000",
//!       "started_at": "2026-01-16T21:00:00Z",
//!       "completed_at": "2026-01-16T21:00:02Z",
//!       "duration_ms": 2150,
//!       "status": "success",
//!       "run_mode": "incremental",
//!       "rows_processed": 288,
//!       "rows_flagged": 5,
//!       "rows_rejected": 2,
//!       "watermark_before": "2026-01-16T20:00:00Z",
//!       "watermark_after": "2026-01-16T21:00:00Z"
//!     }
//!   ],
//!   "summary": {
//!     "total_returned": 10,
//!     "total_available": 1440
//!   }
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::EtlRunStore;

/// Maximum limit for history query.
const MAX_LIMIT: usize = 100;

/// Default limit for history query.
const DEFAULT_LIMIT: usize = 10;

/// Arguments for the etl_history tool.
#[derive(Debug, Clone, Deserialize)]
pub struct EtlHistoryArgs {
    /// Required: Stream ID to query history for.
    pub stream_id: Option<String>,

    /// Maximum number of runs to return (default: 10, max: 100).
    #[serde(default)]
    pub limit: Option<usize>,

    /// ISO 8601 timestamp to filter runs after.
    #[serde(default)]
    pub since: Option<String>,

    /// Filter by status: "running", "success", "failed", "partial".
    #[serde(default)]
    pub status: Option<String>,
}

impl Default for EtlHistoryArgs {
    fn default() -> Self {
        Self {
            stream_id: None,
            limit: Some(DEFAULT_LIMIT),
            since: None,
            status: None,
        }
    }
}

/// Response structure for etl_history tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtlHistoryResponse {
    /// Success flag.
    pub success: bool,

    /// Stream identifier.
    pub stream_id: String,

    /// List of ETL runs.
    pub runs: Vec<RunDetailInfo>,

    /// Summary of the query result.
    pub summary: HistorySummaryInfo,
}

/// Detailed information about an ETL run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetailInfo {
    /// Unique run identifier.
    pub id: String,

    /// When the run started (ISO 8601).
    pub started_at: String,

    /// When the run completed (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,

    /// Duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,

    /// Run status: "success", "failed", "running", "partial".
    pub status: String,

    /// Run mode: "incremental", "full", "backfill".
    pub run_mode: String,

    /// Number of rows processed.
    pub rows_processed: i64,

    /// Number of rows flagged by DQ rules.
    pub rows_flagged: i64,

    /// Number of rows rejected by DQ rules.
    pub rows_rejected: i64,

    /// High watermark before this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_before: Option<String>,

    /// High watermark after this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_after: Option<String>,

    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    /// Additional error context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_context: Option<serde_json::Value>,
}

/// Summary of the history query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySummaryInfo {
    /// Number of runs returned.
    pub total_returned: i32,

    /// Total runs available (may be more than returned due to limit).
    pub total_available: i32,

    /// Time range of returned runs (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRangeInfo>,
}

/// Time range information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangeInfo {
    /// Minimum (earliest) timestamp.
    pub min: String,

    /// Maximum (latest) timestamp.
    pub max: String,
}

/// Execute the etl_history tool.
///
/// # Arguments
///
/// * `etl_store` - ETL run storage for querying history
/// * `args` - Tool arguments (stream_id, limit, since, status)
///
/// # Returns
///
/// MCP tool result with ETL run history
pub async fn execute<E>(etl_store: &E, args: EtlHistoryArgs) -> McpResult<McpToolResult>
where
    E: EtlRunStore + ?Sized,
{
    // Validate required stream_id
    let stream_id = args.stream_id.ok_or_else(|| {
        McpError::InvalidParams("Missing required parameter: stream_id".to_string())
    })?;

    // Validate and apply limit
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    // Parse since timestamp if provided
    let since: Option<DateTime<Utc>> = match args.since {
        Some(ref ts) => Some(
            DateTime::parse_from_rfc3339(ts)
                .map_err(|e| McpError::InvalidParams(format!("Invalid 'since' timestamp: {}", e)))?
                .with_timezone(&Utc),
        ),
        None => None,
    };

    // Validate status filter if provided
    if let Some(ref status) = args.status {
        let valid_statuses = ["running", "success", "failed", "partial"];
        if !valid_statuses.contains(&status.as_str()) {
            return Err(McpError::InvalidParams(format!(
                "Invalid status '{}'. Must be one of: {:?}",
                status, valid_statuses
            )));
        }
    }

    // Query ETL history from storage
    let history = etl_store
        .get_history(&stream_id, limit, since, args.status)
        .await?;

    // Transform storage types to response types
    let runs: Vec<RunDetailInfo> = history
        .runs
        .into_iter()
        .map(|r| RunDetailInfo {
            id: r.id,
            started_at: r.started_at.to_rfc3339(),
            completed_at: r.completed_at.map(|dt| dt.to_rfc3339()),
            duration_ms: r.duration_ms,
            status: r.status,
            run_mode: r.run_mode,
            rows_processed: r.rows_processed,
            rows_flagged: r.rows_flagged,
            rows_rejected: r.rows_rejected,
            watermark_before: r.watermark_before.map(|dt| dt.to_rfc3339()),
            watermark_after: r.watermark_after.map(|dt| dt.to_rfc3339()),
            error_message: r.error_message,
            error_context: r.error_context,
        })
        .collect();

    let time_range = history.summary.time_range.map(|tr| TimeRangeInfo {
        min: tr.min.to_rfc3339(),
        max: tr.max.to_rfc3339(),
    });

    let response = EtlHistoryResponse {
        success: true,
        stream_id: history.stream_id,
        runs,
        summary: HistorySummaryInfo {
            total_returned: history.summary.total_returned,
            total_available: history.summary.total_available,
            time_range,
        },
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::{EtlHistoryResult, EtlRunDetail, HistorySummary, MockEtlRunStore};
    use chrono::{DateTime, TimeZone, Utc};

    #[tokio::test]
    async fn test_etl_history_returns_runs() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(10),
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

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlHistoryResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.stream_id, "air-quality");
        assert_eq!(response.runs.len(), 2);
        assert_eq!(response.summary.total_returned, 2);
        assert_eq!(response.summary.total_available, 100);
    }

    #[tokio::test]
    async fn test_etl_history_empty_result() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .times(1)
            .returning(|stream_id, _, _, _| {
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![])
                    .with_summary(HistorySummary::new(0, 0)))
            });

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlHistoryResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.runs.is_empty());
        assert_eq!(response.summary.total_returned, 0);
    }

    #[tokio::test]
    async fn test_etl_history_missing_stream_id() {
        let mock = MockEtlRunStore::new();

        let args = EtlHistoryArgs {
            stream_id: None,
            ..Default::default()
        };
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("stream_id"));
    }

    #[tokio::test]
    async fn test_etl_history_invalid_since_timestamp() {
        let mock = MockEtlRunStore::new();

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            since: Some("not-a-timestamp".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("since"));
    }

    #[tokio::test]
    async fn test_etl_history_invalid_status() {
        let mock = MockEtlRunStore::new();

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            status: Some("invalid_status".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("invalid_status"));
    }

    #[tokio::test]
    async fn test_etl_history_with_status_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(10),
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
                    .with_summary(HistorySummary::new(1, 5)))
            });

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            status: Some("failed".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlHistoryResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.runs.len(), 1);
        assert_eq!(response.runs[0].status, "failed");
        assert!(response.runs[0].error_message.is_some());
    }

    #[tokio::test]
    async fn test_etl_history_limit_capped_at_max() {
        let mut mock = MockEtlRunStore::new();

        // Should be capped to MAX_LIMIT (100)
        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(100), // capped from 500
                mockall::predicate::always(),
                mockall::predicate::always(),
            )
            .times(1)
            .returning(|stream_id, _, _, _| {
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![])
                    .with_summary(HistorySummary::new(0, 0)))
            });

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            limit: Some(500), // Exceeds max
            ..Default::default()
        };
        let result = execute(&mock, args).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_etl_history_propagates_storage_error() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .times(1)
            .returning(|stream_id, _, _, _| Err(McpError::StreamNotFound(stream_id.to_string())));

        let args = EtlHistoryArgs {
            stream_id: Some("nonexistent".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_etl_history_with_since_filter() {
        let mut mock = MockEtlRunStore::new();

        // Expected since timestamp (used in withf closure)
        let _since = Utc.with_ymd_and_hms(2026, 1, 17, 0, 0, 0).unwrap();

        mock.expect_get_history()
            .withf(move |stream, limit, since_opt, status| {
                stream == "air-quality"
                    && *limit == 10
                    && since_opt.is_some()
                    && status.is_none()
            })
            .times(1)
            .returning(|stream_id, _, _, _| {
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![])
                    .with_summary(HistorySummary::new(0, 0)))
            });

        let args = EtlHistoryArgs {
            stream_id: Some("air-quality".to_string()),
            since: Some("2026-01-17T00:00:00Z".to_string()),
            ..Default::default()
        };
        let result = execute(&mock, args).await;

        assert!(result.is_ok());
    }
}
