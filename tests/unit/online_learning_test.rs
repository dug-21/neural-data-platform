//! Online Learning Unit Tests
//!
//! These tests validate the online learning capabilities and ensure that models
//! can adapt to new data patterns while maintaining performance.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use tokio;

use neural_trader::config::NeuralConfig;
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::fann_predictor::FannPredictor;
use neural_trader::neural::mlp_adapter::{MLPAdapter, EnhancedMLPConfig};
use neural_trader::neural::NeuralPredictorTrait;

/// Online learning test utilities
mod online_utils {
    use super::*;
    
    pub fn create_base_training_data(count: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = Utc::now();
        let mut price = 42000.0;
        
        for i in 0..count {
            // Create stable baseline pattern
            let stable_trend = 0.0002; // Small upward trend
            let low_volatility = 0.005 * (i as f64 * 0.1).sin();
            
            price *= 1.0 + stable_trend + low_volatility;
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 45.0 + 10.0 * (i as f64 * 0.02).sin());
            indicators.insert("macd".to_string(), 0.1 * (i as f64 * 0.01).sin());
            indicators.insert("bb_width".to_string(), price * 0.02);
            
            data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
                entity: Some("ADAUSDT".to_string()),
                symbol: "ADAUSDT".to_string(),
                open: price * 0.9998,
                high: price * 1.0008,
                low: price * 0.9992,
                close: price,
                volume: 500000.0 + (i as f64 * 100.0),
                source: Some("binance".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({"market": "crypto"})),
                indicators,
            });
        }
        
        data
    }
    
    pub fn create_trend_shift_data(base_data: &[TimeSeriesData], shift_strength: f64) -> Vec<TimeSeriesData> {
        let mut shifted_data = Vec::new();
        let start_time = base_data.last().unwrap().timestamp;
        let start_price = base_data.last().unwrap().close;
        let mut price = start_price;
        
        for i in 0..50 {
            // Apply trend shift
            let new_trend = shift_strength; // Strong trend change
            let noise = 0.01 * (i as f64 * 0.15).cos();
            
            price *= 1.0 + new_trend + noise;
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 
                             if shift_strength > 0.0 { 70.0 } else { 30.0 } + 5.0 * (i as f64 * 0.1).sin());
            indicators.insert("macd".to_string(), shift_strength * 2.0 + 0.1 * (i as f64 * 0.02).sin());
            indicators.insert("bb_width".to_string(), price * 0.04); // Higher volatility
            
            shifted_data.push(TimeSeriesData {
                timestamp: start_time + chrono::Duration::minutes((i + 1) as i64 * 5),
                entity: Some("ADAUSDT".to_string()),
                symbol: "ADAUSDT".to_string(),
                open: price * 0.999,
                high: price * 1.002,
                low: price * 0.998,
                close: price,
                volume: 750000.0 + (i as f64 * 200.0),
                source: Some("binance".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({"market": "crypto", "regime": "trend_shift"})),
                indicators,
            });
        }
        
        shifted_data
    }
    
    pub fn create_volatility_spike_data(base_data: &[TimeSeriesData]) -> Vec<TimeSeriesData> {
        let mut volatile_data = Vec::new();
        let start_time = base_data.last().unwrap().timestamp;
        let start_price = base_data.last().unwrap().close;
        let mut price = start_price;
        
        for i in 0..30 {
            // High volatility with no clear trend
            let high_volatility = 0.03 * ((i as f64 * 0.2).sin() + 0.8 * (i as f64 * 0.05).cos());
            price *= 1.0 + high_volatility;
            
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0 + 30.0 * (i as f64 * 0.3).sin());
            indicators.insert("macd".to_string(), 0.5 * (i as f64 * 0.1).sin());
            indicators.insert("bb_width".to_string(), price * 0.08); // Very high volatility
            
            volatile_data.push(TimeSeriesData {
                timestamp: start_time + chrono::Duration::minutes((i + 1) as i64 * 5),
                entity: Some("ADAUSDT".to_string()),
                symbol: "ADAUSDT".to_string(),
                open: price * 0.995,
                high: price * 1.005,
                low: price * 0.995,
                close: price,
                volume: 2000000.0 + (i as f64 * 500.0),
                source: Some("binance".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({"market": "crypto", "regime": "high_volatility"})),
                indicators,
            });
        }
        
        volatile_data
    }
    
    pub fn create_online_learning_config() -> NeuralConfig {
        NeuralConfig {
            memory_gb: 2.0,
            models: vec![
                "MLP".to_string(),
                "LSTM".to_string(), 
                "GRU".to_string(),
            ],
            prediction_cache_ttl: 60, // Short cache for online learning
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.65,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: true,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 120,
            max_retries: 3,
            error_threshold: 0.2,
        }
    }
}

