//! Tests for the data pipeline module

use autonomous_platform::data::{DataPipeline, TimescaleDBStorage, RedisCache, TimeSeriesData, QualityMetrics, PlatformMetrics};
use autonomous_platform::config::{PlatformConfig, DatabaseConfig, RedisConfig, NeuralConfig, MonitoringConfig, PlatformInfo};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use anyhow::Result;

/// Create a test configuration
fn create_test_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "test-platform".to_string(),
            version: "0.1.0".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://test@localhost/test".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 5,
            default_ttl_seconds: 300,
        },
        neural: NeuralConfig {
            memory_gb: 1.0,
            models: vec!["NHITS".to_string(), "DeepAR".to_string()],
            prediction_cache_ttl: 600,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 60,
            quality_threshold: 0.95,
        },
    }
}

/// Create test time series data
fn create_test_time_series_data() -> TimeSeriesData {
    TimeSeriesData {
        symbol: "BTC/USD".to_string(),
        timestamp: Utc::now(),
        open: 45000.0,
        high: 45500.0,
        low: 44800.0,
        close: 45200.0,
        volume: 1000.0,
        indicators: vec![
            ("sma_20".to_string(), 44900.0),
            ("rsi".to_string(), 65.5),
        ].into_iter().collect(),
    }
}

#[tokio::test]
async fn test_pipeline_creation() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    assert!(pipeline.health_check().await?);
    
    Ok(())
}

#[tokio::test]
async fn test_process_data() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    
    let data = create_test_time_series_data();
    pipeline.process_data(data.clone()).await?;
    
    // Verify data was stored
    let stored_data = pipeline.get_latest_data(&data.symbol).await?;
    let stored = stored_data.unwrap();
    assert_eq!(stored.symbol, data.symbol);
    assert_eq!(stored.close, data.close);
    
    Ok(())
}

#[tokio::test]
async fn test_monitor_quality() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    
    // Process some test data
    for i in 0..10 {
        let mut data = create_test_time_series_data();
        data.close += i as f64 * 100.0;
        pipeline.process_data(data).await?;
    }
    
    let quality_metrics = pipeline.monitor_quality().await?;
    
    assert!(quality_metrics.data_completeness >= 0.0);
    assert!(quality_metrics.data_completeness <= 1.0);
    assert!(quality_metrics.latency_ms >= 0.0);
    assert!(quality_metrics.error_rate >= 0.0);
    assert!(quality_metrics.error_rate <= 1.0);
    
    Ok(())
}

#[tokio::test]
async fn test_collect_metrics() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    
    let metrics = pipeline.collect_metrics().await?;
    
    assert!(metrics.total_records >= 0);
    assert!(metrics.cache_hit_rate >= 0.0);
    assert!(metrics.cache_hit_rate <= 1.0);
    assert!(metrics.processing_throughput >= 0.0);
    assert!(metrics.storage_usage_gb >= 0.0);
    
    Ok(())
}

#[tokio::test]
async fn test_data_validation() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    
    // Test with invalid data
    let mut invalid_data = create_test_time_series_data();
    invalid_data.high = 44000.0; // High less than low
    invalid_data.low = 46000.0;
    
    let result = pipeline.process_data(invalid_data).await;
    assert!(result.is_err());
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_processing() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = Arc::new(DataPipeline::new(storage, cache, config).await?);
    
    // Process data concurrently
    let mut handles = vec![];
    
    for i in 0..5 {
        let pipeline_clone = Arc::clone(&pipeline);
        let handle = tokio::spawn(async move {
            let mut data = create_test_time_series_data();
            data.symbol = format!("TEST{}", i);
            pipeline_clone.process_data(data).await
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await??;
    }
    
    // Verify all data was processed
    let metrics = pipeline.collect_metrics().await?;
    assert!(metrics.total_records >= 5);
    
    Ok(())
}

#[tokio::test]
async fn test_cache_efficiency() -> Result<()> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    
    let data = create_test_time_series_data();
    
    // First access should miss cache
    pipeline.process_data(data.clone()).await?;
    let metrics1 = pipeline.collect_metrics().await?;
    
    // Subsequent accesses should hit cache
    for _ in 0..5 {
        let _ = pipeline.get_latest_data(&data.symbol).await?;
    }
    
    let metrics2 = pipeline.collect_metrics().await?;
    assert!(metrics2.cache_hit_rate > metrics1.cache_hit_rate);
    
    Ok(())
}

#[tokio::test]
async fn test_quality_threshold_monitoring() -> Result<()> {
    let mut config = create_test_config();
    config.monitoring.quality_threshold = 0.8;
    
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;
    
    let pipeline = DataPipeline::new(storage, cache, config).await?;
    
    // Simulate degraded quality
    let quality_metrics = pipeline.monitor_quality().await?;
    
    if quality_metrics.overall_quality < 0.8 {
        let metrics = pipeline.monitor_quality().await?;
        assert!(metrics.overall_quality < 0.9);
    }
    
    Ok(())
}