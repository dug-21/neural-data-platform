//! Neural Trading Library
//!
//! Core trading execution engine with machine learning inference,
//! risk management, and DAA coordination.


pub mod daa;
pub mod execution;
pub mod risk;
pub mod inference;
pub mod events;

// Re-export commonly used types (commented out until modules are properly structured)
// pub use execution::orders::{Order, OrderManager, OrderSide, OrderType};
// pub use inference::predictor::NeuralPredictor;
// pub use inference::cache::InferenceCache;
// pub use daa::coordinator::DAACoordinator;
// pub use execution::engine::ExecutionEngine;
// pub use risk::manager::RiskManager;

#[derive(Debug, Clone)]
pub struct TradingConfig {
    pub redis_url: String,
    pub postgres_url: String,
    pub broker_endpoint: String,
    pub neural_model_path: String,
    pub risk_limits: RiskLimits,
    pub execution_params: ExecutionParams,
}

#[derive(Debug, Clone)]
pub struct RiskLimits {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub max_drawdown: f64,
    pub max_correlation_exposure: f64,
}

#[derive(Debug, Clone)]
pub struct ExecutionParams {
    pub order_timeout_ms: u64,
    pub max_slippage_bps: u32,
    pub min_confidence_threshold: f64,
    pub max_orders_per_minute: u32,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            postgres_url: "postgresql://localhost/neural_trader".to_string(),
            broker_endpoint: "http://localhost:8080".to_string(),
            neural_model_path: "./models/latest.pt".to_string(),
            risk_limits: RiskLimits::default(),
            execution_params: ExecutionParams::default(),
        }
    }
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size: 0.05,
            max_daily_loss: 0.02,
            max_drawdown: 0.10,
            max_correlation_exposure: 0.20,
        }
    }
}

impl Default for ExecutionParams {
    fn default() -> Self {
        Self {
            order_timeout_ms: 5000,
            max_slippage_bps: 10,
            min_confidence_threshold: 0.7,
            max_orders_per_minute: 100,
        }
    }
}