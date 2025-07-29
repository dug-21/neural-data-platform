//! Type conversion utilities for neural models
//!
//! This module provides safe conversion between f64 and f32 types,
//! handling precision loss and overflow conditions gracefully.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use super::neuro_divergent_adapter::NeuralAdapterError;
use crate::data::TimeSeriesData;

/// Temporary implementation of RuvSwarmTimeSeriesData for vendor integration
#[derive(Debug, Clone)]
pub struct RuvSwarmTimeSeriesData {
    pub values: Vec<f32>,
    pub timestamps: Vec<f64>,
    pub frequency: String,
    pub unique_id: String,
}

/// Temporary implementation of NeuralDataPoint for vendor integration
#[derive(Debug, Clone)]
pub struct NeuralDataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f32,
    pub exogenous: Option<Vec<f32>>,
}

/// Safe conversion from f64 to f32 with overflow/underflow handling
pub trait SafeF32Convert {
    fn to_f32_safe(&self) -> Result<f32, NeuralAdapterError>;
}

impl SafeF32Convert for f64 {
    fn to_f32_safe(&self) -> Result<f32, NeuralAdapterError> {
        // Handle special values
        if self.is_nan() {
            return Ok(f32::NAN);
        }
        if self.is_infinite() {
            return Ok(if self.is_sign_positive() {
                f32::INFINITY
            } else {
                f32::NEG_INFINITY
            });
        }

        // Check for overflow/underflow
        if *self > f32::MAX as f64 {
            return Ok(f32::MAX);
        }
        if *self < f32::MIN as f64 {
            return Ok(f32::MIN);
        }

        // Check for subnormal numbers that might lose precision
        let result = *self as f32;
        if result == 0.0 && *self != 0.0 && self.abs() < f32::MIN_POSITIVE as f64 {
            // Value is too small to represent as f32, clamp to 0
            return Ok(0.0);
        }

        Ok(result)
    }
}

/// Convert vector of f64 to f32 with error handling
pub fn convert_f64_vec_to_f32(values: &[f64]) -> Result<Vec<f32>, NeuralAdapterError> {
    values.iter().map(|&v| v.to_f32_safe()).collect()
}

/// Convert HashMap of indicators from f64 to f32
pub fn convert_indicators_to_f32(
    indicators: &HashMap<String, f64>,
) -> Result<HashMap<String, f32>, NeuralAdapterError> {
    indicators
        .iter()
        .map(|(key, &value)| value.to_f32_safe().map(|v| (key.clone(), v)))
        .collect()
}

/// Vendor-specific data converter for different neural model formats
pub struct VendorDataConverter {
    /// Whether to preserve precision when possible
    preserve_precision: bool,
    /// Maximum acceptable precision loss percentage
    max_precision_loss: f64,
}

impl VendorDataConverter {
    /// Create a new converter with default settings
    pub fn new() -> Self {
        Self {
            preserve_precision: true,
            max_precision_loss: 0.01, // 1% max precision loss
        }
    }

    /// Create converter that allows more precision loss for performance
    pub fn with_fast_conversion() -> Self {
        Self {
            preserve_precision: false,
            max_precision_loss: 0.05, // 5% max precision loss
        }
    }

    /// Validate precision loss between original and converted values
    pub fn validate_precision(
        &self,
        original: f64,
        converted: f64,
    ) -> Result<(), NeuralAdapterError> {
        if original == 0.0 && converted == 0.0 {
            return Ok(());
        }

        if original == 0.0 {
            // Original is zero but converted is not
            if converted.abs() > f64::EPSILON {
                return Err(NeuralAdapterError::Conversion(format!(
                    "Non-zero conversion from zero: {}",
                    converted
                )));
            }
            return Ok(());
        }

        let precision_loss = (original - converted).abs() / original.abs();
        if precision_loss > self.max_precision_loss {
            return Err(NeuralAdapterError::Conversion(format!(
                "Precision loss {:.4} exceeds maximum {:.4} (original: {}, converted: {})",
                precision_loss, self.max_precision_loss, original, converted
            )));
        }

        Ok(())
    }

    /// Convert TimeSeriesData to ruv-swarm-ml TimeSeriesData format
    pub fn to_ruv_swarm_format(
        &self,
        data: &[TimeSeriesData],
        symbol: &str,
    ) -> Result<RuvSwarmTimeSeriesData, NeuralAdapterError> {
        if data.is_empty() {
            return Err(NeuralAdapterError::Conversion(
                "Empty data provided".to_string(),
            ));
        }

        // Convert close prices to f32 values
        let values = data
            .iter()
            .map(|d| d.close.to_f32_safe())
            .collect::<Result<Vec<f32>, _>>()?;

        // Convert timestamps to f64 (Unix timestamps)
        let timestamps: Vec<f64> = data
            .iter()
            .map(|d| d.timestamp.timestamp() as f64)
            .collect();

        // Determine frequency from time differences
        let frequency = self
            .infer_frequency(data)
            .unwrap_or_else(|| "1H".to_string());

        Ok(RuvSwarmTimeSeriesData {
            values,
            timestamps,
            frequency,
            unique_id: symbol.to_string(),
        })
    }

