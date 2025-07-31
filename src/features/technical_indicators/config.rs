//! Configuration for technical indicators
//! 
//! This module contains configuration structures and defaults for
//! all technical indicator calculations.

use anyhow::Result;

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

impl IndicatorConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create configuration optimized for high-frequency trading
    pub fn high_frequency() -> Self {
        Self {
            ema_periods: vec![5, 13, 21, 34, 55],
            rsi_period: 7,
            macd_params: (8, 17, 9),
            bb_params: (10, 2.0),
            atr_period: 7,
            stoch_params: (8, 3),
            enable_volume_weighted: true,
            enable_custom: true,
        }
    }
    
    /// Create configuration for swing trading
    pub fn swing_trading() -> Self {
        Self {
            ema_periods: vec![21, 50, 100, 200],
            rsi_period: 21,
            macd_params: (12, 26, 9),
            bb_params: (20, 2.0),
            atr_period: 21,
            stoch_params: (21, 5),
            enable_volume_weighted: true,
            enable_custom: true,
        }
    }
    
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.ema_periods.is_empty() {
            return Err(anyhow::anyhow!("EMA periods cannot be empty"));
        }
        
        if self.rsi_period < 2 {
            return Err(anyhow::anyhow!("RSI period must be at least 2"));
        }
        
        let (fast, slow, signal) = self.macd_params;
        if fast >= slow {
            return Err(anyhow::anyhow!("MACD fast period must be less than slow period"));
        }
        
        if signal < 1 {
            return Err(anyhow::anyhow!("MACD signal period must be at least 1"));
        }
        
        let (period, std_dev) = self.bb_params;
        if period < 2 {
            return Err(anyhow::anyhow!("Bollinger Bands period must be at least 2"));
        }
        
        if std_dev <= 0.0 {
            return Err(anyhow::anyhow!("Bollinger Bands standard deviation must be positive"));
        }
        
        if self.atr_period < 1 {
            return Err(anyhow::anyhow!("ATR period must be at least 1"));
        }
        
        let (k_period, d_period) = self.stoch_params;
        if k_period < 1 || d_period < 1 {
            return Err(anyhow::anyhow!("Stochastic periods must be at least 1"));
        }
        
        Ok(())
    }
    
    /// Get minimum required historical data length for all indicators
    pub fn min_required_data_length(&self) -> usize {
        let mut max_period = self.ema_periods.iter().max().copied().unwrap_or(0);
        max_period = max_period.max(self.rsi_period + 1);
        max_period = max_period.max(self.macd_params.1); // slow period
        max_period = max_period.max(self.bb_params.0);
        max_period = max_period.max(self.atr_period + 1);
        max_period = max_period.max(self.stoch_params.0);
        
        // Add buffer for advanced indicators
        if self.enable_custom {
            max_period = max_period.max(240); // Elliott Wave analysis needs 240 periods
        }
        
        max_period
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = IndicatorConfig::default();
        assert!(!config.ema_periods.is_empty());
        assert!(config.rsi_period > 0);
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_high_frequency_config() {
        let config = IndicatorConfig::high_frequency();
        assert!(config.validate().is_ok());
        assert!(config.rsi_period < IndicatorConfig::default().rsi_period);
    }
    
    #[test]
    fn test_swing_trading_config() {
        let config = IndicatorConfig::swing_trading();
        assert!(config.validate().is_ok());
        assert!(config.rsi_period > IndicatorConfig::default().rsi_period);
    }
    
    #[test]
    fn test_invalid_config() {
        let mut config = IndicatorConfig::default();
        config.macd_params = (26, 12, 9); // fast > slow
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_min_required_data_length() {
        let config = IndicatorConfig::default();
        let min_length = config.min_required_data_length();
        assert!(min_length >= 240); // Should include Elliott Wave requirement
    }
}