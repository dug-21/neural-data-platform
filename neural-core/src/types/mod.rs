//! Core types shared across Neural Trader V2 binaries
//! Module size: <50 lines

pub mod market;
pub mod prediction;
pub mod trading;

// Re-exports
pub use market::{MarketData, MarketContext, PriceBar};
pub use prediction::{Prediction, PredictionResult, ModelOutput};
pub use trading::{Signal, TradingDecision, TradingAction, Position};

// Common type aliases
pub type Symbol = String;
pub type Price = f64;
pub type Volume = u64;
pub type Confidence = f64;