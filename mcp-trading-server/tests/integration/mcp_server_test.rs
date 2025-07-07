use mcp_trading_server::MCPTradingServer;
use mcp_sdk::{Client, Transport};
use serde_json::json;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_mcp_server_startup_and_shutdown() {
    // Arrange
    let server = MCPTradingServer::new().await.unwrap();
    
    // Act
    let handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(500)).await;
    
    // Assert server is running
    let client = Client::connect_stdio().await;
    assert!(client.is_ok());
    
    // Cleanup
    handle.abort();
}

#[tokio::test]
async fn test_market_data_tool_integration() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act
    let result = client.call_tool(
        "get_latest_price",
        json!({
            "symbol": "BTC/USD"
        })
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response["price"].as_f64().is_some());
    assert_eq!(response["symbol"], "BTC/USD");
}

#[tokio::test]
async fn test_cache_tool_integration() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act - First call should hit database
    let first_call = client.call_tool(
        "get_cached_price",
        json!({
            "symbol": "ETH/USD"
        })
    ).await.unwrap();
    
    // Second call should be from cache (faster)
    let start = std::time::Instant::now();
    let second_call = client.call_tool(
        "get_cached_price",
        json!({
            "symbol": "ETH/USD"
        })
    ).await.unwrap();
    let cache_time = start.elapsed();
    
    // Assert
    assert_eq!(first_call, second_call);
    assert!(cache_time.as_millis() < 10); // Cache should be very fast
}

#[tokio::test]
async fn test_neural_prediction_integration() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act
    let result = client.call_tool(
        "get_price_prediction",
        json!({
            "symbol": "BTC/USD",
            "timeframe": "1h",
            "periods": 12
        })
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let prediction = result.unwrap();
    assert!(prediction["predictions"].as_array().unwrap().len() == 12);
    assert!(prediction["confidence"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn test_trading_decision_integration() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act
    let result = client.call_tool(
        "get_trading_signal",
        json!({
            "symbol": "BTC/USD"
        })
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let signal = result.unwrap();
    assert!(["buy", "sell", "hold"].contains(&signal["action"].as_str().unwrap()));
}

#[tokio::test]
async fn test_health_monitoring_integration() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act
    let result = client.call_tool(
        "get_system_status",
        json!({})
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let status = result.unwrap();
    assert!(status["overall_status"].as_str().is_some());
    assert!(status["components"].as_object().is_some());
}

#[tokio::test]
async fn test_concurrent_tool_calls() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act - Make multiple concurrent calls
    let futures = vec![
        client.call_tool("get_latest_price", json!({"symbol": "BTC/USD"})),
        client.call_tool("get_cached_price", json!({"symbol": "ETH/USD"})),
        client.call_tool("get_price_prediction", json!({"symbol": "SOL/USD", "timeframe": "1h", "periods": 6})),
        client.call_tool("get_trading_signal", json!({"symbol": "BTC/USD"})),
        client.call_tool("get_system_status", json!({})),
    ];
    
    let results = futures::future::join_all(futures).await;
    
    // Assert all calls succeeded
    for result in results {
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_error_handling() {
    // Arrange
    let server = start_test_server().await;
    let client = Client::connect_stdio().await.unwrap();
    
    // Act - Call with invalid parameters
    let result = client.call_tool(
        "get_latest_price",
        json!({
            "symbol": "INVALID/SYMBOL"
        })
    ).await;
    
    // Assert
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Symbol not found"));
}

async fn start_test_server() -> tokio::task::JoinHandle<()> {
    let server = MCPTradingServer::new().await.unwrap();
    tokio::spawn(async move {
        server.start().await.unwrap();
    })
}