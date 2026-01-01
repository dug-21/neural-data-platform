//! Comprehensive Unit Tests for DataConverter
//!
//! Tests format conversion, normalization, technical indicators, and error handling.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;

use crate::data::data_converter::{
    DataConverter, DataConverterConfig, ConversionMetadata, NormalizationStats
};
use crate::data::TimeSeriesData;
use crate::neural::vendor_predictor::{VendorTimeSeriesData, ForecastResult};

// Test utilities
fn create_test_time_series_data(values: Vec<f64>) -> TimeSeriesData {
    let timestamps: Vec<DateTime<Utc>> = (0..values.len())
        .map(|i| Utc.timestamp_opt(1600000000 + i as i64 * 3600, 0).unwrap())
        .collect();
    
    let mut metadata = HashMap::new();
    metadata.insert("symbol".to_string(), serde_json::json!("AAPL"));
    metadata.insert("source".to_string(), serde_json::json!("test"));
    
    TimeSeriesData {
        values,
        timestamps,
        metadata,
        symbol: "AAPL".to_string(),
        metadata_map: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!("AAPL"));
            map
        }
    }
}

fn create_test_config() -> DataConverterConfig {
    DataConverterConfig {
        normalize_data: true,
        normalization_method: "minmax".to_string(),
        remove_outliers: true,
        outlier_method: "iqr".to_string(),
        max_missing_percent: 10.0,
        missing_fill_method: "forward".to_string(),
        enable_feature_engineering: true,
        technical_indicators: vec![
            "sma_5".to_string(),
            "sma_20".to_string(),
            "rsi_14".to_string(),
            "macd".to_string(),
        ],
        time_features: vec![
            "hour".to_string(),
            "day_of_week".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_data_converter_creation() {
        let config = create_test_config();
        let converter = DataConverter::new(config.clone());
        
        // Verify configuration is stored correctly
        assert_eq!(converter.config.normalize_data, config.normalize_data);
        assert_eq!(converter.config.normalization_method, config.normalization_method);
        assert_eq!(converter.config.technical_indicators.len(), 4);
    }
    
    #[test]
    fn test_default_configuration() {
        let converter = DataConverter::default();
        
        // Verify default configuration
        assert!(converter.config.normalize_data);
        assert_eq!(converter.config.normalization_method, "minmax");
        assert!(converter.config.remove_outliers);
        assert_eq!(converter.config.outlier_method, "iqr");
        assert_eq!(converter.config.max_missing_percent, 5.0);
        assert_eq!(converter.config.missing_fill_method, "forward");
        assert!(converter.config.enable_feature_engineering);
        assert!(converter.config.technical_indicators.len() > 0);
    }
    
    #[test]
    fn test_basic_conversion_to_vendor_format() {
        let mut converter = DataConverter::default();
        let test_data = create_test_time_series_data(vec![100.0, 101.0, 99.0, 102.0, 98.0]);
        
        let result = converter.to_vendor_format(&test_data, "AAPL");
        assert!(result.is_ok());
        
        let (vendor_data, metadata) = result.unwrap();
        
        // Verify vendor data structure
        assert!(!vendor_data.values.is_empty());
        assert!(vendor_data.values.iter().all(|&v| v.is_finite()));
        
        // Verify metadata
        assert_eq!(metadata.source_format, "TimeSeriesData");
        assert_eq!(metadata.target_format, "VendorTimeSeriesData<f32>");
        assert_eq!(metadata.original_length, 5);
        assert!(metadata.converted_length >= 5); // May be larger due to feature engineering
    }
    
    #[test]
    fn test_normalization_minmax() {
        let mut converter = DataConverter::new(DataConverterConfig {
            normalize_data: true,
            normalization_method: "minmax".to_string(),
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let test_data = create_test_time_series_data(vec![100.0, 200.0, 150.0, 300.0, 50.0]);
        let result = converter.to_vendor_format(&test_data, "TEST");
        
        assert!(result.is_ok());
        let (vendor_data, metadata) = result.unwrap();
        
        // Check normalization was applied (values between 0 and 1)
        assert!(vendor_data.values.iter().all(|&v| v >= 0.0 && v <= 1.0));
        
        // Check normalization stats
        assert!(metadata.normalization_stats.is_some());
        let stats = metadata.normalization_stats.unwrap();
        assert_eq!(stats.method, "minmax");
        assert_eq!(stats.min_value, 50.0);
        assert_eq!(stats.max_value, 300.0);
    }
    
    #[test]
    fn test_normalization_zscore() {
        let mut converter = DataConverter::new(DataConverterConfig {
            normalize_data: true,
            normalization_method: "zscore".to_string(),
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let test_data = create_test_time_series_data(vec![100.0, 200.0, 150.0, 300.0, 50.0]);
        let result = converter.to_vendor_format(&test_data, "TEST");
        
        assert!(result.is_ok());
        let (vendor_data, metadata) = result.unwrap();
        
        // Check normalization stats
        assert!(metadata.normalization_stats.is_some());
        let stats = metadata.normalization_stats.unwrap();
        assert_eq!(stats.method, "zscore");
        assert!(stats.mean > 0.0);
        assert!(stats.std_dev > 0.0);
    }
    
    #[test]
    fn test_missing_value_handling_forward_fill() {
        let converter = DataConverter::new(DataConverterConfig {
            missing_fill_method: "forward".to_string(),
            normalize_data: false,
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let values = vec![1.0, f64::NAN, 3.0, f64::NAN, 5.0];
        let result = converter.handle_missing_values(&values);
        
        assert!(result.is_ok());
        let processed = result.unwrap();
        
        // Check no NaN values remain
        assert!(processed.iter().all(|v| !v.is_nan()));
        assert_eq!(processed[1], 1.0); // Forward filled from index 0
        assert_eq!(processed[3], 3.0); // Forward filled from index 2
    }
    
    #[test]
    fn test_missing_value_handling_backward_fill() {
        let converter = DataConverter::new(DataConverterConfig {
            missing_fill_method: "backward".to_string(),
            normalize_data: false,
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let values = vec![f64::NAN, 2.0, f64::NAN, 4.0, 5.0];
        let result = converter.handle_missing_values(&values);
        
        assert!(result.is_ok());
        let processed = result.unwrap();
        
        // Check no NaN values remain
        assert!(processed.iter().all(|v| !v.is_nan()));
        assert_eq!(processed[0], 2.0); // Backward filled from index 1
        assert_eq!(processed[2], 4.0); // Backward filled from index 3
    }
    
    #[test]
    fn test_missing_value_handling_mean_fill() {
        let converter = DataConverter::new(DataConverterConfig {
            missing_fill_method: "mean".to_string(),
            normalize_data: false,
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let values = vec![1.0, f64::NAN, 3.0, f64::NAN, 5.0];
        let result = converter.handle_missing_values(&values);
        
        assert!(result.is_ok());
        let processed = result.unwrap();
        
        // Check no NaN values remain
        assert!(processed.iter().all(|v| !v.is_nan()));
        
        // Mean of [1.0, 3.0, 5.0] = 3.0
        assert_eq!(processed[1], 3.0);
        assert_eq!(processed[3], 3.0);
    }
    
    #[test]
    fn test_missing_value_handling_interpolation() {
        let converter = DataConverter::new(DataConverterConfig {
            missing_fill_method: "interpolate".to_string(),
            normalize_data: false,
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let values = vec![1.0, f64::NAN, 5.0];
        let result = converter.handle_missing_values(&values);
        
        assert!(result.is_ok());
        let processed = result.unwrap();
        
        // Check no NaN values remain
        assert!(processed.iter().all(|v| !v.is_nan()));
        
        // Linear interpolation between 1.0 and 5.0 should be 3.0
        assert_eq!(processed[1], 3.0);
    }
    
    #[test]
    fn test_outlier_removal_iqr() {
        let converter = DataConverter::new(DataConverterConfig {
            remove_outliers: true,
            outlier_method: "iqr".to_string(),
            normalize_data: false,
            enable_feature_engineering: false,
            ..Default::default()
        });
        
        // Dataset with obvious outliers
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0, 200.0]; // 100, 200 are outliers
        let original_len = values.len();
        
        let result = converter.remove_outliers(&mut values);
        assert!(result.is_ok());
        
        let removed_count = result.unwrap();
        
        // Should have removed some outliers
        assert!(removed_count > 0);
        assert!(values.len() < original_len);
        
        // Outliers should be gone
        assert!(!values.contains(&100.0));
        assert!(!values.contains(&200.0));
    }
    
    #[test]
    fn test_outlier_removal_zscore() {
        let converter = DataConverter::new(DataConverterConfig {
            remove_outliers: true,
            outlier_method: "zscore".to_string(),
            normalize_data: false,
            enable_feature_engineering: false,
            ..Default::default()
        });
        
        // Dataset with obvious outliers
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0]; // 100 is outlier
        let original_len = values.len();
        
        let result = converter.remove_outliers(&mut values);
        assert!(result.is_ok());
        
        let removed_count = result.unwrap();
        
        // Should have removed the outlier
        assert!(removed_count > 0);
        assert!(values.len() < original_len);
        assert!(!values.contains(&100.0));
    }
    
    #[test]
    fn test_technical_indicators_sma() {
        let converter = DataConverter::new(DataConverterConfig {
            enable_feature_engineering: true,
            technical_indicators: vec!["sma_3".to_string()],
            normalize_data: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut features_added = Vec::new();
        
        let result = converter.add_technical_indicators(&values, &mut features_added);
        assert!(result.is_ok());
        
        let enhanced = result.unwrap();
        
        // Should have added SMA feature
        assert!(enhanced.len() > values.len());
        assert!(features_added.contains(&"sma_3".to_string()));
    }
    
    #[test]
    fn test_technical_indicators_rsi() {
        let converter = DataConverter::new(DataConverterConfig {
            enable_feature_engineering: true,
            technical_indicators: vec!["rsi_14".to_string()],
            normalize_data: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        // Create data with enough points for RSI calculation
        let values: Vec<f64> = (1..20).map(|i| i as f64).collect();
        let mut features_added = Vec::new();
        
        let result = converter.add_technical_indicators(&values, &mut features_added);
        assert!(result.is_ok());
        
        let enhanced = result.unwrap();
        
        // Should have added RSI feature
        assert!(enhanced.len() > values.len());
        assert!(features_added.contains(&"rsi_14".to_string()));
    }
    
    #[test]
    fn test_technical_indicators_macd() {
        let converter = DataConverter::new(DataConverterConfig {
            enable_feature_engineering: true,
            technical_indicators: vec!["macd".to_string()],
            normalize_data: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        // Create data with enough points for MACD calculation
        let values: Vec<f64> = (1..50).map(|i| 100.0 + (i as f64 * 0.5)).collect();
        let mut features_added = Vec::new();
        
        let result = converter.add_technical_indicators(&values, &mut features_added);
        assert!(result.is_ok());
        
        let enhanced = result.unwrap();
        
        // Should have added MACD feature
        assert!(enhanced.len() > values.len());
        assert!(features_added.contains(&"macd".to_string()));
    }
    
    #[test]
    fn test_sma_calculation() {
        let converter = DataConverter::default();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let sma = converter.calculate_sma(&values, 3);
        
        assert_eq!(sma.len(), 3);
        assert_eq!(sma[0], 2.0); // (1+2+3)/3 = 2
        assert_eq!(sma[1], 3.0); // (2+3+4)/3 = 3
        assert_eq!(sma[2], 4.0); // (3+4+5)/3 = 4
    }
    
    #[test]
    fn test_rsi_calculation() {
        let converter = DataConverter::default();
        let values = vec![44.0, 44.25, 44.5, 43.75, 44.5, 44.25, 44.0, 44.25, 44.5, 44.75, 45.0];
        
        let rsi = converter.calculate_rsi(&values, 3);
        
        assert!(!rsi.is_empty());
        // RSI values should be between 0 and 100
        assert!(rsi.iter().all(|&v| v >= 0.0 && v <= 100.0));
    }
    
    #[test]
    fn test_ema_calculation() {
        let converter = DataConverter::default();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let ema = converter.calculate_ema(&values, 3);
        
        assert_eq!(ema.len(), values.len());
        
        // First EMA value should be the initial SMA
        let expected_first = (1.0 + 2.0 + 3.0) / 3.0;
        assert_eq!(ema[0], expected_first);
        
        // Subsequent values should follow EMA formula
        assert!(ema[1] > ema[0]); // Should increase with increasing input
    }
    
    #[test]
    fn test_time_features() {
        let converter = DataConverter::new(DataConverterConfig {
            time_features: vec!["hour".to_string(), "day_of_week".to_string()],
            ..Default::default()
        });
        
        let timestamps = vec![
            Utc.timestamp_opt(1600000000, 0).unwrap(), // Sept 13, 2020 12:26:40 UTC (Sunday)
            Utc.timestamp_opt(1600003600, 0).unwrap(), // One hour later
        ];
        
        let result = converter.add_time_features(&timestamps);
        assert!(result.is_ok());
        
        let time_features = result.unwrap();
        
        // Should have hour and day_of_week features
        assert!(time_features.contains_key("hour"));
        assert!(time_features.contains_key("day_of_week"));
        
        let hours = &time_features["hour"];
        let days = &time_features["day_of_week"];
        
        assert_eq!(hours.len(), 2);
        assert_eq!(days.len(), 2);
        
        // Check hour values
        assert_eq!(hours[1], hours[0] + 1.0); // One hour difference
        
        // Check day values (both should be same day)
        assert_eq!(days[0], days[1]);
    }
    
    #[test]
    fn test_from_vendor_format_conversion() {
        let converter = DataConverter::default();
        
        // Create mock forecast result
        let forecast = ForecastResult {
            forecasts: vec![0.5, 0.7, 0.3],
            confidence: Some(0.8),
            metadata: None,
        };
        
        // Create mock metadata with normalization stats
        let metadata = ConversionMetadata {
            source_format: "TimeSeriesData".to_string(),
            target_format: "VendorTimeSeriesData<f32>".to_string(),
            conversion_timestamp: Utc::now(),
            normalization_stats: Some(NormalizationStats {
                method: "minmax".to_string(),
                min_value: 100.0,
                max_value: 200.0,
                mean: 150.0,
                std_dev: 28.87,
                median: 150.0,
                q25: 125.0,
                q75: 175.0,
            }),
            features_added: vec!["sma_5".to_string()],
            outliers_removed: 2,
            missing_filled: 1,
            original_length: 100,
            converted_length: 101,
        };
        
        let result = converter.from_vendor_format(&forecast, &metadata, "AAPL");
        assert!(result.is_ok());
        
        let converted = result.unwrap();
        
        // Check conversion back to original scale
        assert_eq!(converted.len(), 3);
        assert!(converted.iter().all(|&v| v >= 100.0 && v <= 200.0));
    }
    
    #[test]
    fn test_validation_empty_data() {
        let converter = DataConverter::default();
        let empty_data = create_test_time_series_data(vec![]);
        
        let result = converter.validate_input_data(&empty_data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }
    
    #[test]
    fn test_validation_too_many_missing_values() {
        let converter = DataConverter::new(DataConverterConfig {
            max_missing_percent: 10.0,
            ..Default::default()
        });
        
        // 50% missing values (above 10% threshold)
        let data_with_missing = create_test_time_series_data(vec![
            1.0, f64::NAN, 3.0, f64::NAN, 5.0, f64::NAN, 7.0, f64::NAN
        ]);
        
        let result = converter.validate_input_data(&data_with_missing);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Too many missing values"));
    }
    
    #[test]
    fn test_normalization_cache() {
        let mut converter = DataConverter::default();
        let test_data = create_test_time_series_data(vec![100.0, 200.0, 150.0]);
        
        // First conversion should create cache entry
        let result1 = converter.to_vendor_format(&test_data, "AAPL");
        assert!(result1.is_ok());
        
        // Check cache was populated
        let stats = converter.get_normalization_stats("AAPL");
        assert!(stats.is_some());
        
        let cached_stats = stats.unwrap();
        assert_eq!(cached_stats.method, "minmax");
        assert_eq!(cached_stats.min_value, 100.0);
        assert_eq!(cached_stats.max_value, 200.0);
        
        // Clear cache and verify it's empty
        converter.clear_cache();
        let stats_after_clear = converter.get_normalization_stats("AAPL");
        assert!(stats_after_clear.is_none());
    }
    
    #[test]
    fn test_edge_case_single_value() {
        let mut converter = DataConverter::new(DataConverterConfig {
            normalize_data: true,
            normalization_method: "minmax".to_string(),
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let single_value_data = create_test_time_series_data(vec![42.0]);
        let result = converter.to_vendor_format(&single_value_data, "SINGLE");
        
        assert!(result.is_ok());
        let (vendor_data, _) = result.unwrap();
        
        // With min=max, normalization should handle gracefully
        assert_eq!(vendor_data.values.len(), 1);
        assert!(vendor_data.values[0].is_finite());
    }
    
    #[test]
    fn test_edge_case_all_same_values() {
        let mut converter = DataConverter::new(DataConverterConfig {
            normalize_data: true,
            normalization_method: "zscore".to_string(),
            enable_feature_engineering: false,
            remove_outliers: false,
            ..Default::default()
        });
        
        let same_values_data = create_test_time_series_data(vec![42.0, 42.0, 42.0, 42.0]);
        let result = converter.to_vendor_format(&same_values_data, "SAME");
        
        assert!(result.is_ok());
        let (vendor_data, _) = result.unwrap();
        
        // With std_dev=0, z-score normalization should handle gracefully
        assert_eq!(vendor_data.values.len(), 4);
        assert!(vendor_data.values.iter().all(|v| v.is_finite()));
    }
    
    #[test]
    fn test_concurrent_conversions() {
        use std::sync::Arc;
        use std::thread;
        
        let converter = Arc::new(DataConverter::default());
        let mut handles = vec![];
        
        // Spawn multiple threads doing conversions
        for i in 0..5 {
            let converter_clone = Arc::clone(&converter);
            let handle = thread::spawn(move || {
                let data = create_test_time_series_data(vec![i as f64; 10]);
                let mut converter_ref = converter_clone.as_ref().clone();
                converter_ref.to_vendor_format(&data, &format!("SYMBOL_{}", i))
            });
            handles.push(handle);
        }
        
        // Wait for all threads and check results
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.is_ok());
        }
    }
}