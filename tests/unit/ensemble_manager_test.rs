//! Tests for Ensemble Manager and Model Diversity
//!
//! Focused tests for:
//! - Market regime detection accuracy
//! - Dynamic weight calculation
//! - Model performance tracking
//! - Diversity metrics calculation
//! - Adaptive model selection

use autonomous_platform::neural::fann_predictor::FannPredictor;
use autonomous_platform::neural::NeuralPredictorTrait;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use serde_json::json;

/// Helper to create market data with specific characteristics
fn create_market_data(
    regime: &str,
    size: usize,
    base_price: f64,
) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();
    let base_volume = 1_000_000.0;
    
    for i in 0..size {
        let (price_multiplier, volume_multiplier, rsi, momentum) = match regime {
            "strong_bullish" => (
                1.0 + 0.003 * i as f64,  // 0.3% growth per period
                1.0 + 0.002 * i as f64,  // Increasing volume
                65.0 + 0.5 * i as f64,   // Rising RSI
                1.2 + 0.01 * i as f64,   // Positive momentum
            ),
            "strong_bearish" => (
                1.0 - 0.003 * i as f64,  // 0.3% decline per period
                1.0 + 0.001 * i as f64,  // Slightly increasing volume
                35.0 - 0.5 * i as f64,   // Falling RSI
                0.8 - 0.01 * i as f64,   // Negative momentum
            ),
            "high_volatility" => {
                let volatility = 0.1 * (i as f64 * 0.5).sin();
                (
                    1.0 + volatility,
                    1.0 + 0.5 * (i as f64 * 0.3).cos().abs(),
                    50.0 + 30.0 * (i as f64 * 0.2).sin(),
                    1.0 + 0.5 * (i as f64 * 0.4).cos(),
                )
            },
            "low_volatility" => (
                1.0 + 0.0001 * (i as f64).sin(),  // Minimal movement
                1.0 + 0.01 * (i as f64 * 0.1).cos(),
                50.0 + 2.0 * (i as f64 * 0.1).sin(),
                1.0 + 0.01 * (i as f64 * 0.05).cos(),
            ),
            "sideways" => (
                1.0 + 0.001 * (i as f64 * 0.1).sin(),
                1.0,
                50.0,
                1.0,
            ),
            _ => (1.0, 1.0, 50.0, 1.0),
        };
        
        let price = base_price * price_multiplier;
        let volume = base_volume * volume_multiplier;
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), rsi.clamp(0.0, 100.0));
        indicators.insert("momentum".to_string(), momentum);
        indicators.insert("macd_signal".to_string(), (momentum - 1.0) * 100.0);
        indicators.insert("bb_width".to_string(), 
            if regime == "high_volatility" { 500.0 } else { 200.0 });
        
        let high_low_spread = match regime {
            "high_volatility" => 0.02,
            "low_volatility" => 0.002,
            _ => 0.005,
        };
        
        data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
            entity: "test_market".to_string(),
            symbol: "MARKET/USD".to_string(),
            open: price * (1.0 - high_low_spread / 2.0),
            high: price * (1.0 + high_low_spread),
            low: price * (1.0 - high_low_spread),
            close: price,
            volume,
            source: "test".to_string(),
            metadata: HashMap::from([("regime".to_string(), regime.to_string())]),
            indicators,
        });
    }
    
    data
}

/// Create predictions with known accuracy for testing
fn create_test_predictions(
    base_time: DateTime<Utc>,
    base_value: f64,
    count: usize,
    model_name: &str,
    accuracy_factor: f64,
) -> Vec<autonomous_platform::neural::PredictionResult> {
    (0..count).map(|i| {
        autonomous_platform::neural::PredictionResult {
            timestamp: base_time + chrono::Duration::minutes((i + 1) as i64 * 5),
            value: base_value * (1.0 + 0.001 * i as f64) * accuracy_factor,
            confidence: 0.8 - 0.05 * i as f64,
            interval_low: base_value * (0.98 - 0.01 * i as f64),
            interval_high: base_value * (1.02 + 0.01 * i as f64),
            model_name: model_name.to_string(),
        }
    }).collect()
}

#[cfg(test)]
mod market_regime_tests {
    use super::*;

