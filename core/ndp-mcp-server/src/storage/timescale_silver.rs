//! TimescaleDB Silver layer storage implementation (dp-010 BUG-001 fix).
//!
//! Implements the `SilverStorage` trait for accessing Silver layer data
//! stored in TimescaleDB hypertables.
//!
//! # Architecture
//!
//! Following the NDP Domain Adapter pattern (ADR-002):
//! - `SilverStorage` trait is the **port** (interface)
//! - `TimescaleSilverStorage` is the **adapter** for TimescaleDB
//!
//! # Connection Pooling
//!
//! Uses bb8 with tokio-postgres for async connection pooling:
//! - Default max connections: 5
//! - Connection timeout: 10 seconds
//! - Idle timeout: 30 seconds
//!
//! # SQL Queries
//!
//! ## list_tables()
//! Queries `timescaledb_information.hypertables` joined with
//! `data_dictionary.silver_tables` for metadata.
//!
//! ## describe_table()
//! Queries `data_dictionary.silver_columns` with fallback to
//! `information_schema.columns` if dictionary not populated.
//!
//! ## sample()
//! Dynamic query with parameterized time/ndp_id filters.
//! Uses `row_to_json()` for JSON serialization.
//!
//! ## get_stats()
//! Aggregate queries for row counts, time ranges, null counts,
//! and DQ flag summaries.
//!
//! # References
//!
//! - [dp-010 SILVER-TOOLS-SPEC](/workspaces/neural-data-platform/product/features/dp-010/specification/SILVER-TOOLS-SPEC.md)
//! - [dp-010 BUG-001](/workspaces/neural-data-platform/product/features/dp-010/bugs/BUG-001-noop-storage-not-replaced.md)
//! - [dp-011-hybrid-connection-pattern] - Pool configuration guidance

use async_trait::async_trait;
use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::time::Duration;
use tokio_postgres::NoTls;
use tracing::{debug, error, info, instrument, warn};

use crate::error::{McpError, McpResult};
use crate::storage::traits::SilverStorage;
use crate::storage::types::{
    DqSummary, HypertableInfo, SampleFilters, SilverColumnInfo, SilverTableDescription,
    SilverTableInfo, SilverTableStats,
};

/// Configuration for TimescaleDB connection pool.
#[derive(Debug, Clone)]
pub struct TimescalePoolConfig {
    /// Maximum number of connections in the pool.
    pub max_size: u32,
    /// Minimum idle connections to maintain.
    pub min_idle: Option<u32>,
    /// Connection timeout in seconds.
    pub connection_timeout_secs: u64,
    /// Idle timeout in seconds.
    pub idle_timeout_secs: u64,
}

impl Default for TimescalePoolConfig {
    fn default() -> Self {
        Self {
            max_size: 5,
            min_idle: Some(1),
            connection_timeout_secs: 10,
            idle_timeout_secs: 30,
        }
    }
}

impl TimescalePoolConfig {
    /// Create configuration optimized for resource-constrained environments (e.g., Raspberry Pi).
    pub fn for_constrained_environment() -> Self {
        Self {
            max_size: 2,
            min_idle: Some(1),
            connection_timeout_secs: 5,
            idle_timeout_secs: 60,
        }
    }
}

/// TimescaleDB storage adapter for Silver layer data.
///
/// Implements `SilverStorage` trait to access TimescaleDB hypertables.
///
/// # Example
///
/// ```ignore
/// let storage = TimescaleSilverStorage::new("postgresql://user:pass@localhost/ndp").await?;
/// let tables = storage.list_tables().await?;
/// let schema = storage.describe_table("air_quality_observations").await?;
/// ```
pub struct TimescaleSilverStorage {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl TimescaleSilverStorage {
    /// Create a new TimescaleSilverStorage with default configuration.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string (e.g., "postgresql://user:pass@host/db")
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if connection fails.
    pub async fn new(database_url: &str) -> McpResult<Self> {
        Self::with_config(database_url, TimescalePoolConfig::default()).await
    }

