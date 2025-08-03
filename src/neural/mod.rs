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
// pub mod fann; // New modular FANN architecture (DISABLED - missing implementation)
mod fann_model_adapter;
mod streaming_connector;
mod online_validator;
mod online_learning_manager;
mod enhanced_predictor;
mod performance_optimizer;
mod batch_optimizer;

// Phase 1: Vendor model integration
pub mod vendor_predictor;
pub mod memory_optimized_predictor;
pub mod sector_aggregator;
pub mod model_factory;

// Phase 3: Real-time training extensions
pub mod realtime_training;

// Re-export vendor integration components
pub use vendor_predictor::{VendorPredictor, VendorPredictorConfig};
pub use memory_optimized_predictor::{
    MemoryOptimizedPredictor,
    MemoryOptimizedConfig,
    MemoryUsageStats,
    OptimizationResult,
};
pub use sector_aggregator::{
    SectorAggregator, SectorAggregatorConfig, SectorAggregation, 
    ETFCorrelation, BreadthConfig
};

// NOTE: predictor.rs module removed - using VendorPredictor directly as NeuralPredictor

// Performance benchmarking module
#[cfg(test)]
pub mod performance_benchmarks;

// Ensemble types
pub mod ensemble_types;

// Performance channel communication
pub mod performance_channel;

// Performance events aggregation
pub mod performance_events;

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

// Re-export VendorPredictor as the main NeuralPredictor
pub use vendor_predictor::VendorPredictor as NeuralPredictor;

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

impl Default for PredictionResult {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            value: 0.0,
            confidence: 0.0,
            interval_low: 0.0,
            interval_high: 0.0,
            model_name: "unknown".to_string(),
            metadata: None,
        }
    }
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

// VendorPredictor is now our main NeuralPredictor (re-exported above)

// Note: fann module exports disabled - using VendorPredictor instead
// Re-export additional vendor predictor components
pub use vendor_predictor::{
    ModelConfig,
    ClusterModelPool,
    VendorPredictor as FannPredictor, // Backward compatibility alias
    DataRequirements,
};

// Re-export performance events types
pub use performance_events::{
    PerformanceSnapshot, 
    TradingPerformanceMetrics, 
    AccuracyMetrics, 
    DataTypeMetrics, 
    ChannelMetrics
};

// LEGACY: Removed legacy predictor - now using modular FannPredictor from fann/ module

// Re-export HealthStatus from adapters module
pub use crate::adapters::HealthStatus;

// Phase 3B: Removed additional monitoring re-exports - architectural layers not allowed

// Integration tests are now in src/neural/predictor.rs
// This ensures tests are co-located with the implementation

