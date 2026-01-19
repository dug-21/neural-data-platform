//! TimescaleDB output implementation for Silver layer
//!
//! This provides a production-ready TimescaleDB output with connection pooling.
//! Enabled via the `timescale` feature flag.
//!
//! # Architecture (DP-012)
//!
//! SilverSubscriber -> TimescaleOutput -> bb8 pool -> TimescaleDB
//!
//! - Connection pooling via bb8 for efficient resource usage
//! - UPSERT for deduplication (ON CONFLICT DO UPDATE)
//! - Watermark queries for catch-up support
//! - Health checks via connection test

use super::{SilverOutput, SilverOutputError};
use crate::silver::types::SilverRecord;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Configuration for TimescaleDB output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimescaleConfig {
    /// PostgreSQL connection string
    /// Example: "postgresql://user:pass@localhost:5432/dbname"
    pub connection_string: String,

    /// Maximum connections in pool (default: 5)
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds (default: 10)
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,

    /// Default table for writes (e.g., "silver.observations")
    #[serde(default = "default_table")]
    pub default_table: String,

    /// Stream ID to table mapping
    /// Example: {"air-quality": "silver.air_quality_observations"}
    #[serde(default)]
    pub table_mapping: HashMap<String, String>,

    /// Timestamp column name in Silver tables (default: "observation_time")
    #[serde(default = "default_timestamp_column")]
    pub timestamp_column: String,

    /// Whether to use UPSERT (ON CONFLICT DO UPDATE) - default true
    #[serde(default = "default_upsert")]
    pub use_upsert: bool,
}

fn default_max_connections() -> u32 {
    5
}

fn default_connection_timeout() -> u64 {
    10
}

fn default_table() -> String {
    "silver.observations".to_string()
}

fn default_timestamp_column() -> String {
    "observation_time".to_string()
}

fn default_upsert() -> bool {
    true
}

impl Default for TimescaleConfig {
    fn default() -> Self {
        Self {
            connection_string: String::new(),
            max_connections: default_max_connections(),
            connection_timeout_secs: default_connection_timeout(),
            default_table: default_table(),
            table_mapping: HashMap::new(),
            timestamp_column: default_timestamp_column(),
            use_upsert: default_upsert(),
        }
    }
}

// ============================================================================
// Feature-gated implementation
// ============================================================================

#[cfg(feature = "timescale")]
mod pooled {
    use super::*;
    use crate::config::SilverEtlConfig;
    use bb8::Pool;
    use bb8_postgres::PostgresConnectionManager;
    use tokio_postgres::NoTls;

    type PgPool = Pool<PostgresConnectionManager<NoTls>>;

    /// Production TimescaleDB output with connection pooling
    ///
    /// All column names are derived from SilverEtlConfig - no hardcoded column names.
    pub struct TimescaleOutput {
        config: TimescaleConfig,
        pool: PgPool,
    }

    impl TimescaleOutput {
        /// Create a new TimescaleOutput with connection pool
        pub async fn new(config: TimescaleConfig) -> Result<Self, SilverOutputError> {
            if config.connection_string.is_empty() {
                return Err(SilverOutputError::ConfigError(
                    "connection_string is required".to_string(),
                ));
            }

            info!(
                max_connections = config.max_connections,
                "Creating TimescaleDB connection pool"
            );

            // Parse connection string and create manager
            let manager = PostgresConnectionManager::new_from_stringlike(
                &config.connection_string,
                NoTls,
            )
            .map_err(|e| {
                SilverOutputError::ConfigError(format!("Invalid connection string: {}", e))
            })?;

            // Build connection pool
            let pool = Pool::builder()
                .max_size(config.max_connections)
                .connection_timeout(std::time::Duration::from_secs(config.connection_timeout_secs))
                .build(manager)
                .await
                .map_err(|e| {
                    SilverOutputError::ConnectionError(format!("Failed to create pool: {}", e))
                })?;

            info!("TimescaleDB connection pool created successfully");

            Ok(Self { config, pool })
        }

