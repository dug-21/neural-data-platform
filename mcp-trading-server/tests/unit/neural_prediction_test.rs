use mcp_trading_server::tools::neural::NeuralPredictionTool;
use mcp_trading_server::integrations::neural::NeuralClient;
use serde_json::json;
use mockall::predicate::*;

#[tokio::test]
async fn test_get_price_prediction() {
    // Arrange
    let neural_client = setup_test_neural_client().await;
    let neural_tool = NeuralPredictionTool::new(neural_client);
    
    // Act
    let result = neural_tool.get_price_prediction(
        "BTC/USD",
        "1h",
        24
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let prediction = result.unwrap();
    assert_eq!(prediction["symbol"], "BTC/USD");
    assert_eq!(prediction["timeframe"], "1h");
    assert!(prediction["predictions"].as_array().unwrap().len() == 24);
    assert!(prediction["confidence"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn test_get_trend_analysis() {
    // Arrange
    let neural_client = setup_test_neural_client().await;
    let neural_tool = NeuralPredictionTool::new(neural_client);
    
    // Act
    let result = neural_tool.get_trend_analysis("BTC/USD").await;
    
    // Assert
    assert!(result.is_ok());
    let analysis = result.unwrap();
    assert!(["bullish", "bearish", "neutral"].contains(&analysis["trend"].as_str().unwrap()));
    assert!(analysis["strength"].as_f64().unwrap() >= 0.0);
    assert!(analysis["strength"].as_f64().unwrap() <= 1.0);
}

#[tokio::test]
async fn test_get_pattern_recognition() {
    // Arrange
    let neural_client = setup_test_neural_client().await;
    let neural_tool = NeuralPredictionTool::new(neural_client);
    
    // Act
    let result = neural_tool.get_pattern_recognition("BTC/USD", "4h").await;
    
    // Assert
    assert!(result.is_ok());
    let patterns = result.unwrap();
    assert!(patterns["patterns"].as_array().is_some());
    
    if let Some(pattern_array) = patterns["patterns"].as_array() {
        for pattern in pattern_array {
            assert!(pattern["name"].as_str().is_some());
            assert!(pattern["confidence"].as_f64().unwrap() >= 0.0);
            assert!(pattern["confidence"].as_f64().unwrap() <= 1.0);
        }
    }
}

#[tokio::test]
async fn test_get_risk_assessment() {
    // Arrange
    let neural_client = setup_test_neural_client().await;
    let neural_tool = NeuralPredictionTool::new(neural_client);
    
    // Act
    let result = neural_tool.get_risk_assessment(
        "BTC/USD",
        10000.0,
        "long"
    ).await;
    
    // Assert
    assert!(result.is_ok());
    let risk = result.unwrap();
    assert!(risk["risk_score"].as_f64().unwrap() >= 0.0);
    assert!(risk["risk_score"].as_f64().unwrap() <= 100.0);
    assert!(risk["stop_loss"].as_f64().is_some());
    assert!(risk["take_profit"].as_f64().is_some());
    assert_eq!(risk["position_type"], "long");
}

#[tokio::test]
async fn test_batch_predictions() {
    // Arrange
    let neural_client = setup_test_neural_client().await;
    let neural_tool = NeuralPredictionTool::new(neural_client);
    
    // Act
    let symbols = vec!["BTC/USD", "ETH/USD", "SOL/USD"];
    let mut results = Vec::new();
    
    for symbol in symbols {
        let result = neural_tool.get_price_prediction(symbol, "1h", 6).await;
        results.push(result);
    }
    
    // Assert
    assert_eq!(results.len(), 3);
    for result in results {
        assert!(result.is_ok());
    }
}

async fn setup_test_neural_client() -> NeuralClient {
    // For unit tests, we'll use a real neural client that connects to the actual service
    // In integration tests, we'll verify the full flow
    NeuralClient::new("http://localhost:8001").await.unwrap()
}