    #[tokio::test]
    async fn test_accurate_regime_detection() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["LSTM".to_string(), "TCN".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Test each regime type
        let test_cases = vec![
            ("strong_bullish", "Bullish"),
            ("strong_bearish", "Bearish"),
            ("high_volatility", "HighVolatility"),
            ("low_volatility", "LowVolatility"),
            ("sideways", "Sideways"),
        ];
        
        for (data_regime, expected_detection) in test_cases {
            let data = create_market_data(data_regime, 50, 50000.0);
            
            // Trigger regime detection through ensemble prediction
            let models = vec!["LSTM".to_string(), "TCN".to_string()];
            let _ = predictor.predict_ensemble(&data, 5, &models, None).await.unwrap();
            
            // Check detected regime
            let stats = predictor.get_ensemble_stats().await.unwrap();
            let current_regime = stats.get("current_regime")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            assert!(
                current_regime.contains(expected_detection),
                "Expected {} regime for {}, got {}",
                expected_detection, data_regime, current_regime
            );
        }
    }

    #[tokio::test]
    async fn test_regime_transition_detection() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["LSTM".to_string(), "GRU".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data with regime transition
        let mut data = create_market_data("strong_bullish", 30, 50000.0);
        let transition_price = data.last().unwrap().close;
        data.extend(create_market_data("high_volatility", 30, transition_price));
        
        // First prediction - should detect bullish
        let models = vec!["LSTM".to_string(), "GRU".to_string()];
        let _ = predictor.predict_ensemble(&data[..30], 5, &models, None).await.unwrap();
        
        let stats1 = predictor.get_ensemble_stats().await.unwrap();
        let regime1 = stats1.get("current_regime").and_then(|v| v.as_str()).unwrap_or("");
        assert!(regime1.contains("Bullish"));
        
        // Second prediction - should detect high volatility
        let _ = predictor.predict_ensemble(&data, 5, &models, None).await.unwrap();
        
        let stats2 = predictor.get_ensemble_stats().await.unwrap();
        let regime2 = stats2.get("current_regime").and_then(|v| v.as_str()).unwrap_or("");
        assert!(regime2.contains("Volatility"));
    }
}

#[cfg(test)]
mod dynamic_weight_tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_based_weight_adjustment() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "LSTM".to_string(),
                "GRU".to_string(),
                "TCN".to_string(),
                "DeepAR".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Reset to start fresh
        predictor.reset_ensemble_performance().await.unwrap();
        
        // Create test data
        let data = create_market_data("strong_bullish", 100, 50000.0);
        
        // Simulate multiple prediction rounds with varying model performance
        for round in 0..15 {
            // Get predictions from each model
            for model in &["LSTM", "GRU", "TCN", "DeepAR"] {
                let predictions = predictor.predict_with_model(model, &data, 5).await.unwrap();
                
                // Simulate actual values with model-specific accuracy
                let accuracy_factor = match model.as_str() {
                    "LSTM" => 1.01 - 0.001 * round as f64,    // Starts good, degrades
                    "GRU" => 1.02,                            // Consistently accurate
                    "TCN" => 1.05 + 0.002 * round as f64,     // Starts poor, improves
                    "DeepAR" => 1.03,                         // Moderate accuracy
                    _ => 1.05,
                };
                
                let actual_values: Vec<f64> = predictions.iter()
                    .map(|p| p.value * accuracy_factor)
                    .collect();
                
                predictor.update_performance(model, &actual_values, &predictions).await.unwrap();
            }
            
            // Trigger ensemble prediction to update weights
            let models = vec![
                "LSTM".to_string(),
                "GRU".to_string(),
                "TCN".to_string(),
                "DeepAR".to_string(),
            ];
            let _ = predictor.predict_ensemble(&data, 5, &models, None).await.unwrap();
        }
        
        // Check final dynamic weights
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(weights) = stats.get("dynamic_weights").and_then(|v| v.as_object()) {
            let lstm_weight = weights.get("LSTM").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let gru_weight = weights.get("GRU").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let tcn_weight = weights.get("TCN").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let deepar_weight = weights.get("DeepAR").and_then(|v| v.as_f64()).unwrap_or(0.0);
            
            // GRU should have highest weight (best consistent performance)
            assert!(gru_weight > lstm_weight);
            assert!(gru_weight > deepar_weight);
            
            // TCN should have improved weight
            assert!(tcn_weight > 0.5);
            
            // LSTM should have degraded weight
            assert!(lstm_weight < gru_weight);
        }
    }

    #[tokio::test]
    async fn test_volatility_based_adjustments() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "LSTM".to_string(),
                "DeepAR".to_string(),
                "TCN".to_string(),
                "Transformer".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Test with high volatility data
        let high_vol_data = create_market_data("high_volatility", 100, 50000.0);
        let models = vec![
            "LSTM".to_string(),
            "DeepAR".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
        ];
        
        let _ = predictor.predict_ensemble(&high_vol_data, 5, &models, None).await.unwrap();
        
        let stats_high_vol = predictor.get_ensemble_stats().await.unwrap();
        let vol_adj_high = stats_high_vol.get("volatility_adjustments")
            .and_then(|v| v.as_object())
            .unwrap();
        
        // In high volatility, LSTM and DeepAR should get boost
        let lstm_adj_high = vol_adj_high.get("LSTM").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let deepar_adj_high = vol_adj_high.get("DeepAR").and_then(|v| v.as_f64()).unwrap_or(1.0);
        assert!(lstm_adj_high >= 1.0);
        assert!(deepar_adj_high >= 1.0);
        
        // Test with low volatility data
        predictor.reset_ensemble_performance().await.unwrap();
        let low_vol_data = create_market_data("low_volatility", 100, 50000.0);
        
        let _ = predictor.predict_ensemble(&low_vol_data, 5, &models, None).await.unwrap();
        
        let stats_low_vol = predictor.get_ensemble_stats().await.unwrap();
        let vol_adj_low = stats_low_vol.get("volatility_adjustments")
            .and_then(|v| v.as_object())
            .unwrap();
        
        // In low volatility, TCN and Transformer should get boost
        let tcn_adj_low = vol_adj_low.get("TCN").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let transformer_adj_low = vol_adj_low.get("Transformer").and_then(|v| v.as_f64()).unwrap_or(1.0);
        assert!(tcn_adj_low >= 1.0 || transformer_adj_low >= 1.0);
    }
}

