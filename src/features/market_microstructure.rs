//! Market Microstructure Analysis
//! 
//! Advanced features for analyzing market microstructure including
//! order flow, bid-ask dynamics, and liquidity patterns.

use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc, Duration};
use crate::data::TimeSeriesData;

/// Market microstructure analyzer
pub struct MicrostructureAnalyzer {
    /// Order book snapshots buffer
    order_book_buffer: VecDeque<OrderBookSnapshot>,
    
    /// Trade flow buffer
    trade_flow_buffer: VecDeque<TradeEvent>,
    
    /// Configuration
    config: MicrostructureConfig,
}

/// Order book snapshot
#[derive(Debug, Clone)]
pub struct OrderBookSnapshot {
    pub timestamp: DateTime<Utc>,
    pub bids: Vec<(f64, f64)>, // (price, size)
    pub asks: Vec<(f64, f64)>, // (price, size)
}

/// Trade event
#[derive(Debug, Clone)]
pub struct TradeEvent {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub size: f64,
    pub side: TradeSide,
    pub aggressive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Configuration for microstructure analysis
#[derive(Debug, Clone)]
pub struct MicrostructureConfig {
    /// Maximum order book depth to analyze
    pub max_book_depth: usize,
    
    /// Time window for flow analysis (seconds)
    pub flow_window_seconds: u64,
    
    /// Enable tick-level analysis
    pub enable_tick_analysis: bool,
    
    /// Enable order flow imbalance
    pub enable_flow_imbalance: bool,
    
