//! Core backtesting engine implementation
//! 
//! This module implements the main backtesting logic with support for:
//! - Realistic order execution with slippage and market impact
//! - Transaction cost modeling
//! - Position management and portfolio tracking
//! - Performance metric calculation

use super::*;
use crate::strategies::{TradingStrategy, MarketContext, Signal, Position, PositionSide};
use crate::data::TimeSeriesData;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Main backtesting engine implementation
pub struct StandardBacktestEngine {
    /// Performance tracker
    performance_tracker: Arc<Mutex<PerformanceTracker>>,
    
    /// Market data cache for efficient lookups
    market_data_cache: Arc<Mutex<MarketDataCache>>,
    
    /// Transaction cost calculator
    transaction_cost_calculator: TransactionCostCalculator,
}

impl StandardBacktestEngine {
    pub fn new() -> Self {
        Self {
            performance_tracker: Arc::new(Mutex::new(PerformanceTracker::new())),
            market_data_cache: Arc::new(Mutex::new(MarketDataCache::new())),
            transaction_cost_calculator: TransactionCostCalculator::new(),
        }
    }
    
    /// Execute a single trade with realistic modeling
    async fn execute_trade(
        &self,
        signal: &Signal,
        market_context: &MarketContext,
        portfolio: &mut Portfolio,
        config: &BacktestConfig,
    ) -> Result<Option<Trade>, BacktestError> {
        match signal {
            Signal::Buy { confidence, size, reason } => {
                let trade_size = self.calculate_position_size(
                    portfolio,
                    market_context,
                    confidence,
                    size.as_ref(),
                    &config.position_sizing,
                )?;
                
                if trade_size > 0.0 {
                    let (execution_price, slippage) = self.calculate_execution_price(
                        market_context.ask,
                        trade_size,
                        true,
                        &config.slippage_config,
                    );
                    
                    let commission = self.transaction_cost_calculator.calculate_commission(
                        trade_size * execution_price,
                        config.commission_rate,
                    );
                    
                    let trade = Trade {
                        id: Uuid::new_v4().to_string(),
                        symbol: market_context.symbol.clone(),
                        entry_time: DateTime::from_timestamp(market_context.timestamp, 0)
                            .unwrap_or_else(Utc::now),
                        exit_time: DateTime::from_timestamp(market_context.timestamp, 0)
                            .unwrap_or_else(Utc::now), // Will be updated on exit
                        entry_price: execution_price,
                        exit_price: 0.0, // Will be updated on exit
                        size: trade_size,
                        side: TradeSide::Long,
                        pnl: 0.0, // Will be calculated on exit
                        pnl_percent: 0.0,
                        commission,
                        slippage,
                        mae: 0.0,
                        mfe: 0.0,
                        entry_signal: signal.clone(),
                        exit_signal: Signal::Hold { reason: "Position open".to_string() },
                    };
                    
                    portfolio.open_position(trade.clone(), execution_price)?;
                    return Ok(Some(trade));
                }
            }
            Signal::Sell { confidence, size, reason } => {
                if let Some(position_id) = portfolio.get_open_position_id(&market_context.symbol) {
                    let position = portfolio.get_position(&position_id)?;
                    let close_size = size.unwrap_or(position.size);
                    
                    let (execution_price, slippage) = self.calculate_execution_price(
                        market_context.bid,
                        close_size,
                        false,
                        &config.slippage_config,
                    );
                    
                    let commission = self.transaction_cost_calculator.calculate_commission(
                        close_size * execution_price,
                        config.commission_rate,
                    );
                    
                    let mut trade = portfolio.close_position(
                        &position_id,
                        execution_price,
                        DateTime::from_timestamp(market_context.timestamp, 0)
                            .unwrap_or_else(Utc::now),
                    )?;
                    
                    trade.exit_signal = signal.clone();
                    trade.commission += commission;
                    trade.slippage += slippage;
                    
                    return Ok(Some(trade));
                }
            }
            Signal::Hold { .. } => {}
        }
        
        Ok(None)
    }
    