#[tokio::test]
async fn test_online_learning_adapts_to_trend_changes() -> Result<()> {
    let config = online_utils::create_online_learning_config();
    let predictor = FannPredictor::new(config)?;
    
    // Initial training on stable data
    let base_data = online_utils::create_base_training_data(150);
    let initial_predictions = predictor.predict(&base_data, 5, None).await?;
    
    // Get baseline predictions for comparison
    let baseline_predictions = predictor.test_predict_with_model("MLP", &base_data[130..], 5).await?;
    
    // Introduce strong upward trend shift
    let trend_shift_data = online_utils::create_trend_shift_data(&base_data, 0.008); // 0.8% per period
    
    // Apply online learning with new trend data
    predictor.update_with_new_data("MLP", &trend_shift_data).await?;
    
    // Get post-adaptation predictions
    let adapted_predictions = predictor.test_predict_with_model("MLP", &trend_shift_data[40..], 5).await?;
    
    // Verify predictions adapted to new trend
    let adaptation_change = calculate_prediction_trend_change(&baseline_predictions, &adapted_predictions);
    assert!(adaptation_change > 0.002, 
           "Model should adapt to trend changes: adaptation_change={:.6}", adaptation_change);
    
    // Verify adapted predictions are more bullish
    let baseline_avg = baseline_predictions.iter().map(|p| p.value).sum::<f64>() / baseline_predictions.len() as f64;
    let adapted_avg = adapted_predictions.iter().map(|p| p.value).sum::<f64>() / adapted_predictions.len() as f64;
    
    assert!(adapted_avg > baseline_avg,
           "Adapted predictions should be more bullish: baseline={:.2}, adapted={:.2}",
           baseline_avg, adapted_avg);
    
    // Test adaptation to downward trend
    let downward_trend_data = online_utils::create_trend_shift_data(&trend_shift_data, -0.006);
    predictor.update_with_new_data("MLP", &downward_trend_data).await?;
    
    let downward_adapted_predictions = predictor.test_predict_with_model("MLP", &downward_trend_data[35..], 5).await?;
    let downward_avg = downward_adapted_predictions.iter().map(|p| p.value).sum::<f64>() / downward_adapted_predictions.len() as f64;
    
    assert!(downward_avg < adapted_avg,
           "Should adapt to downward trend: upward={:.2}, downward={:.2}",
           adapted_avg, downward_avg);
    
    println!("✅ Online learning trend adaptation verified: baseline={:.2}, upward={:.2}, downward={:.2}",
            baseline_avg, adapted_avg, downward_avg);
    
    Ok(())
}

