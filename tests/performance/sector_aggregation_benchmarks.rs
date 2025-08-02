//! Performance Benchmarks for Sector Aggregation System
//!
//! Comprehensive performance testing to validate <50ms latency requirements
//! and memory efficiency for 100+ symbols.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use crate::data::{
    TimeSeriesData,
    sector_mapper::{SectorMapper, SectorMapperConfig, SectorInfo, SectorId, MarketCapTier},
    sector_aggregator::{SectorAggregator, SectorAggregatorConfig}
};

// Test data generation utilities
fn create_comprehensive_sector_mapper() -> Arc<SectorMapper> {
    let config = SectorMapperConfig::default();
    let mapper = Arc::new(SectorMapper::new(config));
    
    // Add comprehensive test symbols across all sectors
    let test_symbols = vec![
        // Technology (30 symbols)
        ("AAPL", SectorId::Technology, 0.15), ("MSFT", SectorId::Technology, 0.14),
        ("GOOGL", SectorId::Technology, 0.08), ("AMZN", SectorId::Technology, 0.07),
        ("NVDA", SectorId::Technology, 0.06), ("META", SectorId::Technology, 0.05),
        ("TSLA", SectorId::Technology, 0.04), ("NFLX", SectorId::Technology, 0.03),
        ("CRM", SectorId::Technology, 0.02), ("ORCL", SectorId::Technology, 0.02),
        ("ADBE", SectorId::Technology, 0.02), ("INTC", SectorId::Technology, 0.02),
        ("AMD", SectorId::Technology, 0.02), ("QCOM", SectorId::Technology, 0.02),
        ("TXN", SectorId::Technology, 0.015), ("AVGO", SectorId::Technology, 0.015),
        ("IBM", SectorId::Technology, 0.015), ("MU", SectorId::Technology, 0.015),
        ("LRCX", SectorId::Technology, 0.01), ("KLAC", SectorId::Technology, 0.01),
        ("AMAT", SectorId::Technology, 0.01), ("ADI", SectorId::Technology, 0.01),
        ("MRVL", SectorId::Technology, 0.01), ("SNPS", SectorId::Technology, 0.01),
        ("CDNS", SectorId::Technology, 0.01), ("FTNT", SectorId::Technology, 0.01),
        ("CRWD", SectorId::Technology, 0.01), ("ZS", SectorId::Technology, 0.01),
        ("OKTA", SectorId::Technology, 0.01), ("NET", SectorId::Technology, 0.01),
        
        // Financial (25 symbols)
        ("JPM", SectorId::Financial, 0.12), ("BAC", SectorId::Financial, 0.10),
        ("WFC", SectorId::Financial, 0.08), ("GS", SectorId::Financial, 0.06),
        ("MS", SectorId::Financial, 0.05), ("C", SectorId::Financial, 0.05),
        ("BLK", SectorId::Financial, 0.04), ("SPGI", SectorId::Financial, 0.04),
        ("AXP", SectorId::Financial, 0.04), ("USB", SectorId::Financial, 0.03),
        ("TFC", SectorId::Financial, 0.03), ("PNC", SectorId::Financial, 0.03),
        ("SCHW", SectorId::Financial, 0.03), ("CB", SectorId::Financial, 0.03),
        ("MMC", SectorId::Financial, 0.03), ("ICE", SectorId::Financial, 0.02),
        ("CME", SectorId::Financial, 0.02), ("MCO", SectorId::Financial, 0.02),
        ("AON", SectorId::Financial, 0.02), ("TRV", SectorId::Financial, 0.02),
        ("ALL", SectorId::Financial, 0.02), ("MET", SectorId::Financial, 0.02),
        ("PRU", SectorId::Financial, 0.02), ("AIG", SectorId::Financial, 0.02),
        ("AFL", SectorId::Financial, 0.02),
        
        // Healthcare (20 symbols)
        ("UNH", SectorId::Healthcare, 0.15), ("JNJ", SectorId::Healthcare, 0.12),
        ("PFE", SectorId::Healthcare, 0.08), ("ABBV", SectorId::Healthcare, 0.07),
        ("TMO", SectorId::Healthcare, 0.06), ("ABT", SectorId::Healthcare, 0.06),
        ("BMY", SectorId::Healthcare, 0.05), ("MRK", SectorId::Healthcare, 0.05),
        ("LLY", SectorId::Healthcare, 0.05), ("MDT", SectorId::Healthcare, 0.04),
        ("GILD", SectorId::Healthcare, 0.04), ("AMGN", SectorId::Healthcare, 0.04),
        ("CVS", SectorId::Healthcare, 0.04), ("CI", SectorId::Healthcare, 0.03),
        ("HUM", SectorId::Healthcare, 0.03), ("ANTM", SectorId::Healthcare, 0.03),
        ("ISRG", SectorId::Healthcare, 0.03), ("DHR", SectorId::Healthcare, 0.03),
        ("SYK", SectorId::Healthcare, 0.02), ("EW", SectorId::Healthcare, 0.02),
        
        // Energy (15 symbols)
        ("XOM", SectorId::Energy, 0.20), ("CVX", SectorId::Energy, 0.15),
        ("COP", SectorId::Energy, 0.10), ("EOG", SectorId::Energy, 0.08),
        ("SLB", SectorId::Energy, 0.07), ("PSX", SectorId::Energy, 0.06),
        ("VLO", SectorId::Energy, 0.06), ("MPC", SectorId::Energy, 0.05),
        ("KMI", SectorId::Energy, 0.05), ("WMB", SectorId::Energy, 0.05),
        ("OKE", SectorId::Energy, 0.04), ("BKR", SectorId::Energy, 0.03),
        ("HAL", SectorId::Energy, 0.03), ("DVN", SectorId::Energy, 0.02),
        ("FANG", SectorId::Energy, 0.02),
        
        // Consumer Discretionary (15 symbols)  
        ("AMZN", SectorId::ConsumerDiscretionary, 0.25), ("TSLA", SectorId::ConsumerDiscretionary, 0.15),
        ("HD", SectorId::ConsumerDiscretionary, 0.10), ("MCD", SectorId::ConsumerDiscretionary, 0.08),
        ("NKE", SectorId::ConsumerDiscretionary, 0.06), ("SBUX", SectorId::ConsumerDiscretionary, 0.05),
        ("LOW", SectorId::ConsumerDiscretionary, 0.05), ("TJX", SectorId::ConsumerDiscretionary, 0.04),
        ("BKNG", SectorId::ConsumerDiscretionary, 0.04), ("ORLY", SectorId::ConsumerDiscretionary, 0.03),
        ("AZO", SectorId::ConsumerDiscretionary, 0.03), ("GM", SectorId::ConsumerDiscretionary, 0.03),
        ("F", SectorId::ConsumerDiscretionary, 0.03), ("CCL", SectorId::ConsumerDiscretionary, 0.03),
        ("RCL", SectorId::ConsumerDiscretionary, 0.03),
    ];
    
    for (symbol, sector_id, weight) in test_symbols {
        mapper.add_symbol_mapping(symbol, SectorInfo {
            id: sector_id.as_str().to_string(),
            sector_id,
            sub_sector: Some("Benchmark Test".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: weight,
            correlation_group: None,
        });
    }
    
    mapper
}

fn create_benchmark_aggregator() -> SectorAggregator {
    let sector_mapper = create_comprehensive_sector_mapper();
    let config = SectorAggregatorConfig {
        latency_threshold_ms: 50,
        memory_limit_mb: 50,
        etf_correlation_threshold: 0.8,
        update_interval_seconds: 1,
        enable_redis_publishing: false, // Disabled for benchmarks
        enable_performance_tracking: true,
    };
    SectorAggregator::new(sector_mapper, config)
}

fn create_market_data_batch(symbols: &[&str], base_price: f64, batch_size: usize) -> Vec<TimeSeriesData> {
    let mut batch = Vec::with_capacity(symbols.len() * batch_size);
    
    for &symbol in symbols {
        for i in 0..batch_size {
            let price_variation = (i as f64 * 0.01) - 0.5; // Small random variation
            let price = base_price + price_variation;
            
            batch.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: Utc::now() + chrono::Duration::seconds(i as i64),
                open: price - 0.25,
                high: price + 0.50,
                low: price - 0.50,
                close: price,
                volume: 100_000.0 + (i as f64 * 1000.0),
                indicators: HashMap::from([
                    ("rsi".to_string(), 50.0 + (i as f64 % 50.0)),
                    ("macd".to_string(), (i as f64 % 20.0) - 10.0),
                ]),
                source: Some("benchmark".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(price),
                metadata: None,
                values: vec![price],
                timestamps: vec![Utc::now()],
                metadata_map: HashMap::new(),
            });
        }
    }
    
    batch
}

