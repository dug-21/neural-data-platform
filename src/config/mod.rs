//! Modular Configuration System
//!
//! This module provides a clean, modular configuration architecture that breaks down
//! the large monolithic config.rs into focused, maintainable components.
//!
//! ## Migration Strategy
//! 
//! This modular config system runs alongside the legacy config.rs to ensure compatibility.
//! New components should use the modular system for better maintainability.

// Legacy configuration for backward compatibility - REMOVED (Phase 3A cleanup)

pub mod neural;
pub mod database;
pub mod monitoring;
pub mod security;

// Re-export main configuration types
pub use neural::{NeuralConfig, TrainingConfig, EnsembleConfig};
pub use database::{DatabaseConfig, RedisConfig, BackupConfig};
pub use monitoring::{MonitoringConfig, ObservabilityConfig, LoggingConfig, AlertsConfig, PerformanceConfig};
pub use security::{SecurityConfig, CircuitBreakerConfig, GracefulShutdownConfig, AuthConfig, EncryptionConfig};

// Re-export types defined in this module
pub use {PlatformInfo, DevelopmentConfig, FeatureFlags, ModularPlatformConfig};

// Re-export modular types as primary API - legacy aliases below for backward compatibility
pub use ModularPlatformConfig as PlatformConfig;


use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

/// Main platform configuration - modular version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModularPlatformConfig {
    pub platform: PlatformInfo,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub neural: NeuralConfig,
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
    #[serde(default)]
    pub graceful_shutdown: GracefulShutdownConfig,
    #[serde(default)]
    pub development: DevelopmentConfig,
    #[serde(default)]
    pub feature_flags: FeatureFlags,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default)]
    pub training: TrainingConfig,
    #[serde(default)]
    pub ensemble: EnsembleConfig,
}

/// Platform metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub name: String,
    pub version: String,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Development configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentConfig {
    #[serde(default = "default_false")]
    pub enable_debug_mode: bool,
    #[serde(default = "default_false")]
    pub enable_hot_reload: bool,
    #[serde(default = "default_true")]
    pub enable_detailed_logging: bool,
    #[serde(default = "default_false")]
    pub enable_profiling: bool,
}

impl Default for DevelopmentConfig {
    fn default() -> Self {
        Self {
            enable_debug_mode: default_false(),
            enable_hot_reload: default_false(),
            enable_detailed_logging: default_true(),
            enable_profiling: default_false(),
        }
    }
}

/// Feature flags configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    #[serde(default = "default_true")]
    pub enable_enhanced_neural_adapter: bool,
    #[serde(default = "default_false")]
    pub enable_experimental_models: bool,
    #[serde(default = "default_true")]
    pub enable_performance_monitoring: bool,
    #[serde(default = "default_false")]
    pub enable_advanced_analytics: bool,
    #[serde(default = "default_true")]
    pub enable_caching: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enable_enhanced_neural_adapter: default_true(),
            enable_experimental_models: default_false(),
            enable_performance_monitoring: default_true(),
            enable_advanced_analytics: default_false(),
            enable_caching: default_true(),
        }
    }
}

impl Default for ModularPlatformConfig {
    fn default() -> Self {
        Self {
            platform: PlatformInfo {
                name: "Neural Trader".to_string(),
                version: "1.0.0".to_string(),
                environment: default_environment(),
                log_level: default_log_level(),
            },
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            neural: NeuralConfig::default(),
            monitoring: MonitoringConfig::default(),
            observability: ObservabilityConfig::default(),
            security: SecurityConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            alerts: AlertsConfig::default(),
            backup: BackupConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            graceful_shutdown: GracefulShutdownConfig::default(),
            development: DevelopmentConfig::default(),
            feature_flags: FeatureFlags::default(),
            auth: AuthConfig::default(),
            encryption: EncryptionConfig::default(),
            training: TrainingConfig::default(),
            ensemble: EnsembleConfig::default(),
        }
    }
}

impl ModularPlatformConfig {
    /// Load configuration from file with environment variable overrides
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;
        
        let mut config: Self = toml::from_str(&config_str)
            .with_context(|| "Failed to parse config file as TOML")?;
        
        // Apply environment variable overrides
        config.apply_environment_overrides();
        
