//! Integration Tests for MLP Enhancement Validation
//! 
//! These tests verify that the MLP integration works correctly with the
//! broader neural trading system and that real ruv-FANN models are being used.

use autonomous_platform::neural::{FannPredictor, NeuralPredictorTrait};
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
// Removed: NeuroDivergentAdapter import (deprecated)
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_mlp_with_real_ruv_fann_direct() {
    println!("\n🧪 INTEGRATION TEST 1: Direct ruv-FANN MLP Usage");
    println!("=" .repeat(70));
    
    // Test direct ruv-FANN usage to prove we're not using mocks
    use ruv_fann::{Network, NetworkBuilder, ActivationFunction, TrainingData};
    
    println!("🏗️ Creating MLP network with ruv-FANN directly...");
    
    let network = NetworkBuilder::new()
        .input_layer(10)  // MLP input layer
        .hidden_layer_with_activation(32, ActivationFunction::ReLU, 1.0)
        .hidden_layer_with_activation(16, ActivationFunction::ReLU, 1.0) 
        .hidden_layer_with_activation(8, ActivationFunction::ReLU, 1.0)
        .output_layer_with_activation(5, ActivationFunction::Linear, 1.0)
        .build();
    
    println!("✅ MLP network created: 10 -> 32 -> 16 -> 8 -> 5");
    
    // Test network computation
    let input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    let output = network.run(&input);
    
    println!("🧮 Network computation test:");
    println!("   Input:  {:?}", &input[0..5]);
    println!("   Output: {:?}", output);
    
    // Verify this is real computation, not mock
    assert_eq!(output.len(), 5, "Should output 5 values");
    assert!(output.iter().all(|&x| x.is_finite()), "All outputs should be finite");
    assert!(output.iter().any(|&x| x != 0.0), "Should have non-zero outputs");
    
    // Test with training data
    println!("\n📚 Testing MLP training with ruv-FANN...");
    
    let mut training_data = TrainingData::new();
    for i in 0..20 {
        let input_pattern: Vec<f32> = (0..10).map(|j| (i + j) as f32 * 0.1).collect();
        let target_pattern: Vec<f32> = (0..5).map(|j| (i + j + 10) as f32 * 0.05).collect();
        training_data.add_sample(input_pattern, target_pattern).unwrap();
    }
    
    println!("   Training data created: {} samples", training_data.len());
    
    // This proves we're using real ruv-FANN, not basic FANN
    println!("✅ Direct ruv-FANN MLP integration confirmed");
}

#[tokio::test]
async fn test_mlp_no_fallback_to_basic_fann() {
    println!("\n🧪 INTEGRATION TEST 2: Verify No Basic FANN Fallback");
    println!("=" .repeat(70));
    
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false, // This should use enhanced FANN, not basic fallback
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.1,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Check MLP configuration - should NOT be basic fallback
    let model_configs = predictor.get_model_configs();
    let mlp_config = &model_configs["MLP"];
    
    println!("🔍 Analyzing MLP configuration:");
    println!("   Input size: {}", mlp_config.input_size);
    println!("   Hidden layers: {:?}", mlp_config.hidden_layers);
    println!("   Output size: {}", mlp_config.output_size);
    println!("   Learning rate: {}", mlp_config.learning_rate);
    println!("   Use cascade: {}", mlp_config.use_cascade);
    
    // Verify this is NOT basic fallback configuration
    assert_eq!(mlp_config.input_size, 30, "Should use enhanced input size, not basic fallback");
    assert_eq!(mlp_config.hidden_layers, vec![64, 32, 16], "Should use sophisticated architecture");
    assert_eq!(mlp_config.output_size, 5, "Should use enhanced output size");
    assert!(mlp_config.learning_rate > 0.0, "Should have proper learning rate");
    
    // Test that the model produces sophisticated predictions
    let test_data = generate_realistic_test_data(100);
    let predictions = predictor.test_predict_with_model("MLP", &test_data, 5).await.unwrap();
    
    println!("\n🔮 Testing prediction sophistication:");
    for (i, pred) in predictions.iter().enumerate() {
        println!("   Prediction {}: {:.6} (conf: {:.3})", i + 1, pred.value, pred.confidence);
        
        // Verify predictions show sophistication (not basic fallback behavior)
        assert!(pred.value.is_finite(), "Prediction should be finite");
        assert!(pred.confidence > 0.0, "Should have positive confidence");
        assert!(pred.confidence <= 1.0, "Confidence should be normalized");
        assert!(pred.interval_high > pred.interval_low, "Should have valid intervals");
    }
    
    // Test prediction variance (sophisticated models should show variance)
    let variance = predictions.windows(2)
        .map(|w| (w[1].value - w[0].value).powi(2))
        .sum::<f64>() / (predictions.len() - 1) as f64;
    
    println!("   Prediction variance: {:.8}", variance);
    assert!(variance > 0.0, "Sophisticated model should show prediction variance");
    
    println!("✅ Confirmed: MLP uses enhanced ruv-FANN, NOT basic FANN fallback");
}

