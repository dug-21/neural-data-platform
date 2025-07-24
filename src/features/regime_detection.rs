//! Market Regime Detection
//! 
//! Identifies current market regime using various statistical
//! and machine learning techniques.

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;

/// Market regime types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketRegime {
    /// Strong upward trend
    BullTrend = 0,
    
    /// Weak upward or sideways movement
    BullConsolidation = 1,
    
    /// High volatility with no clear direction
    HighVolatility = 2,
    
    /// Weak downward or sideways movement
    BearConsolidation = 3,
    
    /// Strong downward trend
    BearTrend = 4,
    
    /// Low volatility, range-bound
    LowVolatility = 5,
}

/// Regime detector
pub struct RegimeDetector {
    /// Configuration
    config: RegimeConfig,
    
    /// Historical regime probabilities
    regime_history: Vec<RegimeProbabilities>,
}

/// Regime detection configuration
#[derive(Debug, Clone)]
pub struct RegimeConfig {
    /// Lookback period for regime detection
    pub lookback_period: usize,
    
    /// Volatility threshold for high vol regime
    pub high_vol_threshold: f64,
    
    /// Volatility threshold for low vol regime
    pub low_vol_threshold: f64,
    
    /// Trend strength threshold
    pub trend_threshold: f64,
    
    /// Enable Markov regime switching
    pub enable_markov_switching: bool,
    
    /// Enable hidden Markov model
    pub enable_hmm: bool,
}

impl Default for RegimeConfig {
    fn default() -> Self {
        Self {
            lookback_period: 60,
            high_vol_threshold: 0.02, // 2% daily vol
            low_vol_threshold: 0.005, // 0.5% daily vol
            trend_threshold: 0.15, // 15% move for strong trend
            enable_markov_switching: true,
            enable_hmm: true,
        }
    }
}

/// Regime probabilities
#[derive(Debug, Clone)]
pub struct RegimeProbabilities {
    pub bull_trend: f64,
    pub bull_consolidation: f64,
    pub high_volatility: f64,
    pub bear_consolidation: f64,
    pub bear_trend: f64,
    pub low_volatility: f64,
}

impl RegimeProbabilities {
    /// Get the most probable regime
    pub fn get_regime(&self) -> MarketRegime {
        let mut max_prob = 0.0;
        let mut regime = MarketRegime::LowVolatility;
        
        let probs = vec![
            (self.bull_trend, MarketRegime::BullTrend),
            (self.bull_consolidation, MarketRegime::BullConsolidation),
            (self.high_volatility, MarketRegime::HighVolatility),
            (self.bear_consolidation, MarketRegime::BearConsolidation),
            (self.bear_trend, MarketRegime::BearTrend),
            (self.low_volatility, MarketRegime::LowVolatility),
        ];
        
        for (prob, r) in probs {
            if prob > max_prob {
                max_prob = prob;
                regime = r;
            }
        }
        
        regime
    }
}

