//! data_freshness Tool Implementation
//!
//! Report data freshness across Bronze and Silver layers.
//!
//! # Arguments
//!
//! * `layer` - Optional: Filter by layer ("bronze", "silver", "all").
//!   Defaults to "all".
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "checked_at": "2026-01-16T21:05:00Z",
//!   "freshness": [
//!     {
//!       "layer": "silver",
//!       "identifier": "air_quality_observations",
//!       "latest_timestamp": "2026-01-16T21:00:00Z",
//!       "age_seconds": 300,
//!       "freshness_status": "fresh",
//!       "row_count": 50000,
//!       "last_etl_run": "2026-01-16T21:01:00Z"
//!     }
//!   ],
//!   "summary": {
//!     "bronze_streams": 8,
//!     "silver_tables": 4,
//!     "stale_count": 0,
//!     "critical_count": 2
//!   }
//! }
//! ```
//!
//! # Freshness Status Thresholds
//!
//! - `fresh` - Data is less than 5 minutes old
//! - `stale` - Data is between 5 and 30 minutes old
//! - `critical` - Data is more than 30 minutes old
//! - `unknown` - No timestamp available

use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::EtlRunStore;

/// Arguments for the data_freshness tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DataFreshnessArgs {
    /// Optional layer filter: "bronze", "silver", or "all" (default).
    #[serde(default)]
    pub layer: Option<String>,
}

/// Response structure for data_freshness tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFreshnessResponse {
    /// Success flag.
    pub success: bool,

    /// When this report was generated (ISO 8601).
    pub checked_at: String,

    /// Freshness entries for each stream/table.
    pub freshness: Vec<FreshnessInfo>,

    /// Summary of freshness across layers.
    pub summary: FreshnessSummaryInfo,
}

/// Freshness information for a single stream or table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessInfo {
    /// Layer: "bronze" or "silver".
    pub layer: String,

    /// Identifier (stream_id for Bronze, table_name for Silver).
    pub identifier: String,

    /// Latest timestamp in the data (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<String>,

    /// Age in seconds since latest timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<i64>,

    /// Status: "fresh", "stale", "critical", "unknown".
    pub freshness_status: String,

    /// Row count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<i64>,

    /// When the last ETL run completed (Silver only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_etl_run: Option<String>,
}

/// Summary of freshness across layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessSummaryInfo {
    /// Number of Bronze streams checked.
    pub bronze_streams: i32,

    /// Number of Silver tables checked.
    pub silver_tables: i32,

    /// Number of stale entries.
    pub stale_count: i32,

    /// Number of critical entries.
    pub critical_count: i32,
}

