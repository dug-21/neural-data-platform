//! sample_data Tool Implementation
//!
//! Retrieves sample rows from a Bronze stream for data exploration.
//! Returns the full Bronze envelope including timestamp, source_id, ndp_id,
//! context, and raw_payload.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "stream_id": "outdoor-weather",
//!   "row_count": 3,
//!   "rows": [
//!     {
//!       "timestamp": 1767452639760716,
//!       "source_id": "outdoor-weather-Http",
//!       "ndp_id": "weather-owm-002",
//!       "context": {...},
//!       "raw_payload": {...}
//!     }
//!   ],
//!   "source_file": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::BronzeStorage;

/// Input parameters for sample_data tool.
#[derive(Debug, Clone, Deserialize)]
pub struct SampleDataArgs {
    /// Stream identifier (required)
    pub stream_id: String,

    /// Number of rows to return (default: 10, max: 100)
    #[serde(default = "default_n")]
    pub n: usize,
}

fn default_n() -> usize {
    10
}

/// A single row from Bronze storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BronzeRow {
    /// Ingestion timestamp (microseconds since epoch)
    pub timestamp: i64,

    /// Source identifier
    pub source_id: String,

    /// Platform-assigned stable ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Config-derived metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    /// Raw payload from source
    pub raw_payload: Value,
}

/// Response structure for sample_data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleDataResponse {
    /// Success flag
    pub success: bool,

    /// Stream identifier
    pub stream_id: String,

    /// Number of rows returned
    pub row_count: usize,

    /// Sample rows
    pub rows: Vec<BronzeRow>,

    /// Source Parquet file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
}

/// Execute the sample_data tool.
///
/// # Arguments
///
/// * `storage` - Bronze storage for reading Parquet data
/// * `args` - Tool arguments (stream_id, n)
///
/// # Limits
///
/// - Minimum rows: 1
/// - Maximum rows: 100
/// - Default: 10
pub async fn execute<S>(storage: &S, args: Value) -> McpResult<McpToolResult>
where
    S: BronzeStorage + ?Sized,
{
    let args: SampleDataArgs = serde_json::from_value(args)
        .map_err(|e| McpError::InvalidRequest(format!("Invalid arguments: {}", e)))?;

    // Validate stream_id format
    super::describe_schema::validate_stream_id(&args.stream_id)?;

    // Validate n parameter
    if args.n == 0 {
        return Err(McpError::InvalidRequest(
            "Parameter 'n' must be at least 1".to_string(),
        ));
    }

    if args.n > 100 {
        return Err(McpError::InvalidRequest(
            "Parameter 'n' exceeds maximum value of 100".to_string(),
        ));
    }

    // Read sample rows from storage using the sample() method
    let sample_values = storage.sample(&args.stream_id, args.n).await?;

    // Get source file path from latest partition
    let source_file = storage
        .latest_partition(&args.stream_id)
        .await
        .ok()
        .flatten();

    // Convert JSON values to BronzeRow format
    let rows: Vec<BronzeRow> = sample_values
        .into_iter()
        .map(|v| BronzeRow {
            timestamp: v
                .get("timestamp")
                .and_then(|t| t.as_i64())
                .unwrap_or(0),
            source_id: v
                .get("source_id")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            ndp_id: v.get("ndp_id").and_then(|s| s.as_str()).map(String::from),
            context: v.get("context").cloned(),
            raw_payload: v.get("raw_payload").cloned().unwrap_or(Value::Null),
        })
        .collect();

    let response = SampleDataResponse {
        success: true,
        stream_id: args.stream_id,
        row_count: rows.len(),
        rows,
        source_file,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MockBronzeStorage;
    use serde_json::json;

    #[tokio::test]
    async fn test_sample_data_default_n() {
        let mut storage = MockBronzeStorage::new();
        storage.expect_sample().returning(|_, _| {
            Ok(vec![json!({
                "timestamp": 1234567890i64,
                "source_id": "test-stream-Mqtt",
                "ndp_id": "test-001",
                "context": {"key": "value"},
                "raw_payload": {"data": 42}
            })])
        });
        storage
            .expect_latest_partition()
            .returning(|_| Ok(Some("year=2026/month=01/day=03".to_string())));

        let args = json!({
            "stream_id": "test-stream"
        });

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: SampleDataResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.stream_id, "test-stream");
        assert_eq!(response.row_count, 1);
    }

    #[tokio::test]
    async fn test_sample_data_n_exceeds_max() {
        let storage = MockBronzeStorage::new();

        let args = json!({
            "stream_id": "test-stream",
            "n": 200
        });

        let result = execute(&storage, args).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            McpError::InvalidRequest(msg) => {
                assert!(msg.contains("exceeds maximum"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[tokio::test]
    async fn test_sample_data_n_zero() {
        let storage = MockBronzeStorage::new();

        let args = json!({
            "stream_id": "test-stream",
            "n": 0
        });

        let result = execute(&storage, args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sample_data_invalid_stream_id() {
        let storage = MockBronzeStorage::new();

        let args = json!({
            "stream_id": "Invalid-Stream"
        });

        let result = execute(&storage, args).await;
        assert!(result.is_err());
    }
}
