//! Walk-forward analysis for robust parameter optimization
//! 
//! This module implements walk-forward optimization to prevent overfitting:
//! - Rolling window optimization
//! - Out-of-sample validation
//! - Parameter stability analysis
//! - Optimization efficiency metrics

use super::*;
use crate::strategies::{TradingStrategy, StrategyConfig};
use crate::data::TimeSeriesData;
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use rayon::prelude::*;

/// Walk-forward analysis engine
pub struct WalkForwardEngine {
    backtest_engine: StandardBacktestEngine,
}

impl WalkForwardEngine {
    pub fn new() -> Self {
        Self {
            backtest_engine: StandardBacktestEngine::new(),
        }
    }
    
    /// Run complete walk-forward analysis
    pub async fn run_analysis(
        &self,
        strategy_factory: Box<dyn StrategyFactory>,
        data: Vec<TimeSeriesData>,
        base_config: BacktestConfig,
        wf_config: WalkForwardConfig,
    ) -> Result<WalkForwardResults, BacktestError> {
        if data.len() < 100 {
            return Err(BacktestError::InsufficientData(
                "Need at least 100 data points for walk-forward analysis".to_string()
            ));
        }
        
        let total_periods = data.len();
        let in_sample_size = (total_periods as f64 * wf_config.in_sample_ratio) as usize;
        let step_size = (total_periods as f64 * wf_config.step_size_ratio) as usize;
        
        let mut in_sample_periods = Vec::new();
        let mut out_of_sample_periods = Vec::new();
        let mut all_optimal_params = Vec::new();
        
        let mut start_idx = 0;
        
        while start_idx + in_sample_size < total_periods {
            // Define in-sample period
            let is_end_idx = start_idx + in_sample_size;
            let is_data = data[start_idx..is_end_idx].to_vec();
            
            // Define out-of-sample period
            let oos_end_idx = (is_end_idx + step_size).min(total_periods);
            let oos_data = data[is_end_idx..oos_end_idx].to_vec();
            
            if oos_data.is_empty() {
                break;
            }
            
            // Optimize on in-sample data
            let optimal_params = self.optimize_parameters(
                &strategy_factory,
                &is_data,
                &base_config,
                &wf_config,
            ).await?;
            
            // Test on in-sample with optimal parameters
            let mut is_strategy = strategy_factory.create_strategy();
            let is_config = self.create_strategy_config(&optimal_params);
            is_strategy.initialize(is_config).await?;
            
            let is_results = self.backtest_engine.run_backtest(
                is_strategy,
                is_data.clone(),
                base_config.clone(),
            ).await?;
            
            // Test on out-of-sample with same parameters
            let mut oos_strategy = strategy_factory.create_strategy();
            let oos_config = self.create_strategy_config(&optimal_params);
            oos_strategy.initialize(oos_config).await?;
            
            let oos_results = self.backtest_engine.run_backtest(
                oos_strategy,
                oos_data.clone(),
                base_config.clone(),
            ).await?;
            
            // Record periods
            in_sample_periods.push(WalkForwardPeriod {
                start_time: is_data.first().unwrap().timestamp,
                end_time: is_data.last().unwrap().timestamp,
                optimal_parameters: optimal_params.clone(),
                performance: is_results.metrics,
            });
            
            out_of_sample_periods.push(WalkForwardPeriod {
                start_time: oos_data.first().unwrap().timestamp,
                end_time: oos_data.last().unwrap().timestamp,
                optimal_parameters: optimal_params.clone(),
                performance: oos_results.metrics,
            });
            
            all_optimal_params.push(optimal_params);
            
            // Move forward
            start_idx += step_size;
        }
        
        // Calculate overall metrics
        let optimization_efficiency = self.calculate_optimization_efficiency(
            &in_sample_periods,
            &out_of_sample_periods,
        );
        
        let parameter_stability = self.calculate_parameter_stability(&all_optimal_params);
        
        let robustness_score = self.calculate_robustness_score(
            optimization_efficiency,
            &parameter_stability,
            &out_of_sample_periods,
        );
        
        Ok(WalkForwardResults {
            in_sample_periods,
            out_of_sample_periods,
            optimization_efficiency,
            parameter_stability,
            robustness_score,
        })
    }
    
