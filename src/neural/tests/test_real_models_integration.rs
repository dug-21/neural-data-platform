//! Test real models integration in FannPredictor

use crate::neural::FannPredictor;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use chrono::Utc;
use std::collections::HashMap;

/// Helper function to create complete NeuralConfig for tests
fn create_test_config(models: Vec<String>, use_real_models: bool) -> NeuralConfig {
    NeuralConfig {
        memory_gb: 1.0,
        models,
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models,
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
    }
}

#[tokio::test]
async fn test_fann_predictor_with_real_models_enabled() {
    let config = create_test_config(
        vec!["DeepAR".to_string(), "TCN".to_string(), "NHITS".to_string()],
        true,
    );

    let predictor = FannPredictor::new(config).unwrap();

    // Test that adapters are available
    assert!(predictor.has_neuro_divergent_adapter());

    println!("✅ Real models integration test passed");
}

#[tokio::test]
async fn test_fann_predictor_with_real_models_disabled() {
    let config = create_test_config(
        vec!["DeepAR".to_string(), "TCN".to_string(), "NHITS".to_string()],
        false,
    );

    let predictor = FannPredictor::new(config).unwrap();

    // Test that it should still work in FANN-only mode
    let mut test_data = Vec::new();
    let base_time = Utc::now();

    for i in 0..50 {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + i as f64);

        test_data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i),
            entity: Some("test".to_string()),
            symbol: "TEST".to_string(),
            open: 100.0 + i as f64,
            high: 102.0 + i as f64,
            low: 98.0 + i as f64,
            close: 101.0 + i as f64,
            volume: 1000000.0,
            source: Some("test".to_string()),
            value: Some(101.0 + i as f64),
            metadata: Some(serde_json::json!({})),
            indicators,
        });
    }

    let predictions = predictor
        .test_predict_with_model("DeepAR", &test_data, 5)
        .await
        .unwrap();
    assert_eq!(predictions.len(), 5);

    println!("✅ FANN-only mode test passed");
}

#[tokio::test]
async fn test_real_model_specific_behavior() {
    let config = create_test_config(vec!["DeepAR".to_string()], true);

    let predictor = FannPredictor::new(config).unwrap();

    // Create minimal test data
    let mut test_data = Vec::new();
    let base_time = Utc::now();

    for i in 0..30 {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + i as f64);

        test_data.push(TimeSeriesData {
            timestamp: base_time + chrono::Duration::minutes(i),
            entity: Some("test".to_string()),
            symbol: "TEST".to_string(),
            open: 100.0 + i as f64,
            high: 102.0 + i as f64,
            low: 98.0 + i as f64,
            close: 101.0 + i as f64,
            volume: 1000000.0,
            source: Some("test".to_string()),
            value: Some(101.0 + i as f64),
            metadata: Some(serde_json::json!({})),
            indicators,
        });
    }

    // Should attempt real model first, then fallback to FANN
    let predictions = predictor
        .test_predict_with_model("DeepAR", &test_data, 3)
        .await
        .unwrap();
    assert!(!predictions.is_empty());

    println!("✅ Real model specific behavior test passed");
}

#[tokio::test]
async fn test_model_not_real_but_flag_enabled() {
    let config = create_test_config(
        vec!["MLP".to_string()],
        true, // Enable real models
    );

    let predictor = FannPredictor::new(config).unwrap();

    let test_data = vec![TimeSeriesData {
        timestamp: Utc::now(),
        entity: Some("test".to_string()),
        symbol: "TEST".to_string(),
        open: 100.0,
        high: 102.0,
        low: 98.0,
        close: 101.0,
        volume: 1000000.0,
        source: Some("test".to_string()),
        value: Some(101.0),
        metadata: Some(serde_json::json!({})),
        indicators: HashMap::new(),
    }];

    // MLP is not a real model, so should use FANN implementation regardless of flag
    let predictions = predictor
        .test_predict_with_model("MLP", &test_data, 3)
        .await
        .unwrap();
    assert!(!predictions.is_empty());

    println!("✅ Non-real model with flag enabled test passed");
}

#[tokio::test]
async fn test_backward_compatibility() {
    let config = create_test_config(
        vec!["DeepAR".to_string()],
        false, // Explicit false for backward compatibility
    );

    let predictor = FannPredictor::new(config).unwrap();

    // Should work in FANN-only mode
    assert!(predictor.has_neuro_divergent_adapter()); // Adapter may be present but not used

    println!("✅ Backward compatibility test passed");
}
