//! Input conversion implementation for FANN predictor  
//!
//! This module handles the conversion of time series data into neural network
//! input format with proper normalization and feature engineering.

use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, warn};

use super::{
    ConversionConfig, ConversionError, NormalizationMethod, NormalizationStats, 
    DataConverter, utils
};
use crate::data::TimeSeriesData;
use crate::neural::PredictionResult;

/// Input data converter for neural networks
pub struct InputConverter {
    /// Conversion configuration
    config: ConversionConfig,
    /// Normalization statistics for features
    stats: NormalizationStats,
    /// Feature extraction configuration
    feature_config: FeatureExtractionConfig,
    /// Historical data for calculating features
    historical_buffer: Vec<TimeSeriesData>,
    /// Maximum buffer size for historical data
    max_buffer_size: usize,
}

/// Configuration for feature extraction
#[derive(Debug, Clone)]
pub struct FeatureExtractionConfig {
    /// Include price features (OHLC)
    pub include_prices: bool,
    /// Include volume features
    pub include_volume: bool,
    /// Include technical indicators
    pub include_indicators: bool,
    /// Include price changes/returns
    pub include_returns: bool,
    /// Include moving averages
    pub include_moving_averages: bool,
    /// Moving average periods
    pub ma_periods: Vec<usize>,
    /// Include volatility features
    pub include_volatility: bool,
    /// Volatility calculation window
    pub volatility_window: usize,
    /// Number of time lags to include
    pub time_lags: usize,
}

impl Default for FeatureExtractionConfig {
    fn default() -> Self {
        Self {
            include_prices: true,
            include_volume: true,
            include_indicators: true,
            include_returns: true,
            include_moving_averages: true,
            ma_periods: vec![5, 10, 20],
            include_volatility: true,
            volatility_window: 20,
            time_lags: 5,
        }
    }
}

impl InputConverter {
    /// Create a new input converter
    pub fn new(config: ConversionConfig) -> Self {
        let feature_config = FeatureExtractionConfig::default();
        let feature_count = Self::calculate_feature_count(&feature_config);
        
        Self {
            config,
            stats: NormalizationStats::new(feature_count),
            feature_config,
            historical_buffer: Vec::new(),
            max_buffer_size: 1000, // Keep last 1000 data points for features
        }
    }

    /// Create with custom feature configuration
    pub fn with_feature_config(config: ConversionConfig, feature_config: FeatureExtractionConfig) -> Self {
        let feature_count = Self::calculate_feature_count(&feature_config);
        
        Self {
            config,
            stats: NormalizationStats::new(feature_count),
            feature_config,
            historical_buffer: Vec::new(),
            max_buffer_size: 1000,
        }
    }

    /// Update the converter with historical data for better normalization
    pub fn update_with_data(&mut self, data: &[TimeSeriesData]) -> Result<(), ConversionError> {
        // Add to historical buffer
        for item in data {
            if self.historical_buffer.len() >= self.max_buffer_size {
                self.historical_buffer.remove(0);
            }
            self.historical_buffer.push(item.clone());
        }

        // Extract features for normalization statistics
        let feature_vectors = self.extract_features_batch(&self.historical_buffer)?;
        
        // Convert to f64 for statistics calculation
        let f64_vectors: Vec<Vec<f64>> = feature_vectors
            .iter()
            .map(|v| v.iter().map(|&x| x as f64).collect())
            .collect();

        // Update normalization statistics
        self.stats.update(&f64_vectors)?;

        debug!("Updated input converter with {} data points, buffer size: {}", 
               data.len(), self.historical_buffer.len());

        Ok(())
    }

