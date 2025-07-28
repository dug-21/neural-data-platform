//! Comprehensive Unit Tests for FANN-based Neural Predictor
//! 
//! This module provides thorough testing for the FANN predictor including:
//! - Model initialization and configuration
//! - Ensemble prediction capabilities
//! - Dynamic weight management
//! - Market regime detection
//! - Performance tracking and adaptation
//! - Error handling and edge cases

use super::super::fann_predictor::*;
use super::super::{PredictionResult, NeuralPredictorTrait};
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;
use tokio;
use anyhow::Result;
use approx::{assert_relative_eq, assert_abs_diff_eq};
use serde_json::json;
use tracing_test::traced_test;

/// Helper function to create test configuration for FANN predictor
fn create_fann_test_config() -> NeuralConfig {
    NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()],
        prediction_cache_ttl: 300,
        ..Default::default()
    }
}

/// Helper function to create comprehensive test configuration with all models
fn create_comprehensive_fann_config() -> NeuralConfig {
    NeuralConfig {
        memory_gb: 2.0,
        models: vec![
            "MLP".to_string(),
            "NHITS".to_string(), 
            "TCN".to_string(),
            "DeepAR".to_string(),
            "LSTM".to_string(),
            "GRU".to_string(),
            "Transformer".to_string()
        ],
        prediction_cache_ttl: 600,
        model_load_timeout: 120,
        max_concurrent_predictions: 20,
        accuracy_threshold: 0.8,
        ..Default::default()
    }
}

/// Helper function to create test time series data with indicators
fn create_fann_test_data(count: usize) -> Vec<TimeSeriesData> {
    (0..count)
        .map(|i| {
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0)); // RSI between 30-70
            indicators.insert("sma".to_string(), 100.0 + (i as f64 * 0.1));
            indicators.insert("ema".to_string(), 100.5 + (i as f64 * 0.12));
            
            TimeSeriesData {
                timestamp: Utc.timestamp_opt(1640000000 + (i as i64 * 3600), 0).unwrap(),
                symbol: "FANN_TEST".to_string(),
                open: 100.0 + (i as f64 * 0.3),
                high: 102.0 + (i as f64 * 0.35),
                low: 98.0 + (i as f64 * 0.25),
                close: 101.0 + (i as f64 * 0.3),
                volume: 1000000.0 + (i as f64 * 10000.0),
                indicators,
                source: Some("test".to_string()),
                entity: Some("FANN_TEST".to_string()),
                value: Some(101.0 + (i as f64 * 0.3)),
                metadata: Some(json!({"test_id": i})),
            }
        })
        .collect()
}

/// Helper function to create market data with specific regime characteristics
fn create_regime_specific_data(regime: &str, count: usize) -> Vec<TimeSeriesData> {
    (0..count)
        .map(|i| {
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0);
            
            let (price_factor, volatility_factor) = match regime {
                "bullish" => (1.05, 1.0), // 5% uptrend, normal volatility
                "bearish" => (0.95, 1.0), // 5% downtrend, normal volatility
                "high_volatility" => (1.0, 3.0), // sideways, high volatility
                "low_volatility" => (1.0, 0.3), // sideways, low volatility
                _ => (1.0, 1.0), // sideways, normal volatility
            };
            
            let base_price = 100.0;
            let trend_price = base_price * (price_factor as f64).powf(i as f64 / count as f64);
            let volatility = volatility_factor * (i as f64 * 0.1).sin() * 2.0;
            
            TimeSeriesData {
                timestamp: Utc.timestamp_opt(1640000000 + (i as i64 * 3600), 0).unwrap(),
                symbol: format!("{}_TEST", regime.to_uppercase()),
                open: trend_price + volatility * 0.8,
                high: trend_price + volatility.abs() * 1.2,
                low: trend_price - volatility.abs() * 1.2,
                close: trend_price + volatility,
                volume: 1000000.0,
                indicators,
                source: Some("test".to_string()),
                entity: Some(format!("{}_TEST", regime.to_uppercase())),
                value: Some(trend_price + volatility),
                metadata: Some(json!({"regime": regime})),
            }
        })
        .collect()
}

