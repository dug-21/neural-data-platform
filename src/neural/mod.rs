//! Neural Network Integration Module
//! 
//! Provides neural network prediction capabilities with real FANN integration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use async_trait::async_trait;

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

// Module for FANN-based predictions
pub mod fann_predictor;

// Module for enhanced predictor with Phase 6 features
pub mod enhanced_predictor;

// Performance optimization module
pub mod performance_optimizer;

// Batch processing optimization
pub mod batch_optimizer;

// Performance benchmarking module
#[cfg(test)]
pub mod performance_benchmarks;

// Ensemble types
pub mod ensemble_types;

// Test modules
#[cfg(test)]
pub mod tests;

// Re-export the FANN predictor
pub use fann_predictor::{FannPredictor, FannModelConfig};

// Re-export the enhanced predictor
pub use enhanced_predictor::{
    EnhancedNeuralPredictor, 
    EnhancedPredictionResult, 
    ConfidenceBreakdown, 
    RetrainingMetrics,
    PerformanceTracker
};

// Re-export performance optimization components
pub use performance_optimizer::{
    OptimizedFannPredictor,
    PerformanceMetrics
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub confidence: f64,
    pub interval_low: f64,
    pub interval_high: f64,
    pub model_name: String,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

// Public trait for neural predictors
#[async_trait]
pub trait NeuralPredictorTrait: Send + Sync {
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>>;
    
    async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>>;
    
    async fn get_feature_importance(&self) -> Result<HashMap<String, f64>>;
}

/// Main neural predictor that uses FANN for real predictions
pub struct NeuralPredictor {
    fann_predictor: FannPredictor,
}

impl NeuralPredictor {
    pub fn new(config: NeuralConfig) -> Result<Self> {
        let fann_predictor = FannPredictor::new(config)?;
        Ok(Self { fann_predictor })
    }
    
    pub async fn load_historical_data(&self, _data: Vec<TimeSeriesData>) -> Result<()> {
        // Data loading is handled internally by the predictor
        Ok(())
    }
    
    pub async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        self.fann_predictor.predict(data, horizon, features).await
    }
    
    pub async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        self.fann_predictor.predict_ensemble(data, horizon, models, features).await
    }
    
    pub async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        self.fann_predictor.get_feature_importance().await
    }
}

// Default implementation
impl Default for NeuralPredictor {
    fn default() -> Self {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        Self::new(config).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_neural_predictor_creation() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.1,
        };
        
        let predictor = NeuralPredictor::new(config);
        assert!(predictor.is_ok());
    }
}