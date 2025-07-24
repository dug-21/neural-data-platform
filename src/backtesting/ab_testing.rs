//! A/B Testing framework for live strategy validation
//! 
//! This module provides statistical testing for comparing strategies in production:
//! - Traffic splitting and allocation
//! - Statistical significance testing
//! - Performance comparison
//! - Automated decision making

use super::*;
use crate::strategies::TradingStrategy;
use statrs::distribution::{StudentsT, ContinuousCDF};
use statrs::statistics::Statistics;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use chrono::{DateTime, Utc};

/// A/B test controller for live strategy comparison
pub struct ABTestController {
    /// Test configuration
    config: ABTestConfig,
    
    /// Control strategy performance tracker
    control_tracker: Arc<Mutex<PerformanceTracker>>,
    
    /// Variant strategy performance tracker
    variant_tracker: Arc<Mutex<PerformanceTracker>>,
    
    /// Test state
    test_state: Arc<RwLock<TestState>>,
    
    /// Random number generator for allocation
    rng: Arc<Mutex<rand::rngs::StdRng>>,
}

impl ABTestController {
    pub fn new(config: ABTestConfig) -> Self {
        use rand::SeedableRng;
        
        Self {
            config,
            control_tracker: Arc::new(Mutex::new(PerformanceTracker::new("control"))),
            variant_tracker: Arc::new(Mutex::new(PerformanceTracker::new("variant"))),
            test_state: Arc::new(RwLock::new(TestState::new())),
            rng: Arc::new(Mutex::new(rand::rngs::StdRng::from_entropy())),
        }
    }
    
    /// Allocate incoming trade opportunity to control or variant
    pub async fn allocate_trade(&self) -> ABTestAllocation {
        let mut rng = self.rng.lock().await;
        let random_value: f64 = rand::Rng::gen(&mut *rng);
        
        if random_value < self.config.allocation_ratio {
            ABTestAllocation::Control
        } else {
            ABTestAllocation::Variant
        }
    }
    
    /// Record trade result for the appropriate strategy
    pub async fn record_trade_result(
        &self,
        allocation: ABTestAllocation,
        trade: Trade,
    ) -> Result<(), BacktestError> {
        match allocation {
            ABTestAllocation::Control => {
                let mut tracker = self.control_tracker.lock().await;
                tracker.record_trade(trade);
            }
            ABTestAllocation::Variant => {
                let mut tracker = self.variant_tracker.lock().await;
                tracker.record_trade(trade);
            }
        }
        
        // Update test state
        let mut state = self.test_state.write().await;
        state.total_trades += 1;
        state.last_update = Utc::now();
        
        // Check if we should analyze results
        if self.should_analyze(&state).await {
            self.analyze_results().await?;
        }
        
        Ok(())
    }
    
    /// Check if we have enough data to analyze
    async fn should_analyze(&self, state: &TestState) -> bool {
        // Check minimum sample size
        if state.total_trades < self.config.min_sample_size {
            return false;
        }
        
        // Check if test duration has been met
        let duration = Utc::now() - state.start_time;
        if duration.num_days() < self.config.test_duration_days as i64 {
            return false;
        }
        
        true
    }
    
    /// Analyze A/B test results
    pub async fn analyze_results(&self) -> Result<ABTestResults, BacktestError> {
        let control_metrics = {
            let tracker = self.control_tracker.lock().await;
            tracker.calculate_metrics()
        };
        
        let variant_metrics = {
            let tracker = self.variant_tracker.lock().await;
            tracker.calculate_metrics()
        };
        
        // Statistical significance testing
        let significance = self.calculate_statistical_significance(
            &control_metrics,
            &variant_metrics,
        ).await?;
        
        // Determine winner
        let winner = self.determine_winner(
            &control_metrics,
            &variant_metrics,
            significance,
        );
        
        // Calculate confidence interval for the difference
        let confidence_interval = self.calculate_confidence_interval(
            &control_metrics,
            &variant_metrics,
        ).await?;
        
        // Generate recommendation
        let recommendation = self.generate_recommendation(
            &control_metrics,
            &variant_metrics,
            significance,
            &winner,
        );
        
        let results = ABTestResults {
            control_performance: control_metrics,
            variant_performance: variant_metrics,
            statistical_significance: significance,
            winner,
            confidence_interval,
            recommendation,
        };
        
        // Update test state
        let mut state = self.test_state.write().await;
        state.results = Some(results.clone());
        state.completed = true;
        
        Ok(results)
    }
    
