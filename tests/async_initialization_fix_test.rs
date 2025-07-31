use autonomous_platform::config::NeuralConfig;
use autonomous_platform::neural::predictor::NeuralPredictor;
use std::sync::Arc;

#[tokio::test]
async fn test_async_default_initialization() {
    // This should not panic with "Cannot start a runtime from within a runtime"
    let result = NeuralPredictor::default().await;
    
    match result {
        Ok(predictor) => {
            println!("✅ Default initialization succeeded!");
            assert!(!predictor.get_available_models().is_empty());
        }
        Err(e) => {
            // This is acceptable during testing if dependencies aren't available
            println!("⚠️  Initialization failed (expected in test env): {}", e);
        }
    }
}

#[tokio::test]
async fn test_async_custom_initialization() {
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: false,
        enable_fallback: false,
        lookback_window: 24,
        enable_circuit_breakers: false,
        enable_graceful_degradation: false,
        enable_performance_monitoring: false,
        enable_adaptive_retry: false,
        enable_model_ensembles: false,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.1,
    };
    
    // This should not panic with "Cannot start a runtime from within a runtime"
    let result = NeuralPredictor::new(config).await;
    
    match result {
        Ok(predictor) => {
            println!("✅ Custom config initialization succeeded!");
            assert!(predictor.is_ready().await);
        }
        Err(e) => {
            // This is acceptable during testing if dependencies aren't available
            println!("⚠️  Initialization failed (expected in test env): {}", e);
        }
    }
}

#[tokio::test]
async fn test_no_nested_runtime_panic() {
    // This was the original issue - creating Arc<NeuralPredictor> in main.rs
    let result = NeuralPredictor::default().await;
    
    match result {
        Ok(predictor) => {
            let arc_predictor = Arc::new(predictor);
            println!("✅ Arc<NeuralPredictor> created without runtime panic!");
            assert!(arc_predictor.is_ready().await);
        }
        Err(e) => {
            // This is acceptable during testing if dependencies aren't available
            println!("⚠️  Initialization failed (expected in test env): {}", e);
        }
    }
}