    /// Calculate position size based on strategy and risk management
    fn calculate_position_size(
        &self,
        portfolio: &Portfolio,
        market_context: &MarketContext,
        confidence: &f64,
        suggested_size: Option<&f64>,
        sizing_method: &PositionSizing,
    ) -> Result<f64, BacktestError> {
        let base_size = match sizing_method {
            PositionSizing::Fixed(size) => *size,
            PositionSizing::PercentOfEquity(percent) => {
                portfolio.total_equity * percent / market_context.current_price
            }
            PositionSizing::KellyCriterion => {
                // Simplified Kelly criterion implementation
                let win_rate = portfolio.calculate_win_rate();
                let avg_win = portfolio.calculate_avg_win();
                let avg_loss = portfolio.calculate_avg_loss().abs();
                
                if avg_loss > 0.0 {
                    let kelly_percent = (win_rate * avg_win - (1.0 - win_rate) * avg_loss) / avg_win;
                    (portfolio.total_equity * kelly_percent.max(0.0).min(0.25)) / market_context.current_price
                } else {
                    portfolio.total_equity * 0.02 / market_context.current_price
                }
            }
            PositionSizing::VolatilityBased { target_volatility } => {
                let position_vol = market_context.volatility;
                let target_risk = portfolio.total_equity * target_volatility;
                target_risk / (position_vol * market_context.current_price)
            }
            PositionSizing::OptimalF => {
                // Simplified Optimal F implementation
                let f = portfolio.calculate_optimal_f().min(0.25);
                portfolio.total_equity * f / market_context.current_price
            }
        };
        
        // Apply confidence scaling
        let scaled_size = base_size * confidence;
        
        // Override with suggested size if provided
        let final_size = suggested_size.unwrap_or(&scaled_size);
        
        Ok(*final_size)
    }
    
    /// Calculate execution price with slippage
    fn calculate_execution_price(
        &self,
        base_price: f64,
        size: f64,
        is_buy: bool,
        slippage_config: &SlippageConfig,
    ) -> (f64, f64) {
        let fixed_slippage = base_price * slippage_config.fixed_slippage_bps / 10000.0;
        
        let size_impact = match slippage_config.market_impact_model {
            MarketImpactModel::Linear => size * slippage_config.size_impact_factor,
            MarketImpactModel::SquareRoot => size.sqrt() * slippage_config.size_impact_factor,
            MarketImpactModel::Logarithmic => size.ln().max(0.0) * slippage_config.size_impact_factor,
        };
        
        let total_slippage = fixed_slippage + size_impact;
        
        let execution_price = if is_buy {
            base_price + total_slippage
        } else {
            base_price - total_slippage
        };
        
        (execution_price, total_slippage)
    }
}