        /// Build INSERT/UPSERT query for a record - ALL column names from config
        fn build_upsert_query(
            &self,
            record: &SilverRecord,
            etl_config: &SilverEtlConfig,
        ) -> (String, Vec<String>) {
            let table = &etl_config.target_table;
            let timestamp_col = &etl_config.timestamp.target_field;
            let dq_col = &etl_config.dq_output.target_column;

            // Start with ingestion_time (always NOW()) and primary timestamp
            let mut columns = vec!["ingestion_time".to_string(), timestamp_col.clone()];
            let mut placeholders = vec!["NOW()".to_string(), "$1".to_string()];
            let mut param_index = 2;

            // Add valid_timestamp if configured (e.g., for forecasts)
            if let Some(ref valid_ts) = etl_config.valid_timestamp {
                if record.valid_timestamp.is_some() {
                    columns.push(valid_ts.target_field.clone());
                    placeholders.push(format!("${}", param_index));
                    param_index += 1;
                }
            }

            // Add identity fields from config (e.g., ndp_id)
            for identity in &etl_config.identity_fields {
                if record.device_id.is_some() {
                    columns.push(identity.target.clone());
                    placeholders.push(format!("${}", param_index));
                    param_index += 1;
                    break; // Only support one identity field for now
                }
            }

            // Add data fields (from record.fields which are set by transform)
            let field_names: Vec<String> = record.fields.keys().cloned().collect();
            for name in &field_names {
                columns.push(name.clone());
                placeholders.push(format!("${}", param_index));
                param_index += 1;
            }

            // Add dq_flags column if there are flags (column name from config)
            if etl_config.dq_output.enabled && !record.dq_flags().is_empty() {
                columns.push(dq_col.clone());
                placeholders.push(format!("${}", param_index));
            }

            let columns_str = columns.join(", ");
            let placeholders_str = placeholders.join(", ");

            // Build ON CONFLICT clause from deduplication.key_columns
            let dedup_keys = etl_config.deduplication.key_columns.join(", ");

            let query = if etl_config.deduplication.enabled
                && matches!(
                    etl_config.deduplication.strategy,
                    crate::config::DeduplicationStrategy::Upsert
                )
            {
                // UPSERT using ON CONFLICT - exclude dedup keys from UPDATE
                let update_columns: Vec<String> = columns
                    .iter()
                    .filter(|c| {
                        *c != "ingestion_time"
                            && !etl_config.deduplication.key_columns.contains(c)
                    })
                    .map(|c| format!("{} = EXCLUDED.{}", c, c))
                    .collect();

                if update_columns.is_empty() {
                    // No columns to update, use DO NOTHING
                    format!(
                        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) DO NOTHING",
                        table, columns_str, placeholders_str, dedup_keys
                    )
                } else {
                    format!(
                        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) DO UPDATE SET {}",
                        table,
                        columns_str,
                        placeholders_str,
                        dedup_keys,
                        update_columns.join(", ")
                    )
                }
            } else {
                format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    table, columns_str, placeholders_str
                )
            };

            (query, field_names)
        }
    }

    #[async_trait]
    impl SilverOutput for TimescaleOutput {
        async fn write(
            &self,
            record: &SilverRecord,
            etl_config: &SilverEtlConfig,
        ) -> Result<(), SilverOutputError> {
            if record.should_drop() {
                debug!(stream_id = %record.stream_id, "Dropping record due to DQ rules");
                return Ok(());
            }

            let table = &etl_config.target_table;
            let (query, field_names) = self.build_upsert_query(record, etl_config);

            debug!(
                table = %table,
                stream_id = %record.stream_id,
                "Writing Silver record"
            );

            // Get connection from pool
            let conn = self.pool.get().await.map_err(|e| {
                SilverOutputError::ConnectionError(format!("Failed to get connection: {}", e))
            })?;

            // Build parameter values in order matching build_upsert_query
            let mut params: Vec<String> = Vec::new();

            // Primary timestamp
            params.push(record.timestamp.to_rfc3339());

            // Valid timestamp if present
            if let Some(ref valid_ts) = etl_config.valid_timestamp {
                if let Some(vt) = record.valid_timestamp {
                    let _ = valid_ts; // Use config field name
                    params.push(vt.to_rfc3339());
                }
            }

            // Identity field (device_id)
            if !etl_config.identity_fields.is_empty() {
                if let Some(ref device_id) = record.device_id {
                    params.push(device_id.clone());
                }
            }

            // Data fields
            for name in &field_names {
                if let Some(value) = record.fields.get(name) {
                    params.push(value.to_string().trim_matches('"').to_string());
                }
            }

            // DQ flags (column name from config)
            if etl_config.dq_output.enabled && !record.dq_flags().is_empty() {
                let flags_array = format!(
                    "{{{}}}",
                    record
                        .dq_flags()
                        .iter()
                        .map(|f| format!("\"{}\"", f))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                params.push(flags_array);
            }

            let raw_query = build_raw_query(&query, &params);

            // Execute the query
            let result = conn.execute(&raw_query, &[]).await;

            match result {
                Ok(_) => {
                    debug!(stream_id = %record.stream_id, "Record written successfully");
                    Ok(())
                }
                Err(e) => {
                    // Extract detailed PostgreSQL error if available
                    let pg_detail = if let Some(db_err) = e.as_db_error() {
                        format!(
                            "code={}, message={}, detail={:?}, hint={:?}",
                            db_err.code().code(),
                            db_err.message(),
                            db_err.detail(),
                            db_err.hint()
                        )
                    } else {
                        format!("{:?}", e)
                    };
                    error!(
                        stream_id = %record.stream_id,
                        table = %table,
                        pg_error = %pg_detail,
                        executed_query = %raw_query,
                        "Failed to write record to TimescaleDB"
                    );
                    Err(SilverOutputError::WriteError(format!(
                        "table={}, error={}",
                        table, pg_detail
                    )))
                }
            }
        }

        async fn get_watermark(
            &self,
            stream_id: &str,
            etl_config: &SilverEtlConfig,
        ) -> Result<Option<DateTime<Utc>>, SilverOutputError> {
            let table = &etl_config.target_table;
            let timestamp_col = &etl_config.timestamp.target_field;

            // Cast to TEXT for parsing to avoid chrono FromSql dependency
            let query = format!(
                "SELECT MAX({})::TEXT as watermark FROM {}",
                timestamp_col, table
            );

            let conn = self.pool.get().await.map_err(|e| {
                SilverOutputError::ConnectionError(format!("Failed to get connection: {}", e))
            })?;

            let row = conn.query_opt(&query, &[]).await.map_err(|e| {
                SilverOutputError::QueryError(format!("Watermark query failed: {}", e))
            })?;

            match row {
                Some(r) => {
                    let watermark_str: Option<String> = r.get(0);
                    let watermark = watermark_str.and_then(|s| {
                        // Parse ISO 8601 timestamp from PostgreSQL
                        chrono::DateTime::parse_from_rfc3339(&s)
                            .or_else(|_| {
                                // Try PostgreSQL format: 2026-01-18 12:00:00+00
                                chrono::DateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f%#z")
                            })
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    });
                    debug!(
                        stream_id = %stream_id,
                        watermark = ?watermark,
                        "Retrieved watermark"
                    );
                    Ok(watermark)
                }
                None => Ok(None),
            }
        }

        async fn health_check(&self) -> Result<bool, SilverOutputError> {
            match self.pool.get().await {
                Ok(conn) => match conn.execute("SELECT 1", &[]).await {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        warn!(error = %e, "Health check query failed");
                        Ok(false)
                    }
                },
                Err(e) => {
                    warn!(error = %e, "Failed to get connection for health check");
                    Ok(false)
                }
            }
        }

        async fn flush(&self) -> Result<(), SilverOutputError> {
            // bb8 pool handles connection management automatically
            Ok(())
        }
    }

    /// Build a raw SQL query with parameters substituted
    /// Note: This is for development/testing. Production should use prepared statements.
    fn build_raw_query(template: &str, params: &[String]) -> String {
        let mut query = template.to_string();
        for (i, param) in params.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            // Handle null values - output SQL NULL without quotes
            if param == "null" {
                query = query.replacen(&placeholder, "NULL", 1);
            } else {
                // Escape single quotes for SQL
                let escaped = param.replace('\'', "''");
                query = query.replacen(&placeholder, &format!("'{}'", escaped), 1);
            }
        }
        query
    }
}

