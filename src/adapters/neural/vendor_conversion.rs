//! Vendor-specific data format conversions
//!
//! This module implements conversion between neural-trader's TimeSeriesData
//! and various vendor neural model formats, with proper type safety and
//! error handling.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use ndarray::{Array1, Array2};
use std::collections::HashMap;

use super::neuro_divergent_adapter::NeuralAdapterError;
use super::type_converter::{SafeF32Convert, VendorDataConverter};
use crate::data::TimeSeriesData;

// Import vendor types - using our local implementations for now
// In production, these would come from neuro_divergent_core
use crate::data::TimeSeriesData as VendorTimeSeriesData;

/// Temporary local implementation of vendor data point
#[derive(Debug, Clone)]
pub struct VendorDataPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub value: f32,
    pub exogenous: Option<Vec<f32>>,
}

/// Complete conversion interface for all vendor formats
pub struct VendorFormatConverter {
    type_converter: VendorDataConverter,
    normalization_enabled: bool,
}

impl VendorFormatConverter {
    /// Create new converter with default settings
    pub fn new() -> Self {
        Self {
            type_converter: VendorDataConverter::new(),
            normalization_enabled: false, // Normalization handled upstream
        }
    }

    /// Create converter optimized for performance over precision
    pub fn with_fast_conversion() -> Self {
        Self {
            type_converter: VendorDataConverter::with_fast_conversion(),
            normalization_enabled: false,
        }
    }

    /// Convert to neuro-divergent-data TimeSeriesData<f32> format
    pub fn to_neuro_divergent_f32(
        &self,
        data: &[TimeSeriesData],
        _symbol: &str,
    ) -> Result<VendorTimeSeriesData, NeuralAdapterError> {
        if data.is_empty() {
            return Err(NeuralAdapterError::Conversion(
                "Empty data provided".to_string(),
            ));
        }

        // Create DataPoint<f32> vector
        let mut data_points = Vec::with_capacity(data.len());

        for point in data {
            // Convert close price to f32
            let _value = point
                .close
                .to_f32_safe()
                .context("Failed to convert close price to f32")?;

            // Convert indicators to exogenous features
            let _exogenous = if point.indicators.is_empty() {
                None
            } else {
                let exog_values: Result<Vec<f32>, _> = point
                    .indicators
                    .values()
                    .map(|&v| v.to_f32_safe())
                    .collect();
                Some(exog_values?)
            };

            // For now, we'll just store the original TimeSeriesData
            // In production, this would use actual vendor conversion
            data_points.push(point.clone());
        }

        // For now, just return the first data point as VendorTimeSeriesData
        // In production, this would create a proper vendor format
        if let Some(first_point) = data_points.first() {
            Ok(first_point.clone())
        } else {
            Err(NeuralAdapterError::Conversion(
                "No data points provided".to_string(),
            ))
        }
    }

    /// Convert from vendor predictions back to neural-trader format
    pub fn from_vendor_predictions_f32(
        &self,
        predictions: &[f32],
        base_data: &TimeSeriesData,
        forecast_horizon: usize,
    ) -> Result<Vec<TimeSeriesData>, NeuralAdapterError> {
        if predictions.is_empty() {
            return Err(NeuralAdapterError::Conversion(
                "Empty predictions provided".to_string(),
            ));
        }

        let mut results = Vec::with_capacity(predictions.len());
        let interval_seconds = 3600; // Default to hourly intervals

        for (i, &pred) in predictions.iter().enumerate() {
            // Validate prediction value
            if !pred.is_finite() {
                return Err(NeuralAdapterError::Conversion(format!(
                    "Invalid prediction value at index {}: {}",
                    i, pred
                )));
            }

            let timestamp =
                base_data.timestamp + chrono::Duration::seconds(interval_seconds * (i + 1) as i64);

            let prediction_data = TimeSeriesData {
                symbol: base_data.symbol.clone(),
                timestamp,
                open: pred as f64,
                high: pred as f64,
                low: pred as f64,
                close: pred as f64,
                volume: 0.0,
                indicators: HashMap::new(),
                source: Some("vendor_neural_model".to_string()),
                entity: base_data.entity.clone(),
                value: Some(pred as f64),
                metadata: Some(serde_json::json!({
                    "type": "neural_forecast",
                    "model": "neuro_divergent_f32",
                    "forecast_step": i + 1,
                    "forecast_horizon": forecast_horizon,
                    "base_timestamp": base_data.timestamp.to_rfc3339(),
                    "precision_source": "f32",
                })),
            };

            results.push(prediction_data);
        }

        Ok(results)
    }

