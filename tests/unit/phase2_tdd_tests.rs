//! Phase 2 TDD Tests for Neural Trader
//!
//! These tests were generated as part of the Phase 2 development cycle
//! focusing on Test-Driven Development principles for core functionality.

use autonomous_platform::{
    config::{ModularPlatformConfig, NeuralConfig},
    data::TimeSeriesData,
    neural::NeuralPredictor,
};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;

#[cfg(test)]
mod phase2_tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_neural_config_creation() -> Result<()> {
        let config = NeuralConfig::default();
        assert!(!config.models.is_empty());
        assert!(config.memory_gb > 0.0);
        assert!(config.enable_health_checks);
        Ok(())
    }

    #[tokio::test]
    async fn test_time_series_data_creation() -> Result<()> {
        let data = TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: 50000.0,
            high: 51000.0,
            low: 49000.0,
            close: 50500.0,
            volume: vec![1000.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(50500.0),
            metadata: None,
            values: vec![50000.0, 50500.0],
            timestamps: vec![Utc::now()],
            metadata_map: HashMap::new(),
        };

        assert_eq!(data.symbol, "BTC/USD");
        assert!(data.close > data.low);
        assert!(!data.values.is_empty());
        assert!(!data.timestamps.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_modular_platform_config_defaults() -> Result<()> {
        let config = ModularPlatformConfig::default();
        assert_eq!(config.platform.name, "Neural Trader");
        assert!(!config.neural.models.is_empty());
        assert!(config.monitoring.enable_health_checks);
        Ok(())
    }
}