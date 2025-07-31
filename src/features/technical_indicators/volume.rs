//! Volume indicators implementation
//! 
//! This module implements volume-based indicators like OBV, VWAP, MFI, and A/D Line.

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;
use super::config::IndicatorConfig;

/// Volume indicators calculator
pub struct VolumeIndicators<'a> {
    config: &'a IndicatorConfig,
}

impl<'a> VolumeIndicators<'a> {
    pub fn new(config: &'a IndicatorConfig) -> Self {
        Self { config }
    }
    
    /// Compute all volume indicators
    pub async fn compute_all(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        if !self.config.enable_volume_weighted {
            return Ok(());
        }
        
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
        
        // Volume Profile features
        if historical.len() >= 20 {
            let volume_profile = self.calculate_volume_profile(historical, 20)?;
            features.extend(volume_profile);
        }
        
        // Chaikin Money Flow
        if historical.len() >= 20 {
            let cmf = self.calculate_chaikin_money_flow(historical, 20)?;
            features.insert("chaikin_money_flow".to_string(), cmf);
        }
        
        // Volume Oscillator
        if historical.len() >= 28 {
            let vol_osc = self.calculate_volume_oscillator(historical, 14, 28)?;
            features.insert("volume_oscillator".to_string(), vol_osc);
        }
        
        // Klinger Oscillator
        if historical.len() >= 55 {
            let klinger = self.calculate_klinger_oscillator(historical, 34, 55, 13)?;
            features.insert("klinger_oscillator".to_string(), klinger);
        }
        
        // Volume Price Trend (VPT)
        let vpt = self.calculate_volume_price_trend(current, historical)?;
        features.insert("volume_price_trend".to_string(), vpt);
        
        // Ease of Movement
        if historical.len() >= 14 {
            let eom = self.calculate_ease_of_movement(current, historical, 14)?;
            features.insert("ease_of_movement".to_string(), eom);
        }
        
        Ok(())
    }
    