    /// Optimize parameters for a given data period
    async fn optimize_parameters(
        &self,
        strategy_factory: &Box<dyn StrategyFactory>,
        data: &[TimeSeriesData],
        base_config: &BacktestConfig,
        wf_config: &WalkForwardConfig,
    ) -> Result<HashMap<String, serde_json::Value>, BacktestError> {
        // Generate parameter combinations
        let combinations = self.generate_parameter_combinations(&wf_config.parameter_ranges);
        
        // Parallel optimization using rayon
        let results: Vec<(HashMap<String, serde_json::Value>, f64)> = combinations
            .into_par_iter()
            .filter_map(|params| {
                // Create strategy with parameters
                let strategy = strategy_factory.create_strategy();
                let config = self.create_strategy_config(&params);
                
                // Run backtest (synchronously in parallel)
                let runtime = tokio::runtime::Runtime::new().ok()?;
                let result = runtime.block_on(async {
                    let mut strat = strategy;
                    strat.initialize(config).await.ok()?;
                    
                    self.backtest_engine.run_backtest(
                        strat,
                        data.to_vec(),
                        base_config.clone(),
                    ).await.ok()
                })?;
                
                // Calculate optimization metric
                let score = match &wf_config.optimization_metric {
                    OptimizationMetric::SharpeRatio => result.metrics.sharpe_ratio,
                    OptimizationMetric::TotalReturn => result.metrics.total_return,
                    OptimizationMetric::ProfitFactor => result.metrics.profit_factor,
                    OptimizationMetric::Expectancy => result.metrics.expectancy,
                    OptimizationMetric::Custom(metric_name) => {
                        // Custom metric implementation
                        self.calculate_custom_metric(metric_name, &result.metrics)
                    }
                };
                
                Some((params, score))
            })
            .collect();
        
        // Find best parameters
        results.into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(params, _)| params)
            .ok_or_else(|| BacktestError::CalculationError("No valid parameter combinations".to_string()))
    }
    
    /// Generate all parameter combinations from ranges
    fn generate_parameter_combinations(
        &self,
        ranges: &HashMap<String, ParameterRange>,
    ) -> Vec<HashMap<String, serde_json::Value>> {
        let mut combinations = vec![HashMap::new()];
        
        for (param_name, range) in ranges {
            let mut new_combinations = Vec::new();
            
            let mut value = range.min;
            while value <= range.max {
                for combo in &combinations {
                    let mut new_combo = combo.clone();
                    new_combo.insert(param_name.clone(), serde_json::json!(value));
                    new_combinations.push(new_combo);
                }
                value += range.step;
            }
            
            combinations = new_combinations;
        }
        
        combinations
    }
    
    /// Create strategy config from parameters
    fn create_strategy_config(&self, params: &HashMap<String, serde_json::Value>) -> StrategyConfig {
        StrategyConfig {
            name: "optimized_strategy".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 1.0,
            parameters: params.clone(),
        }
    }
    
    /// Calculate optimization efficiency (OOS performance / IS performance)
    fn calculate_optimization_efficiency(
        &self,
        in_sample: &[WalkForwardPeriod],
        out_of_sample: &[WalkForwardPeriod],
    ) -> f64 {
        if in_sample.len() != out_of_sample.len() || in_sample.is_empty() {
            return 0.0;
        }
        
        let is_avg_sharpe = in_sample.iter()
            .map(|p| p.performance.sharpe_ratio)
            .sum::<f64>() / in_sample.len() as f64;
        
        let oos_avg_sharpe = out_of_sample.iter()
            .map(|p| p.performance.sharpe_ratio)
            .sum::<f64>() / out_of_sample.len() as f64;
        
        if is_avg_sharpe > 0.0 {
            oos_avg_sharpe / is_avg_sharpe
        } else {
            0.0
        }
    }
    
    /// Calculate parameter stability across periods
    fn calculate_parameter_stability(
        &self,
        all_params: &[HashMap<String, serde_json::Value>],
    ) -> HashMap<String, f64> {
        let mut stability_scores = HashMap::new();
        
        if all_params.len() < 2 {
            return stability_scores;
        }
        
        // Get all parameter names
        let param_names: Vec<String> = all_params.first()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default();
        
        for param_name in param_names {
            let values: Vec<f64> = all_params.iter()
                .filter_map(|params| {
                    params.get(&param_name)
                        .and_then(|v| v.as_f64())
                })
                .collect();
            
            if values.len() >= 2 {
                // Calculate coefficient of variation (CV)
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f64>() / values.len() as f64;
                let std_dev = variance.sqrt();
                
                // CV = std_dev / mean, stability = 1 - CV (capped at 0)
                let cv = if mean != 0.0 { std_dev / mean.abs() } else { 1.0 };
                let stability = (1.0 - cv).max(0.0);
                
                stability_scores.insert(param_name, stability);
            }
        }
        
        stability_scores
    }
    
    /// Calculate overall robustness score
    fn calculate_robustness_score(
        &self,
        optimization_efficiency: f64,
        parameter_stability: &HashMap<String, f64>,
        out_of_sample_periods: &[WalkForwardPeriod],
    ) -> f64 {
        let mut score_components = Vec::new();
        
        // Component 1: Optimization efficiency (40% weight)
        // Good if > 0.5, excellent if > 0.7
        let efficiency_score = optimization_efficiency.min(1.0) * 0.4;
        score_components.push(efficiency_score);
        
        // Component 2: Parameter stability (30% weight)
        if !parameter_stability.is_empty() {
            let avg_stability = parameter_stability.values().sum::<f64>() 
                / parameter_stability.len() as f64;
            score_components.push(avg_stability * 0.3);
        }
        
        // Component 3: OOS consistency (30% weight)
        if out_of_sample_periods.len() >= 2 {
            let oos_sharpes: Vec<f64> = out_of_sample_periods.iter()
                .map(|p| p.performance.sharpe_ratio)
                .collect();
            
            let mean_sharpe = oos_sharpes.iter().sum::<f64>() / oos_sharpes.len() as f64;
            let variance = oos_sharpes.iter()
                .map(|s| (s - mean_sharpe).powi(2))
                .sum::<f64>() / oos_sharpes.len() as f64;
            let std_dev = variance.sqrt();
            
            // Consistency score based on coefficient of variation
            let cv = if mean_sharpe != 0.0 { std_dev / mean_sharpe.abs() } else { 1.0 };
            let consistency_score = (1.0 - cv.min(1.0)) * 0.3;
            score_components.push(consistency_score);
        }
        
        // Sum all components for final score (0-1 scale)
        score_components.iter().sum::<f64>().min(1.0).max(0.0)
    }
    
    /// Calculate custom optimization metric
    fn calculate_custom_metric(&self, metric_name: &str, performance: &PerformanceMetrics) -> f64 {
        match metric_name {
            "risk_adjusted_return" => {
                // Custom metric: Return per unit of max drawdown
                if performance.max_drawdown > 0.0 {
                    performance.total_return / (performance.max_drawdown / 100.0)
                } else {
                    performance.total_return
                }
            }
            "consistency" => {
                // Custom metric: Win rate * profit factor
                performance.win_rate * performance.profit_factor
            }
            "efficiency" => {
                // Custom metric: Net profit after costs / gross profit
                if performance.net_profit + performance.total_commission_paid + performance.total_slippage_cost > 0.0 {
                    performance.net_profit / (performance.net_profit + performance.total_commission_paid + performance.total_slippage_cost)
                } else {
                    0.0
                }
            }
            _ => performance.sharpe_ratio, // Default to Sharpe
        }
    }
}

