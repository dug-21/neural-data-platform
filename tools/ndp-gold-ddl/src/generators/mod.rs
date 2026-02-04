//! DDL generators for Gold layer
//!
//! This module contains generators for various Gold layer DDL:
//! - Continuous aggregates for single streams
//! - Aligned views for cross-stream correlation
//! - Refresh policies for continuous aggregates

pub mod aligned_view;
pub mod column_builder;
pub mod continuous_aggregate;
pub mod join_builder;
pub mod null_handler;
pub mod refresh_policy;

pub use aligned_view::AlignedViewGenerator;
pub use column_builder::ColumnBuilder;
pub use continuous_aggregate::ContinuousAggregateGenerator;
pub use join_builder::JoinBuilder;
pub use null_handler::{
    CarryForwardNullHandler, InterpolateNullHandler, NullHandler, PreserveNullHandler,
};
pub use refresh_policy::RefreshPolicyGenerator;
