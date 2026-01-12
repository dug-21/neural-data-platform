//! ETL execution engine
//!
//! Executes the generated SQL using DuckDB to transform
//! Bronze Parquet data into Silver TimescaleDB tables.
//!
//! # Architecture
//!
//! The EtlRunner orchestrates the entire ETL process:
//! 1. Connects to DuckDB in-memory database
//! 2. Loads PostgreSQL extension for TimescaleDB writes
//! 3. Resolves Bronze Parquet files for a stream
//! 4. Gets watermark from target table for incremental loads
//! 5. Uses SqlGenerator to generate ETL SQL from config
//! 6. Executes the SQL and collects metrics
//!
//! # Example
//!
//! ```ignore
//! let runner = EtlRunner::from_env()?;
//! let stats = runner.run_etl(&config, "air-quality", "/data/raw")?;
//! println!("Processed {} rows", stats.rows_processed);
//! ```

use std::path::Path;
use std::time::Instant;

use chrono::{DateTime, Utc};
use duckdb::Connection;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use neural_core::config::{DeduplicationStrategy, SilverEtlConfig};

use crate::dq::DqSqlGenerator;
use crate::sql_gen::SqlGenerator;

// =============================================================================
// Error Types
// =============================================================================

/// ETL execution errors
#[derive(Debug, Error)]
pub enum EtlError {
    /// DuckDB initialization or connection error
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),

    /// Failed to load PostgreSQL extension
    #[error("Failed to load PostgreSQL extension: {0}")]
    PostgresExtension(String),

    /// Failed to attach PostgreSQL database
    #[error("Failed to attach PostgreSQL: {0}")]
    PostgresAttach(String),

    /// Failed to resolve Parquet files
    #[error("Failed to resolve Parquet files for stream '{stream_id}': {message}")]
    ParquetResolution { stream_id: String, message: String },

    /// Watermark query error
    #[error("Watermark query failed for table '{table}': {message}")]
    Watermark { table: String, message: String },

    /// SQL generation error
    #[error("SQL generation failed: {0}")]
    SqlGeneration(String),

    /// SQL execution error
    #[error("SQL execution failed: {0}")]
    SqlExecution(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Environment variable error
    #[error("Environment variable '{var}' not set: {message}")]
    EnvVar { var: String, message: String },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// =============================================================================
// ETL Runner
// =============================================================================

/// ETL Runner
///
/// Manages DuckDB connection and executes ETL operations:
/// - Loads postgres extension for TimescaleDB writes
/// - Resolves Bronze Parquet files
/// - Executes generated SQL
/// - Collects metrics
pub struct EtlRunner {
    /// DuckDB connection
    conn: Connection,

    /// PostgreSQL connection string (for when we attach)
    pg_conn_str: Option<String>,

    /// Whether PostgreSQL extension is loaded
    postgres_loaded: bool,

    /// Whether PostgreSQL is attached
    postgres_attached: bool,
}

impl EtlRunner {
    /// Create an in-memory DuckDB connection for testing
    ///
    /// This creates a DuckDB instance without PostgreSQL attachment,
    /// suitable for unit tests that don't need database writes.
    pub fn new_in_memory() -> Result<Self, EtlError> {
        debug!("Creating in-memory DuckDB connection");

        let conn = Connection::open_in_memory()?;

        // Enable JSON and Parquet support (built-in for bundled DuckDB)
        conn.execute_batch(
            r#"
            SET enable_progress_bar = false;
            "#,
        )?;

        debug!("In-memory DuckDB connection created");

        Ok(Self {
            conn,
            pg_conn_str: None,
            postgres_loaded: false,
            postgres_attached: false,
        })
    }

    /// Create runner with PostgreSQL connection
    ///
    /// # Arguments
    ///
    /// * `pg_conn_str` - PostgreSQL connection string in the format:
    ///   `postgresql://user:password@host:port/database`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let runner = EtlRunner::with_postgres("postgresql://ndp:secret@localhost:5432/ndp")?;
    /// ```
    pub fn with_postgres(pg_conn_str: &str) -> Result<Self, EtlError> {
        info!(pg_host = %extract_host(pg_conn_str), "Creating DuckDB connection with PostgreSQL");

        let conn = Connection::open_in_memory()?;

        // Enable JSON and Parquet support
        conn.execute_batch(
            r#"
            SET enable_progress_bar = false;
            "#,
        )?;

        let mut runner = Self {
            conn,
            pg_conn_str: Some(pg_conn_str.to_string()),
            postgres_loaded: false,
            postgres_attached: false,
        };

        // Load and attach PostgreSQL
        runner.load_postgres_extension()?;
        runner.attach_postgres()?;

        Ok(runner)
    }

    /// Create runner from environment variables
    ///
    /// Reads `TIMESCALE_URL` or `NDP_TIMESCALE_URL` environment variable.
    /// Falls back to individual variables if URL not set:
    /// - `NDP_TIMESCALE_HOST` (default: localhost)
    /// - `NDP_TIMESCALE_PORT` (default: 5432)
    /// - `NDP_TIMESCALE_DB` (default: ndp)
    /// - `NDP_TIMESCALE_USER` (default: ndp)
    /// - `NDP_TIMESCALE_PASSWORD` (required)
    pub fn from_env() -> Result<Self, EtlError> {
        let conn_str = if let Ok(url) = std::env::var("TIMESCALE_URL") {
            url
        } else if let Ok(url) = std::env::var("NDP_TIMESCALE_URL") {
            url
        } else {
            // Build from individual variables
            let host =
                std::env::var("NDP_TIMESCALE_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("NDP_TIMESCALE_PORT").unwrap_or_else(|_| "5432".to_string());
            let db = std::env::var("NDP_TIMESCALE_DB").unwrap_or_else(|_| "ndp".to_string());
            let user = std::env::var("NDP_TIMESCALE_USER").unwrap_or_else(|_| "ndp".to_string());
            let password =
                std::env::var("NDP_TIMESCALE_PASSWORD").map_err(|_| EtlError::EnvVar {
                    var: "NDP_TIMESCALE_PASSWORD".to_string(),
                    message: "Password required for PostgreSQL connection".to_string(),
                })?;

            format!(
                "postgresql://{}:{}@{}:{}/{}",
                user, password, host, port, db
            )
        };

        Self::with_postgres(&conn_str)
    }

    /// Check if runner is connected and ready
    pub fn is_connected(&self) -> bool {
        // Try a simple query to verify connection
        self.conn.execute("SELECT 1", []).is_ok()
    }

    /// Load the PostgreSQL extension
    ///
    /// Installs and loads the DuckDB postgres extension for writing
    /// to TimescaleDB. This must be called before `attach_postgres()`.
    pub fn load_postgres_extension(&mut self) -> Result<(), EtlError> {
        if self.postgres_loaded {
            debug!("PostgreSQL extension already loaded");
            return Ok(());
        }

        info!("Loading PostgreSQL extension");

        // Install the postgres extension
        self.conn
            .execute_batch("INSTALL postgres; LOAD postgres;")
            .map_err(|e| EtlError::PostgresExtension(e.to_string()))?;

        self.postgres_loaded = true;
        debug!("PostgreSQL extension loaded successfully");

        Ok(())
    }

    /// Attach PostgreSQL database as 'pg'
    fn attach_postgres(&mut self) -> Result<(), EtlError> {
        if self.postgres_attached {
            debug!("PostgreSQL already attached");
            return Ok(());
        }

        let pg_conn_str = self.pg_conn_str.as_ref().ok_or_else(|| {
            EtlError::PostgresAttach("No PostgreSQL connection string configured".to_string())
        })?;

        // Parse the connection string to extract components for DuckDB attach
        let attach_sql = format!("ATTACH '{}' AS pg (TYPE postgres)", pg_conn_str);

        info!("Attaching PostgreSQL database");

        self.conn
            .execute_batch(&attach_sql)
            .map_err(|e| EtlError::PostgresAttach(e.to_string()))?;

        // Verify connection with a simple query
        self.conn
            .execute("SELECT 1 FROM pg.information_schema.tables LIMIT 1", [])
            .map_err(|e| {
                EtlError::PostgresAttach(format!("Connection verification failed: {}", e))
            })?;

        self.postgres_attached = true;
        info!("PostgreSQL attached successfully");

        Ok(())
    }

    /// Resolve Parquet files for a stream
    ///
    /// Finds all Parquet files matching the pattern:
    /// `{bronze_path}/{stream_id}/**/*.parquet`
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier (e.g., "air-quality")
    /// * `bronze_path` - Base path to Bronze data (e.g., "/data/raw")
    ///
    /// # Returns
    ///
    /// Vector of absolute paths to Parquet files, empty if none found.
    pub fn resolve_parquet_files(
        &self,
        stream_id: &str,
        bronze_path: &str,
    ) -> Result<Vec<String>, EtlError> {
        let stream_dir = Path::new(bronze_path).join(stream_id);

        debug!(
            stream_id = %stream_id,
            stream_dir = %stream_dir.display(),
            "Resolving Parquet files"
        );

        // Check if directory exists
        if !stream_dir.exists() {
            debug!(
                stream_id = %stream_id,
                "Stream directory does not exist, returning empty list"
            );
            return Ok(Vec::new());
        }

        // Use recursive directory walking to find all parquet files
        let mut files = Vec::new();
        find_parquet_files_recursive(&stream_dir, &mut files)?;

        debug!(
            stream_id = %stream_id,
            file_count = files.len(),
            "Found Parquet files"
        );

        Ok(files)
    }

    /// Get the current watermark (max timestamp) from target table
    ///
    /// # Arguments
    ///
    /// * `table` - Fully qualified table name (e.g., "silver.air_quality_observations")
    /// * `column` - Watermark column name (e.g., "observation_time")
    ///
    /// # Returns
    ///
    /// `None` if table is empty or doesn't exist, otherwise the max timestamp.
    pub fn get_watermark(
        &self,
        table: &str,
        column: &str,
    ) -> Result<Option<DateTime<Utc>>, EtlError> {
        if !self.postgres_attached {
            return Err(EtlError::Watermark {
                table: table.to_string(),
                message: "PostgreSQL not attached".to_string(),
            });
        }

        let sql = format!("SELECT MAX({}) AS watermark FROM pg.{}", column, table);

        debug!(table = %table, column = %column, "Querying watermark");

        match self.conn.query_row(&sql, [], |row| {
            // DuckDB returns timestamps as strings, parse them
            let watermark_str: Option<String> = row.get(0)?;
            Ok(watermark_str)
        }) {
            Ok(Some(ts_str)) => {
                // Parse the timestamp string
                match DateTime::parse_from_rfc3339(&ts_str) {
                    Ok(dt) => {
                        let watermark = dt.with_timezone(&Utc);
                        debug!(table = %table, watermark = %watermark, "Retrieved watermark");
                        Ok(Some(watermark))
                    }
                    Err(_) => {
                        // Try alternative parsing
                        match chrono::NaiveDateTime::parse_from_str(&ts_str, "%Y-%m-%d %H:%M:%S%.f")
                        {
                            Ok(naive) => {
                                let watermark = DateTime::from_naive_utc_and_offset(naive, Utc);
                                debug!(table = %table, watermark = %watermark, "Retrieved watermark");
                                Ok(Some(watermark))
                            }
                            Err(e) => {
                                warn!(
                                    table = %table,
                                    timestamp = %ts_str,
                                    error = %e,
                                    "Failed to parse watermark timestamp"
                                );
                                Ok(None)
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                debug!(table = %table, "No watermark found (table empty)");
                Ok(None)
            }
            Err(e) => {
                // Check if table doesn't exist
                if e.to_string().contains("does not exist") {
                    debug!(table = %table, "Table does not exist, no watermark");
                    Ok(None)
                } else {
                    Err(EtlError::Watermark {
                        table: table.to_string(),
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    /// Run ETL for a stream
    ///
    /// Executes the complete ETL pipeline:
    /// 1. Resolve Parquet files
    /// 2. Get current watermark (if incremental)
    /// 3. Generate SQL from config
    /// 4. Execute SQL
    /// 5. Collect statistics
    ///
    /// # Arguments
    ///
    /// * `config` - Silver ETL configuration
    /// * `stream_id` - Stream identifier
    /// * `bronze_path` - Path to Bronze data
    pub fn run_etl(
        &self,
        config: &SilverEtlConfig,
        stream_id: &str,
        bronze_path: &str,
    ) -> Result<EtlStats, EtlError> {
        let start = Instant::now();

        info!(stream_id = %stream_id, "Starting ETL run");

        // Check if there are any files to process
        let files = self.resolve_parquet_files(stream_id, bronze_path)?;
        if files.is_empty() {
            info!(stream_id = %stream_id, "No Parquet files found, nothing to process");
            return Ok(EtlStats {
                stream_id: stream_id.to_string(),
                rows_processed: 0,
                rows_with_dq_flags: 0,
                rows_rejected: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                watermark_before: None,
                watermark_after: None,
            });
        }

        // Get current watermark if incremental
        let watermark_before = if config.incremental.enabled && self.postgres_attached {
            self.get_watermark(&config.target_table, &config.incremental.watermark_column)?
        } else {
            None
        };

        // Check if pre-transform is enabled
        // TODO (dp-007): When pre_transform.rs is implemented, call apply_pre_transform here
        // to populate the pre_transformed temp table before SQL generation
        let use_pre_transform = config.pre_transform.is_some();
        if use_pre_transform {
            info!(stream_id = %stream_id, "Pre-transform enabled - will use pre_transformed temp table");
            // Future: self.apply_pre_transform_if_needed(&config, stream_id, bronze_path)?;
        }

        // Generate ETL SQL
        let sql_gen = SqlGenerator::new();
        let dq_gen = DqSqlGenerator::new();

        let sql = self
            .generate_full_etl_sql(
                &sql_gen,
                &dq_gen,
                config,
                stream_id,
                bronze_path,
                watermark_before,
                use_pre_transform,
            )
            .map_err(|e| EtlError::SqlGeneration(e.to_string()))?;

        debug!(stream_id = %stream_id, "Generated ETL SQL:\n{}", sql);

        // Execute the SQL
        let rows_processed = if self.postgres_attached {
            self.conn
                .execute(&sql, [])
                .map_err(|e| EtlError::SqlExecution(e.to_string()))? as u64
        } else {
            // In-memory mode: just validate the SQL by preparing it
            debug!(stream_id = %stream_id, "In-memory mode: validating SQL only");
            0
        };

        // Get new watermark
        let watermark_after = if config.incremental.enabled && self.postgres_attached {
            self.get_watermark(&config.target_table, &config.incremental.watermark_column)?
        } else {
            None
        };

        // Query DQ statistics if enabled
        let (rows_with_dq_flags, rows_rejected) =
            if config.dq_output.enabled && self.postgres_attached {
                self.query_dq_stats(
                    &config.target_table,
                    &config.dq_output.target_column,
                    watermark_before,
                    watermark_after,
                )?
            } else {
                (0, 0)
            };

        let duration_ms = start.elapsed().as_millis() as u64;

        let stats = EtlStats {
            stream_id: stream_id.to_string(),
            rows_processed,
            rows_with_dq_flags,
            rows_rejected,
            duration_ms,
            watermark_before,
            watermark_after,
        };

        info!(
            stream_id = %stream_id,
            rows_processed = stats.rows_processed,
            rows_flagged = stats.rows_with_dq_flags,
            duration_ms = stats.duration_ms,
            "ETL run completed"
        );

        Ok(stats)
    }

    /// Generate SQL without executing (dry-run)
    ///
    /// Useful for debugging and validating ETL configuration.
    pub fn dry_run(
        &self,
        config: &SilverEtlConfig,
        stream_id: &str,
        bronze_path: &str,
    ) -> Result<String, EtlError> {
        info!(stream_id = %stream_id, "Generating ETL SQL (dry-run)");

        // Check if pre-transform is enabled
        let use_pre_transform = config.pre_transform.is_some();

        let sql_gen = SqlGenerator::new();
        let dq_gen = DqSqlGenerator::new();

        self.generate_full_etl_sql(
            &sql_gen,
            &dq_gen,
            config,
            stream_id,
            bronze_path,
            None,
            use_pre_transform,
        )
        .map_err(|e| EtlError::SqlGeneration(e.to_string()))
    }

    /// Generate complete ETL SQL statement
    ///
    /// # Arguments
    ///
    /// * `sql_gen` - SQL generator for field expressions
    /// * `dq_gen` - DQ SQL generator for quality flags
    /// * `config` - Silver ETL configuration
    /// * `stream_id` - Stream identifier
    /// * `bronze_path` - Path to Bronze data
    /// * `watermark` - Optional watermark for incremental loads
    /// * `use_pre_transform` - If true, select from `pre_transformed` table instead of Parquet
    fn generate_full_etl_sql(
        &self,
        sql_gen: &SqlGenerator,
        dq_gen: &DqSqlGenerator,
        config: &SilverEtlConfig,
        stream_id: &str,
        bronze_path: &str,
        watermark: Option<DateTime<Utc>>,
        use_pre_transform: bool,
    ) -> Result<String, EtlError> {
        let mut sql_parts = Vec::new();

        // Build column list
        let mut columns = Vec::new();
        let mut select_exprs = Vec::new();

        // Add ingestion_time
        columns.push("ingestion_time".to_string());
        select_exprs.push("current_timestamp AS ingestion_time".to_string());

        // Add timestamp mapping
        columns.push(config.timestamp.target_field.clone());
        let ts_expr = sql_gen.generate_timestamp_expr(&config.timestamp);
        select_exprs.push(ts_expr);

        // Add identity fields
        for field in &config.identity_fields {
            columns.push(field.target.clone());
            let expr = sql_gen.generate_identity_expr(field);
            select_exprs.push(expr);
        }

        // Add field mappings
        for mapping in &config.field_mappings {
            columns.push(mapping.target_column.clone());
            let expr = sql_gen.generate_select_expr(mapping);
            select_exprs.push(expr);
        }

        // Add DQ flags if enabled
        if config.dq_output.enabled {
            columns.push(config.dq_output.target_column.clone());
            let dq_expr = dq_gen.generate_dq_flags_expr_from_config(config);
            select_exprs.push(dq_expr);
        }

        // Build INSERT INTO
        let target_table = if self.postgres_attached {
            format!("pg.{}", config.target_table)
        } else {
            config.target_table.clone()
        };

        sql_parts.push(format!(
            "INSERT INTO {} ({})",
            target_table,
            columns.join(", ")
        ));

        // Build SELECT
        sql_parts.push(format!("SELECT\n    {}", select_exprs.join(",\n    ")));

        // Build FROM clause - use pre_transformed table if pre-transform is enabled
        if use_pre_transform {
            // Pre-transform stage has already populated a temp table with flattened data
            sql_parts.push("FROM pre_transformed".to_string());
            debug!(stream_id = %stream_id, "Using pre_transformed temp table as source");
        } else {
            // Standard path: read directly from Bronze Parquet files
            let parquet_glob = format!("{}/{}/**/*.parquet", bronze_path, stream_id);
            sql_parts.push(format!("FROM read_parquet('{}')", parquet_glob));
        }

        // Build WHERE clause for incremental
        // Note: When using pre_transformed, watermark filtering should already be applied
        // during the pre-transform stage, but we apply it here as well for safety
        if config.incremental.enabled {
            if let Some(wm) = watermark {
                let lag_seconds = parse_interval_to_seconds(&config.incremental.lag_interval);
                let adjusted_wm = wm - chrono::Duration::seconds(lag_seconds);

                // Convert to microseconds for Bronze timestamp comparison
                let wm_micros = adjusted_wm.timestamp_micros();
                sql_parts.push(format!(
                    "WHERE {} > {}",
                    config.timestamp.source_field, wm_micros
                ));
            }
        }

        // Build ON CONFLICT for deduplication
        if config.deduplication.enabled && !config.deduplication.key_columns.is_empty() {
            let key_cols = config.deduplication.key_columns.join(", ");
            let update_cols: Vec<String> = columns
                .iter()
                .filter(|c| !config.deduplication.key_columns.contains(c))
                .map(|c| format!("{} = EXCLUDED.{}", c, c))
                .collect();

            match config.deduplication.strategy {
                DeduplicationStrategy::Upsert => {
                    sql_parts.push(format!(
                        "ON CONFLICT ({}) DO UPDATE SET {}",
                        key_cols,
                        update_cols.join(", ")
                    ));
                }
                DeduplicationStrategy::Skip => {
                    sql_parts.push(format!("ON CONFLICT ({}) DO NOTHING", key_cols));
                }
                DeduplicationStrategy::Replace => {
                    // Replace uses same syntax as upsert
                    sql_parts.push(format!(
                        "ON CONFLICT ({}) DO UPDATE SET {}",
                        key_cols,
                        update_cols.join(", ")
                    ));
                }
            }
        }

        Ok(sql_parts.join("\n"))
    }

    /// Query DQ statistics for processed rows
    fn query_dq_stats(
        &self,
        table: &str,
        dq_column: &str,
        watermark_before: Option<DateTime<Utc>>,
        watermark_after: Option<DateTime<Utc>>,
    ) -> Result<(u64, u64), EtlError> {
        // Count rows with any DQ flags in the new data window
        let mut sql = format!(
            r#"SELECT
                COUNT(*) FILTER (WHERE array_length({}, 1) > 0) AS flagged,
                COUNT(*) FILTER (WHERE array_length(
                    ARRAY(SELECT unnest({}) WHERE unnest LIKE '%:reject%'), 1
                ) > 0) AS rejected
            FROM pg.{}"#,
            dq_column, dq_column, table
        );

        // Add time window filter if we have watermarks
        if let (Some(_before), Some(after)) = (watermark_before, watermark_after) {
            sql.push_str(&format!(
                " WHERE observation_time <= '{}'",
                after.to_rfc3339()
            ));
        }

        match self.conn.query_row(&sql, [], |row| {
            let flagged: i64 = row.get(0).unwrap_or(0);
            let rejected: i64 = row.get(1).unwrap_or(0);
            Ok((flagged as u64, rejected as u64))
        }) {
            Ok(stats) => Ok(stats),
            Err(e) => {
                warn!(error = %e, "Failed to query DQ stats, returning zeros");
                Ok((0, 0))
            }
        }
    }
}

// =============================================================================
// ETL Statistics
// =============================================================================

/// ETL execution statistics
#[derive(Debug, Clone)]
pub struct EtlStats {
    /// Stream that was processed
    pub stream_id: String,

    /// Number of rows processed
    pub rows_processed: u64,

    /// Number of rows with DQ flags
    pub rows_with_dq_flags: u64,

    /// Number of rows rejected
    pub rows_rejected: u64,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Watermark before ETL run
    pub watermark_before: Option<DateTime<Utc>>,

    /// Watermark after ETL run
    pub watermark_after: Option<DateTime<Utc>>,
}

impl EtlStats {
    /// Check if any rows were processed
    pub fn is_empty(&self) -> bool {
        self.rows_processed == 0
    }

    /// Get the percentage of flagged rows
    pub fn flagged_percentage(&self) -> f64 {
        if self.rows_processed == 0 {
            0.0
        } else {
            (self.rows_with_dq_flags as f64 / self.rows_processed as f64) * 100.0
        }
    }
}

// =============================================================================
// Bronze Raw Data (for pre-transform)
// =============================================================================

/// Raw data extracted from Bronze Parquet for pre-transformation
///
/// Used when pre-transform is enabled to extract the raw JSON payloads
/// along with timestamps and identifiers before applying the parser.
#[derive(Debug, Default)]
pub struct BronzeRawData {
    /// Timestamps from Bronze layer (microseconds since epoch)
    pub timestamps: Vec<i64>,
    /// NDP IDs from Bronze layer
    pub ndp_ids: Vec<Option<String>>,
    /// Raw JSON payloads from Bronze layer
    pub raw_payloads: Vec<serde_json::Value>,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Extract host from PostgreSQL connection string for logging
fn extract_host(conn_str: &str) -> &str {
    // Format: postgresql://user:pass@host:port/db
    if let Some(at_pos) = conn_str.find('@') {
        let after_at = &conn_str[at_pos + 1..];
        if let Some(colon_pos) = after_at.find(':') {
            return &after_at[..colon_pos];
        }
        if let Some(slash_pos) = after_at.find('/') {
            return &after_at[..slash_pos];
        }
        return after_at;
    }
    "unknown"
}

/// Recursively find Parquet files in a directory
fn find_parquet_files_recursive(dir: &Path, files: &mut Vec<String>) -> Result<(), EtlError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            find_parquet_files_recursive(&path, files)?;
        } else if path.extension().map(|e| e == "parquet").unwrap_or(false) {
            files.push(path.to_string_lossy().to_string());
        }
    }

    Ok(())
}

/// Parse interval string to seconds
///
/// Supports formats like "5 minutes", "1 hour", "30 seconds"
fn parse_interval_to_seconds(interval: &str) -> i64 {
    let parts: Vec<&str> = interval.split_whitespace().collect();
    if parts.len() != 2 {
        warn!(interval = %interval, "Invalid interval format, defaulting to 300s");
        return 300;
    }

    let value: i64 = parts[0].parse().unwrap_or(5);
    let unit = parts[1].to_lowercase();

    match unit.as_str() {
        "second" | "seconds" | "s" => value,
        "minute" | "minutes" | "m" => value * 60,
        "hour" | "hours" | "h" => value * 3600,
        "day" | "days" | "d" => value * 86400,
        _ => {
            warn!(unit = %unit, "Unknown interval unit, defaulting to minutes");
            value * 60
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::config::{
        DeduplicationConfig, DqOutputConfig, IdentityField, IncrementalConfig, SilverFieldMapping,
        TimestampMapping, TimestampTransform,
    };
    use tempfile::TempDir;

    // ============================================================
    // Test 1: DuckDB connection initializes
    // ============================================================
    #[test]
    fn test_duckdb_connection_initializes() {
        let runner = EtlRunner::new_in_memory().expect("Should create in-memory DuckDB connection");

        assert!(runner.is_connected());
    }

    // ============================================================
    // Test 2: PostgreSQL extension loads (requires integration)
    // ============================================================
    #[test]
    #[ignore] // Requires DuckDB with postgres extension
    fn test_postgres_extension_loads() {
        let mut runner = EtlRunner::new_in_memory().expect("Should create connection");

        let result = runner.load_postgres_extension();
        assert!(result.is_ok(), "Should load postgres extension");
    }

    // ============================================================
    // Test 3: Parquet glob resolves files
    // ============================================================
    #[test]
    fn test_parquet_glob_resolves_files() {
        let temp_dir = TempDir::new().unwrap();
        let stream_dir = temp_dir
            .path()
            .join("air-quality/year=2026/month=01/day=10");
        std::fs::create_dir_all(&stream_dir).unwrap();

        // Create a minimal parquet file (empty file for test)
        let parquet_path = stream_dir.join("data.parquet");
        create_test_parquet(&parquet_path);

        let runner = EtlRunner::new_in_memory().unwrap();
        let files = runner
            .resolve_parquet_files("air-quality", temp_dir.path().to_str().unwrap())
            .unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("data.parquet"));
    }

    // ============================================================
    // Test 4: Watermark query returns correct value
    // ============================================================
    #[test]
    #[ignore] // Requires PostgreSQL
    fn test_watermark_query_returns_max_timestamp() {
        // This test requires a real PostgreSQL connection
        // Run with: cargo test --ignored
    }

    // ============================================================
    // Test 5: ETL handles empty data
    // ============================================================
    #[test]
    fn test_etl_handles_empty_data() {
        let temp_dir = TempDir::new().unwrap();
        // No parquet files created

        let config = create_test_silver_config();
        let runner = EtlRunner::new_in_memory().unwrap();

        let result = runner.run_etl(&config, "air-quality", temp_dir.path().to_str().unwrap());

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.rows_processed, 0);
    }

    // ============================================================
    // Test 6: ETL stats have correct structure
    // ============================================================
    #[test]
    fn test_etl_stats_structure() {
        let stats = EtlStats {
            stream_id: "air-quality".to_string(),
            rows_processed: 100,
            rows_with_dq_flags: 5,
            rows_rejected: 2,
            duration_ms: 1500,
            watermark_before: None,
            watermark_after: Some(Utc::now()),
        };

        assert_eq!(stats.rows_processed, 100);
        assert_eq!(stats.rows_with_dq_flags, 5);
        assert!(stats.duration_ms > 0);
        assert!(!stats.is_empty());
        assert!((stats.flagged_percentage() - 5.0).abs() < 0.01);
    }

    // ============================================================
    // Test 7: Error handling - missing parquet files returns empty
    // ============================================================
    #[test]
    fn test_missing_parquet_files_returns_empty() {
        let runner = EtlRunner::new_in_memory().unwrap();

        let result = runner.resolve_parquet_files("nonexistent-stream", "/nonexistent/path");

        // Should return empty vec, not error
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ============================================================
    // Test 8: Dry-run generates SQL
    // ============================================================
    #[test]
    fn test_dry_run_generates_sql() {
        let config = create_test_silver_config();
        let runner = EtlRunner::new_in_memory().unwrap();

        let sql = runner
            .dry_run(&config, "air-quality", "/data/raw")
            .expect("Should generate SQL");

        // Verify SQL structure
        assert!(sql.contains("INSERT INTO"));
        assert!(sql.contains("silver.air_quality"));
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM read_parquet"));
    }

    // ============================================================
    // Test 9: Parse interval to seconds
    // ============================================================
    #[test]
    fn test_parse_interval_to_seconds() {
        assert_eq!(parse_interval_to_seconds("5 minutes"), 300);
        assert_eq!(parse_interval_to_seconds("1 hour"), 3600);
        assert_eq!(parse_interval_to_seconds("30 seconds"), 30);
        assert_eq!(parse_interval_to_seconds("2 hours"), 7200);
        assert_eq!(parse_interval_to_seconds("1 day"), 86400);

        // Invalid formats default to 300
        assert_eq!(parse_interval_to_seconds("invalid"), 300);
    }

    // ============================================================
    // Test 10: Extract host from connection string
    // ============================================================
    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("postgresql://user:pass@localhost:5432/db"),
            "localhost"
        );
        assert_eq!(
            extract_host("postgresql://user:pass@timescale.local:5432/ndp"),
            "timescale.local"
        );
        assert_eq!(extract_host("invalid"), "unknown");
    }

    // ============================================================
    // Test 11: EtlStats empty check
    // ============================================================
    #[test]
    fn test_etl_stats_empty() {
        let empty_stats = EtlStats {
            stream_id: "test".to_string(),
            rows_processed: 0,
            rows_with_dq_flags: 0,
            rows_rejected: 0,
            duration_ms: 100,
            watermark_before: None,
            watermark_after: None,
        };

        assert!(empty_stats.is_empty());
        assert_eq!(empty_stats.flagged_percentage(), 0.0);
    }

    // Helper functions

    fn create_test_parquet(path: &std::path::Path) {
        use polars::prelude::*;

        let df = df! {
            "timestamp" => &[1704886800000000_i64],
            "ndp_id" => &["test-sensor"],
            "source_id" => &["mqtt://test"],
            "context" => &[r#"{"location":{"path":"test"}}"#],
            "raw_payload" => &[r#"{"pm02":25.5}"#]
        }
        .unwrap();

        let file = std::fs::File::create(path).unwrap();
        ParquetWriter::new(file).finish(&mut df.clone()).unwrap();
    }

    fn create_test_silver_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![IdentityField {
                source: "ndp_id".to_string(),
                target: "ndp_id".to_string(),
            }],
            field_mappings: vec![SilverFieldMapping {
                source_path: "raw_payload.pm02".to_string(),
                target_column: "pm25".to_string(),
                column_type: "double_precision".to_string(),
                nullable: true,
                transform: None,
                dq_rules: vec![],
            }],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        }
    }

    // ============================================================
    // Pre-Transform Integration Tests (dp-007)
    // ============================================================

    // ============================================================
    // Test 12: Pre-transform not applied when disabled (no pre_transform config)
    // ============================================================
    #[test]
    fn test_pre_transform_not_applied_when_disabled() {
        // Config without pre_transform section (None)
        let config = create_test_silver_config();
        assert!(config.pre_transform.is_none());

        let runner = EtlRunner::new_in_memory().unwrap();

        // Dry run should produce SQL using read_parquet, not pre_transformed table
        let sql = runner
            .dry_run(&config, "air-quality", "/data/raw")
            .expect("Should generate SQL");

        // Should use read_parquet since pre-transform is disabled
        assert!(
            sql.contains("FROM read_parquet"),
            "SQL should use read_parquet when pre-transform is disabled"
        );
        assert!(
            !sql.contains("FROM pre_transformed"),
            "SQL should NOT use pre_transformed table when pre-transform is disabled"
        );
    }

    // ============================================================
    // Test 13: Pre-transform applied when enabled
    // ============================================================
    #[test]
    fn test_pre_transform_applied_when_enabled() {
        use neural_core::config::{
            ArrayExplosionConfig, MetricExplosionMapping, PreTransformConfig, PreTransformType,
        };

        // Config with pre_transform enabled
        let mut config = create_test_silver_config();
        config.pre_transform = Some(PreTransformConfig {
            transform_type: PreTransformType::ArrayExplosion(ArrayExplosionConfig {
                metrics_base_path: "properties".to_string(),
                timestamp_field: "validTime".to_string(),
                value_field: "value".to_string(),
                values_path: "values".to_string(),
                metrics: vec![MetricExplosionMapping {
                    metric_path: "temperature".to_string(),
                    target_column: "temperature_c".to_string(),
                    column_type: "double_precision".to_string(),
                }],
            }),
        });

        assert!(config.pre_transform.is_some());

        let runner = EtlRunner::new_in_memory().unwrap();

        // Dry run should produce SQL using pre_transformed table
        let sql = runner
            .dry_run(&config, "nws-gridpoints-forecast", "/data/raw")
            .expect("Should generate SQL");

        // Should use pre_transformed table since pre-transform is enabled
        assert!(
            sql.contains("FROM pre_transformed"),
            "SQL should use pre_transformed table when pre-transform is enabled. Got: {}",
            sql
        );
        assert!(
            !sql.contains("FROM read_parquet"),
            "SQL should NOT use read_parquet when pre-transform is enabled"
        );
    }

    // ============================================================
    // Test 14: SQL uses temp table after pre-transform
    // ============================================================
    #[test]
    fn test_sql_uses_temp_table_after_pre_transform() {
        use neural_core::config::{
            ArrayExplosionConfig, MetricExplosionMapping, PreTransformConfig, PreTransformType,
        };

        let mut config = create_test_silver_config();
        config.target_table = "silver.nws_forecasts".to_string();
        config.pre_transform = Some(PreTransformConfig {
            transform_type: PreTransformType::ArrayExplosion(ArrayExplosionConfig {
                metrics_base_path: "properties".to_string(),
                timestamp_field: "validTime".to_string(),
                value_field: "value".to_string(),
                values_path: "values".to_string(),
                metrics: vec![
                    MetricExplosionMapping {
                        metric_path: "temperature".to_string(),
                        target_column: "temperature_c".to_string(),
                        column_type: "double_precision".to_string(),
                    },
                    MetricExplosionMapping {
                        metric_path: "windSpeed".to_string(),
                        target_column: "wind_speed_ms".to_string(),
                        column_type: "double_precision".to_string(),
                    },
                ],
            }),
        });

        let runner = EtlRunner::new_in_memory().unwrap();
        let sql = runner
            .dry_run(&config, "nws-gridpoints-forecast", "/data/raw")
            .expect("Should generate SQL");

        // Verify SQL structure uses temp table
        assert!(sql.contains("INSERT INTO"));
        assert!(sql.contains("silver.nws_forecasts"));
        assert!(
            sql.contains("FROM pre_transformed"),
            "Generated SQL should select from pre_transformed table"
        );
    }

    // ============================================================
    // Test 15: BronzeRawData structure is correct
    // ============================================================
    #[test]
    fn test_bronze_raw_data_default() {
        let data = BronzeRawData::default();
        assert!(data.timestamps.is_empty());
        assert!(data.ndp_ids.is_empty());
        assert!(data.raw_payloads.is_empty());
    }
}
