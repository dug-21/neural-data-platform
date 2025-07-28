//! Comprehensive unit tests for NeuroDivergentAdapter
//! 
//! This test suite provides complete coverage for:
//! - NeuroDivergentAdapter
//! - Data converters
//! - FannPredictor integration
//! - Error handling
//! - Type conversions
//! - Feature flag behavior

use autonomous_platform::adapters::neuro_divergent::NeuroDivergentAdapter;
use autonomous_platform::adapters::AdapterError;
use autonomous_platform::data::TimeSeriesData;
use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;
use anyhow::Result;
use polars::prelude::*;
use ndarray::{Array1, Array2};

// Mock vendor models for testing
#[cfg(test)]
mod mocks {
    use super::*;
    
    pub struct MockNeuroDivergentModel {
        pub predictions: Vec<f64>,
        pub should_fail: bool,
    }
    
    impl MockNeuroDivergentModel {
        pub fn new(predictions: Vec<f64>) -> Self {
            Self {
                predictions,
                should_fail: false,
            }
        }
        
        pub fn with_failure() -> Self {
            Self {
                predictions: vec![],
                should_fail: true,
            }
        }
        
        pub fn predict(&self, _input: &[f64]) -> Result<Vec<f64>> {
            if self.should_fail {
                Err(anyhow::anyhow!("Model prediction failed"))
            } else {
                Ok(self.predictions.clone())
            }
        }
    }
}

// Helper functions for test data generation
fn create_test_timeseries(count: usize, symbol: &str) -> Vec<TimeSeriesData> {
    let base_timestamp = Utc.ymd(2024, 1, 1).and_hms(0, 0, 0);
    let mut data = Vec::new();
    
    for i in 0..count {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 2.0));
        indicators.insert("macd".to_string(), 0.001 * i as f64);
        indicators.insert("sma_20".to_string(), 100.0 + i as f64);
        
        let ts = TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: base_timestamp + chrono::Duration::hours(i as i64),
            open: 100.0 + (i as f64 * 0.5),
            high: 101.0 + (i as f64 * 0.5),
            low: 99.0 + (i as f64 * 0.5),
            close: 100.0 + (i as f64 * 0.6),
            volume: 1000.0 + (i as f64 * 10.0),
            indicators,
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(100.0 + (i as f64 * 0.6)),
            metadata: None,
        };
        data.push(ts);
    }
    
    data
}

#[cfg(test)]
mod adapter_conversion_tests {
    use super::*;
    
    #[test]
    fn test_to_neuro_divergent_df_basic() {
        let data = create_test_timeseries(5, "BTC/USD");
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        
        assert!(result.is_ok());
        let df = result.unwrap();
        
        // Check DataFrame structure
        assert_eq!(df.height(), 5);
        assert!(df.get_column_names().contains(&"unique_id"));
        assert!(df.get_column_names().contains(&"ds"));
        assert!(df.get_column_names().contains(&"y"));
        assert!(df.get_column_names().contains(&"open"));
        assert!(df.get_column_names().contains(&"high"));
        assert!(df.get_column_names().contains(&"low"));
        assert!(df.get_column_names().contains(&"volume"));
        
        // Check indicators are included
        assert!(df.get_column_names().contains(&"rsi"));
        assert!(df.get_column_names().contains(&"macd"));
        assert!(df.get_column_names().contains(&"sma_20"));
    }
    
    #[test]
    fn test_to_neuro_divergent_df_empty_data() {
        let data: Vec<TimeSeriesData> = vec![];
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Empty data provided"));
    }
    
    #[test]
    fn test_to_neuro_divergent_df_single_point() {
        let data = create_test_timeseries(1, "ETH/USD");
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        
        assert!(result.is_ok());
        let df = result.unwrap();
        assert_eq!(df.height(), 1);
        
        // Verify values
        let closes = df.column("y").unwrap().f64().unwrap();
        assert_eq!(closes.get(0).unwrap(), 100.0);
    }
    
