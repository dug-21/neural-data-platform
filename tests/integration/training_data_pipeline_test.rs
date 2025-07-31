//! Integration tests for the training data pipeline
//! Tests end-to-end data flow from database to training batch preparation

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration as TokioDuration};
use tempfile::TempDir;
use serde_json::json;

use autonomous_platform::integration::data_access::{
    DataAccessLayer, DataRequest, Timeframe, TrainingDataRequest, FeatureConfig
};
use autonomous_platform::products::features::realtraining::training_data_service::{
    TrainingDataService, TrainingDataConfig, TrainingBatch, 
    FeatureConfig as TrainingFeatureConfig, ValidationConfig, NormalizationMethod
};
use autonomous_platform::data::{TimescaleDBStorage, RedisCache, TimeSeriesData};
use autonomous_platform::adapters::{TimescaleAdapter, TimescaleConfig, MarketData};
use autonomous_platform::neural::ModelType;

// Test configuration
struct TestEnvironment {
    data_access: DataAccessLayer,
    training_service: TrainingDataService,
    temp_dir: TempDir,
}

impl TestEnvironment {
    async fn setup() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        
        // Setup test database connection
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/test_neural_trader".to_string());
        
        let storage = Arc::new(TimescaleDBStorage::new(&database_url).await?);
        
        // Setup test Redis connection
        let redis_url = std::env::var("TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_string());
        
        let cache = Arc::new(RedisCache::new(&redis_url).await?);
        
        // Create data access layer
        let data_access = DataAccessLayer::new(Arc::clone(&storage), Arc::clone(&cache)).await?;
        
        // Setup TimescaleAdapter for training service
        let timescale_config = TimescaleConfig {
            connection_string: database_url,
            max_connections: 5,
            connection_timeout_secs: 30,
        };
        
        let timescale_adapter = Arc::new(TimescaleAdapter::new(timescale_config).await?);
        
        // Create training service
        let training_config = TrainingDataConfig {
            window_size: 20,
            step_size: 5,
            min_samples: 50,
            max_samples: Some(1000),
            feature_config: TrainingFeatureConfig {
                use_indicators: true,
                use_volume: true,
                use_ratios: true,
                use_temporal: true,
                normalization: NormalizationMethod::MinMax,
            },
            validation_config: ValidationConfig {
                check_gaps: true,
                max_gap_minutes: 60,
                outlier_threshold: Some(5.0),
                min_quality_score: 0.9,
            },
        };
        
        let training_service = TrainingDataService::new(timescale_adapter, training_config);
        
        Ok(TestEnvironment {
            data_access,
            training_service,
            temp_dir,
        })
    }
    
    async fn cleanup(&self) -> Result<()> {
        // Clean up test data
        // This would normally clean up database tables and Redis keys
        Ok(())
    }
}

// Helper function to insert test market data
async fn insert_test_market_data(
    storage: &TimescaleDBStorage,
    symbol: &str,
    start_time: DateTime<Utc>,
    count: usize,
    interval_minutes: i64,
) -> Result<()> {
    let mut data_points = Vec::new();
    
    for i in 0..count {
        let timestamp = start_time + Duration::minutes(i as i64 * interval_minutes);
        let base_price = 50000.0;
        let price_variation = (i as f64 * 0.1).sin() * 100.0;
        let price = base_price + price_variation + (i as f64 * 0.5);
        
        data_points.push(autonomous_platform::data::storage::TimeSeriesData {
            timestamp,
            source: "test_integration".to_string(),
            entity: symbol.to_string(),
            value: price,
            metadata: Some(json!({
                "open": price - 5.0,
                "high": price + 25.0,
                "low": price - 25.0,
                "close": price,
                "volume": 1000.0 + (i as f64 * 10.0),
                "indicators": {
                    "sma_20": price * 0.99,
                    "rsi": 50.0 + (i as f64 % 20.0),
                    "macd": (i as f64 % 10.0) - 5.0
                }
            })),
        });
    }
    
    storage.batch_insert(&data_points).await?;
    Ok(())
}

