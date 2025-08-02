//! Test modules for neural network functionality
//!
//! This module organizes comprehensive tests for all neural network components
//! including the enhanced predictor, FANN integration, and DAA coordination.
//!
//! Test Coverage:
//! - Enhanced Neural Predictor (85%+ coverage)
//! - FANN-based Neural Predictor
//! - DAA Integration Tests  
//! - Performance Benchmarks and Stress Tests

pub mod test_daa_integration;
pub mod test_enhanced_predictor;
pub mod test_fann_predictor;
pub mod test_feature_flag;
pub mod test_performance_benchmarks;
pub mod test_performance_channel;
pub mod test_performance_regression;
pub mod test_sector_aggregator;
pub mod test_predict_enhanced_fix;
pub mod test_real_models_integration;

// Re-export test utilities for other test modules
pub use test_daa_integration::*;
pub use test_enhanced_predictor::*;
pub use test_fann_predictor::*;
pub use test_feature_flag::*;
pub use test_performance_benchmarks::*;
pub use test_real_models_integration::*;