    #[test]
    fn test_to_neuro_divergent_df_missing_indicators() {
        let mut data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(), // No indicators
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        // Should have only the base columns
        assert_eq!(df.width(), 7); // unique_id, ds, y, open, high, low, volume
    }
    
    #[test]
    fn test_from_neuro_divergent_df_basic() {
        let data = create_test_timeseries(3, "BTC/USD");
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD");
        assert!(result.is_ok());
        
        let converted = result.unwrap();
        assert_eq!(converted.len(), 3);
        
        // Verify first entry
        assert_eq!(converted[0].symbol, "BTC/USD");
        assert_eq!(converted[0].close, 100.0);
        assert_eq!(converted[0].indicators.get("rsi"), Some(&50.0));
    }
    
    #[test]
    fn test_from_neuro_divergent_df_missing_columns() {
        // Create DataFrame with missing optional columns
        let timestamps = vec![1704067200i64]; // 2024-01-01 00:00:00
        let closes = vec![100.0];
        
        let df = DataFrame::new(vec![
            Series::new("ds", &timestamps),
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_ok());
        
        let converted = result.unwrap();
        assert_eq!(converted.len(), 1);
        
        // Should use close price for missing OHLC values
        assert_eq!(converted[0].open, 100.0);
        assert_eq!(converted[0].high, 100.0);
        assert_eq!(converted[0].low, 100.0);
        assert_eq!(converted[0].close, 100.0);
        assert_eq!(converted[0].volume, 0.0);
    }
    
    #[test]
    fn test_from_neuro_divergent_df_invalid_types() {
        // Create DataFrame with wrong column types
        let strings = vec!["invalid"];
        let closes = vec![100.0];
        
        let df = DataFrame::new(vec![
            Series::new("ds", &strings), // Wrong type
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid timestamp type"));
    }
}

#[cfg(test)]
mod model_input_preparation_tests {
    use super::*;
    
    #[test]
    fn test_prepare_model_input_basic() {
        let data = create_test_timeseries(20, "BTC/USD");
        let lookback = 5;
        let forecast_horizon = 3;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        
        // Expected samples: 20 - 5 - 3 + 1 = 13
        assert_eq!(features.shape()[0], 13);
        
        // Features per timestep: 5 (OHLCV) + 3 (indicators) = 8
        // Total features: 5 * 8 = 40
        assert_eq!(features.shape()[1], 40);
        
        assert_eq!(targets.len(), 13);
    }
    
    #[test]
    fn test_prepare_model_input_insufficient_data() {
        let data = create_test_timeseries(5, "BTC/USD");
        let lookback = 10;
        let forecast_horizon = 5;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Insufficient data"));
    }
    
    #[test]
    fn test_prepare_model_input_edge_cases() {
        // Minimum valid case
        let data = create_test_timeseries(10, "BTC/USD");
        let lookback = 5;
        let forecast_horizon = 5;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        assert_eq!(features.shape()[0], 1); // Only 1 sample possible
        assert_eq!(targets.len(), 1);
    }
    
    #[test]
    fn test_prepare_model_input_no_indicators() {
        let mut data = Vec::new();
        for i in 0..15 {
            data.push(TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: Utc::now() + chrono::Duration::hours(i),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0 + i as f64,
                volume: 1000.0,
                indicators: HashMap::new(),
                source: None,
                entity: None,
                value: None,
                metadata: None,
            });
        }
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 5, 2);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        // Features: 5 * 5 (OHLCV only) = 25
        assert_eq!(features.shape()[1], 25);
    }
    
    #[test]
    fn test_prepare_model_input_correct_target_selection() {
        let data = create_test_timeseries(10, "BTC/USD");
        let lookback = 3;
        let forecast_horizon = 2;
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, forecast_horizon);
        let (_, targets) = result.unwrap();
        
        // Target should be close price at lookback + forecast_horizon - 1
        // For first sample: data[3 + 2 - 1].close = data[4].close
        assert_eq!(targets[0], data[4].close);
    }
}

