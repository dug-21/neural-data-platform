//! Data conversion modules for FANN predictor
//!
//! This module provides input and output conversion functionality
//! for transforming data between different formats used by the neural networks.

pub mod input;
pub mod output;

// Re-export commonly used types
pub use input::InputConverter;
pub use output::OutputConverter;

use serde::{Deserialize, Serialize};
use crate::data::TimeSeriesData;
use crate::neural::PredictionResult;

/// Conversion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionConfig {
    /// Normalization method for input data
    pub normalization_method: NormalizationMethod,
    /// Feature scaling parameters
    pub feature_scaling: FeatureScalingConfig,
    /// Output transformation parameters
    pub output_transform: OutputTransformConfig,
    /// Enable data validation
    pub validate_data: bool,
}

/// Normalization methods for input data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationMethod {
    /// Min-max normalization to [0, 1]
    MinMax,
    /// Z-score standardization (mean=0, std=1)
    ZScore,
    /// Robust scaling using median and IQR
    Robust,
    /// No normalization
    None,
}

/// Feature scaling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScalingConfig {
    /// Scale price features
    pub scale_prices: bool,
    /// Scale volume features
    pub scale_volumes: bool,
    /// Scale indicator features
    pub scale_indicators: bool,
    /// Custom scaling factors
    pub custom_factors: Option<Vec<f64>>,
}

/// Output transformation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputTransformConfig {
    /// Transform predictions to price format
    pub to_price_format: bool,
    /// Apply inverse normalization
    pub denormalize: bool,
    /// Confidence threshold for filtering predictions
    pub confidence_threshold: f64,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            normalization_method: NormalizationMethod::MinMax,
            feature_scaling: FeatureScalingConfig::default(),
            output_transform: OutputTransformConfig::default(),
            validate_data: true,
        }
    }
}

impl Default for FeatureScalingConfig {
    fn default() -> Self {
        Self {
            scale_prices: true,
            scale_volumes: true,
            scale_indicators: true,
            custom_factors: None,
        }
    }
}

impl Default for OutputTransformConfig {
    fn default() -> Self {
        Self {
            to_price_format: true,
            denormalize: true,
            confidence_threshold: 0.5,
        }
    }
}

/// Data validation errors
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Invalid input data: {0}")]
    InvalidInput(String),
    #[error("Normalization failed: {0}")]
    NormalizationError(String),
    #[error("Feature scaling error: {0}")]
    FeatureScalingError(String),
    #[error("Output transformation error: {0}")]
    OutputTransformError(String),
    #[error("Data validation failed: {0}")]
    ValidationError(String),
}

/// Statistics for normalization and scaling
#[derive(Debug, Clone)]
pub struct NormalizationStats {
    /// Minimum values for each feature
    pub min_values: Vec<f64>,
    /// Maximum values for each feature
    pub max_values: Vec<f64>,
    /// Mean values for each feature
    pub mean_values: Vec<f64>,
    /// Standard deviation for each feature
    pub std_values: Vec<f64>,
    /// Median values for each feature (for robust scaling)
    pub median_values: Vec<f64>,
    /// Interquartile range for each feature
    pub iqr_values: Vec<f64>,
}

impl NormalizationStats {
    /// Create new normalization statistics
    pub fn new(feature_count: usize) -> Self {
        Self {
            min_values: vec![f64::INFINITY; feature_count],
            max_values: vec![f64::NEG_INFINITY; feature_count],
            mean_values: vec![0.0; feature_count],
            std_values: vec![1.0; feature_count],
            median_values: vec![0.0; feature_count],
            iqr_values: vec![1.0; feature_count],
        }
    }

