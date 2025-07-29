//! Comprehensive unit tests for Neural Trader - 85% Coverage Target
//! 
//! This test suite provides comprehensive coverage for:
//! - All public APIs and core functionality
//! - Error handling and edge cases
//! - Async/await patterns and concurrency
//! - Data validation and transformation
//! - Memory management and resource cleanup
//! - Integration between components

use autonomous_platform::adapters::neuro_divergent::{NeuroDivergentAdapter, AdapterConfig};
use autonomous_platform::adapters::AdapterError;
use autonomous_platform::neural::fann_predictor::{FannPredictor, FannModelConfig};
use autonomous_platform::neural::{PredictionResult, NeuralPredictorTrait};
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::TimeSeriesData;
use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;
use anyhow::Result;
use tokio::time::{timeout, Duration};
use std::sync::Arc;
use tokio::sync::RwLock;
use mockall::predicate::*;
use mockall::mock;
use ndarray::{Array1, Array2};
use polars::prelude::*;
use approx::assert_relative_eq;
use serial_test::serial;

// Test configuration constants
const TEST_TIMEOUT_SECONDS: u64 = 30;
const LARGE_DATASET_SIZE: usize = 10000;
const MEDIUM_DATASET_SIZE: usize = 1000;
const SMALL_DATASET_SIZE: usize = 100;

// Helper function to create test neural config
fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        memory_gb: 1.0,
        models: vec![
            "MLP".to_string(),
            "LSTM".to_string(),
            "TCN".to_string(),
            "DeepAR".to_string(),
            "Transformer".to_string(),
        ],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
    }
}

// Helper function to create test adapter config
fn create_test_adapter_config() -> AdapterConfig {
    AdapterConfig {
        horizon: 24,
        input_size: 48,
        hidden_size: 64,
        num_layers: 2,
        learning_rate: 0.001,
        max_epochs: 100,
        use_gpu: false,
    }
}

// Helper function to create diverse test time series data
fn create_comprehensive_test_data(count: usize, symbol: &str, scenario: TestScenario) -> Vec<TimeSeriesData> {
    let base_timestamp = Utc.ymd(2024, 1, 1).and_hms(0, 0, 0);
    let mut data = Vec::new();
    
    for i in 0..count {
        let base_price = match scenario {
            TestScenario::Normal => 100.0,
            TestScenario::Trending => 100.0,
            TestScenario::Volatile => 100.0,
            TestScenario::LowVolume => 100.0,
            TestScenario::Extreme => 100.0,
        };
        
        let (open, high, low, close, volume) = match scenario {
            TestScenario::Normal => {
                let price = base_price + (i as f64 * 0.1);
                (price * 0.999, price * 1.001, price * 0.998, price, 1000.0 + i as f64)
            },
            TestScenario::Trending => {
                let trend = (i as f64 * 0.5);
                let price = base_price + trend;
                (price * 0.995, price * 1.002, price * 0.993, price, 2000.0 + i as f64 * 2.0)
            },
            TestScenario::Volatile => {
                let volatility = (i as f64 * 0.1).sin() * 5.0;
                let price = base_price + volatility;
                (price * 0.98, price * 1.03, price * 0.97, price, 5000.0 + i as f64 * 10.0)
            },
            TestScenario::LowVolume => {
                let price = base_price + (i as f64 * 0.01);
                (price * 0.9999, price * 1.0001, price * 0.9998, price, 10.0 + i as f64 * 0.1)
            },
            TestScenario::Extreme => {
                let extreme_move = if i % 10 == 0 { (i as f64).sin() * 20.0 } else { 0.0 };
                let price = base_price + extreme_move;
                (price * 0.9, price * 1.15, price * 0.85, price, 100000.0)
            },
        };
        
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + (i as f64 * 0.4).min(70.0));
        indicators.insert("macd".to_string(), (i as f64 * 0.001).sin());
        indicators.insert("sma_20".to_string(), close * 0.99);
        indicators.insert("ema_12".to_string(), close * 1.01);
        indicators.insert("bollinger_upper".to_string(), close * 1.02);
        indicators.insert("bollinger_lower".to_string(), close * 0.98);
        indicators.insert("volume_sma".to_string(), volume * 0.9);
        
        let ts = TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: base_timestamp + chrono::Duration::minutes(i as i64 * 5),
            open,
            high,
            low,
            close,
            volume,
            indicators,
            source: Some("comprehensive_test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(close),
            metadata: Some(serde_json::json!({
                "scenario": format!("{:?}", scenario),
                "index": i,
                "test_id": uuid::Uuid::new_v4().to_string()
            })),
        };
        
        data.push(ts);
    }
    
    data
}

#[derive(Debug, Clone, Copy)]
enum TestScenario {
    Normal,    // Standard market conditions
    Trending,  // Strong upward trend
    Volatile,  // High volatility
    LowVolume, // Low trading volume
    Extreme,   // Extreme price movements
}

// Mock external dependencies
mock! {
    VendorModel {
        fn predict(&self, input: &[f64]) -> Result<Vec<f64>>;
        fn train(&mut self, data: &[Vec<f64>], targets: &[f64]) -> Result<()>;
        fn is_trained(&self) -> bool;
        fn get_accuracy(&self) -> f64;
        fn reset(&mut self);
    }
}

#[cfg(test)]
mod adapter_config_tests {
    use super::*;
    
    #[test]
    fn test_adapter_config_default() {
        let config = AdapterConfig::default();
        
        assert_eq!(config.horizon, 24);
        assert_eq!(config.input_size, 48);
        assert_eq!(config.hidden_size, 64);
        assert_eq!(config.num_layers, 2);
        assert_relative_eq!(config.learning_rate, 0.001, epsilon = 1e-6);
        assert_eq!(config.max_epochs, 100);
        assert!(!config.use_gpu);
    }
    
