//! Performance Benchmarking Suite for ruv-FANN
//!
//! Comprehensive benchmarks for:
//! - Model loading times
//! - Prediction latency
//! - Batch processing throughput
//! - Memory usage patterns
//! - Cache efficiency
//! - Parallel scaling

use anyhow::Result;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::FannPredictor;

/// Generate synthetic time series data for benchmarking
fn generate_benchmark_data(size: usize) -> Vec<TimeSeriesData> {
    (0..size)
        .map(|i| {
            let base_price = 100.0 + (i as f64 * 0.1).sin() * 10.0;
            TimeSeriesData {
                timestamp: chrono::Utc::now() - chrono::Duration::minutes(i as i64),
                open: base_price - 0.5,
                high: base_price + 1.0,
                low: base_price - 1.0,
                close: base_price,
                volume: 1_000_000.0 + (i as f64 * 100.0),
                indicators: std::collections::HashMap::from([
                    ("rsi".to_string(), 50.0 + (i as f64 * 0.2).sin() * 20.0),
                    ("macd".to_string(), (i as f64 * 0.05).sin()),
                ]),
            }
        })
        .collect()
}

/// Benchmark model loading performance
fn benchmark_model_loading(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("model_loading");
    group.measurement_time(Duration::from_secs(10));

    for model_name in &["MLP", "LSTM", "GRU", "TCN", "Transformer"] {
        group.bench_with_input(
            BenchmarkId::new("load", model_name),
            model_name,
            |b, &model_name| {
                b.iter(|| {
                    runtime.block_on(async {
                        let config = NeuralConfig::default();
                        let predictor = FannPredictor::new(config).unwrap();

                        // Force model initialization
                        let data = generate_benchmark_data(100);
                        predictor.predict(model_name, &data, 5).await.unwrap();
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark single prediction latency
fn benchmark_prediction_latency(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Setup predictor
    let predictor = runtime.block_on(async {
        let config = NeuralConfig::default();
        FannPredictor::new(config).unwrap()
    });

    let mut group = c.benchmark_group("prediction_latency");
    group.measurement_time(Duration::from_secs(10));

    let data_sizes = vec![50, 100, 200, 500];
    let horizons = vec![1, 5, 10, 20];

    for data_size in &data_sizes {
        for horizon in &horizons {
            let data = generate_benchmark_data(*data_size);

            group.bench_with_input(
                BenchmarkId::new("predict", format!("data_{}_horizon_{}", data_size, horizon)),
                &(data.clone(), *horizon),
                |b, (data, horizon)| {
                    b.iter(|| {
                        runtime.block_on(async {
                            black_box(predictor.predict("MLP", data, *horizon).await.unwrap());
                        });
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark batch processing throughput
fn benchmark_batch_throughput(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Setup optimized predictor
    let predictor = runtime.block_on(async {
        let config = NeuralConfig::default();
        let base = std::sync::Arc::new(FannPredictor::new(config).unwrap());
        crate::neural::performance_optimizer::OptimizedFannPredictor::new(base).await.unwrap()
    });

    let mut group = c.benchmark_group("batch_throughput");
    group.measurement_time(Duration::from_secs(20));

    let batch_sizes = vec![1, 8, 16, 32, 64, 128];
    let data = generate_benchmark_data(200);

    for batch_size in &batch_sizes {
        let batch_data: Vec<_> = (0..*batch_size).map(|_| data.as_slice()).collect();

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_data,
            |b, batch| {
                b.iter(|| {
                    runtime.block_on(async {
                        black_box(
                            predictor
                                .predict_batch("MLP", batch.clone(), 5)
                                .await
                                .unwrap(),
                        );
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory_efficiency(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_efficiency");
    group.measurement_time(Duration::from_secs(10));

    // Compare standard vs optimized memory usage
    group.bench_function("standard_allocation", |b| {
        b.iter(|| {
            let mut buffers = Vec::new();
            for _ in 0..100 {
                let buffer: Vec<f32> = vec![0.0; 256];
                buffers.push(black_box(buffer));
            }
        });
    });

    group.bench_function("pooled_allocation", |b| {
        use super::performance_optimizer::MemoryPool;
        let pool = MemoryPool::new(100, 256);

        b.iter(|| {
            let mut buffers = Vec::new();
            for _ in 0..100 {
                let buffer = pool.get_input_buffer();
                buffers.push(black_box(buffer));
            }
            for buffer in buffers {
                pool.return_input_buffer(buffer);
            }
        });
    });

    group.finish();
}

/// Benchmark parallel scaling efficiency
fn benchmark_parallel_scaling(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let predictor = runtime.block_on(async {
        let config = NeuralConfig::default();
        let base = std::sync::Arc::new(FannPredictor::new(config).unwrap());
        crate::neural::performance_optimizer::OptimizedFannPredictor::new(base).await.unwrap()
    });

    let mut group = c.benchmark_group("parallel_scaling");
    group.measurement_time(Duration::from_secs(15));

    let data = generate_benchmark_data(1000);
    let thread_counts = vec![1, 2, 4, 8, 16];

    for threads in &thread_counts {
        group.bench_with_input(
            BenchmarkId::new("threads", threads),
            threads,
            |b, &threads| {
                // Set rayon thread pool size
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build_global()
                    .ok();

                let batch: Vec<_> = (0..32).map(|_| data.as_slice()).collect();

                b.iter(|| {
                    runtime.block_on(async {
                        black_box(
                            predictor
                                .predict_batch("MLP", batch.clone(), 5)
                                .await
                                .unwrap(),
                        );
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cache efficiency
fn benchmark_cache_efficiency(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let predictor = runtime.block_on(async {
        let config = NeuralConfig::default();
        let base = std::sync::Arc::new(FannPredictor::new(config).unwrap());
        crate::neural::performance_optimizer::OptimizedFannPredictor::new(base).await.unwrap()
    });

    let mut group = c.benchmark_group("cache_efficiency");

    // Generate different data patterns
    let unique_data: Vec<_> = (0..100).map(|i| generate_benchmark_data(100 + i)).collect();

    let repeated_data = generate_benchmark_data(100);

    // Benchmark unique predictions (cache misses)
    group.bench_function("cache_misses", |b| {
        let mut idx = 0;
        b.iter(|| {
            runtime.block_on(async {
                let data = &unique_data[idx % unique_data.len()];
                idx += 1;
                black_box(
                    predictor
                        .predict_batch("MLP", vec![data.as_slice()], 5)
                        .await
                        .unwrap(),
                );
            });
        });
    });

    // Benchmark repeated predictions (cache hits)
    group.bench_function("cache_hits", |b| {
        b.iter(|| {
            runtime.block_on(async {
                black_box(
                    predictor
                        .predict_batch("MLP", vec![repeated_data.as_slice()], 5)
                        .await
                        .unwrap(),
                );
            });
        });
    });

    group.finish();
}

/// Benchmark ensemble prediction performance
fn benchmark_ensemble_performance(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let predictor = runtime.block_on(async {
        let config = NeuralConfig::default();
        FannPredictor::new(config).unwrap()
    });

    let mut group = c.benchmark_group("ensemble_performance");
    group.measurement_time(Duration::from_secs(15));

    let data = generate_benchmark_data(200);
    let ensemble_sizes = vec![1, 3, 5, 7];

    for size in &ensemble_sizes {
        let models: Vec<_> = vec!["MLP", "LSTM", "GRU", "TCN", "Transformer"]
            .into_iter()
            .take(*size)
            .collect();

        group.bench_with_input(BenchmarkId::new("ensemble", size), &models, |b, models| {
            b.iter(|| {
                runtime.block_on(async {
                    black_box(predictor.ensemble_predict(models, &data, 5).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

/// Main benchmark groups
criterion_group!(
    benches,
    benchmark_model_loading,
    benchmark_prediction_latency,
    benchmark_batch_throughput,
    benchmark_memory_efficiency,
    benchmark_parallel_scaling,
    benchmark_cache_efficiency,
    benchmark_ensemble_performance
);

criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_generation() {
        let data = generate_benchmark_data(100);
        assert_eq!(data.len(), 100);
        assert!(data[0].close > 0.0);
    }
}