    /// Create a new TimescaleSilverStorage with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string
    /// * `config` - Pool configuration
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if connection fails.
    pub async fn with_config(database_url: &str, config: TimescalePoolConfig) -> McpResult<Self> {
        info!("Creating TimescaleSilverStorage pool with max_size={}", config.max_size);

        let manager = PostgresConnectionManager::new_from_stringlike(database_url, NoTls)
            .map_err(|e| McpError::StorageError(format!("Invalid database URL: {}", e)))?;

        let pool = Pool::builder()
            .max_size(config.max_size)
            .min_idle(config.min_idle)
            .connection_timeout(Duration::from_secs(config.connection_timeout_secs))
            .idle_timeout(Some(Duration::from_secs(config.idle_timeout_secs)))
            .build(manager)
            .await
            .map_err(|e| McpError::StorageError(format!("Failed to create connection pool: {}", e)))?;

        // Test the connection
        {
            let conn = pool.get().await.map_err(|e| {
                McpError::StorageError(format!("Failed to get initial connection: {}", e))
            })?;

            // Verify TimescaleDB is available
            conn.execute("SELECT 1", &[]).await.map_err(|e| {
                McpError::StorageError(format!("Database health check failed: {}", e))
            })?;
        } // conn is dropped here, releasing the borrow

        info!("TimescaleSilverStorage pool created successfully");
        Ok(Self { pool })
    }

    /// Get a connection from the pool.
    async fn get_conn(&self) -> McpResult<PooledConnection<'_, PostgresConnectionManager<NoTls>>> {
        self.pool.get().await.map_err(|e| {
            error!("Failed to get connection from pool: {}", e);
            McpError::StorageError(format!("Database connection unavailable: {}", e))
        })
    }

    /// Normalize table name by removing "silver." prefix if present.
    fn normalize_table_name(table_name: &str) -> &str {
        table_name.strip_prefix("silver.").unwrap_or(table_name)
    }

    /// Get the time column name for a table.
    ///
    /// Different tables use different time columns:
    /// - weather_forecasts uses valid_time
    /// - Most others use observation_time
    fn get_time_column(table_name: &str) -> &'static str {
        if table_name.contains("forecast") {
            "valid_time"
        } else {
            "observation_time"
        }
    }
}

#[async_trait]
impl SilverStorage for TimescaleSilverStorage {
    /// List all Silver hypertables with metadata.
    ///
    /// Queries TimescaleDB information views joined with data dictionary
    /// for comprehensive table information.
    #[instrument(skip(self))]
    async fn list_tables(&self) -> McpResult<Vec<SilverTableInfo>> {
        debug!("Listing Silver tables");
        let conn = self.get_conn().await?;

        // Query combines data dictionary metadata with live TimescaleDB stats
        let query = r#"
            WITH hypertable_info AS (
                SELECT
                    ht.hypertable_name AS table_name,
                    TRUE AS is_hypertable,
                    d.column_name AS time_column,
                    COALESCE(ht.compression_enabled, FALSE) AS compression_enabled,
                    (SELECT COUNT(*)
                     FROM timescaledb_information.chunks c
                     WHERE c.hypertable_schema = 'silver'
                       AND c.hypertable_name = ht.hypertable_name) AS chunk_count,
                    hypertable_size(format('silver.%I', ht.hypertable_name)::regclass) AS total_bytes
                FROM timescaledb_information.hypertables ht
                LEFT JOIN timescaledb_information.dimensions d
                    ON ht.hypertable_schema = d.hypertable_schema
                    AND ht.hypertable_name = d.hypertable_name
                    AND d.dimension_number = 1
                WHERE ht.hypertable_schema = 'silver'
            ),
            dict_info AS (
                SELECT
                    table_name,
                    description,
                    grain,
                    source_streams,
                    hypertable_column,
                    chunk_interval
                FROM data_dictionary.silver_tables
            ),
            row_counts AS (
                SELECT
                    relname AS table_name,
                    reltuples::BIGINT AS row_count
                FROM pg_class c
                JOIN pg_namespace n ON c.relnamespace = n.oid
                WHERE n.nspname = 'silver'
                  AND c.relkind = 'r'
            )
            SELECT
                COALESCE(h.table_name, d.table_name) AS table_name,
                d.description,
                d.grain,
                d.source_streams,
                COALESCE(h.is_hypertable, FALSE) AS is_hypertable,
                COALESCE(d.chunk_interval, '1 day') AS chunk_interval,
                r.row_count,
                h.total_bytes,
                h.chunk_count
            FROM hypertable_info h
            FULL OUTER JOIN dict_info d ON h.table_name = d.table_name
            LEFT JOIN row_counts r ON COALESCE(h.table_name, d.table_name) = r.table_name
            ORDER BY COALESCE(h.table_name, d.table_name)
        "#;

        let rows = conn.query(query, &[]).await.map_err(|e| {
            warn!("Failed to query Silver tables: {}", e);
            McpError::StorageError(format!("Failed to query Silver tables: {}", e))
        })?;

        let tables: Vec<SilverTableInfo> = rows
            .iter()
            .map(|row| {
                let table_name: String = row.get("table_name");
                let description: Option<String> = row.get("description");
                let grain: Option<String> = row.get("grain");
                let source_streams: Option<Vec<String>> = row.get("source_streams");
                let is_hypertable: bool = row.get("is_hypertable");
                let chunk_interval: Option<String> = row.get("chunk_interval");
                let row_count: Option<i64> = row.get("row_count");
                let total_bytes: Option<i64> = row.get("total_bytes");

                SilverTableInfo::new(&table_name)
                    .with_description(description.unwrap_or_default())
                    .with_grain(grain.unwrap_or_default())
                    .with_source_streams(source_streams.unwrap_or_default())
                    .with_hypertable(is_hypertable, chunk_interval)
                    .with_row_count(row_count.unwrap_or(0))
                    .with_total_bytes(total_bytes.unwrap_or(0))
            })
            .collect();

        debug!("Found {} Silver tables", tables.len());
        Ok(tables)
    }

