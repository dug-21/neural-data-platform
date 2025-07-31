//! Performance Benchmarks for Real Training System
//!
//! Comprehensive benchmarks measuring data loading, feature generation,
//! model training, and persistence operations to detect regressions.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use tokio::runtime::Runtime;

use neural_trader::{
    neural::{TechnicalIndicatorTransformer, VolumeProfileTransformer},
    realtraining::{
        DataSelector, FeatureEngine, ModelStorage, ModelType, SelectionStrategy, TrainingConfig,
        TrainingPipeline,
    },
    storage::{RedisCache, TimescaleDBStorage},
};

/// Benchmark data loading from TimescaleDB
fn benchmark_data_loading(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    // Setup test environment
    let (storage, cache) = runtime.block_on(async {
        let storage = setup_test_timescale().await.unwrap();
        let cache = setup_test_redis().await.unwrap();

        // Pre-populate with test data
        populate_benchmark_data(&storage, 1_000_000).await.unwrap();

        (storage, cache)
    });

    let mut group = c.benchmark_group("data_loading");

    // Benchmark different data sizes
    for days in [1, 7, 30].iter() {
        group.bench_with_input(BenchmarkId::new("recent_data", days), days, |b, &days| {
            b.iter(|| {
                runtime.block_on(async {
                    let selector = DataSelector::new_with_storage(storage.clone(), cache.clone());

                    let data = selector
                        .select_data(SelectionStrategy::RecencyBased { days: *days })
                        .await
                        .unwrap();

                    black_box(data);
                });
            });
        });
    }

    // Benchmark with cache hits
    group.bench_function("cached_data_7d", |b| {
        // Warm up cache
        runtime.block_on(async {
            let selector = DataSelector::new_with_storage(storage.clone(), cache.clone());
            selector
                .select_data(SelectionStrategy::RecencyBased { days: 7 })
                .await
                .unwrap();
        });

        b.iter(|| {
            runtime.block_on(async {
                let selector = DataSelector::new_with_storage(storage.clone(), cache.clone());

                let data = selector
                    .select_data(SelectionStrategy::RecencyBased { days: 7 })
                    .await
                    .unwrap();

                black_box(data);
            });
        });
    });

    group.finish();
}