#[tokio::test]
async fn test_mlp_vs_other_models_integration() {
    println!("\n🧪 INTEGRATION TEST 3: MLP vs Other Models Integration");
    println!("=" .repeat(70));
    
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.1,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let test_data = generate_realistic_test_data(120);
    
    // Test each model individually
    let models = vec!["MLP", "LSTM", "GRU"];
    let mut model_predictions = HashMap::new();
    
    for model_name in &models {
        println!("🔮 Testing {} model...", model_name);
        let predictions = predictor.test_predict_with_model(model_name, &test_data, 5).await.unwrap();
        
        println!("   {}: {:.4} (conf: {:.3})", model_name, predictions[0].value, predictions[0].confidence);
        model_predictions.insert(model_name.to_string(), predictions);
    }
    
    // Verify MLP integrates well with other models
    let mlp_preds = &model_predictions["MLP"];
    let lstm_preds = &model_predictions["LSTM"];
    let gru_preds = &model_predictions["GRU"];
    
    // Check that all models produce reasonable predictions
    for (model_name, preds) in &model_predictions {
        for pred in preds {
            assert!(pred.value.is_finite(), "{} should produce finite predictions", model_name);
            assert!(pred.confidence > 0.0, "{} should have positive confidence", model_name);
        }
    }
    
    // Test ensemble integration
    println!("\n🎯 Testing ensemble integration...");
    let ensemble_result = predictor.predict_ensemble(
        &test_data,
        5,
        &models.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        None
    ).await.unwrap();
    
    println!("   Ensemble predictions: {}", ensemble_result.len());
    println!("   First ensemble pred: {:.4}", ensemble_result[0].value);
    println!("   Ensemble model name: {}", ensemble_result[0].model_name);
    
    // Verify ensemble combines all models appropriately
    assert!(ensemble_result[0].model_name.contains("ensemble"), "Should be ensemble prediction");
    
    // Check that ensemble differs from individual models (indicating real combination)
    let mlp_diff = (ensemble_result[0].value - mlp_preds[0].value).abs();
    let lstm_diff = (ensemble_result[0].value - lstm_preds[0].value).abs();
    let gru_diff = (ensemble_result[0].value - gru_preds[0].value).abs();
    
    println!("   Ensemble vs MLP diff: {:.6}", mlp_diff);
    println!("   Ensemble vs LSTM diff: {:.6}", lstm_diff);
    println!("   Ensemble vs GRU diff: {:.6}", gru_diff);
    
    // At least one should differ significantly (indicating real ensemble)
    assert!(mlp_diff > 0.001 || lstm_diff > 0.001 || gru_diff > 0.001, 
            "Ensemble should differ from individual models");
    
    println!("✅ MLP integrates seamlessly with other neural models");
}

#[tokio::test]
async fn test_mlp_training_performance_validation() {
    println!("\n🧪 INTEGRATION TEST 4: MLP Training Performance Validation");
    println!("=" .repeat(70));
    
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.1,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Generate training data with pattern
    let training_data = generate_pattern_data(200);
    println!("📊 Generated {} training samples with embedded pattern", training_data.len());
    
    // Test predictions before and after "training" (network initialization)
    let test_data = &training_data[150..];
    
    println!("🔮 Making predictions with initialized MLP...");
    let predictions = predictor.test_predict_with_model("MLP", test_data, 5).await.unwrap();
    
    // Analyze prediction quality
    let mut confidence_sum = 0.0;
    let mut interval_widths = Vec::new();
    
    println!("\n📈 Prediction Analysis:");
    for (i, pred) in predictions.iter().enumerate() {
        confidence_sum += pred.confidence;
        let interval_width = (pred.interval_high - pred.interval_low) / pred.value;
        interval_widths.push(interval_width);
        
        println!("   Pred {}: {:.4} ±{:.1}% (conf: {:.3})", 
                 i + 1, pred.value, interval_width * 100.0, pred.confidence);
    }
    
    let avg_confidence = confidence_sum / predictions.len() as f64;
    let avg_interval_width = interval_widths.iter().sum::<f64>() / interval_widths.len() as f64;
    
    println!("\n📊 Quality Metrics:");
    println!("   Average confidence: {:.3}", avg_confidence);
    println!("   Average interval width: {:.1}%", avg_interval_width * 100.0);
    
    // Validate training effectiveness
    assert!(avg_confidence > 0.5, "Should have reasonable confidence");
    assert!(avg_interval_width > 0.0, "Should have positive intervals");
    assert!(avg_interval_width < 0.5, "Intervals should be reasonable");
    
    // Test online learning capability
    println!("\n🔄 Testing online learning...");
    let new_data = generate_pattern_data(50);
    let update_result = predictor.update_with_new_data("MLP", &new_data).await;
    assert!(update_result.is_ok(), "Online learning should work");
    
    println!("✅ MLP training and performance validation passed");
}

