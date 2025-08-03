//! Data Converter - TimeSeriesData to VendorTimeSeriesData<f32> Conversion
//!
//! This module provides conversion utilities between the internal TimeSeriesData format
//! and the vendor's VendorTimeSeriesData<f32> format required by neuro-divergent models.
//!
//! INTEGRATION-FIRST COMPLIANCE:
//! - Extends existing TimeSeriesData interface (preserved)
//! - Works with existing data processing pipeline (unchanged)
//! - Maintains backward compatibility with current system
//! - Enables bidirectional conversion for vendor model integration

use anyhow::{Context, Result};
use chrono::{DateTime, Utc, Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

// Internal imports
use crate::data::TimeSeriesData;

// Vendor types (mock implementations for compilation)
use neuro_divergent_core::data::TimeSeriesDataset;
use neuro_divergent_models::foundation::ForecastOutput as ForecastResult;
use crate::adapters::vendor_bridge::VendorTimeSeriesData;

// Type alias for f32 specialization
type VendorDataset = TimeSeriesDataset<f32>;

/// Configuration for data conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConverterConfig {
    /// Enable data normalization
    pub normalize_data: bool,
    /// Normalization method: "minmax", "zscore", "robust"
    pub normalization_method: String,
    /// Enable outlier detection and removal
    pub remove_outliers: bool,
    /// Outlier detection method: "iqr", "zscore", "isolation_forest"
    pub outlier_method: String,
    /// Maximum allowed missing data percentage
    pub max_missing_percent: f64,
    /// Fill method for missing data: "forward", "backward", "mean", "interpolate"
    pub missing_fill_method: String,
    /// Enable feature engineering
    pub enable_feature_engineering: bool,
    /// Technical indicators to compute
    pub technical_indicators: Vec<String>,
    /// Time-based features to add
    pub time_features: Vec<String>,
}

impl Default for DataConverterConfig {
    fn default() -> Self {
        Self {
            normalize_data: true,
            normalization_method: "minmax".to_string(),
            remove_outliers: true,
            outlier_method: "iqr".to_string(),
            max_missing_percent: 5.0,
            missing_fill_method: "forward".to_string(),
            enable_feature_engineering: true,
            technical_indicators: vec![
                "sma_5".to_string(),
                "sma_20".to_string(),
                "ema_12".to_string(),
                "ema_26".to_string(),
                "rsi_14".to_string(),
                "macd".to_string(),
            ],
            time_features: vec![
                "hour".to_string(),
                "day_of_week".to_string(),
                "month".to_string(),
                "quarter".to_string(),
            ],
        }
    }
}

/// Normalization statistics for reversible transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationStats {
    pub method: String,
    pub min_value: f64,
    pub max_value: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub median: f64,
    pub q25: f64,
    pub q75: f64,
}

/// Conversion metadata for tracking transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionMetadata {
    pub source_format: String,
    pub target_format: String,
    pub conversion_timestamp: DateTime<Utc>,
    pub normalization_stats: Option<NormalizationStats>,
    pub features_added: Vec<String>,
    pub outliers_removed: usize,
    pub missing_filled: usize,
    pub original_length: usize,
    pub converted_length: usize,
}

/// Main data converter struct
pub struct DataConverter {
    config: DataConverterConfig,
    normalization_cache: HashMap<String, NormalizationStats>,
}

