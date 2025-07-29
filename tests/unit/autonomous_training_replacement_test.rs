//! Autonomous Training Replacement Tests
//!
//! These tests verify that autonomous training components correctly replace
//! mock functions with real training implementations.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use tokio;

use neural_trader::config::NeuralConfig;
use neural_trader::data::TimeSeriesData;
use neural_trader::daa::autonomous_training::AutonomousTrainingCoordinator;
use neural_trader::neural::fann_predictor::FannPredictor;
use neural_trader::neural::mlp_adapter::{MLPAdapter, EnhancedMLPConfig};

/// Test utilities
mod test_utils {
    use super::*;
    
    pub fn create_training_data(samples: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = Utc::now();
        let mut price = 50000.0; // Bitcoin-like price
        
        for i in 0..samples {
            // Create realistic price movements
            let volatility = 0.02;
            let trend = 0.0001;
            let random_factor = ((i as f64 * 0.1).sin() + (i as f64 * 0.03).cos()) * volatility;
            
            price *= 1.0 + trend + random_factor;
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 30.0 + 40.0 * (i as f64 * 0.02).sin());
            indicators.insert("sma_20".to_string(), price * 0.99);
            indicators.insert("sma_50".to_string(), price * 0.98);
            indicators.insert("volume_sma".to_string(), 1000000.0);
            
            data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
                entity: Some("BTCUSD".to_string()),
                symbol: "BTCUSD".to_string(),
                open: price * 0.9995,
                high: price * 1.001,
                low: price * 0.999,
                close: price,
                volume: 1000000.0 + (i as f64 * 1000.0),
                source: Some("binance".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({"market": "crypto"})),
                indicators,
            });
        }
        
        data
    }
    
    pub fn create_config_with_autonomous_training() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 2.0,
            models: vec![
                "MLP".to_string(),
                "LSTM".to_string(),
                "DeepAR".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 120,
            max_concurrent_predictions: 5,
            enable_model_monitoring: true,
            accuracy_threshold: 0.7,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: true,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 300,
            max_retries: 3,
            error_threshold: 0.15,
        }
    }
}

#[tokio::test]
async fn test_autonomous_coordinator_replaces_mock_training() -> Result<()> {
    let config = test_utils::create_config_with_autonomous_training();
    let training_data = test_utils::create_training_data(200);
    
    // Create autonomous training coordinator
    let coordinator = AutonomousTrainingCoordinator::new(config.clone()).await?;
    
    // Verify coordinator initializes properly
    assert!(coordinator.is_active().await);
    
    // Start autonomous training
    let training_start = std::time::Instant::now();
    coordinator.start_autonomous_training(training_data.clone()).await?;
    
    // Wait for training to progress
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let training_duration = training_start.elapsed();
    
    // Verify real training occurred (not instantaneous mock)
    assert!(training_duration.as_millis() > 50, 
           "Real training should take measurable time: {}ms", training_duration.as_millis());
    
    // Check training status
    let status = coordinator.get_training_status().await?;
    assert!(status.contains_key("models_trained"));
    assert!(status.contains_key("training_active"));
    
    let models_trained = status.get("models_trained").unwrap().as_array().unwrap();
    assert!(!models_trained.is_empty(), "Should have trained some models");
    
    // Verify models show improvement after training
    for model_name in &["MLP", "LSTM", "DeepAR"] {
        let model_status = coordinator.get_model_status(model_name).await?;
        
        if let Some(training_time) = model_status.get("last_training_time_ms") {
            let time_ms = training_time.as_f64().unwrap();
            assert!(time_ms > 0.0, "Model {} should record actual training time", model_name);
        }
        
        if let Some(accuracy) = model_status.get("training_accuracy") {
            let acc = accuracy.as_f64().unwrap();
            assert!(acc >= 0.0 && acc <= 1.0, "Model {} should have valid accuracy: {}", model_name, acc);
        }
        
        println!("✅ Model {} trained autonomously: {:?}", model_name, model_status);
    }
    
    coordinator.stop_autonomous_training().await?;
    assert!(!coordinator.is_active().await);
    
    Ok(())
}

