//! Trend indicators implementation
//! 
//! This module implements trend-following indicators like EMA, MACD, ADX, and Ichimoku.

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;
use super::config::IndicatorConfig;

/// Trend indicators calculator
pub struct TrendIndicators<'a> {
    config: &'a IndicatorConfig,
}

impl<'a> TrendIndicators<'a> {
    pub fn new(config: &'a IndicatorConfig) -> Self {
        Self { config }
    }
    
    /// Compute all trend indicators
    pub async fn compute_all(
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
    
    /// Calculate Exponential Moving Average
    pub fn calculate_ema(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Simple Moving Average
    pub fn calculate_sma(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
        if data.len() < period {
            return Err(anyhow::anyhow!("Insufficient data for SMA calculation"));
        }
        
        let sum: f64 = data.iter()
            .rev()
            .take(period)
            .map(|d| d.close)
            .sum();
        
        Ok(sum / period as f64)
    }
    
    /// Calculate MACD (Moving Average Convergence Divergence)
    pub fn calculate_macd(
        &self,
        data: &[TimeSeriesData],
        fast: usize,
        slow: usize,
        signal: usize,
    ) -> Result<(f64, f64, f64)> {
        let fast_ema = self.calculate_ema(data, fast)?;
        let slow_ema = self.calculate_ema(data, slow)?;
        let macd_line = fast_ema - slow_ema;
        
        // For signal line, we need MACD history - simplified implementation
        let signal_line = macd_line * 0.9; // Approximation for demo
        let histogram = macd_line - signal_line;
        
        Ok((macd_line, signal_line, histogram))
    }
    
    /// Get previous MACD histogram sign for crossover detection
    fn get_prev_macd_histogram_sign(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 2 {
            return Ok(0.0);
        }
        
        let (fast, slow, signal) = self.config.macd_params;
        let prev_data = &data[..data.len() - 1];
        let (_, _, histogram) = self.calculate_macd(prev_data, fast, slow, signal)?;
        
        Ok(histogram.signum())
    }
    
    /// Calculate ADX (Average Directional Index)
    pub fn calculate_adx(&self, data: &[TimeSeriesData], period: usize) -> Result<f64> {
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
    
    /// Calculate Ichimoku Cloud components
    pub fn calculate_ichimoku(
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
    
    /// Calculate Parabolic SAR
    pub fn calculate_parabolic_sar(
        &self,
        data: &[TimeSeriesData],
        acceleration_factor: f64,
        max_acceleration: f64,
    ) -> Result<Vec<f64>> {
        if data.len() < 2 {
            return Err(anyhow::anyhow!("Insufficient data for Parabolic SAR"));
        }
        
        let mut sar_values = vec![data[0].low]; // Start with first low
        let mut ep = data[0].high; // Extreme point
        let mut af = acceleration_factor;
        let mut is_uptrend = true;
        
        for i in 1..data.len() {
            let mut sar = sar_values[i - 1] + af * (ep - sar_values[i - 1]);
            
            if is_uptrend {
                // In uptrend
                if data[i].low <= sar {
                    // Trend reversal
                    is_uptrend = false;
                    sar = ep;
                    ep = data[i].low;
                    af = acceleration_factor;
                } else {
                    // Continue uptrend
                    if data[i].high > ep {
                        ep = data[i].high;
                        af = (af + acceleration_factor).min(max_acceleration);
                    }
                    // Ensure SAR doesn't go above previous two lows
                    let min_low = data[i.saturating_sub(2)..=i.saturating_sub(1)]
                        .iter()
                        .map(|d| d.low)
                        .fold(f64::MAX, f64::min);
                    sar = sar.min(min_low);
                }
            } else {
                // In downtrend
                if data[i].high >= sar {
                    // Trend reversal
                    is_uptrend = true;
                    sar = ep;
                    ep = data[i].high;
                    af = acceleration_factor;
                } else {
                    // Continue downtrend
                    if data[i].low < ep {
                        ep = data[i].low;
                        af = (af + acceleration_factor).min(max_acceleration);
                    }
                    // Ensure SAR doesn't go below previous two highs
                    let max_high = data[i.saturating_sub(2)..=i.saturating_sub(1)]
                        .iter()
                        .map(|d| d.high)
                        .fold(f64::MIN, f64::max);
                    sar = sar.max(max_high);
                }
            }
            
            sar_values.push(sar);
        }
        
        Ok(sar_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use chrono::{DateTime, Utc};
    
    fn create_test_data() -> Vec<TimeSeriesData> {
        vec![
            TimeSeriesData {
                timestamp: DateTime::<Utc>::from_timestamp(1640995200, 0).unwrap(),
                open: 100.0,
                high: 105.0,
                low: 95.0,
                close: 102.0,
                volume: 1000.0,
            },
            TimeSeriesData {
                timestamp: DateTime::<Utc>::from_timestamp(1640995260, 0).unwrap(),
                open: 102.0,
                high: 107.0,
                low: 98.0,
                close: 104.0,
                volume: 1100.0,
            },
            TimeSeriesData {
                timestamp: DateTime::<Utc>::from_timestamp(1640995320, 0).unwrap(),
                open: 104.0,
                high: 109.0,
                low: 101.0,
                close: 106.0,
                volume: 1200.0,
            },
        ]
    }
    
    #[test]
    fn test_ema_calculation() {
        let config = IndicatorConfig::default();
        let trend = TrendIndicators::new(&config);
        let data = create_test_data();
        
        let ema = trend.calculate_ema(&data, 3).unwrap();
        assert!(ema > 0.0);
    }
    
    #[test]
    fn test_sma_calculation() {
        let config = IndicatorConfig::default();
        let trend = TrendIndicators::new(&config);
        let data = create_test_data();
        
        let sma = trend.calculate_sma(&data, 3).unwrap();
        assert_eq!(sma, (102.0 + 104.0 + 106.0) / 3.0);
    }
    
    #[test]
    fn test_macd_calculation() {
        let config = IndicatorConfig::default();
        let trend = TrendIndicators::new(&config);
        let data = create_test_data();
        
        let (macd_line, signal_line, histogram) = trend.calculate_macd(&data, 2, 3, 1).unwrap();
        assert!(macd_line.is_finite());
        assert!(signal_line.is_finite());
        assert!(histogram.is_finite());
    }
    
    #[test]
    fn test_adx_calculation() {
        let config = IndicatorConfig::default();
        let trend = TrendIndicators::new(&config);
        let data = create_test_data();
        
        let adx = trend.calculate_adx(&data, 2).unwrap();
        assert!(adx >= 0.0 && adx <= 100.0);
    }
}