//! Market data types
//! Module size: <300 lines as per requirements

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::errors::{CoreError, Result};

/// Core market data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    symbol: String,
    price: f64,
    volume: u64,
    timestamp: DateTime<Utc>,
    bid: Option<f64>,
    ask: Option<f64>,
    metadata: MarketMetadata,
}

impl MarketData {
    /// Create new market data instance
    pub fn new(symbol: String, price: f64, volume: u64, timestamp: DateTime<Utc>) -> Self {
        Self {
            symbol,
            price,
            volume,
            timestamp,
            bid: None,
            ask: None,
            metadata: MarketMetadata::default(),
        }
    }
    
    /// Validate market data
    pub fn validate(&self) -> Result<()> {
        if self.price < 0.0 {
            return Err(CoreError::Validation("Price cannot be negative".into()));
        }
        if self.symbol.is_empty() {
            return Err(CoreError::Validation("Symbol cannot be empty".into()));
        }
        Ok(())
    }
    
    // Getters
    pub fn symbol(&self) -> &String { &self.symbol }
    pub fn price(&self) -> f64 { self.price }
    pub fn volume(&self) -> u64 { self.volume }
    pub fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
    pub fn bid(&self) -> Option<f64> { self.bid }
    pub fn ask(&self) -> Option<f64> { self.ask }
}

/// Market context for decision making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub symbol: String,
    pub current_price: f64,
    pub volume_24h: u64,
    pub volatility: f64,
    pub trend: MarketTrend,
    pub regime: MarketRegime,
    pub timestamp: DateTime<Utc>,
}

/// Market trend direction
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MarketTrend {
    Bullish,
    Bearish,
    Neutral,
}

/// Market regime classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MarketRegime {
    Trending,
    RangeBound,
    Volatile,
    Quiet,
}

/// OHLCV price bar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceBar {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub timestamp: DateTime<Utc>,
}

impl PriceBar {
    pub fn new(open: f64, high: f64, low: f64, close: f64, volume: u64, timestamp: DateTime<Utc>) -> Self {
        Self { open, high, low, close, volume, timestamp }
    }
    
    /// Calculate bar range
    pub fn range(&self) -> f64 {
        self.high - self.low
    }
    
    /// Calculate bar body size
    pub fn body(&self) -> f64 {
        (self.close - self.open).abs()
    }
    
    /// Is bullish bar
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }
}

/// Market metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketMetadata {
    pub exchange: Option<String>,
    pub data_source: Option<String>,
    pub latency_ms: Option<u32>,
    pub sequence_number: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_price_bar_calculations() {
        let bar = PriceBar::new(100.0, 105.0, 99.0, 103.0, 1000000, Utc::now());
        
        assert_eq!(bar.range(), 6.0);
        assert_eq!(bar.body(), 3.0);
        assert!(bar.is_bullish());
    }
    
    #[test]
    fn test_market_data_validation() {
        let invalid = MarketData::new("".to_string(), 100.0, 1000, Utc::now());
        assert!(invalid.validate().is_err());
        
        let valid = MarketData::new("AAPL".to_string(), 100.0, 1000, Utc::now());
        assert!(valid.validate().is_ok());
    }
}