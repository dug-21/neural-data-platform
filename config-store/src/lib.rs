//! Configuration Store - A hierarchical configuration management system
//! 
//! This crate provides a trait-based configuration store system that supports
//! hierarchical organization, inheritance, versioning, and multiple storage backends.

pub mod types;
pub mod traits;
pub mod stores;
pub mod security;
pub mod secure_async_store;
pub mod configs;
pub mod platform_config;

#[cfg(test)]
mod traits_tests;

// Legacy modules for backward compatibility (but commented out to avoid conflicts)
// pub mod error;
// pub mod in_memory;
// pub mod redis_store;

// Re-export specification-compliant types
pub use types::{
    ConfigValue, ConfigError, ConfigTree, ConfigNode, 
    ConfigMetadata, ConfigVersion, ConfigSnapshot
};

pub use traits::{ConfigStore, ConfigTransaction, path_utils};

pub use stores::{InMemoryConfigStore};

// Re-export platform configuration types
pub use platform_config::{
    PlatformConfig, PlatformInfo, DevelopmentConfig,
    ConfigBuilder, load_default_config, load_production_config,
    load_development_config, load_config_for_environment
};

// Re-export configuration types
pub use configs::{
    DatabaseConfig, RedisConfig, BackupConfig,
    MonitoringConfig, ObservabilityConfig, LoggingConfig,
    AlertsConfig, PerformanceConfig,
    NeuralConfig, TrainingConfig, EnsembleConfig,
    EnhancedNeuralConfig,
    SecurityConfig, CircuitBreakerConfig, GracefulShutdownConfig,
    AuthConfig, EncryptionConfig,
    FeatureFlags
};

// Re-export commonly used types
pub use serde_json::Value as JsonValue;
pub use std::sync::Arc;