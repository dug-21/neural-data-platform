//! Ensemble prediction types for neural models

use std::collections::HashMap;

/// Result from ensemble prediction combining multiple models
#[derive(Debug, Clone)]
pub struct EnsemblePrediction {
    /// The main prediction values
    pub predictions: Vec<f64>,
    /// Overall confidence score
    pub confidence: Option<f64>,
    /// Prediction intervals if available
    pub prediction_intervals: Option<Vec<(f64, f64)>>,
    /// Additional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl Default for EnsemblePrediction {
    fn default() -> Self {
        Self {
            predictions: Vec::new(),
            confidence: None,
            prediction_intervals: None,
            metadata: None,
        }
    }
}