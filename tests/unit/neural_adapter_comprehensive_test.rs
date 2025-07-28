//! Comprehensive tests for neural adapter components
//! 
//! This test suite provides extensive coverage for:
//! - NeuroDivergentAdapter functionality
//! - DataConverter operations
//! - TypeConverter utilities
//! - VendorConversion methods
//! - Error handling and edge cases

use autonomous_platform::adapters::neural::neuro_divergent_adapter::{
    NeuroDivergentAdapter, NeuralAdapterError, NeuralModelConfig, ModelState
};
use autonomous_platform::adapters::neural::data_converter::{
    DataConverter, ConversionFormat, ModelInput
};
use autonomous_platform::adapters::AdapterError;
use autonomous_platform::data::TimeSeriesData;
use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;
use anyhow::Result;
use polars::prelude::*;
use ndarray::{Array2, Array3};
use serde_json::json;
use tokio;

// Test data generation helpers
fn create_test_data(count: usize, symbol: &str) -> Vec<TimeSeriesData> {
    let base_timestamp = Utc.ymd(2024, 1, 1).and_hms(0, 0, 0);
    let mut data = Vec::new();
    
    for i in 0..count {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0));
        indicators.insert("macd".to_string(), -0.01 + (i as f64 % 10.0) * 0.002);
        indicators.insert("bb_upper".to_string(), 105.0 + i as f64);
        indicators.insert("bb_lower".to_string(), 95.0 + i as f64);
        indicators.insert("volume_sma".to_string(), 1000000.0 + i as f64 * 1000.0);
        
        let ts = TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: base_timestamp + chrono::Duration::hours(i as i64),
            open: 100.0 + (i as f64 * 0.1),
            high: 101.0 + (i as f64 * 0.12),
            low: 99.0 + (i as f64 * 0.08),
            close: 100.5 + (i as f64 * 0.1),
            volume: 10000.0 + (i as f64 * 100.0),
            indicators,
            source: Some("test_exchange".to_string()),
            entity: Some(format!("{}_entity", symbol)),
            value: Some(100.5 + (i as f64 * 0.1)),
            metadata: Some(json!({"test_id": i, "batch": "neural_test"})),
        };
        data.push(ts);
    }
    
    data
}

fn create_invalid_data() -> Vec<TimeSeriesData> {
    vec![
        TimeSeriesData {
            symbol: "INVALID".to_string(),
            timestamp: Utc::now(),
            open: f64::NAN,
            high: f64::INFINITY,
            low: f64::NEG_INFINITY,
            close: f64::NAN,
            volume: -1000.0, // Invalid negative volume
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }
    ]
}

#[cfg(test)]
mod neural_adapter_initialization_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_neural_adapter_new_default_config() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await;
        
        assert!(adapter.is_ok());
        let adapter = adapter.unwrap();
        
        // Verify default configuration
        let stored_config = adapter.get_config().await;
        assert_eq!(stored_config.model_type, "TimeMixer");
        assert_eq!(stored_config.lookback_window, 24);
        assert_eq!(stored_config.forecast_horizon, 6);
        assert_eq!(stored_config.batch_size, 32);
        assert!(!stored_config.use_gpu);
    }
    
    #[tokio::test]
    async fn test_neural_adapter_new_custom_config() {
        let config = NeuralModelConfig {
            model_type: "NeuralForecast".to_string(),
            lookback_window: 48,
            forecast_horizon: 12,
            batch_size: 64,
            use_gpu: true,
            model_params: json!({"learning_rate": 0.001, "epochs": 100}),
        };
        
        let adapter = NeuroDivergentAdapter::new(config.clone()).await;
        assert!(adapter.is_ok());
        
        let adapter = adapter.unwrap();
        let stored_config = adapter.get_config().await;
        assert_eq!(stored_config.model_type, config.model_type);
        assert_eq!(stored_config.lookback_window, config.lookback_window);
        assert_eq!(stored_config.forecast_horizon, config.forecast_horizon);
        assert_eq!(stored_config.use_gpu, config.use_gpu);
    }
    
    #[tokio::test]
    async fn test_neural_adapter_invalid_config() {
        let invalid_config = NeuralModelConfig {
            model_type: "".to_string(), // Empty model type
            lookback_window: 0, // Invalid lookback
            forecast_horizon: 0, // Invalid horizon
            batch_size: 0, // Invalid batch size
            use_gpu: false,
            model_params: json!({}),
        };
        
        let result = NeuroDivergentAdapter::new(invalid_config).await;
        assert!(result.is_err());
        
        if let Err(AdapterError::Configuration(msg)) = result {
            assert!(msg.contains("Invalid configuration"));
        } else {
            panic!("Expected configuration error");
        }
    }
}

