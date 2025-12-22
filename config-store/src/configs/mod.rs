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
pub use database::{BackupConfig, DatabaseConfig, RedisConfig};
pub use feature_flags::FeatureFlags;
pub use monitoring::{
    AlertsConfig, LoggingConfig, MonitoringConfig, ObservabilityConfig, PerformanceConfig,
};
pub use neural_base::{EnsembleConfig, NeuralConfig, TrainingConfig};
pub use neural_enhanced::EnhancedNeuralConfig;
pub use security::{
    AuthConfig, CircuitBreakerConfig, EncryptionConfig, GracefulShutdownConfig, OAuthProvider,
    SecurityConfig,
};
