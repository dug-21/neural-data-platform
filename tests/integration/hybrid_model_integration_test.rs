//! Integration tests for hybrid FANN/real model functionality
//!
//! These tests verify that the FannPredictor properly integrates with both
//! FANN neural networks and real neuro-divergent models seamlessly.

use std::collections::HashMap;
use chrono::{Duration, Utc};
use anyhow::Result;

use neural_trader::config::NeuralConfig;
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::fann_predictor::FannPredictor;
use neural_trader::neural::NeuralPredictorTrait;

/// Create test data for integration testing
fn create_test_data(count: usize, symbol: &str) -> Vec<TimeSeriesData> {
    let base_time = Utc::now();
    let mut data = Vec::new();
    
    for i in 0..count {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.1).sin() * 30.0);
        indicators.insert("macd".to_string(), 0.001 * (i as f64 * 0.05).cos());
        indicators.insert("bb_upper".to_string(), 105.0 + (i as f64 * 0.02).sin() * 5.0);
        indicators.insert("bb_lower".to_string(), 95.0 + (i as f64 * 0.02).sin() * 5.0);
        
        data.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: base_time + Duration::minutes(i as i64),
            open: 100.0 + (i as f64 * 0.1).sin() * 10.0,
            high: 102.0 + (i as f64 * 0.1).sin() * 10.0,
            low: 98.0 + (i as f64 * 0.1).sin() * 10.0,
            close: 100.0 + (i as f64 * 0.1).sin() * 10.0 + (i as f64 * 0.01),
            volume: 1000000.0 + (i as f64 * 100.0),
            indicators,
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: None,
            metadata: None,
        });
    }
    
    data
}

#[tokio::test]
async fn test_fann_only_configuration() -> Result<()> {
    // Test FANN-only configuration (use_real_models = false)
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "LSTM".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false, // FANN only
    };
    
    let predictor = FannPredictor::new(config)?;
    
    // Should not have neuro-divergent adapters
    assert!(!predictor.has_neuro_divergent_adapter());
    
    // Test prediction with FANN models
    let test_data = create_test_data(150, "BTC/USD");
    let predictions = predictor.predict(&test_data, 5, None).await?;
    
    assert_eq!(predictions.len(), 5);
    assert!(predictions.iter().all(|p| !p.model_name.contains("enhanced")));
    assert!(predictions.iter().all(|p| !p.model_name.contains("real")));
    
    println!("✅ FANN-only configuration test passed");
    Ok(())
}

#[tokio::test]
async fn test_hybrid_configuration() -> Result<()> {
    // Test hybrid configuration (use_real_models = true)
    let config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["TimeMixer".to_string(), "DeepAR".to_string(), "MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true, // Hybrid mode
    };
    
    let predictor = FannPredictor::new(config)?;
    
    // Should have neuro-divergent adapters
    assert!(predictor.has_neuro_divergent_adapter());
    
    // Initialize the enhanced adapter
    predictor.init_enhanced_adapter().await?;
    
    // Test status
    let status = predictor.get_enhanced_adapter_status().await;
    assert!(status.is_some());
    assert!(status.unwrap().contains("Connected: true"));
    
    println!("✅ Hybrid configuration test passed");
    Ok(())
}

#[tokio::test]
async fn test_model_routing_logic() -> Result<()> {
    let config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["TimeMixer".to_string(), "NHITS".to_string(), "MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
    };
    
    let predictor = FannPredictor::new(config)?;
    predictor.init_enhanced_adapter().await?;
    
    let test_data = create_test_data(150, "ETH/USD");
    
    // Test TimeMixer (should use enhanced adapter)
    let timemixer_predictions = predictor.test_predict_with_model("TimeMixer", &test_data, 6).await?;
    assert_eq!(timemixer_predictions.len(), 6);
    assert!(timemixer_predictions[0].model_name.contains("enhanced"));
    assert!(timemixer_predictions[0].confidence > 0.9); // Enhanced models have higher confidence
    
    // Test MLP (should use FANN)
    let mlp_predictions = predictor.test_predict_with_model("MLP", &test_data, 6).await?;
    assert_eq!(mlp_predictions.len(), 6);
    assert!(!mlp_predictions[0].model_name.contains("enhanced"));
    assert!(!mlp_predictions[0].model_name.contains("real"));
    
    // Test NHITS (should use enhanced adapter)
    let nhits_predictions = predictor.test_predict_with_model("NHITS", &test_data, 6).await?;
    assert_eq!(nhits_predictions.len(), 6);
    assert!(nhits_predictions[0].model_name.contains("enhanced"));
    
    println!("✅ Model routing logic test passed");
    Ok(())
}