#[async_trait]
impl BacktestEngine for StandardBacktestEngine {
    async fn run_backtest(
        &self,
        mut strategy: Box<dyn TradingStrategy>,
        data: Vec<TimeSeriesData>,
        config: BacktestConfig,
    ) -> Result<BacktestResults, BacktestError> {
        if data.is_empty() {
            return Err(BacktestError::InsufficientData("No data provided".to_string()));
        }
        
        // Initialize portfolio
        let mut portfolio = Portfolio::new(config.initial_capital);
        let mut trades: Vec<Trade> = Vec::new();
        let mut equity_curve: Vec<EquityPoint> = Vec::new();
        let mut drawdown_series: Vec<DrawdownPoint> = Vec::new();
        
        // Initialize strategy
        let strategy_config = crate::strategies::StrategyConfig {
            name: strategy.name().to_string(),
            enabled: true,
            risk_limit: config.risk_config.max_position_size,
            position_size: 1.0,
            parameters: HashMap::new(),
        };
        strategy.initialize(strategy_config).await?;
        
        // Track peak equity for drawdown calculation
        let mut peak_equity = config.initial_capital;
        let mut underwater_days = 0;
        
        // Process each data point
        for (i, data_point) in data.iter().enumerate() {
            let market_context = MarketContext {
                symbol: data_point.symbol.clone(),
                current_price: data_point.close,
                bid: data_point.close - (data_point.close * 0.0001), // Simulated spread
                ask: data_point.close + (data_point.close * 0.0001),
                volume_24h: data_point.volume_value,
                volatility: self.calculate_volatility(&data[..=i], 20),
                timestamp: data_point.timestamp.timestamp(),
            };
            
            // Update open positions
            portfolio.update_positions(&market_context);
            
            // Check if strategy can execute
            if strategy.can_execute(&market_context)? {
                // Get current position if any
                let position = portfolio.get_open_position_id(&market_context.symbol)
                    .and_then(|id| portfolio.get_position(&id).ok())
                    .map(|p| Position {
                        id: p.id.clone(),
                        symbol: p.symbol.clone(),
                        side: match p.side {
                            TradeSide::Long => PositionSide::Long,
                            TradeSide::Short => PositionSide::Short,
                        },
                        size: p.size,
                        entry_price: p.entry_price,
                        current_price: market_context.current_price,
                        pnl: p.unrealized_pnl,
                        timestamp: market_context.timestamp,
                    });
                
                // Generate signal
                let signal = strategy.generate_signal(&market_context, position.as_ref()).await?;
                
                // Execute trade if signaled
                if let Some(trade) = self.execute_trade(&signal, &market_context, &mut portfolio, &config).await? {
                    trades.push(trade);
                }
            }
            
            // Record equity point
            let current_equity = portfolio.total_equity;
            let daily_return = if i > 0 {
                (current_equity - equity_curve[i - 1].equity) / equity_curve[i - 1].equity
            } else {
                0.0
            };
            
            equity_curve.push(EquityPoint {
                timestamp: data_point.timestamp,
                equity: current_equity,
                cash: portfolio.cash,
                positions_value: portfolio.positions_value,
                daily_return,
                cumulative_return: (current_equity - config.initial_capital) / config.initial_capital,
            });
            
            // Update drawdown
            if current_equity > peak_equity {
                peak_equity = current_equity;
                underwater_days = 0;
            } else {
                underwater_days += 1;
            }
            
            let drawdown = (peak_equity - current_equity) / peak_equity;
            drawdown_series.push(DrawdownPoint {
                timestamp: data_point.timestamp,
                drawdown_percent: drawdown * 100.0,
                drawdown_value: peak_equity - current_equity,
                underwater_days,
            });
        }
        
        // Close any remaining positions
        let last_data = data.last().unwrap();
        let final_context = MarketContext {
            symbol: last_data.symbol.clone(),
            current_price: last_data.close,
            bid: last_data.close - (last_data.close * 0.0001),
            ask: last_data.close + (last_data.close * 0.0001),
            volume_24h: last_data.volume_value,
            volatility: self.calculate_volatility(&data, 20),
            timestamp: last_data.timestamp.timestamp(),
        };
        
        for position_id in portfolio.get_all_position_ids() {
            if let Ok(mut trade) = portfolio.close_position(
                &position_id,
                final_context.current_price,
                last_data.timestamp,
            ) {
                trade.exit_signal = Signal::Sell {
                    confidence: 1.0,
                    size: Some(trade.size),
                    reason: "End of backtest".to_string(),
                };
                trades.push(trade);
            }
        }
        
        // Calculate final metrics
        let metrics = self.calculate_performance_metrics(
            &trades,
            &equity_curve,
            &drawdown_series,
            config.initial_capital,
            &data,
        )?;
        
        // Calculate risk metrics
        let risk_metrics = self.calculate_risk_metrics(&equity_curve, &data)?;
        
        // Transaction cost analysis
        let transaction_cost_analysis = self.analyze_transaction_costs(&trades, &portfolio);
        
        Ok(BacktestResults {
            metrics,
            trades,
            equity_curve,
            drawdown_series,
            risk_metrics,
            regime_analysis: None, // Will be implemented in market_regimes.rs
            monte_carlo_results: None, // Will be implemented in monte_carlo.rs
            walk_forward_results: None, // Will be implemented in walk_forward.rs
            transaction_cost_analysis,
        })
    }
    
    async fn run_walk_forward_analysis(
        &self,
        strategy: Box<dyn TradingStrategy>,
        data: Vec<TimeSeriesData>,
        config: BacktestConfig,
        walk_forward_config: WalkForwardConfig,
    ) -> Result<WalkForwardResults, BacktestError> {
        use crate::backtesting::walk_forward::WalkForwardEngine;
        
        // Create a strategy factory from the provided strategy
        struct SingleStrategyFactory {
            strategy_name: String,
        }
        
        impl StrategyFactory for SingleStrategyFactory {
            fn create_strategy(&self) -> Box<dyn TradingStrategy> {
                // Note: This would need proper cloning in production
                // For now, return a placeholder that implements the interface
                unimplemented!("Strategy factory cloning needs implementation for walk-forward")
            }
        }
        
        let wf_engine = WalkForwardEngine::new();
        let strategy_factory = Box::new(SingleStrategyFactory { 
            strategy_name: "default".to_string() 
        });
        
        wf_engine.run_analysis(strategy_factory, data, config, walk_forward_config).await
    }
    
