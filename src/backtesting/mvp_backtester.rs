//! MVP Backtesting Engine
//!
//! Simple backtesting framework to validate neural network predictions
//! Focuses on essential performance metrics without complex portfolio management

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, info};

use crate::neural::mvp_predictor::{MVPPredictionResult, TradingDecision};

/// Backtest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// Initial capital in USD
    pub initial_capital: f64,
    /// Transaction cost per trade (fraction, e.g., 0.001 = 0.1%)
    pub transaction_cost: f64,
    /// Position size as fraction of capital (e.g., 0.1 = 10%)
    pub position_size: f64,
    /// Maximum number of concurrent positions
    pub max_positions: u32,
    /// Risk-free rate for Sharpe ratio calculation (annual)
    pub risk_free_rate: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            transaction_cost: 0.001, // 0.1% per trade
            position_size: 0.1,      // 10% of capital
            max_positions: 1,        // Single position for MVP
            risk_free_rate: 0.02,    // 2% annual risk-free rate
        }
    }
}

/// Individual trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Trade entry timestamp
    pub entry_time: DateTime<Utc>,
    /// Trade exit timestamp
    pub exit_time: DateTime<Utc>,
    /// Trade direction (Buy/Sell)
    pub direction: TradingDecision,
    /// Entry price
    pub entry_price: f64,
    /// Exit price
    pub exit_price: f64,
    /// Position size (number of shares)
    pub shares: f64,
    /// Gross profit/loss
    pub gross_pnl: f64,
    /// Net profit/loss (after costs)
    pub net_pnl: f64,
    /// Transaction costs
    pub costs: f64,
    /// Holding period in days
    pub holding_days: f64,
}

impl Trade {
    /// Calculate trade return as percentage
    pub fn return_pct(&self) -> f64 {
        if self.entry_price > 0.0 {
            (self.exit_price - self.entry_price) / self.entry_price
        } else {
            0.0
        }
    }
    
    /// Check if trade was profitable
    pub fn is_profitable(&self) -> bool {
        self.net_pnl > 0.0
    }
}

/// Portfolio position
#[derive(Debug, Clone)]
struct Position {
    entry_time: DateTime<Utc>,
    direction: TradingDecision,
    shares: f64,
    entry_price: f64,
    entry_value: f64,
}

/// Comprehensive backtest results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// Test period
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    
    /// Portfolio performance
    pub initial_capital: f64,
    pub final_capital: f64,
    pub total_return: f64,
    pub annual_return: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    
    /// Trade statistics
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub avg_holding_days: f64,
    
    /// Risk metrics
    pub volatility: f64,
    pub downside_deviation: f64,
    pub var_95: f64, // Value at Risk 95%
    pub max_consecutive_losses: u32,
    
    /// Benchmark comparison
    pub benchmark_return: f64,
    pub alpha: f64,
    pub beta: f64,
    pub information_ratio: f64,
    
    /// All individual trades
    pub trades: Vec<Trade>,
    /// Daily portfolio values
    pub equity_curve: Vec<(DateTime<Utc>, f64)>,
}

/// MVP Backtester - Simple implementation focused on essentials
pub struct MVPBacktester {
    config: BacktestConfig,
    current_capital: f64,
    positions: Vec<Position>,
    trades: Vec<Trade>,
    equity_curve: Vec<(DateTime<Utc>, f64)>,
    max_capital: f64,
    daily_returns: Vec<f64>,
}

impl MVPBacktester {
    /// Create new backtester with configuration
    pub fn new(config: BacktestConfig) -> Self {
        info!("🔄 MVP Backtester initialized with ${:.0} capital", config.initial_capital);
        
        Self {
            current_capital: config.initial_capital,
            max_capital: config.initial_capital,
            config,
            positions: Vec::new(),
            trades: Vec::new(),
            equity_curve: Vec::new(),
            daily_returns: Vec::new(),
        }
    }
    
