//! MVP Feature Engineering
//!
//! Simplified feature set focused on essential technical indicators
//! Designed to produce exactly 20 features for the MVP neural network

use crate::features::{FeatureExtractor, FeatureVector};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// MVP Feature Extractor - Essential technical indicators only
#[derive(Debug, Clone)]
pub struct MVPFeatureExtractor {
    window_size: usize,
}

impl MVPFeatureExtractor {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size: window_size.max(50), // Need at least 50 for SMA_50
        }
    }
    
    /// Calculate Simple Moving Average
    fn sma(&self, data: &[f32], period: usize) -> f32 {
        if data.len() < period {
            return data.iter().sum::<f32>() / data.len() as f32;
        }
        
        let start_idx = data.len() - period;
        data[start_idx..].iter().sum::<f32>() / period as f32
    }
    
    /// Calculate Relative Strength Index (14-period)
    fn rsi(&self, data: &[f32], period: usize) -> f32 {
        if data.len() < period + 1 {
            return 50.0; // Neutral RSI
        }
        
        let mut gains = 0.0;
        let mut losses = 0.0;
        let start_idx = data.len() - period;
        
        for i in start_idx..data.len() {
            if i > 0 {
                let change = data[i] - data[i - 1];
                if change > 0.0 {
                    gains += change;
                } else {
                    losses -= change; // Make positive
                }
            }
        }
        
        let avg_gain = gains / period as f32;
        let avg_loss = losses / period as f32;
        
        if avg_loss == 0.0 {
            return 100.0;
        }
        
        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }
    
    /// Calculate MACD components
    fn macd(&self, data: &[f32]) -> (f32, f32, f32) {
        let ema12 = self.ema(data, 12);
        let ema26 = self.ema(data, 26);
        let macd_line = ema12 - ema26;
        
        // Simple signal line approximation (normally EMA of MACD)
        let signal_line = macd_line * 0.9; // Simplified
        let histogram = macd_line - signal_line;
        
        (macd_line, signal_line, histogram)
    }
    
    /// Calculate Exponential Moving Average
    fn ema(&self, data: &[f32], period: usize) -> f32 {
        if data.is_empty() {
            return 0.0;
        }
        if data.len() <= period {
            return self.sma(data, data.len());
        }
        
        let multiplier = 2.0 / (period + 1) as f32;
        let mut ema = self.sma(&data[..period], period);
        
        for &price in &data[period..] {
            ema = (price * multiplier) + (ema * (1.0 - multiplier));
        }
        
        ema
    }
    
    /// Calculate Bollinger Bands
    fn bollinger_bands(&self, data: &[f32], period: usize, std_dev: f32) -> (f32, f32, f32) {
        let sma = self.sma(data, period);
        
        // Calculate standard deviation
        let variance = if data.len() >= period {
            let start_idx = data.len() - period;
            let sum_sq_diff: f32 = data[start_idx..].iter()
                .map(|&x| (x - sma).powi(2))
                .sum();
            sum_sq_diff / period as f32
        } else {
            0.0
        };
        
        let std = variance.sqrt();
        
        let upper = sma + (std * std_dev);
        let lower = sma - (std * std_dev);
        
        (upper, sma, lower)
    }
    
    /// Calculate price returns for different periods
    fn price_returns(&self, data: &[f32]) -> (f32, f32, f32) {
        let current = data.last().copied().unwrap_or(0.0);
        
        let return_1d = if data.len() >= 2 {
            let prev_1 = data[data.len() - 2];
            if prev_1 != 0.0 { (current - prev_1) / prev_1 } else { 0.0 }
        } else { 0.0 };
        
        let return_5d = if data.len() >= 6 {
            let prev_5 = data[data.len() - 6];
            if prev_5 != 0.0 { (current - prev_5) / prev_5 } else { 0.0 }
        } else { 0.0 };
        
        let return_20d = if data.len() >= 21 {
            let prev_20 = data[data.len() - 21];
            if prev_20 != 0.0 { (current - prev_20) / prev_20 } else { 0.0 }
        } else { 0.0 };
        
        (return_1d, return_5d, return_20d)
    }
}

