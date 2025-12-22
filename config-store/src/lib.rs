//! Configuration Store - A hierarchical configuration management system
//!
//! This crate provides a trait-based configuration store system that supports
//! hierarchical organization, inheritance, versioning, and multiple storage backends.

pub mod configs;
pub mod platform_config;
pub mod secure_async_store;
pub mod security;
pub mod stores;
pub mod traits;
pub mod types;

#[cfg(test)]
mod traits_tests;

// Legacy modules for backward compatibility (but commented out to avoid conflicts)
// pub mod error;
// pub mod in_memory;
// pub mod redis_store;

// Re-export specification-compliant types
pub use types::{
    ConfigError, ConfigMetadata, ConfigNode, ConfigSnapshot, ConfigTree, ConfigValue, ConfigVersion,
};

pub use traits::{path_utils, ConfigStore, ConfigTransaction};

pub use stores::InMemoryConfigStore;

// Re-export platform configuration types
pub use platform_config::{
    load_config_for_environment, load_default_config, load_development_config,
    load_production_config, ConfigBuilder, DevelopmentConfig, PlatformConfig, PlatformInfo,
};

// Re-export configuration types
pub use configs::{
    AlertsConfig, AuthConfig, BackupConfig, CircuitBreakerConfig, DatabaseConfig, EncryptionConfig,
    EnhancedNeuralConfig, EnsembleConfig, FeatureFlags, GracefulShutdownConfig, LoggingConfig,
    MonitoringConfig, NeuralConfig, ObservabilityConfig, PerformanceConfig, RedisConfig,
    SecurityConfig, TrainingConfig,
};

// Re-export commonly used types
pub use serde_json::Value as JsonValue;
pub use std::sync::Arc;
