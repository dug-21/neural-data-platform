//! Feature Engineering for Neural Model Training
//! 
//! This module provides comprehensive feature extraction and engineering specifically
//! designed for training neural networks in trading applications. It includes
//! technical indicators, price transformations, market microstructure features,
//! and rolling statistics with proper scaling and normalization.

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use crate::data::TimeSeriesData;

/// Feature engineering engine for neural network training
pub struct TrainingFeatureEngine {
    config: FeatureConfig,
    scalers: HashMap<String, FeatureScaler>,
    feature_metadata: HashMap<String, FeatureMetadata>,
}

/// Configuration for feature engineering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Technical indicator periods
    pub indicator_periods: Vec<usize>,
    
    /// Price transformation settings
    pub return_periods: Vec<usize>,
    
    /// Volatility window sizes
    pub volatility_windows: Vec<usize>,
    
    /// Market microstructure settings
    pub microstructure_enabled: bool,
    
    /// Rolling statistics windows
    pub rolling_windows: Vec<usize>,
    
    /// Normalization method
    pub normalization: NormalizationMethod,
    
    /// Handle missing data
    pub handle_missing: MissingDataStrategy,
    
    /// Feature selection threshold
    pub min_feature_variance: f64,
    
    /// Enable incremental updates
    pub incremental_updates: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            indicator_periods: vec![5, 10, 20, 50, 100],
            return_periods: vec![1, 5, 10, 20],
            volatility_windows: vec![10, 20, 30, 60],
            microstructure_enabled: true,
            rolling_windows: vec![5, 10, 20, 50],
            normalization: NormalizationMethod::ZScore,
            handle_missing: MissingDataStrategy::Forward,
            min_feature_variance: 1e-6,
            incremental_updates: true,
        }
    }
}

/// Normalization methods for features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationMethod {
    MinMax,
    ZScore,
    RobustScaler,
    Tanh,
    Percentile,
}

/// Strategies for handling missing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MissingDataStrategy {
    Drop,
    Forward,
    Backward,
    Interpolate,
    Mean,
}

/// Feature scaler for normalization
#[derive(Debug, Clone)]
pub struct FeatureScaler {
    method: NormalizationMethod,
    params: ScalerParams,
}

/// Parameters for different scaling methods
#[derive(Debug, Clone)]
pub enum ScalerParams {
    MinMax { min: f64, max: f64 },
    ZScore { mean: f64, std: f64 },
    Robust { median: f64, mad: f64 },
    Percentile { p5: f64, p95: f64 },
}

/// Metadata for each feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    pub name: String,
    pub category: String,
    pub importance: f64,
    pub variance: f64,
    pub missing_ratio: f64,
    pub last_updated: DateTime<Utc>,
}

/// Feature set for neural network training
#[derive(Debug, Clone)]
pub struct TrainingFeatures {
    pub features: HashMap<String, Vec<f64>>,
    pub feature_names: Vec<String>,
    pub timestamps: Vec<DateTime<Utc>>,
    pub symbols: Vec<String>,
    pub metadata: HashMap<String, FeatureMetadata>,
}

impl TrainingFeatureEngine {
    /// Create a new feature engineering engine
    pub fn new(config: FeatureConfig) -> Self {
        Self {
            config,
            scalers: HashMap::new(),
            feature_metadata: HashMap::new(),
        }
    }
    
    /// Extract all features for neural network training
    pub async fn extract_features(
        &mut self,
        symbol: &str,
        data: &[TimeSeriesData],
    ) -> Result<TrainingFeatures> {
        if data.is_empty() {
            return Err(anyhow!("No data provided for feature extraction"));
        }
        
        let mut all_features = HashMap::new();
        let mut timestamps = Vec::new();
        let mut symbols = Vec::new();
        
        // Technical indicators
        let technical_features = self.compute_technical_indicators(data)?;
        all_features.extend(technical_features);
        
        // Price transformations
        let price_features = self.compute_price_transformations(data)?;
        all_features.extend(price_features);
        
        // Market microstructure features
        if self.config.microstructure_enabled {
            let microstructure_features = self.compute_microstructure_features(data)?;
            all_features.extend(microstructure_features);
        }
        
        // Rolling statistics
        let rolling_features = self.compute_rolling_statistics(data)?;
        all_features.extend(rolling_features);
        
        // Volatility features
        let volatility_features = self.compute_volatility_features(data)?;
        all_features.extend(volatility_features);
        
        // Time-based features
        let time_features = self.compute_time_features(data)?;
        all_features.extend(time_features);
        
        // Handle missing data
        self.handle_missing_data(&mut all_features)?;
        
        // Normalize features
        self.normalize_features(&mut all_features)?;
        
        // Feature quality checks
        self.validate_features(&all_features)?;
        
        // Update metadata
        self.update_feature_metadata(&all_features)?;
        
        // Prepare output
        for item in data {
            timestamps.push(item.timestamp);
            symbols.push(symbol.to_string());
        }
        
        let feature_names: Vec<String> = all_features.keys().cloned().collect();
        
        Ok(TrainingFeatures {
            features: all_features,
            feature_names,
            timestamps,
            symbols,
            metadata: self.feature_metadata.clone(),
        })
    }
    