    /// Extract features from a single data point with context
    pub fn extract_features(&self, data: &[TimeSeriesData], index: usize) -> Result<Vec<f32>, ConversionError> {
        if index >= data.len() {
            return Err(ConversionError::InvalidInput(
                format!("Index {} out of bounds for data length {}", index, data.len())
            ));
        }

        let current = &data[index];
        let mut features = Vec::new();

        // Basic OHLC features
        if self.feature_config.include_prices {
            features.extend_from_slice(&[
                current.open as f32,
                current.high as f32,
                current.low as f32,
                current.close as f32,
            ]);
        }

        // Volume features
        if self.feature_config.include_volume {
            features.push(current.volume as f32);
            
            // Volume-weighted average price (VWAP) approximation
            if current.volume > 0.0 {
                let vwap = (current.high + current.low + current.close) / 3.0;
                features.push(vwap as f32);
            } else {
                features.push(current.close as f32);
            }
        }

        // Technical indicators
        if self.feature_config.include_indicators {
            for (name, value) in &current.indicators {
                features.push(*value as f32);
            }
        }

        // Price returns and changes
        if self.feature_config.include_returns && index > 0 {
            let prev = &data[index - 1];
            
            // Simple return
            let simple_return = utils::percentage_change(current.close, prev.close);
            features.push(simple_return as f32);
            
            // Log return
            let log_return = utils::log_return(current.close, prev.close);
            features.push(log_return as f32);
            
            // High-low range
            let hl_range = (current.high - current.low) / current.close;
            features.push(hl_range as f32);
        } else if self.feature_config.include_returns {
            // Fill with zeros for first data point
            features.extend_from_slice(&[0.0, 0.0, 0.0]);
        }

        // Moving averages
        if self.feature_config.include_moving_averages {
            for &period in &self.feature_config.ma_periods {
                let ma = self.calculate_moving_average(data, index, period);
                features.push(ma as f32);
                
                // MA deviation
                let ma_dev = (current.close - ma) / ma.max(1e-8);
                features.push(ma_dev as f32);
            }
        }

        // Volatility features
        if self.feature_config.include_volatility {
            let volatility = self.calculate_volatility(data, index, self.feature_config.volatility_window);
            features.push(volatility as f32);
            
            // Volatility-adjusted return
            if index > 0 && volatility > 0.0 {
                let prev = &data[index - 1];
                let vol_adj_return = utils::percentage_change(current.close, prev.close) / volatility;
                features.push(vol_adj_return as f32);
            } else {
                features.push(0.0);
            }
        }

        // Time lag features
        if self.feature_config.time_lags > 0 {
            for lag in 1..=self.feature_config.time_lags {
                if index >= lag {
                    let lagged = &data[index - lag];
                    let lag_return = utils::percentage_change(current.close, lagged.close);
                    features.push(lag_return as f32);
                } else {
                    features.push(0.0);
                }
            }
        }

        // Validate feature count
        let expected_count = self.feature_count();
        if features.len() != expected_count {
            return Err(ConversionError::FeatureScalingError(
                format!("Feature count mismatch: expected {}, got {}", 
                        expected_count, features.len())
            ));
        }

        Ok(features)
    }

    /// Extract features from multiple data points
    pub fn extract_features_batch(&self, data: &[TimeSeriesData]) -> Result<Vec<Vec<f32>>, ConversionError> {
        let mut feature_batch = Vec::new();
        
        for i in 0..data.len() {
            let features = self.extract_features(data, i)?;
            feature_batch.push(features);
        }

        Ok(feature_batch)
    }

    /// Apply normalization to features
    pub fn normalize_features(&self, features: &[f32]) -> Result<Vec<f32>, ConversionError> {
        if !self.stats.is_valid() {
            return Err(ConversionError::NormalizationError(
                "Normalization statistics not properly initialized".to_string()
            ));
        }

        if features.len() != self.stats.min_values.len() {
            return Err(ConversionError::NormalizationError(
                format!("Feature dimension mismatch: expected {}, got {}", 
                        self.stats.min_values.len(), features.len())
            ));
        }

        let normalized = match self.config.normalization_method {
            NormalizationMethod::MinMax => {
                features.iter().enumerate().map(|(i, &value)| {
                    let normalized = utils::min_max_normalize(
                        value as f64, 
                        self.stats.min_values[i], 
                        self.stats.max_values[i]
                    );
                    utils::sanitize_value(normalized, 0.5) as f32
                }).collect()
            },
            NormalizationMethod::ZScore => {
                features.iter().enumerate().map(|(i, &value)| {
                    let normalized = utils::z_score_normalize(
                        value as f64,
                        self.stats.mean_values[i],
                        self.stats.std_values[i]
                    );
                    utils::sanitize_value(normalized, 0.0) as f32
                }).collect()
            },
            NormalizationMethod::Robust => {
                features.iter().enumerate().map(|(i, &value)| {
                    let normalized = utils::robust_scale(
                        value as f64,
                        self.stats.median_values[i],
                        self.stats.iqr_values[i]
                    );
                    utils::sanitize_value(normalized, 0.0) as f32
                }).collect()
            },
            NormalizationMethod::None => features.to_vec(),
        };

        Ok(normalized)
    }