impl FeatureExtractor for MVPFeatureExtractor {
    fn extract(&self, data: &[f32]) -> FeatureVector {
        let mut features = FeatureVector::new();
        
        if data.is_empty() {
            // Return zero features if no data
            for i in 0..20 {
                features.add_feature(format!("feature_{}", i), 0.0);
            }
            return features;
        }
        
        let current_price = data.last().copied().unwrap_or(0.0);
        
        // Technical Indicators (12 features)
        let sma_5 = self.sma(data, 5);
        let sma_10 = self.sma(data, 10);
        let sma_20 = self.sma(data, 20);
        let sma_50 = self.sma(data, 50);
        
        features.add_feature("SMA_5".to_string(), sma_5);
        features.add_feature("SMA_10".to_string(), sma_10);
        features.add_feature("SMA_20".to_string(), sma_20);
        features.add_feature("SMA_50".to_string(), sma_50);
        
        let rsi_14 = self.rsi(data, 14);
        features.add_feature("RSI_14".to_string(), rsi_14);
        
        let (macd, macd_signal, macd_hist) = self.macd(data);
        features.add_feature("MACD".to_string(), macd);
        features.add_feature("MACD_Signal".to_string(), macd_signal);
        features.add_feature("MACD_Histogram".to_string(), macd_hist);
        
        let (bb_upper, bb_middle, bb_lower) = self.bollinger_bands(data, 20, 2.0);
        features.add_feature("BB_Upper".to_string(), bb_upper);
        features.add_feature("BB_Middle".to_string(), bb_middle);
        features.add_feature("BB_Lower".to_string(), bb_lower);
        
        // Volume feature (simplified - using price as proxy)
        let volume_sma_20 = self.sma(data, 20);
        features.add_feature("Volume_SMA_20".to_string(), volume_sma_20);
        
        // Price Features (5 features)
        let (return_1d, return_5d, return_20d) = self.price_returns(data);
        features.add_feature("Price_Return_1d".to_string(), return_1d);
        features.add_feature("Price_Return_5d".to_string(), return_5d);
        features.add_feature("Price_Return_20d".to_string(), return_20d);
        
        // High-Low ratio (using SMA range as approximation)
        let high_low_ratio = if sma_50 != 0.0 {
            (sma_5 - sma_20).abs() / sma_50
        } else {
            0.0
        };
        features.add_feature("High_Low_Ratio".to_string(), high_low_ratio);
        
        // Close to SMA ratio
        let close_sma_ratio = if sma_20 != 0.0 {
            current_price / sma_20
        } else {
            1.0
        };
        features.add_feature("Close_SMA_Ratio".to_string(), close_sma_ratio);
        
        // Volume Features (3 features)
        let volume_ratio_5d = if sma_5 != 0.0 {
            current_price / sma_5
        } else {
            1.0
        };
        features.add_feature("Volume_Ratio_5d".to_string(), volume_ratio_5d);
        
        let volume_ratio_20d = if sma_20 != 0.0 {
            current_price / sma_20
        } else {
            1.0
        };
        features.add_feature("Volume_Ratio_20d".to_string(), volume_ratio_20d);
        
        // Price-Volume correlation approximation
        let pv_correlation = if data.len() >= 5 {
            let recent_volatility = data[data.len().saturating_sub(5)..]
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .sum::<f32>() / 4.0;
            recent_volatility / current_price.max(1.0)
        } else {
            0.0
        };
        features.add_feature("Price_Volume_Correlation".to_string(), pv_correlation);
        
        // Ensure we have exactly 20 features
        if features.len() != 20 {
            tracing::warn!("MVP Feature count mismatch: expected 20, got {}", features.len());
            // Pad or truncate to exactly 20
            while features.len() < 20 {
                features.add_feature(format!("padding_{}", features.len()), 0.0);
            }
            if features.len() > 20 {
                features.features.truncate(20);
                features.feature_names.truncate(20);
            }
        }
        
        features.timestamp = chrono::Utc::now().timestamp();
        features
    }
    
    fn get_feature_count(&self) -> usize {
        20 // Fixed feature count for MVP
    }
    
    fn get_feature_names(&self) -> Vec<String> {
        vec![
            // Technical Indicators (12)
            "SMA_5".to_string(),
            "SMA_10".to_string(), 
            "SMA_20".to_string(),
            "SMA_50".to_string(),
            "RSI_14".to_string(),
            "MACD".to_string(),
            "MACD_Signal".to_string(),
            "MACD_Histogram".to_string(),
            "BB_Upper".to_string(),
            "BB_Middle".to_string(),
            "BB_Lower".to_string(),
            "Volume_SMA_20".to_string(),
            
            // Price Features (5)
            "Price_Return_1d".to_string(),
            "Price_Return_5d".to_string(),
            "Price_Return_20d".to_string(),
            "High_Low_Ratio".to_string(),
            "Close_SMA_Ratio".to_string(),
            
            // Volume Features (3)
            "Volume_Ratio_5d".to_string(),
            "Volume_Ratio_20d".to_string(),
            "Price_Volume_Correlation".to_string(),
        ]
    }
}

/// Real-time feature calculator for streaming data
#[derive(Debug)]
pub struct StreamingMVPFeatures {
    extractor: MVPFeatureExtractor,
    price_buffer: VecDeque<f32>,
    window_size: usize,
}