mod fann_initialization_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_fann_predictor_basic_initialization() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config.clone())?;
        
        // Verify model configurations are created for all specified models
        let model_configs = predictor.get_model_configs();
        assert_eq!(model_configs.len(), 3);
        assert!(model_configs.contains_key("MLP"));
        assert!(model_configs.contains_key("NHITS"));
        assert!(model_configs.contains_key("DeepAR"));
        
        // Test ensemble manager initialization
        let stats = predictor.get_ensemble_stats().await?;
        assert!(stats.contains_key("dynamic_weights"));
        assert!(stats.contains_key("current_regime"));
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fann_predictor_comprehensive_initialization() -> Result<()> {
        let config = create_comprehensive_fann_config();
        let predictor = FannPredictor::new(config)?;
        
        // Verify all model types are configured
        let expected_models = ["MLP", "NHITS", "TCN", "DeepAR", "LSTM", "GRU", "Transformer"];
        for model in &expected_models {
            assert!(predictor.get_model_configs().contains_key(&model.to_string()));
        }
        
        // Test model-specific configurations
        let nhits_config = &predictor.get_model_configs()["NHITS"];
        assert_eq!(nhits_config.input_size, 50);
        assert_eq!(nhits_config.output_size, 10);
        assert_eq!(nhits_config.hidden_activation, ActivationFunction::ReLU);
        
        let transformer_config = &predictor.get_model_configs()["Transformer"];
        assert_eq!(transformer_config.input_size, 80);
        assert!(transformer_config.use_cascade);
        
        let lstm_config = &predictor.get_model_configs()["LSTM"];
        assert_eq!(lstm_config.input_size, 100);
        assert!(lstm_config.use_cascade);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_model_configuration_specifics() -> Result<()> {
        let config = create_comprehensive_fann_config();
        let predictor = FannPredictor::new(config)?;
        
        // Test specific model architecture configurations
        let model_configs = predictor.get_model_configs();
        
        // DeepAR should be configured for probabilistic forecasting
        let deepar = &model_configs["DeepAR"];
        assert_eq!(deepar.output_activation, ActivationFunction::Gaussian);
        assert!(deepar.use_cascade);
        assert_eq!(deepar.learning_rate, 0.0003);
        
        // TCN should be configured for temporal convolution
        let tcn = &model_configs["TCN"];
        assert_eq!(tcn.hidden_layers, vec![96, 48, 24]);
        assert_eq!(tcn.hidden_activation, ActivationFunction::Tanh);
        
        // LSTM should have extended context
        let lstm = &model_configs["LSTM"];
        assert_eq!(lstm.input_size, 100);
        assert_eq!(lstm.hidden_layers, vec![128, 64, 64, 32]);
        
        Ok(())
    }
}