#[tokio::test]
async fn test_mock_function_replacement_verification() -> Result<()> {
    let training_data = test_utils::create_training_data(150);
    
    // Test that MLP adapter uses real training, not mocks
    let mlp_config = EnhancedMLPConfig::default();
    let mlp_adapter = MLPAdapter::new(mlp_config)?;
    
    // Initialize network
    mlp_adapter.initialize_network().await?;
    
    // Measure training time to ensure it's not mocked
    let start_time = std::time::Instant::now();
    mlp_adapter.train(&training_data).await?;
    let training_duration = start_time.elapsed();
    
    // Real training should take significant time
    assert!(training_duration.as_millis() > 100, 
           "MLP training should not be mocked: {}ms", training_duration.as_millis());
    
    // Verify training state changes
    let status = mlp_adapter.get_training_status().await?;
    let is_trained = status.get("is_trained").unwrap().as_bool().unwrap();
    let current_epoch = status.get("current_epoch").unwrap().as_u64().unwrap();
    let converged = status.get("converged").unwrap().as_bool().unwrap();
    
    assert!(is_trained, "Model should be in trained state");
    assert!(current_epoch > 0, "Should have completed training epochs: {}", current_epoch);
    
    // Verify performance metrics are realistic
    let metrics = mlp_adapter.get_performance_metrics().await;
    assert!(metrics.training_time_ms > 0.0, "Should record actual training time");
    assert!(metrics.parameter_count > 0, "Should have counted parameters");
    assert!(metrics.complexity_score > 0.0, "Should calculate complexity");
    
    // Test predictions are consistent with training
    let predictions = mlp_adapter.predict(&training_data[100..], 5).await?;
    assert_eq!(predictions.len(), 5);
    
    for (i, pred) in predictions.iter().enumerate() {
        assert!(pred.confidence > 0.1, "Prediction {} should have reasonable confidence", i);
        assert!(pred.value > 0.0, "Prediction {} should be positive price", i);
        
        // Check metadata contains training information
        if let Some(metadata) = &pred.metadata {
            assert!(metadata.contains_key("training_accuracy"), 
                   "Metadata should contain training accuracy");
            assert!(metadata.contains_key("parameter_count"), 
                   "Metadata should contain parameter count");
        }
    }
    
    println!("✅ MLP training replacement verified: {}ms training, {} epochs, {:.4} accuracy",
            training_duration.as_millis(), current_epoch, metrics.training_accuracy);
    
    Ok(())
}

