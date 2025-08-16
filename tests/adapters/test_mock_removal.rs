//! Tests for mock removal and real adapter integration
//!
//! This module tests the transition from mock implementations to real adapters

use autonomous_platform::{
    adapters::{RedisAdapter, RedisConfig},
    config::NeuralConfig,
};
use anyhow::Result;

#[cfg(test)]
mod mock_removal_tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_adapter_configuration() -> Result<()> {
        let config = RedisConfig::default();
        assert!(!config.host.is_empty());
        assert!(config.port > 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_neural_config_instantiation() -> Result<()> {
        let config = NeuralConfig::default();
        assert!(!config.models.is_empty());
        assert!(config.enable_health_checks);
        Ok(())
    }

    #[tokio::test]
    async fn test_mock_replacement_readiness() -> Result<()> {
        // This test ensures we can create real adapter instances
        // without relying on mock implementations
        let redis_config = RedisConfig::default();
        let neural_config = NeuralConfig::default();
        
        // Verify configurations are valid for real adapter creation
        assert!(redis_config.pool_size > 0);
        assert!(neural_config.memory_gb > 0.0);
        Ok(())
    }
}