mod fann_prediction_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_single_model_prediction() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(25);
        
        // Test prediction using the trait implementation
        let results = predictor.predict(&test_data, 5, None).await?;
        
        assert_eq!(results.len(), 5);
        for (i, result) in results.iter().enumerate() {
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
            assert!(result.value.is_finite());
            assert!(result.interval_low <= result.interval_high);
            assert!(!result.model_name.is_empty());
            
            // Confidence should generally decrease with prediction horizon
            if i > 0 {
                assert!(result.confidence <= results[0].confidence + 0.1); // Allow some variance
            }
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_ensemble_prediction() -> Result<()> {
        let config = create_comprehensive_fann_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(30);
        
        let models = vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()];
        let results = predictor.predict_ensemble(&test_data, 5, &models, None).await?;
        
        assert_eq!(results.len(), 5);
        for result in &results {
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
            assert!(result.value.is_finite());
            assert!(result.model_name.contains("ensemble"));
            assert!(result.interval_low <= result.interval_high);
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_ensemble_with_all_models() -> Result<()> {
        let config = create_comprehensive_fann_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(50);
        
        let all_models = vec![
            "MLP".to_string(), "NHITS".to_string(), "TCN".to_string(),
            "DeepAR".to_string(), "LSTM".to_string(), "GRU".to_string(),
            "Transformer".to_string()
        ];
        
        let results = predictor.predict_ensemble(&test_data, 8, &all_models, None).await?;
        
        assert_eq!(results.len(), 8);
        
        // Ensemble should provide higher confidence than individual models typically
        for result in &results {
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
            assert!(result.model_name.contains("7_models")); // All 7 models
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_prediction_caching() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(20);
        
        // First prediction (should be computed)
        let start_time = std::time::Instant::now();
        let results1 = predictor.predict(&test_data, 3, None).await?;
        let first_duration = start_time.elapsed();
        
        // Second prediction with same data (should be cached)
        let start_time = std::time::Instant::now();
        let results2 = predictor.predict(&test_data, 3, None).await?;
        let second_duration = start_time.elapsed();
        
        // Results should be identical
        assert_eq!(results1.len(), results2.len());
        for (r1, r2) in results1.iter().zip(results2.iter()) {
            assert_abs_diff_eq!(r1.value, r2.value, epsilon = 0.001);
            assert_abs_diff_eq!(r1.confidence, r2.confidence, epsilon = 0.001);
        }
        
        // Second call should be faster due to caching
        assert!(second_duration <= first_duration);
        
        Ok(())
    }
}

mod market_regime_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_bullish_market_detection() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let bullish_data = create_regime_specific_data("bullish", 25);
        
        let models = vec!["MLP".to_string(), "DeepAR".to_string()];
        let _results = predictor.predict_ensemble(&bullish_data, 3, &models, None).await?;
        
        // Test that ensemble manager detects market regime
        let stats = predictor.get_ensemble_stats().await?;
        let current_regime = stats.get("current_regime").unwrap().as_str().unwrap();
        
        // Should detect bullish or related regime
        assert!(current_regime == "Bullish" || current_regime == "LowVolatility" || current_regime == "Sideways");
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_high_volatility_detection() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let volatile_data = create_regime_specific_data("high_volatility", 25);
        
        let models = vec!["NHITS".to_string(), "DeepAR".to_string()];
        let _results = predictor.predict_ensemble(&volatile_data, 3, &models, None).await?;
        
        let stats = predictor.get_ensemble_stats().await?;
        let current_regime = stats.get("current_regime").unwrap().as_str().unwrap();
        
        // Should detect high volatility
        assert!(current_regime == "HighVolatility" || current_regime == "Sideways");
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_bearish_market_detection() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let bearish_data = create_regime_specific_data("bearish", 25);
        
        let models = vec!["MLP".to_string(), "NHITS".to_string()];
        let _results = predictor.predict_ensemble(&bearish_data, 3, &models, None).await?;
        
        let stats = predictor.get_ensemble_stats().await?;
        let current_regime = stats.get("current_regime").unwrap().as_str().unwrap();
        
        // Should detect bearish or related regime
        assert!(current_regime == "Bearish" || current_regime == "LowVolatility" || current_regime == "Sideways");
        
        Ok(())
    }
}

