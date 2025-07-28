//! Error handling and edge case tests for NeuroDivergent integration
//!
//! This module tests error conditions, edge cases, and recovery scenarios

use autonomous_platform::adapters::neuro_divergent::NeuroDivergentAdapter;
use autonomous_platform::adapters::AdapterError;
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::neural::fann_predictor::{FannPredictor, FannModelConfig};
use autonomous_platform::config::NeuralConfig;
use chrono::{DateTime, Utc, TimeZone};
use std::collections::HashMap;
use anyhow::Result;
use polars::prelude::*;

#[cfg(test)]
mod adapter_error_tests {
    use super::*;
    
    #[test]
    fn test_empty_data_error() {
        let empty_data: Vec<TimeSeriesData> = vec![];
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&empty_data);
        
        assert!(result.is_err());
        match result.unwrap_err().downcast::<AdapterError>() {
            Ok(AdapterError::Serialization(msg)) => {
                assert!(msg.contains("Empty data provided"));
            }
            _ => panic!("Expected AdapterError::Serialization"),
        }
    }
    
    #[test]
    fn test_missing_required_columns() {
        // Create a DataFrame missing required columns
        let df = DataFrame::new(vec![
            Series::new("random_column", vec![1, 2, 3]),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'ds' column"));
    }
    
    #[test]
    fn test_invalid_timestamp_format() {
        let invalid_timestamps = vec!["not_a_timestamp", "invalid", "xyz"];
        let closes = vec![100.0, 101.0, 102.0];
        
        let df = DataFrame::new(vec![
            Series::new("ds", &invalid_timestamps),
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_type_mismatch_error() {
        let timestamps = vec![1704067200i64];
        let closes = vec!["not_a_number"]; // String instead of f64
        
        let df = DataFrame::new(vec![
            Series::new("ds", &timestamps),
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid close price type"));
    }
}

#[cfg(test)]
mod boundary_condition_tests {
    use super::*;
    
    #[test]
    fn test_single_data_point() {
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close: 100.0,
            volume: 0.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        // Should succeed with single point
        let df_result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(df_result.is_ok());
        
        // But model preparation should fail with insufficient data
        let prep_result = NeuroDivergentAdapter::prepare_model_input(&data, 5, 1);
        assert!(prep_result.is_err());
    }
    
    #[test]
    fn test_exact_minimum_data_points() {
        // Create exactly the minimum required data points
        let lookback = 5;
        let horizon = 3;
        let min_points = lookback + horizon;
        
        let data: Vec<_> = (0..min_points).map(|i| TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(i as i64),
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
        }).collect();
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, lookback, horizon);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        assert_eq!(features.shape()[0], 1); // Only one sample possible
        assert_eq!(targets.len(), 1);
    }
    
    #[test]
    fn test_maximum_indicators() {
        // Test with many indicators
        let mut indicators = HashMap::new();
        for i in 0..100 {
            indicators.insert(format!("indicator_{}", i), i as f64);
        }
        
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
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
        assert!(result.is_ok());
        
        let df = result.unwrap();
        // Should have 7 base columns + 100 indicators
        assert_eq!(df.width(), 107);
    }
}

#[cfg(test)]
mod nan_infinity_tests {
    use super::*;
    
    #[test]
    fn test_nan_values_handling() {
        let mut indicators = HashMap::new();
        indicators.insert("nan_indicator".to_string(), f64::NAN);
        indicators.insert("valid_indicator".to_string(), 50.0);
        
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: f64::NAN,
            low: 99.0,
            close: 100.0,
            volume: f64::NAN,
            indicators,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let df_result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(df_result.is_ok());
        
        let df = df_result.unwrap();
        let high_col = df.column("high").unwrap().f64().unwrap();
        assert!(high_col.get(0).unwrap().is_nan());
    }
    
    #[test]
    fn test_infinity_values() {
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: f64::INFINITY,
            high: f64::NEG_INFINITY,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let df_result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(df_result.is_ok());
        
        let df = df_result.unwrap();
        let open_col = df.column("open").unwrap().f64().unwrap();
        assert!(open_col.get(0).unwrap().is_infinite());
    }
    
    #[test]
    fn test_mixed_valid_invalid_data() {
        let data = vec![
            TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: Utc::now(),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                volume: 1000.0,
                indicators: HashMap::new(),
                source: None,
                entity: None,
                value: None,
                metadata: None,
            },
            TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: Utc::now() + chrono::Duration::hours(1),
                open: f64::NAN,
                high: f64::INFINITY,
                low: f64::NEG_INFINITY,
                close: 0.0,
                volume: f64::NAN,
                indicators: HashMap::new(),
                source: None,
                entity: None,
                value: None,
                metadata: None,
            },
        ];
        
        let result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(result.is_ok());
        
        let df = result.unwrap();
        assert_eq!(df.height(), 2);
    }
}

#[cfg(test)]
mod timestamp_edge_cases {
    use super::*;
    
    #[test]
    fn test_unix_epoch_timestamp() {
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: DateTime::from_timestamp(0, 0).unwrap(), // Unix epoch
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }];
        
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST").unwrap();
        
        assert_eq!(converted[0].timestamp.timestamp(), 0);
    }
    
    #[test]
    fn test_future_timestamp() {
        let future = Utc.ymd(2100, 1, 1).and_hms(0, 0, 0);
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: future,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
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
    fn test_invalid_timestamp_recovery() {
        let timestamps = vec![i64::MAX]; // Invalid timestamp
        let closes = vec![100.0];
        
        let df = DataFrame::new(vec![
            Series::new("ds", &timestamps),
            Series::new("y", &closes),
        ]).unwrap();
        
        let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "TEST");
        assert!(result.is_ok());
        
        // Should fallback to current time
        let converted = result.unwrap();
        let now = Utc::now();
        let diff = (converted[0].timestamp - now).num_seconds().abs();
        assert!(diff < 60); // Within 1 minute of now
    }
}

