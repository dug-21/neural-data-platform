//! Unit tests for Vendor Predictor module (formerly FANN Predictor)
//! 
//! Updated to use VendorPredictor directly instead of NeuralPredictor wrapper

use autonomous_platform::neural::vendor_predictor::VendorPredictor;
use autonomous_platform::neural::NeuralPredictorTrait;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::data::sector_mapper::{SectorMapper, SectorMapperConfig};
use autonomous_platform::monitoring::model_performance_tracker::ModelPerformanceTracker;
use std::sync::Arc;
use chrono::Utc;
use std::collections::HashMap;

fn create_test_config() -> NeuralConfig {
    NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "NHITS".to_string(), "TCN".to_string()],
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
        lookback_window: 24,
        // Required fields for NeuralConfig
        input_size: 60,
        output_size: 1,
        hidden_layers: vec![128, 64, 32],
        learning_rate: 0.001,
        prediction_horizon: Some(24),
        normalization_method: Some("z-score".to_string()),
    }
}

fn create_test_data(size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_price = 50000.0;
    let base_volume = 1000.0;
    
    for i in 0..size {
        let price_variation = (i as f64 * 0.01).sin() * 0.02;
        let volume_variation = (i as f64 * 0.02).cos() * 0.1;
        
        let close_price = base_price * (1.0 + price_variation);
        let volume = base_volume * (1.0 + volume_variation);
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + 20.0 * price_variation);
        indicators.insert("macd".to_string(), price_variation * 100.0);
        
        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
            open: close_price * 0.999,
            high: close_price * 1.002,
            low: close_price * 0.998,
            close: close_price,
            volume: vec![volume],
            indicators,
            source: Some("test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(close_price),
            metadata: None,
            values: vec![close_price],
            timestamps: vec![Utc::now() + chrono::Duration::minutes(i as i64)],
            metadata_map: HashMap::new(),
        });
    }
    
    data
}

// Model config tests moved to integration tests due to visibility

#[tokio::test]
async fn test_vendor_predictor_initialization() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker);
    
    // Just verify it creates successfully
    assert!(predictor.is_ok());
}

#[tokio::test]
async fn test_single_model_prediction() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    let test_data = create_test_data(150);
    
    // Test prediction
    let predictions = predictor.predict(&test_data, 5, None).await.unwrap();
    
    assert_eq!(predictions.len(), 5);
    // VendorPredictor uses ensemble by default
    assert!(predictions[0].model_name.contains("ensemble") || predictions[0].model_name == "none");
    
    // Verify prediction structure
    for (i, pred) in predictions.iter().enumerate() {
        assert!(pred.confidence > 0.0 && pred.confidence <= 1.0);
        assert!(pred.interval_low < pred.value);
        assert!(pred.interval_high > pred.value);
        assert_eq!(pred.timestamp, test_data.last().unwrap().timestamp + chrono::Duration::minutes((i + 1) as i64));
    }
}

#[tokio::test]
async fn test_ensemble_prediction() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    let test_data = create_test_data(150);
    
    let models = vec!["MLP".to_string(), "NHITS".to_string(), "TCN".to_string()];
    let predictions = predictor.predict_ensemble(&test_data, 5, &models, None).await.unwrap();
    
    assert_eq!(predictions.len(), 5);
    assert!(predictions[0].model_name.contains("ensemble"));
    
    // Ensemble predictions should have high confidence
    for pred in &predictions {
        assert!(pred.confidence > 0.5);
        assert!(pred.confidence <= 0.95); // Capped at 95%
    }
}

#[tokio::test]
async fn test_prediction_with_insufficient_data() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    let test_data = create_test_data(5); // Too small for window size
    
    // Should handle gracefully with placeholder predictions
    let result = predictor.predict(&test_data, 3, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_feature_importance() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    
    let importance = predictor.get_feature_importance().await.unwrap();
    
    assert!(importance.contains_key("price"));
    assert!(importance.contains_key("volume"));
    assert!(importance.contains_key("rsi"));
    
    // Verify importance values sum to approximately 1.0
    let total: f64 = importance.values().sum();
    assert!((total - 1.0).abs() < 0.01);
}

#[tokio::test]
async fn test_volatility_calculation() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    
    // Create data with known volatility
    let mut data = Vec::new();
    for i in 0..10 {
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::minutes(i),
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close: 100.0 + (i as f64),
            volume: vec![1000.0],
            volume_value: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("TEST".to_string()),
            value: Some(100.0 + (i as f64)),
            metadata: None,
            // Enhanced fields for vendor model integration
            values: vec![100.0 + (i as f64)],
            intervals: vec![],
            timestamps: vec![Utc::now() + chrono::Duration::minutes(i)],
            metadata_map: HashMap::new(),
        });
    }
    
    // Use reflection or test through predictions
    let predictions = predictor.predict(&data, 1, None).await.unwrap();
    
    // Predictions should have reasonable intervals based on volatility
    assert!(predictions[0].interval_high > predictions[0].value);
    assert!(predictions[0].interval_low < predictions[0].value);
}

#[tokio::test]
async fn test_prediction_caching() {
    let mut config = create_test_config();
    config.prediction_cache_ttl = 5; // Short TTL for testing
    
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    let test_data = create_test_data(150);
    
    // First prediction
    let start = std::time::Instant::now();
    let predictions1 = predictor.predict(&test_data, 5, None).await.unwrap();
    let first_duration = start.elapsed();
    
    // Second prediction (should be cached)
    let start = std::time::Instant::now();
    let predictions2 = predictor.predict(&test_data, 5, None).await.unwrap();
    let cached_duration = start.elapsed();
    
    // Cached should be faster
    assert!(cached_duration < first_duration / 2);
    
    // Results should be identical
    for (p1, p2) in predictions1.iter().zip(predictions2.iter()) {
        assert_eq!(p1.value, p2.value);
        assert_eq!(p1.confidence, p2.confidence);
    }
}

#[tokio::test]
async fn test_online_learning() {
    let config = create_test_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
    let initial_data = create_test_data(150);
    let new_data = create_test_data(50);
    
    // Initial training
    let _ = predictor.predict(&initial_data, 5, None).await.unwrap();
    
    // Update with new data (using available method)
    let result = predictor.update_model(&new_data[0]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_model_specific_configurations() {
    let mut config = create_test_config();
    config.models = vec!["DeepAR".to_string(), "Transformer".to_string()];
    
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker);
    assert!(predictor.is_ok());
}

// Training data preparation tests moved to integration tests

// Concurrent prediction tests moved to integration tests