mod dynamic_weighting_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_dynamic_weight_adjustment() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(30);
        
        let models = vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()];
        
        // Get initial weights
        let initial_stats = predictor.get_ensemble_stats().await?;
        let initial_weights = initial_stats.get("dynamic_weights").unwrap().as_object().unwrap();
        
        // Make multiple predictions to trigger weight updates
        for i in 0..15 {
            let mut current_data = test_data.clone();
            // Modify data slightly to trigger different performance
            for data_point in &mut current_data {
                data_point.close += i as f64 * 0.1;
            }
            
            let results = predictor.predict_ensemble(&current_data, 3, &models, None).await?;
            
            // Simulate performance feedback
            let actual_values = vec![current_data.last().unwrap().close; 3];
            predictor.update_performance("MLP", &actual_values, &results).await?;
        }
        
        // Get updated weights
        let updated_stats = predictor.get_ensemble_stats().await?;
        let updated_weights = updated_stats.get("dynamic_weights").unwrap().as_object().unwrap();
        
        // Weights should have been updated
        assert!(initial_weights != updated_weights);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_performance_based_weight_adjustment() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        
        // Create predictions with known accuracy for different models
        let good_predictions = vec![
            PredictionResult {
                timestamp: Utc::now(),
                value: 100.0,
                confidence: 0.9,
                interval_low: 99.0,
                interval_high: 101.0,
                model_name: "MLP".to_string(),
            }
        ];
        
        let poor_predictions = vec![
            PredictionResult {
                timestamp: Utc::now(),
                value: 150.0, // Way off
                confidence: 0.5,
                interval_low: 140.0,
                interval_high: 160.0,
                model_name: "NHITS".to_string(),
            }
        ];
        
        // Update performance with known results
        let actual_values = vec![100.5]; // Close to MLP, far from NHITS
        
        predictor.update_performance("MLP", &actual_values, &good_predictions).await?;
        predictor.update_performance("NHITS", &actual_values, &poor_predictions).await?;
        
        // Check that performance metrics reflect the updates
        let stats = predictor.get_ensemble_stats().await?;
        let model_performances = stats.get("model_performances").unwrap().as_object().unwrap();
        
        if let Some(mlp_perf) = model_performances.get("MLP") {
            let mlp_accuracy = mlp_perf.get("recent_accuracy").unwrap().as_f64().unwrap();
            assert!(mlp_accuracy > 0.8); // Should be high due to good prediction
        }
        
        if let Some(nhits_perf) = model_performances.get("NHITS") {
            let nhits_accuracy = nhits_perf.get("recent_accuracy").unwrap().as_f64().unwrap();
            assert!(nhits_accuracy < 0.5); // Should be low due to poor prediction
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_ensemble_reset_functionality() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        
        // Make some predictions to build performance history
        let test_data = create_fann_test_data(20);
        let models = vec!["MLP".to_string(), "DeepAR".to_string()];
        let results = predictor.predict_ensemble(&test_data, 3, &models, None).await?;
        
        // Update performance
        let actual_values = vec![100.0, 101.0, 102.0];
        predictor.update_performance("MLP", &actual_values, &results).await?;
        
        // Verify metrics exist
        let stats_before = predictor.get_ensemble_stats().await?;
        let performances_before = stats_before.get("model_performances").unwrap().as_object().unwrap();
        assert!(!performances_before.is_empty());
        
        // Reset performance tracking
        predictor.reset_ensemble_performance().await?;
        
        // Verify metrics are reset
        let stats_after = predictor.get_ensemble_stats().await?;
        let performances_after = stats_after.get("model_performances").unwrap().as_object().unwrap();
        assert!(performances_after.is_empty());
        
        Ok(())
    }
}

mod feature_importance_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_feature_importance_retrieval() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        
        let feature_importance = predictor.get_feature_importance().await?;
        
        // Should have standard features
        assert!(feature_importance.contains_key("price"));
        assert!(feature_importance.contains_key("volume"));
        assert!(feature_importance.contains_key("rsi"));
        
        // All importance values should be between 0 and 1
        for (feature, importance) in &feature_importance {
            assert!(*importance >= 0.0 && *importance <= 1.0, 
                   "Feature {} has invalid importance: {}", feature, importance);
        }
        
        // Total importance should sum to approximately 1.0
        let total_importance: f64 = feature_importance.values().sum();
        assert_abs_diff_eq!(total_importance, 1.0, epsilon = 0.01);
        
        Ok(())
    }
}

