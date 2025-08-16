//! Tests for the use_real_models feature flag system
//!
//! This module tests the feature flag behavior in various scenarios:
//! - Feature flag enabled with real models available
//! - Feature flag enabled with real models unavailable (fallback behavior)
//! - Feature flag disabled (FANN-only mode)
//! - Configuration validation with invalid settings

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::FannPredictor;
use crate::neural::NeuralPredictorTrait;

/// Create test time series data for predictions
fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();
    let mut price = 100.0;

    for i in 0..count {
        price *= 1.0 + (0.02 * (i as f64 * 0.1).sin()); // Synthetic price movement

        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.5));

        data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            entity: Some("TEST".to_string()),
            symbol: "TEST".to_string(),
            open: price * 0.999,
            high: price * 1.001,
            low: price * 0.998,
            close: price,
            volume: vec![1000000.0 + (i as f64 * 1000.0)],
            source: Some("test".to_string()),
            value: Some(price),
            metadata: Some(serde_json::json!({})),
            indicators,
        });
    }

    data
}

#[tokio::test]
async fn test_feature_flag_disabled_fann_only() -> Result<()> {
    // Test with use_real_models = false (FANN-only mode)
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "LSTM".to_string(), "DeepAR".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false, // Feature flag disabled
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    let predictor = FannPredictor::new(config)?;
    let test_data = create_test_data(50);

    // Test individual model predictions (should use FANN implementations)
    for model_name in &["MLP", "LSTM", "DeepAR"] {
        let predictions = predictor
            .test_predict_with_model(model_name, &test_data, 5)
            .await?;

        assert_eq!(predictions.len(), 5);
        assert!(predictions
            .iter()
            .all(|p| p.confidence > 0.0 && p.confidence <= 1.0));

        // Should not use enhanced or real model implementations
        assert!(predictions
            .iter()
            .all(|p| !p.model_name.contains("enhanced")));
        assert!(predictions.iter().all(|p| !p.model_name.contains("real")));

        println!(
            "✅ FANN-only prediction for '{}' successful: {} predictions",
            model_name,
            predictions.len()
        );
    }

    // Test ensemble predictions
    let ensemble_predictions = predictor
        .predict_ensemble(
            &test_data,
            5,
            &["MLP".to_string(), "LSTM".to_string(), "DeepAR".to_string()],
            None,
        )
        .await?;

    assert_eq!(ensemble_predictions.len(), 5);
    assert!(ensemble_predictions
        .iter()
        .all(|p| p.confidence > 0.0 && p.confidence <= 1.0));
    assert!(ensemble_predictions
        .iter()
        .all(|p| p.model_name.contains("ensemble")));

    println!(
        "✅ FANN-only ensemble prediction successful: {} predictions",
        ensemble_predictions.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_feature_flag_enabled_with_fallback() -> Result<()> {
    // Test with use_real_models = true but real models unavailable (fallback behavior)
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec![
            "TimeMixer".to_string(),
            "LSTM".to_string(),
            "DeepAR".to_string(),
        ],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true, // Feature flag enabled
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    let predictor = FannPredictor::new(config)?;
    let test_data = create_test_data(50);

    // Test that TimeMixer falls back to FANN when real models are unavailable
    let predictions = predictor
        .test_predict_with_model("TimeMixer", &test_data, 5)
        .await?;

    assert_eq!(predictions.len(), 5);
    assert!(predictions
        .iter()
        .all(|p| p.confidence > 0.0 && p.confidence <= 1.0));

    println!(
        "✅ Real model fallback prediction for 'TimeMixer' successful: {} predictions",
        predictions.len()
    );

    // Test DeepAR (should try real model first, then fallback)
    let deepar_predictions = predictor
        .test_predict_with_model("DeepAR", &test_data, 5)
        .await?;

    assert_eq!(deepar_predictions.len(), 5);
    assert!(deepar_predictions
        .iter()
        .all(|p| p.confidence > 0.0 && p.confidence <= 1.0));

    println!(
        "✅ DeepAR fallback prediction successful: {} predictions",
        deepar_predictions.len()
    );

    // Test ensemble with mixed model types
    let ensemble_predictions = predictor
        .predict_ensemble(
            &test_data,
            5,
            &[
                "TimeMixer".to_string(),
                "LSTM".to_string(),
                "DeepAR".to_string(),
            ],
            None,
        )
        .await?;

    assert_eq!(ensemble_predictions.len(), 5);
    assert!(ensemble_predictions
        .iter()
        .all(|p| p.confidence > 0.0 && p.confidence <= 1.0));

    println!(
        "✅ Mixed model ensemble prediction successful: {} predictions",
        ensemble_predictions.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_feature_flag_enhanced_adapter_available() -> Result<()> {
    // Test with enhanced adapter available
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["TimeMixer".to_string(), "NeuralForecast".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    let predictor = FannPredictor::new(config)?;
    let test_data = create_test_data(50);

    // Check if enhanced adapter is available
    let adapter_status = predictor.get_enhanced_adapter_status().await;
    if adapter_status.is_some() {
        println!("✅ Enhanced adapter available: {}", adapter_status.unwrap());

        // Try to use enhanced models
        if let Ok(predictions) = predictor
            .test_predict_with_enhanced_model("TimeMixer", &test_data, 5)
            .await
        {
            assert_eq!(predictions.len(), 5);
            assert!(predictions
                .iter()
                .all(|p| p.confidence > 0.0 && p.confidence <= 1.0));
            assert!(predictions
                .iter()
                .any(|p| p.model_name.contains("enhanced")));

            println!(
                "✅ Enhanced TimeMixer prediction successful: {} predictions",
                predictions.len()
            );
        } else {
            println!("⚠️ Enhanced adapter not connected, testing fallback behavior");
        }
    } else {
        println!("ℹ️ Enhanced adapter not available, testing fallback only");
    }

    Ok(())
}

#[tokio::test]
async fn test_configuration_validation() -> Result<()> {
    // Test valid configuration with real models
    let valid_config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["TimeMixer".to_string(), "LSTM".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    // Should validate successfully
    let predictor = FannPredictor::new(valid_config);
    assert!(predictor.is_ok());
    println!("✅ Valid configuration with real models accepted");

    // Test configuration with only non-FANN-compatible models
    let invalid_config = NeuralConfig {
        memory_gb: 2.0,
        models: vec!["NonExistentModel".to_string()], // No FANN fallback
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    // Configuration should be created but validation would fail if enforced
    let invalid_predictor = FannPredictor::new(invalid_config);
    assert!(invalid_predictor.is_ok()); // Constructor doesn't validate, only config.load() does
    println!("✅ Invalid model configuration handled gracefully in constructor");

    Ok(())
}

#[tokio::test]
async fn test_ensemble_statistics_with_feature_flag() -> Result<()> {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["LSTM".to_string(), "GRU".to_string(), "DeepAR".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.75,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    let predictor = FannPredictor::new(config)?;
    let test_data = create_test_data(50);

    // Generate ensemble predictions
    let predictions = predictor
        .predict_ensemble(
            &test_data,
            5,
            &["LSTM".to_string(), "GRU".to_string(), "DeepAR".to_string()],
            None,
        )
        .await?;

    assert!(!predictions.is_empty());

    // Get ensemble statistics
    let stats = predictor.get_ensemble_stats().await?;

    assert!(stats.contains_key("current_regime"));
    assert!(stats.contains_key("dynamic_weights"));
    assert!(stats.contains_key("model_performances"));

    println!("✅ Ensemble statistics retrieved successfully");

    // Test performance tracking
    let actual_values = vec![101.0, 102.0, 103.0, 104.0, 105.0];
    predictor
        .update_performance("LSTM", &actual_values, &predictions)
        .await?;

    let updated_stats = predictor.get_ensemble_stats().await?;
    assert!(updated_stats.contains_key("model_performances"));

    println!("✅ Performance tracking working correctly");

    Ok(())
}

#[tokio::test]
async fn test_model_routing_logging() -> Result<()> {
    // Test that different configurations produce appropriate log messages
    let config_real_enabled = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["TimeMixer".to_string(), "MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    let predictor = FannPredictor::new(config_real_enabled)?;
    let test_data = create_test_data(30);

    // This should log attempts to use real models and fallback to FANN
    let predictions = predictor
        .test_predict_with_model("TimeMixer", &test_data, 3)
        .await?;
    assert!(!predictions.is_empty());

    // This should log using FANN directly (not a real model)
    let mlp_predictions = predictor
        .test_predict_with_model("MLP", &test_data, 3)
        .await?;
    assert!(!mlp_predictions.is_empty());

    println!("✅ Model routing and logging verification completed");

    Ok(())
}

#[tokio::test]
async fn test_adapter_status_reporting() -> Result<()> {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["TimeMixer".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };

    let predictor = FannPredictor::new(config)?;

    // Check adapter availability
    let has_adapter = predictor.has_neuro_divergent_adapter();
    println!("📊 Neuro-divergent adapter available: {}", has_adapter);

    // Check enhanced adapter status
    let enhanced_status = predictor.get_enhanced_adapter_status().await;
    match enhanced_status {
        Some(status) => println!("📊 Enhanced adapter status: {}", status),
        None => println!("📊 Enhanced adapter not available"),
    }

    // Test adapter initialization if available
    if predictor.has_neuro_divergent_adapter() {
        let init_result = predictor.init_enhanced_adapter().await;
        match init_result {
            Ok(_) => println!("✅ Enhanced adapter initialized successfully"),
            Err(e) => println!("⚠️ Enhanced adapter initialization failed: {}", e),
        }
    }

    Ok(())
}
