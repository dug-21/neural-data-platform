//! Advanced Technical Indicators for Trading
//! 
//! Implements a comprehensive set of technical indicators optimized for
//! high-frequency trading and neural network feature engineering.

use anyhow::Result;
use std::collections::HashMap;
use ta::{DataItem, Next};
use crate::data::TimeSeriesData;

/// Technical indicator computation engine
pub struct TechnicalIndicatorEngine {
    /// Indicator configuration
    config: IndicatorConfig,
}

/// Configuration for technical indicators
#[derive(Debug, Clone)]
pub struct IndicatorConfig {
    /// EMA periods
    pub ema_periods: Vec<usize>,
    
    /// RSI period
    pub rsi_period: usize,
    
    /// MACD parameters (fast, slow, signal)
    pub macd_params: (usize, usize, usize),
    
    /// Bollinger Bands parameters (period, std_dev)
    pub bb_params: (usize, f64),
    
    /// ATR period
    pub atr_period: usize,
    
    /// Stochastic parameters (k_period, d_period)
    pub stoch_params: (usize, usize),
    
    /// Volume-weighted indicators
    pub enable_volume_weighted: bool,
    
    /// Custom indicators
    pub enable_custom: bool,
}

impl Default for IndicatorConfig {
    fn default() -> Self {
        Self {
            ema_periods: vec![9, 21, 50, 100, 200],
            rsi_period: 14,
            macd_params: (12, 26, 9),
            bb_params: (20, 2.0),
            atr_period: 14,
            stoch_params: (14, 3),
            enable_volume_weighted: true,
            enable_custom: true,
        }
    }
}

impl TechnicalIndicatorEngine {
    /// Create a new technical indicator engine
    pub fn new() -> Self {
        Self {
            config: IndicatorConfig::default(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(config: IndicatorConfig) -> Self {
        Self { config }
    }
    
    /// Compute all technical indicators
    pub async fn compute_all(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Price-based features
        self.compute_price_features(current, historical, &mut features)?;
        
        // Momentum indicators
        self.compute_momentum_indicators(current, historical, &mut features)?;
        
        // Volatility indicators
        self.compute_volatility_indicators(current, historical, &mut features)?;
        
        // Volume indicators
        if self.config.enable_volume_weighted {
            self.compute_volume_indicators(current, historical, &mut features)?;
        }
        
        // Trend indicators
        self.compute_trend_indicators(current, historical, &mut features)?;
        
        // Custom indicators
        if self.config.enable_custom {
            self.compute_custom_indicators(current, historical, &mut features)?;
        }
        
        Ok(features)
    }
    
    /// Compute price-based features
    fn compute_price_features(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Price ratios
        features.insert("high_low_ratio".to_string(), current.high / current.low);
        features.insert("close_open_ratio".to_string(), current.close / current.open);
        
        // Price position within range
        let range = current.high - current.low;
        if range > 0.0 {
            features.insert(
                "close_position_in_range".to_string(),
                (current.close - current.low) / range,
            );
        }
        
        // Gap features
        if let Some(prev) = historical.last() {
            let gap = (current.open - prev.close) / prev.close;
            features.insert("gap_percentage".to_string(), gap * 100.0);
            features.insert("gap_filled".to_string(), 
                if (gap > 0.0 && current.low <= prev.close) || 
                   (gap < 0.0 && current.high >= prev.close) { 1.0 } else { 0.0 }
            );
        }
        
        // Price acceleration
        if historical.len() >= 3 {
            let prices: Vec<f64> = historical.iter()
                .rev()
                .take(3)
                .map(|d| d.close)
                .collect();
            
            let velocity1 = prices[0] - prices[1];
            let velocity2 = prices[1] - prices[2];
            let acceleration = velocity1 - velocity2;
            
            features.insert("price_acceleration".to_string(), acceleration);
            features.insert("price_jerk".to_string(), 
                if historical.len() >= 4 {
                    let prev_acc = velocity2 - (prices[2] - historical[historical.len() - 4].close);
                    acceleration - prev_acc
                } else { 0.0 }
            );
        }
        
        Ok(())
    }
    
    /// Compute momentum indicators
    fn compute_momentum_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        if historical.len() < self.config.rsi_period {
            return Ok(());
        }
        
        // RSI
        let rsi = self.calculate_rsi(historical, self.config.rsi_period)?;
        features.insert("rsi".to_string(), rsi);
        features.insert("rsi_oversold".to_string(), if rsi < 30.0 { 1.0 } else { 0.0 });
        features.insert("rsi_overbought".to_string(), if rsi > 70.0 { 1.0 } else { 0.0 });
        
        // Rate of Change (ROC)
        for period in &[5, 10, 20] {
            if historical.len() > *period {
                let past_price = historical[historical.len() - period].close;
                let roc = ((current.close - past_price) / past_price) * 100.0;
                features.insert(format!("roc_{}", period), roc);
            }
        }
        
        // Williams %R
        let (highest_high, lowest_low) = self.get_high_low_range(historical, 14)?;
        let williams_r = ((highest_high - current.close) / (highest_high - lowest_low)) * -100.0;
        features.insert("williams_r".to_string(), williams_r);
        
        // Commodity Channel Index (CCI)
        let cci = self.calculate_cci(current, historical, 20)?;
        features.insert("cci".to_string(), cci);
        features.insert("cci_oversold".to_string(), if cci < -100.0 { 1.0 } else { 0.0 });
        features.insert("cci_overbought".to_string(), if cci > 100.0 { 1.0 } else { 0.0 });
        
        Ok(())
    }
    
    /// Compute volatility indicators
    fn compute_volatility_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // ATR (Average True Range)
        if historical.len() >= self.config.atr_period {
            let atr = self.calculate_atr(historical, self.config.atr_period)?;
            features.insert("atr".to_string(), atr);
            features.insert("atr_percentage".to_string(), (atr / current.close) * 100.0);
        }
        
        // Bollinger Bands
        let (period, std_dev_multiplier) = self.config.bb_params;
        if historical.len() >= period {
            let (middle, upper, lower) = self.calculate_bollinger_bands(
                historical,
                period,
                std_dev_multiplier,
            )?;
            
            features.insert("bb_middle".to_string(), middle);
            features.insert("bb_upper".to_string(), upper);
            features.insert("bb_lower".to_string(), lower);
            features.insert("bb_width".to_string(), upper - lower);
            features.insert("bb_width_ratio".to_string(), (upper - lower) / middle);
            
            // Price position relative to bands
            features.insert("bb_position".to_string(), 
                (current.close - lower) / (upper - lower)
            );
        }
        
        // Historical Volatility
        for period in &[10, 20, 30] {
            if historical.len() > *period {
                let volatility = self.calculate_historical_volatility(historical, *period)?;
                features.insert(format!("volatility_{}", period), volatility);
            }
        }
        
        // Parkinson volatility (using high-low range)
        if historical.len() >= 20 {
            let parkinson_vol = self.calculate_parkinson_volatility(historical, 20)?;
            features.insert("parkinson_volatility".to_string(), parkinson_vol);
        }
        
        Ok(())
    }
    
    /// Compute volume indicators
    fn compute_volume_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Volume rate of change
        if historical.len() >= 10 {
            let past_volume = historical[historical.len() - 10].volume;
            if past_volume > 0.0 {
                let volume_roc = ((current.volume - past_volume) / past_volume) * 100.0;
                features.insert("volume_roc".to_string(), volume_roc);
            }
        }
        
        // On-Balance Volume (OBV) trend
        let obv_trend = self.calculate_obv_trend(current, historical, 20)?;
        features.insert("obv_trend".to_string(), obv_trend);
        
        // Volume-Weighted Average Price (VWAP)
        let vwap = self.calculate_vwap(historical, 20)?;
        features.insert("vwap".to_string(), vwap);
        features.insert("price_to_vwap_ratio".to_string(), current.close / vwap);
        
        // Money Flow Index (MFI)
        if historical.len() >= 14 {
            let mfi = self.calculate_mfi(historical, 14)?;
            features.insert("mfi".to_string(), mfi);
            features.insert("mfi_oversold".to_string(), if mfi < 20.0 { 1.0 } else { 0.0 });
            features.insert("mfi_overbought".to_string(), if mfi > 80.0 { 1.0 } else { 0.0 });
        }
        
        // Accumulation/Distribution Line slope
        let ad_slope = self.calculate_ad_line_slope(historical, 20)?;
        features.insert("ad_line_slope".to_string(), ad_slope);
        
        Ok(())
    }
    
