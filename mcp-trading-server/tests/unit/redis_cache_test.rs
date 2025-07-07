use mcp_trading_server::tools::cache::CacheTool;
use mcp_trading_server::integrations::redis::RedisClient;
use redis::AsyncCommands;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn test_get_cached_price() {
    // Arrange
    let redis_client = setup_test_redis().await;
    let cache_tool = CacheTool::new(redis_client);
    
    // Pre-populate cache
    let test_price = json!({
        "symbol": "BTC/USD",
        "price": 50000.0,
        "timestamp": "2025-01-05T10:00:00Z"
    });
    cache_tool.set_price("BTC/USD", &test_price, Duration::from_secs(60)).await.unwrap();
    
    // Act
    let result = cache_tool.get_cached_price("BTC/USD").await;
    
    // Assert
    assert!(result.is_ok());
    let cached_price = result.unwrap();
    assert!(cached_price.is_some());
    assert_eq!(cached_price.unwrap(), test_price);
}

#[tokio::test]
async fn test_get_cached_indicators() {
    // Arrange
    let redis_client = setup_test_redis().await;
    let cache_tool = CacheTool::new(redis_client);
    
    // Pre-populate cache
    let test_indicators = json!({
        "rsi": 65.5,
        "macd": {
            "macd": 100.5,
            "signal": 95.2,
            "histogram": 5.3
        },
        "timestamp": "2025-01-05T10:00:00Z"
    });
    cache_tool.set_indicators("BTC/USD", &test_indicators, Duration::from_secs(300)).await.unwrap();
    
    // Act
    let result = cache_tool.get_cached_indicators("BTC/USD").await;
    
    // Assert
    assert!(result.is_ok());
    let cached_indicators = result.unwrap();
    assert!(cached_indicators.is_some());
    assert_eq!(cached_indicators.unwrap()["rsi"], 65.5);
}

#[tokio::test]
async fn test_cache_expiration() {
    // Arrange
    let redis_client = setup_test_redis().await;
    let cache_tool = CacheTool::new(redis_client);
    
    // Set with very short TTL
    let test_data = json!({"test": "data"});
    cache_tool.set_general("test_key", &test_data, Duration::from_millis(100)).await.unwrap();
    
    // Act - immediate get should work
    let immediate_result = cache_tool.get_general("test_key").await.unwrap();
    assert!(immediate_result.is_some());
    
    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Act - after expiration should return None
    let expired_result = cache_tool.get_general("test_key").await.unwrap();
    
    // Assert
    assert!(expired_result.is_none());
}

#[tokio::test]
async fn test_batch_cache_operations() {
    // Arrange
    let redis_client = setup_test_redis().await;
    let cache_tool = CacheTool::new(redis_client);
    
    // Set multiple items
    let symbols = vec!["BTC/USD", "ETH/USD", "SOL/USD"];
    for (i, symbol) in symbols.iter().enumerate() {
        let price = json!({
            "symbol": symbol,
            "price": 50000.0 + (i as f64 * 1000.0),
            "timestamp": "2025-01-05T10:00:00Z"
        });
        cache_tool.set_price(symbol, &price, Duration::from_secs(60)).await.unwrap();
    }
    
    // Act - get all prices
    let mut results = Vec::new();
    for symbol in &symbols {
        let result = cache_tool.get_cached_price(symbol).await.unwrap();
        results.push(result);
    }
    
    // Assert
    assert_eq!(results.len(), 3);
    for result in results {
        assert!(result.is_some());
    }
}

async fn setup_test_redis() -> RedisClient {
    let client = redis::Client::open("redis://127.0.0.1:6379/1").unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    
    // Clear test database
    let _: () = redis::cmd("FLUSHDB").query_async(&mut conn).await.unwrap();
    
    RedisClient::new(conn)
}