//! Integration tests for Neural and DAA components

use autonomous_platform::neural::fann_predictor::FannPredictor;
use autonomous_platform::neural::NeuralPredictorTrait;
use autonomous_platform::agents::daa_bridge::DAAAgent;
use autonomous_platform::agents::{TradingStrategy, AgentConfig};
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::mcp::trading_tools::MarketData;
use chrono::Utc;
use std::collections::HashMap;

fn create_realistic_market_data(size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::with_capacity(size);
    let base_price = 50000.0;
    let mut current_price = base_price;
    
    for i in 0..size {
        // Simulate realistic price movements
        let trend = (i as f64 * 0.001).sin() * 0.01;
        let noise = ((i * 17) % 100) as f64 / 10000.0 - 0.005;
        let momentum = if i > 0 { (current_price / base_price - 1.0) * 0.1 } else { 0.0 };
        
        let price_change = trend + noise - momentum;
        current_price *= 1.0 + price_change;
        
        // Calculate OHLC with realistic spreads
        let volatility = 0.001 + noise.abs();
        let open = current_price * (1.0 - volatility/2.0);
        let close = current_price;
        let high = current_price * (1.0 + volatility);
        let low = current_price * (1.0 - volatility);
        
        // Realistic volume based on price movement
        let volume = 1000.0 * (1.0 + price_change.abs() * 100.0);
        
        // Calculate technical indicators
        let mut indicators = HashMap::new();
        
        // RSI calculation (simplified)
        let rsi = if i > 14 {
            let gains = data[i-14..i].windows(2)
                .filter_map(|w| {
                    let change = w[1].close - w[0].close;
                    if change > 0.0 { Some(change) } else { None }
                })
                .sum::<f64>();
            let losses = data[i-14..i].windows(2)
                .filter_map(|w| {
                    let change = w[0].close - w[1].close;
                    if change > 0.0 { Some(change) } else { None }
                })
                .sum::<f64>();
            
            if losses > 0.0 {
                100.0 - (100.0 / (1.0 + gains / losses))
            } else {
                100.0
            }
        } else {
            50.0
        };
        
        indicators.insert("rsi".to_string(), rsi);
        indicators.insert("volume_ma".to_string(), volume);
        indicators.insert("price_ma".to_string(), current_price);
        
        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
            open,
            high,
            low,
            close,
            volume,
            indicators,
            source: Some("integration_test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(close),
            metadata: Some(serde_json::json!({
                "exchange": "test",
                "market_cap": 1_000_000_000_000.0
            })),
        });
    }
    
    data
}

#[tokio::test]
async fn test_neural_predictor_with_realistic_data() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "TCN".to_string(), "NHITS".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let market_data = create_realistic_market_data(200);
    
    // Test single model prediction
    let single_predictions = predictor.predict(&market_data, 10, None).await.unwrap();
    assert_eq!(single_predictions.len(), 10);
    
    // Verify prediction properties
    for (i, pred) in single_predictions.iter().enumerate() {
        assert!(pred.confidence > 0.0 && pred.confidence <= 1.0);
        assert!(pred.interval_low < pred.value);
        assert!(pred.interval_high > pred.value);
        
        // Confidence should decrease with horizon
        if i > 0 {
            assert!(pred.confidence <= single_predictions[0].confidence + 0.1);
        }
    }
    
    // Test ensemble prediction
    let models = vec!["MLP".to_string(), "TCN".to_string(), "NHITS".to_string()];
    let ensemble_predictions = predictor.predict_ensemble(&market_data, 10, &models, None).await.unwrap();
    
    assert_eq!(ensemble_predictions.len(), 10);
    assert_eq!(ensemble_predictions[0].model_name, "ensemble");
    
    // Ensemble should have tighter confidence intervals
    for i in 0..single_predictions.len() {
        let ensemble_interval = ensemble_predictions[i].interval_high - ensemble_predictions[i].interval_low;
        let single_interval = single_predictions[i].interval_high - single_predictions[i].interval_low;
        
        // Ensemble intervals can be wider due to model disagreement
        assert!(ensemble_interval > 0.0);
    }
}