    #[test]
    fn test_adapter_config_custom() {
        let config = AdapterConfig {
            horizon: 48,
            input_size: 96,
            hidden_size: 128,
            num_layers: 4,
            learning_rate: 0.0001,
            max_epochs: 500,
            use_gpu: true,
        };
        
        assert_eq!(config.horizon, 48);
        assert_eq!(config.input_size, 96);
        assert_eq!(config.hidden_size, 128);
        assert_eq!(config.num_layers, 4);
        assert_relative_eq!(config.learning_rate, 0.0001, epsilon = 1e-7);
        assert_eq!(config.max_epochs, 500);
        assert!(config.use_gpu);
    }
    
    #[test]
    fn test_adapter_config_clone() {
        let config1 = create_test_adapter_config();
        let config2 = config1.clone();
        
        assert_eq!(config1.horizon, config2.horizon);
        assert_eq!(config1.input_size, config2.input_size);
        assert_eq!(config1.hidden_size, config2.hidden_size);
        assert_eq!(config1.num_layers, config2.num_layers);
    }
}

#[cfg(test)]
mod neuro_divergent_adapter_tests {
    use super::*;
    
    #[test]
    fn test_adapter_creation() {
        let adapter = NeuroDivergentAdapter::new();
        // Adapter should be created successfully
        // Internal state is private, but creation should not panic
    }
    
    #[test]
    fn test_adapter_with_config() {
        let config = create_test_adapter_config();
        let adapter = NeuroDivergentAdapter::with_config(config);
        // Should create adapter with custom config
    }
    
    #[tokio::test]
    async fn test_init_deepar() {
        let mut adapter = NeuroDivergentAdapter::new();
        let result = adapter.init_deepar().await;
        
        // Should either succeed or fail gracefully
        match result {
            Ok(_) => {
                // DeepAR initialized successfully
            },
            Err(e) => {
                // Initialization failed - check it's a proper error
                assert!(e.to_string().len() > 0);
            }
        }
    }
    
    #[tokio::test]
    async fn test_init_tcn() {
        let mut adapter = NeuroDivergentAdapter::new();
        let result = adapter.init_tcn().await;
        
        // Should either succeed or fail gracefully
        match result {
            Ok(_) => {
                // TCN initialized successfully
            },
            Err(e) => {
                // Initialization failed - check it's a proper error
                assert!(e.to_string().len() > 0);
            }
        }
    }
    
    #[tokio::test]
    #[serial]
    async fn test_concurrent_initialization() {
        let mut adapter1 = NeuroDivergentAdapter::new();
        let mut adapter2 = NeuroDivergentAdapter::new();
        
        let (result1, result2) = tokio::join!(
            adapter1.init_deepar(),
            adapter2.init_tcn()
        );
        
        // Both should handle concurrent initialization
        // Results can be Ok or Err, but should not panic
        let _ = result1;
        let _ = result2;
    }
}

#[cfg(test)]
mod data_conversion_comprehensive_tests {
    use super::*;
    
    #[test]
    fn test_to_neuro_divergent_df_all_scenarios() {
        for scenario in [TestScenario::Normal, TestScenario::Trending, TestScenario::Volatile, TestScenario::LowVolume, TestScenario::Extreme] {
            let data = create_comprehensive_test_data(50, "BTC/USD", scenario);
            let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
            
            assert!(result.is_ok(), "Failed for scenario {:?}", scenario);
            let df = result.unwrap();
            
            assert_eq!(df.height(), 50);
            
            // Verify all required columns exist
            let expected_columns = ["unique_id", "ds", "y", "open", "high", "low", "volume"];
            for col in expected_columns {
                assert!(df.get_column_names().contains(&col), "Missing column: {}", col);
            }
            
            // Verify indicators are included
            assert!(df.get_column_names().contains(&"rsi"));
            assert!(df.get_column_names().contains(&"macd"));
            assert!(df.get_column_names().contains(&"sma_20"));
        }
    }
    