    /// Calculate statistical significance using t-test
    async fn calculate_statistical_significance(
        &self,
        control: &PerformanceMetrics,
        variant: &PerformanceMetrics,
    ) -> Result<f64, BacktestError> {
        let control_tracker = self.control_tracker.lock().await;
        let variant_tracker = self.variant_tracker.lock().await;
        
        let control_returns = &control_tracker.daily_returns;
        let variant_returns = &variant_tracker.daily_returns;
        
        if control_returns.len() < 2 || variant_returns.len() < 2 {
            return Ok(1.0); // No significance if insufficient data
        }
        
        // Welch's t-test (for unequal variances)
        let control_mean = control_returns.mean();
        let variant_mean = variant_returns.mean();
        
        let control_var = control_returns.variance();
        let variant_var = variant_returns.variance();
        
        let control_n = control_returns.len() as f64;
        let variant_n = variant_returns.len() as f64;
        
        let t_statistic = (variant_mean - control_mean) / 
            ((control_var / control_n) + (variant_var / variant_n)).sqrt();
        
        // Calculate degrees of freedom using Welch-Satterthwaite equation
        let df = ((control_var / control_n + variant_var / variant_n).powi(2)) /
            ((control_var / control_n).powi(2) / (control_n - 1.0) +
             (variant_var / variant_n).powi(2) / (variant_n - 1.0));
        
        // Calculate p-value
        let t_dist = StudentsT::new(0.0, 1.0, df).map_err(|e| 
            BacktestError::CalculationError(format!("Invalid t-distribution: {}", e))
        )?;
        
        let p_value = 2.0 * (1.0 - t_dist.cdf(t_statistic.abs()));
        
        Ok(p_value)
    }
    
    /// Calculate confidence interval for the difference in means
    async fn calculate_confidence_interval(
        &self,
        control: &PerformanceMetrics,
        variant: &PerformanceMetrics,
    ) -> Result<(f64, f64), BacktestError> {
        let control_tracker = self.control_tracker.lock().await;
        let variant_tracker = self.variant_tracker.lock().await;
        
        let control_returns = &control_tracker.daily_returns;
        let variant_returns = &variant_tracker.daily_returns;
        
        if control_returns.is_empty() || variant_returns.is_empty() {
            return Ok((0.0, 0.0));
        }
        
        let control_mean = control_returns.mean();
        let variant_mean = variant_returns.mean();
        let mean_diff = variant_mean - control_mean;
        
        let control_se = control_returns.std_dev() / (control_returns.len() as f64).sqrt();
        let variant_se = variant_returns.std_dev() / (variant_returns.len() as f64).sqrt();
        let se_diff = (control_se.powi(2) + variant_se.powi(2)).sqrt();
        
        // Use t-distribution for confidence interval
        let df = control_returns.len() + variant_returns.len() - 2;
        let t_critical = 1.96; // Approximate for 95% confidence
        
        let margin_of_error = t_critical * se_diff;
        
        Ok((mean_diff - margin_of_error, mean_diff + margin_of_error))
    }
    
    /// Determine winner based on metrics and significance
    fn determine_winner(
        &self,
        control: &PerformanceMetrics,
        variant: &PerformanceMetrics,
        p_value: f64,
    ) -> Option<String> {
        // Check if statistically significant
        if p_value > (1.0 - self.config.confidence_level) {
            return None; // No significant difference
        }
        
        // Compare primary metric (Sharpe ratio)
        if variant.sharpe_ratio > control.sharpe_ratio {
            Some(self.config.variant_strategy.clone())
        } else {
            Some(self.config.control_strategy.clone())
        }
    }
    
    /// Generate recommendation based on results
    fn generate_recommendation(
        &self,
        control: &PerformanceMetrics,
        variant: &PerformanceMetrics,
        p_value: f64,
        winner: &Option<String>,
    ) -> String {
        let significance_threshold = 1.0 - self.config.confidence_level;
        
        if p_value > significance_threshold {
            return format!(
                "No statistically significant difference detected (p={:.3}). \
                Continue testing or maintain current strategy.",
                p_value
            );
        }
        
        if let Some(winner_name) = winner {
            let improvement = if winner_name == &self.config.variant_strategy {
                ((variant.sharpe_ratio - control.sharpe_ratio) / control.sharpe_ratio.abs()) * 100.0
            } else {
                ((control.sharpe_ratio - variant.sharpe_ratio) / variant.sharpe_ratio.abs()) * 100.0
            };
            
            format!(
                "Statistically significant result (p={:.3}). \
                {} shows {:.1}% improvement in Sharpe ratio. \
                Recommendation: Adopt {} strategy.",
                p_value, winner_name, improvement.abs(), winner_name
            )
        } else {
            "Error determining winner despite significant results.".to_string()
        }
    }
    
