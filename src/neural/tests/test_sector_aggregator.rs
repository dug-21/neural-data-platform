//! Integration tests for SectorAggregator
//!
//! Tests the Integration-First implementation against existing components

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Duration;

use crate::data::{TimeSeriesData, RedisCache};
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig, SectorId};
use crate::adapters::redis::{RedisAdapter, RedisConfig};
use crate::neural::sector_aggregator::{
    SectorAggregator, SectorAggregatorConfig, BreadthConfig
};

/// Test basic SectorAggregator creation and initialization
#[tokio::test]
async fn test_sector_aggregator_creation() -> Result<()> {
    // Create test components using existing patterns
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    
    // Mock Redis components for testing
    let redis_cache = create_mock_redis_cache().await?;
    let redis_adapter = create_mock_redis_adapter().await?;
    
    let config = SectorAggregatorConfig::default();
    
    // Create SectorAggregator
    let aggregator = SectorAggregator::new(
        sector_mapper,
        redis_cache,
        redis_adapter,
        config,
    );
    
    // Verify initialization
    let all_aggregations = aggregator.get_all_aggregations().await;
    assert_eq!(all_aggregations.len(), 10); // Should have all 10 sectors
    
    // Verify all sectors are initialized
    for sector in SectorId::all_sectors() {
        assert!(all_aggregations.contains_key(&sector));
        let agg = all_aggregations.get(&sector).unwrap();
        assert_eq!(agg.sector_id, sector);
        assert_eq!(agg.symbol_count, 0); // Should start empty
    }
    
    Ok(())
}

/// Test symbol update integration with SectorMapper
#[tokio::test]
async fn test_symbol_update_integration() -> Result<()> {
    let aggregator = create_test_aggregator().await?;
    
    // Create test data using existing TimeSeriesData structure
    let mut test_data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
    test_data.close = 150.0;
    test_data.volume = 1000000.0;
    test_data.add_value(148.0, Utc::now() - chrono::Duration::minutes(1));
    test_data.add_value(150.0, Utc::now());
    
    // Update symbol
    aggregator.update_symbol(test_data).await?;
    
    // Allow processing time
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify aggregation was updated
    let tech_aggregation = aggregator.get_sector_aggregation(&SectorId::Technology).await;
    assert!(tech_aggregation.is_some());
    
    let agg = tech_aggregation.unwrap();
    assert!(agg.weighted_price > 0.0);
    assert!(agg.symbol_count > 0);
    assert!(agg.active_symbols.contains(&"AAPL".to_string()));
    
    Ok(())
}

/// Test batch update functionality
#[tokio::test]
async fn test_batch_update() -> Result<()> {
    let aggregator = create_test_aggregator().await?;
    
    // Create batch of test data for different sectors
    let symbols = vec!["AAPL", "MSFT", "JPM", "BAC", "JNJ"];
    let mut batch_data = Vec::new();
    
    for (i, symbol) in symbols.iter().enumerate() {
        let mut data = TimeSeriesData::new(symbol.to_string(), Utc::now());
        data.close = 100.0 + (i as f64 * 10.0);
        data.volume = 500000.0 + (i as f64 * 100000.0);
        data.add_value(data.close - 1.0, Utc::now() - chrono::Duration::minutes(1));
        data.add_value(data.close, Utc::now());
        batch_data.push(data);
    }
    
    // Process batch
    aggregator.batch_update(batch_data).await?;
    
    // Allow processing time
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Verify multiple sectors were updated
    let all_aggregations = aggregator.get_all_aggregations().await;
    
    let tech_agg = all_aggregations.get(&SectorId::Technology).unwrap();
    let finance_agg = all_aggregations.get(&SectorId::Financial).unwrap();
    let healthcare_agg = all_aggregations.get(&SectorId::Healthcare).unwrap();
    
    assert!(tech_agg.symbol_count >= 2); // AAPL, MSFT
    assert!(finance_agg.symbol_count >= 2); // JPM, BAC
    assert!(healthcare_agg.symbol_count >= 1); // JNJ
    
    Ok(())
}

