//! Trading decision and signal types
//! Module size: <200 lines as per requirements

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::types::prediction::Prediction;

/// Trading signal from strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    symbol: String,
    action: TradingAction,
    strength: f64,
    strategy_name: String,
    timestamp: DateTime<Utc>,
}

impl Signal {
    /// Create buy signal
    pub fn buy(symbol: &str, strength: f64) -> Self {
        Self {
            symbol: symbol.to_string(),
            action: TradingAction::Buy,
            strength: strength.max(0.0).min(1.0), // Normalize to [0, 1]
            strategy_name: "default".to_string(),
            timestamp: Utc::now(),
        }
    }
    
    /// Create sell signal
    pub fn sell(symbol: &str, strength: f64) -> Self {
        Self {
            symbol: symbol.to_string(),
            action: TradingAction::Sell,
            strength: strength.max(0.0).min(1.0),
            strategy_name: "default".to_string(),
            timestamp: Utc::now(),
        }
    }
    
    // Getters
    pub fn symbol(&self) -> &str { &self.symbol }
    pub fn action(&self) -> TradingAction { self.action }
    pub fn strength(&self) -> f64 { self.strength }
    pub fn strategy_name(&self) -> &str { &self.strategy_name }
}

/// Trading action enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
    Close,
}

/// Trading decision from DAA Coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingDecision {
    action: TradingAction,
    symbol: String,
    confidence: f64,
    quantity: Option<u32>,
    price_target: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    reasoning: String,
    timestamp: DateTime<Utc>,
}

impl TradingDecision {
    /// Create decision from prediction
    pub fn from_prediction(prediction: &Prediction, current_price: f64) -> Self {
        let action = if prediction.value() > current_price * 1.01 {
            TradingAction::Buy
        } else if prediction.value() < current_price * 0.99 {
            TradingAction::Sell
        } else {
            TradingAction::Hold
        };
        
        Self {
            action,
            symbol: "UNKNOWN".to_string(),
            confidence: prediction.confidence(),
            quantity: None,
            price_target: Some(prediction.value()),
            stop_loss: None,
            take_profit: None,
            reasoning: format!("Prediction: {:.2}, Current: {:.2}", prediction.value(), current_price),
            timestamp: Utc::now(),
        }
    }
    
    // Getters
    pub fn action(&self) -> TradingAction { self.action }
    pub fn confidence(&self) -> f64 { self.confidence }
    pub fn symbol(&self) -> &str { &self.symbol }
}

/// Position tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: i32,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Position {
    /// Calculate current P&L
    pub fn calculate_pnl(&mut self, current_price: f64) {
        self.current_price = current_price;
        self.unrealized_pnl = (current_price - self.entry_price) * self.quantity as f64;
        self.updated_at = Utc::now();
    }
    
    /// Check if position is profitable
    pub fn is_profitable(&self) -> bool {
        self.unrealized_pnl > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_signal_strength_normalization() {
        let signal = Signal::buy("AAPL", 2.5);
        assert_eq!(signal.strength(), 1.0);
        
        let signal2 = Signal::sell("AAPL", -0.5);
        assert_eq!(signal2.strength(), 0.0);
    }
    
    #[test]
    fn test_position_pnl_calculation() {
        let mut position = Position {
            symbol: "AAPL".to_string(),
            quantity: 100,
            entry_price: 150.0,
            current_price: 150.0,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        position.calculate_pnl(155.0);
        assert_eq!(position.unrealized_pnl, 500.0);
        assert!(position.is_profitable());
    }
}