#[tokio::test]
async fn test_ensemble_hybrid_functionality() -> Result<()> {
    let config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["TimeMixer".to_string(), "DeepAR".to_string(), "MLP".to_string(), "LSTM".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
    };
    
    let predictor = FannPredictor::new(config)?;
    predictor.init_enhanced_adapter().await?;
    
    let test_data = create_test_data(200, "ADA/USD");
    let models = vec!["TimeMixer".to_string(), "DeepAR".to_string(), "MLP".to_string(), "LSTM".to_string()];
    
    // Test ensemble prediction with hybrid models
    let ensemble_predictions = predictor.predict_ensemble(&test_data, 8, &models, None).await?;
    
    assert_eq!(ensemble_predictions.len(), 8);
    
    // Should be a hybrid ensemble
    assert!(ensemble_predictions[0].model_name.contains("hybrid_ensemble"));
    
    // Parse ensemble composition from model name
    let model_name = &ensemble_predictions[0].model_name;
    assert!(model_name.contains("E:")); // Enhanced models
    assert!(model_name.contains("F:")); // FANN models
    
    // Hybrid ensemble should have higher confidence due to enhanced models
    assert!(ensemble_predictions[0].confidence > 0.85);
    
    println!("✅ Ensemble hybrid functionality test passed");
    Ok(())
}

#[tokio::test]
async fn test_enhanced_model_specific_predictions() -> Result<()> {
    let config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["TimeMixer".to_string(), "NeuralForecast".to_string(), "TimesFM".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
    };
    
    let predictor = FannPredictor::new(config)?;
    predictor.init_enhanced_adapter().await?;
    
    let test_data = create_test_data(150, "DOT/USD");
    
    // Test different enhanced models
    let models_to_test = vec!["TimeMixer", "NeuralForecast", "TimesFM"];
    let expected_confidence_order = vec![0.95, 0.93, 0.91]; // TimeMixer highest, TimesFM lowest
    
    for (i, model_name) in models_to_test.iter().enumerate() {
        let predictions = predictor.test_predict_with_enhanced_model(model_name, &test_data, 5).await?;
        
        assert_eq!(predictions.len(), 5);
        assert!(predictions[0].model_name.contains("enhanced"));
        assert!(predictions[0].confidence >= expected_confidence_order[i] - 0.05); // Allow some tolerance
        
        // Enhanced models should have tighter prediction intervals
        let interval_width = (predictions[0].interval_high - predictions[0].interval_low) / predictions[0].value;
        assert!(interval_width < 0.2); // Less than 20% interval width
    }
    
    println!("✅ Enhanced model specific predictions test passed");
    Ok(())
}

#[tokio::test]
async fn test_backward_compatibility() -> Result<()> {
    // Test that existing FANN-only code still works unchanged
    let old_style_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["DeepAR".to_string(), "LSTM".to_string(), "GRU".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false, // Old behavior: FANN only
    };
    
    let predictor = FannPredictor::new(old_style_config)?;
    let test_data = create_test_data(120, "LTC/USD");
    
    // Standard NeuralPredictorTrait interface should work
    let predictions = predictor.predict(&test_data, 5, None).await?;
    assert_eq!(predictions.len(), 5);
    
    // Ensemble prediction should work
    let models = vec!["DeepAR".to_string(), "LSTM".to_string()];
    let ensemble_predictions = predictor.predict_ensemble(&test_data, 5, &models, None).await?;
    assert_eq!(ensemble_predictions.len(), 5);
    assert!(ensemble_predictions[0].model_name.contains("fann_ensemble"));
    
    // Feature importance should work
    let feature_importance = predictor.get_feature_importance().await?;
    assert!(!feature_importance.is_empty());
    
    println!("✅ Backward compatibility test passed");
    Ok(())
}