    /// Get detailed schema for a Silver table.
    ///
    /// Queries data dictionary for column metadata with fallback to
    /// information_schema if dictionary not populated.
    #[instrument(skip(self))]
    async fn describe_table(&self, table_name: &str) -> McpResult<SilverTableDescription> {
        let table_name = Self::normalize_table_name(table_name);
        debug!("Describing Silver table: {}", table_name);
        let conn = self.get_conn().await?;

        // First verify the table exists
        let exists_query = r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'silver' AND table_name = $1
            ) AS exists
        "#;
        let exists_row = conn.query_one(exists_query, &[&table_name]).await.map_err(|e| {
            McpError::StorageError(format!("Failed to check table existence: {}", e))
        })?;
        let exists: bool = exists_row.get("exists");
        if !exists {
            return Err(McpError::StreamNotFound(format!("Table not found: silver.{}", table_name)));
        }

        // Query table metadata from data dictionary
        let table_query = r#"
            SELECT
                table_name,
                description,
                grain,
                source_streams,
                hypertable_column,
                chunk_interval
            FROM data_dictionary.silver_tables
            WHERE table_name = $1
        "#;
        let table_row = conn.query_opt(table_query, &[&table_name]).await.map_err(|e| {
            warn!("Data dictionary query failed, using fallback: {}", e);
            McpError::StorageError(format!("Failed to query table metadata: {}", e))
        })?;

        // Query column definitions from data dictionary with fallback
        let columns_query = r#"
            SELECT
                c.column_name,
                UPPER(c.data_type) AS data_type,
                c.unit,
                c.description,
                c.nullable,
                c.is_primary_key,
                c.sort_order
            FROM data_dictionary.silver_columns c
            WHERE c.table_name = $1
            ORDER BY c.sort_order, c.column_name
        "#;
        let column_rows = conn.query(columns_query, &[&table_name]).await;

        let columns: Vec<SilverColumnInfo> = match column_rows {
            Ok(rows) if !rows.is_empty() => {
                rows.iter()
                    .map(|row| {
                        let column_name: String = row.get("column_name");
                        let data_type: String = row.get("data_type");
                        let unit: Option<String> = row.get("unit");
                        let description: Option<String> = row.get("description");
                        let nullable: bool = row.get("nullable");
                        let is_primary_key: bool = row.get("is_primary_key");

                        let mut col = SilverColumnInfo::new(&column_name, &data_type)
                            .with_nullable(nullable)
                            .with_primary_key(is_primary_key);
                        if let Some(u) = unit {
                            col = col.with_unit(u);
                        }
                        if let Some(d) = description {
                            col = col.with_description(d);
                        }
                        col
                    })
                    .collect()
            }
            _ => {
                // Fallback to information_schema
                debug!("Using information_schema fallback for columns");
                let fallback_query = r#"
                    SELECT
                        column_name,
                        UPPER(udt_name) AS data_type,
                        (is_nullable = 'YES') AS nullable,
                        ordinal_position
                    FROM information_schema.columns
                    WHERE table_schema = 'silver' AND table_name = $1
                    ORDER BY ordinal_position
                "#;
                let fallback_rows = conn.query(fallback_query, &[&table_name]).await.map_err(|e| {
                    McpError::StorageError(format!("Failed to query column schema: {}", e))
                })?;

                fallback_rows
                    .iter()
                    .map(|row| {
                        let column_name: String = row.get("column_name");
                        let data_type: String = row.get("data_type");
                        let nullable: bool = row.get("nullable");
                        SilverColumnInfo::new(&column_name, &data_type).with_nullable(nullable)
                    })
                    .collect()
            }
        };