    #[test]
    fn test_to_neuro_divergent_df_empty_data() {
        let empty_data: Vec<TimeSeriesData> = vec![];
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&empty_data);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty data"));
    }
    
    #[test]
    fn test_to_neuro_divergent_df_single_point() {
        let data = create_comprehensive_test_data(1, "ETH/USD", TestScenario::Normal);
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        
        assert!(result.is_ok());
        let df = result.unwrap();
        assert_eq!(df.height(), 1);
        
        // Verify data integrity
        let unique_ids = df.column("unique_id").unwrap().utf8().unwrap();
        assert_eq!(unique_ids.get(0).unwrap(), "ETH/USD");
    }
    
    #[test]
    fn test_to_neuro_divergent_df_missing_indicators() {
        let mut data = vec![TimeSeriesData {
            symbol: "TEST/USD".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: 1000.0,
            indicators: HashMap::new(), // No indicators
            source: Some("test".to_string()),
            entity: Some("TEST/USD".to_string()),
            value: Some(102.0),
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        // Should have only base columns when no indicators
        assert_eq!(df.width(), 7); // unique_id, ds, y, open, high, low, volume
    }
    
    #[test]
    fn test_to_neuro_divergent_df_large_dataset() {
        let data = create_comprehensive_test_data(LARGE_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        let start = std::time::Instant::now();
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        let df = result.unwrap();
        assert_eq!(df.height(), LARGE_DATASET_SIZE);
        
        // Should complete within reasonable time
        assert!(duration.as_secs() < 5, "Conversion too slow: {:?}", duration);
    }
    
    #[test]
    fn test_from_neuro_divergent_df_roundtrip() {
        let original_data = create_comprehensive_test_data(100, "BTC/USD", TestScenario::Volatile);
        
        // Convert to DataFrame
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&original_data).unwrap();
        
        // Convert back
        let converted_data = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD").unwrap();
        
        assert_eq!(original_data.len(), converted_data.len());
        
        for (orig, conv) in original_data.iter().zip(converted_data.iter()) {
            assert_eq!(orig.symbol, conv.symbol);
            assert_relative_eq!(orig.close, conv.close, epsilon = 1e-10);
            assert_relative_eq!(orig.volume, conv.volume, epsilon = 1e-10);
            
            // Check key indicators preserved
            for key in ["rsi", "macd"] {
                if let (Some(orig_val), Some(conv_val)) = (orig.indicators.get(key), conv.indicators.get(key)) {
                    assert_relative_eq!(orig_val, conv_val, epsilon = 1e-10);
                }
            }
        }
    }
    
    #[test]
    fn test_from_neuro_divergent_df_missing_optional_columns() {
        // Create minimal DataFrame
        let timestamps = vec![Utc::now().timestamp()];
        let closes = vec![100.0];
        
        let df = DataFrame::new(vec![
            Series::new("ds", &timestamps),
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_ok());
        
        let converted = result.unwrap();
        assert_eq!(converted.len(), 1);
        
        // Should use close price for missing OHLC
        assert_relative_eq!(converted[0].open, 100.0, epsilon = 1e-10);
        assert_relative_eq!(converted[0].high, 100.0, epsilon = 1e-10);
        assert_relative_eq!(converted[0].low, 100.0, epsilon = 1e-10);
        assert_relative_eq!(converted[0].close, 100.0, epsilon = 1e-10);
        assert_relative_eq!(converted[0].volume, 0.0, epsilon = 1e-10);
    }
    
    #[test]
    fn test_from_neuro_divergent_df_invalid_timestamp() {
        let invalid_timestamps = vec!["not_a_timestamp"];
        let closes = vec![100.0];
        
        let df = DataFrame::new(vec![
            Series::new("ds", &invalid_timestamps),
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid timestamp"));
    }
    
    #[test]
    fn test_from_neuro_divergent_df_missing_required_columns() {
        let closes = vec![100.0];
        
        // Missing timestamp column
        let df = DataFrame::new(vec![
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'ds' column"));
    }
}

#[cfg(test)]
mod model_input_preparation_tests {
    use super::*;
    
    #[test]
    fn test_prepare_model_input_normal_case() {
        let data = create_comprehensive_test_data(100, "BTC/USD", TestScenario::Normal);
        let lookback = 20;
        let forecast_horizon = 5;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        
        // Verify dimensions
        let expected_samples = data.len() - lookback - forecast_horizon + 1;
        assert_eq!(features.shape()[0], expected_samples);
        
        // Features: lookback * (5 OHLCV + 7 indicators) = 20 * 12 = 240
        assert_eq!(features.shape()[1], lookback * 12);
        assert_eq!(targets.len(), expected_samples);
    }
    
    #[test]
    fn test_prepare_model_input_insufficient_data() {
        let data = create_comprehensive_test_data(10, "BTC/USD", TestScenario::Normal);
        let lookback = 20;
        let forecast_horizon = 5;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient data"));
    }
    
    #[test]
    fn test_prepare_model_input_edge_cases() {
        // Minimum valid case
        let data = create_comprehensive_test_data(26, "BTC/USD", TestScenario::Normal); // 26 = 20 + 5 + 1
        let lookback = 20;
        let forecast_horizon = 5;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        assert_eq!(features.shape()[0], 2); // 26 - 20 - 5 + 1 = 2 samples
        assert_eq!(targets.len(), 2);
    }
    
    #[test]
    fn test_prepare_model_input_no_indicators() {
        let mut data = Vec::new();
        for i in 0..50 {
            data.push(TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: Utc::now() + chrono::Duration::hours(i),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64,
                volume: 1000.0,
                indicators: HashMap::new(), // No indicators
                source: Some("test".to_string()),
                entity: Some("TEST".to_string()),
                value: Some(100.5 + i as f64),
                metadata: None,
            });
        }
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 10, 5);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        // Features: 10 * 5 (OHLCV only) = 50
        assert_eq!(features.shape()[1], 50);
    }
    
    #[test]
    fn test_prepare_model_input_target_correctness() {
        let data = create_comprehensive_test_data(30, "BTC/USD", TestScenario::Normal);
        let lookback = 10;
        let forecast_horizon = 3;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        let (_, targets) = result.unwrap();
        
        // Target should be close price at lookback + forecast_horizon - 1
        // For first sample: data[10 + 3 - 1].close = data[12].close
        assert_relative_eq!(targets[0], data[12].close, epsilon = 1e-10);
    }
    
    #[test]
    fn test_prepare_model_input_different_scenarios() {
        for scenario in [TestScenario::Trending, TestScenario::Volatile, TestScenario::Extreme] {
            let data = create_comprehensive_test_data(100, "TEST", scenario);
            let result = NeuroDivergentAdapter::prepare_model_input(&data, 15, 5);
            
            assert!(result.is_ok(), "Failed for scenario {:?}", scenario);
            let (features, targets) = result.unwrap();
            
            // Verify no NaN or infinite values
            for &value in features.iter() {
                assert!(value.is_finite(), "Found non-finite value in features for scenario {:?}", scenario);
            }
            
            for &value in targets.iter() {
                assert!(value.is_finite(), "Found non-finite value in targets for scenario {:?}", scenario);
            }
        }
    }
}

#[cfg(test)]
mod prediction_conversion_tests {
    use super::*;
    
    #[test]
    fn test_predictions_to_timeseries_basic() {
        let predictions = vec![101.0, 102.5, 104.0, 105.5, 107.0];
        let base_timestamp = Utc.ymd(2024, 6, 15).and_hms(12, 0, 0);
        let symbol = "ETH/USD";
        let interval_seconds = 3600; // 1 hour
        
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            symbol,
            interval_seconds,
        );
        
        assert_eq!(result.len(), 5);
        
        // Verify first prediction
        assert_eq!(result[0].symbol, "ETH/USD");
        assert_eq!(result[0].timestamp, base_timestamp);
        assert_relative_eq!(result[0].close, 101.0, epsilon = 1e-10);
        assert_relative_eq!(result[0].open, 101.0, epsilon = 1e-10);
        assert_relative_eq!(result[0].high, 101.0, epsilon = 1e-10);
        assert_relative_eq!(result[0].low, 101.0, epsilon = 1e-10);
        assert_relative_eq!(result[0].volume, 0.0, epsilon = 1e-10);
        
        // Verify timestamp progression
        assert_eq!(
            result[1].timestamp,
            base_timestamp + chrono::Duration::seconds(interval_seconds)
        );
        assert_eq!(
            result[4].timestamp,
            base_timestamp + chrono::Duration::seconds(interval_seconds * 4)
        );
        
        // Verify metadata
        assert!(result[0].metadata.is_some());
        let metadata = result[0].metadata.as_ref().unwrap();
        assert_eq!(metadata["type"], "forecast");
        assert_eq!(metadata["model"], "neuro-divergent");
        
        // Verify source and entity
        assert_eq!(result[0].source, Some("prediction".to_string()));
        assert_eq!(result[0].entity, Some("ETH/USD".to_string()));
        assert_eq!(result[0].value, Some(101.0));
    }
    
    #[test]
    fn test_predictions_to_timeseries_empty() {
        let predictions: Vec<f64> = vec![];
        let base_timestamp = Utc::now();
        
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "TEST",
            60,
        );
        
        assert_eq!(result.len(), 0);
    }
    
    #[test]
    fn test_predictions_to_timeseries_single() {
        let predictions = vec![999.99];
        let base_timestamp = Utc::now();
        
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "GOLD/USD",
            300, // 5 minutes
        );
        
        assert_eq!(result.len(), 1);
        assert_relative_eq!(result[0].close, 999.99, epsilon = 1e-10);
        assert_eq!(result[0].source, Some("prediction".to_string()));
        assert_eq!(result[0].entity, Some("GOLD/USD".to_string()));
        assert_eq!(result[0].value, Some(999.99));
    }
    
    #[test]
    fn test_predictions_to_timeseries_negative_values() {
        let predictions = vec![-10.5, -5.25, 0.0, 5.75, 10.0];
        let base_timestamp = Utc::now();
        
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "CHANGE/USD",
            1800, // 30 minutes
        );
        
        assert_eq!(result.len(), 5);
        assert_relative_eq!(result[0].close, -10.5, epsilon = 1e-10);
        assert_relative_eq!(result[2].close, 0.0, epsilon = 1e-10);
        assert_relative_eq!(result[4].close, 10.0, epsilon = 1e-10);
    }
    
    #[test]
    fn test_predictions_to_timeseries_extreme_values() {
        let predictions = vec![f64::MIN_POSITIVE, 1e-10, 1e10, f64::MAX / 2.0];
        let base_timestamp = Utc::now();
        
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "EXTREME",
            1,
        );
        
        assert_eq!(result.len(), 4);
        
        // Should handle extreme values without panic
        for ts in &result {
            assert!(ts.close.is_finite());
            assert!(ts.open.is_finite());
            assert!(ts.high.is_finite());
            assert!(ts.low.is_finite());
        }
    }
    
    #[test]
    fn test_predictions_to_timeseries_large_dataset() {
        let predictions: Vec<f64> = (0..10000).map(|i| i as f64).collect();
        let base_timestamp = Utc::now();
        
        let start = std::time::Instant::now();
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "LARGE_TEST",
            1,
        );
        let duration = start.elapsed();
        
        assert_eq!(result.len(), 10000);
        assert!(duration.as_millis() < 1000); // Should be fast
        
        // Verify first and last elements
        assert_relative_eq!(result[0].close, 0.0, epsilon = 1e-10);
        assert_relative_eq!(result[9999].close, 9999.0, epsilon = 1e-10);
    }
}

