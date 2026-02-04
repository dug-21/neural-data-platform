//! ndp-gold-ddl - Gold layer DDL generation for NDP
//!
//! This library provides DDL generation for:
//! - TimescaleDB continuous aggregates for individual streams
//! - Aligned materialized views for cross-stream correlation
//!
//! ## Architecture
//!
//! The tool follows a modular design:
//! - `config`: Configuration loading and types
//! - `generators`: SQL DDL generators
//! - `error`: Structured error types
//!
//! ## Usage
//!
//! ```bash
//! # Generate Gold layer DDL for a stream
//! ndp-gold-ddl generate --stream air-quality
//!
//! # Generate aligned view for a domain
//! ndp-gold-ddl generate --domain indoor-air-quality
//!
//! # Validate configuration
//! ndp-gold-ddl validate --stream air-quality
//! ```

pub mod config;
pub mod error;
pub mod generators;
pub mod registry;
pub mod validation;

// Re-exports for convenient access
pub use config::{
    Action, AlignedStream, AlignmentConfig, ConfigLoader, DomainConfig, FileSystemConfigLoader,
    GoldEtlConfig, JoinStrategy, NullHandling, StreamConfig, StreamRef, StreamRole, StreamType,
};

pub use error::{GoldDdlError, Result};

pub use generators::{
    generate_classification_sql, generate_gold_table_sql, AlignedViewGenerator,
    ClassificationSyncer, ContinuousAggregateGenerator, DefaultClassificationSyncer,
    RefreshPolicyGenerator,
};

pub use registry::{FeatureConfig, FeatureGenerator, FeatureRegistry, SqlColumn};

pub use validation::{validate_gold_config, ConfigValidator};