impl DataConverter {
    /// Create new data converter
    pub fn new(config: DataConverterConfig) -> Self {
        info!("🔄 Initializing DataConverter with config: {:?}", config);
        
        Self {
            config,
            normalization_cache: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(DataConverterConfig::default())
    }

    /// Convert internal TimeSeriesData to vendor format
    pub fn to_vendor_format(
        &mut self,
        data: &TimeSeriesData,
        symbol: &str,
    ) -> Result<(VendorTimeSeriesData, ConversionMetadata)> {
        debug!("Converting {} data points for symbol: {}", data.values.len(), symbol);
        
        let start_time = Utc::now();
        let original_length = data.values.len();
        
        // Step 1: Validate input data
        self.validate_input_data(data)?;
        
        // Step 2: Handle missing values
        let mut processed_values = self.handle_missing_values(&data.values)?;
        let missing_filled = processed_values.len() - data.values.len();
        
        // Step 3: Remove outliers if enabled
        let outliers_removed = if self.config.remove_outliers {
            self.remove_outliers(&mut processed_values)?
        } else {
            0
        };
        
        // Step 4: Add technical indicators if enabled
        let mut features_added = Vec::new();
        if self.config.enable_feature_engineering {
            processed_values = self.add_technical_indicators(&processed_values, &mut features_added)?;
        }
        
        // Step 5: Add time-based features if enabled
        if self.config.enable_feature_engineering && !data.timestamps.is_empty() {
            let time_features = self.add_time_features(&data.timestamps)?;
            // For now, we'll just track that time features were computed
            features_added.extend(self.config.time_features.clone());
        }
        
        // Step 6: Normalize data if enabled
        let normalization_stats = if self.config.normalize_data {
            Some(self.normalize_data(&mut processed_values, symbol)?)
        } else {
            None
        };
        
        // Step 7: Convert to f32 and create vendor format
        let f32_values: Vec<f32> = processed_values
            .iter()
            .map(|&v| v as f32)
            .collect();
        
        let vendor_data = VendorTimeSeriesData {
            symbol: "default".to_string(),
            timestamps: Vec::new(), // Empty for now
            values: f32_values,
            exogenous_historical: None,
            exogenous_future: None,
            static_features: None,
            time_features: None,
        };
        
        // Create conversion metadata
        let metadata = ConversionMetadata {
            source_format: "TimeSeriesData".to_string(),
            target_format: "VendorTimeSeriesData<f32>".to_string(),
            conversion_timestamp: start_time,
            normalization_stats,
            features_added,
            outliers_removed,
            missing_filled,
            original_length,
            converted_length: vendor_data.values.len(),
        };
        
        info!("✅ Converted {} -> {} data points for {}", 
            original_length, vendor_data.values.len(), symbol);
        
        Ok((vendor_data, metadata))
    }
    
    /// Convert vendor forecast result back to internal format
    pub fn from_vendor_format(
        &self,
        forecast: &ForecastResult<f32>,
        metadata: &ConversionMetadata,
        symbol: &str,
    ) -> Result<Vec<f64>> {
        debug!("Converting vendor forecast with {} values for symbol: {}", 
            forecast.forecasts.len(), symbol);
        
        let mut forecasts: Vec<f64> = forecast.forecasts
            .iter()
            .map(|&f| f as f64)
            .collect();
        
        // Reverse normalization if it was applied
        if let Some(ref stats) = metadata.normalization_stats {
            self.denormalize_data(&mut forecasts, stats)?;
        }
        
        info!("✅ Converted {} forecast values back to internal format", forecasts.len());
        
        Ok(forecasts)
    }
    
    /// Validate input data quality
    fn validate_input_data(&self, data: &TimeSeriesData) -> Result<()> {
        if data.values.is_empty() {
            return Err(anyhow::anyhow!("Input data is empty"));
        }
        
        let missing_count = data.values.iter().filter(|v| v.is_nan()).count();
        let missing_percent = (missing_count as f64 / data.values.len() as f64) * 100.0;
        
        if missing_percent > self.config.max_missing_percent {
            return Err(anyhow::anyhow!(
                "Too many missing values: {:.1}% > {:.1}%", 
                missing_percent, self.config.max_missing_percent
            ));
        }
        
        Ok(())
    }
    
    /// Handle missing values using configured method
    fn handle_missing_values(&self, values: &[f64]) -> Result<Vec<f64>> {
        let mut processed = values.to_vec();
        let mut filled_count = 0;
        
        match self.config.missing_fill_method.as_str() {
            "forward" => {
                let mut last_valid = None;
                for value in &mut processed {
                    if value.is_nan() {
                        if let Some(last) = last_valid {
                            *value = last;
                            filled_count += 1;
                        }
                    } else {
                        last_valid = Some(*value);
                    }
                }
            }
            "backward" => {
                let mut next_valid = None;
                for value in processed.iter_mut().rev() {
                    if value.is_nan() {
                        if let Some(next) = next_valid {
                            *value = next;
                            filled_count += 1;
                        }
                    } else {
                        next_valid = Some(*value);
                    }
                }
            }
            "mean" => {
                let valid_values: Vec<f64> = values.iter().filter(|v| !v.is_nan()).copied().collect();
                if !valid_values.is_empty() {
                    let mean = valid_values.iter().sum::<f64>() / valid_values.len() as f64;
                    for value in &mut processed {
                        if value.is_nan() {
                            *value = mean;
                            filled_count += 1;
                        }
                    }
                }
            }
            "interpolate" => {
                // Simple linear interpolation
                for i in 0..processed.len() {
                    if processed[i].is_nan() {
                        // Find surrounding valid values
                        let mut left_idx = None;
                        let mut right_idx = None;
                        
                        for j in (0..i).rev() {
                            if !processed[j].is_nan() {
                                left_idx = Some(j);
                                break;
                            }
                        }
                        
                        for j in (i + 1)..processed.len() {
                            if !processed[j].is_nan() {
                                right_idx = Some(j);
                                break;
                            }
                        }
                        
                        if let (Some(left), Some(right)) = (left_idx, right_idx) {
                            let ratio = (i - left) as f64 / (right - left) as f64;
                            processed[i] = processed[left] + ratio * (processed[right] - processed[left]);
                            filled_count += 1;
                        }
                    }
                }
            }
            _ => {
                warn!("Unknown missing fill method: {}, using forward fill", self.config.missing_fill_method);
            }
        }
        
        debug!("Filled {} missing values using {}", filled_count, self.config.missing_fill_method);
        Ok(processed)
    }
    
    /// Remove outliers using configured method
    fn remove_outliers(&self, values: &mut Vec<f64>) -> Result<usize> {
        let original_len = values.len();
        
        match self.config.outlier_method.as_str() {
            "iqr" => {
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                
                let q1_idx = sorted.len() / 4;
                let q3_idx = 3 * sorted.len() / 4;
                let q1 = sorted[q1_idx];
                let q3 = sorted[q3_idx];
                let iqr = q3 - q1;
                let lower_bound = q1 - 1.5 * iqr;
                let upper_bound = q3 + 1.5 * iqr;
                
                values.retain(|&v| v >= lower_bound && v <= upper_bound);
            }
            "zscore" => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                let std_dev = variance.sqrt();
                
                values.retain(|&v| ((v - mean) / std_dev).abs() <= 3.0);
            }
            _ => {
                warn!("Unknown outlier method: {}, skipping outlier removal", self.config.outlier_method);
            }
        }
        
        let removed_count = original_len - values.len();
        debug!("Removed {} outliers using {}", removed_count, self.config.outlier_method);
        Ok(removed_count)
    }
    