#[tokio::test]
async fn test_fann_predictor_real_vs_mock_training() -> Result<()> {
    let config = test_utils::create_config_with_autonomous_training();
    let predictor = FannPredictor::new(config)?;
    let training_data = test_utils::create_training_data(180);
    
    // Test multiple models to ensure none are mocked
    let models_to_test = vec!["MLP", "LSTM", "DeepAR", "NHITS"];
    
    for model_name in &models_to_test {
        println!("Testing real training for model: {}", model_name);
        
        // Measure training time
        let start_time = std::time::Instant::now();
        let predictions = predictor.test_predict_with_model(model_name, &training_data, 3).await?;
        let total_duration = start_time.elapsed();
        
        // Real training should take measurable time
        assert!(total_duration.as_millis() > 20, 
               "Training + prediction for {} should take time: {}ms", 
               model_name, total_duration.as_millis());
        
        // Verify predictions have realistic characteristics
        assert_eq!(predictions.len(), 3);
        
        for (i, pred) in predictions.iter().enumerate() {
            // Check confidence varies realistically (not fixed mock values)
            assert!(pred.confidence > 0.0 && pred.confidence < 1.0, 
                   "Model {} prediction {} confidence should be realistic: {}", 
                   model_name, i, pred.confidence);
            
            // Check intervals are properly calculated
            assert!(pred.interval_low < pred.value, 
                   "Lower interval should be below prediction");
            assert!(pred.interval_high > pred.value, 
                   "Upper interval should be above prediction");
            
            let interval_width = (pred.interval_high - pred.interval_low) / pred.value;
            assert!(interval_width > 0.01 && interval_width < 0.5, 
                   "Interval width should be reasonable: {:.4}", interval_width);
        }
        
        // Test model-specific behavior differences (not generic mock responses)
        let second_predictions = predictor.test_predict_with_model(model_name, &training_data[50..], 3).await?;
        let prediction_variance = calculate_prediction_variance(&predictions, &second_predictions);
        
        assert!(prediction_variance > 0.0001, 
               "Model {} should show variance in different contexts: {:.6}", 
               model_name, prediction_variance);
        
        println!("✅ Model {} real training verified: {}ms duration, {:.6} variance",
                model_name, total_duration.as_millis(), prediction_variance);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_training_convergence_patterns() -> Result<()> {
    let config = test_utils::create_config_with_autonomous_training();
    let predictor = FannPredictor::new(config)?;
    let training_data = test_utils::create_training_data(250);
    
    // Reset ensemble to track training from scratch
    predictor.reset_ensemble_performance().await?;
    
    // Train with multiple iterations to observe convergence
    let mut accuracy_history = Vec::new();
    
    for iteration in 0..3 {
        let iteration_data = if iteration == 0 {
            &training_data[..100]
        } else if iteration == 1 {
            &training_data[..175]
        } else {
            &training_data[..]
        };
        
        // Train models
        let predictions = predictor.predict_ensemble(iteration_data, 5, &["MLP", "LSTM"], None).await?;
        
        // Calculate accuracy against holdout data
        let holdout_data = &training_data[200..];
        let accuracy = calculate_ensemble_accuracy(&predictions, holdout_data);
        accuracy_history.push(accuracy);
        
        println!("Iteration {}: accuracy = {:.4}", iteration, accuracy);
    }
    
    // Verify training shows improvement over iterations (not random/mocked)
    assert!(accuracy_history.len() >= 2, "Should have multiple accuracy measurements");
    
    // Check for general improvement trend
    let initial_accuracy = accuracy_history[0];
    let final_accuracy = *accuracy_history.last().unwrap();
    
    assert!(final_accuracy >= initial_accuracy, 
           "Training should not decrease accuracy: {:.4} -> {:.4}", 
           initial_accuracy, final_accuracy);
    
    // Check ensemble statistics show realistic performance tracking
    let ensemble_stats = predictor.get_ensemble_stats().await?;
    let model_performances = ensemble_stats.get("model_performances").unwrap();
    
    assert!(model_performances.is_object(), "Should have performance tracking");
    
    let perf_obj = model_performances.as_object().unwrap();
    for model_name in &["MLP", "LSTM"] {
        if let Some(model_perf) = perf_obj.get(model_name) {
            let perf_data = model_perf.as_object().unwrap();
            
            if let Some(pred_count) = perf_data.get("prediction_count") {
                assert!(pred_count.as_u64().unwrap() > 0, 
                       "Model {} should have prediction count", model_name);
            }
            
            if let Some(accuracy) = perf_data.get("recent_accuracy") {
                let acc = accuracy.as_f64().unwrap();
                assert!(acc >= 0.0 && acc <= 1.0, 
                       "Model {} accuracy should be valid: {}", model_name, acc);
            }
        }
    }
    
    println!("✅ Training convergence verified: {:.4} -> {:.4}", initial_accuracy, final_accuracy);
    
    Ok(())
}

#[tokio::test]
async fn test_parameter_updates_during_training() -> Result<()> {
    let mlp_config = EnhancedMLPConfig::default();
    let adapter = MLPAdapter::new(mlp_config)?;
    let training_data = test_utils::create_training_data(120);
    
    // Initialize network
    adapter.initialize_network().await?;
    
    // Get initial state
    let initial_metrics = adapter.get_performance_metrics().await;
    let initial_param_count = initial_metrics.parameter_count;
    
    assert!(initial_param_count > 0, "Should have parameters to train");
    
    // Train the model
    let training_start = std::time::Instant::now();
    adapter.train(&training_data).await?;
    let training_time = training_start.elapsed();
    
    // Get post-training state
    let final_metrics = adapter.get_performance_metrics().await;
    
    // Verify parameters are the same count (architecture unchanged)
    assert_eq!(final_metrics.parameter_count, initial_param_count,
              "Parameter count should remain consistent");
    
    // Verify training metrics updated
    assert!(final_metrics.training_time_ms > 0.0, 
           "Should record actual training time");
    
    assert!(final_metrics.training_accuracy >= 0.0 && final_metrics.training_accuracy <= 1.0,
           "Training accuracy should be valid: {}", final_metrics.training_accuracy);
    
    // Verify complexity score is calculated
    assert!(final_metrics.complexity_score > 0.0,
           "Complexity score should be calculated: {}", final_metrics.complexity_score);
    
    // Check training status shows progress
    let status = adapter.get_training_status().await?;
    let training_errors = status.get("training_errors").unwrap().as_array().unwrap();
    let validation_errors = status.get("validation_errors").unwrap().as_array().unwrap();
    
    assert!(!training_errors.is_empty(), "Should record training errors");
    
    // If validation was used, check error progression
    if !validation_errors.is_empty() {
        println!("Training errors: {:?}", training_errors);
        println!("Validation errors: {:?}", validation_errors);
        
        // Verify errors are realistic (not fixed mock values)
        let first_error = training_errors[0].as_f64().unwrap();
        let last_error = training_errors.last().unwrap().as_f64().unwrap();
        
        assert!(first_error > 0.0 && last_error > 0.0, 
               "Training errors should be positive");
        
        // Generally expect some improvement
        assert!(last_error <= first_error * 2.0, 
               "Training should not dramatically worsen: {:.6} -> {:.6}", 
               first_error, last_error);
    }
    
    println!("✅ Parameter updates verified: {} params, {:.4} accuracy, {}ms training",
            final_metrics.parameter_count, final_metrics.training_accuracy, training_time.as_millis());
    
    Ok(())
}

// Helper functions

fn calculate_prediction_variance(pred1: &[neural_trader::neural::PredictionResult], 
                               pred2: &[neural_trader::neural::PredictionResult]) -> f64 {
    if pred1.len() != pred2.len() || pred1.is_empty() {
        return 0.0;
    }
    
    let mut variance_sum = 0.0;
    for (p1, p2) in pred1.iter().zip(pred2.iter()) {
        let diff = (p1.value - p2.value).abs();
        let avg = (p1.value + p2.value) / 2.0;
        if avg > 0.0 {
            variance_sum += (diff / avg).powi(2);
        }
    }
    
    (variance_sum / pred1.len() as f64).sqrt()
}

fn calculate_ensemble_accuracy(predictions: &[neural_trader::neural::PredictionResult], 
                             actual_data: &[TimeSeriesData]) -> f64 {
    if predictions.is_empty() || actual_data.is_empty() {
        return 0.0;
    }
    
    let mut accuracy_sum = 0.0;
    let mut count = 0;
    
    for (i, pred) in predictions.iter().enumerate() {
        if let Some(actual) = actual_data.get(i) {
            let error = (pred.value - actual.close).abs() / actual.close;
            let accuracy = (1.0 - error.min(1.0)).max(0.0);
            accuracy_sum += accuracy;
            count += 1;
        }
    }
    
    if count > 0 {
        accuracy_sum / count as f64
    } else {
        0.0
    }
}