// Core benchmarks
fn benchmark_single_symbol_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("single_symbol_processing", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            aggregator.update_market_cap("AAPL", 3_000_000_000_000.0);
            
            let data = create_market_data_batch(&["AAPL"], 150.0, 1);
            aggregator.process_market_data(&data).await.unwrap();
        });
    });
}

fn benchmark_small_batch_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let symbols = vec!["AAPL", "MSFT", "GOOGL", "NVDA", "JPM"];
    
    c.bench_function("small_batch_processing_5_symbols", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            // Set market caps
            for symbol in &symbols {
                aggregator.update_market_cap(symbol, 2_000_000_000_000.0);
            }
            
            let data = create_market_data_batch(&symbols, 150.0, 1);
            aggregator.process_market_data(&data).await.unwrap();
        });
    });
}

fn benchmark_medium_batch_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let symbols: Vec<&str> = vec![
        "AAPL", "MSFT", "GOOGL", "NVDA", "JPM", "BAC", "JNJ", "PFE", "XOM", "CVX",
        "UNH", "ABBV", "TMO", "ABT", "BMY", "MRK", "LLY", "MDT", "GILD", "AMGN"
    ];
    
    c.bench_function("medium_batch_processing_20_symbols", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            // Set market caps
            for symbol in &symbols {
                aggregator.update_market_cap(symbol, 1_500_000_000_000.0);
            }
            
            let data = create_market_data_batch(&symbols, 150.0, 1);
            aggregator.process_market_data(&data).await.unwrap();
        });
    });
}

