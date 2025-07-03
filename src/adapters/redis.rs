//! Redis adapter for real-time market data streaming
//! 
//! Provides high-performance pub/sub and caching capabilities
//! for real-time market data and order book updates.

use super::{AdapterError, DataAdapter, MarketData, OrderBook};
use async_trait::async_trait;
use redis::{aio::MultiplexedConnection, AsyncCommands, Client, Value, streams::{StreamRangeReply}};
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
            .get_async_connection()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?
            .into_pubsub();
        
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
        // Validate order book before caching
        if order_book.symbol.is_empty() {
            return Err(AdapterError::Serialization("Order book symbol cannot be empty".to_string()));
        }
        
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let key = format!("orderbook:{}", order_book.symbol);
        let json = serde_json::to_string(order_book)
            .map_err(|e| AdapterError::Serialization(e.to_string()))?;

        let mut conn = conn.write().await;
        conn.set_ex::<_, _, ()>(&key, json, 60) // Expire after 60 seconds
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
        conn.set::<_, _, ()>(&key, value)
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

    /// Add market data to Redis stream
    pub async fn add_to_stream(
        &self,
        stream_key: &str,
        data: &MarketData,
    ) -> Result<String, AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let mut conn = conn.write().await;
        
        // Create stream entry fields with owned strings
        let timestamp_str = data.timestamp.to_string();
        let open_str = data.open.to_string();
        let high_str = data.high.to_string();
        let low_str = data.low.to_string();
        let close_str = data.close.to_string();
        let volume_str = data.volume.to_string();
        
        let fields = vec![
            ("symbol", data.symbol.as_str()),
            ("timestamp", timestamp_str.as_str()),
            ("open", open_str.as_str()),
            ("high", high_str.as_str()),
            ("low", low_str.as_str()),
            ("close", close_str.as_str()),
            ("volume", volume_str.as_str()),
        ];

        // Add to stream with automatic ID generation
        let stream_id: String = conn
            .xadd(stream_key, "*", &fields)
            .await
            .map_err(|e| AdapterError::Query(format!("Failed to add to stream: {}", e)))?;

        Ok(stream_id)
    }

    /// Read from Redis stream
    pub async fn read_from_stream(
        &self,
        stream_key: &str,
        start_id: &str,
        count: usize,
    ) -> Result<Vec<MarketData>, AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let mut conn = conn.write().await;
        
        // Read from stream
        let reply: StreamRangeReply = conn
            .xrange_count(stream_key, start_id, "+", count)
            .await
            .map_err(|e| AdapterError::Query(format!("Failed to read from stream: {}", e)))?;

        let mut market_data_vec = Vec::new();

        for stream_entry in reply.ids {
            let mut data = MarketData {
                symbol: String::new(),
                timestamp: 0,
                open: 0.0,
                high: 0.0,
                low: 0.0,
                close: 0.0,
                volume: 0.0,
            };

            // Parse fields from stream entry
            for (key, value) in stream_entry.map {
                match key.as_str() {
                    "symbol" => {
                        if let Value::Data(bytes) = value {
                            data.symbol = String::from_utf8_lossy(&bytes).to_string();
                        }
                    }
                    "timestamp" => {
                        if let Value::Data(bytes) = value {
                            data.timestamp = String::from_utf8_lossy(&bytes)
                                .parse()
                                .unwrap_or(0);
                        }
                    }
                    "open" => {
                        if let Value::Data(bytes) = value {
                            data.open = String::from_utf8_lossy(&bytes)
                                .parse()
                                .unwrap_or(0.0);
                        }
                    }
                    "high" => {
                        if let Value::Data(bytes) = value {
                            data.high = String::from_utf8_lossy(&bytes)
                                .parse()
                                .unwrap_or(0.0);
                        }
                    }
                    "low" => {
                        if let Value::Data(bytes) = value {
                            data.low = String::from_utf8_lossy(&bytes)
                                .parse()
                                .unwrap_or(0.0);
                        }
                    }
                    "close" => {
                        if let Value::Data(bytes) = value {
                            data.close = String::from_utf8_lossy(&bytes)
                                .parse()
                                .unwrap_or(0.0);
                        }
                    }
                    "volume" => {
                        if let Value::Data(bytes) = value {
                            data.volume = String::from_utf8_lossy(&bytes)
                                .parse()
                                .unwrap_or(0.0);
                        }
                    }
                    _ => {}
                }
            }

            market_data_vec.push(data);
        }

        Ok(market_data_vec)
    }

    /// Create consumer group for stream
    pub async fn create_consumer_group(
        &self,
        stream_key: &str,
        group_name: &str,
    ) -> Result<(), AdapterError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| AdapterError::Connection("Not connected".to_string()))?;

        let mut conn = conn.write().await;
        
        // Create consumer group starting from beginning of stream
        let _: Result<(), _> = conn
            .xgroup_create_mkstream(stream_key, group_name, "$")
            .await;

        Ok(())
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