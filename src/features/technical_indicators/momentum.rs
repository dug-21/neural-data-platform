//! Momentum indicators implementation
//! 
//! This module implements momentum-based indicators like RSI, ROC, Williams %R, CCI, and Stochastic.

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;
use super::config::IndicatorConfig;

/// Momentum indicators calculator
pub struct MomentumIndicators<'a> {
    config: &'a IndicatorConfig,
}

impl<'a> MomentumIndicators<'a> {
    pub fn new(config: &'a IndicatorConfig) -> Self {
        Self { config }
    }
    
    /// Compute all momentum indicators
    pub async fn compute_all(
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
        
        // Stochastic Oscillator
        let (k_period, d_period) = self.config.stoch_params;
        if historical.len() >= k_period {
            let (stoch_k, stoch_d) = self.calculate_stochastic(current, historical, k_period, d_period)?;
            features.insert("stochastic_k".to_string(), stoch_k);
            features.insert("stochastic_d".to_string(), stoch_d);
            features.insert("stochastic_oversold".to_string(), if stoch_k < 20.0 { 1.0 } else { 0.0 });
            features.insert("stochastic_overbought".to_string(), if stoch_k > 80.0 { 1.0 } else { 0.0 });
        }
        
        // Momentum (Price change over period)
        for period in &[10, 14, 20] {
            if historical.len() > *period {
                let momentum = current.close - historical[historical.len() - period].close;
                features.insert(format!("momentum_{}", period), momentum);
            }
        }
        
        // Ultimate Oscillator
        if historical.len() >= 28 {
            let ult_osc = self.calculate_ultimate_oscillator(current, historical)?;
            features.insert("ultimate_oscillator".to_string(), ult_osc);
        }
        
        Ok(())
    }
    
