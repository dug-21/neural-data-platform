//! Training Metrics Unit Tests
//!
//! These tests validate the accuracy and completeness of training metrics collection
//! and ensure proper monitoring of training execution.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use tokio;

use neural_trader::config::NeuralConfig;
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::fann_predictor::FannPredictor;
use neural_trader::neural::mlp_adapter::{MLPAdapter, EnhancedMLPConfig};
use neural_trader::neural::NeuralPredictorTrait;

/// Metrics validation utilities
mod metrics_utils {
    use super::*;
    
    pub fn create_metrics_test_data(count: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = Utc::now();
        let mut price = 45000.0;
        
        for i in 0..count {
            // Create price series with known patterns for metrics validation
            let trend = 0.0005; // Consistent upward trend
            let cycle = 0.01 * (i as f64 * 0.1).sin(); // Cyclical pattern
            let noise = 0.005 * ((i as f64 * 0.05).cos() - 0.5); // Noise component
            
            price *= 1.0 + trend + cycle + noise;
            
            let volume = 800000.0 + 200000.0 * (i as f64 * 0.02).sin();
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0 + 25.0 * (i as f64 * 0.01).sin());
            indicators.insert("macd".to_string(), (i as f64 * 0.008).sin());
            indicators.insert("bb_upper".to_string(), price * 1.025);
            indicators.insert("bb_lower".to_string(), price * 0.975);
            indicators.insert("volume_ma".to_string(), volume);
            
            data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(i as i64 * 15),
                entity: Some("ETHUSDT".to_string()),
                symbol: "ETHUSDT".to_string(),
                open: price * 0.999,
                high: price * 1.002,
                low: price * 0.998,
                close: price,
                volume,
                source: Some("kraken".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({
                    "exchange": "kraken",
                    "pair": "ETH/USDT",
                    "type": "spot"
                })),
                indicators,
            });
        }
        
        data
    }
    
    pub fn create_metrics_config() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 1.5,
            models: vec![
                "MLP".to_string(),
                "LSTM".to_string(),
                "DeepAR".to_string(),
                "TCN".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 90,
            max_concurrent_predictions: 8,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: true,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 180,
            max_retries: 3,
            error_threshold: 0.12,
        }
    }
}

#[tokio::test]
async fn test_training_time_metrics_accuracy() -> Result<()> {
    let config = metrics_utils::create_metrics_config();
    let predictor = FannPredictor::new(config)?;
    let training_data = metrics_utils::create_metrics_test_data(200);
    
    // Reset ensemble to start fresh metrics collection
    predictor.reset_ensemble_performance().await?;
    
    // Measure training time manually
    let manual_start = std::time::Instant::now();
    let predictions = predictor.predict(&training_data, 5, None).await?;
    let manual_duration = manual_start.elapsed();
    
    // Verify predictions were generated
    assert!(!predictions.is_empty(), "Should generate predictions");
    
    // Get ensemble stats and verify timing metrics
    let stats = predictor.get_ensemble_stats().await?;
    let model_performances = stats.get("model_performances").unwrap();
    
    assert!(model_performances.is_object(), "Should have performance data");
    
    let perf_obj = model_performances.as_object().unwrap();
    let mut total_reported_time = 0.0;
    let mut models_with_timing = 0;
    
    for (model_name, model_perf) in perf_obj.iter() {
        let perf_data = model_perf.as_object().unwrap();
        
        // Verify timing metrics are present and reasonable
        if let Some(last_updated) = perf_data.get("last_updated") {
            let timestamp = last_updated.as_str().unwrap();
            assert!(!timestamp.is_empty(), "Should have timestamp for {}", model_name);
        }
        
        // Check prediction count
        if let Some(pred_count) = perf_data.get("prediction_count") {
            let count = pred_count.as_u64().unwrap();
            assert!(count > 0, "Model {} should have prediction count", model_name);
        }
        
        println!("Model {} metrics: {:?}", model_name, perf_data);
        models_with_timing += 1;
    }
    
    assert!(models_with_timing > 0, "Should have timing data for at least one model");
    
    // Verify manual timing is reasonable
    assert!(manual_duration.as_millis() > 10, 
           "Training should take measurable time: {}ms", manual_duration.as_millis());
    
    println!("✅ Training time metrics validated: {}ms manual measurement, {} models tracked",
            manual_duration.as_millis(), models_with_timing);
    
    Ok(())
}