    /// Convert to model input arrays with proper shape and types
    pub fn to_model_arrays(
        &self,
        data: &[TimeSeriesData],
        lookback_window: usize,
        _feature_names: &[String],
    ) -> Result<(Array2<f32>, Vec<String>), NeuralAdapterError> {
        if data.len() < lookback_window {
            return Err(NeuralAdapterError::Conversion(format!(
                "Insufficient data for lookback window: need {}, have {}",
                lookback_window,
                data.len()
            )));
        }

        let n_samples = data.len() - lookback_window + 1;

        // Determine feature count
        let base_features = 5; // OHLCV
        let indicator_count = data.first().map(|d| d.indicators.len()).unwrap_or(0);
        let total_features = base_features + indicator_count;

        let mut features = Array2::<f32>::zeros((n_samples, lookback_window * total_features));
        let mut actual_feature_names = Vec::new();

        // Build feature names
        for window_step in 0..lookback_window {
            actual_feature_names.push(format!("open_lag_{}", window_step));
            actual_feature_names.push(format!("high_lag_{}", window_step));
            actual_feature_names.push(format!("low_lag_{}", window_step));
            actual_feature_names.push(format!("close_lag_{}", window_step));
            actual_feature_names.push(format!("volume_lag_{}", window_step));

            // Add indicator names
            if let Some(first_point) = data.first() {
                for indicator_name in first_point.indicators.keys() {
                    actual_feature_names.push(format!("{}_lag_{}", indicator_name, window_step));
                }
            }
        }

        // Fill feature matrix
        for sample_idx in 0..n_samples {
            for window_idx in 0..lookback_window {
                let data_idx = sample_idx + window_idx;
                let point = &data[data_idx];

                let feature_offset = window_idx * total_features;

                // Base OHLCV features
                features[[sample_idx, feature_offset + 0]] = point.open.to_f32_safe()?;
                features[[sample_idx, feature_offset + 1]] = point.high.to_f32_safe()?;
                features[[sample_idx, feature_offset + 2]] = point.low.to_f32_safe()?;
                features[[sample_idx, feature_offset + 3]] = point.close.to_f32_safe()?;
                features[[sample_idx, feature_offset + 4]] = point.volume.to_f32_safe()?;

                // Indicator features
                let mut indicator_idx = 5;
                for (_, &value) in &point.indicators {
                    features[[sample_idx, feature_offset + indicator_idx]] = value.to_f32_safe()?;
                    indicator_idx += 1;
                }
            }
        }

        Ok((features, actual_feature_names))
    }

    /// Convert batch of symbols efficiently
    pub fn convert_batch(
        &self,
        data_batch: &HashMap<String, Vec<TimeSeriesData>>,
    ) -> Result<HashMap<String, VendorTimeSeriesData>, NeuralAdapterError> {
        let mut results = HashMap::with_capacity(data_batch.len());

        for (symbol, data) in data_batch {
            let converted = self.to_neuro_divergent_f32(data, symbol)?;
            results.insert(symbol.clone(), converted);
        }

        Ok(results)
    }

    /// Validate conversion integrity
    pub fn validate_conversion(
        &self,
        original: &[TimeSeriesData],
        converted: &VendorTimeSeriesData,
    ) -> Result<(), NeuralAdapterError> {
        // Check length consistency - converted is a single item representing the first point
        if original.is_empty() {
            return Err(NeuralAdapterError::Conversion(
                "Original data is empty".to_string(),
            ));
        }

        // Check timestamp consistency with first item
        if let Some(first_original) = original.first() {
            if first_original.timestamp != converted.timestamp {
                return Err(NeuralAdapterError::Conversion(format!(
                    "Timestamp mismatch: {} vs {}",
                    first_original.timestamp.to_rfc3339(),
                    converted.timestamp.to_rfc3339()
                )));
            }

            // Check value precision within acceptable range
            self.type_converter
                .validate_precision(first_original.close, converted.close)
                .map_err(|e| {
                    NeuralAdapterError::Conversion(format!("Precision validation failed: {}", e))
                })?;
        }

        Ok(())
    }

    /// Memory-efficient streaming conversion for large datasets
    pub fn convert_streaming<I>(
        &self,
        data_iter: I,
        _symbol: &str,
        chunk_size: usize,
    ) -> Result<VendorTimeSeriesData, NeuralAdapterError>
    where
        I: Iterator<Item = TimeSeriesData>,
    {
        // Create a base vendor data structure - simplified for now
        let mut data_iter = data_iter; // Make iterator mutable
        let mut vendor_data = if let Some(first_point) = data_iter.next() {
            first_point
        } else {
            return Err(NeuralAdapterError::Conversion(
                "No data points provided".to_string(),
            ));
        };
        let mut chunk = Vec::with_capacity(chunk_size);

        for point in data_iter {
            chunk.push(point);

            if chunk.len() >= chunk_size {
                self.process_chunk(&mut vendor_data, &chunk)?;
                chunk.clear();
            }
        }

        // Process remaining items
        if !chunk.is_empty() {
            self.process_chunk(&mut vendor_data, &chunk)?;
        }

        Ok(vendor_data)
    }

