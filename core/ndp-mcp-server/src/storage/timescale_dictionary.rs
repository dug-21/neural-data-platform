//! TimescaleDB adapter for the DictionaryStore trait (dp-010).
//!
//! Implements `DictionaryStore` for accessing the NDP data dictionary stored
//! in TimescaleDB's `data_dictionary` schema. Uses bb8 connection pooling for
//! efficient database access.
//!
//! # Database Tables Used
//!
//! - `data_dictionary.v_complete_dictionary` - Unified view of Bronze/Silver columns
//! - `data_dictionary.silver_tables` - Silver table metadata
//! - `data_dictionary.silver_columns` - Silver column definitions
//! - `data_dictionary.silver_lineage` - Bronze->Silver field mappings
//! - `data_dictionary.silver_dq_rules` - DQ rules per column
//! - `data_dictionary.fields` - Bronze field definitions
//!
//! # Example
//!
//! ```ignore
//! use ndp_mcp_server::storage::TimescaleDictionaryStore;
//!
//! let store = TimescaleDictionaryStore::new("postgresql://user:pass@host/db").await?;
//! let results = store.search("temperature", Some("silver".to_string())).await?;
//! ```

use async_trait::async_trait;
use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres::NoTls;
use tracing::{debug, instrument};

use super::traits::DictionaryStore;
use super::types::{
    ColumnDescription, DictionaryEntry, DqRuleInfo, LineageSource, LineageTrace, SourceInfo,
    ValidationRange,
};
use crate::error::{McpError, McpResult};

/// Type alias for the PostgreSQL connection pool.
pub type PgPool = Pool<PostgresConnectionManager<NoTls>>;

/// Type alias for a pooled PostgreSQL connection.
pub type PgConnection<'a> = PooledConnection<'a, PostgresConnectionManager<NoTls>>;

/// TimescaleDB adapter for the DictionaryStore trait.
///
/// Provides access to the NDP data dictionary stored in TimescaleDB.
/// Uses bb8 connection pooling for efficient concurrent access.
///
/// # Construction
///
/// Use `TimescaleDictionaryStore::new()` with a PostgreSQL connection string
/// or `TimescaleDictionaryStore::from_pool()` with an existing connection pool.
pub struct TimescaleDictionaryStore {
    pool: PgPool,
}

impl TimescaleDictionaryStore {
    /// Create a new TimescaleDictionaryStore with connection pooling.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string
    ///
    /// # Pool Configuration
    ///
    /// - max_size: 2 (following dp-011 hybrid connection pattern for Pi resources)
    /// - min_idle: 1
    /// - connection_timeout: 5 seconds
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if the connection pool cannot be created.
    pub async fn new(database_url: &str) -> McpResult<Self> {
        let manager = PostgresConnectionManager::new_from_stringlike(database_url, NoTls)
            .map_err(|e| McpError::StorageError(format!("Invalid database URL: {}", e)))?;

        let pool = Pool::builder()
            .max_size(2)
            .min_idle(Some(1))
            .connection_timeout(std::time::Duration::from_secs(5))
            .build(manager)
            .await
            .map_err(|e| McpError::StorageError(format!("Failed to create pool: {}", e)))?;

        Ok(Self { pool })
    }