fn benchmark_large_batch_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // 50 symbols across different sectors
    let symbols: Vec<&str> = vec![
        // Technology
        "AAPL", "MSFT", "GOOGL", "NVDA", "META", "AMZN", "TSLA", "NFLX", "CRM", "ORCL",
        "ADBE", "INTC", "AMD", "QCOM", "TXN", "AVGO", "IBM", "MU", "LRCX", "KLAC",
        // Financial
        "JPM", "BAC", "WFC", "GS", "MS", "C", "BLK", "SPGI", "AXP", "USB",
        // Healthcare
        "UNH", "JNJ", "PFE", "ABBV", "TMO", "ABT", "BMY", "MRK", "LLY", "MDT",
        // Energy
        "XOM", "CVX", "COP", "EOG", "SLB", "PSX", "VLO", "MPC", "KMI", "WMB"
    ];
    
    c.bench_function("large_batch_processing_50_symbols", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            // Set market caps
            for symbol in &symbols {
                aggregator.update_market_cap(symbol, 1_000_000_000_000.0);
            }
            
            let data = create_market_data_batch(&symbols, 150.0, 1);
            aggregator.process_market_data(&data).await.unwrap();
        });
    });
}

fn benchmark_very_large_batch_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // Generate 100 symbols
    let symbols: Vec<String> = (0..100).map(|i| format!("SYM{:03}", i)).collect();
    let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
    
    c.bench_function("very_large_batch_processing_100_symbols", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            // Set market caps
            for symbol in &symbol_refs {
                aggregator.update_market_cap(symbol, 500_000_000_000.0);
            }
            
            let data = create_market_data_batch(&symbol_refs, 100.0, 1);
            let start = Instant::now();
            aggregator.process_market_data(&data).await.unwrap();
            let elapsed = start.elapsed();
            
            // Assert latency requirement during benchmark
            assert!(elapsed.as_millis() < 50, 
                "Latency requirement violated: {}ms >= 50ms", elapsed.as_millis());
        });
    });
}

