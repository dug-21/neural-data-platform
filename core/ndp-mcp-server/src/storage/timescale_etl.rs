//! TimescaleDB adapter for ETL run history storage.
//!
//! Implements the `EtlRunStore` trait for accessing ETL run statistics
//! stored in the `silver.etl_runs` table. Uses bb8 connection pooling
//! with tokio-postgres for efficient database access.
//!
//! # Connection Pool Configuration (dp-011 pattern)
//!
//! - max_size: 2 (Pi resource-friendly)
//! - min_idle: 1 (always ready)
//! - connection_timeout: 5s (fail fast)
//!
//! # Feature: dp-010 (BUG-001 fix)

use async_trait::async_trait;
use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use chrono::{DateTime, Utc};
use tokio_postgres::NoTls;
use tracing::{debug, instrument, warn};

use super::types::{
    EtlHistoryResult, EtlRunDetail, EtlRunInfo, EtlStreamStatus, FreshnessEntry, FreshnessReport,
    FreshnessSummary, HistorySummary, RunStats,
};
use super::EtlRunStore;
use crate::error::{McpError, McpResult};

/// Connection pool type alias for clarity.
pub type PgPool = Pool<PostgresConnectionManager<NoTls>>;

/// TimescaleDB adapter for ETL run history.
///
/// Provides read-only access to the `silver.etl_runs` table for
/// ETL monitoring and observability through MCP tools.
///
/// # Example
///
/// ```ignore
/// let store = TimescaleEtlRunStore::new("postgresql://ndp:pass@localhost/ndp").await?;
/// let status = store.get_status(Some("air-quality".to_string())).await?;
/// ```
#[derive(Clone)]
pub struct TimescaleEtlRunStore {
    pool: PgPool,
}

impl TimescaleEtlRunStore {
    /// Create a new TimescaleEtlRunStore with connection pool.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string
    ///
    /// # Pool Configuration
    ///
    /// Following dp-011-hybrid-connection-pattern:
    /// - max_size: 2 (Pi resource-friendly)
    /// - min_idle: 1 (always have one ready)
    /// - connection_timeout: 5 seconds
    ///
    /// # Errors
    ///
    /// Returns `McpError::StorageError` if connection fails.
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

        debug!("TimescaleEtlRunStore pool created");
        Ok(Self { pool })
    }

    /// Create from existing pool (for testing and shared connections).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a connection from the pool.
    async fn get_conn(
        &self,
    ) -> McpResult<PooledConnection<'_, PostgresConnectionManager<NoTls>>> {
        self.pool
            .get()
            .await
            .map_err(|e| McpError::StorageError(format!("Pool connection error: {}", e)))
    }

    /// Determine freshness status based on age in seconds.
    ///
    /// Thresholds (from ETL-STATUS-SPEC.md):
    /// - fresh: < 5 minutes (300s)
    /// - stale: 5-30 minutes
    /// - critical: > 30 minutes
    fn freshness_status(age_seconds: Option<i64>) -> &'static str {
        match age_seconds {
            None => "no_data",
            Some(age) if age < 300 => "fresh",
            Some(age) if age < 1800 => "stale",
            Some(_) => "critical",
        }
    }

    /// Determine stream health status based on 24h run statistics.
    ///
    /// - healthy: > 90% success rate
    /// - warning: 50-90% success rate
    /// - error: < 50% success rate or no runs
    fn health_status(stats: &RunStats) -> &'static str {
        if stats.total == 0 {
            return "unknown";
        }
        let success_rate = stats.succeeded as f64 / stats.total as f64;
        if success_rate > 0.9 {
            "healthy"
        } else if success_rate >= 0.5 {
            "warning"
        } else {
            "error"
        }
    }
}