    /// Calculate RSI (Relative Strength Index)
    pub fn calculate_rsi(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Commodity Channel Index (CCI)
    pub fn calculate_cci(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
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
    
    /// Calculate Stochastic Oscillator
    pub fn calculate_stochastic(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        k_period: usize,
        d_period: usize,
    ) -> Result<(f64, f64)> {
        let (highest_high, lowest_low) = self.get_high_low_range(data, k_period)?;
        
        let stoch_k = if highest_high != lowest_low {
            ((current.close - lowest_low) / (highest_high - lowest_low)) * 100.0
        } else {
            50.0
        };
        
        // For %D, we need historical %K values - simplified implementation
        let stoch_d = stoch_k; // Should be SMA of %K over d_period
        
        Ok((stoch_k, stoch_d))
    }
    
    /// Calculate Ultimate Oscillator
    pub fn calculate_ultimate_oscillator(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
    ) -> Result<f64> {
        let periods = [7, 14, 28];
        let mut bp_sums = [0.0; 3];
        let mut tr_sums = [0.0; 3];
        
        for (idx, &period) in periods.iter().enumerate() {
            if data.len() < period + 1 {
                continue;
            }
            
            for i in (data.len() - period)..data.len() {
                if i == 0 { continue; }
                
                let bp = data[i].close - data[i].low.min(data[i - 1].close);
                let tr = data[i].high.max(data[i - 1].close) - data[i].low.min(data[i - 1].close);
                
                bp_sums[idx] += bp;
                tr_sums[idx] += tr;
            }
        }
        
        let mut avg_values = [0.0; 3];
        for i in 0..3 {
            if tr_sums[i] != 0.0 {
                avg_values[i] = bp_sums[i] / tr_sums[i];
            }
        }
        
        let ultimate_oscillator = 100.0 * (
            (4.0 * avg_values[0]) + (2.0 * avg_values[1]) + avg_values[2]
        ) / 7.0;
        
        Ok(ultimate_oscillator)
    }
    
    /// Calculate Awesome Oscillator
    pub fn calculate_awesome_oscillator(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 34 {
            return Ok(0.0);
        }
        
        // Calculate 5-period SMA of midpoint
        let sma5 = self.calculate_midpoint_sma(data, 5)?;
        
        // Calculate 34-period SMA of midpoint
        let sma34 = self.calculate_midpoint_sma(data, 34)?;
        
        Ok(sma5 - sma34)
    }
    
    /// Calculate SMA of midpoints
    fn calculate_midpoint_sma(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.len() < period {
            return Err(anyhow::anyhow!("Insufficient data for midpoint SMA"));
        }
        
        let sum: f64 = data.iter()
            .rev()
            .take(period)
            .map(|d| (d.high + d.low) / 2.0)
            .sum();
        
        Ok(sum / period as f64)
    }
    
    /// Calculate True Strength Index (TSI)
    pub fn calculate_tsi(&self, data: &[TimeSeriesData], first_smooth: usize, second_smooth: usize) -> Result<f64> {
        if data.len() < first_smooth + second_smooth {
            return Ok(0.0);
        }
        
        // Calculate price changes
        let mut price_changes = Vec::new();
        let mut abs_price_changes = Vec::new();
        
        for i in 1..data.len() {
            let change = data[i].close - data[i - 1].close;
            price_changes.push(change);
            abs_price_changes.push(change.abs());
        }
        
        // Double smoothing of price changes
        let smoothed_changes = self.double_smooth(&price_changes, first_smooth, second_smooth)?;
        let smoothed_abs_changes = self.double_smooth(&abs_price_changes, first_smooth, second_smooth)?;
        
        if smoothed_abs_changes == 0.0 {
            return Ok(0.0);
        }
        
        Ok(100.0 * smoothed_changes / smoothed_abs_changes)
    }
    
    /// Double smoothing helper function
    fn double_smooth(&self, values: &[f64], first_period: usize, second_period: usize) -> Result<f64> {
        if values.len() < first_period + second_period {
            return Ok(0.0);
        }
        
        // First smoothing - EMA
        let alpha1 = 2.0 / (first_period as f64 + 1.0);
        let mut first_smooth = values[0];
        
        for &value in values.iter().skip(1) {
            first_smooth = alpha1 * value + (1.0 - alpha1) * first_smooth;
        }
        
        // Second smoothing - EMA of first smoothed values
        let alpha2 = 2.0 / (second_period as f64 + 1.0);
        let second_smooth = first_smooth; // Simplified - should maintain history
        
        Ok(second_smooth)
    }
    
    /// Get high and low range for a given period
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
    
    /// Calculate Money Flow Index (MFI) - Volume-weighted RSI
    pub fn calculate_mfi(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut positive_flow = 0.0;
        let mut negative_flow = 0.0;
        
        for i in (data.len() - period)..data.len() {
            if i == 0 { continue; }
            
            let typical_price = (data[i].high + data[i].low + data[i].close) / 3.0;
            let prev_typical = (data[i - 1].high + data[i - 1].low + data[i - 1].close) / 3.0;
            let money_flow = typical_price * data[i].volume_value;
            
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use chrono::{DateTime, Utc};
    
    fn create_test_data() -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        for i in 0..50 {
            let mut ts_data = TimeSeriesData::new("TEST".to_string(), DateTime::<Utc>::from_timestamp(1640995200 + i * 60, 0).unwrap());
            ts_data.open = 100.0 + i as f64 * 0.5;
            ts_data.high = 105.0 + i as f64 * 0.5;
            ts_data.low = 95.0 + i as f64 * 0.5;
            ts_data.close = 102.0 + i as f64 * 0.5;
            ts_data.volume = vec![1000.0 + i as f64 * 10.0];
            data.push(ts_data);
        }
        data
    }
    
    #[test]
    fn test_rsi_calculation() {
        let config = IndicatorConfig::default();
        let momentum = MomentumIndicators::new(&config);
        let data = create_test_data();
        
        let rsi = momentum.calculate_rsi(&data, 14).unwrap();
        assert!(rsi >= 0.0 && rsi <= 100.0);
    }
    
    #[test]
    fn test_cci_calculation() {
        let config = IndicatorConfig::default();
        let momentum = MomentumIndicators::new(&config);
        let data = create_test_data();
        let current = &data[data.len() - 1];
        
        let cci = momentum.calculate_cci(current, &data, 20).unwrap();
        assert!(cci.is_finite());
    }
    
    #[test]
    fn test_stochastic_calculation() {
        let config = IndicatorConfig::default();
        let momentum = MomentumIndicators::new(&config);
        let data = create_test_data();
        let current = &data[data.len() - 1];
        
        let (stoch_k, stoch_d) = momentum.calculate_stochastic(current, &data, 14, 3).unwrap();
        assert!(stoch_k >= 0.0 && stoch_k <= 100.0);
        assert!(stoch_d >= 0.0 && stoch_d <= 100.0);
    }
    
    #[test]
    fn test_ultimate_oscillator() {
        let config = IndicatorConfig::default();
        let momentum = MomentumIndicators::new(&config);
        let data = create_test_data();
        let current = &data[data.len() - 1];
        
        let ult_osc = momentum.calculate_ultimate_oscillator(current, &data).unwrap();
        assert!(ult_osc >= 0.0 && ult_osc <= 100.0);
    }
    
    #[test]
    fn test_mfi_calculation() {
        let config = IndicatorConfig::default();
        let momentum = MomentumIndicators::new(&config);
        let data = create_test_data();
        
        let mfi = momentum.calculate_mfi(&data, 14).unwrap();
        assert!(mfi >= 0.0 && mfi <= 100.0);
    }
}