#[cfg(test)]
mod memory_stress_tests {
    use super::*;
    
    #[test]
    fn test_large_indicator_count() {
        // Create data with many indicators
        let mut indicators = HashMap::new();
        for i in 0..1000 {
            indicators.insert(format!("ind_{}", i), i as f64);
        }
        
        let data = vec![TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
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
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_very_long_symbol_names() {
        let long_symbol = "A".repeat(1000);
        let data = vec![TimeSeriesData {
            symbol: long_symbol.clone(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("B".repeat(500)),
            entity: Some("C".repeat(500)),
            value: None,
            metadata: None,
        }];
        
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, &long_symbol).unwrap();
        
        assert_eq!(converted[0].symbol, long_symbol);
    }
}

#[cfg(test)]
mod concurrent_error_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task;
    
    #[tokio::test]
    async fn test_concurrent_empty_data_errors() {
        let mut handles = vec![];
        
        for _ in 0..10 {
            let handle = task::spawn(async {
                let empty: Vec<TimeSeriesData> = vec![];
                let result = NeuroDivergentAdapter::to_neuro_divergent_df(&empty);
                assert!(result.is_err());
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_invalid_conversions() {
        let invalid_df = Arc::new(DataFrame::new(vec![
            Series::new("invalid", vec![1, 2, 3]),
        ]).unwrap());
        
        let mut handles = vec![];
        
        for _ in 0..5 {
            let df_clone = Arc::clone(&invalid_df);
            let handle = task::spawn(async move {
                let result = NeuroDivergentAdapter::from_neuro_divergent_df(&df_clone, "TEST");
                assert!(result.is_err());
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
    }
}

#[cfg(test)]
mod recovery_scenario_tests {
    use super::*;
    
    #[test]
    fn test_partial_data_recovery() {
        // Some valid, some invalid data
        let mut data = vec![];
        
        // Valid data point
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        });
        
        // Invalid data point (will be processed with NaN)
        let mut invalid_indicators = HashMap::new();
        invalid_indicators.insert("bad".to_string(), f64::NAN);
        
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(1),
            open: f64::NAN,
            high: f64::NAN,
            low: f64::NAN,
            close: f64::NAN,
            volume: f64::NAN,
            indicators: invalid_indicators,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        });
        
        let df_result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(df_result.is_ok());
        
        let df = df_result.unwrap();
        assert_eq!(df.height(), 2);
    }
    
    #[test]
    fn test_indicator_mismatch_recovery() {
        // Different indicators across data points
        let mut data = vec![];
        
        let mut indicators1 = HashMap::new();
        indicators1.insert("rsi".to_string(), 50.0);
        
        let mut indicators2 = HashMap::new();
        indicators2.insert("macd".to_string(), 0.001);
        
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: indicators1,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        });
        
        data.push(TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(1),
            open: 101.0,
            high: 102.0,
            low: 100.0,
            close: 101.0,
            volume: 1100.0,
            indicators: indicators2,
            source: None,
            entity: None,
            value: None,
            metadata: None,
        });
        
        let df_result = NeuroDivergentAdapter::to_neuro_divergent_df(&data);
        assert!(df_result.is_ok());
        
        let df = df_result.unwrap();
        // Missing indicators should be filled with 0.0
        assert!(df.get_column_names().contains(&"rsi"));
        assert!(df.get_column_names().contains(&"macd"));
    }
}

#[cfg(test)]
mod model_preparation_edge_cases {
    use super::*;
    
    #[test]
    fn test_zero_lookback() {
        let data: Vec<_> = (0..10).map(|i| TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(i),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }).collect();
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 0, 1);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_zero_horizon() {
        let data: Vec<_> = (0..10).map(|i| TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(i),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        }).collect();
        
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 5, 0);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_extreme_lookback_horizon() {
        let data: Vec<_> = (0..100).map(|i| TimeSeriesData {
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
        }).collect();
        
        // Extreme but valid values
        let result = NeuroDivergentAdapter::prepare_model_input(&data, 50, 49);
        assert!(result.is_ok());
        
        let (features, targets) = result.unwrap();
        assert_eq!(features.shape()[0], 1); // Only 1 sample possible
    }
}