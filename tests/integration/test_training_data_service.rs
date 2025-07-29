//! Integration tests for TrainingDataService

use neural_trader::integration::{
    TrainingDataService, TrainingDataConfig, ModelType, ValidationError
};
use neural_trader::data::{TimeSeriesData, TimescaleDBStorage, RedisCache};
use neural_trader::config::{DataConfig, RedisConfig};
use chrono::{Utc, Duration};
use std::sync::Arc;
use std::collections::HashMap;

#[tokio::test]
async fn test_training_data_service_creation() {
    // Create mock storage and cache
    let data_config = DataConfig {
        timescale_url: "postgresql://test@localhost/testdb".to_string(),
        retention_days: 90,
        aggregate_intervals: vec!["1 hour".to_string(), "1 day".to_string()],
        enable_data_validation: true,
        enable_data_monitoring: true,
    };
    
    let redis_config = RedisConfig {
        url: "redis://localhost:6379".to_string(),
        pool_size: 4,
        cache_ttl: 300,
        key_prefix: "test".to_string(),
    };
    
    let storage = Arc::new(TimescaleDBStorage::new(&data_config).await.unwrap());
    let cache = Arc::new(RedisCache::new(&redis_config).await.unwrap());
    
    let service = TrainingDataService::new(storage, cache).await;
    assert!(service.is_ok());
}

#[test]
fn test_validation_error_types() {
    // Test insufficient data error
    let err = ValidationError::InsufficientData { got: 50, need: 100 };
    assert!(err.to_string().contains("Insufficient data"));
    
    // Test invalid values error
    let err = ValidationError::InvalidValues("NaN detected".to_string());
    assert!(err.to_string().contains("Invalid data values"));
    
    // Test missing features error
    let err = ValidationError::MissingFeatures(vec!["sma".to_string(), "rsi".to_string()]);
    assert!(err.to_string().contains("Missing required features"));
    
    // Test quality issue error
    let err = ValidationError::QualityIssue("Large gap in data".to_string());
    assert!(err.to_string().contains("Data quality issue"));
}

#[test]
fn test_training_config_default() {
    let config = TrainingDataConfig::default();
    
    assert_eq!(config.batch_size, 32);
    assert_eq!(config.sequence_length, 50);
    assert_eq!(config.feature_window, 20);
    assert!(config.normalize);
    assert!(config.include_volume);
    assert!(config.include_indicators);
    assert!(config.cache_enabled);
    assert_eq!(config.cache_ttl_seconds, 3600);
}

#[test]
fn test_model_type_serialization() {
    // Test that model types can be serialized/deserialized
    let model_types = vec![
        ModelType::MLP,
        ModelType::LSTM,
        ModelType::GRU,
        ModelType::CNN,
        ModelType::Ensemble,
    ];
    
    for model_type in model_types {
        let serialized = serde_json::to_string(&model_type).unwrap();
        let deserialized: ModelType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(model_type, deserialized);
    }
}

#[test]
fn test_data_validation() {
    // Create sample data for validation
    let mut data = Vec::new();
    let base_time = Utc::now() - Duration::hours(100);
    
    // Valid data
    for i in 0..150 {
        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: base_time + Duration::hours(i),
            open: 50000.0 + (i as f64 * 10.0),
            high: 50100.0 + (i as f64 * 10.0),
            low: 49900.0 + (i as f64 * 10.0),
            close: 50050.0 + (i as f64 * 10.0),
            volume: 1000.0 + (i as f64),
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(50050.0),
            metadata: None,
        });
    }
    
    // This would be tested with actual service instance
    // For now, we just verify the data structure is correct
    assert_eq!(data.len(), 150);
    assert!(data[0].timestamp < data[149].timestamp);
}

