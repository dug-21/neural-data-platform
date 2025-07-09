//! Benchmarks for Neural Predictions vs Placeholder Implementation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use autonomous_platform::neural::{NeuralPredictor, PredictionResult, NeuralPredictorTrait};
use autonomous_platform::neural::fann_predictor::FannPredictor;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use chrono::Utc;
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn create_test_data(size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::with_capacity(size);
    let base_price = 50000.0;
    
    for i in 0..size {
        let variation = (i as f64 * 0.01).sin() * 0.02;
        let price = base_price * (1.0 + variation);
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + 20.0 * variation);
        
        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
            open: price * 0.999,
            high: price * 1.001,
            low: price * 0.998,
            close: price,
            volume: 1000.0 * (1.0 + variation.abs()),
            indicators,
            source: Some("benchmark".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    data
}

fn benchmark_placeholder_prediction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = NeuralPredictor::new(config).unwrap();
    
    let mut group = c.benchmark_group("placeholder_predictions");
    
    for size in [100, 500, 1000].iter() {
        let data = create_test_data(*size);
        
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    let _ = predictor.predict(black_box(&data), 5, None).await;
                })
            });
        });
    }
    
    group.finish();
}

fn benchmark_fann_prediction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    
    let mut group = c.benchmark_group("fann_predictions");
    
    for size in [100, 500, 1000].iter() {
        let data = create_test_data(*size);
        
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    let _ = predictor.predict(black_box(&data), 5, None).await;
                })
            });
        });
    }
    
    group.finish();
}

fn benchmark_ensemble_prediction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "TCN".to_string(), "NHITS".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config.clone()).unwrap();
    
    let mut group = c.benchmark_group("ensemble_predictions");
    
    let data = create_test_data(500);
    let models = config.models.clone();
    
    group.bench_function("3_models", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = predictor.predict_ensemble(
                    black_box(&data), 
                    5, 
                    black_box(&models), 
                    None
                ).await;
            })
        });
    });
    
    group.finish();
}

fn benchmark_prediction_with_caching(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let data = create_test_data(200);
    
    let mut group = c.benchmark_group("cached_predictions");
    
    // First call (uncached)
    group.bench_function("first_call", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = predictor.predict(black_box(&data), 5, None).await;
            })
        });
    });
    
    // Subsequent calls (cached)
    group.bench_function("cached_call", |b| {
        // Warm up cache
        rt.block_on(async {
            let _ = predictor.predict(&data, 5, None).await;
        });
        
        b.iter(|| {
            rt.block_on(async {
                let _ = predictor.predict(black_box(&data), 5, None).await;
            })
        });
    });
    
    group.finish();
}

fn benchmark_prediction_horizons(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let data = create_test_data(300);
    
    let mut group = c.benchmark_group("prediction_horizons");
    
    for horizon in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(horizon), 
            horizon, 
            |b, &horizon| {
                b.iter(|| {
                    rt.block_on(async {
                        let _ = predictor.predict(
                            black_box(&data), 
                            black_box(horizon), 
                            None
                        ).await;
                    })
                });
            }
        );
    }
    
    group.finish();
}

fn benchmark_model_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let data = create_test_data(200);
    
    let mut group = c.benchmark_group("model_comparison");
    
    let models = vec!["MLP", "TCN", "NHITS", "DeepAR", "Transformer"];
    
    for model in models {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec![model.to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
        };
        
        let predictor = FannPredictor::new(config).unwrap();
        
        group.bench_function(model, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let _ = predictor.predict(black_box(&data), 5, None).await;
                })
            });
        });
    }
    
    group.finish();
}

fn benchmark_concurrent_predictions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = FannPredictor::new(config).unwrap();
    let data = create_test_data(200);
    
    let mut group = c.benchmark_group("concurrent_predictions");
    
    for num_concurrent in [1, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_concurrent), 
            num_concurrent, 
            |b, &num_concurrent| {
                b.iter(|| {
                    rt.block_on(async {
                        let futures: Vec<_> = (0..num_concurrent)
                            .map(|_| predictor.predict(&data, 5, None))
                            .collect();
                        
                        let _ = futures::future::join_all(futures).await;
                    })
                });
            }
        );
    }
    
    group.finish();
}

fn benchmark_feature_extraction(c: &mut Criterion) {
    let data = create_test_data(1000);
    
    c.bench_function("feature_extraction", |b| {
        b.iter(|| {
            for window in data.windows(30) {
                // Extract features similar to FANN predictor
                let mut features = Vec::with_capacity(90);
                
                for item in window {
                    let price_norm = (item.close - window[0].close) / window[0].close;
                    let volume_norm = (item.volume / 1_000_000.0).ln();
                    let rsi = item.indicators.get("rsi").copied().unwrap_or(50.0) / 100.0;
                    
                    features.push(black_box(price_norm));
                    features.push(black_box(volume_norm));
                    features.push(black_box(rsi));
                }
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_placeholder_prediction,
    benchmark_fann_prediction,
    benchmark_ensemble_prediction,
    benchmark_prediction_with_caching,
    benchmark_prediction_horizons,
    benchmark_model_comparison,
    benchmark_concurrent_predictions,
    benchmark_feature_extraction
);

criterion_main!(benches);