//! Comprehensive tests for neural model enhancements
//!
//! Tests focus on:
//! 1. LSTM/GRU recurrent state management
//! 2. Attention mechanism implementation for Transformer
//! 3. Ensemble optimization with market regime detection
//! 4. Dynamic weight adjustment system
//! 5. Model diversity metrics
//! 6. Mock FANN network interactions

use autonomous_platform::neural::fann_predictor::{FannPredictor, FannModelConfig};
use autonomous_platform::neural::{NeuralPredictorTrait, PredictionResult};
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use futures::future::join_all;

/// Mock FANN network for testing
mod mock_fann {
    use ::ruv_fann::{Network, ActivationFunction};
    
    pub struct MockNetwork {
        input_size: usize,
        output_size: usize,
        activation: ActivationFunction,
    }
    
    impl MockNetwork {
        pub fn new(input_size: usize, output_size: usize, activation: ActivationFunction) -> Self {
            Self { input_size, output_size, activation }
        }
        
        pub fn run(&self, inputs: &[f32]) -> Vec<f32> {
            // Simple mock prediction: transform inputs based on activation
            let mut outputs = vec![0.0f32; self.output_size];
            for i in 0..self.output_size {
                let base = inputs.iter().take(3).sum::<f32>() / 3.0;
                outputs[i] = match self.activation {
                    ActivationFunction::SigmoidSymmetric => base.tanh(),
                    ActivationFunction::ReLU => base.max(0.0),
                    ActivationFunction::Tanh => base.tanh(),
                    ActivationFunction::Linear => base,
                    ActivationFunction::Gaussian => (-base * base).exp(),
                    _ => base,
                } * (1.0 - 0.1 * i as f32); // Decay for multi-step
            }
            outputs
        }
    }
}

/// Helper function to create test time series data with specific patterns
fn create_pattern_data(pattern: &str, size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_price = 50000.0;
    let base_volume = 1000000.0;
    let base_time = Utc::now();
    
    for i in 0..size {
        let (price_change, volume_change, rsi) = match pattern {
            "bullish" => (
                0.02 * (1.0 + 0.001 * i as f64), // Steady upward trend
                1.2 + 0.01 * i as f64,           // Increasing volume
                60.0 + 0.5 * i as f64,           // Rising RSI
            ),
            "bearish" => (
                -0.02 * (1.0 + 0.001 * i as f64), // Steady downward trend
                0.8 - 0.01 * i as f64,             // Decreasing volume
                40.0 - 0.5 * i as f64,             // Falling RSI
            ),
            "volatile" => (
                0.1 * (i as f64 * 0.5).sin(),     // High volatility
                1.0 + 0.5 * (i as f64 * 0.3).cos(), // Variable volume
                50.0 + 30.0 * (i as f64 * 0.2).sin(), // Oscillating RSI
            ),
            "sideways" => (
                0.001 * (i as f64 * 0.1).sin(),   // Minimal movement
                1.0 + 0.05 * (i as f64 * 0.1).cos(), // Stable volume
                50.0 + 5.0 * (i as f64 * 0.1).sin(), // Stable RSI
            ),
            _ => (0.0, 1.0, 50.0),
        };
        
        let price = base_price * (1.0 + price_change);
        let volume = base_volume * volume_change;
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), rsi.clamp(0.0, 100.0));
        indicators.insert("macd".to_string(), price_change * 100.0);
        indicators.insert("bb_upper".to_string(), price * 1.02);
        indicators.insert("bb_lower".to_string(), price * 0.98);
        indicators.insert("volume_ratio".to_string(), volume_change);
        
        let mut metadata = HashMap::new();
        metadata.insert("pattern".to_string(), pattern.to_string());
        
        data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
            entity: "test_asset".to_string(),
            symbol: "TEST/USD".to_string(),
            open: price * 0.999,
            high: price * 1.001,
            low: price * 0.998,
            close: price,
            volume,
            source: "test".to_string(),
            metadata,
            indicators,
        });
    }
    
    data
}

/// Test configuration for neural models
fn create_test_neural_config(models: Vec<String>) -> NeuralConfig {
    NeuralConfig {
        memory_gb: 2.0,
        models,
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.7,
    }
}

