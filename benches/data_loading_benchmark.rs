//! Performance benchmarks for data loading operations
//! Measures throughput and latency of training data pipeline components

use chrono::{DateTime, Duration, Utc};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

use autonomous_platform::adapters::{MarketData, TimescaleAdapter, TimescaleConfig};
use autonomous_platform::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use autonomous_platform::integration::data_access::DataAccessLayer;
use autonomous_platform::neural::ModelType;
use autonomous_platform::products::features::realtraining::training_data_service::{
    FeatureConfig, NormalizationMethod, TrainingDataConfig, TrainingDataService, ValidationConfig,
};

// Mock adapter for benchmarking
struct BenchmarkTimescaleAdapter {
    data: Vec<MarketData>,
}

impl BenchmarkTimescaleAdapter {
    fn new(data_size: usize) -> Self {
        let mut data = Vec::with_capacity(data_size);
        let base_time = Utc::now() - Duration::hours(data_size as i64 / 60);

        for i in 0..data_size {
            let timestamp = base_time + Duration::minutes(i as i64);
            let price = 50000.0 + (i as f64 * 0.1).sin() * 1000.0;

            data.push(MarketData {
                symbol: "BTC/USD".to_string(),
                timestamp: timestamp.timestamp(),
                open: price - 5.0,
                high: price + 25.0,
                low: price - 25.0,
                close: price,
                volume: 1000.0 + (i as f64 * 2.0),
            });
        }

        Self { data }
    }
}

#[async_trait::async_trait]
impl autonomous_platform::adapters::TimescaleAdapterTrait for BenchmarkTimescaleAdapter {
    async fn query_market_data(
        &self,
        _symbol: &str,
        _start_ts: i64,
        _end_ts: i64,
    ) -> anyhow::Result<Vec<MarketData>> {
        Ok(self.data.clone())
    }
}

// Helper function to create TimeSeriesData for benchmarking
fn create_time_series_data(count: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::with_capacity(count);
    let base_time = Utc::now() - Duration::hours(count as i64 / 60);

    for i in 0..count {
        let timestamp = base_time + Duration::minutes(i as i64);
        let price = 50000.0 + (i as f64 * 0.1).sin() * 1000.0;

        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp,
            open: price - 5.0,
            high: price + 25.0,
            low: price - 25.0,
            close: price,
            volume: 1000.0 + (i as f64 * 2.0),
            indicators: HashMap::new(),
            source: Some("benchmark".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(price),
            metadata: None,
        });
    }

    data
}

