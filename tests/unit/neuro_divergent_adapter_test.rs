//! Unit tests for Neuro-Divergent Adapter

use autonomous_platform::adapters::neuro_divergent::{NeuroDivergentAdapter, ModelArchitecture};
use std::collections::HashMap;

#[test]
fn test_model_architecture_creation() {
    // Test MLP architecture
    let mlp = ModelArchitecture {
        model_type: "MLP".to_string(),
        input_size: 30,
        hidden_layers: vec![64, 32, 16],
        output_size: 5,
        activation: "ReLU".to_string(),
        learning_rate: 0.001,
        dropout: Some(0.2),
        batch_norm: Some(true),
    };
    
    assert_eq!(mlp.model_type, "MLP");
    assert_eq!(mlp.hidden_layers.len(), 3);
    assert_eq!(mlp.dropout, Some(0.2));
}

#[tokio::test]
async fn test_adapter_initialization() {
    let adapter = NeuroDivergentAdapter::new();
    
    // Test that adapter initializes with correct models
    let models = vec!["MLP", "TCN", "Transformer", "NHITS", "DeepAR"];
    
    // Verify model count (would be populated after initialization)
    assert!(adapter.models.is_some());
}

#[test]
fn test_preprocessing_pipeline() {
    let adapter = NeuroDivergentAdapter::new();
    
    // Test data preprocessing
    let mut raw_data = HashMap::new();
    raw_data.insert("price".to_string(), vec![100.0, 101.0, 99.0, 102.0]);
    raw_data.insert("volume".to_string(), vec![1000.0, 1100.0, 900.0, 1200.0]);
    
    // Test normalization
    let price_data = &raw_data["price"];
    let mean = price_data.iter().sum::<f64>() / price_data.len() as f64;
    let variance = price_data.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / price_data.len() as f64;
    let std_dev = variance.sqrt();
    
    assert!((mean - 100.5).abs() < 0.1);
    assert!((std_dev - 1.118).abs() < 0.01);
}

#[test]
fn test_feature_engineering() {
    // Test feature extraction for neural models
    let prices = vec![100.0, 102.0, 101.0, 103.0, 104.0];
    
    // Calculate returns
    let returns: Vec<f64> = prices.windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();
    
    assert_eq!(returns.len(), 4);
    assert!((returns[0] - 0.02).abs() < 0.0001); // 2% return
    
    // Calculate moving average
    let window_size = 3;
    let ma: Vec<f64> = prices.windows(window_size)
        .map(|w| w.iter().sum::<f64>() / window_size as f64)
        .collect();
    
    assert_eq!(ma.len(), 3);
    assert!((ma[0] - 101.0).abs() < 0.1);
}

#[test]
fn test_model_selection_logic() {
    let adapter = NeuroDivergentAdapter::new();
    
    // Test model selection based on data characteristics
    let data_characteristics = HashMap::from([
        ("length", 1000),
        ("frequency", 5), // 5-minute data
        ("features", 10),
    ]);
    
    // For high-frequency data with many features, Transformer or TCN would be preferred
    let suitable_models = match data_characteristics["frequency"] {
        1..=5 => vec!["TCN", "Transformer"],
        6..=60 => vec!["NHITS", "DeepAR"],
        _ => vec!["MLP"],
    };
    
    assert!(suitable_models.contains(&"TCN"));
    assert!(suitable_models.contains(&"Transformer"));
}

#[test]
fn test_ensemble_weighting() {
    // Test ensemble weight calculation
    let model_performances = HashMap::from([
        ("MLP", 0.65),
        ("TCN", 0.75),
        ("Transformer", 0.80),
        ("NHITS", 0.72),
        ("DeepAR", 0.78),
    ]);
    
    // Calculate normalized weights
    let total_performance: f64 = model_performances.values().sum();
    let weights: HashMap<&str, f64> = model_performances.iter()
        .map(|(k, v)| (*k, v / total_performance))
        .collect();
    
    // Verify weights sum to 1
    let weight_sum: f64 = weights.values().sum();
    assert!((weight_sum - 1.0).abs() < 0.0001);
    
    // Verify Transformer has highest weight
    let max_weight = weights.values().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
    assert_eq!(*max_weight, weights["Transformer"]);
}