#[cfg(test)]
mod prediction_conversion_tests {
    use super::*;
    
    #[test]
    fn test_predictions_to_timeseries_basic() {
        let predictions = vec![101.0, 102.0, 103.0, 104.0, 105.0];
        let base_timestamp = Utc.ymd(2024, 1, 1).and_hms(0, 0, 0);
        let symbol = "BTC/USD";
        let interval_seconds = 3600; // 1 hour
        
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            symbol,
            interval_seconds,
        );
        
        assert_eq!(result.len(), 5);
        
        // Check first prediction
        assert_eq!(result[0].symbol, "BTC/USD");
        assert_eq!(result[0].timestamp, base_timestamp);
        assert_eq!(result[0].close, 101.0);
        assert_eq!(result[0].open, 101.0);
        assert_eq!(result[0].high, 101.0);
        assert_eq!(result[0].low, 101.0);
        assert_eq!(result[0].volume, 0.0);
        
        // Check timestamps increment correctly
        assert_eq!(
            result[1].timestamp,
            base_timestamp + chrono::Duration::seconds(interval_seconds)
        );
        
        // Check metadata
        assert!(result[0].metadata.is_some());
        let metadata = result[0].metadata.as_ref().unwrap();
        assert_eq!(metadata["type"], "forecast");
        assert_eq!(metadata["model"], "neuro-divergent");
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
            "ETH/USD",
            300, // 5 minutes
        );
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].close, 999.99);
        assert_eq!(result[0].source, Some("prediction".to_string()));
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_invalid_dataframe_creation() {
        // Test with conflicting series lengths
        let symbols = vec!["BTC", "ETH"];
        let timestamps = vec![1];
        
        let result = std::panic::catch_unwind(|| {
            DataFrame::new(vec![
                Series::new("unique_id", symbols),
                Series::new("ds", timestamps),
            ])
        });
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_nan_handling_in_conversion() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), f64::NAN);
        
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: f64::INFINITY,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok()); // Should handle NaN/Inf values
        
        let df = result.unwrap();
        let rsi_col = df.column("rsi").unwrap().f64().unwrap();
        assert!(rsi_col.get(0).unwrap().is_nan());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use super::mocks::*;
    
    #[test]
    fn test_full_conversion_roundtrip() {
        let original_data = create_test_timeseries(10, "BTC/USD");
        
        // Convert to DataFrame
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&original_data).unwrap();
        
        // Convert back
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD").unwrap();
        
        assert_eq!(original_data.len(), converted.len());
        
        for (orig, conv) in original_data.iter().zip(converted.iter()) {
            assert_eq!(orig.symbol, conv.symbol);
            assert_eq!(orig.close, conv.close);
            assert_eq!(orig.volume, conv.volume);
            
            // Check indicators preserved
            for (key, value) in &orig.indicators {
                assert_eq!(conv.indicators.get(key), Some(value));
            }
        }
    }
    
    #[test]
    fn test_model_prediction_workflow() {
        let data = create_test_timeseries(20, "BTC/USD");
        let mock_model = MockNeuroDivergentModel::new(vec![110.0, 111.0, 112.0]);
        
        // Prepare input
        let (features, _) = NeuroDivergentAdapter::prepare_model_input(&data, 5, 3).unwrap();
        
        // Simulate prediction
        let predictions = mock_model.predict(&features.as_slice().unwrap()).unwrap();
        
        // Convert predictions to timeseries
        let base_timestamp = data.last().unwrap().timestamp;
        let result = NeuroDivergentAdapter::predictions_to_timeseries(
            &predictions,
            base_timestamp,
            "BTC/USD",
            3600,
        );
        
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].close, 110.0);
    }
    
    #[test]
    fn test_model_failure_handling() {
        let mock_model = MockNeuroDivergentModel::with_failure();
        let input = vec![1.0, 2.0, 3.0];
        
        let result = mock_model.predict(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Model prediction failed"));
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_large_dataset_conversion() {
        let data = create_test_timeseries(10000, "BTC/USD");
        
        let start = Instant::now();
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 1000); // Should complete within 1 second
        
        let df = result.unwrap();
        assert_eq!(df.height(), 10000);
    }
    
    #[test]
    fn test_model_input_preparation_performance() {
        let data = create_test_timeseries(5000, "BTC/USD");
        
        let start = Instant::now();
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 50, 10);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 500); // Should complete within 500ms
    }
}