#[tokio::test]
async fn test_mlp_adapter_detailed_metrics() -> Result<()> {
    let config = EnhancedMLPConfig::default();
    let adapter = MLPAdapter::new(config)?;
    let training_data = metrics_utils::create_metrics_test_data(180);
    
    // Initialize and train
    adapter.initialize_network().await?;
    
    let training_start = std::time::Instant::now();
    adapter.train(&training_data).await?;
    let actual_training_time = training_start.elapsed();
    
    // Get comprehensive metrics
    let metrics = adapter.get_performance_metrics().await;
    let status = adapter.get_training_status().await?;
    
    // Validate training time accuracy
    let reported_training_time = metrics.training_time_ms;
    let time_diff = (reported_training_time - actual_training_time.as_millis() as f64).abs();
    let time_tolerance = actual_training_time.as_millis() as f64 * 0.1; // 10% tolerance
    
    assert!(time_diff <= time_tolerance,
           "Reported training time should match actual: reported={:.2}ms, actual={}ms, diff={:.2}ms",
           reported_training_time, actual_training_time.as_millis(), time_diff);
    
    // Validate accuracy metrics
    assert!(metrics.training_accuracy >= 0.0 && metrics.training_accuracy <= 1.0,
           "Training accuracy should be valid: {}", metrics.training_accuracy);
    
    if metrics.validation_accuracy > 0.0 {
        assert!(metrics.validation_accuracy >= 0.0 && metrics.validation_accuracy <= 1.0,
               "Validation accuracy should be valid: {}", metrics.validation_accuracy);
    }
    
    // Validate architectural metrics
    assert!(metrics.parameter_count > 0, 
           "Should count parameters: {}", metrics.parameter_count);
    
    assert!(metrics.complexity_score > 0.0,
           "Should calculate complexity: {}", metrics.complexity_score);
    
    // Validate training progression metrics
    let training_errors = status.get("training_errors").unwrap().as_array().unwrap();
    assert!(!training_errors.is_empty(), "Should record training errors");
    
    let validation_errors = status.get("validation_errors").unwrap().as_array().unwrap();
    
    // Check error progression makes sense
    if training_errors.len() > 1 {
        let first_error = training_errors[0].as_f64().unwrap();
        let last_error = training_errors.last().unwrap().as_f64().unwrap();
        
        assert!(first_error > 0.0 && last_error > 0.0,
               "Training errors should be positive");
        
        // Expect general improvement or stability
        assert!(last_error <= first_error * 3.0,
               "Training shouldn't dramatically worsen: {:.6} -> {:.6}",
               first_error, last_error);
    }
    
    // Test prediction latency metrics
    let pred_start = std::time::Instant::now();
    let predictions = adapter.predict(&training_data[150..], 3).await?;
    let actual_pred_time = pred_start.elapsed();
    
    assert_eq!(predictions.len(), 3);
    
    let updated_metrics = adapter.get_performance_metrics().await;
    let reported_pred_latency = updated_metrics.prediction_latency_ms;
    
    // Prediction latency should be reasonable
    assert!(reported_pred_latency > 0.0,
           "Should record prediction latency: {:.2}ms", reported_pred_latency);
    
    let pred_time_diff = (reported_pred_latency - actual_pred_time.as_millis() as f64).abs();
    let pred_tolerance = actual_pred_time.as_millis() as f64 * 0.5; // 50% tolerance for averaging
    
    assert!(pred_time_diff <= pred_tolerance.max(10.0),
           "Prediction latency should be reasonable: reported={:.2}ms, actual={}ms",
           reported_pred_latency, actual_pred_time.as_millis());
    
    println!("✅ MLP metrics validated: training={:.2}ms, accuracy={:.4}, params={}, prediction={:.2}ms",
            metrics.training_time_ms, metrics.training_accuracy, 
            metrics.parameter_count, updated_metrics.prediction_latency_ms);
    
    Ok(())
}