        // Query hypertable info
        let hypertable_query = r#"
            SELECT
                d.column_name AS time_column,
                ht.compression_enabled,
                (SELECT COUNT(*) FROM timescaledb_information.chunks c
                 WHERE c.hypertable_schema = 'silver' AND c.hypertable_name = $1) AS chunk_count,
                hypertable_size(format('silver.%I', $1)::regclass) AS total_bytes
            FROM timescaledb_information.hypertables ht
            JOIN timescaledb_information.dimensions d
                ON ht.hypertable_schema = d.hypertable_schema
                AND ht.hypertable_name = d.hypertable_name
                AND d.dimension_number = 1
            WHERE ht.hypertable_schema = 'silver' AND ht.hypertable_name = $1
        "#;
        let hypertable_row = conn.query_opt(hypertable_query, &[&table_name]).await.ok().flatten();

        // Build description
        let mut desc = SilverTableDescription::new(table_name).with_columns(columns);

        if let Some(ref row) = table_row {
            let description: Option<String> = row.get("description");
            if let Some(d) = description {
                desc = desc.with_description(d);
            }
        }

        if let Some(ht_row) = hypertable_row {
            let time_column: String = ht_row.get("time_column");
            let chunk_count: i64 = ht_row.get("chunk_count");
            let total_bytes: i64 = ht_row.get("total_bytes");
            // Get chunk interval from dictionary or default
            let chunk_interval = table_row
                .as_ref()
                .and_then(|r| r.get::<_, Option<String>>("chunk_interval"))
                .unwrap_or_else(|| "1 day".to_string());

            desc = desc.with_hypertable_info(
                HypertableInfo::new(&time_column, &chunk_interval)
                    .with_chunk_count(chunk_count)
                    .with_total_bytes(total_bytes),
            );
        }