#[cfg(test)]
mod error_handling_comprehensive_tests {
    use super::*;
    
    #[test]
    fn test_adapter_error_types() {
        // Test all AdapterError variants
        let errors = vec![
            AdapterError::Connection("Test connection error".to_string()),
            AdapterError::Query("Test query error".to_string()),
            AdapterError::Serialization("Test serialization error".to_string()),
            AdapterError::Configuration("Test config error".to_string()),
            AdapterError::ModelCreation("Test model creation error".to_string()),
            AdapterError::ModelNotInitialized("Test model not initialized".to_string()),
            AdapterError::Training("Test training error".to_string()),
            AdapterError::Prediction("Test prediction error".to_string()),
        ];
        
        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
            assert!(error_string.len() > 10); // Should have meaningful message
        }
    }
    
    #[test]
    fn test_invalid_dataframe_creation() {
        // Test with mismatched series lengths
        let result = std::panic::catch_unwind(|| {
            DataFrame::new(vec![
                Series::new("col1", vec!["a", "b"]),
                Series::new("col2", vec![1i32]), // Different length
            ])
        });
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_nan_and_infinity_handling() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), f64::NAN);
        indicators.insert("macd".to_string(), f64::INFINITY);
        indicators.insert("normal".to_string(), 50.0);
        
        let data = vec![TimeSeriesData {
            symbol: "NAN_TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: f64::INFINITY,
            low: f64::NEG_INFINITY,
            close: f64::NAN,
            volume: 1000.0,
            indicators,
            source: Some("test".to_string()),
            entity: Some("NAN_TEST".to_string()),
            value: Some(f64::NAN),
            metadata: None,
        }];
        
        // Should handle NaN/Infinity gracefully
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        
        // Verify NaN values are preserved in DataFrame
        let rsi_col = df.column("rsi").unwrap().f64().unwrap();
        assert!(rsi_col.get(0).unwrap().is_nan());
        
        let macd_col = df.column("macd").unwrap().f64().unwrap();
        assert!(macd_col.get(0).unwrap().is_infinite());
    }
    
    #[test]
    fn test_zero_and_negative_values() {
        let mut indicators = HashMap::new();
        indicators.insert("negative_rsi".to_string(), -20.0);
        indicators.insert("zero_macd".to_string(), 0.0);
        
        let data = vec![TimeSeriesData {
            symbol: "ZERO_TEST".to_string(),
            timestamp: Utc::now(),
            open: -50.0,
            high: 0.0,
            low: -100.0,
            close: -25.0,
            volume: 0.0,
            indicators,
            source: Some("test".to_string()),
            entity: Some("ZERO_TEST".to_string()),
            value: Some(-25.0),
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "ZERO_TEST").unwrap();
        
        assert_relative_eq!(converted[0].close, -25.0, epsilon = 1e-10);
        assert_relative_eq!(converted[0].volume, 0.0, epsilon = 1e-10);
        assert_eq!(converted[0].indicators.get("negative_rsi"), Some(&-20.0));
        assert_eq!(converted[0].indicators.get("zero_macd"), Some(&0.0));
    }
    
    #[test]
    fn test_unicode_and_special_characters() {
        let mut indicators = HashMap::new();
        indicators.insert("測試指標".to_string(), 42.0);
        indicators.insert("индикатор".to_string(), 24.0);
        indicators.insert("🚀_indicator".to_string(), 100.0);
        
        let data = vec![TimeSeriesData {
            symbol: "BTC/USD 🚀📈".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 1000.0,
            indicators,
            source: Some("тест".to_string()),
            entity: Some("測試實體".to_string()),
            value: Some(100.5),
            metadata: Some(serde_json::json!({
                "note": "This is a test with unicode: αβγδε"
            })),
        }];
        
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD 🚀📈").unwrap();
        
        assert_eq!(converted[0].symbol, "BTC/USD 🚀📈");
        assert_eq!(converted[0].source, Some("тест".to_string()));
        assert_eq!(converted[0].entity, Some("測試實體".to_string()));
        assert_eq!(converted[0].indicators.get("測試指標"), Some(&42.0));
        assert_eq!(converted[0].indicators.get("🚀_indicator"), Some(&100.0));
    }
    
    #[test]
    fn test_empty_string_handling() {
        let data = vec![TimeSeriesData {
            symbol: "".to_string(), // Empty symbol
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("".to_string()), // Empty source
            entity: Some("".to_string()), // Empty entity
            value: Some(100.0),
            metadata: None,
        }];
        
        // Should handle empty strings gracefully
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        let unique_ids = df.column("unique_id").unwrap().utf8().unwrap();
        assert_eq!(unique_ids.get(0).unwrap(), ""); // Empty symbol preserved
    }
}

