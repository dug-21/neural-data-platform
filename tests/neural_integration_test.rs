//! Integration tests for neural network predictions with ruv-fann

use autonomous_platform::neural::{FannPredictor, FannModelConfig, NeuralPredictorTrait};
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::config::NeuralConfig;
use chrono::{Utc, Duration};
use std::collections::HashMap;

#[tokio::test]
async fn test_fann_predictor_creation() {
    let config = NeuralConfig {
        models: vec!["NHITS".to_string(), "TCN".to_string()],
        horizon: 10,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config);
    assert!(predictor.is_ok());
}

#[tokio::test]
async fn test_single_model_prediction() {
    let config = NeuralConfig {
        models: vec!["MLP".to_string()],
        horizon: 5,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Create test data
    let mut data = Vec::new();
    let base_time = Utc::now();
    for i in 0..50 {
        data.push(TimeSeriesData {
            timestamp: base_time - Duration::minutes(i as i64),
            value: 100.0 + (i as f64).sin() * 10.0,
            volume: 1000.0,
        });
    }
    
    let predictions = predictor.predict(&data, 5, None).await;
    assert!(predictions.is_ok());
    let results = predictions.unwrap();
    assert_eq!(results.len(), 5);
    
    // Verify predictions have proper structure
    for pred in &results {
        assert!(pred.confidence > 0.0 && pred.confidence <= 1.0);
        assert!(pred.interval_high >= pred.value);
        assert!(pred.interval_low <= pred.value);
        assert_eq!(pred.model_name, "MLP");
    }
}

#[tokio::test]
async fn test_ensemble_prediction() {
    let config = NeuralConfig {
        models: vec!["NHITS".to_string(), "TCN".to_string(), "MLP".to_string()],
        horizon: 10,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Create test data with trend
    let mut data = Vec::new();
    let base_time = Utc::now();
    for i in 0..100 {
        data.push(TimeSeriesData {
            timestamp: base_time - Duration::minutes(i as i64),
            value: 100.0 + (i as f64) * 0.1 + (i as f64).sin() * 5.0,
            volume: 1000.0 + (i as f64) * 10.0,
        });
    }
    
    let models = vec!["NHITS".to_string(), "TCN".to_string(), "MLP".to_string()];
    let predictions = predictor.predict_ensemble(&data, 10, &models, None).await;
    assert!(predictions.is_ok());
    let results = predictions.unwrap();
    assert_eq!(results.len(), 10);
    
    // Ensemble predictions should have model name "ensemble"
    for pred in &results {
        assert_eq!(pred.model_name, "ensemble");
        assert!(pred.confidence > 0.0);
    }
}

#[tokio::test]
async fn test_feature_importance() {
    let config = NeuralConfig {
        models: vec!["TCN".to_string()],
        horizon: 5,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let importance = predictor.get_feature_importance().await;
    
    assert!(importance.is_ok());
    let features = importance.unwrap();
    
    // Should have some feature importance values
    assert!(!features.is_empty());
    
    // All importance values should be between 0 and 1
    for (_, value) in &features {
        assert!(*value >= 0.0 && *value <= 1.0);
    }
}

#[tokio::test]
async fn test_prediction_with_features() {
    let config = NeuralConfig {
        models: vec!["DeepAR".to_string()],
        horizon: 7,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Create test data
    let mut data = Vec::new();
    let base_time = Utc::now();
    for i in 0..60 {
        data.push(TimeSeriesData {
            timestamp: base_time - Duration::minutes(i as i64),
            value: 100.0 + (i as f64).cos() * 10.0,
            volume: 1000.0,
        });
    }
    
    // Add custom features
    let mut features = HashMap::new();
    features.insert("volatility".to_string(), serde_json::Value::from(0.15));
    features.insert("trend_strength".to_string(), serde_json::Value::from(0.8));
    
    let predictions = predictor.predict(&data, 7, Some(features)).await;
    assert!(predictions.is_ok());
    let results = predictions.unwrap();
    
    // DeepAR should provide uncertainty intervals
    for pred in &results {
        assert!(pred.interval_high > pred.interval_low);
        assert_eq!(pred.model_name, "DeepAR");
    }
}

#[tokio::test]
async fn test_online_learning() {
    let config = NeuralConfig {
        models: vec!["MLP".to_string()],
        horizon: 3,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Create initial data
    let mut data = Vec::new();
    let base_time = Utc::now();
    for i in 0..30 {
        data.push(TimeSeriesData {
            timestamp: base_time - Duration::minutes(i as i64),
            value: 100.0 + (i as f64) * 0.5,
            volume: 1000.0,
        });
    }
    
    // Make initial prediction
    let pred1 = predictor.predict(&data, 3, None).await.unwrap();
    
    // Add more data
    for i in 30..40 {
        data.push(TimeSeriesData {
            timestamp: base_time - Duration::minutes(i as i64),
            value: 100.0 + (i as f64) * 0.5,
            volume: 1000.0,
        });
    }
    
    // Make another prediction (should trigger online learning)
    let pred2 = predictor.predict(&data, 3, None).await.unwrap();
    
    // Both predictions should be valid
    assert_eq!(pred1.len(), 3);
    assert_eq!(pred2.len(), 3);
}

#[tokio::test]
async fn test_transformer_model() {
    let config = NeuralConfig {
        models: vec!["Transformer".to_string()],
        horizon: 12,
        update_frequency: 300,
        confidence_threshold: 0.7,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Create complex pattern data
    let mut data = Vec::new();
    let base_time = Utc::now();
    for i in 0..200 {
        let seasonal = (i as f64 * 0.1).sin() * 20.0;
        let trend = i as f64 * 0.05;
        let noise = ((i * 7) % 13) as f64 - 6.5;
        data.push(TimeSeriesData {
            timestamp: base_time - Duration::minutes(i as i64),
            value: 100.0 + seasonal + trend + noise,
            volume: 1000.0 + (i as f64) * 5.0,
        });
    }
    
    let predictions = predictor.predict(&data, 12, None).await;
    assert!(predictions.is_ok());
    let results = predictions.unwrap();
    assert_eq!(results.len(), 12);
    
    // Transformer should capture complex patterns
    for pred in &results {
        assert_eq!(pred.model_name, "Transformer");
        assert!(pred.confidence > 0.0);
    }
}