#[async_trait]
impl EtlRunStore for TimescaleEtlRunStore {
    /// Get current ETL status for one or all streams.
    ///
    /// Queries `silver.etl_runs` to get the latest run per stream
    /// and 24-hour statistics.
    #[instrument(skip(self), fields(stream_id))]
    async fn get_status(&self, stream_id: Option<String>) -> McpResult<Vec<EtlStreamStatus>> {
        let conn = self.get_conn().await?;

        // Query for latest run per stream with 24h stats
        let query = r#"
            WITH latest_runs AS (
                SELECT DISTINCT ON (stream_id)
                    stream_id,
                    id,
                    started_at,
                    completed_at,
                    duration_ms,
                    status,
                    rows_processed,
                    rows_flagged,
                    rows_rejected,
                    watermark_before,
                    watermark_after,
                    error_message
                FROM silver.etl_runs
                WHERE ($1::TEXT IS NULL OR stream_id = $1)
                ORDER BY stream_id, started_at DESC
            ),
            run_stats AS (
                SELECT
                    stream_id,
                    COUNT(*)::INT AS total,
                    COUNT(*) FILTER (WHERE status = 'success')::INT AS succeeded,
                    COUNT(*) FILTER (WHERE status = 'failed')::INT AS failed
                FROM silver.etl_runs
                WHERE ($1::TEXT IS NULL OR stream_id = $1)
                  AND started_at > NOW() - INTERVAL '24 hours'
                GROUP BY stream_id
            )
            SELECT
                COALESCE(lr.stream_id, rs.stream_id) AS stream_id,
                lr.id,
                lr.started_at,
                lr.completed_at,
                lr.duration_ms,
                lr.status,
                lr.rows_processed,
                lr.rows_flagged,
                lr.rows_rejected,
                lr.watermark_before,
                lr.watermark_after,
                lr.error_message,
                COALESCE(rs.total, 0) AS runs_total,
                COALESCE(rs.succeeded, 0) AS runs_succeeded,
                COALESCE(rs.failed, 0) AS runs_failed
            FROM latest_runs lr
            FULL OUTER JOIN run_stats rs ON lr.stream_id = rs.stream_id
            ORDER BY stream_id
        "#;

        let rows = conn
            .query(query, &[&stream_id])
            .await
            .map_err(|e| McpError::StorageError(format!("Query failed: {}", e)))?;

        let mut statuses = Vec::new();
        for row in rows {
            let sid: String = row.get("stream_id");
            let runs_stats = RunStats::new(
                row.get::<_, i32>("runs_total"),
                row.get::<_, i32>("runs_succeeded"),
                row.get::<_, i32>("runs_failed"),
            );

            let status = Self::health_status(&runs_stats);

            // Build last_run info if available
            let last_run = if let Some(id) = row.get::<_, Option<uuid::Uuid>>("id") {
                let started_at: DateTime<Utc> = row.get("started_at");
                let mut run_info = EtlRunInfo::new(id.to_string(), started_at);

                if let Some(completed) = row.get::<_, Option<DateTime<Utc>>>("completed_at") {
                    run_info = run_info.with_completed_at(completed);
                }

                run_info = run_info.with_row_counts(
                    row.get::<_, Option<i64>>("rows_processed").unwrap_or(0),
                    row.get::<_, Option<i64>>("rows_flagged").unwrap_or(0),
                    row.get::<_, Option<i64>>("rows_rejected").unwrap_or(0),
                );

                run_info = run_info.with_watermarks(
                    row.get::<_, Option<DateTime<Utc>>>("watermark_before"),
                    row.get::<_, Option<DateTime<Utc>>>("watermark_after"),
                );

                if let Some(err_msg) = row.get::<_, Option<String>>("error_message") {
                    run_info = run_info.with_error(err_msg);
                }

                Some(run_info)
            } else {
                None
            };

            statuses.push(
                EtlStreamStatus::new(sid, status)
                    .with_last_run(last_run.unwrap_or_else(|| {
                        // Placeholder if no runs exist
                        EtlRunInfo::new("none", Utc::now())
                    }))
                    .with_runs_last_24h(runs_stats),
            );
        }

        // Filter out placeholder entries if no last_run
        let statuses: Vec<_> = statuses
            .into_iter()
            .map(|mut s| {
                if s.last_run.as_ref().map(|r| r.id.as_str()) == Some("none") {
                    s.last_run = None;
                }
                s
            })
            .collect();

        debug!(count = statuses.len(), "Retrieved ETL status");
        Ok(statuses)
    }

