//! Configuration module for the autonomous platform
//! 
//! This module handles loading configuration from TOML files and environment variables.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

/// Main platform configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub platform: PlatformInfo,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub neural: NeuralConfig,
    pub monitoring: MonitoringConfig,
}

/// Platform metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub name: String,
    pub version: String,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub default_ttl_seconds: u64,
}

/// Neural model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralConfig {
    pub memory_gb: f32,
    pub models: Vec<String>,
    pub prediction_cache_ttl: u64,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub metrics_interval_secs: u64,
    pub quality_threshold: f64,
}

impl PlatformConfig {
    /// Load configuration from file with environment variable overrides
    pub fn load(config_path: impl AsRef<Path>) -> Result<Self> {
        // Load from TOML file
        let config_content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path.as_ref()))?;
        
        let mut config: PlatformConfig = toml::from_str(&config_content)
            .context("Failed to parse configuration TOML")?;
        
        // Apply environment variable overrides
        config.apply_env_overrides()?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) -> Result<()> {
        // Database overrides
        if let Ok(url) = env::var("DATABASE_URL") {
            self.database.url = url;
        }
        if let Ok(max_conn) = env::var("DATABASE_MAX_CONNECTIONS") {
            self.database.max_connections = max_conn.parse()
                .context("Invalid DATABASE_MAX_CONNECTIONS")?;
        }
        if let Ok(min_conn) = env::var("DATABASE_MIN_CONNECTIONS") {
            self.database.min_connections = min_conn.parse()
                .context("Invalid DATABASE_MIN_CONNECTIONS")?;
        }
        
        // Redis overrides
        if let Ok(url) = env::var("REDIS_URL") {
            self.redis.url = url;
        }
        if let Ok(max_conn) = env::var("REDIS_MAX_CONNECTIONS") {
            self.redis.max_connections = max_conn.parse()
                .context("Invalid REDIS_MAX_CONNECTIONS")?;
        }
        if let Ok(ttl) = env::var("REDIS_DEFAULT_TTL_SECONDS") {
            self.redis.default_ttl_seconds = ttl.parse()
                .context("Invalid REDIS_DEFAULT_TTL_SECONDS")?;
        }
        
        // Neural overrides
        if let Ok(memory) = env::var("NEURAL_MEMORY_GB") {
            self.neural.memory_gb = memory.parse()
                .context("Invalid NEURAL_MEMORY_GB")?;
        }
        if let Ok(models) = env::var("NEURAL_MODELS") {
            self.neural.models = models.split(',').map(String::from).collect();
        }
        if let Ok(ttl) = env::var("NEURAL_PREDICTION_CACHE_TTL") {
            self.neural.prediction_cache_ttl = ttl.parse()
                .context("Invalid NEURAL_PREDICTION_CACHE_TTL")?;
        }
        
        // Monitoring overrides
        if let Ok(interval) = env::var("MONITORING_METRICS_INTERVAL_SECS") {
            self.monitoring.metrics_interval_secs = interval.parse()
                .context("Invalid MONITORING_METRICS_INTERVAL_SECS")?;
        }
        if let Ok(threshold) = env::var("MONITORING_QUALITY_THRESHOLD") {
            self.monitoring.quality_threshold = threshold.parse()
                .context("Invalid MONITORING_QUALITY_THRESHOLD")?;
        }
        
        Ok(())
    }
    
    /// Validate all configuration settings
    fn validate(&self) -> Result<()> {
        // Validate database settings
        if self.database.url.is_empty() {
            anyhow::bail!("Database URL cannot be empty");
        }
        if self.database.min_connections > self.database.max_connections {
            anyhow::bail!("Database min_connections cannot exceed max_connections");
        }
        if self.database.max_connections == 0 {
            anyhow::bail!("Database max_connections must be greater than 0");
        }
        
        // Validate Redis settings
        if self.redis.url.is_empty() {
            anyhow::bail!("Redis URL cannot be empty");
        }
        if self.redis.max_connections == 0 {
            anyhow::bail!("Redis max_connections must be greater than 0");
        }
        
        // Validate neural settings
        if self.neural.memory_gb <= 0.0 {
            anyhow::bail!("Neural memory_gb must be positive");
        }
        if self.neural.models.is_empty() {
            anyhow::bail!("At least one neural model must be configured");
        }
        
        // Validate monitoring settings
        if self.monitoring.metrics_interval_secs == 0 {
            anyhow::bail!("Monitoring metrics_interval_secs must be greater than 0");
        }
        if !(0.0..=1.0).contains(&self.monitoring.quality_threshold) {
            anyhow::bail!("Monitoring quality_threshold must be between 0 and 1");
        }
        
        Ok(())
    }
}

/// Default configuration path
pub const DEFAULT_CONFIG_PATH: &str = "config/platform.toml";

/// Load default configuration
pub fn load_default_config() -> Result<PlatformConfig> {
    PlatformConfig::load(DEFAULT_CONFIG_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[test]
    fn test_load_valid_config() {
        let config_content = r#"
[platform]
name = "test-platform"
version = "0.1.0"

[database]
url = "postgres://test@localhost/test"
max_connections = 10
min_connections = 2

[redis]
url = "redis://localhost:6379"
max_connections = 5
default_ttl_seconds = 300

[neural]
memory_gb = 2.0
models = ["NHITS", "DeepAR"]
prediction_cache_ttl = 600

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        
        let config = PlatformConfig::load(temp_file.path()).unwrap();
        assert_eq!(config.platform.name, "test-platform");
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.neural.models.len(), 2);
    }
    
    #[test]
    fn test_env_override() {
        let config_content = r#"
[platform]
name = "test-platform"
version = "0.1.0"

[database]
url = "postgres://test@localhost/test"
max_connections = 10
min_connections = 2

[redis]
url = "redis://localhost:6379"
max_connections = 5
default_ttl_seconds = 300

[neural]
memory_gb = 2.0
models = ["NHITS"]
prediction_cache_ttl = 600

[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        
        // Set environment variable
        env::set_var("DATABASE_MAX_CONNECTIONS", "20");
        
        let config = PlatformConfig::load(temp_file.path()).unwrap();
        assert_eq!(config.database.max_connections, 20);
        
        // Clean up
        env::remove_var("DATABASE_MAX_CONNECTIONS");
    }
    
    #[test]
    fn test_validation_errors() {
        let invalid_configs = vec![
            // Empty database URL
            r#"
[platform]
name = "test"
version = "1.0"
[database]
url = ""
max_connections = 10
min_connections = 2
[redis]
url = "redis://localhost"
max_connections = 5
default_ttl_seconds = 300
[neural]
memory_gb = 1.0
models = ["test"]
prediction_cache_ttl = 300
[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#,
            // Invalid connection settings
            r#"
[platform]
name = "test"
version = "1.0"
[database]
url = "postgres://test"
max_connections = 5
min_connections = 10
[redis]
url = "redis://localhost"
max_connections = 5
default_ttl_seconds = 300
[neural]
memory_gb = 1.0
models = ["test"]
prediction_cache_ttl = 300
[monitoring]
metrics_interval_secs = 30
quality_threshold = 0.9
"#,
        ];
        
        for config_content in invalid_configs {
            let mut temp_file = NamedTempFile::new().unwrap();
            temp_file.write_all(config_content.as_bytes()).unwrap();
            
            assert!(PlatformConfig::load(temp_file.path()).is_err());
        }
    }
}