    async fn run_monte_carlo(
        &self,
        base_results: &BacktestResults,
        num_simulations: u32,
        config: MonteCarloConfig,
    ) -> Result<MonteCarloResults, BacktestError> {
        use crate::backtesting::monte_carlo::MonteCarloEngine;
        
        let mut mc_engine = MonteCarloEngine::new(config.random_seed);
        mc_engine.run_simulation(base_results, num_simulations, config).await
    }
    
    async fn run_stress_tests(
        &self,
        strategy: Box<dyn TradingStrategy>,
        data: Vec<TimeSeriesData>,
        config: BacktestConfig,
        stress_scenarios: Vec<StressScenario>,
    ) -> Result<HashMap<String, BacktestResults>, BacktestError> {
        // Implement stress testing by running backtests under various scenarios
        let mut stress_results = HashMap::new();
        
        for scenario in stress_scenarios {
            // Apply stress scenario to data
            let stressed_data = self.apply_stress_scenario(&data, &scenario)?;
            
            // Clone strategy for each stress test (would need proper implementation)
            // For now, we'll use a simplified approach
            let stressed_config = self.apply_stress_to_config(&config, &scenario);
            
            // Run backtest with stressed data and config
            let result = self.run_backtest(
                Box::new(PlaceholderStrategy::new()), // Placeholder for strategy cloning
                stressed_data,
                stressed_config,
            ).await?;
            
            stress_results.insert(scenario.name.clone(), result);
        }
        
        Ok(stress_results)
    }
}

impl StandardBacktestEngine {
    /// Calculate comprehensive performance metrics
    fn calculate_performance_metrics(
        &self,
        trades: &[Trade],
        equity_curve: &[EquityPoint],
        drawdown_series: &[DrawdownPoint],
        initial_capital: f64,
        market_data: &[TimeSeriesData],
    ) -> Result<PerformanceMetrics, BacktestError> {
        let total_trades = trades.len() as u32;
        let winning_trades = trades.iter().filter(|t| t.pnl > 0.0).count() as u32;
        let losing_trades = trades.iter().filter(|t| t.pnl < 0.0).count() as u32;
        
        let total_return = if let Some(last) = equity_curve.last() {
            (last.equity - initial_capital) / initial_capital
        } else {
            0.0
        };
        
        let trading_days = equity_curve.len() as f64;
        let years = trading_days / 252.0;
        let annualized_return = (1.0 + total_return).powf(1.0 / years) - 1.0;
        
        // Calculate monthly returns
        let monthly_returns = self.calculate_monthly_returns(equity_curve);
        
        // Calculate volatility
        let daily_returns: Vec<f64> = equity_curve.iter().map(|p| p.daily_return).collect();
        let volatility = self.calculate_std_dev(&daily_returns) * (252.0_f64).sqrt();
        
        // Calculate downside deviation
        let negative_returns: Vec<f64> = daily_returns.iter()
            .filter(|&&r| r < 0.0)
            .copied()
            .collect();
        let downside_deviation = self.calculate_std_dev(&negative_returns) * (252.0_f64).sqrt();
        
        // Max drawdown
        let max_drawdown = drawdown_series.iter()
            .map(|d| d.drawdown_percent)
            .fold(0.0_f64, |max, val| max.max(val));
        
        let max_drawdown_duration_days = drawdown_series.iter()
            .map(|d| d.underwater_days)
            .max()
            .unwrap_or(0);
        
        // Risk-adjusted returns
        let risk_free_rate = 0.02; // 2% annual risk-free rate
        let excess_return = annualized_return - risk_free_rate;
        let sharpe_ratio = if volatility > 0.0 {
            excess_return / volatility
        } else {
            0.0
        };
        
        let sortino_ratio = if downside_deviation > 0.0 {
            excess_return / downside_deviation
        } else {
            0.0
        };
        
        let calmar_ratio = if max_drawdown > 0.0 {
            annualized_return / (max_drawdown / 100.0)
        } else {
            0.0
        };
        
        // Trade statistics
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        
        let total_win = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum::<f64>();
        let total_loss = trades.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum::<f64>();
        
        let profit_factor = if total_loss > 0.0 {
            total_win / total_loss
        } else if total_win > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        
        let avg_win = if winning_trades > 0 {
            total_win / winning_trades as f64
        } else {
            0.0
        };
        
        let avg_loss = if losing_trades > 0 {
            -total_loss / losing_trades as f64
        } else {
            0.0
        };
        
        let expectancy = if total_trades > 0 {
            trades.iter().map(|t| t.pnl).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };
        
        let largest_win = trades.iter().map(|t| t.pnl).fold(0.0_f64, |max, val| max.max(val));
        let largest_loss = trades.iter().map(|t| t.pnl).fold(0.0_f64, |min, val| min.min(val));
        
        let avg_trade_duration_hours = if !trades.is_empty() {
            trades.iter()
                .map(|t| (t.exit_time - t.entry_time).num_hours() as f64)
                .sum::<f64>() / trades.len() as f64
        } else {
            0.0
        };
        
        // VaR and CVaR
        let sorted_returns = self.sort_returns(&daily_returns);
        let var_index = ((sorted_returns.len() as f64) * 0.05) as usize;
        let value_at_risk_95 = sorted_returns.get(var_index).copied().unwrap_or(0.0) * (252.0_f64).sqrt();
        
        let conditional_value_at_risk_95 = if var_index > 0 {
            sorted_returns[..var_index].iter().sum::<f64>() / var_index as f64 * (252.0_f64).sqrt()
        } else {
            0.0
        };
        
        // Calculate beta and alpha (simplified - would need benchmark data)
        let beta = 1.0; // Placeholder
        let alpha = annualized_return - (beta * 0.08); // Assuming 8% market return
        let correlation_to_benchmark = 0.0; // Placeholder
        
        // Transaction costs
        let total_commission_paid = trades.iter().map(|t| t.commission).sum();
        let total_slippage_cost = trades.iter().map(|t| t.slippage * t.size).sum();
        let net_profit = trades.iter().map(|t| t.pnl).sum::<f64>() - total_commission_paid - total_slippage_cost;
        
        Ok(PerformanceMetrics {
            total_return,
            annualized_return,
            monthly_returns,
            volatility,
            downside_deviation,
            max_drawdown,
            max_drawdown_duration_days,
            sharpe_ratio,
            sortino_ratio,
            calmar_ratio,
            information_ratio: 0.0, // Would need benchmark
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            profit_factor,
            expectancy,
            avg_win,
            avg_loss,
            largest_win,
            largest_loss,
            avg_trade_duration_hours,
            value_at_risk_95,
            conditional_value_at_risk_95,
            beta,
            alpha,
            correlation_to_benchmark,
            total_commission_paid,
            total_slippage_cost,
            net_profit,
        })
    }
    
