//! Neural adapter module for neuro-divergent models
//!
//! This module provides specialized adapters for integrating with neuro-divergent
//! neural network models, handling data format conversions and model interactions.

pub mod data_converter;
pub mod neuro_divergent_adapter;
pub mod type_converter;
pub mod vendor_conversion;

pub use data_converter::{ConversionFormat, DataConverter};
pub use neuro_divergent_adapter::{NeuralAdapterError, NeuroDivergentAdapter};
pub use type_converter::*;
pub use vendor_conversion::*;

/// Re-export common types
pub use super::{AdapterError, DataAdapter};
