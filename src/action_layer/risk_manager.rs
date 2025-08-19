//! Basic Risk Management System
//!
//! Implements essential risk controls for the MVP:
//! - Position size limits
//! - Daily loss limits
//! - Portfolio risk management
//! - Stop-loss validation

use crate::action_layer::{
    ActionLayerError, Order, OrderSide, Position, RiskLimits, RiskManager, TradingAccount
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;

pub struct BasicRiskManager {
    limits: RiskLimits,
    daily_start_equity: f64,
    session_start: chrono::DateTime<Utc>,
}

impl BasicRiskManager {
    pub fn new(limits: RiskLimits) -> Self {
        Self {
            limits,
            daily_start_equity: 0.0,
            session_start: Utc::now(),
        }
    }
    
    /// Initialize daily equity baseline
    pub fn set_daily_baseline(&mut self, equity: f64) {
        // Reset daily baseline if it's a new trading day
        let now = Utc::now();
        if now.date_naive() != self.session_start.date_naive() {
            self.daily_start_equity = equity;
            self.session_start = now;
        } else if self.daily_start_equity == 0.0 {
            self.daily_start_equity = equity;
        }
    }
    
    /// Calculate position risk as percentage of portfolio
    fn calculate_position_risk(&self, symbol: &str, quantity: f64, price: f64, portfolio_value: f64) -> f64 {
        let position_value = quantity * price;
        if portfolio_value > 0.0 {
            position_value / portfolio_value
        } else {
            1.0 // 100% risk if no portfolio value
        }
    }
    
    /// Calculate portfolio concentration for a symbol
    fn calculate_symbol_concentration(&self, symbol: &str, positions: &HashMap<String, Position>, new_quantity: f64, current_price: f64, portfolio_value: f64) -> f64 {
        let existing_exposure = positions.get(symbol)
            .map(|pos| pos.quantity * pos.current_price)
            .unwrap_or(0.0);
        
        let new_exposure = new_quantity * current_price;
        let total_exposure = existing_exposure + new_exposure;
        
        if portfolio_value > 0.0 {
            total_exposure / portfolio_value
        } else {
            1.0
        }
    }
    
    /// Check if daily loss limit is exceeded
    fn check_daily_loss(&self, account: &TradingAccount) -> bool {
        if self.daily_start_equity == 0.0 {
            return true; // Allow if no baseline set
        }
        
        let current_loss = (self.daily_start_equity - account.equity) / self.daily_start_equity;
        current_loss < self.limits.max_daily_loss
    }
    
    /// Check correlation-based exposure limits
    fn check_correlation_exposure(&self, symbol: &str, positions: &HashMap<String, Position>, new_quantity: f64, current_price: f64, portfolio_value: f64) -> bool {
        // Simplified correlation check - group by sector/asset class
        let sector_symbols = self.get_correlated_symbols(symbol);
        
        let total_correlated_exposure: f64 = positions.iter()
            .filter(|(sym, _)| sector_symbols.contains(sym))
            .map(|(_, pos)| pos.quantity * pos.current_price)
            .sum();
        
        let new_exposure = new_quantity * current_price;
        let total_exposure_ratio = (total_correlated_exposure + new_exposure) / portfolio_value;
        
        total_exposure_ratio <= self.limits.max_correlation_exposure
    }
    
    /// Get symbols that are considered correlated (simplified sector grouping)
    fn get_correlated_symbols(&self, symbol: &str) -> Vec<String> {
        // Simplified correlation mapping - in production, this would use actual correlation data
        match symbol {
            s if s.starts_with("AAPL") || s.starts_with("MSFT") || s.starts_with("GOOGL") || s.starts_with("AMZN") => {
                vec!["AAPL".to_string(), "MSFT".to_string(), "GOOGL".to_string(), "AMZN".to_string(), "META".to_string()]
            }
            s if s.starts_with("JPM") || s.starts_with("BAC") || s.starts_with("WFC") => {
                vec!["JPM".to_string(), "BAC".to_string(), "WFC".to_string(), "C".to_string()]
            }
            s if s.starts_with("XLF") || s.starts_with("XLK") || s.starts_with("XLY") => {
                vec!["XLF".to_string(), "XLK".to_string(), "XLY".to_string(), "SPY".to_string()]
            }
            _ => vec![symbol.to_string()], // No correlation assumed for unknown symbols
        }
    }
}

#[async_trait]
impl RiskManager for BasicRiskManager {
    async fn validate_order(&self, order: &Order, account: &TradingAccount) -> Result<bool, ActionLayerError> {
        // Check if daily loss limit exceeded
        if !self.check_daily_loss(account) {
            return Ok(false);
        }
        
        // Get current price for position sizing
        let current_price = match order.order_type {
            crate::action_layer::OrderType::Market => {
                // For market orders, we need current price - this would come from market data
                // For MVP, we'll use a placeholder or the limit price if available
                order.price.unwrap_or(100.0) // Placeholder
            }
            crate::action_layer::OrderType::Limit => {
                order.price.ok_or_else(|| ActionLayerError::RiskLimitExceeded(
                    "Limit order requires price".to_string()
                ))?
            }
            crate::action_layer::OrderType::StopLoss => {
                order.price.ok_or_else(|| ActionLayerError::RiskLimitExceeded(
                    "Stop loss order requires price".to_string()
                ))?
            }
        };
        
        // Check position size limit
        let position_risk = self.calculate_position_risk(
            &order.symbol, 
            order.quantity, 
            current_price, 
            account.portfolio_value
        );
        
        if position_risk > self.limits.max_position_size {
            return Ok(false);
        }
        
        // Check symbol concentration
        let concentration = self.calculate_symbol_concentration(
            &order.symbol,
            &account.positions,
            order.quantity,
            current_price,
            account.portfolio_value
        );
        
        if concentration > self.limits.max_position_size * 2.0 { // Allow 2x position size for concentration
            return Ok(false);
        }
        
        // Check correlation exposure
        if !self.check_correlation_exposure(
            &order.symbol,
            &account.positions,
            order.quantity,
            current_price,
            account.portfolio_value
        ) {
            return Ok(false);
        }
        
        // Check buying power
        let order_value = order.quantity * current_price;
        if matches!(order.side, OrderSide::Buy) && order_value > account.buying_power {
            return Ok(false);
        }
        
        // Check if selling more than owned
        if matches!(order.side, OrderSide::Sell) {
            if let Some(position) = account.positions.get(&order.symbol) {
                if matches!(position.side, crate::action_layer::PositionSide::Long) && order.quantity > position.quantity {
                    return Ok(false);
                }
            } else {
                // Trying to sell without a position (short selling not allowed in MVP)
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    async fn check_position_limits(&self, symbol: &str, quantity: f64, account: &TradingAccount) -> Result<bool, ActionLayerError> {
        // This is a simplified check - would normally use current market price
        let estimated_price = 100.0; // Placeholder
        let position_risk = self.calculate_position_risk(symbol, quantity, estimated_price, account.portfolio_value);
        
        Ok(position_risk <= self.limits.max_position_size)
    }
    
    async fn check_daily_limits(&self, account: &TradingAccount) -> Result<bool, ActionLayerError> {
        Ok(self.check_daily_loss(account))
    }
    
    async fn calculate_position_size(&self, signal_strength: f64, account: &TradingAccount) -> Result<f64, ActionLayerError> {
        // Kelly criterion approximation for position sizing
        // Position size = (signal_strength * win_rate - (1 - win_rate)) / signal_strength
        
        let base_position_size = self.limits.max_position_size;
        let win_rate = 0.55; // Assumed 55% win rate for MVP
        
        // Apply signal strength and risk scaling
        let kelly_fraction = (signal_strength * win_rate - (1.0 - win_rate)) / signal_strength;
        let risk_adjusted_size = base_position_size * kelly_fraction.max(0.0).min(1.0);
        
        // Apply volatility adjustment (simplified)
        let volatility_adjustment = 0.8; // Assume 20% volatility reduction
        let final_size = risk_adjusted_size * volatility_adjustment;
        
        // Ensure minimum and maximum bounds
        let position_value = final_size * account.portfolio_value;
        let min_position = 1000.0; // Minimum $1000 position
        let max_position = account.portfolio_value * self.limits.max_position_size;
        
        Ok(position_value.max(min_position).min(max_position) / account.portfolio_value)
    }
}

/// Risk assessment result
#[derive(Debug)]
pub struct RiskAssessment {
    pub passed: bool,
    pub risk_score: f64,
    pub violations: Vec<String>,
    pub recommendations: Vec<String>,
}

impl BasicRiskManager {
    /// Perform comprehensive risk assessment
    pub async fn assess_risk(&self, order: &Order, account: &TradingAccount) -> Result<RiskAssessment, ActionLayerError> {
        let mut violations = Vec::new();
        let mut recommendations = Vec::new();
        let mut risk_score = 0.0;
        
        // Check daily loss limit
        if !self.check_daily_loss(account) {
            violations.push("Daily loss limit exceeded".to_string());
            recommendations.push("Stop trading for the day".to_string());
            risk_score += 0.3;
        }
        
        // Check position sizing
        let estimated_price = order.price.unwrap_or(100.0);
        let position_risk = self.calculate_position_risk(
            &order.symbol, 
            order.quantity, 
            estimated_price, 
            account.portfolio_value
        );
        
        if position_risk > self.limits.max_position_size {
            violations.push(format!("Position size too large: {:.2}% > {:.2}%", 
                position_risk * 100.0, 
                self.limits.max_position_size * 100.0
            ));
            recommendations.push("Reduce position size".to_string());
            risk_score += 0.2;
        }
        
        // Check portfolio concentration
        let concentration = self.calculate_symbol_concentration(
            &order.symbol,
            &account.positions,
            order.quantity,
            estimated_price,
            account.portfolio_value
        );
        
        if concentration > self.limits.max_position_size * 2.0 {
            violations.push("Excessive concentration in symbol".to_string());
            recommendations.push("Diversify positions".to_string());
            risk_score += 0.15;
        }
        
        // Check buying power
        let order_value = order.quantity * estimated_price;
        if matches!(order.side, OrderSide::Buy) && order_value > account.buying_power {
            violations.push("Insufficient buying power".to_string());
            recommendations.push("Reduce order size or add capital".to_string());
            risk_score += 0.25;
        }
        
        // Calculate overall risk score (0.0 = low risk, 1.0 = high risk)
        risk_score = risk_score.min(1.0);
        
        Ok(RiskAssessment {
            passed: violations.is_empty(),
            risk_score,
            violations,
            recommendations,
        })
    }
}