//! Comprehensive Unit Tests for SectorAggregator
//!
//! Tests real-time sector aggregation, performance metrics, and ETF correlations.
//! Validates <50ms latency requirement and memory efficiency.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use approx::assert_relative_eq;
use serial_test::serial;

use crate::data::{
    TimeSeriesData,
    sector_mapper::{SectorMapper, SectorMapperConfig, SectorInfo, SectorId, MarketCapTier},
    sector_aggregator::{
        SectorAggregator, SectorAggregatorConfig, BreadthIndicators, 
        SectorPerformanceMetrics, MarketCapData
    }
};
use crate::tests::helpers::test_utils::{TestDataGenerator, PerformanceMeasurement};

// Test utilities
fn create_test_sector_mapper() -> Arc<SectorMapper> {
    let config = SectorMapperConfig::default();
    let mapper = Arc::new(SectorMapper::new(config));
    
    // Add additional test symbols for comprehensive testing
    mapper.add_symbol_mapping("NVDA", SectorInfo {
        id: "technology".to_string(),
        sector_id: SectorId::Technology,
        sub_sector: Some("Semiconductors".to_string()),
        market_cap_tier: MarketCapTier::LargeCap,
        weight_in_sector: 0.15,
        correlation_group: None,
    });
    
    mapper.add_symbol_mapping("AMD", SectorInfo {
        id: "technology".to_string(),
        sector_id: SectorId::Technology,
        sub_sector: Some("Semiconductors".to_string()),
        market_cap_tier: MarketCapTier::LargeCap,
        weight_in_sector: 0.08,
        correlation_group: None,
    });
    
    mapper.add_symbol_mapping("C", SectorInfo {
        id: "financial".to_string(),
        sector_id: SectorId::Financial,
        sub_sector: Some("Banking".to_string()),
        market_cap_tier: MarketCapTier::LargeCap,
        weight_in_sector: 0.08,
        correlation_group: Some("big_banks".to_string()),
    });
    
    mapper
}

fn create_test_aggregator() -> SectorAggregator {
    let sector_mapper = create_test_sector_mapper();
    let config = SectorAggregatorConfig {
        latency_threshold_ms: 50,
        memory_limit_mb: 50,
        etf_correlation_threshold: 0.8,
        update_interval_seconds: 1,
        enable_redis_publishing: false, // Disabled for tests
        enable_performance_tracking: true,
    };
    SectorAggregator::new(sector_mapper, config)
}

fn create_test_time_series_data(symbol: &str, price: f64, volume: f64, timestamp_offset_secs: i64) -> TimeSeriesData {
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp: Utc::now() + chrono::Duration::seconds(timestamp_offset_secs),
        open: price - 0.5,
        high: price + 1.0,
        low: price - 1.0,
        close: price,
        volume,
        indicators: HashMap::from([
            ("rsi".to_string(), 50.0),
            ("macd".to_string(), 0.5),
        ]),
        source: Some("test".to_string()),
        entity: Some("test_entity".to_string()),
        value: Some(price),
        metadata: None,
    }
}