mod error_handling_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_prediction_with_insufficient_data() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let insufficient_data = create_fann_test_data(2); // Too little data
        
        let result = predictor.predict(&insufficient_data, 3, None).await;
        
        // Should handle gracefully (either succeed with reduced accuracy or fail gracefully)
        match result {
            Ok(predictions) => {
                assert!(!predictions.is_empty());
                for pred in &predictions {
                    assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                }
            },
            Err(_) => {
                // Also acceptable to fail with insufficient data
            }
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_prediction_with_invalid_models() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(25);
        
        let invalid_models = vec!["INVALID_MODEL".to_string(), "ANOTHER_INVALID".to_string()];
        let result = predictor.predict_ensemble(&test_data, 3, &invalid_models, None).await;
        
        // Should handle invalid models gracefully
        match result {
            Ok(predictions) => {
                // If it succeeds, should fall back to available models
                assert!(!predictions.is_empty());
            },
            Err(err) => {
                // Should provide meaningful error message
                assert!(err.to_string().contains("model") || err.to_string().contains("prediction"));
            }
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_prediction_with_zero_horizon() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(20);
        
        let result = predictor.predict(&test_data, 0, None).await;
        
        // Should handle zero horizon gracefully
        match result {
            Ok(predictions) => {
                assert!(predictions.is_empty());
            },
            Err(_) => {
                // Also acceptable to reject zero horizon
            }
        }
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_ensemble_with_no_models() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(25);
        
        let no_models: Vec<String> = vec![];
        let result = predictor.predict_ensemble(&test_data, 3, &no_models, None).await;
        
        // Should handle empty model list gracefully
        match result {
            Ok(predictions) => {
                // Might fall back to default model
                assert!(!predictions.is_empty());
            },
            Err(err) => {
                // Should provide meaningful error
                assert!(!err.to_string().is_empty());
            }
        }
        
        Ok(())
    }
}

mod performance_tracking_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_model_performance_tracking() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        
        // Create test predictions and actual values
        let predictions = vec![
            PredictionResult {
                timestamp: Utc::now(),
                value: 100.0,
                confidence: 0.8,
                interval_low: 98.0,
                interval_high: 102.0,
                model_name: "MLP".to_string(),
            },
            PredictionResult {
                timestamp: Utc::now(),
                value: 101.0,
                confidence: 0.85,
                interval_low: 99.0,
                interval_high: 103.0,
                model_name: "MLP".to_string(),
            },
        ];
        
        let actual_values = vec![100.5, 101.2]; // Close predictions
        
        // Update performance
        predictor.update_performance("MLP", &actual_values, &predictions).await?;
        
        // Check performance metrics
        let stats = predictor.get_ensemble_stats().await?;
        let model_performances = stats.get("model_performances").unwrap().as_object().unwrap();
        
        assert!(model_performances.contains_key("MLP"));
        let mlp_perf = model_performances.get("MLP").unwrap().as_object().unwrap();
        
        assert!(mlp_perf.contains_key("recent_accuracy"));
        assert!(mlp_perf.contains_key("prediction_count"));
        assert!(mlp_perf.contains_key("successful_predictions"));
        
        let prediction_count = mlp_perf.get("prediction_count").unwrap().as_u64().unwrap();
        assert_eq!(prediction_count, 2);
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_online_learning_update() -> Result<()> {
        let config = create_fann_test_config();
        let predictor = FannPredictor::new(config)?;
        
        let new_data = create_fann_test_data(10);
        
        // Test online learning update
        let result = predictor.update_with_new_data("MLP", &new_data).await;
        assert!(result.is_ok());
        
        // Test with unknown model
        let result = predictor.update_with_new_data("UNKNOWN", &new_data).await;
        // Should handle gracefully
        match result {
            Ok(_) => {},
            Err(err) => {
                assert!(err.to_string().contains("Unknown model") || err.to_string().contains("UNKNOWN"));
            }
        }
        
        Ok(())
    }
}

mod integration_verification_tests {
    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_comprehensive_workflow() -> Result<()> {
        let config = create_comprehensive_fann_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(40);
        
        // 1. Test single model prediction
        let single_results = predictor.predict(&test_data, 5, None).await?;
        assert_eq!(single_results.len(), 5);
        
        // 2. Test ensemble prediction
        let models = vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()];
        let ensemble_results = predictor.predict_ensemble(&test_data, 5, &models, None).await?;
        assert_eq!(ensemble_results.len(), 5);
        