    /// Enable liquidity analysis
    pub enable_liquidity_analysis: bool,
}

impl Default for MicrostructureConfig {
    fn default() -> Self {
        Self {
            max_book_depth: 10,
            flow_window_seconds: 300, // 5 minutes
            enable_tick_analysis: true,
            enable_flow_imbalance: true,
            enable_liquidity_analysis: true,
        }
    }
}

impl MicrostructureAnalyzer {
    /// Create a new microstructure analyzer
    pub fn new() -> Self {
        Self {
            order_book_buffer: VecDeque::with_capacity(1000),
            trade_flow_buffer: VecDeque::with_capacity(10000),
            config: MicrostructureConfig::default(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(config: MicrostructureConfig) -> Self {
        Self {
            order_book_buffer: VecDeque::with_capacity(1000),
            trade_flow_buffer: VecDeque::with_capacity(10000),
            config,
        }
    }
    
    /// Analyze market microstructure
    pub async fn analyze(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Spread analysis
        self.analyze_spread(current, &mut features)?;
        
        // Order flow imbalance
        if self.config.enable_flow_imbalance {
            self.analyze_order_flow_imbalance(current, historical, &mut features)?;
        }
        
        // Tick-level analysis
        if self.config.enable_tick_analysis {
            self.analyze_tick_patterns(current, historical, &mut features)?;
        }
        
        // Liquidity analysis
        if self.config.enable_liquidity_analysis {
            self.analyze_liquidity(current, historical, &mut features)?;
        }
        
        // Price impact analysis
        self.analyze_price_impact(current, historical, &mut features)?;
        
        // Trade intensity
        self.analyze_trade_intensity(current, historical, &mut features)?;
        
        // Microstructure noise
        self.analyze_microstructure_noise(historical, &mut features)?;
        
        Ok(features)
    }
    
    /// Analyze bid-ask spread dynamics
    fn analyze_spread(
        &self,
        current: &TimeSeriesData,
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Current spread (using high-low as proxy)
        let spread = current.high - current.low;
        let spread_percentage = (spread / current.close) * 100.0;
        
        features.insert("spread".to_string(), spread);
        features.insert("spread_percentage".to_string(), spread_percentage);
        
        // Relative spread
        let mid_price = (current.high + current.low) / 2.0;
        let relative_spread = spread / mid_price;
        features.insert("relative_spread".to_string(), relative_spread);
        
        // Spread volatility indicator
        features.insert("spread_volatility".to_string(), 
            if spread_percentage > 0.5 { 1.0 } else { 0.0 }
        );
        
        Ok(())
    }
    
    /// Analyze order flow imbalance
    fn analyze_order_flow_imbalance(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Volume-weighted price movement
        let mut buy_volume = 0.0;
        let mut sell_volume = 0.0;
        
        for (i, data) in historical.iter().enumerate() {
            if i == 0 { continue; }
            
            if data.close > historical[i - 1].close {
                buy_volume += data.volume;
            } else if data.close < historical[i - 1].close {
                sell_volume += data.volume;
            }
        }
        
        // Current volume attribution
        if let Some(prev) = historical.last() {
            if current.close > prev.close {
                buy_volume += current.volume;
            } else if current.close < prev.close {
                sell_volume += current.volume;
            }
        }
        
        let total_volume = buy_volume + sell_volume;
        if total_volume > 0.0 {
            let flow_imbalance = (buy_volume - sell_volume) / total_volume;
            features.insert("order_flow_imbalance".to_string(), flow_imbalance);
            features.insert("buy_volume_ratio".to_string(), buy_volume / total_volume);
            features.insert("sell_volume_ratio".to_string(), sell_volume / total_volume);
        }
        
        // Volume-synchronized probability of informed trading (VPIN)
        let vpin = self.calculate_vpin(historical)?;
        features.insert("vpin".to_string(), vpin);
        
        // Kyle's lambda (price impact coefficient)
        let kyle_lambda = self.calculate_kyle_lambda(historical)?;
        features.insert("kyle_lambda".to_string(), kyle_lambda);
        
        // Order Flow Toxicity Metrics
        let toxicity_metrics = self.calculate_order_flow_toxicity(current, historical)?;
        features.extend(toxicity_metrics);
        
        Ok(())
    }
    
    /// Analyze tick-level patterns
    fn analyze_tick_patterns(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Tick direction indicators
        let mut upticks = 0;
        let mut downticks = 0;
        let mut zero_ticks = 0;
        
        for i in 1..historical.len() {
            if historical[i].close > historical[i - 1].close {
                upticks += 1;
            } else if historical[i].close < historical[i - 1].close {
                downticks += 1;
            } else {
                zero_ticks += 1;
            }
        }
        
        let total_ticks = upticks + downticks + zero_ticks;
        if total_ticks > 0 {
            features.insert("uptick_ratio".to_string(), upticks as f64 / total_ticks as f64);
            features.insert("downtick_ratio".to_string(), downticks as f64 / total_ticks as f64);
            features.insert("zero_tick_ratio".to_string(), zero_ticks as f64 / total_ticks as f64);
        }
        
        // Tick rule indicator
        if let Some(prev) = historical.last() {
            let tick_rule = if current.close > prev.close { 1.0 }
                          else if current.close < prev.close { -1.0 }
                          else { 0.0 };
            features.insert("tick_rule".to_string(), tick_rule);
        }
        
        // Run analysis (consecutive moves in same direction)
        let runs = self.analyze_price_runs(historical)?;
        features.insert("positive_runs".to_string(), runs.0 as f64);
        features.insert("negative_runs".to_string(), runs.1 as f64);
        features.insert("run_ratio".to_string(), 
            if runs.1 > 0 { runs.0 as f64 / runs.1 as f64 } else { runs.0 as f64 }
        );
        
        Ok(())
    }
    
    /// Analyze market liquidity
    fn analyze_liquidity(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Amihud illiquidity measure
        let amihud = self.calculate_amihud_illiquidity(historical)?;
        features.insert("amihud_illiquidity".to_string(), amihud);
        
        // Roll's implicit spread estimator
        let roll_spread = self.calculate_roll_spread(historical)?;
        features.insert("roll_spread".to_string(), roll_spread);
        
        // Volume-weighted average trade size
        let avg_trade_size = if historical.is_empty() {
            current.volume
        } else {
            let total_volume: f64 = historical.iter().map(|d| d.volume).sum();
            total_volume / historical.len() as f64
        };
        features.insert("avg_trade_size".to_string(), avg_trade_size);
        
        // Liquidity ratio (volume to price change)
        if let Some(prev) = historical.last() {
            let price_change = (current.close - prev.close).abs();
            if price_change > 0.0 {
                let liquidity_ratio = current.volume / price_change;
                features.insert("liquidity_ratio".to_string(), liquidity_ratio);
            }
        }
        
        // Market depth proxy (using volume and high-low range)
        let depth_proxy = current.volume / (current.high - current.low + 0.0001);
        features.insert("market_depth_proxy".to_string(), depth_proxy);
        
        Ok(())
    }
    
    /// Analyze price impact
    fn analyze_price_impact(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Temporary price impact
        if historical.len() >= 5 {
            let pre_trade_price = historical[historical.len() - 5].close;
            let immediate_impact = (current.close - pre_trade_price) / pre_trade_price;
            features.insert("temporary_price_impact".to_string(), immediate_impact);
            
            // Permanent price impact (simplified)
            if historical.len() >= 10 {
                let long_term_price = historical[historical.len() - 10].close;
                let permanent_impact = (current.close - long_term_price) / long_term_price;
                features.insert("permanent_price_impact".to_string(), permanent_impact);
            }
        }
        
        // Volume-normalized price impact
        if current.volume > 0.0 && historical.len() > 0 {
            let price_return = (current.close - historical.last().unwrap().close) 
                / historical.last().unwrap().close;
            let normalized_impact = price_return / current.volume.ln();
            features.insert("normalized_price_impact".to_string(), normalized_impact);
        }
        
        Ok(())
    }
    
    /// Analyze trade intensity
    fn analyze_trade_intensity(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Trade arrival rate (trades per period)
        let periods = historical.len().max(1);
        let avg_volume_per_period = historical.iter()
            .map(|d| d.volume)
            .sum::<f64>() / periods as f64;
        
        // Current vs average intensity
        let intensity_ratio = current.volume / (avg_volume_per_period + 0.0001);
        features.insert("trade_intensity_ratio".to_string(), intensity_ratio);
        
        // Volume concentration
        if historical.len() >= 20 {
            let recent_volume: f64 = historical.iter()
                .rev()
                .take(5)
                .map(|d| d.volume)
                .sum();
            
            let total_volume: f64 = historical.iter()
                .rev()
                .take(20)
                .map(|d| d.volume)
                .sum();
            
            if total_volume > 0.0 {
                let volume_concentration = recent_volume / total_volume;
                features.insert("volume_concentration".to_string(), volume_concentration);
            }
        }
        
        // Trade size distribution
        let volume_variance = self.calculate_volume_variance(historical)?;
        features.insert("volume_variance".to_string(), volume_variance);
        
        Ok(())
    }
    
    /// Analyze microstructure noise
    fn analyze_microstructure_noise(
        &self,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        if historical.len() < 20 {
            return Ok(());
        }
        
        // Realized variance at different frequencies
        let rv_1min = self.calculate_realized_variance(historical, 1)?;
        let rv_5min = self.calculate_realized_variance(historical, 5)?;
        
        // Variance ratio test for microstructure noise
        if rv_1min > 0.0 {
            let variance_ratio = rv_5min / (5.0 * rv_1min);
            features.insert("variance_ratio".to_string(), variance_ratio);
            
            // Noise indicator (deviation from 1)
            let noise_indicator = (variance_ratio - 1.0).abs();
            features.insert("microstructure_noise".to_string(), noise_indicator);
        }
        
        // First-order autocorrelation (negative indicates bid-ask bounce)
        let autocorr = self.calculate_return_autocorrelation(historical, 1)?;
        features.insert("return_autocorrelation".to_string(), autocorr);
        
        Ok(())
    }
    
    // Helper calculation methods
    
    fn calculate_vpin(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 50 {
            return Ok(0.5); // Neutral VPIN
        }
        
        // Simplified VPIN calculation
        let mut buy_volumes = Vec::new();
        let mut sell_volumes = Vec::new();
        
        for i in 1..data.len() {
            let price_change = data[i].close - data[i - 1].close;
            if price_change > 0.0 {
                buy_volumes.push(data[i].volume);
                sell_volumes.push(0.0);
            } else if price_change < 0.0 {
                buy_volumes.push(0.0);
                sell_volumes.push(data[i].volume);
            } else {
                // Split volume equally for unchanged price
                buy_volumes.push(data[i].volume / 2.0);
                sell_volumes.push(data[i].volume / 2.0);
            }
        }
        
        // Calculate VPIN over buckets
        let bucket_size = 10;
        let mut vpin_values = Vec::new();
        
        for i in (bucket_size..buy_volumes.len()).step_by(bucket_size) {
            let buy_sum: f64 = buy_volumes[i - bucket_size..i].iter().sum();
            let sell_sum: f64 = sell_volumes[i - bucket_size..i].iter().sum();
            let total = buy_sum + sell_sum;
            
            if total > 0.0 {
                vpin_values.push((buy_sum - sell_sum).abs() / total);
            }
        }
        
        if vpin_values.is_empty() {
            return Ok(0.5);
        }
        
        Ok(vpin_values.iter().sum::<f64>() / vpin_values.len() as f64)
    }
    
    fn calculate_kyle_lambda(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 10 {
            return Ok(0.0);
        }
        
        // Kyle's lambda: price impact per unit of net order flow
        let mut price_changes = Vec::new();
        let mut net_volumes = Vec::new();
        
        for i in 1..data.len() {
            let price_change = (data[i].close - data[i - 1].close).abs();
            price_changes.push(price_change);
            
            // Approximate net order flow
            let net_volume = if data[i].close > data[i - 1].close {
                data[i].volume
            } else {
                -data[i].volume
            };
            net_volumes.push(net_volume.abs());
        }
        
        // Simple regression of price change on net volume
        let n = price_changes.len() as f64;
        let x_mean = net_volumes.iter().sum::<f64>() / n;
        let y_mean = price_changes.iter().sum::<f64>() / n;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for i in 0..price_changes.len() {
            numerator += (net_volumes[i] - x_mean) * (price_changes[i] - y_mean);
            denominator += (net_volumes[i] - x_mean).powi(2);
        }
        
        if denominator == 0.0 {
            return Ok(0.0);
        }
        
        Ok(numerator / denominator)
    }
    
    fn analyze_price_runs(&self, data: &[TimeSeriesData]) -> Result<(usize, usize)> {
        let mut positive_runs = 0;
        let mut negative_runs = 0;
        let mut current_run_positive = false;
        let mut in_run = false;
        
        for i in 1..data.len() {
            let price_change = data[i].close - data[i - 1].close;
            
            if price_change > 0.0 {
                if !in_run || !current_run_positive {
                    positive_runs += 1;
                    current_run_positive = true;
                    in_run = true;
                }
            } else if price_change < 0.0 {
                if !in_run || current_run_positive {
                    negative_runs += 1;
                    current_run_positive = false;
                    in_run = true;
                }
            } else {
                in_run = false;
            }
        }
        
        Ok((positive_runs, negative_runs))
    }
    
    fn calculate_amihud_illiquidity(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.is_empty() {
            return Ok(0.0);
        }
        
        let mut illiquidity_values = Vec::new();
        
        for i in 1..data.len() {
            let price_return = ((data[i].close - data[i - 1].close) / data[i - 1].close).abs();
            if data[i].volume > 0.0 {
                illiquidity_values.push(price_return / data[i].volume);
            }
        }
        
        if illiquidity_values.is_empty() {
            return Ok(0.0);
        }
        
        Ok(illiquidity_values.iter().sum::<f64>() / illiquidity_values.len() as f64 * 1e6)
    }
    
    fn calculate_roll_spread(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 2 {
            return Ok(0.0);
        }
        
        let mut price_changes = Vec::new();
        
        for i in 1..data.len() {
            price_changes.push(data[i].close - data[i - 1].close);
        }
        
        // Calculate autocovariance
        let mean_change = price_changes.iter().sum::<f64>() / price_changes.len() as f64;
        let mut autocovariance = 0.0;
        
        for i in 1..price_changes.len() {
            autocovariance += (price_changes[i] - mean_change) * (price_changes[i - 1] - mean_change);
        }
        
        autocovariance /= (price_changes.len() - 1) as f64;
        
        // Roll's spread estimator
        if autocovariance < 0.0 {
            Ok(2.0 * (-autocovariance).sqrt())
        } else {
            Ok(0.0) // No bid-ask bounce detected
        }
    }
    
    fn calculate_volume_variance(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.is_empty() {
            return Ok(0.0);
        }
        
        let volumes: Vec<f64> = data.iter().map(|d| d.volume).collect();
        let mean_volume = volumes.iter().sum::<f64>() / volumes.len() as f64;
        
        let variance = volumes.iter()
            .map(|&v| (v - mean_volume).powi(2))
            .sum::<f64>() / volumes.len() as f64;
        
        Ok(variance.sqrt() / mean_volume) // Coefficient of variation
    }
    
    fn calculate_realized_variance(&self, data: &[TimeSeriesData], interval: usize) -> Result<f64> {
        if data.len() < interval * 2 {
            return Ok(0.0);
        }
        
        let mut returns = Vec::new();
        
        for i in (interval..data.len()).step_by(interval) {
            let log_return = (data[i].close / data[i - interval].close).ln();
            returns.push(log_return);
        }
        
        if returns.is_empty() {
            return Ok(0.0);
        }
        
        let variance = returns.iter()
            .map(|&r| r.powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        Ok(variance * 252.0) // Annualized
    }
    
    fn calculate_return_autocorrelation(&self, data: &[TimeSeriesData], lag: usize) -> Result<f64> {
        if data.len() < lag + 2 {
            return Ok(0.0);
        }
        
        let mut returns = Vec::new();
        
        for i in 1..data.len() {
            returns.push((data[i].close / data[i - 1].close).ln());
        }
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for i in lag..returns.len() {
            numerator += (returns[i] - mean_return) * (returns[i - lag] - mean_return);
        }
        
        for r in &returns {
            denominator += (r - mean_return).powi(2);
        }
        
        if denominator == 0.0 {
            return Ok(0.0);
        }
        
        Ok(numerator / denominator)
    }
    
    /// Calculate comprehensive order flow toxicity metrics
    fn calculate_order_flow_toxicity(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        
        if historical.len() < 20 {
            return Ok(metrics);
        }
        
        // 1. Adverse Selection Component (ASC)
        // Measures the probability that trades are initiated by informed traders
        let mut informed_trades = 0.0;
        let mut total_trades = 0.0;
        
        for i in 1..historical.len() {
            let price_change = (historical[i].close - historical[i - 1].close).abs();
            let avg_price = (historical[i].close + historical[i - 1].close) / 2.0;
            let normalized_change = price_change / avg_price;
            
            // Large price moves with high volume indicate informed trading
            if normalized_change > 0.001 && historical[i].volume > historical[i - 1].volume * 1.5 {
                informed_trades += 1.0;
            }
            total_trades += 1.0;
        }
        
        let adverse_selection = if total_trades > 0.0 { 
            informed_trades / total_trades 
        } else { 
            0.0 
        };
        metrics.insert("adverse_selection_component".to_string(), adverse_selection);
        
        // 2. Realized Spread Toxicity
        // Measures post-trade price movement indicating toxic flow
        let mut toxic_spreads = Vec::new();
        let look_ahead = 5; // 5 periods ahead
        
        for i in 0..(historical.len() - look_ahead) {
            let trade_price = historical[i].close;
            let future_price = historical[i + look_ahead].close;
            let mid_price = (historical[i].high + historical[i].low) / 2.0;
            
            // Realized spread = 2 * trade_direction * (future_price - trade_price)
            let trade_direction = if historical[i].close > mid_price { 1.0 } else { -1.0 };
            let realized_spread = 2.0 * trade_direction * (future_price - trade_price) / trade_price;
            
            toxic_spreads.push(realized_spread);
        }
        
        if !toxic_spreads.is_empty() {
            let avg_toxicity = toxic_spreads.iter().sum::<f64>() / toxic_spreads.len() as f64;
            let toxicity_volatility = {
                let mean = avg_toxicity;
                let variance = toxic_spreads.iter()
                    .map(|&x| (x - mean).powi(2))
                    .sum::<f64>() / toxic_spreads.len() as f64;
                variance.sqrt()
            };
            
            metrics.insert("realized_spread_toxicity".to_string(), avg_toxicity);
            metrics.insert("toxicity_volatility".to_string(), toxicity_volatility);
        }
        
        // 3. Flow Toxicity Index (FTI)
        // Composite measure of order flow toxicity
        let vpin = self.calculate_vpin(historical).unwrap_or(0.5);
        let kyle_lambda = self.calculate_kyle_lambda(historical).unwrap_or(0.0);
        
        // Normalize components
        let normalized_vpin = (vpin - 0.5) * 2.0; // Center around 0
        let normalized_lambda = kyle_lambda.min(1.0); // Cap at 1
        let normalized_adverse = adverse_selection;
        
        // Weighted combination
        let flow_toxicity_index = (normalized_vpin * 0.4 + 
                                  normalized_lambda * 0.3 + 
                                  normalized_adverse * 0.3).max(0.0).min(1.0);
        
        metrics.insert("flow_toxicity_index".to_string(), flow_toxicity_index);
        
        // 4. Predatory Trading Indicator
        // Detects patterns of predatory HFT behavior
        let mut momentum_trades = 0;
        let mut reversal_trades = 0;
        
        for i in 2..historical.len() {
            let prev_move = historical[i - 1].close - historical[i - 2].close;
            let curr_move = historical[i].close - historical[i - 1].close;
            
            if prev_move * curr_move > 0.0 {
                momentum_trades += 1;
            } else {
                reversal_trades += 1;
            }
        }
        
        let total_pattern_trades = momentum_trades + reversal_trades;
        if total_pattern_trades > 0 {
            let predatory_ratio = reversal_trades as f64 / total_pattern_trades as f64;
            metrics.insert("predatory_trading_indicator".to_string(), predatory_ratio);
        }
        
        // 5. Quote Stuffing Detection
        // High message-to-trade ratio indicates potential manipulation
        let mut high_activity_periods = 0;
        let mut total_periods = 0;
        
        for i in 1..historical.len() {
            let volume_ratio = historical[i].volume / historical[i - 1].volume.max(1.0);
            let price_movement = (historical[i].close - historical[i - 1].close).abs() / historical[i - 1].close;
            
            // High volume with minimal price movement suggests quote stuffing
            if volume_ratio > 3.0 && price_movement < 0.0001 {
                high_activity_periods += 1;
            }
            total_periods += 1;
        }
        
        let quote_stuffing_indicator = if total_periods > 0 {
            high_activity_periods as f64 / total_periods as f64
        } else {
            0.0
        };
        metrics.insert("quote_stuffing_indicator".to_string(), quote_stuffing_indicator);
        
        // 6. Spoofing Detection Score
        // Detects patterns of order placement and immediate cancellation
        let mut spoof_patterns = 0;
        
        for i in 2..historical.len() {
            let vol1 = historical[i - 2].volume;
            let vol2 = historical[i - 1].volume;
            let vol3 = historical[i].volume;
            
            let price1 = historical[i - 2].close;
            let price2 = historical[i - 1].close;
            let price3 = historical[i].close;
            
            // Pattern: High volume spike with immediate reversal and volume drop
            if vol2 > vol1 * 2.0 && vol3 < vol2 * 0.5 && 
               (price2 - price1).signum() != (price3 - price2).signum() {
                spoof_patterns += 1;
            }
        }
        
        let spoofing_score = if historical.len() > 2 {
            spoof_patterns as f64 / (historical.len() - 2) as f64
        } else {
            0.0
        };
        metrics.insert("spoofing_detection_score".to_string(), spoofing_score);
        
        // 7. Toxicity Level Classification
        let overall_toxicity = flow_toxicity_index;
        let toxicity_level = if overall_toxicity < 0.3 {
            0.0 // Low toxicity
        } else if overall_toxicity < 0.6 {
            1.0 // Medium toxicity
        } else {
            2.0 // High toxicity
        };
        metrics.insert("toxicity_level".to_string(), toxicity_level);
        
        Ok(metrics)
    }
}

#[cfg(test)]
#[path = "market_microstructure_tests.rs"]
mod market_microstructure_tests;

impl Default for MicrostructureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}