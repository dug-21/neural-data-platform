//! TimescaleDB adapter for historical market data
//!
//! Provides efficient storage and retrieval of time-series market data
//! using TimescaleDB's hypertable features.

use super::{AdapterError, DataAdapter, MarketData};
use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;

/// TimescaleDB configuration
#[derive(Debug, Clone)]
pub struct TimescaleConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: u32,
}

impl Default for TimescaleConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "trading".to_string(),
            username: "postgres".to_string(),
            password: "postgres".to_string(),
            max_connections: 10,
        }
    }
}

/// TimescaleDB adapter
pub struct TimescaleAdapter {
    config: TimescaleConfig,
    pool: Option<Arc<Pool<Postgres>>>,
}

impl TimescaleAdapter {
    /// Create a new TimescaleDB adapter
    pub fn new(config: TimescaleConfig) -> Self {
        Self { config, pool: None }
    }

    /// Query historical market data
    pub async fn query_market_data(
        &self,
        symbol: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<MarketData>, AdapterError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let query = r#"
            SELECT symbol, timestamp, open, high, low, close, volume
            FROM market_data
            WHERE symbol = $1 AND timestamp >= $2 AND timestamp <= $3
            ORDER BY timestamp ASC
        "#;

        let rows = sqlx::query_as::<_, (String, i64, f64, f64, f64, f64, f64)>(query)
            .bind(symbol)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(pool.as_ref())
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(
                |(symbol, timestamp, open, high, low, close, volume)| MarketData {
                    symbol,
                    timestamp,
                    open,
                    high,
                    low,
                    close,
                    volume,
                },
            )
            .collect())
    }

    /// Insert market data
    pub async fn insert_market_data(&self, data: &[MarketData]) -> Result<(), AdapterError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        // Validate data before insertion
        for item in data {
            // Validate symbol
            if item.symbol.is_empty() {
                return Err(AdapterError::Configuration(
                    "Symbol cannot be empty".to_string(),
                ));
            }

            // Validate timestamp
            if item.timestamp < 0 {
                return Err(AdapterError::Configuration(
                    "Timestamp must be non-negative".to_string(),
                ));
            }

            // Validate prices
            if item.open < 0.0 || item.high < 0.0 || item.low < 0.0 || item.close < 0.0 {
                return Err(AdapterError::Configuration(
                    "Prices must be non-negative".to_string(),
                ));
            }

            // Validate OHLC relationships
            if item.high < item.low {
                return Err(AdapterError::Configuration(
                    "High price must be >= low price".to_string(),
                ));
            }

            if item.high < item.open || item.high < item.close {
                return Err(AdapterError::Configuration(
                    "High price must be the highest price".to_string(),
                ));
            }

            if item.low > item.open || item.low > item.close {
                return Err(AdapterError::Configuration(
                    "Low price must be the lowest price".to_string(),
                ));
            }

            // Validate volume
            if item.volume < 0.0 {
                return Err(AdapterError::Configuration(
                    "Volume must be non-negative".to_string(),
                ));
            }
        }

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        for item in data {
            let query = r#"
                INSERT INTO market_data (symbol, timestamp, open, high, low, close, volume)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (symbol, timestamp) DO UPDATE
                SET open = EXCLUDED.open,
                    high = EXCLUDED.high,
                    low = EXCLUDED.low,
                    close = EXCLUDED.close,
                    volume = EXCLUDED.volume
            "#;

            sqlx::query(query)
                .bind(&item.symbol)
                .bind(item.timestamp)
                .bind(item.open)
                .bind(item.high)
                .bind(item.low)
                .bind(item.close)
                .bind(item.volume)
                .execute(&mut *tx)
                .await
                .map_err(|e| AdapterError::Query(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        Ok(())
    }

    /// Create hypertable for market data
    pub async fn create_hypertable(&self) -> Result<(), AdapterError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        // Create table if not exists
        let create_table = r#"
            CREATE TABLE IF NOT EXISTS market_data (
                symbol VARCHAR(32) NOT NULL,
                timestamp BIGINT NOT NULL,
                open DOUBLE PRECISION NOT NULL,
                high DOUBLE PRECISION NOT NULL,
                low DOUBLE PRECISION NOT NULL,
                close DOUBLE PRECISION NOT NULL,
                volume DOUBLE PRECISION NOT NULL,
                PRIMARY KEY (symbol, timestamp)
            )
        "#;

        sqlx::query(create_table)
            .execute(pool.as_ref())
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        // Convert to hypertable
        let create_hypertable = r#"
            SELECT create_hypertable('market_data', 'timestamp',
                chunk_time_interval => INTERVAL '1 day',
                if_not_exists => TRUE
            )
        "#;

        sqlx::query(create_hypertable)
            .execute(pool.as_ref())
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl DataAdapter for TimescaleAdapter {
    async fn connect(&mut self) -> Result<(), AdapterError> {
        let connection_str = format!(
            "postgres://{}:{}@{}:{}/{}",
            self.config.username,
            self.config.password,
            self.config.host,
            self.config.port,
            self.config.database
        );

        let pool = PgPoolOptions::new()
            .max_connections(self.config.max_connections)
            .connect(&connection_str)
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        self.pool = Some(Arc::new(pool));
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AdapterError> {
        if let Some(pool) = self.pool.take() {
            // Pool will be closed when Arc count reaches 0
            drop(pool);
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    fn name(&self) -> &str {
        "TimescaleDB"
    }
}
