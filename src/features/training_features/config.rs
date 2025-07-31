//! Configuration for feature engineering
//!
//! This module provides configuration structures and enums for the training
//! feature engineering system. It defines normalization methods, missing data
//! strategies, and feature extraction parameters.

use serde::{Serialize, Deserialize};

/// Configuration for feature engineering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Technical indicator periods
    pub indicator_periods: Vec<usize>,
    
    /// Price transformation settings
    pub return_periods: Vec<usize>,
    
    /// Volatility window sizes
    pub volatility_windows: Vec<usize>,
    
    /// Market microstructure settings
    pub microstructure_enabled: bool,
    
    /// Rolling statistics windows
    pub rolling_windows: Vec<usize>,
    
    /// Normalization method
    pub normalization: NormalizationMethod,
    
    /// Handle missing data
    pub handle_missing: MissingDataStrategy,
    
    /// Feature selection threshold
    pub min_feature_variance: f64,
    
    /// Enable incremental updates
    pub incremental_updates: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            indicator_periods: vec![5, 10, 20, 50, 100],
            return_periods: vec![1, 5, 10, 20],
            volatility_windows: vec![10, 20, 30, 60],
            microstructure_enabled: true,
            rolling_windows: vec![5, 10, 20, 50],
            normalization: NormalizationMethod::ZScore,
            handle_missing: MissingDataStrategy::Forward,
            min_feature_variance: 1e-6,
            incremental_updates: true,
        }
    }
}

/// Normalization methods for features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationMethod {
    MinMax,
    ZScore,
    RobustScaler,
    Tanh,
    Percentile,
}

/// Strategies for handling missing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MissingDataStrategy {
    Drop,
    Forward,
    Backward,
    Interpolate,
    Mean,
}

/// Parameters for different scaling methods
#[derive(Debug, Clone)]
pub enum ScalerParams {
    MinMax { min: f64, max: f64 },
    ZScore { mean: f64, std: f64 },
    Robust { median: f64, mad: f64 },
    Percentile { p5: f64, p95: f64 },
}

/// Feature scaler for normalization
#[derive(Debug, Clone)]
pub struct FeatureScaler {
    pub method: NormalizationMethod,
    pub params: ScalerParams,
}

impl FeatureScaler {
    /// Create a new scaler with the given method and parameters
    pub fn new(method: NormalizationMethod, params: ScalerParams) -> Self {
        Self { method, params }
    }
}