//! etl_status Tool Implementation
//!
//! Get current/latest ETL status for one or all streams.
//!
//! # Arguments
//!
//! * `stream_id` - Optional: If provided, return status only for this stream.
//!   If omitted, return status for all streams.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "streams": [
//!     {
//!       "stream_id": "air-quality",
//!       "status": "healthy",
//!       "last_run": {
//!         "id": "550e8400-e29b-41d4-a716-446655440000",
//!         "started_at": "2026-01-16T21:00:00Z",
//!         "completed_at": "2026-01-16T21:00:02Z",
//!         "duration_ms": 2150,
//!         "rows_processed": 288,
//!         "rows_flagged": 5,
//!         "rows_rejected": 2
//!       },
//!       "runs_last_24h": {
//!         "total": 288,
//!         "succeeded": 287,
//!         "failed": 1
//!       }
//!     }
//!   ]
//! }
//! ```
//!
//! # Status Values
//!
//! - `healthy` - All recent runs succeeded
//! - `warning` - Some recent failures (< 20%)
//! - `error` - High failure rate or current run failing
//! - `unknown` - No ETL runs recorded

use serde::{Deserialize, Serialize};

use crate::error::McpResult;
use crate::mcp::protocol::McpToolResult;
use crate::storage::EtlRunStore;

/// Arguments for the etl_status tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EtlStatusArgs {
    /// Optional stream ID to filter by. If omitted, returns all streams.
    #[serde(default)]
    pub stream_id: Option<String>,
}

/// Response structure for etl_status tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtlStatusResponse {
    /// Success flag.
    pub success: bool,

    /// List of stream statuses.
    pub streams: Vec<StreamStatusInfo>,
}

/// Status information for a single stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatusInfo {
    /// Stream identifier.
    pub stream_id: String,

    /// Current status: "healthy", "warning", "error", "unknown".
    pub status: String,

    /// Information about the last ETL run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<LastRunInfo>,

    /// Statistics for runs in the last 24 hours.
    pub runs_last_24h: RunStatsInfo,
}

/// Information about the last ETL run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastRunInfo {
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

    /// Number of rows processed.
    pub rows_processed: i64,

    /// Number of rows flagged by DQ rules.
    pub rows_flagged: i64,

    /// Number of rows rejected by DQ rules.
    pub rows_rejected: i64,
}

/// Statistics for ETL runs over a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatsInfo {
    /// Total number of runs.
    pub total: i32,

    /// Number of successful runs.
    pub succeeded: i32,

    /// Number of failed runs.
    pub failed: i32,
}

/// Execute the etl_status tool.
///
/// # Arguments
///
/// * `etl_store` - ETL run storage for querying status
/// * `args` - Tool arguments (optional stream_id filter)
///
/// # Returns
///
/// MCP tool result with stream status information
pub async fn execute<E>(etl_store: &E, args: EtlStatusArgs) -> McpResult<McpToolResult>
where
    E: EtlRunStore + ?Sized,
{
    // Query ETL status from storage
    let statuses = etl_store.get_status(args.stream_id).await?;

    // Transform storage types to response types
    let streams: Vec<StreamStatusInfo> = statuses
        .into_iter()
        .map(|s| {
            let last_run = s.last_run.map(|r| LastRunInfo {
                id: r.id,
                started_at: r.started_at.to_rfc3339(),
                completed_at: r.completed_at.map(|dt| dt.to_rfc3339()),
                duration_ms: r.duration_ms,
                rows_processed: r.rows_processed,
                rows_flagged: r.rows_flagged,
                rows_rejected: r.rows_rejected,
            });

            StreamStatusInfo {
                stream_id: s.stream_id,
                status: s.status,
                last_run,
                runs_last_24h: RunStatsInfo {
                    total: s.runs_last_24h.total,
                    succeeded: s.runs_last_24h.succeeded,
                    failed: s.runs_last_24h.failed,
                },
            }
        })
        .collect();

    let response = EtlStatusResponse {
        success: true,
        streams,
    };

    McpToolResult::success(&response)
        .map_err(|e| crate::error::McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::{EtlRunInfo, EtlStreamStatus, MockEtlRunStore, RunStats};
    use chrono::{TimeZone, Utc};

    #[tokio::test]
    async fn test_etl_status_returns_all_streams() {
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

        let args = EtlStatusArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlStatusResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.streams.len(), 2);
        assert_eq!(response.streams[0].stream_id, "air-quality");
        assert_eq!(response.streams[0].status, "healthy");
        assert!(response.streams[0].last_run.is_some());
        assert_eq!(response.streams[0].runs_last_24h.total, 24);
    }

    #[tokio::test]
    async fn test_etl_status_with_stream_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(Some("air-quality".to_string())))
            .times(1)
            .returning(|_| {
                Ok(vec![EtlStreamStatus::new("air-quality", "healthy")
                    .with_runs_last_24h(RunStats::new(24, 24, 0))])
            });

        let args = EtlStatusArgs {
            stream_id: Some("air-quality".to_string()),
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlStatusResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.streams.len(), 1);
        assert_eq!(response.streams[0].stream_id, "air-quality");
    }

    #[tokio::test]
    async fn test_etl_status_empty_result() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(Some("nonexistent".to_string())))
            .times(1)
            .returning(|_| Ok(vec![]));

        let args = EtlStatusArgs {
            stream_id: Some("nonexistent".to_string()),
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlStatusResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.streams.is_empty());
    }

    #[tokio::test]
    async fn test_etl_status_propagates_storage_error() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .times(1)
            .returning(|_| Err(McpError::StorageError("Database connection failed".to_string())));

        let args = EtlStatusArgs::default();
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_etl_status_serializes_last_run_correctly() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .times(1)
            .returning(|_| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
                let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();
                Ok(vec![EtlStreamStatus::new("air-quality", "warning")
                    .with_last_run(
                        EtlRunInfo::new("run-123", started)
                            .with_completed_at(completed)
                            .with_row_counts(500, 10, 3),
                    )
                    .with_runs_last_24h(RunStats::new(24, 20, 4))])
            });

        let args = EtlStatusArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlStatusResponse = serde_json::from_str(text).unwrap();

        let last_run = response.streams[0].last_run.as_ref().unwrap();
        assert_eq!(last_run.id, "run-123");
        assert_eq!(last_run.rows_processed, 500);
        assert_eq!(last_run.rows_flagged, 10);
        assert_eq!(last_run.rows_rejected, 3);
        assert!(last_run.duration_ms.is_some());
    }

    #[tokio::test]
    async fn test_etl_status_handles_no_last_run() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .times(1)
            .returning(|_| {
                Ok(vec![EtlStreamStatus::new("new-stream", "unknown")
                    .with_runs_last_24h(RunStats::new(0, 0, 0))])
            });

        let args = EtlStatusArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: EtlStatusResponse = serde_json::from_str(text).unwrap();

        assert!(response.streams[0].last_run.is_none());
        assert_eq!(response.streams[0].status, "unknown");
    }
}