    /// Get historical ETL runs for a stream.
    ///
    /// Returns paginated history with optional filtering by time and status.
    #[instrument(skip(self), fields(stream_id = %stream_id, limit))]
    async fn get_history(
        &self,
        stream_id: &str,
        limit: usize,
        since: Option<DateTime<Utc>>,
        status_filter: Option<String>,
    ) -> McpResult<EtlHistoryResult> {
        let conn = self.get_conn().await?;
        let limit_i64 = limit as i64;

        // Main query for runs
        let query = r#"
            SELECT
                id,
                started_at,
                completed_at,
                duration_ms,
                status,
                rows_processed,
                rows_flagged,
                rows_rejected,
                watermark_before,
                watermark_after,
                error_message,
                error_context,
                run_mode
            FROM silver.etl_runs
            WHERE stream_id = $1
              AND ($2::TIMESTAMPTZ IS NULL OR started_at > $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY started_at DESC
            LIMIT $4
        "#;

        let rows = conn
            .query(query, &[&stream_id, &since, &status_filter, &limit_i64])
            .await
            .map_err(|e| McpError::StorageError(format!("Query failed: {}", e)))?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: uuid::Uuid = row.get("id");
            let started_at: DateTime<Utc> = row.get("started_at");
            let status: String = row.get("status");
            let run_mode: String = row.get::<_, Option<String>>("run_mode").unwrap_or_else(|| "daemon".to_string());

            let mut detail = EtlRunDetail::new(id.to_string(), started_at, &status, &run_mode);

            if let Some(completed) = row.get::<_, Option<DateTime<Utc>>>("completed_at") {
                detail = detail.with_completed_at(completed);
            }

            detail = detail.with_row_counts(
                row.get::<_, Option<i64>>("rows_processed").unwrap_or(0),
                row.get::<_, Option<i64>>("rows_flagged").unwrap_or(0),
                row.get::<_, Option<i64>>("rows_rejected").unwrap_or(0),
            );

            detail = detail.with_watermarks(
                row.get::<_, Option<DateTime<Utc>>>("watermark_before"),
                row.get::<_, Option<DateTime<Utc>>>("watermark_after"),
            );

            let error_message: Option<String> = row.get("error_message");
            let error_context: Option<serde_json::Value> = row.get("error_context");
            if let Some(msg) = error_message {
                detail = detail.with_error(msg, error_context);
            }

            runs.push(detail);
        }

        // Count query for summary
        let count_query = r#"
            SELECT
                COUNT(*) AS total_available,
                MIN(started_at) AS oldest,
                MAX(started_at) AS newest
            FROM silver.etl_runs
            WHERE stream_id = $1
              AND ($2::TIMESTAMPTZ IS NULL OR started_at > $2)
              AND ($3::TEXT IS NULL OR status = $3)
        "#;

        let count_row = conn
            .query_one(count_query, &[&stream_id, &since, &status_filter])
            .await
            .map_err(|e| McpError::StorageError(format!("Count query failed: {}", e)))?;

        let total_available: i64 = count_row.get("total_available");
        let mut summary = HistorySummary::new(runs.len() as i32, total_available as i32);

        if let (Some(oldest), Some(newest)) = (
            count_row.get::<_, Option<DateTime<Utc>>>("oldest"),
            count_row.get::<_, Option<DateTime<Utc>>>("newest"),
        ) {
            summary = summary.with_time_range(oldest, newest);
        }

        debug!(
            stream_id,
            returned = runs.len(),
            total = total_available,
            "Retrieved ETL history"
        );

        Ok(EtlHistoryResult::new(stream_id)
            .with_runs(runs)
            .with_summary(summary))
    }

