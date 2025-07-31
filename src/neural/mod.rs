//! Neural Network Integration Module
//!
//! Provides neural network prediction capabilities with real FANN integration

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

// Internal modules - temporarily public for compilation
// REMOVED: fann_predictor_legacy_deprecated - was causing 131 compilation errors
pub mod fann; // New modular FANN architecture (PREFERRED)
mod fann_model_adapter;
mod streaming_connector;
mod online_validator;
mod online_learning_manager;
mod enhanced_predictor;
mod performance_optimizer;
mod batch_optimizer;

// Public predictor module - using predictor.rs file  
pub mod predictor;

// Performance benchmarking module
#[cfg(test)]
pub mod performance_benchmarks;

// Ensemble types
pub mod ensemble_types;

// Phase 3B: Removed monitoring module - architectural layer not allowed

// Test modules
#[cfg(test)]
pub mod tests;

// Online learning test suite
#[cfg(test)]
pub mod online_learning_tests;

// CLEAN ARCHITECTURE ENFORCEMENT: Single routing path
// NeuralPredictor → EnhancedNeuralAdapter → FannPredictor
// All production features preserved while eliminating routing complexity

// Phase 3B: Removed monitoring re-exports - these were architectural layers

// Re-export the main neural predictor (clean wrapper)
pub use predictor::{NeuralPredictor as CleanNeuralPredictor};

// Performance types already re-exported above - remove duplicate

// Internal implementations - DO NOT EXPORT
// - FannPredictor: Internal implementation detail
// - FannModelAdapter: Internal implementation detail
// - EnhancedNeuralPredictor: Internal implementation detail
// - OptimizedFannPredictor: Internal implementation detail
// Access to these must go through NeuralPredictor only

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

// Re-export the clean NeuralPredictor implementation
pub use predictor::NeuralPredictor;

// Re-export modular FANN components (PRIMARY EXPORTS)
pub use fann::{
    FannPredictor, // Primary FannPredictor implementation - TRAIT IMPLEMENTATION ADDED ✅
    ModelConfig, 
    FannModelConfig,
    ModelPerformance,
    MarketRegime,
    NeuralError,
    EnsembleManager,
    StreamingConfig,
    TrainingResult,
    TrainingAlgorithm,
    NetworkArchitecture,
    ConversionConfig,
    NormalizationMethod,
    RecurrentState,
};

// LEGACY: Removed legacy predictor - now using modular FannPredictor from fann/ module

// Re-export HealthStatus from adapters module
pub use crate::adapters::HealthStatus;

// Phase 3B: Removed additional monitoring re-exports - architectural layers not allowed

// Integration tests are now in src/neural/predictor.rs
// This ensures tests are co-located with the implementation