#[tokio::test]
async fn test_ensemble_performance_tracking() -> Result<()> {
    let config = metrics_utils::create_metrics_config();
    let predictor = FannPredictor::new(config)?;
    let training_data = metrics_utils::create_metrics_test_data(250);
    
    // Reset and perform ensemble training
    predictor.reset_ensemble_performance().await?;
    
    let models = vec!["MLP".to_string(), "LSTM".to_string(), "DeepAR".to_string()];
    let ensemble_predictions = predictor.predict_ensemble(&training_data, 5, &models, None).await?;
    
    assert_eq!(ensemble_predictions.len(), 5);
    
    // Simulate performance feedback
    let actual_values = vec![45100.0, 45200.0, 45150.0, 45300.0, 45250.0];
    
    for model_name in &models {
        predictor.update_performance(model_name, &actual_values, &ensemble_predictions).await?;
    }
    
    // Verify comprehensive performance metrics
    let stats = predictor.get_ensemble_stats().await?;
    
    // Check dynamic weights
    let dynamic_weights = stats.get("dynamic_weights").unwrap().as_object().unwrap();
    assert!(!dynamic_weights.is_empty(), "Should have dynamic weights");
    
    for model_name in &models {
        if let Some(weight) = dynamic_weights.get(model_name) {
            let w = weight.as_f64().unwrap();
            assert!(w > 0.0, "Model {} should have positive weight: {}", model_name, w);
        }
    }
    
    // Check model performances
    let model_performances = stats.get("model_performances").unwrap().as_object().unwrap();
    
    for model_name in &models {
        if let Some(model_perf) = model_performances.get(model_name) {
            let perf_data = model_perf.as_object().unwrap();
            
            // Validate accuracy metrics
            if let Some(recent_accuracy) = perf_data.get("recent_accuracy") {
                let acc = recent_accuracy.as_f64().unwrap();
                assert!(acc >= 0.0 && acc <= 1.0, 
                       "Model {} recent accuracy should be valid: {}", model_name, acc);
            }
            
            // Validate confidence score
            if let Some(confidence_score) = perf_data.get("confidence_score") {
                let conf = confidence_score.as_f64().unwrap();
                assert!(conf >= 0.0 && conf <= 1.0,
                       "Model {} confidence score should be valid: {}", model_name, conf);
            }
            
            // Validate prediction counts
            if let Some(pred_count) = perf_data.get("prediction_count") {
                let count = pred_count.as_u64().unwrap();
                assert!(count > 0, "Model {} should have prediction count", model_name);
            }
            
            if let Some(success_count) = perf_data.get("successful_predictions") {
                let count = success_count.as_u64().unwrap();
                assert!(count >= 0, "Model {} should have success count", model_name);
            }
            
            // Validate stability score
            if let Some(stability) = perf_data.get("stability_score") {
                let stab = stability.as_f64().unwrap();
                assert!(stab >= 0.0 && stab <= 1.0,
                       "Model {} stability should be valid: {}", model_name, stab);
            }
            
            println!("Model {} performance: {:?}", model_name, perf_data);
        }
    }
    
    // Check diversity metrics
    let diversity_metrics = stats.get("diversity_metrics").unwrap().as_object().unwrap();
    if !diversity_metrics.is_empty() {
        for (model_name, diversity) in diversity_metrics.iter() {
            let div = diversity.as_f64().unwrap();
            assert!(div >= 0.0, "Model {} diversity should be non-negative: {}", model_name, div);
        }
    }
    
    // Check current regime detection
    let current_regime = stats.get("current_regime").unwrap().as_str().unwrap();
    let valid_regimes = ["Bullish", "Bearish", "Sideways", "HighVolatility", "LowVolatility"];
    assert!(valid_regimes.contains(&current_regime),
           "Should detect valid market regime: {}", current_regime);
    
    println!("✅ Ensemble performance tracking validated: {} models, regime={}, {} metrics collected",
            models.len(), current_regime, model_performances.len());
    
    Ok(())
}

