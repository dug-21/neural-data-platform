//! Monte Carlo simulation for robustness testing
//! 
//! This module implements various Monte Carlo methods to test strategy robustness:
//! - Trade randomization
//! - Return path simulation
//! - Parameter sensitivity analysis
//! - Confidence interval calculation

use super::*;
use rand::{Rng, SeedableRng, rngs::StdRng};
use rand::seq::SliceRandom;
use statrs::distribution::{Normal, ContinuousCDF};
use std::collections::HashMap;

/// Monte Carlo simulation engine
pub struct MonteCarloEngine {
    rng: StdRng,
}

impl MonteCarloEngine {
    pub fn new(seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };
        
        Self { rng }
    }
    
    /// Run Monte Carlo simulation on backtest results
    pub async fn run_simulation(
        &mut self,
        base_results: &BacktestResults,
        num_simulations: u32,
        config: MonteCarloConfig,
    ) -> Result<MonteCarloResults, BacktestError> {
        let mut simulation_results = Vec::with_capacity(num_simulations as usize);
        let mut return_distribution = Vec::with_capacity(num_simulations as usize);
        let mut max_drawdown_distribution = Vec::with_capacity(num_simulations as usize);
        let mut sharpe_distribution = Vec::with_capacity(num_simulations as usize);
        let mut var_distribution = Vec::with_capacity(num_simulations as usize);
        
        // Extract base statistics
        let base_trades = &base_results.trades;
        let base_returns: Vec<f64> = base_results.equity_curve.iter()
            .map(|p| p.daily_return)
            .collect();
        
        for sim_idx in 0..num_simulations {
            let simulated_equity = match config.bootstrap_method {
                BootstrapMethod::Simple => {
                    self.simple_bootstrap(&base_returns, &base_trades, &config)?
                }
                BootstrapMethod::BlockBootstrap { block_size } => {
                    self.block_bootstrap(&base_returns, &base_trades, block_size, &config)?
                }
                BootstrapMethod::StationaryBootstrap { avg_block_size } => {
                    self.stationary_bootstrap(&base_returns, &base_trades, avg_block_size, &config)?
                }
            };
            
            // Calculate metrics for this simulation
            let metrics = self.calculate_simulation_metrics(&simulated_equity);
            
            return_distribution.push(metrics.total_return);
            max_drawdown_distribution.push(metrics.max_drawdown);
            sharpe_distribution.push(metrics.sharpe_ratio);
            var_distribution.push(metrics.value_at_risk_95);
            
            simulation_results.push(simulated_equity);
        }
        
        // Calculate confidence intervals
        let confidence_intervals = self.calculate_confidence_intervals(
            &return_distribution,
            &sharpe_distribution,
            &max_drawdown_distribution,
        );
        
        // Calculate probability of ruin
        let probability_of_ruin = self.calculate_probability_of_ruin(&simulation_results);
        
        // Expected maximum drawdown
        let expected_max_drawdown = max_drawdown_distribution.iter().sum::<f64>() 
            / max_drawdown_distribution.len() as f64;
        
        // Create percentile equity curves
        let percentile_equity_curves = self.create_percentile_curves(&simulation_results);
        
        Ok(MonteCarloResults {
            num_simulations,
            confidence_intervals,
            probability_of_ruin,
            expected_max_drawdown,
            var_distribution,
            return_distribution,
            percentile_equity_curves,
        })
    }
    
    /// Simple bootstrap - randomly sample returns with replacement
    fn simple_bootstrap(
        &mut self,
        base_returns: &[f64],
        base_trades: &[Trade],
        config: &MonteCarloConfig,
    ) -> Result<Vec<EquityPoint>, BacktestError> {
        let mut equity_curve = Vec::new();
        let mut current_equity = 10000.0; // Standard starting capital
        
        let num_periods = base_returns.len();
        
        for i in 0..num_periods {
            let return_val = if config.randomize_returns {
                // Sample from return distribution
                let idx = self.rng.gen_range(0..base_returns.len());
                let sampled_return = base_returns[idx];
                
                if config.add_noise {
                    // Add Gaussian noise
                    let noise = Normal::new(0.0, config.noise_level).unwrap();
                    sampled_return + noise.sample(&mut self.rng)
                } else {
                    sampled_return
                }
            } else {
                base_returns[i % base_returns.len()]
            };
            
            current_equity *= 1.0 + return_val;
            
            equity_curve.push(EquityPoint {
                timestamp: chrono::Utc::now(), // Placeholder
                equity: current_equity,
                cash: current_equity * 0.3, // Assume 30% cash
                positions_value: current_equity * 0.7,
                daily_return: return_val,
                cumulative_return: (current_equity - 10000.0) / 10000.0,
            });
        }
        
        Ok(equity_curve)
    }
    
    /// Block bootstrap - preserve autocorrelation by sampling blocks
    fn block_bootstrap(
        &mut self,
        base_returns: &[f64],
        base_trades: &[Trade],
        block_size: usize,
        config: &MonteCarloConfig,
    ) -> Result<Vec<EquityPoint>, BacktestError> {
        let mut equity_curve = Vec::new();
        let mut current_equity = 10000.0;
        
        let num_blocks = (base_returns.len() + block_size - 1) / block_size;
        let mut sampled_returns = Vec::new();
        
        // Sample blocks
        for _ in 0..num_blocks {
            let start_idx = self.rng.gen_range(0..base_returns.len().saturating_sub(block_size));
            let end_idx = (start_idx + block_size).min(base_returns.len());
            
            sampled_returns.extend_from_slice(&base_returns[start_idx..end_idx]);
        }
        
        // Truncate to original length
        sampled_returns.truncate(base_returns.len());
        
        // Build equity curve
        for return_val in sampled_returns {
            let adjusted_return = if config.add_noise {
                let noise = Normal::new(0.0, config.noise_level).unwrap();
                return_val + noise.sample(&mut self.rng)
            } else {
                return_val
            };
            
            current_equity *= 1.0 + adjusted_return;
            
            equity_curve.push(EquityPoint {
                timestamp: chrono::Utc::now(),
                equity: current_equity,
                cash: current_equity * 0.3,
                positions_value: current_equity * 0.7,
                daily_return: adjusted_return,
                cumulative_return: (current_equity - 10000.0) / 10000.0,
            });
        }
        
        Ok(equity_curve)
    }
    
    /// Stationary bootstrap - variable block length
    fn stationary_bootstrap(
        &mut self,
        base_returns: &[f64],
        base_trades: &[Trade],
        avg_block_size: f64,
        config: &MonteCarloConfig,
    ) -> Result<Vec<EquityPoint>, BacktestError> {
        let mut equity_curve = Vec::new();
        let mut current_equity = 10000.0;
        
        let p = 1.0 / avg_block_size; // Probability of starting new block
        let mut sampled_returns = Vec::new();
        let mut current_idx = self.rng.gen_range(0..base_returns.len());
        
        while sampled_returns.len() < base_returns.len() {
            sampled_returns.push(base_returns[current_idx]);
            
            // Decide whether to continue block or start new one
            if self.rng.gen::<f64>() < p {
                // Start new block
                current_idx = self.rng.gen_range(0..base_returns.len());
            } else {
                // Continue current block
                current_idx = (current_idx + 1) % base_returns.len();
            }
        }
        
        // Build equity curve
        for return_val in sampled_returns {
            let adjusted_return = if config.add_noise {
                let noise = Normal::new(0.0, config.noise_level).unwrap();
                return_val + noise.sample(&mut self.rng)
            } else {
                return_val
            };
            
            current_equity *= 1.0 + adjusted_return;
            
            equity_curve.push(EquityPoint {
                timestamp: chrono::Utc::now(),
                equity: current_equity,
                cash: current_equity * 0.3,
                positions_value: current_equity * 0.7,
                daily_return: adjusted_return,
                cumulative_return: (current_equity - 10000.0) / 10000.0,
            });
        }
        
        Ok(equity_curve)
    }
    
    /// Calculate metrics for a simulated equity curve
    fn calculate_simulation_metrics(&self, equity_curve: &[EquityPoint]) -> SimulationMetrics {
        let initial_equity = equity_curve.first().map(|p| p.equity).unwrap_or(10000.0);
        let final_equity = equity_curve.last().map(|p| p.equity).unwrap_or(initial_equity);
        
        let total_return = (final_equity - initial_equity) / initial_equity;
        
        // Calculate returns
        let returns: Vec<f64> = equity_curve.iter().map(|p| p.daily_return).collect();
        
        // Volatility
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let volatility = variance.sqrt() * (252.0_f64).sqrt();
        
        // Sharpe ratio
        let risk_free_rate = 0.02 / 252.0; // Daily risk-free rate
        let excess_returns: Vec<f64> = returns.iter()
            .map(|r| r - risk_free_rate)
            .collect();
        let sharpe_ratio = if volatility > 0.0 {
            (mean_return - risk_free_rate) * (252.0_f64).sqrt() / volatility
        } else {
            0.0
        };
        
        // Maximum drawdown
        let mut peak = initial_equity;
        let mut max_drawdown = 0.0;
        
        for point in equity_curve {
            if point.equity > peak {
                peak = point.equity;
            }
            let drawdown = (peak - point.equity) / peak;
            max_drawdown = max_drawdown.max(drawdown);
        }
        
        // VaR calculation
        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let var_index = ((sorted_returns.len() as f64) * 0.05) as usize;
        let value_at_risk_95 = sorted_returns.get(var_index).copied().unwrap_or(0.0);
        
        SimulationMetrics {
            total_return,
            volatility,
            sharpe_ratio,
            max_drawdown: max_drawdown * 100.0,
            value_at_risk_95,
        }
    }
    
    /// Calculate confidence intervals for key metrics
    fn calculate_confidence_intervals(
        &self,
        returns: &[f64],
        sharpes: &[f64],
        drawdowns: &[f64],
    ) -> ConfidenceIntervals {
        let return_95 = self.calculate_percentile_range(returns, 0.025, 0.975);
        let return_99 = self.calculate_percentile_range(returns, 0.005, 0.995);
        let sharpe_95 = self.calculate_percentile_range(sharpes, 0.025, 0.975);
        let max_drawdown_95 = self.calculate_percentile_range(drawdowns, 0.025, 0.975);
        
        ConfidenceIntervals {
            return_95,
            return_99,
            sharpe_95,
            max_drawdown_95,
        }
    }
    
    /// Calculate percentile range
    fn calculate_percentile_range(&self, data: &[f64], lower_pct: f64, upper_pct: f64) -> (f64, f64) {
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let lower_idx = ((sorted.len() as f64) * lower_pct) as usize;
        let upper_idx = ((sorted.len() as f64) * upper_pct) as usize;
        
        let lower = sorted.get(lower_idx).copied().unwrap_or(0.0);
        let upper = sorted.get(upper_idx.min(sorted.len() - 1)).copied().unwrap_or(0.0);
        
        (lower, upper)
    }
    
    /// Calculate probability of ruin (ending below starting capital)
    fn calculate_probability_of_ruin(&self, simulations: &[Vec<EquityPoint>]) -> f64 {
        let ruin_count = simulations.iter()
            .filter(|sim| {
                sim.last().map(|p| p.equity < 10000.0).unwrap_or(false)
            })
            .count();
        
        ruin_count as f64 / simulations.len() as f64
    }
    
    /// Create percentile equity curves
    fn create_percentile_curves(
        &self,
        simulations: &[Vec<EquityPoint>],
    ) -> HashMap<String, Vec<EquityPoint>> {
        let mut percentile_curves = HashMap::new();
        
        if simulations.is_empty() {
            return percentile_curves;
        }
        
        let num_points = simulations[0].len();
        let percentiles = vec![
            ("p5".to_string(), 0.05),
            ("p25".to_string(), 0.25),
            ("p50".to_string(), 0.50),
            ("p75".to_string(), 0.75),
            ("p95".to_string(), 0.95),
        ];
        
        for (name, pct) in percentiles {
            let mut curve = Vec::new();
            
            for i in 0..num_points {
                let mut values_at_time: Vec<f64> = simulations.iter()
                    .filter_map(|sim| sim.get(i).map(|p| p.equity))
                    .collect();
                
                values_at_time.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                
                let idx = ((values_at_time.len() as f64) * pct) as usize;
                let percentile_value = values_at_time.get(idx.min(values_at_time.len() - 1))
                    .copied()
                    .unwrap_or(10000.0);
                
                curve.push(EquityPoint {
                    timestamp: chrono::Utc::now(),
                    equity: percentile_value,
                    cash: percentile_value * 0.3,
                    positions_value: percentile_value * 0.7,
                    daily_return: 0.0,
                    cumulative_return: (percentile_value - 10000.0) / 10000.0,
                });
            }
            
            percentile_curves.insert(name, curve);
        }
        
        percentile_curves
    }
}

