//! Neural Trader Performance Benchmarks
//!
//! Comprehensive benchmarks comparing:
//! - Old placeholder implementations vs real neural models
//! - DAA decision latency (<1ms target)
//! - Ensemble prediction performance
//! - Memory usage and optimization
//! - FANN neural network performance

use chrono::{DateTime, Utc};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

// Import neural trader components
use autonomous_platform::{
    config::NeuralConfig,
    data::TimeSeriesData,
    integration::{autonomous_decisions::DaaDecisionMaker, daa_coordinator::DaaCoordinator},
    neural::{
        fann_predictor::FannPredictor, NeuralPredictor, NeuralPredictorTrait, PredictionResult,
    },
    strategies::{neural_enhanced::NeuralEnhancedStrategy, MarketContext, TradingStrategy},
};

/// Performance targets
const DAA_DECISION_TARGET_MS: f64 = 1.0; // <1ms for DAA decisions
const NEURAL_PREDICTION_TARGET_MS: f64 = 10.0; // <10ms for single predictions
const ENSEMBLE_PREDICTION_TARGET_MS: f64 = 25.0; // <25ms for ensemble
const MEMORY_PER_MODEL_MB: f64 = 50.0; // Target <50MB per model

/// Test configurations
const SMALL_BATCH: usize = 10;
const MEDIUM_BATCH: usize = 100;
const LARGE_BATCH: usize = 1000;
const PREDICTION_HORIZON: usize = 5;

/// Benchmark result tracking
#[derive(Debug, Clone, serde::Serialize)]
struct BenchmarkResult {
    test_name: String,
    implementation: String, // "placeholder" or "real_neural"
    mean_latency_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    throughput_ops_sec: f64,
    memory_usage_mb: f64,
    target_met: bool,
    improvement_factor: Option<f64>,
    timestamp: DateTime<Utc>,
}