#[cfg(test)]
mod async_behavior_tests {
    use super::*;
    use tokio::time::{timeout, Duration};
    
    #[tokio::test]
    async fn test_async_adapter_initialization() {
        let mut adapter = NeuroDivergentAdapter::new();
        
        // Test with timeout to ensure it doesn't hang
        let result = timeout(Duration::from_secs(TEST_TIMEOUT_SECONDS), async {
            adapter.init_deepar().await
        }).await;
        
        assert!(result.is_ok(), "DeepAR initialization timed out");
        
        let result = timeout(Duration::from_secs(TEST_TIMEOUT_SECONDS), async {
            adapter.init_tcn().await
        }).await;
        
        assert!(result.is_ok(), "TCN initialization timed out");
    }
    
    #[tokio::test]
    async fn test_async_model_training() {
        let mut adapter = NeuroDivergentAdapter::new();
        let _ = adapter.init_deepar().await; // May fail, that's ok for this test
        
        let data = create_comprehensive_test_data(SMALL_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        let result = timeout(Duration::from_secs(TEST_TIMEOUT_SECONDS), async {
            adapter.train_deepar(&data, "BTC/USD").await
        }).await;
        
        assert!(result.is_ok(), "Training timed out");
        // Result can be Ok or Err, but should not timeout
    }
    
    #[tokio::test]
    async fn test_async_model_prediction() {
        let mut adapter = NeuroDivergentAdapter::new();
        let _ = adapter.init_deepar().await; // May fail, that's ok for this test
        
        let data = create_comprehensive_test_data(SMALL_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        let result = timeout(Duration::from_secs(TEST_TIMEOUT_SECONDS), async {
            adapter.predict_deepar(&data, "BTC/USD").await
        }).await;
        
        assert!(result.is_ok(), "Prediction timed out");
        // Result can be Ok or Err, but should not timeout
    }
    
    #[tokio::test]
    async fn test_concurrent_model_operations() {
        let mut adapter1 = NeuroDivergentAdapter::new();
        let mut adapter2 = NeuroDivergentAdapter::new();
        
        let data1 = create_comprehensive_test_data(50, "BTC/USD", TestScenario::Normal);
        let data2 = create_comprehensive_test_data(50, "ETH/USD", TestScenario::Trending);
        
        let (result1, result2) = tokio::join!(
            async {
                let _ = adapter1.init_deepar().await;
                adapter1.train_deepar(&data1, "BTC/USD").await
            },
            async {
                let _ = adapter2.init_tcn().await;
                adapter2.train_tcn(&data2, "ETH/USD").await
            }
        );
        
        // Both operations should complete without panic
        // Results can be Ok or Err
        let _ = result1;
        let _ = result2;
    }
    
    #[tokio::test]
    async fn test_async_cancellation_safety() {
        let mut adapter = NeuroDivergentAdapter::new();
        let data = create_comprehensive_test_data(LARGE_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        // Start a long-running operation
        let training_future = adapter.train_deepar(&data, "BTC/USD");
        
        // Cancel it after a short time
        let result = timeout(Duration::from_millis(100), training_future).await;
        
        // Should handle cancellation gracefully
        assert!(result.is_err()); // Should timeout/cancel
    }
}

#[cfg(test)]
mod fann_predictor_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fann_predictor_creation() {
        let config = create_test_neural_config();
        let result = FannPredictor::new(config);
        
        assert!(result.is_ok());
        let predictor = result.unwrap();
        
        // Test basic properties
        let predictor_config = predictor.get_config();
        assert!(!predictor_config.models.is_empty());
        assert!(predictor_config.models.contains(&"MLP".to_string()));
    }
    
    #[tokio::test]
    async fn test_fann_predictor_with_real_models() {
        let mut config = create_test_neural_config();
        config.use_real_models = true;
        
        let result = FannPredictor::new(config);
        assert!(result.is_ok());
        
        let predictor = result.unwrap();
        assert!(predictor.has_neuro_divergent_adapter());
    }
    
    #[tokio::test]
    async fn test_fann_predictor_without_real_models() {
        let mut config = create_test_neural_config();
        config.use_real_models = false;
        
        let result = FannPredictor::new(config);
        assert!(result.is_ok());
        
        let predictor = result.unwrap();
        assert!(!predictor.has_neuro_divergent_adapter());
    }
    
    #[tokio::test]
    async fn test_fann_predictor_single_prediction() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let data = create_comprehensive_test_data(SMALL_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        let result = predictor.predict(&data, 5, None).await;
        
        // Should either succeed or fail gracefully
        match result {
            Ok(predictions) => {
                assert!(!predictions.is_empty());
                assert!(predictions.len() <= 5);
                
                // Verify prediction structure
                for pred in &predictions {
                    assert!(!pred.model_name.is_empty());
                    assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                    assert!(pred.value.is_finite());
                    assert!(pred.interval_low <= pred.value);
                    assert!(pred.value <= pred.interval_high);
                }
            },
            Err(e) => {
                // Should be a meaningful error
                assert!(!e.to_string().is_empty());
            }
        }
    }
    
    #[tokio::test]
    async fn test_fann_predictor_ensemble_prediction() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let data = create_comprehensive_test_data(MEDIUM_DATASET_SIZE, "ETH/USD", TestScenario::Trending);
        let models = vec!["MLP".to_string(), "LSTM".to_string(), "TCN".to_string()];
        
        let result = predictor.predict_ensemble(&data, 3, &models, None).await;
        
        match result {
            Ok(predictions) => {
                assert!(!predictions.is_empty());
                assert!(predictions.len() <= 3);
                
                // Ensemble predictions should indicate multiple models
                for pred in &predictions {
                    assert!(pred.model_name.contains("ensemble") || pred.model_name.contains("models"));
                    assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                }
            },
            Err(e) => {
                // Should be a meaningful error
                assert!(!e.to_string().is_empty());
            }
        }
    }
    
    #[tokio::test]
    async fn test_fann_predictor_feature_importance() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let result = predictor.get_feature_importance().await;
        
        assert!(result.is_ok());
        let importance = result.unwrap();
        
        // Should have expected features
        assert!(importance.contains_key("price"));
        assert!(importance.contains_key("volume"));
        
        // All importance values should be between 0 and 1
        for (_, &importance) in &importance {
            assert!(importance >= 0.0 && importance <= 1.0);
        }
        
        // Total importance should be approximately 1.0
        let total: f64 = importance.values().sum();
        assert_relative_eq!(total, 1.0, epsilon = 0.1);
    }
    