    /// Run backtest with predictions and actual prices
    pub fn run_backtest(
        &mut self,
        predictions: &[MVPPredictionResult],
        actual_prices: &[f64],
        timestamps: &[DateTime<Utc>],
    ) -> Result<BacktestResult> {
        if predictions.len() != actual_prices.len() || predictions.len() != timestamps.len() {
            return Err(anyhow!("Mismatched data lengths"));
        }
        
        if predictions.is_empty() {
            return Err(anyhow!("No prediction data provided"));
        }
        
        info!("🚀 Running backtest with {} predictions", predictions.len());
        
        self.reset();
        
        // Process each prediction
        for i in 0..predictions.len() {
            let prediction = &predictions[i];
            let current_price = actual_prices[i];
            let timestamp = timestamps[i];
            
            // Handle existing positions first
            self.process_positions(current_price, timestamp);
            
            // Make new trading decisions
            self.process_prediction(prediction, current_price, timestamp)?;
            
            // Record portfolio value
            let portfolio_value = self.calculate_portfolio_value(current_price);
            self.equity_curve.push((timestamp, portfolio_value));
            
            // Track daily returns
            if i > 0 {
                let prev_value = self.equity_curve[i - 1].1;
                let daily_return = (portfolio_value - prev_value) / prev_value;
                self.daily_returns.push(daily_return);
            }
            
            self.max_capital = self.max_capital.max(portfolio_value);
        }
        
        // Close any remaining positions
        if let (Some(last_price), Some(last_timestamp)) = (actual_prices.last(), timestamps.last()) {
            self.close_all_positions(*last_price, *last_timestamp);
        }
        
        // Calculate and return results
        let result = self.calculate_results(predictions, actual_prices, timestamps)?;
        
        info!("✅ Backtest completed: {:.1}% return, {:.1}% win rate, {:.2} Sharpe", 
              result.total_return * 100.0, result.win_rate * 100.0, result.sharpe_ratio);
        
        Ok(result)
    }
    
    /// Reset backtester state
    fn reset(&mut self) {
        self.current_capital = self.config.initial_capital;
        self.max_capital = self.config.initial_capital;
        self.positions.clear();
        self.trades.clear();
        self.equity_curve.clear();
        self.daily_returns.clear();
    }
    
    /// Process existing positions (check for exits)
    fn process_positions(&mut self, current_price: f64, timestamp: DateTime<Utc>) {
        let mut positions_to_close = Vec::new();
        
        for (i, position) in self.positions.iter().enumerate() {
            // Simple exit strategy: hold for 1 day
            let holding_period = timestamp - position.entry_time;
            if holding_period >= Duration::days(1) {
                positions_to_close.push(i);
            }
        }
        
        // Close positions (reverse order to maintain indices)
        for &pos_idx in positions_to_close.iter().rev() {
            self.close_position(pos_idx, current_price, timestamp);
        }
    }
    
    /// Process new prediction and make trading decision
    fn process_prediction(
        &mut self, 
        prediction: &MVPPredictionResult, 
        current_price: f64, 
        timestamp: DateTime<Utc>
    ) -> Result<()> {
        // Only trade if we have available capital and no current positions (for MVP)
        if !self.positions.is_empty() || self.current_capital < self.config.initial_capital * 0.1 {
            return Ok(());
        }
        
        match prediction.decision {
            TradingDecision::Buy | TradingDecision::Sell => {
                self.open_position(prediction, current_price, timestamp)?;
            }
            TradingDecision::Hold => {
                // No action
            }
        }
        
        Ok(())
    }
    
    /// Open new position based on prediction
    fn open_position(
        &mut self, 
        prediction: &MVPPredictionResult, 
        price: f64, 
        timestamp: DateTime<Utc>
    ) -> Result<()> {
        let position_value = self.current_capital * self.config.position_size;
        let shares = position_value / price;
        let transaction_cost = position_value * self.config.transaction_cost;
        
        // Check if we have enough capital
        if self.current_capital < position_value + transaction_cost {
            return Ok(());
        }
        
        let position = Position {
            entry_time: timestamp,
            direction: prediction.decision,
            shares,
            entry_price: price,
            entry_value: position_value,
        };
        
        self.positions.push(position);
        self.current_capital -= position_value + transaction_cost;
        
        debug!("📈 Opened {} position: {} shares @ ${:.2} (cost: ${:.2})", 
               prediction.decision, shares, price, transaction_cost);
        
        Ok(())
    }
    