    /// Convert to neuro-divergent DataPoint format
    pub fn to_neuro_divergent_datapoints(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<Vec<NeuralDataPoint>, NeuralAdapterError> {
        data.iter()
            .map(|point| {
                let value = point.close.to_f32_safe()?;

                // Convert indicators to exogenous features
                let exogenous = if point.indicators.is_empty() {
                    None
                } else {
                    let exog_values = point
                        .indicators
                        .values()
                        .map(|&v| v.to_f32_safe())
                        .collect::<Result<Vec<f32>, _>>()?;
                    Some(exog_values)
                };

                Ok(NeuralDataPoint {
                    timestamp: point.timestamp,
                    value,
                    exogenous,
                })
            })
            .collect()
    }

    /// Convert prediction results back to TimeSeriesData format
    pub fn from_f32_predictions(
        &self,
        predictions: &[f32],
        base_data: &TimeSeriesData,
        start_offset: i64,
        interval_seconds: i64,
    ) -> Vec<TimeSeriesData> {
        predictions
            .iter()
            .enumerate()
            .map(|(i, &pred)| {
                let timestamp = base_data.timestamp
                    + chrono::Duration::seconds(start_offset + (interval_seconds * i as i64));

                TimeSeriesData {
                    symbol: base_data.symbol.clone(),
                    timestamp,
                    open: pred as f64,
                    high: pred as f64,
                    low: pred as f64,
                    close: pred as f64,
                    volume: 0.0,
                    indicators: HashMap::new(),
                    source: Some("neural_prediction_f32".to_string()),
                    entity: base_data.entity.clone(),
                    value: Some(pred as f64),
                    metadata: Some(serde_json::json!({
                        "type": "forecast",
                        "model": "vendor_model",
                        "precision": "f32_converted",
                        "base_timestamp": base_data.timestamp.to_rfc3339(),
                    })),
                }
            })
            .collect()
    }

    /// Infer frequency from timestamp differences
    fn infer_frequency(&self, data: &[TimeSeriesData]) -> Option<String> {
        if data.len() < 2 {
            return None;
        }

        let diff_seconds = (data[1].timestamp - data[0].timestamp).num_seconds();

        match diff_seconds {
            60 => Some("1T".to_string()),    // 1 minute
            300 => Some("5T".to_string()),   // 5 minutes
            900 => Some("15T".to_string()),  // 15 minutes
            3600 => Some("1H".to_string()),  // 1 hour
            14400 => Some("4H".to_string()), // 4 hours
            86400 => Some("1D".to_string()), // 1 day
            _ => Some("1H".to_string()),     // Default to hourly
        }
    }

    /// Validate conversion precision for f32 conversions
    pub fn validate_precision_f32(
        &self,
        original: f64,
        converted: f32,
    ) -> Result<(), NeuralAdapterError> {
        if !self.preserve_precision {
            return Ok(());
        }

        let reconverted = converted as f64;
        let precision_loss = ((original - reconverted).abs() / original.abs()).abs();

        if precision_loss > self.max_precision_loss {
            return Err(NeuralAdapterError::Conversion(format!(
                "Precision loss too high: {:.4}% (max allowed: {:.2}%)",
                precision_loss * 100.0,
                self.max_precision_loss * 100.0
            )));
        }

        Ok(())
    }
}

impl Default for VendorDataConverter {
    fn default() -> Self {
        Self::new()
    }
}

// Duplicate struct definitions removed - using the structs defined at the beginning of the file

/// Batch conversion utilities
pub struct BatchConverter;

impl BatchConverter {
    /// Convert multiple symbols worth of data efficiently
    pub fn convert_multi_symbol(
        data_by_symbol: &HashMap<String, Vec<TimeSeriesData>>,
        converter: &VendorDataConverter,
    ) -> Result<HashMap<String, RuvSwarmTimeSeriesData>, NeuralAdapterError> {
        data_by_symbol
            .iter()
            .map(|(symbol, data)| {
                converter
                    .to_ruv_swarm_format(data, symbol)
                    .map(|converted| (symbol.clone(), converted))
            })
            .collect()
    }