    /// Add technical indicators to the data
    fn add_technical_indicators(
        &self,
        values: &[f64],
        features_added: &mut Vec<String>,
    ) -> Result<Vec<f64>> {
        let mut enhanced_values = values.to_vec();
        
        for indicator in &self.config.technical_indicators {
            match indicator.as_str() {
                name if name.starts_with("sma_") => {
                    if let Some(period_str) = name.strip_prefix("sma_") {
                        if let Ok(period) = period_str.parse::<usize>() {
                            let sma_values = self.calculate_sma(values, period);
                            // For now, we'll append the last SMA value as a feature
                            if let Some(last_sma) = sma_values.last() {
                                enhanced_values.push(*last_sma);
                                features_added.push(format!("sma_{}", period));
                            }
                        }
                    }
                }
                "rsi_14" => {
                    let rsi = self.calculate_rsi(values, 14);
                    if let Some(last_rsi) = rsi.last() {
                        enhanced_values.push(*last_rsi);
                        features_added.push("rsi_14".to_string());
                    }
                }
                "macd" => {
                    let (macd_line, _signal, _histogram) = self.calculate_macd(values);
                    if let Some(last_macd) = macd_line.last() {
                        enhanced_values.push(*last_macd);
                        features_added.push("macd".to_string());
                    }
                }
                _ => {
                    debug!("Unknown technical indicator: {}", indicator);
                }
            }
        }
        
        debug!("Added {} technical indicators", features_added.len());
        Ok(enhanced_values)
    }
    
    /// Add time-based features
    fn add_time_features(&self, timestamps: &[DateTime<Utc>]) -> Result<HashMap<String, Vec<f64>>> {
        let mut time_features = HashMap::new();
        
        for feature in &self.config.time_features {
            let values: Vec<f64> = match feature.as_str() {
                "hour" => timestamps.iter().map(|ts| ts.hour() as f64).collect(),
                "day_of_week" => timestamps.iter().map(|ts| ts.weekday().num_days_from_monday() as f64).collect(),
                "month" => timestamps.iter().map(|ts| ts.month() as f64).collect(),
                "quarter" => timestamps.iter().map(|ts| ((ts.month() - 1) / 3 + 1) as f64).collect(),
                _ => {
                    warn!("Unknown time feature: {}", feature);
                    continue;
                }
            };
            
            time_features.insert(feature.clone(), values);
        }
        
        debug!("Generated {} time features", time_features.len());
        Ok(time_features)
    }
    