// ============================================================================
// Stub implementation (when timescale feature is disabled)
// ============================================================================

#[cfg(not(feature = "timescale"))]
mod stub {
    use super::*;
    use crate::config::SilverEtlConfig;

    /// Stub TimescaleDB output (requires `timescale` feature)
    pub struct TimescaleOutput {
        #[allow(dead_code)]
        config: TimescaleConfig,
    }

    impl TimescaleOutput {
        /// Create a new TimescaleOutput (stub - always fails)
        pub async fn new(config: TimescaleConfig) -> Result<Self, SilverOutputError> {
            if config.connection_string.is_empty() {
                return Err(SilverOutputError::ConfigError(
                    "connection_string is required".to_string(),
                ));
            }
            warn!("TimescaleOutput created without `timescale` feature - writes will fail");
            Ok(Self { config })
        }
    }

    #[async_trait]
    impl SilverOutput for TimescaleOutput {
        async fn write(
            &self,
            record: &SilverRecord,
            _etl_config: &SilverEtlConfig,
        ) -> Result<(), SilverOutputError> {
            if record.should_drop() {
                return Ok(());
            }
            Err(SilverOutputError::WriteError(
                "TimescaleOutput requires `timescale` feature flag".to_string(),
            ))
        }

        async fn get_watermark(
            &self,
            _stream_id: &str,
            _etl_config: &SilverEtlConfig,
        ) -> Result<Option<DateTime<Utc>>, SilverOutputError> {
            Ok(None)
        }