    /// Calculate risk metrics over time
    fn calculate_risk_metrics(
        &self,
        equity_curve: &[EquityPoint],
        market_data: &[TimeSeriesData],
    ) -> Result<RiskMetrics, BacktestError> {
        // Placeholder implementation
        Ok(RiskMetrics {
            daily_var: vec![],
            rolling_volatility: vec![],
            rolling_sharpe: vec![],
            rolling_beta: vec![],
            leverage_series: vec![],
        })
    }
    
    /// Analyze transaction costs
    fn analyze_transaction_costs(
        &self,
        trades: &[Trade],
        portfolio: &Portfolio,
    ) -> TransactionCostAnalysis {
        let total_commission: f64 = trades.iter().map(|t| t.commission).sum();
        let total_slippage: f64 = trades.iter().map(|t| t.slippage * t.size).sum();
        let total_market_impact = total_slippage; // Simplified
        
        let total_volume: f64 = trades.iter().map(|t| t.size * t.entry_price).sum();
        let avg_cost_per_trade = if !trades.is_empty() {
            (total_commission + total_slippage) / trades.len() as f64
        } else {
            0.0
        };
        
        let cost_as_percent_of_volume = if total_volume > 0.0 {
            (total_commission + total_slippage) / total_volume * 100.0
        } else {
            0.0
        };
        
        // Breakeven win rate calculation
        let avg_win = portfolio.calculate_avg_win();
        let avg_loss = portfolio.calculate_avg_loss().abs();
        let breakeven_win_rate = if avg_win + avg_loss > 0.0 {
            (avg_loss + avg_cost_per_trade) / (avg_win + avg_loss + 2.0 * avg_cost_per_trade)
        } else {
            0.5
        };
        
        TransactionCostAnalysis {
            total_commission,
            total_slippage,
            total_market_impact,
            avg_cost_per_trade,
            cost_as_percent_of_volume,
            breakeven_win_rate,
        }
    }
    