    /// Close specific position
    fn close_position(&mut self, position_idx: usize, price: f64, timestamp: DateTime<Utc>) {
        if position_idx >= self.positions.len() {
            return;
        }
        
        let position = self.positions.remove(position_idx);
        let exit_value = position.shares * price;
        let transaction_cost = exit_value * self.config.transaction_cost;
        let net_proceeds = exit_value - transaction_cost;
        
        // Calculate P&L
        let gross_pnl = match position.direction {
            TradingDecision::Buy => exit_value - position.entry_value,
            TradingDecision::Sell => position.entry_value - exit_value,
            TradingDecision::Hold => 0.0,
        };
        
        let total_costs = position.entry_value * self.config.transaction_cost + transaction_cost;
        let net_pnl = gross_pnl - total_costs;
        
        // Create trade record
        let trade = Trade {
            entry_time: position.entry_time,
            exit_time: timestamp,
            direction: position.direction,
            entry_price: position.entry_price,
            exit_price: price,
            shares: position.shares,
            gross_pnl,
            net_pnl,
            costs: total_costs,
            holding_days: (timestamp - position.entry_time).num_days() as f64,
        };
        
        self.trades.push(trade);
        self.current_capital += net_proceeds;
        
        debug!("📉 Closed position: ${:.2} P&L (${:.2} gross, ${:.2} costs)", 
               net_pnl, gross_pnl, total_costs);
    }
    
    /// Close all open positions
    fn close_all_positions(&mut self, price: f64, timestamp: DateTime<Utc>) {
        let position_count = self.positions.len();
        for _ in 0..position_count {
            self.close_position(0, price, timestamp); // Always close first position
        }
    }
    
    /// Calculate current portfolio value
    fn calculate_portfolio_value(&self, current_price: f64) -> f64 {
        let position_value: f64 = self.positions.iter()
            .map(|pos| pos.shares * current_price)
            .sum();
        
        self.current_capital + position_value
    }
    
    /// Calculate comprehensive backtest results
    fn calculate_results(
        &self,
        predictions: &[MVPPredictionResult],
        actual_prices: &[f64],
        timestamps: &[DateTime<Utc>],
    ) -> Result<BacktestResult> {
        if self.equity_curve.is_empty() || timestamps.is_empty() {
            return Err(anyhow!("No data to calculate results"));
        }
        
        let start_date = timestamps[0];
        let end_date = *timestamps.last().unwrap();
        let final_capital = self.equity_curve.last().unwrap().1;
        
        // Calculate basic performance metrics
        let total_return = (final_capital - self.config.initial_capital) / self.config.initial_capital;
        let days = (end_date - start_date).num_days() as f64;
        let annual_return = if days > 0.0 {
            (1.0 + total_return).powf(365.0 / days) - 1.0
        } else {
            0.0
        };
        
        // Calculate drawdown
        let max_drawdown = self.calculate_max_drawdown();
        
        // Calculate volatility and Sharpe ratio
        let volatility = self.calculate_volatility();
        let sharpe_ratio = self.calculate_sharpe_ratio(annual_return, volatility);
        
        // Trade statistics
        let total_trades = self.trades.len();
        let winning_trades = self.trades.iter().filter(|t| t.is_profitable()).count();
        let losing_trades = total_trades - winning_trades;
        let win_rate = if total_trades > 0 { winning_trades as f64 / total_trades as f64 } else { 0.0 };
        
        let (avg_win, avg_loss) = self.calculate_avg_win_loss();
        let profit_factor = if avg_loss.abs() > 0.0 { avg_win / avg_loss.abs() } else { 0.0 };
        
        let avg_holding_days = if !self.trades.is_empty() {
            self.trades.iter().map(|t| t.holding_days).sum::<f64>() / self.trades.len() as f64
        } else {
            0.0
        };
        
        // Risk metrics
        let downside_deviation = self.calculate_downside_deviation();
        let sortino_ratio = if downside_deviation > 0.0 {
            (annual_return - self.config.risk_free_rate) / downside_deviation
        } else {
            0.0
        };
        let calmar_ratio = if max_drawdown > 0.0 { annual_return / max_drawdown } else { 0.0 };
        let var_95 = self.calculate_var_95();
        let max_consecutive_losses = self.calculate_max_consecutive_losses();
        
        // Benchmark comparison (buy and hold)
        let benchmark_return = self.calculate_benchmark_return(actual_prices);
        let (alpha, beta, information_ratio) = self.calculate_alpha_beta_ir(annual_return, benchmark_return);
        
        Ok(BacktestResult {
            start_date,
            end_date,
            initial_capital: self.config.initial_capital,
            final_capital,
            total_return,
            annual_return,
            max_drawdown,
            sharpe_ratio,
            sortino_ratio,
            calmar_ratio,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            avg_win,
            avg_loss,
            profit_factor,
            avg_holding_days,
            volatility,
            downside_deviation,
            var_95,
            max_consecutive_losses,
            benchmark_return,
            alpha,
            beta,
            information_ratio,
            trades: self.trades.clone(),
            equity_curve: self.equity_curve.clone(),
        })
    }
    
