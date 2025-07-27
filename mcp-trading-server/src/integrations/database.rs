use crate::error::{Error, Result};
use crate::models::{PriceData, OHLCV, Orderbook, OrderbookLevel, MarketStats};
use chrono::{DateTime, Utc};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::{NoTls, Row};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone)]
pub struct DatabaseClient {
    pool: Arc<Pool>,
}

impl DatabaseClient {
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to database...");
        
        let config = database_url.parse::<tokio_postgres::Config>()
            .map_err(|e| Error::Config(format!("Invalid database URL: {}", e)))?;
        
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        
        let mgr = Manager::from_config(config, NoTls, mgr_config);
        let pool = Pool::builder(mgr)
            .max_size(16)
            .build()
            .map_err(|_| Error::Config("Failed to create database pool".to_string()))?;
        
        // Test connection
        let _client = pool.get().await
            .map_err(|e| Error::ServiceUnavailable(format!("Database pool error: {}", e)))?;
        
        info!("Database connection established");
        
        Ok(Self {
            pool: Arc::new(pool),
        })
    }
    
    pub async fn get_latest_price(&self, symbol: &str) -> Result<PriceData> {
        let client = self.pool.get().await
            .map_err(|_| Error::ServiceUnavailable("Database connection pool error".to_string()))?;
        
        let row = client.query_one(
            "SELECT symbol, price, timestamp, volume, bid, ask 
             FROM market_data 
             WHERE symbol = $1 
             ORDER BY timestamp DESC 
             LIMIT 1",
            &[&symbol]
        ).await
        .map_err(|e| {
            if e.to_string().contains("no rows") {
                Error::NotFound(format!("Symbol not found: {}", symbol))
            } else {
                Error::Database(e)
            }
        })?;
        
        Ok(self.row_to_price_data(&row))
    }

    pub async fn get_latest_prices(&self, symbol: &str, limit: usize) -> Result<Vec<PriceData>> {
        let client = self.pool.get().await
            .map_err(|_| Error::ServiceUnavailable("Database connection pool error".to_string()))?;
        
        let rows = client.query(
            "SELECT symbol, price, timestamp, volume, bid, ask 
             FROM market_data 
             WHERE symbol = $1 
             ORDER BY timestamp DESC 
             LIMIT $2",
            &[&symbol, &(limit as i64)]
        ).await
        .map_err(|e| {
            if e.to_string().contains("no rows") {
                Error::NotFound(format!("Symbol not found: {}", symbol))
            } else {
                Error::Database(e)
            }
        })?;
        
        Ok(rows.iter().map(|row| self.row_to_price_data(row)).collect())
    }
    
    pub async fn get_historical_data(
        &self,
        symbol: &str,
        interval: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<OHLCV>> {
        let client = self.pool.get().await
            .map_err(|_| Error::ServiceUnavailable("Database connection pool error".to_string()))?;
        
        let interval_seconds = self.parse_interval(interval)?;
        
        let rows = client.query(
            "SELECT 
                time_bucket($1::interval, timestamp) as bucket,
                first(price, timestamp) as open,
                max(price) as high,
                min(price) as low,
                last(price, timestamp) as close,
                sum(volume) as volume
             FROM market_data
             WHERE symbol = $2 
                AND timestamp >= $3 
                AND timestamp <= $4
             GROUP BY bucket
             ORDER BY bucket",
            &[&format!("{} seconds", interval_seconds), &symbol, &start_time, &end_time]
        ).await?;
        
        Ok(rows.iter().map(|row| OHLCV {
            timestamp: row.get("bucket"),
            open: row.get("open"),
            high: row.get("high"),
            low: row.get("low"),
            close: row.get("close"),
            volume: row.get("volume"),
        }).collect())
    }
    
    pub async fn get_orderbook(&self, symbol: &str, depth: usize) -> Result<Orderbook> {
        let client = self.pool.get().await
            .map_err(|_| Error::ServiceUnavailable("Database connection pool error".to_string()))?;
        
        // Get bids
        let bid_rows = client.query(
            "SELECT price, quantity 
             FROM orderbook 
             WHERE symbol = $1 AND side = 'bid' 
             ORDER BY price DESC 
             LIMIT $2",
            &[&symbol, &(depth as i64)]
        ).await?;
        
        // Get asks
        let ask_rows = client.query(
            "SELECT price, quantity 
             FROM orderbook 
             WHERE symbol = $1 AND side = 'ask' 
             ORDER BY price ASC 
             LIMIT $2",
            &[&symbol, &(depth as i64)]
        ).await?;
        
        let bids: Vec<OrderbookLevel> = bid_rows.iter().map(|row| OrderbookLevel {
            price: row.get("price"),
            quantity: row.get("quantity"),
        }).collect();
        
        let asks: Vec<OrderbookLevel> = ask_rows.iter().map(|row| OrderbookLevel {
            price: row.get("price"),
            quantity: row.get("quantity"),
        }).collect();
        
        Ok(Orderbook {
            symbol: symbol.to_string(),
            bids,
            asks,
            timestamp: Utc::now(),
        })
    }
    
    pub async fn get_market_stats(&self, symbol: &str, period: &str) -> Result<MarketStats> {
        let client = self.pool.get().await
            .map_err(|_| Error::ServiceUnavailable("Database connection pool error".to_string()))?;
        
        let period_interval = match period {
            "1h" => "1 hour",
            "24h" => "24 hours",
            "7d" => "7 days",
            "30d" => "30 days",
            _ => return Err(Error::InvalidParameter(format!("Invalid period: {}", period))),
        };
        
        let row = client.query_one(
            "SELECT 
                $1::text as symbol,
                $2::text as period,
                sum(volume) as volume,
                max(price) as high,
                min(price) as low,
                first(price, timestamp) as open,
                last(price, timestamp) as close,
                count(*) as trade_count
             FROM market_data
             WHERE symbol = $1 
                AND timestamp >= NOW() - $3::interval",
            &[&symbol, &period, &period_interval]
        ).await?;
        
        let open: f64 = row.get("open");
        let close: f64 = row.get("close");
        let change_amount = close - open;
        let change_percent = if open != 0.0 { (change_amount / open) * 100.0 } else { 0.0 };
        
        Ok(MarketStats {
            symbol: row.get("symbol"),
            period: row.get("period"),
            volume: row.get("volume"),
            high: row.get("high"),
            low: row.get("low"),
            open,
            close,
            change_amount,
            change_percent,
            trade_count: row.get::<_, i64>("trade_count") as u64,
        })
    }
    
    fn row_to_price_data(&self, row: &Row) -> PriceData {
        PriceData {
            symbol: row.get("symbol"),
            price: row.get("price"),
            timestamp: row.get("timestamp"),
            volume: row.try_get("volume").ok(),
            bid: row.try_get("bid").ok(),
            ask: row.try_get("ask").ok(),
            open: row.try_get("open").ok(),
            high: row.try_get("high").ok(),
            low: row.try_get("low").ok(),
            close: row.try_get("close").ok(),
        }
    }
    
    fn parse_interval(&self, interval: &str) -> Result<i64> {
        match interval {
            "1m" => Ok(60),
            "5m" => Ok(300),
            "15m" => Ok(900),
            "1h" => Ok(3600),
            "4h" => Ok(14400),
            "1d" => Ok(86400),
            _ => Err(Error::InvalidParameter(format!("Invalid interval: {}", interval))),
        }
    }
}