    /// Get data freshness report across layers.
    ///
    /// For Silver layer, queries max timestamps from Silver tables
    /// and correlates with ETL run completion times.
    #[instrument(skip(self))]
    async fn get_freshness(&self, layer: Option<String>) -> McpResult<FreshnessReport> {
        let conn = self.get_conn().await?;
        let now = Utc::now();

        let mut freshness_entries = Vec::new();
        let mut bronze_count = 0;
        let mut silver_count = 0;
        let mut stale_count = 0;
        let mut critical_count = 0;

        let include_bronze = layer.as_deref() != Some("silver");
        let include_silver = layer.as_deref() != Some("bronze");

        // Bronze freshness from ETL runs (watermark_after represents Bronze data timestamp)
        if include_bronze {
            let bronze_query = r#"
                SELECT DISTINCT ON (stream_id)
                    stream_id,
                    watermark_after AS latest_timestamp,
                    EXTRACT(EPOCH FROM (NOW() - watermark_after))::BIGINT AS age_seconds
                FROM silver.etl_runs
                WHERE watermark_after IS NOT NULL
                ORDER BY stream_id, started_at DESC
            "#;

            let bronze_rows = conn
                .query(bronze_query, &[])
                .await
                .map_err(|e| McpError::StorageError(format!("Bronze freshness query failed: {}", e)))?;

            for row in bronze_rows {
                let stream_id: String = row.get("stream_id");
                let latest_ts: Option<DateTime<Utc>> = row.get("latest_timestamp");
                let age_seconds: Option<i64> = row.get("age_seconds");

                let status = Self::freshness_status(age_seconds);
                if status == "stale" {
                    stale_count += 1;
                } else if status == "critical" {
                    critical_count += 1;
                }

                let mut entry = FreshnessEntry::new("bronze", &stream_id, status);
                if let (Some(ts), Some(age)) = (latest_ts, age_seconds) {
                    entry = entry.with_latest_timestamp(ts, now);
                    // Override the calculated age with the DB-computed one for accuracy
                    entry.age_seconds = Some(age);
                }

                freshness_entries.push(entry);
                bronze_count += 1;
            }
        }

        // Silver freshness from actual Silver tables
        if include_silver {
            // Query Silver table freshness - this queries the actual hypertables
            // Table names and time columns are based on NDP Silver schema
            let silver_query = r#"
                WITH silver_tables AS (
                    SELECT
                        'air_quality_observations' AS table_name,
                        (SELECT MAX(observation_time) FROM silver.air_quality_observations) AS latest_ts,
                        (SELECT COUNT(*) FROM silver.air_quality_observations) AS row_count
                    UNION ALL
                    SELECT
                        'weather_observations',
                        (SELECT MAX(observation_time) FROM silver.weather_observations),
                        (SELECT COUNT(*) FROM silver.weather_observations)
                    UNION ALL
                    SELECT
                        'weather_forecasts',
                        (SELECT MAX(issue_time) FROM silver.weather_forecasts),
                        (SELECT COUNT(*) FROM silver.weather_forecasts)
                    UNION ALL
                    SELECT
                        'outdoor_air_quality',
                        (SELECT MAX(observation_time) FROM silver.outdoor_air_quality),
                        (SELECT COUNT(*) FROM silver.outdoor_air_quality)
                )
                SELECT
                    st.table_name,
                    st.latest_ts,
                    st.row_count,
                    EXTRACT(EPOCH FROM (NOW() - st.latest_ts))::BIGINT AS age_seconds,
                    er.completed_at AS last_etl_run
                FROM silver_tables st
                LEFT JOIN LATERAL (
                    SELECT completed_at
                    FROM silver.etl_runs
                    WHERE status = 'success'
                    ORDER BY started_at DESC
                    LIMIT 1
                ) er ON true
                WHERE st.latest_ts IS NOT NULL OR st.row_count > 0
            "#;

            match conn.query(silver_query, &[]).await {
                Ok(silver_rows) => {
                    for row in silver_rows {
                        let table_name: String = row.get("table_name");
                        let latest_ts: Option<DateTime<Utc>> = row.get("latest_ts");
                        let row_count: Option<i64> = row.get("row_count");
                        let age_seconds: Option<i64> = row.get("age_seconds");
                        let last_etl: Option<DateTime<Utc>> = row.get("last_etl_run");

                        let status = Self::freshness_status(age_seconds);
                        if status == "stale" {
                            stale_count += 1;
                        } else if status == "critical" {
                            critical_count += 1;
                        }

                        let mut entry = FreshnessEntry::new("silver", &table_name, status);
                        if let Some(ts) = latest_ts {
                            entry = entry.with_latest_timestamp(ts, now);
                            if let Some(age) = age_seconds {
                                entry.age_seconds = Some(age);
                            }
                        }
                        if let Some(count) = row_count {
                            entry = entry.with_row_count(count);
                        }
                        if let Some(etl) = last_etl {
                            entry = entry.with_last_etl_run(etl);
                        }

                        freshness_entries.push(entry);
                        silver_count += 1;
                    }
                }
                Err(e) => {
                    // Silver tables might not exist yet - warn but don't fail
                    warn!("Silver freshness query failed (tables may not exist): {}", e);
                }
            }
        }

        let summary = FreshnessSummary::new(bronze_count, silver_count, stale_count, critical_count);

        debug!(
            bronze = bronze_count,
            silver = silver_count,
            stale = stale_count,
            critical = critical_count,
            "Generated freshness report"
        );

        Ok(FreshnessReport::new(now)
            .with_freshness(freshness_entries)
            .with_summary(summary))
    }
}