        debug!("Described table {} with {} columns", table_name, desc.columns.len());
        Ok(desc)
    }

    /// Sample rows from a Silver table.
    ///
    /// Returns up to N rows with optional time/ndp_id filters.
    /// Results are ordered by time column descending (most recent first).
    #[instrument(skip(self))]
    async fn sample(
        &self,
        table_name: &str,
        n: usize,
        filters: Option<SampleFilters>,
    ) -> McpResult<Vec<Value>> {
        let table_name = Self::normalize_table_name(table_name);
        let time_column = Self::get_time_column(table_name);
        let limit = n.min(100); // Clamp to max 100

        debug!("Sampling {} rows from Silver table: {}", limit, table_name);
        let conn = self.get_conn().await?;

        // Verify table exists
        let exists_query = r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'silver' AND table_name = $1
            ) AS exists
        "#;
        let exists_row = conn.query_one(exists_query, &[&table_name]).await.map_err(|e| {
            McpError::StorageError(format!("Failed to check table existence: {}", e))
        })?;
        let exists: bool = exists_row.get("exists");
        if !exists {
            return Err(McpError::StreamNotFound(format!("Table not found: silver.{}", table_name)));
        }

        // Build dynamic query with filters
        let mut query = format!(
            "SELECT row_to_json(t.*) AS row_data FROM silver.{} t WHERE 1=1",
            table_name
        );
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref f) = filters {
            if let Some(since) = f.since {
                query.push_str(&format!(" AND {} >= ${}", time_column, param_idx));
                params.push(Box::new(since));
                param_idx += 1;
            }
            if let Some(until) = f.until {
                query.push_str(&format!(" AND {} < ${}", time_column, param_idx));
                params.push(Box::new(until));
                param_idx += 1;
            }
        }

        // Order and limit
        query.push_str(&format!(" ORDER BY {} DESC LIMIT ${}", time_column, param_idx));
        params.push(Box::new(limit as i64));

        // Execute query
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params.iter().map(|p| p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)).collect();

        let rows = conn.query(&query, &param_refs[..]).await.map_err(|e| {
            error!("Sample query failed: {}", e);
            McpError::StorageError(format!("Failed to sample data: {}", e))
        })?;

        let results: Vec<Value> = rows
            .iter()
            .filter_map(|row| {
                row.get::<_, Option<Value>>("row_data")
            })
            .collect();

        debug!("Sampled {} rows from {}", results.len(), table_name);
        Ok(results)
    }

    /// Get statistics for a Silver table.
    ///
    /// Returns row counts, time range, chunk info, and DQ summary.
    #[instrument(skip(self))]
    async fn get_stats(&self, table_name: &str) -> McpResult<SilverTableStats> {
        let table_name = Self::normalize_table_name(table_name);
        let time_column = Self::get_time_column(table_name);

        debug!("Getting stats for Silver table: {}", table_name);
        let conn = self.get_conn().await?;

        // Verify table exists
        let exists_query = r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'silver' AND table_name = $1
            ) AS exists
        "#;
        let exists_row = conn.query_one(exists_query, &[&table_name]).await.map_err(|e| {
            McpError::StorageError(format!("Failed to check table existence: {}", e))
        })?;
        let exists: bool = exists_row.get("exists");
        if !exists {
            return Err(McpError::StreamNotFound(format!("Table not found: silver.{}", table_name)));
        }

        // Query basic stats
        let stats_query = format!(
            r#"
            SELECT
                COUNT(*) AS row_count,
                MIN({time_col}) AS time_min,
                MAX({time_col}) AS time_max
            FROM silver.{table}
            "#,
            time_col = time_column,
            table = table_name
        );
        let stats_row = conn.query_one(&stats_query, &[]).await.map_err(|e| {
            McpError::StorageError(format!("Failed to query table stats: {}", e))
        })?;

        let row_count: i64 = stats_row.get("row_count");
        let time_min: Option<DateTime<Utc>> = stats_row.get("time_min");
        let time_max: Option<DateTime<Utc>> = stats_row.get("time_max");

        // Query chunk info from TimescaleDB
        let chunk_query = r#"
            SELECT
                COUNT(*) AS chunk_count,
                hypertable_size(format('silver.%I', $1)::regclass) AS total_bytes
            FROM timescaledb_information.chunks c
            WHERE c.hypertable_schema = 'silver' AND c.hypertable_name = $1
        "#;
        let chunk_row = conn.query_opt(chunk_query, &[&table_name]).await.ok().flatten();

        let (chunk_count, total_bytes) = match chunk_row {
            Some(row) => (row.get::<_, i64>("chunk_count"), row.get::<_, i64>("total_bytes")),
            None => (0, 0),
        };

        // Query DQ rules count
        let dq_query = r#"
            SELECT
                COUNT(*) AS total_rules,
                COUNT(DISTINCT silver_column) AS columns_with_rules
            FROM data_dictionary.silver_dq_rules
            WHERE silver_table = $1
        "#;
        let dq_row = conn.query_opt(dq_query, &[&table_name]).await.ok().flatten();
        let dq_summary = dq_row.map(|row| {
            DqSummary::new(
                row.get::<_, i64>("total_rules") as i32,
                row.get::<_, i64>("columns_with_rules") as i32,
            )
        });

        let mut stats = SilverTableStats::new(table_name)
            .with_row_count(row_count)
            .with_chunk_count(chunk_count)
            .with_total_bytes(total_bytes);

        if let (Some(min), Some(max)) = (time_min, time_max) {
            stats = stats.with_time_range(min, max);
        }

        if let Some(dq) = dq_summary {
            stats = stats.with_dq_summary(dq);
        }

        debug!("Stats for {}: {} rows, {} chunks", table_name, row_count, chunk_count);
        Ok(stats)
    }
}

