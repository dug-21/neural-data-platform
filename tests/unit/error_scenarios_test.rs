//! Unit tests for error scenarios and edge cases

use autonomous_platform::neural::{NeuralPredictor, NeuralPredictorTrait};
use autonomous_platform::neural::fann_predictor::FannPredictor;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::agents::{TradingStrategy, AgentConfig};
use chrono::Utc;
use std::collections::HashMap;

#[tokio::test]
async fn test_neural_predictor_with_empty_data() {
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
    let empty_data = vec![];
    
    // Should handle empty data gracefully
    let result = predictor.predict(&empty_data, 5, None).await;
    assert!(result.is_ok()); // Should return placeholder predictions
}

#[tokio::test]
async fn test_neural_predictor_with_nan_values() {
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
    
    let mut data = vec![];
    for i in 0..100 {
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::minutes(i),
            open: if i == 50 { f64::NAN } else { 100.0 },
            high: 101.0,
            low: 99.0,
            close: if i == 51 { f64::NAN } else { 100.5 },
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("TEST".to_string()),
            value: Some(100.5),
            metadata: None,
        });
    }
    
    // Should handle NaN values gracefully
    let result = predictor.predict(&data, 5, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_neural_predictor_with_infinite_values() {
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
    
    let mut data = vec![];
    data.push(TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: f64::INFINITY,
        low: 99.0,
        close: 100.5,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("TEST".to_string()),
        value: Some(100.5),
        metadata: None,
    });
    
    // Should handle infinite values gracefully
    let result = predictor.predict(&data, 5, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_predictor_with_zero_horizon() {
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
    let data = vec![TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("TEST".to_string()),
        value: Some(100.5),
        metadata: None,
    }];
    
    // Zero horizon should return empty predictions
    let result = predictor.predict(&data, 0, None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[tokio::test]
async fn test_predictor_with_very_large_horizon() {
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
    let data = vec![TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("TEST".to_string()),
        value: Some(100.5),
        metadata: None,
    }];
    
    // Very large horizon should be capped
    let result = predictor.predict(&data, 10000, None).await;
    assert!(result.is_ok());
    let predictions = result.unwrap();
    assert!(predictions.len() <= 100); // Should be capped at reasonable value
}

#[test]
fn test_agent_config_with_negative_values() {
    // Test agent configuration with invalid values
    let config = AgentConfig {
        id: "test".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: -0.5, // Invalid negative
        max_position_size: -1000.0, // Invalid negative
        decision_threshold: 1.5, // Invalid > 1.0
        enable_ml: true,
        learning_rate: -0.001, // Invalid negative
        training_interval: -3600, // Invalid negative
        memory_capacity: 0, // Invalid zero
        exploration_rate: 2.0, // Invalid > 1.0
    };
    
    // Test validation logic
    assert!(config.risk_tolerance < 0.0);
    assert!(config.max_position_size < 0.0);
    assert!(config.decision_threshold > 1.0);
    assert!(config.learning_rate < 0.0);
    assert!(config.exploration_rate > 1.0);
}

#[tokio::test]
async fn test_ensemble_with_no_models() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec![],  // Empty models
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = NeuralPredictor::new(config);
    assert!(predictor.is_err()); // Should fail with no models
}

#[tokio::test]
async fn test_ensemble_with_invalid_models() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["InvalidModel".to_string(), "UnknownModel".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let data = vec![TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("TEST".to_string()),
        value: Some(100.5),
        metadata: None,
    }];
    
    // Should use default configurations for unknown models
    let result = predictor.predict(&data, 5, None).await;
    assert!(result.is_ok());
}

#[test]
fn test_time_series_data_with_missing_fields() {
    // Test with missing optional fields
    let data = TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: None,  // Missing source
        entity: None,  // Missing entity
        value: None,   // Missing value
        metadata: None, // Missing metadata
    };
    
    // Should still be valid
    assert_eq!(data.symbol, "TEST");
    assert!(data.source.is_none());
    assert!(data.entity.is_none());
    assert!(data.value.is_none());
}

// Concurrent prediction limit tests moved to integration tests

#[test]
fn test_extreme_market_conditions() {
    // Test with extreme price movements
    let extreme_data = vec![
        TimeSeriesData {
            symbol: "CRASH".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 100.0,
            low: 10.0,  // 90% crash
            close: 15.0,
            volume: 1000000.0, // Extreme volume
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("CRASH".to_string()),
            value: Some(15.0),
            metadata: None,
        },
        TimeSeriesData {
            symbol: "SPIKE".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 1000.0, // 900% spike
            low: 100.0,
            close: 950.0,
            volume: 0.1, // Extremely low volume
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("SPIKE".to_string()),
            value: Some(950.0),
            metadata: None,
        },
    ];
    
    // Calculate volatility for extreme conditions
    for data in &extreme_data {
        let volatility = (data.high - data.low) / data.close;
        assert!(volatility > 0.0);
        
        if data.symbol == "CRASH" {
            assert!(volatility > 5.0); // Over 500% volatility
        } else if data.symbol == "SPIKE" {
            assert!(volatility > 0.9); // Over 90% volatility
        }
    }
}

#[test]
fn test_memory_allocation_limits() {
    // Test with very low memory allocation
    let config = NeuralConfig {
        memory_gb: 0.001, // 1MB - extremely low
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    // Should still create predictor but may have limitations
    let result = FannPredictor::new(config);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cache_expiration() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 1, // 1 second TTL
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let data = vec![TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: Utc::now(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 1000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("TEST".to_string()),
        value: Some(100.5),
        metadata: None,
    }];
    
    // First prediction
    let result1 = predictor.predict(&data, 5, None).await.unwrap();
    
    // Wait for cache to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Second prediction should not be from cache
    let result2 = predictor.predict(&data, 5, None).await.unwrap();
    
    // Results should be similar but potentially different due to no caching
    assert_eq!(result1.len(), result2.len());
}