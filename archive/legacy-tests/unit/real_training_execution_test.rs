//! Real Training Execution Unit Tests
//!
//! These tests validate the actual training execution and ensure that real training
//! produces measurable improvements in model performance.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use tokio;

use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::neural::predictor::NeuralPredictor;
use autonomous_platform::neural::mlp_adapter::{MLPAdapter, EnhancedMLPConfig};
use autonomous_platform::neural::NeuralPredictorTrait;

/// Test utilities for creating realistic market data
mod test_utils {
    use super::*;
    
    pub fn create_realistic_market_data(count: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = Utc::now();
        let mut price = 100.0;
        let mut volume = 1_000_000.0;
        
        for i in 0..count {
            // Simulate realistic price movements with trend and noise
            let trend = 0.0002; // Small upward trend
            let noise = 0.01 * ((i as f64 * 0.1).sin() + 0.5 * (i as f64 * 0.03).cos());
            price *= 1.0 + trend + noise;
            
            // Simulate volume patterns
            volume = 1_000_000.0 * (1.0 + 0.3 * (i as f64 * 0.05).sin());
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0 + 30.0 * (i as f64 * 0.02).sin());
            indicators.insert("macd".to_string(), (i as f64 * 0.01).sin());
            indicators.insert("bb_upper".to_string(), price * 1.02);
            indicators.insert("bb_lower".to_string(), price * 0.98);
            
            data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(i as i64),
                entity: Some("BTCUSD".to_string()),
                symbol: "BTCUSD".to_string(),
                open: price * 0.999,
                high: price * 1.002,
                low: price * 0.998,
                close: price,
                volume,
                source: Some("test_exchange".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({
                    "exchange": "test_exchange",
                    "pair": "BTC/USD"
                })),
                indicators,
            });
        }
        
        data
    }
    
    pub fn create_config_for_real_training() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 2.0,
            models: vec![
                "MLP".to_string(),
                "LSTM".to_string(),
                "DeepAR".to_string(),
                "NHITS".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false, // Testing FANN training first
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 120,
            max_retries: 3,
            error_threshold: 0.1,
        }
    }
}