#[cfg(test)]
mod environment_tests {
    use super::*;
    use std::env;
    use std::sync::Arc;
    use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig};
    use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
    use crate::config::NeuralConfig;

    #[tokio::test]
    async fn test_neural_vendor_environment_variable_respected() {
        // Test that NEURAL_VENDOR environment variable is properly respected
        
        // Save original value
        let original_vendor = env::var("NEURAL_VENDOR").ok();
        
        // Test with different vendor settings
        env::set_var("NEURAL_VENDOR", "ruv_fann");
        
        let config = NeuralConfig {
            use_real_models: true,
            models: vec!["LSTM".to_string()],
            input_size: 10,
            output_size: 1,
            hidden_layers: vec![20],
            learning_rate: 0.01,
            ..Default::default()
        };
        
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        // VendorPredictor should respect the NEURAL_VENDOR environment variable
        let result = VendorPredictor::new(&config, sector_mapper, performance_tracker);
        assert!(result.is_ok(), "VendorPredictor should initialize with NEURAL_VENDOR=ruv_fann");
        
        // Test with fallback vendor
        env::set_var("NEURAL_VENDOR", "fallback");
        
        let sector_mapper2 = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker2 = Arc::new(ModelPerformanceTracker::new());
        
        let result2 = VendorPredictor::new(&config, sector_mapper2, performance_tracker2);
        assert!(result2.is_ok(), "VendorPredictor should handle fallback vendor gracefully");
        
        // Restore original value
        match original_vendor {
            Some(val) => env::set_var("NEURAL_VENDOR", val),
            None => env::remove_var("NEURAL_VENDOR"),
        }
    }

    #[tokio::test]
    async fn test_training_mode_environment_variable_respected() {
        // Test that TRAINING_MODE environment variable is properly respected
        
        // Save original value
        let original_mode = env::var("TRAINING_MODE").ok();
        
        // Test with online training mode
        env::set_var("TRAINING_MODE", "online");
        
        let config = NeuralConfig {
            use_real_models: true,
            models: vec!["LSTM".to_string()],
            input_size: 10,
            output_size: 1,
            hidden_layers: vec![20],
            learning_rate: 0.01,
            ..Default::default()
        };
        
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        // VendorPredictor should respect the TRAINING_MODE environment variable
        let result = VendorPredictor::new(&config, sector_mapper, performance_tracker);
        assert!(result.is_ok(), "VendorPredictor should handle TRAINING_MODE=online");
        
        // Test with batch training mode
        env::set_var("TRAINING_MODE", "batch");
        
        let sector_mapper2 = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker2 = Arc::new(ModelPerformanceTracker::new());
        
        let result2 = VendorPredictor::new(&config, sector_mapper2, performance_tracker2);
        assert!(result2.is_ok(), "VendorPredictor should handle TRAINING_MODE=batch");
        
        // Test with disabled training mode
        env::set_var("TRAINING_MODE", "disabled");
        
        let sector_mapper3 = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker3 = Arc::new(ModelPerformanceTracker::new());
        
        let result3 = VendorPredictor::new(&config, sector_mapper3, performance_tracker3);
        assert!(result3.is_ok(), "VendorPredictor should handle TRAINING_MODE=disabled");
        
        // Restore original value
        match original_mode {
            Some(val) => env::set_var("TRAINING_MODE", val),
            None => env::remove_var("TRAINING_MODE"),
        }
    }

    #[tokio::test]
    async fn test_environment_variables_with_different_configurations() {
        // Test environment variables with different neural configurations
        
        let original_vendor = env::var("NEURAL_VENDOR").ok();
        let original_mode = env::var("TRAINING_MODE").ok();
        
        // Test configuration 1: Real models with ruv_fann vendor
        env::set_var("NEURAL_VENDOR", "ruv_fann");
        env::set_var("TRAINING_MODE", "online");
        
        let config1 = NeuralConfig {
            use_real_models: true,
            models: vec!["LSTM".to_string(), "GRU".to_string()],
            input_size: 24,
            output_size: 6,
            hidden_layers: vec![128, 64, 32],
            learning_rate: 0.001,
            prediction_horizon: Some(6),
            ..Default::default()
        };
        
        let sector_mapper1 = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker1 = Arc::new(ModelPerformanceTracker::new());
        let result1 = VendorPredictor::new(&config1, sector_mapper1, performance_tracker1);
        assert!(result1.is_ok(), "Should handle real models with ruv_fann vendor");
        
        // Test configuration 2: Mock models with fallback vendor
        env::set_var("NEURAL_VENDOR", "mock");
        env::set_var("TRAINING_MODE", "disabled");
        
        let config2 = NeuralConfig {
            use_real_models: false,
            models: vec!["MLP".to_string()],
            input_size: 10,
            output_size: 1,
            hidden_layers: vec![20],
            learning_rate: 0.01,
            ..Default::default()
        };
        
        let sector_mapper2 = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker2 = Arc::new(ModelPerformanceTracker::new());
        let result2 = VendorPredictor::new(&config2, sector_mapper2, performance_tracker2);
        assert!(result2.is_ok(), "Should handle mock models with fallback vendor");
        
        // Restore original values
        match original_vendor {
            Some(val) => env::set_var("NEURAL_VENDOR", val),
            None => env::remove_var("NEURAL_VENDOR"),
        }
        match original_mode {
            Some(val) => env::set_var("TRAINING_MODE", val),
            None => env::remove_var("TRAINING_MODE"),
        }
    }
}