    /// Calculate moving average at a specific index
    fn calculate_moving_average(&self, data: &[TimeSeriesData], index: usize, period: usize) -> f64 {
        let start = if index + 1 >= period { index + 1 - period } else { 0 };
        let end = index + 1;
        
        let sum: f64 = data[start..end].iter().map(|d| d.close).sum();
        let count = end - start;
        
        if count > 0 {
            sum / count as f64
        } else {
            data[index].close
        }
    }

    /// Calculate volatility at a specific index
    fn calculate_volatility(&self, data: &[TimeSeriesData], index: usize, window: usize) -> f64 {
        let start = if index + 1 >= window { index + 1 - window } else { 0 };
        let end = index + 1;
        
        if end - start < 2 {
            return 0.05; // Default volatility
        }

        let returns: Vec<f64> = data[start..end]
            .windows(2)
            .map(|w| utils::log_return(w[1].close, w[0].close))
            .collect();

        if returns.is_empty() {
            return 0.05;
        }

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;

        variance.sqrt()
    }

    /// Calculate expected feature count
    fn calculate_feature_count(config: &FeatureExtractionConfig) -> usize {
        let mut count = 0;

        if config.include_prices {
            count += 4; // OHLC
        }

        if config.include_volume {
            count += 2; // Volume + VWAP
        }

        // Technical indicators count is dynamic, use reasonable default
        if config.include_indicators {
            count += 3; // Common indicators: RSI, MACD, etc.
        }

        if config.include_returns {
            count += 3; // Simple return, log return, HL range
        }

        if config.include_moving_averages {
            count += config.ma_periods.len() * 2; // MA + MA deviation for each period
        }

        if config.include_volatility {
            count += 2; // Volatility + volatility-adjusted return
        }

        if config.time_lags > 0 {
            count += config.time_lags; // One feature per lag
        }

        count
    }

    /// Get feature names for debugging
    pub fn get_feature_names(&self) -> Vec<String> {
        let mut names = Vec::new();

        if self.feature_config.include_prices {
            names.extend_from_slice(&["open".to_string(), "high".to_string(), "low".to_string(), "close".to_string()]);
        }

        if self.feature_config.include_volume {
            names.extend_from_slice(&["volume".to_string(), "vwap".to_string()]);
        }

        if self.feature_config.include_indicators {
            names.extend_from_slice(&["rsi".to_string(), "macd".to_string(), "signal".to_string()]);
        }

        if self.feature_config.include_returns {
            names.extend_from_slice(&["simple_return".to_string(), "log_return".to_string(), "hl_range".to_string()]);
        }

        if self.feature_config.include_moving_averages {
            for &period in &self.feature_config.ma_periods {
                names.push(format!("ma_{}", period));
                names.push(format!("ma_{}_dev", period));
            }
        }

        if self.feature_config.include_volatility {
            names.extend_from_slice(&["volatility".to_string(), "vol_adj_return".to_string()]);
        }

        if self.feature_config.time_lags > 0 {
            for lag in 1..=self.feature_config.time_lags {
                names.push(format!("lag_{}_return", lag));
            }
        }

        names
    }

    /// Get current configuration
    pub fn config(&self) -> &ConversionConfig {
        &self.config
    }

    /// Get feature extraction configuration
    pub fn feature_config(&self) -> &FeatureExtractionConfig {
        &self.feature_config
    }

    /// Check if normalization statistics are ready
    pub fn is_ready(&self) -> bool {
        self.stats.is_valid()
    }
}

impl DataConverter for InputConverter {
    fn convert_input(&self, data: &[TimeSeriesData]) -> Result<Vec<Vec<f32>>, ConversionError> {
        self.validate_input(data)?;

        let mut converted_inputs = Vec::new();
        
        for i in 0..data.len() {
            let features = self.extract_features(data, i)?;
            let normalized_features = if self.is_ready() {
                self.normalize_features(&features)?
            } else {
                warn!("Normalization statistics not ready, using raw features");
                features
            };
            
            converted_inputs.push(normalized_features);
        }

        debug!("Converted {} data points to {} features each", 
               data.len(), self.feature_count());

        Ok(converted_inputs)
    }

