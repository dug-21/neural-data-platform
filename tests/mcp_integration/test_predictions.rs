//! TDD Tests for MCP Neural Predictions Tool

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use autonomous_platform::mcp::trading_tools::TradingMcpTools;
use autonomous_platform::neural::{NeuralPredictor, NeuralConfig};
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::config::load_default_config;

#[tokio::test]
async fn test_request_prediction_single_symbol() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["NHITS".to_string()],
        device: "cpu".to_string(),
        batch_size: 32,
        optimization_level: 2,
    };
    
    let predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), predictor.clone(), Default::default());
    
    // Prepare historical data for prediction
    let historical_data = vec![
        TimeSeriesData {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(5),
            symbol: "BTC/USD".to_string(),
            value: 44800.0,
            volume: Some(1000.0),
            metadata: Default::default(),
        },
        TimeSeriesData {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(4),
            symbol: "BTC/USD".to_string(),
            value: 44900.0,
            volume: Some(1100.0),
            metadata: Default::default(),
        },
        TimeSeriesData {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(3),
            symbol: "BTC/USD".to_string(),
            value: 45000.0,
            volume: Some(1200.0),
            metadata: Default::default(),
        },
        TimeSeriesData {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(2),
            symbol: "BTC/USD".to_string(),
            value: 45100.0,
            volume: Some(1300.0),
            metadata: Default::default(),
        },
        TimeSeriesData {
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(1),
            symbol: "BTC/USD".to_string(),
            value: 45200.0,
            volume: Some(1400.0),
            metadata: Default::default(),
        },
    ];
    
    // Load data into predictor
    predictor.load_historical_data(historical_data).await?;
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "horizon": 5,
        "confidence_threshold": 0.7
    });
    
    let result = tools.request_prediction(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "BTC/USD");
    assert_eq!(result["horizon"], 5);
    assert!(result["predictions"].is_array());
    
    let predictions = result["predictions"].as_array().unwrap();
    assert_eq!(predictions.len(), 5);
    
    // Verify prediction structure
    for prediction in predictions {
        assert!(prediction["timestamp"].is_string());
        assert!(prediction["value"].is_number());
        assert!(prediction["confidence"].as_f64().unwrap() >= 0.0);
        assert!(prediction["confidence"].as_f64().unwrap() <= 1.0);
    }
    
    // Check metadata
    assert!(result["model_used"].is_string());
    assert!(result["computation_time_ms"].is_number());
    
    Ok(())
}

#[tokio::test]
async fn test_request_prediction_with_multiple_models() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let neural_config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["NHITS".to_string(), "TCN".to_string(), "DeepAR".to_string()],
        device: "cpu".to_string(),
        batch_size: 32,
        optimization_level: 2,
    };
    
    let predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), predictor.clone(), Default::default());
    
    // Act
    let params = json!({
        "symbol": "ETH/USD",
        "horizon": 10,
        "ensemble": true,
        "models": ["NHITS", "TCN"]
    });
    
    let result = tools.request_prediction(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "ETH/USD");
    assert!(result["ensemble"], true);
    assert!(result["models_used"].is_array());
    
    let models_used = result["models_used"].as_array().unwrap();
    assert!(models_used.contains(&json!("NHITS")));
    assert!(models_used.contains(&json!("TCN")));
    
    // Ensemble should have aggregated predictions
    assert!(result["predictions"].is_array());
    assert!(result["prediction_intervals"].is_object());
    
    Ok(())
}

#[tokio::test]
async fn test_request_prediction_with_features() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        device: "cpu".to_string(),
        batch_size: 32,
        optimization_level: 2,
    };
    
    let predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), predictor.clone(), Default::default());
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "horizon": 3,
        "features": {
            "technical_indicators": ["RSI", "MACD", "BB"],
            "market_sentiment": 0.75,
            "volume_profile": true
        }
    });
    
    let result = tools.request_prediction(params).await?;
    
    // Assert
    assert!(result["features_used"].is_object());
    assert!(result["feature_importance"].is_object());
    
    Ok(())
}

#[tokio::test]
async fn test_request_prediction_handles_invalid_horizon() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["NHITS".to_string()],
        device: "cpu".to_string(),
        batch_size: 32,
        optimization_level: 2,
    };
    
    let predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), predictor.clone(), Default::default());
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "horizon": 1000  // Too large
    });
    
    let result = tools.request_prediction(params).await;
    
    // Assert
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("horizon"));
    
    Ok(())
}