    /// Compute technical indicators
    fn compute_technical_indicators(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, Vec<f64>>> {
        let mut features = HashMap::new();
        let closes: Vec<f64> = data.iter().map(|d| d.close).collect();
        let highs: Vec<f64> = data.iter().map(|d| d.high).collect();
        let lows: Vec<f64> = data.iter().map(|d| d.low).collect();
        let volumes: Vec<f64> = data.iter().map(|d| d.volume).collect();
        
        // RSI for different periods
        for &period in &self.config.indicator_periods {
            if data.len() >= period {
                let rsi = self.calculate_rsi(&closes, period)?;
                features.insert(format!("rsi_{}", period), rsi);
            }
        }
        
        // MACD
        if data.len() >= 26 {
            let (macd_line, signal, histogram) = self.calculate_macd(&closes)?;
            features.insert("macd_line".to_string(), macd_line);
            features.insert("macd_signal".to_string(), signal);
            features.insert("macd_histogram".to_string(), histogram);
        }
        
        // Bollinger Bands
        for &period in &[20, 50] {
            if data.len() >= period {
                let (upper, middle, lower) = self.calculate_bollinger_bands(&closes, period)?;
                features.insert(format!("bb_upper_{}", period), upper);
                features.insert(format!("bb_middle_{}", period), middle);
                features.insert(format!("bb_lower_{}", period), lower);
                
                // Price position relative to bands
                let bb_position: Vec<f64> = closes.iter().zip(&upper).zip(&lower)
                    .map(|((close, upper), lower)| {
                        if upper != lower {
                            (close - lower) / (upper - lower)
                        } else {
                            0.5
                        }
                    })
                    .collect();
                features.insert(format!("bb_position_{}", period), bb_position);
            }
        }
        
        // ATR (Average True Range)
        for &period in &[14, 20] {
            if data.len() >= period {
                let atr = self.calculate_atr(&highs, &lows, &closes, period)?;
                features.insert(format!("atr_{}", period), atr);
            }
        }
        
        // Stochastic Oscillator
        if data.len() >= 14 {
            let (k, d) = self.calculate_stochastic(&highs, &lows, &closes, 14)?;
            features.insert("stoch_k".to_string(), k);
            features.insert("stoch_d".to_string(), d);
        }
        
        // OBV (On-Balance Volume)
        let obv = self.calculate_obv(&closes, &volumes)?;
        features.insert("obv".to_string(), obv);
        
        // Money Flow Index
        if data.len() >= 14 {
            let mfi = self.calculate_mfi(&highs, &lows, &closes, &volumes, 14)?;
            features.insert("mfi".to_string(), mfi);
        }
        
        Ok(features)
    }
    
    /// Compute price transformations
    fn compute_price_transformations(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, Vec<f64>>> {
        let mut features = HashMap::new();
        let closes: Vec<f64> = data.iter().map(|d| d.close).collect();
        
        // Returns for different periods
        for &period in &self.config.return_periods {
            if data.len() > period {
                let returns = self.calculate_returns(&closes, period)?;
                features.insert(format!("return_{}", period), returns);
                
                // Log returns
                let log_returns = self.calculate_log_returns(&closes, period)?;
                features.insert(format!("log_return_{}", period), log_returns);
            }
        }
        
        // Price ratios
        let price_ratios: Vec<f64> = data.iter()
            .map(|d| {
                if d.open != 0.0 {
                    d.close / d.open
                } else {
                    1.0
                }
            })
            .collect();
        features.insert("close_open_ratio".to_string(), price_ratios);
        
        // High-Low spread
        let hl_spread: Vec<f64> = data.iter()
            .map(|d| {
                if d.close != 0.0 {
                    (d.high - d.low) / d.close
                } else {
                    0.0
                }
            })
            .collect();
        features.insert("hl_spread".to_string(), hl_spread);
        
        // Price position in daily range
        let price_position: Vec<f64> = data.iter()
            .map(|d| {
                if d.high != d.low {
                    (d.close - d.low) / (d.high - d.low)
                } else {
                    0.5
                }
            })
            .collect();
        features.insert("price_position".to_string(), price_position);
        
        Ok(features)
    }
    
    /// Compute market microstructure features
    fn compute_microstructure_features(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, Vec<f64>>> {
        let mut features = HashMap::new();
        
        // Bid-Ask spread proxy (using high-low)
        let spread_proxy: Vec<f64> = data.iter()
            .map(|d| {
                if d.close != 0.0 {
                    2.0 * (d.high - d.low) / (d.high + d.low)
                } else {
                    0.0
                }
            })
            .collect();
        features.insert("spread_proxy".to_string(), spread_proxy);
        
        // Volume profile features
        let volume_mean = data.iter().map(|d| d.volume).sum::<f64>() / data.len() as f64;
        let volume_ratio: Vec<f64> = data.iter()
            .map(|d| {
                if volume_mean != 0.0 {
                    d.volume / volume_mean
                } else {
                    1.0
                }
            })
            .collect();
        features.insert("volume_ratio".to_string(), volume_ratio);
        
        // Kyle's lambda (price impact)
        if data.len() >= 20 {
            let lambda = self.calculate_kyles_lambda(data)?;
            features.insert("kyles_lambda".to_string(), lambda);
        }
        
        // Amihud illiquidity
        let amihud: Vec<f64> = data.iter()
            .map(|d| {
                if d.volume * d.close != 0.0 {
                    ((d.close - d.open).abs() / d.open) / (d.volume * d.close)
                } else {
                    0.0
                }
            })
            .collect();
        features.insert("amihud_illiquidity".to_string(), amihud);
        
        // Roll's implicit spread
        if data.len() >= 2 {
            let roll_spread = self.calculate_roll_spread(data)?;
            features.insert("roll_spread".to_string(), roll_spread);
        }
        
        Ok(features)
    }
    
    /// Compute rolling statistics
    fn compute_rolling_statistics(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, Vec<f64>>> {
        let mut features = HashMap::new();
        let closes: Vec<f64> = data.iter().map(|d| d.close).collect();
        let returns = self.calculate_returns(&closes, 1)?;
        
        for &window in &self.config.rolling_windows {
            if data.len() >= window {
                // Rolling mean
                let rolling_mean = self.calculate_rolling_mean(&closes, window)?;
                features.insert(format!("rolling_mean_{}", window), rolling_mean);
                
                // Rolling standard deviation
                let rolling_std = self.calculate_rolling_std(&closes, window)?;
                features.insert(format!("rolling_std_{}", window), rolling_std);
                
                // Rolling skewness
                let rolling_skew = self.calculate_rolling_skew(&returns, window)?;
                features.insert(format!("rolling_skew_{}", window), rolling_skew);
                
                // Rolling kurtosis
                let rolling_kurt = self.calculate_rolling_kurtosis(&returns, window)?;
                features.insert(format!("rolling_kurtosis_{}", window), rolling_kurt);
                
                // Rolling correlation with volume
                let volumes: Vec<f64> = data.iter().map(|d| d.volume).collect();
                let rolling_corr = self.calculate_rolling_correlation(&closes, &volumes, window)?;
                features.insert(format!("price_volume_corr_{}", window), rolling_corr);
            }
        }
        
        Ok(features)
    }
    
    /// Compute volatility features
    fn compute_volatility_features(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, Vec<f64>>> {
        let mut features = HashMap::new();
        let closes: Vec<f64> = data.iter().map(|d| d.close).collect();
        let returns = self.calculate_returns(&closes, 1)?;
        
        for &window in &self.config.volatility_windows {
            if data.len() >= window {
                // Historical volatility
                let hist_vol = self.calculate_historical_volatility(&returns, window)?;
                features.insert(format!("hist_vol_{}", window), hist_vol);
                
                // Parkinson volatility
                let parkinson_vol = self.calculate_parkinson_volatility(data, window)?;
                features.insert(format!("parkinson_vol_{}", window), parkinson_vol);
                
                // Garman-Klass volatility
                let gk_vol = self.calculate_garman_klass_volatility(data, window)?;
                features.insert(format!("garman_klass_vol_{}", window), gk_vol);
                
                // Rogers-Satchell volatility
                let rs_vol = self.calculate_rogers_satchell_volatility(data, window)?;
                features.insert(format!("rogers_satchell_vol_{}", window), rs_vol);
            }
        }
        
        // Volatility regime indicators
        if data.len() >= 60 {
            let vol_regime = self.detect_volatility_regime(&returns)?;
            features.insert("volatility_regime".to_string(), vol_regime);
        }
        
        Ok(features)
    }
    
    /// Compute time-based features
    fn compute_time_features(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, Vec<f64>>> {
        let mut features = HashMap::new();
        
        // Hour of day (normalized)
        let hour_of_day: Vec<f64> = data.iter()
            .map(|d| d.timestamp.hour() as f64 / 24.0)
            .collect();
        features.insert("hour_of_day".to_string(), hour_of_day);
        
        // Day of week (normalized)
        let day_of_week: Vec<f64> = data.iter()
            .map(|d| d.timestamp.weekday().num_days_from_monday() as f64 / 7.0)
            .collect();
        features.insert("day_of_week".to_string(), day_of_week);
        
        // Day of month (normalized)
        let day_of_month: Vec<f64> = data.iter()
            .map(|d| d.timestamp.day() as f64 / 31.0)
            .collect();
        features.insert("day_of_month".to_string(), day_of_month);
        
        // Month of year (normalized)
        let month_of_year: Vec<f64> = data.iter()
            .map(|d| d.timestamp.month() as f64 / 12.0)
            .collect();
        features.insert("month_of_year".to_string(), month_of_year);
        
        // Quarter indicator
        let quarter: Vec<f64> = data.iter()
            .map(|d| ((d.timestamp.month() - 1) / 3 + 1) as f64 / 4.0)
            .collect();
        features.insert("quarter".to_string(), quarter);
        
        // Trading session indicators
        let trading_session: Vec<f64> = data.iter()
            .map(|d| {
                let hour = d.timestamp.hour();
                if hour >= 9 && hour < 16 {
                    1.0  // Regular trading hours
                } else if hour >= 4 && hour < 9 {
                    0.5  // Pre-market
                } else if hour >= 16 && hour < 20 {
                    0.5  // After-hours
                } else {
                    0.0  // Closed
                }
            })
            .collect();
        features.insert("trading_session".to_string(), trading_session);
        
        Ok(features)
    }
    
    // Helper calculation methods
    
    fn calculate_rsi(&self, closes: &[f64], period: usize) -> Result<Vec<f64>> {
        let mut rsi = vec![50.0; closes.len()];
        
        if closes.len() < period + 1 {
            return Ok(rsi);
        }
        
        for i in period..closes.len() {
            let mut gains = 0.0;
            let mut losses = 0.0;
            
            for j in (i - period + 1)..=i {
                let change = closes[j] - closes[j - 1];
                if change > 0.0 {
                    gains += change;
                } else {
                    losses += change.abs();
                }
            }
            
            let avg_gain = gains / period as f64;
            let avg_loss = losses / period as f64;
            
            if avg_loss == 0.0 {
                rsi[i] = 100.0;
            } else {
                let rs = avg_gain / avg_loss;
                rsi[i] = 100.0 - (100.0 / (1.0 + rs));
            }
        }
        
        Ok(rsi)
    }
    
    fn calculate_macd(&self, closes: &[f64]) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
        let fast_period = 12;
        let slow_period = 26;
        let signal_period = 9;
        
        let fast_ema = self.calculate_ema(closes, fast_period)?;
        let slow_ema = self.calculate_ema(closes, slow_period)?;
        
        let macd_line: Vec<f64> = fast_ema.iter()
            .zip(&slow_ema)
            .map(|(fast, slow)| fast - slow)
            .collect();
        
        let signal_line = self.calculate_ema(&macd_line, signal_period)?;
        
        let histogram: Vec<f64> = macd_line.iter()
            .zip(&signal_line)
            .map(|(macd, signal)| macd - signal)
            .collect();
        
        Ok((macd_line, signal_line, histogram))
    }
    
    fn calculate_ema(&self, values: &[f64], period: usize) -> Result<Vec<f64>> {
        if values.is_empty() {
            return Ok(vec![]);
        }
        
        let mut ema = vec![values[0]; values.len()];
        let alpha = 2.0 / (period as f64 + 1.0);
        
        for i in 1..values.len() {
            ema[i] = alpha * values[i] + (1.0 - alpha) * ema[i - 1];
        }
        
        Ok(ema)
    }
    
    fn calculate_bollinger_bands(
        &self,
        closes: &[f64],
        period: usize,
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
        let sma = self.calculate_rolling_mean(closes, period)?;
        let std = self.calculate_rolling_std(closes, period)?;
        
        let upper: Vec<f64> = sma.iter()
            .zip(&std)
            .map(|(mean, std)| mean + 2.0 * std)
            .collect();
        
        let lower: Vec<f64> = sma.iter()
            .zip(&std)
            .map(|(mean, std)| mean - 2.0 * std)
            .collect();
        
        Ok((upper, sma, lower))
    }
    
    fn calculate_atr(
        &self,
        highs: &[f64],
        lows: &[f64],
        closes: &[f64],
        period: usize,
    ) -> Result<Vec<f64>> {
        let mut true_ranges = vec![0.0; highs.len()];
        
        for i in 1..highs.len() {
            let high_low = highs[i] - lows[i];
            let high_close = (highs[i] - closes[i - 1]).abs();
            let low_close = (lows[i] - closes[i - 1]).abs();
            true_ranges[i] = high_low.max(high_close).max(low_close);
        }
        
        self.calculate_rolling_mean(&true_ranges, period)
    }
    
    fn calculate_stochastic(
        &self,
        highs: &[f64],
        lows: &[f64],
        closes: &[f64],
        period: usize,
    ) -> Result<(Vec<f64>, Vec<f64>)> {
        let mut k = vec![50.0; closes.len()];
        
        for i in period..closes.len() {
            let highest = highs[(i - period + 1)..=i]
                .iter()
                .fold(f64::MIN, |a, &b| a.max(b));
            let lowest = lows[(i - period + 1)..=i]
                .iter()
                .fold(f64::MAX, |a, &b| a.min(b));
            
            if highest != lowest {
                k[i] = 100.0 * (closes[i] - lowest) / (highest - lowest);
            }
        }
        
        let d = self.calculate_rolling_mean(&k, 3)?;
        
        Ok((k, d))
    }
    
    fn calculate_obv(&self, closes: &[f64], volumes: &[f64]) -> Result<Vec<f64>> {
        let mut obv = vec![0.0; closes.len()];
        
        if !closes.is_empty() {
            obv[0] = volumes[0];
        }
        
        for i in 1..closes.len() {
            if closes[i] > closes[i - 1] {
                obv[i] = obv[i - 1] + volumes[i];
            } else if closes[i] < closes[i - 1] {
                obv[i] = obv[i - 1] - volumes[i];
            } else {
                obv[i] = obv[i - 1];
            }
        }
        
        Ok(obv)
    }
    
    fn calculate_mfi(
        &self,
        highs: &[f64],
        lows: &[f64],
        closes: &[f64],
        volumes: &[f64],
        period: usize,
    ) -> Result<Vec<f64>> {
        let mut mfi = vec![50.0; closes.len()];
        
        for i in period..closes.len() {
            let mut positive_flow = 0.0;
            let mut negative_flow = 0.0;
            
            for j in (i - period + 1)..=i {
                let typical_price = (highs[j] + lows[j] + closes[j]) / 3.0;
                let prev_typical = (highs[j - 1] + lows[j - 1] + closes[j - 1]) / 3.0;
                let money_flow = typical_price * volumes[j];
                
                if typical_price > prev_typical {
                    positive_flow += money_flow;
                } else if typical_price < prev_typical {
                    negative_flow += money_flow;
                }
            }
            
            if negative_flow == 0.0 {
                mfi[i] = 100.0;
            } else {
                let money_ratio = positive_flow / negative_flow;
                mfi[i] = 100.0 - (100.0 / (1.0 + money_ratio));
            }
        }
        
        Ok(mfi)
    }
    
    fn calculate_returns(&self, closes: &[f64], period: usize) -> Result<Vec<f64>> {
        let mut returns = vec![0.0; closes.len()];
        
        for i in period..closes.len() {
            if closes[i - period] != 0.0 {
                returns[i] = (closes[i] - closes[i - period]) / closes[i - period];
            }
        }
        
        Ok(returns)
    }
    
    fn calculate_log_returns(&self, closes: &[f64], period: usize) -> Result<Vec<f64>> {
        let mut log_returns = vec![0.0; closes.len()];
        
        for i in period..closes.len() {
            if closes[i - period] > 0.0 && closes[i] > 0.0 {
                log_returns[i] = (closes[i] / closes[i - period]).ln();
            }
        }
        
        Ok(log_returns)
    }
    
    fn calculate_kyles_lambda(&self, data: &[TimeSeriesData]) -> Result<Vec<f64>> {
        let mut lambda = vec![0.0; data.len()];
        let window = 20;
        
        for i in window..data.len() {
            let mut price_changes = Vec::new();
            let mut signed_volumes = Vec::new();
            
            for j in (i - window + 1)..=i {
                let price_change = (data[j].close - data[j - 1].close).abs();
                let volume_sign = if data[j].close > data[j - 1].close { 1.0 } else { -1.0 };
                
                price_changes.push(price_change);
                signed_volumes.push(volume_sign * data[j].volume);
            }
            
            // Simple regression coefficient
            let vol_sum: f64 = signed_volumes.iter().map(|v| v.abs()).sum();
            let price_sum: f64 = price_changes.iter().sum();
            
            if vol_sum != 0.0 {
                lambda[i] = price_sum / vol_sum;
            }
        }
        
        Ok(lambda)
    }
    
    fn calculate_roll_spread(&self, data: &[TimeSeriesData]) -> Result<Vec<f64>> {
        let mut spreads = vec![0.0; data.len()];
        
        if data.len() < 2 {
            return Ok(spreads);
        }
        
        for i in 1..data.len() {
            let price_change = data[i].close - data[i - 1].close;
            spreads[i] = 2.0 * price_change.abs().sqrt();
        }
        
        Ok(spreads)
    }
    
    fn calculate_rolling_mean(&self, values: &[f64], window: usize) -> Result<Vec<f64>> {
        let mut means = vec![0.0; values.len()];
        
        for i in 0..values.len() {
            let start = if i >= window { i - window + 1 } else { 0 };
            let sum: f64 = values[start..=i].iter().sum();
            means[i] = sum / (i - start + 1) as f64;
        }
        
        Ok(means)
    }
    
    fn calculate_rolling_std(&self, values: &[f64], window: usize) -> Result<Vec<f64>> {
        let means = self.calculate_rolling_mean(values, window)?;
        let mut stds = vec![0.0; values.len()];
        
        for i in 0..values.len() {
            let start = if i >= window { i - window + 1 } else { 0 };
            let mean = means[i];
            
            let variance: f64 = values[start..=i].iter()
                .map(|&v| (v - mean).powi(2))
                .sum::<f64>() / (i - start + 1) as f64;
            
            stds[i] = variance.sqrt();
        }
        
        Ok(stds)
    }
    
    fn calculate_rolling_skew(&self, values: &[f64], window: usize) -> Result<Vec<f64>> {
        let means = self.calculate_rolling_mean(values, window)?;
        let stds = self.calculate_rolling_std(values, window)?;
        let mut skews = vec![0.0; values.len()];
        
        for i in 0..values.len() {
            let start = if i >= window { i - window + 1 } else { 0 };
            let mean = means[i];
            let std = stds[i];
            
            if std != 0.0 {
                let n = (i - start + 1) as f64;
                let sum_cubed: f64 = values[start..=i].iter()
                    .map(|&v| ((v - mean) / std).powi(3))
                    .sum();
                
                skews[i] = (n / ((n - 1.0) * (n - 2.0))) * sum_cubed;
            }
        }
        
        Ok(skews)
    }
    
    fn calculate_rolling_kurtosis(&self, values: &[f64], window: usize) -> Result<Vec<f64>> {
        let means = self.calculate_rolling_mean(values, window)?;
        let stds = self.calculate_rolling_std(values, window)?;
        let mut kurts = vec![0.0; values.len()];
        
        for i in 0..values.len() {
            let start = if i >= window { i - window + 1 } else { 0 };
            let mean = means[i];
            let std = stds[i];
            
            if std != 0.0 {
                let n = (i - start + 1) as f64;
                let sum_fourth: f64 = values[start..=i].iter()
                    .map(|&v| ((v - mean) / std).powi(4))
                    .sum();
                
                kurts[i] = (n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0))) * sum_fourth
                    - 3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0));
            }
        }
        
        Ok(kurts)
    }
    
    fn calculate_rolling_correlation(
        &self,
        x: &[f64],
        y: &[f64],
        window: usize,
    ) -> Result<Vec<f64>> {
        let mut correlations = vec![0.0; x.len()];
        
        for i in window..x.len() {
            let x_window = &x[(i - window + 1)..=i];
            let y_window = &y[(i - window + 1)..=i];
            
            let x_mean = x_window.iter().sum::<f64>() / window as f64;
            let y_mean = y_window.iter().sum::<f64>() / window as f64;
            
            let cov: f64 = x_window.iter()
                .zip(y_window)
                .map(|(xi, yi)| (xi - x_mean) * (yi - y_mean))
                .sum::<f64>() / window as f64;
            
            let x_std = (x_window.iter()
                .map(|xi| (xi - x_mean).powi(2))
                .sum::<f64>() / window as f64).sqrt();
            
            let y_std = (y_window.iter()
                .map(|yi| (yi - y_mean).powi(2))
                .sum::<f64>() / window as f64).sqrt();
            
            if x_std != 0.0 && y_std != 0.0 {
                correlations[i] = cov / (x_std * y_std);
            }
        }
        
        Ok(correlations)
    }
    
    fn calculate_historical_volatility(
        &self,
        returns: &[f64],
        window: usize,
    ) -> Result<Vec<f64>> {
        let rolling_std = self.calculate_rolling_std(returns, window)?;
        
        // Annualize (assuming 252 trading days)
        let annualized: Vec<f64> = rolling_std.iter()
            .map(|&std| std * (252.0_f64).sqrt())
            .collect();
        
        Ok(annualized)
    }
    
    fn calculate_parkinson_volatility(
        &self,
        data: &[TimeSeriesData],
        window: usize,
    ) -> Result<Vec<f64>> {
        let mut volatilities = vec![0.0; data.len()];
        let factor = 1.0 / (4.0 * (2.0_f64).ln());
        
        for i in window..data.len() {
            let sum_sq_log_hl: f64 = data[(i - window + 1)..=i].iter()
                .filter(|d| d.high > 0.0 && d.low > 0.0)
                .map(|d| ((d.high / d.low).ln()).powi(2))
                .sum();
            
            volatilities[i] = (factor * sum_sq_log_hl / window as f64).sqrt()
                * (252.0_f64).sqrt();
        }
        
        Ok(volatilities)
    }
    
    fn calculate_garman_klass_volatility(
        &self,
        data: &[TimeSeriesData],
        window: usize,
    ) -> Result<Vec<f64>> {
        let mut volatilities = vec![0.0; data.len()];
        
        for i in window..data.len() {
            let mut sum = 0.0;
            
            for j in (i - window + 1)..=i {
                if data[j].high > 0.0 && data[j].low > 0.0 && data[j].open > 0.0 {
                    let hl_term = 0.5 * ((data[j].high / data[j].low).ln()).powi(2);
                    let co_term = (2.0 * (2.0_f64).ln() - 1.0) 
                        * ((data[j].close / data[j].open).ln()).powi(2);
                    sum += hl_term - co_term;
                }
            }
            
            volatilities[i] = (sum / window as f64).sqrt() * (252.0_f64).sqrt();
        }
        
        Ok(volatilities)
    }
    
    fn calculate_rogers_satchell_volatility(
        &self,
        data: &[TimeSeriesData],
        window: usize,
    ) -> Result<Vec<f64>> {
        let mut volatilities = vec![0.0; data.len()];
        
        for i in window..data.len() {
            let mut sum = 0.0;
            
            for j in (i - window + 1)..=i {
                if data[j].high > 0.0 && data[j].close > 0.0 && 
                   data[j].low > 0.0 && data[j].open > 0.0 {
                    let hc = (data[j].high / data[j].close).ln();
                    let ho = (data[j].high / data[j].open).ln();
                    let lc = (data[j].low / data[j].close).ln();
                    let lo = (data[j].low / data[j].open).ln();
                    
                    sum += hc * ho + lc * lo;
                }
            }
            
            volatilities[i] = (sum / window as f64).sqrt() * (252.0_f64).sqrt();
        }
        
        Ok(volatilities)
    }
    
    fn detect_volatility_regime(&self, returns: &[f64]) -> Result<Vec<f64>> {
        let short_vol = self.calculate_historical_volatility(returns, 10)?;
        let long_vol = self.calculate_historical_volatility(returns, 60)?;
        
        let regime: Vec<f64> = short_vol.iter()
            .zip(&long_vol)
            .map(|(&short, &long)| {
                if long > 0.0 {
                    let ratio = short / long;
                    if ratio > 1.5 {
                        2.0  // High volatility regime
                    } else if ratio < 0.7 {
                        0.0  // Low volatility regime
                    } else {
                        1.0  // Normal volatility regime
                    }
                } else {
                    1.0
                }
            })
            .collect();
        
        Ok(regime)
    }
    
    /// Handle missing data according to strategy
    fn handle_missing_data(&self, features: &mut HashMap<String, Vec<f64>>) -> Result<()> {
        match self.config.handle_missing {
            MissingDataStrategy::Drop => {
                // Remove features with any missing values
                features.retain(|_, values| {
                    !values.iter().any(|v| v.is_nan() || v.is_infinite())
                });
            }
            MissingDataStrategy::Forward => {
                // Forward fill missing values
                for values in features.values_mut() {
                    let mut last_valid = 0.0;
                    for v in values.iter_mut() {
                        if v.is_nan() || v.is_infinite() {
                            *v = last_valid;
                        } else {
                            last_valid = *v;
                        }
                    }
                }
            }
            MissingDataStrategy::Backward => {
                // Backward fill missing values
                for values in features.values_mut() {
                    let mut next_valid = 0.0;
                    for v in values.iter_mut().rev() {
                        if v.is_nan() || v.is_infinite() {
                            *v = next_valid;
                        } else {
                            next_valid = *v;
                        }
                    }
                }
            }
            MissingDataStrategy::Interpolate => {
                // Linear interpolation
                for values in features.values_mut() {
                    let mut last_valid_idx = None;
                    let mut i = 0;
                    
                    while i < values.len() {
                        if !values[i].is_nan() && !values[i].is_infinite() {
                            if let Some(start) = last_valid_idx {
                                if i > start + 1 {
                                    // Interpolate between start and i
                                    let start_val = values[start];
                                    let end_val = values[i];
                                    let steps = (i - start) as f64;
                                    
                                    for j in (start + 1)..i {
                                        let progress = (j - start) as f64 / steps;
                                        values[j] = start_val + (end_val - start_val) * progress;
                                    }
                                }
                            }
                            last_valid_idx = Some(i);
                        }
                        i += 1;
                    }
                }
            }
            MissingDataStrategy::Mean => {
                // Replace with mean
                for values in features.values_mut() {
                    let valid_values: Vec<f64> = values.iter()
                        .filter(|v| !v.is_nan() && !v.is_infinite())
                        .cloned()
                        .collect();
                    
                    if !valid_values.is_empty() {
                        let mean = valid_values.iter().sum::<f64>() / valid_values.len() as f64;
                        
                        for v in values.iter_mut() {
                            if v.is_nan() || v.is_infinite() {
                                *v = mean;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Normalize features according to the specified method
    fn normalize_features(&mut self, features: &mut HashMap<String, Vec<f64>>) -> Result<()> {
        for (name, values) in features.iter_mut() {
            // Calculate scaler parameters if not already cached
            if !self.scalers.contains_key(name) {
                let scaler = match self.config.normalization {
                    NormalizationMethod::MinMax => {
                        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                        FeatureScaler {
                            method: NormalizationMethod::MinMax,
                            params: ScalerParams::MinMax { min, max },
                        }
                    }
                    NormalizationMethod::ZScore => {
                        let mean = values.iter().sum::<f64>() / values.len() as f64;
                        let variance = values.iter()
                            .map(|&v| (v - mean).powi(2))
                            .sum::<f64>() / values.len() as f64;
                        let std = variance.sqrt();
                        FeatureScaler {
                            method: NormalizationMethod::ZScore,
                            params: ScalerParams::ZScore { mean, std },
                        }
                    }
                    NormalizationMethod::RobustScaler => {
                        let mut sorted = values.clone();
                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let median = sorted[sorted.len() / 2];
                        
                        let deviations: Vec<f64> = sorted.iter()
                            .map(|&v| (v - median).abs())
                            .collect();
                        let mut sorted_dev = deviations;
                        sorted_dev.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let mad = sorted_dev[sorted_dev.len() / 2];
                        
                        FeatureScaler {
                            method: NormalizationMethod::RobustScaler,
                            params: ScalerParams::Robust { median, mad },
                        }
                    }
                    NormalizationMethod::Tanh => {
                        let mean = values.iter().sum::<f64>() / values.len() as f64;
                        let std = (values.iter()
                            .map(|&v| (v - mean).powi(2))
                            .sum::<f64>() / values.len() as f64).sqrt();
                        FeatureScaler {
                            method: NormalizationMethod::Tanh,
                            params: ScalerParams::ZScore { mean, std },
                        }
                    }
                    NormalizationMethod::Percentile => {
                        let mut sorted = values.clone();
                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let p5_idx = (sorted.len() as f64 * 0.05) as usize;
                        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
                        let p5 = sorted[p5_idx];
                        let p95 = sorted[p95_idx];
                        FeatureScaler {
                            method: NormalizationMethod::Percentile,
                            params: ScalerParams::Percentile { p5, p95 },
                        }
                    }
                };
                
                self.scalers.insert(name.clone(), scaler);
            }
            
            // Apply normalization
            if let Some(scaler) = self.scalers.get(name) {
                match &scaler.params {
                    ScalerParams::MinMax { min, max } => {
                        if max > min {
                            for v in values.iter_mut() {
                                *v = (*v - min) / (max - min);
                            }
                        }
                    }
                    ScalerParams::ZScore { mean, std } => {
                        if *std > 0.0 {
                            for v in values.iter_mut() {
                                *v = (*v - mean) / std;
                                if matches!(scaler.method, NormalizationMethod::Tanh) {
                                    *v = v.tanh();
                                }
                            }
                        }
                    }
                    ScalerParams::Robust { median, mad } => {
                        if *mad > 0.0 {
                            for v in values.iter_mut() {
                                *v = (*v - median) / (1.4826 * mad);
                            }
                        }
                    }
                    ScalerParams::Percentile { p5, p95 } => {
                        if p95 > p5 {
                            for v in values.iter_mut() {
                                *v = (*v - p5) / (p95 - p5);
                                *v = v.max(0.0).min(1.0);  // Clip to [0, 1]
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Validate feature quality
    fn validate_features(&self, features: &HashMap<String, Vec<f64>>) -> Result<()> {
        for (name, values) in features {
            // Check for constant features
            let variance = if values.len() > 1 {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
            } else {
                0.0
            };
            
            if variance < self.config.min_feature_variance {
                eprintln!("Warning: Feature '{}' has very low variance: {}", name, variance);
            }
            
            // Check for extreme values
            for (i, &v) in values.iter().enumerate() {
                if v.abs() > 10.0 {
                    eprintln!("Warning: Feature '{}' has extreme value {} at index {}", 
                        name, v, i);
                }
            }
        }
        
        Ok(())
    }
    
    /// Update feature metadata
    fn update_feature_metadata(&mut self, features: &HashMap<String, Vec<f64>>) -> Result<()> {
        for (name, values) in features {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values.iter()
                .map(|&v| (v - mean).powi(2))
                .sum::<f64>() / values.len() as f64;
            
            let missing_count = values.iter()
                .filter(|&&v| v.is_nan() || v.is_infinite())
                .count();
            let missing_ratio = missing_count as f64 / values.len() as f64;
            
            let metadata = FeatureMetadata {
                name: name.clone(),
                category: self.infer_category(name),
                importance: 0.0,  // To be updated by model feedback
                variance,
                missing_ratio,
                last_updated: Utc::now(),
            };
            
            self.feature_metadata.insert(name.clone(), metadata);
        }
        
        Ok(())
    }
    
    /// Infer feature category from name
    fn infer_category(&self, name: &str) -> String {
        if name.contains("rsi") || name.contains("macd") || name.contains("stoch") {
            "momentum".to_string()
        } else if name.contains("vol") || name.contains("atr") {
            "volatility".to_string()
        } else if name.contains("return") {
            "returns".to_string()
        } else if name.contains("bb_") || name.contains("ema") || name.contains("sma") {
            "trend".to_string()
        } else if name.contains("volume") || name.contains("obv") || name.contains("mfi") {
            "volume".to_string()
        } else if name.contains("spread") || name.contains("lambda") || name.contains("amihud") {
            "microstructure".to_string()
        } else if name.contains("hour") || name.contains("day") || name.contains("month") {
            "time".to_string()
        } else if name.contains("skew") || name.contains("kurt") || name.contains("corr") {
            "statistics".to_string()
        } else {
            "other".to_string()
        }
    }
    
    /// Update feature importances from model feedback
    pub fn update_importances(&mut self, importances: HashMap<String, f64>) -> Result<()> {
        for (name, importance) in importances {
            if let Some(metadata) = self.feature_metadata.get_mut(&name) {
                metadata.importance = importance;
                metadata.last_updated = Utc::now();
            }
        }
        Ok(())
    }
    
    /// Get top features by importance
    pub fn get_top_features(&self, n: usize) -> Vec<String> {
        let mut features: Vec<_> = self.feature_metadata.iter()
            .map(|(name, meta)| (name.clone(), meta.importance))
            .collect();
        
        features.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        features.into_iter()
            .take(n)
            .map(|(name, _)| name)
            .collect()
    }
    
    /// Incremental feature update for online learning
    pub async fn update_features_incremental(
        &mut self,
        symbol: &str,
        new_data: &TimeSeriesData,
        window_size: usize,
    ) -> Result<HashMap<String, f64>> {
        if !self.config.incremental_updates {
            return Err(anyhow!("Incremental updates not enabled"));
        }
        
        // This is a simplified version - in production, you'd maintain
        // rolling windows and update features efficiently
        let mut features = HashMap::new();
        
        // Update price-based features
        features.insert("close".to_string(), new_data.close);
        features.insert("volume".to_string(), new_data.volume);
        features.insert("high_low_ratio".to_string(), new_data.high / new_data.low);
        
        // Add more incremental calculations as needed
        
        Ok(features)
    }
}

impl Default for TrainingFeatureEngine {
    fn default() -> Self {
        Self::new(FeatureConfig::default())
    }
}

#[cfg(test)]
#[path = "training_features_tests.rs"]
mod tests;