/// Execute the data_freshness tool.
///
/// # Arguments
///
/// * `etl_store` - ETL run storage for querying freshness
/// * `args` - Tool arguments (optional layer filter)
///
/// # Returns
///
/// MCP tool result with data freshness report
pub async fn execute<E>(etl_store: &E, args: DataFreshnessArgs) -> McpResult<McpToolResult>
where
    E: EtlRunStore + ?Sized,
{
    // Validate layer filter if provided
    let layer = match args.layer {
        Some(ref l) if l == "all" => None,
        Some(ref l) if l == "bronze" || l == "silver" => Some(l.clone()),
        Some(ref l) => {
            return Err(McpError::InvalidParams(format!(
                "Invalid layer '{}'. Must be one of: bronze, silver, all",
                l
            )));
        }
        None => None,
    };

    // Query freshness from storage
    let report = etl_store.get_freshness(layer).await?;

    // Transform storage types to response types
    let freshness: Vec<FreshnessInfo> = report
        .freshness
        .into_iter()
        .map(|f| FreshnessInfo {
            layer: f.layer,
            identifier: f.identifier,
            latest_timestamp: f.latest_timestamp.map(|dt| dt.to_rfc3339()),
            age_seconds: f.age_seconds,
            freshness_status: f.freshness_status,
            row_count: f.row_count,
            last_etl_run: f.last_etl_run.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    let response = DataFreshnessResponse {
        success: true,
        checked_at: report.checked_at.to_rfc3339(),
        freshness,
        summary: FreshnessSummaryInfo {
            bronze_streams: report.summary.bronze_streams,
            silver_tables: report.summary.silver_tables,
            stale_count: report.summary.stale_count,
            critical_count: report.summary.critical_count,
        },
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Calculate freshness status based on age in seconds.
///
/// # Thresholds
///
/// - fresh: < 5 minutes (300 seconds)
/// - stale: 5-30 minutes (300-1800 seconds)
/// - critical: > 30 minutes (1800+ seconds)
#[allow(dead_code)]
pub fn calculate_freshness_status(age_seconds: Option<i64>) -> &'static str {
    match age_seconds {
        None => "unknown",
        Some(age) if age < 300 => "fresh",
        Some(age) if age < 1800 => "stale",
        Some(_) => "critical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;
    use crate::storage::{FreshnessEntry, FreshnessReport, FreshnessSummary, MockEtlRunStore};
    use chrono::Utc;

    #[tokio::test]
    async fn test_data_freshness_returns_all_layers() {
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

        let args = DataFreshnessArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: DataFreshnessResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.freshness.len(), 2);
        assert_eq!(response.summary.bronze_streams, 1);
        assert_eq!(response.summary.silver_tables, 1);
    }

    #[tokio::test]
    async fn test_data_freshness_with_bronze_filter() {
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

        let args = DataFreshnessArgs {
            layer: Some("bronze".to_string()),
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: DataFreshnessResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.freshness.len(), 1);
        assert_eq!(response.freshness[0].layer, "bronze");
    }

    #[tokio::test]
    async fn test_data_freshness_with_silver_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .with(mockall::predicate::eq(Some("silver".to_string())))
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![FreshnessEntry::new(
                        "silver",
                        "air_quality_readings",
                        "fresh",
                    )
                    .with_last_etl_run(now)])
                    .with_summary(FreshnessSummary::new(0, 1, 0, 0)))
            });

        let args = DataFreshnessArgs {
            layer: Some("silver".to_string()),
        };
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: DataFreshnessResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.freshness.len(), 1);
        assert_eq!(response.freshness[0].layer, "silver");
    }

    #[tokio::test]
    async fn test_data_freshness_with_all_filter() {
        let mut mock = MockEtlRunStore::new();

        // "all" should be treated as None
        mock.expect_get_freshness()
            .with(mockall::predicate::eq(None::<String>))
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![])
                    .with_summary(FreshnessSummary::new(0, 0, 0, 0)))
            });

        let args = DataFreshnessArgs {
            layer: Some("all".to_string()),
        };
        let result = execute(&mock, args).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_data_freshness_invalid_layer() {
        let mock = MockEtlRunStore::new();

        let args = DataFreshnessArgs {
            layer: Some("invalid".to_string()),
        };
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("invalid"));
    }

    #[tokio::test]
    async fn test_data_freshness_empty_result() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![])
                    .with_summary(FreshnessSummary::new(0, 0, 0, 0)))
            });

        let args = DataFreshnessArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: DataFreshnessResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.freshness.is_empty());
    }

    #[tokio::test]
    async fn test_data_freshness_propagates_storage_error() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .times(1)
            .returning(|_| Err(McpError::StorageError("Database connection failed".to_string())));

        let args = DataFreshnessArgs::default();
        let result = execute(&mock, args).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_data_freshness_detects_stale_data() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                let stale_timestamp = now - chrono::Duration::hours(2);
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![FreshnessEntry::new("bronze", "air-quality", "stale")
                        .with_latest_timestamp(stale_timestamp, now)])
                    .with_summary(FreshnessSummary::new(1, 0, 1, 0)))
            });

        let args = DataFreshnessArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: DataFreshnessResponse = serde_json::from_str(text).unwrap();

        assert_eq!(response.summary.stale_count, 1);
        assert_eq!(response.freshness[0].freshness_status, "stale");
    }

    #[tokio::test]
    async fn test_data_freshness_critical_entries() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .times(1)
            .returning(|_| {
                let now = Utc::now();
                let critical_timestamp = now - chrono::Duration::hours(6);
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![FreshnessEntry::new(
                        "silver",
                        "outdoor_weather_readings",
                        "critical",
                    )
                    .with_latest_timestamp(critical_timestamp, now)])
                    .with_summary(FreshnessSummary::new(0, 1, 0, 1)))
            });

        let args = DataFreshnessArgs::default();
        let result = execute(&mock, args).await.unwrap();

        let text = &result.content[0].text;
        let response: DataFreshnessResponse = serde_json::from_str(text).unwrap();

        assert_eq!(response.summary.critical_count, 1);
        assert_eq!(response.freshness[0].freshness_status, "critical");
    }

    // Test the helper function for calculating freshness status
    #[test]
    fn test_calculate_freshness_status_fresh() {
        assert_eq!(calculate_freshness_status(Some(0)), "fresh");
        assert_eq!(calculate_freshness_status(Some(299)), "fresh");
    }

    #[test]
    fn test_calculate_freshness_status_stale() {
        assert_eq!(calculate_freshness_status(Some(300)), "stale");
        assert_eq!(calculate_freshness_status(Some(1799)), "stale");
    }

    #[test]
    fn test_calculate_freshness_status_critical() {
        assert_eq!(calculate_freshness_status(Some(1800)), "critical");
        assert_eq!(calculate_freshness_status(Some(10000)), "critical");
    }

    #[test]
    fn test_calculate_freshness_status_unknown() {
        assert_eq!(calculate_freshness_status(None), "unknown");
    }
}