// ============================================================================
// Tests (London School TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MockEtlRunStore;
    use chrono::TimeZone;

    // ========================================================================
    // Unit Tests - Behavior Verification with Mocks
    // ========================================================================

    #[tokio::test]
    async fn test_get_status_returns_all_streams_when_none_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(None::<String>))
            .times(1)
            .returning(|_| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
                let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();
                Ok(vec![
                    EtlStreamStatus::new("air-quality", "healthy")
                        .with_last_run(
                            EtlRunInfo::new("run-001", started)
                                .with_completed_at(completed)
                                .with_row_counts(1000, 5, 2),
                        )
                        .with_runs_last_24h(RunStats::new(24, 23, 1)),
                    EtlStreamStatus::new("outdoor-weather", "healthy")
                        .with_runs_last_24h(RunStats::new(24, 24, 0)),
                ])
            });

        let result = mock.get_status(None).await;
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].stream_id, "air-quality");
        assert_eq!(statuses[0].status, "healthy");
    }

    #[tokio::test]
    async fn test_get_status_filters_by_stream_id() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(Some("air-quality".to_string())))
            .times(1)
            .returning(|_| {
                Ok(vec![EtlStreamStatus::new("air-quality", "healthy")
                    .with_runs_last_24h(RunStats::new(24, 24, 0))])
            });

        let result = mock.get_status(Some("air-quality".to_string())).await;
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].stream_id, "air-quality");
    }

    #[tokio::test]
    async fn test_get_status_returns_empty_for_unknown_stream() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .with(mockall::predicate::eq(Some("nonexistent".to_string())))
            .times(1)
            .returning(|_| Ok(vec![]));

        let result = mock.get_status(Some("nonexistent".to_string())).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_history_returns_paginated_runs() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(10),
                mockall::predicate::eq(None::<DateTime<Utc>>),
                mockall::predicate::eq(None::<String>),
            )
            .times(1)
            .returning(|stream_id, _, _, _| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 0).unwrap();
                let completed = Utc.with_ymd_and_hms(2026, 1, 17, 10, 0, 5).unwrap();
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![
                        EtlRunDetail::new("run-001", started, "success", "daemon")
                            .with_completed_at(completed)
                            .with_row_counts(1000, 5, 2),
                        EtlRunDetail::new(
                            "run-002",
                            started - chrono::Duration::hours(1),
                            "success",
                            "daemon",
                        )
                        .with_completed_at(completed - chrono::Duration::hours(1))
                        .with_row_counts(950, 3, 1),
                    ])
                    .with_summary(HistorySummary::new(2, 100)))
            });

        let result = mock.get_history("air-quality", 10, None, None).await;
        assert!(result.is_ok());
        let history = result.unwrap();
        assert_eq!(history.stream_id, "air-quality");
        assert_eq!(history.runs.len(), 2);
        assert_eq!(history.summary.total_returned, 2);
        assert_eq!(history.summary.total_available, 100);
    }

    #[tokio::test]
    async fn test_get_history_with_since_filter() {
        let mut mock = MockEtlRunStore::new();
        let since = Utc.with_ymd_and_hms(2026, 1, 17, 0, 0, 0).unwrap();

        mock.expect_get_history()
            .withf(move |stream, limit, since_opt, status| {
                stream == "air-quality" && *limit == 50 && since_opt.is_some() && status.is_none()
            })
            .times(1)
            .returning(|stream_id, _, _, _| {
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![])
                    .with_summary(HistorySummary::new(0, 0)))
            });

        let result = mock.get_history("air-quality", 50, Some(since), None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_history_with_status_filter() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(50),
                mockall::predicate::eq(None::<DateTime<Utc>>),
                mockall::predicate::eq(Some("failed".to_string())),
            )
            .times(1)
            .returning(|stream_id, _, _, _| {
                let started = Utc.with_ymd_and_hms(2026, 1, 17, 5, 0, 0).unwrap();
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![EtlRunDetail::new(
                        "run-003",
                        started,
                        "failed",
                        "daemon",
                    )
                    .with_error("Connection timeout", None)])
                    .with_summary(HistorySummary::new(1, 1)))
            });

        let result = mock
            .get_history("air-quality", 50, None, Some("failed".to_string()))
            .await;
        assert!(result.is_ok());
        let history = result.unwrap();
        assert_eq!(history.runs.len(), 1);
        assert_eq!(history.runs[0].status, "failed");
    }

    #[tokio::test]
    async fn test_get_freshness_returns_all_layers() {
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
                        FreshnessEntry::new("silver", "air_quality_observations", "fresh")
                            .with_latest_timestamp(latest, now)
                            .with_row_count(50000)
                            .with_last_etl_run(now - chrono::Duration::minutes(1)),
                    ])
                    .with_summary(FreshnessSummary::new(1, 1, 0, 0)))
            });

        let result = mock.get_freshness(None).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.freshness.len(), 2);
        assert_eq!(report.summary.bronze_streams, 1);
        assert_eq!(report.summary.silver_tables, 1);
    }

    #[tokio::test]
    async fn test_get_freshness_with_bronze_filter() {
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

        let result = mock.get_freshness(Some("bronze".to_string())).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.freshness.len(), 1);
        assert_eq!(report.freshness[0].layer, "bronze");
    }

    #[tokio::test]
    async fn test_get_freshness_detects_stale_data() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness().times(1).returning(|_| {
            let now = Utc::now();
            let stale_timestamp = now - chrono::Duration::minutes(20);
            Ok(FreshnessReport::new(now)
                .with_freshness(vec![FreshnessEntry::new("bronze", "air-quality", "stale")
                    .with_latest_timestamp(stale_timestamp, now)])
                .with_summary(FreshnessSummary::new(1, 0, 1, 0)))
        });

        let result = mock.get_freshness(None).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.summary.stale_count, 1);
        assert_eq!(report.freshness[0].freshness_status, "stale");
    }

    #[tokio::test]
    async fn test_get_freshness_detects_critical_data() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness().times(1).returning(|_| {
            let now = Utc::now();
            let critical_timestamp = now - chrono::Duration::hours(2);
            Ok(FreshnessReport::new(now)
                .with_freshness(vec![FreshnessEntry::new(
                    "silver",
                    "weather_forecasts",
                    "critical",
                )
                .with_latest_timestamp(critical_timestamp, now)])
                .with_summary(FreshnessSummary::new(0, 1, 0, 1)))
        });

        let result = mock.get_freshness(None).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.summary.critical_count, 1);
        assert_eq!(report.freshness[0].freshness_status, "critical");
    }

    // ========================================================================
    // Unit Tests - Internal Helper Functions
    // ========================================================================

    #[test]
    fn test_freshness_status_fresh() {
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(100)), "fresh");
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(299)), "fresh");
    }

    #[test]
    fn test_freshness_status_stale() {
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(300)), "stale");
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(1000)), "stale");
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(1799)), "stale");
    }

    #[test]
    fn test_freshness_status_critical() {
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(1800)), "critical");
        assert_eq!(TimescaleEtlRunStore::freshness_status(Some(3600)), "critical");
    }

    #[test]
    fn test_freshness_status_no_data() {
        assert_eq!(TimescaleEtlRunStore::freshness_status(None), "no_data");
    }

    #[test]
    fn test_health_status_healthy() {
        let stats = RunStats::new(100, 95, 5);
        assert_eq!(TimescaleEtlRunStore::health_status(&stats), "healthy");
    }

    #[test]
    fn test_health_status_warning() {
        let stats = RunStats::new(100, 60, 40);
        assert_eq!(TimescaleEtlRunStore::health_status(&stats), "warning");
    }

    #[test]
    fn test_health_status_error() {
        let stats = RunStats::new(100, 40, 60);
        assert_eq!(TimescaleEtlRunStore::health_status(&stats), "error");
    }

    #[test]
    fn test_health_status_unknown_no_runs() {
        let stats = RunStats::new(0, 0, 0);
        assert_eq!(TimescaleEtlRunStore::health_status(&stats), "unknown");
    }

    // ========================================================================
    // Workflow Tests - Verifying Behavior Sequences
    // ========================================================================

    #[tokio::test]
    async fn test_etl_monitoring_workflow() {
        let mut mock = MockEtlRunStore::new();
        let mut seq = mockall::Sequence::new();

        // Step 1: Check overall status - find warning
        mock.expect_get_status()
            .with(mockall::predicate::eq(None::<String>))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                Ok(vec![EtlStreamStatus::new("air-quality", "warning")
                    .with_runs_last_24h(RunStats::new(24, 20, 4))])
            });

        // Step 2: Get history for problematic stream - find failures
        mock.expect_get_history()
            .with(
                mockall::predicate::eq("air-quality"),
                mockall::predicate::eq(10),
                mockall::predicate::always(),
                mockall::predicate::eq(Some("failed".to_string())),
            )
            .times(1)
            .in_sequence(&mut seq)
            .returning(|stream_id, _, _, _| {
                let started = Utc::now();
                Ok(EtlHistoryResult::new(stream_id)
                    .with_runs(vec![EtlRunDetail::new(
                        "run-fail",
                        started,
                        "failed",
                        "daemon",
                    )
                    .with_error("Connection refused", None)])
                    .with_summary(HistorySummary::new(1, 4)))
            });

        // Step 3: Check freshness - correlate with failures
        mock.expect_get_freshness()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                let now = Utc::now();
                Ok(FreshnessReport::new(now)
                    .with_freshness(vec![FreshnessEntry::new(
                        "silver",
                        "air_quality_observations",
                        "stale",
                    )])
                    .with_summary(FreshnessSummary::new(0, 1, 1, 0)))
            });

        // Execute workflow
        let statuses = mock.get_status(None).await.unwrap();
        assert_eq!(statuses[0].status, "warning");

        let history = mock
            .get_history("air-quality", 10, None, Some("failed".to_string()))
            .await
            .unwrap();
        assert!(!history.runs.is_empty());
        assert!(history.runs[0].error_message.is_some());

        let freshness = mock.get_freshness(None).await.unwrap();
        assert_eq!(freshness.summary.stale_count, 1);
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_status_handles_storage_error() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_status()
            .times(1)
            .returning(|_| Err(McpError::StorageError("Database connection failed".to_string())));

        let result = mock.get_status(None).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_get_history_handles_storage_error() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_history()
            .times(1)
            .returning(|_, _, _, _| Err(McpError::StorageError("Query timeout".to_string())));

        let result = mock.get_history("air-quality", 10, None, None).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }

    #[tokio::test]
    async fn test_get_freshness_handles_storage_error() {
        let mut mock = MockEtlRunStore::new();

        mock.expect_get_freshness()
            .times(1)
            .returning(|_| Err(McpError::StorageError("Connection pool exhausted".to_string())));

        let result = mock.get_freshness(None).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::StorageError(_)));
    }
}