    /// Get current test status
    pub async fn get_status(&self) -> ABTestStatus {
        let state = self.test_state.read().await;
        let control_tracker = self.control_tracker.lock().await;
        let variant_tracker = self.variant_tracker.lock().await;
        
        ABTestStatus {
            test_name: self.config.test_name.clone(),
            start_time: state.start_time,
            total_trades: state.total_trades,
            control_trades: control_tracker.trade_count(),
            variant_trades: variant_tracker.trade_count(),
            is_complete: state.completed,
            current_leader: self.get_current_leader(&control_tracker, &variant_tracker),
            estimated_completion: self.estimate_completion(&state),
        }
    }
    
    /// Get current leader based on performance
    fn get_current_leader(
        &self,
        control: &PerformanceTracker,
        variant: &PerformanceTracker,
    ) -> Option<String> {
        let control_sharpe = control.calculate_sharpe_ratio();
        let variant_sharpe = variant.calculate_sharpe_ratio();
        
        if control_sharpe > variant_sharpe {
            Some(self.config.control_strategy.clone())
        } else if variant_sharpe > control_sharpe {
            Some(self.config.variant_strategy.clone())
        } else {
            None
        }
    }
    
    /// Estimate test completion time
    fn estimate_completion(&self, state: &TestState) -> Option<DateTime<Utc>> {
        if state.completed {
            return None;
        }
        
        let elapsed = Utc::now() - state.start_time;
        let trades_per_day = state.total_trades as f64 / elapsed.num_days().max(1) as f64;
        
        let remaining_trades = self.config.min_sample_size.saturating_sub(state.total_trades);
        let estimated_days = remaining_trades as f64 / trades_per_day;
        
        Some(Utc::now() + chrono::Duration::days(estimated_days as i64))
    }
}

/// Performance tracker for individual strategies
#[derive(Debug)]
struct PerformanceTracker {
    strategy_name: String,
    trades: Vec<Trade>,
    daily_returns: Vec<f64>,
    equity_curve: Vec<f64>,
}

impl PerformanceTracker {
    fn new(name: &str) -> Self {
        Self {
            strategy_name: name.to_string(),
            trades: Vec::new(),
            daily_returns: Vec::new(),
            equity_curve: vec![10000.0], // Starting equity
        }
    }
    
    fn record_trade(&mut self, trade: Trade) {
        self.trades.push(trade.clone());
        
        // Update equity
        let last_equity = self.equity_curve.last().copied().unwrap_or(10000.0);
        let new_equity = last_equity + trade.pnl;
        self.equity_curve.push(new_equity);
        
        // Calculate daily return
        let daily_return = trade.pnl / last_equity;
        self.daily_returns.push(daily_return);
    }
    
    fn calculate_metrics(&self) -> PerformanceMetrics {
        let total_trades = self.trades.len() as u32;
        let winning_trades = self.trades.iter().filter(|t| t.pnl > 0.0).count() as u32;
        let losing_trades = self.trades.iter().filter(|t| t.pnl < 0.0).count() as u32;
        
        let total_return = if let (Some(first), Some(last)) = (self.equity_curve.first(), self.equity_curve.last()) {
            (last - first) / first
        } else {
            0.0
        };
        
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        
        let sharpe_ratio = self.calculate_sharpe_ratio();
        
        // Calculate other metrics...
        PerformanceMetrics {
            total_return,
            annualized_return: total_return * (252.0 / self.daily_returns.len() as f64),
            monthly_returns: vec![], // Simplified
            volatility: self.calculate_volatility(),
            downside_deviation: 0.0, // Simplified
            max_drawdown: self.calculate_max_drawdown(),
            max_drawdown_duration_days: 0,
            sharpe_ratio,
            sortino_ratio: 0.0, // Simplified
            calmar_ratio: 0.0, // Simplified
            information_ratio: 0.0,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            profit_factor: self.calculate_profit_factor(),
            expectancy: self.calculate_expectancy(),
            avg_win: self.calculate_avg_win(),
            avg_loss: self.calculate_avg_loss(),
            largest_win: self.trades.iter().map(|t| t.pnl).fold(0.0_f64, |a, b| a.max(b)),
            largest_loss: self.trades.iter().map(|t| t.pnl).fold(0.0_f64, |a, b| a.min(b)),
            avg_trade_duration_hours: 24.0, // Simplified
            value_at_risk_95: 0.0,
            conditional_value_at_risk_95: 0.0,
            beta: 1.0,
            alpha: 0.0,
            correlation_to_benchmark: 0.0,
            total_commission_paid: self.trades.iter().map(|t| t.commission).sum(),
            total_slippage_cost: self.trades.iter().map(|t| t.slippage).sum(),
            net_profit: self.trades.iter().map(|t| t.pnl).sum(),
        }
    }
    
