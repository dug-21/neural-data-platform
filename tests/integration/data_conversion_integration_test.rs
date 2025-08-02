//! Integration tests for the complete data conversion pipeline
//! 
//! These tests verify that the conversion layer works correctly between
//! neural-trader and vendor neural model formats.

use std::collections::HashMap;
use chrono::Utc;
use anyhow::Result;

use neural_trader::data::TimeSeriesData;
use neural_trader::adapters::neural::{
    VendorFormatConverter,
    VendorDataConverter,
    SafeF32Convert,
    StreamingConverter,
    BatchConverter,
    ConversionErrorRecovery,
};

fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
    let base_time = Utc::now();
    
    (0..count).map(|i| {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + (i % 70) as f64);
        indicators.insert("macd".to_string(), -0.1 + (i as f64 * 0.01));
        indicators.insert("ema_20".to_string(), 50000.0 + i as f64 * 100.0);
        indicators.insert("bollinger_upper".to_string(), 52000.0 + i as f64 * 150.0);
        indicators.insert("bollinger_lower".to_string(), 48000.0 + i as f64 * 50.0);
        
        TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: base_time + chrono::Duration::hours(i as i64),
            open: 50000.0 + i as f64 * 100.0,
            high: 51000.0 + i as f64 * 120.0,
            low: 49500.0 + i as f64 * 80.0,
            close: 50500.0 + i as f64 * 110.0,
            volume: 1000.0 + i as f64 * 50.0,
            indicators,
            source: Some("integration_test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: None,
            metadata: None,
        }
    }).collect()
}

#[test]
fn test_complete_conversion_pipeline() -> Result<()> {
    let data = create_test_data(48); // 2 days of hourly data
    let converter = VendorFormatConverter::new();
    
    // Test conversion to vendor format
    let vendor_data = converter.to_neuro_divergent_f32(&data, "BTC/USD")?;
    
    // Validate conversion
    assert_eq!(vendor_data.len(), 48);
    assert_eq!(vendor_data.series_id, "BTC/USD");
    
    // Check first data point
    let first_point = &vendor_data.data_points[0];
    assert_eq!(first_point.value, 50500.0_f32);
    assert!(first_point.exogenous.is_some());
    assert_eq!(first_point.exogenous.as_ref().unwrap().len(), 5); // 5 indicators
    
    // Validate conversion integrity
    converter.validate_conversion(&data, &vendor_data)?;
    
    Ok(())
}

#[test]
fn test_type_conversion_safety() -> Result<()> {
    // Test various edge cases for f64 to f32 conversion
    let test_values = vec![
        0.0,
        1.0,
        -1.0,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        123456789.123456789,
        -987654321.987654321,
        1e-10,
        1e10,
    ];
    
    for &value in &test_values {
        let converted = value.to_f32_safe()?;
        assert!(converted.is_finite() || value.is_infinite());
        
        // Check that conversion is reasonable
        if value.is_finite() && value.abs() <= f32::MAX as f64 && value.abs() >= f32::MIN_POSITIVE as f64 {
            let relative_error = ((value - converted as f64).abs() / value.abs()).abs();
            assert!(relative_error < 0.01, "Too much precision loss: {} -> {}", value, converted);
        }
    }
    
    Ok(())
}

#[test]
fn test_special_value_handling() -> Result<()> {
    let special_values = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
    
    for &value in &special_values {
        let converted = value.to_f32_safe()?;
        
        if value.is_nan() {
            assert!(converted.is_nan());
        } else if value.is_infinite() {
            assert!(converted.is_infinite());
            assert_eq!(value.is_sign_positive(), converted.is_sign_positive());
        }
    }
    
    Ok(())
}

#[test]
fn test_batch_conversion() -> Result<()> {
    let mut data_batch = HashMap::new();
    
    // Create data for multiple symbols
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD", "DOT/USD"];
    for symbol in &symbols {
        let symbol_data = create_test_data(24);
        data_batch.insert(symbol.to_string(), symbol_data);
    }
    
    let converter = VendorFormatConverter::new();
    let converted_batch = converter.convert_batch(&data_batch)?;
    
    // Verify all symbols were converted
    assert_eq!(converted_batch.len(), 4);
    for symbol in &symbols {
        assert!(converted_batch.contains_key(*symbol));
        assert_eq!(converted_batch[*symbol].len(), 24);
    }
    
    // Verify batch conversion integrity
    BatchConverter::verify_conversions(&data_batch, &converted_batch)?;
    
    Ok(())
}

#[test]
fn test_streaming_conversion() -> Result<()> {
    let large_data = create_test_data(1000);
    let converter = VendorFormatConverter::new();
    
    // Convert using streaming (chunked) approach
    let streaming_result = converter.convert_streaming(
        large_data.iter().cloned(),
        "LARGE/DATASET",
        100, // chunk size
    )?;
    
    assert_eq!(streaming_result.len(), 1000);
    assert_eq!(streaming_result.series_id, "LARGE/DATASET");
    
    // Compare with direct conversion
    let direct_result = converter.to_neuro_divergent_f32(&large_data, "LARGE/DATASET")?;
    
    assert_eq!(streaming_result.len(), direct_result.len());
    
    // Verify values match
    for (stream, direct) in streaming_result.data_points.iter().zip(direct_result.data_points.iter()) {
        assert_eq!(stream.value, direct.value);
        assert_eq!(stream.timestamp, direct.timestamp);
    }
    
    Ok(())
}