/// Strategy factory trait for creating strategy instances
#[async_trait]
pub trait StrategyFactory: Send + Sync {
    /// Create a new strategy instance
    fn create_strategy(&self) -> Box<dyn TradingStrategy>;
}

/// Anchored walk-forward analysis
/// Uses fixed anchor point and expanding window
pub struct AnchoredWalkForward {
    engine: WalkForwardEngine,
}

impl AnchoredWalkForward {
    pub fn new() -> Self {
        Self {
            engine: WalkForwardEngine::new(),
        }
    }
    
    /// Run anchored walk-forward (expanding window)
    pub async fn run_analysis(
        &self,
        strategy_factory: Box<dyn StrategyFactory>,
        data: Vec<TimeSeriesData>,
        base_config: BacktestConfig,
        wf_config: WalkForwardConfig,
        min_training_size: usize,
    ) -> Result<WalkForwardResults, BacktestError> {
        let mut in_sample_periods = Vec::new();
        let mut out_of_sample_periods = Vec::new();
        let mut all_optimal_params = Vec::new();
        
        let step_size = (data.len() as f64 * wf_config.step_size_ratio) as usize;
        let mut current_size = min_training_size;
        
        while current_size < data.len() {
            // Expanding in-sample window (always starts from beginning)
            let is_data = data[..current_size].to_vec();
            
            // Out-of-sample window
            let oos_end = (current_size + step_size).min(data.len());
            if oos_end <= current_size {
                break;
            }
            
            let oos_data = data[current_size..oos_end].to_vec();
            
            // Optimize and test
            let optimal_params = self.engine.optimize_parameters(
                &strategy_factory,
                &is_data,
                &base_config,
                &wf_config,
            ).await?;
            
            // Record results (similar to regular walk-forward)
            // ... (implementation similar to regular walk-forward)
            
            all_optimal_params.push(optimal_params);
            current_size += step_size;
        }
        
        // Calculate metrics
        let optimization_efficiency = self.engine.calculate_optimization_efficiency(
            &in_sample_periods,
            &out_of_sample_periods,
        );
        
        let parameter_stability = self.engine.calculate_parameter_stability(&all_optimal_params);
        
        let robustness_score = self.engine.calculate_robustness_score(
            optimization_efficiency,
            &parameter_stability,
            &out_of_sample_periods,
        );
        
        Ok(WalkForwardResults {
            in_sample_periods,
            out_of_sample_periods,
            optimization_efficiency,
            parameter_stability,
            robustness_score,
        })
    }
}