    fn convert_output(&self, _outputs: &[f32], _base_data: &TimeSeriesData) -> Result<Vec<PredictionResult>, ConversionError> {
        // Input converter doesn't handle output conversion
        Err(ConversionError::OutputTransformError(
            "Input converter cannot convert outputs".to_string()
        ))
    }

    fn validate_input(&self, data: &[TimeSeriesData]) -> Result<(), ConversionError> {
        if !self.config.validate_data {
            return Ok(());
        }

        if data.is_empty() {
            return Err(ConversionError::ValidationError("Empty input data".to_string()));
        }

        for (i, item) in data.iter().enumerate() {
            // Validate price data
            if !utils::is_valid_value(item.open) || !utils::is_valid_value(item.high) ||
               !utils::is_valid_value(item.low) || !utils::is_valid_value(item.close) {
                return Err(ConversionError::ValidationError(
                    format!("Invalid price data at index {}", i)
                ));
            }

            // Validate price relationships
            if item.high < item.low || item.high < item.open || item.high < item.close ||
               item.low > item.open || item.low > item.close {
                return Err(ConversionError::ValidationError(
                    format!("Invalid OHLC relationships at index {}", i)
                ));
            }

            // Validate volume
            if !utils::is_valid_value(item.volume) || item.volume < 0.0 {
                return Err(ConversionError::ValidationError(
                    format!("Invalid volume data at index {}", i)
                ));
            }

            // Validate indicators
            for (name, value) in &item.indicators {
                if !utils::is_valid_value(*value) {
                    return Err(ConversionError::ValidationError(
                        format!("Invalid indicator '{}' at index {}: {}", name, i, value)
                    ));
                }
            }
        }

        Ok(())
    }

    fn feature_count(&self) -> usize {
        Self::calculate_feature_count(&self.feature_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_data() -> Vec<TimeSeriesData> {
        vec![
            TimeSeriesData {
                timestamp: Utc::now(),
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 102.0,
                volume: 1000.0,
                indicators: {
                    let mut map = HashMap::new();
                    map.insert("rsi".to_string(), 50.0);
                    map
                },
            },
            TimeSeriesData {
                timestamp: Utc::now(),
                open: 102.0,
                high: 108.0,
                low: 100.0,
                close: 105.0,
                volume: 1200.0,
                indicators: {
                    let mut map = HashMap::new();
                    map.insert("rsi".to_string(), 55.0);
                    map
                },
            },
        ]
    }

    #[test]
    fn test_input_converter_creation() {
        let config = ConversionConfig::default();
        let converter = InputConverter::new(config);
        
        assert!(converter.feature_count() > 0);
        assert!(!converter.is_ready()); // No data yet
    }

    #[test]
    fn test_feature_extraction() {
        let config = ConversionConfig::default();
        let converter = InputConverter::new(config);
        let data = create_test_data();
        
        let features = converter.extract_features(&data, 1).unwrap();
        assert_eq!(features.len(), converter.feature_count());
    }

    #[test]
    fn test_data_validation() {
        let config = ConversionConfig::default();
        let converter = InputConverter::new(config);
        let data = create_test_data();
        
        assert!(converter.validate_input(&data).is_ok());
        
        // Test invalid data
        let mut invalid_data = data.clone();
        invalid_data[0].high = invalid_data[0].low - 1.0; // Invalid OHLC
        
        assert!(converter.validate_input(&invalid_data).is_err());
    }

    #[test]
    fn test_batch_conversion() {
        let config = ConversionConfig::default();
        let mut converter = InputConverter::new(config);
        let data = create_test_data();
        
        // Update with data for normalization
        converter.update_with_data(&data).unwrap();
        
        let converted = converter.convert_input(&data).unwrap();
        assert_eq!(converted.len(), data.len());
        assert_eq!(converted[0].len(), converter.feature_count());
    }

    #[test]
    fn test_feature_names() {
        let config = ConversionConfig::default();
        let converter = InputConverter::new(config);
        
        let names = converter.get_feature_names();
        assert_eq!(names.len(), converter.feature_count());
        assert!(names.contains(&"close".to_string()));
        assert!(names.contains(&"volume".to_string()));
    }
}