//! Comprehensive Backtesting Framework for Strategy Validation
//! 
//! This module provides advanced backtesting capabilities including:
//! - Walk-forward analysis
//! - Monte Carlo simulations
//! - Market regime testing
//! - Stress testing scenarios
//! - Transaction cost modeling
//! - Slippage estimation

pub mod engine;
pub mod metrics;
pub mod monte_carlo;
pub mod walk_forward;
pub mod market_regimes;
pub mod stress_tests;
pub mod transaction_costs;
pub mod validation;
pub mod ab_testing;

use crate::strategies::{TradingStrategy, MarketContext, Signal, Position};
use crate::data::TimeSeriesData;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BacktestError {
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Insufficient data: {0}")]
    InsufficientData(String),
    
    #[error("Strategy error: {0}")]
    StrategyError(#[from] crate::strategies::StrategyError),
    
    #[error("Calculation error: {0}")]
    CalculationError(String),
    
    #[error("Data quality issue: {0}")]
    DataQualityIssue(String),
}

/// Backtesting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// Initial capital in base currency
    pub initial_capital: f64,
    
    /// Commission per trade (percentage)
    pub commission_rate: f64,
    
    /// Slippage model configuration
    pub slippage_config: SlippageConfig,
    
    /// Position sizing method
    pub position_sizing: PositionSizing,
    
    /// Risk management rules
    pub risk_config: RiskConfig,
    
    /// Enable transaction cost modeling
    pub enable_transaction_costs: bool,
    
    /// Enable realistic order execution
    pub enable_realistic_execution: bool,
    
    /// Market regime detection
    pub enable_regime_detection: bool,
    
    /// Benchmark symbol for relative performance
    pub benchmark_symbol: Option<String>,
    
    /// Random seed for Monte Carlo simulations
    pub random_seed: Option<u64>,
}

/// Slippage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageConfig {
    /// Fixed slippage in basis points
    pub fixed_slippage_bps: f64,
    
    /// Variable slippage based on order size
    pub size_impact_factor: f64,
    
    /// Market impact model
    pub market_impact_model: MarketImpactModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketImpactModel {
    Linear,
    SquareRoot,
    Logarithmic,
}

/// Position sizing methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionSizing {
    Fixed(f64),
    PercentOfEquity(f64),
    KellyCriterion,
    VolatilityBased { target_volatility: f64 },
    OptimalF,
}

/// Risk management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum position size as % of portfolio
    pub max_position_size: f64,
    
    /// Maximum portfolio leverage
    pub max_leverage: f64,
    
    /// Maximum daily loss limit
    pub daily_loss_limit: f64,
    
    /// Maximum drawdown before stopping
    pub max_drawdown_limit: f64,
    
    /// Correlation limits for portfolio
    pub max_correlation: f64,
    
    /// Value at Risk (VaR) limit
    pub var_limit: f64,
}

/// Comprehensive backtesting results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResults {
    /// Performance metrics
    pub metrics: PerformanceMetrics,
    
    /// Trade history
    pub trades: Vec<Trade>,
    
    /// Equity curve
    pub equity_curve: Vec<EquityPoint>,
    
    /// Drawdown series
    pub drawdown_series: Vec<DrawdownPoint>,
    
    /// Risk metrics over time
    pub risk_metrics: RiskMetrics,
    
    /// Market regime analysis
    pub regime_analysis: Option<RegimeAnalysis>,
    
    /// Monte Carlo results
    pub monte_carlo_results: Option<MonteCarloResults>,
    
    /// Walk-forward analysis results
    pub walk_forward_results: Option<WalkForwardResults>,
    
    /// Transaction cost analysis
    pub transaction_cost_analysis: TransactionCostAnalysis,
}

/// Core performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // Returns
    pub total_return: f64,
    pub annualized_return: f64,
    pub monthly_returns: Vec<f64>,
    
    // Risk metrics
    pub volatility: f64,
    pub downside_deviation: f64,
    pub max_drawdown: f64,
    pub max_drawdown_duration_days: i64,
    
    // Risk-adjusted returns
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub information_ratio: f64,
    
    // Trade statistics
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub expectancy: f64,
    
    // Trade metrics
    pub avg_win: f64,
    pub avg_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub avg_trade_duration_hours: f64,
    
    // Risk metrics
    pub value_at_risk_95: f64,
    pub conditional_value_at_risk_95: f64,
    pub beta: f64,
    pub alpha: f64,
    pub correlation_to_benchmark: f64,
    
    // Efficiency metrics
    pub total_commission_paid: f64,
    pub total_slippage_cost: f64,
    pub net_profit: f64,
}

/// Individual trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub symbol: String,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub entry_price: f64,
    pub exit_price: f64,
    pub size: f64,
    pub side: TradeSide,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub commission: f64,
    pub slippage: f64,
    pub mae: f64, // Maximum Adverse Excursion
    pub mfe: f64, // Maximum Favorable Excursion
    pub entry_signal: Signal,
    pub exit_signal: Signal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Long,
    Short,
}

