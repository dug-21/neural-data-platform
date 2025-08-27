//! Core traits for Neural Trader V2
//! Module size: <50 lines as per requirements

pub mod storage;
pub mod predictor;

// Re-exports
pub use storage::{Storage, StorageBackend, StorageConfig};
pub use predictor::{Predictor, ModelMetrics, TrainingConfig};