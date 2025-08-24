//! Neural Core - Shared foundation library for Neural Trader V2
//!
//! This crate provides common types, traits, and utilities shared across
//! all Neural Trader V2 binaries (neural-ml-ops, neural-trading).

// Module declarations - each module is <500 lines as per requirements
pub mod errors;
pub mod events;
pub mod interfaces;
pub mod traits;
pub mod types;

// Re-exports for convenience
pub use errors::{CoreError, Result};
pub use events::{Event, EventBus};
pub use interfaces::{
    MarketDataServiceTrait, FeatureEngineeringServiceTrait,
    ModelManagementServiceTrait, TradingServiceTrait,
    ServiceError, ServiceResult
};
pub use traits::{Predictor, Storage};
pub use types::{MarketData, Prediction, Signal};

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");