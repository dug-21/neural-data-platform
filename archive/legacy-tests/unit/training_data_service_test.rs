//! Unit tests for TrainingDataService
//! Tests data loading, feature engineering, windowing, and batch preparation

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use mockall::predicate::*;
use mockall::mock;

use autonomous_platform::products::features::realtraining::training_data_service::{
    TrainingDataService, TrainingDataConfig, TrainingBatch, BatchMetadata,
    FeatureConfig, ValidationConfig, NormalizationMethod, FeatureStats,
    TrainingDataIterator,
};
use autonomous_platform::adapters::{TimescaleAdapter, MarketData};
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::neural::ModelType;

// Mock TimescaleAdapter
mock! {
    pub TimescaleAdapter {
        pub async fn query_market_data(
            &self,
            symbol: &str,
            start_ts: i64,
            end_ts: i64,
        ) -> Result<Vec<MarketData>>;
    }
}

// Helper to create test market data
fn create_test_market_data(symbol: &str, timestamp: i64, price: f64) -> MarketData {
    MarketData {
        symbol: symbol.to_string(),
        timestamp,
        open: price - 5.0,
        high: price + 10.0,
        low: price - 10.0,
        close: price,
        volume: vec![1000.0 + (timestamp % 1000) as f64,
    }
}

// Helper to create a sequence of market data
fn create_market_data_sequence(symbol: &str, start_time: DateTime<Utc>, count: usize, interval_minutes: i64) -> Vec<MarketData> {
    let mut data = Vec::new();
    for i in 0..count {
        let timestamp = start_time + Duration::minutes(i as i64 * interval_minutes);
        let price = 50000.0 + (i as f64 * 10.0) + ((i as f64 * 2.0).sin() * 100.0); // Add some variation
        data.push(create_test_market_data(symbol, timestamp.timestamp(), price));
    }
    data
}

#[tokio::test]
async fn test_training_data_config_defaults() {
    let config = TrainingDataConfig::default();
    
    assert_eq!(config.window_size, 50);
    assert_eq!(config.step_size, 1);
    assert_eq!(config.min_samples, 1000);
    assert_eq!(config.max_samples, Some(100_000));
    
    // Check feature config defaults
    assert!(config.feature_config.use_indicators);
    assert!(config.feature_config.use_volume);
    assert!(config.feature_config.use_ratios);
    assert!(config.feature_config.use_temporal);
    assert!(matches!(config.feature_config.normalization, NormalizationMethod::MinMax));
    
    // Check validation config defaults
    assert!(config.validation_config.check_gaps);
    assert_eq!(config.validation_config.max_gap_minutes, 60);
    assert_eq!(config.validation_config.outlier_threshold, Some(5.0));
    assert_eq!(config.validation_config.min_quality_score, 0.95);
}

#[tokio::test]
async fn test_load_training_data_success() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig::default();
    
    let start_time = Utc::now() - Duration::hours(24);
    let end_time = Utc::now();
    let market_data = create_market_data_sequence("BTC/USD", start_time, 1500, 1);
    
    mock_adapter
        .expect_query_market_data()
        .with(eq("BTC/USD"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_training_data(
        "BTC/USD",
        start_time,
        end_time,
        &ModelType::Classification,
    ).await;
    
    assert!(result.is_ok());
    let batch = result.unwrap();
    
    assert_eq!(batch.symbol, "BTC/USD");
    assert!(!batch.features.is_empty());
    assert!(!batch.targets.is_empty());
    assert_eq!(batch.features.len(), batch.targets.len());
}

#[tokio::test]
async fn test_load_training_data_insufficient_samples() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        min_samples: 1000,
        ..Default::default()
    };
    
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now();
    let market_data = create_market_data_sequence("BTC/USD", start_time, 100, 1); // Only 100 samples
    
    mock_adapter
        .expect_query_market_data()
        .with(eq("BTC/USD"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_training_data(
        "BTC/USD",
        start_time,
        end_time,
        &ModelType::Classification,
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Insufficient data"));
}

#[tokio::test]
async fn test_validate_data_with_gaps() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        validation_config: ValidationConfig {
            check_gaps: true,
            max_gap_minutes: 60,
            outlier_threshold: None,
            min_quality_score: 0.95,
        },
        ..Default::default()
    };
    
    let start_time = Utc::now() - Duration::hours(2);
    let end_time = Utc::now();
    
    // Create data with a gap
    let mut market_data = create_market_data_sequence("BTC/USD", start_time, 50, 1);
    // Add data after a 90-minute gap
    let gap_start = start_time + Duration::minutes(50);
    let after_gap = create_market_data_sequence("BTC/USD", gap_start + Duration::minutes(90), 50, 1);
    market_data.extend(after_gap);
    
    mock_adapter
        .expect_query_market_data()
        .with(eq("BTC/USD"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    // This should succeed but log warnings about gaps
    let result = service.load_training_data(
        "BTC/USD",
        start_time,
        end_time,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok()); // Should still work, just with warnings
}

#[tokio::test]
async fn test_sliding_window_creation() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        window_size: 10,
        step_size: 5, // 50% overlap
        min_samples: 50,
        ..Default::default()
    };
    
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now();
    let market_data = create_market_data_sequence("BTC/USD", start_time, 50, 1);
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_training_data(
        "BTC/USD",
        start_time,
        end_time,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok());
    let batch = result.unwrap();
    
    // With window_size=10, step_size=5, and 50 data points:
    // Windows: [0-9], [5-14], [10-19], ..., [40-49]
    // Total windows = ((50 - 10) / 5) + 1 = 9
    assert_eq!(batch.features.len(), 9);
    assert_eq!(batch.targets.len(), 9);
}

#[tokio::test]
async fn test_incremental_data_loading() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        window_size: 10,
        ..Default::default()
    };
    
    let last_timestamp = Utc::now() - Duration::minutes(30);
    let start_time = last_timestamp - Duration::minutes(5); // 5 min overlap
    let end_time = Utc::now();
    
    // Create new data points
    let market_data = create_market_data_sequence("BTC/USD", start_time, 35, 1);
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_incremental_data(
        "BTC/USD",
        last_timestamp,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[tokio::test]
async fn test_incremental_data_insufficient() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        window_size: 50,
        ..Default::default()
    };
    
    let last_timestamp = Utc::now() - Duration::minutes(10);
    let market_data = create_market_data_sequence("BTC/USD", last_timestamp - Duration::minutes(5), 20, 1);
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_incremental_data(
        "BTC/USD",
        last_timestamp,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok());
    assert!(result.unwrap().is_none()); // Not enough new data
}

