//! list_dq_rules Tool Implementation
//!
//! List data quality rules applied to Silver tables/columns.
//!
//! # Response Format
//!
//! ```json
//! {
//!   "success": true,
//!   "table": "air_quality_observations",
//!   "column": null,
//!   "rule_count": 5,
//!   "rules": [
//!     {
//!       "silver_table": "air_quality_observations",
//!       "silver_column": "pm25",
//!       "rule_name": "range_check",
//!       "rule_params": {"min": 0, "max": 500},
//!       "action": "flag",
//!       "scope": "column"
//!     },
//!     {
//!       "silver_table": "air_quality_observations",
//!       "silver_column": null,
//!       "rule_name": "completeness_check",
//!       "rule_params": {"min_rows": 100},
//!       "action": "warn",
//!       "scope": "cross-field"
//!     }
//!   ]
//! }
//! ```
//!
//! # Arguments
//!
//! - `table` (optional): Filter by Silver table name
//! - `column` (optional): Filter by column name (requires table)
//!
//! # Example Requests
//!
//! Get all DQ rules:
//! ```json
//! {}
//! ```
//!
//! Get rules for a specific table:
//! ```json
//! {
//!   "table": "air_quality_observations"
//! }
//! ```
//!
//! Get rules for a specific column:
//! ```json
//! {
//!   "table": "air_quality_observations",
//!   "column": "pm25"
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{McpError, McpResult};
use crate::mcp::protocol::McpToolResult;
use crate::storage::DictionaryStore;

/// Arguments for the list_dq_rules tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListDqRulesArgs {
    /// Filter by Silver table name (optional).
    pub table: Option<String>,

    /// Filter by column name (optional, requires table).
    pub column: Option<String>,
}

/// Response structure for list_dq_rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDqRulesResponse {
    /// Success flag.
    pub success: bool,

    /// Table filter that was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,

    /// Column filter that was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,

    /// Number of rules found.
    pub rule_count: usize,

    /// Matching DQ rules.
    pub rules: Vec<DqRuleResult>,
}

/// A single DQ rule in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DqRuleResult {
    /// Silver table this rule applies to.
    pub silver_table: String,

    /// Column this rule applies to (None for table-level/cross-field rules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silver_column: Option<String>,

    /// Rule name (e.g., "range_check", "not_null").
    pub rule_name: String,

    /// Rule parameters as JSON.
    pub rule_params: serde_json::Value,

    /// Action on rule violation: "flag", "reject", "warn".
    pub action: String,

    /// Scope: "column" for column-level rules, "cross-field" for table-level rules.
    pub scope: String,
}

