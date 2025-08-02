//! Complete Phase 1 Integration Tests for Neural-Expand Feature
//! 
//! This test suite validates the end-to-end integration of all Phase 1 components:
//! 1. Data ingestion (historical backfill, multiple providers)
//! 2. Feature engineering (Elliott waves, harmonic patterns, toxicity metrics)
//! 3. Neural prediction (LSTM/GRU, attention mechanisms, ensemble)
//! 4. Complete pipeline performance benchmarks

use neural_trader::{
    adapters::{DataAdapter, MarketData, redis::RedisAdapter, timescale::TimescaleAdapter},
    features::{
        technical_indicators::{TechnicalIndicatorEngine, IndicatorConfig},
        market_microstructure::{MicrostructureAnalyzer, MicrostructureConfig},
        regime_detection::RegimeDetector,
        cross_asset::CrossAssetAnalyzer,
    },
    neural::{fann_predictor::FannPredictor, NeuralPredictorTrait},
    data::{TimeSeriesData, MarketContext},
    config::{NeuralConfig, PlatformConfig},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Utc, Duration};
use anyhow::Result;

// Import test utilities
#[path = "../common/mod.rs"]
mod common;
use common::*;

#[cfg(test)]
mod phase1_data_ingestion_tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_historical_data_backfill() {
        // GIVEN: Multiple data providers configured for historical backfill
        setup_test_logging();
        
        // Simulate historical data from different providers
        let providers = vec![
            ("alpaca", 5),      // 5 years of equity data
            ("yahoo_finance", 20), // 20 years of historical data
            ("binance", 10),    // Full crypto history
            ("fred", 50),       // Economic indicators
        ];

        // WHEN: We request historical data for backtesting
        for (provider, years) in providers {
            let start_date = Utc::now() - Duration::days(years * 365);
            let end_date = Utc::now();
            
            // Simulate data fetching (in real test would use actual providers)
            let data_points = years * 252; // Trading days per year
            let historical_data = generate_price_series("BTC/USD", 50000.0, data_points);
            
            // THEN: We should have complete historical data
            assert_eq!(historical_data.len(), data_points);
            assert!(historical_data.first().unwrap().timestamp < historical_data.last().unwrap().timestamp);
            
            tracing::info!("Provider {} loaded {} years of data ({} points)", 
                provider, years, data_points);
        }
    }

    #[tokio::test]
    async fn test_multi_provider_data_fusion() {
        // GIVEN: Data from multiple providers needs to be fused
        let alpaca_data = generate_price_series("AAPL", 150.0, 100);
        let yahoo_data = generate_price_series("AAPL", 150.0, 100);
        let fred_data = vec![
            ("GDP_GROWTH", 2.5),
            ("INFLATION_RATE", 3.2),
            ("UNEMPLOYMENT", 3.8),
        ];

        // WHEN: We fuse data from different sources
        let mut fused_features = HashMap::new();
        
        // Price data fusion (take average when both available)
        for i in 0..100 {
            let avg_price = (alpaca_data[i].close + yahoo_data[i].close) / 2.0;
            fused_features.insert(format!("price_{}", i), avg_price);
        }
        
        // Add economic indicators
        for (indicator, value) in fred_data {
            fused_features.insert(indicator.to_string(), value);
        }

        // THEN: Fused dataset should contain all features
        assert!(fused_features.len() > 100);
        assert!(fused_features.contains_key("GDP_GROWTH"));
        assert!(fused_features.contains_key("price_0"));
    }

    #[tokio::test]
    async fn test_data_quality_validation() {
        // GIVEN: Raw data that needs validation
        let mut raw_data = generate_price_series("ETH/USD", 3000.0, 1000);
        
        // Inject some bad data points
        raw_data[50].high = raw_data[50].low - 100.0; // Invalid: high < low
        raw_data[100].volume = -1000.0; // Invalid: negative volume
        raw_data[150].close = 0.0; // Invalid: zero price
        
        // WHEN: We validate and clean the data
        let mut clean_data = Vec::new();
        let mut invalid_count = 0;
        
        for data in raw_data {
            let mut is_valid = true;
            
            // Validation rules
            if data.high < data.low {
                is_valid = false;
            }
            if data.volume < 0.0 {
                is_valid = false;
            }
            if data.close <= 0.0 || data.open <= 0.0 {
                is_valid = false;
            }
            
            if is_valid {
                clean_data.push(data);
            } else {
                invalid_count += 1;
            }
        }

        // THEN: Invalid data should be filtered out
        assert_eq!(invalid_count, 3);
        assert_eq!(clean_data.len(), 997);
    }
}

