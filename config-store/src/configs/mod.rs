//! Configuration modules for the config-store
//!
//! This module organizes all configuration types into logical groups
//! for better maintainability and modularity.

pub mod database;
pub mod feature_flags;
pub mod monitoring;
pub mod neural_base;
pub mod neural_enhanced;
pub mod security;

// Re-export commonly used types for convenience
pub use database::{DatabaseConfig, RedisConfig, BackupConfig};
pub use feature_flags::FeatureFlags;
pub use monitoring::{
    MonitoringConfig, ObservabilityConfig, LoggingConfig, 
    AlertsConfig, PerformanceConfig
};
pub use neural_base::{NeuralConfig, TrainingConfig, EnsembleConfig};
pub use neural_enhanced::EnhancedNeuralConfig;
pub use security::{
    SecurityConfig, CircuitBreakerConfig, GracefulShutdownConfig,
    AuthConfig, EncryptionConfig, OAuthProvider
};