/// Execute the list_dq_rules tool.
///
/// # Arguments
///
/// * `dictionary` - Dictionary store for metadata lookup
/// * `args` - Tool arguments as JSON value
///
/// # Returns
///
/// MCP tool result with DQ rules
pub async fn execute<D>(dictionary: &D, args: serde_json::Value) -> McpResult<McpToolResult>
where
    D: DictionaryStore + ?Sized,
{
    // Parse arguments
    let parsed_args: ListDqRulesArgs =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    // Validate: column requires table
    if parsed_args.column.is_some() && parsed_args.table.is_none() {
        return Err(McpError::InvalidParams(
            "Parameter 'column' requires 'table' to be specified".to_string(),
        ));
    }

    // Validate non-empty strings
    let table = match &parsed_args.table {
        Some(t) if t.trim().is_empty() => {
            return Err(McpError::InvalidParams(
                "Parameter 'table' cannot be empty".to_string(),
            ))
        }
        Some(t) => Some(t.clone()),
        None => None,
    };

    let column = match &parsed_args.column {
        Some(c) if c.trim().is_empty() => {
            return Err(McpError::InvalidParams(
                "Parameter 'column' cannot be empty".to_string(),
            ))
        }
        Some(c) => Some(c.clone()),
        None => None,
    };

    // Get DQ rules from dictionary
    let rules = dictionary.list_dq_rules(table.clone(), column.clone()).await?;

    // Build response with correct scope mapping
    let rule_results: Vec<DqRuleResult> = rules
        .into_iter()
        .map(|rule| {
            // Determine scope: if silver_column is None, it's a cross-field rule
            let scope = if rule.silver_column.is_none() {
                "cross-field".to_string()
            } else {
                rule.scope
            };

            DqRuleResult {
                silver_table: rule.silver_table,
                silver_column: rule.silver_column,
                rule_name: rule.rule_name,
                rule_params: rule.rule_params,
                action: rule.action,
                scope,
            }
        })
        .collect();

    let response = ListDqRulesResponse {
        success: true,
        table,
        column,
        rule_count: rule_results.len(),
        rules: rule_results,
    };

    McpToolResult::success(&response)
        .map_err(|e| McpError::Internal(format!("Serialization error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DqRuleInfo, MockDictionaryStore};
    use serde_json::json;

    #[tokio::test]
    async fn test_list_dq_rules_all() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(None::<String>),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DqRuleInfo::new("air_quality_readings", "range_check", "flag", "column")
                        .with_silver_column("pm25")
                        .with_rule_params(json!({"min": 0, "max": 500})),
                    DqRuleInfo::new("air_quality_readings", "not_null", "reject", "column")
                        .with_silver_column("timestamp"),
                    DqRuleInfo::new(
                        "outdoor_weather_readings",
                        "range_check",
                        "flag",
                        "column",
                    )
                    .with_silver_column("temperature"),
                ])
            });

        let args = json!({});

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: ListDqRulesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert!(response.table.is_none());
        assert!(response.column.is_none());
        assert_eq!(response.rule_count, 3);
        assert_eq!(response.rules.len(), 3);
    }

    #[tokio::test]
    async fn test_list_dq_rules_by_table() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(Some("air_quality_readings".to_string())),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DqRuleInfo::new("air_quality_readings", "range_check", "flag", "column")
                        .with_silver_column("pm25"),
                    DqRuleInfo::new("air_quality_readings", "not_null", "reject", "column")
                        .with_silver_column("timestamp"),
                ])
            });

        let args = json!({
            "table": "air_quality_readings"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: ListDqRulesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.table, Some("air_quality_readings".to_string()));
        assert!(response.column.is_none());
        assert_eq!(response.rule_count, 2);
        assert!(response
            .rules
            .iter()
            .all(|r| r.silver_table == "air_quality_readings"));
    }

    #[tokio::test]
    async fn test_list_dq_rules_by_table_and_column() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(Some("air_quality_readings".to_string())),
                mockall::predicate::eq(Some("pm25".to_string())),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![DqRuleInfo::new(
                    "air_quality_readings",
                    "range_check",
                    "flag",
                    "column",
                )
                .with_silver_column("pm25")
                .with_rule_params(json!({"min": 0, "max": 500}))])
            });

        let args = json!({
            "table": "air_quality_readings",
            "column": "pm25"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: ListDqRulesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.table, Some("air_quality_readings".to_string()));
        assert_eq!(response.column, Some("pm25".to_string()));
        assert_eq!(response.rule_count, 1);
        assert_eq!(
            response.rules[0].silver_column,
            Some("pm25".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_dq_rules_empty_results() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(Some("empty_table".to_string())),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| Ok(vec![]));

        let args = json!({
            "table": "empty_table"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: ListDqRulesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.rule_count, 0);
        assert!(response.rules.is_empty());
    }

    #[tokio::test]
    async fn test_list_dq_rules_cross_field_scope() {
        let mut mock = MockDictionaryStore::new();

        // When silver_column is NULL, scope should be "cross-field"
        mock.expect_list_dq_rules()
            .with(
                mockall::predicate::eq(Some("air_quality_readings".to_string())),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    // Column-level rule
                    DqRuleInfo::new("air_quality_readings", "range_check", "flag", "column")
                        .with_silver_column("pm25"),
                    // Table-level rule (no column, but storage might return "table" scope)
                    DqRuleInfo::new(
                        "air_quality_readings",
                        "completeness_check",
                        "warn",
                        "table",
                    )
                    .with_rule_params(json!({"min_rows": 100})),
                    // This simulates storage returning scope "table" but we should convert to "cross-field"
                ])
            });

        let args = json!({
            "table": "air_quality_readings"
        });

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: ListDqRulesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.rule_count, 2);

        // Column-level rule should have scope "column"
        let column_rule = response
            .rules
            .iter()
            .find(|r| r.silver_column.is_some())
            .unwrap();
        assert_eq!(column_rule.scope, "column");

        // Table-level rule should have scope "cross-field" (converted from NULL column)
        let table_rule = response
            .rules
            .iter()
            .find(|r| r.silver_column.is_none())
            .unwrap();
        assert_eq!(table_rule.scope, "cross-field");
    }

    #[tokio::test]
    async fn test_list_dq_rules_column_without_table() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "column": "pm25"
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("column"));
        assert!(err.to_string().contains("table"));
    }

    #[tokio::test]
    async fn test_list_dq_rules_empty_table() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "table": "   "
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_list_dq_rules_empty_column() {
        let mock = MockDictionaryStore::new();

        let args = json!({
            "table": "air_quality_readings",
            "column": ""
        });

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::InvalidParams(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn test_list_dq_rules_error_propagation() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules().times(1).returning(|_, _| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let args = json!({});

        let result = execute(&mock, args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_list_dq_rules_rule_params_preserved() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .times(1)
            .returning(|_, _| {
                Ok(vec![DqRuleInfo::new(
                    "air_quality_readings",
                    "range_check",
                    "flag",
                    "column",
                )
                .with_silver_column("pm25")
                .with_rule_params(json!({
                    "min": 0,
                    "max": 500,
                    "unit": "ug/m3"
                }))])
            });

        let args = json!({});

        let result = execute(&mock, args).await.unwrap();
        let text = &result.content[0].text;
        let response: ListDqRulesResponse = serde_json::from_str(text).unwrap();

        assert!(response.success);
        assert_eq!(response.rules[0].rule_params["min"], 0);
        assert_eq!(response.rules[0].rule_params["max"], 500);
        assert_eq!(response.rules[0].rule_params["unit"], "ug/m3");
    }
}