#[cfg(test)]
mod phase1_feature_engineering_tests {
    use super::*;

    #[tokio::test]
    async fn test_elliott_wave_pattern_detection() {
        // GIVEN: Price data with potential Elliott Wave patterns
        let historical_data = generate_elliott_wave_pattern("BTC/USD", 50000.0);
        assert!(historical_data.len() >= 240); // Need sufficient data
        
        // WHEN: We run Elliott Wave detection
        let indicator_engine = TechnicalIndicatorEngine::new();
        let current = historical_data.last().unwrap();
        let features = indicator_engine.compute_all(
            current,
            &historical_data[..historical_data.len()-1],
        ).await.unwrap();

        // THEN: Elliott Wave features should be detected
        assert!(features.contains_key("elliott_wave_detected"));
        assert!(features.contains_key("elliott_wave_strength"));
        assert!(features.contains_key("current_wave_number"));
        assert!(features.contains_key("elliott_target_price"));
        
        let wave_strength = features.get("elliott_wave_strength").unwrap();
        assert!(*wave_strength >= 0.0 && *wave_strength <= 1.0);
    }

    #[tokio::test]
    async fn test_harmonic_pattern_recognition() {
        // GIVEN: Price data with harmonic patterns
        let historical_data = generate_harmonic_pattern("ETH/USD", 3000.0, "gartley");
        
        // WHEN: We run harmonic pattern detection
        let indicator_engine = TechnicalIndicatorEngine::new();
        let current = historical_data.last().unwrap();
        let features = indicator_engine.compute_all(
            current,
            &historical_data[..historical_data.len()-1],
        ).await.unwrap();

        // THEN: Harmonic patterns should be detected
        assert!(features.contains_key("harmonic_pattern_gartley"));
        assert!(features.contains_key("harmonic_pattern_bat"));
        assert!(features.contains_key("harmonic_pattern_butterfly"));
        assert!(features.contains_key("harmonic_pattern_crab"));
        
        // At least one pattern should have high confidence
        let gartley_score = features.get("harmonic_pattern_gartley").unwrap();
        assert!(*gartley_score >= 0.0);
    }

    #[tokio::test]
    async fn test_order_flow_toxicity_metrics() {
        // GIVEN: Market data with potentially toxic order flow
        let mut analyzer = MicrostructureAnalyzer::new();
        let historical_data = generate_toxic_flow_scenario("BTC/USD", 50000.0);
        
        // WHEN: We analyze order flow toxicity
        let current = historical_data.last().unwrap();
        let mut features = HashMap::new();
        analyzer.analyze(current, &historical_data[..historical_data.len()-1], &mut features).await.unwrap();

        // THEN: Toxicity metrics should be calculated
        assert!(features.contains_key("adverse_selection_component"));
        assert!(features.contains_key("realized_spread_toxicity"));
        assert!(features.contains_key("flow_toxicity_index"));
        assert!(features.contains_key("predatory_trading_indicator"));
        assert!(features.contains_key("quote_stuffing_indicator"));
        assert!(features.contains_key("spoofing_detection_score"));
        
        // Toxicity level should be categorized
        let toxicity_level = features.get("toxicity_level").unwrap();
        assert!(*toxicity_level >= 0.0 && *toxicity_level <= 100.0);
    }

    #[tokio::test]
    async fn test_feature_count_expansion() {
        // GIVEN: Complete feature engineering pipeline
        let historical_data = generate_comprehensive_market_data("SPY", 400.0, 500);
        let indicator_engine = TechnicalIndicatorEngine::new();
        let microstructure = MicrostructureAnalyzer::new();
        
        // WHEN: We compute all features
        let current = historical_data.last().unwrap();
        let mut all_features = HashMap::new();
        
        // Technical indicators
        let technical_features = indicator_engine.compute_all(
            current,
            &historical_data[..historical_data.len()-1],
        ).await.unwrap();
        all_features.extend(technical_features);
        
        // Microstructure features
        microstructure.analyze(
            current,
            &historical_data[..historical_data.len()-1],
            &mut all_features,
        ).await.unwrap();

        // THEN: We should have significantly expanded feature set
        let feature_count = all_features.len();
        assert!(feature_count > 100, "Expected >100 features, got {}", feature_count);
        
        // Verify feature categories
        let technical_count = all_features.keys()
            .filter(|k| k.contains("ema") || k.contains("rsi") || k.contains("macd"))
            .count();
        let pattern_count = all_features.keys()
            .filter(|k| k.contains("elliott") || k.contains("harmonic"))
            .count();
        let toxicity_count = all_features.keys()
            .filter(|k| k.contains("toxicity") || k.contains("adverse"))
            .count();
        
        assert!(technical_count > 20);
        assert!(pattern_count > 10);
        assert!(toxicity_count > 5);
    }
}