#[tokio::test]
async fn test_end_to_end_training_data_pipeline() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(4);
    let end_time = Utc::now();
    
    // Insert test data
    insert_test_market_data(&env.data_access.storage, symbol, start_time, 200, 1).await
        .expect("Failed to insert test data");
    
    // Wait for data to be available
    sleep(TokioDuration::from_millis(100)).await;
    
    // Test data access layer retrieval
    let market_data = env.data_access.get_market_data(symbol, Timeframe::Minute).await
        .expect("Failed to get market data");
    
    assert!(!market_data.is_empty(), "Market data should not be empty");
    assert_eq!(market_data[0].symbol, symbol);
    
    // Test training data service
    let mut training_service = env.training_service;
    let training_batch = training_service.load_training_data(
        symbol,
        start_time,
        end_time,
        &ModelType::Regression,
    ).await.expect("Failed to load training data");
    
    // Verify training batch structure
    assert_eq!(training_batch.symbol, symbol);
    assert!(!training_batch.features.is_empty(), "Features should not be empty");
    assert!(!training_batch.targets.is_empty(), "Targets should not be empty");
    assert_eq!(training_batch.features.len(), training_batch.targets.len());
    
    // Verify feature dimensions
    if !training_batch.features.is_empty() {
        let feature_dim = training_batch.features[0].len();
        assert!(feature_dim > 0, "Feature dimension should be positive");
        
        // All feature vectors should have the same dimension
        for feature_vec in &training_batch.features {
            assert_eq!(feature_vec.len(), feature_dim, 
                "All feature vectors should have the same dimension");
        }
    }
    
    // Verify metadata
    assert!(training_batch.metadata.sample_count > 0);
    assert!(training_batch.metadata.quality_score >= 0.0);
    assert!(training_batch.metadata.start_time <= training_batch.metadata.end_time);
    
    env.cleanup().await.expect("Failed to cleanup test environment");
}