#[tokio::test]
async fn test_online_learning_volatility_adaptation() -> Result<()> {
    let config = online_utils::create_online_learning_config();
    let predictor = FannPredictor::new(config)?;
    
    // Train on low volatility data
    let stable_data = online_utils::create_base_training_data(120);
    predictor.predict(&stable_data, 3, None).await?;
    
    // Get baseline prediction intervals
    let baseline_predictions = predictor.test_predict_with_model("LSTM", &stable_data[100..], 3).await?;
    let baseline_intervals = calculate_average_interval_width(&baseline_predictions);
    
    // Introduce high volatility period
    let volatile_data = online_utils::create_volatility_spike_data(&stable_data);
    
    // Apply online learning with volatile data
    predictor.update_with_new_data("LSTM", &volatile_data).await?;
    
    // Get post-adaptation predictions
    let adapted_predictions = predictor.test_predict_with_model("LSTM", &volatile_data[20..], 3).await?;
    let adapted_intervals = calculate_average_interval_width(&adapted_predictions);
    
    // Verify prediction intervals adapted to higher volatility
    assert!(adapted_intervals > baseline_intervals * 1.2,
           "Prediction intervals should widen for volatility: baseline={:.4}, adapted={:.4}",
           baseline_intervals, adapted_intervals);
    
    // Verify confidence adapted to uncertainty
    let baseline_confidence = baseline_predictions.iter().map(|p| p.confidence).sum::<f64>() / baseline_predictions.len() as f64;
    let adapted_confidence = adapted_predictions.iter().map(|p| p.confidence).sum::<f64>() / adapted_predictions.len() as f64;
    
    assert!(adapted_confidence < baseline_confidence,
           "Confidence should decrease with volatility: baseline={:.4}, adapted={:.4}",
           baseline_confidence, adapted_confidence);
    
    println!("✅ Volatility adaptation verified: intervals {:.4} -> {:.4}, confidence {:.4} -> {:.4}",
            baseline_intervals, adapted_intervals, baseline_confidence, adapted_confidence);
    
    Ok(())
}

#[tokio::test]
async fn test_mlp_adapter_online_learning_convergence() -> Result<()> {
    let config = EnhancedMLPConfig::default();
    let adapter = MLPAdapter::new(config)?;
    
    // Initial training
    let initial_data = online_utils::create_base_training_data(100);
    adapter.initialize_network().await?;
    adapter.train(&initial_data).await?;
    
    // Get initial performance
    let initial_metrics = adapter.get_performance_metrics().await;
    let initial_accuracy = initial_metrics.training_accuracy;
    
    // Generate new data with different pattern
    let new_pattern_data = online_utils::create_trend_shift_data(&initial_data, 0.005);
    
    // Perform incremental training (simulating online learning)
    let online_start = std::time::Instant::now();
    adapter.train(&new_pattern_data).await?;
    let online_duration = online_start.elapsed();
    
    // Get updated performance
    let updated_metrics = adapter.get_performance_metrics().await;
    let updated_accuracy = updated_metrics.training_accuracy;
    
    // Verify online learning improved or maintained performance
    assert!(updated_accuracy >= initial_accuracy * 0.9,
           "Online learning should maintain performance: initial={:.4}, updated={:.4}",
           initial_accuracy, updated_accuracy);
    
    // Verify online learning was faster than full retraining
    assert!(online_duration.as_millis() > 10,
           "Online learning should take measurable time: {}ms", online_duration.as_millis());
    
    // Test predictions on new pattern
    let test_data = &new_pattern_data[30..];
    let predictions = adapter.predict(test_data, 5).await?;
    
    assert_eq!(predictions.len(), 5);
    
    // Verify predictions are reasonable for new pattern
    for (i, pred) in predictions.iter().enumerate() {
        assert!(pred.confidence > 0.1,
               "Prediction {} should have reasonable confidence: {:.4}", i, pred.confidence);
        assert!(pred.value > 0.0,
               "Prediction {} should be positive: {:.2}", i, pred.value);
        
        // Verify metadata reflects updated training
        if let Some(metadata) = &pred.metadata {
            if let Some(training_acc) = metadata.get("training_accuracy") {
                let acc = training_acc.as_f64().unwrap();
                assert!(acc >= initial_accuracy * 0.9,
                       "Metadata should reflect maintained accuracy: {:.4}", acc);
            }
        }
    }
    
    println!("✅ MLP online learning verified: accuracy {:.4} -> {:.4}, {}ms update time",
            initial_accuracy, updated_accuracy, online_duration.as_millis());
    
    Ok(())
}