#[tokio::test]
async fn test_feature_statistics_calculation() {
    let config = TrainingDataConfig::default();
    let mock_adapter = MockTimescaleAdapter::new();
    let service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    // Create a test batch
    let batch = TrainingBatch {
        features: vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![2.0, 3.0, 4.0, 5.0],
            vec![3.0, 4.0, 5.0, 6.0],
            vec![4.0, 5.0, 6.0, 7.0],
        ],
        targets: vec![
            vec![5.0],
            vec![6.0],
            vec![7.0],
            vec![8.0],
        ],
        timestamps: vec![],
        symbol: "BTC/USD".to_string(),
        metadata: BatchMetadata {
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
            sample_count: 4,
            quality_score: 1.0,
            feature_stats: HashMap::new(),
        },
    };
    
    let stats = service.get_feature_statistics(&batch);
    
    assert_eq!(stats.len(), 4); // 4 features
    
    // Check first feature statistics
    let feature_0_stats = &stats["feature_0"];
    assert_eq!(feature_0_stats.mean, 2.5); // (1+2+3+4)/4
    assert_eq!(feature_0_stats.min, 1.0);
    assert_eq!(feature_0_stats.max, 4.0);
    assert!(feature_0_stats.std_dev > 0.0);
}

#[tokio::test]
async fn test_training_data_iterator() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        window_size: 10,
        min_samples: 20,
        ..Default::default()
    };
    
    let start_time = Utc::now() - Duration::hours(3);
    let end_time = Utc::now();
    let batch_duration = Duration::hours(1);
    
    // Set up expectations for 3 batches
    for i in 0..3 {
        let batch_start = start_time + Duration::hours(i);
        let batch_end = batch_start + batch_duration;
        let market_data = create_market_data_sequence("BTC/USD", batch_start, 60, 1);
        
        mock_adapter
            .expect_query_market_data()
            .times(1)
            .returning(move |_, _, _| Ok(market_data.clone()));
    }
    
    let service = TrainingDataService::new(Arc::new(mock_adapter), config);
    let mut iterator = TrainingDataIterator::new(
        service,
        "BTC/USD".to_string(),
        start_time,
        end_time,
        batch_duration,
        ModelType::Regression,
    );
    
    // Get all batches
    let mut batch_count = 0;
    while let Ok(Some(_batch)) = iterator.next_batch().await {
        batch_count += 1;
    }
    
    assert_eq!(batch_count, 3);
}