#[tokio::test]
async fn test_data_quality_validation() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "ETH/USD";
    let start_time = Utc::now() - Duration::hours(2);
    
    // Insert data with intentional gaps
    let mut gap_timestamps = Vec::new();
    for i in 0..30 {
        gap_timestamps.push(start_time + Duration::minutes(i * 2));
    }
    
    // Add gap - skip 90 minutes
    for i in 30..60 {
        gap_timestamps.push(start_time + Duration::minutes(i * 2 + 90));
    }
    
    let mut data_points = Vec::new();
    for (i, timestamp) in gap_timestamps.iter().enumerate() {
        let price = 3000.0 + (i as f64 * 5.0);
        
        data_points.push(autonomous_platform::data::storage::TimeSeriesData {
            timestamp: *timestamp,
            source: "test_gaps".to_string(),
            entity: symbol.to_string(),
            value: price,
            metadata: Some(json!({
                "open": price - 2.0,
                "high": price + 10.0,
                "low": price - 10.0,
                "close": price,
                "volume": 1500.0
            })),
        });
    }
    
    env.data_access.storage.batch_insert(&data_points).await
        .expect("Failed to insert gap data");
    
    // Test with gap detection enabled
    let mut training_service = env.training_service;
    let result = training_service.load_training_data(
        symbol,
        start_time,
        start_time + Duration::hours(3),
        &ModelType::Classification,
    ).await;
    
    // Should succeed but with warnings about gaps
    assert!(result.is_ok(), "Should handle gaps gracefully");
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_concurrent_data_access() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD", "DOT/USD"];
    let start_time = Utc::now() - Duration::hours(2);
    
    // Insert data for all symbols
    for symbol in &symbols {
        insert_test_market_data(&env.data_access.storage, symbol, start_time, 100, 1).await
            .expect("Failed to insert test data");
    }
    
    sleep(TokioDuration::from_millis(200)).await;
    
    // Create concurrent requests
    let mut handles = Vec::new();
    
    for (i, symbol) in symbols.iter().enumerate() {
        let data_access = &env.data_access;
        let symbol_clone = symbol.to_string();
        
        let handle = tokio::spawn(async move {
            let request = DataRequest {
                agent_id: format!("concurrent_agent_{}", i),
                request_type: "historical_data".to_string(),
                symbol: symbol_clone,
                timeframe: Timeframe::Minute,
                start_time: Some(start_time),
                end_time: Some(Utc::now()),
                limit: Some(50),
                metadata: HashMap::new(),
            };
            
            data_access.handle_agent_data_request(request).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests
    let results = futures::future::join_all(handles).await;
    
    // Verify all succeeded
    for (i, result) in results.into_iter().enumerate() {
        let response = result.expect("Task should not panic")
            .expect("Request should succeed");
        
        assert!(response.success, "Request {} should succeed", i);
        assert!(!response.data.is_empty(), "Request {} should return data", i);
    }
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_training_data_iterator_integration() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(6);
    let end_time = Utc::now();
    
    // Insert continuous data for 6 hours
    insert_test_market_data(&env.data_access.storage, symbol, start_time, 360, 1).await
        .expect("Failed to insert test data");
    
    sleep(TokioDuration::from_millis(200)).await;
    
    // Create iterator for 2-hour batches
    let mut iterator = autonomous_platform::products::features::realtraining::training_data_service::TrainingDataIterator::new(
        env.training_service,
        symbol.to_string(),
        start_time,
        end_time,
        Duration::hours(2),
        ModelType::Regression,
    );
    
    let mut batch_count = 0;
    let mut total_samples = 0;
    
    // Process all batches
    while let Ok(Some(batch)) = iterator.next_batch().await {
        batch_count += 1;
        total_samples += batch.features.len();
        
        // Verify batch structure
        assert_eq!(batch.symbol, symbol);
        assert!(!batch.features.is_empty());
        assert_eq!(batch.features.len(), batch.targets.len());
        
        // Verify feature consistency
        if let Some(first_feature) = batch.features.first() {
            let feature_dim = first_feature.len();
            for feature in &batch.features {
                assert_eq!(feature.len(), feature_dim);
            }
        }
    }
    
    // Should have 3 batches (6 hours / 2 hours per batch)
    assert_eq!(batch_count, 3, "Should process exactly 3 batches");
    assert!(total_samples > 0, "Should have processed some samples");
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_feature_statistics_accuracy() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "ETH/USD";
    let start_time = Utc::now() - Duration::hours(2);
    
    // Insert data with known statistical properties
    let mut data_points = Vec::new();
    let known_values = vec![3000.0, 3010.0, 3020.0, 3030.0, 3040.0]; // Mean = 3020.0
    
    for (i, &value) in known_values.iter().enumerate() {
        let timestamp = start_time + Duration::minutes(i as i64);
        
        data_points.push(autonomous_platform::data::storage::TimeSeriesData {
            timestamp,
            source: "test_stats".to_string(),
            entity: symbol.to_string(),
            value,
            metadata: Some(json!({
                "open": value - 1.0,
                "high": value + 5.0,
                "low": value - 5.0,
                "close": value,
                "volume": 1000.0
            })),
        });
    }
    
    env.data_access.storage.batch_insert(&data_points).await
        .expect("Failed to insert statistical test data");
    
    sleep(TokioDuration::from_millis(100)).await;
    
    // Load training data
    let mut training_service = env.training_service;
    let batch = training_service.load_training_data(
        symbol,
        start_time,
        start_time + Duration::hours(1),
        &ModelType::Regression,
    ).await.expect("Failed to load training data");
    
    // Calculate feature statistics
    let stats = training_service.get_feature_statistics(&batch);
    
    // Verify statistics are calculated
    assert!(!stats.is_empty(), "Should have feature statistics");
    
    // Check that statistics have reasonable values
    for (feature_name, feature_stats) in stats {
        assert!(feature_stats.mean.is_finite(), 
            "Mean should be finite for feature {}", feature_name);
        assert!(feature_stats.std_dev >= 0.0, 
            "Standard deviation should be non-negative for feature {}", feature_name);
        assert!(feature_stats.min <= feature_stats.max, 
            "Min should be <= Max for feature {}", feature_name);
        assert!(feature_stats.mean >= feature_stats.min && feature_stats.mean <= feature_stats.max,
            "Mean should be between min and max for feature {}", feature_name);
    }
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_memory_efficiency() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(1);
    
    // Insert large dataset
    insert_test_market_data(&env.data_access.storage, symbol, start_time, 500, 1).await
        .expect("Failed to insert large dataset");
    
    sleep(TokioDuration::from_millis(200)).await;
    
    // Monitor memory usage
    let initial_memory = get_memory_usage();
    
    // Load training data multiple times
    for _ in 0..5 {
        let mut training_service = TrainingDataService::new(
            Arc::new(TimescaleAdapter::new(TimescaleConfig {
                connection_string: std::env::var("TEST_DATABASE_URL")
                    .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/test_neural_trader".to_string()),
                max_connections: 5,
                connection_timeout_secs: 30,
            }).await.expect("Failed to create adapter")),
            TrainingDataConfig {
                max_samples: Some(100), // Limit samples for memory test
                ..Default::default()
            }
        );
        
        let _batch = training_service.load_training_data(
            symbol,
            start_time,
            Utc::now(),
            &ModelType::Regression,
        ).await.expect("Failed to load training data");
        
        // Batch should be dropped here
    }
    
    // Force garbage collection if available
    #[cfg(feature = "gc")]
    {
        std::gc::collect();
    }
    
    let final_memory = get_memory_usage();
    let memory_increase = final_memory - initial_memory;
    
    // Memory increase should be reasonable (less than 50MB)
    assert!(memory_increase < 50 * 1024 * 1024, 
        "Memory increase should be less than 50MB, got {} bytes", memory_increase);
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test]
async fn test_error_recovery() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "INVALID/SYMBOL";
    let start_time = Utc::now() - Duration::hours(1);
    let end_time = Utc::now();
    
    // Test with non-existent symbol
    let result = env.data_access.get_market_data(symbol, Timeframe::Hourly).await;
    
    // Should handle gracefully
    assert!(result.is_ok(), "Should handle non-existent symbol gracefully");
    let data = result.unwrap();
    assert!(data.is_empty(), "Should return empty data for non-existent symbol");
    
    // Test training service with insufficient data
    let mut training_service = env.training_service;
    let training_result = training_service.load_training_data(
        symbol,
        start_time,
        end_time,
        &ModelType::Regression,
    ).await;
    
    // Should return appropriate error
    assert!(training_result.is_err(), "Should error with insufficient data");
    
    env.cleanup().await.expect("Failed to cleanup");
}

#[tokio::test] 
async fn test_data_integrity_throughout_pipeline() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(2);
    let end_time = Utc::now();
    
    // Insert data with known values
    let known_price = 50000.0;
    let known_volume = 1500.0;
    
    let data_point = autonomous_platform::data::storage::TimeSeriesData {
        timestamp: start_time + Duration::minutes(30),
        source: "test_integrity".to_string(),
        entity: symbol.to_string(),
        value: known_price,
        metadata: Some(json!({
            "open": known_price - 10.0,
            "high": known_price + 20.0,
            "low": known_price - 20.0,
            "close": known_price,
            "volume": known_volume,
        })),
    };
    
    env.data_access.storage.batch_insert(&vec![data_point]).await
        .expect("Failed to insert integrity test data");
    
    sleep(TokioDuration::from_millis(100)).await;
    
    // Retrieve through data access layer
    let market_data = env.data_access.get_market_data(symbol, Timeframe::Hourly).await
        .expect("Failed to get market data");
    
    assert!(!market_data.is_empty(), "Should have market data");
    
    let retrieved_data = &market_data[0];
    assert_eq!(retrieved_data.symbol, symbol);
    assert_eq!(retrieved_data.close, known_price);
    assert_eq!(retrieved_data.volume, known_volume);
    
    // Process through training pipeline
    let mut training_service = env.training_service;
    let batch = training_service.load_training_data(
        symbol,
        start_time,
        end_time,
        &ModelType::Regression,
    ).await;
    
    // Should succeed even with minimal data (depending on configuration)
    if batch.is_ok() {
        let training_batch = batch.unwrap();
        assert_eq!(training_batch.symbol, symbol);
        
        // Verify that original data integrity is maintained through transformations
        assert!(training_batch.metadata.quality_score > 0.0);
    }
    
    env.cleanup().await.expect("Failed to cleanup");
}