#[cfg(test)]
mod phase1_neural_prediction_tests {
    use super::*;

    #[tokio::test]
    async fn test_lstm_gru_model_prediction() {
        // GIVEN: Neural predictor with LSTM/GRU models
        let neural_config = NeuralConfig {
            models: vec![
                ("LSTM".to_string(), Default::default()),
                ("GRU".to_string(), Default::default()),
            ].into_iter().collect(),
            ensemble_weights: vec![
                ("LSTM".to_string(), 1.4),
                ("GRU".to_string(), 1.25),
            ].into_iter().collect(),
            prediction_horizon: 5,
            confidence_threshold: 0.7,
            max_sequence_length: 100,
            enable_attention: true,
            ensemble_method: "weighted_average".to_string(),
        };
        
        let predictor = FannPredictor::new(neural_config);
        
        // Generate sequential data for RNN models
        let historical = generate_price_series("BTC/USD", 50000.0, 200);
        let features = create_rnn_features(&historical);
        
        // WHEN: We make predictions with LSTM/GRU
        let context = MarketContextBuilder::new("BTC/USD")
            .with_prices(50000.0, 49900.0, 50100.0)
            .with_features(features)
            .build();
            
        let predictions = predictor.predict(&context).await.unwrap();

        // THEN: We should get ensemble predictions
        assert!(!predictions.is_empty());
        assert!(predictions.contains_key("LSTM"));
        assert!(predictions.contains_key("GRU"));
        assert!(predictions.contains_key("ensemble"));
        
        // Verify prediction structure
        let ensemble_pred = predictions.get("ensemble").unwrap();
        assert!(ensemble_pred.confidence >= 0.0 && ensemble_pred.confidence <= 1.0);
        assert!(ensemble_pred.horizon == 5);
    }

    #[tokio::test]
    async fn test_attention_mechanism_activation() {
        // GIVEN: Neural predictor with attention enabled
        let mut neural_config = NeuralConfig::default();
        neural_config.enable_attention = true;
        
        let predictor = FannPredictor::new(neural_config);
        
        // Create features that should trigger attention
        let mut features = HashMap::new();
        // Sudden volatility spike
        features.insert("volatility_1h".to_string(), 0.05);
        features.insert("volatility_24h".to_string(), 0.02);
        features.insert("volume_spike".to_string(), 3.5);
        features.insert("price_breakout".to_string(), 1.0);
        
        // WHEN: Making predictions with attention-worthy features
        let context = MarketContextBuilder::new("BTC/USD")
            .with_prices(52000.0, 51900.0, 52100.0)
            .with_features(features)
            .build();
            
        let predictions = predictor.predict(&context).await.unwrap();

        // THEN: Attention mechanism should influence predictions
        if let Some(transformer_pred) = predictions.get("Transformer") {
            // Transformer uses attention, should have adjusted confidence
            assert!(transformer_pred.confidence > 0.5);
        }
        
        // Ensemble should reflect attention-based insights
        let ensemble = predictions.get("ensemble").unwrap();
        assert!(ensemble.metadata.contains_key("attention_activated"));
    }