// Throughput benchmarks
fn benchmark_throughput_scaling(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("throughput_scaling");
    
    let symbol_counts = vec![10, 25, 50, 75, 100];
    
    for &count in &symbol_counts {
        let symbols: Vec<String> = (0..count).map(|i| format!("THRU{:03}", i)).collect();
        let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
        
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("symbols", count), &count, |b, &count| {
            b.to_async(&rt).iter(|| async {
                let aggregator = create_benchmark_aggregator();
                
                // Set market caps
                for symbol in &symbol_refs {
                    aggregator.update_market_cap(symbol, 1_000_000_000_000.0);
                }
                
                let data = create_market_data_batch(&symbol_refs, 100.0, 1);
                aggregator.process_market_data(&data).await.unwrap();
            });
        });
    }
    
    group.finish();
}

// Memory efficiency benchmarks
fn benchmark_memory_efficiency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("memory_efficiency_sustained_load", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            // Simulate sustained load with rolling data
            let symbols: Vec<String> = (0..50).map(|i| format!("MEM{:03}", i)).collect();
            let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
            
            // Set market caps
            for symbol in &symbol_refs {
                aggregator.update_market_cap(symbol, 1_000_000_000_000.0);
            }
            
            // Process multiple batches to simulate sustained load
            for batch in 0..20 {
                let data = create_market_data_batch(&symbol_refs, 100.0 + batch as f64, 1);
                aggregator.process_market_data(&data).await.unwrap();
            }
            
            // Check memory usage
            let memory_mb = aggregator.estimate_memory_usage();
            assert!(memory_mb < 50.0, "Memory usage {}MB exceeds 50MB limit", memory_mb);
        });
    });
}

// Concurrent processing benchmarks
fn benchmark_concurrent_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("concurrent_processing_multiple_sectors", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = Arc::new(create_benchmark_aggregator());
            
            // Define symbols per sector
            let sector_symbols = vec![
                vec!["AAPL", "MSFT", "GOOGL", "NVDA", "META"],
                vec!["JPM", "BAC", "WFC", "GS", "MS"],
                vec!["UNH", "JNJ", "PFE", "ABBV", "TMO"],
                vec!["XOM", "CVX", "COP", "EOG", "SLB"],
            ];
            
            // Set market caps
            for symbols in &sector_symbols {
                for symbol in symbols {
                    aggregator.update_market_cap(symbol, 2_000_000_000_000.0);
                }
            }
            
            // Process sectors concurrently
            let mut handles = Vec::new();
            
            for symbols in sector_symbols {
                let agg_clone = Arc::clone(&aggregator);
                let handle = tokio::spawn(async move {
                    let symbol_refs: Vec<&str> = symbols.iter().map(|s| *s).collect();
                    let data = create_market_data_batch(&symbol_refs, 150.0, 1);
                    agg_clone.process_market_data(&data).await.unwrap();
                });
                handles.push(handle);
            }
            
            // Wait for all sectors to complete
            for handle in handles {
                handle.await.unwrap();
            }
        });
    });
}

// Real-time streaming simulation
fn benchmark_real_time_streaming(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("real_time_streaming_simulation", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            let symbols = vec!["AAPL", "MSFT", "GOOGL", "JPM", "UNH", "XOM"];
            
            // Set market caps
            for symbol in &symbols {
                aggregator.update_market_cap(symbol, 2_000_000_000_000.0);
            }
            
            // Simulate 10 rapid updates (like real-time streaming)
            for update in 0..10 {
                let data = create_market_data_batch(&symbols, 150.0 + update as f64 * 0.1, 1);
                let start = Instant::now();
                aggregator.process_market_data(&data).await.unwrap();
                let elapsed = start.elapsed();
                
                // Each update must be under 50ms
                assert!(elapsed.as_millis() < 50, 
                    "Update {} exceeded 50ms: {}ms", update, elapsed.as_millis());
            }
        });
    });
}

