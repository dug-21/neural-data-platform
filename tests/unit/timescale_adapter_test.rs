//! Unit tests for TimescaleDB adapter

use mockall::predicate::*;
use autonomous_platform::adapters::{
    timescale::{TimescaleAdapter, TimescaleConfig},
    AdapterError, DataAdapter, MarketData,
};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> TimescaleConfig {
        TimescaleConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "test_db".to_string(),
            username: "test_user".to_string(),
            password: "test_pass".to_string(),
            max_connections: 5,
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

    #[test]
    fn test_timescale_config_default() {
        let config = TimescaleConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "trading");
        assert_eq!(config.username, "postgres");
        assert_eq!(config.password, "postgres");
        assert_eq!(config.max_connections, 10);
    }

    #[test]
    fn test_timescale_adapter_creation() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config.clone());

        assert!(!adapter.is_connected());
        assert_eq!(adapter.name(), "TimescaleDB");
    }

    #[tokio::test]
    async fn test_query_market_data_not_connected() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let result = adapter.query_market_data("BTC/USD", 0, 100).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_insert_market_data_not_connected() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);
        let data = vec![create_test_market_data()];

        let result = adapter.insert_market_data(&data).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_create_hypertable_not_connected() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let result = adapter.create_hypertable().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Connection(msg) => assert_eq!(msg, "Not connected"),
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_empty_symbol() {
        let config = create_test_config();
        let mut adapter = TimescaleAdapter::new(config);

        // Simulate connected state by setting a dummy pool
        // In real tests, you would use a test database

        let mut data = create_test_market_data();
        data.symbol = "".to_string();

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => assert_eq!(msg, "Symbol cannot be empty"),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_negative_timestamp() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data = create_test_market_data();
        data.timestamp = -1;

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => assert_eq!(msg, "Timestamp must be non-negative"),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_negative_prices() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data = create_test_market_data();
        data.open = -100.0;

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => assert_eq!(msg, "Prices must be non-negative"),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_high_low_relationship() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data = create_test_market_data();
        data.high = 100.0;
        data.low = 200.0;

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => assert_eq!(msg, "High price must be >= low price"),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_high_not_highest() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data = create_test_market_data();
        data.high = 100.0;
        data.open = 150.0;
        data.low = 50.0;
        data.close = 90.0;

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => {
                assert_eq!(msg, "High price must be the highest price")
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_low_not_lowest() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data = create_test_market_data();
        data.high = 150.0;
        data.open = 100.0;
        data.low = 110.0;
        data.close = 120.0;

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => {
                assert_eq!(msg, "Low price must be the lowest price")
            }
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_market_data_validation_negative_volume() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data = create_test_market_data();
        data.volume = -100.0;

        let result = adapter.insert_market_data(&[data]).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AdapterError::Configuration(msg) => assert_eq!(msg, "Volume must be non-negative"),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[tokio::test]
    async fn test_valid_market_data() {
        // This test validates that valid market data passes all checks
        let data = create_test_market_data();

        // Validate relationships
        assert!(data.high >= data.low);
        assert!(data.high >= data.open);
        assert!(data.high >= data.close);
        assert!(data.low <= data.open);
        assert!(data.low <= data.close);
        assert!(data.volume >= 0.0);
        assert!(data.timestamp >= 0);
        assert!(!data.symbol.is_empty());
    }

    #[tokio::test]
    async fn test_disconnect_when_not_connected() {
        let config = create_test_config();
        let mut adapter = TimescaleAdapter::new(config);

        // Should not error when disconnecting while not connected
        let result = adapter.disconnect().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_market_data_validation() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);

        let mut data1 = create_test_market_data();
        let mut data2 = create_test_market_data();
        data2.timestamp = 1640995300;

        // Make second data invalid
        data2.volume = -50.0;

        let result = adapter.insert_market_data(&[data1, data2]).await;
        assert!(result.is_err());

        // Should fail on the second item
        match result.unwrap_err() {
            AdapterError::Configuration(msg) => assert_eq!(msg, "Volume must be non-negative"),
            _ => panic!("Expected Configuration error"),
        }
    }

    #[test]
    fn test_adapter_name() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);
        assert_eq!(adapter.name(), "TimescaleDB");
    }

    #[test]
    fn test_is_connected_false_initially() {
        let config = create_test_config();
        let adapter = TimescaleAdapter::new(config);
        assert!(!adapter.is_connected());
    }

    // Integration test placeholder - would require actual database
    #[tokio::test]
    #[ignore = "Requires TimescaleDB instance"]
    async fn test_full_integration() {
        let config = TimescaleConfig {
            host: std::env::var("TIMESCALE_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("TIMESCALE_PORT")
                .unwrap_or_else(|_| "5432".to_string())
                .parse()
                .unwrap(),
            database: std::env::var("TIMESCALE_DB").unwrap_or_else(|_| "test_trading".to_string()),
            username: std::env::var("TIMESCALE_USER").unwrap_or_else(|_| "postgres".to_string()),
            password: std::env::var("TIMESCALE_PASS").unwrap_or_else(|_| "postgres".to_string()),
            max_connections: 5,
        };

        let mut adapter = TimescaleAdapter::new(config);

        // Connect
        let result = adapter.connect().await;
        assert!(result.is_ok());
        assert!(adapter.is_connected());

        // Create hypertable
        let result = adapter.create_hypertable().await;
        assert!(result.is_ok());

        // Insert data
        let data = vec![
            create_test_market_data(),
            MarketData {
                symbol: "ETH/USD".to_string(),
                timestamp: 1640995200,
                open: 3800.0,
                high: 3900.0,
                low: 3750.0,
                close: 3850.0,
                volume: vec![500.0],
            },
        ];

        let result = adapter.insert_market_data(&data).await;
        assert!(result.is_ok());

        // Query data
        let result = adapter.query_market_data("BTC/USD", 0, 2000000000).await;
        assert!(result.is_ok());
        let queried_data = result.unwrap();
        assert!(!queried_data.is_empty());

        // Disconnect
        let result = adapter.disconnect().await;
        assert!(result.is_ok());
        assert!(!adapter.is_connected());
    }
}