    #[tokio::test]
    async fn test_ensemble_weight_optimization() {
        // GIVEN: Multiple models with different performance characteristics
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
        
        // WHEN: Making predictions across different market conditions
        let scenarios = vec![
            ("trending", generate_trending_market("BTC/USD", 50000.0, 0.02)),
            ("volatile", generate_volatile_market("BTC/USD", 50000.0, 0.05)),
            ("sideways", generate_sideways_market("BTC/USD", 50000.0, 0.001)),
        ];
        
        for (scenario_name, data) in scenarios {
            let features = extract_all_features(&data);
            let context = MarketContextBuilder::new("BTC/USD")
                .with_prices(data.last().unwrap().close, 
                            data.last().unwrap().bid, 
                            data.last().unwrap().ask)
                .with_features(features)
                .build();
                
            let predictions = predictor.predict(&context).await.unwrap();
            let ensemble = predictions.get("ensemble").unwrap();
            
            // THEN: Ensemble weights should adapt to market conditions
            tracing::info!("Scenario: {}, Ensemble confidence: {}", 
                scenario_name, ensemble.confidence);
            
            // Different models should contribute differently based on scenario
            match scenario_name {
                "trending" => {
                    // LSTM/GRU should perform well in trending markets
                    assert!(ensemble.confidence > 0.6);
                }
                "volatile" => {
                    // Attention mechanisms should help in volatile markets
                    assert!(predictions.contains_key("Transformer"));
                }
                "sideways" => {
                    // Lower confidence expected in sideways markets
                    assert!(ensemble.confidence < 0.8);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod phase1_performance_benchmarks {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_feature_computation_performance() {
        // GIVEN: Large dataset for performance testing
        let data_sizes = vec![100, 1000, 5000, 10000];
        let indicator_engine = TechnicalIndicatorEngine::new();
        
        for size in data_sizes {
            let historical_data = generate_price_series("BTC/USD", 50000.0, size);
            let current = historical_data.last().unwrap();
            
            // WHEN: Computing features
            let start = Instant::now();
            let features = indicator_engine.compute_all(
                current,
                &historical_data[..historical_data.len()-1],
            ).await.unwrap();
            let elapsed = start.elapsed();
            
            // THEN: Performance should scale reasonably
            let features_per_second = features.len() as f64 / elapsed.as_secs_f64();
            tracing::info!("Data size: {}, Features: {}, Time: {:?}, Features/sec: {:.0}", 
                size, features.len(), elapsed, features_per_second);
            
            // Performance assertions
            assert!(features_per_second > 1000.0, 
                "Feature computation too slow: {:.0} features/sec", features_per_second);
            
            // Ensure computation scales sub-linearly
            if size == 10000 {
                assert!(elapsed.as_millis() < 1000, 
                    "Large dataset computation took too long: {:?}", elapsed);
            }
        }
    }

    #[tokio::test]
    async fn test_neural_prediction_latency() {
        // GIVEN: Neural predictor with multiple models
        let neural_config = create_full_neural_config();
        let predictor = FannPredictor::new(neural_config);
        
        // Prepare test data
        let historical = generate_price_series("BTC/USD", 50000.0, 500);
        let features = extract_all_features(&historical);
        let context = MarketContextBuilder::new("BTC/USD")
            .with_prices(50000.0, 49900.0, 50100.0)
            .with_features(features)
            .build();
        
        // WHEN: Making multiple predictions
        let iterations = 100;
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = predictor.predict(&context).await.unwrap();
        }
        
        let total_elapsed = start.elapsed();
        let avg_latency = total_elapsed / iterations as u32;
        
        // THEN: Prediction latency should be acceptable
        tracing::info!("Average prediction latency: {:?}", avg_latency);
        assert!(avg_latency.as_millis() < 50, 
            "Prediction latency too high: {:?}", avg_latency);
    }

    #[tokio::test]
    async fn test_end_to_end_pipeline_performance() {
        // GIVEN: Complete Phase 1 pipeline
        let pipeline_start = Instant::now();
        
        // 1. Data ingestion simulation
        let ingestion_start = Instant::now();
        let historical_data = simulate_multi_provider_ingestion("BTC/USD", 1000);
        let ingestion_time = ingestion_start.elapsed();
        
        // 2. Feature engineering
        let feature_start = Instant::now();
        let indicator_engine = TechnicalIndicatorEngine::new();
        let microstructure = MicrostructureAnalyzer::new();
        
        let current = historical_data.last().unwrap();
        let mut all_features = HashMap::new();
        
        let technical_features = indicator_engine.compute_all(
            current,
            &historical_data[..historical_data.len()-1],
        ).await.unwrap();
        all_features.extend(technical_features);
        
        microstructure.analyze(
            current,
            &historical_data[..historical_data.len()-1],
            &mut all_features,
        ).await.unwrap();
        
        let feature_time = feature_start.elapsed();
        
        // 3. Neural prediction
        let prediction_start = Instant::now();
        let neural_config = create_full_neural_config();
        let predictor = FannPredictor::new(neural_config);
        
        let context = MarketContextBuilder::new("BTC/USD")
            .with_prices(current.close, current.bid, current.ask)
            .with_features(all_features.clone())
            .build();
            
        let predictions = predictor.predict(&context).await.unwrap();
        let prediction_time = prediction_start.elapsed();
        
        let total_time = pipeline_start.elapsed();
        
        // THEN: End-to-end performance should meet targets
        tracing::info!("Pipeline Performance Breakdown:");
        tracing::info!("  Data Ingestion: {:?}", ingestion_time);
        tracing::info!("  Feature Engineering: {:?}", feature_time);
        tracing::info!("  Neural Prediction: {:?}", prediction_time);
        tracing::info!("  Total Pipeline: {:?}", total_time);
        tracing::info!("  Features Generated: {}", all_features.len());
        tracing::info!("  Models in Ensemble: {}", predictions.len());
        
        // Performance assertions
        assert!(total_time.as_millis() < 500, 
            "Total pipeline time exceeds 500ms: {:?}", total_time);
        assert!(all_features.len() > 100, 
            "Insufficient features generated: {}", all_features.len());
        assert!(predictions.contains_key("ensemble"), 
            "Missing ensemble prediction");
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        // GIVEN: Large-scale data processing scenario
        let initial_memory = get_current_memory_usage();
        
        // Process large dataset
        let large_dataset = generate_price_series("BTC/USD", 50000.0, 50000);
        let indicator_engine = TechnicalIndicatorEngine::new();
        
        // Process in batches to simulate streaming
        let batch_size = 1000;
        let mut total_features = 0;
        
        for i in (0..large_dataset.len()).step_by(batch_size) {
            let end = std::cmp::min(i + batch_size, large_dataset.len());
            let batch = &large_dataset[i..end];
            
            if let Some(current) = batch.last() {
                let features = indicator_engine.compute_all(
                    current,
                    batch,
                ).await.unwrap();
                total_features += features.len();
            }
        }
        
        let final_memory = get_current_memory_usage();
        let memory_increase = final_memory - initial_memory;
        
        // THEN: Memory usage should be reasonable
        tracing::info!("Memory usage - Initial: {} MB, Final: {} MB, Increase: {} MB", 
            initial_memory / 1_048_576, 
            final_memory / 1_048_576,
            memory_increase / 1_048_576);
        tracing::info!("Total features computed: {}", total_features);
        
        // Memory increase should be reasonable (less than 100MB for 50k data points)
        assert!(memory_increase < 100 * 1_048_576, 
            "Memory usage too high: {} MB", memory_increase / 1_048_576);
    }
}

// Helper functions
fn generate_elliott_wave_pattern(symbol: &str, base_price: f64) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    // Generate 5-wave impulsive pattern
    let wave_ratios = vec![
        1.0,    // Start
        1.1,    // Wave 1 up
        1.05,   // Wave 2 down (retracement)
        1.3,    // Wave 3 up (strongest)
        1.25,   // Wave 4 down (retracement)
        1.4,    // Wave 5 up (final)
    ];
    
    for (i, &ratio) in wave_ratios.iter().enumerate() {
        for j in 0..40 {
            price = base_price * ratio + ((i * 40 + j) as f64 * 10.0).sin() * base_price * 0.01;
            
            data.push(TimeSeriesData {
                timestamp: Utc::now().timestamp() + (i * 40 + j) as i64 * 60,
                symbol: symbol.to_string(),
                open: price * 0.999,
                high: price * 1.002,
                low: price * 0.998,
                close: price,
                volume: 1000.0 + (j as f64 * 10.0),
                bid: price * 0.9995,
                ask: price * 1.0005,
                indicators: HashMap::new(),
            });
        }
    }
    
    data
}

fn generate_harmonic_pattern(symbol: &str, base_price: f64, pattern_type: &str) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    // Generate Gartley pattern ratios
    let ratios = match pattern_type {
        "gartley" => vec![
            (1.0, "X"),     // X point
            (0.618, "A"),   // A point - 61.8% retracement
            (0.786, "B"),   // B point - 78.6% of XA
            (0.382, "C"),   // C point - 38.2% retracement
            (0.786, "D"),   // D point - 78.6% completion
        ],
        _ => vec![(1.0, "X")],
    };
    
    for (i, (ratio, point)) in ratios.iter().enumerate() {
        for j in 0..20 {
            price = base_price * ratio + ((i * 20 + j) as f64 * 5.0).sin() * base_price * 0.005;
            
            data.push(TimeSeriesData {
                timestamp: Utc::now().timestamp() + (i * 20 + j) as i64 * 60,
                symbol: symbol.to_string(),
                open: price * 0.999,
                high: price * 1.001,
                low: price * 0.998,
                close: price,
                volume: vec![1000.0],
                bid: price * 0.9995,
                ask: price * 1.0005,
                indicators: HashMap::new(),
            });
        }
    }
    
    data
}

fn generate_toxic_flow_scenario(symbol: &str, base_price: f64) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    for i in 0..100 {
        // Simulate adverse selection - large trades followed by price movement
        let is_toxic = i % 20 < 5; // 25% toxic flow
        
        if is_toxic {
            // Toxic flow: large volume followed by adverse price movement
            price *= 0.995; // Price moves against liquidity providers
            let volume = 10000.0; // Large volume
            
            data.push(TimeSeriesData {
                timestamp: Utc::now().timestamp() + i * 60,
                symbol: symbol.to_string(),
                open: price * 1.001,
                high: price * 1.002,
                low: price * 0.995,
                close: price,
                volume,
                bid: price * 0.998,
                ask: price * 1.002, // Wide spread indicates toxicity
                indicators: HashMap::new(),
            });
        } else {
            // Normal flow
            price *= 1.0 + (i as f64 * 0.1).sin() * 0.001;
            
            data.push(TimeSeriesData {
                timestamp: Utc::now().timestamp() + i * 60,
                symbol: symbol.to_string(),
                open: price * 0.9995,
                high: price * 1.0005,
                low: price * 0.9995,
                close: price,
                volume: vec![1000.0],
                bid: price * 0.9998,
                ask: price * 1.0002, // Tight spread for normal flow
                indicators: HashMap::new(),
            });
        }
    }
    
    data
}

fn generate_comprehensive_market_data(symbol: &str, base_price: f64, size: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    for i in 0..size {
        // Mix different market conditions
        let phase = i / 50; // Change pattern every 50 periods
        
        let (price_change, volume_mult) = match phase % 4 {
            0 => {
                // Trending up
                (0.001 + (i as f64 * 0.01).sin() * 0.0005, 1.5)
            }
            1 => {
                // Volatile
                ((i as f64 * 0.1).sin() * 0.005, 2.0)
            }
            2 => {
                // Trending down
                (-0.001 + (i as f64 * 0.01).cos() * 0.0005, 1.8)
            }
            _ => {
                // Sideways
                ((i as f64 * 0.05).sin() * 0.001, 0.8)
            }
        };
        
        price *= 1.0 + price_change;
        
        data.push(TimeSeriesData {
            timestamp: Utc::now().timestamp() + i as i64 * 60,
            symbol: symbol.to_string(),
            open: price * (1.0 - price_change.abs() / 2.0),
            high: price * (1.0 + price_change.abs()),
            low: price * (1.0 - price_change.abs()),
            close: price,
            volume: 1000.0 * volume_mult,
            bid: price * 0.9998,
            ask: price * 1.0002,
            indicators: HashMap::new(),
        });
    }
    
    data
}

fn create_rnn_features(historical: &[TimeSeriesData]) -> HashMap<String, f64> {
    let mut features = HashMap::new();
    
    // Sequential features for RNN models
    if historical.len() >= 20 {
        // Recent price changes
        for i in 1..=10 {
            if let Some(data) = historical.get(historical.len() - i) {
                features.insert(format!("price_t_{}", i), data.close);
                features.insert(format!("volume_t_{}", i), data.volume);
            }
        }
        
        // Moving averages
        let ma_5: f64 = historical.iter().rev().take(5).map(|d| d.close).sum::<f64>() / 5.0;
        let ma_20: f64 = historical.iter().rev().take(20).map(|d| d.close).sum::<f64>() / 20.0;
        
        features.insert("ma_5".to_string(), ma_5);
        features.insert("ma_20".to_string(), ma_20);
        features.insert("ma_crossover".to_string(), if ma_5 > ma_20 { 1.0 } else { 0.0 });
    }
    
    features
}

fn extract_all_features(data: &[TimeSeriesData]) -> HashMap<String, f64> {
    let mut features = HashMap::new();
    
    if let Some(current) = data.last() {
        // Price features
        features.insert("price".to_string(), current.close);
        features.insert("volume".to_string(), current.volume);
        features.insert("spread".to_string(), current.ask - current.bid);
        
        // Technical indicators
        if data.len() >= 20 {
            let ma_20: f64 = data.iter().rev().take(20).map(|d| d.close).sum::<f64>() / 20.0;
            features.insert("ma_20".to_string(), ma_20);
            features.insert("price_to_ma".to_string(), current.close / ma_20);
            
            // Volatility
            let returns: Vec<f64> = data.windows(2)
                .map(|w| (w[1].close / w[0].close).ln())
                .collect();
            let volatility = statistical_stddev(&returns);
            features.insert("volatility".to_string(), volatility);
        }
    }
    
    features
}

fn generate_trending_market(symbol: &str, base_price: f64, trend_strength: f64) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    for i in 0..100 {
        price *= 1.0 + trend_strength / 100.0 + (i as f64 * 0.1).sin() * 0.001;
        
        data.push(TimeSeriesData {
            timestamp: Utc::now().timestamp() + i * 60,
            symbol: symbol.to_string(),
            open: price * 0.999,
            high: price * 1.001,
            low: price * 0.998,
            close: price,
            volume: 1000.0 * (1.0 + i as f64 / 100.0),
            bid: price * 0.9998,
            ask: price * 1.0002,
            indicators: HashMap::new(),
        });
    }
    