#[cfg(test)]
mod diversity_metrics_tests {
    use super::*;

    #[tokio::test]
    async fn test_model_diversity_calculation() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "LSTM".to_string(),
                "GRU".to_string(),
                "TCN".to_string(),
                "Transformer".to_string(),
                "NHITS".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create complex pattern data
        let mut data = Vec::new();
        data.extend(create_market_data("strong_bullish", 50, 50000.0));
        data.extend(create_market_data("high_volatility", 50, 52500.0));
        data.extend(create_market_data("sideways", 50, 53000.0));
        
        // Get ensemble predictions to calculate diversity
        let models = vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
            "NHITS".to_string(),
        ];
        
        let _ = predictor.predict_ensemble(&data, 10, &models, None).await.unwrap();
        
        // Check diversity metrics
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(diversity) = stats.get("diversity_metrics").and_then(|v| v.as_object()) {
            // Should have diversity scores for each model
            assert_eq!(diversity.len(), 5);
            
            // Verify diversity values are in valid range
            for (model, score) in diversity {
                let diversity_score = score.as_f64().unwrap_or(0.0);
                assert!(
                    diversity_score >= 0.0 && diversity_score <= 1.0,
                    "Model {} has invalid diversity score: {}",
                    model, diversity_score
                );
            }
            
            // Models should have different diversity scores
            let scores: Vec<f64> = diversity.values()
                .filter_map(|v| v.as_f64())
                .collect();
            
            let mean_diversity = scores.iter().sum::<f64>() / scores.len() as f64;
            assert!(mean_diversity > 0.1, "Ensemble lacks diversity");
        }
    }

    #[tokio::test]
    async fn test_diversity_bonus_in_weights() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "LSTM".to_string(),
                "GRU".to_string(),
                "MLP".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        predictor.reset_ensemble_performance().await.unwrap();
        
        let data = create_market_data("high_volatility", 150, 50000.0);
        let models = vec!["LSTM".to_string(), "GRU".to_string(), "MLP".to_string()];
        
        // Perform multiple predictions to establish diversity
        for _ in 0..10 {
            let _ = predictor.predict_ensemble(&data, 5, &models, None).await.unwrap();
            
            // Update with simulated performance
            for model in &models {
                let predictions = predictor.predict_with_model(model, &data, 5).await.unwrap();
                let actual: Vec<f64> = predictions.iter()
                    .map(|p| p.value * 1.02) // All models reasonably accurate
                    .collect();
                predictor.update_performance(model, &actual, &predictions).await.unwrap();
            }
        }
        
        // Check that diversity affects weights
        let stats = predictor.get_ensemble_stats().await.unwrap();
        let diversity = stats.get("diversity_metrics")
            .and_then(|v| v.as_object())
            .unwrap();
        let weights = stats.get("dynamic_weights")
            .and_then(|v| v.as_object())
            .unwrap();
        
        // Find model with highest diversity
        let (most_diverse_model, _) = diversity.iter()
            .max_by(|a, b| {
                let a_score = a.1.as_f64().unwrap_or(0.0);
                let b_score = b.1.as_f64().unwrap_or(0.0);
                a_score.partial_cmp(&b_score).unwrap()
            })
            .unwrap();
        
        // Most diverse model should have good weight
        let diverse_weight = weights.get(most_diverse_model)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        
        assert!(diverse_weight > 0.8, "Diverse model should have good weight");
    }
}