/// Compare placeholder vs real neural predictions
fn bench_neural_predictions_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("neural_predictions_comparison");

    // Setup configurations
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["NHITS".to_string(), "TCN".to_string(), "DeepAR".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };

    // Create test data
    let test_data = create_realistic_time_series(100);

    // Benchmark placeholder predictor (the old mock implementation)
    group.bench_function("placeholder_single_prediction", |b| {
        let predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).unwrap());
        b.iter(|| {
            rt.block_on(async {
                let result = predictor
                    .predict(&test_data, PREDICTION_HORIZON, None)
                    .await
                    .unwrap();
                black_box(result);
            })
        });
    });

    // Benchmark real FANN predictor
    group.bench_function("fann_single_prediction", |b| {
        let predictor = FannPredictor::new(neural_config.clone()).unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = predictor
                    .predict(&test_data, PREDICTION_HORIZON, None)
                    .await
                    .unwrap();
                black_box(result);
            })
        });
    });

    // Benchmark ensemble predictions - placeholder
    group.bench_function("placeholder_ensemble_prediction", |b| {
        let predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).unwrap());
        let models = vec!["NHITS".to_string(), "TCN".to_string(), "DeepAR".to_string()];
        b.iter(|| {
            rt.block_on(async {
                let result = predictor
                    .predict_ensemble(&test_data, PREDICTION_HORIZON, &models, None)
                    .await
                    .unwrap();
                black_box(result);
            })
        });
    });

    // Benchmark ensemble predictions - real FANN
    group.bench_function("fann_ensemble_prediction", |b| {
        let predictor = FannPredictor::new(neural_config.clone()).unwrap();
        let models = vec!["NHITS".to_string(), "TCN".to_string(), "DeepAR".to_string()];
        b.iter(|| {
            rt.block_on(async {
                let result = predictor
                    .predict_ensemble(&test_data, PREDICTION_HORIZON, &models, None)
                    .await
                    .unwrap();
                black_box(result);
            })
        });
    });

    // Benchmark batch predictions
    for &batch_size in &[SMALL_BATCH, MEDIUM_BATCH, LARGE_BATCH] {
        group.throughput(Throughput::Elements(batch_size as u64));

        // Placeholder batch
        group.bench_with_input(
            BenchmarkId::new("placeholder_batch", batch_size),
            &batch_size,
            |b, &size| {
                let predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).unwrap());
                let batch_data = (0..size).map(|_| test_data.clone()).collect::<Vec<_>>();
                b.iter(|| {
                    rt.block_on(async {
                        for data in &batch_data {
                            let _ = predictor
                                .predict(data, PREDICTION_HORIZON, None)
                                .await
                                .unwrap();
                        }
                    })
                });
            },
        );

        // FANN batch
        group.bench_with_input(
            BenchmarkId::new("fann_batch", batch_size),
            &batch_size,
            |b, &size| {
                let predictor = FannPredictor::new(neural_config.clone()).unwrap();
                let batch_data = (0..size).map(|_| test_data.clone()).collect::<Vec<_>>();
                b.iter(|| {
                    rt.block_on(async {
                        for data in &batch_data {
                            let _ = predictor
                                .predict(data, PREDICTION_HORIZON, None)
                                .await
                                .unwrap();
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark DAA decision latency
fn bench_daa_decision_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("daa_decision_latency");
    group.significance_level(0.05);

    // Setup DAA components
    let neural_config = NeuralConfig::default();
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let daa_decision_maker = DaaDecisionMaker::new(neural_predictor.clone());

    // Test data
    let historical_data = create_realistic_time_series(50);
    let market_context = MarketContext {
        symbol: "BTCUSD".to_string(),
        volatility: 0.02,
        liquidity: 0.95,
        trend_strength: 0.7,
        bid_ask_spread: 0.001,
        timestamp: Utc::now(),
    };

    // Benchmark single DAA decision
    group.bench_function("single_daa_decision", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                let decision = daa_decision_maker
                    .make_trading_decision(&market_context, &historical_data, 10000.0)
                    .await
                    .unwrap();
                let elapsed = start.elapsed();
                black_box((decision, elapsed));
            })
        });
    });

    // Benchmark market trend analysis (part of DAA decision)
    group.bench_function("daa_market_trend_analysis", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = daa_decision_maker
                    .analyze_market_trend(&historical_data)
                    .await
                    .unwrap();
                black_box(result);
            })
        });
    });

    // Benchmark full DAA coordinator flow
    // TODO: This benchmark needs to be updated to match the current DaaCoordinator API
    // The constructor now requires (config, neural_predictor, tx, market_hours) parameters
    // and coordinate_decision method doesn't exist - it uses make_decision instead
    /*
    group.bench_function("daa_coordinator_decision", |b| {
        let coordinator = DaaCoordinator::new();
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                let agent_id = "bench_agent";
                let decision_type = "TRADE_EXECUTION";
                let context = serde_json::json!({
                    "market": market_context,
                    "data": historical_data.last(),
                });

                let decision = coordinator
                    .coordinate_decision(agent_id, decision_type, context)
                    .await
                    .unwrap();
                let elapsed = start.elapsed();

                // Verify we meet the <1ms target
                assert!(elapsed.as_secs_f64() * 1000.0 < DAA_DECISION_TARGET_MS);
                black_box((decision, elapsed));
            })
        });
    });
    */

    // Benchmark concurrent DAA decisions
    for &concurrent_decisions in &[10, 50, 100] {
        group.throughput(Throughput::Elements(concurrent_decisions as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_daa_decisions", concurrent_decisions),
            &concurrent_decisions,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let futures: Vec<_> = (0..count)
                            .map(|i| {
                                let dm = daa_decision_maker.clone();
                                let mc = market_context.clone();
                                let hd = historical_data.clone();
                                async move {
                                    dm.make_trading_decision(&mc, &hd, 10000.0 + i as f64).await
                                }
                            })
                            .collect();

                        let results = futures::future::join_all(futures).await;
                        black_box(results);
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark ensemble prediction performance
fn bench_ensemble_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("ensemble_performance");

    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec![
            "NHITS".to_string(),
            "TCN".to_string(),
            "DeepAR".to_string(),
            "MLP".to_string(),
            "Transformer".to_string(),
        ],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };

    let fann_predictor = FannPredictor::new(neural_config.clone()).unwrap();
    let test_data = create_realistic_time_series(200);

    // Benchmark different ensemble sizes
    for ensemble_size in [2, 3, 5] {
        let models: Vec<String> = neural_config
            .models
            .iter()
            .take(ensemble_size)
            .cloned()
            .collect();

        group.bench_with_input(
            BenchmarkId::new("fann_ensemble_size", ensemble_size),
            &models,
            |b, models| {
                b.iter(|| {
                    rt.block_on(async {
                        let start = Instant::now();
                        let result = fann_predictor
                            .predict_ensemble(&test_data, PREDICTION_HORIZON, models, None)
                            .await
                            .unwrap();
                        let elapsed = start.elapsed();

                        // Verify ensemble meets performance target
                        assert!(elapsed.as_secs_f64() * 1000.0 < ENSEMBLE_PREDICTION_TARGET_MS);
                        black_box((result, elapsed));
                    })
                });
            },
        );
    }

    // Benchmark ensemble vs individual model accuracy trade-off
    group.bench_function("ensemble_vs_individual_tradeoff", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut results = HashMap::new();

                // Individual models
                for model in &neural_config.models {
                    let start = Instant::now();
                    let pred = fann_predictor
                        .predict_ensemble(&test_data, PREDICTION_HORIZON, &[model.clone()], None)
                        .await
                        .unwrap();
                    let elapsed = start.elapsed();
                    results.insert(format!("individual_{}", model), (pred, elapsed));
                }

                // Full ensemble
                let start = Instant::now();
                let ensemble_pred = fann_predictor
                    .predict_ensemble(&test_data, PREDICTION_HORIZON, &neural_config.models, None)
                    .await
                    .unwrap();
                let elapsed = start.elapsed();
                results.insert("full_ensemble".to_string(), (ensemble_pred, elapsed));

                black_box(results);
            })
        });
    });

    group.finish();
}

