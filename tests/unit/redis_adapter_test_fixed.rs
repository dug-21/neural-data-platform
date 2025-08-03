//! Unit tests for Redis adapter - Fixed version

use futures::StreamExt;
use autonomous_platform::adapters::{
    redis::{RedisAdapter, RedisConfig},
    AdapterError, DataAdapter, MarketData, OrderBook, OrderBookEntry,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> RedisConfig {
        RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            default_ttl_seconds: 300,
            connection_timeout_ms: 5000,
            cluster_mode: false,
            pool_max_idle: 5,
            pool_timeout_seconds: 30,
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
            volume: vec![1000.0],
        }
    }

    fn create_test_order_book() -> OrderBook {
        OrderBook {
            symbol: "BTC/USD".to_string(),
            timestamp: 1640995200,
            bids: vec![
                OrderBookEntry { price: 48000.0, quantity: 1.5, timestamp: 1640995200 },
                OrderBookEntry { price: 47900.0, quantity: 2.0, timestamp: 1640995200 },
                OrderBookEntry { price: 47800.0, quantity: 1.0, timestamp: 1640995200 },
            ],
            asks: vec![
                OrderBookEntry { price: 48100.0, quantity: 1.2, timestamp: 1640995200 },
                OrderBookEntry { price: 48200.0, quantity: 2.5, timestamp: 1640995200 },
                OrderBookEntry { price: 48300.0, quantity: 1.8, timestamp: 1640995200 },
            ],
        }
    }

    #[test]
    fn test_redis_config_creation() {
        let config = create_test_config();
        assert_eq!(config.url, "redis://localhost:6379");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.pool_max_idle, 5);
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
    async fn test_set_latest_price_not_connected() {
        let config = create_test_config();
        let adapter = RedisAdapter::new(config);

        let result = adapter
            .set_latest_price("BTC/USD", 48500.0, 1640995200)
            .await;
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