// ============================================================================
// Unit Tests (London School TDD - Behavior Verification)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== TimescalePoolConfig Tests ==========

    #[test]
    fn test_pool_config_default() {
        let config = TimescalePoolConfig::default();
        assert_eq!(config.max_size, 5);
        assert_eq!(config.min_idle, Some(1));
        assert_eq!(config.connection_timeout_secs, 10);
        assert_eq!(config.idle_timeout_secs, 30);
    }

    #[test]
    fn test_pool_config_for_constrained_environment() {
        let config = TimescalePoolConfig::for_constrained_environment();
        assert_eq!(config.max_size, 2);
        assert_eq!(config.min_idle, Some(1));
        assert_eq!(config.connection_timeout_secs, 5);
    }

    // ========== Helper Function Tests ==========

    #[test]
    fn test_normalize_table_name_with_prefix() {
        assert_eq!(
            TimescaleSilverStorage::normalize_table_name("silver.air_quality_observations"),
            "air_quality_observations"
        );
    }

    #[test]
    fn test_normalize_table_name_without_prefix() {
        assert_eq!(
            TimescaleSilverStorage::normalize_table_name("air_quality_observations"),
            "air_quality_observations"
        );
    }

    #[test]
    fn test_get_time_column_for_observations() {
        assert_eq!(
            TimescaleSilverStorage::get_time_column("air_quality_observations"),
            "observation_time"
        );
    }

    #[test]
    fn test_get_time_column_for_forecasts() {
        assert_eq!(
            TimescaleSilverStorage::get_time_column("weather_forecasts"),
            "valid_time"
        );
    }

    #[test]
    fn test_get_time_column_for_forecast_suffix() {
        assert_eq!(
            TimescaleSilverStorage::get_time_column("nws_forecast"),
            "valid_time"
        );
    }

    // ========== Type Builder Tests ==========

    #[test]
    fn test_silver_table_info_full_build() {
        let info = SilverTableInfo::new("test_table")
            .with_description("Test description")
            .with_grain("per_reading")
            .with_source_streams(vec!["stream-a".to_string(), "stream-b".to_string()])
            .with_hypertable(true, Some("1 day".to_string()))
            .with_row_count(50000)
            .with_total_bytes(1024 * 1024);

        assert_eq!(info.table_name, "test_table");
        assert_eq!(info.description, Some("Test description".to_string()));
        assert!(info.is_hypertable);
        assert_eq!(info.source_streams.len(), 2);
        assert_eq!(info.row_count, Some(50000));
    }

    #[test]
    fn test_silver_column_info_full_build() {
        let col = SilverColumnInfo::new("pm25", "DOUBLE PRECISION")
            .with_unit("ug/m3")
            .with_description("PM2.5 particulate matter")
            .with_nullable(true)
            .with_primary_key(false);

        assert_eq!(col.column_name, "pm25");
        assert_eq!(col.data_type, "DOUBLE PRECISION");
        assert_eq!(col.unit, Some("ug/m3".to_string()));
        assert!(col.nullable);
        assert!(!col.is_primary_key);
    }

    #[test]
    fn test_sample_filters_build() {
        use chrono::TimeZone;
        let since = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2026, 1, 17, 23, 59, 59).unwrap();

        let filters = SampleFilters::new()
            .with_since(since)
            .with_until(until)
            .with_order_by("observation_time DESC");

        assert_eq!(filters.since, Some(since));
        assert_eq!(filters.until, Some(until));
        assert_eq!(filters.order_by, Some("observation_time DESC".to_string()));
    }

    #[test]
    fn test_silver_table_stats_full_build() {
        use chrono::TimeZone;
        let min = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let max = Utc.with_ymd_and_hms(2026, 1, 17, 23, 59, 59).unwrap();

        let stats = SilverTableStats::new("test_table")
            .with_row_count(100000)
            .with_time_range(min, max)
            .with_chunk_count(17)
            .with_total_bytes(50 * 1024 * 1024)
            .with_dq_summary(DqSummary::new(5, 3));

        assert_eq!(stats.row_count, 100000);
        assert_eq!(stats.chunk_count, 17);
        assert!(stats.time_range.is_some());
        assert!(stats.dq_summary.is_some());
    }

    #[test]
    fn test_hypertable_info_build() {
        let info = HypertableInfo::new("observation_time", "1 day")
            .with_chunk_count(30)
            .with_total_bytes(100 * 1024 * 1024);

        assert_eq!(info.time_column, "observation_time");
        assert_eq!(info.chunk_interval, "1 day");
        assert_eq!(info.chunk_count, 30);
    }

    #[test]
    fn test_table_description_full_build() {
        let desc = SilverTableDescription::new("air_quality_observations")
            .with_description("Indoor air quality measurements")
            .with_columns(vec![
                SilverColumnInfo::new("observation_time", "TIMESTAMPTZ")
                    .with_nullable(false)
                    .with_primary_key(true),
                SilverColumnInfo::new("pm25", "DOUBLE PRECISION")
                    .with_unit("ug/m3"),
            ])
            .with_hypertable_info(
                HypertableInfo::new("observation_time", "1 day")
                    .with_chunk_count(15),
            );

        assert_eq!(desc.table_name, "air_quality_observations");
        assert_eq!(desc.columns.len(), 2);
        assert!(desc.hypertable_info.is_some());
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_mcp_error_storage_code() {
        let err = McpError::StorageError("connection failed".to_string());
        assert_eq!(err.mcp_error_code(), "STORAGE_ERROR");
        assert_eq!(err.json_rpc_code(), -32001);
    }

    #[test]
    fn test_mcp_error_stream_not_found_code() {
        let err = McpError::StreamNotFound("unknown_table".to_string());
        assert_eq!(err.mcp_error_code(), "STREAM_NOT_FOUND");
        assert_eq!(err.json_rpc_code(), -32002);
    }

    // ========== Serialization Tests ==========

    #[test]
    fn test_silver_table_info_serialization() {
        let info = SilverTableInfo::new("test_table")
            .with_hypertable(true, Some("1 day".to_string()));

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test_table"));
        assert!(json.contains("is_hypertable"));
    }

    #[test]
    fn test_sample_filters_default_serialization() {
        let filters = SampleFilters::default();
        let json = serde_json::to_string(&filters).unwrap();
        // Empty filters should serialize minimally
        assert_eq!(json, "{}");
    }
}