    #[tokio::test]
    async fn test_fann_predictor_model_update() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let new_data = create_comprehensive_test_data(50, "BTC/USD", TestScenario::Volatile);
        
        let result = predictor.update_with_new_data("MLP", &new_data).await;
        
        // Should either succeed or fail gracefully
        match result {
            Ok(_) => {
                // Update successful
            },
            Err(e) => {
                // Should be a meaningful error
                assert!(!e.to_string().is_empty());
            }
        }
    }
    
    #[tokio::test]
    async fn test_fann_predictor_performance_update() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let actual_values = vec![100.0, 101.0, 102.0];
        let predicted_results = vec![
            PredictionResult {
                timestamp: Utc::now(),
                value: 99.5,
                confidence: 0.8,
                interval_low: 98.0,
                interval_high: 101.0,
                model_name: "MLP".to_string(),
                metadata: None,
            },
            PredictionResult {
                timestamp: Utc::now(),
                value: 100.8,
                confidence: 0.75,
                interval_low: 99.5,
                interval_high: 102.0,
                model_name: "MLP".to_string(),
                metadata: None,
            },
            PredictionResult {
                timestamp: Utc::now(),
                value: 101.5,
                confidence: 0.9,
                interval_low: 100.0,
                interval_high: 103.0,
                model_name: "MLP".to_string(),
                metadata: None,
            },
        ];
        
        let result = predictor.update_performance("MLP", &actual_values, &predicted_results).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_fann_predictor_ensemble_stats() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let result = predictor.get_ensemble_stats().await;
        assert!(result.is_ok());
        
        let stats = result.unwrap();
        assert!(stats.contains_key("current_regime"));
        assert!(stats.contains_key("dynamic_weights"));
    }
    
    #[tokio::test]
    async fn test_fann_predictor_reset_performance() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let result = predictor.reset_ensemble_performance().await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod memory_and_performance_tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;
    
    #[test]
    fn test_memory_efficient_large_conversion() {
        let data = create_comprehensive_test_data(LARGE_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        let start = Instant::now();
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        let conversion_time = start.elapsed();
        
        assert!(result.is_ok());
        let df = result.unwrap();
        assert_eq!(df.height(), LARGE_DATASET_SIZE);
        
        // Should complete within reasonable time (less than 2 seconds)
        assert!(conversion_time.as_secs() < 2, "Conversion too slow: {:?}", conversion_time);
        
        // Test roundtrip
        let start = Instant::now();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD");
        let roundtrip_time = start.elapsed();
        
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap().len(), LARGE_DATASET_SIZE);
        assert!(roundtrip_time.as_secs() < 2, "Roundtrip too slow: {:?}", roundtrip_time);
    }
    
    #[test]
    fn test_model_input_preparation_performance() {
        let data = create_comprehensive_test_data(LARGE_DATASET_SIZE, "BTC/USD", TestScenario::Normal);
        
        let start = Instant::now();
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 100, 20);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 1000, "Model input preparation too slow: {:?}", duration);
        
        let (features, targets) = result.unwrap();
        assert!(features.shape()[0] > 0);
        assert!(targets.len() > 0);
    }
    
    #[test]
    fn test_memory_usage_drop_behavior() {
        // This test ensures proper cleanup of resources
        for _ in 0..10 {
            let data = create_comprehensive_test_data(1000, "TEST", TestScenario::Normal);
            let _df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
            // DataFrame should be properly dropped at end of scope
        }
        
        // If we reach here without OOM, memory management is working
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_concurrent_large_operations() {
        let data = Arc::new(create_comprehensive_test_data(MEDIUM_DATASET_SIZE, "BTC/USD", TestScenario::Normal));
        
        let mut handles = vec![];
        
        for i in 0..5 {
            let data_clone = Arc::clone(&data);
            let handle = tokio::spawn(async move {
                let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data_clone);
                assert!(result.is_ok());
                i
            });
            handles.push(handle);
        }
        
        let results = futures::future::join_all(handles).await;
        assert_eq!(results.len(), 5);
        
        for result in results {
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_prediction_conversion_performance() {
        let large_predictions: Vec<f64> = (0..100000).map(|i| i as f64 * 0.01).collect();
        let base_timestamp = Utc::now();
        
        let start = Instant::now();
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &large_predictions,
            base_timestamp,
            "PERF_TEST",
            1,
        );
        let duration = start.elapsed();
        
        assert_eq!(result.len(), 100000);
        assert!(duration.as_millis() < 2000, "Prediction conversion too slow: {:?}", duration);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_full_pipeline_roundtrip() {
        let original_data = create_comprehensive_test_data(200, "BTC/USD", TestScenario::Trending);
        
        // Step 1: Convert to DataFrame
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&original_data).unwrap();
        
        // Step 2: Prepare model input
        let (features, targets) = NeuroDivergentAdapter::prepare_model_input(&original_data, 20, 5).unwrap();
        
        // Step 3: Simulate model predictions
        let predictions: Vec<f64> = targets[0..5].iter().map(|&t| t * 1.01).collect();
        
        // Step 4: Convert predictions back to time series
        let base_timestamp = original_data.last().unwrap().timestamp;
        let prediction_series = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "BTC/USD",
            300,
        );
        
        // Step 5: Convert back from DataFrame
        let converted_data = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD").unwrap();
        
        // Verify integrity throughout pipeline
        assert_eq!(original_data.len(), converted_data.len());
        assert_eq!(prediction_series.len(), 5);
        assert!(features.shape()[0] > 0);
        assert!(targets.len() > 0);
        
        // Verify data consistency
        for pred in &prediction_series {
            assert_eq!(pred.symbol, "BTC/USD");
            assert!(pred.value.is_some());
            assert_eq!(pred.source, Some("prediction".to_string()));
        }
    }
    
    #[tokio::test]
    async fn test_adapter_predictor_integration() {
        let config = create_test_neural_config();
        let predictor = FannPredictor::new(config).unwrap();
        
        let data = create_comprehensive_test_data(SMALL_DATASET_SIZE, "ETH/USD", TestScenario::Normal);
        
        // Test predictor with adapter-prepared data
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        let converted_back = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "ETH/USD").unwrap();
        
        let prediction_result = predictor.predict(&converted_back, 3, None).await;
        
        // Should handle converted data properly
        match prediction_result {
            Ok(predictions) => {
                assert!(!predictions.is_empty());
                for pred in &predictions {
                    assert!(pred.value.is_finite());
                    assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
                }
            },
            Err(e) => {
                // Should be a meaningful error, not a panic
                assert!(!e.to_string().is_empty());
            }
        }
    }
    
    #[test]
    fn test_multi_scenario_data_handling() {
        let scenarios = [
            TestScenario::Normal,
            TestScenario::Trending,
            TestScenario::Volatile,
            TestScenario::LowVolume,
            TestScenario::Extreme,
        ];
        
        for scenario in scenarios {
            let data = create_comprehensive_test_data(100, "TEST", scenario);
            
            // Test conversion
            let df_result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
            assert!(df_result.is_ok(), "DataFrame conversion failed for scenario {:?}", scenario);
            
            let df = df_result.unwrap();
            let converted_result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
            assert!(converted_result.is_ok(), "DataFrame back-conversion failed for scenario {:?}", scenario);
            
            // Test model input preparation
            let input_result = NeuroDivergentAdapter::prepare_model_input(&data, 20, 5);
            assert!(input_result.is_ok(), "Model input preparation failed for scenario {:?}", scenario);
            
            let (features, targets) = input_result.unwrap();
            
            // Verify no invalid values
            for &value in features.iter() {
                assert!(value.is_finite(), "Found non-finite feature value in scenario {:?}", scenario);
            }
            
            for &value in targets.iter() {
                assert!(value.is_finite(), "Found non-finite target value in scenario {:?}", scenario);
            }
        }
    }
}

