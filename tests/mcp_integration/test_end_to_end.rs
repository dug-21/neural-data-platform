//! End-to-end integration tests for MCP tools

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::postgres::PgPoolOptions;

use autonomous_platform::{
    config::load_default_config,
    data::{TimescaleDBStorage, RedisCache},
    neural::NeuralPredictor,
    agents::{AutonomousAgent, AgentConfig, TradingStrategy},
    monitoring::HealthMonitor,
    mcp::TradingMcpTools,
};

#[tokio::test]
async fn test_full_trading_workflow() -> Result<()> {
    // Arrange - Initialize all components
    let config = load_default_config()?;
    
    // Database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database.clone())?);
    
    // Cache
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    
    // Neural predictor
    let predictor = Arc::new(NeuralPredictor::new(config.neural.clone())?);
    
    // Agent
    let agent_config = AgentConfig {
        id: "test-agent".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    
    // Health monitor
    let monitor = Arc::new(HealthMonitor::new(config.clone()));
    monitor.start_monitoring().await?;
    
    // Create MCP tools
    let mut tools = TradingMcpTools::new(storage.clone(), cache.clone(), predictor, agent);
    tools = TradingMcpTools::with_monitor(monitor);
    
    // Insert test market data
    for i in 0..50 {
        sqlx::query!(
            r#"
            INSERT INTO market_data (timestamp, symbol, open, high, low, close, volume)
            VALUES (NOW() - INTERVAL '1 minute' * $1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (timestamp, symbol) DO NOTHING
            "#,
            i as i32,
            "BTC/USD",
            45000.0 + (i as f64 * 10.0),
            45100.0 + (i as f64 * 10.0),
            44900.0 + (i as f64 * 10.0),
            45050.0 + (i as f64 * 10.0),
            1000.0 + (i as f64 * 10.0)
        )
        .execute(&storage.pool)
        .await?;
    }
    
    // Cache some data
    {
        let mut cache_guard = cache.write().await;
        cache_guard.set_json("market:btc:latest", &json!({
            "price": 45500.0,
            "volume": 2000.0,
            "trend": "bullish"
        }), 300).await?;
    }
    
    // Act & Assert - Test complete workflow
    
    // 1. Check system status
    let status = tools.system_status(json!({"detailed": true})).await?;
    assert_eq!(status["status"], "operational");
    
    // 2. Query market data
    let market_data = tools.query_market_data(json!({
        "symbol": "BTC/USD",
        "limit": 10
    })).await?;
    assert_eq!(market_data["symbol"], "BTC/USD");
    assert!(market_data["data"].as_array().unwrap().len() > 0);
    
    // 3. Check cache
    let cache_data = tools.get_cache_data(json!({
        "key": "market:btc:latest"
    })).await?;
    assert_eq!(cache_data["found"], true);
    assert_eq!(cache_data["data"]["price"], 45500.0);
    
    // 4. Get prediction
    let prediction = tools.request_prediction(json!({
        "symbol": "BTC/USD",
        "horizon": 5
    })).await?;
    assert_eq!(prediction["symbol"], "BTC/USD");
    assert_eq!(prediction["predictions"].as_array().unwrap().len(), 5);
    
    // 5. Get trading decision
    let decision = tools.agent_decision(json!({
        "symbol": "BTC/USD",
        "position_size": 5000.0,
        "portfolio_value": 100000.0
    })).await?;
    assert!(["buy", "sell", "hold"].contains(&decision["decision"].as_str().unwrap()));
    
    Ok(())
}

#[tokio::test]
async fn test_mcp_tools_error_handling() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database.clone())?);
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    let predictor = Arc::new(NeuralPredictor::new(config.neural.clone())?);
    let agent_config = AgentConfig {
        id: "test-agent".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    
    let tools = TradingMcpTools::new(storage, cache, predictor, agent);
    
    // Test invalid parameters
    
    // Invalid prediction horizon
    let result = tools.request_prediction(json!({
        "symbol": "BTC/USD",
        "horizon": 1000
    })).await;
    assert!(result.is_err());
    
    // Missing required parameter
    let result = tools.query_market_data(json!({})).await;
    assert!(result.is_ok()); // Should use default symbol
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_mcp_operations() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database.url)
        .await?;
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database.clone())?);
    let cache = Arc::new(RwLock::new(RedisCache::new(&config.redis).await?));
    let predictor = Arc::new(NeuralPredictor::new(config.neural.clone())?);
    let agent_config = AgentConfig {
        id: "test-agent".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    
    let tools = Arc::new(TradingMcpTools::new(storage, cache, predictor, agent));
    
    // Act - Run multiple operations concurrently
    let mut handles = vec![];
    
    // Market data queries
    for symbol in ["BTC/USD", "ETH/USD", "SOL/USD"] {
        let tools_clone = tools.clone();
        let handle = tokio::spawn(async move {
            tools_clone.query_market_data(json!({
                "symbol": symbol,
                "limit": 5
            })).await
        });
        handles.push(handle);
    }
    
    // Cache operations
    for key in ["cache:1", "cache:2", "cache:3"] {
        let tools_clone = tools.clone();
        let handle = tokio::spawn(async move {
            tools_clone.get_cache_data(json!({
                "key": key
            })).await
        });
        handles.push(handle);
    }
    
    // Assert - All operations should complete successfully
    for handle in handles {
        let result = handle.await?;
        assert!(result.is_ok());
    }
    
    Ok(())
}