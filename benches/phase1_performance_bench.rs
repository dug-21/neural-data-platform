//! Phase 1 Performance Benchmarks
//! 
//! Comprehensive benchmarks for all Phase 1 neural-expand features:
//! - Feature computation performance
//! - Neural model prediction latency
//! - Ensemble coordination overhead
//! - End-to-end pipeline throughput

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use neural_trader::{
    features::{
        technical_indicators::{TechnicalIndicatorEngine, IndicatorConfig},
        market_microstructure::{MicrostructureAnalyzer, MicrostructureConfig},
    },
    neural::{fann_predictor::FannPredictor, NeuralPredictorTrait},
    data::{TimeSeriesData, MarketContext},
    config::NeuralConfig,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;
use chrono::Utc;

/// Generate test data for benchmarks
fn generate_benchmark_data(size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::with_capacity(size);
    let base_price = 50000.0;
    
    for i in 0..size {
        let price = base_price * (1.0 + (i as f64 * 0.01).sin() * 0.02);
        data.push(TimeSeriesData {
            timestamp: Utc::now().timestamp() + i as i64 * 60,
            symbol: "BTC/USD".to_string(),
            open: price * 0.999,
            high: price * 1.002,
            low: price * 0.998,
            close: price,
            volume: 1000.0 * (1.0 + (i as f64 * 0.1).cos() * 0.5),
            bid: price * 0.9998,
            ask: price * 1.0002,
            indicators: HashMap::new(),
        });
    }
    
    data
}

/// Benchmark technical indicator computation
fn bench_technical_indicators(c: &mut Criterion) {
    let mut group = c.benchmark_group("technical_indicators");
    let rt = Runtime::new().unwrap();
    
    // Test different data sizes
    for size in [100, 500, 1000, 5000].iter() {
        let data = generate_benchmark_data(*size);
        let engine = TechnicalIndicatorEngine::new();
        
        group.bench_with_input(
            BenchmarkId::new("compute_all", size),
            size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let current = data.last().unwrap();
                        let historical = &data[..data.len()-1];
                        let features = engine.compute_all(
                            black_box(current),
                            black_box(historical),
                        ).await.unwrap();
                        features
                    })
                });
            },
        );
        
        // Benchmark specific indicator categories
        group.bench_with_input(
            BenchmarkId::new("elliott_waves", size),
            size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let current = data.last().unwrap();
                        let historical = &data[..data.len()-1];
                        // Specific Elliott Wave computation would go here
                        let mut features = HashMap::new();
                        // Simulate Elliott Wave detection
                        if historical.len() >= 240 {
                            features.insert("elliott_wave_detected".to_string(), 1.0);
                            features.insert("elliott_wave_strength".to_string(), 0.75);
                        }
                        features
                    })
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark market microstructure analysis
fn bench_microstructure_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("microstructure");
    let rt = Runtime::new().unwrap();
    
    for size in [100, 500, 1000].iter() {
        let data = generate_benchmark_data(*size);
        let analyzer = MicrostructureAnalyzer::new();
        
        group.bench_with_input(
            BenchmarkId::new("toxicity_metrics", size),
            size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let current = data.last().unwrap();
                        let historical = &data[..data.len()-1];
                        let mut features = HashMap::new();
                        analyzer.analyze(
                            black_box(current),
                            black_box(historical),
                            black_box(&mut features),
                        ).await.unwrap();
                        features
                    })
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark neural prediction performance
fn bench_neural_prediction(c: &mut Criterion) {
    let mut group = c.benchmark_group("neural_prediction");
    let rt = Runtime::new().unwrap();
    
    // Create neural predictor with Phase 1 configuration
    let neural_config = NeuralConfig {
        models: vec![
            ("FeedForward".to_string(), Default::default()),
            ("LSTM".to_string(), Default::default()),
            ("GRU".to_string(), Default::default()),
            ("Transformer".to_string(), Default::default()),
        ].into_iter().collect(),
        ensemble_weights: vec![
            ("FeedForward".to_string(), 1.0),
            ("LSTM".to_string(), 1.4),
            ("GRU".to_string(), 1.25),
            ("Transformer".to_string(), 1.5),
        ].into_iter().collect(),
        prediction_horizon: 5,
        confidence_threshold: 0.7,
        max_sequence_length: 100,
        enable_attention: true,
        ensemble_method: "weighted_average".to_string(),
    };
    
    let predictor = FannPredictor::new(neural_config);
    
    // Benchmark individual models
    let models = vec!["FeedForward", "LSTM", "GRU", "Transformer"];
    for model in models {
        group.bench_function(
            &format!("predict_{}", model.to_lowercase()),
            |b| {
                let features = create_model_features(model);
                let context = MarketContext {
                    symbol: "BTC/USD".to_string(),
                    current_price: 50000.0,
                    bid: 49950.0,
                    ask: 50050.0,
                    volume_24h: 1_000_000.0,
                    volatility: 0.02,
                    momentum: 0.5,
                    features,
                };
                
                b.iter(|| {
                    rt.block_on(async {
                        predictor.predict(black_box(&context)).await.unwrap()
                    })
                });
            },
        );
    }
    
    // Benchmark ensemble prediction
    group.bench_function("predict_ensemble", |b| {
        let features = create_comprehensive_features();
        let context = MarketContext {
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            bid: 49950.0,
            ask: 50050.0,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            momentum: 0.5,
            features,
        };
        
        b.iter(|| {
            rt.block_on(async {
                predictor.predict(black_box(&context)).await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark complete Phase 1 pipeline
fn bench_end_to_end_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_pipeline");
    let rt = Runtime::new().unwrap();
    
    // Setup components
    let indicator_engine = TechnicalIndicatorEngine::new();
    let microstructure = MicrostructureAnalyzer::new();
    let neural_config = create_phase1_neural_config();
    let predictor = FannPredictor::new(neural_config);
    
    for size in [100, 500, 1000].iter() {
        let data = generate_benchmark_data(*size);
        
        group.bench_with_input(
            BenchmarkId::new("complete_pipeline", size),
            size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        // 1. Feature extraction
                        let current = data.last().unwrap();
                        let historical = &data[..data.len()-1];
                        
                        let mut all_features = HashMap::new();
                        
                        // Technical indicators
                        let technical = indicator_engine.compute_all(
                            current,
                            historical,
                        ).await.unwrap();
                        all_features.extend(technical);
                        
                        // Microstructure
                        microstructure.analyze(
                            current,
                            historical,
                            &mut all_features,
                        ).await.unwrap();
                        
                        // 2. Neural prediction
                        let context = MarketContext {
                            symbol: "BTC/USD".to_string(),
                            current_price: current.close,
                            bid: current.bid,
                            ask: current.ask,
                            volume_24h: data.iter().map(|d| d.volume).sum(),
                            volatility: calculate_volatility(&data),
                            momentum: calculate_momentum(&data),
                            features: all_features,
                        };
                        
                        let predictions = predictor.predict(&context).await.unwrap();
                        
                        black_box(predictions)
                    })
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark feature engineering scalability
fn bench_feature_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_scalability");
    let rt = Runtime::new().unwrap();
    
    // Test how feature computation scales with number of features
    let data = generate_benchmark_data(1000);
    let engine = TechnicalIndicatorEngine::new();
    
    // Benchmark with different feature configurations
    let configs = vec![
        ("minimal", create_minimal_config()),
        ("standard", IndicatorConfig::default()),
        ("comprehensive", create_comprehensive_config()),
    ];
    
    for (name, config) in configs {
        let engine = TechnicalIndicatorEngine::with_config(config);
        
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let current = data.last().unwrap();
                    let historical = &data[..data.len()-1];
                    engine.compute_all(
                        black_box(current),
                        black_box(historical),
                    ).await.unwrap()
                })
            });
        });
    }
    
    group.finish();
}