/// Metrics for individual simulation
struct SimulationMetrics {
    total_return: f64,
    volatility: f64,
    sharpe_ratio: f64,
    max_drawdown: f64,
    value_at_risk_95: f64,
}

/// Trade randomization for robustness testing
pub struct TradeRandomizer {
    rng: StdRng,
}

impl TradeRandomizer {
    pub fn new(seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };
        
        Self { rng }
    }
    
    /// Randomize trade order while preserving trade characteristics
    pub fn randomize_trades(&mut self, trades: &[Trade]) -> Vec<Trade> {
        let mut randomized = trades.to_vec();
        randomized.shuffle(&mut self.rng);
        
        // Recalculate timestamps to maintain chronological order
        let sorted_timestamps: Vec<_> = trades.iter()
            .map(|t| t.entry_time)
            .collect();
        
        for (i, trade) in randomized.iter_mut().enumerate() {
            if i < sorted_timestamps.len() {
                let duration = trade.exit_time - trade.entry_time;
                trade.entry_time = sorted_timestamps[i];
                trade.exit_time = sorted_timestamps[i] + duration;
            }
        }
        
        randomized
    }
    
    /// Add noise to trade results
    pub fn add_trade_noise(&mut self, trades: &[Trade], noise_level: f64) -> Vec<Trade> {
        let noise_dist = Normal::new(0.0, noise_level).unwrap();
        
        trades.iter().map(|trade| {
            let mut noisy_trade = trade.clone();
            
            // Add noise to PnL
            let pnl_noise = noise_dist.sample(&mut self.rng) * trade.pnl.abs();
            noisy_trade.pnl += pnl_noise;
            noisy_trade.pnl_percent = noisy_trade.pnl / (trade.entry_price * trade.size);
            
            // Add noise to execution prices
            let price_noise_entry = noise_dist.sample(&mut self.rng) * trade.entry_price * 0.001;
            let price_noise_exit = noise_dist.sample(&mut self.rng) * trade.exit_price * 0.001;
            
            noisy_trade.entry_price += price_noise_entry;
            noisy_trade.exit_price += price_noise_exit;
            
            noisy_trade
        }).collect()
    }
}

/// Parameter sensitivity analysis
pub struct ParameterSensitivity {
    parameter_ranges: HashMap<String, Vec<f64>>,
}

impl ParameterSensitivity {
    pub fn new() -> Self {
        Self {
            parameter_ranges: HashMap::new(),
        }
    }
    
    /// Add parameter range for sensitivity analysis
    pub fn add_parameter(&mut self, name: String, min: f64, max: f64, steps: usize) {
        let range: Vec<f64> = (0..steps)
            .map(|i| min + (max - min) * (i as f64) / ((steps - 1) as f64))
            .collect();
        
        self.parameter_ranges.insert(name, range);
    }
    
    /// Generate parameter combinations for testing
    pub fn generate_combinations(&self) -> Vec<HashMap<String, f64>> {
        let mut combinations = vec![HashMap::new()];
        
        for (param_name, values) in &self.parameter_ranges {
            let mut new_combinations = Vec::new();
            
            for value in values {
                for combo in &combinations {
                    let mut new_combo = combo.clone();
                    new_combo.insert(param_name.clone(), *value);
                    new_combinations.push(new_combo);
                }
            }
            
            combinations = new_combinations;
        }
        
        combinations
    }
}