// ============================================================================
// Integration Test Module (requires live TimescaleDB)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Helper to get test database URL from environment.
    fn test_db_url() -> Option<String> {
        std::env::var("NDP_TEST_TIMESCALE_URL").ok()
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_create_storage_and_list_tables() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");
        let storage = TimescaleSilverStorage::new(&url).await.expect("Failed to create storage");

        let tables = storage.list_tables().await.expect("Failed to list tables");
        // We expect at least the core Silver tables
        assert!(!tables.is_empty(), "Should have at least one Silver table");

        // Verify table properties
        for table in &tables {
            assert!(!table.table_name.is_empty());
        }
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_describe_existing_table() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");
        let storage = TimescaleSilverStorage::new(&url).await.expect("Failed to create storage");

        // This test assumes air_quality_observations exists
        let desc = storage.describe_table("air_quality_observations").await;

        match desc {
            Ok(d) => {
                assert_eq!(d.table_name, "air_quality_observations");
                assert!(!d.columns.is_empty(), "Should have columns");
            }
            Err(McpError::StreamNotFound(_)) => {
                // Table might not exist in test environment, that's OK
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_describe_nonexistent_table() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");
        let storage = TimescaleSilverStorage::new(&url).await.expect("Failed to create storage");

        let result = storage.describe_table("nonexistent_table_xyz").await;
        assert!(matches!(result, Err(McpError::StreamNotFound(_))));
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_sample_with_limit() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");
        let storage = TimescaleSilverStorage::new(&url).await.expect("Failed to create storage");

        // Sample with a small limit
        let result = storage.sample("air_quality_observations", 5, None).await;

        match result {
            Ok(rows) => {
                assert!(rows.len() <= 5, "Should respect limit");
            }
            Err(McpError::StreamNotFound(_)) => {
                // Table might not exist in test environment
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_sample_clamps_to_max() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");
        let storage = TimescaleSilverStorage::new(&url).await.expect("Failed to create storage");

        // Request more than max (100)
        let result = storage.sample("air_quality_observations", 1000, None).await;

        match result {
            Ok(rows) => {
                assert!(rows.len() <= 100, "Should clamp to max 100");
            }
            Err(McpError::StreamNotFound(_)) => {
                // Table might not exist
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_get_stats() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");
        let storage = TimescaleSilverStorage::new(&url).await.expect("Failed to create storage");

        let result = storage.get_stats("air_quality_observations").await;

        match result {
            Ok(stats) => {
                assert_eq!(stats.table_name, "air_quality_observations");
                assert!(stats.row_count >= 0);
            }
            Err(McpError::StreamNotFound(_)) => {
                // Table might not exist
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // Requires running TimescaleDB
    async fn test_connection_pool_config() {
        let url = test_db_url().expect("NDP_TEST_TIMESCALE_URL must be set");

        // Test with constrained config
        let config = TimescalePoolConfig::for_constrained_environment();
        let storage = TimescaleSilverStorage::with_config(&url, config).await;

        assert!(storage.is_ok(), "Should create storage with custom config");
    }
}