#[tokio::test]
async fn test_configuration_edge_cases() -> Result<()> {
    // Test with empty models list
    let empty_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec![], // Empty models
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
    };
    
    let predictor = FannPredictor::new(empty_config)?;
    let test_data = create_test_data(50, "XRP/USD");
    
    // Should handle empty models gracefully
    let result = predictor.predict(&test_data, 5, None).await;
    assert!(result.is_err()); // Should fail gracefully with proper error
    
    // Test with mixed supported/unsupported models
    let mixed_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["TimeMixer".to_string(), "UnsupportedModel".to_string(), "MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
    };
    
    let mixed_predictor = FannPredictor::new(mixed_config)?;
    mixed_predictor.init_enhanced_adapter().await?;
    
    // Should handle mixed models correctly
    let mixed_models = vec!["TimeMixer".to_string(), "UnsupportedModel".to_string(), "MLP".to_string()];
    let ensemble_result = mixed_predictor.predict_ensemble(&test_data, 5, &mixed_models, None).await;
    
    // Should work with supported models and gracefully handle unsupported ones
    assert!(ensemble_result.is_ok());
    let predictions = ensemble_result?;
    assert_eq!(predictions.len(), 5);
    
    println!("✅ Configuration edge cases test passed");
    Ok(())
}

#[tokio::test]
async fn test_performance_and_caching() -> Result<()> {
    let config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["TimeMixer".to_string(), "DeepAR".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
    };
    
    let predictor = FannPredictor::new(config)?;
    predictor.init_enhanced_adapter().await?;
    
    let test_data = create_test_data(100, "ATOM/USD");
    
    // First prediction (cold)
    let start_time = std::time::Instant::now();
    let first_predictions = predictor.test_predict_with_model("TimeMixer", &test_data, 5).await?;
    let first_duration = start_time.elapsed();
    
    // Second prediction (should be cached)
    let start_time = std::time::Instant::now();
    let second_predictions = predictor.test_predict_with_model("TimeMixer", &test_data, 5).await?;
    let second_duration = start_time.elapsed();
    
    // Verify predictions are the same (cached)
    assert_eq!(first_predictions.len(), second_predictions.len());
    for (first, second) in first_predictions.iter().zip(second_predictions.iter()) {
        assert!((first.value - second.value).abs() < 0.001); // Should be identical from cache
    }
    
    // Second call should be faster due to caching
    assert!(second_duration < first_duration);
    
    println!("✅ Performance and caching test passed");
    println!("First call: {:?}, Second call: {:?}", first_duration, second_duration);
    Ok(())
}

/// Integration test runner
#[tokio::test]
async fn run_all_integration_tests() -> Result<()> {
    println!("🚀 Running comprehensive hybrid model integration tests...\n");
    
    // Run tests in sequence to avoid resource conflicts
    test_fann_only_configuration().await?;
    test_hybrid_configuration().await?;
    test_model_routing_logic().await?;
    test_ensemble_hybrid_functionality().await?;
    test_enhanced_model_specific_predictions().await?;
    test_backward_compatibility().await?;
    test_configuration_edge_cases().await?;
    test_performance_and_caching().await?;
    
    println!("\n🎉 All hybrid model integration tests passed successfully!");
    println!("✅ FANN-only mode: Working");
    println!("✅ Hybrid mode: Working");
    println!("✅ Model routing: Working");
    println!("✅ Ensemble functionality: Working");
    println!("✅ Enhanced model predictions: Working");
    println!("✅ Backward compatibility: Maintained");
    println!("✅ Edge cases: Handled");
    println!("✅ Performance & caching: Optimized");
    
    Ok(())
}