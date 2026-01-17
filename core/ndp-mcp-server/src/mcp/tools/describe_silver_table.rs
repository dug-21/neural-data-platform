//! describe_silver_table Tool Implementation
//!
//! Get detailed schema for a Silver table including columns and hypertable info.
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
//!   "description": "Air quality sensor readings",
//!   "columns": [
//!     {
//!       "column_name": "timestamp",
//!       "data_type": "TIMESTAMPTZ",
//!       "unit": null,
//!       "description": "Reading timestamp",
//!       "nullable": false,
//!       "is_primary_key": true
//!     },
//!     {
//!       "column_name": "pm25",
//!       "data_type": "DOUBLE PRECISION",
//!       "unit": "ug/m3",
//!       "description": "PM2.5 particulate matter",
//!       "nullable": true,
//!       "is_primary_key": false
//!     }
//!   ],
//!   "hypertable_info": {
//!     "time_column": "timestamp",
//!     "chunk_interval": "1 day",
//!     "chunk_count": 30,
//!     "total_bytes": 104857600
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

use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::SilverStorage;

/// Input arguments for describe_silver_table.
#[derive(Debug, Clone, Deserialize)]
pub struct DescribeSilverTableArgs {
    /// Table name to describe (required)
    pub table_name: String,
}

/// Response structure for describe_silver_table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeSilverTableResponse {
    /// Success flag
    pub success: bool,

    /// Table name
    pub table_name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Column definitions
    pub columns: Vec<ColumnEntry>,

    /// Hypertable metadata (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hypertable_info: Option<HypertableEntry>,
}

/// Column information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnEntry {
    /// Column name
    pub column_name: String,

    /// PostgreSQL data type
    pub data_type: String,

    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the column can contain NULL
    pub nullable: bool,

    /// Whether this is part of the primary key
    pub is_primary_key: bool,
}

/// Hypertable information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypertableEntry {
    /// Time column name
    pub time_column: String,

    /// Chunk interval
    pub chunk_interval: String,

    /// Number of chunks
    pub chunk_count: i64,

    /// Total bytes used
    pub total_bytes: i64,
}

/// Execute the describe_silver_table tool.
///
/// # Arguments
///
/// * `storage` - Silver storage implementation
/// * `args` - Tool arguments containing table_name
///
/// # Returns
///
/// MCP tool result with table schema
pub async fn execute<S>(storage: &S, args: DescribeSilverTableArgs) -> McpResult<McpToolResult>
where
    S: SilverStorage + ?Sized,
{
    // Validate required argument
    if args.table_name.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "table_name is required and cannot be empty".to_string(),
        ));
    }

    // Get table description from storage
    let desc = storage.describe_table(&args.table_name).await?;

    // Convert columns
    let columns: Vec<ColumnEntry> = desc
        .columns
        .into_iter()
        .map(|c| ColumnEntry {
            column_name: c.column_name,
            data_type: c.data_type,
            unit: c.unit,
            description: c.description,
            nullable: c.nullable,
            is_primary_key: c.is_primary_key,
        })
        .collect();

    // Convert hypertable info
    let hypertable_info = desc.hypertable_info.map(|h| HypertableEntry {
        time_column: h.time_column,
        chunk_interval: h.chunk_interval,
        chunk_count: h.chunk_count,
        total_bytes: h.total_bytes,
    });

    let response = DescribeSilverTableResponse {
        success: true,
        table_name: desc.table_name,
        description: desc.description,
        columns,
        hypertable_info,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

/// Parse arguments from JSON value.
pub fn parse_args(args: Option<serde_json::Value>) -> McpResult<DescribeSilverTableArgs> {
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
    use crate::storage::{HypertableInfo, MockSilverStorage, SilverColumnInfo, SilverTableDescription};

    #[tokio::test]
    async fn test_describe_silver_table_success() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_describe_table()
            .with(mockall::predicate::eq("air_quality_readings"))
            .returning(|_| {
                Ok(SilverTableDescription::new("air_quality_readings")
                    .with_description("Air quality sensor readings")
                    .with_columns(vec![
                        SilverColumnInfo::new("timestamp", "TIMESTAMPTZ")
                            .with_nullable(false)
                            .with_primary_key(true)
                            .with_description("Reading timestamp"),
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

        let args = DescribeSilverTableArgs {
            table_name: "air_quality_readings".to_string(),
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: DescribeSilverTableResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.table_name, "air_quality_readings");
        assert_eq!(
            response.description,
            Some("Air quality sensor readings".to_string())
        );
        assert_eq!(response.columns.len(), 2);

        // Check timestamp column
        let ts_col = &response.columns[0];
        assert_eq!(ts_col.column_name, "timestamp");
        assert!(!ts_col.nullable);
        assert!(ts_col.is_primary_key);

        // Check pm25 column
        let pm_col = &response.columns[1];
        assert_eq!(pm_col.column_name, "pm25");
        assert_eq!(pm_col.unit, Some("ug/m3".to_string()));
        assert!(pm_col.nullable);
        assert!(!pm_col.is_primary_key);

        // Check hypertable info
        assert!(response.hypertable_info.is_some());
        let ht = response.hypertable_info.unwrap();
        assert_eq!(ht.time_column, "timestamp");
        assert_eq!(ht.chunk_interval, "1 day");
        assert_eq!(ht.chunk_count, 30);
    }

    #[tokio::test]
    async fn test_describe_silver_table_not_found() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_describe_table()
            .with(mockall::predicate::eq("nonexistent_table"))
            .returning(|table| Err(McpError::StreamNotFound(table.to_string())));

        let args = DescribeSilverTableArgs {
            table_name: "nonexistent_table".to_string(),
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
        assert!(err.to_string().contains("nonexistent_table"));
    }

    #[tokio::test]
    async fn test_describe_silver_table_empty_table_name() {
        let storage = MockSilverStorage::new();

        let args = DescribeSilverTableArgs {
            table_name: "   ".to_string(),
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("table_name"));
    }

    #[tokio::test]
    async fn test_describe_silver_table_storage_error() {
        let mut storage = MockSilverStorage::new();

        storage
            .expect_describe_table()
            .returning(|_| Err(McpError::StorageError("Connection timeout".to_string())));

        let args = DescribeSilverTableArgs {
            table_name: "some_table".to_string(),
        };

        let result = execute(&storage, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_describe_silver_table_without_hypertable() {
        let mut storage = MockSilverStorage::new();

        storage.expect_describe_table().returning(|_| {
            Ok(SilverTableDescription::new("regular_table").with_columns(vec![
                SilverColumnInfo::new("id", "INTEGER"),
                SilverColumnInfo::new("name", "TEXT"),
            ]))
        });

        let args = DescribeSilverTableArgs {
            table_name: "regular_table".to_string(),
        };

        let result = execute(&storage, args).await.unwrap();
        let text = &result.content[0].text;
        let response: DescribeSilverTableResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.hypertable_info.is_none());
        assert_eq!(response.columns.len(), 2);
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
}