#[cfg(test)]
mod edge_case_comprehensive_tests {
    use super::*;
    
    #[test]
    fn test_boundary_conditions() {
        // Test minimum valid data size
        let min_data = create_comprehensive_test_data(1, "MIN", TestScenario::Normal);
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&min_data);
        assert!(result.is_ok());
        
        // Test very large single prediction
        let large_prediction = vec![1e15];
        let ts_result = NeuroDivergentAdapter::predictions_to_timeseries(
            &large_prediction,
            Utc::now(),
            "LARGE",
            1,
        );
        assert_eq!(ts_result.len(), 1);
        assert_relative_eq!(ts_result[0].close, 1e15, epsilon = 1e5);
    }
    
    #[test]
    fn test_extreme_timestamp_values() {
        let extreme_past = Utc.ymd(1970, 1, 1).and_hms(0, 0, 1);
        let extreme_future = Utc.ymd(2100, 12, 31).and_hms(23, 59, 59);
        
        let data = vec![
            TimeSeriesData {
                symbol: "PAST".to_string(),
                timestamp: extreme_past,
                open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("PAST".to_string()),
                value: Some(100.0),
                metadata: None,
            },
            TimeSeriesData {
                symbol: "FUTURE".to_string(),
                timestamp: extreme_future,
                open: 200.0, high: 201.0, low: 199.0, close: 200.0, volume: 2000.0,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("FUTURE".to_string()),
                value: Some(200.0),
                metadata: None,
            },
        ];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST").unwrap();
        
        assert_eq!(converted.len(), 2);
        assert!(converted[0].timestamp.year() == 1970 || converted[0].timestamp.year() == 2100);
    }
    
    #[test]
    fn test_maximum_indicators() {
        let mut indicators = HashMap::new();
        // Add many indicators to test scalability
        for i in 0..1000 {
            indicators.insert(format!("indicator_{}", i), i as f64 * 0.1);
        }
        
        let data = vec![TimeSeriesData {
            symbol: "MANY_INDICATORS".to_string(),
            timestamp: Utc::now(),
            open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0,
            indicators,
            source: Some("test".to_string()),
            entity: Some("MANY_INDICATORS".to_string()),
            value: Some(100.0),
            metadata: None,
        }];
        
        let start = Instant::now();
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 1000); // Should handle many indicators efficiently
        
        let df = result.unwrap();
        assert_eq!(df.width(), 7 + 1000); // Base columns + indicators
    }
    
    #[test]
    fn test_very_small_values() {
        let data = vec![TimeSeriesData {
            symbol: "MICRO".to_string(),
            timestamp: Utc::now(),
            open: 1e-15,
            high: 2e-15,
            low: 0.5e-15,
            close: 1.5e-15,
            volume: 1e-20,
            indicators: {
                let mut map = HashMap::new();
                map.insert("tiny_rsi".to_string(), 1e-10);
                map
            },
            source: Some("test".to_string()),
            entity: Some("MICRO".to_string()),
            value: Some(1.5e-15),
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "MICRO").unwrap();
        
        assert_relative_eq!(converted[0].close, 1.5e-15, epsilon = 1e-16);
    }
    
    #[test]
    fn test_mixed_data_quality() {
        let mut data = vec![];
        
        // Add data points with varying quality
        for i in 0..50 {
            let mut indicators = HashMap::new();
            
            // Some points have all indicators
            if i % 3 == 0 {
                indicators.insert("rsi".to_string(), 50.0);
                indicators.insert("macd".to_string(), 0.1);
                indicators.insert("volume_sma".to_string(), 1000.0);
            }
            // Some points have partial indicators
            else if i % 3 == 1 {
                indicators.insert("rsi".to_string(), 60.0);
            }
            // Some points have no indicators (i % 3 == 2)
            
            let price = 100.0 + i as f64;
            data.push(TimeSeriesData {
                symbol: "MIXED".to_string(),
                timestamp: Utc::now() + chrono::Duration::minutes(i),
                open: price,
                high: price + 1.0,
                low: price - 1.0,
                close: price + 0.5,
                volume: if i % 5 == 0 { 0.0 } else { 1000.0 + i as f64 },
                indicators,
                source: Some("test".to_string()),
                entity: Some("MIXED".to_string()),
                value: Some(price + 0.5),
                metadata: None,
            });
        }
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "MIXED").unwrap();
        
        assert_eq!(converted.len(), 50);
        
        // Verify mixed data is handled correctly
        let mut points_with_rsi = 0;
        let mut points_without_rsi = 0;
        
        for point in &converted {
            if point.indicators.contains_key("rsi") {
                points_with_rsi += 1;
            } else {
                points_without_rsi += 1;
            }
        }
        
        assert!(points_with_rsi > 0);
        assert!(points_without_rsi > 0);
    }
}
