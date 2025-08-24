//! Prediction and model output types
//! Module size: <200 lines as per requirements

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Neural model prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    value: f64,
    confidence: f64,
    lower_bound: Option<f64>,
    upper_bound: Option<f64>,
    horizon_minutes: u32,
    timestamp: DateTime<Utc>,
}

impl Prediction {
    /// Create new prediction
    pub fn new(value: f64, confidence: f64) -> Self {
        // Clamp confidence to [0, 1]
        let confidence = confidence.max(0.0).min(1.0);
        
        Self {
            value,
            confidence,
            lower_bound: None,
            upper_bound: None,
            horizon_minutes: 60, // Default 1 hour
            timestamp: Utc::now(),
        }
    }
    
    /// Set prediction bounds
    pub fn with_bounds(mut self, lower: f64, upper: f64) -> Self {
        self.lower_bound = Some(lower);
        self.upper_bound = Some(upper);
        self
    }
    
    /// Set prediction horizon
    pub fn with_horizon(mut self, minutes: u32) -> Self {
        self.horizon_minutes = minutes;
        self
    }
    
    // Getters
    pub fn value(&self) -> f64 { self.value }
    pub fn confidence(&self) -> f64 { self.confidence }
    pub fn lower_bound(&self) -> Option<f64> { self.lower_bound }
    pub fn upper_bound(&self) -> Option<f64> { self.upper_bound }
    pub fn horizon_minutes(&self) -> u32 { self.horizon_minutes }
    pub fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
}

/// Result from model prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub model_name: String,
    pub predictions: Vec<Prediction>,
    pub feature_importance: HashMap<String, f64>,
    pub inference_time_ms: u64,
    pub model_version: String,
}

impl PredictionResult {
    pub fn new(model_name: String, predictions: Vec<Prediction>) -> Self {
        Self {
            model_name,
            predictions,
            feature_importance: HashMap::new(),
            inference_time_ms: 0,
            model_version: "1.0.0".to_string(),
        }
    }
    
    /// Get primary prediction
    pub fn primary(&self) -> Option<&Prediction> {
        self.predictions.first()
    }
    
    /// Average confidence across predictions
    pub fn avg_confidence(&self) -> f64 {
        if self.predictions.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = self.predictions.iter().map(|p| p.confidence).sum();
        sum / self.predictions.len() as f64
    }
}

/// Raw model output before post-processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOutput {
    pub raw_values: Vec<f64>,
    pub probabilities: Option<Vec<f64>>,
    pub features_used: Vec<String>,
    pub model_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prediction_confidence_clamping() {
        let p1 = Prediction::new(100.0, 1.5);
        assert_eq!(p1.confidence(), 1.0);
        
        let p2 = Prediction::new(100.0, -0.5);
        assert_eq!(p2.confidence(), 0.0);
    }
    
    #[test]
    fn test_prediction_result_aggregation() {
        let predictions = vec![
            Prediction::new(100.0, 0.8),
            Prediction::new(101.0, 0.9),
            Prediction::new(99.0, 0.7),
        ];
        
        let result = PredictionResult::new("test_model".to_string(), predictions);
        assert!((result.avg_confidence() - 0.8).abs() < 0.0001);
    }
}