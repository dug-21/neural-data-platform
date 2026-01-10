//! Configuration types for Neural Data Platform
//!
//! This module contains configuration structures for various platform components,
//! including Silver layer ETL configuration.

pub mod silver_etl;

pub use silver_etl::{
    ConversionFormula, DeduplicationConfig, DeduplicationStrategy, DqAction, DqOutputConfig,
    DqRule, IdentityField, IncrementalConfig, SilverConfigError, SilverEtlConfig,
    SilverFieldMapping, TimestampMapping, TimestampTransform, TransformConfig,
};