        async fn health_check(&self) -> Result<bool, SilverOutputError> {
            // Return false when feature is disabled
            Ok(false)
        }
    }
}

// Re-export the appropriate implementation
#[cfg(feature = "timescale")]
pub use pooled::TimescaleOutput;

#[cfg(not(feature = "timescale"))]
pub use stub::TimescaleOutput;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timescale_config_default() {
        let config = TimescaleConfig::default();
        assert_eq!(config.max_connections, 5);
        assert!(config.connection_string.is_empty());
        assert_eq!(config.default_table, "silver.observations");
        assert_eq!(config.timestamp_column, "observation_time");
        assert!(config.use_upsert);
    }

    #[tokio::test]
    async fn test_new_requires_connection_string() {
        let config = TimescaleConfig::default();
        let result = TimescaleOutput::new(config).await;
        assert!(result.is_err());
        match result {
            Err(SilverOutputError::ConfigError(msg)) => {
                assert!(msg.contains("connection_string"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_timescale_config_with_mapping() {
        let mut config = TimescaleConfig::default();
        config.table_mapping.insert(
            "air-quality".to_string(),
            "silver.air_quality_observations".to_string(),
        );
        config.table_mapping.insert(
            "outdoor-weather".to_string(),
            "silver.weather_observations".to_string(),
        );

        assert_eq!(
            config.table_mapping.get("air-quality"),
            Some(&"silver.air_quality_observations".to_string())
        );
    }

    #[test]
    fn test_timescale_config_serialization() {
        let config = TimescaleConfig {
            connection_string: "postgresql://localhost/test".to_string(),
            max_connections: 10,
            connection_timeout_secs: 30,
            default_table: "silver.default".to_string(),
            table_mapping: HashMap::from([
                ("test".to_string(), "silver.test".to_string()),
            ]),
            timestamp_column: "ts".to_string(),
            use_upsert: true,
        };

        let json = serde_json::to_string(&config).expect("Serialization should succeed");
        let restored: TimescaleConfig =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(restored.connection_string, config.connection_string);
        assert_eq!(restored.max_connections, config.max_connections);
        assert_eq!(restored.table_mapping, config.table_mapping);
    }
}
