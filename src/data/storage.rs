use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

#[derive(Debug, Clone)]
pub struct TimescaleDBStorage {
    pub pool: PgPool,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TimeSeriesData {
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub entity: String,
    pub value: f64,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PredictionData {
    pub timestamp: DateTime<Utc>,
    pub entity: String,
    pub model_id: String,
    pub prediction_value: f64,
    pub confidence: f64,
    pub horizon_minutes: i32,
    pub features_used: Option<serde_json::Value>,
}

impl TimescaleDBStorage {
    /// Creates a new TimescaleDB storage instance with connection pooling.
    ///
    /// This function establishes a connection pool to the TimescaleDB database
    /// and configures it for optimal time series data operations.
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string (e.g., "postgres://user:pass@host:port/db")
    ///
    /// # Returns
    ///
    /// Returns `Ok(TimescaleDBStorage)` on successful connection, or an error if
    /// the database is unreachable or the connection string is invalid.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The database URL is malformed
    /// - The database server is unreachable
    /// - Authentication fails
    /// - The database doesn't exist
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use autonomous_platform::data::TimescaleDBStorage;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let storage = TimescaleDBStorage::new(
    ///     "postgres://neural_trader:password@localhost/neural_trader_db"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Create necessary tables and hypertables
    pub async fn create_tables(&self) -> Result<()> {
        // Create TimescaleDB extension
        sqlx::query("CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE")
            .execute(&self.pool)
            .await?;

        // Create time series data table (legacy format for compatibility)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS time_series_data (
                timestamp TIMESTAMPTZ NOT NULL,
                source VARCHAR(100) NOT NULL,
                entity VARCHAR(100) NOT NULL,
                value DOUBLE PRECISION NOT NULL,
                metadata JSONB,
                PRIMARY KEY (timestamp, entity, source)
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Convert to hypertable
        sqlx::query(
            r#"
            SELECT create_hypertable(
                'time_series_data', 
                'timestamp',
                if_not_exists => TRUE
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Create market data tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS market_data_raw (
                timestamp TIMESTAMPTZ NOT NULL,
                symbol VARCHAR(50) NOT NULL,
                open DOUBLE PRECISION NOT NULL,
                high DOUBLE PRECISION NOT NULL,
                low DOUBLE PRECISION NOT NULL,
                close DOUBLE PRECISION NOT NULL,
                volume DOUBLE PRECISION NOT NULL,
                PRIMARY KEY (timestamp, symbol)
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Convert to hypertable
        sqlx::query(
            r#"
            SELECT create_hypertable(
                'market_data_raw', 
                'timestamp',
                if_not_exists => TRUE
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Create predictions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS predictions (
                timestamp TIMESTAMPTZ NOT NULL,
                entity VARCHAR(100) NOT NULL,
                model_id VARCHAR(100) NOT NULL,
                prediction_value DOUBLE PRECISION NOT NULL,
                confidence DOUBLE PRECISION NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
                horizon_minutes INTEGER NOT NULL,
                features_used JSONB,
                PRIMARY KEY (timestamp, entity, model_id)
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Convert predictions to hypertable
        sqlx::query(
            r#"
            SELECT create_hypertable(
                'predictions', 
                'timestamp',
                if_not_exists => TRUE
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_time_series_entity_time ON time_series_data (entity, timestamp DESC)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_predictions_entity_model_time ON predictions (entity, model_id, timestamp DESC)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Stores a single time series data point in the database.
    ///
    /// This function inserts a time series data point into the TimescaleDB hypertable,
    /// using an upsert operation to handle duplicate timestamps gracefully.
    ///
    /// # Arguments
    ///
    /// * `data` - The time series data point to store
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful storage, or an error if the operation fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The database connection is lost
    /// - The data violates database constraints
    /// - A database-level error occurs
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use autonomous_platform::data::{TimescaleDBStorage, TimeSeriesData};
    /// use chrono::Utc;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let storage = TimescaleDBStorage::new("postgres://...").await?;
    ///
    /// let data = TimeSeriesData {
    ///     timestamp: Utc::now(),
    ///     source: "market_feed".to_string(),
    ///     entity: "BTCUSD".to_string(),
    ///     value: 50000.0,
    ///     metadata: None,
    /// };
    ///
    /// storage.store_time_series(&data).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn store_time_series(&self, data: &TimeSeriesData) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO time_series_data (timestamp, source, entity, value, metadata)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (timestamp, entity, source) 
            DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata
        "#,
        )
        .bind(data.timestamp)
        .bind(&data.source)
        .bind(&data.entity)
        .bind(data.value)
        .bind(&data.metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Query time series data by time range
    pub async fn query_range(
        &self,
        symbol: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TimeSeriesData>> {
        // Calculate the duration in days for proper minute data limit calculation
        let duration_days = (end - start).num_days().max(1); // Ensure at least 1 day
        
        // First try market_data_1h (hourly aggregated data)
        let hourly_results = sqlx::query(
            r#"
            SELECT bucket as timestamp, symbol, open, high, low, close, volume::float8 as volume
            FROM market_data_1h
            WHERE symbol = $1 
              AND bucket >= $2 
              AND bucket <= $3
            ORDER BY bucket ASC
        "#,
        )
        .bind(symbol)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in hourly_results {
            let timestamp: DateTime<Utc> = row.get("timestamp");
            let symbol: String = row.get("symbol");
            let open: f64 = row.get("open");
            let high: f64 = row.get("high");
            let low: f64 = row.get("low");
            let close: f64 = row.get("close");
            let volume: f64 = row.get("volume");

            results.push(TimeSeriesData {
                timestamp,
                source: "market_data_1h".to_string(),
                entity: symbol,
                value: close, // Use close price as the main value
                metadata: Some(serde_json::json!({
                    "open": open,
                    "high": high,
                    "low": low,
                    "close": close,
                    "volume": volume
                })),
            });
        }

        log::info!("Found {} records from market_data_1h for symbol {} between {} and {}", 
                  results.len(), symbol, start.format("%Y-%m-%d"), end.format("%Y-%m-%d"));

        // Calculate expected hourly records for the requested time period
        // Assuming ~8 hours of trading per day (typical market hours)
        let expected_hourly_records = duration_days * 8; // 8 trading hours per day
        let sufficient_data_threshold = (expected_hourly_records as f64 * 0.5) as usize; // At least 50% coverage

        // If no hourly data or insufficient hourly data, fall back to raw market_data table
        // This is the main table where historical data lives (130,593+ records)
        // Ensure we have meaningful data for training (minimum 100 records)
        let minimum_records_for_training = 100;
        if results.is_empty() || results.len() < sufficient_data_threshold || results.len() < minimum_records_for_training {
            if !results.is_empty() {
                log::info!("Hourly data insufficient ({} < {} expected), supplementing with raw data", 
                          results.len(), sufficient_data_threshold);
            }
            // Calculate appropriate limit for raw data based on the requested duration
            // Raw data can be high-frequency (potentially minute-level or tick-level)
            // Formula: days * estimated records per day (assume ~390 minutes trading day * records)
            // Add buffer for different market hours and data density variations
            let estimated_records_per_day = 400; // Conservative estimate
            let raw_data_limit = (duration_days * estimated_records_per_day) as i64;
            let buffered_limit = (raw_data_limit as f64 * 1.5) as i64; // 50% buffer
            
            // Cap the limit at reasonable maximum to prevent memory issues
            let final_limit = buffered_limit.min(500000); // Maximum ~1250 days of data
            
            log::info!("Falling back to raw market_data table for symbol {} with limit {} (duration: {} days)", 
                      symbol, final_limit, duration_days);
            
            let raw_results = sqlx::query(
                r#"
                SELECT timestamp, symbol, open, high, low, close, volume
                FROM market_data
                WHERE symbol = $1 
                  AND timestamp >= $2 
                  AND timestamp <= $3
                ORDER BY timestamp ASC
                LIMIT $4
            "#,
            )
            .bind(symbol)
            .bind(start)
            .bind(end)
            .bind(final_limit)
            .fetch_all(&self.pool)
            .await;

            if let Ok(raw_rows) = raw_results {
                log::info!("Retrieved {} raw records from market_data table for symbol {}", 
                          raw_rows.len(), symbol);
                
                // If we already have some hourly data, we need to be careful about duplicates
                // and potentially prefer raw data over aggregated data for better granularity
                let mut raw_data = Vec::new();
                
                for row in raw_rows {
                    let timestamp: DateTime<Utc> = row.get("timestamp");
                    let symbol: String = row.get("symbol");
                    let open: f64 = row.get("open");
                    let high: f64 = row.get("high");
                    let low: f64 = row.get("low");
                    let close: f64 = row.get("close");
                    let volume: f64 = row.get("volume");

                    raw_data.push(TimeSeriesData {
                        timestamp,
                        source: "market_data".to_string(), // Source indicates raw data table
                        entity: symbol,
                        value: close, // Use close price as the main value
                        metadata: Some(serde_json::json!({
                            "open": open,
                            "high": high,
                            "low": low,
                            "close": close,
                            "volume": volume
                        })),
                    });
                }
                
                // If we had insufficient hourly data, replace it with raw data for better coverage
                if results.is_empty() {
                    results = raw_data;
                } else {
                    // Supplement existing hourly data with raw data
                    // Raw data provides higher granularity and fills gaps
                    log::info!("Replacing {} hourly records with {} raw records for better data coverage", 
                              results.len(), raw_data.len());
                    results = raw_data; // Use raw data for higher fidelity
                }
            } else {
                log::warn!("Failed to retrieve raw data for symbol {} from market_data table - no historical data available", symbol);
            }
        }

        log::info!("Final result: {} total records for symbol {} covering requested period", 
                  results.len(), symbol);

        Ok(results)
    }

    /// Store a neural network prediction
    pub async fn store_prediction(&self, prediction: &PredictionData) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO predictions (timestamp, entity, model_id, prediction_value, confidence, horizon_minutes, features_used)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (timestamp, entity, model_id)
            DO UPDATE SET 
                prediction_value = EXCLUDED.prediction_value,
                confidence = EXCLUDED.confidence,
                horizon_minutes = EXCLUDED.horizon_minutes,
                features_used = EXCLUDED.features_used
        "#)
        .bind(prediction.timestamp)
        .bind(&prediction.entity)
        .bind(&prediction.model_id)
        .bind(prediction.prediction_value)
        .bind(prediction.confidence)
        .bind(prediction.horizon_minutes)
        .bind(&prediction.features_used)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Batch insert time series data for better performance
    pub async fn batch_insert(&self, data: &[TimeSeriesData]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // Use COPY for efficient batch insert
        let mut transaction = self.pool.begin().await?;

        for chunk in data.chunks(1000) {
            let mut query_builder = sqlx::QueryBuilder::new(
                "INSERT INTO time_series_data (timestamp, source, entity, value, metadata) ",
            );

            query_builder.push_values(chunk.iter(), |mut b, data| {
                b.push_bind(data.timestamp)
                    .push_bind(&data.source)
                    .push_bind(&data.entity)
                    .push_bind(data.value)
                    .push_bind(&data.metadata);
            });

            query_builder.push(" ON CONFLICT (timestamp, entity, source) DO NOTHING");

            let query = query_builder.build();
            query.execute(&mut *transaction).await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    /// Get the latest prediction for an entity and model
    pub async fn get_latest_prediction(
        &self,
        entity: &str,
        model_id: &str,
    ) -> Result<Option<PredictionData>> {
        let result = sqlx::query_as::<_, PredictionData>(r#"
            SELECT timestamp, entity, model_id, prediction_value, confidence, horizon_minutes, features_used
            FROM predictions
            WHERE entity = $1 AND model_id = $2
            ORDER BY timestamp DESC
            LIMIT 1
        "#)
        .bind(entity)
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Clean up data older than specified days
    pub async fn cleanup_old_data(&self, days_to_keep: i64) -> Result<u64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days_to_keep);

        let result = sqlx::query(
            r#"
            DELETE FROM time_series_data
            WHERE timestamp < $1
        "#,
        )
        .bind(cutoff_date)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get aggregated statistics for a symbol
    pub async fn get_statistics(
        &self,
        symbol: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval: &str,
    ) -> Result<Vec<AggregatedStats>> {
        // Try to get stats from market_data_1h first, fallback to time_series_data
        let hourly_results = sqlx::query(
            r#"
            SELECT 
                time_bucket($1::interval, bucket) as bucket,
                symbol as entity,
                AVG(close) as avg_value,
                MIN(low) as min_value,
                MAX(high) as max_value,
                COUNT(*) as count,
                STDDEV(close) as stddev
            FROM market_data_1h
            WHERE symbol = $2 
              AND bucket >= $3 
              AND bucket <= $4
            GROUP BY time_bucket($1::interval, bucket), symbol
            ORDER BY bucket ASC
        "#,
        )
        .bind(interval)
        .bind(symbol)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await;

        if let Ok(rows) = hourly_results {
            let mut results = Vec::new();
            for row in rows {
                let bucket: DateTime<Utc> = row.get("bucket");
                let entity: String = row.get("entity");
                let avg_value: Option<f64> = row.get("avg_value");
                let min_value: Option<f64> = row.get("min_value");
                let max_value: Option<f64> = row.get("max_value");
                let count: i64 = row.get("count");
                let stddev: Option<f64> = row.get("stddev");

                results.push(AggregatedStats {
                    bucket,
                    entity,
                    avg_value,
                    min_value,
                    max_value,
                    count,
                    stddev,
                });
            }
            if !results.is_empty() {
                return Ok(results);
            }
        }

        // Fallback to time_series_data table
        let results = sqlx::query_as::<_, AggregatedStats>(
            r#"
            SELECT 
                time_bucket($1::interval, timestamp) as bucket,
                entity,
                AVG(value) as avg_value,
                MIN(value) as min_value,
                MAX(value) as max_value,
                COUNT(*) as count,
                STDDEV(value) as stddev
            FROM time_series_data
            WHERE entity = $2 
              AND timestamp >= $3 
              AND timestamp <= $4
            GROUP BY bucket, entity
            ORDER BY bucket ASC
        "#,
        )
        .bind(interval)
        .bind(symbol)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(results)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AggregatedStats {
    pub bucket: DateTime<Utc>,
    pub entity: String,
    pub avg_value: Option<f64>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub count: i64,
    pub stddev: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_series_data_serialization() {
        let data = TimeSeriesData {
            timestamp: Utc::now(),
            source: "test".to_string(),
            entity: "BTC/USD".to_string(),
            value: 42000.0,
            metadata: Some(serde_json::json!({"exchange": "binance"})),
        };

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: TimeSeriesData = serde_json::from_str(&json).unwrap();

        assert_eq!(data.entity, deserialized.entity);
        assert_eq!(data.value, deserialized.value);
    }
}