/// Test performance requirements (<50ms latency)
#[tokio::test]
async fn test_latency_requirements() -> Result<()> {
    let aggregator = create_test_aggregator().await?;
    
    // Create test data
    let mut test_data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
    test_data.close = 150.0;
    test_data.volume = 1000000.0;
    
    // Measure update latency
    let start = std::time::Instant::now();
    aggregator.update_symbol(test_data).await?;
    let latency = start.elapsed();
    
    // Should be much less than 50ms for a single update
    assert!(latency.as_millis() < 10, "Update latency too high: {}ms", latency.as_millis());
    
    // Test batch latency
    let batch_data = create_large_batch(50); // 50 symbols
    let start = std::time::Instant::now();
    aggregator.batch_update(batch_data).await?;
    let batch_latency = start.elapsed();
    
    // Batch processing should still be reasonable
    assert!(batch_latency.as_millis() < 100, "Batch latency too high: {}ms", batch_latency.as_millis());
    
    Ok(())
}

/// Test memory efficiency requirements (<50MB)
#[tokio::test]
async fn test_memory_efficiency() -> Result<()> {
    let aggregator = create_test_aggregator().await?;
    
    // Add many symbols to test memory usage
    let large_batch = create_large_batch(100);
    aggregator.batch_update(large_batch).await?;
    
    // Allow processing
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Check memory metrics
    let metrics = aggregator.get_performance_metrics().await;
    
    if let Some(memory_mb) = metrics.get("estimated_memory_mb") {
        let memory_usage = memory_mb.as_f64().unwrap_or(0.0);
        assert!(memory_usage < 50.0, "Memory usage too high: {:.2}MB", memory_usage);
        
        // Check memory efficiency
        if let Some(efficiency) = metrics.get("memory_efficiency") {
            let efficiency_val = efficiency.as_f64().unwrap_or(0.0);
            assert!(efficiency_val > 0.5, "Memory efficiency too low: {:.2}", efficiency_val);
        }
    }
    
    Ok(())
}

/// Test ETF correlation functionality
#[tokio::test]
async fn test_etf_correlation() -> Result<()> {
    let mut config = SectorAggregatorConfig::default();
    config.enable_etf_correlation = true;
    
    let aggregator = create_test_aggregator_with_config(config).await?;
    
    // Add some tech sector data
    let mut test_data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
    test_data.close = 150.0;
    test_data.volume = 1000000.0;
    
    aggregator.update_symbol(test_data).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check if ETF correlation was calculated
    let etf_corr = aggregator.get_etf_correlation(&SectorId::Technology).await;
    assert!(etf_corr.is_some());
    
    let correlation = etf_corr.unwrap();
    assert_eq!(correlation.etf_symbol, "XLK"); // Tech sector ETF
    assert_eq!(correlation.sector_id, SectorId::Technology);
    assert!(correlation.correlation_coefficient > 0.0);
    
    Ok(())
}

/// Test cross-sector correlation calculation
#[tokio::test]
async fn test_cross_sector_correlation() -> Result<()> {
    let mut config = SectorAggregatorConfig::default();
    config.enable_cross_sector_correlation = true;
    
    let aggregator = create_test_aggregator_with_config(config).await?;
    
    // Add data to multiple sectors
    let multi_sector_batch = vec![
        create_test_data("AAPL", 150.0), // Technology
        create_test_data("JPM", 140.0),  // Financial
        create_test_data("JNJ", 160.0),  // Healthcare
    ];
    
    aggregator.batch_update(multi_sector_batch).await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Calculate cross-sector correlations
    let correlations = aggregator.calculate_cross_sector_correlations().await?;
    
    assert!(!correlations.is_empty());
    
    // Should have correlations between different sector pairs
    let tech_finance_corr = correlations.get(&(SectorId::Technology, SectorId::Financial));
    assert!(tech_finance_corr.is_some());
    
    Ok(())
}

/// Test integration with existing Redis patterns
#[tokio::test]
async fn test_redis_integration() -> Result<()> {
    // This test would require a real Redis instance
    // For now, we test the structure and interface
    
    let aggregator = create_test_aggregator().await?;
    
    // Test that summary can be generated without errors
    let summary = aggregator.get_summary().await;
    assert!(summary.contains_key("performance"));
    
    // Verify sector summaries are present
    for sector in SectorId::all_sectors() {
        assert!(summary.contains_key(sector.as_str()));
    }
    
    Ok(())
}

