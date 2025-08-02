//! Integration module for external services and APIs
//!
//! This module provides integration with various external services including:
//! - Market data providers
//! - Trading platforms
//! - Analytics services
//! - Data access layer for DAA agents

use anyhow::Result;

pub mod autonomous_decisions;
pub mod autonomous_neural_coordinator;
// Original monolithic module (Phase 3B will use this)
pub mod daa_coordinator;

// Refactored modular version (available for Phase 3C migration)
pub mod daa_coordinator_modular {
    pub mod config;
    pub mod core; 
    pub mod decisions;
    pub mod strategies;
    pub mod agents;
}
pub mod data_access;
pub mod training_data_service;
pub mod model_persistence_service;

// Phase 3B: Removed architectural layers - these should NOT exist
// Only simple field additions allowed, no new patterns!

// Re-export commonly used types
pub use autonomous_decisions::{DaaDecisionMaker, MarketTrend};
pub use daa_coordinator::{
    AutonomousDecision, DaaConfig, DaaCoordinator, TradingAction,
    SectorDAACoordinator, SectorDAAConfig, SectorAwareDecision, 
    SectorDecisionContext, SectorMetrics
};
pub use training_data_service::{
    ModelType, PreparedTrainingData, TrainingDataConfig, TrainingDataService, ValidationError,
};
pub use model_persistence_service::{
    ModelPersistenceService, ModelPersistenceConfig, ModelOperation, ModelOperationResult,
};

// Phase 3B: Removed re-exports of architectural components
// These were mistakenly added and violate Phase 3B requirements

/// Trait for market data providers
pub trait MarketDataProvider: Send + Sync {
    /// Get real-time market data
    async fn get_real_time_data(&self, symbol: &str) -> Result<crate::data::TimeSeriesData>;

    /// Subscribe to market data stream
    async fn subscribe(&self, symbols: Vec<String>) -> Result<()>;

    /// Unsubscribe from market data stream
    async fn unsubscribe(&self, symbols: Vec<String>) -> Result<()>;
}

/// Trait for trading platform integration
pub trait TradingPlatform: Send + Sync {
    /// Execute a trade
    async fn execute_trade(&self, order: TradeOrder) -> Result<TradeResult>;

    /// Get account balance
    async fn get_balance(&self) -> Result<AccountBalance>;

    /// Get open positions
    async fn get_positions(&self) -> Result<Vec<Position>>;
}

/// Trade order structure
#[derive(Debug, Clone)]
pub struct TradeOrder {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub order_type: OrderType,
    pub price: Option<f64>,
}

/// Order side
#[derive(Debug, Clone)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

/// Trade result
#[derive(Debug, Clone)]
pub struct TradeResult {
    pub order_id: String,
    pub status: OrderStatus,
    pub executed_price: Option<f64>,
    pub executed_quantity: Option<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Order status
#[derive(Debug, Clone)]
pub enum OrderStatus {
    Pending,
    Filled,
    PartiallyFilled,
    Cancelled,
    Rejected,
}

/// Account balance
#[derive(Debug, Clone)]
pub struct AccountBalance {
    pub currency: String,
    pub total: f64,
    pub available: f64,
    pub locked: f64,
}

/// Trading position
#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

// Provider implementations available in separate modules:
// - binance: crate::adapters::binance_adapter
// - coinbase: crate::adapters::coinbase_adapter
// - polygon: crate::features::dataIngestion::providers::polygon
// Add additional provider modules as needed for exchange integrations