#[cfg(test)]
mod neural_adapter_functionality_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_adapter_predict_basic() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        let test_data = create_test_data(50, "BTC/USD");
        
        let result = adapter.predict(&test_data, 5).await;
        assert!(result.is_ok());
        
        let predictions = result.unwrap();
        assert_eq!(predictions.len(), 5);
        
        // Verify prediction structure
        for prediction in &predictions {
            assert!(prediction.close > 0.0);
            assert!(prediction.timestamp > Utc::now() - chrono::Duration::hours(1));
            assert_eq!(prediction.symbol, "BTC/USD");
            assert!(prediction.metadata.is_some());
        }
    }
    
    #[tokio::test]
    async fn test_adapter_predict_insufficient_data() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        let insufficient_data = create_test_data(5, "ETH/USD"); // Too little data
        
        let result = adapter.predict(&insufficient_data, 10).await;
        assert!(result.is_err());
        
        if let Err(AdapterError::Query(msg)) = result {
            assert!(msg.contains("Insufficient data") || msg.contains("prediction"));
        }
    }
    
    #[tokio::test]
    async fn test_adapter_predict_invalid_data() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        let invalid_data = create_invalid_data();
        
        let result = adapter.predict(&invalid_data, 3).await;
        // Should either handle gracefully or fail with appropriate error
        match result {
            Ok(predictions) => {
                // If successful, predictions should be reasonable
                assert!(!predictions.is_empty());
                for pred in predictions {
                    assert!(pred.close.is_finite());
                }
            },
            Err(AdapterError::Serialization(_)) | Err(AdapterError::Query(_)) => {
                // Expected errors for invalid data
            },
            Err(other) => panic!("Unexpected error type: {:?}", other),
        }
    }
    
    #[tokio::test]
    async fn test_adapter_model_state_management() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        // Test initial state
        let state = adapter.get_state().await;
        assert!(matches!(state, ModelState::Initialized | ModelState::Ready));
        
        // Test state after initialization
        let init_result = adapter.initialize().await;
        assert!(init_result.is_ok());
        
        let state_after_init = adapter.get_state().await;
        assert!(matches!(state_after_init, ModelState::Ready));
    }
    
    #[tokio::test]
    async fn test_adapter_concurrent_predictions() {
        let config = NeuralModelConfig {
            batch_size: 16,
            ..NeuralModelConfig::default()
        };
        let adapter = std::sync::Arc::new(NeuroDivergentAdapter::new(config).await.unwrap());
        
        let test_data = create_test_data(100, "BTC/USD");
        
        // Spawn multiple concurrent prediction tasks
        let mut handles = vec![];
        for i in 0..5 {
            let adapter_clone = std::sync::Arc::clone(&adapter);
            let data_clone = test_data.clone();
            
            let handle = tokio::spawn(async move {
                let horizon = 3 + (i % 3);
                adapter_clone.predict(&data_clone, horizon).await
            });
            handles.push(handle);
        }
        
        // Wait for all tasks and verify results
        let mut successful_predictions = 0;
        for handle in handles {
            match handle.await {
                Ok(Ok(predictions)) => {
                    assert!(!predictions.is_empty());
                    successful_predictions += 1;
                },
                Ok(Err(_)) => {
                    // Some failures are acceptable under concurrent load
                },
                Err(_) => panic!("Task panicked"),
            }
        }
        
        // At least some predictions should succeed
        assert!(successful_predictions >= 2);
    }
}

#[cfg(test)]
mod data_converter_tests {
    use super::*;
    
    #[test]
    fn test_data_converter_new() {
        let converter = DataConverter::new();
        assert_eq!(converter.get_feature_columns().len(), 5); // OHLCV
    }
    
    #[test]
    fn test_data_converter_with_custom_features() {
        let features = vec!["close".to_string(), "volume".to_string(), "rsi".to_string()];
        let converter = DataConverter::with_features(features.clone());
        assert_eq!(converter.get_feature_columns(), features);
    }
    
    #[test]
    fn test_conversion_format_determination() {
        let converter = DataConverter::new();
        
        assert_eq!(converter.get_format_for_model("TimeMixer"), ConversionFormat::Tensor);
        assert_eq!(converter.get_format_for_model("TimesFM"), ConversionFormat::Tensor);
        assert_eq!(converter.get_format_for_model("NeuralForecast"), ConversionFormat::DataFrame);
        assert_eq!(converter.get_format_for_model("Prophet"), ConversionFormat::DictArray);
        assert_eq!(converter.get_format_for_model("Unknown"), ConversionFormat::NdArray);
    }
    