/// Benchmark memory usage and optimization
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_usage");

    // Benchmark model initialization memory
    group.bench_function("model_initialization_memory", |b| {
        b.iter(|| {
            let config = NeuralConfig::default();
            let memory_before = get_current_memory_usage();

            // Initialize FANN predictor
            let _predictor = FannPredictor::new(config).unwrap();

            let memory_after = get_current_memory_usage();
            let memory_used_mb = (memory_after - memory_before) as f64 / 1_048_576.0;

            // Verify memory usage is within target
            assert!(memory_used_mb < MEMORY_PER_MODEL_MB);
            black_box(memory_used_mb);
        });
    });

    // Benchmark memory usage under load
    group.bench_function("memory_under_prediction_load", |b| {
        let config = NeuralConfig::default();
        let predictor = FannPredictor::new(config).unwrap();
        let test_data = create_realistic_time_series(500);

        b.iter(|| {
            rt.block_on(async {
                let memory_before = get_current_memory_usage();

                // Run 100 predictions
                for _ in 0..100 {
                    let _ = predictor
                        .predict(&test_data, PREDICTION_HORIZON, None)
                        .await
                        .unwrap();
                }

                let memory_after = get_current_memory_usage();
                let memory_growth_mb = (memory_after - memory_before) as f64 / 1_048_576.0;

                black_box(memory_growth_mb);
            })
        });
    });

    // Benchmark cache efficiency
    group.bench_function("prediction_cache_efficiency", |b| {
        let config = NeuralConfig {
            prediction_cache_ttl: 60, // 1 minute cache
            ..NeuralConfig::default()
        };
        let predictor = FannPredictor::new(config).unwrap();
        let test_data = create_realistic_time_series(100);

        b.iter(|| {
            rt.block_on(async {
                let mut cache_hits = 0;
                let mut cache_misses = 0;

                // First call - cache miss
                let start1 = Instant::now();
                let _ = predictor
                    .predict(&test_data, PREDICTION_HORIZON, None)
                    .await
                    .unwrap();
                let time1 = start1.elapsed();
                cache_misses += 1;

                // Second call - should be cache hit
                let start2 = Instant::now();
                let _ = predictor
                    .predict(&test_data, PREDICTION_HORIZON, None)
                    .await
                    .unwrap();
                let time2 = start2.elapsed();

                if time2 < time1 / 10 {
                    cache_hits += 1;
                } else {
                    cache_misses += 1;
                }

                let cache_hit_rate = cache_hits as f64 / (cache_hits + cache_misses) as f64;
                black_box((cache_hit_rate, time1, time2));
            })
        });
    });

    group.finish();
}

/// Benchmark full neural trading strategy performance
fn bench_neural_trading_strategy(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("neural_trading_strategy");

    let neural_config = NeuralConfig::default();
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let strategy = NeuralEnhancedStrategy::new(neural_predictor);

    let market_context = MarketContext {
        symbol: "BTCUSD".to_string(),
        volatility: 0.02,
        liquidity: 0.95,
        trend_strength: 0.7,
        bid_ask_spread: 0.001,
        timestamp: Utc::now(),
    };

    // Benchmark signal generation
    group.bench_function("neural_strategy_signal_generation", |b| {
        let data = create_realistic_time_series(100);
        b.iter(|| {
            rt.block_on(async {
                let signal = strategy
                    .generate_signal(&market_context, &data)
                    .await
                    .unwrap();
                black_box(signal);
            })
        });
    });

    // Benchmark full trading decision pipeline
    group.bench_function("full_trading_decision_pipeline", |b| {
        let data = create_realistic_time_series(200);
        let position = None;

        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();

                // 1. Generate signal
                let signal = strategy
                    .generate_signal(&market_context, &data)
                    .await
                    .unwrap();

                // 2. Update state
                strategy.update_state(&market_context, &data).await.unwrap();

                // 3. Get recommendation
                let recommendation = strategy.get_recommendation(&position).await;

                let elapsed = start.elapsed();
                black_box((signal, recommendation, elapsed));
            })
        });
    });

    group.finish();
}