    /// Compute trend indicators
    fn compute_trend_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Multiple EMAs
        for period in &self.config.ema_periods {
            if historical.len() >= *period {
                let ema = self.calculate_ema(historical, *period)?;
                features.insert(format!("ema_{}", period), ema);
                features.insert(
                    format!("price_to_ema_{}_ratio", period),
                    current.close / ema,
                );
            }
        }
        
        // MACD
        let (fast, slow, signal) = self.config.macd_params;
        if historical.len() >= slow {
            let (macd_line, signal_line, histogram) = 
                self.calculate_macd(historical, fast, slow, signal)?;
            
            features.insert("macd_line".to_string(), macd_line);
            features.insert("macd_signal".to_string(), signal_line);
            features.insert("macd_histogram".to_string(), histogram);
            features.insert("macd_crossover".to_string(), 
                if histogram.signum() != self.get_prev_macd_histogram_sign(historical)? {
                    histogram.signum()
                } else { 0.0 }
            );
        }
        
        // ADX (Average Directional Index)
        if historical.len() >= 14 {
            let adx = self.calculate_adx(historical, 14)?;
            features.insert("adx".to_string(), adx);
            features.insert("trending_market".to_string(), if adx > 25.0 { 1.0 } else { 0.0 });
        }
        
        // Ichimoku Cloud components
        if historical.len() >= 52 {
            let ichimoku = self.calculate_ichimoku(current, historical)?;
            features.extend(ichimoku);
        }
        