    fn calculate_sharpe_ratio(&self) -> f64 {
        if self.daily_returns.len() < 2 {
            return 0.0;
        }
        
        let mean_return = self.daily_returns.mean();
        let std_dev = self.daily_returns.std_dev();
        
        if std_dev > 0.0 {
            let annual_return = mean_return * 252.0;
            let annual_vol = std_dev * (252.0_f64).sqrt();
            let risk_free_rate = 0.02; // 2% annual
            (annual_return - risk_free_rate) / annual_vol
        } else {
            0.0
        }
    }
    
    fn calculate_volatility(&self) -> f64 {
        if self.daily_returns.len() < 2 {
            return 0.0;
        }
        self.daily_returns.std_dev() * (252.0_f64).sqrt()
    }
    
    fn calculate_max_drawdown(&self) -> f64 {
        let mut peak = self.equity_curve.first().copied().unwrap_or(10000.0);
        let mut max_dd = 0.0;
        
        for &equity in &self.equity_curve {
            if equity > peak {
                peak = equity;
            }
            let dd = (peak - equity) / peak;
            max_dd = max_dd.max(dd);
        }
        
        max_dd * 100.0
    }
    
    fn calculate_profit_factor(&self) -> f64 {
        let gross_profit: f64 = self.trades.iter()
            .filter(|t| t.pnl > 0.0)
            .map(|t| t.pnl)
            .sum();
        
        let gross_loss: f64 = self.trades.iter()
            .filter(|t| t.pnl < 0.0)
            .map(|t| t.pnl.abs())
            .sum();
        
        if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        }
    }
    
    fn calculate_expectancy(&self) -> f64 {
        if self.trades.is_empty() {
            return 0.0;
        }
        self.trades.iter().map(|t| t.pnl).sum::<f64>() / self.trades.len() as f64
    }
    
    fn calculate_avg_win(&self) -> f64 {
        let wins: Vec<f64> = self.trades.iter()
            .filter(|t| t.pnl > 0.0)
            .map(|t| t.pnl)
            .collect();
        
        if wins.is_empty() {
            0.0
        } else {
            wins.iter().sum::<f64>() / wins.len() as f64
        }
    }
    
    fn calculate_avg_loss(&self) -> f64 {
        let losses: Vec<f64> = self.trades.iter()
            .filter(|t| t.pnl < 0.0)
            .map(|t| t.pnl)
            .collect();
        
        if losses.is_empty() {
            0.0
        } else {
            losses.iter().sum::<f64>() / losses.len() as f64
        }
    }
    
    fn trade_count(&self) -> u32 {
        self.trades.len() as u32
    }
}

/// Test state tracking
#[derive(Debug)]
struct TestState {
    start_time: DateTime<Utc>,
    last_update: DateTime<Utc>,
    total_trades: u32,
    completed: bool,
    results: Option<ABTestResults>,
}

impl TestState {
    fn new() -> Self {
        let now = Utc::now();
        Self {
            start_time: now,
            last_update: now,
            total_trades: 0,
            completed: false,
            results: None,
        }
    }
}

/// Allocation decision for A/B test
#[derive(Debug, Clone, Copy)]
pub enum ABTestAllocation {
    Control,
    Variant,
}

/// A/B test status
#[derive(Debug, Clone)]
pub struct ABTestStatus {
    pub test_name: String,
    pub start_time: DateTime<Utc>,
    pub total_trades: u32,
    pub control_trades: u32,
    pub variant_trades: u32,
    pub is_complete: bool,
    pub current_leader: Option<String>,
    pub estimated_completion: Option<DateTime<Utc>>,
}

/// Sequential testing for early stopping
pub struct SequentialTesting {
    alpha: f64,
    beta: f64,
    effect_size: f64,
}

impl SequentialTesting {
    pub fn new(alpha: f64, beta: f64, effect_size: f64) -> Self {
        Self { alpha, beta, effect_size }
    }
    
    /// Check if we can stop the test early
    pub fn should_stop(
        &self,
        control_metrics: &PerformanceMetrics,
        variant_metrics: &PerformanceMetrics,
        n_samples: usize,
    ) -> Option<ABTestDecision> {
        // Simplified sequential probability ratio test (SPRT)
        let control_mean = control_metrics.sharpe_ratio;
        let variant_mean = variant_metrics.sharpe_ratio;
        
        let log_likelihood_ratio = n_samples as f64 * 
            (variant_mean - control_mean) * self.effect_size;
        
        let upper_bound = ((1.0 - self.beta) / self.alpha).ln();
        let lower_bound = (self.beta / (1.0 - self.alpha)).ln();
        
        if log_likelihood_ratio > upper_bound {
            Some(ABTestDecision::AcceptVariant)
        } else if log_likelihood_ratio < lower_bound {
            Some(ABTestDecision::AcceptControl)
        } else {
            None // Continue testing
        }
    }
}

#[derive(Debug, Clone)]
pub enum ABTestDecision {
    AcceptControl,
    AcceptVariant,
    ContinueTesting,
}