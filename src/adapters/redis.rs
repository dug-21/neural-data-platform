//! Redis adapter for real-time market data streaming
//! 
//! Provides high-performance pub/sub and caching capabilities
//! for real-time market data and order book updates.

use super::{AdapterError, DataAdapter, MarketData, OrderBook};
use async_trait::async_trait;
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};
use std::sync::Arc;
use tokio::sync::RwLock;
use futures::StreamExt;

/// Redis configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub db: i64,
    pub pool_size: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: 0,
            pool_size: 10,
        }
    }
}

/// Redis adapter for real-time data
pub struct RedisAdapter {
    config: RedisConfig,
    client: Option<Client>,
    connection: Option<Arc<RwLock<MultiplexedConnection>>>,
}

impl RedisAdapter {
    /// Create a new Redis adapter
    pub fn new(config: RedisConfig) -> Self {
        Self {
            config,
            client: None,
            connection: None,
        }
    }

    /// Publish market data to a channel
    pub async fn publish_market_data(
        &self,
        channel: &str,
        data: &MarketData,
    ) -> Result<(), AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let json = serde_json::to_string(data)
            .map_err(|e| AdapterError::Serialization(e.to_string()))?;

        let mut conn = conn.write().await;
        conn.publish::<_, _, ()>(channel, json)
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        Ok(())
    }

    /// Subscribe to market data channel
    pub async fn subscribe_market_data(
        &self,
        channel: &str,
    ) -> Result<impl futures::Stream<Item = Result<MarketData, AdapterError>>, AdapterError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?
            .clone();

        let pubsub_conn = client
            .get_async_pubsub()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;
        
        let mut pubsub = pubsub_conn;

        pubsub
            .subscribe(channel)
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        let stream = pubsub.into_on_message().map(|msg| {
            let payload: String = msg.get_payload().map_err(|e| {
                AdapterError::Serialization(format!("Failed to get payload: {}", e))
            })?;

            serde_json::from_str::<MarketData>(&payload)
                .map_err(|e| AdapterError::Serialization(e.to_string()))
        });

        Ok(stream)
    }

    /// Cache order book snapshot
    pub async fn cache_order_book(&self, order_book: &OrderBook) -> Result<(), AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let key = format!("orderbook:{}", order_book.symbol);
        let json = serde_json::to_string(order_book)
            .map_err(|e| AdapterError::Serialization(e.to_string()))?;

        let mut conn = conn.write().await;
        conn.set_ex(&key, json, 60) // Expire after 60 seconds
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        Ok(())
    }

    /// Get cached order book
    pub async fn get_order_book(&self, symbol: &str) -> Result<Option<OrderBook>, AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let key = format!("orderbook:{}", symbol);
        let mut conn = conn.write().await;
        
        let result: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        match result {
            Some(json) => {
                let order_book = serde_json::from_str(&json)
                    .map_err(|e| AdapterError::Serialization(e.to_string()))?;
                Ok(Some(order_book))
            }
            None => Ok(None),
        }
    }

    /// Store latest price for a symbol
    pub async fn set_latest_price(
        &self,
        symbol: &str,
        price: f64,
        timestamp: i64,
    ) -> Result<(), AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let key = format!("price:latest:{}", symbol);
        let value = format!("{}:{}", price, timestamp);

        let mut conn = conn.write().await;
        conn.set(&key, value)
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        Ok(())
    }

    /// Get latest price for a symbol
    pub async fn get_latest_price(
        &self,
        symbol: &str,
    ) -> Result<Option<(f64, i64)>, AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let key = format!("price:latest:{}", symbol);
        let mut conn = conn.write().await;
        
        let result: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| AdapterError::Query(e.to_string()))?;

        match result {
            Some(value) => {
                let parts: Vec<&str> = value.split(':').collect();
                if parts.len() == 2 {
                    let price = parts[0]
                        .parse::<f64>()
                        .map_err(|e| AdapterError::Serialization(e.to_string()))?;
                    let timestamp = parts[1]
                        .parse::<i64>()
                        .map_err(|e| AdapterError::Serialization(e.to_string()))?;
                    Ok(Some((price, timestamp)))
                } else {
                    Err(AdapterError::Serialization("Invalid price format".to_string()))
                }
            }
            None => Ok(None),
        }
    }
}

#[async_trait]
impl DataAdapter for RedisAdapter {
    async fn connect(&mut self) -> Result<(), AdapterError> {
        let redis_url = if let Some(password) = &self.config.password {
            format!(
                "redis://:{}@{}:{}/{}",
                password, self.config.host, self.config.port, self.config.db
            )
        } else {
            format!(
                "redis://{}:{}/{}",
                self.config.host, self.config.port, self.config.db
            )
        };

        let client = Client::open(redis_url)
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        let connection = client.get_multiplexed_async_connection()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        self.client = Some(client);
        self.connection = Some(Arc::new(RwLock::new(connection)));

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AdapterError> {
        self.connection = None;
        self.client = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    fn name(&self) -> &str {
        "Redis"
    }
}