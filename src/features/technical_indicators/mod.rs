//! Modular Technical Indicators System
//! 
//! This module provides a comprehensive technical analysis system organized into
//! specialized modules for different types of indicators.

pub mod config;
pub mod trend;
pub mod momentum;
pub mod volatility;
pub mod volume;
pub mod advanced;

use anyhow::Result;
use std::collections::HashMap;
use crate::data::TimeSeriesData;

pub use config::IndicatorConfig;
pub use trend::TrendIndicators;
pub use momentum::MomentumIndicators;
pub use volatility::VolatilityIndicators;
pub use volume::VolumeIndicators;
pub use advanced::AdvancedIndicators;

/// Main technical indicator computation engine
pub struct TechnicalIndicatorEngine {
    /// Indicator configuration
    config: IndicatorConfig,
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
    
    /// Get the current configuration
    pub fn config(&self) -> &IndicatorConfig {
        &self.config
    }
    
    /// Update the configuration
    pub fn set_config(&mut self, config: IndicatorConfig) {
        self.config = config;
    }
    
    /// Compute all technical indicators
    pub async fn compute_all(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        // Validate configuration
        self.config.validate()?;
        
        // Check minimum data requirements
        let min_required = self.config.min_required_data_length();
        if historical.len() < min_required.saturating_sub(50) {
            return Err(anyhow::anyhow!(
                "Insufficient historical data. Need at least {} periods, got {}",
                min_required.saturating_sub(50),
                historical.len()
            ));
        }
        
        let mut features = HashMap::new();
        
        // Initialize specialized calculators
        let trend_calculator = TrendIndicators::new(&self.config);
        let momentum_calculator = MomentumIndicators::new(&self.config);
        let volatility_calculator = VolatilityIndicators::new(&self.config);
        let volume_calculator = VolumeIndicators::new(&self.config);
        let advanced_calculator = AdvancedIndicators::new(&self.config);
        
        // Compute indicators by category
        trend_calculator.compute_all(current, historical, &mut features).await?;
        momentum_calculator.compute_all(current, historical, &mut features).await?;
        volatility_calculator.compute_all(current, historical, &mut features).await?;
        
        if self.config.enable_volume_weighted {
            volume_calculator.compute_all(current, historical, &mut features).await?;
        }
        
        if self.config.enable_custom {
            advanced_calculator.compute_all(current, historical, &mut features).await?;
        }
        
        // Add metadata
        features.insert("indicator_count".to_string(), features.len() as f64);
        features.insert("data_length".to_string(), historical.len() as f64);
        features.insert("timestamp".to_string(), current.timestamp.timestamp() as f64);
        
        Ok(features)
    }
    