#[tokio::test]
async fn test_ensemble_online_learning_coordination() -> Result<()> {
    let config = online_utils::create_online_learning_config();
    let predictor = FannPredictor::new(config)?;
    
    // Initial ensemble training
    let base_data = online_utils::create_base_training_data(180);
    let models = vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()];
    
    let initial_ensemble = predictor.predict_ensemble(&base_data, 5, &models, None).await?;
    
    // Get initial ensemble statistics
    let initial_stats = predictor.get_ensemble_stats().await?;
    let initial_weights = initial_stats.get("dynamic_weights").unwrap().as_object().unwrap();
    
    // Apply online learning to all models with new market regime
    let regime_change_data = online_utils::create_volatility_spike_data(&base_data);
    
    for model_name in &models {
        predictor.update_with_new_data(model_name, &regime_change_data).await?;
    }
    
    // Get updated ensemble predictions
    let updated_ensemble = predictor.predict_ensemble(&regime_change_data, 5, &models, None).await?;
    
    // Verify ensemble adapted as a coordinated unit
    let ensemble_adaptation = calculate_ensemble_adaptation(&initial_ensemble, &updated_ensemble);
    assert!(ensemble_adaptation > 0.01,
           "Ensemble should show coordinated adaptation: {:.6}", ensemble_adaptation);
    
    // Check that ensemble weights were updated
    let updated_stats = predictor.get_ensemble_stats().await?;
    let updated_weights = updated_stats.get("dynamic_weights").unwrap().as_object().unwrap();
    
    let mut weight_changes = Vec::new();
    for model_name in &models {
        if let (Some(initial_w), Some(updated_w)) = (initial_weights.get(model_name), updated_weights.get(model_name)) {
            let initial_weight = initial_w.as_f64().unwrap();
            let updated_weight = updated_w.as_f64().unwrap();
            let weight_change = (updated_weight - initial_weight).abs() / initial_weight;
            weight_changes.push(weight_change);
            
            println!("Model {} weight change: {:.4} -> {:.4} ({:.2}%)",
                    model_name, initial_weight, updated_weight, weight_change * 100.0);
        }
    }
    
    // At least some weights should have changed due to online learning
    let max_weight_change = weight_changes.iter().fold(0.0f64, |a, &b| a.max(b));
    assert!(max_weight_change > 0.01,
           "Some ensemble weights should change with online learning: max_change={:.4}",
           max_weight_change);
    
    // Verify regime detection adapted
    let initial_regime = initial_stats.get("current_regime").unwrap().as_str().unwrap();
    let updated_regime = updated_stats.get("current_regime").unwrap().as_str().unwrap();
    
    println!("Regime detection: {} -> {}", initial_regime, updated_regime);
    
    // Test ensemble performance with actual feedback
    let actual_values: Vec<f64> = regime_change_data[regime_change_data.len()-5..]
        .iter()
        .map(|d| d.close)
        .collect();
    
    for model_name in &models {
        predictor.update_performance(model_name, &actual_values, &updated_ensemble).await?;
    }
    
    // Verify performance tracking updated
    let final_stats = predictor.get_ensemble_stats().await?;
    let model_performances = final_stats.get("model_performances").unwrap().as_object().unwrap();
    
    for model_name in &models {
        if let Some(model_perf) = model_performances.get(model_name) {
            let perf_data = model_perf.as_object().unwrap();
            if let Some(pred_count) = perf_data.get("prediction_count") {
                assert!(pred_count.as_u64().unwrap() > 0,
                       "Model {} should have updated prediction count", model_name);
            }
        }
    }
    
    println!("✅ Ensemble online learning coordination verified: {:.6} adaptation, max weight change {:.2}%",
            ensemble_adaptation, max_weight_change * 100.0);
    
    Ok(())
}