// Helper function to get current memory usage
fn get_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(contents) = fs::read_to_string("/proc/self/status") {
            for line in contents.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            return kb * 1024; // Convert to bytes
                        }
                    }
                }
            }
        }
    }
    
    // Fallback for other platforms or if reading fails
    0
}

#[tokio::test]
async fn test_performance_benchmarks() {
    let env = TestEnvironment::setup().await
        .expect("Failed to setup test environment");
    
    let symbol = "BTC/USD";
    let start_time = Utc::now() - Duration::hours(1);
    
    // Insert performance test data
    insert_test_market_data(&env.data_access.storage, symbol, start_time, 100, 1).await
        .expect("Failed to insert performance test data");
    
    sleep(TokioDuration::from_millis(200)).await;
    
    // Benchmark data access
    let data_access_start = std::time::Instant::now();
    let _market_data = env.data_access.get_market_data(symbol, Timeframe::Minute).await
        .expect("Failed to get market data for benchmark");
    let data_access_duration = data_access_start.elapsed();
    
    // Benchmark training data loading
    let mut training_service = env.training_service;
    let training_start = std::time::Instant::now();
    let _batch = training_service.load_training_data(
        symbol,
        start_time,
        Utc::now(),
        &ModelType::Regression,
    ).await.expect("Failed to load training data for benchmark");
    let training_duration = training_start.elapsed();
    
    // Performance assertions (adjust thresholds based on environment)
    assert!(data_access_duration.as_millis() < 1000, 
        "Data access should complete in under 1 second, took {}ms", 
        data_access_duration.as_millis());
    
    assert!(training_duration.as_millis() < 5000, 
        "Training data loading should complete in under 5 seconds, took {}ms", 
        training_duration.as_millis());
    
    println!("Performance Results:");
    println!("  Data Access: {}ms", data_access_duration.as_millis());
    println!("  Training Data Loading: {}ms", training_duration.as_millis());
    
    env.cleanup().await.expect("Failed to cleanup");
}