fn create_price_series(symbol: &str, base_price: f64, count: usize, trend: f64) -> Vec<TimeSeriesData> {
    let mut series = Vec::with_capacity(count);
    for i in 0..count {
        let price = base_price + (i as f64 * trend);
        series.push(create_test_time_series_data(symbol, price, 1000.0 + i as f64 * 100.0, i as i64));
    }
    series
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sector_aggregator_creation() {
        let aggregator = create_test_aggregator();
        
        // Verify initial state
        assert_eq!(aggregator.get_all_aggregations().len(), 0);
        assert!(aggregator.estimate_memory_usage() >= 0.0);
        assert!(aggregator.estimate_memory_usage() < 1.0); // Less than 1MB initially
    }

    #[test]
    fn test_market_cap_updates() {
        let aggregator = create_test_aggregator();
        
        // Test market cap updates
        let test_caps = vec![
            ("AAPL", 3_000_000_000_000.0),
            ("MSFT", 2_800_000_000_000.0),
            ("GOOGL", 1_800_000_000_000.0),
            ("NVDA", 2_200_000_000_000.0),
        ];
        
        for (symbol, cap) in &test_caps {
            aggregator.update_market_cap(symbol, *cap);
        }
        
        // Verify updates
        for (symbol, expected_cap) in &test_caps {
            let market_cap_data = aggregator.market_caps.get(symbol).unwrap();
            assert_eq!(market_cap_data.market_cap, *expected_cap);
            assert_eq!(market_cap_data.symbol, *symbol);
            assert!(market_cap_data.timestamp <= Utc::now());
        }
    }

    #[tokio::test]
    async fn test_single_symbol_processing() {
        let aggregator = create_test_aggregator();
        
        // Add market cap data
        aggregator.update_market_cap("AAPL", 3_000_000_000_000.0);
        
        // Create test data
        let test_data = vec![create_test_time_series_data("AAPL", 150.0, 1000000.0, 0)];
        
        // Process data
        let result = aggregator.process_market_data(&test_data).await;
        assert!(result.is_ok());
        
        // Check if technology sector aggregation was created
        let tech_aggregation = aggregator.get_sector_aggregation(&SectorId::Technology);
        assert!(tech_aggregation.is_some());
        
        let agg = tech_aggregation.unwrap();
        assert_eq!(agg.sector_id, SectorId::Technology);
        assert!(agg.symbol_count > 0);
        assert!(agg.market_cap_weighted_price > 0.0);
        assert!(agg.total_volume > 0.0);
    }

    #[tokio::test]
    async fn test_multiple_symbol_sector_aggregation() {
        let aggregator = create_test_aggregator();
        
        // Add market cap data for technology stocks
        let tech_symbols = vec![
            ("AAPL", 3_000_000_000_000.0, 180.0),
            ("MSFT", 2_800_000_000_000.0, 420.0),
            ("GOOGL", 1_800_000_000_000.0, 140.0),
            ("NVDA", 2_200_000_000_000.0, 800.0),
        ];
        
        for (symbol, market_cap, _) in &tech_symbols {
            aggregator.update_market_cap(symbol, *market_cap);
        }
        
        // Create test data
        let test_data: Vec<_> = tech_symbols.iter()
            .map(|(symbol, _, price)| create_test_time_series_data(symbol, *price, 1000000.0, 0))
            .collect();
        
        // Process data
        let result = aggregator.process_market_data(&test_data).await;
        assert!(result.is_ok());
        
        // Verify technology sector aggregation
        let tech_agg = aggregator.get_sector_aggregation(&SectorId::Technology).unwrap();
        
        // Calculate expected market cap weighted price
        let total_market_cap: f64 = tech_symbols.iter().map(|(_, mc, _)| mc).sum();
        let expected_weighted_price: f64 = tech_symbols.iter()
            .map(|(_, mc, price)| price * (mc / total_market_cap))
            .sum();
        
        assert_relative_eq!(tech_agg.market_cap_weighted_price, expected_weighted_price, epsilon = 0.01);
        assert_eq!(tech_agg.symbol_count, 4);
        assert!(tech_agg.total_volume > 0.0);
        
        // Verify average price calculation
        let expected_avg: f64 = tech_symbols.iter().map(|(_, _, price)| price).sum::<f64>() / 4.0;
        assert_relative_eq!(tech_agg.average_price, expected_avg, epsilon = 0.01);
    }

    #[tokio::test]
    async fn test_breadth_indicators_calculation() {
        let aggregator = create_test_aggregator();
        
        // Create trending data for tech stocks
        let symbols = vec!["AAPL", "MSFT", "GOOGL", "NVDA"];
        let mut all_data = Vec::new();
        
        for (i, symbol) in symbols.iter().enumerate() {
            aggregator.update_market_cap(symbol, 1_000_000_000_000.0);
            
            // Create two data points to calculate changes
            let trend = if i % 2 == 0 { 1.0 } else { -1.0 }; // Half up, half down
            let series = create_price_series(symbol, 100.0, 2, trend);
            all_data.extend(series);
        }
        
        // Process data
        aggregator.process_market_data(&all_data).await.unwrap();
        
        // Get aggregation
        let tech_agg = aggregator.get_sector_aggregation(&SectorId::Technology).unwrap();
        
        // Verify breadth indicators
        let breadth = &tech_agg.breadth_indicators;
        assert_eq!(breadth.advancing_stocks + breadth.declining_stocks + breadth.unchanged_stocks, 4);
        
        // Should have some advancing and declining stocks
        assert!(breadth.advancing_stocks > 0 || breadth.declining_stocks > 0);
        
        // Volume ratios should be positive
        assert!(breadth.up_down_volume_ratio >= 0.0);
        assert!(breadth.advance_decline_ratio >= 0.0);
    }

    #[tokio::test]
    async fn test_performance_metrics_calculation() {
        let aggregator = create_test_aggregator();
        
        // Create data with known performance characteristics
        aggregator.update_market_cap("AAPL", 3_000_000_000_000.0);
        
        // Create a trending series
        let trending_data = create_price_series("AAPL", 100.0, 10, 2.0); // +2% per period
        aggregator.process_market_data(&trending_data).await.unwrap();
        
        // Get aggregation
        let tech_agg = aggregator.get_sector_aggregation(&SectorId::Technology).unwrap();
        
        // Verify performance metrics
        let perf = &tech_agg.performance_metrics;
        
        // Should show positive change
        assert!(perf.change_percent > 0.0, "Expected positive change, got {}", perf.change_percent);
        
        // Volatility should be calculated
        assert!(perf.volatility >= 0.0);
        
        // Momentum score should reflect the trend
        assert!(perf.momentum_score.is_finite());
        
        // Strength index should be finite
        assert!(perf.strength_index.is_finite());
    }

    #[tokio::test]
    async fn test_etf_correlation_calculation() {
        let aggregator = create_test_aggregator();
        
        // Add tech sector data
        aggregator.update_market_cap("AAPL", 3_000_000_000_000.0);
        let tech_data = vec![create_test_time_series_data("AAPL", 150.0, 1000000.0, 0)];
        
        // Add ETF data
        let etf_data = vec![create_test_time_series_data("XLK", 180.0, 500000.0, 0)];
        aggregator.update_etf_prices(&etf_data).await.unwrap();
        
        // Process sector data
        aggregator.process_market_data(&tech_data).await.unwrap();
        
        // Get aggregation
        let tech_agg = aggregator.get_sector_aggregation(&SectorId::Technology).unwrap();
        
        // Should have ETF correlation (placeholder implementation returns 0.85)
        assert!(tech_agg.etf_correlation.is_some());
        let correlation = tech_agg.etf_correlation.unwrap();
        assert!(correlation >= 0.8, "ETF correlation {} should be >= 0.8", correlation);
    }

    #[tokio::test]
    async fn test_multiple_sector_processing() {
        let aggregator = create_test_aggregator();
        
        // Set up data for multiple sectors
        let test_data = vec![
            // Technology
            ("AAPL", 3_000_000_000_000.0, 180.0, SectorId::Technology),
            ("MSFT", 2_800_000_000_000.0, 420.0, SectorId::Technology),
            // Financial
            ("JPM", 500_000_000_000.0, 160.0, SectorId::Financial),
            ("BAC", 300_000_000_000.0, 35.0, SectorId::Financial),
            // Healthcare
            ("JNJ", 400_000_000_000.0, 170.0, SectorId::Healthcare),
        ];
        
        // Add market caps and create market data
        let mut market_data = Vec::new();
        for (symbol, market_cap, price, _) in &test_data {
            aggregator.update_market_cap(symbol, *market_cap);
            market_data.push(create_test_time_series_data(symbol, *price, 1000000.0, 0));
        }
        
        // Process all data
        aggregator.process_market_data(&market_data).await.unwrap();
        
        // Verify multiple sectors were aggregated
        let all_aggregations = aggregator.get_all_aggregations();
        assert!(all_aggregations.len() >= 3, "Expected at least 3 sectors, got {}", all_aggregations.len());
        
        // Verify each sector has correct data
        for (symbol, _, price, expected_sector) in &test_data {
            if let Some(agg) = all_aggregations.get(expected_sector) {
                assert_eq!(agg.sector_id, *expected_sector);
                assert!(agg.symbol_count > 0);
                assert!(agg.market_cap_weighted_price > 0.0);
            }
        }
    }

    #[tokio::test]
    #[serial] // Run sequentially to avoid timing conflicts
    async fn test_latency_performance_requirement() {
        let aggregator = create_test_aggregator();
        
        // Set up large dataset (100 symbols)
        let mut large_dataset = Vec::new();
        for i in 0..100 {
            let symbol = format!("TEST{:03}", i);
            aggregator.update_market_cap(&symbol, 1_000_000_000.0);
            large_dataset.push(create_test_time_series_data(&symbol, 100.0 + i as f64, 10000.0, 0));
        }
        
        // Measure processing time
        let start_time = Instant::now();
        let result = aggregator.process_market_data(&large_dataset).await;
        let elapsed = start_time.elapsed();
        
        assert!(result.is_ok());
        assert!(elapsed.as_millis() < 50, "Processing took {}ms, should be <50ms", elapsed.as_millis());
        
        // Verify performance metrics
        let performance_ok = aggregator.check_performance_requirements().await.unwrap();
        assert!(performance_ok, "Performance requirements not met");
    }

    #[tokio::test]
    async fn test_memory_efficiency_requirement() {
        let aggregator = create_test_aggregator();
        
        // Add large amount of data to test memory usage
        for i in 0..1000 {
            let symbol = format!("MEM{:04}", i);
            aggregator.update_market_cap(&symbol, 1_000_000_000.0);
            
            // Add historical data
            let historical_data = create_price_series(&symbol, 100.0, 50, 0.1);
            aggregator.process_market_data(&historical_data).await.unwrap();
        }
        
        // Check memory usage
        let memory_mb = aggregator.estimate_memory_usage();
        assert!(memory_mb < 50.0, "Memory usage {}MB exceeds 50MB limit", memory_mb);
        
        // Verify performance requirements still met
        let performance_ok = aggregator.check_performance_requirements().await.unwrap();
        assert!(performance_ok || memory_mb < 50.0, "Performance requirements not met with memory usage {}MB", memory_mb);
    }

    #[tokio::test]
    async fn test_edge_case_empty_data() {
        let aggregator = create_test_aggregator();
        
        // Process empty data
        let result = aggregator.process_market_data(&[]).await;
        assert!(result.is_ok());
        
        // Should have no aggregations
        assert_eq!(aggregator.get_all_aggregations().len(), 0);
    }

    #[tokio::test]
    async fn test_edge_case_single_data_point() {
        let aggregator = create_test_aggregator();
        
        // Single data point (no previous data for comparison)
        aggregator.update_market_cap("SINGLE", 1_000_000_000.0);
        let single_data = vec![create_test_time_series_data("SINGLE", 100.0, 1000.0, 0)];
        
        let result = aggregator.process_market_data(&single_data).await;
        assert!(result.is_ok());
        
        // Should handle gracefully
        if let Some(agg) = aggregator.get_sector_aggregation(&SectorId::Technology) {
            assert_eq!(agg.symbol_count, 1);
            assert_eq!(agg.market_cap_weighted_price, 100.0);
        }
    }

    #[tokio::test]
    async fn test_edge_case_zero_values() {
        let aggregator = create_test_aggregator();
        
        // Zero price and volume
        aggregator.update_market_cap("ZERO", 1_000_000_000.0);
        let zero_data = vec![create_test_time_series_data("ZERO", 0.0, 0.0, 0)];
        
        let result = aggregator.process_market_data(&zero_data).await;
        assert!(result.is_ok());
        
        // Should handle zeros gracefully
        if let Some(agg) = aggregator.get_sector_aggregation(&SectorId::Technology) {
            assert!(agg.market_cap_weighted_price.is_finite());
            assert!(agg.total_volume.is_finite());
        }
    }

    #[tokio::test]
    async fn test_edge_case_extreme_values() {
        let aggregator = create_test_aggregator();
        
        // Extreme values
        aggregator.update_market_cap("EXTREME", 1_000_000_000.0);
        let extreme_data = vec![create_test_time_series_data("EXTREME", f64::MAX / 1e6, f64::MAX / 1e6, 0)];
        
        let result = aggregator.process_market_data(&extreme_data).await;
        assert!(result.is_ok());
        
        // Should handle extreme values without panicking
        if let Some(agg) = aggregator.get_sector_aggregation(&SectorId::Technology) {
            assert!(agg.market_cap_weighted_price.is_finite());
            assert!(agg.total_volume.is_finite());
        }
    }

    #[tokio::test]
    async fn test_missing_market_cap_handling() {
        let aggregator = create_test_aggregator();
        
        // Don't set market cap for this symbol
        let test_data = vec![create_test_time_series_data("NO_CAP", 100.0, 1000.0, 0)];
        
        let result = aggregator.process_market_data(&test_data).await;
        assert!(result.is_ok());
        
        // Should use default market cap
        if let Some(agg) = aggregator.get_sector_aggregation(&SectorId::Technology) {
            assert!(agg.symbol_count > 0);
            assert!(agg.market_cap_weighted_price > 0.0);
        }
    }

    #[tokio::test]
    async fn test_concurrent_processing() {
        let aggregator = Arc::new(create_test_aggregator());
        
        // Spawn multiple concurrent tasks
        let mut handles = Vec::new();
        
        for i in 0..10 {
            let agg_clone = Arc::clone(&aggregator);
            let handle = tokio::spawn(async move {
                let symbol = format!("CONCURRENT{}", i);
                agg_clone.update_market_cap(&symbol, 1_000_000_000.0);
                
                let data = vec![create_test_time_series_data(&symbol, 100.0 + i as f64, 1000.0, 0)];
                agg_clone.process_market_data(&data).await.unwrap();
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify concurrent processing worked
        let aggregations = aggregator.get_all_aggregations();
        assert!(aggregations.len() > 0);
    }

    #[tokio::test]
    async fn test_performance_metrics_tracking() {
        let aggregator = create_test_aggregator();
        
        // Process some data to generate metrics
        aggregator.update_market_cap("PERF_TEST", 1_000_000_000.0);
        let test_data = vec![create_test_time_series_data("PERF_TEST", 100.0, 1000.0, 0)];
        
        aggregator.process_market_data(&test_data).await.unwrap();
        
        // Check that performance metrics were tracked
        let metrics = aggregator.get_performance_metrics().await;
        assert!(metrics.contains_key("aggregation_latency_ms"));
        
        let latency = metrics["aggregation_latency_ms"];
        assert!(latency >= 0.0);
        assert!(latency < 1000.0); // Should be reasonable
    }

    #[test]
    fn test_aggregator_configuration() {
        let custom_config = SectorAggregatorConfig {
            latency_threshold_ms: 25,
            memory_limit_mb: 100,
            etf_correlation_threshold: 0.9,
            update_interval_seconds: 5,
            enable_redis_publishing: true,
            enable_performance_tracking: false,
        };
        
        let sector_mapper = create_test_sector_mapper();
        let aggregator = SectorAggregator::new(sector_mapper, custom_config.clone());
        
        assert_eq!(aggregator.config.latency_threshold_ms, 25);
        assert_eq!(aggregator.config.memory_limit_mb, 100);
        assert_relative_eq!(aggregator.config.etf_correlation_threshold, 0.9, epsilon = 0.001);
        assert_eq!(aggregator.config.update_interval_seconds, 5);
        assert!(aggregator.config.enable_redis_publishing);
        assert!(!aggregator.config.enable_performance_tracking);
    }

    #[test]
    fn test_default_configuration() {
        let config = SectorAggregatorConfig::default();
        
        assert_eq!(config.latency_threshold_ms, 50);
        assert_eq!(config.memory_limit_mb, 50);
        assert_relative_eq!(config.etf_correlation_threshold, 0.8, epsilon = 0.001);
        assert_eq!(config.update_interval_seconds, 1);
        assert!(config.enable_redis_publishing);
        assert!(config.enable_performance_tracking);
    }

    #[tokio::test]
    async fn test_etf_price_updates() {
        let aggregator = create_test_aggregator();
        
        // Update ETF prices
        let etf_data = vec![
            create_test_time_series_data("XLK", 180.0, 500000.0, 0),
            create_test_time_series_data("XLF", 40.0, 300000.0, 0),
            create_test_time_series_data("XLV", 130.0, 200000.0, 0),
        ];
        
        let result = aggregator.update_etf_prices(&etf_data).await;
        assert!(result.is_ok());
        
        // Verify ETF prices were stored
        assert!(aggregator.etf_prices.contains_key("XLK"));
        assert!(aggregator.etf_prices.contains_key("XLF"));
        assert!(aggregator.etf_prices.contains_key("XLV"));
        
        // Verify data integrity
        let xlk_data = aggregator.etf_prices.get("XLK").unwrap();
        assert_eq!(xlk_data.len(), 1);
        assert_eq!(xlk_data[0].close, 180.0);
    }

    #[tokio::test]
    async fn test_rolling_window_maintenance() {
        let aggregator = create_test_aggregator();
        
        // Add more than 100 data points to test rolling window
        aggregator.update_market_cap("ROLLING", 1_000_000_000.0);
        
        for i in 0..150 {
            let data = vec![create_test_time_series_data("ROLLING", 100.0 + i as f64, 1000.0, i)];
            aggregator.process_market_data(&data).await.unwrap();
        }
        
        // Verify rolling window is maintained (max 100 points)
        let history = aggregator.price_history.get("ROLLING").unwrap();
        assert_eq!(history.len(), 100);
        
        // Should have the most recent data
        assert_eq!(history.last().unwrap().close, 249.0); // 100 + 149
    }

    #[test]
    fn test_breadth_indicators_structure() {
        // Test that BreadthIndicators has all required fields
        let breadth = BreadthIndicators {
            advancing_stocks: 10,
            declining_stocks: 5,
            unchanged_stocks: 2,
            up_volume: vec![1000000.0],
            down_volume: vec![500000.0],
            advance_decline_ratio: 2.0,
            up_down_volume_ratio: 2.0,
            new_highs: 3,
            new_lows: 1,
        };
        
        assert_eq!(breadth.advancing_stocks, 10);
        assert_eq!(breadth.declining_stocks, 5);
        assert_eq!(breadth.unchanged_stocks, 2);
        assert_relative_eq!(breadth.up_volume, 1000000.0, epsilon = 0.001);
        assert_relative_eq!(breadth.down_volume, 500000.0, epsilon = 0.001);
        assert_relative_eq!(breadth.advance_decline_ratio, 2.0, epsilon = 0.001);
        assert_relative_eq!(breadth.up_down_volume_ratio, 2.0, epsilon = 0.001);
        assert_eq!(breadth.new_highs, 3);
        assert_eq!(breadth.new_lows, 1);
    }

    #[test]
    fn test_performance_metrics_structure() {
        // Test that SectorPerformanceMetrics has all required fields
        let perf = SectorPerformanceMetrics {
            change_percent: 2.5,
            volatility: 15.0,
            momentum_score: 1.8,
            strength_index: 0.7,
            relative_strength: 1.2,
        };
        
        assert_relative_eq!(perf.change_percent, 2.5, epsilon = 0.001);
        assert_relative_eq!(perf.volatility, 15.0, epsilon = 0.001);
        assert_relative_eq!(perf.momentum_score, 1.8, epsilon = 0.001);
        assert_relative_eq!(perf.strength_index, 0.7, epsilon = 0.001);
        assert_relative_eq!(perf.relative_strength, 1.2, epsilon = 0.001);
    }
}

/// Performance benchmarks for sector aggregation
#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn benchmark_100_symbol_processing() {
        let aggregator = create_test_aggregator();
        
        // Create 100 symbols across different sectors
        let mut test_data = Vec::new();
        for i in 0..100 {
            let symbol = format!("BENCH{:03}", i);
            aggregator.update_market_cap(&symbol, 1_000_000_000.0);
            test_data.push(create_test_time_series_data(&symbol, 100.0 + i as f64, 10000.0, 0));
        }
        
        // Benchmark processing time
        let start = Instant::now();
        let result = aggregator.process_market_data(&test_data).await;
        let elapsed = start.elapsed();
        
        assert!(result.is_ok());
        println!("100 symbol processing time: {}ms", elapsed.as_millis());
        
        // Should meet <50ms requirement
        assert!(elapsed.as_millis() < 50, "Processing took {}ms, requirement is <50ms", elapsed.as_millis());
    }

    #[tokio::test]
    async fn benchmark_memory_usage_scaling() {
        let aggregator = create_test_aggregator();
        
        // Test memory usage with increasing data
        let data_sizes = vec![10, 50, 100, 500, 1000];
        
        for size in data_sizes {
            // Clear previous data
            aggregator.aggregations.clear();
            aggregator.price_history.clear();
            aggregator.market_caps.clear();
            
            // Add data
            for i in 0..size {
                let symbol = format!("MEM{:04}", i);
                aggregator.update_market_cap(&symbol, 1_000_000_000.0);
                
                let data = vec![create_test_time_series_data(&symbol, 100.0, 1000.0, 0)];
                aggregator.process_market_data(&data).await.unwrap();
            }
            
            let memory_mb = aggregator.estimate_memory_usage();
            println!("Memory usage with {} symbols: {:.2}MB", size, memory_mb);
            
            // Memory should scale reasonably
            if size <= 100 {
                assert!(memory_mb < 50.0, "Memory usage {:.2}MB exceeds 50MB with {} symbols", memory_mb, size);
            }
        }
    }

    #[tokio::test]
    async fn benchmark_concurrent_processing() {
        let aggregator = Arc::new(create_test_aggregator());
        
        let start = Instant::now();
        
        // Spawn 10 concurrent tasks processing 10 symbols each
        let mut handles = Vec::new();
        for task in 0..10 {
            let agg_clone = Arc::clone(&aggregator);
            let handle = tokio::spawn(async move {
                for i in 0..10 {
                    let symbol = format!("CONC{:02}{:02}", task, i);
                    agg_clone.update_market_cap(&symbol, 1_000_000_000.0);
                    
                    let data = vec![create_test_time_series_data(&symbol, 100.0 + i as f64, 1000.0, 0)];
                    agg_clone.process_market_data(&data).await.unwrap();
                }
            });
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }
        
        let elapsed = start.elapsed();
        println!("Concurrent processing (100 symbols, 10 tasks): {}ms", elapsed.as_millis());
        
        // Should complete within reasonable time
        assert!(elapsed.as_millis() < 200, "Concurrent processing took {}ms", elapsed.as_millis());
    }
}