    /// Calculate On-Balance Volume trend using linear regression
    pub fn calculate_obv_trend(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
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
    
    /// Calculate Volume-Weighted Average Price
    pub fn calculate_vwap(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Money Flow Index
    pub fn calculate_mfi(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Accumulation/Distribution Line slope
    pub fn calculate_ad_line_slope(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Volume Profile (simplified)
    pub fn calculate_volume_profile(&self, data: &[TimeSeriesData], period: usize) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let recent: Vec<&TimeSeriesData> = data.iter()
            .rev()
            .take(period)
            .collect();
        
        // Find price levels with highest volume
        let mut price_volumes: Vec<(f64, f64)> = Vec::new();
        
        for d in &recent {
            let typical_price = (d.high + d.low + d.close) / 3.0;
            price_volumes.push((typical_price, d.volume));
        }
        
        // Sort by price
        price_volumes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        
        // Find volume-weighted average price (VWAP)
        let total_pv: f64 = price_volumes.iter().map(|(p, v)| p * v).sum();
        let total_v: f64 = price_volumes.iter().map(|(_, v)| *v).sum();
        let vwap = if total_v > 0.0 { total_pv / total_v } else { 0.0 };
        
        // Calculate volume above and below VWAP
        let (volume_above, volume_below) = price_volumes.iter()
            .fold((0.0, 0.0), |(above, below), &(price, volume)| {
                if price > vwap {
                    (above + volume, below)
                } else {
                    (above, below + volume)
                }
            });
        
        features.insert("volume_above_vwap".to_string(), volume_above);
        features.insert("volume_below_vwap".to_string(), volume_below);
        features.insert("volume_imbalance".to_string(), 
            if total_v > 0.0 { (volume_above - volume_below) / total_v } else { 0.0 }
        );
        
        // Volume concentration at specific price levels
        let max_volume = price_volumes.iter().map(|(_, v)| *v).fold(0.0, f64::max);
        features.insert("max_volume_concentration".to_string(), max_volume / total_v);
        
        Ok(features)
    }
    
    /// Calculate Chaikin Money Flow
    pub fn calculate_chaikin_money_flow(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        let mut money_flow_volume_sum = 0.0;
        let mut volume_sum = 0.0;
        
        for d in data.iter().rev().take(period) {
            let money_flow_multiplier = if d.high != d.low {
                ((d.close - d.low) - (d.high - d.close)) / (d.high - d.low)
            } else {
                0.0
            };
            
            let money_flow_volume = money_flow_multiplier * d.volume;
            money_flow_volume_sum += money_flow_volume;
            volume_sum += d.volume;
        }
        
        if volume_sum == 0.0 {
            return Ok(0.0);
        }
        
        Ok(money_flow_volume_sum / volume_sum)
    }
    
    /// Calculate Volume Oscillator
    pub fn calculate_volume_oscillator(
        &self,
        data: &[TimeSeriesData],
        short_period: usize,
        long_period: usize,
    ) -> Result<f64> {
        if data.len() < long_period {
            return Ok(0.0);
        }
        
        // Calculate short and long volume averages
        let short_avg: f64 = data.iter()
            .rev()
            .take(short_period)
            .map(|d| d.volume)
            .sum::<f64>() / short_period as f64;
        
        let long_avg: f64 = data.iter()
            .rev()
            .take(long_period)
            .map(|d| d.volume)
            .sum::<f64>() / long_period as f64;
        
        if long_avg == 0.0 {
            return Ok(0.0);
        }
        
        Ok(((short_avg - long_avg) / long_avg) * 100.0)
    }
    
    /// Calculate Klinger Oscillator
    pub fn calculate_klinger_oscillator(
        &self,
        data: &[TimeSeriesData],
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
    ) -> Result<f64> {
        if data.len() < slow_period + 1 {
            return Ok(0.0);
        }
        
        let mut volume_force = Vec::new();
        
        for i in 1..data.len() {
            let typical_price = (data[i].high + data[i].low + data[i].close) / 3.0;
            let prev_typical = (data[i - 1].high + data[i - 1].low + data[i - 1].close) / 3.0;
            
            let trend = if typical_price > prev_typical { 1.0 } else { -1.0 };
            let vf = data[i].volume * trend * ((typical_price - prev_typical) / prev_typical).abs();
            
            volume_force.push(vf);
        }
        
        // Calculate EMAs (simplified)
        let alpha_fast = 2.0 / (fast_period as f64 + 1.0);
        let alpha_slow = 2.0 / (slow_period as f64 + 1.0);
        
        let mut ema_fast = volume_force[0];
        let mut ema_slow = volume_force[0];
        
        for &vf in volume_force.iter().skip(1) {
            ema_fast = alpha_fast * vf + (1.0 - alpha_fast) * ema_fast;
            ema_slow = alpha_slow * vf + (1.0 - alpha_slow) * ema_slow;
        }
        
        Ok(ema_fast - ema_slow)
    }
    
    /// Calculate Volume Price Trend
    pub fn calculate_volume_price_trend(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
    ) -> Result<f64> {
        if data.is_empty() {
            return Ok(0.0);
        }
        
        let mut vpt = 0.0;
        
        for i in 1..data.len() {
            let price_change_ratio = (data[i].close - data[i - 1].close) / data[i - 1].close;
            vpt += data[i].volume * price_change_ratio;
        }
        
        // Add current
        if let Some(last) = data.last() {
            let price_change_ratio = (current.close - last.close) / last.close;
            vpt += current.volume * price_change_ratio;
        }
        
        Ok(vpt)
    }
    
    /// Calculate Ease of Movement
    pub fn calculate_ease_of_movement(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<f64> {
        let mut em_values = Vec::new();
        
        for i in 1..data.len().min(period + 1) {
            let distance_moved = ((data[i].high + data[i].low) / 2.0) - 
                                ((data[i - 1].high + data[i - 1].low) / 2.0);
            
            let box_height = data[i].high - data[i].low;
            let box_ratio = if box_height > 0.0 { data[i].volume / box_height } else { 0.0 };
            
            let em = if box_ratio > 0.0 { distance_moved / box_ratio } else { 0.0 };
            em_values.push(em);
        }
        
        // Add current calculation
        if let Some(last) = data.last() {
            let distance_moved = ((current.high + current.low) / 2.0) - 
                                ((last.high + last.low) / 2.0);
            
            let box_height = current.high - current.low;
            let box_ratio = if box_height > 0.0 { current.volume / box_height } else { 0.0 };
            
            let em = if box_ratio > 0.0 { distance_moved / box_ratio } else { 0.0 };
            em_values.push(em);
        }
        
        // Return SMA of Ease of Movement values
        if em_values.is_empty() {
            return Ok(0.0);
        }
        
        Ok(em_values.iter().sum::<f64>() / em_values.len() as f64)
    }
    
    /// Calculate Negative Volume Index
    pub fn calculate_negative_volume_index(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 2 {
            return Ok(1000.0); // Starting value
        }
        
        let mut nvi = 1000.0;
        
        for i in 1..data.len() {
            if data[i].volume < data[i - 1].volume {
                nvi *= (data[i].close / data[i - 1].close);
            }
        }
        
        Ok(nvi)
    }
    
    /// Calculate Positive Volume Index
    pub fn calculate_positive_volume_index(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 2 {
            return Ok(1000.0); // Starting value
        }
        
        let mut pvi = 1000.0;
        
        for i in 1..data.len() {
            if data[i].volume > data[i - 1].volume {
                pvi *= (data[i].close / data[i - 1].close);
            }
        }
        
        Ok(pvi)
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
                open: 100.0 + (i as f64 * 0.1).sin(),
                high: 105.0 + (i as f64 * 0.1).sin(),
                low: 95.0 + (i as f64 * 0.1).sin(),
                close: 102.0 + (i as f64 * 0.1).sin(),
                volume: 1000.0 + (i as f64 * 50.0).abs(),
            });
        }
        data
    }
    
    #[test]
    fn test_obv_trend() {
        let config = IndicatorConfig::default();
        let volume = VolumeIndicators::new(&config);
        let data = create_test_data();
        let current = &data[data.len() - 1];
        
        let obv_trend = volume.calculate_obv_trend(current, &data, 20).unwrap();
        assert!(obv_trend.is_finite());
    }
    
    #[test]
    fn test_vwap() {
        let config = IndicatorConfig::default();
        let volume = VolumeIndicators::new(&config);
        let data = create_test_data();
        
        let vwap = volume.calculate_vwap(&data, 20).unwrap();
        assert!(vwap > 0.0);
    }
    
    #[test]
    fn test_mfi() {
        let config = IndicatorConfig::default();
        let volume = VolumeIndicators::new(&config);
        let data = create_test_data();
        
        let mfi = volume.calculate_mfi(&data, 14).unwrap();
        assert!(mfi >= 0.0 && mfi <= 100.0);
    }
    
    #[test]
    fn test_ad_line_slope() {
        let config = IndicatorConfig::default();
        let volume = VolumeIndicators::new(&config);
        let data = create_test_data();
        
        let ad_slope = volume.calculate_ad_line_slope(&data, 20).unwrap();
        assert!(ad_slope.is_finite());
    }
    
    #[test]
    fn test_chaikin_money_flow() {
        let config = IndicatorConfig::default();
        let volume = VolumeIndicators::new(&config);
        let data = create_test_data();
        
        let cmf = volume.calculate_chaikin_money_flow(&data, 20).unwrap();
        assert!(cmf >= -1.0 && cmf <= 1.0);
    }
    
    #[test]
    fn test_volume_oscillator() {
        let config = IndicatorConfig::default();
        let volume = VolumeIndicators::new(&config);
        let data = create_test_data();
        
        let vol_osc = volume.calculate_volume_oscillator(&data, 14, 28).unwrap();
        assert!(vol_osc.is_finite());
    }
}