    /// Compute only trend indicators
    pub async fn compute_trend_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        let calculator = TrendIndicators::new(&self.config);
        calculator.compute_all(current, historical, &mut features).await?;
        Ok(features)
    }
    
    /// Compute only momentum indicators
    pub async fn compute_momentum_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        let calculator = MomentumIndicators::new(&self.config);
        calculator.compute_all(current, historical, &mut features).await?;
        Ok(features)
    }
    
    /// Compute only volatility indicators
    pub async fn compute_volatility_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        let calculator = VolatilityIndicators::new(&self.config);
        calculator.compute_all(current, historical, &mut features).await?;
        Ok(features)
    }
    
    /// Compute only volume indicators
    pub async fn compute_volume_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        if !self.config.enable_volume_weighted {
            return Ok(HashMap::new());
        }
        
        let mut features = HashMap::new();
        let calculator = VolumeIndicators::new(&self.config);
        calculator.compute_all(current, historical, &mut features).await?;
        Ok(features)
    }
    
    /// Compute only advanced indicators
    pub async fn compute_advanced_indicators(
        &self,
        current: &TimeSeriesData,
        historical: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        if !self.config.enable_custom {
            return Ok(HashMap::new());
        }
        
        let mut features = HashMap::new();
        let calculator = AdvancedIndicators::new(&self.config);
        calculator.compute_all(current, historical, &mut features).await?;
        Ok(features)
    }
    
    /// Get list of all available indicators
    pub fn available_indicators(&self) -> Vec<String> {
        let mut indicators = Vec::new();
        
        // Trend indicators
        for period in &self.config.ema_periods {
            indicators.push(format!("ema_{}", period));
            indicators.push(format!("price_to_ema_{}_ratio", period));
        }
        indicators.extend_from_slice(&[
            "macd_line".to_string(),
            "macd_signal".to_string(),
            "macd_histogram".to_string(),
            "macd_crossover".to_string(),
            "adx".to_string(),
            "trending_market".to_string(),
            "ichimoku_tenkan".to_string(),
            "ichimoku_kijun".to_string(),
            "ichimoku_senkou_a".to_string(),
            "ichimoku_senkou_b".to_string(),
            "ichimoku_cloud_thickness".to_string(),
            "ichimoku_position".to_string(),
            "ichimoku_tk_cross".to_string(),
        ]);
        
        // Momentum indicators
        indicators.extend_from_slice(&[
            "rsi".to_string(),
            "rsi_oversold".to_string(),
            "rsi_overbought".to_string(),
            "williams_r".to_string(),
            "cci".to_string(),
            "cci_oversold".to_string(),
            "cci_overbought".to_string(),
            "stochastic_k".to_string(),
            "stochastic_d".to_string(),
            "stochastic_oversold".to_string(),
            "stochastic_overbought".to_string(),
            "ultimate_oscillator".to_string(),
        ]);
        
        for period in &[5, 10, 20] {
            indicators.push(format!("roc_{}", period));
            indicators.push(format!("momentum_{}", period));
        }
        
        // Volatility indicators
        indicators.extend_from_slice(&[
            "atr".to_string(),
            "atr_percentage".to_string(),
            "bb_middle".to_string(),
            "bb_upper".to_string(),
            "bb_lower".to_string(),
            "bb_width".to_string(),
            "bb_width_ratio".to_string(),
            "bb_position".to_string(),
            "bb_squeeze".to_string(),
            "parkinson_volatility".to_string(),
            "garman_klass_volatility".to_string(),
            "rogers_satchell_volatility".to_string(),
            "keltner_middle".to_string(),
            "keltner_upper".to_string(),
            "keltner_lower".to_string(),
            "keltner_position".to_string(),
            "donchian_upper".to_string(),
            "donchian_lower".to_string(),
            "donchian_middle".to_string(),
            "donchian_position".to_string(),
        ]);
        
        for period in &[10, 20, 30] {
            indicators.push(format!("volatility_{}", period));
        }
        
        // Volume indicators (if enabled)
        if self.config.enable_volume_weighted {
            indicators.extend_from_slice(&[
                "volume_roc".to_string(),
                "obv_trend".to_string(),
                "vwap".to_string(),
                "price_to_vwap_ratio".to_string(),
                "mfi".to_string(),
                "mfi_oversold".to_string(),
                "mfi_overbought".to_string(),
                "ad_line_slope".to_string(),
                "volume_above_vwap".to_string(),
                "volume_below_vwap".to_string(),
                "volume_imbalance".to_string(),
                "max_volume_concentration".to_string(),
                "chaikin_money_flow".to_string(),
                "volume_oscillator".to_string(),
                "klinger_oscillator".to_string(),
                "volume_price_trend".to_string(),
                "ease_of_movement".to_string(),
            ]);
        }
        
        // Advanced indicators (if enabled)
        if self.config.enable_custom {
            indicators.extend_from_slice(&[
                "high_low_ratio".to_string(),
                "close_open_ratio".to_string(),
                "close_position_in_range".to_string(),
                "gap_percentage".to_string(),
                "gap_filled".to_string(),
                "price_acceleration".to_string(),
                "price_jerk".to_string(),
                "ha_body_size".to_string(),
                "ha_upper_shadow".to_string(),
                "ha_lower_shadow".to_string(),
                "ha_trend".to_string(),
                "ha_trend_strength".to_string(),
                "value_area_high".to_string(),
                "value_area_low".to_string(),
                "point_of_control".to_string(),
                "price_in_value_area".to_string(),
                "closest_fib_level".to_string(),
                "closest_fib_distance".to_string(),
                "pivot_point".to_string(),
                "nearest_pivot_level".to_string(),
                "nearest_pivot_distance".to_string(),
                "nearest_resistance".to_string(),
                "nearest_support".to_string(),
                "resistance_distance".to_string(),
                "support_distance".to_string(),
                "resistance_strength".to_string(),
                "support_strength".to_string(),
                "market_structure_bullish".to_string(),
                "market_structure_bearish".to_string(),
            ]);
            
            // Fibonacci levels
            for level in &["fib_0", "fib_236", "fib_382", "fib_500", "fib_618", "fib_786", "fib_100"] {
                indicators.push(format!("{}_level", level));
                indicators.push(format!("{}_distance", level));
            }
            
            // Pivot levels
            for level in &["resistance_1", "resistance_2", "resistance_3", "support_1", "support_2", "support_3"] {
                indicators.push(level.to_string());
            }
        }
        
        // Metadata
        indicators.extend_from_slice(&[
            "indicator_count".to_string(),
            "data_length".to_string(),
            "timestamp".to_string(),
        ]);
        
        indicators.sort();
        indicators
    }
    
    /// Get indicator category
    pub fn get_indicator_category(&self, indicator_name: &str) -> Option<&'static str> {
        match indicator_name {
            name if name.starts_with("ema_") || name.starts_with("price_to_ema_") => Some("trend"),
            name if name.starts_with("macd_") => Some("trend"),
            name if name.starts_with("adx") || name == "trending_market" => Some("trend"),
            name if name.starts_with("ichimoku_") => Some("trend"),
            
            name if name.starts_with("rsi") => Some("momentum"),
            name if name.starts_with("roc_") => Some("momentum"),
            name if name.starts_with("momentum_") => Some("momentum"),
            name if name == "williams_r" => Some("momentum"),
            name if name.starts_with("cci") => Some("momentum"),
            name if name.starts_with("stochastic_") => Some("momentum"),
            name if name == "ultimate_oscillator" => Some("momentum"),
            
            name if name.starts_with("atr") => Some("volatility"),
            name if name.starts_with("bb_") => Some("volatility"),
            name if name.starts_with("volatility_") => Some("volatility"),
            name if name.ends_with("_volatility") => Some("volatility"),
            name if name.starts_with("keltner_") => Some("volatility"),
            name if name.starts_with("donchian_") => Some("volatility"),
            
            name if name.starts_with("volume_") => Some("volume"),
            name if name == "obv_trend" => Some("volume"),
            name if name.starts_with("vwap") || name == "price_to_vwap_ratio" => Some("volume"),
            name if name.starts_with("mfi") => Some("volume"),
            name if name == "ad_line_slope" => Some("volume"),
            name if name == "chaikin_money_flow" => Some("volume"),
            name if name == "klinger_oscillator" => Some("volume"),
            name if name == "volume_price_trend" => Some("volume"),
            name if name == "ease_of_movement" => Some("volume"),
            
            name if name.starts_with("ha_") => Some("advanced"),
            name if name.starts_with("fib_") => Some("advanced"),
            name if name.starts_with("value_area_") || name == "point_of_control" => Some("advanced"),
            name if name.contains("pivot") => Some("advanced"),
            name if name.contains("resistance") || name.contains("support") => Some("advanced"),
            name if name.starts_with("market_structure_") => Some("advanced"),
            name if name.contains("elliott") || name.contains("harmonic") => Some("advanced"),
            name if name.ends_with("_ratio") || name.ends_with("_percentage") => Some("advanced"),
            name if name.contains("gap") || name.contains("acceleration") => Some("advanced"),
            
            name if name == "indicator_count" || name == "data_length" || name == "timestamp" => Some("metadata"),
            
            _ => None,
        }
    }
}