        Ok(())
    }
    
    /// Compute custom advanced indicators
    fn compute_custom_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        // Heikin-Ashi transformation
        if let Some(prev) = historical.last() {
            let ha_close = (current.open + current.high + current.low + current.close) / 4.0;
            let ha_open = (prev.open + prev.close) / 2.0;
            let ha_high = current.high.max(ha_open).max(ha_close);
            let ha_low = current.low.min(ha_open).min(ha_close);
            
            features.insert("ha_body_size".to_string(), (ha_close - ha_open).abs());
            features.insert("ha_upper_shadow".to_string(), ha_high - ha_close.max(ha_open));
            features.insert("ha_lower_shadow".to_string(), ha_close.min(ha_open) - ha_low);
            features.insert("ha_trend".to_string(), (ha_close - ha_open).signum());
        }
        
        // Market Profile Value Area
        if historical.len() >= 20 {
            let (vah, val, poc) = self.calculate_value_area(historical, 20)?;
            features.insert("value_area_high".to_string(), vah);
            features.insert("value_area_low".to_string(), val);
            features.insert("point_of_control".to_string(), poc);
            features.insert("price_in_value_area".to_string(), 
                if current.close >= val && current.close <= vah { 1.0 } else { 0.0 }
            );
        }
        
        // Fibonacci retracement levels
        if historical.len() >= 100 {
            let (high, low) = self.get_high_low_range(historical, 100)?;
            let range = high - low;
            
            let fib_levels = vec![
                ("fib_0", low),
                ("fib_236", low + range * 0.236),
                ("fib_382", low + range * 0.382),
                ("fib_500", low + range * 0.500),
                ("fib_618", low + range * 0.618),
                ("fib_786", low + range * 0.786),
                ("fib_100", high),
            ];
            
            for (name, level) in fib_levels {
                features.insert(format!("{}_level", name), level);
                features.insert(format!("{}_distance", name), 
                    ((current.close - level) / level * 100.0).abs()
                );
            }
        }
        
        // Pivot points
        if let Some(prev) = historical.last() {
            let pivot = (prev.high + prev.low + prev.close) / 3.0;
            let r1 = 2.0 * pivot - prev.low;
            let s1 = 2.0 * pivot - prev.high;
            let r2 = pivot + (prev.high - prev.low);
            let s2 = pivot - (prev.high - prev.low);
            
            features.insert("pivot_point".to_string(), pivot);
            features.insert("resistance_1".to_string(), r1);
            features.insert("resistance_2".to_string(), r2);
            features.insert("support_1".to_string(), s1);
            features.insert("support_2".to_string(), s2);
        }
        
        // Elliott Wave Pattern Detection
        if historical.len() >= 240 {  // Need sufficient data for wave analysis
            let elliott_features = self.detect_elliott_waves(current, historical)?;
            features.extend(elliott_features);
        }
        
        // Harmonic Pattern Recognition
        if historical.len() >= 100 {
            let harmonic_features = self.detect_harmonic_patterns(current, historical)?;
            features.extend(harmonic_features);
        }
        
        Ok(())
    }
    
    // Helper calculation methods
    
    fn calculate_rsi(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.len() < period + 1 {
            return Ok(50.0); // Neutral RSI
        }
        
        let mut gains = 0.0;
        let mut losses = 0.0;
        
        for i in (data.len() - period)..data.len() {
            let change = data[i].close - data[i - 1].close;
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }
        
        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;
        
        if avg_loss == 0.0 {
            return Ok(100.0);
        }
        
        let rs = avg_gain / avg_loss;
        Ok(100.0 - (100.0 / (1.0 + rs)))
    }
    
    fn calculate_ema(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.is_empty() {
            return Err(anyhow::anyhow!("No data for EMA calculation"));
        }
        
        let alpha = 2.0 / (period as f64 + 1.0);
        let mut ema = data[0].close;
        
        for d in data.iter().skip(1) {
            ema = alpha * d.close + (1.0 - alpha) * ema;
        }
        
        Ok(ema)
    }
    
    fn calculate_atr(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.len() < period + 1 {
            return Err(anyhow::anyhow!("Insufficient data for ATR"));
        }
        
        let mut true_ranges = Vec::new();
        
        for i in 1..data.len() {
            let high_low = data[i].high - data[i].low;
            let high_close = (data[i].high - data[i - 1].close).abs();
            let low_close = (data[i].low - data[i - 1].close).abs();
            
            true_ranges.push(high_low.max(high_close).max(low_close));
        }
        
        let recent_trs: Vec<f64> = true_ranges.iter()
            .rev()
            .take(period)
            .cloned()
            .collect();
        
        Ok(recent_trs.iter().sum::<f64>() / period as f64)
    }
    
    fn calculate_bollinger_bands(
        &self,
        data: &[TimeSeriesData],
        period: usize,
        std_dev: f64,
    ) -> Result<(f64, f64, f64)> {
        let closes: Vec<f64> = data.iter()
            .rev()
            .take(period)
            .map(|d| d.close)
            .collect();
        
        let mean = closes.iter().sum::<f64>() / period as f64;
        let variance = closes.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / period as f64;
        let std_deviation = variance.sqrt();
        
        Ok((
            mean,
            mean + std_dev * std_deviation,
            mean - std_dev * std_deviation,
        ))
    }
    
    fn get_high_low_range(&self, data: &[TimeSeriesData], period: usize) -> Result<(f64, f64)> {
        let recent_data: Vec<&TimeSeriesData> = data.iter()
            .rev()
            .take(period)
            .collect();
        
        let highest = recent_data.iter()
            .map(|d| d.high)
            .fold(f64::MIN, f64::max);
        
        let lowest = recent_data.iter()
            .map(|d| d.low)
            .fold(f64::MAX, f64::min);
        
        Ok((highest, lowest))
    }
    
    fn calculate_cci(&self, current: &TimeSeriesData, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let typical_prices: Vec<f64> = data.iter()
            .rev()
            .take(period - 1)
            .map(|d| (d.high + d.low + d.close) / 3.0)
            .chain(std::iter::once((current.high + current.low + current.close) / 3.0))
            .collect();
        
        let mean = typical_prices.iter().sum::<f64>() / period as f64;
        let mean_deviation = typical_prices.iter()
            .map(|&tp| (tp - mean).abs())
            .sum::<f64>() / period as f64;
        
        if mean_deviation == 0.0 {
            return Ok(0.0);
        }
        
        Ok((typical_prices.last().unwrap() - mean) / (0.015 * mean_deviation))
    }
    
    fn calculate_historical_volatility(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let returns: Vec<f64> = data.windows(2)
            .rev()
            .take(period)
            .map(|w| (w[1].close / w[0].close).ln())
            .collect();
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|&r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        Ok(variance.sqrt() * (252.0_f64).sqrt() * 100.0) // Annualized
    }
    
    fn calculate_parkinson_volatility(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let sum_sq_log_hl: f64 = data.iter()
            .rev()
            .take(period)
            .map(|d| ((d.high / d.low).ln()).powi(2))
            .sum();
        
        let factor = 1.0 / (4.0 * (2.0_f64).ln());
        Ok((factor * sum_sq_log_hl / period as f64).sqrt() * (252.0_f64).sqrt() * 100.0)
    }
    
    fn calculate_obv_trend(&self, current: &TimeSeriesData, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut obv = 0.0;
        let mut obv_values = Vec::new();
        
        for i in 1..data.len().min(period) {
            if data[i].close > data[i - 1].close {
                obv += data[i].volume;
            } else if data[i].close < data[i - 1].close {
                obv -= data[i].volume;
            }
            obv_values.push(obv);
        }
        
        // Add current
        if let Some(last) = data.last() {
            if current.close > last.close {
                obv += current.volume;
            } else if current.close < last.close {
                obv -= current.volume;
            }
            obv_values.push(obv);
        }
        
        // Calculate trend using linear regression
        if obv_values.len() < 2 {
            return Ok(0.0);
        }
        
        let n = obv_values.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = obv_values.iter().sum::<f64>() / n;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for (i, &y) in obv_values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }
        
        if denominator == 0.0 {
            return Ok(0.0);
        }
        
        Ok(numerator / denominator)
    }
    
    fn calculate_vwap(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let recent: Vec<&TimeSeriesData> = data.iter()
            .rev()
            .take(period)
            .collect();
        
        let total_value: f64 = recent.iter()
            .map(|d| ((d.high + d.low + d.close) / 3.0) * d.volume)
            .sum();
        
        let total_volume: f64 = recent.iter()
            .map(|d| d.volume)
            .sum();
        
        if total_volume == 0.0 {
            return Ok(recent.last().map(|d| d.close).unwrap_or(0.0));
        }
        
        Ok(total_value / total_volume)
    }
    
    fn calculate_mfi(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut positive_flow = 0.0;
        let mut negative_flow = 0.0;
        
        for i in (data.len() - period)..data.len() {
            if i == 0 { continue; }
            
            let typical_price = (data[i].high + data[i].low + data[i].close) / 3.0;
            let prev_typical = (data[i - 1].high + data[i - 1].low + data[i - 1].close) / 3.0;
            let money_flow = typical_price * data[i].volume;
            
            if typical_price > prev_typical {
                positive_flow += money_flow;
            } else if typical_price < prev_typical {
                negative_flow += money_flow;
            }
        }
        
        if negative_flow == 0.0 {
            return Ok(100.0);
        }
        
        let money_ratio = positive_flow / negative_flow;
        Ok(100.0 - (100.0 / (1.0 + money_ratio)))
    }
    
    fn calculate_ad_line_slope(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut ad_values = Vec::new();
        let mut ad = 0.0;
        
        for d in data.iter().rev().take(period) {
            let money_flow_multiplier = if d.high != d.low {
                ((d.close - d.low) - (d.high - d.close)) / (d.high - d.low)
            } else {
                0.0
            };
            
            let money_flow_volume = money_flow_multiplier * d.volume;
            ad += money_flow_volume;
            ad_values.push(ad);
        }
        
        // Linear regression for slope
        if ad_values.len() < 2 {
            return Ok(0.0);
        }
        
        let n = ad_values.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = ad_values.iter().sum::<f64>() / n;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for (i, &y) in ad_values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }
        
        if denominator == 0.0 {
            return Ok(0.0);
        }
        
        Ok(numerator / denominator)
    }
    
    fn calculate_macd(
        &self,
        data: &[TimeSeriesData],
        fast: usize,
        slow: usize,
        signal: usize,
    ) -> Result<(f64, f64, f64)> {
        let fast_ema = self.calculate_ema(data, fast)?;
        let slow_ema = self.calculate_ema(data, slow)?;
        let macd_line = fast_ema - slow_ema;
        
        // For signal line, we need MACD history
        // Simplified: using current MACD as signal
        let signal_line = macd_line * 0.9; // Approximation
        let histogram = macd_line - signal_line;
        
        Ok((macd_line, signal_line, histogram))
    }
    
    fn get_prev_macd_histogram_sign(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 2 {
            return Ok(0.0);
        }
        
        let (fast, slow, signal) = self.config.macd_params;
        let prev_data = &data[..data.len() - 1];
        let (_, _, histogram) = self.calculate_macd(prev_data, fast, slow, signal)?;
        
        Ok(histogram.signum())
    }
    
    fn calculate_adx(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.len() < period + 1 {
            return Ok(0.0);
        }
        
        let mut plus_dm_sum = 0.0;
        let mut minus_dm_sum = 0.0;
        let mut tr_sum = 0.0;
        
        for i in (data.len() - period)..data.len() {
            if i == 0 { continue; }
            
            let high_diff = data[i].high - data[i - 1].high;
            let low_diff = data[i - 1].low - data[i].low;
            
            let plus_dm = if high_diff > low_diff && high_diff > 0.0 { high_diff } else { 0.0 };
            let minus_dm = if low_diff > high_diff && low_diff > 0.0 { low_diff } else { 0.0 };
            
            let tr = (data[i].high - data[i].low)
                .max((data[i].high - data[i - 1].close).abs())
                .max((data[i].low - data[i - 1].close).abs());
            
            plus_dm_sum += plus_dm;
            minus_dm_sum += minus_dm;
            tr_sum += tr;
        }
        
        if tr_sum == 0.0 {
            return Ok(0.0);
        }
        
        let plus_di = (plus_dm_sum / tr_sum) * 100.0;
        let minus_di = (minus_dm_sum / tr_sum) * 100.0;
        
        let di_sum = plus_di + minus_di;
        if di_sum == 0.0 {
            return Ok(0.0);
        }
        
        let dx = ((plus_di - minus_di).abs() / di_sum) * 100.0;
        Ok(dx) // Simplified ADX (should be smoothed)
    }
    
    fn calculate_ichimoku(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Tenkan-sen (Conversion Line): (9-period high + 9-period low) / 2
        let (high_9, low_9) = self.get_high_low_range(data, 9)?;
        let tenkan = (high_9 + low_9) / 2.0;
        features.insert("ichimoku_tenkan".to_string(), tenkan);
        
        // Kijun-sen (Base Line): (26-period high + 26-period low) / 2
        let (high_26, low_26) = self.get_high_low_range(data, 26)?;
        let kijun = (high_26 + low_26) / 2.0;
        features.insert("ichimoku_kijun".to_string(), kijun);
        
        // Senkou Span A (Leading Span A): (Tenkan + Kijun) / 2
        let senkou_a = (tenkan + kijun) / 2.0;
        features.insert("ichimoku_senkou_a".to_string(), senkou_a);
        
        // Senkou Span B (Leading Span B): (52-period high + 52-period low) / 2
        let (high_52, low_52) = self.get_high_low_range(data, 52)?;
        let senkou_b = (high_52 + low_52) / 2.0;
        features.insert("ichimoku_senkou_b".to_string(), senkou_b);
        
        // Cloud thickness
        features.insert("ichimoku_cloud_thickness".to_string(), (senkou_a - senkou_b).abs());
        
        // Price position relative to cloud
        let cloud_top = senkou_a.max(senkou_b);
        let cloud_bottom = senkou_a.min(senkou_b);
        
        if current.close > cloud_top {
            features.insert("ichimoku_position".to_string(), 1.0); // Above cloud
        } else if current.close < cloud_bottom {
            features.insert("ichimoku_position".to_string(), -1.0); // Below cloud
        } else {
            features.insert("ichimoku_position".to_string(), 0.0); // Inside cloud
        }
        
        // TK Cross
        features.insert("ichimoku_tk_cross".to_string(), (tenkan - kijun).signum());
        
        Ok(features)
    }
    
    fn calculate_value_area(
        &self,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<(f64, f64, f64)> {
        // Simplified market profile calculation
        let recent: Vec<&TimeSeriesData> = data.iter()
            .rev()
            .take(period)
            .collect();
        
        // Create price histogram
        let mut price_volumes: Vec<(f64, f64)> = Vec::new();
        
        for d in &recent {
            let typical_price = (d.high + d.low + d.close) / 3.0;
            price_volumes.push((typical_price, d.volume));
        }
        
        // Sort by price
        price_volumes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        
        // Find point of control (price with highest volume)
        let poc = price_volumes.iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|pv| pv.0)
            .unwrap_or(data.last().map(|d| d.close).unwrap_or(0.0));
        
        // Calculate value area (70% of volume)
        let total_volume: f64 = price_volumes.iter().map(|pv| pv.1).sum();
        let target_volume = total_volume * 0.7;
        
        let mut accumulated_volume = 0.0;
        let mut vah = poc;
        let mut val = poc;
        
        for (price, volume) in &price_volumes {
            accumulated_volume += volume;
            if accumulated_volume >= target_volume * 0.15 && *price < poc {
                val = *price;
            }
            if accumulated_volume >= target_volume * 0.85 {
                vah = *price;
                break;
            }
        }
        
        Ok((vah, val, poc))
    }
    
    /// Detect Elliott Wave patterns in price data
    fn detect_elliott_waves(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Find local peaks and troughs for wave identification
        let swings = self.find_swing_points(historical, 13)?; // 13-period swing detection
        
        if swings.len() >= 5 {
            // Analyze the last 5 swings for Elliott Wave patterns
            let recent_swings: Vec<_> = swings.iter().rev().take(5).rev().collect();
            
            // Check for impulsive wave pattern (5 waves)
            if let Some(wave_pattern) = self.analyze_impulsive_waves(&recent_swings) {
                features.insert("elliott_wave_type".to_string(), 1.0); // Impulsive
                features.insert("elliott_wave_position".to_string(), wave_pattern.current_wave as f64);
                features.insert("elliott_wave_strength".to_string(), wave_pattern.strength);
                features.insert("elliott_wave_completion".to_string(), wave_pattern.completion_ratio);
                
                // Fibonacci relationships between waves
                if wave_pattern.current_wave >= 3 {
                    let wave1_size = (recent_swings[1].price - recent_swings[0].price).abs();
                    let wave3_size = (recent_swings[3].price - recent_swings[2].price).abs();
                    features.insert("elliott_wave3_to_wave1_ratio".to_string(), wave3_size / wave1_size);
                }
                
                // Project next wave targets
                if let Some(target) = self.project_elliott_target(&wave_pattern, current.close) {
                    features.insert("elliott_wave_target".to_string(), target);
                    features.insert("elliott_wave_target_distance".to_string(), 
                        ((target - current.close) / current.close * 100.0).abs());
                }
            }
            
            // Check for corrective wave pattern (3 waves)
            else if let Some(corrective_pattern) = self.analyze_corrective_waves(&recent_swings) {
                features.insert("elliott_wave_type".to_string(), -1.0); // Corrective
                features.insert("elliott_wave_position".to_string(), corrective_pattern.current_wave as f64);
                features.insert("elliott_corrective_type".to_string(), corrective_pattern.pattern_type as f64);
                features.insert("elliott_wave_completion".to_string(), corrective_pattern.completion_ratio);
            }
        }
        
        // Wave degree analysis (multiple timeframes)
        let wave_degrees = vec![21, 55, 89, 144]; // Fibonacci numbers for different wave degrees
        for degree in wave_degrees {
            if historical.len() >= degree * 2 {
                let degree_swings = self.find_swing_points(historical, degree)?;
                if degree_swings.len() >= 2 {
                    let trend_direction = (degree_swings.last().unwrap().price - 
                                         degree_swings.first().unwrap().price).signum();
                    features.insert(format!("elliott_degree_{}_trend", degree), trend_direction);
                }
            }
        }
        
        Ok(features)
    }
    
    /// Detect Harmonic patterns in price data
    fn detect_harmonic_patterns(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Find swing points for pattern detection
        let swings = self.find_swing_points(historical, 8)?;
        
        if swings.len() >= 5 {
            // Get the last 5 swing points (X, A, B, C, D)
            let points: Vec<_> = swings.iter().rev().take(5).rev().collect();
            let x = points[0];
            let a = points[1];
            let b = points[2];
            let c = points[3];
            let d = points[4];
            
            // Calculate retracement and extension ratios
            let xa_move = a.price - x.price;
            let ab_move = b.price - a.price;
            let bc_move = c.price - b.price;
            let cd_move = d.price - c.price;
            
            let ab_xa_ratio = ab_move.abs() / xa_move.abs();
            let bc_ab_ratio = bc_move.abs() / ab_move.abs();
            let cd_bc_ratio = cd_move.abs() / bc_move.abs();
            let ad_xa_ratio = (d.price - a.price).abs() / xa_move.abs();
            
            // Gartley Pattern (0.618 AB/XA, 0.382-0.886 BC/AB, 1.13-1.618 CD/BC, 0.786 AD/XA)
            if (ab_xa_ratio - 0.618).abs() < 0.05 &&
               bc_ab_ratio >= 0.382 && bc_ab_ratio <= 0.886 &&
               cd_bc_ratio >= 1.13 && cd_bc_ratio <= 1.618 &&
               (ad_xa_ratio - 0.786).abs() < 0.05 {
                features.insert("harmonic_pattern_gartley".to_string(), xa_move.signum());
                features.insert("harmonic_gartley_completion".to_string(), 1.0);
                features.insert("harmonic_gartley_target".to_string(), 
                    d.price - cd_move * 0.618); // 61.8% retracement target
            }
            
            // Bat Pattern (0.382-0.5 AB/XA, 0.382-0.886 BC/AB, 1.618-2.618 CD/BC, 0.886 AD/XA)
            if ab_xa_ratio >= 0.382 && ab_xa_ratio <= 0.5 &&
               bc_ab_ratio >= 0.382 && bc_ab_ratio <= 0.886 &&
               cd_bc_ratio >= 1.618 && cd_bc_ratio <= 2.618 &&
               (ad_xa_ratio - 0.886).abs() < 0.05 {
                features.insert("harmonic_pattern_bat".to_string(), xa_move.signum());
                features.insert("harmonic_bat_completion".to_string(), 1.0);
            }
            
            // Butterfly Pattern (0.786 AB/XA, 0.382-0.886 BC/AB, 1.618-2.24 CD/BC, 1.27-1.41 AD/XA)
            if (ab_xa_ratio - 0.786).abs() < 0.05 &&
               bc_ab_ratio >= 0.382 && bc_ab_ratio <= 0.886 &&
               cd_bc_ratio >= 1.618 && cd_bc_ratio <= 2.24 &&
               ad_xa_ratio >= 1.27 && ad_xa_ratio <= 1.41 {
                features.insert("harmonic_pattern_butterfly".to_string(), xa_move.signum());
                features.insert("harmonic_butterfly_completion".to_string(), 1.0);
            }
            
            // Crab Pattern (0.382-0.618 AB/XA, 0.382-0.886 BC/AB, 2.618-3.618 CD/BC, 1.618 AD/XA)
            if ab_xa_ratio >= 0.382 && ab_xa_ratio <= 0.618 &&
               bc_ab_ratio >= 0.382 && bc_ab_ratio <= 0.886 &&
               cd_bc_ratio >= 2.618 && cd_bc_ratio <= 3.618 &&
               (ad_xa_ratio - 1.618).abs() < 0.1 {
                features.insert("harmonic_pattern_crab".to_string(), xa_move.signum());
                features.insert("harmonic_crab_completion".to_string(), 1.0);
            }
            
            // General harmonic pattern metrics
            features.insert("harmonic_ab_xa_ratio".to_string(), ab_xa_ratio);
            features.insert("harmonic_bc_ab_ratio".to_string(), bc_ab_ratio);
            features.insert("harmonic_cd_bc_ratio".to_string(), cd_bc_ratio);
            features.insert("harmonic_ad_xa_ratio".to_string(), ad_xa_ratio);
            
            // Pattern potential (how close to forming a pattern)
            let pattern_scores = vec![
                self.calculate_pattern_score(ab_xa_ratio, bc_ab_ratio, cd_bc_ratio, ad_xa_ratio, 
                    &[0.618, 0.382, 1.13, 0.786], &[0.05, 0.5, 0.5, 0.05]), // Gartley
                self.calculate_pattern_score(ab_xa_ratio, bc_ab_ratio, cd_bc_ratio, ad_xa_ratio,
                    &[0.441, 0.5, 2.0, 0.886], &[0.12, 0.5, 0.6, 0.05]), // Bat
                self.calculate_pattern_score(ab_xa_ratio, bc_ab_ratio, cd_bc_ratio, ad_xa_ratio,
                    &[0.786, 0.5, 1.93, 1.34], &[0.05, 0.5, 0.3, 0.07]), // Butterfly
                self.calculate_pattern_score(ab_xa_ratio, bc_ab_ratio, cd_bc_ratio, ad_xa_ratio,
                    &[0.5, 0.5, 3.14, 1.618], &[0.12, 0.5, 0.5, 0.1]), // Crab
            ];
            
            let max_score = pattern_scores.iter().cloned().fold(f64::MIN, f64::max);
            features.insert("harmonic_pattern_potential".to_string(), max_score);
        }
        
        // ABCD Pattern (simpler pattern)
        if swings.len() >= 4 {
            let points: Vec<_> = swings.iter().rev().take(4).rev().collect();
            let a = points[0];
            let b = points[1];
            let c = points[2];
            let d = points[3];
            
            let ab_move = b.price - a.price;
            let bc_move = c.price - b.price;
            let cd_move = d.price - c.price;
            
            let bc_ab_ratio = bc_move.abs() / ab_move.abs();
            let cd_ab_ratio = cd_move.abs() / ab_move.abs();
            
            // Classic ABCD: BC = 0.618 or 0.786 of AB, CD = 1.27 or 1.618 of BC
            if (bc_ab_ratio >= 0.618 && bc_ab_ratio <= 0.786) &&
               (cd_ab_ratio >= 1.27 && cd_ab_ratio <= 1.618) {
                features.insert("harmonic_pattern_abcd".to_string(), ab_move.signum());
                features.insert("harmonic_abcd_completion".to_string(), 1.0);
            }
        }
        
        Ok(features)
    }
    
    /// Find swing highs and lows in the data
    fn find_swing_points(&self, data: &[TimeSeriesData], period: usize) -> Result<Vec<SwingPoint>> {
        let mut swings = Vec::new();
        
        if data.len() < period * 2 + 1 {
            return Ok(swings);
        }
        
        for i in period..(data.len() - period) {
            let is_swing_high = (0..period).all(|j| data[i].high >= data[i - j - 1].high) &&
                               (0..period).all(|j| data[i].high >= data[i + j + 1].high);
            
            let is_swing_low = (0..period).all(|j| data[i].low <= data[i - j - 1].low) &&
                              (0..period).all(|j| data[i].low <= data[i + j + 1].low);
            
            if is_swing_high {
                swings.push(SwingPoint {
                    index: i,
                    price: data[i].high,
                    swing_type: SwingType::High,
                    timestamp: data[i].timestamp,
                });
            } else if is_swing_low {
                swings.push(SwingPoint {
                    index: i,
                    price: data[i].low,
                    swing_type: SwingType::Low,
                    timestamp: data[i].timestamp,
                });
            }
        }
        
        Ok(swings)
    }
    
    /// Analyze potential impulsive wave pattern
    fn analyze_impulsive_waves(&self, swings: &[&SwingPoint]) -> Option<ElliottWavePattern> {
        if swings.len() < 5 {
            return None;
        }
        
        // Check if pattern alternates properly (up-down-up-down-up or vice versa)
        let is_uptrend = swings[1].price > swings[0].price;
        
        for i in 0..4 {
            let expected_higher = if i % 2 == 0 { is_uptrend } else { !is_uptrend };
            let actual_higher = swings[i + 1].price > swings[i].price;
            
            if expected_higher != actual_higher {
                return None;
            }
        }
        
        // Verify wave 3 is not the shortest
        let wave1_size = (swings[1].price - swings[0].price).abs();
        let wave3_size = (swings[3].price - swings[2].price).abs();
        let wave5_size = (swings[4].price - swings[3].price).abs();
        
        if wave3_size < wave1_size && wave3_size < wave5_size {
            return None;
        }
        
        // Calculate pattern strength based on Fibonacci relationships
        let mut strength = 0.0;
        
        // Wave 2 typically retraces 50-61.8% of wave 1
        let wave2_retrace = (swings[2].price - swings[1].price).abs() / wave1_size;
        if wave2_retrace >= 0.5 && wave2_retrace <= 0.618 {
            strength += 0.25;
        }
        
        // Wave 3 often extends to 161.8% of wave 1
        let wave3_extension = wave3_size / wave1_size;
        if wave3_extension >= 1.5 && wave3_extension <= 1.7 {
            strength += 0.25;
        }
        
        // Wave 4 typically retraces 38.2-50% of wave 3
        if swings.len() > 3 {
            let wave4_retrace = (swings[3].price - swings[2].price).abs() / wave3_size;
            if wave4_retrace >= 0.382 && wave4_retrace <= 0.5 {
                strength += 0.25;
            }
        }
        
        // Wave 5 often equals wave 1 or extends to 61.8% of waves 1-3
        if wave5_size >= wave1_size * 0.9 && wave5_size <= wave1_size * 1.1 {
            strength += 0.25;
        }
        
        Some(ElliottWavePattern {
            pattern_type: WavePatternType::Impulsive,
            current_wave: 5,
            strength,
            completion_ratio: 1.0,
            direction: if is_uptrend { 1.0 } else { -1.0 },
        })
    }
    
    /// Analyze potential corrective wave pattern
    fn analyze_corrective_waves(&self, swings: &[&SwingPoint]) -> Option<CorrectiveWavePattern> {
        if swings.len() < 3 {
            return None;
        }
        
        let wave_a_size = (swings[1].price - swings[0].price).abs();
        let wave_b_size = (swings[2].price - swings[1].price).abs();
        let wave_b_retrace = wave_b_size / wave_a_size;
        
        // Determine corrective pattern type
        let pattern_type = if wave_b_retrace <= 0.618 {
            CorrectivePatternType::Zigzag
        } else if wave_b_retrace >= 0.9 && wave_b_retrace <= 1.0 {
            CorrectivePatternType::Flat
        } else if wave_b_retrace > 1.0 {
            CorrectivePatternType::Irregular
        } else {
            CorrectivePatternType::Complex
        };
        
        let completion_ratio = if swings.len() >= 3 { 1.0 } else { 0.67 };
        
        Some(CorrectiveWavePattern {
            pattern_type,
            current_wave: swings.len().min(3),
            completion_ratio,
        })
    }
    
    /// Project Elliott Wave targets
    fn project_elliott_target(&self, pattern: &ElliottWavePattern, current_price: f64) -> Option<f64> {
        // Simplified target projection based on wave relationships
        match pattern.current_wave {
            3 => Some(current_price * 1.618), // Wave 3 often extends to 161.8%
            5 => Some(current_price * 1.0),   // Wave 5 often equals wave 1
            _ => None,
        }
    }
    
    /// Calculate how closely ratios match a harmonic pattern
    fn calculate_pattern_score(
        &self,
        ab_xa: f64,
        bc_ab: f64,
        cd_bc: f64,
        ad_xa: f64,
        targets: &[f64],
        tolerances: &[f64],
    ) -> f64 {
        let ratios = vec![ab_xa, bc_ab, cd_bc, ad_xa];
        let mut score = 0.0;
        
        for i in 0..4 {
            let diff = (ratios[i] - targets[i]).abs();
            if diff <= tolerances[i] {
                score += 0.25 * (1.0 - diff / tolerances[i]);
            }
        }
        
        score
    }
}

/// Swing point structure for pattern detection
#[derive(Debug, Clone)]
struct SwingPoint {
    index: usize,
    price: f64,
    swing_type: SwingType,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq)]
enum SwingType {
    High,
    Low,
}

/// Elliott Wave pattern structure
#[derive(Debug)]
struct ElliottWavePattern {
    pattern_type: WavePatternType,
    current_wave: usize,
    strength: f64,
    completion_ratio: f64,
    direction: f64,
}

#[derive(Debug)]
enum WavePatternType {
    Impulsive,
    Corrective,
}

/// Corrective wave pattern structure
#[derive(Debug)]
struct CorrectiveWavePattern {
    pattern_type: CorrectivePatternType,
    current_wave: usize,
    completion_ratio: f64,
}

#[derive(Debug)]
enum CorrectivePatternType {
    Zigzag = 1,
    Flat = 2,
    Irregular = 3,
    Complex = 4,
}

impl Default for TechnicalIndicatorEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "technical_indicators_tests.rs"]
mod technical_indicators_tests;