    /// Calculate maximum drawdown
    fn calculate_max_drawdown(&self) -> f64 {
        let mut max_dd = 0.0;
        let mut peak = self.config.initial_capital;
        
        for &(_timestamp, value) in &self.equity_curve {
            if value > peak {
                peak = value;
            }
            let drawdown = (peak - value) / peak;
            max_dd = max_dd.max(drawdown);
        }
        
        max_dd
    }
    
    /// Calculate portfolio volatility
    fn calculate_volatility(&self) -> f64 {
        if self.daily_returns.len() < 2 {
            return 0.0;
        }
        
        let mean_return = self.daily_returns.iter().sum::<f64>() / self.daily_returns.len() as f64;
        let variance = self.daily_returns.iter()
            .map(|&r| (r - mean_return).powi(2))
            .sum::<f64>() / (self.daily_returns.len() - 1) as f64;
        
        variance.sqrt() * (252.0_f64).sqrt() // Annualized
    }
    
    /// Calculate Sharpe ratio
    fn calculate_sharpe_ratio(&self, annual_return: f64, volatility: f64) -> f64 {
        if volatility > 0.0 {
            (annual_return - self.config.risk_free_rate) / volatility
        } else {
            0.0
        }
    }
    
    /// Calculate average win and loss
    fn calculate_avg_win_loss(&self) -> (f64, f64) {
        let wins: Vec<f64> = self.trades.iter()
            .filter(|t| t.is_profitable())
            .map(|t| t.net_pnl)
            .collect();
        
        let losses: Vec<f64> = self.trades.iter()
            .filter(|t| !t.is_profitable())
            .map(|t| t.net_pnl)
            .collect();
        
        let avg_win = if !wins.is_empty() { wins.iter().sum::<f64>() / wins.len() as f64 } else { 0.0 };
        let avg_loss = if !losses.is_empty() { losses.iter().sum::<f64>() / losses.len() as f64 } else { 0.0 };
        
        (avg_win, avg_loss)
    }
    
    /// Calculate downside deviation
    fn calculate_downside_deviation(&self) -> f64 {
        let downside_returns: Vec<f64> = self.daily_returns.iter()
            .filter(|&&r| r < 0.0)
            .cloned()
            .collect();
        
        if downside_returns.is_empty() {
            return 0.0;
        }
        
        let mean_downside = downside_returns.iter().sum::<f64>() / downside_returns.len() as f64;
        let downside_variance = downside_returns.iter()
            .map(|&r| (r - mean_downside).powi(2))
            .sum::<f64>() / downside_returns.len() as f64;
        
        downside_variance.sqrt() * (252.0_f64).sqrt() // Annualized
    }
    
