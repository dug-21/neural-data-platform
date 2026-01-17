//! sample_silver_data Tool Implementation
//!
//! Sample rows from a Silver table with optional time filtering.
//!
//! # Arguments
//!
//! ```json
//! {
//!   "table_name": "air_quality_observations",
//!   "n": 10,
//!   "since": "2026-01-01T00:00:00Z",
//!   "until": "2026-01-17T00:00:00Z",
//!   "order_by": "timestamp DESC"
//! }
//! ```
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "table_name": "air_quality_observations",
//!   "row_count": 10,
//!   "rows": [
//!     {
//!       "timestamp": "2026-01-17T10:00:00Z",
//!       "pm25": 12.5,
//!       "temperature": 23.4,
//!       "dq_flags": null
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
//!   "error": "Table not found: nonexistent_table",
//!   "code": "STREAM_NOT_FOUND"
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::{SampleFilters, SilverStorage};

/// Maximum number of rows that can be sampled.
const MAX_SAMPLE_SIZE: usize = 100;

/// Default number of rows to sample.
const DEFAULT_SAMPLE_SIZE: usize = 10;

/// Input arguments for sample_silver_data.
#[derive(Debug, Clone, Deserialize)]
pub struct SampleSilverDataArgs {
    /// Table name to sample from (required)
    pub table_name: String,

    /// Number of rows to return (default: 10, max: 100)
    #[serde(default)]
    pub n: Option<usize>,

    /// Only include rows after this timestamp (ISO 8601)
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,

    /// Only include rows before this timestamp (ISO 8601)
    #[serde(default)]
    pub until: Option<DateTime<Utc>>,

    /// Order by clause (e.g., "timestamp DESC")
    #[serde(default)]
    pub order_by: Option<String>,
}

/// Response structure for sample_silver_data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleSilverDataResponse {
    /// Success flag
    pub success: bool,

    /// Table name that was sampled
    pub table_name: String,

    /// Number of rows returned
    pub row_count: usize,

    /// Sample rows as JSON objects
    pub rows: Vec<Value>,
}