impl StreamingMVPFeatures {
    pub fn new(window_size: usize) -> Self {
        Self {
            extractor: MVPFeatureExtractor::new(window_size),
            price_buffer: VecDeque::with_capacity(window_size),
            window_size,
        }
    }
    
    /// Add new price and get updated features if window is full
    pub fn update(&mut self, price: f32) -> Option<FeatureVector> {
        self.price_buffer.push_back(price);
        
        if self.price_buffer.len() > self.window_size {
            self.price_buffer.pop_front();
        }
        
        if self.price_buffer.len() >= 50 { // Minimum for SMA_50
            let data: Vec<f32> = self.price_buffer.iter().cloned().collect();
            Some(self.extractor.extract(&data))
        } else {
            None
        }
    }
    
    /// Get current features if window is sufficient
    pub fn get_features(&self) -> Option<FeatureVector> {
        if self.price_buffer.len() >= 50 {
            let data: Vec<f32> = self.price_buffer.iter().cloned().collect();
            Some(self.extractor.extract(&data))
        } else {
            None
        }
    }
    
    /// Check if ready to produce features
    pub fn is_ready(&self) -> bool {
        self.price_buffer.len() >= 50
    }
    
    /// Get current buffer size
    pub fn buffer_size(&self) -> usize {
        self.price_buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn generate_test_data(len: usize) -> Vec<f32> {
        let mut data = Vec::new();
        let mut price = 100.0;
        
        for i in 0..len {
            // Simple trending data with some noise
            price += (i as f32 * 0.1).sin() * 2.0 + 0.1;
            data.push(price);
        }
        
        data
    }
    
    #[test]
    fn test_mvp_feature_extractor() {
        let extractor = MVPFeatureExtractor::new(100);
        let data = generate_test_data(100);
        
        let features = extractor.extract(&data);
        
        assert_eq!(features.len(), 20, "Should extract exactly 20 features");
        assert_eq!(features.feature_names.len(), 20, "Should have 20 feature names");
        
        // Check that all features are finite numbers
        for (i, &value) in features.features.iter().enumerate() {
            assert!(value.is_finite(), "Feature {} should be finite, got {}", i, value);
        }
        
        // Verify specific features exist
        let feature_names = extractor.get_feature_names();
        assert!(feature_names.contains(&"SMA_5".to_string()));
        assert!(feature_names.contains(&"RSI_14".to_string()));
        assert!(feature_names.contains(&"MACD".to_string()));
        assert!(feature_names.contains(&"Price_Return_1d".to_string()));
    }
    
    #[test]
    fn test_feature_calculations() {
        let extractor = MVPFeatureExtractor::new(100);
        
        // Test with simple increasing data
        let data = vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0];
        
        let sma_3 = extractor.sma(&data, 3);
        assert!((sma_3 - 104.0).abs() < 0.01, "SMA should be approximately 104");
        
        // Test RSI calculation
        let rsi = extractor.rsi(&data, 5);
        assert!(rsi > 50.0, "RSI should be above 50 for uptrending data");
        
        // Test price returns
        let (return_1d, _return_5d, _return_20d) = extractor.price_returns(&data);
        assert!(return_1d > 0.0, "1-day return should be positive");
    }
    
    #[test]
    fn test_streaming_features() {
        let mut streaming = StreamingMVPFeatures::new(60);
        
        assert!(!streaming.is_ready());
        assert_eq!(streaming.buffer_size(), 0);
        
        // Add enough data to make it ready
        for i in 0..60 {
            let result = streaming.update(100.0 + i as f32 * 0.1);
            if i < 49 {
                assert!(result.is_none(), "Should not return features until buffer is full enough");
            }
        }
        
        assert!(streaming.is_ready());
        assert_eq!(streaming.buffer_size(), 60);
        
        // Should now produce features
        let features = streaming.get_features().unwrap();
        assert_eq!(features.len(), 20);
    }
    
    #[test]
    fn test_empty_data_handling() {
        let extractor = MVPFeatureExtractor::new(50);
        let features = extractor.extract(&[]);
        
        assert_eq!(features.len(), 20, "Should return 20 zero features for empty data");
        
        // All features should be 0.0 for empty data
        for &value in &features.features {
            assert_eq!(value, 0.0, "Empty data features should be zero");
        }
    }
    
    #[test]
    fn test_insufficient_data() {
        let extractor = MVPFeatureExtractor::new(50);
        let small_data = vec![100.0, 101.0, 102.0]; // Only 3 data points
        
        let features = extractor.extract(&small_data);
        assert_eq!(features.len(), 20, "Should still return 20 features");
        
        // Features should be calculated with available data
        for &value in &features.features {
            assert!(value.is_finite(), "All features should be finite numbers");
        }
    }
}