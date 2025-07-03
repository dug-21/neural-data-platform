//! Data source adapters module
//! 
//! This module provides adapters for various data sources including
//! TimescaleDB for historical data and Redis for real-time streaming.

pub mod redis;
pub mod timescale;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Query error: {0}")]
    Query(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Common trait for all data adapters
#[async_trait]
pub trait DataAdapter: Send + Sync {
    /// Connect to the data source
    async fn connect(&mut self) -> Result<(), AdapterError>;
    
    /// Disconnect from the data source
    async fn disconnect(&mut self) -> Result<(), AdapterError>;
    
    /// Check if the adapter is connected
    fn is_connected(&self) -> bool;
    
    /// Get adapter name
    fn name(&self) -> &str;
}

/// Market data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Order book entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderBookEntry {
    pub price: f64,
    pub quantity: f64,
    pub timestamp: i64,
}

/// Order book snapshot
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub timestamp: i64,
    pub bids: Vec<OrderBookEntry>,
    pub asks: Vec<OrderBookEntry>,
}