    /// Create a TimescaleDictionaryStore from an existing connection pool.
    ///
    /// Useful for sharing a pool across multiple storage adapters.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a connection from the pool.
    async fn get_conn(&self) -> McpResult<PgConnection<'_>> {
        self.pool
            .get()
            .await
            .map_err(|e| McpError::StorageError(format!("Connection pool error: {}", e)))
    }

    /// Check if an entity is a Silver table.
    /// Handles both qualified (silver.table) and unqualified (table) names.
    async fn is_silver_table(&self, conn: &PgConnection<'_>, name: &str) -> McpResult<bool> {
        // Strip silver. prefix if present for normalized lookup
        let normalized = name.strip_prefix("silver.").unwrap_or(name);
        let qualified = format!("silver.{}", normalized);

        let row = conn
            .query_opt(
                "SELECT EXISTS(SELECT 1 FROM data_dictionary.silver_tables WHERE table_name = $1 OR table_name = $2) AS is_silver",
                &[&name, &qualified],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Database query error: {}", e)))?;

        match row {
            Some(r) => {
                let is_silver: bool = r.get("is_silver");
                Ok(is_silver)
            }
            None => Ok(false),
        }
    }

    /// Check if an entity is a Bronze stream.
    async fn is_bronze_stream(&self, conn: &PgConnection<'_>, name: &str) -> McpResult<bool> {
        let row = conn
            .query_opt(
                "SELECT EXISTS(SELECT 1 FROM data_dictionary.streams WHERE stream_id = $1) AS is_bronze",
                &[&name],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Database query error: {}", e)))?;

        match row {
            Some(r) => {
                let is_bronze: bool = r.get("is_bronze");
                Ok(is_bronze)
            }
            None => Ok(false),
        }
    }
}

#[async_trait]
impl DictionaryStore for TimescaleDictionaryStore {
    /// Search for columns matching a query across Bronze and Silver layers.
    ///
    /// Uses ILIKE for case-insensitive partial matching on column names
    /// and descriptions. Results are limited to 50 entries.
    ///
    /// # SQL Query
    ///
    /// Queries `data_dictionary.v_complete_dictionary` with optional layer filter.
    #[instrument(skip(self), fields(query, layer))]
    async fn search(&self, query: &str, layer: Option<String>) -> McpResult<Vec<DictionaryEntry>> {
        if query.is_empty() {
            return Err(McpError::InvalidRequest(
                "query cannot be empty".to_string(),
            ));
        }

        let conn = self.get_conn().await?;
        let layer_filter = layer.as_deref().unwrap_or("all");

        debug!(query = %query, layer = %layer_filter, "Searching data dictionary");

        let rows = conn
            .query(
                r#"
                SELECT
                    layer,
                    entity,
                    column_name,
                    data_type,
                    unit,
                    description
                FROM data_dictionary.v_complete_dictionary
                WHERE
                    ($1 = 'all' OR layer = $1)
                    AND (
                        column_name ILIKE '%' || $2 || '%'
                        OR description ILIKE '%' || $2 || '%'
                    )
                ORDER BY layer, entity, column_name
                LIMIT 50
                "#,
                &[&layer_filter, &query],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Search query failed: {}", e)))?;

        let entries: Vec<DictionaryEntry> = rows
            .iter()
            .map(|row| {
                let mut entry = DictionaryEntry::new(
                    row.get::<_, String>("layer"),
                    row.get::<_, String>("entity"),
                    row.get::<_, String>("column_name"),
                    row.get::<_, String>("data_type"),
                );

                if let Some(unit) = row.get::<_, Option<String>>("unit") {
                    entry = entry.with_unit(unit);
                }
                if let Some(desc) = row.get::<_, Option<String>>("description") {
                    entry = entry.with_description(desc);
                }

                entry
            })
            .collect();

        debug!(count = entries.len(), "Search returned results");
        Ok(entries)
    }

    /// Get detailed information about a specific column.
    ///
    /// First determines if the entity is a Silver table or Bronze stream,
    /// then queries the appropriate tables for full metadata including
    /// source lineage and DQ rules.
    #[instrument(skip(self), fields(table_or_stream, column_name))]
    async fn describe_column(
        &self,
        table_or_stream: &str,
        column_name: &str,
    ) -> McpResult<ColumnDescription> {
        let conn = self.get_conn().await?;

        // Determine layer
        let is_silver = self.is_silver_table(&conn, table_or_stream).await?;
        let is_bronze = if !is_silver {
            self.is_bronze_stream(&conn, table_or_stream).await?
        } else {
            false
        };

        if !is_silver && !is_bronze {
            return Err(McpError::StreamNotFound(format!(
                "'{}' not found as Silver table or Bronze stream",
                table_or_stream
            )));
        }

        if is_silver {
            self.describe_silver_column(&conn, table_or_stream, column_name)
                .await
        } else {
            self.describe_bronze_field(&conn, table_or_stream, column_name)
                .await
        }
    }

    /// Trace lineage from Silver column back to Bronze source(s).
    ///
    /// Returns the complete lineage chain showing how a Silver column
    /// maps back to Bronze source fields, including transformations.
    #[instrument(skip(self), fields(silver_table, silver_column))]
    async fn trace_lineage(
        &self,
        silver_table: &str,
        silver_column: &str,
    ) -> McpResult<LineageTrace> {
        let conn = self.get_conn().await?;

        // Verify table exists
        let is_silver = self.is_silver_table(&conn, silver_table).await?;
        if !is_silver {
            return Err(McpError::StreamNotFound(format!(
                "Silver table '{}' not found",
                silver_table
            )));
        }

        // Get Silver column info
        let col_row = conn
            .query_opt(
                r#"
                SELECT data_type, unit
                FROM data_dictionary.silver_columns
                WHERE table_name = $1 AND column_name = $2
                "#,
                &[&silver_table, &silver_column],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Column query failed: {}", e)))?;

        let col_row = col_row.ok_or_else(|| {
            McpError::InvalidRequest(format!(
                "Column '{}' not found in table '{}'",
                silver_column, silver_table
            ))
        })?;

        let silver_type: String = col_row.get("data_type");
        let silver_unit: Option<String> = col_row.get("unit");

        // Get lineage sources
        let lineage_rows = conn
            .query(
                r#"
                SELECT
                    l.source_stream,
                    l.source_path,
                    l.transformation,
                    f.field_type AS bronze_type,
                    f.unit AS bronze_unit
                FROM data_dictionary.silver_lineage l
                LEFT JOIN data_dictionary.fields f
                    ON l.source_stream = f.stream_id
                    AND l.source_path = f.field_name
                WHERE l.silver_table = $1
                  AND l.silver_column = $2
                ORDER BY l.source_stream
                "#,
                &[&silver_table, &silver_column],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Lineage query failed: {}", e)))?;

        let lineage: Vec<LineageSource> = lineage_rows
            .iter()
            .map(|row| {
                let mut source = LineageSource::new(
                    row.get::<_, String>("source_stream"),
                    row.get::<_, String>("source_path"),
                );

                if let Some(transformation) = row.get::<_, Option<String>>("transformation") {
                    source = source.with_transformation(transformation);
                }
                if let Some(bronze_type) = row.get::<_, Option<String>>("bronze_type") {
                    source = source.with_bronze_type(bronze_type);
                }
                if let Some(bronze_unit) = row.get::<_, Option<String>>("bronze_unit") {
                    source = source.with_bronze_unit(bronze_unit);
                }

                source
            })
            .collect();

        // Get DQ rules (both column-level and cross-field)
        let dq_rows = conn
            .query(
                r#"
                SELECT
                    rule_name,
                    rule_params,
                    action,
                    CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope,
                    silver_column
                FROM data_dictionary.silver_dq_rules
                WHERE silver_table = $1
                  AND (silver_column = $2 OR silver_column IS NULL)
                ORDER BY rule_name
                "#,
                &[&silver_table, &silver_column],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("DQ rules query failed: {}", e)))?;

        let dq_rules: Vec<DqRuleInfo> = dq_rows
            .iter()
            .map(|row| {
                let mut rule = DqRuleInfo::new(
                    silver_table,
                    row.get::<_, String>("rule_name"),
                    row.get::<_, String>("action"),
                    row.get::<_, String>("scope"),
                );

                if let Some(col) = row.get::<_, Option<String>>("silver_column") {
                    rule = rule.with_silver_column(col);
                }

                let params: serde_json::Value = row.get("rule_params");
                rule = rule.with_rule_params(params);

                rule
            })
            .collect();

        let mut trace = LineageTrace::new(silver_table, silver_column, silver_type)
            .with_lineage(lineage)
            .with_dq_rules(dq_rules);

        if let Some(unit) = silver_unit {
            trace = trace.with_silver_unit(unit);
        }

        debug!(
            table = %silver_table,
            column = %silver_column,
            lineage_count = trace.lineage.len(),
            dq_rules_count = trace.dq_rules.len(),
            "Lineage trace complete"
        );

        Ok(trace)
    }

    /// List DQ rules with optional table and column filters.
    ///
    /// Returns rules ordered by table, then column (with table-level
    /// cross-field rules first), then rule name.
    #[instrument(skip(self), fields(table, column))]
    async fn list_dq_rules(
        &self,
        table: Option<String>,
        column: Option<String>,
    ) -> McpResult<Vec<DqRuleInfo>> {
        // Column filter requires table filter
        if column.is_some() && table.is_none() {
            return Err(McpError::InvalidRequest(
                "column filter requires table filter".to_string(),
            ));
        }

        let conn = self.get_conn().await?;

        debug!(
            table = ?table,
            column = ?column,
            "Listing DQ rules"
        );

        // Normalize table name - data_dictionary stores with 'silver.' prefix
        let qualified_table = table.as_ref().map(|t| {
            if t.starts_with("silver.") {
                t.clone()
            } else {
                format!("silver.{}", t)
            }
        });

        // Build query based on filters
        // Note: Use qualified table name for data_dictionary lookup
        let (query, params): (&str, Vec<&(dyn tokio_postgres::types::ToSql + Sync)>) = match (
            &table,
            &qualified_table,
            &column,
        ) {
            (None, _, None) => (
                r#"
                    SELECT
                        silver_table,
                        silver_column,
                        rule_name,
                        rule_params,
                        action,
                        CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope
                    FROM data_dictionary.silver_dq_rules
                    ORDER BY silver_table,
                             CASE WHEN silver_column IS NULL THEN 1 ELSE 0 END,
                             silver_column,
                             rule_name
                    LIMIT 100
                    "#,
                vec![],
            ),
            (Some(t), Some(qt), None) => (
                r#"
                    SELECT
                        silver_table,
                        silver_column,
                        rule_name,
                        rule_params,
                        action,
                        CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope
                    FROM data_dictionary.silver_dq_rules
                    WHERE silver_table = $1 OR silver_table = $2
                    ORDER BY CASE WHEN silver_column IS NULL THEN 1 ELSE 0 END,
                             silver_column,
                             rule_name
                    LIMIT 100
                    "#,
                vec![t, qt],
            ),
            (Some(t), Some(qt), Some(c)) => (
                r#"
                    SELECT
                        silver_table,
                        silver_column,
                        rule_name,
                        rule_params,
                        action,
                        CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope
                    FROM data_dictionary.silver_dq_rules
                    WHERE (silver_table = $1 OR silver_table = $2)
                      AND (silver_column = $3 OR (silver_column IS NULL AND $3 IS NULL))
                    ORDER BY rule_name
                    LIMIT 100
                    "#,
                vec![t, qt, c],
            ),
            (None, _, Some(_)) => unreachable!(), // Already handled above
            (Some(_), None, _) => unreachable!(), // qualified_table is always Some when table is Some
        };

        let rows = conn
            .query(query, &params)
            .await
            .map_err(|e| McpError::StorageError(format!("DQ rules query failed: {}", e)))?;

        let rules: Vec<DqRuleInfo> = rows
            .iter()
            .map(|row| {
                let mut rule = DqRuleInfo::new(
                    row.get::<_, String>("silver_table"),
                    row.get::<_, String>("rule_name"),
                    row.get::<_, String>("action"),
                    row.get::<_, String>("scope"),
                );

                if let Some(col) = row.get::<_, Option<String>>("silver_column") {
                    rule = rule.with_silver_column(col);
                }

                let params: serde_json::Value = row.get("rule_params");
                rule = rule.with_rule_params(params);

                rule
            })
            .collect();

        debug!(count = rules.len(), "DQ rules query complete");
        Ok(rules)
    }
}

impl TimescaleDictionaryStore {
    /// Get detailed information about a Silver column.
    async fn describe_silver_column(
        &self,
        conn: &PgConnection<'_>,
        table_name: &str,
        column_name: &str,
    ) -> McpResult<ColumnDescription> {
        // Query silver_columns with optional lineage join
        let row = conn
            .query_opt(
                r#"
                SELECT
                    sc.data_type,
                    sc.unit,
                    sc.description,
                    sc.nullable,
                    sl.source_stream,
                    sl.source_path,
                    sl.transformation
                FROM data_dictionary.silver_columns sc
                LEFT JOIN data_dictionary.silver_lineage sl
                    ON sc.table_name = sl.silver_table
                   AND sc.column_name = sl.silver_column
                WHERE sc.table_name = $1
                  AND sc.column_name = $2
                "#,
                &[&table_name, &column_name],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Column query failed: {}", e)))?;

        let row = row.ok_or_else(|| {
            McpError::InvalidRequest(format!(
                "Column '{}' not found in table '{}'",
                column_name, table_name
            ))
        })?;

        let mut desc = ColumnDescription::new(
            "silver",
            table_name,
            column_name,
            row.get::<_, String>("data_type"),
        );

        if let Some(unit) = row.get::<_, Option<String>>("unit") {
            desc = desc.with_unit(unit);
        }
        if let Some(description) = row.get::<_, Option<String>>("description") {
            desc = desc.with_description(description);
        }

        let nullable: bool = row.get("nullable");
        desc = desc.with_nullable(nullable);

        // Add source info if lineage exists
        if let Some(stream) = row.get::<_, Option<String>>("source_stream") {
            let path = row
                .get::<_, Option<String>>("source_path")
                .unwrap_or_default();
            let mut source = SourceInfo::new(stream, path);
            if let Some(transformation) = row.get::<_, Option<String>>("transformation") {
                source = source.with_transformation(transformation);
            }
            desc = desc.with_source(source);
        }

        // Get DQ rules for this column
        let dq_rows = conn
            .query(
                r#"
                SELECT
                    rule_name,
                    rule_params,
                    action,
                    CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope
                FROM data_dictionary.silver_dq_rules
                WHERE silver_table = $1
                  AND (silver_column = $2 OR silver_column IS NULL)
                "#,
                &[&table_name, &column_name],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("DQ rules query failed: {}", e)))?;

        let dq_rules: Vec<DqRuleInfo> = dq_rows
            .iter()
            .map(|row| {
                let mut rule = DqRuleInfo::new(
                    table_name,
                    row.get::<_, String>("rule_name"),
                    row.get::<_, String>("action"),
                    row.get::<_, String>("scope"),
                )
                .with_silver_column(column_name);

                let params: serde_json::Value = row.get("rule_params");
                rule = rule.with_rule_params(params);
                rule
            })
            .collect();

        desc = desc.with_dq_rules(dq_rules);

        // Extract validation range from DQ rules if present
        for rule in &desc.dq_rules {
            if rule.rule_name == "range_check" {
                if let Some(params) = rule.rule_params.as_object() {
                    let min = params.get("min").and_then(|v| v.as_f64());
                    let max = params.get("max").and_then(|v| v.as_f64());
                    if min.is_some() || max.is_some() {
                        desc = desc.with_validation_range(ValidationRange::new(min, max));
                        break;
                    }
                }
            }
        }

        Ok(desc)
    }

    /// Get detailed information about a Bronze field.
    async fn describe_bronze_field(
        &self,
        conn: &PgConnection<'_>,
        stream_id: &str,
        field_name: &str,
    ) -> McpResult<ColumnDescription> {
        let row = conn
            .query_opt(
                r#"
                SELECT
                    field_type AS data_type,
                    unit,
                    description,
                    nullable,
                    validation_min,
                    validation_max
                FROM data_dictionary.fields
                WHERE stream_id = $1
                  AND field_name = $2
                "#,
                &[&stream_id, &field_name],
            )
            .await
            .map_err(|e| McpError::StorageError(format!("Field query failed: {}", e)))?;

        let row = row.ok_or_else(|| {
            McpError::InvalidRequest(format!(
                "Field '{}' not found in stream '{}'",
                field_name, stream_id
            ))
        })?;

        let mut desc = ColumnDescription::new(
            "bronze",
            stream_id,
            field_name,
            row.get::<_, String>("data_type"),
        );

        if let Some(unit) = row.get::<_, Option<String>>("unit") {
            desc = desc.with_unit(unit);
        }
        if let Some(description) = row.get::<_, Option<String>>("description") {
            desc = desc.with_description(description);
        }

        let nullable: bool = row.get("nullable");
        desc = desc.with_nullable(nullable);

        // Bronze fields don't have lineage (they're the source)
        // Bronze fields don't have DQ rules (applied at Silver)

        // Add validation range if present
        let val_min: Option<f64> = row.get("validation_min");
        let val_max: Option<f64> = row.get("validation_max");
        if val_min.is_some() || val_max.is_some() {
            desc = desc.with_validation_range(ValidationRange::new(val_min, val_max));
        }

        Ok(desc)
    }
}

// ============================================================================
// TESTS - London TDD (Behavior Verification)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::traits::MockDictionaryStore;
    use serde_json::json;

    // ========== SEARCH TESTS ==========

    #[tokio::test]
    async fn test_search_returns_matching_entries() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .withf(|query, layer| query == "temperature" && layer.is_none())
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DictionaryEntry::new(
                        "silver",
                        "weather_observations",
                        "temperature_c",
                        "DOUBLE PRECISION",
                    )
                    .with_unit("Celsius")
                    .with_description("Ambient air temperature"),
                    DictionaryEntry::new("bronze", "outdoor-weather", "temperature", "float")
                        .with_unit("celsius"),
                ])
            });

        let results = mock.search("temperature", None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].layer, "silver");
        assert_eq!(results[0].column_name, "temperature_c");
        assert_eq!(results[1].layer, "bronze");
    }

    #[tokio::test]
    async fn test_search_with_silver_layer_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .withf(|query, layer| query == "pm25" && layer.as_deref() == Some("silver"))
            .times(1)
            .returning(|_, _| {
                Ok(vec![DictionaryEntry::new(
                    "silver",
                    "air_quality_observations",
                    "pm25",
                    "DOUBLE PRECISION",
                )
                .with_unit("ug/m3")])
            });

        let results = mock
            .search("pm25", Some("silver".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].layer, "silver");
    }

    #[tokio::test]
    async fn test_search_returns_empty_for_no_matches() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search().times(1).returning(|_, _| Ok(vec![]));

        let results = mock.search("nonexistent", None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_search_rejects_empty_query() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search()
            .withf(|query, _| query.is_empty())
            .times(1)
            .returning(|_, _| {
                Err(McpError::InvalidRequest(
                    "query cannot be empty".to_string(),
                ))
            });

        let result = mock.search("", None).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidRequest(_)));
    }

    // ========== DESCRIBE_COLUMN TESTS ==========

    #[tokio::test]
    async fn test_describe_silver_column_returns_full_metadata() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .withf(|table, col| table == "air_quality_observations" && col == "pm25")
            .times(1)
            .returning(|_, _| {
                Ok(ColumnDescription::new(
                    "silver",
                    "air_quality_observations",
                    "pm25",
                    "DOUBLE PRECISION",
                )
                .with_unit("ug/m3")
                .with_description("PM2.5 particulate matter concentration (humidity-compensated)")
                .with_nullable(false)
                .with_source(
                    SourceInfo::new("air-quality", "raw_payload.pm02Compensated")
                        .with_transformation("direct"),
                )
                .with_dq_rules(vec![DqRuleInfo::new(
                    "air_quality_observations",
                    "range_check",
                    "flag",
                    "column",
                )
                .with_silver_column("pm25")
                .with_rule_params(json!({"min": 0.0, "max": 1000.0}))])
                .with_validation_range(ValidationRange::bounded(0.0, 1000.0)))
            });

        let result = mock
            .describe_column("air_quality_observations", "pm25")
            .await
            .unwrap();

        assert_eq!(result.layer, "silver");
        assert_eq!(result.column_name, "pm25");
        assert_eq!(result.data_type, "DOUBLE PRECISION");
        assert!(result.source.is_some());
        assert!(!result.dq_rules.is_empty());
        assert!(result.validation_range.is_some());
    }

    #[tokio::test]
    async fn test_describe_bronze_field_returns_metadata() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .withf(|table, col| table == "air-quality" && col == "temperature")
            .times(1)
            .returning(|_, _| {
                Ok(
                    ColumnDescription::new("bronze", "air-quality", "temperature", "float")
                        .with_unit("celsius")
                        .with_description("Ambient temperature")
                        .with_nullable(true)
                        .with_validation_range(ValidationRange::bounded(-40.0, 85.0)),
                )
            });

        let result = mock
            .describe_column("air-quality", "temperature")
            .await
            .unwrap();

        assert_eq!(result.layer, "bronze");
        assert!(result.source.is_none()); // Bronze has no source
        assert!(result.dq_rules.is_empty()); // Bronze has no DQ rules
    }

    #[tokio::test]
    async fn test_describe_column_table_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .times(1)
            .returning(|table, _| {
                Err(McpError::StreamNotFound(format!(
                    "'{}' not found as Silver table or Bronze stream",
                    table
                )))
            });

        let result = mock
            .describe_column("nonexistent_table", "any_column")
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_describe_column_column_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .times(1)
            .returning(|table, col| {
                Err(McpError::InvalidRequest(format!(
                    "Column '{}' not found in table '{}'",
                    col, table
                )))
            });

        let result = mock
            .describe_column("air_quality_observations", "nonexistent")
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidRequest(_)));
    }

    // ========== TRACE_LINEAGE TESTS ==========

    #[tokio::test]
    async fn test_trace_lineage_single_source() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
            .withf(|table, col| table == "air_quality_observations" && col == "pm25")
            .times(1)
            .returning(|_, _| {
                Ok(
                    LineageTrace::new("air_quality_observations", "pm25", "DOUBLE PRECISION")
                        .with_silver_unit("ug/m3")
                        .with_lineage(vec![LineageSource::new(
                            "air-quality",
                            "raw_payload.pm02Compensated",
                        )
                        .with_transformation("direct")
                        .with_bronze_type("float")
                        .with_bronze_unit("ug/m3")])
                        .with_dq_rules(vec![DqRuleInfo::new(
                            "air_quality_observations",
                            "range_check",
                            "flag",
                            "column",
                        )
                        .with_silver_column("pm25")
                        .with_rule_params(json!({"min": 0.0, "max": 1000.0}))]),
                )
            });

        let trace = mock
            .trace_lineage("air_quality_observations", "pm25")
            .await
            .unwrap();

        assert_eq!(trace.silver_table, "air_quality_observations");
        assert_eq!(trace.silver_column, "pm25");
        assert_eq!(trace.lineage.len(), 1);
        assert_eq!(trace.lineage[0].source_stream, "air-quality");
        assert!(!trace.dq_rules.is_empty());
    }

    #[tokio::test]
    async fn test_trace_lineage_multi_source() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
            .withf(|table, col| table == "weather_observations" && col == "temperature_c")
            .times(1)
            .returning(|_, _| {
                Ok(
                    LineageTrace::new("weather_observations", "temperature_c", "DOUBLE PRECISION")
                        .with_silver_unit("Celsius")
                        .with_lineage(vec![
                            LineageSource::new("outdoor-weather", "raw_payload.main.temp")
                                .with_transformation("direct")
                                .with_bronze_type("float"),
                            LineageSource::new("nws-observations", "raw_payload.temperature.value")
                                .with_transformation("direct")
                                .with_bronze_type("float"),
                        ]),
                )
            });

        let trace = mock
            .trace_lineage("weather_observations", "temperature_c")
            .await
            .unwrap();

        assert_eq!(trace.lineage.len(), 2);
        assert_eq!(trace.lineage[0].source_stream, "outdoor-weather");
        assert_eq!(trace.lineage[1].source_stream, "nws-observations");
    }

    #[tokio::test]
    async fn test_trace_lineage_table_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage().times(1).returning(|table, _| {
            Err(McpError::StreamNotFound(format!(
                "Silver table '{}' not found",
                table
            )))
        });

        let result = mock.trace_lineage("nonexistent_table", "column").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
    }

    #[tokio::test]
    async fn test_trace_lineage_column_not_found() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage()
            .times(1)
            .returning(|table, col| {
                Err(McpError::InvalidRequest(format!(
                    "Column '{}' not found in table '{}'",
                    col, table
                )))
            });

        let result = mock
            .trace_lineage("air_quality_observations", "nonexistent")
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidRequest(_)));
    }

    // ========== LIST_DQ_RULES TESTS ==========

    #[tokio::test]
    async fn test_list_dq_rules_all() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .withf(|table, column| table.is_none() && column.is_none())
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DqRuleInfo::new("air_quality_observations", "range_check", "flag", "column")
                        .with_silver_column("pm25")
                        .with_rule_params(json!({"min": 0.0, "max": 1000.0})),
                    DqRuleInfo::new("air_quality_observations", "range_check", "flag", "column")
                        .with_silver_column("pm10")
                        .with_rule_params(json!({"min": 0.0, "max": 2000.0})),
                    DqRuleInfo::new("weather_observations", "range_check", "flag", "column")
                        .with_silver_column("temperature_c")
                        .with_rule_params(json!({"min": -60.0, "max": 60.0})),
                ])
            });

        let rules = mock.list_dq_rules(None, None).await.unwrap();

        assert_eq!(rules.len(), 3);
    }

    #[tokio::test]
    async fn test_list_dq_rules_with_table_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .withf(|table, column| {
                table.as_deref() == Some("air_quality_observations") && column.is_none()
            })
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    DqRuleInfo::new("air_quality_observations", "range_check", "flag", "column")
                        .with_silver_column("pm25"),
                    DqRuleInfo::new(
                        "air_quality_observations",
                        "cross_field_check",
                        "flag",
                        "cross-field",
                    ),
                ])
            });

        let rules = mock
            .list_dq_rules(Some("air_quality_observations".to_string()), None)
            .await
            .unwrap();

        assert_eq!(rules.len(), 2);
        assert!(rules
            .iter()
            .all(|r| r.silver_table == "air_quality_observations"));
    }

    #[tokio::test]
    async fn test_list_dq_rules_with_column_filter() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .withf(|table, column| {
                table.as_deref() == Some("air_quality_observations")
                    && column.as_deref() == Some("pm25")
            })
            .times(1)
            .returning(|_, _| {
                Ok(vec![DqRuleInfo::new(
                    "air_quality_observations",
                    "range_check",
                    "flag",
                    "column",
                )
                .with_silver_column("pm25")
                .with_rule_params(json!({"min": 0.0, "max": 1000.0}))])
            });

        let rules = mock
            .list_dq_rules(
                Some("air_quality_observations".to_string()),
                Some("pm25".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].silver_column, Some("pm25".to_string()));
    }

    #[tokio::test]
    async fn test_list_dq_rules_column_without_table_error() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .withf(|table, column| table.is_none() && column.is_some())
            .times(1)
            .returning(|_, _| {
                Err(McpError::InvalidRequest(
                    "column filter requires table filter".to_string(),
                ))
            });

        let result = mock.list_dq_rules(None, Some("pm25".to_string())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn test_list_dq_rules_includes_cross_field() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules().times(1).returning(|_, _| {
            Ok(vec![
                DqRuleInfo::new("air_quality_observations", "range_check", "flag", "column")
                    .with_silver_column("pm25"),
                DqRuleInfo::new(
                    "air_quality_observations",
                    "pm10_gte_pm25",
                    "flag",
                    "cross-field",
                )
                .with_rule_params(json!({
                    "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25",
                    "message": "pm10_less_than_pm25"
                })),
            ])
        });

        let rules = mock
            .list_dq_rules(Some("air_quality_observations".to_string()), None)
            .await
            .unwrap();

        assert!(rules.iter().any(|r| r.scope == "cross-field"));
        assert!(rules.iter().any(|r| r.silver_column.is_none()));
    }

    // ========== WORKFLOW TESTS ==========

    #[tokio::test]
    async fn test_dictionary_workflow_search_then_describe_then_lineage() {
        let mut mock = MockDictionaryStore::new();
        let mut seq = mockall::Sequence::new();

        // Step 1: Search for columns
        mock.expect_search()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                Ok(vec![DictionaryEntry::new(
                    "silver",
                    "air_quality_observations",
                    "pm25",
                    "DOUBLE PRECISION",
                )])
            });

        // Step 2: Describe the found column
        mock.expect_describe_column()
            .with(
                mockall::predicate::eq("air_quality_observations"),
                mockall::predicate::eq("pm25"),
            )
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                Ok(ColumnDescription::new(
                    "silver",
                    "air_quality_observations",
                    "pm25",
                    "DOUBLE PRECISION",
                )
                .with_source(SourceInfo::new("air-quality", "raw_payload.pm25")))
            });

        // Step 3: Trace lineage
        mock.expect_trace_lineage()
            .with(
                mockall::predicate::eq("air_quality_observations"),
                mockall::predicate::eq("pm25"),
            )
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| {
                Ok(
                    LineageTrace::new("air_quality_observations", "pm25", "DOUBLE PRECISION")
                        .with_lineage(vec![LineageSource::new("air-quality", "raw_payload.pm25")]),
                )
            });

        // Execute workflow
        let search_results = mock.search("pm25", None).await.unwrap();
        assert_eq!(search_results.len(), 1);

        let description = mock
            .describe_column("air_quality_observations", "pm25")
            .await
            .unwrap();
        assert!(description.source.is_some());

        let lineage = mock
            .trace_lineage("air_quality_observations", "pm25")
            .await
            .unwrap();
        assert!(!lineage.lineage.is_empty());
    }

    // ========== ERROR HANDLING TESTS ==========

    #[tokio::test]
    async fn test_search_propagates_storage_error() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_search().times(1).returning(|_, _| {
            Err(McpError::StorageError(
                "Database connection failed".to_string(),
            ))
        });

        let result = mock.search("test", None).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_describe_column_propagates_storage_error() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_describe_column()
            .times(1)
            .returning(|_, _| Err(McpError::StorageError("Query timeout".to_string())));

        let result = mock.describe_column("table", "column").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_trace_lineage_propagates_storage_error() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_trace_lineage().times(1).returning(|_, _| {
            Err(McpError::StorageError(
                "Connection pool exhausted".to_string(),
            ))
        });

        let result = mock.trace_lineage("table", "column").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_list_dq_rules_propagates_storage_error() {
        let mut mock = MockDictionaryStore::new();

        mock.expect_list_dq_rules()
            .times(1)
            .returning(|_, _| Err(McpError::StorageError("Database unavailable".to_string())));

        let result = mock.list_dq_rules(None, None).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }
}