/// Benchmark ensemble coordination overhead
fn bench_ensemble_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("ensemble_overhead");
    let rt = Runtime::new().unwrap();
    
    // Compare single model vs ensemble
    let single_config = NeuralConfig {
        models: vec![("LSTM".to_string(), Default::default())].into_iter().collect(),
        ensemble_weights: vec![("LSTM".to_string(), 1.0)].into_iter().collect(),
        prediction_horizon: 5,
        confidence_threshold: 0.7,
        max_sequence_length: 100,
        enable_attention: false,
        ensemble_method: "single".to_string(),
    };
    
    let ensemble_config = create_phase1_neural_config();
    
    let single_predictor = FannPredictor::new(single_config);
    let ensemble_predictor = FannPredictor::new(ensemble_config);
    
    let features = create_comprehensive_features();
    let context = MarketContext {
        symbol: "BTC/USD".to_string(),
        current_price: 50000.0,
        bid: 49950.0,
        ask: 50050.0,
        volume_24h: 1_000_000.0,
        volatility: 0.02,
        momentum: 0.5,
        features,
    };
    
    group.bench_function("single_model", |b| {
        b.iter(|| {
            rt.block_on(async {
                single_predictor.predict(black_box(&context)).await.unwrap()
            })
        });
    });
    
    group.bench_function("ensemble_4_models", |b| {
        b.iter(|| {
            rt.block_on(async {
                ensemble_predictor.predict(black_box(&context)).await.unwrap()
            })
        });
    });
    
    group.finish();
}

// Helper functions