    /// Update statistics with new data
    pub fn update(&mut self, data: &[Vec<f64>]) -> Result<(), ConversionError> {
        if data.is_empty() {
            return Err(ConversionError::ValidationError("Empty data provided".to_string()));
        }

        let feature_count = data[0].len();
        if feature_count != self.min_values.len() {
            return Err(ConversionError::ValidationError(
                format!("Feature count mismatch: expected {}, got {}", 
                        self.min_values.len(), feature_count)
            ));
        }

        // Update min/max values
        for sample in data {
            for (i, &value) in sample.iter().enumerate() {
                if value.is_finite() {
                    self.min_values[i] = self.min_values[i].min(value);
                    self.max_values[i] = self.max_values[i].max(value);
                }
            }
        }

        // Calculate mean and std
        for i in 0..feature_count {
            let values: Vec<f64> = data.iter()
                .map(|sample| sample[i])
                .filter(|v| v.is_finite())
                .collect();

            if !values.is_empty() {
                self.mean_values[i] = values.iter().sum::<f64>() / values.len() as f64;
                
                let variance = values.iter()
                    .map(|v| (v - self.mean_values[i]).powi(2))
                    .sum::<f64>() / values.len() as f64;
                
                self.std_values[i] = variance.sqrt().max(1e-8); // Avoid division by zero

                // Calculate median and IQR for robust scaling
                let mut sorted_values = values.clone();
                sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                
                let len = sorted_values.len();
                self.median_values[i] = if len % 2 == 0 {
                    (sorted_values[len / 2 - 1] + sorted_values[len / 2]) / 2.0
                } else {
                    sorted_values[len / 2]
                };

                // Calculate IQR (Q3 - Q1)
                let q1_idx = len / 4;
                let q3_idx = 3 * len / 4;
                let q1 = sorted_values[q1_idx];
                let q3 = sorted_values[q3_idx];
                self.iqr_values[i] = (q3 - q1).max(1e-8); // Avoid division by zero
            }
        }

        Ok(())
    }

    /// Check if statistics are valid
    pub fn is_valid(&self) -> bool {
        self.min_values.iter().all(|v| v.is_finite()) &&
        self.max_values.iter().all(|v| v.is_finite()) &&
        self.mean_values.iter().all(|v| v.is_finite()) &&
        self.std_values.iter().all(|&v| v > 0.0 && v.is_finite()) &&
        self.median_values.iter().all(|v| v.is_finite()) &&
        self.iqr_values.iter().all(|&v| v > 0.0 && v.is_finite())
    }
}

/// Trait for data conversion operations
pub trait DataConverter {
    /// Convert input data to neural network format
    fn convert_input(&self, data: &[TimeSeriesData]) -> Result<Vec<Vec<f32>>, ConversionError>;
    
    /// Convert neural network output to prediction results
    fn convert_output(&self, outputs: &[f32], base_data: &TimeSeriesData) -> Result<Vec<PredictionResult>, ConversionError>;
    
    /// Validate input data
    fn validate_input(&self, data: &[TimeSeriesData]) -> Result<(), ConversionError>;
    
    /// Get feature count for this converter
    fn feature_count(&self) -> usize;
}

/// Utility functions for data conversion
pub mod utils {
    use super::*;

    /// Apply min-max normalization to a value
    pub fn min_max_normalize(value: f64, min: f64, max: f64) -> f64 {
        if (max - min).abs() < f64::EPSILON {
            0.5 // Default to middle value if range is zero
        } else {
            (value - min) / (max - min)
        }
    }

    /// Apply z-score normalization to a value
    pub fn z_score_normalize(value: f64, mean: f64, std: f64) -> f64 {
        if std.abs() < f64::EPSILON {
            0.0 // Default to zero if std is zero
        } else {
            (value - mean) / std
        }
    }

    /// Apply robust scaling to a value
    pub fn robust_scale(value: f64, median: f64, iqr: f64) -> f64 {
        if iqr.abs() < f64::EPSILON {
            0.0 // Default to zero if IQR is zero
        } else {
            (value - median) / iqr
        }
    }

    /// Inverse min-max normalization
    pub fn inverse_min_max_normalize(normalized_value: f64, min: f64, max: f64) -> f64 {
        normalized_value * (max - min) + min
    }