    /// Verify conversion integrity for all symbols
    pub fn verify_conversions(
        original: &HashMap<String, Vec<TimeSeriesData>>,
        converted: &HashMap<String, RuvSwarmTimeSeriesData>,
    ) -> Result<(), NeuralAdapterError> {
        for (symbol, orig_data) in original {
            let conv_data = converted.get(symbol).ok_or_else(|| {
                NeuralAdapterError::Conversion(format!("Missing conversion for symbol: {}", symbol))
            })?;

            if orig_data.len() != conv_data.values.len() {
                return Err(NeuralAdapterError::Conversion(format!(
                    "Length mismatch for {}: {} != {}",
                    symbol,
                    orig_data.len(),
                    conv_data.values.len()
                )));
            }
        }
        Ok(())
    }
}

/// Memory-efficient streaming converter for large datasets
pub struct StreamingConverter {
    converter: VendorDataConverter,
    chunk_size: usize,
}

impl StreamingConverter {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            converter: VendorDataConverter::new(),
            chunk_size,
        }
    }

    /// Convert data in chunks to manage memory usage
    pub fn convert_chunked(
        &self,
        data: &[TimeSeriesData],
        symbol: &str,
    ) -> Result<RuvSwarmTimeSeriesData, NeuralAdapterError> {
        if data.len() <= self.chunk_size {
            return self.converter.to_ruv_swarm_format(data, symbol);
        }

        let mut all_values = Vec::with_capacity(data.len());
        let mut all_timestamps = Vec::with_capacity(data.len());

        for chunk in data.chunks(self.chunk_size) {
            let chunk_converted = self.converter.to_ruv_swarm_format(chunk, symbol)?;
            all_values.extend(chunk_converted.values);
            all_timestamps.extend(chunk_converted.timestamps);
        }

        // Use frequency from first chunk
        let frequency = self
            .converter
            .infer_frequency(data)
            .unwrap_or_else(|| "1H".to_string());

        Ok(RuvSwarmTimeSeriesData {
            values: all_values,
            timestamps: all_timestamps,
            frequency,
            unique_id: symbol.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_data() -> Vec<TimeSeriesData> {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 65.5);
        indicators.insert("macd".to_string(), 0.0012);

        vec![
            TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: Utc::now(),
                open: 50000.0,
                high: 51000.0,
                low: 49500.0,
                close: 50500.0,
                volume: 1000.0,
                indicators: indicators.clone(),
                source: Some("test".to_string()),
                entity: None,
                value: None,
                metadata: None,
            },
            TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: Utc::now() + chrono::Duration::hours(1),
                open: 50500.0,
                high: 51500.0,
                low: 50000.0,
                close: 51000.0,
                volume: 1100.0,
                indicators,
                source: Some("test".to_string()),
                entity: None,
                value: None,
                metadata: None,
            },
        ]
    }

    #[test]
    fn test_f64_to_f32_conversion() {
        let value = 12345.6789_f64;
        let converted = value.to_f32_safe().unwrap();
        assert!((converted - 12345.679_f32).abs() < 0.001);
    }

    #[test]
    fn test_overflow_handling() {
        let huge_value = f64::MAX;
        let converted = huge_value.to_f32_safe().unwrap();
        assert_eq!(converted, f32::MAX);
    }

    #[test]
    fn test_ruv_swarm_conversion() {
        let data = create_test_data();
        let converter = VendorDataConverter::new();

        let result = converter.to_ruv_swarm_format(&data, "BTC/USD").unwrap();

        assert_eq!(result.values.len(), 2);
        assert_eq!(result.timestamps.len(), 2);
        assert_eq!(result.unique_id, "BTC/USD");
        assert_eq!(result.values[0], 50500.0_f32);
        assert_eq!(result.values[1], 51000.0_f32);
    }

    #[test]
    fn test_neural_datapoint_conversion() {
        let data = create_test_data();
        let converter = VendorDataConverter::new();

        let result = converter.to_neuro_divergent_datapoints(&data).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value, 50500.0_f32);
        assert!(result[0].exogenous.is_some());
        assert_eq!(result[0].exogenous.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_precision_validation() {
        let converter = VendorDataConverter::new();

        // Small precision loss should pass
        assert!(converter.validate_precision(100.0, 100.001).is_ok());

        // Large precision loss should fail
        assert!(converter.validate_precision(100.0, 110.0).is_err());
    }

    #[test]
    fn test_streaming_conversion() {
        let data = create_test_data();
        let converter = StreamingConverter::new(1);

        let result = converter.convert_chunked(&data, "BTC/USD").unwrap();

        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0], 50500.0_f32);
        assert_eq!(result.values[1], 51000.0_f32);
    }
}
