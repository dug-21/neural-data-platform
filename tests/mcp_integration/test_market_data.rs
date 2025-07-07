//! TDD Tests for MCP Market Data Tool

use anyhow::Result;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

use autonomous_platform::mcp::trading_tools::TradingMcpTools;
use autonomous_platform::data::TimescaleDBStorage;
use autonomous_platform::config::{DatabaseConfig, load_default_config};

#[tokio::test]
async fn test_query_market_data_returns_real_data() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database)?);
    let tools = TradingMcpTools::new(storage.clone(), Default::default(), Default::default(), Default::default());
    
    // Insert test data
    sqlx::query!(
        r#"
        INSERT INTO market_data (timestamp, symbol, open, high, low, close, volume)
        VALUES (NOW(), $1, $2, $3, $4, $5, $6)
        "#,
        "BTC/USD",
        45000.0,
        45500.0,
        44800.0,
        45200.0,
        1000.0
    )
    .execute(&storage.pool)
    .await?;
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "interval": "1m",
        "limit": 10
    });
    
    let result = tools.query_market_data(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "BTC/USD");
    assert!(result["data"].is_array());
    let data = result["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data[0]["symbol"], "BTC/USD");
    assert_eq!(data[0]["close"], 45200.0);
    
    Ok(())
}

#[tokio::test]
async fn test_query_market_data_with_time_range() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database)?);
    let tools = TradingMcpTools::new(storage.clone(), Default::default(), Default::default(), Default::default());
    
    // Act
    let params = json!({
        "symbol": "ETH/USD",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T01:00:00Z",
        "interval": "5m"
    });
    
    let result = tools.query_market_data(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "ETH/USD");
    assert_eq!(result["interval"], "5m");
    assert!(result["data"].is_array());
    
    Ok(())
}

#[tokio::test]
async fn test_query_market_data_aggregated() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database)?);
    let tools = TradingMcpTools::new(storage.clone(), Default::default(), Default::default(), Default::default());
    
    // Insert multiple data points
    for i in 0..10 {
        sqlx::query!(
            r#"
            INSERT INTO market_data (timestamp, symbol, open, high, low, close, volume)
            VALUES (NOW() - INTERVAL '1 minute' * $1, $2, $3, $4, $5, $6, $7)
            "#,
            i as i32,
            "BTC/USD",
            45000.0 + (i as f64 * 10.0),
            45500.0 + (i as f64 * 10.0),
            44800.0 + (i as f64 * 10.0),
            45200.0 + (i as f64 * 10.0),
            1000.0
        )
        .execute(&storage.pool)
        .await?;
    }
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "interval": "15m",
        "aggregation": "ohlc"
    });
    
    let result = tools.query_market_data(params).await?;
    
    // Assert
    assert!(result["data"].is_array());
    let data = result["data"].as_array().unwrap();
    
    // Verify aggregation worked
    if !data.is_empty() {
        assert!(data[0]["high"].as_f64().unwrap() >= data[0]["low"].as_f64().unwrap());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_query_market_data_handles_missing_symbol() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;
    
    let storage = Arc::new(TimescaleDBStorage::new(pool, config.database)?);
    let tools = TradingMcpTools::new(storage.clone(), Default::default(), Default::default(), Default::default());
    
    // Act
    let params = json!({
        "symbol": "NONEXISTENT/PAIR",
        "limit": 10
    });
    
    let result = tools.query_market_data(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "NONEXISTENT/PAIR");
    assert!(result["data"].is_array());
    assert_eq!(result["data"].as_array().unwrap().len(), 0);
    
    Ok(())
}