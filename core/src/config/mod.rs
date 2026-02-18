//! Configuration types for Neural Data Platform
//!
//! This module contains configuration structures for various platform components,
//! including Silver layer ETL configuration.

pub mod mock_loader;
pub mod silver_etl;

// ConfigLoader trait and MockConfigLoader for infrastructure-free testing (dp-018)
pub use mock_loader::{ConfigLoader, ConfigLoaderError, MockConfigLoader};

pub use silver_etl::{
    // Pre-transform configuration types (dp-007)
    ArrayExplosionConfig,
    // Standard configuration types
    ConversionFormula,
    DeduplicationConfig,
    DeduplicationStrategy,
    DqAction,
    DqOutputConfig,
    DqRule,
    // Valid timestamp types
    FieldSource,
    IdentityField,
    IncrementalConfig,
    MetricExplosionMapping,
    PreTransformConfig,
    PreTransformType,
    RowIteratorConfig,
    SilverConfigError,
    SilverEtlConfig,
    SilverFieldMapping,
    TimestampMapping,
    TimestampTransform,
    TransformConfig,
    ValidTimestampMapping,
    ValidTimestampSource,
};