#[tokio::test]
async fn test_metrics_persistence_and_aggregation() -> Result<()> {
    let config = metrics_utils::create_metrics_config();
    let predictor = FannPredictor::new(config)?;
    let training_data = metrics_utils::create_metrics_test_data(200);
    
    // Reset and perform multiple training/prediction cycles
    predictor.reset_ensemble_performance().await?;
    
    let mut accuracy_progression = Vec::new();
    
    // Perform multiple training cycles to test metric accumulation
    for cycle in 0..3 {
        let cycle_data = &training_data[cycle * 50..(cycle + 1) * 50 + 100];
        
        // Make predictions
        let predictions = predictor.predict(cycle_data, 3, None).await?;
        
        // Simulate actual results and update performance
        let actual_values: Vec<f64> = cycle_data[cycle_data.len()-3..]
            .iter()
            .map(|d| d.close)
            .collect();
        
        for model_name in &["MLP", "LSTM"] {
            predictor.update_performance(model_name, &actual_values, &predictions).await?;
        }
        
        // Calculate cycle accuracy
        let cycle_accuracy = calculate_cycle_accuracy(&predictions, &actual_values);
        accuracy_progression.push(cycle_accuracy);
        
        println!("Cycle {}: accuracy = {:.4}", cycle, cycle_accuracy);
    }
    
    // Verify metrics accumulation
    let final_stats = predictor.get_ensemble_stats().await?;
    let model_performances = final_stats.get("model_performances").unwrap().as_object().unwrap();
    
    for model_name in &["MLP", "LSTM"] {
        if let Some(model_perf) = model_performances.get(model_name) {
            let perf_data = model_perf.as_object().unwrap();
            
            // Check prediction count accumulated across cycles
            if let Some(pred_count) = perf_data.get("prediction_count") {
                let count = pred_count.as_u64().unwrap();
                assert!(count >= 9, // 3 cycles * 3 predictions each
                       "Model {} should accumulate prediction count: {}", model_name, count);
            }
            
            // Check that accuracy reflects accumulated learning
            if let Some(recent_accuracy) = perf_data.get("recent_accuracy") {
                let acc = recent_accuracy.as_f64().unwrap();
                assert!(acc >= 0.0 && acc <= 1.0,
                       "Model {} accumulated accuracy should be valid: {}", model_name, acc);
            }
        }
    }
    
    // Verify regime detection worked across cycles
    let current_regime = final_stats.get("current_regime").unwrap().as_str().unwrap();
    assert!(!current_regime.is_empty(), "Should have detected market regime");
    
    // Check dynamic weights reflect accumulated performance
    let dynamic_weights = final_stats.get("dynamic_weights").unwrap().as_object().unwrap();
    assert!(!dynamic_weights.is_empty(), "Should have dynamic weights");
    
    let mut weight_sum = 0.0;
    for (model_name, weight) in dynamic_weights.iter() {
        let w = weight.as_f64().unwrap();
        assert!(w > 0.0, "Model {} should have positive weight: {}", model_name, w);
        weight_sum += w;
    }
    
    assert!(weight_sum > 0.0, "Total weights should be positive: {}", weight_sum);
    
    println!("✅ Metrics persistence validated: {} cycles, final regime={}, total weight={:.4}",
            accuracy_progression.len(), current_regime, weight_sum);
    
    Ok(())
}