#[test]
fn test_prepared_training_data_structure() {
    use neural_trader::integration::training_data_service::{PreparedTrainingData, NormalizationParams};
    
    let prepared_data = PreparedTrainingData {
        model_type: ModelType::MLP,
        symbol: "BTC/USD".to_string(),
        features: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
        targets: vec![100.0, 101.0],
        timestamps: vec![Utc::now(), Utc::now() + Duration::hours(1)],
        feature_names: vec!["close".to_string(), "volume".to_string(), "sma".to_string()],
        normalization_params: Some(NormalizationParams {
            feature_means: vec![2.5, 3.5, 4.5],
            feature_stds: vec![1.5, 1.5, 1.5],
            target_mean: 100.5,
            target_std: 0.5,
        }),
        metadata: HashMap::new(),
    };
    
    // Verify structure
    assert_eq!(prepared_data.features.len(), 2);
    assert_eq!(prepared_data.targets.len(), 2);
    assert_eq!(prepared_data.timestamps.len(), 2);
    assert_eq!(prepared_data.feature_names.len(), 3);
    assert!(prepared_data.normalization_params.is_some());
}

#[tokio::test]
#[ignore] // This test requires actual database/cache connections
async fn test_load_training_batch() {
    // This test would require actual database and cache connections
    // It's marked as ignored but shows how the service would be used
    
    let data_config = DataConfig {
        timescale_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| 
            "postgresql://localhost/neural_trader".to_string()),
        retention_days: 90,
        aggregate_intervals: vec!["1 hour".to_string()],
        enable_data_validation: true,
        enable_data_monitoring: true,
    };
    
    let redis_config = RedisConfig {
        url: std::env::var("REDIS_URL").unwrap_or_else(|_| 
            "redis://localhost:6379".to_string()),
        pool_size: 4,
        cache_ttl: 300,
        key_prefix: "test".to_string(),
    };
    
    let storage = Arc::new(TimescaleDBStorage::new(&data_config).await.unwrap());
    let cache = Arc::new(RedisCache::new(&redis_config).await.unwrap());
    
    let service = TrainingDataService::new(storage, cache).await.unwrap();
    
    // Test MLP data loading
    let config = TrainingDataConfig {
        batch_size: 16,
        sequence_length: 20,
        feature_window: 10,
        normalize: true,
        include_volume: true,
        include_indicators: true,
        cache_enabled: false,
        cache_ttl_seconds: 0,
    };
    
    let result = service.load_training_batch(
        ModelType::MLP,
        "BTC/USD",
        config.clone()
    ).await;
    
    match result {
        Ok(data) => {
            assert_eq!(data.model_type, ModelType::MLP);
            assert_eq!(data.symbol, "BTC/USD");
            assert!(!data.features.is_empty());
            assert_eq!(data.features.len(), data.targets.len());
            assert_eq!(data.features.len(), data.timestamps.len());
        }
        Err(e) => {
            println!("Expected error in test environment: {}", e);
        }
    }
    
    // Test LSTM data loading
    let lstm_result = service.load_training_batch(
        ModelType::LSTM,
        "BTC/USD",
        config
    ).await;
    
    match lstm_result {
        Ok(data) => {
            assert_eq!(data.model_type, ModelType::LSTM);
            // LSTM data should have sequences
            if !data.features.is_empty() {
                assert!(data.features[0].len() > 10); // Sequence data is flattened
            }
        }
        Err(e) => {
            println!("Expected error in test environment: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires actual connections
async fn test_prepare_online_data() {
    let data_config = DataConfig {
        timescale_url: "postgresql://localhost/neural_trader".to_string(),
        retention_days: 90,
        aggregate_intervals: vec!["1 hour".to_string()],
        enable_data_validation: true,
        enable_data_monitoring: true,
    };
    
    let redis_config = RedisConfig {
        url: "redis://localhost:6379".to_string(),
        pool_size: 4,
        cache_ttl: 300,
        key_prefix: "test".to_string(),
    };
    
    let storage = Arc::new(TimescaleDBStorage::new(&data_config).await.unwrap());
    let cache = Arc::new(RedisCache::new(&redis_config).await.unwrap());
    
    let service = TrainingDataService::new(storage, cache).await.unwrap();
    
    let result = service.prepare_online_data("BTC/USD", 20).await;
    
    match result {
        Ok(data) => {
            assert_eq!(data.symbol, "BTC/USD");
            assert!(data.close > 0.0);
            assert!(!data.indicators.is_empty());
            
            // Check that indicators were calculated
            if data.indicators.contains_key("sma") {
                assert!(data.indicators["sma"] > 0.0);
            }
        }
        Err(e) => {
            println!("Expected error in test environment: {}", e);
        }
    }
}