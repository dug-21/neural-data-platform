//! Unit tests for Redis adapter

use neural_trader::adapters::{
    redis::{RedisAdapter, RedisConfig},
    AdapterError, DataAdapter, MarketData, OrderBook,
};
use mockall::predicate::*;
use futures::StreamExt;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> RedisConfig {
        RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: Some("test_password".to_string()),
            db: 1,
            pool_size: 5,
        }
    }

    fn create_test_market_data() -> MarketData {
        MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: 1640995200,
            open: 48000.0,
            high: 49000.0,
            low: 47500.0,
            close: 48500.0,
            volume: 1000.0,
        }
    }

    fn create_test_order_book() -> OrderBook {
        OrderBook {
            symbol: "BTC/USD".to_string(),
            timestamp: 1640995200,
            bids: vec![
                (48000.0, 1.5),
                (47900.0, 2.0),
                (47800.0, 1.0),
            ],
            asks: vec![
                (48100.0, 1.2),
                (48200.0, 2.5),
                (48300.0, 1.8),
            ],
        }
    }

    #[test]
    fn test_redis_config_default() {
        let config = RedisConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 6379);
        assert!(config.password.is_none());
        assert_eq!(config.db, 0);
        assert_eq!(config.pool_size, 10);
    }

    #[test]
    fn test_redis_adapter_creation() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        assert!(!adapter.is_connected());
        assert_eq!(adapter.name(), "Redis");
    }

    #[tokio::test]
    async fn test_publish_market_data_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        let data = create_test_market_data();
        
        let result = adapter.publish_market_data("test_channel", &data).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_subscribe_market_data_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let result = adapter.subscribe_market_data("test_channel").await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_cache_order_book_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        let order_book = create_test_order_book();
        
        let result = adapter.cache_order_book(&order_book).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_cache_order_book_empty_symbol() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let mut order_book = create_test_order_book();
        order_book.symbol = "".to_string();
        
        let result = adapter.cache_order_book(&order_book).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Serialization(msg) => assert_eq!(msg, "Order book symbol cannot be empty"),
            _ => panic!("Expected Serialization error"),
        }
    }

    #[tokio::test]
    async fn test_get_order_book_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let result = adapter.get_order_book("BTC/USD").await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_set_latest_price_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let result = adapter.set_latest_price("BTC/USD", 48500.0, 1640995200).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_get_latest_price_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let result = adapter.get_latest_price("BTC/USD").await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_add_to_stream_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        let data = create_test_market_data();
        
        let result = adapter.add_to_stream("test_stream", &data).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_read_from_stream_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let result = adapter.read_from_stream("test_stream", "0", 10).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_create_consumer_group_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        
        let result = adapter.create_consumer_group("test_stream", "test_group").await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected() {
        let config = create_test_config();
        let mut adapter = RedisAdapter::new(config);
        
        // Should not error when disconnecting while not connected
        let result = adapter.disconnect().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_name() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        assert_eq!(adapter.name(), "Redis");
    }

    #[test]
    fn test_is_connected_false_initially() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_config_with_password() {
        let config = RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: Some("secret".to_string()),
            db: 2,
            pool_size: 10,
        };
        
        let adapter = RedisAdapter::new(config);
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_config_without_password() {
        let config = RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: 0,
            pool_size: 10,
        };
        
        let adapter = RedisAdapter::new(config);
        assert!(!adapter.is_connected());
    }

    // Integration test placeholder - would require actual Redis instance
    #[tokio::test]
    #[ignore = "Requires Redis instance"]
    async fn test_full_integration() {
        let config = RedisConfig {
            host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("REDIS_PORT")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap(),
            password: std::env::var("REDIS_PASSWORD").ok(),
            db: 0,
            pool_size: 5,
        };

        let mut adapter = RedisAdapter::new(config);
        
        // Connect
        let result = adapter.connect().await;
        assert!(result.is_ok());
        assert!(adapter.is_connected());
        
        // Test market data publish/subscribe
        let data = create_test_market_data();
        let channel = "test_market_data";
        
        // Start subscription before publishing
        let mut stream = adapter.subscribe_market_data(channel).await.unwrap();
        
        // Publish data
        let result = adapter.publish_market_data(channel, &data).await;
        assert!(result.is_ok());
        
        // Test order book caching
        let order_book = create_test_order_book();
        let result = adapter.cache_order_book(&order_book).await;
        assert!(result.is_ok());
        
        // Retrieve cached order book
        let result = adapter.get_order_book("BTC/USD").await;
        assert!(result.is_ok());
        let cached = result.unwrap();
        assert!(cached.is_some());
        let cached_book = cached.unwrap();
        assert_eq!(cached_book.symbol, order_book.symbol);
        
        // Test latest price
        let result = adapter.set_latest_price("BTC/USD", 48500.0, 1640995200).await;
        assert!(result.is_ok());
        
        let result = adapter.get_latest_price("BTC/USD").await;
        assert!(result.is_ok());
        let price_data = result.unwrap();
        assert!(price_data.is_some());
        let (price, timestamp) = price_data.unwrap();
        assert_eq!(price, 48500.0);
        assert_eq!(timestamp, 1640995200);
        
        // Test streams
        let stream_key = "test_stream";
        let result = adapter.add_to_stream(stream_key, &data).await;
        assert!(result.is_ok());
        let stream_id = result.unwrap();
        assert!(!stream_id.is_empty());
        
        // Read from stream
        let result = adapter.read_from_stream(stream_key, "0", 10).await;
        assert!(result.is_ok());
        let stream_data = result.unwrap();
        assert!(!stream_data.is_empty());
        
        // Create consumer group
        let result = adapter.create_consumer_group(stream_key, "test_group").await;
        assert!(result.is_ok());
        
        // Disconnect
        let result = adapter.disconnect().await;
        assert!(result.is_ok());
        assert!(!adapter.is_connected());
    }

    #[tokio::test]
    async fn test_market_data_serialization() {
        let data = create_test_market_data();
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: MarketData = serde_json::from_str(&json).unwrap();
        
        assert_eq!(data.symbol, deserialized.symbol);
        assert_eq!(data.timestamp, deserialized.timestamp);
        assert_eq!(data.open, deserialized.open);
        assert_eq!(data.high, deserialized.high);
        assert_eq!(data.low, deserialized.low);
        assert_eq!(data.close, deserialized.close);
        assert_eq!(data.volume, deserialized.volume);
    }

    #[tokio::test]
    async fn test_order_book_serialization() {
        let order_book = create_test_order_book();
        let json = serde_json::to_string(&order_book).unwrap();
        let deserialized: OrderBook = serde_json::from_str(&json).unwrap();
        
        assert_eq!(order_book.symbol, deserialized.symbol);
        assert_eq!(order_book.timestamp, deserialized.timestamp);
        assert_eq!(order_book.bids.len(), deserialized.bids.len());
        assert_eq!(order_book.asks.len(), deserialized.asks.len());
    }
}