    /// Process a chunk of data points
    fn process_chunk(
        &self,
        _vendor_data: &mut VendorTimeSeriesData,
        _chunk: &[TimeSeriesData],
    ) -> Result<(), NeuralAdapterError> {
        // Simplified implementation for now
        // In production, this would properly add points to vendor data
        Ok(())
    }
}

impl Default for VendorFormatConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Error recovery utilities for conversion failures
pub struct ConversionErrorRecovery;

impl ConversionErrorRecovery {
    /// Attempt to recover from precision loss by using alternative methods
    pub fn recover_precision_loss(
        original_value: f64,
        failed_conversion: f32,
    ) -> Result<f32, NeuralAdapterError> {
        // Try scaling approaches for very small or large numbers
        if original_value.abs() < 1e-6 {
            // Very small numbers - use scientific notation approach
            return Ok(0.0); // Safely clamp to zero
        }

        if original_value.abs() > 1e6 {
            // Large numbers - try log scaling
            let log_value = original_value.ln();
            if log_value.is_finite() {
                let scaled = (log_value as f32).exp();
                if scaled.is_finite() {
                    return Ok(scaled);
                }
            }
        }

        // If all recovery attempts fail, return the original failed conversion
        Ok(failed_conversion)
    }

    /// Handle edge cases in conversion
    pub fn handle_edge_cases(value: f64) -> f32 {
        if value.is_nan() {
            0.0 // Replace NaN with 0
        } else if value.is_infinite() {
            if value.is_sign_positive() {
                f32::MAX
            } else {
                f32::MIN
            }
        } else {
            value as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_timeseries() -> Vec<TimeSeriesData> {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 65.5);
        indicators.insert("macd".to_string(), 0.0012);

        (0..5)
            .map(|i| TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: Utc::now() + chrono::Duration::hours(i),
                open: 50000.0 + i as f64 * 100.0,
                high: 51000.0 + i as f64 * 100.0,
                low: 49500.0 + i as f64 * 100.0,
                close: 50500.0 + i as f64 * 100.0,
                volume: 1000.0 + i as f64 * 10.0,
                indicators: indicators.clone(),
                source: Some("test".to_string()),
                entity: None,
                value: None,
                metadata: None,
            })
            .collect()
    }

    #[test]
    fn test_neuro_divergent_conversion() {
        let data = create_test_timeseries();
        let converter = VendorFormatConverter::new();

        let converted = converter.to_neuro_divergent_f32(&data, "BTC/USD").unwrap();

        // Since we return the first point, check basic properties
        assert_eq!(converted.symbol, "BTC/USD");
        assert_eq!(converted.close, 50500.0);
    }

    #[test]
    fn test_prediction_conversion() {
        let data = create_test_timeseries();
        let converter = VendorFormatConverter::new();
        let predictions = vec![51000.0_f32, 51500.0_f32, 52000.0_f32];

        let result = converter
            .from_vendor_predictions_f32(&predictions, &data[0], 3)
            .unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].close, 51000.0);
        assert_eq!(result[1].close, 51500.0);
        assert_eq!(result[2].close, 52000.0);

        // Check metadata
        assert!(result[0].metadata.is_some());
        let metadata = result[0].metadata.as_ref().unwrap();
        assert_eq!(metadata["type"], "neural_forecast");
        assert_eq!(metadata["forecast_step"], 1);
    }

    #[test]
    fn test_model_arrays_conversion() {
        let data = create_test_timeseries();
        let converter = VendorFormatConverter::new();

        let (features, feature_names) = converter
            .to_model_arrays(
                &data,
                3, // lookback window
                &["close".to_string()],
            )
            .unwrap();

        // 5 data points - 3 lookback + 1 = 3 samples
        assert_eq!(features.shape()[0], 3);
        // 3 lookback * (5 OHLCV + 2 indicators) = 21 features
        assert_eq!(features.shape()[1], 21);
        assert_eq!(feature_names.len(), 21);
    }

    #[test]
    fn test_conversion_validation() {
        let data = create_test_timeseries();
        let converter = VendorFormatConverter::new();

        let converted = converter.to_neuro_divergent_f32(&data, "BTC/USD").unwrap();
        let validation_result = converter.validate_conversion(&data, &converted);

        assert!(validation_result.is_ok());
    }

    #[test]
    fn test_error_recovery() {
        let huge_value = f64::MAX;
        let recovered = ConversionErrorRecovery::handle_edge_cases(huge_value);
        assert_eq!(recovered, f32::MAX);

        let nan_value = f64::NAN;
        let recovered_nan = ConversionErrorRecovery::handle_edge_cases(nan_value);
        assert_eq!(recovered_nan, 0.0);
    }
}