/// Point in equity curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub equity: f64,
    pub cash: f64,
    pub positions_value: f64,
    pub daily_return: f64,
    pub cumulative_return: f64,
}

/// Drawdown information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownPoint {
    pub timestamp: DateTime<Utc>,
    pub drawdown_percent: f64,
    pub drawdown_value: f64,
    pub underwater_days: i64,
}

/// Risk metrics over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub daily_var: Vec<f64>,
    pub rolling_volatility: Vec<f64>,
    pub rolling_sharpe: Vec<f64>,
    pub rolling_beta: Vec<f64>,
    pub leverage_series: Vec<f64>,
}

/// Market regime analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeAnalysis {
    pub regimes: Vec<MarketRegime>,
    pub performance_by_regime: HashMap<String, PerformanceMetrics>,
    pub regime_transitions: Vec<RegimeTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRegime {
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub characteristics: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeTransition {
    pub from_regime: String,
    pub to_regime: String,
    pub timestamp: DateTime<Utc>,
}

/// Monte Carlo simulation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResults {
    pub num_simulations: u32,
    pub confidence_intervals: ConfidenceIntervals,
    pub probability_of_ruin: f64,
    pub expected_max_drawdown: f64,
    pub var_distribution: Vec<f64>,
    pub return_distribution: Vec<f64>,
    pub percentile_equity_curves: HashMap<String, Vec<EquityPoint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceIntervals {
    pub return_95: (f64, f64),
    pub return_99: (f64, f64),
    pub sharpe_95: (f64, f64),
    pub max_drawdown_95: (f64, f64),
}

/// Walk-forward analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResults {
    pub in_sample_periods: Vec<WalkForwardPeriod>,
    pub out_of_sample_periods: Vec<WalkForwardPeriod>,
    pub optimization_efficiency: f64,
    pub parameter_stability: HashMap<String, f64>,
    pub robustness_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardPeriod {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub optimal_parameters: HashMap<String, serde_json::Value>,
    pub performance: PerformanceMetrics,
}

/// Transaction cost analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCostAnalysis {
    pub total_commission: f64,
    pub total_slippage: f64,
    pub total_market_impact: f64,
    pub avg_cost_per_trade: f64,
    pub cost_as_percent_of_volume: f64,
    pub breakeven_win_rate: f64,
}

/// Main backtesting engine trait
#[async_trait]
pub trait BacktestEngine: Send + Sync {
    /// Run a complete backtest
    async fn run_backtest(
        &self,
        strategy: Box<dyn TradingStrategy>,
        data: Vec<TimeSeriesData>,
        config: BacktestConfig,
    ) -> Result<BacktestResults, BacktestError>;
    
    /// Run walk-forward analysis
    async fn run_walk_forward_analysis(
        &self,
        strategy: Box<dyn TradingStrategy>,
        data: Vec<TimeSeriesData>,
        config: BacktestConfig,
        walk_forward_config: WalkForwardConfig,
    ) -> Result<WalkForwardResults, BacktestError>;
    
    /// Run Monte Carlo simulation
    async fn run_monte_carlo(
        &self,
        base_results: &BacktestResults,
        num_simulations: u32,
        config: MonteCarloConfig,
    ) -> Result<MonteCarloResults, BacktestError>;
    
    /// Run stress tests
    async fn run_stress_tests(
        &self,
        strategy: Box<dyn TradingStrategy>,
        data: Vec<TimeSeriesData>,
        config: BacktestConfig,
        stress_scenarios: Vec<StressScenario>,
    ) -> Result<HashMap<String, BacktestResults>, BacktestError>;
}

/// Walk-forward configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    pub in_sample_ratio: f64,
    pub step_size_ratio: f64,
    pub optimization_metric: OptimizationMetric,
    pub parameter_ranges: HashMap<String, ParameterRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationMetric {
    SharpeRatio,
    TotalReturn,
    ProfitFactor,
    Expectancy,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// Monte Carlo configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloConfig {
    pub randomize_returns: bool,
    pub randomize_trade_order: bool,
    pub add_noise: bool,
    pub noise_level: f64,
    pub bootstrap_method: BootstrapMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BootstrapMethod {
    Simple,
    BlockBootstrap { block_size: usize },
    StationaryBootstrap { avg_block_size: f64 },
}

/// Stress test scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    pub name: String,
    pub description: String,
    pub market_conditions: MarketConditions,
    pub duration_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    pub volatility_multiplier: f64,
    pub trend_bias: f64,
    pub liquidity_factor: f64,
    pub correlation_shift: f64,
    pub spread_widening_factor: f64,
}

/// A/B testing framework for live strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    pub test_name: String,
    pub control_strategy: String,
    pub variant_strategy: String,
    pub allocation_ratio: f64,
    pub min_sample_size: u32,
    pub confidence_level: f64,
    pub test_duration_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestResults {
    pub control_performance: PerformanceMetrics,
    pub variant_performance: PerformanceMetrics,
    pub statistical_significance: f64,
    pub winner: Option<String>,
    pub confidence_interval: (f64, f64),
    pub recommendation: String,
}