    /// Calculate 95% Value at Risk
    fn calculate_var_95(&self) -> f64 {
        if self.daily_returns.is_empty() {
            return 0.0;
        }
        
        let mut sorted_returns = self.daily_returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let var_index = ((self.daily_returns.len() as f64) * 0.05).floor() as usize;
        if var_index < sorted_returns.len() {
            sorted_returns[var_index].abs()
        } else {
            0.0
        }
    }
    
    /// Calculate maximum consecutive losses
    fn calculate_max_consecutive_losses(&self) -> u32 {
        let mut max_consecutive = 0u32;
        let mut current_consecutive = 0u32;
        
        for trade in &self.trades {
            if trade.is_profitable() {
                current_consecutive = 0;
            } else {
                current_consecutive += 1;
                max_consecutive = max_consecutive.max(current_consecutive);
            }
        }
        
        max_consecutive
    }
    
    /// Calculate benchmark (buy and hold) return
    fn calculate_benchmark_return(&self, prices: &[f64]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }
        
        (prices.last().unwrap() - prices[0]) / prices[0]
    }
    
    /// Calculate alpha, beta, and information ratio
    fn calculate_alpha_beta_ir(&self, strategy_return: f64, benchmark_return: f64) -> (f64, f64, f64) {
        // Simplified calculations for MVP
        let beta = 1.0; // Assume beta of 1 for simplicity
        let alpha = strategy_return - benchmark_return;
        let information_ratio = if benchmark_return != 0.0 { alpha / benchmark_return.abs() } else { 0.0 };
        
        (alpha, beta, information_ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::mvp_predictor::TradingDecision;
    
    fn create_test_predictions(count: usize) -> Vec<MVPPredictionResult> {
        let mut predictions = Vec::new();
        for i in 0..count {
            let decision = match i % 3 {
                0 => TradingDecision::Buy,
                1 => TradingDecision::Sell,
                _ => TradingDecision::Hold,
            };
            
            predictions.push(MVPPredictionResult {
                predicted_return: (i as f32) * 0.01 - 0.5,
                confidence: 0.7,
                decision,
                timestamp: Utc::now() + Duration::days(i as i64),
                metadata: std::collections::HashMap::new(),
            });
        }
        predictions
    }
    
    fn create_test_prices(count: usize) -> Vec<f64> {
        (0..count).map(|i| 100.0 + (i as f64) * 0.5).collect()
    }
    
    fn create_test_timestamps(count: usize) -> Vec<DateTime<Utc>> {
        (0..count).map(|i| Utc::now() + Duration::days(i as i64)).collect()
    }
    
    #[test]
    fn test_backtest_config_defaults() {
        let config = BacktestConfig::default();
        assert_eq!(config.initial_capital, 100_000.0);
        assert_eq!(config.transaction_cost, 0.001);
        assert_eq!(config.position_size, 0.1);
    }
    
    #[test]
    fn test_trade_calculations() {
        let trade = Trade {
            entry_time: Utc::now(),
            exit_time: Utc::now() + Duration::days(1),
            direction: TradingDecision::Buy,
            entry_price: 100.0,
            exit_price: 105.0,
            shares: 10.0,
            gross_pnl: 50.0,
            net_pnl: 48.0,
            costs: 2.0,
            holding_days: 1.0,
        };
        
        assert_eq!(trade.return_pct(), 0.05); // 5% return
        assert!(trade.is_profitable());
    }
    
    #[test]
    fn test_backtester_creation() {
        let config = BacktestConfig::default();
        let backtester = MVPBacktester::new(config.clone());
        
        assert_eq!(backtester.current_capital, config.initial_capital);
        assert!(backtester.positions.is_empty());
        assert!(backtester.trades.is_empty());
    }
    
    #[test]
    fn test_basic_backtest() {
        let config = BacktestConfig::default();
        let mut backtester = MVPBacktester::new(config);
        
        let predictions = create_test_predictions(10);
        let prices = create_test_prices(10);
        let timestamps = create_test_timestamps(10);
        
        let result = backtester.run_backtest(&predictions, &prices, &timestamps).unwrap();
        
        assert!(result.total_trades > 0);
        assert!(result.final_capital > 0.0);
        assert_eq!(result.equity_curve.len(), 10);
    }
}