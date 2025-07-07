use mcp_trading_server::tools::market_data::MarketDataTool;
use mcp_trading_server::integrations::database::DatabaseClient;
use tokio_postgres::{NoTls, Client};
use chrono::{DateTime, Utc};
use serde_json::json;

#[tokio::test]
async fn test_get_latest_price() {
    // Arrange
    let db_client = setup_test_database().await;
    let market_data_tool = MarketDataTool::new(db_client);
    
    // Act
    let result = market_data_tool.get_latest_price("BTC/USD").await;
    
    // Assert
    assert!(result.is_ok());
    let price_data = result.unwrap();
    assert_eq!(price_data["symbol"], "BTC/USD");
    assert!(price_data["price"].as_f64().unwrap() > 0.0);
    assert!(price_data["timestamp"].as_str().is_some());
}

#[tokio::test]
async fn test_get_historical_data() {
    // Arrange
    let db_client = setup_test_database().await;
    let market_data_tool = MarketDataTool::new(db_client);
    let start_time = Utc::now() - chrono::Duration::hours(24);
    let end_time = Utc::now();
    
    // Act
    let result = market_data_tool.get_historical_data(
        "BTC/USD",
        "1h",
        start_time,
        end_time
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let historical_data = result.unwrap();
    assert!(historical_data.as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_get_orderbook() {
    // Arrange
    let db_client = setup_test_database().await;
    let market_data_tool = MarketDataTool::new(db_client);
    
    // Act
    let result = market_data_tool.get_orderbook("BTC/USD", 10).await;
    
    // Assert
    assert!(result.is_ok());
    let orderbook = result.unwrap();
    assert!(orderbook["bids"].as_array().is_some());
    assert!(orderbook["asks"].as_array().is_some());
    assert!(orderbook["timestamp"].as_str().is_some());
}

#[tokio::test]
async fn test_get_market_stats() {
    // Arrange
    let db_client = setup_test_database().await;
    let market_data_tool = MarketDataTool::new(db_client);
    
    // Act
    let result = market_data_tool.get_market_stats("BTC/USD", "24h").await;
    
    // Assert
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert!(stats["volume"].as_f64().is_some());
    assert!(stats["high"].as_f64().is_some());
    assert!(stats["low"].as_f64().is_some());
    assert!(stats["change_percent"].as_f64().is_some());
}

async fn setup_test_database() -> DatabaseClient {
    let config = tokio_postgres::config::Config::from_str(
        "host=localhost dbname=neural_trader_test user=postgres password=postgres"
    ).unwrap();
    
    let (client, connection) = config.connect(NoTls).await.unwrap();
    
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database connection error: {}", e);
        }
    });
    
    DatabaseClient::new(client)
}