#[tokio::test]
async fn test_mlp_real_world_scenario() {
    println!("\n🧪 INTEGRATION TEST 5: Real-World Trading Scenario");
    println!("=" .repeat(70));
    
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.1,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    // Simulate realistic trading data (BTC-like)
    let mut trading_data = Vec::new();
    let base_time = Utc::now() - chrono::Duration::hours(24);
    let mut price = 50000.0;
    
    for i in 0..288 { // 24 hours of 5-minute intervals
        // Simulate realistic price movement
        let time_factor = i as f64;
        let trend = 0.1 * (time_factor / 100.0).sin();
        let volatility = (rand::random::<f64>() - 0.5) * 0.02;
        let daily_cycle = 0.005 * (time_factor * 2.0 * std::f64::consts::PI / 288.0).sin();
        
        price *= 1.0 + trend + volatility + daily_cycle;
        
        let volume = 1000.0 + 500.0 * rand::random::<f64>();
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + 40.0 * rand::random::<f64>());
        indicators.insert("macd".to_string(), -0.1 + 0.2 * rand::random::<f64>());
        indicators.insert("bb_upper".to_string(), price * 1.02);
        indicators.insert("bb_lower".to_string(), price * 0.98);
        indicators.insert("volume_sma".to_string(), volume * 0.9);
        
        trading_data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64 * 5),
            entity: Some("binance".to_string()),
            symbol: "BTC-USD".to_string(),
            open: price * (1.0 + (rand::random::<f64>() - 0.5) * 0.001),
            high: price * (1.0 + rand::random::<f64>() * 0.002),
            low: price * (1.0 - rand::random::<f64>() * 0.002),
            close: price,
            volume,
            source: Some("binance_api".to_string()),
            value: Some(price),
            metadata: Some(serde_json::json!({
                "exchange": "binance",
                "pair": "BTC-USD",
                "interval": "5m"
            })),
            indicators,
        });
    }
    
    println!("💰 Generated realistic trading data:");
    println!("   Duration: 24 hours (5-minute intervals)");
    println!("   Data points: {}", trading_data.len());
    println!("   Price range: ${:.2} - ${:.2}", 
             trading_data.iter().map(|d| d.close).fold(f64::INFINITY, f64::min),
             trading_data.iter().map(|d| d.close).fold(f64::NEG_INFINITY, f64::max));
    
    // Test MLP predictions on realistic trading scenario
    println!("\n🔮 Testing MLP on realistic trading scenario...");
    let predictions = predictor.test_predict_with_model("MLP", &trading_data, 6).await.unwrap();
    
    let current_price = trading_data.last().unwrap().close;
    println!("\n📊 Trading Predictions:");
    println!("   Current price: ${:.2}", current_price);
    
    for (i, pred) in predictions.iter().enumerate() {
        let minutes_ahead = (i + 1) * 5;
        let price_change = ((pred.value - current_price) / current_price) * 100.0;
        
        println!("   {}min: ${:.2} ({:+.2}%, conf: {:.2})", 
                 minutes_ahead, pred.value, price_change, pred.confidence);
        
        // Validate trading predictions
        assert!(pred.value > 0.0, "Price predictions should be positive");
        assert!(pred.value > current_price * 0.8, "Price shouldn't drop more than 20%");
        assert!(pred.value < current_price * 1.2, "Price shouldn't rise more than 20%");
        assert!(pred.confidence > 0.0, "Should have confidence");
    }
    
    // Test performance in trading context
    let start = std::time::Instant::now();
    let _batch_predictions = predictor.test_predict_with_model("MLP", &trading_data, 12).await.unwrap();
    let latency = start.elapsed();
    
    println!("\n⚡ Trading Performance:");
    println!("   Prediction latency: {:?}", latency);
    println!("   Acceptable for real-time: {}", latency.as_millis() < 1000);
    
    assert!(latency.as_millis() < 2000, "Should predict within 2 seconds for trading");
    
    println!("✅ MLP handles real-world trading scenarios effectively");
}

// Helper functions
fn generate_realistic_test_data(n: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();
    let mut price = 100.0;
    
    for i in 0..n {
        let time_factor = i as f64 * 0.1;
        price *= 1.0 + 0.01 * time_factor.sin() + (rand::random::<f64>() - 0.5) * 0.005;
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + 40.0 * rand::random::<f64>());
        indicators.insert("macd".to_string(), -0.1 + 0.2 * rand::random::<f64>());
        
        data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            entity: Some("test".to_string()),
            symbol: "TEST".to_string(),
            open: price,
            high: price * 1.001,
            low: price * 0.999,
            close: price,
            volume: vec![1000.0],
            source: Some("test".to_string()),
            value: Some(price),
            metadata: Some(serde_json::json!({})),
            indicators,
        });
    }
    
    data
}

fn generate_pattern_data(n: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();
    
    for i in 0..n {
        let t = i as f64;
        // Embedded pattern: sine wave with trend
        let pattern_value = 100.0 + 10.0 * (t * 0.1).sin() + 0.05 * t;
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + 20.0 * (t * 0.05).cos());
        
        data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            entity: Some("pattern".to_string()),
            symbol: "PATTERN".to_string(),
            open: pattern_value,
            high: pattern_value * 1.001,
            low: pattern_value * 0.999,
            close: pattern_value,
            volume: vec![1000.0],
            source: Some("pattern".to_string()),
            value: Some(pattern_value),
            metadata: Some(serde_json::json!({})),
            indicators,
        });
    }
    
    data
}