impl Default for TechnicalIndicatorEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use chrono::{DateTime, Utc};
    
    fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        for i in 0..count {
            let mut ts_data = TimeSeriesData::new("TEST".to_string(), DateTime::<Utc>::from_timestamp(1640995200 + i as i64 * 60, 0).unwrap());
            ts_data.open = 100.0 + (i as f64 * 0.1).sin();
            ts_data.high = 105.0 + (i as f64 * 0.1).sin();
            ts_data.low = 95.0 + (i as f64 * 0.1).sin();
            ts_data.close = 102.0 + (i as f64 * 0.1).sin();
            ts_data.volume = vec![1000.0 + i as f64 * 10.0];
            data.push(ts_data);
        }
        data
    }
    
    #[tokio::test]
    async fn test_compute_all_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_test_data(100);
        let current = &data[data.len() - 1];
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_all(current, historical).await.unwrap();
        
        assert!(!features.is_empty());
        assert!(features.contains_key("indicator_count"));
        assert!(features.contains_key("rsi"));
        assert!(features.contains_key("ema_21"));
        assert!(features.contains_key("atr"));
    }
    
    #[tokio::test]
    async fn test_compute_trend_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_test_data(100);
        let current = &data[data.len() - 1];
        let historical = &data[..data.len() - 1];
        
        let features = engine.compute_trend_indicators(current, historical).await.unwrap();
        
        assert!(features.contains_key("ema_21"));
        assert!(features.contains_key("macd_line"));
        assert!(!features.contains_key("rsi")); // Should not contain momentum indicators
    }
    
    #[tokio::test]
    async fn test_available_indicators() {
        let engine = TechnicalIndicatorEngine::new();
        let indicators = engine.available_indicators();
        
        assert!(!indicators.is_empty());
        assert!(indicators.contains(&"rsi".to_string()));
        assert!(indicators.contains(&"ema_21".to_string()));
        assert!(indicators.contains(&"atr".to_string()));
    }
    
    #[test]
    fn test_indicator_categories() {
        let engine = TechnicalIndicatorEngine::new();
        
        assert_eq!(engine.get_indicator_category("rsi"), Some("momentum"));
        assert_eq!(engine.get_indicator_category("ema_21"), Some("trend"));
        assert_eq!(engine.get_indicator_category("atr"), Some("volatility"));
        assert_eq!(engine.get_indicator_category("vwap"), Some("volume"));
        assert_eq!(engine.get_indicator_category("fib_618_level"), Some("advanced"));
        assert_eq!(engine.get_indicator_category("invalid_indicator"), None);
    }
    
    #[test]
    fn test_configuration() {
        let mut engine = TechnicalIndicatorEngine::new();
        let config = IndicatorConfig::high_frequency();
        
        engine.set_config(config.clone());
        assert_eq!(engine.config().rsi_period, config.rsi_period);
    }
    
    #[tokio::test]
    async fn test_insufficient_data() {
        let engine = TechnicalIndicatorEngine::new();
        let data = create_test_data(5); // Very small dataset
        let current = &data[data.len() - 1];
        let historical = &data[..data.len() - 1];
        
        let result = engine.compute_all(current, historical).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_disabled_features() {
        let mut config = IndicatorConfig::default();
        config.enable_volume_weighted = false;
        config.enable_custom = false;
        
        let engine = TechnicalIndicatorEngine::with_config(config);
        let data = create_test_data(100);
        let current = &data[data.len() - 1];
        let historical = &data[..data.len() - 1];
        
        let volume_features = engine.compute_volume_indicators(current, historical).await.unwrap();
        let advanced_features = engine.compute_advanced_indicators(current, historical).await.unwrap();
        
        assert!(volume_features.is_empty());
        assert!(advanced_features.is_empty());
    }
}