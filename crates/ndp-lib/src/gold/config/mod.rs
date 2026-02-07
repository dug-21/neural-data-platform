//! Configuration module for Gold DDL generation
//!
//! Provides types and loading functionality for Gold ETL configurations.

pub mod domain;
pub mod loader;
pub mod types;

pub use domain::{
    AlignedStream, AlignmentConfig, DomainConfig, JoinStrategy, NullHandling, ObjectiveConfig,
    Priority, StreamRef, StreamRole, StreamType, TargetConfig,
};
pub use loader::{default_loader, resolve_config_dir, ConfigLoader, FileSystemConfigLoader};
pub use types::{
    Action, AggregatesConfig, FeaturesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig,
    LagConfig, RefreshPolicyConfig, RollingConfig, SilverEtlConfig, StreamConfig, TimestampConfig,
    TransitionsConfig, TrendConfig, VALID_METRICS, VALID_ROLLING_STATS,
};
