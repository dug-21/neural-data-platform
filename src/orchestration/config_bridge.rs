//! Configuration bridge for converting platform URLs to adapter configs
//!
//! This module provides the integration between platform configuration (using URLs)
//! and adapter configurations (using individual connection parameters).

use super::config_utils::{parse_postgres_url, parse_redis_url};
use crate::adapters::{redis::RedisConfig, timescale::TimescaleConfig};
use crate::config::PlatformConfig;
use anyhow::Result;

/// Configuration bridge for converting platform config to adapter configs
pub struct ConfigBridge;

impl ConfigBridge {
    /// Create RedisConfig from platform configuration
    ///
    /// # Arguments
    ///
    /// * `platform_config` - The platform configuration containing Redis URL
    ///
    /// # Returns
    ///
    /// Returns a `RedisConfig` with parsed connection parameters from the URL
    ///
    /// # Examples
    ///
    /// ```rust
    /// use neural_trader::orchestration::config_bridge::ConfigBridge;
    /// use neural_trader::config::PlatformConfig;
    ///
    /// let platform_config = PlatformConfig::load("config/platform.toml")?;
    /// let redis_config = ConfigBridge::redis_config_from_platform(&platform_config)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn redis_config_from_platform(platform_config: &PlatformConfig) -> Result<RedisConfig> {
        let mut redis_config = parse_redis_url(&platform_config.redis.url)?;

        // Override with platform-specific settings
        redis_config.pool_size = platform_config.redis.max_connections;