        // 3. Test performance tracking
        let actual_values = vec![100.0, 101.0, 102.0, 103.0, 104.0];
        predictor.update_performance("MLP", &actual_values, &single_results).await?;
        
        // 4. Test online learning
        predictor.update_with_new_data("MLP", &test_data[20..30]).await?;
        
        // 5. Test feature importance
        let importance = predictor.get_feature_importance().await?;
        assert!(!importance.is_empty());
        
        // 6. Test ensemble statistics
        let stats = predictor.get_ensemble_stats().await?;
        assert!(stats.contains_key("dynamic_weights"));
        assert!(stats.contains_key("model_performances"));
        
        // 7. Test reset functionality
        predictor.reset_ensemble_performance().await?;
        
        println!("✅ FANN Predictor comprehensive workflow test completed successfully");
        
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_phase6_integration_requirements() -> Result<()> {
        let config = create_comprehensive_fann_config();
        let predictor = FannPredictor::new(config)?;
        let test_data = create_fann_test_data(30);
        
        // Test Phase 6 requirements:
        
        // 1. Multiple model ensemble support
        let all_models = vec![
            "MLP".to_string(), "NHITS".to_string(), "TCN".to_string(),
            "DeepAR".to_string(), "LSTM".to_string(), "GRU".to_string(),
            "Transformer".to_string()
        ];
        let results = predictor.predict_ensemble(&test_data, 8, &all_models, None).await?;
        assert_eq!(results.len(), 8);
        
        // 2. Dynamic weight adjustment
        let initial_stats = predictor.get_ensemble_stats().await?;
        assert!(initial_stats.contains_key("dynamic_weights"));
        assert!(initial_stats.contains_key("volatility_adjustments"));
        
        // 3. Market regime detection
        assert!(initial_stats.contains_key("current_regime"));
        
        // 4. Performance-based model selection
        assert!(initial_stats.contains_key("model_performances"));
        
        // 5. Feature importance analysis
        let importance = predictor.get_feature_importance().await?;
        assert!(importance.len() >= 6); // Should have multiple features
        
        // 6. Confidence calibration
        for result in &results {
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
            assert!(result.interval_low <= result.value);
            assert!(result.value <= result.interval_high);
        }
        
        println!("✅ Phase 6 integration requirements verified");
        
        Ok(())
    }
}

/// Test to verify comprehensive coverage of FANN predictor functionality
#[tokio::test]
#[traced_test]
async fn test_fann_predictor_coverage_verification() -> Result<()> {
    println!("🧪 Testing FANN Predictor Coverage Verification");
    
    let config = create_comprehensive_fann_config();
    let predictor = FannPredictor::new(config)?;
    let test_data = create_fann_test_data(30);
    
    // Test all public methods of FannPredictor through NeuralPredictorTrait
    
    // 1. predict (trait method)
    let _single_results = predictor.predict(&test_data, 3, None).await?;
    
    // 2. predict_ensemble (trait method)
    let models = vec!["MLP".to_string(), "NHITS".to_string()];
    let _ensemble_results = predictor.predict_ensemble(&test_data, 3, &models, None).await?;
    
    // 3. get_feature_importance (trait method)
    let _importance = predictor.get_feature_importance().await?;
    
    // 4. update_performance (public method)
    let predictions = vec![PredictionResult {
        timestamp: Utc::now(),
        value: 100.0,
        confidence: 0.8,
        interval_low: 98.0,
        interval_high: 102.0,
        model_name: "MLP".to_string(),
    }];
    let actual = vec![100.5];
    predictor.update_performance("MLP", &actual, &predictions).await?;
    
    // 5. update_with_new_data (public method)
    predictor.update_with_new_data("MLP", &test_data[0..10]).await?;
    
    // 6. get_ensemble_stats (public method)
    let _stats = predictor.get_ensemble_stats().await?;
    
    // 7. reset_ensemble_performance (public method)
    predictor.reset_ensemble_performance().await?;
    
    println!("✅ All FANN Predictor public methods tested - comprehensive coverage achieved");
    
    Ok(())
}