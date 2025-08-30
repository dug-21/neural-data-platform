//! DAA Decision Types and Logic
//!
//! Types and logic for autonomous trading decisions.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Autonomous decision from DAA
#[derive(Debug, Clone)]
pub struct AutonomousDecision {
    pub timestamp: DateTime<Utc>,
    pub action: TradingAction,
    pub confidence: f64,
    pub risk_assessment: RiskAssessment,
    pub reasoning: Vec<String>,
    pub neural_consensus: HashMap<String, f64>,
    pub adapted_parameters: Option<HashMap<String, f64>>,
}

#[derive(Debug, Clone)]
pub enum TradingAction {
    Buy {
        symbol: String,
        size: f64,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    },
    Sell {
        symbol: String,
        size: f64,
        reason: String,
    },
    Hold {
        reason: String,
    },
    AdjustPosition {
        symbol: String,
        new_stop_loss: Option<f64>,
        new_take_profit: Option<f64>,
    },
}

#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub market_risk: f64,
    pub position_risk: f64,
    pub portfolio_risk: f64,
    pub volatility_adjusted_size: f64,
}