//! Neural adapter module for neuro-divergent models
//!
//! This module provides specialized adapters for integrating with neuro-divergent
//! neural network models, handling data format conversions and model interactions.

// DELETED: pub mod data_converter; - removed with parent data_converter.rs
// neuro_divergent_adapter module removed - use enhanced_neural_adapter with FANN predictor
pub mod type_converter;
pub mod vendor_conversion;

// DELETED: pub use data_converter::{ConversionFormat, DataConverter}; - removed with data_converter.rs
// NeuroDivergentAdapter exports removed - use enhanced_neural_adapter with FANN predictor
pub use type_converter::*;
pub use vendor_conversion::*;

/// Re-export common types
pub use super::{AdapterError, DataAdapter};
