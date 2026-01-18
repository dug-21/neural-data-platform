//! describe_column Tool Implementation
//!
//! Get comprehensive details for a specific column including lineage and DQ rules.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "column": {
//!     "layer": "silver",
//!     "table_or_stream": "air_quality_observations",
//!     "column_name": "pm25",
//!     "data_type": "DOUBLE PRECISION",
//!     "unit": "ug/m3",
//!     "description": "PM2.5 particulate matter concentration",
//!     "nullable": true,
//!     "source": {
//!       "stream": "air-quality",
//!       "path": "$.pm25",
//!       "transformation": null
//!     },
//!     "dq_rules": [
//!       {
//!         "silver_table": "air_quality_observations",
//!         "silver_column": "pm25",
//!         "rule_name": "range_check",
//!         "rule_params": {"min": 0, "max": 500},
//!         "action": "flag",
//!         "scope": "column"
//!       }
//!     ],
//!     "validation_range": {
//!       "min": 0.0,
//!       "max": 500.0
//!     }
//!   }
//! }
//! ```
//!
//! # Arguments
//!
//! - `table_or_stream` (required): Table name (Silver) or stream ID (Bronze)
//! - `column_name` (required): The column name to describe
//!
//! # Example Request
//!
//! ```json
//! {
//!   "table_or_stream": "air_quality_observations",
//!   "column_name": "pm25"
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::{ColumnDescription, DictionaryStore};

/// Arguments for the describe_column tool.
#[derive(Debug, Clone, Deserialize)]
pub struct DescribeColumnArgs {
    /// Table name (Silver) or stream ID (Bronze).
    pub table_or_stream: Option<String>,

    /// The column name to describe.
    pub column_name: Option<String>,
}

/// Response structure for describe_column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeColumnResponse {
    /// Success flag.
    pub success: bool,

    /// The column description.
    pub column: ColumnDescription,
}

/// Execute the describe_column tool.
///
/// # Arguments
///
/// * `dictionary` - Dictionary store for metadata lookup
/// * `args` - Tool arguments as JSON value
///
/// # Returns
///
/// MCP tool result with column description
pub async fn execute<D>(dictionary: &D, args: serde_json::Value) -> McpResult<McpToolResult>
where
    D: DictionaryStore + ?Sized,
{
    // Parse arguments
    let parsed_args: DescribeColumnArgs =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    // Validate required parameters
    let table_or_stream = parsed_args.table_or_stream.ok_or_else(|| {
        McpError::InvalidParams("Missing required parameter: table_or_stream".to_string())
    })?;

    if table_or_stream.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "Parameter 'table_or_stream' cannot be empty".to_string(),
        ));
    }

    let column_name = parsed_args.column_name.ok_or_else(|| {
        McpError::InvalidParams("Missing required parameter: column_name".to_string())
    })?;

    if column_name.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "Parameter 'column_name' cannot be empty".to_string(),
        ));
    }

    // Get column description from dictionary
    let column = dictionary
        .describe_column(&table_or_stream, &column_name)
        .await?;

    // Build response
    let response = DescribeColumnResponse {
        success: true,
        column,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DqRuleInfo, MockDictionaryStore, SourceInfo, ValidationRange};
    use serde_json::json;

    #[tokio::test]
    async fn test_describe_column_success() {
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

        let args = json!({
            "table_or_stream": "air_quality_readings",
            "column_name": "pm25"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: DescribeColumnResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.column.layer, "silver");
        assert_eq!(response.column.table_or_stream, "air_quality_readings");
        assert_eq!(response.column.column_name, "pm25");
        assert_eq!(response.column.data_type, "DOUBLE PRECISION");
        assert_eq!(response.column.unit, Some("ug/m3".to_string()));
        assert!(response.column.source.is_some());
        assert!(!response.column.dq_rules.is_empty());
        assert!(response.column.validation_range.is_some());
    }

    #[tokio::test]
    async fn test_describe_column_bronze_stream() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq("temperature"),
            )
            .times(1)
            .returning(|_, _| {
                Ok(
                    ColumnDescription::new("bronze", "air-quality", "temperature", "number")
                        .with_unit("celsius")
                        .with_description("Temperature reading from sensor"),
                )
            });

        let args = json!({
            "table_or_stream": "air-quality",
            "column_name": "temperature"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: DescribeColumnResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.column.layer, "bronze");
        assert!(response.column.source.is_none()); // Bronze doesn't have source lineage
    }

    #[tokio::test]
    async fn test_describe_column_table_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .with(
                mockall::predicate::eq("nonexistent_table"),
                mockall::predicate::eq("col"),
            )
            .times(1)
            .returning(|table, _| Err(McpError::StreamNotFound(table.to_string())));

        let args = json!({
            "table_or_stream": "nonexistent_table",
            "column_name": "col"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
        assert!(err.to_string().contains("nonexistent_table"));
    }

    #[tokio::test]
    async fn test_describe_column_column_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .with(
                mockall::predicate::eq("air_quality_readings"),
                mockall::predicate::eq("nonexistent_column"),
            )
            .times(1)
            .returning(|_, col| {
                Err(McpError::InvalidRequest(format!(
                    "Column not found: {}",
                    col
                )))
            });

        let args = json!({
            "table_or_stream": "air_quality_readings",
            "column_name": "nonexistent_column"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidRequest(_)));
        assert!(err.to_string().contains("nonexistent_column"));
    }

    #[tokio::test]
    async fn test_describe_column_missing_table_parameter() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "column_name": "pm25"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("table_or_stream"));
    }

    #[tokio::test]
    async fn test_describe_column_missing_column_parameter() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "table_or_stream": "air_quality_readings"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("column_name"));
    }

    #[tokio::test]
    async fn test_describe_column_empty_table() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "table_or_stream": "   ",
            "column_name": "pm25"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_describe_column_empty_column() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "table_or_stream": "air_quality_readings",
            "column_name": ""
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_describe_column_error_propagation() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column().times(1).returning(|_, _| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let args = json!({
            "table_or_stream": "test_table",
            "column_name": "test_col"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }
}