impl RegimeDetector {
    /// Create a new regime detector
    pub fn new() -> Self {
        Self {
            config: RegimeConfig::default(),
            regime_history: Vec::new(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(config: RegimeConfig) -> Self {
        Self {
            config,
            regime_history: Vec::new(),
        }
    }
    
    /// Detect current market regime
    pub async fn detect_regime(&self, data: &[TimeSeriesData]) -> Result<MarketRegime> {
        let probabilities = self.calculate_regime_probabilities(data)?;
        Ok(probabilities.get_regime())
    }
    
    /// Get regime probabilities
    pub async fn get_regime_probabilities(&self, data: &[TimeSeriesData]) -> Result<RegimeProbabilities> {
        self.calculate_regime_probabilities(data)
    }
    
    /// Calculate regime probabilities
    fn calculate_regime_probabilities(&self, data: &[TimeSeriesData]) -> Result<RegimeProbabilities> {
        if data.len() < self.config.lookback_period {
            return Err(anyhow::anyhow!("Insufficient data for regime detection"));
        }
        
        // Calculate base statistics
        let stats = self.calculate_market_statistics(data)?;
        
        // Rule-based regime probabilities
        let mut probs = self.rule_based_regime_detection(&stats)?;
        
        // Markov regime switching
        if self.config.enable_markov_switching {
            let markov_probs = self.markov_regime_switching(data, &stats)?;
            probs = self.combine_probabilities(probs, markov_probs, 0.5);
        }
        
        // Hidden Markov Model
        if self.config.enable_hmm {
            let hmm_probs = self.hidden_markov_regime(data, &stats)?;
            probs = self.combine_probabilities(probs, hmm_probs, 0.3);
        }
        
        // Normalize probabilities
        self.normalize_probabilities(&mut probs);
        
        Ok(probs)
    }
    
    /// Calculate market statistics
    fn calculate_market_statistics(&self, data: &[TimeSeriesData]) -> Result<MarketStatistics> {
        let recent_data: Vec<&TimeSeriesData> = data.iter()
            .rev()
            .take(self.config.lookback_period)
            .collect();
        
        // Calculate returns
        let mut returns = Vec::new();
        for i in 1..recent_data.len() {
            let ret = (recent_data[i - 1].close / recent_data[i].close) - 1.0;
            returns.push(ret);
        }
        
        // Basic statistics
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let volatility = self.calculate_volatility(&returns);
        
        // Trend statistics
        let start_price = recent_data.last().unwrap().close;
        let end_price = recent_data.first().unwrap().close;
        let total_return = (end_price - start_price) / start_price;
        
        // Directional movement
        let mut up_days = 0;
        let mut down_days = 0;
        for ret in &returns {
            if *ret > 0.0 {
                up_days += 1;
            } else if *ret < 0.0 {
                down_days += 1;
            }
        }
        
        // Maximum drawdown
        let max_drawdown = self.calculate_max_drawdown(&recent_data);
        
        // Volatility of volatility
        let vol_of_vol = self.calculate_volatility_of_volatility(&recent_data)?;
        
        Ok(MarketStatistics {
            mean_return,
            volatility,
            total_return,
            up_days,
            down_days,
            max_drawdown,
            vol_of_vol,
            skewness: self.calculate_skewness(&returns),
            kurtosis: self.calculate_kurtosis(&returns),
        })
    }
    
    /// Rule-based regime detection
    fn rule_based_regime_detection(&self, stats: &MarketStatistics) -> Result<RegimeProbabilities> {
        let mut probs = RegimeProbabilities {
            bull_trend: 0.0,
            bull_consolidation: 0.0,
            high_volatility: 0.0,
            bear_consolidation: 0.0,
            bear_trend: 0.0,
            low_volatility: 0.0,
        };
        
        // Volatility-based classification
        if stats.volatility > self.config.high_vol_threshold {
            probs.high_volatility = 0.5;
        } else if stats.volatility < self.config.low_vol_threshold {
            probs.low_volatility = 0.5;
        }
        
        // Trend-based classification
        if stats.total_return > self.config.trend_threshold {
            probs.bull_trend = 0.6;
            probs.bull_consolidation = 0.2;
        } else if stats.total_return < -self.config.trend_threshold {
            probs.bear_trend = 0.6;
            probs.bear_consolidation = 0.2;
        } else if stats.total_return > 0.0 {
            probs.bull_consolidation = 0.5;
            probs.bull_trend = 0.2;
        } else {
            probs.bear_consolidation = 0.5;
            probs.bear_trend = 0.2;
        }
        
        // Adjust based on up/down day ratio
        let total_days = (stats.up_days + stats.down_days) as f64;
        if total_days > 0.0 {
            let up_ratio = stats.up_days as f64 / total_days;
            
            if up_ratio > 0.65 {
                probs.bull_trend *= 1.2;
                probs.bear_trend *= 0.8;
            } else if up_ratio < 0.35 {
                probs.bear_trend *= 1.2;
                probs.bull_trend *= 0.8;
            }
        }
        
        // Adjust based on drawdown
        if stats.max_drawdown > 0.1 {
            probs.bear_trend *= 1.3;
            probs.high_volatility *= 1.2;
        }
        
        // Volatility of volatility adjustment
        if stats.vol_of_vol > 0.5 {
            probs.high_volatility *= 1.3;
            probs.low_volatility *= 0.5;
        }
        
        Ok(probs)
    }
    
    /// Markov regime switching model
    fn markov_regime_switching(
        &self,
        data: &[TimeSeriesData],
        stats: &MarketStatistics,
    ) -> Result<RegimeProbabilities> {
        // Simplified 2-state Markov switching (Bull/Bear)
        let threshold = 0.0;
        
        // Count regime transitions
        let mut bull_to_bull = 0;
        let mut bull_to_bear = 0;
        let mut bear_to_bull = 0;
        let mut bear_to_bear = 0;
        
        for i in 2..data.len() {
            let prev_return = (data[i - 1].close - data[i - 2].close) / data[i - 2].close;
            let curr_return = (data[i].close - data[i - 1].close) / data[i - 1].close;
            
            if prev_return > threshold && curr_return > threshold {
                bull_to_bull += 1;
            } else if prev_return > threshold && curr_return <= threshold {
                bull_to_bear += 1;
            } else if prev_return <= threshold && curr_return > threshold {
                bear_to_bull += 1;
            } else {
                bear_to_bear += 1;
            }
        }
        
        // Calculate transition probabilities
        let total_bull = (bull_to_bull + bull_to_bear) as f64;
        let total_bear = (bear_to_bull + bear_to_bear) as f64;
        
        let p_bull_to_bull = if total_bull > 0.0 { bull_to_bull as f64 / total_bull } else { 0.5 };
        let p_bear_to_bear = if total_bear > 0.0 { bear_to_bear as f64 / total_bear } else { 0.5 };
        
        // Current state probability
        let last_return = if data.len() >= 2 {
            (data[data.len() - 1].close - data[data.len() - 2].close) / data[data.len() - 2].close
        } else {
            0.0
        };
        
        let is_bull = last_return > threshold;
        
        // Calculate regime probabilities based on persistence
        let mut probs = RegimeProbabilities {
            bull_trend: 0.0,
            bull_consolidation: 0.0,
            high_volatility: 0.0,
            bear_consolidation: 0.0,
            bear_trend: 0.0,
            low_volatility: 0.0,
        };
        
        if is_bull {
            probs.bull_trend = p_bull_to_bull * 0.6;
            probs.bull_consolidation = p_bull_to_bull * 0.4;
            probs.bear_trend = (1.0 - p_bull_to_bull) * 0.6;
            probs.bear_consolidation = (1.0 - p_bull_to_bull) * 0.4;
        } else {
            probs.bear_trend = p_bear_to_bear * 0.6;
            probs.bear_consolidation = p_bear_to_bear * 0.4;
            probs.bull_trend = (1.0 - p_bear_to_bear) * 0.6;
            probs.bull_consolidation = (1.0 - p_bear_to_bear) * 0.4;
        }
        
        // Adjust for volatility
        if stats.volatility > self.config.high_vol_threshold {
            probs.high_volatility = 0.3;
        } else if stats.volatility < self.config.low_vol_threshold {
            probs.low_volatility = 0.3;
        }
        
        Ok(probs)
    }
    
    /// Hidden Markov Model regime detection
    fn hidden_markov_regime(
        &self,
        data: &[TimeSeriesData],
        stats: &MarketStatistics,
    ) -> Result<RegimeProbabilities> {
        // Simplified HMM with observable features
        let features = self.extract_hmm_features(data)?;
        
        // Define emission probabilities for each regime
        let mut regime_scores = HashMap::new();
        
        // Bull trend: positive returns, low volatility, high momentum
        regime_scores.insert(MarketRegime::BullTrend, 
            features.momentum * 0.4 + 
            (1.0 - features.volatility_norm) * 0.3 + 
            features.trend_strength * 0.3
        );
        
        // Bear trend: negative returns, moderate volatility, negative momentum
        regime_scores.insert(MarketRegime::BearTrend,
            (-features.momentum) * 0.4 + 
            features.volatility_norm * 0.2 + 
            (1.0 - features.trend_strength) * 0.4
        );
        
        // High volatility: high vol, low trend, high dispersion
        regime_scores.insert(MarketRegime::HighVolatility,
            features.volatility_norm * 0.5 + 
            features.price_dispersion * 0.3 + 
            (1.0 - features.trend_strength.abs()) * 0.2
        );
        
        // Low volatility: low vol, low dispersion, neutral momentum
        regime_scores.insert(MarketRegime::LowVolatility,
            (1.0 - features.volatility_norm) * 0.5 + 
            (1.0 - features.price_dispersion) * 0.3 + 
            (1.0 - features.momentum.abs()) * 0.2
        );
        
        // Consolidation regimes
        regime_scores.insert(MarketRegime::BullConsolidation,
            features.momentum.max(0.0) * 0.3 + 
            (0.5 - features.volatility_norm).abs() * 0.4 + 
            (1.0 - features.trend_strength) * 0.3
        );
        
        regime_scores.insert(MarketRegime::BearConsolidation,
            (-features.momentum).max(0.0) * 0.3 + 
            (0.5 - features.volatility_norm).abs() * 0.4 + 
            (1.0 - features.trend_strength) * 0.3
        );
        
        // Convert scores to probabilities
        let total_score: f64 = regime_scores.values().sum();
        
        Ok(RegimeProbabilities {
            bull_trend: regime_scores[&MarketRegime::BullTrend] / total_score,
            bull_consolidation: regime_scores[&MarketRegime::BullConsolidation] / total_score,
            high_volatility: regime_scores[&MarketRegime::HighVolatility] / total_score,
            bear_consolidation: regime_scores[&MarketRegime::BearConsolidation] / total_score,
            bear_trend: regime_scores[&MarketRegime::BearTrend] / total_score,
            low_volatility: regime_scores[&MarketRegime::LowVolatility] / total_score,
        })
    }
    
    /// Extract features for HMM
    fn extract_hmm_features(&self, data: &[TimeSeriesData]) -> Result<HmmFeatures> {
        let recent: Vec<&TimeSeriesData> = data.iter()
            .rev()
            .take(self.config.lookback_period)
            .collect();
        
        // Calculate momentum
        let momentum = if recent.len() >= 20 {
            let recent_avg = recent[..10].iter()
                .map(|d| d.close)
                .sum::<f64>() / 10.0;
            let older_avg = recent[10..20].iter()
                .map(|d| d.close)
                .sum::<f64>() / 10.0;
            
            (recent_avg - older_avg) / older_avg
        } else {
            0.0
        };
        
        // Calculate normalized volatility
        let returns: Vec<f64> = recent.windows(2)
            .map(|w| (w[0].close / w[1].close).ln())
            .collect();
        
        let volatility = self.calculate_volatility(&returns);
        let volatility_norm = (volatility / 0.03).min(1.0); // Normalize to [0, 1]
        
        // Calculate trend strength
        let first_price = recent.last().unwrap().close;
        let last_price = recent.first().unwrap().close;
        let trend_strength = ((last_price - first_price) / first_price) / (recent.len() as f64).sqrt();
        
        // Calculate price dispersion
        let mean_price = recent.iter().map(|d| d.close).sum::<f64>() / recent.len() as f64;
        let price_dispersion = recent.iter()
            .map(|d| ((d.close - mean_price) / mean_price).abs())
            .sum::<f64>() / recent.len() as f64;
        
        Ok(HmmFeatures {
            momentum,
            volatility_norm,
            trend_strength,
            price_dispersion,
        })
    }
    
    /// Combine probabilities from different models
    fn combine_probabilities(
        &self,
        probs1: RegimeProbabilities,
        probs2: RegimeProbabilities,
        weight2: f64,
    ) -> RegimeProbabilities {
        let weight1 = 1.0 - weight2;
        
        RegimeProbabilities {
            bull_trend: probs1.bull_trend * weight1 + probs2.bull_trend * weight2,
            bull_consolidation: probs1.bull_consolidation * weight1 + probs2.bull_consolidation * weight2,
            high_volatility: probs1.high_volatility * weight1 + probs2.high_volatility * weight2,
            bear_consolidation: probs1.bear_consolidation * weight1 + probs2.bear_consolidation * weight2,
            bear_trend: probs1.bear_trend * weight1 + probs2.bear_trend * weight2,
            low_volatility: probs1.low_volatility * weight1 + probs2.low_volatility * weight2,
        }
    }
    
    /// Normalize probabilities to sum to 1
    fn normalize_probabilities(&self, probs: &mut RegimeProbabilities) {
        let total = probs.bull_trend + probs.bull_consolidation + probs.high_volatility +
                   probs.bear_consolidation + probs.bear_trend + probs.low_volatility;
        
        if total > 0.0 {
            probs.bull_trend /= total;
            probs.bull_consolidation /= total;
            probs.high_volatility /= total;
            probs.bear_consolidation /= total;
            probs.bear_trend /= total;
            probs.low_volatility /= total;
        }
    }
    
    // Helper methods
    
    fn calculate_volatility(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|&r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        variance.sqrt()
    }
    
    fn calculate_max_drawdown(&self, data: &[&TimeSeriesData]) -> f64 {
        let mut max_price = 0.0;
        let mut max_drawdown = 0.0;
        
        for d in data.iter().rev() {
            if d.close > max_price {
                max_price = d.close;
            }
            
            let drawdown = (max_price - d.close) / max_price;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }
        
        max_drawdown
    }
    
    fn calculate_volatility_of_volatility(&self, data: &[&TimeSeriesData]) -> Result<f64> {
        let window = 10;
        if data.len() < window * 2 {
            return Ok(0.0);
        }
        
        let mut volatilities = Vec::new();
        
        for i in window..data.len() {
            let window_data = &data[i - window..i];
            let returns: Vec<f64> = window_data.windows(2)
                .map(|w| (w[1].close / w[0].close).ln())
                .collect();
            
            let vol = self.calculate_volatility(&returns);
            volatilities.push(vol);
        }
        
        Ok(self.calculate_volatility(&volatilities))
    }
    
    fn calculate_skewness(&self, returns: &[f64]) -> f64 {
        if returns.len() < 3 {
            return 0.0;
        }
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let std = self.calculate_volatility(returns);
        
        if std == 0.0 {
            return 0.0;
        }
        
        let n = returns.len() as f64;
        let skewness = returns.iter()
            .map(|&r| ((r - mean) / std).powi(3))
            .sum::<f64>() / n;
        
        skewness
    }
    
    fn calculate_kurtosis(&self, returns: &[f64]) -> f64 {
        if returns.len() < 4 {
            return 3.0; // Normal distribution kurtosis
        }
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let std = self.calculate_volatility(returns);
        
        if std == 0.0 {
            return 3.0;
        }
        
        let n = returns.len() as f64;
        let kurtosis = returns.iter()
            .map(|&r| ((r - mean) / std).powi(4))
            .sum::<f64>() / n;
        
        kurtosis
    }
}

/// Market statistics for regime detection
#[derive(Debug)]
struct MarketStatistics {
    mean_return: f64,
    volatility: f64,
    total_return: f64,
    up_days: usize,
    down_days: usize,
    max_drawdown: f64,
    vol_of_vol: f64,
    skewness: f64,
    kurtosis: f64,
}

/// Features for Hidden Markov Model
#[derive(Debug)]
struct HmmFeatures {
    momentum: f64,
    volatility_norm: f64,
    trend_strength: f64,
    price_dispersion: f64,
}

impl Default for RegimeDetector {
    fn default() -> Self {
        Self::new()
    }
}