// Benchmark data loading from different data sizes
fn bench_training_data_loading(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("training_data_loading");

    for data_size in [100, 500, 1000, 2000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("load_training_data", data_size),
            data_size,
            |b, &size| {
                let adapter = Arc::new(BenchmarkTimescaleAdapter::new(size));
                let config = TrainingDataConfig {
                    window_size: 50,
                    step_size: 10,
                    min_samples: 100,
                    max_samples: Some(10000),
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let mut service =
                        TrainingDataService::new(Arc::clone(&adapter), config.clone());
                    let result = service
                        .load_training_data(
                            "BTC/USD",
                            Utc::now() - Duration::hours(size as i64 / 60),
                            Utc::now(),
                            &ModelType::Regression,
                        )
                        .await;

                    black_box(result.unwrap())
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark different window sizes
fn bench_window_size_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("window_size_impact");

    let data_size = 1000;
    let adapter = Arc::new(BenchmarkTimescaleAdapter::new(data_size));

    for window_size in [10, 25, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("window_size", window_size),
            window_size,
            |b, &ws| {
                let config = TrainingDataConfig {
                    window_size: ws,
                    step_size: ws / 4, // 25% step size
                    min_samples: 100,
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let mut service =
                        TrainingDataService::new(Arc::clone(&adapter), config.clone());
                    let result = service
                        .load_training_data(
                            "BTC/USD",
                            Utc::now() - Duration::hours(data_size as i64 / 60),
                            Utc::now(),
                            &ModelType::Regression,
                        )
                        .await;

                    black_box(result.unwrap())
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark different normalization methods
fn bench_normalization_methods(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("normalization_methods");

    let data_size = 1000;
    let adapter = Arc::new(BenchmarkTimescaleAdapter::new(data_size));

    let normalization_methods = [
        ("MinMax", NormalizationMethod::MinMax),
        ("ZScore", NormalizationMethod::ZScore),
        ("PercentChange", NormalizationMethod::PercentChange),
        ("LogReturns", NormalizationMethod::LogReturns),
    ];

    for (name, method) in normalization_methods.iter() {
        group.bench_function(*name, |b| {
            let config = TrainingDataConfig {
                window_size: 50,
                step_size: 10,
                min_samples: 100,
                feature_config: FeatureConfig {
                    normalization: method.clone(),
                    ..Default::default()
                },
                ..Default::default()
            };

            b.iter(|| {
                    rt.block_on(async {
                let mut service = TrainingDataService::new(Arc::clone(&adapter), config.clone());
                let result = service
                    .load_training_data(
                        "BTC/USD",
                        Utc::now() - Duration::hours(data_size as i64 / 60),
                        Utc::now(),
                        &ModelType::Regression,
                    )
                    .await;

                black_box(result.unwrap())
            });
        });
    }

    group.finish();
}

// Benchmark feature statistics calculation
fn bench_feature_statistics(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("feature_statistics");

    for sample_count in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("calculate_stats", sample_count),
            sample_count,
            |b, &count| {
                let adapter = Arc::new(BenchmarkTimescaleAdapter::new(count * 2)); // Ensure enough data
                let config = TrainingDataConfig {
                    window_size: 20,
                    step_size: 1,
                    min_samples: 50,
                    max_samples: Some(count),
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let mut service =
                        TrainingDataService::new(Arc::clone(&adapter), config.clone());
                    let batch = service
                        .load_training_data(
                            "BTC/USD",
                            Utc::now() - Duration::hours(2),
                            Utc::now(),
                            &ModelType::Regression,
                        )
                        .await
                        .unwrap();

                    let stats = service.get_feature_statistics(&batch);
                    black_box(stats)
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark incremental data loading
fn bench_incremental_loading(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("incremental_loading");

    for new_data_size in [10, 25, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("incremental_data", new_data_size),
            new_data_size,
            |b, &size| {
                let adapter = Arc::new(BenchmarkTimescaleAdapter::new(size));
                let config = TrainingDataConfig {
                    window_size: 20,
                    step_size: 5,
                    min_samples: 10,
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let mut service =
                        TrainingDataService::new(Arc::clone(&adapter), config.clone());
                    let result = service
                        .load_incremental_data(
                            "BTC/USD",
                            Utc::now() - Duration::minutes(30),
                            &ModelType::Regression,
                        )
                        .await;

                    black_box(result.unwrap())
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark data validation overhead
fn bench_validation_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("validation_overhead");

    let data_size = 1000;
    let adapter = Arc::new(BenchmarkTimescaleAdapter::new(data_size));

    // Test with validation enabled
    group.bench_function("with_validation", |b| {
        let config = TrainingDataConfig {
            window_size: 50,
            step_size: 10,
            min_samples: 100,
            validation_config: ValidationConfig {
                check_gaps: true,
                max_gap_minutes: 60,
                outlier_threshold: Some(3.0),
                min_quality_score: 0.95,
            },
            ..Default::default()
        };

        b.iter(|| {
                    rt.block_on(async {
            let mut service = TrainingDataService::new(Arc::clone(&adapter), config.clone());
            let result = service
                .load_training_data(
                    "BTC/USD",
                    Utc::now() - Duration::hours(2),
                    Utc::now(),
                    &ModelType::Regression,
                )
                .await;

            black_box(result.unwrap())
        });
    });

    // Test with minimal validation
    group.bench_function("minimal_validation", |b| {
        let config = TrainingDataConfig {
            window_size: 50,
            step_size: 10,
            min_samples: 100,
            validation_config: ValidationConfig {
                check_gaps: false,
                max_gap_minutes: 3600, // 1 hour
                outlier_threshold: None,
                min_quality_score: 0.0,
            },
            ..Default::default()
        };

        b.iter(|| {
                    rt.block_on(async {
            let mut service = TrainingDataService::new(Arc::clone(&adapter), config.clone());
            let result = service
                .load_training_data(
                    "BTC/USD",
                    Utc::now() - Duration::hours(2),
                    Utc::now(),
                    &ModelType::Regression,
                )
                .await;

            black_box(result.unwrap())
        });
    });

    group.finish();
}

// Benchmark memory usage patterns
fn bench_memory_efficiency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_efficiency");

    for max_samples in [500, 1000, 2000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("max_samples", max_samples),
            max_samples,
            |b, &max| {
                let adapter = Arc::new(BenchmarkTimescaleAdapter::new(max * 2)); // More data than limit
                let config = TrainingDataConfig {
                    window_size: 30,
                    step_size: 5,
                    min_samples: 100,
                    max_samples: Some(max),
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let mut service =
                        TrainingDataService::new(Arc::clone(&adapter), config.clone());
                    let result = service
                        .load_training_data(
                            "BTC/USD",
                            Utc::now() - Duration::hours(4),
                            Utc::now(),
                            &ModelType::Regression,
                        )
                        .await;

                    let batch = result.unwrap();
                    // Ensure we don't exceed the limit
                    assert!(batch.features.len() <= max);
                    black_box(batch)
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark concurrent access patterns
fn bench_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_access");

    for concurrency in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_requests", concurrency),
            concurrency,
            |b, &conc| {
                let data_size = 500;
                let adapter = Arc::new(BenchmarkTimescaleAdapter::new(data_size));
                let config = TrainingDataConfig {
                    window_size: 25,
                    step_size: 5,
                    min_samples: 50,
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let handles: Vec<_> = (0..conc)
                        .map(|i| {
                            let adapter_clone = Arc::clone(&adapter);
                            let config_clone = config.clone();
                            let symbol = format!("BTC/USD_{}", i);

                            tokio::spawn(async move {
                                let mut service =
                                    TrainingDataService::new(adapter_clone, config_clone);
                                service
                                    .load_training_data(
                                        &symbol,
                                        Utc::now() - Duration::hours(1),
                                        Utc::now(),
                                        &ModelType::Regression,
                                    )
                                    .await
                                    .unwrap()
                            })
                        })
                        .collect();

                    let results = futures::future::join_all(handles).await;
                    black_box(results.into_iter().map(|r| r.unwrap()).collect::<Vec<_>>())
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark different step sizes
fn bench_step_size_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("step_size_impact");

    let data_size = 1000;
    let window_size = 50;
    let adapter = Arc::new(BenchmarkTimescaleAdapter::new(data_size));

    for step_size in [1, 5, 10, 25, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("step_size", step_size),
            step_size,
            |b, &step| {
                let config = TrainingDataConfig {
                    window_size,
                    step_size: step,
                    min_samples: 100,
                    ..Default::default()
                };

                b.iter(|| {
                    rt.block_on(async {
                    let mut service =
                        TrainingDataService::new(Arc::clone(&adapter), config.clone());
                    let result = service
                        .load_training_data(
                            "BTC/USD",
                            Utc::now() - Duration::hours(2),
                            Utc::now(),
                            &ModelType::Regression,
                        )
                        .await;

                    black_box(result.unwrap())
                    });
                });
            },
        );
    }

    group.finish();
}

// Benchmark time series conversion performance
fn bench_time_series_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("time_series_conversion");

    for data_size in [100, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::new("convert_market_data", data_size),
            data_size,
            |b, &size| {
                let market_data: Vec<MarketData> = (0..size)
                    .map(|i| MarketData {
                        symbol: "BTC/USD".to_string(),
                        timestamp: (Utc::now() - Duration::minutes(size as i64 - i as i64))
                            .timestamp(),
                        open: 50000.0 + i as f64,
                        high: 50100.0 + i as f64,
                        low: 49900.0 + i as f64,
                        close: 50050.0 + i as f64,
                        volume: 1000.0 + i as f64,
                    })
                    .collect();

                b.iter(|| {
                    // Simulate the conversion process that happens in TrainingDataService
                    let time_series: Vec<TimeSeriesData> = market_data
                        .iter()
                        .map(|d| {
                            let timestamp =
                                DateTime::<Utc>::from_timestamp(d.timestamp, 0).unwrap();
                            TimeSeriesData {
                                symbol: d.symbol.clone(),
                                timestamp,
                                open: d.open,
                                high: d.high,
                                low: d.low,
                                close: d.close,
                                volume: d.volume,
                                indicators: HashMap::new(),
                                source: Some("benchmark".to_string()),
                                entity: None,
                                value: Some(d.close),
                                metadata: None,
                            }
                        })
                        .collect();

                    black_box(time_series)
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_training_data_loading,
    bench_window_size_impact,
    bench_normalization_methods,
    bench_feature_statistics,
    bench_incremental_loading,
    bench_validation_overhead,
    bench_memory_efficiency,
    bench_concurrent_access,
    bench_step_size_impact,
    bench_time_series_conversion
);

criterion_main!(benches);