/// Benchmark feature generation pipeline
fn benchmark_feature_generation(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("feature_generation");

    // Generate test data sets of different sizes
    let data_sizes = vec![("small", 1_000), ("medium", 10_000), ("large", 100_000)];

    for (name, size) in data_sizes {
        let data = generate_benchmark_timeseries(size);

        group.bench_with_input(
            BenchmarkId::new("technical_indicators", name),
            &data,
            |b, data| {
                b.iter(|| {
                    runtime.block_on(async {
                        let transformer = TechnicalIndicatorTransformer::new();
                        let features = transformer.transform(data).await.unwrap();
                        black_box(features);
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("volume_profile", name),
            &data,
            |b, data| {
                b.iter(|| {
                    runtime.block_on(async {
                        let transformer = VolumeProfileTransformer::new();
                        let features = transformer.transform(data).await.unwrap();
                        black_box(features);
                    });
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("full_pipeline", name), &data, |b, data| {
            b.iter(|| {
                runtime.block_on(async {
                    let engine = FeatureEngine::new()
                        .add_transformer(Box::new(TechnicalIndicatorTransformer::new()))
                        .add_transformer(Box::new(VolumeProfileTransformer::new()));

                    let features = engine.process(data).await.unwrap();
                    black_box(features);
                });
            });
        });
    }

    group.finish();
}

/// Benchmark model training performance
fn benchmark_model_training(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("model_training");
    group.sample_size(10); // Reduce sample size for longer operations
    group.measurement_time(Duration::from_secs(60));

    // Prepare training data
    let training_data = runtime.block_on(async { prepare_training_dataset(10_000).await });

    // Benchmark different model types
    let model_configs = vec![
        ("mlp_small", ModelType::MLP, 10, 32),
        ("mlp_large", ModelType::MLP, 20, 64),
        ("lstm_small", ModelType::LSTM, 10, 32),
    ];

    for (name, model_type, epochs, batch_size) in model_configs {
        group.bench_function(name, |b| {
            b.iter(|| {
                runtime.block_on(async {
                    let pipeline = create_benchmark_pipeline().await;

                    let config = TrainingConfig {
                        model_type,
                        epochs,
                        batch_size,
                        learning_rate: 0.001,
                        early_stopping: false,
                        validation_split: 0.2,
                    };

                    let result = pipeline
                        .train_model(config, training_data.clone())
                        .await
                        .unwrap();

                    black_box(result);
                });
            });
        });
    }

    group.finish();
}

/// Benchmark model persistence operations
fn benchmark_model_persistence(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("model_persistence");

    // Create test models of different sizes
    let model_sizes = vec![
        ("small", create_model_bytes(1_000_000)),   // 1MB
        ("medium", create_model_bytes(10_000_000)), // 10MB
        ("large", create_model_bytes(100_000_000)), // 100MB
    ];

    for (name, model_bytes) in model_sizes {
        let model = create_test_model_with_size(model_bytes);

        group.bench_with_input(
            BenchmarkId::new("save_uncompressed", name),
            &model,
            |b, model| {
                b.iter(|| {
                    runtime.block_on(async {
                        let storage = ModelStorage::new("/tmp/bench_models");
                        let result = storage.save_model(model, Default::default()).await.unwrap();
                        black_box(result);

                        // Cleanup
                        tokio::fs::remove_dir_all("/tmp/bench_models").await.ok();
                    });
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("save_compressed", name),
            &model,
            |b, model| {
                b.iter(|| {
                    runtime.block_on(async {
                        let storage = ModelStorage::new("/tmp/bench_models")
                            .with_compression(CompressionStrategy::Zstd { level: 3 });

                        let result = storage.save_model(model, Default::default()).await.unwrap();
                        black_box(result);

                        // Cleanup
                        tokio::fs::remove_dir_all("/tmp/bench_models").await.ok();
                    });
                });
            },
        );
    }

    // Benchmark model loading
    runtime.block_on(async {
        let storage = ModelStorage::new("/tmp/bench_models");
        let model = create_test_model_with_size(10_000_000);
        let save_result = storage
            .save_model(&model, Default::default())
            .await
            .unwrap();

        group.bench_function("load_model_10mb", |b| {
            b.iter(|| {
                runtime.block_on(async {
                    let loaded = storage
                        .load_model(&model.id, &save_result.version)
                        .await
                        .unwrap();
                    black_box(loaded);
                });
            });
        });

        // Cleanup
        tokio::fs::remove_dir_all("/tmp/bench_models").await.ok();
    });

    group.finish();
}

/// Benchmark complete training pipeline
fn benchmark_complete_pipeline(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("complete_pipeline");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(120));

    group.bench_function("end_to_end_training", |b| {
        b.iter(|| {
            runtime.block_on(async {
                // Setup
                let pipeline = create_full_pipeline().await;

                // Data selection
                let data = pipeline
                    .select_training_data(SelectionStrategy::RecencyBased { days: 7 })
                    .await
                    .unwrap();

                // Feature generation
                let features = pipeline.generate_features(&data).await.unwrap();

                // Model training
                let config = TrainingConfig {
                    model_type: ModelType::MLP,
                    epochs: 10,
                    batch_size: 64,
                    learning_rate: 0.001,
                    early_stopping: true,
                    validation_split: 0.2,
                };

                let result = pipeline
                    .train_with_features(config, features)
                    .await
                    .unwrap();

                // Model persistence
                let storage_result = pipeline
                    .persist_model(&result.model, &result.metadata)
                    .await
                    .unwrap();

                black_box(storage_result);
            });
        });
    });

    group.finish();
}

/// Benchmark market hours calculations
fn benchmark_market_hours(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("market_hours");

    let monitor = runtime.block_on(async { MarketHoursMonitor::new() });

    group.bench_function("is_market_open", |b| {
        b.iter(|| {
            let result = monitor.is_market_open("NYSE");
            black_box(result);
        });
    });

    group.bench_function("next_close_time", |b| {
        b.iter(|| {
            let result = monitor.next_close_time("NYSE");
            black_box(result);
        });
    });

    group.bench_function("time_until_close", |b| {
        b.iter(|| {
            let result = monitor.time_until_close("NYSE");
            black_box(result);
        });
    });

    group.bench_function("is_holiday", |b| {
        let test_date = chrono::NaiveDate::from_ymd_opt(2024, 12, 25).unwrap();
        b.iter(|| {
            let result = monitor.is_holiday("NYSE", test_date);
            black_box(result);
        });
    });

    group.finish();
}

// Helper functions

async fn setup_test_timescale() -> Result<TimescaleDBStorage, Box<dyn std::error::Error>> {
    let config = TimescaleConfig {
        host: "localhost",
        port: 5432,
        database: "neural_trader_bench",
        username: "postgres",
        password: "postgres",
    };

    Ok(TimescaleDBStorage::new(config).await?)
}

async fn setup_test_redis() -> Result<RedisCache, Box<dyn std::error::Error>> {
    let config = RedisConfig {
        url: "redis://localhost:6379",
        pool_size: 10,
    };

    Ok(RedisCache::new(config).await?)
}

async fn populate_benchmark_data(
    storage: &TimescaleDBStorage,
    records: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = generate_market_data(vec!["AAPL", "GOOGL", "MSFT", "AMZN", "TSLA"], records);

    storage.insert_batch(data).await?;
    Ok(())
}

fn generate_benchmark_timeseries(size: usize) -> TimeSeriesData {
    use rand::prelude::*;
    let mut rng = thread_rng();

    let mut prices = Vec::with_capacity(size);
    let mut volumes = Vec::with_capacity(size);
    let mut timestamps = Vec::with_capacity(size);

    let mut price = 100.0;
    let start = Utc::now() - Duration::from_secs((size * 60) as u64);

    for i in 0..size {
        price += (rng.gen::<f64>() - 0.5) * 2.0;
        prices.push(price.max(1.0));
        volumes.push((rng.gen::<f64>() * 1_000_000.0) as i64);
        timestamps.push(start + Duration::from_secs((i * 60) as u64));
    }

    TimeSeriesData {
        symbol: "BENCH".to_string(),
        timestamps,
        prices,
        volumes,
    }
}

async fn prepare_training_dataset(samples: usize) -> TrainingDataset {
    let features = Array2::random((samples, 50), Uniform::new(-1.0, 1.0));
    let labels = Array1::random(samples, Uniform::new(0.0, 1.0));

    TrainingDataset {
        features,
        labels,
        metadata: Default::default(),
    }
}

async fn create_benchmark_pipeline() -> TrainingPipeline {
    let storage = setup_test_timescale().await.unwrap();
    let cache = setup_test_redis().await.unwrap();
    let model_storage = ModelStorage::new("/tmp/bench_models");

    TrainingPipeline::builder()
        .with_storage(Arc::new(storage))
        .with_cache(Arc::new(cache))
        .with_model_storage(Arc::new(model_storage))
        .build()
        .unwrap()
}

fn create_model_bytes(size: usize) -> Vec<u8> {
    vec![0u8; size]
}

fn create_test_model_with_size(size: usize) -> TrainedModel {
    TrainedModel {
        id: Uuid::new_v4(),
        model_type: ModelType::MLP,
        weights: create_model_bytes(size),
        architecture: Default::default(),
        metadata: Default::default(),
    }
}

criterion_group!(
    benches,
    benchmark_data_loading,
    benchmark_feature_generation,
    benchmark_model_training,
    benchmark_model_persistence,
    benchmark_complete_pipeline,
    benchmark_market_hours
);

criterion_main!(benches);