        Ok(config)
    }
    
    /// Apply environment variable overrides
    pub fn apply_environment_overrides(&mut self) {
        // Database overrides
        if let Ok(url) = env::var("DATABASE_URL") {
            self.database.url = url;
        }
        
        // Redis overrides
        if let Ok(url) = env::var("REDIS_URL") {
            self.redis.url = url;
        }
        
        // Neural config overrides
        if let Ok(use_real_models) = env::var("NEURAL_USE_REAL_MODELS") {
            self.neural.use_real_models = use_real_models.parse().unwrap_or(false);
        }
        
        // Security overrides
        if let Ok(api_key) = env::var("API_KEY") {
            self.security.api_key = Some(api_key);
        }
        
        // Logging level override
        if let Ok(log_level) = env::var("LOG_LEVEL") {
            self.platform.log_level = log_level;
            self.logging.level = self.platform.log_level.clone();
        }
        
        // Development mode override
        if let Ok(debug_mode) = env::var("DEBUG_MODE") {
            self.development.enable_debug_mode = debug_mode.parse().unwrap_or(false);
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate neural config
        if self.neural.input_size == 0 {
            return Err(anyhow::anyhow!("Neural input size must be greater than 0"));
        }
        
        if self.neural.output_size == 0 {
            return Err(anyhow::anyhow!("Neural output size must be greater than 0"));
        }
        
        // Validate database config
        if self.database.url.is_empty() {
            return Err(anyhow::anyhow!("Database URL cannot be empty"));
        }
        
        // Validate monitoring config
        if self.monitoring.metrics_interval_secs == 0 {
            return Err(anyhow::anyhow!("Metrics interval must be greater than 0"));
        }
        
        Ok(())
    }
    
    /// Get a subset configuration for specific components
    pub fn get_neural_config(&self) -> &NeuralConfig {
        &self.neural
    }
    
    pub fn get_database_config(&self) -> &DatabaseConfig {
        &self.database
    }
    
    pub fn get_monitoring_config(&self) -> &MonitoringConfig {
        &self.monitoring
    }
    
    pub fn get_security_config(&self) -> &SecurityConfig {
        &self.security
    }
}

// Default value functions
fn default_environment() -> String { "development".to_string() }
fn default_log_level() -> String { "info".to_string() }
fn default_true() -> bool { true }
fn default_false() -> bool { false }

/// Default configuration path
pub const DEFAULT_CONFIG_PATH: &str = "config/platform.toml";

/// Load default configuration using modular system
pub fn load_default_config() -> Result<PlatformConfig> {
    PlatformConfig::load_from_file(DEFAULT_CONFIG_PATH)
}

/// Load production configuration
pub fn load_production_config() -> Result<PlatformConfig> {
    PlatformConfig::load_from_file("config/production.toml")
}

/// Load development configuration  
pub fn load_development_config() -> Result<PlatformConfig> {
    PlatformConfig::load_from_file("config/development.toml")
}

/// Get configuration based on environment
pub fn load_config_for_environment(environment: &str) -> Result<PlatformConfig> {
    match environment {
        "production" => load_production_config(),
        "development" => load_development_config(),
        _ => load_default_config(),
    }
}

/// Configuration builder for easier construction
pub struct ConfigBuilder {
    config: ModularPlatformConfig,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ModularPlatformConfig::default(),
        }
    }
    
    pub fn with_neural_config(mut self, neural_config: NeuralConfig) -> Self {
        self.config.neural = neural_config;
        self
    }
    
    pub fn with_database_config(mut self, database_config: DatabaseConfig) -> Self {
        self.config.database = database_config;
        self
    }
    
    pub fn with_monitoring_config(mut self, monitoring_config: MonitoringConfig) -> Self {
        self.config.monitoring = monitoring_config;
        self
    }
    
    pub fn with_security_config(mut self, security_config: SecurityConfig) -> Self {
        self.config.security = security_config;
        self
    }
    
    pub fn with_environment(mut self, environment: String) -> Self {
        self.config.platform.environment = environment;
        self
    }
    
    pub fn build(self) -> Result<ModularPlatformConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = ModularPlatformConfig::default();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .with_environment("test".to_string())
            .build();
        
        assert!(config.is_ok());
        assert_eq!(config.unwrap().platform.environment, "test");
    }
    
    #[test]
    fn test_environment_overrides() {
        env::set_var("DATABASE_URL", "test://localhost");
        env::set_var("NEURAL_USE_REAL_MODELS", "true");
        
        let mut config = ModularPlatformConfig::default();
        config.apply_environment_overrides();
        
        assert_eq!(config.database.url, "test://localhost");
        assert_eq!(config.neural.use_real_models, true);
        
        // Cleanup
        env::remove_var("DATABASE_URL");
        env::remove_var("NEURAL_USE_REAL_MODELS");
    }
}

// Re-export the load functions at the end of the module
pub use {load_default_config, load_production_config, load_development_config, load_config_for_environment};