/// Combinatorial purged cross-validation (CPCV)
/// Advanced method that handles time series data properly
pub struct CombinatorialPurgedCV {
    n_splits: usize,
    purge_gap: usize,
}

impl CombinatorialPurgedCV {
    pub fn new(n_splits: usize, purge_gap: usize) -> Self {
        Self { n_splits, purge_gap }
    }
    
    /// Generate train/test splits with purging
    pub fn split(&self, data: &[TimeSeriesData]) -> Vec<(Vec<usize>, Vec<usize>)> {
        let n = data.len();
        let test_size = n / self.n_splits;
        let mut splits = Vec::new();
        
        for i in 0..self.n_splits {
            let test_start = i * test_size;
            let test_end = ((i + 1) * test_size).min(n);
            
            let mut train_indices = Vec::new();
            let mut test_indices = Vec::new();
            
            // Add test indices
            for j in test_start..test_end {
                test_indices.push(j);
            }
            
            // Add train indices with purging
            for j in 0..n {
                // Skip if within purge gap of test set
                if j < test_start.saturating_sub(self.purge_gap) || 
                   j >= test_end + self.purge_gap {
                    train_indices.push(j);
                }
            }
            
            if !train_indices.is_empty() && !test_indices.is_empty() {
                splits.push((train_indices, test_indices));
            }
        }
        
        splits
    }
}