//! query_dictionary Tool Implementation
//!
//! Search the data dictionary for columns matching a query.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "query": "temperature",
//!   "layer": "all",
//!   "result_count": 4,
//!   "results": [
//!     {
//!       "layer": "silver",
//!       "entity": "air_quality_observations",
//!       "column_name": "temperature_c",
//!       "data_type": "DOUBLE PRECISION",
//!       "unit": "Celsius"
//!     }
//!   ]
//! }
//! ```
//!
//! # Arguments
//!
//! - `query` (required): Search term for partial matching on column names/descriptions
//! - `layer` (optional): Filter by layer - "bronze", "silver", or "all" (default: "all")
//!
//! # Example Request
//!
//! ```json
//! {
//!   "query": "pm25",
//!   "layer": "silver"
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::DictionaryStore;

/// Arguments for the query_dictionary tool.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryDictionaryArgs {
    /// Search term for partial matching on column names/descriptions.
    pub query: Option<String>,

    /// Layer filter: "bronze", "silver", or "all".
    /// Defaults to "all" if not specified.
    #[serde(default = "default_layer")]
    pub layer: String,
}

fn default_layer() -> String {
    "all".to_string()
}

/// Response structure for query_dictionary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDictionaryResponse {
    /// Success flag.
    pub success: bool,

    /// The search query that was executed.
    pub query: String,

    /// Layer filter that was applied.
    pub layer: String,

    /// Number of results found.
    pub result_count: usize,

    /// Matching dictionary entries.
    pub results: Vec<DictionaryResult>,
}

/// A single dictionary search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryResult {
    /// Layer: "bronze" or "silver".
    pub layer: String,

    /// Entity name (stream_id for Bronze, table_name for Silver).
    pub entity: String,

    /// Column name.
    pub column_name: String,

    /// Data type.
    pub data_type: String,

    /// Unit of measurement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Execute the query_dictionary tool.
///
/// # Arguments
///
/// * `dictionary` - Dictionary store for metadata lookup
/// * `args` - Tool arguments as JSON value
///
/// # Returns
///
/// MCP tool result with matching dictionary entries
pub async fn execute<D>(dictionary: &D, args: serde_json::Value) -> McpResult<McpToolResult>
where
    D: DictionaryStore + ?Sized,
{
    // Parse arguments
    let parsed_args: QueryDictionaryArgs =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    // Validate required query parameter
    let query = parsed_args
        .query
        .ok_or_else(|| McpError::InvalidParams("Missing required parameter: query".to_string()))?;

    if query.trim().is_empty() {
        return Err(McpError::InvalidParams(
            "Parameter 'query' cannot be empty".to_string(),
        ));
    }

    // Validate layer parameter
    let layer = parsed_args.layer.to_lowercase();
    let layer_filter = match layer.as_str() {
        "all" => None,
        "bronze" | "silver" => Some(layer.clone()),
        _ => {
            return Err(McpError::InvalidParams(format!(
                "Invalid layer '{}'. Must be 'bronze', 'silver', or 'all'",
                layer
            )))
        }
    };

    // Search the dictionary
    let entries = dictionary.search(&query, layer_filter).await?;

    // Build response
    let results: Vec<DictionaryResult> = entries
        .into_iter()
        .map(|entry| DictionaryResult {
            layer: entry.layer,
            entity: entry.entity,
            column_name: entry.column_name,
            data_type: entry.data_type,
            unit: entry.unit,
            description: entry.description,
        })
        .collect();

    let response = QueryDictionaryResponse {
        success: true,
        query,
        layer,
        result_count: results.len(),
        results,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DictionaryEntry, MockDictionaryStore};
    use serde_json::json;

    #[tokio::test]
    async fn test_query_dictionary_success() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .with(
                mockall::predicate::eq("temperature"),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DictionaryEntry::new("bronze", "air-quality", "temperature", "number")
                        .with_unit("celsius"),
                    DictionaryEntry::new(
                        "silver",
                        "air_quality_readings",
                        "temperature_c",
                        "DOUBLE PRECISION",
                    )
                    .with_unit("celsius")
                    .with_description("Temperature in Celsius"),
                ])
            });

        let args = json!({
            "query": "temperature"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: QueryDictionaryResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.query, "temperature");
        assert_eq!(response.layer, "all");
        assert_eq!(response.result_count, 2);
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].layer, "bronze");
        assert_eq!(response.results[1].layer, "silver");
    }

    #[tokio::test]
    async fn test_query_dictionary_with_layer_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .with(
                mockall::predicate::eq("pm25"),
                mockall::predicate::eq(Some("silver".to_string())),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![DictionaryEntry::new(
                    "silver",
                    "air_quality_readings",
                    "pm25",
                    "DOUBLE PRECISION",
                )
                .with_unit("ug/m3")])
            });

        let args = json!({
            "query": "pm25",
            "layer": "silver"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: QueryDictionaryResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.layer, "silver");
        assert_eq!(response.result_count, 1);
        assert_eq!(response.results[0].layer, "silver");
    }

    #[tokio::test]
    async fn test_query_dictionary_empty_results() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .with(
                mockall::predicate::eq("nonexistent"),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| Ok(vec![]));

        let args = json!({
            "query": "nonexistent"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: QueryDictionaryResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.result_count, 0);
        assert!(response.results.is_empty());
    }

    #[tokio::test]
    async fn test_query_dictionary_error_propagation() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search().times(1).returning(|_, _| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let args = json!({
            "query": "test"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_query_dictionary_missing_query_parameter() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "layer": "silver"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("query"));
    }

    #[tokio::test]
    async fn test_query_dictionary_empty_query() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "query": "   "
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_query_dictionary_invalid_layer() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "query": "test",
            "layer": "gold"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("gold"));
    }

    #[tokio::test]
    async fn test_query_dictionary_default_layer() {
        let mut mock = MockDictionaryStore::new();

        // When no layer is specified, it should default to "all" (None filter)
        mock.expect_search()
            .with(
                mockall::predicate::eq("test"),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| Ok(vec![]));

        let args = json!({
            "query": "test"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: QueryDictionaryResponse = serde_json::from_str(text).unwrap();

        assert_eq!(response.layer, "all");
    }

    #[tokio::test]
    async fn test_query_dictionary_layer_case_insensitive() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .with(
                mockall::predicate::eq("test"),
                mockall::predicate::eq(Some("bronze".to_string())),
            )
            .times(1)
            .returning(|_, _| Ok(vec![]));

        let args = json!({
            "query": "test",
            "layer": "BRONZE"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: QueryDictionaryResponse = serde_json::from_str(text).unwrap();

        assert_eq!(response.layer, "bronze");
    }
}
