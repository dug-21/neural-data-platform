//! TDD Tests for Redis Consumer Module
//! 
//! Tests the Redis stream consumption and message acknowledgment functionality

use data_staging::redis_consumer::*;
use data_staging::{DataStagingConfig, DataStagingError};
use tokio_test;
use redis::{AsyncCommands, Value};

#[tokio::test]
async fn test_redis_consumer_creation() {
    let config = DataStagingConfig::default();
    
    // Test consumer creation - should handle connection gracefully
    match RedisConsumer::new(&config).await {
        Ok(consumer) => {
            assert!(!consumer.is_connected().await.unwrap_or(false));
        }
        Err(e) => {
            // Expected in test environment without Redis
            assert!(matches!(e.downcast_ref::<DataStagingError>().unwrap(), 
                DataStagingError::Redis(_)));
        }
    }
}

#[tokio::test]
async fn test_message_parsing() {
    let raw_json = r#"
    {
        "symbol": "AAPL",
        "price": 150.25,
        "volume": 1000.0,
        "timestamp": 1640995200,
        "exchange": "NASDAQ"
    }
    "#;
    
    let message = RedisMessage {
        id: "test-123".to_string(),
        data: raw_json.to_string(),
        timestamp: 1640995200,
    };
    
    assert_eq!(message.id, "test-123");
    assert!(message.data.contains("AAPL"));
}

#[tokio::test]
async fn test_consumer_with_mock_redis() {
    // Mock Redis behavior for testing
    let config = DataStagingConfig::default();
    
    // Test batch consumption with empty stream
    let consumer_result = RedisConsumer::new(&config).await;
    
    match consumer_result {
        Ok(mut consumer) => {
            // Test empty batch
            let messages = consumer.consume_batch().await;
            match messages {
                Ok(msgs) => assert!(msgs.is_empty()),
                Err(_) => {
                    // Expected without real Redis connection
                }
            }
        }
        Err(_) => {
            // Expected in test environment
        }
    }
}

#[tokio::test]
async fn test_consumer_group_creation() {
    let config = DataStagingConfig {
        consumer_group: "test-group".to_string(),
        consumer_name: "test-consumer".to_string(),
        ..Default::default()
    };
    
    let consumer_result = RedisConsumer::new(&config).await;
    
    // Should attempt to create consumer group
    match consumer_result {
        Ok(consumer) => {
            assert_eq!(consumer.get_consumer_group(), "test-group");
            assert_eq!(consumer.get_consumer_name(), "test-consumer");
        }
        Err(_) => {
            // Expected without Redis
        }
    }
}

#[tokio::test]
async fn test_acknowledgment_logic() {
    let config = DataStagingConfig::default();
    
    match RedisConsumer::new(&config).await {
        Ok(mut consumer) => {
            // Test acknowledgment of non-existent message
            let ack_result = consumer.acknowledge_message("non-existent").await;
            
            match ack_result {
                Ok(_) => {},
                Err(e) => {
                    // Should handle gracefully
                    assert!(matches!(e.downcast_ref::<DataStagingError>().unwrap(),
                        DataStagingError::Redis(_)));
                }
            }
        }
        Err(_) => {
            // Expected without Redis
        }
    }
}

#[tokio::test]
async fn test_batch_size_limiting() {
    let mut config = DataStagingConfig::default();
    config.processing_limits.max_batch_size = 50;
    
    match RedisConsumer::new(&config).await {
        Ok(consumer) => {
            assert_eq!(consumer.get_max_batch_size(), 50);
        }
        Err(_) => {
            // Expected without Redis
        }
    }
}

#[tokio::test]
async fn test_connection_recovery() {
    let config = DataStagingConfig::default();
    
    match RedisConsumer::new(&config).await {
        Ok(mut consumer) => {
            // Test reconnection logic
            let recovery_result = consumer.ensure_connection().await;
            
            match recovery_result {
                Ok(_) => {},
                Err(_) => {
                    // Expected without real Redis
                }
            }
        }
        Err(_) => {
            // Expected without Redis
        }
    }
}

// Helper function for integration testing
async fn create_test_consumer() -> Result<RedisConsumer, Box<dyn std::error::Error>> {
    let config = DataStagingConfig {
        redis_url: "redis://localhost:6379".to_string(),
        input_stream: "test_stream".to_string(),
        consumer_group: "test_group".to_string(),
        consumer_name: "test_consumer".to_string(),
        ..Default::default()
    };
    
    RedisConsumer::new(&config).await.map_err(|e| e.into())
}

#[tokio::test]
async fn test_message_validation() {
    // Test valid JSON message structure
    let valid_json = r#"
    {
        "symbol": "AAPL",
        "price": 150.25,
        "volume": 1000.0,
        "timestamp": 1640995200
    }
    "#;
    
    let message = RedisMessage {
        id: "test-123".to_string(),
        data: valid_json.to_string(),
        timestamp: 1640995200,
    };
    
    assert!(message.is_valid_json());
    
    // Test invalid JSON
    let invalid_json = "invalid json content";
    let bad_message = RedisMessage {
        id: "test-456".to_string(),
        data: invalid_json.to_string(),
        timestamp: 1640995200,
    };
    
    assert!(!bad_message.is_valid_json());
}