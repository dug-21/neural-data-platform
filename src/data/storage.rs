use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

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

        // Create time series data table
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
        entity: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TimeSeriesData>> {
        let results = sqlx::query_as::<_, TimeSeriesData>(
            r#"
            SELECT timestamp, source, entity, value, metadata
            FROM time_series_data
            WHERE entity = $1 
              AND timestamp >= $2 
              AND timestamp <= $3
            ORDER BY timestamp ASC
        "#,
        )
        .bind(entity)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

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

    /// Get aggregated statistics for an entity
    pub async fn get_statistics(
        &self,
        entity: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval: &str,
    ) -> Result<Vec<AggregatedStats>> {
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
        .bind(entity)
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