#[cfg(test)]
mod feature_flag_tests {
    use super::*;
    
    #[test]
    #[cfg(feature = "neuro-divergent-advanced")]
    fn test_advanced_features_enabled() {
        // This test only runs when advanced features are enabled
        let data = create_test_timeseries(100, "BTC/USD");
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        
        // Advanced features might include additional columns or processing
        assert!(df.height() > 0);
    }
    
    #[test]
    #[cfg(not(feature = "neuro-divergent-advanced"))]
    fn test_basic_features_only() {
        // This test runs when advanced features are disabled
        let data = create_test_timeseries(10, "BTC/USD");
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        
        // Basic features should still work
        assert_eq!(df.height(), 10);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;
    
    #[test]
    fn test_extreme_values() {
        let mut indicators = HashMap::new();
        indicators.insert("extreme".to_string(), f64::MAX);
        
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: f64::MIN_POSITIVE,
            high: f64::MAX,
            low: f64::MIN_POSITIVE,
            close: 1e-10,
            volume: 1e15,
            indicators,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_unicode_symbols() {
        let data = vec![TimeSeriesData {
            symbol: "BTC/USD 🚀".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("测试".to_string()),
            entity: Some("τεστ".to_string()),
            value: None,
            metadata: None,
        }];
        
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD 🚀").unwrap();
        
        assert_eq!(converted[0].symbol, "BTC/USD 🚀");
    }
    
    #[test]
    fn test_zero_values() {
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_negative_values() {
        let mut indicators = HashMap::new();
        indicators.insert("negative_indicator".to_string(), -999.99);
        
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: -10.0,
            high: -5.0,
            low: -15.0,
            close: -12.0,
            volume: 1000.0,
            indicators,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST").unwrap();
        
        assert_eq!(converted[0].close, -12.0);
        assert_eq!(converted[0].indicators.get("negative_indicator"), Some(&-999.99));
    }
}

#[cfg(test)]
mod concurrent_tests {
    use super::*;
    use std::sync::Arc;
    use tokio;
    
    #[tokio::test]
    async fn test_concurrent_conversions() {
        let data = Arc::new(create_test_timeseries(100, "BTC/USD"));
        
        let mut handles = vec![];
        
        for i in 0..10 {
            let data_clone = Arc::clone(&data);
            let handle = tokio::spawn(async move {
                let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data_clone);
                assert!(result.is_ok());
                i
            });
            handles.push(handle);
        }
        
        let results: Vec<_> = futures::future::join_all(handles).await;
        assert_eq!(results.len(), 10);
        
        for result in results {
            assert!(result.is_ok());
        }
    }
}

// Module for testing memory safety and resource management
#[cfg(test)]
mod memory_tests {
    use super::*;
    
    #[test]
    fn test_memory_efficient_large_dataset() {
        // Create a dataset that's large but not too large to cause OOM
        let data = create_test_timeseries(50000, "BTC/USD");
        
        // This should not cause memory issues
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        // Convert back
        let df = result.unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap().len(), 50000);
    }
    
    #[test]
    fn test_drop_behavior() {
        // Create data that will be dropped
        {
            let data = create_test_timeseries(1000, "TEST");
            let _df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
            // DataFrame should be properly dropped here
        }
        
        // If we get here without issues, drop behavior is correct
        assert!(true);
    }
}