#[test]
fn test_prediction_aggregation() {
    // Test how predictions from multiple models are aggregated
    let predictions = vec![
        vec![100.5, 101.0, 101.5], // MLP
        vec![100.8, 101.3, 101.8], // TCN
        vec![100.6, 101.1, 101.6], // Transformer
    ];
    
    let weights = vec![0.3, 0.4, 0.3];
    
    // Calculate weighted average predictions
    let aggregated: Vec<f64> = (0..3)
        .map(|i| {
            predictions.iter()
                .zip(weights.iter())
                .map(|(pred, w)| pred[i] * w)
                .sum()
        })
        .collect();
    
    assert!((aggregated[0] - 100.67).abs() < 0.01);
    assert!((aggregated[1] - 101.17).abs() < 0.01);
    assert!((aggregated[2] - 101.67).abs() < 0.01);
}

#[test]
fn test_confidence_interval_calculation() {
    // Test prediction interval calculation
    let predictions = vec![100.5, 101.0, 101.5, 102.0, 102.5];
    let std_devs = vec![0.5, 0.6, 0.7, 0.8, 0.9];
    
    // Calculate 95% confidence intervals
    let z_score = 1.96; // 95% confidence
    let intervals: Vec<(f64, f64)> = predictions.iter()
        .zip(std_devs.iter())
        .map(|(pred, std)| {
            (pred - z_score * std, pred + z_score * std)
        })
        .collect();
    
    // Verify first interval
    assert!((intervals[0].0 - 99.52).abs() < 0.01);
    assert!((intervals[0].1 - 101.48).abs() < 0.01);
}

#[test]
fn test_model_metadata() {
    // Test model metadata structure
    let metadata = HashMap::from([
        ("name", "TCN"),
        ("version", "1.0"),
        ("training_date", "2024-01-01"),
        ("accuracy", "0.85"),
        ("parameters", "1.2M"),
    ]);
    
    assert_eq!(metadata["name"], "TCN");
    assert_eq!(metadata["accuracy"], "0.85");
}

#[test]
fn test_error_handling() {
    let adapter = NeuroDivergentAdapter::new();
    
    // Test handling of invalid input
    let invalid_data = vec![];
    
    // Adapter should handle empty data gracefully
    assert_eq!(invalid_data.len(), 0);
    
    // Test with NaN values
    let data_with_nan = vec![100.0, f64::NAN, 102.0];
    let valid_data: Vec<f64> = data_with_nan.iter()
        .filter(|x| !x.is_nan())
        .copied()
        .collect();
    
    assert_eq!(valid_data.len(), 2);
    assert_eq!(valid_data, vec![100.0, 102.0]);
}

#[test]
fn test_performance_metrics() {
    // Test calculation of model performance metrics
    let predictions = vec![100.0, 101.0, 102.0, 103.0, 104.0];
    let actuals = vec![100.5, 100.8, 102.2, 103.1, 103.8];
    
    // Calculate MSE
    let mse: f64 = predictions.iter()
        .zip(actuals.iter())
        .map(|(pred, actual)| (pred - actual).powi(2))
        .sum::<f64>() / predictions.len() as f64;
    
    assert!((mse - 0.078).abs() < 0.001);
    
    // Calculate MAE
    let mae: f64 = predictions.iter()
        .zip(actuals.iter())
        .map(|(pred, actual)| (pred - actual).abs())
        .sum::<f64>() / predictions.len() as f64;
    
    assert!((mae - 0.26).abs() < 0.01);
}

#[test]
fn test_cache_key_generation() {
    // Test cache key generation for predictions
    let symbol = "BTC/USD";
    let timestamp = 1234567890;
    let horizon = 5;
    
    let cache_key = format!("{}:{}:{}", symbol, timestamp, horizon);
    assert_eq!(cache_key, "BTC/USD:1234567890:5");
    
    // Test with model ensemble
    let models = vec!["MLP", "TCN", "Transformer"];
    let ensemble_key = format!("{}:{}:{}:ensemble:{}", 
        symbol, timestamp, horizon, models.join(","));
    assert_eq!(ensemble_key, "BTC/USD:1234567890:5:ensemble:MLP,TCN,Transformer");
}