//! Re-exports dimension types from the config module.
//!
//! The canonical definitions live in `crate::config`. This module re-exports
//! them so that consumers can write `dimension::types::DimensionConfig` if
//! they prefer.

pub use crate::config::{
    DimensionConfig, DimensionField, DimensionLoad, DimensionSchema, DimensionSource,
    DimensionTarget,
};