#[cfg(test)]
mod lstm_gru_tests {
    use super::*;

    #[tokio::test]
    async fn test_lstm_recurrent_state_initialization() {
        let config = create_test_neural_config(vec!["LSTM".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create sequential data that LSTM should learn patterns from
        let data = create_pattern_data("bullish", 200);
        
        // First prediction to initialize state
        let predictions1 = predictor.predict(&data[..100], 5, None).await.unwrap();
        assert_eq!(predictions1.len(), 5);
        
        // Second prediction with more data should leverage state
        let predictions2 = predictor.predict(&data[..150], 5, None).await.unwrap();
        
        // LSTM with state should produce different predictions
        for i in 0..5 {
            assert_ne!(predictions1[i].value, predictions2[i].value);
        }
    }

    #[tokio::test]
    async fn test_gru_state_management() {
        let config = create_test_neural_config(vec!["GRU".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data with clear pattern changes
        let mut data = create_pattern_data("bullish", 100);
        data.extend(create_pattern_data("bearish", 100));
        
        // GRU should adapt to pattern change
        let predictions = predictor.predict(&data, 10, None).await.unwrap();
        
        // Verify predictions show adaptation
        assert_eq!(predictions.len(), 10);
        
        // Later predictions should reflect bearish trend
        let first_half_avg = predictions[..5].iter().map(|p| p.value).sum::<f64>() / 5.0;
        let second_half_avg = predictions[5..].iter().map(|p| p.value).sum::<f64>() / 5.0;
        
        // GRU should predict declining values
        assert!(second_half_avg < first_half_avg);
    }

    #[tokio::test]
    async fn test_recurrent_context_window() {
        let config = create_test_neural_config(vec!["LSTM".to_string(), "GRU".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create cyclical pattern data
        let data = create_pattern_data("volatile", 300);
        
        // Test ensemble with recurrent models
        let models = vec!["LSTM".to_string(), "GRU".to_string()];
        let predictions = predictor.predict_ensemble(&data, 20, &models, None).await.unwrap();
        
        // Verify context window influences predictions
        assert_eq!(predictions.len(), 20);
        
        // Check for cyclical pattern recognition
        let cycle_length = 10;
        for i in 0..10 {
            let diff = (predictions[i].value - predictions[i + cycle_length].value).abs();
            let avg = (predictions[i].value + predictions[i + cycle_length].value) / 2.0;
            let relative_diff = diff / avg;
            
            // Should recognize similar points in cycle
            assert!(relative_diff < 0.2, "Cycle recognition failed at position {}", i);
        }
    }

    #[tokio::test]
    async fn test_lstm_cell_state_persistence() {
        let config = create_test_neural_config(vec!["LSTM".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data with sudden spike
        let mut data = create_pattern_data("sideways", 100);
        // Insert anomaly
        data[80].close *= 1.5;
        data[81].close *= 1.4;
        
        // LSTM should remember the spike
        let predictions = predictor.predict(&data, 10, None).await.unwrap();
        
        // Predictions should show elevated values due to spike memory
        let baseline = data[..50].iter().map(|d| d.close).sum::<f64>() / 50.0;
        let predicted_avg = predictions.iter().map(|p| p.value).sum::<f64>() / predictions.len() as f64;
        
        // Should predict higher than baseline due to spike
        assert!(predicted_avg > baseline * 1.05);
    }
}

#[cfg(test)]
mod attention_mechanism_tests {
    use super::*;

    #[tokio::test]
    async fn test_transformer_attention_mechanism() {
        let config = create_test_neural_config(vec!["Transformer".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data with important events at different positions
        let mut data = create_pattern_data("sideways", 150);
        
        // Insert significant events
        data[30].close *= 1.2;  // Early event
        data[30].volume *= 3.0;
        data[100].close *= 0.8; // Late event
        data[100].volume *= 3.0;
        
        let predictions = predictor.predict(&data, 5, None).await.unwrap();
        
        // Transformer should attend to both events
        assert_eq!(predictions.len(), 5);
        
        // Predictions should reflect both events' influence
        for pred in &predictions {
            assert!(pred.confidence > 0.7); // High confidence from attention
        }
    }

    #[tokio::test]
    async fn test_multi_head_attention_diversity() {
        let config = create_test_neural_config(vec!["Transformer".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create complex pattern data
        let mut data = Vec::new();
        data.extend(create_pattern_data("bullish", 50));
        data.extend(create_pattern_data("volatile", 50));
        data.extend(create_pattern_data("bearish", 50));
        
        // Multiple predictions to test attention consistency
        let mut all_predictions = Vec::new();
        for _ in 0..3 {
            let preds = predictor.predict(&data, 5, None).await.unwrap();
            all_predictions.push(preds);
        }
        
        // Verify attention produces consistent but not identical results
        for i in 0..5 {
            let values: Vec<f64> = all_predictions.iter().map(|p| p[i].value).collect();
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
            
            // Should have some variance but not too much
            assert!(variance > 0.0, "Attention too deterministic");
            assert!(variance / mean.powi(2) < 0.01, "Attention too variable");
        }
    }

    #[tokio::test]
    async fn test_positional_encoding_influence() {
        let config = create_test_neural_config(vec!["Transformer".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data where position matters
        let data = create_pattern_data("volatile", 200);
        
        // Test predictions with different data lengths
        let predictions_short = predictor.predict(&data[..100], 5, None).await.unwrap();
        let predictions_long = predictor.predict(&data[..180], 5, None).await.unwrap();
        
        // Positional encoding should cause different predictions
        for i in 0..5 {
            assert_ne!(predictions_short[i].value, predictions_long[i].value);
            
            // Longer context should have higher confidence
            assert!(predictions_long[i].confidence >= predictions_short[i].confidence);
        }
    }
}

#[cfg(test)]
mod ensemble_optimization_tests {
    use super::*;

    #[tokio::test]
    async fn test_market_regime_detection() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Test different market regimes
        let regimes = vec!["bullish", "bearish", "volatile", "sideways"];
        
        for regime in regimes {
            let data = create_pattern_data(regime, 100);
            
            // Get ensemble predictions
            let models = vec!["LSTM".to_string(), "GRU".to_string(), "TCN".to_string()];
            let predictions = predictor.predict_ensemble(&data, 5, &models, None).await.unwrap();
            
            // Get ensemble stats to verify regime detection
            let stats = predictor.get_ensemble_stats().await.unwrap();
            
            // Verify regime is detected
            if let Some(current_regime) = stats.get("current_regime") {
                let regime_str = current_regime.as_str().unwrap();
                
                match regime {
                    "bullish" => assert!(regime_str.contains("Bullish")),
                    "bearish" => assert!(regime_str.contains("Bearish")),
                    "volatile" => assert!(regime_str.contains("Volatility")),
                    "sideways" => assert!(regime_str.contains("Sideways")),
                    _ => {}
                }
            }
            
            assert_eq!(predictions.len(), 5);
        }
    }

    #[tokio::test]
    async fn test_dynamic_weight_adjustment() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "DeepAR".to_string(),
            "TCN".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data with known pattern
        let data = create_pattern_data("bullish", 200);
        
        // Initial predictions
        let models = vec!["LSTM".to_string(), "DeepAR".to_string(), "TCN".to_string()];
        let predictions1 = predictor.predict_ensemble(&data[..100], 5, &models, None).await.unwrap();
        
        // Update performance with actual values (simulate good LSTM performance)
        let actual_values: Vec<f64> = predictions1.iter()
            .map(|p| p.value * 1.01) // LSTM predictions were close
            .collect();
        
        predictor.update_performance("LSTM", &actual_values, &predictions1).await.unwrap();
        
        // Update with poor TCN performance
        let tcn_predictions = predictor.predict_with_model("TCN", &data[..100], 5).await.unwrap();
        let actual_values_tcn: Vec<f64> = tcn_predictions.iter()
            .map(|p| p.value * 1.2) // TCN was way off
            .collect();
        
        predictor.update_performance("TCN", &actual_values_tcn, &tcn_predictions).await.unwrap();
        
        // Get updated weights
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(weights) = stats.get("dynamic_weights").and_then(|v| v.as_object()) {
            let lstm_weight = weights.get("LSTM").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let tcn_weight = weights.get("TCN").and_then(|v| v.as_f64()).unwrap_or(1.0);
            
            // LSTM should have higher weight due to better performance
            assert!(lstm_weight > tcn_weight);
        }
        
        // New predictions should favor LSTM
        let predictions2 = predictor.predict_ensemble(&data[100..], 5, &models, None).await.unwrap();
        assert_eq!(predictions2.len(), 5);
    }

    #[tokio::test]
    async fn test_ensemble_diversity_metrics() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
            "NHITS".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create challenging data
        let data = create_pattern_data("volatile", 200);
        
        // Get predictions from all models
        let models = vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
            "NHITS".to_string(),
        ];
        
        let predictions = predictor.predict_ensemble(&data, 10, &models, None).await.unwrap();
        
        // Get diversity metrics
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(diversity) = stats.get("diversity_metrics").and_then(|v| v.as_object()) {
            // Should have diversity scores for each model
            assert!(diversity.len() >= 3);
            
            // Verify diversity values are reasonable
            for (model, score) in diversity {
                let diversity_score = score.as_f64().unwrap_or(0.0);
                assert!(diversity_score >= 0.0 && diversity_score <= 1.0,
                    "Invalid diversity score for {}: {}", model, diversity_score);
            }
        }
        
        // Ensemble should benefit from diversity
        assert_eq!(predictions.len(), 10);
        for pred in &predictions {
            assert!(pred.confidence > 0.6); // Diverse ensemble = higher confidence
        }
    }

    #[tokio::test]
    async fn test_adaptive_model_selection() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "DeepAR".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create data and get initial predictions
        let data = create_pattern_data("bullish", 150);
        let all_models = vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "DeepAR".to_string(),
        ];
        
        // Simulate poor performance for some models
        for _ in 0..5 {
            let predictions = predictor.predict_ensemble(&data, 5, &all_models, None).await.unwrap();
            
            // Update with actual values showing TCN and GRU perform poorly
            let actual: Vec<f64> = predictions.iter().map(|p| p.value * 1.05).collect();
            
            // Good performance for LSTM and DeepAR
            predictor.update_performance("LSTM", &actual, &predictions).await.unwrap();
            predictor.update_performance("DeepAR", &actual, &predictions).await.unwrap();
            
            // Poor performance for TCN and GRU
            let poor_actual: Vec<f64> = predictions.iter().map(|p| p.value * 1.3).collect();
            predictor.update_performance("TCN", &poor_actual, &predictions).await.unwrap();
            predictor.update_performance("GRU", &poor_actual, &predictions).await.unwrap();
        }
        
        // Get stats to verify adaptive selection
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
            // Verify performance tracking
            for (model, perf) in performances {
                let perf_obj = perf.as_object().unwrap();
                let accuracy = perf_obj.get("recent_accuracy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                
                match model.as_str() {
                    "LSTM" | "DeepAR" => assert!(accuracy > 0.7),
                    "TCN" | "GRU" => assert!(accuracy < 0.5),
                    _ => {}
                }
            }
        }
    }

    #[tokio::test]
    async fn test_volatility_based_adjustments() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "DeepAR".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Test with different volatility patterns
        let low_vol_data = create_pattern_data("sideways", 150);
        let high_vol_data = create_pattern_data("volatile", 150);
        
        let models = vec![
            "LSTM".to_string(),
            "DeepAR".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
        ];
        
        // Low volatility predictions
        let low_vol_preds = predictor.predict_ensemble(&low_vol_data, 5, &models, None).await.unwrap();
        
        // High volatility predictions
        let high_vol_preds = predictor.predict_ensemble(&high_vol_data, 5, &models, None).await.unwrap();
        
        // Get stats to check volatility adjustments
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(vol_adj) = stats.get("volatility_adjustments").and_then(|v| v.as_object()) {
            // LSTM and DeepAR should have higher weights in high volatility
            let lstm_adj = vol_adj.get("LSTM").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let deepar_adj = vol_adj.get("DeepAR").and_then(|v| v.as_f64()).unwrap_or(1.0);
            
            // TCN and Transformer should have higher weights in low volatility
            let tcn_adj = vol_adj.get("TCN").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let transformer_adj = vol_adj.get("Transformer").and_then(|v| v.as_f64()).unwrap_or(1.0);
            
            // Verify adjustments make sense
            assert!(lstm_adj >= 1.0 || deepar_adj >= 1.0);
            assert!(tcn_adj <= 1.0 || transformer_adj <= 1.0);
        }
        
        // High volatility should have wider prediction intervals
        for i in 0..5 {
            let low_vol_range = low_vol_preds[i].interval_high - low_vol_preds[i].interval_low;
            let high_vol_range = high_vol_preds[i].interval_high - high_vol_preds[i].interval_low;
            
            assert!(high_vol_range > low_vol_range * 1.5);
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_concurrent_model_predictions() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
        ]);
        let predictor = Arc::new(FannPredictor::new(config).unwrap());
        
        let data = Arc::new(create_pattern_data("volatile", 200));
        
        // Spawn concurrent prediction tasks
        let mut handles = Vec::new();
        for model in &["LSTM", "GRU", "TCN", "Transformer"] {
            let predictor_clone = predictor.clone();
            let data_clone = data.clone();
            let model_name = model.to_string();
            
            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let result = predictor_clone.predict_with_model(&model_name, &data_clone, 10).await;
                let duration = start.elapsed();
                (model_name, result, duration)
            });
            
            handles.push(handle);
        }
        
        // Wait for all predictions
        let results = join_all(handles).await;
        
        // Verify all succeeded and were concurrent
        let mut max_duration = std::time::Duration::from_secs(0);
        for result in results {
            let (model, prediction_result, duration) = result.unwrap();
            assert!(prediction_result.is_ok(), "Model {} failed", model);
            assert_eq!(prediction_result.unwrap().len(), 10);
            
            if duration > max_duration {
                max_duration = duration;
            }
        }
        
        // Concurrent execution should be faster than sequential
        assert!(max_duration.as_secs() < 2, "Concurrent predictions took too long");
    }

    #[tokio::test]
    async fn test_prediction_cache_effectiveness() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["LSTM".to_string()],
            prediction_cache_ttl: 10, // 10 second cache
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        let data = create_pattern_data("bullish", 150);
        
        // First prediction (cache miss)
        let start = Instant::now();
        let predictions1 = predictor.predict(&data, 5, None).await.unwrap();
        let first_duration = start.elapsed();
        
        // Second prediction (cache hit)
        let start = Instant::now();
        let predictions2 = predictor.predict(&data, 5, None).await.unwrap();
        let cached_duration = start.elapsed();
        
        // Verify cache hit is much faster
        assert!(cached_duration < first_duration / 5);
        
        // Results should be identical
        for i in 0..5 {
            assert_eq!(predictions1[i].value, predictions2[i].value);
            assert_eq!(predictions1[i].confidence, predictions2[i].confidence);
        }
        
        // Wait for cache expiry
        tokio::time::sleep(tokio::time::Duration::from_secs(11)).await;
        
        // Third prediction (cache miss again)
        let start = Instant::now();
        let predictions3 = predictor.predict(&data, 5, None).await.unwrap();
        let expired_duration = start.elapsed();
        
        // Should be slow again after cache expiry
        assert!(expired_duration > cached_duration * 5);
    }

    #[tokio::test]
    async fn test_ensemble_performance_tracking() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Reset performance tracking
        predictor.reset_ensemble_performance().await.unwrap();
        
        // Generate predictions and update performance multiple times
        let data = create_pattern_data("bullish", 200);
        let models = vec!["LSTM".to_string(), "GRU".to_string(), "TCN".to_string()];
        
        for round in 0..10 {
            let predictions = predictor.predict_ensemble(&data, 5, &models, None).await.unwrap();
            
            // Simulate actual values with varying accuracy per model
            for model in &models {
                let model_predictions = predictor.predict_with_model(model, &data, 5).await.unwrap();
                
                let accuracy_factor = match model.as_str() {
                    "LSTM" => 1.02 + 0.01 * round as f64, // Getting better
                    "GRU" => 1.05,                        // Stable
                    "TCN" => 1.10 - 0.01 * round as f64,  // Getting worse
                    _ => 1.05,
                };
                
                let actual: Vec<f64> = model_predictions.iter()
                    .map(|p| p.value * accuracy_factor)
                    .collect();
                
                predictor.update_performance(model, &actual, &model_predictions).await.unwrap();
            }
        }
        
        // Verify performance tracking
        let stats = predictor.get_ensemble_stats().await.unwrap();
        if let Some(performances) = stats.get("model_performances").and_then(|v| v.as_object()) {
            // LSTM should have improving performance
            if let Some(lstm_perf) = performances.get("LSTM").and_then(|v| v.as_object()) {
                let accuracy = lstm_perf.get("recent_accuracy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                assert!(accuracy > 0.8, "LSTM accuracy should be high: {}", accuracy);
            }
            
            // TCN should have declining performance
            if let Some(tcn_perf) = performances.get("TCN").and_then(|v| v.as_object()) {
                let accuracy = tcn_perf.get("recent_accuracy").and_then(|v| v.as_f64()).unwrap_or(1.0);
                assert!(accuracy < 0.5, "TCN accuracy should be low: {}", accuracy);
            }
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_neural_workflow() {
        let config = create_test_neural_config(vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
            "DeepAR".to_string(),
            "NHITS".to_string(),
        ]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Create multi-regime data
        let mut full_data = Vec::new();
        full_data.extend(create_pattern_data("bullish", 100));
        full_data.extend(create_pattern_data("volatile", 100));
        full_data.extend(create_pattern_data("bearish", 100));
        full_data.extend(create_pattern_data("sideways", 100));
        
        // Test single model predictions
        for model in &["LSTM", "GRU", "TCN", "Transformer", "DeepAR", "NHITS"] {
            let predictions = predictor.predict_with_model(model, &full_data[..200], 5).await;
            assert!(predictions.is_ok(), "Model {} failed", model);
            assert_eq!(predictions.unwrap().len(), 5);
        }
        
        // Test ensemble predictions
        let all_models = vec![
            "LSTM".to_string(),
            "GRU".to_string(),
            "TCN".to_string(),
            "Transformer".to_string(),
            "DeepAR".to_string(),
            "NHITS".to_string(),
        ];
        
        let ensemble_predictions = predictor.predict_ensemble(&full_data, 20, &all_models, None).await.unwrap();
        assert_eq!(ensemble_predictions.len(), 20);
        
        // Verify ensemble properties
        for pred in &ensemble_predictions {
            assert!(pred.confidence > 0.5);
            assert!(pred.interval_low < pred.value);
            assert!(pred.interval_high > pred.value);
            assert!(pred.model_name.contains("ensemble"));
        }
        
        // Test feature importance
        let importance = predictor.get_feature_importance().await.unwrap();
        assert!(importance.contains_key("price"));
        assert!(importance.contains_key("volume"));
        
        // Test online learning
        let new_data = create_pattern_data("bullish", 50);
        for model in &["LSTM", "GRU"] {
            let result = predictor.update_with_new_data(model, &new_data).await;
            assert!(result.is_ok());
        }
        
        // Get final ensemble stats
        let stats = predictor.get_ensemble_stats().await.unwrap();
        assert!(stats.contains_key("current_regime"));
        assert!(stats.contains_key("dynamic_weights"));
        assert!(stats.contains_key("model_performances"));
    }

    #[tokio::test]
    async fn test_error_handling_and_recovery() {
        let config = create_test_neural_config(vec!["LSTM".to_string(), "InvalidModel".to_string()]);
        let predictor = FannPredictor::new(config).unwrap();
        
        // Test with insufficient data
        let small_data = create_pattern_data("bullish", 5);
        let result = predictor.predict(&small_data, 3, None).await;
        assert!(result.is_err());
        
        // Test with invalid model in ensemble
        let data = create_pattern_data("bullish", 100);
        let models = vec!["LSTM".to_string(), "InvalidModel".to_string()];
        let ensemble_result = predictor.predict_ensemble(&data, 5, &models, None).await;
        
        // Should succeed with valid models only
        assert!(ensemble_result.is_ok());
        
        // Test performance update with mismatched data
        let predictions = predictor.predict_with_model("LSTM", &data, 5).await.unwrap();
        let wrong_actual = vec![100.0]; // Wrong size
        let update_result = predictor.update_performance("LSTM", &wrong_actual, &predictions).await;
        
        // Should handle gracefully
        assert!(update_result.is_ok());
    }
}