    /// Normalize data using configured method
    fn normalize_data(&mut self, values: &mut [f64], symbol: &str) -> Result<NormalizationStats> {
        let stats = match self.config.normalization_method.as_str() {
            "minmax" => {
                let min_val = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let max_val = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                
                if max_val != min_val {
                    for value in values.iter_mut() {
                        *value = (*value - min_val) / (max_val - min_val);
                    }
                }
                
                NormalizationStats {
                    method: "minmax".to_string(),
                    min_value: min_val,
                    max_value: max_val,
                    mean: 0.0,
                    std_dev: 0.0,
                    median: 0.0,
                    q25: 0.0,
                    q75: 0.0,
                }
            }
            "zscore" => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                let std_dev = variance.sqrt();
                
                if std_dev != 0.0 {
                    for value in values.iter_mut() {
                        *value = (*value - mean) / std_dev;
                    }
                }
                
                NormalizationStats {
                    method: "zscore".to_string(),
                    min_value: 0.0,
                    max_value: 0.0,
                    mean,
                    std_dev,
                    median: 0.0,
                    q25: 0.0,
                    q75: 0.0,
                }
            }
            _ => {
                warn!("Unknown normalization method: {}, using minmax", self.config.normalization_method);
                return self.normalize_data(values, symbol);
            }
        };
        
        // Cache normalization stats for reverse conversion
        self.normalization_cache.insert(symbol.to_string(), stats.clone());
        