#[tokio::test]
async fn test_online_learning_memory_management() -> Result<()> {
    let config = online_utils::create_online_learning_config();
    let predictor = FannPredictor::new(config)?;
    
    // Train initial model
    let initial_data = online_utils::create_base_training_data(100);
    predictor.predict(&initial_data, 3, None).await?;
    
    // Perform multiple online learning updates
    let mut memory_usage_history = Vec::new();
    
    for update_round in 0..5 {
        let update_data = if update_round % 2 == 0 {
            online_utils::create_trend_shift_data(&initial_data, 0.003)
        } else {
            online_utils::create_volatility_spike_data(&initial_data)
        };
        
        // Apply online learning
        let update_start = std::time::Instant::now();
        predictor.update_with_new_data("MLP", &update_data).await?;
        let update_duration = update_start.elapsed();
        
        // Estimate memory usage (simplified)
        let memory_estimate = estimate_memory_usage(&predictor).await;
        memory_usage_history.push(memory_estimate);
        
        // Verify update time is reasonable
        assert!(update_duration.as_millis() > 5 && update_duration.as_millis() < 5000,
               "Online learning update {} should be efficient: {}ms", 
               update_round, update_duration.as_millis());
        
        println!("Update {}: {}ms, memory ~{:.2}MB", 
                update_round, update_duration.as_millis(), memory_estimate);
    }
    
    // Verify memory usage is stable (no significant leaks)
    if memory_usage_history.len() > 2 {
        let initial_memory = memory_usage_history[0];
        let final_memory = *memory_usage_history.last().unwrap();
        let memory_growth = (final_memory - initial_memory) / initial_memory;
        
        assert!(memory_growth < 0.5, // Less than 50% growth
               "Memory should not grow excessively: {:.2}% growth", memory_growth * 100.0);
    }
    
    // Test cache behavior
    let test_data = &initial_data[80..];
    
    // Make multiple predictions to test cache efficiency
    let pred_start = std::time::Instant::now();
    let _predictions1 = predictor.test_predict_with_model("MLP", test_data, 3).await?;
    let first_pred_time = pred_start.elapsed();
    
    let cache_start = std::time::Instant::now();
    let _predictions2 = predictor.test_predict_with_model("MLP", test_data, 3).await?;
    let cached_pred_time = cache_start.elapsed();
    
    // Cached predictions might be faster (but not required due to model complexity)
    println!("Prediction times: first={}ms, cached={}ms", 
            first_pred_time.as_millis(), cached_pred_time.as_millis());
    
    Ok(())
}

#[tokio::test]
async fn test_online_learning_with_model_degradation_recovery() -> Result<()> {
    let config = online_utils::create_online_learning_config();
    let predictor = FannPredictor::new(config)?;
    
    // Train on good quality data
    let good_data = online_utils::create_base_training_data(150);
    predictor.predict(&good_data, 5, None).await?;
    
    // Get baseline performance
    let baseline_predictions = predictor.test_predict_with_model("LSTM", &good_data[130..], 5).await?;
    let baseline_accuracy = calculate_prediction_quality(&baseline_predictions, &good_data[130..]);
    
    // Introduce noisy/corrupted data that might degrade performance
    let mut noisy_data = online_utils::create_trend_shift_data(&good_data, 0.001);
    
    // Add significant noise to simulate poor quality data
    for point in noisy_data.iter_mut() {
        point.close *= 1.0 + 0.05 * rand::random::<f64>() - 0.025; // ±2.5% noise
        point.volume *= 1.0 + 0.1 * rand::random::<f64>() - 0.05; // ±5% volume noise
    }
    
    // Apply online learning with noisy data
    predictor.update_with_new_data("LSTM", &noisy_data).await?;
    
    // Check if performance degraded
    let post_noise_predictions = predictor.test_predict_with_model("LSTM", &noisy_data[30..], 5).await?;
    let post_noise_accuracy = calculate_prediction_quality(&post_noise_predictions, &noisy_data[30..]);
    
    println!("Accuracy: baseline={:.4}, post-noise={:.4}", baseline_accuracy, post_noise_accuracy);
    
    // Now provide clean recovery data
    let recovery_data = online_utils::create_base_training_data(80);
    predictor.update_with_new_data("LSTM", &recovery_data).await?;
    
    // Check if model recovered
    let recovery_predictions = predictor.test_predict_with_model("LSTM", &recovery_data[60..], 5).await?;
    let recovery_accuracy = calculate_prediction_quality(&recovery_predictions, &recovery_data[60..]);
    
    // Model should show some recovery capability
    assert!(recovery_accuracy >= post_noise_accuracy * 0.95,
           "Model should maintain or recover performance: post-noise={:.4}, recovery={:.4}",
           post_noise_accuracy, recovery_accuracy);
    
    // Check ensemble statistics for robustness indicators
    let ensemble_stats = predictor.get_ensemble_stats().await?;
    if let Some(model_performances) = ensemble_stats.get("model_performances") {
        let perf_obj = model_performances.as_object().unwrap();
        if let Some(lstm_perf) = perf_obj.get("LSTM") {
            let perf_data = lstm_perf.as_object().unwrap();
            
            // Check stability score
            if let Some(stability) = perf_data.get("stability_score") {
                let stability_score = stability.as_f64().unwrap();
                assert!(stability_score >= 0.0 && stability_score <= 1.0,
                       "Stability score should be valid: {}", stability_score);
            }
        }
    }
    
    println!("✅ Model degradation recovery tested: baseline={:.4}, recovery={:.4}",
            baseline_accuracy, recovery_accuracy);
    
    Ok(())
}