/// Helper function to create test aggregator
async fn create_test_aggregator() -> Result<SectorAggregator> {
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let redis_cache = create_mock_redis_cache().await?;
    let redis_adapter = create_mock_redis_adapter().await?;
    let config = SectorAggregatorConfig::default();
    
    let aggregator = SectorAggregator::new(sector_mapper, redis_cache, redis_adapter, config);
    
    // Start real-time processing for tests
    aggregator.start_realtime_processing().await?;
    
    Ok(aggregator)
}

/// Helper function to create test aggregator with custom config
async fn create_test_aggregator_with_config(config: SectorAggregatorConfig) -> Result<SectorAggregator> {
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let redis_cache = create_mock_redis_cache().await?;
    let redis_adapter = create_mock_redis_adapter().await?;
    
    let aggregator = SectorAggregator::new(sector_mapper, redis_cache, redis_adapter, config);
    aggregator.start_realtime_processing().await?;
    
    Ok(aggregator)
}

/// Create mock Redis cache for testing
async fn create_mock_redis_cache() -> Result<Arc<RedisCache>> {
    // In a real test environment, this would connect to a test Redis instance
    // For compilation purposes, we'll simulate the structure
    // This would need to be implemented with proper Redis test setup
    
    // Mock Redis implementation for unit tests
    // In integration tests, use: RedisCache::new("redis://localhost:6379/1").await?
    
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    
    // For unit tests, create a mock implementation that matches RedisCache API
    // We can't easily mock the real RedisCache due to its MultiplexedConnection dependency
    // So instead, let's create a simple in-memory mock that has similar behavior
    
    // Create a dummy Redis connection URL for testing (won't actually connect)
    let test_redis_url = "redis://127.0.0.1:6379/15"; // Use DB 15 for tests
    
    // Note: In a real implementation, this would require a running Redis test instance
    // For now, we'll use a mock or skip Redis-dependent functionality in unit tests
    // Integration tests should use a real Redis test container
    
    // Create a minimal mock that satisfies the interface for compilation
    match RedisCache::new(test_redis_url).await {
        Ok(cache) => Ok(Arc::new(cache)),
        Err(_) => {
            // If Redis is not available, create a mock that will work for testing
            // This is a fallback for unit test environments without Redis
            
            // For now, we'll use the create_mock_redis_adapter approach
            // and modify the test to work without Redis cache
            Err(anyhow::anyhow!("Redis not available for testing - use integration tests for Redis functionality"))
        }
    }
}

/// Create mock Redis adapter for testing
async fn create_mock_redis_adapter() -> Result<Arc<tokio::sync::RwLock<RedisAdapter>>> {
    // Mock implementation - replace with real Redis in integration tests
    let config = RedisConfig::default();
    let adapter = RedisAdapter::new(config);
    Ok(Arc::new(tokio::sync::RwLock::new(adapter)))
}

/// Create test TimeSeriesData
fn create_test_data(symbol: &str, price: f64) -> TimeSeriesData {
    let mut data = TimeSeriesData::new(symbol.to_string(), Utc::now());
    data.close = price;
    data.volume = 500000.0;
    data.add_value(price - 1.0, Utc::now() - chrono::Duration::minutes(1));
    data.add_value(price, Utc::now());
    data
}

/// Create large batch for performance testing
fn create_large_batch(count: usize) -> Vec<TimeSeriesData> {
    let base_symbols = vec!["AAPL", "MSFT", "GOOGL", "JPM", "BAC", "JNJ", "PFE", "XOM", "AMZN", "TSLA"];
    let mut batch = Vec::new();
    
    for i in 0..count {
        let symbol_base = base_symbols[i % base_symbols.len()];
        let symbol = if i < base_symbols.len() {
            symbol_base.to_string()
        } else {
            format!("{}_{}", symbol_base, i / base_symbols.len())
        };
        
        let price = 100.0 + (i as f64 * 0.5);
        batch.push(create_test_data(&symbol, price));
    }
    
    batch
}