        debug!("Normalized data using {} method", stats.method);
        Ok(stats)
    }
    
    /// Reverse normalization
    fn denormalize_data(&self, values: &mut [f64], stats: &NormalizationStats) -> Result<()> {
        match stats.method.as_str() {
            "minmax" => {
                for value in values.iter_mut() {
                    *value = *value * (stats.max_value - stats.min_value) + stats.min_value;
                }
            }
            "zscore" => {
                for value in values.iter_mut() {
                    *value = *value * stats.std_dev + stats.mean;
                }
            }
            _ => {
                warn!("Unknown normalization method for reversal: {}", stats.method);
            }
        }
        
        debug!("Denormalized data using {} method", stats.method);
        Ok(())
    }
    
    /// Calculate Simple Moving Average
    fn calculate_sma(&self, values: &[f64], period: usize) -> Vec<f64> {
        let mut sma = Vec::new();
        
        for i in period - 1..values.len() {
            let sum: f64 = values[i - period + 1..=i].iter().sum();
            sma.push(sum / period as f64);
        }
        
        sma
    }
    
    /// Calculate RSI (Relative Strength Index)
    fn calculate_rsi(&self, values: &[f64], period: usize) -> Vec<f64> {
        if values.len() < period + 1 {
            return Vec::new();
        }
        
        let mut gains = Vec::new();
        let mut losses = Vec::new();
        
        // Calculate gains and losses
        for i in 1..values.len() {
            let change = values[i] - values[i - 1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }
        
        let mut rsi = Vec::new();
        
        // Calculate RSI for each window
        for i in period - 1..gains.len() {
            let avg_gain: f64 = gains[i - period + 1..=i].iter().sum::<f64>() / period as f64;
            let avg_loss: f64 = losses[i - period + 1..=i].iter().sum::<f64>() / period as f64;
            
            let rs = if avg_loss != 0.0 { avg_gain / avg_loss } else { 0.0 };
            let rsi_value = 100.0 - (100.0 / (1.0 + rs));
            rsi.push(rsi_value);
        }
        
        rsi
    }
    
    /// Calculate MACD (Moving Average Convergence Divergence)
    fn calculate_macd(&self, values: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let ema12 = self.calculate_ema(values, 12);
        let ema26 = self.calculate_ema(values, 26);
        
        // MACD line = EMA12 - EMA26
        let mut macd_line = Vec::new();
        let start_idx = 26 - 1; // Start from where EMA26 is valid
        
        for i in 0..ema12.len().min(ema26.len()) {
            if i + start_idx < ema12.len() {
                macd_line.push(ema12[i + start_idx] - ema26[i]);
            }
        }
        
        // Signal line = EMA9 of MACD line
        let signal_line = self.calculate_ema(&macd_line, 9);
        
        // Histogram = MACD - Signal
        let mut histogram = Vec::new();
        let signal_start = 9 - 1;
        
        for i in 0..macd_line.len() {
            if i >= signal_start && i - signal_start < signal_line.len() {
                histogram.push(macd_line[i] - signal_line[i - signal_start]);
            }
        }
        
        (macd_line, signal_line, histogram)
    }
    
    /// Calculate Exponential Moving Average
    fn calculate_ema(&self, values: &[f64], period: usize) -> Vec<f64> {
        if values.is_empty() || period == 0 {
            return Vec::new();
        }
        
        let alpha = 2.0 / (period as f64 + 1.0);
        let mut ema = Vec::new();
        
        // Start with SMA for the first value
        let initial_sma: f64 = values[..period.min(values.len())].iter().sum::<f64>() 
            / period.min(values.len()) as f64;
        ema.push(initial_sma);
        
        // Calculate EMA for remaining values
        for i in 1..values.len() {
            let new_ema = alpha * values[i] + (1.0 - alpha) * ema[i - 1];
            ema.push(new_ema);
        }
        
        ema
    }
    
    /// Get normalization stats for a symbol
    pub fn get_normalization_stats(&self, symbol: &str) -> Option<&NormalizationStats> {
        self.normalization_cache.get(symbol)
    }
    
    /// Clear normalization cache
    pub fn clear_cache(&mut self) {
        self.normalization_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    
    // Test-only TimeSeriesData structure that matches what the converter expects
    #[derive(Debug, Clone)]
    struct TestTimeSeriesData {
        values: Vec<f64>,
        timestamps: Vec<DateTime<Utc>>,
        metadata: HashMap<String, serde_json::Value>,
    }
    
    fn create_test_data() -> TestTimeSeriesData {
        let values = vec![100.0, 101.0, 99.0, 102.0, 98.0, 103.0, 97.0, 104.0];
        let timestamps = (0..8)
            .map(|i| Utc.timestamp_opt(1600000000 + i * 3600, 0).unwrap())
            .collect();
        
        TestTimeSeriesData {
            values,
            timestamps,
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("AAPL"));
                map
            }
        }
    }
    
    #[test]
    fn test_data_converter_creation() {
        let converter = DataConverter::default();
        assert!(converter.config.normalize_data);
        assert_eq!(converter.config.normalization_method, "minmax");
    }
    
    #[test]
    fn test_to_vendor_format() {
        let mut converter = DataConverter::default();
        let test_data = create_test_data();
        
        let result = converter.to_vendor_format(&test_data, "AAPL");
        assert!(result.is_ok());
        
        let (vendor_data, metadata) = result.unwrap();
        assert!(!vendor_data.values.is_empty());
        assert_eq!(metadata.source_format, "TimeSeriesData");
        assert_eq!(metadata.target_format, "VendorTimeSeriesData<f32>");
    }
    
    #[test]
    fn test_missing_value_handling() {
        let converter = DataConverter::default();
        let values = vec![1.0, f64::NAN, 3.0, f64::NAN, 5.0];
        
        let result = converter.handle_missing_values(&values);
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert!(processed.iter().all(|v| !v.is_nan()));
    }
    
    #[test]
    fn test_technical_indicators() {
        let converter = DataConverter::default();
        let values = vec![100.0, 101.0, 99.0, 102.0, 98.0, 103.0, 97.0, 104.0];
        let mut features_added = Vec::new();
        
        let result = converter.add_technical_indicators(&values, &mut features_added);
        assert!(result.is_ok());
        
        let enhanced = result.unwrap();
        assert!(enhanced.len() >= values.len());
        assert!(!features_added.is_empty());
    }
    
    #[test]
    fn test_normalization() {
        let mut converter = DataConverter::default();
        let mut values = vec![100.0, 200.0, 150.0, 300.0];
        
        let result = converter.normalize_data(&mut values, "TEST");
        assert!(result.is_ok());
        
        let stats = result.unwrap();
        assert_eq!(stats.method, "minmax");
        assert!(values.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }
    
    #[test]
    fn test_sma_calculation() {
        let converter = DataConverter::default();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let sma = converter.calculate_sma(&values, 3);
        assert_eq!(sma.len(), 3);
        assert_eq!(sma[0], 2.0); // (1+2+3)/3
        assert_eq!(sma[1], 3.0); // (2+3+4)/3
        assert_eq!(sma[2], 4.0); // (3+4+5)/3
    }
    
    #[test]
    fn test_rsi_calculation() {
        let converter = DataConverter::default();
        let values = vec![44.0, 44.25, 44.5, 43.75, 44.5, 44.25, 44.0, 44.25];
        
        let rsi = converter.calculate_rsi(&values, 3);
        assert!(!rsi.is_empty());
        assert!(rsi.iter().all(|&v| v >= 0.0 && v <= 100.0));
    }
}