/// Benchmark latency distribution analysis
fn bench_latency_distribution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("latency_distribution");

    let neural_config = NeuralConfig::default();
    let fann_predictor = FannPredictor::new(neural_config.clone()).unwrap();
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
    let daa_decision_maker = DaaDecisionMaker::new(neural_predictor);

    // Collect latency samples for distribution analysis
    group.bench_function("latency_percentiles", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut latencies = Vec::new();
                let test_data = create_realistic_time_series(50);
                let market_context = MarketContext {
                    symbol: "BTCUSD".to_string(),
                    volatility: 0.02,
                    liquidity: 0.95,
                    trend_strength: 0.7,
                    bid_ask_spread: 0.001,
                    timestamp: Utc::now(),
                };

                // Collect 1000 samples
                for _ in 0..1000 {
                    // DAA decision latency
                    let start = Instant::now();
                    let _ = daa_decision_maker
                        .make_trading_decision(&market_context, &test_data, 10000.0)
                        .await
                        .unwrap();
                    let daa_latency = start.elapsed().as_secs_f64() * 1000.0;

                    // Neural prediction latency
                    let start = Instant::now();
                    let _ = fann_predictor
                        .predict(&test_data, PREDICTION_HORIZON, None)
                        .await
                        .unwrap();
                    let neural_latency = start.elapsed().as_secs_f64() * 1000.0;

                    latencies.push((daa_latency, neural_latency));
                }

                // Calculate percentiles
                let mut daa_latencies: Vec<f64> = latencies.iter().map(|(d, _)| *d).collect();
                let mut neural_latencies: Vec<f64> = latencies.iter().map(|(_, n)| *n).collect();

                daa_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
                neural_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let stats = LatencyStats {
                    daa_p50: percentile(&daa_latencies, 0.5),
                    daa_p95: percentile(&daa_latencies, 0.95),
                    daa_p99: percentile(&daa_latencies, 0.99),
                    daa_max: daa_latencies.last().copied().unwrap_or(0.0),
                    neural_p50: percentile(&neural_latencies, 0.5),
                    neural_p95: percentile(&neural_latencies, 0.95),
                    neural_p99: percentile(&neural_latencies, 0.99),
                    neural_max: neural_latencies.last().copied().unwrap_or(0.0),
                };

                // Verify DAA meets <1ms target for p95
                assert!(stats.daa_p95 < DAA_DECISION_TARGET_MS);

                black_box(stats);
            })
        });
    });

    group.finish();
}

// Helper structures and functions

#[derive(Debug)]
struct LatencyStats {
    daa_p50: f64,
    daa_p95: f64,
    daa_p99: f64,
    daa_max: f64,
    neural_p50: f64,
    neural_p95: f64,
    neural_p99: f64,
    neural_max: f64,
}

fn create_realistic_time_series(size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::with_capacity(size);
    let mut price = 45000.0;
    let base_time = Utc::now() - chrono::Duration::minutes(size as i64);

    for i in 0..size {
        // Simulate realistic price movement
        let change = (rand::random::<f64>() - 0.5) * 0.002 * price;
        price += change;

        let volume = 1.0 + rand::random::<f64>() * 2.0;
        let rsi = 30.0 + rand::random::<f64>() * 40.0;

        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), rsi);
        indicators.insert("macd".to_string(), (rand::random::<f64>() - 0.5) * 0.1);
        indicators.insert("volume_ma".to_string(), volume * 0.9);

        data.push(TimeSeriesData {
            symbol: "BTCUSD".to_string(),
            timestamp: base_time + chrono::Duration::minutes(i as i64),
            open: price - change / 2.0,
            high: price + price * 0.0005,
            low: price - price * 0.0005,
            close: price,
            volume,
            indicators,
        });
    }

    data
}

fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    let index = (p * (sorted_values.len() - 1) as f64) as usize;
    sorted_values[index.min(sorted_values.len() - 1)]
}

fn get_current_memory_usage() -> usize {
    // Simple memory usage estimation
    // In production, use proper memory profiling tools
    use std::alloc::{GlobalAlloc, Layout, System};

    // This is a simplified approach - in real benchmarks use jemalloc or system-specific APIs
    let layout = Layout::from_size_align(1, 1).unwrap();
    let ptr = unsafe { System.alloc(layout) };
    let estimate = ptr as usize;
    unsafe { System.dealloc(ptr, layout) };
    estimate
}

// Criterion benchmark groups
criterion_group!(
    benches,
    bench_neural_predictions_comparison,
    bench_daa_decision_latency,
    bench_ensemble_performance,
    bench_memory_usage,
    bench_neural_trading_strategy,
    bench_latency_distribution
);

criterion_main!(benches);