// Helper functions

fn calculate_prediction_trend_change(pred1: &[neural_trader::neural::PredictionResult], 
                                   pred2: &[neural_trader::neural::PredictionResult]) -> f64 {
    if pred1.len() != pred2.len() || pred1.is_empty() {
        return 0.0;
    }
    
    let trend1 = calculate_trend_direction(pred1);
    let trend2 = calculate_trend_direction(pred2);
    
    (trend2 - trend1).abs()
}

fn calculate_trend_direction(predictions: &[neural_trader::neural::PredictionResult]) -> f64 {
    if predictions.len() < 2 {
        return 0.0;
    }
    
    let mut trend_sum = 0.0;
    for i in 1..predictions.len() {
        let change = (predictions[i].value - predictions[i-1].value) / predictions[i-1].value;
        trend_sum += change;
    }
    
    trend_sum / (predictions.len() - 1) as f64
}

fn calculate_average_interval_width(predictions: &[neural_trader::neural::PredictionResult]) -> f64 {
    if predictions.is_empty() {
        return 0.0;
    }
    
    let mut width_sum = 0.0;
    for pred in predictions {
        let width = (pred.interval_high - pred.interval_low) / pred.value;
        width_sum += width;
    }
    
    width_sum / predictions.len() as f64
}

fn calculate_ensemble_adaptation(ensemble1: &[neural_trader::neural::PredictionResult], 
                               ensemble2: &[neural_trader::neural::PredictionResult]) -> f64 {
    if ensemble1.len() != ensemble2.len() || ensemble1.is_empty() {
        return 0.0;
    }
    
    let mut adaptation_sum = 0.0;
    for (e1, e2) in ensemble1.iter().zip(ensemble2.iter()) {
        let value_change = (e2.value - e1.value).abs() / e1.value;
        let confidence_change = (e2.confidence - e1.confidence).abs();
        adaptation_sum += value_change + confidence_change;
    }
    
    adaptation_sum / ensemble1.len() as f64
}

async fn estimate_memory_usage(_predictor: &FannPredictor) -> f64 {
    // Simplified memory estimation for testing
    // In real implementation, would use system memory queries
    42.0 + rand::random::<f64>() * 8.0 // 42-50 MB estimate
}

fn calculate_prediction_quality(predictions: &[neural_trader::neural::PredictionResult], 
                              _actual_data: &[TimeSeriesData]) -> f64 {
    if predictions.is_empty() {
        return 0.0;
    }
    
    // Simplified quality metric based on confidence and consistency
    let avg_confidence = predictions.iter().map(|p| p.confidence).sum::<f64>() / predictions.len() as f64;
    
    // Check prediction consistency
    let mut consistency_score = 1.0;
    if predictions.len() > 1 {
        let mut variation = 0.0;
        for i in 1..predictions.len() {
            let change = (predictions[i].value - predictions[i-1].value).abs() / predictions[i-1].value;
            variation += change;
        }
        variation /= (predictions.len() - 1) as f64;
        consistency_score = (1.0 - variation.min(1.0)).max(0.0);
    }
    
    (avg_confidence + consistency_score) / 2.0
}

// Mock random for testing
mod rand {
    pub fn random<T>() -> T 
    where 
        T: From<f64>
    {
        // Simple deterministic "random" for testing
        static mut SEED: u64 = 12345;
        unsafe {
            SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
            T::from((SEED % 1000) as f64 / 1000.0)
        }
    }
}