#[test]
fn test_prediction_result_conversion() -> Result<()> {
    let base_data = create_test_data(1);
    let converter = VendorFormatConverter::new();
    
    // Simulate neural model predictions
    let predictions = vec![
        51000.0_f32, 51500.0_f32, 52000.0_f32, 
        52200.0_f32, 52800.0_f32, 53100.0_f32,
        53500.0_f32, 54000.0_f32
    ];
    
    let forecast_results = converter.from_vendor_predictions_f32(
        &predictions,
        &base_data[0],
        8, // forecast horizon
    )?;
    
    assert_eq!(forecast_results.len(), 8);
    
    // Verify prediction values
    for (i, result) in forecast_results.iter().enumerate() {
        assert_eq!(result.close, predictions[i] as f64);
        assert_eq!(result.symbol, "BTC/USD");
        assert!(result.source.as_ref().unwrap().contains("vendor"));
        
        // Check metadata
        assert!(result.metadata.is_some());
        let metadata = result.metadata.as_ref().unwrap();
        assert_eq!(metadata["forecast_step"], i + 1);
        assert_eq!(metadata["forecast_horizon"], 8);
    }
    
    Ok(())
}

#[test]
fn test_model_array_conversion() -> Result<()> {
    let data = create_test_data(50);
    let converter = VendorFormatConverter::new();
    
    let lookback_window = 24;
    let (features, feature_names) = converter.to_model_arrays(
        &data,
        lookback_window,
        &["close".to_string()],
    )?;
    
    // Verify array dimensions
    let expected_samples = data.len() - lookback_window + 1;
    let expected_features = lookback_window * (5 + 5); // 5 OHLCV + 5 indicators
    
    assert_eq!(features.shape()[0], expected_samples);
    assert_eq!(features.shape()[1], expected_features);
    assert_eq!(feature_names.len(), expected_features);
    
    // Verify feature names format
    assert!(feature_names[0].starts_with("open_lag_"));
    assert!(feature_names.iter().any(|name| name.contains("rsi_lag_")));
    assert!(feature_names.iter().any(|name| name.contains("macd_lag_")));
    
    // Verify all values are finite
    for row in 0..features.shape()[0] {
        for col in 0..features.shape()[1] {
            assert!(features[[row, col]].is_finite(), 
                "Non-finite value at [{}, {}]: {}", row, col, features[[row, col]]);
        }
    }
    
    Ok(())
}

#[test]
fn test_error_recovery_mechanisms() -> Result<()> {
    // Test precision loss recovery
    let large_value = 1e15_f64;
    let failed_conversion = large_value as f32;
    
    let recovered = ConversionErrorRecovery::recover_precision_loss(large_value, failed_conversion)?;
    assert!(recovered.is_finite());
    
    // Test edge case handling
    let nan_recovered = ConversionErrorRecovery::handle_edge_cases(f64::NAN);
    assert_eq!(nan_recovered, 0.0);
    
    let inf_recovered = ConversionErrorRecovery::handle_edge_cases(f64::INFINITY);
    assert_eq!(inf_recovered, f32::MAX);
    
    let neg_inf_recovered = ConversionErrorRecovery::handle_edge_cases(f64::NEG_INFINITY);
    assert_eq!(neg_inf_recovered, f32::MIN);
    
    Ok(())
}

#[test]
fn test_conversion_with_missing_indicators() -> Result<()> {
    // Create data with varying indicator sets
    let mut data = Vec::new();
    let base_time = Utc::now();
    
    for i in 0..10 {
        let mut indicators = HashMap::new();
        
        // Not all points have all indicators
        if i % 2 == 0 {
            indicators.insert("rsi".to_string(), 50.0);
        }
        if i % 3 == 0 {
            indicators.insert("macd".to_string(), 0.001);
        }
        
        let point = TimeSeriesData {
            symbol: "TEST/USD".to_string(),
            timestamp: base_time + chrono::Duration::hours(i),
            open: 1000.0,
            high: 1010.0,
            low: 990.0,
            close: 1005.0,
            volume: vec![1000.0],
            indicators,
            source: Some("test".to_string()),
            entity: None,
            value: None,
            metadata: None,
        };
        
        data.push(point);
    }
    
    let converter = VendorFormatConverter::new();
    let result = converter.to_neuro_divergent_f32(&data, "TEST/USD")?;
    
    // Should handle missing indicators gracefully
    assert_eq!(result.len(), 10);
    
    Ok(())
}

#[test]
fn test_memory_efficiency() -> Result<()> {
    // Test that large conversions don't consume excessive memory
    let large_data = create_test_data(10000);
    
    let converter = VendorFormatConverter::new();
    
    // This should complete without memory issues
    let result = converter.to_neuro_divergent_f32(&large_data, "MEMORY/TEST")?;
    
    assert_eq!(result.len(), 10000);
    
    // Test streaming conversion for even larger datasets
    let streaming_converter = StreamingConverter::new(500);
    let streaming_result = streaming_converter.convert_chunked(&large_data, "STREAM/TEST")?;
    
    assert_eq!(streaming_result.values.len(), 10000);
    assert_eq!(streaming_result.timestamps.len(), 10000);
    
    Ok(())
}

#[test]
fn test_precision_validation() -> Result<()> {
    let converter = VendorFormatConverter::new();
    
    // Test cases where precision should be acceptable
    assert!(converter.type_converter.validate_precision(100.0, 100.01).is_ok());
    assert!(converter.type_converter.validate_precision(1000.0, 1000.1).is_ok());
    
    // Test cases where precision loss is too high (should fail with default settings)
    assert!(converter.type_converter.validate_precision(100.0, 110.0).is_err());
    assert!(converter.type_converter.validate_precision(1000.0, 1100.0).is_err());
    
    // Test with fast converter (more permissive)
    let fast_converter = VendorFormatConverter::with_fast_conversion();
    assert!(fast_converter.type_converter.validate_precision(100.0, 104.0).is_ok());
    
    Ok(())
}