        Ok(redis_config)
    }

    /// Create TimescaleConfig from platform configuration
    ///
    /// # Arguments
    ///
    /// * `platform_config` - The platform configuration containing database URL
    ///
    /// # Returns
    ///
    /// Returns a `TimescaleConfig` with parsed connection parameters from the URL
    ///
    /// # Examples
    ///
    /// ```rust
    /// use neural_trader::orchestration::config_bridge::ConfigBridge;
    /// use neural_trader::config::PlatformConfig;
    ///
    /// let platform_config = PlatformConfig::load("config/platform.toml")?;
    /// let timescale_config = ConfigBridge::timescale_config_from_platform(&platform_config)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn timescale_config_from_platform(
        platform_config: &PlatformConfig,
    ) -> Result<TimescaleConfig> {
        let mut timescale_config = parse_postgres_url(&platform_config.database.url)?;

        // Override with platform-specific settings
        timescale_config.max_connections = platform_config.database.max_connections;

        Ok(timescale_config)
    }

    /// Create both Redis and TimescaleDB configs from platform configuration
    ///
    /// # Arguments
    ///
    /// * `platform_config` - The platform configuration
    ///
    /// # Returns
    ///
    /// Returns a tuple of (RedisConfig, TimescaleConfig) with parsed connection parameters
    pub fn adapter_configs_from_platform(
        platform_config: &PlatformConfig,
    ) -> Result<(RedisConfig, TimescaleConfig)> {
        let redis_config = Self::redis_config_from_platform(platform_config)?;
        let timescale_config = Self::timescale_config_from_platform(platform_config)?;

        Ok((redis_config, timescale_config))
    }

    /// Validate that both Redis and database URLs are properly formatted
    ///
    /// # Arguments
    ///
    /// * `platform_config` - The platform configuration to validate
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if both URLs are valid, or an error with details
    pub fn validate_connection_urls(platform_config: &PlatformConfig) -> Result<()> {
        // Validate Redis URL
        parse_redis_url(&platform_config.redis.url)
            .map_err(|e| anyhow::anyhow!("Invalid Redis URL: {}", e))?;

        // Validate PostgreSQL URL
        parse_postgres_url(&platform_config.database.url)
            .map_err(|e| anyhow::anyhow!("Invalid PostgreSQL URL: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        DatabaseConfig, MonitoringConfig, NeuralConfig, PlatformInfo,
        RedisConfig as PlatformRedisConfig,
    };

    fn create_test_platform_config() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformInfo {
                name: "test-platform".to_string(),
                version: "1.0.0".to_string(),
                environment: "development".to_string(),
                log_level: "info".to_string(),
            },
            feature_flags: crate::config::FeatureFlags::default(),
            database: DatabaseConfig {
                url: "postgres://user:pass@localhost:5432/testdb".to_string(),
                max_connections: 20,
                min_connections: 5,
                connection_timeout: 30,
                idle_timeout: 600,
                max_query_time: 30,
            },
            redis: PlatformRedisConfig {
                url: "redis://:mypass@localhost:6379/1".to_string(),
                max_connections: 15,
                default_ttl_seconds: 300,
                connection_timeout_ms: 5000,
                cluster_mode: false,
                pool_max_idle: 10,
                pool_timeout_seconds: 30,
            },
            neural: NeuralConfig {
                memory_gb: 2.0,
                models: vec!["NHITS".to_string()],
                prediction_cache_ttl: 600,
                model_load_timeout: 300,
                max_concurrent_predictions: 50,
                enable_model_monitoring: true,
                accuracy_threshold: 0.85,
                use_real_models: false,
                enable_health_checks: true,
                enable_fallback: true,
                enable_circuit_breakers: true,
                enable_graceful_degradation: false,
                enable_performance_monitoring: true,
                enable_adaptive_retry: true,
                enable_model_ensembles: false,
                model_timeout_seconds: 300,
                max_retries: 3,
                error_threshold: 0.05,
                lookback_window: 24,
            },
            monitoring: MonitoringConfig {
                metrics_interval_secs: 30,
                quality_threshold: 0.9,
                prometheus_port: Some(8080),
                prometheus_path: "/metrics".to_string(),
                enable_performance_metrics: true,
                enable_memory_monitoring: true,
                enable_error_monitoring: true,
                cpu_usage_threshold: 80.0,
                memory_usage_threshold: 85.0,
                error_rate_threshold: 0.05,
            },
            observability: Default::default(),
            security: Default::default(),
            performance: Default::default(),
            logging: Default::default(),
            alerts: Default::default(),
            backup: Default::default(),
            circuit_breaker: Default::default(),
            graceful_shutdown: Default::default(),
            development: Default::default(),
        }
    }

    #[test]
    fn test_redis_config_from_platform() {
        let platform_config = create_test_platform_config();
        let redis_config = ConfigBridge::redis_config_from_platform(&platform_config).unwrap();

        assert_eq!(redis_config.host, "localhost");
        assert_eq!(redis_config.port, 6379);
        assert_eq!(redis_config.password, Some("mypass".to_string()));
        assert_eq!(redis_config.db, 1);
        assert_eq!(redis_config.pool_size, 15); // From platform config
    }

    #[test]
    fn test_timescale_config_from_platform() {
        let platform_config = create_test_platform_config();
        let timescale_config =
            ConfigBridge::timescale_config_from_platform(&platform_config).unwrap();

        assert_eq!(timescale_config.host, "localhost");
        assert_eq!(timescale_config.port, 5432);
        assert_eq!(timescale_config.username, "user");
        assert_eq!(timescale_config.password, "pass");
        assert_eq!(timescale_config.database, "testdb");
        assert_eq!(timescale_config.max_connections, 20); // From platform config
    }

    #[test]
    fn test_adapter_configs_from_platform() {
        let platform_config = create_test_platform_config();
        let (redis_config, timescale_config) =
            ConfigBridge::adapter_configs_from_platform(&platform_config).unwrap();

        // Test Redis config
        assert_eq!(redis_config.host, "localhost");
        assert_eq!(redis_config.port, 6379);
        assert_eq!(redis_config.pool_size, 15);

        // Test TimescaleDB config
        assert_eq!(timescale_config.host, "localhost");
        assert_eq!(timescale_config.port, 5432);
        assert_eq!(timescale_config.max_connections, 20);
    }

    #[test]
    fn test_validate_connection_urls_success() {
        let platform_config = create_test_platform_config();
        assert!(ConfigBridge::validate_connection_urls(&platform_config).is_ok());
    }

    #[test]
    fn test_validate_connection_urls_invalid_redis() {
        let mut platform_config = create_test_platform_config();
        platform_config.redis.url = "invalid://not-a-real-redis-url".to_string();

        assert!(ConfigBridge::validate_connection_urls(&platform_config).is_err());
    }

    #[test]
    fn test_validate_connection_urls_invalid_postgres() {
        let mut platform_config = create_test_platform_config();
        platform_config.database.url = "mysql://wrong-scheme@localhost/db".to_string();

        assert!(ConfigBridge::validate_connection_urls(&platform_config).is_err());
    }
}
