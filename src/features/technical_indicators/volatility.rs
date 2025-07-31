//! Volatility indicators implementation
//! 
//! This module implements volatility-based indicators like ATR, Bollinger Bands, 
//! and various volatility measures.

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;
use super::config::IndicatorConfig;

/// Volatility indicators calculator
pub struct VolatilityIndicators<'a> {
    config: &'a IndicatorConfig,
}

impl<'a> VolatilityIndicators<'a> {
    pub fn new(config: &'a IndicatorConfig) -> Self {
        Self { config }
    }
    
    /// Compute all volatility indicators
    pub async fn compute_all(
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
            
            // Bollinger Band squeeze detection
            let bb_squeeze = self.detect_bollinger_squeeze(historical, period, std_dev_multiplier)?;
            features.insert("bb_squeeze".to_string(), if bb_squeeze { 1.0 } else { 0.0 });
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
        
        // Garman-Klass volatility
        if historical.len() >= 20 {
            let gk_vol = self.calculate_garman_klass_volatility(historical, 20)?;
            features.insert("garman_klass_volatility".to_string(), gk_vol);
        }
        
        // Rogers-Satchell volatility
        if historical.len() >= 20 {
            let rs_vol = self.calculate_rogers_satchell_volatility(historical, 20)?;
            features.insert("rogers_satchell_volatility".to_string(), rs_vol);
        }
        
        // Keltner Channels
        if historical.len() >= 20 {
            let (kc_middle, kc_upper, kc_lower) = self.calculate_keltner_channels(historical, 20, 2.0)?;
            features.insert("keltner_middle".to_string(), kc_middle);
            features.insert("keltner_upper".to_string(), kc_upper);
            features.insert("keltner_lower".to_string(), kc_lower);
            features.insert("keltner_position".to_string(), 
                (current.close - kc_lower) / (kc_upper - kc_lower)
            );
        }
        
        // Donchian Channels
        if historical.len() >= 20 {
            let (dc_upper, dc_lower, dc_middle) = self.calculate_donchian_channels(historical, 20)?;
            features.insert("donchian_upper".to_string(), dc_upper);
            features.insert("donchian_lower".to_string(), dc_lower);
            features.insert("donchian_middle".to_string(), dc_middle);
            features.insert("donchian_position".to_string(), 
                (current.close - dc_lower) / (dc_upper - dc_lower)
            );
        }
        
        Ok(())
    }
    
