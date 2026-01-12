//! Configuration types for Neural Data Platform
//!
//! This module contains configuration structures for various platform components,
//! including Silver layer ETL configuration.

pub mod silver_etl;

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
    SilverConfigError,
    SilverEtlConfig,
    SilverFieldMapping,
    TimestampMapping,
    TimestampTransform,
    TransformConfig,
    ValidTimestampMapping,
    ValidTimestampSource,
};