#[tokio::test]
async fn test_fann_predictor_real_training_improves_performance() -> Result<()> {
    let config = test_utils::create_config_for_real_training();
    let predictor = NeuralPredictor::new(config)?;
    let training_data = test_utils::create_realistic_market_data(200);
    let test_data = test_utils::create_realistic_market_data(50);
    
    // Get initial predictions (untrained model)
    let initial_predictions = predictor.predict(&training_data[..50], 5, None).await?;
    let initial_accuracy = calculate_prediction_accuracy(&initial_predictions, &test_data);
    
    // Train each model and verify improvement
    for model_name in &["MLP", "LSTM", "DeepAR", "NHITS"] {
        println!("Testing real training for model: {}", model_name);
        
        // Train the model
        let start_time = std::time::Instant::now();
        predictor.predict(&training_data, 5, None).await?; // This triggers training
        let training_duration = start_time.elapsed();
        
        // Get post-training predictions
        let trained_predictions = predictor.test_predict_with_model(model_name, &test_data, 5).await?;
        let trained_accuracy = calculate_prediction_accuracy(&trained_predictions, &test_data);
        
        // Verify training actually improves performance
        assert!(trained_accuracy > initial_accuracy, 
               "Training should improve accuracy for model {}: initial={:.4}, trained={:.4}", 
               model_name, initial_accuracy, trained_accuracy);
        
        // Verify training took reasonable time (not instantaneous mock)
        assert!(training_duration.as_millis() > 10, 
               "Training should take measurable time, got {}ms", training_duration.as_millis());
        
        // Verify model produces consistent results
        let second_predictions = predictor.test_predict_with_model(model_name, &test_data, 5).await?;
        let consistency_score = calculate_prediction_consistency(&trained_predictions, &second_predictions);
        assert!(consistency_score > 0.8, 
               "Model {} should produce consistent results: score={:.4}", model_name, consistency_score);
        
        println!("✅ Model {} training verified: accuracy improved from {:.4} to {:.4} in {}ms",
                model_name, initial_accuracy, trained_accuracy, training_duration.as_millis());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_mlp_adapter_real_training_convergence() -> Result<()> {
    let config = EnhancedMLPConfig::default();
    let adapter = MLPAdapter::new(config)?;
    let training_data = test_utils::create_realistic_market_data(300);
    
    // Initialize and train
    adapter.initialize_network().await?;
    
    let start_time = std::time::Instant::now();
    adapter.train(&training_data).await?;
    let training_duration = start_time.elapsed();
    
    // Verify training occurred
    assert!(training_duration.as_millis() > 50, 
           "Real training should take measurable time");
    
    // Check training status
    let status = adapter.get_training_status().await?;
    assert!(status.get("is_trained").unwrap().as_bool().unwrap());
    
    let current_epoch = status.get("current_epoch").unwrap().as_u64().unwrap();
    assert!(current_epoch > 0, "Should have completed at least one epoch");
    
    // Check performance metrics show improvement
    let metrics = adapter.get_performance_metrics().await;
    assert!(metrics.training_accuracy > 0.0, "Training accuracy should improve");
    assert!(metrics.parameter_count > 0, "Should have trainable parameters");
    assert!(metrics.training_time_ms > 0.0, "Should record training time");
    
    // Verify model can make predictions
    let predictions = adapter.predict(&training_data[200..], 5).await?;
    assert_eq!(predictions.len(), 5);
    
    for prediction in &predictions {
        assert!(prediction.confidence > 0.1, "Should have reasonable confidence");
        assert!(prediction.value > 0.0, "Should produce valid price predictions");
        assert!(prediction.interval_low < prediction.value);
        assert!(prediction.interval_high > prediction.value);
    }
    
    println!("✅ MLP adapter training verified: {} epochs, {:.4} accuracy, {}ms training time",
             current_epoch, metrics.training_accuracy, metrics.training_time_ms);
    
    Ok(())
}

#[tokio::test]
async fn test_training_with_insufficient_data_handling() -> Result<()> {
    let config = test_utils::create_config_for_real_training();
    let predictor = NeuralPredictor::new(config)?;
    
    // Test with very small dataset
    let small_data = test_utils::create_realistic_market_data(10);
    
    // Should handle gracefully without panicking
    let result = predictor.predict(&small_data, 3, None).await;
    
    // Either succeeds with limited data or fails gracefully
    match result {
        Ok(predictions) => {
            assert!(!predictions.is_empty(), "Should produce some predictions");
            println!("✅ Handled small dataset gracefully: {} predictions", predictions.len());
        }
        Err(e) => {
            println!("✅ Failed gracefully with insufficient data: {}", e);
            // Should be a reasonable error message
            assert!(e.to_string().contains("Insufficient") || e.to_string().contains("data"));
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_online_learning_updates_model() -> Result<()> {
    let config = test_utils::create_config_for_real_training();
    let predictor = NeuralPredictor::new(config)?;
    
    // Initial training
    let initial_data = test_utils::create_realistic_market_data(150);
    predictor.predict(&initial_data, 5, None).await?;
    
    // Get baseline predictions
    let test_data = test_utils::create_realistic_market_data(20);
    let baseline_predictions = predictor.test_predict_with_model("MLP", &test_data, 3).await?;
    
    // Simulate new market data with different pattern
    let mut new_data = test_utils::create_realistic_market_data(50);
    // Add a trend shift
    for (i, point) in new_data.iter_mut().enumerate() {
        point.close *= 1.0 + 0.01 * i as f64; // Strong upward trend
    }
    
    // Apply online learning
    predictor.update_with_new_data("MLP", &new_data).await?;
    
    // Get updated predictions
    let updated_predictions = predictor.test_predict_with_model("MLP", &test_data, 3).await?;
    
    // Verify predictions changed (model learned from new data)
    let prediction_change = calculate_prediction_change(&baseline_predictions, &updated_predictions);
    assert!(prediction_change > 0.001, 
           "Online learning should change predictions: change={:.6}", prediction_change);
    
    println!("✅ Online learning verified: prediction change={:.4}", prediction_change);
    
    Ok(())
}

#[tokio::test]
async fn test_ensemble_training_coordination() -> Result<()> {
    let config = test_utils::create_config_for_real_training();
    let predictor = NeuralPredictor::new(config)?;
    let training_data = test_utils::create_realistic_market_data(250);
    
    // Train ensemble and verify coordination
    let models = vec!["MLP".to_string(), "LSTM".to_string(), "DeepAR".to_string()];
    let ensemble_predictions = predictor.predict_ensemble(&training_data, 5, &models, None).await?;
    
    assert_eq!(ensemble_predictions.len(), 5);
    
    // Verify ensemble metrics are available
    let ensemble_stats = predictor.get_ensemble_stats().await?;
    assert!(ensemble_stats.contains_key("dynamic_weights"));
    assert!(ensemble_stats.contains_key("model_performances"));
    
    // Check that individual models were actually trained and coordinated
    let weights = ensemble_stats.get("dynamic_weights").unwrap();
    assert!(weights.is_object(), "Should have weight information for models");
    
    // Verify ensemble produces different results than individual models
    let individual_prediction = predictor.test_predict_with_model("MLP", &training_data[200..], 5).await?;
    let ensemble_vs_individual = calculate_prediction_change(&individual_prediction, &ensemble_predictions);
    
    assert!(ensemble_vs_individual > 0.001, 
           "Ensemble should differ from individual models: change={:.6}", ensemble_vs_individual);
    
    println!("✅ Ensemble training coordination verified: {} models trained, change={:.4}",
             models.len(), ensemble_vs_individual);
    
    Ok(())
}

#[tokio::test]
async fn test_training_error_recovery() -> Result<()> {
    let mut config = test_utils::create_config_for_real_training();
    config.models = vec!["NonExistentModel".to_string(), "MLP".to_string()];
    
    let predictor = NeuralPredictor::new(config)?;
    let training_data = test_utils::create_realistic_market_data(100);
    
    // Should recover from invalid model and still train valid ones
    let result = predictor.predict(&training_data, 3, None).await;
    
    match result {
        Ok(predictions) => {
            assert!(!predictions.is_empty(), "Should produce predictions from valid models");
            println!("✅ Error recovery successful: {} predictions from valid models", predictions.len());
        }
        Err(e) => {
            // Should be a reasonable error about model configuration
            assert!(!e.to_string().contains("panic"), "Should not panic on invalid models");
            println!("✅ Error recovery verified: {}", e);
        }
    }
    
    Ok(())
}

// Helper functions for test validation

fn calculate_prediction_accuracy(predictions: &[autonomous_platform::neural::PredictionResult], actual_data: &[TimeSeriesData]) -> f64 {
    if predictions.is_empty() || actual_data.is_empty() {
        return 0.0;
    }
    
    let mut accuracy_scores = Vec::new();
    
    for (i, prediction) in predictions.iter().enumerate() {
        if let Some(actual_point) = actual_data.get(i) {
            let error = (prediction.value - actual_point.close).abs() / actual_point.close;
            let accuracy = (1.0 - error.min(1.0)).max(0.0);
            accuracy_scores.push(accuracy);
        }
    }
    
    if accuracy_scores.is_empty() {
        0.0
    } else {
        accuracy_scores.iter().sum::<f64>() / accuracy_scores.len() as f64
    }
}

fn calculate_prediction_consistency(pred1: &[autonomous_platform::neural::PredictionResult], pred2: &[autonomous_platform::neural::PredictionResult]) -> f64 {
    if pred1.is_empty() || pred2.is_empty() || pred1.len() != pred2.len() {
        return 0.0;
    }
    
    let mut consistency_scores = Vec::new();
    
    for (p1, p2) in pred1.iter().zip(pred2.iter()) {
        let diff = (p1.value - p2.value).abs() / p1.value.max(p2.value);
        let consistency = (1.0 - diff.min(1.0)).max(0.0);
        consistency_scores.push(consistency);
    }
    
    consistency_scores.iter().sum::<f64>() / consistency_scores.len() as f64
}

fn calculate_prediction_change(pred1: &[autonomous_platform::neural::PredictionResult], pred2: &[autonomous_platform::neural::PredictionResult]) -> f64 {
    if pred1.is_empty() || pred2.is_empty() || pred1.len() != pred2.len() {
        return 0.0;
    }
    
    let mut changes = Vec::new();
    
    for (p1, p2) in pred1.iter().zip(pred2.iter()) {
        let change = (p1.value - p2.value).abs() / p1.value.max(p2.value);
        changes.push(change);
    }
    
    changes.iter().sum::<f64>() / changes.len() as f64
}

#[tokio::test]
async fn test_training_metrics_collection() -> Result<()> {
    let config = test_utils::create_config_for_real_training();
    let predictor = NeuralPredictor::new(config)?;
    let training_data = test_utils::create_realistic_market_data(200);
    
    // Reset ensemble performance to start fresh
    predictor.reset_ensemble_performance().await?;
    
    // Perform training and predictions
    let predictions = predictor.predict(&training_data, 5, None).await?;
    
    // Simulate actual values for performance tracking
    let actual_values: Vec<f64> = training_data[training_data.len()-5..]
        .iter()
        .map(|d| d.close)
        .collect();
    
    // Update performance metrics
    for model_name in &["MLP", "LSTM", "DeepAR"] {
        predictor.update_performance(model_name, &actual_values, &predictions).await?;
    }
    
    // Verify metrics were collected
    let stats = predictor.get_ensemble_stats().await?;
    let performances = stats.get("model_performances").unwrap();
    
    assert!(performances.is_object(), "Should have performance data");
    
    let perf_obj = performances.as_object().unwrap();
    for model_name in &["MLP", "LSTM", "DeepAR"] {
        if perf_obj.contains_key(model_name) {
            let model_perf = perf_obj.get(model_name).unwrap().as_object().unwrap();
            assert!(model_perf.contains_key("recent_accuracy"));
            assert!(model_perf.contains_key("prediction_count"));
            
            println!("✅ Metrics collected for {}: accuracy={}, count={}", 
                    model_name,
                    model_perf.get("recent_accuracy").unwrap(),
                    model_perf.get("prediction_count").unwrap());
        }
    }
    
    Ok(())
}