    /// Calculate Average True Range (ATR)
    pub fn calculate_atr(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Bollinger Bands
    pub fn calculate_bollinger_bands(
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
    
    /// Detect Bollinger Band squeeze
    pub fn detect_bollinger_squeeze(
        &self,
        data: &[TimeSeriesData],
        period: usize,
        std_dev: f64,
    ) -> Result<bool> {
        if data.len() < period * 2 {
            return Ok(false);
        }
        
        // Calculate current BB width
        let (_, upper, lower) = self.calculate_bollinger_bands(data, period, std_dev)?;
        let current_width = upper - lower;
        
        // Calculate historical BB widths
        let mut widths = Vec::new();
        for i in period..data.len() {
            let subset = &data[..=i];
            if let Ok((_, u, l)) = self.calculate_bollinger_bands(subset, period, std_dev) {
                widths.push(u - l);
            }
        }
        
        if widths.len() < 20 {
            return Ok(false);
        }
        
        // Check if current width is in lowest 20% of recent widths
        widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let percentile_20 = widths[widths.len() / 5];
        
        Ok(current_width <= percentile_20)
    }
    
    /// Calculate historical volatility
    pub fn calculate_historical_volatility(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Parkinson volatility (high-low estimator)
    pub fn calculate_parkinson_volatility(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let sum_sq_log_hl: f64 = data.iter()
            .rev()
            .take(period)
            .map(|d| ((d.high / d.low).ln()).powi(2))
            .sum();
        
        let factor = 1.0 / (4.0 * (2.0_f64).ln());
        Ok((factor * sum_sq_log_hl / period as f64).sqrt() * (252.0_f64).sqrt() * 100.0)
    }
    
    /// Calculate Garman-Klass volatility
    pub fn calculate_garman_klass_volatility(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut sum = 0.0;
        
        for i in 1..=period.min(data.len() - 1) {
            let idx = data.len() - i;
            let d = &data[idx];
            let prev_d = &data[idx - 1];
            
            let log_hl = (d.high / d.low).ln();
            let log_co = (d.close / d.open).ln();
            let log_oc = (d.open / prev_d.close).ln();
            
            sum += 0.5 * log_hl.powi(2) - (2.0 * (2.0_f64).ln() - 1.0) * log_co.powi(2) + log_oc.powi(2);
        }
        
        Ok((sum / period as f64).sqrt() * (252.0_f64).sqrt() * 100.0)
    }
    
    /// Calculate Rogers-Satchell volatility
    pub fn calculate_rogers_satchell_volatility(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut sum = 0.0;
        
        for i in 0..period.min(data.len()) {
            let idx = data.len() - 1 - i;
            let d = &data[idx];
            
            let log_ho = (d.high / d.open).ln();
            let log_hc = (d.high / d.close).ln();
            let log_lo = (d.low / d.open).ln();
            let log_lc = (d.low / d.close).ln();
            
            sum += log_ho * log_hc + log_lo * log_lc;
        }
        
        Ok((sum / period as f64).sqrt() * (252.0_f64).sqrt() * 100.0)
    }
    
    /// Calculate Keltner Channels
    pub fn calculate_keltner_channels(
        &self,
        data: &[TimeSeriesData],
        period: usize,
        multiplier: f64,
    ) -> Result<(f64, f64, f64)> {
        // Middle line is EMA of typical price
        let typical_prices: Vec<f64> = data.iter()
            .rev()
            .take(period)
            .map(|d| (d.high + d.low + d.close) / 3.0)
            .collect();
        
        let alpha = 2.0 / (period as f64 + 1.0);
        let mut ema = typical_prices[typical_prices.len() - 1];
        
        for &price in typical_prices.iter().rev().skip(1) {
            ema = alpha * price + (1.0 - alpha) * ema;
        }
        
        // ATR for channel width
        let atr = self.calculate_atr(data, period)?;
        
        Ok((
            ema,                           // middle
            ema + multiplier * atr,        // upper
            ema - multiplier * atr,        // lower
        ))
    }
    
    /// Calculate Donchian Channels
    pub fn calculate_donchian_channels(&self, data: &[TimeSeriesData], period: usize) -> Result<(f64, f64, f64)> {
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
        
        let middle = (highest + lowest) / 2.0;
        
        Ok((highest, lowest, middle))
    }
    
    /// Calculate Chaikin Volatility
    pub fn calculate_chaikin_volatility(&self, data: &[TimeSeriesData], period: usize, rate_of_change_period: usize) -> Result<f64> {
        if data.len() < period + rate_of_change_period {
            return Ok(0.0);
        }
        
        // Calculate high-low spreads
        let spreads: Vec<f64> = data.iter()
            .map(|d| d.high - d.low)
            .collect();
        
        // Calculate EMA of spreads
        let alpha = 2.0 / (period as f64 + 1.0);
        let mut ema_current = spreads[spreads.len() - 1];
        let mut ema_past = spreads[spreads.len() - 1 - rate_of_change_period];
        
        for i in 1..period.min(spreads.len()) {
            let idx = spreads.len() - 1 - i;
            ema_current = alpha * spreads[idx] + (1.0 - alpha) * ema_current;
            
            if idx >= rate_of_change_period {
                ema_past = alpha * spreads[idx - rate_of_change_period] + (1.0 - alpha) * ema_past;
            }
        }
        
        if ema_past == 0.0 {
            return Ok(0.0);
        }
        
        Ok(((ema_current - ema_past) / ema_past) * 100.0)
    }
    
    /// Calculate normalized volatility (Z-score)
    pub fn calculate_normalized_volatility(&self, data: &[TimeSeriesData], period: usize, lookback: usize) -> Result<f64> {
        if data.len() < period + lookback {
            return Ok(0.0);
        }
        
        let current_vol = self.calculate_historical_volatility(data, period)?;
        
        // Calculate historical volatilities
        let mut historical_vols = Vec::new();
        for i in period..lookback.min(data.len() - period) {
            let end_idx = data.len() - i;
            let subset = &data[..end_idx];
            if let Ok(vol) = self.calculate_historical_volatility(subset, period) {
                historical_vols.push(vol);
            }
        }
        
        if historical_vols.is_empty() {
            return Ok(0.0);
        }
        
        let mean_vol = historical_vols.iter().sum::<f64>() / historical_vols.len() as f64;
        let vol_variance = historical_vols.iter()
            .map(|&v| (v - mean_vol).powi(2))
            .sum::<f64>() / historical_vols.len() as f64;
        let vol_std = vol_variance.sqrt();
        
        if vol_std == 0.0 {
            return Ok(0.0);
        }
        
        Ok((current_vol - mean_vol) / vol_std)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use chrono::{DateTime, Utc};
    
    fn create_test_data() -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        for i in 0..50 {
            data.push(TimeSeriesData {
                timestamp: DateTime::<Utc>::from_timestamp(1640995200 + i * 60, 0).unwrap(),
                open: 100.0 + (i as f64 * 0.1 * (i as f64 / 5.0).sin()),
                high: 105.0 + (i as f64 * 0.1 * (i as f64 / 5.0).sin()),
                low: 95.0 + (i as f64 * 0.1 * (i as f64 / 5.0).sin()),
                close: 102.0 + (i as f64 * 0.1 * (i as f64 / 5.0).sin()),
                volume: 1000.0 + i as f64 * 10.0,
            });
        }
        data
    }
    
    #[test]
    fn test_atr_calculation() {
        let config = IndicatorConfig::default();
        let volatility = VolatilityIndicators::new(&config);
        let data = create_test_data();
        
        let atr = volatility.calculate_atr(&data, 14).unwrap();
        assert!(atr > 0.0);
    }
    
    #[test]
    fn test_bollinger_bands() {
        let config = IndicatorConfig::default();
        let volatility = VolatilityIndicators::new(&config);
        let data = create_test_data();
        
        let (middle, upper, lower) = volatility.calculate_bollinger_bands(&data, 20, 2.0).unwrap();
        assert!(upper > middle);
        assert!(middle > lower);
    }
    
    #[test]
    fn test_historical_volatility() {
        let config = IndicatorConfig::default();
        let volatility = VolatilityIndicators::new(&config);
        let data = create_test_data();
        
        let vol = volatility.calculate_historical_volatility(&data, 20).unwrap();
        assert!(vol > 0.0);
    }
    
    #[test]
    fn test_parkinson_volatility() {
        let config = IndicatorConfig::default();
        let volatility = VolatilityIndicators::new(&config);
        let data = create_test_data();
        
        let parkinson_vol = volatility.calculate_parkinson_volatility(&data, 20).unwrap();
        assert!(parkinson_vol > 0.0);
    }
    
    #[test]
    fn test_keltner_channels() {
        let config = IndicatorConfig::default();
        let volatility = VolatilityIndicators::new(&config);
        let data = create_test_data();
        
        let (middle, upper, lower) = volatility.calculate_keltner_channels(&data, 20, 2.0).unwrap();
        assert!(upper > middle);
        assert!(middle > lower);
    }
    
    #[test]
    fn test_donchian_channels() {
        let config = IndicatorConfig::default();
        let volatility = VolatilityIndicators::new(&config);
        let data = create_test_data();
        
        let (upper, lower, middle) = volatility.calculate_donchian_channels(&data, 20).unwrap();
        assert!(upper >= lower);
        assert!(middle >= lower && middle <= upper);
    }
}