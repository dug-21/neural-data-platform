//! Cross-Asset Correlation Analysis
//! 
//! Analyzes correlations and relationships between multiple assets
//! for enhanced predictive power and risk management.

use anyhow::Result;
use std::collections::HashMap;
use nalgebra::{DMatrix, DVector};
use crate::data::TimeSeriesData;
use serde::{Deserialize, Serialize};

/// Correlation regime types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CorrelationRegime {
    HighCorrelation,
    LowCorrelation,
    NegativeCorrelation,
    StableCorrelation,
    VolatileCorrelation,
    TrendingUp,
    TrendingDown,
}

/// Sector rotation signal strength
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RotationSignal {
    Strong,
    Moderate,
    Weak,
    None,
}

/// Cross-asset correlation engine
pub struct CrossAssetCorrelationEngine {
    /// Correlation lookback periods
    correlation_periods: Vec<usize>,
    
    /// Rolling correlation windows (multiple windows for dynamic analysis)
    rolling_windows: Vec<usize>,
    
    /// Minimum correlation threshold
    correlation_threshold: f64,
    
    /// Enable dynamic correlation
    enable_dynamic_correlation: bool,
    
    /// Regime detection sensitivity
    regime_sensitivity: f64,
    
    /// Momentum lookback periods
    momentum_periods: Vec<usize>,
    
    /// Carry trade threshold
    carry_threshold: f64,
}

impl CrossAssetCorrelationEngine {
    /// Create a new cross-asset correlation engine
    pub fn new() -> Self {
        Self {
            correlation_periods: vec![20, 60, 120, 252],
            rolling_windows: vec![10, 20, 40, 60],
            correlation_threshold: 0.3,
            enable_dynamic_correlation: true,
            regime_sensitivity: 0.15,
            momentum_periods: vec![5, 10, 20, 60],
            carry_threshold: 0.02,
        }
    }
    
    /// Compute cross-asset correlations
    pub async fn compute_correlations(
        &self,
        target_symbol: &str,
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Get target asset data
        let target_data = market_context.get(target_symbol)
            .ok_or_else(|| anyhow::anyhow!("Target symbol not found in market context"))?;
        
        // Major market indices correlations
        self.compute_index_correlations(target_data, market_context, &mut features)?;
        
        // Sector correlations
        self.compute_sector_correlations(target_data, market_context, &mut features)?;
        
        // Currency correlations
        self.compute_currency_correlations(target_data, market_context, &mut features)?;
        
        // Commodity correlations
        self.compute_commodity_correlations(target_data, market_context, &mut features)?;
        
        // Interest rate correlations
        self.compute_rate_correlations(target_data, market_context, &mut features)?;
        
        // Dynamic correlation analysis with multiple windows
        if self.enable_dynamic_correlation {
            self.compute_dynamic_correlations(target_data, market_context, &mut features)?;
        }
        
        // Correlation regime detection
        self.compute_correlation_regimes(target_data, market_context, &mut features)?;
        
        // Cross-asset momentum indicators
        self.compute_cross_asset_momentum(target_data, market_context, &mut features)?;
        
        // Sector rotation signals
        self.compute_sector_rotation_signals(target_data, market_context, &mut features)?;
        
        // Currency carry trade indicators
        self.compute_carry_trade_indicators(target_data, market_context, &mut features)?;
        
        // Enhanced beta calculations
        self.compute_market_betas(target_data, market_context, &mut features)?;
        
        // Advanced correlation stability metrics
        self.compute_correlation_stability(target_data, market_context, &mut features)?;
        
        Ok(features)
    }
    