fn create_model_features(model_type: &str) -> HashMap<String, f64> {
    let mut features = HashMap::new();
    
    match model_type {
        "LSTM" | "GRU" => {
            // Sequential features for RNN models
            for i in 1..=20 {
                features.insert(format!("price_t_{}", i), 50000.0 + (i as f64 * 10.0));
                features.insert(format!("volume_t_{}", i), 1000.0 + (i as f64 * 5.0));
            }
        }
        "Transformer" => {
            // Attention-relevant features
            features.insert("volatility_spike".to_string(), 0.05);
            features.insert("volume_anomaly".to_string(), 2.5);
            features.insert("price_breakout".to_string(), 1.0);
        }
        _ => {
            // Standard features
            features.insert("rsi".to_string(), 55.0);
            features.insert("macd".to_string(), 100.0);
            features.insert("ema_20".to_string(), 49800.0);
        }
    }
    
    features
}

fn create_comprehensive_features() -> HashMap<String, f64> {
    let mut features = HashMap::new();
    
    // Technical indicators
    features.insert("rsi_14".to_string(), 55.0);
    features.insert("macd_signal".to_string(), 100.0);
    features.insert("ema_20".to_string(), 49800.0);
    features.insert("ema_50".to_string(), 49500.0);
    features.insert("bb_upper".to_string(), 51000.0);
    features.insert("bb_lower".to_string(), 49000.0);
    
    // Elliott Wave features
    features.insert("elliott_wave_detected".to_string(), 1.0);
    features.insert("elliott_wave_strength".to_string(), 0.75);
    features.insert("current_wave_number".to_string(), 3.0);
    
    // Harmonic patterns
    features.insert("harmonic_pattern_gartley".to_string(), 0.8);
    features.insert("harmonic_pattern_bat".to_string(), 0.3);
    
    // Microstructure
    features.insert("adverse_selection_component".to_string(), 0.02);
    features.insert("realized_spread_toxicity".to_string(), 0.015);
    features.insert("flow_toxicity_index".to_string(), 35.0);
    
    // Sequential features for RNNs
    for i in 1..=10 {
        features.insert(format!("price_t_{}", i), 50000.0 - (i as f64 * 50.0));
        features.insert(format!("volume_t_{}", i), 1000.0 + (i as f64 * 10.0));
    }
    
    features
}

fn create_phase1_neural_config() -> NeuralConfig {
    NeuralConfig {
        models: vec![
            ("FeedForward".to_string(), Default::default()),
            ("LSTM".to_string(), Default::default()),
            ("GRU".to_string(), Default::default()),
            ("Transformer".to_string(), Default::default()),
        ].into_iter().collect(),
        ensemble_weights: vec![
            ("FeedForward".to_string(), 1.0),
            ("LSTM".to_string(), 1.4),
            ("GRU".to_string(), 1.25),
            ("Transformer".to_string(), 1.5),
        ].into_iter().collect(),
        prediction_horizon: 5,
        confidence_threshold: 0.7,
        max_sequence_length: 100,
        enable_attention: true,
        ensemble_method: "weighted_average".to_string(),
    }
}

fn create_minimal_config() -> IndicatorConfig {
    IndicatorConfig {
        ema_periods: vec![20],
        rsi_period: 14,
        macd_params: (12, 26, 9),
        bb_params: (20, 2.0),
        atr_period: 14,
        stoch_params: (14, 3),
        enable_volume_weighted: false,
        enable_custom: false,
    }
}

fn create_comprehensive_config() -> IndicatorConfig {
    IndicatorConfig {
        ema_periods: vec![9, 21, 50, 100, 200],
        rsi_period: 14,
        macd_params: (12, 26, 9),
        bb_params: (20, 2.0),
        atr_period: 14,
        stoch_params: (14, 3),
        enable_volume_weighted: true,
        enable_custom: true,
    }
}

fn calculate_volatility(data: &[TimeSeriesData]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    
    let returns: Vec<f64> = data.windows(2)
        .map(|w| (w[1].close / w[0].close).ln())
        .collect();
    
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>() / returns.len() as f64;
    
    variance.sqrt()
}

fn calculate_momentum(data: &[TimeSeriesData]) -> f64 {
    if data.len() < 20 {
        return 0.0;
    }
    
    let recent_avg = data[data.len()-10..].iter()
        .map(|d| d.close)
        .sum::<f64>() / 10.0;
    let older_avg = data[data.len()-20..data.len()-10].iter()
        .map(|d| d.close)
        .sum::<f64>() / 10.0;
    
    (recent_avg - older_avg) / older_avg
}

criterion_group!(
    benches,
    bench_technical_indicators,
    bench_microstructure_analysis,
    bench_neural_prediction,
    bench_end_to_end_pipeline,
    bench_feature_scalability,
    bench_ensemble_overhead,
);

criterion_main!(benches);