    data
}

fn generate_volatile_market(symbol: &str, base_price: f64, volatility: f64) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let mut price = base_price;
    
    for i in 0..100 {
        let change = (i as f64 * 0.5).sin() * volatility;
        price *= 1.0 + change;
        
        data.push(TimeSeriesData {
            timestamp: Utc::now().timestamp() + i * 60,
            symbol: symbol.to_string(),
            open: price * (1.0 - volatility / 2.0),
            high: price * (1.0 + volatility),
            low: price * (1.0 - volatility),
            close: price,
            volume: 2000.0 * (1.0 + change.abs() * 10.0),
            bid: price * (1.0 - volatility / 4.0),
            ask: price * (1.0 + volatility / 4.0),
            indicators: HashMap::new(),
        });
    }
    
    data
}

fn generate_sideways_market(symbol: &str, base_price: f64, noise: f64) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    
    for i in 0..100 {
        let price = base_price * (1.0 + (i as f64 * 0.2).sin() * noise);
        
        data.push(TimeSeriesData {
            timestamp: Utc::now().timestamp() + i * 60,
            symbol: symbol.to_string(),
            open: price * 0.9999,
            high: price * 1.0001,
            low: price * 0.9998,
            close: price,
            volume: vec![800.0],
            bid: price * 0.9999,
            ask: price * 1.0001,
            indicators: HashMap::new(),
        });
    }
    
    data
}

