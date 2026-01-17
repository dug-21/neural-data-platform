//! trace_lineage Tool Implementation
//!
//! Trace a Silver column back to its Bronze source(s).
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "lineage": {
//!     "silver_table": "air_quality_observations",
//!     "silver_column": "pm25",
//!     "silver_type": "DOUBLE PRECISION",
//!     "silver_unit": "ug/m3",
//!     "lineage": [
//!       {
//!         "source_stream": "air-quality",
//!         "source_path": "$.pm25",
//!         "transformation": "cast to double",
//!         "bronze_type": "number",
//!         "bronze_unit": "ug/m3"
//!       }
//!     ],
//!     "dq_rules": [
//!       {
//!         "silver_table": "air_quality_observations",
//!         "silver_column": "pm25",
//!         "rule_name": "range_check",
//!         "rule_params": {"min": 0, "max": 500},
//!         "action": "flag",
//!         "scope": "column"
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! # Arguments
//!
//! - `silver_table` (required): The Silver table name
//! - `silver_column` (required): The Silver column name to trace
//!
//! # Example Request
//!
//! ```json
//! {
//!   "silver_table": "air_quality_observations",
//!   "silver_column": "pm25"
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::{DictionaryStore, LineageTrace};

/// Arguments for the trace_lineage tool.
#[derive(Debug, Clone, Deserialize)]
pub struct TraceLineageArgs {
    /// The Silver table name.
    pub silver_table: Option<String>,

    /// The Silver column name to trace.
    pub silver_column: Option<String>,
}

/// Response structure for trace_lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceLineageResponse {
    /// Success flag.
    pub success: bool,

    /// The lineage trace result.
    pub lineage: LineageTrace,
}

/// Execute the trace_lineage tool.
///
/// # Arguments
///
/// * `dictionary` - Dictionary store for metadata lookup
/// * `args` - Tool arguments as JSON value
///
/// # Returns
///
/// MCP tool result with lineage trace
pub async fn execute<D>(dictionary: &D, args: serde_json::Value) -> McpResult<McpToolResult>
where
    D: DictionaryStore + ?Sized,
{
    // Parse arguments
    let parsed_args: TraceLineageArgs =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    // Validate required parameters
    let silver_table = parsed_args.silver_table.ok_or_else(|| {
        McpError::InvalidParams("Missing required parameter: silver_table".to_string())
    })?;

    if silver_table.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "Parameter 'silver_table' cannot be empty".to_string(),
        ));
    }

    let silver_column = parsed_args.silver_column.ok_or_else(|| {
        McpError::InvalidParams("Missing required parameter: silver_column".to_string())
    })?;

    if silver_column.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "Parameter 'silver_column' cannot be empty".to_string(),
        ));
    }

    // Trace lineage from dictionary
    let lineage = dictionary
        .trace_lineage(&silver_table, &silver_column)
        .await?;

    // Build response
    let response = TraceLineageResponse {
        success: true,
        lineage,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DqRuleInfo, LineageSource, MockDictionaryStore};
    use serde_json::json;

    #[tokio::test]
    async fn test_trace_lineage_success() {
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
                        .with_silver_column("pm25")
                        .with_rule_params(json!({"min": 0, "max": 500}))]),
                )
            });

        let args = json!({
            "silver_table": "air_quality_readings",
            "silver_column": "pm25"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: TraceLineageResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.lineage.silver_table, "air_quality_readings");
        assert_eq!(response.lineage.silver_column, "pm25");
        assert_eq!(response.lineage.silver_type, "DOUBLE PRECISION");
        assert_eq!(response.lineage.silver_unit, Some("ug/m3".to_string()));
        assert_eq!(response.lineage.lineage.len(), 1);
        assert_eq!(response.lineage.lineage[0].source_stream, "air-quality");
        assert_eq!(response.lineage.lineage[0].source_path, "$.pm25");
        assert!(!response.lineage.dq_rules.is_empty());
    }

    #[tokio::test]
    async fn test_trace_lineage_multiple_sources() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
            .with(
                mockall::predicate::eq("weather_readings"),
                mockall::predicate::eq("temperature_c"),
            )
            .times(1)
            .returning(|_, _| {
                Ok(
                    LineageTrace::new("weather_readings", "temperature_c", "DOUBLE PRECISION")
                        .with_silver_unit("celsius")
                        .with_lineage(vec![
                            LineageSource::new("outdoor-weather", "$.main.temp")
                                .with_bronze_type("number")
                                .with_bronze_unit("kelvin")
                                .with_transformation("kelvin_to_celsius"),
                            LineageSource::new("weather-station", "$.temperature")
                                .with_bronze_type("number")
                                .with_bronze_unit("fahrenheit")
                                .with_transformation("fahrenheit_to_celsius"),
                        ]),
                )
            });

        let args = json!({
            "silver_table": "weather_readings",
            "silver_column": "temperature_c"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: TraceLineageResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.lineage.lineage.len(), 2);
        assert_eq!(response.lineage.lineage[0].source_stream, "outdoor-weather");
        assert_eq!(response.lineage.lineage[1].source_stream, "weather-station");
    }

    #[tokio::test]
    async fn test_trace_lineage_empty_lineage() {
        let mut mock = MockDictionaryStore::new();

        // Some columns might be computed, not from Bronze
        mock.expect_trace_lineage()
            .with(
                mockall::predicate::eq("air_quality_readings"),
                mockall::predicate::eq("ingestion_time"),
            )
            .times(1)
            .returning(|_, _| {
                Ok(LineageTrace::new(
                    "air_quality_readings",
                    "ingestion_time",
                    "TIMESTAMPTZ",
                )
                .with_lineage(vec![]))
            });

        let args = json!({
            "silver_table": "air_quality_readings",
            "silver_column": "ingestion_time"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: TraceLineageResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.lineage.lineage.is_empty());
    }

    #[tokio::test]
    async fn test_trace_lineage_table_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
            .with(
                mockall::predicate::eq("nonexistent_table"),
                mockall::predicate::eq("col"),
            )
            .times(1)
            .returning(|table, _| Err(McpError::StreamNotFound(table.to_string())));

        let args = json!({
            "silver_table": "nonexistent_table",
            "silver_column": "col"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
        assert!(err.to_string().contains("nonexistent_table"));
    }

    #[tokio::test]
    async fn test_trace_lineage_column_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
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
            "silver_table": "air_quality_readings",
            "silver_column": "nonexistent_column"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidRequest(_)));
        assert!(err.to_string().contains("nonexistent_column"));
    }

    #[tokio::test]
    async fn test_trace_lineage_missing_table_parameter() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "silver_column": "pm25"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("silver_table"));
    }

    #[tokio::test]
    async fn test_trace_lineage_missing_column_parameter() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "silver_table": "air_quality_readings"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("silver_column"));
    }

    #[tokio::test]
    async fn test_trace_lineage_empty_table() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "silver_table": "   ",
            "silver_column": "pm25"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_trace_lineage_empty_column() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "silver_table": "air_quality_readings",
            "silver_column": ""
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_trace_lineage_error_propagation() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage().times(1).returning(|_, _| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let args = json!({
            "silver_table": "test_table",
            "silver_column": "test_col"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }
}