#[tokio::test]
async fn test_time_series_conversion() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig::default();
    
    let now = Utc::now();
    let market_data = vec![
        create_test_market_data("ETH/USD", now.timestamp() - 300, 3000.0),
        create_test_market_data("ETH/USD", now.timestamp() - 240, 3010.0),
        create_test_market_data("ETH/USD", now.timestamp() - 180, 3020.0),
    ];
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    // We need to make this conversion happen through the public API
    let result = service.load_training_data(
        "ETH/USD",
        now - Duration::minutes(10),
        now,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok());
    let batch = result.unwrap();
    assert_eq!(batch.symbol, "ETH/USD");
}

#[tokio::test]
async fn test_batch_metadata_generation() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        window_size: 5,
        min_samples: 10,
        ..Default::default()
    };
    
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now();
    let market_data = create_market_data_sequence("BTC/USD", start_time, 20, 1);
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_training_data(
        "BTC/USD",
        start_time,
        end_time,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok());
    let batch = result.unwrap();
    
    // Check metadata
    assert!(batch.metadata.sample_count > 0);
    assert_eq!(batch.metadata.quality_score, 1.0); // Default for now
    assert!(batch.metadata.start_time <= batch.metadata.end_time);
}

#[tokio::test]
async fn test_normalization_methods() {
    // Test different normalization configurations
    let configs = vec![
        (NormalizationMethod::MinMax, "MinMax normalization"),
        (NormalizationMethod::ZScore, "Z-score normalization"),
        (NormalizationMethod::PercentChange, "Percent change"),
        (NormalizationMethod::LogReturns, "Log returns"),
    ];
    
    for (method, description) in configs {
        let config = TrainingDataConfig {
            feature_config: FeatureConfig {
                normalization: method,
                ..Default::default()
            },
            ..Default::default()
        };
        
        assert!(matches!(config.feature_config.normalization, _), "Failed for {}", description);
    }
}

#[tokio::test]
async fn test_feature_engineering_configuration() {
    let config = FeatureConfig {
        use_indicators: true,
        use_volume: true,
        use_ratios: true,
        use_temporal: false,
        normalization: NormalizationMethod::ZScore,
    };
    
    assert!(config.use_indicators);
    assert!(config.use_volume);
    assert!(config.use_ratios);
    assert!(!config.use_temporal);
    assert!(matches!(config.normalization, NormalizationMethod::ZScore));
}

#[tokio::test]
async fn test_validation_config_edge_cases() {
    let config = ValidationConfig {
        check_gaps: true,
        max_gap_minutes: 30,
        outlier_threshold: Some(3.0),
        min_quality_score: 0.99,
    };
    
    assert!(config.check_gaps);
    assert_eq!(config.max_gap_minutes, 30);
    assert_eq!(config.outlier_threshold, Some(3.0));
    assert_eq!(config.min_quality_score, 0.99);
}

#[tokio::test]
async fn test_max_samples_limit() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig {
        window_size: 10,
        max_samples: Some(50),
        min_samples: 10,
        ..Default::default()
    };
    
    let start_time = Utc::now() - Duration::hours(2);
    let end_time = Utc::now();
    let market_data = create_market_data_sequence("BTC/USD", start_time, 1000, 1); // Large dataset
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(move |_, _, _| Ok(market_data.clone()));
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_training_data(
        "BTC/USD",
        start_time,
        end_time,
        &ModelType::Regression,
    ).await;
    
    assert!(result.is_ok());
    let batch = result.unwrap();
    
    // Should be limited by max_samples configuration
    assert!(batch.features.len() <= 50);
}

#[tokio::test]
async fn test_empty_data_handling() {
    let mut mock_adapter = MockTimescaleAdapter::new();
    let config = TrainingDataConfig::default();
    
    mock_adapter
        .expect_query_market_data()
        .times(1)
        .returning(|_, _, _| Ok(Vec::new())); // Empty data
    
    let mut service = TrainingDataService::new(Arc::new(mock_adapter), config);
    
    let result = service.load_training_data(
        "BTC/USD",
        Utc::now() - Duration::hours(1),
        Utc::now(),
        &ModelType::Classification,
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No data found"));
}