/// Execute the sample_silver_data tool.
///
/// # Arguments
///
/// * `storage` - Silver storage implementation
/// * `args` - Tool arguments
///
/// # Returns
///
/// MCP tool result with sample rows
pub async fn execute<S>(storage: &S, args: SampleSilverDataArgs) -> McpResult<McpToolResult>
where
    S: SilverStorage + ?Sized,
{
    // Validate required argument
    if args.table_name.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "table_name is required and cannot be empty".to_string(),
        ));
    }

    // Determine sample size with bounds checking
    let n = args
        .n
        .map(|size| size.min(MAX_SAMPLE_SIZE).max(1))
        .unwrap_or(DEFAULT_SAMPLE_SIZE);

    // Build filters if any time constraints specified
    let filters = if args.since.is_some() || args.until.is_some() || args.order_by.is_some() {
        let mut f = SampleFilters::new();
        if let Some(since) = args.since {
            f = f.with_since(since);
        }
        if let Some(until) = args.until {
            f = f.with_until(until);
        }
        if let Some(order_by) = args.order_by {
            f = f.with_order_by(order_by);
        }
        Some(f)
    } else {
        None
    };

    // Execute sample query
    let rows = storage.sample(&args.table_name, n, filters).await?;

    let response = SampleSilverDataResponse {
        success: true,
        table_name: args.table_name,
        row_count: rows.len(),
        rows,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Parse arguments from JSON value.
pub fn parse_args(args: Option<serde_json::Value>) -> McpResult<SampleSilverDataArgs> {
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
    use crate::storage::MockSilverStorage;
    use chrono::TimeZone;
    use serde_json::json;

    #[tokio::test]
    async fn test_sample_silver_data_success() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_sample()
            .with(
                mockall::predicate::eq("air_quality_readings"),
                mockall::predicate::eq(10),
                mockall::predicate::always(),
            )
            .returning(|_, _, _| {
                Ok(vec![
                    json!({
                        "timestamp": "2026-01-17T10:00:00Z",
                        "pm25": 12.5,
                        "temperature": 23.4
                    }),
                    json!({
                        "timestamp": "2026-01-17T10:01:00Z",
                        "pm25": 13.0,
                        "temperature": 23.5
                    }),
                ])
            });

        let args = SampleSilverDataArgs {
            table_name: "air_quality_readings".to_string(),
            n: None,
            since: None,
            until: None,
            order_by: None,
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SampleSilverDataResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.table_name, "air_quality_readings");
        assert_eq!(response.row_count, 2);
        assert_eq!(response.rows.len(), 2);
        assert_eq!(response.rows[0]["pm25"], 12.5);
    }

    #[tokio::test]
    async fn test_sample_silver_data_with_filters() {
        let mut storage = MockSilverStorage::new();

        let since = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2026, 1, 17, 0, 0, 0).unwrap();

        storage
            .expect_sample()
            .withf(|table, n, filters| {
                table == "test_table" && *n == 5 && filters.is_some() && {
                    let f = filters.as_ref().unwrap();
                    f.since.is_some() && f.until.is_some() && f.order_by.is_some()
                }
            })
            .returning(|_, _, _| Ok(vec![json!({"data": "value"})]));

        let args = SampleSilverDataArgs {
            table_name: "test_table".to_string(),
            n: Some(5),
            since: Some(since),
            until: Some(until),
            order_by: Some("timestamp DESC".to_string()),
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SampleSilverDataResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.row_count, 1);
    }

    #[tokio::test]
    async fn test_sample_silver_data_empty_result() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_sample()
            .returning(|_, _, _| Ok(vec![]));

        let args = SampleSilverDataArgs {
            table_name: "empty_table".to_string(),
            n: Some(10),
            since: None,
            until: None,
            order_by: None,
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SampleSilverDataResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.row_count, 0);
        assert!(response.rows.is_empty());
    }

    #[tokio::test]
    async fn test_sample_silver_data_table_not_found() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_sample()
            .returning(|table, _, _| Err(McpError::StreamNotFound(table.to_string())));

        let args = SampleSilverDataArgs {
            table_name: "nonexistent".to_string(),
            n: None,
            since: None,
            until: None,
            order_by: None,
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_sample_silver_data_empty_table_name() {
        let storage = MockSilverStorage::new();

        let args = SampleSilverDataArgs {
            table_name: "".to_string(),
            n: None,
            since: None,
            until: None,
            order_by: None,
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn test_sample_silver_data_n_capped_at_max() {
        let mut storage = MockSilverStorage::new();

        // Verify n is capped at 100
        storage
            .expect_sample()
            .with(
                mockall::predicate::eq("test_table"),
                mockall::predicate::eq(100), // Should be capped to MAX_SAMPLE_SIZE
                mockall::predicate::always(),
            )
            .returning(|_, _, _| Ok(vec![]));

        let args = SampleSilverDataArgs {
            table_name: "test_table".to_string(),
            n: Some(500), // Request more than max
            since: None,
            until: None,
            order_by: None,
        };

        let result = execute(&storage, args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sample_silver_data_n_minimum_one() {
        let mut storage = MockSilverStorage::new();

        // Verify n is at least 1
        storage
            .expect_sample()
            .with(
                mockall::predicate::eq("test_table"),
                mockall::predicate::eq(1), // Should be at least 1
                mockall::predicate::always(),
            )
            .returning(|_, _, _| Ok(vec![]));

        let args = SampleSilverDataArgs {
            table_name: "test_table".to_string(),
            n: Some(0), // Request 0
            since: None,
            until: None,
            order_by: None,
        };

        let result = execute(&storage, args).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_args_success() {
        let json = json!({
            "table_name": "test_table",
            "n": 20,
            "since": "2026-01-01T00:00:00Z"
        });

        let args = parse_args(Some(json)).unwrap();
        assert_eq!(args.table_name, "test_table");
        assert_eq!(args.n, Some(20));
        assert!(args.since.is_some());
    }

    #[test]
    fn test_parse_args_minimal() {
        let json = json!({
            "table_name": "test_table"
        });

        let args = parse_args(Some(json)).unwrap();
        assert_eq!(args.table_name, "test_table");
        assert!(args.n.is_none());
        assert!(args.since.is_none());
    }

    #[test]
    fn test_parse_args_missing() {
        let result = parse_args(None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidParams(_)));
    }

    #[test]
    fn test_parse_args_invalid_json() {
        let json = json!({
            "invalid": "fields"
        });

        let result = parse_args(Some(json));
        assert!(result.is_err());
    }
}
