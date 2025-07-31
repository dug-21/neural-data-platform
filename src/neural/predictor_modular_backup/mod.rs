//! Neural Predictor Module
//!
//! This module contains the modularized components of the FANN predictor system,
//! split from the original monolithic fann_predictor.rs file for better maintainability.

pub mod core;
pub mod networks;
pub mod training;
pub mod conversion;
pub mod cache;
pub mod factory;
pub mod persistence;

// Re-export main types for backward compatibility
pub use core::{FannPredictor, ModelConfig, TrainingResult};
pub use networks::{NetworkManager, ModelKey, RecurrentState};
pub use training::{OnlineTrainingManager, ConceptDriftDetector, OnlinePerformanceMetrics};
pub use conversion::{DataConverter, ModelPerformance};
pub use cache::{NetworkCache, CacheManager};
pub use factory::{NetworkFactory, FannModelConfig};
pub use persistence::{ModelPersistence, EnsembleManager, StreamingConfig};