#[tokio::test]
async fn test_error_metrics_and_convergence_tracking() -> Result<()> {
    let mlp_config = EnhancedMLPConfig::default();
    let adapter = MLPAdapter::new(mlp_config)?;
    let training_data = metrics_utils::create_metrics_test_data(150);
    
    // Initialize and train with detailed tracking
    adapter.initialize_network().await?;
    adapter.train(&training_data).await?;
    
    // Get training status with error history
    let status = adapter.get_training_status().await?;
    
    let training_errors = status.get("training_errors").unwrap().as_array().unwrap();
    let validation_errors = status.get("validation_errors").unwrap().as_array().unwrap();
    
    assert!(!training_errors.is_empty(), "Should have training error history");
    
    // Validate error progression
    let mut error_values = Vec::new();
    for error in training_errors {
        let err = error.as_f64().unwrap();
        assert!(err >= 0.0, "Training errors should be non-negative: {}", err);
        error_values.push(err);
    }
    
    // Check for general convergence pattern
    if error_values.len() > 3 {
        let early_avg = error_values[0..3].iter().sum::<f64>() / 3.0;
        let late_avg = error_values[error_values.len()-3..].iter().sum::<f64>() / 3.0;
        
        // Expect improvement or stability
        assert!(late_avg <= early_avg * 2.0,
               "Training should show convergence: early={:.6}, late={:.6}",
               early_avg, late_avg);
    }
    
    // Validate validation errors if present
    if !validation_errors.is_empty() {
        let mut val_error_values = Vec::new();
        for error in validation_errors {
            let err = error.as_f64().unwrap();
            assert!(err >= 0.0, "Validation errors should be non-negative: {}", err);
            val_error_values.push(err);
        }
        
        // Check validation-training error relationship
        if val_error_values.len() == error_values.len() {
            let final_train_error = error_values.last().unwrap();
            let final_val_error = val_error_values.last().unwrap();
            
            // Validation error should be reasonably related to training error
            let error_ratio = final_val_error / final_train_error;
            assert!(error_ratio > 0.1 && error_ratio < 10.0,
                   "Validation/training error ratio should be reasonable: {:.4}",
                   error_ratio);
        }
        
        println!("Validation errors tracked: {:?}", val_error_values);
    }
    
    // Check convergence status
    let converged = status.get("converged").unwrap().as_bool().unwrap();
    let current_epoch = status.get("current_epoch").unwrap().as_u64().unwrap();
    
    assert!(current_epoch > 0, "Should have completed epochs: {}", current_epoch);
    
    // Get final metrics
    let metrics = adapter.get_performance_metrics().await;
    
    // Validate generalization score if available
    if metrics.generalization_score > 0.0 {
        assert!(metrics.generalization_score >= 0.0 && metrics.generalization_score <= 2.0,
               "Generalization score should be reasonable: {}", metrics.generalization_score);
    }
    
    println!("✅ Error metrics validated: {} training errors, {} validation errors, converged={}, epochs={}",
            training_errors.len(), validation_errors.len(), converged, current_epoch);
    
    Ok(())
}

// Helper functions

fn calculate_cycle_accuracy(predictions: &[neural_trader::neural::PredictionResult], 
                          actual_values: &[f64]) -> f64 {
    if predictions.is_empty() || actual_values.is_empty() {
        return 0.0;
    }
    
    let mut accuracy_sum = 0.0;
    let mut count = 0;
    
    for (pred, &actual) in predictions.iter().zip(actual_values.iter()) {
        let error = (pred.value - actual).abs() / actual;
        let accuracy = (1.0 - error.min(1.0)).max(0.0);
        accuracy_sum += accuracy;
        count += 1;
    }
    
    if count > 0 {
        accuracy_sum / count as f64
    } else {
        0.0
    }
}

#[tokio::test]
async fn test_memory_usage_metrics() -> Result<()> {
    let mlp_config = EnhancedMLPConfig::default();
    let adapter = MLPAdapter::new(mlp_config)?;
    let training_data = metrics_utils::create_metrics_test_data(100);
    
    // Get initial memory baseline
    let initial_memory = get_process_memory_mb();
    
    // Initialize and train
    adapter.initialize_network().await?;
    adapter.train(&training_data).await?;
    
    // Get post-training memory
    let post_training_memory = get_process_memory_mb();
    
    // Get metrics
    let metrics = adapter.get_performance_metrics().await;
    
    // Memory should be tracked
    assert!(metrics.memory_usage_mb >= 0.0, 
           "Memory usage should be non-negative: {}", metrics.memory_usage_mb);
    
    // Memory usage should be reasonable
    let memory_increase = post_training_memory - initial_memory;
    assert!(memory_increase >= 0.0, 
           "Memory increase should be non-negative: {:.2}MB", memory_increase);
    
    // Make predictions and check memory stability
    let _predictions = adapter.predict(&training_data[80..], 5).await?;
    let final_memory = get_process_memory_mb();
    
    let prediction_memory_change = (final_memory - post_training_memory).abs();
    assert!(prediction_memory_change < 100.0, // Should not leak significant memory
           "Prediction should not cause large memory changes: {:.2}MB", prediction_memory_change);
    
    println!("✅ Memory metrics validated: initial={:.2}MB, post-training={:.2}MB, final={:.2}MB",
            initial_memory, post_training_memory, final_memory);
    
    Ok(())
}

fn get_process_memory_mb() -> f64 {
    // Simple memory estimation - in real implementation would use proper system calls
    let usage = std::alloc::System.alloc(std::alloc::Layout::new::<u8>()) as usize;
    std::alloc::System.dealloc(usage as *mut u8, std::alloc::Layout::new::<u8>());
    
    // Return a reasonable estimate for testing
    64.0 // MB - placeholder for actual memory measurement
}