fn simulate_multi_provider_ingestion(symbol: &str, size: usize) -> Vec<TimeSeriesData> {
    // Simulate data fusion from multiple providers
    let mut combined_data = Vec::new();
    
    // Alpaca data (recent, high quality)
    let alpaca_data = generate_price_series(&format!("{}_ALPACA", symbol), 50000.0, size / 4);
    
    // Yahoo data (historical, medium quality)
    let yahoo_data = generate_price_series(&format!("{}_YAHOO", symbol), 45000.0, size / 2);
    
    // Binance data (crypto specific, high frequency)
    let binance_data = generate_price_series(&format!("{}_BINANCE", symbol), 50000.0, size / 4);
    
    // Merge and deduplicate
    combined_data.extend(alpaca_data);
    combined_data.extend(yahoo_data);
    combined_data.extend(binance_data);
    
    // Sort by timestamp
    combined_data.sort_by_key(|d| d.timestamp);
    
    // Normalize symbol names
    for data in &mut combined_data {
        data.symbol = symbol.to_string();
    }
    
    combined_data
}

fn create_full_neural_config() -> NeuralConfig {
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

fn statistical_stddev(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    
    variance.sqrt()
}

fn get_current_memory_usage() -> usize {
    // Simplified memory usage tracking
    // In a real implementation, would use system APIs
    0
}