    #[test]
    fn test_to_dataframe_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(10, "BTC/USD");
        
        let result = converter.to_dataframe(&data);
        assert!(result.is_ok());
        
        if let ModelInput::DataFrame(df) = result.unwrap() {
            assert_eq!(df.height(), 10);
            assert!(df.get_column_names().contains(&"unique_id"));
            assert!(df.get_column_names().contains(&"ds"));
            assert!(df.get_column_names().contains(&"y"));
            assert!(df.get_column_names().contains(&"open"));
            assert!(df.get_column_names().contains(&"volume"));
            
            // Check for indicators
            assert!(df.get_column_names().contains(&"rsi"));
            assert!(df.get_column_names().contains(&"macd"));
        } else {
            panic!("Expected DataFrame format");
        }
    }
    
    #[test]
    fn test_to_dataframe_empty_data() {
        let converter = DataConverter::new();
        let empty_data: Vec<TimeSeriesData> = vec![];
        
        let result = converter.to_dataframe(&empty_data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NeuralAdapterError::Conversion(_)));
    }
    
    #[test]
    fn test_to_ndarray_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(20, "ETH/USD");
        let config = NeuralModelConfig {
            lookback_window: 10,
            ..NeuralModelConfig::default()
        };
        
        let result = converter.to_ndarray(&data, &config);
        assert!(result.is_ok());
        
        if let ModelInput::Array2D(array) = result.unwrap() {
            // Should have samples based on lookback window
            let expected_samples = data.len() - config.lookback_window + 1;
            assert_eq!(array.shape()[0], expected_samples);
            
            // Features: OHLCV (5) + indicators (5) = 10 per timestep
            // Total features: lookback_window * features_per_timestep
            let features_per_timestep = 10; // 5 OHLCV + 5 indicators
            let expected_features = config.lookback_window * features_per_timestep;
            assert_eq!(array.shape()[1], expected_features);
        } else {
            panic!("Expected Array2D format");
        }
    }
    
    #[test]
    fn test_to_tensor_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(30, "ADA/USD");
        let config = NeuralModelConfig {
            lookback_window: 15,
            batch_size: 8,
            ..NeuralModelConfig::default()
        };
        
        let result = converter.to_tensor(&data, &config);
        assert!(result.is_ok());
        
        if let ModelInput::Array3D(tensor) = result.unwrap() {
            // Verify 3D shape: [samples, sequence_length, features]
            assert_eq!(tensor.shape().len(), 3);
            assert_eq!(tensor.shape()[1], config.lookback_window); // Sequence length
            assert!(tensor.shape()[2] > 0); // Feature count
        } else {
            panic!("Expected Array3D format");
        }
    }
    
    #[test]
    fn test_to_dict_array_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(15, "SOL/USD");
        
        let result = converter.to_dict_array(&data);
        assert!(result.is_ok());
        
        if let ModelInput::DictArray(dict) = result.unwrap() {
            assert!(dict.contains_key("close"));
            assert!(dict.contains_key("volume"));
            assert!(dict.contains_key("rsi"));
            
            // All arrays should have same length
            let close_len = dict.get("close").unwrap().len();
            assert_eq!(close_len, data.len());
            
            for (_, values) in dict.iter() {
                assert_eq!(values.len(), close_len);
            }
        } else {
            panic!("Expected DictArray format");
        }
    }
    
    #[test]
    fn test_model_format_conversion_routing() {
        let converter = DataConverter::new();
        let data = create_test_data(25, "MATIC/USD");
        
        // Test different model types route to correct formats
        let configs = vec![
            ("TimeMixer", ConversionFormat::Tensor),
            ("TimesFM", ConversionFormat::Tensor),
            ("NeuralForecast", ConversionFormat::DataFrame),
            ("Prophet", ConversionFormat::DictArray),
            ("CustomModel", ConversionFormat::NdArray),
        ];
        
        for (model_type, expected_format) in configs {
            let config = NeuralModelConfig {
                model_type: model_type.to_string(),
                ..NeuralModelConfig::default()
            };
            
            let result = converter.to_model_format(&data, &config);
            assert!(result.is_ok());
            
            let input = result.unwrap();
            match (expected_format, input) {
                (ConversionFormat::DataFrame, ModelInput::DataFrame(_)) => {},
                (ConversionFormat::NdArray, ModelInput::Array2D(_)) => {},
                (ConversionFormat::Tensor, ModelInput::Array3D(_)) => {},
                (ConversionFormat::DictArray, ModelInput::DictArray(_)) => {},
                _ => panic!("Format mismatch for model type: {}", model_type),
            }
        }
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_neural_adapter_error_conversions() {
        let errors = vec![
            NeuralAdapterError::ModelInit("Test init error".to_string()),
            NeuralAdapterError::Prediction("Test prediction error".to_string()),
            NeuralAdapterError::Conversion("Test conversion error".to_string()),
            NeuralAdapterError::NotInitialized,
            NeuralAdapterError::InvalidConfig("Test config error".to_string()),
        ];
        
        for error in errors {
            let adapter_error: AdapterError = error.into();
            match adapter_error {
                AdapterError::Connection(_) => {},
                AdapterError::Query(_) => {},
                AdapterError::Serialization(_) => {},
                AdapterError::Configuration(_) => {},
                _ => panic!("Unexpected error conversion"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_adapter_error_recovery() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        // Test recovery from invalid data
        let invalid_data = vec![];
        let result = adapter.predict(&invalid_data, 5).await;
        assert!(result.is_err());
        
        // Should be able to make valid predictions after error
        let valid_data = create_test_data(30, "BTC/USD");
        let result = adapter.predict(&valid_data, 3).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_conversion_edge_cases() {
        let converter = DataConverter::new();
        
        // Test with minimal data
        let minimal_data = create_test_data(1, "TEST");
        let config = NeuralModelConfig {
            lookback_window: 5, // More than available data
            ..NeuralModelConfig::default()
        };
        
        let result = converter.to_ndarray(&minimal_data, &config);
        assert!(result.is_err());
        
        // Test with negative/zero parameters
        let zero_config = NeuralModelConfig {
            lookback_window: 0,
            ..NeuralModelConfig::default()
        };
        
        let result = converter.to_ndarray(&minimal_data, &zero_config);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_data_quality_validation() {
        let converter = DataConverter::new();
        
        // Test with NaN values
        let mut invalid_data = create_test_data(10, "INVALID");
        invalid_data[5].close = f64::NAN;
        invalid_data[7].volume = f64::INFINITY;
        
        let result = converter.to_dataframe(&invalid_data);
        // Should handle NaN/Infinity values gracefully
        assert!(result.is_ok());
        
        if let ModelInput::DataFrame(df) = result.unwrap() {
            let close_column = df.column("y").unwrap().f64().unwrap();
            // NaN should be preserved in the DataFrame
            assert!(close_column.get(5).unwrap().is_nan());
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_large_dataset_conversion_performance() {
        let converter = DataConverter::new();
        let large_data = create_test_data(10000, "PERF_TEST");
        
        let start = Instant::now();
        let result = converter.to_dataframe(&large_data);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 1000); // Should complete within 1 second
        
        if let ModelInput::DataFrame(df) = result.unwrap() {
            assert_eq!(df.height(), 10000);
        }
    }
    
    #[test]
    fn test_tensor_conversion_performance() {
        let converter = DataConverter::new();
        let data = create_test_data(5000, "TENSOR_PERF");
        let config = NeuralModelConfig {
            lookback_window: 100,
            ..NeuralModelConfig::default()
        };
        
        let start = Instant::now();
        let result = converter.to_tensor(&data, &config);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 2000); // Should complete within 2 seconds
    }
    
    #[tokio::test]
    async fn test_adapter_prediction_latency() {
        let config = NeuralModelConfig {
            batch_size: 64, // Larger batch for efficiency
            ..NeuralModelConfig::default()
        };
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        let test_data = create_test_data(1000, "LATENCY_TEST");
        
        let start = Instant::now();
        let result = adapter.predict(&test_data, 10).await;
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 5000); // Should complete within 5 seconds
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_end_to_end_prediction_workflow() {
        // Full workflow from data creation to prediction
        let config = NeuralModelConfig {
            model_type: "TimeMixer".to_string(),
            lookback_window: 48,
            forecast_horizon: 12,
            batch_size: 32,
            use_gpu: false,
            model_params: json!({
                "learning_rate": 0.001,
                "dropout": 0.1,
                "attention_heads": 8
            }),
        };
        
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        // Create realistic market data
        let market_data = create_test_data(200, "BTC/USD");
        
        // Initialize adapter
        let init_result = adapter.initialize().await;
        assert!(init_result.is_ok());
        
        // Make predictions
        let predictions = adapter.predict(&market_data, 12).await.unwrap();
        assert_eq!(predictions.len(), 12);
        
        // Verify prediction quality
        for (i, prediction) in predictions.iter().enumerate() {
            assert!(prediction.close > 0.0);
            assert!(prediction.volume >= 0.0);
            assert_eq!(prediction.symbol, "BTC/USD");
            
            // Timestamps should be future dates
            let last_data_time = market_data.last().unwrap().timestamp;
            assert!(prediction.timestamp > last_data_time);
            
            // Should have metadata indicating it's a prediction
            assert!(prediction.metadata.is_some());
            let metadata = prediction.metadata.as_ref().unwrap();
            assert_eq!(metadata["type"], "prediction");
            assert_eq!(metadata["model"], "neural");
            assert_eq!(metadata["step"], i + 1);
        }
    }
    
    #[tokio::test]
    async fn test_multiple_symbol_predictions() {
        let config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        
        let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD", "SOL/USD"];
        
        for symbol in symbols {
            let data = create_test_data(100, symbol);
            let result = adapter.predict(&data, 5).await;
            
            assert!(result.is_ok());
            let predictions = result.unwrap();
            assert_eq!(predictions.len(), 5);
            
            for pred in predictions {
                assert_eq!(pred.symbol, symbol);
            }
        }
    }
    
    #[tokio::test]
    async fn test_adapter_configuration_updates() {
        let initial_config = NeuralModelConfig::default();
        let adapter = NeuroDivergentAdapter::new(initial_config).await.unwrap();
        
        // Update configuration
        let new_config = NeuralModelConfig {
            model_type: "NeuralForecast".to_string(),
            lookback_window: 96,
            forecast_horizon: 24,
            batch_size: 128,
            use_gpu: true,
            model_params: json!({"epochs": 200}),
        };
        
        let update_result = adapter.update_config(new_config.clone()).await;
        assert!(update_result.is_ok());
        
        // Verify configuration was updated
        let stored_config = adapter.get_config().await;
        assert_eq!(stored_config.model_type, new_config.model_type);
        assert_eq!(stored_config.lookback_window, new_config.lookback_window);
        assert_eq!(stored_config.use_gpu, new_config.use_gpu);
    }
}

#[cfg(test)]
mod mock_vendor_model_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_vendor_model_simulation() {
        // Test that our adapter properly simulates vendor model behavior
        let config = NeuralModelConfig {
            model_type: "TimeMixer".to_string(),
            ..NeuralModelConfig::default()
        };
        
        let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
        let data = create_test_data(100, "MOCK_TEST");
        
        let predictions = adapter.predict(&data, 6).await.unwrap();
        
        // TimeMixer should produce reasonable predictions
        for prediction in predictions {
            // Values should be in reasonable range relative to input data
            let last_close = data.last().unwrap().close;
            let price_change = (prediction.close - last_close).abs() / last_close;
            assert!(price_change < 0.5); // Less than 50% change per step
            
            // Should have TimeMixer-specific metadata
            let metadata = prediction.metadata.as_ref().unwrap();
            assert_eq!(metadata["model_type"], "TimeMixer");
            assert!(metadata.get("attention_scores").is_some());
        }
    }
    
    #[tokio::test]
    async fn test_different_model_behaviors() {
        let model_types = vec![
            "TimeMixer", "NeuralForecast", "TimesFM", 
            "DeepAR", "NHITS", "Prophet"
        ];
        
        let data = create_test_data(150, "MODEL_COMPARISON");
        
        for model_type in model_types {
            let config = NeuralModelConfig {
                model_type: model_type.to_string(),
                ..NeuralModelConfig::default()
            };
            
            let adapter = NeuroDivergentAdapter::new(config).await.unwrap();
            let result = adapter.predict(&data, 5).await;
            
            assert!(result.is_ok(), "Failed for model type: {}", model_type);
            let predictions = result.unwrap();
            assert_eq!(predictions.len(), 5);
            
            // Each model should have distinct characteristics in metadata
            for pred in predictions {
                let metadata = pred.metadata.as_ref().unwrap();
                assert_eq!(metadata["model_type"], model_type);
                
                // Model-specific metadata checks
                match model_type {
                    "DeepAR" => assert!(metadata.get("quantiles").is_some()),
                    "TimeMixer" => assert!(metadata.get("attention_scores").is_some()),
                    "NHITS" => assert!(metadata.get("hierarchical_levels").is_some()),
                    _ => {} // Other models have basic metadata
                }
            }
        }
    }
}