#[cfg(test)]
mod adaptive_selection_tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_threshold_selection() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "LSTM".to_string(),
                "GRU".to_string(),
                "TCN".to_string(),
                "MLP".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        predictor.reset_ensemble_performance().await.unwrap();
        
        let data = create_market_data("strong_bullish", 150, 50000.0);
        
        // Establish different performance levels
        for _ in 0..10 {
            for model in &["LSTM", "GRU", "TCN", "MLP"] {
                let predictions = predictor.predict_with_model(model, &data, 5).await.unwrap();
                
                let accuracy_factor = match model {
                    &"LSTM" => 1.01,  // Good performance
                    &"GRU" => 1.02,   // Good performance
                    &"TCN" => 1.15,   // Poor performance
                    &"MLP" => 1.20,   // Very poor performance
                    _ => 1.05,
                };
                
                let actual: Vec<f64> = predictions.iter()
                    .map(|p| p.value * accuracy_factor)
                    .collect();
                
                predictor.update_performance(model, &actual, &predictions).await.unwrap();
            }
        }
        
        // Check model performances
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
            // Verify LSTM and GRU have good performance
            if let Some(lstm_perf) = performances.get("LSTM").and_then(|v| v.as_object()) {
                let accuracy = lstm_perf.get("recent_accuracy")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                assert!(accuracy > 0.6, "LSTM should have good accuracy");
            }
            
            // Verify TCN and MLP have poor performance
            if let Some(tcn_perf) = performances.get("TCN").and_then(|v| v.as_object()) {
                let accuracy = tcn_perf.get("recent_accuracy")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                assert!(accuracy < 0.6, "TCN should have poor accuracy");
            }
        }
    }

    #[tokio::test]
    async fn test_regime_specific_model_selection() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "LSTM".to_string(),
                "DeepAR".to_string(),
                "TCN".to_string(),
                "Transformer".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.5,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        predictor.reset_ensemble_performance().await.unwrap();
        
        // Test different regimes
        let regimes = vec![
            ("strong_bullish", vec!["LSTM", "DeepAR"]),  // These should perform well
            ("high_volatility", vec!["LSTM", "DeepAR"]), // These handle volatility well
            ("sideways", vec!["TCN", "Transformer"]),     // These handle stable markets
        ];
        
        for (regime, expected_good_models) in regimes {
            let data = create_market_data(regime, 100, 50000.0);
            
            // Train models in specific regime
            for _ in 0..5 {
                let all_models = vec![
                    "LSTM".to_string(),
                    "DeepAR".to_string(),
                    "TCN".to_string(),
                    "Transformer".to_string(),
                ];
                
                let _ = predictor.predict_ensemble(&data, 5, &all_models, None).await.unwrap();
                
                // Update performance based on regime
                for model in &all_models {
                    let predictions = predictor.predict_with_model(model, &data, 5).await.unwrap();
                    
                    let accuracy_factor = if expected_good_models.contains(&model.as_str()) {
                        1.01 // Good performance
                    } else {
                        1.08 // Poorer performance
                    };
                    
                    let actual: Vec<f64> = predictions.iter()
                        .map(|p| p.value * accuracy_factor)
                        .collect();
                    
                    predictor.update_performance(model, &actual, &predictions).await.unwrap();
                }
            }
            
            // Check regime-specific performance
            let stats = predictor.get_ensemble_stats().await.unwrap();
            if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
                for (model, perf_data) in performances {
                    if let Some(perf_obj) = perf_data.as_object() {
                        if let Some(regime_perf) = perf_obj.get("regime_performance").and_then(|v| v.as_object()) {
                            // Models should have regime-specific performance recorded
                            assert!(!regime_perf.is_empty(), 
                                "Model {} should have regime performance data", model);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod performance_tracking_tests {
    use super::*;

    #[tokio::test]
    async fn test_time_weighted_accuracy() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        predictor.reset_ensemble_performance().await.unwrap();
        
        let data = create_market_data("strong_bullish", 100, 50000.0);
        
        // First phase: poor performance
        for _ in 0..5 {
            let predictions = predictor.predict_with_model("LSTM", &data, 5).await.unwrap();
            let actual: Vec<f64> = predictions.iter()
                .map(|p| p.value * 1.15) // 15% error
                .collect();
            predictor.update_performance("LSTM", &actual, &predictions).await.unwrap();
        }
        
        // Second phase: improving performance
        for _ in 0..10 {
            let predictions = predictor.predict_with_model("LSTM", &data, 5).await.unwrap();
            let actual: Vec<f64> = predictions.iter()
                .map(|p| p.value * 1.02) // 2% error
                .collect();
            predictor.update_performance("LSTM", &actual, &predictions).await.unwrap();
        }
        
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
            if let Some(lstm_perf) = performances.get("LSTM").and_then(|v| v.as_object()) {
                let recent_acc = lstm_perf.get("recent_accuracy")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let time_weighted_acc = lstm_perf.get("time_weighted_accuracy")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                
                // Recent accuracy should be better than time-weighted
                // because recent performance is better
                assert!(recent_acc > time_weighted_acc);
                assert!(recent_acc > 0.7); // Should reflect recent good performance
            }
        }
    }

    #[tokio::test]
    async fn test_confidence_calibration() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["DeepAR".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        predictor.reset_ensemble_performance().await.unwrap();
        
        let data = create_market_data("low_volatility", 100, 50000.0);
        
        // Test confidence calibration
        for _ in 0..10 {
            let predictions = predictor.predict_with_model("DeepAR", &data, 5).await.unwrap();
            
            // DeepAR predictions with their confidence
            for (i, pred) in predictions.iter().enumerate() {
                let actual = if pred.confidence > 0.8 {
                    pred.value * 1.01 // High confidence, accurate
                } else {
                    pred.value * 1.05 // Lower confidence, less accurate
                };
                
                predictor.update_performance("DeepAR", &[actual], &[pred.clone()]).await.unwrap();
            }
        }
        
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
            if let Some(deepar_perf) = performances.get("DeepAR").and_then(|v| v.as_object()) {
                let conf_score = deepar_perf.get("confidence_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                
                // Confidence should be reasonably calibrated
                assert!(conf_score > 0.6, "DeepAR should have calibrated confidence");
            }
        }
    }

    #[tokio::test]
    async fn test_stability_score_calculation() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["TCN".to_string(), "GRU".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.6,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        predictor.reset_ensemble_performance().await.unwrap();
        
        let data = create_market_data("sideways", 100, 50000.0);
        
        // TCN: stable predictions
        for i in 0..10 {
            let predictions = predictor.predict_with_model("TCN", &data, 5).await.unwrap();
            let actual: Vec<f64> = predictions.iter()
                .map(|p| p.value * (1.01 + 0.001 * (i % 2) as f64)) // Small, consistent error
                .collect();
            predictor.update_performance("TCN", &actual, &predictions).await.unwrap();
        }
        
        // GRU: unstable predictions
        for i in 0..10 {
            let predictions = predictor.predict_with_model("GRU", &data, 5).await.unwrap();
            let actual: Vec<f64> = predictions.iter()
                .map(|p| p.value * (1.0 + 0.1 * (i % 3) as f64)) // Large, varying error
                .collect();
            predictor.update_performance("GRU", &actual, &predictions).await.unwrap();
        }
        
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
            let tcn_stability = performances.get("TCN")
                .and_then(|v| v.as_object())
                .and_then(|v| v.get("stability_score"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            
            let gru_stability = performances.get("GRU")
                .and_then(|v| v.as_object())
                .and_then(|v| v.get("stability_score"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            
            // TCN should have higher stability
            assert!(tcn_stability > gru_stability);
            assert!(tcn_stability > 0.8, "TCN should be stable");
            assert!(gru_stability < 0.7, "GRU should be unstable");
        }
    }
}