    /// Calculate volatility over a window
    fn calculate_volatility(&self, data: &[TimeSeriesData], window: usize) -> f64 {
        if data.len() < window {
            return 0.2; // Default volatility
        }
        
        let start = data.len().saturating_sub(window);
        let returns: Vec<f64> = data[start..].windows(2)
            .map(|w| (w[1].close / w[0].close).ln())
            .collect();
        
        self.calculate_std_dev(&returns) * (252.0_f64).sqrt()
    }
    
    /// Calculate standard deviation
    fn calculate_std_dev(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        
        variance.sqrt()
    }
    
    /// Calculate monthly returns from equity curve
    fn calculate_monthly_returns(&self, equity_curve: &[EquityPoint]) -> Vec<f64> {
        let mut monthly_returns = Vec::new();
        let mut current_month_start = 0;
        
        for (i, point) in equity_curve.iter().enumerate() {
            if i == 0 {
                continue;
            }
            
            let current_month = point.timestamp.month();
            let prev_month = equity_curve[i - 1].timestamp.month();
            
            if current_month != prev_month || i == equity_curve.len() - 1 {
                let start_equity = equity_curve[current_month_start].equity;
                let end_equity = equity_curve[i - 1].equity;
                let monthly_return = (end_equity - start_equity) / start_equity;
                monthly_returns.push(monthly_return);
                current_month_start = i;
            }
        }
        
        monthly_returns
    }
    
    /// Sort returns for VaR calculation
    fn sort_returns(&self, returns: &[f64]) -> Vec<f64> {
        let mut sorted = returns.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }
}

/// Portfolio management
struct Portfolio {
    cash: f64,
    initial_capital: f64,
    positions: HashMap<String, OpenPosition>,
    closed_trades: Vec<Trade>,
    total_equity: f64,
    positions_value: f64,
}

impl Portfolio {
    fn new(initial_capital: f64) -> Self {
        Self {
            cash: initial_capital,
            initial_capital,
            positions: HashMap::new(),
            closed_trades: Vec::new(),
            total_equity: initial_capital,
            positions_value: 0.0,
        }
    }
    
    fn open_position(&mut self, trade: Trade, execution_price: f64) -> Result<(), BacktestError> {
        let cost = trade.size * execution_price + trade.commission;
        
        if cost > self.cash {
            return Err(BacktestError::RiskLimitExceeded("Insufficient cash".to_string()));
        }
        
        self.cash -= cost;
        
        let position = OpenPosition {
            trade: trade.clone(),
            current_price: execution_price,
            unrealized_pnl: -trade.commission,
            high_water_mark: execution_price,
            low_water_mark: execution_price,
        };
        
        self.positions.insert(trade.id.clone(), position);
        self.update_equity();
        
        Ok(())
    }
    
    fn close_position(
        &mut self,
        position_id: &str,
        exit_price: f64,
        exit_time: DateTime<Utc>,
    ) -> Result<Trade, BacktestError> {
        let mut position = self.positions.remove(position_id)
            .ok_or_else(|| BacktestError::Execution("Position not found".to_string()))?;
        
        position.trade.exit_price = exit_price;
        position.trade.exit_time = exit_time;
        
        let gross_pnl = match position.trade.side {
            TradeSide::Long => (exit_price - position.trade.entry_price) * position.trade.size,
            TradeSide::Short => (position.trade.entry_price - exit_price) * position.trade.size,
        };
        
        position.trade.pnl = gross_pnl - position.trade.commission;
        position.trade.pnl_percent = gross_pnl / (position.trade.entry_price * position.trade.size);
        
        position.trade.mfe = match position.trade.side {
            TradeSide::Long => (position.high_water_mark - position.trade.entry_price) * position.trade.size,
            TradeSide::Short => (position.trade.entry_price - position.low_water_mark) * position.trade.size,
        };
        
        position.trade.mae = match position.trade.side {
            TradeSide::Long => (position.low_water_mark - position.trade.entry_price) * position.trade.size,
            TradeSide::Short => (position.trade.entry_price - position.high_water_mark) * position.trade.size,
        };
        
        self.cash += position.trade.size * exit_price - position.trade.commission;
        self.closed_trades.push(position.trade.clone());
        self.update_equity();
        
        Ok(position.trade)
    }
    
