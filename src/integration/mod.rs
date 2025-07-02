//! Integration module for external services and APIs
//! 
//! This module provides integration with various external services including:
//! - Market data providers
//! - Trading platforms
//! - Analytics services
//! - External neural model providers
//! - Data access layer for DAA agents
//! - DAA-FANN neural prediction integration

use anyhow::Result;

pub mod data_access;
pub mod neural_predictions;
pub mod daa_fann;
pub mod platform_orchestrator;
pub mod streaming;

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

// TODO: Implement specific providers like Binance, Coinbase, etc.