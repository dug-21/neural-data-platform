use mcp_trading_server::tools::trading::TradingDecisionTool;
use mcp_trading_server::integrations::agent::AgentClient;
use serde_json::json;

#[tokio::test]
async fn test_get_trading_signal() {
    // Arrange
    let agent_client = setup_test_agent_client().await;
    let trading_tool = TradingDecisionTool::new(agent_client);
    
    // Act
    let result = trading_tool.get_trading_signal("BTC/USD").await;
    
    // Assert
    assert!(result.is_ok());
    let signal = result.unwrap();
    assert!(["buy", "sell", "hold"].contains(&signal["action"].as_str().unwrap()));
    assert!(signal["confidence"].as_f64().unwrap() >= 0.0);
    assert!(signal["confidence"].as_f64().unwrap() <= 1.0);
    assert!(signal["reasoning"].as_str().is_some());
}

#[tokio::test]
async fn test_execute_trade() {
    // Arrange
    let agent_client = setup_test_agent_client().await;
    let trading_tool = TradingDecisionTool::new(agent_client);
    
    // Act
    let result = trading_tool.execute_trade(
        "BTC/USD",
        "buy",
        0.1,
        Some(45000.0),
        Some(48000.0),
        Some(44000.0)
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let trade = result.unwrap();
    assert!(trade["order_id"].as_str().is_some());
    assert_eq!(trade["status"], "pending");
    assert_eq!(trade["symbol"], "BTC/USD");
    assert_eq!(trade["side"], "buy");
    assert_eq!(trade["quantity"], 0.1);
}

#[tokio::test]
async fn test_get_portfolio_status() {
    // Arrange
    let agent_client = setup_test_agent_client().await;
    let trading_tool = TradingDecisionTool::new(agent_client);
    
    // Act
    let result = trading_tool.get_portfolio_status().await;
    
    // Assert
    assert!(result.is_ok());
    let portfolio = result.unwrap();
    assert!(portfolio["total_value"].as_f64().is_some());
    assert!(portfolio["positions"].as_array().is_some());
    assert!(portfolio["cash_balance"].as_f64().is_some());
    assert!(portfolio["pnl"].as_object().is_some());
}

#[tokio::test]
async fn test_get_active_orders() {
    // Arrange
    let agent_client = setup_test_agent_client().await;
    let trading_tool = TradingDecisionTool::new(agent_client);
    
    // Act
    let result = trading_tool.get_active_orders().await;
    
    // Assert
    assert!(result.is_ok());
    let orders = result.unwrap();
    assert!(orders.as_array().is_some());
    
    if let Some(order_array) = orders.as_array() {
        for order in order_array {
            assert!(order["order_id"].as_str().is_some());
            assert!(order["symbol"].as_str().is_some());
            assert!(order["status"].as_str().is_some());
        }
    }
}

#[tokio::test]
async fn test_cancel_order() {
    // Arrange
    let agent_client = setup_test_agent_client().await;
    let trading_tool = TradingDecisionTool::new(agent_client);
    
    // First create an order
    let trade_result = trading_tool.execute_trade(
        "BTC/USD",
        "buy",
        0.1,
        Some(45000.0),
        None,
        None
    ).await.unwrap();
    
    let order_id = trade_result["order_id"].as_str().unwrap();
    
    // Act
    let result = trading_tool.cancel_order(order_id).await;
    
    // Assert
    assert!(result.is_ok());
    let cancel_result = result.unwrap();
    assert_eq!(cancel_result["order_id"], order_id);
    assert_eq!(cancel_result["status"], "cancelled");
}

#[tokio::test]
async fn test_get_trading_strategy() {
    // Arrange
    let agent_client = setup_test_agent_client().await;
    let trading_tool = TradingDecisionTool::new(agent_client);
    
    // Act
    let result = trading_tool.get_trading_strategy("BTC/USD").await;
    
    // Assert
    assert!(result.is_ok());
    let strategy = result.unwrap();
    assert!(strategy["strategy_name"].as_str().is_some());
    assert!(strategy["parameters"].as_object().is_some());
    assert!(strategy["risk_parameters"].as_object().is_some());
}

async fn setup_test_agent_client() -> AgentClient {
    // Connect to actual agent service for real implementation testing
    AgentClient::new("http://localhost:8002").await.unwrap()
}