    fn update_positions(&mut self, market_context: &MarketContext) {
        for position in self.positions.values_mut() {
            if position.trade.symbol == market_context.symbol {
                position.current_price = market_context.current_price;
                position.high_water_mark = position.high_water_mark.max(market_context.current_price);
                position.low_water_mark = position.low_water_mark.min(market_context.current_price);
                
                position.unrealized_pnl = match position.trade.side {
                    TradeSide::Long => {
                        (market_context.current_price - position.trade.entry_price) * position.trade.size
                    }
                    TradeSide::Short => {
                        (position.trade.entry_price - market_context.current_price) * position.trade.size
                    }
                } - position.trade.commission;
            }
        }
        self.update_equity();
    }
    
    fn update_equity(&mut self) {
        self.positions_value = self.positions.values()
            .map(|p| p.trade.size * p.current_price)
            .sum();
        
        let unrealized_pnl: f64 = self.positions.values()
            .map(|p| p.unrealized_pnl)
            .sum();
        
        self.total_equity = self.cash + self.positions_value;
    }
    
    fn get_open_position_id(&self, symbol: &str) -> Option<String> {
        self.positions.iter()
            .find(|(_, p)| p.trade.symbol == symbol)
            .map(|(id, _)| id.clone())
    }
    
    fn get_position(&self, id: &str) -> Result<&OpenPosition, BacktestError> {
        self.positions.get(id)
            .ok_or_else(|| BacktestError::Execution("Position not found".to_string()))
    }
    
    fn get_all_position_ids(&self) -> Vec<String> {
        self.positions.keys().cloned().collect()
    }
    
    fn calculate_win_rate(&self) -> f64 {
        if self.closed_trades.is_empty() {
            return 0.5;
        }
        
        let wins = self.closed_trades.iter().filter(|t| t.pnl > 0.0).count();
        wins as f64 / self.closed_trades.len() as f64
    }
    
    fn calculate_avg_win(&self) -> f64 {
        let winning_trades: Vec<&Trade> = self.closed_trades.iter()
            .filter(|t| t.pnl > 0.0)
            .collect();
        
        if winning_trades.is_empty() {
            return 0.0;
        }
        
        winning_trades.iter().map(|t| t.pnl).sum::<f64>() / winning_trades.len() as f64
    }
    
    fn calculate_avg_loss(&self) -> f64 {
        let losing_trades: Vec<&Trade> = self.closed_trades.iter()
            .filter(|t| t.pnl < 0.0)
            .collect();
        
        if losing_trades.is_empty() {
            return 0.0;
        }
        
        losing_trades.iter().map(|t| t.pnl).sum::<f64>() / losing_trades.len() as f64
    }
    
    fn calculate_optimal_f(&self) -> f64 {
        // Simplified optimal f calculation
        if self.closed_trades.is_empty() {
            return 0.02;
        }
        
        let returns: Vec<f64> = self.closed_trades.iter()
            .map(|t| t.pnl_percent)
            .collect();
        
        // Use Kelly criterion as approximation
        let avg_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - avg_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        if variance > 0.0 {
            (avg_return / variance).max(0.0).min(0.25)
        } else {
            0.02
        }
    }
}

#[derive(Clone)]
struct OpenPosition {
    trade: Trade,
    current_price: f64,
    unrealized_pnl: f64,
    high_water_mark: f64,
    low_water_mark: f64,
}

/// Performance tracking
struct PerformanceTracker {
    trades: Vec<Trade>,
    daily_returns: Vec<f64>,
    equity_curve: Vec<f64>,
}

impl PerformanceTracker {
    fn new() -> Self {
        Self {
            trades: Vec::new(),
            daily_returns: Vec::new(),
            equity_curve: Vec::new(),
        }
    }
}

/// Market data cache for efficient lookups
struct MarketDataCache {
    data: HashMap<String, Vec<TimeSeriesData>>,
    volatility_cache: HashMap<(String, usize), f64>,
}

impl MarketDataCache {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
            volatility_cache: HashMap::new(),
        }
    }
}

/// Transaction cost calculator
struct TransactionCostCalculator;

impl TransactionCostCalculator {
    fn new() -> Self {
        Self
    }
    
    fn calculate_commission(&self, trade_value: f64, commission_rate: f64) -> f64 {
        trade_value * commission_rate
    }
}