//! DDL generators for Gold layer
//!
//! This module contains generators for various Gold layer DDL:
//! - Continuous aggregates for single streams
//! - Aligned views for cross-stream correlation
//! - Refresh policies for continuous aggregates
//! - Stream classification for correlation analysis (v11-002)
//! - State transitions for state_event streams (v11-006)
//! - Events hypertable for unified event storage (v11-013)

pub mod aligned_view;
pub mod classification;
pub mod column_builder;
pub mod constants;
pub mod continuous_aggregate;
pub mod events;
pub mod join_builder;
pub mod null_handler;
pub mod refresh_policy;
pub mod state_transitions;

pub use aligned_view::AlignedViewGenerator;
pub use classification::{
    generate_classification_sql, generate_gold_table_sql, ClassificationSyncer,
    DefaultClassificationSyncer,
};
pub use column_builder::ColumnBuilder;
pub use continuous_aggregate::ContinuousAggregateGenerator;
pub use events::{EventsConfig, EventsGenerator, IEventsGenerator};
pub use join_builder::JoinBuilder;
pub use null_handler::{
    CarryForwardNullHandler, InterpolateNullHandler, NullHandler, PreserveNullHandler,
};
pub use refresh_policy::RefreshPolicyGenerator;
pub use state_transitions::{
    DeviceTypeRule, ITransitionGenerator, StateTransitionGenerator, TransitionConfig,
};