    /// Compute correlations with major indices
    fn compute_index_correlations(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let indices = vec!["SPY", "QQQ", "IWM", "DIA", "VIX"];
        
        for index in indices {
            if let Some(index_data) = market_context.get(index) {
                for &period in &self.correlation_periods {
                    if let Ok(correlation) = self.calculate_correlation(target_data, index_data, period) {
                        features.insert(
                            format!("corr_{}_{}", index.to_lowercase(), period),
                            correlation,
                        );
                        
                        // Correlation strength indicator
                        if correlation.abs() > self.correlation_threshold {
                            features.insert(
                                format!("strong_corr_{}_{}", index.to_lowercase(), period),
                                correlation.signum(),
                            );
                        }
                    }
                }
                
                // Multiple rolling correlation windows
                for &window in &self.rolling_windows {
                    if let Ok(rolling_corr) = self.calculate_rolling_correlation(
                        target_data,
                        index_data,
                        window,
                    ) {
                        features.insert(
                            format!("rolling_corr_{}_{}", index.to_lowercase(), window),
                            rolling_corr,
                        );
                        
                        // Correlation strength at different windows
                        if rolling_corr.abs() > self.correlation_threshold {
                            features.insert(
                                format!("strong_rolling_corr_{}_{}", index.to_lowercase(), window),
                                rolling_corr.signum(),
                            );
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute sector correlations
    fn compute_sector_correlations(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let sectors = vec![
            ("XLF", "financials"),
            ("XLK", "technology"),
            ("XLE", "energy"),
            ("XLV", "healthcare"),
            ("XLI", "industrials"),
            ("XLY", "consumer_disc"),
            ("XLP", "consumer_stap"),
            ("XLU", "utilities"),
            ("XLB", "materials"),
            ("XLRE", "real_estate"),
        ];
        
        for (symbol, name) in sectors {
            if let Some(sector_data) = market_context.get(symbol) {
                if let Ok(correlation) = self.calculate_correlation(target_data, sector_data, 60) {
                    features.insert(format!("sector_corr_{}", name), correlation);
                }
            }
        }
        
        // Find dominant sector correlation
        let mut max_corr = 0.0;
        let mut dominant_sector = "";
        
        for (_, name) in &sectors {
            if let Some(&corr) = features.get(&format!("sector_corr_{}", name)) {
                if corr.abs() > max_corr.abs() {
                    max_corr = corr;
                    dominant_sector = name;
                }
            }
        }
        
        if !dominant_sector.is_empty() {
            features.insert("dominant_sector_corr".to_string(), max_corr);
            features.insert(
                format!("is_dominant_sector_{}", dominant_sector),
                1.0,
            );
        }
        
        Ok(())
    }
    
    /// Compute currency correlations
    fn compute_currency_correlations(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let currencies = vec![
            ("DXY", "dollar_index"),
            ("EUR", "euro"),
            ("JPY", "yen"),
            ("GBP", "pound"),
            ("AUD", "aussie"),
            ("CAD", "canadian"),
        ];
        
        for (symbol, name) in currencies {
            if let Some(currency_data) = market_context.get(symbol) {
                if let Ok(correlation) = self.calculate_correlation(target_data, currency_data, 20) {
                    features.insert(format!("currency_corr_{}", name), correlation);
                    
                    // Currency sensitivity
                    if correlation.abs() > 0.5 {
                        features.insert(format!("currency_sensitive_{}", name), 1.0);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute commodity correlations
    fn compute_commodity_correlations(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let commodities = vec![
            ("GLD", "gold"),
            ("SLV", "silver"),
            ("USO", "oil"),
            ("UNG", "nat_gas"),
            ("DBA", "agriculture"),
            ("DBB", "base_metals"),
        ];
        
        for (symbol, name) in commodities {
            if let Some(commodity_data) = market_context.get(symbol) {
                if let Ok(correlation) = self.calculate_correlation(target_data, commodity_data, 60) {
                    features.insert(format!("commodity_corr_{}", name), correlation);
                }
                
                // Lead-lag analysis
                if let Ok(lead_lag) = self.calculate_lead_lag_correlation(
                    target_data,
                    commodity_data,
                    5,
                ) {
                    features.insert(format!("lead_lag_{}", name), lead_lag);
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute interest rate correlations
    fn compute_rate_correlations(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let rates = vec![
            ("TLT", "long_term_rates"),
            ("IEF", "medium_term_rates"),
            ("SHY", "short_term_rates"),
            ("TIP", "inflation_protected"),
            ("HYG", "high_yield"),
            ("LQD", "investment_grade"),
        ];
        
        for (symbol, name) in rates {
            if let Some(rate_data) = market_context.get(symbol) {
                if let Ok(correlation) = self.calculate_correlation(target_data, rate_data, 60) {
                    features.insert(format!("rate_corr_{}", name), correlation);
                    
                    // Rate sensitivity classification
                    if correlation < -0.3 {
                        features.insert("rate_sensitive_negative".to_string(), 1.0);
                    } else if correlation > 0.3 {
                        features.insert("rate_sensitive_positive".to_string(), 1.0);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute dynamic correlations using DCC-GARCH approach
    fn compute_dynamic_correlations(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Simplified DCC calculation
        let key_assets = vec!["SPY", "TLT", "GLD", "DXY"];
        
        for asset in key_assets {
            if let Some(asset_data) = market_context.get(asset) {
                // Calculate time-varying correlation
                let correlations = self.calculate_time_varying_correlation(
                    target_data,
                    asset_data,
                    20,
                )?;
                
                if let Some(latest_corr) = correlations.last() {
                    features.insert(format!("dcc_{}", asset.to_lowercase()), *latest_corr);
                    
                    // Correlation trend
                    if correlations.len() >= 5 {
                        let recent_avg = correlations[correlations.len() - 5..]
                            .iter()
                            .sum::<f64>() / 5.0;
                        let older_avg = correlations[correlations.len() - 10..correlations.len() - 5]
                            .iter()
                            .sum::<f64>() / 5.0;
                        
                        let corr_trend = recent_avg - older_avg;
                        features.insert(
                            format!("dcc_trend_{}", asset.to_lowercase()),
                            corr_trend,
                        );
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute market betas
    fn compute_market_betas(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Calculate beta against major indices
        let benchmarks = vec![("SPY", "market"), ("QQQ", "tech"), ("IWM", "small_cap")];
        
        for (symbol, name) in benchmarks {
            if let Some(benchmark_data) = market_context.get(symbol) {
                for &period in &[20, 60, 252] {
                    if let Ok(beta) = self.calculate_beta(target_data, benchmark_data, period) {
                        features.insert(format!("beta_{}_{}", name, period), beta);
                        
                        // Beta classification
                        if beta > 1.5 {
                            features.insert(format!("high_beta_{}", name), 1.0);
                        } else if beta < 0.5 {
                            features.insert(format!("low_beta_{}", name), 1.0);
                        }
                    }
                }
                
                // Rolling beta
                if let Ok(rolling_beta) = self.calculate_rolling_beta(
                    target_data,
                    benchmark_data,
                    60,
                    20,
                ) {
                    features.insert(format!("rolling_beta_{}", name), rolling_beta);
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute correlation stability metrics
    fn compute_correlation_stability(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        if let Some(spy_data) = market_context.get("SPY") {
            // Calculate correlation over different windows
            let windows = vec![20, 40, 60, 120];
            let mut correlations = Vec::new();
            
            for &window in &windows {
                if let Ok(corr) = self.calculate_correlation(target_data, spy_data, window) {
                    correlations.push(corr);
                }
            }
            
            if correlations.len() >= 2 {
                // Correlation stability (standard deviation of correlations)
                let mean_corr = correlations.iter().sum::<f64>() / correlations.len() as f64;
                let stability = correlations.iter()
                    .map(|&c| (c - mean_corr).powi(2))
                    .sum::<f64>() / correlations.len() as f64;
                
                features.insert("correlation_stability".to_string(), stability.sqrt());
                
                // Correlation regime
                if mean_corr > 0.7 {
                    features.insert("high_correlation_regime".to_string(), 1.0);
                } else if mean_corr < -0.3 {
                    features.insert("negative_correlation_regime".to_string(), 1.0);
                } else if stability.sqrt() < 0.1 {
                    features.insert("stable_correlation_regime".to_string(), 1.0);
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute correlation regimes using advanced detection
    fn compute_correlation_regimes(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        if let Some(spy_data) = market_context.get("SPY") {
            // Calculate correlations across multiple windows
            let mut regime_correlations = Vec::new();
            
            for &window in &self.rolling_windows {
                if let Ok(corr) = self.calculate_correlation(target_data, spy_data, window) {
                    regime_correlations.push(corr);
                }
            }
            
            if !regime_correlations.is_empty() {
                let mean_corr = regime_correlations.iter().sum::<f64>() / regime_correlations.len() as f64;
                let corr_std = {
                    let variance = regime_correlations.iter()
                        .map(|&c| (c - mean_corr).powi(2))
                        .sum::<f64>() / regime_correlations.len() as f64;
                    variance.sqrt()
                };
                
                // Detect regime
                let regime = if mean_corr > 0.7 {
                    CorrelationRegime::HighCorrelation
                } else if mean_corr < -0.3 {
                    CorrelationRegime::NegativeCorrelation
                } else if corr_std < self.regime_sensitivity {
                    CorrelationRegime::StableCorrelation
                } else if corr_std > 0.4 {
                    CorrelationRegime::VolatileCorrelation
                } else if regime_correlations.len() >= 2 && 
                         regime_correlations.last().unwrap() > regime_correlations.first().unwrap() {
                    CorrelationRegime::TrendingUp
                } else if regime_correlations.len() >= 2 && 
                         regime_correlations.last().unwrap() < regime_correlations.first().unwrap() {
                    CorrelationRegime::TrendingDown
                } else {
                    CorrelationRegime::LowCorrelation
                };
                
                // Store regime features
                features.insert("correlation_regime_mean".to_string(), mean_corr);
                features.insert("correlation_regime_volatility".to_string(), corr_std);
                
                match regime {
                    CorrelationRegime::HighCorrelation => features.insert("regime_high_corr".to_string(), 1.0),
                    CorrelationRegime::NegativeCorrelation => features.insert("regime_negative_corr".to_string(), 1.0),
                    CorrelationRegime::StableCorrelation => features.insert("regime_stable_corr".to_string(), 1.0),
                    CorrelationRegime::VolatileCorrelation => features.insert("regime_volatile_corr".to_string(), 1.0),
                    CorrelationRegime::TrendingUp => features.insert("regime_trending_up".to_string(), 1.0),
                    CorrelationRegime::TrendingDown => features.insert("regime_trending_down".to_string(), 1.0),
                    CorrelationRegime::LowCorrelation => features.insert("regime_low_corr".to_string(), 1.0),
                };
            }
        }
        
        Ok(())
    }
    
    /// Compute cross-asset momentum indicators
    fn compute_cross_asset_momentum(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let key_assets = vec![
            ("SPY", "equity"),
            ("TLT", "bond"),
            ("GLD", "gold"),
            ("DXY", "dollar"),
            ("VIX", "volatility"),
        ];
        
        for (symbol, asset_type) in key_assets {
            if let Some(asset_data) = market_context.get(symbol) {
                // Calculate momentum for multiple periods
                for &period in &self.momentum_periods {
                    // Asset momentum
                    if let Ok(asset_momentum) = self.calculate_momentum(asset_data, period) {
                        features.insert(
                            format!("momentum_{}_{}", asset_type, period),
                            asset_momentum,
                        );
                    }
                    
                    // Target asset momentum
                    if let Ok(target_momentum) = self.calculate_momentum(target_data, period) {
                        features.insert(
                            format!("target_momentum_{}", period),
                            target_momentum,
                        );
                        
                        // Relative momentum
                        if let Ok(asset_momentum) = self.calculate_momentum(asset_data, period) {
                            let relative_momentum = target_momentum - asset_momentum;
                            features.insert(
                                format!("relative_momentum_{}_{}", asset_type, period),
                                relative_momentum,
                            );
                            
                            // Momentum divergence signal
                            if relative_momentum.abs() > 0.02 {
                                features.insert(
                                    format!("momentum_divergence_{}_{}", asset_type, period),
                                    relative_momentum.signum(),
                                );
                            }
                        }
                    }
                }
                
                // Cross-asset momentum correlation
                if let Ok(momentum_corr) = self.calculate_momentum_correlation(
                    target_data,
                    asset_data,
                    20,
                ) {
                    features.insert(
                        format!("momentum_corr_{}", asset_type),
                        momentum_corr,
                    );
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute sector rotation signals
    fn compute_sector_rotation_signals(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let sectors = vec![
            ("XLF", "financials"),
            ("XLK", "technology"),
            ("XLE", "energy"),
            ("XLV", "healthcare"),
            ("XLI", "industrials"),
            ("XLY", "consumer_disc"),
            ("XLP", "consumer_stap"),
            ("XLU", "utilities"),
            ("XLB", "materials"),
            ("XLRE", "real_estate"),
        ];
        
        let mut sector_performances = Vec::new();
        let mut sector_momentums = Vec::new();
        
        // Calculate sector performances and momentum
        for (symbol, name) in &sectors {
            if let Some(sector_data) = market_context.get(*symbol) {
                // Recent performance (20-day)
                if let Ok(perf) = self.calculate_momentum(sector_data, 20) {
                    sector_performances.push((name, perf));
                    
                    // Store individual sector performance
                    features.insert(format!("sector_perf_{}", name), perf);
                }
                
                // Medium-term momentum (60-day)
                if let Ok(momentum) = self.calculate_momentum(sector_data, 60) {
                    sector_momentums.push((name, momentum));
                    features.insert(format!("sector_momentum_{}", name), momentum);
                }
            }
        }
        
        // Sort sectors by performance
        sector_performances.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sector_momentums.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Identify rotation patterns
        if !sector_performances.is_empty() {
            let top_performer = sector_performances[0];
            let bottom_performer = sector_performances.last().unwrap();
            
            features.insert("top_sector_performance".to_string(), top_performer.1);
            features.insert("bottom_sector_performance".to_string(), bottom_performer.1);
            features.insert(
                format!("is_top_sector_{}", top_performer.0),
                1.0,
            );
            
            // Performance spread (rotation intensity)
            let performance_spread = top_performer.1 - bottom_performer.1;
            features.insert("sector_rotation_intensity".to_string(), performance_spread);
            
            // Rotation signal strength
            let rotation_signal = if performance_spread > 0.1 {
                RotationSignal::Strong
            } else if performance_spread > 0.05 {
                RotationSignal::Moderate
            } else if performance_spread > 0.02 {
                RotationSignal::Weak
            } else {
                RotationSignal::None
            };
            
            match rotation_signal {
                RotationSignal::Strong => features.insert("rotation_signal_strong".to_string(), 1.0),
                RotationSignal::Moderate => features.insert("rotation_signal_moderate".to_string(), 1.0),
                RotationSignal::Weak => features.insert("rotation_signal_weak".to_string(), 1.0),
                RotationSignal::None => features.insert("rotation_signal_none".to_string(), 1.0),
            };
        }
        
        // Calculate target asset's sector alignment
        if let Some(spy_data) = market_context.get("SPY") {
            for (symbol, name) in &sectors {
                if let Some(sector_data) = market_context.get(*symbol) {
                    // Correlation with target vs correlation with market
                    if let Ok(target_corr) = self.calculate_correlation(target_data, sector_data, 60) {
                        if let Ok(market_corr) = self.calculate_correlation(spy_data, sector_data, 60) {
                            let sector_alignment = target_corr - market_corr;
                            features.insert(
                                format!("sector_alignment_{}", name),
                                sector_alignment,
                            );
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Compute currency carry trade indicators
    fn compute_carry_trade_indicators(
        &self,
        target_data: &[TimeSeriesData],
        market_context: &HashMap<String, Vec<TimeSeriesData>>,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        let carry_pairs = vec![
            ("AUD", "JPY", "audjpy_carry"),
            ("NZD", "JPY", "nzdjpy_carry"),
            ("EUR", "USD", "eurusd_carry"),
            ("GBP", "USD", "gbpusd_carry"),
            ("USD", "CHF", "usdchf_carry"),
        ];
        
        for (base_currency, quote_currency, carry_name) in carry_pairs {
            if let (Some(base_data), Some(quote_data)) = 
                (market_context.get(base_currency), market_context.get(quote_currency)) {
                
                // Calculate carry trade momentum (interest rate differential effect)
                if let Ok(base_momentum) = self.calculate_momentum(base_data, 60) {
                    if let Ok(quote_momentum) = self.calculate_momentum(quote_data, 60) {
                        let carry_momentum = base_momentum - quote_momentum;
                        features.insert(
                            format!("carry_{}_momentum", carry_name),
                            carry_momentum,
                        );
                        
                        // Carry trade signal
                        if carry_momentum > self.carry_threshold {
                            features.insert(format!("carry_{}_bullish", carry_name), 1.0);
                        } else if carry_momentum < -self.carry_threshold {
                            features.insert(format!("carry_{}_bearish", carry_name), 1.0);
                        }
                    }
                }
                
                // Correlation with carry trades
                if let Ok(carry_corr) = self.calculate_carry_correlation(
                    target_data,
                    base_data,
                    quote_data,
                    40,
                ) {
                    features.insert(
                        format!("target_carry_corr_{}", carry_name),
                        carry_corr,
                    );
                    
                    // High carry sensitivity
                    if carry_corr.abs() > 0.4 {
                        features.insert(format!("carry_sensitive_{}", carry_name), 1.0);
                    }
                }
            }
        }
        
        // Dollar strength impact on target
        if let Some(dxy_data) = market_context.get("DXY") {
            // DXY momentum impact
            if let Ok(dxy_momentum) = self.calculate_momentum(dxy_data, 20) {
                features.insert("dxy_momentum_impact".to_string(), dxy_momentum);
                
                // Strong dollar regime
                if dxy_momentum > 0.01 {
                    features.insert("strong_dollar_regime".to_string(), 1.0);
                } else if dxy_momentum < -0.01 {
                    features.insert("weak_dollar_regime".to_string(), 1.0);
                }
            }
            
            // Target's dollar sensitivity
            for &period in &[10, 20, 60] {
                if let Ok(dollar_beta) = self.calculate_beta(target_data, dxy_data, period) {
                    features.insert(
                        format!("dollar_beta_{}", period),
                        dollar_beta,
                    );
                }
            }
        }
        
        Ok(())
    }
    
    // Enhanced helper calculation methods
    
    /// Calculate momentum over a given period
    fn calculate_momentum(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.len() < period + 1 {
            return Err(anyhow::anyhow!("Insufficient data for momentum calculation"));
        }
        
        let current_price = data[data.len() - 1].close;
        let past_price = data[data.len() - period - 1].close;
        
        Ok((current_price / past_price - 1.0).ln())
    }
    
    /// Calculate correlation between momentum series
    fn calculate_momentum_correlation(
        &self,
        data1: &[TimeSeriesData],
        data2: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
        let mut momentum1_series = Vec::new();
        let mut momentum2_series = Vec::new();
        
        // Calculate rolling momentum for both series
        let lookback = 60.min(data1.len()).min(data2.len());
        for i in period..lookback {
            if let (Ok(mom1), Ok(mom2)) = (
                self.calculate_momentum(&data1[..i+1], period),
                self.calculate_momentum(&data2[..i+1], period),
            ) {
                momentum1_series.push(mom1);
                momentum2_series.push(mom2);
            }
        }
        
        self.calculate_correlation_from_returns(&momentum1_series, &momentum2_series)
    }
    
    /// Calculate correlation with carry trade (base vs quote currency)
    fn calculate_carry_correlation(
        &self,
        target_data: &[TimeSeriesData],
        base_data: &[TimeSeriesData],
        quote_data: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
        // Calculate carry trade performance (base - quote)
        let min_len = target_data.len().min(base_data.len()).min(quote_data.len());
        if min_len < period + 1 {
            return Err(anyhow::anyhow!("Insufficient data for carry correlation"));
        }
        
        let target_returns = self.calculate_returns(target_data, period)?;
        let base_returns = self.calculate_returns(base_data, period)?;
        let quote_returns = self.calculate_returns(quote_data, period)?;
        
        // Create carry trade returns (long base, short quote)
        let mut carry_returns = Vec::new();
        for i in 0..base_returns.len().min(quote_returns.len()) {
            carry_returns.push(base_returns[i] - quote_returns[i]);
        }
        
        if carry_returns.len() != target_returns.len() {
            let min_len = carry_returns.len().min(target_returns.len());
            self.calculate_correlation_from_returns(
                &target_returns[target_returns.len() - min_len..],
                &carry_returns[carry_returns.len() - min_len..],
            )
        } else {
            self.calculate_correlation_from_returns(&target_returns, &carry_returns)
        }
    }
    
    fn calculate_correlation(
        &self,
        data1: &[TimeSeriesData],
        data2: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
        let returns1 = self.calculate_returns(data1, period)?;
        let returns2 = self.calculate_returns(data2, period)?;
        
        if returns1.len() != returns2.len() || returns1.is_empty() {
            return Err(anyhow::anyhow!("Mismatched data lengths for correlation"));
        }
        
        let n = returns1.len() as f64;
        let mean1 = returns1.iter().sum::<f64>() / n;
        let mean2 = returns2.iter().sum::<f64>() / n;
        
        let mut covariance = 0.0;
        let mut var1 = 0.0;
        let mut var2 = 0.0;
        
        for i in 0..returns1.len() {
            let dev1 = returns1[i] - mean1;
            let dev2 = returns2[i] - mean2;
            
            covariance += dev1 * dev2;
            var1 += dev1 * dev1;
            var2 += dev2 * dev2;
        }
        
        if var1 == 0.0 || var2 == 0.0 {
            return Ok(0.0);
        }
        
        Ok(covariance / (var1.sqrt() * var2.sqrt()))
    }
    
    fn calculate_rolling_correlation(
        &self,
        data1: &[TimeSeriesData],
        data2: &[TimeSeriesData],
        window: usize,
    ) -> Result<f64> {
        let returns1 = self.calculate_returns(data1, window)?;
        let returns2 = self.calculate_returns(data2, window)?;
        
        if returns1.len() < window || returns2.len() < window {
            return Err(anyhow::anyhow!("Insufficient data for rolling correlation"));
        }
        
        // Get the most recent window
        let recent_returns1 = &returns1[returns1.len() - window..];
        let recent_returns2 = &returns2[returns2.len() - window..];
        
        self.calculate_correlation_from_returns(recent_returns1, recent_returns2)
    }
    
    fn calculate_lead_lag_correlation(
        &self,
        data1: &[TimeSeriesData],
        data2: &[TimeSeriesData],
        max_lag: usize,
    ) -> Result<f64> {
        let returns1 = self.calculate_returns(data1, 60)?;
        let returns2 = self.calculate_returns(data2, 60)?;
        
        let mut max_correlation = 0.0;
        let mut best_lag = 0i32;
        
        for lag in -(max_lag as i32)..=max_lag as i32 {
            let correlation = if lag > 0 {
                // data2 leads data1
                let start1 = lag as usize;
                let end2 = returns2.len() - lag as usize;
                
                if start1 < returns1.len() && end2 <= returns2.len() {
                    self.calculate_correlation_from_returns(
                        &returns1[start1..],
                        &returns2[..end2],
                    ).unwrap_or(0.0)
                } else {
                    0.0
                }
            } else if lag < 0 {
                // data1 leads data2
                let start2 = (-lag) as usize;
                let end1 = returns1.len() - (-lag) as usize;
                
                if start2 < returns2.len() && end1 <= returns1.len() {
                    self.calculate_correlation_from_returns(
                        &returns1[..end1],
                        &returns2[start2..],
                    ).unwrap_or(0.0)
                } else {
                    0.0
                }
            } else {
                // No lag
                self.calculate_correlation_from_returns(&returns1, &returns2).unwrap_or(0.0)
            };
            
            if correlation.abs() > max_correlation.abs() {
                max_correlation = correlation;
                best_lag = lag;
            }
        }
        
        Ok(best_lag as f64)
    }
    
    fn calculate_time_varying_correlation(
        &self,
        data1: &[TimeSeriesData],
        data2: &[TimeSeriesData],
        window: usize,
    ) -> Result<Vec<f64>> {
        let returns1 = self.calculate_returns(data1, data1.len())?;
        let returns2 = self.calculate_returns(data2, data2.len())?;
        
        let min_len = returns1.len().min(returns2.len());
        if min_len < window {
            return Err(anyhow::anyhow!("Insufficient data for time-varying correlation"));
        }
        
        let mut correlations = Vec::new();
        
        for i in window..=min_len {
            let window_returns1 = &returns1[i - window..i];
            let window_returns2 = &returns2[i - window..i];
            
            if let Ok(corr) = self.calculate_correlation_from_returns(window_returns1, window_returns2) {
                correlations.push(corr);
            }
        }
        
        Ok(correlations)
    }
    
    fn calculate_beta(
        &self,
        asset_data: &[TimeSeriesData],
        market_data: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
        let asset_returns = self.calculate_returns(asset_data, period)?;
        let market_returns = self.calculate_returns(market_data, period)?;
        
        if asset_returns.len() != market_returns.len() {
            return Err(anyhow::anyhow!("Mismatched data lengths for beta calculation"));
        }
        
        let n = asset_returns.len() as f64;
        let mean_asset = asset_returns.iter().sum::<f64>() / n;
        let mean_market = market_returns.iter().sum::<f64>() / n;
        
        let mut covariance = 0.0;
        let mut market_variance = 0.0;
        
        for i in 0..asset_returns.len() {
            let asset_dev = asset_returns[i] - mean_asset;
            let market_dev = market_returns[i] - mean_market;
            
            covariance += asset_dev * market_dev;
            market_variance += market_dev * market_dev;
        }
        
        if market_variance == 0.0 {
            return Ok(1.0); // Default beta
        }
        
        Ok(covariance / market_variance)
    }
    
    fn calculate_rolling_beta(
        &self,
        asset_data: &[TimeSeriesData],
        market_data: &[TimeSeriesData],
        lookback: usize,
        window: usize,
    ) -> Result<f64> {
        if asset_data.len() < lookback || market_data.len() < lookback {
            return Err(anyhow::anyhow!("Insufficient data for rolling beta"));
        }
        
        let recent_asset = &asset_data[asset_data.len() - lookback..];
        let recent_market = &market_data[market_data.len() - lookback..];
        
        self.calculate_beta(recent_asset, recent_market, window)
    }
    
    fn calculate_returns(&self, data: &[TimeSeriesData], max_len: usize) -> Result<Vec<f64>> {
        if data.len() < 2 {
            return Ok(Vec::new());
        }
        
        let start_idx = if data.len() > max_len + 1 {
            data.len() - max_len - 1
        } else {
            0
        };
        
        let mut returns = Vec::new();
        
        for i in (start_idx + 1)..data.len() {
            let log_return = (data[i].close / data[i - 1].close).ln();
            returns.push(log_return);
        }
        
        Ok(returns)
    }
    
    fn calculate_correlation_from_returns(&self, returns1: &[f64], returns2: &[f64]) -> Result<f64> {
        if returns1.len() != returns2.len() || returns1.is_empty() {
            return Err(anyhow::anyhow!("Invalid returns data for correlation"));
        }
        
        let n = returns1.len() as f64;
        let mean1 = returns1.iter().sum::<f64>() / n;
        let mean2 = returns2.iter().sum::<f64>() / n;
        
        let mut covariance = 0.0;
        let mut var1 = 0.0;
        let mut var2 = 0.0;
        
        for i in 0..returns1.len() {
            let dev1 = returns1[i] - mean1;
            let dev2 = returns2[i] - mean2;
            
            covariance += dev1 * dev2;
            var1 += dev1 * dev1;
            var2 += dev2 * dev2;
        }
        
        if var1 == 0.0 || var2 == 0.0 {
            return Ok(0.0);
        }
        
        Ok(covariance / (var1.sqrt() * var2.sqrt()))
    }
}

impl Default for CrossAssetCorrelationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "cross_asset_tests.rs"]
mod cross_asset_tests;