// ETF correlation benchmark
fn benchmark_etf_correlation_calculation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("etf_correlation_calculation", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            // Set up tech sector
            let tech_symbols = vec!["AAPL", "MSFT", "GOOGL", "NVDA"];
            for symbol in &tech_symbols {
                aggregator.update_market_cap(symbol, 2_500_000_000_000.0);
            }
            
            // Add ETF data
            let etf_data = create_market_data_batch(&["XLK"], 180.0, 10);
            aggregator.update_etf_prices(&etf_data).await.unwrap();
            
            // Process sector data with correlation calculation
            let sector_data = create_market_data_batch(&tech_symbols, 200.0, 5);
            aggregator.process_market_data(&sector_data).await.unwrap();
            
            // Verify correlation was calculated
            let tech_agg = aggregator.get_sector_aggregation(&SectorId::Technology).unwrap();
            assert!(tech_agg.etf_correlation.is_some());
        });
    });
}

// Breadth indicators calculation benchmark
fn benchmark_breadth_indicators_calculation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("breadth_indicators_calculation", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            let symbols = vec!["AAPL", "MSFT", "GOOGL", "NVDA", "META", "AMZN", "TSLA", "NFLX"];
            
            // Set market caps
            for symbol in &symbols {
                aggregator.update_market_cap(symbol, 2_000_000_000_000.0);
            }
            
            // Create historical data for breadth calculation
            let mut all_data = Vec::new();
            for i in 0..25 { // 25 time periods for breadth calculation
                let batch = create_market_data_batch(&symbols, 150.0 + i as f64 * 0.5, 1);
                all_data.extend(batch);
            }
            
            aggregator.process_market_data(&all_data).await.unwrap();
            
            let tech_agg = aggregator.get_sector_aggregation(&SectorId::Technology).unwrap();
            
            // Verify breadth indicators were calculated
            let breadth = &tech_agg.breadth_indicators;
            assert!(breadth.advance_decline_ratio >= 0.0);
            assert!(breadth.up_down_volume_ratio >= 0.0);
        });
    });
}

// Historical data processing benchmark
fn benchmark_historical_data_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("historical_data_processing_100_periods", |b| {
        b.to_async(&rt).iter(|| async {
            let aggregator = create_benchmark_aggregator();
            
            let symbols = vec!["AAPL", "MSFT", "GOOGL", "NVDA", "JPM"];
            
            // Set market caps
            for symbol in &symbols {
                aggregator.update_market_cap(symbol, 2_000_000_000_000.0);
            }
            
            // Process 100 historical periods
            let mut all_data = Vec::new();
            for period in 0..100 {
                let mut period_data = Vec::new();
                for symbol in &symbols {
                    let price = 100.0 + period as f64 * 0.1;
                    period_data.push(TimeSeriesData {
                        symbol: symbol.to_string(),
                        timestamp: Utc::now() + chrono::Duration::seconds(period),
                        open: price - 0.25,
                        high: price + 0.50,
                        low: price - 0.50,
                        close: price,
                        volume: 100_000.0,
                        indicators: HashMap::new(),
                        source: Some("historical".to_string()),
                        entity: Some(symbol.to_string()),
                        value: Some(price),
                        metadata: None,
                        values: vec![price],
                        timestamps: vec![Utc::now()],
                        metadata_map: HashMap::new(),
                    });
                }
                all_data.extend(period_data);
            }
            
            let start = Instant::now();
            aggregator.process_market_data(&all_data).await.unwrap();
            let elapsed = start.elapsed();
            
            // Should process 500 data points (5 symbols * 100 periods) quickly
            assert!(elapsed.as_millis() < 100, 
                "Historical processing took {}ms, should be <100ms", elapsed.as_millis());
        });
    });
}

criterion_group!(
    sector_aggregation_benchmarks,
    benchmark_single_symbol_processing,
    benchmark_small_batch_processing,
    benchmark_medium_batch_processing,
    benchmark_large_batch_processing,
    benchmark_very_large_batch_processing,
    benchmark_throughput_scaling,
    benchmark_memory_efficiency,
    benchmark_concurrent_processing,
    benchmark_real_time_streaming,
    benchmark_etf_correlation_calculation,
    benchmark_breadth_indicators_calculation,
    benchmark_historical_data_processing
);

criterion_main!(sector_aggregation_benchmarks);