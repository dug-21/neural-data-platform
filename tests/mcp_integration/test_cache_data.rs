//! TDD Tests for MCP Cache Data Tool

use anyhow::Result;
use serde_json::json;
use redis::AsyncCommands;
use std::sync::Arc;
use tokio::sync::RwLock;

use autonomous_platform::mcp::trading_tools::TradingMcpTools;
use autonomous_platform::data::RedisCache;
use autonomous_platform::config::load_default_config;

#[tokio::test]
async fn test_get_cache_data_retrieves_existing_key() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    let tools = TradingMcpTools::new(Default::default(), cache.clone(), Default::default(), Default::default());
    
    // Set test data in Redis
    {
        let mut cache_guard = cache.write().await;
        let test_data = json!({
            "price": 45000.0,
            "volume": 1234.5,
            "timestamp": "2024-01-01T00:00:00Z"
        });
        cache_guard.client.set_ex("test:market:btc", test_data.to_string(), 300).await?;
    }
    
    // Act
    let params = json!({
        "key": "test:market:btc"
    });
    
    let result = tools.get_cache_data(params).await?;
    
    // Assert
    assert_eq!(result["key"], "test:market:btc");
    assert!(result["found"], true);
    assert_eq!(result["data"]["price"], 45000.0);
    assert_eq!(result["data"]["volume"], 1234.5);
    assert!(result["ttl"].as_i64().unwrap() > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_get_cache_data_with_pattern_matching() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    let tools = TradingMcpTools::new(Default::default(), cache.clone(), Default::default(), Default::default());
    
    // Set multiple test keys
    {
        let mut cache_guard = cache.write().await;
        cache_guard.client.set_ex("market:btc:latest", json!({"price": 45000}).to_string(), 300).await?;
        cache_guard.client.set_ex("market:eth:latest", json!({"price": 3000}).to_string(), 300).await?;
        cache_guard.client.set_ex("market:sol:latest", json!({"price": 100}).to_string(), 300).await?;
    }
    
    // Act
    let params = json!({
        "pattern": "market:*:latest"
    });
    
    let result = tools.get_cache_data(params).await?;
    
    // Assert
    assert!(result["keys"].is_array());
    let keys = result["keys"].as_array().unwrap();
    assert!(keys.len() >= 3);
    assert!(result["data"].is_object());
    
    Ok(())
}

#[tokio::test]
async fn test_get_cache_data_handles_missing_key() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    let tools = TradingMcpTools::new(Default::default(), cache.clone(), Default::default(), Default::default());
    
    // Act
    let params = json!({
        "key": "nonexistent:key"
    });
    
    let result = tools.get_cache_data(params).await?;
    
    // Assert
    assert_eq!(result["key"], "nonexistent:key");
    assert_eq!(result["found"], false);
    assert!(result["data"].is_null());
    
    Ok(())
}

#[tokio::test]
async fn test_get_cache_data_with_type_info() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    let tools = TradingMcpTools::new(Default::default(), cache.clone(), Default::default(), Default::default());
    
    // Set different data types
    {
        let mut cache_guard = cache.write().await;
        // String
        cache_guard.client.set("cache:string", "simple value").await?;
        // List
        cache_guard.client.rpush("cache:list", vec!["item1", "item2", "item3"]).await?;
        // Hash
        cache_guard.client.hset_multiple("cache:hash", &[("field1", "value1"), ("field2", "value2")]).await?;
    }
    
    // Act & Assert for each type
    let string_result = tools.get_cache_data(json!({"key": "cache:string"})).await?;
    assert_eq!(string_result["type"], "string");
    
    let list_result = tools.get_cache_data(json!({"key": "cache:list"})).await?;
    assert_eq!(list_result["type"], "list");
    assert_eq!(list_result["length"], 3);
    
    let hash_result = tools.get_cache_data(json!({"key": "cache:hash"})).await?;
    assert_eq!(hash_result["type"], "hash");
    
    Ok(())
}