    /// Inverse z-score normalization
    pub fn inverse_z_score_normalize(normalized_value: f64, mean: f64, std: f64) -> f64 {
        normalized_value * std + mean
    }

    /// Inverse robust scaling
    pub fn inverse_robust_scale(scaled_value: f64, median: f64, iqr: f64) -> f64 {
        scaled_value * iqr + median
    }

    /// Calculate percentage change
    pub fn percentage_change(current: f64, previous: f64) -> f64 {
        if previous.abs() < f64::EPSILON {
            0.0
        } else {
            (current - previous) / previous
        }
    }

    /// Calculate log return
    pub fn log_return(current: f64, previous: f64) -> f64 {
        if previous <= 0.0 || current <= 0.0 {
            0.0
        } else {
            (current / previous).ln()
        }
    }

    /// Clamp value to a range
    pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
        value.max(min).min(max)
    }

    /// Check if value is valid (finite and not NaN)
    pub fn is_valid_value(value: f64) -> bool {
        value.is_finite()
    }

    /// Replace invalid values with a default
    pub fn sanitize_value(value: f64, default: f64) -> f64 {
        if is_valid_value(value) {
            value
        } else {
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::utils::*;

    #[test]
    fn test_min_max_normalization() {
        assert_eq!(min_max_normalize(5.0, 0.0, 10.0), 0.5);
        assert_eq!(min_max_normalize(0.0, 0.0, 10.0), 0.0);
        assert_eq!(min_max_normalize(10.0, 0.0, 10.0), 1.0);
        assert_eq!(min_max_normalize(5.0, 5.0, 5.0), 0.5); // Zero range case
    }

    #[test]
    fn test_z_score_normalization() {
        assert_eq!(z_score_normalize(10.0, 10.0, 2.0), 0.0);
        assert_eq!(z_score_normalize(12.0, 10.0, 2.0), 1.0);
        assert_eq!(z_score_normalize(8.0, 10.0, 2.0), -1.0);
        assert_eq!(z_score_normalize(10.0, 10.0, 0.0), 0.0); // Zero std case
    }

    #[test]
    fn test_robust_scaling() {
        assert_eq!(robust_scale(10.0, 10.0, 4.0), 0.0);
        assert_eq!(robust_scale(14.0, 10.0, 4.0), 1.0);
        assert_eq!(robust_scale(6.0, 10.0, 4.0), -1.0);
        assert_eq!(robust_scale(10.0, 10.0, 0.0), 0.0); // Zero IQR case
    }

    #[test]
    fn test_percentage_change() {
        assert_eq!(percentage_change(110.0, 100.0), 0.1);
        assert_eq!(percentage_change(90.0, 100.0), -0.1);
        assert_eq!(percentage_change(100.0, 0.0), 0.0); // Zero previous case
    }

    #[test]
    fn test_value_validation() {
        assert!(is_valid_value(1.0));
        assert!(is_valid_value(-1.0));
        assert!(is_valid_value(0.0));
        assert!(!is_valid_value(f64::NAN));
        assert!(!is_valid_value(f64::INFINITY));
        assert!(!is_valid_value(f64::NEG_INFINITY));

        assert_eq!(sanitize_value(1.0, 0.0), 1.0);
        assert_eq!(sanitize_value(f64::NAN, 0.0), 0.0);
        assert_eq!(sanitize_value(f64::INFINITY, 0.0), 0.0);
    }

    #[test]
    fn test_normalization_stats() {
        let mut stats = NormalizationStats::new(2);
        let data = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
        ];

        stats.update(&data).unwrap();

        assert_eq!(stats.min_values, vec![1.0, 2.0]);
        assert_eq!(stats.max_values, vec![5.0, 6.0]);
        assert_eq!(stats.mean_values, vec![3.0, 4.0]);
        assert!(stats.is_valid());
    }
}