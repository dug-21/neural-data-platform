//! Advanced technical indicators implementation
//! 
//! This module implements advanced indicators like Elliott Wave patterns, 
//! Harmonic patterns, Market Profile, and other sophisticated analysis tools.

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;
use super::config::IndicatorConfig;

/// Advanced indicators calculator
pub struct AdvancedIndicators<'a> {
    config: &'a IndicatorConfig,
}

impl<'a> AdvancedIndicators<'a> {
    pub fn new(config: &'a IndicatorConfig) -> Self {
        Self { config }
    }
    
    /// Compute all advanced indicators
    pub async fn compute_all(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
        features: &mut HashMap<String, f64>,
    ) -> Result<()> {
        if !self.config.enable_custom {
            return Ok(());
        }
        
        // Price-based features
        self.compute_price_features(current, historical, features)?;
        
        // Heikin-Ashi transformation
        if let Some(prev) = historical.last() {
            let ha_features = self.calculate_heikin_ashi(current, prev)?;
            features.extend(ha_features);
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
            let fib_features = self.calculate_fibonacci_levels(current, historical, 100)?;
            features.extend(fib_features);
        }
        
        // Pivot points
        if let Some(prev) = historical.last() {
            let pivot_features = self.calculate_pivot_points(current, prev)?;
            features.extend(pivot_features);
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
        
        // Support and Resistance levels
        if historical.len() >= 50 {
            let sr_features = self.calculate_support_resistance(current, historical, 50)?;
            features.extend(sr_features);
        }
        
        // Market Structure analysis
        if historical.len() >= 30 {
            let structure_features = self.analyze_market_structure(current, historical, 30)?;
            features.extend(structure_features);
        }
        
        Ok(())
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
    
    /// Calculate Heikin-Ashi features
    pub fn calculate_heikin_ashi(
        &self,
        current: &TimeSeriesData,
        prev: &TimeSeriesData,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let ha_close = (current.open + current.high + current.low + current.close) / 4.0;
        let ha_open = (prev.open + prev.close) / 2.0;
        let ha_high = current.high.max(ha_open).max(ha_close);
        let ha_low = current.low.min(ha_open).min(ha_close);
        
        features.insert("ha_body_size".to_string(), (ha_close - ha_open).abs());
        features.insert("ha_upper_shadow".to_string(), ha_high - ha_close.max(ha_open));
        features.insert("ha_lower_shadow".to_string(), ha_close.min(ha_open) - ha_low);
        features.insert("ha_trend".to_string(), (ha_close - ha_open).signum());
        
        // Heikin-Ashi trend strength
        let body_to_range_ratio = if ha_high != ha_low {
            (ha_close - ha_open).abs() / (ha_high - ha_low)
        } else {
            0.0
        };
        features.insert("ha_trend_strength".to_string(), body_to_range_ratio);
        
        Ok(features)
    }
    
    /// Calculate Market Profile Value Area
    pub fn calculate_value_area(
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
            price_volumes.push((typical_price, d.volume_value));
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
    
    /// Calculate Fibonacci retracement levels
    pub fn calculate_fibonacci_levels(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let (high, low) = self.get_high_low_range(data, period)?;
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
        
        // Find closest Fibonacci level
        let mut min_distance = f64::MAX;
        let mut closest_fib = 0.0;
        
        for (_, level) in &[
            ("fib_236", low + range * 0.236),
            ("fib_382", low + range * 0.382),
            ("fib_500", low + range * 0.500),
            ("fib_618", low + range * 0.618),
            ("fib_786", low + range * 0.786),
        ] {
            let distance = (current.close - level).abs();
            if distance < min_distance {
                min_distance = distance;
                closest_fib = *level;
            }
        }
        
        features.insert("closest_fib_level".to_string(), closest_fib);
        features.insert("closest_fib_distance".to_string(), min_distance);
        
        Ok(features)
    }
    
    /// Calculate pivot points
    pub fn calculate_pivot_points(
        &self,
        current: &TimeSeriesData,
        prev: &TimeSeriesData,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let pivot = (prev.high + prev.low + prev.close) / 3.0;
        let r1 = 2.0 * pivot - prev.low;
        let s1 = 2.0 * pivot - prev.high;
        let r2 = pivot + (prev.high - prev.low);
        let s2 = pivot - (prev.high - prev.low);
        let r3 = r1 + (prev.high - prev.low);
        let s3 = s1 - (prev.high - prev.low);
        
        features.insert("pivot_point".to_string(), pivot);
        features.insert("resistance_1".to_string(), r1);
        features.insert("resistance_2".to_string(), r2);
        features.insert("resistance_3".to_string(), r3);
        features.insert("support_1".to_string(), s1);
        features.insert("support_2".to_string(), s2);
        features.insert("support_3".to_string(), s3);
        
        // Distance to nearest pivot level
        let pivot_levels = vec![s3, s2, s1, pivot, r1, r2, r3];
        let mut min_distance = f64::MAX;
        let mut nearest_level = pivot;
        
        for level in &pivot_levels {
            let distance = (current.close - level).abs();
            if distance < min_distance {
                min_distance = distance;
                nearest_level = *level;
            }
        }
        
        features.insert("nearest_pivot_level".to_string(), nearest_level);
        features.insert("nearest_pivot_distance".to_string(), min_distance);
        
        Ok(features)
    }
    
    /// Detect Elliott Wave patterns (simplified)
    pub fn detect_elliott_waves(
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
    
    /// Detect Harmonic patterns (simplified)
    pub fn detect_harmonic_patterns(
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
            let xa_move = points[1].price - points[0].price;
            let ab_move = points[2].price - points[1].price;
            let bc_move = points[3].price - points[2].price;
            let cd_move = points[4].price - points[3].price;
            
            let ab_xa_ratio = ab_move.abs() / xa_move.abs();
            let bc_ab_ratio = bc_move.abs() / ab_move.abs();
            let cd_bc_ratio = cd_move.abs() / bc_move.abs();
            let ad_xa_ratio = (points[4].price - points[1].price).abs() / xa_move.abs();
            
            // Gartley Pattern detection
            if (ab_xa_ratio - 0.618).abs() < 0.05 &&
               bc_ab_ratio >= 0.382 && bc_ab_ratio <= 0.886 &&
               cd_bc_ratio >= 1.13 && cd_bc_ratio <= 1.618 &&
               (ad_xa_ratio - 0.786).abs() < 0.05 {
                features.insert("harmonic_pattern_gartley".to_string(), xa_move.signum());
                features.insert("harmonic_gartley_completion".to_string(), 1.0);
            }
            
            // General harmonic pattern metrics
            features.insert("harmonic_ab_xa_ratio".to_string(), ab_xa_ratio);
            features.insert("harmonic_bc_ab_ratio".to_string(), bc_ab_ratio);
            features.insert("harmonic_cd_bc_ratio".to_string(), cd_bc_ratio);
            features.insert("harmonic_ad_xa_ratio".to_string(), ad_xa_ratio);
        }
        
        Ok(features)
    }
    
    /// Calculate support and resistance levels
    pub fn calculate_support_resistance(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let swings = self.find_swing_points(data, 5)?; // 5-period swings for S/R
        
        // Separate highs and lows
        let resistance_levels: Vec<f64> = swings.iter()
            .filter(|s| matches!(s.swing_type, SwingType::High))
            .map(|s| s.price)
            .collect();
        
        let support_levels: Vec<f64> = swings.iter()
            .filter(|s| matches!(s.swing_type, SwingType::Low))
            .map(|s| s.price)
            .collect();
        
        // Find nearest support and resistance
        let nearest_resistance = resistance_levels.iter()
            .filter(|&&level| level > current.close)
            .min_by(|a, b| ((**a) - current.close).partial_cmp(&((**b) - current.close)).unwrap())
            .copied()
            .unwrap_or(current.high);
        
        let nearest_support = support_levels.iter()
            .filter(|&&level| level < current.close)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap_or(current.low);
        
        features.insert("nearest_resistance".to_string(), nearest_resistance);
        features.insert("nearest_support".to_string(), nearest_support);
        features.insert("resistance_distance".to_string(), 
            ((nearest_resistance - current.close) / current.close * 100.0).abs());
        features.insert("support_distance".to_string(), 
            ((current.close - nearest_support) / current.close * 100.0).abs());
        
        // Support/Resistance strength (number of touches)
        let resistance_strength = resistance_levels.iter()
            .filter(|&&level| (level - nearest_resistance).abs() < nearest_resistance * 0.01)
            .count() as f64;
        
        let support_strength = support_levels.iter()
            .filter(|&&level| (level - nearest_support).abs() < nearest_support * 0.01)
            .count() as f64;
        
        features.insert("resistance_strength".to_string(), resistance_strength);
        features.insert("support_strength".to_string(), support_strength);
        
        Ok(features)
    }
    
    /// Analyze market structure
    pub fn analyze_market_structure(
        &self,
        current: &TimeSeriesData,
        data: &[TimeSeriesData],
        period: usize,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        let swings = self.find_swing_points(data, 5)?;
        
        if swings.len() >= 4 {
            // Analyze trend structure (higher highs, higher lows, etc.)
            let recent_swings: Vec<_> = swings.iter().rev().take(4).collect();
            
            let mut higher_highs = 0;
            let mut lower_lows = 0;
            let mut higher_lows = 0;
            let mut lower_highs = 0;
            
            for i in 1..recent_swings.len() {
                let current_swing = recent_swings[i];
                let prev_swing = recent_swings[i - 1];
                
                match (current_swing.swing_type, prev_swing.swing_type) {
                    (SwingType::High, SwingType::High) => {
                        if current_swing.price > prev_swing.price {
                            higher_highs += 1;
                        } else {
                            lower_highs += 1;
                        }
                    }
                    (SwingType::Low, SwingType::Low) => {
                        if current_swing.price > prev_swing.price {
                            higher_lows += 1;
                        } else {
                            lower_lows += 1;
                        }
                    }
                    _ => {}
                }
            }
            
            // Market structure bias
            let bullish_structure = (higher_highs + higher_lows) as f64;
            let bearish_structure = (lower_highs + lower_lows) as f64;
            let total_structure = bullish_structure + bearish_structure;
            
            if total_structure > 0.0 {
                features.insert("market_structure_bullish".to_string(), bullish_structure / total_structure);
                features.insert("market_structure_bearish".to_string(), bearish_structure / total_structure);
            }
            
            features.insert("higher_highs_count".to_string(), higher_highs as f64);
            features.insert("higher_lows_count".to_string(), higher_lows as f64);
            features.insert("lower_highs_count".to_string(), lower_highs as f64);
            features.insert("lower_lows_count".to_string(), lower_lows as f64);
        }
        
        Ok(features)
    }
    
    // Helper methods
    
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
        
        // Calculate pattern strength
        let mut strength = 0.5; // Base strength
        
        // Wave 3 often extends to 161.8% of wave 1
        let wave3_extension = wave3_size / wave1_size;
        if wave3_extension >= 1.5 && wave3_extension <= 1.7 {
            strength += 0.25;
        }
        
        Some(ElliottWavePattern {
            current_wave: 5,
            strength,
            completion_ratio: 1.0,
        })
    }
}

// Supporting structures

#[derive(Debug, Clone)]
pub struct SwingPoint {
    pub index: usize,
    pub price: f64,
    pub swing_type: SwingType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwingType {
    High,
    Low,
}

#[derive(Debug)]
pub struct ElliottWavePattern {
    pub current_wave: usize,
    pub strength: f64,
    pub completion_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use chrono::{DateTime, Utc};
    
    fn create_test_data() -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        for i in 0..100 {
            let base_price = 100.0;
            let trend = i as f64 * 0.1;
            let noise = (i as f64 / 5.0).sin() * 2.0;
            
            let mut ts_data = TimeSeriesData::new("TEST".to_string(), DateTime::<Utc>::from_timestamp(1640995200 + i * 60, 0).unwrap());
            ts_data.open = base_price + trend + noise;
            ts_data.high = base_price + trend + noise + 2.0;
            ts_data.low = base_price + trend + noise - 2.0;
            ts_data.close = base_price + trend + noise + 0.5;
            ts_data.volume = vec![1000.0 + i as f64 * 10.0];
            data.push(ts_data);
        }
        data
    }
    
    #[test]
    fn test_heikin_ashi() {
        let config = IndicatorConfig::default();
        let advanced = AdvancedIndicators::new(&config);
        let data = create_test_data();
        
        let ha_features = advanced.calculate_heikin_ashi(&data[1], &data[0]).unwrap();
        assert!(!ha_features.is_empty());
        assert!(ha_features.contains_key("ha_body_size"));
    }
    
    #[test]
    fn test_value_area() {
        let config = IndicatorConfig::default();
        let advanced = AdvancedIndicators::new(&config);
        let data = create_test_data();
        
        let (vah, val, poc) = advanced.calculate_value_area(&data, 20).unwrap();
        assert!(vah >= val);
        assert!(poc >= val && poc <= vah);
    }
    
    #[test]
    fn test_fibonacci_levels() {
        let config = IndicatorConfig::default();
        let advanced = AdvancedIndicators::new(&config);
        let data = create_test_data();
        let current = &data[data.len() - 1];
        
        let fib_features = advanced.calculate_fibonacci_levels(current, &data, 50).unwrap();
        assert!(fib_features.contains_key("fib_618_level"));
        assert!(fib_features.contains_key("closest_fib_level"));
    }
    
    #[test]
    fn test_pivot_points() {
        let config = IndicatorConfig::default();
        let advanced = AdvancedIndicators::new(&config);
        let data = create_test_data();
        
        let pivot_features = advanced.calculate_pivot_points(&data[1], &data[0]).unwrap();
        assert!(pivot_features.contains_key("pivot_point"));
        assert!(pivot_features.contains_key("resistance_1"));
        assert!(pivot_features.contains_key("support_1"));
    }
    
    #[test]
    fn test_swing_points() {
        let config = IndicatorConfig::default();
        let advanced = AdvancedIndicators::new(&config);
        let data = create_test_data();
        
        let swings = advanced.find_swing_points(&data, 5).unwrap();
        assert!(!swings.is_empty());
    }
}