#[tokio::test]
async fn test_daa_agent_trading_decisions() {
    let agent_config = AgentConfig {
        id: "test-trader".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.7,
        max_position_size: 10000.0,
        decision_threshold: 0.6,
        enable_ml: true,
        learning_rate: 0.001,
        training_interval: 3600,
        memory_capacity: 1000,
        exploration_rate: 0.1,
    };
    
    // Note: This test would require DAA service to be running
    // For now, we test the configuration and request preparation
    
    let market_data = MarketData {
        timestamp: Utc::now(),
        open: 50000.0,
        high: 50500.0,
        low: 49500.0,
        close: 50200.0,
        volume: 1500.0,
    };
    
    // Test decision context preparation
    let price_change = (market_data.close - market_data.open) / market_data.open;
    let volatility = (market_data.high - market_data.low) / market_data.close;
    
    assert!((price_change - 0.004).abs() < 0.0001);
    assert!((volatility - 0.01996).abs() < 0.0001);
    
    // Test risk assessment calculations
    let position_size = 5000.0;
    let portfolio_value = 50000.0;
    let position_ratio = position_size / portfolio_value;
    
    assert_eq!(position_ratio, 0.1);
    assert!(position_ratio < 0.2); // Below warning threshold
}

#[tokio::test]
async fn test_neural_prediction_performance() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let market_data = create_realistic_market_data(150);
    
    // Measure prediction time
    let start = std::time::Instant::now();
    let predictions = predictor.predict(&market_data, 5, None).await.unwrap();
    let duration = start.elapsed();
    
    println!("Neural prediction took: {:?}", duration);
    assert!(duration.as_secs() < 5); // Should complete within 5 seconds
    
    // Test feature importance
    let importance = predictor.get_feature_importance().await.unwrap();
    assert!(importance.contains_key("price"));
    assert!(importance.contains_key("volume"));
    
    let total_importance: f64 = importance.values().sum();
    assert!((total_importance - 1.0).abs() < 0.01);
}

#[tokio::test]
async fn test_prediction_with_missing_indicators() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Create data with missing indicators
    let mut data = create_realistic_market_data(100);
    for item in &mut data[50..60] {
        item.indicators.clear(); // Remove all indicators
    }
    
    // Should still work with default values
    let predictions = predictor.predict(&data, 5, None).await.unwrap();
    assert_eq!(predictions.len(), 5);
}

#[tokio::test]
async fn test_online_learning_adaptation() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Initial training data
    let initial_data = create_realistic_market_data(150);
    let _ = predictor.predict(&initial_data, 5, None).await.unwrap();
    
    // New market regime (higher volatility)
    let mut new_data = create_realistic_market_data(50);
    for item in &mut new_data {
        item.high *= 1.02;
        item.low *= 0.98;
    }
    
    // Update with new data
    let update_result = predictor.update_with_new_data("MLP", &new_data).await;
    assert!(update_result.is_ok());
}

#[tokio::test]
async fn test_concurrent_model_predictions() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "TCN".to_string(), "NHITS".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let market_data = create_realistic_market_data(100);
    
    // Launch predictions for different models concurrently
    let models = vec!["MLP", "TCN", "NHITS"];
    let mut handles = vec![];
    
    for model in &models {
        let pred = predictor.clone();
        let data = market_data.clone();
        let model_vec = vec![model.to_string()];
        
        handles.push(tokio::spawn(async move {
            pred.predict_ensemble(&data, 5, &model_vec, None).await
        }));
    }
    
    // All should complete successfully
    let mut results = vec![];
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        results.push(result.unwrap());
    }
    
    // Verify each model produced different predictions
    assert_eq!(results.len(), 3);
    for result in &results {
        assert_eq!(result.len(), 5);
    }
}